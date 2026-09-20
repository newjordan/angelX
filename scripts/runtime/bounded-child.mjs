import { spawnSync } from 'node:child_process'
import { fileURLToPath } from 'node:url'

const helper = fileURLToPath(new URL('./bounded-child.py', import.meta.url))

export function boundedSpawnSync(command, args, options = {}) {
  const { timeout = 600_000, ...rest } = options
  if (!Number.isFinite(timeout) || timeout <= 0)
    throw new Error('bounded child requires a positive finite timeout')
  const result = spawnSync(
    'python3',
    [helper, '--timeout-ms', String(Math.ceil(timeout)), '--', command, ...args],
    {
      maxBuffer: 16 * 1024 * 1024,
      ...rest,
      detached: true,
      timeout: Math.ceil(timeout) + 5_000,
      killSignal: 'SIGTERM',
    },
  )
  if (result.status === 124 && !result.error)
    result.error = Object.assign(new Error('child deadline exceeded'), { code: 'ETIMEDOUT' })
  return result
}
