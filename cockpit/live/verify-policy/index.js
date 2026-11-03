// Thin adapter: the host sends plain JSON features, the checked Bend module
// decides, and the decision returns as plain JSON. No policy lives here.
const bend = require('./verify-policy.compiled.js')

const TOOLS = { shell: 'Shell', cargo: 'Cargo', fmt: 'Fmt', check: 'Check' }
const VERBS = { test: 'Test', check: 'CheckVerb', clippy: 'Clippy', fmt: 'FmtVerb', build: 'Build' }

const segment = s => (s.verifier === true
  ? (s.build === true ? { $: 'RunsBoth' } : { $: 'RunsVerifier' })
  : (s.build === true ? { $: 'RunsBuild' } : { $: 'RunsNothing' }))

const segments = list => (list || []).reduceRight(
  (tail, s) => ({ $: 'Con', head: segment(s), tail }),
  { $: 'Nil' }
)

module.exports = {
  apiVersion: 1,
  render: input => {
    const decision = bend.decide({
      $: 'Features',
      tool: { $: TOOLS[input.tool] || 'Other' },
      fmt_check: input.fmt_check === true,
      verb: { $: VERBS[input.verb] || 'NoVerb' },
      check_flag: input.check_flag === true,
      mutation: input.mutation === true,
      targets: (input.targets | 0) >>> 0,
      code_targets: (input.code_targets | 0) >>> 0,
      or_branch: input.or_branch === true,
      async_control: input.async_control === true,
      status: segments(input.status),
      anywhere: segments(input.anywhere),
    })
    return {
      requires_verification: decision.requires_verification === true,
      is_verification: decision.is_verification === true,
      is_progress_verifier: decision.is_progress_verifier === true,
      runs_tests_anywhere: decision.runs_tests_anywhere === true,
    }
  },
}
