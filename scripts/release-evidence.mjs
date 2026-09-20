#!/usr/bin/env node

import { spawnSync } from 'node:child_process'
import { createHash } from 'node:crypto'
import {
  chmodSync,
  existsSync,
  lstatSync,
  mkdirSync,
  readFileSync,
  realpathSync,
  renameSync,
  rmSync,
  statSync,
  writeFileSync,
} from 'node:fs'
import { basename, dirname, isAbsolute, join, relative, resolve, sep } from 'node:path'
import { pathToFileURL } from 'node:url'

export const RELEASE_SCHEMA = 'angel0-source-release-evidence/v1'
// The archive is the ordinary terminal cockpit and its headless task runtime.
export const RELEASE_SCOPE = 'ordinary-terminal-cockpit-source'
export const SUPPORTED_CARGO_TARGET = 'x86_64-unknown-linux-gnu'

// Files consumed by compile-time include_* macros (release build and unit
// tests). Keep these explicit: selecting all of cockpit/ also publishes
// operator-only skills, personas, fleet metadata, design work, and source
// artwork that the runner neither builds nor needs. The release test derives
// the include set from the packaged crates and fails when this list drifts.
export const REQUIRED_COCKPIT_EMBEDDED_FILES = Object.freeze([
  // Offline Sloptomizer options are embedded; no experimental checkout dependency.
  'cockpit/research/sloptomizer/runner.py',
  'cockpit/research/sloptomizer/UPSTREAM.json',
  'cockpit/research/sloptomizer/autoresearch/__init__.py',
  'cockpit/research/sloptomizer/autoresearch/gepa/__init__.py',
  'cockpit/research/sloptomizer/autoresearch/gepa/population.py',
  'cockpit/research/sloptomizer/autoresearch/gepa/select.py',
  'cockpit/research/sloptomizer/orchestrator/__init__.py',
  'cockpit/research/sloptomizer/orchestrator/self_improvement/__init__.py',
  'cockpit/research/sloptomizer/orchestrator/self_improvement/bandits.py',
  'cockpit/research/sloptomizer/orchestrator/self_improvement/stats.py',
  'cockpit/research/sloptomizer/orchestrator/micro_llm/__init__.py',
  'cockpit/research/sloptomizer/orchestrator/micro_llm/core.py',
  'cockpit/research/sloptomizer/orchestrator/caseops_codec.py',
  'cockpit/research/sloptomizer/orchestrator/caseops_crystal_codec.py',
  'cockpit/research/sloptomizer/orchestrator/caseops_taxonomy_codec.py',
  'cockpit/research/sloptomizer/orchestrator/caseops_vocab_analogy_codec.py',
  'cockpit/research/sloptomizer/orchestrator/caseops_vocab_tensor_codec.py',
  'cockpit/modules/agent.toml',
  'cockpit/modules/artifacts.toml',
  'cockpit/modules/core.toml',
  'cockpit/modules/graph.toml',
  'cockpit/modules/image.toml',
  'cockpit/modules/shell.toml',
  'cockpit/personas/architect/PERSONA.md',
  'cockpit/personas/code-help/PERSONA.md',
  'cockpit/personas/librarian/PERSONA.md',
  'cockpit/personas/reviewer/PERSONA.md',
  'cockpit/personas/security/PERSONA.md',
  'cockpit/personas/skeptic/PERSONA.md',
  'cockpit/personas/speed-freak/PERSONA.md',
  'cockpit/personas/treebeard/PERSONA.md',
  'cockpit/skills/benchmark-inference/SKILL.md',
  'cockpit/skills/competition-loop/SKILL.md',
  'cockpit/skills/gpu-fleet-recon/SKILL.md',
  'cockpit/skills/hf-model-ops/SKILL.md',
  'cockpit/skills/navigate-code/SKILL.md',
  'cockpit/skills/plan/SKILL.md',
  'cockpit/skills/quantize-model/SKILL.md',
  'cockpit/skills/requesting-code-review/SKILL.md',
  'cockpit/skills/research-answer/SKILL.md',
  'cockpit/skills/self-modify/SKILL.md',
  'cockpit/skills/serve-local-llm/SKILL.md',
  'cockpit/skills/simplify-code/SKILL.md',
  'cockpit/skills/systematic-debugging/SKILL.md',
  'cockpit/skills/test-driven-development/SKILL.md',
  'cockpit/skills/verify-changes/SKILL.md',
  'cockpit/skills/write-skill/SKILL.md',
  'cockpit/fixtures/agentviz-portal/empty-v1.json',
  'cockpit/fixtures/agentviz-portal/invalid-v1.json',
  'cockpit/fixtures/agentviz-portal/normal-v1.json',
  'cockpit/fixtures/agentviz-portal/oversized-v1.json',
  'cockpit/fixtures/agentviz-portal/stale-v1.json',
  'cockpit/fixtures/observatory/manifest-v2.json',
  'cockpit/assets/excalibur/rise.png',
  'cockpit/assets/realm/ambient/chapel-mountain-v2-source.png',
  'cockpit/assets/realm/ambient/chapel-mountain-v2.png',
  'cockpit/assets/realm/ambient/gatehouse-mountain-v2-source.png',
  'cockpit/assets/realm/ambient/gatehouse-mountain-v2.png',
  'cockpit/assets/realm/ambient/keep-waterfall-source.png',
  'cockpit/assets/realm/ambient/keep-waterfall.png',
  'cockpit/assets/realm/ambient/motion.json',
  'cockpit/assets/realm/ambient/observatory-mountain-source.png',
  'cockpit/assets/realm/ambient/observatory-mountain.png',
  'cockpit/assets/realm/ambient/rookery-mountain-v2-source.png',
  'cockpit/assets/realm/ambient/rookery-mountain-v2.png',
  'cockpit/assets/realm/ambient/round-table-mountain-source.png',
  'cockpit/assets/realm/ambient/round-table-mountain.png',
  'cockpit/assets/realm/ambient/scriptorium-mountain-v2-source.png',
  'cockpit/assets/realm/ambient/scriptorium-mountain-v2.png',
  'cockpit/assets/realm/ambient/smithy-mountain-source.png',
  'cockpit/assets/realm/ambient/smithy-mountain.png',
  'cockpit/assets/realm/palette.json',
  // Telemetry tables the cockpit compiles in for cost/calibration/store caps.
  'docs/telemetry/model-calibration.toml',
  'docs/telemetry/prices.toml',
  'docs/telemetry/store-caps.toml',
  // Synthetic wire fixtures consumed by the provider parser tests.
  'cockpit/fixtures/usage/native-usage-wire-v1.json',
  'cockpit/fixtures/usage/supplemental-chat-usage-wire-v1.json',
])

