// Tests for conductor C1: telemetry substrates -> agenda hypotheses -> EIG
// ranking. These are pure unit cases; the CLI's filesystem shell is deliberately
// not involved.

import { test } from 'node:test'
import assert from 'node:assert/strict'

import CausalGraph, { NODE_TYPE } from '../../lib/research/CausalGraph.js'
import { scoreHypotheses } from '../../scripts/causal-loop.mjs'
import {
  CONDUCTOR_PROJECT,
  TRAP_BELIEF_MIN,
  TURN_CLUSTER_MIN,
  agendaNodeId,
  agendaNodes,
  benchDeltaSignals,
  dossierTrapSignals,
  driftSignals,
  ingestAgenda,
  mineAgenda,
  proposeAgenda,
  reflexPendingSignals,
  turnFailureSignals,
} from '../../scripts/conductor.mjs'

const NOW = '2026-07-07T12:00:00.000Z'
const NOW_TS = Math.floor(Date.parse(NOW) / 1000)

const turn = (
  seq,
  { stop = 'answer', hash = 'base', failovers = [], ts = NOW_TS - seq, knobs = {} } = {},
) => ({
  kind: 'turn',
  v: 1,
  ts,
  seq,
  cfg: { hash, knobs },
  outcome: {
    ok: stop === 'answer',
    stop,
    latency_ms: 1000,
    hops: 2,
    tokens: {},
    counters: {},
    failovers,
  },
})

test('invalid observation windows cannot admit failure clusters', () => {
  const rows = Array.from({ length: TURN_CLUSTER_MIN }, (_, i) => turn(i, { stop: 'error_stop' }))
  for (const opts of [{ now: 'not-a-date' }, { now: NOW, lookbackDays: NaN }]) {
    assert.deepEqual(turnFailureSignals(rows, opts), [])
  }
})

test('turnFailureSignals clusters stops and routes config-correlated failures to config', () => {
  const rows = [
    turn(1, { stop: 'error_stop', hash: 'bad' }),
    turn(2, { stop: 'error_stop', hash: 'bad' }),
    turn(3, { stop: 'error_stop', hash: 'bad' }),
    turn(4, { hash: 'base' }),
    turn(5, { hash: 'base' }),
    turn(6, { hash: 'base' }),
    turn(7, { hash: 'base' }),
    turn(8, { hash: 'base' }),
    turn(9, { hash: 'base' }),
  ]

  const signals = turnFailureSignals(rows, { now: NOW, lookbackDays: 2 })
  assert.equal(signals.length, 1)
  const [s] = signals
  assert.equal(s.rung, 'config')
  assert.equal(s.slug, 'stop-error-stop')
  assert.equal(s.observed.samples, 3)
  assert.equal(s.observed.cfgHash, 'bad')
  assert.equal(s.severity, 1)
  assert.deepEqual(s.evidenceRefs, ['ledger#1', 'ledger#2', 'ledger#3'])
})

test('turnFailureSignals clusters failover triples and routes non-correlated failures to code', () => {
  const failover = { from: 'longcat', to: 'codex-run', reason: 'raw tool markup' }
  const rows = [
    turn(1, { failovers: [failover], hash: 'base' }),
    turn(2, { failovers: [failover], hash: 'base' }),
    turn(3, { failovers: [failover], hash: 'base' }),
    turn(4, { failovers: [failover], hash: 'base' }),
  ]

  const [s] = turnFailureSignals(rows, { now: NOW, lookbackDays: 2 })
  assert.equal(s.rung, 'code')
  assert.equal(s.slug, 'failover-longcat-codex-run-raw-tool-markup')
  assert.match(s.goal, /Stop longcat failing over to codex-run/)
  assert.equal(s.observed.samples, 4)
})

test('a 2-occurrence turn cluster mints nothing (threshold floor)', () => {
  assert.equal(TURN_CLUSTER_MIN, 3)
  const rows = [turn(1, { stop: 'spin' }), turn(2, { stop: 'spin' }), turn(3, {}), turn(4, {})]
  assert.deepEqual(turnFailureSignals(rows, { now: NOW, lookbackDays: 2 }), [])
})

