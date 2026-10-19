# Mossbell

## The feeling
A heavy brass marble drops toward the gap; your knight snaps both wooden paddles
and sends it singing into a stubborn seed-pod keeper before ten seconds pass.

## The loop
Launch, catch the falling ball with left and right flippers, light three different
bumpers and crack the keeper for the common jackpot, then compare scores.

## Words on the title card
KEEP IT ROLLING

## Who plays
Every seated knight at once on a personal table, with identical round seed and
a small live score strip. A jackpot belongs to the first qualifying sample tick;
exact same-tick claims share it. The round continues for 60 seconds.

## Controls
Left and right swing their corresponding flipper; F swings both and launches;
up nudges once per press. E opens the table and restarts the podium in one tick.

## The five numbers
All physics is integer fixed point (256 units per pixel), four collision substeps
per 30 Hz tick. These are live F2 knobs, then promoted after bot play.
1. moss_flipper = 1250 units/tick — lift and satisfying contact.
2. moss_gravity = 28 units/tick squared — a readable heavy descent.
3. moss_bounce = 85 percent — a firm rebound without endless floating.
4. moss_nudge = 180 units/tick — a small once-per-press rescue.
5. moss_launch = 1450 units/tick — reaches the crown without skipping collisions.

## Fail and retry
A drain breaks the chain, shows a seed-shell burst and relaunches in 12 ticks.
No lives menu. A released E rematches the podium in one tick.

## Version 1 is only
One enchanted table in the bathhouse conservatory, one marble per seat, three shaped bumpers,
two flippers, one keeper with root-sweep and crown-clamp tells of 30 ticks or more.
No multiball, scrolling camera or bullet patterns.

## Pays
10 gold per seated knight, paid once each round to the score winner(s).
Ties use fewer drains, then earlier jackpot in seconds; exact ties split the purse
with spare gold visible by seat. The existing crown and carried bomb buff apply.

## Feel ledger
| Rank | Knight does | Table replies | Number |
|---|---|---|---|
| 1 | catches the fall | marble thumps up the lane | moss_flipper |
| 2 | reads its weight | descent accelerates visibly | moss_gravity |
| 3 | launches | a curved upper-bank rebound | moss_launch |
| 4 | banks a bumper | a rounded ring blooms | moss_bounce |
| 5 | nudges | a small sideways rescue | moss_nudge |
| 6 | chains targets | a large CHAIN pop | three unique bumpers |
| 7 | hits keeper | shell flinch and dark seams | six hits |
| 8 | reads root tell | exact horizontal sweep band | 36 ticks |
| 9 | reads crown tell | exact upper target gate | 30 ticks |
| 10 | takes jackpot | petals, shake, shared race strip | first sample tick |

## Send-back art pass
Keep the rules, sample-tick fairness and all five tuned numbers above. The Keeper
fills 40–44 of its 48 pixels with a cool leaf hood, warm shell, a readable curved
visor and broad dark-around-light cracks. Nine poses must differ in real cells.
The crown warning uses bowed leaf jaws and an advancing downward fan across the
existing upper gate; the root sweep keeps its exact horizontal danger band.
Widen the displayed field with an integer horizontal coordinate map; collision
coordinates stay the same. Keep sprites at one-pixel grain. A 20×24 knight works
the paddles with clear feet. Shape-specific bumper flashes retain their bodies.
Chain plates live above the Keeper. Race names, gutters, fixed row pitch and
plain controls must survive both the bitmap font and the real terminal labels.

## Themed tell polish
Root tendrils advance through four distinct silhouettes along the exact
horizontal band, with an amber leading bud and airy branches. Crown jaws
spread down and out through four silhouettes inside the original landing gate.
Both use nearest-sampled finished pixel art with cool teal shadows and amber
heads. Keep all timings, hit regions and tuned integers. Broaden damage-1's
fractures for cells, quiet the hood's rim, and separate the player's helmet
and feet from the cabinet. Quiet dormant roots ground the empty lower field.
The goal reads BUMPERS, THEN KEEPER; podium verbs and numeric columns agree
between the browser and terminal, using seat numbers for default rivals.

## Landing-gate clarity
Crown hooks originate at x60 and x100, sweep inward and down over y20..43,
and close through four clearly different silhouettes. Dim floor tint sits
behind the Keeper from stage 0; a cool jade rim separates the jaws from its
amber shell. Require at least 15 advancing real cells at each crown step,
including rage. Root stems use two teal core tones, a one-pixel coloured
outline and an amber head. Centre drop targets sit over the root; its right
edge stops at the plunger wall. Keep all geometry, timing and other art.
