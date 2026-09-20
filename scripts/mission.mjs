// mission — the operator's persisted completion objective.
//
// The Conductor mines WHAT to improve from telemetry; the Mission ledger holds
// the operator's explicit long-running objective across cockpit restarts and
// bounds its autonomous rounds. This is the harness's own goal system — one
// active objective, an append-only audit ledger, and fail-safe status rules:
//
//   • a mission is ACTIVE, PAUSED, COMPLETE, or BLOCKED — persisted, not
//     in-process, so a restart (or a fork) resumes with the same state;
//   • continuation is bounded by maxRounds — an unbounded autonomous loop is
//     never implied by an objective, only by a round cap the operator set;
//   • BLOCKED is earned, not claimed: the same blocking reason must be ticked
//     for BLOCKED_MIN_ROUNDS consecutive rounds before the status flips, so a
//     transient failure can never stop a long run;
//   • every mutation appends a schema-versioned row to <id>.jsonl (audit) and
//     atomically replaces <id>.json (the current-state pointer);
//   • revision increments on every mutation so downstream readers can detect
//     a stale read without guessing.
//
// Pure core (no file I/O, no network, no Math.random) + a CLI shell at the
// bottom, exactly the conductor.mjs layout. Secrets discipline: only
// mission-state files are touched; .angel.env is never read.

export const MISSION_SCHEMA = 'angel.mission/v1'
export const BLOCKED_MIN_ROUNDS = 3
export const DEFAULT_MAX_ROUNDS = 100
export const MAX_ROUNDS_HARD = 10_000
export const MAX_OBJECTIVE_LEN = 4000
export const MAX_BLOCKED_REASON_LEN = 400
export const MAX_ID_LEN = 64

const ID_RE = /^[0-9a-z][0-9a-z-]*$/

const slug = (s) =>
  String(s ?? '')
    .toLowerCase()
    .replace(/[^0-9a-z]+/g, '-')
    .replace(/^-+|-+$/g, '')
    .slice(0, 48) || 'x'

const normalizeNow = (now) => {
  const t = now == null ? new Date() : new Date(now)
  return Number.isNaN(t.getTime()) ? new Date(0).toISOString() : t.toISOString()
}

const clone = (m) => ({ ...m })

// ─── validation (pure, CLI shells call these before mutation) ───────────────

/** @returns {string|null} an error description, or null when the id is safe. */
export function validateMissionId(id) {
  const s = String(id ?? '')
  if (s.length === 0 || s.length > MAX_ID_LEN || !ID_RE.test(s)) {
    return `mission id must match [0-9a-z-], be 1-${MAX_ID_LEN} chars, and start with [0-9a-z]`
  }
  return null
}

/** @returns {string|null} an error description, or null when the objective is safe. */
export function validateObjective(text) {
  const s = String(text ?? '').trim()
  if (s.length === 0) return 'mission objective must be non-empty'
  if (s.length > MAX_OBJECTIVE_LEN) return `mission objective exceeds ${MAX_OBJECTIVE_LEN} chars`
  // eslint-disable-next-line no-control-regex -- explicit C0 range is clearer than a loop
  if (/[\u0000-\u0008\u000b\u000c\u000e-\u001f]/.test(s))
    return 'mission objective has control characters'
  return null
}

/** @returns {string|null} an error description, or null when the reason is safe. */
export function validateBlockedReason(text) {
  const s = String(text ?? '').trim()
  if (s.length === 0) return 'blocked reason must be non-empty'
  if (s.length > MAX_BLOCKED_REASON_LEN)
    return `blocked reason exceeds ${MAX_BLOCKED_REASON_LEN} chars`
  return null
}

/** Clamp a round cap into [1, MAX_ROUNDS_HARD]; non-numeric input → default. */
export function clampMaxRounds(n) {
  const v = Number(n)
  if (!Number.isFinite(v)) return DEFAULT_MAX_ROUNDS
  return Math.max(1, Math.min(MAX_ROUNDS_HARD, Math.floor(v)))
}

// ─── core state machine (pure) ──────────────────────────────────────────────

