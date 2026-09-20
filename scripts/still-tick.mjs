import { runStoreCli } from './worker-lock.mjs'
import { workerPaths } from './worker-paths.mjs'
// still-tick — Angel's Share scoring + weekly distillation + unconditional
// evaporation (AS2+AS3 of docs/WORKERS.md), sibling of reflex-tick
// (M5), dossier-tick (D4), and habitsmith-tick (H5): same gate stack (kill
// switch, night window, idle, lockfile, heartbeat), pointed at the barrel the
// cockpit's Rust writer fills (cockpit/src/barrel.rs).
//
// One headless, cron-driven tick, three duties in strict order:
//   1. EVAPORATE (unconditional): delete every shard older than
//      ANGEL_BARREL_TTL_DAYS — scored or not, distilled or not. A missed week
//      is a counted loss (the angels' share), never accumulation. This runs
//      even when ANGEL_STILL=0 kills the rest: the TTL bound is the
//      operator's hard privacy/disk guarantee; disarm ANGEL_BARREL to stop
//      the pipeline, not the evaporator.
//   2. SCORE (idle GPU work, bounded per tick): shadow-replay unscored
//      captures from *closed* (previous-day) shards on the student surface,
//      have the judge grade teacher-vs-student, annotate records in place.
//      Today's shard belongs to the Rust writer and is never touched.
//   3. DISTILL (required weekly): on the first tick on/after the target
//      weekday each ISO week, select the highest-value scored captures into
//      a capped distillate-<week>.jsonl of forge-ingestible training records
//      ({ts_ms, club, answer, messages, reward} — external trainer input),
//      copy it into the trajectories dir so forge-feed.sh ships it, fold the
//      week into ~/.angel0/still/gap.json, append a still_run
//      ledger record, and write the briefing + /still status export.
//
// No model surface is hardcoded (the fleet-model-discovery rule): scoring
// needs ANGEL_STILL_STUDENT_URL (+_MODEL); unset means scoring is skipped
// with a logged reason while evaporation still holds the TTL line.

import {
  isIdle,
  makeHeartbeat,
  angelTtyRunning,
  rollBudget,
  recordRun,
  budgetAllows,
} from './reflex-tick.mjs'
import { inWindow } from './conductor-tick.mjs'

export const DEFAULT_WINDOW = '02:00-07:00'
export const DEFAULT_TTL_DAYS = 7
export const DEFAULT_DISTILLATE_MAX_MB = 32
export const DEFAULT_MAX_SCORES = 40
/** Minimum quality×gap for a scored capture to be distillate-worthy. */
export const DEFAULT_MIN_VALUE = 10

/** Kill switch: ANGEL_STILL=0 disables scoring+distillation (never evaporation). */
export function killed(env = {}) {
  return String(env.ANGEL_STILL ?? '').trim() === '0'
}

// ─── calendar helpers (all UTC, all pure) ────────────────────────────────────

/** 'barrel-19700102.jsonl' -> '19700102' (null for anything else). */
export function shardDay(name) {
  const m = /^barrel-(\d{8})\.jsonl$/.exec(name ?? '')
  return m ? m[1] : null
}

export function utcDayStr(tsSec) {
  return new Date(tsSec * 1000).toISOString().slice(0, 10).replace(/-/g, '')
}

/** Shards strictly before today — the Rust writer owns today's file. */
export function closedShards(names, todayStr) {
  return (names ?? []).filter((n) => {
    const d = shardDay(n)
    return d && d < todayStr
  })
}

/** First day (YYYYMMDD) still allowed to live: today - ttlDays. */
export function cutoffDayStr(nowSec, ttlDays) {
  return utcDayStr(nowSec - ttlDays * 86_400)
}

/** Shards past TTL — these evaporate unconditionally. */
export function expiredShards(names, cutoffStr) {
  return (names ?? []).filter((n) => {
    const d = shardDay(n)
    return d && d < cutoffStr
  })
}

/** ISO-8601 week label, e.g. '2026-W28'. */
export function isoWeek(tsSec) {
  const d = new Date(tsSec * 1000)
  const t = new Date(Date.UTC(d.getUTCFullYear(), d.getUTCMonth(), d.getUTCDate()))
  const day = t.getUTCDay() || 7
  t.setUTCDate(t.getUTCDate() + 4 - day)
  const yearStart = Date.UTC(t.getUTCFullYear(), 0, 1)
  const week = Math.ceil(((t.getTime() - yearStart) / 86_400_000 + 1) / 7)
  return `${t.getUTCFullYear()}-W${String(week).padStart(2, '0')}`
}

