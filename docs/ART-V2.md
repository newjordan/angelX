# Delve art v2 — black space and braille

`ANGEL_ART=v2` opts into this renderer. An unset variable, `v1`, or any other
value keeps v1. Run Cargo from `cockpit/`; this work changes presentation, not
simulation coordinates, collision, camera anchors, seat state or attack timing.

```sh
CARGO_TARGET_DIR=/work/build-cache/angelx-artv2 TMPDIR=/work/tmp \
  ANGEL_ART=v2 cargo run --bin angel
```

## The shared angelX contract

The terminal's **true black is the ground of the picture**. Rooms, settlements
and fields are lit islands. The old full-frame stone/grass fill and navy haze
are superseded. Black belongs to gameplay scenery as well as transitions.

`cockpit/src/stage/world_viz/art.rs` owns the shared material palette, grade,
coloured outline rule, compact light falloff, floor extinction threshold,
navy/brass UI inks, key gutter, native dot composition and terminal cell painter.
The Delve renderer and mini-viz kingdom walk both consume this contract.
Scene code supplies geometry and light-source locations rather than a new grade.

- Stone, steel, wood, foliage and party colours retain warm highlight / cool
  shadow ramps. Low material samples become coloured occlusion, not a white
  contour. Rounded armour and the knights-sheet figure standard remain.
- Light has compact quadratic falloff and becomes exactly zero outside its
  radius. Floors have no ambient fill. Only touched material details survive;
  a low ordered threshold feathers their edges into true black. Buildings and
  figures have restrained visibility floors. There is no full-frame fog wash
  or camera vignette that changes a world sample when the camera moves.
- Contact shadows multiply toward black; they never deposit blue paper into
  unlit space. Stone cracks, grass and flowers stay quieter than bodies/cores.
- Pixels carry material silhouettes, figures, collision cores and sharp shot
  centres. Ivory is tonal `[226,222,204]`. Authored health, true-point, shield,
  timing and other gameplay cues retain their signal colours and positions.
- **Atmosphere is a separate coloured braille plane:** torch glow, window and
  morning shafts, haze, dust, steam, sparks, magic, shot halos and Helios fire.
  Helios's retained cold silhouette stays pixels; its gold-to-ivory emission
  mask moves to this plane and extinguishes at rest.
- Braille uses the existing ride/ambient `frame_to_braille_graded` converter,
  including its ordered tone curve, luminance-weighted ink and quantization.
  Native pictures rasterize those coloured dots over dark material. They do
  not substitute smooth RGB bloom for the terminal aesthetic.
- A terminal cell owns one glyph. Bright figure/core coverage keeps a half
  block; atmosphere owns a braille glyph over the dark material background.
  Ranked subcell samples retain small shots without turning every thin rail
  into a full-bright block. Local, joined and mini views share this painter.
- UI wells use black-surrounded navy, brass trim and tonal grey text. Native
  action plates reserve an eight-pixel gutter, separating `F+2` from `SPC`.
  Terminal trays have real text labels. Vote, race and result panels replace
  raster glyphs before drawing live text; unused space is explicitly cleared.
  Vote selection/rating/averages, race place/lap/caltrops and result/ranking
  values come from the same seat/simulation data as the native panels.
- Animation is driven by tick, stride, seed and simulation positions. There
  is no wall-clock input or random generator in composition.

## The material system (the art style, in every renderer)

This is the one style the Delve, the countryside, the dungeon and the kingdom
map are drawn in. It lives in `crates/delve/src/render/overworld/material.rs`;
scene code says *what* stands where and the helpers decide *how* it looks. The
v1 pixel renderer (the default) uses it today; the v2 layers keep the same
rules.

**Black paper and lit masses.** True black is the ground. Things that stand on
it (trees, hedges, water, walls, buildings, rocks, hay, barrels) are solid,
shaded masses. Things that lie flat (grass, sand, snow, trodden earth, mud,
scree) are dot textures with paper showing between the dots. Nothing fills a
floor edge to edge.

**One light.** From the north-west and a little above (`material::sun`).
Every mass is lit on its upper-left and falls a step into its own bank's
shadow on its lower-right, and leaves a dark dotted contact shadow on the
ground to its south-east. A wall with floor to its south shows its front
face; a cap meeting open floor gets a lit lip. Fire and lanterns add warm
pools through `dusk`; ambient dusk then holds each colour one or two steps
down its bank, dithering only across the band between two steps so a calm
surface keeps its texture.

**Ramps, not outlines.** Each material has a 6-8 step ramp inside the realm
palette, dark to light, shadows leaning cool and lights warm:

