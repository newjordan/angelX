# The open kingdom

> **The shipped realm is the blank-slate plan (2026-11-08).** The operator:
> "one tiny castle, started, for the main castle, and then surrounding
> hillsides with nothing, but a camp, where the other models castles should
> be built. close to tabula rasa with just the basics pretty much." The
> upper world is now one open grass country with the Keep, a tiny castle
> just begun, in the middle of it, a camp for each model house on a loose
> ring of hillside round it, and the way down to the Delve beside the Keep.
> Everything below this box describes the **full plan**, the realm as it
> was; none of it is deleted, and it comes back by changing one line:
> `PLAN` in `crates/delve/src/world/plan.rs` from `Plan::Tabula` to
> `Plan::Full`. See "The blank-slate plan" at the end of this file.

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
post, the region entered names the arm at a biome border, and at a fork the
destination that way does (see "Where the fingerposts point" below). The post is a world prop in the dusk
timbers (drawn in the landscape for the cockpit map and the Delve atlas);
the arm names are lettered beside it in small grey only while a knight is
within 6 tiles, so they read as you pass and never clutter the map.

Entering a biome writes its name **small in the top-left corner** of that
knight's own view (top right while the goal's arrow holds that corner),
grey, fading over three seconds. No centre banner, no
minimap: the ground names (one per biome, on an open green) stay.

## Where the fingerposts point

At a biome border a post's arms name the region the road runs into. At a fork
they name the destination down that road: a place with a door, a waystation,
or a way down into a dungeon (`world/signs.rs`). Of what lies down a road the
nearest wins; a way down counts half as far as it lies and a halt a little
farther, so a halt is named only where it is the nearest by a margin. A way
down's arm wears a small stair going down before its name. A post also stands
where the road to each way down leaves the main road (`rules::world::MOUTHS`),
and a door within the ring round a post is named on the arm it lies toward.

## Ways down

Three entrances lead down from the kingdom: the Delve's gate (the Undercroft
and the Winding Stair), the shaft at the Mines' head and the Swamp's plate in
Sir Damp's marsh. `rules::world::MOUTHS` lists them, and a test keeps every
entrance that goes down on the list. Each has its door on the map, reached by
road from the Keep, and a halt within twenty tiles. A sign stands by each door:
a stair going down on a dark board, DOWN under it, and on the Delve's the
party's deepest floor once they have been down.

## Waystations

Eleven halts stand beside the roads (`world/waystations.rs`, drawn by
`render/overworld/arena/waystation.rs`, the keeper's talk in
`rules/waystation.rs`). Each is a lean-to with its keeper and kettle, a coach
at its post, a rally pole and a lantern on a post, seven tiles across and four
deep with the apron before it. Their ground is worked out once from the roads
and the open ground beside them, on the realm with no wishes raised, and
no wish's lot may stand on it (`wishes::lots`); nothing about a halt is solid or
changes the map, so the kingdom's tiles are as they were.

