#!/usr/bin/env node

import { spawnSync } from 'node:child_process'
import {
  chmodSync,
  copyFileSync,
  existsSync,
  lstatSync,
  mkdirSync,
  mkdtempSync,
  readFileSync,
  renameSync,
  rmSync,
  writeFileSync,
} from 'node:fs'
import { tmpdir } from 'node:os'
import { basename, dirname, join, relative, resolve, sep } from 'node:path'
import { pathToFileURL } from 'node:url'

import { ReleaseGateError, canonicalJson, sha256Bytes, sha256File } from './release-evidence.mjs'
import { CONTAINER_VERIFICATION_SCHEMA, cleanContainerPolicy } from './verify-release-container.mjs'
import { REQUIRED_RUNNER_CAPABILITIES, verifyReleaseSet } from './verify-release-evidence.mjs'
import { verifyRunnerContractSmoke } from './verify-runner-smoke.mjs'

export const INSTALL_VERIFICATION_SCHEMA = 'angel0-prefix-install-verification/v1'
export const PREFIX_INSTALL_SCHEMA = 'angel0-prefix-install/v1'
export const INSTALL_JOURNAL_SCHEMA = 'angel0-prefix-install-journal/v1'

const DOCKER = '/usr/bin/docker'
const MAX_COMMAND_OUTPUT = 8 * 1024 * 1024
const PROBE_TIMEOUT_MS = 30_000

function fail(message) {
  throw new ReleaseGateError(message)
}

function regularFile(path, label, { executable = false } = {}) {
  if (!existsSync(path)) fail(`${label} is unavailable: ${path}`)
  const info = lstatSync(path)
  if (!info.isFile() || info.isSymbolicLink()) {
    fail(`${label} must be a regular non-symlink file`)
  }
  if (executable && (info.mode & 0o111) === 0) fail(`${label} must be executable`)
  return info
}

function readJson(path, label) {
  regularFile(path, label)
  try {
    return JSON.parse(readFileSync(path, 'utf8'))
  } catch {
    fail(`${label} is not valid JSON`)
  }
}

function verifySidecar(path) {
  const sidecar = `${path}.sha256`
  regularFile(sidecar, 'checksum sidecar')
  const expected = `${sha256File(path)}  ${basename(path)}\n`
  if (readFileSync(sidecar, 'utf8') !== expected) {
    fail(`checksum sidecar differs for ${basename(path)}`)
  }
}

function atomicJson(path, value, mode = 0o644) {
  const temporary = `${path}.tmp-${process.pid}`
  writeFileSync(temporary, canonicalJson(value), { mode })
  renameSync(temporary, path)
}

function copyExecutable(source, destination) {
  const temporary = `${destination}.tmp-${process.pid}`
  copyFileSync(source, temporary)
  chmodSync(temporary, 0o755)
  renameSync(temporary, destination)
}

export function assertManagedPrefix(sandboxRoot, prefix) {
  const root = resolve(sandboxRoot)
  const target = resolve(prefix)
  const relation = relative(root, target)
  if (!relation || relation === '..' || relation.startsWith(`..${sep}`)) {
    fail('install prefix must be a strict descendant of the isolated lifecycle root')
  }
  if (existsSync(target) && lstatSync(target).isSymbolicLink()) {
    fail('install prefix cannot be a symbolic link')
  }
  return target
}

export function prefixLayout(sandboxRoot, prefix = join(sandboxRoot, 'prefix')) {
  const root = resolve(sandboxRoot)
  const managedPrefix = assertManagedPrefix(root, prefix)
  return {
    root,
    prefix: managedPrefix,
    binDirectory: join(managedPrefix, 'bin'),
    binary: join(managedPrefix, 'bin', 'angel'),
    shareDirectory: join(managedPrefix, 'share', 'angel0'),
    metadata: join(managedPrefix, 'share', 'angel0', 'install.json'),
    journal: join(managedPrefix, '.angel0-install-journal.json'),
    rollbackDirectory: join(managedPrefix, '.angel0-rollback'),
    rollbackBinary: join(managedPrefix, '.angel0-rollback', 'angel'),
    rollbackMetadata: join(managedPrefix, '.angel0-rollback', 'install.json'),
  }
}

function validateInstallCandidate(candidate, metadata) {
  const info = regularFile(candidate, 'install candidate', { executable: true })
  if (
    metadata.schema !== PREFIX_INSTALL_SCHEMA ||
    metadata.binary?.bytes !== info.size ||
    metadata.binary?.sha256 !== sha256File(candidate) ||
    metadata.binary?.mode !== '0755' ||
    metadata.binary?.platform !== 'linux-x86_64'
  ) {
    fail('install candidate differs from its source-bound install metadata')
  }
}

