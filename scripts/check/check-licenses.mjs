#!/usr/bin/env node
// Third-party license gate.
//
// The release evidence already proves that every dependency *has* license
// metadata. This gate answers the different question: is the license itself one
// we may redistribute without obligation? It is default-deny — an identifier
// the allowlist does not name fails the gate — so a new copyleft or unfamiliar
// license stops the release instead of shipping unnoticed.
//
// SPDX expression handling, so a disjunction is not mistaken for a violation:
//   OR    passes when either side passes   (Apache-2.0 OR GPL-2.0-only passes)
//   AND   passes only when both sides pass (MIT AND GPL-2.0-only fails)
//   WITH  passes only when the base license and the named exception are allowed
//   ()    groups
// The legacy `A/B` disjunction that crates and npm emit is read as OR.
//
// Offline and dependency-free (stdlib + `cargo metadata`). Exit codes:
//   0  every dependency is under an allowed license
//   1  a dependency is denied, unrecognized, or carries no license
//   2  the check could not run
import { execFileSync } from 'node:child_process'
import { readFileSync } from 'node:fs'
import { join, resolve } from 'node:path'
import { pathToFileURL } from 'node:url'

const SCRIPT = 'check-licenses'

// Permissive licenses: redistribute freely, and the only obligations are the
// notice retention the release artifact already performs.
export const ALLOWED_LICENSES = new Set(
  [
    '0bsd',
    'apache-2.0',
    'blueoak-1.0.0',
    'bsd-2-clause',
    'bsd-3-clause',
    'bsl-1.0',
    'cc0-1.0',
    'cdla-permissive-2.0',
    'isc',
    'mit',
    'mit-0',
    'unlicense',
    'unicode-3.0',
    'unicode-dfs-2016',
    'wtfpl',
    'x11',
    'zlib',
  ].map((id) => id.toLowerCase()),
)

// Exceptions that leave a permissive base license permissive. `Apache-2.0 WITH
// LLVM-exception` is Apache-2.0 with a GCC-style runtime-linking carve-out.
export const ALLOWED_EXCEPTIONS = new Set(['llvm-exception'])

// Licenses that are recognized and deliberately refused. Naming them separately
// from an unknown identifier keeps the failure message useful: this one needs a
// policy decision, that one needs a human to read a license they have never
// seen. File-level copyleft (MPL-2.0, CDDL, EPL) sits here too — it is only
// ever one file away from a source-disclosure obligation, so including it is a
// deliberate change to this list, not an accident.
export const REFUSED_LICENSES = new Set(
  [
    'agpl-1.0',
    'agpl-3.0',
    'agpl-3.0-only',
    'agpl-3.0-or-later',
    'cc-by-nc-4.0',
    'cc-by-sa-4.0',
    'cddl-1.0',
    'cddl-1.1',
    'cpal-1.0',
    'epl-1.0',
    'epl-2.0',
    'eupl-1.2',
    'gpl-2.0',
    'gpl-2.0-only',
    'gpl-2.0-or-later',
    'gpl-3.0',
    'gpl-3.0-only',
    'gpl-3.0-or-later',
    'lgpl-2.1',
    'lgpl-2.1-only',
    'lgpl-2.1-or-later',
    'lgpl-3.0',
    'lgpl-3.0-only',
    'lgpl-3.0-or-later',
    'mpl-2.0',
    'osl-3.0',
    'sspl-1.0',
  ].map((id) => id.toLowerCase()),
)

/** Lowercase an SPDX identifier so matching is case-insensitive. */
export function normalizeId(raw) {
  return String(raw).trim().toLowerCase()
}

/**
 * Split an SPDX expression into tokens. The legacy `A/B` and comma-joined forms
 * are disjunctions, so they become OR.
 */
export function tokenize(expression) {
  const spaced = String(expression)
    .replace(/[()]/g, (m) => ` ${m} `)
    .replace(/[/,]/g, ' OR ')
  return spaced.split(/\s+/).filter(Boolean)
}

