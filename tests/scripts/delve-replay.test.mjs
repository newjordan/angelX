// The Delve replay gate (scripts/delve/replay.mjs): its reader, its BJ1 codec,
// its deltas, its comparison, and the whole thing end to end against a small
// stand-in worker, in plain JSON, in BJ1 and resident.
//   node --test tests/scripts/delve-replay.test.mjs
import { test } from 'node:test'
import assert from 'node:assert/strict'
import { spawnSync } from 'node:child_process'
import { mkdtempSync, writeFileSync, chmodSync, rmSync } from 'node:fs'
import { tmpdir } from 'node:os'
import { join } from 'node:path'
import { gzipSync } from 'node:zlib'
import { Fl, applyOps, compare, diff, fields, fromBj1, parse, show, toBj1 } from '../../scripts/delve/replay-lib.mjs'

const here = new URL('../../scripts/delve/', import.meta.url).pathname

test('the reader keeps integers and floats apart, and big integers whole', () => {
  const v = parse('{"a":1,"b":1.0,"c":-0.0,"d":1e3,"e":18446744073709551615,"f":[true,null,"x\\"y"]}')
  assert.equal(v.a, 1)
  assert.ok(v.b instanceof Fl && v.b.v === 1)
  assert.ok(Object.is(v.c.v, -0))
  assert.ok(v.d instanceof Fl && v.d.v === 1000)
  assert.equal(v.e, 18446744073709551615n)
  assert.deepEqual(v.f, [true, null, 'x"y'])
  assert.equal(show(v), '{"a":1,"b":1.0,"c":-0.0,"d":1000.0,"e":18446744073709551615,"f":[true,null,"x\\"y"]}')
})

test('BJ1 turns floats into f32 bits and wide integers into hex, and back', () => {
  const json = '{"a":0,"b":4294967295,"c":-4294967295,"d":4294967296,"e":18446744073709551615,"f":0.5,"g":-0.0,"h":"f3f800000 w0000000000000001"}'
  const bj = toBj1(json)
  assert.equal(
    bj,
    '{"a":0,"b":4294967295,"c":-4294967295,"d":w0000000100000000,"e":wffffffffffffffff,"f":f3f000000,"g":f80000000,"h":"f3f800000 w0000000000000001"}',
  )
  assert.equal(fromBj1(bj), '{"a":0,"b":4294967295,"c":-4294967295,"d":4294967296,"e":18446744073709551615,"f":0.5,"g":-0.0,"h":"f3f800000 w0000000000000001"}')
  assert.throws(() => toBj1('{"x":0.1}'), /not an f32/)
  assert.throws(() => toBj1('{"x":-4294967296}'), /outside BJ1/)
  // The literals false, true and null are not numbers.
  assert.equal(toBj1('[false,true,null]'), '[false,true,null]')
})

test('field spans come out of an object without parsing the rest', () => {
  const text = '{"kind":"apply","before":null,"event":{"a":[1,2,{"b":"}"}]},"after":{"run":1}}'
  assert.deepEqual(fields(text, ['kind', 'event', 'after']), { kind: '"apply"', event: '{"a":[1,2,{"b":"}"}]}', after: '{"run":1}' })
})

test('deltas turn one state into another and back', () => {
  const a = parse('{"x":{"y":[1,2,3,4],"z":1.5,"gone":true},"k":"v"}')
  const b = parse('{"x":{"y":[1,9,3],"z":2.5,"new":[0.0]},"k":"v","extra":null}')
  const ops = diff(a, b)
  assert.deepEqual(
    ops.map((o) => o[0]).sort(),
    ['d', 's', 's', 's', 's', 't'],
  )
  const c = applyOps(parse(show(a)), parse(show(ops)))
  assert.deepEqual(compare(b, c, 5), [])
  assert.throws(() => applyOps(parse('{"a":[1]}'), parse('[["s",["a",3],1]]')), /past the end/)
  assert.throws(() => applyOps(parse('{"a":1}'), parse('[["d",["b"]]]')), /missing key/)
})

test('comparison ignores key order and tells what the tick got wrong', () => {
  assert.deepEqual(compare(parse('{"a":1,"b":[1.5,2]}'), parse('{"b":[1.5,2],"a":1}')), [])
  const one = (e, g) => compare(parse(e), parse(g))[0]
  assert.match(one('{"a":0.0}', '{"a":-0.0}').why, /float/)
  assert.match(one('{"a":1}', '{"a":1.0}').why, /expected int, got float/)
  assert.match(one('{"a":null}', '{}').why, /missing/)
  assert.match(one('{"a":[1,2]}', '{"a":[1]}').why, /length/)
  assert.equal(one('{"p":{"q":[0,{"r":1}]}}', '{"p":{"q":[0,{"r":2}]}}').path, 'p.q[1].r')
  assert.match(one('{"a":9007199254740993}', '{"a":9007199254740992}').why, /int/)
  // f32 bits, not decimal spelling
  assert.deepEqual(compare(parse('{"a":0.30000001192092896}'), parse('{"a":0.30000001192092896}')), [])
  assert.equal(one('{"a":0.30000001192092896}', '{"a":0.3000000417232513}').expected.includes('0x3e99999a'), true)
})

