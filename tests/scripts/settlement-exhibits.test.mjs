import { test } from 'node:test'
import assert from 'node:assert/strict'
import fs from 'node:fs'
import os from 'node:os'
import path from 'node:path'
import { spawnSync } from 'node:child_process'
import { fileURLToPath } from 'node:url'
import { encodeManifest, manifestPath, validateManifest } from '../../lib/settlement/exhibits.mjs'
import { publishManifest } from '../../scripts/runtime/settlement-exhibits.mjs'

const cli = fileURLToPath(new URL('../../scripts/runtime/settlement-exhibits.mjs', import.meta.url))
const linuxPublisher = { skip: process.platform !== 'linux' ? 'Linux publisher uses /proc/self/fd' : false }
function fixture(t) {
  const root = fs.mkdtempSync(path.join(os.tmpdir(), 'settlement-publish-'))
  t.after(() => fs.rmSync(root, { recursive: true, force: true }))
  fs.mkdirSync(path.join(root, 'reports'))
  fs.writeFileSync(path.join(root, 'reports/result-1.txt'), 'Measured 37 samples. Inconclusive.\n')
  const manifest = { schema: 'angel.settlement-exhibits/v1', site_id: 'site-test', loop_id: 'loop-test', iteration: 1,
    artifacts: [{ key: 'result-1', title: 'Local report', source: 'reports/result-1.txt', status: 'reported' }] }
  return { root, name: 'research-1', manifest }
}

test('manifest rules reject unknown fields, false verification, unsafe names/paths and bounds', () => {
  for (const name of ['', '.', '../a', 'a.json', 'a b', 'a\nb', 'a\n', 'a\r', '_a', '-a', ' a', 'a ', 'é', 'a'.repeat(65)])
    assert.throws(() => manifestPath(name))
  assert.equal(manifestPath('A_1-b'), '.angelX/settlement-exhibits/A_1-b.json')
  const valid = { schema: 'angel.settlement-exhibits/v1', site_id: 'site', loop_id: 'loop', iteration: 2,
    artifacts: [{ key: 'result', title: 'Report', source: 'reports/a.txt', status: 'failed' }] }
  assert.deepEqual(JSON.parse(encodeManifest(valid)), valid)
  for (const patch of [{ iteration: 0 }, { iteration: 1.5 }, { iteration: Number.MAX_SAFE_INTEGER + 1 },
    { schema: 'other' }, { extra: true }, { site_id: '' }, { artifacts: [] },
    { artifacts: [valid.artifacts[0], valid.artifacts[0]] }])
    assert.throws(() => validateManifest({ ...valid, ...patch }))
  for (const patch of [{ status: 'verified' }, { title: 'é'.repeat(65) }, { title: '\uD800' },
    { extra: 1 }, { source: '/tmp/report' }, { source: '../result' }, { source: 'a/./b' },
    { source: 'a//b' }, { source: 'a\\b' }, { source: 'a/'.repeat(121) }, { key: 'bad key' }, { key: 'key\n' }])
    assert.throws(() => validateManifest({ ...valid, artifacts: [{ ...valid.artifacts[0], ...patch }] }))
})

test('publication is private, complete, idempotent and never overwrites another named batch', linuxPublisher, (t) => {
  const f = fixture(t)
  const result = publishManifest(f)
  assert.equal(result.created, true)
  assert.equal(result.command, '/dungeon deposit research-1')
  assert.match(result.authority, /Published only/)
  const target = path.join(f.root, result.manifest)
  assert.equal(fs.readFileSync(target, 'utf8'), encodeManifest(f.manifest))
  assert.equal(fs.statSync(target).mode & 0o777, 0o600)
  assert.equal(fs.statSync(path.dirname(target)).mode & 0o777, 0o700)
  assert.equal(fs.statSync(target).nlink, 1)
  assert.equal(publishManifest(f).created, false)
  const changed = structuredClone(f.manifest)
  changed.artifacts[0].title = 'Different claim'
  assert.throws(() => publishManifest({ ...f, manifest: changed }), /different bytes/)
  assert.equal(fs.readFileSync(target, 'utf8'), encodeManifest(f.manifest))
  assert.equal(publishManifest({ ...f, name: 'research-2', manifest: changed }).created, true)
  assert.deepEqual(fs.readdirSync(path.dirname(target)).sort(), ['research-1.json', 'research-2.json'])
})

