# Lightgallop art: identity before pixels

Everything on the Moonwell Road ships as a **labelled block** now
(`slots.json`: canvas, bottom-centre anchor, collision geometry for reference,
animation timing). The game plays in full with blocks. The art wave paints the
slots below, runs the native-pixel import, and rebuilds; no rule changes.

**Art never sets a hitbox or the terrain.** The session's numbers do (a rider's
body is 14 wide and 28 tall, 14 tall rolled; foes and props have the boxes in
`rules/lightgallop/world.rs`). The `hitbox` lines in `slots.json` are a guide to
what the box is, so the picture can sit right inside it. The road is drawn from
the very curves the sim rides: the surface art tiles *along* the curve
(`lightgallop.road`, `.rail`, `.arch`), it never decides where the surface is.

Concept sheets painted for this wave sit in
`~/angelX_minigames/games/lightgallop/assets/raw/` (read-only
references: `keyart`, `rider-gallop`, `rider-actions`, `foes`, `props`,
`tex-road`, three backgrounds). Anchor on them; do not copy them into the repo.

The realm in one line: a dark-fantasy kingdom at dusk whose purpose is to spread
light. The Gloam snuffs lights. Kind, warm-lit, a little funny; never gory.

## The rotation note (the loop)

The rider is rotated about the body's centre by the lean the rules keep
(`Rider::ang`, 1/1024 of a turn, eased so it never jumps), on every surface: a
slope, the loop's wall and its ceiling, the coil. So the rider art needs **no
loop frames**: paint the horse upright, facing right, and the renderer turns it
(nearest sample at pixel centres, integer maths, identical on every host). Keep
the silhouette readable at 45, 90 and 180 degrees (a knight upside down on the
ceiling must still read as a knight). A left-facing rider is the same art
mirrored first. Pivot: (24, 24) in the 48 by 40 canvas, which is the body's
centre; the hooves are 14 pixels below it on the surface's normal.

## The slots

| slot | canvas | anchor | what it is |
|---|---|---|---|
| `lightgallop.rider` | 48x40 | (24,40) | the mounted knight |
| `lightgallop.snuffer` | 28x30 | (14,30) | a hooded wisp walking a beat on the road |
| `lightgallop.floater` | 28x28 | (14,28) | the same wisp drifting in the air |
| `lightgallop.bramble` | 32x22 | (16,22) | a low tangle of thorns |
| `lightgallop.warden` | 64x112 | (32,112) | the Bell Gate's guardian |
| `lightgallop.hay` | 36x22 | (18,22) | the hay-bale spring |
| `lightgallop.post` | 26x56 | (13,56) | the lantern post (checkpoint); 26 wide so the painted post keeps its natural width |
| `lightgallop.spark` | 10x10 | (5,5) | a spark, the knight's health |
| `lightgallop.bell` | 48x72 | (24,72) | the goal bell on its yoke |
| `lightgallop.pad` | 40x8 | (20,8) | the boost pad |
| `lightgallop.crate` | 24x24 | (12,24) | the lantern crate |
| `lightgallop.dimming` | 64x128 | (64,128) | the Dimming's leading edge, a tile |
| `lightgallop.sign` | 56x28 | (28,28) | a roadside plank |
| `lightgallop.arch` | 24x16 | (12,8) | one tile of the loop's stone track |
| `lightgallop.road` | 64x48 | (0,0) | a tileable road surface |
| `lightgallop.rail` | 32x10 | (0,0) | a tileable plank for the high rails |
| `lightgallop.layer-far/mid/near` | 192x96 | (96,96) | parallax land, three speeds |
| `lightgallop.fx-dust` | 16x16 | (8,16) | hoof dust |
| `lightgallop.fx-burst` | 24x24 | (12,12) | a snuffer's pop |

States and frame timings (ticks at 30 Hz) are in `slots.json`; the main ones:

| slot | states (frames x ticks each) |
|---|---|
| rider | gallop 6x3, idle 2x8, jump-rise 1x6, jump-fall 1x6, roll 4x2, crouch 1x6, charge 2x2, spur 4x2, lunge 1x4, skid 1x4, hurt 2x6, victory 3x8 |
| snuffer | idle 2x8, alert 2x8, hurt 1x6, pop 4x3 |
| floater | idle 2x8, alert 2x6, pop 4x3 |
| warden | idle 2x10, hit 1x8, roar 1x12, defeat 4x4 |
| hay | rest 1, sprung 4x3 |
| post | dark 1, lit 4x4 (flicker) |
| bell | rest 1, ring 6x4 (swings out and back) |
| spark | idle 4x4, loose 2x2 (the flicker before a scattered spark fades) |

