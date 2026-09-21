import assert from 'node:assert/strict'
import { spawnSync } from 'node:child_process'
import { chmodSync, mkdtempSync, mkdirSync, readFileSync, rmSync, writeFileSync } from 'node:fs'
import { tmpdir } from 'node:os'
import { dirname, join } from 'node:path'
import test from 'node:test'

import {
  buildManifest,
  canonicalJson,
  cockpitSourceSha256,
  createSourceArchive,
  inventoryReleaseFiles,
  sha256Bytes,
  sha256File,
} from '../../scripts/release/release-evidence.mjs'
import {
  extractVerifiedRows,
  parseUstarArchive,
  verifyPinnedV8Archive,
  verifyReleaseSet,
} from '../../scripts/release/verify-release-evidence.mjs'

function command(cwd, executable, args) {
  const result = spawnSync(executable, args, { cwd, encoding: 'utf8' })
  assert.equal(result.status, 0, result.stderr)
  return result.stdout.trim()
}

function write(path, value, mode = 0o644) {
  mkdirSync(dirname(path), { recursive: true })
  writeFileSync(path, value, { mode })
  chmodSync(path, mode)
}

function fixture(t, { dotmax = false } = {}) {
  const root = mkdtempSync(join(tmpdir(), 'angelX-release-verifier-test-'))
  t.after(() => rmSync(root, { recursive: true, force: true }))
  command(root, 'git', ['init', '--quiet'])
  command(root, 'git', ['config', 'user.name', 'Release Verifier Test'])
  command(root, 'git', ['config', 'user.email', 'release-verifier@example.invalid'])
  write(join(root, 'cockpit/Cargo.toml'), '[package]\nname="angelX-cockpit"\nversion="0.1.0"\n')
  write(join(root, 'cockpit/Cargo.lock'), 'version = 4\n', 0o660)
  write(join(root, 'cockpit/src/main.rs'), 'fn main() {}\n')
  write(join(root, 'cockpit/src/helper.sh'), '#!/bin/sh\nexit 0\n', 0o755)
  write(
    join(root, 'cockpit/personas/reviewer/PERSONA.md'),
    '---\nname: reviewer\ndescription: fixture\n---\nreview fixture\n',
  )
  if (dotmax) {
    write(join(root, 'vendor/dotmax/Cargo.toml'), '[package]\nname="dotmax"\nversion="1.0.0"\n')
    write(join(root, 'vendor/dotmax/src/lib.rs'), 'pub fn dotmax() -> u8 { 1 }\n')
    write(join(root, 'vendor/dotmax/src/progress/mod.rs'), 'pub fn progress() -> u8 { 1 }\n')
  }
  command(root, 'git', ['add', 'cockpit', ...(dotmax ? ['vendor/dotmax'] : [])])
  command(root, 'git', ['commit', '--quiet', '-m', 'fixture'])

  const releasePaths = ['cockpit', ...(dotmax ? ['vendor/dotmax'] : [])]
  const entries = inventoryReleaseFiles(root, releasePaths)
  const artifact = createSourceArchive(root, join(root, 'release/source.tar'), entries)
  const commit = command(root, 'git', ['rev-parse', 'HEAD'])
  const tree = command(root, 'git', ['rev-parse', 'HEAD^{tree}'])
  const sourceIdentity = cockpitSourceSha256(root, entries)
  const pinnedV8Bytes = Buffer.alloc(10, 9)
  const manifest = buildManifest({
    version: '1.2.3',
    commit,
    tree,
    entries,
    archive: artifact,
    nodeDependencies: { lockfile_sha256: 'a'.repeat(64) },
    rustDependencies: {
      toolchain: { pinned_version: '1.95.0' },
      prebuilt_artifacts: [
        {
          name: 'rusty_v8',
          crate: 'v8',
          crate_version: '150.0.0',
          target: 'x86_64-unknown-linux-gnu',
          profile: 'release',
          format: 'decompressed-static-library',
          bytes: 10,
          sha256: sha256Bytes(pinnedV8Bytes),
          trust: 'fixture only',
        },
      ],
    },
    tools: { node: 'v24.0.0' },
    cockpitSource: sourceIdentity,
  })
  const manifestPath = join(root, 'release/source.manifest.json')
  write(manifestPath, canonicalJson(manifest))
  write(`${manifestPath}.sha256`, `${sha256File(manifestPath)}  source.manifest.json\n`)
  write(`${join(root, 'release/source.tar')}.sha256`, `${artifact.sha256}  source.tar\n`)
  return {
    root,
    manifestPath,
    archivePath: join(root, 'release/source.tar'),
    pinnedV8Bytes,
    releasePaths,
  }
}

function rewriteHeaderChecksum(header) {
  header.fill(32, 148, 156)
  let sum = 0
  for (const byte of header) sum += byte
  header.write(`${sum.toString(8).padStart(6, '0')}\0 `, 148, 8, 'ascii')
}

