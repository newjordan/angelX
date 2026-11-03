#!/usr/bin/env node
// The Delve parity gate: drive a rules worker through a recorded corpus and
// report where its state first departs from the recording.
//
//   node scripts/delve/replay.mjs <worker-binary> <corpus-dir-or-file>
//        [--scenario name[,name]] [--from N] [--max-diffs K] [--timeout ms]
//        [--worker-arg A] [--no-restore-check] [--quiet]
//
// A corpus file is `<name>.jsonl.gz`: line 1 `{"kind":"restore","state":S}`,
// then `{"kind":"apply","before":S|null,"event":E,"after":S}` a tick (see
// scripts/delve/README.md). The worker speaks the framed protocol of
// crates/live/src/system.rs: prepare, restore, activate, apply..., dispose. If
// its prepare reply advertises "codec":"bj1" every later frame is BJ1; if it
// advertises "resident":true, apply carries only the delta from the state the
// worker holds. Exits nonzero on the first divergence of any scenario.
import { createReadStream, readdirSync, statSync } from 'node:fs';
import { basename, join } from 'node:path';
import { createInterface } from 'node:readline';
import { performance } from 'node:perf_hooks';
import { pipeline, PassThrough } from 'node:stream';
import { createGunzip } from 'node:zlib';
import { Worker, applyOps, compare, diff, fields, parse, show } from './replay-lib.mjs';

function usage(why) {
  if (why) console.error(`replay: ${why}`);
  console.error('usage: replay.mjs <worker-binary> <corpus-dir-or-file> [--scenario name[,name]] [--from N] [--max-diffs K] [--timeout ms] [--worker-arg A] [--no-restore-check] [--quiet]');
  process.exit(2);
}

function options(argv) {
  const o = { scenario: [], from: 0, maxDiffs: 3, timeout: 120000, restoreCheck: true, quiet: false, args: [] };
  const rest = [];
  for (let i = 0; i < argv.length; i++) {
    const a = argv[i];
    const val = () => argv[++i] ?? usage(`${a} needs a value`);
    if (a === '--scenario') o.scenario.push(...val().split(','));
    else if (a === '--from') o.from = Number(val());
    else if (a === '--max-diffs') o.maxDiffs = Number(val());
    else if (a === '--timeout') o.timeout = Number(val());
    else if (a === '--worker-arg') o.args.push(val());
    else if (a === '--no-restore-check') o.restoreCheck = false;
    else if (a === '--quiet') o.quiet = true;
    else if (a.startsWith('--')) usage(`unknown option ${a}`);
    else rest.push(a);
  }
  if (rest.length !== 2) usage();
  [o.worker, o.corpus] = rest;
  if (!Number.isInteger(o.from) || o.from < 0) usage('--from wants a tick number');
  if (!Number.isInteger(o.maxDiffs) || o.maxDiffs < 1) usage('--max-diffs wants a count');
  return o;
}

/** Lines of a gzip'd (or plain) JSONL file; a truncated or unreadable file throws. */
function lines(file) {
  const input = new PassThrough();
  const stages = file.endsWith('.gz') ? [createReadStream(file), createGunzip(), input] : [createReadStream(file), input];
  pipeline(stages, (err) => err && input.destroy(err));
  return createInterface({ input, crlfDelay: Infinity });
}

