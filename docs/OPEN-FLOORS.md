# Open floors

The room game walks the party through a door, bars it, and fights what is
in the room. An open floor is one map to explore: chambers joined by
passages, companies waiting where they live, fights where the party meets
them. Now and then an area locks (the guardian's chamber); most of the time
the way is open.

## The switch

- `fortune::Mode::OpenFloors` ("Open Floors"): a wedge on Dame Fortune's
  wheel, from the first rung.
- `Run::open_floors`: every floor of the delve is open, whatever the wheel
  said. A host sets it from `ANGEL_DELVE_OPEN=1`
  (`settings::Settings::open_floors`, read by the cockpit and the dev
  host). `Run::new` copies it in, and `Run::wants_open` also reads the
  host's setting, so a save made before the switch was thrown opens its
  next floor too.

With neither, nothing changes: the room game is the room game.

## The map (phase A)

`layout::open_floor(depth, pack, party, rng)` builds the floor:

- **One room**, `RoomKind::Depths`, up to `OPEN_MAX_COLS` x `OPEN_MAX_ROWS`
  (168 x 108) tiles. Only that kind may pass the 48 x 28 limit
  (`Room::valid_snapshot`). Camera, collision, tiles and the stage frames
  work as they do for a great hall.
- **Cells** on a grid sized for the party: the pitch is the guardian's
  chamber plus 8 tiles of rock (4x3 cells for one or two knights, 3x3 for
  three or four). The cells grow as the lower tunnels grow
  (`world::routing::TunnelVariant`: Warrens on the first floors, then
  Gallery, Warrens, Karst by depth), `8 + depth` of them.
- **Passages**: a depth-first maze through the cells (the way winds), then
  a few loops (10/25/45% of the spare neighbours by variant, at most a
  quarter of the cells). Each is 3 or 4 tiles wide, with a dogleg through
  the rock between the two chambers. It is carved only through rock, so
  chamber walls stay walls.
- **Chambers** are stamped from the chamber library through `furnish` (the
  rest of `room()`): pack templates and `rooms::small` at one screen, the
  dungeon masters' `rooms::great` quadrants (`AUTHORED_SHARE`) or the
  pillared grid at the party's sizes (`party_room`). A chamber's company is
  the room's (`foes` + the critter), laid for one knight's room.
- **Kinds.** The arrival (`Start`) is a bottom-row cell and is quiet. The
  farthest cell along the passages is the guardian's (`Stairs`, or `Lair`
  on floor 3). Up to two 3-way junctions are crossings (`Hall`, a few
  bats). The rest are `Fight`.
- `Floor::open: Option<Open>` records the map's index, each `Chamber`
  (rect, kind, roster, dist, cleared, sealed, seen, stirred, gates), the
  links, the side doors, the guardian and the arrival point. Every field
  is `serde(default)`.
- **Walls read as walls.** Everything that is not chamber or passage is
  `Tile::Wall` rock. Rock that touches no floor is drawn as black void, and
  rock that faces a floor gets the pack's wall face. Blocks stay inside
  chambers as the templates put them.
- **No pockets.** Any floor tile not reachable from the arrival is turned
  to stone.

## Side rooms

They stay rooms, each behind a two-tile door (`SideDoor`) in a chamber wall
that has no passage, on a cell no chamber holds:

- the stair's **Sanctuary** below floor 1 (room 0; the party arrives in it
  and steps out onto the map)
- the **Treasure** vault, off the farthest dead end
- the **Ledge** on floor 1, the **Pit** on about half of floors 2-5, the
  **Wanted** hall on every floor
- the **Secret** vault, now and then, behind a cracked wall that a bomb
  near it opens
- the **Threshold** off the last chamber on floor 6

Nooks, the forge hall and the steward's hall hang off those side rooms by
their usual cell rules. Stepping on a side door takes the whole party
through it, as a room's door always has; the map's companies wait where
they were (`Run::parked`, never sent to a client) until the party is back. Stepping on a side door enters its room. Leaving
the room puts the party back on the map at that door (`Floor::neighbour`
knows both ways).

## Minions in place

