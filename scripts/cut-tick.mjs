import { runGraphCli } from './worker-lock.mjs'
import { workerPaths } from './worker-paths.mjs'
// The Cut folds settled authored-file observations into the shared graph.
// Score only the final write to a path, exclude delegate scratch work, and
// keep unavailable files outside the discard denominator. Retention is
// observational: it ranks experiments but cannot establish a causal gain.
// CLI writers share worker-lock.mjs; graph writes also require the Conductor's
// lease. See docs/WORKERS.md for the worker and measurement contracts.
//
// node scripts/cut-tick.mjs --dry-run
// node scripts/cut-tick.mjs             # invoked by Conductor

import { NODE_TYPE } from '../lib/research/CausalGraph.js'
import { updateBeliefs } from './causal-loop.mjs'
import { classify } from './cut-evidence.mjs'
import {
  isIdle,
  rollBudget,
  budgetAllows,
  recordRun,
  makeHeartbeat,
  angelTtyRunning,
} from './reflex-tick.mjs'

export const CUT_PROJECT = Object.freeze({ id: 'cut', label: 'The Cut' })

/** Hours a write must sit before the tree's answer counts as a human verdict. */
export const DEFAULT_SETTLE_HOURS = 24

/** The observational ceiling. Passive logs prioritize; only a bench concludes. */
export const MAX_OBS_CONFIDENCE = 0.4

/** Stale-lock TTL, shared with every other tick in the family. */
export const LOCK_TTL_MS = 60 * 60_000

/** Verdicts. The first three are the judged corpus; the rest are exclusions. */
export const KEPT = 'KEPT'
export const EDITED = 'EDITED'
export const DISCARDED = 'DISCARDED'
export const SELF_SUPERSEDED = 'SELF-SUPERSEDED'
export const SANDBOX = 'SANDBOX'
export const UNRESOLVED = 'UNRESOLVED'
export const UNSETTLED = 'UNSETTLED'

const JUDGED = new Set([KEPT, EDITED, DISCARDED])

/** Is this row part of the corpus that faced a human verdict? */
export const isJudged = (v) => JUDGED.has(v?.verdict ?? v)

/** Kill switch: ANGEL_CUT=0 disables the whole loop. */
export function killed(env = {}) {
  return String(env.ANGEL_CUT ?? '').trim() === '0'
}

/** Angel's own delegate sandbox — rule 2. Overridable only for tests. */
export function sandboxRoot(home) {
  return `${home}/.angel0/workspace`
}

const under = (path, dir) => path === dir || String(path ?? '').startsWith(`${dir}/`)

// ─── scoring (pure) ──────────────────────────────────────────────────────────

/** Absolute path of an authored row; `path` may be repo-relative or absolute. */
export function absPath(entry) {
  const p = String(entry?.path ?? '')
  if (!p) return null
  if (p.startsWith('/')) return p
  const root = entry?.repo?.root
  return root ? `${String(root).replace(/\/+$/, '')}/${p}` : null
}

/** Settled = old enough that the tree's answer is the human's, not a work-in-progress. */
export function isSettled(entry, { nowSec, settleHours = DEFAULT_SETTLE_HOURS }) {
  const ts = Number(entry?.ts)
  if (!Number.isFinite(ts)) return false
  return nowSec - ts >= settleHours * 3600
}

/**
 * Score a whole manifest corpus. Pure over an injected `readFile(abs) -> string
 * | null` (null = unreadable, which is UNRESOLVED, never DISCARDED).
 *
 * Supersession is computed over the WHOLE corpus, not just the settled slice:
 * a write that a later write replaced is angel iterating, whether or not the
 * later one has settled yet.
 *
 * @returns {{scored: object[]}} every row with {verdict, reason, judged}.
 */
