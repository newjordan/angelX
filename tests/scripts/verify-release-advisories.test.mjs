import assert from 'node:assert/strict'
import { mkdtempSync, rmSync } from 'node:fs'
import { tmpdir } from 'node:os'
import { join } from 'node:path'
import test from 'node:test'

import {
  advisoryPolicy,
  dockerScanArgs,
  evaluateScan,
  validateScannerInspection,
  validateScannerVersion,
} from '../../scripts/release/verify-release-advisories.mjs'

const SCANNER = {
  name: 'osv-scanner',
  version: '2.3.8',
  commit: '408fcd6f8707999a29e7ba45e15809764cf24f67',
  reference:
    'ghcr.io/google/osv-scanner@sha256:64e86bec6df2466feea5137fc7c78fb3b7c21ec077f014d7130f64810e50676b',
  digest: 'sha256:64e86bec6df2466feea5137fc7c78fb3b7c21ec077f014d7130f64810e50676b',
  operating_system: 'linux',
  architecture: 'amd64',
  trust: 'official image pinned by digest; local provenance verification not yet performed',
}

function fixture() {
  const exception = {
    lockfile: 'cockpit/Cargo.lock',
    package: 'lru',
    version: '0.12.5',
    ids: ['GHSA-rhfx-m35p-ff5j', 'RUSTSEC-2026-0002'],
    kind: 'unsound',
    expires: '2026-08-18',
    reason: 'The affected iterator is not called while the coordinated UI stack is migrated.',
  }
  const policy = {
    lockfiles: [
      {
        path: 'cockpit/Cargo.lock',
        ecosystem: 'crates.io',
        sha256: 'a'.repeat(64),
        package_count: 2,
      },
      {
        path: 'cockpit/portal-renderer/Cargo.lock',
        ecosystem: 'crates.io',
        sha256: 'b'.repeat(64),
        package_count: 1,
      },
      {
        path: 'package-lock.json',
        ecosystem: 'npm',
        sha256: 'c'.repeat(64),
        package_count: 1,
      },
    ],
    python: { status: 'not-applicable', reason: 'no Python dependency manifest' },
    exceptions: [exception],
  }
  const manifest = {
    dependencies: {
      rust: {
        locked_package_count: 2,
        advisory_scanners: [SCANNER],
        advisory_policy: policy,
      },
      node: { registry_package_count: 1 },
    },
  }
  const rustsec = {
    id: 'RUSTSEC-2026-0002',
    published: '2026-01-01T00:00:00Z',
    modified: '2026-01-02T00:00:00Z',
    summary: 'IterMut violates Stacked Borrows',
    affected: [{ database_specific: { informational: 'unsound' } }],
  }
  const ghsa = {
    id: 'GHSA-rhfx-m35p-ff5j',
    published: '2026-01-01T00:00:00Z',
    modified: '2026-01-02T00:00:00Z',
    summary: 'IterMut violates Stacked Borrows',
    affected: [{ database_specific: { informational: null } }],
  }
  const scan = {
    results: [
      {
        source: { path: '/scan/cockpit/Cargo.lock', type: 'lockfile' },
        packages: [
          {
            package: { name: 'lru', version: '0.12.5', ecosystem: 'crates.io' },
            groups: [
              {
                ids: ['RUSTSEC-2026-0002', 'GHSA-rhfx-m35p-ff5j'],
                aliases: ['GHSA-rhfx-m35p-ff5j', 'RUSTSEC-2026-0002'],
                max_severity: '2.7',
              },
            ],
            vulnerabilities: [rustsec, ghsa],
          },
          {
            package: { name: 'safe-rust', version: '1.0.0', ecosystem: 'crates.io' },
            groups: [],
            vulnerabilities: [],
          },
        ],
      },
      {
        source: { path: '/scan/cockpit/portal-renderer/Cargo.lock', type: 'lockfile' },
        packages: [
          {
            package: { name: 'safe-renderer', version: '1.0.0', ecosystem: 'crates.io' },
            groups: [],
            vulnerabilities: [],
          },
        ],
      },
      {
        source: { path: '/scan/package-lock.json', type: 'lockfile' },
        packages: [
          {
            package: { name: 'safe-node', version: '1.0.0', ecosystem: 'npm' },
            groups: [],
            vulnerabilities: [],
          },
        ],
      },
    ],
  }
  return { manifest, policy, scan }
}

