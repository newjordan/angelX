# Cinder Stack

Inside the Alchemy Lab, approach Crock's kiln at the southeast corner and
press E. The whole seated party joins. Each knight sees their own board.
A/D slide a clay pair, F turns it, E drops it. Match three pots of the same
colour in a row or column. Impressed marks also distinguish the colours.

Matches crack Crock; cascades multiply the cracks. A match beside slag
breaks it. The amber column boundary and landing square show the exact
next slag drop. The spirit has three phases, with one drop at a time.
Overflow coughs out the board and returns the pair in 15 ticks (0.5 s),
keeping earned cracks. The first knight to 36 cracks wins; at 60 seconds
all boards are ranked by cracks, then fewer coughs, then seat order.

A clear crowns the winner. The realm's first clear pays that knight 25
carried gold; replays and later outings do not pay again. The host keeps
the best and win record with the other games. E on the result board starts
a fresh heat in one tick. Esc uses the normal room exit.

The feel brief is the header of `crates/delve/src/rules/cinder.rs`.
The empty-room verb was exercised with independent seat inputs and a
scripted clear; the four-seat room adds the rival and crown around it.

Live dev-host trial, browser plus three native bots, 8951 / CDP 9051:

| Knob | First proposal | Promoted | Reason |
|---|---:|---:|---|
| cinder_fall | 24 | 20 ticks/cell | Less waiting when learning without hard drop. |
| cinder_slide | 6 | 5 ticks/repeat | A held slide crosses the board promptly. |
| cinder_lock | 12 | 10 ticks | Landings settle without a long pause. |
| cinder_tell | 30 | 27 ticks | Still a clear 0.9-second danger cue. |
| cinder_attack | 180 | 165 ticks rest | Rival acts before a clean short solve ends. |

The live sliders and host tune JSON confirmed all five values. Bots scored
31 / 36 / 31 against the browser's 3; no bot overflowed. The browser retried
once. Rematch was visible after 0.3 seconds. Bot policy is a deterministic
landing search, so this proves rules and seat transport, not human mastery.

Art sources and exact prompts are recorded in `crates/delve/assets/cinder/ART.md`.
The converter keeps nearest sampling and sixteen existing realm colours.
The first projectile crop, header-crossing projectile, and leftover podium
sparks were rejected. The final pass keeps the drop inside the board,
clears effects at the finish, and makes blue pots as readable as the others.

Dev review: `delve-dev-host --scene cinder --bots 3 --dev` with a browser
as the fourth seat. Bots wait for the full table, play their own published
boards, and print the host's podium. The smoke crate includes the same room.
