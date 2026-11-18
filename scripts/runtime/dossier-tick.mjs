import { runGraphCli, tickLockHolder, tickLockOwner } from './worker-lock.mjs'
import { workerPaths } from './worker-paths.mjs'
import { existsSync, realpathSync } from 'node:fs'
import { pathToFileURL } from 'node:url'
import { boundedSpawnSync } from './bounded-child.mjs'
import { spawnSync } from 'node:child_process'

import {
  mineRepoFacts,
  ingestRepoFacts,
  compileDossier,
  proposeDossierProbe,
  assertRepoKey,
} from '../../lib/dossier/core.mjs'
import { redactText } from './private-store-fs.mjs'
import {
  commandToken,
  p0Verdict,
  p1Verdict,
  applyProbe,
} from '../../lib/dossier/probes.mjs'
export * from '../../lib/dossier/probes.mjs'
import { collectManifest } from './cut-evidence.mjs'
import { boundedDossierText } from './repo-dossier.mjs'
import {
  isIdle,
  rollBudget,
  budgetAllows,
  recordRun,
  makeHeartbeat,
  angelTtyRunning,
} from './idle.mjs'

export function killed(env = {}) {
  return String(env.ANGEL_DOSSIER ?? '').trim() === '0'
}

export function commandAvailable(token, cwd) {
  if (!token) return null
  const result = spawnSync('sh', ['-c', 'command -v "$1" >/dev/null 2>&1', 'angel-dossier-probe', token], {
    encoding: 'utf8', timeout: 5000, maxBuffer: 4096, ...(cwd ? { cwd } : {}),
  })
  return result.status === 0 ? true : result.status === 1 ? false : null
}

async function cli(argv, selectedGraph) {
  const fs = await import('./private-store-fs.mjs')
  const { execFileSync } = await import('node:child_process')
  const { fileURLToPath } = await import('node:url')
  const { dirname, join } = await import('node:path')
  const os = await import('node:os')

  const ROOT = join(dirname(fileURLToPath(import.meta.url)), '..', '..')
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
  const graphPath = selectedGraph ?? flag('--graph', workerPaths().graph)
  const ledgerPath = flag('--ledger', process.env.ANGEL_EXPERIENCE_LOG) || workerPaths().ledger
  const cutDir = flag('--cut', process.env.ANGEL_CUT_DIR || workerPaths().cut)
  const idleMs = Number(flag('--idle-min', '15')) * 60_000
  const cap = Number(flag('--cap', process.env.ANGEL_DOSSIER_MAX_PROBES_PER_DAY || '8'))
  const ritualsOn = String(process.env.ANGEL_DOSSIER_PROBE_RITUALS ?? '').trim() === '1'
  const p1TimeoutMs = Number(flag('--p1-timeout', '180')) * 1000

  const now = Date.now()
  const nowIso = new Date(now).toISOString()
  const today = nowIso.slice(0, 10)
  if (!dryRun) fs.mkdirSync(stateDir, { recursive: true })
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

  const isKilled = killed(process.env)
  const angelTty = angelTtyRunning(execFileSync)
  let newestLedgerMs = null
  try {
    newestLedgerMs = fs.statSync(ledgerPath).mtimeMs
  } catch {
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

  const holder = dryRun ? null : tickLockHolder(readJson(lockPath, null), now)
  if (holder) {
    writeHeartbeat(
      makeHeartbeat({
        now: nowIso,
        action: 'skip',
        reason: `another tick holds the lock — ${holder.reason}`,
        budget,
      }),
    )
    console.log('dossier-tick: another tick holds the lock; skipping.')
    return 0
  }
  if (!dryRun) fs.writeFileSync(lockPath, JSON.stringify(tickLockOwner(now)))

  try {
    const CausalGraph = (await import('../../lib/research/CausalGraph.js')).default
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

    const mined = mineRepoFacts(rows, collectManifest(cutDir))
    for (const [key, rec] of Object.entries(mined)) {
      if (onlyRepo && key !== onlyRepo) continue
      ingestRepoFacts(graph, key, rec, { now: nowIso })
    }

    const { ranking } = proposeDossierProbe(graph, onlyRepo, { now: nowIso, limit: maxProbes * 2 })
    const probes = []
    for (const target of ranking) {
      if (probes.length >= maxProbes || !budgetAllows(budget)) break
      const node = graph.getNode(target.hypothesisId)
      const root = node.repoRoot
      const token = commandToken(node.factText)
      const rootExists = !!root && fs.existsSync(root)
      const binaryFound = rootExists ? commandAvailable(token, root) : null
      let verdict = p0Verdict(node, { rootExists, binaryFound })

      if (ritualsOn && rootExists && binaryFound) {
        const porcelain = spawnSync('git', ['-C', root, 'status', '--porcelain'], {
          encoding: 'utf8',
        })
        const clean = porcelain.status === 0 && porcelain.stdout.trim() === ''
        if (clean) {
          if (dryRun) {
            console.log(`  would P1 re-run: \`${node.factText}\` in ${root}`)
          } else {
            const run = boundedSpawnSync('sh', ['-c', node.factText], {
              cwd: root,
              encoding: 'utf8',
              timeout: p1TimeoutMs,
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
  process.argv[1] &&
  existsSync(process.argv[1]) &&
  import.meta.url === pathToFileURL(realpathSync(process.argv[1])).href
if (isCli) {
  runGraphCli(cli, process.argv.slice(2)).then((code) => process.exit(code ?? 0))
}
