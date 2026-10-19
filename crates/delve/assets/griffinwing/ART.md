# Griffinwing: art requests (identity before pixels)

Everything on screen today is a labelled block: the stage lays the canyon and
every actor as boxes and flat quads in true 3D, and the reticle, brackets,
footprints and HUD over it in the realm's palette. `slots.json` names every
slot the art wave will paint, with the native canvas, the bottom-centre
anchor, the animation timing in ticks (30 Hz) and the sim's radius where one
applies. **Art never sets a hitbox**: every slot's `hitbox` is `[0,0,0,0]`;
the sim judges every hit in rail space by the radii in the brief, and a
painted actor may not look bigger or smaller than its radius by more than a
tenth of a unit.

Painted concept sheets (read-only, not copied into the repo) live in
`~/angelX_minigames/games/griffinwing/assets/raw/`:
`griffin-rear`, `wingmen`, `kites`, `gloam-units`, `wyrm`, `props`,
`canyon-tex`, `clouds`, `sky-thunderhead`, `keyart`.

## House look

A dusk canyon under a teal moon: purple-blue clouds, a warm horizon, ruined
arches and towers hung with lanterns. Dark contours, hue-shifted materials,
no near-white cores except the cold light itself. The realm's own palette (75
inks) has no violet: the Gloam's cold light is aqua and white, ours is gold.
Gold means ours (lance bolts, hearth rings, gates, brackets); cold aqua-white
means the Gloam (its bolts, lamps' shields, snuffer eyes); red is danger (the
tell's footprints, the shield running low).

## Slots, with the one thing each must say

| Slot | Silhouette and materials | The signature feature | States and ticks |
|---|---|---|---|
| `griffinwing.griffin` | A griffin from behind and above, wings out: tawny lion's body, white eagle head just over the shoulder, a plumed rider in armour, a saddle cloth in the seat's colour (six seat trims by recolour pairs) | The wings: they bank, climb, dive and spin | level / bank-l / bank-r / climb / dive 4x6; roll 6x5 (30 ticks, the roll knob); boost 4x4; brake 4x6; fall 5x8 |
| `griffinwing.rider-aster/mira/tovin/nessa` | 20x20 portraits for the call-outs: Aster Vale (Lantern Rider, gold), Mira Quill (Bright Witness, aqua), Tovin Reed (Pathfinder, green), Nessa Flint (Kindly Shield, blue) | Each shouts with the mouth open and a hand raised | idle 1x12; shout 4x3 |
| `griffinwing.wisp` | The small snuffer: a hooded wisp, a wick-stub tail, two pale eyes, facing the camera | The eyes: they brighten through a shooting wisp's tell | fly 6x5; tell 5x10; pop 5x3 (cold smoke and a spark of returned light) |
| `griffinwing.kite` | A lantern-bat: pointed iron hood, a purple-lit lantern body, leathery wings | The lantern: it flares when the kite latches on | fly 6x4; hunt 4x3; latched 4x6; flee 4x4; pop 5x3 |
| `griffinwing.turret` | A hooded lantern tower on a mossy stone plinth | The cannon-mouth aperture: it brightens for the whole tell | idle 2x15; tell 5x10; fire 4x3; pop 6x3 |
| `griffinwing.bomber` | An iron arrowhead mask, two pale eyes, bat-wing fins | The mask: it points where the dive lane goes | tell 5x10; dive 4x4; pop 5x3 |
| `griffinwing.lamp` | An iron lantern with a flame | The shield: a swirling cold-light bubble that parts when opened and shatters when the lamp breaks | shut 6x6; open 4x5; break 6x3; out 1 |
| `griffinwing.wyrm-head` | The Gloam Wyrm: a smoke-dark head in scales, lightning through the smoke, a brass horn-cone with a lantern on it, teal eyes | The horn lantern, and jaws that open wide for the spit and the roar | rise 6x15; idle 4x8; tell 5x10; spit 3x4; roar 4x6; hurt 6x6; die 10x6 |
| `griffinwing.wyrm-coil` | The body: a C and an S of scaled coil | The scales: a brass stripe along each coil | c, s, sweep 6x4 |
| `griffinwing.bolts` | 16x16 each: gold lance, white-gold starburst, a wingman's blue bolt, the snuffer's small spark, the cold Gloam bolt, and the Gloam bolt turned round (gold-white core, cold ring) | The turned bolt must read as ours at a glance | 2 to 4 frames of 3 ticks |
| `griffinwing.fx` | Pop, hit, reflect ring, lock bracket snap, gate ring, scraped rock, feathers, lamp shield parting, the Starfall star | The Starfall star fills the screen for 36 ticks | see `slots.json` |
| `griffinwing.gate`, `.hearth-ring` | A hoop of lit lanterns; a gold ring of eight lanterns | The lanterns: they chase round the hoop | idle 4x8; taken 5x3 |
| `griffinwing.arch`, `.ridge`, `.shutter` | A rock arch with its opening where the sim puts it; a ridge a bomber hides behind; iron-bound shutter plates | The shutter's warning lamps at its inner edges | idle; warn 4x6 |
| `griffinwing.beacon` | A tower with a great lantern | Dark, then lit when the Wyrm falls | unlit 1; lit 4x8 |
| `griffinwing.canyon-tex` | 64x64 seamless textures: strata rock, ruined stone, causeway brick, dragon bone, lava crack, moon mist | The causeway brick: lit windows and banners hang on it | static |
| `griffinwing.sky` | 384x112 bands: the moon's dusk with a warm horizon, the roost's smoke, the way home's dawn, cloud banks | Clouds that slide with the camera's heading, so a bend in the canyon turns the sky | static |
| `griffinwing.reticle` | The far reticle (ring and cross), the near one (four corner ticks), the lock bracket, the charge ring | The bracket's snap | lock 4x2; charge 8x5 |

## Delivery

Transparent RGBA sheets, measured crop JSON and SHA-256, one PNG per
`<state>-<index>` at the slot's native size with the common anchor, hard
edges (no dithering, no noise), palette and material masks. Keep the slot
names so the integration can swap placeholders for paintings. Numbers that
the art must respect, all in the brief: lance bolt 10, starburst 16 to 36,
turned bolt 60, Gloam bolt 25, spark 10; a tell shows 40 ticks at the least
(the stage's footprint rings are the truth, art does not move them); a roll
spins 30 ticks (reflecting the first 20).

Sound requests (the stage reuses existing names for now): a crisp bracket
snap for the lock; a short chain of rising pops for the release; a bright
chime for a hearth ring; a ringing reflect; a deep Wyrm roar and a lamp's
glassy parting. The mix names today: `spoil_pickup` (lock), `hit_magic`
(starburst hits), `kill` (a pop), `shield_block` (reflect), `roll`,
`shot_bolt`, `shot_spark`, `bomb` (Starfall), `gold` (gate), `shield_up`
(hearth), `boss_roar`, `boss_rage`, `boss_rise`, `boss_death`.

## Painted slots

Each has a map in `assets/art-slots/griffinwing.<id>/` (an ordinary import, see
`docs/SLOT-ART-IMPORT.md`, with a local `request.json` beside it, since
Griffinwing is not in the shared request inventory; `prepare-art.py` here
measures the deliveries, which carry no crop metadata, and pastes the six
picked canyon swatches on one sheet). The stage reads a frame by
`bank::at(id, state, tick)` or `bank::pic(id, state, index)`. A slot is painted
whole or not at all: if any frame of any state is missing, every state of it is
the code's art, so a set piece never changes look when it changes state.
Durations are `slots.json`'s. Every picture that is not painted draws exactly
as it did before (the whole review set is byte-identical with the painted
pieces off).

| Slot | State |
|---|---|
| `sky` | On, behind the canyon. Each dot the scene leaves bare looks along its ray (azimuth to a column, elevation to a row), the bands two screen dots to a picture dot: the dusk is its band and the cloud band side by side, the smoke and the dawn their own band turned back at each repeat. Drawn under a dark layer of a fifth (`KEEP` in `sky.rs`: the painted amber is as bright as the gold marks over it). The painted moon stands at the code's own moon bearing, 2.35 rad, which the rail's heading (about -0.5 to 0.5) never turns to, so it is as off screen as the code's was; `MOON_AT` brings it into view. |
| `canyon-tex` | On, for this stage alone (`raster::Painted`, an opt-in over the rasteriser's shared material table; nothing else in the game hands it one). Rock, stone, dark stone (72 percent), the causeway's walls in brick and its road in stone, the roost's bone, the cloud sea below in mist (its own commit), all at 90 percent of their light (`KEEP` in `paint.rs`: at the lane's 70 the gorge falls under the canyon-reads test's floor). Lava stays the code's emissive. |
| `gate`, `hearth-ring` | On, as billboards over the 3D scene, scaled until the clear opening is the rules' radius less a tenth (2.0 and 2.2 units), idle frames on the sheet's ticks. The taken frames are imported, not played: a flown ring is not drawn for its seat, and the flash is the `fx` slot's. |
| `beacon` | On: dark until the Wyrm falls, then the four flames, from the tower's foot at the old box's height. |
| `arch` | Imported, off. The rules' arch is a wall from cliff to cliff with a rectangular opening 4.8 by 3.0 units (2.4 by 1.5 half), and the painting's opening is 78 by 92 dots with a round top and no wall: fitting the rectangle stretches it 1.6 to 1 across, puts stone in the opening's top corners where the rules let a craft through, and (tried in the engine, not kept) leaves the rest of the wall as the rock block with a squat arch set in it, whose right-hand pier is cut off by the wall's own depth where the camera is turned from the rail. An arch drawn square, with the wall's own stones round it, would fit. |
| `shutter` | Imported, off. The rules keep no warn clock, only the gap's triangle wave (`shutter_gap`), so the sheet's four warn frames would need a lamp count invented from the gap; and each plate is a wall from the gap to the cliff, up to eleven units across, while the painted plate is a fifth as wide as it is tall. |
| `ridge` | Imported, off (`runtime: false`). The drawing is 2.24 to 1 (128 by 57 on the 128 by 96 canvas); the rules' ridge is a box 6 units wide and 11 high. |
