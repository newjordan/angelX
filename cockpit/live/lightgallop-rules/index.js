// Thin adapter for the Lightgallop rules. The session arrives as plain JSON,
// is laid out into the slots the compiled rules read (their order is the
// order of rs.bend and hs.bend), and the answer is read back out the same way.
// Nothing here decides anything: no arithmetic beyond the 32-bit bit patterns
// and no rule of the ride lives in this file.
//
// A signed number crosses as the same bits Rust keeps. A rider's spark bits are
// 64-bit words and cross as pairs of 32-bit halves, high half first, because a
// JSON number past 2^53 does not survive JavaScript.
const rules = require('./lightgallop-rules.compiled')

const DEPTH = 11
const SLOTS = 1 << DEPTH

// ---- the layout ----------------------------------------------------------------

const RIDER = ['x', 'y', 'vx', 'vy', 'ground_on', 'ground_path', 'ground_s', 'gsp', 'dir', 'roll', 'ball', 'crouch', 'skid',
  'charge', 'spur', 'gauge', 'lock_on', 'lock_target', 'lock_ticks', 'lock_cd', 'spring_cd', 'freeze', 'shake', 'lunge', 'lunged',
  'chain', 'best_chain', 'sparks', 'shield', 'hurt', 'ctrl', 'down', 'coyote', 'buffer', 'jump_key', 'spur_key', 'jumping', 'air',
  'draft', 'ang', 'cam_x', 'cam_y', 'zoom', 'looping', 'want_lock', 'crates', 'collected', 'falls', 'hits', 'kills', 'finish_on',
  'finish_at', 'score', 'loose_n', 'id', 'joined']
const AT = Object.fromEntries(RIDER.map((name, i) => [name, i]))
const FLAGS = ['roll', 'ball', 'crouch', 'skid', 'spur', 'lunged', 'shield', 'jump_key', 'spur_key', 'jumping', 'draft', 'looping', 'want_lock']
const SIGNED = ['x', 'y', 'vx', 'vy', 'gsp', 'dir', 'charge', 'gauge', 'ang', 'cam_x', 'cam_y', 'zoom']
const SEAT = id => 768 + (id - 1) * 192
const TAKEN = 56
const LOOSE = 80
const FOES = 288
const TIMERS = { post_t: 256, spring_t: 264, pad_t: 272 }
const TALLY = ['at', 'time', 'sparks', 'chain', 'posts', 'falls', 'points', 'rank', 'best']
const NUMBERS = ['slope', 'loop_min', 'spur_cap', 'lock_reach', 'dimming']
const INPUT = id => 114 + id * 6
const PLAYERS = 6

const u32 = value => Number(value) >>> 0
const bits = value => (value === true ? 1 : value === false || value == null ? 0 : Number(value) >>> 0)

// ---- slots to a tree and back ----------------------------------------------------

const tree = slots => {
  let level = slots.map(v => ({ $: 'Lf', v }))
  while (level.length > 1) {
    const next = []
    for (let i = 0; i < level.length; i += 2) next.push({ $: 'Nd', lo: level[i], hi: level[i + 1] })
    level = next
  }
  return level[0]
}

const slotsOf = root => {
  const out = new Array(SLOTS)
  const walk = (node, from, width) => {
    if (node.$ === 'Lf') out[from] = node.v >>> 0
    else {
      walk(node.lo, from, width / 2)
      walk(node.hi, from + width / 2, width / 2)
    }
  }
  walk(root, 0, SLOTS)
  return out
}

const list = values => values.reduceRight((tail, head) => ({ $: 'Con', head, tail }), { $: 'Nil' })
const arr = node => {
  const out = []
  for (; node.$ === 'Con'; node = node.tail) out.push(node.head)
  return out
}

// ---- the session as slots -----------------------------------------------------------

const putRider = (s, base, id, seat) => {
  const r = seat.rider
  const put = (name, value) => { s[base + AT[name]] = bits(value) }
  for (const name of RIDER) if (r[name] !== undefined && !['ground_on', 'ground_path', 'ground_s', 'lock_on', 'lock_target', 'lock_ticks', 'finish_on', 'finish_at', 'loose_n', 'id', 'joined'].includes(name)) put(name, r[name])
  put('ground_on', r.ground ? 1 : 0)
  put('ground_path', r.ground ? r.ground.path : 0)
  put('ground_s', r.ground ? r.ground.s : 0)
  put('lock_on', r.lock ? 1 : 0)
  put('lock_target', r.lock ? r.lock.target : 0)
  put('lock_ticks', r.lock ? r.lock.ticks : 0)
  put('finish_on', r.finish == null ? 0 : 1)
  put('finish_at', r.finish == null ? 0 : r.finish)
  put('id', id)
  put('joined', seat.joined)
  r.taken.forEach((half, i) => { s[base + TAKEN + i] = bits(half) })
  put('loose_n', r.loose.length)
  r.loose.forEach((l, i) => {
    const o = base + LOOSE + i * 5
    s[o] = bits(l.x); s[o + 1] = bits(l.y); s[o + 2] = bits(l.vx); s[o + 3] = bits(l.vy); s[o + 4] = bits(l.age)
  })
}