test('verifier binds manifest sidecars to normalized USTAR paths, modes, sizes, and bytes', (t) => {
  const release = fixture(t)
  const verified = verifyReleaseSet(release.manifestPath)
  assert.equal(verified.rows.length, 5)
  assert.deepEqual(
    verified.rows.map((row) => [row.path, row.mode]),
    [
      ['cockpit/Cargo.lock', 0o644],
      ['cockpit/Cargo.toml', 0o644],
      ['cockpit/personas/reviewer/PERSONA.md', 0o644],
      ['cockpit/src/helper.sh', 0o755],
      ['cockpit/src/main.rs', 0o644],
    ],
  )
  const extraction = mkdtempSync(join(tmpdir(), 'angelX-release-extract-test-'))
  t.after(() => rmSync(extraction, { recursive: true, force: true }))
  const source = extractVerifiedRows(verified.rows, extraction)
  assert.equal(readFileSync(join(source, 'cockpit/src/main.rs'), 'utf8'), 'fn main() {}\n')
})

test('archive byte tampering fails before extraction', (t) => {
  const release = fixture(t)
  const archive = readFileSync(release.archivePath)
  archive[512] ^= 1
  writeFileSync(release.archivePath, archive)
  assert.throws(() => verifyReleaseSet(release.manifestPath), /archive SHA-256 differs/u)
})

test('verifier rejects resealed dotmax manifest and nested source mutations', (t) => {
  for (const [path, mutation] of [
    ['vendor/dotmax/Cargo.toml', '[package]\nname="dotmax"\nversion="2.0.0"\n'],
    ['vendor/dotmax/src/progress/mod.rs', 'pub fn progress() -> u8 { 2 }\n'],
  ]) {
    const release = fixture(t, { dotmax: true })
    const original = verifyReleaseSet(release.manifestPath).manifest
    write(join(release.root, path), mutation)
    command(release.root, 'git', ['add', path])
    const entries = inventoryReleaseFiles(release.root, release.releasePaths)
    const artifact = createSourceArchive(release.root, release.archivePath, entries)
    const resealed = buildManifest({
      version: original.version,
      commit: original.source.commit,
      tree: original.source.tree,
      entries,
      archive: artifact,
      nodeDependencies: original.dependencies.node,
      rustDependencies: original.dependencies.rust,
      tools: original.tools,
      cockpitSource: original.source.cockpit_source_sha256,
    })
    write(release.manifestPath, canonicalJson(resealed))
    write(
      `${release.manifestPath}.sha256`,
      `${sha256File(release.manifestPath)}  source.manifest.json\n`,
    )
    write(`${release.archivePath}.sha256`, `${artifact.sha256}  source.tar\n`)

    assert.throws(
      () => verifyReleaseSet(release.manifestPath),
      /archive cockpit source identity differs from manifest/u,
      `${path} must be independently rebound by the verifier`,
    )
  }
})

test('offline prebuilt dependency must match the manifest byte count and SHA-256', (t) => {
  const release = fixture(t)
  const manifest = verifyReleaseSet(release.manifestPath).manifest
  const archive = join(release.root, 'release/librusty_v8.a')
  write(archive, release.pinnedV8Bytes)
  assert.equal(verifyPinnedV8Archive(archive, manifest).policy.name, 'rusty_v8')
  write(archive, Buffer.alloc(10, 8))
  assert.throws(() => verifyPinnedV8Archive(archive, manifest), /differs from the pinned/u)
})

test('semantic manifest tampering fails even when JSON remains valid', (t) => {
  const release = fixture(t)
  const manifest = JSON.parse(readFileSync(release.manifestPath, 'utf8'))
  manifest.entries[0].bytes += 1
  write(release.manifestPath, canonicalJson(manifest))
  write(
    `${release.manifestPath}.sha256`,
    `${sha256File(release.manifestPath)}  source.manifest.json\n`,
  )
  assert.throws(() => verifyReleaseSet(release.manifestPath), /semantic manifest SHA-256 differs/u)
})

test('USTAR parser rejects links and checkout-dependent permission modes with valid headers', (t) => {
  const release = fixture(t)
  const original = readFileSync(release.archivePath)

  const link = Buffer.from(original)
  link[156] = '2'.charCodeAt(0)
  rewriteHeaderChecksum(link.subarray(0, 512))
  assert.throws(() => parseUstarArchive(link), /not a regular file/u)

  const mode = Buffer.from(original)
  mode.fill(0, 100, 108)
  mode.write('0000664\0', 100, 8, 'ascii')
  rewriteHeaderChecksum(mode.subarray(0, 512))
  assert.throws(() => parseUstarArchive(mode), /normalized to 0644 or 0755/u)
})

test('USTAR parser rejects hidden trailer data even with an unchanged release payload', (t) => {
  const release = fixture(t)
  const archive = readFileSync(release.archivePath)
  const final = Buffer.from(archive)
  final[final.length - 1] = 1
  assert.throws(() => parseUstarArchive(final), /no hidden data/u)
  assert.notEqual(sha256Bytes(final), sha256Bytes(archive))
})
