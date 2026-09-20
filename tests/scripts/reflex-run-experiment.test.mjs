// Tests for reflex-run-experiment: the interventional runner that turns a config
// proposal into a Control Bench A/B and folds the signed verdict into the graph.
// The bench spawn is not exercised here — concludeFromReport takes a parsed
// report, so the conclude-with-correct-sign logic is proven with a mock report
// (no Python, no fleet).

import { test } from 'node:test'
import assert from 'node:assert/strict'

import CausalGraph from '../../lib/research/CausalGraph.js'
import { beliefProbability } from '../../scripts/causal-loop.mjs'
import { seedConfigHypotheses, configHypothesisId } from '../../scripts/config-causal.mjs'
import { contractFromControlExperiment } from '../../scripts/experiment-contracts.mjs'
import {
  buildSubjectSpec,
  extractAB,
  varianceGuard,
  concludeFromReport,
} from '../../scripts/reflex-run-experiment.mjs'

const NOW = '2026-07-04T00:00:00.000Z'
const AGG_ACC = configHypothesisId('agg', 'codex-run', 'accuracy_pct')

// A control-bench contract for AGG=codex-run vs a longcat baseline.
const makeContract = () =>
  contractFromControlExperiment(
    {
      hypothesisId: AGG_ACC,
      knob: 'ANGEL_SOTA_MOA_AGG_CLUB',
      value: 'codex-run',
      label: 'agg codex-run',
    },
    {
      now: NOW,
      suite: 'control-code-v2',
      seeds: 2,
      quick: true,
      baselineKnobs: { ANGEL_SOTA_MOA_AGG_CLUB: 'longcat' },
    },
  )

// A parsed Control Bench report with the two arms.
const report = (baselineAcc, treatmentAcc, std = 5) => ({
  control: { suite: 'control-code-v2' },
  metrics: [
    { subject: 'reflex-baseline', accuracy_pct: baselineAcc, accuracy_std: std, n: 6 },
    { subject: 'reflex-treatment', accuracy_pct: treatmentAcc, accuracy_std: std, n: 6 },
  ],
})

test('contractFromControlExperiment builds a control-bench + metric_delta contract', () => {
  const c = makeContract()
  assert.equal(c.runner.type, 'control-bench')
  assert.equal(c.runner.suite, 'control-code-v2')
  assert.equal(c.verifier.type, 'metric_delta')
  assert.equal(c.verifier.direction, 'higher')
  assert.deepEqual(c.intervention.knobs, { ANGEL_SOTA_MOA_AGG_CLUB: 'codex-run' })
  assert.deepEqual(c.intervention.baselineKnobs, { ANGEL_SOTA_MOA_AGG_CLUB: 'longcat' })
})

test('buildSubjectSpec produces the two-arm do-operator spec', () => {
  const spec = buildSubjectSpec(makeContract())
  assert.equal(spec.suite, 'control-code-v2')
  assert.equal(spec.seeds, 2)
  assert.equal(spec.subjects.length, 2)
  const t = spec.subjects.find((s) => s.name === 'reflex-treatment')
  const b = spec.subjects.find((s) => s.name === 'reflex-baseline')
  assert.deepEqual(t.env, { ANGEL_SOTA_MOA_AGG_CLUB: 'codex-run' })
  assert.deepEqual(b.env, { ANGEL_SOTA_MOA_AGG_CLUB: 'longcat' })
})

test('extractAB + varianceGuard read the report and size the guard', () => {
  const ab = extractAB(report(40, 75, 5))
  assert.equal(ab.baseline.accuracy_pct, 40)
  assert.equal(ab.treatment.accuracy_pct, 75)
  assert.equal(varianceGuard(ab), 10) // 2 × max std (5)
})

test('treatment clearly better → SUPPORTS, belief rises, hypothesis concludes', () => {
  const g = new CausalGraph()
  seedConfigHypotheses(g, { now: NOW })
  const before = beliefProbability(g, AGG_ACC)
  assert.equal(before, 0.5) // untested → neutral

  const { verification, ab } = concludeFromReport(g, makeContract(), report(40, 75, 5), {
    now: NOW,
  })
  assert.equal(verification.verdict, 'supports')
  assert.equal(ab.treatment.accuracy_pct, 75)
  // signed edge folded in → belief moves above neutral, hypothesis concluded
  assert.ok(beliefProbability(g, AGG_ACC) > before)
  assert.equal(g.getNode(AGG_ACC).status, 'concluded')
  assert.equal(g.getNode(AGG_ACC).outcome, 'positive')
})

test('treatment clearly worse → CONTRADICTS with correct sign', () => {
  const g = new CausalGraph()
  seedConfigHypotheses(g, { now: NOW })
  const { verification } = concludeFromReport(g, makeContract(), report(75, 40, 5), { now: NOW })
  assert.equal(verification.verdict, 'contradicts')
  assert.ok(beliefProbability(g, AGG_ACC) < 0.5)
  assert.equal(g.getNode(AGG_ACC).outcome, 'negative')
})

test('delta inside the variance guard → inconclusive, not concluded', () => {
  const g = new CausalGraph()
  seedConfigHypotheses(g, { now: NOW })
  // delta = 3, guard = 2×5 = 10 → within noise
  const { verification } = concludeFromReport(g, makeContract(), report(50, 53, 5), { now: NOW })
  assert.equal(verification.verdict, 'inconclusive')
  assert.equal(g.getNode(AGG_ACC).status, 'testing') // stays open
})

test('null result (delta 0, no variance) is inconclusive, never a false SUPPORTS', () => {
  // Regression: with --seeds 1 both arms can score 0 → std 0. An unfloored guard
  // of 0 made delta=0 read as SUPPORTS. The floor keeps a null result open.
  const g = new CausalGraph()
  seedConfigHypotheses(g, { now: NOW })
  assert.equal(varianceGuard(extractAB(report(0, 0, 0))), 1.0)
  const { verification } = concludeFromReport(g, makeContract(), report(0, 0, 0), { now: NOW })
  assert.equal(verification.verdict, 'inconclusive')
  assert.equal(g.getNode(AGG_ACC).status, 'testing')
})

test('a report missing an arm is skipped, not crashed', () => {
  const g = new CausalGraph()
  seedConfigHypotheses(g, { now: NOW })
  const bad = { metrics: [{ subject: 'reflex-treatment', accuracy_pct: 70, accuracy_std: 1 }] }
  const out = concludeFromReport(g, makeContract(), bad, { now: NOW })
  assert.ok(out.skipped)
  assert.equal(g.getNode(AGG_ACC).status, 'testing')
})

test('unknown seed variance cannot manufacture a causal conclusion', () => {
  const g = new CausalGraph()
  seedConfigHypotheses(g, { now: NOW })
  const unmeasured = report(10, 90, null)
  assert.equal(extractAB(unmeasured).baseline.accuracy_std, null)
  assert.equal(varianceGuard(extractAB(unmeasured)), null)
  const result = concludeFromReport(g, makeContract(), unmeasured, { now: NOW })
  assert.match(result.skipped, /variance/)
  assert.equal(result.verification, null)
  assert.equal(g.getNode(AGG_ACC).status, 'testing')
})
