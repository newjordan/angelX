// Thin adapter for the deep-cut episode reducer's Bend leg. Plain JSON in, the
// compiled leg's decision out: the append-only chain, the state arm, the
// sequence increment and the permits live in episode-reducer.bend. Rust owns
// the ids, the canonical hashes, the stored bodies, the durability authority and
// every error string, and translates the fault enum back to that string.
const bend = require('./episode-reducer.compiled.js')

const flag = value => value === true
const word = value => (Number.isFinite(value) ? Math.trunc(value) >>> 0 : 0)

const kind = value => {
  if (value === 'start') return { $: 'KStart' }
  if (value === 'terminal') return { $: 'KTerminal' }
  return { $: 'KEvidence' }
}

const state = value => ({
  $: 'St',
  has_episode: flag(value && value.has_episode),
  has_terminal: flag(value && value.has_terminal),
  seq_hi: word(value && value.seq_hi),
  seq_lo: word(value && value.seq_lo),
  journal_hi: word(value && value.journal_hi),
  journal_lo: word(value && value.journal_lo),
})

const event = value => ({
  $: 'Ev',
  kind: kind(value && value.kind),
  schema_ok: flag(value && value.schema_ok),
  sha_ok: flag(value && value.sha_ok),
  sequence_ok: flag(value && value.sequence_ok),
  previous_ok: flag(value && value.previous_ok),
  episode_ok: flag(value && value.episode_ok),
  start_episode_equal: flag(value && value.start_episode_equal),
  start_valid: flag(value && value.start_valid),
  link_valid: flag(value && value.link_valid),
  link_post_terminal: flag(value && value.link_post_terminal),
  terminal_valid: flag(value && value.terminal_valid),
  end_hi: word(value && value.end_hi),
  end_lo: word(value && value.end_lo),
})

const receipt = value => ({
  $: 'Rc',
  episode_equal: flag(value && value.episode_equal),
  seq_hi: word(value && value.seq_hi),
  seq_lo: word(value && value.seq_lo),
  sha_equal: flag(value && value.sha_equal),
  start_valid: flag(value && value.start_valid),
  completed_hi: word(value && value.completed_hi),
  completed_lo: word(value && value.completed_lo),
  started_hi: word(value && value.started_hi),
  started_lo: word(value && value.started_lo),
})

const faults = {
  NotAppendOnly: 'not_append_only',
  StartFirst: 'start_first',
  DuplicateStart: 'duplicate_start',
  TerminalEffects: 'terminal_effects',
  RangeBackward: 'range_backward',
  StartInvalid: 'start_invalid',
  LinkInvalid: 'link_invalid',
  TerminalInvalid: 'terminal_invalid',
  SequenceExhausted: 'sequence_exhausted',
  EpisodeClosed: 'episode_closed',
  NeedsStart: 'needs_start',
  NotOfficial: 'not_official',
  ReceiptMismatch: 'receipt_mismatch',
}

const phases = { PNone: 'none', PStarted: 'started', PClosed: 'closed', PCompleted: 'completed' }

const decision = value => {
  if (value.$ === 'Deny') {
    return {
      accept: false,
      fault: faults[value.fault.$] || 'not_append_only',
      phase: null,
      next_hi: 0,
      next_lo: 0,
      post_terminal: false,
    }
  }
  return {
    accept: true,
    fault: null,
    phase: phases[value.phase.$] || 'none',
    next_hi: Number(value.next_hi) >>> 0,
    next_lo: Number(value.next_lo) >>> 0,
    post_terminal: flag(value.post_terminal),
  }
}

module.exports = {
  apiVersion: 1,
  render(input = {}) {
    switch (input.op) {
      case 'append':
        return decision(bend.append(event(input.event)))
      case 'link':
        return decision(bend.link(state(input.state), event(input.event)))
      case 'complete':
        return decision(bend.complete(receipt(input.receipt)))
      case 'effect':
        return decision(bend.effect(state(input.state)))
      case 'evidence':
        return decision(bend.evidence(state(input.state)))
      case 'validate':
        return decision(bend.validate(state(input.state), event(input.event)))
      default:
        return decision(bend.durable(state(input.state), event(input.event)))
    }
  },
}
