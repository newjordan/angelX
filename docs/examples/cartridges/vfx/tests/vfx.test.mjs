// SPDX-License-Identifier: MIT
import { test, before, after } from 'node:test'
import assert from 'node:assert/strict'
import { mkdtemp, readFile, writeFile, rm, symlink, readdir } from 'node:fs/promises'
import { join } from 'node:path'
import { tmpdir } from 'node:os'
import { spawn, execFile } from 'node:child_process'
import { promisify } from 'node:util'
import { inflateSync } from 'node:zlib'
import { TYPES, normalize, fromPrompt, wire, request, RENDER_SCHEMA } from '../lib/recipes.mjs'
import { BINARY, ROOT, doctor, render } from '../lib/render.mjs'
import { api } from '../lib/main.mjs'

const exec = promisify(execFile)
let workspace,
  counter = 0
before(async () => {
  workspace = await mkdtemp(join(tmpdir(), 'angel-vfx-test-'))
  assert.equal((await doctor()).ready, true, 'build the Bend engine before testing')
})
after(async () => {
  if (workspace) await rm(workspace, { recursive: true, force: true })
})
function child(program, args, input, { cwd = workspace, env = process.env } = {}) {
  return new Promise((resolve, reject) => {
    const p = spawn(program, args, { cwd, env, stdio: ['pipe', 'pipe', 'pipe'] }),
      stdout = [],
      stderr = []
    p.on('error', reject)
    p.stdout.on('data', (v) => stdout.push(v))
    p.stderr.on('data', (v) => stderr.push(v))
    p.on('close', (code) =>
      resolve({
        code,
        stdout: Buffer.concat(stdout).toString(),
        stderr: Buffer.concat(stderr).toString(),
      }),
    )
    p.stdin.end(input)
  })
}
async function native(recipe) {
  const r = normalize(recipe),
    base = join(workspace, `raw-${counter++}`),
    input = base + '.wire',
    output = base + '.rgba'
  await writeFile(input, wire(r))
  await exec(BINARY, [input, output, '+RTS', '--threads', '4', '-RTS'], { timeout: 15000 })
  const raw = await readFile(output)
  assert.equal(raw.length, r.w * r.h * r.frames * 4)
  return { r, raw }
}
function decodePng(data) {
  assert.deepEqual([...data.subarray(0, 8)], [137, 80, 78, 71, 13, 10, 26, 10])
  const w = data.readUInt32BE(16),
    h = data.readUInt32BE(20),
    pieces = []
  for (let at = 8; at < data.length; ) {
    const len = data.readUInt32BE(at)
    if (data.toString('ascii', at + 4, at + 8) === 'IDAT')
      pieces.push(data.subarray(at + 8, at + 8 + len))
    at += len + 12
  }
  const scan = inflateSync(Buffer.concat(pieces)),
    rgba = Buffer.alloc(w * h * 4)
  assert.equal(scan.length, (w * 4 + 1) * h)
  for (let y = 0; y < h; y++) {
    assert.equal(scan[y * (w * 4 + 1)], 0)
    scan.copy(rgba, y * w * 4, y * (w * 4 + 1) + 1, (y + 1) * (w * 4 + 1))
  }
  return { w, h, rgba }
}
test('every Bend layer paints binary-alpha pixels and a completed one-shot becomes transparent', async () => {
  for (const type of TYPES) {
    const { r, raw } = await native({
      w: 24,
      h: 24,
      frames: 6,
      seed: 7,
      layers: [{ type, size: 0.08, count: 8, text: '+100' }],
    })
    let painted = 0
    for (let i = 3; i < raw.length; i += 4) {
      assert.ok(raw[i] === 0 || raw[i] === 255, `${type}: partial alpha`)
      if (raw[i]) painted++
    }
    assert.ok(painted > 0, `${type} painted no pixels`)
    assert.ok(
      raw.subarray((r.frames - 1) * r.w * r.h * 4).every((v) => v === 0),
      `${type}: one-shot did not finish`,
    )
  }
})
test('same recipe has identical native bytes; seed changes alter an explosion', async () => {
  const r = { ...fromPrompt('fire explosion', 7), w: 32, h: 24, frames: 8 }
  const a = await native(r),
    b = await native(r),
    c = await native({ ...r, seed: 19 })
  assert.deepEqual(a.raw, b.raw)
  assert.notDeepEqual(a.raw, c.raw)
})
test('portal loops remain visible and move throughout the clip', async () => {
  const { r, raw } = await native({ ...fromPrompt('ice portal', 3), w: 32, h: 32, frames: 12 })
  const size = r.w * r.h * 4
  for (let i = 0; i < r.frames; i++)
    assert.ok(raw.subarray(i * size, (i + 1) * size).some((v) => v !== 0))
  assert.notDeepEqual(raw.subarray(0, size), raw.subarray(size, size * 2))
})
test('invalid vocabulary, recipes and resource limits fail before a render', () => {
  for (const input of [
    {},
    { prompt: 'ice', recipe: {} },
    { prompt: 'unicorn sandwiches' },
    { prompt: 44 },
    { recipe: { layers: [{ type: 'missing' }] } },
    { recipe: { layers: [{ type: 'ring', unknown: 1 }] } },
    { recipe: { w: NaN, layers: [{ type: 'ring' }] } },
    { recipe: { seed: true, layers: [{ type: 'ring' }] } },
    { recipe: { frames: 64, w: 256, h: 256, layers: [{ type: 'ring' }] } },
    { recipe: { layers: [{ type: 'ring', t0: 0.9, t1: 0.1 }] } },
    { recipe: { loop: true, layers: [{ type: 'flame', speed: 0.5 }] } },
    { recipe: { layers: [{ type: 'text', text: 'ABC' }] } },
    { prompt: 'ice', scale: 1.5 },
    { prompt: 'ice', extra: true },
    { recipe: { layers: [{ type: 'particles', count: 65 }] } },
  ])
    assert.throws(() => request(input), JSON.stringify(input))
})
test('sprite sheet matches native pixels, atlas matches frame cells and recipes round-trip', async () => {
  const recipe = { ...fromPrompt('holy sword slash', 7), w: 24, h: 24, frames: 8 }
  const result = await render({ recipe, out: 'exports', scale: 2, frames: true }, { workspace })
  const path = (ext) => result.files.find((p) => p.endsWith(ext)),
    sheet = decodePng(await readFile(path('.png'))),
    atlas = JSON.parse(await readFile(path('.json')))
  assert.equal(sheet.w, recipe.w * recipe.frames)
  assert.equal(sheet.h, recipe.h)
  const { raw } = await native(recipe),
    frameSize = recipe.w * recipe.h * 4
  for (let i = 0; i < recipe.frames; i++) {
    assert.deepEqual(atlas.frames[i].frame, { x: i * recipe.w, y: 0, w: recipe.w, h: recipe.h })
    for (let y = 0; y < recipe.h; y++)
      assert.deepEqual(
        sheet.rgba.subarray(
          (y * sheet.w + i * recipe.w) * 4,
          (y * sheet.w + (i + 1) * recipe.w) * 4,
        ),
        raw.subarray(i * frameSize + y * recipe.w * 4, i * frameSize + (y + 1) * recipe.w * 4),
      )
  }
  const saved = JSON.parse(await readFile(path('.recipe.json'))),
    again = await native(saved)
  assert.deepEqual(raw, again.raw)
  const gif = await readFile(path('.gif'))
  assert.equal(gif.toString('ascii', 0, 6), 'GIF89a')
  assert.equal(gif.readUInt16LE(6), 48)
  const tres = await readFile(path('.tres'), 'utf8')
  assert.ok(tres.includes('region = Rect2(168, 0, 24, 24)'))
  const checked = await exec('python3', [
    '-c',
    'import zipfile,sys; z=zipfile.ZipFile(sys.argv[1]); assert z.testzip() is None; assert len([n for n in z.namelist() if n.startswith("frames/")]) == 8',
    path('.zip'),
  ])
  assert.equal(checked.stderr, '')
  const original = await readFile(path('.png'))
  await assert.rejects(render({ recipe, out: 'exports' }, { workspace }), /already exists/)
  assert.deepEqual(await readFile(path('.png')), original)
})
test('palette locking, outlines and cleanup survive the native boundary', async () => {
  const { raw } = await native({
    w: 24,
    h: 24,
    frames: 4,
    palette: '1-bit',
    outline: 'outer',
    clean: true,
    dither: 0.3,
    layers: [{ type: 'burst' }],
  })
  const colors = new Set()
  for (let i = 0; i < raw.length; i += 4)
    if (raw[i + 3]) colors.add(raw.subarray(i, i + 3).toString('hex'))
  assert.ok(colors.size >= 2)
  assert.ok(colors.size <= 3)
})
test('output traversal and symlinked parents are refused', async () => {
  await assert.rejects(
    render({ prompt: 'ice', out: '../escape' }, { workspace }),
    /inside the workspace/,
  )
  const outside = await mkdtemp(join(tmpdir(), 'angel-vfx-outside-'))
  try {
    await symlink(outside, join(workspace, 'linked'))
    await assert.rejects(
      render({ prompt: 'ice', out: 'linked/output' }, { workspace }),
      /real workspace directories/,
    )
    assert.deepEqual(await readdir(outside), [])
  } finally {
    await rm(outside, { recursive: true, force: true })
  }
})
test('cartridge manifest tools match the shared API and their stdin entrypoints execute', async () => {
  const parsed = await exec('python3', [
    '-c',
    'import json,tomllib,sys; print(json.dumps(tomllib.load(open(sys.argv[1],"rb"))))',
    join(ROOT, 'cartridge.toml'),
  ])
  const manifest = JSON.parse(parsed.stdout)
  assert.equal(manifest.commands[0].name, 'vfx')
  assert.deepEqual(manifest.commands[0].skills, ['pixel-vfx'])
  const tool = manifest.tools.find((t) => t.name === 'vfx_render')
  assert.deepEqual(
    Object.keys(tool.params.properties).sort(),
    Object.keys(RENDER_SCHEMA.properties).sort(),
  )
  assert.deepEqual(tool.params.oneOf, RENDER_SCHEMA.oneOf)
  const called = await child(
    join(ROOT, tool.run),
    [],
    JSON.stringify({ prompt: 'ice hit', out: 'cartridge', scale: 1 }),
    { env: { ...process.env, ANGEL_WORKSPACE: workspace } },
  )
  assert.equal(called.code, 0, called.stderr)
  assert.equal(JSON.parse(called.stdout).engine, 'Bend 2 native')
  const cat = await child(join(ROOT, 'bin/vfx-catalog'), [], '{}')
  assert.equal(JSON.parse(cat.stdout).layers.length, 22)
})
test('stdio MCP initializes, lists tools, renders and reports errors without corrupting framing', async () => {
  const messages =
    [
      {
        id: 1,
        method: 'initialize',
        params: {
          protocolVersion: '2024-11-05',
          capabilities: {},
          clientInfo: { name: 'test', version: '1' },
        },
      },
      { method: 'notifications/initialized' },
      { id: 2, method: 'tools/list' },
      {
        id: 3,
        method: 'tools/call',
        params: { name: 'vfx_render', arguments: { prompt: 'fire hit', out: 'mcp', scale: 1 } },
      },
      {
        id: 4,
        method: 'tools/call',
        params: { name: 'vfx_render', arguments: { prompt: 'nonsense' } },
      },
      { id: 5, method: 'ping' },
      { id: 6, method: 'unknown' },
    ]
      .map((m) => JSON.stringify({ jsonrpc: '2.0', ...m }))
      .join('\n') + '\n'
  const ran = await child(join(ROOT, 'bin/vfx'), ['mcp'], messages)
  assert.equal(ran.code, 0, ran.stderr)
  assert.equal(ran.stderr, '')
  const replies = ran.stdout
    .trim()
    .split('\n')
    .map((l) => JSON.parse(l))
  assert.deepEqual(
    replies.map((m) => m.id),
    [1, 2, 3, 4, 5, 6],
  )
  assert.equal(replies[0].result.protocolVersion, '2024-11-05')
  assert.deepEqual(
    replies[1].result.tools.map((t) => t.name),
    ['vfx_catalog', 'vfx_render'],
  )
  assert.equal(JSON.parse(replies[2].result.content[0].text).engine, 'Bend 2 native')
  assert.equal(replies[3].result.isError, true)
  assert.deepEqual(replies[4].result, {})
  assert.equal(replies[5].error.code, -32601)
})
test('HTTP API renders, serves actual artifacts, handles races and stays usable after errors', async () => {
  const server = api({ workspace })
  await new Promise((resolve) => server.listen(0, '127.0.0.1', resolve))
  const base = `http://127.0.0.1:${server.address().port}`
  try {
    assert.equal((await (await fetch(base + '/health')).json()).ready, true)
    assert.equal((await (await fetch(base + '/v1/catalog')).json()).layers.length, 22)
    const post = (data) =>
      fetch(base + '/v1/render', {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify(data),
      })
    const response = await post({ prompt: 'ice portal', out: 'http', scale: 1, frames: true })
    assert.equal(response.status, 201)
    const result = await response.json()
    for (const download of result.downloads) {
      const asset = await fetch(base + download.url)
      assert.equal(asset.status, 200)
      assert.deepEqual(
        Buffer.from(await asset.arrayBuffer()),
        await readFile(join(result.out, download.file)),
      )
    }
    assert.equal((await post({ prompt: 'unrecognized' })).status, 400)
    assert.equal((await fetch(base + '/v1/render', { method: 'POST', body: '{}' })).status, 415)
    assert.equal(
      (await fetch(base + '/health', { headers: { Origin: 'https://example.com' } })).status,
      403,
    )
    assert.equal((await fetch(base + '/v1/files/unknown/missing.png')).status, 404)
    const races = await Promise.all([
      post({ prompt: 'ice hit', out: 'race', scale: 1 }),
      post({ prompt: 'ice hit', out: 'race', scale: 1 }),
    ])
    assert.deepEqual(races.map((r) => r.status).sort(), [201, 400])
    assert.equal((await fetch(base + '/health')).status, 200)
  } finally {
    await new Promise((resolve) => server.close(resolve))
  }
})
