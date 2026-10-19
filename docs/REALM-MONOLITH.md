# The realm in the monolith look

The operator, 2026-10-18, after the castle-and-river proof (`crates/glass/src/monolith.rs`, docs/GLASS_CORE.md "A look that lights its scene"): "def better as an art direction than where we were... redo the entire realm".

This replaces the painted-plate direction (docs/WORLD-3D.md, projection painting) for the realm's look. The blockout generators, set files, chunk streaming and the browser's set transport stay: they are how geometry gets everywhere. What changes is what the geometry is and how it is drawn.

## The direction

- **Colour dialled way back.** Each place has one clean, nearly monotone palette: an 8-entry ramp, dark to light. Walking from one place to the next shifts the whole palette's hue smoothly (zones with a feather), never with a seam. Warm light (ember ramp) only where something is happening: light = activity.
- **Scale and geometry dialled way up.** Large, almost monolithic brutalist shapes:
  - stepped plinths and batters;
  - full-height fins and deep slots;
  - heavy cornices;
  - masses of trees, never tree by tree.
  
  Castles are great exterior walls you walk round and never see whole.
- **Solid colours.** Surfaces are flat greys of chosen brightness (a kind's tone), lit by the look's sun with shadows and by its lamps, then placed on the place's ramp by the Bayer dither. No textures, no plates.
- **One continuous ground.** No visible chunk cells or seams: the ground is a height field that runs on across chunk borders, with water lowered into it and animated, roads the brightest ground (a ribbon to follow), objects placed on it properly.
- **Figures stay sprites** and keep their own colours (`Role::Figure`).

The reference frames are in `/work/artifacts/monolith/review/` (the film, `contact.png`, `stills/`).

## Scale

Units stay the Delve's stage pixels. A tile is 16 px. The kingdom's view is 24 x 14 tiles (384 x 224 px). A knight stands about 22 px, so a metre is about 11 px.

| thing | old | monolith |
|---|---|---|
| castle | 3 x 2 tiles, a block | a walled exterior 14 to 24 tiles wide; curtain wall 160 to 260 px high, towers 280 to 420 px, so never whole on screen |
| building of a place | a cottage, 2 to 4 tiles | one or two big masses, 4 to 10 tiles, 60 to 160 px high, stepped and slotted |
| trees | one cone a tile | woods as packed spires 60 to 160 px high, overlapping into one ragged mass |
| rocks | boulders | outcrops as great slabs and stepped shelves |
| water | a flat tile | lowered 10 to 20 px into the ground, banks sloping down, the water surface `Role::Water` |

The camera is the game's own (`Tilt` at `GAME_TILT`, docs/GLASS_CORE.md): it is what keeps the floor's flat layers (walkers, signs, shots, marks) registered with the 3D world. Do not replace it.

## How it is drawn (the engine)

- `scene::World::look: Option<Look>`: a World with a look that lights (`Look::lights`) is drawn through the lit look at the stage's own pixel grain (internal size = the stage's size, so a stage pixel is an art pixel). The floor, blocks and extra blocks are made final there; the figures are drawn after it against its depth, as now.
- The look crosses the stage wire with the world (sun, zones, lamps, ramps), so the browser draws the same.
- **Set files gain** (docs/WORLD-3D.md "Set files" grows these statements):
  - `ground <cols> <rows> step <s> at <x> <z>` then `heights <h...>` and `tones <t...>`: a height field of `(cols+1) x (rows+1)` corners, `s` px apart, its corner at floor point `(x, z)`, heights in px up from the floor, tones 0 to 100 (a corner's brightness). Neighbouring chunks give their shared edge the same heights and tones, so the ground is one surface.
  - Kind tones: every surface kind has a tone (`set::Kind::tone`), its brightness before light, and a role. `water` is `Role::Water` (bands and glints, the look's water ramp); `window` and `fire` are accents; everything else is plain and lit.
  - `lamp at <x> <y> <z> reach <r> strength <s>`: a lamp the look lights with (light = activity; at most 8 nearest the view are used).
- `look` is made per view by the realm (below), not written in a set.

## The realm (lanes)

- **realm/map.** The map changes castles need. The houses' castles on the March, the Keep in Castle Town and the Dragon Keep become great walled exteriors on the map: big footprints, closed ground, a gate with its stand, roads re-run round them. Old saves load; party check passes.
- **realm/kit.** The kingdom in the monolith language:
  - `world3d::realm`, each chunk's set from the map (the same chunks, streaming as now): the ground height field (biome relief, water lowered, ridges, the road a bright ribbon), water surfaces, the woods as massed spires, outcrops as slabs, each place's buildings and landmarks as monoliths, the castles on their new footprints as curtain walls, towers, gates and stepped plinths;
  - the look for a view: one palette per biome (11 ramps), zones from the biomes near the view (at most 4), the sun, lamps from the places where something happens.
- **realm/rooms.** The places' rooms, the dungeon chambers and the seeded rooms in the lit look: a palette and a lamp set per room theme, kinds toned, interiors' masses made monolithic where the generators allow.

Each lane works in its own branch off `world/monolith`, owns its files, and proves its work with offscreen shots plus the crates' tests.

## Castles on the map

Every castle on the map is a great walled exterior (realm/map,
`crates/delve/src/world/castles.rs`): a rectangle of **closed ground** the
walker goes round and never enters, with **one gate** in its south face
and its **stand**, the road tile before the gate, where its door is
entered. Nothing else stands on a castle's ground: roads, lanes, lots,
fingerposts, dressing and finds all keep off it, and the ground under it
is plain (meadow, ash for the Dragon Keep) for the kit to build on.

`world::castles()` is the one list to build them from, the sixteen house
castles in house order (`houses::ids()`), then the Keep and the Dragon
Keep. Each `Castle` holds:

| field | what |
|---|---|
| `kind` | `CastleKind::House(HouseId)`, `Keep` or `DragonKeep` |
| `name` | the house's castle (`Dawnward Keep`...), `the Keep`, `the Dragon Keep` |
| `foot` | `(tx, ty, tw, th)` in realm tiles: the curtain wall and all it holds |
| `gate` | `(tx, ty, tw, th)`: the wall tiles the gate opens through, `GATE_W` (3) wide, a tile deep, in the face `facing` names |
| `facing` | `Facing::South` for all of them today |
| `stand` | the tile before the gate's middle: the house knight's home spot, where E goes in |
| `colours` | a house's banner inks `(field, charge)` once its model chose them (letters of the signal ink bank); `None` for the Keep and the Dragon Keep |
| `doors` | other doors in its face: each place whose building stands inside the wall, with the tile before its door |

`Castle::holds`, `Castle::ground` and `Castle::place`, `world::castles::footprints()`,
`world::castles::at(x, y)` and `world::castles::of_house(id)` go with it. A house's
walls, tops, towers and roof (as its model chose them) stay in `houses::get(id)`.

| castle | footprint | gate tiles | stand | inside |
|---|---|---|---|---|
| the sixteen house castles | 14 to 21 wide, 8 to 10 deep (`docs/KINGDOM-OPEN.md` "The March" lists each) | the middle three of the south face | on the frontier road (eight, backs to the Marchwater) or the far lane (eight) | the house's own keep, in the ward |
| the Keep | (81, 49, 15, 5) | (82, 53, 3, 1) | (83, 54), its old door | the old keep as its gatehouse, the Rookery (door (86, 54)) and the Chapel (door (89, 54)) in its face |
| the Dragon Keep | (171, 16, 19, 7) | (179, 22, 3, 1) | (180, 23), its old door | the old broken keep as its gatehouse |

Each castle lies wholly inside one chunk, so one chunk's set holds it all.
The house castles keep six tiles of open ground between any two; the March's
roads run level along each face with the stand on their first row. The Keep
has its back on the Kingsriver's bank and the North Road along its west
wall; the Dragon Keep fills the crater's north rim in the Wastes. The flat
view (terminal and fallback) draws each as its wall ring: a curtain wall
round the footprint, corner towers, the gate cut in the front, the house's
keep in the ward (`render::overworld::castles`).

## The realm, lit

realm/kit (`crates/delve/src/world3d/realm*.rs`): the kingdom's 110 chunks in
the monolith language, the same chunks, keys and streaming as before
(`world3d::kingdom`), drawn lit with no plates. With `ANGEL_WORLD3D` on, the
kingdom's 3D world is the realm, in the window and on a page.

**A chunk's set** (`realm::chunk_set`, made once a chunk, 18 to 30 KB of text):

- **its cut of one ground** (`realm_ground`): a height field over the whole
  kingdom, a corner every half tile (8 px), each corner's height and tone one
  function of the map, so neighbours share their edges exactly
  (`neighbours_share_their_edges_heights_and_tones`). A corner touching a
  tile a knight walks or a building stands on is the floor (height 0): the
  knights stand as cards on the floor and the flat marks lie on it. The rest
  rises with its distance from the floor by its biome's relief, eased across
  chunk borders (the Highlands 38 px, the downs 18, the Wastes 22 in flat
  shelves, the Fen 4). Water lies 6 to 16 px down, its bank the floor, its
  surface (`plane water`) at -5; lava 4 to 8 down under a surface at -3 that
  glows dimly on the warm ramp (`role mud ember`, `tone mud 30`). Tones are each tile's ground
  (road 50, path 41, sand 38, grass 22, mud 15, a wood's floor 12) averaged
  at the corners, so the road is a soft bright ribbon; crop fields stripe.
- **stone bridges**: a deck a hair over the floor, parapets on the water's sides.
- **castles** (`realm_castle`, from `world::castles()`): curtain walls on a
  two-tier plinth, ribbed with fins, string courses and a heavy cornice, slits
  high up (some lit); square corner towers of 210 to 245 px; two gate towers
  and the gate's deep slot between them, a lamp hung in it, braziers at its
  mouth; the other doors in the face (the Keep's Rookery and Chapel) as deep
  slots; a keep in the ward, or, in a ward too shallow for one (the Keep,
  5 tiles deep), one great mass between the walls. Walls 160 to 190 px (the
  Keep 190); the Dragon Keep is slate, broken, its towers ragged. A house's
  walls choose its stone (plaster, stone, timber), its tops its crown
  (merlons, a stepped crown, caps of its roof). Builds inside a castle's wall
  are the castle's.
- **each place** (`realm_place`): one or two big masses on its footprint, its
  door a deep slot on the side its door faces, the plinth parted round it;
  heights from 44 px (one tile) to 120 (`height_for`: 60 on two tiles, 90 on
  four), towers half again. Archetypes: halls (cottages, workshops with a
  stack, barns), towers (the Rookery, wards, belfries, a lighthouse with its
  lantern lit), temples (chapels, the School, the College, the Observatory and
  the Bathhouse with a dome), a round hall (the Round Table), gatehouses,
  heaps of rock slabs with a portal (the mine, the Delve's gate, crags), arenas
  (the Colosseum, the Hippodrome), waterworks, ruins, wells and the open
  places (a low kerb round orchards, gardens, butts, flags as standing slabs).
  Nothing stands past a footprint.
- **woods** as dark six-sided spires, one or two a tile, packed and
  overlapping, 60 to 160 px rising toward a wood's middle; **outcrops** as
  great slabs on each two by two of rock with a shelf on the bigger, a lesser
  slab on a lone tile; **dead trees** as bare spikes on the ash; **the rim** as
  woods, slabs or nothing (the Coast's sea) by its biome. What stands may be
  at most `34 + 16 x` the tiles north of it no knight walks, so a wood never
  buries the road behind it.
- the fingerposts and the realm's set dressing as the chunks had them, and at
  most three **lamps** a chunk where something happens (a castle's gate, a
  forge, the Delve's gate, the Colosseum, a lighthouse), the weightiest kept.

**The look of a view** (`realm_look::look`, made in the frame for the stage's
own camera, so the window and a page make the same):

- one 8-entry palette a biome, dark, nearly monotone, each its own hue on the
  proof's value ramp: Castle Town indigo slate, the Greenwood pine, the
  Highlands steel blue, the Wastes ash and rust, the Coast sea teal, the Fen
  marsh olive, the Tilt-Yards ochre, the Fairground plum, Wind Country sage,
  Longbow moss, the March oxblood;
- the view's own ramp is the biome under its middle; the four other biomes
  nearest it are zones, each the largest box of its chunks round the nearest
  one in the five by five round the view, shrunk so its 224 px feather is
  centred on the border: walking over a border slides the palette, the same
  mix either side of the flip (`a_views_look_is_valid_and_slides_its_palette_over_a_border`);
- the proof's sun (low in the west-south-west, long shadows up and to the
  right, the fronts lit), its shadows over 960 px round the view; the water's
  teal ramp, the ember ramp; `metre` 11; the eight lamps of the near chunks
  (and what a run burns) nearest the view, flickering; the knight kept in
  sight 8 px over its feet, 30 px round, and the party's other knights
  (`Look::reveals`), once the renderer measures the reveal from a stage
  camera's own eye (`realm_look::REVEAL_ON_STAGE`, off till then).

**In the game.** `kingdom::Chunks` makes every chunk from the realm on a host,
asset folder or none (`Chunks::host`); a page keeps what it is sent.
`Chunk::lit` bakes a chunk's ground and shapes into one mesh a material, the
tones in the vertices' colours: a window draws a chunk in a handful of draws.
`StageMaker::kingdom_world` gives a window those nodes and meshes and a page
each chunk in sight by reference (its set text the first time), with the look
(`STG6`). `wire::WORLD_CHUNKS` carries set text only. What a run stands
(`world3d::lots`: builds in the realm's language, finds as small monoliths, a
campfire's and a candle's light) is lit extra blocks to a window and a set of
its own a chunk (`set lots`) to a page, so its windows and fires keep their
accents across the wire. `RoomScene::realm` carries the view the look is made
for.

**Proof.** `cargo run --release -p angel-glass --example delve_kingdom -- <dir>
--contact` shoots the window at Castle Town, the Village, the Coast, the Fen,
the Wastes (the Dragon Keep), the March castles, the Greenwood, the Delve's
gate, the Colosseum, behind a building, a biome's border and a road over a
chunk border; `--lots` the party's builds and the run's finds; `--walk` the
frame's cost. Shots of 2026-10-18: `/work/artifacts/realm-kit/` (`contact.png`,
`lots/contact.png`).

| walk east across three chunk borders, window | before (old blockout, no plates) | the realm, lit |
|---|---|---|
| made, p95 | 0.29 ms | 0.30 ms |
| made and drawn (GPU offscreen at 3x, waited for), p95 | 1.84 ms | 0.84 ms |

(The window's by `delve_kingdom --walk`, release. Before baking, every
spire and slab its own node, the realm cost 6.41 ms made and drawn: each is
drawn three times, for the shadows, lit and for the figures' depth.)

**In a browser** (2026-10-18): `delve-dev-host --scene kingdom` with
`ANGEL_WORLD3D=1` and no asset folder, the bundle by `crates/web/build.sh
web/delve delve`, headless Chromium with `?present=webgl` (ANGLE GL,
RTX 3090). The page draws the kingdom lit from the sets it is sent, flat
only until it holds every chunk its view overlaps; a knight walked the
Tilt-Yards to the Hippodrome and north to the Kingsriver's bank, the
palette sliding into the Wastes'. 60 fps, no frame dropped; the client's
wasm 0.6 / 0.8 ms p50 / p95, presenting 5.2 to 6.9 / 6.6 to 9.0 ms; 0.46 MB in
at the join (16 chunks' set text) and 0.64 MB after the walk (a painted
kingdom's walk took in 6.1 MB). A page fills each set into a node a shape;
it would draw faster baked as the window does (`kingdom::bake`, a stage-side
`Sets::fill` could do the same). Shots: `/work/artifacts/realm-kit/browser/`.

**Left, and asked of the engine** (`ENGINE-ASKS.md` on realm/kit): the reveal
measures its line of sight from the camera's translation, which for the
stage's custom projection is the stage's origin, so on a stage it opens a
stray hole and nothing near the knight (the realm leaves it off); and the
figures' depth pass knows no reveal. Until both are fixed a knight behind a
mass (north of a castle, behind a wood) is hidden; the window draws the
followed knight's card in the lit world for when they are. The stage wire drops a node's accent and role. The
reveal's margins are metres.

## Rooms, lit

The realm/rooms branch (2026-10-18): every room the game draws from above stands up lit in the monolith look whenever rooms are in 3D (`ANGEL_WORLD3D=1`), with no plate. It is the default 3D; a host that says `ANGEL_WORLD3D_PLATES=1` draws rooms as before (a painted item's plate cast on its blocks, plain rooms with `ANGEL_WORLD3D_PLAIN=1`). The places' rooms (`world::ROOMS`), the chambers of the seven delves, the side rooms, and the seeded floors' fight rooms, stairs, passages and open floors all stand up so; the side-on halls (the Ledge, the Wanted hall), the kingdom (its chunks) and a room the window draws as one picture (the Swamp's fog) do not.

### What a room is made of (`world3d::rooms_lit`)

- **Its own blockout.** `blockout::blockout` run over the room as the dice laid it, dressed as `plain::dressing_for` says (a place as its item, with its hand-placed `landmark.set` while the room is laid as the item's own; any other room as a chamber of its delve, or an interior of the home), so its walls stand on its wall tiles, its doorways are where its doors are, and nothing stands on ground a knight walks. No `fit`, no item: a room is its own set.
- **Monolithic masses** (`world3d::rooms_lit_mass`):
  - the far wall stands on its tiles as a plinth the tile deep (10 px), its face set back 5 px above it, the mass running on 28 px out of the room and rising to 112 px, out of the room camera's sight; full-height fins 8 px wide every third tile, and every other bay between them cut back 15 px into the wall as a deep slot over a sill and under a lintel (not beside a doorway, nor behind what hangs on the wall);
  - the side walls the same, plinth, set-back face, fins, at the height the generator gives them (they keep the floor in view);
  - a delve room's west wall (the kingdom's halls keep theirs), where it stands full height, is opened in every other bay between its fins (never at a run's end): a window 34 px wide from 14 to 44 px up, a sill under it and a lintel over, thin (10 px) and so are the 24 px of walling and the fins either side, with nothing behind, so a delve's low sun (it comes from the west) falls through as a hard-edged slanting shaft across the floor, 40 to 100 px from the wall and north of the window, in the shadow the wall throws;
  - a doorway's leaf is set back into the wall's thickness, the way through a deep slot between the wall's returns under a lintel as thick as the wall and as high;
  - what stands on blocked tiles and pillars, a box a tile or more across, steps up from a plinth to a body set in under a slab as wide as the plinth;
  - the rock's cores: where a stretch of wall stands in the room (an open floor's rock between its chambers, a passage's walls), each tile deep in it rises as far as the floor north of it lets the camera see over it (0.8 px a pixel of floor, at most 120), under a cap of dark slate, so it stands as a great mass stepping up away from the floor, not a low plateau;
  - torches, windows, banners and lamps go back with the face they hang on; the near wall runs on, low, toward the eye.
  Every change is kept only where it hides no more of the floor than the generator's blocks did (`blockout::worst_hidden_ok`).
- **Its ground** is one height field (`ground`): a quarter of a tile apart (a half in a room bigger than 30 x 18 tiles), at the floor line wherever a knight walks, sunk where every tile a corner touches is a hazard (a pit 36 px deep and black, water's, lava's and spores' beds 9 px under their surfaces), raised 3 px into a dais under what stands on blocked tiles. Each corner is toned by its floor (the kind's tone x 0.72: flags, earth, grass; a road and a path the brightest; mire darker; the way down a brighter landing), a few hundredths of variation tile by tile, and darker by a quarter at the foot of what stands.
- **What burns** is not the set's: its flames (torches, braziers: `# flame` lines), beds of coals (`# coals`), lava (`# lava`) and water (`# water`) are comment lines of the set's text, drawn by the frame as extra blocks. Flames burn as accents while a room is fought or lived in and sink to low warm-lit embers once a fight is won; lava and a hearth's bed are a deep ember crust (`#7c2a14`, an accent) cracked with fire that comes and goes (a quiet bed's few still cracks); water is drawn at tone 0.3 instead of the water kind's 0.5, so it runs on the middle of its ramp, banded and glinting.
- **Its lamps** (`lamp`): a lamp over each flame or cluster of them (reach 66, 86 or 104 px by its size, strength 0.95), one every four tiles along a lava pool (steady: lava burns whatever the room does), one over the way down (56, 0.7), and out of doors one by each lit window (34, 0.5). The frame adds what burns in flight (an ember, a fireball, a blast: the three strongest) as the first lamps.
- **A delve's floor is lit from its doors.** In a whole delve room spill falls from each door along a lane toward the room's heart (0.7 brighter than the floor beside it at the door, 2.5 tiles wide, its edge crisp, dimming by 0.7 toward the middle), and the floor loses 0.45 of its tone toward the walls, so the fight sits in the light and the rim falls away (`floor_light`).
- **An open floor's map** stands up in pieces of a kingdom chunk's size, as `plain` cuts it, each its own set (its rock's cores rising as the whole map's floor lets them); a view draws the pieces it overlaps and a ring round them.
- **An open floor is lit from its passages** (`OpenLight`, `open_braziers`). Its chambers have no doors, so the light comes from the mouths of the passages: a gap of two to eight walkable tiles (or a doorway) along a wall line, walled at both ends, a chamber wider than the gap on one side and a passage on the other. From each mouth a lane falls into the chamber as a door's does (0.7 brighter, a half tile wider than the gap each side, its edge crisp, 8 tiles long and dimming by 0.7 along it) and three tiles back down the passage; and the floor loses 0.45 of its tone toward every wall, from half a tile of it to five, so a chamber's middle is the light and its passages are dark. Braziers burn on the top of the wall (24 px up): one on each jamb of a mouth, but not on the near wall (it would stand in the camera's way), one along a long wall on a slant every 12 tiles (never within 6 of a jamb's), and on 20 in 100 of what stands on blocked tiles (tombs, shelves, boulders, mushroom caps). They are flames of the set like a room's torches, drawn by the frame, each with a lamp.

### The look (`world3d::rooms_lit_look`)

One dark, nearly monotone ramp of eight per kind of room (`Tone`), dark to light:

| tone | what | ramp |
|---|---|---|
| crypt | Night slate: the castle proof's indigo with a breath of violet | `#0d0d1a` `#141528` `#1c1e38` `#27294a` `#33365d` `#444774` `#5b5e8d` `#7c80ad` |
| mines | Slate: wet rock under a cold light, so a lamp is warm against it | `#0b0d10` `#111519` `#191e24` `#232a31` `#2f3841` `#3f4a55` `#53606d` `#718291` |
| dragon-keep | Wine: soot and old brick gone to mauve, so the lava is the only red | `#110608` `#1a0a0c` `#250f12` `#32171a` `#401f22` `#512b2d` `#673a3c` `#824e50` |
| archive | Drowned teal: stone the water got into | `#060f0f` `#0a1717` `#102120` `#172e2c` `#213d3b` `#2d504d` `#3d6863` `#568b84` |
| fungal | Violet: a forest of mushrooms in the dark | `#130a17` `#1e1024` `#2b1731` `#3a2142` `#4d2d56` `#643d6f` `#80508b` `#aa6db6` |
| unknown | Void: near-black ultramarine, cold to the last entry | `#060811` `#0a0d1c` `#101427` `#181d35` `#212946` `#2e385a` `#3f4b74` `#5a699b` |
| swamp | Moss: the mire under Sir Damp's Marsh | `#0c0f0a` `#141910` `#1d2417` `#28301e` `#353f27` `#455232` `#5b6a42` `#7b8b59` |
| keep | Slate: the castle proof's indigo | `#0e0f19` `#141727` `#1b1f36` `#242a47` `#2f3658` `#3e466b` `#545c82` `#777fa2` |
| tavern | Smoke: a tavern's soot-dark timber and plaster | `#100e0d` `#191614` `#24201d` `#312b27` `#403933` `#534a42` `#6d6257` `#918475` |
| chapel | Limestone: cool blue-grey | `#0d0f13` `#16191e` `#20242b` `#2b313a` `#39404b` `#4b5461` `#646f7d` `#8994a1` |
| forge | Gunmetal: iron and soot | `#0c0d0e` `#151618` `#1f2124` `#2a2d31` `#373b40` `#484d53` `#60666d` `#838990` |
| hall | Granite: a warm-neutral grey | `#0e0e0e` `#171717` `#212120` `#2d2c2b` `#3b3a38` `#4e4c49` `#67645f` `#8b877f` |
| tower | Night: violet-blue | `#0c0b16` `#131122` `#1b1830` `#252040` `#312a52` `#413768` `#584b84` `#7b6ca8` |
| college | Verdigris: green-slate | `#0b100f` `#121918` `#1a2422` `#23302d` `#2e3e3a` `#3d514b` `#536b63` `#758f85` |
| undercroft | Ochre stone: the company's cellar halls | `#0f0d0b` `#181511` `#231f18` `#2f2920` `#3e362a` `#514737` `#6b5e49` `#8f7f63` |
| meadow | The castle proof's meadow: sage greys | `#0e1210` `#171f1b` `#222d27` `#2f3c33` `#404f42` `#556552` `#707e67` `#969f84` |
| dune | Dune: pale ochre in shadow | `#12100d` `#1d1a15` `#2a251d` `#393227` `#4a4233` `#605643` `#7c7058` `#a19375` |
| snow | Snow: blue ice, the lightest ramp | `#0f1218` `#191e27` `#252c38` `#343d4c` `#475263` `#5f6b7e` `#7f8b9d` `#a8b2c0` |
| marsh | Marsh: olive | `#0e100b` `#171a12` `#22261a` `#2e3323` `#3d432e` `#50573c` `#69714f` `#8c946a` |
| scree | Scree: grey-green stone | `#0e1010` `#171a1a` `#222625` `#2e3331` `#3d4340` `#505753` `#69716c` `#8d958e` |
| earth | Earth: umber | `#110e0b` `#1b1612` `#272019` `#342b22` `#44382c` `#584939` `#73604b` `#988166` |

- A delve's rooms take its stone's tone; the kingdom's interiors theirs by kind (the Keep's halls and grander rooms, a tavern, the Priory and the Sanctuary, a smithy and the forge-hall, the halls of dressed stone, the tower, the College, the Undercroft's rooms); its open places theirs by ground. Water runs on its own dark teal ramp (a mire's darker green).
- **Warm only where something burns**: lamplight moves what it lights toward the ember ramp (the castle proof's); the accents are the water's glint, a flame (`#e8742c`), a lit window (`#eeae55`) and the deep ember of lava and coals. What burns is its place's: the Fungal Deep's lamps burn spore green (a green warm ramp `#091503` to `#a6c659`, a flame of `#b4e05c`) and the Archive's candle gold (`#151106` to `#c8b97c`, `#f2c25a`); the other delves burn embers. A delve's ramps share the castle proof's brightness ladder and differ in hue (the Crypt's lilac indigo, the Mines' steel, Dragon Keep's wine, the Archive's teal, the Fungal Deep's violet, the Unknown's ultramarine), so each reads as its own place with its light the one warm thing in it.
- **Light = activity**: a room being fought burns its lamps full; a room lived in or with no fight at 0.8 of their strength (0.92 of their reach); a fight room won at 0.42 (0.72), its flames embers.
- **The key light**: indoors a high window to the front left (toward `(-0.52, 0.72, 0.46)`, strength 0.7 over an ambient of 0.24), under the ground a hard, low light from the front left (toward `(-0.70, 0.36, 0.50)`, 1.2 over 0.10, its shadows run 1.6 times the view's reach, its lamps burn 1.5 times as strong and throw 0.72 as far, a bright core and a falloff the dither shows and not a wash), so what stands throws long shadows and the lamps carry the room, out of doors the castle proof's low sun (`(-0.66, 0.52, 0.46)`, 1.05 over 0.26). Its shadows fall back and to the right, where the camera sees them, round the view's middle.
- The reveal opens what stands between the eye and the chest of the knight on show (13 px up, 18 px round); a metre is 11 px; a 4 x 4 Bayer dither; at most eight lamps, the nearest the view's middle.

### In the game

- `room::world` (and `room::world_in`, which adds the run's moment: how the room burns, where the knight is, its clock) hands a `RoomWorld` whose look is `Lit`; `stage_frame::lit_world` hands the window a lit `World` (its pieces' floor and blocks toned and lit, their grounds' meshes, the flames as extra blocks, the room's look) and, by reference, each piece's set text the first frame it is drawn. A lit room's still art lays none of its baked lights over it.
- A page makes the same: the run's tree carries a one-screen room's lit set (`wire::WORLD_ITEM`: its id, key and text, no plate), which the page keeps (`rooms_lit::receive`) so it draws the room's landmark with no asset folder; an open floor's pieces it makes itself from the run's tiles. Its frames carry the set's text once, then its key, and the look (`STG6`); `Sets::fill` makes the same lit blocks (`a_page_that_draws_scenes_is_sent_a_lit_room_by_its_set_and_its_look`).
- A terminal (a page that draws no scenes) sees the room from above on the same ramp (`Lit::picture`): each tile the tone of what stands tallest on it, lit from above, the floor dark and shadowed to the north-east of what stands, warmed round its lamps, lava and coals their crust and cracks, water its ramp, a pit black.

### Proof

- `cargo run -p angel-glass --example delve_rooms -- <dir> [--only NAME]`: the Village, the King's hall, the tavern, the Priory, the smithy, the market, the Sanctuary, the College's hall, the wizard's study, a chamber of the Crypt, the Mines, Dragon Keep, the Archive, the Fungal Deep and the Unknown, seeded fight rooms (Crypt, Mines, Fungal, Unknown), stairs (Crypt, Dragon Keep), passages (Crypt, Archive, Swamp), the Crypt's fight room won, and an open floor, two knights in each: `<case>.png` lit, `<case>-flat.png`, `<case>-terminal.png`.
- `cargo run -p angel-glass --example delve_world -- <dir>`: every item's proof room (138), lit.
- Shots in `/work/artifacts/monolith/rooms/`: `contact-rooms.png` (the cases above, lit, in that order), `contact-rooms-terminal.png` (the same as a terminal sees them), `contact-items-1.png` to `-4.png` (every item), the frames in `final/` and `items/`. The Swamp's fight rooms are drawn as one picture under their fog, so its passage stands for it.
- The agreement: `lit_rooms_agree_with_the_game` (every item's room, the seeded floors' rooms of every delve, floors 1 to 3, and an open floor's pieces: walls on their tiles, nothing over walkable ground, the floor in view, blocked tiles blocked, hazards low); `rooms_lit__tests` (the ground at the floor line wherever a knight walks and sunk under every hazard, what burns drawn by the frame, a window's and a page's blocks the same, a terminal's picture on the room's ramp, each kind of room its tone).
- The painted path's tests read plain test plates when a checkout has none (`assets::plate_pack`: the pack is git-ignored, and no longer the default look).

What they showed (2026-10-18): every room reads as one dark monotone place with its masses lit by the key and warm only round its fires; the far wall's fins and slots and the side walls' masses read as brutalist walls you never see whole; lava reads as a cracked ember crust, water as a dark pool; knights read clearly against the dark floors. The weak spots: the interiors' furniture (a tavern's tables, a market's stalls) is still the generator's small boxes, so a tavern reads as a bare hall; the tall side walls' inner faces, unlit by the key, are large dark planes at the frame's edges.

What the ambience pass showed (2026-10-18; boards in `/work/artifacts/monolith-private/ambience/`): the proof rooms read as dusk in their own stone (indigo, slate, wine, teal, violet, ultramarine) with a few small warm pools, nearer the lantern corridor than the washes they were. Over the six proof rooms the share of the frame a quarter or more of the way to ember fell from 42% to 12% (half or more, 27% to 5%) and the share brighter than luma 0.505, the brand guide's 1% line with the figures counted, from 2.4% to 0.9%; their medians fell by 0.05 at most (the Mines' and the Fungal Deep's frames now sit at 0.115, just under the guide's 0.12) and the seeded fights' darkest percentile is as it was. An open floor's arrival now has two to four lit tombs, shelves, boulders or mushroom caps in view and, where a passage is near, a lit mouth, where it was flat ground. Still weak: an open floor's arrival chamber is still a wide plain, its rock's long shadow the one hard edge across it; the west wall's windows throw a shaft two tiles wide at best, only where a side wall stands full height, and the shadows' soft edge limits how crisp it reads; the fights of the Mines, the Fungal Deep and the Unknown are calm wide floors; the Swamp's rooms and the kingdom's are untouched.
