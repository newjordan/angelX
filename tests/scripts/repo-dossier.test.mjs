// Tests for repo-dossier: D1 ledger events → per-repo facts with beliefs.
// Unit cases use a fresh graph for determinism; the coexistence case seeds
// research + self + config + dossier domains into one graph and proves the
// shared scorer keeps them independent (the same guarantee self-causal and
// config-causal each established for their domain).

import { test } from 'node:test'
import assert from 'node:assert/strict'
import { mkdtempSync, mkdirSync, readFileSync, rmSync, writeFileSync, existsSync } from 'node:fs'
import { tmpdir } from 'node:os'
import { join } from 'node:path'
import { fileURLToPath } from 'node:url'
import { spawnSync } from 'node:child_process'

import CausalGraph, { NODE_TYPE, EDGE_TYPE } from '../../lib/research/CausalGraph.js'
import {
  classifyCommand,
  normalizeCommand,
  mineRepoFacts,
  ingestRepoFacts,
  dossierFacts,
  factBelief,
  compileDossier,
  boundedDossierText,
  DOSSIER_STORE_BYTES,
  DOSSIER_STORE_FACTS,
  proposeDossierProbe,
  cutSample,
  DOSSIER_PROJECT,
  RITUAL_MIN_RUNS,
  TRAP_MIN_FAILS,
  dossierPaths,
} from '../../scripts/runtime/repo-dossier.mjs'

test('dossier paths use the native cockpit stores and respect explicit overrides', () => {
  assert.deepEqual(dossierPaths([], {}, '/tmp/dossier-user'), {
    graphPath: '/tmp/dossier-user/.angelX/dossier/graph.json',
    ledgerPath: '/tmp/dossier-user/.angelX/experience/ledger.jsonl',
    outDir: '/tmp/dossier-user/.angelX/dossier',
    cutDir: '/tmp/dossier-user/.angelX/cut',
    onlyRepo: undefined,
  })
  const env = {
    ANGEL_DOSSIER_DIR: '/tmp/configured-dossier',
    ANGEL_EXPERIENCE_LOG: '/tmp/configured-ledger.jsonl',
    ANGEL_CUT_DIR: '/tmp/configured-cut',
  }
  assert.equal(dossierPaths([], env).graphPath, '/tmp/configured-dossier/graph.json')
  const explicit = dossierPaths(
    ['--out', '/tmp/explicit', '--ledger', '/tmp/ledger', '--cut', '/tmp/cut'],
    env,
  )
  assert.equal(explicit.outDir, '/tmp/explicit')
  assert.equal(explicit.ledgerPath, '/tmp/ledger')
  assert.equal(explicit.cutDir, '/tmp/cut')
})

test('refresh publishes ledger facts in one invocation without a browser data tree', (t) => {
  const root = mkdtempSync(join(tmpdir(), 'angel-dossier-refresh-'))
  t.after(() => rmSync(root, { recursive: true, force: true }))
  const ledger = join(root, 'ledger.jsonl')
  const out = join(root, 'dossier')
  const cut = join(root, 'cut')
  mkdirSync(cut)
  const now = Math.floor(Date.now() / 1000)
  writeFileSync(
    ledger,
    [1, 2, 3]
      .map((session) => JSON.stringify(cmdV3('cargo check', 'pass', { session, ts: now })))
      .join('\n') + '\n',
  )
  const result = spawnSync(
    process.execPath,
    [
      fileURLToPath(new URL('../../scripts/runtime/repo-dossier.mjs', import.meta.url)),
      '--refresh',
      '--ledger',
      ledger,
      '--cut',
      cut,
      '--out',
      out,
    ],
    { cwd: root, encoding: 'utf8', timeout: 10_000 },
  )
  assert.equal(result.status, 0, result.stderr)
  assert.ok(existsSync(join(out, 'graph.json')))
  const artifact = JSON.parse(readFileSync(join(out, `${KEY}.json`), 'utf8'))
  assert.ok(artifact.facts.some((fact) => fact.text === 'cargo check'))
  assert.equal(existsSync(join(root, 'public')), false)
})

