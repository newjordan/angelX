// Tests for cut-tick (T3 of docs/plans/the-cut.md): the three measurement rules,
// the Conductor-lock refusal, and the observational-evidence ceiling.
//
// The three rules each moved the audited keep rate by 20+ points, so each one is
// asserted twice: that the row is excluded, AND that it is excluded for the RIGHT
// REASON. A future "simplification" that folds self-supersession into DISCARDED
// (or scores the sandbox, or calls an unreadable file a rejection) produces a
// prettier number that means nothing — these tests are the rail against that.

import { test } from 'node:test'
import assert from 'node:assert/strict'
import { mkdtempSync, writeFileSync, mkdirSync, readFileSync, existsSync } from 'node:fs'
import { tmpdir } from 'node:os'
import { join } from 'node:path'
import { execFileSync } from 'node:child_process'
import { fileURLToPath } from 'node:url'
import { dirname } from 'node:path'

import CausalGraph, { EDGE_TYPE, NODE_TYPE } from '../../lib/research/CausalGraph.js'
import { beliefProbability, scoreHypotheses } from '../../scripts/runtime/causal-loop.mjs'
import {
  KEPT,
  EDITED,
  DISCARDED,
  SELF_SUPERSEDED,
  SANDBOX,
  UNRESOLVED,
  UNSETTLED,
  MAX_OBS_CONFIDENCE,
  CUT_PROJECT,
  killed,
  isSettled,
  absPath,
  scoreManifest,
  summarize,
  sliceCorpus,
  cutHypothesisId,
  observationConfidence,
  ingestCutObservations,
  conductorHoldsLock,
  graphWriteAllowed,
  statusExport,
} from '../../scripts/runtime/cut-tick.mjs'

const SCRIPTS = join(dirname(fileURLToPath(import.meta.url)), '..', '..', 'scripts')
const HOME = '/home/user'
const NOW_SEC = 1_800_000_000
const NOW_ISO = new Date(NOW_SEC * 1000).toISOString()
const DAY = 86_400

/** One T1 manifest row (docs/plans/the-cut.md §T1). */
const entry = (over = {}) => ({
  v: 1,
  ts: NOW_SEC - 2 * DAY,
  session: 378423,
  seq: 1,
  turn_id: 't1',
  tool: 'str_replace',
  repo: { key: 'home-user-proj', root: '/home/user/proj', slug: 'user/proj' },
  path: 'src/lib.rs',
  authored: 'fn survivor() -> u32 { 41 + 1 }',
  authored_sha256: 'deadbeef',
  authored_bytes: 32,
  driver: 'sota-moa',
  model: 'gpt-5.2',
  hop: 7,
  ...over,
})

/** A fake working tree: abs path -> text (missing = unreadable). */
const tree = (files) => (abs) => (abs in files ? files[abs] : null)

const score = (entries, files, opts = {}) =>
  scoreManifest(entries, {
    nowSec: NOW_SEC,
    settleHours: 24,
    home: HOME,
    readFile: tree(files),
    ...opts,
  }).scored

// ─── plumbing ────────────────────────────────────────────────────────────────

test('kill switch: only ANGEL_CUT=0 disables', () => {
  assert.equal(killed({ ANGEL_CUT: '0' }), true)
  assert.equal(killed({ ANGEL_CUT: '1' }), false)
  assert.equal(killed({}), false)
})

test('absPath joins repo-relative paths and passes absolute ones through', () => {
  assert.equal(absPath(entry()), '/home/user/proj/src/lib.rs')
  assert.equal(absPath(entry({ path: '/etc/hosts' })), '/etc/hosts')
  assert.equal(absPath(entry({ repo: {}, path: 'x.rs' })), null)
})

test('isSettled honours the settle window', () => {
  assert.equal(isSettled({ ts: NOW_SEC - 25 * 3600 }, { nowSec: NOW_SEC, settleHours: 24 }), true)
  assert.equal(isSettled({ ts: NOW_SEC - 23 * 3600 }, { nowSec: NOW_SEC, settleHours: 24 }), false)
  assert.equal(isSettled({}, { nowSec: NOW_SEC, settleHours: 24 }), false)
})