export function scoreManifest(entries, { nowSec, settleHours, home = '', readFile }) {
  const sandbox = sandboxRoot(home)
  const rows = [...(entries ?? [])]
    .filter((e) => e && typeof e.path === 'string' && typeof e.authored === 'string')
    .sort((a, b) => (a.ts ?? 0) - (b.ts ?? 0) || (a.seq ?? 0) - (b.seq ?? 0))

  // Rule 1: only the final write to a path ever faced a human verdict.
  const finalWrite = new Map()
  for (const e of rows) {
    const abs = absPath(e)
    if (abs) finalWrite.set(abs, e)
  }

  const cache = new Map()
  const currentText = (abs) => {
    if (!cache.has(abs)) cache.set(abs, readFile(abs))
    return cache.get(abs)
  }

  const scored = rows.map((e) => {
    const abs = absPath(e)
    const decide = () => {
      // Rule 2: angel grading its own homework in an empty room.
      if (!abs || under(abs, sandbox) || under(String(e.repo?.root ?? ''), sandbox)) {
        return {
          verdict: SANDBOX,
          reason: `sandbox — ${sandbox} is angel's own delegate repo; nobody judged it`,
        }
      }
      // Rule 1.
      if (finalWrite.get(abs) !== e) {
        return {
          verdict: SELF_SUPERSEDED,
          reason: 'self-superseded — angel overwrote this itself; not a human verdict',
        }
      }
      if (!isSettled(e, { nowSec, settleHours })) {
        return {
          verdict: UNSETTLED,
          reason: `unsettled — younger than the ${settleHours}h settle window`,
        }
      }
      const current = currentText(abs)
      // Rule 3: unresolvable is not discarded. A file we cannot read may have
      // been renamed, moved, or live in a tree that is not checked out — none of
      // which is a rejection. It leaves the denominator entirely.
      if (typeof current !== 'string') {
        return {
          verdict: UNRESOLVED,
          reason: 'unresolvable — the file is not readable; cannot score it',
        }
      }
      const verdict = classify(e.authored, current)
      return {
        verdict,
        reason:
          verdict === KEPT
            ? 'survived verbatim in the working tree'
            : verdict === EDITED
              ? 'survived altered — you rewrote it (the gradient)'
              : 'gone from the working tree',
      }
    }
    const { verdict, reason } = decide()
    return { ...e, abs, verdict, reason, judged: JUDGED.has(verdict) }
  })

  return { scored }
}

// ─── summary + slices (pure) ─────────────────────────────────────────────────

const count = (rows, v) => rows.filter((r) => r.verdict === v).length

/** The headline numbers, with the three exclusions kept visible and separate. */
export function summarize(scored) {
  const rows = scored ?? []
  const judged = rows.filter((r) => r.judged)
  const kept = count(judged, KEPT)
  const machine = rows.filter((r) => Number.isFinite(r.machine?.exit))
  const identified = machine.filter((r) => r.machine.verification_id)
  return {
    authored: rows.length,
    selfSuperseded: count(rows, SELF_SUPERSEDED),
    sandbox: count(rows, SANDBOX),
    unresolved: count(rows, UNRESOLVED),
    unsettled: count(rows, UNSETTLED),
    judged: judged.length,
    kept,
    edited: count(judged, EDITED),
    discarded: count(judged, DISCARDED),
    keepRate: judged.length ? kept / judged.length : null,
    // T2's machine verdict rides along when present; absent until T2 lands.
    machineVerdicts: machine.length,
    machinePass: machine.filter((r) => r.machine.exit === 0).length,
    // These are completed execution receipts, unlike per-authored-row labels.
    // Legacy records have no sharing ID and retain their historical count.
    machineExecutions:
      new Set(identified.map((r) => r.machine.verification_id)).size +
      machine.length -
      identified.length,
    sharedMachineRows: machine.filter((r) => r.machine.shared).length,
  }
}

const slug = (s) =>
  String(s ?? '')
    .toLowerCase()
    .replace(/[^0-9a-z]+/g, '-')
    .replace(/^-+|-+$/g, '')
    .slice(0, 48) || 'unknown'

/** Deterministic hypothesis id for one slice of the corpus. */
export function cutHypothesisId(kind, value) {
  return `cut_${kind}_${slug(value)}`
}

/** The slices angel can actually act on: which driver / model / repo you keep. */
export const SLICE_KINDS = Object.freeze([
  { kind: 'driver', of: (e) => e.driver, noun: 'driver' },
  { kind: 'model', of: (e) => e.model, noun: 'model' },
  { kind: 'repo', of: (e) => e.repo?.key, noun: 'repo' },
])

/**
 * Group the JUDGED rows into per-slice tallies. Excluded rows never reach a
 * denominator — that is the whole point of the three rules.
 */
