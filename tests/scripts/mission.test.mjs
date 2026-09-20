// Tests for mission: the persisted completion objective (pure core + CLI shell
// against a temp ANGEL_MISSION_DIR).

import { test } from 'node:test'
import assert from 'node:assert/strict'
import { execFileSync } from 'node:child_process'
import { mkdtempSync, readFileSync, readdirSync, statSync, rmSync, writeFileSync } from 'node:fs'
import { tmpdir } from 'node:os'
import { join } from 'node:path'

import {
  BLOCKED_MIN_ROUNDS,
  DEFAULT_MAX_ROUNDS,
  MAX_ROUNDS_HARD,
  canContinueMission,
  clampMaxRounds,
  createMission,
  deriveMissionId,
  editMission,
  GOAL_STATUS_TO_MISSION,
  MISSION_STATUS_TO_GOAL,
  goalToMission,
  missionAllowsDispatch,
  missionGateReason,
  missionSummary,
  missionToGoal,
  parseMission,
  persistMission,
  pauseMission,
  replayMissions,
  resumeMission,
  serializeMission,
  tickMission,
  validateBlockedReason,
  validateMissionId,
  validateObjective,
} from '../../scripts/mission.mjs'

const NOW = '2026-07-07T12:00:00.000Z'

// ─── validation ──────────────────────────────────────────────────────────────

test('objective validation rejects empty, oversized, and control-char text', () => {
  assert.equal(validateObjective(''), 'mission objective must be non-empty')
  assert.equal(validateObjective('   '), 'mission objective must be non-empty')
  assert.match(validateObjective('x'.repeat(4001)), /exceeds/)
  assert.equal(validateObjective('bad\u0000byte'), 'mission objective has control characters')
  assert.equal(validateObjective('a real objective'), null)
})

test('id validation enforces the slug charset', () => {
  assert.equal(validateMissionId('good-id-1'), null)
  assert.equal(
    validateMissionId('-leading-dash'),
    'mission id must match [0-9a-z-], be 1-64 chars, and start with [0-9a-z]',
  )
  assert.equal(
    validateMissionId('UPPER'),
    'mission id must match [0-9a-z-], be 1-64 chars, and start with [0-9a-z]',
  )
  assert.equal(
    validateMissionId(''),
    'mission id must match [0-9a-z-], be 1-64 chars, and start with [0-9a-z]',
  )
})

test('blocked reason and round cap validation', () => {
  assert.equal(validateBlockedReason(''), 'blocked reason must be non-empty')
  assert.equal(validateBlockedReason('no gpu'), null)
  assert.equal(clampMaxRounds(5), 5)
  assert.equal(clampMaxRounds(0), 1)
  assert.equal(clampMaxRounds(10 ** 9), MAX_ROUNDS_HARD)
  assert.equal(clampMaxRounds('nope'), DEFAULT_MAX_ROUNDS)
})

// ─── creation and ids ────────────────────────────────────────────────────────

test('createMission mints an active revision-1 mission', () => {
  const m = createMission({ objective: 'win the benchmark', maxRounds: 7, now: NOW })
  assert.equal(m.schema, 'angel.mission/v1')
  assert.equal(m.status, 'active')
  assert.equal(m.revision, 1)
  assert.equal(m.roundsStarted, 0)
  assert.equal(m.maxRounds, 7)
  assert.equal(m.objective, 'win the benchmark')
  assert.equal(m.createdAt, NOW)
  assert.equal(m.updatedAt, NOW)
  assert.equal(m.blockedReason, null)
  assert.equal(m.blockedStreak, 0)
})

test('derived ids are slug-safe, bounded, and time-distinct', () => {
  const a = deriveMissionId('Solve THE Benchmark!!', NOW)
  const b = deriveMissionId('Solve THE Benchmark!!', '2026-07-08T12:00:00.000Z')
  assert.match(a, /^[0-9a-z][0-9a-z-]*$/)
  assert.ok(a.length <= 64)
  assert.notEqual(a, b)
})

