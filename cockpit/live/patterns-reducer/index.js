const { reduce } = require('./patterns-reducer.compiled.js')

// Thin adapter for the pattern reducer's Bend leg. Plain JSON in, the compiled
// fold out. Rust owns every id and every ordering; this file only turns the
// plan into Bend constructors and the leg's counters back into plain halves.
const nil = { $: 'Nil' }
const cons = values => values.reduceRight((tail, head) => ({ $: 'Con', head, tail }), nil)
const flag = value => value === true
const word = value => (Number.isFinite(value) && value > 0 ? Math.trunc(value) >>> 0 : 0)
const list = value => (Array.isArray(value) ? value : [])

const evidence = rows => cons(list(rows).map(row => ({
  $: 'Ev',
  kind: word(row.kind),
  outcome: word(row.outcome),
  uncertainty: word(row.uncertainty),
  cost_hi: word(row.cost_hi),
  cost_lo: word(row.cost_lo),
})))

const patterns = rows => cons(list(rows).map(row => ({
  $: 'Pat',
  evidence: evidence(row.evidence),
})))

const buckets = rows => cons(list(rows).map(row => ({
  $: 'Bucket',
  patterns: patterns(row.patterns),
})))

const count = value => ({ hi: word(value.hi), lo: word(value.lo), over: flag(value.over) })

const aggregate = value => ({
  confirmed_official: count(value.confirmed_official),
  provisional_transfer: count(value.provisional_transfer),
  operational: count(value.operational),
  improvements: count(value.improvements),
  ties: count(value.ties),
  regressions: count(value.regressions),
  failures: count(value.failures),
  timeouts: count(value.timeouts),
  uncertainty_millis: count(value.uncertainty_millis),
  cost_microunits: count(value.cost_microunits),
})

const cells = (head, walk) => {
  const out = []
  for (let cell = head; cell.$ === 'Con'; cell = cell.tail) out.push(walk(cell.head))
  return out
}

module.exports = {
  apiVersion: 1,
  render(input = {}) {
    const reduced = reduce(buckets(input.buckets))
    return {
      buckets: cells(reduced, bucket => ({
        patterns: cells(bucket.patterns, value => ({ aggregate: aggregate(value) })),
      })),
    }
  },
}
