# Render foundation

angelX draws its own pixels. In a window of its own, the cockpit's text is one
layer of a GPU compositor, and anything else stacks above or below it at the
same time: transparent 3D models, video, video with an alpha channel, floating
panels, other players' layers. The same renderer runs in the browser seat over
WebGPU. That is the base for a multiplayer terminal.

The plan from 2026-10-16 on, with everything WebGPU and 3D underneath, is
[GLASS_CORE.md](GLASS_CORE.md). This is a design for the 0.3 line, written 2026-10-15. Work happens on the
`render/foundation` branch (`/work/angelX-render`), behind a cargo
feature, and reaches dev only through reviewed merges.

## Principles

- **Ours.** No ported terminal. Nothing from Rio, Sugarloaf, libghostty, Warp
  or another emulator's architecture. Libraries are fine for platform plumbing
  (wgpu, winit, a font shaper, ffmpeg); the compositor, the text renderer, the
  layer model and the protocol are written here.
- **The terminal stays first-class.** The TUI in a plain terminal, over SSH or
  in tmux is unchanged. The window is a second host for the same cockpit, not a
  rewrite of it.
- **One renderer, two hosts.** wgpu runs natively (Vulkan, Metal, DX12) and in
  the browser (WebGPU, WebGL2 fallback). The compositor compiles for both.
- **No picture crosses the network.** As in the Delve engine
  ([ENGINE.md](ENGINE.md)): seats exchange layer state and asset references,
  and every seat draws its own picture.
- **Nothing slower.** Off by default, behind the `glass` cargo feature, so a
  terminal-only build pays nothing. Keypress-to-photon in the window must meet
  the cockpit's existing 25 ms budget.
- **Measured and looked at.** Every milestone ends with a latency number and
  screenshots someone opened.

## What exists today

| Piece | Where | What it does |
|---|---|---|
| TUI | ratatui 0.30.2 on crossterm; `ui()` in `cockpit/src/ui/draw.rs`; loop in `cockpit/src/main.rs` (`run()`) | On-demand redraw: 200 ms idle, 33 ms busy, 60 fps with a delve on screen |
| Input | reader thread in `cockpit/src/ui/input.rs`; Kitty keyboard protocol pushed for games (`cockpit/src/ui/term.rs`) | crossterm events into the app |
| Images in the terminal | `cockpit/src/ui/dots/protocol.rs`, `cockpit/src/ui/viewer.rs` | Kitty protocol: base64 PNG, Unicode placeholders, two ids swapped for animation; sixel, iTerm2 or half blocks elsewhere |
| Video | `vendor/dotmax` `video` feature (ffmpeg-next 9); `cockpit/src/ui/scryglass.rs` | Decodes to RGBA, then draws braille or Kitty images |
| 3D | `crates/delve/src/render/world3d/` | Software rasterizer with a z-buffer at braille-dot resolution |
| Games | `crates/web/src/lib.rs`, `crates/muster` | Software framebuffers (`0xAARRGGBB`, 320×180) |
| Browser seat | `crates/web/shell/present.js` | Uploads the framebuffer as a WebGL2 texture, nearest-neighbour |
| Netcode | `crates/net`, `crates/serve` | Authoritative server, commands up, snapshots and patches down, WebSocket |
| Shell pane | vt100 + portable-pty + tui-term (`cockpit/src/platform/pty.rs`) | A real terminal emulated in-process as a cell grid |

## The key move: a ratatui backend that draws on the GPU

The cockpit already produces a grid of cells every frame. In the window there is
no need to emulate a terminal: a `GlassBackend` implements ratatui's `Backend`
trait and hands the cells to the compositor. Every panel, the realm, the
toolstrip and the shell pane come along unchanged, because they already draw
into ratatui's buffer.

- `draw()` receives the changed cells. The backend applies them to its own full
  grid; `flush()` publishes a snapshot of the grid to the render thread.
- `size()` and `window_size()` report cells and pixels from the window, so the
  existing layout and the cell-pixel maths in `viewer.rs` keep working.
- Input: winit keyboard and mouse events become the same crossterm `Event`s the
  reader thread delivers today, with press, repeat and release (what the Kitty
  keyboard protocol gives games now).
- Images: in the window, code that would send Kitty escapes hands pixels to a
  layer instead (see the layer sink below). Until that lands, image areas show
  their half-block fallback.

## Architecture

```
cockpit app thread                     render thread (owns the GPU)
  ui() -> ratatui Buffer                 winit event loop (main thread on macOS)
  GlassBackend.draw/flush  ── grid ──▶   compositor
  layer sink               ── layers ─▶    background (window alpha)
  input channel            ◀── events ─    layers below text (z < 0)
                                           text grid (z = 0)
                                           layers above text (z > 0)
                                           cursor and selection
                                         present (vsync or mailbox)
```

### Host

On a desktop `angelX` opens a winit window with a transparent
wgpu surface. winit owns the main thread, which macOS requires; the cockpit's
app loop runs on its own thread and talks to the renderer over channels. A
redraw is requested when a grid snapshot or a layer change arrives, so idle
costs what it costs today.

### Layers

A layer is one entry in an ordered list:

