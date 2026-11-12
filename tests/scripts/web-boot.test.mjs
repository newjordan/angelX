// Execute the shipped browser shell (boot.js) in a bare VM: a fake page, a fake
// module and a hand-turned clock, with no browser, host or deps.
import assert from 'node:assert/strict'
import { readFileSync } from 'node:fs'
import test from 'node:test'
import vm from 'node:vm'

const source = readFileSync(new URL('../../crates/web/shell/boot.js', import.meta.url), 'utf8')
const flush = async () => {
  for (let i = 0; i < 20; i++) await Promise.resolve()
}

function element() {
  const listeners = new Map()
  return {
    textContent: '',
    hidden: false,
    disabled: false,
    value: '',
    dataset: {},
    style: { setProperty() {} },
    classList: { add() {}, remove() {}, toggle() {}, contains: () => false },
    addEventListener(type, fn) {
      listeners.set(type, [...(listeners.get(type) ?? []), fn])
    },
    dispatch(type, event = {}) {
      for (const fn of listeners.get(type) ?? []) fn({ preventDefault() {}, ...event })
    },
    focus() {},
    blur() {},
    append() {},
    replaceChildren() {},
    replaceWith() {},
    cloneNode: () => element(),
    contains: () => false,
    getBoundingClientRect: () => ({ left: 0, top: 0, width: 0, height: 0 }),
  }
}

// A page with a game module behind it. `wasm` is what the module's fetch does.
function page({ wasm = 'loads' } = {}) {
  let now = 0
  let nextTimer = 0
  const timers = new Map()
  const elements = new Map()
  const sockets = []
  const storage = new Map()
  const documentEvents = element()
  const game = {
    idleCalls: 0,
    trap: false,
    exports: null,
  }
  const memory = { buffer: new ArrayBuffer(1 << 16) }
  let heapTop = 8
  game.exports = {
    memory,
    angel_alloc: (n) => {
      const at = heapTop
      heapTop += n
      return at
    },
    angel_free() {},
    angel_text_ptr: () => 0,
    angel_connected() {},
    angel_out: () => 0,
    angel_recv() {
      if (game.trap) throw new Error('unreachable executed')
      return 1
    },
    angel_status: () => 0,
    angel_idle() {
      game.idleCalls++
    },
    angel_rtt: () => 0,
  }
  const document = {
    hidden: false,
    body: Object.assign(element(), { dataset: { wasm: '/game.wasm' } }),
    documentElement: Object.assign(element(), { requestFullscreen() {} }),
    getElementById(id) {
      if (!elements.has(id)) elements.set(id, element())
      return elements.get(id)
    },
    createElement: element,
    addEventListener: documentEvents.addEventListener,
    fonts: { add() {} },
  }
  const sandbox = {
    document,
    location: { search: '', hash: '#game-key', protocol: 'http:', host: 'localhost' },
    localStorage: {
      getItem: (k) => storage.get(k) ?? null,
      setItem: (k, v) => storage.set(k, String(v)),
      removeItem: (k) => storage.delete(k),
    },
    navigator: {},
    performance: { now: () => now },
    console: { error() {}, warn() {}, info() {}, log() {} },
    TextDecoder,
    TextEncoder,
    URLSearchParams,
    WebAssembly:
      wasm === 'loads'
        ? { instantiate: async () => ({ instance: { exports: game.exports } }) }
        : {},
    fetch: async () => {
      if (wasm !== 'loads') throw new TypeError('Failed to fetch')
      return { arrayBuffer: async () => new ArrayBuffer(0) }
    },
    WebSocket: class {
      constructor() {
        this.readyState = 0
        sockets.push(this)
      }
      send() {}
      close() {
        this.readyState = 3
      }
    },
    requestAnimationFrame: () => 1,
    cancelAnimationFrame() {},
    setInterval(fn, ms) {
      timers.set(++nextTimer, { fn, ms, due: now + ms, every: true })
      return nextTimer
    },
    setTimeout(fn, ms) {
      timers.set(++nextTimer, { fn, ms, due: now + ms, every: false })
      return nextTimer
    },
    clearInterval: (id) => timers.delete(id),
    clearTimeout: (id) => timers.delete(id),
    makePresenter: () => ({ kind: 'canvas2d', clear() {}, draw() {} }),
    makeInput: () => ({ releaseAll() {}, poll() {}, setControls() {} }),
    makeSound: () => ({
      volume: 1,
      muted: false,
      unlock() {},
      setVolume() {},
      toggleMute() {},
      suspend() {},
      resume() {},
      sfx() {},
      music() {},
    }),
    el: element,
    keyHint: element,
    drawSheet() {},
  }
  sandbox.window = sandbox
  sandbox.addEventListener = element().addEventListener
  vm.runInNewContext(source, sandbox)
  const turn = (ms) => {
    const until = now + ms
    for (;;) {
      const due = [...timers.entries()]
        .filter(([, t]) => t.due <= until)
        .sort(([, a], [, b]) => a.due - b.due)[0]
      if (!due) break
      const [id, timer] = due
      now = timer.due
      if (timer.every) timer.due += timer.ms
      else timers.delete(id)
      timer.fn()
    }
    now = until
  }
  return {
    angel: () => sandbox.__angel,
    statusLine: () => elements.get('status').textContent,
    game,
    sockets,
    turn,
    // Take a seat's worth of the way: the page's form submitted, the socket open.
    async play() {
      await flush()
      document.getElementById('name').value = 'tester'
      document.getElementById('join').dispatch('submit')
      sockets[0].readyState = 1
      sockets[0].onopen()
    },
    hide(hidden) {
      document.hidden = hidden
      documentEvents.dispatch('visibilitychange')
    },
    // A message the module traps on.
    trap() {
      game.trap = true
      sockets[0].onmessage({ data: new ArrayBuffer(8) })
    },
  }
}

test('a hidden tab keeps the module idle once a second, and a visible one lets it go', async () => {
  const p = page()
  await p.play()
  p.hide(true)
  assert.equal(p.game.idleCalls, 1)
  p.turn(3000)
  assert.equal(p.game.idleCalls, 4)
  p.hide(false)
  p.turn(3000)
  assert.equal(p.game.idleCalls, 4)
})

test('a game that traps in a hidden tab stops the idle timer and does not wake it again', async () => {
  const p = page()
  await p.play()
  p.hide(true)
  p.trap()
  assert.equal(p.angel().status, 'crashed')
  const after = p.game.idleCalls
  p.turn(5000)
  p.hide(false)
  p.turn(5000)
  assert.equal(p.game.idleCalls, after, 'the trapped module was woken again')
})

test('a module that fails to load leaves the loading status, and says so', async () => {
  const p = page({ wasm: 'fails' })
  assert.equal(p.angel().status, 'loading')
  await flush()
  assert.match(p.statusLine(), /^the game did not load: /)
  assert.notEqual(p.angel().status, 'loading', 'a probe still reads loading')
  assert.equal(p.angel().status, 'failed')
})

test('a module that loads reads ready', async () => {
  const p = page()
  await flush()
  assert.equal(p.angel().status, 'ready')
})