test('dossier compiler rejects repository keys that escape its output directory', (t) => {
  const root = mkdtempSync(join(tmpdir(), 'angel-dossier-invalid-key-'))
  t.after(() => rmSync(root, { recursive: true, force: true }))
  const result = spawnSync(
    process.execPath,
    [
      fileURLToPath(new URL('../../scripts/runtime/repo-dossier.mjs', import.meta.url)),
      '--compile',
      '--repo',
      '../escape',
      '--out',
      join(root, 'dossier'),
    ],
    { cwd: root, encoding: 'utf8', timeout: 10_000 },
  )
  assert.notEqual(result.status, 0)
  assert.match(result.stderr, /Invalid dossier repository key/u)
  assert.equal(existsSync(join(root, 'escape.json')), false)
})

const NOW = '2026-07-06T00:00:00.000Z'
const NOW_TS = Math.floor(Date.parse(NOW) / 1000)
const KEY = 'home-u-proj-0011223344556677'
const KEY2 = 'home-u-other-8899aabbccddeeff'

// One D1 cmd-event line, as experience.rs writes it.
const cmd = (
  text,
  exit,
  { key = KEY, session = 1, ts = NOW_TS, dur = 100, timedOut = false } = {},
) => ({
  kind: 'event',
  v: 1,
  ts,
  session,
  seq: 0,
  event: 'cmd',
  repo: { key, root: '/home/u/proj', slug: 'u/proj' },
  cmd: { text, exit, timed_out: timedOut, dur_ms: dur, tool: 'shell', bytes_out: 10 },
})

// One current (experience schema v3) row, including the verdict/provenance
// contract the Rust writer places on the ledger.
const cmdV3 = (
  text,
  verdict,
  {
    key = KEY,
    session = 1,
    ts = NOW_TS,
    dur = 100,
    exit = verdict === 'pass' ? 0 : verdict === 'fail' ? 1 : null,
    pipefail = true,
    independent = true,
  } = {},
) => ({
  kind: 'event',
  v: 3,
  ts,
  session,
  seq: 0,
  event: 'cmd',
  repo: { key, root: '/home/u/proj', slug: 'u/proj' },
  cmd: {
    text,
    exit,
    verdict,
    verdict_reason: verdict === 'no_verdict' ? 'signal' : null,
    shell: 'bash',
    pipefail,
    source: independent ? 'agent' : 'dossier',
    independent,
    timed_out: verdict === 'no_verdict',
    dur_ms: dur,
    tool: 'shell',
    bytes_out: 10,
  },
})

const turn = (ts, stop, { key = KEY, driver = 'gemma', ok = true } = {}) => ({
  kind: 'turn',
  v: 1,
  ts,
  session: 9,
  seq: 0,
  path: 'single',
  driver,
  repo: { key, root: '/home/u/proj', slug: 'u/proj' },
  cfg: { hash: 'h', knobs: {} },
  outcome: { ok, stop, latency_ms: 1, hops: 1, tokens: {}, counters: {}, failovers: [] },
})

// A repo history: a healthy test ritual, a recurring trap, a one-off command.
const ROWS = [
  cmd('cargo test -p cockpit', 0, { session: 1 }),
  cmd('cargo test -p cockpit', 0, { session: 2 }),
  cmd('cargo test -p cockpit', 101, { session: 2 }),
  cmd('cargo test', 0, { session: 3 }),
  cmd('npm test', 1, { session: 1 }),
  cmd('npm test', 1, { session: 2 }),
  cmd('npm test', 1, { session: 3 }),
  cmd('echo one-off', 0, { session: 1 }),
  turn(NOW_TS - 100, 'answer'),
  turn(NOW_TS - 50, 'max_hops', { ok: false }),
]

test('classifyCommand buckets by leading tokens', () => {
  assert.equal(classifyCommand('cargo test -p cockpit'), 'test')
  assert.equal(classifyCommand('npm run test -- --watch'), 'test')
  assert.equal(classifyCommand('cargo build --release'), 'build')
  assert.equal(classifyCommand('cargo clippy -- -D warnings'), 'lint')
  assert.equal(classifyCommand('npm run dev'), 'run')
  assert.equal(classifyCommand('echo hello'), null)
  assert.equal(classifyCommand('ls sidecar/'), null)
})

