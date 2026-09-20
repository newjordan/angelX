// Tests for reflex-tick's pure decision logic: the layered gates, daily-budget
// accounting, freshness check, and heartbeat/bench shapes. The tick's process/
// spawn side effects are exercised by the offline acceptance, not here.

import { test } from 'node:test'
import assert from 'node:assert/strict'

import {
  killed,
  isIdle,
  rollBudget,
  budgetAllows,
  recordRun,
  binaryStale,
  decideTick,
  makeHeartbeat,
  benchRecord,
} from '../../scripts/runtime/reflex-tick.mjs'

const MIN = 60_000

test('kill switch: only ANGEL_REFLEX=0 disables', () => {
  assert.equal(killed({ ANGEL_REFLEX: '0' }), true)
  assert.equal(killed({ ANGEL_REFLEX: '1' }), false)
  assert.equal(killed({}), false)
})

test('idle requires no live angel TTY and a quiet ledger', () => {
  const now = 1_000 * MIN
  const idleMs = 15 * MIN
  // busy: a live session
  assert.equal(isIdle({ angelTty: true, newestLedgerMs: now - 60 * MIN, now, idleMs }), false)
  // busy: recent ledger activity
  assert.equal(isIdle({ angelTty: false, newestLedgerMs: now - 5 * MIN, now, idleMs }), false)
  // idle: quiet long enough
  assert.equal(isIdle({ angelTty: false, newestLedgerMs: now - 20 * MIN, now, idleMs }), true)
  // idle: no ledger at all
  assert.equal(isIdle({ angelTty: false, newestLedgerMs: null, now, idleMs }), true)
})

test('budget rolls per day and caps runs', () => {
  const today = '2026-07-04'
  // fresh day resets the counter
  assert.deepEqual(rollBudget({ date: '2026-07-03', runs: 4 }, today, 4), {
    date: today,
    runs: 0,
    cap: 4,
  })
  // same day keeps it
  const b = rollBudget({ date: today, runs: 3 }, today, 4)
  assert.equal(b.runs, 3)
  assert.equal(budgetAllows(b), true)
  assert.equal(budgetAllows(recordRun(b)), false) // 4/4 → exhausted
  // missing/garbage budget → 0
  assert.equal(rollBudget(null, today, 4).runs, 0)
})

test('binaryStale: missing or older-than-last-commit is stale', () => {
  assert.equal(binaryStale(null, 1000), true) // missing
  assert.equal(binaryStale(500, 1000), true) // older than commit
  assert.equal(binaryStale(1500, 1000), false) // newer → fresh
  assert.equal(binaryStale(500, NaN), false) // unknown commit time → assume fresh
})

test('decideTick applies the gates in order with a clear reason', () => {
  assert.deepEqual(decideTick({ isKilled: true, idle: true, budgetOk: true }).run, false)
  assert.match(decideTick({ isKilled: true, idle: true, budgetOk: true }).reason, /disabled/)
  assert.match(decideTick({ isKilled: false, idle: false, budgetOk: true }).reason, /busy/)
  assert.match(decideTick({ isKilled: false, idle: true, budgetOk: false }).reason, /budget/)
  assert.equal(decideTick({ isKilled: false, idle: true, budgetOk: true }).run, true)
})

test('heartbeat + bench records carry the watchdog/mining fields', () => {
  const hb = makeHeartbeat({
    now: '2026-07-04T00:00:00.000Z',
    action: 'ran',
    reason: 'experiment complete',
    budget: { date: '2026-07-04', runs: 1, cap: 4 },
    experiment: { knob: 'ANGEL_SOTA_MOA_AGG_CLUB', value: 'codex-run' },
  })
  assert.equal(hb.action, 'ran')
  assert.equal(hb.budget.runs, 1)
  assert.equal(hb.experiment.value, 'codex-run')

  const rec = benchRecord({
    now: 1_700_000_000,
    suite: 'control-code-v2',
    knob: 'ANGEL_SOTA_MOA_AGG_CLUB',
    value: 'codex-run',
    verification: { verdict: 'supports', confidence: 0.8 },
    ab: { baseline: { accuracy_pct: 40 }, treatment: { accuracy_pct: 75 } },
  })
  assert.equal(rec.kind, 'bench')
  assert.equal(rec.verdict, 'supports')
  assert.equal(rec.delta, 35)
})
