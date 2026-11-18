import { updateBeliefs } from '../research/causal-loop.mjs'
import { EDGE_TYPE } from '../research/CausalGraph.js'

const INDIRECT_COMMANDS = new Set(['env', 'command', 'builtin', 'exec', 'timeout', 'gtimeout', 'nice', 'nohup', 'sudo', 'doas', 'time', 'xargs', 'find', 'sh', 'bash', 'dash', 'zsh', 'fish'])

const LEGACY_P0_LABELS = new Set([
  'p0: root + binary present (weak)',
  'p0: root + binary present (weak; not a verified run)',
  'p0: command binary missing — still fails',
  'p0: command binary not on PATH',
  'p0: workspace root missing',
])

export function commandToken(text) {
  const source = String(text ?? '').trim()
  for (let i = 0; i < source.length;) {
    while (/\s/.test(source[i] ?? '') && i < source.length) i++
    const start = i
    let word = '', quote = null
    while (i < source.length) {
      const c = source[i]
      if (!quote && /\s/.test(c)) break
      if (quote === "'") {
        if (c === "'") quote = null
        else word += c
      } else if (c === '\\') {
        if (++i >= source.length) return null
        const next = source[i]
        if (quote === '"' && !['$', '`', '"', '\\', '\n'].includes(next)) word += '\\'
        if (next !== '\n') word += next
      } else if (c === quote) quote = null
      else if (!quote && (c === "'" || c === '"')) quote = c
      else if (c === '$' || c === '`' || (!quote && /[|;&<>()#~*?\[]/.test(c))) return null
      else word += c
      i++
    }
    if (quote) return null
    const raw = source.slice(start, i)
    if (/^[A-Za-z_][A-Za-z0-9_]*=/.test(raw)) {
      if (raw.startsWith('PATH=')) return null
      continue
    }
    if (INDIRECT_COMMANDS.has(word.split('/').at(-1))) return null
    return word && !word.startsWith('-') ? word : null
  }
  return null
}

export function p0Verdict(fact, { rootExists, binaryFound }) {
  return {
    ...p0Observation(fact, { rootExists, binaryFound }),
    evidenceKey: 'p0-environment',
    verifies: false,
  }
}

function p0Observation(fact, { rootExists, binaryFound }) {
  if (!rootExists) {
    return fact.factKind === 'trap'
      ? { verdict: 'inconclusive', confidence: 0.35, evidence: 'p0: workspace root missing' }
      : { verdict: 'contradicts', confidence: 0.7, evidence: 'p0: workspace root missing' }
  }
  if (binaryFound === null) {
    return { verdict: 'inconclusive', confidence: 0, evidence: 'p0: command lookup unavailable or dynamic' }
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
    : { verdict: 'supports', confidence: 0.4, evidence: 'p0: root + binary present (weak; not a verified run)' }
}

export function p1Verdict(fact, { exit, timedOut }) {
  if (!timedOut && (!Number.isInteger(exit) || exit < 0)) {
    return { verdict: 'inconclusive', confidence: 0, evidenceKey: 'p1-execution', evidence: 'p1: no exit status — command outcome unknown' }
  }
  const failed = timedOut || exit !== 0
  const expectFail = fact.factKind === 'trap'
  const asExpected = failed === expectFail
  const how = timedOut ? 'timed out' : `exit ${exit}`
  return {
    verdict: asExpected ? 'supports' : 'contradicts',
    confidence: timedOut ? 0.7 : 0.85,
    evidenceKey: 'p1-execution',
    evidence: `p1: re-ran \`${fact.factText}\` — ${how}`,
  }
}

export function applyProbe(graph, factId, verdict, { now }) {
  const snapshot = verdict.evidenceKey === 'p0-environment' && verdict.verifies === false
  const supersedesSnapshot = snapshot || verdict.evidenceKey === 'p1-execution'
  const previousVotes = supersedesSnapshot ? graph.queryEdges(e =>
    e.dst === factId && [EDGE_TYPE.SUPPORTS, EDGE_TYPE.CONTRADICTS].includes(e.type) &&
    (['p0-environment', 'p0-presence'].includes(e.evidenceKey) || LEGACY_P0_LABELS.has(e.label)),
  ) : []
  const { changes, experimentNodeId } = updateBeliefs(graph, {
    hypothesisId: factId,
    verdict: verdict.verdict,
    confidence: verdict.confidence,
    evidence: verdict.evidence,
    now,
    bud: false,
  })
  if (snapshot) {
    if (verdict.verdict !== 'inconclusive') {
      const type = verdict.verdict === 'supports' ? EDGE_TYPE.SUPPORTS : EDGE_TYPE.CONTRADICTS
      const [vote] = graph.queryEdges(e =>
        e.src === experimentNodeId && e.dst === factId && e.type === type)
      graph.addEdge({ ...vote, evidenceKey: 'p0-environment' })
    }
    graph.addEdge({ src: experimentNodeId, dst: factId, type: EDGE_TYPE.TESTS, confidence: 0, label: verdict.evidence, auditOnly: true })
  }
  for (const edge of previousVotes) {
    graph.addEdge({ src: edge.src, dst: factId, type: EDGE_TYPE.TESTS, confidence: 0, label: edge.label, auditOnly: true })
    graph.removeEdge(edge.id)
  }
  graph.updateNode(factId, {
    status: 'testing', concludedAt: null, lastProbedAt: now,
    ...(['supports', 'contradicts'].includes(verdict.verdict) && verdict.verifies !== false ? { lastVerifiedAt: now } : {}),
  })
  return changes
}
