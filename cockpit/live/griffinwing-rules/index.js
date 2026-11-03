// Thin adapter for Griffinwing's rules. The session arrives as the JSON the Delve already saves and
// sends, is laid out as the flat word frame the Bend rules read (`griffinwing-rules.bend`, section
// "layout"), and the next session is read back out of the answer the same way. Every rule, number
// and branch is in the Bend; this file only names fields and orders words. Rust owns the sound
// names: sounds come back as indices into its table.
const bend = require('./griffinwing-rules.compiled.js')

const E = {
  Kind: ['Wisp', 'Kite', 'Turret', 'Bomber', 'Lamp'],
  St: ['Fly', 'Tell', 'Rest', 'Hunt', 'Latched', 'Flee', 'Hide', 'Dive', 'Shut', 'Open', 'Broken'],
  Arm: ['Lance', 'Star', 'Ally', 'Spark', 'Gloam', 'Turned'],
  Thing: ['Gate', 'Ring', 'Arch', 'Ridge', 'Shutter'],
  Line: ['Hello', 'Hold', 'Ring', 'Arch', 'Latched', 'Thanks', 'Lost', 'Turret', 'Roll', 'Convoy', 'Bombers', 'Shutter',
    'Wyrm', 'Open', 'Reflect', 'Coils', 'Roar', 'Clear', 'Home', 'Chain', 'Hurt', 'Join', 'Low', 'Roost', 'Ruins'],
  Burst: ['Pop', 'Hit', 'Turn', 'Lock', 'Gate', 'Scrape', 'Fall', 'Hurt', 'Lamp', 'Starfall'],
  Wyrm: ['Rise', 'Idle', 'Tell', 'Coil', 'Roar', 'Hurt', 'Dying'],
  Mode: ['Play', 'Retry', 'Victory', 'Tally'],
  Medal: ['Bronze', 'Silver', 'Gold'],
}

// A record is its fields in word order: `name`, `name:b` for a flag, `name:Enum` for a variant.
const rec = names => names.map(n => { const [k, t] = n.split(':'); return [k, t === 'b' ? 'b' : t ? E[t] : null] })
const CRAFT_A = rec(['x', 'z', 'vx', 'vz', 'bank', 'lift', 'back', 'ds', 'rx', 'rz', 'shield', 'hurt', 'down', 'roll',
  'roll_len', 'dir', 'rest', 'held', 'charge', 'cool', 'nlocks'])
const CRAFT_B = rec(['dwell', 'meter', 'spent:b', 'boost:b', 'brake:b', 'bombs', 'keys', 'tap_dir', 'tap_ticks', 'last_x',
  'chain', 'chain_ticks', 'pts', 'hits', 'saved', 'gates'])
const FOE = rec(['id', 'kind:Kind', 's', 'x', 'z', 'vs', 'vx', 'vz', 'hp', 'full', 'seats', 'st:St', 'ticks', 'fx', 'fz',
  'on', 'flash', 'mode', 'phase'])
const BOLT = rec(['shot:Arm', 'owner', 's', 'x', 'z', 'vs', 'vx', 'vz', 'dmg', 'life', 'aim', 'src', 'delay', 'full:b'])
const PROP = rec(['thing:Thing', 's', 'x', 'z', 'w', 'h', 'len', 'taken', 'phase'])
const CALL = rec(['who', 'line:Line', 'ticks'])
const FLARE = rec(['what:Burst', 's', 'x', 'z', 'ticks', 'n'])
const SEAT = rec(['present:b', 'joined'])
const WING = rec(['id', 'lost:b', 'fire'])
const BEAST_A = rec(['phase', 'st:Wyrm', 'ticks', 'x', 'z'])
const BEAST_B = rec(['gap', 'side', 'col', 'atk', 'seq'])
const TALLY = rec(['pts', 'base', 'offered', 'kills', 'saved', 'lost', 'gates'])

// where the tables stand, and how many rows each holds (the Bend's `layout` section)
const G = 128, SEAT_W = 51, WING_W = 52, FOE_W = 19, BOLT_W = 14, PROP_W = 9, CALL_W = 3, FX_W = 6, BOSS_W = 30
const SEATS = 128, WINGS = 434, FOES = 642, BOLTS = 1402, PROPS = 3642, CALLS = 4002, FXS = 4014, BOSS = 4302, END = 4332
const SND = 45, LEAVE = 57, KEYS = 71, BOT_OUT = 4336 + 1400
const CAP = { foes: 40, props: 40, bolts: 160, wing: 4, calls: 4, fx: 48 }
const NAMES = ['elapsed', 'mode', 'mode_t', 'stage', 't', 'cp', 'next', 'ids', 'retries', 'medal', 'paid', 'settled',
  'done_some', 'done', 'home', 'first']