export const REQUIRED_RUNTIME_HELPERS = Object.freeze([
  'scripts/repo-dossier.mjs',
  'scripts/repo-dossier.test.mjs',
  'scripts/store-caps.mjs',
  'scripts/causal-loop.mjs',
  'scripts/cut-evidence.mjs',
  'lib/research/CausalGraph.js',
  'scripts/private-store-fs.mjs',
  'scripts/private-store-bridge.py',
  'scripts/angel-machine-queue.py',
  'scripts/angel-machine-queue.test.py',
  'scripts/pxpipe-transform.mjs',
])

const PUBLIC_RELEASE_DOCS = Object.freeze([
  'docs/FEATURES.md',
  'docs/COMMANDS.md',
  'docs/images/README.md',
  'docs/images/provenance.json',
  'docs/images/cockpit.png',
  'docs/images/command-picker.png',
  'docs/images/world.png',
])

const PUBLIC_RELEASE_NOTICES = Object.freeze([
  'THIRD_PARTY_NOTICES.md',
  'third-party/openai-codex-LICENSE.txt',
  'third-party/openai-codex-NOTICE.txt',
  'third-party/oh-my-pi-LICENSE.txt',
  'third-party/reasonix-LICENSE.txt',
  'third-party/hermes-agent-LICENSE.txt',
  'third-party/sources.json',
])

export const RELEASE_PATHS = Object.freeze([
  '.gitignore',
  'README.md',
  'LICENSE',
  'CHANGELOG.md',
  'CONTRIBUTING.md',
  'SECURITY.md',
  ...PUBLIC_RELEASE_DOCS,
  ...PUBLIC_RELEASE_NOTICES,
  ...REQUIRED_RUNTIME_HELPERS,
  'package.json',
  'package-lock.json',
  'rust-toolchain.toml',
  'release/supply-chain-policy.json',
  'release/Dockerfile.clean-builder',
  'bin/angel0',
  'scripts/angel-club-policy.sh',
  'scripts/angel-club-policy.test.mjs',
  'cockpit/Cargo.toml',
  'cockpit/README.md',
  'cockpit/Cargo.lock',
  'cockpit/src',
  'cockpit/tests',
  'cockpit/assets',
  'cockpit/graphs/council.toml',
  'cockpit/graphs/research-pool.toml',
  'cockpit/graphs/research-write-review.toml',
  ...REQUIRED_COCKPIT_EMBEDDED_FILES,
  'cockpit/research/sloptomizer/README.md',
  'cockpit/docs/COMPETITION_RUNNER.md',
  'cockpit/docs/ENV.md',
  'cockpit/docs/JEV.md',
  'cockpit/portal-renderer/Cargo.toml',
  'cockpit/portal-renderer/Cargo.lock',
  'cockpit/portal-renderer/src',
  'scripts/cockpit-source-digest.sh',
  'scripts/check-cockpit-fast.sh',
  'scripts/check-cockpit-quality.sh',
  'scripts/check-active-connections.py',
  'scripts/test_active_connections.py',
  'scripts/trace_schema.py',
  'scripts/receipt_provenance.py',
  'scripts/aging-parity-cohort.py',
  'scripts/repo-search-cohort.py',
  'scripts/m05-taskjson-health-receipt.py',
  'scripts/astra-w01-http-proof.py',
  'scripts/research-grounding-cohort.py',
  'scripts/gen-harness-rollout-seed.py',
  'scripts/harness-stress.py',
  'scripts/trajectory-redact.py',
  'scripts/private_store_io.py',
  'vendor/dotmax/Cargo.toml',
  'vendor/dotmax/LICENSE-APACHE',
  'vendor/dotmax/LICENSE-MIT',
  'vendor/dotmax/README.md',
  'vendor/dotmax/src',
  // ureq is the cockpit's second local Cargo path dependency (a vendored,
  // patched fork); the archive cannot build offline without it.
  'vendor/ureq/ANGEL_PATCH.md',
  'vendor/ureq/Cargo.toml',
  'vendor/ureq/LICENSE-APACHE',
  'vendor/ureq/LICENSE-MIT',
  'vendor/ureq/README.md',
  'vendor/ureq/src',
  'scripts/release-evidence.mjs',
  'scripts/release-evidence.test.mjs',
  'scripts/check-legacy-terminal-boundary.sh',
  'scripts/verify-release-evidence.mjs',
  'scripts/verify-release-evidence.test.mjs',
  'scripts/verify-release-container.mjs',
  'scripts/verify-release-container.test.mjs',
  'scripts/verify-release-advisories.mjs',
  'scripts/verify-release-advisories.test.mjs',
  'scripts/verify-release-install.mjs',
  'scripts/verify-release-install.test.mjs',
  'scripts/verify-release-rollback.mjs',
  'scripts/verify-runner-smoke.mjs',
  'scripts/verify-runner-smoke.test.mjs',
  'docs/release-evidence.md',
  'docs/telemetry/trace-schema-v1.json',
])

export const REQUIRED_RELEASE_FILES = Object.freeze([
  ...PUBLIC_RELEASE_DOCS,
  ...PUBLIC_RELEASE_NOTICES,
  ...REQUIRED_RUNTIME_HELPERS,
  'cockpit/README.md',
  'cockpit/graphs/council.toml',
  'cockpit/graphs/research-pool.toml',
  'cockpit/graphs/research-write-review.toml',
  '.gitignore',
  'README.md',
  'LICENSE',
  'SECURITY.md',
  'package.json',
  'package-lock.json',
  'rust-toolchain.toml',
  'release/supply-chain-policy.json',
  'release/Dockerfile.clean-builder',
  'bin/angel0',
  'scripts/angel-club-policy.sh',
  'scripts/angel-club-policy.test.mjs',
  'cockpit/Cargo.toml',
  'cockpit/Cargo.lock',
  'cockpit/src/main.rs',
  'cockpit/tests/interactive_terminal_smoke.rs',
  ...REQUIRED_COCKPIT_EMBEDDED_FILES,
  'cockpit/research/sloptomizer/README.md',
  'cockpit/docs/COMPETITION_RUNNER.md',
  'cockpit/docs/ENV.md',
  'cockpit/docs/JEV.md',
  'cockpit/portal-renderer/Cargo.toml',
  'cockpit/portal-renderer/Cargo.lock',
  'cockpit/portal-renderer/src/main.rs',
  'scripts/cockpit-source-digest.sh',
  'scripts/check-cockpit-fast.sh',
  'scripts/check-cockpit-quality.sh',
  'scripts/check-active-connections.py',
  'scripts/test_active_connections.py',
  'scripts/trace_schema.py',
  'scripts/receipt_provenance.py',
  'scripts/aging-parity-cohort.py',
  'scripts/repo-search-cohort.py',
  'scripts/m05-taskjson-health-receipt.py',
  'scripts/astra-w01-http-proof.py',
  'scripts/research-grounding-cohort.py',
  'scripts/gen-harness-rollout-seed.py',
  'scripts/harness-stress.py',
  'scripts/trajectory-redact.py',
  'scripts/private_store_io.py',
  'vendor/dotmax/Cargo.toml',
  'vendor/dotmax/LICENSE-APACHE',
  'vendor/dotmax/LICENSE-MIT',
  'vendor/dotmax/README.md',
  'vendor/dotmax/src/lib.rs',
  'vendor/ureq/ANGEL_PATCH.md',
  'vendor/ureq/Cargo.toml',
  'vendor/ureq/LICENSE-APACHE',
  'vendor/ureq/LICENSE-MIT',
  'vendor/ureq/README.md',
  'vendor/ureq/src/lib.rs',
  'scripts/release-evidence.mjs',
  'scripts/release-evidence.test.mjs',
  'scripts/check-legacy-terminal-boundary.sh',
  'scripts/verify-release-evidence.mjs',
  'scripts/verify-release-evidence.test.mjs',
  'scripts/verify-release-container.mjs',
  'scripts/verify-release-container.test.mjs',
  'scripts/verify-release-advisories.mjs',
  'scripts/verify-release-advisories.test.mjs',
  'scripts/verify-release-install.mjs',
  'scripts/verify-release-install.test.mjs',
  'scripts/verify-release-rollback.mjs',
  'scripts/verify-runner-smoke.mjs',
  'scripts/verify-runner-smoke.test.mjs',
  'docs/release-evidence.md',
  'docs/telemetry/trace-schema-v1.json',
])

