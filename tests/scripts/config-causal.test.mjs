// Tests for config-causal: angel0's own configuration → causal hypotheses,
// ranked by the SAME EIG engine that drives research + self-model discovery.
// Isolated graphs verify that config hypotheses leave existing research intact.

import { test } from 'node:test'
import assert from 'node:assert/strict'

import CausalGraph, { NODE_TYPE } from '../../lib/research/CausalGraph.js'
import { scoreHypotheses } from '../../scripts/runtime/causal-loop.mjs'
import {
  seedConfigHypotheses,
  ledgerMetrics,
  ingestLedgerObservations,
  proposeConfigExperiment,
  configHypotheses,
  configHypothesisId,
  KNOB_CATALOG,
  CONFIG_PROJECT,
} from '../../scripts/runtime/config-causal.mjs'

const NOW = '2026-07-04T00:00:00.000Z'

// Total hypotheses the catalog seeds = Σ (values × metrics) per knob.
const CATALOG_TOTAL = KNOB_CATALOG.reduce((n, k) => n + k.values.length * k.metrics.length, 0)

// A ledger `turn` row with a given AGG seat and optional failover.
const row = (agg, { stop = 'answer', failover = false, tokens = 1000 } = {}) => ({
  kind: 'turn',
  v: 1,
  cfg: { hash: 'h', knobs: { ANGEL_SOTA_MOA_AGG_CLUB: agg } },
  outcome: {
    ok: stop === 'answer',
    stop,
    latency_ms: 5000,
    hops: 3,
    tokens: { [agg]: { in: tokens, out: 200 } },
    counters: { deferred_nudges: 0, spin: 0, err_streak: 0, churn: 0 },
    failovers: failover ? [{ from: 'longcat', to: 'openai', reason: 'raw tool markup' }] : [],
  },
})

// The failover story: longcat AGG fumbles raw tool markup, codex-run doesn't.
const LEDGER = [
  ...Array.from({ length: 5 }, (_, i) => row('longcat', { failover: i < 3 })),
  ...Array.from({ length: 5 }, () => row('codex-run')),
]

test('seed mints one testing config hypothesis per (knob, value, metric)', () => {
  const g = new CausalGraph()
  const { added, hypotheses } = seedConfigHypotheses(g, { now: NOW })

  assert.equal(added.nodes, CATALOG_TOTAL)
  assert.equal(added.updated, 0)
  assert.equal(hypotheses.length, CATALOG_TOTAL)

  const configs = configHypotheses(g)
  assert.equal(configs.length, CATALOG_TOTAL)
  for (const h of configs) {
    assert.equal(h.type, NODE_TYPE.HYPOTHESIS)
    assert.equal(h.status, 'testing') // open → scorer surfaces it
    assert.equal(h.projectId, CONFIG_PROJECT.id)
    assert.ok(h.metric?.name)
    assert.ok(h.knobName?.startsWith('ANGEL_'))
    assert.ok(h.knobValue !== undefined)
    assert.deepEqual(h.patchTargets, ['.angel.auto.env'])
  }

  // The headline knob is present with the right id shape.
  const id = configHypothesisId('agg', 'codex-run', 'failover_rate')
  assert.equal(id, 'reflex_agg_codex-run_failover_rate')
  assert.ok(g.hasNode(id))
})

test('seed is idempotent — re-seed refreshes, never duplicates', () => {
  const g = new CausalGraph()
  seedConfigHypotheses(g, { now: NOW })
  const before = g.nodeCount()

  const { added } = seedConfigHypotheses(g, { now: '2026-07-05T00:00:00.000Z' })
  assert.equal(added.nodes, 0)
  assert.equal(added.updated, CATALOG_TOTAL)
  assert.equal(g.nodeCount(), before)
  assert.equal(
    g.getNode(configHypothesisId('agg', 'codex-run', 'failover_rate')).lastSeenAt,
    '2026-07-05T00:00:00.000Z',
  )
})

test('the generic EIG scorer ranks config hypotheses unchanged (zero engine edits)', () => {
  const g = new CausalGraph()
  seedConfigHypotheses(g, { now: NOW })
  const scored = scoreHypotheses(g)
  assert.equal(scored.length, CATALOG_TOTAL)
  // Every open hypothesis is neutral (p=0.5) → entropy 1.0, the EIG maximum.
  for (const s of scored) assert.ok(Math.abs(s.entropy - 1) < 1e-9)
})

test('ledgerMetrics aggregates per-knob-value + a seat failover breakdown', () => {
  const m = ledgerMetrics(LEDGER)
  assert.equal(m.total, 10)

  const agg = m.byKnob.ANGEL_SOTA_MOA_AGG_CLUB
  assert.ok(agg.longcat && agg['codex-run'])
  assert.equal(agg.longcat.n, 5)
  // 3 of 5 longcat turns failed over; 0 of 5 codex-run turns did.
  assert.ok(Math.abs(agg.longcat.metrics.failover_rate.mean - 0.6) < 1e-9)
  assert.equal(agg['codex-run'].metrics.failover_rate.mean, 0)

  // Failover breakdown groups by the seat that fumbled + why.
  assert.equal(m.failoversByClub.longcat.count, 3)
  assert.equal(m.failoversByClub.longcat.reasons['raw tool markup'], 3)
})

