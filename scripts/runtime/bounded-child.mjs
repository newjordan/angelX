import { spawnSync } from 'node:child_process'
import { fileURLToPath } from 'node:url'

const helper = fileURLToPath(new URL('./bounded-child.py', import.meta.url))

export function boundedSpawnSync(command, args, options = {}) {
  const { timeout = 600_000, stdio = 'pipe', ...rest } = options
  if (!Number.isFinite(timeout) || timeout <= 0)
    throw new Error('bounded child requires a positive finite timeout')
  const streams = Array.isArray(stdio) ? [...stdio] : [stdio, stdio, stdio]
  while (streams.length < 3) streams.push(undefined)
  const statusFd = streams.length
  streams.push('pipe')
  const result = spawnSync(
    'python3',
    [helper, '--timeout-ms', String(Math.ceil(timeout)), '--status-fd', String(statusFd), '--', command, ...args],
    {
      maxBuffer: 16 * 1024 * 1024,
      ...rest,
      stdio: streams,
      detached: true,
      timeout: Math.ceil(timeout) + 5_000,
      killSignal: 'SIGTERM',
    },
  )
  const metadata = result.output?.[statusFd]
  const marker = Buffer.from('deadline\n')
  const deadline = Buffer.isBuffer(metadata)
    ? metadata.equals(marker)
    : typeof metadata === 'string' && metadata === marker.toString(rest.encoding)
  if (result.output) result.output = result.output.slice(0, statusFd)
  if (deadline)
    result.error = Object.assign(
      new Error('child deadline exceeded', result.error ? { cause: result.error } : undefined),
      { code: 'ETIMEDOUT' },
    )
  return result
}
