#!/usr/bin/env node

import { spawnSync } from 'node:child_process'
import {
  chmodSync,
  existsSync,
  lstatSync,
  mkdirSync,
  mkdtempSync,
  readFileSync,
  realpathSync,
  renameSync,
  rmSync,
  statSync,
  writeFileSync,
} from 'node:fs'
import { homedir, tmpdir } from 'node:os'
import { basename, dirname, join, relative, resolve, sep } from 'node:path'
import { pathToFileURL } from 'node:url'

import {
  RELEASE_SCHEMA,
  RELEASE_SCOPE,
  ReleaseGateError,
  SUPPORTED_CARGO_TARGET,
  assertSafeReleasePath,
  canonicalJson,
  cockpitSourceIdentityFromRows,
  sha256Bytes,
  sha256File,
} from './release-evidence.mjs'
import { verifyRunnerContractSmoke } from './verify-runner-smoke.mjs'

export const VERIFICATION_SCHEMA = 'angelX-source-release-verification/v1'
export const REQUIRED_RUNNER_CAPABILITIES = Object.freeze([
  'task-json/v1',
  'task-runtime-config/v1',
  'task-rollout-binding/v1',
  'external-verifier/v1',
  'harness-rollout-ref/v1',
  'rollout-audit-receipt/v1',
  'required-rollout/v1',
  'finite-task-defaults/v1',
  'agent-graph-episode/v1',
])
const BUBBLEWRAP = '/usr/bin/bwrap'
const MAX_COMMAND_OUTPUT = 64 * 1024 * 1024
const BUILD_TIMEOUT_MS = 15 * 60 * 1000
const decoder = new TextDecoder('utf-8', { fatal: true })

function fail(message) {
  throw new ReleaseGateError(message)
}

function regularFile(path, label) {
  if (!existsSync(path)) fail(`${label} does not exist: ${path}`)
  const info = lstatSync(path)
  if (!info.isFile() || info.isSymbolicLink()) fail(`${label} must be a regular non-symlink file`)
  return realpathSync(path)
}

function executableFile(path, label) {
  const canonical = regularFile(path, label)
  if ((statSync(canonical).mode & 0o111) === 0) fail(`${label} is not executable`)
  return canonical
}

function run(command, args, { cwd, env = process.env, timeout = BUILD_TIMEOUT_MS } = {}) {
  const started = performance.now()
  const result = spawnSync(command, args, {
    cwd,
    env,
    encoding: 'utf8',
    maxBuffer: MAX_COMMAND_OUTPUT,
    timeout,
    stdio: ['ignore', 'pipe', 'pipe'],
  })
  const durationMs = Math.round(performance.now() - started)
  if (result.error) fail(`could not run ${command}: ${result.error.message}`)
  if (result.status !== 0) {
    const diagnostic = String(result.stderr || result.stdout || '')
      .trim()
      .slice(-8000)
    fail(`${command} exited ${result.status}: ${diagnostic || 'no diagnostic'}`)
  }
  return { ...result, durationMs }
}

function assertHex(value, label) {
  if (!/^[0-9a-f]{64}$/u.test(String(value || ''))) fail(`${label} must be a SHA-256`)
  return value
}

function assertGitIdentity(value, label) {
  if (!/^(?:[0-9a-f]{40}|[0-9a-f]{64})$/u.test(String(value || ''))) {
    fail(`${label} must be a Git object identity`)
  }
  return value
}

function assertChecksumSidecar(path, expected) {
  const sidecar = regularFile(`${path}.sha256`, `${basename(path)} checksum sidecar`)
  const wanted = `${expected}  ${basename(path)}\n`
  if (readFileSync(sidecar, 'utf8') !== wanted)
    fail(`checksum sidecar differs for ${basename(path)}`)
}

function headerString(header, start, length, label) {
  const bytes = header.subarray(start, start + length)
  const zero = bytes.indexOf(0)
  const content = zero < 0 ? bytes : bytes.subarray(0, zero)
  if (zero >= 0 && bytes.subarray(zero).some((byte) => byte !== 0)) {
    fail(`USTAR ${label} has data after its terminator`)
  }
  try {
    return decoder.decode(content)
  } catch {
    fail(`USTAR ${label} is not valid UTF-8`)
  }
}

