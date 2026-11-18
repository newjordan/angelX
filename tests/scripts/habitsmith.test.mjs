import { test } from 'node:test'
import assert from 'node:assert/strict'
import { spawnSync } from 'node:child_process'
import { existsSync, mkdirSync, mkdtempSync, readFileSync, rmSync, writeFileSync } from 'node:fs'
import { tmpdir } from 'node:os'
import { join } from 'node:path'

import * as habitsCore from '../../lib/habits/core.mjs'
import * as habitsWorker from '../../scripts/runtime/habitsmith.mjs'

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
} from '../../lib/habits/core.mjs'
import {
  mineSkillUsage,
  attachSkillUsage,
  foldVerdicts,
  compileHabitsStatus,
  DRIFT_BELIEF,
} from '../../lib/habits/core.mjs'
import { killed, factMapFrom } from '../../scripts/runtime/habitsmith-tick.mjs'
import { tickLockOwner } from '../../scripts/runtime/worker-lock.mjs'
import {
  dossierFacts,
  ingestRepoFacts,
  mineRepoFacts,
  proposeDossierProbe,
} from '../../lib/dossier/core.mjs'

const NOW = '2026-07-06T00:00:00.000Z'
const NOW_TS = Math.floor(Date.parse(NOW) / 1000)
const KEY = 'home-u-proj-0011223344556677'

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
    cmd('cargo build', 0, { session: 1 }),
    cmd('export KEY=… && deploy', 0, { session: 1 }),
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

