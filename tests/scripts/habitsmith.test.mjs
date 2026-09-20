// Tests for habitsmith H1/H2: D1 cmd events → per-repo workflow facts.
// Fixture sessions are built step-by-step with explicit seq numbers, since
// ORDER is the whole point. The coexistence case proves habit facts and
// dossier facts share the graph without leaking into each other's queues.

import { test } from 'node:test'
import assert from 'node:assert/strict'

import CausalGraph, { NODE_TYPE, EDGE_TYPE } from '../../lib/research/CausalGraph.js'
import {
  sessionStepSeqs,
  matchPattern,
  mineWorkflows,
  ingestWorkflows,
  habitFacts,
  workflowBelief,
  workflowId,
  chainSlug,
  extractSkillSteps,
  stepJaccard,
  skillName,
  renderSkillMd,
  proposeSkills,
  HABIT_PROJECT,
  WORKFLOW_MIN_SESSIONS,
  MAX_GAP,
  RISKY_RE,
  DEFAULT_MIN_BELIEF,
} from '../../scripts/habitsmith.mjs'
import {
  mineSkillUsage,
  attachSkillUsage,
  foldVerdicts,
  compileHabitsStatus,
  DRIFT_BELIEF,
} from '../../scripts/habitsmith.mjs'
import { killed, factMapFrom } from '../../scripts/habitsmith-tick.mjs'
import {
  dossierFacts,
  ingestRepoFacts,
  mineRepoFacts,
  proposeDossierProbe,
} from '../../scripts/repo-dossier.mjs'

const NOW = '2026-07-06T00:00:00.000Z'
const NOW_TS = Math.floor(Date.parse(NOW) / 1000)
const KEY = 'home-u-proj-0011223344556677'

// One D1 cmd-event line, as experience.rs writes it.
let autoSeq = 0
const cmd = (
  text,
  exit,
  { key = KEY, session = 1, seq = ++autoSeq, ts = NOW_TS, dur = 100 } = {},
) => ({
  kind: 'event',
  v: 1,
  ts,
  session,
  seq,
  event: 'cmd',
  repo: { key, root: '/home/u/proj', slug: 'u/proj' },
  cmd: { text, exit, timed_out: false, dur_ms: dur, tool: 'shell', bytes_out: 10 },
})

// One session that runs the canonical habit: build → test → run, optionally
// with `noise` unrelated commands wedged between build and test.
const habitSession = (session, { noise = 0, buildExit = 0, testExit = 0, runExit = 0 } = {}) => {
  const rows = [cmd('cargo build', buildExit, { session })]
  for (let i = 0; i < noise; i++) rows.push(cmd(`echo noise-${i}`, 0, { session }))
  rows.push(cmd('cargo test -p cockpit', testExit, { session }))
  rows.push(cmd('npm run dev', runExit, { session }))
  return rows
}

test('sessionStepSeqs: classifies, collapses retries to the last attempt, nulls truncated steps', () => {
  const rows = [
    cmd('cargo build', 101, { session: 1 }),
    cmd('cargo build', 0, { session: 1 }), // retry-until-pass = one step, final ok wins
    cmd('export KEY=… && deploy', 0, { session: 1 }), // masked → id:null, keeps its slot
    cmd('cargo test', 0, { session: 1 }),
  ]
  const seq = sessionStepSeqs(rows)[KEY].sessions[1]
  assert.equal(seq.length, 3)
  assert.deepEqual(
    seq.map((s) => s.id),
    ['build', null, 'test'],
  )
  assert.equal(seq[0].ok, true, 'collapsed retry keeps the final attempt')
})

test('matchPattern: gap tolerance is the knob — ≤MAX_GAP matches, more breaks it', () => {
  const seq = (noise) => sessionStepSeqs(habitSession(1, { noise }))[KEY].sessions[1]
  assert.equal(matchPattern(seq(0), ['build', 'test']).length, 1)
  assert.equal(matchPattern(seq(MAX_GAP), ['build', 'test']).length, 1)
  assert.equal(matchPattern(seq(MAX_GAP + 1), ['build', 'test']).length, 0)
})

