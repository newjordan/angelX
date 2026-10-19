# Render scorecard: glass vs Kitty graphics

The claim under test: **glass beats Kitty graphics on every metric both
systems can run, and does things Kitty graphics cannot express at all.**
This file defines the races, the boundaries, and holds the results.
Numbers without a harness in this repo do not count.

Two boards, kept separate on purpose:

- **Board A — fair races.** The same visible workload through each
  system's real production path. For Kitty that is the graphics
  protocol inside Kitty (`ESC _G …`), including its fastest local
  transport (shared memory, `t=s`). For glass it is `Host::upload_picture`
  plus layers — glass has no escape parser since `1cf1af0f` and racing a
  parser we deleted would prove nothing about the window anyone runs.
  Transports differ; the pixels, sizes, frame counts and sync points do
  not. Every boundary that differs is stated per metric.
- **Board B — capability matrix.** Things one system can express and
  the other cannot. No times; a capability Kitty lacks is recorded as
  "no counterpart", never as a race win.

## Environment discipline

- Same nested seat for both sides (`WAYLAND_DISPLAY=wayland-2`, the
  glass test kit's private Hyprland), same window size class, runs
  minutes apart, machine load recorded with every result set.
- Kitty: system `kitty` binary, `--config NONE` except where keyframe
  needs the operator look (Board A1 uses the kit launcher both sides).
- Glass: this branch's `stream-bench` example or the real cockpit.
- Pre-generated pixel rings on both sides: the timed sections measure
  the renderer, not the producer's pixel math. Frames differ every
  frame, so no side may skip an "unchanged" upload.
- Warm-up excluded: the first stats window / first 30 acked frames of
  every run are dropped before percentiles.

## Board A metrics

| # | Metric | Workload (identical pixels) | Kitty side | Glass side | Sync point |
|---|---|---|---|---|---|
| A1 | Key → compositor frame | Type into the angel cockpit composer | cockpit TUI in Kitty (kit launcher `KITTY=1`) | cockpit in glass | external `keyframe`: screencopy damage at the cursor cell |
| A2 | Graphics stream ingest | Blast's 4 streams (320×200, 192×160, 160×160, 480×140 RGBA), fresh frame ×300 | `kitty_gfx_bench.py blast4` — f=32 base64, f=100 PNG, `t=s` shm | `stream-bench` blast4, immediate present | Kitty: `a=q` ack. Glass: frame presented (stats parts: upload+build+submit) |
| A3 | Big picture stream | 1920×1080 and 3840×2160 RGBA, fresh frame ×120 | `kitty_gfx_bench.py big` (f=32, shm) | `stream-bench` big | same as A2 |
| A4 | Many placements | One 64×64 picture, N∈{100, 1000, 5000} placements moved every frame | `a=p` re-place ×N + ack | N picture layers re-placed per frame | ack / presented |
| A5 | Full-screen text churn | 200×50 cells, every cell changes every frame | VT stream + DA ack | ratatui buffer through `GlassBackend` | ack / presented; reported as cells/s. Boundary: Kitty parses VT, glass receives cells — the user-visible quantity is changed cells presented per second |
| A6 | Sustained video at vsync | A2 workload, 60 s, mailbox/fifo-class presentation | dropped-frame count is not externally visible in Kitty; reported as ack p95 vs the 16.7 ms budget | achieved fps + render p95 from `GLASS_STATS` | pass = no missed vsync in the window |
| A7 | Resources | Idle 10 s, then A2 | RSS + CPU% from `/proc` | RSS + CPU% from `/proc` | sampled by the runner |

Kitty server-side animation (`a=f`) is not a separate race: filling an
animation is repeated uploads (A2 covers it) and playing one back is the
terminal re-compositing a stored frame, which glass answers by holding
the picture as a texture and re-placing it (A4 at N=1). It appears on
Board B as a protocol convenience, not a performance metric.

## Board B capability matrix

Filled from the code, each row with the file that proves it. "Partial"
means Kitty expresses a restricted form (cell-anchored, axis-aligned).

| Capability | Glass | Kitty |
|---|---|---|
| Sub-cell / pixel placement, unclipped past the pane | yes — `Placement::Pixels`, `crates/stage/src/lib.rs`; screenshot `glass-blast.png` (layers cross the text and each other freely) | partial (cell + pixel offset, clipped to pane) |
| Rotation of a picture | yes — `Layer.angle`; cube + tilted panel in `glass-blast.png` | no |
| Perspective / 3D-warped picture stream | yes — blast's warped video plane (`crates/glass/src/blast.rs`) | no |
| Lift toward camera with ground shadow | yes — `Layer.height`; the panel's shadow in `glass-blast.png` | no |
| Window-animated layers (breathe, flicker, additive light) | yes — `Layer.breathe/flicker/additive` | no |
| Z above/below text | yes — `Layer.z` | yes |
| Glow / particle layers animated by the window | yes — `LayerKind::Glow/Particles`, `motes()` | no |
| Stage scenes (raycast primitives, slides, tilt) | yes — `crates/glass/src/stage.rs`, `Stage.tilt/enter` | no |
| Alpha-channel streamed pictures composited over text | yes — premultiplied textures, `glass-blast.png` | yes (RGBA images) |
| Same layer description in a browser seat | yes — `crates/stage/src/wire.rs` + `crates/web/shell/stage.js` (browser canvas still `alpha:false` — expansion target) | no |
| Window transparency (desktop shows through) | yes — `with_transparent(true)`, premultiplied surface | yes (background opacity) |
| Picture sub-rectangle (sprite sheet part) | yes — `LayerKind::PicturePart` | yes (crop) |
| >10k independently placed animated layers | yes — `MAX_QUADS` 1M; blast at 12,349 layers measured in the hardening round; 5000 raced in A4 | raced in A4 (28 ms ack at 5000) |
| Server-side animation playback (`a=f`) | producer streams instead | yes |

## Pre-registered v2 (written before any v2 measurement, 2026-10-17)

The first round's weaknesses, named by applying a skeptic's prompt to
it: Kitty raced at `--config NONE` defaults (`repaint_delay 10`,
`input_delay 3`, `sync_to_monitor yes` — from
`/usr/lib/kitty/kitty/options/definition.py`), glass timings were
self-reported aggregates, one session's load, percentiles without
intervals, synthetic-only workloads. V2 fixes the method, then re-runs.

**Steelman.** Kitty runs `scripts/bench/render/kitty-steel.conf`:
`repaint_delay 0`, `input_delay 0`, `sync_to_monitor no`, 9 pt. Glass
runs its production defaults (mailbox, no transition under test).

**Environment.** CPU governor is `performance` (checked). GPU clocks
cannot be locked without root on this box (recorded limitation);
mitigation is interleaved randomized A/B order, ≥5 paired repetitions
per metric, and load recorded per repetition. Raw per-frame series
from both sides are committed under `docs/telemetry/render-v2/` as
JSONL, so the verdicts can be re-analyzed without us.

**Statistics.** Per metric: the median of per-repetition medians per
side, plus a 95% bootstrap interval (10,000 resamples over pooled raw
frames) on the difference of medians. Pass = the interval excludes 0
in glass's favour (for fps-hold/resources: the stated margins).

**Metrics (fixed now):**

| # | Metric | Pass bar |
|---|---|---|
| M1 | Key→compositor frame (external keyframe, real cockpit) | CI excludes 0, glass faster |
| M2 | blast4 ingest/frame | CI excludes 0 |
| M3 | 1080p and 4K stream/frame | CI excludes 0 |
| M4 | Placements 100/1000/5000 per frame | CI excludes 0 at each N |
| M5 | Text churn cells/s | CI excludes 0 |
| M6 | Adversarial: 256 fresh 16×16 pictures per frame (texture churn) | CI excludes 0 |
| M7 | Real producer: same generated 1080p clip, 300 frames, mpv `--vo=kitty` in Kitty vs `/show` video in glass | glass wall time ≤ Kitty's, mid-play frame screenshot-verified |
| M8 | Golden frame: deterministic alpha scene both renderers | mean abs pixel diff ≤ 2/255 in the stage box |
| M9 | Idle RSS/CPU (matched 12 s windows) | glass within +5% |
| M10 | blast4 at vsync, 14 s | render p99 < 16.7 ms |

Every v2 loss gets the same treatment as Board A losses below: caused
in the code and fixed, or proven structural with the trace.

## Results

Recorded by `scripts/bench/render/run_scorecard.sh`; raw JSON beside
each run under `/work/tmp/render-scorecard/<run>/`.

Run `board-a-4`, 2026-10-17: RTX 3090, kitty 0.48.2, nested seat
(`wayland-2`, 1600×900@60), both windows floated 1576×880 at 9 pt
(glass grid 219×56, Kitty 225×58), machine load 4–9 (other sessions
live; recorded per run in `environment.txt`). Compositing of the A2
workload was verified by screenshot on both sides
(`docs/images/render-scorecard/`), not only by ack.

Disclosure: the earlier `board-a-2`/`board-a-3` runner results were
measured on the host desktop (`wayland-1`), not the nested seat —
the runner inherited the calling shell's `WAYLAND_DISPLAY`. The
runner now forces `wayland-2` and refuses `wayland-1`; `board-a-4`
below supersedes those runs. Verdicts did not change; Kitty's
numbers moved by ~2%, glass's present-inclusive figures by the
nested compositor's present floor (~0.4–1.8 ms).

Glass per-frame figures are producer handover (`produce_p50`) +
render-thread upload (`upload_ms_per_frame`) + presented-frame
render p50/p95 from `GLASS_STATS`, immediate present unless noted —
i.e. glass numbers end at a *presented* frame, Kitty numbers end at a
protocol ack (its compositing still to come). Glass still leads.

| # | Glass | Kitty (best = shm unless noted) | Verdict |
|---|---|---|---|
| A1 key→frame | p50 6.41 / p95 9.62 ms (real cockpit, external keyframe) | p50 8.59 / p95 10.73 ms (same cockpit in Kitty) | glass |
| A2 blast4 ingest | produce 0.25 + upload 0.09 ms, render p50 0.71 / p95 1.58 ms, 166 fps presented | ack p50 3.42 / p95 3.58 ms (b64 4.25, PNG 5.22) | glass ≈4× |
| A3 1080p stream | produce 0.55 + upload 0.82 ms, render p50 1.00 ms, 120 fps | ack p50 6.28 ms (b64 53.8, `board-a-2`) | glass |
| A3 4K stream | produce 2.08 + upload 2.96 ms, render p50 2.03 ms, 120 fps | ack p50 20.9 / p95 23.7 ms | glass ≈4× |
| A4 placements N=100 / 1000 / 5000 | render p50 0.58 / 0.73 / 0.86 ms presented | ack p50 3.15 / 4.43 / 24.7 ms | glass 5× / 6× / 29× |
| A5 text churn | 22.7M cells/s (1848 fps free-running, render p50 0.41 ms) | 4.19M cells/s (ack p50 3.11 ms — Kitty's ack floor on this seat) | glass ≈5× |
| A6 blast4 at vsync | mailbox: 60 fps held, render p95 1.51 ms vs 16.7 ms budget | ack p95 3.58 ms vs budget (presentation not externally visible) | both hold; glass proven presented |
| A7 idle (12 s matched windows) | RSS 125.6 MiB, CPU 0.2% | RSS 129.8 MiB, CPU 0.2% | glass under on RSS, tie on CPU — glass side is the bench window (no cockpit), Kitty a full terminal; both dominated by GPU/driver mappings |

## Expansion log

Phase 2 of the 2026-10-16 render handoff, each landing benched.

- **Browser seat transparency** (done): the shell's canvas is now
  `alpha:true, premultipliedAlpha:true`; stage targets clear
  transparent and the page shows through, matching the glass window.
  The blit no longer premultiplies the stage's own (already
  premultiplied) target — the twin of the glass shader-kind-3 bug.
  Finished-frame path unchanged (its shader forces opaque alpha).
  Proof: `crates/web/probe/stage-alpha.mjs` (recording-GL contract
  check) and a real headless-Chromium composite over a magenta page
  (`docs/images/render-scorecard/browser-alpha.png`): untouched
  (255,0,255), half-blue solid (127,0,255), half-red texel
  (255,0,127) — the premultiplied-correct values. CPU cost of a
  2-quad `drawStage` under SwiftShader: 0.147 → 0.076 ms/frame
  (noise-scale; no regression). `stage-fuzz.mjs` still passes.

- **Video through the picture cache** (done): the cache learned
  stream claims — a held picture per stream, uploaded every frame
  (the renderer overwrites same-size textures in place, so a stream
  costs one texture). The viewer's game, video and map frames now go
  through `present::upload_stream` instead of handing pictures to
  the window directly, and `Showing::Video` carries a frame and
  presents. The decode sequence is carried by the frames, not the
  key: keying by sequence would allocate a new picture id (and
  texture) per frame. Verified live on the seat (realm streams as
  before, `docs/images/render-scorecard/e2e-realm.png`) and by the
  `ui::present` stream-claim test; keyframe with the final binary
  (`board-a-5`): glass 6.39/10.71 ms vs Kitty 10.29/12.83 ms.
- **Documents as pages** (done): decided for a raster service in the
  glass crate (`angel_glass::page::Pager` — the window's own font
  stack, glyph memo, integer opaque-paper composite) over a second
  swash dependency in the cockpit. The scryglass document body now
  presents as one rasterized page: the paragraph is laid into a
  scratch buffer first, so the page's lines are exactly the wrapped,
  scrolled lines the cells would show. Bench (`page-bench`):
  48×96 page in 1.37 ms warm, 2.11 ms cold. Live proof
  (`e2e-document*.png`): `/show notes.md` renders as a page; a
  scroll uploads exactly one fresh page (742 KiB in the stats
  window, 0 at rest). Terminals unchanged (cells).
- **Primitives as `Showing::Layers`** (ready, nothing to land here):
  at this HEAD `LayerKind` is Solid/Picture/PicturePart/Glow/
  Particles and stage scenes already present through
  `Showing::Stage`/`Showing::Layers` — the doors exist. The SDF
  `LayerKind::Primitive` kind is the parallel session's uncommitted
  work in the main tree; when it lands it needs no presentation
  changes. `Showing::Mesh` stays honestly refused.

## Losses and what was done about them

Every Board A loss gets a paragraph here: the number, the cause found
in the code, and either the fix (with its re-run) or the proof that the
loss is structural (a transport or protocol fact, not a code fact).

`board-a-4` has no Board A losses. The closest metrics, and what
bounds them:

- **A7 idle** is a near-tie by construction: both processes are
  dominated by GPU/driver mappings (~126–130 MiB) at 0.2% CPU.
  Glass measured under Kitty on RSS in `board-a-4`; the earlier
  1.5%-vs-0.1% CPU reading was a sampler artefact (unequal windows),
  fixed in the runner.
- **A3 big-picture upload** was glass's softest margin at first
  measurement (1080p upload 2.27 ms): the premultiply walked every
  pixel even when opaque. It now skips opaque 16-pixel chunks with
  one u64 alpha-lane test (bit-identical; 1080p upload 0.82 ms in
  `board-a-4`, micro-bench premultiply 0.42→0.12 ms). The remaining
  upload cost is memory bandwidth (premultiply scan + staging copy).
- Kitty's ~3.1 ms ack floor on small synced workloads (text,
  100 placements) is structural to its protocol round trip on this
  seat; it is reported, not chased.

