#!/usr/bin/env node
// Sloptomizer research-embed guard.
//
// The cockpit compiles a curated subset of the original Sloptomizer source into
// the binary with `include_bytes!` (see cockpit/src/drive/rl_ctl/research_bridge.rs),
// and the same bytes are retained in full under experimental/sloptomizer/upstream
// as the provenance archive. That arrangement is deliberate but fragile in one
// specific way: nothing stops the embedded copy and the archived copy from
// silently diverging, which would turn "the same file twice" into a fork with no
// receipt. This check closes that gap.
//
// It asserts four things:
//   1. every file receipted in cockpit/research/sloptomizer/UPSTREAM.json is present
//      in the embed and hashes to the recorded sha256;
//   2. the Rust `FILES` table embeds exactly that set (plus runner.py + UPSTREAM.json);
//   3. when the archival tree is present, each receipted path is byte-identical
//      there too (this is the duplicate-content guard proper);
//   4. when the archive receipt is present, the archived path hashes to the
//      sha256 the import recorded, so the archive itself is untampered.
//
// Checks 3 and 4 are skipped, not failed, in trees that do not carry the archive
// (the published tree ships the embed only). Offline, stdlib only. Exit codes:
//   0 pass, 1 mismatch, 2 could not run.
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
// Non-receipted files the runtime still needs. `files[]` in UPSTREAM.json covers
// only the modules copied byte-for-byte from upstream.
const EMBED_EXTRAS = ['runner.py', 'UPSTREAM.json']
// UPSTREAM.json's `adaptation` field: "Package initializers restrict imports to
// the selected stdlib algorithms." Those initializers are therefore rewritten,
// not copied, and carry no upstream hash.
const isAdaptationPath = (path) => path.endsWith('__init__.py')

const sha256 = (path) => createHash('sha256').update(readFileSync(path)).digest('hex')

/** Relative paths embedded by `include_bytes!` in research_bridge.rs, in order. */
export function embeddedPaths(bridgeSource) {
  const found = []
  // Depth-agnostic: the bridge's `../` prefix tracks how deep in `src/` it sits,
  // and a hard-coded depth silently matched nothing when the layers were added.
  const pattern = /include_bytes!\(\s*"(?:\.\.\/)+research\/sloptomizer\/([^"]+)"\s*\)/gu
  for (const match of bridgeSource.matchAll(pattern)) found.push(match[1])
  return found
}

/**
 * Compare the receipt, the embed and (when present) the archive.
 * Returns a report with `status` of 'pass' or 'fail'.
 */
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

  // 1. Embed matches the recorded hashes.
  for (const entry of receipted) {
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

  // 2. The binary embeds exactly the receipted set.
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

  // 3 + 4. Archive pair: identical bytes, and faithful to the import receipt.
  const archivePresent = existsSync(at(ARCHIVE_DIR))
  if (!archivePresent) {
    skipped.push(`no ${ARCHIVE_DIR}; this tree ships the embed only`)
  } else {
    const archiveReceiptPath = at(ARCHIVE_RECEIPT_PATH)
    const archiveReceipt = existsSync(archiveReceiptPath)
      ? JSON.parse(readFileSync(archiveReceiptPath, 'utf8'))
      : null
    if (archiveReceipt === null) {
      skipped.push(`no ${ARCHIVE_RECEIPT_PATH}; skipped archive hash comparison`)
    }
    const recorded = new Map((archiveReceipt?.files || []).map((e) => [e.path, e.sha256]))
    for (const entry of receipted) {
      const abs = at(`${ARCHIVE_DIR}/${entry.path}`)
      if (!existsSync(abs)) {
        // The archive is a partial import; absence here is not a mismatch as long
        // as the embedded copy already matched its own receipt above.
        findings.push(`archive missing receipted file: ${ARCHIVE_DIR}/${entry.path}`)
        continue
      }
      const digest = sha256(abs)
      if (digest !== entry.sha256) {
        findings.push(
          `embed/archive fork: ${entry.path}\n    embed   ${entry.sha256}\n    archive ${digest}`,
        )
        continue
      }
      if (recorded.size && !recorded.has(entry.path)) {
        findings.push(`${ARCHIVE_RECEIPT_PATH}: does not list ${entry.path}`)
      } else if (recorded.size && recorded.get(entry.path) !== digest) {
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
    lines.push('', 'The embedded runtime copy and its receipted source have diverged.')
    lines.push('Restore the file from experimental/sloptomizer/upstream (or the upstream commit')
    lines.push(`named in ${RECEIPT_PATH}) and update the receipt in the same change.`)
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
