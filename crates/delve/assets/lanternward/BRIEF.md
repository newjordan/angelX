# Lanternward

> The top-down cousin of the classic lock-on sword-and-shield dungeon
> adventure, for the whole table. The Underkeep, the vaults beneath Moonwell
> Keep, and a beacon to relight. Keep every verb, change only the camera.
> Re-read this before every change; when the code and this file disagree, fix
> this file first.

## The feeling
You hold a button and the nearest snuffer is yours: a small gold spark settles
over it, the knight swings square to it and the view leans in with a crisp
tick. You circle it, hop sideways out of its lunge, strike the third cut a
little heavier than the first two, and the shield you raised in the last
moment rings with a chime and a flash and leaves it staggered for a full
second. That is the first ten seconds: **lock, circle, hop, cut**.

## The loop (one sentence)
Five rooms and one boss: lock on and cut the Stair's snuffer, light the Lamp
Hall's braziers from one ember to open the Lantern, shield a turret's bolt into
a crystal in the Arrow Gallery, burn the hood off the Key Vault's snuffers in
the dark for the Moon Key and the Boss Key, then light the Warden's lamp-heart
and strike it while it burns, and relight the Beacon.

## Words on the title card
LOCK ON. STRIKE. BLOCK.

## Who plays
Every seated knight, at once, on their own screen and their own camera. The
foes, the braziers, the doors, the keys and the checkpoints are the party's;
health, flame and gems are each knight's. Up to six; one knight alone can
finish it (slower, and with every foe scaled down to one share, never below).

## Controls (the Delve's own buttons; nothing new)
| Button | Keyboard | Pad | Does |
|---|---|---|---|
| Move | WASD | d-pad / left stick | walk; locked on, a sideways step circles the foe |
| Face | arrows | right stick | turn without walking; while locked, a fresh press moves the lock to the next target that way |
| **Lock** (hold) | **R** | LB | lock on |
| Sword | **Q** | B | tap: three-cut combo; hold and let go: spin; locked and walking in: lunge; in the air: jump attack |
| Shield (hold) | **Space** | A | block the front; the first `lanternward_parry` ticks parry |
| **Context** | **E** | Y | open, read, talk, lift, throw, take flame, light, unlock, pull, revive, relight; with nothing to do: roll, or locked on, hop (stick to the side), backflip (stick back or nothing), forward flip (stick toward) |
| Lantern | **F** | RB / RT | tap: throw a bolt of light; hold: flare |
| Bomb | X | X | lay one |
| Leave | hold Esc | menu | the Delve's own 15-tick retreat |

