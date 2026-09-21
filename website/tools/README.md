# Frame plates — how the images on the site are produced

Nothing on the page is drawn by hand. Each plate is output of the harness's own
renderers, captured through the production code paths and rasterized.

## 1. Capture (Rust, inert unless `ANGELX_FRAMES_DIR` is set)

```sh
cd cockpit
ANGELX_FRAMES_DIR=/tmp/angelx-frames cargo test --no-default-features frames_ -- --nocapture --test-threads=1
```

| module | writes | what it renders |
| --- | --- | --- |
| `tests/cockpit/app/frames__capture_tests.rs` | `fig01`, `fig01b`, `fig01c`, `fig02`, `fig04`, `figA`, `figB` (`.txt` + `.json`) | the real compositor (`ui/draw::ui`) into a ratatui `TestBackend` — console, mid-coding turn, agent-bay thinking trace, formations deck — plus `crate::stage::raytrace::render`, the Reinforce stage with `crate::ui::chart::line_chart` (star nodes), and dotmax `progress::draw::vblock` vertical bars |
| `tests/cockpit/world_viz/frames__world_tests.rs` | `fig03-world.png` | `world3d::raster::render_scene` on `scene::SceneKey::COURT` — RGBA frames, saved directly |

`fig01`/`fig01b` carry seeded sessions (user turn, answer, tool result, live
partial) so the plates are the console *at work*. `fig01c` seeds
`app.reasoning` + `Thinking::pending_for_test` for the agent-bay trace.
`fig05-operator.png` is a real operator screenshot, not a capture.

## 2. Rasterize terminal frames

```sh
NODE_PATH=/tmp/shot/node_modules node tools/frames-to-png.mjs /tmp/angelx-frames assets/frames --cell 8x16 --scale 2
```

`frames-to-png.mjs` maps ratatui `Color` debug strings (Rgb / Indexed-256 /
named / Reset) plus BOLD·DIM·REVERSED modifiers onto a headless Chromium cell
grid, using a monospace stack chosen for braille + box-drawing coverage.

It rasterizes **only the plates `index.html` references** — the capture run
produces more frames than the page shows, and a capture nothing references is a
byproduct of the run rather than an asset, so promoting it would ship bytes no
visitor loads. `--all` converts every capture; `--only fig01-console,fig03-loop`
names them. Referencing a plate from `index.html` is what puts it in
`assets/frames/`. (`fig03-world.png` is written straight to the capture dir by
the world test above and stays there until a section shows it.)

## 3. Gate

```sh
python3 -m http.server 8711 --bind 127.0.0.1   # from website/
NODE_PATH=/tmp/shot/node_modules node tools/verify-frames.mjs http://127.0.0.1:8711/index.html
```

Exits non-zero unless every plate (`PLATES`, default 5 — the four frame plates
plus the section backdrop `index.html` shows today) loads as a real bitmap
(`naturalWidth > 0`) with no failed requests.

## 4. A slow Y pan over one plate

```sh
cd website
NODE_PATH=/tmp/shot/node_modules node tools/pan-to-mp4.mjs /tmp/angelx-teaser shotB /tmp/angelx-answer-pan.mp4
```

`pan-to-mp4.mjs` takes one captured plate — `shotB` is the answer to "what is
angelX?" holding in the transcript — renders it once through the same headless
cell renderer `teaser-to-mp4.mjs` uses, and rolls a window down it with
ffmpeg's per-frame `crop` expression: a constant-speed Y pan with no easing,
because the pace is the whole point. The still is left beside the captures as
`<prefix>-still.png`.

The duration is not a taste call. `pan-pace.mjs` derives it from the words under
the window at 275 wpm (a moderate delivery) times a 1.8 comfort margin, floored
at 18s, and the tool refuses a pan that would carry a row off screen before a
moderate reader is finished with it:

```text
frame shotB-00 · 120x40 cells · window 72x18 cells (4.00:1)
text 45 words over 8 rows → 9.8s at 275 wpm → take 18s (1.8x margin)
pan rows 0→6 (6 rows = 216 px at 36 px/cell) · 0.33 rows/s · 3.00s per row · 12.0 px/s · 0.40 px/frame
readability: 3.00s per row on screen vs 1.23s to read it (5.6 words/row) → camera never outruns the reader
hold: rows 5–10 (41 words) whole in frame for 15.0s vs 10.4s to read + a beat → held
```

`PAN_WINDOW_ROWS`, `PAN_FROM`/`PAN_TO`, `PAN_CROP_COLS`, `PAN_READ_ROWS` (the
words the shot asks the viewer to read), `PAN_BLOCK_ROWS` (the response itself)
and `PAN_TEXT_COLS` override the geometry. The reading assumption stays at 275
wpm with a 1.8 margin, so a slower shot means fewer rows or a longer floor, never
a weaker reading rule. `PAN_DRY=1` prints the
report and exits without rendering — exit 3 means the move would outrun a
moderate reader. `tests/scripts/pan-pace.test.mjs` pins that arithmetic, so the
"slow enough to read" rule is a test rather than a comment.
