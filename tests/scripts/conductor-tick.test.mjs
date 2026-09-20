// Tests for conductor-tick C3 pure scheduler logic: default-off arming, night
// windows, cooldown filtering, dispatch routing, and status export shape.

import { test } from 'node:test'
import assert from 'node:assert/strict'
import fs from 'node:fs'
import os from 'node:os'
import { spawnSync } from 'node:child_process'
import { dirname, join } from 'node:path'
import { fileURLToPath } from 'node:url'

import CausalGraph, { NODE_TYPE } from '../../lib/research/CausalGraph.js'
import { agendaNodeId, ingestAgenda } from '../../scripts/runtime/conductor.mjs'
import {
  DEFAULT_MIN_CORPUS,
  DEFAULT_MIN_MACHINE,
  anyTierOpen,
  conductorMode,
  cooldownExcludes,
  decideTick,
  densityGate,
  dispatchPlan,
  foldVerdicts,
  gateFor,
  gatedMode,
  inWindow,
  killed,
  metricDeltaVerdict,
  reportAccuracy,
  rungEvidence,
  statusExport,
} from '../../scripts/runtime/conductor-tick.mjs'
import { beliefProbability } from '../../scripts/runtime/causal-loop.mjs'
import { createMission, parseMission, tickMission, serializeMission } from '../../scripts/runtime/mission.mjs'

const NOW = '2026-07-07T03:30:00.000Z'

test('killed is default-off and arms only on exact ANGEL_CONDUCTOR=1', () => {
  assert.equal(killed({}), true)
  assert.equal(killed({ ANGEL_CONDUCTOR: '0' }), true)
  assert.equal(killed({ ANGEL_CONDUCTOR: ' 1 ' }), true)
  assert.equal(killed({ ANGEL_CONDUCTOR: '1' }), false)
})

test('conductorMode has three states and a typo fails into the one that cannot act', () => {
  assert.equal(conductorMode({}), 'off')
  assert.equal(conductorMode({ ANGEL_CONDUCTOR: '0' }), 'off')
  assert.equal(conductorMode({ ANGEL_CONDUCTOR: 'measur' }), 'off')
  assert.equal(conductorMode({ ANGEL_CONDUCTOR: '1' }), 'armed')
  assert.equal(conductorMode({ ANGEL_CONDUCTOR: 'measure' }), 'measure')
  assert.equal(conductorMode({ ANGEL_CONDUCTOR: ' Measure ' }), 'measure')
  // measure is NOT killed: it must clear every gate armed mode clears.
  assert.equal(killed({ ANGEL_CONDUCTOR: 'measure' }), false)
})

// ─── the tiered density gate ─────────────────────────────────────────────────
// The floor a rung must clear depends on the evidence that rung EATS. Gating
// Reflex's config knobs on 300 human-judged writes was a wall, not a gate: at the
// measured 0.70 judged writes/day it is 1.2 years away, and it was erected right
// after Loop 0 shipped a machine verdict on every source write. Correctness rungs
// gate on machine labels; taste rungs gate on the human's. See conductor-tick's
// header and docs/plans/the-flywheel.md.

test('rungEvidence tiers the rungs and sends an unknown rung to the strictest floor', () => {
  assert.equal(rungEvidence('config'), 'machine') // does it build?
  assert.equal(rungEvidence('skills'), 'taste') // what did you keep?
  assert.equal(rungEvidence('knowledge'), 'taste')
  assert.equal(rungEvidence('code'), 'taste')
  assert.equal(rungEvidence('weights'), 'taste')
  // A rung nobody classified must not inherit the fast lane by accident.
  assert.equal(rungEvidence('brand-new-rung'), 'taste')
  assert.equal(rungEvidence(null), 'taste')
})