const FORBIDDEN_NAMESPACE = 'off-limits'
const MAX_COMMAND_OUTPUT = 64 * 1024 * 1024

export class ReleaseGateError extends Error {
  constructor(message, details = undefined) {
    super(message)
    this.name = 'ReleaseGateError'
    this.details = details
  }
}

function fail(message, details) {
  throw new ReleaseGateError(message, details)
}

function run(command, args, { cwd, encoding = 'utf8' } = {}) {
  const result = spawnSync(command, args, {
    cwd,
    encoding,
    maxBuffer: MAX_COMMAND_OUTPUT,
    stdio: ['ignore', 'pipe', 'pipe'],
  })
  if (result.error) fail(`could not run ${command}: ${result.error.message}`)
  if (result.status !== 0) {
    const stderr = Buffer.isBuffer(result.stderr)
      ? result.stderr.toString('utf8')
      : String(result.stderr || '')
    fail(`${command} exited ${result.status}: ${stderr.trim() || 'no diagnostic'}`)
  }
  return result.stdout
}

export function sha256Bytes(value) {
  return createHash('sha256').update(value).digest('hex')
}

export function sha256File(path) {
  return sha256Bytes(readFileSync(path))
}

export function isCockpitSourceIdentityInput(path) {
  // dotmax is the cockpit's local Cargo path dependency, so its manifest and
  // complete source tree must share the binary's source identity.
  return (
    path === 'cockpit/Cargo.toml' ||
    path === 'cockpit/Cargo.lock' ||
    path.startsWith('cockpit/src/') ||
    path === 'vendor/dotmax/Cargo.toml' ||
    path.startsWith('vendor/dotmax/src/') ||
    REQUIRED_COCKPIT_EMBEDDED_FILES.includes(path)
  )
}

function u64Frame(value) {
  if (!Number.isSafeInteger(value) || value < 0) fail(`invalid identity frame length: ${value}`)
  const frame = Buffer.alloc(8)
  frame.writeBigUInt64BE(BigInt(value))
  return frame
}

export function cockpitSourceIdentityFromRows(rows) {
  const source = rows
    .filter((row) => isCockpitSourceIdentityInput(row.path))
    .toSorted((left, right) => left.path.localeCompare(right.path))
  if (source.length === 0) fail('release contains no cockpit source identity inputs')
  const hash = createHash('sha256')
  hash.update('angel0-cockpit-source-identity/v2\0')
  for (const row of source) {
    const path = Buffer.from(row.path, 'utf8')
    const content = Buffer.from(row.content)
    hash.update(u64Frame(path.length))
    hash.update(path)
    hash.update(u64Frame(content.length))
    hash.update(content)
  }
  return hash.digest('hex')
}

function normalizedObject(value) {
  if (Array.isArray(value)) return value.map(normalizedObject)
  if (value && typeof value === 'object') {
    return Object.fromEntries(
      Object.keys(value)
        .sort()
        .map((key) => [key, normalizedObject(value[key])]),
    )
  }
  return value
}

export function canonicalJson(value) {
  return `${JSON.stringify(normalizedObject(value), null, 2)}\n`
}

export function assertSafeReleasePath(path) {
  if (!path || isAbsolute(path) || path.includes('\\')) fail(`unsafe release path: ${path}`)
  if ([...path].some((character) => character.codePointAt(0) < 32 || character === '\x7f')) {
    fail(`release path contains control bytes: ${path}`)
  }
  const pieces = path.split('/')
  if (pieces.some((piece) => !piece || piece === '.' || piece === '..')) {
    fail(`release path is not normalized: ${path}`)
  }
  if (path === FORBIDDEN_NAMESPACE || path.startsWith(`${FORBIDDEN_NAMESPACE}/`)) {
    fail(`quarantined path cannot enter a release: ${path}`)
  }
  return path
}

function repositoryRoot(cwd) {
  const root = String(run('git', ['rev-parse', '--show-toplevel'], { cwd })).trim()
  if (!root) fail('git did not report a repository root')
  return realpathSync(root)
}

function parseIndexRows(output) {
  const rows = String(output).split('\0').filter(Boolean)
  return rows.map((row) => {
    const tab = row.indexOf('\t')
    if (tab < 0) fail(`could not parse git index row: ${row}`)
    const [mode, object, stage] = row.slice(0, tab).split(' ')
    const path = assertSafeReleasePath(row.slice(tab + 1))
    if (!/^[0-9]{6}$/u.test(mode) || !/^[0-9a-f]{40,64}$/u.test(object) || stage !== '0') {
      fail(`unsupported git index row for ${path}`)
    }
    return { path, mode, object }
  })
}

export function inventoryReleaseIndex(repoRoot, pathspecs = RELEASE_PATHS) {
  const output = run('git', ['ls-files', '--stage', '-z', '--', ...pathspecs], {
    cwd: repoRoot,
  })
  const rows = parseIndexRows(output).sort((a, b) =>
    a.path < b.path ? -1 : a.path > b.path ? 1 : 0,
  )
  if (rows.length === 0) fail('release path allowlist selected no tracked files')
  const seen = new Set()
  for (const row of rows) {
    if (seen.has(row.path)) fail(`duplicate release path: ${row.path}`)
    seen.add(row.path)
    if (row.mode !== '100644' && row.mode !== '100755') {
      fail(`release entries must be regular files, found mode ${row.mode}: ${row.path}`)
    }
  }
  return rows
}

