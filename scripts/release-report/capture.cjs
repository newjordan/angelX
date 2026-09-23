// Capture a scoreboard page as an H.264 MP4 and/or a PNG of its final frame.
// Same pipeline as scripts/render_scoreboard_mp4.cjs: headless Chromium over the DevTools
// protocol, PNG frames piped into ffmpeg (libx264, yuv420p, faststart). The page's directory is
// served on 127.0.0.1 so canvases can read its images (file:// would taint them).
//
// Page contract: window.ready (promise), window.DURATION (s), window.renderAt(t) -> key|null
// (an unchanged non-null key reuses the previous frame), window.renderFinal().
//
// usage: node capture.cjs --html dir/page.html --size 1080x1080 [--mp4 out.mp4] [--png final.png] [--fps 30]
'use strict';
const fs = require('fs');
const os = require('os');
const path = require('path');
const http = require('http');
const { spawn } = require('child_process');

function args() {
  const a = process.argv.slice(2), o = { fps: 30 };
  for (let i = 0; i < a.length; i += 2) o[a[i].replace(/^--/, '')] = a[i + 1];
  if (!o.html || !o.size) throw new Error('need --html and --size');
  const [w, h] = o.size.split('x').map(Number);
  return { ...o, w, h, fps: +o.fps };
}

const TYPES = { '.html': 'text/html; charset=utf-8', '.png': 'image/png', '.js': 'text/javascript', '.css': 'text/css' };
function serve(root) {
  const server = http.createServer((req, res) => {
    const file = path.join(root, decodeURIComponent(new URL(req.url, 'http://x').pathname));
    if (!file.startsWith(root) || !fs.existsSync(file) || fs.statSync(file).isDirectory()) { res.writeHead(404); res.end(); return; }
    res.writeHead(200, { 'content-type': TYPES[path.extname(file)] || 'application/octet-stream' });
    fs.createReadStream(file).pipe(res);
  });
  return new Promise(r => server.listen(0, '127.0.0.1', () => r(server)));
}

const sleep = ms => new Promise(r => setTimeout(r, ms));

async function devtoolsPort(dir, chrome) {
  const file = path.join(dir, 'DevToolsActivePort');
  for (let i = 0; i < 200; i++) {
    if (chrome.exitCode !== null) throw new Error('chromium exited early');
    if (fs.existsSync(file)) {
      const port = fs.readFileSync(file, 'utf8').split('\n')[0].trim();
      if (port) return port;
    }
    await sleep(50);
  }
  throw new Error('chromium did not open a DevTools port');
}

async function main() {
  const o = args();
  const root = path.dirname(path.resolve(o.html));
  const server = await serve(root);
  const url = `http://127.0.0.1:${server.address().port}/${path.basename(o.html)}`;
  const profile = fs.mkdtempSync(path.join(os.tmpdir(), 'angelx-capture-'));
  const chrome = spawn('chromium', [
    '--headless=new', '--disable-gpu', '--no-sandbox', '--hide-scrollbars', '--no-first-run',
    '--force-color-profile=srgb', '--remote-debugging-port=0', `--user-data-dir=${profile}`,
    `--window-size=${o.w},${o.h}`, 'about:blank',
  ], { stdio: 'ignore' });
  const cleanup = () => {
    try { chrome.kill(); } catch (e) { /* already gone */ }
    try { fs.rmSync(profile, { recursive: true, force: true }); } catch (e) { /* best effort */ }
    server.close();
  };
  process.on('exit', cleanup);

  const port = await devtoolsPort(profile, chrome);
  const tabs = await (await fetch(`http://127.0.0.1:${port}/json`)).json();
  const tab = tabs.find(t => t.type === 'page');
  if (!tab) throw new Error('no page target');
  const ws = new WebSocket(tab.webSocketDebuggerUrl);
  await new Promise((res, rej) => { ws.onopen = res; ws.onerror = rej; });

  let nextId = 1;
  const pending = new Map(), waiters = new Map();
  ws.onmessage = ev => {
    const msg = JSON.parse(ev.data);
    if (msg.id && pending.has(msg.id)) {
      const { resolve, reject } = pending.get(msg.id);
      pending.delete(msg.id);
      msg.error ? reject(new Error(msg.error.message)) : resolve(msg.result);
    } else if (msg.method && waiters.has(msg.method)) {
      waiters.get(msg.method)(msg.params);
      waiters.delete(msg.method);
    }
  };
  const cdp = (method, params = {}) => new Promise((resolve, reject) => {
    const id = nextId++;
    pending.set(id, { resolve, reject });
    ws.send(JSON.stringify({ id, method, params }));
  });
  const once = method => new Promise(r => waiters.set(method, r));
  const evaluate = async expression => {
    const r = await cdp('Runtime.evaluate', { expression, awaitPromise: true, returnByValue: true });
    if (r.exceptionDetails) throw new Error(r.exceptionDetails.exception?.description || r.exceptionDetails.text);
    return r.result.value;
  };
  const shot = async () => {
    const s = await cdp('Page.captureScreenshot', { format: 'png', clip: { x: 0, y: 0, width: o.w, height: o.h, scale: 1 } });
    return Buffer.from(s.data, 'base64');
  };

  await cdp('Page.enable');
  await cdp('Runtime.enable');
  await cdp('Emulation.setDeviceMetricsOverride', { width: o.w, height: o.h, deviceScaleFactor: 1, mobile: false });
  const loaded = once('Page.loadEventFired');
  await cdp('Page.navigate', { url });
  await loaded;
  const info = await evaluate('window.ready');
  console.log(`${path.basename(o.html)}: ${o.w}x${o.h}, grid pitch ${info.pitch}px, stage ${Math.round(info.height)}px`
    + (info.sword !== undefined ? `, excalibur ${info.sword ? 'on' : 'unavailable'}` : ''));

  if (o.mp4) {
    const duration = await evaluate('window.DURATION');
    const total = Math.round(duration * o.fps);
    const ff = spawn('ffmpeg', [
      '-y', '-loglevel', 'error', '-f', 'image2pipe', '-framerate', String(o.fps), '-c:v', 'png', '-i', '-',
      '-c:v', 'libx264', '-profile:v', 'high', '-preset', 'slow', '-crf', '18', '-pix_fmt', 'yuv420p',
      '-r', String(o.fps), '-movflags', '+faststart', o.mp4,
    ], { stdio: ['pipe', 'inherit', 'inherit'] });
    const closed = new Promise((res, rej) => ff.on('close', c => (c === 0 ? res() : rej(new Error('ffmpeg exited ' + c)))));
    let lastKey = null, lastBuf = null, shots = 0;
    for (let f = 0; f < total; f++) {
      const key = await evaluate(`window.renderAt(${f / o.fps})`);
      if (key === null || key !== lastKey || !lastBuf) { lastBuf = await shot(); shots++; }
      lastKey = key;
      if (!ff.stdin.write(lastBuf)) await new Promise(r => ff.stdin.once('drain', r));
    }
    ff.stdin.end();
    await closed;
    console.log(`wrote ${o.mp4} (${total} frames, ${(total / o.fps).toFixed(1)} s, ${shots} captured)`);
  }
  if (o.png) {
    await evaluate('window.renderFinal()');
    fs.writeFileSync(o.png, await shot());
    console.log(`wrote ${o.png}`);
  }
  ws.close();
  cleanup();
}

main().then(() => process.exit(0), err => { console.error(err); process.exit(1); });
