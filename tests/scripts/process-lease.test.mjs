import { test } from 'node:test'
import assert from 'node:assert/strict'
import fs from 'node:fs'
import os from 'node:os'
import { spawn } from 'node:child_process'
import { join } from 'node:path'
import { pathToFileURL } from 'node:url'

import {
  PROCESS_LEASE_SCHEMA,
  acquireProcessLease,
  classifyProcessLeaseOwner,
  processStartToken,
  releaseProcessLease,
} from '../../lib/control/process-lease.mjs'

function fixturePaths() {
  const root = fs.mkdtempSync(join(os.tmpdir(), 'angelX-process-lease-'))
  return {
    root,
    leaseDir: join(root, 'lock.lease'),
    compatibilityPath: join(root, 'lock'),
  }
}

test('owner classification distinguishes a live incarnation, death, PID reuse, and remote TTL', () => {
  const nowMs = 10_000
  const base = {
    schema: PROCESS_LEASE_SCHEMA,
    token: 'owner-a',
    pid: 42,
    host: 'local',
    process_start: '100',
    acquired_at_ms: 1,
    ts: 1,
  }
  const options = {
    nowMs,
    ttlMs: 100,
    localHost: 'local',
    isAlive: () => true,
    readStartToken: () => '100',
  }

  assert.equal(classifyProcessLeaseOwner(base, options).held, true, 'a live long run beats TTL')
  assert.match(classifyProcessLeaseOwner(base, options).reason, /alive/u)

  const dead = classifyProcessLeaseOwner(base, { ...options, isAlive: () => false })
  assert.equal(dead.reclaimable, true)
  assert.match(dead.reason, /dead/u)

  const reused = classifyProcessLeaseOwner(base, {
    ...options,
    readStartToken: () => '101',
  })
  assert.equal(reused.reclaimable, true)
  assert.match(reused.reason, /reused/u)

  const remoteFresh = classifyProcessLeaseOwner(
    { ...base, host: 'remote', acquired_at_ms: nowMs - 50 },
    options,
  )
  assert.equal(remoteFresh.held, true)
  const remoteStale = classifyProcessLeaseOwner(
    { ...base, host: 'remote', acquired_at_ms: nowMs - 101 },
    options,
  )
  assert.equal(remoteStale.reclaimable, true)
})

test('only the matching owner releases an acquired lease and compatibility receipt', () => {
  const paths = fixturePaths()
  try {
    const acquired = acquireProcessLease({ ...paths, ttlMs: 60_000 })
    assert.equal(acquired.acquired, true)
    assert.equal(fs.existsSync(paths.leaseDir), true)
    assert.equal(fs.existsSync(paths.compatibilityPath), true)
    assert.ok(processStartToken(process.pid))

    const contender = acquireProcessLease({ ...paths, ttlMs: 60_000 })
    assert.equal(contender.acquired, false)
    assert.match(contender.reason, /alive/u)

    assert.equal(
      releaseProcessLease({ ...paths, owner: { ...acquired.owner, token: 'not-the-owner' } }),
      false,
    )
    assert.equal(fs.existsSync(paths.leaseDir), true)
    assert.equal(releaseProcessLease({ ...paths, owner: acquired.owner }), true)
    assert.equal(fs.existsSync(paths.leaseDir), false)
    assert.equal(fs.existsSync(paths.compatibilityPath), false)
  } finally {
    fs.rmSync(paths.root, { recursive: true, force: true })
  }
})

test('legacy JSON locks remain honored while live and are replaced after owner death', () => {
  const paths = fixturePaths()
  try {
    fs.writeFileSync(paths.compatibilityPath, JSON.stringify({ pid: process.pid, ts: Date.now() }))
    const liveLegacy = acquireProcessLease({ ...paths, ttlMs: 60_000 })
    assert.equal(liveLegacy.acquired, false)
    assert.match(liveLegacy.reason, /alive/u)
    assert.equal(fs.existsSync(paths.leaseDir), false)

    fs.writeFileSync(
      paths.compatibilityPath,
      JSON.stringify({ pid: 2_147_483_647, ts: Date.now() }),
    )
    const recovered = acquireProcessLease({ ...paths, ttlMs: 60_000 })
    assert.equal(recovered.acquired, true)
    assert.equal(
      JSON.parse(fs.readFileSync(paths.compatibilityPath, 'utf8')).schema,
      PROCESS_LEASE_SCHEMA,
    )
    assert.equal(releaseProcessLease({ ...paths, owner: recovered.owner }), true)
  } finally {
    fs.rmSync(paths.root, { recursive: true, force: true })
  }
})