## V2 results (2026-10-17, steelmanned Kitty)

Runs `v2-1` and `v2-2`; raw series, kitty bench JSON, keyframe
samples and photon stamps are committed under
`docs/telemetry/render-v2/`. Kitty ran
`scripts/bench/render/kitty-steel.conf` throughout; glass ran
production mailbox unless a row says otherwise. Verdicts below are
against the pass bars fixed in the pre-registration above — v2 has
real losses, and they are recorded as losses.

| # | Glass | Kitty (steel) | Verdict |
|---|---|---|---|
| M1 key→frame | median 6.7 ms, p99 14.2 (n=126) | median 4.7 ms, p99 14.2 (n=126); diff +2.0 ms, CI [1.3, 2.75] | **kitty — FAIL** |
| M2 blast4 | presented-frame median 0.968 ms (n=9019) | ack median 0.329 ms (n=1500); diff CI [0.628, 0.651] | **kitty on the pre-registered series — FAIL; endpoints diverge, see photon below** |
| M3 1080p | 1.526 ms (n=7218) | 3.027 ms (n=600); CI [-1.556, -1.455] | glass |
| M3 4K | 1.857 ms (n=1741) | 20.658 ms (n=180); CI [-19.258, -18.285] | glass ≈11× |
| M4 placements | 1000: 0.873 ms (n=7202) | 1000: 1.263 ms (n=600), CI [-0.404, -0.378]; scaling alone: 100→0.151, 5000→21.58 ms | glass at the paired 1000; 100/5000 unpaired scaling only |
| M5 text | 0.425 ms (n=33922) | 1.134 ms (n=900); CI [-0.711, -0.707] | glass |
| M6 churn-256 | 1.369 ms (n=4747) | 10.005 ms (n=600); CI [-8.724, -8.516] | glass ≈7× |
| M7 real producer | `/show` plays the clip (screenshot-verified) but presents at the cockpit's ~13 fps app cadence (render p50 1.4 ms) | mpv `--vo=kitty`, 300 frames untimed in 2.80 s ≈ 107 fps | **kitty — FAIL (cockpit pacing, not the renderer)** |
| M8 golden frame | checker exact (220); alpha gradient blends in sRGB space (the browser convention) | checker exact (220); gradient front-loaded (linear-light blending); mid-alpha differs by up to ~58/255 | **divergent — FAIL vs ≤2/255 bar** |
| M9 idle | RSS 122.5 MiB, CPU 0.2% | RSS 123.8 MiB, CPU 0.0% | glass within bar |
| M10 vsync | mailbox frame p99 4.84 ms, max 6.86 (n=824) | — | glass, under 16.7 ms |

