import { NODE_TYPE } from '../research/CausalGraph.js'
import { classifyCommand, factBelief, factBeliefs, ledgerOutcome, ledgerIndependent, normalizeCommand, refreshFact } from '../dossier/core.mjs'
import { applyProbe } from '../dossier/probes.mjs'

export const HABIT_PROJECT = Object.freeze({ id: 'habitsmith', label: 'Habitsmith' })

export const WORKFLOW_MIN_SESSIONS = 3
export const WORKFLOW_MIN_LEN = 2
export const WORKFLOW_MAX_LEN = 6
export const MAX_GAP = 2
export const ORDER_MIN = 0.7
export const TRUNCATION_MARK = '…'

const slug = (s) =>
  String(s ?? '')
    .toLowerCase()
    .replace(/[^0-9a-z]+/g, '-')
    .replace(/^-+|-+$/g, '')
    .slice(0, 48) || 'x'

const isHabit = (node) => node?.projectId === HABIT_PROJECT.id
const isCmdEvent = (r) => r?.kind === 'event' && r?.event === 'cmd' && r?.repo?.key && r?.cmd?.text

export function sessionStepSeqs(rows) {
  const repos = {}
  const events = (rows || []).filter(isCmdEvent)
  events.sort((a, b) => (a.session ?? 0) - (b.session ?? 0) || (a.seq ?? 0) - (b.seq ?? 0))
  for (const r of events) {
    const rec = (repos[r.repo.key] ||= {
      root: r.repo.root ?? null,
      slug: r.repo.slug ?? null,
      sessions: {},
    })
    const text = r.cmd.text
    const norm = normalizeCommand(text, r.repo)
    const step = {
      id: !norm || text.includes(TRUNCATION_MARK) ? null : (classifyCommand(text) ?? text),
      text,
      ok: norm ? ledgerOutcome(r, { attributable: typeof r.cmd.verdict === 'string' || norm.attributable }) : null,
      independent: ledgerIndependent(r.cmd),
      ts: Number(r.ts) || 0,
    }
    const seq = (rec.sessions[r.session ?? 0] ||= [])
    const prev = seq[seq.length - 1]
    if (prev && prev.id !== null && prev.id === step.id && prev.independent === step.independent) seq[seq.length - 1] = step
    else seq.push(step)
  }
  return repos
}

export function matchPattern(seq, pattern, maxGap = MAX_GAP) {
  const found = []
  let from = 0
  outer: while (from < seq.length) {
    let i = from
    while (i < seq.length && seq[i].id !== pattern[0]) i++
    if (i >= seq.length) break
    const samples = [seq[i]]
    let at = i
    for (let p = 1; p < pattern.length; p++) {
      let j = at + 1
      const limit = at + 1 + maxGap
      while (j < seq.length && j <= limit && seq[j].id !== pattern[p]) j++
      if (j >= seq.length || j > limit) {
        from = i + 1
        continue outer
      }
      samples.push(seq[j])
      at = j
    }
    found.push(samples)
    from = at + 1
  }
  return found
}

function occurrences(rec, pattern) {
  const out = []
  for (const [session, seq] of Object.entries(rec.sessions)) {
    for (const samples of matchPattern(seq, pattern)) {
      out.push({ session, samples, allOk: samples.every((s) => s.ok) })
    }
  }
  return out
}

const sessionCount = (occs) => new Set(occs.map((o) => o.session)).size