test('advisory policy binds exact scanner and every release lockfile', () => {
  const { manifest } = fixture()
  const result = advisoryPolicy(manifest)
  assert.equal(result.scanner.digest, SCANNER.digest)
  assert.deepEqual(
    result.policy.lockfiles.map((row) => row.path),
    ['cockpit/Cargo.lock', 'cockpit/portal-renderer/Cargo.lock', 'package-lock.json'],
  )
})

test('scanner inspection and runtime identity must match policy', () => {
  const inspection = {
    Id: SCANNER.digest,
    Os: 'linux',
    Architecture: 'amd64',
    RepoDigests: [SCANNER.reference],
  }
  assert.equal(validateScannerInspection(SCANNER, inspection).digest, SCANNER.digest)
  assert.throws(
    () => validateScannerInspection(SCANNER, { ...inspection, Architecture: 'arm64' }),
    /differs/u,
  )
  assert.deepEqual(
    validateScannerVersion(
      SCANNER,
      `osv-scanner version: 2.3.8\nosv-scalibr version: 0.4.5\ncommit: ${SCANNER.commit}\nbuilt at: 2026-05-08T04:54:35Z\n`,
    ),
    [
      'osv-scanner version: 2.3.8',
      'osv-scalibr version: 0.4.5',
      `commit: ${SCANNER.commit}`,
      'built at: 2026-05-08T04:54:35Z',
    ],
  )
})

test('scanner container exposes only exact locks and a bounded live query', (t) => {
  const root = mkdtempSync(join(tmpdir(), 'angelX-advisory-args-'))
  t.after(() => rmSync(root, { recursive: true, force: true }))
  const { policy } = fixture()
  const args = dockerScanArgs(SCANNER, root, policy)
  assert.ok(args.includes('never'))
  assert.ok(args.includes('bridge'))
  assert.ok(args.includes('ALL'))
  assert.ok(args.includes('no-new-privileges'))
  assert.ok(args.includes('--lockfile=/scan/cockpit/Cargo.lock'))
  assert.ok(args.includes('--lockfile=/scan/cockpit/portal-renderer/Cargo.lock'))
  assert.ok(args.includes('--lockfile=/scan/package-lock.json'))
  assert.ok(args.some((value) => value.includes(`src=${root},dst=/scan,readonly`)))
  assert.equal(
    args.some((value) => value.includes(process.cwd())),
    false,
  )
})

test('complete scan accepts exact unexpired exception and hashes returned records', () => {
  const { policy, scan } = fixture()
  const result = evaluateScan(scan, policy, new Date('2026-07-18T12:00:00Z'))
  assert.equal(result.coverage[0].packages, 2)
  assert.equal(result.coverage[1].packages, 1)
  assert.equal(result.coverage[2].packages, 1)
  assert.equal(result.findings.length, 1)
  assert.equal(result.findings[0].kind, 'unsound')
  assert.equal(result.findings[0].exception.expires, '2026-08-18')
  assert.equal(result.advisory_records.length, 2)
  assert.match(result.advisory_record_set_sha256, /^[0-9a-f]{64}$/u)
  assert.equal(result.advisory_clean, false)
  assert.equal(result.policy_passed, true)
})

test('coverage mismatch, unexpected findings, stale exceptions, and expiry fail closed', () => {
  const { policy, scan } = fixture()

  const incomplete = structuredClone(scan)
  incomplete.results[0].packages.pop()
  assert.throws(
    () => evaluateScan(incomplete, policy, new Date('2026-07-18T12:00:00Z')),
    /coverage differs/u,
  )

  assert.throws(() => evaluateScan(scan, { ...policy, exceptions: [] }), /unaccepted advisory/u)

  const clean = structuredClone(scan)
  clean.results[0].packages[0].groups = []
  clean.results[0].packages[0].vulnerabilities = []
  assert.throws(
    () => evaluateScan(clean, policy, new Date('2026-07-18T12:00:00Z')),
    /stale exceptions/u,
  )

  assert.throws(() => evaluateScan(scan, policy, new Date('2026-08-19T00:00:00Z')), /expired/u)
})
