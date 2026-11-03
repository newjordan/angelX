# Delve replay: the parity gate

A golden corpus of exchanges with the Delve rules worker, and a driver that
puts any other worker (the Bend one) through the same exchanges and names the
first place its state departs. Design: `docs/architecture/delve-in-bend.md`
("Parity and deletion").

## Record

Test-only, `#[ignore]`d, in `crates/delve-system/tests/record_corpus.rs`. It
plays scripted and bot-driven games and, every tick, does what the host does
(`capture_for_rules`, the event the realm host sends) and what the worker does
(`angel_delve_system::apply`: restore, settings, phrasebook, `Run::step`,
`capture`), then continues from the worker's reply.

```sh
DELVE_REPLAY_OUT=~/delve-replay CARGO_TARGET_DIR=crates/target/legs \
  cargo test --manifest-path crates/Cargo.toml -p angel-delve-system --profile fast \
  --test record_corpus -- --ignored --nocapture record_corpus
# a few scenarios only (writes MANIFEST.partial.json):
DELVE_REPLAY_ONLY=duo-f2-crawl,nooks ...
```

Seeds and scripts are fixed, so a re-record reproduces the corpus byte for byte
(`gzip -n`). `MANIFEST.json` lists every scenario: its ticks, size, seats, floors,
rooms, games and which parts of the `Run` moved.

### Format

One file per scenario, `<name>.jsonl.gz`, one JSON value a line:

```
{"kind":"restore","state":<checkpoint>}
{"kind":"apply","before":<checkpoint|null>,"event":<event>,"after":<checkpoint>}
```

- A checkpoint is `{"schema":6,"run":{...}}`: exactly
  `runtime_state::capture_for_rules(&run).into_value()` as the host sends it,
  `capture(&run).into_value()` as the worker replies.
- `before` is `null` when it is the same text as the previous line's `after` (the
  `restore` state, for the first apply). The host drains cues, sounds and
  notices between ticks and moves seats, presence and the tune, so a `before`
  that differs shows what the host changed. A resident worker is sent the delta
  between its state and `before`.
- `event` is `{"inputs":{seat:Input},"settings":{...},"phrasebook":[text,errors],"step":bool}`.
  The phrasebook (14 KB) is written only on the first apply and when it changes;
  a reader carries it forward.
- The `after` of a tick is the worker's whole reply state.

## Replay

```sh
CARGO_TARGET_DIR=crates/target/legs \
  cargo build --manifest-path crates/Cargo.toml -p angel-delve-system --profile fast   # plain build, not `test`
node scripts/delve/replay.mjs crates/target/legs/fast/angel-delve-system ~/delve-replay
node scripts/delve/replay.mjs <worker> <dir-or-file> [--scenario a,b] [--from N] [--max-diffs K]
```

It spawns the worker per scenario, sends `prepare`, reads the capabilities,
`restore`s line 1, `activate`s, then every `apply`, comparing the worker's
state to `after`. A scenario stops at its first divergence and reports the tick,
the JSON path, expected and got; the next scenario still runs. The exit status is
nonzero if any scenario diverged or the worker failed. `--from N` starts at tick
N (restoring the state the host held there); `--max-diffs K` lists up to K
differing paths at the divergent tick (default 3).

The comparison is semantic: key order is ignored; floats compare as f32 bits
(`-0` is not `0`; a float an f32 cannot hold compares as f64); integers
exactly (BigInt beyond 2^53); a missing key and a `null` differ. The reader
keeps integers and floats apart by their tokens (a `.`, `e` or `E` makes a float).

A worker whose `prepare` reply says `"codec":"bj1"` is spoken to in BJ1 (floats
`f`+8 hex of the f32 bits, integers of 2^32 and up `w`+16 hex), and one that says
`"resident":true` is sent `{"delta":ops,"event":e}` and answers
`{"delta":ops,"output":o}` (ops `["s",path,v]`, `["d",path]`, `["t",path,n]`);
the driver applies the reply's ops to `before` to get the new state.

### Files

- `replay.mjs` the driver.
- `replay-lib.mjs` the reader, the BJ1 codec, deltas, comparison and the framed worker.
- `replay-shim.mjs` a worker stand-in that speaks BJ1 and resident in front of
  the Rust worker; `SHIM_WORKER=<rust worker> [SHIM_BJ1=1] [SHIM_RESIDENT=1]
  [SHIM_BREAK=N] node scripts/delve/replay.mjs scripts/delve/replay-shim.mjs <corpus>`
  proves the codec and delta paths of the driver, and (with `SHIM_BREAK`, one
  float off by one f32 step) that the gate fails when it should.
- `tests/scripts/delve-replay.test.mjs`: `node --test tests/scripts/delve-replay.test.mjs`.

The worker under replay must be the production build: `cargo test` builds the
same binary with the `testing` feature of `angel-delve` (the recorder needs the
fixtures), so rebuild with `cargo build` before a replay.

### Offline card-art review

`python3 scripts/delve/card_gallery.py --output /tmp/delve-cards.svg`

Open the SVG at **100% zoom**. Each labeled card includes its unscaled pixels
and 8x nearest-neighbour samples on dark and light backdrops, using the current
Rust game palette. Optional positional `.card` paths select a smaller review
batch. No API credentials, provider calls, or imaging dependencies are needed.
The loader rejects unknown inks and art exceeding the game's 16-pixel bound;
smaller legacy icons retain their original dimensions (they are not stretched).
This sheet is an inspection aid, not visual approval: review silhouettes,
contrast, material cues, and distinctness, then test readability in combat.

Both native-size samples now have their own dark/light backdrops, matching the
8x previews. Renderer regression checks (including XML parsing of every current
card) run with `python3 scripts/delve/test_card_gallery.py`.

Add `--audit /tmp/delve-art-audit.json` to write deterministic review hints:
opaque pixel count, rendered colour count, eight-connected component sizes,
fully transparent icons, isolated pixels, and exact rendered duplicates (including
palette aliases). These are triage hints, not quality scores: sparks can be
intentionally disconnected and different silhouettes can still look too similar.
The JSON explicitly records `approval: false`. Tests cover alias-colour duplicates,
transparency, and isolated components as well as the renderer.

The JSON art audit also emits `similar_silhouettes`: fixed-canvas opaque-mask
pairs with at least 90% intersection-over-union and at least 16 opaque pixels
in each sprite. Exact RGB duplicates remain in their own field. This catches
recolours that exact-duplicate checks miss, without translating or rescaling
art. These are review candidates, not automatic rejections or visual approval;
related bottle shapes can legitimately overlap.

`translated_silhouettes` separately flags bounding-box-origin-normalized opaque
masks with >=90% overlap and >=16 pixels that the fixed-canvas check misses.
This catches shifted/recoloured shapes without rotating or resizing them. Canvas
placement can be intentional; this is a review hint, not a rejection or approval.
