// Thin adapter for the Knights Wish leg. Plain JSON in, the compiled rules'
// answer out: every timer, comparison, score and payout lives in
// deathmatch-rules.bend. Objects take the Bend constructor their key names;
// arrays are lists; nothing is computed here.
const bend = require('./deathmatch-rules.compiled.js')

const nil = { $: 'Nil' }
const cons = values => values.reduceRight((tail, head) => ({ $: 'Con', head, tail }), nil)

// The constructor each key's object (or each of its array's objects) is built as.
const shape = {
  rows: 'Seat', turn: 'Turn', turns: 'Turn', fighter: 'Body', hero: 'Kit', champ: 'Stance', blow: 'Blow', feed: 'Frag',
  struck: 'Struck', grab: 'Grab', spots: 'Spot', arm: 'Arm', acted: 'Acted', land: 'Land',
  sweep: 'Sweep', fields: 'Field', wrap: 'Wrap', muzzles: 'Muzzle', stakes: 'Stake',
}

// A field's `kind` is an enum Rust saves as its name.
const into = (value, key) => {
  if (key === 'kind' && typeof value === 'string') return { $: value }
  if (Array.isArray(value)) return cons(value.map(item => into(item, key)))
  if (value !== null && typeof value === 'object') {
    const out = { $: shape[key] }
    for (const [name, field] of Object.entries(value)) out[name] = into(field, name)
    return out
  }
  return value
}

const outof = value => {
  if (value === null || typeof value !== 'object') return typeof value === 'bigint' ? Number(value) : value
  if (value.$ === 'Nil') return []
  if (value.$ === 'Con') {
    const out = []
    for (let at = value; at.$ === 'Con'; at = at.tail) out.push(outof(at.head))
    return out
  }
  const names = Object.keys(value).filter(name => name !== '$')
  if (names.length === 0) return value.$
  const out = { $: value.$ }
  for (const name of names) out[name] = outof(value[name])
  return out
}

// The questions Rust asks, and the order their arguments go to the rules.
const asks = {
  consts: [],
  melee_tick: ['winner', 'connected'],
  contact_tier: ['damage'],
  sync: ['crossroads', 'winner', 'rows'],
  begin: ['winner', 'ended', 'connected', 'ticks', 'pickups'],
  turn: ['turn'],
  turns: ['turns'],
  spawn: ['clear'],
  hit: ['blow'],
  struck: ['struck'],
  grab: ['grab'],
  arm: ['arm'],
  armed_end: ['acted'],
  landed: ['land'],
  close: ['sweep'],
  wrap: ['wrap'],
}

module.exports = {
  apiVersion: 1,
  render(input = {}) {
    const names = asks[input.op]
    if (!names) throw Error('unknown knights wish question')
    return outof(bend[input.op](...names.map(name => into(input[name], name))))
  },
}
