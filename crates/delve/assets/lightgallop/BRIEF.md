# Lightgallop: the Moonwell Road

*A momentum ride for the whole table. Setting in one line: the Moonwell Road at
dusk, the Dimming behind you. The memory it borrows is the classic momentum
platformer (speed you build, slopes, loops, springs, sparks that scatter, a
boost, two routes through one level); every name, map and picture is our own.*

Re-read this file before every change. When the code and this file disagree, fix
this file first. The rules are `rules/lightgallop.rs` and `rules/lightgallop/`
(`math`, `terrain`, `rider`, `world`, `level`, `session`, `bot`), the picture is
`render/overworld/arena/lightgallop.rs`, the terminal text is
`cockpit/src/stage/world_viz/art/roundtable/lightgallop.rs`, art notes are in `ART.md`, slots in
`slots.json`. It is a side-view stage game (Forest Road's family): integer fixed
point (Q = 256 sub-pixels) on the 30 Hz sim, with its own curved terrain.

## The feeling
The moment a knight must feel in the first ten seconds: *the horse gathers pace
down the gate's ramp until the road is rushing past, the camera pulls ahead of
the way he rides and draws back, and the loop ahead is a thing he can take.*

The classic's whole feel is one idea: **the ground is a curve and you are speed
along it.** A slope gives and takes pace, a rolling tuck runs a hill faster, a
loop holds you on its ceiling only while you are fast enough, and when you are
not it lets you go and you fall, plainly and fairly, from the top.

## The loop (one sentence)
Ride the Moonwell Road with the whole table, building speed on its slopes,
popping snuffers and chaining lance-locks for spur, lighting lantern posts as
the Dimming chases the pack, break the warden at the Bell Gate and ring the bell;
a fall is a lantern carrying you home inside a second, and a tally and an
instant retry wait at the end.

## Words on the title card
`A D RUN   E LEAP   SPACE SPUR`. The road teaches the rest on plain plank signs
of two or three words: A D RUN, E JUMP, S ROLL, SPACE SPUR, HAY LOFT, SPEED
HOLDS, S E CHARGE, FAST UP, E HOP, SLOW DOWN, E E E CHAIN, ROLL OR LEAP.

## Where the words stand
Every word is set through one call (`say`, a `Word`), so a test can read back
where it stands: inside the frame, clear of every other word, and, at the
tally, clear of the shower of sparks.
- The top bar's columns keep at least eight pixels between them (`CLOCK_X`,
  `GAUGE_X`, `PIPS_X`, `GLOAM_X`), a chain of any length too.
- The words that come and go stand in the sky on lines of their own: the title
  card (two lines), `THE LANTERN CARRIES YOU`, `RING THE BELL` (`TITLE_Y`,
  `FALL_Y`, `BELL_Y`). The riders' number tags stand over their horses, and
  move along (right first, so a pack on one spot reads 2 3 4 5 6) or up or down
  to keep clear of every word and of one another; a rider out of the picture
  has none.
- The names on placeholder blocks keep their size when the camera draws back
  (`stamp_block`); a painted block has none.
- The pixel font has no star: the best rider's score wears a drawn crown.
- The terminal's cells are four pixels wide, so the picture's lettering is mush
  there: `cockpit/src/stage/world_viz/art/roundtable/lightgallop.rs` says the title card, the
  fall, the bell, the keys and the bell's name again as text over their own
  cells, and the tally as a box over the whole panel, on the same lines.

## Who plays
Every seated knight, at once, on their own screen, with their own camera, spark
count, spur gauge and chain. Nobody waits and nobody watches; every knight has
the whole set of moves. The road is shared (below).

## Controls (existing buttons only; `Input` is closed)
| Verb | Keys | Input field |
|---|---|---|
| run, brake, skid turn | A D (or the stick) | `move_x` |
| tuck roll (moving), crouch to charge (still) | S, or the down arrow | `move_y > 0` or `aim_y > 0` |
| leap (variable height, coyote 4, buffer 5) | E, or W | `interact` or `move_y < 0` |
| charge dash | hold S while still, tap E to rev, let S go | `move_y`, `interact` |
| lance-lock | tap E again in the air | `interact` (a fresh press) |
| spur (the boost) | Space, or F, held | `dash` or `fire` |
| ride again (at the tally) | E | `interact` |
| leave | hold Esc (15 ticks, `exits.rs`) | `leave_held` |

