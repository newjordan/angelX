import { boundedSpawnSync } from './bounded-child.mjs'
import { runGraphCli } from './worker-lock.mjs'
import { workerPaths } from './worker-paths.mjs'
// reflex-tick — the idle-time experimenter (Reflex M5).
//
// One headless, cron-driven tick of the whole loop. When the box is idle and
// within budget, it: refreshes the config hypotheses from the live experience
// ledger (M2), picks the most informative experiment by EIG, runs it as a Control
// Bench A/B (M3), and — if anything newly concluded — rewrites the launch overlay
// (M4). This is the scheduler that ties M2–M4 together and finally builds deli's
// unbuilt external-orchestrator half: an unattended loop with layered guardians
// (kill switch, idle gate, daily budget, lockfile) and a heartbeat watchdog file.
//
// Gates (all must pass to run an experiment):
//   1. kill switch — ANGEL_REFLEX=0 exits immediately.
//   2. idle — no live `angel` TTY session AND the newest experience-ledger line is
//      older than --idle-min (default 15m), so a tick never competes with a user.
//   3. budget — a daily cap on bench runs (~/.angel0/reflex/budget.json;
//      ANGEL_REFLEX_MAX_RUNS_PER_DAY, default 4) so cloud spend stays bounded.
//   4. freshness — the release binary the bench shells must be ≥ the last commit;
//      a stale binary is rebuilt first (a known recurring trap).
//
// Pure decision logic (gates, budget accounting, heartbeat shape) is separated
// from the process/fs/spawn side effects so it is unit-tested without a real tick.

// ─── pure gate + accounting logic ────────────────────────────────────────────

/** Kill switch: ANGEL_REFLEX=0 disables the whole loop. */
export function killed(env = {}) {
  return String(env.ANGEL_REFLEX ?? '').trim() === '0'
}

/**
 * Idle iff no live `angel` TTY session AND the experience ledger has been quiet
 * for at least `idleMs`. `newestLedgerMs` is the newest ledger write (ms), or
 * null when there is no ledger yet (quiet by definition).
 */
export function isIdle({ angelTty, newestLedgerMs, now, idleMs }) {
  if (angelTty) return false
  if (newestLedgerMs == null) return true
  return now - newestLedgerMs >= idleMs
}

/** Normalize the persisted budget for `today`, resetting the counter on a new day. */
export function rollBudget(raw, today, cap) {
  const sameDay = raw && raw.date === today
  return { date: today, runs: sameDay && Number.isFinite(raw.runs) ? raw.runs : 0, cap }
}

export function budgetAllows(budget) {
  return budget.runs < budget.cap
}

export function recordRun(budget) {
  return { ...budget, runs: budget.runs + 1 }
}

/**
 * Environment for a cockpit rebuild that bakes the source digest in, so the
 * rebuilt binary's `--build-info --json` reports a real cockpit_source_sha256
 * (sha256 of `git archive HEAD`, see scripts/check/cockpit-source-digest.sh) rather
 * than "unbound". A dirty cockpit tree (or a missing helper) builds unbound and
 * says so in one line — the same rule bin/angel0 applies.
 */
export function sourceBoundEnv(root, spawnSync, tag) {
  const env = { ...process.env }
  if (env.ANGEL_BUILD_SOURCE_SHA256) return env
  delete env.ANGEL_BUILD_SOURCE_SHA256
  if (env.ANGEL_BIND_SOURCE === '0') return env
  const helper = `${root}/scripts/check/cockpit-source-digest.sh`
  const r = spawnSync('bash', [helper], { encoding: 'utf8' })
  if (r.status === 0 && /^[0-9a-f]{64}\s*$/.test(r.stdout || '')) {
    env.ANGEL_BUILD_SOURCE_SHA256 = r.stdout.trim()
    console.log(`${tag}: source-bound build ${env.ANGEL_BUILD_SOURCE_SHA256.slice(0, 12)}…`)
  } else {
    const why = (r.stderr || r.error?.message || 'source digest unavailable').trim().split('\n')[0]
    console.log(`${tag}: ${why} — building unbound`)
  }
  return env
}