test('densityGate tiers the floors: correctness on machine labels, taste on judged writes', () => {
  assert.equal(DEFAULT_MIN_CORPUS, 100) // lowered from an arbitrary, unreachable 300
  assert.equal(DEFAULT_MIN_MACHINE, 300)

  // Today's real substrate: 17 judged writes, and a machine corpus that fills ~7x
  // faster. The config rung may go; the keep-rate rungs may not.
  const today = densityGate({ judged: 17, machine: 400 })
  assert.equal(today.taste.ok, false)
  assert.match(today.taste.reason, /judged corpus is 17, below the 100/)
  assert.match(today.taste.reason, /too thin/)
  assert.equal(today.taste.knob, 'ANGEL_CUT_MIN_CORPUS')
  assert.equal(today.machine.ok, true)
  assert.equal(today.machine.knob, 'ANGEL_CUT_MIN_MACHINE')

  // gateFor picks the tier the rung actually consumes.
  assert.equal(gateFor(today, 'config'), today.machine)
  assert.equal(gateFor(today, 'skills'), today.taste)
  assert.equal(gateFor(today, null), today.taste)

  // Boundaries, per tier.
  assert.equal(densityGate({ judged: 99, machine: 999 }).taste.ok, false)
  assert.equal(densityGate({ judged: 100, machine: 999 }).taste.ok, true)
  assert.equal(densityGate({ judged: 999, machine: 299 }).machine.ok, false)
  assert.equal(densityGate({ judged: 999, machine: 300 }).machine.ok, true)

  // A fat machine corpus does NOT buy a taste rung its floor, and vice versa —
  // that conflation is the entire failure this tiering exists to prevent.
  const noTaste = densityGate({ judged: 3, machine: 5000 })
  assert.deepEqual(gatedMode('armed', noTaste, 'skills'), {
    mode: 'measure',
    gated: true,
    tier: noTaste.taste,
  })
  assert.deepEqual(gatedMode('armed', noTaste, 'config'), {
    mode: 'armed',
    gated: false,
    tier: noTaste.machine,
  })
  const noMachine = densityGate({ judged: 5000, machine: 3 })
  assert.equal(gatedMode('armed', noMachine, 'config').gated, true)
  assert.equal(gatedMode('armed', noMachine, 'skills').gated, false)

  // BOTH tiers fail closed: an unknown corpus is not a passing corpus.
  const unreadable = densityGate({ error: 'cut-audit exited 1' })
  assert.equal(unreadable.taste.ok, false)
  assert.equal(unreadable.machine.ok, false)
  assert.match(unreadable.machine.reason, /refusing to dispatch onto an unknown substrate/)
  assert.equal(anyTierOpen(unreadable), false)
  // A corpus-wide error beats any count that came with it.
  assert.equal(densityGate({ judged: 900, machine: 900, error: 'boom' }).taste.ok, false)
  // A count that is simply absent (an older cut-audit with no machine block) fails
  // only its own tier — it must not read as "zero machine labels".
  const oldAudit = densityGate({ judged: 400, machine: NaN })
  assert.equal(oldAudit.machine.ok, false)
  assert.equal(oldAudit.machine.count, null)
  assert.equal(oldAudit.taste.ok, true)

  // ...and only an explicit floor of 0 disarms a tier.
  const off = densityGate({ judged: 17, machine: 1, minCorpus: 0, minMachine: 0 })
  assert.equal(off.taste.ok, true)
  assert.equal(off.machine.ok, true)
  assert.match(off.taste.reason, /disabled \(ANGEL_CUT_MIN_CORPUS=0\)/)
  assert.match(off.machine.reason, /disabled \(ANGEL_CUT_MIN_MACHINE=0\)/)

  // Demotion, not a stop: a gated armed tick still measures.
  assert.equal(gatedMode('measure', today, 'config').gated, false)
  assert.equal(gatedMode('measure', today, 'config').mode, 'measure')
  assert.equal(gatedMode('off', today, 'skills').mode, 'off')
})

test('inWindow handles empty, normal, invalid, and midnight-wrapping windows', () => {
  assert.equal(inWindow(new Date('2026-07-07T12:00:00'), ''), true)
  assert.equal(inWindow(new Date('2026-07-07T03:00:00'), '02:00-07:00'), true)
  assert.equal(inWindow(new Date('2026-07-07T08:00:00'), '02:00-07:00'), false)
  assert.equal(inWindow(new Date('2026-07-07T23:30:00'), '22:00-03:00'), true)
  assert.equal(inWindow(new Date('2026-07-07T02:30:00'), '22:00-03:00'), true)
  assert.equal(inWindow(new Date('2026-07-07T12:00:00'), 'bogus'), false)
})

test('decideTick reports the first failing gate in conductor order', () => {
  assert.deepEqual(decideTick({ isKilled: true, windowOk: true, idle: true, budgetOk: true }), {
    run: false,
    reason: 'disabled (ANGEL_CONDUCTOR is not 1 or measure)',
  })
  assert.deepEqual(decideTick({ isKilled: false, windowOk: false, idle: true, budgetOk: true }), {
    run: false,
    reason: 'outside conductor window',
  })
  assert.deepEqual(decideTick({ isKilled: false, windowOk: true, idle: false, budgetOk: true }), {
    run: false,
    reason: 'busy - live session or recent ledger activity',
  })
  assert.deepEqual(decideTick({ isKilled: false, windowOk: true, idle: true, budgetOk: false }), {
    run: false,
    reason: 'daily conductor budget exhausted',
  })
  assert.deepEqual(decideTick({ isKilled: false, windowOk: true, idle: true, budgetOk: true }), {
    run: true,
    reason: 'idle, in window, and within budget',
  })
})

