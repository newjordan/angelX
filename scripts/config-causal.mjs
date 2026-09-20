import { runGraphCli } from './worker-lock.mjs'
import { workerPaths } from './worker-paths.mjs'
// config-causal — point Active Causal Discovery at angel0's OWN CONFIGURATION.
//
// The causal engine (causal-loop.mjs) ranks open hypotheses by expected
// information gain and proposes the single most informative one to test next.
// self-causal.mjs already proved the engine is domain-portable by feeding it the
// agent's repair telemetry. This module feeds it a THIRD kind of hypothesis: a
// configuration intervention — "pinning knob X to value V moves metric M" — mined
// from the experience ledger (experience.rs, M1). Structurally identical to a
// research or self-model hypothesis, so it slots into the same belief net with no
// change to the scorer.
//
// Two evidence channels, same graph (docs/WORKERS.md):
//   • Interventional — a Control Bench A/B (M3) concludes a hypothesis via a
//     signed edge. High confidence. Not this module's job.
//   • Observational — ledger mining (here): which pinned configs correlate with
//     fewer failovers / spin-stops / tokens. Low confidence (≤0.4): it MINTS and
//     PRIORITIZES hypotheses; it never concludes one. So this module attaches
//     observed evidence to nodes and re-weights the ranking — it adds NO signed
//     SUPPORTS/CONTRADICTS edges (only an experiment does).
//
// Three hard properties, by construction (mirroring self-causal):
//   • Pure. Takes a live graph + ledger rows, returns {graph, added} or a
//     proposal. No file I/O, no network, no Math.random — the CLI/caller persists.
//   • Propose-only. It mints hypotheses and ranks them; enacting one is a
//     separate gated step (the Control Bench runner in M3, the reconfigurator in
//     M4). Reflex never edits .angel.env or secrets.
//   • Zero engine changes. scoreHypotheses / nextExperiment are reused verbatim;
//     the per-experiment `cost` hook (dormant until now) is finally driven.

import { NODE_TYPE } from '../lib/research/CausalGraph.js'
import { scoreHypotheses } from './causal-loop.mjs'

export const CONFIG_PROJECT = Object.freeze({ id: 'reflex', label: 'Reflex config' })

// Metric vocabulary. Control Bench metrics (accuracy_pct, mean_latency_s,
// error_rate) plus ledger-native ones the experience ledger can observe directly.
// `direction` is which way is BETTER — descriptive (the engine ignores it, the
// prompt and the observational gradient use it).
export const METRICS = Object.freeze({
  accuracy_pct: { name: 'accuracy_pct', direction: 'higher' },
  mean_latency_s: { name: 'mean_latency_s', direction: 'lower' },
  error_rate: { name: 'error_rate', direction: 'lower' },
  failover_rate: { name: 'failover_rate', direction: 'lower' },
  spin_stop_rate: { name: 'spin_stop_rate', direction: 'lower' },
  deferred_stop_rate: { name: 'deferred_stop_rate', direction: 'lower' },
  tokens_per_turn: { name: 'tokens_per_turn', direction: 'lower' },
})

// Which ledger-native metrics this module can compute from a ledger row, and how.
// Each maps a metric name to a per-row (numerator, denominator-always-1) sample:
// rate metrics contribute 0/1, mean metrics contribute the raw value. Keyed so
// ledgerMetrics and the tests share one source of truth.
const LEDGER_METRIC_FNS = Object.freeze({
  failover_rate: (o) => (Array.isArray(o.failovers) && o.failovers.length > 0 ? 1 : 0),
  spin_stop_rate: (o) => (o.stop === 'spin' ? 1 : 0),
  deferred_stop_rate: (o) => (o.stop === 'deferred_stop' ? 1 : 0),
  error_rate: (o) => (o.stop === 'error_stop' ? 1 : 0),
  mean_latency_s: (o) => (Number.isFinite(o.latency_ms) ? o.latency_ms / 1000 : null),
  tokens_per_turn: (o) => turnTokens(o),
})