// ─── the survival gradient ───────────────────────────────────────────────────

test('the tree decides: KEPT verbatim, EDITED when most lines survive, DISCARDED when gone', () => {
  const authored = ['fn a() { one() }', 'fn b() { two() }', 'fn c() { three() }'].join('\n')
  const rows = score(
    [
      entry({ path: 'kept.rs', authored, seq: 1 }),
      entry({ path: 'edited.rs', authored, seq: 2 }),
      entry({ path: 'gone.rs', authored, seq: 3 }),
    ],
    {
      '/home/user/proj/kept.rs': `// header\n${authored}\n`,
      // two of three substantive lines survive → 0.67 ≥ 0.4 → EDITED
      '/home/user/proj/edited.rs':
        'fn a() { one() }\nfn b() { two() }\nfn zzz() { rewritten() }\n',
      '/home/user/proj/gone.rs': 'fn totally() { different() }\n',
    },
  )
  assert.deepEqual(
    rows.map((r) => r.verdict),
    [KEPT, EDITED, DISCARDED],
  )
  const s = summarize(rows)
  assert.equal(s.judged, 3)
  assert.equal(s.keepRate, 1 / 3)
})

// ─── RULE 1 — self-supersession is not rejection ─────────────────────────────

test('RULE 1: an overwritten earlier write is SELF-SUPERSEDED, not DISCARDED', () => {
  const rows = score(
    [
      entry({ seq: 1, ts: NOW_SEC - 3 * DAY, authored: 'let draft = 1' }),
      entry({ seq: 2, ts: NOW_SEC - 2 * DAY, authored: 'let final_answer = 2' }),
    ],
    { '/home/user/proj/src/lib.rs': 'let final_answer = 2\n' },
  )

  const [first, second] = rows
  assert.equal(first.verdict, SELF_SUPERSEDED)
  assert.match(first.reason, /self-superseded — angel overwrote this itself/)
  assert.equal(first.judged, false, 'the intermediate must not reach the denominator')
  assert.notEqual(
    first.verdict,
    DISCARDED,
    'scoring iteration as rejection reports a ~12% keep rate',
  )

  // Only the final write to the path faced a human.
  assert.equal(second.verdict, KEPT)
  const s = summarize(rows)
  assert.equal(s.selfSuperseded, 1)
  assert.equal(s.judged, 1)
  assert.equal(s.keepRate, 1) // NOT 0.5 — the draft is not a rejection
})

test('RULE 1: supersession is per-path, and a later UNSETTLED write still supersedes', () => {
  const rows = score(
    [
      entry({ path: 'a.rs', seq: 1, ts: NOW_SEC - 3 * DAY, authored: 'draft a' }),
      // the newest write to a.rs is too young to judge; the draft is still angel's own overwrite
      entry({ path: 'a.rs', seq: 2, ts: NOW_SEC - 1 * 3600, authored: 'fresh a' }),
      entry({ path: 'b.rs', seq: 3, ts: NOW_SEC - 3 * DAY, authored: 'only b' }),
    ],
    { '/home/user/proj/a.rs': 'fresh a\n', '/home/user/proj/b.rs': 'only b\n' },
  )
  // scoreManifest sorts by (ts, seq): a.rs draft, b.rs, then the fresh a.rs write.
  assert.deepEqual(
    rows.map((r) => `${r.path}:${r.verdict}`),
    [`a.rs:${SELF_SUPERSEDED}`, `b.rs:${KEPT}`, `a.rs:${UNSETTLED}`],
  )
  assert.match(rows[2].reason, /unsettled — younger than the 24h settle window/)
  assert.equal(summarize(rows).judged, 1, 'only b.rs faced a verdict')
})

// ─── RULE 2 — angel's own sandbox does not count ─────────────────────────────

