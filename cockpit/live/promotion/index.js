// Thin adapter for the Bend promotion gate. Plain JSON in, the compiled gate's
// verdict out: no ordering, thresholds or arithmetic live here. Scores travel
// as exact f32 values and the margin travels back as raw f32 bits.
const gate = require('./promotion.compiled')

const nil = { $: 'Nil' }
const cons = values => values.reduceRight((tail, head) => ({ $: 'Con', head, tail }), nil)
const samples = values => cons(values.map(value => Math.fround(value)))
const cohort = cases => cons(cases.map(item => ({
  $: 'Sample',
  complete: item.complete,
  incumbent: samples(item.incumbent),
  candidate: samples(item.candidate),
})))

const verdict = result => result.$ === 'Scored'
  ? { decision: result.decision, mean_delta_bits: result.mean_delta_bits, delta_lower_bound_bits: result.lower_bound_bits }
  : { decision: result.decision, mean_delta_bits: null, delta_lower_bound_bits: null }

module.exports = {
  apiVersion: 1,
  render: input => verdict(gate.judge(
    cohort(input.cases),
    input.case_count,
    input.min_cases,
    input.samples_per_case,
    Math.fround(input.confidence_z),
    Math.fround(input.absolute_floor),
    Math.fround(input.max_case_regression),
    Math.fround(input.min_mean_delta)
  )),
}
