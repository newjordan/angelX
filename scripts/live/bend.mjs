// Bend source build adapter. The live host supplies an immutable source capture;
// compiler IO stays here, outside the V8/game fast path.
import {spawnSync} from 'node:child_process'
import {createHash} from 'node:crypto'
import * as fs from 'node:fs'
import * as path from 'node:path'
import * as os from 'node:os'
import {performance} from 'node:perf_hooks'
import {fileURLToPath} from 'node:url'

export const PIN = '221a350c2f7f7134884740018e6e90001fb6d230'
export const VERSION = '2.0.39'
const LIMIT = 512 * 1024
const local = name => typeof name === 'string' && name.length <= 256 && name.split('/').every(p => /^[A-Za-z0-9_-][A-Za-z0-9_.-]*$/.test(p) && p !== '..')
export const toolRoot = () => path.join(process.env.XDG_CACHE_HOME || path.join(os.homedir(), '.cache'), 'angel', 'bend2', PIN)
export const compilerPath = () => process.env.ANGEL_BEND_COMPILER || path.join(toolRoot(), 'bin', 'bend')
export function sourceDigest(config, sources) {
  const hash = createHash('sha256')
  const add = text => { const b = Buffer.from(text); const n = Buffer.alloc(8); n.writeBigUInt64LE(BigInt(b.length)); hash.update(n); hash.update(b) }
  for (const value of [config.entry, config.proof, config.output]) add(value)
  for (const name of Object.keys(sources).sort()) { add(name); add(sources[name]) }
  return hash.digest('hex')
}
const digest = text => createHash('sha256').update(text).digest('hex')
function invoke(compiler, args, cwd) {
  const typed = compiler.endsWith('.ts')
  const result = spawnSync(typed ? process.env.BUN || 'bun' : compiler, typed ? [compiler, ...args] : args, {
    cwd, encoding: 'utf8', timeout: 4000, maxBuffer: 1024 * 1024,
    env: {...process.env, BEND_NO_TELEMETRY: '1'},
  })
  const output = (result.stdout || '') + (result.stderr || '')
  if (result.error || result.status !== 0) throw Error((result.error?.message || output || `compiler exited ${result.status}`).slice(0, 6000))
  return output
}
export function build(request) {
  const start = performance.now()
  if (request.schema !== 1) throw Error('unsupported Bend build request')
  const {bend, sources} = request
  if (!bend || !['entry', 'proof', 'output'].every(key => local(bend[key])) || !bend.entry.endsWith('.bend') || !bend.proof.endsWith('.bend') || !bend.output.endsWith('.js')) throw Error('Bend build paths must be local source, proof and JS output files')
  if (!sources || !Object.hasOwn(sources, bend.entry) || !Object.hasOwn(sources, bend.proof) || Object.keys(sources).length > 64) throw Error('Bend source or proof missing')
  let bytes = 0
  const imports = {}
  for (const [name, text] of Object.entries(sources)) {
    if (!local(name) || !name.endsWith('.bend') || typeof text !== 'string') throw Error('Bend capture contains an invalid source')
    bytes += Buffer.byteLength(text)
    imports[name] = []
    // Pure modules import Base or captured relative Bend files. No hub lookup,
    // arbitrary foreign files, or changing source-directory reads during build.
    for (const line of text.split('\n')) {
      const match = line.match(/^\s*import\s+([^\s#]+)/)
      if (!match || match[1] === 'Base') continue
      const target = match[1]
      if (!target.startsWith('./') || !target.endsWith('.bend') || !local(target.slice(2))) throw Error(`unsupported Bend import in ${name}: ${target}`)
      const resolved = path.posix.join(path.posix.dirname(name), target.slice(2))
      if (!Object.hasOwn(sources, resolved)) throw Error(`uncaptured Bend import: ${resolved}`)
      imports[name].push(resolved)
    }
  }
  if (bytes > LIMIT) throw Error('Bend source exceeds 512 KiB')
  const reached = new Set()
  function reach(name) { if (reached.has(name)) return; reached.add(name); for (const child of imports[name]) reach(child) }
  reach(bend.proof)
  const laws = path.posix.join(path.posix.dirname(bend.proof), 'LAWS.bend')
  if (path.posix.basename(bend.proof) !== 'PROOF.bend' || !reached.has(laws) || !reached.has(bend.entry)) throw Error('Bend PROOF.bend must import its LAWS.bend and checked entry')
  const directory = fs.mkdtempSync(path.join(process.env.ANGEL_BEND_SCRATCH || os.tmpdir(), 'angel-bend-build-'))
  try {
    for (const [name, text] of Object.entries(sources)) { const file = path.join(directory, name); fs.mkdirSync(path.dirname(file), {recursive:true}); fs.writeFileSync(file, text) }
    const compiler = compilerPath()
    if (!fs.existsSync(compiler)) throw Error('Bend compiler unavailable; run node scripts/live/bend.mjs setup, then /reload')
    const version = invoke(compiler, ['version'], directory)
    if (version.trim() !== `bend ${VERSION}`) throw Error(`expected Bend ${VERSION}, observed ${version.trim()}`)
    const checked = performance.now()
    const proof = invoke(compiler, [bend.proof, '--check-only'], directory)
    if (!proof.includes('ALL PROOFS CHECK') || proof.includes('SOME PROOFS FAIL')) throw Error(proof.slice(0, 6000) || 'Bend proof check did not pass')
    const emitted = performance.now()
    invoke(compiler, [bend.entry, '-o', 'candidate.mjs'], directory)
    const module = fs.readFileSync(path.join(directory, 'candidate.mjs'), 'utf8')
    // A program with lambdas ends its code with their helpers, with no newline before the export.
    const marker = 'export default {\n'
    const at = module.lastIndexOf(marker)
    if (at < 0 || !module.endsWith('};\n')) throw Error('Bend emitted an incompatible module')
    const head = module.slice(0, at)
    const code = '// Generated by scripts/live/bend.mjs; edit the Bend sources.\n' + head + (head.endsWith('\n') ? '' : '\n') + 'module.exports = {\n' + module.slice(at + marker.length)
    if (Buffer.byteLength(code) > LIMIT) throw Error('Bend output exceeds 512 KiB')
    return {schema:1, ok:true, code, receipt:{schema:1, compiler_version:VERSION, compiler_pin:PIN, source_digest:sourceDigest(bend, sources), output_digest:digest(code)}, timing_ms:{check:emitted-checked, emit:performance.now()-emitted, total:performance.now()-start}}
  } finally { fs.rmSync(directory, {recursive:true,force:true}) }
}

export function publish(moduleRoot) {
  const manifest = JSON.parse(fs.readFileSync(path.join(moduleRoot, 'module.json'), 'utf8'))
  const sources = {}
  function walk(dir, prefix='') { for (const e of fs.readdirSync(dir,{withFileTypes:true})) { if(e.name.startsWith('.')) continue; const name=prefix+e.name; if(e.isDirectory())walk(path.join(dir,e.name),name+'/'); else if(e.isFile()&&name.endsWith('.bend'))sources[name]=fs.readFileSync(path.join(dir,e.name),'utf8') } }
  walk(moduleRoot)
  const result = build({schema:1,bend:manifest.bend,sources})
  for (const [name, body] of [[manifest.bend.output,result.code],['bend-build.json',JSON.stringify(result.receipt,null,2)+'\n']]) {
    const destination=path.join(moduleRoot,name); fs.mkdirSync(path.dirname(destination),{recursive:true}); const temp=destination+`.${process.pid}.tmp`; fs.writeFileSync(temp,body); fs.renameSync(temp,destination)
  }
  return result
}
export function setup(source) {
  const root = toolRoot()
  fs.mkdirSync(root, {recursive:true})
  const checkout = source || path.join(root,'source')
  const run = (program,args) => { const r=spawnSync(program,args,{encoding:'utf8',maxBuffer:1024*1024}); if(r.status!==0) throw Error(r.stderr||r.error?.message||'setup failed'); return r.stdout.trim() }
  if (!source && !fs.existsSync(path.join(checkout,'.git'))) {
    fs.mkdirSync(checkout,{recursive:true})
    run('git',['-C',checkout,'init'])
  }
  if (!source && spawnSync('git',['-C',checkout,'rev-parse','HEAD'],{encoding:'utf8'}).stdout?.trim() !== PIN) {
    run('git',['-C',checkout,'fetch','--depth','1','https://github.com/bendlang/bend.git',PIN])
    run('git',['-C',checkout,'checkout','--detach',PIN])
  }
  if(run('git',['-C',checkout,'rev-parse','HEAD'])!==PIN || run('git',['-C',checkout,'status','--porcelain','--untracked-files=no'])) throw Error('Bend setup requires the clean pinned compiler checkout')
  fs.mkdirSync(path.join(root,'bin'),{recursive:true})
  run(process.env.BUN || 'bun',['build','--compile',path.join(checkout,'bend2/main.ts'),'--outfile',path.join(root,'bin/bend')])
  for(const dir of ['bend2','guide']) fs.cpSync(path.join(checkout,dir),path.join(root,dir),{recursive:true,filter:file=>!file.endsWith('.ts')&&!file.endsWith('.lean')&&!file.includes('/pack/')})
  return path.join(root,'bin/bend')
}
if (process.argv[1] && path.resolve(process.argv[1]) === fileURLToPath(import.meta.url)) {
  try {
    const [action, value] = process.argv.slice(2)
    if(action==='setup') console.log(setup(value && path.resolve(value)))
    else if(action==='build') { const result=publish(path.resolve(value)); console.log(JSON.stringify({ok:result.ok,timing_ms:result.timing_ms,receipt:result.receipt})) }
    else throw Error('usage: node scripts/live/bend.mjs setup [pinned-checkout] | build <module-directory>')
  } catch(error) { console.error(error.message); process.exitCode=1 }
}
