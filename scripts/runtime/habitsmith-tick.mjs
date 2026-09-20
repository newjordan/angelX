import { runGraphCli } from './worker-lock.mjs'
import { workerPaths } from './worker-paths.mjs'
// habitsmith-tick — the idle-time habit loop closer (H5 of
// docs/WORKERS.md), sibling of dossier-tick (D4) and reflex-tick (M5):
// same gate stack (kill switch, idle, lockfile, heartbeat), pointed at the
// user's observed workflows.
//
// One headless, cron-driven tick:
//   1. fold the /habits verdict spool (~/.angel0/habitsmith/verdicts.jsonl,
//      written Rust-side) into SUPPORTS/CONTRADICTS 0.9 edges — the graph is
//      Node-owned, so the cockpit never touches it directly;
//   2. re-mine workflow facts from the live ledger (observational refresh —
//      belief keeps tracking whether the habit's steps still pass);
//   3. attach skill-usage telemetry (event:"skill" rows) to the facts that
//      spawned installed skills, via their `fact:` frontmatter;
//   4. compile ~/.angel0/habitsmith/status.json — beliefs + drift flags, the
//      artifact /habits renders alongside the drafts;
//   5. run the propose pass (same daily budget + dedupe as the CLI), so new
//      habits surface as drafts while the box is idle and greet the user via
//      the startup notice.
//
// Kill switch family: ANGEL_HABITS=0 disables the whole loop.

import {
  mineWorkflows,
  ingestWorkflows,
  mineSkillUsage,
  attachSkillUsage,
  foldVerdicts,
  compileHabitsStatus,
  proposeToDisk,
} from './habitsmith.mjs'
import { isIdle, makeHeartbeat, angelTtyRunning } from './idle.mjs'

/** Kill switch: ANGEL_HABITS=0 disables the whole loop. */
export function killed(env = {}) {
  return String(env.ANGEL_HABITS ?? '').trim() === '0'
}

/**
 * The name→factId map for installed habitsmith skills: every SKILL.md in the
 * live dir carrying `fact:` frontmatter. Pure over the supplied reader.
 */
export function factMapFrom(entries) {
  const map = {}
  for (const { name, text } of entries || []) {
    const fact = /^---[\s\S]*?\nfact:\s*([^\n]+)/.exec(text ?? '')?.[1]?.trim()
    const front = (key) =>
      new RegExp(`^---[\\s\\S]*?\\n${key}:\\s*([^\\n]+)`)
        .exec(text ?? '')?.[1]
        ?.trim()
        .replace(/^"|"$/g, '')
    const skill = front('name') || name
    const repo = front('repo_key')
    if (fact) map[repo ? `${repo}\u0000${skill}` : skill] = fact
  }
  return map
}

// ─── CLI — the one tick ──────────────────────────────────────────────────────

