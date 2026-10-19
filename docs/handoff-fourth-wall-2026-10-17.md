# Handoff — the fourth-wall night, 2026-10-17

Branch `feat/fourth-wall` in worktree `.-worktrees/fourth-wall`,
based on dev `fbc41b81`. Everything is committed on that branch; nothing
merged, nothing pushed. The main checkout's own uncommitted work was never
touched.

## What the night built

Two glass systems that float above the black terminal, plus the stress
and measurement pass that proves they cost nothing:

1. **The Orrery** (`ANGEL_GLASS_ORRERY=1`, `cockpit/src/ui/orrery.rs`).
   The live agent graph as a leaning holo-panel in the window's top
   right. One thread of its own — the cockpit's loop never draws it,
   never wakes for it. Nodes are breathing lights laid out by dependency
   depth, edges carry pulses into running nodes, and while no graph has
   run the panel keeps a gold sun with two turning orbits and a wave.
   Pure geometry (`tilted_rect`, `segment_rect`, `node_positions`,
   `ribbon_points`) tested without a window; the assembly into layers is
   deterministic and capped (42 nodes / 96 edges / 300 layers). A hidden
   panel (small grid) is sent once, not per frame.

2. **The Stereopticon** (`cockpit/src/ui/stereopticon.rs`, wired in
   `ui/draw/stage_view.rs`). In the window, the artifacts pane's videos
   stop drawing braille and play as glass: the same off-thread decoder,
   clock, fps cap and controls, one RGBA upload a frame, presented as a
   leaning plate — rim, brackets, screen spill on the floor, a progress
   track on the screen's bottom edge with quarter ticks, pause bars, and
   an ended replay that holds a beat then starts itself again. In a
   terminal every entry point returns false; the cells paint as before.
   This is the RENDER_FOUNDATION "M2 video" slice for the pane, through
   the mini-viz surface's declared-but-unimplemented `Showing::Video`.

3. **Glass probe Enter** (`crates/glass/src/probe.rs`):
   `ANGEL_GLASS_PROBE_ENTER=1` submits the typed draft, so scripted QA
   can drive a command (`/show file.mp4`) in the window, not just text.

4. **Docs**: RENDER_FOUNDATION.md "Panels over the terminal (2026-10-17)".
   Tests: `tests/cockpit/app/orrery__tests.rs` (11 glass / 9 plain),
   `stereopticon__tests.rs` (6 glass / 3 plain). Full suite on this
   branch: the bin suite is fully green (5806 passed / 0 failed); two
   wall-clock latency smokes in `interactive_terminal_smoke`
   (input_latency, motion_unicode — keypress-to-visible p95 25.1–26.5ms
   against a 25.000ms budget, p50 4ms) fail identically on the clean
   base `fbc41b81` with the same sources restored — verified by
   checkout — so they are the shared box's load (another agent's
   `cargo test` and a live angelX session were running), not this
   branch. An earlier full run's yukon/harness pair also passed in
   isolation.

## Stress results (RTX 3090, Vulkan, Mailbox present, 110x34 window)

| case | fps | render p50 | render p95 |
|---|---|---|---|
| baseline idle | 47–52 | 0.46–0.62 ms | 0.72–0.83 ms |
| orrery on | 66–75 | 0.42–0.65 ms | 0.57–0.94 ms |
| blast x1 | 76–90 | 0.42–0.46 ms | 0.52–0.92 ms |
| blast x8 (≈1.5k layers) | 78–88 | 0.44–0.76 ms | 0.77–1.24 ms |
| blast x32 (≈6k layers) | 65–86 | 0.83–1.55 ms | 1.05–4.09 ms |
| blast x64 (≈12k layers) | 65–83 | 1.39–2.33 ms | 1.69–4.79 ms |
| orrery + blast x64 | 74–90 | 1.34–1.55 ms | 1.73–2.27 ms |

Reading: the compositor holds 60 fps at twelve thousand layers; the
Orrery on top of the heaviest scene is free (within noise). The x64 cost
is in `build` (layer expansion, CPU), not submit (≤0.5 ms) — GPU headroom
remains. Filming (the 30 fps offscreen capture path) halves interactive
fps to ~15 while it runs: a QA path cost, not a playback one. The replay
deck uploads ~2.2 MB/s at its 12 fps cap with 1–2 ms of upload cost per
two-second stats window — negligible next to the decoder.

The three-way interleave holds by construction and by measurement: video
decodes on the Scryglass worker, the Orrery describes itself on its own
thread, and the cockpit's phases (poll/input/advance/settle) are not on
any of those paths. `ANGEL_FRAME_TIMING_LOG` from the replay session is
in `artifacts/fourth-wall/frame-timing.tsv`.

## The visual pass

Four capture-and-judge rounds against a vision model, each feeding the
next (scores: 4/10 → 7/10 → 8/10 → 9/10, "strongly surreal yet
precise"):

1. First light read too faint — stronger rim, brackets, bigger lights.
2. Interior didn't inherit the tilt — added foreshortening
   (`deep_point`/`deep_rect`: rows above the centre recede and
   compress), drawn orbit paths, a bigger sun with a core, a grounding
   pool of light.
3. Round things were squashed — the panel was designed in cells, and a
   cell is 12 × 25 px. Rewrote the Orrery in window pixels
   (`Placement::Pixels`); round lights are round.
4. The orbit band and the wave blended — the wave now runs low, under
   the orbits, as two separated bands.

The Stereopticon's one flaw (a screen-space progress bar crossing the
leaned plate) was fixed the same way: back on the plate's plane, inside
the screen's bottom edge.

## How to see it

```
cd .-worktrees/fourth-wall/cockpit
cargo build --release --features glass
ANGEL_GLASS=1 ANGEL_GLASS_ORRERY=1 ./target/release/angel
# and, with the window at 100+ columns:
#   /show artifacts/fourth-wall/replay.mp4
```

Snapshots and films: `artifacts/fourth-wall/` (PNG stills, `replay.mp4`,
`orrery.gif`, `stereopticon.gif`). Regenerate films with
`ANGEL_GLASS_BLAST_FILM=<dir>` then ffmpeg, as ever.

## Environment notes for whoever automates next

- The desktop's `hyprctl` is a Lua-bridged fork: raw window control is
  `echo 'dispatch hl.dsp.window.float("0x..", true)' | nc -U
  /run/user/1000/hypr/<sig>/.socket.sock` — the classic
  `togglefloating address:0x..` syntax is rejected. Enumerate a table
  by throwing its keys through `error()`.
- New angelX windows auto-tile; float them, then
  `hl.dsp.window.fullscreen(addr)` for a ≥100-column grid (the artifacts
  pane needs 100x30 to show media).
- The offscreen capture saves odd-height PNGs; crop to even before x264.
- There is a `GLASSCAP` 1700x1000 virtual monitor on this machine —
  likely a previous capture setup; unused tonight.

## Not done, deliberately

- No `/orrery` command or pane-state integration yet: the panel is
  env-gated (`ANGEL_GLASS_ORRERY=1`). A command that toggles it and a
  "show while a graph runs" policy are one small slice away.
- The Stereopticon presents the pane's existing video only; a film
  carousel (the session's own captures replayed on demand) wants a
  producer that collects `ANGEL_GLASS_BLAST_FILM` output.
- Blast x64's build cost (~2.3 ms p50) could drop with a retained-mode
  layer expansion cache; not worth it until a real scene needs 12k
  layers.