test('matchPattern: occurrences are leftmost-greedy and non-overlapping', () => {
  const rows = [...habitSession(1), ...habitSession(1)]
  const seq = sessionStepSeqs(rows)[KEY].sessions[1]
  assert.equal(matchPattern(seq, ['build', 'test']).length, 2)
})

test('mining finds the habit across sessions, with per-step pass rates and clean-run count', () => {
  const rows = [
    ...habitSession(1),
    ...habitSession(2, { noise: 2 }), // gap-tolerant occurrence
    ...habitSession(3, { testExit: 101 }), // dirty run: test step failed
  ]
  const mined = mineWorkflows(rows)
  const wfs = mined[KEY].workflows
  assert.equal(
    wfs.length,
    1,
    `closed pruning leaves only the maximal chain: ${JSON.stringify(wfs)}`,
  )
  const wf = wfs[0]
  assert.deepEqual(
    wf.steps.map((s) => s.class),
    ['build', 'test', 'run'],
  )
  assert.equal(wf.steps[0].text, 'cargo build')
  assert.equal(wf.steps[1].text, 'cargo test -p cockpit')
  assert.equal(wf.support, 3)
  assert.equal(wf.sessions, 3)
  assert.equal(wf.passes, 2, 'the session whose test failed is not a clean pass')
  assert.equal(wf.steps[1].passRate, Number((2 / 3).toFixed(3)))
  assert.equal(wf.steps[0].passRate, 1)
})

test('below the session floor nothing is mined', () => {
  const rows = [...habitSession(1), ...habitSession(2)]
  const mined = mineWorkflows(rows)
  assert.equal(mined[KEY].workflows.length, 0, `needs ${WORKFLOW_MIN_SESSIONS} sessions`)
})

test('order threshold: a sequence that flips direction too often is rejected', () => {
  // 3 sessions build→test, 2 sessions test→build: 3/5 = 60% < 70% → no fact.
  const fwd = (s) => [cmd('cargo build', 0, { session: s }), cmd('cargo test', 0, { session: s })]
  const rev = (s) => [cmd('cargo test', 0, { session: s }), cmd('cargo build', 0, { session: s })]
  const flippy = mineWorkflows([...fwd(1), ...fwd(2), ...fwd(3), ...rev(4), ...rev(5)])
  assert.equal(flippy[KEY].workflows.length, 0)
  // 5 fwd, 2 rev: 5/7 ≈ 71% ≥ 70% → the habit stands.
  const steady = mineWorkflows([
    ...fwd(1),
    ...fwd(2),
    ...fwd(3),
    ...fwd(4),
    ...fwd(5),
    ...rev(6),
    ...rev(7),
  ])
  assert.equal(steady[KEY].workflows.length, 1)
  assert.deepEqual(
    steady[KEY].workflows[0].steps.map((s) => s.class),
    ['build', 'test'],
  )
})

test('truncated/masked commands never become skill steps', () => {
  const rows = [1, 2, 3].flatMap((s) => [
    cmd('cargo build', 0, { session: s }),
    cmd('export TOKEN=… && curl api', 0, { session: s }), // recurs every session, but masked
    cmd('cargo test', 0, { session: s }),
  ])
  const mined = mineWorkflows(rows)
  const texts = JSON.stringify(mined[KEY].workflows)
  assert.ok(!texts.includes('…'), `masked step leaked into a workflow: ${texts}`)
  assert.equal(mined[KEY].workflows.length, 1, 'build→test still mined around the masked slot')
})

test('closed pruning: sub-chains with the same support are folded into the maximal chain', () => {
  const rows = [1, 2, 3, 4].flatMap((s) => habitSession(s))
  const wfs = mineWorkflows(rows)[KEY].workflows
  assert.equal(wfs.length, 1)
  assert.equal(wfs[0].steps.length, 3)
  // …but a sub-chain that occurs MORE often than the full chain survives on its own.
  const extra = [5, 6, 7].flatMap((s) => [
    cmd('cargo build', 0, { session: s }),
    cmd('cargo test', 0, { session: s }),
  ])
  const wfs2 = mineWorkflows([...rows, ...extra])[KEY].workflows
  const chains = wfs2.map((w) => chainSlug(w.steps)).sort()
  assert.deepEqual(chains, ['build-test', 'build-test-run'])
})