// ─── continuation rounds ─────────────────────────────────────────────────────

test('tickMission advances rounds and revision', () => {
  const m = createMission({ objective: 'x', now: NOW })
  const t = tickMission(m, { now: '2026-07-07T13:00:00.000Z' })
  assert.equal(t.roundsStarted, 1)
  assert.equal(t.revision, 2)
  assert.equal(t.updatedAt, '2026-07-07T13:00:00.000Z')
})

test('completion is terminal and absorbs later ticks idempotently', () => {
  let m = createMission({ objective: 'x', now: NOW })
  m = tickMission(m, { completed: true })
  assert.equal(m.status, 'complete')
  const again = tickMission(m, {})
  assert.deepEqual(again, m, 'terminal states never bump revision')
  assert.equal(canContinueMission(m), false)
})

test('plain ticks clear a blocked candidate streak', () => {
  let m = createMission({ objective: 'x', now: NOW })
  m = tickMission(m, { blockedReason: 'no gpu' })
  m = tickMission(m, { blockedReason: 'no gpu' })
  assert.equal(m.blockedStreak, 2)
  m = tickMission(m, {})
  assert.equal(m.blockedStreak, 0)
  assert.equal(m.blockedReason, null)
  assert.equal(m.status, 'active')
})

test('blocked requires the SAME reason for BLOCKED_MIN_ROUNDS consecutive rounds', () => {
  let m = createMission({ objective: 'x', now: NOW })
  m = tickMission(m, { blockedReason: 'no gpu' })
  m = tickMission(m, { blockedReason: 'no gpu' })
  assert.equal(m.status, 'active', 'two rounds is not blocked yet')
  // A different reason resets the streak — the condition did not persist.
  m = tickMission(m, { blockedReason: 'network down' })
  assert.equal(m.blockedStreak, 1)
  assert.equal(m.status, 'active')
  // Three consecutive identical reasons now flip it terminal.
  m = tickMission(m, { blockedReason: 'no gpu' })
  m = tickMission(m, { blockedReason: 'no gpu' })
  m = tickMission(m, { blockedReason: 'no gpu' })
  assert.equal(m.blockedStreak, BLOCKED_MIN_ROUNDS)
  assert.equal(m.status, 'blocked')
  assert.equal(canContinueMission(m), false)
})

test('round cap bounds autonomous continuation', () => {
  let m = createMission({ objective: 'x', maxRounds: 2, now: NOW })
  assert.equal(canContinueMission(m), true)
  m = tickMission(m, {})
  m = tickMission(m, {})
  assert.equal(m.roundsStarted, 2)
  assert.equal(canContinueMission(m), false)
})

// ─── pause / resume / edit ───────────────────────────────────────────────────

test('pause stops continuation; resume re-arms only from paused', () => {
  let m = createMission({ objective: 'x', now: NOW })
  m = pauseMission(m)
  assert.equal(m.status, 'paused')
  assert.equal(canContinueMission(m), false)
  m = resumeMission(m)
  assert.equal(m.status, 'active')
  assert.equal(canContinueMission(m), true)
  // Terminal missions are not resumable — mint a new one.
  const done = tickMission(createMission({ objective: 'y' }), { completed: true })
  assert.deepEqual(resumeMission(done), done)
})

test('edit replaces objective and cap, clears blocking state, and is inert on terminals', () => {
  let m = createMission({ objective: 'old', maxRounds: 10, now: NOW })
  m = tickMission(m, { blockedReason: 'no gpu' })
  m = editMission(m, { objective: 'new objective', maxRounds: 25 })
  assert.equal(m.objective, 'new objective')
  assert.equal(m.maxRounds, 25)
  assert.equal(m.blockedReason, null)
  assert.equal(m.blockedStreak, 0)
  const done = tickMission(createMission({ objective: 'z' }), { completed: true })
  assert.deepEqual(editMission(done, { objective: 'nope' }), done)
})

