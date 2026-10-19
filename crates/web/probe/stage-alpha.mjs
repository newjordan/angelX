// The browser seat's transparency contract, checked with a recording
// GL stub (the stage-fuzz pattern): the stage's own targets clear
// TRANSPARENT and hold premultiplied colour, exactly like the glass
// window's stage targets, and the blit onto the page must not
// premultiply a second time. The presenter must ask for an alpha,
// premultiplied canvas so the page can show through; the finished-
// frame path stays opaque by forcing alpha in its shader.
//
//   node crates/web/probe/stage-alpha.mjs
import { readFileSync } from 'node:fs';
import { fileURLToPath } from 'node:url';
import { dirname, join } from 'node:path';

const here = dirname(fileURLToPath(import.meta.url));
const stageSrc = readFileSync(join(here, '..', 'shell', 'stage.js'), 'utf8');
const presentSrc = readFileSync(join(here, '..', 'shell', 'present.js'), 'utf8');

let failed = 0;
const check = (name, ok) => {
  if (!ok) failed++;
  console.log(`${ok ? 'ok  ' : 'FAIL'} ${name}`);
};

// A GL stub that records the calls the alpha contract is about.
function recordingGl() {
  const calls = { clearColor: [], shaderSource: [], bufferData: [] };
  let next = 1;
  const gl = new Proxy({}, {
    get(_t, prop) {
      if (prop === 'getProgramParameter') return () => true;
      if (prop === 'getAttribLocation' || prop === 'getUniformLocation') return () => 0;
      if (prop === 'clearColor') return (...a) => calls.clearColor.push(a);
      if (prop === 'shaderSource') return (_s, src) => calls.shaderSource.push(src);
      if (prop === 'bufferData') return (_t2, data) => calls.bufferData.push(data);
      if (typeof prop === 'string' && /^[A-Z_0-9]+$/.test(prop)) return next++;
      return () => ({});
    },
  });
  return { gl, calls };
}

// --- the stage -------------------------------------------------------------
const { gl, calls } = recordingGl();
const makeStage = eval(stageSrc + '\nmakeStage;');
const stage = makeStage(gl, (w, h) => [0, 0, w, h], { w: 800, h: 600 });

// One valid frame: one 1x1 picture, one textured quad (as stage-fuzz).
const bytes = [];
const u32 = (v) => { const b = new Uint8Array(4); new DataView(b.buffer).setUint32(0, v, true); bytes.push(...b); };
const f32 = (v) => { const b = new Uint8Array(4); new DataView(b.buffer).setFloat32(0, v, true); bytes.push(...b); };
bytes.push(0x53, 0x54, 0x47, 0x32);
u32(384); u32(224); u32(1); u32(1); u32(9);
f32(NaN); f32(NaN); f32(0); f32(0); f32(0); f32(0); f32(0);
u32(5); u32(1); u32(1); bytes.push(1, 2, 3, 4);
for (let k = 0; k < 12; k++) f32(1);
f32(0); f32(2); f32(0); f32(0); u32(5); u32(0);
const frame = new Uint8Array(bytes);
check('valid frame draws', stage.draw(frame, 0, frame.length, 0) === true);

// The stage target and the page both clear transparent during a draw.
check('a transparent clear happens in draw',
  calls.clearColor.some(([r, g, b, a]) => r === 0 && g === 0 && b === 0 && a === 0));

// The blit is the last single-instance bufferData: its quad must take
// the passthrough path (info.x == 2), never the premultiplying one.
const single = calls.bufferData.filter((d) => d && d.length === 18);
check('blit quad uses the passthrough kind (info.x == 2)',
  single.length > 0 && single[single.length - 1][12] === 2);
check('fragment shader has a passthrough branch',
  calls.shaderSource.some((s) => s.includes('kind.x>1.5')));

// --- the presenter ---------------------------------------------------------
globalThis.ResizeObserver = class { observe() {} };
const contexts = [];
const canvas = {
  width: 0, height: 0, clientWidth: 800, clientHeight: 600,
  addEventListener() {},
  getContext(kind, opts) {
    contexts.push([kind, opts]);
    return recordingGl().gl;
  },
};
const makePresenter = eval(presentSrc + '\nmakePresenter;');
makePresenter(canvas);
const glOpts = (contexts.find(([k]) => k === 'webgl2') || [])[1] || {};
check('canvas asks for alpha', glOpts.alpha === true);
check('canvas is premultiplied', glOpts.premultipliedAlpha === true);
check('finished-frame shader forces opaque alpha',
  /vec4\(texture\(t,uv\)\.bgr,1\.0\)/.test(presentSrc));

console.log(failed ? `${failed} FAILED` : 'stage-alpha: all ok');
process.exit(failed ? 1 : 0);
