import { runGraphCli } from './worker-lock.mjs'
import { workerPaths } from './worker-paths.mjs'
// conductor — the agenda miner + scorer (C1 of docs/WORKERS.md).
//
// The Conductor is the meta-loop that decides WHAT angel should improve next.
// Every self-improvement rung already writes telemetry — the experience ledger
// (turn stops, failovers), Control Bench reports (accuracy deltas), habitsmith
// status (drift flags), reflex Tier-B proposals, and dossier trap facts. This
// module mines those five substrates into IMPROVEMENT-AGENDA candidates and
// folds them into the shared causal graph as HYPOTHESIS nodes — the fifth
// instance of the ingest→score→propose pattern (self-causal → config-causal →
// repo-dossier → habitsmith → here):
//   • mining attaches OBSERVATIONAL stats only (they ride on the node, no
//     signed edges) — observation prioritizes, verification concludes;
//   • dispatch outcomes (C3/C5) conclude via updateBeliefs, not this module;
//   • proposeAgenda reuses scoreHypotheses VERBATIM and re-weights the
//     conductor subset post hoc (the proposeConfigExperiment shape):
//     priority = score × (1 + log2(1 + samples)) × severity / max(1, cost/10).
//
// Pure core (no file I/O, no network, no Math.random) + a CLI shell at the
// bottom, exactly the repo-dossier.mjs layout. Secrets discipline: the only
// thing ever read is telemetry; .angel.env is never touched, and secret-named
// knobs are refused at the reflex adapter (belt-and-suspenders).

import { NODE_TYPE } from '../lib/research/CausalGraph.js'
import { scoreHypotheses } from './causal-loop.mjs'
import { factBelief } from './repo-dossier.mjs'

export const CONDUCTOR_PROJECT = Object.freeze({ id: 'conductor', label: 'Conductor' })

// ─── mining thresholds ───────────────────────────────────────────────────────
// A turn-failure cluster needs recurrence before it's an agenda item — one-off
// stops never become work.
export const TURN_CLUSTER_MIN = 3
// Only recent history clusters; ancient stops describe a binary long replaced.
export const DEFAULT_LOOKBACK_DAYS = 14
// A bench drop must clear the reflex variance guard: max(2×accuracy_std, 1.0).
// The 1.0-pt floor is load-bearing — with --seeds 1 there is no std estimate.
export const BENCH_DROP_FLOOR = 1.0
// A dossier trap must be believed (factBelief ≥ this) before it's worth a
// night run — a decayed or contested trap becomes a dossier probe instead.
export const TRAP_BELIEF_MIN = 0.7
// Estimated dispatch cost per rung, in minutes (drives the EIG cost divisor):
// config = one reflex bench A/B; code = a gated headless run (C4); skills /
// knowledge = one habitsmith / dossier tick.
export const RUNG_COST_MIN = Object.freeze({ config: 8, code: 45, skills: 5, knowledge: 5 })

const slug = (s) =>
  String(s ?? '')
    .toLowerCase()
    .replace(/[^0-9a-z]+/g, '-')
    .replace(/^-+|-+$/g, '')
    .slice(0, 48) || 'x'

const isConductor = (node) => node?.projectId === CONDUCTOR_PROJECT.id

// Severity 1–3 from an occurrence count: 3× the cluster floor is severe.
const sevFromCount = (n) => (n >= TURN_CLUSTER_MIN * 3 ? 3 : n >= TURN_CLUSTER_MIN * 2 ? 2 : 1)

// Cap evidence lists so a hot cluster can't bloat the graph file.
const EVIDENCE_CAP = 12
const capRefs = (refs) => refs.slice(0, EVIDENCE_CAP)

// ─── signal adapters ─────────────────────────────────────────────────────────
// Each is pure and returns candidate structs:
//   {rung, slug, goal, evidenceRefs, estCostMin, severity, observed, label}
// `observed.samples` is the evidence-volume term proposeAgenda's re-weighting
// reads; the rest of `observed` is adapter-specific observational stats.

const isTurnRow = (r) => (r?.kind === 'turn' || r?.kind === 'moa_turn') && r?.outcome
const ledgerRef = (r) => `ledger#${r.seq ?? `ts${r.ts ?? '?'}`}`

