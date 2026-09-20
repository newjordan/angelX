import { runGraphCli } from './worker-lock.mjs'
import { workerPaths } from './worker-paths.mjs'
// habitsmith — angel writes its own skills from watching your habits.
// H1/H2 of docs/plans/habitsmith.md, the fourth instance of the
// ingest→score→propose pattern (self-causal → config-causal → repo-dossier).
//
// The Dossier (repo-dossier.mjs) learns single commands; real habits are
// SEQUENCES. This module mines the same D1 cmd events — grouped per repo, per
// session, ordered by seq — into recurring multi-step workflows
// ("build → test → relaunch"), and folds the recurrent ones into the shared
// causal graph as HYPOTHESIS nodes with factKind 'workflow':
//   • mining attaches OBSERVATIONAL stats only (no signed edges) — the locked
//     rule: observation prioritizes, verification concludes;
//   • belief reuses the Dossier's factBelief verbatim — for a workflow the
//     evidence rate is the fraction of occurrences where EVERY step passed;
//   • facts are PERPETUAL (the D4 lesson): probes re-verify, nothing concludes.
//
// Pure core (no file I/O, no network) + a CLI shell at the bottom. Node ids
// embed the repo key VERBATIM (minted Rust-side, never re-derived here).

import { NODE_TYPE } from '../lib/research/CausalGraph.js'
import { classifyCommand, factBelief } from './repo-dossier.mjs'
import { applyProbe } from './dossier-tick.mjs'

export const HABIT_PROJECT = Object.freeze({ id: 'habitsmith', label: 'Habitsmith' })

// ─── mining thresholds ───────────────────────────────────────────────────────
// A workflow needs recurrence across sessions before it's believed to be "how
// work happens here" — a sequence seen in one or two sessions is coincidence.
export const WORKFLOW_MIN_SESSIONS = 3
// Pattern length bounds: a single command is the Dossier's job; past six steps
// it's a script, not a habit.
export const WORKFLOW_MIN_LEN = 2
export const WORKFLOW_MAX_LEN = 6
// Gap tolerance — the knob that decides everything: up to this many unrelated
// commands may sit between two steps of a habit and it still counts.
export const MAX_GAP = 2
// Of all the times two adjacent steps co-occur (either direction), at least
// this fraction must be in the pattern's order.
export const ORDER_MIN = 0.7
// D1's 400-char cap and the secret scrubber both leave this marker. An
// incomplete or masked step must never become a skill step, so any command
// containing it is disqualified as a pattern element (it still fills a gap).
export const TRUNCATION_MARK = '…'

const slug = (s) =>
  String(s ?? '')
    .toLowerCase()
    .replace(/[^0-9a-z]+/g, '-')
    .replace(/^-+|-+$/g, '')
    .slice(0, 48) || 'x'

const isHabit = (node) => node?.projectId === HABIT_PROJECT.id
const isCmdEvent = (r) => r?.kind === 'event' && r?.event === 'cmd' && r?.repo?.key && r?.cmd?.text

// ─── H1: sequence mining ─────────────────────────────────────────────────────

/**
 * Group cmd events into per-repo, per-session ordered step sequences. A step's
 * identity is its ritual class when classified (so `cargo test` and
 * `cargo test -p x` are the same step), else the exact scrubbed text.
 * Consecutive same-identity commands collapse to the LAST attempt —
 * retry-until-pass is one logical step, and its eventual outcome is the one
 * the habit depends on. Truncated/masked commands (TRUNCATION_MARK) keep their
 * position (they consume gap budget) but carry id:null so they can never match
 * a pattern element.
 *
 * @returns {Object<string,{root:string|null, slug:string|null,
 *   sessions:Object<string,{id:string|null,text:string,ok:boolean,ts:number}[]>}>}
 */
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
    const step = {
      id: text.includes(TRUNCATION_MARK) ? null : (classifyCommand(text) ?? text),
      text,
      ok: r.cmd.exit === 0 && !r.cmd.timed_out,
      ts: Number(r.ts) || 0,
    }
    const seq = (rec.sessions[r.session ?? 0] ||= [])
    const prev = seq[seq.length - 1]
    if (prev && prev.id !== null && prev.id === step.id) seq[seq.length - 1] = step
    else seq.push(step)
  }
  return repos
}

