// Execute the shipped classic browser script, without a browser, host or deps.
import assert from 'node:assert/strict'
import { readFileSync } from 'node:fs'
import test from 'node:test'
import vm from 'node:vm'

const source = readFileSync(new URL('../../crates/web/shell/sound.js', import.meta.url), 'utf8')
const flush = async () => {
  for (let i = 0; i < 20; i++) await Promise.resolve()
}
function deferred() {
  let resolve
  let reject
  const promise = new Promise((yes, no) => {
    resolve = yes
    reject = no
  })
  return { promise, resolve, reject }
}
function rig({ unlocked = true } = {}) {
  let now = 0
  const requests = []
  const sources = []
  const timers = new Map()
  const storage = new Map()
  let timerId = 0
  const ctx = {
    state: 'running',
    currentTime: 0,
    destination: {},
    resume() {
      this.state = 'running'
      return Promise.resolve()
    },
    suspend() {
      this.state = 'suspended'
      return Promise.resolve()
    },
    createGain() {
      return { gain: {}, connect() {} }
    },
    decodeAudioData(bytes) {
      return Promise.resolve({ name: bytes, duration: 300 })
    },
    createBufferSource() {
      const src = {
        connect() {},
        disconnect() {},
        start() {
          this.started = true
        },
        stop() {
          this.stopped = true
          this.onended?.()
        },
      }
      sources.push(src)
      return src
    },
  }
  const manifest = Object.fromEntries(
    ['hit', ...Array.from({ length: 75 }, (_, i) => `music-${i}`)].map((n) => [n, n]),
  )
  const sound = vm.runInNewContext(`${source}\nmakeSound(manifest)`, {
    manifest,
    window: {
      AudioContext: function () {
        return ctx
      },
    },
    AbortController,
    performance: { now: () => now },
    localStorage: {
      getItem: (k) => storage.get(k) ?? null,
      setItem: (k, v) => storage.set(k, v),
    },
    setTimeout(fn, ms) {
      timers.set(++timerId, { fn, ms })
      return timerId
    },
    clearTimeout(id) {
      timers.delete(id)
    },
    fetch(url, options) {
      const d = deferred()
      requests.push({ url, options, ...d })
      return d.promise
    },
  })
  if (unlocked) sound.unlock()
  return {
    sound,
    ctx,
    sources,
    requests,
    timers,
    advance(ms) {
      now += ms
    },
    async deliver(index = requests.length - 1) {
      requests[index].resolve({ ok: true, arrayBuffer: async () => requests[index].url })
      await flush()
    },
    async fail(index = requests.length - 1) {
      requests[index].reject(new Error('packet loss'))
      await flush()
    },
    async retry() {
      const pending = [...timers.values()]
      timers.clear()
      for (const { fn } of pending) fn()
      await flush()
    },
    music() {
      return sources.filter((s) => s.loop && s.started && !s.stopped)
    },
  }
}

test('music prepares its replacement before stopping the current source', async () => {
  const r = rig()
  r.sound.music('music-0')
  await r.deliver()
  const old = r.music()[0]
  r.sound.music('music-1')
  assert.equal(old.stopped, undefined, 'keep the old music through network/decode delay')
  await r.deliver()
  assert.equal(old.stopped, true)
  assert.equal(r.music()[0].buffer.name, 'music-1')
})

test('a playlist pass does not retain decoded music; SFX still reuse their cache', async () => {
  const r = rig()
  for (let pass = 0; pass < 2; pass++) {
    for (let i = 0; i < 75; i++) {
      const count = r.requests.length
      r.sound.music(`music-${i}`)
      assert.equal(r.requests.length, count + 1, 'stopped music must be fetched/decoded again')
      await r.deliver()
      assert.equal(r.music().length, 1)
      for (const s of r.sources.filter((s) => s.stopped)) {
        assert.equal(s.buffer, null, 'release stopped decoded buffers')
      }
    }
    r.sound.music('')
    assert.equal(r.music().length, 0)
  }
  r.sound.sfx('hit')
  await r.deliver()
  const count = r.requests.length
  r.sound.sfx('hit')
  await flush()
  assert.equal(r.requests.length, count)
})

