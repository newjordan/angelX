// Marshalling for the Moonfall rules leg. Rust sends the session as it saves,
// the key rows, who is awake and the page count of each open talk; every rule is
// in moonfall-rules.bend. Enums become small numbers on the way in (the tables
// below are the order of the Rust enums) and names again on the way out.
const bend = require('./moonfall-rules.compiled.js')

const FOLK = ['Mira', 'Tovin', 'Nessa', 'Orin', 'Pella', 'Aster']
const ITEM = ['Empty', 'Ink', 'Reeds', 'Pass', 'Scroll', 'Shoe']
const MASK = ['Bare', 'Swift', 'Stout']
const FACT = ['OrinLost', 'NessaCold', 'PellaScroll', 'TovinPass', 'MiraSeal', 'AsterComing', 'AsterLame', 'Verse']
const RING = ['Rewind', 'Dusk', 'Dawn']
const SIGN = ['NoInk', 'InkFound', 'ReedsFound', 'HandsFull', 'NorthLocked', 'NorthOpened', 'SouthLocked', 'SouthOpened', 'TooHeavy', 'WellIdle', 'WellOiled', 'TowerIdle', 'TowerNeedsVerse', 'SealKept', 'KeyKept', 'Relit', 'Dropped']
const TOPIC_BARE = ['', 'Request', 'Give', 'Thanks', 'Done']

// The knight (33 words) and the session's scalars (43 words) are trees of words, four
// children to a node and three deep, filled in the order of these names.
const KN = ['id', 'joined', 'x', 'y', 'face', 'stride', 'roll', 'roll_rest', 'roll_dx', 'roll_dy', 'keys', 'held', 'held_at', 'mask', 'candles', 'hurt', 'down', 'hold', 'push', 'read', 'panel', 'parg', 'say_on', 'say_folk', 'say_tk', 'say_ta', 'say_page', 'say_age', 'note_on', 'note_kind', 'note_a', 'note_b', 'note_left']
const SS = ['elapsed', 'clock', 'phase', 'p_t', 'p_from', 'p_to', 'p_fell', 'card', 'done_on', 'done_at', 'peak', 'flags', 'ink_on', 'ink_given', 'shod_on', 'shod', 'crate_on', 'crate_x', 'crate_y', 'snuffed', 'lit0', 'lit1', 'lit2', 'facts', 'ticks', 'masks', 'keeps', 'loops', 'call_on', 'call_ring', 'call_by', 'call_ayes', 'call_left', 'cry_on', 'cry_folk', 'cry_tk', 'cry_ta', 'cry_age', 'n_day', 'n_walk', 'n_wisps', 'n_rewind', 'n_notice']
const grow = (values, at, size) => {
  if (size === 1) return { $: 'Lf', x: values[at] || 0 }
  const q = size / 4
  return { $: 'Qd', a: grow(values, at, q), b: grow(values, at + q, q), c: grow(values, at + 2 * q, q), d: grow(values, at + 3 * q, q) }
}
const treeOf = (names, fields) => grow(names.map(n => fields[n]), 0, 64)
const fieldsOf = (names, tree) => {
  const out = []
  const walk = t => (t.$ === 'Lf' ? out.push(t.x) : [t.a, t.b, t.c, t.d].forEach(walk))
  walk(tree)
  return Object.fromEntries(names.map((n, i) => [n, out[i]]))
}

const word = value => (Number.isFinite(value) ? Math.trunc(value) >>> 0 : 0)
const index = (table, name) => {
  const at = table.indexOf(name)
  if (at < 0) throw Error('unknown ' + name)
  return at
}
const nil = { $: 'Nil' }
const cons = values => values.reduceRight((tail, head) => ({ $: 'Con', head, tail }), nil)
const array = node => {
  const out = []
  for (let at = node; at.$ === 'Con'; at = at.tail) out.push(at.head)
  return out
}
const tag = value => (typeof value === 'string' ? [value, undefined] : Object.entries(value)[0])

