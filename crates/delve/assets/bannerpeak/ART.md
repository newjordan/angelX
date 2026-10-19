# Bannerpeak art (for the second wave)

Every actor, block and prop ships now as a labelled block in
`render/overworld/arena/bannerpeak.rs` (each in its own function named for its
slot). `slots.json` is the contract: per slot the native canvas, the bottom-centre
anchor, the rules' hitbox, the placeholder ink, and each state's frame durations
in ticks (30 to the second). **Art never sets a collision shape.** The hitbox
column is what `rules/bannerpeak/*` uses; a painted frame is drawn on the anchor
and may spill past its box. Nothing scales in play: native pixels, one grain.

The stage is 384 by 224 native pixels; a tile is 16. The small knight's body is
12 by 20, the big 14 by 30, a snuffer 14 by 14, King Cinder 48 by 64. A knight is
about a tenth of the screen's height, so a pose must read at that size.

Concept sheets (painted, read-only references; look, do not copy into the repo):
`~/angelX_minigames/games/bannerpeak/assets/raw/` (`hero-air.png`,
`hero-ground.png`, `snuffer.png`, `cinder.png`, `props.png`, `keyart.png`, `sky-dusk.png`,
`tex-grass-marsh.png`, `tex-mossy-stone.png`, `tex-boardwalk.png`).

## Identity before pixels

One line each: silhouette, materials, the one feature a player knows it by.

- **Knight** (`knight`, `knight-big`, `knight-flame`): a helmeted knight with a
  long plume that streams behind the run and snaps forward on a turn; a
  heater shield on the arm (the guard, the slam, the pole). Seat colour on plume,
  tabard trim and shield rim; four-tone seat ramps baked with the poses, plus a
  one-pixel seat contour so YOU read against friends. Signature: the plume. Big
  is the same knight a head taller in a full cuirass. Flame carries a small
  lantern in the near hand.
- **Snuffer** (`snuffer`, `wisp`): a hooded wisp, a pointed brimmed hood over
  two big pale eyes, a smoke tail and a wick-stub that glows at its end. Signature:
  the eyes, which spiral when stomped. Pops into cold smoke and
  a returned spark of light, never gore.
- **Capped snuffer** (`capped`, `cap`): the same wisp under a dented iron cap.
  The cap is a separate bowl of iron that can spin along the floor, be carried
  (it rattles in the last thirty ticks of a carry) and wake.
- **King Cinder** (`cinder`, `crown`, `cinder-wave`, `cinder-ember`): a round
  coal-black king in a charred red cloak with a clasp, a jagged ember crown with
  red gems, and a bronze horn-headed staff. Signature: the crown (three of them in
  turn; a plate comes off per slam), and he weeps when it falls. His body is soft
  and rounded, never frightening; the tell (raised staff, flared nostrils, glowing
  gems) is the loudest thing on the screen for the tell's whole length.
- **Banners** (`banner`, `great-pole`, `door`): crimson pennants with a crescent and
  a stone well, eight colourways as the pole's slots. Signature: the crescent. The
  Great Pole is a tall mast with a yard and eight pegs; the banner rides up it
  in six-tick steps after the item-get pose.
- **Gate and flagpole** (`gate`, `flagpole`): the checkpoint gate is a small
  pennon post; the flagpole is a taller staff with a banner that climbs as the
  knight slides.
- **Sparks** (`spark`, `ring-spark`): an eight-pointed star of warm light that bobs;
  the ring spark is the same star in a turning ring so a player knows it from a
  coin at a glance.
- **Rune block** (`rune-block`, `brick`, `cracked`): moss-stained stone with a
  carved rune that glows from within; struck, it lifts eight ticks and goes dull.
  A cracked block shows a bright seam.
- **Loaf and flame** (`loaf`, `flame`, `bolt`): a warm round hearth loaf with a
  slit; a lantern with a live flame (the glass clear and bright), bolts of light
  that bounce low.
- **Bell boulder** (`boulder`): a giant verdigris bronze sleigh-bell with a dark
  slit that rolls; its lip tolls thirty-six ticks before one rolls out.
- **Waterwheel and logs** (`wheel`, `log`): a mossy iron-bound wooden wheel with six
  paddle buckets (the platforms), wet and dripping; the log is a bark-ringed round.
