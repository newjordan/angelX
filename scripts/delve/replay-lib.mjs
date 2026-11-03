// The pieces of the Delve replay gate that a worker, a shim and a test share:
// a JSON reader that keeps integers and floats apart, the BJ1 number codec,
// the state-delta ops, and the semantic comparison.
//
// A parsed state is plain JS: objects (null-prototype), arrays, strings,
// booleans, null; integers as numbers (BigInt beyond 2^53); floats as `Fl`.

import { spawn } from 'node:child_process';

export const API = 1;

export class Fl {
  constructor(v) {
    this.v = v;
  }
}

const SAFE = BigInt(Number.MAX_SAFE_INTEGER);

/** Read JSON text into a tree that keeps integer and float tokens apart. */
export function parse(text) {
  let i = 0;
  const n = text.length;
  const fail = (what) => {
    throw new SyntaxError(`${what} at ${i}: ${JSON.stringify(text.slice(Math.max(0, i - 20), i + 20))}`);
  };
  const ws = () => {
    for (;;) {
      const c = text.charCodeAt(i);
      if (c === 32 || c === 10 || c === 13 || c === 9) i++;
      else return;
    }
  };
  const str = () => {
    // text[i] is the opening quote
    let j = i + 1;
    let plain = true;
    for (;;) {
      const c = text.charCodeAt(j);
      if (c === 34) break;
      if (c === 92) {
        plain = false;
        j += 2;
        continue;
      }
      if (Number.isNaN(c)) fail('unterminated string');
      j++;
    }
    const raw = text.slice(i, j + 1);
    i = j + 1;
    return plain ? raw.slice(1, -1) : JSON.parse(raw);
  };
  const value = () => {
    ws();
    const c = text.charCodeAt(i);
    if (c === 123) {
      i++;
      const o = Object.create(null);
      ws();
      if (text.charCodeAt(i) === 125) {
        i++;
        return o;
      }
      for (;;) {
        ws();
        if (text.charCodeAt(i) !== 34) fail('expected key');
        const k = str();
        ws();
        if (text.charCodeAt(i) !== 58) fail('expected :');
        i++;
        o[k] = value();
        ws();
        const d = text.charCodeAt(i++);
        if (d === 44) continue;
        if (d === 125) return o;
        fail('expected , or }');
      }
    }
    if (c === 91) {
      i++;
      const a = [];
      ws();
      if (text.charCodeAt(i) === 93) {
        i++;
        return a;
      }
      for (;;) {
        a.push(value());
        ws();
        const d = text.charCodeAt(i++);
        if (d === 44) continue;
        if (d === 93) return a;
        fail('expected , or ]');
      }
    }
    if (c === 34) return str();
    if (c === 116 && text.startsWith('true', i)) return ((i += 4), true);
    if (c === 102 && text.startsWith('false', i)) return ((i += 5), false);
    if (c === 110 && text.startsWith('null', i)) return ((i += 4), null);
    // a number
    let j = i;
    let float = false;
    for (; j < n; j++) {
      const d = text.charCodeAt(j);
      if ((d >= 48 && d <= 57) || d === 45 || d === 43) continue;
      if (d === 46 || d === 101 || d === 69) {
        float = true;
        continue;
      }
      break;
    }
    if (j === i) fail('unexpected token');
    const tok = text.slice(i, j);
    i = j;
    if (float) return new Fl(Number(tok));
    const v = Number(tok);
    if (Number.isSafeInteger(v)) return v;
    const b = BigInt(tok);
    return b <= SAFE && b >= -SAFE ? Number(b) : b;
  };
  const out = value();
  ws();
  if (i !== n) fail('trailing text');
  return out;
}

const f32 = new Float32Array(1);
const u32 = new Uint32Array(f32.buffer);
const f64 = new Float64Array(1);
const u64 = new BigUint64Array(f64.buffer);

/** The IEEE-754 binary32 bits of a number (rounded to f32 first). */
export function f32bits(v) {
  f32[0] = v;
  return u32[0];
}
export function f32from(bits) {
  u32[0] = bits;
  return f32[0];
}
export function f64bits(v) {
  f64[0] = v;
  return u64[0];
}
export const isF32 = (v) => Object.is(Math.fround(v), v);

/** JSON text for a float that reads back as a float with the same value. */
export function floatText(v) {
  if (Object.is(v, -0)) return '-0.0';
  if (Number.isInteger(v) && Math.abs(v) < 1e21) return `${v}.0`;
  const t = String(v);
  return /[.eE]/.test(t) ? t : `${t}.0`;
}