// Topic, note, panel, phase and wisp mode: externally tagged on the Rust side.
const topicIn = value => {
  const [name, arg] = tag(value)
  const bare = TOPIC_BARE.indexOf(name)
  if (bare > 0) return [bare, 0]
  return [name === 'Hello' ? 0 : name === 'Nag' ? 5 : 6, word(arg)]
}
const topicOut = (kind, arg) => (kind === 0 ? { Hello: arg } : kind === 5 ? { Nag: arg } : kind === 6 ? { Cry: arg } : TOPIC_BARE[kind])

const noteIn = value => {
  const [name, arg] = tag(value)
  switch (name) {
    case 'Wrote': return [0, index(FOLK, arg[0]), word(arg[1])]
    case 'Learned': return [1, index(FACT, arg), 0]
    case 'Ticked': return [2, index(FOLK, arg), 0]
    case 'Remind': return [3, index(FOLK, arg[0]), word(arg[1])]
    case 'Sign': return [4, index(SIGN, arg), 0]
    case 'Got': return [5, index(ITEM, arg), 0]
    case 'Mask': return [6, index(MASK, arg), 0]
    case 'Candle': return [7, word(arg), 0]
    case 'Snuffed': return [8, 0, 0]
    case 'Lit': return [9, word(arg), 0]
    case 'Out': return [10, 0, 0]
    case 'Ring': return [11, word(arg), 0]
    default: throw Error('unknown note')
  }
}
const noteOut = (kind, a, b) => {
  switch (kind) {
    case 0: return { Wrote: [FOLK[a], b] }
    case 1: return { Learned: FACT[a] }
    case 2: return { Ticked: FOLK[a] }
    case 3: return { Remind: [FOLK[a], b] }
    case 4: return { Sign: SIGN[a] }
    case 5: return { Got: ITEM[a] }
    case 6: return { Mask: MASK[a] }
    case 7: return { Candle: a }
    case 8: return 'Snuffed'
    case 9: return { Lit: a }
    case 10: return 'Out'
    case 11: return { Ring: a }
    default: throw Error('unknown note kind')
  }
}

const knightIn = (id, seat) => {
  const k = seat.knight
  const [pn, pa] = tag(k.panel)
  const say = k.say
  const [stk, sta] = say ? topicIn(say.topic) : [0, 0]
  const note = k.note ? noteIn(k.note[0]) : [0, 0, 0]
  return {
    $: 'Kn',
    w: treeOf(KN, {
      id, joined: word(seat.joined), x: word(k.x), y: word(k.y), face: word(k.face), stride: word(k.stride), roll: word(k.roll), roll_rest: word(k.roll_rest),
      roll_dx: word(k.roll_dir[0]), roll_dy: word(k.roll_dir[1]), keys: word(k.keys),
      held: index(ITEM, k.held), held_at: word(k.held_at), mask: index(MASK, k.mask), candles: word(k.candles), hurt: word(k.hurt), down: word(k.down), hold: word(k.hold), push: word(k.push), read: word(k.read),
      panel: pn === 'Shut' ? 0 : pn === 'Ledger' ? 1 : 2, parg: word(pa),
      say_on: say ? 1 : 0, say_folk: say ? index(FOLK, say.folk) : 0, say_tk: stk, say_ta: sta, say_page: say ? word(say.page) : 0, say_age: say ? word(say.age) : 0,
      note_on: k.note ? 1 : 0, note_kind: note[0], note_a: note[1], note_b: note[2], note_left: k.note ? word(k.note[1]) : 0,
    }),
  }
}
const seatsIn = map => cons(Object.keys(map || {}).map(Number).sort((a, b) => a - b).map(id => knightIn(id, map[id])))

