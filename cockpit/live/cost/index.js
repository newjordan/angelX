// Thin adapter for the `cost` leg. No decision logic lives here: the plain
// JSON input mirrors the Rust feature builder's fields 1:1, the compiled Bend
// decides every gate and all of the arithmetic, and the answer converts back
// to the leg's plain decision shape (a basis plus integer e8 money, so no
// float ever crosses the decision boundary).
const { 'Cost.decide': decide } = require('./cost.compiled')

const BASIS = Object.freeze({
  Fee: 'subscription-marginal',
  List: 'list-price',
  NoRow: 'no-price-row',
  Unreported: 'unreported-usage',
})

module.exports = {
  apiVersion: 1,
  render(input) {
    const decision = decide(
      input.subscription,
      input.has_usage,
      input.any_usage_field,
      input.row_exists,
      input.in_total,
      input.in_cached,
      input.in_uncached,
      input.has_in_total,
      input.has_cache_read,
      input.has_uncached,
      input.has_output,
      input.out_tokens,
      input.has_reasoning,
      input.reason_tokens,
      input.cached_le_input,
      input.reasoning_separate,
      input.rate_cached_p,
      input.rate_uncached_p,
      input.rate_output_p,
      input.rate_reasoning_p,
      input.rate_cached_cents,
      input.rate_uncached_cents,
      input.rate_output_cents,
      input.rate_reasoning_cents,
      input.fee_e8,
    )
    const basis = BASIS[decision.basis.$]
    if (!basis) throw Error(`cost leg returned an unknown basis: ${decision.basis.$}`)
    if (decision.$ === 'Blank') return { basis }
    return {
      basis,
      paid_e8: decision.paid_e8,
      cached_e8: decision.has_cached ? decision.cached_e8 : null,
      fee_e8: decision.fee_e8,
    }
  },
}