test('invalid, missing, oversized and non-UTF8 sources leave no manifest directory', linuxPublisher, (t) => {
  const f = fixture(t)
  const source = path.join(f.root, f.manifest.artifacts[0].source)
  for (const body of [Buffer.alloc(65537, 65), Buffer.from([0xff, 0xfe])]) {
    fs.writeFileSync(source, body)
    assert.throws(() => publishManifest(f))
    assert.equal(fs.existsSync(path.join(f.root, '.angelX')), false)
  }
  fs.unlinkSync(source)
  assert.throws(() => publishManifest(f))
  assert.equal(fs.existsSync(path.join(f.root, '.angelX')), false)
})

test('source symlinks, linked files, directories and parent symlinks are refused', linuxPublisher, (t) => {
  const f = fixture(t)
  const source = path.join(f.root, f.manifest.artifacts[0].source)
  const real = path.join(f.root, 'actual.txt')
  fs.renameSync(source, real)
  fs.symlinkSync(real, source)
  assert.throws(() => publishManifest(f))
  fs.unlinkSync(source)
  fs.linkSync(real, source)
  assert.throws(() => publishManifest(f))
  fs.unlinkSync(source)
  fs.mkdirSync(source)
  assert.throws(() => publishManifest(f))
  fs.rmdirSync(source)
  fs.renameSync(real, source)
  fs.renameSync(path.join(f.root, 'reports'), path.join(f.root, 'real-reports'))
  fs.symlinkSync('real-reports', path.join(f.root, 'reports'))
  assert.throws(() => publishManifest(f))
  assert.equal(fs.existsSync(path.join(f.root, '.angelX')), false)
})

test('symlink workspace and queue paths cannot escape into other directories', linuxPublisher, (t) => {
  const f = fixture(t)
  const alias = `${f.root}-alias`
  fs.symlinkSync(f.root, alias)
  t.after(() => fs.unlinkSync(alias))
  assert.throws(() => publishManifest({ ...f, root: alias }))
  assert.throws(() => publishManifest({ ...f, root: `${f.root}/../${path.basename(f.root)}` }))
  fs.mkdirSync(path.join(f.root, 'outside'))
  fs.symlinkSync('outside', path.join(f.root, '.angelX'))
  assert.throws(() => publishManifest(f))
  assert.deepEqual(fs.readdirSync(path.join(f.root, 'outside')), [])
  fs.unlinkSync(path.join(f.root, '.angelX'))
  fs.mkdirSync(path.join(f.root, '.angelX'))
  fs.symlinkSync('../outside', path.join(f.root, '.angelX/settlement-exhibits'))
  assert.throws(() => publishManifest(f))
  assert.deepEqual(fs.readdirSync(path.join(f.root, 'outside')), [])
})

test('existing symlink or hardlinked destinations are refused without changing the target', linuxPublisher, (t) => {
  const f = fixture(t)
  const dir = path.join(f.root, '.angelX/settlement-exhibits')
  fs.mkdirSync(dir, { recursive: true })
  const sentinel = path.join(f.root, 'sentinel')
  fs.writeFileSync(sentinel, encodeManifest(f.manifest))
  const target = path.join(dir, 'research-1.json')
  fs.symlinkSync(sentinel, target)
  assert.throws(() => publishManifest(f))
  fs.unlinkSync(target)
  fs.linkSync(sentinel, target)
  assert.throws(() => publishManifest(f))
  assert.equal(fs.readFileSync(sentinel, 'utf8'), encodeManifest(f.manifest))
  assert.deepEqual(fs.readdirSync(dir), ['research-1.json'])
})

