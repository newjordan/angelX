import test from 'node:test'
import assert from 'node:assert/strict'
import * as fs from 'node:fs'
import * as path from 'node:path'
import * as os from 'node:os'
import vm from 'node:vm'
import {createHash} from 'node:crypto'
import {build, publish, compilerPath, sourceDigest} from '../../scripts/live/bend.mjs'
const shipped = new URL('../../cockpit/live/ability-code/', import.meta.url)
const hash = text => createHash('sha256').update(text).digest('hex')
function fixture(t) {
  const root = fs.mkdtempSync(path.join(os.tmpdir(), 'angel-bend-test-'))
  t.after(() => fs.rmSync(root, {recursive:true,force:true}))
  fs.cpSync(shipped, root, {recursive:true})
  const bend = JSON.parse(fs.readFileSync(path.join(root,'module.json'),'utf8')).bend
  const sources = Object.fromEntries(['fairies.bend','LAWS.bend','PROOF.bend'].map(name=>[name,fs.readFileSync(path.join(root,name),'utf8')]))
  return {root,bend,sources}
}
function evaluate(code) {
  const context = vm.createContext({module:{exports:{}}})
  vm.runInContext(code,context,{timeout:200})
  return context.module.exports
}
const fresh = () => ({$:'Fresh'})
const event = (age=0, location='room') => ({$:'Tick',age,x:8,y:8,location})
const params = () => ({$:'Params',radius:3.5,duration:150,hold:12,pulse:12})
const compiler = fs.existsSync(compilerPath())

test('packaged Bend receipt matches its complete sources and emitted code', t => {
  const {root,bend,sources}=fixture(t)
  const receipt=JSON.parse(fs.readFileSync(path.join(root,'bend-build.json'),'utf8'))
  const code=fs.readFileSync(path.join(root,bend.output),'utf8')
  assert.equal(receipt.source_digest,sourceDigest(bend,sources))
  assert.equal(receipt.output_digest,hash(code))
  const api=evaluate(code)
  assert.equal(api.definition().name,'Fairy Stun Field')
  const first=api.tick(fresh(),event(),params())
  assert.equal(first.area.value.ticks,12)
  assert.equal(first.area.value.radius,3.5)
  assert.equal(api.tick(first.field,event(1),params()).area.$,'None')
  assert.equal(api.tick(first.field,event(12),params()).area.$,'Some')
  assert.equal(api.tick(first.field,event(150),params()).$,'End')
  assert.equal(api.tick(first.field,event(1,'other room'),params()).$,'End')
})
test('Bend build accepts only complete captured local proof graphs within budget', t => {
  const {bend,sources}=fixture(t)
  for (const mutate of [
    x=>{x.bend.entry='../outside.bend'},
    x=>{x.sources['fairies.bend']='import 0x123/remote.bend as Remote'},
    x=>{x.sources['fairies.bend']='import "' + './foreign.js"'},
    x=>{x.sources['fairies.bend']='import ./missing.bend as Missing'},
    x=>{x.sources['LAWS.bend']='import Base\n'},
    x=>{x.sources['PROOF.bend']='import Base\n'},
    x=>{x.sources['fairies.bend']+='\n#'+ 'x'.repeat(512*1024)},
  ]) {
    const request=structuredClone({schema:1,bend,sources});mutate(request)
    assert.throws(()=>build(request))
  }
})
test('real Bend replacement preserves the captured function and rejected proof publishes nothing', {skip:!compiler}, t => {
  const {root,bend,sources}=fixture(t)
  const old=evaluate(fs.readFileSync(path.join(root,bend.output),'utf8'))
  const before=old.tick(fresh(),event(),params())
  fs.writeFileSync(path.join(root,bend.entry),sources[bend.entry].replace('Params{3.5, 150, 12, 12}','Params{7.0, 210, 24, 12}'))
  const result=publish(root)
  assert.equal(result.ok,true)
  const next=evaluate(result.code)
  assert.equal(next.definition().parameters.radius,7)
  assert.equal(next.tick(fresh(),event(),next.definition().parameters).area.value.ticks,24)
  assert.equal(old.tick(before.field,event(12),params()).area.value.ticks,12)
  const accepted=fs.readFileSync(path.join(root,bend.output),'utf8')
  const receipt=fs.readFileSync(path.join(root,'bend-build.json'),'utf8')
  fs.writeFileSync(path.join(root,bend.entry),sources[bend.entry].replace('case False{} _:\n      End{}','case False{} _:\n      Keep{Planted{0.0, 0.0, "broken", 0}, False{}, None{}}'))
  assert.throws(()=>publish(root),/SOME PROOFS FAIL/)
  assert.equal(fs.readFileSync(path.join(root,bend.output),'utf8'),accepted)
  assert.equal(fs.readFileSync(path.join(root,'bend-build.json'),'utf8'),receipt)
})
test('pinned compiler regenerates the packaged runtime artifact byte for byte', {skip:!compiler}, t => {
  const {root}=fixture(t)
  const code=fs.readFileSync(path.join(root,'fairies.compiled.js'),'utf8')
  const result=publish(root)
  assert.equal(result.code,code)
})