const MODULE_URL = pathToFileURL(
  join(import.meta.dirname, '..', '..', 'lib', 'control', 'process-lease.mjs'),
)

const WORKER = String.raw`
import { acquireProcessLease, releaseProcessLease } from ${JSON.stringify(MODULE_URL.href)}
const lease = acquireProcessLease({
  leaseDir: process.env.FIXTURE_LEASE_DIR,
  compatibilityPath: process.env.FIXTURE_COMPAT_PATH,
  ttlMs: 60_000,
})
process.stdout.write(JSON.stringify({
  acquired: lease.acquired,
  reason: lease.reason,
  recovered: lease.recovered ?? null,
  pid: process.pid,
}) + '\n')
if (lease.acquired) {
  await new Promise((resolve) => setTimeout(resolve, Number(process.env.FIXTURE_HOLD_MS)))
  releaseProcessLease({
    leaseDir: process.env.FIXTURE_LEASE_DIR,
    compatibilityPath: process.env.FIXTURE_COMPAT_PATH,
    owner: lease.owner,
  })
}
`

function worker(paths, holdMs) {
  return spawn(process.execPath, ['--input-type=module', '--eval', WORKER], {
    stdio: ['ignore', 'pipe', 'pipe'],
    env: {
      ...process.env,
      FIXTURE_LEASE_DIR: paths.leaseDir,
      FIXTURE_COMPAT_PATH: paths.compatibilityPath,
      FIXTURE_HOLD_MS: String(holdMs),
    },
  })
}

function firstJsonLine(child) {
  return new Promise((resolve, reject) => {
    let stdout = ''
    let stderr = ''
    child.stdout.setEncoding('utf8')
    child.stderr.setEncoding('utf8')
    child.stdout.on('data', (chunk) => {
      stdout += chunk
      const newline = stdout.indexOf('\n')
      if (newline < 0) return
      try {
        resolve(JSON.parse(stdout.slice(0, newline)))
      } catch (error) {
        reject(error)
      }
    })
    child.stderr.on('data', (chunk) => {
      stderr += chunk
    })
    child.on('error', reject)
    child.on('exit', (code) => {
      if (!stdout.includes('\n')) reject(new Error(`worker exited ${code}: ${stderr}`))
    })
  })
}

function exited(child) {
  return new Promise((resolve, reject) => {
    if (child.exitCode != null || child.signalCode != null) {
      resolve({ code: child.exitCode, signal: child.signalCode })
      return
    }
    child.once('error', reject)
    child.once('exit', (code, signal) => resolve({ code, signal }))
  })
}

test('after a real owner is killed, exactly one concurrent successor reclaims the lease', async () => {
  const paths = fixturePaths()
  try {
    const original = worker(paths, 60_000)
    const originalRow = await firstJsonLine(original)
    assert.equal(originalRow.acquired, true)
    original.kill('SIGKILL')
    await exited(original)
    assert.equal(fs.existsSync(paths.leaseDir), true, 'the killed owner leaves durable evidence')

    const successors = [worker(paths, 500), worker(paths, 500)]
    const rows = await Promise.all(successors.map(firstJsonLine))
    assert.equal(rows.filter((row) => row.acquired).length, 1)
    assert.equal(rows.filter((row) => !row.acquired).length, 1)
    const winner = rows.find((row) => row.acquired)
    assert.match(winner.recovered.reason, /dead/u)

    await Promise.all(successors.map(exited))
    assert.equal(fs.existsSync(paths.leaseDir), false)
    assert.equal(fs.existsSync(paths.compatibilityPath), false)
  } finally {
    fs.rmSync(paths.root, { recursive: true, force: true })
  }
})

