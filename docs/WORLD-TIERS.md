# The world's rungs

The direction (2026-10-18): the game starts as pixel and, as things upgrade, changes dimension and ends in 3D; the pixel art goes from hard-to-discern to full images along the same climb. So every look the Delve has is a rung of one world, not a rival: the flat pixel Delve, the tilted sprite cards, the lit rooms of the monolith (docs/REALM-MONOLITH.md, "Rooms, lit"), and what is above them. Nobody had written down the rungs between the flat Delve and the monolith. This does, and proves each with the same room shot at every rung (`delve_tiers`).

## What the engine gives

A look is a few dials. These are the ones that exist today and what they do to a room.

| dial | where | range | in a room |
|---|---|---|---|
| tilt | `angel_stage::Tilt`, `Frame::tilt` | 0 (flat) to `MAX_TILT` 0.6; the game's is 0.3 (`stage_frame::TILT`, pinned to `set::GAME_TILT`) | the floor foreshortens, the far wall shows above where a flat view ends in floor, what stands is placed on its foot and never tipped |
| world | `world3d::set_enabled` | still art, lit blocks | `ANGEL_WORLD3D` off: the room's still art. On: lit blocks |
| grain | the stage's size | the Delve's is 384 x 224 | a stage pixel is an art pixel (below) |
| ramp | `Look::ramp` | 2 to 16 entries | colours: each pixel is placed between two entries by its brightness |
| dither | `Look::dither` | `Bayer4`, `Bayer8`, `None` | the weave between two entries: 16 levels, 64, or bands |
| sun | `Look::sun` | direction, strength, ambient, `rise`, `Shadow` | the key light, how dark the unlit sides go, whether a face pales toward its top, whether it casts |
| lamps, warm | `Look::lamps`, `warm` | up to `LAMP_MAX` (16) | pools of light that move what they light toward the ember ramp: light = activity |
| accents | `Look::accent` | up to 4 | colours never dithered: flame, window, glint, the lava crust |
| fog | `Look::fog` | an entry of the ramp, a start and an end | distance fades to that entry; it takes the warm out of what it covers (below) |
| cards | `Look::cards` | least, most, warm, rim, reach, normals | the figures take the room's light as cards ("Lit cards and depth of field", below), pixel by pixel by their normal maps ("Normal maps") |
| dof | `Look::dof` | focus points, margin, spread, radius | near and far softened in stage pixels, the figures kept sharp (the same) |

Two things the engine does not give, found by building the rungs:

- **`Look::pixel_rows` is not a dial for a room.** The GPU sets it to the stage's own rows (`crates/gpu/src/stage.rs`, "look.pixel_rows = stage_size.1.max(16)") so a stage pixel is an art pixel. The grain of a lit room is the stage's size, and the window's is fixed at 384 x 224. A finer lit room needs a bigger stage with the world made bigger with it. `delve_tiers` does that after the frame is made (`regrain`: every root node's translation and scale, the look's distances, the layers); the stage's camera takes its size from the stage (`Tilt::for_stage`: "any scale of it gives the same picture"), so nothing else moves. The engine has no such setting.
- **The second art kit is cut** (2026-10-20). It drew 2D frames at 768 x 448 as one picture, so it never stood a room in 3D. Its art is archived in `<archive>/2026-10-20/art-v2/`, its code at `refs/archive/cut-20261020/before-art-cuts`.

Fog, measured: the stage camera's eye sits a long way back, so a fog from 250 to 900 pixels swallows the whole room and one from 800 to 2000 hardly shows. Where it bites it also drains the warm out of the lamps (`warm * (1 - fog)`), which is the opposite of what the top rung wants, so it is left off (`--set r4.fog=500,1500` shows it).

## The rungs

Shot in the Crypt: a fight room, the stairs passage, the bone chapel, an open floor. The rung below these, the Atari board of floor one (A0, experimental), has its own section below. Each rung is the same room, the same camera and the same knights, with these dials moved.

