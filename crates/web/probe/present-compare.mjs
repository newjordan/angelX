// The renderer module against the legacy presenter, on the same frames
// (docs/GLASS_CORE.md, step 1.3): headless Chromium over CDP, never a visible
// browser.
//
//   node crates/web/probe/present-compare.mjs <link> --dist crates/web/dist/delve --out <dir>
//
// <link> is a running host's page with its key (the Delve's dev host:
// `delve-dev-host --port 8820 --bundle crates/web/dist/delve --scene fight --key KEY`,
// then `http://127.0.0.1:8820/#KEY`).
//
// 1. Record. The page is opened with `?present=legacy`, a seat is taken, and the
//    stage frames the game makes (`angel_stage`, the bytes that go to the
//    presenter) are kept for --frames frames, from the first one (which carries
//    every picture). Then the page is opened again with `&stage=0`, which keeps
//    finished frames (the game draws no pixels of its own while it stages),
//    and one finished frame is kept.
// 2. Replay. A page of this script's own loads the shell's present.js, stage.js
//    and gpu.js and the bundle's renderer build, and draws every recorded
//    frame, in order and on the same clock, through the legacy presenter and
//    through the renderer, each on a canvas of its own. The two final images,
//    read from the canvases in the task that drew them, are compared pixel by
//    pixel. A game in motion cannot be compared with itself; the same bytes
//    drawn twice can.
//
// 3. Pages. The real page, with the real game, is loaded once per way of
//    drawing (--modes: `?present=legacy`, `gpu`, `webgl`), and the canvas is
//    read in the task of a frame (as above; headless Chromium's screenshot
//    leaves out a WebGPU canvas) after the same number of frames. A game in
//    motion differs from itself, so these are for looking at, and for the
//    number of pixels that differ; the replay is the exact comparison.
//    --skip-replay goes straight to this.
//
// For each canvas size (--sizes), each renderer build (--builds) and each of the
// stage frame and the finished frame it writes <out>/<case>-legacy.png, -gpu.png
// and -diff.png (the largest channel difference, times 8), and prints one JSON
// line of numbers. Node built-ins only.
import { spawn } from 'node:child_process';
import fs from 'node:fs';
import http from 'node:http';
import path from 'node:path';

const args = process.argv.slice(2);
const link = args[0] && !args[0].startsWith('--') ? args[0] : '';
const opt = (name, fallback) => {
  const i = args.indexOf('--' + name);
  return i >= 0 ? args[i + 1] : fallback;
};
if (!link || !opt('dist', '') || !opt('out', '')) {
  console.error('usage: node present-compare.mjs <link> --dist <bundle dir> --out <dir> [--frames 40] [--sizes 1152x672,1280x800] [--builds webgpu,webgl] [--modes legacy,gpu,webgl] [--skip-replay] [--cdp-port 8812] [--port 8813]');
  process.exit(2);
}
const dist = path.resolve(opt('dist', ''));
const out = path.resolve(opt('out', ''));
const frames = Number(opt('frames', 40));
const sizes = opt('sizes', '1152x672,1280x800,1000x600,500x300').split(',').map(s => s.split('x').map(Number));
const builds = opt('builds', 'webgpu,webgl').split(',');
const modes = opt('modes', 'legacy,gpu,webgl').split(',').filter(Boolean);
const skipReplay = args.includes('--skip-replay');
const CDP_PORT = Number(opt('cdp-port', 8812));
const PORT = Number(opt('port', 8813));
const sleep = ms => new Promise(r => setTimeout(r, ms));
fs.mkdirSync(out, { recursive: true });

// ---- the replay page ----

