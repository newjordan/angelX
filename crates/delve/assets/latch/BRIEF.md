# Latch Duel

The feeling: a heavy copper practice automaton commits its arm, you catch the
last beat, and your knight's riposte buckles its bellows with a ringing clash.
First ten seconds: read the overhead hook or floor rake, hold that height,
then press F as the shaped tell closes; feel the first clean counter freeze.
Loop: read Clasp's high or low wind-up, counter at its height in the closing
window, and race every other seated knight to three clean counters.
Title card instruction: READ THEN COUNTER.
Every seated knight fights their own Clasp at once, same seed and attack deck.
Controls: up/down chooses guard height; F counters; E starts or rematches.

Five live knobs, integer ticks at 30 Hz:
- latch_tell = 36: deliberate readable wind-up, never below 23 ticks.
- latch_window = 7: final ticks that accept a counter; no early mashing.
- latch_rest = 24: breath between commitments, shortens by 3 per damage stage.
- latch_recover = 18: clear fail/clean beat, returns control within 0.6 seconds.
- latch_stop = 4: local hit freeze before recoil and the next commitment.

Miss: a blunt copper sweep through the exact published high/low contact band,
a short red-brown guard collapse and a dull sound; score stays, chain breaks.
Three clean counters wins the race. Equal counters use fewer misses, then
smaller summed timing error; exact ties share purse and crown, spare coins go
in seat order visibly. Every completed round pays 10 gold per seat, at most
40 total, to the leaders. Crown uses the existing next-delve bomb buff.
E rematches on the next tick after release; no menu or ante.

Version 1: one smithy practice bay inside The Dented Anvil; two distinct arm
attacks, independent seat states, live guttered progress, timed result podium.
No movement arena, bullets, extra weapon or new economy. Reuse existing cues.

Ranked feel ledger:
1. Wait for the arm to commit -> a wide shaped tell closes -> latch_tell.
2. Catch the last beat at its height -> clean contact freezes -> latch_window.
3. Land -> copper recoil and burst -> latch_stop.
4. Miss -> guard folds -> latch_recover.
5. Breathe -> next arm winds -> latch_rest.
6. Read the other arm -> distinct high hook / low rake -> deferred third move.
7. See friends' counters -> independent strips -> existing seats.
8. Gain damage stages -> buckled plates -> three counters, fixed.
9. Finish -> timed/error ranked podium -> same rules above.
10. Rematch -> next shared deck -> E edge, one tick.

Tuning review: five bot rounds per proposal (1, 2, 4, 2, 4 seats). The faster
30/5/18/15/3 proposal finished in 6.3-6.4s, leaving little first-read space.
45/10/30/24/6 took 9.8-9.9s and held each clean beat too long. Promoted
36/7/24/18/4: 7.7-7.8s with a 1.2s tell, 0.23s counter window and 0.13s
local freeze. Scripted bots missed zero. In the live four-seat round the three network
bots landed 2/3/2 counters with zero misses; one won while the idle browser
seat missed. Bot presses move earlier within the same window to allow transit.

## Polish: two bodies meeting (GAME-2B)

The first counter must visibly travel from the knight's braced body into Clasp.
Keep the existing rules, all five timing knobs, nine rival states, effect shapes,
purse, tiebreak and podium. No new attacks or tuning changes.

1. Reuse each knight's armour, colours and plume. Attach a broad shield to the
   sword-side arm: top at the helmet for high, down at the shins with a 2px crouch
   for low. Three cream-ramp thrust frames replace the thin stock sword stroke.
   Bake enlarged figures and overlays; integer stamps only during frames.
2. Bring the knight 24px toward Clasp; compose a clear contact gap under 40px.
   Use 4x knight pixels and a cached 100px rival, keeping their floor anchors.
   Anchor the retained effects to the hook/fist, remove stray green contact lines,
   and place a dim smithy recess in the unused upper space.
