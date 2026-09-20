import assert from 'node:assert/strict'
import { chmodSync, mkdtempSync, readFileSync, rmSync, statSync, writeFileSync } from 'node:fs'
import { tmpdir } from 'node:os'
import { join } from 'node:path'
import test from 'node:test'

import {
  cleanContainerPolicy,
  dockerNativePackageArgs,
  dockerPhaseArgs,
  publishInstallableBinary,
  validateDockerImageInspection,
  validateNativePackages,
} from './verify-release-container.mjs'

const DIGEST = `sha256:${'6'.repeat(64)}`
const IMAGE = {
  name: 'clean_release_builder',
  reference: DIGEST,
  digest: DIGEST,
  local_tag: 'angel0-clean-builder:rust-1.95.0-bookworm-v1',
  base_reference: `docker.io/library/rust@${`sha256:${'5'.repeat(64)}`}`,
  base_digest: `sha256:${'5'.repeat(64)}`,
  dockerfile_sha256: '4'.repeat(64),
  operating_system: 'linux',
  architecture: 'amd64',
  distribution: 'Debian GNU/Linux 12 (bookworm)',
  rust_version: '1.95.0',
  native_packages: [
    'libavcodec-dev:amd64=7:5.1.9-0+deb12u1',
    'libavdevice-dev:amd64=7:5.1.9-0+deb12u1',
    'libavfilter-dev:amd64=7:5.1.9-0+deb12u1',
    'libavformat-dev:amd64=7:5.1.9-0+deb12u1',
    'libavutil-dev:amd64=7:5.1.9-0+deb12u1',
    'libclang-dev=1:14.0-55.7~deb12u1',
    'libswresample-dev:amd64=7:5.1.9-0+deb12u1',
    'libswscale-dev:amd64=7:5.1.9-0+deb12u1',
    'pkg-config:amd64=1.8.1-1',
  ],
  trust: 'locally built from a pinned base; no signature verified',
}

function manifest(image = IMAGE) {
  return {
    support: { rust: '1.95.0' },
    dependencies: { rust: { clean_build_container_images: [image] } },
  }
}

function fixture(t) {
  const scratch = mkdtempSync(join(tmpdir(), 'angel0-container-args-test-'))
  t.after(() => rmSync(scratch, { recursive: true, force: true }))
  const v8Archive = join(scratch, 'rusty-v8.a')
  writeFileSync(v8Archive, 'fixture')
  return { scratch, v8Archive }
}

function optionValue(args, option) {
  const index = args.indexOf(option)
  assert.notEqual(index, -1, `missing ${option}`)
  return args[index + 1]
}

function environmentRows(args) {
  const rows = []
  for (let index = 0; index < args.length; index += 1) {
    if (args[index] === '--env') rows.push(args[index + 1])
  }
  return rows
}

function phase(t, name) {
  const paths = fixture(t)
  return dockerPhaseArgs({
    phase: name,
    image: IMAGE,
    ...paths,
    rustVersion: '1.95.0',
    sourceIdentity: 'a'.repeat(64),
    uid: 1234,
    gid: 5678,
  })
}

test('container policy requires a digest-only derived image matching the release toolchain', () => {
  assert.equal(cleanContainerPolicy(manifest()), IMAGE)
  assert.throws(
    () => cleanContainerPolicy(manifest({ ...IMAGE, reference: 'rust:1.95.0-bookworm' })),
    /incomplete or inconsistent/u,
  )
  assert.throws(
    () => cleanContainerPolicy({ ...manifest(), support: { rust: '1.94.0' } }),
    /incomplete or inconsistent/u,
  )
})

test('Docker inspection must match digest, platform, registry identity, and a minimal image environment', () => {
  const inspection = {
    Id: DIGEST,
    RepoTags: ['angel0-clean-builder:rust-1.95.0-bookworm-v1'],
    Os: 'linux',
    Architecture: 'amd64',
    Config: {
      Env: [
        'PATH=/usr/local/cargo/bin:/usr/bin:/bin',
        'RUSTUP_HOME=/usr/local/rustup',
        'CARGO_HOME=/usr/local/cargo',
        'RUST_VERSION=1.95.0',
        'DEBIAN_FRONTEND=noninteractive',
      ],
      Labels: { 'org.opencontainers.image.base.name': IMAGE.base_reference },
    },
  }
  assert.equal(validateDockerImageInspection(IMAGE, inspection).id, DIGEST)
  assert.throws(
    () => validateDockerImageInspection(IMAGE, { ...inspection, Architecture: 'arm64' }),
    /identity or platform differs/u,
  )
  assert.throws(
    () =>
      validateDockerImageInspection(IMAGE, {
        ...inspection,
        Config: { Env: [...inspection.Config.Env, 'TOKEN=secret'] },
      }),
    /unexpected environment key/u,
  )
})