export function inventoryReleaseFiles(repoRoot, pathspecs = RELEASE_PATHS) {
  const rows = inventoryReleaseIndex(repoRoot, pathspecs)

  const entries = rows.map((row) => {
    const absolute = resolve(repoRoot, row.path)
    const lexicalRelative = relative(repoRoot, absolute)
    if (lexicalRelative.startsWith(`..${sep}`) || lexicalRelative === '..') {
      fail(`release path escapes repository: ${row.path}`)
    }
    const info = lstatSync(absolute)
    if (!info.isFile()) fail(`release entry is not a regular file: ${row.path}`)
    const executable = (info.mode & 0o111) !== 0
    if (executable !== (row.mode === '100755')) {
      fail(`checkout mode differs from git mode for ${row.path}`)
    }
    const content = readFileSync(absolute)
    const gitObject = String(run('git', ['hash-object', '--', row.path], { cwd: repoRoot })).trim()
    if (gitObject !== row.object) fail(`working file differs from the git index: ${row.path}`)
    return {
      path: row.path,
      mode: row.mode,
      bytes: content.byteLength,
      sha256: sha256Bytes(content),
    }
  })

  return entries
}

export function assertRequiredReleaseFiles(entries, required = REQUIRED_RELEASE_FILES) {
  const paths = new Set(entries.map((entry) => entry.path))
  const missing = required.filter((path) => !paths.has(path))
  if (missing.length > 0) fail(`release is missing required files: ${missing.join(', ')}`)
}

const SYNTHETIC_HOME_NAMES = new Set(['angel', 'probe', 'proof', 'u', 'user', 'x'])
const TEXT_RELEASE_PATH =
  /(?:\.(?:gitignore|json|lock|md|mjs|patch|py|rs|sh|toml|txt|ya?ml)|Dockerfile[^/]*)$/u
const LIVE_TAILNET_DNS = /\b[A-Za-z0-9.-]+\.tail[0-9a-f]{4,}\.ts\.net\b/iu
const HARDWARE_HOSTNAME = new RegExp(`\\b${['d', 'g', 'x'].join('')}-[a-z0-9-]+\\b`, 'iu')
const HOST_MOUNT_COORDINATE = /\/mnt\/[A-Za-z0-9._-]+/u

function isTestFixturePath(path) {
  return /(?:^|\/)(?:fixtures?|tests?)(?:\/|\.|$)/u.test(path) || /\.test\.[cm]?js$/u.test(path)
}

function isTextReleasePath(path) {
  return path === 'LICENSE' || path === 'bin/angel0' || TEXT_RELEASE_PATH.test(path)
}

function isCgnatHost(host) {
  const octets = host.split('.').map(Number)
  return (
    octets.length === 4 &&
    octets.every((octet) => Number.isInteger(octet) && octet >= 0 && octet <= 255) &&
    octets[0] === 100 &&
    octets[1] >= 64 &&
    octets[1] <= 127
  )
}

/**
 * Reject operator-local material even if a future edit widens RELEASE_PATHS.
 *
 * Synthetic values are allowed only in test/fixture sources so private-network
 * behavior remains testable. Production source and documentation may not carry
 * any CGNAT literal or absolute per-user home path.
 */
export function assertPublicReleaseEntries(repoRoot, entries) {
  const embedded = new Set(REQUIRED_COCKPIT_EMBEDDED_FILES)
  for (const entry of entries) {
    const { path } = entry
    if (
      path === 'scripts/heads/heads.json' ||
      path.startsWith('scripts/heads/') ||
      (path.startsWith('cockpit/skills/') && !embedded.has(path)) ||
      (path.startsWith('cockpit/personas/') && !embedded.has(path)) ||
      (path.startsWith('cockpit/graphs/') &&
        ![
          'cockpit/graphs/council.toml',
          'cockpit/graphs/research-pool.toml',
          'cockpit/graphs/research-write-review.toml',
        ].includes(path)) ||
      path.startsWith('benchmarks/') ||
      (path.startsWith('docs/') &&
        !['docs/release-evidence.md', 'docs/telemetry/trace-schema-v1.json'].includes(path) &&
        !PUBLIC_RELEASE_DOCS.includes(path) &&
        !embedded.has(path)) ||
      (path.startsWith('cockpit/docs/') &&
        ![
          'cockpit/docs/COMPETITION_RUNNER.md',
          'cockpit/docs/ENV.md',
          'cockpit/docs/JEV.md',
        ].includes(path)) ||
      (path.startsWith('cockpit/portal-renderer/') &&
        !['cockpit/portal-renderer/Cargo.toml', 'cockpit/portal-renderer/Cargo.lock'].includes(
          path,
        ) &&
        !path.startsWith('cockpit/portal-renderer/src/')) ||
      (path.startsWith('cockpit/fixtures/') && !embedded.has(path)) ||
      (path.startsWith('cockpit/modules/') && !embedded.has(path)) ||
      (path.startsWith('cockpit/research/') &&
        path !== 'cockpit/research/sloptomizer/README.md' &&
        !embedded.has(path)) ||
      path.startsWith('experimental/')
    ) {
      fail(`operator-only path cannot enter the public release: ${path}`)
    }

    if (!isTextReleasePath(path)) continue
    const source = readFileSync(join(repoRoot, path), 'utf8')
    for (const match of source.matchAll(/\/(?:home|Users)\/([A-Za-z0-9._-]+)(?:\/|\b)/gu)) {
      if (!SYNTHETIC_HOME_NAMES.has(match[1])) {
        fail(`machine-specific absolute home path in public release entry: ${path}`)
      }
    }
    for (const match of source.matchAll(/[A-Za-z]:\\Users\\([^\\\s]+)/gu)) {
      if (!SYNTHETIC_HOME_NAMES.has(match[1])) {
        fail(`machine-specific absolute home path in public release entry: ${path}`)
      }
    }
    for (const match of source.matchAll(/\b(?:\d{1,3}\.){3}\d{1,3}\b/gu)) {
      const host = match[0]
      if (isCgnatHost(host) && !isTestFixturePath(path)) {
        fail(`private CGNAT endpoint in public release entry: ${path}`)
      }
    }
    if (LIVE_TAILNET_DNS.test(source)) {
      fail(`live tailnet DNS name in public release entry: ${path}`)
    }
    if (HARDWARE_HOSTNAME.test(source)) {
      fail(`machine-specific hardware hostname in public release entry: ${path}`)
    }
    if (HOST_MOUNT_COORDINATE.test(source)) {
      fail(`machine-specific mount coordinate in public release entry: ${path}`)
    }
  }
}

export function cockpitSourceSha256(repoRoot, entries) {
  return cockpitSourceIdentityFromRows(
    entries.map((entry) => ({
      path: entry.path,
      content: readFileSync(join(repoRoot, entry.path)),
    })),
  )
}