- **kind**: `cells` (the text grid; more than one is allowed, e.g. a floating
  shell), `framebuffer` (an RGBA buffer the producer updates), `video`, `mesh`
  (a 3D scene), `panel` (an image or document card).
- **placement**: a rect in cells or pixels; it may float past the grid or the
  window.
- **z**: below text is negative, above is positive.
- **opacity, blend, clip, transform**.

Premultiplied alpha everywhere. The layer list is plain data
(serde-serializable), so the same description drives the native window, the
browser, and a remote seat.

The **layer sink** is the cockpit's API for anything that is not cells:
`sink.framebuffer(id, w, h, rgba)`, `sink.video(id, source)`,
`sink.mesh(id, scene)`, `sink.place(id, placement)`, `sink.remove(id)`. In
terminal mode the same calls fall back to today's paths (Kitty, braille, half
blocks).

### Text

Our own glyph atlas and instanced quads: one quad per non-blank cell, colours
and attributes per instance, underline and strike styles as geometry.

- Shaping and rasterizing come from a font library (decision below); the atlas,
  caching and drawing are ours.
- **Braille, block and box-drawing characters are drawn by our code, not taken
  from a font.** They are rendered into the atlas at the cell's exact pixel
  size (`crates/glass/src/sprites.rs`), so the realm, the Delve and the
  minimaps tile without seams at every size and need no font that covers them.
  Lines are 1 and 2 points thick, as Kitty draws them; braille is round dots
  (`ANGEL_GLASS_BRAILLE=pixels` fills each eighth instead).
- Font fallback for emoji and CJK, and HiDPI scale factors from the window.

### Media

- **Framebuffers.** A game's 320×180 `0xAARRGGBB` buffer becomes a texture,
  nearest-neighbour, at the window's full resolution: the Delve at 60 fps with
  no PNG encode and no Kitty transfer.
- **Video.** Decode through the existing ffmpeg path into RGBA and upload per
  frame, which is enough for 1080p60. **Alpha** uses stacked alpha: colour and
  matte side by side in one ordinary H.264, HEVC or AV1 stream, so any decoder,
  hardware ones included, can play it, and a shader recombines them. VP9-alpha
  and HEVC-alpha files are also accepted where the decoder supports them.
  Hardware decode straight into GPU memory (VA-API or Vulkan Video, and
  VideoToolbox) is a later optimization.
- **3D.** glTF scenes with a small PBR-lite shader, depth-tested opaque pass,
  then sorted transparent pass. The software rasterizer stays for terminal mode;
  the same meshes feed both.
- **Panels.** Floating cards for images, documents and media, above or below
  the text, movable.

### Window transparency

The window can show the desktop through any pixel the layers leave empty. This
works differently on each platform, and the M0 prototype checks each one:

| Platform | Expected route | Risk |
|---|---|---|
| Linux, Wayland (Hyprland) | Vulkan surface, premultiplied alpha mode | Low |
| Linux, X11 | ARGB visual | Medium |
| macOS | Metal, transparent NSWindow | Low |
| Windows | DX12 with a composition swapchain | Medium; verify wgpu support |

If a platform cannot do window alpha, the window keeps an opaque background and
every layer still composites inside it.

## Browser

The compositor crate builds for `wasm32-unknown-unknown` with wgpu's WebGPU
backend, falling back to WebGL2. The browser shell's `present.js` becomes one
host of the compositor instead of a one-texture blit. Browser seats get the same
layers, text and media as the window.

## The multiplayer terminal

- **Layers replicate as entities** over `crates/net`: a layer's description,
  placement and asset reference, never its pixels.
- **Shared or private.** A shared layer (a model on the table, a video everyone
  watches) is the same on every seat; a private one stays local.
- **Assets are addressed by content hash**, fetched once and cached.
- **Video stays in sync by timeline:** a seat seeks to the shared clock instead
  of receiving frames.
- **The host decides what a guest may place.**

## Milestones

Each milestone ends with a number and screenshots.

1. **M0 prototype: the cockpit in a window.** `angelX --glass` opens a
   transparent window; `GlassBackend` draws the full cockpit, with braille and
   blocks as geometry; input works; one transparent quad sits above the text.
   - **Exit:** daily use is possible.
   - Keypress-to-photon at 25 ms p95 or better at idle, measured with the
     existing latency harness adapted to the window, against Kitty on the same
     machine.
2. **M1 layers.** The compositor with z-order, the layer sink, and the Delve's
   framebuffer as a native texture layer, plus a floating panel.
   - **Exit:** the Delve plays at 60 fps in the window with no Kitty.
3. **M2 media.** Plain and stacked-alpha video layers and a glTF layer with
   transparency.
   - **Exit:** one scene with 1080p video, an alpha video, a transparent model
     and live text, all at once, holding 60 fps.
4. **M3 browser.** The compositor in wasm over WebGPU, with the browser seat on
   it.
   - **Exit:** the M2 scene in Chrome and Firefox.
5. **M4 shared layers.** Layers replicated through `crates/net`.
   - **Exit:** two seats see the same model and the same video in sync.
6. **M5 parity.** IME, font fallback, HiDPI, accessibility, macOS and Windows,
   and prebuilt packages with and without `glass`.

