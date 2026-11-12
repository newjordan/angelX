# Remote glass

Our own tmux, where the client is the glass window. A cockpit session runs on
one machine (its files, models, tools and GPU work) and a glass window on the
machine you sit at draws every pixel of it. The session outlives the window:
close it, attach again later from anywhere.

It continues [RENDER_FOUNDATION.md](RENDER_FOUNDATION.md) and
[GLASS_CORE.md](GLASS_CORE.md): one renderer, and no picture of the screen
crosses the network. The window draws the session from its state with the same
glass renderer a local cockpit has.

## Commands

| | |
|---|---|
| `angelX <host>` | Attach to the session `main` on `<host>`, starting it if there is none. |
| `angelX <host>:work` | The session `work` on `<host>`. |
| `angelX --attach <host>[:name]` | The same; `--term` attaches in this terminal, `--glass` in a window. |
| `angelX --attach :name` | A session on this machine (no SSH). |
| `angelX --sessions [host]` | List the sessions on this machine or on `host`. |
| `angelX --kill-session host:name` | End a session. |
| Ctrl+\\ | Detach (as `dtach` has it). Closing the window detaches too. |

A word that is not a directory here and reads as a host (`name`, `user@host`,
`host:session`) attaches; a directory is still the workspace, as before.

With a display, outside tmux and SSH, the attach opens a window (the
launcher's own rule for a local launch); otherwise it attaches in the
terminal. A plain local launch and the terminal over SSH are unchanged.

The far side's workspace is where its launcher starts over SSH (the home
folder). `ANGEL_REMOTE_ANGELX` names the angelX to run there (a test
checkout) and `ANGEL_REMOTE_ENV` (`NAME=value` words) sets its environment.

## The session model

- **A session is a process.** `ssh <host> angelX --session-host <name>` (the
  attach runs it) connects to `~/.angelX/remote-glass/<name>.sock`. When
  nothing answers there, it starts the session: the same binary in a session
  of its own (`setsid`, no terminal, stdin and stdout closed, stderr to
  `<name>.err`, shown in `ps` as `angelX-session:<name>`), under a lock so two
  windows attaching at once start one. Then it relays bytes between its stdin
  and stdout and the socket until either side closes. The session is the
  cockpit under a glass host with no window (`angel_glass::remote::headless`).
- **The far side's angelX** is found as `angelX` on its PATH, then the
  fleet's `~/.local/bin/angelX`, then `~/angelX/bin/angelX`. Its launcher
  builds and loads its env files as for any launch, and routes
  `--session-host` to a run whose stdout is the link (`bin/angelX`,
  `SESSION_MODE`).
- **Transport.** `ssh -T` (no terminal on the far side: the link is binary)
  with keepalives every 5 s, so SSH's keys and the tailnet are the only
  doors. No new port, no new key.