// Registered SOTA link aliases usable as a role seat (club/bag.rs). The seat
// hypotheses A/B these against the live baseline.
export const SEAT_CANDIDATES = Object.freeze([
  'codex-run',
  'longcat',
  'deepseek',
  'glm',
  'gpt-5.2',
  'cerebras',
])

// The curated ~12 high-leverage knobs (NOT the full 242-var inventory). Each
// declares the metrics it plausibly moves; seeding is the (value × metric) cross
// product, so metric lists are kept to 1–2. `cost` is the estimated Control Bench
// run cost in minutes for one A/B on this knob — it drives the EIG cost term so
// expensive experiments (ALWAYS amplifies every turn) get deprioritized.
export const KNOB_CATALOG = Object.freeze([
  // Routing seats (Tier A — reflex may auto-write these).
  {
    name: 'ANGEL_SOTA_MOA_AGG_CLUB',
    knob: 'agg',
    kind: 'seat',
    values: SEAT_CANDIDATES,
    metrics: [METRICS.failover_rate, METRICS.accuracy_pct],
    cost: 6,
  },
  {
    name: 'ANGEL_SOTA_MOA_PROPOSE_CLUB',
    knob: 'propose',
    kind: 'seat',
    values: SEAT_CANDIDATES,
    metrics: [METRICS.failover_rate, METRICS.accuracy_pct],
    cost: 6,
  },
  {
    name: 'ANGEL_SOTA_MOA_JUDGE_CLUB',
    knob: 'judge',
    kind: 'seat',
    values: SEAT_CANDIDATES,
    metrics: [METRICS.failover_rate, METRICS.accuracy_pct],
    cost: 6,
  },
  {
    name: 'ANGEL_SOTA_MOA_VERIFY_CLUB',
    knob: 'verify',
    kind: 'seat',
    values: SEAT_CANDIDATES,
    metrics: [METRICS.failover_rate, METRICS.accuracy_pct, METRICS.error_rate],
    cost: 6,
  },
  // Behavior toggles / counts.
  {
    name: 'ANGEL_SOTA_MOA_ALWAYS',
    knob: 'always',
    kind: 'toggle',
    values: ['0', '1'],
    metrics: [METRICS.accuracy_pct, METRICS.mean_latency_s],
    cost: 12,
  },
  {
    name: 'ANGEL_SOTA_MOA_JUDGE',
    knob: 'judge_on',
    kind: 'toggle',
    values: ['0', '1'],
    metrics: [METRICS.accuracy_pct],
    cost: 6,
  },
  {
    name: 'ANGEL_SOTA_MOA_VERIFY',
    knob: 'verify_rounds',
    kind: 'numeric',
    values: ['0', '1', '2'],
    metrics: [METRICS.accuracy_pct, METRICS.tokens_per_turn],
    cost: 8,
  },
  {
    name: 'ANGEL_MOA_JUDGE_FANOUT',
    knob: 'judge_fanout',
    kind: 'numeric',
    values: ['6', '12', '18'],
    metrics: [METRICS.accuracy_pct, METRICS.tokens_per_turn],
    cost: 6,
  },
  {
    name: 'ANGEL_PXPIPE',
    knob: 'pxpipe',
    kind: 'toggle',
    values: ['0', '1'],
    metrics: [METRICS.error_rate],
    cost: 4,
  },
  {
    name: 'ANGEL_SOTA_CAVEMAN',
    knob: 'caveman',
    kind: 'toggle',
    values: ['0', '1'],
    metrics: [METRICS.accuracy_pct],
    cost: 6,
  },
  // Harness guardrails (Tier B — propose-only).
  {
    name: 'ANGEL_SPIN_LIMIT',
    knob: 'spin_limit',
    kind: 'numeric',
    values: ['4', '8', '12'],
    metrics: [METRICS.spin_stop_rate, METRICS.mean_latency_s],
    cost: 4,
  },
  {
    name: 'ANGEL_MAX_HOPS',
    knob: 'max_hops',
    kind: 'numeric',
    values: ['32', '64', '0'],
    metrics: [METRICS.deferred_stop_rate, METRICS.accuracy_pct],
    cost: 4,
  },
])