test('observations re-weight so the codex-run AGG swap outranks longcat (the hand-fix)', () => {
  const g = new CausalGraph()
  seedConfigHypotheses(g, { now: NOW })
  const { updated } = ingestLedgerObservations(g, LEDGER, { now: NOW })
  assert.ok(updated >= 2, 'both AGG failover_rate hypotheses get evidence')

  // codex-run looks better than the longcat baseline (lower failover_rate) → +gradient
  const codex = g.getNode(configHypothesisId('agg', 'codex-run', 'failover_rate'))
  const longcat = g.getNode(configHypothesisId('agg', 'longcat', 'failover_rate'))
  assert.ok(codex.observed && codex.observed.gradient > 0)
  assert.ok(longcat.observed && longcat.observed.gradient < 0)

  const { ranking, proposal, dryRun } = proposeConfigExperiment(g, { limit: 100 })
  assert.equal(dryRun, true)
  assert.ok(proposal)
  const codexRank = ranking.findIndex((r) => r.hypothesisId === codex.id)
  const longcatRank = ranking.findIndex((r) => r.hypothesisId === longcat.id)
  assert.ok(codexRank >= 0 && longcatRank >= 0)
  assert.ok(codexRank < longcatRank, 'codex-run swap ranks above staying on longcat')
})

test('cost term deprioritizes expensive experiments (dormant hook, now driven)', () => {
  const g = new CausalGraph()
  seedConfigHypotheses(g, { now: NOW })
  const { ranking } = proposeConfigExperiment(g, { limit: 500 })
  // ALWAYS (cost 12) and a seat (cost 6) both mint accuracy_pct hypotheses with
  // identical entropy+impact → identical base score. The cheaper seat must rank
  // above the pricier ALWAYS toggle on that shared metric.
  const acc = ranking.filter((r) => r.metric === 'accuracy_pct')
  const seat = acc.find((r) => r.knob === 'ANGEL_SOTA_MOA_JUDGE_CLUB')
  const always = acc.find((r) => r.knob === 'ANGEL_SOTA_MOA_ALWAYS')
  assert.ok(seat && always)
  assert.ok(seat.priority > always.priority)
})

test('empty ledger yields no observations but still ranks the seeded queue', () => {
  const g = new CausalGraph()
  seedConfigHypotheses(g, { now: NOW })
  const { updated, total } = ingestLedgerObservations(g, [], { now: NOW })
  assert.equal(updated, 0)
  assert.equal(total, 0)
  const { proposal, ranking } = proposeConfigExperiment(g)
  assert.ok(proposal)
  assert.ok(ranking.length > 0)
})

test('config hypotheses coexist with unrelated research without modifying its nodes', () => {
  const g = new CausalGraph()
  g.addNode({
    id: 'research-one',
    type: NODE_TYPE.HYPOTHESIS,
    projectId: 'research',
    label: 'Fixture research hypothesis',
    status: 'testing',
    impact: 2,
    cost: 1,
  })
  g.addNode({
    id: 'research-two',
    type: NODE_TYPE.HYPOTHESIS,
    projectId: 'research',
    label: 'Second fixture hypothesis',
    status: 'testing',
    impact: 1,
    cost: 2,
  })
  const originalNodes = JSON.stringify(g.nodesOfType(NODE_TYPE.HYPOTHESIS))
  const researchBefore = g
    .nodesOfType(NODE_TYPE.HYPOTHESIS)
    .filter((h) => h.projectId !== CONFIG_PROJECT.id).length

  seedConfigHypotheses(g, { now: NOW })
  ingestLedgerObservations(g, LEDGER, { now: NOW })

  // research hypotheses untouched; config hypotheses added alongside
  const researchAfter = g
    .nodesOfType(NODE_TYPE.HYPOTHESIS)
    .filter((h) => h.projectId !== CONFIG_PROJECT.id).length
  assert.equal(researchAfter, researchBefore)
  assert.equal(
    JSON.stringify(
      g.nodesOfType(NODE_TYPE.HYPOTHESIS).filter((h) => h.projectId !== CONFIG_PROJECT.id),
    ),
    originalNodes,
  )
  assert.equal(configHypotheses(g).length, CATALOG_TOTAL)

  // the generic scorer still ranks research hypotheses too (no interference)
  const scored = scoreHypotheses(g)
  assert.ok(scored.some((r) => g.getNode(r.id).projectId !== CONFIG_PROJECT.id))
  // and the config proposal only ever returns reflex hypotheses
  const { proposal } = proposeConfigExperiment(g)
  assert.equal(g.getNode(proposal.hypothesisId).projectId, CONFIG_PROJECT.id)
})