test('turns outside the lookback window do not count toward a cluster', () => {
  const stale = NOW_TS - 30 * 86_400
  const rows = [
    turn(1, { stop: 'spin' }),
    turn(2, { stop: 'spin' }),
    turn(3, { stop: 'spin', ts: stale }), // third occurrence is stale
    turn(4, {}),
  ]
  assert.deepEqual(turnFailureSignals(rows, { now: NOW, lookbackDays: 2 }), [])
})

test('benchDeltaSignals reports accuracy regressions past the variance guard', () => {
  const reports = [
    {
      file: 'control_code_20260707010000.json',
      control: { suite: 'control-code-v2', timestamp: '20260707010000' },
      metrics: [
        { subject: 'edit-small', accuracy_pct: 91, accuracy_std: 0.2 },
        { subject: 'stable', accuracy_pct: 80, accuracy_std: 0.5 },
      ],
    },
    {
      file: 'control_code_20260707020000.json',
      control: { suite: 'control-code-v2', timestamp: '20260707020000' },
      metrics: [
        { subject: 'edit-small', accuracy_pct: 86, accuracy_std: 0.4 },
        // Drop is 0.5, below the 1.0 floor.
        { subject: 'stable', accuracy_pct: 79.5, accuracy_std: 0.1 },
      ],
    },
  ]

  const signals = benchDeltaSignals(reports)
  assert.equal(signals.length, 1)
  const [s] = signals
  assert.equal(s.rung, 'code')
  assert.equal(s.slug, 'bench-control-code-v2-edit-small')
  assert.equal(s.observed.drop, 5)
  assert.equal(s.observed.guard, 1)
  assert.deepEqual(s.evidenceRefs, [
    'control_code_20260707010000.json',
    'control_code_20260707020000.json',
  ])
})

test('benchDeltaSignals does not interpret missing seed variance as stability', () => {
  const reports = [91, 10].map((accuracy, index) => ({
    file: `unknown-variance-${index}.json`,
    control: { suite: 'fixture', timestamp: `202607070${index}0000` },
    metrics: [{ subject: 'candidate', accuracy_pct: accuracy, accuracy_std: null }],
  }))
  assert.deepEqual(benchDeltaSignals(reports), [])
})

test('a bench delta inside the variance floor mints nothing', () => {
  const pair = (prevAcc, currAcc, std = 0) => [
    {
      file: 'control_x_20260707010000.json',
      control: { suite: 'x', timestamp: '20260707010000' },
      metrics: [{ subject: 's', accuracy_pct: prevAcc, accuracy_std: std }],
    },
    {
      file: 'control_x_20260707020000.json',
      control: { suite: 'x', timestamp: '20260707020000' },
      metrics: [{ subject: 's', accuracy_pct: currAcc, accuracy_std: std }],
    },
  ]
  // Drop 0.8 < the 1.0-pt floor (std 0) → nothing.
  assert.deepEqual(benchDeltaSignals(pair(90, 89.2)), [])
  // Drop 5 ≤ guard 2×3 = 6 on a noisy suite → the std term dominates the floor.
  assert.deepEqual(benchDeltaSignals(pair(90, 85, 3)), [])
  // Improvements never mint.
  assert.deepEqual(benchDeltaSignals(pair(80, 90)), [])
})

test('driftSignals turns Habitsmith DRIFT flags into skills re-verify candidates', () => {
  const signals = driftSignals({
    v: 1,
    driftBelief: 0.55,
    facts: [
      {
        id: 'habit_a',
        name: 'proj-build-test-run',
        belief: 0.32,
        drifting: true,
        skill: { uses: 5 },
      },
      { id: 'habit_b', name: 'quiet', belief: 0.9, drifting: false, skill: { uses: 2 } },
    ],
  })

  assert.equal(signals.length, 1)
  assert.equal(signals[0].rung, 'skills')
  assert.equal(signals[0].slug, 'drift-proj-build-test-run')
  assert.equal(signals[0].severity, 3)
  assert.equal(signals[0].observed.samples, 5)
})