export function mineWorkflows(rows) {
  const repos = sessionStepSeqs(rows)
  const out = {}
  for (const [repoKey, rec] of Object.entries(repos)) {
    const independentRec = { ...rec, sessions: Object.fromEntries(
      Object.entries(rec.sessions).map(([session, seq]) => [session,
        seq.filter(step => step.independent),
      ]),
    ) }
    const idSessions = new Map()
    for (const [session, seq] of Object.entries(independentRec.sessions))
      for (const s of seq)
        if (s.id !== null)
          (idSessions.get(s.id) ?? idSessions.set(s.id, new Set()).get(s.id)).add(session)
    const ids = [...idSessions.entries()]
      .filter(([, ss]) => ss.size >= WORKFLOW_MIN_SESSIONS)
      .map(([id]) => id)
      .sort()

    const pairCount = new Map()
    const countPair = (a, b) => {
      const k = `${a}\u0000${b}`
      if (!pairCount.has(k)) pairCount.set(k, occurrences(independentRec, [a, b]).length)
      return pairCount.get(k)
    }
    const orderOk = (pattern) => {
      for (let i = 0; i + 1 < pattern.length; i++) {
        const fwd = countPair(pattern[i], pattern[i + 1])
        const rev = countPair(pattern[i + 1], pattern[i])
        if (fwd + rev === 0 || fwd / (fwd + rev) < ORDER_MIN) return false
      }
      return true
    }

    let level = []
    for (const a of ids)
      for (const b of ids) {
        if (a === b || !orderOk([a, b])) continue
        const occs = occurrences(independentRec, [a, b])
        if (sessionCount(occs) >= WORKFLOW_MIN_SESSIONS) level.push({ pattern: [a, b], occs })
      }
    const frequent = [...level]
    while (level.length && level[0].pattern.length < WORKFLOW_MAX_LEN) {
      const next = []
      for (const { pattern } of level)
        for (const b of ids) {
          if (b === pattern[pattern.length - 1] || !orderOk([pattern[pattern.length - 1], b])) continue
          const grown = [...pattern, b]
          const occs = occurrences(independentRec, grown)
          if (sessionCount(occs) >= WORKFLOW_MIN_SESSIONS) next.push({ pattern: grown, occs })
        }
      frequent.push(...next)
      level = next
    }

    const consistent = frequent.filter((c) => orderOk(c.pattern))
    const subsumes = (long, short) => {
      let i = 0
      for (const x of long) if (x === short[i]) i++
      return i >= short.length
    }
    const closed = consistent.filter(
      (c) =>
        !consistent.some(
          (d) =>
            d.pattern.length > c.pattern.length &&
            subsumes(d.pattern, c.pattern) &&
            sessionCount(d.occs) >= sessionCount(c.occs),
        ),
    )

    const workflows = closed
      .map(({ pattern, occs }) => {
        const variants = new Map()
        for (const occurrence of occs) {
          const key = JSON.stringify(occurrence.samples.map(s => s.text))
          const variant = variants.get(key) ?? { key, occurrences: [], passes: 0 }
          variant.occurrences.push(occurrence)
          if (occurrence.allOk) variant.passes++
          variants.set(key, variant)
        }
        const selected = [...variants.values()].sort((a, b) =>
          b.passes - a.passes || b.occurrences.length - a.occurrences.length ||
          (a.key < b.key ? -1 : a.key > b.key ? 1 : 0),
        )[0]
        const own = selected.occurrences
        if (sessionCount(own) < WORKFLOW_MIN_SESSIONS) return null
        const steps = own[0].samples.map(s => ({ text: s.text, class: classifyCommand(s.text) }))
        const dissent = occurrences(rec, pattern).filter(o =>
          o.samples.some(s => !s.independent) &&
          o.samples.some(s => s.ok === false) &&
          o.samples.every((s, i) => s.text === steps[i].text),
        )
        const contributing = [...own, ...dissent]
        for (const [i, step] of steps.entries()) {
          const known = [...own.map(o => o.samples[i]), ...dissent.map(o => o.samples[i]).filter(s => s.ok === false)]
            .filter(s => s.ok !== null)
          step.passRate = known.length ? Number((known.filter(s => s.ok).length / known.length).toFixed(3)) : null
        }
        return {
          steps,
          support: contributing.length,
          passes: own.filter((o) => o.allOk).length,
          verdicts: contributing.filter((o) => o.allOk || o.samples.some((s) => s.ok === false)).length,
          sessions: sessionCount(own),
          lastSeen: Math.max(...own.map((o) => o.samples[o.samples.length - 1].ts)),
          repoKey,
        }
      })
      .filter(Boolean)
      .sort(
        (a, b) =>
          b.sessions - a.sessions ||
          b.support - a.support ||
          (chainSlug(a.steps) < chainSlug(b.steps) ? -1 : 1),
      )

    out[repoKey] = { root: rec.root, slug: rec.slug, workflows }
  }
  return out
}

