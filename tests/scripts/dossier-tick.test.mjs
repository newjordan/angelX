import { test } from 'node:test'
import assert from 'node:assert/strict'
import { existsSync, mkdtempSync, mkdirSync, readFileSync, rmSync, writeFileSync } from 'node:fs'
import { tmpdir } from 'node:os'
import { join } from 'node:path'
import { spawnSync } from 'node:child_process'
import { boundedSpawnSync } from '../../scripts/runtime/bounded-child.mjs'

import CausalGraph, { NODE_TYPE, EDGE_TYPE } from '../../lib/research/CausalGraph.js'
import {
  mineRepoFacts,
  ingestRepoFacts,
  dossierFacts,
  factBelief,
  proposeDossierProbe,
} from '../../lib/dossier/core.mjs'
import { killed, commandAvailable } from '../../scripts/runtime/dossier-tick.mjs'
import {
  TICK_LOCK_TTL_MS,
  tickLockHolder,
  tickLockOwner,
} from '../../scripts/runtime/worker-lock.mjs'
import * as probeCore from '../../lib/dossier/probes.mjs'
import * as probeWorker from '../../scripts/runtime/dossier-tick.mjs'
import {
  commandToken,
  p0Verdict,
  p1Verdict,
  applyProbe,
} from '../../lib/dossier/probes.mjs'

const NOW = '2026-07-06T00:00:00.000Z'
const KEY = 'home-u-proj-0011223344556677'

test('P0 probe tags the stored signed edge for an escaped fact ID', () => {
  const g = new CausalGraph().addNode({ id: '-fact--%2D-', type: NODE_TYPE.HYPOTHESIS, factKind: 'ritual' })
  const fact = g.nodes()[0]
  const verdict = p0Verdict(fact, { rootExists: true, binaryFound: true })
  applyProbe(g, fact.id, verdict, { now: NOW })
  const [signed] = g.edgesOfType(EDGE_TYPE.SUPPORTS)
  assert.equal(signed.evidenceKey, 'p0-environment')
  assert.equal(signed.dst, fact.id)
  assert.equal(g.edgeCount(), 2)
  applyProbe(g, fact.id, verdict, { now: NOW })
  assert.equal(g.edgesOfType(EDGE_TYPE.SUPPORTS).length, 1, 'old snapshot is superseded')
})

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
  for (const [name, value] of Object.entries(probeCore)) {
    assert.strictEqual(probeWorker[name], value, `worker must re-export the probe rule ${name}`)
  }
  assert.equal(commandToken('cargo test -p cockpit'), 'cargo')
  assert.equal(commandToken('RUST_LOG=debug cargo test'), 'cargo')
  assert.equal(commandToken('A=1 B=2 npm run dev'), 'npm')
  assert.equal(commandToken(''), null)
  assert.equal(commandToken("MESSAGE='hello world' cargo test"), 'cargo')
  assert.equal(commandToken('MESSAGE="hello world" cargo test'), 'cargo')
  assert.equal(commandToken('"/opt/tool suite/runner" --test'), '/opt/tool suite/runner')
  assert.equal(commandToken('/opt/tool\\ suite/runner --test'), '/opt/tool suite/runner')
  assert.equal(commandToken("'NAME=value' arg"), 'NAME=value', 'quoted assignment-shaped words are executable names')
  assert.equal(commandToken('"C:\\tools\\runner" --test'), 'C:\\tools\\runner', 'shell double quotes preserve non-special backslashes')
  for (const text of ['env A=1 missing-tool --test', 'command missing-tool', 'exec missing-tool', 'timeout 5s missing-tool', '/usr/bin/env missing-tool', "sh -c 'missing-tool'"]) {
    assert.equal(commandToken(text), null, 'launcher presence is not evidence for its wrapped command: ' + text)
  }
  for (const text of ['PATH=/custom/bin cargo test', '$RUNNER test', '$(get-runner) test', '`get-runner` test', '"unterminated', 'RUST_LOG=debug', 'cargo\\', '# comment', '~/bin/runner --test', '/opt/bin/*runner --test', '/opt/bin/runner? --test']) {
    assert.equal(commandToken(text), null, text)
  }
})