The gait quickens with the pace: the renderer advances the gallop one frame for
every 11 pixels run, so the frames must loop cleanly at any rate. Eight
gallop frames would suit (the concept sheet has eight); the blocks use six.

## Identity, one signature feature each

- **Mounted knight** (the Red Sun Knight on a bay war-horse, or the knight's
  own house kit in the colour of the seat). Silhouette: a long, low horse with
  a high-set knight leaning into the run, a swept red plume streaming back.
  Materials: bay hide, black mane and tail, worn steel, a red caparison with
  gold trim and small gold suns, a heater shield on the near arm. Signature: the
  **plume and caparison hem stream backward** and lengthen with speed. Poses to
  paint: gallop cycle; leap rise (front legs folded) and fall (front legs
  reaching); the tuck roll (rider and horse tucked into a tight ball, readable
  at 14 pixels); the crouch and the charge (horse down on its haunches, hooves
  scraping, shaking with the rev); the spur (stretched flat out, head low); the
  lance lunge (a lance levelled forward, a small red pennant); the skid
  (hooves planted, dust); the hurt stumble; the victory (lance raised).
  Each seat tints the surcoat and plume with its own colour (the blocks use
  blue, tan, green, rust, pale, lime).
- **Snuffer**: a small floating shadow-wisp shaped like a snuffed candle flame
  in a pointed candle-snuffer hood, **two round pale eyes**, stubby smoke arms,
  a curled burnt wick for a tail. Cute and a little grumpy. Violet-black smoke.
  Alert pose leans forward, arms out. Pop: a ring of cold pale-blue smoke with
  **one warm gold spark** at its heart (the spark returns to the knight).
- **Floater**: the same wisp, tail trailing, bobbing; it is the lance-lock
  target, so its eyes must read from 90 pixels away.
- **Bramble**: a low black thorn tangle, a couple of dull-red berries, base flat
  on the baseline; the taller second version has longer curving thorns.
- **Warden**: a tall hooded snuffer, three times the wisp's height, a wick-staff
  in one hand and a lantern-hook in the other, eyes that dim and flare with its
  hits. Signature: **its hood tips toward the knight** that struck it last.
- **Hay bale**: a big round golden bale on its side bound with red cord, a small
  gold eight-point sun on the round face; sprung it flattens and bursts straw up
  and out. The diagonal bale is the same bale turned.
- **Lantern post**: a slender wrought-iron post with a curled hook holding a
  glass lantern. Dark: cold grey-blue, a spent wick. Lit: a warm gold flame,
  soft glow, a slow flicker. Signature: the lantern **spins as it catches** (the
  renderer draws a ring of light closing in for the first second).
- **Spark**: a small four-pointed golden mote with a white core, twinkling;
  loose sparks (after a hit) blink in their last second.
- **Bell**: a large bronze bell on a short carved timber yoke with two posts, a
  faint gold sun on its side; ringing, it swings out about 25 degrees with
  little gold sound-sparkles.
- **Boost pad**: a worn stone slab set in the road with three chevrons pointing
  right, glowing aqua when fired.
- **Lantern crate**: a small timber crate with a lantern-glass window; the
  window's colour says what it holds (blue shield, gold sparks, amber spur).
- **The Dimming**: a wall of violet-black smoke with pale eyes drifting in it, a
  ragged swaying leading edge that thins to dither where it meets the road.
  Tile it down the screen; the front is the sim's number.
- **Road, rails and loops**: the road is packed pale earth and flat cobbles with
  moss in the gaps, a fringe of grass blades on top, layered dark soil beneath;
  the rails are timber planks on trestles; the loop and the coil are a great
  weathered stone ring-track, moss-grown, lit by a lantern at its foot. Texture
  tiles repeat along the curve and must meet seamlessly at both ends.
- **Parallax**: far hills under a great pale-aqua moon with violet and ember sky
  bands; mid dark pines with a few lit windows; near reeds, fence posts, fireflies.

## Sound cues

The sim names existing cues only (`step_wood` leap, `roll`, `shot_spark` for each
rev tap, `engine_whistle` for the charge's release, `spell_fire` for the spur's
bite, `step_gravel` skid, `wheel_spin` for the loop's rising whoosh, `wall_up`
a spring, `vent_fire` a pad, `circle_toll` a lantern or the bell, `hit_magic`
`card_pickup` `gold` `spoil_pickup` `wish_grant` a lance-lock chain rising a
step per link, `gold` a spark, `hero_hurt` with `spoil_pickup` a scatter,
`knight_fall`, `boss_roar` the Dimming, `boss_death` the warden's fall).
A later audio pass may want its own: a hoof-gallop loop whose rate follows the
pace, a true rising whoosh for the loop, a coin-chime ladder for the chain,
a lantern-catch chime, a low rumble for the Dimming that swells as it nears.
