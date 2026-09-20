import assert from 'node:assert/strict'
import { chmodSync, existsSync, mkdtempSync, readFileSync, rmSync, writeFileSync } from 'node:fs'
import { join } from 'node:path'
import test from 'node:test'

import { sha256File } from '../../scripts/release-evidence.mjs'
import {
  PREFIX_INSTALL_SCHEMA,
  assertManagedPrefix,
  dockerInstallProbeArgs,
  injectInterruptedReplacement,
  installCandidate,
  prefixLayout,
  recoverInterruptedInstall,
} from '../../scripts/verify-release-install.mjs'

function fixture(t) {
  const root = mkdtempSync(join(process.cwd(), '.angel0-install-test-'))
  t.after(() => rmSync(root, { recursive: true, force: true }))
  const candidate = join(root, 'candidate')
  writeFileSync(candidate, '#!/bin/sh\necho angel\n', { mode: 0o755 })
  chmodSync(candidate, 0o755)
  const metadata = {
    schema: PREFIX_INSTALL_SCHEMA,
    release: { semantic_manifest_sha256: 'a'.repeat(64) },
    binary: {
      name: 'angel-fixture',
      media_type: 'application/vnd.angel0.cockpit-executable',
      platform: 'linux-x86_64',
      mode: '0755',
      bytes: readFileSync(candidate).length,
      sha256: sha256File(candidate),
    },
  }
  return { root, candidate, metadata }
}

test('managed install prefixes cannot escape or replace the isolated root', (t) => {
  const { root } = fixture(t)
  assert.equal(assertManagedPrefix(root, join(root, 'prefix')), join(root, 'prefix'))
  assert.throws(() => assertManagedPrefix(root, root), /strict descendant/u)
  assert.throws(() => assertManagedPrefix(root, join(root, '..', 'outside')), /strict descendant/u)
})

test('empty-prefix install and same-artifact replacement are exact and journal-free', (t) => {
  const { root, candidate, metadata } = fixture(t)
  const first = installCandidate({ sandboxRoot: root, candidate, metadata })
  assert.equal(first.replaced, false)
  assert.equal(first.binary_sha256, metadata.binary.sha256)
  const second = installCandidate({ sandboxRoot: root, candidate, metadata })
  assert.equal(second.replaced, true)
  assert.equal(second.binary_sha256, metadata.binary.sha256)
  const layout = prefixLayout(root)
  assert.equal(existsSync(layout.journal), false)
  assert.equal(existsSync(layout.rollbackDirectory), false)
})

test('checksum mismatch fails before mutating an installed prefix', (t) => {
  const { root, candidate, metadata } = fixture(t)
  installCandidate({ sandboxRoot: root, candidate, metadata })
  const layout = prefixLayout(root)
  const before = sha256File(layout.binary)
  const invalid = { ...metadata, binary: { ...metadata.binary, sha256: 'b'.repeat(64) } }
  assert.throws(
    () => installCandidate({ sandboxRoot: root, candidate, metadata: invalid }),
    /differs from its source-bound install metadata/u,
  )
  assert.equal(sha256File(layout.binary), before)
  assert.equal(existsSync(layout.journal), false)
})

test('installed resources remain bound to the binary release and survive rollback', (t) => {
  const { root, candidate, metadata } = fixture(t)
  const identity = 'c'.repeat(64)
  const resources = { manifest: { entries_manifest_sha256: identity }, rows: [
    { path: 'scripts/worker.mjs', mode: 0o644, content: Buffer.from('// exact worker\n') },
    { path: 'cockpit/assets/fixture.txt', mode: 0o644, content: Buffer.from('exact asset\n') },
  ] }
  metadata.release.resources_sha256 = identity
  const installed = installCandidate({ sandboxRoot: root, candidate, metadata, resources })
  const worker = join(installed.layout.shareDirectory, 'bundles', identity, 'scripts/worker.mjs')
  assert.equal(readFileSync(worker, 'utf8'), '// exact worker\n')
  injectInterruptedReplacement(root)
  assert.equal(recoverInterruptedInstall(root).recovered, true)
  assert.equal(readFileSync(worker, 'utf8'), '// exact worker\n')
  assert.throws(() => installCandidate({ sandboxRoot: root, candidate, metadata }), /requires its source-bound resource bundle/u)
  writeFileSync(worker, '// corrupted\n')
  assert.throws(() => installCandidate({ sandboxRoot: root, candidate, metadata, resources }), /resource differs/u)
})

