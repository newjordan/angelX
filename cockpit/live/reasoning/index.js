const reasoning = require('./reasoning.compiled.js')

// The metered-input unit Rust counts milestones in. It crosses as whole
// millions because the leg's integers are U32; the adapter converts and the
// leg decides (exact for every turn under 4.29e15 metered input tokens).
const MILESTONE = 1_000_000

// `Bool` crosses this ABI as a plain JS boolean (the compiled code tests
// truthiness); only sum types such as `Maybe` carry a `$` tag.
const flag = on => on === true
const list = items => items.reduceRight((tail, head) => ({ $: 'Con', head, tail }), { $: 'Nil' })
const level = value => (value == null ? { $: 'None' } : { $: 'Some', value: String(value) })
const count = value => (Number.isFinite(value) && value > 0 ? Math.min(Math.trunc(value), 4294967295) : 0)
// Divide before narrowing: a u64 count keeps its whole millions.
const millions = value => count(Math.floor((Number.isFinite(value) && value > 0 ? value : 0) / MILESTONE))
const levels = items => list((items ?? []).map(String))
const answer = maybe => (maybe.$ === 'Some' ? maybe.value : null)

// Thin adapter: the caller names the decision, Bend decides, the answer comes
// back as plain JSON. No branch here chooses anything.
module.exports = {
  apiVersion: 1,
  render(input = {}) {
    switch (input.op) {
      case 'ladder':
        return {
          effort: answer(
            reasoning.ladder({
              $: 'Ladder',
              levels: levels(input.levels),
              effort: level(input.effort),
              hop: count(input.hop),
              friction: flag(input.friction === true),
            }),
          ),
        }
      case 'burn':
        return {
          effort: answer(
            reasoning.demote({
              $: 'Burn',
              levels: levels(input.levels),
              cumulative: count(input.cumulative),
              submitted: flag(input.submitted === true),
              limit: count(input.limit),
              locked: flag(input.locked === true),
            }),
          ),
        }
      case 'milestone': {
        const crossed = answer(
          reasoning.milestone({
            $: 'Crossing',
            previous_millions: millions(input.previous),
            current_millions: millions(input.current),
          }),
        )
        return { tokens: crossed == null ? null : crossed * MILESTONE }
      }
      case 'budget':
        return {
          expired:
            reasoning.expired({
              $: 'Budget',
              elapsed: count(input.elapsed),
              budget: count(input.budget),
            }) === true,
        }
      default:
        throw new Error(`reasoning leg: unknown decision ${input.op}`)
    }
  },
}
