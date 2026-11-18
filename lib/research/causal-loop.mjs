import { NODE_TYPE, EDGE_TYPE } from './CausalGraph.js'

const LOG2 = Math.LN2

const log2 = (x) => Math.log(x) / LOG2

const logistic = (x) => 1 / (1 + Math.exp(-x))

const clampProb = (p) => Math.min(0.999, Math.max(0.001, p))

export function beliefProbability(graph, hypothesisId) {
  let sum = 0
  let seen = 0
  for (const e of graph.edges()) {
    if (e.dst !== hypothesisId) continue
    const c = Number.isFinite(e.confidence) ? e.confidence : 0.5
    if (e.type === EDGE_TYPE.SUPPORTS) {
      sum += c
      seen++
    } else if (e.type === EDGE_TYPE.CONTRADICTS) {
      sum -= c
      seen++
    }
  }
  return signedBelief(sum, seen)
}

const signedBelief = (sum, seen) => seen === 0 ? 0.5 : clampProb(logistic(sum))

function aggregateEvidence(graph) {
  const testsByDst = new Map()
  const signedByDst = new Map()
  for (const e of graph.edges()) {
    if (e.type === EDGE_TYPE.TESTS && e.auditOnly !== true) testsByDst.set(e.dst, (testsByDst.get(e.dst) ?? 0) + 1)
    else if (e.type === EDGE_TYPE.SUPPORTS || e.type === EDGE_TYPE.CONTRADICTS) {
      const state = signedByDst.get(e.dst) ?? { sum: 0, seen: 0 }
      const confidence = Number.isFinite(e.confidence) ? e.confidence : 0.5
      state.sum += e.type === EDGE_TYPE.SUPPORTS ? confidence : -confidence
      state.seen++
      signedByDst.set(e.dst, state)
    }
  }
  return { testsByDst, signedByDst }
}

export function beliefProbabilities(graph) {
  const { signedByDst } = aggregateEvidence(graph)
  return new Map([...signedByDst].map(([id, state]) => [id, signedBelief(state.sum, state.seen)]))
}

export function scoreHypotheses(graph, opts = {}) {
  const cost = opts.cost ?? 1
  const exclude = opts.exclude instanceof Set ? opts.exclude : new Set(opts.exclude || [])
  const hyps = graph.nodesOfType(NODE_TYPE.HYPOTHESIS)

  const metricCounts = new Map()
  for (const h of hyps) {
    const m = h.metric?.name
    if (m) metricCounts.set(m, (metricCounts.get(m) ?? 0) + 1)
  }

  const { testsByDst, signedByDst } = aggregateEvidence(graph)

  const scored = hyps
    .filter((h) => h.status === 'testing' && !exclude.has(h.id))
    .map((h) => {
      const signed = signedByDst.get(h.id)
      const p = signed ? signedBelief(signed.sum, signed.seen) : 0.5
      const entropy = -p * log2(p) - (1 - p) * log2(1 - p)

      const metricName = h.metric?.name ?? null
      const sharedTotal = metricName ? metricCounts.get(metricName) : 1
      const testsCount = testsByDst.get(h.id) ?? 0
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

  scored.sort((a, b) => b.score - a.score || (a.id < b.id ? -1 : a.id > b.id ? 1 : 0))
  return scored
}

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

const sanitizeId = (s) => String(s).replace(/[^0-9a-zA-Z]/g, '')

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
  if (!['supports', 'contradicts', 'inconclusive'].includes(verdict)) {
    throw new Error('updateBeliefs: verdict must be supports, contradicts, or inconclusive')
  }
  if (!Number.isFinite(confidence) || confidence < 0 || confidence > 1) {
    throw new Error('updateBeliefs: confidence must be a finite number in [0, 1]')
  }

  const hyp = graph.getNode(hypothesisId)
  if (!hyp) throw new Error(`updateBeliefs: hypothesis "${hypothesisId}" not found`)

  const changes = []
  const label = evidence ? evidence.slice(0, 160) : undefined

  let expId = experimentNodeId
  if (expId && graph.hasNode(expId)) {
    changes.push(`reused experiment node ${expId}`)
  } else {
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

  let edgeType
  if (verdict === 'supports') edgeType = EDGE_TYPE.SUPPORTS
  else if (verdict === 'contradicts') edgeType = EDGE_TYPE.CONTRADICTS
  else edgeType = EDGE_TYPE.TESTS
  graph.addEdge({ src: expId, dst: hypothesisId, type: edgeType, confidence, label })
  changes.push(`added ${edgeType} edge ${expId} → ${hypothesisId} (confidence ${confidence})`)

  const patch = {}
  if (verdict === 'supports') patch.outcome = 'positive'
  else if (verdict === 'contradicts') patch.outcome = 'negative'

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

  let childHypothesisId = null
  if (bud && concluded) {
    const parentHid = hyp.hypothesisId ?? hyp.id.replace(/^hyp_/, '')
    let childId = `hyp_${parentHid}_followup`
    let n = 2
    while (graph.hasNode(childId)) childId = `hyp_${parentHid}_followup_${n++}`

    const metricName = hyp.metric?.name ?? 'the metric'
    const projectLabel = hyp.projectLabel ?? hyp.projectId ?? 'this project'
    let childLabel
    let question
    let prediction
    if (verdict === 'supports') {
      childLabel = `Generalization of: ${hyp.label}`
      question = `Does the ${metricName} effect generalize beyond ${projectLabel}?`
      prediction = `The ${metricName} effect holds outside ${projectLabel}.`
    } else {
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

  return { changes, hypothesis, childHypothesisId, experimentNodeId: expId }
}
