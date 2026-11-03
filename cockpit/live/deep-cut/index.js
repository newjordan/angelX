// The deep-cut direction leg. Eligibility, scoring and the pick happen in
// deep-cut.bend; this adapter only translates the plain JSON contract, and the
// native allocator still validates whatever comes back.
const leg = require('./deep-cut.compiled')

const KINDS = {
  target_evidence: 'TargetEvidence',
  marked_provisional_transfer: 'MarkedProvisionalTransfer',
}
const OUTCOMES = {
  improvement: 'Improvement',
  tie: 'Tie',
  regression: 'Regression',
  failure: 'Failure',
  timeout: 'Timeout',
}
const BASIS = ['target_evidence', 'explicit_provisional_transfer', 'untried_fallback']

const list = values => values.reduceRight((tail, head) => ({ $: 'Con', head, tail }), { $: 'Nil' })

const use = value => ({
  $: 'Use',
  active: value.active === true,
  matches_target: value.matches_target === true,
  provisional: value.provisional === true,
  kind: { $: KINDS[value.kind] },
  outcome: { $: OUTCOMES[value.outcome] },
  cost_hi: Number(value.cost_hi) >>> 0,
  cost_lo: Number(value.cost_lo) >>> 0,
})

const direction = value => ({
  $: 'Direction',
  id: value.id,
  rank: Number(value.rank) >>> 0,
  uses: list((value.uses || []).map(use)),
})

module.exports = {
  apiVersion: 1,
  render({ directions = [], fallback_id = '' }) {
    const decision = leg.allocate({
      $: 'Offer',
      fallback_id,
      directions: list(directions.map(direction)),
    })
    return { selected_direction_id: decision.selected, basis: BASIS[decision.basis] }
  },
}
