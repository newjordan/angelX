// Thin adapter for the competition-mode leg. The host hands over the argv words
// it already classified, the phrase flags it already matched and the `git show`
// facts; competition-mode.bend decides, and the decision returns as plain JSON
// with no policy in this file.
const bend = require('./competition-mode.compiled.js')

const nil = { $: 'Nil' }
const cons = values => values.reduceRight((tail, head) => ({ $: 'Con', head, tail }), nil)
const flags = values => cons((values || []).map(value => value === true))

const TOKENS = { ls: 'Ls', word: 'Word', recursive: 'Recursive', plain: 'Plain' }
const tokens = list => cons((list || []).map(token => ({
  $: TOKENS[(token && token.kind) || 'plain'],
})))

const FIRST = { inventory: 'InventoryWord', git: 'GitWord', other: 'OtherWord' }
const first = value => ({ $: FIRST[value] || 'OtherWord' })

const git = value => ({
  $: 'Git',
  has_git: !!value && value.has_git === true,
  has_show: !!value && value.has_show === true,
  list_flag: !!value && value.list_flag === true,
  pathspec: !!value && value.pathspec === true,
  blob: !!value && value.blob === true,
})

const TRIGGERS = { Env: 'env', Phrase: 'phrase', Goal: 'goal', NoTrigger: 'none' }

module.exports = {
  apiVersion: 1,
  render: input => {
    const op = input && input.op
    if (op === 'trigger') {
      const decision = bend.trigger(
        input.env === true,
        flags(input.phrases),
        input.goal_present === true,
        flags(input.goal_phrases),
      )
      const kind = TRIGGERS[decision.$]
      if (!kind) throw Error(`competition-mode leg: unknown trigger ${decision.$}`)
      return { trigger: kind, index: decision.$ === 'Phrase' ? Number(decision.index) >>> 0 : null }
    }
    if (op === 'ls_recursive') return { hit: bend.recursive_of(tokens(input.tokens)).$ === 'Hit' }
    if (op === 'git_show') return { hit: bend.verdict(bend.git_show(git(input.git))).$ === 'Hit' }
    if (op === 'shell_thrash') {
      const hit = bend.shell_thrash(
        input.thrash_hit === true,
        tokens(input.ls),
        git(input.git),
        first(input.first),
        flags(input.git_args),
      )
      return { hit: bend.verdict(hit).$ === 'Hit' }
    }
    throw Error(`competition-mode leg: unknown op ${op}`)
  },
}
