# Glass term: open intake, glass expansion

`glass` (crates/term) is a terminal in angelX's own window. A program
runs in a pseudo-terminal; what it writes comes in, and glass expands it
into everything the window can show: the text grid, and above and below
it pictures, layers, particles, 3D scenes and the showcase.

Two words carry the design.

- **Intake.** Glass takes in any dialect a program speaks to a terminal.
  No dialect is the reference one. Each is an adapter that turns its own
  escape sequences into one shared description of what to show. Programs
  learn which dialects glass takes in by asking it, and glass answers the
  standard queries truthfully: what it does not take in yet goes
  unanswered, so programs fall back.
- **Expansion.** That shared description is drawn with glass's own
  machinery, which goes far past what any terminal dialect can describe.
  Glass also has a dialect of its own that reaches all of it.

## Where it stands

- `src/engine.rs`: `alacritty_terminal`'s `Term` behind one interface,
  with the queries only the window can answer handled by a scanner in
  front of it.
- `src/session.rs`: the pseudo-terminal, a reader thread (read, then
  `pump`) fed by a read-ahead thread (`src/read_ahead.rs`), the replies a terminal owes (DA1/DA2, DSR,
  XTVERSION, XTWINOPS 14/16/18, OSC 10/11/12, DECRQM 2026, focus
  reports, synchronized output 2026, OSC 52 copy, titles, bell), and the
  environment scrub.
- `src/keys.rs`: keys, pastes and the mouse as legacy bytes, and
  as the CSI u dialect when its flags are pushed.
- `src/view.rs`: the engine's cells as glass cells, selection, words,
  links and URLs.
- `src/main.rs`: arguments (the `xdg-terminal-exec` convention), the
  event loop, selection, copy, scrollback, links on Ctrl+click.
- `src/intake/`: the picture intake (section 1) — the splitter, the
  three dialect adapters, and the frame's layers.
- Pictures reach glass's layers today; the other expansions (particles,
  3D, the showcase) still wait on section 2's own dialect.

## The glass side, for adapters to target

- `angel_glass::Host` (crates/glass/src/lib.rs): `upload_picture`,
  `set_layer`, `set_layer_group`, `set_layers`, `remove_layer`,
  `set_scene`/`remove_scene`, `set_stage`; the showcase in
  crates/glass/src/showcase.rs (`Host::showcase`, `showcase_frame`).
- Pictures: `angel_glass::graphics` / `angel_gpu::Picture`
  (crates/gpu/src/picture.rs). Ids from `LAYER_PICTURES` up belong to
  layers.
- Layers: `angel_stage::Layer` (crates/stage/src/lib.rs). `z < 0` draws
  below the text, `z >= 0` above it (crates/glass/src/render.rs, the
  `below_layers`/`above_layers` split). `Placement::Cells` places by cell
  and allows fractions. Kinds: picture, picture part, solid, glow,
  particles, lit primitives. Up to 65,536 layers in a set.

## 1. Intake: pictures

Built. `src/intake/` holds the core and the three adapters, and
`session.rs` wires them into the terminal's read path and frame.

A stream splitter sits in front of the VT engine. It passes text through
and lifts out the media sequences, keeping their order relative to the
text, so a picture is placed where the cursor stood when it arrived. The
splitter must not depend on the engine behind it: the engine may
change (section 3).
It is incremental (a sequence may split across reads) and engine-blind:
it only produces `Piece`s of text and `Media` payloads.

The core (`src/intake/mod.rs`) owns the one shared description —
pictures and placements, in dialect ids minted per window and mapped to
window ids from `LAYER_PICTURES` up — and reaches the engine only
through the small `Engine` trait (cursor, size, scrolled lines,
alternate screen, feed bytes back). Adapters never see `Host`; the
session applies each frame's `Description` (uploads dirty pictures,
`set_layer_group` with `Placement::Cells` against the visible grid), so
tests run against a recording canvas with no window.

The engine merge is done: the intake is wired to the
`alacritty_terminal` engine through `pump` (section 3), the scrolled
count it hears is exact and keeps counting past a full history, and the
clear and cover operations are driven by the engine's own events —
`ED 2` pushes the screen's lines into history first so pictures in the
scrollback stay on their text, `ED 3` and `RIS` clear as their dialects
ask, and a cell write under an in-cell picture (sixel or OSC 1337)
takes it away whole while an APC-graphics placement survives text,
`EL` and `ED 0`.