/** Stable id from the objective + creation time (operator ids are optional). */
export function deriveMissionId(objective, now) {
  const ts = normalizeNow(now)
  const millis = Date.parse(ts) || 0
  const stem = slug(objective).slice(0, 32)
  return `${stem}-${millis.toString(36)}`.slice(0, MAX_ID_LEN)
}

/**
 * Mint a fresh ACTIVE mission. Inputs are assumed validated (the CLI shell
 * validates); this only guarantees shape + revision 1.
 */
export function createMission({ objective, maxRounds = DEFAULT_MAX_ROUNDS, id, now } = {}) {
  const ts = normalizeNow(now)
  return {
    schema: MISSION_SCHEMA,
    id: id || deriveMissionId(objective, ts),
    revision: 1,
    objective,
    maxRounds: clampMaxRounds(maxRounds),
    roundsStarted: 0,
    status: 'active',
    blockedReason: null,
    blockedStreak: 0,
    createdAt: ts,
    updatedAt: ts,
  }
}

/**
 * Whether the mission may keep running autonomously: still active and inside
 * its round cap. Terminal/paused missions are false — continuation is an
 * explicit operator act (`resumeMission`).
 */
export function canContinueMission(mission) {
  return mission.status === 'active' && mission.roundsStarted < mission.maxRounds
}

/**
 * One continuation round. Terminal states absorb ticks idempotently (no
 * revision bump — nothing changed). `completed` finishes the mission; a
 * `blockedReason` only flips the status to BLOCKED after the same reason has
 * persisted for BLOCKED_MIN_ROUNDS consecutive rounds (a changing reason
 * resets the streak and the mission stays active). A plain tick clears any
 * blocked candidates.
 */
export function tickMission(mission, { completed = false, blockedReason = null, now } = {}) {
  if (mission.status === 'complete' || mission.status === 'blocked') {
    return clone(mission)
  }
  const ts = normalizeNow(now)
  const next = {
    ...mission,
    revision: mission.revision + 1,
    roundsStarted: mission.roundsStarted + 1,
    updatedAt: ts,
  }
  if (completed) {
    next.status = 'complete'
    next.blockedReason = null
    next.blockedStreak = 0
    return next
  }
  if (blockedReason) {
    const reason = String(blockedReason).trim()
    next.blockedReason = reason
    next.blockedStreak = mission.blockedReason === reason ? mission.blockedStreak + 1 : 1
    if (next.blockedStreak >= BLOCKED_MIN_ROUNDS) next.status = 'blocked'
    return next
  }
  next.blockedReason = null
  next.blockedStreak = 0
  return next
}

/**
 * Whether a persisted mission permits an autonomous conductor/dispatch tick.
 * No mission = legacy behavior (always allowed). A mission gates by the same
 * rules as the cockpit loop: only an ACTIVE mission inside its round budget
 * may be driven; blocked/complete/paused/spent budgets all refuse.
 */
export function missionAllowsDispatch(mission) {
  if (mission === null || mission === undefined) return true
  return mission.status === 'active' && mission.roundsStarted < mission.maxRounds
}

/** Concrete refusal reason for briefings/heartbeats (matches missionAllowsDispatch). */
export function missionGateReason(mission) {
  switch (mission?.status) {
    case 'blocked':
      return (
        'mission is blocked (' +
        (mission.blockedReason ?? 'no reason recorded') +
        ') — `mission resume <id>` after the blocker clears'
      )
    case 'complete':
      return 'mission is complete — mint a new one (mission create …)'
    case 'paused':
      return 'mission is paused — `mission resume <id>` to re-arm'
    case 'active':
      return `mission round budget reached (${mission.roundsStarted}/${mission.maxRounds}) — raise it (mission edit <id> --max-rounds N) or complete it`
    default:
      return 'mission is not continuable'
  }
}

/** Explicit operator pause (the only way an active mission stops without a tick). */
export function pauseMission(mission, { now } = {}) {
  if (mission.status !== 'active') return clone(mission)
  return {
    ...mission,
    revision: mission.revision + 1,
    status: 'paused',
    blockedReason: null,
    blockedStreak: 0,
    updatedAt: normalizeNow(now),
  }
}