test('reflexPendingSignals parses proposal bullets and refuses secret-named knobs', () => {
  const md = [
    '# Reflex Tier-B proposals',
    '',
    '- `ANGEL_SPIN_LIMIT=8` — p=0.91 · bench Δspin -2',
    '- `ANGEL_OPENAI_API_KEY=leak` — should be ignored',
    '',
  ].join('\n')

  const signals = reflexPendingSignals(md)
  assert.equal(signals.length, 1)
  assert.equal(signals[0].rung, 'config')
  assert.equal(signals[0].slug, 'reflex-angel-spin-limit-8')
  assert.match(signals[0].goal, /ANGEL_SPIN_LIMIT=8/)
})

test('dossierTrapSignals surfaces strongly believed repo-dossier traps as code work', () => {
  const g = new CausalGraph()
  g.addNode({
    id: 'hyp_dossier_repo_trap_npm-test',
    type: NODE_TYPE.HYPOTHESIS,
    projectId: 'repo-dossier',
    factKind: 'trap',
    factText: 'npm test',
    repoKey: 'repo',
    repoRoot: '/tmp/repo',
    observed: { runs: 6, passes: 0, fails: 6, sessions: 3, lastSeen: NOW_TS },
  })

  const signals = dossierTrapSignals(g, { now: NOW })
  assert.equal(signals.length, 1)
  assert.equal(signals[0].rung, 'code')
  assert.ok(signals[0].observed.belief >= TRAP_BELIEF_MIN)
  assert.equal(signals[0].evidenceRefs[0], 'fact:hyp_dossier_repo_trap_npm-test')
})

test('ingestAgenda mints idempotent conductor nodes without signed edges', () => {
  const g = new CausalGraph()
  const candidate = {
    rung: 'code',
    slug: 'bench-regression',
    goal: 'Investigate the bench regression.',
    label: 'Bench regression',
    evidenceRefs: ['report-a.json'],
    estCostMin: 45,
    severity: 2,
    observed: { samples: 4 },
  }

  const first = ingestAgenda(g, [candidate], { now: NOW })
  assert.equal(first.added.nodes, 1)
  assert.equal(first.added.updated, 0)
  assert.equal(g.edgeCount(), 0)

  const id = agendaNodeId('code', 'bench-regression')
  const node = g.getNode(id)
  assert.equal(node.type, NODE_TYPE.HYPOTHESIS)
  assert.equal(node.projectId, CONDUCTOR_PROJECT.id)
  assert.equal(node.factKind, 'agenda')
  assert.equal(node.status, 'testing')
  assert.equal(node.rung, 'code')
  assert.deepEqual(node.evidenceRefs, ['report-a.json'])

  const second = ingestAgenda(g, [{ ...candidate, goal: 'Do not rewrite identity.' }], {
    now: '2026-07-08T00:00:00.000Z',
  })
  assert.equal(second.added.nodes, 0)
  assert.equal(second.added.updated, 1)
  assert.equal(g.nodeCount(), 1)
  assert.equal(g.getNode(id).goal, 'Investigate the bench regression.')
  assert.equal(g.getNode(id).seenCount, 2)
  assert.equal(g.getNode(id).lastSeenAt, '2026-07-08T00:00:00.000Z')
})

test('proposeAgenda reuses EIG then applies samples × severity ÷ cost priority', () => {
  const g = new CausalGraph()
  ingestAgenda(
    g,
    [
      {
        rung: 'code',
        slug: 'high-value',
        goal: 'Fix the high-value issue.',
        evidenceRefs: ['a'],
        estCostMin: 20,
        severity: 2,
        observed: { samples: 7 },
      },
      {
        rung: 'code',
        slug: 'low-value',
        goal: 'Fix the low-value issue.',
        evidenceRefs: ['b'],
        estCostMin: 10,
        severity: 1,
        observed: { samples: 1 },
      },
    ],
    { now: NOW },
  )

  const { proposal, ranking, dryRun } = proposeAgenda(g, { limit: 10 })
  assert.equal(dryRun, true)
  assert.equal(ranking.length, 2)
  assert.equal(proposal.hypothesisId, agendaNodeId('code', 'high-value'))

  const high = ranking.find((r) => r.hypothesisId.endsWith('high-value'))
  const low = ranking.find((r) => r.hypothesisId.endsWith('low-value'))
  // Same rung => same metric and same base EIG score: 2.0. Priority formula:
  // high = 2 * (1 + log2(8)) * 2 / 2 = 8; low = 2 * (1 + log2(2)) * 1 / 1 = 4.
  assert.equal(high.score, 2)
  assert.equal(low.score, 2)
  assert.equal(high.priority, 8)
  assert.equal(low.priority, 4)
})