/**
 * Cluster recent `turn`/`moa_turn` records by non-`answer` stop reason and by
 * failover `{from,to,reason}` triple. A cluster with ≥ TURN_CLUSTER_MIN
 * occurrences becomes a candidate: rung 'config' when the cluster concentrates
 * in one cfg.hash that is NOT simply the dominant config overall (the failure
 * correlates with a knob setting), rung 'code' otherwise. Pure.
 *
 * @param {object[]} rows  parsed ledger records (any kinds; only turns read)
 * @param {{now?:string, lookbackDays?:number}} [opts]
 * @returns {object[]} candidate structs
 */
export function turnFailureSignals(rows, opts = {}) {
  const lookbackDays = opts.lookbackDays ?? DEFAULT_LOOKBACK_DAYS
  const nowSec = opts.now ? Math.floor(Date.parse(opts.now) / 1000) : Math.floor(Date.now() / 1000)
  const cutoff = nowSec - lookbackDays * 86_400

  // One in-window pass: hash base-rates + per-cluster n/byHash/refs/lastSeen.
  let turnCount = 0
  const hashTotals = new Map()
  const clusters = new Map()
  const add = (key, seed, h, ts, ref) => {
    const c = clusters.get(key) ?? { ...seed, n: 0, byHash: new Map(), refs: [], lastSeen: ts }
    c.n++
    c.byHash.set(h, (c.byHash.get(h) ?? 0) + 1)
    if (c.refs.length < EVIDENCE_CAP) c.refs.push(ref)
    if (ts > c.lastSeen) c.lastSeen = ts
    clusters.set(key, c)
  }
  for (const r of rows || []) {
    if (!(isTurnRow(r) && (Number(r.ts) || 0) >= cutoff)) continue
    turnCount++
    const h = r.cfg?.hash ?? '(none)'
    hashTotals.set(h, (hashTotals.get(h) ?? 0) + 1)
    const stop = r.outcome?.stop
    const failovers = Array.isArray(r.outcome?.failovers) ? r.outcome.failovers : []
    if (!(stop && stop !== 'answer') && failovers.length === 0) continue
    const ts = Number(r.ts) || 0
    const ref = ledgerRef(r)
    if (stop && stop !== 'answer') add(`stop:${stop}`, { kind: 'stop', stop }, h, ts, ref)
    for (const f of failovers) {
      const key = `failover:${f.from ?? '?'}→${f.to ?? '?'}:${f.reason ?? '?'}`
      add(key, { kind: 'failover', from: f.from, to: f.to, reason: f.reason }, h, ts, ref)
    }
  }
  if (turnCount === 0) return []

  const out = []
  for (const c of clusters.values()) {
    const n = c.n
    if (n < TURN_CLUSTER_MIN) continue

    // cfg correlation: densest hash, then lex-smaller (same as the old sort).
    const [topHash, topCount] = [...c.byHash.entries()].sort(
      (a, b) => b[1] - a[1] || (a[0] < b[0] ? -1 : 1),
    )[0]
    const topShare = topCount / n
    const overallShare = (hashTotals.get(topHash) ?? 0) / turnCount
    const configCorrelated = topShare >= 0.8 && overallShare <= 0.5
    const rung = configCorrelated ? 'config' : 'code'

    const what =
      c.kind === 'stop'
        ? `'${c.stop}' stop`
        : `${c.from ?? '?'}→${c.to ?? '?'} failover (${c.reason ?? '?'})`
    const goal = configCorrelated
      ? `Retune the config (hash ${topHash}) implicated in ${n} ${what}s.`
      : c.kind === 'stop'
        ? `Eliminate the '${c.stop}' stop cluster (${n} turns in ${lookbackDays}d).`
        : `Stop ${c.from ?? '?'} failing over to ${c.to ?? '?'} (${c.reason ?? '?'}) — ${n} occurrences in ${lookbackDays}d.`

    out.push({
      rung,
      slug:
        c.kind === 'stop' ? `stop-${slug(c.stop)}` : slug(`failover-${c.from}-${c.to}-${c.reason}`),
      goal,
      label: `Turn failures: ${what} ×${n}`,
      evidenceRefs: c.refs,
      estCostMin: RUNG_COST_MIN[rung],
      severity: sevFromCount(n),
      observed: {
        samples: n,
        windowDays: lookbackDays,
        lastSeen: c.lastSeen,
        cfgHash: configCorrelated ? topHash : null,
        cfgShare: topShare,
        cfgOverallShare: overallShare,
      },
    })
  }
  return out.sort((a, b) => (a.slug < b.slug ? -1 : 1))
}