test('summary is a one-line terminal row with no secrets', () => {
  const m = createMission({ objective: 'solve the action-agent benchmark', now: NOW })
  const row = missionSummary(m)
  assert.match(row, /^\[active\] r0\/100 solve the action-agent benchmark$/)
  assert.equal(row.includes('"'), false)
})

// ─── conductor gate helpers ──────────────────────────────────────────────────

test('missionAllowsDispatch: only an active mission inside its budget may be driven', () => {
  assert.equal(missionAllowsDispatch(null), true, 'no mission = legacy behavior')
  const active = createMission({ objective: 'x', maxRounds: 2, now: NOW })
  assert.equal(missionAllowsDispatch(active), true)
  const spent = tickMission(tickMission(active, {}), {})
  assert.equal(missionAllowsDispatch(spent), false, 'a spent budget gates')
  assert.equal(missionAllowsDispatch(pauseMission(active)), false, 'paused gates')
  let blocked = active
  for (let i = 0; i < BLOCKED_MIN_ROUNDS; i += 1) {
    blocked = tickMission(blocked, { blockedReason: 'gpu down' })
  }
  assert.equal(missionAllowsDispatch(blocked), false, 'blocked gates')
  assert.equal(
    missionAllowsDispatch(tickMission(active, { completed: true })),
    false,
    'complete gates',
  )
})

test('missionGateReason names the concrete refusal for every non-continuable state', () => {
  const active = createMission({ objective: 'x', maxRounds: 2, now: NOW })
  const spent = tickMission(tickMission(active, {}), {})
  assert.match(missionGateReason(spent), /round budget reached \(2\/2\)/)
  let blocked = active
  for (let i = 0; i < BLOCKED_MIN_ROUNDS; i += 1) {
    blocked = tickMission(blocked, { blockedReason: 'gpu down' })
  }
  assert.match(missionGateReason(blocked), /blocked \(gpu down\)/)
  assert.match(missionGateReason(pauseMission(active)), /paused/)
  assert.match(
    missionGateReason(tickMission(active, { completed: true })),
    /complete — mint a new one/,
  )
})

test('persistMission writes the canonical ledger + atomic pointer and reloads', async () => {
  const dir = mkdtempSync(join(tmpdir(), 'mission-persist-'))
  try {
    const m = createMission({ objective: 'persisted objective', now: NOW })
    await persistMission(m, dir)
    const reloaded = parseMission(readFileSync(join(dir, `${m.id}.json`), 'utf8'))
    assert.deepEqual(reloaded, m)
    const ledger = readFileSync(join(dir, `${m.id}.jsonl`), 'utf8')
    assert.equal(ledger.trim().split('\n').length, 1)
    // A second mutation appends, and the pointer follows.
    const t = tickMission(m, {})
    await persistMission(t, dir)
    assert.equal(
      readFileSync(join(dir, `${m.id}.jsonl`), 'utf8')
        .trim()
        .split('\n').length,
      2,
    )
    assert.equal(parseMission(readFileSync(join(dir, `${m.id}.json`), 'utf8')).roundsStarted, 1)
  } finally {
    rmSync(dir, { recursive: true, force: true })
  }
})

// ─── serialization ───────────────────────────────────────────────────────────

test('serialize/parse round-trips; garbage is rejected', () => {
  const m = createMission({ objective: 'x', now: NOW })
  const back = parseMission(serializeMission(m))
  assert.deepEqual(back, m)
  assert.equal(parseMission('not json'), null)
  assert.equal(parseMission(JSON.stringify({ schema: 'other/v1' })), null)
  assert.equal(parseMission(JSON.stringify({ ...m, status: 'exploded' })), null)
  const missing = { ...m }
  delete missing.maxRounds
  assert.equal(parseMission(JSON.stringify(missing)), null)
})