R is the Delve's ultimate key; inside Lanternward nothing reads it but the
lock (the ride takes the seats' keys for the tick, as every other ride does).
The label at the foot of the screen says what E would do now (`E OPEN`,
`E TAKE FLAME`, `E ROLL`, `E HOP`). The spells on 1 to 4 do nothing here.

## How the Familiar contract is met
1. **Lock-on.** Hold R: the nearest foe in front, a foe before a switch, the
   nearest at all if nothing is in front, nothing through a wall. A gold
   wick-spark floats over the target with four brackets that close on the take;
   the knight faces it on the same tick; a tick sound (`card_pickup`); the view
   leans 38 % of the way to it, fast for 6 ticks then easing; on release it
   eases back over 10. While the Warden is up and is not the lock the view
   also leans toward it (up to 52 px) so its whole body stays on screen, and its
   bar sits at the foot of the picture, not over it. Circling keeps the radius (`kit::orbit`). Hop, backflip
   and forward flip are E while locked; Q while locked and walking in is a
   lunge. Targets: foes, the Warden's lamp-heart, signs, the kingsman,
   crystals, braziers, the great brazier, sigils, levers; a switch shows its
   hint line while locked.
2. **Sword.** Three cuts (2, 2, 4), the third a wider, longer arc with more
   hit-stop and a shake; hold Q 22 ticks and let go for the spin (3 all round);
   E then Q in the air is the jump attack (4, a ring where the knight lands).
3. **Shield.** Hold Space: blocks the front 130 degrees, walking slowly. A
   bolt that meets it is turned back (dim and short if late, bright and fast
   if a parry), toward the lock if there is one, else where the shield faces.
4. **One context button.** E; its label changes with what the knight faces.
   *Not met: climb* (a top-down room has no height to climb).
5. **Hearts, gems, pots and grass.** Five hearts in quarters (a container adds
   one); hearts and gold gems fall from foes, pots and grass; pots lift and
   throw; one pot in the Stair always holds three bombs.
6. **Dungeon grammar.** A map (a chest in the Stair; the five rooms in the
   corner), small keys and a locked door, a Boss Key door (carried, dropped
   where its carrier falls, spent on the door), the Lantern (braziers by bolt,
   hidden sigils by flare, hooded snuffers by flare), a push-block and a
   pressure plate, torch-lighting from an ember, crystal switches that take a
   thrown or a parried bolt, bombs for a cracked wall, a lever.
7. **Boss.** The Warden: tells of at least 40 ticks, the lamp-heart shut until
   light falls on it, three phases, a heart container and the item-get pose.
8. **Hint companion.** A wick-wisp floats by the knight; one short line at the
   foot ("LOCK ON, THEN HOP: E").

## How the Quality bar is met
- **The snap**: facing on the tick, the tick sound, the view's fast swing and
  slow ease; `locking_on_snaps_the_view...` holds all three.
- **Hit-stop on every sword connection**: each connection asks the engine for
  its hold (`hitstop::Impact::Heavy`, rate-limited by the engine's own
  cooldown), a kill asks for the elite-kill hold, a parry or a phase for the
  phase hold, the Warden's fall for the boss-kill hold; and the knight who
  struck freezes 2 to 4 ticks and the foe 3 on its own, so six knights never
  freeze each other.
- **Block ring and parry chime**: `shield_block` for a block, `holy_water` and a
  white flash for a parry (shield raised at most `lanternward_parry` ticks
  before the blow); the foe is staggered 30 ticks (one second) and its next hit
  counts double.
- **Puzzle solved**: `wish_grant`, a star burst, and **every camera looks at the
  door or switch for 44 ticks** (the cue), lighting it if the room is dark.
  **Item-get**: the knight holds it up for 60 ticks and every screen names it.
- **Low health**: at two hearts or under the hearts pulse white and a chirp
  sounds every 40 ticks. The third cut is heavier; foes flinch; a snuffer dies
  in a puff of smoke and a spark of light and stays where it fell.
- **Fair bosses**: the Warden's and the turret's tells are `lanternward_tell`
  ticks for one knight, 3 less for each extra, never under 40; the footprint
  (fan, circle, ring, line) is drawn from the first tick of the tell and never
  changes (`every_big_foe_shows_its_whole_footprint...`). A snuffer's tell is a
  fixed 24 ticks.

## Co-op layer (light; every verb is every knight's)
Each knight has their own lantern flame (a tank, `lanternward_flame`) and a
friend's flare kindles it (stand within 28 px: the tank fills twice as fast).
Group moments: the Lamp Hall's three braziers are all to be alight at once
(each burns 20 seconds alone, 2 less for each extra knight, never under 10) so
more knights light them faster against a shorter burn; the Gallery's
partition gate needs two plates from two seats, **the block on the first and
another knight on the second**, who then holds the door while a third parries
the turret's bolt into the crystal; the Vault's snuffers are hooded in the
dark, so one knight's flare lets the others' blades count; from the Warden's
second third **two knights' light must be on the lamp-heart at once** (one
knight alone needs one), and a parried beam opens it too. Fallen knights are
lifted by a friend holding E (45 ticks), or carried to the room's brazier
after 10 seconds, or on the party's way into the next room.

