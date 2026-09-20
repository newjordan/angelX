import { randomUUID } from 'node:crypto'
import { join } from 'node:path'
import { boundedSpawnSync } from './bounded-child.mjs'
import { mkdirSync, writeFileSync } from './private-store-fs.mjs'
import { workerPaths } from './worker-paths.mjs'

// External evaluators own tasks and held-out verifiers. Pass a JSON argv array;
// stdin carries the request, stdout returns its bound, freshly measured report.
export function runBenchmark(
  spec,
  {
    command = process.env.ANGEL_BENCHMARK_COMMAND,
    reportsDir = workerPaths().reports,
    timeout = 600_000,
    env = process.env,
  } = {},
) {
  let argv
  try {
    argv = JSON.parse(command)
  } catch {
    /* diagnosed below */
  }
  if (!Array.isArray(argv) || !argv.length || argv.some((arg) => typeof arg !== 'string' || !arg))
    throw new Error(
      'ANGEL_BENCHMARK_COMMAND must be a JSON argv array for the task/verifier runner',
    )
  const request = { schema: 'angel.benchmark_request/v1', request_id: randomUUID(), spec }
  const child = boundedSpawnSync(argv[0], argv.slice(1), {
    input: JSON.stringify(request),
    encoding: 'utf8',
    timeout,
    env,
  })
  if (child.status !== 0)
    throw new Error(`benchmark runner failed (${child.status ?? child.error?.code ?? 'spawn'})`)
  let report
  try {
    report = JSON.parse(child.stdout)
  } catch {
    throw new Error('benchmark runner returned invalid JSON')
  }
  if (
    report?.schema !== 'angel.benchmark_report/v1' ||
    report.request_id !== request.request_id ||
    report.control?.suite !== spec.suite ||
    !/^[0-9a-f]{64}$/.test(report.control?.evaluation_sha256 || '') ||
    !Array.isArray(report.metrics)
  )
    throw new Error('benchmark report does not match this request and suite')
  const subjects = spec.subjects.map((subject) => subject.name).sort()
  if (JSON.stringify(report.metrics.map((row) => row.subject).sort()) !== JSON.stringify(subjects))
    throw new Error('benchmark report must contain exactly the requested subjects')
  for (const row of report.metrics) {
    if (
      typeof row.accuracy_pct !== 'number' ||
      !Number.isFinite(row.accuracy_pct) ||
      row.accuracy_pct < 0 ||
      row.accuracy_pct > 100 ||
      typeof row.accuracy_std !== 'number' ||
      !Number.isFinite(row.accuracy_std) ||
      row.accuracy_std < 0 ||
      !Number.isInteger(row.n) ||
      row.n < 2
    )
      throw new Error(
        'benchmark report requires measured accuracy, variance, and at least two observations per subject',
      )
  }
  mkdirSync(reportsDir, { recursive: true })
  const file = join(reportsDir, `benchmark-${request.request_id}.json`)
  writeFileSync(file, JSON.stringify({ ...report, request }, null, 2))
  return { report, file }
}
