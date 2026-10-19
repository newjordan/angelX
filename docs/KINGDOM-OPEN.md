# The open kingdom

The kingdom map (the Delve's surface and the cockpit's mini-viz realm, one
geography) was a grid of 16x11 screens fenced with tree hedges: about a
hundred same-size places, tree tiles everywhere, one-tile roads, castles in
two rows of eight. This reworks it into biomes with room between things.
Every place, castle, NPC, trinket, wish and game stays; only where things
stand, the ground between them and the colours change.

## Size and fixed numbers

- Map **240 x 176 tiles** (was 112 x 121): ten by eleven **chunks of 24 x 16**
  (the cut for the 3D blockout/paint pass), fifteen by sixteen 16x11 screens.
  The baked landscape grows to about 87 MB with its ranks (was 28 MB).
- The authored screens become **set pieces**: each sits centred in its own
  chunk (4 tiles of ground left and right, 2 above, 3 below), and its outer
  hedge of trees opens onto the biome around it. Authored coordinates still
  go through `place_tile`/`place_px`, now a per-screen origin table.
- **Main roads 3 tiles** wide, **lanes 2**, town streets as authored plus verge.
- **Groves, not carpet**: trees and rocks come in clumps (radius 1-4) on a
  jittered grid with a set chance per biome; open greens stay open; no
  feature within 2 tiles of a road, nothing on a lot or set piece rim.