function headerOctal(header, start, length, label) {
  const raw = header
    .subarray(start, start + length)
    .toString('ascii')
    .replace(/\0.*$/u, '')
    .trim()
  if (!/^[0-7]+$/u.test(raw)) fail(`USTAR ${label} is not canonical octal`)
  const value = Number.parseInt(raw, 8)
  if (!Number.isSafeInteger(value)) fail(`USTAR ${label} exceeds safe integer range`)
  return value
}

function verifyHeaderChecksum(header) {
  const recorded = headerOctal(header, 148, 8, 'checksum')
  let actual = 0
  for (let index = 0; index < header.byteLength; index += 1) {
    actual += index >= 148 && index < 156 ? 32 : header[index]
  }
  if (recorded !== actual)
    fail(`USTAR header checksum mismatch: expected ${recorded}, got ${actual}`)
}

function allZero(buffer) {
  return buffer.every((byte) => byte === 0)
}

export function parseUstarArchive(buffer) {
  if (!Buffer.isBuffer(buffer) || buffer.byteLength === 0 || buffer.byteLength % 512 !== 0) {
    fail('release archive must be a non-empty 512-byte-aligned USTAR buffer')
  }
  const rows = []
  let offset = 0
  let foundTrailer = false
  while (offset + 512 <= buffer.byteLength) {
    const header = buffer.subarray(offset, offset + 512)
    if (allZero(header)) {
      const trailer = buffer.subarray(offset)
      if (trailer.byteLength < 1024 || !allZero(trailer)) {
        fail('USTAR trailer must contain at least two zero blocks and no hidden data')
      }
      foundTrailer = true
      break
    }

    verifyHeaderChecksum(header)
    if (!header.subarray(257, 263).equals(Buffer.from('ustar\0'))) {
      fail('archive entry is not canonical USTAR')
    }
    if (!header.subarray(263, 265).equals(Buffer.from('00'))) {
      fail('archive entry has an unsupported USTAR version')
    }
    const type = header[156]
    if (type !== 0 && type !== 48) fail(`archive entry type ${type} is not a regular file`)
    if (headerString(header, 157, 100, 'link name')) fail('archive links are forbidden')

    const name = headerString(header, 0, 100, 'name')
    const prefix = headerString(header, 345, 155, 'prefix')
    const path = assertSafeReleasePath(prefix ? `${prefix}/${name}` : name)
    const mode = headerOctal(header, 100, 8, 'mode')
    if (mode !== 0o644 && mode !== 0o755) {
      fail(`archive mode must be normalized to 0644 or 0755: ${path}`)
    }
    if (headerOctal(header, 108, 8, 'uid') !== 0 || headerOctal(header, 116, 8, 'gid') !== 0) {
      fail(`archive ownership must be normalized to uid/gid zero: ${path}`)
    }
    if (headerOctal(header, 136, 12, 'mtime') !== 0) {
      fail(`archive mtime must be normalized to zero: ${path}`)
    }
    const bytes = headerOctal(header, 124, 12, 'size')
    const contentStart = offset + 512
    const contentEnd = contentStart + bytes
    const nextOffset = contentStart + Math.ceil(bytes / 512) * 512
    if (contentEnd > buffer.byteLength || nextOffset > buffer.byteLength) {
      fail(`archive content extends beyond EOF: ${path}`)
    }
    if (!allZero(buffer.subarray(contentEnd, nextOffset))) {
      fail(`archive padding contains non-zero bytes: ${path}`)
    }
    const content = buffer.subarray(contentStart, contentEnd)
    rows.push({ path, mode, bytes, sha256: sha256Bytes(content), content })
    offset = nextOffset
  }
  if (!foundTrailer) fail('archive has no canonical USTAR trailer')
  return rows
}

