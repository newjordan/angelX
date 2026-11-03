// Thin adapter for Bannerpeak's authored rules. Plain JSON in, the compiled
// leg's answer out: which tiles a cell is, whether they are solid and whether a
// knight may come up through them all live in map.bend and tiles.bend, and King
// Cinder's tick and his slam in boss.bend. Rust owns the session, the knight's
// shape, the saves and the wire.
const rules = require('./bannerpeak-rules.compiled.js')

const word = value => (Number.isFinite(value) ? Math.trunc(value) >>> 0 : 0)
const flag = value => value === true

const BEATS = ['Dormant', 'Rest', 'Tell', 'Strike', 'Wince', 'Rage', 'Down']
const ATTACKS = ['Stomp', 'Sweep', 'Embers']
const index = (names, value) => {
  const at = names.indexOf(value)
  if (at < 0) throw Error('bannerpeak-rules: unknown name')
  return at
}

const planks = list => {
  const out = []
  for (let node = list; node.$ === 'Con'; node = node.tail) {
    out.push({ id: Number(node.head.id), x: Number(node.head.x), y: Number(node.head.y), w: Number(node.head.w) })
  }
  return out
}

// A list of records in, the same list as Bend's cons cells out (and back).
const cons = (values, shape) => {
  let list = { $: 'Nil' }
  for (let at = values.length - 1; at >= 0; at--) list = { $: 'Con', head: shape(values[at]), tail: list }
  return list
}
const cells = (list, shape) => {
  const out = []
  for (let node = list; node.$ === 'Con'; node = node.tail) out.push(shape(node.head))
  return out
}

// The feet of the seats standing, in seat order, in six fixed slots.
const feet = values => {
  const slots = (values || []).slice(0, 6).map(pair => [word(pair[0]), word(pair[1])])
  while (slots.length < 6) slots.push([0, 0])
  return {
    $: 'Feet',
    n: slots.length ? (values || []).length : 0,
    ax: slots[0][0], ay: slots[0][1],
    bx: slots[1][0], by: slots[1][1],
    cx: slots[2][0], cy: slots[2][1],
    dx: slots[3][0], dy: slots[3][1],
    ex: slots[4][0], ey: slots[4][1],
    fx: slots[5][0], fy: slots[5][1],
  }
}

const state = boss => ({
  $: 'State',
  x: word(boss.x),
  beat: index(BEATS, boss.beat),
  kind: index(ATTACKS, boss.kind),
  t: word(boss.t),
  phase: word(boss.phase),
  plates: word(boss.plates),
  count: word(boss.count),
  head: word(boss.head),
  head_before: word(boss.head_before),
  x_before: word(boss.x_before),
  flinch: word(boss.flinch),
})

const hazard = value => ({
  $: 'Hazard',
  x: word(value.x), y: word(value.y), w: word(value.w), h: word(value.h),
  vx: word(value.vx), tell: word(value.tell), live: word(value.live),
})
const hazards = values => cons(values || [], hazard)

const back = (value, list) => ({
  x: Number(value.x) | 0,
  beat: BEATS[Number(value.beat)],
  kind: ATTACKS[Number(value.kind)],
  t: Number(value.t),
  phase: Number(value.phase),
  plates: Number(value.plates),
  count: Number(value.count),
  head: Number(value.head) | 0,
  head_before: Number(value.head_before) | 0,
  x_before: Number(value.x_before) | 0,
  flinch: Number(value.flinch),
  hazards: cells(list, h => ({
    x: Number(h.x) | 0, y: Number(h.y) | 0, w: Number(h.w) | 0, h: Number(h.h) | 0,
    vx: Number(h.vx) | 0, tell: Number(h.tell), live: Number(h.live),
  })),
})

const op = input => {
  switch (input && input.op) {
    case 'world':
      return { $: 'World' }
    case 'tile':
      return { $: 'Tile', tx: word(input.tx), ty: word(input.ty) }
    case 'movers':
      return { $: 'Movers', t: word(input.t) }
    default:
      throw Error('bannerpeak-rules: unknown op')
  }
}

const answer = value => {
  switch (value.$) {
    case 'Whole':
      return { op: 'world', rows: Number(value.rows), runs: Number(value.runs), width: Number(value.width), height: Number(value.height) }
    case 'Code':
      return { op: 'tile', code: Number(value.code), solid: flag(value.solid), one_way: flag(value.one_way) }
    case 'Movers':
      return { op: 'movers', planks: planks(value.list) }
    default:
      throw Error('bannerpeak-rules: unknown answer')
  }
}

// King Cinder's tick, and the slam that takes a plate off his crown.
const tick = input => {
  if (!input || !input.boss) throw Error('bannerpeak-rules: no boss')
  const next = rules.bannerpeak_boss(
    word(input.elapsed),
    word(input.seats),
    feet(input.feet),
    state(input.boss),
    hazards(input.boss.hazards)
  )
  return { op: 'boss', boss: back(next.state, next.hazards) }
}

const slammed = input => {
  if (!input || !input.boss) throw Error('bannerpeak-rules: no boss')
  const next = rules.bannerpeak_slam(
    state(input.boss),
    word(input.x),
    word(input.y),
    hazards(input.boss.hazards)
  )
  return { op: 'slam', hit: flag(next.hit), boss: back(next.state, next.hazards) }
}

module.exports = {
  apiVersion: 1,
  render: input => {
    if (input && input.op === 'boss') return tick(input)
    if (input && input.op === 'slam') return slammed(input)
    return answer(rules.run(op(input)))
  },
}
