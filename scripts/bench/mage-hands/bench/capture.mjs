// Capture 1920x1080 screenshots of public pages in an isolated headless Chrome
// and record every visible, named, clickable element's bbox as ground truth.
//   node bin/capture.mjs <out-dir> <url>...
import { spawn } from "node:child_process";
import { mkdirSync, writeFileSync, readFileSync } from "node:fs";
import { setTimeout as sleep } from "node:timers/promises";

const [outDir, ...urls] = process.argv.slice(2);
mkdirSync(`${outDir}/shots`, { recursive: true });
const port = 9447;
const chrome = spawn("google-chrome-stable", [
  "--headless=new", `--remote-debugging-port=${port}`,
  `--user-data-dir=${outDir}/profile`, "--no-first-run", "--no-default-browser-check",
  "--window-size=1920,1080", "--hide-scrollbars", "--mute-audio", "about:blank",
], { stdio: "ignore" });
writeFileSync(`${outDir}/chrome.pid`, String(chrome.pid));

let ws;
for (let i = 0; i < 50 && !ws; i++) {
  await sleep(200);
  try {
    const list = await (await fetch(`http://127.0.0.1:${port}/json/list`)).json();
    const page = list.find((t) => t.type === "page");
    if (page) ws = new WebSocket(page.webSocketDebuggerUrl);
  } catch {}
}
await new Promise((r) => ws.addEventListener("open", r, { once: true }));
let seq = 0;
const waiters = new Map();
const events = [];
ws.addEventListener("message", (m) => {
  const msg = JSON.parse(m.data);
  if (msg.id && waiters.has(msg.id)) { waiters.get(msg.id)(msg); waiters.delete(msg.id); }
  else if (msg.method) events.push(msg.method);
});
const send = (method, params = {}) => new Promise((resolve, reject) => {
  const id = ++seq;
  const timer = setTimeout(() => { waiters.delete(id); reject(new Error(`${method} timed out`)); }, 20000);
  waiters.set(id, (msg) => { clearTimeout(timer); msg.error ? reject(new Error(JSON.stringify(msg.error))) : resolve(msg.result); });
  ws.send(JSON.stringify({ id, method, params }));
});

await send("Page.enable");
await send("Emulation.setDeviceMetricsOverride", { width: 1920, height: 1080, deviceScaleFactor: 1, mobile: false });

const collect = `(() => {
  const sel = 'a[href], button, input:not([type=hidden]), select, textarea, [role=button], [role=link], [role=tab], [role=menuitem], [role=checkbox], summary';
  const out = [];
  for (const el of document.querySelectorAll(sel)) {
    const r = el.getBoundingClientRect();
    if (r.width < 6 || r.height < 6 || r.right <= 0 || r.bottom <= 0 || r.left >= innerWidth || r.top >= innerHeight) continue;
    const cs = getComputedStyle(el);
    if (cs.visibility !== 'visible' || +cs.opacity < 0.2) continue;
    const cx = r.left + r.width / 2, cy = r.top + r.height / 2;
    const top = document.elementFromPoint(Math.min(Math.max(cx, 0), innerWidth - 1), Math.min(Math.max(cy, 0), innerHeight - 1));
    if (!top || !(top === el || el.contains(top) || top.contains(el))) continue;
    const text = (el.innerText || el.value || '').trim().replace(/\\s+/g, ' ');
    const label = (el.getAttribute('aria-label') || el.getAttribute('title') || el.getAttribute('placeholder') || el.getAttribute('alt') || '').trim();
    const tag = el.tagName.toLowerCase(), role = el.getAttribute('role') || '', type = el.getAttribute('type') || '';
    out.push({ tag, role, type, text: text.slice(0, 80), label: label.slice(0, 80),
      box: [Math.max(0, r.left), Math.max(0, r.top), Math.min(innerWidth, r.right), Math.min(innerHeight, r.bottom)] });
  }
  return out;
})()`;

const pages = [];
for (const [i, url] of urls.entries()) {
  const name = String(i).padStart(2, "0");
  try {
    events.length = 0;
    await send("Page.navigate", { url });
    for (let t = 0; t < 100 && !events.includes("Page.loadEventFired"); t++) await sleep(150);
    await sleep(2500);
    const { result } = await send("Runtime.evaluate", { expression: collect, returnByValue: true });
    const shot = await send("Page.captureScreenshot", { format: "png" });
    writeFileSync(`${outDir}/shots/${name}.png`, Buffer.from(shot.data, "base64"));
    pages.push({ name, url, shot: `shots/${name}.png`, elements: result.value });
    console.log(name, url, result.value.length, "elements");
    writeFileSync(`${outDir}/pages.json`, JSON.stringify(pages, null, 1));
  } catch (e) {
    console.log(name, url, "FAILED", e.message);
  }
}
writeFileSync(`${outDir}/pages.json`, JSON.stringify(pages, null, 1));
ws.close();
chrome.kill("SIGTERM");