export const WEEKDAYS = { sun: 0, mon: 1, tue: 2, wed: 3, thu: 4, fri: 5, sat: 6 }

/**
 * The weekly-required rule: distillation is due on the first tick on/after
 * the target weekday of each ISO week — computed from the most recent
 * occurrence of that weekday, so a missed Sunday catches up on Monday
 * instead of silently waiting a week.
 */
export function distillDue({ nowSec, targetDow = 0, lastWeek = null }) {
  const dow = new Date(nowSec * 1000).getUTCDay()
  const back = (dow - targetDow + 7) % 7
  const week = isoWeek(nowSec - back * 86_400)
  return { due: week !== lastWeek, week }
}

// ─── record shaping (pure) ───────────────────────────────────────────────────

export function clip(text, max) {
  const t = String(text ?? '')
  return t.length <= max ? t : `${t.slice(0, max)}\n…[clipped ${t.length - max} chars]`
}

/**
 * Barrel context -> plain role/content messages for the student replay.
 * OpenAI-compatible surfaces reject assistant tool_calls without live call
 * ids, so structured calls/results are folded into text — shadow fidelity,
 * not protocol fidelity.
 */
export function flattenForReplay(context) {
  return (context ?? []).map((m) => {
    let content = m.content ?? ''
    if (Array.isArray(m.tool_calls) && m.tool_calls.length) {
      const calls = m.tool_calls.map((c) => `[called ${c.name}: ${clip(c.args, 400)}]`).join('\n')
      content = content ? `${content}\n${calls}` : calls
    }
    const role = m.role === 'tool' ? 'user' : m.role
    if (m.role === 'tool') content = `[tool result] ${content}`
    return { role, content }
  })
}

export function judgePrompt(capture, studentAnswer) {
  const convo = flattenForReplay(capture.context)
    .map((m) => `${m.role}: ${m.content}`)
    .join('\n')
  return [
    'You are grading a student model against a teacher model on the same conversation.',
    'Reply with ONLY a JSON object: {"quality": q, "gap": g} where q (0-10) is the',
    'quality of the TEACHER answer, and g (0-10) is how much BETTER the teacher answer',
    'is than the student answer (0 = student equal or better).',
    '',
    `[CONVERSATION]\n${clip(convo, 6000)}`,
    '',
    `[TEACHER ANSWER]\n${clip(capture.answer, 2500)}`,
    '',
    `[STUDENT ANSWER]\n${clip(studentAnswer, 2500)}`,
  ].join('\n')
}

/** Tolerant judge parse: first {...} JSON island; clamp both axes to 0-10. */
export function parseJudge(text) {
  const m = /\{[\s\S]*?\}/.exec(String(text ?? ''))
  if (!m) return null
  let obj
  try {
    obj = JSON.parse(m[0])
  } catch {
    return null
  }
  const clamp = (v) => Math.max(0, Math.min(10, Number(v)))
  const quality = clamp(obj.quality)
  const gap = clamp(obj.gap)
  if (Number.isNaN(quality) || Number.isNaN(gap)) return null
  return { quality, gap }
}

/** Selection value: a sample matters when the teacher is good AND the student can't do it yet. */
export function scoreValue({ quality, gap }) {
  return quality * gap
}

export function extractChatContent(data) {
  const c = data?.choices?.[0]
  return c?.message?.content ?? c?.text ?? ''
}

/**
 * One forge-ingestible training record (local dataset contract:
 * ts_ms/club/answer/messages/reward; extra keys ride along as provenance).
 * reward = teacher quality (0-1) so FORGE_MIN_REWARD filters on quality;
 * the student gap stays in meta — it drove selection, not the label.
 */
export function distillateRecord(rec, week) {
  return {
    ts_ms: (rec.ts ?? 0) * 1000,
    club: rec.teacher?.club ?? '',
    answer: rec.answer ?? '',
    messages: (rec.context ?? []).map((m) => ({
      role: m.role,
      content: m.content ?? '',
      ...(Array.isArray(m.tool_calls) && m.tool_calls.length ? { tool_calls: m.tool_calls } : {}),
    })),
    reward: (rec.score?.quality ?? 0) / 10,
    week,
    digest: rec.digest,
    repo: rec.repo?.key ?? null,
    teacher_model: rec.teacher?.model ?? null,
    gap: rec.score?.gap ?? null,
  }
}

