import { NODE_TYPE, EDGE_TYPE } from '../research/CausalGraph.js'
import { beliefProbability, beliefProbabilities, scoreHypotheses } from '../research/causal-loop.mjs'
import { isMachineLabeled } from '../evidence/authored-write.mjs'
import { resolve as resolvePath, sep as pathSep } from 'node:path'

export function assertRepoKey(key) {
  if (typeof key !== 'string' || !/^[A-Za-z0-9][A-Za-z0-9_-]{0,191}$/u.test(key)) {
    throw new Error('Invalid dossier repository key')
  }
}

export const DOSSIER_PROJECT = Object.freeze({ id: 'repo-dossier', label: 'Repo dossier' })

export const RITUAL_MIN_RUNS = 3
export const RITUAL_MIN_SESSIONS = 2
export const TRAP_MIN_FAILS = 3
export const TRAP_MAX_PASS_RATE = 0.2
export const OBSERVATIONAL_WEIGHT = 0.4
export const DEFAULT_HALF_LIFE_DAYS = 14

const slug = (s) =>
  String(s ?? '')
    .toLowerCase()
    .replace(/[^0-9a-z]+/g, '-')
    .replace(/^-+|-+$/g, '')
    .slice(0, 48) || 'x'

const isDossier = (node) => node?.projectId === DOSSIER_PROJECT.id

const CHDIR_RE = /^\s*cd\s+("[^"]*"|'[^']*'|(?:\$\([^)]*\)|[^\s;&|])+)\s*(?:&&|;)\s*/
const STAGE_END_RE = /\s*(\|\||&&|[|;])\s*/
const REDIR_RE = /\s*(?:(?:(?<!\S)\d+)?(?:>>?\s*\S+|>&(?:\d+|-))|&>>?\s*\S+)\s*$/