test('RULE 2: writes under ~/.angel0/workspace are SANDBOX — angel grading its own homework', () => {
  const sandboxRoot = `${HOME}/.angel0/workspace`
  const rows = score(
    [
      entry({
        repo: { key: 'angel-sandbox', root: sandboxRoot, slug: 'angel/workspace' },
        path: 'main.rs',
        authored: 'fn perfect() {}',
      }),
      entry({ seq: 2, path: 'real.rs', authored: 'fn real() {}' }),
    ],
    {
      [`${sandboxRoot}/main.rs`]: 'fn perfect() {}\n', // would score 100% KEPT
      '/home/user/proj/real.rs': 'fn nothing() {}\n',
    },
  )

  const [sandboxRow, realRow] = rows
  assert.equal(sandboxRow.verdict, SANDBOX)
  assert.match(sandboxRow.reason, /sandbox/)
  assert.match(sandboxRow.reason, /nobody judged it/)
  assert.equal(sandboxRow.judged, false)
  assert.notEqual(sandboxRow.verdict, KEPT, 'the sandbox scored 100% because nobody ever looked')

  assert.equal(realRow.verdict, DISCARDED)
  const s = summarize(rows)
  assert.equal(s.sandbox, 1)
  assert.equal(s.judged, 1)
  assert.equal(s.keepRate, 0) // NOT 0.5 — the sandbox never inflates the number
  // and it never reaches a slice denominator either
  assert.equal(sliceCorpus(rows).find((x) => x.kind === 'driver').samples, 1)
})

test('RULE 2: an absolute path inside the sandbox is caught even with a foreign repo root', () => {
  const rows = score(
    [
      entry({
        repo: { key: 'k', root: '/home/user/proj' },
        path: `${HOME}/.angel0/workspace/x.rs`,
      }),
    ],
    { [`${HOME}/.angel0/workspace/x.rs`]: 'fn survivor() -> u32 { 41 + 1 }' },
  )
  assert.equal(rows[0].verdict, SANDBOX)
})

// ─── RULE 3 — unresolvable is not discarded ──────────────────────────────────

test('RULE 3: a file we cannot read is UNRESOLVED and leaves the denominator', () => {
  const rows = score(
    [
      entry({ path: 'vanished.rs', seq: 1, authored: 'fn gone_from_disk() {}' }),
      entry({ path: 'here.rs', seq: 2, authored: 'fn here() {}' }),
    ],
    { '/home/user/proj/here.rs': 'fn here() {}\n' }, // vanished.rs is unreadable
  )

  const [missing, present] = rows
  assert.equal(missing.verdict, UNRESOLVED)
  assert.match(missing.reason, /unresolvable — the file is not readable/)
  assert.equal(missing.judged, false)
  assert.notEqual(
    missing.verdict,
    DISCARDED,
    '"I cannot find the file" is not "you threw it away" — it may be renamed or not checked out',
  )

  const s = summarize(rows)
  assert.equal(s.unresolved, 1)
  assert.equal(s.discarded, 0, 'UNRESOLVED must never be folded into DISCARDED')
  assert.equal(s.judged, 1) // excluded from the denominator, not counted against angel
  assert.equal(s.keepRate, 1)
  assert.equal(present.verdict, KEPT)
})

// ─── the graph fold: prioritize, never conclude ──────────────────────────────