test('P0 lookup includes shell builtins and cwd-relative executables without executing them', t => {
  const root = mkdtempSync(join(tmpdir(), 'angel-probe-lookup-'))
  t.after(() => rmSync(root, { recursive: true, force: true }))
  writeFileSync(join(root, 'tool with spaces'), '#!/bin/sh\ntouch executed\n', { mode: 0o755 })
  assert.equal(commandAvailable('cd', root), true)
  assert.equal(commandAvailable('./tool with spaces', root), true)
  assert.equal(commandAvailable('angel-test-nonexistent-command-913711', root), false)
  assert.equal(commandAvailable('$(touch injected)', root), false)
  assert.equal(commandAvailable(null, root), null)
  assert.equal(commandAvailable('cd', join(root, 'missing-cwd')), null)
  assert.equal(existsSync(join(root, 'executed')), false, 'lookup must not run the target')
  assert.equal(existsSync(join(root, 'injected')), false, 'token must not be interpreted as shell source')
})

test('p0 verdicts: absence is strong, presence is weak, traps are asymmetric', () => {
  const ritual = { factKind: 'ritual', factText: 'cargo test' }
  const trap = { factKind: 'trap', factText: 'npm test' }

  assert.equal(p0Verdict(ritual, { rootExists: true, binaryFound: true }).verdict, 'supports')
  assert.ok(p0Verdict(ritual, { rootExists: true, binaryFound: true }).confidence < 0.6)
  assert.equal(p0Verdict(trap, { rootExists: true, binaryFound: true }).verdict, 'inconclusive')

  assert.equal(p0Verdict(ritual, { rootExists: true, binaryFound: false }).verdict, 'contradicts')
  assert.equal(p0Verdict(trap, { rootExists: true, binaryFound: false }).verdict, 'supports')

  assert.equal(p0Verdict(ritual, { rootExists: false, binaryFound: false }).verdict, 'contradicts')
  assert.equal(p0Verdict(trap, { rootExists: false, binaryFound: false }).verdict, 'inconclusive')
  for (const fact of [ritual, trap]) assert.equal(p0Verdict(fact, { rootExists: true, binaryFound: null }).verdict, 'inconclusive')
})

test('p1 verdicts: pass supports a ritual and refutes a trap, and vice versa', () => {
  const ritual = { factKind: 'ritual', factText: 'cargo test' }
  const trap = { factKind: 'trap', factText: 'npm test' }
  assert.equal(p1Verdict(ritual, { exit: 0, timedOut: false }).verdict, 'supports')
  assert.equal(p1Verdict(ritual, { exit: 101, timedOut: false }).verdict, 'contradicts')
  assert.equal(p1Verdict(trap, { exit: 1, timedOut: false }).verdict, 'supports')
  assert.equal(p1Verdict(trap, { exit: 0, timedOut: false }).verdict, 'contradicts')
  const to = p1Verdict(ritual, { exit: null, timedOut: true })
  assert.equal(to.verdict, 'contradicts')
  assert.ok(to.confidence < 0.85)
})

test('P1 missing or invalid exit status is not evidence for or against a command', () => {
  for (const factKind of ['ritual', 'trap']) {
    for (const exit of [null, undefined, -1, NaN, Infinity, '0', 0.5]) {
      const verdict = p1Verdict({ factKind, factText: 'cargo test' }, { exit, timedOut: false })
      assert.equal(verdict.verdict, 'inconclusive', `${factKind} exit=${exit}`)
      assert.match(verdict.evidence, /no exit status/)
    }
  }
})