- **Tiles** (`tiles`, `water`, `arch`): mossy stone, packed snow (slippery, a pale
  edge), a one-way wooden ledge with pegs, spikes, a cellar hatch with a ring, a
  tapestry door of the same crimson cloth. Stone takes a tint per section (the
  courtyard's cool, the marsh's green, the mill's grey, the shaft's dark, the
  snow's blue, the summit's ash).
- **Scenery** (`sky`): parallax of a dusk moon over mountain ranges, a bell tower
  among cliffs and waterfalls, mist over the marsh with firefly lights. Three layers
  at one quarter, one half and three quarters of the camera.

## What art must keep

- The seat's own colour on every knight pose; the knight is always the most
  readable thing on the screen.
- Tells: Cinder's staff and gems, the bell's toll, a cracked block's seam. A tell is on screen for its whole rules-length (`slots.json`) and never less.
- Hazards unmistakable: spikes, embers and sweeps are warm and hard-edged; every
  friendly thing (blocks, banners, sparks) is warm and soft.
- Effects are solid body layers over the picture (`fx`: coin, pop, dust, ring,
  bump, debris, gleam, smoke), fourteen ticks long.
- Seat colours, terminal cells and the 96 by 28 cell picture read the same as the
  block placeholders do; opening the `write_bannerpeak_review` pictures after
  every art import is the check.

## Poses of the knight, by name

idle, run (advances by speed), skid, crouch, slide, rise, fall, second (the second
hop), triple (the somersault), long, backflip, wall, slam-wind, slam, slam-land,
dive, belly, hurt, brace, item-get, pole, bubble. The sim picks the pose
(`body::Pose`); art maps a name to a frame set.

## Reproduce the review

`ANGEL_ARENA_SHOTS=<dir> cargo test -p angel-delve --lib write_bannerpeak_review
-- --ignored` writes every moment at 1x and 3x and the terminal layers; the
cockpit's `write_roundtable_terminal_review` writes the 96 by 28 cells.

## Painted slots

Each has a map in `assets/art-slots/bannerpeak.<id>/` (an ordinary import, see
`docs/SLOT-ART-IMPORT.md`; a local `request.json` beside it, since Bannerpeak is
not in the shared request inventory) and the stage reads a frame by
`bank::at(id, state, tick)` or `bank::pic(id, state, index)`. A slot is painted
whole or not at all: if any frame of any state is missing, every state of it is
the labelled block. Durations are `slots.json`'s.

| Slot | State |
|---|---|
| `sky` | On. One static picture under the tiles, props, foes and knights, drawn under a dark layer of a quarter (`KEEP` in `bannerpeak/sky.rs`), fading to the flat deep dark from the street's stone down so the cellars are not moonlit. It does not scroll, so the per-section mountain tints are gone. |
| `tiles` | On: stone, snow, ledge, spikes, hatch (shut). `hatch-open` is imported, not played: the rules keep no open hatch. The stage has never tinted tiles by section (only the old backdrop's mountains), so the neutral painted stone has no tint to fight. |
| `rune-block` | On: idle, bump (eight ticks) and spent. One picture for coin, power and many. |
| `boulder` | On: the roll frames, counter-clockwise (it rolls west). The toll frames are imported, not played: the rules' toll is a sound 36 ticks before a ball rolls out (`circle_toll`), with no boulder to paint yet, and `slots.json` gives `toll` 18 ticks, not 36. |
| `spark` | On: the bob frames. The take frames are imported, not played (a taken spark is simply gone). |
| `flame` | On: rise over the item's sixteen rising ticks, then idle. |
| `great-pole` | On, 72x136 (the slot's 64 scaled by height is 72). The code's eight banner places hang from it. |
| `gate` | On, 30x40 with the post at x=8 and the pennon spilling right: down until raised, the up flutters after. The raise frames are imported, not played: no rules clock for the climb. |
| `wheel` | Imported, off. The rules' six 28 px planks (radius 40, 270 ticks a turn, clockwise) and the painted buckets visibly disagree: the six frames are about 0, 9, 13, 21, 30 and 37 degrees, so the nearest is up to about 12 degrees off a plank, the paddles are radial where a knight stands on a flat plank, and the planks peek out round them. A wheel painted at 60 degree steps with flat bucket tops would fit. |
| `flagpole` | Imported, off. The rules grab anywhere on nine tiles (144 px) and the block pole is as tall; the painted pole is 100 px, so a knight on the stair beside it grabs in the air above its tip. `slots.json` documents a 96 px box. A pole drawn 144 px tall would need its pennon (about 65 px wide at that scale) looked at again. |
