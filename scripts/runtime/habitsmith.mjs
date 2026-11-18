import { runGraphCli } from './worker-lock.mjs'
import { workerPaths } from './worker-paths.mjs'
import { existsSync, realpathSync } from 'node:fs'
import { pathToFileURL } from 'node:url'
import {
  DEFAULT_MIN_BELIEF,
  DEFAULT_MAX_PROPOSALS_PER_DAY,
  extractSkillSteps,
  proposeSkills,
  mineWorkflows,
  ingestWorkflows,
} from '../../lib/habits/core.mjs'

export * from '../../lib/habits/core.mjs'

export async function proposeToDisk(graph, opts = {}) {
  const fs = await import('./private-store-fs.mjs')
  const { join, dirname } = await import('node:path')
  const { fileURLToPath } = await import('node:url')
  const os = await import('node:os')
  const home = os.homedir()
  const ROOT = opts.root ?? join(dirname(fileURLToPath(import.meta.url)), '..', '..')
  const proposedDir =
    opts.proposedDir || process.env.ANGEL_HABIT_PROPOSED_DIR || workerPaths().proposed
  const userSkillsDir = opts.skillsDir || process.env.ANGEL_SKILLS_DIR || workerPaths().skills
  const bundledDir = process.env.ANGEL_BUNDLED_SKILLS_DIR || join(ROOT, 'cockpit/skills')
  const stateDir = opts.stateDir || workerPaths().habits
  const minBelief = Number(process.env.ANGEL_HABIT_MIN_BELIEF || DEFAULT_MIN_BELIEF)
  const cap = Number(process.env.ANGEL_HABIT_MAX_PROPOSALS_PER_DAY || DEFAULT_MAX_PROPOSALS_PER_DAY)
  const log = opts.log ?? console.log
  const dryRun = !!opts.dryRun

  const existing = []
  for (const dir of [bundledDir, userSkillsDir, proposedDir]) {
    let entries
    try {
      entries = fs.readdirSync(dir, { withFileTypes: true })
    } catch {
      continue
    }
    for (const e of entries) {
      const md = e.isDirectory() ? join(dir, e.name, 'SKILL.md') : join(dir, e.name)
      if (!e.isDirectory() && !e.name.endsWith('.md')) continue
      let text
      try {
        text = fs.readFileSync(md, 'utf8')
      } catch {
        continue
      }
      const name = /^---[\s\S]*?\nname:\s*([^\n]+)/.exec(text)?.[1]?.trim()
      const repoKey = /^---[\s\S]*?\nrepo_key:\s*([^\n]+)/
        .exec(text)?.[1]
        ?.trim()
        .replace(/^"|"$/g, '')
      if (repoKey && opts.repoKey && repoKey !== opts.repoKey) continue
      existing.push({
        name: name || e.name.replace(/\.md$/, ''),
        steps: extractSkillSteps(text),
      })
    }
  }

  const today = new Date().toISOString().slice(0, 10)
  const budgetPath = join(stateDir, 'proposals.json')
  let budget
  try {
    budget = JSON.parse(fs.readFileSync(budgetPath, 'utf8'))
  } catch {
    budget = null
  }
  budget = { date: today, runs: budget?.date === today ? (budget.runs ?? 0) : 0, cap }

  const {
    proposals,
    skipped,
    budget: spent,
  } = proposeSkills(graph, { repoKey: opts.repoKey, minBelief, budget, existing })

  for (const p of proposals) {
    if (!/^[A-Za-z0-9][A-Za-z0-9_-]{0,191}$/u.test(p.repoKey))
      throw new Error('Invalid habit repository key')
    const dir = join(proposedDir, `${p.name}--${p.repoKey}`)
    const note = `${dir}/SKILL.md (belief ${p.belief.toFixed(2)}${p.risky ? ', RISKY' : ''})`
    if (dryRun) {
      log(`would propose ${note}`)
      continue
    }
    fs.mkdirSync(dir, { recursive: true })
    fs.writeFileSync(join(dir, 'SKILL.md'), p.markdown)
    log(`proposed ${note}`)
  }
  for (const s of skipped) log(`skip ${s.name}: ${s.reason}`)
  if (!dryRun) {
    fs.mkdirSync(stateDir, { recursive: true })
    fs.writeFileSync(budgetPath, JSON.stringify(spent, null, 2))
  }
  return { proposals, skipped }
}