test('the workspace binding fields are validated, not silently dropped', () => {
  // Step-3 field ownership: workspace/projectKey are optional string fields
  // on the mission record. Absent stays valid; a malformed type fails closed
  // so the bridge can never round-trip a corrupted binding.
  const base = createMission({ objective: 'x', now: NOW })
  assert.ok(parseMission(JSON.stringify(base)), 'no binding is valid')
  assert.ok(
    parseMission(JSON.stringify({ ...base, workspace: '/ws', projectKey: 'key' })),
    'string bindings are valid',
  )
  assert.equal(
    parseMission(JSON.stringify({ ...base, workspace: 42 })),
    null,
    'a non-string workspace fails closed',
  )
  assert.equal(
    parseMission(JSON.stringify({ ...base, projectKey: { nope: true } })),
    null,
    'a non-string projectKey fails closed',
  )
  // And a valid binding still round-trips through the bridge unchanged.
  const mission = goalToMission({ text: 'x', workspace: '/ws', project_key: 'key' }, { now: NOW })
  const back = missionToGoal(parseMission(JSON.stringify(mission)))
  assert.equal(back.workspace, '/ws')
  assert.equal(back.project_key, 'key')
})

test('replayMissions folds an ordered ledger to the last valid row', () => {
  const m = createMission({ objective: 'x', now: NOW })
  const t = tickMission(m, { now: '2026-07-07T14:00:00.000Z' })
  const rows = [serializeMission(m), 'corrupted row', serializeMission(t)]
  const current = replayMissions(rows)
  assert.equal(current.roundsStarted, 1)
})

// ─── CLI shell (temp ANGEL_MISSION_DIR) ──────────────────────────────────────

const cliEnv = () => ({
  ...process.env,
  ANGEL_MISSION_DIR: mkdtempSync(join(tmpdir(), 'mission-test-')),
})

const run = (env, args) =>
  execFileSync(process.execPath, ['scripts/mission.mjs', ...args], { env, encoding: 'utf8' })

test('cli create → tick → blocked after three rounds persists across invocations', () => {
  const env = cliEnv()
  try {
    const created = run(env, ['create', '--objective', 'win the benchmark', '--max-rounds', '5'])
    assert.match(created.trim(), /^\[active\] r0\/5 win the benchmark$/)
    const id = deriveMissionIdFor(env)
    assert.ok(id, 'a mission pointer file must exist')

    run(env, ['tick', id, '--blocked', 'no gpu'])
    run(env, ['tick', id, '--blocked', 'no gpu'])
    const stillActive = run(env, ['show', id])
    assert.match(stillActive, /"status":"active"/)

    const blocked = run(env, ['tick', id, '--blocked', 'no gpu'])
    assert.match(blocked, /BLOCKED after 3 consecutive rounds/)
    const shown = run(env, ['show', id])
    assert.match(shown, /"status":"blocked"/)

    // The ledger audit has one row per mutation (create + 3 ticks).
    const ledger = readFileSync(join(env.ANGEL_MISSION_DIR, `${id}.jsonl`), 'utf8')
    assert.equal(ledger.trim().split('\n').length, 4)

    // Owner-only pointer (0600 on Unix).
    const mode = statSync(join(env.ANGEL_MISSION_DIR, `${id}.json`)).mode & 0o777
    assert.equal(mode, 0o600)

    // A blocked mission re-arms with rounds intact (complete stays terminal).
    const resumed = run(env, ['resume', id])
    assert.match(resumed.trim(), /^\[active\]/)
    const rearmed = JSON.parse(run(env, ['show', id]))
    assert.equal(rearmed.roundsStarted, 3, 'rounds survive re-arm')
  } finally {
    rmSync(env.ANGEL_MISSION_DIR, { recursive: true, force: true })
  }
})

