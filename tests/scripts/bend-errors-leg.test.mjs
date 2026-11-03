import test from 'node:test'
import assert from 'node:assert/strict'
import * as fs from 'node:fs'
import * as path from 'node:path'
import * as os from 'node:os'
import vm from 'node:vm'
import {createHash} from 'node:crypto'
import {publish, compilerPath, sourceDigest} from '../../scripts/live/bend.mjs'
const shipped = new URL('../../cockpit/live/errors/', import.meta.url)
const hash = text => createHash('sha256').update(text).digest('hex')
const names = ['errors.bend', 'LAWS.bend', 'PROOF.bend']
function fixture(t) {
  const root = fs.mkdtempSync(path.join(os.tmpdir(), 'angel-errors-leg-test-'))
  t.after(() => fs.rmSync(root, {recursive: true, force: true}))
  fs.cpSync(shipped, root, {recursive: true})
  const bend = JSON.parse(fs.readFileSync(path.join(root, 'module.json'), 'utf8')).bend
  return {root, bend}
}
function adapter(root) {
  const context = vm.createContext({module: {exports: {}}})
  const require = name => {
    const sandbox = vm.createContext({module: {exports: {}}})
    vm.runInContext(fs.readFileSync(path.join(root, name.replace('./', '') + '.js'), 'utf8'), sandbox, {timeout: 200})
    return sandbox.module.exports
  }
  context.require = require
  vm.runInContext(fs.readFileSync(path.join(root, 'index.js'), 'utf8'), context, {timeout: 200})
  return context.module.exports
}
const facts = (over = {}) => ({kind: 'status', status: 503, veto: false, config: false, cert: false, auth: false, quota: false, rejected: false, context: false, answered: false, capped: false, attempt: 0, bounded: false, budget: 0, backoff_ms: 500, cap_ms: 2000, elapsed_ms: 0, ...over})
const compiler = fs.existsSync(compilerPath())

test('errors leg receipt matches its complete sources and emitted code', t => {
  const {root, bend} = fixture(t)
  const sources = Object.fromEntries(names.map(name => [name, fs.readFileSync(path.join(root, name), 'utf8')]))
  const receipt = JSON.parse(fs.readFileSync(path.join(root, 'bend-build.json'), 'utf8'))
  assert.equal(receipt.source_digest, sourceDigest(bend, sources))
  assert.equal(receipt.output_digest, hash(fs.readFileSync(path.join(root, bend.output), 'utf8')))
})
test('errors leg answers the shipped native ladder through its adapter', t => {
  const {root} = fixture(t)
  const adapted = adapter(root)
  const leg = {render: input => JSON.parse(JSON.stringify(adapted.render(input)))}
  assert.deepEqual(leg.render(facts()), {class: 'transient', retry: true, delay_ms: 500})
  assert.deepEqual(leg.render(facts({attempt: 5})), {class: 'transient', retry: true, delay_ms: 2000})
  assert.deepEqual(leg.render(facts({status: 401})), {class: 'permanent', retry: false, delay_ms: 0})
  assert.deepEqual(leg.render(facts({status: 429, quota: true})), {class: 'quota', retry: false, delay_ms: 0})
  assert.deepEqual(leg.render(facts({status: 413, context: true})), {class: 'context', retry: false, delay_ms: 0})
  assert.deepEqual(leg.render(facts({kind: 'transport', status: 0, cert: true})), {class: 'permanent', retry: false, delay_ms: 0})
  assert.deepEqual(leg.render(facts({bounded: true, budget: 3, attempt: 3})), {class: 'transient', retry: false, delay_ms: 0})
  assert.throws(() => leg.render(facts({attempt: -1})))
  assert.throws(() => leg.render(facts({kind: 'unheard-of'})))
  assert.throws(() => leg.render(facts({veto: 1})))
})
test('pinned compiler regenerates the errors artifact byte for byte', {skip: !compiler}, t => {
  const {root, bend} = fixture(t)
  const code = fs.readFileSync(path.join(root, bend.output), 'utf8')
  assert.equal(publish(root).code, code)
})
test('a leg edit that retries a rejected certificate is refused by its law and publishes nothing', {skip: !compiler}, t => {
  const {root, bend} = fixture(t)
  const source = fs.readFileSync(path.join(root, bend.entry), 'utf8')
  const accepted = fs.readFileSync(path.join(root, bend.output), 'utf8')
  const receipt = fs.readFileSync(path.join(root, 'bend-build.json'), 'utf8')
  assert.ok(source.includes('veto || config || cert || auth'))
  fs.writeFileSync(path.join(root, bend.entry), source.replace('veto || config || cert || auth', 'veto || config || auth'))
  assert.throws(() => publish(root), /SOME PROOFS FAIL/)
  assert.equal(fs.readFileSync(path.join(root, bend.output), 'utf8'), accepted)
  assert.equal(fs.readFileSync(path.join(root, 'bend-build.json'), 'utf8'), receipt)
  // A different pace is a legal edit: the laws are about replay, not timing.
  fs.writeFileSync(path.join(root, bend.entry), source.replace('(backoff * step : U32)', '(backoff : U32)'))
  const next = publish(root)
  assert.equal(adapter(root).render(facts({attempt: 1})).delay_ms, 500)
  assert.notEqual(next.code, accepted)
})
