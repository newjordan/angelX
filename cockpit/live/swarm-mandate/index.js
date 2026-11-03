const leg = require('./swarm-mandate.compiled.js')

// Thin adapter: plain boolean facts in, the compiled leg's verdict out. Every
// text test (verdict upper-casing and substring tests, answer trim, the
// advisor's BLOCK:/NOTE: prefix parse) stays in Rust; no decision lives here.
const flag = value => value === true

module.exports = {
  apiVersion: 1,
  render(input = {}) {
    switch (input.kind) {
      case 'classify': {
        const out = leg.classify({
          $: 'Verdict',
          call_ok: flag(input.call_ok),
          saw_tight: flag(input.saw_tight),
          saw_open: flag(input.saw_open),
        })
        return { mandate: out.$ === 'Tight' ? 'tight' : 'open' }
      }
      case 'advise': {
        const out = leg.advise({
          $: 'Review',
          enabled: flag(input.enabled),
          blank: flag(input.blank),
          annotated: flag(input.annotated),
        })
        return { advise: out.advise }
      }
      case 'annotate': {
        const out = leg.annotate({
          $: 'Note',
          clear: flag(input.clear),
          has_annotation: flag(input.has_annotation),
        })
        return { annotate: out.annotate }
      }
      case 'pass': {
        const out = leg.pass({
          $: 'Pass',
          smart_route: flag(input.smart_route),
          research_route: flag(input.research_route),
          fastpath: flag(input.fastpath),
          tight: flag(input.tight),
          classify_open: flag(input.classify_open),
          deliberation: flag(input.deliberation),
          always: flag(input.always),
        })
        return { direct: out.direct }
      }
      case 'research_route': {
        const out = leg.research_route({
          $: 'Ask',
          wants_live: flag(input.wants_live),
          research_enabled: flag(input.research_enabled),
          researcher_present: flag(input.researcher_present),
        })
        return { research_route: out.research_route }
      }
      default:
        throw new Error(`swarm-mandate: unknown decision kind ${JSON.stringify(input.kind)}`)
    }
  },
}
