# Moonfall (Three Bells to Moonfall)

A three-day time loop in a fen town. The party shares one clock and one town;
ringing the Moonwell Bell winds all of it back to the first dawn, and what the
party learned and earned stays. Three bell towers must burn together at the Third
Midnight, or the Gloam Moon falls.

Setting in one line: Moonwell Keep's fen town, three days before the Gloam Moon
falls. The classic time-loop adventure is the feel this honours; nothing of its
names, maps, art or music is used.

## The feeling
Standing in the plaza at dawn on a later try, already knowing the baker's hours,
already walking to where the herald will cry, with a notebook that writes itself
and a bell that takes it all back.

The moment a knight must feel in the first 10 seconds: the dawn bell tolls, a
card says "Dawn of the First Day", the clock bar at the foot starts to run, a
great pale moon with a frown hangs in the sky, and a lamplighter is already
reading on her porch. Press Q and the Ledger is blank but waiting.

## The loop (one sentence)
Learn who is where and when, do the favours that exist only at certain hours,
and when the moon falls (or the table rings the Bell) the town rewinds in about a
second with the Ledger, the masks and the keys kept, until the three lights burn
together at the Third Midnight.

## Words on the title card
TALK. WRITE. REWIND. (and "DAWN OF THE FIRST DAY" at every dawn)

## Who plays
Every seated knight, at once, on their own screen and camera. One shared town,
one shared clock, one shared Ledger. A knight who joins late sits down at the
Moonwell; a knight who steps away (a hidden tab) is kept where they stood with
what they carry; hold Esc leaves.