| Material | Ramp (palette inks) |
| --- | --- |
| Oak, ash | `f D F E l m A y` |
| Hedge, far canopy | `f D F E N m L` |
| Pine, yew | `s D F e e l C` |
| Copper (autumn, orchard) | `n b I p R o O` |
| Field and dressed stone | `K X g j G J h i` |
| Slate, cliff, roof stone | `s x S u U v V` |
| Timber | `n b I B P r R o` |
| Thatch, straw | `b I P r o O t` |
| Roof tile | `n b B p R r o` |
| Water by depth | `S` deep, `q` mid, `u` shallow, `Q z` glints, `b` north lip, `U v` south foam |

There is no black outline. A silhouette ends in its own darkest step where it
turns from the light (the rim of a leaf clump, the base course of a wall) and
in a lighter step where it faces the light. Signal inks (digits, `w a @ $ c`)
are never used for material; tonal greys stand in for white.

**How the dots carry texture.** The terminal's braille glyph is a two-by-four
cell, and flat ground is laid out in exactly those cells: each cell holds at
most a small pattern (a two-dot grass blade, a crest dot of a sand ripple, a
snow speck, a lit pebble), chosen by a slow patchy density field so the
ground reads as clumps and bare patches rather than an even speckle. Masses
carry *grouped* texture instead of noise: leaf clumps each lit on their own
north-west, wood grain along boards, stone courses with dark mortar and a lit
top edge, ripple dashes drifting east on every fourth row of water. All marks
key on fixed seeds and world pixels; water ripples and mill sails move with
the scenery's six-tick step and nothing else.

**Braille encoding.** Both braille encoders (the scryglass's
`ride::frame_to_braille_graded`, which also feeds the v2 effects layer, and
`ui::term::art`'s image braille) use one structure-aware fitter,
`ui::term::art::fit_braille`. For each two-by-four cell it picks the dots
that reproduce the cell's own light/dark split: a dot is on where the pixel
is brighter than a mix of the cell's mean, its neighbours' mean and the
midpoint of its own range; true black under an absolute floor is always
empty; a pixel on the threshold copies its left (else upper) neighbour so
edges run on across cells; only a genuinely flat cell is filled, solid once
it is more than dim, thinning by ordered dither only in the darkest fills.
The cell's colour is the mean of its lit pixels (gently lifted so a dim dot
shows), so value lives in colour and shape lives in the dots. Cost: 0.19 ms
for a 96x30 pane (0.16 ms before), 1.97 ms at 300x100, optimised.