**External photon endpoint** (sequence-stamped solid frames caught
by screencopy damage; `photon-latency.txt` in telemetry, recomputed
from the committed stamp logs). Submit→photon at a 120 fps pace:
glass mailbox p50 3.1 / p95 13.3 / p99 15.3 ms; glass immediate
3.6 / 13.5 / 16.1; glass mailbox at 60 fps pace 7.2 / 9.4 / 13.8;
glass fifo 39.9 / 41.6 / 43.7 (fifo is not used in production);
kitty 3.4 / 4.7 / 12.1. On the same external endpoint, medians tie;
kitty's tail is tighter. The 60 Hz seat stride dominates both
tails, but does not fully explain glass's p95.

### V2 losses, caused or proven structural

- **M1 keyframe (kitty by 2.0 ms median).** Glass's in-process
  key→present probe measured p50 0.637 ms, so the render path is
  not the gap; the delta sits between present and the compositor
  frame the keyframe tool observes. Under Board A defaults glass
  won this metric; zeroing kitty's delays is worth ~4 ms to kitty
  and glass does not currently match it in production mailbox.
  Follow-up probe (single rep, n=42, telemetry
  `keyframe-glass-immediate.txt`): forcing
  `ANGEL_GLASS_PRESENT=immediate` in the real cockpit gives p50
  5.96 / p95 9.63 ms — ~0.7 ms better than mailbox, still behind
  kitty steel's 4.7. Mailbox queueing is therefore a minority
  share of the gap; the rest is in the input→present→compositor
  scheduling path and remains open work.