export function sliceCorpus(scored) {
  const out = []
  for (const { kind, of, noun } of SLICE_KINDS) {
    const groups = new Map()
    for (const row of scored ?? []) {
      if (!row.judged) continue
      const value = of(row)
      if (!value) continue
      if (!groups.has(value)) groups.set(value, [])
      groups.get(value).push(row)
    }
    for (const [value, rows] of [...groups.entries()].sort((a, b) => (a[0] < b[0] ? -1 : 1))) {
      const kept = count(rows, KEPT)
      out.push({
        id: cutHypothesisId(kind, value),
        kind,
        noun,
        value,
        samples: rows.length,
        kept,
        edited: count(rows, EDITED),
        discarded: count(rows, DISCARDED),
        keepRate: kept / rows.length,
      })
    }
  }
  return out
}

/**
 * Observational confidence: grows with the sample count, hard-capped at 0.4.
 * Never reaches updateBeliefs' 0.6 conclusion threshold — belt AND suspenders,
 * because the verdict is always 'inconclusive' too (an unsigned `tests` edge).
 */
export function observationConfidence(samples) {
  const n = Math.max(0, Number(samples) || 0)
  return Math.min(MAX_OBS_CONFIDENCE, Math.round((0.1 + 0.05 * n) * 100) / 100)
}

// ─── graph fold ──────────────────────────────────────────────────────────────

/** Merge this batch's tally into whatever the node already observed. */
function mergeObserved(prev, slice, now) {
  const p = prev && Number.isFinite(prev.samples) ? prev : null
  const samples = (p?.samples ?? 0) + slice.samples
  const kept = (p?.kept ?? 0) + slice.kept
  const edited = (p?.edited ?? 0) + slice.edited
  const discarded = (p?.discarded ?? 0) + slice.discarded
  return {
    samples,
    kept,
    edited,
    discarded,
    keepRate: samples ? kept / samples : null,
    batch: { samples: slice.samples, kept: slice.kept, keepRate: slice.keepRate },
    observedAt: now,
  }
}

/**
 * Fold one scored batch into the graph: mint (idempotently) a keep-rate
 * hypothesis per slice, attach the cumulative observation, and record the
 * evidence through the shared engine — verdict 'inconclusive' at ≤0.4, so the
 * edge is an unsigned `tests` edge and no belief moves. Zero engine changes.
 *
 * Every fold in a tick shares ONE experiment node (created here so updateBeliefs
 * reuses it via its experimentNodeId branch) — one tick, one experiment.
 *
 * @returns {{minted:string[], folded:object[], experimentId:string|null}}
 */
export function ingestCutObservations(graph, scored, { now = new Date().toISOString() } = {}) {
  const slices = sliceCorpus(scored)
  if (!slices.length) return { minted: [], folded: [], experimentId: null }

  const summary = summarize(scored)
  const experimentId = `exp_cut_${String(now).replace(/[^0-9a-zA-Z]/g, '')}`
  if (!graph.hasNode(experimentId)) {
    graph.addNode({
      id: experimentId,
      type: NODE_TYPE.EXPERIMENT,
      label: `The Cut — ${summary.judged} judged write(s) ${now}`,
      runAt: now,
      observational: true, // NOT an intervention. It cannot conclude anything.
      evidence: `keep rate ${(summary.keepRate ?? 0).toFixed(2)} over ${summary.judged} judged write(s)`,
    })
  }

  const minted = []
  const folded = []
  for (const slice of slices) {
    if (!graph.hasNode(slice.id)) {
      graph.addNode({
        id: slice.id,
        type: NODE_TYPE.HYPOTHESIS,
        label: `${slice.noun} ${slice.value} → higher keep rate`,
        hypothesisId: slice.id.replace(/^cut_/, ''),
        projectId: CUT_PROJECT.id,
        projectLabel: CUT_PROJECT.label,
        metric: { name: 'keep_rate', direction: 'higher' },
        status: 'testing',
        proposedAt: now,
        concludedAt: null,
        question: `Does ${slice.noun} ${slice.value} produce work you keep?`,
        prediction: `Work authored on ${slice.noun} ${slice.value} survives into the tree more often than the baseline.`,
        outcome: 'neutral',
        // Domain fields ride along; the scorer ignores them.
        cutKind: slice.kind,
        cutValue: slice.value,
        confounded: true, // observational: you pick drivers by task difficulty.
        observed: null,
      })
      minted.push(slice.id)
    }

    const confidence = observationConfidence(slice.samples)
    const evidence =
      `The Cut (observational, confounded): ${slice.noun} ${slice.value} kept ` +
      `${slice.kept}/${slice.samples} (${(slice.keepRate * 100).toFixed(0)}%) — ` +
      `prioritizes only; a Control Bench A/B must conclude.`

    updateBeliefs(graph, {
      hypothesisId: slice.id,
      verdict: 'inconclusive', // → an unsigned `tests` edge. Never SUPPORTS/CONTRADICTS.
      confidence,
      evidence,
      experimentNodeId: experimentId,
      now,
      bud: false, // observations don't bud follow-ups; conclusions do, and we never conclude.
    })

    // Perpetual: a concluded node drops out of scoreHypotheses and is never
    // looked at again. updateBeliefs leaves 'inconclusive' as-is, but stamp it
    // anyway so a node concluded by some other writer returns to the scorer.
    const node = graph.getNode(slice.id)
    graph.updateNode(slice.id, {
      status: 'testing',
      concludedAt: null,
      lastVerifiedAt: now,
      observed: mergeObserved(node?.observed, slice, now),
    })
    folded.push({ id: slice.id, samples: slice.samples, keepRate: slice.keepRate, confidence })
  }

  return { minted, folded, experimentId }
}