/** Re-arm a paused or blocked mission. Terminal-by-completion stays inert —
 *  a complete mission is history; mint a new one. */
export function resumeMission(mission, { now } = {}) {
  if (mission.status !== 'paused' && mission.status !== 'blocked') return clone(mission)
  return {
    ...mission,
    revision: mission.revision + 1,
    status: 'active',
    blockedReason: null,
    blockedStreak: 0,
    updatedAt: normalizeNow(now),
  }
}

/**
 * Edit the objective (or round cap) of a non-terminal mission. A fresh
 * objective clears any blocking candidates — the blocking question is now a
 * different one.
 */
export function editMission(mission, { objective, maxRounds, now } = {}) {
  if (mission.status === 'complete' || mission.status === 'blocked') {
    return { ...clone(mission), updatedAt: normalizeNow(now) }
  }
  return {
    ...mission,
    revision: mission.revision + 1,
    objective: objective !== undefined ? objective : mission.objective,
    maxRounds: maxRounds !== undefined ? clampMaxRounds(maxRounds) : mission.maxRounds,
    blockedReason: null,
    blockedStreak: 0,
    updatedAt: normalizeNow(now),
  }
}

// ─── cockpit goal ↔ mission bridge (docs/plans/goal-mission-conformance.md) ──

// The conformance contract's vocabulary mapping. One table, two directions:
// the cockpit `/goal` statuses on the left, the mission statuses on the right.
export const GOAL_STATUS_TO_MISSION = {
  active: 'active',
  done: 'complete',
  paused: 'paused',
  blocked: 'blocked',
}

export const MISSION_STATUS_TO_GOAL = {
  active: 'active',
  complete: 'done',
  paused: 'paused',
  blocked: 'blocked',
}

/**
 * Convert a cockpit `/goal` store record into a mission record (revision 1).
 *
 * Fails closed on unknown goal statuses and on objectives the mission schema
 * rejects. Conversion rules from the contract: `max_rounds: null` (unbounded)
 * becomes `DEFAULT_MAX_ROUNDS` — the mission schema has no unbounded form —
 * and the workspace binding (`workspace`/`project_key`) travels as optional
 * `workspace`/`projectKey` fields so the round trip stays cockpit-loadable.
 * Cockpit-only fields (acceptance criteria, `accept_cmd`, notes) do not fit
 * the mission schema and are not carried.
 */
export function goalToMission(goal, { id, now } = {}) {
  const g = goal && typeof goal === 'object' ? goal : {}
  const text = String(g.text ?? '').trim()
  const objectiveError = validateObjective(text)
  if (objectiveError) throw new Error(`goal → mission: ${objectiveError}`)
  const status = String(g.status ?? 'active')
  const missionStatus = GOAL_STATUS_TO_MISSION[status]
  if (!missionStatus) throw new Error(`goal → mission: unknown goal status '${status}'`)
  const ts = normalizeNow(now)
  const created = Number(g.created_ms) > 0 ? new Date(Number(g.created_ms)).toISOString() : ts
  const updated = Number(g.updated_ms) > 0 ? new Date(Number(g.updated_ms)).toISOString() : ts
  const mission = {
    schema: MISSION_SCHEMA,
    id: id || deriveMissionId(text, ts),
    revision: 1,
    objective: text,
    maxRounds: g.max_rounds == null ? DEFAULT_MAX_ROUNDS : clampMaxRounds(Number(g.max_rounds)),
    roundsStarted: Math.max(0, Math.trunc(Number(g.rounds) || 0)),
    status: missionStatus,
    blockedReason:
      typeof g.blocked_reason === 'string' && g.blocked_reason.trim()
        ? g.blocked_reason.trim()
        : null,
    blockedStreak: Math.max(0, Math.trunc(Number(g.blocked_streak) || 0)),
    createdAt: created,
    updatedAt: updated,
  }
  if (typeof g.workspace === 'string' && g.workspace.trim()) mission.workspace = g.workspace
  if (typeof g.project_key === 'string' && g.project_key.trim()) mission.projectKey = g.project_key
  return mission
}