test('ingest is idempotent, ids embed the repo key, facts are perpetual-shaped', () => {
  const g = new CausalGraph()
  const mined = mineWorkflows([1, 2, 3].flatMap((s) => habitSession(s)))
  const first = ingestWorkflows(g, KEY, mined[KEY], { now: NOW })
  assert.equal(first.added.nodes, 1)
  const again = ingestWorkflows(g, KEY, mined[KEY], { now: NOW })
  assert.equal(again.added.nodes, 0)
  assert.equal(again.added.updated, 1)

  const [fact] = habitFacts(g, KEY)
  assert.equal(fact.type, NODE_TYPE.HYPOTHESIS)
  assert.equal(fact.projectId, HABIT_PROJECT.id)
  assert.equal(fact.factKind, 'workflow')
  assert.equal(fact.status, 'testing')
  assert.ok(fact.id.includes(KEY), 'repo key rides verbatim in the id')
  assert.equal(fact.id, workflowId(KEY, mined[KEY].workflows[0]))
  assert.equal(fact.factText, 'cargo build && cargo test -p cockpit && npm run dev')
  assert.equal(fact.factSteps.length, 3)
  assert.equal(fact.observed.runs, 3)
})

test('belief: clean workflow > 0.5 and capped; a probe edge lifts it; evidence rate is all-steps-passed', () => {
  const g = new CausalGraph()
  const clean = mineWorkflows([1, 2, 3].flatMap((s) => habitSession(s)))
  ingestWorkflows(g, KEY, clean[KEY], { now: NOW })
  const [fact] = habitFacts(g, KEY)
  const b = workflowBelief(g, fact, { now: NOW }).belief
  assert.ok(b > 0.5 && b <= 0.7, `observational belief capped (${b})`)

  // A workflow whose runs never fully pass sits at/below neutral.
  const g2 = new CausalGraph()
  const dirty = mineWorkflows([1, 2, 3].flatMap((s) => habitSession(s, { runExit: 1 })))
  ingestWorkflows(g2, KEY, dirty[KEY], { now: NOW })
  const b2 = workflowBelief(g2, habitFacts(g2, KEY)[0], { now: NOW }).belief
  assert.ok(b2 <= 0.5, `no clean run → belief at or below neutral (${b2})`)

  // A SUPPORTS edge (approval / probe) raises belief past the observational cap.
  g.addNode({ id: 'exp_habit_probe', type: NODE_TYPE.EXPERIMENT, label: 'probe' })
  g.addEdge({ src: 'exp_habit_probe', dst: fact.id, type: EDGE_TYPE.SUPPORTS, confidence: 0.9 })
  g.updateNode(fact.id, { lastVerifiedAt: NOW })
  const lifted = workflowBelief(g, g.getNode(fact.id), { now: NOW }).belief
  assert.ok(lifted > 0.75, `verified workflow belief rises (${lifted})`)
})

// ─── H3: skill compiler ──────────────────────────────────────────────────────

// Enough clean sessions that observational belief clears the 0.75 gate (~5+).
const confidentGraph = () => {
  const g = new CausalGraph()
  const rows = [1, 2, 3, 4, 5, 6].flatMap((s) => habitSession(s))
  ingestWorkflows(g, KEY, mineWorkflows(rows)[KEY], { now: NOW })
  return g
}

test('renderSkillMd: deterministic draft that the harness frontmatter rules can parse', () => {
  const g = confidentGraph()
  const [fact] = habitFacts(g, KEY)
  const a = renderSkillMd(fact, { graph: g, now: NOW })
  const b = renderSkillMd(fact, { graph: g, now: NOW })
  assert.equal(a.markdown, b.markdown, 'render is deterministic')

  assert.equal(a.name, 'proj-build-test-run', 'repo slug (last segment) always in the name')
  assert.equal(skillName(fact), a.name)
  // Frontmatter exactly as harness/skills.rs parse_skill reads it.
  assert.match(a.markdown, /^---\nname: proj-build-test-run\ndescription: .+/)
  assert.match(a.markdown, new RegExp(`fact: ${fact.id}`), 'provenance rides in frontmatter')
  assert.ok(!a.markdown.includes('risky:'), 'clean steps → no risky tag')
  // Numbered steps with the canonical commands, then the evidence footer.
  assert.match(a.markdown, /1\. `cargo build` \(build, passes 100%\)/)
  assert.match(a.markdown, /Evidence: 6 run\(s\) across 6 session\(s\), 6 clean end-to-end/)
  assert.match(a.markdown, /belief 0\.\d\d/)
})

