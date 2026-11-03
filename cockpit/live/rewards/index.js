// Thin adapter for the reward ledger's Bend leg. Plain JSON in, the compiled
// leg's decision out: the row flags, the revisions and the order the ledger
// scans them live in rewards.bend. Rust owns the ids, receipts, scores,
// canonical decimal arithmetic, binding digests, validation and storage.
const bend = require('./rewards.compiled.js')

const nil = { $: 'Nil' }
const cons = values => values.reduceRight((tail, head) => ({ $: 'Con', head, tail }), nil)
const flags = values => cons((values || []).map(value => value === true))
const flag = value => value === true
const word = value => (Number.isFinite(value) ? Math.trunc(value) >>> 0 : 0)

const shapingRows = rows => cons((rows || []).map(row => ({
  $: 'ShapingRow',
  id_equal: flag(row.id_equal),
  entry_equal: flag(row.entry_equal),
})))

const bindingRows = rows => cons((rows || []).map(row => ({
  $: 'BindingRow',
  result_id_equal: flag(row.result_id_equal),
  entry_equal: flag(row.entry_equal),
  provenance_equal: flag(row.provenance_equal),
  same_event: flag(row.same_event),
  episode_equal: flag(row.episode_equal),
  identity_equal: flag(row.identity_equal),
  correction_targets: flag(row.correction_targets),
  revision_hi: word(row.revision_hi),
  revision_lo: word(row.revision_lo),
})))

const incoming = value => ({
  $: 'Incoming',
  has_correction: flag(value && value.has_correction),
  incoming_hi: word(value && value.revision_hi),
  incoming_lo: word(value && value.revision_lo),
})

const selection = choice => {
  if (choice.$ === 'Official') return { kind: 'official', index: Number(choice.index) }
  if (choice.$ === 'Provisional') return { kind: 'provisional', index: Number(choice.index) }
  return { kind: 'absent', index: null }
}

const shaping = decision => {
  if (decision.$ === 'Append') return { outcome: 'append', index: null }
  if (decision.$ === 'Duplicate') return { outcome: 'duplicate', index: Number(decision.index) }
  return { outcome: 'conflict', index: Number(decision.index) }
}

const binding = decision => {
  switch (decision.$) {
    case 'Appended':
      return { outcome: 'append', index: null }
    case 'AlreadyBound':
      return { outcome: 'duplicate', index: Number(decision.index) }
    case 'ConflictId':
      return { outcome: 'conflict-id', index: Number(decision.index) }
    case 'ConflictReceipt':
      return { outcome: 'conflict-receipt', index: Number(decision.index) }
    case 'RejectedFirstCorrection':
      return { outcome: 'reject-first-correction', index: null }
    case 'RejectedCorrection':
      return { outcome: 'reject-correction', index: Number(decision.index) }
    default:
      throw Error(`rewards leg: unknown binding decision ${decision.$}`)
  }
}

module.exports = {
  apiVersion: 1,
  render: input => {
    const op = input && input.op
    if (op === 'select') return selection(bend.select(flags(input.bindings), flags(input.shaping)))
    if (op === 'record_shaping') return shaping(bend.record_shaping(shapingRows(input.rows)))
    if (op === 'bind_official') {
      return binding(bend.bind_official(bindingRows(input.rows), incoming(input.incoming)))
    }
    throw Error(`rewards leg: unknown op ${op}`)
  },
}