export function chainSlug(steps) {
  return slug(steps.map((s) => s.class ?? s.text).join('-'))
}

export const workflowId = (repoKey, wf) => `hyp_habit_${repoKey}_${chainSlug(wf.steps)}`

const arrow = (steps) => steps.map((s) => s.class ?? s.text).join(' → ')

export function ingestWorkflows(graph, repoKey, mined, opts = {}) {
  const now = opts.now || new Date().toISOString()
  let nodes = 0
  let updated = 0
  const hypotheses = []
  for (const wf of mined.workflows || []) {
    const id = workflowId(repoKey, wf)
    hypotheses.push(id)
    const fields = {
      factKind: 'workflow',
      factClass: null,
      factText: wf.steps.map((s) => s.text).join(' && '),
      factSteps: wf.steps,
      label: `Workflow: ${arrow(wf.steps)}`,
      question: `Is "${arrow(wf.steps)}" still how work happens in ${mined.root ?? repoKey}?`,
      prediction: `Running the ${wf.steps.length}-step sequence in ${mined.root ?? repoKey} succeeds end-to-end`,
      observed: {
        runs: wf.support,
        passes: wf.passes,
        verdicts: wf.verdicts ?? wf.support,
        fails: (wf.verdicts ?? wf.support) - wf.passes,
        sessions: wf.sessions,
        lastSeen: wf.lastSeen,
      },
    }
    if (graph.hasNode(id)) {
      refreshFact(graph, id, fields, now)
      updated++
      continue
    }
    graph.addNode({
      id,
      type: NODE_TYPE.HYPOTHESIS,
      hypothesisId: id.replace(/^hyp_/, ''),
      projectId: HABIT_PROJECT.id,
      projectLabel: HABIT_PROJECT.label,
      metric: { name: `habits-${repoKey}`, direction: 'higher' },
      status: 'testing',
      proposedAt: now,
      lastSeenAt: now,
      concludedAt: null,
      outcome: 'neutral',
      repoKey,
      repoRoot: mined.root ?? null,
      repoSlug: mined.slug ?? null,
      ...fields,
    })
    nodes++
  }
  return { graph, added: { nodes, updated }, hypotheses }
}

export function habitFacts(graph, repoKey) {
  return graph
    .nodesOfType(NODE_TYPE.HYPOTHESIS)
    .filter(isHabit)
    .filter((n) => !repoKey || n.repoKey === repoKey)
}

export const workflowBelief = (graph, node, opts = {}) => factBelief(graph, node, opts)

export const DEFAULT_MIN_BELIEF = 0.75 // ANGEL_HABIT_MIN_BELIEF
export const DEFAULT_MAX_PROPOSALS_PER_DAY = 2 // ANGEL_HABIT_MAX_PROPOSALS_PER_DAY
export const JACCARD_SKIP = 0.6
export const RISKY_RE =
  /\b(push|publish|deploy|rm -rf|curl .*-X (POST|PUT|DELETE))\b|(?<!-)\brelease\b/

export const normalizeStep = (t) =>
  String(t ?? '')
    .trim()
    .toLowerCase()
    .replace(/\s+/g, ' ')