const layOut = ask => {
  const s = new Array(SLOTS).fill(0)
  const session = ask.session
  const present = ask.present || []
  present.forEach(id => { if (id >= 1 && id <= PLAYERS) s[111 + id] = 1 })
  const n = ask.numbers || {}
  NUMBERS.forEach((name, i) => { if (n[name] !== undefined) s[31 + i] = bits(n[name]) })
  if (n.slope === undefined) { s[31] = 100; s[32] = 512; s[33] = 2048; s[34] = 96; s[35] = 1000 }
  Object.entries(ask.inputs || {}).forEach(([key, input]) => {
    const id = Number(key)
    if (!(id >= 1 && id <= PLAYERS)) return
    const o = INPUT(id)
    ;[input.move_x, input.move_y, input.aim_y, input.interact, input.dash, input.fire].forEach((v, i) => { s[o + i] = bits(v) })
  })
  if (session) {
    const h = ['elapsed', 'dim', 'posts', 'provisioned', 'warden_inv']
    h.forEach((name, i) => { s[i] = bits(session[name]) })
    s[5] = session.bell_t == null ? 0 : 1
    s[6] = session.bell_t == null ? 0 : bits(session.bell_t)
    s[7] = bits(session.paid)
    s[8] = bits(session.again)
    const t = session.tally
    s[9] = t ? 1 : 0
    if (t) {
      TALLY.forEach((name, i) => { s[10 + i] = bits(t[name]) })
      Object.entries(t.scores).forEach(([key, score]) => { s[18 + Number(key)] = bits(score); s[24 + Number(key)] = 1 })
    }
    for (const [name, base] of Object.entries(TIMERS)) session[name].forEach((v, i) => { s[base + i] = bits(v) })
    session.foes.forEach((f, i) => { s[FOES + i * 4] = bits(f.hp); s[FOES + i * 4 + 1] = bits(f.pop); s[FOES + i * 4 + 2] = bits(f.flinch) })
    s[106] = session.foes.length
    Object.entries(session.seats).forEach(([key, seat]) => putRider(s, SEAT(Number(key)), Number(key), seat))
  }
  return s
}

// ---- slots to the session -------------------------------------------------------------

const getRider = (s, base) => {
  const get = name => s[base + AT[name]]
  const signed = name => get(name) | 0
  const r = {}
  for (const name of RIDER) {
    if (['ground_on', 'ground_path', 'ground_s', 'lock_on', 'lock_target', 'lock_ticks', 'finish_on', 'finish_at', 'loose_n', 'id', 'joined'].includes(name)) continue
    r[name] = FLAGS.includes(name) ? get(name) !== 0 : SIGNED.includes(name) ? signed(name) : get(name)
  }
  r.ground = get('ground_on') ? { path: get('ground_path'), s: signed('ground_s') } : null
  r.lock = get('lock_on') ? { target: get('lock_target'), ticks: get('lock_ticks') } : null
  r.finish = get('finish_on') ? get('finish_at') : null
  r.loose = Array.from({ length: get('loose_n') }, (_, i) => {
    const o = base + LOOSE + i * 5
    return { x: s[o] | 0, y: s[o + 1] | 0, vx: s[o + 2] | 0, vy: s[o + 3] | 0, age: s[o + 4] }
  })
  r.taken = Array.from({ length: 24 }, (_, i) => s[base + TAKEN + i])
  return r
}

const readOut = (s, foes, timers) => {
  const seats = {}
  for (let id = 1; id <= PLAYERS; id++) {
    const base = SEAT(id)
    if (s[base + AT.id] !== 0) seats[id] = { joined: s[base + AT.joined], rider: getRider(s, base) }
  }
  const scores = {}
  for (let id = 1; id <= PLAYERS; id++) if (s[24 + id] === 1) scores[id] = s[18 + id]
  const tally = s[9]
    ? Object.assign(Object.fromEntries(TALLY.map((name, i) => [name, s[10 + i]])), { scores })
    : null
  return {
    seats,
    elapsed: s[0],
    dim: s[1] | 0,
    posts: s[2],
    post_t: Array.from({ length: timers.post_t }, (_, i) => s[TIMERS.post_t + i]),
    provisioned: s[3],
    foes: Array.from({ length: foes }, (_, i) => ({ hp: s[FOES + i * 4], pop: s[FOES + i * 4 + 1], flinch: s[FOES + i * 4 + 2] })),
    spring_t: Array.from({ length: timers.spring_t }, (_, i) => s[TIMERS.spring_t + i]),
    pad_t: Array.from({ length: timers.pad_t }, (_, i) => s[TIMERS.pad_t + i]),
    warden_inv: s[4],
    bell_t: s[5] ? s[6] : null,
    tally,
    paid: s[7] !== 0,
    again: s[8] !== 0,
  }
}

// ---- the road ------------------------------------------------------------------------------

// The road is laid out the first time it is asked for and kept for as long as
// the generation lives.
let laid = null
const road = () => laid || (laid = rules.road())