/** A stale release binary: missing, or older than the last commit it should reflect. */
export function binaryStale(binMtimeMs, lastCommitMs) {
  if (binMtimeMs == null) return true
  if (!Number.isFinite(lastCommitMs)) return false
  return binMtimeMs < lastCommitMs
}

/** Top-level gate decision from the three boolean gates. */
export function decideTick({ isKilled, idle, budgetOk }) {
  if (isKilled) return { run: false, reason: 'disabled (ANGEL_REFLEX=0)' }
  if (!idle) return { run: false, reason: 'busy — live session or recent ledger activity' }
  if (!budgetOk) return { run: false, reason: 'daily bench budget exhausted' }
  return { run: true, reason: 'idle and within budget' }
}

/** The heartbeat.json body the deli watchdog polls. */
export function makeHeartbeat({ now, action, reason, budget, experiment = null }) {
  return {
    v: 1,
    ts: now,
    action, // 'ran' | 'skip' | 'error'
    reason,
    budget,
    experiment, // {knob,value,verdict,delta} | null
  }
}

/** One `bench` record for the experience ledger — the interventional trace M2 mines. */
export function benchRecord({ now, suite, knob, value, verification, ab }) {
  return {
    kind: 'bench',
    v: 1,
    ts: now,
    suite,
    knob,
    value,
    verdict: verification?.verdict ?? null,
    confidence: verification?.confidence ?? null,
    baseline: ab?.baseline?.accuracy_pct ?? null,
    treatment: ab?.treatment?.accuracy_pct ?? null,
    delta:
      ab?.treatment?.accuracy_pct != null && ab?.baseline?.accuracy_pct != null
        ? ab.treatment.accuracy_pct - ab.baseline.accuracy_pct
        : null,
  }
}

// ─── CLI — the one tick ──────────────────────────────────────────────────────

