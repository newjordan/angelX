# Bomrekedan kingdom data (imported)

Frank's Dwarf Fortress fortress, read tile by tile from the live save by
`tools/angelx-kingdom-export.lua` and turned into game data by the
`angel-df-import` crate (`crates/df-import`). Everything here and in
`crates/delve/assets/bomrekedan/` is generated: edit the importer, not the
files.

## Regenerate

From the repository root, with the raw export in
`expansions/bomrekedan/df-export/` (gitignored):

```
cargo run -p angel-df-import --release -- expansions/bomrekedan/df-export expansions/bomrekedan/kingdom crates/delve/assets/bomrekedan
```

Where Windows application control refuses to start a freshly built binary,
the same run is a test harness away:
`cargo test -p angel-df-import --release -- --ignored regenerate`.
A re-run on the same export is byte-identical (`rerun_is_byte_identical`).

## Files

The game includes these (release builds ship only the crate's assets):

| File | What |
|---|---|
| `crates/delve/assets/bomrekedan/levels/<slug>.txt` | one map per playable level, all the same size and origin |
| `crates/delve/assets/bomrekedan/kingdom.json` | source, crop, levels (links, entrance, counts), places, bedroom blocks, seams, artifacts |
| `crates/delve/assets/bomrekedan/people.json` | every living citizen, resident and visitor: skills, home job, district, where they stand |

Kept here, for design:

| File | What |
|---|---|
| `races.json` | the fort's peoples, the world's civilizations by race, the notable and modded races with their history in the fort, all intelligent race names |
| `resources.json` | mineable wall classes per level and overall, map-wide prospect, stocks in the game's spoil words, top 40 stock lines by value |

## Levels

The surface entrance (the level with the trade depot) and the seven levels
with the most fortress content (a weighted count of rooms, workshops,
furnishings) are kept; the rest are listed in `kingdom.json` ->
`dropped_levels`. The forges and the barracks are always kept as well (the
steel line and the soldiers need a level), and the crop grows to hold them.
Each level is named for its dominant function. A link whose two ends are
not joined by an unbroken DF shaft is marked `deep_stair: true` (none are,
at present).

All levels are cropped to one rectangle (117 x 100), so the fort's main staircase is at
the same x,y on every level. Its border row and column are wall. Coordinates
in every file are level coordinates: `df_x = ORIGIN.x + x`.

```
LEVEL <slug>
NAME <Display Name>
Z <df z>
SIZE <w> <h>
ORIGIN <df x0> <df y0>
TILES   h rows
DRESS   h rows
ROCK    h rows
```

**TILES**: `.` floor, `#` wall, `B` blocking thing (statue, well, machine,
tree trunk, a workshop's back row and middle), `~` hazard (deep water, magma,
open air, pit), `D` door, `>` a link (see below). Beds, tables, chairs,
cabinets, boxes and the like stand on `.`. Unrevealed rock is `#`.

**DRESS** (what stands there): `b` bed, `t` table, `c` chair, `s` statue,
`k` cabinet, `K` bookcase, `h` box/chest/cage, `a` armor stand, `r` weapon
rack, `W` workshop or trade depot, `F` furnace, `f` farm plot, `p`
stockpile, `P` screw pump, `g` gear/axle, `m` windmill, `w` well, `o` altar,
`C` coffin/slab, `d` display case, `n` nest box, `T` tree, `"` shrub, `=`
hatch/grate/bridge, `x` trap/chain, `A` archery target, `j` traction bench,
`+` unfinished construction.

**ROCK** (what a natural wall is made of): ` ` not a natural wall, `s` soil,
`r` stone, `f` flux, `i` iron ore, `c` copper ore, `t` tin, `g` gold, `v`
silver, `P` platinum, `n` nickel/zinc/lead/bismuth/aluminum/cobalt, `k` coal,
`j` gem, `J` precious gem (diamond, emerald, ruby, sapphire, star gems,
opals), `x` reagent/industrial (gypsum, saltpeter, kaolinite, microcline,
...), `a` adamantine, `o` obsidian. Gems and ores are as DFHack's prospect
report lists them. Unrevealed rock is the level's commonest of soil, stone
and flux: a guess.

**Links.** Only the game's links are `>`; every DF stair and ramp is plain
floor. Each pair of neighbouring kept levels has one link, a rect on each
level (`kingdom.json` -> `levels[].links[]`: `to`, `direction`, `at`,
`arrive`). Where the two levels share a DF staircase at the same x,y the rect
is a part of it and the same on both levels; the main 5x5 stair is split in
two halves, alternating, so a level's up and down links are distinct rects.
`arrive` is the nearest floor tile with five plain floor tiles in a row
centred on it. The top level also has `entrance` (a clear 3x3 by the trade
depot, also `>`): where a party comes in from the upper world.

Every walkable tile of a level can be walked to from its links: what could
not (outdoor slopes of the lower levels, cut-off pockets) is walled, counted
in `walled_pockets`.

**The surface.** The fortress stands in rounded hills. On the surface level
only, open sky and canopy show the ground below them (up to 8 levels down),
so the hillside is one walkable map; elsewhere open air is `~`. Tree trunks
are `B`; branches and leaves are not drawn.

## In the Delve

A home that already used the old 128 room slots still receives the ten
levels. Slots above that are the fort's. What does not fit, and a saved
level that already holds goods, is named on the floor (`fort_admit`) rather
than dropped without a word. Untouched saved ground is brought up to the
import. A level with goods, a chest, a roster or a door is left as the save
had it.

Underground workshops are drawn without the outdoor roof: a lintel, a beam
and a lamp. The 3D blockout reads the same DRESS grid (a furnace, a pier, a
tree) instead of one boulder over the level.

## Known approximations

- A workshop's solid tiles are its back (top) row and middle, not DF's exact
  per-workshop layout.
- A temple dedicated to a religion (rather than a deity) has no `deity`; it
  has `religion` (the religion's English name, e.g. The Strong Cult),
  `religion_native` and `religion_entity` (its DF entity id) instead.
- People on dropped levels have `level: null` and their DF position.