test('a fresh contender winning the reclaim publish gap is reported as contention', () => {
  const paths = fixturePaths()
  const reclaimDir = `${paths.leaseDir}.reclaim`
  const originalMkdirSync = fs.mkdirSync
  const nowMs = Date.now()
  const competingOwner = {
    schema: PROCESS_LEASE_SCHEMA,
    token: 'gap-winner',
    pid: process.pid,
    host: os.hostname(),
    process_start: processStartToken(process.pid),
    acquired_at_ms: nowMs,
    ts: nowMs,
  }
  let injected = false

  try {
    const stale = acquireProcessLease({
      ...paths,
      ttlMs: 60_000,
      pid: 2_147_483_647,
      token: 'dead-owner',
      startToken: 'dead-start',
    })
    assert.equal(stale.acquired, true)

    fs.mkdirSync = (path, options) => {
      if (
        !injected &&
        path === paths.leaseDir &&
        fs.existsSync(reclaimDir) &&
        !fs.existsSync(paths.leaseDir)
      ) {
        injected = true
        originalMkdirSync.call(fs, path, { mode: 0o700 })
        fs.writeFileSync(join(path, 'owner.json'), `${JSON.stringify(competingOwner)}\n`)
        fs.writeFileSync(paths.compatibilityPath, `${JSON.stringify(competingOwner)}\n`)
      }
      return originalMkdirSync.call(fs, path, options)
    }

    const recovered = acquireProcessLease({ ...paths, ttlMs: 60_000 })
    assert.equal(injected, true, 'the fixture exercised the reclaim publish gap')
    assert.equal(recovered.acquired, false)
    assert.equal(recovered.owner.token, competingOwner.token)
    assert.match(recovered.reason, /alive/u)
    assert.equal(fs.existsSync(reclaimDir), false)
  } finally {
    fs.mkdirSync = originalMkdirSync
    releaseProcessLease({ ...paths, owner: competingOwner })
    fs.rmSync(paths.root, { recursive: true, force: true })
  }
})

// Logical timestamps make expiry boundaries reproducible without scheduler sleeps.
test('renewal extends heartbeat expiry, but a stale live owner cannot renew or release its successor', async () => {
  const { renewProcessLease } = await import('../../lib/control/process-lease.mjs')
  const paths = fixturePaths()
  try {
    const nowMs = Date.now()
    const lease = acquireProcessLease({ ...paths, nowMs, heartbeatMs: 10, staleAfterMs: 50 })
    assert.equal(
      renewProcessLease({ ...paths, owner: lease.owner, nowMs: nowMs + 40 }).renewed,
      true,
    )
    assert.equal(acquireProcessLease({ ...paths, nowMs: nowMs + 89 }).acquired, false)
    assert.equal(
      renewProcessLease({ ...paths, owner: lease.owner, nowMs: nowMs + 90 }).reason,
      'stale-heartbeat',
    )
    const next = acquireProcessLease({ ...paths, nowMs: nowMs + 90 })
    assert.equal(next.acquired, true)
    assert.equal(next.recovered.reason, 'stale-heartbeat')
    assert.equal(next.recovered.prior_token, lease.owner.token)
    assert.equal(next.recovered.prior_pid, process.pid)
    assert.equal(renewProcessLease({ ...paths, owner: lease.owner }).reason, 'owner-token-mismatch')
    assert.equal(releaseProcessLease({ ...paths, owner: lease.owner }), false)
    assert.equal(JSON.parse(fs.readFileSync(paths.compatibilityPath)).token, next.owner.token)
    releaseProcessLease({ ...paths, owner: next.owner })
  } finally {
    fs.rmSync(paths.root, { recursive: true, force: true })
  }
})

