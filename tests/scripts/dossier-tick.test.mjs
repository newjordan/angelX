// Tests for dossier-tick: the pure probe logic (verdicts, perpetual-fact
// application, kill switch, token extraction). The fs/spawn side effects live
// only in the CLI and are exercised by the --dry-run/--force smoke.

import { test } from 'node:test'
import assert from 'node:assert/strict'
import { mkdtempSync, mkdirSync, readFileSync, rmSync, writeFileSync } from 'node:fs'
import { tmpdir } from 'node:os'
import { join } from 'node:path'
import { spawnSync } from 'node:child_process'

import CausalGraph, { NODE_TYPE, EDGE_TYPE } from '../../lib/research/CausalGraph.js'
import { mineRepoFacts, ingestRepoFacts, dossierFacts, factBelief } from '../../scripts/runtime/repo-dossier.mjs'
import { killed, commandToken, p0Verdict, p1Verdict, applyProbe } from '../../scripts/runtime/dossier-tick.mjs'

const NOW = '2026-07-06T00:00:00.000Z'
const KEY = 'home-u-proj-0011223344556677'

const cmd = (text, exit, session) => ({
  kind: 'event',
  v: 1,
  ts: Math.floor(Date.parse(NOW) / 1000),
  session,
  seq: 0,
  event: 'cmd',
  repo: { key: KEY, root: '/home/u/proj', slug: 'u/p' },
  cmd: { text, exit, timed_out: false, dur_ms: 10, tool: 'shell', bytes_out: 1 },
})

function seededGraph() {
  const g = new CausalGraph()
  const rows = [
    cmd('cargo test', 0, 1),
    cmd('cargo test', 0, 2),
    cmd('cargo test', 0, 3),
    cmd('npm test', 1, 1),
    cmd('npm test', 1, 2),
    cmd('npm test', 1, 3),
  ]
  const mined = mineRepoFacts(rows)
  ingestRepoFacts(g, KEY, mined[KEY], { now: NOW })
  return g
}

test('kill switch reads ANGEL_DOSSIER=0 only', () => {
  assert.equal(killed({ ANGEL_DOSSIER: '0' }), true)
  assert.equal(killed({ ANGEL_DOSSIER: '1' }), false)
  assert.equal(killed({}), false)
})

test('commandToken skips env assignments and finds the executable', () => {
  assert.equal(commandToken('cargo test -p cockpit'), 'cargo')
  assert.equal(commandToken('RUST_LOG=debug cargo test'), 'cargo')
  assert.equal(commandToken('A=1 B=2 npm run dev'), 'npm')
  assert.equal(commandToken(''), null)
})

test('p0 verdicts: absence is strong, presence is weak, traps are asymmetric', () => {
  const ritual = { factKind: 'ritual', factText: 'cargo test' }
  const trap = { factKind: 'trap', factText: 'npm test' }

  // Everything present: weak support for a ritual, no signal for a trap.
  assert.equal(p0Verdict(ritual, { rootExists: true, binaryFound: true }).verdict, 'supports')
  assert.ok(p0Verdict(ritual, { rootExists: true, binaryFound: true }).confidence < 0.6)
  assert.equal(p0Verdict(trap, { rootExists: true, binaryFound: true }).verdict, 'inconclusive')

  // Missing binary: contradicts a ritual, supports a trap (it still fails).
  assert.equal(p0Verdict(ritual, { rootExists: true, binaryFound: false }).verdict, 'contradicts')
  assert.equal(p0Verdict(trap, { rootExists: true, binaryFound: false }).verdict, 'supports')

  // Missing root: contradicts a ritual, says nothing about a trap.
  assert.equal(p0Verdict(ritual, { rootExists: false, binaryFound: false }).verdict, 'contradicts')
  assert.equal(p0Verdict(trap, { rootExists: false, binaryFound: false }).verdict, 'inconclusive')
})

test('p1 verdicts: pass supports a ritual and refutes a trap, and vice versa', () => {
  const ritual = { factKind: 'ritual', factText: 'cargo test' }
  const trap = { factKind: 'trap', factText: 'npm test' }
  assert.equal(p1Verdict(ritual, { exit: 0, timedOut: false }).verdict, 'supports')
  assert.equal(p1Verdict(ritual, { exit: 101, timedOut: false }).verdict, 'contradicts')
  assert.equal(p1Verdict(trap, { exit: 1, timedOut: false }).verdict, 'supports')
  assert.equal(p1Verdict(trap, { exit: 0, timedOut: false }).verdict, 'contradicts')
  // A timeout is a failure (lower confidence).
  const to = p1Verdict(ritual, { exit: null, timedOut: true })
  assert.equal(to.verdict, 'contradicts')
  assert.ok(to.confidence < 0.85)
})

