# Kiln fx: effects as recipes

`crates/kiln/src/fx`. An effect is a JSON recipe: layers of shapes and emitters, each with its timing
and its ramp. It renders any frame on its own, at any grain, in the realm's inks only. This page is the
reference for whoever writes recipes, model or person (docs/KILN.md is the contract above it).

```sh
cargo run --manifest-path crates/Cargo.toml -p angel-kiln --profile fast --example fx_sheet -- [--out DIR] [NAME ...]
```

That renders every recipe in `crates/kiln/examples/fx/` (and the card and cartridge examples) to
`<name>.gif` (grain 4), `<name>-3x.gif`, `<name>-sheet.png` (every frame, grain 1 drawn four times over
beside grain 4), `<name>-key.png` (the busiest frame at grain 4, three times over) and `catalog.png`, and
prints milliseconds a frame. Look at the sheet, not just the GIF.

## Units

- **Place and size**: game pixels (16 a tile), x right, y down. A layer's `x`, `y` are offsets from the
  recipe's `pivot`. At grain g a game pixel is g picture pixels, so `r1: 8` is 8 pixels at grain 1 and
  32 at grain 4.
- **Time**: frames, at `fps` (12 unless set). A layer plays from `t0` up to but not including `t1`.
- **Angles**: degrees; 0 is right, 90 is down, -90 is up. `spin` is degrees a frame.
- **Heat**: 0 to 1, how hot a pixel is; it picks the ink off the ramp (1 the lightest, the core). Below
  0.06 nothing is drawn; below 0.3 it is a sparse ordered dither of the darkest inks, the halo.

## A recipe

```json
{ "fx": 0, "name": "sword slash", "canvas": [48, 32], "pivot": [18, 18], "fps": 12, "seed": 5,
  "beats": { "anticipation": 2, "action": 2, "follow": 3, "settle": 2 },
  "layers": [
    { "type": "star", "ramp": "steel", "x": -6, "y": -11, "r1": 3, "count": 4, "t0": 0, "t1": "action+1" },
    { "type": "slash", "ramp": "steel", "r1": 16, "th": 6, "angle": -110, "arc": 170, "squash": 0.8, "t0": "action", "t1": "settle" },
    { "type": "particles", "form": "spark", "ramp": "spark", "x": 13, "y": 1, "count": 7, "angle": 0, "spread": 110,
      "speed": [1.5, 3.5], "life": [3, 5], "t0": "action+1", "t1": "end" } ] }
```