test('normalizeCommand strips the chdir agents always prefix, and keeps the command', () => {
  const repo = { root: '/home/u/proj' }
  // The shape that made 99.5% of the live ledger unclassifiable.
  assert.deepEqual(normalizeCommand('cd /home/u/proj/cockpit && cargo check', repo), {
    command: 'cargo check',
    attributable: true,
  })
  // …and its canonical text is the COMMAND, not a path that may not exist
  // tomorrow (a worktree) — this text is handed to The Cut as a verify command.
  assert.equal(
    classifyCommand(normalizeCommand('cd /home/u/proj && cargo test -p x', repo).command),
    'test',
  )
  // Chained chdirs, quoted targets, `$(pwd)`, relative — all still in-repo.
  assert.equal(
    normalizeCommand('cd /home/u/proj; cd cockpit && npm test', repo).command,
    'npm test',
  )
  assert.equal(normalizeCommand('cd "$(pwd)" && npm test', repo).command, 'npm test')
  // A plain command is untouched.
  assert.deepEqual(normalizeCommand('cargo test', repo), {
    command: 'cargo test',
    attributable: true,
  })
})

test("a pipeline's exit belongs to its last stage, so the command gets NO verdict", () => {
  const repo = { root: '/home/u/proj' }
  // `sh -c` has no pipefail: `cargo check … | tail` exits 0 even when the build
  // fails. Recurrence is real; the verdict is not.
  assert.deepEqual(normalizeCommand('cd /home/u/proj && cargo check 2>&1 | tail -20', repo), {
    command: 'cargo check',
    attributable: false,
  })
  assert.deepEqual(normalizeCommand('cargo test; echo done', repo), {
    command: 'cargo test',
    attributable: false,
  })
  // A bare redirect is not another stage — that exit IS cargo's.
  assert.deepEqual(normalizeCommand('cargo test 2>&1', repo), {
    command: 'cargo test',
    attributable: true,
  })
})

test("a chdir into ANOTHER repo is that repo's evidence, never this one's", () => {
  // Run from gpug, but the command builds angelX: it must not become a gpug fact.
  const gpug = { root: '/home/user/gpug' }
  assert.equal(normalizeCommand('cd /home/user/angelX/cockpit && cargo check', gpug), null)
  // The worktree case: the row's repo is the MAIN checkout, the work happened in
  // a linked worktree (`cwd`), and that is still this repo's evidence.
  const wt = { root: '/home/user/angelX', cwd: '/tmp/cut-forge/item-3' }
  assert.equal(
    normalizeCommand('cd /tmp/cut-forge/item-3/cockpit && cargo check', wt).command,
    'cargo check',
  )
})

test('unverifiable runs prove recurrence but never move belief', () => {
  // Three piped `cargo check`s across three sessions: a real ritual candidate
  // with zero observed exit statuses.
  const piped = [1, 2, 3].map((s) =>
    cmd('cd /home/u/proj/cockpit && cargo check 2>&1 | tail -20', 0, { session: s }),
  )
  const rec = mineRepoFacts(piped)[KEY]
  assert.equal(rec.rituals.length, 1)
  const r = rec.rituals[0]
  assert.equal(r.canonical, 'cargo check', 'the fact is the command, not the plumbing')
  assert.equal(r.runs, 3, 'recurrence is real')
  assert.equal(r.verdicts, 0, 'no exit status was ever this command’s')
  assert.equal(r.passes, 0)

  // Belief must stay at exactly 0.5 — NOT 0.5+0.2 (a fabricated pass rate) and
  // NOT 0.5-0.2 (which "0 of 3 passed" would otherwise imply).
  const g = new CausalGraph()
  ingestRepoFacts(g, KEY, rec, { now: NOW })
  const node = dossierFacts(g, KEY)[0]
  assert.equal(factBelief(g, node, { now: NOW }).belief, 0.5)

  // …so it is withheld from the injected block, and says why.
  const artifact = compileDossier(g, KEY, { now: NOW })
  assert.equal(artifact.facts[0].verdicts, 0)
  assert.match(artifact.facts[0].evidence, /no attributable verdict/)
})

test('v3 pipefail passes cross the Rust-to-miner boundary and mint a real fact', () => {
  const rows = [1, 2, 3].map((session) =>
    cmdV3('cd /home/u/proj/cockpit && cargo check 2>&1 | tail -20', 'pass', { session }),
  )
  const rec = mineRepoFacts(rows)[KEY]
  assert.equal(rec.rituals.length, 1)
  const build = rec.rituals[0]
  assert.equal(build.canonical, 'cargo check')
  assert.equal(build.runs, 3)
  assert.equal(build.verdicts, 3, 'pipefail proves every stage passed')
  assert.equal(build.passes, 3)

  const g = new CausalGraph()
  ingestRepoFacts(g, KEY, rec, { now: NOW })
  assert.ok(factBelief(g, dossierFacts(g, KEY)[0], { now: NOW }).belief > 0.5)
})

