// tests pin the SPDX semantics that decide allow/refuse — especially the
import { test } from 'node:test'
import assert from 'node:assert/strict'
import { mkdtempSync, rmSync, writeFileSync } from 'node:fs'
import { tmpdir } from 'node:os'
import { join } from 'node:path'

import {
  ALLOWED_EXCEPTIONS,
  ALLOWED_LICENSES,
  REFUSED_LICENSES,
  auditLicenses,
  evaluateExpression,
  npmRecords,
  parseExpression,
  tokenize,
} from '../../scripts/check/check-licenses.mjs'

const allowed = (expression) => evaluateExpression(expression).allowed

test('npm workspace links defer to their audited target, never bypassing license checks', (t) => {
  const root = mkdtempSync(join(tmpdir(), 'angel-license-workspace-'))
  t.after(() => rmSync(root, { recursive: true, force: true }))
  const packages = {
    '': { name: 'root', license: 'MIT' },
    'node_modules/a': { resolved: 'packages/a', link: true },
    'node_modules/@scope/b': { resolved: 'packages/b', link: true },
    'packages/a': { version: '1.0.0', license: 'MIT' },
    'packages/b': { version: '2.0.0', license: 'Apache-2.0' },
    'node_modules/regular': { version: '3.0.0', license: 'ISC' },
  }
  const collect = () => {
    writeFileSync(join(root, 'package-lock.json'), JSON.stringify({ lockfileVersion: 3, packages }))
    return npmRecords(root)
  }
  const records = collect()
  assert.equal(records.length, 3, 'the links are not extra dependency records')
  assert.deepEqual(records.map((record) => record.version).sort(), ['1.0.0', '2.0.0', '3.0.0'])
  assert.deepEqual(auditLicenses(records).violations, [])
  packages['packages/a'].license = 'GPL-3.0-only'
  assert.deepEqual(auditLicenses(collect()).violations.map((item) => item.kind), ['refused'])
  delete packages['packages/a'].license
  assert.deepEqual(auditLicenses(collect()).violations.map((item) => item.kind), ['missing'])
  delete packages['packages/a']
  assert.deepEqual(auditLicenses(collect()).violations.map((item) => item.kind), ['missing'],
    'an absent target must not let a link evade the gate')
})

test('permissive licenses pass, case-insensitively', () => {
  for (const expression of [
    'MIT',
    'mit',
    'APACHE-2.0',
    'ISC',
    'Zlib',
    'BSD-3-Clause',
    'Unicode-3.0',
    'CDLA-Permissive-2.0',
  ]) {
    assert.equal(allowed(expression), true, `${expression} should be allowed`)
  }
})

test('a copyleft arm of a disjunction does not fail the dependency', () => {
  assert.equal(allowed('Apache-2.0 OR GPL-2.0-only'), true)
  assert.equal(allowed('MIT OR Apache-2.0 OR LGPL-2.1-or-later'), true)
  assert.equal(allowed('Apache-2.0 OR AGPL-3.0'), true)
})

test('a copyleft arm of a conjunction fails the dependency', () => {
  const result = evaluateExpression('MIT AND GPL-2.0-only')
  assert.equal(result.allowed, false)
  assert.deepEqual(result.denied, ['gpl-2.0-only'])
})

test('a copyleft license on its own is refused, not merely unrecognized', () => {
  for (const expression of [
    'GPL-2.0-only',
    'GPL-3.0-or-later',
    'AGPL-3.0',
    'SSPL-1.0',
    'EUPL-1.2',
    'CC-BY-NC-4.0',
  ]) {
    const result = evaluateExpression(expression)
    assert.equal(result.allowed, false, `${expression} should be refused`)
    assert.deepEqual(result.unknown, [], `${expression} should be a known refusal`)
    assert.equal(result.denied.length, 1)
  }
})

test('file-level copyleft is refused by policy, not by accident', () => {
  for (const expression of ['MPL-2.0', 'CDDL-1.0', 'EPL-2.0']) {
    const result = evaluateExpression(expression)
    assert.equal(result.allowed, false)
    assert.deepEqual(result.unknown, [], `${expression} must be a deliberate refusal`)
    assert.ok(REFUSED_LICENSES.has(expression.toLowerCase()))
  }
})

test('an unfamiliar identifier is refused and flagged for a human', () => {
  const result = evaluateExpression('Prosperity-3.0.0')
  assert.equal(result.allowed, false)
  assert.deepEqual(result.denied, [])
  assert.deepEqual(result.unknown, ['prosperity-3.0.0'])
  assert.ok(!ALLOWED_LICENSES.has('prosperity-3.0.0'))
})