/**
 * Convert a mission record into a cockpit `/goal` store record.
 *
 * Fails closed on unknown mission statuses. Cockpit-only fields come back
 * defaulted (`acceptance: []`, `accept_cmd: null`, `notes: []`), `maxRounds`
 * (always numeric in the mission schema) becomes `max_rounds: Some(n)`, and
 * an imported workspace binding round-trips so the cockpit can load the file
 * for the same project.
 */
export function missionToGoal(mission) {
  const m = mission && typeof mission === 'object' ? mission : {}
  const status = MISSION_STATUS_TO_GOAL[String(m.status ?? '')]
  if (!status) throw new Error(`mission → goal: unknown mission status '${m.status}'`)
  const objectiveError = validateObjective(m.objective)
  if (objectiveError) throw new Error(`mission → goal: ${objectiveError}`)
  const created = Date.parse(String(m.createdAt ?? '')) || 0
  const updated = Date.parse(String(m.updatedAt ?? '')) || 0
  const goal = {
    text: String(m.objective ?? '').trim(),
    acceptance: [],
    accept_cmd: null,
    notes: [],
    status,
    rounds: Math.max(0, Math.trunc(Number(m.roundsStarted) || 0)),
    max_rounds: m.maxRounds == null ? null : clampMaxRounds(Number(m.maxRounds)),
    blocked_reason:
      typeof m.blockedReason === 'string' && m.blockedReason.trim() ? m.blockedReason.trim() : null,
    blocked_streak: Math.max(0, Math.trunc(Number(m.blockedStreak) || 0)),
    created_ms: created,
    updated_ms: updated,
  }
  if (typeof m.workspace === 'string' && m.workspace.trim()) goal.workspace = m.workspace
  if (typeof m.projectKey === 'string' && m.projectKey.trim()) goal.project_key = m.projectKey
  return goal
}

/** One-line operator summary (no secrets — it's a terminal row). */
export function missionSummary(mission) {
  const objective = mission.objective.replace(/\s+/g, ' ').slice(0, 72)
  const more = mission.objective.length > 72 ? '…' : ''
  const blocked = mission.blockedReason
    ? ` · blocked(${mission.blockedStreak}/${BLOCKED_MIN_ROUNDS}): ${mission.blockedReason.slice(0, 60)}`
    : ''
  return `[${mission.status}] r${mission.roundsStarted}/${mission.maxRounds} ${objective}${more}${blocked}`
}

/** Serialize one state row for the append-only ledger. */
export function serializeMission(mission) {
  return JSON.stringify(mission)
}

/**
 * Parse one ledger line back into a mission, or null when the row is not a
 * well-formed `angel.mission/v1` state (a corrupted row must never be treated
 * as live state — the pointer file remains authoritative).
 */
export function parseMission(line) {
  try {
    const v = JSON.parse(line)
    if (
      v?.schema !== MISSION_SCHEMA ||
      typeof v.id !== 'string' ||
      typeof v.revision !== 'number' ||
      !['active', 'paused', 'complete', 'blocked'].includes(v.status) ||
      typeof v.objective !== 'string' ||
      typeof v.maxRounds !== 'number' ||
      typeof v.roundsStarted !== 'number' ||
      typeof v.createdAt !== 'string' ||
      typeof v.updatedAt !== 'string'
    ) {
      return null
    }
    // Step-3 field ownership (conformance contract): the workspace binding
    // travels on the mission record as OPTIONAL string fields. Absent is
    // valid (legacy/import-less missions); any other type fails closed —
    // a malformed binding must never be silently dropped by the bridge.
    for (const key of ['workspace', 'projectKey']) {
      if (v[key] !== undefined && typeof v[key] !== 'string') {
        return null
      }
    }
    return v
  } catch {
    return null
  }
}

/** Fold an ordered ledger replay into one current mission (last valid row wins). */
export function replayMissions(rows) {
  let current = null
  for (const line of rows) {
    const m = parseMission(line)
    if (m) current = m
  }
  return current
}