test('v3 pipeline failures stay unattributed, and dossier echoes cannot self-mint', () => {
  // Pipefail says some stage failed, not which one. Do not turn a broken `tail`
  // into a claim that cargo failed.
  const failed = [1, 2, 3].map((session) =>
    cmdV3('cargo check | tail -20', 'fail', { session, exit: 1 }),
  )
  const failureFact = mineRepoFacts(failed)[KEY].rituals[0]
  assert.equal(failureFact.verdicts, 0)
  assert.equal(failureFact.passes, 0)

  // Even trustworthy passes are inert when the dossier put the command into
  // the prompt. This is the ledger-side mirror of machine.source=dossier.
  const echoes = [1, 2, 3].map((session) =>
    cmdV3('cargo check', 'pass', { session, independent: false }),
  )
  assert.equal(mineRepoFacts(echoes)[KEY].rituals.length, 0)
})

test('the recurrence floor is enforced on the fact minted, not on the class pool', () => {
  // Three DIFFERENT one-off `python3 -c …` scripts across three sessions: the
  // `run` class pool clears 3 runs / 2 sessions, but no single command recurs.
  // The canonical form would carry ONE run — a belief out of thin evidence.
  // (This is verbatim what the live gpug ledger minted before the fix.)
  const oneOffs = ['python3 -c "print(1)"', 'python3 -c "print(2)"', 'python3 -c "print(3)"'].map(
    (t, i) => cmd(t, 0, { session: i + 1 }),
  )
  assert.equal(mineRepoFacts(oneOffs)[KEY].rituals.length, 0)

  // The same class, but now one command actually recurs → that one is the fact.
  const recurring = [
    ...oneOffs,
    cmd('python3 -m app', 0, { session: 1 }),
    cmd('python3 -m app', 0, { session: 2 }),
    cmd('python3 -m app', 0, { session: 3 }),
  ]
  const rituals = mineRepoFacts(recurring)[KEY].rituals
  assert.equal(rituals.length, 1)
  assert.equal(rituals[0].canonical, 'python3 -m app')
  assert.equal(rituals[0].runs, 3)
})

test('mining mints rituals and traps past their thresholds, with the newest turn as thread', () => {
  const mined = mineRepoFacts(ROWS)
  const rec = mined[KEY]
  assert.ok(rec)

  // `npm test` is in the same class, but only helps SELECT the canonical form
  // (most frequent passing text); the ritual's evidence is the canonical
  // command's own runs, so npm's failures don't dilute cargo's record.
  assert.equal(rec.rituals.length, 1)
  const ritual = rec.rituals[0]
  assert.equal(ritual.class, 'test')
  assert.equal(ritual.canonical, 'cargo test -p cockpit')
  assert.equal(ritual.runs, 3) // the canonical text's own runs
  assert.equal(ritual.passes, 2)
  assert.ok(ritual.sessions >= 2)

  // The exact `npm test` text never passes → trap.
  assert.equal(rec.traps.length, 1)
  assert.equal(rec.traps[0].text, 'npm test')
  assert.equal(rec.traps[0].fails, TRAP_MIN_FAILS)

  // One-off never becomes a fact (recurrence floor).
  assert.ok(!JSON.stringify(rec.rituals).includes('one-off'))

  // Thread = newest turn.
  assert.equal(rec.thread.stop, 'max_hops')
  assert.equal(rec.thread.ok, false)
})

test('below-threshold commands mint nothing', () => {
  const few = [cmd('cargo test', 0, { session: 1 }), cmd('cargo test', 0, { session: 1 })]
  const mined = mineRepoFacts(few)
  assert.equal(mined[KEY].rituals.length, 0, `needs ${RITUAL_MIN_RUNS} runs across 2 sessions`)
  assert.equal(mined[KEY].traps.length, 0)
})

test('events without a repo stamp or with unknown kinds are ignored', () => {
  const mined = mineRepoFacts([
    { kind: 'event', event: 'cmd', cmd: { text: 'x', exit: 0 } }, // no repo
    { kind: 'bench', suite: 'control-v1' },
    null,
  ])
  assert.deepEqual(mined, {})
})