### M0 progress (2026-10-15)

Landed on `render/foundation`:

- **`crates/glass`**: the window (winit; wgpu on Vulkan or Metal), the
  `GlassBackend`, and the compositor: one instanced-quad pipeline,
  premultiplied alpha, a transparent surface. It also has our glyph atlas over
  swash outlines and colour bitmaps, box lines, blocks and braille drawn by us,
  and font fallback through fontconfig (emoji, CJK). Window keys and the mouse
  arrive as crossterm events, with press, repeat and release. Solid layers, a
  latency probe and an in-window capture are in too.
- **The cockpit**: the `glass` feature, on by default. `ANGEL_GLASS=1`, which
  the launcher sets on a desktop, runs `run()` on its own thread against the window. The title, the bell, the
  clipboard and the game keyboard mode go to the window instead of stdout.
  Images use half blocks until the layer sink exists.
- **The look** comes from Kitty's config when there is one: font, size, the 16
  colours, background opacity, padding. `ANGEL_GLASS_*` overrides it.

Measured on the daily machine (RTX 3090, Vulkan, Mailbox, Hyprland) with the
release build and the practice driver. The probe typed 30 keys into the
composer through the window's input channel; three runs:

| | p50 | p95 | worst |
|---|---|---|---|
| Window: keypress to `present()` | 0.43–0.47 ms | 0.55–0.75 ms | 0.86–1.28 ms |
| Window: its own frame (build, upload, present) | 0.12–0.15 ms | 0.15–0.36 ms | |
| Terminal: keypress to bytes on the PTY (existing harness, 2026-10-09) | 0.28–0.33 ms | 0.33–0.42 ms | |

The terminal number stops where Kitty starts: Kitty then waits out its
`input_delay` (3 ms) and `repaint_delay` (10 ms) defaults before it draws.

To compare the two end to end, both cockpits ran in a private nested Hyprland
seat, with the same release binary and environment. `keyframe` typed 30 keys
from its own virtual keyboard and timed each key to the compositor frame
that showed the character (screencopy with damage). The tool is in
`/work/artifacts/glass-test-kit`.

| Keypress to compositor frame | p50 | p95 | worst |
|---|---|---|---|
| Glass window (two runs) | 3.6 / 5.1 ms | 11.5 / 10.5 ms | 14.1 / 11.9 ms |
| The terminal cockpit in Kitty (two runs) | 10.0 / 10.0 ms | 14.7 / 11.3 ms | 15.1 / 13.3 ms |

Kitty ran on the daily Kitty config without its background image (the probe finds the
cursor by its white block). Both runs share the nested compositor's frame
pacing, so the gap is the terminals' own: Kitty's floor is its repaint delay.

Screenshots, opened:

- `docs/images/glass/cockpit-window.png`: the cockpit at 0.82 background
  opacity over the desktop. `hello` was typed through real key events, and the
  M0 demo quad sits above the text.
- `docs/images/glass/test-card.png`: `cargo run --example sampler` in
  `crates/`. It shows styles, the palette, box drawing, blocks, braille, emoji,
  CJK and combining marks.

Run it with `cargo build --release --features glass` in `cockpit/`, then
`ANGEL_GLASS=1 target/release/angel`. Ctrl+Shift+V pastes, and
Ctrl+Shift+= / − / 0 zoom. `ANGEL_GLASS_PROBE=1` prints the latency line, and
`ANGEL_GLASS_DEMO=1` adds the quad.

Driven in the nested seat, with its own keyboard (`wtype`), pointer (a
wlr virtual pointer) and clipboard: typing, Enter, Ctrl+Shift+V paste, zoom,
a click on a realm button, a drag selection that the cockpit copied to the
clipboard, and the wheel scrolling the transcript. `angelX --glass` built the
`glass` release through the launcher and ran the window there. Stopping it
with SIGTERM found a crash: the reaper thread's `exit()` tore the NVIDIA
driver down while the window was presenting. A signal's exit now runs on the
window's thread after the GPU is released, and TERM, INT, HUP and a normal
close all exit cleanly.

### Parity with Kitty (2026-10-15, later)

The bar before anyone runs it: the window must look at least as good as the
cockpit in Kitty, and be faster.

- **Pictures.** The cockpit already sends every picture to Kitty as Kitty
  graphics: an upload, then Unicode placeholder cells. This covers the realm
  art, portraits, the Round Table and the Delve's frames. Both travel
  inside cell symbols, so the window decodes that subset
  (`crates/glass/src/graphics.rs`: transmit, placeholders, delete) into GPU
  textures. Every picture path shows at full resolution with no second code
  path in the cockpit. Decoding runs on its own thread, so a large PNG or a
  60 fps animation never holds up a frame, and an animation's frames write
  into the texture it already has. This is our own decoder of a public
  format, not a ported terminal. The layer sink stays the API for sources
  that never were Kitty pictures.