const put = (w, base, spec, o) => spec.forEach(([k, t], i) => {
  const v = o[k]
  const word = t === 'b' ? (v ? 1 : 0) : t ? t.indexOf(v) : v
  if (!Number.isInteger(word) || word < -(2 ** 31) || word >= 2 ** 31) throw Error(`griffinwing: bad ${k}`)
  w[base + i] = word
})
const get = (w, base, spec) => Object.fromEntries(spec.map(([k, t], i) => {
  const v = w[base + i]
  return [k, t === 'b' ? v !== 0 : t ? t[v] : v]
}))
const rows = (w, base, width, spec, list, cap, what) => {
  if (list.length > cap) throw Error(`griffinwing: too many ${what}`)
  list.forEach((o, i) => put(w, base + i * width, spec, o))
}
const out = (w, base, width, spec, n) => Array.from({ length: n }, (_, i) => get(w, base + i * width, spec))

const putCraft = (w, base, c) => {
  put(w, base, CRAFT_A, { ...c, nlocks: c.locks.length })
  c.locks.slice(0, 6).forEach((l, k) => { w[base + 21 + 2 * k] = l.foe; w[base + 22 + 2 * k] = l.n })
  put(w, base + 33, CRAFT_B, c)
}
const getCraft = (w, base) => {
  const { nlocks, ...a } = get(w, base, CRAFT_A)
  const locks = Array.from({ length: nlocks }, (_, k) => ({ foe: w[base + 21 + 2 * k], n: w[base + 22 + 2 * k] }))
  return { ...a, locks, ...get(w, base + 33, CRAFT_B) }
}

// the session, the keys, who is at the table and the five numbers, as one frame of words
function frame(input) {
  const s = input.session, w = new Int32Array(16384)
  NAMES.forEach((k, i) => {
    const v = s[k]
    w[i] = k === 'mode' ? E.Mode.indexOf(v) : k === 'medal' ? (v ? E.Medal.indexOf(v) + 1 : 0)
      : k === 'done_some' ? (s.done == null ? 0 : 1) : k === 'done' ? (s.done ?? 0)
      : k === 'settled' || k === 'first' ? (v ? 1 : 0) : v
  })
  TALLY.forEach(([k], i) => { w[16 + i] = s.bank[k]; w[23 + i] = s.beat[k] })
  w[30] = s.foes.length; w[31] = s.props.length; w[32] = s.bolts.length; w[33] = s.wing.length
  w[34] = s.calls.length; w[35] = s.fx.length; w[36] = s.boss ? 1 : 0
  const n = input.nums
  ;[n.charge, n.roll, n.window, n.spur, n.tell].forEach((v, i) => { w[39 + i] = v })
  w[44] = input.here.reduce((m, id) => (id >= 1 && id <= 6 ? m | (1 << (id - 1)) : m), 0)
  for (const [id, i] of Object.entries(input.keys || {})) {
    if (id >= 1 && id <= 6) {
      ;[i.move_x, i.move_y, i.aim_x, i.aim_y, i.fire, i.dash, i.bomb, i.interact, i.swing].forEach((v, j) => {
        w[KEYS + (id - 1) * 9 + j] = Number(v)
      })
    }
  }
  for (const [id, seat] of Object.entries(s.seats)) {
    if (id < 1 || id > 6) throw Error('griffinwing: a seat out of range')
    const b = SEATS + (id - 1) * SEAT_W
    put(w, b, SEAT, { present: true, joined: seat.joined })
    putCraft(w, b + 2, seat.craft)
  }
  s.wing.forEach((m, k) => { put(w, WINGS + k * WING_W, WING, m); putCraft(w, WINGS + k * WING_W + 3, m.craft) })
  rows(w, FOES, FOE_W, FOE, s.foes, CAP.foes, 'foes')
  rows(w, PROPS, PROP_W, PROP, s.props, CAP.props, 'props')
  rows(w, BOLTS, BOLT_W, BOLT, s.bolts, CAP.bolts, 'bolts')
  rows(w, CALLS, CALL_W, CALL, s.calls, CAP.calls, 'calls')
  rows(w, FXS, FX_W, FLARE, s.fx, CAP.fx, 'fx')
  if (s.boss) {
    const b = s.boss
    put(w, BOSS, BEAST_A, b)
    b.lamps.forEach((l, k) => { w[BOSS + 5 + k] = l })
    if (b.marks.length > 8) throw Error('griffinwing: too many marks')
    w[BOSS + 8] = b.marks.length
    b.marks.forEach(([x, z], k) => { w[BOSS + 9 + 2 * k] = x; w[BOSS + 10 + 2 * k] = z })
    put(w, BOSS + 25, BEAST_B, b)
  }
  return w
}

