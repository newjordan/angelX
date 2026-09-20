// causal-loop — the Active Causal Discovery engine.
//
// Pure + testable: no network, no server, no file I/O. Callers pass in a live
// CausalGraph instance; we read it, score it, and (for updateBeliefs) mutate it
// in place. Persistence is the caller's job — keeping this module side-effect
// free is what lets the test suite drive it deterministically.
//
// The loop is: score every open hypothesis by expected information gain →
// pick the most informative one to test next → fold the verdict back into the
// graph as signed evidence, conclude the hypothesis, and bud a follow-up. Over
// many turns this turns the static seed graph into a self-updating belief net.

import { NODE_TYPE, EDGE_TYPE } from '../../lib/research/CausalGraph.js'

// ─── math helpers ──────────────────────────────────────────────────────────

const LOG2 = Math.LN2

// log base 2 — entropy is conventionally measured in bits.
const log2 = (x) => Math.log(x) / LOG2

// Standard logistic. Maps a signed evidence sum onto a probability: 0 → 0.5,
// large positive → ~1, large negative → ~0.
const logistic = (x) => 1 / (1 + Math.exp(-x))

// Keep beliefs strictly inside (0,1) so entropy/log stay finite.
const clampProb = (p) => Math.min(0.999, Math.max(0.001, p))

// ─── belief ──────────────────────────────────────────────────────────────────

/**
 * Belief that a hypothesis's prediction holds, in (0.001, 0.999).
 *
 * We treat each incoming SUPPORTS/CONTRADICTS edge as a signed log-odds nudge
 * weighted by its confidence: SUPPORTS pushes +confidence, CONTRADICTS pushes
 * -confidence. Summing in log-odds space and squashing through a logistic is
 * the natural way to pool independent evidence — two weak supports compound,
 * a support and an equal contradict cancel back to 0.5.
 *
 * We read raw edges (filtered by dst) rather than graph.incoming() so the
 * belief still counts evidence whose source node may not (yet) exist.
 *
 * @returns {number} probability in (0.001, 0.999); 0.5 with no signed evidence.
 */
export function beliefProbability(graph, hypothesisId) {
  let sum = 0
  let seen = 0
  for (const e of graph.edges()) {
    if (e.dst !== hypothesisId) continue
    // Missing confidence on a signed edge still carries some signal; 0.5 is a
    // neutral-but-nonzero default so the edge isn't silently ignored.
    const c = Number.isFinite(e.confidence) ? e.confidence : 0.5
    if (e.type === EDGE_TYPE.SUPPORTS) {
      sum += c
      seen++
    } else if (e.type === EDGE_TYPE.CONTRADICTS) {
      sum -= c
      seen++
    }
  }
  if (seen === 0) return 0.5
  return clampProb(logistic(sum))
}

// ─── scoring ───────────────────────────────────────────────────────────────

/**
 * Score every open hypothesis by expected information gain.
 *
 * Only HYPOTHESIS nodes with status 'testing' are candidates — concluded ones
 * are done. For each we compute:
 *   p       — current belief (see beliefProbability)
 *   entropy — binary entropy of p, in bits. This is the EIG proxy: a near
 *             certain hypothesis (p→0 or 1) has entropy→0 and is barely worth
 *             testing; a neutral p=0.5 has entropy 1.0, the maximum, because a
 *             clean verdict there resolves the most uncertainty.
 *   impact  — importance/connectivity: 1 + (other hypotheses on the same
 *             metric) + (tests edges already aimed at it). Hypotheses on a
 *             hotly-shared metric or with lots of prior testing matter more.
 *   cost    — opts.cost ?? 1; a hook for per-experiment cost weighting.
 *   score   — entropy * impact / cost. Maximize information per unit cost.
 *
 * Deterministic: no Math.random; ties break by id so ordering is stable.
 *
 * @returns {Array<{id,label,metric,status,p,entropy,impact,cost,score,rationale}>}
 *          sorted by score descending.
 */
