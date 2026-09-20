import { test } from 'node:test'
import assert from 'node:assert/strict'
import fs from 'node:fs'
import os from 'node:os'
import { join } from 'node:path'
import { fileURLToPath } from 'node:url'
import { spawn } from 'node:child_process'
import { createServer } from 'node:http'
import { workerPaths } from '../../scripts/worker-paths.mjs'
import { boundedSpawnSync } from '../../scripts/bounded-child.mjs'
import { runBenchmark } from '../../scripts/benchmark-runner.mjs'
import { isMachineLabeled } from '../../scripts/cut-evidence.mjs'
import CausalGraph from '../../lib/research/CausalGraph.js'
import { ingestAgenda } from '../../scripts/conductor.mjs'

const scripts = fileURLToPath(new URL('../../scripts/', import.meta.url))
const json = (path) => JSON.parse(fs.readFileSync(path, 'utf8'))
function fixture(t) {
  const dir = fs.mkdtempSync(join(os.tmpdir(), 'angel-worker-wiring-'))
  t.after(() => fs.rmSync(dir, { recursive: true, force: true }))
  const env = {
    ...process.env,
    NODE_OPTIONS: '',
    NODE_TEST_CONTEXT: '',
    ANGEL_CAUSAL_GRAPH: join(dir, 'graph.json'),
    ANGEL_EXPERIENCE_LOG: join(dir, 'ledger.jsonl'),
    ANGEL_HABITS_DIR: join(dir, 'habits'),
    ANGEL_SKILLS_DIR: join(dir, 'skills'),
    ANGEL_HABIT_PROPOSED_DIR: join(dir, 'proposed'),
    ANGEL_DOSSIER_DIR: join(dir, 'dossier'),
    ANGEL_CUT_DIR: join(dir, 'cut'),
    ANGEL_MISSION_DIR: join(dir, 'missions'),
    ANGEL_CONDUCTOR_DIR: join(dir, 'conductor'),
    ANGEL_REFLEX_DIR: join(dir, 'reflex'),
    ANGEL_BENCHMARK_REPORTS_DIR: join(dir, 'reports'),
    ANGEL_STILL_DIR: join(dir, 'still'),
    ANGEL_BARREL_DIR: join(dir, 'barrel'),
    ANGEL_TRAJECTORY_DIR: join(dir, 'trajectories'),
    ANGEL_HABIT_MIN_BELIEF: '',
    ANGEL_HABIT_MAX_PROPOSALS_PER_DAY: '',
    ANGEL_HABITS: '1',
    ANGEL_STILL: '1',
    ANGEL_STILL_STUDENT_KEY: '',
    ANGEL_STILL_JUDGE_KEY: '',
    ANGEL_BRAIN_KEY: '',
    ANGEL_STILL_JUDGE_URL: '',
    ANGEL_STILL_JUDGE_MODEL: '',
    ANGEL_BARREL_TTL_DAYS: '7',
    ANGEL_STILL_MAX_SCORES: '40',
    ANGEL_STILL_MAX_SCORES_PER_DAY: '40',
    ANGEL_GRAPH_LEASE_TOKEN: '',
    ANGEL_BENCHMARK_COMMAND: '',
  }
  return { dir, env }
}
function run(script, args, env) {
  return new Promise((resolve, reject) => {
    const child = spawn(process.execPath, [join(scripts, script), ...args], {
      env,
      stdio: ['ignore', 'pipe', 'pipe'],
    })
    let stdout = '',
      stderr = ''
    child.stdout.on('data', (chunk) => {
      stdout += chunk
    })
    child.stderr.on('data', (chunk) => {
      stderr += chunk
    })
    const timer = setTimeout(() => child.kill('SIGTERM'), 15_000)
    child.on('error', reject)
    child.on('close', (status) => {
      clearTimeout(timer)
      resolve({ status, stdout, stderr })
    })
  })
}
function seedBarrel(env, count = 1) {
  fs.mkdirSync(env.ANGEL_BARREL_DIR, { recursive: true })
  const date = new Date(Date.now() - 86_400_000).toISOString().slice(0, 10).replaceAll('-', '')
  const path = join(env.ANGEL_BARREL_DIR, `barrel-${date}.jsonl`)
  fs.writeFileSync(
    path,
    Array.from({ length: count }, (_, i) =>
      JSON.stringify({
        v: 1,
        ts: Math.floor(Date.now() / 1000) - 86_400,
        digest: `fixture-${i}`,
        context: [{ role: 'user', content: 'Local fixture: add two and two.' }],
        answer: '4',
        model: 'fixture-teacher',
        repo: { key: 'fixture' },
      }),
    ).join('\n') + '\n',
  )
  return path
}
async function server(t, handler) {
  const instance = createServer(handler)
  await new Promise((resolve) => instance.listen(0, '127.0.0.1', resolve))
  t.after(() => new Promise((resolve) => instance.close(resolve)))
  return `http://127.0.0.1:${instance.address().port}/v1`
}