test('risky verbs annotate the step and tag the skill; --release flag is NOT risky', () => {
  assert.ok(RISKY_RE.test('git push origin main'))
  assert.ok(RISKY_RE.test('rm -rf dist'))
  assert.ok(RISKY_RE.test('npm publish'))
  assert.ok(RISKY_RE.test('cargo release'))
  assert.ok(RISKY_RE.test('curl -X POST http://x'))
  assert.ok(!RISKY_RE.test('cargo build --release'), 'the --release flag is a build, not a release')
  assert.ok(!RISKY_RE.test('cargo test -p cockpit'))

  const g = new CausalGraph()
  const rows = [1, 2, 3, 4, 5, 6].flatMap((s) => [
    cmd('cargo build --release', 0, { session: s }),
    cmd('git push origin main', 0, { session: s }),
  ])
  ingestWorkflows(g, KEY, mineWorkflows(rows)[KEY], { now: NOW })
  const [fact] = habitFacts(g, KEY)
  const draft = renderSkillMd(fact, { graph: g, now: NOW })
  assert.match(draft.markdown, /scope: project/)
  assert.match(draft.markdown, new RegExp(`repo_key: "${KEY}"`))
  assert.match(draft.markdown, /repo_root: "\/home\/u\/proj"/)
  assert.equal(draft.risky, true)
  assert.match(draft.markdown, /risky: true/)
  assert.match(draft.markdown, /`git push origin main` .*⚠ requires explicit confirmation/)
  assert.ok(!/`cargo build --release`.*⚠/.test(draft.markdown))
})

test('extractSkillSteps pulls fenced and inline code; stepJaccard measures overlap', () => {
  const steps = extractSkillSteps(
    '# x\n\nRun `cargo build` first.\n\n```bash\n# comment\ncargo  test\nnpm run dev\n```\n',
  )
  assert.deepEqual([...steps].sort(), ['cargo build', 'cargo test', 'npm run dev'])
  assert.equal(stepJaccard(steps, new Set(['cargo build', 'cargo test', 'npm run dev'])), 1)
  assert.equal(stepJaccard(new Set(['a']), new Set(['b'])), 0)
})

test('proposeSkills: belief gate, dedupe against existing, daily cap — in that order', () => {
  const g = confidentGraph()
  const budget = { date: '2026-07-06', runs: 0, cap: 2 }

  // Confident + novel → proposed.
  const fresh = proposeSkills(g, { now: NOW, budget, existing: [] })
  assert.equal(fresh.proposals.length, 1)
  assert.equal(fresh.budget.runs, 1)
  assert.equal(fresh.proposals[0].factId, habitFacts(g, KEY)[0].id)

  // An exact name collision (our own earlier draft) is a dupe no matter how
  // diluted the other file's step-set is.
  const named = proposeSkills(g, {
    now: NOW,
    budget,
    existing: [{ name: 'proj-build-test-run', steps: new Set(['something else entirely']) }],
  })
  assert.equal(named.proposals.length, 0)
  assert.match(named.skipped[0].reason, /already exists/)

  // Same steps already in a skill (any dir) → skipped with the collision named.
  const dup = proposeSkills(g, {
    now: NOW,
    budget,
    existing: [
      {
        name: 'run-angel0',
        steps: new Set(['cargo build', 'cargo test -p cockpit', 'npm run dev']),
      },
    ],
  })
  assert.equal(dup.proposals.length, 0)
  assert.match(dup.skipped[0].reason, /duplicate of skill 'run-angel0'/)

  // Budget already spent → skipped as budget, not proposed.
  const broke = proposeSkills(g, {
    now: NOW,
    budget: { date: '2026-07-06', runs: 2, cap: 2 },
    existing: [],
  })
  assert.equal(broke.proposals.length, 0)
  assert.match(broke.skipped[0].reason, /budget/)

  // Not confident enough (3 sessions only) → belief gate names the number.
  const g2 = new CausalGraph()
  ingestWorkflows(g2, KEY, mineWorkflows([1, 2, 3].flatMap((s) => habitSession(s)))[KEY], {
    now: NOW,
  })
  const timid = proposeSkills(g2, { now: NOW, budget, existing: [] })
  assert.equal(timid.proposals.length, 0)
  assert.match(timid.skipped[0].reason, new RegExp(`belief 0\\.\\d\\d < ${DEFAULT_MIN_BELIEF}`))
})