test('cooldownExcludes suppresses recently dispatched agenda nodes only', () => {
  const g = new CausalGraph()
  ingestAgenda(
    g,
    [
      {
        rung: 'code',
        slug: 'fresh',
        goal: 'Fresh dispatch.',
        estCostMin: 45,
        severity: 1,
        observed: { samples: 3 },
      },
      {
        rung: 'code',
        slug: 'old',
        goal: 'Old dispatch.',
        estCostMin: 45,
        severity: 1,
        observed: { samples: 3 },
      },
      {
        rung: 'skills',
        slug: 'never',
        goal: 'Never dispatched.',
        estCostMin: 5,
        severity: 1,
        observed: { samples: 1 },
      },
    ],
    { now: NOW },
  )
  g.updateNode(agendaNodeId('code', 'fresh'), {
    lastDispatchedAt: '2026-07-07T02:30:00.000Z',
  })
  g.updateNode(agendaNodeId('code', 'old'), {
    lastDispatchedAt: '2026-07-01T02:30:00.000Z',
  })

  const excluded = cooldownExcludes(g, { nowMs: Date.parse(NOW), cooldownH: 48 })
  assert.deepEqual([...excluded], [agendaNodeId('code', 'fresh')])
})

test('dispatchPlan routes each rung without invoking children', () => {
  const root = '/repo'
  const base = { hypothesisId: 'conductor_x', goal: 'Do it.' }
  assert.deepEqual(dispatchPlan({ ...base, rung: 'config' }, { root }), {
    kind: 'spawn',
    cmd: 'node',
    args: ['/repo/scripts/runtime/reflex-tick.mjs', '--force'],
  })
  assert.deepEqual(dispatchPlan({ ...base, rung: 'skills' }, { root }), {
    kind: 'spawn',
    cmd: 'node',
    args: ['/repo/scripts/runtime/habitsmith-tick.mjs'],
  })
  assert.deepEqual(dispatchPlan({ ...base, rung: 'knowledge' }, { root }), {
    kind: 'spawn',
    cmd: 'node',
    args: ['/repo/scripts/runtime/dossier-tick.mjs'],
  })
  assert.deepEqual(
    dispatchPlan({ ...base, rung: 'code' }, { root, env: { ANGEL_CONDUCTOR_DRIVER: '' } }),
    { kind: 'skip', reason: 'code rung requires ANGEL_CONDUCTOR_DRIVER' },
  )
  assert.deepEqual(
    dispatchPlan({ ...base, rung: 'code' }, { root, env: { ANGEL_CONDUCTOR_DRIVER: 'practice' } }),
    {
      kind: 'spawn',
      cmd: 'node',
      args: ['/repo/scripts/runtime/conductor-code-run.mjs', '--agenda', 'conductor_x'],
    },
  )
  const weights = dispatchPlan({ ...base, rung: 'weights' }, { root })
  assert.equal(weights.kind, 'briefing')
  assert.match(weights.text, /briefing only/)
})

test('statusExport writes the small cockpit-facing top-five shape', () => {
  const ranking = Array.from({ length: 7 }, (_, i) => ({
    hypothesisId: `h${i}`,
    rung: i % 2 ? 'config' : 'code',
    goal: `Goal ${i}`,
    priority: 10 - i,
    severity: 1,
    estCostMin: 5,
    evidenceRefs: [`ref${i}`],
  }))
  const status = statusExport(ranking, { now: NOW })
  assert.equal(status.v, 1)
  assert.equal(status.generatedAt, NOW)
  assert.equal(status.ranking.length, 5)
  assert.deepEqual(status.ranking[0], {
    hypothesisId: 'h0',
    rung: 'code',
    goal: 'Goal 0',
    priority: 10,
    severity: 1,
    estCostMin: 5,
    evidenceRefs: ['ref0'],
  })
  assert.deepEqual(JSON.parse(JSON.stringify(status)), status)
})

test('foldVerdicts adds signed evidence and keeps agenda nodes perpetual', () => {
  const g = new CausalGraph()
  ingestAgenda(
    g,
    [
      {
        rung: 'code',
        slug: 'approve-me',
        goal: 'Approve me.',
        estCostMin: 45,
        severity: 1,
        observed: { samples: 1 },
      },
      {
        rung: 'code',
        slug: 'reject-me',
        goal: 'Reject me.',
        estCostMin: 45,
        severity: 1,
        observed: { samples: 1 },
      },
    ],
    { now: NOW },
  )
  const approveId = agendaNodeId('code', 'approve-me')
  const rejectId = agendaNodeId('code', 'reject-me')
  const { folded, orphaned } = foldVerdicts(
    g,
    [
      { action: 'approve', name: 'q1', fact: approveId },
      { action: 'reject', name: 'q2', fact: rejectId },
      { action: 'approve', name: 'missing', fact: 'nope' },
    ],
    { now: NOW },
  )
  assert.equal(folded.length, 2)
  assert.equal(orphaned.length, 1)
  assert.ok(beliefProbability(g, approveId) > 0.5)
  assert.ok(beliefProbability(g, rejectId) < 0.5)
  assert.equal(g.getNode(approveId).status, 'testing')
  assert.equal(g.getNode(rejectId).status, 'testing')
  assert.equal(g.getNode(approveId).lastVerifiedAt, NOW)
})