export function scoreHypotheses(graph, opts = {}) {
  const cost = opts.cost ?? 1
  // Hypotheses to skip (e.g. already tested this discovery run). Set or array of ids.
  const exclude = opts.exclude instanceof Set ? opts.exclude : new Set(opts.exclude || [])
  const hyps = graph.nodesOfType(NODE_TYPE.HYPOTHESIS)

  // How many hypotheses sit on each metric — the shared-metric importance term.
  const metricCounts = new Map()
  for (const h of hyps) {
    const m = h.metric?.name
    if (m) metricCounts.set(m, (metricCounts.get(m) ?? 0) + 1)
  }

  // How many tests edges point at each hypothesis — the prior-effort term.
  const testsByDst = new Map()
  for (const e of graph.edgesOfType(EDGE_TYPE.TESTS)) {
    testsByDst.set(e.dst, (testsByDst.get(e.dst) ?? 0) + 1)
  }

  const scored = hyps
    .filter((h) => h.status === 'testing' && !exclude.has(h.id))
    .map((h) => {
      const p = beliefProbability(graph, h.id)
      const entropy = -p * log2(p) - (1 - p) * log2(1 - p)

      const metricName = h.metric?.name ?? null
      const sharedTotal = metricName ? metricCounts.get(metricName) : 1
      const testsCount = testsByDst.get(h.id) ?? 0
      // 1 baseline + OTHER hypotheses on this metric (sharedTotal - 1) + tests.
      const impact = 1 + Math.max(0, sharedTotal - 1) + testsCount

      const score = (entropy * impact) / cost

      const rationale =
        `p=${p.toFixed(2)} (entropy ${entropy.toFixed(2)}) · ` +
        `metric ${metricName ?? '—'} shared by ${sharedTotal} hypotheses · ` +
        `score ${score.toFixed(2)}`

      return {
        id: h.id,
        label: h.label,
        metric: h.metric,
        status: h.status,
        p,
        entropy,
        impact,
        cost,
        score,
        rationale,
      }
    })

  // Descending by score; id as a deterministic tiebreaker.
  scored.sort((a, b) => b.score - a.score || (a.id < b.id ? -1 : a.id > b.id ? 1 : 0))
  return scored
}

// ─── next experiment ─────────────────────────────────────────────────────────

/**
 * Pick the highest-scoring open hypothesis and build a ready-to-run
 * autoresearch spec for it. The prompt is assembled from the hypothesis so a
 * downstream agent can act on it directly and report a SUPPORTS/CONTRADICTS
 * verdict back into updateBeliefs.
 *
 * @returns {null | {hypothesisId,label,metric,prediction,question,prompt,target_title,score,rationale}}
 */
export function nextExperiment(graph, opts = {}) {
  const top = scoreHypotheses(graph, opts)[0]
  if (!top) return null

  const node = graph.getNode(top.id)
  const metric = node.metric ?? {}
  const direction = metric.direction ?? 'better'
  const question = node.question ?? ''
  const prediction = node.prediction ?? ''

  const prompt =
    `Test the hypothesis "${node.label}": ${question} ` +
    `Prediction: ${prediction} ` +
    `Run experiments and determine whether the prediction holds for metric ` +
    `${metric.name ?? 'the target metric'} (want it ${direction}). ` +
    `Report SUPPORTS or CONTRADICTS with a confidence.`

  return {
    hypothesisId: top.id,
    label: node.label,
    metric,
    prediction,
    question,
    prompt,
    target_title: node.label ?? node.projectLabel ?? top.id,
    score: top.score,
    rationale: top.rationale,
  }
}

// ─── belief update ───────────────────────────────────────────────────────────

// Build a node-id-safe slug from an ISO timestamp.
const sanitizeId = (s) => String(s).replace(/[^0-9a-zA-Z]/g, '')

/**
 * Fold an experiment's verdict back into the graph, mutating it in place.
 *
 * Steps:
 *   1. Resolve the experiment node — reuse experimentNodeId if it exists,
 *      otherwise mint an auto experiment node.
 *   2. Link experiment → hypothesis with a signed edge: SUPPORTS / CONTRADICTS
 *      for a clear verdict, or a low-signal TESTS edge when inconclusive.
 *   3. Update the hypothesis outcome; conclude it when the verdict is decisive
 *      (not inconclusive) and confidence is high enough (>= 0.6).
 *   4. If it just concluded and budding is on, spawn ONE deterministic
 *      follow-up hypothesis and link parent → child with INFORMED.
 *
 * The caller is responsible for serializing — we never touch disk.
 *
 * @param {CausalGraph} graph
 * @param {{hypothesisId, verdict, confidence?, evidence?, experimentNodeId?, now?, bud?}} args
 * @returns {{changes:string[], hypothesis, childHypothesisId:string|null}}
 */
