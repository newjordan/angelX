import { afterEach, test } from 'node:test'
import assert from 'node:assert/strict'
import fs from 'node:fs'
import os from 'node:os'
import { execFileSync } from 'node:child_process'
import { join } from 'node:path'
import { syncBuiltinESMExports } from 'node:module'

import {
  DEFAULT_RULES,
  auditDuplicateContent,
  explainGroup,
  findDuplicateGroups,
  globToRegExp,
  matchesAny,
} from '../../scripts/check/check-duplicate-content.mjs'

const repoRoot = execFileSync('git', ['rev-parse', '--show-toplevel'], { encoding: 'utf8' }).trim()
const fixtureRoots = new Set()

afterEach(() => {
  for (const root of fixtureRoots) fs.rmSync(root, { recursive: true, force: true })
  fixtureRoots.clear()
})

function fixture(files) {
  const root = fs.mkdtempSync(join(os.tmpdir(), 'angel-duplicate-content-'))
  fixtureRoots.add(root)
  for (const [rel, body] of Object.entries(files)) {
    const abs = join(root, rel)
    fs.mkdirSync(join(abs, '..'), { recursive: true })
    fs.writeFileSync(abs, body)
  }
  return root
}

const member = (path, size = 10) => ({ path, size, digest: 'x'.repeat(64) })

test('globToRegExp anchors and treats ** as zero or more segments', () => {
  assert.ok(globToRegExp('a/**/b').test('a/b'))
  assert.ok(globToRegExp('a/**/b').test('a/x/b'))
  assert.ok(globToRegExp('a/**/b').test('a/x/y/b'))
  assert.ok(!globToRegExp('a/**/b').test('a/x/y'))
  assert.ok(!globToRegExp('a/*/b').test('a/x/y/b'))
  assert.ok(globToRegExp('a/*/b').test('a/x/b'))
  assert.ok(!globToRegExp('b').test('a/b'))
  assert.ok(matchesAny('cockpit/research/sloptomizer/x/y.py', ['cockpit/research/sloptomizer/**']))
})

test('a duplicate is unexplained until a rule names it', () => {
  const unexplained = [member('src/a.py'), member('src/b.py')]
  assert.equal(explainGroup(unexplained), null)

  const markers = [member('pkg/__init__.py', 1), member('pkg/sub/__init__.py', 1)]
  assert.equal(explainGroup(markers).id, 'python-package-markers')

  const heavy = [member('pkg/__init__.py', 9000), member('pkg/sub/__init__.py', 9000)]
  assert.equal(explainGroup(heavy), null)
})

test('the embed/archive rule needs both sides of the pair', () => {
  const embed = 'cockpit/research/sloptomizer/orchestrator/self_improvement/stats.py'
  const archive = 'experimental/sloptomizer/upstream/orchestrator/self_improvement/stats.py'
  assert.equal(explainGroup([member(embed), member(archive)]).id, 'research-embed-archive-pair')
  assert.equal(
    explainGroup([member(embed), member('cockpit/research/sloptomizer/orchestrator/x.py')]),
    null,
  )
})

test('exact_paths covers only the named pair', () => {
  const pair = ['cockpit/assets/agents/codex-active.png', 'cockpit/assets/agents/codex-neutral.png']
  assert.equal(explainGroup(pair.map((p) => member(p, 2644740))).id, 'retained-portrait-generation')
  assert.equal(
    explainGroup(
      [...pair, 'cockpit/assets/agents/sparky-neutral.png'].map((p) => member(p, 2644740)),
    ),
    null,
  )
})

test('a synthetic tree with a copied source file fails the gate', () => {
  const root = fixture({
    'src/one.py': 'print("same")\n',
    'src/two.py': 'print("same")\n',
    'src/three.py': 'print("different")\n',
  })
  const report = auditDuplicateContent(root, {
    files: ['src/one.py', 'src/two.py', 'src/three.py'],
  })
  assert.equal(report.status, 'fail')
  assert.equal(report.unexplained_groups, 1)
  assert.deepEqual(
    report.unexplained[0].members.map((m) => m.path),
    ['src/one.py', 'src/two.py'],
  )
  assert.equal(report.redundant_bytes, 'print("same")\n'.length)
})

test('only shared sizes are hashed; equal size alone is not a duplicate', (t) => {
  const files = {
    'unique-size.bin': Buffer.alloc(8193, 7),
    'same-a.bin': Buffer.from([0, 255, 128, 13]),
    'same-b.bin': Buffer.from([0, 255, 128, 13]),
    'same-size-different.bin': Buffer.from([255, 0, 128, 13]),
    'empty-a': '',
    'empty-b': '',
  }
  const root = fixture(files)
  fs.mkdirSync(join(root, 'directory'))
  const read = fs.readFileSync
  const reads = []
  const mock = t.mock.method(fs, 'readFileSync', function (path, ...args) {
    reads.push(path)
    return read.call(this, path, ...args)
  })
  syncBuiltinESMExports()
  let groups
  try {
    groups = findDuplicateGroups(root, [...Object.keys(files), 'missing', 'directory'].reverse())
  } finally {
    mock.mock.restore()
    syncBuiltinESMExports()
  }
  assert.deepEqual(reads.sort(), Object.keys(files)
    .filter((path) => path !== 'unique-size.bin')
    .map((path) => join(root, path)).sort())
  assert.deepEqual(groups.map((group) => ({
    size: group.size,
    redundant: group.redundant_bytes,
    paths: group.members.map((entry) => entry.path),
  })), [
    { size: 4, redundant: 4, paths: ['same-a.bin', 'same-b.bin'] },
    { size: 0, redundant: 0, paths: ['empty-a', 'empty-b'] },
  ])
})

test('an empty or entirely unique-size inventory has no groups', () => {
  const root = fixture({ first: 'a', second: 'bb', third: '' })
  assert.deepEqual(findDuplicateGroups(root, []), [])
  assert.deepEqual(findDuplicateGroups(root, ['first', 'second', 'third']), [])
})

test('rule order is deterministic and later rules do not rescue earlier groups', () => {
  const ids = DEFAULT_RULES.map((r) => r.id)
  assert.deepEqual(ids, [...new Set(ids)], 'rule ids must be unique')
  assert.ok(
    DEFAULT_RULES.every((r) => r.reason && r.reason.length > 40),
    'every rule states a reason',
  )
})

test('the checked-in tree explains every duplicate group it contains', () => {
  const report = auditDuplicateContent(repoRoot)
  assert.equal(report.unexplained_groups, 0, JSON.stringify(report.unexplained, null, 2))
  assert.equal(report.status, 'pass')
  for (const group of report.explained) {
    assert.ok(group.members.length >= 2)
    assert.ok(group.rule_id)
  }
})