test('interrupted replacement restores the verified binary and refuses corrupt rollback', (t) => {
  const { root, candidate, metadata } = fixture(t)
  installCandidate({ sandboxRoot: root, candidate, metadata })
  const layout = prefixLayout(root)
  injectInterruptedReplacement(root)
  assert.notEqual(sha256File(layout.binary), metadata.binary.sha256)
  const recovered = recoverInterruptedInstall(root)
  assert.equal(recovered.recovered, true)
  assert.equal(sha256File(layout.binary), metadata.binary.sha256)

  injectInterruptedReplacement(root)
  writeFileSync(layout.rollbackBinary, 'corrupt rollback\n', { mode: 0o755 })
  assert.throws(() => recoverInterruptedInstall(root), /differs from the install journal/u)
  assert.equal(existsSync(layout.journal), true)
})

test('installed probe is networkless, checkout-free, and limited to lifecycle state', (t) => {
  const { root } = fixture(t)
  const sandboxRoot = join(root, 'source-workspace')
  const digest = `sha256:${'c'.repeat(64)}`
  const args = dockerInstallProbeArgs({
    image: { reference: digest },
    sandboxRoot,
    uid: 123,
    gid: 456,
  })
  const option = (name) => args[args.indexOf(name) + 1]
  assert.equal(option('--network'), 'none')
  assert.equal(option('--pull'), 'never')
  assert.equal(option('--user'), '123:456')
  const mounts = args.flatMap((row, index) => (row === '--mount' ? [args[index + 1]] : []))
  assert.deepEqual(mounts, [`type=bind,src=${sandboxRoot},dst=/lifecycle`])
  assert.equal(option('--workdir'), '/lifecycle/home')
  assert.ok(!args.includes('--volume') && !args.includes('-v'))
  assert.deepEqual(args.slice(-3), ['/lifecycle/prefix/bin/angel', '--build-info', '--json'])
})


import { runRollbackVerification } from '../../scripts/verify-release-rollback.mjs'

function rollbackFixture(t) {
  const { root } = fixture(t)
  const previous = join(root, 'N'), next = join(root, 'N1')
  for (const [path, digest] of [[previous, 'a'], [next, 'b']]) {
    const info = { schema: 'angel-build-info/v1', cockpit_source_sha256: digest.repeat(64) }
    writeFileSync(path, `#!/bin/sh\nprintf '%s\\n' '${JSON.stringify(info)}'\n`, { mode: 0o755 })
  }
  return { previous, next, previousSha256: sha256File(previous), nextSha256: sha256File(next),
    receiptPath: join(root, 'receipt.json'), worktree: root }
}

test('host prefix installs N, upgrades to distinct N+1, rolls back to N and removes prefix', (t) => {
  const options = rollbackFixture(t)
  const receipt = runRollbackVerification(options)
  assert.equal(receipt.status, 'pass', receipt.error)
  const steps = receipt.steps.filter((s) => s.phase === 'verified')
  assert.deepEqual(steps.map((s) => s.sha256), [options.previousSha256, options.nextSha256, options.previousSha256])
  assert.deepEqual(steps.map((s) => s.replaced), [false, true, true])
  assert.equal(receipt.cleanup.removed, true)
  assert.deepEqual(JSON.parse(readFileSync(options.receiptPath)), receipt)
})

test('rollback rejects a candidate not matching its reviewed digest and writes failure receipt', (t) => {
  const options = rollbackFixture(t)
  options.nextSha256 = 'c'.repeat(64)
  const receipt = runRollbackVerification(options)
  assert.equal(receipt.status, 'fail')
  assert.match(receipt.error, /reviewed sha256/)
  assert.equal(receipt.cleanup.prefix_created, false)
})

test('rollback records probe failure and removes temporary prefix', (t) => {
  const options = rollbackFixture(t)
  writeFileSync(options.next, '#!/bin/sh\nexit 7\n')
  options.nextSha256 = sha256File(options.next)
  const receipt = runRollbackVerification(options)
  assert.equal(receipt.status, 'fail')
  assert.match(receipt.error, /exit 7/)
  assert.equal(receipt.cleanup.removed, true)
})
