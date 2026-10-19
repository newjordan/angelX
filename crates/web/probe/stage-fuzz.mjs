// The wire decoder's fuzz check: stage.js reads frames from a socket,
// so every byte is untrusted. Mutations of a valid frame (and pure
// garbage) must fail closed — draw() returns false or draws, never
// throws — and structurally lying headers must be refused.
//
//   node crates/web/probe/stage-fuzz.mjs
import { readFileSync } from 'node:fs';
import { fileURLToPath } from 'node:url';
import { dirname, join } from 'node:path';

const here = dirname(fileURLToPath(import.meta.url));
const src = readFileSync(join(here, '..', 'shell', 'stage.js'), 'utf8');
const makeStage = eval(src + '\nmakeStage;');

let next = 1;
const gl = new Proxy({}, {
  get(_t, prop) {
    if (prop === 'getProgramParameter') return () => true;
    if (prop === 'getAttribLocation') return () => 0;
    if (typeof prop === 'string' && /^[A-Z_0-9]+$/.test(prop)) return next++;
    return () => ({});
  },
});
const stage = makeStage(gl, (w, h) => [0, 0, w, h], { w: 800, h: 600 });

function frame({ w = 384, h = 224, pics = [[5, 1, 1, [1, 2, 3, 4]]], nquads = 1, truncate = 0, npicsOverride = null, nquadsOverride = null }) {
  const bytes = [];
  const u32 = (v) => { const b = new Uint8Array(4); new DataView(b.buffer).setUint32(0, v, true); bytes.push(...b); };
  const f32 = (v) => { const b = new Uint8Array(4); new DataView(b.buffer).setFloat32(0, v, true); bytes.push(...b); };
  bytes.push(0x53, 0x54, 0x47, 0x32);
  u32(w); u32(h); u32(npicsOverride ?? pics.length); u32(nquadsOverride ?? nquads); u32(9);
  f32(NaN); f32(NaN);
  f32(w / 2); f32(h * 0.85); f32(0.3 / h); f32(1.0); f32(0.925);
  for (const [id, pw, ph, rgba] of pics) { u32(id); u32(pw); u32(ph); bytes.push(...rgba); }
  for (let i = 0; i < nquads; i++) { for (let k = 0; k < 12; k++) f32(1); f32(0.3); f32(i % 3); f32(5); f32(9); u32(5); u32(0); }
  let out = new Uint8Array(bytes);
  if (truncate) out = out.subarray(0, out.length - truncate);
  return out;
}

let failed = 0;
const check = (name, bytes, want) => {
  let got, threw = null;
  try { got = stage.draw(bytes, 0, bytes.length, 0); } catch (e) { threw = e; }
  const ok = !threw && got === want;
  if (!ok) failed++;
  console.log(`${ok ? 'ok  ' : 'FAIL'} ${name}: got=${got} want=${want}${threw ? ' threw ' + threw.message : ''}`);
};

check('valid frame draws', frame({}), true);
check('tiny buffer', new Uint8Array(10), false);
check('bad tag', (() => { const f = frame({}); f[0] = 0; return f; })(), false);
check('zero stage width', frame({ w: 0 }), false);
check('oversized stage', frame({ w: 9000 }), false);
check('oversized picture', frame({ pics: [[5, 9000, 1, new Array(36000).fill(0)]] }), false);
check('picture bytes truncated', frame({ truncate: 74 }), false);
check('quad bytes truncated', frame({ truncate: 10 }), false);
check('npics lies high', frame({ npicsOverride: 5 }), false);
check('nquads lies high', frame({ nquadsOverride: 100000 }), false);
check('zero-size picture', frame({ pics: [[5, 0, 1, []]] }), false);

// Mutations: any bit of a valid frame may flip on the wire. Whatever
// still parses must draw; whatever does not must refuse. Never throw.
const valid = frame({ pics: [[5, 2, 2, [...Array(16).keys()]]], nquads: 2 });
let state = 0x2545f491;
const rnd = () => { state = (Math.imul(state, 1103515245) + 12345) >>> 0; return state / 4294967296; };
let drew = 0, refused = 0;
for (let i = 0; i < 3000 && !failed; i++) {
  const buf = new Uint8Array(valid);
  const flips = 1 + Math.floor(rnd() * 8);
  for (let f = 0; f < flips; f++) buf[Math.floor(rnd() * buf.length)] = Math.floor(rnd() * 256);
  const len = rnd() < 0.25 ? Math.floor(rnd() * buf.length) : buf.length;
  let got;
  try { got = stage.draw(buf, 0, len, 0); } catch (e) {
    console.log(`FAIL mutation ${i} threw: ${e.message}`); failed++; break;
  }
  if (typeof got !== 'boolean') { console.log(`FAIL mutation ${i}: non-boolean ${got}`); failed++; break; }
  got ? drew++ : refused++;
}
for (let i = 0; i < 500 && !failed; i++) {
  const buf = new Uint8Array(Math.floor(rnd() * 400));
  for (let j = 0; j < buf.length; j++) buf[j] = Math.floor(rnd() * 256);
  try { stage.draw(buf, 0, buf.length, 0); } catch (e) {
    console.log(`FAIL garbage ${i} threw: ${e.message}`); failed++; break;
  }
}
if (!failed) console.log(`ok   3000 mutated frames (drew=${drew} refused=${refused}) + 500 garbage buffers: no throws`);
process.exit(failed ? 1 : 0);
