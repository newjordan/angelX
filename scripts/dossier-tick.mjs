import { runGraphCli } from './worker-lock.mjs'
import { workerPaths } from './worker-paths.mjs'
// dossier-tick — the idle-time fact verifier (Repo Dossier D4,
// docs/WORKERS.md).
//
// One headless, cron-driven tick: when the box is idle and within budget, it
// refreshes the dossier facts from the live experience ledger, picks the
// stalest/most-uncertain facts by EIG (proposeDossierProbe — scoreHypotheses
// verbatim × staleness), verifies them with cheap deterministic probes, folds
// the verdicts back through updateBeliefs, and recompiles the per-repo
// artifacts the cockpit injects at session start (D3). The sibling of
// reflex-tick (M5): same gate stack (kill switch, idle, daily budget,
// lockfile, heartbeat), pointed outward at the user's repos instead of inward
// at angel's config.
//
// Probe tiers:
//   • P0 (always allowed) — side-effect-free checks: the workspace root still
//     exists, the command's binary is still on PATH. Weak evidence for a
//     ritual (supports at low confidence), strong when something is missing.
//   • P1 (ANGEL_DOSSIER_PROBE_RITUALS=1, clean git tree only) — actually
//     re-run the fact's command in its repo, bounded by a timeout. Decisive
//     either way. Off by default: running builds unattended is a policy
//     decision, not a default.
//
// Dossier facts are PERPETUAL: updateBeliefs would conclude a hypothesis at
// confidence ≥ 0.6, but a concluded fact drops out of the scorer and would
// never be re-probed — so after every verdict the fact is flipped back to
// 'testing' and stamped lastVerifiedAt (which is what the compile-side decay
// clock reads). Evidence edges accumulate; conclusions don't.

import {
  mineRepoFacts,
  ingestRepoFacts,
  compileDossier,
  boundedDossierText,
  proposeDossierProbe,
} from './repo-dossier.mjs'
import { redactText } from './private-store-fs.mjs'
import { updateBeliefs } from './causal-loop.mjs'
import { collectManifest } from './cut-evidence.mjs'
import { assertRepoKey } from './repo-dossier.mjs'
import {
  isIdle,
  rollBudget,
  budgetAllows,
  recordRun,
  makeHeartbeat,
  angelTtyRunning,
} from './reflex-tick.mjs'

// ─── pure probe logic ────────────────────────────────────────────────────────

/** Kill switch: ANGEL_DOSSIER=0 disables the whole loop (same var as D3). */
export function killed(env = {}) {
  return String(env.ANGEL_DOSSIER ?? '').trim() === '0'
}

/**
 * The executable token a command line starts with, skipping leading VAR=val
 * assignments — the thing a P0 `which` check verifies.
 */
export function commandToken(text) {
  for (const tok of String(text ?? '')
    .trim()
    .split(/\s+/)) {
    if (/^[A-Za-z_][A-Za-z0-9_]*=/.test(tok)) continue
    return tok || null
  }
  return null
}

/**
 * Fold P0 observations into a verdict for one fact. Conservative by design:
 * presence is weak evidence (a binary on PATH says nothing about passing),
 * absence is strong (a ritual whose binary is gone cannot be the ritual; a
 * trap whose binary is gone certainly still "fails here").
 */
export function p0Verdict(fact, { rootExists, binaryFound }) {
  if (!rootExists) {
    return fact.factKind === 'trap'
      ? { verdict: 'inconclusive', confidence: 0.35, evidence: 'p0: workspace root missing' }
      : { verdict: 'contradicts', confidence: 0.7, evidence: 'p0: workspace root missing' }
  }
  if (!binaryFound) {
    return fact.factKind === 'trap'
      ? {
          verdict: 'supports',
          confidence: 0.6,
          evidence: 'p0: command binary missing — still fails',
        }
      : { verdict: 'contradicts', confidence: 0.6, evidence: 'p0: command binary not on PATH' }
  }
  return fact.factKind === 'trap'
    ? { verdict: 'inconclusive', confidence: 0.35, evidence: 'p0: present (cannot refute a trap)' }
    : { verdict: 'supports', confidence: 0.4, evidence: 'p0: root + binary present (weak)' }
}

/** Fold a P1 re-run into a verdict: decisive in both directions. */
export function p1Verdict(fact, { exit, timedOut }) {
  const failed = timedOut || exit !== 0
  const expectFail = fact.factKind === 'trap'
  const asExpected = failed === expectFail
  const how = timedOut ? 'timed out' : `exit ${exit}`
  return {
    verdict: asExpected ? 'supports' : 'contradicts',
    confidence: timedOut ? 0.7 : 0.85,
    evidence: `p1: re-ran \`${fact.factText}\` — ${how}`,
  }
}

