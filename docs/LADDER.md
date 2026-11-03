# The resolution ladder

2026-10-28, the operator: "over time you increase resolution, thats part of the game". A knight starts a run at the game's own grain, 16 px top to toe, and climbs to 32, 64 and 128 px as the run goes on. The game still composes at 1x (the 384 x 224 room, 16 px a tile); gameplay, the wire's snapshots and the terminal are as they were. What climbs is the picture a window draws in a figure's place: a finer drawing of that figure, found by what the figure is, on the finest rung the knight has reached.

Three parts, each its own place in the code:

| part | where | what |
|---|---|---|
| keys | `arena::figure_keys`, `room_scene::Body::named` | a figure stamped from a source the game knows carries its name beside its pixel hash |
| the rung rule | `rules::resolution` | which rung a knight is on, from the run as it stands |
| the pack | `arena::hd_pack` (`ANGEL_GLASS_ART`), `scripts/delve/ladder_pack.py` | named drawings per rung, matched by name first, hash second |

The browser has the same path behind a flag (below, "The browser").

## Keys

Before this, an HD drawing was matched by an FNV hash of the final stamped figure's pixels (`room_scene::look_key`), so any recolour, flip, pose or gear change orphaned it. The hash path still works, unchanged. A figure the game stamps from a known source now carries a name too (`figure_keys::Named`):

- **key** `<kind>/<design>/<clip>/<frame>/<colours>/<facing>`, e.g. `knight/sugarloaf-crusader/slash/3/seat2/l`. Kind is `knight` (foes and props come the same way later). Design is the rig's (`figure_keys::design_of`): a seat's knight by its design's name (`quick_rig::Design::name`: seat 1 and 5 `great-helm`, 2 `sugarloaf-crusader`, 3 and 6 `plumed-bucket`, 4 `kettle-hat`), and `realm` for the old knight (`knight.json`) a design falls back to when its rig does not read. Clip and frame are the rig's sample (`quick_rig::Clip`'s name, the frame's index in its clip). Colours are `seat<N>` for a seat's own (`knights::realm_colours`), `colours-<hash>` for a company's. Facing is `r` as the rig is drawn, `l` turned over.
- **dress** what the figure carries over its body, as stamped: the arm (`bow`, `crossbow`, `handgonne`, `sword`, `axe`, `maul`, `spear`, `cleaver`, `sweep` while a stroke draws it out), the shield and the way it faces (`shield-across`, `shield-toward`, `shield-away`, `shield-up-<view>[-struck]`), a hat (`laurel`, `hat-<game>`); `bare` for nothing. The default knight is `bow+shield-across`.
- **pivot** the feet in the figure's own picture (the rig's pivot as stamped), so a drawing of any size lands on them.
- **rung** the knight's rung (`rules::resolution::rung`).
- **bare** the rig's body alone as stamped, before its shield, arm and hat, in the figure's picture: what a body-alone drawing takes the place of.

Where it is made: `knights::dressed_frame`, on the rig branch, when the rig is the game's own (not a wished one), nothing is worn over it and nothing tints it. `figures.rs` gives it the knight's rung, turns it over with the figure (`Named::mirrored`), and lets it go whenever the frame is bent from the rig's own: possessed, a frog, chilled, crouched, veiled, a statue, a rolled tumble that is not the authored roll clip, tossed, flung off a tower, struck (the flinch pose). It reaches the frame through the capture (`capture::name_next`): the next sprite of that exact look stood up takes the name (`note_body`).

## The rung rule

`rules::resolution` is the one place that says which rung a knight is on. It is worked out from the run, never saved or sent, so every seat holding the same snapshot agrees and an old save climbs as a new one does.

| rule | how | table |
|---|---|---|
| `Floor` (default) | the floor the party has reached (`Run::floor`, the Deep's own count below the Unknown) | floor 1: 16; 2-3: 32; 4-6: 64 (the deep below the dragon, where rooms stand up in 3D); 7+: 128 (the Deep's road and below) |
| `Level` | the knight's level, kept by the realm from run to run (`home.knights`, `talents::LEVELS`) | level 1: 16; 2: 32; 3-4: 64; 5: 128 |
| `Fixed(r)` | one rung for everyone | shots, tests, a look at a rung |