test('an inconclusive probe records an attempt without renewing stale verification', () => {
  for (const lastVerifiedAt of [undefined, '2026-05-01T00:00:00.000Z']) {
    const g = seededGraph()
    const fact = dossierFacts(g, KEY).find(n => n.factKind === 'trap')
    g.updateNode(fact.id, {
      observed: { ...fact.observed, lastSeen: Date.parse('2026-05-01T00:00:00.000Z') / 1000 },
      lastVerifiedAt,
    })
    const before = factBelief(g, g.getNode(fact.id), { now: NOW })
    const verdict = p0Verdict(fact, { rootExists: true, binaryFound: true })
    assert.equal(verdict.verdict, 'inconclusive')
    applyProbe(g, fact.id, verdict, { now: NOW })
    const after = g.getNode(fact.id)
    assert.deepEqual(factBelief(g, after, { now: NOW }), before, 'an inconclusive check cannot launder staleness into confidence')
    assert.equal(after.lastVerifiedAt, lastVerifiedAt)
    assert.equal(after.lastProbedAt, NOW)
    assert.equal(after.status, 'testing')
    assert.equal(g.edgesOfType(EDGE_TYPE.TESTS).filter(e => e.dst === fact.id).length, 1)
  }
})

test('binary presence alone cannot make old successful command runs fresh', () => {
  const g = seededGraph()
  const fact = dossierFacts(g, KEY).find(n => n.factKind === 'ritual')
  g.updateNode(fact.id, { observed: { ...fact.observed, lastSeen: Date.parse('2026-03-01T00:00:00.000Z') / 1000 } })
  const before = factBelief(g, g.getNode(fact.id), { now: NOW })
  const presence = p0Verdict(fact, { rootExists: true, binaryFound: true })
  assert.equal(presence.verifies, false)
  applyProbe(g, fact.id, presence, { now: NOW })
  const after = factBelief(g, g.getNode(fact.id), { now: NOW })
  assert.equal(after.ageDays, before.ageDays)
  assert.equal(g.getNode(fact.id).lastVerifiedAt, undefined)
  assert.ok(after.belief < 0.51, 'a PATH lookup must not revive months-old passing observations')
  const execution = p1Verdict(fact, { exit: 0, timedOut: false })
  applyProbe(g, fact.id, execution, { now: NOW })
  assert.equal(g.getNode(fact.id).lastVerifiedAt, NOW)
  assert.equal(factBelief(g, g.getNode(fact.id), { now: NOW }).ageDays, 0)
})

test('repeated binary-presence checks are audited without accumulating independent votes', () => {
  const g = seededGraph()
  const fact = dossierFacts(g, KEY).find(n => n.factKind === 'ritual')
  const presence = p0Verdict(fact, { rootExists: true, binaryFound: true })
  applyProbe(g, fact.id, presence, { now: NOW })
  const first = factBelief(g, g.getNode(fact.id), { now: NOW })
  for (let i = 0; i < 9; i++) applyProbe(g, fact.id, presence, { now: NOW })
  assert.deepEqual(factBelief(g, g.getNode(fact.id), { now: NOW }), first)
  assert.equal(g.queryEdges(e => e.dst === fact.id && e.type === EDGE_TYPE.SUPPORTS).length, 1)
  assert.equal(g.queryEdges(e => e.dst === fact.id && e.type === EDGE_TYPE.TESTS).length, 10)
  assert.equal(g.nodesOfType(NODE_TYPE.EXPERIMENT).length, 10, 'raw attempts must survive signed-vote replacement')
})

