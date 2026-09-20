import { runBenchmark } from './benchmark-runner.mjs'
import { boundedSpawnSync } from './bounded-child.mjs'
import { runGraphCli } from './worker-lock.mjs'
import { workerPaths } from './worker-paths.mjs'
// One serial scheduler tick: fold evidence, rank agenda items and dispatch at
// most one worker. Off is the default; measure folds/ranks without dispatch.
// Armed dispatch requires sufficient evidence for its rung (machine outcomes
// for config, retained-write observations for skills/knowledge/code).
// Persist before spawning and reload after: a parent must never overwrite the
// child's graph with its earlier in-memory copy. See conductor-tick.test.mjs.

import {
  isIdle,
  rollBudget,
  budgetAllows,
  recordRun,
  makeHeartbeat,
  angelTtyRunning,
  binaryStale,
  sourceBoundEnv,
} from './reflex-tick.mjs'
import { mineAgenda, ingestAgenda, proposeAgenda, agendaNodes } from './conductor.mjs'
import { updateBeliefs } from './causal-loop.mjs'
import { acquireProcessLease, holdProcessLease } from '../../lib/control/process-lease.mjs'
import {
  missionAllowsDispatch,
  missionGateReason,
  parseMission,
  tickMission,
  persistMission,
} from './mission.mjs'

export const DEFAULT_WINDOW = '02:00-07:00'
export const DEFAULT_COOLDOWN_H = 48
export const LOCK_TTL_MS = 60 * 60_000

/**
 * TASTE floor: human-judged writes (`cut-corpus --json` → writes.judged) required
 * before a keep-rate-driven rung may dispatch. 100, not the original 300: at the
 * measured 0.70 judged writes/day, 300 is 1.2 years away. Taste is sparse by
 * nature; the floor has to be reachable or the rung is simply off forever.
 */
export const DEFAULT_MIN_CORPUS = 100

/**
 * CORRECTNESS floor: machine-labeled writes (`cut-corpus --json` → machine.labeled)
 * required before a rung that learns from exit codes may dispatch. Loop 0 stamps
 * one on every source write, so this fills at roughly the authoring rate rather
 * than the review rate.
 */
export const DEFAULT_MIN_MACHINE = 300

/**
 * What each rung actually learns from — the axis the gate tiers on.
 *
 * `config` (Reflex) tunes knobs like ANGEL_SPIN_LIMIT against build/test outcomes:
 * it needs to know whether the code compiled, not whether you liked it. Everything
 * else in the ladder reasons from keep rate, which only the human can supply.
 */
export const RUNG_EVIDENCE = Object.freeze({
  config: 'machine', // correctness: did it build?
  skills: 'taste', // keep-rate driven
  knowledge: 'taste',
  code: 'taste',
  weights: 'taste',
})

/** An unknown rung gets the strictest tier. A new rung must opt IN to the fast one. */
export function rungEvidence(rung) {
  return RUNG_EVIDENCE[String(rung ?? '')] ?? 'taste'
}

/**
 * Arming state. Default off; `1` arms dispatch; `measure` runs the whole night
 * path and dispatches nothing. Anything else is off, deliberately — a typo must
 * fail into the state that cannot act.
 */
export function conductorMode(env = {}) {
  const raw = String(env.ANGEL_CONDUCTOR ?? '')
  if (raw === '1') return 'armed'
  if (raw.trim().toLowerCase() === 'measure') return 'measure'
  return 'off'
}

/** The tick does nothing but write a skip heartbeat unless armed or measuring. */
export function killed(env = {}) {
  return conductorMode(env) === 'off'
}

function minuteOfDay(hhmm) {
  const m = /^(\d{1,2}):(\d{2})$/.exec(String(hhmm ?? '').trim())
  if (!m) return null
  const h = Number(m[1])
  const min = Number(m[2])
  if (h < 0 || h > 23 || min < 0 || min > 59) return null
  return h * 60 + min
}

/**
 * Night-window check. Empty string means anytime (tests/manual). Ranges can wrap
 * midnight, e.g. 22:00-03:00.
 */
export function inWindow(now = new Date(), window = DEFAULT_WINDOW) {
  if (window === '') return true
  const [startRaw, endRaw] = String(window || DEFAULT_WINDOW).split('-', 2)
  const start = minuteOfDay(startRaw)
  const end = minuteOfDay(endRaw)
  if (start == null || end == null) return false
  const cur = now.getHours() * 60 + now.getMinutes()
  if (start === end) return true
  if (start < end) return cur >= start && cur < end
  return cur >= start || cur < end
}

export function decideTick({ isKilled, windowOk, idle, budgetOk }) {
  if (isKilled) return { run: false, reason: 'disabled (ANGEL_CONDUCTOR is not 1 or measure)' }
  if (!windowOk) return { run: false, reason: 'outside conductor window' }
  if (!idle) return { run: false, reason: 'busy - live session or recent ledger activity' }
  if (!budgetOk) return { run: false, reason: 'daily conductor budget exhausted' }
  return { run: true, reason: 'idle, in window, and within budget' }
}