test('applyProbe adds the evidence edge, keeps the fact perpetual, and moves belief', () => {
  const g = seededGraph()
  const fact = dossierFacts(g, KEY).find((n) => n.factKind === 'ritual')
  const before = factBelief(g, fact, { now: NOW }).belief

  applyProbe(
    g,
    fact.id,
    { verdict: 'supports', confidence: 0.85, evidence: 'p1: exit 0' },
    { now: NOW },
  )

  const after = g.getNode(fact.id)
  // Perpetual: updateBeliefs would have concluded at 0.85 confidence — the
  // probe path flips it straight back so the scorer keeps surfacing it.
  assert.equal(after.status, 'testing')
  assert.equal(after.concludedAt, null)
  assert.equal(after.lastVerifiedAt, NOW)
  // The SUPPORTS edge exists and raises the compiled belief.
  const edges = g.edgesOfType(EDGE_TYPE.SUPPORTS).filter((e) => e.dst === fact.id)
  assert.equal(edges.length, 1)
  const verified = factBelief(g, after, { now: NOW }).belief
  assert.ok(verified > before, `${before} → ${verified}`)

  // A contradicting probe pulls it back down below the pre-probe belief.
  applyProbe(
    g,
    fact.id,
    { verdict: 'contradicts', confidence: 0.85, evidence: 'p1: exit 101' },
    { now: NOW },
  )
  const refuted = factBelief(g, g.getNode(fact.id), { now: NOW }).belief
  assert.ok(refuted < verified, `${verified} → ${refuted}`)
  assert.equal(g.getNode(fact.id).status, 'testing')

  // The experiment nodes minted by the probes are real graph nodes.
  assert.ok(g.nodesOfType(NODE_TYPE.EXPERIMENT).length >= 2)
})

test('forced tick turns fresh v3 ledger passes into a compiled dossier fact', () => {
  const dir = mkdtempSync(join(tmpdir(), 'angel-dossier-tick-'))
  try {
    const repoRoot = join(dir, 'repo')
    const stateDir = join(dir, 'state')
    const outDir = join(dir, 'out')
    const ledger = join(dir, 'ledger.jsonl')
    const cutDir = join(dir, 'cut')
    const graph = join(dir, 'graph.json')
    mkdirSync(repoRoot)
    mkdirSync(cutDir)

    const ts = Math.floor(Date.now() / 1000)
    const rows = [1, 2, 3].map((session) => ({
      kind: 'event',
      v: 3,
      ts,
      session,
      seq: 1,
      event: 'cmd',
      repo: { key: KEY, root: repoRoot, slug: 'local/probe' },
      cmd: {
        text: `cd ${repoRoot} && cargo check 2>&1 | tail -20`,
        exit: 0,
        verdict: 'pass',
        verdict_reason: null,
        shell: 'bash',
        pipefail: true,
        source: 'agent',
        independent: true,
        timed_out: false,
        dur_ms: 10,
        tool: 'shell',
        bytes_out: 1,
      },
    }))
    writeFileSync(ledger, rows.map((row) => JSON.stringify(row)).join('\n') + '\n')

    const run = spawnSync(
      process.execPath,
      [
        'scripts/runtime/dossier-tick.mjs',
        '--force',
        '--max',
        '1',
        '--repo',
        KEY,
        '--ledger',
        ledger,
        '--graph',
        graph,
        '--cut',
        cutDir,
        '--state-dir',
        stateDir,
        '--out',
        outDir,
      ],
      { cwd: new URL('../..', import.meta.url), encoding: 'utf8' },
    )
    assert.equal(run.status, 0, run.stderr || run.stdout)
    assert.match(run.stdout, /1 probe\(s\) · 1 dossier\(s\) recompiled/)

    const artifact = JSON.parse(readFileSync(join(outDir, `${KEY}.json`), 'utf8'))
    assert.equal(artifact.facts.length, 1)
    assert.equal(artifact.facts[0].kind, 'ritual')
    assert.equal(artifact.facts[0].class, 'build')
    assert.equal(artifact.facts[0].text, 'cargo check')
    assert.equal(artifact.facts[0].verdicts, 3)
    assert.equal(artifact.facts[0].evidence.includes('3 pass'), true)
    assert.ok(
      artifact.facts[0].belief >= 0.7,
      `first verified fact clears the cockpit injection gate (${artifact.facts[0].belief})`,
    )

    const heartbeat = JSON.parse(readFileSync(join(stateDir, 'heartbeat.json'), 'utf8'))
    assert.equal(heartbeat.action, 'ran')
  } finally {
    rmSync(dir, { recursive: true, force: true })
  }
})