// a frame as a persistent quad trie of words, the shape the Bend reads (all-zero stretches are shared)
const Z = { $: 'Z' }
function lay(w, lo = 0, size = 16384) {
  if (lo >= END) return Z
  if (size === 1) return w[lo] === 0 ? Z : { $: 'Leaf', v: w[lo] >>> 0 }
  const q = size >> 2
  const a = lay(w, lo, q), b = lay(w, lo + q, q), c = lay(w, lo + 2 * q, q), d = lay(w, lo + 3 * q, q)
  return a === Z && b === Z && c === Z && d === Z ? Z : { $: 'Node', a, b, c, d }
}
function flat(m, w = new Int32Array(16384), lo = 0, size = 16384) {
  if (m.$ === 'Leaf') w[lo] = m.v | 0
  else if (m.$ === 'Node') {
    const q = size >> 2
    flat(m.a, w, lo, q); flat(m.b, w, lo + q, q); flat(m.c, w, lo + 2 * q, q); flat(m.d, w, lo + 3 * q, q)
  }
  return w
}

// the next session, read back out of the words
function session(w) {
  const s = {}
  NAMES.forEach((k, i) => {
    const v = w[i]
    s[k] = k === 'mode' ? E.Mode[v] : k === 'medal' ? (v ? E.Medal[v - 1] : null)
      : k === 'settled' || k === 'first' ? v !== 0 : v
  })
  s.done = w[12] ? w[13] : null
  delete s.done_some
  s.bank = {}; s.beat = {}
  TALLY.forEach(([k], i) => { s.bank[k] = w[16 + i]; s.beat[k] = w[23 + i] })
  s.seats = {}
  for (let id = 1; id <= 6; id++) {
    const b = SEATS + (id - 1) * SEAT_W
    if (w[b]) s.seats[id] = { joined: w[b + 1], craft: getCraft(w, b + 2) }
  }
  s.wing = Array.from({ length: w[33] }, (_, k) => ({ ...get(w, WINGS + k * WING_W, WING), craft: getCraft(w, WINGS + k * WING_W + 3) }))
  s.foes = out(w, FOES, FOE_W, FOE, w[30])
  s.props = out(w, PROPS, PROP_W, PROP, w[31])
  s.bolts = out(w, BOLTS, BOLT_W, BOLT, w[32])
  s.calls = out(w, CALLS, CALL_W, CALL, w[34])
  s.fx = out(w, FXS, FX_W, FLARE, w[35])
  s.boss = null
  if (w[36]) {
    s.boss = {
      ...get(w, BOSS, BEAST_A), lamps: [w[BOSS + 5], w[BOSS + 6], w[BOSS + 7]],
      marks: Array.from({ length: w[BOSS + 8] }, (_, k) => [w[BOSS + 9 + 2 * k], w[BOSS + 10 + 2 * k]]),
      ...get(w, BOSS + 25, BEAST_B),
    }
  }
  return s
}

module.exports = {
  apiVersion: 1,
  render(input = {}) {
    const m = bend[input.op === 'bot' ? 'bot' : 'tick']
    if (input.op === 'bot') {
      const w = flat(m(lay(frame({ ...input, here: [], keys: {} })), input.id >>> 0, input.novice === true))
      const [move_x, move_y, aim_x, aim_y, fire, dash, bomb, interact, swing] = Array.from({ length: 9 }, (_, k) => w[BOT_OUT + k])
      return { move_x, move_y, aim_x, aim_y, fire: fire !== 0, dash: dash !== 0, bomb: bomb !== 0, interact: interact !== 0, swing: swing !== 0 }
    }
    const w = flat(m(lay(frame(input))))
    return {
      session: session(w),
      sounds: Array.from(w.subarray(SND, SND + w[37])),
      leave: Array.from(w.subarray(LEAVE, LEAVE + w[38])),
    }
  },
}