/**
 * One tier of the density gate.
 *
 * FAILS CLOSED. An unreadable corpus is an unknown corpus, and we do not dispatch
 * onto an unknown corpus — the failure this gate exists to prevent (a learner fed
 * on 17 data points) looks exactly like a learner fed on a number we could not
 * read. Only an explicit floor of 0 disables a tier.
 */
function tier({ kind, noun, knob, count, floor }) {
  const f = Number(floor)
  const n = Number(count)
  if (!(f > 0)) {
    return {
      ok: true,
      kind,
      knob,
      floor: 0,
      count: Number.isFinite(n) ? n : null,
      reason: `${kind} gate disabled (${knob}=0)`,
    }
  }
  if (!Number.isFinite(n)) {
    return {
      ok: false,
      kind,
      knob,
      floor: f,
      count: null,
      reason:
        `cannot read the ${noun} - refusing to dispatch onto an unknown substrate ` +
        `(run \`node scripts/runtime/cut-corpus.mjs\` to see it)`,
    }
  }
  if (n < f) {
    return {
      ok: false,
      kind,
      knob,
      floor: f,
      count: n,
      reason:
        `${noun} is ${n}, below the ${f} required to dispatch a ${kind} rung ` +
        `(${knob}) - the substrate is too thin to learn from`,
    }
  }
  return { ok: true, kind, knob, floor: f, count: n, reason: `${noun} ${n} >= ${f}` }
}

/**
 * The density gate (tiered by the evidence each worker consumes),
 * tiered by the evidence a rung consumes. See the header: one floor for the
 * machine's verdict (fast, dense, correctness) and one for the human's (slow,
 * sparse, taste). `error` is a corpus-wide read failure and fails BOTH tiers;
 * a single missing count fails only its own.
 */
export function densityGate({
  judged,
  machine,
  minCorpus = DEFAULT_MIN_CORPUS,
  minMachine = DEFAULT_MIN_MACHINE,
  error = null,
} = {}) {
  const seen = (v) => (error ? NaN : v)
  return {
    error: error ?? null,
    taste: tier({
      kind: 'taste',
      noun: 'judged corpus',
      knob: 'ANGEL_CUT_MIN_CORPUS',
      count: seen(judged),
      floor: minCorpus,
    }),
    machine: tier({
      kind: 'correctness',
      noun: 'machine-labeled corpus',
      knob: 'ANGEL_CUT_MIN_MACHINE',
      count: seen(machine),
      floor: minMachine,
    }),
  }
}

/** The tier that governs this rung. No rung, no proposal: assume the strictest. */
export function gateFor(gate, rung) {
  return rungEvidence(rung) === 'machine' ? gate?.machine : gate?.taste
}

/** Could an armed tick dispatch ANYTHING on this substrate? (Cheap short-circuit.) */
export function anyTierOpen(gate) {
  return Boolean(gate?.taste?.ok || gate?.machine?.ok)
}

/**
 * What this tick may actually do with the rung it picked. An armed tick that
 * fails its rung's tier is demoted to measure — it still folds, ranks and
 * reports, it just does not act. `tier` carries the reason and the knob that
 * moves it, so every refusal names both.
 */
export function gatedMode(mode, gate, rung = null) {
  if (mode !== 'armed') return { mode, gated: false, tier: null }
  const t = gateFor(gate, rung)
  if (t?.ok) return { mode: 'armed', gated: false, tier: t }
  return { mode: 'measure', gated: true, tier: t ?? null }
}

/** Agenda nodes dispatched within the cooldown window are excluded from ranking. */
export function cooldownExcludes(
  graph,
  { nowMs = Date.now(), cooldownH = DEFAULT_COOLDOWN_H } = {},
) {
  const cutoff = nowMs - cooldownH * 60 * 60_000
  return new Set(
    agendaNodes(graph)
      .filter((n) => {
        const t = Date.parse(n.lastDispatchedAt || '')
        return Number.isFinite(t) && t > cutoff
      })
      .map((n) => n.id),
  )
}

/**
 * Pure route planner. The caller performs the spawn/append. Returns:
 *   {kind:'spawn', cmd, args}
 *   {kind:'briefing', text}
 *   {kind:'skip', reason}
 */
export function dispatchPlan(proposal, { root, env = process.env } = {}) {
  if (!proposal) return { kind: 'skip', reason: 'no open agenda item' }
  const rung = proposal.rung
  if (rung === 'config') {
    return { kind: 'spawn', cmd: 'node', args: [`${root}/scripts/runtime/reflex-tick.mjs`, '--force'] }
  }
  if (rung === 'skills') {
    return { kind: 'spawn', cmd: 'node', args: [`${root}/scripts/runtime/habitsmith-tick.mjs`] }
  }
  if (rung === 'knowledge') {
    return { kind: 'spawn', cmd: 'node', args: [`${root}/scripts/runtime/dossier-tick.mjs`] }
  }
  if (rung === 'code') {
    const driver = String(env.ANGEL_CONDUCTOR_DRIVER ?? '').trim()
    if (!driver) {
      return { kind: 'skip', reason: 'code rung requires ANGEL_CONDUCTOR_DRIVER' }
    }
    return {
      kind: 'spawn',
      cmd: 'node',
      args: [`${root}/scripts/runtime/conductor-code-run.mjs`, '--agenda', proposal.hypothesisId],
    }
  }
  if (rung === 'weights') {
    return {
      kind: 'briefing',
      text:
        `- ${new Date().toISOString()} weights agenda (briefing only): ` +
        `${proposal.goal} (${proposal.hypothesisId})\n`,
    }
  }
  return { kind: 'skip', reason: `unknown conductor rung: ${rung}` }
}

