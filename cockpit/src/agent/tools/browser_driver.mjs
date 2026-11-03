// angel browser driver: Chromium CDP over the global WebSocket (Node >= 22).
// Zero npm dependencies. Protocol: one JSON command per stdin line, one JSON
// reply per stdout line: {id, ok, result|error}. Spawned by the `browser`
// tool with the CDP port as argv[2]; Chromium itself is owned by Rust.
import { createInterface } from "node:readline";
import { writeFile } from "node:fs/promises";

const port = process.argv[2];
let ws = null;
let seq = 0;
const pending = new Map();

function send(obj) { process.stdout.write(JSON.stringify(obj) + "\n"); }

async function connect() {
  const deadline = Date.now() + 15000;
  for (;;) {
    try {
      const targets = await (await fetch(`http://127.0.0.1:${port}/json/list`)).json();
      const page = targets.find((t) => t.type === "page" && t.webSocketDebuggerUrl);
      if (page) {
        ws = new WebSocket(page.webSocketDebuggerUrl);
        break;
      }
    } catch {}
    if (Date.now() > deadline) throw new Error("no CDP page target on port " + port);
    await new Promise((r) => setTimeout(r, 250));
  }
  await new Promise((res, rej) => { ws.onopen = res; ws.onerror = rej; });
  ws.onmessage = (ev) => {
    const msg = JSON.parse(ev.data);
    if (msg.id && pending.has(msg.id)) {
      const { resolve, reject } = pending.get(msg.id);
      pending.delete(msg.id);
      if (msg.error) reject(new Error(msg.error.message || "CDP error"));
      else resolve(msg.result);
    }
  };
  await cdp("Page.enable");
  await cdp("Runtime.enable");
}

function cdp(method, params = {}) {
  const id = ++seq;
  return new Promise((resolve, reject) => {
    pending.set(id, { resolve, reject });
    ws.send(JSON.stringify({ id, method, params }));
    setTimeout(() => { if (pending.delete(id)) reject(new Error(method + " timed out")); }, 30000);
  });
}

async function evaluate(expression) {
  const r = await cdp("Runtime.evaluate", { expression, returnByValue: true, awaitPromise: true });
  if (r.exceptionDetails) throw new Error(r.exceptionDetails.text || "page exception");
  return r.result ? r.result.value : undefined;
}

const SNAPSHOT_JS = `(() => {
  const els = [...document.querySelectorAll('a,button,input,select,textarea,[role="button"],[onclick]')]
    .filter(e => e.offsetParent !== null || e === document.activeElement)
    .slice(0, 60)
    .map((e, i) => {
      const label = (e.innerText || e.value || e.getAttribute('aria-label') || e.placeholder || e.name || e.id || '').trim().replace(/\\s+/g, ' ').slice(0, 60);
      const sel = e.id ? '#' + CSS.escape(e.id) : e.tagName.toLowerCase() + (e.name ? '[name="' + e.name + '"]' : '') + ':nth-of-type(' + ([...e.parentElement ? e.parentElement.children : []].indexOf(e) + 1) + ')';
      return '[' + i + '] <' + e.tagName.toLowerCase() + (e.type ? ' type=' + e.type : '') + '> ' + JSON.stringify(label) + ' sel=' + sel + (e.href ? ' href=' + e.href : '');
    }).join('\\n');
  const text = (document.body ? document.body.innerText : '').replace(/\\n{3,}/g, '\\n\\n').slice(0, 4000);
  return 'title: ' + document.title + '\\nurl: ' + location.href + '\\n--- interactive ---\\n' + (els || '(none)') + '\\n--- text ---\\n' + text;
})()`;

async function handle(cmd) {
  switch (cmd.action) {
    case "goto": {
      await cdp("Page.navigate", { url: cmd.url });
      await new Promise((r) => setTimeout(r, cmd.wait_ms ?? 1200));
      return await evaluate(SNAPSHOT_JS);
    }
    case "snapshot": return await evaluate(SNAPSHOT_JS);
    case "click": {
      const found = await evaluate(`(() => {
        const sel = ${JSON.stringify("SEL")}, text = ${JSON.stringify("TEXT")};
        let el = sel ? document.querySelector(sel) : null;
        if (!el && text) el = [...document.querySelectorAll('a,button,input,[role="button"]')]
          .find(e => ((e.innerText || e.value || '')).trim() === text) ||
          [...document.querySelectorAll('a,button,input,[role="button"]')]
          .find(e => ((e.innerText || e.value || '')).includes(text));
        if (!el) return 'not found';
        el.scrollIntoView({block: 'center'}); el.click(); return 'clicked ' + el.tagName.toLowerCase();
      })()`.replace('"SEL"', JSON.stringify(cmd.selector ?? "")).replace('"TEXT"', JSON.stringify(cmd.text ?? "")));
      if (found === "not found") throw new Error("click target not found: " + (cmd.selector || cmd.text));
      await new Promise((r) => setTimeout(r, cmd.wait_ms ?? 800));
      return found + "\n" + (await evaluate(SNAPSHOT_JS));
    }
    case "type": {
      const r = await evaluate(`(() => {
        const el = document.querySelector(${JSON.stringify(cmd.selector)});
        if (!el) return 'not found';
        el.focus();
        const proto = el instanceof HTMLTextAreaElement ? HTMLTextAreaElement.prototype : HTMLInputElement.prototype;
        const setter = Object.getOwnPropertyDescriptor(proto, 'value')?.set;
        if (setter) setter.call(el, ${JSON.stringify(cmd.text)}); else el.value = ${JSON.stringify(cmd.text)};
        el.dispatchEvent(new Event('input', {bubbles: true}));
        el.dispatchEvent(new Event('change', {bubbles: true}));
        return 'typed into ' + el.tagName.toLowerCase();
      })()`);
      if (r === "not found") throw new Error("type target not found: " + cmd.selector);
      return r;
    }
    case "press": {
      const key = cmd.key;
      await cdp("Input.dispatchKeyEvent", { type: "keyDown", key, code: key, text: key.length === 1 ? key : undefined });
      await cdp("Input.dispatchKeyEvent", { type: "keyUp", key, code: key });
      await new Promise((r) => setTimeout(r, cmd.wait_ms ?? 500));
      return "pressed " + key + "\n" + (await evaluate(SNAPSHOT_JS));
    }
    case "eval": {
      const v = await evaluate(cmd.script);
      return typeof v === "string" ? v.slice(0, 8000) : JSON.stringify(v)?.slice(0, 8000) ?? "undefined";
    }
    case "screenshot": {
      const r = await cdp("Page.captureScreenshot", { format: "png" });
      await writeFile(cmd.path, Buffer.from(r.data, "base64"));
      return "browser screenshot saved: " + cmd.path;
    }
    default: throw new Error("unknown browser action " + cmd.action);
  }
}

try {
  await connect();
  send({ id: 0, ok: true, result: "ready" });
} catch (e) {
  send({ id: 0, ok: false, error: String(e.message || e) });
  process.exit(1);
}

const rl = createInterface({ input: process.stdin });
for await (const line of rl) {
  if (!line.trim()) continue;
  let cmd;
  try { cmd = JSON.parse(line); } catch { continue; }
  try { send({ id: cmd.id, ok: true, result: await handle(cmd) }); }
  catch (e) { send({ id: cmd.id, ok: false, error: String(e.message || e) }); }
}
