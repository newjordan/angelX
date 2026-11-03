import test from 'node:test'
import assert from 'node:assert/strict'
import { readFileSync } from 'node:fs'
import vm from 'node:vm'

function fixture() {
  const sources = [], requests = [], channels = []
  class AudioContext {
    state = 'running'
    currentTime = 0
    destination = {}
    createGain() {
      const node = { gain: { value: 1 }, connect() {}, disconnect() { this.retired = true } }
      channels.push(node)
      return node
    }
    createBufferSource() {
      const source = { connect(node) { this.channel = node }, disconnect() {}, start() {}, stop() { this.stopped = true; this.onended?.() } }
      sources.push(source)
      return source
    }
    decodeAudioData(bytes) { return Promise.resolve({ bytes }) }
    resume() { return Promise.resolve() }
  }
  const context = vm.createContext({
    window: { AudioContext },
    localStorage: { getItem() { return null }, setItem() {} },
    fetch: async url => { requests.push(url); return { ok: true, arrayBuffer: async () => url } }
  })
  vm.runInContext(readFileSync(new URL('../../crates/web/shell/sound.js', import.meta.url), 'utf8'), context)
  const sound = vm.runInContext('makeSound({})', context)
  sound.unlock()
  return { sound, sources, requests, channels }
}
const flush = () => new Promise(resolve => setImmediate(resolve))

test('new content changes future voices while the old voice keeps its original buffer and gain', async () => {
  const f = fixture(), old = `live-${'a'.repeat(48)}.wav`, next = `live-${'b'.repeat(48)}.wav`
  f.sound.sfx(`${old}@25`)
  await flush()
  f.sound.sfx(`${next}@70`)
  await flush()
  assert.deepEqual(f.requests, [`/a/${old}`, `/a/${next}`])
  assert.equal(f.sources[0].buffer.bytes, `/a/${old}`)
  assert.equal(f.sources[1].buffer.bytes, `/a/${next}`)
  assert.equal(f.sources[0].channel.gain.value, 0.25)
  assert.equal(f.sources[1].channel.gain.value, 0.7)
  assert.equal(f.sources[0].stopped, undefined)
  f.sources[0].onended()
  assert.equal(f.sources[0].channel.retired, true)
  f.sound.sfx(`${next}@40`)
  await flush()
  assert.equal(f.requests.length, 2, 'volume changes refetched identical content')
})

test('dynamic buffers retire from the bounded cache and unknown names never become arbitrary URLs', async () => {
  const f = fixture()
  f.sound.sfx('https://example.test/clip.wav@50')
  f.sound.sfx(`live-${'a'.repeat(48)}.wav@101`)
  await flush()
  assert.equal(f.requests.length, 0)
  for (let i = 0; i < 33; i++) {
    f.sound.sfx(`live-${i.toString(16).padStart(48, '0')}.wav@50`)
    await flush()
  }
  f.sound.sfx(`live-${'0'.repeat(48)}.wav@50`)
  await flush()
  assert.equal(f.requests.length, 34, 'evicted content remained in the cache')
  assert.ok(f.sources.slice(0, 27).every(source => source.stopped))
})
