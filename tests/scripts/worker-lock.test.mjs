import { test } from 'node:test'
import assert from 'node:assert/strict'
import { mkdtempSync, mkdirSync, readFileSync, writeFileSync, existsSync, rmSync } from 'node:fs'
import { tmpdir } from 'node:os'
import { dirname, join } from 'node:path'
import { spawnSync } from 'node:child_process'
import { holdProcessLease } from '../../lib/control/process-lease.mjs'
import { runGraphCli } from '../../scripts/runtime/worker-lock.mjs'

const initialGraph = JSON.stringify({ version: 1, nodes: [{ id: 'keep', type: 'dataset' }], edges: [] }) + '\n'

for (const [script, action] of [['repo-dossier.mjs', '--rank'], ['habitsmith.mjs', '--propose']]) {
  for (const filename of ['--mine', '--dry-run']) {
    test(`configured graph ${filename} is data, not an action (${script})`, () => {
      const root = mkdtempSync(join(tmpdir(), 'angel-graph-option-name-'))
      try {
        const graph = join(root, filename)
        writeFileSync(graph, initialGraph)
        const invoke = (args) => spawnSync(process.execPath, [
          join(import.meta.dirname, '../../scripts/runtime', script), ...args,
        ], { cwd: root, env: {
          PATH: process.env.PATH, HOME: join(root, 'home'), USERPROFILE: join(root, 'home'),
          ANGEL_CAUSAL_GRAPH: filename, ANGEL_EXPERIENCE_LOG: join(root, 'missing-ledger'),
          ANGEL_DOSSIER_DIR: join(root, 'dossier'), ANGEL_CUT_DIR: join(root, 'cut'),
          ANGEL_HABITS_DIR: join(root, 'habits'), ANGEL_HABIT_PROPOSED_DIR: join(root, 'proposals'),
          ANGEL_SKILLS_DIR: join(root, 'skills'),
        }, encoding: 'utf8', timeout: 10_000 })
        const read = invoke([action])
        assert.equal(read.error, undefined)
        assert.equal(read.status, 0, read.stderr)
        assert.equal(readFileSync(graph, 'utf8'), initialGraph)
        const write = invoke(['--mine'])
        assert.equal(write.error, undefined)
        assert.equal(write.status, 0, write.stderr)
        assert.match(write.stdout, / mine →/u)
        assert.notEqual(readFileSync(graph, 'utf8'), initialGraph)
        assert.equal(existsSync(`${graph}.lease`), false)
        assert.equal(existsSync(`${graph}.lock`), false)
      } finally {
        rmSync(root, { recursive: true, force: true })
      }
    })
  }
}

test('graph CLI shares its resolved target and restores the inherited token after failure', async () => {
  const root = mkdtempSync(join(tmpdir(), 'angel-graph-target-'))
  const graph = join(root, 'selected.json')
  const previous = process.env.ANGEL_GRAPH_LEASE_TOKEN
  process.env.ANGEL_GRAPH_LEASE_TOKEN = 'unrelated-inherited-token'
  try {
    await assert.rejects(runGraphCli(async (argv, selectedGraph) => {
      assert.equal(selectedGraph, graph)
      assert.deepEqual(argv, ['--out', join(root, 'artifacts')])
      const owner = JSON.parse(readFileSync(`${graph}.lock`, 'utf8'))
      assert.equal(process.env.ANGEL_GRAPH_LEASE_TOKEN, owner.token)
      assert.notEqual(owner.token, 'unrelated-inherited-token')
      const before = readFileSync(`${graph}.lock`, 'utf8')
      assert.equal(await runGraphCli((childArgv, childGraph) => {
        assert.equal(childGraph, graph)
        assert.deepEqual(childArgv, ['--graph', graph])
        return 42
      }, ['--graph', graph]), 42)
      assert.equal(readFileSync(`${graph}.lock`, 'utf8'), before)
      throw new Error('fixture callback failed')
    }, ['--out', join(root, 'artifacts')], graph), /fixture callback failed/u)
    assert.equal(process.env.ANGEL_GRAPH_LEASE_TOKEN, 'unrelated-inherited-token')
    assert.equal(existsSync(`${graph}.lease`), false)
    assert.equal(existsSync(`${graph}.lock`), false)
    assert.equal(existsSync(join(root, 'artifacts')), false)
  } finally {
    if (previous === undefined) delete process.env.ANGEL_GRAPH_LEASE_TOKEN
    else process.env.ANGEL_GRAPH_LEASE_TOKEN = previous
    rmSync(root, { recursive: true, force: true })
  }
})