- `Run::populate_open` runs on the first walk onto the map. Each chamber's
  roster spawns at its own floor spots, **dormant**
  (`Enemy::dormant`, `Enemy::home` = the chamber). It adds party extras
  `(EXTRA_FOES[d] + EXTRA_PER_KNIGHT * (knights - 1)) * ramp / 100`, where
  the ramp grows with the chamber's distance from the arrival
  (`RAMP_FIRST` to 100). Elites are rolled as `deepen` rolls them. The
  guardian stands alone on floors 1-3 with its escort's hoard
  (`BOSS_ALONE_TO`); below that it keeps two escorts. Health and damage
  are untouched.
- **Caps.** `OPEN_FOES` (96) on the floor, extras trimmed first.
  `AWAKE_CAP` (32) awake at once; the rest wait.
- **Waking.** A dormant company stirs as one when a knight steps into its
  chamber, comes within `WAKE_REACH` of a member with a clear line of
  sight, or hurts one. A woken foe gets a half telegraph, then fights.
- **Leash.** A foe never moves farther than `LEASH` tiles outside its
  chamber's stone (the step is undone). A company with no knight within
  `SLEEP_REACH` dozes off again.
- Anything spawned mid-fight (summons, splits, mimics) takes the nearest
  chamber as home and is awake.

## Fights are local

- On the map the phase stays `Exploring`. `Run::fighting()` (any awake foe
  on the map, or `Phase::Fighting`) stands in for the old phase test in
  stakes, vigil, ult charge and runes.
- Room-wide damage (a knight's bomb, the Thunder Scroll's nova) reaches
  `OPEN_BLAST` units on the map, not the whole floor.
- **The guardian's lock-in.** The guardian's chamber's mouths are door
  tiles (`Chamber::gates`). When a knight steps inside with the guardian
  alive, the chamber seals (`barred()` is true on the map): the rest of
  the party is drawn in beside them, as a room entry would bring them, and
  the company wakes. It opens when the company is dead. The stairs in its
  middle are shut until then.
- **A chamber clear pays once** (`Chamber::cleared`): the monsters' shots
  near it turn to coins, plus the barony mark, the bond for two standing,
  flawless/room_clear, `door_open` and the wish window. The guardian's
  chamber adds the floor-1 hallowing, or the dragon's ending on floor 3
  (stairs, the light home, the hell gate, at the chamber).

## The map on screen

The floor map UI is built on its own branch (play/dungeon-map: `Run::floor_map`
gives a `FloorMap`, a corner minimap and a full map on a key). Open floors
build no map UI of their own. They expose what that model needs:

- each `Chamber`'s `rect` (map tiles), `kind`, `seen` (fog of war per
  chamber: a knight has stood in it), `cleared` and `sealed`
- `Open::links` (the passages, chamber to chamber) and `Open::doors` (the
  side doors, each with its room, chamber and wall)
- `Open::guardian` and `Open::arrival`

The two meet in phase B.

## Cost (release build, 4 knights, floor 5, 168 x 108 tiles)

- A tick: 0.04 ms with 93 foes raised and 32 awake, 165 shots in flight.
- The wire: `publish` and its diff take 0.7 ms a tick and send 1.5 KB a
  tick on average (9.7 KB at most). The first snapshot is 131 KB.
- A seat's frame in parts: 1.5 ms. It was 29 ms before great rooms were
  taken apart in a window round the view (`Img::window`). As one picture
  it is 0.45 ms.
- The map's scenery is baked whole in 110 ms, once on arrival and again
  when it changes (a seal, a chest, the vault). It is not redrawn at every
  torch step; a window lights its own fires.

## What each coupled system does on an open floor (phase A)