| | R0 flat pixel | R1 tilted cards | R2 first light | R3 the monolith | R4 image-like |
|---|---|---|---|---|---|
| world | still art | still art | lit blocks | lit blocks | lit blocks |
| tilt | 0 | 0.3 | 0.3 | 0.3 | 0.3 |
| stage | 384 x 224, art at half grain (192 x 112) | 384 x 224 | 384 x 224 | 384 x 224 | 768 x 448 |
| colours | the art's own | the art's own | 4 of the tone's, in a hue half the still art's | 8, the tone's ramp | 16, the tone's ramp mixed up |
| dither | none | none | `Bayer4` | `Bayer4` | `Bayer8` |
| sun | baked in the art | baked in the art | strength 1, ambient 0.25 (2.5 x the room's), shadows | the room's key, shadows | ambient 1.6 x, `rise` 90 px, shadows |
| lamps | baked | baked | none | the room's, burning as it is fought, the ember ramp | 1.15 x as strong, 1.3 x as far |
| figures | sprites standing on their feet | the same | cards in the world, hidden by what stands before them | the same | the same, a sprite pixel two stage pixels |

What each step is, and how it reads in the shots:

- **R0 to R1, the floor tips and the art is fine.** Tilt 0 to 0.3, and the art's grain doubles. R0 is the old top-down Delve: the view is the room's 24 x 14 tiles, nothing past its edge. R1 is what the window shows with rooms in 3D off: the floor foreshortened, the far wall shown above the view's top, knights and foes on their feet. R0's art is a stand-in (below).
- **R1 to R2, the dimension changes.** The room stands up in blocks (`world3d::rooms_lit`), walls hide knights, one low sun throws shadows across the floor, and everything is four colours with a dither between them. The look is that of a four-colour dithered 3D game: it reads as pixel art that has been given depth. The still art's hue is carried a half of the way (`still_ramp`: the four brightness quarters of R1's own pixels), so the step is one of shape and light before it is one of colour.
- **R2 to R3, colour arrives with the light.** Eight colours, the tone's own hue (the Crypt's indigo), the lamps lit: the braziers and the gate throw ember light across the floor, and the room is dark where nothing burns. Light = activity.
- **R3 to R4, the grain halves and the ramp doubles.** The stage is 768 x 448, sixteen colours between a finer weave, ambient up, the light pale toward the top of a wall, the lamps bigger. The dither is nearly gone. It reads as the same room seen closer, in better light. The knights stay sprites (a sprite pixel is two stage pixels, so they look chunkier against the world), as in the target frame (`look-reference.png`, a fine dither with a pixel knight).
- **R5, the painted plate, is cut** (2026-10-20). It was the room's item's plate cast on its blocks, true colour: the one rung with surface detail. The plates, the items and the rooms stood up plain in a theme's patches are archived in `<archive>/2026-10-20/plate-world/`, the code at `refs/archive/cut-20261020/before-art-cuts`.

The step between R1 and R4 is a loss of surface, not a gain. The lit look has solid tones and no textures by design ("No textures, no plates", REALM-MONOLITH.md): R2 to R4 have less on a floor than R1 does. What they gain is depth and light.

## A0 the Atari board (experimental, `ANGEL_DELVE_ATARI=1`)

2026-10-20: knights now begin as plain shapes and wish their bodies into being, and "the first map to be like... atari/pong". So the climb has a rung below R0: the first floor drawn as an Atari/Pong board, where a knight who is only a shape begins. Experimental and off by default: `ANGEL_DELVE_ATARI=1` turns it on, and so does `ANGEL_DELVE_IDENTITY=1`, whose three-room first floor it draws (DELVE.md, "The identity start") (the same kind of switch as `ANGEL_DELVE_WANTED`), and tests keep today's floor one unless a test turns it on (`atari::set`). The host sends it to every page in its welcome (`wire::Params::atari`, flag bit 3, only when on).

| | A0 the Atari board |
|---|---|
| where | floor one only, every dungeon room of it (not a side-on hall, a ride, or a floor lit from below) |
| world | its own field, no still art, no delivered room kit, no rooms in 3D |
| tilt | 0, in the window and the browser too (`RoomScene::board`) |
| grain | 384 x 224, two room pixels to a block |
| colours | black, two greys and Pong's toned white, one rust hue (foes, shut doors, what hurts) and a gold one (loot): seven inks, every one on the realm's palette |
| dither | none |
| light | none: no baked dusk, no pools of light, no motes, no pickup glows |
| figures | foes and guardians blocky in rust, loot in gold, pots and plates grey; a sprite's signal inks pale grey, its darkest inner blocks holes. Knights as they are drawn (they start as shapes in their own lane) |

What it is, in the code (`crates/delve/src/render/overworld/arena/atari.rs`):

