import { test } from 'node:test'
import assert from 'node:assert/strict'
import {
  copyFileSync,
  mkdtempSync,
  mkdirSync,
  readFileSync,
  readdirSync,
  rmSync,
  writeFileSync,
  existsSync,
  symlinkSync,
  openSync,
  closeSync,
} from 'node:fs'
import { tmpdir } from 'node:os'
import { dirname, join } from 'node:path'
import { fileURLToPath } from 'node:url'
import { spawnSync } from 'node:child_process'
import {
  RELEASE_PATHS,
  REQUIRED_COCKPIT_EMBEDDED_FILES,
  assertRuntimeHelperClosure,
  assertPublicReleaseEntries,
} from '../../scripts/release/release-evidence.mjs'

import CausalGraph, { NODE_TYPE, EDGE_TYPE } from '../../lib/research/CausalGraph.js'
import * as core from '../../lib/dossier/core.mjs'
import * as worker from '../../scripts/runtime/repo-dossier.mjs'
import {
  classifyCommand,
  normalizeCommand,
  mineRepoFacts,
  ingestRepoFacts,
  dossierFacts,
  factBelief,
  compileDossier,
  proposeDossierProbe,
  cutSample,
  DOSSIER_PROJECT,
  RITUAL_MIN_RUNS,
  TRAP_MIN_FAILS,
} from '../../lib/dossier/core.mjs'
import {
  dossierPaths,
  boundedDossierText,
  DOSSIER_STORE_BYTES,
  DOSSIER_STORE_FACTS,
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

test('bundled workers refresh dossier facts and habit drafts without a browser data tree', (t) => {
  const root = mkdtempSync(join(tmpdir(), 'angel-dossier-refresh-'))
  t.after(() => rmSync(root, { recursive: true, force: true }))
  const bundle = join(root, 'bundle')
  const resources = RELEASE_PATHS.filter(
    (path) =>
      path.startsWith('scripts/runtime/') ||
      path.startsWith('lib/') ||
      path === 'package.json' ||
      path === 'docs/telemetry/store-caps.toml',
  ).map((path) => ({ path }))
  for (const { path } of resources) {
    const target = join(bundle, path)
    mkdirSync(dirname(target), { recursive: true })
    copyFileSync(new URL(`../../${path}`, import.meta.url), target)
  }
  assertRuntimeHelperClosure(bundle, resources)
  const bundleAlias = join(root, 'bundle-alias')
  symlinkSync(bundle, bundleAlias, 'dir')
  const ledger = join(root, 'ledger.jsonl')
  const out = join(root, 'dossier')
  const cut = join(root, 'cut')
  mkdirSync(cut)
  const now = Math.floor(Date.now() / 1000)
  writeFileSync(
    ledger,
    [1, 2, 3, 4, 5, 6]
      .flatMap((session) => [
        cmdV3('cargo check', 'pass', { session, ts: now }),
        { ...cmdV3('cargo test', 'pass', { session, ts: now }), seq: 1 },
      ])
      .map((row) => JSON.stringify(row))
      .join('\n') + '\n',
  )
  const result = spawnSync(
    process.execPath,
    [
      join(bundleAlias, 'scripts/runtime/repo-dossier.mjs'),
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

  const proposed = join(root, 'proposed')
  const state = join(root, 'habits-state')
  const habitWorker = join(bundleAlias, 'scripts/runtime/habitsmith.mjs')
  const habitRun = (args) =>
    spawnSync(
      process.execPath,
      [habitWorker, '--graph', join(out, 'graph.json'), ...args, '--ledger', ledger],
      {
        cwd: root,
        encoding: 'utf8',
        timeout: 10_000,
        env: {
          ...process.env,
          ANGEL_HABIT_MIN_BELIEF: '0.75',
          ANGEL_HABIT_MAX_PROPOSALS_PER_DAY: '2',
          ANGEL_SKILLS_DIR: join(root, 'installed-skills'),
          ANGEL_BUNDLED_SKILLS_DIR: join(root, 'bundled-skills'),
        },
      },
    )
  const listed = habitRun(['--list'])
  assert.equal(listed.status, 0, listed.stderr)
  assert.match(listed.stdout, /build → test/)
  const refreshed = habitRun([
    '--refresh',
    '--proposed-dir',
    proposed,
    '--state-dir',
    state,
  ])
  assert.equal(refreshed.status, 0, refreshed.stderr)
  const drafts = readdirSync(proposed)
  assert.equal(drafts.length, 1)
  const draft = readFileSync(join(proposed, drafts[0], 'SKILL.md'), 'utf8')
  assert.match(draft, /1\. `cargo check`/)
  assert.match(draft, /2\. `cargo test`/)
  assert.equal(JSON.parse(readFileSync(join(state, 'proposals.json'), 'utf8')).runs, 1)
  assert.equal(existsSync(join(root, 'installed-skills')), false, 'drafts are not installed')
  assert.equal(existsSync(join(root, 'public')), false)
})

test('bundled cut report runs without its producing checkout', (t) => {
  const root = mkdtempSync(join(tmpdir(), 'angel-cut-bundle-'))
  t.after(() => rmSync(root, { recursive: true, force: true }))
  for (const path of ['scripts/runtime/cut-report.mjs', 'lib/cut/report.mjs']) {
    assert.ok(RELEASE_PATHS.includes(path), `${path} must ship`)
    const target = join(root, path)
    mkdirSync(dirname(target), { recursive: true })
    copyFileSync(new URL(`../../${path}`, import.meta.url), target)
  }
  const report = spawnSync(process.execPath, [join(root, 'scripts/runtime/cut-report.mjs'), '--dir', join(root, 'missing-cut'), '--workspace', root], {
    cwd: root,
    encoding: 'utf8',
    timeout: 10_000,
  })
  assert.equal(report.status, 0, report.stderr)
  assert.match(report.stdout, /authored: 0/)
})

test('bundled moa report runs without its producing checkout', (t) => {
  const root = mkdtempSync(join(tmpdir(), 'angel-moa-bundle-'))
  t.after(() => rmSync(root, { recursive: true, force: true }))
  const path = 'scripts/runtime/moa-report.mjs'
  assert.ok(RELEASE_PATHS.includes(path), `${path} must ship`)
  const target = join(root, path)
  mkdirSync(dirname(target), { recursive: true })
  copyFileSync(new URL(`../../${path}`, import.meta.url), target)
  const report = spawnSync(process.execPath, [target], {
    cwd: root,
    encoding: 'utf8',
    input: JSON.stringify({ records: [], lo: 0.45, hi: 0.7 }),
    timeout: 5000,
  })
  assert.equal(report.status, 0, report.stderr)
  assert.match(report.stdout, /moa ledger · 0 turn\(s\)/)
})

test('bundled science and repository policies run without their producing checkout', (t) => {
  const root = mkdtempSync(join(tmpdir(), 'angel-policy-bundle-'))
  t.after(() => rmSync(root, { recursive: true, force: true }))
  const paths = [
    'scripts/runtime/science-brief.mjs', 'lib/science/brief.mjs',
    'scripts/runtime/repo-search.mjs', 'lib/repo-search/rules.mjs',
  ]
  for (const path of paths) {
    assert.ok(RELEASE_PATHS.includes(path), `${path} must ship`)
    const target = join(root, path)
    mkdirSync(dirname(target), { recursive: true })
    copyFileSync(new URL(`../../${path}`, import.meta.url), target)
  }
  assertRuntimeHelperClosure(root, paths.map((path) => ({ path })))
  for (const [script, request] of [
    ['scripts/runtime/science-brief.mjs', { op: 'fold', papers: [] }],
    ['scripts/runtime/repo-search.mjs', { version: 1, op: 'rank', mode: 'best', repos: [] }],
  ]) {
    const input = join(root, 'request.json')
    writeFileSync(input, JSON.stringify(request))
    const fd = openSync(input, 'r')
    try {
      const result = spawnSync(process.execPath, [join(root, script)], {
        cwd: root, encoding: 'utf8', timeout: 10_000, stdio: [fd, 'pipe', 'pipe'],
      })
      assert.equal(result.status, 0, result.stderr)
      assert.doesNotThrow(() => JSON.parse(result.stdout))
    } finally { closeSync(fd) }
  }
})

test('bundled Sloptomizer publisher can publish a complete generation', (t) => {
  const root = mkdtempSync(join(tmpdir(), 'angel-research-bundle-'))
  t.after(() => rmSync(root, { recursive: true, force: true }))
  const base = 'cockpit/research/sloptomizer/'
  const publisher = `${base}publish_runtime.py`
  assert.ok(RELEASE_PATHS.includes(publisher), 'the publisher must ship')
  assertPublicReleaseEntries(fileURLToPath(new URL('../..', import.meta.url)), [{ path: publisher }])
  const paths = [publisher, ...REQUIRED_COCKPIT_EMBEDDED_FILES.filter((path) => path.startsWith(base))]
  for (const path of paths) {
    const target = join(root, path)
    mkdirSync(dirname(target), { recursive: true })
    copyFileSync(new URL(`../../${path}`, import.meta.url), target)
  }
  const dir = join(root, base)
  const result = spawnSync('python3', [join(root, publisher), dir], { cwd: root, encoding: 'utf8', timeout: 10_000 })
  assert.equal(result.status, 0, result.stderr)
  const manifest = JSON.parse(readFileSync(join(dir, 'generation.json'), 'utf8'))
  assert.equal(manifest.schema, 'angel.sloptomizer-generation/v1')
  assert.equal(Object.keys(manifest.files).length, paths.length - 1)
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
  for (const [name, value] of Object.entries(core)) {
    assert.strictEqual(worker[name], value, `worker must re-export the core's ${name}`)
  }
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
  assert.deepEqual(normalizeCommand('cd /home/u/proj/cockpit && cargo check', repo), {
    command: 'cargo check',
    attributable: true,
  })
  assert.equal(
    classifyCommand(normalizeCommand('cd /home/u/proj && cargo test -p x', repo).command),
    'test',
  )
  assert.equal(
    normalizeCommand('cd /home/u/proj; cd cockpit && npm test', repo).command,
    'npm test',
  )
  assert.equal(normalizeCommand('cd "$(pwd)" && npm test', repo).command, 'npm test')
  assert.deepEqual(normalizeCommand('cargo test', repo), {
    command: 'cargo test',
    attributable: true,
  })
})

test("a pipeline's exit belongs to its last stage, so the command gets NO verdict", () => {
  const repo = { root: '/home/u/proj' }
  assert.deepEqual(normalizeCommand('cd /home/u/proj && cargo check 2>&1 | tail -20', repo), {
    command: 'cargo check',
    attributable: false,
  })
  assert.deepEqual(normalizeCommand('cargo test; echo done', repo), {
    command: 'cargo test',
    attributable: false,
  })
  assert.deepEqual(normalizeCommand('cargo test 2>&1', repo), {
    command: 'cargo test',
    attributable: true,
  })
})

test('normalization preserves quoted and escaped operator literals', () => {
  for (const text of [
    `node -e 'console.log("a|b"); process.exit(0)'`,
    `node -e 'console.log("> file")'`,
    'cargo test --filter "left || right; > output"',
    String.raw`cargo test --filter left\|right`,
    String.raw`cargo test --filter a\;b`,
    String.raw`cargo test --filter a\>b`,
  ]) assert.deepEqual(normalizeCommand(text), { command: text, attributable: true }, text)
  assert.deepEqual(normalizeCommand(`cargo test --filter 'a|b' 2>&1 | tail -20`), {
    command: `cargo test --filter 'a|b'`, attributable: false,
  })
  assert.deepEqual(normalizeCommand(`cargo test > "file with spaces.log" 2>&1`), {
    command: 'cargo test', attributable: true,
  })
  for (const redirect of ['12>&1', '123> output.log', '&> output.log', '&>> "output log"']) {
    assert.deepEqual(normalizeCommand(`cargo test ${redirect}`), { command: 'cargo test', attributable: true }, redirect)
  }
  for (const text of ['cargo test filter12>log', 'cargo test filter12>&1']) {
    assert.deepEqual(normalizeCommand(text), { command: 'cargo test filter12', attributable: true }, text)
  }
  for (const text of ['cargo test <<EOF', 'cargo test <<< "input"', 'cargo test < input', 'cargo test filter12< input', 'cargo test 0<&3', 'cargo test <> input']) {
    assert.equal(normalizeCommand(text), null, text)
  }
  assert.deepEqual(normalizeCommand('cargo test --filter "a<b"'), { command: 'cargo test --filter "a<b"', attributable: true })
  for (const text of ['cargo test "unfinished', 'cargo test $(echo nested)', 'cargo test &', 'cargo test\necho done']) {
    assert.equal(normalizeCommand(text), null, text)
  }
})

test("a chdir into ANOTHER repo is that repo's evidence, never this one's", () => {
  const gpug = { root: '/home/user/gpug' }
  assert.equal(normalizeCommand('cd /home/user/angelX/cockpit && cargo check', gpug), null)
  const wt = { root: '/home/user/angelX', cwd: '/tmp/cut-forge/item-3' }
  assert.equal(
    normalizeCommand('cd /tmp/cut-forge/item-3/cockpit && cargo check', wt).command,
    'cargo check',
  )
})

test('relative chdirs resolve from the stamped cwd and cannot hide repository escapes', () => {
  const repo = { root: '/home/user/project', cwd: '/home/user/project/cockpit' }
  assert.equal(normalizeCommand('cd ../../other && cargo check', repo), null)
  assert.deepEqual(normalizeCommand('cd ../src && cargo test', repo), {
    command: 'cargo test',
    attributable: true,
  })
})

test('chained relative chdirs carry the effective cwd between prefixes', () => {
  const repo = { root: '/home/user/project' }
  assert.deepEqual(normalizeCommand('cd cockpit; cd ../src && npm test', repo), {
    command: 'npm test',
    attributable: true,
  })
  assert.equal(normalizeCommand('cd cockpit; cd ../../../other && cargo check', repo), null)
  for (const text of ['cd "$HOME" && cargo test', 'cd ${DEST} && cargo test', 'cd ~/other && cargo test', 'cd - && cargo test', 'cd "$(get-repo)" && cargo test']) {
    assert.equal(normalizeCommand(text, repo), null, text)
  }
  assert.deepEqual(normalizeCommand("cd '$HOME' && cargo test", repo), { command: 'cargo test', attributable: true })
  assert.deepEqual(normalizeCommand("cd '~' && cargo test", repo), { command: 'cargo test', attributable: true })
  assert.deepEqual(normalizeCommand('cd "$(pwd)" && cargo test', repo), { command: 'cargo test', attributable: true })
})

test('unverifiable runs prove recurrence but never move belief', () => {
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

  const g = new CausalGraph()
  ingestRepoFacts(g, KEY, rec, { now: NOW })
  const node = dossierFacts(g, KEY)[0]
  assert.equal(factBelief(g, node, { now: NOW }).belief, 0.5)

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

test('quoted operator literals do not suppress an attributable v3 pipefail pass', () => {
  const text = `cargo test --filter 'left; right || other' 2>&1 | tail -20`
  const rows = [1, 2, 3].map(session => cmdV3(text, 'pass', { session }))
  const facts = mineRepoFacts(rows)[KEY]
  assert.equal(facts.rituals.length, 1)
  assert.equal(facts.rituals[0].canonical, `cargo test --filter 'left; right || other'`)
  assert.equal(facts.rituals[0].verdicts, 3)
  assert.equal(facts.rituals[0].passes, 3)
})

test('v3 pipeline failures stay unattributed, and dossier echoes cannot self-mint', () => {
  const failed = [1, 2, 3].map((session) =>
    cmdV3('cargo check | tail -20', 'fail', { session, exit: 1 }),
  )
  const failureFact = mineRepoFacts(failed)[KEY].rituals[0]
  assert.equal(failureFact.verdicts, 0)
  assert.equal(failureFact.passes, 0)

  const echoes = [1, 2, 3].map((session) =>
    cmdV3('cargo check', 'pass', { session, independent: false }),
  )
  assert.equal(mineRepoFacts(echoes)[KEY].rituals.length, 0)
})

test('the recurrence floor is enforced on the fact minted, not on the class pool', () => {
  const oneOffs = ['python3 -c "print(1)"', 'python3 -c "print(2)"', 'python3 -c "print(3)"'].map(
    (t, i) => cmd(t, 0, { session: i + 1 }),
  )
  assert.equal(mineRepoFacts(oneOffs)[KEY].rituals.length, 0)

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

  assert.equal(rec.rituals.length, 1)
  const ritual = rec.rituals[0]
  assert.equal(ritual.class, 'test')
  assert.equal(ritual.canonical, 'cargo test -p cockpit')
  assert.equal(ritual.runs, 3)
  assert.equal(ritual.passes, 2)
  assert.ok(ritual.sessions >= 2)

  assert.equal(rec.traps.length, 1)
  assert.equal(rec.traps[0].text, 'npm test')
  assert.equal(rec.traps[0].fails, TRAP_MIN_FAILS)

  assert.ok(!JSON.stringify(rec.rituals).includes('one-off'))

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
    { kind: 'event', event: 'cmd', cmd: { text: 'x', exit: 0 } },
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
  assert.equal(first.added.nodes, 2)
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

test('changing a canonical command invalidates only that command\'s probe evidence', () => {
  for (const edgeType of [EDGE_TYPE.SUPPORTS, EDGE_TYPE.CONTRADICTS]) {
    const g = new CausalGraph()
    const oldRows = [1, 2, 3].map((session) => cmdV3('cargo test -p old', 'no_verdict', { session }))
    const original = mineRepoFacts(oldRows)[KEY]
    ingestRepoFacts(g, KEY, original, { now: NOW })
    const fact = dossierFacts(g, KEY)[0]
    g.addNode({ id: 'probe', type: NODE_TYPE.EXPERIMENT, evidence: 'tested old command' })
    g.addNode({ id: 'other', type: NODE_TYPE.HYPOTHESIS })
    g.addEdge({ src: 'probe', dst: fact.id, type: edgeType, confidence: 0.85 })
    g.addEdge({ src: 'probe', dst: fact.id, type: EDGE_TYPE.TESTS })
    g.addEdge({ src: 'probe', dst: 'other', type: edgeType, confidence: 0.85 })
    g.updateNode(fact.id, { lastVerifiedAt: NOW, outcome: edgeType === EDGE_TYPE.SUPPORTS ? 'positive' : 'negative' })

    ingestRepoFacts(g, KEY, original, { now: NOW })
    assert.equal(g.getNode(fact.id).lastVerifiedAt, NOW)
    assert.equal(g.edges().length, 3)
    const reloaded = CausalGraph.deserialize(g.serialize())
    const replacement = mineRepoFacts([
      ...oldRows,
      ...[4, 5, 6, 7].map((session) => cmdV3('cargo test -p new', 'no_verdict', { session })),
    ])[KEY]
    ingestRepoFacts(reloaded, KEY, replacement, { now: NOW })
    const updated = reloaded.getNode(fact.id)
    assert.equal(updated.factText, 'cargo test -p new')
    assert.equal(updated.lastVerifiedAt, null, 'the new command has never been probed')
    assert.equal(updated.outcome, 'neutral')
    assert.equal(factBelief(reloaded, updated, { now: NOW }).belief, 0.5)
    assert.equal(reloaded.edges().length, 1, 'unrelated probe edges survive')
    assert.equal(reloaded.getNode('probe').evidence, 'tested old command', 'raw experiment evidence survives')
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

  const rb = factBelief(g, ritual, { now: NOW }).belief
  assert.ok(rb > 0.5 && rb <= 0.7, `ritual observational belief capped (${rb})`)
  const tb = factBelief(g, trap, { now: NOW }).belief
  assert.ok(tb > rb, `an always-failing trap (${tb}) is more certain than a 2/3 ritual (${rb})`)

  g.addNode({ id: 'exp_probe_1', type: NODE_TYPE.EXPERIMENT, label: 'probe' })
  g.addEdge({ src: 'exp_probe_1', dst: ritual.id, type: EDGE_TYPE.SUPPORTS, confidence: 0.8 })
  g.updateNode(ritual.id, { lastVerifiedAt: NOW })
  const verified = factBelief(g, g.getNode(ritual.id), { now: NOW }).belief
  assert.ok(verified > 0.75, `verified ritual belief rises (${verified})`)

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
  assert.deepEqual(JSON.parse(JSON.stringify(artifact)), artifact)
})

test('dossier batch paths scan evidence once and refresh after graph mutations', () => {
  const g = new CausalGraph()
  ingestRepoFacts(g, KEY, mineRepoFacts(ROWS)[KEY], { now: NOW })
  const facts = dossierFacts(g, KEY)
  assert.equal(facts.length, 2)
  const a = facts[0].id, b = facts[1].id
  g.addEdge({ id: 'batch_support', src: 'missing', dst: a, type: EDGE_TYPE.SUPPORTS, confidence: 0.8 })
  g.addEdge({ id: 'batch_contradiction', src: 'missing', dst: a, type: EDGE_TYPE.CONTRADICTS, confidence: NaN })
  g.addEdge({ id: 'batch_zero', src: 'missing', dst: b, type: EDGE_TYPE.SUPPORTS, confidence: 0 })
  g.addEdge({ id: 'batch_test', src: 'missing', dst: b, type: EDGE_TYPE.TESTS })
  const edges = g.edges.bind(g)
  let scans = 0
  g.edges = () => { scans++; return edges() }
  for (const mutate of [
    () => {},
    () => g.addEdge({ ...g.getEdge('batch_support'), confidence: -0.7 }),
    () => g.removeEdge('batch_contradiction'),
  ]) {
    mutate()
    const expected = new Map(facts.map(n => [n.id, factBelief(g, n, { now: NOW }).belief]))
    scans = 0
    const compiled = compileDossier(g, KEY, { now: NOW })
    assert.equal(scans, 1, 'compile must not scan evidence per fact')
    assert.equal(compiled.facts.length, facts.length)
    for (const entry of compiled.facts) assert.equal(entry.belief, Number(expected.get(entry.id).toFixed(3)))
    scans = 0
    const { ranking } = proposeDossierProbe(g, KEY, { now: NOW })
    assert.equal(scans, 1, 'probe must reuse the scorer belief')
    assert.equal(ranking.length, facts.length)
    for (const entry of ranking) assert.equal(entry.belief, expected.get(entry.hypothesisId))
  }
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

  const { ranking } = proposeDossierProbe(g, KEY, { now: NOW })
  assert.ok(ranking.every((r) => r.hypothesisId.startsWith('hyp_dossier_')))
  assert.equal(g.getNode('hyp_research_1').projectId, 'research')
  compileDossier(g, KEY, { now: NOW })
  assert.equal(g.getNode('hyp_research_1').status, 'testing')
})

const CUTKEY = 'home-u-proj-0011223344556677'

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

const verdict = (exit, source, opts = {}) =>
  cutRow({ cmd: 'cargo check', dir: 'cockpit', dur_ms: 3000, exit, source, timed_out: false }, opts)

const BASE = [
  verdict(0, 'default', { session: 1, ts: NOW_TS }),
  verdict(0, 'default', { session: 1, ts: NOW_TS }),
  verdict(0, 'default', { session: 2, ts: NOW_TS }),
]

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

  const withFail = [BASE[0], BASE[1], verdict(101, 'default', { session: 2 })]
  const failed = buildFact(withFail)
  assert.equal(failed.observed.passes, 2)
  assert.equal(failed.observed.fails, 1)
  assert.ok(failed.belief < fact.belief, 'an independent failure lowers belief')
})

test('cut: a dossier-sourced verdict CANNOT raise belief — the loop is cut', () => {
  const base = buildFact(BASE)

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
  const LATER = '2026-07-20T00:00:00.000Z'
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
  const rotted = buildFact([...BASE, verdict(101, 'dossier', { session: 9 })])

  assert.ok(rotted.belief < base.belief, `belief must fall (${base.belief} → ${rotted.belief})`)
  assert.equal(rotted.observed.verdicts, 4, 'the failure joins the denominator')
  assert.equal(rotted.observed.fails, 1)
  assert.equal(rotted.observed.passes, 3, 'but it adds no passes')

  const worse = buildFact([
    ...BASE,
    verdict(101, 'dossier', { session: 9 }),
    verdict(101, 'dossier', { session: 10 }),
    verdict(101, 'dossier', { session: 11 }),
  ])
  assert.ok(worse.belief < rotted.belief, 'more failures, less belief')
})

test('cut: dossier-sourced verdicts cannot mint a ritual into existence on their own', () => {
  const echoOnly = Array.from({ length: 8 }, (_, i) =>
    verdict(0, 'dossier', { session: i + 1, ts: NOW_TS }),
  )
  assert.equal(buildFact(echoOnly), null)
})

test('cut: an unprovenanced verdict is fail-closed — contradict-only, never support', () => {
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
  const mined = mineRepoFacts([], [...BASE, verdict(0, 'default', { key: KEY2, session: 5 })])
  assert.ok(mined[CUTKEY], 'this repo got its own verdicts')
  assert.equal(mined[CUTKEY].rituals.length, 1)
  assert.equal(mined[CUTKEY].rituals[0].runs, 3, "and only its own — not the other repo's")
  assert.equal(mined[KEY2].rituals.length, 0, 'one run is not a ritual anywhere')
})

test('cut: the ledger and the manifest are one evidence pool, not two', () => {
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