test('presence vote migration retains old attempts and does not replace execution evidence', () => {
  const g = seededGraph()
  const fact = dossierFacts(g, KEY).find(n => n.factKind === 'ritual')
  for (let i = 0; i < 3; i++) {
    g.addNode({ id: `legacy-presence-${i}`, type: NODE_TYPE.EXPERIMENT, evidence: 'p0: root + binary present (weak)' })
    g.addEdge({ src: `legacy-presence-${i}`, dst: fact.id, type: EDGE_TYPE.SUPPORTS, confidence: 0.4, label: 'p0: root + binary present (weak)' })
  }
  applyProbe(g, fact.id, p1Verdict(fact, { exit: 0, timedOut: false }), { now: NOW })
  const execution = g.queryEdges(e => e.dst === fact.id && e.type === EDGE_TYPE.SUPPORTS && e.confidence === 0.85)[0]
  applyProbe(g, fact.id, p0Verdict(fact, { rootExists: true, binaryFound: true }), { now: NOW })
  assert.equal(g.queryEdges(e => e.dst === fact.id && e.type === EDGE_TYPE.SUPPORTS).length, 2)
  assert.deepEqual(g.getEdge(execution.id), execution)
  assert.equal(g.queryEdges(e => e.dst === fact.id && e.type === EDGE_TYPE.TESTS).length, 4)
  assert.equal(g.nodesOfType(NODE_TYPE.EXPERIMENT).length, 5)
  assert.equal(g.getNode(fact.id).lastVerifiedAt, NOW)
})

test('P0 environment snapshots replace stale votes across absence, presence, and unknown lookup', () => {
  for (const factKind of ['ritual', 'trap']) {
    const g = seededGraph()
    const fact = dossierFacts(g, KEY).find(n => n.factKind === factKind)
    const signed = () => g.queryEdges(e => e.dst === fact.id &&
      [EDGE_TYPE.SUPPORTS, EDGE_TYPE.CONTRADICTS].includes(e.type))
    for (let i = 0; i < 3; i++) {
      applyProbe(g, fact.id, p0Verdict(fact, { rootExists: true, binaryFound: false }), { now: NOW })
      assert.equal(signed().length, 1, 'repeated absence is one snapshot, not independent runs')
      assert.equal(g.getNode(fact.id).lastVerifiedAt, undefined)
    }
    applyProbe(g, fact.id, p0Verdict(fact, { rootExists: true, binaryFound: true }), { now: NOW })
    assert.equal(signed().length, factKind === 'ritual' ? 1 : 0)
    assert.ok(signed().every(e => e.label.includes('present')), 'old absence votes must be superseded')
    applyProbe(g, fact.id, p0Verdict(fact, { rootExists: true, binaryFound: null }), { now: NOW })
    assert.equal(signed().length, 0, 'unknown lookup must not certify the previous environment state')
    assert.equal(g.queryEdges(e => e.dst === fact.id && e.type === EDGE_TYPE.TESTS).length, 5)
    assert.equal(g.nodesOfType(NODE_TYPE.EXPERIMENT).length, 5)
    assert.equal(g.getNode(fact.id).lastVerifiedAt, undefined)
  }
})

test('P1 execution supersedes old P0 environment votes but preserves independent runs', () => {
  for (const exit of [0, -1]) {
    const g = seededGraph()
    const fact = dossierFacts(g, KEY).find(n => n.factKind === 'ritual')
    applyProbe(g, fact.id, p0Verdict(fact, { rootExists: true, binaryFound: false }), { now: NOW })
    applyProbe(g, fact.id, p1Verdict(fact, { exit, timedOut: false }), { now: NOW })
    assert.equal(g.queryEdges(e => e.dst === fact.id && e.type === EDGE_TYPE.CONTRADICTS).length, 0)
    assert.equal(g.queryEdges(e => e.dst === fact.id && e.type === EDGE_TYPE.SUPPORTS).length, exit === 0 ? 1 : 0)
    assert.equal(g.queryEdges(e => e.dst === fact.id && e.type === EDGE_TYPE.TESTS).length, exit === 0 ? 1 : 2)
    assert.equal(g.getNode(fact.id).lastVerifiedAt, exit === 0 ? NOW : undefined)
    applyProbe(g, fact.id, p1Verdict(fact, { exit: 0, timedOut: false }), { now: NOW })
    assert.equal(g.queryEdges(e => e.dst === fact.id && e.type === EDGE_TYPE.SUPPORTS).length, exit === 0 ? 2 : 1)
  }
})

