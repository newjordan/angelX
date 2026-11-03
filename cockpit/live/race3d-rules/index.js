// Thin adapter for the 3D races' Bend leg. The tick takes the race the way the
// saves and the wire hold it (names for enums, a map of riders, a u64 of rings,
// a null for "not home yet") and answers in the same shape. Every
// representation trick lives here: enum indices, the rings' two halves, f32
// rounding, the Bend lists. The rules are in race3d-rules.bend.
const rules = require('./race3d-rules.compiled')

const nil = { $: 'Nil' }
const cons = values => values.reduceRight((tail, head) => ({ $: 'Con', head, tail }), nil)
const ordered = list => {
  const out = []
  for (let cell = list; cell && cell.$ === 'Con'; cell = cell.tail) out.push(cell.head)
  return out
}

const f = value => Math.fround(Number.isFinite(value) ? value : 0)
const u = value => (Number.isFinite(value) ? Math.trunc(value) >>> 0 : 0)
const b = value => value === true

// The enums, in the order the Bend codes them.
const COURSES = ['Shaft', 'Cart', 'Griffin', 'Touge']
const WRECKS = ['None', 'Fall', 'Bonk', 'Buffer', 'Gap', 'Timber', 'Tumble', 'SpinOut']
const GROUNDS = ['Floor', 'Bridge', 'Crossing', 'Lodestone', 'Sluice', 'Collapse', 'Sorter', 'Rails', 'Trestle', 'Gorge']
const KINDS = ['Gem', 'Ring', 'Gargoyle', 'Timber']
// The room's sounds by the Bend's codes.
const SOUNDS = { 0: 'roll', 1: 'clang', 2: 'gold', 3: 'knight_fall', 4: 'helmet_pop', 5: 'rock_land', 6: 'wheel_land', 7: 'wheel_spin', 8: 'swing', 9: 'shot_bolt', 10: 'hit_arrow', 11: 'step_wet', 12: 'circle_toll', 14: 'play_card' }
const code = (names, name) => {
  const index = names.indexOf(name)
  if (index < 0) throw new Error('race3d-rules: no ' + String(name) + ' in the race')
  return index
}

// A u64 crosses as a number, and the Bend holds it as two u32 halves.
const HALF = 4294967296

const drift = value => ({
  $: 'Drift',
  heading: f(value.heading),
  travel: f(value.travel),
  sliding: b(value.sliding),
  charge: u(value.charge),
  boost: u(value.boost),
  stage: u(value.boost_stage),
  corner: u(value.corner),
  last_corner: u(value.last_corner),
  score: u(value.score),
  corners: u(value.corners),
})

const rider = (id, value) => ({
  $: 'Rd',
  id,
  drift: drift(value.drift || {}),
  s: f(value.s),
  x: f(value.x),
  z: f(value.z),
  vs: f(value.vs),
  vx: f(value.vx),
  vz: f(value.vz),
  lane: u(value.lane),
  spin: f(value.spin),
  crash: u(value.crash),
  wreck: code(WRECKS, value.wreck),
  crashes: u(value.crashes),
  stun: u(value.stun),
  burst: u(value.burst),
  rolling: u(value.rolling),
  rest: u(value.rest),
  reload: u(value.reload),
  haul: u(value.haul),
  rings_hi: u(Math.floor(value.rings / HALF)),
  rings_lo: u(value.rings % HALF),
  fin: u(value.finish),
  keys: u(value.keys),
})

const bolt = value => ({ $: 'Bolt', owner: u(value.owner), s: f(value.s), x: f(value.x), z: f(value.z), life: u(value.life) })

const sample = value => ({
  $: 'Smp',
  slope: f(value.slope),
  bend: f(value.bend),
  width: f(value.width),
  walls: b(value.walls),
  ground: code(GROUNDS, value.ground),
  lanes: u(value.lanes),
  funnel: b(value.funnel),
})