test('evidence lands at ≤0.4 confidence with NO signed edges and never concludes', () => {
  const graph = new CausalGraph()
  const rows = score(
    [
      entry({ path: 'a.rs', seq: 1, authored: 'fn a() {}' }),
      entry({ path: 'b.rs', seq: 2, authored: 'fn b() {}' }),
      entry({ path: 'c.rs', seq: 3, authored: 'fn c() {}', driver: 'swarm' }),
    ],
    {
      '/home/user/proj/a.rs': 'fn a() {}\n',
      '/home/user/proj/b.rs': 'fn nope() {}\n',
      '/home/user/proj/c.rs': 'fn c() {}\n',
    },
  )
  const { minted, folded, experimentId } = ingestCutObservations(graph, rows, { now: NOW_ISO })

  // one hypothesis per slice value: 2 drivers + 1 model + 1 repo
  assert.equal(minted.length, 4)
  assert.ok(minted.includes(cutHypothesisId('driver', 'sota-moa')))
  assert.ok(minted.includes(cutHypothesisId('driver', 'swarm')))
  assert.equal(folded.length, 4)

  // NO signed edges — ever. Only unsigned `tests` edges from one shared experiment.
  assert.equal(graph.edgesOfType(EDGE_TYPE.SUPPORTS).length, 0)
  assert.equal(graph.edgesOfType(EDGE_TYPE.CONTRADICTS).length, 0)
  const tests = graph.edgesOfType(EDGE_TYPE.TESTS)
  assert.equal(tests.length, 4)
  for (const e of tests) {
    assert.ok(
      e.confidence <= MAX_OBS_CONFIDENCE,
      `confidence ${e.confidence} exceeds the 0.4 ceiling`,
    )
    assert.equal(e.src, experimentId)
    assert.match(e.label, /observational, confounded/)
  }
  assert.equal(graph.nodesOfType(NODE_TYPE.EXPERIMENT).length, 1)

  // Beliefs do not move, and nothing concludes.
  for (const id of minted) {
    const node = graph.getNode(id)
    assert.equal(beliefProbability(graph, id), 0.5, 'observational evidence must not move a belief')
    assert.equal(node.status, 'testing')
    assert.equal(node.concludedAt, null)
    assert.equal(node.outcome, 'neutral')
    assert.equal(node.lastVerifiedAt, NOW_ISO, 'perpetual: stamp lastVerifiedAt after every fold')
    assert.equal(node.projectId, CUT_PROJECT.id)
    assert.equal(node.confounded, true)
  }

  // Perpetual facts stay in the EIG scorer (a concluded node silently leaves it).
  const scored = scoreHypotheses(graph)
  assert.equal(scored.filter((h) => h.id.startsWith('cut_')).length, 4)

  // Cumulative observation, batch visible.
  const drv = graph.getNode(cutHypothesisId('driver', 'sota-moa'))
  assert.deepEqual(
    { samples: drv.observed.samples, kept: drv.observed.kept, keepRate: drv.observed.keepRate },
    { samples: 2, kept: 1, keepRate: 0.5 },
  )
})

test('observationConfidence grows with evidence but is hard-capped at the 0.4 ceiling', () => {
  assert.equal(observationConfidence(1), 0.15)
  assert.equal(observationConfidence(4), 0.3)
  assert.equal(observationConfidence(6), MAX_OBS_CONFIDENCE)
  assert.equal(observationConfidence(10_000), MAX_OBS_CONFIDENCE)
  // Below updateBeliefs' 0.6 conclusion threshold by construction.
  assert.ok(MAX_OBS_CONFIDENCE < 0.6)
})

test('a second fold accumulates onto the same node without minting or concluding', () => {
  const graph = new CausalGraph()
  const batch = (path, text) =>
    score([entry({ path, authored: `fn ${path}() {}` })], { [`/home/user/proj/${path}`]: text })

  ingestCutObservations(graph, batch('a', 'fn a() {}\n'), { now: NOW_ISO })
  const later = new Date((NOW_SEC + 3600) * 1000).toISOString()
  const { minted } = ingestCutObservations(graph, batch('b', 'gone\n'), { now: later })

  assert.deepEqual(minted, [], 'existing slices are refreshed, not duplicated')
  const drv = graph.getNode(cutHypothesisId('driver', 'sota-moa'))
  assert.equal(drv.observed.samples, 2)
  assert.equal(drv.observed.kept, 1)
  assert.equal(drv.observed.keepRate, 0.5)
  assert.equal(drv.lastVerifiedAt, later)
  assert.equal(graph.edgesOfType(EDGE_TYPE.SUPPORTS).length, 0)
  assert.equal(graph.edgesOfType(EDGE_TYPE.CONTRADICTS).length, 0)
})