export function assertReleaseInputsClean(repoRoot, pathspecs = RELEASE_PATHS) {
  const tagged = run('git', ['ls-files', '-v', '-z', '--cached', '--', ...pathspecs], {
    cwd: repoRoot,
  })
  for (const row of String(tagged).split('\0').filter(Boolean)) {
    if (row.length < 3 || row[1] !== ' ') fail(`could not parse git index flag row: ${row}`)
    const tag = row[0]
    const path = assertSafeReleasePath(row.slice(2))
    if (tag === 'S' || tag === 's') {
      fail(`release input has skip-worktree set: ${path}`)
    }
    if (/^[a-z]$/u.test(tag)) {
      fail(`release input has assume-unchanged set: ${path}`)
    }
  }

  const dirty = run(
    'git',
    ['status', '--porcelain=v1', '-z', '--untracked-files=all', '--', ...pathspecs],
    { cwd: repoRoot },
  )
  if (String(dirty).length > 0) {
    fail('release inputs are dirty; commit or remove changes before producing evidence')
  }
}

export function inspectNodeLock(repoRoot) {
  const packageJson = JSON.parse(readFileSync(join(repoRoot, 'package.json'), 'utf8'))
  const lockPath = join(repoRoot, 'package-lock.json')
  const lock = JSON.parse(readFileSync(lockPath, 'utf8'))
  if (packageJson.private !== true || packageJson.license !== 'MIT') {
    fail('release policy requires MIT licensing and disabled npm publication (private: true)')
  }
  if (packageJson.engines?.node !== '^20.19.0 || ^22.13.0 || >=24') {
    fail('supported Node policy must match the ESLint 10 runtime floor')
  }
  if (lock.lockfileVersion !== 3 || !lock.packages || typeof lock.packages !== 'object') {
    fail('package-lock.json must use lockfileVersion 3 with a packages graph')
  }
  if (lock.name !== packageJson.name || lock.version !== packageJson.version) {
    fail('package-lock root identity differs from package.json')
  }
  if (lock.packages['']?.license !== packageJson.license) {
    fail('package-lock root license differs from package.json')
  }

  const registry = []
  for (const [path, dependency] of Object.entries(lock.packages)) {
    if (!path) continue
    const resolved = String(dependency.resolved || '')
    if (!resolved.startsWith('https://registry.npmjs.org/')) {
      fail(`non-registry Node dependency is not permitted in the release graph: ${path}`)
    }
    const integrity = String(dependency.integrity || '')
    const integrityMatch = integrity.match(/^sha512-([A-Za-z0-9+/]+={0,2})$/u)
    if (!integrityMatch || Buffer.from(integrityMatch[1], 'base64').byteLength !== 64) {
      fail(`Node dependency lacks sha512 integrity: ${path}`)
    }
    if (!dependency.license || typeof dependency.license !== 'string') {
      fail(`Node dependency lacks license metadata: ${path}`)
    }
    registry.push(path)
  }

  return {
    lockfile_sha256: sha256File(lockPath),
    lockfile_version: lock.lockfileVersion,
    package_count: Object.keys(lock.packages).length,
    registry_package_count: registry.length,
    integrity_policy: 'all registry packages carry sha512 lock integrity',
    license_policy: 'all registry packages carry lockfile license metadata',
    product_license: 'MIT',
  }
}

function parseRustToolchain(repoRoot) {
  const path = join(repoRoot, 'rust-toolchain.toml')
  const source = readFileSync(path, 'utf8')
  const match = source.match(/^channel\s*=\s*"([^"]+)"\s*$/mu)
  if (!match) fail('rust-toolchain.toml must declare an exact channel')
  if (!/^\d+\.\d+\.\d+$/u.test(match[1])) fail('Rust channel must be an exact stable version')
  const componentsMatch = source.match(/^components\s*=\s*\[([^\]]+)\]\s*$/mu)
  const components = componentsMatch
    ? [...componentsMatch[1].matchAll(/"([^"]+)"/gu)].map((component) => component[1]).sort()
    : []
  if (canonicalJson(components) !== canonicalJson(['clippy', 'rustfmt'])) {
    fail('Rust toolchain must pin both clippy and rustfmt components')
  }
  return { version: match[1], components, sha256: sha256File(path) }
}

function assertWithin(root, candidate, label) {
  const answer = relative(root, candidate)
  if (answer === '..' || answer.startsWith(`..${sep}`) || isAbsolute(answer)) {
    fail(`${label} escapes its package root`)
  }
}

function parseCargoLock(repoRoot) {
  const path = join(repoRoot, 'cockpit/Cargo.lock')
  const source = readFileSync(path, 'utf8')
  if (!/^version = 4$/mu.test(source)) fail('Cargo.lock must use lockfile version 4')
  const packages = source
    .split(/^\[\[package\]\]\s*$/mu)
    .slice(1)
    .map((block) => {
      const field = (name) => block.match(new RegExp(`^${name} = "([^"]+)"$`, 'mu'))?.[1] || null
      return {
        name: field('name'),
        version: field('version'),
        source: field('source'),
        checksum: field('checksum'),
      }
    })
  if (packages.some((pkg) => !pkg.name || !pkg.version))
    fail('Cargo.lock contains an invalid package row')
  const rows = new Map()
  let registryPackages = 0
  for (const pkg of packages) {
    const key = `${pkg.name}\0${pkg.version}\0${pkg.source || ''}`
    if (rows.has(key))
      fail(`Cargo.lock contains a duplicate package identity: ${pkg.name}@${pkg.version}`)
    rows.set(key, pkg)
    if (pkg.source) {
      if (!pkg.source.startsWith('registry+')) {
        fail(`Cargo.lock contains a non-registry source: ${pkg.name}@${pkg.version}`)
      }
      if (!pkg.checksum || !/^[0-9a-f]{64}$/u.test(pkg.checksum)) {
        fail(`Cargo.lock registry package lacks a checksum: ${pkg.name}@${pkg.version}`)
      }
      registryPackages += 1
    }
  }
  return { path, packages, rows, registryPackages }
}

// Packaged lockfiles that are frozen candidate input rather than a dependency
// graph of the release. The narrowed archive carries only its three dependency
// locks, so nothing is exempt; the manifest still records the (empty) list.
const NON_DEPENDENCY_FIXTURE_LOCKFILES = new Set()

function advisoryLockfileEcosystem(path) {
  if (NON_DEPENDENCY_FIXTURE_LOCKFILES.has(path)) return null
  const name = basename(path)
  if (name === 'Cargo.lock') return 'crates.io'
  if (name === 'package-lock.json') return 'npm'
  if (
    ['uv.lock', 'Pipfile.lock', 'poetry.lock', 'pdm.lock', 'pylock.toml'].includes(name) ||
    /^requirements(?:-[^.]+)?\.txt$/u.test(name)
  ) {
    return 'PyPI'
  }
  return null
}