function shellSyntax(text) {
  if (!/['"\\`$()\n\r#<&]/.test(text)) return text
  const syntax = text.split('')
  let quote = null
  for (let i = 0; i < text.length; i++) {
    const c = text[i]
    if (quote === "'") {
      syntax[i] = 'x'
      if (c === "'") quote = null
    } else if (c === '\\') {
      syntax[i] = 'x'
      if (++i >= text.length) return null
      syntax[i] = 'x'
    } else if (c === quote) {
      syntax[i] = 'x'; quote = null
    } else if (!quote && (c === "'" || c === '"')) {
      syntax[i] = 'x'; quote = c
    } else if (c === '`' || (c === '$' && /[({]/.test(text[i + 1] ?? ''))) return null
    else if (quote) syntax[i] = 'x'
    else if (/[()\n\r]/.test(c) || (c === '#' && (i === 0 || /\s/.test(text[i - 1])))) return null
    else if (c === '<' && text[i + 1] === '<') return null
    else if (c === '&' && !/[&<>]/.test(text[i - 1] ?? '') && !/[&>]/.test(text[i + 1] ?? '')) return null
  }
  return quote ? null : syntax.join('')
}

const unquote = (s) => s.replace(/^["']|["']$/g, '')

function resolveStaticChdir(target, base) {
  const raw = String(target ?? '').trim()
  const t = unquote(raw)
  if ((!raw.startsWith("'") && /[$`]/.test(t)) ||
      (!raw.startsWith("'") && !raw.startsWith('"') && t.startsWith('~')) || t === '-') return null
  if (!t.startsWith('/') && !base) return null
  return resolvePath(base ?? '/', t)
}

function chdirTargetIsInRepo(target, { root, cwd } = {}) {
  return [root, cwd].some((base) => {
    if (!base) return false
    const resolvedBase = resolvePath(base)
    const boundary = resolvedBase.endsWith(pathSep) ? resolvedBase : resolvedBase + pathSep
    return target === resolvedBase || target.startsWith(boundary)
  })
}

export function normalizeCommand(text, repo = {}) {
  let t = String(text ?? '').trim()
  let m
  let effectiveCwd = repo.cwd ?? repo.root
  while ((m = CHDIR_RE.exec(t))) {
    if (!['$(pwd)', '"$(pwd)"'].includes(m[1])) {
      const target = resolveStaticChdir(m[1], effectiveCwd)
      if (target === null || !chdirTargetIsInRepo(target, repo)) return null
      effectiveCwd = target
    }
    t = t.slice(m[0].length)
  }
  let syntax = shellSyntax(t)
  if (syntax === null) return null
  const end = syntax.search(STAGE_END_RE)
  const attributable = end < 0
  if (!attributable) { t = t.slice(0, end); syntax = syntax.slice(0, end) }
  if (syntax.includes('<')) return null
  while (syntax.includes('>') && (m = REDIR_RE.exec(syntax))) {
    t = t.slice(0, m.index); syntax = syntax.slice(0, m.index)
  }
  return { command: t.trim(), attributable }
}

const CLASS_RULES = [
  [
    'test',
    /^(cargo (test|nextest)|npm (run )?test|npx (vitest|jest)|pytest|go test|node --test|make test)\b/,
  ],
  ['build', /^(cargo build|npm run build|npx tsc|tsc|go build|make(\s|$)|cargo check)\b/],
  [
    'lint',
    /^(cargo (clippy|fmt)|npm run (lint|format)|npx (eslint|prettier)|ruff|eslint|prettier)\b/,
  ],
  ['run', /^(cargo run|npm (run )?(dev|start)|npx vite|python3? \S)\b/],
]

export function classifyCommand(text) {
  const t = String(text ?? '').trim()
  for (const [cls, re] of CLASS_RULES) if (re.test(t)) return cls
  return null
}

export const CUT_INDEPENDENT_SOURCES = Object.freeze(['default', 'env'])

export function cutSample(row) {
  if (row?.kind !== 'authored' || !row?.repo?.key) return null
  if (!isMachineLabeled(row)) return null
  const m = row.machine
  const text = String(m.cmd ?? '').trim()
  if (!text) return null
  return {
    text,
    ok: Number(m.exit) === 0,
    ts: Number(row.ts) || 0,
    session: row.session ?? 0,
    dur: Number(m.dur_ms) || 0,
    independent: CUT_INDEPENDENT_SOURCES.includes(String(m.source ?? '')),
  }
}

const isCmdEvent = (r) => r?.kind === 'event' && r?.event === 'cmd' && r?.repo?.key && r?.cmd?.text
const isTurn = (r) => (r?.kind === 'turn' || r?.kind === 'moa_turn') && r?.repo?.key

export function ledgerOutcome(row, norm) {
  const cmd = row.cmd || {}
  if (typeof cmd.verdict !== 'string') {
    return norm.attributable ? cmd.exit === 0 && !cmd.timed_out : null
  }
  if (cmd.verdict === 'no_verdict') return null
  if (cmd.verdict !== 'pass' && cmd.verdict !== 'fail') return null
  if (norm.attributable) return cmd.verdict === 'pass'

  const text = String(cmd.text ?? '')
  const syntax = shellSyntax(text)
  if (
    cmd.verdict === 'pass' &&
    cmd.pipefail === true &&
    syntax !== null &&
    !syntax.includes(';') &&
    !syntax.includes('||')
  ) {
    return true
  }
  return null
}

export function ledgerIndependent(cmd) {
  if (typeof cmd?.independent === 'boolean') return cmd.independent
  if (typeof cmd?.source === 'string') return cmd.source !== 'dossier'
  return true
}

export function mineRepoFacts(rows, cutRows = []) {
  const repos = {}
  const repo = (r) =>
    (repos[r.repo.key] ||= {
      root: r.repo.root ?? null,
      slug: r.repo.slug ?? null,
      byClass: {},
      byText: {},
      rituals: [],
      traps: [],
      thread: null,
    })
  const push = (rec, sample) => {
    const cls = classifyCommand(sample.text)
    if (cls) (rec.byClass[cls] ||= []).push(sample)
    ;(rec.byText[sample.text] ||= []).push(sample)
  }

  for (const r of rows || []) {
    if (isCmdEvent(r)) {
      const rec = repo(r)
      const norm = normalizeCommand(r.cmd.text, r.repo)
      if (!norm?.command) continue
      const ok = ledgerOutcome(r, norm)
      push(rec, {
        text: norm.command,
        ok,
        ts: Number(r.ts) || 0,
        session: r.session ?? 0,
        dur: Number(r.cmd.dur_ms) || 0,
        independent: ledgerIndependent(r.cmd),
      })
    } else if (isTurn(r)) {
      const rec = repo(r)
      const t = {
        ts: Number(r.ts) || 0,
        stop: r.outcome?.stop ?? r.outcome?.route ?? null,
        driver: r.driver ?? null,
        ok: r.outcome?.ok ?? null,
      }
      if (!rec.thread || t.ts > rec.thread.ts) rec.thread = t
    }
  }

  for (const r of cutRows || []) {
    const sample = cutSample(r)
    if (sample) push(repo(r), sample)
  }

  for (const rec of Object.values(repos)) {
    for (const [cls, samples] of Object.entries(rec.byClass)) {
      const indep = samples.filter((s) => s.independent)
      const sessions = new Set(indep.map((s) => s.session)).size
      if (indep.length < RITUAL_MIN_RUNS || sessions < RITUAL_MIN_SESSIONS) continue
      const freq = (list) => {
        const m = new Map()
        for (const s of list) m.set(s.text, (m.get(s.text) || 0) + 1)
        return [...m.entries()].sort((a, b) => b[1] - a[1] || (a[0] < b[0] ? -1 : 1))[0]?.[0]
      }
      const canonical = freq(indep.filter((s) => s.ok === true)) ?? freq(indep)
      const own = indep.filter((s) => s.text === canonical)
      const ownSessions = new Set(own.map((s) => s.session)).size
      if (own.length < RITUAL_MIN_RUNS || ownSessions < RITUAL_MIN_SESSIONS) continue
      const dissent = samples.filter(
        (s) => !s.independent && s.text === canonical && s.ok === false,
      )
      const contributing = [...own, ...dissent]
      const verdicts = contributing.filter((s) => s.ok !== null).length
      const passes = contributing.filter((s) => s.ok === true).length
      rec.rituals.push({
        class: cls,
        canonical,
        runs: contributing.length,
        verdicts,
        passes,
        fails: verdicts - passes,
        sessions: ownSessions,
        lastSeen: Math.max(...own.map((s) => s.ts)),
        meanDurMs: Math.round(own.reduce((a, s) => a + s.dur, 0) / own.length),
      })
    }
    for (const [text, samples] of Object.entries(rec.byText)) {
      const contributing = samples.filter((s) => s.independent || s.ok === false)
      const verdicts = contributing.filter((s) => s.ok !== null)
      if (verdicts.length === 0) continue
      const fails = verdicts.filter((s) => !s.ok).length
      const passes = verdicts.length - fails
      const sessions = new Set(verdicts.map((s) => s.session)).size
      if (fails < TRAP_MIN_FAILS || sessions < RITUAL_MIN_SESSIONS) continue
      if (passes / verdicts.length > TRAP_MAX_PASS_RATE) continue
      rec.traps.push({
        text,
        runs: contributing.length,
        verdicts: verdicts.length,
        passes,
        fails,
        sessions,
        lastSeen: Math.max(...contributing.map((s) => s.ts)),
      })
    }
    rec.rituals.sort((a, b) => (a.class < b.class ? -1 : 1))
    rec.traps.sort((a, b) => b.fails - a.fails || (a.text < b.text ? -1 : 1))
    delete rec.byClass
    delete rec.byText
  }

  return repos
}

const ritualId = (repoKey, f) => `hyp_dossier_${repoKey}_ritual_${slug(f.class)}`
const trapId = (repoKey, f) => `hyp_dossier_${repoKey}_trap_${slug(f.text)}`

export function refreshFact(graph, id, fields, now) {
  const previous = graph.getNode(id)
  const stepTexts = (fact) => fact.factSteps?.map((step) => step.text) ?? null
  const changed = previous.factText !== fields.factText ||
    JSON.stringify(stepTexts(previous)) !== JSON.stringify(stepTexts(fields))
  if (changed) {
    for (const edge of graph.queryEdges((e) =>
      e.dst === id && [EDGE_TYPE.SUPPORTS, EDGE_TYPE.CONTRADICTS, EDGE_TYPE.TESTS].includes(e.type),
    )) graph.removeEdge(edge.id)
  }
  return graph.updateNode(id, {
    ...fields,
    lastSeenAt: now,
    ...(changed ? { lastVerifiedAt: null, outcome: 'neutral', status: 'testing', concludedAt: null } : {}),
  })
}

export function ingestRepoFacts(graph, repoKey, mined, opts = {}) {
  const now = opts.now || new Date().toISOString()
  let nodes = 0
  let updated = 0
  const hypotheses = []

  const fold = (id, fields) => {
    hypotheses.push(id)
    if (graph.hasNode(id)) {
      refreshFact(graph, id, fields, now)
      updated++
      return
    }
    graph.addNode({
      id,
      type: NODE_TYPE.HYPOTHESIS,
      hypothesisId: id.replace(/^hyp_/, ''),
      projectId: DOSSIER_PROJECT.id,
      projectLabel: DOSSIER_PROJECT.label,
      metric: { name: `dossier-${repoKey}`, direction: 'higher' },
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

  for (const f of mined.rituals || []) {
    fold(ritualId(repoKey, f), {
      factKind: 'ritual',
      factClass: f.class,
      factText: f.canonical,
      label: `Ritual: ${f.class} = \`${f.canonical}\``,
      question: `Is \`${f.canonical}\` still this repo's working ${f.class} command?`,
      prediction: `Running \`${f.canonical}\` in ${mined.root ?? repoKey} succeeds`,
      observed: {
        runs: f.runs,
        verdicts: f.verdicts,
        passes: f.passes,
        fails: f.fails,
        sessions: f.sessions,
        lastSeen: f.lastSeen,
        meanDurMs: f.meanDurMs,
      },
    })
  }
  for (const f of mined.traps || []) {
    fold(trapId(repoKey, f), {
      factKind: 'trap',
      factClass: null,
      factText: f.text,
      label: `Trap: \`${f.text}\` fails here`,
      question: `Does \`${f.text}\` still fail in ${mined.root ?? repoKey}?`,
      prediction: `Running \`${f.text}\` in ${mined.root ?? repoKey} fails`,
      observed: {
        runs: f.runs,
        verdicts: f.verdicts,
        passes: f.passes,
        fails: f.fails,
        sessions: f.sessions,
        lastSeen: f.lastSeen,
      },
    })
  }

  return { graph, added: { nodes, updated }, hypotheses }
}

export function dossierFacts(graph, repoKey) {
  return graph
    .nodesOfType(NODE_TYPE.HYPOTHESIS)
    .filter(isDossier)
    .filter((n) => !repoKey || n.repoKey === repoKey)
}

const clamp = (p, lo = 0.02, hi = 0.98) => Math.min(hi, Math.max(lo, p))

export function factBelief(graph, node, opts = {}) {
  return factBeliefFromProbability(node, opts, beliefProbability(graph, node.id))
}

export function factBeliefs(graph, nodes, opts = {}) {
  if (!nodes.length) return new Map()
  const probabilities = beliefProbabilities(graph)
  return new Map(nodes.map(node => [node.id, factBeliefFromProbability(node, opts, probabilities.get(node.id) ?? 0.5)]))
}

function factBeliefFromProbability(node, opts, probability) {
  const nowMs = opts.now ? Date.parse(opts.now) : Date.now()
  const halfLife = opts.halfLifeDays ?? DEFAULT_HALF_LIFE_DAYS
  const o = node.observed || {}
  const verdicts = Number(o.verdicts ?? o.runs) || 0

  let obsShift = 0
  if (verdicts > 0) {
    const evidenceRate = node.factKind === 'trap' ? o.fails / verdicts : o.passes / verdicts
    const saturation = Math.min(1, Math.log2(1 + verdicts) / 4)
    obsShift = (evidenceRate - 0.5) * OBSERVATIONAL_WEIGHT * saturation * 2
  }
  const edgeShift = probability - 0.5
  const raw = clamp(0.5 + obsShift + edgeShift)

  const lastConfirmed = Date.parse(node.lastVerifiedAt || '') || (Number(o.lastSeen) || 0) * 1000
  const ageDays = lastConfirmed > 0 ? Math.max(0, (nowMs - lastConfirmed) / 86_400_000) : halfLife
  const decay = Math.pow(0.5, ageDays / halfLife)
  return { belief: clamp(0.5 + (raw - 0.5) * decay), ageDays }
}

export function compileDossier(graph, repoKey, opts = {}) {
  const nodes = dossierFacts(graph, repoKey)
  const probabilities = nodes.length ? beliefProbabilities(graph) : new Map()
  const order = { test: 0, build: 1, lint: 2, run: 3 }
  const facts = nodes
    .map((n) => {
      const { belief, ageDays } = factBeliefFromProbability(n, opts, probabilities.get(n.id) ?? 0.5)
      const o = n.observed || {}
      const when = o.lastSeen ? new Date(o.lastSeen * 1000).toISOString().slice(0, 10) : 'unknown'
      const verdicts = Number(o.verdicts ?? o.runs) || 0
      const evidence =
        verdicts === 0
          ? `${o.runs ?? 0} runs, no attributable verdict, ` +
            `${o.sessions ?? 0} session(s), last ${when}`
          : `${o.runs ?? 0} runs, ${o.passes ?? 0} pass` +
            (n.factKind === 'trap' ? ` (${o.fails ?? 0} fail)` : '') +
            `, ${o.sessions ?? 0} session(s), last ${when}`
      return {
        id: n.id,
        kind: n.factKind,
        class: n.factClass,
        text: n.factText,
        meanDurMs: o.meanDurMs ?? null,
        belief: Number(belief.toFixed(3)),
        ageDays: Number(ageDays.toFixed(1)),
        verdicts,
        evidence,
        lastVerifiedAt: n.lastVerifiedAt ?? null,
      }
    })
    .sort(
      (a, b) =>
        (a.kind === 'trap' ? 1 : 0) - (b.kind === 'trap' ? 1 : 0) ||
        (order[a.class] ?? 9) - (order[b.class] ?? 9) ||
        b.belief - a.belief,
    )
  const root = nodes[0]?.repoRoot ?? null
  const repoSlug = nodes[0]?.repoSlug ?? null
  return {
    v: 1,
    repo: { key: repoKey, root, slug: repoSlug },
    generatedAt: opts.now || new Date().toISOString(),
    facts,
    thread: opts.thread ?? null,
  }
}

export function proposeDossierProbe(graph, repoKey, opts = {}) {
  const scored = scoreHypotheses(graph, opts)
  const lastAttempt = id => {
    const ms = Date.parse(graph.getNode(id)?.lastProbedAt)
    return Number.isFinite(ms) ? ms : 0
  }
  const ranked = scored
    .map((row) => {
      const node = graph.getNode(row.id)
      if (!isDossier(node) || (repoKey && node.repoKey !== repoKey)) return null
      const { belief, ageDays } = factBeliefFromProbability(node, opts, row.p)
      const staleness = 1 + Math.min(4, ageDays / (opts.halfLifeDays ?? DEFAULT_HALF_LIFE_DAYS))
      return {
        hypothesisId: row.id,
        label: node.label,
        factKind: node.factKind,
        text: node.factText,
        repoKey: node.repoKey,
        belief,
        score: row.score,
        priority: row.score * staleness,
        rationale: `${row.rationale} · belief ${belief.toFixed(2)} · ${ageDays.toFixed(0)}d old`,
      }
    })
    .filter(Boolean)
    .sort((a, b) => b.priority - a.priority ||
      lastAttempt(a.hypothesisId) - lastAttempt(b.hypothesisId) ||
      (a.hypothesisId < b.hypothesisId ? -1 : 1))
  return { proposal: ranked[0] ?? null, ranking: ranked.slice(0, opts.limit ?? 8), dryRun: true }
}
