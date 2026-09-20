import { test } from 'node:test'
import assert from 'node:assert/strict'
import fs from 'node:fs'
import os from 'node:os'
import { spawn, spawnSync } from 'node:child_process'
import { dirname, join } from 'node:path'
import { fileURLToPath, pathToFileURL } from 'node:url'

import CausalGraph from '../../lib/research/CausalGraph.js'
import { agendaNodeId, ingestAgenda } from '../../scripts/runtime/conductor.mjs'

const SCRIPTS = join(dirname(fileURLToPath(import.meta.url)), '..', '..', 'scripts')
const ROOT = join(SCRIPTS, '..')
const REAL_REFLEX_URL = pathToFileURL(join(SCRIPTS, 'reflex-tick.mjs')).href

function fakeReflex(body) {
  return `
export * from ${JSON.stringify(REAL_REFLEX_URL)}
const isCli = process.argv[1] &&
  (await import('node:url')).fileURLToPath(import.meta.url) === process.argv[1]
if (isCli) {
${body}
}
`
}

function conductorArgs(root, stateDir, graphPath) {
  return [
    join(root, 'scripts', 'conductor-tick.mjs'),
    '--force',
    '--no-build',
    '--graph',
    graphPath,
    '--state-dir',
    stateDir,
    '--ledger',
    join(root, 'missing-ledger.jsonl'),
    '--reports-dir',
    join(root, 'reports'),
    '--habits-status',
    join(root, 'missing-habits.json'),
    '--proposals',
    join(root, 'missing-proposals.md'),
  ]
}

