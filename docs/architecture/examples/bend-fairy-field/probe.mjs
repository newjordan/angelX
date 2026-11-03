import assert from 'node:assert/strict';
import {readFileSync,writeFileSync,mkdtempSync,rmSync} from 'node:fs';
import {tmpdir} from 'node:os';
import {join,dirname} from 'node:path';
import {fileURLToPath} from 'node:url';
import {spawnSync} from 'node:child_process';
import {performance} from 'node:perf_hooks';
const compiler=process.env.BEND_COMPILER;
assert.ok(compiler, 'Set BEND_COMPILER to the absolute path of the pinned Bend 2 bend2/main.ts');
const base=dirname(fileURLToPath(import.meta.url));
const before=process.cwd();
const scratch=mkdtempSync(join(tmpdir(),'angelx-bend-field-'));
for(const name of ['field.bend','LAWS.bend','PROOF.bend']) writeFileSync(join(scratch,name),readFileSync(join(base,name)));
process.chdir(scratch);
try {
const source=readFileSync('field.bend','utf8');
const run=(args,expected=0)=>{
 const r=spawnSync(process.env.BUN ?? 'bun',[compiler,...args],{encoding:'utf8',env:{...process.env,BEND_NO_TELEMETRY:'1'}});
 assert.equal(r.status,expected,r.stdout+r.stderr);return r.stdout+r.stderr;
};
const load=async(path)=> (await import(`file://${process.cwd()}/${path}?v=${Date.now()}`)).default;
assert.match(run(['PROOF.bend','--check-only']),/ALL PROOFS CHECK/);
run(['field.bend','-o','old.mjs']);
const old=await load('old.mjs');
const captured=old.tick;
assert.equal(captured({$: 'Active',ticks:2,radius:4}).stun,24);
const activation=[];
let replacement;
for(let i=0;i<10;i++){
 const start=performance.now();
 writeFileSync('field.bend',source.replace(', radius, 24}',`, radius, ${36+i}}`));
 assert.match(run(['PROOF.bend','--check-only']),/ALL PROOFS CHECK/);
 run(['field.bend','-o',`next-${i}.mjs`]);
 replacement=await load(`next-${i}.mjs`);
 assert.equal(replacement.tick({$: 'Active',ticks:2,radius:4}).stun,36+i);
 activation.push(performance.now()-start);
}
assert.equal(captured({$: 'Active',ticks:1,radius:4}).stun,24);
assert.deepEqual(captured({$: 'Active',ticks:0,radius:4}),{$:'Quiet',next:{$:'Dormant'}});
writeFileSync('field.bend',source.replace('case Active{0n, radius}:\n      Quiet{Dormant{}}','case Active{0n, radius}:\n      Pulse{Dormant{}, radius, 99}'));
const broken=run(['PROOF.bend','--check-only'],1);
assert.match(broken,/SOME PROOFS FAIL/);
assert.equal(replacement.tick({$: 'Active',ticks:1,radius:4}).stun,45);
writeFileSync('field.bend',source);
const samples=[];let sum=0;
for(let i=0;i<101;i++){
 const start=performance.now();
 for(let n=0;n<10000;n++)sum+=captured({$:'Active',ticks:2,radius:4}).stun;
 if(i)samples.push((performance.now()-start)*1e6/10000);
}
const q=(xs,f)=>xs.sort((a,b)=>a-b)[Math.ceil(xs.length*f)-1];
console.log(JSON.stringify({compiler_commit:'221a350c2f7f7134884740018e6e90001fb6d230',compiler_version:'2.0.39',laws:'2 regular-checker PASS; broken expiry rule rejected; no --verdict claim',behavior:'old/new captured functions, zero-tick expiry, dormant law, invalid candidate retains accepted module',activation:'source write + checker startup + proof checking + compiler startup + JS emit + Node ES-module import; not AngelX activation',activation_ms:{p50:q(activation,.5),p95:q(activation,.95)},pure_tick_ns:{p50:q(samples,.5),p95:q(samples,.95)},output_bytes:readFileSync('old.mjs').length,pid:process.pid,sum},null,2));
} finally { process.chdir(before); rmSync(scratch,{recursive:true,force:true}); }