function validateManifestEntries(manifest) {
  if (!Array.isArray(manifest.entries) || manifest.entries.length === 0) {
    fail('release manifest has no entries')
  }
  const paths = new Set()
  let previous = ''
  for (const entry of manifest.entries) {
    if (
      canonicalJson(Object.keys(entry).sort()) !==
      canonicalJson(['bytes', 'mode', 'path', 'sha256'])
    ) {
      fail('release manifest entry has an unexpected shape')
    }
    assertSafeReleasePath(entry.path)
    if (paths.has(entry.path) || (previous && entry.path <= previous)) {
      fail(`release manifest entries are duplicate or unsorted: ${entry.path}`)
    }
    if ([...paths].some((path) => entry.path.startsWith(`${path}/`))) {
      fail(`release manifest has a file/directory collision: ${entry.path}`)
    }
    if (entry.mode !== '100644' && entry.mode !== '100755') {
      fail(`release manifest has an unsupported mode: ${entry.path}`)
    }
    if (!Number.isSafeInteger(entry.bytes) || entry.bytes < 0) {
      fail(`release manifest has an invalid byte count: ${entry.path}`)
    }
    assertHex(entry.sha256, `entry hash for ${entry.path}`)
    paths.add(entry.path)
    previous = entry.path
  }
  const identity = sha256Bytes(canonicalJson(manifest.entries))
  if (identity !== manifest.entries_manifest_sha256) fail('entries manifest SHA-256 differs')
}

function cockpitSourceFromRows(rows) {
  return cockpitSourceIdentityFromRows(rows)
}

function prebuiltV8Policy(manifest) {
  const artifacts = manifest.dependencies?.rust?.prebuilt_artifacts
  if (!Array.isArray(artifacts) || artifacts.length !== 1) {
    fail('release manifest must bind exactly one prebuilt build artifact')
  }
  const artifact = artifacts[0]
  if (
    artifact.name !== 'rusty_v8' ||
    artifact.crate !== 'v8' ||
    artifact.target !== SUPPORTED_CARGO_TARGET ||
    artifact.profile !== 'release' ||
    artifact.format !== 'decompressed-static-library' ||
    !Number.isSafeInteger(artifact.bytes) ||
    artifact.bytes <= 0
  ) {
    fail('release manifest has an invalid rusty_v8 build-artifact policy')
  }
  assertHex(artifact.sha256, 'rusty_v8 archive hash')
  return artifact
}

export function verifyReleaseSet(manifestPath) {
  const canonicalManifest = regularFile(resolve(manifestPath), 'release manifest')
  const manifestBytes = readFileSync(canonicalManifest)
  let manifest
  try {
    manifest = JSON.parse(manifestBytes)
  } catch {
    fail('release manifest is not valid JSON')
  }
  if (manifest.schema !== RELEASE_SCHEMA || manifest.scope !== RELEASE_SCOPE) {
    fail('release manifest schema or scope is unsupported')
  }
  assertHex(manifest.manifest_sha256, 'semantic manifest identity')
  const { manifest_sha256: semanticIdentity, ...unsigned } = manifest
  if (sha256Bytes(canonicalJson(unsigned)) !== semanticIdentity) {
    fail('semantic manifest SHA-256 differs')
  }
  assertChecksumSidecar(canonicalManifest, sha256Bytes(manifestBytes))
  validateManifestEntries(manifest)
  if (manifest.source?.release_inputs_dirty !== false) fail('release inputs were not clean')
  assertGitIdentity(manifest.source?.commit, 'source commit')
  assertGitIdentity(manifest.source?.tree, 'source tree')
  assertHex(manifest.source?.cockpit_source_sha256, 'cockpit source identity')
  if (manifest.policy?.clean_host_proven !== false) {
    fail('source producer must not claim clean-host proof')
  }
  if (manifest.support?.cargo_target !== SUPPORTED_CARGO_TARGET) {
    fail('release manifest targets an unsupported Cargo platform')
  }
  prebuiltV8Policy(manifest)

  const artifactName = manifest.artifact?.name
  if (!artifactName || basename(artifactName) !== artifactName)
    fail('artifact name must be a basename')
  if (manifest.artifact.media_type !== 'application/x-tar')
    fail('artifact media type is unsupported')
  assertHex(manifest.artifact.sha256, 'artifact hash')
  const archivePath = regularFile(join(dirname(canonicalManifest), artifactName), 'release archive')
  const archiveInfo = statSync(archivePath)
  if (archiveInfo.size !== manifest.artifact.bytes) fail('release archive byte count differs')
  if (sha256File(archivePath) !== manifest.artifact.sha256) fail('release archive SHA-256 differs')
  assertChecksumSidecar(archivePath, manifest.artifact.sha256)

  const rows = parseUstarArchive(readFileSync(archivePath))
  if (rows.length !== manifest.entries.length) fail('archive entry count differs from manifest')
  for (let index = 0; index < rows.length; index += 1) {
    const row = rows[index]
    const expected = manifest.entries[index]
    if (
      row.path !== expected.path ||
      row.mode !== (expected.mode === '100755' ? 0o755 : 0o644) ||
      row.bytes !== expected.bytes ||
      row.sha256 !== expected.sha256
    ) {
      fail(`archive entry differs from manifest at index ${index}: ${row.path}`)
    }
  }
  if (cockpitSourceFromRows(rows) !== manifest.source.cockpit_source_sha256) {
    fail('archive cockpit source identity differs from manifest')
  }
  return {
    manifest,
    manifestPath: canonicalManifest,
    manifestFileSha256: sha256Bytes(manifestBytes),
    archivePath,
    rows,
  }
}