The coaches run the halts in a ring: Harbour, Gatehill, Mineside,
Ashroad, Listsend, Coastgate, Longbow, Homecoming, Midway, Marchgate, Swamp
Lane and back to Harbour (`the ring is the order of `STOPS`). E beside a
keeper and they greet the knight; E again and they ask where to:

- **1** the coach on to the next halt, **2** the coach back to the one before.
  It carries every living knight within ten tiles of its post, together, and sets
  them down side by side on the next halt's apron. A knight left behind has a
  rally called for them at the halt the coach ran to.
- **3** a rally at this halt: every other knight's arrow points here, named
  RALLY, for three minutes or until each of them has come within six tiles. It
  stands down at once if the party is already gathered, and with no one else on
  the road it calls no one.

The rally pole's pennon flies only while a rally stands at its halt, and the
lantern burns only while a rally stands there or a coach has run from or to it
in the last twenty seconds. A puff of dust rises at the post when a coach comes
or goes. The older coach posts by the Keep, the Village and the other doors
keep their single-knight hop.

## Class banners

No banner stands on the road. A class is taken up only at the Sozzled Griffin's
tables, each at its patron's; the lots beside the halls' doors where the stones
stood stay empty, so the kingdom's trinkets and the herald keep their places.

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


## The blank-slate plan

The map is the same size (240 x 176 tiles, ten by eleven chunks of 24 x 16),
so saves, chunk streaming and the wire are untouched. What it holds is the
whole of the plan (`world/plan.rs`, `world::plan::tabula`):

| what | where |
|---|---|
| the ground | open meadow edge to edge, no trees, rock, water, roads or paths; one biome (The March); the 2D tufts crowd on the crests of broad swells and thin in the hollows, the 3D floor takes the same swells as light and shade |
| the Keep | a tiny castle just begun, `8 x 5` tiles at (124, 83) in the middle chunk (5, 5): a half-raised squat tower, a stump of a twin, low wall stubs with a gap and a timber frame for the gate, scaffolding, a hoist, a heap of stone, the Keep's banner. Small enough to see whole (the view is 24 x 14). It has no door and no inside |
| the spawn | the tile before the Keep, (128, 88) (`Place::Keep.stand_world()`, `kingdom::keep_stand()`) |
| the way down | the Delve's gate, `3 x 3` at (139, 84), its stand (140, 87): the only door on the map (`Door::Delve`, `wishes::delve_gate()`, `delve_stand()`) |
| the camps | sixteen, one for each model house in house order, `7 x 4` tiles each: two tents and a fire and the house's banner in its own colours (`House::field`/`charge`, the dusk inks as every banner on the map), the house's knight standing at home on `houses::stand(id)`, its name carved on the ground below. One to a chunk of the ring two chunks out from the Keep's, each set by hand in its chunk (`tabula::CAMPS`), 29 to 59 tiles from the Keep (five to nine seconds on foot), at least 15 from each other. No wall, no gate, no door, nothing inside |
| wishes | stand on the open ground as ever (`world::wishes`), found near the Keep and the Delve's gate first, never on the Keep, a camp, a camp's knight tile and name, or the gate |

`houses::footprint(id)` and `houses::stand(id)` are the one place a house's
site is decided: under the Full plan its castle's wall on the March
(`houses::SITES`), under the blank-slate plan its camp. `castles::castles()`
is the sixteen camps and the Keep (`Castle::is_camp`, `Castle::is_tiny`);
the Dragon Keep is not on the map.

### What came off the map (and is still in the code)

The eleven biomes and their borders; the fourteen authored set pieces
(`map::SET_PIECES`/`AUTHORED`, `STRUCTURES`) and every place in them; the
thirty-seven countryside places (`kingdom::REGIONS`), the kingdom's halls and
Bomrekedan's gate; the roads (`roads::ROADS`), fingerposts, the eleven
waystations, the thirty finds and the set dressing; the College and its
approach; the Squires' Yard; the region names lettered on the ground; the
twelve road walkers, the birds and the chimney smoke; the Dragon Keep and the
March's great castles. Each is behind one question, `plan::tabula()` or
`plan::full()`, asked once at the edge (`map::Realm`, `map::set_pieces`,
`map::structures`, `biome::chunk`, `roads::raster`, `doors::doors`,
`kingdom::geography`, `kingdom::closed_tiles`, `wishes::delve_gate`/`reserved`,
`college::lot`, `signs::posts`, `finds::finds`, `dressing::get`,
`waystations::placed`, `squires_town::on_map`, the scene's authored staging,
`world3d::chunks::builds`). A play that wants the old country back changes
`PLAN`.

### Tests

This crate's own test build keeps the Full plan (`plan::active()`), so the
rooms' and systems' tests that assert the old layout stay green; set
`ANGEL_REALM_PLAN=tabula` to stand the blank-slate plan up for a run. The
plan's own facts (`tests/world/plan__tests.rs`) are checked two ways: the
first group builds the plan directly (`plan::tabula`: one castle, sixteen
camps wholly in their chunks on walkable meadow, the Delve's door reachable
on foot from the spawn, nothing overlapping), the second reads the global map
through its modules and is ignored unless the plan is standing
(`ANGEL_REALM_PLAN=tabula cargo test -p angel-delve --lib world::plan --
--include-ignored`).

### Saves

Old saves load: the kingdom room's map is replaced as before
(`Run::raise_kingdom`); a knight standing where the Keep, a camp or the gate
now stands steps onto the nearest open ground; a return door to a place that
is off the map comes out at the Keep; a wish's lot chosen where the plan now
stands something is chosen again by the realm (`wishes::lots`).

### Nothing points at a place that is gone

What tells a knight to go somewhere asks `Run::on_the_way(room)` first: in the
full plan every place has its door; on the blank slate a place is on the way
only when the home floor's doors and stairs lead to it from the Delve's gate
(`rules/kingdom.rs`). So Maud's gossip, Merlin's errand and the King's herald
never name the Eyrie, a Colosseum arena or the Land Office, and the
herald's and the Foreman's "at the Land Office" lines are not said. A castle is
never raided there (`Holding::raidable`: its raid is fought in its hall, which
is not on the map), so a castle the realm owns keeps its rent and its walls;
its deed, coffer and bays are what they were and its pennon flies over its
camp. An old save's errand or rumour at a place with no door, a coach's rally,
a road traveller and a journal entry on ground that is closed now are let go
when the map is replaced (`forget_what_is_gone`); a return door or a knight on
closed ground comes out on the nearest open ground, as before.

In 3D the look is one ramp a place, so a camp's tents and banner would take the
hillside's green: each camp near the view lies two small zones over its cloth
in a ramp of its house's colour, and the Keep's banner one in the Keep's red
(`world3d::realm_look::camp_zones`, `cloth_ramp`; none in the full plan).

### What lost its way in

Every room, system or game whose only door on the surface was one of the
places above can no longer be walked to from the surface. All of it is intact
and comes back under the Full plan. Found by walking the home floor's real
doors and stairs from the Delve's gate (`world/plan` test
`report_rooms_with_no_way_in_once_the_places_are_off_the_map`): of the 54
rooms the full map had a door to, 49 are still reachable that way (the home
floor's own roads join the Forest, Village, Fairground, Smithy, Market, the
Mines' head and the rest to the Delve's courtyard), and five are not: Labyrinth
College (grounds and, through it, the hall, class, library, lab, tower and
study), the Colosseum's hall and its arena games, the Griffin Eyrie, the Land
Office (the King's court) and the shared realm hall (the Keep's and every
castle's hall: the castle deeds and the Holdings Court). Gates and systems on
the map itself that have no way in: the Round Table adventures (Lanternward,
Moonfall, Bannerpeak, Griffinwing, Lightgallop), Deathmatch and Knight
Racers' kingdom gate (all enter through `roundtable::enter`, which needs the
Dark Forest's door; Knight Racers keeps its Crossroads card), the Forest road's
mouth and the Old Oak, the Squires' Yard, the waystations' coaches and rally,
the fingerposts, the thirty finds, the roads' travellers, the class stones'
lots (classes are still taken at the Sozzled Griffin) and the mission doors at
places that are gone. The Dark Forest's solo Knight Racers entry is one of
these: with the Forest off the map it is reached through the Crossroads card
only.