// ─── the Conductor gate ──────────────────────────────────────────────────────

/** Does the Conductor currently hold its lock? (`{pid, ts}`, 60-min stale TTL.) */
export function conductorHoldsLock(lock, { nowMs, ttlMs = LOCK_TTL_MS } = {}) {
  const ts = Number(lock?.ts)
  if (!Number.isFinite(ts)) return false
  const age = nowMs - ts
  return age < ttlMs && age > -60_000 // tolerate a minute of clock skew
}

/**
 * May this tick write the shared graph? Only inside the Conductor's lock: the
 * graph file has no locking and the Conductor is the single serializing writer.
 * Refusing is not an error — we score, report, and leave the manifest unstamped
 * so the same evidence folds on the next Conductor-invoked pass.
 */
export function graphWriteAllowed({ conductorLock, nowMs, ttlMs = LOCK_TTL_MS } = {}) {
  if (conductorHoldsLock(conductorLock, { nowMs, ttlMs })) {
    return { allowed: true, reason: 'conductor lock held' }
  }
  return {
    allowed: false,
    reason:
      'refusing to write the causal graph outside the Conductor lock ' +
      '(causal-graph.json has no locking; the Conductor is the single writer)',
  }
}

/** The /cut-facing status artifact (T4 reads this; signal density stays on screen). */
export function statusExport(summary, slices, { now = new Date().toISOString() } = {}) {
  return {
    v: 1,
    generatedAt: now,
    summary,
    slices: slices.map((s) => ({
      kind: s.kind,
      value: s.value,
      samples: s.samples,
      kept: s.kept,
      edited: s.edited,
      discarded: s.discarded,
      keepRate: s.keepRate,
    })),
  }
}

// ─── CLI — the one tick ──────────────────────────────────────────────────────

