# Glass hardening — handoff (2026-10-16, overnight)

Branch `glass/foundation-hardening` in worktree `/work/tmp/angelx-glass-muse`,
based on `render/foundation` as of its Oct 15 evening tip (the branch was
rebased once, when the foundation moved 19 commits mid-evening; at last check
it is 0 behind). 56 commits, 23 files, +3009/-236. Nothing is merged or
pushed; bringing it together is a reviewed merge whenever you say so.

## What it is

Foundational strength for glass, per the 2026-10-13 scope: every input
boundary fails closed, the frame loop allocates nothing in steady state,
and the numbers in `RENDER_FOUNDATION.md` were re-measured rather than
trusted. No visual or game-design changes; the one new visible thing is a
test scene.

## Headline evidence (nested seat, RTX 3090)

- Keypress → compositor frame (keyframe kit, realm animating, same binary):
  at branch HEAD (load ~3, minutes apart): Glass p50 4.04 / p95 7.55 ms
  vs Kitty p50 8.16 / p95 10.62 ms. Under load 5.6–10 earlier rounds:
  Glass 7.4–8.0 / 8.2–10.7 vs Kitty 9.5–10.3 / 12.5–13.3. Glass ahead on
  both percentiles in every pairing.
- The blast (`ANGEL_GLASS_DEMO=blast`): a 3D-warped video stream, raycast
  primitives, sine line formations across z, all drifting over live text.
  `ANGEL_GLASS_BLAST_X=64` = 12,349 layers: build 0.90–0.98 ms mean,
  render p95 3.1–6.1 ms (spread is present). Generation 2.2–2.7 ms/frame CPU.
- Suites: angel-glass 79 tests, angel-stage 11, angel-web 7, delve
  scene-parity, wasm checks for angel-stage/angel-web, clippy gate clean,
  wire fuzz probe (`crates/web/probe/stage-fuzz.mjs`) 11/11 + 3500
  mutated/garbage frames without a throw.

## Real bugs found and fixed

- Config: `#界界` as a colour panicked the hex parser (byte-slicing);
  `font_size nan` and friends flowed into cell metrics and the clear colour.
- Graphics: a PNG header claiming 100000×100000 allocated its claimed frame
  (40 GiB) before any size check — in both PNG decoders; chunked Kitty
  uploads grew memory without limit; direct uploads reached `write_texture`
  unchecked.
- Font: underline placement u32-underflowed on thick-stroke fonts; rasters
  were indexed by claimed sizes without matching byte counts; cell metrics
  were unbounded (allocation attack via advance width).