export function updateBeliefs(graph, args) {
  const {
    hypothesisId,
    verdict,
    confidence = 0.7,
    evidence = '',
    experimentNodeId = null,
    now = new Date().toISOString(),
    bud = true,
  } = args ?? {}

  if (!hypothesisId) throw new Error('updateBeliefs: args.hypothesisId is required')
  if (!verdict) throw new Error('updateBeliefs: args.verdict is required')

  const hyp = graph.getNode(hypothesisId)
  if (!hyp) throw new Error(`updateBeliefs: hypothesis "${hypothesisId}" not found`)

  const changes = []
  const label = evidence ? evidence.slice(0, 160) : undefined

  // 1. Resolve / create the experiment node.
  let expId = experimentNodeId
  if (expId && graph.hasNode(expId)) {
    changes.push(`reused experiment node ${expId}`)
  } else {
    // Unique even when many updates land in the same millisecond (e.g. a dry
    // discovery run loops with the default `now`) — otherwise same-ms ids would
    // overwrite each other and silently merge distinct experiments.
    expId = `exp_auto_${sanitizeId(now)}`
    for (let n = 1; graph.hasNode(expId); n++) expId = `exp_auto_${sanitizeId(now)}_${n}`
    graph.addNode({
      id: expId,
      type: NODE_TYPE.EXPERIMENT,
      label: 'Auto experiment ' + now,
      runAt: now,
      evidence,
    })
    changes.push(`created experiment node ${expId}`)
  }

  // 2. Link experiment → hypothesis with a signed (or low-signal) edge.
  let edgeType
  if (verdict === 'supports') edgeType = EDGE_TYPE.SUPPORTS
  else if (verdict === 'contradicts') edgeType = EDGE_TYPE.CONTRADICTS
  else edgeType = EDGE_TYPE.TESTS // inconclusive → it was tested, but no signed signal
  graph.addEdge({ src: expId, dst: hypothesisId, type: edgeType, confidence, label })
  changes.push(`added ${edgeType} edge ${expId} → ${hypothesisId} (confidence ${confidence})`)

  // 3. Update the hypothesis outcome / status.
  const patch = {}
  if (verdict === 'supports') patch.outcome = 'positive'
  else if (verdict === 'contradicts') patch.outcome = 'negative'
  // inconclusive: leave outcome as-is ('neutral').

  const concluded = verdict !== 'inconclusive' && confidence >= 0.6
  if (concluded) {
    patch.status = 'concluded'
    patch.concludedAt = now
  }
  const hypothesis = graph.updateNode(hypothesisId, patch)
  changes.push(
    concluded
      ? `concluded ${hypothesisId} as ${patch.outcome}`
      : `recorded ${verdict} on ${hypothesisId} (still testing)`,
  )

  // 4. Budding — one deterministic follow-up when a hypothesis just concluded.
  let childHypothesisId = null
  if (bud && concluded) {
    const parentHid = hyp.hypothesisId ?? hyp.id.replace(/^hyp_/, '')
    let childId = `hyp_${parentHid}_followup`
    // Guarantee uniqueness if a follow-up already exists.
    let n = 2
    while (graph.hasNode(childId)) childId = `hyp_${parentHid}_followup_${n++}`

    const metricName = hyp.metric?.name ?? 'the metric'
    const projectLabel = hyp.projectLabel ?? hyp.projectId ?? 'this project'
    let childLabel
    let question
    let prediction
    if (verdict === 'supports') {
      // Confirmed effect → probe whether it generalizes.
      childLabel = `Generalization of: ${hyp.label}`
      question = `Does the ${metricName} effect generalize beyond ${projectLabel}?`
      prediction = `The ${metricName} effect holds outside ${projectLabel}.`
    } else {
      // Refuted effect → hunt for the confound behind the null result.
      childLabel = `Confound check: ${hyp.label}`
      question = `What confound explains the null result on: ${hyp.label}?`
      prediction = `A controllable confound accounts for the ${metricName} null result; removing it restores the effect.`
    }

    graph.addNode({
      id: childId,
      type: NODE_TYPE.HYPOTHESIS,
      label: childLabel,
      hypothesisId: childId.replace(/^hyp_/, ''),
      projectId: hyp.projectId,
      projectLabel: hyp.projectLabel,
      metric: hyp.metric ? { ...hyp.metric } : undefined,
      status: 'testing',
      proposedAt: now,
      concludedAt: null,
      question,
      prediction,
      outcome: 'neutral',
      derivedFrom: hyp.id,
    })
    graph.addEdge({
      src: hyp.id,
      dst: childId,
      type: EDGE_TYPE.INFORMED,
      label: `budded from ${verdict} verdict`,
    })
    childHypothesisId = childId
    changes.push(`budded follow-up hypothesis ${childId} (INFORMED from ${hyp.id})`)
  }

  return { changes, hypothesis, childHypothesisId }
}
