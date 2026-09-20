#!/usr/bin/env node

import { spawnSync } from 'node:child_process'
import {
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
import { basename, dirname, join, resolve } from 'node:path'
import { pathToFileURL } from 'node:url'

import { ReleaseGateError, canonicalJson, sha256Bytes, sha256File } from './release-evidence.mjs'
import { extractVerifiedRows, verifyReleaseSet } from './verify-release-evidence.mjs'

export const ADVISORY_VERIFICATION_SCHEMA = 'angel0-release-advisory-verification/v1'
const DOCKER = '/usr/bin/docker'
const MAX_COMMAND_OUTPUT = 64 * 1024 * 1024
const SCAN_TIMEOUT_MS = 5 * 60 * 1000

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

function run(command, args, { env, timeout = SCAN_TIMEOUT_MS, accepted = [0] } = {}) {
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
  if (!accepted.includes(result.status)) {
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

function safeMountSource(path) {
  const canonical = realpathSync(path)
  if (canonical.includes(',')) fail('advisory scan path cannot contain a comma')
  return canonical
}

function bindMount(source, target, readonly = false) {
  return `type=bind,src=${safeMountSource(source)},dst=${target}${readonly ? ',readonly' : ''}`
}

export function advisoryPolicy(manifest) {
  const scanners = manifest.dependencies?.rust?.advisory_scanners
  const policy = manifest.dependencies?.rust?.advisory_policy
  if (!Array.isArray(scanners) || scanners.length !== 1 || !policy) {
    fail('release manifest must bind one advisory scanner and policy')
  }
  const scanner = scanners[0]
  if (
    scanner.name !== 'osv-scanner' ||
    scanner.version !== '2.3.8' ||
    !/^[0-9a-f]{40}$/u.test(String(scanner.commit || '')) ||
    !/^sha256:[0-9a-f]{64}$/u.test(String(scanner.digest || '')) ||
    scanner.reference !== `ghcr.io/google/osv-scanner@${scanner.digest}` ||
    scanner.operating_system !== 'linux' ||
    scanner.architecture !== 'amd64'
  ) {
    fail('release advisory scanner identity is incomplete')
  }
  if (
    !Array.isArray(policy.lockfiles) ||
    policy.lockfiles.length !== 3 ||
    !Array.isArray(policy.exceptions) ||
    policy.python?.status !== 'not-applicable'
  ) {
    fail('release advisory scope or exception policy is incomplete')
  }
  const expected = new Map([
    ['cockpit/Cargo.lock', 'crates.io'],
    ['cockpit/portal-renderer/Cargo.lock', 'crates.io'],
    ['package-lock.json', 'npm'],
  ])
  for (const lockfile of policy.lockfiles) {
    if (
      expected.get(lockfile.path) !== lockfile.ecosystem ||
      !/^[0-9a-f]{64}$/u.test(String(lockfile.sha256 || '')) ||
      !Number.isSafeInteger(lockfile.package_count) ||
      lockfile.package_count <= 0
    ) {
      fail(`invalid advisory lockfile policy: ${lockfile.path || 'unknown'}`)
    }
    expected.delete(lockfile.path)
  }
  if (expected.size > 0) fail('advisory policy omitted a release lockfile')
  return { scanner, policy }
}

export function validateScannerInspection(scanner, inspection) {
  if (!inspection || typeof inspection !== 'object') fail('scanner image inspection is absent')
  if (
    inspection.Id !== scanner.digest ||
    inspection.Os !== scanner.operating_system ||
    inspection.Architecture !== scanner.architecture
  ) {
    fail('local advisory scanner identity or platform differs from release policy')
  }
  const digests = Array.isArray(inspection.RepoDigests) ? inspection.RepoDigests : []
  if (!digests.includes(scanner.reference)) fail('local advisory scanner lacks the policy digest')
  return {
    name: scanner.name,
    version: scanner.version,
    commit: scanner.commit,
    reference: scanner.reference,
    digest: inspection.Id,
    operating_system: inspection.Os,
    architecture: inspection.Architecture,
    trust: scanner.trust,
  }
}

export function validateScannerVersion(scanner, output) {
  const version = String(output).match(/^osv-scanner version:\s*(\S+)$/mu)?.[1]
  const commit = String(output).match(/^commit:\s*([0-9a-f]{40})$/mu)?.[1]
  if (version !== scanner.version || commit !== scanner.commit) {
    fail('advisory scanner runtime version differs from release policy')
  }
  return String(output).trim().split('\n')
}

export function dockerScanArgs(scanner, scanRoot, policy) {
  const args = [
    'run',
    '--rm',
    '--pull',
    'never',
    '--network',
    'bridge',
    '--read-only',
    '--cap-drop',
    'ALL',
    '--security-opt',
    'no-new-privileges',
    '--pids-limit',
    '128',
    '--tmpfs',
    '/tmp:rw,nosuid,nodev,noexec,size=67108864',
    '--workdir',
    '/tmp',
    '--mount',
    bindMount(scanRoot, '/scan', true),
    scanner.reference,
    'scan',
    'source',
    '--verbosity=error',
    '--all-packages',
    '--format=json',
  ]
  for (const lockfile of policy.lockfiles) {
    args.push(`--lockfile=/scan/${lockfile.path}`)
  }
  return args
}

function findingKind(pkg, group) {
  const groupIds = new Set([...(group.ids || []), ...(group.aliases || [])])
  const kinds = new Set()
  for (const vulnerability of pkg.vulnerabilities || []) {
    if (!groupIds.has(vulnerability.id)) continue
    for (const affected of vulnerability.affected || []) {
      const informational = affected.database_specific?.informational
      if (informational) kinds.add(informational)
    }
  }
  if (kinds.size === 0) return 'vulnerability'
  if (kinds.size === 1) return [...kinds][0]
  fail(`advisory group mixes informational kinds for ${pkg.package?.name || 'unknown'}`)
}

function advisoryRecords(scan) {
  const records = new Map()
  for (const result of scan.results) {
    for (const pkg of result.packages || []) {
      for (const vulnerability of pkg.vulnerabilities || []) {
        const record = {
          id: vulnerability.id,
          modified: vulnerability.modified || null,
          published: vulnerability.published || null,
          sha256: sha256Bytes(Buffer.from(canonicalJson(vulnerability))),
        }
        const prior = records.get(record.id)
        if (prior && canonicalJson(prior) !== canonicalJson(record)) {
          fail(`OSV returned conflicting records for ${record.id}`)
        }
        records.set(record.id, record)
      }
    }
  }
  return [...records.values()].sort((left, right) => left.id.localeCompare(right.id))
}

function normalizedFinding(lockfile, pkg, group) {
  const ids = [...new Set([...(group.ids || []), ...(group.aliases || [])])].sort()
  if (!pkg.package?.name || !pkg.package?.version || ids.length === 0) {
    fail(`OSV returned an incomplete finding for ${lockfile}`)
  }
  const summaries = [
    ...new Set(
      (pkg.vulnerabilities || [])
        .filter((vulnerability) => ids.includes(vulnerability.id))
        .map((vulnerability) => vulnerability.summary)
        .filter(Boolean),
    ),
  ].sort()
  return {
    lockfile,
    ecosystem: pkg.package.ecosystem,
    package: pkg.package.name,
    version: pkg.package.version,
    ids,
    kind: findingKind(pkg, group),
    max_severity: group.max_severity || null,
    summaries,
  }
}

function findingKey(value) {
  return `${value.lockfile}\0${value.package}\0${value.version}\0${value.ids.join(',')}`
}

export function evaluateScan(scan, policy, now = new Date()) {
  if (!scan || !Array.isArray(scan.results) || scan.results.length !== policy.lockfiles.length) {
    fail('OSV scan did not return every policy lockfile')
  }
  const expectedPolicies = new Map(policy.lockfiles.map((row) => [row.path, row]))
  const findings = []
  const coverage = []
  for (const result of scan.results) {
    const source = String(result.source?.path || '')
    if (!source.startsWith('/scan/')) fail('OSV result source is outside the sealed scan root')
    const lockfile = source.slice('/scan/'.length)
    const lockPolicy = expectedPolicies.get(lockfile)
    if (!lockPolicy || result.source?.type !== 'lockfile') {
      fail(`OSV returned an unexpected result source: ${source || 'unknown'}`)
    }
    expectedPolicies.delete(lockfile)
    const packages = Array.isArray(result.packages) ? result.packages : []
    const expectedCount = lockPolicy.package_count
    if (!Number.isSafeInteger(expectedCount) || packages.length !== expectedCount) {
      fail(`OSV package coverage differs for ${lockfile}: ${packages.length}/${expectedCount}`)
    }
    const packageKeys = new Set()
    for (const pkg of packages) {
      if (!pkg.package?.name || !pkg.package?.version) fail(`OSV returned an invalid package row`)
      const key = `${pkg.package.name}\0${pkg.package.version}`
      if (packageKeys.has(key)) fail(`OSV returned a duplicate package row for ${lockfile}`)
      packageKeys.add(key)
      for (const group of pkg.groups || []) findings.push(normalizedFinding(lockfile, pkg, group))
    }
    coverage.push({
      lockfile,
      ecosystem: lockPolicy.ecosystem,
      lockfile_sha256: lockPolicy.sha256,
      packages: packages.length,
      finding_groups: findings.filter((finding) => finding.lockfile === lockfile).length,
    })
  }
  if (expectedPolicies.size > 0) fail('OSV omitted a policy lockfile')
  coverage.sort((left, right) => left.lockfile.localeCompare(right.lockfile))
  findings.sort((left, right) => findingKey(left).localeCompare(findingKey(right)))

  const exceptionMap = new Map()
  for (const exception of policy.exceptions) {
    const key = findingKey(exception)
    if (exceptionMap.has(key)) fail('advisory policy contains a duplicate exception')
    exceptionMap.set(key, exception)
  }
  const today = now.toISOString().slice(0, 10)
  const accepted = []
  for (const finding of findings) {
    const key = findingKey(finding)
    const exception = exceptionMap.get(key)
    if (!exception)
      fail(`unaccepted advisory finding: ${finding.package} ${finding.ids.join(', ')}`)
    if (exception.kind !== finding.kind) {
      fail(`advisory exception kind differs for ${finding.package}`)
    }
    if (!/^\d{4}-\d{2}-\d{2}$/u.test(exception.expires) || exception.expires < today) {
      fail(`advisory exception expired for ${finding.package}: ${exception.expires || 'invalid'}`)
    }
    exceptionMap.delete(key)
    accepted.push({
      ...finding,
      exception: { expires: exception.expires, reason: exception.reason },
    })
  }
  if (exceptionMap.size > 0) {
    fail(
      `advisory policy contains stale exceptions: ${[...exceptionMap.values()]
        .map((row) => row.package)
        .join(', ')}`,
    )
  }
  const records = advisoryRecords(scan)
  return {
    coverage,
    findings: accepted,
    advisory_records: records,
    advisory_record_set_sha256: sha256Bytes(Buffer.from(canonicalJson(records))),
    advisory_clean: accepted.length === 0,
    policy_passed: true,
  }
}

function writeEvidence(path, content) {
  const output = resolve(path)
  mkdirSync(dirname(output), { recursive: true, mode: 0o755 })
  const temporary = `${output}.tmp-${process.pid}`
  writeFileSync(temporary, content, { mode: 0o644 })
  renameSync(temporary, output)
  const sidecar = `${output}.sha256`
  const temporarySidecar = `${sidecar}.tmp-${process.pid}`
  writeFileSync(temporarySidecar, `${sha256File(output)}  ${basename(output)}\n`, { mode: 0o644 })
  renameSync(temporarySidecar, sidecar)
  return output
}

function scannerVersionArgs(scanner) {
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
    scanner.reference,
    '--version',
  ]
}

export function runAdvisoryVerification(
  manifestPath,
  { receiptPath, rawPath, now = new Date() } = {},
) {
  if (process.platform !== 'linux' || process.arch !== 'x64') {
    fail('release advisory verification currently supports only Linux x86_64')
  }
  const docker = executableFile(DOCKER, 'Docker CLI')
  const verified = verifyReleaseSet(manifestPath)
  const { scanner, policy } = advisoryPolicy(verified.manifest)
  const scratch = mkdtempSync(join(tmpdir(), 'angel0-release-advisory-'))
  try {
    const sourceRoot = extractVerifiedRows(verified.rows, scratch)
    const scanRoot = join(scratch, 'scan')
    // The official scanner runs as a non-host UID. Directory traversal must be
    // world-readable even though the bind remains read-only and contains only
    // the exact public release lockfiles.
    mkdirSync(join(scanRoot, 'cockpit'), { recursive: true, mode: 0o755 })
    for (const lockfile of policy.lockfiles) {
      const source = join(sourceRoot, lockfile.path)
      if (sha256File(source) !== lockfile.sha256) {
        fail(`extracted advisory lockfile differs from policy: ${lockfile.path}`)
      }
      const destination = join(scanRoot, lockfile.path)
      mkdirSync(dirname(destination), { recursive: true, mode: 0o755 })
      copyFileSync(source, destination)
    }

    const hostEnv = dockerEnvironment(join(scratch, 'docker-home'))
    mkdirSync(hostEnv.HOME, { mode: 0o700 })
    const inspected = run(docker, ['image', 'inspect', scanner.reference], {
      env: hostEnv,
      timeout: 30_000,
    })
    let inspection
    try {
      const parsed = JSON.parse(inspected.stdout)
      inspection = Array.isArray(parsed) && parsed.length === 1 ? parsed[0] : null
    } catch {
      fail('Docker scanner inspect did not return valid JSON')
    }
    const scannerIdentity = validateScannerInspection(scanner, inspection)
    const version = run(docker, scannerVersionArgs(scanner), { env: hostEnv, timeout: 30_000 })
    scannerIdentity.runtime = validateScannerVersion(scanner, version.stdout)
    const scanned = run(docker, dockerScanArgs(scanner, scanRoot, policy), {
      env: hostEnv,
      accepted: [0, 1],
    })
    let rawScan
    try {
      rawScan = JSON.parse(scanned.stdout)
    } catch {
      fail('OSV scanner did not return valid JSON')
    }
    const evaluation = evaluateScan(rawScan, policy, now)
    const expectedExit = evaluation.findings.length > 0 ? 1 : 0
    if (scanned.status !== expectedExit) {
      fail(`OSV scanner exit ${scanned.status} differs from finding state ${expectedExit}`)
    }

    const rawOutput = resolve(
      rawPath || verified.manifestPath.replace(/\.manifest\.json$/u, '.advisory-osv.json'),
    )
    if (rawOutput === verified.manifestPath)
      fail('advisory raw output path must differ from manifest')
    writeEvidence(rawOutput, canonicalJson(rawScan))
    const raw = {
      file: basename(rawOutput),
      bytes: statSync(rawOutput).size,
      sha256: sha256File(rawOutput),
    }
    const receipt = {
      schema: ADVISORY_VERIFICATION_SCHEMA,
      release_schema: verified.manifest.schema,
      release_scope: verified.manifest.scope,
      manifest_file_sha256: verified.manifestFileSha256,
      semantic_manifest_sha256: verified.manifest.manifest_sha256,
      artifact_sha256: verified.manifest.artifact.sha256,
      scanned_at: now.toISOString(),
      scanner: scannerIdentity,
      query: {
        command:
          'osv-scanner scan source --all-packages --format=json --lockfile=<exact release locks>',
        network: 'Docker bridge (OSV.dev query only)',
        duration_ms: scanned.durationMs,
        credentials_mounted: false,
        source_checkout_mounted: false,
        exact_lockfiles_only: true,
        database_global_snapshot_identity_available: false,
        exact_returned_advisory_records_hashed: true,
      },
      raw_osv: raw,
      coverage: evaluation.coverage,
      python: policy.python,
      findings: evaluation.findings,
      finding_group_count: evaluation.findings.length,
      exception_count: evaluation.findings.length,
      advisory_clean: evaluation.advisory_clean,
      policy_passed: evaluation.policy_passed,
      advisory_records: evaluation.advisory_records,
      advisory_record_set_sha256: evaluation.advisory_record_set_sha256,
    }
    const output = resolve(
      receiptPath ||
        verified.manifestPath.replace(/\.manifest\.json$/u, '.advisory-verification.json'),
    )
    if (output === verified.manifestPath || output === rawOutput) {
      fail('advisory receipt path must differ from other evidence files')
    }
    writeEvidence(output, canonicalJson(receipt))
    return { receipt, receiptPath: output, rawPath: rawOutput }
  } finally {
    rmSync(scratch, { recursive: true, force: true })
  }
}

function parseArgs(argv) {
  if (argv.length === 0) {
    fail('usage: verify-release-advisories.mjs MANIFEST [--receipt PATH] [--raw PATH]')
  }
  const options = { manifestPath: argv[0] }
  for (let index = 1; index < argv.length; index += 1) {
    if (!['--receipt', '--raw'].includes(argv[index]) || !argv[index + 1]) {
      fail(`unknown or incomplete argument: ${argv[index]}`)
    }
    if (argv[index] === '--receipt') options.receiptPath = argv[index + 1]
    else options.rawPath = argv[index + 1]
    index += 1
  }
  return options
}

const invokedPath = process.argv[1] ? pathToFileURL(resolve(process.argv[1])).href : ''
if (import.meta.url === invokedPath) {
  try {
    const args = parseArgs(process.argv.slice(2))
    const result = runAdvisoryVerification(args.manifestPath, args)
    process.stdout.write(
      canonicalJson({
        schema: result.receipt.schema,
        receipt: result.receiptPath,
        raw_osv: result.rawPath,
        artifact_sha256: result.receipt.artifact_sha256,
        coverage: result.receipt.coverage,
        finding_group_count: result.receipt.finding_group_count,
        exception_count: result.receipt.exception_count,
        advisory_clean: result.receipt.advisory_clean,
        policy_passed: result.receipt.policy_passed,
      }),
    )
  } catch (error) {
    process.stderr.write(`release advisory verification failed: ${error.message}\n`)
    process.exitCode = 1
  }
}