The knob: `ANGEL_DELVE_LADDER=floor|level|16|32|64|128` (read once), or `resolution::set_rule` on a thread (shots, tests). The tables are `FLOOR_RUNGS` and `LEVEL_RUNGS`. `resolution::next(run, id)` says the next rung and the floor or level it begins at, for a rung-up moment or a run's end ("the next rung, at floor 4").

## The pack

A pack is the HD art pack of docs/WORLD-TIERS.md ("HD art packs"), with named entries beside its look entries:

```json
{ "key": "knight/sugarloaf-crusader/idle/0/seat2/r", "rung": 64, "pivot": [24, 63], "file": "knight/sugarloaf-crusader/idle/seat2@64.png" },
{ "key": "knight/sugarloaf-crusader/idle/0/seat2/l", "rung": 64, "pivot": [24, 63], "file": "knight/sugarloaf-crusader/idle/seat2@64.png", "flip": true },
{ "key": "knight/great-helm/walk/3/seat1/r", "rung": 64, "pivot": [18, 59], "covers": "body", "file": "knight/great-helm/walk/3/seat1@64.png" }
```

| field | |
|---|---|
| `key` | the figure's name (above); six parts, lower case |
| `rung` | 32, 64 or 128: the drawing's pixels to the game's are its sixteenth (2, 4, 8) |
| `pivot` | its feet, in its own pixels (the bottom middle unless it says); turned over with it when `flip` |
| `file` | the PNG in the folder (`<name>.png`, then `<key>.png`, unless it says) |
| `flip` | the drawing is the file mirrored |
| `dress` | drawn for this dress only; none: drawn for whatever the figure carries |
| `covers` | `whole` (the default): the figure, drawn over it in its place. `body`: the body alone, drawn under what the figure carries, which keeps the game's inks over it |

A named drawing is any size: it lands by its feet, its feet pixel's middle on the figure's feet pixel's middle, its bottom on theirs (`HdArt::named_picture`), and the same rule turned over lands a mirror on a mirror. A figure is matched by name first: on the finest drawing its rung allows (and the drawer's `max_scale`), a whole figure of its own dress, else a body alone (if it has its bare body), else a whole figure of any dress; then by its look's hash, as before. At rung 16 a figure is never drawn by name. A whole figure's drawing takes all its pixels out of the 1x picture and is drawn over; a body's takes out only the bare body's pixels nothing it carries covers, and is drawn under the figure, so the 1x bow and shield stand on the finer body (`stage_frame::with_art`; on a lit room's cards, the body's card goes before the figure's).

### The knight pack from Sol's stills

`scripts/delve/ladder_pack.py stills <scratch>/knights-v2/source/stills <pack>` makes the knights' pack from the touched stills (`<design>-<16|32|64|128>.png`, realm inks): each design at 32, 64 and 128, in the colours of each seat that wears it (the designs and seats read from `quick_rig::Design`; the colours by the game's own swaps, `realm_colours`, read from `knights.rs` and applied in order as `Img::recolor` does), facing right and left (one file, `flip`), each standing for every frame of the design's idle clip (`knight-<design>.json`), as a whole figure of any dress (a still carries its own arm and shield). Its feet are found between its two feet at the bottom (checked against the 16's feet carried up by its width). `--seats 1-6` makes every design in every seat's colours; `--stand-in realm` keys each seat's still as the old knight too. 18 PNGs, 216 entries, 100 KB on disk (4 KB of manifest gzipped); read in the window as it opens.

Only ink `7` of the seat inks appears in the stills (their gold trims are other golds), so the seat colour moves the cloth and the crusader's cross and leaves the trims.

### From a rig at any rung

`scripts/delve/ladder_pack.py rig <rig.json> <pack> --design NAME --rung 64 [--clips idle,walk] [--seats 1-6]` makes body-alone drawings from a rig JSON drawn at that rung (version 1's shape, `quick_rig.rs`, at its rung's canvas: 46 x 46 at 16 is 184 x 184 at 64; no size limit here): each clip's frames in realm inks, each seat's colours, both facings, cut to their ink with the feet moved with them. Its keys are `knight/<design>/<clip>/<frame>/seat<N>/<r|l>`, so the frame numbers must be the 16 px rig's: frame `k` of the 64 rig's slash is drawn for frame `k` of the game's slash.