export function extractSkillSteps(markdown) {
  const steps = new Set()
  const text = String(markdown ?? '')
  for (const fence of text.matchAll(/```[^\n]*\n([\s\S]*?)```/g))
    for (const line of fence[1].split('\n')) {
      const t = normalizeStep(line)
      if (t && !t.startsWith('#')) steps.add(t)
    }
  for (const span of text.replace(/```[\s\S]*?```/g, '').matchAll(/`([^`\n]+)`/g)) {
    const t = normalizeStep(span[1])
    if (t) steps.add(t)
  }
  return steps
}

export function stepJaccard(a, b) {
  const A = a instanceof Set ? a : new Set(a)
  const B = b instanceof Set ? b : new Set(b)
  if (A.size === 0 && B.size === 0) return 0
  let inter = 0
  for (const x of A) if (B.has(x)) inter++
  return inter / (A.size + B.size - inter)
}

const lastSegment = (s) =>
  String(s ?? '')
    .split('/')
    .filter(Boolean)
    .pop()

export function skillName(fact) {
  const repo = slug(lastSegment(fact.repoSlug) ?? lastSegment(fact.repoRoot) ?? fact.repoKey)
  return `${repo}-${chainSlug(fact.factSteps)}`
}

export function renderSkillMd(fact, opts = {}) {
  const belief = opts.belief ?? workflowBelief(opts.graph, fact, opts).belief
  const now = opts.now || new Date().toISOString()
  const steps = fact.factSteps || []
  const name = skillName(fact)
  const where = fact.repoRoot ?? fact.repoKey
  const risky = steps.some((s) => RISKY_RE.test(s.text))
  const o = fact.observed || {}
  const chain = steps.map((s) => s.class ?? s.text).join(' → ')
  const description =
    `Run this repo's observed ${chain} workflow in ${where} ` +
    `the way its sessions do it: ${steps.map((s) => s.text).join(', then ')}.`

  const lines = [
    '---',
    `name: ${name}`,
    `description: ${description}`,
    'scope: project',
    `repo_key: ${JSON.stringify(fact.repoKey ?? '')}`,
    `repo_root: ${JSON.stringify(fact.repoRoot ?? '')}`,
    `fact: ${fact.id}`,
  ]
  if (risky) lines.push('risky: true')
  lines.push('---', '', `# ${name}`, '')
  lines.push(
    `Observed workflow for \`${where}\`, proposed by Habitsmith because this`,
    'sequence recurred across sessions (evidence below). Run the steps in order,',
    'from the workspace root.',
    '',
    'Preconditions:',
    `- \`${where}\` exists and is the cwd for every step.`,
    '- Mutating steps assume a reasonably clean tree — stash surprises first.',
    '',
    'Steps:',
  )
  steps.forEach((s, i) => {
    const rate = s.passRate == null ? 'no verdict' : `passes ${Math.round(s.passRate * 100)}%`
    const warn = RISKY_RE.test(s.text) ? ' — ⚠ requires explicit confirmation' : ''
    lines.push(`${i + 1}. \`${s.text}\` (${s.class ?? 'unclassified'}, ${rate})${warn}`)
  })
  lines.push(
    '',
    '---',
    `Evidence: ${o.runs ?? 0} run(s) across ${o.sessions ?? 0} session(s), ` +
      `${o.passes ?? 0} clean end-to-end · belief ${belief.toFixed(2)} · ` +
      `fact \`${fact.id}\` · generated ${now.slice(0, 10)} by habitsmith`,
    '',
  )
  return { name, risky, belief, markdown: lines.join('\n') }
}

export function proposeSkills(graph, opts = {}) {
  const minBelief = opts.minBelief ?? DEFAULT_MIN_BELIEF
  const existing = opts.existing ?? []
  let budget = opts.budget ?? { date: '', runs: 0, cap: DEFAULT_MAX_PROPOSALS_PER_DAY }
  const proposals = []
  const skipped = []

  const nodes = habitFacts(graph, opts.repoKey)
  const beliefs = factBeliefs(graph, nodes, opts)
  const candidates = nodes
    .map((fact) => ({ fact, belief: beliefs.get(fact.id).belief }))
    .sort((a, b) => b.belief - a.belief || (a.fact.id < b.fact.id ? -1 : 1))

  for (const { fact, belief } of candidates) {
    const name = skillName(fact)
    const mySteps = new Set((fact.factSteps || []).map((s) => normalizeStep(s.text)))
    if (belief < minBelief) {
      skipped.push({ factId: fact.id, name, reason: `belief ${belief.toFixed(2)} < ${minBelief}` })
      continue
    }
    if (existing.some((e) => e.name === name)) {
      skipped.push({ factId: fact.id, name, reason: `skill '${name}' already exists` })
      continue
    }
    const twin = existing
      .map((e) => ({ ...e, j: stepJaccard(mySteps, e.steps) }))
      .filter((e) => e.j >= JACCARD_SKIP)
      .sort((a, b) => b.j - a.j)[0]
    if (twin) {
      skipped.push({
        factId: fact.id,
        name,
        reason: `duplicate of skill '${twin.name}' (jaccard ${twin.j.toFixed(2)})`,
      })
      continue
    }
    if (budget.runs >= budget.cap) {
      skipped.push({ factId: fact.id, name, reason: 'daily proposal budget exhausted' })
      continue
    }
    budget = { ...budget, runs: budget.runs + 1 }
    proposals.push({
      ...renderSkillMd(fact, { graph, belief, now: opts.now }),
      factId: fact.id,
      repoKey: fact.repoKey,
    })
  }
  return { proposals, skipped, budget }
}