More seats: more foes (a second snuffer at four seats in the Stair, up to four
in the Lamp Hall's wave, up to five in the Vault, a second turret at four, up
to four summoned by the Warden), a fixed health share per seat times the seats
at the table (a snuffer `SNUFFER_HP`, the Warden `WARDEN_HP`; a knight who
joins mid-run scales the standing foes up once and keeps their share), tells 3
ticks tighter per seat down to the floor of 40. Never fewer foes, never less
health, never less damage.

## The adventure (a bot clears it in 2.5 to 3.5 minutes; a table's first time is 5 to 8)
1. **The Stair** (22 by 18): the party wakes by a resting brazier. A sign
   ("HOLD R TO LOCK ON"), a kingsman, pots, grass, a bomb pot, a pit with a
   chest across it (roll or hop), one snuffer.
2. **The Lamp Hall** (28 by 16): take flame from the ember bowl, light the three
   braziers all at once, the niche gate rises, the **Lantern** is in the chest
   (item-get). The Gloam answers with a wave; then throw light through the bars
   at the great brazier to open the way. A hidden sigil in the corner is shown
   by a flare and opens a closet.
3. **The Arrow Gallery** (36 by 14): push the block onto the plate (and, from
   two seats, hold the second plate), cross, and meet the turret's bolt with a
   shield toward the crystal in the east wall. A cracked wall takes a bomb.
4. **The Key Vault** (30 by 18, dark): hooded snuffers take no blade until light
   burns the hood off. When the last falls the Moon Key's chest appears and the
   braziers light; open the small-key door, take the Boss Key, carry it to the
   Boss Key door. A lever across the room opens a closet.
5. **The Warden** (30 by 24): light the lamp-heart (a thrown bolt holds it open
   for 90 ticks) and strike it; parry the beam, hop the ring, shield or dodge
   the summons; at the last third the Gloam snuffs the arena's lamps. Take the
   heart container, then hold E at the **Beacon** to relight it.

## The five numbers (live F2 knobs; fixed integers; HZ is 30)
1. `lanternward_lock` = **112** px (48 to 200): how far a lock reaches. Under
   about 104 the Gallery's crystal is out of reach of the lock (the player can
   still face it and parry).
2. `lanternward_parry` = **8** ticks (3 to 14): how soon after the shield goes
   up a blow is a parry.
3. `lanternward_combo` = **14** ticks (6 to 24): how long after a cut the next
   one still chains.
4. `lanternward_flame` = **150** ticks (60 to 300): the lantern's tank; a bolt
   costs a fifth, a flare a tick a tick, it refills a tick every second tick,
   three a tick by a resting brazier.
5. `lanternward_tell` = **56** ticks (40 to 90, never under 40): a big foe's
   warning for one knight; 3 less for each extra knight down to 40.

### Tuning record
Every row is bots playing the whole adventure (or the Warden alone) from the
door, replayed byte for byte (`print_the_tuning_tables` and
`print_the_bot_record` in `tests/rules/together_lanternward__tests.rs`; the
bots are `rules/lanternward/bot.rs`, which presses the same buttons a person
has). A cell is one deterministic run, so it finds a cliff and does not rank
close neighbours; the parry and combo rows are the micro-rounds that map the
window itself. Ticks are 1/30 s; hearts are in quarters.