test('fetch and decode failures retry only a bounded number of times while old music continues', async () => {
  const r = rig()
  r.sound.music('music-0')
  await r.deliver()
  r.sound.music('music-1')
  await r.fail()
  assert.equal(r.music()[0].buffer.name, 'music-0')
  assert.equal(r.timers.size, 1)
  await r.retry()
  await r.fail()
  assert.equal(r.timers.size, 0, 'no endless retry loop')
  r.sound.music('music-1')
  await r.deliver()
  assert.equal(r.music()[0].buffer.name, 'music-1', 'a later request can recover the same name')
})

test('superseded fetch/decode/retry and explicit stop cannot start stale music', async () => {
  const r = rig()
  const decode = deferred()
  r.ctx.decodeAudioData = () => decode.promise
  r.sound.music('music-0')
  await r.deliver()
  r.sound.music('music-1')
  r.sound.music('music-0') // ABA: same name, different request
  assert.equal(r.requests.length, 1, 'serialize uncancellable decodes to bound pending buffers')
  decode.resolve({ name: 'stale' })
  await flush()
  assert.equal(r.music().length, 0)
  r.ctx.decodeAudioData = async (name) => ({ name })
  await r.deliver()
  assert.equal(r.music()[0].buffer.name, 'music-0')
  r.sound.music('music-2')
  await r.fail()
  assert.equal(r.timers.size, 1)
  r.sound.music('')
  await r.retry()
  assert.equal(r.music().length, 0)
  assert.equal(r.timers.size, 0)
  r.sound.music('music-3')
  r.sound.music('')
  await r.deliver()
  assert.equal(r.music().length, 0)
})

test('SFX use wall time and reject pending sounds across suspension, even with a frozen audio clock', async () => {
  const r = rig()
  r.sound.sfx('hit')
  r.advance(1000) // AudioContext.currentTime can freeze during a stall/suspension.
  await r.deliver()
  assert.equal(r.sound.played, 0)
  r.sound.sfx('hit')
  r.sound.suspend()
  r.sound.resume()
  await flush()
  assert.equal(r.sound.played, 0, 'pre-suspension requests never replay on resume')
  r.sound.sfx('hit')
  await flush()
  assert.equal(r.sound.played, 1, 'fresh post-resume effects work')
})

test('failed effects can retry, while locked/suspended effects do not queue', async () => {
  const r = rig()
  r.sound.sfx('hit')
  await r.fail()
  r.sound.sfx('hit')
  assert.equal(r.requests.length, 2)
  await r.deliver()
  assert.equal(r.sound.played, 1)
  r.sound.suspend()
  r.sound.sfx('hit')
  r.sound.resume()
  await flush()
  assert.equal(r.sound.played, 1)
})

test('music requested before unlock starts once, same-name calls do not refetch, and controls persist', async () => {
  const r = rig({ unlocked: false })
  r.sound.sfx('hit')
  r.sound.music('music-0')
  assert.equal(r.requests.length, 0)
  r.sound.unlock()
  await r.deliver()
  r.sound.music('music-0')
  r.sound.unlock()
  r.sound.resume()
  await flush()
  assert.equal(r.requests.length, 1)
  assert.equal(r.music().length, 1)
  r.sound.setVolume(0.4)
  assert.equal(r.sound.volume, 0.4)
  r.sound.toggleMute()
  assert.equal(r.sound.muted, true)
  r.sound.setVolume(0.6)
  assert.equal(r.sound.muted, false)
})

test('HTTP and decode errors are retried; changing the request cancels retry', async () => {
  const r = rig()
  r.sound.music('music-0')
  r.requests[0].resolve({ ok: false, status: 503 })
  await flush()
  assert.equal(r.timers.size, 1)
  await r.retry()
  r.ctx.decodeAudioData = async () => {
    throw new Error('bad MP3')
  }
  await r.deliver()
  assert.equal(r.timers.size, 0)
  r.ctx.decodeAudioData = async (name) => ({ name })
  r.sound.music('music-0')
  await r.fail()
  r.sound.music('music-1')
  assert.equal(r.timers.size, 1, 'only the new fetch timeout remains')
  await r.deliver()
  assert.equal(r.music()[0].buffer.name, 'music-1')
})

test('an abandoned fetch is aborted, and a late response cannot replace the requested track', async () => {
  const r = rig()
  r.sound.music('music-0')
  r.sound.music('music-1')
  assert.equal(r.requests[0].options.signal.aborted, true)
  await r.deliver(0) // Simulate a late response despite cancellation.
  assert.equal(r.music().length, 0)
  await r.deliver(1)
  assert.equal(r.music()[0].buffer.name, 'music-1')
})