const REPLAY_HTML = `<!doctype html><meta charset="utf-8"><title>replay</title>
<style>html,body{margin:0;background:#000}canvas{display:block}</style>
<script src="/shell/stage.js"></script><script src="/shell/present.js"></script><script src="/shell/gpu.js"></script>
<script>
const getBytes = async url => new Uint8Array(await (await fetch(url)).arrayBuffer());
const frame = () => new Promise(r => requestAnimationFrame(r));
// A PNG data URL as pixels.
const decode = async url => {
  const bitmap = await createImageBitmap(await (await fetch(url)).blob());
  const o = document.createElement('canvas');
  o.width = bitmap.width; o.height = bitmap.height;
  const x = o.getContext('2d');
  x.drawImage(bitmap, 0, 0);
  return x.getImageData(0, 0, o.width, o.height);
};
const png = (w, h, rgba) => {
  const o = document.createElement('canvas');
  o.width = w; o.height = h;
  o.getContext('2d').putImageData(new ImageData(new Uint8ClampedArray(rgba), w, h), 0, 0);
  return o.toDataURL('image/png');
};
// The numbers for two screenshots, and the picture of where they differ.
async function compare(urlA, urlB) {
  const a = await decode(urlA), b = await decode(urlB);
  const n = a.width * a.height;
  const diff = new Uint8ClampedArray(n * 4);
  let differ = 0, over8 = 0, over32 = 0, max = 0, sum = 0, minX = 1e9, minY = 1e9, maxX = -1, maxY = -1;
  for (let i = 0; i < n; i++) {
    let m = 0;
    for (let k = 0; k < 3; k++) m = Math.max(m, Math.abs(a.data[i * 4 + k] - b.data[i * 4 + k]));
    if (m) {
      differ++;
      if (m > 8) over8++;
      if (m > 32) over32++;
      max = Math.max(max, m);
      sum += m;
      const x = i % a.width, y = (i / a.width) | 0;
      if (m > 8) { minX = Math.min(minX, x); maxX = Math.max(maxX, x); minY = Math.min(minY, y); maxY = Math.max(maxY, y); }
    }
    diff[i * 4] = diff[i * 4 + 1] = diff[i * 4 + 2] = Math.min(255, m * 8);
    diff[i * 4 + 3] = 255;
  }
  return {
    stats: { width: a.width, height: a.height, pixels: n, differ, over8, over32, max, mean: +(sum / n).toFixed(4), bigDiffBox: maxX < 0 ? null : [minX, minY, maxX, maxY] },
    diff: png(a.width, a.height, diff),
  };
}
let legacy, gpu, stages, fin, canvases, lost = null;
// Two canvases, one over the other, w x h CSS pixels each; the legacy
// presenter on the first, the renderer on the second.
async function setup(build, w, h) {
  document.body.textContent = '';
  const mk = () => { const c = document.createElement('canvas'); c.style.cssText = 'width:' + w + 'px;height:' + h + 'px'; document.body.appendChild(c); return c; };
  const cl = mk(), cg = mk();
  canvases = [cl, cg];
  legacy = makePresenter(cl, null);
  const mod = await loadRenderer({ js: '/shell/render-' + build + '.js', wasm: '/shell/render-' + build + '_bg.wasm' });
  gpu = await makeGpuPresenter(cg, mod, build);
  gpu.onlost = why => { lost = String(why); };
  for (let i = 0; i < 6; i++) await frame();
  const rec = await getBytes('/rec.bin');
  fin = await getBytes('/finished.bin');
  const view = new DataView(rec.buffer);
  stages = [];
  for (let at = 0; at < rec.length;) {
    const len = view.getUint32(at, true);
    stages.push(rec.subarray(at + 4, at + 4 + len));
    at += 4 + len;
  }
  return { kind: gpu.kind, adapter: gpu.adapter, legacyKind: legacy.kind, canvas: [cl.width, cl.height, cg.width, cg.height], frames: stages.length };
}
// Both canvases as PNGs. Read in the task of the draw: a canvas is only
// readable then (the legacy one is not kept, and headless Chromium's
// screenshot does not capture a WebGPU canvas).
const grab = () => canvases.map(c => c.toDataURL('image/png'));
// Every recorded stage frame, in order, on the same clock, through both.
async function drawStages() {
  let drawn = null, shots = null;
  for (let i = 0; i < stages.length; i++) {
    const t = 1000 + i * 16.7;
    drawn = [legacy.drawStage(stages[i], 0, stages[i].length, t), gpu.drawStage(stages[i], 0, stages[i].length, t)];
    if (i === stages.length - 1) shots = grab();
    await frame();
  }
  return { drawn, lost, shots };
}
// The finished frame: the game's BGRA words for the legacy presenter, the
// RGBA copy (the module's own swizzle) for the renderer.
async function drawFinished() {
  const dv = new DataView(fin.buffer);
  const fw = dv.getUint32(0, true), fh = dv.getUint32(4, true);
  const bgra = fin.subarray(8, 8 + fw * fh * 4);
  const rgba = new Uint8Array(bgra.length);
  for (let i = 0; i < bgra.length; i += 4) { rgba[i] = bgra[i + 2]; rgba[i + 1] = bgra[i + 1]; rgba[i + 2] = bgra[i]; rgba[i + 3] = bgra[i + 3]; }
  legacy.draw(bgra, 0, fw, fh);
  gpu.draw(rgba, 0, fw, fh);
  const shots = grab();
  await frame();
  return { size: [fw, fh], lost, shots };
}
</script>`;

