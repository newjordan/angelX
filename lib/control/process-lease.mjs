import fs from 'node:fs'
import os from 'node:os'
import { randomUUID } from 'node:crypto'
import { dirname, join } from 'node:path'

export const PROCESS_LEASE_SCHEMA = 'angelX-process-lease/v1'
export const DEFAULT_INITIALIZATION_GRACE_MS = 10_000
export const DEFAULT_HEARTBEAT_MS = 5_000
export const DEFAULT_STALE_AFTER_MS = 30_000

function readJson(path) {
  try {
    return JSON.parse(fs.readFileSync(path, 'utf8'))
  } catch {
    return null
  }
}

function statMtime(path) {
  try {
    return fs.statSync(path).mtimeMs
  } catch {
    return null
  }
}

function syncJson(path, value, { mode = 0o600 } = {}) {
  const fd = fs.openSync(path, 'wx', mode)
  try {
    fs.writeFileSync(fd, `${JSON.stringify(value, null, 2)}\n`)
    fs.fsyncSync(fd)
  } finally {
    fs.closeSync(fd)
  }
}

function replaceJson(path, value, token) {
  fs.mkdirSync(dirname(path), { recursive: true })
  const temporary = `${path}.${token}.tmp`
  const fd = fs.openSync(temporary, 'wx', 0o600)
  try {
    fs.writeFileSync(fd, `${JSON.stringify(value, null, 2)}\n`)
    fs.fsyncSync(fd)
  } finally {
    fs.closeSync(fd)
  }
  try {
    fs.renameSync(temporary, path)
  } catch (error) {
    try {
      fs.unlinkSync(temporary)
    } catch {
      // The rename may have completed before an unrelated error surfaced.
    }
    throw error
  }
}

/** Linux process start-time token (field 22 of /proc/<pid>/stat). */
export function processStartToken(pid, { procRoot = '/proc' } = {}) {
  if (!Number.isInteger(Number(pid)) || Number(pid) <= 0) return null
  try {
    const stat = fs.readFileSync(join(procRoot, String(pid), 'stat'), 'utf8')
    const commandEnd = stat.lastIndexOf(')')
    if (commandEnd < 0) return null
    // Fields after the command begin at field 3 (state); starttime is field 22.
    const fields = stat
      .slice(commandEnd + 1)
      .trim()
      .split(/\s+/u)
    return fields[19] || null
  } catch {
    return null
  }
}

export function processIsAlive(pid) {
  if (!Number.isInteger(Number(pid)) || Number(pid) <= 0) return false
  try {
    process.kill(Number(pid), 0)
    return true
  } catch (error) {
    return error?.code === 'EPERM'
  }
}

export function makeProcessLeaseOwner({
  pid = process.pid,
  nowMs = Date.now(),
  host = os.hostname(),
  token = randomUUID(),
  startToken = processStartToken(pid),
  heartbeatMs = DEFAULT_HEARTBEAT_MS,
  staleAfterMs = DEFAULT_STALE_AFTER_MS,
  priority = 'background',
  criticalSection = false,
} = {}) {
  if (!(heartbeatMs > 0 && staleAfterMs > heartbeatMs && Number.isFinite(staleAfterMs))) {
    throw new Error('heartbeatMs must be positive and less than finite staleAfterMs')
  }
  if (!['foreground', 'background'].includes(priority)) throw new Error('invalid priority')
  return {
    schema: PROCESS_LEASE_SCHEMA,
    token,
    pid: Number(pid),
    host,
    process_start: startToken,
    acquired_at_ms: nowMs,
    heartbeat_at_ms: nowMs,
    heartbeat_ms: heartbeatMs,
    stale_after_ms: staleAfterMs,
    priority,
    critical_section: criticalSection,
    // Compatibility with the existing Conductor/Cut lock receipt.
    ts: nowMs,
  }
}

/**
 * Classify an observed owner without mutating the lease.
 *
 * Heartbeat owners expire outside declared critical sections. Live critical
 * sections fail closed: a timeout is not proof that hardware work has stopped.
 * Pre-heartbeat v1 owners retain their previous PID-incarnation semantics.
 */
