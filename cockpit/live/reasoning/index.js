const reasoning = require('./reasoning.compiled.js')

const MILESTONE = 1_000_000

const flag = on => on === true
const list = items => items.reduceRight((tail, head) => ({ $: 'Con', head, tail }), { $: 'Nil' })
const level = value => (value == null ? { $: 'None' } : { $: 'Some', value: String(value) })
const count = value => (Number.isFinite(value) && value > 0 ? Math.min(Math.trunc(value), 4294967295) : 0)
const millions = value => count(Math.floor((Number.isFinite(value) && value > 0 ? value : 0) / MILESTONE))
const levels = items => list((items ?? []).map(String))
const answer = maybe => (maybe.$ === 'Some' ? maybe.value : null)

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
