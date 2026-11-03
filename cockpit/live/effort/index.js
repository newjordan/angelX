const effort = require('./effort.compiled')

// Emitted Bend reads a Bool as a JS boolean (as every other leg passes it);
// an object here would be truthy and read as True.
const flag = on => on === true
const list = items => items.reduceRight((tail, head) => ({ $: 'Con', head, tail }), { $: 'Nil' })
const mentions = (text, words) => words.some(word => text.includes(word))

// Thin adapter: Rust passes the request facts, Bend decides. Task-class
// features come from the head of the user's message, so their word lists edit
// here without a native build.
module.exports = {
  apiVersion: 1,
  render(input) {
    const task = String(input.task ?? '').toLowerCase()
    const answer = effort.decide({
      $: 'Ask',
      native: input.native == null ? { $: 'None' } : { $: 'Some', value: input.native },
      allowed: list(input.allowed ?? []),
      facts: {
        $: 'Facts',
        seat: String(input.seat ?? ''),
        hop: input.hop >>> 0,
        friction: flag(input.friction === true),
        tool_errors: input.tool_errors >>> 0,
        failed_checks: input.failed_checks >>> 0,
        task_len: input.task_len >>> 0,
        tests: flag(mentions(task, ['test', 'spec', 'assert'])),
        errors: flag(mentions(task, ['error', 'panic', 'traceback', 'exception', 'fails', 'failing', 'crash'])),
        design: flag(mentions(task, ['design', 'architect', 'refactor', 'plan'])),
        red: flag(input.red === true),
        stuck: flag(input.stuck === true),
      },
    })
    return { effort: answer.$ === 'Some' ? answer.value : null }
  },
}
