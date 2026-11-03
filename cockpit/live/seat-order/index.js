const seat = require('./seat-order.compiled.js')

// Thin adapter: plain JSON in, the compiled decision out. Rust has already done
// the text work (alias/label/model equality against the allowlist and against
// the cheap order names) and the env reads; nothing here orders, filters or
// compares anything.
const nil = { $: 'Nil' }
const cons = (values, shape) => values.reduceRight(
  (tail, value) => ({ $: 'Con', head: shape(value), tail }),
  nil,
)
const count = value => (Number.isFinite(value) && value > 0 ? Math.trunc(value) : 0)
const flag = value => value === true

const link = wire => ({
  $: 'Lnk',
  index: count(wire.index),
  group: count(wire.group),
  allowed: flag(wire.allowed),
  matches: cons(Array.isArray(wire.matches) ? wire.matches : [], count),
})

const chain = wire => ({
  $: 'Chn',
  index: count(wire.index),
  gated: flag(wire.gated),
  gate_open: flag(wire.gate_open),
  has_metadata: flag(wire.has_metadata),
  supports_tools: flag(wire.supports_tools),
  context_window: count(wire.context_window),
})

const plan = result => {
  const ids = []
  for (let node = result.ids; node.$ === 'Con'; node = node.tail) ids.push(node.head)
  return { plan: ids }
}

module.exports = {
  apiVersion: 1,
  render(input = {}) {
    switch (input.mode) {
      case 'order':
        return plan(seat.order(cons(input.order || [], count), cons(input.links || [], link)))
      case 'walk':
        return plan(seat.walk(
          flag(input.chat),
          flag(input.has_tools),
          count(input.required_tokens),
          cons(input.chain || [], chain),
        ))
      case 'advance': {
        const step = seat.advance(
          count(input.step),
          count(input.plan_len),
          flag(input.streaming),
          flag(input.emitted),
          flag(input.cancelled),
        )
        return { proceed: step.proceed, next: step.next, abort: step.abort }
      }
      default:
        throw new Error(`seat-order: unknown decision mode ${JSON.stringify(input.mode)}`)
    }
  },
}
