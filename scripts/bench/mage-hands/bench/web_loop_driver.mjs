// CDP driver for the closed-loop web bench: one isolated headless Chrome, a
// screenshot/click/eval control surface on 127.0.0.1, so a Python scorer can
// ask Mage for a point, click it for real and check what the page did.
//   node web_loop_driver.mjs <work-dir> [control-port] [scale]
// Nothing here touches the operator's Chrome profile, pointer or clipboard.
import { spawn } from "node:child_process";
import { mkdirSync, writeFileSync } from "node:fs";
import { createServer } from "node:http";
import { setTimeout as sleep } from "node:timers/promises";

//   node web_loop_driver.mjs <work-dir> [control-port] [scale] [WxH]
// `scale` is the device-pixel ratio (screenshot size), `WxH` the CSS viewport
// (layout size), so a laptop frame and a HiDPI frame are both reachable.
const [workDir, controlArg, scaleArg, viewArg] = process.argv.slice(2);
if (!workDir) {
  console.error("usage: node web_loop_driver.mjs <work-dir> [control-port] [scale] [WxH]");
  process.exit(2);
}
const controlPort = Number(controlArg || 9451);
const scale = Number(scaleArg || 1);
const vm = /^(\d+)x(\d+)$/.exec(viewArg || "");
const VIEW = { width: vm ? Number(vm[1]) : 1920, height: vm ? Number(vm[2]) : 1080 };
const CDP_PORT = controlPort + 1;
mkdirSync(`${workDir}/profile`, { recursive: true });

const chrome = spawn("google-chrome-stable", [
  "--headless=new", `--remote-debugging-port=${CDP_PORT}`,
  `--user-data-dir=${workDir}/profile`, "--no-first-run", "--no-default-browser-check",
  `--window-size=${VIEW.width},${VIEW.height}`, "--hide-scrollbars", "--mute-audio",
  "--disable-gpu", "--force-device-scale-factor=1", "about:blank",
], { stdio: ["ignore", "ignore", "pipe"] });
let chromeErr = "";
chrome.stderr.on("data", (d) => { chromeErr = (chromeErr + d).slice(-4000); });
writeFileSync(`${workDir}/chrome.pid`, String(chrome.pid));

let ws;
for (let i = 0; i < 75 && !ws; i++) {
  await sleep(200);
  try {
    const list = await (await fetch(`http://127.0.0.1:${CDP_PORT}/json/list`)).json();
    const page = list.find((t) => t.type === "page");
    if (page) ws = new WebSocket(page.webSocketDebuggerUrl);
  } catch {}
}
if (!ws) { console.error("chrome did not expose a page target\n" + chromeErr); process.exit(1); }
await new Promise((r, j) => { ws.addEventListener("open", r, { once: true }); ws.addEventListener("error", j, { once: true }); });

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
  const timer = setTimeout(() => { waiters.delete(id); reject(new Error(`${method} timed out`)); }, 30000);
  waiters.set(id, (msg) => { clearTimeout(timer); msg.error ? reject(new Error(JSON.stringify(msg.error))) : resolve(msg.result); });
  ws.send(JSON.stringify({ id, method, params }));
});

await send("Page.enable");
await send("Runtime.enable");
await send("Emulation.setDeviceMetricsOverride", {
  width: VIEW.width, height: VIEW.height, deviceScaleFactor: scale, mobile: false,
});