| Number | Values tried | Rounds | What it did | Chosen, and why |
|---|---|---|---|---|
| lock (px) | 64, 80, 96, 112, 160 | 10 adventures (1 and 3 seats) | 64 to 96: the Gallery's crystal (about 100 px from where the bolt can be met) is out of reach and the bots stop there; 112: through in 4,315 / 5,307 ticks; 160: 5,124 / 5,353, and it takes things across the room before the knight has seen them | **112**: seven tiles; enough for the crystal and for a room, short of grabbing what is far off. (A player can still face the crystal with the arrows and parry without a lock.) |
| parry (ticks) | 4, 6, 8, 12 | 32 lunges (a shield raised 0 to 16 ticks before the blow) | the parry band is as wide as the number: at 4 only a shield raised in the last 4 ticks parries, at 12 one raised 12 early still does, and a held-down shield nearly always parries | **8** (the brief's start): a quarter second, to be found inside a snuffer's 24-tick tell and the Warden's 56; a shield held early still blocks |
| combo (ticks) | 6, 10, 14, 20 | 28 mashers (a cut pressed every 8 to 24 ticks) | at 6 the chain drops if the next press is 20 ticks late; 10 and over hold it to a 20-tick gap; at 24 only 14 and 20 still chain | **14**: a casual tapper keeps the combo, a slow one has to start over |
| flame (ticks) | 60, 100, 150, 220 | 12 Warden fights (1, 3 and 6 seats) | the Warden falls in 2,191 / 1,595 / 1,429 ticks at 150 and within noise of that at 60 and 220: a bolt costs a fifth of whatever the tank is, so five bolts is five bolts; what the tank sets is the longest flare: 2 s, 3, 5, 7 | **150**: five seconds of flare burns a hood and lights a heart twice, short enough that the Warden room's braziers still matter |
| tell (ticks) | 40, 48, 56, 70, 90 | 15 Warden fights (1, 3 and 6 seats) | bots read the footprint exactly, so the cells separate little; at 40 three seats lose 54 quarters of hearts against 28 at 56, and the solo bot needs a retry at 40 to 56; 70 and 90 lengthen the fight without losing fewer hearts | **56**: 1.9 seconds, the Forest Road's 60 / 53 / 46 convention of at least 1.5; at six seats it falls to 41, never under the floor of 40 |


### The fixed numbers (not knobs)
A knight walks 1.7 px a tick, 1.3 locked, 0.9 shielded. Slashes: wind 2, 2, 4
and active 3, 3, 4 ticks, rests 6, 6, 12; reach 18, 18, 22 px; damage 2, 2, 4.
Spin 14 ticks, 22 px, 3; wound after 22 ticks of Q. Lunge 3 + 6 + 10 ticks, 30
px, 3. A swing's recovery gives way to a dodge or the shield. Roll 15 ticks
(i-frames 2 to 10), 56 px; hop 18 ticks (i-frames 2 to 14), about 40 px and 14
px high. A hit wards the knight 36 ticks. A snuffer: drifts at 0.8 px a tick,
lunges 44 px in 8 ticks after a 24-tick tell, 2 damage; the hood stays off 60
ticks after light. The turret: its bolt flies 2.4 px a tick, 2 damage, and is
stunned 90 ticks by a parried bolt. The Warden: its beam sweeps 36 ticks
across a 100-degree wedge, 4; its ring runs 30 ticks out to 132 px, 4; a light
holds the heart open 90 ticks; a parry staggers it 75.

## Fail and retry
A knight at zero hearts falls where they stand; a friend's E (45 ticks) lifts
them with three hearts, or the room's brazier carries them after 10 seconds. A
fall into a pit costs half a heart and puts the knight back at the edge. When
**every** knight is down, 24 ticks later (under a second) the party is back at
the room's start with full hearts, the encounter as it opened (the Warden at
the start of the phase it was in) and every solved thing still solved: keys,
the Lantern, lit braziers, open gates. No menu between tries. Leaving through
the door and coming back is a fresh run.

## Version 1 is only
Five rooms, one boss, one beacon; the Gloam's small foes and the Warden; no
shop (gems are the purse), no second map, no spells, no climbing, no swimming,
no mounted fight. The camera is fixed above, one per seat.

## Door
Kingdom tile (104, 23): the Mines' yard, west of the mine mouth. A stone arch over
a dark stair with a lantern hung in it; the lantern burns once the Beacon has
been lit. Walk up and press E; hold Esc leaves; the way back is the same tile.
The door is the integrator's to move.

