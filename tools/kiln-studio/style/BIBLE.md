# The Delve's style bible, v0

What every kiln job is held to. The numbers are in `bible.json` beside this file; the lessons people
leave on the review page are appended to `~/.angelX/kiln/style/lessons.md`, which directors and job
agents read after this. Sources are cited so a rule can be checked against the game.

## Grain and size

- The game draws **16 px a tile** (grain 1); HD packs draw **64 px a tile** (grain 4) over the same
  looks (`hd_pack.rs`, scale 4). A kiln recipe renders at every grain; a figure must read at both.
- Stature in game pixels (`crates/delve/src/render/overworld/arena/stature.rs`): a knight 16, the
  smallest foe 10, a chief 24, a guardian 32. At grain 4: 64, 40, 96, 128. Game art is only ever
  enlarged, never shrunk.
- A figure for one tile sits on a **64 x 64** canvas at grain 4, pivot bottom centre, its feet on
  the bottom row. A canvas is a whole number of game pixels (a multiple of 4 at grain 4).

## Light

- **One light, from the upper left** and a little toward the viewer (kiln `[-0.55, -0.9, 0.7]`; the
  game's sun `[-0.55, -0.75, 0.6]`, `material.rs`).
- Lit planes face up and left; the lower right steps **one tone down its own ramp**, never into
  another hue's dark (`docs/ART-V2.md`, "Shading"). Shadows lean cool, lights warm.
- A contact shadow falls under and to the lower right; the game draws a foe's under it
  (`s x s`), so a sprite leaves the floor clear.
- Glows, eyes and signature lights use the signal inks and are not lit; one or two such lights per
  figure, placed by hand (`docs/BOSS-ART.md`).

## Line

- The silhouette is closed by a **near-black outline**, ink `k` (#0b0a08), with **selout**: where the
  line touches a lit colour it takes that colour's ramp two steps darker, so the line softens on
  the lit side and stays dark on the shadow side and against the floor (KILN.md: `outline: { ink:
  "k", selout: 2 }`).
- Inner lines, 1 px, only where parts or depth break (an arm over the ribs, a helm's rim). One-pixel
  limbs and blades keep their midtone; they don't get a black line on each side.
- Lines are drawn as by hand: no doubled steps on a smooth curve, no L-corners a shape doesn't have,
  no 3 x 3 blocks of line (`docs/ART-ARCADE.md`).

Open with the owner: ART-V2 and BOSS-ART say "no black outline" (coloured edges, darkest step of the
material). This bible follows KILN.md (k with selout) for kiln's HD figures; the review votes decide.

## The realm's inks

The game's own 74 (`crates/kiln/assets/realm.hex`), nothing else: no colour between two inks, no
half-transparent pixel. Inks are used as **ramps**, dark to light:

| material | ramp | from |
|---|---|---|
| steel | `sxSuUvVW` (shadow steel starts at `S`) | KILN.md, ART-ARCADE |
| red cloth | `n87`, trimmed in gold `5` | KILN.md, hero.rs |
| gold | `a456`; brass `PotT` | KILN.md, BOSS-ART |
| leather | `nbIBpr`; timber `nbIBPrRo` | KILN.md, material.rs |
| gem | `q1z3` | KILN.md |
| bone | `SviH` (the game's foe bake); `jGJhiH` for a warmer grey | foe_material.rs, pfx.rs |
| rust | `b7Ro` (BOSS-ART lists `bR7o`; `7` is the darker) | BOSS-ART |
| stone | `KXgjGJhi` (warm), slate `sxSuUvV` (cool) | material.rs |
| skin | `ProtT` | mirror.rs |
| violet dye | `d()[]^` | realm.hex |
| pink dye | `!?;%+` | realm.hex |
| foliage | `fDFElmAy` | material.rs |

- Signal inks (`0123w a4@56 $c9 87`) are for glows, eyes and fire, never a material.
- The white-core inks `W`, `w` and `9` are glints, a pixel or two, never a core or a fill
  (ART-ARCADE's white-core test).
- About 12 to 18 inks make a figure; a boss at game grain uses at most 10.

## Figures

- **Chibi.** At 64 px the head is about 40% of the height (the knight at 16 px is half head);
  short limbs, heavy hands and feet, a round belly on the stout ones (`docs/ART-REQUESTS.md`).
- **Three-quarter view, facing right.** The game has three views: toward, across (facing right;
  left is the flip) and away. Figures stand upright; the floor's tilt never tips them.
- **Silhouette first.** At grain 1 the figure is 10 to 16 px: what it is, which way it faces and what
  it holds must read from the silhouette alone. A weapon or a limb thinner than 4 px at grain 4
  vanishes at grain 1; thicken it or let it go.
- Big shapes, grouped values: the head, the torso, the weapon, each one clear mass of its ramp.
- The Crypt's dead: the game's skeleton (`sprites.rs`, 11 x 15) is a rounded skull, separate ribs
  and joints, bones in `SviH`, sockets dark. A skeleton warrior at 64 px keeps that read: a big
  skull, a ribcage you can count, thin limbs made thick enough to survive grain 1, rusted or worn
  gear. Its eyes may hold one signal light (the wraith's teal `3`).

## Effects

- Effects draw in the vfx ramps of `bible.json` (`$fire`, `$ice`, `$holy`, ...), all realm inks.
- Hard colour bands, no blur, no gradients, no haze or full-frame glow, no loose single-pixel
  particles (`docs/BOSS-ART.md`, `docs/ART-REQUESTS.md`).
- Timing: anticipation, a fast hit, a slow settle; a one-shot ends clear.

## Avoid

- **Pillow shading**: light in the middle, dark all round the edge, whatever the light. Light comes
  from the upper left only.
- **Dithering as texture**, checker or diamond cloth, speckle, mottling, noise.
- **Lone pixels** and 1 to 3 px islands, spurs and notches (an eye or a glint is the exception).
- **Banding**: two or three tones in parallel stripes following the outline.
- Gradients, blur, haze, anti-aliasing into the floor, pure white, enlarging a small drawing to fake
  detail, likenesses of other games' characters.

## Rubric (the critic's and the reviewers')

Each 0 to 10: **silhouette at 16 px**, **readability at true size**, **palette and light**,
**proportions**, **fit to the brief**, **craft**. `bible.json` `rubric` says what each means.
