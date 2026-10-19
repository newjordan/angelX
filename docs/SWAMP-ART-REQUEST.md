# The Swamp of Horrors: the art request

Items 10 to 13 of `docs/SWAMP-OF-HORRORS.md`, for the art team. The rules
side is built and playable with borrowed placeholder art; every swamp picture
goes through one module with fixed public functions, the way
`arena/wax_art.rs` does for the Wax Maze. Draw into those functions and the
art drops in: nothing else in the game changes.

- **The module:** `crates/delve/src/render/overworld/arena/swamp_art.rs`.
  Keep every signature below exactly; replace the bodies. Set
  `PLACEHOLDER` to `false` when the last borrowed body is gone.
- **The establishing plate** is the one exception: it is painted in
  `crates/delve/src/render/overworld/arena/plates.rs` with that file's own
  `Brush`, as every other floor plate is (see 13).
- **The contract test** is in the module (`swamp_art::tests`): sizes, the
  realm palette, and how much may glow. Run
  `cargo test -p angel-delve swamp_art`.

## Rules for every drawing

- **Palette.** Realm inks only (`crates/delve/src/render/overworld/ink.rs`),
  one character per pixel, `.` for black paper (transparent):
  - greys `k K Z X g j G J h i H`
  - greens `f D F E e l N m L A M y C Y`
  - wood `n b I B p P r R o O t T`
  - stone `s x S u U v V W q Q z`
  - signal `0 1 2 3 w a 4 @ 5 6 $ c 9 8 7`: the bank that glows. Only for
    eyes, fire, a wisp's flame, a lock that holds: state, never dressing.
    Each function below says how many signal pixels it may use.
- **Look.** Metal Slug era in our dark palette: chunky readable silhouettes,
  tonal shading, dark colours kept. No stark white (`w`) for large areas, no
  hypercontrast. The swamp is night: black water, green-black mud, pale
  things that should not be there.
- **Monsters are raw ink rows.** Return the undressed drawing; the arena
  dresses it (`foe_material::dress`), keeps it, mirrors it to face the
  knights, and scales elites. Draw facing east.
- **Tells are part of the rules.** Each horror has a wind-up its picture
  must show for the whole tell (stage 1 below). The floor marks (ripple
  ring, cleave arc, lure rings) are drawn by `arena/swamp.rs` from the
  monster's own state; draw the creature's side of the tell (fingers
  breaking the surface, the blade overhead, the flare) and keep it legible
  at native size.
- **Determinism.** Same arguments, same pixels: use `ink::hash` and
  `ink::vnoise` for variety, never randomness or time.

## 10. Swamp ground, bog water, reeds, dead trees, mist

