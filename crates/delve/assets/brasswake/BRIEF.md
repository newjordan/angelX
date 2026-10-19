# Brasswake

## The feeling
A copper brewery comes alive around your fists: step into a cask porter,
land three weighty blows, then heave its barrel over the aisle divider.
In the first ten seconds the knight braces, punches a rounded porter back,
and sees a rival's incoming cask trace a clear landing mark.

## Presentation
The shallow vat wall gives way to a broad dark floor. The knight occupies
at least 28% of the stage height at its original pixel grain; the keeper is
roughly 1.6 knights tall, with a broad copper body and two curved teal lenses.
Four-tone material ramps, shaped lights and connected colour clusters unite
the characters and props. Damage two breaks a lens and opens a broad seam.
The title band holds the combo plate. Live standings use the podium's work,
falls and finish order, with the local seat separated below the ranked rows.
Overlapping porters leave a seat-colour contour around the local knight.
All stage positions and danger rectangles share the same camera translation;
the rules, anchors, timings, stake and judging remain unchanged.

## The loop
Walk the brewery aisle, clear three gates with punches and kicks, collect
bread and casks, then read the shared vat keeper's slam and steam rush;
most cleared work wins, with fewer falls and earlier completion breaking ties.

## Words on the title card
BRAWL THEN THROW

## Who plays
Every seated knight, at once, in an independent copy of the same seeded aisle.
A live name-and-score strip shows the whole table. One vat keeper appears
in every aisle; damage is personal so every knight fights the end boss.

## Controls
Move: walk in two dimensions. F: punch; every third contact is a kick.
E: grab a nearby cask, then throw into the next rival aisle; without a cask,
hold E to guard. Spells remain on 1 to 4 outside this room.

## The five numbers
1. brass_stride = 3 pixels/tick — closing distance without skating.
2. brass_combo = 12 ticks — the space between weighty combo blows.
3. brass_tell = 27 ticks — a clear wind-up to leave a marked danger area.
4. brass_shove = 12 pixels — the physical kick and cask contact.
5. brass_stop = 2 ticks — local contact freeze, never another seat's clock.

## Fail and retry
A hit flinches; an empty heart bar knocks the knight down for 18 ticks,
then stands them up in the same aisle. The chain breaks and the fall counts.
A released E then a fresh E restarts the podium in one tick.

## Version 1 is only
One 75-second stage, three two-porter waves, bread and casks, and one boss.
No campaign, jumps, extra weapons, enemy projectiles or extra enemy types.
Integer feet and camera positions; all seats use the same seed and wave deck.
Client sample ticks judge fresh blows against recent lane history using the
existing measured latency allowance. Paid results are final.

## Pays
10 gold per seated knight each round, exact ties split the purse; spare gold
is visible. The winner earns the existing crown and its delve bomb buff.
Best work score and completed-stage wins persist through the existing records.
The ledger stores work in units of five and the display restores full points.

## Ranked feel ledger
| Rank | Knight does | Room answers | Number |
|---|---|---|---|
| 1 | Punches near a porter | Body flinches and slides | brass_shove |
| 2 | Lands three blows | Whole-body kick and broad contact burst | brass_combo |
| 3 | Leaves a floor mark | Slam lands exactly inside it | brass_tell |
| 4 | Walks toward a gate | Camera follows the personal aisle | brass_stride |
| 5 | Throws a cask | Rival gets a marked landing, then a rolling body | brass_tell |
| 6 | Takes a hit | Brief flinch and broken chain | brass_stop |
| 7 | Guards a rush | Shield braces against a shaped steam front | brass_tell |
| 8 | Collects bread | Hearts refill with a warm bloom | fixed pickup |
| 9 | Falls | Body folds, then rises without a menu | 18 ticks |
| 10 | Finishes | Names, work, falls and finish seconds on a podium | 75 seconds |