// ---- end to end against a stand-in worker --------------------------------------

const STAND_IN = `#!/usr/bin/env node
// A tiny worker: the state is {"schema":6,"run":{"tick":N,"x":F,"big":4294967297,"list":[...]}}
// and every stepped apply adds one to the tick, 1/32 to x, and grows the list.
import { Buffer } from 'node:buffer'
let buf = Buffer.alloc(0)
const reply = (r, g, ok) => {
  const t = JSON.stringify({ api: 1, generation: g, request: r, result: { Ok: ok } })
  const b = Buffer.from(t); const h = Buffer.alloc(4); h.writeUInt32BE(b.length); process.stdout.write(Buffer.concat([h, b]))
}
const step = (s) => { const run = s.run; return { schema: 6, run: { ...run, tick: run.tick + 1, x: run.x + 0.03125, list: [...run.list, run.tick] } } }
process.stdin.on('data', (d) => {
  buf = Buffer.concat([buf, d])
  while (buf.length >= 4 && buf.length >= 4 + buf.readUInt32BE(0)) {
    const n = buf.readUInt32BE(0)
    const q = JSON.parse(buf.toString('utf8', 4, 4 + n)); buf = buf.subarray(4 + n)
    if (q.method === 'prepare') reply(q.request, q.generation, { id: 'delve', api: 1 })
    else if (q.method === 'apply') reply(q.request, q.generation, { state: q.body.event.step === false ? q.body.state : step(q.body.state), output: null })
    else if (q.method === 'restore') reply(q.request, q.generation, q.body)
    else reply(q.request, q.generation, null)
    if (q.method === 'dispose') process.exit(0)
  }
})
`

function corpus(dir, name, ticks) {
  const state = (t, x, list) => ({ schema: 6, run: { tick: t, x, big: 4294967297, list } })
  const lines = [JSON.stringify({ kind: 'restore', state: state(0, 0.3125, []) })]
  let x = 0.3125
  let n = 0
  let list = []
  for (let t = 0; t < ticks; t++) {
    const step = t % 5 !== 3
    if (step) { list = [...list, n]; n++; x += 0.03125 }
    const after = JSON.stringify(state(n, x, list))
    // a paused tick returns its input as it is
    lines.push(`{"kind":"apply","before":null,"event":{"inputs":{},"settings":{},"phrasebook":["",[]],"step":${step}},"after":${after}}`)
  }
  writeFileSync(join(dir, `${name}.jsonl.gz`), gzipSync(lines.join('\n') + '\n'))
}

test('end to end: a worker that follows the corpus passes, one that drifts is caught at its tick', () => {
  const dir = mkdtempSync(join(tmpdir(), 'delve-replay-'))
  try {
    const worker = join(dir, 'worker.mjs')
    writeFileSync(worker, STAND_IN)
    chmodSync(worker, 0o755)
    corpus(dir, 'steady', 12)
    // the stand-in grows its tick by one only when stepping; keep the file honest
    const run = (extra = [], env = {}) =>
      spawnSync(process.execPath, [join(here, 'replay.mjs'), worker, dir, ...extra], { encoding: 'utf8', env: { ...process.env, ...env } })
    const ok = run()
    assert.equal(ok.status, 0, ok.stdout + ok.stderr)
    assert.match(ok.stdout, /ok +steady +12\/12 ticks/)
    const from = run(['--from', '7'])
    assert.equal(from.status, 0, from.stdout)
    assert.match(from.stdout, /5\/12 ticks/)
    // the shim in front of the stand-in: BJ1, resident, and both
    for (const mode of [{ SHIM_BJ1: '1' }, { SHIM_RESIDENT: '1' }, { SHIM_BJ1: '1', SHIM_RESIDENT: '1' }]) {
      const shimmed = spawnSync(process.execPath, [join(here, 'replay.mjs'), join(here, 'replay-shim.mjs'), dir], {
        encoding: 'utf8',
        env: { ...process.env, ...mode, SHIM_WORKER: worker },
      })
      assert.equal(shimmed.status, 0, `${JSON.stringify(mode)}\n${shimmed.stdout}${shimmed.stderr}`)
    }
    // a worker that drifts one f32 step from tick 6
    const broken = spawnSync(process.execPath, [join(here, 'replay.mjs'), join(here, 'replay-shim.mjs'), dir], {
      encoding: 'utf8',
      env: { ...process.env, SHIM_BJ1: '1', SHIM_BREAK: '7', SHIM_WORKER: worker },
    })
    assert.equal(broken.status, 1, broken.stdout)
    assert.match(broken.stdout, /diverges at tick 6/)
    assert.match(broken.stdout, /run\.x/)
  } finally {
    rmSync(dir, { recursive: true, force: true })
  }
})
