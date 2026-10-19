# Bannerpeak

A side-view scrolling platformer for the whole table, with the later
three-dimensional entry's move vocabulary on the same flat ground, a courtyard
hub, and eight banners to bring home. Slug `bannerpeak`, door at kingdom tile
(174, 23). The setting is Moonwell Keep's courtyard and the mountain behind it.

## The feeling
A run that gathers speed and a jump that leaves the ground the same tick, so a
knight can feel the difference between a hop, a long leap and a somersault.
First ten seconds: a sign says RUN. JUMP.; hold right, press space, and the
knight is already in the air over the first sparks.

## The loop (one sentence)
Run and jump the courtyard and Moonwell Mountain, earn eight banners by eight
different goals (the Great Pole shows the count), and bring down King Cinder's
crown three times; a fall costs a wedge and puts the knight back on the last safe
ledge in under half a second.

## Words on the title card
RAISE EIGHT BANNERS

## Who plays
Every seated knight, at once, on their own camera, in one shared level. Foes,
gates, blocks, doors and banners are the party's: one knight's gate is
everyone's checkpoint, a banner raised is raised for all. A knight who joins
mid-run sits down at the shared gate. Health is a pie of eight wedges per knight
(six to start). A hundred gold sparks refill it, twenty give a wedge.
- More seats, more pressure, never weaker foes: 3 or more seats add extra
  snuffers (marks `e`), 5 or more add extra capped snuffers (marks `f`); bell
  boulders roll every 300 ticks alone and every 180 at six seats, a little faster
  for each seat, with a second beat from 4 seats; King Cinder's crown carries a
  plate for every seat (a slam from any knight takes one off).
- Rescue bubble: a knight who loses the last wedge while a friend is up floats
  in a bubble for 11 seconds; a friend who reaches it brings them back with three
  wedges, and if nobody does the last gate takes them back. When every knight is
  down, 20 ticks later all are at the last gate with at least four wedges.
- Optional: a knight holding the guard (E) is a ledge. A friend can land on the
  shield and jump from it for a higher spring. Never required: the tower's
  banner has the triple-jump lane as well.
- Hold Esc retreats, as everywhere; the run goes on with whoever is left.

## Controls
Existing buttons only. A and D run (acceleration, a skid on the turn). Space or
W jumps (hold it for height, tap for a hop). S crouches, and in the air S is the
ground pound. F dives in the air, and on the ground picks up a cap, carries it
while held and throws it on release; with a lantern flame F throws a bolt. E
holds the guard (a shield) and, at an arch, a gate or the course board, opens the
board. Hold Esc returns to the kingdom. No new `Input` bit.

| Move | Keys |
|---|---|
| hop, high jump | space, held longer is higher; speed adds height |
| triple jump | three landings in rhythm while running, each higher |
| long jump | crouch (S) while running, then jump |
| backflip | crouch standing still, then jump |
| wall kick | against a wall in the air, jump; between two close walls, up the shaft |
| ground pound | S in the air: a hover, a drop, a shock and a hit-stop |
| dive and belly slide | F in the air |
| stomp | land on a snuffer; hold jump for a higher bounce |
| kick or carry a cap | F beside a spent cap; release to throw it |

## What the player finds
- Courtyard hub: the Great Pole with eight banner slots that fill in, two banners
  in the courtyard (the tower, by the triple-jump lane to its roof, and the
  cellar under a cracked floor that a slam breaks), a tapestry arch that opens at
  two banners and the course board behind it.
- Moonwell Mountain: the marsh bank (logs, rune blocks, a bog to leap, a cellar
  hatch down to a bonus room, a flagpole where the height of the grab scores and
  the knight slides down), the mill-race and its waterwheel (buckets to ride, a
  backflip for a spark), the bell shaft (wall kicks up two close walls), the snow
  run under rolling bell boulders, and the summit.
- Rune blocks: coins (one, or up to five from a many-block), a hearth loaf that
  rises out of its block (small becomes big, big is mended two wedges) and a
  lantern flame for a big knight (bouncing bolts of light). A hit takes a size
  before a wedge.
- Snuffers walk, capped snuffers leave a cap when stomped (kick it, carry it),
  wisps float. Pits and the bog cost a wedge and return the knight to the last
  safe ledge.