/** JSON text of a tree. */
export function show(v) {
  if (v === null) return 'null';
  switch (typeof v) {
    case 'boolean':
      return v ? 'true' : 'false';
    case 'number':
    case 'bigint':
      return String(v);
    case 'string':
      return JSON.stringify(v);
  }
  if (v instanceof Fl) return floatText(v.v);
  if (Array.isArray(v)) return `[${v.map(show).join(',')}]`;
  return `{${Object.keys(v)
    .map((k) => `${JSON.stringify(k)}:${show(v[k])}`)
    .join(',')}}`;
}

// ---- BJ1: JSON text whose number tokens Bend can read -----------------------

const LEX = /"(?:[^"\\]|\\.)*"|-?\d+(?:\.\d+)?(?:[eE][+-]?\d+)?|f[0-9a-f]{8}(?![0-9a-zA-Z])|w[0-9a-f]{16}(?![0-9a-zA-Z])/g;
const TWO32 = 4294967296n;
const TWO64 = 18446744073709551616n;

/** JSON text -> BJ1 text (floats `f`+8 hex, integers of 2^32 and up `w`+16 hex). */
export function toBj1(text) {
  return text.replace(LEX, (tok) => {
    const c = tok.charCodeAt(0);
    if (c === 34) return tok;
    if (/[.eE]/.test(tok)) {
      const v = Number(tok);
      if (!isF32(v)) throw new RangeError(`float ${tok} is not an f32`);
      return `f${f32bits(v).toString(16).padStart(8, '0')}`;
    }
    const b = BigInt(tok);
    if (b >= 0n && b < TWO32) return tok;
    if (b < 0n && b > -TWO32) return tok;
    if (b >= TWO32 && b < TWO64) return `w${b.toString(16).padStart(16, '0')}`;
    throw new RangeError(`integer ${tok} is outside BJ1`);
  });
}

/** BJ1 text -> JSON text. */
export function fromBj1(text) {
  return text.replace(LEX, (tok) => {
    const c = tok.charCodeAt(0);
    if (c === 102 && tok.length === 9) return floatText(f32from(parseInt(tok.slice(1), 16)));
    if (c === 119 && tok.length === 17) return BigInt(`0x${tok.slice(1)}`).toString();
    return tok;
  });
}

// ---- the byte spans of an object's fields, without parsing the values --------

/** Index just past the JSON value that starts at `i` (after spaces). */
export function skip(text, i) {
  while (text.charCodeAt(i) <= 32) i++;
  const c = text.charCodeAt(i);
  if (c === 34) {
    for (i++; ; i++) {
      const d = text.charCodeAt(i);
      if (d === 92) i++;
      else if (d === 34) return i + 1;
      else if (Number.isNaN(d)) throw new SyntaxError('unterminated string');
    }
  }
  if (c === 123 || c === 91) {
    let depth = 0;
    for (; ; i++) {
      const d = text.charCodeAt(i);
      if (d === 34) {
        i = skip(text, i) - 1;
      } else if (d === 123 || d === 91) depth++;
      else if (d === 125 || d === 93) {
        if (--depth === 0) return i + 1;
      } else if (Number.isNaN(d)) throw new SyntaxError('unterminated value');
    }
  }
  while (i < text.length && ![44, 125, 93, 32, 10, 13, 9].includes(text.charCodeAt(i))) i++;
  return i;
}

/** The text of fields `want` of the object `text`: {key: valueText}. */
export function fields(text, want) {
  const out = {};
  let left = want.length;
  let i = text.indexOf('{') + 1;
  for (;;) {
    while (text.charCodeAt(i) <= 32) i++;
    if (text.charCodeAt(i) === 125 || i >= text.length) return out;
    if (text.charCodeAt(i) === 44) {
      i++;
      continue;
    }
    const kEnd = skip(text, i);
    const key = JSON.parse(text.slice(i, kEnd));
    i = kEnd;
    while (text.charCodeAt(i) <= 32 || text.charCodeAt(i) === 58) i++;
    const vEnd = skip(text, i);
    if (want.includes(key)) {
      out[key] = text.slice(i, vEnd);
      if (--left === 0) return out;
    }
    i = vEnd;
  }
}

// ---- deltas: ["s", path, v], ["d", path], ["t", path, n] ---------------------

