import test from 'node:test'
import assert from 'node:assert/strict'
import { copyFileSync, mkdirSync, mkdtempSync, rmSync, writeFileSync } from 'node:fs'
import { spawnSync } from 'node:child_process'
import { tmpdir } from 'node:os'
import { dirname, join } from 'node:path'
import { pathToFileURL } from 'node:url'
import { project } from '../../lib/test-runner/report.mjs'
import { RELEASE_PATHS } from '../../scripts/release/release-evidence.mjs'

const max = '18446744073709551615'
const report = (lang, stdout, stderr = '') => project({version: 1, lang, stdout, stderr, max})
const counts = (passed, failed, skipped) => ({version: 1, passed, failed, skipped})

test('node, jest, vitest, mocha and TAP counts', () => {
  assert.deepEqual(report('js', '# pass 1\n# fail 1\n# skipped 2\n'), counts('1', '1', '2'))
  assert.deepEqual(report('js', '', 'Tests: 1 failed, 3 passed, 4 total\n'), counts('3', '1', '0'))
  assert.deepEqual(report('js', 'Tests  3 passed | 1 failed (4)\n'), counts('3', '1', '0'))
  assert.deepEqual(report('js', ' 3 passing\n 1 failing\n 2 pending\n'), counts('3', '1', '2'))
  assert.deepEqual(report('js', 'ok 1 - a\nnot ok 2 - b\n'), counts('1', '1', '0'))
})

test('Node cancellations are failures in both TAP and spec summaries', () => {
  assert.deepEqual(report('js', '# pass 1\n# fail 2\n# cancelled 3\n# skipped 4\n# todo 5\n'), counts('1', '5', '9'))
  assert.deepEqual(report('js', 'ℹ cancelled 3\nℹ pass 1\nℹ fail 2\nℹ skipped 4\n'), counts('1', '5', '4'))
  assert.deepEqual(report('js', '# cancelled 1\n'), counts('0', '1', '0'))
})

test('runner whitespace boundaries retain Unicode White_Space semantics', () => {
  const whitespace = [' ', '\t', '\r', '\v', '\f', '\u0085', '\u00a0', '\u1680', '\u2000', '\u200a', '\u2028', '\u2029', '\u202f', '\u205f', '\u3000']
  for (const w of whitespace) {
    assert.deepEqual(report('js', `${w}# pass${w}2${w}\n${w}# fail${w}1${w}\n`), counts('2', '1', '0'), JSON.stringify(w))
  }
  for (const literal of ['\ufeff', '\u200b']) {
    assert.deepEqual(report('go', `${literal}--- PASS: fixture\n`), counts('0', '0', '0'), 'non-White_Space must not be trimmed')
  }
  assert.deepEqual(report('go', `diagnostic a${' '.repeat(20000)}b\n--- PASS: fixture\n`), counts('1', '0', '0'))
})

