# Griffinwing

Moonfall Pass: a canyon run on griffin wings to relight the last beacon. It
honors the classic on-rails flight shooter with a squad of wingmen. Every
seated knight flies their own griffin in one formation, behind their own
chase camera, over one shared rail, one set of foes and one Wyrm. The code is
`rules/griffinwing.rs` and `rules/griffinwing/`, the stage
`render/overworld/arena/griffinwing.rs` and `.../griffinwing/`, the terminal
text `cockpit/src/stage/world_viz/art/roundtable/griffinwing.rs`. `ART.md` and `slots.json` are
the art wave's; the tests are `together_griffinwing__tests.rs` (the shell's
checks), `together_griffinwing_play__tests.rs` (the game) and
`arena__griffinwing__tests.rs` (the pictures).

## The feeling
The memory: the world flies at you down a canyon, you bank into the turns with
the camera trailing a little behind, and the reticle floats ahead of you,
leading the shot. You tap and the bolts snap out. You hold, and everything the
reticle sweeps over wears a bracket that snaps on; you let go and the
starbursts pop one after another down the line. A wingman shouts that a kite
is on his tail, and you shoot it off. You roll, and a bolt that would have hit
you turns round and goes home.

The moment a knight must feel in the first ten seconds: *hold F, sweep the
reticle across a flight of snuffers, let go, and hear them pop in a chain.*

## The loop (one sentence)
Fly the rail, aim and lock and pop what the Gloam sends at you, roll its bolts
back at it, save the wingmen, thread the gates and arches, then open the
Wyrm's three lamps with turned bolts and break them with charged locks; a fall
puts the beat back in under a second.

## Words on the title card
Beat one: `THUNDERHEAD RUN` / `AIM, HOLD, RELEASE`. Beat two: `THE RUINED
CAUSEWAY` / `ROLL TO REFLECT`. Beat three: `THE WYRM ROOST` / `BREAK THREE
LAMPS`. The wing's short lines (28 characters at most) teach the rest, and
the key strip along the bottom (`F SHOOT  HOLD F LOCK  SPACE ROLL  E SPUR  Q
BRAKE  X BOMB`) shows for the first forty seconds.

## Who plays
Every seated knight, at once, on their own screen: their own griffin, their
own chase camera and reticle, their own shield, bombs and spur. Empty seats up
to a wing of four are flown by the Round Table's riders (Aster, Mira, Tovin,
Nessa), who fly in formation ahead, shoot a little, call dangers and get
kites latched onto them. Foes, bolts, lamps, gates and the clock are the
party's.

## Controls (existing buttons only; the Input set is closed)
| Verb | Button | Notes |
|---|---|---|
| Steer (bank, climb, dive) | WASD, the left stick | the craft banks into the turn and the lean lags the stick |
| Reticle | the arrows, the right stick | instant, and it stays where you put it; a near and a far reticle on one ray |
| Shoot, charge and lock | F | a tap is a twin bolt; held past seven ticks it charges and locks what the reticle sweeps over; let go to send the starbursts |
| Barrel roll (the shield-spin) | Space (Dash), or Shift with a direction, or a double tap of A or D | reflects bolts for its first two thirds; no shooting while it spins |
| Spur | E | slides the craft forward along the rail on a short meter |
| Brake | Q | slides it back; costs nothing |
| Starfall (the smart bomb) | X | three a beat |
| Leave | hold Esc | the shell's 15-tick retreat |
| Return after the tally | E | after a moment, a fresh press |

On a pad the right stick aims and, as in every Delve game, holds F: aiming
charges. That is the client's own mapping (`aim_holds` in `CONTROLS`), not
this game's, and a shared file; main may want it off for this game.

## The five numbers (one fixed integer each, live on the F2 panel)
HZ is 30. Units: a tenth of a tile for the box, ticks for the rest.
1. `griffinwing_lock` = 40 (20 to 90): ticks a hold takes to reach full charge.
   Slid up, the multi-lock is a slow wind-up; down, it is a double tap. A full
   charge is what breaks a lamp; a short one pops small things for 16 to 36.
2. `griffinwing_roll` = 30 (18 to 60): ticks a roll runs. Its reflect window is
   the roll less ten ticks (never under eight), then a twelve-tick rest. Slid
   up, a long, safe spin that locks the knight out of shooting for longer.