test('excluded rows never reach the graph — the three rules hold through the fold', () => {
  const graph = new CausalGraph()
  const rows = score(
    [
      entry({ path: 'x.rs', seq: 1, ts: NOW_SEC - 3 * DAY, authored: 'draft', driver: 'ghost' }),
      entry({ path: 'x.rs', seq: 2, ts: NOW_SEC - 2 * DAY, authored: 'final', driver: 'ghost' }),
      entry({
        repo: { key: 'sandbox', root: `${HOME}/.angel0/workspace` },
        path: 's.rs',
        seq: 3,
        driver: 'ghost',
      }),
      entry({ path: 'missing.rs', seq: 4, driver: 'ghost' }),
    ],
    { '/home/user/proj/x.rs': 'final\n' },
  )
  ingestCutObservations(graph, rows, { now: NOW_ISO })
  const drv = graph.getNode(cutHypothesisId('driver', 'ghost'))
  assert.equal(drv.observed.samples, 1, 'only the one judged write counts')
  assert.equal(drv.observed.keepRate, 1)
})

// ─── the Conductor lock ──────────────────────────────────────────────────────

test('conductorHoldsLock: fresh lock held, stale lock past the 60-min TTL is not', () => {
  const nowMs = NOW_SEC * 1000
  assert.equal(conductorHoldsLock({ pid: 1, ts: nowMs - 60_000 }, { nowMs }), true)
  assert.equal(conductorHoldsLock({ pid: 1, ts: nowMs - 61 * 60_000 }, { nowMs }), false)
  assert.equal(conductorHoldsLock(null, { nowMs }), false)
  assert.equal(conductorHoldsLock({ pid: 1 }, { nowMs }), false)
})

test('graphWriteAllowed refuses outside the Conductor lock (the graph has no locking)', () => {
  const nowMs = NOW_SEC * 1000
  const held = graphWriteAllowed({ conductorLock: { pid: 9, ts: nowMs - 1000 }, nowMs })
  assert.equal(held.allowed, true)

  for (const lock of [null, undefined, {}, { pid: 9, ts: nowMs - 2 * 60 * 60_000 }]) {
    const gate = graphWriteAllowed({ conductorLock: lock, nowMs })
    assert.equal(gate.allowed, false)
    assert.match(gate.reason, /refusing to write the causal graph outside the Conductor lock/)
  }
})