const item = value => ({ $: 'It', s: f(value.s), x: f(value.x), z: f(value.z), kind: code(KINDS, value.kind) })

const block = value => ({
  $: 'Bl',
  s: f(value.s),
  len: f(value.len),
  x0: f(value.x[0]),
  x1: f(value.x[1]),
  z0: f(value.z[0]),
  z1: f(value.z[1]),
})

const gap = value => ({ $: 'Gp', lane: u(value.lane), s0: f(value.s[0]), s1: f(value.s[1]) })

const track = value => ({
  $: 'Trk',
  n: u(value.samples.length),
  looped: b(value.looped),
  finish: f(value.finish),
  samples: cons(value.samples.map(sample)),
  posts: cons(value.posts.map(f)),
  items: cons(value.items.map(item)),
  blocks: cons(value.blocks.map(block)),
  gaps: cons(value.gaps.map(gap)),
})

// One rider's seat for the tick: the keys it holds (none, when the host sent
// none), whether it stands, and the sine the sluice reads at its position.
const one = (id, ask) => {
  const held = ask.inputs[id] || {}
  return {
    $: 'One',
    seat: {
      $: 'Seat',
      id,
      mx: f(held.move_x),
      my: f(held.move_y),
      fire: b(held.fire),
      interact: b(held.interact),
      dash: b(held.dash),
      standing: ask.standing.includes(id),
    },
    sin_s: f(ask.sine[id]),
  }
}

// The planks are a map of plank to the tick it began to go; the Bend holds
// them as a flat list of pairs.
const flat = planks => cons(Object.entries(planks).flatMap(([plank, when]) => [u(Number(plank)), u(when)]))
const unflat = list => {
  const out = {}
  for (let at = 0; at + 1 < list.length; at += 2) out[list[at]] = list[at + 1]
  return out
}

const tickIn = ask => {
  const race = ask.race
  const course = code(COURSES, race.course)
  const trk = track(ask.track)
  const ids = Object.keys(race.riders).map(Number)
  return {
    $: 'Tick',
    t: u(race.t),
    course,
    finish: rules.finish_distance(trk, course),
    limit: rules.time_limit(course, b(race.crossroads)),
    riders: cons(ids.map(id => rider(id, race.riders[id]))),
    ones: cons(ids.map(id => one(id, ask))),
    rs: { $: 'Rs', t: u(race.t), taken: cons(race.taken.map(u)), planks: flat(race.planks), bolts: cons(race.bolts.map(bolt)) },
    trk,
  }
}

// A rider back as the saves hold it: what the Bend carries as indices, halves
// and zeros goes back to names, a u64 and null.
const riderOut = ({ $, id, drift: { $: _, stage, ...drift }, wreck, rings_hi, rings_lo, fin, ...rest }) => ({
  ...rest,
  drift: { ...drift, boost_stage: stage },
  wreck: WRECKS[wreck],
  rings: rings_hi * HALF + rings_lo,
  finish: fin === 0 ? null : fin,
})

const tickOut = done => ({
  riders: Object.fromEntries(ordered(done.riders).map(value => [value.id, riderOut(value)])),
  taken: ordered(done.rs.taken),
  planks: unflat(ordered(done.rs.planks)),
  bolts: ordered(done.rs.bolts).map(({ owner, s, x, z, life }) => ({ owner, s, x, z, life })),
  sounds: ordered(done.snd).map(sound => SOUNDS[sound]),
  all_home: done.all_home === true,
  timed_out: done.timed_out === true,
})

module.exports = {
  apiVersion: 1,
  render(input = {}) {
    switch (input.op) {
      case 'plank':
        return rules.rotten(u(input.plank))
      case 'grid':
        return rules.grid(u(input.course), u(input.seat))
      case 'tick':
        return tickOut(rules.tick(tickIn(input)))
      default:
        throw new Error('race3d-rules: unknown op ' + String(input.op))
    }
  },
}
