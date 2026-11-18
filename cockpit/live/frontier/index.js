const { choose } = require('./frontier.compiled')

const nil = { $: 'Nil' }
const cons = values => values.reduceRight((tail, head) => ({ $: 'Con', head, tail }), nil)
const count = value => (Number.isFinite(value) && value >= 0 ? Math.trunc(value) : 0)
const flag = value => value === true
const rows = list => cons(list.map(row => ({
  $: 'Row',
  index: count(row.index),
  score: count(row.score),
  rank_hi: count(row.rank_hi),
  rank_lo: count(row.rank_lo),
  id: count(row.id),
  personal: flag(row.personal),
  source: flag(row.source),
})))
const picked = seat => (seat.$ === 'Seat' ? seat.index : null)

module.exports = {
  apiVersion: 1,
  render: input => {
    const choice = choose(rows((input && input.rows) || []))
    return {
      global: picked(choice.global),
      personal: picked(choice.personal),
      source: picked(choice.source),
    }
  },
}