const slug = (s) =>
  String(s ?? '')
    .toLowerCase()
    .replace(/[^0-9a-z]+/g, '-')
    .replace(/^-+|-+$/g, '')
    .slice(0, 48) || 'x'

const isConfig = (node) => node?.projectId === CONFIG_PROJECT.id

/**
 * Deterministic node id for a (knob, value, metric) hypothesis. `knob` and
 * `metric` are clean controlled-vocab identifiers used verbatim; only the value
 * (which can carry dots, e.g. `gpt-5.2`) is slugged.
 */
export function configHypothesisId(knob, value, metric) {
  return `reflex_${knob}_${slug(value)}_${metric}`
}

// ─── seed ──────────────────────────────────────────────────────────────────

/**
 * Mint the config-experiment hypotheses for the knob catalog into the graph.
 *
 * One HYPOTHESIS node per (knob, candidate value, plausible metric): "does
 * pinning <name>=<value> move <metric> vs the live baseline?" Idempotent on the
 * deterministic id — re-seeding refreshes `lastSeenAt` rather than duplicating,
 * so this can run every tick. Status starts (and stays) 'testing' until an
 * interventional verdict concludes it — an untested config is maximally
 * uncertain, exactly what the EIG scorer surfaces.
 *
 * @param {CausalGraph} graph
 * @param {{now?:string, catalog?:object[]}} [opts]
 * @returns {{graph:CausalGraph, added:{nodes:number, updated:number}, hypotheses:string[]}}
 */
export function seedConfigHypotheses(graph, opts = {}) {
  const now = opts.now || new Date().toISOString()
  const catalog = opts.catalog || KNOB_CATALOG
  let nodes = 0
  let updated = 0
  const hypotheses = []

  for (const knob of catalog) {
    for (const value of knob.values) {
      for (const metric of knob.metrics) {
        const id = configHypothesisId(knob.knob, value, metric.name)
        hypotheses.push(id)
        if (graph.hasNode(id)) {
          graph.updateNode(id, { lastSeenAt: now })
          updated++
          continue
        }
        const dir = metric.direction === 'higher' ? 'raise' : 'lower'
        graph.addNode({
          id,
          type: NODE_TYPE.HYPOTHESIS,
          label: `${knob.name}=${value} → ${dir} ${metric.name}`,
          hypothesisId: id.replace(/^reflex_/, ''),
          projectId: CONFIG_PROJECT.id,
          projectLabel: CONFIG_PROJECT.label,
          metric: { name: metric.name, direction: metric.direction },
          status: 'testing',
          proposedAt: now,
          lastSeenAt: now,
          concludedAt: null,
          question: `Does pinning ${knob.name}=${value} ${dir} ${metric.name} vs the live baseline seat?`,
          prediction: `Setting ${knob.name}=${value} moves ${metric.name} in the ${metric.direction}-is-better direction.`,
          outcome: 'neutral',
          // config specifics (extra fields ride along untouched by the scorer)
          knobName: knob.name,
          knob: knob.knob,
          knobKind: knob.kind,
          knobValue: value,
          benchCost: knob.cost,
          patchTargets: ['.angel.auto.env'],
          observed: null,
        })
        nodes++
      }
    }
  }
  return { graph, added: { nodes, updated }, hypotheses }
}

// ─── ledger mining (observational channel) ───────────────────────────────────

// Total tokens across every club in one outcome's token map.
function turnTokens(outcome) {
  const t = outcome?.tokens
  if (!t || typeof t !== 'object') return null
  let sum = 0
  let any = false
  for (const v of Object.values(t)) {
    const i = Number(v?.in) || 0
    const o = Number(v?.out) || 0
    if (i || o) any = true
    sum += i + o
  }
  return any ? sum : null
}