## Controls (existing buttons only; no new `Input` bit)
| Button | Does |
|---|---|
| WASD (move) | walk; in the Ledger, A/D turn the page; in the bell menu, W/S choose |
| E (interact) | talk, take, hand over, open a door, relight a lantern; **hold E** at a tower or the Moonwell to light it; turns a page of a conversation |
| Q (swing) | open and close the Ledger |
| R (ult), or E at the bell post | the Moonwell Bell: open its menu (rewind to first dawn, wait for dusk, wait for dawn), E rings; with a ring pending, R puts your hand on the rope |
| Z (quaff) | wear the next mask you own: bare, Swift, Stout |
| X (bomb) | put down what is in your hand |
| Space (guard) | a sidestep through a wisp; six ticks, then a rest |
| Esc held | leave (15 ticks, the shell's) |
F and 1 to 4 are not used (there is no combat). Arrow aim keys also turn pages and
choose in menus.

## The five numbers
Fixed integers; live F2 knobs (`Tune::roundtable[1]`); HZ is 30. Positions are
pixels times 256 on a 16-pixel tile map, the clock is ticks: a stage-style integer
sim, no floats in the rules.
1. `moonfall_day` = 3000 ticks (1800 to 6000): a day. The loop is three, 5 minutes;
   an hour is 125 ticks. Longer and the folk are easier to catch; shorter and the
   favours press.
2. `moonfall_walk` = 150 percent (100 to 300): a knight's pace against a villager's
   (1.5 pixels a tick). 2.25 pixels a tick, the Forest Road's run. Swift is 1.6 times
   that, Stout 0.7.
3. `moonfall_wisps` = 4 (0 to 8): wisps after dusk for one knight. Each extra seat adds
   two wisps and a tenth to their speed; the Final Hours add one. They are never fewer.
4. `moonfall_rewind` = 40 ticks (20 to 44): the ritual, 1.33 s. The clock spins back
   for the first 28 ticks, the town lands on its dawn, control returns (under a
   second), the card finishes fading over the rest.
5. `moonfall_notice` = 64 pixels (24 to 160): how near a knight must be for the
   Ledger to write what they see.

Not knobs, but fixed in the code and the Ledger: Orin's pass is five hours' work and
smudges after two; reeds take twelve hours in the hand; a flame burns four and a
half hours (less by a quarter-hour for each seat beyond the first, never under two
and a half); lighting is a 24-tick hold; a candle ward is 75 ticks; a snuffed knight
lies 90 ticks.

## The town, the clock and the folk
The loop is 72 hours, 06:00 on the first day to the dawn the moon falls. Hours are
bells: a big toll at 06, 12, 18 and 24, and every hour in the Final Hours (from the
Third Midnight). The sky is a function of the clock: dawn peach, noon blue, dusk
copper, night navy with stars, and the moon grows and sinks a little each night. The
bridge goes up at 20:00 and comes down at 06:00. The lamps are lit by Mira's round
from 17:00. Each knight keeps three candles.

Every villager walks a real route (pure functions of the clock, so the rewind runs
them backward for free). At the default day length (times are 24-hour):

| Who | Day | Stops (arrival) |
|---|---|---|
| Mira Quill, lamplighter and Ledger keeper | each | 06:30 Archive porch (reads) - 12:30 bakery (lunch) - 14:00 porch - 17:00 to 19:30 lights the six lamps west to east - 21:00 keeps the well watch (the third night: the verse) - 22:30 asleep |
| Tovin Reed, keeper of the north bridge | each | 06:30 bridge post (lowers the bridge) - 12:00 noon meal in the hut - 13:00 post - 20:00 hut (raises it) |
| Nessa Flint, baker and smith | 1, 3 | 06:30 bakery counter - 12:00 forge - 16:30 oven - 18:30 asleep |
| | 2 | 06:30 forge (the iron cart comes at dawn, gone by eight) and all day - 16:30 oven |
| Orin Wren, scribe who lost his ink | each | 07:00 searching the reed bank - 08:30 his desk - 17:30 supper at the bakery (across the bridge) - 20:30 asleep |
| Pella Starling, herald | each | 07:00 Herald House - 12:00 cries noon at the podium - 13:30 house - 18:00 cries dusk (the first day: a rider comes; the second: he is at the gate; the third: three towers, one midnight) - 20:30 asleep |
| Aster Vale, lantern rider | 2 only | 16:30 arrives at the east gate, his horse lame; waits there until 23:30 unless shod, then rides to the well |

The third evening everyone out hurries home before the Third Midnight; the town is
left to the knights.

## The favours (the dependency graph)
The towers want a key, a seal and oil. Each is earned once and **kept across the
rewind**; the doors they open, and anything carried, are not. A hand holds one
thing at a time. Four of the favours are carries that overlap in time, so one hand
can do only one of them in a loop; a party splits them.

| Favour | When and where | Needs | Gives (kept) |
|---|---|---|---|
| Pella's scroll to Mira | take it 07:00 to 08:30 on the first morning (Herald House); Mira takes it at her 21:00 well watch on the second or third night | ask Pella once | the Swift mask (light, quick, crosses the creaking planks), a tick for Pella |
| Reeds to Nessa | cut at the reed bank, carried twelve hours, handed over 08:30 to 10:30 on the second morning | ask Nessa once | the Stout mask (slow, pushes the crate), a tick for Nessa |
| Orin's ink pot to Orin, then his pass to Tovin | the pot to Orin 07:00 to 08:30 on the first morning; the pass is ready five hours on and keeps two; Tovin airs the keys 17:00 to 20:00 on the second day | ask Orin and Tovin once | the north key, ticks for Orin and Tovin |
| A horseshoe to Aster | the iron cart is at the forge 06:30 to 08:00 on the second day, and only gives to a knight whose Ledger says the rider has come; Aster is at the gate from 16:30 | have seen Aster arrive (a prior loop) | the oil in the well (he rides it there), a tick for Aster |
| Mira's seal | any dawn 06:30 to 09:00, for a knight who wears both masks | Swift and Stout | the south seal, a tick for Mira |
| The verse | Mira says it at the third night's 21:00 well watch | be there | the right to light |
| The three lights | from the Third Midnight, hold E at the north tower, the south tower and the Moonwell | the verse; the key, the seal and the oil; the doors open (the key at the north door; the seal at the south door once the crate is pushed down the causeway into the ditch by a Stout knight) | when all three burn at once, the moon turns back |

**Why a blind single pass cannot finish** (asserted in
`a_blind_single_pass_cannot_finish_however_many_knights_play_it`): the oil needs a
shoe, the shoe needs the Ledger to say the rider has come, and the rider first comes
at 16:30 on the second day, after the iron cart has gone at 08:00. Whatever the table
does in a first loop, the moon falls. Bots with an empty Ledger at one, two, four and
six seats all see the moon fall in their first loop.

**Why a lone knight needs four loops at the least**
(`one_hand_cannot_do_two_of_the_four_favours_in_a_loop_so_a_lone_knight_needs_four`):
the scroll, the pass, the reeds and the shoe must each be held through a stretch of
the loop; every pair of those stretches overlaps, and a hand holds one thing. A
table of four splits them; a table of two does two a loop.

## Co-op layer (light)
- One town, one clock, one Ledger: what any knight sees is written for all (to the
  whole table's screens, with a card-pick sound), and a tick beside a name is the
  party's.
- The Bell is the table's: a knight asks, and more than half the hands must be on
  the rope (R) within five seconds. Alone, it rings at once.
- Parallel favours: the carries above, and a seat to guard each flame at midnight.
- Pressure scales with the table and never eases: wisps +2 per seat, a tenth
  faster per seat, one more in the Final Hours, flames a quarter-hour shorter per
  seat. A knight who leaves does not ease it (the table was that size).
- Extra seats expose extra optional work: relighting the street lanterns, which are
  decoys the Gloam goes for first, and standing guard at a lit flame (a knight within
  40 pixels keeps wisps off it).
- Mid-run join: placed at the Moonwell, candles full, reading the same Ledger.

## Fail and retry
- The moon falls at the end of the loop: a white wipe (the moon comes down the
  screen), the clock spins back, the town lands on its first dawn, control returns at
  tick 28 of the ritual: **under a second from the wipe's start, no menu**. The
  Ledger, the masks, the key, the seal and the oil stay.
- A wisp's touch costs a candle (a 75-tick ward); three candles and a knight is
  snuffed out: carried to the Moonwell after 90 ticks with one candle, what they held
  lost (the ink pot goes back to the bank). Nothing is a wipe but the moon.
- The Bell is a retry by choice: rewind, or wait for dusk or dawn to wait for a
  schedule (never past the Third Midnight).

## The Ledger
One page for the town and one for each of the six folk. Each page lists what the party
has heard them ask (with the hours in it), their routine as witnessed, a tick when the
favour is done, and notes. It writes itself: a routine line when a knight is within the
notice radius of a villager at a stop; a request when they ask; a fact when it comes
to pass (the herald's cry, the rider, the verse). An hour before a known appointment it
reminds the table. It is the real inventory.

## Familiar contract (each item met)
1. Three-day clock: the dawn card, a persistent clock bar with a sun/moon marker and
   the Final Hours red, bells, FINAL HOURS and a countdown. Met.
2. The doomed sky: a Gloam Moon with a menacing face, lower and closer each night,
   sky tint through dawn, noon, dusk and night. Met.
3. Reset to dawn: the Bell; the clock spins back, the sky runs in reverse, the folk
   walk backward, the card fades in; 40 ticks, control at 28; keeps the Ledger and the
   keepsakes, resets carried things, doors, the crate. Met.
4. Skip time: the bell's menu, dusk or dawn. Met.
5. NPC schedules: real routes, followable, interceptable; their lines change with
   the hour and with whether the town has been rewound. Met.
6. The notebook: the Ledger above, with ticks and reminders. Met.
7. A mask or two: Swift and Stout, earned by favours and kept. Met.

Quality bar: the reset is the 1.33 s signature (met); final-hours dread (the moon's
shadow sweeping the town, lanterns going out one by one, folk hurrying home, a bell
every hour, the red edge and countdown; **no music to thin: not met**); gentle odd
lines with a little portrait (met); the notebook as a tool (met).

## Honest fidelity limits
- The camera is a top-down view with a sky band, not a 3D town; the moon is a picture
  in the band, not the real sky. Interiors are not entered: folk go behind their door.
- No music (the Delve has none to thin); the toll cadence and the grading carry it.
- Gold resets because there is none; the favours are the economy.
- A loop is 5 minutes (the brief's number); a first-time party is two loops, about
  ten minutes, not three to six. The charter's range is for one loop.
- Blocks, not art: every picture is a labelled placeholder (`ART.md`, `slots.json`).

## Version 1 is only
One fen town, six folk with schedules, the Ledger, the Bell, five favours, two masks,
wisps and the finale. No combat, no interiors, no shops, no second town.

## Door
Kingdom tile (78, 92): the Village's south-west edge, in Castle Town. Walk up and
press E. Hold Esc leaves; the way back is the same tile.

## Pays
The campaign mark `moonfall:campaign-begun` and a party mark for the table that turned
the moon back (`moonfall:party-1` to `-6`), settled into the realm through `settle`:
the `BEGUN` bit of `Home::roundtable[1]`, and, **once** (that bit is what stops a second
payment), **30 gold and 1 wax for each knight at the table**, so 30 to 180 gold and 1 to 6
wax. Why that cap: it is one finished adventure, a few minutes of play per loop and
more than one loop of work, and it is paid once; the gold buys a barony room or a
horse, not a fortune. No crown buff: nothing here is a heat to win. A best (1000
minus 150 for each loop beyond the first, at least 100) and the win count live in
`Records::roundtable[1]`.

## Tuning table
Rounds: each cell is one scripted full run by the Ledger-reading bots from a blank
Ledger to the moon turning back (loops, ticks), so these are replayable, not guessed.
`cargo test -p angel-delve --lib tuning_table -- --ignored --nocapture` prints it.
The operator picks the number on the hour of tuning; these are the proposals and why.

| Number | Tried | 1 seat (loops, ticks) | 2 seats | 4 seats | 6 seats | Chosen, and why |
|---|---|---|---|---|---|---|
| `moonfall_day` | 1800 | 4, 21,055 | 3, 15,604 | 3, 15,372 | 3, 15,371 | 3000. At 1800 a table of four can no longer hold a loop and needs a third; 4000 and 6000 only make every loop longer (46k and 71k ticks for a lone knight) for the same loop counts. 3000 is the brief's five minutes. |
| | 2400 | 4, 27,914 | 3, 20,659 | 2, 13,974 | 2, 13,973 | |
| | 3000 | 4, 35,737 | 2, 17,654 | 2, 17,424 | 2, 17,423 | |
| | 4000 | 4, 46,197 | 2, 23,404 | 2, 23,173 | 2, 23,173 | |
| | 6000 | 4, 70,985 | 2, 34,906 | 2, 34,674 | 2, 34,674 | |
| `moonfall_walk` | 100 | 4, 34,868 | 3, 25,819 | 2, 17,424 | 2, 17,424 | 150. The loop counts barely move from 100 to 200, so it is a feel number: at 100 you can follow a villager but not cut them off; at 300 the town is crossed in 4 seconds and nothing is a journey; 150 is the Forest Road's run. |
| | 120 | 4, 35,786 | 3, 25,768 | 2, 17,424 | 2, 17,424 | |
| | 150 | 4, 35,737 | 2, 17,654 | 2, 17,424 | 2, 17,423 | |
| | 200 | 4, 35,686 | 2, 17,605 | 2, 17,423 | 2, 17,423 | |
| | 300 | 5, 44,661 | 2, 17,552 | 2, 17,423 | 2, 17,423 | |
| `moonfall_wisps` | 0 | 4, 35,730 | 2, 17,658 | 2, 17,424 | 2, 17,424 | 4. Up to 3 a lone knight lights the towers at the first try once the favours are done (4 loops, the structural least); at 4 the lone knight's first finale fails to the wisps once (5 loops, "about five"); more than that changes nothing because the +2 a seat is what presses a table, and the table's guards hold it. |
| | 3 | 4, 35,737 | 2, 17,654 | 2, 17,424 | 2, 17,423 | |
| | 4 | 5, 44,762 | 2, 17,654 | 2, 17,424 | 2, 17,423 | |
| | 5 | 5, 44,762 | 2, 17,654 | 2, 17,424 | 2, 17,423 | |
| | 8 | 5, 44,762 | 2, 17,654 | 2, 17,424 | 2, 17,423 | |
| `moonfall_rewind` | 20 to 44 | (not a bot number) | | | | 40. Control returns at 70 percent (28 ticks, 0.93 s) and the card fades over the rest: under the brief's 1.5 s and the charter's second. Below 30 the clock spin is too fast to read; the knob's top is the brief's bound. |
| `moonfall_notice` | 24 to 160 | 4, 35,737 each | 2, 17,654 each | 2, 17,424 each | 2, 17,423 each | 64 (four tiles). The bots stand on what they witness so it does not move their loops; for a person, 24 means standing on top of folk and 160 writes the whole plaza without following anyone. |

Evidence for the shipped defaults (3000, 150, 4, 40, 64), blank Ledger, bots:
1 seat 5 loops 44,762 ticks; 2 seats 2 loops 17,654; 4 seats 2 loops 17,424; 6 seats 2
loops 17,423. With every fact already written (an informed party) 4 and 6 seats finish in
**one loop (8,396 ticks)**; 1 seat still needs 5, 2 and 3 seats 2.

## Tests that prove it
`cargo test -p angel-delve --lib moonfall` (72 tests, a few seconds): the door and the
leave hold, two seats, six seats and byte-identical replay, mid-run join, parked seats,
the restart under a second; the clock, the schedules (no one late, no one in the water,
no one jumps), the Ledger (written once, reminders), the bell (rewind keeps the Ledger,
control inside 30 ticks, skip, the vote), each favour and its windows, the masks and the
crate, the wisps (the ward, the touch, three snuff, a roll goes through), the scaling
(never fewer), the finale and the purse once, the fall and the retry, the blind pass
that cannot finish, the hand check, the informed party that can, the bots at one, two,
four and six seats, validity, repair and a shake-it-about run of random keys at one, three and six seats. Renders: `write_moonfall_review` (1x and 3x,
every hour and state) and, in the cockpit, `write_moonfall_terminal_review`.

Also run: the shared shell's checks (`roundtable`, `escape_hold`) are green; the cockpit's
`roundtable` tests and `write_moonfall_terminal_review` (the cells with the Ledger, the
talk, the bell and the Final Hours printed as text); the wasm smoke scene and the parity
script (`parity.mjs <wasm> moonfall`: 488 lines, identical to the native run; the stage
draws in about 5 ms a frame in wasm; the wire is 70 bytes a tick); the cell-proof pins stay
fresh (nothing pinned is touched).