/**
 * One pass: truthy `score` rows contribute once to the scored list and to
 * quality/gap sums. Selection and the gap gauge share this scan.
 */
function collectScored(records) {
  const list = records ?? []
  const scored = []
  let qualitySum = 0
  let gapSum = 0
  for (const rec of list) {
    const score = rec.score
    if (!score) continue
    scored.push(rec)
    qualitySum += score.quality
    gapSum += score.gap
  }
  return { captures: list.length, scored, qualitySum, gapSum }
}

function gapSummaryFromScan(
  { captures, scored, qualitySum, gapSum },
  week,
  { evaporatedUnscored = 0, distillateBytes = 0, ts = 0 } = {},
) {
  const n = scored.length
  const mean = (sum) => (n ? sum / n : null)
  const round = (v) => (v == null ? null : Math.round(v * 100) / 100)
  return {
    week,
    ts,
    captures,
    scored: n,
    mean_quality: round(mean(qualitySum)),
    mean_gap: round(mean(gapSum)),
    evaporated_unscored: evaporatedUnscored,
    distillate_bytes: distillateBytes,
  }
}

/**
 * Greedy fill by cached value desc under the byte cap; ties break toward
 * newer captures. Serializes each considered line once (returned as `lines`).
 */
function selectGoldenFromScan(scan, capBytes, { minValue = DEFAULT_MIN_VALUE, week = '' } = {}) {
  const eligible = []
  for (const rec of scan.scored) {
    const value = scoreValue(rec.score)
    if (value >= minValue) eligible.push({ rec, value, ts: rec.ts ?? 0 })
  }
  eligible.sort((a, b) => b.value - a.value || b.ts - a.ts)
  const golden = []
  const lines = []
  let bytes = 0
  let droppedCap = 0
  for (const { rec } of eligible) {
    const line = JSON.stringify(distillateRecord(rec, week))
    if (bytes + line.length + 1 > capBytes) {
      droppedCap += 1
      continue
    }
    bytes += line.length + 1
    golden.push(rec)
    lines.push(line)
  }
  return {
    golden,
    bytes,
    droppedLowValue: scan.scored.length - eligible.length,
    droppedCap,
    lines,
  }
}

/**
 * Greedy fill by value desc under the byte cap; ties break toward newer
 * captures. Returns { golden, bytes, droppedLowValue, droppedCap }.
 */
export function selectGolden(records, capBytes, opts = {}) {
  const { golden, bytes, droppedLowValue, droppedCap } = selectGoldenFromScan(
    collectScored(records),
    capBytes,
    opts,
  )
  return { golden, bytes, droppedLowValue, droppedCap }
}

/** One gap-gauge row — the weekly trend point the Observatory charts. */
export function gapSummary(records, week, extras = {}) {
  return gapSummaryFromScan(collectScored(records), week, extras)
}

/** Re-emit shard lines with scores attached by digest. Unparseable lines pass through. */
export function annotateLines(lines, scoresByDigest) {
  return (lines ?? []).map((line) => {
    let rec
    try {
      rec = JSON.parse(line)
    } catch {
      return line
    }
    const score = scoresByDigest[rec?.digest]
    if (!score || rec.score) return line
    return JSON.stringify({ ...rec, score })
  })
}

// ─── student surface discovery ───────────────────────────────────────────────
// Endpoints are candidates only — the model id is always read live from
// /models (the fleet-model-discovery rule: no hardcoded model names), and
// ANGEL_STILL_STUDENT_URL/_MODEL pin explicitly when set. When nothing is
// serving, scoring skips with a logged reason and evaporation still runs.

export const DEFAULT_STUDENT_ENDPOINTS = [
  'http://127.0.0.1:8090/v1',
  'http://127.0.0.1:8093/v1',
  'http://127.0.0.1:8092/v1',
]

