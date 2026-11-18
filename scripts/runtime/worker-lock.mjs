import { readFileSync, mkdirSync } from 'node:fs'
import { resolve, dirname, join } from 'node:path'
import {
  holdProcessLease,
  classifyProcessLeaseOwner,
  makeProcessLeaseOwner,
} from '../../lib/control/process-lease.mjs'
import { workerPaths } from './worker-paths.mjs'

export async function runGraphCli(cli, argv, defaultGraph = workerPaths().graph) {
  const index = argv.indexOf('--graph')
  if (index >= 0 && (!argv[index + 1]?.trim() || argv[index + 1].startsWith('--'))) {
    console.error('--graph requires a non-empty path')
    return 2
  }
  const selectedGraph = index >= 0 ? argv[index + 1] : defaultGraph
  const graph = resolve(selectedGraph)
  if (argv.includes('--dry-run')) return cli(argv, selectedGraph)
  mkdirSync(dirname(graph), { recursive: true, mode: 0o700 })
  const leaseDir = `${graph}.lease`
  const compatibilityPath = `${graph}.lock`
  let owner
  try {
    owner = JSON.parse(readFileSync(compatibilityPath, 'utf8'))
  } catch {
  }
  if (
    process.env.ANGEL_GRAPH_LEASE_TOKEN &&
    owner?.token === process.env.ANGEL_GRAPH_LEASE_TOKEN &&
    classifyProcessLeaseOwner(owner).held
  )
    return cli(argv, selectedGraph)
  const held = await holdProcessLease(
    async ({ owner: acquired }) => {
      const previous = process.env.ANGEL_GRAPH_LEASE_TOKEN
      process.env.ANGEL_GRAPH_LEASE_TOKEN = acquired.token
      try {
        return await cli(argv, selectedGraph)
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

export const TICK_LOCK_TTL_MS = 60 * 60_000

export function tickLockHolder(lock, nowMs = Date.now()) {
  if (!lock) return null
  if (nowMs - (Number(lock.ts) || 0) >= TICK_LOCK_TTL_MS) return null
  const status = classifyProcessLeaseOwner(lock, { nowMs, ttlMs: TICK_LOCK_TTL_MS })
  return status.held ? { pid: lock.pid ?? null, reason: status.reason } : null
}

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