test('worker defaults match native stores and share one overrideable graph', () => {
  const defaults = workerPaths({}, '/synthetic-home')
  assert.equal(defaults.graph, '/synthetic-home/.angel0/dossier/graph.json')
  assert.equal(defaults.proposed, '/synthetic-home/.angel0/skills-proposed')
  assert.equal(defaults.gauge, '/synthetic-home/.angel0/still/gap.json')
  const override = workerPaths(
    { ANGEL_DOSSIER_DIR: '/custom/dossier', ANGEL_STILL_DIR: '/custom/still' },
    '/synthetic-home',
  )
  assert.equal(override.graph, '/custom/dossier/graph.json')
  assert.equal(override.gauge, '/custom/still/gap.json')
  for (const value of Object.values(defaults)) assert.ok(!value.includes('public/data'))
})

test('missing and timed-out machine exits do not satisfy the evidence floor', () => {
  for (const exit of [null, '', undefined])
    assert.equal(isMachineLabeled({ machine: { exit } }), false)
  assert.equal(isMachineLabeled({ machine: { exit: 0, timed_out: true } }), false)
  assert.equal(isMachineLabeled({ machine: { exit: 0 } }), true)
})

test('real Conductor dispatch preserves custom graph, ledger, proposals, and child mutations', async (t) => {
  const { dir, env } = fixture(t)
  const graph = new CausalGraph()
  ingestAgenda(graph, [
    {
      rung: 'skills',
      slug: 'wiring',
      goal: 'Refresh recorded workflow proposals',
      severity: 1,
      estCostMin: 1,
      observed: { samples: 10 },
    },
  ])
  fs.writeFileSync(env.ANGEL_CAUSAL_GRAPH, JSON.stringify(graph.serialize()))
  const rows = []
  for (let session = 1; session <= 12; session++)
    for (const [seq, text] of ['cargo build', 'cargo test'].entries()) {
      rows.push({
        kind: 'event',
        event: 'cmd',
        v: 3,
        session,
        seq,
        ts: Math.floor(Date.now() / 1000),
        repo: { key: 'fixture-workspace', root: dir, slug: 'fixture-workspace' },
        cmd: {
          text,
          exit: 0,
          timed_out: false,
          verdict: 'pass',
          pipefail: true,
          independent: true,
          source: 'agent',
        },
      })
    }
  fs.writeFileSync(env.ANGEL_EXPERIENCE_LOG, rows.map((row) => JSON.stringify(row)).join('\n'))
  const result = await run(
    'conductor-tick.mjs',
    ['--force', '--no-build', '--graph', env.ANGEL_CAUSAL_GRAPH],
    {
      ...env,
      ANGEL_CONDUCTOR: '1',
      ANGEL_CUT_MIN_CORPUS: '0',
      ANGEL_CUT_MIN_MACHINE: '0',
      ANGEL_CUT: '1',
    },
  )
  assert.equal(result.status, 0, result.stdout + result.stderr)
  assert.equal(json(join(env.ANGEL_CONDUCTOR_DIR, 'heartbeat.json')).action, 'ran')
  assert.equal(json(join(env.ANGEL_CONDUCTOR_DIR, 'budget.json')).runs, 1)
  assert.equal(json(join(env.ANGEL_HABITS_DIR, 'status.json')).facts.length, 1)
  assert.equal(fs.readdirSync(env.ANGEL_HABIT_PROPOSED_DIR).length, 1)
  const updated = CausalGraph.deserialize(json(env.ANGEL_CAUSAL_GRAPH))
  assert.ok(updated.nodes().some((node) => node.projectId === 'habitsmith'))
  assert.ok(updated.nodes().some((node) => node.lastDispatchedAt))
})