/** Candidate base URLs, explicit pins first, deduped. */
export function studentCandidates(env = {}) {
  if (env.ANGEL_STILL_STUDENT_URL) return [String(env.ANGEL_STILL_STUDENT_URL).replace(/\/+$/, '')]
  const list = [env.ANGEL_TURBO_URL, env.TURBO_BASE_URL]
    .concat(DEFAULT_STUDENT_ENDPOINTS)
    .filter(Boolean)
    .map((u) => String(u).replace(/\/+$/, ''))
  return [...new Set(list)]
}

function authHeaders(key) {
  return key ? { authorization: `Bearer ${key}` } : {}
}

/**
 * First candidate with a live /models wins; model id comes from the surface.
 * The bearer key (ANGEL_STILL_STUDENT_KEY, else ANGEL_BRAIN_KEY — turbo's
 * nex serving needs one) rides both the probe and the returned handle.
 */
export async function discoverStudent(env = {}, { fetchImpl = fetch, timeoutMs = 5_000 } = {}) {
  const pinned = (env.ANGEL_STILL_STUDENT_MODEL || '').trim()
  const key = env.ANGEL_STILL_STUDENT_KEY || env.ANGEL_BRAIN_KEY || ''
  for (const base of studentCandidates(env)) {
    try {
      const res = await fetchImpl(`${base}/models`, {
        headers: authHeaders(key),
        signal: AbortSignal.timeout(timeoutMs),
      })
      if (!res.ok) continue
      const data = await res.json()
      const model = pinned || data?.data?.[0]?.id || ''
      if (model) return { url: base, model, key }
    } catch {
      /* next candidate */
    }
  }
  return null
}

/** POST one chat completion to an OpenAI-compatible base URL (…/v1). */
export async function chatOnce(
  baseUrl,
  model,
  messages,
  { fetchImpl = fetch, maxTokens = 1024, temperature = 0, timeoutMs = 120_000, apiKey = '' } = {},
) {
  const url = `${String(baseUrl).replace(/\/+$/, '')}/chat/completions`
  const res = await fetchImpl(url, {
    method: 'POST',
    headers: { 'content-type': 'application/json', ...authHeaders(apiKey) },
    body: JSON.stringify({ model, messages, max_tokens: maxTokens, temperature }),
    signal: AbortSignal.timeout(timeoutMs),
  })
  if (!res.ok) throw new Error(`chat ${res.status} from ${url}`)
  return extractChatContent(await res.json())
}

// ─── CLI — the one tick ──────────────────────────────────────────────────────