## Pays
- The campaign mark `lanternward:campaign-begun`, settled once into the Home as
  the `BEGUN` bit of `Home::roundtable[SLOT]` (the door's lantern then burns);
  `Records::roundtable[SLOT]` counts plays and wins and keeps a best (1000 less
  2 for each second over three minutes, at least 1).
- **A purse of gold, paid once to each knight at the table at the Beacon**: 40
  for the first clear, 12 after, plus the gems that knight carried, up to 20: at
  most 60 a knight, 360 for a full table. The cap is the Pennon Rush's pot (10 a
  seat for 75 seconds) scaled to an adventure about eight times as long, and no
  more: gems come only from the Underkeep's own floor and are bounded by what
  it holds. A repeated finish pays nothing again.
- The realm's usual crown: every knight at the Beacon wears the gold crown.
- A knight who left before the Beacon is not paid; one who sat down late and is
  there is.

## Honest fidelity limits
- **No real 3D.** The classic's camera sits behind the knight and turns with
  the lock; here it is fixed above and *leans* toward the lock, and the
  knight's facing, not the view, turns. A hop is a hop along the ground with a
  shadow and a little height (for the jump attack and for clearing a low
  blow); there is no vertical space to climb, swim or fall through but the pit.
- Circling is screen-relative with a magnet to the tangent (input within 37
  degrees of sideways is all sideways, within 37 of toward or away is all
  that), because a keyboard cannot aim at a tangent: held sideways it circles
  at the same radius until the stick is well off, then it closes or opens the
  gap. A pad's stick gets the same rule.
- The Delve's own hit-stop pauses the whole room; it is rate-limited by its
  cooldown, so every landed cut asks for it but a table of six is held at most
  once in 14 ticks. The knight's and the foe's own freeze is local.
- No climbing, no horse, no boat; the context button stops at open, read,
  talk, lift, throw, take, light, unlock, pull, revive, relight.
- The art is blocks: the Delve's own knight stands in for the hero, and every
  foe and prop is a labelled placeholder.

## Reskin map (original to ours)
Lock-on arrow to a gold wick-spark and four brackets; sword to a broad sword
with a sweeping arc; shield to a heater shield (parry chime); boomerang or
hookshot item to the **Lantern** (light as a tool and a weapon); fire-lighting
torches to braziers lit from an ember; the boss's item weakness to a lamp-heart
lit only by lantern light; rupees to gold gems; the fairy to a wick-wisp; the
light and dark worlds to lit rooms versus the Gloam-dark Vault; the map and
compass to the five rooms in the corner; pass the lantern to a friend's flare
kindling yours.

## Identity before pixels
In `ART.md`; slots in `slots.json`; concept sheets (read-only) in
`~/angelX_minigames/games/lanternward/assets/raw/`.

## Test and bot record
`cargo test -p angel-delve --lib lanternward`. Four network-style bots (one
per seat, `roundtable_inputs`) clear the whole adventure from the door,
identically on every run, at one to six seats; the ticks to the Beacon are
the table below. The tests: valid and repair; a scripted win at one to six
seats and what it pays, once; a wipe and its retry inside a second; a fallen
knight lifted; two seats where both matter (the plates, the lamp-heart); six
seats and the scaling that never goes down; a knight who joins mid-fight; the
escape hold; determinism over half a minute of play; a block never shoved
against a wall; one test for each verb and each item of the quality bar. The
renders are `write_lanternward_review` (forty moments at 1x and 2x, and
filmstrips of each verb) and the cockpit's `write_lanternward_terminal_review`.

| Seats | Ticks to the Beacon | Seconds | Retries |
|---|---|---|---|
| 1 | 4,315 | 143 | 0 |
| 2 | 5,082 | 169 | 0 |
| 3 | 5,307 | 176 | 0 |
| 4 | 5,576 | 185 | 0 |
| 5 | 5,090 | 169 | 0 |
| 6 | 5,275 | 175 | 0 |

The bots read every footprint exactly and never dawdle; a table's first time
through (reading the hints, learning the lock, missing a parry) is about twice
that. That is an estimate from the pacing of each room, not a measurement of people: the table (five to seven minutes) should be played and the Warden's health (`WARDEN_HP`) trimmed if it runs over.