test('priority: the estCostMin divisor floors at 1, severity multiplies, ids break ties', () => {
  const g = new CausalGraph()
  ingestAgenda(
    g,
    [
      // A: cheap (cost 5) → the divisor must FLOOR at 1, not drop to 0.5.
      {
        rung: 'code',
        slug: 'aaa',
        goal: 'Do A.',
        evidenceRefs: [],
        estCostMin: 5,
        severity: 1,
        observed: { samples: 0 },
      },
      // B: evidence 1+log2(8)=4, sev 3, divisor 4 → exactly 3× A.
      {
        rung: 'code',
        slug: 'bbb',
        goal: 'Do B.',
        evidenceRefs: [],
        estCostMin: 40,
        severity: 3,
        observed: { samples: 7 },
      },
      // C: sev 3, divisor 1, no evidence → also exactly 3× A; ties B → id order.
      {
        rung: 'code',
        slug: 'ccc',
        goal: 'Do C.',
        evidenceRefs: [],
        estCostMin: 10,
        severity: 3,
        observed: { samples: 0 },
      },
    ],
    { now: NOW },
  )

  const scoreOf = new Map(scoreHypotheses(g).map((r) => [r.id, r.score]))
  const { ranking } = proposeAgenda(g)
  const byId = new Map(ranking.map((r) => [r.hypothesisId, r]))
  const a = byId.get('conductor_code_aaa')
  const b = byId.get('conductor_code_bbb')
  const c = byId.get('conductor_code_ccc')

  // Exact formula, score taken verbatim from the engine.
  const expect = (id, samples, sev, cost) =>
    (scoreOf.get(id) * (1 + Math.log2(1 + samples)) * sev) / Math.max(1, cost / 10)
  assert.ok(Math.abs(a.priority - expect(a.hypothesisId, 0, 1, 5)) < 1e-12)
  assert.ok(Math.abs(b.priority - expect(b.hypothesisId, 7, 3, 40)) < 1e-12)
  assert.ok(Math.abs(c.priority - expect(c.hypothesisId, 0, 3, 10)) < 1e-12)

  // Cost floor: at cost 5 the priority equals the raw score (an unfloored
  // divisor of 0.5 would have doubled it).
  assert.equal(a.priority, scoreOf.get(a.hypothesisId))
  // Severity multiplier in isolation: C = 3 × A (same samples, floored cost).
  assert.ok(Math.abs(c.priority - 3 * a.priority) < 1e-12)
  // B and C tie exactly (4×3/4 = 1×3/1) → deterministic id tiebreak, bbb first.
  assert.ok(Math.abs(b.priority - c.priority) < 1e-12)
  assert.deepEqual(
    ranking.map((r) => r.hypothesisId),
    ['conductor_code_bbb', 'conductor_code_ccc', 'conductor_code_aaa'],
  )
})

test('proposeAgenda on a graph without agenda nodes proposes nothing', () => {
  const { proposal, ranking, dryRun } = proposeAgenda(new CausalGraph())
  assert.equal(proposal, null)
  assert.deepEqual(ranking, [])
  assert.equal(dryRun, true)
})