No new button, no new command, no shared-file change. `latch_tick` is not used:
the buffer (5 ticks) and coyote (4) already forgive a seat's latency.

## Reskin map
the runner is a **mounted knight**; rings are **sparks**; the spin dash is the
**charge dash** (crouch, rev, release); the spin ball is the **tuck roll**;
springs are **hay bales**; checkpoint posts are **lantern posts**; the goal
signpost is the **Bell Gate**; the boost is the **spur** with the lantern gauge;
the homing dash is the **lance-lock**; loops, half-pipes and the corkscrew are
**the wheel arch**, **the half-pipe** and **the coil**. Co-op, tuned for a table:
one pack, the slipstream, shared lantern checkpoints, the Dimming at the pack's
heels.

## Familiar contract: where each item lives
| # | The classic's item | Ours | Proved by |
|---|---|---|---|
| 1 | Momentum: slopes give and take, a real top speed, brake and skid, never stopping dead on a slope | Ground speed along a polyline; a slope's pull is the tangent's slant (`SLOPE` 28); top pace 5 px a tick; a brake against the run skids; a slope steeper than about 25 degrees slides a knight at rest, a gentle one holds him; a 66 px drop lifts a gallop by 12 percent and the climb takes it back | physics tests |
| 2 | Loops: stick above a speed, fall off below it; a half-pipe; a corkscrew hill | Two full loops (radius 40) and a coil (radius 26, 130 px long) are paths that curl over themselves; above `lightgallop_loop` the ceiling holds, below it the knight drops with the speed he had, from the wall's top; a half-pipe and four rolling crests | physics tests, the real loops |
| 3 | Tuck roll, charge dash, variable jump, bounce chain | S while moving rolls (small, rolls through foes, runs a downhill 2.5 times harder); S while still then taps of E rev 320 each to 1280 and the release launches at 768 plus the charge; E held longer leaps higher; landing on a foe springs up and counts a chain | physics and play tests |
| 4 | Rings as health that scatter and can be taken back, a count, a shield pickup | Sparks: a hit throws up to 16 in a fixed fan, they bounce on the road and can be taken back from 24 to 120 ticks; with none a hit fells the knight; the HUD counts; the lantern crate's shield takes one hit and scatters nothing | play tests |
| 5 | Springs, boost pads, checkpoints, a spinning goal | Hay bales (up, diagonal), pads (kick to 7.5 px a tick), six lantern posts that spin and light as shared checkpoints, the Bell Gate's bell | play tests |
| 6 | Two routes that split and rejoin | At the fork a rider at a gallop (1200 or more) is carried onto the high rails (planks, gaps to hop, a pad, a crate, brambles) and a rider who has eased off follows the valley road; both meet before the third lantern | the ride test: both are ridden |
| 7 | The boost with a gauge that refills from rings and lock-dash chains | Hold the spur: a kick of 256, push to `lightgallop_spur`, 12 a tick from a 1000 lantern gauge; sparks +12, a kill +60, a chain link +140 each (to five), a lantern +200, a spur crate fills it. Lance-lock: tap E in the air, the nearest foe in a 60 degree cone within `lightgallop_lock` is dashed at 12 px a tick and popped, lifts him and chains to the next | play tests |
| 8 | Act-clear tally: time against par, rings, best chain, a rank, an instant retry | Time (par 2:50), sparks, best chain, lanterns lit, the lantern bonus on a line of its own (a hundred for all six, part of the points and not of each knight's own score below them), points to 1000, rank C, B, A or S (420, 650, 850), the numbers ticking up one after another, the rank stamped, a shower of sparks behind the words, each knight's own score under `EACH KNIGHT` with the best rider's crown, the bell swinging; one E starts a fresh ride in the same tick | ride tests |

## Quality bar
| Item | How |
|---|---|
| Speed has weight | The camera leads the way he rides (22 ticks of travel, up to 120 px, eased) and draws back to three quarters between a gallop and a spur; speed lines and hoof dust at pace, a flame at the saddle on the spur; the gait advances one frame every 11 pixels run |
| The loop is a set piece | A rising whoosh (`wheel_spin`) as he enters; the camera barely moves (a slow vertical follow); the rider is a block turned about its centre by an eased lean (`Rider::ang`), integer nearest-sample, so it turns clean through 360 degrees with no frames to jitter; the stone ring is drawn from the very curve the rules ride, with no seam at its foot |
| Scatter is delicious | Sixteen sparks arc in a fixed fan and bounce, chime (`hero_hurt` then `spoil_pickup`) and blink before they fade; a chain climbs a chime ladder (`hit_magic`, `card_pickup`, `gold`, `spoil_pickup`, `wish_grant`) one step a link |
| Controls are tight | Variable jump, coyote 4, buffer 5; the roll and the rev respond on the tick pressed; a hit stops the body 2 to 4 ticks (this rider only: the shared road never pauses) and the picture shakes |
| The tally is a small celebration | Counters tick up in turn, the rank lands with a stamp, sparks fall over the party, the bell swings |

## The road (about 20,000 px: a bot rides it in 100 to 120 s, a first ride runs 2 to 4 minutes)
| Act | From x | What |
|---|---|---|
| 1 The Downhill Gate | 0 | the gate and its wall, a run-up and a ramp, a pad, the first bramble and snuffer, a rolling hill, the trench (a hay bale at its foot), the first lantern, a rolling meadow, the hay loft (a bale lifts you to a plank over the meadow, with a crate), a long downhill |
| 2 The Wheel Arch | 4300 | loop one, the second lantern, the half-pipe (a diagonal bale on its far lip throws you over the first crest), four rolling crests, the corkscrew coil, a rhythm of three small valleys, the fork (high rails or the valley road), the third lantern |
| 3 The Reed Fen | 11300 | a deep valley of reeds, brambles and snuffers with a line of nine wisps floating over it to chain (a second, higher line for fuller tables), a bridge of sparks on the far bank, reed hills, the fourth lantern, a short marsh with a second, staggered chain |
| 4 The Bell Gate | 15700 | a last downhill, loop two (no pad: speed or nothing), the fifth lantern, a rise, the last lantern (it holds the Dimming furthest back), the warden (its fall gives every knight ten sparks and a full spur for the run to the bell), the bell |

432 sparks, 5 hay bales, 4 boost pads, 6 lantern posts, 7 lantern crates (4
shield, 1 sparks, 2 spur), 46 foes for one knight rising to 70 for six (20 to 32
snuffers, 13 to 19 floaters, 12 to 18 brambles), one warden. The terrain is eight
paths (the road, two loops and the coil, three rails, the hay loft's plank),
drawn and ridden from the same polylines.

## Co-op layer (light, and it never gets in the way of the feel)
- **One road, per-seat cameras.** Each knight has their own camera, spur gauge,
  chain and spark count. Sparks and crates are taken once *per knight* (a pack
  never robs the knight behind); the foes, lanterns, springs and the clock are
  shared.
- **The Dimming** chases the pack at a fixed pace, `lightgallop_dimming`, times
  90, 110, 130 and 150 percent by act, and 8 percent quicker for each knight past
  the first. It never lets the rearmost knight lead it by more than 1200 px (a
  pack that outruns it is gained on at once, so the chase stays on). A knight it
  overtakes is carried to the last lit post (24 ticks, under a second) and it is
  held back again (520 px; 1000 at the last lantern; 25 px less for each knight
  past the first). A solo knight can beat it with good play: the bots never lose
  a ride. It stops at the bell.
- **Slipstream.** A knight 14 to 110 px behind a faster knight on the same level
  is in his wake: 128 more top pace and an extra push (wind lines drawn over the
  horse), so a pack outruns a lone rider.
- **Lanterns are shared checkpoints.** Any knight who lights one lights it for
  all, holds the Dimming back and is where every fallen knight wakes. The tally
  counts the lanterns the pack lit.
- **Per-seat scaling: more pressure, never weaker foes.** Each seat that sits
  down makes the Dimming 8 percent quicker and the lanterns hold it 25 px less,
  wakes more snuffers, brambles and a second chain of wisps (46 foes for one, 70
  for six) and adds a hit to the warden (3 for one, 8 for six). A knight who
  leaves takes nothing off; one who rejoins a seat already provisioned scales
  nothing twice.
- **Mid-run join.** A late knight sits down at the last lit post (or the gate),
  with the gauge half full and a moment of grace; the road adapts.
- **Retreat.** Hold Esc for 15 ticks: the knight stands up (their rider is
  dropped), the rest ride on; the way back is the door's tile.

## The five numbers (fixed integers, live on the F2 panel, `Tune::roundtable[4]`)
| # | Knob | Starts at | Bounds | What sliding it does to the feeling |
|---|---|---|---|---|
| 1 | `lightgallop_slope` | 100 (percent) | 20 to 300 | slope pull and friction together: how heavy the road is. Low, hills are decoration; high, every rise is a wall |
| 2 | `lightgallop_loop` | 512 (1/256 px a tick, 2 px) | 128 to 1536 | the slowest pace that holds the ceiling: how fast a rider must be to take a loop |
| 3 | `lightgallop_spur` | 2048 (8 px a tick) | 1280 to 3072 | the spur's top speed: how far past a gallop the boost carries |
| 4 | `lightgallop_lock` | 96 (px) | 32 to 240 | how far the lance-lock reaches: how loose the chain between wisps may be |
| 5 | `lightgallop_dimming` | 1000 (3.9 px a tick for one knight) | 0 to 2048 | the Dimming's pace: how close the chase runs |

Fixed constants that are *not* knobs (picked from play, in `rider.rs` and
`session.rs`): top pace 1280, push 26, brake 130, friction 12, gravity 100, leap
1560, gauge 1000 and its drain 12, charge step 320, lock dash 3072 for 14 ticks,
scatter 16, down 24 ticks, warden hits 3 plus 1 a seat.

### The tuning table (rounds played, values tried, why the chosen value)
Every round is a deterministic ride on the 30 Hz sim by a bot (`bot.rs`:
`Skill::ACE` rolls, spurs and chains; `Skill::FIRST_RIDE` gallops, hops late and
hesitates; `Skill::SLOW` eases off a quarter of the time), the number set through
the F2 knob path (`Tune::set`). `cargo test -p angel-delve --lib -- --ignored
the_tuning_report the_dimming_and_loop_report` replays them. "Falls" are knights
carried home; "hits" blows taken.

1. **`lightgallop_slope`**: 5 values by 2 riders by (solo, four) = 20 rides, plus
   the first grey-curve rounds (flat, hill, half-pipe, loop) that set the pull
   (28) against the push (26) so steep rises need momentum and a gentle rise can
   be walked.

   | value | ace solo | first ride solo | what it felt like |
   |---|---|---|---|
   | 40 | 102 s | 126 s | hills barely move you; the roll is pointless |
   | 70 | 107 s | 126 s | pleasant, but the trench is a speed bump |
   | **100** | **113 s** | **127 s** | **a drop gives a gallop its push, a climb takes it back; a 25 degree slope is where a standing knight begins to slide** |
   | 150 | 110 s | 128 s | rises start to stall a walker (four seats: 4 carried home) |
   | 250 | 119 s | 134 s | a wall at every crest (the first ride takes 9 hits) |

2. **`lightgallop_loop`**: 5 values by 2 by 2 = 20 rides (every bot makes the
   loops, because they arrive fast), and a grey-curve scan of the slowest entry
   speed that goes round the real loop (the table is that scan):

   | value | slowest entry that goes round |
   |---|---|
   | 256 | 3.6 px a tick (a trot makes it) |
   | **512** | **4.0 px a tick: a gallop (5.0) has a quarter to spare, a trot (under 3.5) falls from the wall near the top, a knight crawling at about 1 px a tick rolls back down the ramp, the spur (8.0) is certain** |
   | 768 | 4.5 px a tick (a gallop that dipped slips off: too fussy for a first ride) |
   | 1024 | 6.1 px a tick (needs a downhill or the spur: the hard setting) |
   | 1280 | 7.0 px a tick (the spur only: a first ride at four seats slipped off a loop) |

3. **`lightgallop_spur`**: 5 by 2 by 2 = 20 rides. 1536 (6 px) is hardly a boost
   (the ace's 125 s is slower than 1792's 118 s and a table of four is carried home
   six times); **2048 (8 px)** is the chosen pace: the ace clears in 113 s, hazards
   stay readable under the pulled-back camera, and a gauge (1000 at 12 a tick) is
   83 ticks of spur, about 660 px. 2560 and 3072 (10 and 12 px) take the ace down
   to 99 s and 89 s but push a rider past what the eye can read: a table of four
   takes 13 and 19 hits where it took 10, and the camera would have to draw back
   past three quarters.

4. **`lightgallop_lock`**: 5 by 2 by 2 = 20 rides. At 40 px the first ride's chain
   breaks (5 of 9 where the wisps hang a hundred pixels apart, a table of four 8);
   at 64 it holds on a good line (8 to 10); **96** chains nine of nine for both
   riders and eleven for a table of four across both lines; 160 and 240 reach too
   far and lock the wrong wisp (the ace's table of four takes 17 and 19 hits where
   it took 10), so the chain stops being a line and becomes a lottery.

5. **`lightgallop_dimming`**: 6 by 2 by 2 = 24 rides, plus 5 by 2 by 4 = 40 rides
   by seat count (the numbers are knights carried home; a "NOT" is a ride that
   did not clear in 500 s):

   | value | 1 seat | 2 seats | 4 seats | 6 seats |
   |---|---|---|---|---|
   | 700 | 0 | 0 | 0 | 0 |
   | 900 | 0 | 0 | 1 | 4 |
   | **1000** | **0** | **0** | **2 to 7** | **4 to 5** |
   | 1100 | 0 to 4 (a slow rider takes 218 s) | 2 | 5 to 6 | 18 to 21 |
   | 1300 | NOT (25 carried) | 6 to 7 | 14 to 22 | 22 to 27 |

   The chosen 1000 (3.5 to 5.9 px a tick for one knight across the four acts, 4.9
   to 8.2 for six) is felt by a table that dawdles and by a full one, never a wall
   for a solo rider. At 1200 even the ace alone is carried home 42 times and never
   clears: the Dimming outruns a gallop in act 4 and a solo rider is caught for
   ever. The knob's upper bound (2048) is for the operator's tuning, not for play.

**Bot evidence** (`ride_tests`, deterministic; ticks at 30 Hz; every ride
cleared, rank A or S, all six lanterns lit):

| seats | upper route | lower route | mixed (odd seats high) |
|---|---|---|---|
| 1 | 3399 (113 s) | 3533 (117 s) | 3399 |
| 2 | 3358 (111 s) | 3381 (112 s) | 3411 |
| 4 | 3224 (107 s) | 3312 (110 s) | 3449 |
| 6 | 3284 (109 s) | 3324 (110 s) | 3120 |

(The bots ride well and never pause; a first ride, which hesitates, hops late
and reaches for no spur, takes 125 to 135 s, and a person learning the road takes
longer: two to four minutes.)

## Fail and retry
A hit with no sparks, or the Dimming, or the world's floor fells a knight: he
tumbles for 24 ticks (0.8 s), then the lantern sets him at the last lit post (the
gate before the first) with a moment of grace, the spur gauge at least three
tenths full and every count kept. The world's progress is kept: popped foes stay
popped until they re-form 180 ticks later, lit lanterns stay lit, the warden's
wounds stay. A knight who has rung the bell is past the Gloam's reach and every
foe's. No menu between tries. At the tally one E starts a fresh ride in the same
tick.

## Pays
The first clear writes the campaign mark `lightgallop:campaign-begun` and sets
`Home::roundtable[4] & BEGUN`, once, through `settle` (idempotent, crash-safe
through the intent journal). Each seated knight at the finale is paid once, in
gold, by the party's rank: **C 10, B 20, A 30, S 40** (a cap of 40 a seat; a
Pennon round's pot is 10 a seat); a repeat ride pays a token **5**, so the road
is a place to come back to and never a faucet. The best rider wears the laurel.
The record keeps the best points (`Records::roundtable[4].best`, at most 1000)
and the wins. A knight who left, or never sat, is not paid.

## Honest fidelity limits
- **No real 3D and no free terrain.** The side-view stage rides curves we author;
  the loops and the coil are on rails (a rider follows the polyline), and the
  physics does not let a rider leave the surface at a convex crest: a crest is
  ridden, not jumped. Real launches are a path's end, the springs and the rails'
  gaps.
- **A rider is a block turned about its centre.** The art wave paints one upright
  horse; no loop frames exist (`ART.md`).
- **Riders do not collide.** The slipstream and a shared warden are the only ways
  they affect one another, by design: the pack is a draft, not a traffic jam.
- **Sparks and crates are per knight, foes are shared.** The charter's default is
  that all pickups are shared; a leader who took every spark would leave the
  knight behind with nothing to scatter, so the sparks are each knight's own and
  the foes re-form for the pack behind.
- **The sounds are the realm's existing cues.** A chain's pitch ladder is five
  existing effects stepping up; a true pitch-shifted chime and a rising loop
  whoosh want new cues (`ART.md`).
- **Camera zoom is nearest-sample.** At speed the block art is drawn back to
  three quarters; painted art will want a cleaner downsample.
- **No art and no music.** Everything on the road is a labelled block.

## Version 1 is only
One road, four acts, one warden, every seated knight, a start and a finish. No
rider-against-rider race, no second map, no ghost, no replay.

## Door
Kingdom tile (175, 71): the Tilt-Yards, west of the Horse Ranch. Walk up
and press E. Hold Esc leaves; the way back is the same tile. The door is the
integrator's to move.

## Evidence (everything below runs under `cargo test -p angel-delve`)
- `rules/lightgallop/` unit tests: the integer square root, the sine and
  arctangent tables, the polylines, a landing from above and a rise through from
  below, every thing on the level standing on its road.
- `tests/rules/together_lightgallop_physics__tests.rs`, on grey curves before any
  level: a top speed that holds, coasting, the brake and skid, a downhill that
  gives and an uphill that takes, a slope that slides a knight and one that holds
  him, the half-pipe, the loop (fast holds the ceiling, slow falls, slower rolls
  back), the spur makes the loop from a short run and a trot does not, the lean
  turning a full circle in steps no bigger than 40 brads, variable jump, coyote
  and buffer, the roll, the charge, the spur's gauge, the draft, determinism.
- `tests/rules/together_lightgallop_play__tests.rs`: a roll pops a snuffer and a
  walker is hurt, the bounce, re-forming, a scatter in a fan and taken back, a
  fall carried home in under a second, a shield, a crate and a hay bale, a pad, a
  lantern lit by one for all, the lance-lock chain refilling the gauge, the
  lunge, sparks taken once per knight, the Dimming, scaling by seats (never
  down), the wake, a late join, the bell and its grace, each of the five knobs,
  no valley a trap, both real loops at a gallop, a trot and the spur.
- `tests/rules/together_lightgallop_ride__tests.rs`: a bot clears the road at 1,
  2, 4 and 6 seats by either route in 2700 to 4800 ticks, both routes are really
  ridden, replay is byte for byte, the purse is paid once and a repeat pays the
  token, the tally and the one-tick rematch, a scripted loss and retry, a whole
  table downed at once, a late join, a retreat, validity and the wire through a
  ride, a broken ride refused and mended.
- `tests/render/arena__lightgallop__tests.rs`: the picture on both routes at 1, 2,
  4 and 6 seats; the words (inside the frame, clear of one another, the tags
  apart, the tally's words clear of its sparks, the lantern bonus line, the
  crown, the names on blocks at three quarters); `write_lightgallop_review` writes
  the pictures to open (the road by act, 14 moments, the loop turning, the moves,
  the words at 1, 2, 4 and 6 seats, the tally, every block).