test('cli enforces the round cap and supports pause/resume/edit/list', () => {
  const env = cliEnv()
  try {
    run(env, ['create', '--objective', 'capped mission', '--max-rounds', '1', '--id', 'cap-test'])
    run(env, ['tick', 'cap-test'])
    assert.throws(() => run(env, ['tick', 'cap-test']), /round cap reached/)

    run(env, ['create', '--objective', 'pause me', '--id', 'pause-test'])
    const paused = run(env, ['pause', 'pause-test'])
    assert.match(paused.trim(), /^\[paused\]/)
    const resumed = run(env, ['resume', 'pause-test'])
    assert.match(resumed.trim(), /^\[active\]/)

    const edited = run(env, [
      'edit',
      'pause-test',
      '--objective',
      'edited objective',
      '--max-rounds',
      '9',
    ])
    assert.match(edited, /edited objective/)
    const shown = JSON.parse(run(env, ['show', 'pause-test']))
    assert.equal(shown.maxRounds, 9)
    assert.equal(shown.objective, 'edited objective')

    const list = run(env, ['list'])
    assert.match(list, /edited objective/)
    assert.match(list, /cap-test/)
  } finally {
    rmSync(env.ANGEL_MISSION_DIR, { recursive: true, force: true })
  }
})

test('cli refuses malformed input with an error and no files', () => {
  const env = cliEnv()
  try {
    assert.throws(() => run(env, ['create', '--objective', '']), /non-empty/)
    assert.throws(() => run(env, ['tick', 'does-not-exist']), /no mission/)
    assert.throws(() => run(env, ['create', '--objective', 'x', '--id', 'Bad Id']), /must match/)
    assert.throws(() => run(env, ['tick']), /usage/)
    assert.deepEqual(readdirSync(env.ANGEL_MISSION_DIR), [])
  } finally {
    rmSync(env.ANGEL_MISSION_DIR, { recursive: true, force: true })
  }
})

// The CLI's derived id is not exported for shell use; recover it from the dir.
const deriveMissionIdFor = (env) => {
  try {
    return readdirSync(env.ANGEL_MISSION_DIR)
      .find((n) => n.endsWith('.json'))
      ?.slice(0, -5)
  } catch {
    return undefined
  }
}

// ─── goal↔mission conformance (docs/plans/goal-mission-conformance.md) ───────

test('the Rust goal vocabulary is mechanically pinned to the bridge', () => {
  // A cross-language drift pin: the cockpit's GoalStatus serde names are
  // parsed straight out of goal.rs, so renaming a Rust status without
  // updating the bridge (or this table) fails the Node suite too — the
  // conformance contract's "update both test suites" rule, enforced.
  const url = new URL('../../cockpit/src/goal.rs', import.meta.url)
  const src = readFileSync(url, 'utf8')
  const block = src.match(/pub enum GoalStatus \{([\s\S]*?)\n\}/)
  assert.ok(block, 'GoalStatus enum not found in goal.rs')
  const body = block[1]
  const snake = (name) => name.replace(/[A-Z]/g, (c, i) => (i > 0 ? '_' : '') + c.toLowerCase())

  const renames = new Map()
  for (const m of body.matchAll(
    /#\[serde\(rename = "([a-z_]+)"(?:, alias = "([a-z_]+)")?\)\]\s*\n\s*([A-Za-z_]+),/g,
  )) {
    renames.set(m[3], { name: m[1], alias: m[2] ?? null })
  }
  const variants = [...body.matchAll(/^\s*([A-Z][A-Za-z_]*),$/gm)].map((m) => m[1])
  assert.deepEqual(variants.sort(), ['Active', 'Blocked', 'Done', 'Paused'].sort())

  const serdeNames = variants.map((v) => renames.get(v)?.name ?? snake(v)).sort()
  assert.deepEqual(
    serdeNames,
    ['active', 'blocked', 'done', 'paused'],
    'GoalStatus serde vocabulary drifted from the contract table',
  )
  // The deprecated Abandoned spelling stays readable (read alias); the
  // variant itself is retired, so nothing can ever write it again.
  assert.equal(renames.get('Paused')?.alias, 'abandoned')
  assert.ok(!variants.includes('Abandoned'), 'the Abandoned variant is retired')

  // Both bridge directions speak exactly these vocabularies.
  assert.deepEqual(Object.keys(GOAL_STATUS_TO_MISSION).sort(), [
    'active',
    'blocked',
    'done',
    'paused',
  ])
  assert.deepEqual(Object.keys(MISSION_STATUS_TO_GOAL).sort(), [
    'active',
    'blocked',
    'complete',
    'paused',
  ])
  assert.deepEqual(Object.values(GOAL_STATUS_TO_MISSION).sort(), [
    'active',
    'blocked',
    'complete',
    'paused',
  ])
})

