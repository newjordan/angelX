const { wants } = require('./swarm-route.compiled.js')

// The adapter only carries the feature record in and the decision out. Text
// work (trim, lowercase, word split, phrase windows) happens in Rust; every
// combination of those features lives in swarm-route.bend.
const flag = value => value === true
const count = value => (Number.isFinite(value) && value > 0 ? Math.trunc(value) : 0)

module.exports = {
  apiVersion: 1,
  render(features = {}) {
    const decision = wants({
      $: 'Feat',
      blank: flag(features.blank),
      stripped_blank: flag(features.stripped_blank),
      bytes: count(features.bytes),
      chars: count(features.chars),
      open_hits: count(features.open_hits),
      enum_lines: count(features.enum_lines),
      trivial: flag(features.trivial),
      moa_exact: flag(features.moa_exact),
      moa_prefix: flag(features.moa_prefix),
      open_phrase: flag(features.open_phrase),
      open_word: flag(features.open_word),
      action_word: flag(features.action_word),
      live_phrase: flag(features.live_phrase),
      live_word: flag(features.live_word),
    })
    return {
      tight: decision.tight,
      moa: decision.moa,
      synthesis: decision.synthesis,
      deliberation: decision.deliberation,
      research: decision.research,
      hard: decision.hard,
    }
  },
}
