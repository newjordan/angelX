#!/usr/bin/env node
import { Worker, applyOps, diff, f32bits, f32from, fields, fromBj1, parse, show, toBj1, Fl, API } from './replay-lib.mjs';

const rust = new Worker(process.env.SHIM_WORKER, [], 120000);
const bj1 = process.env.SHIM_BJ1 === '1';
const resident = process.env.SHIM_RESIDENT === '1';
const breakAt = process.env.SHIM_BREAK ? Number(process.env.SHIM_BREAK) : -1;
let live = false;
let held = null;
let applies = 0;

let buf = Buffer.alloc(0);
const frames = [];
let wake = null;
process.stdin.on('data', (d) => {
  buf = Buffer.concat([buf, d]);
  while (buf.length >= 4 && buf.length >= 4 + buf.readUInt32BE(0)) {
    const size = buf.readUInt32BE(0);
    frames.push(buf.toString('utf8', 4, 4 + size));
    buf = buf.subarray(4 + size);
  }
  wake?.();
});
process.stdin.on('end', () => {
  frames.push(null);
  wake?.();
});
const next = async () => {
  while (!frames.length) await new Promise((r) => (wake = r));
  return frames.shift();
};

function reply(request, generation, ok, err) {
  let text = `{"api":${API},"generation":${JSON.stringify(generation)},"request":${request},"result":${
    err === undefined ? `{"Ok":${ok}}` : `{"Err":${JSON.stringify(err)}}`
  }}`;
  if (live && bj1) text = toBj1(text);
  const payload = Buffer.from(text, 'utf8');
  const head = Buffer.alloc(4);
  head.writeUInt32BE(payload.length);
  process.stdout.write(Buffer.concat([head, payload]));
}

function nudge(tree) {
  if (tree instanceof Fl) {
    tree.v = f32from(f32bits(tree.v) ^ 1);
    return true;
  }
  if (tree && typeof tree === 'object') {
    for (const k of Object.keys(tree)) if (nudge(tree[k])) return true;
  }
  return false;
}

for (;;) {
  let text = await next();
  if (text === null) break;
  if (live && bj1) text = fromBj1(text);
  const f = fields(text, ['generation', 'request', 'method', 'body']);
  const request = Number(f.request);
  const generation = JSON.parse(f.generation);
  const method = JSON.parse(f.method);
  try {
    if (method === 'prepare') {
      const answer = JSON.parse(await rust.call('prepare', f.body));
      if (bj1) answer.codec = 'bj1';
      if (resident) answer.resident = true;
      reply(request, generation, JSON.stringify(answer));
      live = true;
    } else if (method === 'restore') {
      const answer = await rust.call('restore', f.body);
      if (resident) held = parse(answer);
      reply(request, generation, answer);
    } else if (method === 'apply') {
      applies++;
      let body = f.body;
      let sent = null;
      if (resident) {
        const b = fields(f.body, ['delta', 'event']);
        sent = applyOps(held, parse(b.delta));
        body = `{"state":${show(sent)},"event":${b.event}}`;
      }
      const answer = await rust.call('apply', body);
      const out = fields(answer, ['state', 'output']);
      let tree = null;
      if (resident || applies === breakAt) tree = parse(out.state);
      if (applies === breakAt) nudge(tree.run);
      if (resident) {
        const ops = diff(sent, tree);
        held = tree;
        reply(request, generation, `{"delta":${show(ops)},"output":${out.output ?? 'null'}}`);
      } else {
        reply(request, generation, `{"state":${tree ? show(tree) : out.state},"output":${out.output ?? 'null'}}`);
      }
    } else {
      reply(request, generation, await rust.call(method, f.body));
    }
  } catch (e) {
    reply(request, generation, undefined, e.message);
  }
  if (method === 'dispose') break;
}
rust.stop();
process.exit(0);