/**
 * Mine experience-ledger rows into per-(knob, value) metric aggregates plus a
 * seat-level failover breakdown. Pure — takes already-parsed rows.
 *
 * For every catalog knob, rows are grouped by the value that knob had in the
 * row's config snapshot (`cfg.knobs[name]`, or '(default)' when the knob wasn't
 * pinned). Within a group each ledger-observable metric is averaged. The
 * failover breakdown groups every recorded failover by its `from` seat + reason
 * — a direct, config-independent read on which seat fumbles (the LongCat
 * raw-markup story), since an unpinned seat leaves no knob value to group on.
 *
 * @param {object[]} rows  parsed ledger records (turn / moa_turn kinds)
 * @returns {{byKnob:object, failoversByClub:object, total:number}}
 */
export function ledgerMetrics(rows) {
  const byKnob = {}
  const failoversByClub = {}
  let total = 0

  const acc = () => ({ n: 0, sums: {}, counts: {} })
  const add = (bucket, metric, sample) => {
    if (sample === null || sample === undefined) return
    bucket.sums[metric] = (bucket.sums[metric] || 0) + sample
    bucket.counts[metric] = (bucket.counts[metric] || 0) + 1
  }

  for (const row of rows || []) {
    const o = row?.outcome
    if (!o) continue
    total++
    const knobs = (row.cfg && row.cfg.knobs) || {}

    for (const knob of KNOB_CATALOG) {
      const value = knobs[knob.name] ?? '(default)'
      const groups = (byKnob[knob.name] ||= {})
      const bucket = (groups[value] ||= acc())
      bucket.n++
      for (const metric of knob.metrics) {
        const fn = LEDGER_METRIC_FNS[metric.name]
        if (fn) add(bucket, metric.name, fn(o))
      }
    }

    for (const f of Array.isArray(o.failovers) ? o.failovers : []) {
      const key = f.from || '?'
      const rec = (failoversByClub[key] ||= { count: 0, reasons: {}, to: {} })
      rec.count++
      if (f.reason) rec.reasons[f.reason] = (rec.reasons[f.reason] || 0) + 1
      if (f.to) rec.to[f.to] = (rec.to[f.to] || 0) + 1
    }
  }

  // Collapse sums/counts → means for a compact, testable summary.
  const finalize = (bucket) => {
    const out = { n: bucket.n, metrics: {} }
    for (const m of Object.keys(bucket.sums)) {
      out.metrics[m] = { mean: bucket.sums[m] / bucket.counts[m], samples: bucket.counts[m] }
    }
    return out
  }
  const byKnobOut = {}
  for (const [name, groups] of Object.entries(byKnob)) {
    byKnobOut[name] = {}
    for (const [value, bucket] of Object.entries(groups)) byKnobOut[name][value] = finalize(bucket)
  }
  return { byKnob: byKnobOut, failoversByClub, total }
}

/**
 * Fold ledger observations onto the config hypotheses as low-confidence evidence.
 *
 * Observational only: attaches an `observed` summary to each matching hypothesis
 * (sample count, the value's mean metric, the baseline mean, and a signed
 * `gradient` = does this value look BETTER than baseline on this metric). It adds
 * NO signed edges — the engine's belief for a config hypothesis moves only on an
 * interventional Control Bench verdict (M3). The attached evidence re-weights the
 * proposal ranking (see proposeConfigExperiment).
 *
 * @param {CausalGraph} graph
 * @param {object[]} rows  parsed ledger records
 * @param {{now?:string, metrics?:object, minSamples?:number}} [opts]
 * @returns {{graph:CausalGraph, updated:number, total:number}}
 */
