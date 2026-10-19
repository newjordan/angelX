# The world's rungs

The direction (2026-10-18): the game starts as pixel and, as things upgrade, changes dimension and ends in 3D; the pixel art goes from hard-to-discern to full images along the same climb. So every look the Delve has is a rung of one world, not a rival: the flat pixel Delve, the tilted sprite cards, the lit rooms of the monolith (docs/REALM-MONOLITH.md, "Rooms, lit"), and what is above them. Nobody had written down the rungs between the flat Delve and the monolith. This does, and proves each with the same room shot at every rung (`delve_tiers`).

## What the engine gives

A look is a few dials. These are the ones that exist today and what they do to a room.

| dial | where | range | in a room |
|---|---|---|---|
| tilt | `angel_stage::Tilt`, `Frame::tilt` | 0 (flat) to `MAX_TILT` 0.6; the game's is 0.3 (`stage_frame::TILT`, pinned to `set::GAME_TILT`) | the floor foreshortens, the far wall shows above where a flat view ends in floor, what stands is placed on its foot and never tipped |
| world | `world3d::set_enabled`, `rooms_lit::on` | still art, lit blocks, painted plate | `ANGEL_WORLD3D` off: the room's still art. On: lit blocks (the default). On with `ANGEL_WORLD3D_PLATES=1`: its item's painted plate cast on blocks, where it has one |
| grain | the stage's size | the Delve's is 384 x 224 | a stage pixel is an art pixel (below) |
| ramp | `Look::ramp` | 2 to 16 entries | colours: each pixel is placed between two entries by its brightness |
| dither | `Look::dither` | `Bayer4`, `Bayer8`, `None` | the weave between two entries: 16 levels, 64, or bands |
| sun | `Look::sun` | direction, strength, ambient, `rise`, `Shadow` | the key light, how dark the unlit sides go, whether a face pales toward its top, whether it casts |
| lamps, warm | `Look::lamps`, `warm` | up to `LAMP_MAX` (16) | pools of light that move what they light toward the ember ramp: light = activity |
| accents | `Look::accent` | up to 4 | colours never dithered: flame, window, glint, the lava crust |
| fog | `Look::fog` | an entry of the ramp, a start and an end | distance fades to that entry; it takes the warm out of what it covers (below) |

Two things the engine does not give, found by building the rungs:

- **`Look::pixel_rows` is not a dial for a room.** The GPU sets it to the stage's own rows (`crates/gpu/src/stage.rs`, "look.pixel_rows = stage_size.1.max(16)") so a stage pixel is an art pixel. The grain of a lit room is the stage's size, and the window's is fixed at 384 x 224. A finer lit room needs a bigger stage with the world made bigger with it. `delve_tiers` does that after the frame is made (`regrain`: every root node's translation and scale, the look's distances, the layers); the stage's camera takes its size from the stage (`Tilt::for_stage`: "any scale of it gives the same picture"), so nothing else moves. The engine has no such setting.
- **The second art kit cannot stand a room in 3D.** `ANGEL_ART=v2` draws 2D frames at 768 x 448 (`art_v2::size`), but a room the kit supports is not "layered" (`room_scene::layered`: "the second art kit ... stay one picture for now"), so it has no world. Shot with the kit on, the Crypt's fight room came back as one picture. So the kit that has finer art and the path that has depth do not yet meet.

Fog, measured: the stage camera's eye sits a long way back, so a fog from 250 to 900 pixels swallows the whole room and one from 800 to 2000 hardly shows. Where it bites it also drains the warm out of the lamps (`warm * (1 - fog)`), which is the opposite of what the top rung wants, so it is left off (`--set r4.fog=500,1500` shows it).

## The rungs

Shot in the Crypt: a fight room, the stairs passage, the bone chapel, an open floor. Each rung is the same room, the same camera and the same knights, with these dials moved.