const knightOut = kn => {
  const k = fieldsOf(KN, kn.w)
  const panel = k.panel === 0 ? 'Shut' : k.panel === 1 ? { Ledger: k.parg } : { Bell: k.parg }
  return {
    joined: k.joined,
    knight: {
      x: k.x | 0, y: k.y | 0, face: k.face, stride: k.stride, held: ITEM[k.held], held_at: k.held_at, mask: MASK[k.mask], candles: k.candles,
      hurt: k.hurt, down: k.down, roll: k.roll, roll_rest: k.roll_rest, roll_dir: [k.roll_dx | 0, k.roll_dy | 0], keys: k.keys, panel,
      say: k.say_on ? { folk: FOLK[k.say_folk], topic: topicOut(k.say_tk, k.say_ta), page: k.say_page, age: k.say_age } : null,
      hold: k.hold, push: k.push,
      note: k.note_on ? [noteOut(k.note_kind, k.note_a, k.note_b), k.note_left] : null,
      read: k.read,
    },
  }
}
const seatsOut = list => {
  const out = {}
  for (const kn of array(list)) out[fieldsOf(KN, kn.w).id] = knightOut(kn)
  return out
}

const some = value => (value === null || value === undefined ? [0, 0] : [1, word(value)])
const option = (on, value) => (on ? value : null)

const sessionIn = (frame) => {
  const s = frame.session
  const w = s.world
  const m = s.memory
  if (w.crates.length > 1) throw Error('more than one crate')
  const crate = w.crates[0]
  const [phase, pt, pfrom, pto, pfell] = (() => {
    const [name, v] = tag(s.phase)
    switch (name) {
      case 'Play': return [0, 0, 0, 0, 0]
      case 'Rewind': return [1, word(v.t), word(v.from), 0, v.fell ? 1 : 0]
      case 'Skip': return [2, word(v.t), word(v.from), word(v.to), 0]
      default: return [3, word(v.t), 0, 0, 0]
    }
  })()
  const [inkOn, ink] = some(w.ink_given)
  const [shodOn, shod] = some(w.shod)
  const [doneOn, done] = some(s.completed)
  const call = s.call
  const cry = s.cry
  const [ctk, cta] = cry ? topicIn(cry[1]) : [0, 0]
  const n = s.nums
  return {
    $: 'Sess',
    seats: seatsIn(s.seats),
    parked: seatsIn(s.parked),
    wisps: cons((w.wisps || []).map(x => {
      const [name, t] = tag(x.mode)
      return { $: 'Wp', x: word(x.x), y: word(x.y), mode: name === 'Hunt' ? 0 : name === 'Snuff' ? 1 : 2, t: word(t), aim: word(x.aim), born: word(x.born) }
    })),
    news: cons((s.news || []).map(([note, left]) => {
      const [kind, a, b] = noteIn(note)
      return { $: 'Nw', kind, a, b, left: word(left) }
    })),
    rt: cons(m.routines.map(word)),
    cues: nil,
    pending: cons((frame.pending || []).map(word)),
    inputs: cons((frame.inputs || []).map(r => ({ $: 'In', id: word(r[0]), valid: r[1] ? 1 : 0, interact: r[2] ? 1 : 0, take: r[3] ? 1 : 0, swing: r[4] ? 1 : 0, ult: r[5] ? 1 : 0, play: r[6] ? 1 : 0, bomb: r[7] ? 1 : 0, dash: r[8] ? 1 : 0, mx: word(r[9]), my: word(r[10]), ax: word(r[11]), ay: word(r[12]) }))),
    heroes: cons((frame.heroes || []).map(r => ({ $: 'He', id: word(r[0]), awake: r[1] ? 1 : 0 }))),
    pages: cons((frame.pages || []).map(r => ({ $: 'Pg', id: word(r[0]), pages: word(r[1]) }))),
    w: treeOf(SS, {
      elapsed: word(s.elapsed), clock: word(s.clock), phase, p_t: pt, p_from: pfrom, p_to: pto, p_fell: pfell, card: word(s.card), done_on: doneOn, done_at: done, peak: word(s.peak),
      flags: word(w.flags), ink_on: inkOn, ink_given: ink, shod_on: shodOn, shod, crate_on: crate ? 1 : 0, crate_x: crate ? word(crate[0]) : 0, crate_y: crate ? word(crate[1]) : 0,
      snuffed: word(w.snuffed), lit0: word(w.lit_until[0]), lit1: word(w.lit_until[1]), lit2: word(w.lit_until[2]),
      facts: word(m.facts), ticks: word(m.ticks), masks: word(m.masks), keeps: word(m.keeps), loops: word(m.loops),
      call_on: call ? 1 : 0, call_ring: call ? index(RING, call.ring) : 0, call_by: call ? word(call.by) : 0, call_ayes: call ? word(call.ayes) : 0, call_left: call ? word(call.left) : 0,
      cry_on: cry ? 1 : 0, cry_folk: cry ? index(FOLK, cry[0]) : 0, cry_tk: ctk, cry_ta: cta, cry_age: cry ? word(cry[2]) : 0,
      n_day: word(n.day), n_walk: word(n.walk), n_wisps: word(n.wisps), n_rewind: word(n.rewind), n_notice: word(n.notice),
    }),
  }
}