| field | meaning |
|---|---|
| `fx` | version, `0` |
| `name` | a label |
| `canvas` | `[w, h]`, game pixels, 1 to 256 |
| `pivot` | `[x, y]` the point it plays at (a hand, an impact, a figure's feet); the canvas centre if left out |
| `fps` | 1 to 60, default 12 |
| `frames` | how many; else the beats' total; else the last layer's end |
| `loop` | `true` for an aura or a brazier: motes from the round before carry over, a layer the length of the loop holds its heat |
| `seed` | every chance in it hangs on this |
| `beats` | `{ "anticipation", "action", "follow", "settle" }`, frames each |
| `outline` | `true`: a one-pixel rim of ink `k` round everything |
| `layers` | 1 to 32, drawn in order |

Unknown fields are refused, and so is a field a layer's type does not take. Every error names its path:
`layers[2].arc: a ring takes no arc (it takes r0 r1 th squash)`.

### Beats and moments

`t0` and `t1` take a frame or a beat's mark: `"anticipation"` (0), `"action"`, `"follow"`, `"settle"`,
`"end"`, each with `+N` or `-N` frames (`"follow+2"`). With no anticipation, `"action"` is frame 0.

Time an effect in beats: the anticipation gathers (a glint, motes drawn in, a charging orb), the action is
short (one or two frames), the follow-through carries on past it (the smear's tail catching up, the ring
running out), the settle lets it go (smoke, embers, the last sparkles). A hold is a layer that keeps still
for two frames at its peak (a heavy hit's flash).

## Every layer

| field | meaning |
|---|---|
| `type` | what it draws (below) |
| `ramp` | a named ramp; or `inks`, keys dark to light (`"BpR@69"`) |
| `z` | `front` (default; over a figure) or `back` (behind it) |
| `mode` | `light` only brightens, `over` covers, `under` goes only on bare ground and stays behind every later layer. Each type has its own default |
| `t0`, `t1` | its window |
| `ease` | `linear in out both back spring bounce`; default `out` (`back` for a glyph, `linear` for emitters, orbs and comets) |
| `x`, `y` | its place, from the pivot |
| `gain` | heat times, 0 to 4 |
| `heat` | `[from, to]` over its window (an emitter's: over each mote's life) |
| `seed` | a layer's own chance, added to the recipe's |

Every shape cools over the last fifth of its window, so nothing stops dead.

## Types

Defaults in brackets.

| type | draws | takes |
|---|---|---|
| `particles` | an emitter (below) | `form count rate life speed angle spread gravity drag size spawn area motion wobble spin squash x1 y1 lift` |
| `puffs` | an emitter of smoke, drawn under | the same but `form` [count 6, life 10-16, speed 0.4-0.9, angle -90, spread 70] |
| `burst` | a fireball's body: rough hot disc swelling, cooling into holes | `r0 r1 squash` [1, 9] |
| `ring` | a ring opening, its thickness falling away | `r0 r1 th squash` [1, 12, 1.4] |
| `shock` | a ring with a dark wake inside, on the floor | `r0 r1 th squash` [2, 16, 1.2, 0.45; back] |
| `flame` | tongues licking up from the place | `count r1 size speed` [5 tongues, half-width 4, height 10] |
| `beam` | from the place to `x1 y1`, half width `th` | `x1 y1 th speed` [32 right, 2] |
| `pillar` | light from `r1` above down to the place, a glow at its foot | `r1 th speed` [30, 3] |
| `slash` | a blade's smear: a crescent of radius `r1` swept from `angle` through `arc`, `th` thick at its leading edge, the tail catching up | `r1 th angle arc squash` [10, 3, -60, 150] |
| `orb` | a glowing ball, pulsing; travels to `x1 y1` | `r0 r1 speed x1 y1 lift` [3, 3] |
| `flash` | a hard bright disc that cools at once | `r0 r1 speed x1 y1 lift` [2, 7] |
| `comet` | a hot head, a tail `size` long behind it; travels | `r1 size angle x1 y1 lift` [2.2, 10] |
| `vortex` | a swirl of `count` arms | `r1 th count spin squash` [8, 2, 4, 12, 0.6] |
| `rays` | rays shooting out with an overshoot and drawing back | `count r0 r1 th angle spin squash` [10, 2, 14, 0.8] |
| `glyph` | a rune circle drawing itself on: rings, a star of `count` points, runes | `r1 count angle spin squash` [10, 5, 0, 3, 0.45] |
| `bolt` | lightning from the place to `x1 y1`: a leader, then the strike, `count` forks, re-struck every `flicker` frames | `x1 y1 count jag flicker th` [24 down, 2, 14% of its length, 2, 1.2] |
| `cracks` | cracks running out over the ground: a dark seam, a lit lip | `count r1 th angle spread squash jag` [6, 12, 0.8, 0, 360, 0.45, 1.5; back] |
| `spikes` | spikes shooting up from a row `r1` each side of the place, breaking at the end | `count r1 th size angle` [5, 8, 1.6 base half-width, 9 tall] |
| `star` | a spiky star popping in | `count r1 th spin` [8, 6] |
| `streaks` | a thrust: a streak `r1` long along `angle`, thick at its head, and `count - 1` speed lines beside it | `count r1 th angle` [5, 18, 1.6] |
| `bubble` | a shield's bubble popping in | `r1 th squash` [7, 0.8] |
| `text` | `text` rising `speed` pixels: digits, `+ - !` and space | `text size speed` ["+1", 1, 6] |
| `tornado` | a striped funnel `size` tall | `r1 size spin` [6, 20, 20] |

### Emitters

`count` motes are born at `t0`, then `rate` a frame until `t1`. Each mote has its own share of every
range: `life` frames [6-10], `speed` game pixels a frame [1.5-3], a direction within `spread` degrees
[360] of `angle`. It is slowed by `drag` [0.15 a frame], pulled by `gravity` [0; 0.1 for shards,
droplets, coins and petals], squashed by `squash`, and changes `size` and `heat` over its life (pairs are
`[from, to]`). With `x1 y1` its birthplace travels there over the window, arcing by `lift`, so a trail
follows a shot.

- `form`: `spark ember mote droplet shard flake bubble note star petal coin heart worm smoke fly`
- `spawn` (about the place, sized by `area` `[w, h]`): `point`; `line` (level, `w` long); `ring`
  (thrown out from the middle); `disc`; `box` (a figure's box)
- `motion`: `straight`; `wobble` (side to side `wobble` pixels); `spiral`, `curl` (turning `spin`);
  `zigzag`; `bounce`; `rise`, `fall` (gravity); `gather` (drawn in from `area` to the place: a wind-up);
  `orbit` (round the place on `area`)
- An overshooting `ease` (`back spring bounce`) runs each mote's clock past its mark and home.

## Ramps

Dark to light; the last ink is the hot core. The cards' palettes, with the game's inks:

`fire ember frost holy gold arcane genie storm poison nature blood rose poop brew void water bone smoke earth`

and `steel` (a blade, `xSuUvVW`), `spark` (a hit, `pRo@569`), `char` (soot and cracks, `kKgjG`).

## At every grain

Shapes are fields of heat in game pixels, sampled at each picture pixel's centre, so one recipe draws at
every grain. The thinnest stroke is held to half a picture pixel and a little, so it still lays a connected
line at grain 1. Some detail comes in only where there are pixels for it: a rune circle is its rings alone
when it stands under 4 pixels tall, gains its star at 6 and its runes at 8, and a note or a heart at
grain 1 is the game's own mark.

## Craft that the catalog taught

- **Smears show the frame's motion.** A slash's leading edge, a thrust's head and a bolt's leader are
  where they are at the frame's *end*, so the first action frame is never blank.
- **No flat discs.** A `flash` bigger than a few pixels reads as a ball. For an impact use a `star`
  for a frame or two, with a small `flash` under it.
- **Give a burst room.** Motes thrown at speed 4 and up cross 20 pixels in a few frames.
- **Under for smoke and dust.** It sits behind fire whatever order the layers are in.
- **Spread before you crowd.** At grain 1, motes crowded together read as one blob: throw them faster,
  or use fewer.

## The card bridge

`fx::from_card(&["vfx trail worms poop wobble count=3 wobble=2"])` reads a card's `vfx` lines
(`crates/delve/src/rules/vfx.rs`; the reading is written again in kiln, the same words, defaults and
bounds) and gives a recipe and notes. A line that does not read is set aside with a note, never the card.
It plays on a 64 by 32 stage at 15 frames a second (two of the game's ticks a frame): `cast` about the
caster, `fire` at the hand, a shot crossing as a small comet with `trail` behind it, `hit` where it lands
(thrown back toward the shooter), `kill` a frame later (thrown up), `aura` about the caster all along.
Card numbers carry over at the game's scale: `speed` half pixels a tick, `life` ticks, the game's drag and
weights.

## The cartridge importer

`fx::from_cartridge_v1(json)` reads the VFX cartridge's v1 recipes (`~/.angelX/cartridges/vfx`). The
two share the type names and most of the field names. v1 draws at one native size with its shorter side
spanning -1 to 1 and times running 0 to 1; kiln reads a native pixel as a quarter of a game pixel (so the
effect is its own size at grain 4) and times as frames. Each v1 ramp is snapped to the nearest realm
ramp (`ice` is the realm's `genie`); `palette`, `dither` and `clean` are set aside with a note; a v1 slash
becomes kiln's sweeping one. v1's particle `mode`s map to kiln's spawns and motions: `rise`/`fall` a line
and gravity, `ambient` a box, `implode` a gather, `orbit` an orbit, `trail` a box thrown back, `swirl` a
spiral.