/**
 * Leftmost-greedy, non-overlapping occurrences of `pattern` (a list of step
 * ids) in one session's step sequence, allowing up to `maxGap` other commands
 * between consecutive steps. Returns one entry per occurrence with the actual
 * step samples matched.
 */
export function matchPattern(seq, pattern, maxGap = MAX_GAP) {
  const found = []
  let from = 0
  outer: while (from < seq.length) {
    // Anchor: next position matching pattern[0].
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
        from = i + 1 // this anchor can't complete; try the next one
        continue outer
      }
      samples.push(seq[j])
      at = j
    }
    found.push(samples)
    from = at + 1 // non-overlapping: resume after the match
  }
  return found
}

/** Occurrences of a pattern across every session of one repo. */
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

/**
 * Mine parsed ledger rows into per-repo workflow candidates. Pure.
 *
 * Level-wise growth (Apriori-shaped, tiny data): frequent step ids → frequent
 * ordered pairs → extend one step at a time up to WORKFLOW_MAX_LEN, pruning by
 * session support at each level. Survivors must also be order-consistent
 * (every adjacent pair appears in pattern order ≥ ORDER_MIN of the time) and
 * CLOSED (a pattern subsumed by a longer one with the same session support is
 * dropped — `build→test` says nothing `build→test→run` doesn't).
 *
 * @returns {Object<string,{root, slug, workflows:{steps:{text,class,passRate}[],
 *   support, passes, sessions, lastSeen, repoKey}[]}>}
 */
