import { readFileSync, mkdirSync } from 'node:fs'
import { resolve, dirname, join } from 'node:path'
import { holdProcessLease, classifyProcessLeaseOwner } from '../lib/control/process-lease.mjs'
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