const same = (a, b) => {
  if (a === b) return true;
  if (a instanceof Fl && b instanceof Fl) return f64bits(a.v) === f64bits(b.v);
  return false;
};
const isObj = (v) => v !== null && typeof v === 'object' && !(v instanceof Fl) && !Array.isArray(v);

/** The ops that turn tree `a` into tree `b`. */
export function diff(a, b) {
  const ops = [];
  const walk = (x, y, path) => {
    if (same(x, y)) return;
    if (Array.isArray(x) && Array.isArray(y)) {
      const m = Math.min(x.length, y.length);
      for (let i = 0; i < m; i++) walk(x[i], y[i], [...path, i]);
      for (let i = m; i < y.length; i++) ops.push(['s', [...path, i], y[i]]);
      if (y.length < x.length) ops.push(['t', path, y.length]);
    } else if (isObj(x) && isObj(y)) {
      for (const k of Object.keys(x)) if (!(k in y)) ops.push(['d', [...path, k]]);
      for (const k of Object.keys(y)) {
        if (k in x) walk(x[k], y[k], [...path, k]);
        else ops.push(['s', [...path, k], y[k]]);
      }
    } else ops.push(['s', path, y]);
  };
  walk(a, b, []);
  return ops;
}

/** Apply ops to `root` in place; returns the new root. Throws on a bad op. */
export function applyOps(root, ops) {
  if (!Array.isArray(ops)) throw new Error('delta is not a list of ops');
  for (const op of ops) {
    if (!Array.isArray(op) || !Array.isArray(op[1])) throw new Error(`bad op ${show(op)}`);
    const [kind, path] = op;
    if (kind === 's' && path.length === 0) {
      root = op[2];
      continue;
    }
    let at = root;
    for (const step of path.slice(0, -1)) {
      at = Array.isArray(at) ? at[step] : isObj(at) ? at[step] : undefined;
      if (at === undefined) throw new Error(`op path ${show(path)} leaves the state`);
    }
    const last = path.at(-1);
    if (kind === 't') {
      const t = last === undefined ? at : at[last];
      if (!Array.isArray(t) || !Number.isInteger(op[2]) || op[2] > t.length) throw new Error(`bad truncate ${show(op)}`);
      t.length = op[2];
    } else if (kind === 'd') {
      if (!isObj(at) || !(last in at)) throw new Error(`delete of a missing key ${show(op)}`);
      delete at[last];
    } else if (kind === 's') {
      if (Array.isArray(at)) {
        if (!Number.isInteger(last) || last < 0 || last > at.length) throw new Error(`set past the end ${show(op)}`);
        at[last] = op[2];
      } else if (isObj(at) && typeof last === 'string') at[last] = op[2];
      else throw new Error(`bad set ${show(op)}`);
    } else throw new Error(`unknown op ${show(op)}`);
  }
  return root;
}

// ---- semantic comparison ------------------------------------------------------

const kindOf = (v) =>
  v === null
    ? 'null'
    : v instanceof Fl
      ? 'float'
      : Array.isArray(v)
        ? 'array'
        : typeof v === 'number' || typeof v === 'bigint'
          ? 'int'
          : typeof v;

const clip = (s, n = 90) => (s.length > n ? `${s.slice(0, n)}...(${s.length})` : s);

/** How a leaf reads in a report. */
export function describe(v) {
  if (v === undefined) return '(absent)';
  if (v instanceof Fl) {
    return isF32(v.v) ? `${floatText(v.v)} (f32 0x${f32bits(v.v).toString(16).padStart(8, '0')})` : `${floatText(v.v)} (f64)`;
  }
  return clip(show(v));
}

/** The path as `run.players.1.hp` / `run.enemies[3].x`. */
export function pathText(path) {
  return path.map((p, i) => (typeof p === 'number' ? `[${p}]` : i ? `.${p}` : p)).join('') || '(root)';
}

/**
 * Compare expected to got. Object key order is ignored; floats compare as f32
 * bits (so -0 and 0 differ), floats the f32 cannot hold as f64 bits; integers
 * exactly; a missing key and a null differ. Returns up to `limit` differences
 * {path, expected, got, why}.
 */