async function cli(argv) {
  const fs = await import('./private-store-fs.mjs')
  const { execFileSync, spawnSync } = await import('node:child_process')
  const { fileURLToPath } = await import('node:url')
  const { dirname, join } = await import('node:path')
  const os = await import('node:os')

  const ROOT = join(dirname(fileURLToPath(import.meta.url)), '..', '..')
  const flag = (n, d) => {
    const i = argv.indexOf(n)
    return i >= 0 ? argv[i + 1] : d
  }
  const has = (n) => argv.includes(n)

  const dryRun = has('--dry-run')
  const force = has('--force') // bypass the idle gate (testing / manual)
  const noBuild = has('--no-build')
  const stateDir = flag('--state-dir', workerPaths().reflex)
  const graphPath = flag('--graph', workerPaths().graph)
  const ledgerPath = flag('--ledger', process.env.ANGEL_EXPERIENCE_LOG) || workerPaths().ledger
  const idleMs = Number(flag('--idle-min', '15')) * 60_000
  const cap = Number(flag('--cap', process.env.ANGEL_REFLEX_MAX_RUNS_PER_DAY || '4'))
  const seeds = flag('--seeds', '2')
  const suite = flag('--suite', process.env.ANGEL_REFLEX_SUITE || 'reflex')

  const now = Date.now()
  const nowIso = () => new Date(now).toISOString()
  const today = new Date(now).toISOString().slice(0, 10)
  fs.mkdirSync(stateDir, { recursive: true })
  const heartbeatPath = join(stateDir, 'heartbeat.json')
  const budgetPath = join(stateDir, 'budget.json')
  const lockPath = join(stateDir, 'lock')

  const readJson = (p, d) => {
    try {
      return JSON.parse(fs.readFileSync(p, 'utf8'))
    } catch {
      return d
    }
  }
  const writeHeartbeat = (hb) => fs.writeFileSync(heartbeatPath, JSON.stringify(hb, null, 2))

  // ── gate 1: kill switch ──
  const isKilled = killed(process.env)

  // ── gate 2: idle (--force bypasses the whole gate: process AND ledger) ──
  const angelTty = angelTtyRunning(execFileSync)
  let newestLedgerMs = null
  try {
    newestLedgerMs = fs.statSync(ledgerPath).mtimeMs
  } catch {
    /* no ledger yet */
  }
  const idle = force || isIdle({ angelTty, newestLedgerMs, now, idleMs })

  // ── gate 3: budget ──
  const budget = rollBudget(readJson(budgetPath, null), today, cap)

  const decision = decideTick({ isKilled, idle, budgetOk: budgetAllows(budget) })

  if (dryRun) {
    console.log(
      `reflex-tick (dry-run) · ${decision.run ? 'WOULD RUN' : 'WOULD SKIP'} — ${decision.reason}`,
    )
    console.log(
      `  gates: killed=${isKilled} idle=${idle} (angelTty=${angelTty}) budget=${budget.runs}/${budget.cap}`,
    )
    if (decision.run) {
      const target = await topExperiment(ROOT, graphPath, ledgerPath)
      console.log(
        `  next experiment: ${target ? `${target.knob}=${target.value} (${target.label})` : '(none)'}`,
      )
    }
    return 0
  }

  if (!decision.run) {
    writeHeartbeat(
      makeHeartbeat({ now: nowIso(), action: 'skip', reason: decision.reason, budget }),
    )
    console.log(`reflex-tick: skip — ${decision.reason}`)
    return 0
  }

  // ── lockfile: never let two ticks run at once (the graph has no locking) ──
  const existingLock = readJson(lockPath, null)
  if (existingLock && now - (existingLock.ts || 0) < 60 * 60_000) {
    console.log('reflex-tick: another tick holds the lock; skipping.')
    return 0
  }
  fs.writeFileSync(lockPath, JSON.stringify({ pid: process.pid, ts: now }))

  try {
    if (!process.env.ANGEL_BENCHMARK_COMMAND)
      throw new Error('ANGEL_BENCHMARK_COMMAND is required for a measured config experiment')

    // ── gate 4: freshness — rebuild a stale release binary before benching ──
    if (!noBuild) {
      const binPath = join(ROOT, 'cockpit/target/release/angel')
      let binMtime = null
      try {
        binMtime = fs.statSync(binPath).mtimeMs
      } catch {
        /* missing */
      }
      let lastCommitMs = NaN
      try {
        lastCommitMs =
          Number(
            execFileSync('git', ['-C', ROOT, 'log', '-1', '--format=%ct'], {
              encoding: 'utf8',
            }).trim(),
          ) * 1000
      } catch {
        /* not a git repo? leave NaN → not stale */
      }
      if (binaryStale(binMtime, lastCommitMs)) {
        console.log('reflex-tick: release binary is stale — rebuilding…')
        const r = boundedSpawnSync(
          'cargo',
          ['build', '--locked', '--release', '--no-default-features'],
          {
            cwd: join(ROOT, 'cockpit'),
            stdio: 'inherit',
            env: sourceBoundEnv(ROOT, spawnSync, 'reflex-tick'),
          },
        )
        if (r.status !== 0) {
          throw new Error('cargo build --release failed; aborting tick')
        }
      }
    }

    // ── the experiment ──
    const target = await topExperiment(ROOT, graphPath, ledgerPath)
    if (!target) {
      writeHeartbeat(
        makeHeartbeat({ now: nowIso(), action: 'skip', reason: 'no open experiment', budget }),
      )
      console.log('reflex-tick: no open experiment to run.')
      return 0
    }
    console.log(`reflex-tick: running ${target.knob}=${target.value} on ${suite}…`)
    const concludedBefore = await concludedCount(graphPath)
    const runArgs = [
      join(ROOT, 'scripts/runtime/reflex-run-experiment.mjs'),
      '--knob',
      target.knob,
      '--value',
      target.value,
      '--suite',
      suite,
      '--seeds',
      String(seeds),
      '--graph',
      graphPath,
      '--ledger',
      ledgerPath,
    ]
    const nextBudget = recordRun(budget)
    fs.writeFileSync(budgetPath, JSON.stringify(nextBudget, null, 2))
    const run = boundedSpawnSync(process.execPath, runArgs, {
      cwd: ROOT,
      stdio: 'inherit',
      timeout: 660_000,
    })
    const ranOk = run.status === 0

    // Only rewrite the overlay when this experiment actually CONCLUDED something
    // new — an inconclusive run leaves the config untouched (no needless churn).
    let reconfigured = false
    if (ranOk && (await concludedCount(graphPath)) > concludedBefore) {
      const applied = boundedSpawnSync(
        process.execPath,
        [join(ROOT, 'scripts/runtime/reflex-reconfigure.mjs'), '--apply', '--graph', graphPath],
        {
          cwd: ROOT,
          stdio: 'inherit',
        },
      )
      if (applied.status !== 0)
        throw new Error('verified experiment completed but overlay application failed')
      reconfigured = true
    }

    // Budget + heartbeat update (a run counts against the daily cap either way).
    // Append a bench record to the experience ledger (M2 mines these next round).
    try {
      fs.mkdirSync(dirname(ledgerPath), { recursive: true })
      fs.appendFileSync(
        ledgerPath,
        JSON.stringify(
          benchRecord({
            now: Math.floor(now / 1000),
            suite,
            knob: target.knob,
            value: target.value,
            verification: null,
            ab: null,
          }),
        ) + '\n',
      )
    } catch {
      /* best-effort */
    }
    writeHeartbeat(
      makeHeartbeat({
        now: nowIso(),
        action: ranOk ? 'ran' : 'error',
        reason: ranOk ? 'experiment complete' : `runner exited ${run.status}`,
        budget: nextBudget,
        experiment: { knob: target.knob, value: target.value, suite, reconfigured },
      }),
    )
    console.log(
      `reflex-tick: done · budget ${nextBudget.runs}/${nextBudget.cap} · heartbeat → ${heartbeatPath}`,
    )
    return ranOk ? 0 : 1
  } finally {
    try {
      fs.unlinkSync(lockPath)
    } catch {
      /* already gone */
    }
  }
}