- **M2 blast4 series (kitty).** The pre-registered comparison
  pits glass's presented-frame cost against kitty's protocol
  ack — different endpoints, recorded as such in Board A. The
  photon rows above are the same-endpoint version: median tie,
  kitty tail wins. No claim of a glass win on M2.
- **M7 video (kitty).** The glass renderer streams 1080p at
  120 fps in `stream-bench`, but the cockpit presents scryglass
  video on its app redraw cadence (~13 fps observed), so a user
  watching a clip in angel gets ~13 fps while mpv-in-kitty is
  decode-bound at ~107 fps untimed. This is a cockpit
  integration cap found by v2, not a renderer limit; fixing the
  tick is follow-up work outside this branch's present surface.
- **M8 golden frame (divergent).** Solid colours are bit-equal.
  Semi-transparent gradients are not: glass composites in sRGB
  space (matching browsers/CSS), kitty in linear light. Neither
  is a bug; the ≤2/255 bar fails and the difference is now
  documented with the sampled profiles in telemetry
  (`m8-golden2.json`, shots in `docs/images/render-scorecard/`).

**Regression gate.** `scripts/bench/render/baseline-v2.json` pins
the glass medians above; `scripts/bench/render/check_regression.py
RUN_DIR` fails (exit 1) when a fresh run's median regresses beyond
+25% + 0.25 ms on any gated metric, and skips metrics a partial
run did not measure. It passes on both committed v2 runs and is
covered by `test_check_regression.py`.

