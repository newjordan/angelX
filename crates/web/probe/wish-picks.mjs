// Exercise the real shell wish-line handlers and CSS in headless Chromium.
// The game supplies fixture text through angel_picks; Rust client tests
// cover the run-to-text half. No host or launcher is needed.
// node crates/web/probe/wish-picks.mjs [--shots DIR]
import assert from 'node:assert/strict';
import { spawn } from 'node:child_process';
import fs from 'node:fs';
import os from 'node:os';
import path from 'node:path';

const shell = new URL('../shell/', import.meta.url);
const read = name => fs.readFileSync(new URL(name, shell), 'utf8');
const boot = read('boot.js');
const start = boot.indexOf("  // A game's text line");
const end = boot.indexOf("  // A game's plates to drag", start);
assert(start >= 0 && end > start, 'wish-line handlers exist');
const markup = read('index.html').match(/<form id="say"[\s\S]*?<\/form>/)[0];
const at = process.argv.indexOf('--shots');
const shots = at >= 0 ? process.argv[at + 1] : null;
if (shots) fs.mkdirSync(shots, { recursive: true });
const profile = fs.mkdtempSync(path.join(os.tmpdir(), 'wish-picks-'));
const chrome = spawn('/usr/bin/chromium', ['--headless=new', '--no-sandbox', '--disable-gpu',
  '--disable-dev-shm-usage', '--no-first-run', '--no-default-browser-check',
  '--remote-debugging-port=0', `--user-data-dir=${profile}`, 'about:blank'], { stdio: 'ignore' });