export function statusExport(ranking, { now = new Date().toISOString() } = {}) {
  return {
    v: 1,
    generatedAt: now,
    ranking: ranking.slice(0, 5).map((r) => ({
      hypothesisId: r.hypothesisId,
      rung: r.rung,
      goal: r.goal,
      priority: r.priority,
      severity: r.severity,
      estCostMin: r.estCostMin,
      evidenceRefs: r.evidenceRefs ?? [],
    })),
  }
}

export function foldVerdicts(graph, verdicts, { now = new Date().toISOString() } = {}) {
  const folded = []
  const orphaned = []
  for (const v of verdicts || []) {
    const fact = v?.fact
    const action = v?.action
    if (!fact || !graph.hasNode(fact)) {
      orphaned.push(v)
      continue
    }
    const verdict =
      action === 'approve' ? 'supports' : action === 'reject' ? 'contradicts' : 'inconclusive'
    if (verdict === 'inconclusive') {
      orphaned.push(v)
      continue
    }
    updateBeliefs(graph, {
      hypothesisId: fact,
      verdict,
      confidence: 0.9,
      evidence: `conductor ${action}: ${v.name ?? fact}`,
      now,
      bud: false,
    })
    graph.updateNode(fact, {
      status: 'testing',
      concludedAt: null,
      lastVerifiedAt: now,
    })
    folded.push({ ...v, verdict })
  }
  return { folded, orphaned }
}

export function reportAccuracy(report) {
  const vals = (report?.metrics || [])
    .map((m) => m.accuracy_pct)
    .filter((n) => typeof n === 'number' && Number.isFinite(n))
  if (!vals.length) return null
  return vals.reduce((a, b) => a + b, 0) / vals.length
}

export function metricDeltaVerdict(beforeReport, afterReport, { minDelta = 1.0 } = {}) {
  const baseline = reportAccuracy(beforeReport)
  const treatment = reportAccuracy(afterReport)
  const identity = (report) => report?.control?.evaluation_sha256
  const subjects = (report) =>
    JSON.stringify((report?.metrics || []).map((row) => row.subject).sort())
  const comparable =
    /^[0-9a-f]{64}$/.test(identity(beforeReport) || '') &&
    identity(beforeReport) === identity(afterReport) &&
    beforeReport.control.suite === afterReport.control.suite &&
    subjects(beforeReport) === subjects(afterReport)
  if (!comparable || !Number.isFinite(baseline) || !Number.isFinite(treatment)) {
    return {
      verdict: 'inconclusive',
      confidence: 0.35,
      metrics: { baseline: null, treatment: null, delta: null },
      evidence:
        'Conductor measurement lacks comparable task/verifier identities, subjects, or numeric accuracy.',
    }
  }
  const delta = treatment - baseline
  const verdict =
    delta >= Math.abs(minDelta)
      ? 'supports'
      : delta <= -Math.abs(minDelta)
        ? 'contradicts'
        : 'inconclusive'
  return {
    verdict,
    confidence: verdict === 'inconclusive' ? 0.45 : 0.8,
    metrics: { baseline, treatment, delta },
    evidence: `Conductor accuracy ${treatment.toFixed(2)}% vs recorded baseline ${baseline.toFixed(2)}% (delta=${delta.toFixed(2)} percentage points). Threshold-based graph confidence; not statistical confidence or proof of causation.`,
  }
}

