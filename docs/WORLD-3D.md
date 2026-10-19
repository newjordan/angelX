# The world in 3D: blockout, paint-over, project back

The user, 2026-10-17: projection painting is the answer to angelX's 2D-or-3D
question. The world (ground, buildings, rooms, the kingdom) is rebuilt as
plain 3D blocks, a capture of the blocks from the game camera is painted over
in our style, and a shader projects the painted plate back onto the blocks
from that camera. The art stays painted; the window gets real depth
(figures walk behind walls and trees, light falls on the scene).

Characters (knights, foes, villagers, bosses) keep their sprites and stand in
the scene as upright cards. Only the world is redrawn.

## The pipeline, one item at a time

An **item** is one fixed view of the world: a hand-drawn room, or a chunk of
the kingdom.

1. **Blockout.** A set file (below) lists the item's blocks. Rooms and
   kingdom chunks get theirs from a generator that reads the game's own
   layout (tiles, walls, structures), so gameplay and picture agree. A
   landmark can carry hand-placed blocks on top.
2. **Capture.** `set_capture` renders the set from the game camera at the
   plate size: `capture.png` (flat colours by surface kind, lit by the
   scene's one key light from the upper left, a window and a fire shining
   by themselves), `ids.png` (one exact colour per kind, unlit),
   `depth.png`, `edges.png` and `legend.png` (below, "The capture").
3. **Paint-over.** The art lane paints over `capture.png` in the style
   (docs/ART-DIRECTION references, dark Metal Slug-era HD pixel art on
   black, cool shadows, warm lights, no hypercontrast), keeping every edge
   where it is. Output: `plate.png` at the capture's size.
4. **Register and check.** The plate is aligned to the capture's edges and
   scored; a plate that drifted is repainted.