test('metricDeltaVerdict compares pre/post bench accuracy with decisive and inconclusive confidence', () => {
  const report = (a, b) => ({
    control: { suite: 'fixture', evaluation_sha256: 'a'.repeat(64) },
    metrics: [
      { subject: 'a', accuracy_pct: a },
      { subject: 'b', accuracy_pct: b },
    ],
  })
  const before = report(80, 90)
  const better = report(82, 92)
  const same = report(80.2, 90.2)
  const worse = report(78, 88)

  assert.equal(reportAccuracy(before), 85)
  assert.deepEqual(metricDeltaVerdict(before, better).metrics, {
    baseline: 85,
    treatment: 87,
    delta: 2,
  })
  assert.equal(metricDeltaVerdict(before, better).verdict, 'supports')
  assert.equal(metricDeltaVerdict(before, better).confidence, 0.8)
  assert.equal(metricDeltaVerdict(before, same).verdict, 'inconclusive')
  assert.equal(metricDeltaVerdict(before, same).confidence, 0.45)
  assert.equal(metricDeltaVerdict(before, worse).verdict, 'contradicts')
  assert.equal(metricDeltaVerdict({}, better).confidence, 0.35)
  assert.equal(reportAccuracy({ metrics: [{ accuracy_pct: null }] }), null)
  const changedTasks = {
    ...better,
    control: { ...better.control, evaluation_sha256: 'b'.repeat(64) },
  }
  assert.equal(metricDeltaVerdict(before, changedTasks).verdict, 'inconclusive')
  const changedSubjects = { ...better, metrics: [{ subject: 'other', accuracy_pct: 100 }] }
  assert.equal(metricDeltaVerdict(before, changedSubjects).verdict, 'inconclusive')
})

// ─── the clobber guard ───────────────────────────────────────────────────────
// Every rung the Conductor dispatches to (reflex / habitsmith / dossier / code
// run) loads, mutates and writes the configured shared graph itself. The tick
// therefore has two writers inside one lock, and the parent always writes last:
// if it holds a graph it deserialized BEFORE the spawn and persists that copy
// afterwards, it silently erases everything the child just recorded. The whole
// stated purpose of the Conductor is to be "the single serializing scheduler for
// all graph writers" (docs/plans/conductor.md) — and it was the clobberer. It
// only never bit in production because the Conductor has never been armed.
//
// These two tests bracket the fix from both sides, because the obvious repair is
// half a repair:
//   • reloading after the spawn keeps the CHILD's write   → test 1
//   • but loses the PARENT's own pending lastDispatchedAt → test 2
// Only persist-then-spawn-then-reload passes both.
//
// The dispatch path lives inside the unexported cli(), so these drive the real
// script as a subprocess — the only way to exercise the actual spawn bracket.

const SCRIPTS_DIR = join(dirname(fileURLToPath(import.meta.url)), '..', '..', 'scripts')
const REPO_ROOT = join(SCRIPTS_DIR, '..')
// Which conductor-tick to exercise. Overridable so this guard can be pointed at
// a pre-fix build (`git show <rev>:scripts/runtime/conductor-tick.mjs > /tmp/x.mjs`) to
// prove it still catches the regression it was written for.
const TICK_SRC = process.env.ANGEL_CONDUCTOR_TICK_SRC || join(SCRIPTS_DIR, 'conductor-tick.mjs')

const CHILD_NODE_ID = 'conductor_child_write_fixture'
const AGENDA = { rung: 'skills', slug: 'clobber-guard' } // a TASTE rung
const CONFIG_AGENDA = { rung: 'config', slug: 'spin-limit' } // a CORRECTNESS rung

// The script each rung dispatches to (see dispatchPlan). The fixture fakes it.
const RUNG_CHILD = { skills: 'habitsmith-tick.mjs', config: 'reflex-tick.mjs' }

// A stand-in for a rung child, faithful in the one way that matters: it does
// load → mutate → write on the graph FILE, exactly as the real ticks do. It also
// records what the parent had handed it, so the test can see the handoff.
// Dynamic imports so this same body can be dropped inside a CLI guard (below).
const CHILD_BODY = `
const fs = (await import('node:fs')).default
const { default: CausalGraph, NODE_TYPE } = await import(${JSON.stringify(join(REPO_ROOT, 'lib/research/CausalGraph.js'))})
const graphPath = process.env.FIXTURE_GRAPH
const agendaId = process.env.FIXTURE_AGENDA
const g = CausalGraph.deserialize(JSON.parse(fs.readFileSync(graphPath, 'utf8')))
g.addNode({
  id: ${JSON.stringify(CHILD_NODE_ID)},
  type: NODE_TYPE.HYPOTHESIS,
  hypothesisId: ${JSON.stringify(CHILD_NODE_ID)},
  status: 'testing',
  label: 'written by the dispatched child',
  // what the parent had persisted for us to read, if anything
  sawParentDispatchedAt: g.getNode(agendaId)?.lastDispatchedAt ?? null,
})
fs.writeFileSync(graphPath, JSON.stringify(g.serialize(), null, 2))
`