/**
 * Apply one probe verdict to the graph: evidence edge via updateBeliefs
 * (bud:false — facts don't spawn follow-up hypotheses), then flip the fact
 * back to 'testing' and stamp lastVerifiedAt so it stays probe-able and the
 * decay clock resets. Returns the updateBeliefs changes for the heartbeat.
 */
export function applyProbe(graph, factId, verdict, { now }) {
  const { changes } = updateBeliefs(graph, {
    hypothesisId: factId,
    verdict: verdict.verdict,
    confidence: verdict.confidence,
    evidence: verdict.evidence,
    now,
    bud: false,
  })
  graph.updateNode(factId, { status: 'testing', concludedAt: null, lastVerifiedAt: now })
  return changes
}

// ─── CLI — the one tick ──────────────────────────────────────────────────────

async function cli(argv) {
  const fs = await import('./private-store-fs.mjs')
  const { execFileSync, spawnSync } = await import('node:child_process')
  const { fileURLToPath } = await import('node:url')
  const { dirname, join } = await import('node:path')
  const os = await import('node:os')

  const ROOT = join(dirname(fileURLToPath(import.meta.url)), '..')
  const flag = (n, d) => {
    const i = argv.indexOf(n)
    return i >= 0 ? argv[i + 1] : d
  }
  const has = (n) => argv.includes(n)

  const dryRun = has('--dry-run')
  const force = has('--force')
  const onlyRepo = flag('--repo')
  const maxProbes = Number(flag('--max', '3'))
  const stateDir = flag('--state-dir', workerPaths().dossier)
  const outDir = flag('--out', process.env.ANGEL_DOSSIER_DIR || workerPaths().dossier)
  const graphPath = flag('--graph', workerPaths().graph)
  const ledgerPath = flag('--ledger', process.env.ANGEL_EXPERIENCE_LOG) || workerPaths().ledger
  const cutDir = flag('--cut', process.env.ANGEL_CUT_DIR || workerPaths().cut)
  const idleMs = Number(flag('--idle-min', '15')) * 60_000
  const cap = Number(flag('--cap', process.env.ANGEL_DOSSIER_MAX_PROBES_PER_DAY || '8'))
  const ritualsOn = String(process.env.ANGEL_DOSSIER_PROBE_RITUALS ?? '').trim() === '1'
  const p1TimeoutMs = Number(flag('--p1-timeout', '180')) * 1000

  const now = Date.now()
  const nowIso = new Date(now).toISOString()
  const today = nowIso.slice(0, 10)
  fs.mkdirSync(stateDir, { recursive: true })
  const heartbeatPath = join(stateDir, 'heartbeat.json')
  const budgetPath = join(stateDir, 'budget.json')
  const lockPath = join(stateDir, 'lock')
  const readJson = (p, d) => {
    try {
      return JSON.parse(fs.readFileSync(p, 'utf8'))
    } catch {
      return d
    }
  }
  const writeHeartbeat = (hb) => fs.writeFileSync(heartbeatPath, JSON.stringify(hb, null, 2))

  // ── gates (the reflex-tick stack, dossier-flavored) ──
  const isKilled = killed(process.env)
  const angelTty = angelTtyRunning(execFileSync)
  let newestLedgerMs = null
  try {
    newestLedgerMs = fs.statSync(ledgerPath).mtimeMs
  } catch {
    /* no ledger yet */
  }
  const idle = force || isIdle({ angelTty, newestLedgerMs, now, idleMs })
  let budget = rollBudget(readJson(budgetPath, null), today, cap)

  const decision = isKilled
    ? { run: false, reason: 'disabled (ANGEL_DOSSIER=0)' }
    : !idle
      ? { run: false, reason: 'busy — live session or recent ledger activity' }
      : !budgetAllows(budget)
        ? { run: false, reason: 'daily probe budget exhausted' }
        : { run: true, reason: 'idle and within budget' }

  if (dryRun) {
    console.log(
      `dossier-tick (dry-run) · ${decision.run ? 'WOULD RUN' : 'WOULD SKIP'} — ${decision.reason}`,
    )
    console.log(
      `  gates: killed=${isKilled} idle=${idle} (angelTty=${angelTty}) budget=${budget.runs}/${budget.cap} p1=${ritualsOn ? 'on' : 'off'}`,
    )
  }

  if (!decision.run) {
    if (!dryRun) {
      writeHeartbeat(
        makeHeartbeat({ now: nowIso, action: 'skip', reason: decision.reason, budget }),
      )
      console.log(`dossier-tick: skip — ${decision.reason}`)
    }
    return 0
  }

  // ── lockfile (the graph file has no locking) ──
  const existingLock = readJson(lockPath, null)
  if (!dryRun && existingLock && now - (existingLock.ts || 0) < 60 * 60_000) {
    console.log('dossier-tick: another tick holds the lock; skipping.')
    return 0
  }
  if (!dryRun) fs.writeFileSync(lockPath, JSON.stringify({ pid: process.pid, ts: now }))

  try {
    const CausalGraph = (await import('../lib/research/CausalGraph.js')).default
    const graph = fs.existsSync(graphPath)
      ? CausalGraph.deserialize(JSON.parse(fs.readFileSync(graphPath, 'utf8')))
      : new CausalGraph()
    const rows = fs.existsSync(ledgerPath)
      ? fs
          .readFileSync(ledgerPath, 'utf8')
          .split('\n')
          .filter((l) => l.trim())
          .map((l) => {
            try {
              return JSON.parse(l)
            } catch {
              return null
            }
          })
          .filter(Boolean)
      : []

    // 1. Refresh facts from the live ledger (observational only).
    // The ledger proves recurrence; The Cut carries the direct machine verdicts
    // from source writes. The interactive miner already combines both — the
    // unattended tick must consume the same evidence pool or the flywheel stops
    // at the manifest and never materializes a dossier fact.
    const mined = mineRepoFacts(rows, collectManifest(cutDir))
    for (const [key, rec] of Object.entries(mined)) {
      if (onlyRepo && key !== onlyRepo) continue
      ingestRepoFacts(graph, key, rec, { now: nowIso })
    }

    // 2. Probe the top-EIG stale facts, within budget.
    const { ranking } = proposeDossierProbe(graph, onlyRepo, { now: nowIso, limit: maxProbes * 2 })
    const probes = []
    for (const target of ranking) {
      if (probes.length >= maxProbes || !budgetAllows(budget)) break
      const node = graph.getNode(target.hypothesisId)
      const root = node.repoRoot
      const token = commandToken(node.factText)
      const rootExists = !!root && fs.existsSync(root)
      let binaryFound = false
      if (token) {
        const which = spawnSync('which', [token], { encoding: 'utf8' })
        binaryFound = which.status === 0
      }
      let verdict = p0Verdict(node, { rootExists, binaryFound })

      // P1: re-run the command, only when explicitly enabled, the p0 pass
      // found everything present, and the repo tree is clean.
      if (ritualsOn && rootExists && binaryFound) {
        const porcelain = spawnSync('git', ['-C', root, 'status', '--porcelain'], {
          encoding: 'utf8',
        })
        const clean = porcelain.status === 0 && porcelain.stdout.trim() === ''
        if (clean) {
          if (dryRun) {
            console.log(`  would P1 re-run: \`${node.factText}\` in ${root}`)
          } else {
            const run = spawnSync('sh', ['-c', node.factText], {
              cwd: root,
              encoding: 'utf8',
              timeout: p1TimeoutMs,
              killSignal: 'SIGKILL',
            })
            const timedOut = run.error?.code === 'ETIMEDOUT'
            verdict = p1Verdict(node, { exit: run.status ?? -1, timedOut })
          }
        }
      }

      if (dryRun) {
        console.log(
          `  probe ${node.id}: ${verdict.verdict} (${verdict.confidence}) — ${verdict.evidence}`,
        )
      } else {
        applyProbe(graph, node.id, verdict, { now: nowIso })
        budget = recordRun(budget)
      }
      probes.push({ id: node.id, repoKey: node.repoKey, ...verdict })
    }

    if (dryRun) return 0

    // 3. Persist graph, recompile touched artifacts, settle the books.
    fs.writeFileSync(graphPath, JSON.stringify(graph.serialize(), null, 2))
    fs.mkdirSync(outDir, { recursive: true })
    const touched = [...new Set([...Object.keys(mined), ...probes.map((p) => p.repoKey)])].filter(
      (key) => key && (!onlyRepo || key === onlyRepo),
    )
    for (const key of touched) {
      assertRepoKey(key)
      const artifact = compileDossier(graph, key, {
        now: nowIso,
        thread: mined[key]?.thread ?? null,
      })
      fs.writeFileSync(join(outDir, `${key}.json`), boundedDossierText(artifact, redactText))
    }
    fs.writeFileSync(budgetPath, JSON.stringify(budget, null, 2))
    writeHeartbeat(
      makeHeartbeat({
        now: nowIso,
        action: probes.length ? 'ran' : 'skip',
        reason: probes.length
          ? `probed ${probes.length} fact(s), recompiled ${touched.length} dossier(s)`
          : 'no dossier facts to probe',
        budget,
        experiment: probes[0] ?? null,
      }),
    )
    console.log(
      `dossier-tick: ${probes.length} probe(s) · ${touched.length} dossier(s) recompiled · budget ${budget.runs}/${budget.cap}`,
    )
    return 0
  } finally {
    if (!dryRun) fs.rmSync(lockPath, { force: true })
  }
}

const isCli =
  process.argv[1] && (await import('node:url')).fileURLToPath(import.meta.url) === process.argv[1]
if (isCli) {
  runGraphCli(cli, process.argv.slice(2)).then((code) => process.exit(code ?? 0))
}
