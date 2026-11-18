import { afterEach, test } from 'node:test'
import assert from 'node:assert/strict'
import fs from 'node:fs'
import os from 'node:os'
import { execFileSync, spawnSync } from 'node:child_process'
import { createHash } from 'node:crypto'
import { join } from 'node:path'

import { auditResearchEmbed, embeddedPaths } from '../../scripts/check/check-research-embed.mjs'

const repoRoot = execFileSync('git', ['rev-parse', '--show-toplevel'], { encoding: 'utf8' }).trim()
const REL = 'orchestrator/self_improvement/stats.py'
const digest = (bytes) => createHash('sha256').update(bytes).digest('hex')
const fixtureRoots = new Set()

afterEach(() => {
  for (const root of fixtureRoots) fs.rmSync(root, { recursive: true, force: true })
  fixtureRoots.clear()
})

function write(root, rel, body) {
  const abs = join(root, rel)
  fs.mkdirSync(join(abs, '..'), { recursive: true })
  fs.writeFileSync(abs, body)
}

function embedFixture({
  body = 'STATS = 1\n',
  bridge = true,
  archive = null,
  recorded = null,
  upstream = undefined,
} = {}) {
  const root = fs.mkdtempSync(join(os.tmpdir(), 'angel-research-embed-'))
  fixtureRoots.add(root)
  write(root, `cockpit/research/sloptomizer/${REL}`, body)
  write(root, 'cockpit/research/sloptomizer/runner.py', 'runner\n')
  write(root, 'cockpit/research/sloptomizer/relations.py', 'relations\n')
  write(
    root,
    'cockpit/research/sloptomizer/UPSTREAM.json',
    JSON.stringify({
      source_head: 'a'.repeat(40),
      files: [{
        path: REL,
        sha256: digest(body),
        ...(upstream === undefined ? {} : { upstream_sha256: upstream }),
      }],
      adaptation: 'Package initializers restrict imports to the selected stdlib algorithms.',
    }),
  )
  if (bridge) {
    write(
      root,
      'cockpit/src/drive/rl_ctl/research_bridge.rs',
      [
        'const FILES: &[(&str, &[u8])] = &[',
        '    ("runner.py", include_bytes!("../../research/sloptomizer/runner.py")),',
        '    ("relations.py", include_bytes!("../../research/sloptomizer/relations.py")),',
        '    ("UPSTREAM.json", include_bytes!("../../research/sloptomizer/UPSTREAM.json")),',
        `    ("${REL}", include_bytes!("../../research/sloptomizer/${REL}")),`,
        '];',
        '',
      ].join('\n'),
    )
  }
  if (archive !== null) write(root, `experimental/sloptomizer/upstream/${REL}`, archive)
  if (recorded !== null) {
    write(
      root,
      'experimental/sloptomizer/upstream/source-receipt.json',
      JSON.stringify({ head: 'a'.repeat(40), files: [{ path: REL, sha256: recorded }] }),
    )
  }
  return root
}

test('a consistent embed with no archive passes and says so', () => {
  const report = auditResearchEmbed(embedFixture())
  assert.equal(report.status, 'pass')
  assert.equal(report.archive_present, false)
  assert.equal(report.verified, 1)
  assert.ok(report.skipped.some((note) => note.includes('ships the embed only')))
})

test('a drifted embed fails against the recorded sha256', () => {
  const root = embedFixture()
  write(root, `cockpit/research/sloptomizer/${REL}`, 'STATS = 2\n')
  const report = auditResearchEmbed(root)
  assert.equal(report.status, 'fail')
  assert.ok(report.findings.some((f) => f.includes('embed drifted from receipt')))
})

test('a missing receipted file is a finding, not a crash', () => {
  const root = embedFixture()
  fs.rmSync(join(root, `cockpit/research/sloptomizer/${REL}`))
  const report = auditResearchEmbed(root)
  assert.equal(report.status, 'fail')
  assert.ok(report.findings.some((f) => f.includes('embed missing receipted file')))
})

test('an identical archive copy passes and is counted twice', () => {
  const body = 'STATS = 1\n'
  const report = auditResearchEmbed(embedFixture({ body, archive: body, recorded: digest(body) }))
  assert.equal(report.status, 'pass')
  assert.equal(report.archive_present, true)
  assert.equal(report.verified, 2)
})

test('an archive copy that forks from the embed fails', () => {
  const report = auditResearchEmbed(embedFixture({ archive: 'STATS = 9\n' }))
  assert.equal(report.status, 'fail')
  assert.ok(report.findings.some((f) => f.includes('embed/archive fork')))
})

test('an archive whose bytes no longer match the import receipt fails', () => {
  const body = 'STATS = 1\n'
  const report = auditResearchEmbed(
    embedFixture({ body, archive: body, recorded: digest('something else\n') }),
  )
  assert.equal(report.status, 'fail')
  assert.ok(report.findings.some((f) => f.includes('archive tampered since import')))
})

