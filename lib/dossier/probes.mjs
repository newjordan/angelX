// Pure dossier probe rules: observations in, verdicts and graph updates out.
// No probe execution, environment reads, scheduling or persistence here.
import { updateBeliefs } from '../research/causal-loop.mjs'

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