/**
 * Regressions between consecutive Control Bench reports of the same suite:
 * for each subject present in both, an `accuracy_pct` drop strictly greater
 * than max(2×accuracy_std, BENCH_DROP_FLOOR) — the reflex variance guard, std
 * taken as the larger of the pair — is a rung:'code' regression candidate.
 * Reports may carry a `file` property (the CLI attaches the filename); pure.
 *
 * @param {object[]} reports  parsed control_<suite>_<ts14>.json docs
 * @returns {object[]} candidate structs
 */
export function benchDeltaSignals(reports) {
  const bySuite = new Map()
  for (const r of reports || []) {
    const suite = r?.control?.suite
    if (!suite || !Array.isArray(r?.metrics)) continue
    const list = bySuite.get(suite) ?? []
    list.push(r)
    bySuite.set(suite, list)
  }

  const found = new Map() // suite/subject → aggregate
  for (const [suite, list] of bySuite) {
    list.sort((a, b) => {
      const ta = a.control?.timestamp ?? ''
      const tb = b.control?.timestamp ?? ''
      return ta < tb ? -1 : ta > tb ? 1 : (a.file ?? '') < (b.file ?? '') ? -1 : 1
    })
    for (let i = 1; i < list.length; i++) {
      const prev = list[i - 1]
      const curr = list[i]
      const prevBySub = new Map(prev.metrics.map((m) => [m.subject, m]))
      for (const m of curr.metrics) {
        const p = prevBySub.get(m.subject)
        if (!p) continue
        const a = Number(p.accuracy_pct)
        const b = Number(m.accuracy_pct)
        if (!Number.isFinite(a) || !Number.isFinite(b)) continue
        if (p.accuracy_std == null || m.accuracy_std == null) continue
        const std = Math.max(Number(p.accuracy_std) || 0, Number(m.accuracy_std) || 0)
        const guard = Math.max(2 * std, BENCH_DROP_FLOOR)
        const drop = a - b
        if (drop <= guard) continue

        const key = `${suite}/${m.subject}`
        const agg = found.get(key) ?? {
          suite,
          subject: m.subject,
          drops: 0,
          worstDrop: 0,
          guard,
          from: a,
          to: b,
          refs: [],
          lastTimestamp: curr.control?.timestamp ?? null,
        }
        agg.drops++
        if (drop > agg.worstDrop) {
          agg.worstDrop = drop
          agg.guard = guard
          agg.from = a
          agg.to = b
        }
        agg.lastTimestamp = curr.control?.timestamp ?? agg.lastTimestamp
        for (const doc of [prev, curr]) {
          const ref = doc.file ?? `bench:${suite}@${doc.control?.timestamp ?? '?'}`
          if (!agg.refs.includes(ref)) agg.refs.push(ref)
        }
        found.set(key, agg)
      }
    }
  }

  return [...found.values()]
    .map((agg) => ({
      rung: 'code',
      slug: slug(`bench-${agg.suite}-${agg.subject}`),
      goal: `Investigate the ${agg.suite}/${agg.subject} bench regression (accuracy_pct ${agg.from} → ${agg.to}).`,
      label: `Bench regression: ${agg.suite}/${agg.subject} −${agg.worstDrop.toFixed(1)}pt`,
      evidenceRefs: capRefs(agg.refs),
      estCostMin: RUNG_COST_MIN.code,
      severity: agg.worstDrop >= 10 ? 3 : agg.worstDrop >= 5 ? 2 : 1,
      observed: {
        samples: agg.drops,
        drop: agg.worstDrop,
        guard: agg.guard,
        from: agg.from,
        to: agg.to,
        lastTimestamp: agg.lastTimestamp,
      },
    }))
    .sort((a, b) => (a.slug < b.slug ? -1 : 1))
}

/**
 * Habitsmith drift flags → rung:'skills' re-verify candidates. `status` is the
 * ~/.angel0/habitsmith/status.json artifact (compileHabitsStatus shape:
 * {v, generatedAt, driftBelief, facts:[{id, name, belief, drifting, skill…}]}).
 * Pure; a null/empty status yields no candidates.
 */
export function driftSignals(status) {
  return (status?.facts ?? [])
    .filter((f) => f?.drifting === true)
    .map((f) => {
      const belief = Number(f.belief) || 0
      return {
        rung: 'skills',
        slug: slug(`drift-${f.name ?? f.id}`),
        goal: `Re-verify the '${f.name ?? f.id}' habit — its steps are drifting (belief ${belief.toFixed(2)}).`,
        label: `Habit drift: ${f.name ?? f.id}`,
        evidenceRefs: [`fact:${f.id}`],
        estCostMin: RUNG_COST_MIN.skills,
        severity: belief < 0.35 ? 3 : belief < 0.5 ? 2 : 1,
        observed: {
          samples: Number(f.skill?.uses) || 1,
          belief,
          driftBelief: Number(status?.driftBelief) || null,
        },
      }
    })
    .sort((a, b) => (a.slug < b.slug ? -1 : 1))
}