function currentInstallIdentity(layout) {
  if (!existsSync(layout.binary) && !existsSync(layout.metadata)) return null
  regularFile(layout.binary, 'installed binary', { executable: true })
  const metadata = readJson(layout.metadata, 'installed metadata')
  validateInstallCandidate(layout.binary, metadata)
  return {
    binary_sha256: sha256File(layout.binary),
    metadata_sha256: sha256File(layout.metadata),
  }
}

export function recoverInterruptedInstall(sandboxRoot, prefix = join(sandboxRoot, 'prefix')) {
  const layout = prefixLayout(sandboxRoot, prefix)
  if (!existsSync(layout.journal)) return { recovered: false }
  const journal = readJson(layout.journal, 'install journal')
  if (
    journal.schema !== INSTALL_JOURNAL_SCHEMA ||
    !/^[0-9a-f]{64}$/u.test(String(journal.previous?.binary_sha256 || '')) ||
    !/^[0-9a-f]{64}$/u.test(String(journal.previous?.metadata_sha256 || ''))
  ) {
    fail('install journal is incomplete or unsafe')
  }
  regularFile(layout.rollbackBinary, 'rollback binary', { executable: true })
  regularFile(layout.rollbackMetadata, 'rollback metadata')
  if (
    sha256File(layout.rollbackBinary) !== journal.previous.binary_sha256 ||
    sha256File(layout.rollbackMetadata) !== journal.previous.metadata_sha256
  ) {
    fail('rollback payload differs from the install journal')
  }
  const rollbackMetadata = readJson(layout.rollbackMetadata, 'rollback metadata')
  validateInstallCandidate(layout.rollbackBinary, rollbackMetadata)
  renameSync(layout.rollbackBinary, layout.binary)
  renameSync(layout.rollbackMetadata, layout.metadata)
  rmSync(layout.journal)
  rmSync(layout.rollbackDirectory, { recursive: true })
  return { recovered: true, restored_binary_sha256: sha256File(layout.binary) }
}

export function installResourceBundle(layout, resources) {
  const identity = resources.manifest.entries_manifest_sha256
  if (!/^[0-9a-f]{64}$/u.test(identity)) fail('invalid resource bundle identity')
  const destination = join(layout.shareDirectory, 'bundles', identity)
  const check = (root) => {
    for (const row of resources.rows) {
      const path = join(root, row.path)
      regularFile(path, 'installed resource')
      if (sha256File(path) !== sha256Bytes(row.content))
        fail(`installed resource differs: ${row.path}`)
    }
  }
  if (existsSync(destination)) {
    check(destination)
    return destination
  }
  const temporary = `${destination}.incoming-${process.pid}`
  mkdirSync(temporary, { recursive: true, mode: 0o755 })
  try {
    for (const row of resources.rows) {
      const path = resolve(temporary, row.path)
      if (!path.startsWith(`${temporary}${sep}`)) fail('resource path escapes its bundle')
      mkdirSync(dirname(path), { recursive: true, mode: 0o755 })
      writeFileSync(path, row.content, { flag: 'wx', mode: row.mode })
      chmodSync(path, row.mode)
    }
    check(temporary)
    renameSync(temporary, destination)
  } finally {
    if (existsSync(temporary)) rmSync(temporary, { recursive: true })
  }
  return destination
}

export function installCandidate({ sandboxRoot, prefix, candidate, metadata, resources }) {
  const layout = prefixLayout(sandboxRoot, prefix)
  validateInstallCandidate(candidate, metadata)
  mkdirSync(layout.binDirectory, { recursive: true, mode: 0o755 })
  mkdirSync(layout.shareDirectory, { recursive: true, mode: 0o755 })
  if (resources) {
    if (metadata.release?.resources_sha256 !== resources.manifest.entries_manifest_sha256)
      fail('install metadata differs from resource bundle identity')
    installResourceBundle(layout, resources)
  } else if (metadata.release?.resources_sha256) {
    fail('this release requires its source-bound resource bundle')
  }
  recoverInterruptedInstall(sandboxRoot, layout.prefix)
  const previous = currentInstallIdentity(layout)
  if (previous) {
    mkdirSync(layout.rollbackDirectory, { mode: 0o700 })
    copyExecutable(layout.binary, layout.rollbackBinary)
    copyFileSync(layout.metadata, layout.rollbackMetadata)
    atomicJson(layout.journal, {
      schema: INSTALL_JOURNAL_SCHEMA,
      previous,
      replacement_binary_sha256: metadata.binary.sha256,
    })
  }
  copyExecutable(candidate, layout.binary)
  atomicJson(layout.metadata, metadata)
  validateInstallCandidate(layout.binary, metadata)
  if (previous) {
    rmSync(layout.journal)
    rmSync(layout.rollbackDirectory, { recursive: true })
  }
  return { layout, replaced: Boolean(previous), binary_sha256: sha256File(layout.binary) }
}