function advisoryLockfilePackageCount(repoRoot, path, ecosystem) {
  const source = readFileSync(join(repoRoot, path), 'utf8')
  if (ecosystem === 'crates.io') {
    return [...source.matchAll(/^\[\[package\]\]\s*$/gmu)].length
  }
  if (ecosystem === 'npm') {
    const lock = JSON.parse(source)
    if (!lock.packages || typeof lock.packages !== 'object') {
      fail(`advisory lockfile has no npm packages graph: ${path}`)
    }
    return Object.keys(lock.packages).filter(Boolean).length
  }
  fail(`unsupported advisory lockfile ecosystem: ${ecosystem}`)
}

function inspectSupplyChainPolicy(repoRoot, metadata, toolchain, releaseEntries) {
  const path = join(repoRoot, 'release/supply-chain-policy.json')
  const policy = JSON.parse(readFileSync(path, 'utf8'))
  if (
    policy.schema !== 'angel0-release-supply-chain-policy/v1' ||
    !Array.isArray(policy.prebuilt_artifacts) ||
    policy.prebuilt_artifacts.length !== 1 ||
    !Array.isArray(policy.container_images) ||
    policy.container_images.length !== 1 ||
    !Array.isArray(policy.advisory_scanners) ||
    policy.advisory_scanners.length !== 1 ||
    !policy.advisory_policy ||
    typeof policy.advisory_policy !== 'object'
  ) {
    fail('release supply-chain policy must declare exactly one versioned prebuilt artifact')
  }
  const artifact = policy.prebuilt_artifacts[0]
  const locked = metadata.packages.find(
    (pkg) => pkg.name === artifact.crate && pkg.version === artifact.crate_version,
  )
  if (!locked) fail('prebuilt rusty_v8 policy does not match the supported Cargo graph')
  if (
    artifact.name !== 'rusty_v8' ||
    artifact.target !== SUPPORTED_CARGO_TARGET ||
    artifact.profile !== 'release' ||
    artifact.format !== 'decompressed-static-library' ||
    !Number.isSafeInteger(artifact.bytes) ||
    artifact.bytes <= 0 ||
    !/^[0-9a-f]{64}$/u.test(artifact.sha256) ||
    artifact.upstream_url !==
      `https://github.com/denoland/rusty_v8/releases/download/v${artifact.crate_version}/librusty_v8_release_${SUPPORTED_CARGO_TARGET}.a.gz` ||
    !String(artifact.trust).includes('no upstream signature')
  ) {
    fail('prebuilt rusty_v8 policy is incomplete or inconsistent')
  }
  const image = policy.container_images[0]
  const dockerfilePath = join(repoRoot, 'release/Dockerfile.clean-builder')
  const dockerfile = readFileSync(dockerfilePath, 'utf8')
  if (
    image.name !== 'clean_release_builder' ||
    image.reference !== image.digest ||
    !/^sha256:[0-9a-f]{64}$/u.test(image.digest) ||
    image.local_tag !== 'angel0-clean-builder:rust-1.95.0-bookworm-v1' ||
    image.base_reference !== `docker.io/library/rust@${image.base_digest}` ||
    !/^sha256:[0-9a-f]{64}$/u.test(image.base_digest) ||
    image.dockerfile_sha256 !== sha256File(dockerfilePath) ||
    !dockerfile.startsWith(`FROM ${image.base_reference}\n`) ||
    image.operating_system !== 'linux' ||
    image.architecture !== 'amd64' ||
    image.rust_version !== toolchain.version ||
    !String(image.distribution).includes('bookworm') ||
    !Array.isArray(image.native_packages) ||
    image.native_packages.length !== 9 ||
    !String(image.trust).includes('no signature')
  ) {
    fail('clean release container policy is incomplete or inconsistent')
  }
  const scanner = policy.advisory_scanners[0]
  if (
    scanner.name !== 'osv-scanner' ||
    scanner.version !== '2.3.8' ||
    !/^[0-9a-f]{40}$/u.test(String(scanner.commit || '')) ||
    !/^sha256:[0-9a-f]{64}$/u.test(String(scanner.digest || '')) ||
    scanner.reference !== `ghcr.io/google/osv-scanner@${scanner.digest}` ||
    scanner.operating_system !== 'linux' ||
    scanner.architecture !== 'amd64' ||
    !String(scanner.trust).includes('not yet performed')
  ) {
    fail('advisory scanner policy is incomplete or inconsistent')
  }
  const advisory = policy.advisory_policy
  const expectedLockfiles = new Map([
    ['cockpit/Cargo.lock', 'crates.io'],
    ['cockpit/portal-renderer/Cargo.lock', 'crates.io'],
    ['package-lock.json', 'npm'],
  ])
  if (
    !Array.isArray(advisory.lockfiles) ||
    advisory.lockfiles.length !== expectedLockfiles.size ||
    advisory.python?.status !== 'not-applicable' ||
    !String(advisory.python?.reason).includes('no Python dependency') ||
    !Array.isArray(advisory.exceptions) ||
    advisory.exceptions.length !== 3
  ) {
    fail('release advisory policy scope is incomplete')
  }
  for (const lockfile of advisory.lockfiles) {
    const ecosystem = expectedLockfiles.get(lockfile.path)
    if (
      !ecosystem ||
      lockfile.ecosystem !== ecosystem ||
      lockfile.sha256 !== sha256File(join(repoRoot, lockfile.path)) ||
      lockfile.package_count !==
        advisoryLockfilePackageCount(repoRoot, lockfile.path, lockfile.ecosystem)
    ) {
      fail(`release advisory lockfile policy differs: ${lockfile.path || 'unknown'}`)
    }
    expectedLockfiles.delete(lockfile.path)
  }
  if (expectedLockfiles.size > 0) fail('release advisory policy omitted a dependency lockfile')
  if (releaseEntries) {
    const discovered = releaseEntries
      .map((entry) => ({ path: entry.path, ecosystem: advisoryLockfileEcosystem(entry.path) }))
      .filter((entry) => entry.ecosystem)
      .sort((left, right) => left.path.localeCompare(right.path))
    const declared = [...advisory.lockfiles]
      .map(({ path, ecosystem }) => ({ path, ecosystem }))
      .sort((left, right) => left.path.localeCompare(right.path))
    if (canonicalJson(discovered) !== canonicalJson(declared)) {
      fail('release advisory policy does not cover every packaged dependency lockfile')
    }
    if (
      discovered.some((entry) => entry.ecosystem === 'PyPI') !==
      (advisory.python.status !== 'not-applicable')
    ) {
      fail('release advisory Python applicability differs from packaged inputs')
    }
  }
  const exceptionKeys = new Set()
  for (const exception of advisory.exceptions) {
    if (
      exception.lockfile !== 'cockpit/Cargo.lock' ||
      !exception.package ||
      !exception.version ||
      !Array.isArray(exception.ids) ||
      exception.ids.length === 0 ||
      canonicalJson(exception.ids) !== canonicalJson([...new Set(exception.ids)].sort()) ||
      !['unmaintained', 'unsound'].includes(exception.kind) ||
      !/^\d{4}-\d{2}-\d{2}$/u.test(String(exception.expires || '')) ||
      !exception.reason ||
      exception.reason.length < 32
    ) {
      fail('release advisory exception is incomplete or inconsistent')
    }
    const key = `${exception.lockfile}\0${exception.package}\0${exception.version}\0${exception.ids.join(',')}`
    if (exceptionKeys.has(key)) fail('release advisory policy contains duplicate exceptions')
    exceptionKeys.add(key)
  }
  return {
    policy_file_sha256: sha256File(path),
    artifacts: [artifact],
    container_images: [image],
    advisory_scanners: [scanner],
    advisory_policy: advisory,
  }
}

