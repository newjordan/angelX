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
