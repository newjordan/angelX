// A failed provider request becomes {class, retry, delay_ms}. The host reduces
// the error to flags and numbers; errors.bend owns the policy.
const policy = require('./errors.compiled')
const SHAPES = { status: 'Status', transport: 'Transport', stream: 'Stream', empty: 'Empty', other: 'Other' }
const CLASSES = { Permanent: 'permanent', Transient: 'transient', Quota: 'quota', Context: 'context' }
const FLAGS = ['veto', 'config', 'cert', 'auth', 'quota', 'rejected', 'context', 'answered', 'capped', 'bounded']
const COUNTS = { status: 'status', attempt: 'attempt', budget: 'budget', backoff_ms: 'backoff', cap_ms: 'cap', elapsed_ms: 'elapsed' }
const flag = (input, key) => {
  if (typeof input[key] !== 'boolean') throw Error(`errors leg: ${key} must be a boolean`)
  return input[key]
}
const count = (input, key) => {
  const value = input[key]
  if (!Number.isInteger(value) || value < 0 || value > 0xffffffff) throw Error(`errors leg: ${key} must be a 32-bit count`)
  return value
}
module.exports = {
  apiVersion: 1,
  render(input) {
    const shape = SHAPES[input.kind]
    if (!shape) throw Error('errors leg: unknown error kind')
    const facts = { $: 'Facts', kind: { $: shape } }
    for (const key of FLAGS) facts[key] = flag(input, key)
    for (const [key, field] of Object.entries(COUNTS)) facts[field] = count(input, key)
    const verdict = policy.decide(facts)
    return { class: CLASSES[verdict.class.$], retry: verdict.retry, delay_ms: verdict.delay }
  }
}