export function mineWorkflows(rows) {
  const repos = sessionStepSeqs(rows)
  const out = {}
  for (const [repoKey, rec] of Object.entries(repos)) {
    // Frequent ids: an id in a ≥N-session pattern must itself span ≥N sessions.
    const idSessions = new Map()
    for (const [session, seq] of Object.entries(rec.sessions))
      for (const s of seq)
        if (s.id !== null)
          (idSessions.get(s.id) ?? idSessions.set(s.id, new Set()).get(s.id)).add(session)
    const ids = [...idSessions.entries()]
      .filter(([, ss]) => ss.size >= WORKFLOW_MIN_SESSIONS)
      .map(([id]) => id)
      .sort()

    // Level-wise: seed with ordered pairs, grow by one id per level.
    const pairCount = new Map() // 'a\u0000b' -> occurrence count, for the order check
    const countPair = (a, b) => {
      const k = `${a}\u0000${b}`
      if (!pairCount.has(k)) pairCount.set(k, occurrences(rec, [a, b]).length)
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
        if (a === b) continue
        const occs = occurrences(rec, [a, b])
        if (sessionCount(occs) >= WORKFLOW_MIN_SESSIONS) level.push({ pattern: [a, b], occs })
      }
    const frequent = [...level]
    while (level.length && level[0].pattern.length < WORKFLOW_MAX_LEN) {
      const next = []
      for (const { pattern } of level)
        for (const b of ids) {
          if (b === pattern[pattern.length - 1]) continue // no adjacent repeats
          const grown = [...pattern, b]
          const occs = occurrences(rec, grown)
          if (sessionCount(occs) >= WORKFLOW_MIN_SESSIONS) next.push({ pattern: grown, occs })
        }
      frequent.push(...next)
      level = next
    }

    // Order consistency, then closed-pattern pruning.
    const consistent = frequent.filter((c) => orderOk(c.pattern))
    const subsumes = (long, short) => {
      // order-preserving subsequence test
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
        const steps = pattern.map((id, i) => {
          const at = occs.map((o) => o.samples[i])
          const freq = (list) => {
            const m = new Map()
            for (const s of list) m.set(s.text, (m.get(s.text) || 0) + 1)
            return [...m.entries()].sort((a, b) => b[1] - a[1] || (a[0] < b[0] ? -1 : 1))[0]?.[0]
          }
          const text = freq(at.filter((s) => s.ok)) ?? freq(at)
          return {
            text,
            class: classifyCommand(text),
            passRate: Number((at.filter((s) => s.ok).length / at.length).toFixed(3)),
          }
        })
        return {
          steps,
          support: occs.length,
          passes: occs.filter((o) => o.allOk).length,
          sessions: sessionCount(occs),
          lastSeen: Math.max(...occs.map((o) => o.samples[o.samples.length - 1].ts)),
          repoKey,
        }
      })
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

// ─── H2: workflow facts in the graph ─────────────────────────────────────────

/** The class-chain slug that names a workflow: `build-test-run`. */
export function chainSlug(steps) {
  return slug(steps.map((s) => s.class ?? s.text).join('-'))
}

export const workflowId = (repoKey, wf) => `hyp_habit_${repoKey}_${chainSlug(wf.steps)}`

const arrow = (steps) => steps.map((s) => s.class ?? s.text).join(' → ')

/**
 * Fold one repo's mined workflows into the graph as habitsmith HYPOTHESIS
 * nodes. Idempotent on the deterministic id: re-mining refreshes the
 * observational stats instead of duplicating. No signed edges here — mining
 * observes; probes and the /habits verdicts (H4/H5) conclude.
 */
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
        fails: wf.support - wf.passes,
        sessions: wf.sessions,
        lastSeen: wf.lastSeen,
      },
    }
    if (graph.hasNode(id)) {
      graph.updateNode(id, { ...fields, lastSeenAt: now })
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

/** All habitsmith workflow facts in the graph, optionally scoped to one repo. */
export function habitFacts(graph, repoKey) {
  return graph
    .nodesOfType(NODE_TYPE.HYPOTHESIS)
    .filter(isHabit)
    .filter((n) => !repoKey || n.repoKey === repoKey)
}

/**
 * One workflow fact's belief — the Dossier's factBelief verbatim (imported):
 * factKind 'workflow' takes the passes/runs branch, and `passes` here counts
 * occurrences where EVERY step exited 0, so the evidence rate is exactly the
 * plan's "fraction of occurrences where every step passed".
 */
export const workflowBelief = (graph, node, opts = {}) => factBelief(graph, node, opts)

// ─── H3: skill compiler (propose-only) ───────────────────────────────────────
// Confident workflows render into draft skill folders under the PROPOSED dir —
// never the live one. Tier B propose-only, the Reflex M4 doctrine: a human
// keypress (`/habits approve`, H4) is the only path into `~/.angel0/skills`.

export const DEFAULT_MIN_BELIEF = 0.75 // ANGEL_HABIT_MIN_BELIEF
export const DEFAULT_MAX_PROPOSALS_PER_DAY = 2 // ANGEL_HABIT_MAX_PROPOSALS_PER_DAY
// Step-set similarity at or above this against ANY existing skill (bundled +
// live + proposed) skips the proposal — log the collision, don't spam.
export const JACCARD_SKIP = 0.6
// Steps carrying these verbs stay in the draft but get the ⚠ annotation and
// tag the whole skill `risky: true`. One deliberate deviation from the plan's
// regex: `release` must not match the `--release` FLAG (`cargo build
// --release` is the commonest build habit there is), only release-as-command.
export const RISKY_RE =
  /\b(push|publish|deploy|rm -rf|curl .*-X (POST|PUT|DELETE))\b|(?<!-)\brelease\b/

export const normalizeStep = (t) =>
  String(t ?? '')
    .trim()
    .toLowerCase()
    .replace(/\s+/g, ' ')

/**
 * The command-shaped lines of an existing skill's markdown: fenced code-block
 * lines plus inline `code` spans, normalized. This is the step-set an existing
 * skill is deduped against — prose is ignored, commands are what collide.
 */
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

/** Jaccard similarity of two normalized step sets. */
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

/** Proposed skill name: repo slug always included (collision answer: always). */
export function skillName(fact) {
  const repo = slug(lastSegment(fact.repoSlug) ?? lastSegment(fact.repoRoot) ?? fact.repoKey)
  return `${repo}-${chainSlug(fact.factSteps)}`
}

/**
 * Render one workflow fact into a draft SKILL.md — fully deterministic; any
 * LLM prose polish is a separate, optional, gated pass that this render never
 * depends on. The frontmatter drives triggering (name + description); `fact:`
 * carries provenance back to the graph node so H5 can close the loop, and the
 * harness parser ignores keys it doesn't know.
 */
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
    const pct = Math.round((s.passRate ?? 0) * 100)
    const warn = RISKY_RE.test(s.text) ? ' — ⚠ requires explicit confirmation' : ''
    lines.push(`${i + 1}. \`${s.text}\` (${s.class ?? 'unclassified'}, passes ${pct}%)${warn}`)
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

/**
 * The propose pass, pure: belief-gate the graph's workflow facts, dedupe
 * against everything that already exists, spend the daily budget on what
 * survives (highest belief first). The caller does all fs.
 *
 * @param {CausalGraph} graph
 * @param {{now?, repoKey?, minBelief?, budget:{date,runs,cap},
 *   existing:{name:string, steps:Set<string>}[]}} opts
 * @returns {{proposals:{name,factId,repoKey,belief,risky,markdown}[],
 *   skipped:{factId,name,reason}[], budget}}
 */
export function proposeSkills(graph, opts = {}) {
  const minBelief = opts.minBelief ?? DEFAULT_MIN_BELIEF
  const existing = opts.existing ?? []
  let budget = opts.budget ?? { date: '', runs: 0, cap: DEFAULT_MAX_PROPOSALS_PER_DAY }
  const proposals = []
  const skipped = []

  const candidates = habitFacts(graph, opts.repoKey)
    .map((fact) => ({ fact, belief: workflowBelief(graph, fact, opts).belief }))
    .sort((a, b) => b.belief - a.belief || (a.fact.id < b.fact.id ? -1 : 1))

  for (const { fact, belief } of candidates) {
    const name = skillName(fact)
    const mySteps = new Set((fact.factSteps || []).map((s) => normalizeStep(s.text)))
    if (belief < minBelief) {
      skipped.push({ factId: fact.id, name, reason: `belief ${belief.toFixed(2)} < ${minBelief}` })
      continue
    }
    // The name is the workflow's deterministic identity — an exact collision
    // (our own earlier draft, or the approved skill) is a dupe regardless of
    // how much prose surrounds the other file's steps.
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

// ─── H5: the closed loop — skill events, verdicts, drift ────────────────────
// A minted skill keeps verifying itself: the cockpit records every
// `skill(name)` load as a ledger event (experience.rs), /habits verdicts wait
// in a spool (habits.rs writes it; the graph is Node-owned), and the tick
// folds both back onto the workflow facts. Belief keeps flowing from cmd
// events regardless — a skill whose steps start failing decays on its own;
// usage tells us someone still RELIES on it, which is what makes decay DRIFT.

// A workflow fact whose skill is still being used but whose belief fell under
// this is "drifting": the repo changed underneath a habit someone relies on.
export const DRIFT_BELIEF = 0.6

const isSkillEvent = (r) => r?.kind === 'event' && r?.event === 'skill' && r?.skill?.name

/** Per-skill usage mined from `event:"skill"` ledger rows. Pure. */
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

/**
 * Attach mined usage to the workflow facts that spawned the skills, via the
 * name→factId map read from installed drafts' `fact:` frontmatter. Idempotent
 * (full-scan counts overwrite). Observational only — usage never adds edges;
 * it decides whether a decayed belief is flagged as drift.
 */
export function attachSkillUsage(graph, usage, nameToFact) {
  let attached = 0
  for (const [name, factId] of Object.entries(nameToFact || {})) {
    if (!usage[name] || !graph.hasNode(factId)) continue
    graph.updateNode(factId, { skillUsage: { name: name.split('\u0000').at(-1), ...usage[name] } })
    attached++
  }
  return attached
}

/**
 * Fold the /habits verdict spool into the graph: approve → SUPPORTS 0.9,
 * reject → CONTRADICTS 0.9, both via the dossier tick's applyProbe (evidence
 * edge + flip back to 'testing' + lastVerifiedAt — facts stay perpetual).
 * Unknown facts are reported, not fatal (the draft may predate a graph wipe).
 */
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

/**
 * The status artifact the tick writes to `~/.angel0/habitsmith/status.json` and
 * `/habits` renders alongside the drafts: every workflow fact with its belief,
 * skill usage, and the drift flag. Pure.
 */
export function compileHabitsStatus(graph, opts = {}) {
  const now = opts.now || new Date().toISOString()
  const driftBelief = opts.driftBelief ?? DRIFT_BELIEF
  const facts = habitFacts(graph, opts.repoKey)
    .map((n) => {
      const belief = Number(workflowBelief(graph, n, { ...opts, now }).belief.toFixed(3))
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

// ─── shared I/O: the propose pass ────────────────────────────────────────────
// Used by both the CLI (`--propose`) and the habitsmith tick — the one place
// that scans existing skills, spends the daily budget file, and writes drafts.

export async function proposeToDisk(graph, opts = {}) {
  const fs = await import('./private-store-fs.mjs')
  const { join, dirname } = await import('node:path')
  const { fileURLToPath } = await import('node:url')
  const os = await import('node:os')
  const home = os.homedir()
  const ROOT = opts.root ?? join(dirname(fileURLToPath(import.meta.url)), '..')
  const proposedDir =
    opts.proposedDir || process.env.ANGEL_HABIT_PROPOSED_DIR || workerPaths().proposed
  const userSkillsDir = opts.skillsDir || process.env.ANGEL_SKILLS_DIR || workerPaths().skills
  const bundledDir = process.env.ANGEL_BUNDLED_SKILLS_DIR || join(ROOT, 'cockpit/skills')
  const stateDir = opts.stateDir || workerPaths().habits
  const minBelief = Number(process.env.ANGEL_HABIT_MIN_BELIEF || DEFAULT_MIN_BELIEF)
  const cap = Number(process.env.ANGEL_HABIT_MAX_PROPOSALS_PER_DAY || DEFAULT_MAX_PROPOSALS_PER_DAY)
  const log = opts.log ?? console.log
  const dryRun = !!opts.dryRun

  // Every skill that already exists, as a name + normalized step-set. Both
  // layouts (folder-per-SKILL.md, flat <name>.md), all three dirs.
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
      // Explicitly project-scoped skills collide only inside their own
      // repository. Global skills still reserve the name everywhere.
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
    // The human-facing skill name can legitimately recur in unrelated repos;
    // the folder identity cannot, because drafts share one user-global spool.
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

// ─── CLI ─────────────────────────────────────────────────────────────────────
// I/O lives here only. `--mine` folds ledger sequences into the shared graph;
// `--list` prints what the miner sees without touching the graph; `--propose`
// renders belief-gated drafts into the PROPOSED dir (never the live one). Same
// graph-file caveats as config-causal (no locking; ticks hold the lockfile).

async function cli(argv) {
  const fs = await import('./private-store-fs.mjs')
  const { readFileSync, writeFileSync, existsSync } = fs
  const { fileURLToPath } = await import('node:url')
  const { dirname, join } = await import('node:path')
  const os = await import('node:os')

  const ROOT = join(dirname(fileURLToPath(import.meta.url)), '..')
  const flag = (name) => {
    const i = argv.indexOf(name)
    return i >= 0 ? argv[i + 1] : undefined
  }
  const graphPath = flag('--graph') || workerPaths().graph
  const ledgerPath = flag('--ledger') || process.env.ANGEL_EXPERIENCE_LOG || workerPaths().ledger
  const onlyRepo = flag('--repo')

  const CausalGraph = (await import('../lib/research/CausalGraph.js')).default
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
          `  ${arrow(wf.steps)} · ${wf.support} run(s) / ${wf.sessions} session(s), ` +
            `${wf.passes} clean · steps: ${wf.steps.map((s) => `\`${s.text}\` ${Math.round(s.passRate * 100)}%`).join(' → ')}`,
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
    writeFileSync(graphPath, JSON.stringify(graph.serialize(), null, 2))
    console.log(
      `habitsmith mine → ${graphPath}\n` +
        `  ${Object.keys(mined).length} repo(s) in ${ledgerPath}\n` +
        `  ${nodes} new + ${updated} refreshed workflow fact(s)`,
    )
    if (!refresh) return 0
  }

  if (argv.includes('--propose') || refresh) {
    const { proposals, skipped } = await proposeToDisk(loadGraph(), {
      repoKey: onlyRepo,
      dryRun: argv.includes('--dry-run'),
      proposedDir: flag('--proposed-dir'),
      stateDir: flag('--state-dir'),
      root: ROOT,
    })
    if (proposals.length === 0 && skipped.length === 0)
      console.log('no workflow facts in the graph — run --mine first')
    return 0
  }

  console.log(
    'usage: node scripts/habitsmith.mjs --refresh|--mine|--list|--propose [--dry-run] [--repo <key>] [--graph <path>] [--ledger <path>] [--proposed-dir <dir>]',
  )
  return 2
}

const isCli =
  process.argv[1] && (await import('node:url')).fileURLToPath(import.meta.url) === process.argv[1]
if (isCli) {
  runGraphCli(cli, process.argv.slice(2)).then((code) => process.exit(code ?? 0))
}