test('parentheses group, and AND binds tighter than OR', () => {
  assert.equal(allowed('(Apache-2.0 OR MIT) AND BSD-3-Clause'), true)
  assert.equal(allowed('(MIT OR Apache-2.0) AND Unicode-3.0'), true)
  assert.equal(
    allowed('MIT OR Apache-2.0 AND GPL-3.0-only'),
    true,
    'OR yields because the MIT arm carries it',
  )
  assert.equal(allowed('(MIT OR Apache-2.0) AND GPL-3.0-only'), false)
})

test('WITH passes only for an allowlisted exception', () => {
  assert.equal(allowed('Apache-2.0 WITH LLVM-exception'), true)
  assert.equal(allowed('Apache-2.0 WITH LLVM-exception OR Apache-2.0 OR MIT'), true)
  assert.ok(ALLOWED_EXCEPTIONS.has('llvm-exception'))

  const classpath = evaluateExpression('Apache-2.0 WITH Classpath-exception-2.0')
  assert.equal(
    classpath.allowed,
    false,
    'an unlisted exception must not pass on its permissive base',
  )
  assert.deepEqual(classpath.unknown, ['apache-2.0 WITH classpath-exception-2.0'])
})

test('the legacy slash disjunction that crates and npm emit is read as OR', () => {
  assert.deepEqual(tokenize('MIT/Apache-2.0'), ['MIT', 'OR', 'Apache-2.0'])
  assert.equal(allowed('MIT/Apache-2.0'), true)
  assert.equal(allowed('Apache-2.0 / MIT'), true)
})

test('a malformed expression is reported, not thrown at the caller', () => {
  const report = auditLicenses([
    { name: 'dangling', version: '1.0.0', license: 'MIT OR' },
    { name: 'unbalanced', version: '1.0.0', license: '(MIT' },
  ])
  assert.deepEqual(
    report.violations.map((violation) => violation.kind),
    ['unparsable', 'unparsable'],
  )
  assert.equal(report.inspected, 2)
})

test('malformed SPDX factors cannot hide behind an allowed OR arm', () => {
  for (const license of ['MIT OR )', ') OR MIT', 'MIT OR OR', 'MIT OR AND', 'MIT OR WITH', '(MIT OR )', 'MIT OR Apache-2.0 WITH )', 'MIT OR Apache-2.0 WITH AND']) {
    assert.throws(() => evaluateExpression(license), /SPDX|exception/, license)
    const report = auditLicenses([{ name: 'malformed', version: '1', license }])
    assert.equal(report.violations.length, 1, license)
    assert.equal(report.violations[0].kind, 'unparsable', license)
    assert.deepEqual(report.byExpression.get(license), { allowed: 0, refused: 1 }, license)
  }
  assert.equal(evaluateExpression('MIT OR Future-License-1.0').allowed, true,
    'syntactically valid unknown alternatives retain the existing policy')
})

test('a missing license is its own violation kind', () => {
  const report = auditLicenses([{ name: 'bare', version: '0.1.0', license: '' }])
  assert.equal(report.violations.length, 1)
  assert.equal(report.violations[0].kind, 'missing')
  assert.equal(report.byExpression.size, 0)
})

test('parseExpression rejects trailing tokens instead of ignoring them', () => {
  assert.throws(() => parseExpression(tokenize('MIT APACHE-2.0')), /trailing tokens/)
  assert.throws(() => parseExpression(tokenize('Apache-2.0 WITH')), /WITH without an exception/)
})

test('the audit reports tallies per expression and an empty input is clean', () => {
  const report = auditLicenses([
    { name: 'a', version: '1.0.0', license: 'MIT OR Apache-2.0' },
    { name: 'b', version: '1.0.0', license: 'MIT OR Apache-2.0' },
    { name: 'c', version: '1.0.0', license: 'MPL-2.0' },
  ])
  assert.deepEqual(report.byExpression.get('MIT OR Apache-2.0'), {
    allowed: 2,
    refused: 0,
  })
  assert.deepEqual(report.byExpression.get('MPL-2.0'), {
    allowed: 0,
    refused: 1,
  })
  assert.equal(report.inspected, 3)
  assert.equal(report.violations.length, 1)
  assert.equal(report.violations[0].name, 'c')

  const empty = auditLicenses([])
  assert.deepEqual(empty.violations, [])
  assert.equal(empty.inspected, 0)
})

test('the allowlist and the refusal list do not overlap', () => {
  for (const id of ALLOWED_LICENSES) {
    assert.ok(!REFUSED_LICENSES.has(id), `${id} is both allowed and refused`)
  }
})