What is exact now: the splitter is a state machine whose between-read
state is owned, so no read boundary can lose, duplicate or reorder text
(it takes plain text, a CSI or plain escape complete in the read, and a
string body in bulk, giving the very pieces its byte-at-a-time steps
give; a test holds the two together); every corpus stream proves it at every single split and
as one-byte chunks. Media replies go to the pty right after the media
piece that earned them, in stream order with the text's replies around
them. Both decode paths (`image` for PNG, JPEG, GIF, TIFF, WebP, BMP, ICO, TGA and PNM, `o=z` for zlib) run
under explicit limits — 8192 per side, and an allocation cap — so a
hostile payload cannot allocate before the store judges it. TGA has no
magic bytes, so when guessing finds no format a valid header is tried
under those same limits: image type 1, 2, 3, 9, 10 or 11, a pixel depth
of 8, 15, 16, 24 or 32, and nonzero sides. OSC 1337 `name=` is the
file's name in base64; its extension is the format hint before guessing.

Dialects to take in, each its own adapter module under `src/intake/`:

- `apc_graphics.rs`, APC `G` graphics: transmit,
  transmit-and-put, put, delete, query, animation (`a=f` frames, `a=a` control, `a=c` composition), RGB (`f=24`), RGBA (`f=32`),
  PNG, JPEG, GIF, TIFF, WebP, BMP, ICO, TGA and PNM (`f=100`), `o=z` (zlib), chunked (`m=1` on direct `t=d` only), `t=d` direct data,
  z-index above and below text, source rectangles, cursor movement
  after a put (`C=1` stays), and the `q=1`/`q=2` quiet keys.
    Animation is taken in, and the old limit is gone. `a=f` transmits a frame (chunking, `o=z`, and every transmission medium work as for `a=t`; frame 1 is the picture as first transmitted). `a=a` stops, loads, or runs it, sets the loops (with the off-by-one rule), makes a frame current, or changes one frame's gap. A successful `a=a` sends nothing: the reference answers an animation control only when it fails, and that error still follows `q`. `a=c` composes a rectangle of one frame onto another, blended, or replaced with `C=1`, and likewise answers only on failure. `a=f` answers like a transmit, following `q`. Frames count against the same RGBA budget, and against a store-wide count: pictures (at most 4096) plus frame uploads stay at or under 8000, 192 under the window's 8192, so a frame the store still names is not the one the window drops. Frame 1 shares the picture's window id until `a=f` or `a=c` gives it its own pixels, and is not an extra upload until then. Past the count, `a=f` and `a=c` that would add a frame answer `ENOSPC` and store nothing. Deleting a picture, eviction, and RIS free every frame of the pictures they drop; `d=f` frees the frames and keeps the picture. Each of those gives the count its room back. Each frame is uploaded once, under a window id the store mints, and the layer points at the current frame. The window wakes when the next frame is due, and a frame already due is applied before that wait so the loop does not spin. A stopped animation, or one whose loops are spent, wakes nothing; falling behind skips frames rather than drifting.
  - The delete letters are the specification's whole table: lowercase
    takes showings and keeps the data, uppercase also frees the data of
    pictures left with nothing showing. `a`/`A` the screen, `c`/`C` and
    `p`/`P` a cell rectangle (the cursor cell, or `x`,`y`), `i`/`I` one
    image — every showing of it, screen or scrollback — `n`/`N` a
    number's newest picture, `o`/`O` one showing (`i` or `I` with
    `p`), `q`/`Q` a picture's showings on screen, `r`/`R` a range of
    picture ids (`x`..`y`, and their showings go with them), `z`/`Z`
    one stacking level (with `x`,`y`, a rectangle). `f` frees an animation's frames and keeps the picture. `i`, `I`, `p`, `x`,
    `y`, `z` are read from their own keys wherever they sit in the key
    list, `x`/`y` 1-based.
  - File transmission (`t=f`/`t=t`) and shared memory (`t=s`): the
      payload is the base64-encoded path (or, for `t=s`, the shared
      memory object's name), decoded once, padded or not; `o=z` applies
      to the payload's data, not to its name. `S` reads that many bytes
      from offset `O`; with `S` absent the whole file or object is read,
      and a short read is refused only when the format needs more bytes
      than were read. A file's path is canonicalised before it is opened.
      Anything under /proc or /sys, anything under /dev except /dev/shm,
      and anything that is not a regular file — named pipes, devices,
      sockets — is refused before the open. The canonical path is then
      opened with `O_NONBLOCK | O_NOCTTY | O_CLOEXEC | O_NOFOLLOW`. `fstat`
      is checked on the open handle, and the path is judged again through
      `/proc/self/fd`, so `/dev/shm` still reads when it resolves
      elsewhere. A client that removes the name after the answer cannot
      take the bytes away. The detection probe writes the file before its
      query is flushed and sets `S` to the path length; `S` larger than
      the file is a short read, not a failure. The read is bounded. `t=t`
      unlinks only when the path still names the inode that was read, and
      only when the canonical path lies inside a canonicalised temporary
      directory (/tmp, /dev/shm, `$TMPDIR`) and the full path carries
      `tty-graphics-protocol`. `t=s` opens the named object
      `shm_open(O_RDONLY)` — a name may omit its leading `/` — checks it with `fstat` under the same caps
      (a name with a `/` past its first character is refused), reads it,
      closes it and `shm_unlink`s it — after a failed read too. Every
      refusal says which check refused, so live failures can be told
      apart.
    - Chunking belongs to direct transmission (`t=d`, the default).
    For `t=f`, `t=t` and `t=s` each piece is complete and `m` is
    ignored. The first chunk of a direct transmission is the piece with
    `m=1` while none is open, and its keys govern the whole
    transmission. While one is open, a piece whose action is absent,
    `t` or `T` is a continuation whatever other keys it repeats
    (`kitten icat`'s last chunk repeats `a=T`), and a continuation may
    carry `q`. A continuation with `m=0` — or with no `m` at all —
    completes the transmission; the default action `t` applies only
    to a piece outside one. A delete aborts an open transmission;
    only one is open at a time, and a new first chunk drops the old
    one. The bytes held for an open transmission are capped at the
    picture budget, past which it fails with a reply under the reply
    rules.
  - Replies: only when `i` or `I` was given, errors included; `q=1`
    hides OKs, `q=2` hides everything, and the first chunk's `q`
    governs the transmission.
  - `U=1` creates a virtual placement: the picture, an optional
    placement id, and a `c` by `r` box. It draws nothing and moves no
    cursor. Cells of `U+10EEEE` then show it, and those cells are scanned
    only when a virtual placement exists. The foreground colour is
    the image id (a palette index or 24-bit RGB) and a third diacritic
    carries the id's high byte; the underline colour is the placement
    id. The first two diacritics are the row and column in the published
    table; a missing one is inferred from the cell to the left. The
    picture is fitted to the box, keeping its aspect, and each
    placeholder cell shows its part. The glyph is never drawn, and
    copying the cell yields nothing. The cells scroll, reflow and clear
    like text.
  - `P` and `Q` place a showing relative to another (`H` and `V` are
    the cell offset, positive right and down). The child moves and is
    deleted with its parent. A chain is followed, up to eight deep
    (`ETOODEEP` past that, `ECYCLE` for a cycle, `ENOPARENT` for a
    missing parent). A frame follows each parent by id. A relative is dropped only when a placement was actually removed. A virtual placement cannot itself be relative
    (`EINVAL`); one can be a parent, and then its place is the minimum
    of the placeholder cells that refer to it. A relative placement
    does not move the cursor.
  - Pictures are keyed by dialect and id, so a client id cannot replace
    another dialect's picture, nor a minted sixel or OSC 1337 id. Each
    dialect's window ids are minted in their own range.
- `sixel.rs`, DCS sixel: each data byte `?`..`~` carries six vertical
  bits (byte − `?`, bit 0 topmost) and only set bits are painted; the
  image grows as the data reaches new sizes and stays within 8192 a
  side. Raster attributes, RGB and HLS colour registers, repeats,
  transparent background (P2=1), carriage return and next line; the
  pixel aspect follows P1 by DEC's table, which the raster attribute's
  pan/pad replaces; the cursor moves below the image. DA1 answers
  `?62;4;22c`.
[

The engine merge supplies what used to wait for it: the exact scrolled
count (an engine-side sentinel counts every line that leaves the top,
past a full history too — and it is read, its travel folded into the
count, before every clear, screen switch or reset that would drop it,
then replanted), the reply order across text and media (one
reply stream in stream order, a probe answered by a dialect before a
DA1 in the stream comes back in that order, and reversed when the
stream is), and the clear and cover operations driven by the engine's
events. The primary and alternate screens keep their scrolled counts apart
already; a count given while the alternate screen is up never prunes
or moves the primary screen's placements, and an overlay affects only
its own screen.

Shared behaviour, owned by the intake core rather than any one adapter:
pictures stay attached to the text they were placed with as it scrolls,
including into the scrollback view, and go when that text scrolls past
the scrollback or the screen is cleared. Alternate-screen pictures go
when the program leaves it. Memory is bounded: a per-window picture
budget (256 MiB of RGBA), with the oldest evicted first.

## 2. Expansion: glass's own dialect

An APC namespace that only glass acts on and other terminals ignore
safely. It is versioned, discoverable by query, keyed by caller-chosen
ids, and chunked for large payloads. It reaches what the window can do.

The reference for program authors is `docs/GLASS_DIALECT.md`: framing,
discovery, every op and key with its type, range and default, the
replies, the lifecycle, the limits and the id ranges, with printf
examples that work in bash.

Version 2 holds version 1's discovery, pictures, layers, groups,
remove and picture showcase, and adds:

- `scene` — a 3D scene as a versioned JSON envelope, placed like a
  layer and drawn by the window;
- a sized video showcase, with `frame` (a picture upload or raw RGBA,
  chunked) and `progress`;
- `chart` — a picture uploaded with `picture`, and marks over it, for the showcase;
- `model` — scene nodes for the showcase, through the scene wire;
- `page` — text the window rasterises as a page;
- showcase events, only when the program opted in with `events=1`.

Version 3 adds:

- `asset` — a glTF 2.0 model (`.glb`, or `.gltf` with its buffers as
  `data:` URIs), chunked like `picture` or read from a path under the
  APC file medium's rules, and the `gltf` mesh kind that hangs its
  node tree from a scene node. Its meshes go up once through the
  window's mesh budget, not the 4096 vertices of an inline mesh; its
  colour textures go in as pictures. The `gltf` crate gives the JSON
  its types; the container, the accessors and every index are read
  and checked by glass itself (`src/intake/glass_gltf.rs`), so a
  truncated, lying or hostile file is an error reply, never a panic
  or an allocation past a cap.
- `play` — an asset's clips (translation, rotation and scale; linear,
  step and cubic-spline keys) played by glass every display frame,
  with play, pause, seek, loop, speed and stop.
- `tween` — a layer's, a scene node's or a scene camera's properties
  eased by glass every display frame from where they stand to where
  the program says, with the usual easings, `cubic-bezier` and
  `spring`, delay, repeat, yoyo and `after=` chains
  (`src/intake/glass_tween.rs`).

Tweens and clips run on the store's animation clock, the same one the
APC graphics dialect's animations use. The window wakes one display
frame at a time (the monitor's own refresh, `Host::frame_interval`)
only while something moves, at the exact end of a delay, and not at
all when everything is still.

Since then, with no new version (new fields a node takes, which an
older glass refuses by name):

- **Skins.** A glTF skin's joints and inverse bind matrices and each
  vertex's `JOINTS_0` and `WEIGHTS_0` are read, and the mesh shader
  bends the mesh by a palette of its joints' matrices, worked out from
  where the joints stand every frame the scene is drawn: a clip that
  moves them bends it, and at rest it stands in its bind pose. At most
  256 joints a skin and 64 skins an asset. A scene with a look (never
  one of this dialect's) draws a skinned mesh as it lies.
- **Chrome and liquid** (`chrome`, `liquid` on a node, its own mesh or
  every mesh of its asset): a mirror of a studio made in the shader,
  tinted by the node's colour, with a Fresnel rim, and ripples and a
  wobble moving with the window's clock; a scene with anything liquid
  in it is redrawn every display frame by the window itself.
- **The rounded cube** (`roundcube`), a picture on each of a cube's or
  rounded cube's faces (`faces`), and a rounded cube's edges in a
  material of their own (`edge`) that can glow, with a soft halo drawn
  round them in the scene's own pass.

`cargo run --release --example glass_chrome -- --dir <models>` draws all
three at once: skinned glTF figures dancing and walking in liquid
chrome, sixty shapes round them, and a rounded cube with a picture on
each face and glowing edges, the scene sent again every display frame
(`--cube` for the cube close up, `--solo man.glb --clip 1` for one
figure).

Not yet: morph targets, the physically based light the kept metallic,
roughness and texture maps are for (the lit path has one light; the
only reflection is chrome's studio), an alpha test for `MASK`
materials (drawn blended), more than four joints a vertex
(`JOINTS_1`), and textures named by an external URI.

It ships with the reference above and a demo program
(`cargo run --example glass_demo`) that draws a short showcase, a
scene, a chart and a few video frames, and cleans up after itself.

A sampler (`cargo run --example glass_sampler`, source in
`crates/term/examples/glass_sampler/`) walks the dialects in order:
text with a shimmering truecolour band, a synthwave sunset in DCS
sixel (252 registers, ordered dither), APC graphics, Unicode
placeholders, a ray-traced still through OSC 1337, animation, and then
glass's own dialect, which is most of the show (about 90 of its 118
seconds):

- Seven lit solids orbit a paragraph. Each is a `k=prim` with an
  additive glow under it, and the far half of the orbit passes under
  the text.
- A cloud of 720 additive lights, one `group` a frame, morphs through
  a sphere, a torus, a trefoil, a double helix and a galaxy.
- A tesseract turns through the fourth dimension.
- Glass tubing with a neon filament stands over text that shows
  through it.
- An equaliser city of 144 columns rises through a sheet of tinted
  glass.
- An orrery sits under the text, its planets lit from their sun and
  its orbits drawn as comet tails.
- A glTF model made in code (`model.rs`, written with `glb.rs`): a
  brass lantern on a textured plinth, its arms turning by linear keys,
  its gems bobbing by cubic-spline keys, its ring ticking by step keys
  and its glass dome breathing. One `asset` in pieces, one `scene`,
  one `play` and four chained camera `tween`s, then the program is
  silent while glass animates the model and flies the camera round it.
- Twelve easings side by side, a dot each going back and forth for
  ever, beside a scene whose cube turns (slerp, elastic), gem bounces,
  sphere changes colour, cone springs and camera sways, all tweened.
- A chart, then a tunnel played as video.
- A finale stacks the orrery, the knot, the solids, glows and dust.

The 3D scenes are inline meshes shaded on the CPU (`motion.rs`): a
key, a fill, a specular and a fresnel rim baked into the vertex
colours. An opaque node writes depth, and the translucent node's
triangles are sorted far to near. Each scene is rendered at twice the
pixels it covers, capped at 2048 a side, so the linear filter that
draws it smaller smooths every edge. The scenes are sent at 60 frames
a second, paced against the clock.

`--scene a,b` plays only the named scenes (`--list` names them;
`--scene gltf,tweens` shows the model and the tweens), and
`--loop` repeats the show until `q`. It sizes each picture from the window — columns
from `stty size` or XTWINOPS 18, one cell's pixels from XTWINOPS 16,
falling back to 10×20 — so the main picture fills about half the width
and never passes the screen. One thread owns stdin and forwards every
byte. The probe takes DA1 and XTWINOPS 18 and 16 from that channel,
with a deadline, and draws nothing if glass does not answer. `q` or
Ctrl+C leaves; a late reply or a focus report does not. The chart is
a picture uploaded with `picture` and opened with `chart;p=`, and it
stays up for at least two seconds. A showcase closes only when one is
open, and only when that scene ends. Animation transmit-and-places
frame 1 (`a=T` with cell columns and rows), then adds the rest with
`a=f`; `a=a`'s `c` and `r` are frame numbers, not a placement. An
in-cell picture (sixel, OSC 1337) is not written over: the caption
sits below its cell rectangle, and the cursor moves with CUP only.
`--check` sends `r=` on every glass op and drops `q=2` from APC
graphics ops, then prints ops sent, OK, each error code, and the
frame rate each animated scene kept, to stderr. Each scene is written before its hold, and the
screen is not cleared during a hold. It uses the alternate screen, so
the main screen's scrollback is left alone, and it removes every
picture, layer and showcase it made. Underline colour (SGR 58) and
every underline style, including curly (SGR 4:3), reach the cells;
the window draws each style as an underline bar in that colour.

## 3. Terminal fidelity

Built, and measured by tests that feed real byte sequences to the
engine and real key events to the encoder.

- **The engine is `alacritty_terminal`'s `Term`** behind `src/engine.rs`,
  one interface for the rest of the crate: process bytes, resize, scroll
  the view, read cells, cursor, styles and modes, take replies, titles,
  bells and clipboard requests. Resizes reflow, hyperlinks and cursor
  style sit on cells, the keyboard, mouse, focus and paste modes are
  tracked, and terminal queries are answered through the event listener.
  The reader thread stays one path: read, then `pump` — the splitter's
  pieces through the engine and the intake in stream order, so text
  replies and a dialect's answers leave in the order the program wrote
  them. While the store holds nothing text could move, cover or drop,
  the text pieces of a read up to the next media piece are one feed (a
  screen switch ends one, a reset is fed alone); the engine answers the
  queries in a feed in stream order, after the bytes ahead of each, and
  its watch follows a screen switch inside a feed. A media piece settles the engine first: an open synchronized
  update is applied through the same counting path `process` uses (in
  paced slices, so a large buffered frame cannot scroll the sentinel
  through a full history before it is counted), and everything the text
  path does after the engine — the screen switch, the counts, the
  engine's events on the store — happens before the piece is placed, so
  a program that clears the screen and draws a new picture inside one
  synchronized update keeps its new picture. The frame stays held until
  the program's end sequence or glass's own limit. The intake reaches the engine only through the
  adapter: the grid cursor read live (hidden cursors still anchor
  pictures), the screen size, the exact lines scrolled off the top (a
  count that only grows, by what each scroll took into history; a
  cleared history does not lower it; the primary and alternate screens
  keep their counts apart), whether the alternate screen is up, and
  feeding bytes back in. Engine events drive the store: `ED 2` on the
  main screen pushes the screen's lines into history first, so
  Ctrl+L's scrollback keeps its pictures on their text; `ED 3` and
  `RIS` clear as asked; a cell write under an in-cell picture (sixel,
  OSC 1337) takes it away whole, while an APC-graphics placement never
  comes away for text. Every forwarded `Handler` method is proven
  forwarded against a bare `Term`.
- **Everything the terminal did still works**, byte for byte in the
  tests: startup replies (DA1/DA2, DSR, XTVERSION answering `glass`,
  XTWINOPS 14/16/18, OSC 10/11/12, DECRQM 2026), focus reports,
  synchronized output holding the frame while an update is open and
  ending it at its deadline (the loop's tick ends a stalled one and
  marks the frame dirty), OSC 52 copy, titles, the bell requesting
  attention when unfocused, selection (word and line too), copy and
  paste (bracketed or not, wide characters copied once), scrollback by
  wheel and Shift+PageUp/Down (the window's own shortcuts act on press
  and repeat; their releases are swallowed), the wheel as arrow keys on
  the alternate screen, mouse reporting in every mode and
  encoding, and the environment scrub with TERM, COLORTERM and
  TERM_PROGRAM set in `Session::spawn`.
  A scanner in front of `Term` answers the queries that need what only
  the window knows (what this terminal is, one cell's pixels, whether a
  synchronized update holds the frame), so the program sees one reply.
- **Cursor shapes and colour.** DECSCUSR 0 to 6 carry through: block
  and hollow block as a block, underline as an underline, beam as a
  bar, hidden hidden, and OSC 12 cursor colour. The shape and colour
  ride glass's Snapshot additively (`CursorStyle`, `CursorKind`): the
  cockpit's own look does not change unless it asks for something else.
  glass's default, which DECSCUSR 0 restores and which a program that
  never asks gets, is a steady block. DECSCUSR 1, 3 and 5 blink (block,
  underline, bar); 2, 4 and 6 are steady. Mode `?12` sets the blink
  on its own, and resetting it holds the cursor still. The window draws
  a blinking cursor about 530 ms on and 530 ms off. Keys and output
  hold it steady for half a second, then the blink resumes on the on
  phase. While the window is unfocused the cursor is steady and the
  blink does not run; glass tracks focus from the window's focus
  events, starting focused. The timed wait wakes for the next blink
  edge only while the cursor is shown and blinking, and a frame is
  drawn only when that edge changes what is on screen.
- **Hyperlinks.** OSC 8 links and plain URLs (http, https, file) open
  with `xdg-open` on Ctrl+click; only http, https, file and mailto
  open, and the helper is reaped on its own thread. Ctrl+hover
  underlines the extent the link covers and redraws when that changes.
  A plain click still selects.
- **The CSI u progressive keyboard dialect.** Push, pop, set and query
  flags (`CSI > u`, `CSI < u`, `CSI = u`, `CSI ? u`, answered
  truthfully), disambiguation, event types (press, repeat, release),
  alternate keys, all keys as escape codes, and associated text — each
  only as the program's flags request, encoded to the dialect's own
  table: arrows, Home, End and F1/F2/F4 as `CSI 1;mods letter`,
  Insert/Delete/PageUp/PageDown/F3/F5-F12 as `CSI n;mods ~`, the lock
  and pad keys and F13-F35 from the functional table as `CSI n;mods u`.
  The key code is the unshifted codepoint; the shifted key rides along
  only under alternate keys with Shift held. Enter, Tab and Backspace
  stay their legacy bytes under disambiguation alone and only become
  codes under all keys. A key that goes out as text or as a legacy byte
  has no release or repeat. With no flags pushed, keys stay the legacy
  bytes exactly as before, Shift+Enter as ESC CR included. Releases are
  sent only when event types were asked for.

- **Throughput.** A run of text the parser would print a character at a
  time is written into the grid without it: printable ASCII a line at a
  time, UTF-8 decoded once by a strict decoder, each character exactly as
  `Term::input` would take it. It is done only where the parser's state
  is known to be ground with no UTF-8 character half read and no
  synchronized update buffering; that state is followed beside vte from
  its own transition table, from the last ESC before a run (every state
  goes to escape on ESC). The run's last character still goes through the
  parser, for REP. A read reaches `Term` straight from the read buffer
  (only the queries glass answers itself are cut out of it), and the end
  of a run of text is found eight bytes at a time. The pty is read on a
  thread of its own, ahead of the parsing, at most two 16 KiB hand-offs
  ahead. Tests hold the fast paths
  to the parser alone (every cell of history and screen, the cursor, the
  pen, the modes, the counts and the replies) over random streams cut at
  random reads, and feeds of many pieces to feeds of one. The headless
  bench is `cargo test --release -p angel-glass-term speed -- --ignored
  --nocapture` (`GLASS_BENCH_PTY=1` reads through a real pty, which
  reproduces the windowed numbers).
- **The read path on the largest cache.** A flood scrolls lines into the
  ten-thousand-line history, about 37 MB of cells in a full-screen
  window, and each line scrolled rewrites the oldest. Where the history
  fits in the last-level cache the parser runs at the speed of its own
  work (about 570 MB/s of plain text, headless, on the operator's Ryzen 9
  9950X3D); where it does not, every line waits on memory (about 310
  MB/s on the same chip's other die, whose L3 is 32 MB). That traffic is
  the grid's own: `Term` clears the recycled row cell by cell when it
  scrolls, and the text is then written over it. glass prefetches the
  row a line ahead (further ahead, or into another cache level, measured
  no better). On a machine
  whose CPUs share more than one L3 and the L3s differ in size (AMD's
  X3D parts), the two threads of the read path, `pty-read` and
  `pty-parse`, are placed on the CPUs of the largest
  (`src/affinity.rs`, from `/sys/devices/system/cpu/cpu*/cache/index*`
  with level 3: its `size` and `shared_cpu_list`), among the CPUs glass
  was allowed already. No other thread is placed. Nothing is done with
  one L3 or several of one size (most laptops and desktops), on other
  systems, when sysfs cannot be read, or when glass was started on none
  of those CPUs. `GLASS_AFFINITY=off` leaves the threads to the
  scheduler; `auto` is the default.
- **Startup.** The slow parts of opening the window need no window, so
  each starts on a thread of its own the moment `run` begins: the GPU
  (the driver's instance, the adapter and the device, most of startup),
  the fonts (the four styles looked up through fontconfig at once) and
  the background image's decoding. The window opens meanwhile. The
  adapter is picked before there is a surface; one that cannot draw to
  the window gives way to the one the surface picks, as before.
- **The early frame.** What is left is the driver: on the operator's
  machine a bare wgpu program spends about 39 ms in the Vulkan instance
  (32 of them the NVIDIA driver's, the rest the loader and the
  integrated GPU's driver) and 60 ms in the device, and up to twice that
  while other work holds the GPU. A Wayland window is not shown until
  something is drawn into it, so once the window exists (about 15 ms in)
  glass draws its background on the CPU into a shared-memory buffer and
  commits it (`crates/glass/src/early.rs`). That background is the
  background colour at the window's opacity, then the background image
  where its layout puts it, then the tint, as the GPU will draw them. The
  window maps then, and it is drawn again at each size the compositor
  gives it until the GPU is open. The renderer is made when the GPU's
  thread says it is ready. Its first frame replaces the buffer, which is
  let go three frames later.
- **The program starts before the GPU is open.** A tiling compositor
  settles a window's size only when it maps it: Hyprland guesses half
  the screen for a window opened on another workspace, then sends the
  tiled size a few milliseconds after the map. So the program starts on
  the first size after the early frame mapped the window, or 50 ms after
  the map if none comes (a floating window keeps the size it asked for).
  Its grid is measured from the fonts, which are ready by then, as the
  renderer will measure it. If the renderer's grid differs, the program
  is told, as on any resize. Its output waits in the cells until the
  first GPU frame shows it. Without an early frame (X11, other systems,
  or a step failing), the program starts after the GPU's first frame,
  as before.
- **Measured.** Medians of 16 runs on the operator's machine, launched
  through Hyprland on a headless output, against kitty 0.49.2:
  - window on screen: 31 ms (kitty 73);
  - program running: 39.5 ms (kitty 193).
- **Pipelines.** Shader pipelines cost about 2 ms in all, as the
  driver's own shader cache already holds them, so glass keeps no
  pipeline cache of its own. `GLASS_STARTUP_TRACE=1` prints each step on
  stderr: milliseconds since `main`, the wall clock in nanoseconds (to
  line up with a launcher's clock), the thread and the step.

## Filming the window

`ANGEL_GLASS_RECORD=film.mp4` films the window as it is drawn, through
ffmpeg and the GPU's encoder (`crates/glass/src/record.rs`). A clock asks
for a frame on every tick; the window draws it into a texture of its own
and goes on while the GPU copies it back, so filming costs the window a
draw, not a wait. A tick the window could not answer in time repeats the
frame before it, so the film keeps the wall clock's time and a stall
shows as one; the counts are printed when the film closes.

- `ANGEL_GLASS_RECORD_FPS` (60), `ANGEL_GLASS_RECORD_SECONDS` (30),
  `ANGEL_GLASS_RECORD_DELAY_MS` (2000).
- `ANGEL_GLASS_RECORD_CODEC` (`hevc_nvenc`): NVENC takes the RGBA as it
  is and converts it on the GPU; another encoder (`libx264`) converts on
  the CPU.

`examples/glass_mindseye/` is the show it was made for: sacred
geometry streamed as ring, line, glow and lit-solid layers, up to about
eleven thousand a frame, each frame answered by a DA1 so the numbers in
its corner are measured.

`examples/glass_showoff.rs` is the 3D scene in the 0.2.7 film: three
skinned glTF dancers, a video cube, two cards and a chart, drawn by
glass's own `scene`, `picture` and `play` over real source scrolling
behind them, for 31 seconds. It needs `ffmpeg` and a folder holding the
three models from three.js r170, two films (`film-a.mp4`, at least 20 s,
on the cube; `film-b.mp4`, at least 6 s, looped on a card) and a
`cover.png`:

```sh
mkdir -p showoff && cd showoff
for m in Michelle Xbot Soldier; do
  curl -fsSLO https://raw.githubusercontent.com/mrdoob/three.js/r170/examples/models/gltf/$m.glb
done
sha256sum -c <<'SUMS'
7a87e15a99ccbc5e5877be66e1e4ecae0a581adcafa0cce1a5569f49909e968e  Michelle.glb
002f8d269de68e5dce3d25195caf390d1aa359bbfaae3fcf4c8dc78ec36c3ba5  Xbot.glb
dfb230fc1f942f259dd00281a1186953ad602fc5d69067ce63e24b2aa439736b  Soldier.glb
SUMS
cd ..   # then add film-a.mp4, film-b.mp4 and cover.png, and in glass:
cargo run --release --manifest-path crates/Cargo.toml -p angel-glass-term --example glass_showoff -- --dir showoff
```

The models are three.js's examples (MIT repository; the characters are
Mixamo's). With `ANGEL_GLASS_RECORD` set, glass films it as it plays.

## Rules for the work

- Limits that remain, all on the picture side and each deliberate:
  none from the scroll, reflow or cover work. A picture anchored to a
  line moves with that line when a scroll region scrolls, including the
  alternate screen, whose scrolled count is the lines that left its top.
  A picture that leaves a region is cut at the margin through its source
  rectangle. A resize that changes the width reflows anchored pictures
  onto the same text, at the same character offset, including the primary
  screen's scrollback when the alternate screen is up. Text or an erase
  over part of an in-cell picture hides only those cells; the rest still
  shows. An erase below or above the cursor covers the same cells the
  text loses. A cover is recorded on the picture's absolute line, the
  same line the picture is anchored to, including the lines that have
  left the alternate screen's top. Text that does not overlap the
  picture's cells does not hide it.
  Glass's own text-anchored layers follow the same moves and the same
  reflow. Cover rules apply only to in-cell pictures.
- Comments and docs speak of dialects and intake. No dialect is named as
  the standard the others are measured against.
- `cargo test -p angel-glass-term` and `cargo clippy -p angel-glass-term`
  stay clean. Every adapter has tests that feed real sequences and check
  what reaches glass.
- Builds stay in this worktree's `crates/target` (on /work).