// ---- the server: the replay page, the bundle's files, the recording ----

let rec = null, finished = null;
const server = http.createServer((req, res) => {
  const url = new URL(req.url, 'http://local');
  const cors = { 'Access-Control-Allow-Origin': '*' };
  if (req.method === 'POST') {
    const chunks = [];
    req.on('data', c => chunks.push(c));
    req.on('end', () => {
      const body = Buffer.concat(chunks);
      if (url.pathname === '/rec') rec = body;
      if (url.pathname === '/finished') finished = body;
      res.writeHead(200, cors);
      res.end('ok');
    });
    return;
  }
  const send = (type, body) => { res.writeHead(200, { ...cors, 'Content-Type': type }); res.end(body); };
  if (url.pathname === '/') return send('text/html; charset=utf-8', REPLAY_HTML);
  if (url.pathname === '/rec.bin' && rec) return send('application/octet-stream', rec);
  if (url.pathname === '/finished.bin' && finished) return send('application/octet-stream', finished);
  if (url.pathname.startsWith('/shell/')) {
    const file = path.join(dist, path.basename(url.pathname));
    if (fs.existsSync(file)) {
      const type = file.endsWith('.wasm') ? 'application/wasm' : file.endsWith('.js') ? 'text/javascript' : 'application/octet-stream';
      return send(type, fs.readFileSync(file));
    }
  }
  res.writeHead(404, cors);
  res.end();
});
await new Promise(resolve => server.listen(PORT, '127.0.0.1', resolve));

// ---- the browser ----

const profile = `${process.env.TMPDIR || '/work/tmp'}/angel-web-compare-${process.pid}`;
const proc = spawn('/usr/bin/chromium', ['--headless=new', `--remote-debugging-port=${CDP_PORT}`,
  `--user-data-dir=${profile}`, '--no-first-run', '--no-default-browser-check', '--window-size=1600,1000',
  // WebGPU and WebGL on the machine's GPU through Vulkan, as the probe's --webgpu.
  '--enable-unsafe-webgpu', '--enable-features=Vulkan', '--use-angle=vulkan', '--enable-gpu', '--ignore-gpu-blocklist',
  'about:blank'], { stdio: 'ignore' });
let closed = false;
const close = () => {
  if (closed) return;
  closed = true;
  try { proc.kill('SIGKILL'); } catch { /* gone */ }
  try { server.close(); } catch { /* gone */ }
  try { fs.rmSync(profile, { recursive: true, force: true }); } catch { /* held */ }
};
process.on('exit', close);
for (const sig of ['SIGINT', 'SIGTERM', 'SIGHUP']) process.on(sig, () => { close(); process.exit(1); });

