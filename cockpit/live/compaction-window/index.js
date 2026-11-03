// Thin adapter for the Bend compaction window. Plain JSON in, the compiled
// window out: no counting, clamping or pair rules live here. Rust decides which
// message is pinned, which one is a tool result and what each one costs in
// estimated tokens.
const { window } = require('./compaction-window.compiled.js')

const nil = { $: 'Nil' }
const flag = value => value === true
const count = value => (Number.isFinite(value) && value > 0 ? Math.trunc(value) : 0)
const messages = list => (Array.isArray(list) ? list : []).reduceRight(
  (tail, message) => ({
    $: 'Con',
    head: {
      $: 'Say',
      pinned: flag(message.pinned),
      tool: flag(message.tool),
      weight: count(message.weight),
    },
    tail,
  }),
  nil,
)

module.exports = {
  apiVersion: 1,
  render(input = {}) {
    const decision = window(messages(input.messages), count(input.keep_recent), count(input.keep_recent_tokens))
    return decision.$ === 'Chosen' ? { window: [decision.start, decision.end] } : { window: null }
  },
}