| System | Where | On the open map |
|---|---|---|
| `barred()` / `foes_left()` | rules.rs | Adapted: barred is true only while the guardian's chamber is sealed |
| `enter()` reset + roster spawn | rules.rs | Adapted: leaving the map parks its foes (`Run::parked`), coming back restores them, first walk populates |
| `enter()` phase/cues/placement | rules.rs | Adapted: phase `Exploring`; the party lands at the door it came out of, else the arrival |
| `spawn()` / `deepen` | rules.rs, stakes.rs | Adapted: spawns land in the party's chamber; extras come at population |
| `boss_anchor` / `spawn_boss` | rules.rs | Adapted: the guardian stands at its chamber's head |
| Room clear block | rules.rs | Adapted: per chamber (`chamber_clear`); Lair ending at the chamber |
| `travel()` / `descend()` | rules.rs | Adapted: side doors, and stairs shut while the guardian lives |
| `floor_progress` / `ramp` | stakes.rs | Adapted: the party's chamber's distance |
| `stakes()`, vigil, ult charge, runes gate | stakes.rs, rules.rs, ults.rs | Adapted: `fighting()` |
| Bomb / nova room-wide | rules.rs | Adapted: `OPEN_BLAST` radius |
| `blast_wall` / crack / `valid_secret` | secrets.rs, layout.rs | Adapted: crack in a chamber wall, a bomb near it |
| `MAX_ITEMS`, `valid_snapshot` sizes | rules.rs, layout.rs | Adapted: Depths up to 168x108, 96 items |
| Gear cache chest (floors 1-2) | gear.rs | Adapted: in the arrival chamber |
| Mimic wake | hazards.rs | Adapted: no phase change on the map |
| Hireling revive | hireling.rs | Adapted: revives when nothing near is awake |
| `beyond` edge arrows | arena.rs | Adapted: awake foes only |
| Scenery, `wall_tiles`, `room_fixtures` | arena.rs | Adapted: deep rock is void, torches at chamber corners and doors, portcullis on sealed gates |
| art_v2 | art_v2.rs | Disabled on the map (v1 draws it) |
| Boss gates / camps / economy / boss key | boss_gates.rs, boss_ecology.rs, keyring.rs | Disabled (`boss_gates = None`), phase B |
| Folly / gantry / hop / powder / race3d | folly.rs, gantry.rs, ... | Disabled, phase B |
| Merchant camp | merchant.rs | Works as is: it pitches in the Treasure vault or the Sanctuary, both side rooms |
| Waves, traps, perils, `arm_room`, `place_rune` | hazards.rs, perils.rs, runes.rs | Disabled on the map, phase B |
| Thief goblin, frogs, swarm_up | thief.rs, frogs.rs, critters.rs | Disabled on the map (kind-gated), phase B |
| Mimic Fair chest per clear (`fair_chest`) | fortune.rs | Disabled on the map, phase B |
| Side rooms (Sanctuary, Treasure, Pit, Wanted, Ledge, Secret, nooks, forge, holding, Threshold) | layout/open.rs, nooks.rs, barony.rs, holdings.rs | Work as is, as rooms |
| Cage (rescues), cat, spectacle, presence hold | rescues.rs, cat.rs, spectacle.rs, presence.rs | Work as is |
| Dares, bounties, feats, audience | dares.rs, bounties.rs, feats.rs | Work as is from the per-chamber cues |
| `way_on` guide arrow | guide.rs | Hidden on the map, phase B |
| Wire publish | wire.rs | Works as is; the map's tiles go once and diff after |

## Phase B

- Boss-gate camps and the gate economy on open floors: camps as chambers,
  leaders and the boss key, the route lock.
- The mini-game rooms (folly, gantry, hop, powder, race3d) as side rooms
  or chambers.
- Merchant camps in a quiet chamber, not only in the side rooms.
- Wire the chambers into play/dungeon-map's `FloorMap` (nodes = chambers
  and side rooms, links = passages and doors, state from `seen`, `cleared`
  and `sealed`), and draw it with that branch's minimap and full map.
- Waves, traps, perils and runes per chamber. Thief goblins and frogs on
  the map.
- An ambush chamber per floor (it seals while its company lives).
- Mimic Fair's chests per chamber (one map has one `chest` today).
- Patrols in the passages, and companies walking home when they doze off.
- The `way_on` arrow toward the nearest unseen chamber.
- A floor-1 reforge altar after the guardian (floor 2's Sanctuary has one
  now).
- A windowed scenery bake for the map, so arriving and a seal don't
  re-light the whole floor (110 ms), and the torches can step again.
- Fortune's floor-shaping wedges on open floors: Hold the Stair, Gauntlet
  and Hollow Walls lay rooms today, and Open Floors takes precedence.
- The cockpit's sidebar floor map (`shooter_viz::floor_map`) shows the map
  as one cell until the FloorMap wiring lands.