**Surviving braille.** Where the picture reaches the terminal as braille
(the scryglass: the loop's crawl, rides and interiors), a cell gets one
colour and eight dots. So a face
meant for braille keeps its material in the upper-middle of the luminance
range, where dots fill solid, with black joints, fissures and ribs between
(`material::blocks`, `material::rubble`); shapes are at least two pixels
across; near-black areas stay truly black so they draw nothing. The crawl's
walls, floors and vault follow this; painted faces with identity (niches,
shelves, doors) are lifted two steps into that band. Review with
`ANGEL_BRAILLE_SHOTS=<dir> cargo test --bin angel write_braille_shots --
--ignored`, which saves each crawl and room shot as RGB and as braille dots,
plus the scryglass vista, ride and an interior.

**Caves and creatures.** Cave walls are packed boulders and fissures
(`rubble`), never courses. Every creature passes through `shade_sprite`:
its top edge lit a step, its underside a step into shadow, signal inks and
outline untouched, size and footprint unchanged.

**Detail budget.** Tiles are 16 px. The richest detail sits at room
boundaries and landmarks: the wood or rock edge (overlapping trees, south over
north, with undergrowth and reeds at its foot), buildings and set pieces, and
dressed floor along dungeon walls. Open ground keeps a low budget (sparse
tufts, a clover clump, pebbles, toadstools) so actors, shots and rings stay
the brightest, sharpest things on screen. Sprites (knights, foes, critters)
keep their own authored ramps and sizes; scenery never out-contrasts them.
Nothing in the scenery suggests a blocker that is not one, and every blocked
footprint is drawn as something you cannot walk through: a building, a
stack, a hedge, a pier, or a roped-off building site.

**The helpers.** `tree`, `bush`, `boulder`, `reeds`, `flowers`, `hay`,
`barrel`, `crate_box`, `water`/`water_field`, `ashlar`, `boards`, `roof`/
`roof_hip`, `house`, `block`, `tower`, `hedge`, `fence`, `sign`, `wall_tile`,
`skirting` and `ground`. `relief` gives any remaining flat fill the same rims
and grain before the light falls. The kingdom map's forest tiles are grown
from the same `tree`.

Review: `ANGEL_ARENA_SHOTS=<dir> cargo test --bin angel write_art_pass_shots --
--ignored` writes every countryside room, a fight and a start room for each
dungeon pack, and kingdom views. Before/after sets for this pass are in
`/work/artifacts/delve-world-20261013/art/`.

## Coverage and layers

The eleven deterministic review moments are handgonne, flower pattern,
Big Ripper, Grinwick rails, kingdom/College approach, great hall, ranch,
fields/Helios, privy result, chariot race and vote panel. The kingdom walk
mini-viz is reviewed beside the Delve at the same world camera. Its native
and cell panes use the actual mini-viz view sizing; `walk-view.json` records
the extents. A controlled 384×224 layer comparison is also retained.

Crypt/Archive ordinary fights, the two reviewed bosses, Pasture, Ranch,
CollegeHall, Kingdom and Sanctuary support v2. Other rooms and special
unconverted authored states keep their existing treatment. Gear, avatars,
dwarf, altered dress/colours, invisibility, crouch, roll/toss, frozen bodies and
vertical/thrown shields retain their authored cues. The mini-viz keeps
published work, traffic, growth and outcome props/cues around the shared kit.
Changes to an existing building use local braille emission deltas; an animated
legacy tower cannot overwrite the shared structural silhouette.

The native logical canvas is **768×448**. Larger outputs fit it with nearest
sampling. Small `frame_for` snapshot requests retain v1's original signal-ranked
API behavior; the real v2 terminal route consumes the full pixel/atmosphere
layers, so it does not silently switch its scenery back to v1.

`overworld/arena/art_v2/` contains material drawing (`paint.rs`), once-decoded
samples (`kit.rs`), scenery, figures, combat, race and UI composition. Embedded
material windows and figure studies remain reusable assets, not screenshot
backdrops. The same authored world footprints construct both kingdom views.

Every box the game draws (plaques, name tags, key prompts, the ENTER band,
boards, panels, menus, the tray's wells) comes from one ink-kit module,
`overworld/chrome.rs`: corners cut on a small fixed curve that grows with the
box, a lit row under a rim lit along the top and shaded along the foot, slate
falling toward black inside, a dark hairline inside a framed board's brass, and
a one-pixel shadow down and right. Its tones keep a frame's meaning (steel for
names, dim brass for a key's prompt, brass for what opens, timber for signs,
red for a dare, stone for what is spent), all on the realm palette. v2 stamps
these boxes as drawn and rounds its own `ui::well` panels the same way at its
two samples a pixel.

## Cache and frame budget

Static scenery includes sparse floor lighting and the static atmosphere plane.
Cache keys include raid, pack, floor, room fingerprint, barred doors, settlement
state, wishes, world region and course/seed. Ticks and actor positions are
excluded. Dungeon bakes cover the actual room, including tall boss rooms.
Kingdom/race regions advance at 64-world-pixel anchors; camera crops copy both
planes. The mini-viz has four separately bounded regions keyed by wishes,
origin and view size. The Delve retains four regions.

One bounded worker serves cold Delve and mini-viz bakes. Live cells and native
pictures admit work and present their available fallback while it completes;
exact review snapshots bake synchronously. Scheduling/eviction cannot change
snapshot pixels. The native mini map already composes on its image worker.
The two v2 cell routes do not cold-bake. Authored school/glass views retain
their existing presentation route.

Paint budget: **8.333 ms at 120 Hz**; larger/browser delivery: **16.667 ms at
60 Hz**. Measurements use optimized Rust tests and 80 changing ticks per scene.
`timings.csv` separates cold compose, cached median/p95, RGB/RGBA conversion,
content hash and lossless PNG encoding at 768×448 and 1152×672.
`terminal-timings.csv` measures the actual 96×28 ratatui routes: cell composition
and draw, and `Viewer::render_game_img` composition, nearest PNG upload and
Unicode placeholder placement. It excludes GPU/display/network latency.
`walk-timing.txt` records the cached mini-viz layer composition mean.

Measured locally on AMD Ryzen 9 9950X3D. Milliseconds; all cached p95 values
meet the 8.333 ms paint budget. Larger delivery sums meet 16.667 ms. Cold
bakes are excluded from the live paint path.

| Scene | Compose median / p95 | Kitty p95 | Cells p95 | Cold compose |
| --- | ---: | ---: | ---: | ---: |
| handgonne | 1.659 / 1.773 | 4.114 | 2.981 | 8.047 |
| flower | 1.620 / 1.989 | 3.694 | 2.647 | 8.226 |
| ripper | 2.237 / 2.845 | 4.378 | 3.365 | 15.539 |
| grinwick | 2.742 / 2.966 | 4.992 | 4.246 | 15.906 |
| kingdom | 2.553 / 3.041 | 5.759 | 4.244 | 49.486 |
| college | 1.523 / 1.870 | 4.078 | 2.955 | 8.090 |
| ranch | 1.691 / 1.886 | 4.429 | 2.955 | 16.237 |
| fields | 1.543 / 2.086 | 4.192 | 2.748 | 13.790 |
| privy | 5.652 / 6.327 | 8.062 | 6.703 | 12.290 |
| race | 1.722 / 1.950 | 4.296 | 2.932 | 31.024 |
| vote | 4.627 / 5.207 | 6.774 | 5.803 | 11.305 |

Mini-viz cached layers: **0.962 ms mean**. The complete CSVs also include
1152×672 delivery and conversion/hash/encoding costs. These are CPU timings.

## Real terminal reproduction

The ignored Rust writer derives every pair from the original fixed `Run`
fixtures. `moments.json` records tick, room/pack, actor positions and attack
counts. Today v1 and last v2 reference frames are retained from HEAD `342d24a1`
in `/work/tmp/review-shots/artv2-match/`. Native review streams use the real
`Viewer::render_game_img` encoder/upload/placeholder path. Fallback streams use
the same `paint_room_ui`/`art::paint_cells` functions as gameplay.

```sh
cd cockpit
CARGO_TARGET_DIR=/work/build-cache/angelx-artv2 TMPDIR=/work/tmp \
  CARGO_PROFILE_TEST_OPT_LEVEL=3 CARGO_PROFILE_TEST_DEBUG=0 CARGO_INCREMENTAL=0 \
  ANGEL_ART=v2 ANGEL_ART_SHOTS=/work/tmp/review-shots/artv2-black \
  ANGEL_ART_PREVIOUS=/work/tmp/review-shots/artv2-match \
  cargo test --bin angel write_art_v2_shots -- --ignored --nocapture
cd ..
python scripts/art-v2-terminal.py /work/tmp/review-shots/artv2-black
UV_CACHE_DIR=/work/build-cache/angelx-artv2/uv TMPDIR=/work/tmp \
  uv run --no-project --with Pillow python scripts/art-v2-black-review.py \
  /work/tmp/review-shots/artv2-black
```

The terminal runner uses an isolated Kitty window on the existing `ANGELCAP`
Wayland monitor. It shows the serialized ratatui buffers and captures the actual
surface with `grim`. A temporary rule makes only that review window opaque and
animation-free; it is disabled when the window closes. User configuration and
other windows are unchanged. `terminal-capture.json` records font, cell geometry
and transport. Capture PNGs are not font approximations or RGB-canvas stand-ins.

Outputs: `{scene}-four-way.png` (v1, last v2, new Kitty, new cells in reading
order), `kingdom-walk-beside-delve.png`, `contact-sheet.png`, `index.html`, native
and cell screenshots/streams, raw deterministic frames, figure study, manifest
and timing files. Open every four-way, the kingdom comparison and contact sheet
after each final capture pass; inspect native/cell images at full size whenever
the overview exposes a legibility or layering problem.

## Validation and remaining work

`cargo test together`: **892 passed**. `cargo test --bin angel world_viz`:
**398 passed**. Opt-in v2 module tests: **17 passed** with `ANGEL_ART=v2`.
The ignored exact review writer also passed. Coverage includes deterministic
layers, repeatability after eviction, static camera seams, compact light
extinction, bounded asynchronous bakes, protected cues, owner-local panels,
small/default fallback, clipped rail coverage, Helios emission at rest,
published mini-viz positions/lanterns and replacement of raster panel glyphs.


The cell path has less figure and typography detail than the native picture;
its text panels and trays recover the most important live values. Thin rails
and dense gas can still merge at small cell counts. The College name plaque
still crowds arriving figures; authored map/ranch raster labels lose detail
in the small cell projection. The retained map canopies,
field foliage and race rigs remain more regular than a fully authored pose and
environment bank. Some unconverted gear/body states retain original ink. These
are visible conversion limits, not reasons to restore a full-frame floor wash.

Further conversions should extend the shared actor/material bank and preserve
collision footprints, safe gaps, state cues and foot pivots. Add a deterministic
fixture, both real-terminal transports, a camera seam check and a cached budget
measurement before extending opt-in coverage. Complete directional/gait and
special gear atlases remain the next actor-bank work.
