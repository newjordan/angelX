# Delve in Bend

Delve's rules, world and state are one Bend program under `delve/` at the repo
root. Compiled native (`bend delve/main.bend -o <bin>`) it is the rules worker:
the binary the host's system supervisor runs in the `ANGEL_DELVE_SYSTEM` slot,
hot-swapped by generation with state handoff. Compiled to an ES module, the same
source is the browser's rules. Rust keeps Glass (drawing), Link (networking) and
the Kernel (process supervision, the wire codec below).

## The worker protocol

Unchanged from `crates/live/src/system.rs`: a frame is a 4-byte big-endian
length and a JSON text; requests are `{api, generation, request, method, body}`
and replies `{api, generation, request, result}` where `result` is `{"Ok": v}`
or `{"Err": text}`. Methods: `prepare`, `restore`, `activate`, `apply`,
`compute`, `dispose`. `prepare` and its reply are plain JSON.

A worker's `prepare` reply may add two capabilities. The host speaks them only
to a worker that advertised them; the Rust worker advertises neither.

### `"codec": "bj1"` — numbers Bend can read

After `prepare`, every frame in both directions is BJ1: JSON text whose number
tokens are written as

| value | token |
| --- | --- |
| integer `0 ≤ n < 2^32` | decimal digits |
| integer `-2^32 < n < 0` | `-` and decimal digits |
| integer `2^32 ≤ n < 2^64` | `w` and exactly 16 lowercase hex digits |
| float (a JSON number with `.`, `e` or `E`) | `f` and exactly 8 lowercase hex digits: the IEEE-754 binary32 bits |

Every float in Delve's state is an `f32` widened by serde, so the bits are exact
both ways. The encoder writes no whitespace; key order is not meaningful.
`crates/live/src/bj.rs` is the host's codec.

### `"resident": true` — the state lives in the worker

The worker keeps the state it last answered. `restore` still carries the full
checkpoint (handoff, recovery). After it, `apply` carries
`{"delta": ops, "event": e}` instead of `{"state": s, "event": e}`: `ops` turn
the worker's resident state into the state the host now holds (empty when the
host changed nothing between ticks). The reply is `{"delta": ops, "output": o}`:
`ops` turn the state as the request left it into the stepped state. The host
applies them to its copy, validates, and commits; on any error it falls back to
a full `restore`.

An op is one of

- `["s", path, value]` — set: replace the value at `path`, adding an object key
  or, at index `len`, appending to an array;
- `["d", path]` — delete an object key;
- `["t", path, n]` — truncate an array to `n` items;

and `path` is an array of object keys (strings) and array indices (integers),
root first. Ops apply in order.

## The program

```
delve/
  main.bend        the worker: frames, lifecycle, resident state, dispatch
  lib/j.bend       the state tree J, its accessors, BJ1 read/show, deltas
  lib/w64.bend     u64 as two U32 words (ticks, seeds, raid ids)
  lib/i32.bend     signed i32 as its two's-complement U32
  step.bend        Run::step, phase by phase, as rules.rs orders it
  rules/<m>.bend   one file (or folder) per Rust module of crates/delve/src/rules
  world/<m>.bend   crates/delve/src/world
  realm/<m>.bend   crates/delve/src/realm
  tests/           case runners: `bend tests/<m>.bend` checks recorded cases
```

State is the J tree of the Run exactly as `runtime_state::capture_for_rules`
serializes it: the same keys, the same enum spellings, the same omissions. A
rules module reads and writes the Run through `J.get`/`J.set` paths; it never
invents keys the Rust serde shape does not have, because the host restores the
reply into the Rust `Run` until the host itself moves.

Numbers: Rust `u32`/`usize` are `U32`; `i32` is `U32` two's complement
(`lib/i32.bend`); `u64` ticks and seeds are `lib/w64.bend` pairs (`J.w64`);
`f32` is `F32` — the same IEEE operations in the same order give the same bits,
and `F32.sin`/`sqrt`/... are the C library's, as Rust's are.

## Parity and deletion

`<artifacts>/delve-replay/` holds a golden corpus of worker exchanges
recorded from the Rust worker over real scenarios (the recorder:
`ANGEL_SYSTEM_RECORD=<dir>` on the host's system client). `scripts/delve/replay.mjs
<worker> <corpus>` drives any worker binary through it and reports the first
divergence by path. Per-module case files (Rust oracle in a test, Bend runner in
`delve/tests/`) gate a module before the whole step exists. When replay is green
for the whole Run, the default worker becomes the Bend binary and
`crates/delve/src/{rules,world,realm}` logic and the Rust worker are deleted.