test('foreground waits for a renewal boundary and reserves admission ahead of background', async () => {
  const { renewProcessLease } = await import('../../lib/control/process-lease.mjs')
  const paths = fixturePaths()
  try {
    const nowMs = Date.now()
    const lease = acquireProcessLease({ ...paths, nowMs })
    assert.equal(acquireProcessLease({ ...paths, priority: 'foreground', nowMs }).acquired, false)
    assert.equal(
      renewProcessLease({ ...paths, owner: lease.owner, boundary: false, nowMs }).renewed,
      true,
    )
    assert.equal(
      renewProcessLease({ ...paths, owner: lease.owner, nowMs }).reason,
      'foreground-preemption',
    )
    assert.equal(acquireProcessLease({ ...paths, nowMs }).reason, 'foreground request pending')
    const foreground = acquireProcessLease({ ...paths, priority: 'foreground', nowMs })
    assert.equal(foreground.acquired, true)
    assert.equal(releaseProcessLease({ ...paths, owner: lease.owner }), false)
    releaseProcessLease({ ...paths, owner: foreground.owner })
  } finally {
    fs.rmSync(paths.root, { recursive: true, force: true })
  }
})

test('helper renews, protects live critical sections, then hands off without interrupting the callback', async () => {
  const { holdProcessLease } = await import('../../lib/control/process-lease.mjs')
  const paths = fixturePaths()
  try {
    const result = await holdProcessLease(
      async (lease) => {
        assert.equal(acquireProcessLease({ ...paths, priority: 'foreground' }).acquired, false)
        await new Promise((resolve) => setTimeout(resolve, 80))
        const owner = JSON.parse(fs.readFileSync(join(paths.leaseDir, 'owner.json')))
        assert.ok(owner.heartbeat_at_ms > lease.owner.heartbeat_at_ms)
        // Simulate event-loop suspension while protected work remains alive.
        assert.equal(
          acquireProcessLease({ ...paths, nowMs: Date.now() + 1000, priority: 'foreground' })
            .acquired,
          false,
        )
        assert.equal(owner.critical_section, true)
        return 'finished'
      },
      { ...paths, heartbeatMs: 10, staleAfterMs: 50 },
    )
    assert.equal(result.value, 'finished')
    const next = acquireProcessLease({ ...paths, priority: 'foreground' })
    assert.equal(next.acquired, true)
    releaseProcessLease({ ...paths, owner: next.owner })
  } finally {
    fs.rmSync(paths.root, { recursive: true, force: true })
  }
})

test('helper releases on callback failure and an abandoned foreground request expires', async () => {
  const { holdProcessLease } = await import('../../lib/control/process-lease.mjs')
  const paths = fixturePaths()
  try {
    await assert.rejects(
      holdProcessLease(() => {
        throw new Error('fixture failure')
      }, paths),
      /fixture failure/,
    )
    assert.equal(fs.existsSync(paths.leaseDir), false)
    const nowMs = Date.now()
    const lease = acquireProcessLease({ ...paths, nowMs })
    acquireProcessLease({
      ...paths,
      priority: 'foreground',
      nowMs,
      heartbeatMs: 10,
      staleAfterMs: 50,
    })
    releaseProcessLease({ ...paths, owner: lease.owner })
    const next = acquireProcessLease({ ...paths, nowMs: nowMs + 50 })
    assert.equal(next.acquired, true)
    releaseProcessLease({ ...paths, owner: next.owner })
  } finally {
    fs.rmSync(paths.root, { recursive: true, force: true })
  }
})

test('renew and release fail closed while reclamation is serialized', async () => {
  const { renewProcessLease } = await import('../../lib/control/process-lease.mjs')
  const paths = fixturePaths()
  try {
    const lease = acquireProcessLease(paths)
    fs.mkdirSync(`${paths.leaseDir}.reclaim`)
    assert.equal(renewProcessLease({ ...paths, owner: lease.owner }).retry, true)
    assert.equal(releaseProcessLease({ ...paths, owner: lease.owner }), false)
    fs.rmdirSync(`${paths.leaseDir}.reclaim`)
    assert.equal(releaseProcessLease({ ...paths, owner: lease.owner }), true)
  } finally {
    fs.rmSync(paths.root, { recursive: true, force: true })
  }
})