const isStateText = (t) => /^\s*\{/.test(t) && fields(t, ['schema', 'run']).run !== undefined;

/** Replay one scenario file. Returns {name, ticks, total, ms, applied, failure}. */
async function replay(file, o) {
  const name = basename(file).replace(/\.jsonl(\.gz)?$/, '');
  const out = { name, ticks: 0, total: 0, ms: 0, applied: 0, failure: null };
  const worker = new Worker(o.worker, o.args, o.timeout);
  let tick = -1;
  const fail = (what, diffs = []) => {
    out.failure = { tick, what, diffs };
  };
  try {
    const prepared = JSON.parse(await worker.call('prepare', '{"id":"delve","api":1}'));
    const resident = prepared.resident === true;
    const bj1 = prepared.codec === 'bj1';
    if (prepared.codec !== undefined && !bj1) throw new Error(`unknown codec ${prepared.codec}`);
    let stateText = null; // the state the host holds, as text
    let phrasebook = null; // the event's book, carried forward
    let held = null; // resident mode: the tree the worker holds
    let started = false;
    let index = -1; // the corpus line's tick number (restore is -1)
    const begin = async (text) => {
      if (bj1) worker.bj1 = true;
      tick = index;
      const answer = await worker.call('restore', text);
      if (resident || (o.restoreCheck && isStateText(answer))) {
        const asked = parse(text);
        const had = isStateText(answer) ? parse(answer) : asked;
        if (o.restoreCheck && isStateText(answer)) {
          const diffs = compare(asked, had, o.maxDiffs);
          if (diffs.length) return fail('the restore reply differs from the state sent', diffs);
        }
        held = had;
      }
      await worker.call('activate', 'null');
      started = true;
    };
    // Dumb first: bj1 starts after prepare, which was plain JSON.
    for await (const line of lines(file)) {
      if (!line.trim()) continue;
      const f = fields(line, ['kind', 'state', 'before', 'event', 'after']);
      const kind = JSON.parse(f.kind);
      if (kind === 'restore') {
        stateText = f.state;
        if (o.from === 0) {
          index = -1;
          await begin(stateText);
          if (out.failure) break;
        }
        continue;
      }
      index++;
      out.total++;
      const before = f.before === 'null' || f.before === undefined ? stateText : f.before;
      if (index < o.from) {
        stateText = f.after;
        const e = fields(f.event, ['phrasebook']);
        if (e.phrasebook !== undefined) phrasebook = e.phrasebook;
        continue;
      }
      if (!started) {
        stateText = before;
        await begin(before);
        if (out.failure) break;
      }
      tick = index;
      let event = f.event;
      const book = fields(event, ['phrasebook']).phrasebook;
      if (book !== undefined) phrasebook = book;
      else if (phrasebook !== null) event = `${event.slice(0, event.lastIndexOf('}'))},"phrasebook":${phrasebook}}`;
      // The request.
      let body;
      let beforeTree = null;
      if (resident) {
        beforeTree = parse(before);
        const ops = diff(held, beforeTree);
        body = `{"delta":${show(ops)},"event":${event}}`;
      } else {
        body = `{"state":${before},"event":${event}}`;
      }
      const t0 = performance.now();
      const answer = await worker.call('apply', body);
      out.ms += performance.now() - t0;
      out.applied++;
      // The reply.
      const expected = f.after;
      let got;
      if (resident) {
        const r = parse(answer);
        if (!r || r.delta === undefined) throw new Error(`a resident apply reply wants a delta: ${answer.slice(0, 200)}`);
        got = applyOps(beforeTree, r.delta);
        held = got;
        const diffs = compare(parse(expected), got, o.maxDiffs);
        if (diffs.length) {
          fail('state after the worker\'s delta differs from the recording', diffs);
          break;
        }
      } else {
        const r = fields(answer, ['state']);
        if (r.state === undefined) throw new Error(`apply reply has no state: ${answer.slice(0, 200)}`);
        if (r.state !== expected) {
          const diffs = compare(parse(expected), parse(r.state), o.maxDiffs);
          if (diffs.length) {
            fail('state after the tick differs from the recording', diffs);
            break;
          }
        }
      }
      stateText = expected;
      out.ticks++;
    }
    if (!out.failure && !started && out.total === 0) fail('the corpus file has no ticks');
  } catch (e) {
    fail(e.message);
  } finally {
    // A tidy goodbye, so the worker leaves without a complaint about its pipe.
    worker.timeout = 2000;
    await worker.call('dispose', 'null').catch(() => {});
    worker.stop();
  }
  return out;
}

function corpusFiles(path, only) {
  const st = statSync(path);
  const all = st.isDirectory()
    ? readdirSync(path)
        .filter((f) => f.endsWith('.jsonl.gz') || f.endsWith('.jsonl'))
        .sort()
        .map((f) => join(path, f))
    : [path];
  if (!only.length) return all;
  const names = new Map(all.map((f) => [basename(f).replace(/\.jsonl(\.gz)?$/, ''), f]));
  return only.map((n) => names.get(n) ?? usage(`no scenario ${n} in ${path}`));
}

const o = options(process.argv.slice(2));
const files = corpusFiles(o.corpus, o.scenario);
if (!files.length) usage(`no .jsonl.gz files in ${o.corpus}`);
let bad = 0;
let ticks = 0;
let ms = 0;
let applied = 0;
const started = performance.now();
for (const file of files) {
  const r = await replay(file, o);
  ticks += r.ticks;
  ms += r.ms;
  applied += r.applied;
  const per = r.applied ? (r.ms / r.applied).toFixed(2) : '-';
  if (r.failure) {
    bad++;
    const at = r.failure.tick < 0 ? 'restore' : `tick ${r.failure.tick}`;
    console.log(`FAIL  ${r.name}  ${r.ticks}/${r.total || '?'} ticks passed, ${per} ms/apply`);
    console.log(`      diverges at ${at}: ${r.failure.what}`);
    for (const d of r.failure.diffs) {
      console.log(`      ${d.path}  (${d.why})`);
      console.log(`        expected ${d.expected}`);
      console.log(`        got      ${d.got}`);
    }
  } else if (!o.quiet) {
    console.log(`ok    ${r.name}  ${r.ticks}/${r.total} ticks, ${per} ms/apply`);
  }
}
const secs = ((performance.now() - started) / 1000).toFixed(1);
console.log(
  `${bad ? 'FAILED' : 'PASSED'}  ${files.length - bad}/${files.length} scenarios, ${ticks} ticks replayed, ` +
    `${applied ? (ms / applied).toFixed(2) : '-'} ms/apply, ${secs}s`,
);
process.exit(bad ? 1 : 0);