- **The field** (`playfield`): walls are grey bars one tile thick where they meet the room (rock deeper in is the field), pillars grey blocks, pits and water dim rust, doorways gaps with a rust bar across while the room is barred, the stairs pale treads (grey while the guardian holds them), spike plates outlined squares, and a dashed line down a room's middle on its floor. The fixtures the room draws (an altar, grilles, a cracked wall) are taken as blocky grey shapes. The layout is not changed: every door, stair, mission marker and spawn is where it was.
- **The sprites** (`sprite`): a foe's or loot's own drawing taken two pixels to a block, each block drawn where half of it is, in one flat ink. `foe_figure_with` does it for every monster and guardian, so the window's cards are the same blocks; loot (`figures_with`) is stamped as its block sprite on every page, never handed over as a turning card or a model.
- **The floor's own paint** (`board`): what is painted under the figures (hazards' marks, rings, folk, pools) is held to the seven inks in blocks of two, in the terminal (`room_layer`) and in the window's `under` (`scene_for`) alike.
- What is not touched: the knights, shots and spell effects over the figures, the HUD, the boards and banners, the minimap. The window's post pass (a 0.22 vignette and a faint grain) still runs over the stage: turning it off for one frame would need a flag on `angel_stage::Frame` and its wire.

From A0 to R1 is the first step of the climb: floor two is the black 2D Delve, in the window tipped at 0.3 and lit by its fires.

Proof: `delve_tiers` has the rung `a0` (floor one cases only) and two cases for the step: `crypt-stairs` (floor one's way down) and `crypt-fight-two` (floor two's first fight). `--case crypt-fight --case crypt-stairs --case crypt-fight-two --rung a0 --rung r1`. The terminal's shots: `ANGEL_ATARI_SHOTS=<dir> cargo test -p angel-delve --lib write_atari_shots` (`<case>-a0.png`, `-a0-term200.png` as a 200-column terminal boils it down, `-today.png` with the switch off). Shots of 2026-10-20: `<artifacts>/identity-20261020/atari/`.

## Built, and what would have to be

| rung | built | not built |
|---|---|---|
| R0 | tilt 0 (a frame can ask for it) and the still art | a tilt of 0 in the game (`StageMaker::make` always says 0.3) and art at half the grain. The proof takes R1's frame and sets each 2 x 2 square to its commonest colour (`chunked`), which keeps the palette; a drawn 192 x 112 art set would be cleaner |
| R1 | all of it: the window's frame with rooms in 3D off | |
| R2 | the lit blocks, the sun, shadows and dither | a 4-colour look as a setting. The proof makes it from the room's look (`dial`: four entries picked along the room's eight, their hue mixed with R1's) in the example. The hue carry would need a table of each place's still colours, or a read of the room's art, in the game |
| R3 | all of it, on this branch | |
| R4 | the look's dials (16 entries, `Bayer8`, `rise`) | the grain. The stage cannot be made finer by a setting. Proven by scaling the made frame in the example. The figures are not finer: their sprites are the Delve's own |

And one thing that is none of the rungs': **a switch that moves them together.** Today the pieces are set apart: tilt is a constant, rooms in 3D by `ANGEL_WORLD3D`, and the look is made by the room (`rooms_lit_look::look`). A rung would be one value that chooses them all.

## How cards are chosen (what would make the art finer)

Written down, not built.

