// Which way the page draws, and what it does when a way fails
// (docs/GLASS_CORE.md, step 1.3): headless Chromium over CDP, never a visible
// browser. It joins a running host's page in each of these conditions and
// checks the presenter the page ends up with, and what it said in the console:
//
//   node crates/web/probe/present-fallback.mjs <link> [--plain-link <link>]
//
// <link> is a host serving a bundle built with the renderer (`WASM_BINDGEN=...
// crates/web/build.sh web/delve delve`, then `delve-dev-host --bundle ...`),
// with its key; --plain-link one serving a bundle built without it (no
// wasm-bindgen at hand), for the cases that need none. Prints one line per case
// and exits 1 if any failed. Node built-ins only.
import { spawn } from 'node:child_process';
import fs from 'node:fs';

const args = process.argv.slice(2);
const link = args[0] && !args[0].startsWith('--') ? args[0] : '';
const opt = (name, fallback) => {
  const i = args.indexOf('--' + name);
  return i >= 0 ? args[i + 1] : fallback;
};
const plainLink = opt('plain-link', '');
const PORT = Number(opt('cdp-port', 8814));
if (!link) {
  console.error('usage: node present-fallback.mjs <link> [--plain-link <link>] [--cdp-port 8814]');
  process.exit(2);
}
const sleep = ms => new Promise(r => setTimeout(r, ms));

const profile = `${process.env.TMPDIR || '/work/tmp'}/angel-web-fallback-${process.pid}`;
const proc = spawn('/usr/bin/chromium', ['--headless=new', `--remote-debugging-port=${PORT}`,
  `--user-data-dir=${profile}`, '--no-first-run', '--no-default-browser-check', '--window-size=1280,800',
  '--enable-unsafe-webgpu', '--enable-features=Vulkan', '--use-angle=vulkan', '--enable-gpu', '--ignore-gpu-blocklist',
  'about:blank'], { stdio: 'ignore' });
let closed = false;
const close = () => {
  if (closed) return;
  closed = true;
  try { proc.kill('SIGKILL'); } catch { /* gone */ }
  try { fs.rmSync(profile, { recursive: true, force: true }); } catch { /* held */ }
};
process.on('exit', close);
for (const sig of ['SIGINT', 'SIGTERM', 'SIGHUP']) process.on(sig, () => { close(); process.exit(1); });

let target;
for (let i = 0; i < 50 && !target; i++) {
  await sleep(200);
  try { target = (await (await fetch(`http://127.0.0.1:${PORT}/json/list`)).json()).find(t => t.type === 'page'); } catch { /* not up */ }
}
if (!target) { close(); throw new Error('chromium did not come up'); }
const ws = new WebSocket(target.webSocketDebuggerUrl);
await new Promise((resolve, reject) => { ws.onopen = resolve; ws.onerror = reject; });
let id = 0, console_ = [];
const waits = new Map();
ws.onmessage = ev => {
  const m = JSON.parse(ev.data);
  if (m.id && waits.has(m.id)) {
    waits.get(m.id)(m);
    waits.delete(m.id);
  } else if (m.method === 'Runtime.consoleAPICalled') {
    console_.push(`${m.params.type}: ${m.params.args.map(a => a.value ?? a.description).join(' ')}`);
  } else if (m.method === 'Runtime.exceptionThrown') {
    console_.push('exception: ' + (m.params.exceptionDetails.exception?.description || m.params.exceptionDetails.text));
  }
};
const send = (method, params = {}) => new Promise((resolve, reject) => {
  const i = ++id;
  waits.set(i, m => (m.error ? reject(new Error(`${method}: ${m.error.message}`)) : resolve(m.result)));
  ws.send(JSON.stringify({ id: i, method, params }));
});
const evaluate = async expr => {
  const r = await send('Runtime.evaluate', { expression: expr, returnByValue: true, awaitPromise: true });
  if (r.exceptionDetails) throw new Error('page: ' + JSON.stringify(r.exceptionDetails.exception?.description || r.exceptionDetails.text));
  return r.result.value;
};
const until = async (expr, ms, what) => {
  const end = Date.now() + ms;
  while (Date.now() < end) {
    if (await evaluate(expr)) return;
    await sleep(50);
  }
  throw new Error('timed out waiting for ' + what);
};

