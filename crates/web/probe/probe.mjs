// The browser runtime's probe: drives the real shell page in headless
// Chromium over CDP and measures what a player gets.
//
//   node crates/web/probe/probe.mjs <link> [options]
//
//   --runs desktop,retina,laptop4x   which runs (default: all three)
//   --seconds 8                      steady play measured per run
//   --trials 12                      key-to-moved latency trials per run
//   --settle 1.5                     seconds seated before measuring (to
//                                    let a crowd join around this seat)
//   --track color|scroll|own         how the own character is followed:
//                                    by its colour (a fixed camera), by
//                                    the ground scrolling (a camera that
//                                    follows it, as the muster's does), or
//                                    where the game says it drew it
//                                    (`__angel.own()`, the Delve), counted
//                                    as moved only once the pixels there
//                                    changed too
//   --own ffd23f                     the own character's colour, for color
//                                    (exact RGB; the testgame's gold dot)
//   --roll                           walk east and west with a dodge roll
//                                    (Space) each way, and record where the
//                                    own character was drawn on every frame
//                                    (`__angel.own()`): frames it stood still
//                                    or stepped back with a key held, and
//                                    its biggest step against its usual;
//                                    with --shots, a strip across a roll
//   --fire                           fire trials (the Delve): standing, and
//                                    running east or west, press an arrow
//                                    (aim and fire) and find the first frame
//                                    that draws an own shot (`__angel.game()`
//                                    own_shots); report the press to it in
//                                    ms and frames, and how far from the
//                                    drawn knight it showed, in frame
//                                    pixels; with --shots, the frames of
//                                    one running trial from the press on
//   --fire-trials 12                 how many
//   --reconnect                      cut the network and check the seat
//   --cuts 3,8                       how long each cut is, seconds
//   --software                       WebGL on SwiftShader instead of the GPU
//   --webgpu                         WebGPU on the machine's GPU through Vulkan
//                                    (headless Chromium needs these flags; the
//                                    page is then asked for the renderer with
//                                    ?present=gpu, or picks it by itself)
//   --hide                           hide the tab 3 s with a key held
//   --shots <dir>                    save a screenshot per run
//   --json <file>                    write every number
//   --cdp-port 8808                  Chromium's debugging port
//   --no-use                         steady play only walks (no E presses:
//                                    in the Delve's kingdom E goes through
//                                    a door)
//
// The muster (its camera follows the own knight), among a crowd:
//
//   muster-host --port 8800 --bundle crates/web/dist/muster --key <KEY> &
//   angel-bots --url ws://127.0.0.1:8800/ws --join <KEY> --n 200 --secs 600 --stand --delay 6 &
//   node crates/web/probe/probe.mjs 'http://127.0.0.1:8800/#<KEY>' \
//     --track scroll --settle 10 --hide --reconnect
//
// The test game without a host (probe/serve.mjs on 8801):
//
//   node crates/web/probe/probe.mjs 'http://127.0.0.1:8801/?offline=40'
//
// Per run: fps, dropped frames, the module's frame ms p50/p95, present ms,
// load time, input-to-own-character-moved in display frames, and (with
// --reconnect) whether the same seat came back. Node built-ins only. It
// launches its own Chromium on port 8808 with a throwaway profile.
import { spawn } from 'node:child_process';
import fs from 'node:fs';

const RUNS = {
  // A desktop browser on a 1x screen.
  desktop: { width: 1280, height: 800, dpr: 1, throttle: 1 },
  // A Mac's retina screen.
  retina: { width: 1440, height: 900, dpr: 2, throttle: 1 },
  // A low-end laptop: the CPU slowed 4x (phones get their own profiles
  // when the touch layout lands).
  laptop4x: { width: 1280, height: 800, dpr: 1, throttle: 4 },
};