test('ingest is idempotent and keeps repos separate', () => {
  const g = new CausalGraph()
  const mined = mineRepoFacts([
    ...ROWS,
    cmd('cargo test', 0, { key: KEY2, session: 1 }),
    cmd('cargo test', 0, { key: KEY2, session: 2 }),
    cmd('cargo test', 1, { key: KEY2, session: 3 }),
  ])

  const first = ingestRepoFacts(g, KEY, mined[KEY], { now: NOW })
  assert.equal(first.added.nodes, 2) // ritual + trap
  const again = ingestRepoFacts(g, KEY, mined[KEY], { now: NOW })
  assert.equal(again.added.nodes, 0)
  assert.equal(again.added.updated, 2)

  ingestRepoFacts(g, KEY2, mined[KEY2], { now: NOW })
  assert.equal(dossierFacts(g).length, 3)
  assert.equal(dossierFacts(g, KEY).length, 2)
  assert.equal(dossierFacts(g, KEY2).length, 1)

  for (const n of dossierFacts(g)) {
    assert.equal(n.type, NODE_TYPE.HYPOTHESIS)
    assert.equal(n.projectId, DOSSIER_PROJECT.id)
    assert.equal(n.status, 'testing')
    assert.ok(n.id.includes(n.repoKey), 'repo key rides verbatim in the id')
  }
})

test('belief: passing ritual > 0.5, trap ~confidently true, probes move it, age decays it', () => {
  const g = new CausalGraph()
  const mined = mineRepoFacts(ROWS)
  ingestRepoFacts(g, KEY, mined[KEY], { now: NOW })
  const [ritual, trap] = [
    dossierFacts(g, KEY).find((n) => n.factKind === 'ritual'),
    dossierFacts(g, KEY).find((n) => n.factKind === 'trap'),
  ]

  // Observational only: mining-capped, on the right side of 0.5.
  const rb = factBelief(g, ritual, { now: NOW }).belief
  assert.ok(rb > 0.5 && rb <= 0.7, `ritual observational belief capped (${rb})`)
  const tb = factBelief(g, trap, { now: NOW }).belief
  assert.ok(tb > rb, `an always-failing trap (${tb}) is more certain than a 2/3 ritual (${rb})`)

  // A probe SUPPORTS edge raises belief past the observational cap.
  g.addNode({ id: 'exp_probe_1', type: NODE_TYPE.EXPERIMENT, label: 'probe' })
  g.addEdge({ src: 'exp_probe_1', dst: ritual.id, type: EDGE_TYPE.SUPPORTS, confidence: 0.8 })
  g.updateNode(ritual.id, { lastVerifiedAt: NOW })
  const verified = factBelief(g, g.getNode(ritual.id), { now: NOW }).belief
  assert.ok(verified > 0.75, `verified ritual belief rises (${verified})`)

  // Two half-lives later with no re-verification, the same fact has decayed
  // most of the way back to neutral.
  const later = new Date(Date.parse(NOW) + 28 * 86_400_000).toISOString()
  const decayed = factBelief(g, g.getNode(ritual.id), { now: later }).belief
  assert.ok(decayed < 0.5 + (verified - 0.5) / 3, `decays toward 0.5 (${verified} → ${decayed})`)
  assert.ok(decayed > 0.5, 'but never crosses neutral on its own')
})

test('compile emits the injection artifact: rituals before traps, evidence strings, thread', () => {
  const g = new CausalGraph()
  const mined = mineRepoFacts(ROWS)
  ingestRepoFacts(g, KEY, mined[KEY], { now: NOW })

  const artifact = compileDossier(g, KEY, { now: NOW, thread: mined[KEY].thread })
  assert.equal(artifact.v, 1)
  assert.equal(artifact.repo.key, KEY)
  assert.equal(artifact.repo.slug, 'u/proj')
  assert.equal(artifact.facts.length, 2)
  assert.equal(artifact.facts[0].kind, 'ritual')
  assert.equal(artifact.facts[1].kind, 'trap')
  assert.match(artifact.facts[0].evidence, /3 runs, 2 pass/)
  assert.ok(artifact.facts.every((f) => f.belief > 0 && f.belief < 1))
  assert.equal(artifact.thread.stop, 'max_hops')
  // Round-trips as JSON (it's written to disk verbatim).
  assert.deepEqual(JSON.parse(JSON.stringify(artifact)), artifact)
})