- **Gaps**: set pieces never share a chunk; countryside lots keep 3 tiles
  of ground on every side (was 2); castles keep 6 tiles of open ground
  between any two (a road's width and more).
- **Castles are great walled exteriors** (2026-10-18, docs/REALM-MONOLITH.md
  "Castles on the map"): a house's castle is **14 to 21 tiles wide and 8 to
  10 deep** (was 3 x 2), closed ground with a **3-tile gate** in the middle
  of its south face and its stand on the road a tile before it; each lies
  wholly inside one chunk. The Keep's wall is **15 x 5** at (81, 49), the
  Dragon Keep's **19 x 7** at (171, 16); their doors did not move.

## Biomes (chunk columns 0-9, rows 0-10)

```
     0 1 2 3 4 5 6 7 8 9
 0   G G G H H H H W W C      G Greenwood      H Highlands     W The Wastes
 1   G G G H H H W W W C      C The Coast      F The Fen       T Castle Town
 2   G G H H H H W W W C      Y The Tilt-Yards A The Fairground
 3   F T T T T Y Y Y C C      D Wind Country   L Longbow Country
 4   F F T T T Y Y Y C C      M The March
 5   F F T T T Y Y Y Y C
 6   F F F A D D D L L C
 7   F F A A D D D L L C
 8   F A A D D D L L L C
 9   M M M M M M M M M M
10   M M M M M M M M M M
```
| Biome (chunks) | Mood and ground | What stands there |
|---|---|---|
| **Castle Town** (10) | the hub: greens, cobbles, ornamental trees, all roads start here | Keep set piece (Keep, Rookery, Chapel, Round Table, Smithy, Scriptorium), Village, Artisan Quarter, market gardens with the Sloptomizer School, Land Office on the town green, Bathhouse |
| **Greenwood** (8) | old wood: dense groves with glades, streams | Observatory and Dark Forest set piece, Hedge Maze, Bothy, Timberyard, Orchard, Ruins |
| **Highlands** (11) | rock outcrops, pines, falls; mountains on the north rim | Mines, the Delve gate, Labyrinth College, Griffin Eyrie, Crag, Aviary, Belfry, Aqueduct, Priory |
| **The Wastes** (8) | ash heath, dead trees, scree, small lava pools: the exploration biome | Dragon Keep; most of the discoverables |
| **The Coast** (11) | dunes, sand, sea on the east rim | Repo Wards, Lighthouse, Cliffs, Saltworks, Strand |
| **The Fen** (11) | marsh, meres, reeds; the west estuary | Gatehouse and harbour, the Swamp, Millpond, Sluice, Sunken Abbey, Riverbank, Causeway |
| **The Tilt-Yards** (10) | open lists and paddocks: the venue cluster | the Lists, Colosseum, Knights Tournament, Hippodrome, Horse Ranch, Toll Bridge, Caravan Camp, Wheelwright |
| **The Fairground** (5) | one big fair field, a few trees | Fairground, Mummers' Hollow, Laundry |
| **Wind Country** (9) | downs: crop strips, hilltop rocks, few trees | Homecoming fields, Picnic Green, Windmill, Flower Fields, Downs, Kitewright, Loud Flags |
| **Longbow Country** (7) | hedged fields and butts | Scarecrow Field, Longbow Green, Fletchery |
| **The March** (20) | the frontier: great castles on rises, groves and crops between | the sixteen model-house castles |

Set pieces by chunk: Observatory (1,1), Mines (4,1), Dragon Keep (7,1),
Gatehouse (0,3), Keep (3,3), Lists (6,3), Repo Wards (9,4), Swamp (0,5),
market gardens/School (2,5), Village (3,5), Artisan Quarter (4,5),
Colosseum (5,5), Tournament (7,5), Homecoming (5,7). The two sea screens
stay authored but unset (the Coast grows its sea), and the two March
screen rows are retired; the March is grown. The Delve's gate and the
College stand on fixed lots above the North Road.

**Borders follow chunk edges** and are drawn as one natural feature each,
gapped where a road crosses: the **Kingsriver** (2 tiles of water, sand
banks) along row 3's top edge from the Wastes west to the estuary, bridged
on every road; **ridges** of rock between Greenwood, Highlands, Wastes and
Coast with a pass at each road; **streams** between the Fen and its
neighbours; **hedgerows** between the farm biomes and round Castle Town;
the **Marchwater** along row 9's top edge east to the sea. Big features
(rivers, the sea, ridges, set pieces) never straddle a chunk corner.

**Venues.** Games stay where they are played: pads in their rooms, arena
games behind the Colosseum's gates. The kingdom shows the venue (the
place's building) and never a ring; the Tilt-Yards are the attraction
cluster, the castles of the March their own.

**The March.** A frontier road east-west along rows 9-10 and a far lane
south of it, joined at the west end, by the South Road through the middle
and up the east end to the Coast Road. Sixteen great castles stand in two
staggered rows, each on a rise (a band of rock behind it and its back
corners, open toward its road): eight in chunk row 9 with their backs to
the Marchwater and their gates on the frontier road (chunk columns 0-7),
eight in chunk row 10 with their gates on the far lane (columns 0-2 and
4-8; column 3 is the open green where the South Road comes through and
the March's name is lettered). Each road runs level along a castle's face,
its first row the tile before the gate, and bends between. Hand-set sizes
and spots, never a grid: widths 14 to 21, depths 8 to 10, tops on six
rows, fronts on five.

| row | house castles, west to east (`(tx, ty, tw, th)`) |
|---|---|
| on the Marchwater | Sol (3,146,16,8), Luna (30,148,14,8), Grok (50,147,18,8), DeepSeek (80,146,16,8), GLM (102,147,15,8), Kimi (123,146,20,10), Qwen (149,146,15,8), Muse (170,146,21,9) |
| behind the road | LongCat (8,162,16,8), Hy (32,162,15,8), Nemotron (53,163,14,8), Gemma (98,161,19,9), Inkling (123,162,17,8), Laguna (147,162,19,9), North (172,163,14,8), Astra (192,161,17,9) |

## Signposts and biome names

A **fingerpost** stands on the verge at every road crossing of a biome
border and every junction of main roads (none closer than 10 tiles). Its
arms are worked out from the road graph: walking each road that leaves the
post, the first other biome reached within 60 road tiles names the arm,
else the nearest place that way. The post is a world prop in the dusk
timbers (drawn in the landscape for the cockpit map and the Delve atlas);
the arm names are lettered beside it in small grey only while a knight is
within 6 tiles, so they read as you pass and never clutter the map.

Entering a biome writes its name **small in the top-left corner** of that
knight's own view (top right while the goal's arrow holds that corner),
grey, fading over three seconds. No centre banner, no
minimap: the ground names (one per biome, on an open green) stay.

## Set dressing

Between the places the country tells small stories, so every screen of the
open kingdom has something to walk to and none is crowded
(`world/dressing.rs`, drawn by `render/overworld/dressing.rs`). Clusters of
non-interactive things in the realm's dusk inks, nothing lit, fixed for the
realm (placed from hashes of the map, never of a run), kept off roads, lots,
set pieces and the castles and three tiles or more from every door:

| Biome | The stories |
|---|---|
| Greenwood | woodcutter's camp (stump and axe, fallen log, woodpile), mushroom glade, hunter's hide, a felled tree |
| Highlands | cairns, shepherd's fold, a ruined watchtower stub, a prospector's dig, a waymarker cairn |
| The Wastes | bone field, burnt cart, ash pits, a knight's grave, a signal pyre, a roadside heap of bones |
| The Coast | beached boat, net-menders (nets on poles), tide pools, crab pots, a drying net |
| The Fen | sunken punt, a leaning hut on stilts, reed cutters, eel traps |
| Wind Country | hay field (stooks), bee garden (skeps), scarecrow, a sheep pen, a stone stile |
| Longbow Country | the same fields: bee garden, scarecrow, hay field, a fowler's corner, a stile |
| The Tilt-Yards | the horse lines (hitching rail, broken lance rack, trough), a paddock end, a farrier's corner |
| The Fairground | a packed-up wagon, bunting poles (dark), shuttered stalls, stacked barrels |
| The March | a burnt watch post, a wayside cross, a wrecked supply cart, a picket camp, a milestone, a boundary stile |
| Castle Town | market barrows and stalls, the stocks, a horse trough, garden plots |

A **wild camp** is a clearing three tiles wide (trodden ground, where a
find may stand) with its things round it and a **footpath** wandering to the
nearest road; a **wayside** is a thing or two beside a road. Each chunk is
asked for two or three of the one and one of the other (the set-piece chunks
keep their own place), and where a screen's worth of walkable country (21 x
13 tiles round a spot) would still hold nothing, one more goes in. The
other way is capped too: no view of 24 x 14 tiles holds more than nine
things, nor more than four of one kind. The numbers come to 183 clusters and
418 things, about four to a view. They are walk-through scenery, drawn in
the landscape; a cleared wish lot hides what stood on it. Footpaths
are one tile of `path` terrain: narrow ribbons of worn earth with ragged
edges, faint, brown on grass and ash, grey on sand, joining one another and
the roads where they meet.

