// Thin adapter for the Bone Dominoes leg. Plain JSON in, the compiled rule's
// next state and ordered effects out: every timer, branch, score and effect
// lives in ossuary.bend. Rust owns the Run, the geometry (who stands where),
// the sound names, the banner words and the monster bodies, and applies the
// effects in the order they arrive.
const bend = require('./ossuary.compiled.js')

const flag = value => value === true
const word = value => (Number.isFinite(value) ? Math.trunc(value) >>> 0 : 0)
const nil = { $: 'Nil' }
const cons = values => values.reduceRight((tail, head) => ({ $: 'Con', head, tail }), nil)
const array = node => {
  const out = []
  for (let at = node; at.$ === 'Con'; at = at.tail) out.push(at.head)
  return out
}

const state = value => ({
  $: 'Line',
  running: flag(value && value.running),
  done: flag(value && value.done),
  fallen: word(value && value.fallen),
  next: word(value && value.next),
  stalled: word(value && value.stalled),
  raised: word(value && value.raised),
  score: cons(((value && value.score) || []).map(([id, n]) => ({ $: 'Mark', id: word(id), n: word(n) }))),
})

const knight = value => ({
  $: 'Knight',
  id: word(value && value.id),
  pressed: flag(value && value.pressed),
  at_start: flag(value && value.at_start),
  at_bone: flag(value && value.at_bone),
})

const cues = { Clatter: 'clatter', Fall: 'fall', Toll: 'toll', Chest: 'chest' }
const beasts = { Skeleton: 'skeleton' }
const said = value => (value.$ === 'Opens' ? { said: 'opens', gold: Number(value.gold) >>> 0 } : { said: value.$.toLowerCase() })

const effect = value => {
  switch (value.$) {
    case 'Sound': return { k: 'sound', cue: cues[value.cue.$] }
    case 'Say': return { k: 'say', who: Number(value.who) >>> 0, ...said(value.said) }
    case 'Raise': return { k: 'raise', beast: beasts[value.beast.$], x: value.x, y: value.y }
    case 'Grant': return { k: 'grant', id: Number(value.id) >>> 0, gold: Number(value.gold) >>> 0 }
    case 'Crown': return { k: 'crown', id: Number(value.id) >>> 0 }
    default: throw Error('unknown ossuary effect')
  }
}

module.exports = {
  apiVersion: 1,
  render(input = {}) {
    const step = bend.tick(state(input.state), cons((input.knights || []).map(knight)))
    const line = step.state
    return {
      state: {
        running: line.running,
        done: line.done,
        fallen: Number(line.fallen) >>> 0,
        next: Number(line.next) >>> 0,
        stalled: Number(line.stalled) >>> 0,
        raised: Number(line.raised) >>> 0,
        score: array(line.score).map(mark => [Number(mark.id) >>> 0, Number(mark.n) >>> 0]),
      },
      effects: array(step.effects).map(effect),
    }
  },
}