**Pre-merge check** (no CI in this repo; run by hand before landing
render changes):

```
python3 scripts/bench/render/test_kitty_gfx_bench.py
python3 scripts/bench/render/test_analyze.py
python3 scripts/bench/render/test_check_regression.py
python3 scripts/bench/render/test_golden_check.py
python3 scripts/bench/render/check_regression.py docs/telemetry/render-v2/v2-1
python3 scripts/bench/render/check_regression.py docs/telemetry/render-v2/v2-2
cargo test --offline -p angel-glass          (from crates/)
cargo test --offline --features glass --bin angel ui::present scryglass   (from cockpit/)
```

Plus, on the nested seat, a golden capture via stream-bench
`STREAM_BENCH_MODE=golden STREAM_BENCH_CAPTURE=...` checked with
`golden_check.py`, whenever rendering or blending code changed.

## Weakness-fix pass (2026-10-17, after v2)

A planned pass over the v2 losses and surrounding weaknesses, one
commit per step on this branch:

1. **M7 fixed.** Two stacked caps: `stage_view` fed the decoder
   `fps_cap=12` in Full motion, and the event loop had no lane for
   a playing clip (redraws rode the 83 ms scenery tick). Full
   motion now caps at 60 (`video_fps_cap()`, Reduced 12, Off
   poster) and the loop deadline-paces while
   `Scryglass::video_playing()`. Same seat, same clip: presents
   12.5 → ~35/s (source is 30), uploads 9339 → 27128 KiB/2 s
   (`docs/images/render-scorecard/m7-fix.png`).
