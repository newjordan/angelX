#!/usr/bin/env node

import { spawnSync } from 'node:child_process'
import {
  chmodSync,
  copyFileSync,
  existsSync,
  lstatSync,
  mkdirSync,
  mkdtempSync,
  realpathSync,
  renameSync,
  rmSync,
  statSync,
  writeFileSync,
} from 'node:fs'
import { tmpdir } from 'node:os'
import { basename, join, resolve } from 'node:path'
import { pathToFileURL } from 'node:url'

import {
  ReleaseGateError,
  SUPPORTED_CARGO_TARGET,
  canonicalJson,
  sha256Bytes,
  sha256File,
} from './release-evidence.mjs'
import {
  REQUIRED_RUNNER_CAPABILITIES,
  extractVerifiedRows,
  verifyPinnedV8Archive,
  verifyReleaseSet,
} from './verify-release-evidence.mjs'
import { verifyRunnerContractSmoke } from './verify-runner-smoke.mjs'

export const CONTAINER_VERIFICATION_SCHEMA = 'angel0-clean-container-release-verification/v1'
const DOCKER = '/usr/bin/docker'
const MAX_COMMAND_OUTPUT = 64 * 1024 * 1024
const FETCH_TIMEOUT_MS = 10 * 60 * 1000
const BUILD_TIMEOUT_MS = 20 * 60 * 1000

function fail(message) {
  throw new ReleaseGateError(message)
}

function executableFile(path, label) {
  if (!existsSync(path)) fail(`${label} is unavailable: ${path}`)
  const info = lstatSync(path)
  if (!info.isFile() || info.isSymbolicLink() || (info.mode & 0o111) === 0) {
    fail(`${label} must be an executable regular non-symlink file`)
  }
  return realpathSync(path)
}