test('Still scores through the configured local endpoint and publishes its dataset and status', async (t) => {
  const { env } = fixture(t)
  const shard = seedBarrel(env)
  let completions = 0
  const endpoint = await server(t, (req, res) => {
    res.setHeader('content-type', 'application/json')
    if (req.url === '/v1/models')
      return res.end(JSON.stringify({ data: [{ id: 'fixture-student' }] }))
    let body = ''
    req.on('data', (chunk) => {
      body += chunk
    })
    req.on('end', () => {
      assert.equal(JSON.parse(body).model, 'fixture-student')
      completions++
      res.end(
        JSON.stringify({
          choices: [{ message: { content: completions === 1 ? '4' : '{"quality":9,"gap":2}' } }],
        }),
      )
    })
  })
  const result = await run('still-tick.mjs', ['--force', '--distill'], {
    ...env,
    ANGEL_STILL_STUDENT_URL: endpoint,
  })
  assert.equal(result.status, 0, result.stdout + result.stderr)
  assert.equal(completions, 2)
  assert.equal(
    JSON.parse(fs.readFileSync(shard, 'utf8').trim()).score.student_model,
    'fixture-student',
  )
  const status = json(join(env.ANGEL_STILL_DIR, 'status.json'))
  assert.equal(status.scored_this_tick, 1)
  assert.equal(status.unscored, 0)
  assert.equal(status.budget.runs, 1)
  const datasets = fs.readdirSync(env.ANGEL_TRAJECTORY_DIR)
  assert.equal(datasets.length, 1)
  assert.equal(
    fs.readFileSync(join(env.ANGEL_TRAJECTORY_DIR, datasets[0]), 'utf8').trim().split('\n').length,
    1,
  )
})

test('Still charges failed attempts and stops retrying when its daily budget is spent', async (t) => {
  const { env } = fixture(t)
  seedBarrel(env, 5)
  let completions = 0
  const endpoint = await server(t, (req, res) => {
    if (req.url === '/v1/models')
      return res.end(JSON.stringify({ data: [{ id: 'fixture-student' }] }))
    completions++
    res.writeHead(401).end('{}')
  })
  const configured = {
    ...env,
    ANGEL_STILL_STUDENT_URL: endpoint,
    ANGEL_STILL_MAX_SCORES_PER_DAY: '3',
  }
  for (let i = 0; i < 2; i++) {
    const result = await run('still-tick.mjs', ['--force'], configured)
    assert.equal(result.status, 0, result.stdout + result.stderr)
  }
  assert.equal(completions, 3)
  assert.equal(json(join(env.ANGEL_STILL_DIR, 'budget.json')).runs, 3)
})

test('fresh benchmark responses are bound to the request and null metrics are rejected', (t) => {
  const { dir } = fixture(t)
  const backend = join(dir, 'benchmark.mjs')
  const source = (metric, id) => `let text=''; for await (const c of process.stdin) text+=c;
    const request=JSON.parse(text); console.log(JSON.stringify({schema:'angel.benchmark_report/v1',
    request_id:${id}, control:{suite:request.spec.suite,evaluation_sha256:'${'a'.repeat(64)}'}, metrics:request.spec.subjects.map(s=>({
    subject:s.name, accuracy_pct:${metric}, accuracy_std:0, n:4}))}));`
  const spec = {
    suite: 'fixture',
    seeds: 2,
    subjects: [
      { name: 'baseline', env: {} },
      { name: 'treatment', env: {} },
    ],
  }
  const options = {
    command: JSON.stringify([process.execPath, backend]),
    reportsDir: join(dir, 'reports'),
  }
  fs.writeFileSync(backend, source('75', 'request.request_id'))
  const measured = runBenchmark(spec, options)
  assert.equal(measured.report.metrics[0].accuracy_pct, 75)
  fs.writeFileSync(backend, source('75', "'old-request'"))
  assert.throws(() => runBenchmark(spec, options), /does not match/)
  fs.writeFileSync(backend, source('null', 'request.request_id'))
  assert.throws(() => runBenchmark(spec, options), /measured accuracy/)
  assert.equal(
    fs.readdirSync(options.reportsDir).length,
    1,
    'rejected output is never published as measured evidence',
  )
})

test('a timed-out child cannot leave its background descendants running', async (t) => {
  const { dir } = fixture(t)
  const marker = join(dir, 'should-not-exist')
  const result = boundedSpawnSync('bash', ['-c', 'sleep 0.5; touch "$1"', 'fixture', marker], {
    encoding: 'utf8',
    timeout: 80,
  })
  assert.equal(result.error?.code, 'ETIMEDOUT')
  await new Promise((resolve) => setTimeout(resolve, 600))
  assert.equal(fs.existsSync(marker), false)
})
