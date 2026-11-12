import { readFileSync, mkdirSync } from 'node:fs'
import { resolve, dirname, join } from 'node:path'
import {
  holdProcessLease,
  classifyProcessLeaseOwner,
  makeProcessLeaseOwner,
} from '../../lib/control/process-lease.mjs'
import { workerPaths } from './worker-paths.mjs'

// All CLI graph writers share one lease, including manual compiler refreshes.
// A scheduler hands the lease to its synchronous child and reloads the graph
// after that child exits. Other workers fail busy instead of overwriting it.
export async function runGraphCli(cli, argv) {
  const index = argv.indexOf('--graph')
  const outIndex = argv.indexOf('--out')
  const graph = resolve(
    index >= 0
      ? argv[index + 1]
      : process.env.ANGEL_CAUSAL_GRAPH ||
          (outIndex >= 0 ? `${argv[outIndex + 1]}/graph.json` : workerPaths().graph),
  )
  if (argv.includes('--dry-run')) return cli(argv)
  mkdirSync(dirname(graph), { recursive: true, mode: 0o700 })
  const leaseDir = `${graph}.lease`
  const compatibilityPath = `${graph}.lock`
  let owner
  try {
    owner = JSON.parse(readFileSync(compatibilityPath, 'utf8'))
  } catch {
    /* no owner */
  }
  if (
    process.env.ANGEL_GRAPH_LEASE_TOKEN &&
    owner?.token === process.env.ANGEL_GRAPH_LEASE_TOKEN &&
    classifyProcessLeaseOwner(owner).held
  )
    return cli(argv)
  const held = await holdProcessLease(
    async ({ owner: acquired }) => {
      const previous = process.env.ANGEL_GRAPH_LEASE_TOKEN
      process.env.ANGEL_GRAPH_LEASE_TOKEN = acquired.token
      try {
        return await cli(argv)
      } finally {
        if (previous === undefined) delete process.env.ANGEL_GRAPH_LEASE_TOKEN
        else process.env.ANGEL_GRAPH_LEASE_TOKEN = previous
      }
    },
    { leaseDir, compatibilityPath, ttlMs: 3_600_000 },
  )
  if (!held.acquired) {
    console.error(`graph busy: ${held.reason}`)
    return 75
  }
  return held.value
}

// The second guard. The lease above keys on the graph path; a tick also owns the
// files in its state dir (heartbeat, budget, verdict spool), which two ticks aimed
// at different graphs would share. So a tick keeps one lock file there, written as
// the lease's own owner record and judged by the lease's own rules: held while its
// process is alive and under an hour old, free the moment its pid is dead. A tick
// killed outright therefore frees the dir for the next one instead of holding it
// for the hour.
export const TICK_LOCK_TTL_MS = 60 * 60_000

/** Who holds the state-dir lock, or null when it is free. `lock` is the parsed file. */
export function tickLockHolder(lock, nowMs = Date.now()) {
  if (!lock) return null
  // The hour is a hard ceiling, whatever the classifier makes of the pid.
  if (nowMs - (Number(lock.ts) || 0) >= TICK_LOCK_TTL_MS) return null
  const status = classifyProcessLeaseOwner(lock, { nowMs, ttlMs: TICK_LOCK_TTL_MS })
  return status.held ? { pid: lock.pid ?? null, reason: status.reason } : null
}

/** The record a tick writes to the lock file when it takes the dir. */
export function tickLockOwner(nowMs = Date.now()) {
  return makeProcessLeaseOwner({ nowMs, staleAfterMs: TICK_LOCK_TTL_MS })
}

export async function runStoreCli(cli, argv, defaultDir) {
  if (argv.includes('--dry-run')) return cli(argv)
  const index = argv.indexOf('--state-dir')
  const state = resolve(index >= 0 ? argv[index + 1] : defaultDir)
  mkdirSync(state, { recursive: true, mode: 0o700 })
  const held = await holdProcessLease(() => cli(argv), {
    leaseDir: join(state, 'lock.lease'),
    compatibilityPath: join(state, 'lock'),
    ttlMs: 3_600_000,
  })
  if (!held.acquired) {
    console.error(`worker busy: ${held.reason}`)
    return 75
  }
  return held.value
}