/** Parse tokens into an expression tree: {kind:'id'|'with'|'or'|'and'}. */
export function parseExpression(tokens) {
  let at = 0
  const peek = () => tokens[at]
  const take = () => tokens[at++]

  function parseFactor() {
    const token = peek()
    if (token === undefined) throw new Error('unexpected end of SPDX expression')
    if (token === '(') {
      take()
      const inner = parseOr()
      if (take() !== ')') throw new Error('unbalanced parenthesis in SPDX expression')
      return inner
    }
    const id = take()
    if (peek() && peek().toUpperCase() === 'WITH') {
      take()
      const exception = take()
      if (exception === undefined) throw new Error(`WITH without an exception after ${id}`)
      return { kind: 'with', id, exception }
    }
    return { kind: 'id', id }
  }

  function parseAnd() {
    let node = parseFactor()
    while (peek() && peek().toUpperCase() === 'AND') {
      take()
      node = { kind: 'and', left: node, right: parseFactor() }
    }
    return node
  }

  function parseOr() {
    let node = parseAnd()
    while (peek() && peek().toUpperCase() === 'OR') {
      take()
      node = { kind: 'or', left: node, right: parseAnd() }
    }
    return node
  }

  const tree = parseOr()
  if (at !== tokens.length)
    throw new Error(`trailing tokens in SPDX expression: ${tokens.slice(at).join(' ')}`)
  return tree
}

/**
 * Decide one expression. `denied` and `unknown` collect the identifiers that
 * made it fail, so the report can say which arm of a disjunction is at fault.
 */
export function evaluateExpression(expression) {
  const denied = new Set()
  const unknown = new Set()

  function decide(node) {
    switch (node.kind) {
      case 'id': {
        const id = normalizeId(node.id)
        if (ALLOWED_LICENSES.has(id)) return true
        if (REFUSED_LICENSES.has(id)) denied.add(id)
        else unknown.add(id)
        return false
      }
      case 'with': {
        const id = normalizeId(node.id)
        const exception = normalizeId(node.exception)
        const baseOk = decide({ kind: 'id', id: node.id })
        if (!ALLOWED_EXCEPTIONS.has(exception)) {
          if (REFUSED_LICENSES.has(exception)) denied.add(`${id} WITH ${exception}`)
          else unknown.add(`${id} WITH ${exception}`)
          return false
        }
        return baseOk
      }
      case 'or':
        // Both arms are evaluated: a disjunction reports every arm it could not
        // use, even when the other arm carries the result.
        return [decide(node.left), decide(node.right)].some(Boolean)
      case 'and':
        return [decide(node.left), decide(node.right)].every(Boolean)
      default:
        throw new Error(`unknown expression node: ${node.kind}`)
    }
  }

  const allowed = decide(parseExpression(tokenize(expression)))
  return { allowed, denied: [...denied].sort(), unknown: [...unknown].sort() }
}

/**
 * Audit dependency records. Each record is `{name, version, license}`; a record
 * without a license is a violation of its own because nothing can be checked.
 * Returns violations and a per-expression tally for the summary.
 */
export function auditLicenses(records) {
  const violations = []
  const byExpression = new Map()

  const countExpression = (expression, key) => {
    const entry = byExpression.get(expression) ?? { allowed: 0, refused: 0 }
    entry[key] += 1
    byExpression.set(expression, entry)
  }

  for (const record of records) {
    const license = typeof record.license === 'string' ? record.license.trim() : ''
    if (!license) {
      violations.push({
        ...record,
        kind: 'missing',
        detail: 'no license metadata',
      })
      continue
    }
    let result
    try {
      result = evaluateExpression(license)
    } catch (error) {
      // A malformed expression is a violation of this dependency, not a reason
      // to abandon the run: one unreadable string must not hide the rest.
      countExpression(license, 'refused')
      violations.push({ ...record, kind: 'unparsable', detail: error.message })
      continue
    }
    if (result.allowed) {
      countExpression(license, 'allowed')
      continue
    }
    countExpression(license, 'refused')
    violations.push({
      ...record,
      kind: result.denied.length > 0 ? 'refused' : 'unrecognized',
      detail: [...result.denied, ...result.unknown].join(', '),
    })
  }

  violations.sort((a, b) => (a.name < b.name ? -1 : a.name > b.name ? 1 : 0))
  return { violations, byExpression, inspected: records.length }
}