test('probe queue ranks stale/uncertain facts first and scopes by repo', () => {
  const g = new CausalGraph()
  const mined = mineRepoFacts(ROWS)
  ingestRepoFacts(g, KEY, mined[KEY], { now: NOW })

  const { proposal, ranking, dryRun } = proposeDossierProbe(g, KEY, { now: NOW })
  assert.equal(dryRun, true)
  assert.ok(proposal)
  assert.equal(ranking.length, 2)
  assert.ok(ranking[0].priority >= ranking[1].priority)
  assert.equal(proposeDossierProbe(g, 'no-such-repo', { now: NOW }).ranking.length, 0)
})

test('dossier facts coexist with research/self/config domains under the shared scorer', () => {
  const g = new CausalGraph()
  // A research-style hypothesis (foreign domain).
  g.addNode({
    id: 'hyp_research_1',
    type: NODE_TYPE.HYPOTHESIS,
    label: 'research',
    projectId: 'research',
    metric: { name: 'val_bpb', direction: 'lower' },
    status: 'testing',
  })
  const mined = mineRepoFacts(ROWS)
  ingestRepoFacts(g, KEY, mined[KEY], { now: NOW })

  // The dossier propose path never surfaces the foreign hypothesis…
  const { ranking } = proposeDossierProbe(g, KEY, { now: NOW })
  assert.ok(ranking.every((r) => r.hypothesisId.startsWith('hyp_dossier_')))
  // …and the foreign node is untouched by ingest/compile.
  assert.equal(g.getNode('hyp_research_1').projectId, 'research')
  compileDossier(g, KEY, { now: NOW })
  assert.equal(g.getNode('hyp_research_1').status, 'testing')
})

// ─── The Cut as a second verdict source, and the loop it would otherwise close ──
// The ledger records what an agent TYPED, and agents type pipelines, so its
// `exit` is usually tail(1)'s. The Cut runs its verify directly and records the
// real exit — but it CHOOSES that verify from the dossier's own learned ritual,
// which is a feedback loop. These tests pin the rule that cuts it:
//
//     a dossier-sourced verdict may never SUPPORT the fact that selected it;
//     it may only CONTRADICT.

const CUTKEY = 'home-u-proj-0011223344556677' // == KEY: same repo, second source

/** One Cut manifest row, exactly as cockpit/src/cut.rs appends it. */
const cutRow = (
  machine,
  { key = CUTKEY, session = 1, ts = NOW_TS, path = 'cockpit/src/lib.rs' } = {},
) => ({
  v: 1,
  kind: 'authored',
  ts,
  session,
  seq: 1,
  tool: 'str_replace',
  repo: { key, root: '/home/u/proj', slug: 'u/proj' },
  path,
  authored_sha256: 'deadbeef',
  authored_bytes: 10,
  authored: 'fn x() {}',
  driver: 'glm',
  route_driver: 'glm-5.2',
  model: 'glm-5.2',
  hop: 1,
  machine,
})

/** A verify verdict: `cargo check` ran directly, unpiped, and returned `exit`. */
const verdict = (exit, source, opts = {}) =>
  cutRow({ cmd: 'cargo check', dir: 'cockpit', dur_ms: 3000, exit, source, timed_out: false }, opts)

/** The independent base every test builds on: 3 `default`-sourced passes, 2 sessions. */
const BASE = [
  verdict(0, 'default', { session: 1, ts: NOW_TS }),
  verdict(0, 'default', { session: 1, ts: NOW_TS }),
  verdict(0, 'default', { session: 2, ts: NOW_TS }),
]

/** Mine + ingest a cut manifest (no ledger rows) and return the build ritual's belief. */
const buildFact = (cutRows, { now = NOW } = {}) => {
  const g = new CausalGraph()
  const mined = mineRepoFacts([], cutRows)
  if (!mined[CUTKEY]) return null
  ingestRepoFacts(g, CUTKEY, mined[CUTKEY], { now })
  const node = dossierFacts(g, CUTKEY).find((n) => n.factClass === 'build')
  if (!node) return null
  return { node, ...factBelief(g, node, { now }), observed: node.observed }
}