const KINDS = ['Road', 'Rail', 'Ring']
const FOE_KINDS = ['Snuffer', 'Floater', 'Bramble', 'Warden']
const SPRING_KINDS = ['Up', 'Diag']
const CRATE_KINDS = ['Shield', 'Sparks', 'Spur']
const i32 = value => value | 0
const leaves = (node, out) => {
  if (node.$ === 'Pl') out.push(node.p)
  else { leaves(node.lo, out); leaves(node.hi, out) }
  return out
}
const link = (on, path, s) => (on ? { path, s: i32(s) } : null)
// Signed, delta-coded integers in a JSON-safe base-32 varint alphabet.
// The road's geometry is much larger in expanded JSON than on the wire.
const pack = values => {
  const alphabet = 'ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789-_'
  let previous = 0, out = ''
  for (const value of values) {
    const delta = value - previous
    previous = value
    let n = delta < 0 ? -delta * 2 - 1 : delta * 2
    do {
      const part = n % 32
      n = Math.floor(n / 32)
      out += alphabet[part + (n ? 32 : 0)]
    } while (n)
  }
  return out
}
const pathOf = p => {
  const pts = leaves(p.pts, []).slice(0, p.n)
  const segments = pts.slice(0, -1)
  return {
    kind: KINDS[p.kind],
    pts: pack(pts.flatMap(q => [i32(q.x), i32(q.y)])),
    cum: pack(pts.map(q => i32(q.cum))),
    dir: pack(segments.flatMap(q => [i32(q.dx), i32(q.dy)])),
    mid: pack(segments.map(q => i32(q.mid))),
    next: link(p.next_on, p.next_path, p.next_s),
    prev: link(p.prev_on, p.prev_path, p.prev_s),
    portals: arr(p.portals).map(q => ({ s: i32(q.s), dir: i32(q.dir), min_speed: i32(q.min_speed), to: { path: q.to_path, s: i32(q.to_s) } })),
  }
}
const levelOf = r => ({
  terrain: { paths: arr(r.paths).map(pathOf) },
  width: i32(r.width),
  dead_y: i32(r.dead_y),
  acts: [r.a0, r.a1, r.a2, r.a3].map(i32),
  main: r.main,
  start: [r.start_path, i32(r.start_s)],
  sparks: arr(r.sparks).map(q => [i32(q.x), i32(q.y)]),
  springs: arr(r.springs).map(q => ({ x: i32(q.x), y: i32(q.y), kind: SPRING_KINDS[q.kind] })),
  pads: arr(r.pads).map(q => ({ x: i32(q.x), y: i32(q.y) })),
  posts: arr(r.posts).map(q => ({ x: i32(q.x), y: i32(q.y), path: q.path, s: i32(q.s), hold: i32(q.hold) })),
  crates: arr(r.crates).map(q => ({ x: i32(q.x), y: i32(q.y), kind: CRATE_KINDS[q.kind] })),
  foes: arr(r.foes).map(f => ({ kind: FOE_KINDS[f.kind], x: i32(f.x), y: i32(f.y), path: f.path, range: i32(f.range), min_seats: f.min_seats, phase: f.phase })),
  signs: arr(r.signs).map(q => ({ x: i32(q.x), y: i32(q.y), words: q.words })),
  bell: [i32(r.bell_x), i32(r.bell_y)],
  par: r.par,
})

// ---- the asks ------------------------------------------------------------------------------

const timersOf = session => ({ post_t: session.post_t.length, spring_t: session.spring_t.length, pad_t: session.pad_t.length })

const sounds = s => s.slice(37, 37 + s[36])

module.exports = {
  apiVersion: 1,
  render: ask => {
    switch (ask && ask.op) {
      case 'level':
        return levelOf(road())
      case 'start': {
        const s = slotsOf(rules.start(road(), tree(layOut(ask))))
        const lengths = { post_t: arr(road().posts).length, spring_t: arr(road().springs).length, pad_t: arr(road().pads).length }
        return { session: readOut(s, s[106], lengths) }
      }
      case 'tick': {
        const s = slotsOf(rules.tick(road(), tree(layOut(ask))))
        return { session: readOut(s, ask.session.foes.length, timersOf(ask.session)), sounds: sounds(s) }
      }
      case 'bot': {
        const slots = layOut(ask)
        const skill = ask.skill || { spur: true, roll: true, lag: 0, hesitate: 0 }
        slots[192] = ask.route === 'Upper' ? 0 : ask.route === 'Lower' ? 1 : 2
        slots[193] = bits(skill.spur)
        slots[194] = bits(skill.roll)
        slots[195] = bits(skill.lag)
        slots[196] = bits(skill.hesitate)
        const p = rules.bot(road(), tree(slots), u32(ask.id))
        return { input: { move_x: i32(p.x), move_y: p.down, aim_x: 0, aim_y: 0, fire: false, dash: p.spur !== 0, bomb: false, interact: p.jump !== 0 } }
      }
      default:
        throw new Error('lightgallop-rules: unknown op ' + String(ask && ask.op))
    }
  },
}