let target;
for (let i = 0; i < 50 && !target; i++) {
  await sleep(200);
  try { target = (await (await fetch(`http://127.0.0.1:${CDP_PORT}/json/list`)).json()).find(t => t.type === 'page'); } catch { /* not up */ }
}
if (!target) { close(); throw new Error('chromium did not come up'); }
const ws = new WebSocket(target.webSocketDebuggerUrl);
await new Promise((resolve, reject) => { ws.onopen = resolve; ws.onerror = reject; });
let id = 0;
const waits = new Map();
ws.onmessage = ev => {
  const m = JSON.parse(ev.data);
  if (m.id && waits.has(m.id)) {
    waits.get(m.id)(m);
    waits.delete(m.id);
  } else if (m.method === 'Runtime.exceptionThrown') {
    console.error('page exception:', m.params.exceptionDetails.exception?.description || m.params.exceptionDetails.text);
  } else if (m.method === 'Runtime.consoleAPICalled') {
    console.error('page', m.params.type + ':', m.params.args.map(a => a.value ?? a.description).join(' '));
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

try {
  await send('Page.enable');
  await send('Runtime.enable');
  await send('Page.setBypassCSP', { enabled: true });
  await send('Emulation.setDeviceMetricsOverride', { width: 1600, height: 1000, deviceScaleFactor: 1, mobile: false });

  // Click Play as a player does, and wait for the seat.
  const play = async () => {
    await evaluate("document.getElementById('name').value = 'compare'");
    const at = await evaluate("(() => { const r = document.getElementById('play').getBoundingClientRect(); return [r.x + r.width / 2, r.y + r.height / 2]; })()");
    for (const type of ['mousePressed', 'mouseReleased']) {
      await send('Input.dispatchMouseEvent', { type, x: at[0], y: at[1], button: 'left', clickCount: 1 });
    }
  };

  if (!skipReplay) {
    // 1. Record.
    const url = new URL(link);
    url.searchParams.set('present', 'legacy');
    await send('Page.navigate', { url: url.toString() });
    await until("window.__angel && __angel.status !== 'loading'", 30000, 'the module');
    await evaluate(`(() => {
      window.__rec = [];
      __angel.onframe = (heap, ptr, w, h, n, staged, sp, sl) => {
        if (staged && __rec.length < ${frames}) __rec.push(heap.slice(sp, sp + sl));
      };
    })()`);
    await play();
    await until(`window.__rec && __rec.length >= ${frames}`, 30000, `${frames} stage frames`);
    console.log('recorded', JSON.stringify(await evaluate('({ presenter: __angel.presenter, frames: __rec.length, bytes: __rec.reduce((n, f) => n + f.length, 0), first: __rec[0].length })')));
    await evaluate(`(async () => {
      const parts = [];
      for (const f of __rec) { const h = new Uint8Array(4); new DataView(h.buffer).setUint32(0, f.length, true); parts.push(h, f); }
      await fetch('http://127.0.0.1:${PORT}/rec', { method: 'POST', body: new Blob(parts) });
    })()`);

    // The same game, finished frames: one of them.
    url.searchParams.set('stage', '0');
    await send('Page.navigate', { url: url.toString() });
    await until("window.__angel && __angel.status !== 'loading'", 30000, 'the module again');
    await evaluate(`(() => {
      window.__fin = null;
      __angel.onframe = (heap, ptr, w, h, n, staged) => {
        if (!staged && n > 90 && !__fin) __fin = { w, h, bgra: heap.slice(ptr, ptr + w * h * 4) };
      };
    })()`);
    await play();
    await until('window.__fin', 30000, 'a finished frame');
    console.log('finished frame', JSON.stringify(await evaluate('({ w: __fin.w, h: __fin.h, presenter: __angel.presenter })')));
    await evaluate(`(async () => {
      const h = new Uint8Array(8);
      new DataView(h.buffer).setUint32(0, __fin.w, true);
      new DataView(h.buffer).setUint32(4, __fin.h, true);
      await fetch('http://127.0.0.1:${PORT}/finished', { method: 'POST', body: new Blob([h, __fin.bgra]) });
    })()`);
    if (!rec || !finished) throw new Error('the recording did not arrive');

    // 2. Replay, per size and build.
    for (const build of builds) {
      for (const [w, h] of sizes) {
        await send('Emulation.setDeviceMetricsOverride', { width: w, height: 2 * h, deviceScaleFactor: 1, mobile: false });
        await send('Page.navigate', { url: `http://127.0.0.1:${PORT}/` });
        await sleep(400);
        await until("typeof setup === 'function'", 10000, 'the replay page');
        const name = `${build}-${w}x${h}`;
        const result = { case: name, ...(await evaluate(`setup(${JSON.stringify(build)}, ${w}, ${h})`)) };
        for (const part of ['stage', 'finished']) {
          const drew = await evaluate(part === 'stage' ? 'drawStages()' : 'drawFinished()');
          const [legacy, gpu] = drew.shots;
          delete drew.shots;
          result[part === 'stage' ? 'drawnStage' : 'finishedSize'] = drew;
          const compared = await evaluate(`compare(${JSON.stringify(legacy)}, ${JSON.stringify(gpu)})`);
          for (const [kind, url] of [['legacy', legacy], ['gpu', gpu], ['diff', compared.diff]]) {
            fs.writeFileSync(path.join(out, `${name}-${part}-${kind}.png`), Buffer.from(url.split(',')[1], 'base64'));
          }
          result[part] = compared.stats;
        }
        console.log(JSON.stringify(result));
      }
    }
  }

  // 3. The real page in each way of drawing.
  const pageShots = {};
  for (const mode of modes) {
    const page = new URL(link);
    page.searchParams.set('present', mode);
    await send('Emulation.setDeviceMetricsOverride', { width: 1152, height: 672, deviceScaleFactor: 1, mobile: false });
    await send('Page.navigate', { url: page.toString() });
    await until("window.__angel && __angel.status !== 'loading'", 30000, 'the module');
    await evaluate(`(() => {
      window.__shot = null;
      // In the task of a frame, after it was drawn: the canvas is readable only then.
      __angel.onframe = (heap, ptr, w, h, n) => {
        if (n > 120 && !__shot) __shot = document.getElementById('screen').toDataURL('image/png');
      };
    })()`);
    await play();
    await until('window.__shot', 30000, 'a frame');
    const info = await evaluate('({ presenter: __angel.presenter, canvas: [document.getElementById("screen").width, document.getElementById("screen").height], stats: __angel.stats() })');
    const shot = await evaluate('__shot');
    fs.writeFileSync(path.join(out, `page-${mode}.png`), Buffer.from(shot.split(',')[1], 'base64'));
    pageShots[mode] = shot;
    console.log(JSON.stringify({ page: mode, presenter: info.presenter, canvas: info.canvas, frames: info.stats.frames, fps: +info.stats.fps.toFixed(1), presentP50: +info.stats.presentP50.toFixed(3), presentP95: +info.stats.presentP95.toFixed(3), wasmP50: +info.stats.wasmP50.toFixed(3) }));
  }
  // Each against the first: where the pictures differ (a game in motion differs).
  const names = Object.keys(pageShots);
  if (names.length > 1) {
    await send('Page.navigate', { url: `http://127.0.0.1:${PORT}/` });
    await sleep(400);
    await until("typeof compare === 'function'", 10000, 'the replay page');
    for (const mode of names.slice(1)) {
      const compared = await evaluate(`compare(${JSON.stringify(pageShots[names[0]])}, ${JSON.stringify(pageShots[mode])})`);
      fs.writeFileSync(path.join(out, `page-${names[0]}-vs-${mode}-diff.png`), Buffer.from(compared.diff.split(',')[1], 'base64'));
      console.log(JSON.stringify({ pages: `${names[0]} vs ${mode}`, ...compared.stats }));
    }
  }
} finally {
  close();
}
process.exit(0);