test('full pass: five substrates mine → ingest → rank, foreign domains untouched', () => {
  const g = new CausalGraph()
  // A believed dossier trap (6/6 fresh fails → factBelief ≈ 0.78) + a foreign
  // research hypothesis that must survive the whole pass untouched.
  g.addNode({
    id: 'hyp_dossier_repo_trap_npm-test',
    type: NODE_TYPE.HYPOTHESIS,
    projectId: 'repo-dossier',
    factKind: 'trap',
    factText: 'npm test',
    repoKey: 'repo',
    repoRoot: '/tmp/repo',
    status: 'testing',
    metric: { name: 'dossier-repo', direction: 'higher' },
    observed: { runs: 6, passes: 0, fails: 6, sessions: 3, lastSeen: NOW_TS },
  })
  g.addNode({
    id: 'hyp_research_1',
    type: NODE_TYPE.HYPOTHESIS,
    label: 'research',
    projectId: 'research',
    metric: { name: 'val_bpb', direction: 'lower' },
    status: 'testing',
  })
  const before = g.nodeCount()

  const failover = { from: 'longcat', to: 'codex-run', reason: 'raw tool markup' }
  const candidates = mineAgenda(
    {
      rows: [
        turn(1, { stop: 'spin' }),
        turn(2, { stop: 'spin' }),
        turn(3, { stop: 'spin' }),
        turn(4, { failovers: [failover] }),
        turn(5, { failovers: [failover] }),
        turn(6, { failovers: [failover] }),
        turn(7, {}),
        turn(8, {}),
      ],
      reports: [
        {
          file: 'control_code_20260707010000.json',
          control: { suite: 'control-code-v2', timestamp: '20260707010000' },
          metrics: [{ subject: 'edit-small', accuracy_pct: 91, accuracy_std: 0.2 }],
        },
        {
          file: 'control_code_20260707020000.json',
          control: { suite: 'control-code-v2', timestamp: '20260707020000' },
          metrics: [{ subject: 'edit-small', accuracy_pct: 79, accuracy_std: 0.4 }],
        },
      ],
      habitsStatus: {
        facts: [{ id: 'habit_a', name: 'used', belief: 0.4, drifting: true, skill: { uses: 3 } }],
      },
      proposalsMd: '- `ANGEL_SPIN_LIMIT=8` — p=0.91\n- `ANGEL_MAX_HOPS=64` — p=0.86\n',
      graph: g,
    },
    { now: NOW, lookbackDays: 2 },
  )
  // 2 turn clusters + 1 bench + 1 drift + 2 reflex + 1 trap.
  assert.equal(candidates.length, 7)
  assert.deepEqual(new Set(candidates.map((c) => c.rung)), new Set(['code', 'config', 'skills']))

  const { added } = ingestAgenda(g, candidates, { now: NOW })
  assert.equal(added.nodes, 7)
  assert.equal(g.nodeCount(), before + 7)

  const { proposal, ranking, dryRun } = proposeAgenda(g)
  assert.equal(dryRun, true)
  assert.ok(proposal)
  assert.equal(ranking.length, 7)
  assert.ok(ranking.every((r) => r.hypothesisId.startsWith('conductor_')))
  assert.ok(ranking.every((r, i) => i === 0 || ranking[i - 1].priority >= r.priority))
  assert.equal(proposeAgenda(g, { limit: 3 }).ranking.length, 3)

  // Foreign + dossier nodes untouched; the whole pass added no signed edges.
  assert.equal(g.getNode('hyp_research_1').status, 'testing')
  assert.equal(g.getNode('hyp_research_1').projectId, 'research')
  assert.equal(g.getNode('hyp_dossier_repo_trap_npm-test').projectId, 'repo-dossier')
  assert.equal(g.edgeCount(), 0)
})

test('mineAgenda concatenates every substrate and agendaNodes filters conductor scope', () => {
  const g = new CausalGraph()
  g.addNode({ id: 'foreign', type: NODE_TYPE.HYPOTHESIS, projectId: 'research', status: 'testing' })

  const candidates = mineAgenda(
    {
      rows: [
        turn(1, { stop: 'max_hops', hash: 'base' }),
        turn(2, { stop: 'max_hops', hash: 'base' }),
        turn(3, { stop: 'max_hops', hash: 'base' }),
      ],
      habitsStatus: {
        facts: [{ id: 'habit_a', name: 'used', belief: 0.4, drifting: true, skill: { uses: 1 } }],
      },
      proposalsMd: '- `ANGEL_SPIN_LIMIT=12` — p=0.88\n',
      graph: g,
    },
    { now: NOW, lookbackDays: 2 },
  )

  assert.equal(candidates.length, 3)
  ingestAgenda(g, candidates, { now: NOW })
  assert.equal(agendaNodes(g).length, 3)
  assert.equal(agendaNodes(g, 'config').length, 1)
})