export function ingestLedgerObservations(graph, rows, opts = {}) {
  const now = opts.now || new Date().toISOString()
  const minSamples = opts.minSamples ?? 3
  const summary = opts.metrics || ledgerMetrics(rows)
  let updated = 0

  for (const node of graph.nodesOfType(NODE_TYPE.HYPOTHESIS)) {
    if (!isConfig(node) || !node.knobName) continue
    const groups = summary.byKnob[node.knobName]
    if (!groups) continue
    const mine = groups[node.knobValue]
    const metricName = node.metric?.name
    const obs = mine?.metrics?.[metricName]
    if (!obs || obs.samples < minSamples) continue

    // Baseline = the best OTHER observed value for this knob+metric (the bar this
    // value must beat). Direction-aware: lower-is-better picks the min, etc.
    const lowerBetter = node.metric?.direction !== 'higher'
    let baseline = null
    for (const [value, agg] of Object.entries(groups)) {
      if (value === node.knobValue) continue
      const other = agg.metrics?.[metricName]
      if (!other || other.samples < minSamples) continue
      if (baseline === null) baseline = other.mean
      else baseline = lowerBetter ? Math.min(baseline, other.mean) : Math.max(baseline, other.mean)
    }
    // gradient > 0 ⇒ this value looks better than the baseline on this metric.
    let gradient = 0
    if (baseline !== null) gradient = lowerBetter ? baseline - obs.mean : obs.mean - baseline

    graph.updateNode(node.id, {
      observed: {
        samples: obs.samples,
        mean: obs.mean,
        baseline,
        gradient,
        observedAt: now,
      },
    })
    updated++
  }
  return { graph, updated, total: summary.total }
}

// ─── propose ─────────────────────────────────────────────────────────────────

/**
 * Rank the open config experiments and propose the single most worthwhile one.
 *
 * Reuses scoreHypotheses (entropy × impact ÷ cost) verbatim, then re-weights the
 * reflex subset by:
 *   • evidence — 1 + log2(1 + observed.samples): a knob/value we already have
 *     live data on is cheaper to reason about and breaks entropy ties between
 *     never-observed configs.
 *   • gradient — a value the ledger says looks better than baseline gets a mild
 *     boost (worth confirming); one that looks worse is mildly demoted. Gentle,
 *     because observational evidence is low-confidence by policy.
 *   • cost — divide by the knob's estimated Control Bench minutes so an expensive
 *     experiment (ALWAYS amplifies every turn) is deprioritized vs a cheap seat
 *     swap of equal information. This is the EIG cost term, finally driven.
 *
 * Always dryRun. Enacting a proposal is the Control Bench runner (M3) behind the
 * budget/idle gate; the reconfigurator (M4) writes only Tier-A routing knobs.
 *
 * @param {CausalGraph} graph
 * @param {{limit?:number, cost?:number}} [opts]
 * @returns {{proposal:object|null, ranking:object[], dryRun:true}}
 */
export function proposeConfigExperiment(graph, opts = {}) {
  const scored = scoreHypotheses(graph, opts)
  const ranked = scored
    .map((row) => {
      const node = graph.getNode(row.id)
      if (!isConfig(node)) return null
      const obs = node.observed || null
      const samples = obs && Number.isFinite(obs.samples) ? obs.samples : 0
      const evidence = 1 + Math.log2(1 + samples)
      // Gentle gradient nudge in [0.7, 1.5], neutral (1.0) without evidence.
      let gradientFactor = 1
      if (obs && Number.isFinite(obs.gradient) && obs.baseline !== null) {
        gradientFactor = obs.gradient > 0 ? 1.5 : obs.gradient < 0 ? 0.7 : 1
      }
      const costFactor = Number.isFinite(node.benchCost) ? Math.max(1, node.benchCost) : 1
      const priority = (row.score * evidence * gradientFactor) / costFactor
      const dir = node.metric?.direction === 'higher' ? 'raise' : 'lower'
      const prompt =
        `Config experiment: pin ${node.knobName}=${node.knobValue} and measure ${node.metric?.name} ` +
        `(want to ${dir} it) against the live baseline on a frozen Control Bench suite. ` +
        `Run it behind the reflex idle/budget gate; do NOT touch .angel.env or any secret. ` +
        `Report SUPPORTS if ${node.metric?.name} improved by ≥ the variance guard, CONTRADICTS if not, with a confidence.`
      return {
        hypothesisId: row.id,
        label: node.label,
        knob: node.knobName,
        value: node.knobValue,
        metric: node.metric?.name,
        kind: node.knobKind,
        benchCost: node.benchCost,
        observed: obs,
        score: row.score,
        priority,
        rationale:
          `${row.rationale} · ${samples ? `${samples} obs` : 'no obs'}` +
          `${obs && obs.baseline !== null ? ` · gradient ${obs.gradient >= 0 ? '+' : ''}${obs.gradient.toFixed(3)}` : ''}` +
          ` · ~${costFactor}min`,
        prompt,
      }
    })
    .filter(Boolean)
    .sort((a, b) => b.priority - a.priority || (a.hypothesisId < b.hypothesisId ? -1 : 1))

  return { proposal: ranked[0] ?? null, ranking: ranked.slice(0, opts.limit ?? 8), dryRun: true }
}