export function extractVerifiedRows(rows, root) {
  const sourceRoot = join(root, 'source')
  if (existsSync(sourceRoot)) fail('fresh extraction target already exists')
  mkdirSync(sourceRoot, { mode: 0o700 })
  for (const row of rows) {
    const destination = join(sourceRoot, row.path)
    const lexical = relative(sourceRoot, destination)
    if (lexical === '..' || lexical.startsWith(`..${sep}`))
      fail(`extraction path escapes: ${row.path}`)
    mkdirSync(dirname(destination), { recursive: true, mode: 0o755 })
    writeFileSync(destination, row.content, {
      flag: 'wx',
      mode: row.mode,
    })
    chmodSync(destination, row.mode)
    const info = lstatSync(destination)
    if (!info.isFile() || info.isSymbolicLink() || info.size !== row.bytes) {
      fail(`extracted entry identity differs: ${row.path}`)
    }
  }
  return sourceRoot
}

function systemMounts() {
  const mounts = []
  for (const path of ['/usr', '/bin', '/sbin', '/lib', '/lib64', '/etc']) {
    if (existsSync(path)) mounts.push('--ro-bind', path, path)
  }
  return mounts
}

function cargoCacheMounts(cargoHome) {
  const mounts = ['--dir', '/opt/cargo-home']
  for (const name of ['bin', 'registry', 'git']) {
    const source = join(cargoHome, name)
    if (existsSync(source)) mounts.push('--ro-bind', source, `/opt/cargo-home/${name}`)
  }
  return mounts
}

function isolatedEnvironment() {
  const cargoHome = realpathSync(process.env.CARGO_HOME || join(homedir(), '.cargo'))
  const rustupHome = realpathSync(process.env.RUSTUP_HOME || join(homedir(), '.rustup'))
  return {
    cargoHome,
    rustupHome,
    env: {
      HOME: '/workspace/.release-home',
      PATH: '/opt/cargo-home/bin:/usr/bin:/bin',
      CARGO_HOME: '/opt/cargo-home',
      RUSTUP_HOME: '/opt/rustup-home',
      CARGO_NET_OFFLINE: 'true',
      CARGO_INCREMENTAL: '0',
      CARGO_TERM_COLOR: 'never',
      LANG: 'C.UTF-8',
      LC_ALL: 'C.UTF-8',
      SOURCE_DATE_EPOCH: '0',
      TMPDIR: '/tmp',
    },
  }
}