| | R0 flat pixel | R1 tilted cards | R2 first light | R3 the monolith | R4 image-like | (R5 painted plate) |
|---|---|---|---|---|---|---|
| world | still art | still art | lit blocks | lit blocks | lit blocks | plate on blocks |
| tilt | 0 | 0.3 | 0.3 | 0.3 | 0.3 | 0.3 |
| stage | 384 x 224, art at half grain (192 x 112) | 384 x 224 | 384 x 224 | 384 x 224 | 768 x 448 | 384 x 224, drawn at the window's density |
| colours | the art's own | the art's own | 4 of the tone's, in a hue half the still art's | 8, the tone's ramp | 16, the tone's ramp mixed up | true colour |
| dither | none | none | `Bayer4` | `Bayer4` | `Bayer8` | none |
| sun | baked in the art | baked in the art | strength 1, ambient 0.25 (2.5 x the room's), shadows | the room's key, shadows | ambient 1.6 x, `rise` 90 px, shadows | painted in |
| lamps | baked | baked | none | the room's, burning as it is fought, the ember ramp | 1.15 x as strong, 1.3 x as far | painted in |
| figures | sprites standing on their feet | the same | cards in the world, hidden by what stands before them | the same | the same, a sprite pixel two stage pixels | the same |

What each step is, and how it reads in the shots:

- **R0 to R1, the floor tips and the art is fine.** Tilt 0 to 0.3, and the art's grain doubles. R0 is the old top-down Delve: the view is the room's 24 x 14 tiles, nothing past its edge. R1 is what the window shows with rooms in 3D off: the floor foreshortened, the far wall shown above the view's top, knights and foes on their feet. R0's art is a stand-in (below).
- **R1 to R2, the dimension changes.** The room stands up in blocks (`world3d::rooms_lit`), walls hide knights, one low sun throws shadows across the floor, and everything is four colours with a dither between them. The look is that of a four-colour dithered 3D game: it reads as pixel art that has been given depth. The still art's hue is carried a half of the way (`still_ramp`: the four brightness quarters of R1's own pixels), so the step is one of shape and light before it is one of colour.
- **R2 to R3, colour arrives with the light.** Eight colours, the tone's own hue (the Crypt's indigo), the lamps lit: the braziers and the gate throw ember light across the floor, and the room is dark where nothing burns. Light = activity.
- **R3 to R4, the grain halves and the ramp doubles.** The stage is 768 x 448, sixteen colours between a finer weave, ambient up, the light pale toward the top of a wall, the lamps bigger. The dither is nearly gone. It reads as the same room seen closer, in better light. The knights stay sprites (a sprite pixel is two stage pixels, so they look chunkier against the world), as in the target frame (`look-reference.png`, a fine dither with a pixel knight).
- **R5 past the climb: the painted plate.** The room's item's plate (WORLD-3D.md) cast on its blocks, true colour, no dither, drawn at the window's density: the full image. It is the one rung that has surface detail (carved wall panels, a flagged floor, tombs) and so the one that finishes the thought "pixel art goes to full images".

The step between R1 and R4 is a loss of surface, not a gain. The lit look has solid tones and no textures by design ("No textures, no plates", REALM-MONOLITH.md): R2 to R4 have less on a floor than R1 does. What they gain is depth and light. R5 shows what the climb's end could hold; the lit rungs do not lead to it yet.

## Built, and what would have to be

| rung | built | not built |
|---|---|---|
| R0 | tilt 0 (a frame can ask for it) and the still art | a tilt of 0 in the game (`StageMaker::make` always says 0.3) and art at half the grain. The proof takes R1's frame and sets each 2 x 2 square to its commonest colour (`chunked`), which keeps the palette; a drawn 192 x 112 art set would be cleaner |
| R1 | all of it: the window's frame with rooms in 3D off | |
| R2 | the lit blocks, the sun, shadows and dither | a 4-colour look as a setting. The proof makes it from the room's look (`dial`: four entries picked along the room's eight, their hue mixed with R1's) in the example. The hue carry would need a table of each place's still colours, or a read of the room's art, in the game |
| R3 | all of it, on this branch | |
| R4 | the look's dials (16 entries, `Bayer8`, `rise`) | the grain. The stage cannot be made finer by a setting and the second kit cannot stand a room. Proven by scaling the made frame in the example. The figures are not finer: their sprites are the Delve's own |
| R5 | `Look::Painted`, the plates, patches, `ANGEL_WORLD3D_PLATES=1` | a plate for most rooms: 86 plates covered 17% (a party of one) to 24% (two) of seeded rooms, and a plate pack is git-ignored (`assets::plate_pack`). This checkout has none; the proof reads the tabled one at `/work/artifacts/3d-world-tabled-20261018/assets/w3d-plate-pack` |

And one thing that is none of the rungs': **a switch that moves them together.** Today the pieces are set apart: tilt is a constant, rooms in 3D by `ANGEL_WORLD3D`, plates by `ANGEL_WORLD3D_PLATES`, the second kit by `ANGEL_ART`, and the look is made by the room (`rooms_lit_look::look`). A rung would be one value that chooses them all.

## How plates and cards are chosen (what would make the art finer)

Written down, not built.

