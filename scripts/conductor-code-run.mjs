import { boundedSpawnSync } from './bounded-child.mjs'
import { runGraphCli } from './worker-lock.mjs'
import { workerPaths } from './worker-paths.mjs'
// conductor-code-run — C4 code rung for the Conductor.
//
// This imitates `/self` from outside the cockpit: create a parked git worktree,
// run bounded `angel --task` iterations inside it, accept only a Layer-2
// build+test+baseline gate, commit the green branch, and append it to
// ~/.angel0/conductor/queue.json for a human verdict. It never merges.

import { agendaNodeId } from './conductor.mjs'

export const DEFAULT_MAX_ITERS = 4
export const DEFAULT_TASK_DEADLINE_SECS = 900
export const DEFAULT_RUN_DEADLINE_SECS = 3600

const slug = (s) =>
  String(s ?? '')
    .toLowerCase()
    .replace(/[^0-9a-z]+/g, '-')
    .replace(/^-+|-+$/g, '')
    .slice(0, 40) || 'item'

const stamp = (d = new Date()) =>
  d
    .toISOString()
    .replace(/[^0-9]/g, '')
    .slice(0, 14)

export function saysDone(text, sentinel = 'CONDUCTOR_DONE') {
  const compact = sentinel.replace(/_/g, '')
  return String(text ?? '')
    .split(/\r?\n/)
    .some((line) => {
      const t = line.trim().replace(/^[^A-Za-z0-9_]+|[^A-Za-z0-9_]+$/g, '')
      return t.toUpperCase() === sentinel || t.replace(/_/g, '').toUpperCase() === compact
    })
}

export function parseTestResult(output) {
  const out = { passed: 0, failed: 0, ignored: 0 }
  for (const line of String(output ?? '').split(/\r?\n/)) {
    if (!line.trimStart().startsWith('test result:')) continue
    const toks = line.split(/;|\s+/).filter(Boolean)
    for (let i = 0; i < toks.length - 1; i++) {
      const n = Number(toks[i])
      if (!Number.isInteger(n)) continue
      if (toks[i + 1] === 'passed') out.passed += n
      else if (toks[i + 1] === 'failed') out.failed += n
      else if (toks[i + 1] === 'ignored') out.ignored += n
    }
  }
  return out
}

const ran = (t) => (Number(t?.passed) || 0) + (Number(t?.failed) || 0)

export function evaluateGate({ buildOk, tests, baselinePassed = null }) {
  if (!buildOk) {
    return { passed: false, summary: 'REJECTED - the crate does not build' }
  }
  if (ran(tests) === 0) {
    return { passed: false, summary: 'REJECTED - builds but no tests ran' }
  }
  if ((Number(tests.failed) || 0) > 0) {
    return {
      passed: false,
      summary: `REJECTED - builds but ${tests.failed} of ${ran(tests)} tests failed`,
    }
  }
  if (baselinePassed != null && tests.passed < baselinePassed) {
    return {
      passed: false,
      summary: `REJECTED - green, but ${tests.passed} passed < baseline ${baselinePassed}`,
    }
  }
  return { passed: true, summary: `GREEN - builds; ${tests.passed} tests pass` }
}

function tail(s, n = 4000) {
  s = String(s ?? '')
  return s.length <= n ? s : s.slice(-n)
}

function readJson(fs, path, fallback) {
  try {
    return JSON.parse(fs.readFileSync(path, 'utf8'))
  } catch {
    return fallback
  }
}

function writeJson(fs, path, value) {
  fs.writeFileSync(path, JSON.stringify(value, null, 2))
}

function appendJsonl(fs, path, value) {
  fs.appendFileSync(path, JSON.stringify(value) + '\n')
}

function normalizeFinding(s) {
  return String(s ?? '')
    .toLowerCase()
    .replace(/\s+/g, ' ')
    .trim()
}

function persistState(fs, runDir, progress, findings, log) {
  const dir = `${runDir}/state`
  fs.mkdirSync(dir, { recursive: true })
  writeJson(fs, `${dir}/progress.json`, {
    status: progress.status,
    iteration: progress.iteration,
    max_iters: progress.maxIters,
    total_findings: findings.length,
    stale_count: progress.staleCount,
    deadline_secs: progress.deadlineSecs,
    run_elapsed_secs: Math.floor((Date.now() - progress.startedMs) / 1000),
    heartbeat_ms: Date.now(),
    pid: process.pid,
  })
  fs.writeFileSync(
    `${dir}/findings.jsonl`,
    findings.map((finding) => JSON.stringify({ finding })).join('\n'),
  )
  fs.writeFileSync(`${dir}/iteration_log.jsonl`, log.map((l) => JSON.stringify(l)).join('\n'))
}