test('real Node test timeout cannot disappear from the displayed failure count', t => {
  const root = mkdtempSync(join(tmpdir(), 'angel-node-cancel-counts-'))
  t.after(() => rmSync(root, { recursive: true, force: true }))
  const file = join(root, 'cancel.test.mjs')
  writeFileSync(file, "import { test } from 'node:test'\ntest('timeout', {timeout:25}, () => new Promise(() => {}))\n")
  const env = { ...process.env }
  delete env.NODE_TEST_CONTEXT
  const result = spawnSync(process.execPath, ['--test', '--test-concurrency=1', '--test-reporter=tap', file], { encoding: 'utf8', timeout: 5000, env })
  assert.equal(result.status, 1, result.stdout + result.stderr)
  assert.match(result.stdout, /# cancelled 1/)
  assert.deepEqual(report('js', result.stdout, result.stderr), counts('0', '1', '0'))
})

test('pytest, unittest, Go, and Swift counts', () => {
  assert.deepEqual(report('python', '=== 3 passed, 1 failed, 2 skipped in 0.12s ==='), counts('3', '1', '2'))
  assert.deepEqual(report('python', '', 'Ran 4 tests in 0.001s\nFAILED (failures=1, errors=1)'), counts('2', '2', '0'))
  assert.deepEqual(report('go', '--- PASS: A\n--- FAIL: B\n--- SKIP: C\n'), counts('1', '1', '1'))
  assert.deepEqual(report('swift', 'Executed 3 tests, with 1 failures'), counts('2', '1', '0'))
})

test('XCTest skips are omissions, not passing tests', () => {
  for (const lang of ['swift', 'rust']) {
    assert.deepEqual(report(lang, 'Executed 4 tests, with 2 tests skipped and 1 failure (0 unexpected) in 0.01 (0.01) seconds\n'), counts('1', '1', '2'))
    assert.deepEqual(report(lang, 'Executed 1 test, with 1 test skipped and 0 failures (0 unexpected) in 0.01 (0.01) seconds\n'), counts('0', '0', '1'))
    assert.deepEqual(report(lang, 'Executed 2 tests, with 0 failures (0 unexpected) in 0.01 (0.01) seconds\n'), counts('2', '0', '0'))
    assert.deepEqual(report(lang, 'Executed 1 test, with 1 test skipped and 0 failures\nExecuted 4 tests, with 2 tests skipped and 1 failure\n'), counts('1', '1', '2'))
  }
})

test('empty or entirely skipped Go runs do not become passing tests', () => {
  assert.deepEqual(report('go', 'testing: warning: no tests to run\nPASS\nok\texample.test\t0.002s [no tests to run]\n'), counts('0', '0', '0'))
  assert.deepEqual(report('go', '=== RUN   TestSkip\n--- SKIP: TestSkip (0.00s)\nPASS\nok\texample.test\t0.002s\n'), counts('0', '0', '1'))
  assert.deepEqual(report('go', 'ok\texample.test\t0.002s\n'), counts('1', '0', '0'), 'ordinary non-verbose package reports remain supported')
})

test('Go package failures cannot disappear alongside verbose results or be counted twice', () => {
  const pass = '--- PASS: TestPass (0.00s)\nPASS\nok\texample.test/pass\t0.001s\n'
  const fail = '--- FAIL: TestFail (0.00s)\nFAIL\nFAIL\texample.test/fail\t0.001s\n'
  const build = 'FAIL\texample.test/broken [build failed]\n'
  assert.deepEqual(report('go', build + pass + 'FAIL\n'), counts('1', '1', '0'))
  assert.deepEqual(report('go', pass + build + 'FAIL\n'), counts('1', '1', '0'))
  assert.deepEqual(report('go', fail + pass + build + 'FAIL\n'), counts('1', '2', '0'))
  assert.deepEqual(report('go', pass + 'ok\texample.test/quiet\t0.001s\n'), counts('2', '0', '0'))
  assert.deepEqual(report('go', pass + 'panic: TestMain\nFAIL\texample.test/panic\t0.001s\nFAIL\n'), counts('1', '1', '0'))
})

test('real Go mixed-package output retains build failures alongside passing tests', t => {
  const available = spawnSync('go', ['version'], { encoding: 'utf8' })
  if (available.error?.code === 'ENOENT') return t.skip('Go is not installed')
  assert.equal(available.status, 0, available.stderr)
  const root = mkdtempSync(join(tmpdir(), 'angel-go-mixed-counts-'))
  t.after(() => rmSync(root, { recursive: true, force: true }))
  writeFileSync(join(root, 'go.mod'), 'module example.test/counts\n\ngo 1.18\n')
  for (const name of ['passing', 'broken', 'failing']) mkdirSync(join(root, name))
  writeFileSync(join(root, 'passing', 'pass_test.go'), 'package passing\nimport "testing"\nfunc TestPass(t *testing.T) {}\n')
  writeFileSync(join(root, 'broken', 'broken.go'), 'package broken\nvar _ = undefinedFixtureSymbol\n')
  writeFileSync(join(root, 'failing', 'fail_test.go'), 'package failing\nimport "testing"\nfunc TestFail(t *testing.T) { t.Fatal("fixture failure") }\n')
  const result = spawnSync('go', ['test', '-v', './...'], {
    cwd: root, encoding: 'utf8', timeout: 45000,
    env: { ...process.env, GOWORK: 'off', GOTOOLCHAIN: 'local' },
  })
  assert.equal(result.status, 1, result.stdout + result.stderr)
  assert.match(result.stdout, /\[build failed\]/)
  assert.deepEqual(report('go', result.stdout, result.stderr), counts('1', '2', '0'))
})

test('unittest subtest failures cannot leave a phantom passing test', () => {
  assert.deepEqual(report('python', '', 'Ran 1 test in 0.001s\nFAILED (failures=2)\n'), counts('0', '2', '0'))
  assert.deepEqual(report('python', '', 'Ran 1 test in 0.001s\nFAILED (errors=2, skipped=1)\n'), counts('0', '2', '1'))
})

test('pytest skipped-only summaries and TAP directives preserve omission counts', () => {
  assert.deepEqual(report('python', '=== 2 skipped in 0.01s ===\n'), counts('0', '0', '2'))
  assert.deepEqual(report('python', '=== 2 deselected in 0.01s ===\n'), counts('0', '0', '2'))
  assert.deepEqual(report('js', 'ok 1 - disabled # SKIP reason\nnot ok 2 - pending # TODO later\nok 3 - active\n'), counts('1', '0', '2'))
})

test('CTest summary wins and inconsistent summaries fail closed', () => {
  const nested = 'All tests passed (9 assertions in 3 test cases)\n'
  assert.deepEqual(report('cpp', nested + '67% tests passed, 1 tests failed out of 3'), counts('2', '1', '0'))
  assert.deepEqual(report('cpp', nested + '100% tests passed out of +1'), counts('0', '0', '0'))
  assert.deepEqual(report('cpp', '100% tests passed out of 3', '67% tests passed, 1 tests failed out of 3'), counts('0', '0', '0'))
  assert.deepEqual(report('cpp', '100% tests passed out of 18446744073709551615'), counts(max, '0', '0'))
  assert.deepEqual(report('cpp', '[  PASSED  ] 3 tests.\n[  FAILED  ] 1 test, listed below:'), counts('3', '1', '0'))
})

test('CTest skipped and disabled results cannot become passing tests', () => {
  const enabled = '1/3 Test #1: skip ....***Skipped   0.00 sec\n'
  const disabled = '3/3 Test #3: disabled ....***Not Run (Disabled)   0.00 sec\n'
  assert.deepEqual(report('cpp', enabled + disabled + '100% tests passed, 0 tests failed out of 2\n'), counts('1', '0', '2'))
  assert.deepEqual(report('cpp', enabled + '100% tests passed, 0 tests failed out of 1\n'), counts('0', '0', '1'))
  assert.deepEqual(report('cpp', disabled + 'No tests were found!!!\n'), counts('0', '0', '1'))
  assert.deepEqual(report('cpp', enabled + disabled + '67% tests passed, 1 tests failed out of 3\n'), counts('1', '1', '2'))
  assert.deepEqual(report('cpp', enabled + '0% tests passed, 1 tests failed out of 1\n'), counts('0', '0', '0'), 'inconsistent omissions must fail closed')
})

test('real CTest skips, disabled cases, and failures retain distinct counts', t => {
  const available = spawnSync('ctest', ['--version'], { encoding: 'utf8' })
  if (available.error?.code === 'ENOENT') return t.skip('CTest is not installed')
  assert.equal(available.status, 0, available.stderr)
  const root = mkdtempSync(join(tmpdir(), 'angel-ctest-counts-'))
  t.after(() => rmSync(root, { recursive: true, force: true }))
  const node = process.execPath.replaceAll('\\', '/').replaceAll('"', '\\"')
  const cases = {
    pass: `add_test(pass "${node}" "-e" "process.exit(0)")`,
    fail: `add_test(fail "${node}" "-e" "process.exit(1)")`,
    skip: `add_test(skip "${node}" "-e" "process.exit(77)")\nset_tests_properties(skip PROPERTIES SKIP_RETURN_CODE 77)`,
    disabled: `add_test(disabled "${node}" "-e" "process.exit(1)")\nset_tests_properties(disabled PROPERTIES DISABLED TRUE)`,
  }
  for (const [names, expected, exit] of [
    [['pass', 'skip', 'disabled'], counts('1', '0', '2'), 0],
    [['skip'], counts('0', '0', '1'), 0],
    [['disabled'], counts('0', '0', '1'), 0],
    [['pass', 'fail', 'skip', 'disabled'], counts('1', '1', '2'), 8],
  ]) {
    writeFileSync(join(root, 'CTestTestfile.cmake'), names.map(name => cases[name]).join('\n') + '\n')
    const result = spawnSync('ctest', ['--test-dir', root, '--output-on-failure'], { encoding: 'utf8', timeout: 15000, env: { ...process.env, CTEST_PARALLEL_LEVEL: '1' } })
    assert.equal(result.status, exit, result.stdout + result.stderr)
    assert.deepEqual(report('cpp', result.stdout, result.stderr), expected, names.join(', '))
  }
})

test('count overflow is rejected instead of becoming a passing report', () => {
  assert.throws(() => report('js', '# pass 18446744073709551615\n# fail 1\n'), /count/)
  assert.throws(() => report('js', '# skipped 18446744073709551615\n# todo 1\n'), /count/)
})

test('very long digit tokens preserve bounded parsing', () => {
  assert.deepEqual(report('js', `# pass ${'0'.repeat(200000)}1\n`), counts('1', '0', '0'))
  assert.deepEqual(report('js', `# pass ${'9'.repeat(200000)}\n`), counts('0', '0', '0'))
})

test('release assets load from an isolated resource bundle', async t => {
  const paths = ['scripts/runtime/test-runner-report.mjs', 'lib/test-runner/report.mjs']
  const root = mkdtempSync(join(tmpdir(), 'angel-test-summary-bundle-'))
  t.after(() => rmSync(root, {recursive: true, force: true}))
  for (const path of paths) {
    assert.ok(RELEASE_PATHS.includes(path), `${path} is missing from release resources`)
    const target = join(root, path)
    mkdirSync(dirname(target), {recursive: true})
    copyFileSync(new URL(`../../${path}`, import.meta.url), target)
  }
  const module = await import(pathToFileURL(join(root, paths[0])))
  const request = {version: 1, lang: 'js', stdout: '# pass 2\n# fail 0\n', stderr: '', max}
  assert.deepEqual(module.project(request), counts('2', '0', '0'))
})