5. **Project back.** The scene draws the set's blocks with a projected
   material (`Material::projector`): each surface is the plate where the
   projector (the capture's camera) sees it, untinted. A surface the
   projector cannot see is drawn with its kind's patch of the plate
   (`Material::patch`, below "Swatches"): behind the projector, outside the
   plate, a face turned away from it, or one it sees so much more edge-on
   than the camera does that the plate's texels would smear over it (its
   cosine to the projector under half its cosine to the camera,
   `scene::GRAZING`; a camera that is the projector never smears so). The
   patch is tiled over the surface from the side it faces most (a top from
   above, a front from the front, a side from the side), mirrored at each
   repeat, at the plate's own density, and shaded as the plate paints a
   block lit from above: a top 1, a front 0.86, a side 0.74
   (`scene::FACE_SHADES`; a window or a fire not). A kind the item has no
   patch of takes the nearest kind's; with no patches at all (or before the
   patch's picture arrives) the surface is its kind's colour two shades
   down its bank (`Kind::dark`, the realm's `light::step_down` 2), shaded
   the same: never the raw capture colour, never black.
   What stands between the projector and a surface is not tested: seen
   from the capture's own camera nothing hidden shows, and a camera that
   moves off it (a lean, a shake, the kingdom's camera on a knight) shows
   the painted surface in front smeared onto what it hid, a pixel or two
   past a block's edge in the kingdom. A projector depth test (each plate's
   blocks drawn once more from its projector into a depth map, every frame)
   would give those pixels the patch too; measured on the kingdom walk
   below it cost 0.6 to 0.8 ms a frame more (made and drawn, mean, 1.6 to
   2.3-2.4 ms, p95 +0.9 ms), so it is not done.

The terminal and any page that cannot draw scenes show the plate itself as
the picture, so nothing breaks off the window.

## Set files

Plain text, one statement a line, `#` starts a comment. Units are stage
pixels, as the Delve's camera uses them: `x` across to the right, `y` up,
`z` down the floor toward the viewer.

```
set yard-sample
camera room 768 448            # the Delve's room camera for a stage this size
plate 1536 1024                # the capture and plate size in pixels
box   stone  at 200 0 180  size 96 64 48            # a wall block
gable roof   at 200 64 180 size 104 40 56 turn 0     # a roof on it
cone  foliage at 420 0 300 size 40 70 40 id tree-3
```

- `set <id>` comes first. An id is lower case letters, digits and `-`, at
  most 64 of them.
- `camera ...` names the game camera the item is seen through, once:
  - `camera room <w> <h>`: exactly the camera the Delve draws a room on a
    stage of `w` x `h` pixels with (`angel_stage::scene::stage_camera` at
    the Delve's tilt, 0.3, `angel_stage::set::GAME_TILT`). The world is the
    room's stage: its pixel `(x, y)` is the floor point `(x, 0, y)`.
  - `camera chunk <w> <h> centre <cx> <cz>`: a stretch of the kingdom on a
    stage of `w` x `h`, in kingdom pixels: the room camera with the world
    moved so the floor point `(cx, 0, cz)` is where the room camera has the
    stage's middle `(w / 2, 0, h / 2)`.
  - The capture and the projector are both this camera framed in the
    plate: the stage is scaled uniformly (square pixels) to fit the plate
    less a 4% margin on each side, centred, so `projector = diag(sx, sy, 1,
    1) * camera` in clip space with `s = 0.92 * min(pw / w, ph / h)`, `sx =
    s * w / pw`, `sy = s * h / ph`. The margin holds what spills past the
    stage's edges (the tilted floor's near corners, what stands at its
    sides). A 768 x 448 room in a 1536 x 1024 plate is 1.84 plate pixels to
    a stage pixel.
- `plate <w> <h>`: the capture's size, at most once. Plates are 1536 x 1024
  unless a set says otherwise (the art lane's native size). Sides are whole
  pixels, 1 to 8192.
- A shape line: `<shape> <kind> at <x> <y> <z> size <sx> <sy> <sz>`, then
  optionally `turn <degrees>` (about the vertical, counter-clockwise seen
  from above: a quarter turn takes the shape's right to up the floor) and
  `id <name>` (an id as above, unique in the set), each at most once, in
  either order. `at` is the centre of the shape's foot: the middle of its
  footprint at height `y`, which the turn is about. `size` is its extent
  along x, y, z before the turn; an axis the mesh is flat along (a plane's
  height, a quad's depth) keeps its own, so write 0 there. No size is
  negative, and not all three are 0.
- Shapes are the built-in meshes by name (`angel_stage::shapes`): `box`,
  `plane`, `quad`, `sphere`, `gem`, `cone`, and the primitive library as it
  lands: `cylinder`, `frustum`, `wedge`, `gable`, `hip`, `pyramid`, `arch`,
  `stairs`, `rock`, `blob`, `fence`.
  A parameter is a `-<n>` suffix: `rock-7`, `blob-3` (seed), `stairs-6` (steps), `fence-5` (posts), `cylinder-24`, `frustum-8`, `arch-12`, `sphere-24`, `cone-8` (sides), clamped as the shape does; bare names keep their defaults.
- A line the parser cannot read is refused with its line number (every such
  line is told, not just the first); a set with any refused line is not
  drawn. A set with no `set` or no `camera` line is refused as a whole
  (line 0).

## The capture

`cargo run -p angel-glass --example set_capture -- <file.set> <outdir>
[--plate plate.png] [--sprite <png> <x> <z> <height>]...
[--neighbour <file.set> <plate.png>]... [--seam-report]` draws, at the
plate's size through the capture camera:

- `capture.png`: each kind's capture colour (the table below), lit.
- `ids.png`: each kind's id colour, exact and unlit; black is nothing.
- `depth.png`: 16-bit grey, the game camera's depth stretched over what is
  drawn: white the nearest surface, a dark grey (4096) the farthest, black
  where nothing is. (The room camera's depth is the floor row a surface
  stands on: what is level is level, whatever its height.)
- `edges.png`: dark lines on white where the id, the depth or the
  capture's flat colour changes (silhouettes, kinds, and the creases
  between a block's faces), for the painter to keep.
- `legend.png`: every kind the set uses: its capture colour as a top, a
  front and a side face show it, its id colour, and its name.
- with `--swatches` (and `--plate`): only `swatches.png` (below,
  "Swatches").
- with `--plate`: `projected.png`, the plate cast back through the same
  camera (it is the plate wherever a surface is), sprites standing in it
  as upright cards (`--sprite`: a picture, the floor point of its foot
  and its height in stage pixels, its width by its aspect) tested against
  the blocks' depth; and `projected-drift.png`, the same seen from a
  camera turned 4 degrees about the stage's middle, to show the depth. It
  prints how many covered pixels of `projected.png` are the plate's own
  (all of them, when the plate is the capture's size). Off the capture's
  camera a thin silhouette (a cone's tip) can carry a texel of what is
  behind it, and a face the capture saw edge-on is stretched.

## Swatches

`set_capture <file.set> <outdir> --plate plate.png --swatches` writes only
`<outdir>/swatches.png`, which goes beside the plate as
`<item>/swatches.png` (git-ignored with the plates: it is made from them).
For each kind the set has, it takes the largest square of the plate where
`ids.png` is that kind (and the plate is opaque), 3 pixels in from the
kind's edges, at most 64 a side (`angel_stage::set::SWATCH_CELL`), and lays
it at the top left of the kind's cell (`swatch_cell`: kind `i` of the table
at cell `(i % 9, i / 9)`), the rest of the cell clear. A kind with no clean
square 8 pixels across has none. A patch is tiled mirrored (each copy
flipped against its neighbours) at the plate's own density: on a room's
extra blocks, on a kingdom chunk without a plate (each kind's patch its own
picture), and on every surface of a painted item its plate's projector
does not see ("Project back": the patches laid back in one picture as
`swatches.png` lays them, `assets::swatch_atlas`, and each kind's size,
`set::Patches`, which a page that makes the blocks from the set is told
with the set's reference).

## Seams

Kingdom chunks are painted one plate each, so two plates side by side do not
match at their border unless the second was painted to continue the first.
They are painted in two checkerboard waves:

1. Colour the chunk grid like a chessboard, `(column + row)` even or odd.
   Chunks of one colour share no edge (only corners), so **wave 1** (the even
   chunks) is painted from plain captures, all at once.
2. **Wave 2** (the odd chunks) is captured with its painted edge neighbours,
   one `--neighbour <chunk.set> <plate.png>` for each of its up to four
   neighbours that share an edge (all of them wave 1). The capture's margins
   then hold their real paint, and the painter continues it across the
   border. A diagonal neighbour is wave 2 too, not painted yet: leave it out.
3. Wave 1 plates are never repainted. Each wave 2 plate is registered and
   checked as any plate is (the neighbours do not touch `ids.png`), then
   checked against its neighbours with `--plate plate.png --neighbour ...
   --seam-report`.
4. A wave 2 painter can redraw the whole frame instead of editing it (a new
   picture: the layout squashed, the painted margins turned black), and the
   edge score can still pass it, because busy texture puts a painted edge
   near every block line. So each wave 2 plate is also checked for its
   margins: the share of the capture's painted margin the plate turned black
   (6 of 55 on 2026-10-18 lost 54-94%, every other at most 4%), and one that
   lost more than 15% is repainted with a note that it is an edit.

`--neighbour` draws another set in the same render, through this set's capture
camera, as `Mode::Projected` with the neighbour's own plate and its own
projector (its camera line and its plate's framing), depth-tested together
with this set's blocks. Where they share a surface (a ground slab both lay,
the same house in both files) this set's wins: the neighbour is drawn a
hair larger about the camera's eye, which keeps every pixel where it is and
puts it a hair farther. A neighbour that really stands in front of this set's
blocks (a tree on the border) hides them. Each output:

- `capture.png`: this set's blocks flat as always; a neighbour's surfaces
  show their painted plate. Blocks of this set laid into the margin (a ground
  that runs past the stage's edge) cover the neighbour's paint there, so lay
  the ground of a chunk with painted neighbours to its stage's edge on
  those sides, not past it.
- `ids.png`: this set only, the neighbours drawn black (they hide what is
  behind them, and are nothing to the registration, which skips black).
  `edges.png`, `legend.png` and the coverage count are this set's too, and
  `depth.png` is this set alone.
- `projected.png` (with `--plate`): this set cast with its plate, each
  neighbour with its own: the seam check image. Registration is scored on
  this set's pixels only.
- `--seam-report` (needs `--plate` and a `--neighbour` whose stage shares an
  edge with this one's, which are chunk cameras one stage apart on the
  kingdom's floor) scores the colour step across each border. Points lie
  about every 4 stage pixels along the shared edge, on the ground there, and
  each is read 3 plate pixels either side of it: this set cast with its
  plate alone on its own side, the neighbour cast with its plate alone on
  its side (so ground laid past the border by both does not hide one plate
  behind the other). A point counts when both pixels are a ground kind
  (ground, grass, path, road, sand, water or mud: not a building or a tree
  standing on the border). It prints the points scored and the mean absolute
  colour difference, 0 to 255, per channel and over all three, for example
  `seam chunk-b (east border, 512 px): 104 of 128 points on ground kinds,
  mean colour difference 84.4 of 255 (R 119.4 G 16.4 B 117.5)`, and writes
  `seam.png`, `projected.png` with the points marked: magenta this set's
  side, cyan the neighbour's, yellow a point left out.

## Surface kinds

A kind tells the painter what a surface is and gives it a flat colour in the
capture and an exact colour in `ids.png`. The list is fixed; a new kind is a
change to this table and to `angel_stage::set`.

| kind | what it is | capture | id |
|---|---|---|---|
| ground | bare earth | `#857659` | `#303030` |
| grass | grass, turf | `#77a052` | `#303090` |
| path | a dirt path or track | `#ad946a` | `#3030f0` |
| road | a paved or cobbled road | `#8a8c90` | `#309030` |
| sand | sand, shingle | `#dcb878` | `#309090` |
| water | water | `#4a80b0` | `#3090f0` |
| mud | mud, bog | `#4f4a36` | `#30f030` |
| floor | an interior floor (boards, flags) | `#b08050` | `#30f090` |
| rug | a rug or carpet | `#9c4448` | `#30f0f0` |
| stone | dressed stone, masonry | `#b4b2a6` | `#903030` |
| brick | brick | `#a3553d` | `#903090` |
| plaster | plaster, whitewash | `#d4d4cc` | `#9030f0` |
| wood | timber, planks | `#7f552e` | `#909030` |
| roof | a tiled roof | `#c0683e` | `#909090` |
| thatch | a thatched roof | `#b5a258` | `#9090f0` |
| slate | a slate roof | `#5a6475` | `#90f030` |
| metal | iron, steel | `#7a90a4` | `#90f090` |
| cloth | cloth, banners, awnings | `#86599e` | `#90f0f0` |
| foliage | leaves, bushes, canopy | `#487536` | `#f03030` |
| trunk | tree trunks, logs | `#6e4430` | `#f03090` |
| rock | natural rock, boulders | `#6b6a64` | `#f030f0` |
| crop | crops, fields | `#a9b552` | `#f09030` |
| hedge | hedges | `#3b6b5a` | `#f09090` |
| bone | bone, skulls | `#e6d6a6` | `#f090f0` |
| door | doors and gates | `#4b4486` | `#f0f030` |
| window | windows (lit at night: an accent) | `#eec66a` | `#f0f090` |
| fire | fire, lamps, braziers (an accent) | `#e8742c` | `#f0f0f0` |

## In the game

A function of depth (see "Deeper floors" below): a host stands the deep
floors up and keeps the upper ones in the 2D black look, unless
`ANGEL_WORLD3D=1` (every room, the kingdom too) or `ANGEL_WORLD3D=0`
(none) says otherwise. A host that reads it (`delve-dev-host`, and the
cockpit beside its other Delve settings, `drive::delve_settings_from_env`)
hands its mode to every seat's page in the welcome
(`wire::Params::world3d`, `wire::Params::deep3d`). Off, every room draws
exactly as it did.

- **Items.** Nothing is compiled in: `crates/delve/src/world3d/assets.rs`
  reads the items at run time from the asset folder, the one
  `ANGEL_WORLD3D_ASSETS` names, else the crate's own
  `crates/delve/assets/world3d` as it was built (a developer's checkout).
  The host looks the folder over when it turns rooms in 3D on: an item is
  any of `world3d::pilot::every` whose folder has a `plate.png` (plates are
  an asset pack, git-ignored, synced from the paint kit). Its set is what
  the generator makes of its room today (`pilot::Item::set_of`, the
  folder's `landmark.set` appended), the one source of the blocks; the
  committed `<item>.set` files are the generator's output for the art lane
  (`the_committed_sets_are_the_generators`). Each item is read and checked
  the first time a room could be it: usable while `plate.set-sha256` is the
  SHA-256 of that set and the plate is a PNG of the set's plate size, else
  the room is drawn flat (`no_plate_is_stale` lists stale plates). A
  missing folder, or one without plates, and the switch is quietly off.
  An item is the world of the room it was made from
  (`world3d::pilot`): a place of the kingdom by its kind, while its tiles
  are the ones the item was made from (a place built over is drawn as it
  always was); a dungeon chamber, a side room or a nook by its source
  template, whatever the room's doors and loose blocks (`world3d::fit`,
  below).
- **Loose matching** (`world3d::fit`). A place of the kingdom is its
  item's only while every tile is. A room of a delve is its item's while it
  is laid from the item's template: its size and tiles the item's but for
  what the game may lay otherwise, its doorways (open or walled up as the
  floor joins it) and, in a delve that scatters loose blocks over its
  one-screen chambers (the Mines and the Fungal Deep four, the Swamp two),
  up to four times as many blocks on the template's ground (each lands
  mirrored four ways). A side room or nook is matched in the delve its
  stone is. Where the room differs, it is drawn as a variant of the item:
  - what the item's set stands on those tiles is cut from it (a box down to
    the pieces left off them, which the plate still projects onto; any other
    shape whole; a doorway's leaf, lintel and torches with their doorway),
    so the floor or the doorway shows: the set sent and drawn is the
    item's with those lines commented out and the pieces after them, its
    own key;
  - what the generator stands there for the room as it is (the wall of a
    doorway walled up, no taller than the item's wall beside it; a boulder
    rolled in; the floor and the leaf of a doorway the item has none at) is
    drawn as extra blocks (`scene::World::extra`, drawn with the blocks, sent
    across a wire as nodes), each of its kind's patch of the item's plate
    (`swatches.png`; the nearest kind's when the item has none of it; its
    capture colour, lit, when it has no patches): a box's top, front and
    sides cut into cells of the patch, every other cell mirrored, shaded
    1, 0.86 and 0.74 as the plate lights a block; any other shape lit, its
    patch on it by its mesh's uvs. A terminal paints each tile that differs
    with the patch of what stands tallest there.
  On seeded floors (each delve, floors 1 to 3, 40 seeds,
  `world3d_floor_sample`) 17% of all rooms have their item for a party of
  one (3.5% laid exactly as painted, the rest otherwise) and 24% for a
  party of two (7% exactly), with the 86 plates of 2026-10-18. Every other
  room but the side-on halls stands up plain (below, "Rooms with no item").
- **The window.** A room with an item (`RoomScene::world`) hands the
  window its world in the stage's props (`angel_stage::scene::World`)
  instead of its still art: the set's blocks placed on the room's stage
  (`Set::placed_world`, the room's pixels to the set's, scrolled by the
  game's camera, leaning in with a guardian's entrance), each with the
  plate cast on it; and each figure as an upright card (its own picture,
  nearest, on its foot, as far off as its foot), any HD drawing of its
  sprites a card over it. The GPU draws them into pictures of the stage
  texture's own size with one depth buffer: the floor first (what lies on
  it, `Shape::on_floor`, keeps no depth, so nothing sinks into it), the
  blocks over it, then the figures and the shots' and pickups' shapes,
  tested against the blocks. Each picture lies among the stage's layers
  where its 2D part did (the world where the still art was, the figures
  where they were), so the room's light, the shots' light, motes, the
  skills bar's veil and the screen's marks are as they were. What the
  still art drew that the game moves lies on the world in the picture
  under the figures (its traps, a cracked wall: `room_details`; a chest
  stands as its shape, as ever on such a page),
  a doorway's bars stand in it as cards while a fight lasts, and a pack's
  delivered room kit stands none of its objects (the blocks are the room).
- **A terminal** (and a page that draws no scenes) shows the plate laid
  back down on the floor it was painted over (`RoomWorld::picture`): each
  room pixel the plate averaged over the area its patch of floor is cast
  from, so figures drawn flat stand where the window stands them, and the
  floor behind a wall shows the wall, as the painted rooms draw a building
  up from its foot, the room's traps and chest on it. World rooms are true
  colour, so it is not snapped to the realm's palette (a dungeon chamber's
  frame neither, though its pack's kit is).
- **A browser** that draws scenes (`angel_stage_props(1)`) is sent `STG4`
  frames: the world's floor and blocks by their set's reference
  (`SetRef`: the set's text with the room's first frame, then its key), its
  figures as nodes. The plate crosses as the PNG it is read as
  (`SetRef::png`), once an item until the page asks for everything again,
  never as pixels; each item's plate is its own picture
  (`stage_frame::plate_picture`), so the page keeps every plate it was sent.
  Its kinds' patches go as a picture beside it, and the reference names
  it (`SetRef::patches`: the picture and each kind's patch's size).
  The renderer hands the PNG to the page (`plate_wanted`, `take_plate`),
  which decodes it with the browser's own decoder (`createImageBitmap`, off
  the frame) and puts its pixels back (`put_picture`); until then the blocks
  draw without it, and no frame asks for it again. The page's renderer
  makes the blocks again from the set (`set::Sets::fill`) and draws the
  stage with the same core as the window.

Proof shots: `cargo run -p angel-glass --example delve_world -- <dir>
[--item <id>]... [--variant <id>]...` (the window, through the GPU core
offscreen: `<item>.png` and `<item>-flat.png`, crops round each knight) and
`ANGEL_ARENA_SHOTS=<dir> cargo test -p angel-delve write_world3d_shots --
--ignored` (the terminal), both of `world3d::room::proof`: each item's room
as a run stands in it, its own people and props, a knight half behind its
tallest block and one in front of one (a room of a delve's fight laid where
the floor's first fight room was). `--variant` takes
`world3d::room::proof_variant`: the room laid otherwise than the item, its
east doorway walled up and, in a delve that scatters loose blocks, a boulder
rolled in mirrored four ways (`<item>-variant.png`). The first ten were
taken to `<shots>/integrate2/`; sixteen more (four places
and interiors of the kingdom, a nook, the Sanctuary, five bigger chambers of
five delves, and three chambers laid otherwise) to
`<shots>/integrate3/` (`contact.png`), each plate on its
blocks with the knights standing among them; the walled doorways stand as
wall of the plate's own stone, the rolled-in boulders and mushrooms among
the painted ones.

What they showed (2026-10-18): every item's blocks, plate, people, pads,
signs, stall keepers, horses and monsters stand where the game puts them;
nothing floats, a knight behind a cottage or a tomb has its legs hidden. The
plates leave out some things the dressing asked to be painted on open
ground: the orchard's scarecrow and lamp post, the marsh's lamp post and its
notice post (which E reads: its board still shows to a knight at it), and the
marsh's plate down into the Swamp is painted as a wooden deck where the game
has steps down. Those are the art lane's to repaint.

## Deeper floors

The dungeon becomes 3D as the party goes down (`world3d::room::Mode`,
`room::stands`):

- **Floors 1 and 2** keep the Delve's main look, the 2D black dungeon.
- **Floor 3**, the dragon's (`room::TRANSITION`): only its guardian's halls
  (the dragon's lair, the Pit) stand up, lit; the rest of the floor is 2D.
- **Floor 4 and below** (`rules::DEEP_FLOOR`, `room::DEEP`): every room
  stands up lit (`rooms_lit`, docs/REALM-MONOLITH.md "Rooms, lit"). The
  home and the kingdom never do, but with `ANGEL_WORLD3D=1`.

The mode rides the welcome (`Params::deep3d`, bit 2 of its flags); the
floor a seat stands on comes with every snapshot (`Run::dungeon.depth`),
so the host and each page decide room by room alike. A page that draws no
scenes (a terminal, the legacy presenter) keeps the 2D look on every
floor; only `ANGEL_WORLD3D=1` lays a lit room's picture down there.

The deep keeps the black dungeon's language: the field is black and light
is activity (a room's fires burn while it is fought and sink to embers
once it is won); what lies on the floor turns as the same dark card over
a pool of light as in 2D (`pickup_cards::nodes`, plain nodes a wire
carries); monsters are graded under the knights (docs/BRAND.md). A lit
delve's key light and ambient are held low there (`rooms_lit::DEEP_KEY`,
`DEEP_AMBIENT`) and its own lamps throw pools (`DEEP_LAMP_REACH`), so the
field is black and the light is where something burns or happens.

**The beat.** Coming down from above the deep into it marks the run
(`Run::deep_beat`, the tick; a save without it loads as none). For
`room::BEAT` seconds after, played by the run's clock alone so every seat
plays it alike: the cut held black for a moment (`beat_dark`), the camera
close on the knight on show easing back to the room's own view
(`beat_zoom`), and the room's lamps coming up one after another from the
party out (`beat_rise`, `lamp_up`). `delve-dev-host --depth 4
--beat-every 8` replays it to watch.

**What it costs.** `cargo run --release -p angel-glass --example
delve_depth -- <outdir>` fights four knights against a room of monsters on
floor 2, on floor 4 drawn 2D, and on floor 4 stood up, and prints each
frame's cost (the Delve's scene, its stage frame, the GPU's drawing), with
shots of each, the beat, a tour down the floors and the dimensions. In a
browser the same fight is `delve-dev-host --scene fight --depth 4 --bots 3`
with a page at `?present=webgl`. Measured 2026-10-18 (RTX 3090; headless
Chromium needs `--enable-gpu --ignore-gpu-blocklist --use-angle=gl` or it
draws on SwiftShader):

| ms a frame, median / 95th | floor 2, 2D | floor 4, 2D | floor 4, 3D |
|---|---|---|---|
| window: the Delve's scene | 0.49 / 0.71 | 0.57 / 0.86 | 0.69 / 0.98 |
| window: its stage frame | 0.12 / 0.19 | 0.16 / 0.23 | 0.18 / 0.27 |
| window: the GPU's drawing | 0.69 / 1.06 | 1.61 / 2.25 | 2.21 / 3.17 |
| browser: the game's wasm | 0.9 / 1.2 | 1.0 / 1.3 | 1.3 / 1.6 |
| browser: the renderer's present | 1.6 / 2.4 | 2.2 / 3.4 | 5.3 / 7.7 |
| browser: frames a second, dropped | 60, 0 of 608 | 60, 0 of 608 | 60, 0 of 608 |

Standing the deep up costs the window about half a millisecond of the
GPU's and the Delve nothing it notices; in the browser it is the
renderer's present (its wasm turning sets into blocks, and many small
WebGL draws) that grows. With the CPU slowed four times the same fight
holds 44 frames a second drawn 2D and 38 in 3D (present 12 / 15.6 ms
against 17.4 / 27.5): a slow machine's deep floors want the renderer's
draws batched.

**Powers change dimensions** (the hook, `arena::dimensions`). A knight's
shot in a window is a few solid shapes (`shot_meshes`), lying low over the
floor (`Dimension::Low`). It stands up (`Dimension::Raised`: at a knight's
chest, larger, rolling about its flight so its depth shows) on a floor
that stands up in 3D, and on any floor once its knight holds a wish's
third tier (`RAISED_TIER`, the tier a teacher teaches). A monster's shot
keeps low. `Dimensions::of` reads a frame's run once and `shot` decides
each; what a dimension looks like is `Dimension::lift`, `scale` and
`roll`. More dimensions, or powers that change in other ways, hang off
those two places.

## Rooms with no item

With rooms in 3D and `ANGEL_WORLD3D_PLAIN=1`, a room no painted item is
(most of a floor's) is not drawn flat: it stands up as its own blockout (`world3d::plain`), the
generator the items were made from (`blockout::room_set`) run over the room
as the dice laid it, so its walls stand on its wall tiles, its doorways are
where its door tiles are, and nothing stands on ground a knight walks
(`rooms_with_no_item_on_seeded_floors_agree_with_the_game` holds the items'
checks over the rooms of seeded floors, every delve, floors 1 to 3, and over
an open map of each delve piece by piece). Each block wears its kind's
patch of its theme's paint. It is off by default: unpainted, the patches
read darker and muddier than the flat art (Fungal, the Unknown and the
Archive as brown carpet, the Mines' boulders near-black), so a host keeps
them flat and only items' rooms stand up; its pages follow it. The way on is
to paint the room kinds a floor is made of, not to dress them in patches.

- **Dressing.** A room of a kind an item was made from (a nook, the
  Sanctuary, the vault, the forge-hall, a place of the kingdom built over)
  is dressed as that item, in the floor's stone when the item is a delve's;
  a room of the home wing no item is (stockpile, muster, workshop,
  quarters, the realm hall) as an interior; any other (a fight room, the
  stairs, a passage) as a chamber of its delve. The Swamp's blocked tiles
  are tussocks of rushes (`Prop::Tussock`, its painter's reeds). The
  side-on halls (the Ledge, the Wanted hall) are seen from the side and the
  kingdom is its chunks: neither is drawn so. A room the window draws as
  one picture (the Swamp's fog) stays one, as a painted room does.
- **Themes** (`world3d::theme`): each delve, the kingdom's interiors, its
  halls, the tower, the College, its open places. A theme's patch of a kind
  is the best of its painted items' (`swatches.png`): the biggest once laid
  at one density (3.68 plate pixels to a stage pixel, a one-screen room's;
  a bigger chamber's patch scaled to it, nearest, so its texel is as big on
  the stage as painted); of equals, one of the rooms the theme is made of (a
  delve's chambers before its nooks and side rooms), then the one that tiles
  best and is most typical (the least change across it at 8 x 8 blocks,
  plus its colour's distance from the middle of the kind's). Each is evened
  (its light flattened over 10 pixels round each pixel), so where its
  mirrored copies meet a pool of light does not repeat as a pattern. A kind
  the theme has no patch of draws in its capture colour two shades down
  (`kingdom::darker`), lit, never the raw one; the Swamp, no chamber of
  which is painted yet, borrows the Mines' patches, then the Fungal Deep's.
  A host makes them off the frame when it turns rooms in 3D on; the log
  names the item each kind's patch is of.
- **Blocks**, as a room's extra blocks: a box's faces cut into cells of
  the patch, every other mirrored, shaded 1, 0.86 and 0.74; any other shape
  lit, its patch on it by its mesh's uvs. A window is sent a theme's
  patches once, as pictures (`stage_frame::WORLD_THEME_SWATCHES`), and the
  blocks in the world's floor and blocks.
- **An open floor** (`ANGEL_DELVE_OPEN`: `RoomKind::Depths`, up to 168 x
  108 tiles) is stood up in pieces of a kingdom chunk's size (24 x 16
  tiles): each the blockout of its tiles and a ring of the map round them,
  cut back to its own tiles (a box to what lies on them, any other shape by
  its foot's middle), so no room's wall stands at a piece's edge. A view
  draws the pieces near it as the kingdom's chunks are chosen
  (`kingdom::near`: those it overlaps and a ring round them), those in
  sight; the ones it overlaps are made when first asked for, two of the
  ring a frame (`plain::RING_A_CALL`), about 2 ms each (release, native;
  a one-screen room's blockout 5 ms). A piece is kept by its own tiles: a
  gate shutting elsewhere keeps the rest.
- **A browser.** The run's tree carries the theme of the room on show
  (`wire::WORLD_THEME`: its name, its key and the patches as one
  `swatches.png`) while rooms are in 3D, whether the room has an item or
  not, so the tree's delta sends it once a floor, not once a room. The page
  keeps it (`Themes::receive`) and makes the room's blocks itself from the
  run's tiles; its frames carry them as nodes (`World::extra`, no set's).
- **A terminal** paints each tile with the patch of what stands tallest on
  it (a shape covering a quarter of a tile at least: not a torch),
  mirrored at the theme's density; what lies on the floor at 0.8, so what
  stands reads over it; a pit black.

On seeded floors (`world3d_floor_sample`, as above) 88.9% of all rooms
stand up in 3D (a party of one or two; 17.7% and 26.2% with their item):
every room seen from above. The rest are the side-on halls, the Wanted hall
(one a floor) and the Ledge. Of open floors' rooms (the map and its side
rooms, 210 floors) 73% stand up, the rest the same side-on halls.

Proof shots: `cargo run --release -p angel-glass --example delve_rooms --
<dir>`: rooms of seeded floors no item is (floors 1 to 3 of every delve, a
knight in each) and open floors (round the arrival and in a chamber), the
window in 3D and flat, a terminal's frame on and off, and a painted chamber
of each delve beside them: `<shots>/rooms3d/` (`contact.png`,
`contact-terminal.png`).

What they showed (2026-10-18): every room stands where the game lays it,
walls on its wall tiles, its doorways open, its blocks over its blocked
tiles, lava and water sunk, a knight behind a block hidden to the waist; an
open map's pieces meet with no seam. But beside the flat rooms they read
worse: a patch tiles as a regular weave (every other copy mirrored), the
rooms are darker and lose what the flat art draws on the floor (moss,
scattered rock, the pits' rims, the Swamp's green), the Mines' boulders are
near-black spheres, and the Fungal Deep, the Unknown and the Archive read as
brown carpet. The Crypt and Dragon Keep read best. Beside a painted chamber
of the same delve they are plainly a placeholder. In a terminal they read
clearly worse than flat: wall and floor are near one colour, and the flat
art's detail is gone.

## The kingdom

The kingdom is one great room (`RoomKind::Kingdom`, 240 x 176 tiles), each
knight's view following it (`follow_camera`). In 3D it is drawn as its
chunks (`world3d::kingdom`): 110 of 24 x 16 tiles, each its own set
(`chunks::chunk_set` of the map with no wishes, made once, 1.7 ms a chunk)
with its own plate cast through its own projector. It works with any
subset of plates.

- **Which chunks.** A view (24 x 14 tiles) overlaps at most 2 x 2 chunks;
  the ones it overlaps and one ring round them are its near chunks
  (`kingdom::near`, at most 4 x 4 = 16: what drifts in as the camera eases,
  what stands up into the view from below, the far floor a tilted window
  shows above it). A window draws those of them its stage camera can see
  (their blocks' box in clip space overlapping the screen; typically 6 to
  9), the rest only have their plates made ready.
- **Plates, resident.** A chunk's plate is usable while `plate.set-sha256`
  is the SHA-256 of its set and `plate.png` a PNG of its plate size; a chunk
  without one looks again `RECHECK_SECS` (10 s) after it last looked, so
  plates landing while a host runs come in. A plate is read when its chunk
  first comes near a knight and decoded on a thread of its own (3.6 ms a
  plate), and at most `RESIDENT` (16) are kept decoded, the least lately
  used let go: 16 is the most chunks near one view anywhere in the kingdom
  (`a_view_is_near_its_chunks_and_a_ring_round_them` walks every view), so
  one view never thrashes, at 6 MB a plate. A window holds the plates in 16
  pictures of its own (one a chunk it draws, the least lately drawn given
  over to the next) and is sent at most one plate a frame (`PLATES_A_FRAME`):
  a chunk whose plate is not there yet draws unpainted for a frame or two.
- **No plate.** A chunk without a usable plate draws its blocks with its
  kinds' patches (`swatches.png`, a stale plate's included) when it has
  some, else each kind's capture colour two shades down its bank (the
  realm's nearest plain ink, `light::step_down` 2), lit. A painted chunk's
  patches go with it too: the window's camera is centred on a knight and
  each chunk's projector on its chunk, so the window sees faces of a
  chunk's blocks its plate never painted (a wood's side where it ends at a
  chunk border, a pine's flank), and those take their kind's patch
  ("Project back"). The first time
  the kingdom is drawn in 3D (or the switch is turned on) every chunk's set
  is made off the frame and the log says which chunks have no plate
  (`world3d: the kingdom in 3D, N of 110 chunks with plates; without: ...`).
- **The window.** The knights stand among the blocks as cards with their
  contact shadows (`kingdom::layers_3d`), depth tested: a knight north of a
  house or a grove shows its helmet over the roof. What moves and marks the
  floor (walkers, ravens, region names, finds, signs, plaques, fingerpost
  arms, shots) lies on the tilted floor as it is drawn flat; the screen's
  own marks (the biome's name in its corner, the goal's arrow, the boards,
  the door's footer on a terminal, the ballot) stay put. A seat's HUD (its
  skills bar, minimap, hold line and the door's sign) is in a band under the
  room's view (`party::hud_band`), so the scene is made at the shrunk view
  (`view_rows`) the flat view uses and the bands are lifted round it
  (`party::lift_with`). What the run stands that no set has stands up among
  the blocks (below, "What a run stands"). The view is placed at whole
  pixels, as the flat kingdom is. Each chunk's nodes have
  ids of their own (`Chunk::ids`): a scene draws one node an id.
- **A browser** is sent the chunks near its own knight's view in its seat's
  tree (`wire::WORLD_CHUNKS`: each chunk's key, set text, plate and patches
  as PNGs; `wire::seat_tree` keeps only the seat's own, `wire::WORLD_NEAR`
  saying whose are whose), so a chunk crosses once while it is near; the
  page keeps what its tree holds (`Chunks::receive`, `Chunks::keep`). Blobs
  in a tree are shared (`Value::Bytes` is an `Arc`), so a plate is cloned
  and compared for nothing tick to tick. They come a few a snapshot: a
  knight's near chunks are ordered nearest its view's middle, and a seat is
  sent at most `wire::CHUNK_BYTES_A_SNAPSHOT` (2 MB, at least one chunk) of
  chunks new to it a snapshot (`wire::pace`, the hosts' `DocView::pace`).
  The sixteen near a view, painted, are 18 MB, over the 8 MB a page reads
  in one message (`proto::MAX_SERVER_MESSAGE`): sent at once (2026-10-18),
  the page refused that snapshot and its host held every one after it, so
  a knight that came out into the kingdom never appeared. A plate's PNG is
  up to 1.4 MB: a tree's blob reads to the message's end, not to the doc
  wire's 1 MiB count cap. The page draws the kingdom in 3D once it holds
  every chunk its view overlaps (`kingdom::overlaps`), flat till then,
  never with a hole. The page's frames carry the painted
  chunks as set references (`STG5`: an `STG4` world block, then the further
  sets' references, `scene::World::more`), each set's text and PNG once a
  seat, then its key; the page's renderer decodes each PNG itself
  (`set::Sets` keeps 32 sets) into a picture of its own a chunk; unpainted
  chunks cross as nodes (`World::extra`).
- **A terminal** (and a page that draws no scenes) has each chunk's plate
  laid down on the kingdom's baked picture where it has one
  (`kingdom::lay_plates`: each pixel the plate averaged over the area its
  floor is cast from), the figures drawn flat as ever; each chunk laid down
  once (8 ms) and kept, 32 of them. Where a plate is laid, each tile a
  party's build stands on is painted with the patch of what stands tallest
  there, and its name lies on its door row; a chunk with no plate keeps the
  baked kingdom's, which draws the builds already. The finds and the
  herald's posts are drawn flat over it as ever.
- **What a run stands** (`world3d::lots`). The chunks' sets are the map
  with no wishes; what varies run to run stands up as extra blocks in the
  chunk it stands in, drawn as a room's extra blocks are: each of its kind's
  patch of that chunk's plate (`swatches.png`), the nearest kind's when it
  has none of it, never cast with the plate. The nearest is one of the kinds
  that stand, and near enough to pass for it (capture colours within about
  30 a channel, `Chunk::standing_swatch`): a slate roof wears rock, but a
  lean-to's thatch never wears the path it would vanish into, nor a cave's
  mouth the rock round it. With none, it is its capture colour two shades
  down its bank, lit, as a chunk with no plate draws its own; a fire or a
  window shines in its own colour. Made from the run (its wishes, its finds and
  which are found, whether the herald is out), so a page across a wire makes
  the same of the run its tree carries and sends them to its renderer as
  nodes (`World::extra`); kept for four runs' worth, so a build raised or
  cleared in play shows the next frame.
  - *The party's lots* (`world::wishes::lots`): each built wish stands as
    the kit piece nearest what was wished for (`lots::kit_for`: the first
    word of its name, then of its maker's words, that names one, a tavern a
    cottage, a beacon tower a tower, a fellowship stone a cairn and flags;
    else by its size) on its lot's building rows, its door row open, sized
    for its chunk's camera and its bare tiles dressed as a yard, as the
    generator stands a place (`chunks::stand`). The flat kingdom clears a
    lot, so what the chunk's own set stands there is cut from it (a box down
    to the pieces off the lot, a tree or a rock whole; `Share::cut`): the
    chunk is drawn from its set so cut (`Chunk::cut`, its own key, sent to a
    page as its own set) with the same plate, kept and sent under the uncut
    chunk's number (`Chunk::plate_key`). Its name lies carved on its door
    row's ground, under the knights.
  - *A place a wish moved* (a countryside place stands on the free lot
    nearest its spot, so a wish on its ground moves it): its chunk's
    building where it stood is cut, and its kit piece stands where it is.
  - *The run's finds* (`world::finds`): a kit piece each on its tile, as
    its flat art stands it: a shrine (its candle lit once a knight has
    knelt), standing stones, a cave mouth, a campfire's logs and fire, a
    lost pack, a hermit's lean-to, wall stubs (and the cache till it is
    taken), a well. The wanderer and the hermit stay sprites, and E's key.
  - *The King's herald's two answer posts* while he is out; he stays a sprite.
  The castles' pennons are drawn over the castle's top as ever; the class
  banners and the kingdom's trinkets stand where the map puts them in every
  run and so belong in the chunks' sets, which do not have them yet.

Proof (2026-10-18, 24 wave 1 plates of 110): `cargo run --release -p
angel-glass --example delve_kingdom -- <dir>` shoots the window at Castle
Town, the Village, the Coast, the Fen, the Wastes, the March castles, the
Tilt-Yards, a chunk border a road crosses, a knight behind a building and
behind a grove, in 3D and flat: `<shots>/kingdom3d/`
(`contact.png`); the terminal's are `terminal/` there (`ANGEL_ARENA_SHOTS=<dir>
cargo test -p angel-delve --release kingdom_terminal -- --ignored`). The
painted chunks register with the unpainted ones (roads, rivers and lots run
on across their borders), nothing floats, and two chunks' ground never
overlaps (each lays its own stage). A painted chunk's plate keeps the
capture's thin outline along its floor's edge and the black paper past it,
so a border between a painted and an unpainted chunk shows a hairline; wave
2's plates are painted across their borders.

What a run stands (2026-10-18, all 110 chunk plates): `delve_kingdom --
<dir> --lots --contact` builds the realm's catalog wishes beside the
Village (the Tavern, the Wayside Shrine, the Fellowship Stone, the Beacon
Tower: a cottage, a shrine, a cairn and flags, a tower) and shoots them with
a knight at a door and one behind a build, and six of the run's finds
(standing stones, a campfire, a cave, a hermit's lean-to, a ruin, a shrine)
with a knight before each and one behind, flat and in 3D:
`<shots>/lots3d/` (`contact.png`); the terminal's are
`terminal/` there (`ANGEL_ARENA_SHOTS=<dir> cargo test -p angel-delve
lots_terminal -- --ignored`). The builds and finds stand where the flat
kingdom draws them, a knight behind the tower shows over its roof and one
behind the tavern only his helmet, the campfire burns; the site's board
over each build (`plaques`) lies over its blocks as every board does.

Reshot with all 110 plates and the unseen surfaces' patches (2026-10-18,
`<shots>/projfix/`: `before/`, `after/`, the rooms', and
`contact.png`): the bright streaks down pines', cottages' and walls' flanks
(faces a chunk's plate never painted, drawn in their capture colour) are
their kinds' patches now; the rooms, whose camera is their projector, are
as they were but for a few pixels; with `ANGEL_WORLD3D` off every shot is
byte for byte the same. The black band down the Greenwood where chunk-0-0
meets chunk-1-0 is chunk-0-0's plate itself: its painter left the strip
along the stage's east edge (pines and grass in the capture, 20 to 60 plate
pixels wide) black inside the capture's outline, which it kept. That plate
is the art lane's to repaint.

Frame cost walking a knight east across three chunk borders:

| path | flat kingdom | kingdom in 3D |
|---|---|---|
| window, made (scene and stage), p95 | 0.18 ms | 0.28 ms |
| window, made and drawn (GPU offscreen at 3x, waited for), p95 | 0.35 ms | 1.72 ms |
| browser, headless Chromium, `?present=webgl` (ANGLE GL, RTX 3090): the client's wasm, p50 / p95 | 0.4 / 0.5 ms | 0.5 / 0.7 ms |
| browser, presenting, p50 / p95 | 0.3 / 0.4 ms | 5.2 / 7.1 ms |
| terminal frame (`frame_for`, native size) | 0.10-0.16 ms | 0.15-0.29 ms |

(The window's by `delve_kingdom --walk`, release; the browser's by the
probe kit `seat.mjs` holding D for 19 s from the March's west end, both at
60 fps with no frame dropped; the 3D walk took in 6.1 MB, its plates, the
flat one 0.37 MB. The unseen surfaces' patches cost nothing the walk can
tell: made and drawn 1.70 ms mean before them and 1.75 after, five runs
each of builds made alike, within the runs' spread of 0.3 ms.)

## Browser

Checked 2026-10-18 in headless Chromium (a seat script,
`--enable-gpu --ignore-gpu-blocklist --use-angle=gl`) against a local
`delve-dev-host` with `ANGEL_WORLD3D=1`, the bundle built by
`crates/web/build.sh web/delve delve` with `WASM_BINDGEN` at wasm-bindgen
0.2.129. The run was `world3d::room::approach("crypt-bone-chapel")` (a room
one door north of the Bone Chapel, one knight), written by
`ANGEL_WORLD3D_APPROACH=<file> cargo test -p angel-delve
write_world3d_approach -- --ignored` and served with `--scene first
--snapshot <file>` (the default fight scene stands the party back at the
door every tick). A knight walked south into the chapel: with the GPU
renderer (`?present=webgl`) the chapel stands in 3D, its plate decoded by
the page, its doorways barred, its monsters and the knight as cards; with
the page's own presenter (`?present=legacy`) it is the plate laid down.
Shots: `<shots>/integrate2/browser-*.png`.

Checked again with the items read at run time (2026-10-18, night): the
Boulder Rill laid otherwise (`proof_variant`, written by
`ANGEL_WORLD3D_ROOM=<file> cargo test -p angel-delve write_world3d_room --
--ignored`, its fight cleared, served with `--scene first --snapshot`): with
`?present=webgl` the page draws it in 3D from the item the run's tree
carried (about 1 MB in, the plate's PNG most of it), its east doorway walled
up and its rolled-in boulders among the painted ones.
Shots: `<shots>/integrate3/browser-*.png`.

Checked through the full game (2026-10-18, all 110 chunk plates): the
cockpit hosting with `/dungeon_host`, `ANGEL_WORLD3D=1`, its own page. A
knight joined at the Delve's gate in the kingdom in 3D, walked it while its
chunks came in (50 MB in 25 s, no message over 2.1 MB, none unread),
entered the Village (its room in 3D) and walked back out; a seat of a host
with the switch off drew and walked as ever.
Shots: `<shots>/playtest/fixed/` (`contact.png`).

- Headless Chromium has `navigator.gpu` but no WebGPU adapter, so the page
  falls back to its own presenter unless told `?present=webgl` (the WebGL2
  build). Headless Vulkan loses its context; it is not used.
- No plate is compiled into the game's wasm. A page has no asset folder:
  the host puts the item of the room on show in the run's tree
  (`wire::publish`, field `world3d`: its id, key, set text, plate PNG and
  swatches PNG), which the tree's delta sends once a room's item changes,
  and the page keeps the newest few it was sent (`assets::receive`).

## Roles

- The engine lane: the projector material, the set format and capture, the
  generators from the game's layouts, registration and checks, integration
  into the game, final polish.
- GLM: the primitive library (parametric meshes for blockouts).
- The art lane (Codex Luna or Sol agents with image generation): the
  paint-overs, from the capture and the style references. We never paint
  plates ourselves.
- Characters stay sprites.

## Where things live

- `crates/stage/src/shapes.rs`: built-in meshes by name.
- `crates/stage/src/set.rs`: the set format, kinds and the scene a set
  makes (capture, ids, projected).
- `crates/gpu`: the projected material (`mesh.wgsl`).
- `crates/glass/examples/set_capture.rs`: capture and proof renders.
- `crates/delve/src/world3d/plain.rs`: a room with no item stood up as
  its own blockout, an open floor's map in pieces; `theme.rs` the themes'
  patches; `crates/glass/examples/delve_rooms.rs` their proof shots.
- `crates/delve/src/world3d/kingdom.rs`: the kingdom drawn as its chunks
  (which are near a view, their plates read and kept, laid down for a
  terminal); `crates/glass/examples/delve_kingdom.rs` its proof shots.
- `crates/delve/src/world3d/lots.rs`: what a run stands in the kingdom that
  no chunk's set has, as extra blocks.
- Set files and plates: `crates/delve/assets/world3d/<item>/` (`<item>.set`,
  `plate.png`, and the set's hash the plate was painted from).