export const DRIFT_BELIEF = 0.6

const isSkillEvent = (r) => r?.kind === 'event' && r?.event === 'skill' && r?.skill?.name

export function mineSkillUsage(rows) {
  const usage = {}
  for (const r of rows || []) {
    if (!isSkillEvent(r)) continue
    const key = r.repo?.key ? `${r.repo.key}\u0000${r.skill.name}` : r.skill.name
    const u = (usage[key] ||= { uses: 0, ok: 0, lastUsed: 0 })
    u.uses++
    if (r.skill.ok) u.ok++
    u.lastUsed = Math.max(u.lastUsed, Number(r.ts) || 0)
  }
  return usage
}

export function attachSkillUsage(graph, usage, nameToFact) {
  let attached = 0
  for (const [name, factId] of Object.entries(nameToFact || {})) {
    if (!usage[name] || !graph.hasNode(factId)) continue
    graph.updateNode(factId, { skillUsage: { name: name.split('\u0000').at(-1), ...usage[name] } })
    attached++
  }
  return attached
}

export function foldVerdicts(graph, verdicts, opts = {}) {
  const now = opts.now || new Date().toISOString()
  const folded = []
  const orphaned = []
  for (const v of verdicts || []) {
    const node = v?.fact ? graph.getNode(v.fact) : null
    if (
      !node ||
      !['approve', 'reject'].includes(v.action) ||
      (v.repoKey != null && v.repoKey !== node.repoKey) ||
      (v.repoRoot != null && v.repoRoot !== node.repoRoot)
    ) {
      orphaned.push(v?.name ?? '?')
      continue
    }
    applyProbe(
      graph,
      v.fact,
      {
        verdict: v.action === 'approve' ? 'supports' : 'contradicts',
        confidence: 0.9,
        evidence: `user ${v.action}d '${v.name}' via /habits`,
      },
      { now },
    )
    folded.push(v.fact)
  }
  return { folded, orphaned }
}

export function compileHabitsStatus(graph, opts = {}) {
  const now = opts.now || new Date().toISOString()
  const driftBelief = opts.driftBelief ?? DRIFT_BELIEF
  const nodes = habitFacts(graph, opts.repoKey)
  const beliefs = factBeliefs(graph, nodes, { ...opts, now })
  const facts = nodes
    .map((n) => {
      const belief = Number(beliefs.get(n.id).belief.toFixed(3))
      const used = (n.skillUsage?.uses ?? 0) > 0
      return {
        id: n.id,
        name: skillName(n),
        repoKey: n.repoKey,
        repoRoot: n.repoRoot,
        label: n.label,
        steps: n.factSteps ?? [],
        observed: n.observed ?? {},
        skill: n.skillUsage ?? null,
        belief,
        drifting: used && belief < driftBelief,
        lastVerifiedAt: n.lastVerifiedAt ?? null,
      }
    })
    .sort((a, b) => (b.drifting ? 1 : 0) - (a.drifting ? 1 : 0) || b.belief - a.belief)
  return { v: 1, generatedAt: now, driftBelief, facts }
}