test('a rejected workflow (CONTRADICTS from /habits) never re-proposes via the belief gate', () => {
  const g = confidentGraph()
  const [fact] = habitFacts(g, KEY)
  g.addNode({ id: 'exp_habit_reject', type: NODE_TYPE.EXPERIMENT, label: 'user reject' })
  g.addEdge({
    src: 'exp_habit_reject',
    dst: fact.id,
    type: EDGE_TYPE.CONTRADICTS,
    confidence: 0.9,
  })
  const { proposals, skipped } = proposeSkills(g, {
    now: NOW,
    budget: { date: '2026-07-06', runs: 0, cap: 2 },
    existing: [],
  })
  assert.equal(proposals.length, 0)
  assert.match(skipped[0].reason, /belief/)
})

// ─── H5: the closed loop ─────────────────────────────────────────────────────

// One skill-load ledger row, as experience.rs's skill_event_record writes it.
const skillEvent = (name, ok, { ts = NOW_TS, session = 9 } = {}) => ({
  kind: 'event',
  v: 1,
  ts,
  session,
  seq: ++autoSeq,
  event: 'skill',
  repo: { key: KEY, root: '/home/u/proj', slug: 'u/proj' },
  skill: { name, ok, dur_ms: 2 },
})

test('mineSkillUsage counts loads per skill; attachSkillUsage pins them to the spawning fact', () => {
  const usage = mineSkillUsage([
    skillEvent('proj-build-test-run', true),
    skillEvent('proj-build-test-run', true, { ts: NOW_TS + 60 }),
    skillEvent('proj-build-test-run', false),
    skillEvent('unrelated-skill', true),
    cmd('cargo build', 0, { session: 9 }), // cmd rows are not usage
  ])
  assert.deepEqual(usage[`${KEY}\u0000proj-build-test-run`], {
    uses: 3,
    ok: 2,
    lastUsed: NOW_TS + 60,
  })
  assert.equal(usage[`${KEY}\u0000unrelated-skill`].uses, 1)

  const g = confidentGraph()
  const [fact] = habitFacts(g, KEY)
  const n = attachSkillUsage(g, usage, {
    [`${KEY}\u0000proj-build-test-run`]: fact.id,
    'orphan-skill': 'hyp_habit_gone',
  })
  assert.equal(n, 1, 'orphan fact ids attach nothing')
  assert.equal(g.getNode(fact.id).skillUsage.uses, 3)
})

