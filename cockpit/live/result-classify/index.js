// Thin adapter for the result-classify leg. The host reduces a tool result or a
// provider reply to the flags and the exit code below and checks whatever comes
// back; result-classify.bend owns the combination. No scanning lives here.
const leg = require('./result-classify.compiled')

const bool = (input, key) => {
  if (typeof input[key] !== 'boolean') throw Error(`result-classify leg: ${key} must be a boolean`)
  return input[key]
}

const count = input => {
  const value = input.exit
  if (!Number.isInteger(value) || value < 0 || value > 0xffffffff) throw Error('result-classify leg: exit must be a 32-bit code')
  return value >>> 0
}

module.exports = {
  apiVersion: 1,
  render(input) {
    const verdict = leg.decide({
      $: 'Facts',
      error: bool(input, 'error'),
      verifier: bool(input, 'verifier'),
      has_exit: bool(input, 'has_exit'),
      exit: count(input),
      empty_marker: bool(input, 'empty_marker'),
      club_stream: bool(input, 'club_stream'),
    })
    return {
      error: verdict.error,
      red_verifier: verdict.red_verifier,
      dispatch_failure: verdict.dispatch_failure,
      empty_reply: verdict.empty_reply,
    }
  },
}