test('mission status vocabulary matches the conformance contract', () => {
  // The four statuses are the mission side of the mapping table; renaming or
  // adding one without updating the contract doc AND the Rust side is a
  // drift bug this test makes loud.
  const statuses = new Set()
  for (const m of [
    createMission({ objective: 'x', now: NOW }),
    pauseMission(createMission({ objective: 'x', now: NOW })),
    tickMission(createMission({ objective: 'x', now: NOW }), { completed: true }),
  ]) {
    statuses.add(m.status)
  }
  let blocked = createMission({ objective: 'x', now: NOW })
  for (let i = 0; i < BLOCKED_MIN_ROUNDS; i += 1) {
    blocked = tickMission(blocked, { blockedReason: 'gpu down' })
  }
  statuses.add(blocked.status)
  assert.deepEqual([...statuses].sort(), ['active', 'blocked', 'complete', 'paused'])
})

test('mission round and blocker semantics match the shared rules', () => {
  // Rule 1: BLOCKED is earned — same reason, 3 consecutive rounds.
  const earned = createMission({ objective: 'x', maxRounds: 10, now: NOW })
  let m = tickMission(earned, { blockedReason: 'gpu down' })
  m = tickMission(m, { blockedReason: 'network down' }) // resets the streak
  assert.equal(m.blockedStreak, 1)
  m = tickMission(m, { blockedReason: 'gpu down' })
  m = tickMission(m, { blockedReason: 'gpu down' })
  m = tickMission(m, { blockedReason: 'gpu down' })
  assert.equal(m.status, 'blocked')
  // Rule 2: resume keeps rounds; complete is terminal-by-completion.
  const resumed = resumeMission(m)
  assert.equal(resumed.status, 'active')
  assert.equal(resumed.roundsStarted, m.roundsStarted)
  const done = tickMission(createMission({ objective: 'y', now: NOW }), { completed: true })
  assert.deepEqual(resumeMission(done), done)
  // Rule 3: a real round clears blocking candidates.
  let pending = tickMission(createMission({ objective: 'z', now: NOW }), {
    blockedReason: 'gpu down',
  })
  pending = tickMission(pending, {})
  assert.equal(pending.blockedStreak, 0)
})

// ─── cockpit goal ↔ mission bridge (migration step 2) ────────────────────────

test('goalToMission maps every status row of the conformance table', () => {
  for (const [goalStatus, missionStatus] of [
    ['active', 'active'],
    ['done', 'complete'],
    ['paused', 'paused'],
    ['blocked', 'blocked'],
  ]) {
    const m = goalToMission(
      { text: 'x', status: goalStatus, rounds: 2, blocked_reason: 'gpu down', blocked_streak: 1 },
      { now: NOW },
    )
    assert.equal(m.status, missionStatus)
    assert.equal(m.schema, 'angel.mission/v1')
    assert.equal(m.revision, 1)
    assert.equal(m.roundsStarted, 2)
    assert.equal(m.blockedReason, 'gpu down')
    assert.equal(m.blockedStreak, 1)
  }
})