test('the tick refuses to write the graph outside the Conductor lock (end to end)', () => {
  const dir = mkdtempSync(join(tmpdir(), 'cut-tick-'))
  const stateDir = join(dir, 'cut')
  const conductorDir = join(dir, 'conductor')
  const repoRoot = join(dir, 'proj')
  mkdirSync(stateDir, { recursive: true })
  mkdirSync(conductorDir, { recursive: true })
  mkdirSync(repoRoot, { recursive: true })
  writeFileSync(join(repoRoot, 'kept.rs'), 'fn kept() {}\n')

  const manifest = join(stateDir, 'authored-20260709.jsonl')
  const row = {
    ...entry({ path: 'kept.rs', authored: 'fn kept() {}' }),
    ts: Math.floor(Date.now() / 1000) - 3 * DAY,
    repo: { key: 'tmp-proj', root: repoRoot, slug: 'tmp/proj' },
  }
  writeFileSync(manifest, `${JSON.stringify(row)}\n`)

  const graphPath = join(dir, 'graph.json')
  writeFileSync(graphPath, JSON.stringify(new CausalGraph().serialize()))
  const before = readFileSync(graphPath, 'utf8')

  const run = () =>
    execFileSync(
      'node',
      [
        join(SCRIPTS, 'cut-tick.mjs'),
        '--force',
        '--state-dir',
        stateDir,
        '--conductor-dir',
        conductorDir,
        '--graph',
        graphPath,
        '--ledger',
        join(dir, 'no-ledger.jsonl'),
      ],
      { encoding: 'utf8', env: { ...process.env, ANGEL_CUT: '1' } },
    )

  // 1. No Conductor lock → refuse to write the graph, but still score + report.
  const refused = run()
  assert.match(refused, /refusing to write the causal graph outside the Conductor lock/)
  assert.equal(readFileSync(graphPath, 'utf8'), before, 'the graph must be byte-identical')
  assert.ok(existsSync(join(stateDir, 'status.json')), 'status is written even when refusing')
  const hb = JSON.parse(readFileSync(join(stateDir, 'heartbeat.json'), 'utf8'))
  assert.equal(hb.action, 'skip') // heartbeat on EVERY outcome, skips included
  assert.match(hb.reason, /Conductor lock/)
  assert.match(
    readFileSync(manifest, 'utf8'),
    /^(?!.*"cut":)/,
    'a refused fold must leave the manifest unstamped so the evidence is not lost',
  )
  assert.ok(!existsSync(join(stateDir, 'lock')), 'the lock is released in finally')

  // 2. Conductor lock held → the same tick folds and writes.
  writeFileSync(join(conductorDir, 'lock'), JSON.stringify({ pid: 4242, ts: Date.now() }))
  const ran = run()
  assert.match(ran, /folded 3 slice\(s\)/)
  const graph = CausalGraph.deserialize(JSON.parse(readFileSync(graphPath, 'utf8')))
  assert.equal(graph.edgesOfType(EDGE_TYPE.SUPPORTS).length, 0)
  assert.equal(graph.edgesOfType(EDGE_TYPE.CONTRADICTS).length, 0)
  assert.equal(graph.edgesOfType(EDGE_TYPE.TESTS).length, 3)
  assert.ok(graph.hasNode(cutHypothesisId('driver', 'sota-moa')))
  assert.equal(graph.getNode(cutHypothesisId('driver', 'sota-moa')).observed.keepRate, 1)

  // The folded row is stamped, so a second pass cannot double-count it.
  const stamped = JSON.parse(readFileSync(manifest, 'utf8').trim())
  assert.equal(stamped.cut.verdict, KEPT)
  const again = run()
  assert.match(again, /no settled, unfolded writes to score/)
  assert.equal(
    CausalGraph.deserialize(JSON.parse(readFileSync(graphPath, 'utf8'))).edgeCount(),
    3,
    'a re-run must not re-fold the same evidence',
  )
})

test('statusExport keeps the exclusions and the density on screen for /cut', () => {
  const rows = score(
    [
      entry({ path: 'a.rs', seq: 1, authored: 'fn a() {}' }),
      entry({ path: 'missing.rs', seq: 2, authored: 'fn m() {}' }),
    ],
    { '/home/user/proj/a.rs': 'fn a() {}\n' },
  )
  const status = statusExport(summarize(rows), sliceCorpus(rows), { now: NOW_ISO })
  assert.equal(status.v, 1)
  assert.equal(status.summary.judged, 1)
  assert.equal(status.summary.unresolved, 1)
  assert.equal(status.summary.keepRate, 1)
  assert.deepEqual(JSON.parse(JSON.stringify(status)), status)
})

test('machine execution counts distinguish shared labels on separate authored files', () => {
  const first = {
    machine: { exit: 0, timed_out: false, verification_id: 'fixture:1', shared: false },
  }
  const second = { machine: { ...first.machine, shared: true } }
  const summary = summarize([first, second])
  assert.equal(summary.machineVerdicts, 2)
  assert.equal(summary.machinePass, 2)
  assert.equal(summary.machineExecutions, 1)
  assert.equal(summary.sharedMachineRows, 1)
})

test('separate process namespaces retain executions after PID and sequence reuse', () => {
  const rows = ['run-a:123:7', 'run-b:123:7'].flatMap((id) => [
    { session: 123, seq: 7, machine: { exit: 0, verification_id: id, shared: false } },
    { session: 123, seq: 8, machine: { exit: 0, verification_id: id, shared: true } },
  ])
  const summary = summarize(rows)
  assert.equal(summary.machineExecutions, 2)
  assert.equal(summary.sharedMachineRows, 2)
  assert.equal(summary.machineVerdicts, 4)
})
