import { runGraphCli, tickLockHolder, tickLockOwner } from './worker-lock.mjs'
import { workerPaths } from './worker-paths.mjs'

import {
  mineWorkflows,
  ingestWorkflows,
  mineSkillUsage,
  attachSkillUsage,
  foldVerdicts,
  compileHabitsStatus,
} from '../../lib/habits/core.mjs'
import { proposeToDisk } from './habitsmith.mjs'
import { isIdle, makeHeartbeat, angelTtyRunning } from './idle.mjs'
import { existsSync, realpathSync } from 'node:fs'
import { pathToFileURL } from 'node:url'

export function killed(env = {}) {
  return String(env.ANGEL_HABITS ?? '').trim() === '0'
}

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

async function cli(argv, selectedGraph) {
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
  const graphPath = selectedGraph ?? flag('--graph', workerPaths().graph)
  const ledgerPath = flag('--ledger', process.env.ANGEL_EXPERIENCE_LOG) || workerPaths().ledger
  const idleMs = Number(flag('--idle-min', '15')) * 60_000

  const now = Date.now()
  const nowIso = new Date(now).toISOString()
  if (!dryRun) fs.mkdirSync(stateDir, { recursive: true })
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

  const isKilled = killed(process.env)
  const angelTty = angelTtyRunning(execFileSync)
  let newestLedgerMs = null
  try {
    newestLedgerMs = fs.statSync(ledgerPath).mtimeMs
  } catch {
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

  const holder = dryRun ? null : tickLockHolder(readJson(lockPath, null), now)
  if (holder) {
    writeHeartbeat(
      makeHeartbeat({
        now: nowIso,
        action: 'skip',
        reason: `another tick holds the lock — ${holder.reason}`,
      }),
    )
    console.log('habitsmith-tick: another tick holds the lock; skipping.')
    return 0
  }
  if (!dryRun) fs.writeFileSync(lockPath, JSON.stringify(tickLockOwner(now)))

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

    const mined = mineWorkflows(rows)
    if (!dryRun)
      for (const [key, rec] of Object.entries(mined)) {
        if (onlyRepo && key !== onlyRepo) continue
        ingestWorkflows(graph, key, rec, { now: nowIso })
      }

    const entries = []
    try {
      for (const name of fs.readdirSync(liveSkillsDir)) {
        try {
          entries.push({
            name,
            text: fs.readFileSync(join(liveSkillsDir, name, 'SKILL.md'), 'utf8'),
          })
        } catch {
        }
      }
    } catch {
    }
    const usage = mineSkillUsage(rows)
    const attached = dryRun ? 0 : attachSkillUsage(graph, usage, factMapFrom(entries))

    if (dryRun) {
      console.log(
        `  would refresh ${Object.keys(mined).length} repo(s), attach usage for ${Object.keys(usage).length} skill(s)`,
      )
      return 0
    }

    const status = compileHabitsStatus(graph, { now: nowIso })
    fs.writeFileSync(join(stateDir, 'status.json'), JSON.stringify(status, null, 2))

    const { proposals } = await proposeToDisk(graph, {
      repoKey: onlyRepo,
      proposedDir,
      stateDir,
      skillsDir: liveSkillsDir,
      root: ROOT,
    })

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
  process.argv[1] &&
  existsSync(process.argv[1]) &&
  import.meta.url === pathToFileURL(realpathSync(process.argv[1])).href
if (isCli) {
  runGraphCli(cli, process.argv.slice(2)).then((code) => process.exit(code ?? 0))
}