const args = process.argv.slice(2);
// The link comes first.
const link = args[0] && !args[0].startsWith('--') ? args[0] : '';
const opt = (name, fallback) => {
  const i = args.indexOf('--' + name);
  return i >= 0 ? args[i + 1] : fallback;
};
const flag = name => args.includes('--' + name);
if (!link) {
  console.error('usage: node probe.mjs <link> [--runs a,b] [--seconds n] [--trials n] [--own rrggbb] [--reconnect] [--shots dir] [--json file]');
  process.exit(2);
}
const runs = opt('runs', Object.keys(RUNS).join(',')).split(',');
const seconds = Number(opt('seconds', 8));
const trials = Number(opt('trials', 12));
const settle = Number(opt('settle', 1.5));
const cuts = opt('cuts', '3,8').split(',').map(Number);
const own = (0xff000000 | parseInt(opt('own', 'ffd23f'), 16)) >>> 0;
// The canvas fallback hands over the RGBA copy: the same colour reads as
// 0xAABBGGRR there.
const rgbaWord = ((own & 0xff000000) | ((own & 0xff) << 16) | (own & 0xff00) | ((own >>> 16) & 0xff)) >>> 0;
const shots = opt('shots', '');
const track = opt('track', 'color');
const PORT = Number(opt('cdp-port', 8808));
const sleep = ms => new Promise(r => setTimeout(r, ms));

// Page-side code that follows the own character every frame, into
// `__seen` (frame -> [x, y]) and `__ownAt` (the newest x).
//   color:  the centre of the pixels of the own colour (a fixed camera).
//   scroll: the camera follows the own character, so the ground moves
//           instead: each frame, the horizontal shift (within 8 px) that
//           best matches the frame before, added up. Other things moving
//           are outvoted by the ground.
function tracker() {
  return `(() => {
    // The canvas fallback and the GPU renderer both take the RGBA copy.
    const own = /^(canvas2d|gpu-)/.test(__angel.presenter) ? ${rgbaWord} : ${own};
    const mode = ${JSON.stringify(track)};
    let u32 = null, prev = null, cam = 0;
    window.__seen = new Map();
    window.__ownAt = null;
    window.__ownLast = null;
    __angel.onframe = (heap, ptr, w, h, frame, staged) => {
      if (!u32 || u32.buffer !== heap.buffer) u32 = new Uint32Array(heap.buffer);
      const base = ptr >>> 2, n = w * h;
      let at = null;
      if (mode === 'own') {
        // Where the game drew its own character; a new place counts once
        // the frame's pixels around it changed from the frame before. A
        // frame drawn from its parts (a stage) has no finished pixels here:
        // where the game drew it is the place.
        const o = __angel.own();
        const last = window.__ownLast || null;
        if (o) {
          const p = [Math.round(o[0]), Math.round(o[1])];
          if (staged || !last || !prev || prev.length !== n) {
            at = p;
          } else if (p[0] === last[0] && p[1] === last[1]) {
            at = last;
          } else {
            let changed = 0;
            for (let y = Math.max(0, p[1] - 16); y < Math.min(h, p[1] + 8); y++) {
              for (let x = Math.max(0, p[0] - 12); x < Math.min(w, p[0] + 12); x++) {
                if (u32[base + y * w + x] !== prev[y * w + x]) changed++;
              }
            }
            at = changed ? p : last;
          }
          window.__ownLast = at;
        }
        if (!prev || prev.length !== n) prev = new Uint32Array(n);
        prev.set(u32.subarray(base, base + n));
      } else if (mode === 'scroll') {
        if (!prev || prev.length !== n) {
          prev = new Uint32Array(n);
        } else {
          let best = 0, bestMiss = Infinity;
          for (let dx = -8; dx <= 8; dx++) {
            let miss = 0;
            for (let y = 12; y < h; y += 3) {
              const row = base + y * w;
              for (let x = 8; x < w - 8; x += 3) if (u32[row + x] !== prev[y * w + x + dx]) miss++;
            }
            if (miss < bestMiss || (miss === bestMiss && Math.abs(dx) < Math.abs(best))) { bestMiss = miss; best = dx; }
          }
          cam += best;
        }
        prev.set(u32.subarray(base, base + n));
        at = [cam, 0];
      } else {
        let sx = 0, sy = 0, k = 0;
        for (let i = 0; i < n; i++) if (u32[base + i] === own) { sx += i % w; k++; sy += (i / w) | 0; }
        at = k ? [sx / k, sy / k] : null;
      }
      __seen.set(frame, at);
      window.__ownAt = at ? at[0] : null;
      if (__seen.size > 600) __seen.delete(__seen.keys().next().value);
    };
    return true;
  })()`;
}

