const { decide } = require('./board-reducer.compiled')

// The adapter only carries the feature record in and the verdict out. The
// journal, the adapter calls, the canonical board, the raw hashes and the
// `Result` stay in Rust; the disposition order lives in board-reducer.bend.
const flag = value => value === true
const half = value => (Number.isFinite(value) && value > 0 ? Math.trunc(value) >>> 0 : 0)
const NAMES = {
  Conflict: 'conflict',
  Duplicate: 'duplicate',
  Stale: 'stale',
  Gap: 'gap',
  Replace: 'replace',
}

module.exports = {
  apiVersion: 1,
  render(features = {}) {
    const verdict = decide({
      $: 'Obs',
      identity_conflict: flag(features.identity_conflict),
      duplicate: flag(features.duplicate),
      full_snapshot: flag(features.full_snapshot),
      has_prior: flag(features.has_prior),
      prior_hi: half(features.prior_hi),
      prior_lo: half(features.prior_lo),
      has_incoming: flag(features.has_incoming),
      incoming_hi: half(features.incoming_hi),
      incoming_lo: half(features.incoming_lo),
      refresh_pending: flag(features.refresh_pending),
      has_board: flag(features.has_board),
    })
    return { decision: NAMES[verdict.$] }
  },
}