3. `griffinwing_window` = 64 (40 to 90): the steer box's half width in tenths
   of a tile (the half height is five eighths of it). Wide means room to
   dodge and a canyon that feels empty; narrow means every bolt is on you.
4. `griffinwing_spur` = 75 (30 to 180): ticks of spur from a full meter (it
   refills in two and a half times that). It slides a craft 5.5 tiles
   forward (the brake 4.5 back), the only way to cover a friend or time a
   shutter.
5. `griffinwing_tell` = 50 (40 to 120): ticks a turret, a bomber, a shooting
   wisp and every Wyrm attack show their locked footprint before it lands.
   **Never under 40**, in the bound and again in the code.

### Tuning table (bot rounds; the operator picks from human play)
75 rounds, one knob moved at a time with the others at their starts, flown by
a sure bot at one seat and by a novice bot (late on the roll, loose on the
aim, letting go of a charge early, meeting the Wyrm's spit half the time) at
one and at four seats. Each cell is the run's clock in seconds, the Wyrm
fight's length, the hits taken and the lowest shield; `never` or a fall is
marked. The bots never use the spur, so that knob's rows show nothing, and a
human's rounds replace all of this.

`griffinwing_lock`

| value | sure, 1 seat | novice, 1 seat | novice, 4 seats |
|---|---|---|---|
| 20 | 208s / 52s, 5 hits, low 40 | 223s / 67s, 6 hits, low 40 | 205s / 49s, 19 hits, low 20 |
| 30 | 210s / 54s, 4 hits, low 55 | 213s / 57s, 4 hits, low 55 | 207s / 51s, 21 hits, low 5 |
| 40 (start) | 218s / 62s, 4 hits, low 55 | 239s / 83s, 6 hits, low 50 | 209s / 53s, 23 hits, low 0 |
| 60 | 228s / 72s, 4 hits, low 55 | 255s / 100s, 7 hits, low 25 | 218s / 62s, 19 hits, low 5 |
| 90 | 261s / 105s, 4 hits, low 55 | 320s / 164s, 8 hits, low 0, 1 fall | 234s / 78s, 23 hits, low 0 |

`griffinwing_roll`

| value | sure, 1 seat | novice, 1 seat | novice, 4 seats |
|---|---|---|---|
| 18 | 211s / 55s, 4 hits, low 55 | 225s / 69s, 5 hits, low 55 | 211s / 55s, 19 hits, low 20 |
| 24 | 217s / 61s, 4 hits, low 55 | 233s / 77s, 6 hits, low 50 | 211s / 55s, 18 hits, low 20 |
| 30 (start) | 218s / 62s, 4 hits, low 55 | 239s / 83s, 6 hits, low 50 | 209s / 53s, 23 hits, low 0 |
| 45 | 219s / 63s, 4 hits, low 55 | 249s / 93s, 5 hits, low 55 | 212s / 56s, 18 hits, low 20 |
| 60 | 223s / 67s, 4 hits, low 55 | 256s / 100s, 6 hits, low 50 | 219s / 63s, 18 hits, low 20 |

`griffinwing_window`

| value | sure, 1 seat | novice, 1 seat | novice, 4 seats |
|---|---|---|---|
| 40 | 218s / 62s, 3 hits, low 70 | 239s / 83s, 5 hits, low 50 | 209s / 53s, 23 hits, low 0 |
| 52 | 214s / 58s, 4 hits, low 55 | 243s / 88s, 8 hits, low 20 | 212s / 56s, 18 hits, low 20 |
| 64 (start) | 218s / 62s, 4 hits, low 55 | 239s / 83s, 6 hits, low 50 | 209s / 53s, 23 hits, low 0 |
| 78 | 214s / 58s, 5 hits, low 40 | 243s / 87s, 8 hits, low 25 | 212s / 56s, 18 hits, low 20 |
| 90 | 218s / 62s, 6 hits, low 15 | 239s / 83s, 8 hits, low 15 | 212s / 56s, 24 hits, low 0 |

`griffinwing_spur`

| value | sure, 1 seat | novice, 1 seat | novice, 4 seats |
|---|---|---|---|
| 30 | 218s / 62s, 4 hits, low 55 | 239s / 83s, 6 hits, low 50 | 209s / 53s, 23 hits, low 0 |
| 50 | 218s / 62s, 4 hits, low 55 | 239s / 83s, 6 hits, low 50 | 209s / 53s, 23 hits, low 0 |
| 75 (start) | 218s / 62s, 4 hits, low 55 | 239s / 83s, 6 hits, low 50 | 209s / 53s, 23 hits, low 0 |
| 120 | 218s / 62s, 4 hits, low 55 | 239s / 83s, 6 hits, low 50 | 209s / 53s, 23 hits, low 0 |
| 180 | 218s / 62s, 4 hits, low 55 | 239s / 83s, 6 hits, low 50 | 209s / 53s, 23 hits, low 0 |

`griffinwing_tell`

| value | sure, 1 seat | novice, 1 seat | novice, 4 seats |
|---|---|---|---|
| 40 | 216s / 60s, 4 hits, low 55 | 213s / 57s, 5 hits, low 40 | 207s / 51s, 23 hits, low 5 |
| 50 (start) | 218s / 62s, 4 hits, low 55 | 239s / 83s, 6 hits, low 50 | 209s / 53s, 23 hits, low 0 |
| 65 | 214s / 58s, 4 hits, low 55 | 250s / 95s, 6 hits, low 50 | 221s / 65s, 28 hits, low 0 |
| 90 | 219s / 63s, 4 hits, low 55 | 224s / 68s, 4 hits, low 55 | 219s / 63s, 29 hits, low 0 |
| 120 | 224s / 68s, 4 hits, low 55 | 228s / 72s, 4 hits, low 55 | 225s / 69s, 19 hits, low 40 |


Why the starts: **lock 40** is the first value where a hold feels like a
charge (1.3 s) and a lamp's three locks fit in one open window, and the table
shows 20 turns the verb into a double tap while 60 and 90 stretch the Wyrm's
fight from 58 s to 72 and 90 s (a novice alone, 106 and 221 s with a fall).
**roll 30** keeps a twenty-tick reflect window, wide enough to meet a bolt on
the beat and short enough that it is a choice; 18 leaves eight ticks, 60 a
slow safe spin that lengthens every fight. **window 64** is half the canyon's
width and about two seconds across; the bots do not find it matters, a human
will. **spur 75** is two and a half seconds, a shutter's worth. **tell 50** is
the 40-tick floor with ten ticks of margin: the table shows a longer tell
simply lengthens the Wyrm (68 s at 120), a shorter one is not allowed.

## Fail and retry
A knight whose shield runs out falls (the craft tumbles away) and rises 75
ticks later at the formation with sixty hundredths of a shield, unhurt for two
seconds. When every knight is down at once it is a wipe: "TRY AGAIN" shows
for 24 ticks (0.8 s), then the beat starts over from its checkpoint (its
start, or for the Wyrm's fight the Wyrm's rising) with the shield mended and
the wing back. What the party banked stays; so do the Wyrm's lamps already
out. There is no menu. A fall costs the medal five points and a lost wingman
four.

## The adventure (about three and a half minutes)
Three beats laid end to end on one rail: Thunderhead Run (1800 ticks: aim,
hold and let go; the first gates; a rock arch; the first kite that latches
on), the Ruined Causeway (2400 ticks: turrets teach the roll and reflect; kite
convoys pay off the lock; dive-bombers from behind a ridge; shutters that
close on a clock, so the spur and the brake time them), the Wyrm's Roost (an
approach, then the Wyrm: three lamps, three phases, a fly-through home to the
lit beacon). The first two beats run to the clock; only the Wyrm must fall.
The bots clear it in 6260 to 6550 ticks, 3 minutes 29 to 38 seconds.