export function classifyProcessLeaseOwner(
  owner,
  {
    nowMs = Date.now(),
    ttlMs,
    localHost = os.hostname(),
    leaseMtimeMs = null,
    initializationGraceMs = DEFAULT_INITIALIZATION_GRACE_MS,
    isAlive = processIsAlive,
    readStartToken = processStartToken,
  } = {},
) {
  const ageMs = Math.max(0, nowMs - Number(owner?.acquired_at_ms ?? owner?.ts ?? leaseMtimeMs ?? 0))
  const boundedTtl = Number.isFinite(Number(ttlMs)) ? Math.max(0, Number(ttlMs)) : 0

  if (!owner || !Number.isInteger(Number(owner.pid)) || Number(owner.pid) <= 0) {
    if (leaseMtimeMs != null && nowMs - leaseMtimeMs < initializationGraceMs) {
      return { held: true, reclaimable: false, reason: 'lease owner is still initializing' }
    }
    return { held: false, reclaimable: true, reason: 'lease owner record is missing or invalid' }
  }

  const pid = Number(owner.pid)
  const modern = owner.schema === PROCESS_LEASE_SCHEMA && typeof owner.host === 'string'
  const local = modern ? owner.host === localHost : true

  if (local) {
    if (!isAlive(pid)) {
      return { held: false, reclaimable: true, reason: `lease owner pid ${pid} is dead` }
    }
    if (modern && owner.process_start) {
      const currentStart = readStartToken(pid)
      if (currentStart && currentStart !== owner.process_start) {
        return {
          held: false,
          reclaimable: true,
          reason: `lease owner pid ${pid} was reused by another process`,
        }
      }
    }
    if (
      modern &&
      Number.isFinite(owner.stale_after_ms) &&
      nowMs - owner.heartbeat_at_ms >= owner.stale_after_ms
    ) {
      if (owner.critical_section) {
        return {
          held: true,
          reclaimable: false,
          reason: 'live critical section has a stale heartbeat',
        }
      }
      return { held: false, reclaimable: true, reason: 'stale-heartbeat' }
    }
    if (!modern && boundedTtl > 0 && ageMs >= boundedTtl) {
      return { held: false, reclaimable: true, reason: 'legacy lease exceeded its TTL' }
    }
    return { held: true, reclaimable: false, reason: `lease owner pid ${pid} is alive` }
  }

  if (boundedTtl > 0 && ageMs >= boundedTtl) {
    return { held: false, reclaimable: true, reason: 'foreign-host lease exceeded its TTL' }
  }
  return { held: true, reclaimable: false, reason: 'foreign-host lease is within its TTL' }
}

function removeKnownLeaseDirectory(leaseDir) {
  const entries = fs.readdirSync(leaseDir)
  if (entries.some((entry) => entry !== 'owner.json')) {
    throw new Error(`refusing to reclaim process lease with unknown entries: ${entries.join(', ')}`)
  }
  if (entries.includes('owner.json')) fs.unlinkSync(join(leaseDir, 'owner.json'))
  fs.rmdirSync(leaseDir)
}

function compatibilityMatches(path, owner) {
  const receipt = readJson(path)
  if (!receipt) return false
  if (owner?.token) return receipt.token === owner.token
  return Number(receipt.pid) === Number(owner?.pid) && Number(receipt.ts) === Number(owner?.ts)
}

function publishLease({ leaseDir, compatibilityPath, owner, ttlMs }) {
  fs.mkdirSync(leaseDir, { mode: 0o700 })
  try {
    // Do not silently overtake a legacy pre-lease Conductor which still owns the
    // child-visible lock file.
    const legacy = readJson(compatibilityPath)
    if (legacy) {
      const legacyStatus = classifyProcessLeaseOwner(legacy, { ttlMs })
      if (legacyStatus.held) {
        fs.rmdirSync(leaseDir)
        return { acquired: false, owner: legacy, reason: legacyStatus.reason }
      }
    }

    syncJson(join(leaseDir, 'owner.json'), owner)
    replaceJson(compatibilityPath, owner, owner.token)
    return { acquired: true, owner, recovered: null, reason: 'process lease acquired' }
  } catch (error) {
    try {
      removeKnownLeaseDirectory(leaseDir)
    } catch {
      // Preserve the original acquisition error; a later attempt will fail closed.
    }
    throw error
  }
}

/**
 * Acquire an atomic directory lease while retaining a JSON compatibility lock
 * for child processes. Dead-owner reclamation is serialized by a second atomic
 * directory, so two successors cannot both remove and replace the lease.
 */