// The config rung dispatches to reflex-tick.mjs — which conductor-tick ALSO
// imports its whole gate stack from (isIdle / rollBudget / makeHeartbeat / …). So
// the config rung's fake child cannot simply replace that module: it must still BE
// it when imported, and only play the child when spawned as a CLI. It re-exports
// the real module by absolute path and guards the child body on argv[1], exactly
// as the real ticks do. (Never writeFileSync over the symlink — that writes
// straight through to the repo's reflex-tick.mjs.)
const FAKE_REFLEX_CHILD = `export * from ${JSON.stringify(join(SCRIPTS_DIR, 'reflex-tick.mjs'))}
const isCli =
  process.argv[1] && (await import('node:url')).fileURLToPath(import.meta.url) === process.argv[1]
if (isCli) {
${CHILD_BODY}
}
`

// cut-tick is spawned unconditionally by the tick; stub it inert so this stays a
// test about the DISPATCH bracket and nothing else.
const STUB_CUT_TICK = 'process.exit(0)\n'

// The density gate shells out to `cut-audit --json` for BOTH substrates: the
// human-judged corpus (taste rungs) and the machine-labeled one (correctness
// rungs). Stub it so the fixture controls each number independently. `null` omits
// the key entirely — an audit that never reported it, which must fail that tier
// closed rather than read as zero.
const stubCutAudit = ({ judged, machine }) => {
  const out = { writes: {}, cmds: {}, machine: {} }
  if (judged !== null) out.writes.judged = judged
  if (machine !== null) out.machine.labeled = machine
  return `console.log(${JSON.stringify(JSON.stringify(out))})\n`
}

/**
 * Run one real conductor tick against a throwaway root whose dispatched rung is
 * the fake child above, and return the graph and state as they were left on disk.
 *
 * Node resolves symlinks to their realpath, so the sibling modules keep loading
 * from the repo while the COPIED tick sees the fixture as its ROOT — which is
 * what lets us swap in a fake child without touching the repo's scripts.
 */
