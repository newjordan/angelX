#!/usr/bin/env node
import { createHash } from 'node:crypto'
import { existsSync, mkdirSync, readFileSync, statSync, writeFileSync } from 'node:fs'
import { dirname, relative, resolve } from 'node:path'
import { pathToFileURL } from 'node:url'

const SCRIPT = 'check-research-embed'
const EMBED_DIR = 'cockpit/research/sloptomizer'
const RECEIPT_PATH = `${EMBED_DIR}/UPSTREAM.json`
const BRIDGE_PATH = 'cockpit/src/drive/rl_ctl/research_bridge.rs'
const ARCHIVE_DIR = 'experimental/sloptomizer/upstream'
const ARCHIVE_RECEIPT_PATH = `${ARCHIVE_DIR}/source-receipt.json`
const EMBED_EXTRAS = ['runner.py', 'relations.py', 'UPSTREAM.json']
const isAdaptationPath = (path) => path.endsWith('__init__.py')

const sha256 = (path) => createHash('sha256').update(readFileSync(path)).digest('hex')

export function embeddedPaths(bridgeSource) {
  const found = []
  const pattern = /include_bytes!\(\s*"(?:\.\.\/)+research\/sloptomizer\/([^"]+)"\s*\)/gu
  for (const match of bridgeSource.matchAll(pattern)) found.push(match[1])
  return found
}

export function auditResearchEmbed(root) {
  const at = (rel) => resolve(root, rel)
  const findings = []
  const skipped = []
  const checked = []

  if (!existsSync(at(RECEIPT_PATH))) {
    throw new Error(`missing research embed receipt: ${RECEIPT_PATH}`)
  }
  const receipt = JSON.parse(readFileSync(at(RECEIPT_PATH), 'utf8'))
  const receipted = receipt.files || []
  if (!receipted.length) throw new Error(`${RECEIPT_PATH} lists no files`)
  if (!/^[0-9a-f]{40}$/u.test(receipt.source_head || '')) {
    findings.push(`${RECEIPT_PATH}: source_head is not a full commit sha`)
  }

  for (const entry of receipted) {
    if (
      Object.hasOwn(entry, 'upstream_sha256') &&
      (typeof entry.upstream_sha256 !== 'string' || !/^[0-9a-f]{64}$/u.test(entry.upstream_sha256))
    ) {
      findings.push(`${RECEIPT_PATH}: invalid upstream_sha256 for ${entry.path}`)
    }
    const abs = at(`${EMBED_DIR}/${entry.path}`)
    if (!existsSync(abs)) {
      findings.push(`embed missing receipted file: ${EMBED_DIR}/${entry.path}`)
      continue
    }
    const digest = sha256(abs)
    if (digest !== entry.sha256) {
      findings.push(
        `embed drifted from receipt: ${EMBED_DIR}/${entry.path}\n    receipt ${entry.sha256}\n    actual  ${digest}`,
      )
      continue
    }
    checked.push({ path: entry.path, sha256: digest, embed: true })
  }

  const bridge = existsSync(at(BRIDGE_PATH)) ? readFileSync(at(BRIDGE_PATH), 'utf8') : null
  if (bridge === null) {
    skipped.push(`no ${BRIDGE_PATH}; skipped embed-table comparison`)
  } else {
    const declared = embeddedPaths(bridge)
    const declaredSet = new Set(declared)
    if (declared.length !== declaredSet.size) {
      findings.push(`${BRIDGE_PATH}: duplicate include_bytes! entries`)
    }
    const expected = new Set([...receipted.map((e) => e.path), ...EMBED_EXTRAS])
    for (const path of declaredSet) if (isAdaptationPath(path)) expected.add(path)
    for (const path of expected) {
      if (!declaredSet.has(path)) findings.push(`${BRIDGE_PATH}: does not embed ${path}`)
    }
    for (const path of declaredSet) {
      if (!expected.has(path)) {
        findings.push(`${BRIDGE_PATH}: embeds ${path}, which no receipt accounts for`)
      }
    }
    for (const path of declaredSet) {
      if (!isAdaptationPath(path)) continue
      const abs = at(`${EMBED_DIR}/${path}`)
      if (!existsSync(abs) || !statSync(abs).isFile()) {
        findings.push(`${BRIDGE_PATH}: adapts missing file ${EMBED_DIR}/${path}`)
      }
      if (receipted.some((e) => e.path === path)) {
        findings.push(
          `${RECEIPT_PATH}: receipts ${path}, which the adaptation note declares rewritten`,
        )
      }
    }
  }

  const archivePresent = existsSync(at(ARCHIVE_DIR))
  if (!archivePresent) {
    skipped.push(`no ${ARCHIVE_DIR}; this tree ships the embed only`)
  } else {
    const archiveReceiptPath = at(ARCHIVE_RECEIPT_PATH)
    const archiveReceiptPresent = existsSync(archiveReceiptPath)
    const archiveReceipt = archiveReceiptPresent
      ? JSON.parse(readFileSync(archiveReceiptPath, 'utf8'))
      : null
    if (!archiveReceiptPresent) {
      skipped.push(`no ${ARCHIVE_RECEIPT_PATH}; skipped archive hash comparison`)
    }
    const recorded = new Map((archiveReceipt?.files || []).map((e) => [e.path, e.sha256]))
    for (const entry of receipted) {
      const abs = at(`${ARCHIVE_DIR}/${entry.path}`)
      if (!existsSync(abs)) {
        findings.push(`archive missing receipted file: ${ARCHIVE_DIR}/${entry.path}`)
        continue
      }
      const digest = sha256(abs)
      const expected = Object.hasOwn(entry, 'upstream_sha256') ? entry.upstream_sha256 : entry.sha256
      if (digest !== expected) {
        const identity = Object.hasOwn(entry, 'upstream_sha256') ? 'upstream' : 'embed  '
        findings.push(
          `embed/archive fork: ${entry.path}\n    ${identity} ${expected}\n    archive ${digest}`,
        )
        continue
      }
      if (archiveReceiptPresent && !recorded.has(entry.path)) {
        findings.push(`${ARCHIVE_RECEIPT_PATH}: does not list ${entry.path}`)
      } else if (archiveReceiptPresent && recorded.get(entry.path) !== digest) {
        findings.push(
          `archive tampered since import: ${entry.path}\n    recorded ${recorded.get(entry.path)}\n    actual   ${digest}`,
        )
      }
      checked.push({ path: entry.path, sha256: digest, embed: true, archive: true })
    }
  }

  return {
    script: SCRIPT,
    root: relative(process.cwd(), root) || '.',
    source_head: receipt.source_head,
    receipted_files: receipted.length,
    verified: checked.length,
    archive_present: archivePresent,
    skipped,
    findings,
    status: findings.length ? 'fail' : 'pass',
  }
}