test('credentials cause refusal, not mutation of research files', linuxPublisher, (t) => {
  const f = fixture(t)
  const secret = 'sk-' + 'aB3dE5fG7hI9jK1mN3pQ5rS7tU9vW1xY3zA5bC7dE9fG'
  const body = `OPENAI_API_KEY=${secret}\n`
  const source = path.join(f.root, f.manifest.artifacts[0].source)
  fs.writeFileSync(source, body)
  assert.throws(() => publishManifest(f))
  assert.equal(fs.readFileSync(source, 'utf8'), body)
  assert.equal(fs.existsSync(path.join(f.root, '.angelX')), false)
})

test('concurrent publishers cannot replace a competing complete manifest', linuxPublisher, async (t) => {
  const f = fixture(t)
  const other = structuredClone(f.manifest)
  other.artifacts[0].title = 'Competing report'
  // Launch actual competing CLI processes; stdin carries private manifest data.
  const { spawn } = await import('node:child_process')
  const run = (manifest) => new Promise((resolve, reject) => {
    const child = spawn(process.execPath, [cli, 'publish', f.root, f.name], { stdio: ['pipe', 'pipe', 'pipe'] })
    child.on('error', reject)
    child.stdout.resume(); child.stderr.resume()
    child.on('close', (code) => resolve(code))
    child.stdin.end(JSON.stringify(manifest))
  })
  const results = await Promise.all([run(f.manifest), run(other)])
  assert.deepEqual(results.sort(), [0, 1])
  const target = path.join(f.root, manifestPath(f.name))
  const body = fs.readFileSync(target, 'utf8')
  assert.ok([encodeManifest(f.manifest), encodeManifest(other)].includes(body))
  assert.equal(fs.statSync(target).nlink, 1)
  assert.deepEqual(fs.readdirSync(path.dirname(target)), ['research-1.json'])
})

test('CLI publishes from bounded stdin; errors do not disclose private input', linuxPublisher, (t) => {
  const f = fixture(t)
  const run = (input, args = ['publish', f.root, f.name]) => spawnSync(process.execPath, [cli, ...args], { input, encoding: 'utf8' })
  const result = run(JSON.stringify(f.manifest))
  assert.equal(result.status, 0, result.stderr)
  assert.equal(JSON.parse(result.stdout).created, true)
  assert.equal(run('SHOULD-NOT-APPEAR').status, 1)
  const invalid = run('SHOULD-NOT-APPEAR')
  assert.doesNotMatch(invalid.stderr + invalid.stdout, /SHOULD-NOT-APPEAR/)
  assert.equal(run('a'.repeat(16385)).status, 1)
  assert.equal(run('{}', ['publish', f.root, '../bad']).status, 1)
  assert.equal(run('', ['--help']).status, 0)
})


test('credential-shaped names and externally writable queue directories are refused', linuxPublisher, (t) => {
  const f = fixture(t)
  assert.throws(() => publishManifest({ ...f, name: 'ghp_' + 'a'.repeat(36) }))
  assert.equal(fs.existsSync(path.join(f.root, '.angelX')), false)
  const dir = path.join(f.root, '.angelX/settlement-exhibits')
  fs.mkdirSync(dir, { recursive: true })
  fs.chmodSync(dir, 0o777)
  assert.throws(() => publishManifest(f), /owner-controlled/)
  assert.equal(fs.statSync(dir).mode & 0o777, 0o777)
  assert.deepEqual(fs.readdirSync(dir), [])
})

test('staging failure cleans temporary files and does not publish partial JSON', linuxPublisher, (t) => {
  const f = fixture(t)
  const sync = fs.fsyncSync
  fs.fsyncSync = (fd) => {
    if (fs.fstatSync(fd).isFile()) throw new Error('injected staging sync failure')
    return sync(fd)
  }
  try { assert.throws(() => publishManifest(f), /staging sync failure/) }
  finally { fs.fsyncSync = sync }
  assert.deepEqual(fs.readdirSync(path.join(f.root, '.angelX/settlement-exhibits')), [])
  assert.equal(publishManifest(f).created, true)
})