export function inspectCargoGraph(repoRoot, releaseEntries = undefined) {
  const toolchain = parseRustToolchain(repoRoot)
  const lock = parseCargoLock(repoRoot)
  const metadataText = run(
    'cargo',
    [
      'metadata',
      '--manifest-path',
      'cockpit/Cargo.toml',
      '--locked',
      '--offline',
      '--filter-platform',
      SUPPORTED_CARGO_TARGET,
      '--format-version',
      '1',
    ],
    { cwd: repoRoot },
  )
  const metadata = JSON.parse(metadataText)
  const supplyChain = inspectSupplyChainPolicy(repoRoot, metadata, toolchain, releaseEntries)
  const rootId = metadata.resolve?.root
  const rootPackage = metadata.packages.find((pkg) => pkg.id === rootId)
  if (!rootPackage || rootPackage.name !== 'angel0-cockpit') {
    fail('Cargo metadata did not resolve angel0-cockpit as the workspace root')
  }

  let registryPackages = 0
  let pathPackages = 0
  const licenseFiles = []
  for (const pkg of metadata.packages) {
    const manifestPath = realpathSync(pkg.manifest_path)
    const packageRoot = dirname(manifestPath)
    if (pkg.source === null) {
      pathPackages += 1
      assertWithin(repoRoot, manifestPath, `path dependency ${pkg.name}`)
      if (pkg.id === rootId) {
        if (pkg.license !== 'MIT') {
          fail('angel0-cockpit must declare the MIT license')
        }
      } else if (!pkg.license && !pkg.license_file) {
        fail(`path dependency lacks license metadata: ${pkg.name}@${pkg.version}`)
      }
    } else {
      registryPackages += 1
      if (!String(pkg.source).startsWith('registry+')) {
        fail(`non-registry Cargo source is not permitted: ${pkg.name}@${pkg.version}`)
      }
      const locked = lock.rows.get(`${pkg.name}\0${pkg.version}\0${pkg.source}`)
      if (!locked) {
        fail(`resolved Cargo package is absent from Cargo.lock: ${pkg.name}@${pkg.version}`)
      }
      if (!pkg.license && !pkg.license_file) {
        fail(
          `Cargo registry package lacks license metadata or a license file: ${pkg.name}@${pkg.version}`,
        )
      }
    }

    if (pkg.license_file) {
      const licensePath = resolve(packageRoot, pkg.license_file)
      assertWithin(packageRoot, licensePath, `license file for ${pkg.name}@${pkg.version}`)
      if (!existsSync(licensePath) || !statSync(licensePath).isFile()) {
        fail(`declared license file is unavailable: ${pkg.name}@${pkg.version}`)
      }
      const realPackageRoot = realpathSync(packageRoot)
      const realLicensePath = realpathSync(licensePath)
      assertWithin(realPackageRoot, realLicensePath, `license file for ${pkg.name}@${pkg.version}`)
      licenseFiles.push({
        package: `${pkg.name}@${pkg.version}`,
        declared_path: pkg.license_file,
        sha256: sha256File(realLicensePath),
      })
    }
  }
  licenseFiles.sort((a, b) => (a.package < b.package ? -1 : a.package > b.package ? 1 : 0))

  const rustc = String(run('rustc', ['-Vv'], { cwd: repoRoot })).trim()
  const release = rustc.match(/^release:\s*(\S+)$/mu)?.[1]
  if (release !== toolchain.version) {
    fail(`active rustc ${release || 'unknown'} differs from pinned ${toolchain.version}`)
  }

  return {
    lockfile_sha256: sha256File(lock.path),
    locked_package_count: lock.packages.length,
    locked_registry_package_count: lock.registryPackages,
    metadata_target: SUPPORTED_CARGO_TARGET,
    resolved_package_count: metadata.packages.length,
    registry_package_count: registryPackages,
    path_package_count: pathPackages,
    registry_policy: 'registry-only dependencies with Cargo checksums',
    license_policy: 'SPDX metadata or a present hashed license file; product root is MIT',
    license_files: licenseFiles,
    prebuilt_artifacts: supplyChain.artifacts,
    clean_build_container_images: supplyChain.container_images,
    advisory_scanners: supplyChain.advisory_scanners,
    advisory_policy: supplyChain.advisory_policy,
    supply_chain_policy_file_sha256: supplyChain.policy_file_sha256,
    toolchain: {
      pinned_version: toolchain.version,
      pinned_components: toolchain.components,
      toolchain_file_sha256: toolchain.sha256,
      rustc_verbose: rustc.split('\n'),
      cargo: String(run('cargo', ['-V'], { cwd: repoRoot })).trim(),
      clippy: String(run('cargo', ['clippy', '--version'], { cwd: repoRoot })).trim(),
      rustfmt: String(run('cargo', ['fmt', '--version'], { cwd: repoRoot })).trim(),
    },
  }
}

function sameEntries(left, right) {
  return canonicalJson(left) === canonicalJson(right)
}

function atomicWrite(path, value, mode = 0o644) {
  const temporary = `${path}.tmp-${process.pid}`
  writeFileSync(temporary, value, { mode })
  chmodSync(temporary, mode)
  renameSync(temporary, path)
}

export function createSourceArchive(repoRoot, outputPath, entries) {
  mkdirSync(dirname(outputPath), { recursive: true })
  const temporaryArchive = `${outputPath}.tmp-${process.pid}`
  const listPath = `${outputPath}.files-${process.pid}`
  try {
    writeFileSync(listPath, Buffer.from(`${entries.map((entry) => entry.path).join('\0')}\0`))
    run(
      'tar',
      [
        '--format=ustar',
        '--sort=name',
        '--mtime=@0',
        '--owner=0',
        '--group=0',
        '--numeric-owner',
        '--mode=a=rwX,go-w',
        '--no-recursion',
        '--null',
        '--verbatim-files-from',
        '--files-from',
        listPath,
        '-cf',
        temporaryArchive,
      ],
      { cwd: repoRoot, encoding: null },
    )
    const listed = String(run('tar', ['-tf', temporaryArchive], { cwd: repoRoot }))
      .split('\n')
      .filter(Boolean)
    const expected = entries.map((entry) => entry.path)
    if (canonicalJson(listed) !== canonicalJson(expected)) {
      fail('archive entry list differs from the release inventory')
    }
    renameSync(temporaryArchive, outputPath)
  } finally {
    rmSync(listPath, { force: true })
    rmSync(temporaryArchive, { force: true })
  }
  return {
    name: basename(outputPath),
    media_type: 'application/x-tar',
    bytes: statSync(outputPath).size,
    sha256: sha256File(outputPath),
  }
}