function parseArgs(argv) {
  const opts = { root: process.cwd(), json: false, receipt: null }
  for (let i = 0; i < argv.length; i += 1) {
    const arg = argv[i]
    if (arg === '--root') opts.root = resolve(argv[++i])
    else if (arg === '--json') opts.json = true
    else if (arg === '--receipt') opts.receipt = resolve(argv[++i])
    else if (arg === '--help' || arg === '-h') opts.help = true
    else throw new Error(`unknown argument: ${arg}`)
  }
  return opts
}

function render(report) {
  const lines = [
    `research embed: ${report.status}`,
    `  source head      ${report.source_head}`,
    `  receipted files  ${report.receipted_files}`,
    `  verified         ${report.verified}`,
    `  archive present  ${report.archive_present}`,
  ]
  for (const note of report.skipped) lines.push(`  skip ${note}`)
  for (const finding of report.findings) lines.push(`  FAIL ${finding}`)
  if (report.findings.length) {
    lines.push('', 'The research embed or its provenance no longer matches the receipts.')
    lines.push(`Restore bundled bytes to sha256 in ${RECEIPT_PATH}; preserve approved repairs.`)
    lines.push('Restore archive bytes to upstream_sha256 (or sha256 when no original is recorded).')
    lines.push('Do not overwrite the provenance archive with a bundled repair.')
  }
  return lines.join('\n')
}

export function run(argv = process.argv.slice(2)) {
  const opts = parseArgs(argv)
  if (opts.help) {
    console.log('usage: check-research-embed.mjs [--root DIR] [--json] [--receipt FILE]')
    return 0
  }
  const report = auditResearchEmbed(opts.root)
  if (opts.receipt) {
    mkdirSync(dirname(opts.receipt), { recursive: true })
    writeFileSync(opts.receipt, `${JSON.stringify(report, null, 2)}\n`)
  }
  console.log(opts.json ? JSON.stringify(report, null, 2) : render(report))
  return report.status === 'pass' ? 0 : 1
}

if (process.argv[1] && import.meta.url === pathToFileURL(resolve(process.argv[1])).href) {
  try {
    process.exitCode = run()
  } catch (error) {
    console.error(`${SCRIPT}: ${error.message}`)
    process.exitCode = 2
  }
}
