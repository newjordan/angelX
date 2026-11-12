# Pennon Rush

## The feeling
A boot leaves a broken parapet late, the hook-bow barks in mid-air, and the
opposing pennant folds before the knight lands.
First ten seconds: run, late jump a gap, release for a short hop, shoot a marked sentry.

## The loop
Sprint one seeded rampart, jump three breaches, duck or jump sparse aimed bolts,
and knock down the pennant-bearing siege captain before the other knights.

## Words on the title card
PENNON RUSH, BREAK THE SIEGE; then 3, 2, 1, GO (the lanes wait two seconds, so the
whole table starts on one breath)

## Who plays
Every seated knight at once in a personal copy of the same seeded battlement.
The siege muster house, a separate building north of the west-wing loft, contains the game.

## Controls
WASD RUN / ARROWS AIM / F SHOOT / E JUMP / S DUCK / E AGAIN
Arrow keys aim in eight directions independently of running; movement also aims
when the aim stick is released. S without sideways movement ducks.
E is jump during play and the existing interact/rematch button outside play.

## Five numbers
30 Hz, integer fixed point, 256 units per native pixel; live F2 knobs:
1. pennon_gravity = 112 units/tick²: weight after the apex.
2. pennon_jump = 1536 units/tick: held jump clears a breach.
3. pennon_run = 640 units/tick: room to steer a shot during a jump.
4. pennon_coyote = 4 ticks: a late boot still owns the edge.
5. pennon_buffer = 5 ticks: an early jump waits for the landing.
Releasing E halves upward speed once. S without movement ducks; sideways down aims diagonally.

## Feel ledger
| Rank | Knight | Response | Number |
|---|---|---|---|
| 1 | Late jump | Still springs | coyote |
| 2 | Release E | Short arc | jump |
| 3 | Run and shoot | Fast aimed bolt | run |
| 4 | Press before landing | Springs at contact | buffer |
| 5 | Land | Solid dust and crouch | gravity |
| 6 | Hit sentry | Foe freeze, burst, chain | 2 ticks |
| 7 | Fall | Return to safe parapet | 18 ticks |
| 8 | Read captain | Hook lane or overhead banner-drop footprint | >=25 ticks |
| 9 | Win | Captain folds, raised plume, gold | 10G per seat |
| 10 | Rematch | Immediate new rampart | one tick |

## Version 1 is only
One 75-second stage, three sentry waves, two repair pickups, three breaches,
one captain. No campaign, ladders, spells or bullet fans. Reuse Rafterwake's
integer contacts, camera clamp and cached native-art helpers; one shared input
sample allowance, clamped by the host's existing 250 ms function.

## Identity before pixels
Captain: split swallowtail pennant on an offset mast, broad ladder-hook, ribbed
copper crest. Face: rounded slit visor with a curved amber signature light.
Sentry: twin-prong wind vane, forked wooden bow, hanging seed-weight.
Rampart: rounded blue slate merlons, curved iron tie rods, split cloth signals.
One native grain, strict four-tone hue-shifted ramps, coloured contours, no mottle.
Tagged long ribs, seams, rods and hems retain material character in real cells.
Captain stands 77px beside a 45px knight and fills 96% of an 80px cell.
Full states: idle, walk pair, overhead tell/attack, low hook tell/attack, flinch,
two dark-backed damage stages, defeat and recover. Four stages for both tells.
Exact footprints remain fixed throughout each >=0.75-second wind-up.

## Fail, stake and retry
A hit or fall crumples for 18 ticks and returns to the last safe parapet, preserving
work. Host ranks progress (route plus foe hits), fewer falls, then earlier finish;
exact ties split 10 gold per seat and share the existing crown buff. Best saved.
One flat podium panel; E AGAIN sits above its border, away from the volume control.

## Tuning record
Five rounds per proposal at 1/2/4/2/4 seats. The heavy 144/1536/512/2/3 proposal
cannot clear the breach (17 falls; 75-second timeout). The floaty
96/1664/768/5/6 proposal clears in 645 ticks but gives excess air travel.
Selected 112/1536/640/4/5 clears in 681 ticks with no falls in the scripted course;
its lower arc keeps the breach edge relevant. Sixteen seeded delayed-view cases
at 0/3/6/11 ticks also clear. The bot takes off twelve pixels before the breach,
holds through the rise and reads the captain's exact footprint. Live F2 applied all three proposals with three network bots and restored defaults.
The network bots cleared in 793/802/803 ticks with one fall each; F2 captures show
all five selected values.

## The night of 2026-11-07: how it looks and feels
The whole screen is the rampart at night: a crescent moon, a camp of fires and siege
engines sliding at three paces behind the wall, a moat lapping its foot, torches that
throw light, pennants that wave, fire arrows lobbed over. Two thin strips frame it:
the clock, a rail of every knight's pip against the lane's pennons, the table's rally
gauge and the pot above; a chip for each knight (crown on the leader, shield when warded,
a red tick a fall) below. The other knights run the same wall as faint ghosts in your window.
Every blow lands: a white-grey frame, squash and knock-back on the struck pennon, sparks and
a number; a felled pennon throws splinters, holds its lane 4 ticks and flies its score to
your chip; the captain's bar drains with a pale chip; his blow shakes the screen and rings
along the stone. A knight struck flashes, shakes and rims the screen red, and the run of hits
shatters; a fall into a breach splashes in the moat. Calls (SENTRY DOWN, X5 COMBO, THE
CAPTAIN, BANNER TORN, LAST STAND, SIEGE BROKEN, TAKES THE LEAD, WARD FOR ALL) stand over
the strip; the last five seconds are big numbers. Every third pennon felled at the table
hands a ward to every knight without one (the rally). The captain's fall holds the room 6 ticks;
the end is film bars, TIME UP or SIEGE BROKEN, an iris, then the wall top at night with
fireworks, a podium, paper, the pot counting up, NEW BEST and E AGAIN.
Words: at most three plain words a prompt, nothing overlapping, nothing past 384 by 224;
YOU is only the knight looking, every other knight by name (KNIGHT n for the default).
The state is `Session::show` (`rules/pennon/show.rs`); the picture is `arena/pennon/*`.