function toolVersions(repoRoot) {
  const [nodeMajor, nodeMinor] = process.versions.node.split('.').map(Number)
  const nodeSupported =
    (nodeMajor === 20 && nodeMinor >= 19) ||
    (nodeMajor === 22 && nodeMinor >= 13) ||
    nodeMajor >= 24
  if (!nodeSupported) fail('release gate requires Node 20.19+, 22.13+, or 24+')
  const tar = String(run('tar', ['--version'], { cwd: repoRoot }))
    .split('\n')[0]
    .trim()
  if (!tar.startsWith('tar (GNU tar) ')) fail('release gate requires GNU tar')
  return {
    node: process.version,
    git: String(run('git', ['--version'], { cwd: repoRoot })).trim(),
    tar,
  }
}

export function assertSafeOutputDirectory(repoRoot, outputRoot) {
  const outputRelative = relative(repoRoot, outputRoot)
  const outputInsideRepository =
    outputRelative !== '..' && !outputRelative.startsWith(`..${sep}`) && !isAbsolute(outputRelative)
  if (
    outputInsideRepository &&
    outputRelative !== '.angel' &&
    !outputRelative.startsWith(`.angel${sep}`)
  ) {
    fail('release output inside the repository must stay under .angel/')
  }
  return outputRoot
}

export function buildManifest({
  version,
  commit,
  tree,
  entries,
  archive,
  nodeDependencies,
  rustDependencies,
  tools,
  cockpitSource,
}) {
  const unsigned = {
    schema: RELEASE_SCHEMA,
    scope: RELEASE_SCOPE,
    version,
    source: {
      commit,
      tree,
      cockpit_source_sha256: cockpitSource,
      release_inputs_dirty: false,
    },
    support: {
      operating_system: 'Linux',
      architecture: 'x86_64',
      cargo_target: SUPPORTED_CARGO_TARGET,
      node: '^20.19.0 || ^22.13.0 || >=24',
      rust: rustDependencies.toolchain.pinned_version,
    },
    policy: {
      excluded_namespace: `${FORBIDDEN_NAMESPACE}/`,
      source_inputs: 'tracked, committed, regular files from the explicit allowlist',
      non_dependency_fixture_lockfiles: [...NON_DEPENDENCY_FIXTURE_LOCKFILES].sort(),
      network_during_gate: 'disabled for Cargo metadata; no vulnerability feed queried',
      clean_host_proven: false,
    },
    dependencies: { node: nodeDependencies, rust: rustDependencies },
    tools,
    entries_manifest_sha256: sha256Bytes(canonicalJson(entries)),
    entries,
    artifact: archive,
  }
  return {
    ...unsigned,
    manifest_sha256: sha256Bytes(canonicalJson(unsigned)),
  }
}

export function runReleaseGate({ cwd = process.cwd(), outputDirectory } = {}) {
  if (process.platform !== 'linux' || process.arch !== 'x64') {
    fail('the supported release gate currently runs only on Linux x86_64')
  }
  const repoRoot = repositoryRoot(cwd)
  assertReleaseInputsClean(repoRoot)
  const entries = inventoryReleaseFiles(repoRoot)
  assertRequiredReleaseFiles(entries)
  assertPublicReleaseEntries(repoRoot, entries)
  const packageJson = JSON.parse(readFileSync(join(repoRoot, 'package.json'), 'utf8'))
  const commit = String(run('git', ['rev-parse', 'HEAD'], { cwd: repoRoot })).trim()
  const tree = String(run('git', ['rev-parse', 'HEAD^{tree}'], { cwd: repoRoot })).trim()
  const shortCommit = commit.slice(0, 12)
  const outputRoot = resolve(repoRoot, outputDirectory || '.angel/release')
  assertSafeOutputDirectory(repoRoot, outputRoot)
  const stem = `angel0-source-${packageJson.version}-${shortCommit}`
  const archivePath = join(outputRoot, `${stem}.tar`)

  const nodeDependencies = inspectNodeLock(repoRoot)
  const rustDependencies = inspectCargoGraph(repoRoot, entries)
  const tools = toolVersions(repoRoot)
  const cockpitSource = cockpitSourceSha256(repoRoot, entries)
  const archive = createSourceArchive(repoRoot, archivePath, entries)

  assertReleaseInputsClean(repoRoot)
  const finalEntries = inventoryReleaseFiles(repoRoot)
  assertPublicReleaseEntries(repoRoot, finalEntries)
  if (!sameEntries(entries, finalEntries))
    fail('release inputs changed while the archive was built')

  const manifest = buildManifest({
    version: packageJson.version,
    commit,
    tree,
    entries,
    archive,
    nodeDependencies,
    rustDependencies,
    tools,
    cockpitSource,
  })
  const manifestPath = join(outputRoot, `${stem}.manifest.json`)
  atomicWrite(manifestPath, canonicalJson(manifest))
  atomicWrite(`${archivePath}.sha256`, `${archive.sha256}  ${archive.name}\n`)
  atomicWrite(`${manifestPath}.sha256`, `${sha256File(manifestPath)}  ${basename(manifestPath)}\n`)

  return { repoRoot, archivePath, manifestPath, manifest }
}

function parseArgs(argv) {
  const options = {}
  for (let index = 0; index < argv.length; index += 1) {
    if (argv[index] === '--out') {
      index += 1
      if (!argv[index]) fail('--out requires a directory')
      options.outputDirectory = argv[index]
    } else {
      fail(`unknown argument: ${argv[index]}`)
    }
  }
  return options
}

const invokedPath = process.argv[1] ? pathToFileURL(resolve(process.argv[1])).href : ''
if (import.meta.url === invokedPath) {
  try {
    const result = runReleaseGate(parseArgs(process.argv.slice(2)))
    process.stdout.write(
      `${canonicalJson({
        schema: result.manifest.schema,
        artifact: result.manifest.artifact,
        manifest: relative(result.repoRoot, result.manifestPath),
        manifest_sha256: result.manifest.manifest_sha256,
      })}`,
    )
  } catch (error) {
    process.stderr.write(`release gate failed: ${error.message}\n`)
    process.exitCode = 1
  }
}