export function injectInterruptedReplacement(sandboxRoot, prefix = join(sandboxRoot, 'prefix')) {
  const layout = prefixLayout(sandboxRoot, prefix)
  const previous = currentInstallIdentity(layout)
  if (!previous) fail('cannot inject an interrupted replacement without an installed binary')
  mkdirSync(layout.rollbackDirectory, { mode: 0o700 })
  copyExecutable(layout.binary, layout.rollbackBinary)
  copyFileSync(layout.metadata, layout.rollbackMetadata)
  atomicJson(layout.journal, {
    schema: INSTALL_JOURNAL_SCHEMA,
    previous,
    replacement_binary_sha256: sha256Bytes('deliberately incomplete replacement'),
  })
  const incomplete = `${layout.binary}.incomplete`
  writeFileSync(incomplete, 'deliberately incomplete replacement\n', { mode: 0o755 })
  renameSync(incomplete, layout.binary)
  return { previous, faulted_binary_sha256: sha256File(layout.binary) }
}

export function validateInstallInputs(verified, { containerReceiptPath, binaryPath } = {}) {
  const receiptPath = resolve(
    containerReceiptPath ||
      verified.manifestPath.replace(/\.manifest\.json$/u, '.container-verification.json'),
  )
  verifySidecar(receiptPath)
  const receipt = readJson(receiptPath, 'clean-container receipt')
  const artifact = receipt.build?.installable_binary
  if (
    receipt.schema !== CONTAINER_VERIFICATION_SCHEMA ||
    receipt.semantic_manifest_sha256 !== verified.manifest.manifest_sha256 ||
    receipt.manifest_file_sha256 !== verified.manifestFileSha256 ||
    receipt.artifact_sha256 !== verified.manifest.artifact.sha256 ||
    artifact?.platform !== 'linux-x86_64' ||
    artifact?.mode !== '0755' ||
    basename(String(artifact?.name || '')) !== artifact?.name ||
    !/^[0-9a-f]{64}$/u.test(String(artifact?.sha256 || '')) ||
    !Number.isSafeInteger(artifact?.bytes) ||
    receipt.build?.binary?.sha256 !== artifact.sha256 ||
    receipt.build?.binary?.bytes !== artifact.bytes ||
    receipt.build?.binary?.build_info?.cockpit_source_sha256 !==
      verified.manifest.source.cockpit_source_sha256
  ) {
    fail('clean-container receipt is not an installable artifact bound to this release')
  }
  const candidatePath = resolve(binaryPath || join(dirname(verified.manifestPath), artifact.name))
  verifySidecar(candidatePath)
  const info = regularFile(candidatePath, 'installable binary', { executable: true })
  if (info.size !== artifact.bytes || sha256File(candidatePath) !== artifact.sha256) {
    fail('installable binary differs from the clean-container receipt')
  }
  const image = cleanContainerPolicy(verified.manifest)
  if (receipt.container?.image?.id !== image.digest) {
    fail('installable binary receipt uses a different clean-container image')
  }
  return { artifact, binaryPath: candidatePath, containerReceipt: receipt, image, receiptPath }
}

export function dockerInstallProbeArgs({ image, sandboxRoot, uid, gid }) {
  const source = resolve(sandboxRoot)
  if (source.includes(',')) fail('lifecycle root cannot contain a comma for Docker --mount')
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
    '--pids-limit',
    '64',
    '--user',
    `${uid}:${gid}`,
    '--tmpfs',
    '/tmp:rw,nosuid,nodev,noexec,size=16777216',
    '--mount',
    `type=bind,src=${source},dst=/lifecycle`,
    '--workdir',
    '/lifecycle/home',
    '--env',
    'HOME=/lifecycle/home',
    '--env',
    'PATH=/lifecycle/prefix/bin:/usr/bin:/bin',
    '--env',
    'LANG=C.UTF-8',
    '--env',
    'LC_ALL=C.UTF-8',
    image.reference,
    '/lifecycle/prefix/bin/angel',
    '--build-info',
    '--json',
  ]
}

