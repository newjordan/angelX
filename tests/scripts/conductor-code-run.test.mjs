// Tests for conductor-code-run C4. The integration cases use a throwaway Rust
// crate plus a fake `angel` executable, so they exercise real git worktrees and
// cargo gates without fleet/model spend.

import { test } from 'node:test'
import assert from 'node:assert/strict'
import fs from 'node:fs'
import os from 'node:os'
import path from 'node:path'
import { execFileSync, spawnSync } from 'node:child_process'

import { evaluateGate, parseTestResult, runGate, saysDone } from '../../scripts/conductor-code-run.mjs'

const ROOT = path.join(path.dirname(new URL(import.meta.url).pathname), '..', '..')

function tmp(name) {
  return fs.mkdtempSync(path.join(os.tmpdir(), `angel-conductor-${name}-`))
}

function writeToyCrate(dir) {
  fs.mkdirSync(path.join(dir, 'src'), { recursive: true })
  fs.writeFileSync(
    path.join(dir, 'Cargo.toml'),
    '[package]\nname = "toy_conductor"\nversion = "0.1.0"\nedition = "2021"\n\n[lib]\npath = "src/lib.rs"\n',
  )
  fs.writeFileSync(
    path.join(dir, 'src/lib.rs'),
    [
      'pub fn one() -> i32 { 1 }',
      '',
      '#[cfg(test)]',
      'mod tests {',
      '    #[test]',
      '    fn it_works() { assert_eq!(super::one(), 1); }',
      '}',
      '',
    ].join('\n'),
  )
  execFileSync('git', ['init'], { cwd: dir, stdio: 'ignore' })
  execFileSync('git', ['add', '-A'], { cwd: dir })
  execFileSync(
    'git',
    ['-c', 'user.name=test', '-c', 'user.email=test@example.invalid', 'commit', '-m', 'init'],
    { cwd: dir, stdio: 'ignore' },
  )
}

function fakeAngel(dir, body) {
  const p = path.join(dir, 'fake-angel.sh')
  fs.writeFileSync(
    p,
    [
      '#!/usr/bin/env bash',
      'set -euo pipefail',
      'ws=""',
      'while [[ $# -gt 0 ]]; do',
      '  if [[ "$1" == "--workspace" ]]; then ws="$2"; shift 2; else shift; fi',
      'done',
      'cat >/dev/null',
      body,
      'echo CONDUCTOR_DONE',
      '',
    ].join('\n'),
  )
  fs.chmodSync(p, 0o755)
  return p
}

function runCodeRun({ repo, state, angel, extra = [] }) {
  const env = {
    ...process.env,
    ANGEL_CONDUCTOR_DRIVER: 'practice',
    ANGEL_CAUSAL_GRAPH: path.join(state, 'graph.json'),
  }
  delete env.NODE_TEST_CONTEXT
  delete env.NODE_OPTIONS
  return spawnSync(
    'node',
    [
      path.join(ROOT, 'scripts/conductor-code-run.mjs'),
      '--src',
      repo,
      '--state-dir',
      state,
      '--goal',
      'Make a harmless code change.',
      '--max-iters',
      '1',
      '--task-deadline',
      '10',
      '--run-deadline',
      '120',
      '--angel-bin',
      angel,
      ...extra,
    ],
    {
      cwd: ROOT,
      encoding: 'utf8',
      env,
    },
  )
}

test('parseTestResult and saysDone mirror the cockpit gate parsers', () => {
  const out = [
    'test result: ok. 3 passed; 0 failed; 1 ignored;',
    'test result: ok. 2 passed; 0 failed; 0 ignored;',
  ].join('\n')
  assert.deepEqual(parseTestResult(out), { passed: 5, failed: 0, ignored: 1 })
  assert.equal(saysDone('finished\nCONDUCTOR_DONE'), true)
  assert.equal(saysDone('**CONDUCTOR_DONE**'), true)
  assert.equal(saysDone('I will write CONDUCTOR_DONE later'), false)
})

test('evaluateGate rejects build/test/baseline failures and accepts green baseline', () => {
  assert.equal(evaluateGate({ buildOk: false, tests: {}, baselinePassed: 1 }).passed, false)
  assert.equal(
    evaluateGate({ buildOk: true, tests: { passed: 0, failed: 0 }, baselinePassed: 1 }).passed,
    false,
  )
  assert.equal(
    evaluateGate({ buildOk: true, tests: { passed: 2, failed: 1 }, baselinePassed: 1 }).passed,
    false,
  )
  assert.equal(
    evaluateGate({ buildOk: true, tests: { passed: 1, failed: 0 }, baselinePassed: 2 }).passed,
    false,
  )
  assert.equal(
    evaluateGate({ buildOk: true, tests: { passed: 2, failed: 0 }, baselinePassed: 2 }).passed,
    true,
  )
})