export function compare(want, got, limit = 1) {
  const out = [];
  const add = (path, why, e, g) => {
    out.push({ path: pathText(path), why, expected: describe(e), got: describe(g) });
    return out.length >= limit;
  };
  const walk = (e, g, path) => {
    const ek = kindOf(e);
    const gk = kindOf(g);
    if (ek !== gk) return add(path, `type: expected ${ek}, got ${gk}`, e, g);
    switch (ek) {
      case 'float': {
        const exact = isF32(e.v);
        const equal = exact ? f32bits(g.v) === f32bits(e.v) : f64bits(g.v) === f64bits(e.v);
        return equal ? false : add(path, 'float', e, g);
      }
      case 'array': {
        if (e.length !== g.length && add([...path, '(length)'], 'array length', e.length, g.length)) return true;
        for (let i = 0; i < Math.min(e.length, g.length); i++) if (walk(e[i], g[i], [...path, i])) return true;
        return false;
      }
      case 'object': {
        for (const k of Object.keys(e)) {
          if (!(k in g)) {
            if (add([...path, k], 'key missing from the worker state', e[k], undefined)) return true;
          } else if (walk(e[k], g[k], [...path, k])) return true;
        }
        for (const k of Object.keys(g)) if (!(k in e) && add([...path, k], 'key the expected state does not have', undefined, g[k])) return true;
        return false;
      }
      default:
        return e === g ? false : add(path, ek, e, g);
    }
  };
  walk(want, got, []);
  return out;
}

// ---- a worker process and its framed protocol ----------------------------------

export /** A worker process and its framed protocol. */
class Worker {
  constructor(binary, args, timeout) {
    this.timeout = timeout;
    this.child = spawn(binary, args, { stdio: ['pipe', 'pipe', 'inherit'] });
    this.chunks = [];
    this.have = 0;
    this.waiting = null;
    this.serial = 0;
    this.bj1 = false;
    this.exited = null;
    this.child.stdout.on('data', (d) => {
      this.chunks.push(d);
      this.have += d.length;
      this.pump();
    });
    this.child.on('exit', (code, signal) => {
      this.exited = signal ? `signal ${signal}` : `exit code ${code}`;
      this.pump();
    });
    this.child.on('error', (e) => {
      this.exited = `spawn failed: ${e.message}`;
      this.pump();
    });
    this.child.stdin.on('error', () => {});
  }

  /** Hand the waiting request its frame, or its worker's death. */
  pump() {
    if (!this.waiting) return;
    if (this.have >= 4) {
      const buf = Buffer.concat(this.chunks);
      const size = buf.readUInt32BE(0);
      if (buf.length >= 4 + size) {
        this.chunks = [buf.subarray(4 + size)];
        this.have = buf.length - 4 - size;
        const { resolve, timer } = this.waiting;
        this.waiting = null;
        clearTimeout(timer);
        resolve(buf.toString('utf8', 4, 4 + size));
        return;
      }
    }
    if (this.exited) {
      const { reject, timer } = this.waiting;
      this.waiting = null;
      clearTimeout(timer);
      reject(new Error(`the worker stopped (${this.exited})`));
    }
  }

  /** One request: body is JSON text. Resolves the `result` text, rejects on Err. */
  async call(method, bodyText) {
    const request = ++this.serial;
    let text = `{"api":${API},"generation":"replay","request":${request},"method":${JSON.stringify(method)},"body":${bodyText}}`;
    if (this.bj1) text = toBj1(text);
    const payload = Buffer.from(text, 'utf8');
    const head = Buffer.alloc(4);
    head.writeUInt32BE(payload.length);
    const answer = new Promise((resolve, reject) => {
      const timer = setTimeout(() => {
        this.waiting = null;
        reject(new Error(`no reply to ${method} in ${this.timeout} ms`));
      }, this.timeout);
      this.waiting = { resolve, reject, timer };
    });
    if (this.exited) throw new Error(`the worker stopped (${this.exited})`);
    this.child.stdin.write(Buffer.concat([head, payload]));
    this.pump();
    let reply = await answer;
    if (this.bj1) reply = fromBj1(reply);
    const f = fields(reply, ['request', 'result']);
    if (Number(f.request) !== request) throw new Error(`reply to request ${f.request}, wanted ${request}`);
    if (f.result === undefined) throw new Error(`reply to ${method} has no result: ${reply.slice(0, 200)}`);
    const result = fields(f.result, ['Ok', 'Err']);
    if (result.Err !== undefined) throw new Error(`worker error on ${method}: ${JSON.parse(result.Err)}`);
    if (result.Ok === undefined) throw new Error(`reply to ${method} has no result: ${reply.slice(0, 200)}`);
    return result.Ok;
  }

  stop() {
    try {
      this.child.stdin.end();
    } catch {}
    setTimeout(() => this.child.kill('SIGKILL'), 2000).unref();
  }
}