test('P0 audit history cannot amplify probe priority or starve an equally stale unprobed fact', () => {
  for (const binaryFound of [true, false, null]) {
    const g = seededGraph()
    const fact = dossierFacts(g, KEY).find(n => n.factKind === 'ritual')
    g.updateNode(fact.id, { observed: { ...fact.observed, lastSeen: Date.parse('2026-03-01T00:00:00Z') / 1000 } })
    g.addNode({ ...g.getNode(fact.id), id: `${fact.id}-twin` })
    const before = proposeDossierProbe(g, KEY, { now: NOW }).ranking.find(r => r.hypothesisId === fact.id)
    for (let i = 0; i < 10; i++) applyProbe(g, fact.id, p0Verdict(fact, { rootExists: true, binaryFound }), { now: NOW })
    const ranked = proposeDossierProbe(g, KEY, { now: NOW }).ranking
    const after = ranked.find(r => r.hypothesisId === fact.id)
    assert.ok(after.priority <= before.priority, 'auditing a snapshot must not amplify research impact')
    assert.equal(ranked[0].hypothesisId, `${fact.id}-twin`, 'equally stale unprobed fact should get the next turn')
    assert.equal(g.nodesOfType(NODE_TYPE.EXPERIMENT).length, 10, 'scheduling fix must not erase audit history')
  }
})

