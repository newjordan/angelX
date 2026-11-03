import test from 'node:test'
import assert from 'node:assert/strict'
import { copyFileSync, mkdirSync, mkdtempSync, rmSync } from 'node:fs'
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

test('pytest, unittest, Go, and Swift counts', () => {
  assert.deepEqual(report('python', '=== 3 passed, 1 failed, 2 skipped in 0.12s ==='), counts('3', '1', '2'))
  assert.deepEqual(report('python', '', 'Ran 4 tests in 0.001s\nFAILED (failures=1, errors=1)'), counts('2', '2', '0'))
  assert.deepEqual(report('go', '--- PASS: A\n--- FAIL: B\n--- SKIP: C\n'), counts('1', '1', '1'))
  assert.deepEqual(report('swift', 'Executed 3 tests, with 1 failures'), counts('2', '1', '0'))
})

test('CTest summary wins and inconsistent summaries fail closed', () => {
  const nested = 'All tests passed (9 assertions in 3 test cases)\n'
  assert.deepEqual(report('cpp', nested + '67% tests passed, 1 tests failed out of 3'), counts('2', '1', '0'))
  assert.deepEqual(report('cpp', nested + '100% tests passed out of +1'), counts('0', '0', '0'))
  assert.deepEqual(report('cpp', '100% tests passed out of 3', '67% tests passed, 1 tests failed out of 3'), counts('0', '0', '0'))
  assert.deepEqual(report('cpp', '100% tests passed out of 18446744073709551615'), counts(max, '0', '0'))
  assert.deepEqual(report('cpp', '[  PASSED  ] 3 tests.\n[  FAILED  ] 1 test, listed below:'), counts('3', '1', '0'))
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
