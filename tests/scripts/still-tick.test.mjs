import { test } from 'node:test'
import assert from 'node:assert/strict'
import {
  killed,
  studentCandidates,
  discoverStudent,
  DEFAULT_STUDENT_ENDPOINTS,
  shardDay,
  utcDayStr,
  closedShards,
  cutoffDayStr,
  expiredShards,
  isoWeek,
  distillDue,
  clip,
  flattenForReplay,
  judgePrompt,
  parseJudge,
  scoreValue,
  extractChatContent,
  distillateRecord,
  selectGolden,
  gapSummary,
  annotateLines,
} from '../../scripts/runtime/still-tick.mjs'

// 2026-01-01T00:00:00Z and 2026-07-10T00:00:00Z
const JAN1_2026 = Date.UTC(2026, 0, 1) / 1000
const JUL10_2026 = Date.UTC(2026, 6, 10) / 1000

test('kill switch is the exact-zero family convention', () => {
  assert.equal(killed({ ANGEL_STILL: '0' }), true)
  assert.equal(killed({ ANGEL_STILL: '1' }), false)
  assert.equal(killed({}), false)
})

test('shard names parse and everything else is ignored', () => {
  assert.equal(shardDay('barrel-19700102.jsonl'), '19700102')
  assert.equal(shardDay('barrel-19700102.jsonl.tmp'), null)
  assert.equal(shardDay('writer-status.json'), null)
  assert.equal(shardDay('distillate-2026-W28.jsonl'), null)
})

test('closed shards exclude today — the Rust writer owns the live file', () => {
  const names = ['barrel-20260708.jsonl', 'barrel-20260709.jsonl', 'barrel-20260710.jsonl']
  assert.deepEqual(closedShards(names, '20260710'), [
    'barrel-20260708.jsonl',
    'barrel-20260709.jsonl',
  ])
})

test('TTL cutoff and expiry are day-granular', () => {
  assert.equal(utcDayStr(JUL10_2026), '20260710')
  assert.equal(cutoffDayStr(JUL10_2026, 7), '20260703')
  const names = ['barrel-20260702.jsonl', 'barrel-20260703.jsonl', 'barrel-20260709.jsonl']
  assert.deepEqual(expiredShards(names, '20260703'), ['barrel-20260702.jsonl'])
})

test('isoWeek handles plain and year-boundary dates', () => {
  assert.equal(isoWeek(JUL10_2026), '2026-W28')
  assert.equal(isoWeek(JAN1_2026), '2026-W01')
  // 2021-01-01 was a Friday belonging to ISO week 2020-W53.
  assert.equal(isoWeek(Date.UTC(2021, 0, 1) / 1000), '2020-W53')
})

test('distillDue fires on the target weekday and catches up after a miss', () => {
  const sunJul5 = Date.UTC(2026, 6, 5) / 1000
  const monJul6 = Date.UTC(2026, 6, 6) / 1000
  const sunJul12 = Date.UTC(2026, 6, 12) / 1000
  // First ever run: due immediately.
  assert.deepEqual(distillDue({ nowSec: sunJul5, targetDow: 0, lastWeek: null }), {
    due: true,
    week: '2026-W27',
  })
  // Monday after a distilled Sunday: not due again.
  assert.equal(distillDue({ nowSec: monJul6, targetDow: 0, lastWeek: '2026-W27' }).due, false)
  // Monday after a MISSED Sunday: catches up instead of waiting a week.
  assert.deepEqual(distillDue({ nowSec: monJul6, targetDow: 0, lastWeek: '2026-W26' }), {
    due: true,
    week: '2026-W27',
  })
  // Next Sunday: due for the new week.
  assert.equal(distillDue({ nowSec: sunJul12, targetDow: 0, lastWeek: '2026-W27' }).due, true)
})

test('replay flattening folds tool machinery into text', () => {
  const context = [
    { role: 'system', content: 'be brief' },
    { role: 'assistant', content: '', tool_calls: [{ name: 'shell', args: '{"command":"ls"}' }] },
    { role: 'tool', content: 'file-a file-b' },
    { role: 'user', content: 'so?' },
  ]
  const flat = flattenForReplay(context)
  assert.deepEqual(
    flat.map((m) => m.role),
    ['system', 'assistant', 'user', 'user'],
  )
  assert.match(flat[1].content, /\[called shell: \{"command":"ls"\}\]/)
  assert.match(flat[2].content, /^\[tool result\] file-a file-b/)
})

test('judge parse is tolerant of prose wrapping and clamps to 0-10', () => {
  assert.deepEqual(parseJudge('Sure! {"quality": 8, "gap": 3} there you go'), {
    quality: 8,
    gap: 3,
  })
  assert.deepEqual(parseJudge('{"quality": 99, "gap": -2}'), { quality: 10, gap: 0 })
  assert.equal(parseJudge('no json here'), null)
  assert.equal(parseJudge('{"quality": "high", "gap": 1}'), null)
  assert.equal(scoreValue({ quality: 8, gap: 3 }), 24)
})

test('judge prompt embeds clipped conversation and both answers', () => {
  const capture = {
    context: [{ role: 'user', content: 'q'.repeat(10_000) }],
    answer: 'teacher says',
  }
  const p = judgePrompt(capture, 'student says')
  assert.match(p, /\[TEACHER ANSWER\]\nteacher says/)
  assert.match(p, /\[STUDENT ANSWER\]\nstudent says/)
  assert.match(p, /\[clipped \d+ chars\]/)
  assert.equal(clip('short', 10), 'short')
})