/** Is a live, interactive `angel` process running (one with a real controlling TTY)? */
export function angelTtyRunning(execFileSync) {
  try {
    const out = execFileSync('pgrep', ['-x', 'angel'], { encoding: 'utf8' }).trim()
    if (!out) return false
    for (const pid of out.split('\n').filter(Boolean)) {
      try {
        const tty = execFileSync('ps', ['-o', 'tty=', '-p', pid], { encoding: 'utf8' }).trim()
        if (tty && tty !== '?') return true // a real pts/N → interactive session
      } catch {
        /* pid vanished */
      }
    }
    return false
  } catch {
    return false // pgrep exits non-zero when nothing matches
  }
}

/** Refresh config hypotheses from the live ledger and return the top EIG target. */
async function topExperiment(ROOT, graphPath, ledgerPath) {
  const fs = await import('./private-store-fs.mjs')
  const CausalGraph = (await import('../../lib/research/CausalGraph.js')).default
  const {
    seedConfigHypotheses,
    ingestLedgerObservations,
    proposeConfigExperiment,
    configHypotheses,
  } = await import('./config-causal.mjs')
  const graph = fs.existsSync(graphPath)
    ? CausalGraph.deserialize(JSON.parse(fs.readFileSync(graphPath, 'utf8')))
    : new CausalGraph()
  if (configHypotheses(graph).length === 0) seedConfigHypotheses(graph)
  const rows = fs.existsSync(ledgerPath)
    ? fs
        .readFileSync(ledgerPath, 'utf8')
        .split('\n')
        .filter((l) => l.trim())
        .map((l) => {
          try {
            return JSON.parse(l)
          } catch {
            return null
          }
        })
        .filter(Boolean)
    : []
  ingestLedgerObservations(graph, rows)
  const { proposal } = proposeConfigExperiment(graph, { limit: 1 })
  return proposal || null
}

/** Count concluded reflex hypotheses in the on-disk graph (before/after an experiment). */
async function concludedCount(graphPath) {
  const fs = await import('./private-store-fs.mjs')
  if (!fs.existsSync(graphPath)) return 0
  try {
    const data = JSON.parse(fs.readFileSync(graphPath, 'utf8'))
    return (data.nodes || []).filter((n) => n.projectId === 'reflex' && n.status === 'concluded')
      .length
  } catch {
    return 0
  }
}

const isCli =
  process.argv[1] && (await import('node:url')).fileURLToPath(import.meta.url) === process.argv[1]
if (isCli) {
  runGraphCli(cli, process.argv.slice(2)).then((code) => process.exit(code ?? 0))
}