async function cli(argv, selectedGraph) {
  const fs = await import('./private-store-fs.mjs')
  const { readFileSync, writeFileSync, existsSync } = fs
  const { fileURLToPath } = await import('node:url')
  const { dirname, join } = await import('node:path')
  const os = await import('node:os')

  const ROOT = join(dirname(fileURLToPath(import.meta.url)), '..', '..')
  const flag = (name) => {
    const i = argv.indexOf(name)
    return i >= 0 ? argv[i + 1] : undefined
  }
  const graphPath = selectedGraph ?? (flag('--graph') || workerPaths().graph)
  const ledgerPath = flag('--ledger') || process.env.ANGEL_EXPERIENCE_LOG || workerPaths().ledger
  const onlyRepo = flag('--repo')
  const dryRun = argv.includes('--dry-run')
  let previewGraph

  const CausalGraph = (await import('../../lib/research/CausalGraph.js')).default
  const loadGraph = () =>
    existsSync(graphPath)
      ? CausalGraph.deserialize(JSON.parse(readFileSync(graphPath, 'utf8')))
      : new CausalGraph()
  const loadLedger = () => {
    if (!existsSync(ledgerPath)) return []
    return readFileSync(ledgerPath, 'utf8')
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
  }

  if (argv.includes('--list')) {
    const mined = mineWorkflows(loadLedger())
    for (const [key, rec] of Object.entries(mined)) {
      if (onlyRepo && key !== onlyRepo) continue
      console.log(`${key} (${rec.slug ?? rec.root ?? '?'}): ${rec.workflows.length} workflow(s)`)
      for (const wf of rec.workflows)
        console.log(
          `  ${wf.steps.map((s) => s.class ?? s.text).join(' → ')} · ${wf.support} run(s) / ${wf.sessions} session(s), ` +
            `${wf.passes} clean · steps: ${wf.steps.map((s) => `\`${s.text}\` ${s.passRate == null ? 'no verdict' : `${Math.round(s.passRate * 100)}%`}`).join(' → ')}`,
        )
    }
    if (Object.keys(mined).length === 0) console.log(`no cmd events in ${ledgerPath}`)
    return 0
  }

  const refresh = argv.includes('--refresh')
  if (argv.includes('--mine') || refresh) {
    const graph = loadGraph()
    const mined = mineWorkflows(loadLedger())
    let nodes = 0
    let updated = 0
    for (const [key, rec] of Object.entries(mined)) {
      if (onlyRepo && key !== onlyRepo) continue
      const { added } = ingestWorkflows(graph, key, rec)
      nodes += added.nodes
      updated += added.updated
    }
    if (dryRun) previewGraph = graph
    else writeFileSync(graphPath, JSON.stringify(graph.serialize(), null, 2))
    console.log(
      `habitsmith ${dryRun ? 'would mine' : 'mine'} → ${graphPath}\n` +
        `  ${Object.keys(mined).length} repo(s) in ${ledgerPath}\n` +
        `  ${nodes} new + ${updated} refreshed workflow fact(s)`,
    )
    if (!refresh) return 0
  }

  if (argv.includes('--propose') || refresh) {
    const { proposals, skipped } = await proposeToDisk(previewGraph ?? loadGraph(), {
      repoKey: onlyRepo,
      dryRun,
      proposedDir: flag('--proposed-dir'),
      stateDir: flag('--state-dir'),
      root: ROOT,
    })
    if (proposals.length === 0 && skipped.length === 0)
      console.log('no workflow facts in the graph — run --mine first')
    return 0
  }

  console.log(
    'usage: node scripts/runtime/habitsmith.mjs --refresh|--mine|--list|--propose [--dry-run] [--repo <key>] [--graph <path>] [--ledger <path>] [--proposed-dir <dir>]',
  )
  return 2
}

const isCli =
  process.argv[1] &&
  existsSync(process.argv[1]) &&
  import.meta.url === pathToFileURL(realpathSync(process.argv[1])).href
if (isCli) {
  runGraphCli(cli, process.argv.slice(2)).then((code) => process.exit(code ?? 0))
}
