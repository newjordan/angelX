// The license gate is only worth having if its rejection path works, so these
// tests pin the SPDX semantics that decide allow/refuse — especially the
// disjunction case (`Apache-2.0 OR GPL-2.0-only`), where a substring match on
// "GPL" would fail a dependency that carries no obligation at all, and the
// conjunction case, where the same substring would wrongly pass one.
import { test } from 'node:test'
import assert from 'node:assert/strict'

import {
  ALLOWED_EXCEPTIONS,
  ALLOWED_LICENSES,
  REFUSED_LICENSES,
  auditLicenses,
  evaluateExpression,
  parseExpression,
  tokenize,
} from '../../scripts/check/check-licenses.mjs'

const allowed = (expression) => evaluateExpression(expression).allowed

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
  // self_cell in the real graph declares exactly this.
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
