import test from 'node:test'
import assert from 'node:assert/strict'
import * as fs from 'node:fs'
import vm from 'node:vm'

// The Moonfall adapter (cockpit/live/moonfall-rules/index.js) carries the run's phase across the
// hot boundary as one word: 0 Play, 1 Rewind, 2 Skip, 3 Won. The word must never turn into a win
// by default, in either direction.
const dir = new URL('../../cockpit/live/moonfall-rules/', import.meta.url)
const text = name => fs.readFileSync(new URL(name, dir), 'utf8')
const probe = JSON.parse(text('module.json')).probe

// the adapter, loaded the way the host loads it, over the real compiled rules
function load() {
  const bendContext = vm.createContext({module: {exports: {}}})
  vm.runInContext(text('moonfall-rules.compiled.js'), bendContext, {timeout: 5000})
  const bend = bendContext.module.exports
  const context = vm.createContext({module: {exports: {}}, Error, Object, Number, Math, JSON, Array})
  context.require = () => bend
  vm.runInContext(text('index.js'), context, {timeout: 5000})
  return {bend, adapter: context.module.exports}
}

// the session scalars are a tree of words, four children to a node; `phase` is the third word
const PHASE_AT = 2
const leaves = tree => (tree.$ === 'Lf' ? [tree] : [tree.a, tree.b, tree.c, tree.d].flatMap(leaves))
const phaseLeaf = tree => leaves(tree)[PHASE_AT]
// what the adapter returns was built inside the vm; compare it by value
const plain = value => JSON.parse(JSON.stringify(value))

// run one real step, with the phase word it writes back replaced by `word`
function stepWithPhaseWord(word, over = {}) {
  const {bend, adapter} = load()
  const step = bend.main_step
  bend.main_step = frame => {
    const out = step(frame)
    const leaf = phaseLeaf(out.s.w)
    leaf.x = word
    over.p_t !== undefined && (leaves(out.s.w)[3].x = over.p_t)
    over.p_from !== undefined && (leaves(out.s.w)[4].x = over.p_from)
    over.p_to !== undefined && (leaves(out.s.w)[5].x = over.p_to)
    over.p_fell !== undefined && (leaves(out.s.w)[6].x = over.p_fell)
    return out
  }
  return adapter.render(structuredClone(probe))
}

// the phase word the adapter hands the rules for a session in `phase`
function phaseWordSent(phase) {
  const {bend, adapter} = load()
  let sent
  const step = bend.main_step
  bend.main_step = frame => {
    sent = phaseLeaf(frame.w).x
    return step(frame)
  }
  const input = structuredClone(probe)
  input.session.phase = phase
  adapter.render(input)
  return sent
}

test('the adapter names each phase word it reads back', () => {
  assert.equal(stepWithPhaseWord(0).session.phase, 'Play')
  assert.deepEqual(plain(stepWithPhaseWord(1, {p_t: 7, p_from: 3, p_fell: 1}).session.phase), {Rewind: {t: 7, from: 3, fell: true}})
  assert.deepEqual(plain(stepWithPhaseWord(1, {p_t: 7, p_from: 3, p_fell: 0}).session.phase), {Rewind: {t: 7, from: 3, fell: false}})
  assert.deepEqual(plain(stepWithPhaseWord(2, {p_t: 9, p_from: 4, p_to: 8}).session.phase), {Skip: {t: 9, from: 4, to: 8}})
  assert.deepEqual(plain(stepWithPhaseWord(3, {p_t: 11}).session.phase), {Won: {t: 11}})
})

test('a phase word the rules never write is refused, not read as a win', () => {
  for (const word of [4, 5, 255, 0xffffffff, undefined]) {
    assert.throws(() => stepWithPhaseWord(word), /moonfall: unknown phase/, `word ${word}`)
  }
})

test('the adapter sends each phase as its word', () => {
  assert.equal(phaseWordSent('Play'), 0)
  assert.equal(phaseWordSent({Rewind: {t: 1, from: 2, fell: true}}), 1)
  assert.equal(phaseWordSent({Skip: {t: 1, from: 2, to: 3}}), 2)
  assert.equal(phaseWordSent({Won: {t: 1}}), 3)
})

test('a phase the adapter does not know is refused, not sent as a win', () => {
  assert.throws(() => phaseWordSent({Bogus: {t: 1}}), /moonfall: unknown phase/)
  assert.throws(() => phaseWordSent('Bogus'), /moonfall: unknown phase/)
})