`ladder_pack.py designs <pack> [--rungs 32,64,128] [--clips ...]` does it for every design at once, from the rigs beside the game's (`crates/delve/assets/quick-rig/knight-<design>@<rung>.json`), for the seats that wear each. `--placeholder` stands each design's own 16 px rig in where a rung has none, brought up nearest with one pixel of every art pixel lightened: never art, it proves every clip's every frame lands (`delve_ladder --stride`, `--swing`). `ladder_pack.py enlarge <knight-<design>.json> <out.json> --rung 64` writes the 16 px rig brought up to the rung's canvas, its pivot and sockets with it: the canvas to redraw the frames on.

## How to add a rung to any figure

1. Name it where it is stamped: build a `figure_keys::Named` (a key in the six parts, its dress, its feet in its own picture, its rung, its bare body if gear goes over it) and hand it to `capture::name_next(&img, named)` just before the `stand` that stamps it. Let the name go wherever the picture is bent from what the name says.
2. Give its source a design name (`figure_keys::design_of` for a rig; a foe would be `foe/<kind>/<clip>/<frame>/<colours>/<facing>`).
3. Draw it at the rung, keep its feet where the 1x's are, and put it in a pack (`ladder_pack.py rig`, or entries by hand).
4. Shoot it: `delve_ladder <out> --hd <pack>` (below).

### What the knights' lane does to light up its animated frames

The four designs are named already (`figure_keys::design_of` reads `quick_rig::Design`), and every frame of every clip of every seat's knight carries its key. To light them up at the higher rungs:

