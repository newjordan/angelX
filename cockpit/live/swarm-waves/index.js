const leg = require('./swarm-waves.compiled.js')

// Thin adapter: the host resolves every float comparison natively (no f64 in
// Bend), so the wave leg receives the stop predicates as booleans and the
// grown wave size as a precomputed integer; swarm-waves.bend owns the stop
// order, the expand choice and every integer clamp.
const count = value => (Number.isInteger(value) && value >= 0 && value <= 0xffffffff ? value : 0)
const flag = value => value === true
const REASONS = {
  MaxWidth: 'max_width',
  MaxWaves: 'max_waves',
  Failed: 'failed',
  LowNovelty: 'low_novelty',
  Agree: 'agree',
  Stable: 'stable',
}
const COUNTS = ['wave_index', 'max_waves', 'launched_total', 'max_width', 'returned', 'room', 'next_raw']

module.exports = {
  apiVersion: 1,
  render(input) {
    if (input.kind === 'gate') {
      const gate = leg.gate_of(flag(input.relax), flag(input.escalate))
      return { gate: gate.$ === 'Relax' ? 'relax' : gate.$ === 'Escalate' ? 'escalate' : 'hold' }
    }
    const facts = {
      $: 'In',
      novel_low: flag(input.novel_low),
      dissent_agrees: flag(input.dissent_agrees),
      dissent_stable: flag(input.dissent_stable),
    }
    for (const key of COUNTS) facts[key] = count(input[key])
    const wave = leg.wave_decide(facts)
    return wave.$ === 'Stop'
      ? { stop: REASONS[wave.reason.$] }
      : { expand: { next_wave: wave.next_wave } }
  },
}