function runProbe(image, sandboxRoot, { expectFailure = false } = {}) {
  regularFile(DOCKER, 'Docker CLI', { executable: true })
  const dockerHome = join(sandboxRoot, 'docker-home')
  mkdirSync(dockerHome, { recursive: true, mode: 0o700 })
  const started = performance.now()
  const result = spawnSync(
    DOCKER,
    dockerInstallProbeArgs({
      image,
      sandboxRoot,
      uid: process.getuid(),
      gid: process.getgid(),
    }),
    {
      cwd: '/',
      env: { HOME: dockerHome, PATH: '/usr/bin:/bin', LANG: 'C.UTF-8', LC_ALL: 'C.UTF-8' },
      encoding: 'utf8',
      maxBuffer: MAX_COMMAND_OUTPUT,
      timeout: PROBE_TIMEOUT_MS,
      stdio: ['ignore', 'pipe', 'pipe'],
    },
  )
  const durationMs = Math.round(performance.now() - started)
  if (result.error) fail(`could not run installed cockpit probe: ${result.error.message}`)
  if (expectFailure) {
    if (result.status === 0) fail('deliberately incomplete replacement unexpectedly launched')
    return { failed_as_expected: true, exit_status: result.status, duration_ms: durationMs }
  }
  if (result.status !== 0) {
    fail(`installed cockpit probe exited ${result.status}: ${String(result.stderr).trim()}`)
  }
  let buildInfo
  try {
    buildInfo = JSON.parse(result.stdout)
  } catch {
    fail('installed cockpit probe did not emit valid build-info JSON')
  }
  return { duration_ms: durationMs, build_info: buildInfo }
}

