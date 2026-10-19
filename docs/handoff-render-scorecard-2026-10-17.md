# Handoff · 2026-10-17 · the render scorecard and the expansion landings

Branch `render/kitty-scorecard` in worktree `/work/tmp/angelx-render-proof`,
based on `dev` at `fbc41b81`. Nothing merged or pushed; integration is the
operator's reviewed step, as with the glass hardening branch. Main tree's
parallel session was not touched (fresh worktree off committed `dev`).

## Commits

- `f710afc2` bench: the render scorecard harness
- `c88cd25c` bench: harness fixes from the first seat runs
- `833f2aa8` glass: opaque-chunk premultiply skip (bit-identical)
- `3864ec0e` bench: board-a-2 results (superseded — see below)
- `c98c8617` bench: board-a-4, the whole Board A on the nested seat
- `d2e573bc` web: browser seat transparency (+ blit double-premultiply fix)
- `b19dc300` cockpit+glass: video streams + document pages through present
- `290b3f82` chore: fmt/clippy on the new code only
- (plus this handoff and the scorecard's expansion log)

## The claim, as decided with the operator

Two boards (`docs/RENDER_SCORECARD.md`): Board A races the same visible
workload through each system's real production path (Kitty's protocol
inside Kitty — b64, PNG and `t=s` shm, query-acked — vs glass's
`upload_picture` + layers); Board B is a capability matrix, no times.
Scorecard first, then expand; each expansion benched as it lands.

## Board A verdict (`board-a-4`, nested seat, RTX 3090, kitty 0.48.2)

Glass leads every raced metric; glass numbers end at a presented frame,
Kitty's at a protocol ack. Key→frame 6.41/9.62 vs 8.59/10.73 ms (real
cockpit, external keyframe; final-binary recheck `board-a-5`:
6.39/10.71 vs 10.29/12.83). Blast4 ≈4×, 1080p/4K streams ahead
(4K ≈4×), placements 5×/6×/29× at 100/1000/5000, text ≈5×
(22.7M vs 4.19M cells/s), vsync blast held (render p95 1.51 ms),
idle RSS under Kitty (125.6 vs 129.8 MiB; CPU tied 0.2%).
Compositing screenshot-verified both sides
(`docs/images/render-scorecard/`).

**Disclosure (must read):** the `board-a-2`/`board-a-3` runs were
measured on the host desktop, not the nested seat — the runner
inherited the calling shell's `WAYLAND_DISPLAY=wayland-1`. Two
keyframe attempts in that window sent a few stray `r` keystrokes to
whatever held focus on the real desktop (~00:41–00:47, Oct 17). The
runner now forces `wayland-2` and refuses `wayland-1` without an
explicit override; `board-a-4` supersedes those runs. Bench windows
during board-a-2/3 also appeared on the real desktop (class
`angelcap-glass`/`scorecard-*` only; no operator windows were closed —
`close_class` only ever targets the bench class — and none remain).

## Expansions (handoff §"Next, in order")