test('suspension retires in-progress effects rather than replaying their tails on resume', async () => {
  const r = rig()
  r.sound.sfx('hit')
  await r.deliver()
  const effect = r.sources[0]
  r.sound.suspend()
  r.sound.resume()
  await flush()
  assert.equal(effect.stopped, true)
  assert.equal(effect.buffer, null)
})

test('source start failure preserves old music and remains retryable without an unhandled rejection', async () => {
  const r = rig()
  r.sound.music('music-0')
  await r.deliver()
  const create = r.ctx.createBufferSource
  r.ctx.createBufferSource = () => {
    const src = create()
    src.start = () => {
      throw new Error('interrupted audio device')
    }
    return src
  }
  r.sound.music('music-1')
  await r.deliver()
  assert.equal(r.music()[0].buffer.name, 'music-0')
  assert.equal(r.timers.size, 1)
  r.ctx.createBufferSource = create
  await r.retry()
  await r.deliver()
  assert.equal(r.music()[0].buffer.name, 'music-1')
})

test('music fetch has a deadline; a superseded retry cannot restart', async () => {
  const r = rig()
  r.sound.music('music-0')
  const [{ fn, ms }] = r.timers.values()
  assert.equal(ms, 15000)
  fn()
  assert.equal(r.requests[0].options.signal.aborted, true)
  await r.fail()
  r.sound.music('')
  await r.retry()
  assert.equal(r.requests.length, 1)
})

test('visibility changes immediately idle the runtime before suspending audio', () => {
  const boot = readFileSync(new URL('../../crates/web/shell/boot.js', import.meta.url), 'utf8')
  const begin = boot.indexOf("  document.addEventListener('visibilitychange', () => {")
  const end = boot.indexOf('\n  let lock = null;', begin)
  assert.ok(begin >= 0 && end > begin)
  let visibility
  const calls = []
  const document = {
    hidden: true,
    addEventListener(name, callback) {
      assert.equal(name, 'visibilitychange')
      visibility = callback
    },
  }
  vm.runInNewContext(boot.slice(begin, end), {
    document,
    running: true,
    crashed: false,
    raf: 1,
    lastT: 1,
    hiddenTimer: 0,
    lastHeard: 0,
    SILENCE_MS: 10000,
    frame() {},
    input: {
      releaseAll() {
        calls.push('release')
      },
    },
    flush() {
      calls.push('flush')
    },
    cancelAnimationFrame() {},
    requestAnimationFrame() {
      return 2
    },
    x: {
      angel_idle() {
        calls.push('idle')
      },
    },
    sound: {
      suspend() {
        calls.push('suspend')
      },
      resume() {
        calls.push('resume')
      },
    },
    setInterval() {
      calls.push('timer')
      return 1
    },
    clearInterval() {},
    performance: { now: () => 100 },
    crash(e) {
      throw e
    },
    wakeLock() {},
  })
  visibility()
  assert.deepEqual(calls, ['release', 'flush', 'idle', 'flush', 'timer', 'suspend'])
  document.hidden = false
  visibility()
  assert.equal(calls.at(-1), 'resume')
})

test('effects keep their six-voice cap and ended sources release buffers', async () => {
  const r = rig()
  r.sound.sfx('hit')
  await r.deliver()
  for (let i = 0; i < 9; i++) {
    r.sound.sfx('hit')
    await flush()
  }
  assert.equal(r.sound.played, 10)
  assert.equal(r.sources.filter((s) => s.started && !s.stopped).length, 6)
  assert.ok(r.sources.filter((s) => s.stopped).every((s) => s.buffer === null))
})

test('a music switch waits through decode, not just fetch', async () => {
  const r = rig()
  r.sound.music('music-0')
  await r.deliver()
  const old = r.music()[0]
  const decode = deferred()
  r.ctx.decodeAudioData = () => decode.promise
  r.sound.music('music-1')
  await r.deliver()
  assert.equal(old.stopped, undefined)
  decode.resolve({ name: 'music-1' })
  await flush()
  assert.equal(old.stopped, true)
  assert.equal(r.music()[0].buffer.name, 'music-1')
})