// Every browser this probe started, so none outlives it: a run that throws
// before its close, or the probe itself stopped by a signal or a timeout,
// still takes its browser and throwaway profile down with it.
const browsers = new Set();
function reap() {
  for (const b of browsers) {
    try { b.proc.kill('SIGKILL'); } catch { /* already gone */ }
    try { fs.rmSync(b.dir, { recursive: true, force: true }); } catch { /* still held */ }
  }
  browsers.clear();
}
process.on('exit', reap);
for (const sig of ['SIGINT', 'SIGTERM', 'SIGHUP']) {
  process.on(sig, async () => {
    // Kill first, then wait for the browsers to be gone (up to 2 s) so the
    // profile directories are free to delete.
    const gone = [...browsers].map(b => b.proc.exitCode !== null || b.proc.signalCode !== null
      ? Promise.resolve()
      : new Promise(resolve => b.proc.once('exit', resolve)));
    for (const b of browsers) {
      try { b.proc.kill('SIGKILL'); } catch { /* already gone */ }
    }
    await Promise.race([Promise.all(gone), sleep(2000)]);
    await sleep(200);
    reap();
    process.exit(128 + ({ SIGHUP: 1, SIGINT: 2, SIGTERM: 15 })[sig]);
  });
}

async function chromium(name) {
  const dir = `${process.env.TMPDIR || '/work/tmp'}/angel-web-probe-${name}-${process.pid}`;
  const proc = spawn('/usr/bin/chromium', ['--headless=new', `--remote-debugging-port=${PORT}`,
    `--user-data-dir=${dir}`, '--no-first-run', '--no-default-browser-check',
    '--autoplay-policy=no-user-gesture-required', '--window-size=1600,1000',
    // The machine's GPU through ANGLE, as a desktop browser has it; with
    // --software, SwiftShader, as a machine whose GPU is blocklisted.
    ...(flag('software') ? ['--enable-unsafe-swiftshader']
      : flag('webgpu') ? ['--enable-unsafe-webgpu', '--enable-features=Vulkan', '--use-angle=vulkan', '--enable-gpu', '--ignore-gpu-blocklist']
      : ['--use-angle=gl', '--enable-gpu', '--ignore-gpu-blocklist']),
    'about:blank'],
  { stdio: 'ignore' });
  const own = { proc, dir };
  browsers.add(own);
  let target;
  for (let i = 0; i < 50 && !target; i++) {
    await sleep(200);
    try {
      const list = await (await fetch(`http://127.0.0.1:${PORT}/json/list`)).json();
      target = list.find(t => t.type === 'page');
    } catch { /* not up yet */ }
  }
  if (!target) {
    browsers.delete(own);
    proc.kill('SIGKILL');
    throw new Error('chromium did not come up');
  }
  const ws = new WebSocket(target.webSocketDebuggerUrl);
  await new Promise((resolve, reject) => { ws.onopen = resolve; ws.onerror = reject; })
    .catch(e => { browsers.delete(own); proc.kill('SIGKILL'); throw e; });
  let id = 0;
  const waits = new Map();
  ws.onmessage = ev => {
    const m = JSON.parse(ev.data);
    if (m.id && waits.has(m.id)) {
      waits.get(m.id)(m);
      waits.delete(m.id);
    } else if (m.method === 'Runtime.exceptionThrown') {
      console.error(`[${name}] page exception:`, m.params.exceptionDetails.exception?.description || m.params.exceptionDetails.text);
    } else if (m.method === 'Runtime.consoleAPICalled' && (m.params.type === 'error' || m.params.type === 'warning')) {
      console.error(`[${name}] page ${m.params.type}:`, m.params.args.map(a => a.value ?? a.description).join(' '));
    } else if (m.method === 'Log.entryAdded' && m.params.entry.level === 'error') {
      console.error(`[${name}] browser:`, m.params.entry.text, m.params.entry.url || '');
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
  const close = () => {
    browsers.delete(own);
    ws.close();
    proc.kill();
    setTimeout(() => fs.rmSync(dir, { recursive: true, force: true }), 500);
  };
  // The browser itself, for opening a second tab (which hides the page).
  const browser = async (method, params = {}) => {
    const version = await (await fetch(`http://127.0.0.1:${PORT}/json/version`)).json();
    const bws = new WebSocket(version.webSocketDebuggerUrl);
    await new Promise(resolve => { bws.onopen = resolve; });
    const result = await new Promise(resolve => {
      bws.onmessage = ev => resolve(JSON.parse(ev.data).result);
      bws.send(JSON.stringify({ id: 1, method, params }));
    });
    bws.close();
    return result;
  };
  return { send, evaluate, close, browser };
}

async function until(evaluate, expr, ms, what) {
  const end = Date.now() + ms;
  while (Date.now() < end) {
    if (await evaluate(expr)) return;
    await sleep(50);
  }
  throw new Error('timed out waiting for ' + what + ': ' + JSON.stringify(await evaluate('window.__angel && __angel.stats()')));
}

const KEY = {
  d: { code: 'KeyD', key: 'd', windowsVirtualKeyCode: 68 },
  a: { code: 'KeyA', key: 'a', windowsVirtualKeyCode: 65 },
  w: { code: 'KeyW', key: 'w', windowsVirtualKeyCode: 87 },
  s: { code: 'KeyS', key: 's', windowsVirtualKeyCode: 83 },
  e: { code: 'KeyE', key: 'e', windowsVirtualKeyCode: 69 },
  space: { code: 'Space', key: ' ', windowsVirtualKeyCode: 32 },
  up: { code: 'ArrowUp', key: 'ArrowUp', windowsVirtualKeyCode: 38 },
  down: { code: 'ArrowDown', key: 'ArrowDown', windowsVirtualKeyCode: 40 },
  left: { code: 'ArrowLeft', key: 'ArrowLeft', windowsVirtualKeyCode: 37 },
  right: { code: 'ArrowRight', key: 'ArrowRight', windowsVirtualKeyCode: 39 },
};

function median(xs) {
  const s = [...xs].sort((a, b) => a - b);
  return s.length ? s[Math.floor(s.length / 2)] : NaN;
}

async function run(name) {
  const r = RUNS[name];
  if (!r) throw new Error('no run ' + name);
  const b = await chromium(name);
  const { send, evaluate } = b;
  const key = async (type, k) => send('Input.dispatchKeyEvent', { type, ...KEY[k], nativeVirtualKeyCode: KEY[k].windowsVirtualKeyCode });
  const out = { run: name, ...r };
  try {
    await send('Page.enable');
    await send('Runtime.enable');
    await send('Network.enable');
    await send('Log.enable');
    await send('Emulation.setDeviceMetricsOverride', { width: r.width, height: r.height, deviceScaleFactor: r.dpr, mobile: false });
    await send('Emulation.setCPUThrottlingRate', { rate: r.throttle });
    if (flag('webgpu')) {
      // A browser just started may have no GPU process yet, and then its first
      // requestAdapter answers none (or its first contexts are lost as the
      // process settles): draw with the page's own WebGL once first, so the
      // run measures the renderer and not the cold start.
      const warm = new URL(link);
      warm.searchParams.set('present', 'legacy');
      await send('Page.navigate', { url: warm.toString() });
      await until(evaluate, "window.__angel && __angel.status !== 'loading'", 30000, 'the module (warm-up)');
      await sleep(1500);
    }
    await send('Page.navigate', { url: link });
    await until(evaluate, "window.__angel && __angel.status !== 'loading'", 30000, 'the module');
    const ready = await evaluate('({ status: __angel.status, loadedMs: __angel.loadedMs, presenter: __angel.presenter })');
    out.loadedMs = Math.round(ready.loadedMs);
    out.presenter = ready.presenter;
    out.gl = await evaluate(`(() => { const c = document.createElement('canvas'); const gl = c.getContext('webgl2') || c.getContext('webgl');
      if (!gl) return 'none'; const d = gl.getExtension('WEBGL_debug_renderer_info');
      return d ? gl.getParameter(d.UNMASKED_RENDERER_WEBGL) : gl.getParameter(gl.RENDERER); })()`);
    if (ready.status === 'ready') {
      // A real click on Play: a user gesture, as a player's is.
      await evaluate("document.getElementById('name').value = 'probe'");
      const at = await evaluate("(() => { const r = document.getElementById('play').getBoundingClientRect(); return [r.x + r.width / 2, r.y + r.height / 2]; })()");
      for (const type of ['mousePressed', 'mouseReleased']) {
        await send('Input.dispatchMouseEvent', { type, x: at[0], y: at[1], button: 'left', clickCount: 1 });
      }
    }
    await until(evaluate, "__angel.status === 'playing'", 20000, 'a seat');
    out.seat = await evaluate('__angel.seat');
    await sleep(settle * 1000);

    // Steady play: walk around and use, as a player does.
    await evaluate('__angel.reset()');
    const bytesWas = await evaluate('[__angel.bytesIn, __angel.bytesOut]');
    const memoryWas = (await evaluate('__angel.stats()')).memory;
    const t0 = Date.now();
    const pattern = ['d', 's', 'a', 'w'];
    for (let i = 0; Date.now() - t0 < seconds * 1000; i++) {
      const k = pattern[i % 4];
      await key('keyDown', k);
      await sleep(350);
      if (i % 3 === 0 && !flag('no-use')) {
        await key('keyDown', 'e');
        await key('keyUp', 'e');
      }
      await sleep(350);
      await key('keyUp', k);
    }
    const took = (Date.now() - t0) / 1000;
    const s = await evaluate('__angel.stats()');
    out.fps = +(s.frames / took).toFixed(1);
    out.frames = s.frames;
    out.dropped = s.dropped;
    out.wasmP50 = +s.wasmP50.toFixed(3);
    out.wasmP95 = +s.wasmP95.toFixed(3);
    out.presentP50 = +s.presentP50.toFixed(3);
    out.presentP95 = +s.presentP95.toFixed(3);
    out.rtt = Math.round(s.rtt);
    out.kbInPerS = +((s.bytesIn - bytesWas[0]) / 1024 / took).toFixed(2);
    out.kbOutPerS = +((s.bytesOut - bytesWas[1]) / 1024 / took).toFixed(2);
    out.sounds = s.sounds;
    // The module's memory before and after the steady play: it should not
    // grow while nothing new is happening.
    out.memoryKB = [memoryWas / 1024, s.memory / 1024];
    if (shots) {
      fs.mkdirSync(shots, { recursive: true });
      // Headless Chromium's screenshot leaves a WebGPU canvas out, and a
      // WebGL one is only readable in the task of its frame: so the canvas
      // itself is read, in a frame's own task, for the presenters that draw
      // from the GPU renderer; the screenshot serves the rest.
      let png = null;
      if (/^gpu-/.test(out.presenter)) {
        await evaluate("window.__shot = null; __angel.onframe = () => { __shot = document.getElementById('screen').toDataURL('image/png'); __angel.onframe = null; }");
        await until(evaluate, 'window.__shot', 5000, 'a frame to read');
        png = Buffer.from((await evaluate('__shot')).split(',')[1], 'base64');
      } else {
        png = Buffer.from((await send('Page.captureScreenshot', { format: 'png' })).data, 'base64');
      }
      fs.writeFileSync(`${shots}/${name}.png`, png);
    }

    // Key to own character moved, in display frames. Each frame the page
    // finds the own character (see `tracker`); a trial presses D at a
    // different point of the command tick and counts the frames until the
    // character is drawn somewhere else. 1 is the next display frame.
    await evaluate(tracker());
    const lat = [];
    for (let t = 0; t < trials; t++) {
      await sleep(400 + Math.random() * 70);
      const k = t % 2 ? 'a' : 'd';
      await key('keyDown', k);
      const at = await evaluate('__angel.inputFrame');
      await sleep(250);
      await key('keyUp', k);
      const moved = await evaluate(`(() => {
        const at = ${at}, was = __seen.get(at);
        if (!was) return -1;
        for (let f = at + 1; f <= at + 30; f++) {
          const p = __seen.get(f);
          if (p && (p[0] !== was[0] || p[1] !== was[1])) return f - at;
        }
        return -1;
      })()`);
      lat.push(moved);
    }
    await evaluate('__angel.onframe = null');
    out.latencyFrames = lat;
    out.latencyMedian = median(lat.filter(v => v > 0));
    out.latencyNextFrame = lat.filter(v => v === 1).length + '/' + lat.length;

    if (flag('roll')) {
      // Each frame, where the game drew its own character and when.
      await evaluate(`(() => {
        window.__trace = [];
        __angel.onframe = (heap, ptr, w, h, frame) => {
          const o = __angel.own();
          window.__frameW = w;
          if (o) __trace.push([performance.now(), o[0], o[1], frame]);
        };
      })()`);
      // The far side first, so neither walk ends against a wall.
      await sleep(200);
      const start = await evaluate('[__trace.length ? __trace[__trace.length - 1][1] : 0, window.__frameW || 0]');
      const legs = start[0] > start[1] / 2 ? [['a', 'd'], ['d', 'a']] : [['d', 'a'], ['a', 'd']];
      const marks = [];
      const mark = async what => marks.push([await evaluate('performance.now()'), what]);
      const strip = [];
      const snap = async () => {
        if (!shots) return;
        const shot = await send('Page.captureScreenshot', { format: 'png' });
        strip.push(shot.data);
      };
      for (const [k, back] of legs) {
        await mark('hold ' + k);
        await key('keyDown', k);
        await sleep(300);
        await mark('roll');
        await key('keyDown', 'space');
        await sleep(60);
        await key('keyUp', 'space');
        if (k === legs[0][0]) for (let i = 0; i < 8; i++) await snap();
        // Short of the far wall: a knight against stone stands still.
        await sleep(400);
        await key('keyUp', k);
        await mark('let go ' + k);
        await sleep(250);
        void back;
      }
      const trace = await evaluate('__trace');
      await evaluate('__angel.onframe = null');
      // With a key held, the frames the character stood still or stepped
      // back (against the key), and its steps, in frame pixels.
      const held = [];
      for (let i = 0; i < marks.length; i++) {
        if (marks[i][1].startsWith('hold ')) {
          const dir = marks[i][1].endsWith(' d') ? 1 : -1;
          const end = marks.find((m, j) => j > i && m[1].startsWith('let go'));
          // From two command ticks after the key, to the letting go.
          held.push([marks[i][0] + 80, end ? end[0] : Infinity, dir]);
        }
      }
      let frames = 0, still = 0, back = 0;
      const steps = [];
      for (let i = 1; i < trace.length; i++) {
        const [t, x] = trace[i];
        const span = held.find(([a, b]) => t >= a && t <= b);
        if (!span) continue;
        frames++;
        const dx = (x - trace[i - 1][1]) * span[2];
        if (dx === 0) still++;
        else if (dx < 0) back++;
        else steps.push(dx);
      }
      out.roll = {
        frames,
        stoodStill: still,
        steppedBack: back,
        medianStep: +median(steps).toFixed(2),
        biggestStep: +Math.max(0, ...steps).toFixed(2),
        trace: trace.map(([t, x, y]) => [+t.toFixed(1), +x.toFixed(1), +y.toFixed(1)]),
        marks: marks.map(([t, w]) => [+t.toFixed(1), w]),
      };
      if (shots) {
        fs.mkdirSync(shots, { recursive: true });
        strip.forEach((d, i) => fs.writeFileSync(`${shots}/${name}-roll-${i}.png`, Buffer.from(d, 'base64')));
      }
    }

    if (flag('fire')) out.fire = await fireTrials(b, key);

    if (flag('hide')) {
      // Hold D, hide the tab behind another for 3 s, show it again. The
      // page must stop drawing, let go of D (the knight stops where it
      // was, not 15 ticks later) and keep its seat without a reconnect.
      await evaluate(tracker());
      await key('keyDown', 'd');
      await sleep(300);
      const before = await evaluate('({ frames: __angel.frames, reconnects: __angel.reconnects, seat: __angel.seat, x: __ownAt })');
      const other = await b.browser('Target.createTarget', { url: 'about:blank' });
      await sleep(3000);
      const hidden = await evaluate('({ frames: __angel.frames, visibility: document.visibilityState, status: __angel.status })');
      await b.browser('Target.closeTarget', { targetId: other.targetId });
      await sleep(500);
      const shownX = await evaluate('__ownAt');
      await key('keyUp', 'd');
      await sleep(1000);
      const after = await evaluate('({ frames: __angel.frames, reconnects: __angel.reconnects, seat: __angel.seat, status: __angel.status })');
      await evaluate('__angel.onframe = null');
      out.hide = {
        visibilityWhileHidden: hidden.visibility,
        framesWhileHidden: hidden.frames - before.frames,
        framesAfterShown: after.frames - hidden.frames,
        statusWhileHidden: hidden.status,
        sameSeat: after.seat === before.seat,
        reconnected: after.reconnects > before.reconnects,
        // Frame pixels the own character moved from hiding to shown again:
        // only the commands already in flight, if the key was let go.
        ownMovedAcrossHide: shownX === null || before.x === null ? null : shownX - before.x,
      };
    }

    if (flag('reconnect')) {
      // Cut the network, restore it, and wait for play to flow again. A
      // short cut may leave the socket standing (the browser stalls it
      // rather than closing it); one longer than the page's 3 s silence
      // rule makes the page drop it and resume the seat on a new one.
      out.reconnect = [];
      for (const cutS of cuts) {
        const before = await evaluate('({ seat: __angel.seat, reconnects: __angel.reconnects })');
        await send('Network.emulateNetworkConditions', { offline: true, latency: 0, downloadThroughput: -1, uploadThroughput: -1 });
        await sleep(cutS * 1000);
        const during = await evaluate('__angel.status');
        await send('Network.emulateNetworkConditions', { offline: false, latency: 0, downloadThroughput: -1, uploadThroughput: -1 });
        const back = Date.now();
        await until(evaluate, "__angel.status === 'playing' && performance.now() - __angel.stats().heardAt < 250", 30000, 'play again');
        const after = await evaluate('({ seat: __angel.seat, reconnects: __angel.reconnects })');
        out.reconnect.push({
          cutS,
          statusAtRestore: during,
          reconnected: after.reconnects > before.reconnects,
          seatBefore: before.seat,
          seatAfter: after.seat,
          sameSeat: before.seat === after.seat,
          playingMsAfterRestore: Date.now() - back,
        });
        await sleep(1000);
      }
    }
  } finally {
    b.close();
  }
  return out;
}

// Fire key to the own shot on screen. Each frame the page notes when it
// was drawn, where the game drew its own knight, and how many own shots it
// drew and the nearest one's offset from the knight (`__angel.game()`).
// A trial waits for no own shot on screen, presses an arrow (aim and fire),
// and finds the first frame after the press with one.
async function fireTrials(b, key) {
  const { send, evaluate } = b;
  const n = Number(opt('fire-trials', 12));
  await evaluate(`(() => {
    window.__fire = [];
    window.__grab = null;
    __angel.onframe = (heap, ptr, w, h, frame) => {
      const g = __angel.game() || {};
      const o = __angel.own();
      __fire.push([performance.now(), frame, o ? o[0] : null, o ? o[1] : null, g.own_shots || 0,
        g.own_shot_gap_px ?? null, g.own_shot_dx_px ?? null, g.own_shot_dy_px ?? null, g.own_shot_gap ?? null,
        g.shot_ghosts ?? null, g.shot_handovers ?? null, g.shot_unmatched ?? null]);
      if (__fire.length > 6000) __fire.shift();
      const G = window.__grab;
      if (G && G.frames.length < G.n && frame > G.from) {
        G.frames.push(new Uint32Array(heap.buffer, ptr, w * h).slice());
        G.own.push(o);
        G.w = w; G.h = h;
      }
    };
  })()`);
  // Off the bottom wall first, into the room (the Delve's fights start a
  // knight there, under the bar of keys).
  await key('keyDown', 'w');
  await sleep(Number(opt('fire-north-ms', 900)));
  await key('keyUp', 'w');
  const trials = [];
  let strip = null, stripOwn = null;
  for (let t = 0; t < n; t++) {
    // Standing, then running east, then west, in turn; the aim is up, so a
    // shot left behind a running knight shows as a sideways offset.
    const kind = ['stand', 'east', 'west'][t % 3];
    const walk = kind === 'east' ? 'd' : kind === 'west' ? 'a' : null;
    const end = Date.now() + 2000;
    while (Date.now() < end && (await evaluate('__angel.game()?.own_shots || 0')) > 0) await sleep(50);
    await sleep(100 + Math.random() * 40);
    if (walk) {
      await key('keyDown', walk);
      await sleep(350);
    }
    await key('keyDown', 'up');
    const at = await evaluate('[__angel.inputFrame, __angel.inputAt]');
    const grab = shots && kind === 'east' && !strip;
    if (grab) await evaluate(`window.__grab = { from: ${at[0]}, n: 24, frames: [], own: [] }`);
    await sleep(90);
    await key('keyUp', 'up');
    await sleep(500);
    if (walk) await key('keyUp', walk);
    const seen = await evaluate(`__fire.filter(f => f[1] > ${at[0]} && f[4] > 0)[0] || null`);
    // Each frame from the press for half a second: [ms after the press,
    // own shots drawn, ghosts among them, handovers so far, unmatched].
    const trace = await evaluate(`__fire.filter(f => f[1] > ${at[0]} && f[0] - ${at[1]} < 500)
      .map(f => [Math.round(f[0] - ${at[1]}), f[4], f[9], f[10], f[11]])`);
    trials.push(seen ? {
      kind,
      trace,
      ms: +(seen[0] - at[1]).toFixed(1),
      frames: seen[1] - at[0],
      gapPx: seen[5], dxPx: seen[6], dyPx: seen[7], gapUnits: seen[8],
    } : { kind, ms: null });
    if (grab) {
      // The frames from the press on, as PNGs (the module's words are
      // 0xAARRGGBB; the canvas fallback's are already RGBA).
      stripOwn = await evaluate('window.__grab.own');
      strip = await evaluate(`(() => {
        const G = window.__grab; window.__grab = null;
        const c = document.createElement('canvas'); c.width = G.w; c.height = G.h;
        const ctx = c.getContext('2d'), img = ctx.createImageData(G.w, G.h);
        const rgba = __angel.presenter === 'canvas2d';
        return G.frames.map(f => {
          const d = img.data;
          for (let i = 0; i < f.length; i++) {
            const w = f[i];
            if (rgba) { d[4*i] = w & 255; d[4*i+1] = (w >>> 8) & 255; d[4*i+2] = (w >>> 16) & 255; }
            else { d[4*i] = (w >>> 16) & 255; d[4*i+1] = (w >>> 8) & 255; d[4*i+2] = w & 255; }
            d[4*i+3] = 255;
          }
          ctx.putImageData(img, 0, 0);
          return c.toDataURL('image/png').split(',')[1];
        });
      })()`);
    }
  }
  await evaluate('__angel.onframe = null');
  if (strip) {
    fs.mkdirSync(shots, { recursive: true });
    strip.forEach((d, i) => fs.writeFileSync(`${shots}/fire-${String(i + 1).padStart(2, '0')}.png`, Buffer.from(d, 'base64')));
    // Where the knight was drawn on each, for cropping.
    fs.writeFileSync(`${shots}/fire-own.json`, JSON.stringify(stripOwn));
  }
  // The game's own-shot counts at the end (the Delve: ghosts handed to the
  // server's shots, the gaps eased, ghosts it never matched).
  const counts = await evaluate('__angel.game()');
  const ok = trials.filter(t => t.ms !== null);
  const by = kind => ok.filter(t => kind === 'all' || t.kind === kind || (kind === 'run' && t.kind !== 'stand'));
  const sum = kind => {
    const xs = by(kind);
    return {
      n: xs.length,
      msMedian: +median(xs.map(t => t.ms)).toFixed(1),
      msMax: +Math.max(...xs.map(t => t.ms)).toFixed(1),
      framesMedian: median(xs.map(t => t.frames)),
      gapPxMedian: +median(xs.map(t => t.gapPx)).toFixed(1),
      sidewaysPxMedian: +median(xs.map(t => Math.abs(t.dxPx))).toFixed(1),
      gapUnitsMedian: +median(xs.map(t => t.gapUnits)).toFixed(2),
    };
  };
  return { stand: sum('stand'), run: sum('run'), missed: trials.length - ok.length, counts, trials };
}

const results = [];
for (const name of runs) {
  // A run that fails is reported and the next one still runs (and the
  // browser's throwaway profile is still cleaned up below).
  const r = await run(name).catch(e => ({ run: name, error: String(e.message || e) }));
  results.push(r);
  console.log(JSON.stringify(r));
}
if (opt('json', '')) fs.writeFileSync(opt('json'), JSON.stringify(results, null, 2));
await sleep(600);
process.exit(0);