- The course board lists six lines (marsh flag, mill race, bell shaft, snow run,
  eight sparks, King Cinder); each banner is announced with an item-get pose and
  a rising fanfare; the eight ring sparks hidden along the way make a banner of
  their own. The summit door opens at seven banners.
- King Cinder cannot be hit head on. His tell-then-strike pattern (a stomp wave,
  a floor sweep, embers on the knights) runs, he kneels, and a slam onto the
  crown from any knight takes off a plate. Three crowns, each a harder pattern.
- The eighth banner is a short warm finale at the Great Pole. E returns the
  party to the kingdom.
- The camera leads the way a knight is travelling, looks down on a fall, and
  never loses the knight.

## The five numbers
Integers; HZ is 30; positions are pixels, speeds 1/256 pixel a tick. F2 knobs
`bannerpeak_run`, `_hop`, `_coyote`, `_slam`, `_ledge`:
1. `bannerpeak_run` = 1152, range 960 to 1664: top run speed. It sets the length of
   a run jump (135 px), a long jump (214 px) and the height of every jump.
2. `bannerpeak_hop` = 10, range 4 to 30: ticks after a landing in which the next
   hop of the triple jump may begin. Wider forgives; narrower is a rhythm game.
3. `bannerpeak_coyote` = 5, range 2 to 9: a jump pressed this many ticks before a
   landing is kept, and one pressed this many ticks after stepping off a ledge
   still takes (the brief's buffer 5, coyote 4 to 8).
4. `bannerpeak_slam` = 36, range 24 to 96: the ground pound's shock radius in
   pixels. It breaks cracked floor and tumbles the small foes about the landing.
5. `bannerpeak_ledge` = 3000, range 1200 to 4000: how fast a raised shield springs
   a friend upward, 1/256 pixel a tick.

## Tuning table
Measured by `write_bannerpeak_feel_table`, `write_bannerpeak_tuning` and
`write_bannerpeak_slam_reach` (ignored tests in the feel and play files; run with
`-- --ignored --nocapture`), one number moved at a time, the other four at their
shipped values. "Bots" is the scripted party playing the whole adventure through
`Session`: ticks at 1 seat / 4 seats (30 to the second); `fail` is no finish in
9000 ticks.

Run. The bots' look-ahead and hint thresholds assume 1152, so a bot failing at
another run speed is the bot, not the level; the jump lengths are what bound the
range. The marsh bog is 160 px wide.
| run | run jump | long jump | high jump rise | bots |
|---|---|---|---|---|
| 640 | 70 px | 44 px (none at this speed) | 67 px | fail / fail |
| 896 | 102 px | 165 px (barely clears the bog) | 70 px | fail / fail |
| 960 | 109 px | 176 px | 70 px | fail / fail |
| **1152** | 135 px | 214 px | 73 px | 6683 / 5123 |
| 1408 | 165 px | 252 px | 76 px | fail / fail |
| 1664 | 202 px | 252 px | 79 px | fail / fail |
| 2048 | 256 px | 271 px | 84 px | fail / fail |

Below 960 the long jump cannot make the bog by a safe margin, so the range begins
there. Above 1664 a run jump clears the bog by itself and the marsh stops being
the place to learn the long jump; the range ends there. Neither bound is
bot-proved.

Hop window: the rise in pixels of three hops landed in rhythm, by the ticks
waited between landings.
| hop | wait 0 | wait 6 | wait 12 | bots |
|---|---|---|---|---|
| 0 | 73 73 73 | 73 73 73 | 73 73 73 | fail: the tower needs the chain |
| 4 | 73 88 124 | 73 73 73 | 73 73 73 | 6533 / 5521 |
| **10** | 73 88 124 | 73 88 124 | 73 73 73 | 6683 / 5123 |
| 20 | 73 88 124 | 73 88 124 | 73 88 124 | 6683 / 6051 |
| 30 | 73 88 124 | 73 88 124 | 73 88 124 | 6683 / 6051 |

The range begins at 4 because 0 makes the tower unreachable. A wide window makes
a bot chain by accident and wander (a fifth slower at four seats).

Coyote and buffer: a jump pressed N ticks before landing is kept; pressed N ticks
after stepping off a ledge it takes.
| coyote | kept early | takes late | bots |
|---|---|---|---|
| 2 | 2 | 2 | 6683 / 5123 |
| 3 | 3 | 3 | 6683 / 5123 |
| **5** | 5 | 5 | 6683 / 5123 |
| 7 | 6 (the rig's short hop falls for six) | 7 | 6683 / 5123 |
| 9 | 6 (same) | 9 | 6683 / 5123 |

The bots press exactly, so they cannot feel this one. Under 4 a walk-off jump
becomes a thing to time; over 8 a knight seems to hang in the air.

Slam reach: whether a snuffer standing d px from where the knight lands is beaten.
| slam | d 10 | 20 | 30 | 40 | 60 | 90 | bots |
|---|---|---|---|---|---|---|---|
| 16 | no | no | no | no | no | no | fail: the cellar floor is not broken |
| 24 | no | yes | no | no | no | no | 6683 / 5123 |
| **36** | no | yes | yes | no | no | no | 6683 / 5123 |
| 64 | no | yes | yes | yes | yes | no | 6683 / 5123 |
| 96 | no | yes | yes | yes | yes | yes | 6683 / 5123 |

A foe 10 px away has already hurt the knight before the slam can land, so the
shock is for the ones around, not the one beside. The range begins at 24, the
smallest that still breaks the cellar floor.

Shield spring: 1200, 2000, 3000 and 4000 all finish (the spring is optional; the
tower's lane is the other way up), and the feel test
`a_raised_shield_springs_a_friend_higher_than_a_jump` holds at the shipped 3000.

## Fail and retry
A fall into the bog or a pit: a wedge, a 14-tick tumble and the knight stands on
the last safe ledge with 70 ticks of grace. The last wedge: a bubble, and when
every knight is down all are at the last gate 20 ticks on. Nothing is lost:
banners, sparks, gates and the changed floor are the party's, and a retry never
starts over.

## Version 1 is only
Eight banners, one hub, one mountain, one boss. No second boss, no unlockable
moves, no time limit under an hour, no art pass: every actor is a labelled block
and `slots.json` and `ART.md` say what the art wave paints.

## Door
Kingdom tile (174, 23): the Wastes, west of the Dragon Keep's gate. Walk up and
press E. The door is a pole gate with eight pennons, dull until the mountain has
been won once and in colour after. Hold Esc leaves; E at the finale returns
whoever presses it.

## Pays
Campaign mark `bannerpeak:campaign-begun` (`CAMPAIGN_MARK`), settled into the
realm through `settle` as the `BEGUN` bit of `Home::roundtable[SLOT]`, kept
across a crash by the intent journal. A purse of 60 gold to each seated knight on
the first finish, 12 on every later one, once per seat per run. A best score (1000
less a point for each second beyond two minutes, never under 100) and win and play
counts live in `Records::roundtable[SLOT]`.

## Evidence
The scripted party (`bot_input`: a goal list, a reactive controller and map hints,
no memory of its own) clears the whole adventure with every knob at its shipped
value:
| seats | ticks | seconds |
|---|---|---|
| 1 | 6683 | 222 |
| 2 | 5870 | 195 |
| 3 | 5355 | 178 |
| 4 | 5123 | 170 |
| 5 | 5460 | 182 |
| 6 | 6584 | 219 |

Through the shell's `Run::step` (the purse paid once, the Home and the record
settled): 1 seat 6681, 3 seats 5392, 5 seats 5683. Every finish has all eight
banners and all eight sparks up, which is the proof that each banner can be
reached at every table size. The stretches are also run alone from their gates:
marsh 742, mill 1753, shaft 497, snow 801, King Cinder 1183 / 1157 / 1035 at 1, 2
and 6 seats. A party that has passed the snow with a spark missing goes back by
the board (466 ticks at one seat, 581 at three). The bots are quicker than
people; a first run by a person is aimed at 4 to 6 minutes, which no test shows.

## Fidelity limits
The Delve has no free three-dimensional movement, so the whole adventure is
side-view. The half of the heritage that was a scrolling level fits it; the half
that was a hub of rooms to roam does not, and the courtyard is a flat street with
a Great Pole at its middle rather than a place to wander. The triple jump, long
jump, backflip, wall kick, ground pound and dive are all there but flat: their
height and distance are the whole of them. Eight banners are earned by eight
goals, not eight whole worlds: each of the six mountain goals is a stretch of one
scrolling mountain, and a bonus room is one cellar.