function runDispatchTick({
  conductor = '1',
  judged = 999,
  machine = 999,
  audit = true, // false → no cut-audit script at all: the corpus is unreadable
  agenda = AGENDA,
  verdicts = [],
  mission = null, // a mission state object; the tick is pointed at a dir holding it
} = {}) {
  const root = fs.mkdtempSync(join(os.tmpdir(), 'conductor-tick-'))
  try {
    fs.mkdirSync(join(root, 'scripts'))
    fs.mkdirSync(join(root, 'reports'))
    fs.mkdirSync(join(root, 'state'))
    fs.symlinkSync(join(REPO_ROOT, 'lib'), join(root, 'lib'))
    const child = RUNG_CHILD[agenda.rung]
    for (const mod of [
      'reflex-tick.mjs',
      'conductor.mjs',
      'causal-loop.mjs',
      'mission.mjs',
      'private-store-fs.mjs',
      'worker-paths.mjs',
      'worker-lock.mjs',
      'bounded-child.mjs',
      'benchmark-runner.mjs',
    ]) {
      if (mod === child) continue // faked below, as a real file — never a symlink
      fs.symlinkSync(join(SCRIPTS_DIR, mod), join(root, 'scripts', mod))
    }
    fs.copyFileSync(TICK_SRC, join(root, 'scripts', 'conductor-tick.mjs'))
    fs.writeFileSync(
      join(root, 'scripts', child),
      child === 'reflex-tick.mjs' ? FAKE_REFLEX_CHILD : CHILD_BODY,
    )
    fs.writeFileSync(join(root, 'scripts', 'cut-tick.mjs'), STUB_CUT_TICK)
    if (audit)
      fs.writeFileSync(join(root, 'scripts', 'cut-corpus.mjs'), stubCutAudit({ judged, machine }))

    // One open agenda item on the rung under test, so the ranked dispatch is
    // deterministic.
    const graphPath = join(root, 'graph.json')
    const seed = new CausalGraph()
    ingestAgenda(
      seed,
      [
        {
          ...agenda,
          goal: 'Guard the dispatched child against the parent persist.',
          estCostMin: 5,
          severity: 1,
          observed: { samples: 2 },
        },
      ],
      { now: NOW },
    )
    const agendaId = agendaNodeId(agenda.rung, agenda.slug)
    fs.writeFileSync(graphPath, JSON.stringify(seed.serialize(), null, 2))

    // Human verdicts waiting in the spool — the evidence a tick must fold.
    const spoolPath = join(root, 'state', 'verdicts.jsonl')
    fs.writeFileSync(
      spoolPath,
      verdicts.map((v) => JSON.stringify({ ...v, fact: v.fact ?? agendaId })).join('\n'),
    )

    let missionDir = null
    if (mission !== null) {
      missionDir = join(root, 'missions')
      fs.mkdirSync(missionDir, { recursive: true })
      fs.writeFileSync(join(missionDir, `${mission.id}.json`), serializeMission(mission))
      fs.writeFileSync(join(missionDir, `${mission.id}.jsonl`), `${serializeMission(mission)}\n`)
    }

    const res = spawnSync(
      process.execPath,
      [
        join(root, 'scripts', 'conductor-tick.mjs'),
        '--force', // clears the window + idle gates
        '--no-build', // no cargo
        '--graph',
        graphPath,
        '--state-dir',
        join(root, 'state'),
        '--ledger',
        join(root, 'ledger.jsonl'), // absent: nothing to mine
        '--reports-dir',
        join(root, 'reports'), // empty: no bench reports
        '--habits-status',
        join(root, 'missing-habits.json'),
        '--proposals',
        join(root, 'missing-proposals.md'),
      ],
      {
        cwd: root,
        encoding: 'utf8',
        env: {
          ...process.env,
          HOME: root, // keep every default path off the real ~/.angel
          ...(missionDir === null ? {} : { ANGEL_MISSION_DIR: missionDir }),
          ANGEL_CONDUCTOR: conductor,
          ANGEL_CUT_MIN_CORPUS: '', // unset: exercise the real floors, both tiers
          ANGEL_CUT_MIN_MACHINE: '',
          FIXTURE_GRAPH: graphPath, // inherited straight through to the child
          FIXTURE_AGENDA: agendaId,
        },
      },
    )

    const readMaybe = (p) => {
      try {
        return fs.readFileSync(p, 'utf8')
      } catch {
        return ''
      }
    }
    return {
      res,
      agendaId,
      graph: CausalGraph.deserialize(JSON.parse(fs.readFileSync(graphPath, 'utf8'))),
      heartbeat: JSON.parse(readMaybe(join(root, 'state', 'heartbeat.json')) || 'null'),
      briefing: readMaybe(join(root, 'state', 'briefing.md')),
      spool: readMaybe(spoolPath),
      budget: JSON.parse(readMaybe(join(root, 'state', 'budget.json')) || 'null'),
      missionDir,
      // Mission state read BEFORE the fixture root is removed below.
      missionOnDisk:
        missionDir === null
          ? null
          : parseMission(readMaybe(join(missionDir, `${mission.id}.json`)) || 'null'),
      missionLedger: missionDir === null ? '' : readMaybe(join(missionDir, `${mission.id}.jsonl`)),
    }
  } finally {
    fs.rmSync(root, { recursive: true, force: true })
  }
}

test('dispatch preserves the child rung graph write instead of clobbering it', () => {
  const { res, agendaId, graph } = runDispatchTick()
  assert.equal(res.status, 0, `tick failed:\n${res.stdout}\n${res.stderr}`)
  assert.match(res.stdout, /skills child completed/)

  // The child wrote this node while the parent held a deserialized copy of the
  // graph. If the parent persists that stale copy, the node is gone.
  assert.ok(
    graph.hasNode(CHILD_NODE_ID),
    "the dispatched child's graph write was erased by the parent tick's persist",
  )
  assert.equal(graph.getNode(CHILD_NODE_ID).type, NODE_TYPE.HYPOTHESIS)

  // ...and the parent's own pre-spawn mutation is still there too.
  assert.ok(graph.getNode(agendaId).lastDispatchedAt, 'parent lost its own lastDispatchedAt')
})

// ─── loop 2: measure-only, and the tiered density gate ───────────────────────
// docs/plans/the-flywheel.md. Measure mode runs the entire night path — lock,
// gates, verdict fold, cut fold, agenda mining, ranking, briefing — and dispatches
// NOTHING. The density gate then makes the same thing true of an ARMED tick whose
// substrate is too thin to learn from — but "too thin" now depends on WHICH
// substrate the rung consumes: a correctness rung (config) may go on machine labels
// while the judged corpus is still 17, and a taste rung may not go on machine
// labels at all. Every one of these asserts on the fake child's node: it can only
// exist if a rung was actually spawned.