const sessionOut = s => {
  const f = fieldsOf(SS, s.w)
  const t = f, w = f, m = f, c = f, n = f
  const phase = t.phase === 0 ? 'Play' : t.phase === 1 ? { Rewind: { t: t.p_t, from: t.p_from, fell: t.p_fell === 1 } } : t.phase === 2 ? { Skip: { t: t.p_t, from: t.p_from, to: t.p_to } } : { Won: { t: t.p_t } }
  const parked = seatsOut(s.parked)
  const out = {
    seats: seatsOut(s.seats),
    elapsed: t.elapsed,
    clock: t.clock,
    phase,
    card: t.card,
    world: {
      flags: w.flags,
      ink_given: option(w.ink_on, w.ink_given),
      shod: option(w.shod_on, w.shod),
      crates: w.crate_on ? [[w.crate_x, w.crate_y]] : [],
      snuffed: w.snuffed,
      lit_until: [w.lit0, w.lit1, w.lit2],
      wisps: array(s.wisps).map(x => ({ x: x.x | 0, y: x.y | 0, mode: x.mode === 0 ? 'Hunt' : x.mode === 1 ? { Snuff: x.t } : { Shy: x.t }, aim: x.aim, born: x.born })),
    },
    memory: { facts: m.facts, routines: array(s.rt), ticks: m.ticks, masks: m.masks, keeps: m.keeps, loops: m.loops },
    call: c.call_on ? { ring: RING[c.call_ring], by: c.call_by, ayes: c.call_ayes, left: c.call_left } : null,
    cry: c.cry_on ? [FOLK[c.cry_folk], topicOut(c.cry_tk, c.cry_ta), c.cry_age] : null,
    news: array(s.news).map(x => [noteOut(x.kind, x.a, x.b), x.left]),
    completed: option(t.done_on, t.done_at),
    peak: t.peak,
    nums: { day: n.n_day, walk: n.n_walk, wisps: n.n_wisps, rewind: n.n_rewind, notice: n.n_notice },
  }
  if (Object.keys(parked).length) out.parked = parked
  return out
}

module.exports = {
  apiVersion: 1,
  render(input = {}) {
    if (input.op === 'step') {
      const out = bend.main_step(sessionIn(input))
      return {
        session: sessionOut(out.s),
        cues: array(out.s.cues),
        leaving: array(out.leaving),
        won: out.won ? [out.won, out.best] : null,
      }
    }
    if (input.op === 'bot') {
      const ip = bend.main_bot(sessionIn(input), word(input.id))
      return { move_x: ip.mx | 0, move_y: ip.my | 0, interact: ip.interact === 1, ult: ip.ult === 1, swing: ip.swing === 1, play: ip.play, bomb: ip.bomb === 1, dash: ip.dash === 1 }
    }
    throw Error('unknown moonfall op')
  },
}