- **The look.** Kitty's system config (Omarchy's
  `/etc/xdg/kitty/kitty.conf`) is read first, then the user's, as Kitty
  does. That gives the same font, size, colours and 14 pt padding, with the
  grid centred the way Kitty places it. The frame lands on the same pixels
  as in Kitty. Kitty's `background_image` draws under everything with its
  layout and tint. Text weight was measured against Kitty: gamma 1.0 matches
  its brightness on the same text (`ANGEL_GLASS_TEXT_GAMMA` tunes it). A bold
  fallback glyph comes from the fallback font's own bold face.
- **Keys.** Shift+Insert pastes, as Omarchy's Kitty maps it.
- **Measuring.** `ANGEL_GLASS_STATS=1` prints frames per second, render
  times and picture decoding every two seconds.

Speed with pictures on. Release build, `keyframe`, 30 keys, keypress to
compositor frame. The nested seat is a focused window typed into by a
virtual keyboard, as in real use. The real-compositor run types into an
unfocused offscreen window through Hyprland, which switches focus around
every key and adds the headless output's extra frame, so its absolute numbers
are higher for both.

| | Glass p50 / p95 | Kitty p50 / p95 |
|---|---|---|
| Nested seat, realm animating | 6.0–7.7 / 11.9–14.1 ms | 9.5–14.6 / 17.4–19.9 ms |
| Nested seat, at rest (`ANGEL_TUI_MOTION=off`) | 7.8 / 8.9 ms | 12.5 / 16.0 ms |
| Real compositor, offscreen window | 16.5 / 19.5 ms | 23.3 / 34.7 ms |

Re-measured 2026-10-16 on the hardening branch (same kit, realm
animating): Glass p50 7.4–7.7 / p95 8.2–10.3 ms against Kitty p50
10.2–10.3 / p95 12.5–12.7 ms, and again after the frame-recycling,
buffer-pool and Arc layer-list work landed: Glass 7.4–7.6 / 8.4–8.5 ms
against Kitty 9.5 / 13.3 ms the same night. The margin holds with the
sanitized layer path, the timer-paced presents and steps 3–8 in.

Glass presents within about 1 ms of the key (the in-window probe: p50
1.0–1.2 ms with the realm on screen). Two changes won the animated case:
- **One present per frame.** Each frame is presented once, with its pictures.
- **Focus.** A focus change rides the cockpit's own next frame instead of
  presenting one more ahead of it.

The Delve in release presents exactly 60 frames a second, one per game frame,
with render p50 1.5–2.1 ms and worst 5.4 ms. In the window its frames skip PNG
and base64 entirely: the composed pixels go to the GPU, which scales them
nearest-pixel.

Still open before daily use:

- There is no shaping (ligatures, complex scripts) and no IME yet, and the
  cursor is always a block.
- The Delve reaches the window as PNG frames through the Kitty path; M1 gives
  it a raw framebuffer texture with no encode or decode.

### macOS (2026-10-15)

The same build runs on the Mac mini (Apple M4 Pro, Metal; the release
cockpit is built `--no-default-features --features glass`, since the mini has
no ffmpeg):
- the cockpit, its realm art at full resolution, the portrait;
- the assembly transition;
- emoji, CJK and combining marks.

macOS has no fontconfig, so glass finds a family's files in the font
folders, falls back through Apple Color Emoji, Hiragino and Apple Symbols,
counts points at 72 dpi as Kitty does there, and maps Cmd+V and
Cmd+= / − / 0. Metal offers no Mailbox present mode (Fifo is used) and no
premultiplied window alpha, so the window is opaque on macOS.
`docs/images/glass/cockpit-macos.png` is the cockpit there.

### M1 progress (2026-10-15)

- **The layer sink.** A layer is a solid colour or a picture handed over with
  `Host::upload_picture`. It is placed in pixels or cells, sits above or below
  the text in z order, and can be turned. A producer replaces its whole set of
  layers once a frame with `Host::set_layer_group`.
- **Height.** A layer lifted toward the camera grows and rises up the screen,
  while a soft shadow stays on its ground footprint. That's the primitive for
  thrown things and boss skills; `ANGEL_GLASS_DEMO=lob` shows a bomb lobbed
  over the whole cockpit.
- **The Delve's thrown things fly in the window.** In the window, hob bombs,
  implings and globs leave the game picture; their landing rings stay. The
  Delve client reports each one's ground point and flight progress at the
  frame rate (`arena::lifted`). The cockpit lifts them along their arcs,
  three times higher toward the camera than the picture's arc. They fly over
  the whole cockpit, past the game's edges. The 30 Hz simulation is
  unchanged, and terminals still draw them in the picture.
- **The Delve's frames** reach the GPU as their own pixels (no PNG), and the
  window presents exactly 60 a second.