test('runGate accepts a green toy crate with the baseline pass count intact', () => {
  const dir = tmp('gate-green')
  try {
    writeToyCrate(dir)
    const gate = runGate(spawnSync, dir, 1)
    assert.equal(gate.passed, true, gate.summary)
    assert.equal(gate.tests.passed, 1)
  } finally {
    fs.rmSync(dir, { recursive: true, force: true })
  }
})

test('a test process failure cannot pass using an earlier green summary', () => {
  const spawn = (_cmd, args) =>
    args[0] === 'build'
      ? { status: 0, stdout: '', stderr: '' }
      : {
          status: 1,
          stdout: 'test result: ok. 1 passed; 0 failed; 0 ignored;',
          stderr: 'test process crashed',
        }
  assert.equal(runGate(spawn, '/fixture', 1).passed, false)
})

test('a done sentinel from a failed agent never creates a reviewable change', () => {
  const base = tmp('agent-exit')
  const repo = path.join(base, 'repo'),
    state = path.join(base, 'state')
  fs.mkdirSync(repo)
  writeToyCrate(repo)
  const angel = fakeAngel(base, 'echo CONDUCTOR_DONE; exit 9')
  try {
    const result = runCodeRun({ repo, state, angel })
    assert.equal(result.status, 1, result.stdout + result.stderr)
    assert.match(result.stderr, /model task failed \(9\)/)
    assert.equal(fs.existsSync(path.join(state, 'queue.json')), false)
  } finally {
    fs.rmSync(base, { recursive: true, force: true })
  }
})

test('a no-op agent cannot park worker state files as a product change', () => {
  const base = tmp('noop')
  const repo = path.join(base, 'repo'),
    state = path.join(base, 'state')
  fs.mkdirSync(repo)
  writeToyCrate(repo)
  // Cargo.lock is a generated untracked file in this library fixture. Exclude
  // it from the fixture so the only possible changes are worker state/target.
  fs.writeFileSync(path.join(repo, '.gitignore'), 'target/\nCargo.lock\n')
  execFileSync('git', ['add', '.gitignore'], { cwd: repo })
  execFileSync(
    'git',
    [
      '-c',
      'user.name=test',
      '-c',
      'user.email=test@example.invalid',
      'commit',
      '-m',
      'ignore build products',
    ],
    { cwd: repo },
  )
  const angel = fakeAngel(base, ':')
  try {
    const result = runCodeRun({ repo, state, angel })
    assert.equal(result.status, 1, result.stdout + result.stderr)
    assert.match(result.stderr, /no worktree changes/)
    assert.equal(fs.existsSync(path.join(state, 'queue.json')), false)
  } finally {
    fs.rmSync(base, { recursive: true, force: true })
  }
})

test('CLI parks a green fake-agent change in queue.json and leaves the worktree', () => {
  const base = tmp('park')
  const repo = path.join(base, 'repo')
  const state = path.join(base, 'state')
  fs.mkdirSync(repo)
  writeToyCrate(repo)
  const angel = fakeAngel(base, 'echo "// conductor parked change" >> "$ws/src/lib.rs"')

  try {
    const res = runCodeRun({ repo, state, angel })
    assert.equal(res.status, 0, `${res.stdout}\n${res.stderr}`)
    const queue = JSON.parse(fs.readFileSync(path.join(state, 'queue.json'), 'utf8'))
    assert.equal(queue.length, 1)
    assert.match(queue[0].branch, /^angel\/conductor-/)
    assert.equal(queue[0].gate.passed, true)
    assert.equal(queue[0].gate.baseline, 1)
    assert.ok(fs.existsSync(queue[0].worktree), 'parked worktree remains for review')
    assert.match(queue[0].diffstat, /src\/lib.rs/)
  } finally {
    fs.rmSync(base, { recursive: true, force: true })
  }
})

test('CLI rejects a fake-agent test deletion via the baseline pass-count guard', () => {
  const base = tmp('reject')
  const repo = path.join(base, 'repo')
  const state = path.join(base, 'state')
  fs.mkdirSync(repo)
  writeToyCrate(repo)
  const angel = fakeAngel(base, 'cat > "$ws/src/lib.rs" <<EOF\npub fn one() -> i32 { 1 }\nEOF')

  try {
    const res = runCodeRun({ repo, state, angel })
    assert.equal(res.status, 1, `${res.stdout}\n${res.stderr}`)
    assert.ok(!fs.existsSync(path.join(state, 'queue.json')), 'no parked queue entry')
    const runs = fs.readFileSync(path.join(state, 'runs.jsonl'), 'utf8')
    assert.match(runs, /"action":"rejected"/)
    const worktrees = path.join(state, 'worktrees')
    const remaining = fs.existsSync(worktrees) ? fs.readdirSync(worktrees) : []
    assert.deepEqual(remaining, [], 'failed worktree is removed')
  } finally {
    fs.rmSync(base, { recursive: true, force: true })
  }
})
