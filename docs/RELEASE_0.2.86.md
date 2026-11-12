# angelX 0.2.86: the caves update

Knights Wish gets a living cave under its first floor, mines that ship
stone home to the kingdom, and rock that burns, steams and melts. The coding
harness is the same as 0.2.85.

## Upgrade

```sh
angelX update
```

Or install fresh with the one line in the [README](../README.md). From a source
checkout, `git pull` and run `./bin/angelX` again.

## The cave

Stairs in the first floor's start room and in the kingdom yard lead down into
one seeded cavern of four regions, the mine, the foundry, the frost and the
hollow, each with its own stone. The knight's lamp lights the rock around
them and falls off into the dark.

- **Digging.** The shovel cracks rock, which crumbles to rubble or blasts
  open with magic, in each region's colour. The stone it gives goes to the
  home stockyard. A dig to the north cracks the rock above the knight, and
  nothing is drawn over them.
- **The mouth** is a boulder arch over a stone stair.
- **Ice slides.** Grip boots and blades lie on the cave floor as pickups and
  set how far a knight slides.
- **Every knight is drawn as themselves** in the cave, with a spade.
- Guests and late joiners see the same cave, and a walking frame draws in
  about 1.5 ms.

## Mines

Every cave holds eight seeded ore bodies, drawn as veins in their region's
colours. See eight cells of one and it is proven; press E at it to sink a
mine, and a timbered head goes into the rock. The kingdom takes a claim on
that region's seam and raises a quarry at one of eight sites on the surface,
and carts haul its stone to the stockyard. The cave and its mines belong to
the home, so they stay across runs and in a hosted game.

## Materials

- Fire spreads through wood, grass and fungus and leaves ash with fading
  embers.
- Water puts fire out in a burst of steam.
- Acid eats into walls.
- Lava lights wood and melts ice into steam.
- Pools of lava, water and acid are drawn as glowing crust, ripples and
  fizz.

A burning or melting cell keeps its state through a host rebuild, and the
fluid step visits only the cells that are moving, about 6 µs a tick.

## Also in Knights Wish

- **The wish scroll.** T opens the genie's parchment scroll with its three
  choices.
- **Wished sprites move.** A sprite drawn at game size is rigged into walk,
  cast and swing clips.
- **The Keep** rises stone by stone from its yard, and each stone keeps its
  kind, so the walls band limestone, green-veined stone, basalt and
  ice-stone.
- **C** climbs on a friend's shoulders on every keyboard, and **Tab** opens
  the cards.
- A knight picked at the pub shows over a wished whole look.
- The crossroads race between floors is a quarter-lap sprint against a 90 s
  clock.
- Griffin Run and the Ore Cart move every rider again.