Next on M1: boss skills as lifted layers (the bosses' shots are flat today),
a floating panel, and the art's frame as a layer instead of picture cells.

### The blast: a multimedia stress scene (2026-10-15)

`ANGEL_GLASS_DEMO=blast` (`crates/glass/src/blast.rs`, every piece a pure
function of the clock, unit-tested without a window) throws the M2-shaped
load at the compositor over live text: an animated test card raycast onto
a framed plane spinning in 3D and streamed as a same-size picture upload
per frame (the video path, until a video layer kind exists); raycast
spheres and a turning cube as lifted, bobbing picture layers; sine ribbons
as a picture; and a randomized formation of up to 260 thin solid segment
layers — waves, starbursts, weaves, spirals — alternating below and above
the text, re-seeded every four seconds. Everything drifts on Lissajous
paths like a screensaver. Layers are sanitized at the draw boundary
(non-finite or empty dropped, colours and lifts clamped), and stage
textures stay inside the 8192 limit every backend guarantees, so a faulty
or remote producer cannot poison a frame.

Measured in the nested seat (RTX 3090, Vulkan, Mailbox, sampler card
underneath, 255 layers): generation 2.6 ms/frame on the CPU for all four
picture streams and the formation; the window presented ~95 fps with
render p95 4.7 ms. `docs/images/glass/blast.png` is the scene.

Scaling the formation with `ANGEL_GLASS_BLAST_X` (same seat, the four
streams running throughout, ~76–80 MiB of picture uploads per 2 s window
at every step): 255 layers build in 0.19 ms mean; 12,349 layers (×64)
build in 0.90–0.98 ms across runs with render p95 3.1–6.1 ms at ~85 fps
(the spread is present, which varies with compositor load) — well inside
a 60 fps budget, after draw parts learned to join their neighbours by
texture instead of allocating a buffer per layer, sanitizing stopped
copying sound layers, and frame buffers began recycling (the same scene
built in 1.79 ms before the joining). The split timing (build, acquire,
submit, present, in `GLASS_PARTS`) shows the floor is present itself
(~1–2.4 ms mean), then build at roughly 0.07 ms per thousand segment
layers; applying the picture uploads costs ~0.1 ms per blast frame.
Nothing here calls for a compute kernel yet; GPU particles remain the
first candidate when a profile does.

## The game pipeline: the game describes, glass draws

Today the Delve paints one finished picture per frame on the CPU, at the art
tier's size (384 × 224, or 768 × 448 for the second kit), and the window
scales it. The picture caps everything: resolution, light, motion between
pixels, effects. The new pipeline moves the drawing to the window: the game
hands over a **scene**, a description of what is where, and glass draws it
on the GPU at the window's own density. Terminals and the browser keep
working: a CPU painter makes the same picture from the same scene.

The window renders a scene into its own texture each frame (a **stage**),
and the cockpit's cells show that texture as they show any picture, so menus
over the game, the assembly transition and resizing keep working unchanged.

The list, in order. Each step ends with before and after shots and timing.

1. **The scene as data.** The Delve hands over its parts instead of one
   picture: the room's still art (already cached per room and torch step),
   the camera, the moving parts in room pixels, and the screen marks in view
   pixels. `frame_for` composes the same parts for terminals; a test proves
   the composed picture is pixel for pixel what it was.
2. **Glass draws the stage.** A stage is a list of layers in the game's own
   pixels, rendered into a texture at the window's density: the room art
   uploaded once per change and scrolled by the camera, the moving parts,
   the screen marks. *Exit:* the Delve looks the same at rest and the CPU
   does less per frame.
3. **Figures as sprites.** Knights, monsters and shots leave the moving
   layer as sprite records: picture, feet point, depth, height. Glass sorts
   them, draws soft ground shadows and lifts them (jumps, knockback, lobs;
   today's thrown things fold in). Positions and the camera are fractional,
   so motion is smooth at the window's density instead of stepping a whole
   game pixel at a time.
4. **Light.** Torches, braziers, lava, the light home, shots and spells
   become light records. Glass draws warm pools that flicker, restrained and
   tonal, over the dark.
5. **Particles.** Emitter records: embers over torches, dust where knights
   run, sparks on hits, motes in the light. The window simulates them.
6. **HD art packs.** Sprite sheets keyed by sprite name, drawn where a pack
   has one, the game's own sprite where not. Terminals get an automatic
   downsample. First pack: the knights.
7. **Cinematics.** The room change as a window transition (the stone door),
   and boss entrances with a camera push.
8. **The browser.** The browser seat draws the same scene on WebGPU (WebGL2
   as a fallback) instead of blitting a finished frame.

### Pipeline progress (2026-10-15, night)

- **1–2, the scene and the stage.** `arena::scene_for` hands the frame over
  as a `RoomScene`; `RoomScene::compose` is the terminal's picture, and a
  test holds it pixel for pixel to `frame_for` over six fights, the start
  and treasure rooms and a scrolling great hall. In the window the stage
  draws it (`Host::set_stage`); the room's still art goes up only when it
  changes. Plates, the kingdom, a pastime's own screen, rooms lit from below
  or put out, and the second art kit still come as one picture on the same
  stage. The window always draws that stage; it no longer accepts a finished
  Delve picture for comparison.
- **3, figures.** Each knight and monster is painted on its own, over the
  room as painted so far (a shadow darkens what it falls on), and handed
  over with everything painted with it: its shadow, bar and marks. What is
  painted after the crowd (glows, shots, names, boards) keeps only what it
  changed, so the composition stays exact. The window places each figure
  at its true position between the game's pixels, and the camera
  (`arena::camera_exact`) scrolls between them too; the terminal's camera is
  its whole pixel, unchanged.
- **4, light.** The bake already gathered each room's lights (torches,
  braziers, the altar, lava) and threw them away; it now keeps them beside
  the cached art. With shots, bursts and the light home they come as
  `SceneLight`s, drawn as additive pools that flicker on the window's own
  clock, restrained (a third of a light's strength at its middle).
- **5, motes.** `Emitter`s: embers off every fire, dust turning in the
  strong lights, sparks where a blow lands on a knight or a monster. The
  window animates them as a pure function of its clock
  (`LayerKind::Particles`), so nothing is simulated or kept.
- **6, HD art packs.** Each figure names the sprites stood up in it by look
  (`arena::look_key`, a hash of the sprite's pixels). The window draws a
  pack's HD picture of a look in that sprite's place and takes the sprite's
  own pixels out from under it. The pack is a folder of `<look>.png`
  (`ANGEL_GLASS_ART`, default `~/.angelX/art/delve`);
  `ANGEL_GLASS_ART_EXPORT=<dir>` saves every look the game shows, once, with
  `looks.jsonl`, as the list for the art pipeline to redraw. No pack exists
  yet: the knights are the first to draw.

  **Art request, the knights' pack.** Play a few rooms with
  `ANGEL_GLASS_ART_EXPORT=<dir>` and take the knights' looks from `<dir>`
  (each `<look>.png` is one sprite at native size, 16-odd pixels tall).
  Draw each at four times its size in the Metal Slug direction, dark
  palette, keeping the sprite's own rectangle: same aspect, the figure where
  the sprite's figure is, transparent around it, so it lands exactly over
  the sprite. Name each drawing as its look and put them in
  `~/.angelX/art/delve/`. Flashes, frost and the veil are looks of their
  own and can stay native for now; the plain walking and standing looks
  are the ones seen most.
- **7, cinematics.** Through a door the new room slides in the way the party
  went, the old one out (0.42 s, eased); the new room is live while it
  slides. A guardian's entrance leans the camera in toward it and back.
- **8, the browser.** What a stage is lives in its own small crate,
  `crates/stage` (`angel-stage`): layers, motes, flicker, pictures, a frame,
  `expand` to plain quads, and `wire`, the buffer the page reads. The Delve
  makes its stage once (`arena::stage_frame::StageMaker`) and both drawers
  take it: glass draws the layers; the browser runtime asks the game for its
  stage after each frame (`GameClient::stage`, exported as `angel_stage`)
  and the page draws the quads on WebGL2 (`shell/stage.js`) into a texture
  of its own, so rooms slide there too. Asking turns the game's stage on.
  WebGL1 and the 2D canvas keep finished frames, and `?stage=0` keeps them
  on WebGL2. A picture goes again only when it changed (the room's still
  art once a room or torch step, the rest when they move).

Measured on busy crypt fights (`delve-dev-host --scene fight`):

| | Finished frame | Stage |
|---|---|---|
| Native, making the frame (release) | 0.06–0.27 ms | 0.10–0.16 ms, + 0.05 ms to make, expand and encode the stage |
| Browser, module per frame (p50 / p95) | 0.7 / 1.8 ms | 1.5 / 2.8 ms |
| Browser, present (p50 / p95) | 0.1 / 0.2 ms | 0.4 / 0.8 ms |
| Browser, key to the knight moving | 1 frame (3 of 3) | 1 frame (5 of 5) |
| Window, render p50 / p95 (light and motes on) | 0.92 / 2.1 ms | 1.18 / 2.6 ms |
| Window, presents a second | 60 | 60 |

An animation frame (motes, flicker) rides on a frame the cockpit just
presented; with none in the last 15 ms it presents on its own, a display
frame later, on a timer.

The browser runs were headless Chromium on the desktop's GPU, 1280 × 800,
60 fps with no frame dropped either way.

### The hybrid: a tilted floor and shots of light (2026-10-16)

The Delve in the window is now a 2D/3D hybrid: pixel art standing in a
lit diorama.

- **A tilted floor.** A stage frame carries `tilt` (`stage_frame::TILT`,
  0.3) and every layer says how it stands (`angel_stage::Stand`): the room,
  what is painted under and over the crowd, the light and the motes lie
  on the **floor** and tip away with it; knights, monsters and shots stand
  **upright** on their foot point, smaller further away and never tipped;
  the screen's marks stay on the **screen**. One definition of the
  projection (`angel_stage::Tilt`) serves the window (`glass.wgsl`, with
  perspective-correct pictures) and the browser (`stage.js`). The near
  wall lands on the stage's bottom edge, so the whole floor stays in view
  and the far wall recedes into the black above. Thrown things over the
  cockpit follow the same floor.
- **Shots.** In the window and the browser a shot in flight is handed
  over whole (`room_scene::SceneShot`) instead of painted: its shadow on
  the floor, and lifted over it a comet's tail in its colour, a halo, its
  sprite (the terminal's own, on a sheet of its own) and a white-hot core.
  Arrows, blades, bones, ore and pages keep their sprite with a speed line.
  Where shots crowd (a ring just thrown) each glows less, so a burst never
  burns white. A terminal still paints shots in the picture as before.
- **The browser's wire is version 2** (`STG2`): each quad carries its turn
  and its stand, the frame its tilt.
- **A shot gallery.** `delve-dev-host --scene volley` throws a ring of
  every kind of shot every 0.8 s in a room with no monsters, for looking
  at how shots fly.

Measured in the browser seat (headless Chromium on the desktop's GPU,
1280 × 800, the gallery): 60 fps with no frame dropped, module p50 / p95
0.5 / 1.1 ms, present 0.6 / 0.8 ms.

### No Kitty in the window (2026-10-16)

The window was its own terminal from the start; two Kitty leftovers are
gone.

- **Pictures.** Every picture reaches the window as its own pixels. The
  viewer's pictures (`ui::viewer::Pic`: portraits, stills and `/show`,
  video, the council table, the portal) are resized once on their worker
  to the cells they cover and handed over with `Host::upload_picture`; the
  fine dots hand over their raster; the game and map frames already did.
  Each shows through picture cells (`graphics.rs`: the placeholder with its
  row and column marks, its colour naming the picture). Glass no longer
  parses escape codes, assembles uploads, or decodes PNG or base64: the
  decoder thread and the `base64` dependency are gone, and a cell that
  still carries escape codes is drawn blank and said once on stderr. In a
  terminal nothing changed.
- **Settings.** `~/.angelX/glass.conf` is glass's own file, one
  `key value` a line. The first launch writes the defaults there; no other
  program's config is ever read.

### Next: transitions

Every cell is its own GPU quad, redrawn every frame, so the window can
animate the whole cockpit. The first one is an assembly transition in a
fantasy-engineering style (a stone door, a cuckoo clock, gnome works) rather
than a mechanical one. Floor tiles (text, pictures) fly in and settle with a
small overshoot and a stone thunk. Walls (the box-drawing borders) slide in
from the top and bottom and lock into place with a ratchet beat. Dust, gears
and sound come as layers.

### Panels over the terminal (2026-10-17, night)

Two layer producers float above the text, both leaning the same way (one
shared tilt about a panel centre, `ui::orrery::tilted_rect`), both shown as
one layer group a frame:

- **The Orrery** (`ANGEL_GLASS_ORRERY=1`): the live agent graph as a
  holo-panel. One thread, off the cockpit's loop entirely, re-reads the
  graph snapshot and re-describes the panel at 30 fps; the window animates
  the breathing, flicker and motes itself between descriptions. Nodes are
  lights laid out by dependency depth, edges carry pulses into running
  nodes, and with no graph the panel keeps a sun and two turning orbits.
  Everything the panel draws is a pure function of `(t, snapshot)` with
  no window types, so its geometry is tested in a plain test run.
- **The Stereopticon** (`ui::stereopticon`): the M2 video slice for the
  artifacts pane. In the window a pane video stops drawing braille and
  plays as glass — the same off-thread decoder, clock and controls, one
  RGBA upload a frame, presented as a leaning plate with rim, brackets, a
  progress track with quarter ticks, pause bars, and the screen's light
  spilling on the floor. An ended replay holds a beat and starts again;
  seeks and pauses keep working because the machinery underneath is the
  Scryglass's own. In a terminal every entry point returns false and the
  cells paint as before.

### The post pass (2026-10-18)

A stage's world goes through a post pass before it is shown, in the
window and in the browser alike (`crates/gpu/src/post.rs`, `post.wgsl`).
The world is drawn into a texture of its own; its additive lights (fires'
pools, shots' glow, motes) are drawn again alone at half size; then:

- **Bloom**, only in light: what is bright (its brightest channel over
  0.55, a colour counting fully and a white half) blooms where the stage's
  own lights are, and the lights themselves a little. A knight's gold in
  the dark is paint and stays crisp; a shot's heart in its glow is a flame.
  Halved four times and added back up (dual filter), then *screened* over
  the world, its colour deepened: a halo is the light's colour, never a
  white blowout.
- **Vignette**: the corners fall up to 22% toward black; multiplied, so
  black stays black, and under the bloom, so a flame in a corner shines.
- **Grain and dither**: a still canvas grain (scaling the paint, so black
  and empty channels stay empty) and a 4x4 ordered dither at the last
  step to eight bits, both in whole stage pixels: the pixel art stays one
  colour a stage pixel, the soft gradients keep no bands.
- **Impact**: a frame's `impact` (0-1; the Delve's is its screen shake, a
  guardian's fall or a bomb, and the blows the host holds the world still
  for) splits red and blue outward by whole pixels (up to 3 at the corners)
  and lifts the light by up to 22%, for as long as the shake lasts.

The screen's own marks (a stage's trailing `Stand::Screen` layers: the
Delve's HUD and bands) are drawn after, untouched. Off
(`ANGEL_GLASS_POST=0`; in the browser the dev panel's *post* knob, kept per
device, or `?post=0`) a stage draws straight into its texture as before.
Cost, measured on the busy fight (`delve_busy`): about 0.05 ms of CPU and
two full-size and five small passes on the GPU; in the browser on WebGL2,
~5% of the uncapped frame rate at 1280x800.

## Risks

- **Text quality and latency** decide whether people live in the window. M0
  measures both before anything else is built on top.
- **Threading.** winit must own the main thread on macOS, but the cockpit's loop
  owns it today. The app moves to a thread in glass mode only.
- **Image code paths.** ratatui-image and the Kitty upload path assume a
  terminal. Until the layer sink exists they fall back to half blocks inside
  the window.
- **Build cost.** wgpu, winit and the shaper add compile time and binary size.
  The `glass` feature keeps terminal-only builds and the release prebuilts as
  they are until glass ships.
- **Video codecs.** Use the system ffmpeg as Scryglass does; prebuilts do not
  bundle codecs.

## Decisions for the operator

- **The font shaper.** Either take a shaping and rasterizing crate (swash and
  rustybuzz, or cosmic-text) under our own atlas (recommended), or write the
  rasterizer too. *M0 took the recommendation: swash under our atlas, no
  shaper yet; rustybuzz comes in when ligatures or complex scripts are needed.*
- **Platform order.** Linux on Hyprland first (the daily machine), macOS second
  (the mini), Windows after M3.
- **When glass becomes the default.** Decided 2026-10-16: now, and it
  replaces the terminal emulator. On a desktop angelX opens in its own
  window however it is started; typed in a terminal, the window goes off on
  its own and the prompt comes back, so closing the terminal leaves angelX
  open. It stays in the terminal with `--term` (or `ANGEL_GLASS=0`), inside
  tmux, over SSH, with no display, and in its own shell pane. Every
  interactive build carries the window, so the two never rebuild each
  other. Machine modes stay lean.
- **The overlay variant.** A click-through layer over another terminal's window
  works on Hyprland through layer-shell but not on GNOME. Keep it as an
  experiment after M2, not as the foundation.

## Appendix: environment knobs

Every `ANGEL_GLASS_*` variable the code reads today (glass, stage,
web, and the cockpit's glass glue), so the doc and the code cannot
drift silently. Invalid or non-finite values are ignored or clamped
to the stated range; nothing here can panic the window.

Window and look:

- `ANGEL_GLASS=1` (cockpit) — open in the glass window; the launcher sets
  it on any desktop launch outside tmux and SSH, `--term` sets 0. The shell
  pane's children get 0.
- `ANGEL_GLASS_APP_ID` — Wayland app id (test seats set their own).
- `ANGEL_GLASS_CONFIG` — glass.conf path (else `~/.angelX/glass.conf`).
- `ANGEL_GLASS_FONT`, `ANGEL_GLASS_FONT_SIZE` (1–256) — family and size.
- `ANGEL_GLASS_OPACITY` (0–1), `ANGEL_GLASS_PADDING` (0–512 px).
- `ANGEL_GLASS_SIZE` — initial grid, `COLSxROWS`.
- `ANGEL_GLASS_TEXT_GAMMA` (0.3–3.0).
- `ANGEL_GLASS_BRAILLE` — `pixels` for block braille, else dots.
- `ANGEL_GLASS_WALLPAPER` — background image path.
- `ANGEL_GLASS_GPU` — `low` or `high` adapter power preference.
- `ANGEL_GLASS_PRESENT` — `fifo` or `immediate` (default mailbox;
  measured: mailbox and immediate tie, fifo costs a vsync).
- `ANGEL_GLASS_TRANSITION=0` — skip the assembly transition.
- `ANGEL_GLASS_POST=0` — no post pass over stages (bloom, vignette,
  grain, a hit's split; "The post pass").
- `ANGEL_GLASS_ART`, `ANGEL_GLASS_ART_EXPORT` (cockpit) — Delve HD
  art folder and its export folder.

Measurement and capture:

- `ANGEL_GLASS_STATS=1` — `GLASS_STATS`/`GLASS_PARTS` lines on stderr.
- `ANGEL_GLASS_PROBE=1` — in-window key-to-frame probe;
  `ANGEL_GLASS_PROBE_READY`/`_TEXT` set its strings, `_OUT` its
  report file, `_CLOSE=1` closes when done.
- `ANGEL_GLASS_SHOT` — save one offscreen PNG there;
  `ANGEL_GLASS_SHOT_DELAY_MS` (default 5000), `ANGEL_GLASS_SHOT_CLOSE=1`.
- `ANGEL_GLASS_FILM` — film the assembly transition at 30 fps into a
  folder of PNGs; `ANGEL_GLASS_FILM_DELAY_MS` (default 8000).
- `ANGEL_GLASS_BLAST_FILM` — film the live scene at 30 fps;
  `ANGEL_GLASS_BLAST_FILM_SECONDS` (default 6, clamped 1–60),
  `ANGEL_GLASS_BLAST_FILM_DELAY_MS` (default 3000). Encode with
  `ffmpeg -framerate 30 -i frame-%03d.png out.mp4`.

Demos (over any host, sampler or cockpit):

- `ANGEL_GLASS_DEMO=1` — one translucent quad above the text;
  `=lob` — the lobbed bomb;
  `=slide` — the room slide; `=blast` — the multimedia blast
  (warped video plane, raycast orbs and cube, Z-layered line
  formations, all drifting).
- `ANGEL_GLASS_BLAST_X` — blast formation copies, 1–64
  (64 = 12,349 layers).
