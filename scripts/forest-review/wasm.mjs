// Forest Road native/wasm frame and simulation parity, with four seats.
import fs from 'node:fs';
const [file,nativePath,outPath]=process.argv.slice(2);
const native=JSON.parse(fs.readFileSync(nativePath,'utf8'));
const {instance:{exports:w}}=await WebAssembly.instantiate(fs.readFileSync(file),{});
const times=[],sums=[];
w.scene(37,0);w.party();const before=w.state_hash(0)>>>0;w.warm_entry(37);
for(let n=0;n<240;n++) {
  if(n>0&&n%2===0)w.step(1);
  const t=performance.now();sums.push(w.frame(384,224)>>>0);times.push(performance.now()-t);
}
const after=w.state_hash(0)>>>0;
if(before!==native.state_before||after!==native.state_after||JSON.stringify(sums)!==JSON.stringify(native.sums))throw new Error('Forest Road native/wasm parity mismatch');
times.sort((a,b)=>a-b);
const report={scene:'forest-road',seats:4,native_wasm_parity:true,state_before:before,state_after:after,frames:sums.length,wasm_median_ms:times[120],wasm_p95_ms:times[228]};
fs.writeFileSync(outPath,JSON.stringify(report,null,2)+'\n');console.log(report);