function promptFor({ goal, findings, directionsTried, staleCount }) {
  const parts = [
    'You are running in an isolated Conductor worktree.',
    'Make the requested code improvement only in this workspace.',
    'When the work is ready for the external build+test gate, end with a line containing exactly CONDUCTOR_DONE.',
    '',
    `Goal: ${goal}`,
  ]
  if (findings.length) {
    parts.push('', 'Findings so far:', ...findings.map((f) => `- ${f}`))
  }
  if (directionsTried.length) {
    parts.push('', 'Directions tried:', ...directionsTried.map((d) => `- ${d}`))
  }
  if (staleCount >= 2) {
    parts.push('', 'The last two iterations were stale. Make a structural pivot now.')
  }
  return parts.join('\n') + '\n'
}

async function loadGraph(fs, graphPath) {
  const CausalGraph = (await import('../lib/research/CausalGraph.js')).default
  return fs.existsSync(graphPath)
    ? CausalGraph.deserialize(JSON.parse(fs.readFileSync(graphPath, 'utf8')))
    : new CausalGraph()
}

function resolveGoal(graph, agendaId, fallbackGoal) {
  if (fallbackGoal) return fallbackGoal
  const node = agendaId ? graph.getNode(agendaId) : null
  return node?.goal || node?.label || null
}

function runCargo(spawnSync, args, cwd, extra = {}) {
  const res = spawnSync('cargo', args, { cwd, encoding: 'utf8', ...extra })
  const stdout = res.stdout || ''
  const stderr = res.stderr || ''
  return {
    status: res.status,
    ok: res.status === 0,
    stdout,
    stderr,
    combined: stdout + '\n' + stderr,
  }
}

export function runGate(spawnSync, worktree, baselinePassed) {
  const build = runCargo(spawnSync, ['build'], worktree)
  if (!build.ok) {
    return {
      passed: false,
      build: false,
      tests: { passed: 0, failed: 0, ignored: 0 },
      baseline: baselinePassed,
      summary: evaluateGate({ buildOk: false, tests: {}, baselinePassed }).summary,
      output: tail(build.combined),
    }
  }

  let test = runCargo(spawnSync, ['test'], worktree)
  let tests = parseTestResult(test.combined)
  if (!test.ok && /pxpipe/i.test(test.combined)) {
    const rerun = runCargo(spawnSync, ['test', '--', '--test-threads=1'], worktree)
    if (rerun.ok || parseTestResult(rerun.combined).failed <= tests.failed) {
      test = rerun
      tests = parseTestResult(test.combined)
    }
  }
  const verdict = evaluateGate({ buildOk: true, tests, baselinePassed })
  return {
    passed: test.ok && verdict.passed,
    build: true,
    tests,
    baseline: baselinePassed,
    summary: verdict.summary,
    output: tail(test.combined),
  }
}

function captureBaseline(spawnSync, worktree) {
  const test = runCargo(spawnSync, ['test'], worktree)
  const parsed = parseTestResult(test.combined)
  if (!test.ok || ran(parsed) === 0 || parsed.failed > 0) {
    throw new Error(
      `baseline cargo test is not green (${parsed.passed} passed, ${parsed.failed} failed)`,
    )
  }
  return parsed.passed
}

function diffstat(execFileSync, worktree) {
  try {
    return execFileSync('git', ['-C', worktree, 'diff', '--stat', 'HEAD'], {
      encoding: 'utf8',
    }).trim()
  } catch {
    return ''
  }
}

function hasChanges(execFileSync, worktree) {
  try {
    return (
      execFileSync(
        'git',
        ['-C', worktree, 'status', '--porcelain', '--', '.', ':(exclude)target'],
        {
          encoding: 'utf8',
        },
      ).trim() !== ''
    )
  } catch {
    return false
  }
}

function git(execFileSync, args, cwd) {
  return execFileSync('git', args, { cwd, encoding: 'utf8' })
}

function commitWorktree(execFileSync, worktree, message) {
  git(execFileSync, ['-C', worktree, 'add', '-A', '--', '.', ':(exclude)target'], worktree)
  git(
    execFileSync,
    [
      '-C',
      worktree,
      '-c',
      'user.name=angel-conductor',
      '-c',
      'user.email=conductor@local',
      'commit',
      '-m',
      message,
    ],
    worktree,
  )
}

function appendQueue(fs, queuePath, item) {
  const queue = readJson(fs, queuePath, [])
  const arr = Array.isArray(queue) ? queue : Array.isArray(queue.items) ? queue.items : []
  arr.push(item)
  writeJson(fs, queuePath, arr)
}

function noteFailure(graph, agendaId, reason, now) {
  if (!agendaId || !graph.hasNode(agendaId)) return
  graph.updateNode(agendaId, {
    lastCodeFailureAt: now,
    codeFailure: { ts: now, reason },
  })
}