test('dry-run resolves the selected graph without creating directories or taking a lease', async () => {
  const root = mkdtempSync(join(tmpdir(), 'angel-graph-target-'))
  const graph = join(root, 'missing', 'graph.json')
  try {
    assert.equal(await runGraphCli((argv, selectedGraph) => {
      assert.equal(selectedGraph, graph)
      assert.deepEqual(argv, ['--dry-run'])
      return 17
    }, ['--dry-run'], graph), 17)
    assert.equal(existsSync(dirname(graph)), false)
  } finally {
    rmSync(root, { recursive: true, force: true })
  }
})

for (const [script, args, outSelectsGraph] of [
  ['dossier-tick.mjs', ['--force', '--max', '0'], false],
  ['habitsmith-tick.mjs', ['--force'], false],
  ['habitsmith.mjs', ['--mine'], false],
  ['repo-dossier.mjs', ['--mine'], true],
]) {
  for (const override of ['default', 'out', 'env', 'explicit', 'blank-env', 'empty', 'missing', 'flag-operand', 'dry-run']) {
    test(`graph CLI ${script} leases its write target (${override})`, async () => {
      const root = mkdtempSync(join(tmpdir(), 'angel-graph-target-'))
      const home = join(root, 'home')
      const dossier = join(root, 'dossier')
      const out = join(root, 'artifacts')
      const invalid = ['empty', 'missing', 'flag-operand'].includes(override)
      const graph = override === 'explicit'
        ? join(root, 'explicit', 'graph.json')
        : override === 'env'
          ? join(root, 'environment', 'graph.json')
          : outSelectsGraph && !invalid && override !== 'default'
            ? join(out, 'graph.json')
            : join(dossier, 'graph.json')
      const argv = [...args]
      if (!invalid && override !== 'default') argv.push('--out', out)
      if (override === 'explicit') argv.push('--graph', graph)
      if (override === 'empty') argv.push('--graph', '')
      if (override === 'missing') argv.push('--graph')
      if (override === 'flag-operand') argv.push('--graph', '--dry-run')
      if (override === 'dry-run') argv.push('--dry-run')
      const env = {
        PATH: process.env.PATH,
        HOME: home,
        USERPROFILE: home,
        ANGEL_DOSSIER_DIR: dossier,
        ANGEL_HABITS_DIR: join(root, 'habits'),
        ANGEL_EXPERIENCE_LOG: join(root, 'missing-ledger.jsonl'),
        ANGEL_CUT_DIR: join(root, 'missing-cut'),
        ANGEL_CAUSAL_GRAPH: override === 'env' ? graph : override === 'blank-env' ? '   ' : '',
      }
      const leasePaths = { leaseDir: `${graph}.lease`, compatibilityPath: `${graph}.lock` }
      mkdirSync(dirname(graph), { recursive: true })
      writeFileSync(graph, initialGraph)
      try {
        const held = await holdProcessLease(() => {
          const owner = readFileSync(leasePaths.compatibilityPath, 'utf8')
          const result = spawnSync(process.execPath, [
            join(import.meta.dirname, '../../scripts/runtime', script), ...argv,
          ], { env, cwd: root, encoding: 'utf8', timeout: 10_000 })
          assert.equal(result.error, undefined)
          assert.equal(result.status, invalid ? 2 : override === 'dry-run' ? 0 : 75, result.stderr || result.stdout)
          if (invalid) assert.match(result.stderr, /--graph requires a non-empty path/u)
          else if (override !== 'dry-run') assert.match(result.stderr, /graph busy:/u)
          assert.equal(readFileSync(graph, 'utf8'), initialGraph)
          assert.equal(readFileSync(leasePaths.compatibilityPath, 'utf8'), owner)
          assert.equal(existsSync(join(root, 'habits')), false)
          if (graph !== join(out, 'graph.json')) assert.equal(existsSync(out), false)
        }, leasePaths)
        assert.equal(held.acquired, true)
        assert.equal(existsSync(leasePaths.leaseDir), false)
        assert.equal(existsSync(leasePaths.compatibilityPath), false)
      } finally {
        rmSync(root, { recursive: true, force: true })
      }
    })
  }
}
