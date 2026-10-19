# Moonfall: what the art wave paints

Blocks first: every actor, prop, building and the Gloam Moon ships as a
labelled block (drawn in `render/overworld/arena/moonfall/ground.rs` and `sky.rs`),
so the loop plays before any pixel art exists. This file is the request list:
**identity before pixels**, then the slots.

Concept sheets (read-only, painted by Luna): `~/angelX_minigames/games/moonfall/assets/`
(`npcs-a`, `npcs-b`, `knight-a`, `knight-b`, `aster-ride`, `buildings-a`, `buildings-b`,
`props-a`, `props-b`, `ground`, `keyart`). Look at them to anchor identity; do not
copy them into the repo.

## Identity

The town is a fen at dusk-gold and fen-green, kind and a little funny, never gory.
The Gloam is cold: pale aqua smoke, hooded wisps, a moon with a frown.

| Who | Silhouette | Materials | The one signature feature |
|---|---|---|---|
| The knights (seats 1 to 6) | a tall helm over a tabard, the party's house colours | steel, wool, one cloak | a seat colour on the tabard and a plume the colour of the house; walking is a rolling stride, a sidestep is a hop with the cloak out |
| Mira Quill | slim, plumed sallet, a book under one arm | aqua scarf, oxblood tabard with the moon crest | a quill behind the plume and a taper in the free hand (she is the lamplighter) |
| Tovin Reed | wide conical iron hat, a long pole | mossy green cloak, rope coil, key ring on the belt | a reed plume in the hat and a lantern on the pole |
| Nessa Flint | round and broad, a sallet helm | flour-white apron over a leather smith's apron, red tabard | a tray of loaves in front and a shield-board on her back |
| Orin Wren | thin, stooped, always looking down | ink-black fingers, a long coat with ink stains | a quill in each hand and a posture that is searching |
| Pella Starling | upright, a star on the tabard | gold and white, a brass horn | the horn on a lanyard and a rolled scroll |
| Aster Vale | a rider on a tired horse | lantern-rider leathers, a saddle bag of oil | the lantern pole, and the horse that limps (the near hind hoof is bare) until he is shod |
| Wisps | a hooded blob, shoulder-height | pale aqua smoke, two pale eyes, a wick-stub tail | the two pale eyes; when one reaches a light it leans in and the light dims |
| The Gloam Moon | a huge disc with a face | amber-lilac in the key art | a menacing frown and teeth; it grows and sinks each night (`far`, `near`, `close`), `falling` is the wipe, `turned` the end |
| The towers and the Moonwell | a bell tower (north, south) and a stone ring with a bell post | grey stone, aqua water, a flame bowl on each | the flame bowl: dim until the Third Midnight, then lit |

Hitboxes are the rules' own (`rules/moonfall/town.rs`, `walk.rs`): a knight's feet are
an 8 by 6 box, a touch is 9 pixels, a reach is 26. **Art never sets a collision shape**;
the `hitbox` entries in `slots.json` only say what the rules use today.

## The slots

`slots.json` is the machine list (canvas, bottom-centre anchor, hitbox, animation
timing in ticks at 30 Hz). Each slot has an inactive map in
`assets/art-slots/<id>/<id>.map.json`; the ordinary import pipeline fills
`<state>-<index>.png` frames, enables the runtime mapping and rebuilds. The stage
looks a frame up through the slot art registry and draws the block when there is
none, so a half-painted set plays.

