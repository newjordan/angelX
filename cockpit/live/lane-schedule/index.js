// Thin adapter for the Bend lane scheduler: plain JSON lane states in, the
// chosen lane indices out. Seat counts, service gaps, ages, runnable flags and
// the last single-seat lane travel as Bend numbers and constructors; every
// ordering rule lives in lane-schedule.bend.
const { schedule } = require('./lane-schedule.compiled.js')

const nil = { $: 'Nil' }
const cons = (head, tail) => ({ $: 'Con', head, tail })
const list = values => values.reduceRight((tail, head) => cons(head, tail), nil)
const count = value => (Number.isFinite(value) && value > 0 ? Math.trunc(value) : 0)
const lane = state => ({
  $: 'Lane',
  index: count(state.index),
  runnable: state.runnable === true,
  age: count(state.age_ticks),
})
// `null` is the oracle's `None`: no lane was served last tick.
const lastLane = value => (Number.isFinite(value) ? { $: 'Some', value: count(value) } : { $: 'None' })
// The leg answers with a Bend list of Lane records; unwind it back to JSON.
const indices = chosen => {
  const lanes = []
  for (let cell = chosen; cell.$ === 'Con'; cell = cell.tail) lanes.push(cell.head.index)
  return lanes
}

module.exports = {
  apiVersion: 1,
  render(input = {}) {
    const lanes = Array.isArray(input.lanes) ? input.lanes : []
    const chosen = schedule(
      list(lanes.map(lane)),
      count(input.seats),
      count(input.max_service_gap_ticks),
      input.frontier_priority_boost === true,
      lastLane(input.last_single_lane),
    )
    return { lanes: indices(chosen) }
  },
}