test('chat content extraction covers message.content and text shapes', () => {
  assert.equal(extractChatContent({ choices: [{ message: { content: 'hi' } }] }), 'hi')
  assert.equal(extractChatContent({ choices: [{ text: 'raw' }] }), 'raw')
  assert.equal(extractChatContent({}), '')
})

test('distillate records speak the forge trainer contract', () => {
  const rec = {
    ts: 1000,
    digest: 'abcd',
    repo: { key: 'r1' },
    teacher: { club: 'sota', model: 'm' },
    context: [
      { role: 'user', content: 'do it' },
      { role: 'assistant', content: '', tool_calls: [{ name: 'shell', args: '{}' }] },
    ],
    answer: 'done well',
    score: { quality: 9, gap: 6 },
  }
  const d = distillateRecord(rec, '2026-W28')
  assert.equal(d.ts_ms, 1_000_000)
  assert.equal(d.club, 'sota')
  assert.equal(d.answer, 'done well')
  assert.equal(d.reward, 0.9, 'reward carries teacher quality so FORGE_MIN_REWARD filters on it')
  assert.equal(d.messages.length, 2)
  assert.deepEqual(d.messages[1].tool_calls, [{ name: 'shell', args: '{}' }])
  assert.equal(d.gap, 6)
})

test('golden selection ranks by value, floors low value, respects the byte cap', () => {
  const mk = (digest, quality, gap, ts) => ({
    ts,
    digest,
    teacher: { club: 'sota' },
    context: [{ role: 'user', content: 'x' }],
    answer: 'y',
    score: { quality, gap },
  })
  const records = [
    mk('low', 3, 2, 3), // value 6 < min 10 → floored
    mk('mid', 5, 4, 2), // value 20
    mk('top', 9, 8, 1), // value 72
    { ts: 4, digest: 'unscored', context: [], answer: 'z' },
  ]
  const oneRecord = JSON.stringify(distillateRecord(records[2], '2026-W28')).length + 1
  const selection = selectGolden(records, oneRecord + 10, {
    minValue: 10,
    week: '2026-W28',
  })
  assert.deepEqual(Object.keys(selection).sort(), [
    'bytes',
    'droppedCap',
    'droppedLowValue',
    'golden',
  ])
  const { golden, bytes, droppedLowValue, droppedCap } = selection
  assert.deepEqual(
    golden.map((r) => r.digest),
    ['top'],
    'highest value fills first; the cap excludes the rest',
  )
  assert.ok(bytes <= oneRecord + 10)
  assert.equal(droppedLowValue, 1)
  assert.equal(droppedCap, 1)
})

test('gap summary averages only scored records and counts the angels share', () => {
  const records = [
    { score: { quality: 8, gap: 4 } },
    { score: { quality: 6, gap: 2 } },
    { answer: 'never scored' },
  ]
  const row = gapSummary(records, '2026-W28', {
    evaporatedUnscored: 5,
    distillateBytes: 123,
    ts: 42,
  })
  assert.equal(row.captures, 3)
  assert.equal(row.scored, 2)
  assert.equal(row.mean_quality, 7)
  assert.equal(row.mean_gap, 3)
  assert.equal(row.evaporated_unscored, 5)
  assert.equal(row.distillate_bytes, 123)
})

test('an explicit student endpoint is authoritative; automatic endpoints dedupe', () => {
  assert.deepEqual(
    studentCandidates({
      ANGEL_STILL_STUDENT_URL: 'http://pin:9999/v1/',
      ANGEL_TURBO_URL: DEFAULT_STUDENT_ENDPOINTS[0],
    }),
    ['http://pin:9999/v1'],
  )
  const automatic = studentCandidates({ ANGEL_TURBO_URL: DEFAULT_STUDENT_ENDPOINTS[0] })
  assert.equal(automatic.filter((c) => c === DEFAULT_STUDENT_ENDPOINTS[0]).length, 1)
})

test('discovery takes the first live /models and reads the model id from it', async () => {
  const calls = []
  const fetchImpl = async (url) => {
    calls.push(url)
    if (url.startsWith('http://down')) throw new Error('unreachable')
    return {
      ok: true,
      json: async () => ({ data: [{ id: 'nex-live-1' }] }),
    }
  }
  const found = await discoverStudent(
    { ANGEL_STILL_STUDENT_URL: 'http://down:1/v1', ANGEL_TURBO_URL: 'http://up:2/v1' },
    { fetchImpl },
  )
  assert.equal(found, null, 'an unavailable pin must not send captured prompts elsewhere')
  assert.equal(calls.length, 1)
  assert.equal(calls[0], 'http://down:1/v1/models', 'pinned candidate probed first')
  // A pinned model name overrides whatever /models reports.
  const pinned = await discoverStudent(
    { ANGEL_STILL_STUDENT_URL: 'http://up:2/v1', ANGEL_STILL_STUDENT_MODEL: 'my-pin' },
    { fetchImpl },
  )
  assert.equal(pinned.model, 'my-pin')
  // Nothing serving anywhere → null, scoring skips.
  const none = await discoverStudent({}, { fetchImpl: async () => ({ ok: false }) })
  assert.equal(none, null)
})

test('annotation attaches scores by digest, passes garbage, never double-scores', () => {
  const lines = [
    JSON.stringify({ digest: 'a', answer: 'x' }),
    JSON.stringify({ digest: 'b', answer: 'y', score: { quality: 1, gap: 1 } }),
    'not json at all',
  ]
  const out = annotateLines(lines, { a: { quality: 7, gap: 5 }, b: { quality: 9, gap: 9 } })
  assert.equal(JSON.parse(out[0]).score.quality, 7)
  assert.equal(JSON.parse(out[1]).score.quality, 1, 'existing score wins')
  assert.equal(out[2], 'not json at all')
})