- Transition: env-supplied film seconds could panic `Duration::from_secs_f32`
  (and the first fix's `clamp` passed NaN through — its own test caught it).
- Atlas: a glyph working set larger than the atlas reset and re-rastered
  everything every frame; now it saturates and blanks the overflow.
- Clipboard paste: unbounded read, no timeout, on the window thread.
- Shared-state locks: one panicked holder poisoned the session; all recover.
- Wire (browser): truncated/lying frames threw mid-upload; the encoder
  could desync the stream with a malformed picture. Both ends validate.

## Speed work (all measured, A/B where it mattered)

- Draw parts join by texture + clone-free sanitize: build at 12,349 layers
  1.79 → 0.90 ms.
- Frame buffers, build working buffers, picture-group map: recycled.
  Honest A/B: the pool costs ~0.15 ms at 12k layers, nothing at normal
  counts; it buys zero steady-state allocation.
- Layer list crosses threads by `Arc` (was a 1.4 MB clone per draw at 12k,
  outside the render timing entirely).
- Stages borrowed, not cloned, per frame; stage texture views cached.
- Printable ASCII skips the Unicode width table (proven equal).

The blast over the real cockpit (practice driver, realm animating)
rendered p50 0.83–1.08 ms: `docs/images/glass/blast-cockpit.png`.

Film the live blast at 30 frames a second (then `ffmpeg -framerate 30 -i frame-%03d.png blast.mp4`):

```bash
ANGEL_GLASS_DEMO=blast ANGEL_GLASS_BLAST_FILM=/tmp/blastfilm ANGEL_GLASS_BLAST_FILM_SECONDS=5 cargo run --release --example sampler
```

Verified 2026-10-16: 150 frames, 5 s, 1600x900 H.264; contact sheet `docs/images/glass/blast-film-sheet.png`, video at `/work/artifacts/glass-blast-film/blastfilm.mp4`.

Final keyframe spot-check (2026-10-16, same nested seat, minutes apart,
machine load rising 5.6 to 10.0): Glass KEY_TO_FRAME p50 8.03 ms / p95
10.66 ms vs Kitty p50 10.24 ms / p95 13.17 ms. Also landed this round:
config reads capped at 1 MiB, capture GPU poll bounded to 5 s, font loads
size-gated and validated before leaking, fc-match capped at 64 KiB.

Present-mode A/B (blast sampler, nested seat): mailbox (the default) and
immediate are indistinguishable (present 2.4-2.7 ms mean — the nested
compositor's floor, not a glass cost); fifo adds ~12 ms of acquire wait.
No change warranted; the present floor is compositor-side.

Soak fix (the round's real find): frame assembly returned one emptied
buffer per part to the reuse pool while a build only takes five back
out, so the pool retained the difference every frame — a 90-second X64
blast soak grew the sampler from 1.5 GB to 7.4 GB. The pool is capped
(`POOL_MAX=32`, regression-tested fail-first); the same soak now sits
flat at 184 MB, slide included. Frame loop also polls the device each
frame so finished submissions reap promptly.

Verified in the real cockpit (nested seat, blast over the practice
driver): RSS flat at 296,020 KiB across 60 s at ~76 fps. A 5-minute
sampler soak at X64 moved 183,960 -> 184,904 KiB total (page-commit
noise, no slope). Glyph memos are now capped too (65k chars / 16k
strings, clear-on-full).

Gates: the configured clippy invocation
(`-D warnings -A dead-code -A clippy::type-complexity`, from
`scripts/check/check-cockpit-quality.sh`) now passes on angel-glass,
angel-stage and angel-web — including the four lints the foundation
carried. rustfmt is not the house style (421 diffs at the foundation
base under the cockpit manifest), so no mass reformat was done.
Parser proof: 4000 mutated Kitty commands/symbols and 3000
random-bit-pattern layers all fail closed, in durable tests.

Integration dry run: merging `glass/foundation-hardening` into
`render/foundation` in a scratch worktree fast-forwards cleanly (0 behind)
and the merged tree passes its glass + stage suites.

Kitty-protocol comparison (the overlap that can be raced fairly): the
blast's four stream sizes pushed through Kitty's own protocol inside
Kitty (`glass-test-kit/kitty-blast.py`, z-stacked placements, each frame
query-synced, drawing verified by screenshot) ingest at p50 3.86-3.94 ms
per 4-image frame, ~240 MiB/s of base64, RSS flat. Kitty's ingestion is
excellent. The glass protocol path isolated
(`cargo run --release --example proto-bench`): parse 0.23 ms + store
0.32 ms CPU per same frame (raw; PNG 0.95 ms, decode-bound), with GPU
upload measured separately at ~0.07 ms/frame at these sizes — a ~0.6 ms
total against Kitty's 3.9 ms round trip, boundaries stated (Kitty's
number includes PTY transfer and its GPU scheduling; the glass number
is in-process). The glass
wins measured tonight are the native path (no base64/PTY tax: build
~0.12 ms, full frame p50 ~2.5 ms at the same upload sizes), compositor
features the protocol cannot express (lifted, rotated, particle and
glow layers at 12,349 scale), and key-to-frame latency on the real
cockpit workload. Most layer testing has no Kitty counterpart to race.

On the 2026-10-17 dev line glass no longer parses escape codes or keeps an
upload store (every picture is handed over as its own pixels), so
`proto-bench` now measures what replaced them: taking the four streams as
`Picture`s, the well-formed check and the premultiply the GPU side stores
them with. On the same box: 0.05 ms to take a frame's pixels and 0.018 ms
to check and premultiply them (raw); with PNG, the producer's decode is
0.87 ms. The 0.23/0.32 ms figures above are the old parser's.

## Integration packet

The pool leak fixed here exists in `render/foundation` today, so this
merge is also a live-session fix, not only hardening. To bring it in:

```bash
cd /work/angelX-render   # the render/foundation worktree
git merge glass/foundation-hardening   # fast-forward at last check
cd crates && cargo test -p angel-glass -p angel-stage -p angel-web
cargo clippy --locked --offline -p angel-glass -p angel-stage -p angel-web --all-targets -- -D warnings -A dead-code -A clippy::type-complexity
node crates/web/probe/stage-fuzz.mjs
cd ../cockpit && cargo build --release --features glass
ANGEL_GLASS=1 ANGEL_GLASS_DEMO=blast ./target/release/angel
```

If the foundation has moved by then, merge it into
`glass/foundation-hardening` first (in `/work/tmp/angelx-glass-muse`),
re-run the suites there, then merge across. Rollback is the branch
boundary itself: nothing here has touched `render/foundation`.

## Try it

```bash
cd /work/tmp/angelx-glass-muse/crates
ANGEL_GLASS_DEMO=blast ANGEL_GLASS_STATS=1 cargo run --release --example sampler
ANGEL_GLASS_DEMO=blast ANGEL_GLASS_BLAST_X=64 ANGEL_GLASS_STATS=1 cargo run --release --example sampler
cd ../cockpit && cargo build --release --features glass
ANGEL_GLASS=1 ANGEL_GLASS_DEMO=blast ./target/release/angel
```

## Suggested next steps

- Reviewed merge into `render/foundation` (56 small commits, each with its
  evidence in the message) — see the integration packet above.
- GPU particles remain the first compute-kernel candidate; nothing measured
  demands one yet (present is the floor).
- Damage-aware build was tried and measured out: a row-instance cache
  (content hash + atlas-generation stamp, adaptive stand-down) made the
  sampler build slower (0.15–0.24 vs 0.085–0.096 ms) — after FxHash and
  the ASCII width fast path, hashing and copying a row costs more than
  rebuilding it. The experiment is saved at `/tmp/row-cache-experiment.patch`
  if a future profile disagrees.