function bubblewrapArgs(sourceRoot, { writable, cargoHome, rustupHome, prebuiltArchive, command }) {
  return [
    '--die-with-parent',
    '--new-session',
    '--unshare-user',
    '--unshare-pid',
    '--unshare-ipc',
    '--unshare-uts',
    '--unshare-net',
    '--cap-drop',
    'ALL',
    ...systemMounts(),
    '--proc',
    '/proc',
    '--dev',
    '/dev',
    '--tmpfs',
    '/tmp',
    '--dir',
    '/opt',
    ...cargoCacheMounts(cargoHome),
    '--ro-bind',
    rustupHome,
    '/opt/rustup-home',
    ...(prebuiltArchive
      ? [
          '--dir',
          '/opt/release-inputs',
          '--ro-bind',
          prebuiltArchive,
          '/opt/release-inputs/librusty_v8.a',
        ]
      : []),
    '--dir',
    '/home',
    writable ? '--bind' : '--ro-bind',
    sourceRoot,
    '/workspace',
    '--chdir',
    '/workspace',
    ...command,
  ]
}

export function verifyPinnedV8Archive(v8Archive, manifest) {
  const expectedV8 = prebuiltV8Policy(manifest)
  if (!v8Archive) fail('offline release build requires --v8-archive with the pinned static library')
  const canonicalV8 = regularFile(resolve(v8Archive), 'rusty_v8 static library')
  const v8Info = statSync(canonicalV8)
  if (v8Info.size !== expectedV8.bytes || sha256File(canonicalV8) !== expectedV8.sha256) {
    fail('rusty_v8 static library differs from the pinned supply-chain policy')
  }
  return { path: canonicalV8, policy: expectedV8 }
}

export function buildExtractedRelease(sourceRoot, manifest, { v8Archive }) {
  const bubblewrap = executableFile(BUBBLEWRAP, 'bubblewrap')
  const pinnedV8 = verifyPinnedV8Archive(v8Archive, manifest)
  const expectedV8 = pinnedV8.policy
  const canonicalV8 = pinnedV8.path
  mkdirSync(join(sourceRoot, '.release-home'), { mode: 0o700 })
  const isolated = isolatedEnvironment()
  const build = run(
    bubblewrap,
    bubblewrapArgs(sourceRoot, {
      writable: true,
      ...isolated,
      prebuiltArchive: canonicalV8,
      command: [
        '/opt/cargo-home/bin/cargo',
        'build',
        '--manifest-path',
        'cockpit/Cargo.toml',
        '--bin',
        'angel',
        '--no-default-features',
        '--release',
        '--locked',
        '--offline',
      ],
    }),
    {
      cwd: '/',
      env: {
        ...isolated.env,
        ANGEL_BUILD_SOURCE_SHA256: manifest.source.cockpit_source_sha256,
        ANGEL_BUILD_RESOURCE_SHA256: manifest.entries_manifest_sha256,
        RUSTY_V8_ARCHIVE: '/opt/release-inputs/librusty_v8.a',
      },
    },
  )
  const binaryPath = executableFile(
    join(sourceRoot, 'cockpit/target/release/angel'),
    'extracted release binary',
  )
  const probe = run(
    bubblewrap,
    bubblewrapArgs(sourceRoot, {
      writable: false,
      ...isolated,
      command: ['/workspace/cockpit/target/release/angel', '--build-info', '--json'],
    }),
    { cwd: '/', env: isolated.env, timeout: 30_000 },
  )
  let buildInfo
  try {
    buildInfo = JSON.parse(probe.stdout)
  } catch {
    fail('release binary did not emit valid build-info JSON')
  }
  if (
    buildInfo.schema !== 'angel-build-info/v1' ||
    buildInfo.cockpit_source_sha256 !== manifest.source.cockpit_source_sha256 ||
    buildInfo.resources?.sha256 !== manifest.entries_manifest_sha256 ||
    !Array.isArray(buildInfo.capabilities) ||
    !REQUIRED_RUNNER_CAPABILITIES.every((capability) => buildInfo.capabilities.includes(capability))
  ) {
    fail('release binary build-info is not bound to the packaged cockpit source')
  }
  const runnerContract = verifyRunnerContractSmoke(binaryPath)
  const log = Buffer.from(`${build.stdout}\0${build.stderr}`)
  return {
    profile: 'release',
    cargo_policy: 'locked + offline',
    network_isolation: 'bubblewrap-v1 (network namespace unshared)',
    producing_worktree_mounted: false,
    host_cargo_cache_reused: true,
    host_cargo_cache_scope: 'executables + registry/git dependency caches; credentials excluded',
    host_rust_toolchain_reused: true,
    prebuilt_artifacts: [
      {
        name: expectedV8.name,
        crate_version: expectedV8.crate_version,
        bytes: expectedV8.bytes,
        sha256: expectedV8.sha256,
        trust: expectedV8.trust,
      },
    ],
    clean_host_proven: false,
    duration_ms: build.durationMs,
    build_log_sha256: sha256Bytes(log),
    binary: {
      bytes: statSync(binaryPath).size,
      sha256: sha256File(binaryPath),
      build_info: buildInfo,
    },
    runner_contract: runnerContract,
  }
}