- **A figure is a card.** Each `Figure` (the knight, a foe: its sprites composed to one picture at the art's own grain) becomes a picture of its own and a quad standing on its foot (`stage_frame::cards_of`, `card`), drawn nearest with depth. Nothing about it follows the look.
- **A hook for a finer drawing exists and is not fed.** `StageMaker::make` takes `hd: FnMut(&Body) -> Option<u32>`: for a sprite (`Body`, named by `look_key`, a hash of its pixels) the window may have an HD picture, and it is drawn over the card in the sprite's place with the sprite's own pixels taken out from under it (`with_hd`; the test `an_hd_drawing_takes_its_sprite_out_from_under_it`). The Delve's client passes `|_| None` (`client.rs`), and so does every proof. A bank of drawings keyed by `look_key` would be R4's finer knights.
- **The second kit has two grains of art, picked by the host.** Dungeon art is delivered at 384 x 224 and 768 x 448 ("independently BOX sampled", DUNGEON-ART.md; `dungeon_art::bank(pack, scale)` with `scale` 2 when `art_v2::enabled()`), the slot banks keep `v1` and `v2` folders (`crates/delve/assets/art-slots/<slot>/`), and the second kit's knights are its own pose studies or the first kit's drawing brought up three times with its own light (`art_v2/figures.rs`, `scale3x`). It is chosen per host and seat (`settings::Settings::art_v2`, `ANGEL_ART=v2`, a flag in the wire's params), not per rung, and it is the other composition path: one picture, not parts.
- **The painted plate is the finest art the world has.** An item is a set of blocks and a plate (1536 x 1024 for a room's 768 x 448 camera) painted over its capture and projected back (WORLD-3D.md). It is the room as an image. The monolith direction replaced it as the default for rooms (REALM-MONOLITH.md). Plates cover few rooms, and the theme's patches that dress the rest "read darker and muddier than the flat art" (WORLD-3D.md, "Rooms with no item"; `world3d::plain`, off by default).
- **To make the art a ladder**, three things: the figure sprites at more than one fidelity, chosen by a rung (a coarse set for R0, the Delve's for R1 to R3, the second kit's drawings for R4 through the `hd` hook); the lit surfaces given some detail between R3 and R5 (the painted path's patch a kind, `Material::patch`, is the nearest thing there is); and the stage's grain made a setting, so the second kit's art and the lit world can be drawn together.

## Proof

`crates/glass/examples/delve_tiers.rs` makes each rung's frame as the window does (`StageMaker::make`, the GPU core offscreen) and moves only that rung's dials after: `frame.tilt`; `regrain` for the stage's size; `dial` for the look's ramp, dither, sun, lamps and rise; `chunked` for R0. The engine is as it was (no `pub` added; the example uses `arena::in_seat` and `scene_for`, as `delve_rooms` does with `seat_scene`, to shoot the room's view without the party's band and the HUD's).

```
cargo build --release -p angel-glass --example delve_tiers
$CARGO_TARGET_DIR/release/examples/delve_tiers <dir> [--scale N] [--case NAME]... [--rung RN]... [--open SEED] [--set rung.dial=value]...
ANGEL_WORLD3D_ASSETS=<pack of plates> ANGEL_WORLD3D_PLATES=1 \
  $CARGO_TARGET_DIR/release/examples/delve_tiers <dir> --case crypt-bone-chapel --rung r5
python3 scripts/tiers-sheet.py <dir> [--cell 520] [--detail x,y,w,h]
```

- It shoots `crypt-fight`, `crypt-passage`, `crypt-bone-chapel` and `crypt-open-floor` (seed 41, the one of eight seeds tried that shows a wall and a name over its floor), `<case>-r0.png` to `-r4.png` at `N` window pixels to a Delve pixel (4: 1536 x 896). R5 is a second run because the painted path is chosen once for the process.
- `--set` moves any dial of any rung (`r2.dither=none`, `r4.grain=3`, `r3.lamps=1.5,1`, `r4.fog=500,1500`); it is how the rungs' values were found.
- `scripts/tiers-sheet.py` writes `contact.png` (a row a room, a column a rung) and `detail.png` (one patch of every shot as drawn, for the grain).
- Shots of 2026-10-18: `/work/artifacts/monolith-private/tiers/` (`contact.png`, `detail.png`, every `<case>-r<n>.png`).

What the shots show (compared with the target frame, `/work/artifacts/delve-handoff-20261018/look-reference.png`):

- Every rung is a distinct step in all four rooms. The climb is clearest in the fight room and the chapel: pixel art, tipped, given depth and four colours, given colour and ember light, made fine.
- R4 is nearer the target frame than R3 is in grain: the target's weave is finer than R3's, with a pixel knight in it, as R4's is. R4's dither is almost invisible at sixteen entries. With eight entries and `Bayer4` at this grain (`--set r4.ramp=8 --set r4.dither=4`) the weave shows as it does in the target; sixteen was kept for the "image" the rung is meant to be. With `dither=none` at sixteen entries the floor bands.
- Weak steps. R1 to R2 is the largest and the least continuous: the floor's stone, the crates and the walls' carving become plain blocks and tones, and the hue carry softens that only a little. R2 itself is the least handsome rung (grey-violet, dull against R3), and tried without dither, or without shadows, it was worse (a four-tone flat shade where floor and wall land in one band). R3 to R4 is subtle in a still. R0 is a stand-in. The open floor is the weakest room: its first screen is a wide empty hall, dark from R3 on.
- What was not checked: the frame cost of R4's 768 x 448 lit pass (four times R3's pixels); the rungs in the browser; any room but the Crypt's; the rungs in motion.