| Slot | Canvas | Label | States (frames x ticks) |
|---|---|---|---|
| `moonfall.knight` | 16x24 | KNIGHT | idle-down 1x12, walk-down 4x5, roll-down 3x2, idle-up 1x12, walk-up 4x5, roll-up 3x2, idle-left 1x12, walk-left 4x5, roll-left 3x2, idle-right 1x12, walk-right 4x5, roll-right 3x2, snuffed 2x10, lift 2x8 |
| `moonfall.mira` | 18x28 | MIRA | idle-down 1x12, walk-down 4x6, idle-up 1x12, walk-up 4x6, idle-left 1x12, walk-left 4x6, idle-right 1x12, walk-right 4x6, talk 2x8, work 4x8, look-up 1x12, light 3x6 |
| `moonfall.tovin` | 18x28 | TOVIN | idle-down 1x12, walk-down 4x6, idle-up 1x12, walk-up 4x6, idle-left 1x12, walk-left 4x6, idle-right 1x12, walk-right 4x6, talk 2x8, work 4x8, look-up 1x12, raise 3x8 |
| `moonfall.nessa` | 18x28 | NESSA | idle-down 1x12, walk-down 4x6, idle-up 1x12, walk-up 4x6, idle-left 1x12, walk-left 4x6, idle-right 1x12, walk-right 4x6, talk 2x8, work 4x8, look-up 1x12, hammer 4x6 |
| `moonfall.orin` | 18x28 | ORIN | idle-down 1x12, walk-down 4x6, idle-up 1x12, walk-up 4x6, idle-left 1x12, walk-left 4x6, idle-right 1x12, walk-right 4x6, talk 2x8, work 4x8, look-up 1x12, write 4x8, search 4x8 |
| `moonfall.pella` | 18x28 | PELLA | idle-down 1x12, walk-down 4x6, idle-up 1x12, walk-up 4x6, idle-left 1x12, walk-left 4x6, idle-right 1x12, walk-right 4x6, talk 2x8, work 4x8, look-up 1x12, cry 4x6 |
| `moonfall.aster` | 34x32 | ASTER | idle-left 1x12, idle-right 1x12, ride-left 4x6, ride-right 4x6, lame-left 2x10, pour 3x8 |
| `moonfall.wisp` | 18x22 | WISP | drift 4x6, snuff 6x5, shy 2x4 |
| `moonfall.lantern` | 16x34 | LAMP | unlit 1x12, lit 3x7, snuffing 3x6 |
| `moonfall.crate` | 16x16 | CRATE | idle 1x12, push 2x5 |
| `moonfall.moonwell` | 40x44 | MOONWELL | idle 1x12, oiled 2x10, lit 4x6 |
| `moonfall.bell` | 24x36 | BELL | idle 1x12, ring 6x4 |
| `moonfall.bridge` | 56x36 | BRIDGE | down 1x12, rising 3x6, up 1x12 |
| `moonfall.planks` | 16x36 | PLANKS | idle 1x12, creak 2x6 |
| `moonfall.gate` | 20x52 | EAST GATE | closed 1x12 |
| `moonfall.podium` | 26x26 | PODIUM | idle 1x12, cry 4x6 |
| `moonfall.ink-pot` | 12x12 | INK | idle 1x12 |
| `moonfall.reeds` | 16x18 | REEDS | idle 1x12 |
| `moonfall.flame` | 28x40 | FLAME | dim 2x10, lit 4x5, snuffing 3x5 |
| `moonfall.tower-north` | 80x96 | NORTH BELL TOWER | day 1x12, night 1x12, lit 1x12 |
| `moonfall.tower-south` | 80x96 | SOUTH BELL TOWER | day 1x12, night 1x12, lit 1x12 |
| `moonfall.archive` | 80x76 | ARCHIVE | day 1x12, night 1x12, lit 1x12 |
| `moonfall.bakery` | 64x76 | BAKERY | day 1x12, night 1x12, lit 1x12 |
| `moonfall.forge` | 48x76 | FORGE | day 1x12, night 1x12, lit 1x12 |
| `moonfall.herald-house` | 80x76 | HERALD HOUSE | day 1x12, night 1x12, lit 1x12 |
| `moonfall.scriptorium` | 80x76 | SCRIPTORIUM | day 1x12, night 1x12, lit 1x12 |
| `moonfall.bridge-hut` | 48x60 | BRIDGE HUT | day 1x12, night 1x12, lit 1x12 |
| `moonfall.cottage` | 48x72 | COTTAGE | day 1x12, night 1x12, lit 1x12 |
| `moonfall.inn` | 144x60 | INN | day 1x12, night 1x12, lit 1x12 |
| `moonfall.tiles` | 16x16 | TILE | grass 1x12, cobble 1x12, water 4x10, reeds 1x12, hedge 1x12, ditch 1x12, filled 1x12, bridge 1x12, plank 1x12, gate 1x12 |
| `moonfall.moon` | 96x96 | GLOAM MOON | far 1x12, near 1x12, close 1x12, falling 2x6, turned 1x12 |
| `moonfall.sun-marker` | 12x12 | SUN | idle 1x12 |
| `moonfall.moon-marker` | 12x12 | MOON | idle 1x12 |
| `moonfall.ledger` | 300x150 | LEDGER | page 1x12, turn 2x3 |
| `moonfall.portrait-mira` | 36x36 | MIRA | idle 1x12, speak 2x8 |
| `moonfall.portrait-tovin` | 36x36 | TOVIN | idle 1x12, speak 2x8 |
| `moonfall.portrait-nessa` | 36x36 | NESSA | idle 1x12, speak 2x8 |
| `moonfall.portrait-orin` | 36x36 | ORIN | idle 1x12, speak 2x8 |
| `moonfall.portrait-pella` | 36x36 | PELLA | idle 1x12, speak 2x8 |
| `moonfall.portrait-aster` | 36x36 | ASTER | idle 1x12, speak 2x8 |
| `moonfall.mask-swift` | 16x16 | SWIFT | idle 1x12, worn 2x8 |
| `moonfall.mask-stout` | 16x16 | STOUT | idle 1x12, worn 2x8 |
| `moonfall.candle` | 8x14 | CANDLE | lit 2x6, out 1x12 |
| `moonfall.item-ink` | 12x12 | INK | idle 1x12 |
| `moonfall.item-reeds` | 12x12 | REEDS | idle 1x12 |
| `moonfall.item-pass` | 12x12 | PASS | idle 1x12 |
| `moonfall.item-scroll` | 12x12 | SCROLL | idle 1x12 |
| `moonfall.item-shoe` | 12x12 | SHOE | idle 1x12 |
| `moonfall.item-key` | 12x12 | KEY | idle 1x12 |
| `moonfall.item-seal` | 12x12 | SEAL | idle 1x12 |
| `moonfall.card` | 300x48 | DAWN CARD | fade 4x6 |

The knight is one slot recoloured per seat (a seat mask frame, as the other
adventures do), facing four ways. Folk face four ways with `walk` four frames at
six ticks. `moonfall.tiles` is a sheet of sixteen-pixel ground tiles
(one frame per state; water is four frames). The buildings may stand taller than
their footprint in the town map; their bottom edge is the footprint's south edge.

## Notes for the painter

- The sky is drawn in code (a gradient, stars, a roofline); the moon is the only
  sky art. It is drawn over the town with half its pixels dropped where it hangs
  below the roofline; paint it opaque and the stage will do the same.
- The night grading is a palette step per pixel; paint at day values and the
  stage darkens them. Lamplight and window light are signal inks and stay bright.
- The Ledger page art (`moonfall.ledger`) is the whole open book: the stage writes
  the lines on it, so leave the page area plain.
- Six portraits (36x36) sit in the conversation box; they speak with two frames.