// Never let a secret-named knob become agenda text (reflex-reconfigure's rule).
const isSecretName = (name) => /KEY|TOKEN|SECRET|AUTH|PASSWORD/i.test(String(name ?? ''))

/**
 * Reflex Tier-B entries from ~/.angel0/reflex/proposals.md → rung:'config'
 * candidates. Parses the renderProposals line shape:
 * `- \`KNOB=value\` — evidence…`. These already concluded with interventional
 * evidence but are propose-only; the agenda item surfaces them for a verdict.
 * Pure over the markdown text.
 */
export function reflexPendingSignals(proposalsMdText) {
  const out = []
  const re = /^-\s*`([A-Za-z_][A-Za-z0-9_]*)=([^`]*)`(?:\s*[—–-]+\s*(.*))?\s*$/gm
  let m
  while ((m = re.exec(String(proposalsMdText ?? ''))) !== null) {
    const [, knob, value, evidence] = m
    if (isSecretName(knob)) continue
    out.push({
      rung: 'config',
      slug: slug(`reflex-${knob}-${value}`),
      goal: `Confirm and adopt the reflex Tier-B proposal ${knob}=${value}.`,
      label: `Reflex Tier-B pending: ${knob}=${value}`,
      evidenceRefs: [`proposals.md:${knob}=${value}`],
      estCostMin: RUNG_COST_MIN.config,
      // Interventional evidence already exists behind a Tier-B entry.
      severity: 2,
      observed: { samples: 1, evidence: (evidence ?? '').trim() },
    })
  }
  return out.sort((a, b) => (a.slug < b.slug ? -1 : 1))
}

/**
 * Repo-dossier trap facts believed at ≥ TRAP_BELIEF_MIN (via repo-dossier's own
 * factBelief blend: observational rate + probe edges + age decay) →
 * rung:'code' "make <ritual> stop failing" candidates. Pure; reads the graph,
 * never writes it.
 *
 * @param {CausalGraph} graph
 * @param {{now?:string, halfLifeDays?:number}} [opts]
 */
export function dossierTrapSignals(graph, opts = {}) {
  return graph
    .nodesOfType(NODE_TYPE.HYPOTHESIS)
    .filter((n) => n.projectId === 'repo-dossier' && n.factKind === 'trap')
    .map((n) => ({ node: n, belief: factBelief(graph, n, opts).belief }))
    .filter(({ belief }) => belief >= TRAP_BELIEF_MIN)
    .map(({ node: n, belief }) => {
      const o = n.observed || {}
      const fails = Number(o.fails) || 0
      return {
        rung: 'code',
        slug: slug(n.id.replace(/^hyp_dossier_/, '')),
        goal: `Make \`${n.factText}\` stop failing in ${n.repoRoot ?? n.repoKey}.`,
        label: `Trap: \`${n.factText}\` keeps failing`,
        evidenceRefs: [`fact:${n.id}`],
        estCostMin: RUNG_COST_MIN.code,
        severity: fails >= 9 ? 3 : fails >= 6 ? 2 : 1,
        observed: {
          samples: Number(o.runs) || fails || 1,
          fails,
          belief: Number(belief.toFixed(3)),
        },
      }
    })
    .sort((a, b) => (a.slug < b.slug ? -1 : 1))
}

/**
 * All five adapters over their substrates, concatenated. Pure convenience for
 * the CLI and the tick (C3); any input may be absent.
 *
 * @param {{rows?:object[], reports?:object[], habitsStatus?:object|null,
 *          proposalsMd?:string, graph?:CausalGraph|null}} inputs
 * @param {{now?:string, lookbackDays?:number, halfLifeDays?:number}} [opts]
 */
export function mineAgenda(inputs = {}, opts = {}) {
  return [
    ...turnFailureSignals(inputs.rows ?? [], opts),
    ...benchDeltaSignals(inputs.reports ?? []),
    ...driftSignals(inputs.habitsStatus ?? null),
    ...reflexPendingSignals(inputs.proposalsMd ?? ''),
    ...(inputs.graph ? dossierTrapSignals(inputs.graph, opts) : []),
  ]
}