function acquireUnlocked({
  leaseDir,
  compatibilityPath,
  ttlMs,
  nowMs = Date.now(),
  pid = process.pid,
  host = os.hostname(),
  token = randomUUID(),
  startToken = processStartToken(pid),
  heartbeatMs,
  staleAfterMs,
  priority,
  criticalSection,
} = {}) {
  if (!leaseDir || !compatibilityPath)
    throw new Error('leaseDir and compatibilityPath are required')
  const owner = makeProcessLeaseOwner({
    pid,
    nowMs,
    host,
    token,
    startToken,
    heartbeatMs,
    staleAfterMs,
    priority,
    criticalSection,
  })

  try {
    return publishLease({ leaseDir, compatibilityPath, owner, ttlMs })
  } catch (error) {
    if (error?.code !== 'EEXIST') throw error
  }

  const observed = readJson(join(leaseDir, 'owner.json'))
  const status = classifyProcessLeaseOwner(observed, {
    nowMs,
    ttlMs,
    leaseMtimeMs: statMtime(leaseDir),
  })
  if (status.held) return { acquired: false, owner: observed, reason: status.reason }

  {
    // Re-read under the reclamation guard. Another successor may have replaced
    // the owner before this process won the guard.
    const current = readJson(join(leaseDir, 'owner.json'))
    const currentStatus = classifyProcessLeaseOwner(current, {
      nowMs,
      ttlMs,
      leaseMtimeMs: statMtime(leaseDir),
    })
    if (currentStatus.held) {
      return { acquired: false, owner: current, reason: currentStatus.reason }
    }

    if (compatibilityMatches(compatibilityPath, current)) fs.unlinkSync(compatibilityPath)
    removeKnownLeaseDirectory(leaseDir)
    let acquired
    try {
      acquired = publishLease({ leaseDir, compatibilityPath, owner, ttlMs })
    } catch (error) {
      if (error?.code !== 'EEXIST' || error?.path !== leaseDir) throw error

      // A caller that began after we took the reclamation guard can still win
      // the narrow gap between removing the stale directory and publishing our
      // replacement. Treat that atomic mkdir win as normal contention instead
      // of leaking EEXIST from the recovery path.
      const competing = readJson(join(leaseDir, 'owner.json'))
      const competingStatus = classifyProcessLeaseOwner(competing, {
        nowMs: Date.now(),
        ttlMs,
        leaseMtimeMs: statMtime(leaseDir),
      })
      return {
        acquired: false,
        owner: competing,
        reason: competingStatus.held
          ? competingStatus.reason
          : 'another contender changed the process lease during reclamation',
      }
    }
    if (!acquired.acquired) return acquired
    return {
      ...acquired,
      recovered: {
        reason: currentStatus.reason,
        prior_pid: Number(current?.pid) || null,
        prior_host: current?.host ?? null,
        prior_token: current?.token ?? null,
        prior_heartbeat_at_ms: current?.heartbeat_at_ms ?? null,
        reclaimed_at_ms: nowMs,
      },
      reason: `recovered process lease: ${currentStatus.reason}`,
    }
  }
}

/** Release only when the on-disk owner still matches this caller's token. */
function releaseUnlocked({ leaseDir, compatibilityPath, owner } = {}) {
  if (!owner?.token) return false
  const current = readJson(join(leaseDir, 'owner.json'))
  if (current?.token !== owner.token) return false
  if (compatibilityMatches(compatibilityPath, owner)) fs.unlinkSync(compatibilityPath)
  removeKnownLeaseDirectory(leaseDir)
  return true
}

// All owner checks and writes share the reclamation guard. A crashed guard is
// deliberately fail-closed; operators must establish quiescence before repair.
function guarded(leaseDir, fn, busy) {
  const guard = `${leaseDir}.reclaim`
  try {
    fs.mkdirSync(guard, { mode: 0o700 })
  } catch (error) {
    if (error.code === 'EEXIST') return busy
    throw error
  }
  try {
    return fn()
  } finally {
    fs.rmdirSync(guard)
  }
}

function foregroundRequest(leaseDir, nowMs) {
  const path = `${leaseDir}.foreground`
  const request = readJson(path)
  if (!request) return null
  const status = classifyProcessLeaseOwner(request, { nowMs })
  if (!status.held) {
    fs.unlinkSync(path)
    return null
  }
  return request
}