- **A figure is a card.** Each `Figure` (the knight, a foe: its sprites composed to one picture at the art's own grain) becomes a picture of its own and a quad standing on its foot (`stage_frame::cards_of`, `card`), drawn nearest with depth. Its colours are its own; in a lit room it takes the room's light ("Lit cards", below).
- **A hook for a finer drawing, now fed.** `StageMaker::make` takes `hd: FnMut(&Body) -> Option<u32>`: for a sprite (`Body`, named by `look_key`, a hash of its pixels) the window may have an HD picture, and it is drawn over the card in the sprite's place with the sprite's own pixels taken out from under it (`with_art`; the test `an_hd_drawing_takes_its_sprite_out_from_under_it`). Since 2026-10-21 an HD art pack feeds it, and the room's still art too ("HD art packs", below).
- **To make the art a ladder**, three things (the first is built for the knights, 2026-10-28: docs/LADDER.md, the resolution ladder: named figure keys, a rung by floor, the knights' stills at 32, 64 and 128): the figure sprites at more than one fidelity, chosen by a rung (a coarse set for R0, the Delve's for R1 to R3, finer drawings for R4 through the `hd` hook); the lit surfaces given some detail past R3 (a patch a kind, `Material::patch`, is the nearest thing the engine has); and the stage's grain made a setting, so finer art and the lit world can be drawn together.

## HD art packs

2026-10-21: "we want larger tiles in glass and to show better detail ... detail never gets better." The game's grain stays 384 x 224, 16 room pixels a tile, one art pixel a room pixel: gameplay, the wire to browsers and the terminal are as they were. The window was never the limit (`crates/gpu/src/stage.rs`: a stage is drawn into a window-sized target and each picture fills its rect at whatever texels it has, as the paintings do at 6 x). What it lacked was finer pictures to draw. A pack gives it them.

**The pack.** A folder of PNGs and `pack.json` (`arena::hd_pack`; `ANGEL_GLASS_ART`, default `~/.angelX/art/delve`, read once as the window opens). Beside the look entries below, an entry may name a figure by what it is and the resolution rung it is drawn for (docs/LADDER.md): those are matched first. Each entry redraws one look, a sprite's or a room kit tile's picture at the game's grain, named by its key:

```json
{ "scale": 4,
  "art": [
    { "look": "4fe82b10a093d885", "name": "knight-p1/whole-idle-down", "w": 16, "h": 16 },
    { "look": "…", "name": "knight-p1/whole-idle-left", "file": "knight-p1/whole-idle-right.png", "flip": true, "w": 16, "h": 16 },
    { "look": "…", "name": "crypt/floor/plain", "w": 16, "h": 16 } ] }
```

| field | |
|---|---|
| `look` | the look's key (`look_key`, FNV-1a over its size and pixels), sixteen hex digits |
| `name` | what it is; the file is `<name>.png` unless `file` says |
| `scale` | the drawing's pixels across one of the look's: the pack's, 4 unless it says (64 to a tile) |
| `w`, `h` | the look's own size, so a drawing of the wrong size is said as the pack is read |
| `flip` | the drawing is the file mirrored (a knight facing left is the one facing right) |
| `normal` | its normal map; `<file>_n.png` beside it is taken unnamed, and without one a map is made at its grain |

A drawing is its look's size times its scale, the figure where the look's is, clear round it. One of any other size, a file outside the folder, or a look that is not sixteen hex digits is left out and logged; the look keeps its inks. A loose `<look>.png` with no entry (the pack before the manifest) is still read, at whatever size.

**Figures.** The window's `hd` is the pack's (`hd_pack::HdArt`): a sprite whose look the pack draws gets a picture id, its pixels (and its normal map, when lit cards turn by them) sent with the first frame that names it, as a page's paintings are. The Delve's client feeds the same hook (`client.rs`); a browser installs no pack (a page has no folder to read one from), so its page is sent the inks as before. A kit's blocks that stand among the crowd (a tomb a knight walks behind) are noted as bodies of their tile's look (`dungeon_art::stand_object`), so they take the same drawing.

**The room.** Tiles composed at N x, not a picture of the whole room: a room is generated from its kit, so a drawing of one room fits no other, and the kit's `bake` already lays a room at a scale. `dungeon_art::fine_bank(pack, scale)` (3 and up) is the pack's: each 1x kit tile's drawing, found by the tile's own look key, else the tile itself brought up nearest. The same `bake` lays the room at that scale (its floor lit a pixel at a time, not a 1x pixel at a time as the kit's own grains are), and `fine_scenery` adds its fires, motes and fixtures (a chest, a trap: the game's own, brought up). `RoomScene::fine` carries it beside `ground` for a room whose kit the pack has any tile of, whose still art is the kit's (no plate laid down, no entrance art, not stood up in 3D) and that is not a great room's map. The stage sends it as the still art's picture and lays it in the 1x art's rect (`stage_frame`, `GROUND`); nothing else moves. It is kept a torch step at a time, as the 1x art is.

**What the pilot showed** (2026-10-21, a placeholder pack: each look brought up four times with a dot an art pixel, `<artifacts>/hd-art-pilot/`):

- **It lands exactly.** In the Crypt's fight room shot flat (`delve_tiers --case crypt-fight --rung r1 --set r1.tilt=0 --hd <pack>`), every pixel that differs from the 1x shot is a placeholder's dot: the room composed at four times and the knight's drawing sit on the 1x art's pixels. In the window (the own-seat kit, joined to a dev host's Crypt fight) the floor, the walls, the tombs and the first seat's knight show the dots; the second seat's knight, the foes, the shots and the marks keep their inks.
- **The window shows what its size gives.** At 1576 x 880 the room is drawn about 2.25 window pixels to a room pixel, so the four-times picture is sampled down and part of its detail shows; a view of four window pixels to a room pixel (1536 wide) shows all of it.
- **Cost.** Composing the 34 x 20-tile fight room at four times (2176 x 1280) takes 56 ms once a room (a hitch as the party enters, on the render worker); after that a torch step's scene and stage frame take about 5 ms against 0.6 without the pack, and its picture (11 MB) goes up five times a second while the torches flicker. Sending the still base once a room and only what flickers each step would cut both; not built.
- **Seams left.** A figure's contact shadow is darkened 1x floor in its own picture, so under a figure on a finer floor it is 1x (on the black Crypt floor it does not show). A block in front of a figure is drawn whole and upright over the floor's tilted one, where the 1x figure kept only the pixels it covers. A lit room (R2 and up) lays no still art, so only its figures take the pack, and a drawing's card is lit by its own normal map made at its grain: by the brazier it reads paler than the 1x card. Its signal inks stay unlit, as a sprite's card's do.

**The art request.** `ANGEL_HD_REQUEST=<dir> cargo test -p angel-delve write_hd_request -- --ignored` writes every look the first seat's knight shows (dressed as its health leaves it, four facings, standing, its walk, the bow drawn, the sword's swing, the shield up, a hit's flinch; one facing left that is the right one turned over listed as its mirror) and the tiles the Crypt fight room the rungs' proof shoots is laid with, each a 1x PNG named for what it is, with `pack.json` and `request.txt` (the size to draw each at), and the room as composed, for reference. Redrawn in place at four times, the folder is the pack. `scripts/delve/inks_hex.py` writes the game's inks as a pxl palette (`rrggbb k bank`, the ink's own character its key); `scripts/delve/hd_placeholder.py <request> <pack>` makes a placeholder pack from a request (each look brought up nearest, a dot an art pixel: never art). `delve_tiers --hd <pack>` shoots the rungs with a pack.

## Lit cards and depth of field

2026-10-19: "yes do depth of field and lit sprites next", after Replaced (hand-drawn pixel sprites in a 3D world, modern light, depth of field). Two dials of the look (`angel_stage::look::Look::cards`, `Look::dof`), both on in every lit room (`rooms_lit_look::with_figures`, from `stage_frame::lit_world`, so R2 and up), none in a flat or still rung (R0, R1, which have no lit look) and none in any other look (the monolith proof, the shore, the kingdom keep theirs as they were).

| dial | where | default (a lit room) | off |
|---|---|---|---|
| lit cards | `Look::cards: Option<Cards>` | `Cards::ROOM`: least 0.6, most 1.6, warm 0.75, rim 0.6, reach 1.8, normals 1 | `ANGEL_STAGE_LIT_CARDS=0`, or `cards: None` |
| normal maps | `Cards::normals` | 1: each pixel turned as its map has it | `ANGEL_STAGE_CARD_NORMALS=0`, or `normals: 0` |
| depth of field | `Look::dof: Option<Dof>` | `Dof::room(feet)`: margin 32 px, spread 96 px, radius 1.5 stage px | `ANGEL_STAGE_DOF=0`, or `dof: None` |

**Lit cards.** A world's figures were drawn after its lit floor and blocks with nothing of its look: a knight between the braziers was as bright as one beside them. Now, with `Look::cards`, the stage draws them (`stage.rs`, `props_picture` with the world's look) through their own pipelines (`scene.rs`, `card_pipelines`; mesh.wgsl's `fs_card`): the plain pipelines' blend and depth at the stage texture's size, with the look's group 3. Each pixel is lit as an upright face turned to the eye at the point of the card it is (and, since 2026-10-20, turned from that face by its normal map, "Normal maps" below): the sun (its shadow map too: a card behind a wall is in its shadow) as a part of a reference halfway between what the card would take in the open and full light, held between `least` and as drawn; the lamps added to that, reaching a card 1.8 times as far as the floor (an upright figure stands up into a flame's light), up to `most`; the lamps' warmth moving its colours toward the warm ramp's hue at their own brightness; a lamp behind it lighting its outline (`rim`); the fog over it. Its pixels keep their colours: light multiplies them and is never a ramp or a dither, so the sprite stays the sprite. An accent card and the signal inks stay as drawn: `stage_frame::card_rgba` stores a sprite's signal-bank pixels (a knight's mark and plume, a foe's eyes, `ink::is_signal`) at alpha 254 (`look::UNLIT_ALPHA`), which `fs_card` reads as "unlit" and draws opaque. Shadows from the cards onto the floor are not built: the shadow pass draws opaque plain surfaces with no fragment stage, so a card would cast its whole quad; it would need an alpha-tested shadow pipeline with the figures in the world's shadow pass, and in a delve the key light is low (`Tone::key`) and its shadow would hardly show beside the knights' own drop shadows.

**Depth of field.** Depth is the camera's own (`look::depth_row`: its clip `w` over how fast `w` grows, in world units; for a stage camera, how far up the floor from the eye, so a card is as far as its foot). `Dof::band` keeps sharp everything from the nearest to the farthest focus point and `margin` past either; past that the blur grows over `spread` to `radius`. The focus is every figure in view (the nearest and the farthest feet down the floor, `rooms_lit_look::focus`): the knights and what they fight are always in the band, so a fight that fills the room blurs nothing, and the blur shows where the party stands apart (a passage, an open floor's far wall). `fs_look` writes how far each lit pixel is out of focus into the accent mask's second channel (the mask is `Rg8Unorm`), and look.wgsl's post pass softens the pixels that are out of focus among their neighbours in internal pixels (`soft`: a disc, each neighbour taken as far as it is out of focus itself, so a sharp pixel does not smear into the soft ones). It runs before the enlargement, so every stage pixel is one value and the blur keeps the grain. It softens only the world the look places (floor and blocks); figures, props and the 2D layers are drawn after and never blurred.

Decided from the shots:

- **Blur after the dither, by a mean.** The lit pixels are placed on the ramp as they are drawn (`lit_look`), so the blur can only come after the dither. A mean of the neighbours (colours between the ramp's entries, one value a stage pixel) reads as out of focus; the alternative tried, a weave (the darkest or brightest neighbour by the Bayer threshold, keeping to the look's own colours), read as a looser dither, not as a blur.
- **Radius 1.5 stage pixels**: the far wall and the near floor go soft and the dither melts into a smooth tone there, and nothing reads as smeared. R4's stage is twice as fine, and `regrain` doubles the radius with it, so it softens the same.
- **Least 0.6.** The dark delve (`Tone::key`) leaves the floor between the lamps dim, so a card in the open takes the least: at 0.38 the knights went to a muddy grey and read poorly; at 0.6 they are plainly in the dark and still read. A knight within a brazier's reach is lit warm on the side toward it (the crypt-brazier shot), and a portcullis beside a brazier glows (it is a card).
- **Reach 1.8.** A delve's lamps throw small (33 px), so at the floor's reach a knight a tile from a wall torch took none of it.

Frame cost (RTX 3090, the whole stage drawn and waited for, 300 frames, shown at 1536 x 896; `delve_tiers --time 300`): at 384 x 224 the Crypt's fight room is 2.74 ms with both off, 2.77 to 2.80 with either or both; at 768 x 448, 2.77 off and 2.81 to 2.85 on. The passage is the same within 0.1 ms. Both dials together cost under 0.1 ms a frame.

## Normal maps

2026-10-20: normal maps for the game's sprites, so a lit card takes the light pixel by pixel. Before them a card was one face turned to the eye: a knight a tile right of a brazier was lit as evenly as one in front of it.

**What a map is.** A picture the size of the card's, opaque: red is how far a pixel turns to the picture's right, green how far up (the OpenGL way), blue how far out toward the eye, each `n * 0.5 + 0.5` (`angel_stage::normals`). A drawer keeps picture `id`'s map under `normals::picture_of(id)` (the id with bit `0x4000_0000` flipped, so a layer picture's lands past `0xC000_0000`), and a card's material names it (`Material::normals`; `stage_frame::card` names it for every card). A card whose map the drawer does not have is drawn as before.