// The same element collector capture.mjs uses: visible, named, clickable, and
// on top at its own centre (so truth is a real click surface, not a hidden node).
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
    const text = (el.innerText || el.value || '').trim();
    const label = (el.getAttribute('aria-label') || el.getAttribute('title') || el.getAttribute('placeholder') || el.getAttribute('alt') || '').trim();
    out.push({
      tag: el.tagName.toLowerCase(), role: el.getAttribute('role') || '', type: el.getAttribute('type') || '',
      id: el.id || '', text: text.slice(0, 80), label: label.slice(0, 80),
      href: el.getAttribute('href') || '',
      target: el.getAttribute('target') || '',
      box: [Math.max(0, r.left), Math.max(0, r.top), Math.min(innerWidth, r.right), Math.min(innerHeight, r.bottom)],
    });
  }
  return out;
})()`;

const page = async (expression) =>
  (await send("Runtime.evaluate", { expression, returnByValue: true, awaitPromise: true })).result.value;

const handlers = {
  async health() { return { ok: true, scale, view: VIEW }; },
  async elements() { return { url: await page("location.href"), elements: await page(collect) }; },
  async shot() {
    const shot = await send("Page.captureScreenshot", { format: "png", captureBeyondViewport: false });
    return { png: shot.data, scale, view: VIEW };
  },
  async nav({ url }) {
    events.length = 0;
    await send("Page.navigate", { url });
    for (let t = 0; t < 120 && !events.includes("Page.loadEventFired"); t++) await sleep(150);
    await sleep(600);
    return { url: await page("location.href") };
  },
  async click({ x, y, button = "left" }) {
    const base = { x, y, button, buttons: 1, clickCount: 1, pointerType: "mouse" };
    await send("Input.dispatchMouseEvent", { type: "mouseMoved", x, y, button: "none", buttons: 0, pointerType: "mouse" });
    await send("Input.dispatchMouseEvent", { type: "mousePressed", ...base });
    await send("Input.dispatchMouseEvent", { type: "mouseReleased", ...base, buttons: 0 });
    return { clicked: [x, y] };
  },
  async eval({ expr }) { return { value: await page(expr) }; },
  // The keyboard channel. Every act measured so far was a click; a controller
  // that types has to get the *focus* right first, and the only truth about
  // focus is document.activeElement. `type` sends real key events (keyDown with
  // text, keyUp), not insertText, so page handlers see what a user typed.
  async type({ text = "", delay = 0 }) {
    for (const ch of text) {
      await send("Input.dispatchKeyEvent", { type: "keyDown", text: ch, key: ch });
      await send("Input.dispatchKeyEvent", { type: "keyUp", key: ch });
      if (delay) await sleep(delay);
    }
    return { typed: text };
  },
  async focus() {
    return { value: await page(`(() => { const el = document.activeElement;
      if (!el || el === document.body) return { id: '', tag: el ? el.tagName.toLowerCase() : 'none' };
      const r = el.getBoundingClientRect();
      return { id: el.id || '', tag: el.tagName.toLowerCase(),
               value: (el.value === undefined ? '' : String(el.value)).slice(0, 200),
               box: [Math.round(r.left), Math.round(r.top), Math.round(r.right), Math.round(r.bottom)] }; })()`) };
  },
  async blur() { return { value: await page("(document.activeElement && document.activeElement.blur && document.activeElement.blur(), document.activeElement.tagName)") }; },
  async probe({ x, y }) {
    return { value: await page(`(() => { const el = document.elementFromPoint(${x}, ${y});
      if (!el) return null; const r = el.getBoundingClientRect();
      return { id: el.id || '', tag: el.tagName.toLowerCase(),
               text: (el.innerText || el.value || '').trim().slice(0, 40),
               box: [Math.round(r.left), Math.round(r.top), Math.round(r.right), Math.round(r.bottom)] }; })()`) };
  },
  // Real pages can answer a click by opening a *new* tab, which is a real
  // answer but not a change to location.href. Count page targets and close
  // the extras, so "did the click do anything" is not URL-only.
  async targets() {
    const list = await (await fetch(`http://127.0.0.1:${CDP_PORT}/json/list`)).json();
    return { pages: list.filter((t) => t.type === "page").map((t) => ({ id: t.id, url: t.url })) };
  },
  async closetabs() {
    const list = await (await fetch(`http://127.0.0.1:${CDP_PORT}/json/list`)).json();
    const mine = list.find((t) => t.type === "page" && ws && t.webSocketDebuggerUrl === ws.url);
    const keep = mine ? mine.id : list.find((t) => t.type === "page").id;
    let closed = 0;
    for (const t of list.filter((t) => t.type === "page" && t.id !== keep)) {
      await fetch(`http://127.0.0.1:${CDP_PORT}/json/close/${t.id}`).catch(() => {});
      closed++;
    }
    return { closed };
  },
  async close() { setTimeout(() => { chrome.kill("SIGTERM"); process.exit(0); }, 50); return { ok: true }; },
};

const server = createServer(async (req, res) => {
  let body = "";
  for await (const chunk of req) body += chunk;
  const name = req.url === "/health" ? "health" : req.url.replace(/^\//, "");
  const fn = handlers[name];
  const reply = (code, obj) => { res.writeHead(code, { "content-type": "application/json" }); res.end(JSON.stringify(obj)); };
  if (!fn) return reply(404, { error: `no route ${req.url}` });
  try {
    reply(200, await fn(body ? JSON.parse(body) : {}));
  } catch (e) {
    reply(500, { error: String(e && e.message || e), chrome: chromeErr.slice(-1500) });
  }
});
server.listen(controlPort, "127.0.0.1", () => console.log(`driver ${controlPort} scale ${scale} pid ${process.pid}`));

for (const sig of ["SIGINT", "SIGTERM"]) process.on(sig, () => { chrome.kill("SIGTERM"); process.exit(0); });
chrome.on("exit", (code) => { console.error(`chrome exited ${code}\n${chromeErr.slice(-1500)}`); process.exit(1); });