test('cut: skipped / timed-out / unverified rows are not verdicts and take no part', () => {
  assert.equal(cutSample(cutRow({ skipped: 'not-source' })), null)
  assert.equal(
    cutSample(cutRow({ cmd: 'cargo check', dir: '.', timed_out: true, dur_ms: 60000 })),
    null,
  )
  assert.equal(cutSample(cutRow(undefined)), null)
  // A manifest of nothing but non-verdicts mints nothing at all — an unchecked
  // write must never look like a passing one.
  const nonVerdicts = [
    cutRow({ skipped: 'not-source' }),
    cutRow({ skipped: 'not-source' }, { session: 2 }),
    cutRow({ skipped: 'not-source' }, { session: 3 }),
    cutRow({ cmd: 'cargo check', dir: '.', timed_out: true }, { session: 4 }),
  ]
  assert.equal(buildFact(nonVerdicts), null)
})

test('cut: a default-sourced verdict votes in full — it mints the ritual and moves belief', () => {
  const fact = buildFact(BASE)
  assert.ok(fact, 'three independent runs across two sessions mint a build ritual')
  assert.equal(fact.node.factText, 'cargo check')
  assert.equal(fact.observed.runs, 3)
  assert.equal(fact.observed.verdicts, 3, 'unpiped: every run carries a real exit code')
  assert.equal(fact.observed.passes, 3)
  assert.ok(fact.belief > 0.5, `a passing ritual is believed (${fact.belief})`)

  // …and it votes DOWN as readily as up: swap one pass for a failure.
  const withFail = [BASE[0], BASE[1], verdict(101, 'default', { session: 2 })]
  const failed = buildFact(withFail)
  assert.equal(failed.observed.passes, 2)
  assert.equal(failed.observed.fails, 1)
  assert.ok(failed.belief < fact.belief, 'an independent failure lowers belief')
})

test('cut: a dossier-sourced verdict CANNOT raise belief — the loop is cut', () => {
  const base = buildFact(BASE)

  // The Cut ran `cargo check` twenty times BECAUSE the dossier told it to, and it
  // passed every time. That is not evidence — it is an echo. Belief must not move
  // by so much as a float.
  const echoed = Array.from({ length: 20 }, (_, i) =>
    verdict(0, 'dossier', { session: 100 + i, ts: NOW_TS }),
  )
  const after = buildFact([...BASE, ...echoed])

  assert.equal(after.belief, base.belief, 'self-selected passes move belief exactly nowhere')
  assert.equal(after.observed.runs, base.observed.runs, 'nor recurrence')
  assert.equal(after.observed.verdicts, base.observed.verdicts, 'nor the verdict denominator')
  assert.equal(after.observed.passes, base.observed.passes, 'nor the pass count')
  assert.equal(after.observed.sessions, base.observed.sessions, 'nor the session count')
})

test('cut: dossier-sourced passes cannot keep their own fact fresh (the decay back door)', () => {
  // Freshness is a belief-raising channel: belief = 0.5 + (raw-0.5)·0.5^(age/halfLife),
  // so refreshing the clock pushes a >0.5 belief UP. If a self-selected pass could
  // stamp `lastSeen`, a learned ritual would keep itself permanently fresh and
  // permanently believed on its own say-so — the same loop, through a side door.
  const LATER = '2026-07-20T00:00:00.000Z' // 14 days on: one half-life
  const laterTs = Math.floor(Date.parse(LATER) / 1000)

  const base = buildFact(BASE, { now: LATER })
  const echoed = buildFact([...BASE, verdict(0, 'dossier', { session: 9, ts: laterTs })], {
    now: LATER,
  })

  assert.equal(echoed.observed.lastSeen, base.observed.lastSeen, 'the clock did not move')
  assert.equal(echoed.belief, base.belief, 'so the decayed belief did not move either')
  assert.ok(base.ageDays > 13, 'and the fact is correctly aging toward a probe')
})

test('cut: a dossier-sourced FAILURE lowers belief — a rotted ritual must fall out', () => {
  const base = buildFact(BASE)
  // The learned ritual starts failing. THIS is real news, and it is the one thing
  // a dossier-sourced verdict is allowed to say.
  const rotted = buildFact([...BASE, verdict(101, 'dossier', { session: 9 })])

  assert.ok(rotted.belief < base.belief, `belief must fall (${base.belief} → ${rotted.belief})`)
  assert.equal(rotted.observed.verdicts, 4, 'the failure joins the denominator')
  assert.equal(rotted.observed.fails, 1)
  assert.equal(rotted.observed.passes, 3, 'but it adds no passes')

  // And it keeps falling as the rot continues — belief is monotone in dossier fails.
  const worse = buildFact([
    ...BASE,
    verdict(101, 'dossier', { session: 9 }),
    verdict(101, 'dossier', { session: 10 }),
    verdict(101, 'dossier', { session: 11 }),
  ])
  assert.ok(worse.belief < rotted.belief, 'more failures, less belief')
})