## Discoverables

Thirty per run, placed from the run's seed (the same seed, the same thirty)
in the clearings of the wild camps above, so each stands at the end of a
footpath, off roads and clear of doors, weighted toward the Wastes (six) and
the Highlands and the quieter biomes. They reuse the kingdom's trinket flow
(walk up, press E, a short `found` line) and remember what was found in the
kingdom's saved state (one bit each; the first twelve keep their order).

- **wayside shrines** (E mends to full), **ruins with a cache** and **cave
  mouths** (E, once a run, a small purse of renown), **campfires with a
  wanderer** (E, a line that names the biome of a find nobody has made yet),
  **standing stones** (E, they hum and glow a moment);
- **lost packs** (E, once a run: a potion or a thunder scroll in the hand,
  else a bomb, else a purse), **hermits** (E, once a run: a thunder scroll
  for a potion, nothing taken and nothing marked if the knight has none),
  **wishing wells** (E, once a run: a coin of ten renown, the surface's gold,
  for a mend, a potion or a bomb, whichever the knight can use; none, no
  coin).

## Colour pass

Light means activity, nothing else. Pad rings and pad letters go from amber
to dark timber/structure inks, with the signal amber only while a round is
on. Festival booths (`three.rs` attractions), landmark buildings and the
Tilt-Yards' striped tents step down to dusk (no signal inks as decoration,
saturated inks one or two steps darker). Castle heraldry and banners step
down the same way. Nothing new in the signal bank.

## Saves

Old saves (112 x 121 kingdom) load: the old-size kingdom room still passes
the snapshot check, the migration swaps in the new map, the return door
goes to its place's new stand (the Keep's if the place is gone) and a
party out on the surface to the Keep.

Saves from before the castles grew (same size, other ground) load too: the
new ground replaces the saved one, a return door whose place's door moved
(a castle's gate) goes with it and the party with it, and a knight out on
the surface who now stands inside a wall steps onto the nearest open
ground.