test('goal↔mission round trip preserves the shared vocabulary and binding', () => {
  const goal = {
    workspace: '/home/user/project',
    project_key: 'github.com/newjordan/angel0',
    text: 'ship the cockpit',
    acceptance: ['tests green'],
    accept_cmd: 'cargo test',
    notes: ['round one landed'],
    status: 'blocked',
    rounds: 5,
    max_rounds: 9,
    blocked_reason: 'gpu down',
    blocked_streak: 3,
    created_ms: Date.parse(NOW),
    updated_ms: Date.parse(NOW) + 1000,
  }
  const mission = goalToMission(goal, { id: 'bridge-test', now: NOW })
  assert.equal(mission.maxRounds, 9)
  assert.equal(mission.createdAt, NOW)
  const back = missionToGoal(mission)
  assert.equal(back.text, 'ship the cockpit')
  assert.equal(back.status, 'blocked')
  assert.equal(back.rounds, 5)
  assert.equal(back.max_rounds, 9)
  assert.equal(back.blocked_reason, 'gpu down')
  assert.equal(back.blocked_streak, 3)
  assert.equal(back.workspace, '/home/user/project')
  assert.equal(back.project_key, 'github.com/newjordan/angel0')
  // Cockpit-only fields come back defaulted — the mission schema cannot carry them.
  assert.deepEqual(back.acceptance, [])
  assert.equal(back.accept_cmd, null)
  assert.deepEqual(back.notes, [])
})

test('unbounded goal rounds become the mission default; done↔complete round trips', () => {
  const unbounded = goalToMission({ text: 'x', status: 'active' }, { now: NOW })
  assert.equal(unbounded.maxRounds, DEFAULT_MAX_ROUNDS)
  const done = goalToMission({ text: 'x', status: 'done' }, { now: NOW })
  assert.equal(done.status, 'complete')
  const back = missionToGoal(done)
  assert.equal(back.status, 'done')
  assert.equal(back.max_rounds, DEFAULT_MAX_ROUNDS)
})

test('the bridge fails closed on unknown statuses and schema-invalid objectives', () => {
  assert.throws(() => goalToMission({ text: 'x', status: 'exploded' }), /unknown goal status/)
  assert.throws(() => goalToMission({ text: 'x'.repeat(4001) }), /exceeds/)
  assert.throws(() => goalToMission({}), /non-empty/)
  assert.throws(
    () => missionToGoal({ status: 'exploded', objective: 'x' }),
    /unknown mission status/,
  )
  assert.throws(() => missionToGoal({ status: 'active' }), /non-empty/)
})

test('cli import-goal and export-goal move a bound goal through the ledger', () => {
  const env = cliEnv()
  try {
    const goalFile = join(env.ANGEL_MISSION_DIR, 'bound-goal.json')
    writeFileSync(
      goalFile,
      JSON.stringify({
        workspace: '/home/user/project',
        project_key: 'github.com/newjordan/angel0',
        text: 'bridge the stores',
        status: 'paused',
        rounds: 3,
        max_rounds: 12,
      }),
    )
    const imported = run(env, ['import-goal', goalFile, '--id', 'bridge-cli'])
    assert.match(imported.trim(), /^\[paused\] r3\/12 bridge the stores$/)
    const shown = JSON.parse(run(env, ['show', 'bridge-cli']))
    assert.equal(shown.status, 'paused')
    assert.equal(shown.workspace, '/home/user/project')
    assert.equal(shown.projectKey, 'github.com/newjordan/angel0')

    const outFile = join(env.ANGEL_MISSION_DIR, 'exported-goal.json')
    const exported = run(env, ['export-goal', 'bridge-cli', outFile])
    assert.match(exported, /goal written/)
    const back = JSON.parse(readFileSync(outFile, 'utf8'))
    assert.equal(back.text, 'bridge the stores')
    assert.equal(back.status, 'paused')
    assert.equal(back.rounds, 3)
    assert.equal(back.max_rounds, 12)
    assert.equal(back.workspace, '/home/user/project')
    assert.equal(back.project_key, 'github.com/newjordan/angel0')

    assert.throws(() => run(env, ['export-goal', 'missing-id', outFile]), /no mission/)
    assert.throws(
      () => run(env, ['import-goal', join(env.ANGEL_MISSION_DIR, 'nope.json')]),
      /cannot read/,
    )
  } finally {
    rmSync(env.ANGEL_MISSION_DIR, { recursive: true, force: true })
  }
})