- **Several windows** may attach to one session at once. The session is
  sized by the window used last (tmux's `window-size latest`): attaching, a
  key, a click or a resize make a window the one that sizes it. The others
  draw the same grid from the top left, clipped or with space to spare.
- **Detach** leaves the session running. **Quit** in the cockpit ends it and
  every window says so. `/self reborn` replaces the session's process; the
  windows see their link drop and attach again.
- **Fails soft.** An angelX on the far side too old to know `--session-host`
  is named in one line ("the angelX there is too old for remote glass;
  update it there") and the attach stops. A version mismatch is said the
  same way. A dropped link puts "reconnecting to host:name…" over the last
  picture of the session and attaches again (backing off to 2 s), keeping
  the blobs it already holds; the session never notices.

## The protocol

A byte stream both ways. Before its first frame the session's side writes
`\0ANGEL-REMOTE-GLASS\n`, so the window skips whatever a login shell or a
launcher (a build's progress) printed first and keeps its last line to
explain a failure. Then every message is a frame:

```text
length u32 little-endian (the bytes after it), flags u8 (1: deflated),
then the message as bincode (varint integers)
```

Frames over 2 KiB are deflated at the fastest level when that saves a
quarter. The messages are `angel_glass::remote::wire::{Up, Down}`; the
version is `wire::VERSION` (2). A version 2 host also accepts version 1
viewers and echoes their selected version; clip updates are omitted from their
initial catch-up and live frames. Original version 1 message tags remain stable.
A version 2 viewer connecting to an old version 1 host still receives the old
host's update-required response; downgrade requires reconnecting the transport.

Version 2 adds `ShowcaseSquire`: a versioned authored clip replacement or explicit
removal. The latest validated clip is retained for late viewers. Unchanged clips
are omitted from ordinary grid/video frames. Clip frames have a 65 KiB envelope,
selected from the first bincode tag before body allocation. Compressed frames
are inflated incrementally until that tag is available, then use the same limit;
clip strings also check declared lengths before allocation. Other message kinds,
including video blobs and picture deltas, retain their 512 MiB frame allowance.

For Rust callers, `wire::read<T>` now requires `T: FrameMessage` in addition to
owned deserialization. Direct `Up` and `Down` calls keep their signatures and
wire encoding. A generic wrapper must propagate the new bound; a custom type
must supply an explicit first-byte limit policy. This is a source-level API
change for wrappers/custom types, rather than an automatic fallback to the
large video allowance. The framing reader targets tagged remote messages;
`wire::decode<T>` remains available for generic already-bounded byte slices.
Raw frames reserve their validated body size once; compressed frames grow only
as their bounded output arrives.

Window to session (`Up`):

| Message | What |
|---|---|
| `Hello { version, kind, size, have, who }` | `kind` is `Window`, `Terminal`, `Probe` (`--sessions`) or `Watch` (the presence feed). `have` lists the blob hashes the window still holds. |
| `Input(Event)` | The window's keys (press, repeat and release, the Kitty keyboard detail the cockpit uses), mouse, wheel, paste, focus: crossterm's events, as the cockpit reads them. |
| `Resize(GridSize)` | Cells and cell pixels. |
| `Showcase(Feedback)` | The showcase panel's clicks, copies, steps, seek, and its closing. |
| `Kill` | End the session (a probe's). |

Session to window (`Down`):

| Message | What |
|---|---|
| `Grid { cols, rows, full, runs, cursor, card }` | Runs of cells that changed since the last frame (changes within four cells join a run). A cell is its symbol, three colours packed in one number each, and its modifier bits. The card (the setup menu, an approval) travels with the grid it was published with. |
| `Blob { hash, deflated, bytes }` | Content-addressed bytes, sent once per window, before the first message that names them. |
| `Picture { id, size, cells, hash, keep }` | A picture from a blob. A picture id handed over again within 1.5 s is a stream (a video, an animation): `keep: false`, its blob is let go once it is up and never counted as held. |
| `Mesh { id, hash }` | A mesh for scenes and the showcase's model. |
| `Layers` | The window's layer list. |
| `Stage { id, stage, meshes }` | A stage (the Delve's frame in parts, the mini-viz). Its world's meshes leave it as blobs, made once per mesh; the set's text and plate stay home (only a far side that remakes meshes reads them). |
| `Scene { id, scene }` | A 3D scene. |
| `ShowcaseOpen`, `ShowcaseClose`, `ShowcaseProgress` | The showcase panel. |
| `Title`, `Attention`, `Clipboard`, `GameKeyboard`, `Size` | The window's other services. Clipboard text lands on the clipboard of the machine you sit at. |
| `Present` | The end of one of the session's frames: the window presents once per frame, with its pictures, as a local window does. |
| `Ended(why)` | The session is over. |
| `Presence` | Who is at work (below). |

## What crosses the wire

State, never a picture of the screen.

- **The grid** as diffs. Typing one character is a frame of a few dozen bytes.
- **Layers, stages and scenes** as their descriptions (`angel-stage` and
  `angel-gpu` types, serde behind their `serde` feature).
- **Pictures, meshes and world meshes** as content-addressed blobs (128-bit
  hash), each sent once per window and kept by the window across links. A
  newly attached window gets the latest of everything: grid, pictures,
  meshes, layers, stages, scenes, the showcase.
- **Pictures painted on the CPU every frame** go as deltas of the last
  ones: the realm's map in the mini-viz pane and video. Measured on
  the arm64 DGX Spark host with `ANGEL_REMOTE_STATS=1`: the cockpit at rest with the realm
  on screen sends ~13 KiB/s of grid and ~60 KiB/s of realm deltas (165
  KiB/s as whole pictures); a 640x360 video in the Stereopticon was ~1.1
  MiB/s as whole frames, before the deltas.
  The realm becomes a description when the map is drawn as a stage, as the
  Delve's rooms are. For video, shipping the file once and decoding it in
  the window against the session's clock is the next step.

The session's side is a `Host` like a window's: the publisher thread takes
what the window's render thread would take (the snapshot slot, pictures,
meshes, stages, scenes, the showcase request, the layer list) and sends it
to every window. A layers-only change waits up to 4 ms for the frame that
follows it, as the window's own rule shows layers with their frame.

`ANGEL_REMOTE_STATS=1` on the session prints the bytes it sends by kind
every five seconds.

## The Delve

The Delve in a remote session is drawn by the window from its stage, as a
local window draws it: the session's cockpit plays its seat and makes the
stage each game frame (`arena::stage_frame`), and the stage crosses as its
description, its world's meshes as blobs sent once, its pictures as
deltas of the last ones. The window's renderer draws the lit rooms, the
cards, the light, the motes and the post pass itself, at its own density,
so they look as they do in a local window. No finished frame is sent.

What is still painted on the CPU in the session, and so crosses as
picture deltas, is what the stage itself carries as pictures: the
figures' sheet, what lies over them, the shots' sheet and the screen's
marks. Measured with `ANGEL_REMOTE_STATS=1` on
the DGX Spark host, the session at 60 frames a second the whole time: the kingdom
~64 KiB/s, a room (the Delve's Gate, the Undercroft) ~27 KiB/s, of which
the stage's own description is ~19 KiB/s. Before the deltas the kingdom
was ~800 KiB/s of whole pictures.

**The seat path, next.** The fuller form is the window joining the
session's Delve as a seat does (`together_seat`: a `NativeLink` speaking
the seat wire, version 2, through the Publisher and `seat_tree`), over a
channel of this same SSH link, and making the stage on this side. Then
only the game's state crosses and the window predicts its own knight.
It needs one thing from the Delve: a way for a cockpit to hand its own
seat to a window (the window takes the seat over; the cockpit keeps
reading the run for the words around the room) without a second knight
joining. That is the Delve lane's to shape; remote glass carries the
bytes.

## In a terminal, and several at once

`angelX --attach host --term`, or any attach with no display, attaches in
the terminal it was typed in: the session's grid is drawn there through
ratatui (only changed cells go to the terminal), pictures as half blocks
sampled from their pixels where the window would draw them, the keys and
mouse sent back as a window's are, Ctrl+\\ detaches. Stages and 3D scenes
need a window; their cells stay dark in a terminal.

A window and a terminal can be attached to one session together; the one
used last sizes it.

## Presence

A read-only feed beside attach, for a kingdom that draws the knights of
every session it can reach. A link whose hello is `Kind::Watch` gets
`Down::Presence` when it connects and whenever it changes, and nothing
else: no frames, and it does not count as an attached window or size the
session. A `Presence` is the session's name, its workspace's folder name
and its knights: each one's model family (its look), persona, what it is
doing, its status (working, waiting, idle) and its task line. Knights are
models, never machines. The host list a kingdom watches is the user's own
setting; the public build has none.

## Latency

Keypress to compositor frame, measured with the glass test kit's `keyframe`
(30 keys typed by its own virtual keyboard into a private nested Hyprland
seat on the workstation, each timed to the frame that showed it), release builds,
2026-10-20. The workstation is x86_64 (Ryzen 9 9950X3D, RTX 3090); the
remote host is an aarch64 DGX Spark, reached over the tailnet (ping 0.8–2.2 ms, Tailscale SSH).

| | p50 | p95 | worst |
|---|---|---|---|
| Local glass window (the baseline) | 7.0 ms | 11.7 ms | 14.9 ms |
| Remote glass, a session on the same machine (no SSH) | 9.4 ms | 13.7 ms | 17.0 ms |
| Remote glass, workstation → DGX Spark over SSH (three runs) | 10.2–13.5 ms | 13.9–18.2 ms | 22.6–24.9 ms |

The session's side adds about 2 ms (the publisher, the diff, the socket
relay); SSH and the network 1–4 ms more, round trip included. Every
remote key landed inside the 25 ms budget, so typing echoes without local
prediction; the worst keys come near it, and they are the first thing to
look at if a slower link is the norm (OpenSSH sets no TCP_NODELAY on a
session without a terminal). The nested seat's 60 Hz frames make each column bimodal (a key
lands early or late in a frame), as they do locally.

## Tests

- `cargo test -p angel-glass --features remote --lib remote` (in `crates/`):
  frames and messages round-trip, cells and colours, the grid diff, deflate,
  junk before the magic and an old angelX's error, hostile lengths, hashes;
  a headless session's cells reaching a window and a key coming back, a
  picture sent once and a late window holding it already, the last window
  used sizing the session, a probe, a version mismatch, the presence feed;
  the window's page and stream bookkeeping and the detach key.
- `cargo test --features glass --bin angel remote` (in `cockpit/`): targets,
  the far command's quoting, an old angelX named in one line.
