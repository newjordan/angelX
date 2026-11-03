// Thin adapter: plain JSON features in, plain JSON decision out. All
// decision logic lives in the compiled Bend module; an out-of-contract
// feature or result throws so the host falls back to the native leg.
const bend = require('./worker-health.compiled')

module.exports = {
  apiVersion: 1,
  render(input) {
    if (input.kind === 'recovery') {
      const step = bend.recovery(input.phase >>> 0)
      if (step.$ !== 'Step') throw Error('out of contract phase')
      return { state: step.state >>> 0, phase: step.phase >>> 0 }
    }
    if (input.kind === 'semantic') {
      const chosen = bend.semantic(input.progress === true, input.stale_next >>> 0)
      if (chosen.$ !== 'Choose') throw Error('out of contract staleness')
      return { state: chosen.state >>> 0, stale_next: chosen.stale_next >>> 0 }
    }
    throw Error('unknown decision kind')
  },
}