async function cli(argv) {
  const fs = await import('./private-store-fs.mjs')
  const { execFileSync, spawnSync } = await import('node:child_process')
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
  const noBuild = has('--no-build')
  const stateDir = flag('--state-dir', process.env.ANGEL_CONDUCTOR_DIR || workerPaths().conductor)
  const graphPath = flag('--graph', workerPaths().graph)
  const ledgerPath = flag('--ledger', process.env.ANGEL_EXPERIENCE_LOG) || workerPaths().ledger
  const reportsDir = flag('--reports-dir', workerPaths().reports)
  const habitsStatusPath = flag('--habits-status', join(workerPaths().habits, 'status.json'))
  const proposalsPath = flag('--proposals', join(workerPaths().reflex, 'proposals.md'))
  const idleMs = Number(flag('--idle-min', '15')) * 60_000
  const cap = Number(flag('--cap', process.env.ANGEL_CONDUCTOR_MAX_RUNS_PER_DAY || '2'))
  const windowSpec = flag('--window', process.env.ANGEL_CONDUCTOR_WINDOW ?? DEFAULT_WINDOW)
  const cooldownH = Number(flag('--cooldown-h', process.env.ANGEL_CONDUCTOR_COOLDOWN_H || '48'))
  const floor = (raw, dflt) =>
    String(raw ?? '').trim() !== '' && Number.isFinite(Number(raw)) ? Number(raw) : dflt
  const minCorpus = floor(
    flag('--min-corpus', process.env.ANGEL_CUT_MIN_CORPUS),
    DEFAULT_MIN_CORPUS,
  )
  const minMachine = floor(
    flag('--min-machine', process.env.ANGEL_CUT_MIN_MACHINE),
    DEFAULT_MIN_MACHINE,
  )
  const binPath = flag(
    '--angel-bin',
    process.env.ANGEL_BIN || join(ROOT, 'cockpit/target/release/angel'),
  )

  const now = Date.now()
  const nowIso = new Date(now).toISOString()
  const today = nowIso.slice(0, 10)
  fs.mkdirSync(stateDir, { recursive: true })
  const heartbeatPath = join(stateDir, 'heartbeat.json')
  const budgetPath = join(stateDir, 'budget.json')
  const lockPath = join(stateDir, 'lock')
  const leaseDir = join(stateDir, 'lock.lease')
  const statusPath = join(stateDir, 'status.json')
  const briefingPath = join(stateDir, 'briefing.md')
  const recoveryPath = join(stateDir, 'recoveries.jsonl')
  const spoolPath = join(stateDir, 'verdicts.jsonl')

  const loadActiveMission = () => {
    let names
    try {
      names = fs.readdirSync(process.env.ANGEL_MISSION_DIR || workerPaths().missions)
    } catch {
      return null
    }
    const missions = names
      .filter((n) => n.endsWith('.json'))
      .map((n) => {
        try {
          return parseMission(
            fs.readFileSync(
              join(process.env.ANGEL_MISSION_DIR || workerPaths().missions, n),
              'utf8',
            ),
          )
        } catch {
          return null
        }
      })
      .filter(Boolean)
      .sort((a, b) => b.updatedAt.localeCompare(a.updatedAt))
    return missions[0] ?? null
  }
  const missionSummaryForTick = (m) =>
    `[${m.status}] r${m.roundsStarted}/${m.maxRounds} ${m.objective.slice(0, 48)}`

  const readJson = (p, d) => {
    try {
      return JSON.parse(fs.readFileSync(p, 'utf8'))
    } catch {
      return d
    }
  }
  const readJsonl = (p) => {
    try {
      return fs
        .readFileSync(p, 'utf8')
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
    } catch {
      return []
    }
  }
  let truncateVerdicts = false
  const persistGraph = (graph) => {
    fs.writeFileSync(graphPath, JSON.stringify(graph.serialize(), null, 2))
    if (truncateVerdicts) fs.writeFileSync(spoolPath, '')
  }
  const appendBrief = (line) => {
    if (dryRun) return
    fs.appendFileSync(briefingPath, line.endsWith('\n') ? line : `${line}\n`)
  }
  const writeHeartbeat = (hb) => fs.writeFileSync(heartbeatPath, JSON.stringify(hb, null, 2))
  const skip = (reason, budget, { publish = true } = {}) => {
    if (!dryRun && publish) {
      writeHeartbeat(makeHeartbeat({ now: nowIso, action: 'skip', reason, budget }))
      appendBrief(`- ${nowIso} skip: ${reason}`)
    }
    console.log(`conductor-tick: skip - ${reason}`)
    return 0
  }

  const mode = conductorMode(process.env)
  const isKilled = mode === 'off'
  const windowOk = force || inWindow(new Date(now), windowSpec)
  const angelTty = angelTtyRunning(execFileSync)
  let newestLedgerMs = null
  try {
    newestLedgerMs = fs.statSync(ledgerPath).mtimeMs
  } catch {
    /* no ledger yet */
  }
  const idle = force || isIdle({ angelTty, newestLedgerMs, now, idleMs })
  let budget = rollBudget(readJson(budgetPath, null), today, cap)
  const decision = decideTick({
    isKilled,
    windowOk,
    idle,
    budgetOk: budgetAllows(budget),
  })

  if (dryRun) {
    console.log(
      `conductor-tick (dry-run) - ${decision.run ? 'WOULD RUN' : 'WOULD SKIP'} - ${decision.reason}`,
    )
    console.log(
      `  gates: mode=${mode} killed=${isKilled} window=${windowOk} idle=${idle} (angelTty=${angelTty}) budget=${budget.runs}/${budget.cap}`,
    )
  }
  if (!decision.run) return skip(decision.reason, budget)

  const lease = dryRun
    ? null
    : acquireProcessLease({
        leaseDir,
        compatibilityPath: lockPath,
        ttlMs: LOCK_TTL_MS,
        nowMs: now,
      })
  if (lease && !lease.acquired) {
    // A contender must not overwrite the live owner's heartbeat or briefing.
    return skip(`another conductor tick holds the lock (${lease.reason})`, budget, {
      publish: false,
    })
  }
  if (lease?.recovered) {
    fs.appendFileSync(
      recoveryPath,
      `${JSON.stringify({
        schema: 'angel0-conductor-recovery/v1',
        ts: nowIso,
        successor_pid: process.pid,
        prior_pid: lease.recovered.prior_pid,
        prior_host: lease.recovered.prior_host,
        reason: lease.recovered.reason,
      })}\n`,
    )
    appendBrief(
      `- ${nowIso} recovery: reclaimed conductor pid ${lease.recovered.prior_pid ?? 'unknown'} ` +
        `(${lease.recovered.reason})`,
    )
  }

  const run = async () => {
    try {
      // ── the density gate  ──
      // Arming is necessary but not sufficient. Someone will eventually flip
      // ANGEL_CONDUCTOR=1 early; this is what stops that from minting confident
      // agenda nodes out of 17 data points. cut-audit is read-only and ~0.2s, and it
      // reports BOTH substrates: what the machine judged (dense, fast, correctness)
      // and what you judged (sparse, slow, taste). Read them once here; which one
      // actually governs is decided per-rung, once we know what we picked.
      const auditCorpus = () => {
        const audit = spawnSync(
          process.execPath,
          [join(ROOT, 'scripts/runtime/cut-corpus.mjs'), '--json', '--cut', workerPaths().cut],
          {
            cwd: ROOT,
            encoding: 'utf8',
            timeout: 120_000,
          },
        )
        if (audit.status !== 0) {
          return { error: `cut-audit exited ${audit.status ?? audit.error?.message ?? 'unknown'}` }
        }
        let parsed
        try {
          parsed = JSON.parse(audit.stdout)
        } catch {
          return { error: 'cut-corpus --json produced unparseable output' }
        }
        // A missing count is NaN, not 0 — it fails its own tier closed without
        // pretending the corpus is empty (an old cut-audit with no machine block
        // must not read as "zero machine labels", which is a claim it never made).
        return {
          judged: Number(parsed?.writes?.judged),
          machine: Number(parsed?.machine?.labeled),
        }
      }
      const gate =
        mode === 'armed' && (minCorpus > 0 || minMachine > 0)
          ? densityGate({ ...auditCorpus(), minCorpus, minMachine })
          : densityGate({ judged: null, machine: null, minCorpus: 0, minMachine: 0 })
      if (dryRun && mode === 'armed') {
        console.log(`  density (taste):       ${gate.taste.reason}`)
        console.log(`  density (correctness): ${gate.machine.reason}`)
      }

      const CausalGraph = (await import('../../lib/research/CausalGraph.js')).default
      const loadGraph = () =>
        fs.existsSync(graphPath)
          ? CausalGraph.deserialize(JSON.parse(fs.readFileSync(graphPath, 'utf8')))
          : new CausalGraph()
      // A child tick spawned below is a graph writer too, and it runs INSIDE this
      // lock. Hand it our pending mutations first (persistGraph, so the verdict
      // spool settles with them — the habitsmith discipline), then take back what
      // it wrote; otherwise the persist at the end of this tick overwrites the
      // child's evidence with our stale in-memory copy.
      const handOffGraph = () => persistGraph(graph)
      let graph = loadGraph()
      const verdicts = readJsonl(spoolPath)
      const folded = dryRun
        ? { folded: [], orphaned: [] }
        : foldVerdicts(graph, verdicts, { now: nowIso })
      truncateVerdicts = !dryRun && verdicts.length > 0
      if (dryRun && verdicts.length) {
        console.log(`  would fold ${verdicts.length} conductor verdict(s)`)
      }

      // ── The Cut  ──
      // Fold the human verdict on angel's authored diffs: what survived into the
      // tree. This is a MEASUREMENT step, not an agenda item — like measureApprovals
      // below, it runs every tick rather than competing for a dispatch slot. It runs
      // here because causal-graph.json has no file locking and we are the single
      // serializing writer: cut-tick REFUSES to touch the graph unless it can see
      // this lockfile, so this is the only path that ever folds it. ANGEL_CUT=0
      // disarms it.
      if (!dryRun) {
        handOffGraph()
        const cut = spawnSync(
          // NOT the bare string 'node'. This tick's only real caller is cron, whose
          // PATH is /usr/bin:/bin — and on this box node lives under nvm, nowhere
          // near it. A bare 'node' here ENOENTs at 02:05 and the fold silently never
          // happens. Spawn the interpreter that is already running us.
          process.execPath,
          [
            join(ROOT, 'scripts/runtime/cut-tick.mjs'),
            '--force', // we already cleared the idle gate
            '--graph',
            graphPath,
            '--conductor-dir',
            stateDir, // where cut-tick looks for the lock we are holding
            '--ledger',
            ledgerPath,
          ],
          { cwd: ROOT, stdio: 'inherit', env: { ...process.env, ANGEL_CAUSAL_GRAPH: graphPath } },
        )
        graph = loadGraph()
        if (cut.status !== 0) {
          appendBrief(`- ${nowIso} cut-tick exited ${cut.status ?? 'unknown'}`)
        }
      }
      const loadLedger = () =>
        fs.existsSync(ledgerPath)
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
      const loadReports = () =>
        fs.existsSync(reportsDir)
          ? fs
              .readdirSync(reportsDir)
              .filter((f) => /^benchmark-[0-9a-f-]+\.json$/.test(f))
              .sort()
              .map((f) => {
                try {
                  return { file: f, ...JSON.parse(fs.readFileSync(join(reportsDir, f), 'utf8')) }
                } catch {
                  return null
                }
              })
              .filter(Boolean)
          : []
      const readMaybeJson = (p) => {
        try {
          return JSON.parse(fs.readFileSync(p, 'utf8'))
        } catch {
          return null
        }
      }
      const readMaybeText = (p) => {
        try {
          return fs.readFileSync(p, 'utf8')
        } catch {
          return ''
        }
      }
      const newestReport = (suite) => {
        if (!fs.existsSync(reportsDir)) return null
        return (
          fs
            .readdirSync(reportsDir)
            .filter((f) => /^benchmark-[0-9a-f-]+\.json$/.test(f))
            .map((file) => {
              const path = join(reportsDir, file)
              try {
                return {
                  file,
                  path,
                  mtime: fs.statSync(path).mtimeMs,
                  ...JSON.parse(fs.readFileSync(path, 'utf8')),
                }
              } catch {
                return null
              }
            })
            .filter((row) => row && row.control?.suite === suite)
            .sort((a, b) => b.mtime - a.mtime)[0] ?? null
        )
      }
      const measureApprovals = (facts) => {
        if (!facts.length) return null
        if (!process.env.ANGEL_BENCHMARK_COMMAND)
          return {
            skipped:
              'ANGEL_BENCHMARK_COMMAND is not configured; approval is recorded without a performance verdict',
          }
        const suite = process.env.ANGEL_CONDUCTOR_BENCHMARK_SUITE || 'conductor'
        const before = newestReport(suite)
        const measured = runBenchmark(
          { suite, seeds: 2, quick: true, subjects: [{ name: 'current', env: {} }] },
          { reportsDir },
        )
        const after = { ...measured.report, file: measured.file }
        if (!before) return { skipped: 'first benchmark retained; no pre-merge comparison exists' }
        const measurement = metricDeltaVerdict(before, after)
        for (const fact of facts) {
          if (!graph.hasNode(fact)) continue
          updateBeliefs(graph, {
            hypothesisId: fact,
            verdict: measurement.verdict,
            confidence: measurement.confidence,
            evidence: measurement.evidence,
            now: nowIso,
            bud: false,
          })
          const node = graph.getNode(fact)
          graph.updateNode(fact, {
            status: 'testing',
            concludedAt: null,
            lastVerifiedAt: nowIso,
            observed: {
              ...(node.observed || {}),
              measurement: {
                ...measurement.metrics,
                verdict: measurement.verdict,
                confidence: measurement.confidence,
                before: before.file,
                after: after.file,
                measuredAt: nowIso,
              },
            },
          })
        }
        return { measured: facts.length, verdict: measurement.verdict, after: after.file }
      }

      const approvedFacts = folded.folded
        .filter((v) => v.action === 'approve' && v.fact)
        .map((v) => v.fact)
      const measurement = dryRun ? null : measureApprovals(approvedFacts)
      if (measurement?.skipped) {
        console.log(`conductor-tick: measurement skipped - ${measurement.skipped}`)
        appendBrief(`- ${nowIso} measurement skipped: ${measurement.skipped}`)
      } else if (measurement?.measured) {
        appendBrief(
          `- ${nowIso} measurement: ${measurement.measured} approved agenda node(s), verdict ${measurement.verdict}, report ${measurement.after}`,
        )
      }

      const candidates = mineAgenda(
        {
          rows: loadLedger(),
          reports: loadReports(),
          habitsStatus: readMaybeJson(habitsStatusPath),
          proposalsMd: readMaybeText(proposalsPath),
          graph,
        },
        { now: nowIso },
      )
      ingestAgenda(graph, candidates, { now: nowIso })
      const exclude = cooldownExcludes(graph, { nowMs: now, cooldownH })
      const { proposal, ranking } = proposeAgenda(graph, { limit: 12, exclude })
      const status = statusExport(ranking, { now: nowIso })
      if (!dryRun) fs.writeFileSync(statusPath, JSON.stringify(status, null, 2))

      // ── which floor applies, now that we know what we would dispatch ──
      // The gate is tiered by the evidence the rung consumes (see the header): a
      // config rung learns from exit codes and may go on the machine's corpus alone;
      // a keep-rate rung may not. With no proposal there is no rung, and rungEvidence
      // falls back to the strictest tier.
      const gateResult = gatedMode(mode, gate, proposal?.rung ?? null)
      const gated = gateResult.gated
      const gateTier = gateResult.tier
      let runMode = gateResult.mode
      const gateWhy = gateTier?.reason ?? 'no governing tier'
      if (gated) {
        console.log(`conductor-tick: DENSITY GATE - ${gateWhy}`)
        console.log(
          'conductor-tick: armed, but REFUSING TO DISPATCH - falling back to measure-only.',
        )
        appendBrief(
          `- ${nowIso} DENSITY GATE: armed (ANGEL_CONDUCTOR=1) but refusing to dispatch ` +
            `${proposal ? `[${proposal.rung}] ` : ''}- ${gateWhy}. Demoted to measure-only for this ` +
            `tick; run \`node scripts/runtime/cut-corpus.mjs\` to see the substrate, or set ` +
            `${gateTier?.knob ?? 'ANGEL_CUT_MIN_CORPUS'} to move the floor.`,
        )
      }
      if (dryRun) {
        console.log(`  mode: ${mode}${gated ? ` -> ${runMode} (density gate)` : ''}`)
        if (mode === 'armed' && proposal) {
          console.log(
            `  gate: [${proposal.rung}] governed by the ${gateTier?.kind} tier - ${gateWhy}`,
          )
        }
      }

      // ── the mission gate (scripts/runtime/mission.mjs) ──
      // An armed Conductor that drives a persisted mission consumes the mission's
      // round budget: only an ACTIVE mission inside its budget may dispatch, and
      // each real dispatch credits one round. Blocked/paused/complete missions
      // and spent budgets demote the tick to measure-only, exactly like the
      // density gate — the objective bounds the meta-loop. No mission = legacy
      // behavior unchanged.
      let mission = null
      if (mode === 'armed') {
        mission = loadActiveMission()
        if (mission !== null && !missionAllowsDispatch(mission)) {
          runMode = 'measure'
          const why = missionGateReason(mission)
          console.log(`conductor-tick: MISSION GATE - ${why}`)
          console.log(
            'conductor-tick: armed, but REFUSING TO DISPATCH - falling back to measure-only (mission).',
          )
          appendBrief(
            `- ${nowIso} MISSION GATE: armed (ANGEL_CONDUCTOR=1) but refusing to dispatch — ${why}. ` +
              `Demoted to measure-only for this tick.`,
          )
        }
      }
      if (dryRun && mode === 'armed') {
        console.log(
          `  mission: ${mission === null ? 'none (legacy behavior)' : missionSummaryForTick(mission)}`,
        )
      }

      // ── the measure-only outcome  ──
      // Everything above ran for real: the lock, the idle/window/budget gates, the
      // verdict fold, the cut fold, the approval measurement, the agenda mining and
      // the EIG ranking. The only thing measure mode withholds is the ACT — no rung
      // is spawned, nothing is merged, no .angel.auto.env overlay is written, no
      // worktree is touched, and no lastDispatchedAt is stamped (we did not
      // dispatch, so the cooldown must not pretend we did). The heartbeat action is
      // 'measure' — never 'skip', never 'ran' — so a night of this is unambiguous in
      // the log, and the briefing names the proposal we declined and why.
      const finishMeasure = ({ reason, proposal: chosen = null, burn = false }) => {
        if (burn) {
          budget = recordRun(budget)
          fs.writeFileSync(budgetPath, JSON.stringify(budget, null, 2))
        }
        persistGraph(graph)
        appendBrief(`- ${nowIso} measure: ${reason}`)
        writeHeartbeat(
          makeHeartbeat({
            now: nowIso,
            action: 'measure',
            reason,
            budget,
            experiment: chosen
              ? {
                  agenda: chosen.hypothesisId,
                  rung: chosen.rung,
                  goal: chosen.goal,
                  dispatched: false,
                }
              : null,
          }),
        )
        console.log(`conductor-tick: measure - ${reason} - budget ${budget.runs}/${budget.cap}`)
        return 0
      }

      if (!proposal) {
        const foldedNote = folded.folded.length ? `; folded ${folded.folded.length} verdict(s)` : ''
        const reason =
          (exclude.size ? 'no agenda outside cooldown' : 'no open agenda item') + foldedNote
        if (runMode === 'measure' && !dryRun) return finishMeasure({ reason })
        if (!dryRun) persistGraph(graph)
        return skip(reason, budget)
      }

      const plan = dispatchPlan(proposal, { root: ROOT, env: process.env })

      if (runMode === 'measure') {
        const would =
          plan.kind === 'spawn'
            ? `would have run \`${[plan.cmd, ...plan.args].join(' ')}\``
            : plan.kind === 'briefing'
              ? 'would have appended a briefing note only'
              : `would not have dispatched anyway (${plan.reason})`
        const why = gated
          ? `DENSITY GATE — ${gateWhy}`
          : 'measure-only mode (ANGEL_CONDUCTOR=measure)'
        if (dryRun) {
          console.log(
            `  top proposal: [${proposal.rung}] ${proposal.goal} (${proposal.hypothesisId})`,
          )
          console.log(`  DECLINED — ${why}`)
          console.log(`  ${would}`)
          return 0
        }
        return finishMeasure({
          reason: `declined to dispatch [${proposal.rung}] ${proposal.hypothesisId} — ${why}; ${would}`,
          proposal,
          burn: true,
        })
      }

      if (plan.kind === 'skip') {
        if (!dryRun) persistGraph(graph)
        return skip(plan.reason, budget)
      }

      // Only the armed path ever shells out to the angel binary (the code rung), so
      // only the armed path pays for the staleness guard — and only if some tier is
      // open at all, i.e. if there is any rung this tick could actually dispatch.
      if (!dryRun && !noBuild && plan.kind === 'spawn' && proposal.rung === 'code') {
        let binMtime = null
        try {
          binMtime = fs.statSync(binPath).mtimeMs
        } catch {
          /* missing */
        }
        let lastCommitMs = NaN
        try {
          lastCommitMs =
            Number(
              execFileSync('git', ['-C', ROOT, 'log', '-1', '--format=%ct'], {
                encoding: 'utf8',
              }).trim(),
            ) * 1000
        } catch {
          /* not a git repo */
        }
        if (binaryStale(binMtime, lastCommitMs)) {
          console.log('conductor-tick: release binary is stale - rebuilding...')
          const build = boundedSpawnSync(
            'cargo',
            ['build', '--locked', '--release', '--no-default-features'],
            {
              cwd: join(ROOT, 'cockpit'),
              stdio: 'inherit',
              env: sourceBoundEnv(ROOT, spawnSync, 'conductor-tick'),
            },
          )
          if (build.status !== 0) throw new Error('cargo build --release failed; aborting tick')
        }
      }

      let ok = true
      let detail = ''
      if (dryRun) {
        console.log(`  dispatch: [${proposal.rung}] ${proposal.goal}`)
        console.log(`  plan: ${JSON.stringify(plan)}`)
        return 0
      }

      budget = recordRun(budget)
      fs.writeFileSync(budgetPath, JSON.stringify(budget, null, 2))
      // One real dispatch = one mission continuation round (an errored child
      // still consumed the round — work was attempted).
      if (mission !== null) {
        const next = tickMission(mission, {})
        await persistMission(next, process.env.ANGEL_MISSION_DIR || workerPaths().missions)
        appendBrief(
          `- ${nowIso} mission [${next.id}] round ${next.roundsStarted}/${next.maxRounds}`,
        )
      }
      graph.updateNode(proposal.hypothesisId, { lastDispatchedAt: nowIso })
      if (plan.kind === 'briefing') {
        fs.appendFileSync(briefingPath, plan.text)
        detail = `briefing note appended for ${proposal.hypothesisId}`
      } else if (plan.kind === 'spawn') {
        // Every rung's child (reflex / habitsmith / dossier / code run) writes the
        // graph itself. Hand off our mutations, then reload theirs, so persistGraph
        // below settles the merged state instead of clobbering the child's work.
        handOffGraph()
        // dispatchPlan stays a pure 'node' route (it is a plan, not a process); the
        // spawn resolves it to the interpreter running us, because cron's PATH does
        // not contain nvm's node. See the cut-tick spawn above.
        const cmd = plan.cmd === 'node' ? process.execPath : plan.cmd
        const child = boundedSpawnSync(
          cmd,
          [...plan.args, '--graph', graphPath, '--ledger', ledgerPath, '--force'],
          {
            cwd: ROOT,
            stdio: 'inherit',
            timeout: 3_660_000,
            env: {
              ...process.env,
              ANGEL_CAUSAL_GRAPH: graphPath,
              ANGEL_EXPERIENCE_LOG: ledgerPath,
              ANGEL_CONDUCTOR_DIR: stateDir,
              ANGEL_BENCHMARK_REPORTS_DIR: reportsDir,
              ANGEL_BIN: binPath,
            },
          },
        )
        ok = child.status === 0
        graph = loadGraph()
        detail = ok
          ? `${proposal.rung} child completed`
          : `${proposal.rung} child exited ${child.status ?? 'unknown'}`
      }

      persistGraph(graph)
      appendBrief(`- ${nowIso} dispatch [${proposal.rung}] ${proposal.hypothesisId}: ${detail}`)
      writeHeartbeat(
        makeHeartbeat({
          now: nowIso,
          action: ok ? 'ran' : 'error',
          reason: detail,
          budget,
          experiment: {
            agenda: proposal.hypothesisId,
            rung: proposal.rung,
            goal: proposal.goal,
          },
        }),
      )
      console.log(
        `conductor-tick: ${ok ? 'done' : 'error'} - ${detail} - budget ${budget.runs}/${budget.cap}`,
      )
      return ok ? 0 : 1
    } catch (err) {
      if (!dryRun) {
        writeHeartbeat(
          makeHeartbeat({
            now: nowIso,
            action: 'error',
            reason: err?.message || String(err),
            budget,
          }),
        )
        appendBrief(`- ${nowIso} error: ${err?.message || String(err)}`)
      }
      console.error(`conductor-tick: error - ${err?.message || err}`)
      return 1
    }
  }
  if (dryRun) return run()
  const held = await holdProcessLease(run, { leaseDir, compatibilityPath: lockPath, lease })
  return held.acquired ? held.value : skip(held.reason, budget, { publish: false })
}

const isCli =
  process.argv[1] && (await import('node:url')).fileURLToPath(import.meta.url) === process.argv[1]
if (isCli) {
  runGraphCli(cli, process.argv.slice(2)).then((code) => process.exit(code ?? 0))
}