export function acquireProcessLease(options = {}) {
  const { leaseDir, compatibilityPath, priority = 'background', nowMs = Date.now() } = options
  if (!leaseDir || !compatibilityPath)
    throw new Error('leaseDir and compatibilityPath are required')
  return guarded(
    leaseDir,
    () => {
      let request = foregroundRequest(leaseDir, nowMs)
      if (priority === 'foreground' && (!request || request.pid === process.pid)) {
        const firstRequestedAt = request?.acquired_at_ms ?? nowMs
        request = makeProcessLeaseOwner({ ...options, nowMs, priority, criticalSection: false })
        request.acquired_at_ms = firstRequestedAt
        request.ts = firstRequestedAt
        replaceJson(`${leaseDir}.foreground`, request, request.token)
      }
      if (priority !== 'foreground' && request) {
        return {
          acquired: false,
          reason: 'foreground request pending',
          owner: readJson(join(leaseDir, 'owner.json')),
        }
      }
      const result = acquireUnlocked({ ...options, nowMs })
      if (result.acquired && priority === 'foreground' && request)
        fs.unlinkSync(`${leaseDir}.foreground`)
      return result
    },
    { acquired: false, reason: 'process lease reclamation is in progress' },
  )
}

/** Call at a quiescent boundary by default; never continue work after renewed=false. */
export function renewProcessLease({
  leaseDir,
  compatibilityPath,
  owner,
  nowMs = Date.now(),
  boundary = true,
  criticalSection,
} = {}) {
  if (!owner?.token) return { renewed: false, reason: 'owner-token-mismatch' }
  return guarded(
    leaseDir,
    () => {
      const current = readJson(join(leaseDir, 'owner.json'))
      if (current?.token !== owner.token) return { renewed: false, reason: 'owner-token-mismatch' }
      const status = classifyProcessLeaseOwner(current, { nowMs })
      if (!status.held) return { renewed: false, reason: status.reason }
      const request = foregroundRequest(leaseDir, nowMs)
      if (boundary && request && current.priority !== 'foreground') {
        releaseUnlocked({ leaseDir, compatibilityPath, owner })
        return {
          renewed: false,
          reason: 'foreground-preemption',
          requested_at_ms: request.acquired_at_ms,
          preempted_at_ms: nowMs,
        }
      }
      const next = {
        ...current,
        heartbeat_at_ms: nowMs,
        critical_section: criticalSection ?? (boundary ? false : current.critical_section),
      }
      replaceJson(join(leaseDir, 'owner.json'), next, randomUUID())
      replaceJson(compatibilityPath, next, randomUUID())
      return {
        renewed: true,
        owner: next,
        preemptionPending: !!request && current.priority !== 'foreground',
      }
    },
    { renewed: false, retry: true, reason: 'process lease reclamation is in progress' },
  )
}

export function releaseProcessLease(options = {}) {
  if (!options.owner?.token) return false
  return guarded(options.leaseDir, () => releaseUnlocked(options), false)
}

/** One callback is one critical section; await all hardware/child work inside it.
 * No run is cancelled or timed out. Priority takes effect once the callback ends.
 * Pass a previously acquired lease only when no protected work has started yet.
 */
export async function holdProcessLease(fn, options = {}) {
  const lease = options.lease ?? acquireProcessLease({ ...options, criticalSection: true })
  if (!lease.acquired) return lease
  const args = { ...options, owner: lease.owner }
  const delay = () => new Promise((resolve) => setTimeout(resolve, lease.owner.heartbeat_ms))
  let entered
  do {
    entered = renewProcessLease({ ...args, boundary: false, criticalSection: true })
    if (entered.retry) await delay()
  } while (entered.retry)
  if (!entered.renewed) return { ...lease, acquired: false, reason: entered.reason }
  const outcome = { ...lease }
  let renewalError
  const timer = setInterval(() => {
    try {
      const result = renewProcessLease({ ...args, boundary: false })
      if (!result.renewed && !result.retry) renewalError = new Error(result.reason)
    } catch (error) {
      renewalError = error
    }
  }, lease.owner.heartbeat_ms)
  try {
    const value = await fn(lease)
    if (renewalError) throw renewalError
    return Object.assign(outcome, { value })
  } finally {
    clearInterval(timer)
    // Wait for concurrent metadata writes, without bounding the caller's run.
    let result
    do {
      result = renewProcessLease({ ...args, boundary: true })
      if (result.retry) await delay()
    } while (result.retry)
    outcome.handoff = result
    while (result.renewed && !releaseProcessLease(args)) {
      const current = readJson(join(args.leaseDir, 'owner.json'))
      if (current?.token !== args.owner.token) break
      await delay()
    }
  }
}