// Join `url` with `init` run first in the page (to break something), and let
// `act` do what it will once the game is playing. Returns what the page says.
let visits = 0;
async function visit(url, init, act) {
  console_ = [];
  // A link that differs only in its #key would be a jump, not a load.
  url = withParam(url, 'case', String(++visits));
  const script = init ? (await send('Page.addScriptToEvaluateOnNewDocument', { source: init })).identifier : null;
  try {
    await send('Page.navigate', { url });
    await until("window.__angel && __angel.status !== 'loading'", 30000, 'the module');
    if ((await evaluate('__angel.status')) === 'ready') {
      await evaluate("document.getElementById('name').value = 'fallback'");
      const at = await evaluate("(() => { const r = document.getElementById('play').getBoundingClientRect(); return [r.x + r.width / 2, r.y + r.height / 2]; })()");
      for (const type of ['mousePressed', 'mouseReleased']) {
        await send('Input.dispatchMouseEvent', { type, x: at[0], y: at[1], button: 'left', clickCount: 1 });
      }
    }
    await until("__angel.status === 'playing' && __angel.frames > 30", 20000, 'a seat and some frames');
    if (act) await act();
    await sleep(400);
    const framesA = await evaluate('__angel.frames');
    await sleep(500);
    // What the screen shows: the canvas read in the task of a frame (the only
    // time a WebGPU or WebGL canvas can be), then counted for lit pixels.
    await evaluate("window.__shot = null; { const keep = __angel.onframe; __angel.onframe = (...a) => { __shot = document.getElementById('screen').toDataURL('image/png'); __angel.onframe = keep; }; }");
    await until('window.__shot !== null', 5000, 'a frame to read');
    const lit = await evaluate(`(async () => {
      // The page's CSP allows data: images, not fetches of them.
      const image = new Image();
      image.src = __shot;
      await image.decode();
      const x = document.createElement('canvas');
      x.width = image.width; x.height = image.height;
      const g = x.getContext('2d');
      g.drawImage(image, 0, 0);
      const d = g.getImageData(0, 0, x.width, x.height).data;
      let lit = 0;
      for (let i = 0; i < d.length; i += 40) if (d[i] + d[i + 1] + d[i + 2] > 60) lit++;
      return lit;
    })()`);
    return {
      presenter: await evaluate('__angel.presenter'),
      moving: (await evaluate('__angel.frames')) > framesA,
      lit,
      canvases: await evaluate("document.querySelectorAll('canvas').length"),
      console: console_.filter(l => /angelX|exception|error/i.test(l)),
    };
  } finally {
    if (script) await send('Page.removeScriptToEvaluateOnNewDocument', { identifier: script });
  }
}

const withParam = (u, k, v) => { const x = new URL(u); if (v !== null) x.searchParams.set(k, v); return x.toString(); };
const dropGpu = "Object.defineProperty(navigator, 'gpu', { value: undefined, configurable: true });";
const noAdapter = "navigator.gpu.requestAdapter = async () => null;";
const keepDevices = "{ const was = GPUAdapter.prototype.requestDevice; GPUAdapter.prototype.requestDevice = async function (...a) { const d = await was.apply(this, a); (window.__devices ||= []).push(d); return d; }; }";

let failed = 0;
const check = (name, r, want) => {
  const problems = [];
  if (!r.moving) problems.push('the frame count did not move');
  if (!(r.lit > 20)) problems.push(`the canvas looks empty (${r.lit} lit samples)`);
  if (want.presenter && !want.presenter.test(r.presenter)) problems.push(`presenter ${r.presenter}, wanted ${want.presenter}`);
  for (const [re, n] of want.say || []) {
    const got = r.console.filter(l => re.test(l)).length;
    if (got !== n) problems.push(`console lines matching ${re}: ${got}, wanted ${n}`);
  }
  if (r.console.some(l => l.startsWith('exception'))) problems.push('the page threw');
  if (want.canvases && r.canvases !== want.canvases) problems.push(`${r.canvases} canvases, wanted ${want.canvases}`);
  if (problems.length) failed++;
  console.log(`${problems.length ? 'FAIL' : 'ok  '} ${name}: ${r.presenter}${problems.length ? ' | ' + problems.join('; ') : ''}`);
  if (problems.length || process.env.VERBOSE) console.log('     console:', JSON.stringify(r.console));
};

try {
  await send('Page.enable');
  await send('Runtime.enable');
  await send('Emulation.setDeviceMetricsOverride', { width: 1280, height: 800, deviceScaleFactor: 1, mobile: false });

  // A browser that has just started may have no GPU process yet, and its first
  // requestAdapter answers none: draw with present.js once first.
  await visit(withParam(link, 'present', 'legacy'));
  console_ = [];
  check('navigator.gpu there: the WebGPU renderer', await visit(link), { presenter: /^gpu-webgpu$/, say: [[/drawing with the webgpu renderer/, 1]] });
  check('?present=gpu', await visit(withParam(link, 'present', 'gpu')), { presenter: /^gpu-webgpu$/ });
  check('?present=webgl: the WebGL2 build, even where WebGPU is', await visit(withParam(link, 'present', 'webgl')), { presenter: /^gpu-webgl$/, say: [[/drawing with the webgl renderer/, 1]] });
  check('?present=legacy: present.js, no renderer', await visit(withParam(link, 'present', 'legacy')), { presenter: /^webgl2$/, say: [[/angelX/, 0]] });
  check('?present=2d: present.js, the 2D canvas', await visit(withParam(link, 'present', '2d')), { presenter: /^canvas2d$/, say: [[/angelX/, 0]] });
  check('no navigator.gpu: the WebGL2 build', await visit(link, dropGpu), { presenter: /^gpu-webgl$/ });
  check('an adapter that is not there: present.js, said once', await visit(link, noAdapter), { presenter: /^webgl2$/, say: [[/did not start/, 1]], canvases: 1 });
  check('a lost device: present.js from then on, said once', await visit(link, keepDevices, async () => {
    await evaluate('(window.__devices || []).forEach(d => d.destroy())');
  }), { presenter: /^webgl2$/, say: [[/renderer stopped/, 1], [/drawing with the webgpu renderer/, 1]], canvases: 1 });
  if (plainLink) {
    check('a bundle with no renderer: present.js, silently', await visit(plainLink), { presenter: /^webgl2$/, say: [[/angelX/, 0]] });
    check('a bundle with no renderer, ?present=gpu: present.js, said', await visit(withParam(plainLink, 'present', 'gpu')), { presenter: /^webgl2$/, say: [[/has no webgpu renderer/, 1]] });
  }
} finally {
  close();
}
process.exit(failed ? 1 : 0);