## Co-op layer (light)
One set of foes, bolts, gates and the Wyrm for the party. Health is a fixed
share per seat (`Foe::seats`), never less: a wisp 20 a knight, a turret 60, a
bomber 40, a lamp 120. More knights mean more of every wave, a turret's volley
gaining a pair of bolts for every three knights beyond the first, a Wyrm that
rests less between attacks and spits more, and fewer riders in the wing (3, 2,
1, 0). A knight who sits down mid-run flies in at the formation unhurt for a
moment and scales the beat's foes by its share; a seat already provisioned
does not scale them twice. Kites latch onto anyone: a knight can roll one off,
and a friend drops back (Q) until the chaser is in front and shoots it.
The Wyrm shares roles: rollers turn its spit and open the lit lamp's shield,
chargers lock it and break the lamp.

## Engine route (recorded)
*Why not race3d's flight.* `fly_griffin` is private, and race3d is a hall's
ride (a gorge, rivals, antes, a finish board) hosted per room; it has no
shared foes, no lock, no reflect, and I may not edit it. So this is its own
`Session` in rail space (along, across, up) on the integer 30 Hz sim, 256 to
the unit, with its own projection into the engine's CPU rasteriser. It reuses
race3d's *idea* (the chase camera: back, up, ahead, a follow factor), the
rasteriser itself (`render_meshes`) and its dot snap (`dots_with`), and the
mesh idiom for the griffin. There is no free 3D collision: every hit is a
swept test of a bolt's segment against a body in rail space. f32 appears only
in the canyon's pose, the camera and the picture, and their trig is
`rules/griffinwing/trig.rs` (basic arithmetic only), so wasm and the host
agree to the bit.