async function waitFor(check, message, timeoutMs = 5_000) {
  const deadline = Date.now() + timeoutMs
  while (Date.now() < deadline) {
    if (check()) return
    await new Promise((resolve) => setTimeout(resolve, 10))
  }
  throw new Error(message)
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

test('a killed Conductor resumes from its durable dispatch without replaying the child', async () => {
  const root = fs.mkdtempSync(join(os.tmpdir(), 'angel0-conductor-recovery-'))
  const stateDir = join(root, 'state')
  const graphPath = join(root, 'graph.json')
  const marker = join(root, 'child-starts.txt')
  let first = null

  try {
    fs.mkdirSync(join(root, 'scripts'))
    fs.mkdirSync(join(root, 'reports'))
    fs.mkdirSync(stateDir)
    fs.symlinkSync(join(ROOT, 'lib'), join(root, 'lib'))
    fs.copyFileSync(
      join(SCRIPTS, 'conductor-tick.mjs'),
      join(root, 'scripts', 'conductor-tick.mjs'),
    )
    // conductor-tick.mjs is copied (not linked) so its fake siblings below resolve
    // inside the fixture; every real module it imports relative to itself must be
    // linked here, including the private-store-fs adapter it loads at CLI time.
    for (const module of [
      'conductor.mjs',
      'causal-loop.mjs',
      'mission.mjs',
      'private-store-fs.mjs',
      'worker-paths.mjs',
      'worker-lock.mjs',
      'bounded-child.mjs',
      'benchmark-runner.mjs',
    ]) {
      fs.symlinkSync(join(SCRIPTS, module), join(root, 'scripts', module))
    }
    fs.writeFileSync(join(root, 'scripts', 'cut-tick.mjs'), 'process.exit(0)\n')
    fs.writeFileSync(
      join(root, 'scripts', 'cut-corpus.mjs'),
      `console.log(JSON.stringify({ writes: { judged: 999 }, machine: { labeled: 999 } }))\n`,
    )
    fs.writeFileSync(
      join(root, 'scripts', 'reflex-tick.mjs'),
      fakeReflex(`
const fs = (await import('node:fs')).default
fs.appendFileSync(process.env.FIXTURE_CHILD_MARKER, 'started\\n')
setInterval(() => {}, 60_000)
`),
    )

    const graph = new CausalGraph()
    const agenda = { rung: 'config', slug: 'recovery-fixture' }
    ingestAgenda(
      graph,
      [
        {
          ...agenda,
          goal: 'Exercise durable Conductor recovery.',
          estCostMin: 5,
          severity: 1,
          observed: { samples: 2 },
        },
      ],
      { now: new Date().toISOString() },
    )
    const agendaId = agendaNodeId(agenda.rung, agenda.slug)
    fs.writeFileSync(graphPath, JSON.stringify(graph.serialize(), null, 2))

    const env = {
      ...process.env,
      HOME: root,
      ANGEL_CONDUCTOR: '1',
      ANGEL_CUT_MIN_CORPUS: '',
      ANGEL_CUT_MIN_MACHINE: '',
      FIXTURE_CHILD_MARKER: marker,
    }
    first = spawn(process.execPath, conductorArgs(root, stateDir, graphPath), {
      cwd: root,
      detached: true,
      stdio: ['ignore', 'pipe', 'pipe'],
      env,
    })
    await waitFor(() => fs.existsSync(marker), 'the first Conductor never dispatched its child')

    const handedOff = CausalGraph.deserialize(JSON.parse(fs.readFileSync(graphPath, 'utf8')))
    const dispatchedAt = handedOff.getNode(agendaId)?.lastDispatchedAt
    assert.ok(dispatchedAt, 'dispatch identity must reach durable graph state before child launch')
    assert.equal(fs.existsSync(join(stateDir, 'lock.lease')), true)

    const contender = spawnSync(process.execPath, conductorArgs(root, stateDir, graphPath), {
      cwd: root,
      encoding: 'utf8',
      env,
    })
    assert.equal(contender.status, 75)
    assert.match(contender.stderr, /graph busy/u)
    assert.equal(
      fs.existsSync(join(stateDir, 'heartbeat.json')),
      false,
      'a contender must not replace the active owner heartbeat with a skip',
    )

    const firstExit = exited(first)
    process.kill(-first.pid, 'SIGKILL')
    await firstExit
    first = null

    // If recovery forgets the persisted cooldown, this replacement child makes
    // the duplicate externally visible without hanging the test.
    fs.writeFileSync(
      join(root, 'scripts', 'reflex-tick.mjs'),
      fakeReflex(`
const fs = (await import('node:fs')).default
fs.appendFileSync(process.env.FIXTURE_CHILD_MARKER, 'DUPLICATE\\n')
`),
    )
    const successor = spawnSync(process.execPath, conductorArgs(root, stateDir, graphPath), {
      cwd: root,
      encoding: 'utf8',
      env,
    })
    assert.equal(
      successor.status,
      0,
      `successor failed:\n${successor.stdout ?? ''}\n${successor.stderr ?? ''}`,
    )
    assert.deepEqual(fs.readFileSync(marker, 'utf8').trim().split('\n'), ['started'])
    assert.match(successor.stdout, /no agenda outside cooldown/u)
    assert.match(
      fs.readFileSync(join(stateDir, 'briefing.md'), 'utf8'),
      /recovery: reclaimed conductor pid \d+ \(lease owner pid \d+ is dead\)/u,
    )
    const recovery = JSON.parse(fs.readFileSync(join(stateDir, 'recoveries.jsonl'), 'utf8').trim())
    assert.equal(recovery.schema, 'angel0-conductor-recovery/v1')
    assert.match(recovery.reason, /dead/u)
    assert.equal(recovery.prior_pid > 0, true)
    assert.equal(recovery.successor_pid > 0, true)

    const resumed = CausalGraph.deserialize(JSON.parse(fs.readFileSync(graphPath, 'utf8')))
    assert.equal(resumed.getNode(agendaId).lastDispatchedAt, dispatchedAt)
    assert.equal(fs.existsSync(join(stateDir, 'lock.lease')), false)
    assert.equal(fs.existsSync(join(stateDir, 'lock')), false)
  } finally {
    if (first?.pid) {
      try {
        process.kill(-first.pid, 'SIGKILL')
      } catch {
        // The fixture may already have exited.
      }
    }
    fs.rmSync(root, { recursive: true, force: true })
  }
})
