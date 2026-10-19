# Rafterwake

## The feeling
A boot leaves the plank a heartbeat late and still catches the rising lift.
First ten seconds: run, spring, release F for a short hop, then catch a ladder.

## The loop
Climb one seeded bellwright scaffold, dodge marked falling masonry, and jump into
the hoist keeper's exposed head between its chain-drop and floor-sweep tells.

## Words on the title card
CLIMB THE LOFT

## Who plays
Every seated knight, at once, in an independent copy of the same seeded tower.

## Controls
ARROWS RUN / F JUMP / UP CLIMB / E AGAIN

## The five numbers
30 Hz, fixed point at 256 units per native pixel; F2 server knobs:
1. rafter_gravity = 112 units/tick²: weight after the apex.
2. rafter_jump = 1536 units/tick: a full hop clears one 36-pixel deck.
3. rafter_run = 640 units/tick: enough air steering to catch a moving lift.
4. rafter_coyote = 4 ticks: the last step still belongs to the plank.
5. rafter_buffer = 5 ticks: an early press springs on landing.
Ladder pace is 384 units/tick. Releasing F halves upward speed once.

## Feel ledger
| Rank | Knight | Response | Number |
|---|---|---|---|
| 1 | Late jump | Still springs | coyote |
| 2 | Release jump | Low arc | jump |
| 3 | Hold sideways | Catches lift | run |
| 4 | Press before landing | Immediate spring | buffer |
| 5 | Leave ladder | Weight returns | gravity |
| 6 | Land | Crouch and solid dust | 4 ticks |
| 7 | Miss plank | Return to last deck | 18 ticks |
| 8 | Climb | Alternate hand reach | ladder pace |
| 9 | Strike keeper | Freeze, crack, burst | 2 ticks |
| 10 | Win | Raised plume, crown | one tick rematch |

## Fail and retry
Falling or a marked impact gives a short crumple and returns to the last safe deck
in 18 ticks. E rematches in one tick after the podium.

## Version 1 is only
One 75-second scaffold, twelve decks, moving lifts, ladders, sparse falling bricks,
and one original hoist creature. No campaign, spells, bullets or second room.
The bellwright's loft occupies the existing Workshop building.

## Pays
10 gold per seated knight to the winner(s), split exact ties. Height plus keeper
hits ranks first, fewer falls second, earlier finish third. The existing crown
and its delve buff carry down. The host saves the best progress.

## Tuning record
Five bot rounds per proposal, at 1/2/4/2/4 seats. The 144/1536/512/2/3
proposal stalled at 66px because its arc did not clear the next deck. The
96/1664/768/5/6 proposal had excess headroom over a 36px deck. The selected
112/1536/640/4/5 proposal completes the same seeded scaffold and keeper in
725 ticks for the scripted player, with two recoverable falls. A human has
the full 75-second round. The smaller arc keeps the next plank relevant.
The client-sample ladder catch uses the seen rung and height, including when
the body has moved away before the press arrives; the expired edge is rejected.

## Keeper readability
Broad leather and slate material masses keep solid native 4x4 blocks. Named
LINEAR recipe strokes (1–3px wide, >=5px long) preserve the swept teal crest
ribs, curved hoops, chain links, hook, pulley rim and jagged damage cracks.
Saved underlays let the shared gate inspect the original mass beneath a stroke;
untagged patches and flecks still fail. The keeper uses at most ten local inks.
A flush rounded faceplate carries a curved 3x2 amber slit (#e0962c), protected
in the actual half-block sampler. Cached narrow props use the braille layer
in cells; the knight and viewport mask that layer. Cracks have dark backing and an amber core;
second damage branches, springs a hoop and dents the helm. Damage variants bake
from the same recipe into OnceLock rather than adding runtime stripes.
Drop raises the hoist weight; sweep pulls the hook behind the body through four
stages. Both tells retain 33/29/25 ticks. Exact-area marks grow from dim 1-on/
4-off to 2-on/3-off, brighten, then 3-on/1-off with a single warm floor row.
Cool grey-ochre lobed dust stays behind figures and below 17px. All knight poses,
props, anchors, contacts and gameplay numbers stay unchanged.

Rebuild after the concept converter with
`python scripts/rafterwake-clean.py REFERENCE.png REVIEW_DIRECTORY`.
`python scripts/rafterwake-clean.py --check` runs the shared material, LINEAR,
signature, palette and singleton gates, with explicit patch negative controls.