function atomicWrite(path, value) {
  const temporary = `${path}.tmp-${process.pid}`
  writeFileSync(temporary, value, { mode: 0o644 })
  renameSync(temporary, path)
}

export function runReleaseVerification(manifestPath, { receiptPath, v8Archive } = {}) {
  const verified = verifyReleaseSet(manifestPath)
  const scratch = mkdtempSync(join(tmpdir(), 'angelX-release-verification-'))
  try {
    const sourceRoot = extractVerifiedRows(verified.rows, scratch)
    const build = buildExtractedRelease(sourceRoot, verified.manifest, { v8Archive })
    const receipt = {
      schema: VERIFICATION_SCHEMA,
      release_schema: verified.manifest.schema,
      release_scope: verified.manifest.scope,
      manifest_file_sha256: verified.manifestFileSha256,
      semantic_manifest_sha256: verified.manifest.manifest_sha256,
      artifact_sha256: verified.manifest.artifact.sha256,
      entry_count: verified.rows.length,
      extraction: {
        exact_paths_modes_sizes_and_hashes: true,
        symlinks_or_special_files: 0,
        source_checkout_reused: false,
      },
      build,
    }
    const output = resolve(
      receiptPath || verified.manifestPath.replace(/\.manifest\.json$/u, '.verification.json'),
    )
    if (output === verified.manifestPath)
      fail('verification receipt path must differ from manifest')
    atomicWrite(output, canonicalJson(receipt))
    atomicWrite(`${output}.sha256`, `${sha256File(output)}  ${basename(output)}\n`)
    return { receipt, receiptPath: output }
  } finally {
    rmSync(scratch, { recursive: true, force: true })
  }
}

function parseArgs(argv) {
  if (argv.length === 0) {
    fail('usage: verify-release-evidence.mjs MANIFEST --v8-archive PATH [--receipt PATH]')
  }
  const options = { manifestPath: argv[0] }
  for (let index = 1; index < argv.length; index += 1) {
    if (!['--receipt', '--v8-archive'].includes(argv[index]) || !argv[index + 1]) {
      fail(`unknown or incomplete argument: ${argv[index]}`)
    }
    if (argv[index] === '--receipt') options.receiptPath = argv[index + 1]
    else options.v8Archive = argv[index + 1]
    index += 1
  }
  if (!options.v8Archive) fail('--v8-archive is required for an offline release build')
  return options
}

const invokedPath = process.argv[1] ? pathToFileURL(resolve(process.argv[1])).href : ''
if (import.meta.url === invokedPath) {
  try {
    const args = parseArgs(process.argv.slice(2))
    const result = runReleaseVerification(args.manifestPath, args)
    process.stdout.write(
      canonicalJson({
        schema: result.receipt.schema,
        receipt: result.receiptPath,
        artifact_sha256: result.receipt.artifact_sha256,
        entry_count: result.receipt.entry_count,
        build: result.receipt.build,
      }),
    )
  } catch (error) {
    process.stderr.write(`release verification failed: ${error.message}\n`)
    process.exitCode = 1
  }
}