// ─── ingest ──────────────────────────────────────────────────────────────────

/** Deterministic node id for one agenda candidate. */
export function agendaNodeId(rung, slugStr) {
  return `conductor_${rung}_${slugStr}`
}

/**
 * Fold mined candidates into the graph as conductor HYPOTHESIS nodes.
 * Idempotent on the deterministic id: an existing node gets its lastSeenAt +
 * observational counts refreshed (goal/label are never rewritten — the id owns
 * the identity), a new one is minted status 'testing'. NO signed edges are
 * added here — mining observes; dispatch verdicts (C5) conclude.
 *
 * @param {CausalGraph} graph
 * @param {object[]} candidates  adapter candidate structs
 * @param {{now?:string}} [opts]
 * @returns {{graph:CausalGraph, added:{nodes:number, updated:number}, agenda:string[]}}
 */
export function ingestAgenda(graph, candidates, opts = {}) {
  const now = opts.now || new Date().toISOString()
  let nodes = 0
  let updated = 0
  const agenda = []

  for (const c of candidates || []) {
    const id = agendaNodeId(c.rung, c.slug)
    agenda.push(id)
    if (graph.hasNode(id)) {
      const prev = graph.getNode(id)
      graph.updateNode(id, {
        lastSeenAt: now,
        observed: c.observed ?? prev.observed,
        evidenceRefs: c.evidenceRefs ?? prev.evidenceRefs,
        severity: c.severity ?? prev.severity,
        estCostMin: c.estCostMin ?? prev.estCostMin,
        seenCount: (Number(prev.seenCount) || 1) + 1,
      })
      updated++
      continue
    }
    graph.addNode({
      id,
      type: NODE_TYPE.HYPOTHESIS,
      hypothesisId: id,
      projectId: CONDUCTOR_PROJECT.id,
      projectLabel: CONDUCTOR_PROJECT.label,
      // Per-rung metric: agenda items competing for the same rung share it,
      // which is exactly the scorer's shared-metric impact term.
      metric: { name: `conductor-${c.rung}`, direction: 'higher' },
      status: 'testing',
      proposedAt: now,
      lastSeenAt: now,
      concludedAt: null,
      outcome: 'neutral',
      factKind: 'agenda',
      label: c.label ?? c.goal,
      question: `Is "${c.goal}" still the most worthwhile ${c.rung} improvement?`,
      prediction: `Dispatching "${c.goal}" produces a verified ${c.rung}-rung improvement.`,
      rung: c.rung,
      goal: c.goal,
      evidenceRefs: c.evidenceRefs ?? [],
      estCostMin: c.estCostMin,
      severity: c.severity,
      observed: c.observed ?? null,
      seenCount: 1,
    })
    nodes++
  }
  return { graph, added: { nodes, updated }, agenda }
}

/** All conductor agenda nodes in the graph, optionally scoped to one rung. */
export function agendaNodes(graph, rung) {
  return graph
    .nodesOfType(NODE_TYPE.HYPOTHESIS)
    .filter(isConductor)
    .filter((n) => !rung || n.rung === rung)
}

// ─── propose ─────────────────────────────────────────────────────────────────

/**
 * Rank the agenda and propose the single most worthwhile item for the next
 * night window. scoreHypotheses VERBATIM (entropy × impact ÷ cost), filtered
 * to projectId 'conductor', then re-weighted post hoc:
 *
 *   priority = score × (1 + log2(1 + samples)) × severity / max(1, estCostMin/10)
 *
 *   • samples  — observed evidence volume; well-evidenced items break entropy
 *     ties against speculative ones (the config-causal evidence factor).
 *   • severity — 1–3, the adapter's read on how much the failure hurts.
 *   • cost     — estimated dispatch minutes, floored so a cheap item can't
 *     divide by less than one tick's worth of effort.
 *
 * Deterministic (id tiebreak); always dryRun — dispatch is C3's job, behind
 * the arming keys, the window, the lock, and the budget.
 *
 * @param {CausalGraph} graph
 * @param {{limit?:number, cost?:number, exclude?:Set|string[]}} [opts]
 * @returns {{proposal:object|null, ranking:object[], dryRun:true}}
 */
