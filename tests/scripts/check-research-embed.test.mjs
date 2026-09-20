import { test } from 'node:test'
import assert from 'node:assert/strict'
import fs from 'node:fs'
import os from 'node:os'
import { execFileSync } from 'node:child_process'
import { createHash } from 'node:crypto'
import { join } from 'node:path'

import { auditResearchEmbed, embeddedPaths } from '../../scripts/check-research-embed.mjs'

const repoRoot = execFileSync('git', ['rev-parse', '--show-toplevel'], { encoding: 'utf8' }).trim()
const REL = 'orchestrator/self_improvement/stats.py'
const digest = (bytes) => createHash('sha256').update(bytes).digest('hex')

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
} = {}) {
  const root = fs.mkdtempSync(join(os.tmpdir(), 'angel-research-embed-'))
  write(root, `cockpit/research/sloptomizer/${REL}`, body)
  write(root, 'cockpit/research/sloptomizer/runner.py', 'runner\n')
  write(
    root,
    'cockpit/research/sloptomizer/UPSTREAM.json',
    JSON.stringify({
      source_head: 'a'.repeat(40),
      files: [{ path: REL, sha256: digest(body) }],
      adaptation: 'Package initializers restrict imports to the selected stdlib algorithms.',
    }),
  )
  if (bridge) {
    write(
      root,
      'cockpit/src/rl_ctl/research_bridge.rs',
      [
        'const FILES: &[(&str, &[u8])] = &[',
        '    ("runner.py", include_bytes!("../../research/sloptomizer/runner.py")),',
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

test('a bridge that stops embedding a receipted file fails', () => {
  const root = embedFixture()
  write(
    root,
    'cockpit/src/rl_ctl/research_bridge.rs',
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
  const bridge = fs.readFileSync(join(repoRoot, 'cockpit/src/rl_ctl/research_bridge.rs'), 'utf8')
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