test('measure mode folds the evidence and writes the briefing but never spawns a rung', () => {
  const { res, agendaId, graph, heartbeat, briefing, spool, budget } = runDispatchTick({
    conductor: 'measure',
    verdicts: [{ ts: NOW, action: 'approve', name: 'guard' }],
  })
  assert.equal(res.status, 0, `tick failed:\n${res.stdout}\n${res.stderr}`)

  // NOTHING WAS DISPATCHED. The fake skills child writes this node the instant it
  // is spawned; its absence is the whole assertion.
  assert.equal(
    graph.hasNode(CHILD_NODE_ID),
    false,
    'measure mode spawned a rung — it must dispatch nothing',
  )
  assert.doesNotMatch(res.stdout, /child completed/)
  // ...and the cooldown must not pretend we did.
  assert.equal(graph.getNode(agendaId).lastDispatchedAt ?? null, null)

  // But the evidence WAS folded: the spooled approval landed as belief on the
  // graph, and the spool was truncated only after the graph persisted.
  assert.ok(beliefProbability(graph, agendaId) > 0.5, 'the spooled verdict was not folded')
  assert.equal(graph.getNode(agendaId).status, 'testing', 'agenda node must stay perpetual')
  assert.equal(spool.trim(), '', 'the verdict spool was not truncated after the fold')

  // The heartbeat is unambiguous: not 'skip', not 'ran'.
  assert.equal(heartbeat.action, 'measure')
  assert.equal(heartbeat.experiment.agenda, agendaId)
  assert.equal(heartbeat.experiment.dispatched, false)
  assert.equal(budget.runs, 1, 'a measure pass is a run and must cost the same budget')

  // The briefing names the proposal it declined, and why.
  assert.match(briefing, /measure: declined to dispatch \[skills\]/)
  assert.match(briefing, new RegExp(agendaId))
  assert.match(briefing, /measure-only mode/)
  assert.match(briefing, /habitsmith-tick\.mjs/) // what it WOULD have run
})

test('a TASTE rung refuses at the judged corpus of 17, and a fat machine corpus does not rescue it', () => {
  const { res, agendaId, graph, heartbeat, briefing } = runDispatchTick({
    conductor: '1', // ARMED
    agenda: AGENDA, // skills: reasons from keep rate
    judged: 17, // ...onto the taste substrate that actually exists today
    machine: 5000, // and all the exit codes in the world cannot supply taste
  })
  assert.equal(res.status, 0, `tick failed:\n${res.stdout}\n${res.stderr}`)

  // Armed is not sufficient. 17 judged writes is not a substrate for a keep-rate
  // rung, no matter how much the machine has to say.
  assert.equal(
    graph.hasNode(CHILD_NODE_ID),
    false,
    'the density gate let a taste rung dispatch on 17 judged writes',
  )
  assert.equal(graph.getNode(agendaId).lastDispatchedAt ?? null, null)

  // And it says why — loudly, on stdout, in the briefing, and in the heartbeat —
  // naming the tier that blocked it and the knob that moves it.
  assert.match(res.stdout, /DENSITY GATE/)
  assert.match(res.stdout, /REFUSING TO DISPATCH/)
  assert.match(briefing, /DENSITY GATE/)
  assert.match(briefing, /judged corpus is 17, below the 100/)
  assert.match(briefing, /ANGEL_CUT_MIN_CORPUS/)
  assert.equal(heartbeat.action, 'measure')
  assert.match(heartbeat.reason, /DENSITY GATE/)
})

test('a CORRECTNESS rung dispatches on machine labels alone, with a thin judged corpus', () => {
  // The whole point of the tiering. Reflex tuning ANGEL_SPIN_LIMIT does not need to
  // know the user's taste; it needs to know whether the code compiled. Gating it on
  // 300 human verdicts (0.70/day → 1.2 years) was a wall. Loop 0 labels every source
  // write in seconds, so this rung goes now.
  const { res, agendaId, graph, heartbeat, briefing } = runDispatchTick({
    conductor: '1',
    agenda: CONFIG_AGENDA, // config: reasons from exit codes
    judged: 17, // taste substrate: still exactly what it is today
    machine: 400, // correctness substrate: real, and past its floor
  })
  assert.equal(res.status, 0, `tick failed:\n${res.stdout}\n${res.stderr}`)

  assert.ok(
    graph.hasNode(CHILD_NODE_ID),
    'a correctness rung was blocked by a taste floor whose evidence it never consumes',
  )
  assert.match(res.stdout, /config child completed/)
  assert.doesNotMatch(res.stdout, /DENSITY GATE/)
  assert.doesNotMatch(briefing, /DENSITY GATE/)
  assert.ok(graph.getNode(agendaId).lastDispatchedAt, 'a real dispatch must stamp the cooldown')
  assert.equal(heartbeat.action, 'ran')
  assert.equal(heartbeat.experiment.rung, 'config')
})

test('a CORRECTNESS rung still refuses when the MACHINE corpus is thin', () => {
  // The tier is a gate, not a bypass: the correctness rung has a floor too, it is
  // just a floor made of the evidence it actually eats.
  const { res, agendaId, graph, heartbeat, briefing } = runDispatchTick({
    conductor: '1',
    agenda: CONFIG_AGENDA,
    judged: 9999, // all the taste in the world...
    machine: 12, // ...cannot tell you whether the build is green
  })
  assert.equal(res.status, 0, `tick failed:\n${res.stdout}\n${res.stderr}`)
  assert.equal(graph.hasNode(CHILD_NODE_ID), false, 'dispatched a config rung on 12 exit codes')
  assert.equal(graph.getNode(agendaId).lastDispatchedAt ?? null, null)
  assert.match(res.stdout, /DENSITY GATE/)
  assert.match(briefing, /machine-labeled corpus is 12, below the 300/)
  assert.match(briefing, /ANGEL_CUT_MIN_MACHINE/) // the knob that moves THIS floor
  assert.equal(heartbeat.action, 'measure')
})