export function proposeAgenda(graph, opts = {}) {
  const scored = scoreHypotheses(graph, opts)
  const ranked = scored
    .map((row) => {
      const node = graph.getNode(row.id)
      if (!isConductor(node)) return null
      const samples = Number.isFinite(node.observed?.samples) ? node.observed.samples : 0
      const severity = Math.min(3, Math.max(1, Number(node.severity) || 1))
      const estCostMin = Number.isFinite(node.estCostMin) ? node.estCostMin : 10
      const priority =
        (row.score * (1 + Math.log2(1 + samples)) * severity) / Math.max(1, estCostMin / 10)
      return {
        hypothesisId: row.id,
        label: node.label,
        rung: node.rung,
        goal: node.goal,
        evidenceRefs: node.evidenceRefs ?? [],
        estCostMin,
        severity,
        observed: node.observed ?? null,
        score: row.score,
        priority,
        rationale: `${row.rationale} · ${samples} sample(s) · sev ${severity} · ~${estCostMin}min`,
      }
    })
    .filter(Boolean)
    .sort((a, b) => b.priority - a.priority || (a.hypothesisId < b.hypothesisId ? -1 : 1))
  return { proposal: ranked[0] ?? null, ranking: ranked.slice(0, opts.limit ?? 8), dryRun: true }
}

// ─── CLI ─────────────────────────────────────────────────────────────────────
// I/O lives here only; everything above is pure. `--mine` folds the five
// substrates into the shared graph; `--rank` prints the priority queue. Same
// graph-file caveats as the siblings (no locking; the conductor tick, C3, is
// the serializing scheduler for unattended runs).

async function cli(argv) {
  const { readFileSync, writeFileSync, existsSync, readdirSync } =
    await import('./private-store-fs.mjs')
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
  const reportsDir = flag('--reports-dir') || workerPaths().reports
  const habitsStatusPath = flag('--habits-status') || join(workerPaths().habits, 'status.json')
  const proposalsPath = flag('--proposals') || join(workerPaths().reflex, 'proposals.md')

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
  const loadReports = () => {
    if (!existsSync(reportsDir)) return []
    return readdirSync(reportsDir)
      .filter((f) => /^control_.+_\d{14}\.json$/.test(f))
      .sort()
      .map((f) => {
        try {
          return { file: f, ...JSON.parse(readFileSync(join(reportsDir, f), 'utf8')) }
        } catch {
          return null
        }
      })
      .filter(Boolean)
  }
  const loadHabitsStatus = () => {
    try {
      return JSON.parse(readFileSync(habitsStatusPath, 'utf8'))
    } catch {
      return null
    }
  }
  const loadProposalsMd = () => {
    try {
      return readFileSync(proposalsPath, 'utf8')
    } catch {
      return ''
    }
  }

  if (argv.includes('--mine')) {
    const graph = loadGraph()
    const candidates = mineAgenda({
      rows: loadLedger(),
      reports: loadReports(),
      habitsStatus: loadHabitsStatus(),
      proposalsMd: loadProposalsMd(),
      graph,
    })
    const { added } = ingestAgenda(graph, candidates)
    writeFileSync(graphPath, JSON.stringify(graph.serialize(), null, 2))
    const byRung = {}
    for (const c of candidates) byRung[c.rung] = (byRung[c.rung] || 0) + 1
    console.log(
      `conductor mine → ${graphPath}\n` +
        `  ${candidates.length} candidate(s): ` +
        `${
          Object.entries(byRung)
            .map(([r, n]) => `${r} ${n}`)
            .join(' · ') || '(none)'
        }\n` +
        `  ${added.nodes} new + ${added.updated} refreshed agenda node(s)`,
    )
    return 0
  }

  if (argv.includes('--rank')) {
    const graph = loadGraph()
    const { ranking } = proposeAgenda(graph, { limit: 12 })
    console.log(`conductor agenda (top ${ranking.length}):\n`)
    ranking.forEach((r, i) =>
      console.log(
        `${String(i + 1).padStart(2)}. [${r.rung}] ${r.goal}\n` +
          `    priority ${r.priority.toFixed(3)} · ${r.rationale}`,
      ),
    )
    if (ranking.length === 0) console.log('  (no agenda — run --mine first)')
    return 0
  }

  console.log(
    'usage: node scripts/conductor.mjs --mine|--rank [--graph PATH] [--ledger PATH]' +
      ' [--reports-dir PATH] [--habits-status PATH] [--proposals PATH]',
  )
  return 2
}

const isCli =
  process.argv[1] && (await import('node:url')).fileURLToPath(import.meta.url) === process.argv[1]
if (isCli) {
  runGraphCli(cli, process.argv.slice(2)).then((code) => process.exit(code ?? 0))
}