test('foldVerdicts: approve lifts belief past the cap, reject sinks it; facts stay perpetual', () => {
  const g = confidentGraph()
  const [fact] = habitFacts(g, KEY)
  const before = workflowBelief(g, fact, { now: NOW }).belief

  const { folded, orphaned } = foldVerdicts(
    g,
    [
      { ts: NOW_TS, action: 'approve', name: 'proj-build-test-run', fact: fact.id },
      { ts: NOW_TS, action: 'reject', name: 'ghost', fact: 'hyp_habit_missing' },
      { ts: NOW_TS, action: 'approve', name: 'no-fact', fact: null },
    ],
    { now: NOW },
  )
  assert.deepEqual(folded, [fact.id])
  assert.deepEqual(orphaned, ['ghost', 'no-fact'])

  const node = g.getNode(fact.id)
  assert.equal(node.status, 'testing', 'verdict edges never conclude a fact')
  assert.equal(node.lastVerifiedAt, NOW)
  const after = workflowBelief(g, node, { now: NOW }).belief
  assert.ok(after > before && after > 0.8, `approval lifts belief (${before} → ${after})`)

  // A reject on a fresh twin pins it under the propose gate — the actual
  // never-re-propose invariant. (A single CONTRADICTS 0.9 gives edgeShift
  // ≈ −0.21; even maxed-out observational evidence (+0.4) can't reach 0.75.)
  const g2 = confidentGraph()
  const [fact2] = habitFacts(g2, KEY)
  foldVerdicts(g2, [{ ts: NOW_TS, action: 'reject', name: 'x', fact: fact2.id }], { now: NOW })
  const sunk = workflowBelief(g2, g2.getNode(fact2.id), { now: NOW }).belief
  assert.ok(sunk < before, `rejection lowers belief (${before} → ${sunk})`)
  assert.ok(
    sunk < DEFAULT_MIN_BELIEF,
    `rejected fact sits under the propose gate for good (${sunk})`,
  )
})

test('compileHabitsStatus flags drift only when a USED skill’s belief has decayed', () => {
  const g = confidentGraph()
  const [fact] = habitFacts(g, KEY)

  // Confident and used: not drifting.
  attachSkillUsage(g, { s: { uses: 4, ok: 4, lastUsed: NOW_TS } }, { s: fact.id })
  const fresh = compileHabitsStatus(g, { now: NOW })
  assert.equal(fresh.facts.length, 1)
  assert.equal(fresh.facts[0].drifting, false)
  assert.equal(fresh.facts[0].name, 'proj-build-test-run')

  // Two half-lives later, unverified: belief decays under DRIFT_BELIEF while
  // the usage stays — that combination is drift.
  const later = new Date(Date.parse(NOW) + 40 * 86_400_000).toISOString()
  const stale = compileHabitsStatus(g, { now: later })
  assert.ok(stale.facts[0].belief < DRIFT_BELIEF, `belief decayed (${stale.facts[0].belief})`)
  assert.equal(stale.facts[0].drifting, true)

  // The same decay WITHOUT usage is just a quiet fact, not drift.
  const g2 = confidentGraph()
  const quiet = compileHabitsStatus(g2, { now: later })
  assert.equal(quiet.facts[0].drifting, false)
  // Round-trips as JSON (written to disk verbatim).
  assert.deepEqual(JSON.parse(JSON.stringify(stale)), stale)
})

test('tick helpers: ANGEL_HABITS=0 kills; factMapFrom reads fact frontmatter', () => {
  assert.equal(killed({ ANGEL_HABITS: '0' }), true)
  assert.equal(killed({ ANGEL_HABITS: '1' }), false)
  assert.equal(killed({}), false)

  const map = factMapFrom([
    {
      name: 'proj-build-test',
      text: '---\nname: proj-build-test\nfact: hyp_habit_k_chain\n---\nbody',
    },
    { name: 'hand-written', text: '---\nname: hand-written\n---\nno fact line' },
    { name: 'flat', text: 'no frontmatter at all' },
  ])
  assert.deepEqual(map, { 'proj-build-test': 'hyp_habit_k_chain' })
})

test('habit facts coexist with dossier facts: neither leaks into the other’s view', () => {
  const g = new CausalGraph()
  const rows = [1, 2, 3].flatMap((s) => habitSession(s))
  ingestWorkflows(g, KEY, mineWorkflows(rows)[KEY], { now: NOW })
  ingestRepoFacts(g, KEY, mineRepoFacts(rows)[KEY], { now: NOW })

  assert.ok(habitFacts(g, KEY).length >= 1)
  assert.ok(dossierFacts(g, KEY).length >= 1)
  assert.ok(habitFacts(g).every((n) => n.id.startsWith('hyp_habit_')))
  assert.ok(dossierFacts(g).every((n) => n.id.startsWith('hyp_dossier_')))
  // The dossier probe queue never surfaces workflow facts.
  const { ranking } = proposeDossierProbe(g, KEY, { now: NOW })
  assert.ok(ranking.every((r) => r.hypothesisId.startsWith('hyp_dossier_')))
})