## Honest fidelity limits
- **No camera roll.** The rasteriser's view has a heading, a pitch and a field
  of view, nothing more. The bank shows on the craft (leaning up to 24 degrees
  with a lag), in the camera swinging into a turn a little before the craft,
  and in the roll's full spin; the horizon never tilts.
- **The spur and brake slide the craft along the rail; they do not speed the
  world.** The beats run to the clock (a fixed rail is the point), so time on
  the tally is the run's own clock, falls and all.
- **The reticle's ray is judged from the craft; the camera sits above and
  behind it,** so near foes can sit a unit off where the reticle seems to be;
  every lock and body has a unit and a half to forgive it.
- **Every actor is a block** (boxes and flat quads in 3D, labelled), by the
  brief: the painted wave comes next. Far foes are a few dots, so each wears a
  pale pip until it closes.
- **The wingmen do not dodge or roll** and shoot a little; they are fillers
  that need saving, not players.
- **Sounds are the shell's existing names** (a crisp lock is `spoil_pickup`,
  the chain of pops `hit_magic` and `kill`); the art wave's sound requests are
  in ART.md.
- The campaign flags (`BEGUN`, `GOLD`) are set and settled; nothing else in the
  realm reads them yet.

## Pays
Each seat that flew the Wyrm down is paid once: **10 gold and 5 for each rank
of the medal** (15, 20 or 25), 5 and 2 a rank on a replay (the party has lit
the beacon before); the cap is 25, the same as the pastimes' first-win purse,
so a three-and-a-half-minute run is a reason to come back and not a faucet.
The gold medal gives every seat the laurel (the realm's crown buff). The
finish marks the Home once, crash-safe: `griffinwing:campaign-begun`
(`BEGUN`, the beacon lit) and `griffinwing:gold-medal` (`GOLD`), settled
through `settle`, idempotent; the Records hold the plays, the wins and the
best share taken (in tenths of a percent). A seat that joins during the tally
is not paid, and the paid mask keeps every other seat to one purse.

The medal reads the share of what the beats offered that the party took
(kills, gates, the Wyrm), less five for each fall and four for each lost
wingman: gold 85 or more, silver 60, else bronze. The score adds a chain's
bonus (ten for each hit beyond the second in a run of kills), 25 for a turned
bolt, 150 for a rescue and a gate doubled on the spur; those do not move the
medal.

## Tests and evidence
- **Rules** (`together_griffinwing_play__tests.rs`, 37 checks, plus the two explorers
  that print the bots' runs and the tuning rounds): steer and bank; the reticle (instant, kept);
  the tap (a twin bolt to where the reticle points); hold-lock-release in a chain
  (a bracket each, a starburst each, two ticks apart, a hit counter that pays
  more); six locks at most; the roll and its knob (reflect window follows it),
  the double tap; the spur's meter and the brake; gates, rings, the arch's
  scrape, shutters on their clock; Starfall (bolts cleared, three a beat, a held
  key is one); a kite latching onto a rider, draining it, being shot off it
  (rescue, call-out), a rider lost when nobody does, a roll shaking a kite off a
  knight, a friend dropping back to shoot a kite off a friend; the Wyrm's lamps
  (shut takes nothing, a turned bolt opens it, only a charged starburst breaks
  it, three phases, down); every Wyrm attack's footprint shown at least forty
  ticks; the coil and the roar; the wipe and retry in under a second, in the
  Wyrm's fight too (lamps kept, no second Wyrm); two knights both mattering;
  more knights meaning more foes with more health, never less; mid-run join
  (scaled by a share, once); the same keys replaying to the same bytes; the
  five numbers each moving what they name; the purse once and the smaller
  replay purse; a bad save mended; random hands and a table that comes and goes
  never breaking the session (twelve tables of keys held at random for up to forty
  thousand ticks, and bots flying on while knights step away and come back, the
  session valid every tick: it caught a rider rising from a fall out of bounds); every line
  of the wing short; the slots file.
- **Shell** (`together_griffinwing__tests.rs`): valid and repair, the door and a
  personal leave, two seats and a late knight, six seats byte-identical, hold Esc
  (15 ticks, one seat stands up), the restart, marks and knobs, a closed run
  written as today's, the finish settling once.
- **Bots** (`ADVENTURE.bot`, a sure hand that reads the live numbers) fly all three
  beats at 1, 2, 4 and 6 seats to the Wyrm, the tally and the way home: **6550,
  6403, 6346 and 6260 ticks** (3:38, 3:34, 3:32, 3:29), no falls, all gold; at 6
  seats they took 34 hits and one knight fell once (the rest of the party flew on). The novice hand falls
  once in the 75 tuning rounds (a charge of 90 ticks).
- **Pictures** (`write_griffinwing_review`, `write_griffinwing_scenes`,
  `write_griffinwing_canyon` in the delve crate; `write_griffinwing_terminal_review`
  in the cockpit): every state was opened at 1x, 3x and in the terminal's cells:
  the title card, wisps, locks and the charge ring, a latched kite and its
  call-out, gates, turret and bomber footprints, the reflect, a convoy, a
  shutter, the arch, the Wyrm rising, its spit, coil and roar tells, a lamp open,
  the victory and its beacon, the six-seat tally, the retry, a hit, a fall, the
  spur's streaks, Starfall, a roll, a thin shield, and the kingdom door. What
  those pictures changed is listed in the build record below.
- **Suites.** `cargo test -p angel-delve`: 1680 passed, one load-sensitive
  failure that passes alone (`v2_is_deterministic_rgb_and_small_frames_use_v1`,
  on the known list), 200 ignored; no new warning (10 in the library, 15 in its
  tests, the same as `dev`). The cockpit's `roundtable dungeon world_viz`: 474
  passed. The cell-proof pins are fresh (nothing pinned was touched).
- **Wasm.** The smoke runs; `parity.mjs griffinwing` and the native `parity`
  print the same 488 lines (all four state hashes and the frame's hash, every
  tick, both art kits). That took the camera and the trig being done by hand:
  the rasteriser's own `sin` and `cos` differ between the browser's wasm and the
  host in the last place, and one tick in sixty drew differently. A wire tick of
  this scene is 151 bytes (the whole first snapshot 187 KB, as every adventure's).

### What the pictures changed (the build record)
- The canyon was black paper: the realm's night look lights only what the
  moon or a lantern touches. The stage now lifts the tone, hangs lit windows,
  lanterns and braziers on the walls, strata and buttresses so the walls have a
  grain going by, moonlit mist in dashes in the deep, and paints its own sky
  over the bare dots (a dusk that glows at the horizon, clouds that slide with
  the heading, the moon).
- The own craft filled a third of the screen: the camera went back (7 tiles) and
  the craft shrank (0.7), the lean was cut from 31 to 24 degrees, the eye swings
  into a turn a little before the craft.
- Far foes were a few dark dots: wisps and kites doubled in size and carry a pale
  ring of cold light, and every far foe wears a pip; labels are limited to five
  and left off what is locked.
- The Wyrm's head was a grey slab and its lamps hid inside it: it is armoured and
  scaled up, with eyes, brow, teeth and a horn lantern, the lamps hang in front
  of the face, and the lit lamp is the biggest and brightest thing on it.
- Brackets and the reticle were one dot thick and vanished in the cells: they are
  two dots thick now; the hatching for the coil's and the roar's footprints is
  half as dense.
- The call-outs' longest line did not fit its plate: the plate is wider and every
  line is under 28 characters (a test keeps it so).


## Version 1 leaves out
A second rail, branching paths, wingmen that fly the player's moves, real
camera roll, free 3D collision, new buttons, a podium, the painted art and
its sounds. One verb set, three beats, the whole party.