async function cli(argv) {
  const fs = await import('./private-store-fs.mjs')
  const { execFileSync } = await import('node:child_process')
  const { fileURLToPath } = await import('node:url')
  const { dirname, join } = await import('node:path')
  const os = await import('node:os')

  const ROOT = join(dirname(fileURLToPath(import.meta.url)), '..')
  const flag = (n, d) => {
    const i = argv.indexOf(n)
    return i >= 0 ? argv[i + 1] : d
  }
  const dryRun = argv.includes('--dry-run')
  const force = argv.includes('--force')
  const forceDistill = argv.includes('--distill')

  const stateDir = flag('--state-dir', workerPaths().still)
  const barrelDir = flag('--barrel-dir', process.env.ANGEL_BARREL_DIR || workerPaths().barrel)
  const trajDir = flag('--traj-dir', process.env.ANGEL_TRAJECTORY_DIR || workerPaths().trajectories)
  const ledgerPath = flag('--ledger', process.env.ANGEL_EXPERIENCE_LOG) || workerPaths().ledger
  const gaugePath = flag('--gauge', join(stateDir, 'gap.json'))
  const window = flag('--window', process.env.ANGEL_STILL_WINDOW ?? DEFAULT_WINDOW)
  const ttlDays = Number(process.env.ANGEL_BARREL_TTL_DAYS || DEFAULT_TTL_DAYS)
  const targetDow = WEEKDAYS[(process.env.ANGEL_STILL_WEEKDAY || 'sun').toLowerCase()] ?? 0
  const maxScores = Number(process.env.ANGEL_STILL_MAX_SCORES || DEFAULT_MAX_SCORES)
  const distillateCap =
    Number(process.env.ANGEL_DISTILLATE_MAX_MB || DEFAULT_DISTILLATE_MAX_MB) * 1024 * 1024
  const minValue = Number(process.env.ANGEL_STILL_MIN_VALUE || DEFAULT_MIN_VALUE)
  const idleMs = Number(flag('--idle-min', '15')) * 60_000

  const dailyCap = Number(process.env.ANGEL_STILL_MAX_SCORES_PER_DAY || DEFAULT_MAX_SCORES)
  for (const [name, value] of Object.entries({ maxScores, dailyCap, ttlDays, distillateCap })) {
    if (!Number.isFinite(value) || value <= 0)
      throw new Error(`${name} must be positive and finite`)
  }
  const nowMs = Date.now()
  const now = Math.floor(nowMs / 1000)
  const nowIso = new Date(nowMs).toISOString()
  const today = utcDayStr(now)
  fs.mkdirSync(stateDir, { recursive: true })
  const heartbeatPath = join(stateDir, 'heartbeat.json')
  const budgetPath = join(stateDir, 'budget.json')
  const readJson = (p, d) => {
    try {
      return JSON.parse(fs.readFileSync(p, 'utf8'))
    } catch {
      return d
    }
  }
  const writeHeartbeat = (hb) => fs.writeFileSync(heartbeatPath, JSON.stringify(hb, null, 2))
  const readLines = (p) =>
    fs
      .readFileSync(p, 'utf8')
      .split('\n')
      .filter((l) => l.trim())
  const parseLines = (lines) =>
    lines
      .map((l) => {
        try {
          return JSON.parse(l)
        } catch {
          return null
        }
      })
      .filter(Boolean)

  // ── gates ──
  const isKilled = killed(process.env)
  const windowOk = force || inWindow(new Date(nowMs), window)
  const angelTty = angelTtyRunning(execFileSync)
  let newestLedgerMs = null
  try {
    newestLedgerMs = fs.statSync(ledgerPath).mtimeMs
  } catch {
    /* no ledger yet */
  }
  const idle = force || isIdle({ angelTty, newestLedgerMs, now: nowMs, idleMs })

  const shardNames = fs.existsSync(barrelDir)
    ? fs.readdirSync(barrelDir).filter((n) => shardDay(n))
    : []

  // ── 1. EVAPORATE — unconditional, runs even when killed/busy ──
  const cutoff = cutoffDayStr(now, ttlDays)
  const expired = expiredShards(shardNames, cutoff)
  let evaporatedUnscoredNow = 0
  let evaporatedRecordsNow = 0
  for (const name of expired) {
    const p = join(barrelDir, name)
    const recs = parseLines(readLines(p))
    evaporatedRecordsNow += recs.length
    evaporatedUnscoredNow += recs.filter((r) => !r.score).length
    if (!dryRun) fs.rmSync(p, { force: true })
  }
  if (expired.length)
    console.log(
      `still-tick: evaporated ${expired.length} shard(s), ${evaporatedRecordsNow} record(s) ` +
        `(${evaporatedUnscoredNow} never scored — the angels' share)`,
    )
  const liveShards = shardNames.filter((n) => !expired.includes(n))

  // Accumulate evaporation losses across ticks so the weekly gauge row
  // reports the whole week's share, not just distill-night's.
  const lossPath = join(stateDir, 'evaporation-losses.json')
  const losses = readJson(lossPath, { unscored: 0, records: 0 })
  if (!dryRun && (evaporatedUnscoredNow || evaporatedRecordsNow)) {
    losses.unscored += evaporatedUnscoredNow
    losses.records += evaporatedRecordsNow
    fs.writeFileSync(lossPath, JSON.stringify(losses))
  }

  let budget = rollBudget(readJson(budgetPath, null), nowIso.slice(0, 10), dailyCap)
  const publishStatus = (extra = {}) =>
    fs.writeFileSync(
      join(stateDir, 'status.json'),
      JSON.stringify(
        {
          v: 1,
          ts: now,
          shards: liveShards.length,
          unscored: closedShards(liveShards, today).reduce(
            (total, name) =>
              total +
              parseLines(readLines(join(barrelDir, name))).filter((row) => !row.score).length,
            0,
          ),
          scored_this_tick: 0,
          score_failures: 0,
          last_distill_week: readJson(join(stateDir, 'last-distill.json'), {}).week ?? null,
          gap_trend: Array.isArray(readJson(gaugePath, []))
            ? readJson(gaugePath, []).slice(-4)
            : [],
          budget,
          ...extra,
        },
        null,
        2,
      ),
    )

  // ── decide the rest ──
  const decision = isKilled
    ? { run: false, reason: 'disabled (ANGEL_STILL=0)' }
    : !windowOk
      ? { run: false, reason: `outside window ${window}` }
      : !idle
        ? { run: false, reason: 'busy — live session or recent ledger activity' }
        : { run: true, reason: 'idle, in window' }

  if (dryRun) {
    console.log(
      `still-tick (dry-run) · ${decision.run ? 'WOULD RUN' : 'WOULD SKIP'} — ${decision.reason}\n` +
        `  gates: killed=${isKilled} window=${windowOk} idle=${idle} (angelTty=${angelTty})\n` +
        `  barrel: ${shardNames.length} shard(s), ${expired.length} would evaporate (cutoff ${cutoff})`,
    )
    return 0
  }
  if (!decision.run) {
    publishStatus({ action: 'skip', reason: decision.reason })
    writeHeartbeat(makeHeartbeat({ now: nowIso, action: 'skip', reason: decision.reason }))
    console.log(`still-tick: skip — ${decision.reason}`)
    return 0
  }

  // ── 2. SCORE — closed shards only, bounded per tick ──
  const closed = closedShards(liveShards, today)
  let scoredNow = 0
  let scoreFailures = 0
  let unscoredLeft = 0
  // The turbo serving port is the student: discover it live now (after the
  // gates, so skips never probe the network). Explicit env pins win inside
  // studentCandidates/discoverStudent.
  const pendingCount = closed.reduce(
    (sum, name) =>
      sum + parseLines(readLines(join(barrelDir, name))).filter((row) => !row.score).length,
    0,
  )
  const student = pendingCount && budgetAllows(budget) ? await discoverStudent(process.env) : null
  const judgeUrl = process.env.ANGEL_STILL_JUDGE_URL || student?.url || ''
  const judgeModel = process.env.ANGEL_STILL_JUDGE_MODEL || student?.model || ''
  const judgeKey = process.env.ANGEL_STILL_JUDGE_KEY || student?.key || ''
  if (!student && pendingCount && budgetAllows(budget)) {
    console.log(
      `still-tick: scoring skipped — no student surface reachable (${studentCandidates(process.env).length} candidate(s) probed; pin with ANGEL_STILL_STUDENT_URL); evaporation still enforced`,
    )
  } else if (student) {
    let consecutiveFailures = 0
    console.log(`still-tick: student = ${student.model} @ ${student.url}`)
    for (const name of closed) {
      if (
        scoredNow + scoreFailures >= maxScores ||
        !budgetAllows(budget) ||
        consecutiveFailures >= 3
      )
        break
      const p = join(barrelDir, name)
      const lines = readLines(p)
      const recs = parseLines(lines)
      const pending = recs.filter((r) => !r.score && r.digest)
      if (!pending.length) continue
      const scores = {}
      for (const rec of pending) {
        if (
          scoredNow + scoreFailures >= maxScores ||
          !budgetAllows(budget) ||
          consecutiveFailures >= 3
        )
          break
        budget = recordRun(budget)
        fs.writeFileSync(budgetPath, JSON.stringify(budget))
        try {
          const studentAnswer = await chatOnce(
            student.url,
            student.model,
            flattenForReplay(rec.context),
            { apiKey: student.key },
          )
          const verdictText = await chatOnce(
            judgeUrl,
            judgeModel,
            [{ role: 'user', content: judgePrompt(rec, studentAnswer) }],
            { apiKey: judgeKey },
          )
          const verdict = parseJudge(verdictText)
          if (!verdict) {
            scoreFailures += 1
            consecutiveFailures += 1
            continue
          }
          scores[rec.digest] = {
            ...verdict,
            value: scoreValue(verdict),
            student: clip(studentAnswer, 2000),
            student_model: student.model,
            judge: judgeModel,
            judged_at: now,
          }
          scoredNow += 1
          consecutiveFailures = 0
        } catch {
          scoreFailures += 1
          consecutiveFailures += 1
        }
      }
      if (Object.keys(scores).length) {
        const tmp = `${p}.tmp`
        fs.writeFileSync(tmp, `${annotateLines(lines, scores).join('\n')}\n`)
        fs.renameSync(tmp, p)
      }
    }
  }
  for (const name of closed) {
    unscoredLeft += parseLines(readLines(join(barrelDir, name))).filter((r) => !r.score).length
  }

  // ── 3. DISTILL — required weekly ──
  const distillStatePath = join(stateDir, 'last-distill.json')
  const lastWeek = readJson(distillStatePath, {}).week ?? null
  const { due, week } = distillDue({ nowSec: now, targetDow, lastWeek })
  let distilled = null
  if (due || forceDistill) {
    const all = closed.flatMap((name) => parseLines(readLines(join(barrelDir, name))))
    const scan = collectScored(all)
    const { golden, bytes, droppedLowValue, droppedCap, lines } = selectGoldenFromScan(
      scan,
      distillateCap,
      { minValue, week },
    )
    const distillDir = join(barrelDir, 'distillate')
    fs.mkdirSync(distillDir, { recursive: true })
    const fileName = `distillate-${week}.jsonl`
    const body = lines.join('\n')
    const canonical = join(distillDir, fileName)
    fs.writeFileSync(canonical, body ? `${body}\n` : '')
    // Publish the local dataset for an explicitly configured external trainer.
    fs.mkdirSync(trajDir, { recursive: true })
    fs.copyFileSync(canonical, join(trajDir, fileName))

    const gauge = Array.isArray(readJson(gaugePath, [])) ? readJson(gaugePath, []) : []
    const row = gapSummaryFromScan(scan, week, {
      evaporatedUnscored: losses.unscored,
      distillateBytes: bytes,
      ts: now,
    })
    gauge.push(row)
    fs.mkdirSync(dirname(gaugePath), { recursive: true })
    fs.writeFileSync(gaugePath, JSON.stringify(gauge, null, 2))
    fs.writeFileSync(lossPath, JSON.stringify({ unscored: 0, records: 0 }))
    fs.writeFileSync(distillStatePath, JSON.stringify({ week, ts: now }))

    try {
      fs.mkdirSync(dirname(ledgerPath), { recursive: true })
      fs.appendFileSync(
        ledgerPath,
        `${JSON.stringify({
          kind: 'still_run',
          v: 1,
          ts: now,
          week,
          golden: golden.length,
          distillate_bytes: bytes,
          dropped_low_value: droppedLowValue,
          dropped_cap: droppedCap,
          evaporated_unscored: losses.unscored,
        })}\n`,
      )
    } catch {
      /* best-effort, like every ledger writer */
    }
    distilled = { week, golden: golden.length, bytes, droppedLowValue, droppedCap, row }
  }

  // ── status export for /still (Rust reads only this + writer-status.json) ──
  const gaugeNow = Array.isArray(readJson(gaugePath, [])) ? readJson(gaugePath, []) : []
  const status = {
    v: 1,
    ts: now,
    shards: liveShards.length,
    unscored: unscoredLeft,
    scored_this_tick: scoredNow,
    score_failures: scoreFailures,
    last_distill_week: readJson(distillStatePath, {}).week ?? null,
    gap_trend: gaugeNow.slice(-4),
    budget,
  }
  fs.writeFileSync(join(stateDir, 'status.json'), JSON.stringify(status, null, 2))

  const brief = [
    `# still-tick ${nowIso}`,
    `evaporated: ${expired.length} shard(s), ${evaporatedRecordsNow} record(s) (${evaporatedUnscoredNow} unscored)`,
    `scored: ${scoredNow} (+${scoreFailures} failures, ${unscoredLeft} pending)`,
    distilled
      ? `distilled ${distilled.week}: ${distilled.golden} golden, ${distilled.bytes} bytes ` +
        `(dropped ${distilled.droppedLowValue} low-value, ${distilled.droppedCap} over cap)`
      : `distill: not due (last ${lastWeek ?? 'never'})`,
  ].join('\n')
  fs.writeFileSync(join(stateDir, 'briefing.md'), `${brief}\n`)

  writeHeartbeat(
    makeHeartbeat({
      now: nowIso,
      action: 'ran',
      reason: brief.split('\n').slice(1).join(' · '),
    }),
  )
  console.log(`still-tick: ${brief.split('\n').slice(1).join(' · ')}`)
  return 0
}

const isCli =
  process.argv[1] && (await import('node:url')).fileURLToPath(import.meta.url) === process.argv[1]
if (isCli) {
  runStoreCli(cli, process.argv.slice(2), workerPaths().still).then((code) =>
    process.exit(code ?? 0),
  )
}