/** Every Rust package cargo resolved for the supported target. */
export function cargoRecords(repoRoot) {
  const text = execFileSync(
    'cargo',
    [
      'metadata',
      '--manifest-path',
      join(repoRoot, 'cockpit', 'Cargo.toml'),
      '--locked',
      '--offline',
      '--filter-platform',
      'x86_64-unknown-linux-gnu',
      '--format-version',
      '1',
    ],
    { cwd: repoRoot, encoding: 'utf8', maxBuffer: 256 * 1024 * 1024 },
  )
  const metadata = JSON.parse(text)
  return metadata.packages.map((pkg) => ({
    name: pkg.name,
    version: pkg.version,
    license: pkg.license ?? '',
    ecosystem: 'cargo',
  }))
}

/** Every npm package in the lockfile. */
export function npmRecords(repoRoot) {
  const lock = JSON.parse(readFileSync(join(repoRoot, 'package-lock.json'), 'utf8'))
  return Object.entries(lock.packages ?? {})
    .filter(([path]) => path !== '')
    .map(([path, meta]) => ({
      name: path.replace(/^.*node_modules\//, ''),
      version: meta.version ?? '',
      license: meta.license ?? '',
      ecosystem: 'npm',
    }))
}

function run(repoRoot) {
  let records
  try {
    records = [...cargoRecords(repoRoot), ...npmRecords(repoRoot)]
  } catch (error) {
    console.error(`${SCRIPT}: could not collect dependency licenses: ${error.message}`)
    return 2
  }

  const { violations, byExpression, inspected } = auditLicenses(records)

  console.log(
    `${SCRIPT}: ${inspected} dependencies, ${byExpression.size} distinct license expressions`,
  )
  for (const [expression, tally] of [...byExpression.entries()].sort(
    (a, b) => b[1].allowed + b[1].refused - (a[1].allowed + a[1].refused),
  )) {
    const mark = tally.refused === 0 ? 'ok  ' : 'FAIL'
    console.log(`  ${mark} ${String(tally.allowed + tally.refused).padStart(4)}  ${expression}`)
  }

  if (violations.length === 0) {
    console.log(`${SCRIPT}: every dependency is under an allowed license`)
    return 0
  }

  console.error(`\n${SCRIPT}: ${violations.length} dependency license violation(s)`)
  for (const violation of violations) {
    const why =
      violation.kind === 'missing'
        ? 'no license metadata — nothing can be verified'
        : violation.kind === 'refused'
          ? `refused license: ${violation.detail}`
          : violation.kind === 'unparsable'
            ? `unreadable SPDX expression: ${violation.detail}`
            : `unrecognized identifier, needs a human to classify: ${violation.detail}`
    console.error(
      `  ${violation.ecosystem}  ${violation.name}@${violation.version}  [${violation.kind}] ${why}`,
    )
  }
  console.error(
    `\nRefused identifiers are listed in REFUSED_LICENSES; an unrecognized one must be read and\n` +
      `then either added to ALLOWED_LICENSES or to REFUSED_LICENSES in scripts/check/check-licenses.mjs.`,
  )
  return 1
}

if (process.argv[1] && import.meta.url === pathToFileURL(resolve(process.argv[1])).href) {
  const repoRoot = execFileSync('git', ['rev-parse', '--show-toplevel'], {
    encoding: 'utf8',
  }).trim()
  try {
    process.exitCode = run(repoRoot)
  } catch (error) {
    console.error(`${SCRIPT}: ${error.message}`)
    process.exitCode = 2
  }
}