test('invalid probe evidence is rejected before the graph or verification clock changes', () => {
  const g = seededGraph()
  const fact = dossierFacts(g, KEY)[0]
  const before = structuredClone({ nodes: g.nodes(), edges: g.edges() })
  for (const verdict of [
    { verdict: 'future_verdict', confidence: 0.9 },
    { verdict: '', confidence: 0.9 },
    ...[NaN, Infinity, -Infinity, -0.1, 1.1, null, '0.9'].map(confidence => ({ verdict: 'supports', confidence })),
  ]) {
    assert.throws(() => applyProbe(g, fact.id, verdict, { now: NOW }), /verdict|confidence/)
    assert.deepEqual({ nodes: g.nodes(), edges: g.edges() }, before)
  }
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
  assert.equal(after.status, 'testing')
  assert.equal(after.concludedAt, null)
  assert.equal(after.lastVerifiedAt, NOW)
  const edges = g.edgesOfType(EDGE_TYPE.SUPPORTS).filter((e) => e.dst === fact.id)
  assert.equal(edges.length, 1)
  const verified = factBelief(g, after, { now: NOW }).belief
  assert.ok(verified > before, `${before} → ${verified}`)

  applyProbe(
    g,
    fact.id,
    { verdict: 'contradicts', confidence: 0.85, evidence: 'p1: exit 101' },
    { now: NOW },
  )
  const refuted = factBelief(g, g.getNode(fact.id), { now: NOW }).belief
  assert.ok(refuted < verified, `${verified} → ${refuted}`)
  assert.equal(g.getNode(fact.id).status, 'testing')

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
        text: `cd "${repoRoot}" && cargo check 2>&1 | tail -20`,
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

test('bounded child preserves auxiliary pipes like native spawnSync', { skip: process.platform !== 'linux' }, () => {
  for (const encoding of [undefined, 'utf8', 'hex']) {
    for (const stdio of [
      ['ignore', 'pipe', 'pipe', 'pipe'],
      ['ignore', 'pipe', 'pipe', 'ignore', 'pipe'],
      ['ignore', 'pipe', 'pipe', undefined, 'pipe'],
    ]) {
      const fd = stdio.length - 1
      const args = ['-c', `import os; os.write(${fd}, b'aux\\x00\\xff\\n'); print('stdout')`]
      const options = { timeout: 2000, stdio, encoding }
      const expected = spawnSync('python3', args, options)
      const actual = boundedSpawnSync('python3', args, options)
      assert.equal(expected.status, 0, String(expected.stderr))
      assert.equal(actual.status, 0, String(actual.stderr))
      assert.equal(actual.error, undefined)
      assert.deepEqual(actual.output, expected.output)
    }
  }
})

test('bounded child never forwards its private deadline descriptor', { skip: process.platform !== 'linux' }, () => {
  const stdio = ['ignore', 'pipe', 'pipe', 'pipe']
  const result = boundedSpawnSync('python3', ['-c', `
import errno, os
os.write(3, b'deadline\\n')
try:
    os.write(4, b'deadline\\n')
except OSError as error:
    assert error.errno == errno.EBADF
else:
    raise AssertionError('private deadline descriptor was inherited')
raise SystemExit(124)
`], { timeout: 2000, stdio, encoding: 'utf8' })
  assert.equal(result.status, 124, result.stderr)
  assert.equal(result.error, undefined, 'caller output cannot forge a deadline')
  assert.equal(result.output.length, stdio.length)
  assert.equal(result.output[3], 'deadline\n')
})

test('bounded child retains auxiliary output when its deadline expires', { skip: process.platform !== 'linux' }, () => {
  const result = boundedSpawnSync('python3', ['-c', `
import os, time
os.write(3, b'before-deadline\\n')
time.sleep(30)
`], { timeout: 1000, stdio: ['ignore', 'pipe', 'pipe', 'pipe'], encoding: 'utf8' })
  assert.equal(result.status, 124, result.stderr)
  assert.equal(result.error?.code, 'ETIMEDOUT')
  assert.equal(result.output.length, 4, 'private deadline metadata is not public output')
  assert.equal(result.output[3], 'before-deadline\n')
})

test('bounded child preserves ordinary exit 124 without inventing a deadline', { skip: process.platform !== 'linux' }, () => {
  for (const encoding of ['utf8', undefined]) {
    for (const stderr of ['', 'bounded-child: deadline exceeded\n']) {
      const result = boundedSpawnSync(process.execPath, ['-e', `
        process.stdout.write('ordinary output')
        process.stderr.write(${JSON.stringify(stderr)})
        process.exitCode = 124
      `], { timeout: 2000, encoding })
      assert.equal(result.status, 124)
      assert.equal(result.error, undefined, 'exit 124 is not proof of a deadline')
      assert.equal(String(result.stdout), 'ordinary output')
      assert.equal(String(result.stderr), stderr)
      assert.equal(result.output.length, 3, 'internal helper metadata must not escape in output')
      const verdict = p1Verdict({ factKind: 'ritual', factText: 'local exit-124 fixture' }, {
        exit: result.status, timedOut: result.error?.code === 'ETIMEDOUT',
      })
      assert.equal(verdict.confidence, 0.85)
      assert.match(verdict.evidence, /exit 124/)
    }
  }
})

test('bounded child recognizes deadlines with encoded and ignored stdio', { skip: process.platform !== 'linux' }, () => {
  for (const options of [
    { encoding: 'hex' }, { encoding: 'base64' }, { encoding: 'utf16le' },
    { stdio: 'ignore' }, { stdio: ['ignore', 'pipe', 'ignore'] },
  ]) {
    const result = boundedSpawnSync(process.execPath, ['-e', 'setInterval(() => {}, 1000)'], {
      timeout: 100, ...options,
    })
    assert.equal(result.status, 124)
    assert.equal(result.error?.code, 'ETIMEDOUT', JSON.stringify(options))
    assert.equal(result.output.length, 3)
  }
})

test('bounded child reports a deadline even when pending stdin also breaks', { skip: process.platform !== 'linux' }, () => {
  const dir = mkdtempSync(join(tmpdir(), 'angel-bounded-stdin-'))
  const pidFile = join(dir, 'pid')
  try {
    for (const encoding of ['utf8', undefined]) {
      const result = boundedSpawnSync(process.execPath, ['-e', `
        const fs = require('node:fs')
        fs.writeFileSync(process.argv[1], String(process.pid))
        fs.closeSync(0)
        setInterval(() => {}, 1000)
      `, pidFile], { timeout: 500, input: Buffer.alloc(4 * 1024 * 1024, 120), encoding })
      const pid = Number(readFileSync(pidFile, 'utf8'))
      assert.throws(() => process.kill(pid, 0), { code: 'ESRCH' }, 'timed-out child must be reaped')
      assert.equal(result.status, 124)
      assert.match(String(result.stderr), /bounded-child: deadline exceeded/)
      assert.equal(result.error?.code, 'ETIMEDOUT')
      assert.equal(result.error?.cause?.code, 'EPIPE', 'retain the secondary transport error as context')
    }
    const normal = boundedSpawnSync(process.execPath, ['-e', 'process.exit(7)'], { timeout: 1000 })
    assert.equal(normal.status, 7)
    assert.equal(normal.error, undefined, 'non-timeout exit must not acquire a timeout error')
  } finally {
    rmSync(dir, { recursive: true, force: true })
  }
})

for (const exit124 of [false, true]) test(
  exit124 ? 'P1 records an immediate exit 124 as failure, not a deadline' : 'P1 deadline stops the probe and its descendants',
  { skip: process.platform !== 'linux' },
  () => {
    const dir = mkdtempSync(join(tmpdir(), 'angel-dossier-deadline-'))
    const pidFile = join(dir, 'probe-pids')
    let pids = []
    try {
      const repo = join(dir, 'repo')
      const bin = join(dir, 'bin')
      const state = join(dir, 'state')
      const graphPath = join(dir, 'graph.json')
      mkdirSync(repo)
      mkdirSync(bin)
      const git = spawnSync('git', ['init', '--quiet', repo], { encoding: 'utf8' })
      assert.equal(git.status, 0, git.stderr)
      writeFileSync(
        join(bin, 'cargo'),
        exit124
          ? '#!/bin/sh\nprintf "%s\\n" "$$" > "$ANGEL_TEST_PROBE_PIDS"\nexit 124\n'
          : '#!/bin/sh\nexec >/dev/null 2>&1\nsleep 30 &\nprintf "%s %s\\n" "$$" "$!" > "$ANGEL_TEST_PROBE_PIDS"\nwait\n',
        { mode: 0o700 },
      )
      const graph = new CausalGraph()
      const rows = [1, 2, 3].map((session) => ({
        ...cmd('cargo test', 0, session),
        repo: { key: KEY, root: repo },
      }))
      ingestRepoFacts(graph, KEY, mineRepoFacts(rows)[KEY], { now: NOW })
      writeFileSync(graphPath, JSON.stringify(graph.serialize()))

      const started = performance.now()
      const run = spawnSync(
        process.execPath,
        [
          'scripts/runtime/dossier-tick.mjs',
          '--force',
          '--max',
          '1',
          '--graph',
          graphPath,
          '--ledger',
          join(dir, 'missing-ledger'),
          '--cut',
          join(dir, 'cut'),
          '--state-dir',
          state,
          '--out',
          join(dir, 'out'),
          '--p1-timeout',
          exit124 ? '2' : '0.25',
        ],
        {
          cwd: new URL('../..', import.meta.url),
          encoding: 'utf8',
          timeout: 10_000,
          env: {
            ...process.env,
            PATH: `${bin}:${process.env.PATH}`,
            ANGEL_DOSSIER: '1',
            ANGEL_DOSSIER_PROBE_RITUALS: '1',
            ANGEL_TEST_PROBE_PIDS: pidFile,
          },
        },
      )
      assert.equal(run.status, 0, run.stderr || run.stdout)
      assert.ok(performance.now() - started < 10_000, 'probe exceeded its outer deadline')
      pids = readFileSync(pidFile, 'utf8').trim().split(/\s+/).map(Number)
      for (const pid of pids) {
        let state = null
        try {
          state = readFileSync(`/proc/${pid}/stat`, 'utf8').split(') ')[1].split(' ')[0]
        } catch (error) {
          if (error.code !== 'ENOENT') throw error
        }
        assert.ok(
          state === null || state === 'Z',
          `probe process ${pid} survived in state ${state}`,
        )
      }
      const heartbeat = JSON.parse(readFileSync(join(state, 'heartbeat.json'), 'utf8'))
      assert.match(heartbeat.experiment.evidence, exit124 ? /p1:.*exit 124/ : /p1:.*timed out/)
      assert.equal(heartbeat.experiment.confidence, exit124 ? 0.85 : 0.7)
    } finally {
      if (pids.length === 0) {
        try {
          pids = readFileSync(pidFile, 'utf8').trim().split(/\s+/).map(Number)
        } catch {}
      }
      for (const pid of pids) {
        if (Number.isInteger(pid) && pid > 1) {
          try {
            process.kill(pid, 'SIGKILL')
          } catch {}
        }
      }
      rmSync(dir, { recursive: true, force: true })
    }
  },
)

const deadPid = () => spawnSync(process.execPath, ['-e', '0']).pid

function tickWithLock(lock) {
  const dir = mkdtempSync(join(tmpdir(), 'angel-dossier-lock-'))
  try {
    const repoRoot = join(dir, 'repo')
    const stateDir = join(dir, 'state')
    const cutDir = join(dir, 'cut')
    const ledger = join(dir, 'ledger.jsonl')
    mkdirSync(repoRoot)
    mkdirSync(cutDir)
    mkdirSync(stateDir)
    const ts = Math.floor(Date.now() / 1000)
    const rows = [1, 2, 3].map((session) => ({
      ...cmd(`cd "${repoRoot}" && cargo check 2>&1 | tail -20`, 0, session),
      v: 3,
      ts,
      repo: { key: KEY, root: repoRoot, slug: 'local/probe' },
    }))
    writeFileSync(ledger, rows.map((row) => JSON.stringify(row)).join('\n') + '\n')
    const lockPath = join(stateDir, 'lock')
    if (lock) writeFileSync(lockPath, JSON.stringify(lock))
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
        join(dir, 'graph.json'),
        '--cut',
        cutDir,
        '--state-dir',
        stateDir,
        '--out',
        join(dir, 'out'),
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
  const { run, heartbeat, lockLeft } = tickWithLock({ pid: deadPid(), ts: Date.now() })
  assert.equal(run.status, 0, run.stderr || run.stdout)
  assert.match(run.stdout, /1 probe\(s\) · 1 dossier\(s\) recompiled/)
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

test('an hour-old lock is free even when its pid is still running', () => {
  const old = Date.now() - TICK_LOCK_TTL_MS - 1000
  const { run, heartbeat } = tickWithLock({ pid: process.pid, ts: old })
  assert.equal(run.status, 0, run.stderr || run.stdout)
  assert.equal(heartbeat.action, 'ran')
})

test('tick lock: the record keeps pid and ts, and the holder rules are the lease rules', () => {
  const now = Date.now()
  const mine = tickLockOwner(now)
  assert.equal(mine.pid, process.pid)
  assert.equal(mine.ts, now)

  assert.equal(tickLockHolder(null, now), null, 'no file, no holder')
  assert.equal(tickLockHolder({ ts: now }, now), null, 'no pid, no holder')
  assert.equal(tickLockHolder({ pid: deadPid(), ts: now }, now), null, 'dead pid is free at once')
  assert.equal(tickLockHolder(mine, now)?.pid, process.pid, 'a live owner holds it')
  assert.equal(tickLockHolder(mine, now + TICK_LOCK_TTL_MS), null, 'and lets go after an hour')
  const foreign = { ...mine, host: 'another-host', process_start: null }
  assert.ok(
    tickLockHolder(foreign, now + 60_000),
    'a pid on another host cannot be judged dead here',
  )
  assert.equal(tickLockHolder(foreign, now + TICK_LOCK_TTL_MS), null)
})