test('both tiers fail closed when the corpus cannot be read', () => {
  // audit=false → no cut-audit stub in the fixture root at all, so the audit spawn
  // fails. An unreadable corpus is an unknown corpus, and an armed tick does not
  // dispatch onto an unknown corpus — whichever tier governs it.
  for (const agenda of [AGENDA, CONFIG_AGENDA]) {
    const { res, graph, heartbeat } = runDispatchTick({ conductor: '1', agenda, audit: false })
    assert.equal(res.status, 0, `tick failed:\n${res.stdout}\n${res.stderr}`)
    assert.equal(
      graph.hasNode(CHILD_NODE_ID),
      false,
      `${agenda.rung} dispatched despite an unreadable corpus`,
    )
    assert.match(res.stdout, /DENSITY GATE/)
    assert.match(res.stdout, /unknown substrate/)
    assert.equal(heartbeat.action, 'measure')
  }
})

test('a missing machine count fails only its own tier — it is not a report of zero', () => {
  // An older cut-audit with no `machine` block never claimed the corpus was empty.
  // The correctness rung must fail closed on it; the taste rung, whose number IS
  // there, must be unaffected.
  const blocked = runDispatchTick({ conductor: '1', agenda: CONFIG_AGENDA, machine: null })
  assert.equal(blocked.graph.hasNode(CHILD_NODE_ID), false)
  assert.match(blocked.res.stdout, /DENSITY GATE/)

  const open = runDispatchTick({ conductor: '1', agenda: AGENDA, judged: 400, machine: null })
  assert.equal(
    open.graph.hasNode(CHILD_NODE_ID),
    true,
    'a taste rung was blocked by a missing machine count',
  )
  assert.doesNotMatch(open.res.stdout, /DENSITY GATE/)
})

test('dispatch hands its pending mutations to the child before spawning', () => {
  const { agendaId, graph } = runDispatchTick()
  const child = graph.getNode(CHILD_NODE_ID)
  const dispatchedAt = graph.getNode(agendaId).lastDispatchedAt

  // The child must have READ the parent's lastDispatchedAt — i.e. the parent
  // persisted before spawning. A bare reload-after-spawn keeps the child's write
  // but lets the child load a graph without the stamp and write that back, so the
  // parent's own mutation vanishes on reload: the cooldown never engages and the
  // same agenda item is dispatched every single night.
  assert.ok(child, 'the child never reached the persisted graph at all')
  assert.ok(dispatchedAt, 'the parent tick lost its own lastDispatchedAt mutation')
  assert.equal(
    child.sawParentDispatchedAt,
    dispatchedAt,
    'the child did not see the parent mutation that survived the tick',
  )
})

// ─── the mission gate (scripts/runtime/mission.mjs) ───────────────────────────────────
// An armed Conductor that drives a persisted mission consumes the mission's
// round budget: a spent/blocked/paused/complete mission demotes the tick to
// measure-only (no rung spawned), and each real dispatch credits one round on
// disk.

test('a spent mission budget gates an armed tick into measure-only', () => {
  const mission = tickMission(createMission({ objective: 'night run', maxRounds: 1 }), {})
  const { res, graph, briefing, missionOnDisk } = runDispatchTick({ mission })
  assert.equal(res.status, 0, `tick failed:\n${res.stdout}\n${res.stderr}`)
  assert.match(res.stdout, /MISSION GATE/)
  assert.match(res.stdout, /round budget reached \(1\/1\)/)
  assert.equal(graph.hasNode(CHILD_NODE_ID), false, 'a gated mission must dispatch nothing')
  assert.match(briefing, /MISSION GATE/)
  assert.equal(missionOnDisk.roundsStarted, 1, 'a refused tick never credits a round')
})

test('an active mission lets the tick dispatch and credits one round on disk', () => {
  const mission = createMission({ objective: 'night run', maxRounds: 3 })
  const { res, graph, missionOnDisk, missionLedger } = runDispatchTick({ mission })
  assert.equal(res.status, 0, `tick failed:\n${res.stdout}\n${res.stderr}`)
  assert.match(res.stdout, /skills child completed/)
  assert.ok(graph.hasNode(CHILD_NODE_ID), 'the rung was really dispatched')
  assert.equal(missionOnDisk.roundsStarted, 1, 'the dispatch credited one mission round')
  assert.equal(
    missionLedger.trim().split('\n').length,
    2,
    'the credit is appended to the audit ledger',
  )
})