async function cli(argv) {
  const fs = await import('./private-store-fs.mjs')
  const { execFileSync } = await import('node:child_process')
  const { fileURLToPath } = await import('node:url')
  const { dirname, join } = await import('node:path')
  const os = await import('node:os')

  const ROOT = join(dirname(fileURLToPath(import.meta.url)), '..', '..')
  const flag = (n, d) => {
    const i = argv.indexOf(n)
    return i >= 0 ? argv[i + 1] : d
  }
  const dryRun = argv.includes('--dry-run')
  const force = argv.includes('--force')
  const onlyRepo = flag('--repo')
  const stateDir = flag('--state-dir', workerPaths().habits)
  const proposedDir = flag('--proposed-dir')
  const liveSkillsDir = flag('--skills-dir', process.env.ANGEL_SKILLS_DIR || workerPaths().skills)
  const graphPath = flag('--graph', workerPaths().graph)
  const ledgerPath = flag('--ledger', process.env.ANGEL_EXPERIENCE_LOG) || workerPaths().ledger
  const idleMs = Number(flag('--idle-min', '15')) * 60_000

  const now = Date.now()
  const nowIso = new Date(now).toISOString()
  fs.mkdirSync(stateDir, { recursive: true })
  const heartbeatPath = join(stateDir, 'heartbeat.json')
  const lockPath = join(stateDir, 'lock')
  const spoolPath = join(stateDir, 'verdicts.jsonl')
  const readJson = (p, d) => {
    try {
      return JSON.parse(fs.readFileSync(p, 'utf8'))
    } catch {
      return d
    }
  }
  const writeHeartbeat = (hb) => fs.writeFileSync(heartbeatPath, JSON.stringify(hb, null, 2))

  // ── gates ──
  const isKilled = killed(process.env)
  const angelTty = angelTtyRunning(execFileSync)
  let newestLedgerMs = null
  try {
    newestLedgerMs = fs.statSync(ledgerPath).mtimeMs
  } catch {
    /* no ledger yet */
  }
  const idle = force || isIdle({ angelTty, newestLedgerMs, now, idleMs })
  const decision = isKilled
    ? { run: false, reason: 'disabled (ANGEL_HABITS=0)' }
    : !idle
      ? { run: false, reason: 'busy — live session or recent ledger activity' }
      : { run: true, reason: 'idle' }

  if (dryRun)
    console.log(
      `habitsmith-tick (dry-run) · ${decision.run ? 'WOULD RUN' : 'WOULD SKIP'} — ${decision.reason}` +
        `\n  gates: killed=${isKilled} idle=${idle} (angelTty=${angelTty})`,
    )
  if (!decision.run) {
    if (!dryRun) {
      writeHeartbeat(makeHeartbeat({ now: nowIso, action: 'skip', reason: decision.reason }))
      console.log(`habitsmith-tick: skip — ${decision.reason}`)
    }
    return 0
  }

  // ── lockfile (the graph file has no locking) ──
  const existingLock = readJson(lockPath, null)
  if (!dryRun && existingLock && now - (existingLock.ts || 0) < 60 * 60_000) {
    console.log('habitsmith-tick: another tick holds the lock; skipping.')
    return 0
  }
  if (!dryRun) fs.writeFileSync(lockPath, JSON.stringify({ pid: process.pid, ts: now }))

  try {
    const CausalGraph = (await import('../../lib/research/CausalGraph.js')).default
    const graph = fs.existsSync(graphPath)
      ? CausalGraph.deserialize(JSON.parse(fs.readFileSync(graphPath, 'utf8')))
      : new CausalGraph()
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

    // 1. Fold the /habits verdict spool.
    const verdicts = fs.existsSync(spoolPath)
      ? fs
          .readFileSync(spoolPath, 'utf8')
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
    const { folded, orphaned } = dryRun
      ? { folded: [], orphaned: [] }
      : foldVerdicts(graph, verdicts, { now: nowIso })
    if (dryRun && verdicts.length) console.log(`  would fold ${verdicts.length} verdict(s)`)

    // 2. Observational refresh from the live ledger.
    const mined = mineWorkflows(rows)
    if (!dryRun)
      for (const [key, rec] of Object.entries(mined)) {
        if (onlyRepo && key !== onlyRepo) continue
        ingestWorkflows(graph, key, rec, { now: nowIso })
      }

    // 3. Skill-usage telemetry onto the spawning facts.
    const entries = []
    try {
      for (const name of fs.readdirSync(liveSkillsDir)) {
        try {
          entries.push({
            name,
            text: fs.readFileSync(join(liveSkillsDir, name, 'SKILL.md'), 'utf8'),
          })
        } catch {
          /* flat file or not a skill folder */
        }
      }
    } catch {
      /* no live skills dir */
    }
    const usage = mineSkillUsage(rows)
    const attached = dryRun ? 0 : attachSkillUsage(graph, usage, factMapFrom(entries))

    if (dryRun) {
      console.log(
        `  would refresh ${Object.keys(mined).length} repo(s), attach usage for ${Object.keys(usage).length} skill(s)`,
      )
      return 0
    }

    // 4. Status artifact for /habits (drift flags live here).
    const status = compileHabitsStatus(graph, { now: nowIso })
    fs.writeFileSync(join(stateDir, 'status.json'), JSON.stringify(status, null, 2))

    // 5. Propose pass (its own daily budget file caps the spam). The live
    // skills dir rides along so an approved skill dedupes against itself.
    const { proposals } = await proposeToDisk(graph, {
      repoKey: onlyRepo,
      proposedDir,
      stateDir,
      skillsDir: liveSkillsDir,
      root: ROOT,
    })

    // 6. Persist the graph, clear the folded spool, settle the books.
    fs.writeFileSync(graphPath, JSON.stringify(graph.serialize(), null, 2))
    if (verdicts.length) fs.writeFileSync(spoolPath, '')
    const drifting = status.facts.filter((f) => f.drifting).length
    writeHeartbeat(
      makeHeartbeat({
        now: nowIso,
        action: 'ran',
        reason:
          `folded ${folded.length} verdict(s) (${orphaned.length} orphaned), ` +
          `${status.facts.length} fact(s) (${drifting} drifting), ` +
          `${attached} usage attach(es), ${proposals.length} new proposal(s)`,
      }),
    )
    console.log(
      `habitsmith-tick: ${folded.length} verdict(s) folded · ${status.facts.length} fact(s), ` +
        `${drifting} drifting · ${proposals.length} proposal(s)`,
    )
    return 0
  } finally {
    if (!dryRun) fs.rmSync(lockPath, { force: true })
  }
}

const isCli =
  process.argv[1] && (await import('node:url')).fileURLToPath(import.meta.url) === process.argv[1]
if (isCli) {
  runGraphCli(cli, process.argv.slice(2)).then((code) => process.exit(code ?? 0))
}