3. Give the instruction a 3px black surround, lower guard/result/count labels
   8px, match the count to the guard tone, and lift browser volume 16px.
4. Lower the low-tell free fist 12px and take its hook tip to the floor.
   Remove the pointer from both tells and remove the detached red guard pip.
5. Restore two 2x2 warm eyes after cleanup in every Clasp state, on a single
   darker visor row. Replace damage-2 helmet striping with broad metal planes.
6. Pair terminal seat swatches and scores, align podium headers and values,
   preserve panel backgrounds under text, and express timing in seconds.
7. Trace the existing command clock. Judge counters at the client sample tick
   if existing message extensibility supports it; preserve the envelope and
   round trips. Exercise late valid and truly mistimed presses in rules tests.

Review native whole-room high/low/thrust views, 3x pixels, both kits and actual
half-block/braille buffers. Reject lost identity, detached gear, noisy contours,
washed-out lights and contacts that float away from either body. Verify renderer
cost with smoke before/after and run the full delve suite once at the end.

Fairness implementation: a counter carries the tick of the duel actually drawn,
with its selected guard, in an optional field of the existing command body.
The original tell epoch survives the existing miss recovery. A delayed valid
press replaces only its automatic expiry; an earlier attempt consumes the tell.
The existing final-contact hold can accept inputs sampled on the winning tick.
The command envelope, handshake and round trips are unchanged. No new wait or
latency budget is introduced; a tell that has advanced to its next attack, or a
podium already paid out, is final.

## Second polish: two equally made bodies (GAME-2C)

The guard should look braced and the counter should move the knight's whole
weight. Keep the room composition, floor and contact anchors, rules, damage-2,
all five knobs, purse, text plates and volume placement from GAME-2B.

Author high/low guards and three thrust bodies on the same fine pixel grid as
Clasp. Keep the knight about 24x34 display pixels at the half-size cell view
(48x68 native room pixels), seat tabard and plume colours, and one warm shield
emblem for every seat. Use <=16 inks, connected clusters and coloured contours.
Lean the head/chest forward 2-3 pixels, extend the sword arm, and lower the
shield six pixels during each thrust. No scaled stock-body overlay.

Lift result plates clear of the top step by four pixels. Replace the shield's
plank-shaped tell with a two-pixel tinted contact lane. Clasp gets a dark visor
with two 2x2 warm lights, two pixels apart, protected at the actual cell grid.
Judge idle, high tell, flinch and defeat in the real terminal writer.

Show timing in milliseconds; align equal podium rows and seat numbers with
pips. Browser bouts keep the focused count and shared pips; cells keep the
shared pips and paired swatch/count row, without a third focused count.
Bound claimed sample ticks by the host's existing acknowledgement-delay
measurement if it can reach rules without extra messages on the wire. Test a
forged old sample, a miss rollback through payout, late and window-edge bots.
Review the native room, 3x pixels and real cells; measure with delve/smoke.

## Third polish: readable gap and honest results (GAME-M)

The contact lane must float clear of the shield at chest/floor height, while
retaining its exact high/low landing line. Leave five native pixels beyond the
shield's widest rim; keep Clasp, the floor, timing and contact anchors fixed.
End the podium's lighter header fill between rows. Prefix each row's name
with its seat number, matching the winner line; leave timing blank when that
seat landed no counters. Keep rank, counters, misses and payout unchanged.
In the cached cell art, preserve exactly two warm eye blocks in the defeat
pose and darken the metal immediately below the visor to stop false eyes.

Share one host allowance calculation across all three stamped-input games:
cap only measured RTT at 250ms, then retain snapshot interval and command
cushion. At 30Hz that is at most eight transit ticks. Deliberately held
acknowledgements must not widen it. Mossbell must enforce the same allowance
before replaying history. No other rules, knobs, attacks or art change.
Review matched whole fights at 1x, 3x details and the actual terminal podium
and defeat buffers, in both kits. Measure native and wasm smoke frame cost.