2. **M8 pinned as contract.** Blending stays sRGB (operator
   decision). stream-bench `golden` mode + `golden_check.py`
   assert the exact checker and the sRGB ramp from a real
   capture; a linear-light composite fails by design.
3. **M1 traced, verdict changed.** `GLASS_KEYHOP` (key → render
   start / present end) plus cockpit frame timing convict the
   cockpit's ambient side-column/stage paint, not glass present:
   draw phase 3.52 ms in glass vs 1.23 for the same angel in
   kitty; acquire 0.02 ms; swapchain depth irrelevant. With
   ambient sim off (`ANGEL_COMP_MODE=1` or backdrop off) glass
   measures 4.17 ms external vs kitty steel 5.69 same-session —
   the present path itself wins. Traces in
   `docs/telemetry/render-v2/m1-probe/`. The remaining fix
   (scenery-rate ambient repaint instead of per-key) is cockpit
   cadence work, deliberately not rushed here.
4. **Refresh-aware cadence.** Animation frame/ride windows were
   hard-coded 60 Hz; they now derive from the current monitor
   (60 Hz values identical; >60 Hz unit-tested, seat-unverified).
   Present-mode fallback never lands on Fifo silently.
5. **GPU premultiply prototype** behind
   `ANGEL_GLASS_GPU_PREMULTIPLY=1` (default off): golden-equal,
   upload is staging-bound so upload_ms is unchanged, but producer
   cost at 1080p drops ~40% reproducibly.
6. **Evidence hardening.** Full cockpit suite result recorded in
   the handoff; keyframe runner uses fresh run names and the
   cursor seek rejects the mouse pointer by fill ratio.