test('sequence parsing reuse stays scoped to root/cwd and never caches outcomes across rows or calls', () => {
  const text = 'cd /allowed && cargo test'
  const rows = [
    { ...cmd(text, 0, { session: 1 }), repo: { key: KEY, root: '/allowed' } },
    { ...cmd(text, 1, { session: 2 }), repo: { key: KEY, root: '/allowed' } },
    { ...cmd(text, 0, { session: 3 }), repo: { key: 'foreign', root: '/foreign' } },
    { ...cmd(text, 0, { session: 4 }), repo: { key: 'cwd-match', root: '/foreign', cwd: '/allowed' } },
    { ...cmd(text, 0, { session: 5 }), repo: { key: 'cwd-foreign', root: '/foreign', cwd: '/other' } },
  ]
  const parsed = sessionStepSeqs(rows)
  assert.equal(parsed[KEY].sessions[1][0].ok, true)
  assert.equal(parsed[KEY].sessions[2][0].ok, false, 'cached parse must not reuse the previous exit status')
  assert.equal(parsed.foreign.sessions[3][0].id, null)
  assert.notEqual(parsed['cwd-match'].sessions[4][0].id, null)
  assert.equal(parsed['cwd-foreign'].sessions[5][0].id, null)
  rows[0].repo.root = '/foreign'
  assert.equal(sessionStepSeqs(rows)[KEY].sessions[1][0].id, null, 'next call must parse the changed repo stamp anew')
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
  for (const [name, value] of Object.entries(habitsCore)) {
    assert.strictEqual(habitsWorker[name], value, `worker must re-export the core's ${name}`)
  }
  const rows = [
    ...habitSession(1),
    ...habitSession(2, { noise: 2 }),
    ...habitSession(3, { testExit: 101 }),
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

test('unattributable workflow outcomes prove recurrence, not success or failure', () => {
  for (const verdict of ['no_verdict', 'future_verdict']) {
    const rows = [1, 2, 3].flatMap((session) =>
      habitSession(session).map((row) => ({
        ...row,
        v: 3,
        cmd: { ...row.cmd, verdict, pipefail: false },
      })),
    )
    const mined = mineWorkflows(rows)[KEY]
    const wf = mined.workflows[0]
    assert.equal(wf.support, 3)
    assert.equal(wf.passes, 0, verdict)
    assert.equal(wf.verdicts, 0, verdict)
    assert.ok(wf.steps.every((step) => step.passRate === null))
    const graph = new CausalGraph()
    ingestWorkflows(graph, KEY, mined, { now: NOW })
    const fact = habitFacts(graph, KEY)[0]
    assert.equal(fact.observed.fails, 0)
    assert.equal(workflowBelief(graph, fact, { now: NOW }).belief, 0.5)
  }
})

test('workflow verdicts distinguish a failed step from an unknown step', () => {
  const rows = [1, 2, 3].flatMap((session) =>
    habitSession(session).map((row, i) => ({
      ...row,
      v: 3,
      cmd: {
        ...row.cmd,
        verdict: session === 2 && i === 1 ? 'no_verdict' : session === 3 && i === 1 ? 'fail' : 'pass',
      },
    })),
  )
  const wf = mineWorkflows(rows)[KEY].workflows[0]
  assert.equal(wf.support, 3)
  assert.equal(wf.verdicts, 2)
  assert.equal(wf.passes, 1)
  assert.equal(wf.steps[1].passRate, 0.5, 'unknown attempts are outside the pass-rate denominator')
})

test('legacy pipelines without provenance do not certify workflow success', () => {
  const rows = [1, 2, 3].flatMap((session) => [
    cmd('cargo build | tail -1', 0, { session }),
    cmd('cargo test | tail -1', 0, { session }),
  ])
  const wf = mineWorkflows(rows)[KEY].workflows[0]
  assert.equal(wf.passes, 0)
  assert.equal(wf.verdicts, 0)
  const current = rows.map((row) => ({
    ...row,
    v: 3,
    cmd: { ...row.cmd, verdict: 'pass', pipefail: true },
  }))
  assert.equal(mineWorkflows(current)[KEY].workflows[0].passes, 3)
})

test('whole-step verdicts retain compound failures and reject another repo\'s commands', () => {
  const failed = [1, 2, 3].flatMap((session) => [
    { ...cmd('cargo build | cat', 101, { session }), v: 3, cmd: { text: 'cargo build | cat', exit: 101, verdict: 'fail', pipefail: true } },
    cmd('cargo test', 0, { session }),
  ])
  const wf = mineWorkflows(failed)[KEY].workflows[0]
  assert.equal(wf.verdicts, 3, 'the full pipeline is the skill step, so its failure is attributable')
  assert.equal(wf.passes, 0)
  assert.equal(wf.steps[0].passRate, 0)
  const foreign = [1, 2, 3].flatMap((session) => [
    cmd('cd /other/repo && cargo build', 0, { session }),
    cmd('cd /other/repo && cargo test', 0, { session }),
  ])
  assert.equal(mineWorkflows(foreign)[KEY].workflows.length, 0, 'foreign commands cannot become this repo\'s skill')
})

test('self-selected workflow passes cannot reinforce their own belief or freshness', () => {
  const independent = [1, 2, 3].flatMap(s => habitSession(s))
  const echoed = [4, 5, 6, 7].flatMap(s => habitSession(s).map(row => ({
    ...row,
    cmd: { ...row.cmd, independent: false, source: 'dossier' },
    ts: NOW_TS + 3600,
  })))
  const before = mineWorkflows(independent)[KEY]
  const after = mineWorkflows([...independent, ...echoed])[KEY]
  assert.deepEqual(after, before, 'self-selected passes do not change observations')
  assert.equal(mineWorkflows(echoed)[KEY].workflows.length, 0, 'echoes cannot mint a workflow')
  const failed = echoed.slice(0, 3).map((row, i) => ({ ...row, cmd: { ...row.cmd, exit: i === 1 ? 1 : 0 } }))
  const dissent = mineWorkflows([...independent, ...failed])[KEY].workflows[0]
  assert.equal(dissent.passes, 3)
  assert.equal(dissent.verdicts, 4, 'a self-selected failure still disconfirms the existing workflow')
  assert.equal(dissent.lastSeen, before.workflows[0].lastSeen, 'self-selected runs cannot refresh the decay clock')
})

test('self-selected dissent cannot add passing votes to its other workflow steps', () => {
  const independent = [1, 2, 3].flatMap(s => habitSession(s, { buildExit: 1 }))
  const echo = habitSession(4, { testExit: 1 }).map(row => ({
    ...row, cmd: { ...row.cmd, independent: false },
  }))
  const wf = mineWorkflows([...independent, ...echo])[KEY].workflows[0]
  assert.equal(wf.steps[0].passRate, 0, 'an echoed build pass is not independent support')
  assert.equal(wf.steps[1].passRate, 0.75, 'the echoed test failure is real dissent')
})

test('self-selected retries cannot erase preceding independent workflow evidence', () => {
  const independent = [1, 2, 3].flatMap(s => habitSession(s))
  const withRetries = independent.flatMap(row => [row, { ...row, seq: row.seq + 0.5, cmd: { ...row.cmd, independent: false } }])
  assert.deepEqual(mineWorkflows(withRetries)[KEY], mineWorkflows(independent)[KEY])
})

test('workflow provenance does not invent end-to-end passes by combining command variants', () => {
  const rows = [1, 2, 3, 4, 5, 6].flatMap(session => [
    cmd(session % 2 ? 'cargo build -p alpha' : 'cargo build -p beta', 0, { session }),
    cmd(session % 2 ? 'cargo test -p beta' : 'cargo test -p alpha', 0, { session }),
  ])
  const wf = mineWorkflows(rows)[KEY].workflows[0]
  const selected = wf.steps.map(step => step.text)
  assert.ok(selected[0].endsWith('alpha') !== selected[1].endsWith('alpha'), 'canonical steps must come from an actual sequence, not independent modal choices')
  assert.equal(wf.passes, 3, 'only the chosen concrete sequence\'s runs vote')
  assert.equal(wf.verdicts, 3)
})

test('order threshold: a sequence that flips direction too often is rejected', () => {
  const fwd = (s) => [cmd('cargo build', 0, { session: s }), cmd('cargo test', 0, { session: s })]
  const rev = (s) => [cmd('cargo test', 0, { session: s }), cmd('cargo build', 0, { session: s })]
  const flippy = mineWorkflows([...fwd(1), ...fwd(2), ...fwd(3), ...rev(4), ...rev(5)])
  assert.equal(flippy[KEY].workflows.length, 0)
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
    cmd('export TOKEN=… && curl api', 0, { session: s }),
    cmd('cargo test', 0, { session: s }),
  ])
  const mined = mineWorkflows(rows)
  const texts = JSON.stringify(mined[KEY].workflows)
  assert.ok(!texts.includes('…'), `masked step leaked into a workflow: ${texts}`)
  assert.equal(mined[KEY].workflows.length, 1, 'build→test still mined around the masked slot')
})

test('order pruning retains directed cycles and the exact threshold boundary', () => {
  const cycles = [1, 2, 3].flatMap((session) =>
    ['alpha', 'bravo', 'charlie', 'delta', 'echo', 'alpha', 'bravo', 'charlie', 'delta', 'echo'].map(text => cmd(text, 0, { session })),
  )
  const workflows = mineWorkflows(cycles)[KEY].workflows
  assert.ok(workflows.some(wf => wf.steps.map(step => step.text).join(',') === 'alpha,bravo,charlie,delta,echo,alpha'))
  const boundary = (forward) => Array.from({ length: 10 }, (_, session) =>
    (session < forward ? ['cargo build', 'cargo test'] : ['cargo test', 'cargo build'])
      .map(text => cmd(text, 0, { session })),
  ).flat()
  assert.equal(mineWorkflows(boundary(7))[KEY].workflows.length, 1, '70% is included')
  assert.equal(mineWorkflows(boundary(6))[KEY].workflows.length, 0, '60% is excluded')
})

test('closed pruning: sub-chains with the same support are folded into the maximal chain', () => {
  const rows = [1, 2, 3, 4].flatMap((s) => habitSession(s))
  const wfs = mineWorkflows(rows)[KEY].workflows
  assert.equal(wfs.length, 1)
  assert.equal(wfs[0].steps.length, 3)
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

test('a replacement workflow cannot inherit the previous sequence\'s verification', () => {
  const g = new CausalGraph()
  const original = mineWorkflows([1, 2, 3].flatMap((s) => habitSession(s)))[KEY]
  ingestWorkflows(g, KEY, original, { now: NOW })
  const [fact] = habitFacts(g, KEY)
  g.addNode({ id: 'workflow-probe', type: NODE_TYPE.EXPERIMENT })
  g.addEdge({ src: 'workflow-probe', dst: fact.id, type: EDGE_TYPE.SUPPORTS, confidence: 0.85 })
  g.updateNode(fact.id, { lastVerifiedAt: NOW, outcome: 'positive' })
  const changed = structuredClone(original)
  changed.workflows[0].steps[1].text = 'cargo test -p replacement'
  ingestWorkflows(g, KEY, changed, { now: NOW })
  const updated = g.getNode(fact.id)
  assert.match(updated.factText, /replacement/)
  assert.equal(updated.lastVerifiedAt, null)
  assert.equal(updated.outcome, 'neutral')
  assert.equal(g.edges().length, 0)
  assert.ok(g.hasNode('workflow-probe'))
})

test('workflow evidence is bound to step boundaries, not a lossy joined string', () => {
  const g = new CausalGraph()
  const mined = { root: '/home/u/proj', workflows: [{
    steps: [{ text: 'cargo build && echo ready', class: 'build', passRate: 1 }, { text: 'cargo test', class: 'test', passRate: 1 }],
    support: 3, verdicts: 3, passes: 3, sessions: 3, lastSeen: NOW_TS,
  }] }
  ingestWorkflows(g, KEY, mined, { now: NOW })
  const [fact] = habitFacts(g, KEY)
  g.updateNode(fact.id, { lastVerifiedAt: NOW })
  const changed = structuredClone(mined)
  changed.workflows[0].steps[0].text = 'cargo build'
  changed.workflows[0].steps[1].text = 'echo ready && cargo test'
  ingestWorkflows(g, KEY, changed, { now: NOW })
  assert.equal(g.getNode(fact.id).factText, fact.factText, 'joined command strings happen to be identical')
  assert.equal(g.getNode(fact.id).lastVerifiedAt, null, 'step semantics changed despite the equal display string')
})

test('belief: clean workflow > 0.5 and capped; a probe edge lifts it; evidence rate is all-steps-passed', () => {
  const g = new CausalGraph()
  const clean = mineWorkflows([1, 2, 3].flatMap((s) => habitSession(s)))
  ingestWorkflows(g, KEY, clean[KEY], { now: NOW })
  const [fact] = habitFacts(g, KEY)
  const b = workflowBelief(g, fact, { now: NOW }).belief
  assert.ok(b > 0.5 && b <= 0.7, `observational belief capped (${b})`)

  const g2 = new CausalGraph()
  const dirty = mineWorkflows([1, 2, 3].flatMap((s) => habitSession(s, { runExit: 1 })))
  ingestWorkflows(g2, KEY, dirty[KEY], { now: NOW })
  const b2 = workflowBelief(g2, habitFacts(g2, KEY)[0], { now: NOW }).belief
  assert.ok(b2 <= 0.5, `no clean run → belief at or below neutral (${b2})`)

  g.addNode({ id: 'exp_habit_probe', type: NODE_TYPE.EXPERIMENT, label: 'probe' })
  g.addEdge({ src: 'exp_habit_probe', dst: fact.id, type: EDGE_TYPE.SUPPORTS, confidence: 0.9 })
  g.updateNode(fact.id, { lastVerifiedAt: NOW })
  const lifted = workflowBelief(g, g.getNode(fact.id), { now: NOW }).belief
  assert.ok(lifted > 0.75, `verified workflow belief rises (${lifted})`)
})

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
  assert.match(a.markdown, /^---\nname: proj-build-test-run\ndescription: .+/)
  assert.match(a.markdown, new RegExp(`fact: ${fact.id}`), 'provenance rides in frontmatter')
  assert.ok(!a.markdown.includes('risky:'), 'clean steps → no risky tag')
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

  const fresh = proposeSkills(g, { now: NOW, budget, existing: [] })
  assert.equal(fresh.proposals.length, 1)
  assert.equal(fresh.budget.runs, 1)
  assert.equal(fresh.proposals[0].factId, habitFacts(g, KEY)[0].id)

  const named = proposeSkills(g, {
    now: NOW,
    budget,
    existing: [{ name: 'proj-build-test-run', steps: new Set(['something else entirely']) }],
  })
  assert.equal(named.proposals.length, 0)
  assert.match(named.skipped[0].reason, /already exists/)

  const dup = proposeSkills(g, {
    now: NOW,
    budget,
    existing: [
      {
        name: 'run-angelX',
        steps: new Set(['cargo build', 'cargo test -p cockpit', 'npm run dev']),
      },
    ],
  })
  assert.equal(dup.proposals.length, 0)
  assert.match(dup.skipped[0].reason, /duplicate of skill 'run-angelX'/)

  const broke = proposeSkills(g, {
    now: NOW,
    budget: { date: '2026-07-06', runs: 2, cap: 2 },
    existing: [],
  })
  assert.equal(broke.proposals.length, 0)
  assert.match(broke.skipped[0].reason, /budget/)

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
    cmd('cargo build', 0, { session: 9 }),
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

  attachSkillUsage(g, { s: { uses: 4, ok: 4, lastUsed: NOW_TS } }, { s: fact.id })
  const fresh = compileHabitsStatus(g, { now: NOW })
  assert.equal(fresh.facts.length, 1)
  assert.equal(fresh.facts[0].drifting, false)
  assert.equal(fresh.facts[0].name, 'proj-build-test-run')

  const later = new Date(Date.parse(NOW) + 40 * 86_400_000).toISOString()
  const stale = compileHabitsStatus(g, { now: later })
  assert.ok(stale.facts[0].belief < DRIFT_BELIEF, `belief decayed (${stale.facts[0].belief})`)
  assert.equal(stale.facts[0].drifting, true)

  const g2 = confidentGraph()
  const quiet = compileHabitsStatus(g2, { now: later })
  assert.equal(quiet.facts[0].drifting, false)
  assert.deepEqual(JSON.parse(JSON.stringify(stale)), stale)
})

test('status and proposal batches scan evidence once without caching across mutations', () => {
  const g = confidentGraph()
  const [fact] = habitFacts(g, KEY)
  const edges = g.edges.bind(g)
  let scans = 0
  g.edges = () => { scans++; return edges() }
  for (const confidence of [0.1, 0.9]) {
    g.addEdge({ id: 'batch_habit', src: 'missing', dst: fact.id, type: EDGE_TYPE.CONTRADICTS, confidence })
    const expected = workflowBelief(g, g.getNode(fact.id), { now: NOW }).belief
    scans = 0
    const status = compileHabitsStatus(g, { now: NOW, repoKey: KEY })
    assert.equal(scans, 1)
    assert.equal(status.facts[0].belief, Number(expected.toFixed(3)))
    scans = 0
    const proposed = proposeSkills(g, { now: NOW, repoKey: KEY, minBelief: 0 })
    assert.equal(scans, 1)
    assert.equal(proposed.proposals[0].belief, expected)
  }
  scans = 0
  assert.deepEqual(compileHabitsStatus(g, { now: NOW, repoKey: 'absent' }).facts, [])
  assert.deepEqual(proposeSkills(g, { now: NOW, repoKey: 'absent' }).proposals, [])
  assert.equal(scans, 0, 'empty batches need no evidence scan')
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
  const { ranking } = proposeDossierProbe(g, KEY, { now: NOW })
  assert.ok(ranking.every((r) => r.hypothesisId.startsWith('hyp_dossier_')))
})

function tickWithLock(lock) {
  const dir = mkdtempSync(join(tmpdir(), 'angel-habitsmith-lock-'))
  try {
    const stateDir = join(dir, 'state')
    mkdirSync(stateDir)
    const lockPath = join(stateDir, 'lock')
    if (lock) writeFileSync(lockPath, JSON.stringify(lock))
    const run = spawnSync(
      process.execPath,
      [
        'scripts/runtime/habitsmith-tick.mjs',
        '--force',
        '--ledger',
        join(dir, 'ledger.jsonl'),
        '--graph',
        join(dir, 'graph.json'),
        '--state-dir',
        stateDir,
        '--proposed-dir',
        join(dir, 'proposed'),
        '--skills-dir',
        join(dir, 'skills'),
      ],
      { cwd: new URL('../..', import.meta.url), encoding: 'utf8' },
    )
    const beat = join(stateDir, 'heartbeat.json')
    return {
      run,
      heartbeat: existsSync(beat) ? JSON.parse(readFileSync(beat, 'utf8')) : null,
      lockLeft: existsSync(lockPath) ? JSON.parse(readFileSync(lockPath, 'utf8')) : null,
    }
  } finally {
    rmSync(dir, { recursive: true, force: true })
  }
}

test('a lock left by a tick that was killed does not hold the next tick', () => {
  const gone = spawnSync(process.execPath, ['-e', '0']).pid
  const { run, heartbeat, lockLeft } = tickWithLock({ pid: gone, ts: Date.now() })
  assert.equal(run.status, 0, run.stderr || run.stdout)
  assert.match(run.stdout, /habitsmith-tick: 0 verdict\(s\) folded/)
  assert.equal(heartbeat.action, 'ran')
  assert.equal(lockLeft, null, 'the tick that took the lock releases it')
})

test('a live tick keeps the lock, and the skip says why on the heartbeat', () => {
  const holder = tickLockOwner(Date.now())
  const { run, heartbeat, lockLeft } = tickWithLock(holder)
  assert.equal(run.status, 0, run.stderr || run.stdout)
  assert.match(run.stdout, /another tick holds the lock; skipping\./)
  assert.equal(heartbeat?.action, 'skip')
  assert.match(heartbeat?.reason, /^another tick holds the lock — .*alive/)
  assert.equal(lockLeft?.token, holder.token, 'a skipped tick never touches the holder\'s lock')
})