1. **Documents as pages — done.** `angel_glass::page::Pager` (the
   window's font stack; decided against a second swash in the cockpit).
   Scryglass document body presents as a rasterized page fed the exact
   wrapped+scrolled lines from a scratch buffer. 48×96 page: 1.37 ms
   warm / 2.11 ms cold. Live proof: `e2e-document*.png`; one 742 KiB
   upload per scroll, none at rest.
2. **Video through the picture cache — done.** Stream claims in the
   present cache (held picture, upload every frame);
   `present::upload_stream`; the viewer's game/video/map frames route
   through it; `Showing::Video` carries a frame and presents.
3. **Primitives as Layers — ready, blocked upstream.** No `Primitive`
   kind exists at this HEAD; it is the parallel session's uncommitted
   main-tree work. `Showing::Layers`/`Stage` doors already exist, so it
   lands with no present changes. `Mesh` stays refused.
4. **Browser seat transparency — done.** `alpha:true` +
   `premultipliedAlpha:true`; stage targets clear transparent; the
   stage blit no longer premultiplies its own target twice (the twin
   of the glass kind-3 bug). `crates/web/probe/stage-alpha.mjs`
   (fail-first) + pixel-exact Chromium composite
   (`browser-alpha.png`).

## Gates

- `cargo test -p angel-glass -p angel-stage -p angel-web -p angel-table`:
  76+11+7+9 green. Stream-bench cross-language tests green.
- Cockpit: `ui::present` (5), `round_table` (2), `scryglass` (74),
  `still` (62) green under `--features glass`; non-glass `cargo check`
  green. The full 5951-test cockpit suite was not run (the conservative
  boundary — same as the present-surface sessions).
- Clippy: glass crates clean under the configured gate. Cockpit clippy
  and `cargo fmt --check` remain red exactly as at the `dev` base
  (37 error lines / 119 files both sides — measured against a base
  worktree); this branch adds zero new entries in either config
  (default and `--features glass`). Rustc warnings: 14, the base count.
- Node probes: `stage-alpha.mjs`, `stage-fuzz.mjs` green; python bench
  tests green.

## Integration notes

- Main-tree collision risk: the parallel session has uncommitted edits
  in `ui/draw/stage_view.rs`, `ui/glass.rs`, `ui/mod.rs`,
  `crates/glass/src/{window,render}.rs`, `crates/stage/src/lib.rs`
  (per the Oct-16 handoff).
  This branch touches `present.rs` (the Oct-16 surface), `viewer.rs`
  (one arm in `place_game`), `ui/draw/stage_view.rs` (document branch +
  one helper at file top), `crates/glass/src/{graphics,lib,page}.rs`,
  the web shell JS (2 files), and adds bench/doc files. Expect a small
  conflict in `stage_view.rs` only.
- `Showing::Video` changed shape (`{source}` → frame-carrying). No
  producers existed; the one test was updated in the same commit.
- To re-run the proof: `scripts/bench/render/run_scorecard.sh all <angel>`
  (nested seat must be up; kit at `/work/artifacts/glass-test-kit`).
  Raw runs used here live under `/work/tmp/render-scorecard/`.

## V2 addendum (later 2026-10-17)

The pre-registered v2 in `docs/RENDER_SCORECARD.md` was executed
(steelmanned Kitty: `kitty-steel.conf`). Headline change: **glass
does not beat steelmanned Kitty on every metric.** M1 key→frame
and the M2 blast4 series go to kitty, M7 exposes a cockpit ~13 fps
video-present cap, and M8 proves the two compositors blend alpha
in different colour spaces (glass sRGB/browser, kitty
linear-light). Glass still wins big streams (4K ≈11×), text,
placements-1000 and adversarial churn (≈7×), ties submit→photon
medians on the external stamp endpoint (kitty tail tighter), and
holds vsync p99 at 4.84 ms.

- Raw evidence is committed: `docs/telemetry/render-v2/` (series,
  kitty JSON, keyframe samples, photon stamp logs +
  `photon-latency.txt`, `m8-golden2.json`).
- Regression gate: `scripts/bench/render/check_regression.py`
  against `baseline-v2.json`; green on both committed runs;
  `python3 scripts/bench/render/test_*.py` all pass.
- Follow-ups v2 created, in impact order: (1) cockpit video tick
  cap (M7 — user-visible), (2) M1 present→compositor gap
  (in-process key→present is 0.64 ms p50; external mailbox 6.7,
  immediate-present probe 5.96 vs kitty steel 4.7 — present mode
  is a minority share), (3) decide whether linear-light blending
  should be an option (M8 — documented divergence, not a bug).
- Bench-harness notes: pass the angel binary as an **absolute**
  path to `run_scorecard.sh keyframe` (the runner cd's into an
  isolated work dir; a relative path exits 127 and looks like a
  cursor-seek failure). mpv timing files must not share stdout
  with the kitty escape stream (a 296 MB lesson, cleaned up).
  One runner-launched immediate keyframe attempt lost its angel
  (clean exit within a minute, reused run-name state suspected);
  the same probe run directly with a fresh run name worked —
  prefer fresh run names for one-off probes.

## Weakness-fix pass (same day, later)

Executed the six-step plan from the weakness analysis; one commit
per step (`ef6c2d3b`, `5ebfa4d5`, `5f2c02e9`, `b925bbe8`,
`a1c1a967`, plus this docs/step-6 commit). Summary in the
scorecard's "Weakness-fix pass" section. What changed in verdicts:

- **M7 is fixed.** Root cause was sharper than "app cadence":
  `fps_cap=12` in Full motion plus no loop lane. Now source-rate.
- **M1's cause moved.** `GLASS_KEYHOP` + `ANGEL_FRAME_TIMING_LOG`
  convict ambient side-column/stage paint in the cockpit draw
  (3.52 ms glass vs 1.23 kitty-hosted; ~0.4 with comp/backdrop
  off). Glass's present path itself beats kitty steel in comp
  mode (4.17 vs 5.69 ms same-session). Remaining work, in order:
  (1) scenery-rate ambient repaint so key frames don't repaint
  the side column (the real M1 fix), (2) verify >60 Hz cadence on
  real hardware, (3) decide the GPU-premultiply default after a
  translucency-heavy workload exists to judge it by.
- Profiling note: `perf` is absent and ptrace_scope=1 blocks gdb
  attach from the agent shell; the GLASS_KEYHOP / frame-timing
  envs are the supported way to split key latency here.
- Pre-merge command list lives in the scorecard (Regression gate
  section). This pass also ran the full cockpit suite once —
  result in the step-6 commit message.