/**
 * Persist one mission mutation with the canonical store format: append the
 * audit row to `<id>.jsonl`, then atomically swap the `<id>.json` pointer.
 * Exported so the Conductor tick can credit mission rounds without spawning
 * the mission CLI. Lazy-imports fs so importing this module stays cheap.
 */
export async function persistMission(mission, dir) {
  const { mkdirSync, appendFileSync, writeFileSync } = await import('./private-store-fs.mjs')
  const path = await import('node:path')
  mkdirSync(dir, { recursive: true })
  const ledger = path.join(dir, `${mission.id}.jsonl`)
  const pointer = path.join(dir, `${mission.id}.json`)
  appendFileSync(ledger, `${serializeMission(mission)}\n`)
  writeFileSync(pointer, serializeMission(mission))
  return mission
}

// ─── CLI shell (file I/O lives only here, imported lazily) ──────────────────

async function cli(argv) {
  const { readFileSync, renameSync, writeFileSync, readdirSync } =
    await import('./private-store-fs.mjs')
  const os = await import('node:os')
  const path = await import('node:path')

  const missionDir = () =>
    process.env.ANGEL_MISSION_DIR || path.join(os.homedir(), '.angel0', 'missions')

  const pointerPath = (id) => path.join(missionDir(), `${id}.json`)

  const loadMission = (id) => {
    try {
      return parseMission(readFileSync(pointerPath(id), 'utf8'))
    } catch {
      return null
    }
  }

  /** Persist a mutation: append the audit row, then atomically swap the pointer. */
  const commit = async (mission) => {
    await persistMission(mission, missionDir())
    return mission
  }

  const fail = (message) => {
    console.error(`mission: ${message}`)
    return 1
  }

  const listMissions = () => {
    let names
    try {
      names = readdirSync(missionDir())
    } catch {
      return []
    }
    return names
      .filter((n) => n.endsWith('.json'))
      .map((n) => loadMission(n.slice(0, -5)))
      .filter(Boolean)
      .sort((a, b) => b.updatedAt.localeCompare(a.updatedAt))
  }

  const command = argv[0]
  const flag = (names) => {
    const hit = argv.findIndex((a) => names.includes(a))
    return hit >= 0 ? { value: argv[hit + 1], index: hit } : { value: undefined, index: -1 }
  }
  const has = (names) => argv.some((a) => names.includes(a))

  if (command === 'create') {
    const objective = flag(['--objective', '-o']).value
    const objectiveError = validateObjective(objective)
    if (objectiveError) return fail(objectiveError)
    const idFlag = flag(['--id'])
    if (idFlag.value !== undefined) {
      const idError = validateMissionId(idFlag.value)
      if (idError) return fail(idError)
    }
    const maxRounds = flag(['--max-rounds']).value
    const mission = createMission({
      objective: String(objective).trim(),
      maxRounds: maxRounds === undefined ? DEFAULT_MAX_ROUNDS : clampMaxRounds(maxRounds),
      id: idFlag.value,
    })
    await commit(mission)
    console.log(missionSummary(mission))
    return 0
  }

  if (command === 'list') {
    for (const m of listMissions()) console.log(`${m.id}  ${missionSummary(m)}`)
    return 0
  }

  if (command === 'import-goal') {
    const goalPath = argv[1]
    if (!goalPath) return fail('usage: mission import-goal <goal-json-path> [--id slug]')
    let goal
    try {
      goal = JSON.parse(readFileSync(goalPath, 'utf8'))
    } catch {
      return fail(`cannot read goal file '${goalPath}'`)
    }
    const idFlag = flag(['--id'])
    if (idFlag.value !== undefined) {
      const idError = validateMissionId(idFlag.value)
      if (idError) return fail(idError)
    }
    let mission
    try {
      mission = goalToMission(goal, { id: idFlag.value })
    } catch (error) {
      return fail(String(error?.message ?? error))
    }
    await commit(mission)
    console.log(missionSummary(mission))
    return 0
  }

  if (command === 'export-goal') {
    const missionId = argv[1]
    const goalPath = argv[2]
    if (!missionId || !goalPath) {
      return fail('usage: mission export-goal <id> <goal-json-path>')
    }
    const mission = loadMission(missionId)
    if (!mission) return fail(`no mission '${missionId}'`)
    let goal
    try {
      goal = missionToGoal(mission)
    } catch (error) {
      return fail(String(error?.message ?? error))
    }
    const tmp = `${goalPath}.tmp-${process.pid}`
    try {
      writeFileSync(tmp, JSON.stringify(goal, null, 2))
      renameSync(tmp, goalPath)
    } catch {
      return fail(`cannot write goal file '${goalPath}'`)
    }
    console.log(missionSummary(mission))
    console.log(`goal written: ${goalPath}`)
    return 0
  }

  const id = argv[1]
  if (!id) {
    return fail('usage: mission create|import-goal|export-goal|tick|pause|resume|edit|show|list …')
  }

  if (command === 'show') {
    const mission = loadMission(id)
    if (!mission) return fail(`no mission '${id}'`)
    console.log(serializeMission(mission))
    return 0
  }

  const mission = loadMission(id)
  if (!mission) return fail(`no mission '${id}'`)

  if (command === 'tick') {
    if (has(['--completed', '-c'])) {
      await commit(tickMission(mission, { completed: true }))
      console.log(missionSummary(loadMission(id)))
      return 0
    }
    const blocked = flag(['--blocked', '-b']).value
    if (blocked !== undefined) {
      const reasonError = validateBlockedReason(blocked)
      if (reasonError) return fail(reasonError)
      const next = tickMission(mission, { blockedReason: String(blocked).trim() })
      await commit(next)
      if (next.status === 'blocked') {
        console.log(
          `mission: BLOCKED after ${BLOCKED_MIN_ROUNDS} consecutive rounds with the same reason`,
        )
      }
      console.log(missionSummary(next))
      return 0
    }
    if (!canContinueMission(mission)) {
      return fail(
        mission.status === 'complete'
          ? 'mission is complete'
          : mission.status === 'blocked'
            ? 'mission is blocked — mint a new one'
            : `round cap reached (${mission.roundsStarted}/${mission.maxRounds}) — raise --max-rounds via edit or complete it`,
      )
    }
    await commit(tickMission(mission, {}))
    console.log(missionSummary(loadMission(id)))
    return 0
  }

  if (command === 'pause') {
    await commit(pauseMission(mission))
    console.log(missionSummary(loadMission(id)))
    return 0
  }

  if (command === 'resume') {
    await commit(resumeMission(mission))
    console.log(missionSummary(loadMission(id)))
    return 0
  }

  if (command === 'edit') {
    const objective = flag(['--objective', '-o']).value
    if (objective !== undefined) {
      const objectiveError = validateObjective(objective)
      if (objectiveError) return fail(objectiveError)
    }
    const maxRounds = flag(['--max-rounds']).value
    if (maxRounds !== undefined && !Number.isFinite(Number(maxRounds))) {
      return fail('--max-rounds must be a number')
    }
    await commit(
      editMission(mission, {
        objective: objective !== undefined ? String(objective).trim() : undefined,
        maxRounds: maxRounds !== undefined ? Number(maxRounds) : undefined,
      }),
    )
    console.log(missionSummary(loadMission(id)))
    return 0
  }

  return fail(
    'usage: mission create --objective <text> [--id slug] [--max-rounds N] | ' +
      'import-goal <goal-json-path> [--id slug] | export-goal <id> <goal-json-path> | ' +
      'tick <id> [--completed] [--blocked <reason>] | pause <id> | resume <id> | ' +
      'edit <id> [--objective <text>] [--max-rounds N] | show <id> | list',
  )
}

const isCli =
  process.argv[1] && (await import('node:url')).fileURLToPath(import.meta.url) === process.argv[1]
if (isCli) {
  cli(process.argv.slice(2)).then((code) => process.exit(code ?? 0))
}