test('an approved bundled repair keeps the original archive identity', () => {
  const original = 'STATS = 1\n'
  const report = auditResearchEmbed(embedFixture({
    body: 'STATS = 2  # approved repair\n',
    upstream: digest(original),
    archive: original,
    recorded: digest(original),
  }))
  assert.equal(report.status, 'pass', JSON.stringify(report.findings))
  assert.equal(report.verified, 2)
})

test('an approved repair without an archive still validates only the bundled bytes', () => {
  const report = auditResearchEmbed(embedFixture({ upstream: digest('original\n') }))
  assert.equal(report.status, 'pass')
  assert.equal(report.archive_present, false)
})

test('upstream identity does not excuse drift in the embedded repair', () => {
  const original = 'STATS = 1\n'
  const root = embedFixture({
    body: 'STATS = 2\n',
    upstream: digest(original),
    archive: original,
    recorded: digest(original),
  })
  write(root, `cockpit/research/sloptomizer/${REL}`, original)
  const report = auditResearchEmbed(root)
  assert.equal(report.status, 'fail')
  assert.ok(report.findings.some((f) => f.includes('embed drifted from receipt')))
})

test('an archive replaced with the bundled repair fails even with a matching import receipt', () => {
  const repaired = 'STATS = 2\n'
  const report = auditResearchEmbed(embedFixture({
    body: repaired,
    upstream: digest('STATS = 1\n'),
    archive: repaired,
    recorded: digest(repaired),
  }))
  assert.equal(report.status, 'fail')
})

test('a present empty or null import receipt cannot disable archive checks', () => {
  for (const receipt of [{ files: [] }, {}, null]) {
    const body = 'STATS = 1\n'
    const root = embedFixture({ body, archive: body })
    write(root, 'experimental/sloptomizer/upstream/source-receipt.json', JSON.stringify(receipt))
    const report = auditResearchEmbed(root)
    assert.equal(report.status, 'fail', JSON.stringify(receipt))
    assert.ok(report.findings.some((f) => f.includes('does not list')))
  }
})

test('an explicitly invalid original hash is rejected even without an archive', () => {
  for (const upstream of ['', null, 1, [], {}, 'a'.repeat(63), 'g'.repeat(64)]) {
    const report = auditResearchEmbed(embedFixture({ upstream }))
    assert.equal(report.status, 'fail', JSON.stringify(upstream))
    assert.ok(report.findings.some((f) => f.includes('invalid upstream_sha256')))
  }
})

test('failure advice preserves approved repairs instead of overwriting the archive', () => {
  const body = 'STATS = 2\n'
  const root = embedFixture({ body, upstream: digest('STATS = 1\n'), archive: body })
  const result = spawnSync(process.execPath, [
    join(repoRoot, 'scripts/check/check-research-embed.mjs'), '--root', root,
  ], { encoding: 'utf8' })
  assert.equal(result.status, 1)
  assert.equal(result.stderr, '')
  assert.match(result.stdout, /Restore bundled bytes to sha256/)
  assert.match(result.stdout, /Restore archive bytes to upstream_sha256/)
  assert.match(result.stdout, /Do not overwrite the provenance archive with a bundled repair/)
})

test('a bridge that stops embedding a receipted file fails', () => {
  const root = embedFixture()
  write(
    root,
    'cockpit/src/drive/rl_ctl/research_bridge.rs',
    [
      'const FILES: &[(&str, &[u8])] = &[',
      '    ("runner.py", include_bytes!("../../research/sloptomizer/runner.py")),',
      '    ("UPSTREAM.json", include_bytes!("../../research/sloptomizer/UPSTREAM.json")),',
      '];',
      '',
    ].join('\n'),
  )
  const report = auditResearchEmbed(root)
  assert.equal(report.status, 'fail')
  assert.ok(report.findings.some((f) => f.includes('does not embed')))
})

test('a tree without the bridge skips the embed-table comparison', () => {
  const report = auditResearchEmbed(embedFixture({ bridge: false }))
  assert.equal(report.status, 'pass')
  assert.ok(report.skipped.some((note) => note.includes('skipped embed-table comparison')))
})

test('the checked-in bridge embeds the whole receipted set', () => {
  const bridge = fs.readFileSync(
    join(repoRoot, 'cockpit/src/drive/rl_ctl/research_bridge.rs'),
    'utf8',
  )
  const paths = embeddedPaths(bridge)
  assert.equal(paths.length, new Set(paths).size, 'no duplicate include_bytes! entries')
  const receipt = JSON.parse(
    fs.readFileSync(join(repoRoot, 'cockpit/research/sloptomizer/UPSTREAM.json'), 'utf8'),
  )
  for (const entry of receipt.files) assert.ok(paths.includes(entry.path), entry.path)
})

test('the checked-in research embed matches its receipt', () => {
  const report = auditResearchEmbed(repoRoot)
  assert.equal(report.status, 'pass', JSON.stringify(report.findings, null, 2))
  assert.ok(report.verified >= report.receipted_files)
})