test('native package preflight is networkless and exact-version checked', () => {
  const args = dockerNativePackageArgs(IMAGE)
  assert.equal(optionValue(args, '--network'), 'none')
  assert.equal(optionValue(args, '--pull'), 'never')
  assert.ok(args.includes('dpkg-query'))
  assert.deepEqual(
    validateNativePackages(IMAGE, `${[...IMAGE.native_packages].reverse().join('\n')}\n`),
    [...IMAGE.native_packages].sort(),
  )
  assert.throws(
    () => validateNativePackages(IMAGE, `${IMAGE.native_packages.slice(1).join('\n')}\n`),
    /versions differ/u,
  )
})

test('fetch phase alone has bridge networking and can only perform locked Cargo fetch', (t) => {
  const args = phase(t, 'fetch')
  assert.equal(optionValue(args, '--network'), 'bridge')
  assert.equal(optionValue(args, '--pull'), 'never')
  assert.equal(optionValue(args, '--user'), '1234:5678')
  assert.deepEqual(args.slice(-7), [
    'cargo',
    'fetch',
    '--manifest-path',
    'cockpit/Cargo.toml',
    '--locked',
    '--target',
    'x86_64-unknown-linux-gnu',
  ])
  assert.ok(!environmentRows(args).some((row) => row.startsWith('CARGO_NET_OFFLINE=')))
  assert.ok(!environmentRows(args).some((row) => row.startsWith('RUSTY_V8_ARCHIVE=')))
  assert.ok(!args.some((row) => row.includes('librusty_v8.a')))
})

test('build phase is networkless, offline, source-bound, and mounts only the pinned V8 file', (t) => {
  const args = phase(t, 'build')
  assert.equal(optionValue(args, '--network'), 'none')
  assert.ok(args.includes('--offline'))
  assert.ok(args.includes('--no-default-features'))
  assert.ok(environmentRows(args).includes('CARGO_NET_OFFLINE=true'))
  assert.ok(environmentRows(args).includes(`ANGEL_BUILD_SOURCE_SHA256=${'a'.repeat(64)}`))
  assert.ok(environmentRows(args).includes('RUSTY_V8_ARCHIVE=/work/inputs/librusty_v8.a'))
  assert.ok(args.some((row) => row.endsWith('dst=/work/inputs/librusty_v8.a,readonly')))
  assert.ok(!args.some((row) => /TOKEN|SECRET|PASSWORD|PROXY/u.test(row)))
})

test('probe phase has no network, no dependency input, and a read-only verification tree', (t) => {
  const args = phase(t, 'probe')
  assert.equal(optionValue(args, '--network'), 'none')
  assert.ok(args.some((row) => row.endsWith('dst=/work,readonly')))
  assert.ok(!args.some((row) => row.includes('librusty_v8.a')))
  assert.deepEqual(args.slice(-3), [
    '/work/source/cockpit/target/release/angel',
    '--build-info',
    '--json',
  ])
})

test('clean build publishes one checksummed executable beside the source manifest', (t) => {
  const { scratch } = fixture(t)
  const binary = join(scratch, 'angel')
  const manifestPath = join(scratch, 'angel0-source-fixture.manifest.json')
  writeFileSync(binary, 'source-bound binary fixture\n', { mode: 0o755 })
  chmodSync(binary, 0o755)
  writeFileSync(manifestPath, '{}\n')
  const published = publishInstallableBinary(binary, manifestPath)
  assert.equal(published.path, join(scratch, 'angel0-source-fixture.cockpit-linux-x86_64'))
  assert.equal(published.artifact.mode, '0755')
  assert.equal(published.artifact.platform, 'linux-x86_64')
  assert.equal(statSync(published.path).mode & 0o777, 0o755)
  assert.equal(
    readFileSync(`${published.path}.sha256`, 'utf8'),
    `${published.artifact.sha256}  ${published.artifact.name}\n`,
  )
})