test('cut: dossier-sourced verdicts cannot mint a ritual into existence on their own', () => {
  // No independent evidence anywhere — only The Cut running what the dossier told
  // it to run. There is no fact here, only a reflection, and recurrence must not
  // manufacture one.
  const echoOnly = Array.from({ length: 8 }, (_, i) =>
    verdict(0, 'dossier', { session: i + 1, ts: NOW_TS }),
  )
  assert.equal(buildFact(echoOnly), null)
})

test('cut: an unprovenanced verdict is fail-closed — contradict-only, never support', () => {
  // A row with no `machine.source` (a legacy shard, a future writer that forgot).
  // We do not guess provenance: we assume the worst, because a belief inflated by
  // an unprovenanced verdict is exactly the failure this rule exists to prevent.
  assert.equal(cutSample(verdict(0, undefined)).independent, false)
  assert.equal(cutSample(verdict(0, 'default')).independent, true)
  assert.equal(cutSample(verdict(0, 'env')).independent, true)
  assert.equal(cutSample(verdict(0, 'dossier')).independent, false)

  const base = buildFact(BASE)
  const unknownPass = buildFact([...BASE, verdict(0, undefined, { session: 9 })])
  assert.equal(unknownPass.belief, base.belief, 'an unprovenanced pass is inert')

  const unknownFail = buildFact([...BASE, verdict(101, undefined, { session: 9 })])
  assert.ok(unknownFail.belief < base.belief, 'an unprovenanced failure still disconfirms')
})

test('cut: verdicts are attributed to the repo key on the row, verbatim', () => {
  // The key is minted Rust-side (workspace_store::workspace_key, which resolves a
  // linked worktree to its main worktree). The miner never re-derives it and never
  // reassigns it: a worktree's verdict landing on the wrong repo is a fabricated
  // fact, and a fabricated fact is worse than none.
  const mined = mineRepoFacts([], [...BASE, verdict(0, 'default', { key: KEY2, session: 5 })])
  assert.ok(mined[CUTKEY], 'this repo got its own verdicts')
  assert.equal(mined[CUTKEY].rituals.length, 1)
  assert.equal(mined[CUTKEY].rituals[0].runs, 3, "and only its own — not the other repo's")
  assert.equal(mined[KEY2].rituals.length, 0, 'one run is not a ritual anywhere')
})

test('cut: the ledger and the manifest are one evidence pool, not two', () => {
  // A repo whose build command recurs in BOTH sources: the ledger proves it is
  // what the user runs, the manifest proves it actually passes. The ledger's
  // piped rows carry no verdict (tail(1)'s exit), so they add recurrence only —
  // and the fact is still minted, with the manifest supplying every verdict.
  const ledger = [
    cmd('cd cockpit && cargo check 2>&1 | tail -20', 0, { session: 7 }),
    cmd('cd cockpit && cargo check 2>&1 | tail -20', 0, { session: 8 }),
  ]
  const mined = mineRepoFacts(ledger, BASE)
  const build = mined[KEY].rituals.find((r) => r.class === 'build')
  assert.ok(build)
  assert.equal(build.canonical, 'cargo check')
  assert.equal(build.runs, 5, 'recurrence counts both sources')
  assert.equal(build.verdicts, 3, 'but only The Cut ever observed an exit status')
  assert.equal(build.passes, 3)
})

test('M05 dossier publication bounds entries and redacted bytes, newest verified first', () => {
  const facts = Array.from({ length: DOSSIER_STORE_FACTS + 100 }, (_, i) => ({
    id: i,
    text: 'x'.repeat(2000),
    lastVerifiedAt: String(i).padStart(8, '0'),
  }))
  const text = boundedDossierText({ v: 1, facts }, (text) => text.replaceAll('x', 'xx'))
  const result = JSON.parse(text)
  assert.ok(Buffer.byteLength(text) <= DOSSIER_STORE_BYTES)
  assert.ok(result.facts.length <= DOSSIER_STORE_FACTS)
  assert.equal(result.facts[0].id, facts.length - 1)
  assert.throws(() => boundedDossierText({ facts: [], thread: 'x'.repeat(DOSSIER_STORE_BYTES) }))
})
