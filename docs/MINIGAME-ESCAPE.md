# Leaving a mini-game

Hold **Esc**, or the gamepad's **Menu/Start** button, for half a second.
The HUD says `Esc: leave`; its bar fills while the host receives the hold.
Releasing before 15 consecutive 30 Hz steps resets the bar. A tap does nothing.
The same action works during countdowns, play and result boards.

Inside mini-games this takes priority over the former Esc pause menu, the
ballot and game controls. Outside mini-games the existing room menu remains.
On a shared cockpit keyboard, Esc belongs to knight 1 and Delete to knight 2.
A joined cockpit's Esc always belongs to its own seat.
Pad Start now sends Menu; Back keeps vigil (Y still interacts).

The host performs the retreat before the game tick. Each seat returns to its
recorded entrance room and doorway independently. Friends keep playing; the
last departure clears the game and returns the party. A departed seat is not
a target, racer, scorer or payout candidate. Already-paid stakes remain in the
existing pot. The lists keep their existing forfeit verdict and wager rules:
a rider's forfeit settles that contested bout; friends stay in the lists and
can ride again. A gallery departure leaves the ongoing bout untouched.

## Exit audit before this change

| Game/stage | Existing rules exit | Browser / cockpit problem |
| --- | --- | --- |
| Brasswake | Finish or retreat from Tavern | Full-screen controls capture movement; Esc menu / Enter exit was undisclosed in its HUD. |
| Rafterwake | Finish or retreat from Workshop | Same hidden menu; no stage exit control. |
| Pennon Rush | Finish or retreat from Muster | Same hidden menu; no stage exit control. |
| Mossbell | Finish or retreat from Bathhouse | Same hidden menu; no stage exit control. |
| Cinder Stack | Finish or retreat from College Lab | Same hidden menu; no stage exit control. |
| Latch Duel | Finish or retreat from Smithy | Same hidden menu; no stage exit control. |
| Hippodrome horse/chariot race | Timed result returns to coach; room retreat | Captured reins; hidden Esc menu. A starter's retreat reassigned racer 0 to a friend already driving another team. |
| Lists / joust | Result dismounts; room retreat forfeits | Hidden Esc menu. A rider's departure resolves the bout under existing rules. |
| Shaft Run, Ore Cart, Griffin Run / Eyrie practice | Countdown, race, timed podium | Captured movement; ordinary Hall stages were not eligible for room retreat. Departed race bodies could keep racing/scoring. |
| Wax Maze, Belfry Shoot | Round completion opens hall | Hall was not eligible for room retreat; no universal early exit. |
| Keg Gantry | Round completion opens hall | Hidden room menu; stale score could compete after departure. |
| Powder Room, Book Hop | Round completion opens hall | Hall was not eligible for room retreat; no universal early exit. |
| Kitchen, Reading, Bellows, Ossuary, Still, Loft nooks | Door / room retreat | Exit only through room controls, no common in-game HUD hint. |
| Privy stages | Finish dressing / personal retreat | Captured controls and hidden menu. Hold now leaves only that knight's stall. |

All 16 surface pastimes used the room door or the hidden Esc menu followed by
Enter/Backspace (cockpit also offered Backspace/Delete). The stage controls and
result UI did not explain this consistently:

| Pastime | Entrance room |
| --- | --- |
| Broadsheet Boy | Village |
| Barrel-board Derby, Turnip Keepy-uppy, Carnival Duck Range | Fairground |
| Washtub Regatta | Riverbank |
| Cheese Chuck | Downs |
| Hobbyhorse Hurdles | Causeway |
| Squire Sled Slalom | Crag |
| Chamber-pot Put | Strand |
| Wheelbarrow Cross | Timberyard |
| Shield Surf | Millpond |
| Belfry Bat-and-ball | Belfry |
| Jester's Half-pipe | Mummers |
| Cabbage and Roll Racing | Hippodrome |
| Trencher Toss | Goose Green |
| Popinjay Shoot | Longbow Green |

## Implementation and checks

`Input.leave_held` is held command bit 15, preserving earlier flag/optional
field bits. Press-only taps never become a hold. `exits::State.holding` is
serialized host state and omitted when empty. Counting is once per host transport step, including impact holds,
never once per render, turbo substep or local animation. The six newer games
remove departed lanes through their existing lifecycle. Older follies remove
score entries while retaining antes; races release departed teams/bodies.
No movement, damage, game timing, palette or payout formula changes.

The four-seat rules suite covers every row above (three bots plus each possible
leaver), release/reset, entrance return, continuing ticks, forfeited payouts,
last-seat cleanup and byte-identical no-leave traces. Client tests exercise the
actual command codec and held Menu path; cockpit tests exercise local and joined
seats. The ignored `write_escape_hold_review` test captures four-seat Brasswake
and Mossbell hints/fills through native pixels and the production cell painter.