**Made for every figure.** `normals::of_picture` makes one from the picture itself: the distance of each pixel from the outline (a 3-4 chamfer), raised to a round dome that reaches its full height 6 art pixels in (`ROUND`; a knight's body is rounded across, a blade stays flat on), a little of each pixel's brightness over it (`DETAIL`, 0.3 pixels of height black to white, softened over a pixel, so a dark outline and a lit fill read as a surface and the art is never embossed), the whole softened over a quarter of the rounding, and the slope at each pixel turned into its normal. A sprite's signal inks round its feet (a foe's ring) are no part of the dome, and those the body runs on above and below (a tabard in a team's colour, an eye) are. A finer drawing is made at its own grain (`grain`: its pixels to an art pixel), so it rounds over the same span of the figure and keeps its finer surface. The Delve makes each figure's map with its picture (`StageMaker::cards_of`), only for a room in 3D with both switches on, and keeps the last 512 by look, so a knight's few frames are made once: 18 figures made fresh cost 0.1 to 0.15 ms of CPU, once. Drawn nearest, one normal a pixel of the picture: a sprite pixel at R2 to R4 is one face, and nothing shimmers between them.

**Authored where the art has one.** A painting may ship its own: `paint <slot> <file.png> normal=<file_n.png>` (the painting's size, green up, in the sheets folder) keeps it in the paint store by its sha256 as the painting is, and the card says `draw <slot> paint=<sha256> normal=<sha256>`. The kiln maker's Pixel Art Lighter (`kiln-studio lighter`) writes that green-up map; rename its `name-Normal.png` export to `<file>_n.png` and do not invert the channels. It travels in the card text and the save (`Sprite::normal`), loses its word with the painting below a wish's top level (`fidelity::block`), and a joined seat fetches it from the host's `/paint/<sha256>.png` as it fetches the painting (`painting_bytes`). The window cuts it as it cuts the painting, each normal under the painting's alpha so the frames crop and average alike (`paintings::cut_normals`), and mirrors it, red turned over, for a flipped frame (`normals::mirrored`). A painting with none gets one made from its cut frames at their grain (`paintings::made_normals`). An HD drawing (`ANGEL_GLASS_ART`, `<look>.png`) may have `<look>_n.png` beside it; without one, one is made. The art slot sheets are composed into the figure's one picture before it is a card, so a map beside them would have nowhere to go: theirs is the one made for the figure.

**How a pixel is lit.** In `fs_card`, `card_normal` turns the upright face `n0` by the map: the map's red along the picture's right as the card stands (its x, made square to the face), green up, blue along `n0`, the turn scaled by `Cards::normals`. Then, with the pixel's normal `n`:

- the sun: its facing is `n`'s, and the shadow test is pushed out along `n`; the reference it is held against is still the card's (`n0`), so `least` and `most` mean what they meant, a pixel at a time;
- a lamp: the upright face's wrapped light as before (`0.45 + 0.55 * toward`), plus the pixel's turn from that face at a plain Lambert's slope, so a pixel facing as the card does is lit exactly as before and one turned toward the flame more;
- the rim: a lamp behind the card still lights its outline, the more on the side the outline's pixels turn toward it (`1 + 1.5 * lateral . toward`, held between 0 and 1.5 so the lit outline never flares), the same all round with no map;
- warmth and fog as before. Light only multiplies a pixel's colour.

Signal inks and accent cards stay unlit, as before. With `normals` 0, or no map, every pixel is `n0` and the card is drawn exactly as before.

What the shots show (`<artifacts>/identity-20261020/normals/`): where a lamp reaches a figure, the side toward it is lighter and the far side falls off, softly: the knights by the Crypt's brazier read rounded at R3 and R4, and a painted shield shows its raised boss and rim lit from the lamp's side with its own map, and a broad dome without. Where nothing but the delve's glimmer of a sun reaches them (R2, which has no lamps, and the dark between the braziers), a figure is at `least` and the map changes nothing: the black delve's light is not directional enough there to show a shape. Frame cost: within the noise of 300 frames (RTX 3090, `--time 300`, the Crypt's fight room by its brazier and the passage, R3 and R4: 0.88 to 0.92 ms flat against 0.90 to 0.92 with the maps by the brazier, 0.70 to 0.78 either way in the passage, while other builds ran); the map is one more nearest sample a card pixel.

`--normals` on `delve_tiers` shoots each lit rung with the maps off (`-flat`) and on (`-normals`), lays R3's figures beside their maps (`-maps.png`), and with `--time N` times both and what making the maps costs. The case `crypt-painted` (named, not in the default set) is the brazier with the first knight wearing a painted bossed shield whose painter shipped its map: `-flat`, `-made` and `-authored`. `--set r3.normals=1.5` moves the dial.

Not built: the browser draws no lit cards (a page across a wire is sent no room in 3D, `StageMaker::for_wire`), so it is sent no maps and fetches none; the kingdom's lit look has no cards dial, so its figures get none.

## Proof

`crates/glass/examples/delve_tiers.rs` makes each rung's frame as the window does (`StageMaker::make`, the GPU core offscreen) and moves only that rung's dials after: `frame.tilt`; `regrain` for the stage's size; `dial` for the look's ramp, dither, sun, lamps and rise; `chunked` for R0. The engine is as it was (no `pub` added; the example uses `arena::in_seat` and `scene_for`, as `delve_rooms` does with `seat_scene`, to shoot the room's view without the party's band and the HUD's).

```
cargo build --release -p angel-glass --example delve_tiers
$CARGO_TARGET_DIR/release/examples/delve_tiers <dir> [--scale N] [--case NAME]... [--rung RN]... [--open SEED] [--set rung.dial=value]... [--pairs] [--time N]
python3 scripts/tiers-sheet.py <dir> [--cell 520] [--detail x,y,w,h] [--pairs]
```

- It shoots `crypt-fight`, `crypt-passage`, `crypt-bone-chapel` and `crypt-open-floor` (seed 41, the one of eight seeds tried that shows a wall and a name over its floor), `<case>-r0.png` to `-r4.png` at `N` window pixels to a Delve pixel (4: 1536 x 896).
- `--set` moves any dial of any rung (`r2.dither=none`, `r4.grain=3`, `r3.lamps=1.5,1`, `r4.fog=500,1500`); it is how the rungs' values were found.
- `scripts/tiers-sheet.py` writes `contact.png` (a row a room, a column a rung) and `detail.png` (one patch of every shot as drawn, for the grain).
- `--pairs` shoots each lit rung four times more, `<case>-<rung>-off.png`, `-cards.png`, `-dof.png`, `-both.png` (the two dials that follow the figures off, each alone, both), and `tiers-sheet.py --pairs` lays them out as `pairs.png` and `pairs-detail.png`. `--time N` draws each of the four `N` times and prints a frame's cost. `--set r3.cards=0` and `r3.dof=0` turn one off; `r3.cards=least,most`, `r3.dof=radius,margin` and `r3.spread=px` move them (in pixels of the Delve's grain). The case `crypt-brazier` is the fight room with the knights stood by its brightest lamp (`by_lamp`).
- Shots of 2026-10-19 with the two dials: `<artifacts>/monolith-private/tiers-dof-lit/` (`pairs.png`, `pairs-detail.png`, `brazier-detail.png`, every shot).
- Shots of 2026-10-18: `<artifacts>/monolith-private/tiers/` (`contact.png`, `detail.png`, every `<case>-r<n>.png`).

What the shots show (compared with the target frame, `<artifacts>/delve-handoff-20261018/look-reference.png`):

- Every rung is a distinct step in all four rooms. The climb is clearest in the fight room and the chapel: pixel art, tipped, given depth and four colours, given colour and ember light, made fine.
- R4 is nearer the target frame than R3 is in grain: the target's weave is finer than R3's, with a pixel knight in it, as R4's is. R4's dither is almost invisible at sixteen entries. With eight entries and `Bayer4` at this grain (`--set r4.ramp=8 --set r4.dither=4`) the weave shows as it does in the target; sixteen was kept for the "image" the rung is meant to be. With `dither=none` at sixteen entries the floor bands.
- Weak steps. R1 to R2 is the largest and the least continuous: the floor's stone, the crates and the walls' carving become plain blocks and tones, and the hue carry softens that only a little. R2 itself is the least handsome rung (grey-violet, dull against R3), and tried without dither, or without shadows, it was worse (a four-tone flat shade where floor and wall land in one band). R3 to R4 is subtle in a still. R0 is a stand-in. The open floor is the weakest room: its first screen is a wide empty hall, dark from R3 on.
- What was not checked: the rungs in the browser; any room but the Crypt's; the rungs in motion.