const effortShipped = new URL('../../cockpit/live/effort/', import.meta.url)
const effortFixture = t => {
  const root = fs.mkdtempSync(path.join(os.tmpdir(), 'angel-effort-test-'))
  t.after(() => fs.rmSync(root, {recursive:true,force:true}))
  fs.cpSync(effortShipped, root, {recursive:true})
  return root
}
const some = value => ({$:'Some',value}), none = {$:'None'}
const list = items => items.reduceRight((tail,head) => ({$:'Con',head,tail}), {$:'Nil'})
const facts = {$:'Facts',seat:'muse-spark-1.3',hop:2,friction:{$:'True'},tool_errors:1,failed_checks:0,task_len:40,tests:{$:'True'},errors:{$:'False'},design:{$:'False'}}
const plain = value => JSON.parse(JSON.stringify(value))
const ask = (native, allowed) => ({$:'Ask',native,allowed:list(allowed),facts})
const levels = ['minimal','low','medium','high','xhigh']
const choose = value => `Facts{seat, hop, friction, tool_errors, failed_checks, task_len, tests, errors, design}}:\n      ${value}`

test('packaged effort leg matches its receipt and answers the native effort', t => {
  const root = effortFixture(t)
  const bend = JSON.parse(fs.readFileSync(path.join(root,'module.json'),'utf8')).bend
  const sources = Object.fromEntries(['effort.bend','LAWS.bend','PROOF.bend'].map(name=>[name,fs.readFileSync(path.join(root,name),'utf8')]))
  const receipt = JSON.parse(fs.readFileSync(path.join(root,'bend-build.json'),'utf8'))
  const code = fs.readFileSync(path.join(root,bend.output),'utf8')
  assert.equal(receipt.source_digest,sourceDigest(bend,sources))
  assert.equal(receipt.output_digest,hash(code))
  const api = evaluate(code)
  for (const native of [none, ...levels.map(some)]) for (const allowed of [[],['low','high'],levels]) {
    assert.deepEqual(plain(api.decide(ask(native,allowed))), native)
  }
})
test('an edited effort choice is guarded by the allowed levels and the proofs gate the guard', {skip:!compiler}, t => {
  const root = effortFixture(t)
  const source = fs.readFileSync(path.join(root,'effort.bend'),'utf8')
  const edit = value => { fs.writeFileSync(path.join(root,'effort.bend'),source.replace(choose('native'),choose(value))); return evaluate(publish(root).code) }
  const high = edit('Some{"high"}')
  assert.deepEqual(plain(high.decide(ask(some('low'),levels))), some('high'))
  assert.deepEqual(plain(high.decide(ask(some('low'),['low','medium']))), some('low'))
  assert.deepEqual(plain(high.decide(ask(none,['low','medium']))), none)
  assert.deepEqual(plain(edit('Some{"turbo"}').decide(ask(some('low'),levels))), some('low'))
  assert.deepEqual(plain(edit('None{}').decide(ask(some('low'),levels))), none)
  fs.writeFileSync(path.join(root,'effort.bend'),source.replace('guard(allowed, choose(Ask{native, allowed, facts}), native)','choose(Ask{native, allowed, facts})'))
  assert.throws(()=>publish(root),/SOME PROOFS FAIL/)
})
test('pinned compiler regenerates the packaged effort artifact byte for byte', {skip:!compiler}, t => {
  const root = effortFixture(t)
  const code = fs.readFileSync(path.join(root,'effort.compiled.js'),'utf8')
  assert.equal(publish(root).code,code)
})