export function runInstallVerification(
  manifestPath,
  { containerReceiptPath, binaryPath, receiptPath } = {},
) {
  if (process.platform !== 'linux' || process.arch !== 'x64') {
    fail('install verification currently supports only Linux x86_64')
  }
  const verified = verifyReleaseSet(manifestPath)
  const inputs = validateInstallInputs(verified, { containerReceiptPath, binaryPath })
  const scratch = mkdtempSync(join(tmpdir(), 'angel0-install-verification-'))
  try {
    const runnerContract = verifyRunnerContractSmoke(inputs.binaryPath)
    mkdirSync(join(scratch, 'home'), { mode: 0o700 })
    const metadata = {
      schema: PREFIX_INSTALL_SCHEMA,
      release: {
        semantic_manifest_sha256: verified.manifest.manifest_sha256,
        artifact_sha256: verified.manifest.artifact.sha256,
        commit: verified.manifest.source.commit,
        cockpit_source_sha256: verified.manifest.source.cockpit_source_sha256,
        resources_sha256: verified.manifest.entries_manifest_sha256,
      },
      binary: inputs.artifact,
    }
    const initial = installCandidate({
      sandboxRoot: scratch,
      prefix: join(scratch, 'prefix'),
      candidate: inputs.binaryPath,
      metadata,
      resources: verified,
    })
    if (initial.replaced) fail('empty-prefix install unexpectedly replaced an existing binary')
    const firstProbe = runProbe(inputs.image, scratch)
    if (
      firstProbe.build_info?.schema !== 'angel-build-info/v1' ||
      firstProbe.build_info?.cockpit_source_sha256 !==
        verified.manifest.source.cockpit_source_sha256 ||
      firstProbe.build_info?.resources?.sha256 !== verified.manifest.entries_manifest_sha256 ||
      firstProbe.build_info?.resources?.available !== true ||
      !Array.isArray(firstProbe.build_info?.capabilities) ||
      !REQUIRED_RUNNER_CAPABILITIES.every((capability) =>
        firstProbe.build_info.capabilities.includes(capability),
      )
    ) {
      fail('installed cockpit identity differs from the release source')
    }
    const statePath = join(scratch, 'home', '.angel', 'operator-state.json')
    mkdirSync(dirname(statePath), { recursive: true, mode: 0o700 })
    atomicJson(
      statePath,
      {
        schema: 'angel0-install-state-sentinel/v1',
        session: 'preserve-across-package-lifecycle',
      },
      0o600,
    )
    const stateBefore = sha256File(statePath)
    const replacement = installCandidate({
      sandboxRoot: scratch,
      prefix: join(scratch, 'prefix'),
      candidate: inputs.binaryPath,
      metadata,
      resources: verified,
    })
    if (!replacement.replaced) fail('package replacement did not exercise the transaction path')
    const replacementProbe = runProbe(inputs.image, scratch)
    const stateAfterReplacement = sha256File(statePath)
    if (stateAfterReplacement !== stateBefore)
      fail('operator state changed during package replacement')
    const injected = injectInterruptedReplacement(scratch)
    const failedProbe = runProbe(inputs.image, scratch, { expectFailure: true })
    const recovered = recoverInterruptedInstall(scratch)
    if (!recovered.recovered || recovered.restored_binary_sha256 !== inputs.artifact.sha256) {
      fail('interrupted replacement did not restore the source-bound binary')
    }
    const recoveredProbe = runProbe(inputs.image, scratch)
    const stateAfterRecovery = sha256File(statePath)
    if (stateAfterRecovery !== stateBefore) fail('operator state changed during install recovery')

    const receipt = {
      schema: INSTALL_VERIFICATION_SCHEMA,
      release_schema: verified.manifest.schema,
      release_scope: verified.manifest.scope,
      manifest_file_sha256: verified.manifestFileSha256,
      semantic_manifest_sha256: verified.manifest.manifest_sha256,
      artifact_sha256: verified.manifest.artifact.sha256,
      container_receipt_sha256: sha256File(inputs.receiptPath),
      installable_binary: inputs.artifact,
      runner_contract: runnerContract,
      layout: {
        prefix: '/isolated/prefix',
        binary: 'bin/angel',
        metadata: 'share/angel0/install.json',
        state_home: '/isolated/home',
      },
      isolation: {
        container_image: inputs.image.reference,
        network: 'none',
        producing_checkout_mounted: false,
        global_prefix_mutated: false,
        temporary_prefix_only: true,
        host_kernel_and_docker_daemon_reused: true,
        physical_clean_host_proven: false,
      },
      install: {
        empty_prefix: true,
        binary_sha256: initial.binary_sha256,
        probe: firstProbe,
      },
      replacement: {
        transaction_exercised: true,
        same_artifact_reinstall: true,
        cross_version_compatibility_proven: false,
        state_sha256_before: stateBefore,
        state_sha256_after: stateAfterReplacement,
        probe: replacementProbe,
      },
      recovery: {
        injected_fault: 'interrupted binary replacement after rollback journal creation',
        faulted_binary_sha256: injected.faulted_binary_sha256,
        pre_recovery_probe: failedProbe,
        restored_binary_sha256: recovered.restored_binary_sha256,
        state_sha256_after: stateAfterRecovery,
        probe: recoveredProbe,
      },
      negative_claims: {
        package_manager_integration_proven: false,
        cross_version_state_migration_proven: false,
        physical_clean_host_proven: false,
        external_signature_or_attestation_proven: false,
      },
    }
    const output = resolve(
      receiptPath ||
        verified.manifestPath.replace(/\.manifest\.json$/u, '.install-verification.json'),
    )
    if (output === verified.manifestPath) fail('install receipt path must differ from manifest')
    atomicJson(output, receipt)
    const sidecar = `${output}.sha256`
    const temporarySidecar = `${sidecar}.tmp-${process.pid}`
    writeFileSync(temporarySidecar, `${sha256File(output)}  ${basename(output)}\n`, { mode: 0o644 })
    renameSync(temporarySidecar, sidecar)
    return { receipt, receiptPath: output }
  } finally {
    rmSync(scratch, { recursive: true, force: true })
  }
}

function parseArgs(argv) {
  if (argv.length === 0) {
    fail(
      'usage: verify-release-install.mjs MANIFEST [--container-receipt PATH] [--binary PATH] [--receipt PATH]',
    )
  }
  const options = { manifestPath: argv[0] }
  const flags = new Map([
    ['--container-receipt', 'containerReceiptPath'],
    ['--binary', 'binaryPath'],
    ['--receipt', 'receiptPath'],
  ])
  for (let index = 1; index < argv.length; index += 1) {
    const key = flags.get(argv[index])
    if (!key || !argv[index + 1]) fail(`unknown or incomplete argument: ${argv[index]}`)
    options[key] = argv[index + 1]
    index += 1
  }
  return options
}

const invokedPath = process.argv[1] ? pathToFileURL(resolve(process.argv[1])).href : ''
if (import.meta.url === invokedPath) {
  try {
    const args = parseArgs(process.argv.slice(2))
    const result = runInstallVerification(args.manifestPath, args)
    process.stdout.write(
      canonicalJson({
        schema: result.receipt.schema,
        receipt: result.receiptPath,
        artifact_sha256: result.receipt.artifact_sha256,
        installable_binary: result.receipt.installable_binary,
        install: result.receipt.install,
        replacement: result.receipt.replacement,
        recovery: result.receipt.recovery,
      }),
    )
  } catch (error) {
    process.stderr.write(`install verification failed: ${error.message}\n`)
    process.exitCode = 1
  }
}
