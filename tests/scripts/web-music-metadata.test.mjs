// Shipped MP3s carry frame counts (Xing/Info): no ffprobe dependency needed.
import assert from 'node:assert/strict'
import { readFileSync } from 'node:fs'
import test from 'node:test'

const root = new URL('../../', import.meta.url)
const source = readFileSync(new URL('crates/delve/src/rules/music.rs', root), 'utf8')
function duration(bytes) {
  let start = 0
  if (bytes.toString('ascii', 0, 3) === 'ID3') {
    const size = bytes.subarray(6, 10).reduce((n, b) => (n << 7) | b, 0)
    start = 10 + size + (bytes[5] & 0x10 ? 10 : 0)
  }
  // Locate the header using MPEG version/channel layout, not an accidental
  // occurrence of "Info" in an ID3 comment or compressed payload.
  const h = bytes.readUInt32BE(start)
  assert.equal(h >>> 21, 0x7ff, 'MPEG frame sync after ID3')
  assert.equal((h >>> 19) & 3, 3, 'MPEG-1')
  assert.equal((h >>> 17) & 3, 1, 'Layer III')
  const rate = [44100, 48000, 32000][(h >>> 10) & 3]
  assert.ok(rate)
  const mono = ((h >>> 6) & 3) === 3
  const crc = ((h >>> 16) & 1) === 0 ? 2 : 0
  const at = start + 4 + crc + (mono ? 17 : 32)
  assert.match(bytes.toString('ascii', at, at + 4), /^(Xing|Info)$/)
  assert.equal(bytes.readUInt32BE(at + 4) & 1, 1, 'header has a frame count')
  const flags = bytes.readUInt32BE(at + 4)
  let encoder = at + 8
  if (flags & 1) encoder += 4
  if (flags & 2) encoder += 4
  if (flags & 4) encoder += 100
  if (flags & 8) encoder += 4
  assert.match(bytes.toString('ascii', encoder, encoder + 4), /^(Lavc|LAME)$/)
  // LAME-compatible gapless metadata excludes encoder delay and end padding,
  // matching ffprobe's reported duration for the shipped final encodes.
  const gapless = bytes.readUIntBE(encoder + 21, 3)
  const trim = (gapless >>> 12) + (gapless & 0xfff)
  return (bytes.readUInt32BE(at + 8) * 1152 - trim) / rate
}

const lists = [...source.matchAll(/pub const (\w+): &\[Track\] = &\[(.*?)\];/gs)]
assert.equal(lists.length, 4)
for (const [, name, body] of lists) {
  test(`${name} durations match shipped media (whole seconds, less than one second error)`, () => {
    const tracks = [...body.matchAll(/Track \{ name: "([^"]+)", secs: (\d+) \}/g)]
    assert.ok(tracks.length)
    const errors = []
    for (const [, track, declared] of tracks) {
      const bytes = readFileSync(new URL(`cockpit/assets/dungeon/music/${track}.mp3`, root))
      const seconds = duration(bytes)
      if (Math.abs(Number(declared) - seconds) >= 1) {
        errors.push(`${track}: declared ${declared}s, media ${seconds.toFixed(6)}s`)
      }
    }
    assert.deepEqual(errors, [], 'server slots must follow the final shipped MP3s')
  })
}