function cleanupWorktree(execFileSync, srcRoot, branch, worktree) {
  try {
    git(execFileSync, ['worktree', 'remove', '--force', worktree], srcRoot)
  } catch {
    /* best effort */
  }
  try {
    git(execFileSync, ['branch', '-D', branch], srcRoot)
  } catch {
    /* best effort */
  }
}

async function cli(argv) {
  const fs = await import('./private-store-fs.mjs')
  const { execFileSync, spawnSync } = await import('node:child_process')
  const { fileURLToPath } = await import('node:url')
  const { dirname, join, relative, resolve } = await import('node:path')
  const os = await import('node:os')

  const ROOT = join(dirname(fileURLToPath(import.meta.url)), '..')
  const flag = (n, d) => {
    const i = argv.indexOf(n)
    return i >= 0 ? argv[i + 1] : d
  }
  const agendaId = flag('--agenda')
  const explicitGoal = flag('--goal')
  const graphPath = flag('--graph', workerPaths().graph)
  const stateDir = flag('--state-dir', process.env.ANGEL_CONDUCTOR_DIR || workerPaths().conductor)
  const srcRoot = resolve(flag('--src', process.env.ANGEL_SELF_SRC || join(ROOT, 'cockpit')))
  const angelBin = flag(
    '--angel-bin',
    process.env.ANGEL_BIN || join(srcRoot, 'target/release/angel'),
  )
  const maxIters = Number(flag('--max-iters', process.env.ANGEL_CONDUCTOR_MAX_ITERS || '4'))
  const taskDeadline = Number(
    flag('--task-deadline', process.env.ANGEL_CONDUCTOR_TASK_DEADLINE || '900'),
  )
  const runDeadline = Number(
    flag('--run-deadline', process.env.ANGEL_CONDUCTOR_RUN_DEADLINE_SECS || '3600'),
  )
  for (const [name, value] of Object.entries({ maxIters, taskDeadline, runDeadline })) {
    if (!Number.isSafeInteger(value) || value <= 0)
      throw new Error(`${name} must be a positive integer`)
  }
  const driver = String(process.env.ANGEL_CONDUCTOR_DRIVER ?? '').trim()
  if (!driver) {
    console.error('conductor-code-run: ANGEL_CONDUCTOR_DRIVER is required for code dispatch')
    return 2
  }

  fs.mkdirSync(stateDir, { recursive: true })
  const graph = await loadGraph(fs, graphPath)
  const goal = resolveGoal(graph, agendaId, explicitGoal)
  if (!goal) {
    console.error('conductor-code-run: --goal or --agenda pointing at a graph node is required')
    return 2
  }

  const gitRoot = git(execFileSync, ['rev-parse', '--show-toplevel'], srcRoot).trim()
  const rel = relative(gitRoot, srcRoot)
  const now = new Date().toISOString()
  const runStamp = stamp(new Date())
  const itemSlug = slug(goal)
  const branch = `angel/conductor-${runStamp}-${itemSlug}`
  const worktreeTop = join(stateDir, 'worktrees', runStamp)
  const worktree = rel ? join(worktreeTop, rel) : worktreeTop
  const queuePath = join(stateDir, 'queue.json')
  const runDir = join(stateDir, 'runs', runStamp)
  const startedMs = Date.now()
  const deadlineMs = startedMs + runDeadline * 1000
  const runChild = (command, args, options = {}) => {
    const remaining = deadlineMs - Date.now()
    if (remaining <= 0)
      return {
        status: 124,
        stdout: '',
        stderr: 'run deadline exceeded',
        error: Object.assign(new Error('run deadline exceeded'), { code: 'ETIMEDOUT' }),
      }
    return boundedSpawnSync(command, args, {
      ...options,
      timeout: Math.min(options.timeout || remaining, remaining),
    })
  }
  const findings = []
  const seenFindings = new Set()
  const directionsTried = []
  const log = []
  let staleCount = 0
  let lastFailure = 'not started'

  const remember = (finding) => {
    const norm = normalizeFinding(finding)
    if (!norm || seenFindings.has(norm)) return false
    seenFindings.add(norm)
    findings.push(finding)
    return true
  }
  const progress = (status, iteration = 0) =>
    persistState(
      fs,
      runDir,
      {
        status,
        iteration,
        maxIters,
        staleCount,
        deadlineSecs: runDeadline,
        startedMs,
      },
      findings,
      log,
    )

  try {
    fs.mkdirSync(join(stateDir, 'worktrees'), { recursive: true })
    git(execFileSync, ['worktree', 'add', '-b', branch, worktreeTop, 'HEAD'], gitRoot)
    progress('baseline')
    const baseline = captureBaseline(runChild, worktree)
    remember(`baseline: ${baseline} passing test(s)`)
    progress('running')

    for (let iteration = 1; iteration <= maxIters; iteration++) {
      if (Date.now() >= deadlineMs) {
        lastFailure = `run deadline ${runDeadline}s exceeded`
        break
      }
      const prompt = promptFor({ goal, findings, directionsTried, staleCount })
      const remainingMs = Math.max(1, deadlineMs - Date.now())
      const timeout = Math.min(taskDeadline * 1000, remainingMs)
      const task = runChild(
        angelBin,
        [
          '--task',
          '--workspace',
          worktree,
          '--max-hops',
          '40',
          '--deadline-secs',
          String(Math.ceil(timeout / 1000)),
          '-',
        ],
        {
          cwd: srcRoot,
          input: prompt,
          encoding: 'utf8',
          timeout,
          env: {
            ...process.env,
            ANGEL_DRIVER: driver,
            ANGEL_MAX_HOPS: '40',
            ANGEL_TURN_DEADLINE_SECS: String(taskDeadline),
          },
        },
      )
      const output = `${task.stdout || ''}\n${task.stderr || ''}`
      const direction = `iteration ${iteration}`
      directionsTried.push(direction)
      const done = saysDone(output)
      let gate = null
      let newFinding = false

      if (task.error?.code === 'ETIMEDOUT') {
        lastFailure = 'model task timed out'
        newFinding = remember(lastFailure)
      } else if (task.status !== 0) {
        lastFailure = `model task failed (${task.status ?? task.error?.code ?? 'spawn'})`
        newFinding = remember(lastFailure)
      } else if (!done) {
        lastFailure = 'model task did not claim CONDUCTOR_DONE'
        newFinding = remember(lastFailure)
      } else {
        gate = runGate(runChild, worktree, baseline)
        lastFailure = gate.summary
        newFinding = remember(gate.summary)
        if (gate.passed) {
          if (!hasChanges(execFileSync, worktree)) {
            lastFailure = 'green gate but no worktree changes'
            remember(lastFailure)
          } else {
            const stat = diffstat(execFileSync, worktree)
            commitWorktree(execFileSync, worktree, `conductor: ${itemSlug}`)
            const id = `conductor-${runStamp}-${itemSlug}`
            const item = {
              id,
              branch,
              goal,
              agendaNode: agendaId || null,
              gate: {
                build: true,
                tests: gate.tests,
                baseline,
                passed: true,
                summary: gate.summary,
              },
              diffstat: stat,
              worktree,
              worktreeTop,
              ts: now,
            }
            appendQueue(fs, queuePath, item)
            appendJsonl(fs, join(stateDir, 'runs.jsonl'), {
              ts: now,
              id,
              action: 'parked',
              branch,
              agendaNode: agendaId || null,
              gate: item.gate,
            })
            progress('parked', iteration)
            console.log(`conductor-code-run: parked ${branch} (${id})`)
            return 0
          }
        }
      }

      staleCount = newFinding ? 0 : staleCount + 1
      log.push({
        iteration,
        direction,
        new_findings: newFinding ? 1 : 0,
        stale_count: staleCount,
        done,
        gate: gate?.summary ?? null,
        ts_ms: Date.now(),
      })
      progress('running', iteration)
      if (staleCount >= 2) {
        lastFailure = `repeated result without progress: ${lastFailure}`
        break
      }
    }

    noteFailure(graph, agendaId, lastFailure, new Date().toISOString())
    if (agendaId) writeJson(fs, graphPath, graph.serialize())
    appendJsonl(fs, join(stateDir, 'runs.jsonl'), {
      ts: new Date().toISOString(),
      action: 'rejected',
      agendaNode: agendaId || null,
      reason: lastFailure,
      baseline,
    })
    progress('rejected', maxIters)
    cleanupWorktree(execFileSync, gitRoot, branch, worktreeTop)
    console.error(`conductor-code-run: rejected - ${lastFailure}`)
    return 1
  } catch (err) {
    noteFailure(graph, agendaId, err?.message || String(err), new Date().toISOString())
    if (agendaId) writeJson(fs, graphPath, graph.serialize())
    progress('error')
    cleanupWorktree(execFileSync, gitRoot, branch, worktreeTop)
    console.error(`conductor-code-run: error - ${err?.message || err}`)
    return 1
  }
}

const { pathToFileURL } = await import('node:url')
const { resolve: resolvePath } = await import('node:path')
const isCli =
  !process.env.NODE_TEST_CONTEXT &&
  process.argv[1] &&
  pathToFileURL(resolvePath(process.argv[1])).href === import.meta.url
if (isCli) {
  runGraphCli(cli, process.argv.slice(2)).then((code) => process.exit(code ?? 0))
}

// Exported for tests.
export const _private = { promptFor, persistState, agendaNodeId }