1. Draw each design's clips at 32, 64 and 128 as rig JSONs at those canvases, `crates/delve/assets/quick-rig/knight-<design>@<rung>.json` (184 x 184 at 64; `ladder_pack.py enlarge` gives the canvas with the 16's frames brought up on it to draw over): the same clips with the same frame counts as the design's 16 px rig (frame `k` stands for frame `k`), the pivot the 16's carried up (`[px * s + s / 2, py * s + s - 1]`, what `enlarge` writes), the body alone without arm or shield (the rig's own contract), the seat cloth in `7`/`8`/`5`/`4`, no outline, lit from the upper left.
2. `ladder_pack.py designs <pack>` into the pack with the stills (`ANGEL_GLASS_ART`). A body drawn for a frame wins over the idle still (it keeps the knight's own arm and shield), so a design's idle at a rung should be drawn too, or its still stays the idle there.
3. Shoot it: `delve_ladder <out> --hd <pack>`, `--stride 1.7` for the walk, `--swing 5` for the slash.
4. The arm and shield stay the game's 1x inks over the finer body until they get names of their own (`gear/<kind>/...`, not built).

## The browser

The page already draws a stage frame at the screen's density (`stage.js` and the renderer module draw each picture into its rect in screen pixels), so a finer picture in a figure's place shows its pixels; what a page lacked was the drawings (it has no folder to read). Now:

- The host serves the pack at `/hd/` (`angel-serve`, `ServeConfig::hd_dir`: `/hd/pack.json` and `/hd/<file>.png`, lower-case paths in the folder only, revalidated by size and age), behind **`ANGEL_DELVE_HD_WEB=1`** on the host (the window's own pack: `ANGEL_GLASS_ART`, else `~/.angelX/art/delve`).
- A page opened with **`?hd=1`** fetches `/hd/pack.json` and hands it to the game with its whole-number scale (`angel_hd_manifest`). The game builds a pack of named drawings whose pixels are to come (`hd_pack::Pack::from_manifest`); a knight on screen whose rung finds one names its file among the pictures it wants (`hd/<file>`, the paintings' channel); the page fetches it from `/hd/<file>`, decodes it with the browser's own decoder and hands the pixels back (`Pack::put_file`). Until it comes, and if it never does, the knight keeps its inks.
- The page never takes a drawing finer than it shows the game's pixels (`HdArt::max_scale`): at 1080p the page is 4x, so the 64 rung is 1:1 and a 128 rung shows the 64 drawing.

## Shots and costs

**To see it.** In the window: `ANGEL_GLASS_ART=<pack>` (the knights' pack from `ladder_pack.py stills`), and `ANGEL_DELVE_LADDER=64` to be on the 64 rung from floor one (else the floor climbs it). In a browser: the host with `ANGEL_DELVE_HD_WEB=1` (and the same `ANGEL_GLASS_ART`), the page's link with `?hd=1` before its `#` (`&ladder=64` to look at a rung).

`delve_ladder <out> --hd <pack> [--scale N] [--left] [--lit] [--stride S] [--swing N] [--time N]` (crates/glass/examples) shoots the Crypt's first fight room with the four seats standing in a row at each rung (16, 32, 64, 128), as the window draws it (the Delve's `StageMaker` and its HD drawer, the GPU core offscreen, 8 window pixels to a room pixel unless said): `room-<rung>.png`, `seats-<rung>.png`, `ladder.png` (a row a rung). `--left` turns them, `--lit` stands the room up in 3D (lit cards), `--stride` and `--swing` put them on their walk or in a stroke, `--time N` times a frame. `ANGEL_HD_STAGE=<dir> ANGEL_HD_PACK=<pack> cargo test -p angel-delve write_hd_stage_frames -- --ignored` records the stage frames a page is sent with the pack, for drawing with the page's own `stage.js`.

Measured 2026-10-28 on this machine, the four seats' knights (one a design):

- **The window.** The stills pack (216 entries, 18 files) reads in 3 to 5 ms as the window opens; the placeholder pack of every clip at 64 (1116 entries, 558 files) in 17 to 20 ms. A stage frame with the knights' first drawings sent: 2.1 ms at 32, 1.9 at 64, 2.6 at 128 (4.0 at 16, the first frame's own pictures); once sent, 0.55 to 0.65 ms, as at 16. The four drawings are 12 KB of pixels at 32, 49 KB at 64, 196 KB at 128, sent once (twice that in a lit room, with their normal maps). A body frame at 64, cut to its ink, is about 52 x 64.
- **The wire, for a page.** The stills pack's manifest is 27 KB, 1 KB gzipped (the placeholder pack's 162 KB, 7 KB); a still is 1.9 KB at 64 and 3.9 KB at 128, a body frame 0.8 KB. A page fetches only what its knights on screen want, once: one knight standing at 64 is the manifest and one 2 KB PNG.
- **The page.** Driven in headless Chromium (WebGL2, the page's own `stage.js`; the GPU renderer had no adapter headless) against a real host (`ANGEL_DELVE_HD_WEB=1`, `?hd=1&ladder=64`, a kitted knight in the tavern): the manifest (4.5 KB gzipped, an earlier 1080-entry pack) and `knight/great-helm/idle/seat1@64.png` (2.5 KB) came over the wire and the knight was drawn at 64, 1:1 at the page's 4x (`<scratch>/ladder/shots/browser-kitted/`). The module's frame stayed 0.4 to 0.9 ms p50 with and without, its memory the same. `stage.js` given the page's recorded frames (`write_hd_stage_frames`) draws a frame with the drawings in 0.6 to 1.1 ms, as without (1.1).

## Not built

- The unkitted knight: the rig draws a knight only once it has its helm and harness (`default_rig`: gear 0, helmet and body owned). A hosted run's new knight starts in smallclothes with its fists (gear 2) and is drawn by the old realm body (`realm_body`), which carries no name, so it stays at 16 until it is kitted. Naming that body (`knight/realm-body/<view>-<stage>/<step>/...`) and drawing it at the rungs would carry the ladder from the first minute.
- Gear by name: the arm, shield and hat stay 1x over a finer body.
- Foes, props and the room by name (the room's kit tiles keep their hash path, `RoomScene::fine`).
- A rung-up moment (the reward-moments lane, on `resolution::next`).
- Linear or mipmapped sampling for a drawing shown smaller than its pixels: both the window and the page sample nearest, so the page caps its scale, and the window, which does not, shows a 128 drawing sampled down when it is smaller than 8 window pixels to a room pixel.