async function cli(argv) {
  const fs = await import('./private-store-fs.mjs')
  const { execFileSync } = await import('node:child_process')
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
  const force = has('--force') // the Conductor already checked idle before spawning us
  const home = os.homedir()
  const stateDir = flag('--state-dir', process.env.ANGEL_CUT_DIR || workerPaths().cut)
  const conductorDir = flag(
    '--conductor-dir',
    process.env.ANGEL_CONDUCTOR_DIR || workerPaths().conductor,
  )
  const graphPath = flag('--graph', workerPaths().graph)
  const ledgerPath = flag('--ledger', process.env.ANGEL_EXPERIENCE_LOG) || workerPaths().ledger
  const idleMs = Number(flag('--idle-min', '15')) * 60_000
  const cap = Number(flag('--cap', process.env.ANGEL_CUT_MAX_RUNS_PER_DAY || '4'))
  const settleHours = Number(
    flag('--settle-hours', process.env.ANGEL_CUT_SETTLE_HOURS || String(DEFAULT_SETTLE_HOURS)),
  )

  const nowMs = Date.now()
  const nowSec = Math.floor(nowMs / 1000)
  const nowIso = new Date(nowMs).toISOString()
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
  const writeHeartbeat = (hb) => {
    if (!dryRun) fs.writeFileSync(heartbeatPath, JSON.stringify(hb, null, 2))
  }

  // ── gates (the reflex-tick stack, cut-flavored) ──
  const isKilled = killed(process.env)
  const angelTty = angelTtyRunning(execFileSync)
  let newestLedgerMs = null
  try {
    newestLedgerMs = fs.statSync(ledgerPath).mtimeMs
  } catch {
    /* no ledger yet */
  }
  const idle = force || isIdle({ angelTty, newestLedgerMs, now: nowMs, idleMs })
  let budget = rollBudget(readJson(budgetPath, null), today, cap)

  const decision = isKilled
    ? { run: false, reason: 'disabled (ANGEL_CUT=0)' }
    : !idle
      ? { run: false, reason: 'busy — live session or recent ledger activity' }
      : !budgetAllows(budget)
        ? { run: false, reason: 'daily cut budget exhausted' }
        : { run: true, reason: 'idle and within budget' }

  if (dryRun) {
    console.log(
      `cut-tick (dry-run) · ${decision.run ? 'WOULD RUN' : 'WOULD SKIP'} — ${decision.reason}\n` +
        `  gates: killed=${isKilled} idle=${idle} (angelTty=${angelTty}) budget=${budget.runs}/${budget.cap} settle=${settleHours}h`,
    )
  }
  if (!decision.run && !dryRun) {
    // Heartbeat on EVERY outcome, skips included — that is the watchdog seam.
    writeHeartbeat(makeHeartbeat({ now: nowIso, action: 'skip', reason: decision.reason, budget }))
    console.log(`cut-tick: skip — ${decision.reason}`)
    return 0
  }

  // ── our own lockfile: two cut ticks must never score the same manifest ──
  const existingLock = readJson(lockPath, null)
  if (!dryRun && existingLock && nowMs - (existingLock.ts || 0) < LOCK_TTL_MS) {
    writeHeartbeat(
      makeHeartbeat({
        now: nowIso,
        action: 'skip',
        reason: 'another cut tick holds the lock',
        budget,
      }),
    )
    console.log('cut-tick: another tick holds the lock; skipping.')
    return 0
  }
  if (!dryRun) fs.writeFileSync(lockPath, JSON.stringify({ pid: process.pid, ts: nowMs }))

  try {
    // ── read the T1 manifests (authored-YYYYMMDD.jsonl, one row per mutation) ──
    const shards = fs.existsSync(stateDir)
      ? fs
          .readdirSync(stateDir)
          .filter((n) => /^authored-\d{8}\.jsonl$/.test(n))
          .sort()
      : []
    const byShard = new Map()
    for (const name of shards) {
      const lines = fs.readFileSync(join(stateDir, name), 'utf8').split('\n')
      byShard.set(
        name,
        lines.map((line) => {
          if (!line.trim()) return { raw: line, entry: null }
          try {
            return { raw: line, entry: JSON.parse(line) }
          } catch {
            return { raw: line, entry: null } // malformed rows pass through untouched
          }
        }),
      )
    }
    const entries = [...byShard.values()].flatMap((rows) =>
      rows.map((r) => r.entry).filter(Boolean),
    )

    const { scored } = scoreManifest(entries, {
      nowSec,
      settleHours,
      home,
      readFile: (abs) => {
        try {
          return fs.readFileSync(abs, 'utf8')
        } catch {
          return null // unreadable → UNRESOLVED, never DISCARDED (rule 3)
        }
      },
    })
    const summary = summarize(scored)

    // Already-folded rows carry a `cut` stamp; only fresh ones become evidence.
    const fresh = scored.filter((r) => r.judged && !r.cut)
    const freshSummary = summarize(fresh)
    const slices = sliceCorpus(fresh)

    if (dryRun) {
      console.log(
        `  manifest: ${shards.length} shard(s), ${scored.length} authored write(s)\n` +
          `    self-superseded ${summary.selfSuperseded}  (rule 1: angel iterating — not a verdict)\n` +
          `    sandbox         ${summary.sandbox}  (rule 2: ~/.angel0/workspace — nobody looked)\n` +
          `    unresolvable    ${summary.unresolved}  (rule 3: cannot see the file — NOT discarded)\n` +
          `    unsettled       ${summary.unsettled}  (younger than ${settleHours}h)\n` +
          `    judged          ${summary.judged}   <-- the usable corpus\n` +
          `      KEPT ${summary.kept} / EDITED ${summary.edited} / DISCARDED ${summary.discarded}` +
          `${summary.keepRate === null ? '' : `  ·  keep rate ${(summary.keepRate * 100).toFixed(0)}%`}`,
      )
      const gate = graphWriteAllowed({
        conductorLock: readJson(join(conductorDir, 'lock'), null),
        nowMs,
      })
      console.log(
        `  would fold ${slices.length} slice(s) from ${freshSummary.judged} unfolded write(s) ` +
          `at ≤${MAX_OBS_CONFIDENCE} confidence, no signed edges\n` +
          `  graph: ${gate.allowed ? 'WOULD WRITE (conductor lock held)' : `WOULD REFUSE — ${gate.reason}`}`,
      )
      for (const s of slices) {
        console.log(
          `    ${s.kind} ${s.value}: ${s.kept}/${s.samples} kept ` +
            `(conf ${observationConfidence(s.samples)}) → ${s.id}`,
        )
      }
      return 0
    }

    // ── the graph gate: the Conductor is the single serializing writer ──
    const gate = graphWriteAllowed({
      conductorLock: readJson(join(conductorDir, 'lock'), null),
      nowMs,
    })

    // The status artifact and the heartbeat are ours alone — they are written
    // whether or not we may touch the graph, so /cut and the watchdog never go
    // blind just because the Conductor is not running.
    fs.writeFileSync(
      join(stateDir, 'status.json'),
      JSON.stringify(statusExport(summary, sliceCorpus(scored), { now: nowIso }), null, 2),
    )

    if (!gate.allowed) {
      writeHeartbeat(makeHeartbeat({ now: nowIso, action: 'skip', reason: gate.reason, budget }))
      console.log(
        `cut-tick: scored ${summary.judged} judged write(s) — ${gate.reason}. ` +
          `Manifest left unstamped; the next Conductor pass folds it.`,
      )
      return 0
    }

    const graphMod = await import('../lib/research/CausalGraph.js')
    const CausalGraph = graphMod.default
    const graph = fs.existsSync(graphPath)
      ? CausalGraph.deserialize(JSON.parse(fs.readFileSync(graphPath, 'utf8')))
      : new CausalGraph()
    const { minted, folded } = ingestCutObservations(graph, fresh, { now: nowIso })
    fs.writeFileSync(graphPath, JSON.stringify(graph.serialize(), null, 2))

    // Stamp the folded rows so the next tick cannot double-count them. Only
    // after the graph is on disk — the habitsmith spool discipline.
    const stampedIds = new Set(fresh.map((r) => `${r.session}:${r.seq}:${r.ts}:${r.path}`))
    for (const [name, rows] of byShard) {
      let touched = false
      const out = rows.map(({ raw, entry }) => {
        if (!entry) return raw
        const key = `${entry.session}:${entry.seq}:${entry.ts}:${entry.path}`
        if (!stampedIds.has(key)) return raw
        const row = scored.find((r) => `${r.session}:${r.seq}:${r.ts}:${r.path}` === key)
        touched = true
        return JSON.stringify({
          ...entry,
          cut: { verdict: row.verdict, reason: row.reason, scoredAt: nowSec },
        })
      })
      if (touched) {
        const p = join(stateDir, name)
        fs.writeFileSync(`${p}.tmp`, `${out.filter((l) => l.trim()).join('\n')}\n`)
        fs.renameSync(`${p}.tmp`, p)
      }
    }

    budget = recordRun(budget)
    fs.writeFileSync(budgetPath, JSON.stringify(budget, null, 2))
    const reason =
      `folded ${folded.length} slice(s) (${minted.length} new) from ${freshSummary.judged} write(s) · ` +
      `keep rate ${freshSummary.keepRate === null ? 'n/a' : `${(freshSummary.keepRate * 100).toFixed(0)}%`} · ` +
      `excluded ${summary.selfSuperseded} superseded / ${summary.sandbox} sandbox / ${summary.unresolved} unresolvable`
    writeHeartbeat(
      makeHeartbeat({
        now: nowIso,
        action: folded.length ? 'ran' : 'skip',
        reason: folded.length ? reason : 'no settled, unfolded writes to score',
        budget,
        experiment: folded[0] ?? null,
      }),
    )
    console.log(`cut-tick: ${folded.length ? reason : 'no settled, unfolded writes to score'}`)
    return 0
  } catch (err) {
    writeHeartbeat(
      makeHeartbeat({ now: nowIso, action: 'error', reason: err?.message || String(err), budget }),
    )
    console.error(`cut-tick: error — ${err?.message || err}`)
    return 1
  } finally {
    if (!dryRun) fs.rmSync(lockPath, { force: true })
  }
}

const isCli =
  process.argv[1] && (await import('node:url')).fileURLToPath(import.meta.url) === process.argv[1]
if (isCli) {
  runGraphCli(cli, process.argv.slice(2)).then((code) => process.exit(code ?? 0))
}