/** All config (reflex) hypotheses currently in the graph. */
export function configHypotheses(graph) {
  return graph.nodesOfType(NODE_TYPE.HYPOTHESIS).filter(isConfig)
}

// ─── CLI ─────────────────────────────────────────────────────────────────────
// I/O lives here only; everything above is pure. `--seed` folds the catalog +
// ledger observations into the shared causal graph; `--rank` prints the EIG
// ranking. The graph file has no locking (the reflex tick, M5, takes a lockfile);
// this is a manual one-shot.

async function cli(argv) {
  const { readFileSync, writeFileSync, existsSync } = await import('./private-store-fs.mjs')
  const { fileURLToPath } = await import('node:url')
  const { dirname, join } = await import('node:path')
  const os = await import('node:os')

  const ROOT = join(dirname(fileURLToPath(import.meta.url)), '..')
  const flag = (name) => {
    const i = argv.indexOf(name)
    return i >= 0 ? argv[i + 1] : undefined
  }
  const graphPath = flag('--graph') || workerPaths().graph
  const ledgerPath = flag('--ledger') || process.env.ANGEL_EXPERIENCE_LOG || workerPaths().ledger

  const CausalGraph = (await import('../lib/research/CausalGraph.js')).default
  const loadGraph = () =>
    existsSync(graphPath)
      ? CausalGraph.deserialize(JSON.parse(readFileSync(graphPath, 'utf8')))
      : new CausalGraph()
  const loadLedger = () => {
    if (!existsSync(ledgerPath)) return []
    return readFileSync(ledgerPath, 'utf8')
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
  }

  const wantSeed = argv.includes('--seed') || argv[0] === 'seed'
  const wantRank = argv.includes('--rank') || argv[0] === 'rank'

  if (wantSeed) {
    const graph = loadGraph()
    const rows = loadLedger()
    const { added, hypotheses } = seedConfigHypotheses(graph)
    const { updated, total } = ingestLedgerObservations(graph, rows)
    writeFileSync(graphPath, JSON.stringify(graph.serialize(), null, 2))
    console.log(
      `reflex seed → ${graphPath}\n` +
        `  ${added.nodes} new + ${added.updated} refreshed config hypotheses (${hypotheses.length} total)\n` +
        `  ledger: ${total} row(s) from ${ledgerPath} · ${updated} hypothesis(es) got observational evidence`,
    )
    return 0
  }

  if (wantRank) {
    const graph = loadGraph()
    if (configHypotheses(graph).length === 0) {
      // Rank a freshly-seeded in-memory graph so --rank works before --seed.
      seedConfigHypotheses(graph)
      ingestLedgerObservations(graph, loadLedger())
    }
    const { ranking } = proposeConfigExperiment(graph, { limit: 12 })
    console.log(`reflex config experiments, EIG-ranked (top ${ranking.length}):\n`)
    ranking.forEach((r, i) => {
      console.log(
        `${String(i + 1).padStart(2)}. ${r.label}\n` +
          `    priority ${r.priority.toFixed(3)} · ${r.rationale}`,
      )
    })
    if (ranking.length === 0) console.log('  (no open config hypotheses)')
    return 0
  }

  console.log(
    'usage: node scripts/config-causal.mjs <--seed|--rank> [--graph FILE] [--ledger FILE]\n' +
      '  --seed  mint the knob-catalog hypotheses + fold in ledger observations, write the graph\n' +
      '  --rank  print the EIG-ranked config-experiment queue',
  )
  return 0
}

const isCli =
  process.argv[1] && (await import('node:url')).fileURLToPath(import.meta.url) === process.argv[1]
if (isCli) {
  runGraphCli(cli, process.argv.slice(2)).then((code) => process.exit(code ?? 0))
}