const sleep = ms => new Promise(resolve => setTimeout(resolve, ms));
let ws;
try {
  let port;
  for (let i = 0; i < 100 && !port; i++) {
    await sleep(50);
    try { port = Number(fs.readFileSync(path.join(profile, 'DevToolsActivePort'), 'utf8').split('\n')[0]); } catch { /* starting */ }
  }
  assert(port, 'Chromium started');
  const target = (await (await fetch(`http://127.0.0.1:${port}/json/list`)).json()).find(t => t.type === 'page');
  ws = new WebSocket(target.webSocketDebuggerUrl);
  await new Promise((resolve, reject) => { ws.onopen = resolve; ws.onerror = reject; });
  let serial = 0;
  const pending = new Map();
  ws.onmessage = event => {
    const m = JSON.parse(event.data);
    if (pending.has(m.id)) { pending.get(m.id)(m); pending.delete(m.id); }
  };
  const send = (method, params = {}) => new Promise((resolve, reject) => {
    const id = ++serial;
    pending.set(id, m => m.error ? reject(new Error(m.error.message)) : resolve(m.result));
    ws.send(JSON.stringify({ id, method, params }));
  });
  const evaluate = async expression => {
    const result = await send('Runtime.evaluate', { expression, returnByValue: true, awaitPromise: true });
    if (result.exceptionDetails) throw new Error(result.exceptionDetails.exception?.description || result.exceptionDetails.text);
    return result.result.value;
  };
  await send('Page.enable');
  await send('Page.setDocumentContent', { frameId: (await send('Page.getFrameTree')).frameTree.frame.id,
    html: `<html><head><style>${read('skin.css')}\n${read('shell.css')}</style></head><body>${markup}</body></html>` });
  await evaluate(`${read('ui.js')}
    const $ = id => document.getElementById(id);
    const sayForm = $('say'), sayInput = $('said'), sayTitle = $('say-title'), sayKeys = $('say-keys'), sayPicks = $('say-picks');
    const A = { controls: { text: { key: 'KeyT', kind: 2, field: 'words', title: 'Make a wish', placeholder: 'Wish for anything, in your words', send: 'the genie offers how' } } };
    let running = true, crashed = false;
    window.fixture = ''; window.actions = [];
    const x = { angel_picks: () => fixture, angel_action: (kind, body) => actions.push({ kind, body: JSON.parse(body) }) };
    const text = s => s, withString = (s, f) => f(s, s.length), flush = () => {};
    const input = { releaseAll() {} };
    const gameHasKeys = () => sayForm.hidden;
    ${boot.slice(start, end)}
    window.openWish = () => { closeSay(); openSay(); };
    window.refreshPicks = fillPicks;
    window.gameKeys = [];
    window.addEventListener('keydown', e => { if (gameHasKeys()) gameKeys.push(e.code); });
  `);
  const normal = 'Upgrade what you hold\n1\tFirebolt\tI → II\tA firebolt every 4 seconds.\tupgrade Firebolt\n2\tTriple Volley\tI → II\tEvery 6th shot flies three wide.\tupgrade Triple Volley\n\tIron Skin\tIII\tMAX\t';
  const open = async (fixture = normal) => evaluate(`fixture = ${JSON.stringify(fixture)}; openWish()`);
  const key = async (key, code, text = '') => {
    await send('Input.dispatchKeyEvent', { type: 'keyDown', key, code, text });
    await send('Input.dispatchKeyEvent', { type: 'keyUp', key, code });
  };
  const screenshot = async name => {
    if (!shots) return;
    const { data } = await send('Page.captureScreenshot', { format: 'png' });
    fs.writeFileSync(path.join(shots, `${name}.png`), Buffer.from(data, 'base64'));
  };
  for (const [name, width, height] of [['desktop', 1280, 800], ['phone', 390, 844]]) {
    await send('Emulation.setDeviceMetricsOverride', { width, height, deviceScaleFactor: 1, mobile: false });
    await open();
    const layout = await evaluate(`(() => {
      const form = sayForm.getBoundingClientRect(), input = sayInput.getBoundingClientRect();
      return { head: sayPicks.querySelector('h3').textContent, names: [...sayPicks.querySelectorAll('.label')].map(e => e.textContent),
        disabled: sayPicks.querySelectorAll('button:disabled').length, fits: form.x >= 0 && form.right <= innerWidth && form.y >= 0 && input.bottom <= innerHeight };
    })()`);
    assert.equal(layout.head, 'Upgrade what you hold');
    assert.deepEqual(layout.names, ['Firebolt', 'Triple Volley', 'Iron Skin']);
    assert.equal(layout.disabled, 1);
    assert(layout.fits, `${name}: wish line stays in the viewport`);
    await screenshot(name);
  }
  await key('1', 'Digit1', '1');
  assert.deepEqual(await evaluate('actions.pop()'), { kind: 2, body: { words: 'upgrade Firebolt' } });
  await open();
  await send('Emulation.setTouchEmulationEnabled', { enabled: true });
  const tapAt = await evaluate(`(() => { const r = sayPicks.querySelector('button').getBoundingClientRect(); return { x: r.x + r.width / 2, y: r.y + r.height / 2 }; })()`);
  await send('Input.dispatchTouchEvent', { type: 'touchStart', touchPoints: [tapAt] });
  await send('Input.dispatchTouchEvent', { type: 'touchEnd', touchPoints: [] });
  await sleep(100);
  assert.deepEqual(await evaluate('actions.pop()'), { kind: 2, body: { words: 'upgrade Firebolt' } });
  await send('Emulation.setTouchEmulationEnabled', { enabled: false });
  await open();
  const point = await evaluate(`(() => { const r = sayPicks.querySelectorAll('button')[1].getBoundingClientRect(); return { x: r.x + r.width / 2, y: r.y + r.height / 2 }; })()`);
  await send('Input.dispatchMouseEvent', { type: 'mousePressed', ...point, button: 'left', clickCount: 1 });
  await send('Input.dispatchMouseEvent', { type: 'mouseReleased', ...point, button: 'left', clickCount: 1 });
  assert.deepEqual(await evaluate('actions.pop()'), { kind: 2, body: { words: 'upgrade Triple Volley' } });
  await open();
  await evaluate(`sayInput.value = 'a';`);
  await key('1', 'Digit1', '1');
  assert.equal(await evaluate('sayInput.value'), 'a1', 'numbers inside words are typed');
  await evaluate(`sayInput.value = 'bounce my shots';`);
  await key('Enter', 'Enter', '\r');
  assert.deepEqual(await evaluate('actions.pop()'), { kind: 2, body: { words: 'bounce my shots' } });
  await open();
  await evaluate(`fixture = ${JSON.stringify('Upgrade what you hold\n\tFirebolt\tI → II\tA firebolt every 4 seconds.\t\nNo wishes held: each floor brings two')}; refreshPicks();`);
  assert.equal(await evaluate('sayPicks.querySelectorAll("button:enabled").length'), 0);
  await key('1', 'Digit1', '1');
  assert.equal(await evaluate('sayInput.value'), '1');
  assert.equal(await evaluate('actions.length'), 0);
  await screenshot('no-wishes');
  const many = 'Upgrade what you hold\n' + Array.from({ length: 15 }, (_, i) => `\tWish ${i}\tI → II\tThe next tier.\tupgrade Wish ${i}`).join('\n');
  await open(many);
  assert(await evaluate('sayPicks.scrollHeight > sayPicks.clientHeight'), 'long lists scroll');
  await evaluate(`sayPicks.querySelectorAll('button')[14].click()`);
  assert.deepEqual(await evaluate('actions.pop()'), { kind: 2, body: { words: 'upgrade Wish 14' } });
  await open();
  await evaluate(`sayPicks.querySelector('button').focus()`);
  await key('Escape', 'Escape');
  assert(await evaluate('sayForm.hidden'), 'Escape rolls the line up');
  assert.deepEqual(await evaluate('gameKeys'), [], 'wish keys never reach gameplay after the line closes');
  console.log('wish picks: desktop/phone layout, number, click/tap, typed words, MAX, no wishes, live refresh, long list and Escape passed');
} finally {
  ws?.close();
  chrome.kill('SIGKILL');
  await new Promise(resolve => chrome.once('exit', resolve));
  fs.rmSync(profile, { recursive: true, force: true });
}