function run(command, args, { env, timeout = BUILD_TIMEOUT_MS } = {}) {
  const started = performance.now()
  const result = spawnSync(command, args, {
    cwd: '/',
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

function dockerEnvironment(home) {
  return {
    HOME: home,
    PATH: '/usr/bin:/bin',
    LANG: 'C.UTF-8',
    LC_ALL: 'C.UTF-8',
  }
}

export function cleanContainerPolicy(manifest) {
  const images = manifest.dependencies?.rust?.clean_build_container_images
  if (!Array.isArray(images) || images.length !== 1) {
    fail('release manifest must bind exactly one clean build container image')
  }
  const image = images[0]
  if (
    image.name !== 'clean_release_builder' ||
    !/^sha256:[0-9a-f]{64}$/u.test(String(image.digest || '')) ||
    image.reference !== image.digest ||
    image.local_tag !== 'angel0-clean-builder:rust-1.95.0-bookworm-v1' ||
    !/^sha256:[0-9a-f]{64}$/u.test(String(image.base_digest || '')) ||
    image.base_reference !== `docker.io/library/rust@${image.base_digest}` ||
    !/^[0-9a-f]{64}$/u.test(String(image.dockerfile_sha256 || '')) ||
    image.operating_system !== 'linux' ||
    image.architecture !== 'amd64' ||
    image.rust_version !== manifest.support?.rust ||
    !String(image.distribution).includes('bookworm') ||
    !Array.isArray(image.native_packages) ||
    image.native_packages.length !== 9 ||
    !String(image.trust).includes('no signature')
  ) {
    fail('clean build container policy is incomplete or inconsistent')
  }
  return image
}

export function validateDockerImageInspection(policy, inspection) {
  if (!inspection || typeof inspection !== 'object') fail('Docker image inspection is absent')
  if (
    inspection.Id !== policy.digest ||
    inspection.Os !== policy.operating_system ||
    inspection.Architecture !== policy.architecture
  ) {
    fail('local Docker image identity or platform differs from release policy')
  }
  const repoTags = Array.isArray(inspection.RepoTags) ? inspection.RepoTags : []
  if (!repoTags.includes(policy.local_tag)) {
    fail('local Docker image lacks the policy tag')
  }
  const environment = Array.isArray(inspection.Config?.Env) ? inspection.Config.Env : []
  const allowed = new Set(['PATH', 'RUSTUP_HOME', 'CARGO_HOME', 'RUST_VERSION', 'DEBIAN_FRONTEND'])
  for (const row of environment) {
    const key = String(row).split('=', 1)[0]
    if (!allowed.has(key)) fail(`clean build image has an unexpected environment key: ${key}`)
  }
  if (!environment.includes(`RUST_VERSION=${policy.rust_version}`)) {
    fail('clean build image Rust version differs from release policy')
  }
  if (!environment.includes('DEBIAN_FRONTEND=noninteractive')) {
    fail('clean build image does not retain noninteractive package policy')
  }
  if (inspection.Config?.Labels?.['org.opencontainers.image.base.name'] !== policy.base_reference) {
    fail('clean build image base label differs from release policy')
  }
  return {
    id: inspection.Id,
    local_tag: policy.local_tag,
    base_digest: policy.base_digest,
    dockerfile_sha256: policy.dockerfile_sha256,
    operating_system: inspection.Os,
    architecture: inspection.Architecture,
    rust_version: policy.rust_version,
  }
}

export function dockerNativePackageArgs(image) {
  const packages = image.native_packages.map((row) => row.slice(0, row.lastIndexOf('=')))
  return [
    'run',
    '--rm',
    '--pull',
    'never',
    '--network',
    'none',
    '--read-only',
    '--cap-drop',
    'ALL',
    '--security-opt',
    'no-new-privileges',
    image.reference,
    'dpkg-query',
    '-W',
    '-f=${binary:Package}=${Version}\n',
    ...packages,
  ]
}

export function validateNativePackages(policy, output) {
  const observed = String(output).split('\n').filter(Boolean).sort()
  const expected = [...policy.native_packages].sort()
  if (canonicalJson(observed) !== canonicalJson(expected)) {
    fail('clean build image native package versions differ from release policy')
  }
  return observed
}

function safeMountSource(path, label) {
  const canonical = realpathSync(path)
  if (canonical.includes(',')) fail(`${label} path cannot contain a comma for Docker --mount`)
  return canonical
}

function bindMount(source, target, readonly = false) {
  return `type=bind,src=${safeMountSource(source, 'bind source')},dst=${target}${
    readonly ? ',readonly' : ''
  }`
}

function containerEnvironmentArgs({ rustVersion, offline, sourceIdentity, resourceIdentity, v8 }) {
  const rows = [
    'HOME=/work/home',
    'CARGO_HOME=/work/cargo',
    'RUSTUP_HOME=/usr/local/rustup',
    `RUSTUP_TOOLCHAIN=${rustVersion}`,
    'PATH=/usr/local/cargo/bin:/usr/local/bin:/usr/bin:/bin',
    'CARGO_INCREMENTAL=0',
    'CARGO_TERM_COLOR=never',
    'LANG=C.UTF-8',
    'LC_ALL=C.UTF-8',
    'SOURCE_DATE_EPOCH=0',
    'TMPDIR=/tmp',
  ]
  if (offline) rows.push('CARGO_NET_OFFLINE=true')
  if (sourceIdentity) rows.push(`ANGEL_BUILD_SOURCE_SHA256=${sourceIdentity}`)
  if (resourceIdentity) rows.push(`ANGEL_BUILD_RESOURCE_SHA256=${resourceIdentity}`)
  if (v8) rows.push('RUSTY_V8_ARCHIVE=/work/inputs/librusty_v8.a')
  return rows.flatMap((row) => ['--env', row])
}

export function dockerPhaseArgs({
  phase,
  image,
  scratch,
  v8Archive,
  rustVersion,
  sourceIdentity,
  resourceIdentity,
  uid = process.getuid(),
  gid = process.getgid(),
}) {
  if (!['fetch', 'build', 'probe'].includes(phase)) fail(`unsupported container phase: ${phase}`)
  const network = phase === 'fetch' ? 'bridge' : 'none'
  const readonlySource = phase === 'probe'
  const args = [
    'run',
    '--rm',
    '--pull',
    'never',
    '--network',
    network,
    '--read-only',
    '--cap-drop',
    'ALL',
    '--security-opt',
    'no-new-privileges',
    '--pids-limit',
    '1024',
    '--user',
    `${uid}:${gid}`,
    '--tmpfs',
    '/tmp:rw,nosuid,nodev,noexec,size=1073741824',
    '--workdir',
    '/work/source',
    '--mount',
    bindMount(scratch, '/work', readonlySource),
    ...containerEnvironmentArgs({
      rustVersion,
      offline: phase !== 'fetch',
      sourceIdentity: phase === 'build' ? sourceIdentity : undefined,
      resourceIdentity: phase === 'build' ? resourceIdentity : undefined,
      v8: phase === 'build',
    }),
  ]
  if (phase === 'build') {
    args.push('--mount', bindMount(v8Archive, '/work/inputs/librusty_v8.a', true))
  }
  args.push(image.reference)
  if (phase === 'fetch') {
    args.push(
      'cargo',
      'fetch',
      '--manifest-path',
      'cockpit/Cargo.toml',
      '--locked',
      '--target',
      SUPPORTED_CARGO_TARGET,
    )
  } else if (phase === 'build') {
    args.push(
      'cargo',
      'build',
      '--manifest-path',
      'cockpit/Cargo.toml',
      '--bin',
      'angel',
      '--no-default-features',
      '--release',
      '--locked',
      '--offline',
    )
  } else {
    args.push('/work/source/cockpit/target/release/angel', '--build-info', '--json')
  }
  return args
}

function logIdentity(result) {
  return sha256Bytes(Buffer.from(`${result.stdout}\0${result.stderr}`))
}

export function publishInstallableBinary(binaryPath, manifestPath) {
  const output = resolve(manifestPath.replace(/\.manifest\.json$/u, '.cockpit-linux-x86_64'))
  if (output === resolve(manifestPath)) fail('installable binary path must differ from manifest')
  const temporary = `${output}.tmp-${process.pid}`
  copyFileSync(binaryPath, temporary)
  chmodSync(temporary, 0o755)
  const artifact = {
    name: basename(output),
    media_type: 'application/vnd.angel0.cockpit-executable',
    platform: 'linux-x86_64',
    mode: '0755',
    bytes: statSync(temporary).size,
    sha256: sha256File(temporary),
  }
  renameSync(temporary, output)
  const sidecar = `${output}.sha256`
  const temporarySidecar = `${sidecar}.tmp-${process.pid}`
  writeFileSync(temporarySidecar, `${artifact.sha256}  ${artifact.name}\n`, { mode: 0o644 })
  renameSync(temporarySidecar, sidecar)
  return { artifact, path: output }
}

export function runContainerVerification(manifestPath, { v8Archive, receiptPath } = {}) {
  if (process.platform !== 'linux' || process.arch !== 'x64') {
    fail('clean container verification currently supports only Linux x86_64')
  }
  const docker = executableFile(DOCKER, 'Docker CLI')
  const verified = verifyReleaseSet(manifestPath)
  const image = cleanContainerPolicy(verified.manifest)
  const pinnedV8 = verifyPinnedV8Archive(v8Archive, verified.manifest)
  const scratch = mkdtempSync(join(tmpdir(), 'angel0-clean-container-'))
  try {
    const sourceRoot = extractVerifiedRows(verified.rows, scratch)
    mkdirSync(join(scratch, 'cargo'), { mode: 0o700 })
    mkdirSync(join(scratch, 'home'), { mode: 0o700 })
    mkdirSync(join(scratch, 'inputs'), { mode: 0o700 })
    if (sourceRoot !== join(scratch, 'source')) fail('fresh extraction root identity differs')

    const hostEnv = dockerEnvironment(join(scratch, 'docker-home'))
    mkdirSync(hostEnv.HOME, { mode: 0o700 })
    const inspected = run(docker, ['image', 'inspect', image.reference], {
      env: hostEnv,
      timeout: 30_000,
    })
    let inspection
    try {
      const parsed = JSON.parse(inspected.stdout)
      inspection = Array.isArray(parsed) && parsed.length === 1 ? parsed[0] : null
    } catch {
      fail('Docker image inspect did not return valid JSON')
    }
    const imageIdentity = validateDockerImageInspection(image, inspection)
    const packageInspection = run(docker, dockerNativePackageArgs(image), {
      env: hostEnv,
      timeout: 30_000,
    })
    const nativePackages = validateNativePackages(image, packageInspection.stdout)
    const common = {
      image,
      scratch,
      v8Archive: pinnedV8.path,
      rustVersion: image.rust_version,
      sourceIdentity: verified.manifest.source.cockpit_source_sha256,
      resourceIdentity: verified.manifest.entries_manifest_sha256,
    }
    const fetch = run(docker, dockerPhaseArgs({ ...common, phase: 'fetch' }), {
      env: hostEnv,
      timeout: FETCH_TIMEOUT_MS,
    })
    const build = run(docker, dockerPhaseArgs({ ...common, phase: 'build' }), {
      env: hostEnv,
      timeout: BUILD_TIMEOUT_MS,
    })
    const binaryPath = join(sourceRoot, 'cockpit/target/release/angel')
    const binaryInfo = lstatSync(binaryPath)
    if (!binaryInfo.isFile() || binaryInfo.isSymbolicLink() || (binaryInfo.mode & 0o111) === 0) {
      fail('clean container did not produce an executable regular release binary')
    }
    const probe = run(docker, dockerPhaseArgs({ ...common, phase: 'probe' }), {
      env: hostEnv,
      timeout: 30_000,
    })
    let buildInfo
    try {
      buildInfo = JSON.parse(probe.stdout)
    } catch {
      fail('clean-container binary did not emit valid build-info JSON')
    }
    if (
      buildInfo.schema !== 'angel-build-info/v1' ||
      buildInfo.cockpit_source_sha256 !== verified.manifest.source.cockpit_source_sha256 ||
      buildInfo.resources?.sha256 !== verified.manifest.entries_manifest_sha256 ||
      !Array.isArray(buildInfo.capabilities) ||
      !REQUIRED_RUNNER_CAPABILITIES.every((capability) =>
        buildInfo.capabilities.includes(capability),
      )
    ) {
      fail('clean-container binary build-info is not bound to packaged source')
    }
    const runnerContract = verifyRunnerContractSmoke(binaryPath)

    const installable = publishInstallableBinary(binaryPath, verified.manifestPath)

    const receipt = {
      schema: CONTAINER_VERIFICATION_SCHEMA,
      release_schema: verified.manifest.schema,
      release_scope: verified.manifest.scope,
      manifest_file_sha256: verified.manifestFileSha256,
      semantic_manifest_sha256: verified.manifest.manifest_sha256,
      artifact_sha256: verified.manifest.artifact.sha256,
      entry_count: verified.rows.length,
      container: {
        image: imageIdentity,
        native_packages: nativePackages,
        trust: image.trust,
        clean_container_filesystem_proven: true,
        fresh_cargo_home_proven: true,
        source_checkout_reused: false,
        host_cargo_cache_reused: false,
        host_rust_toolchain_reused: false,
        host_kernel_and_docker_daemon_reused: true,
        physical_clean_host_proven: false,
      },
      dependency_fetch: {
        command: `cargo fetch --locked --target ${SUPPORTED_CARGO_TARGET}`,
        network: 'Docker bridge (fetch phase only)',
        duration_ms: fetch.durationMs,
        log_sha256: logIdentity(fetch),
      },
      build: {
        command: 'cargo build --bin angel --no-default-features --release --locked --offline',
        network: 'none',
        duration_ms: build.durationMs,
        log_sha256: logIdentity(build),
        prebuilt_artifacts: [
          {
            name: pinnedV8.policy.name,
            bytes: pinnedV8.policy.bytes,
            sha256: pinnedV8.policy.sha256,
            trust: pinnedV8.policy.trust,
          },
        ],
        binary: {
          bytes: statSync(binaryPath).size,
          sha256: sha256File(binaryPath),
          build_info: buildInfo,
        },
        runner_contract: runnerContract,
        installable_binary: installable.artifact,
      },
    }
    const output = resolve(
      receiptPath ||
        verified.manifestPath.replace(/\.manifest\.json$/u, '.container-verification.json'),
    )
    if (output === verified.manifestPath) fail('container receipt path must differ from manifest')
    const temporary = `${output}.tmp-${process.pid}`
    writeFileSync(temporary, canonicalJson(receipt), { mode: 0o644 })
    renameSync(temporary, output)
    const sidecar = `${output}.sha256`
    const temporarySidecar = `${sidecar}.tmp-${process.pid}`
    writeFileSync(temporarySidecar, `${sha256File(output)}  ${basename(output)}\n`, { mode: 0o644 })
    renameSync(temporarySidecar, sidecar)
    return { receipt, receiptPath: output, installablePath: installable.path }
  } finally {
    rmSync(scratch, { recursive: true, force: true })
  }
}

function parseArgs(argv) {
  if (argv.length === 0) {
    fail('usage: verify-release-container.mjs MANIFEST --v8-archive PATH [--receipt PATH]')
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
  if (!options.v8Archive) fail('--v8-archive is required for the networkless build phase')
  return options
}

const invokedPath = process.argv[1] ? pathToFileURL(resolve(process.argv[1])).href : ''
if (import.meta.url === invokedPath) {
  try {
    const args = parseArgs(process.argv.slice(2))
    const result = runContainerVerification(args.manifestPath, args)
    process.stdout.write(
      canonicalJson({
        schema: result.receipt.schema,
        receipt: result.receiptPath,
        artifact_sha256: result.receipt.artifact_sha256,
        entry_count: result.receipt.entry_count,
        installable_binary: result.installablePath,
        container: result.receipt.container,
        build: result.receipt.build,
      }),
    )
  } catch (error) {
    process.stderr.write(`clean container verification failed: ${error.message}\n`)
    process.exitCode = 1
  }
}