The floor is laid as tiles (16 by 16 native pixels): dry ground (`Floor`:
reed islands and the causeway lanes through each room), bog (`Bog`: walkable,
it holds the feet to 55%), deep water (`Hazard`: stops feet, drawn as the
realm's open water), and blocks (`Block`: reed clumps and dead trees).

| Function | Size | Frames, states | Glow |
|---|---|---|---|
| `pub(super) const STYLE: Style` | | the swamp's rock (`RockKind`) and lit-floor inks `(cool, firelit)` | none |
| `pub const WALL_RAMP: &[char]` | 7 inks | the swamp's walls, dark to light, for `material::wall_tile` | none |
| `pub fn ground(x: i32, y: i32) -> Option<char>` | per native pixel | still; dry ground: tufts, roots, stones on black paper, at most 30% of pixels inked | none |
| `pub fn bog_field(cv: &mut Img, rect: (i32, i32, i32, i32), bog: &dyn Fn(i32, i32) -> bool, step: u32)` | paints only pixels where `bog(x, y)` | `step` moves every 6 ticks: slow bubbles, a skin of scum, glints; at least a quarter of bog pixels inked | none |
| `pub fn reeds(cv: &mut Img, x: i32, y: i32, v: u32)` | the 16 by 16 tile at `(x, y)`, rising at most 14 above it, at most 4 either side | still; `v` picks at least three: a reed clump on a tussock, reeds with bulrushes, a dead tree stump | none |
| `pub fn mist(x: i32, y: i32, tick: u32) -> Option<char>` | per native pixel inside a fog patch | slow drift with `tick`; wisps, at most 15% of pixels | none |

Bog must read at a glance as ground that slows you, never as the deep water
beside it (`material::water_field`) and never as dry ground. Fog patches are
`swamp::FOG_REACH` (7 units, 56 pixels) across; the arena darkens them as the
Lights Out wheel mode does and draws `mist` over the dark.

## 11. The horrors and the Mire King

| Function | Size | States (stage), frames | Glow |
|---|---|---|---|
| `pub fn bog_hand(stage: u8, t: u32, tick: u32) -> Img` | at most 24 by 24 | 0: under the mire, travelling (a bulge and swirl in the mud; shots pass over it); 1: the tell, `t` of `horrors::HAND_TELL` (30) ticks: fingertips breaking the surface; 2: risen, a hand and forearm out of the mud, clutching (`HAND_UP`, 75 ticks; now shots find it). Two frames each on `tick / 8` | none |
| `pub fn drowned(stage: u8, t: u32, tick: u32) -> Img` | at most 28 by 28 | 0: a drowned knight wading, weed-hung rusted plate (two-frame walk); 1: the tell, `t` of `horrors::DROWNED_WINDUP` (28) ticks: the rusted blade lifted overhead, water running off it; 2: the blade stuck in the mud after the cleave (`DROWNED_STUCK`, 45 ticks) | 32 (eyes, the blade's edge) |
| `pub fn wisp(stage: u8, t: u32, tick: u32) -> Img` | at most 16 by 20 | 0: a pale floating flame keeping its distance (flicker, 2 to 4 frames); 1: the tell, `t` of `horrors::WISP_LURE` (45) ticks: flaring and beckoning, brighter and larger; 2: dimmed after its burst (`WISP_DIM`, 60 ticks) | 64 (it is a light) |
| `pub fn mire_king(raged: bool, tick: u32) -> Img` | at most 32 by 32 | the swamp's guardian: a crowned thing of the bog, weed and antler and rot, heavy, Metal Slug boss heft. Two idle frames on `tick / 12`; `raged` (below 35% of his health) shows it | 12 (eyes) |

The Mire King also lives in a boss file: paste his calm frame, as ink rows,
into the `art` block of `cockpit/assets/dungeon/bosses/the-mire-king.boss`
(at most 32 rows of at most 32 inks, replacing the Rootwarden's rows it
borrows now). The Trophy Hall and the guardian pages read the file; the room
draws `mire_king`. His attacks, for the picture's sake: a Ravage (roots and
hands burst up in rings round him, ripples first), then rain, a laser and
slow seekers, each marked before it lands.

## 12. The white horse

Blanchefleur, a white mare the Mire King penned. She joins the herd and
grazes in the Flower Fields; she races; riding her comes later.

| Function | Size | Poses | Glow |
|---|---|---|---|
| `pub fn white_horse(pose: HorsePose) -> Img` | at most 56 by 48, side-on facing east, hooves on the bottom row | `Stand`, `Graze` (head down at the grass), `Gallop(frame)` (four frames, `frame % 4`; the arena passes `tick / 4`), `Frightened` (penned: backing up, ears flat, head high) | 40 (her eye, tack trim; never the coat) |
| `pub fn white_coat() -> (char, char, char)` | three inks | her coat as the joust's destrier is drawn (`hide`, `dark`, `mane`), for the lists and the horse market's pens | none |

White means light tonal greys and stone (`H i h V v`), shaded, never flat
`w`. The same picture stands in the Flower Fields beside the ranch's drafts
(about 0.7 of their scale reads right) and in the pen.

## 13. The pen, the map plaque icon, the establishing plate

| Function | Size | States | Glow |
|---|---|---|---|
| `pub fn pen(open: bool, tick: u32) -> Img` | at most 64 by 64, its foot on the bottom row | the Mire King's pen of bog timber lashed with rope, the horse standing inside (her 56 by 48 must read behind its bars); shut: a lock that holds; open: the gate swung wide | 32 (the lock, while it holds) |
| `pub fn plaque_icon() -> Img` | at most 16 by 16 | the swamp's emblem, on the plate's sign in Sir Damp's Marsh (a white horse's head over reeds reads well) | 4 |

**The establishing plate.** When the party arrives on a swamp floor, the
floor plate shows a 384 by 224 establishing shot titled SWAMP OF HORRORS.
Paint it in `crates/delve/src/render/overworld/arena/plates.rs` with that
file's `Brush`, as `fungal()` and `archive()` are painted: add
`fn swamp(depth: u32) -> (Brush, Lamp)` and replace the placeholder arm
`Kind::Swamp => fungal()` in `build`. Depths 1 to 3 may differ; the third is
the Mire King's hall with the pen.

## Checking it

```sh
cd crates
cargo test -p angel-delve swamp_art                       # the contract
cargo test -p angel-delve approved_bestiary_pixels_stay_exact
ANGEL_ARENA_SHOTS=/work/tmp/swamp-shots \
  cargo test -p angel-delve write_swamp_shots -- --ignored  # the review shots
```

`approved_bestiary_pixels_stay_exact` pins every foe's pixels: once the new
horrors and the King are approved, update their four entries (`v1-BogHand`,
`v1-Drowned`, `v1-Wisp`, `v1-the-mire-king`) in
`crates/delve/tests/render/fixtures/foe-art.json`. The review shots are the
Marsh's plate, a bog-and-fog fight on floors one and two with each horror in
its tell, the Mire King's hall with the pen shut and then open, and the
white horse at grass.

## What is borrowed now

| Function | Placeholder |
|---|---|
| `ground` | the world's meadow ground |
| `bog_field` | the realm's open water turned to murk, the world's mud through it |
| `reeds` | the Marsh's reeds (`material::reeds`), two to a tile |
| `mist` | nothing (the patch's darkness only) |
| `bog_hand` | the mole (mound under, surfaced risen) |
| `drowned` | the Oathbound grave-knight |
| `wisp` | the wraith |
| `mire_king` | the Rootwarden's rows (`fungal-rootwarden.boss`) |
| `white_horse`, `white_coat` | the joust's grey destrier (Mist's coat), saddle and all |
| `pen` | the rescue cage, enlarged |
| `plaque_icon` | a reed clump |
| establishing plate | the Fungal Deep's |
