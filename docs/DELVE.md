# The Delve

A co-op dungeon under the angelX realm: knights against monsters, for spoils
that build the world. The world, the cast and the Grail are in
[DELVE-LORE.md](DELVE-LORE.md). Card files are covered in
[cards/README.md](../cockpit/assets/dungeon/cards/README.md).

## Getting in

1. `/dungeon` sends your knight across the mini-viz map to the Delve's gate:
   a stair into a slate rise near the Mines. Its torches burn while it is
   called or a run is on.
2. When he arrives, the cockpit says so. Click the mini-viz, or press **F4**,
   to open the Delve's intro.
3. On the intro, choose:
   - **a knight of the company**, each with their own weapon, guard, kit and
     colours:
     - Sir Percival, the Seeker: bow, roll, a potion;
     - Dame Lynette, the Shieldmaiden: crossbow, kite shield;
     - Sir Gareth, the Kitchen Knight: handgonne, extra bombs;
     - Sir Galahad, the Pure: blade, more health.

     These are the Keep's own household. While a model house serves, the
     knight who wears each kit is one of that house's knights, named by its
     model ("Ardent of Lanternmere, the Seeker"); see
     [CASTLES.md](CASTLES.md).
   - **where to begin**: the Crypt or the Mines;
   - **who comes along**: alone, a friend on this keyboard, or friends on
     their own angelX (see *Playing with friends*);
   - **voices**: the knights' grunts and their few words at big moments, on
     or off. Nobody narrates; **K** toggles them in play (**Shift+K** with a couch knight).

   Enter begins the run outside the Delve's gate on the continuous kingdom map,
   and the game expands. Press **E** at the gate to enter its courtyard.

`/dungeon start` skips the walk and the intro.

The host plays **inside the expanded cockpit mini-viz**. No browser is needed.

- **F4** or clicking the mini-viz expands a run; **F6** is an alias.
- In minigames and special rooms, **Esc → Leave** exits at any time, including
  during combat. **Backspace / LEAVE** exits directly; couch knight 2 uses **Delete** (or
  selects knight 2 in the pause menu). Each knight returns safely to the entry;
  friends still inside play on; **E** at the entry rejoins them. The last departure
  cancels the activity, and re-entry starts clean. Regroup before taking a shared
  stair to the next floor. Ordinary Delve fights keep their barred doors.
- Elsewhere **Esc** returns to the harness composer, preserving its draft and cursor.
  **F2** also returns to the composer from the activity menu.
- While coding, the mini-viz shows the current room. Solo play pauses.
- While a friend is seated, returning to the composer turns the host into an invulnerable grey
  statue and the friends play on. On the kingdom map, returning keeps the
  host at their own position. Interior visits retain the existing party arrival.
- The expanded view shows harness working/queued/done/approval status.
- Private workspace checkpoints under `.angel/dungeon/session.json` preserve the
  run, checked gear, and invite across orderly exits/rebuilds. Checkpoints are
  also written every three seconds. `/dungeon off` removes the checkpoint.

## Controls

| Action | First knight / joined friend | Couch second knight |
|---|---|---|
| Move | WASD | IJKL |
| Aim and fire | Arrows; F fires toward aim | Enter fires toward movement |
| Interact with plates, folk, levers, cages and booths | E | H (also takes weapons) |
| Guard / roll | Space | O |
| Sword | Q | [ |
| Bomb | X | U |
| Take weapon | P | H |
| Cast spells | 1 2 3 4 | 5 6 7 and backslash (`\`) |
| Quaff the first card in hand | Z | ] |
| Ultimate | R | Y |
| Clear held keys | Backtick (`` ` ``) | Backtick (`` ` ``) |

Quaff plays the first potion or other hand card; the hand shifts left, so
repeated taps work through it. Spells occupy four separate slots and recharge.
Empty spell slots stay hidden; until one is learned the HUD says “spells: none
yet”. The built-in spells have drop weights 10 (Cabbage Storm), 9 (Hedge Maze)
and 8 (The Imposture), so ordinary delves find them more often.

On the walking kingdom, **E** opens a nearby door and **F** fires.
Click the overview, press **P** with it focused, or use `/world walk` to
walk the same continuous map. **Esc** returns to the overview and composer.
A held door key needs a fresh press after returning. The Delve's menu keeps
invitation links behind the explicit hosting choice. **E** reads a LOCAL RESEARCH exhibit only beside its
marker; elsewhere it interacts with the game.

The browser uses the first knight's keys. On a gamepad, **Y** interacts,
**Start** keeps vigil, **R3** quaffs, and **LT + d-pad up / left / right / down**
casts spells 1 / 2 / 3 / 4. **F** remains the action key during running sports
and mini-games. Hold progress uses filled and empty block bars.

## The world above the Delve

Every delve begins outside **the Delve's gate** on one continuous kingdom:
roads, rivers, fields, forest, model castles, the College and built wishes
share the geography of the mini-viz overview. Each seat has its own knight
and smoothly following camera; up to four knights can spread across the
whole bounded map without screen changes. See [KINGDOM.md](KINGDOM.md) for
the architecture, bounds and verification.

Press **E** at a door to visit its separate map. The gate courtyard contains
the stair down to the Undercroft; countryside pockets keep their games, folk
and building plots. Walking out of a pocket returns to the kingdom at the
entry door. The College, ranch and flower fields also keep their marked
exit stairs. Visits still take the party together; leaving restores each
knight's own surface position. The King's Hall remains behind the rebuilt
great door in the mine-head, and the College tower remains inside its grounds.

On the kingdom map the countryside doors stand together in named regions,
each lettered in grey in open ground: **The Woods** (north-west, by the Dark
Forest: hedge maze, bothy, timberyard, orchard, Old Watch), **The High
Roads** (north-east hills past the Dragon Keep: crag, Pigeon Parliament,
belfry, aqueduct, priory), **The Coast** (the north-east shore: beacon,
cliffs, saltworks, strand), **The Wetland** (the western marsh by the
Swamp: millpond, sluice, Sunken Abbey, Washbucket Quay, causeway), **The
Fair** (between the Village and the Artisans: Fool's Green, Mummers'
Hollow, the Royal Spin Cycle), **Wind Country** (the downs south of the
Homecoming fields: Picnic Green, windmill, Flower Fields, Wheyward Downs,
kitewright, Loud Flags) and **The Tilt-Yards** (between the Colosseum and the
Tournament: hippodrome, Horse Ranch, toll bridge, caravan, wheelwright).
Castle Town, The Mountain (the Mines, the Delve's gate and the College),
The Wastes (the Dragon Keep) and The March (the model castles) are lettered
too.

The table below describes the retained visit maps and their original
connections. Find their doors on the kingdom; ordinary pocket edges now
lead back to the kingdom. Built coach, ferry and other travel plates retain
their direct journeys.

| Room | Where | What is there |
|---|---|---|
| The Delve's gate | the courtyard | the gate's stair **down to the Undercroft**; a well; a signpost; whoever is waiting about |
| College of the Labyrinth | College Gate, east side of the courtyard | free school rooms and a four-floor tower; Bartosz the Wizard teaches the Labyrinth and offers shared research quests ([guide](SCHOOL.md)) |
| The stables | west | the realm's three mounts, Wat the groom (see below) |
| The lists | east | the tilt, the stands and their crowd, and the joust (see below) |
| The mine-head | north | the Mines' adit, its shaft **straight down into the Mines**; the fallen gate-hall of Caer Dwfn |
| The King's Hall | through its great door at the mine-head, once rebuilt | King Brannoc's seat, his ledger, the war table of the forges, the Great Forge |
| Firefly Grove | south of the gate | Firefly Hunt and Woodland Memory, shared forest practice games |
| Broadsheet Row | south of the stables, west of the grove | cottages, subscriber mailboxes, and a goose with editorial opinions |
| Fool's Green | south of the lists, east of the grove | an ordered barrel course, turnip keep-up, and a toll-demanding jester |
| Wonky Windmill | south of Broadsheet Row | a lost cog, wheat fields, and a Goose-Proof Bakery plot |
| The Old Watch | south of the grove, between the mill and quay | a lost bell, a Wayfarer's Beacon plot, and the first road lantern |
| Washbucket Quay | south of Fool's Green | a moving wake-ring course, the royal duck, and the Royal Left Sock |
| Wayfarer Meadow | south of the Old Watch | The Bent Spoon Inn plot and, once built, a two-way coach to the gate |
| Apple Apology Orchard | south of the windmill, west of the meadow | apple trees, an unhelpful scarecrow, and the second road lantern |
| Sir Damp's Marsh | west of the orchard | reed pools, the final road lantern, and a buildable ferry office |
| Wheyward Downs | west of the windmill | a Cheese Chuck tee, basket-bearing goats, and a survey stake |
| Gullible Cliffs | north of the Downs, west of Broadsheet Row | a railed sea overlook, the last survey stake, and the Counterweight Goat Lift plot |
| Toll Troll Causeway | east of Fool's Green | a dry sporting bridge, Hobbyhorse Hurdles, and a troll taxing sandwiches instead of travellers |
| Puddingstone Priory | south of the causeway, east of Washbucket Quay | stone cloisters, the abbot's lost hamper, and an Overqualified Pigeon Post plot |
| Candlewick Crag | west of the stables, north of the cliffs | snowy ridge, a brass weather rooster, and Squire Sled Slalom |
| Rookery Pass | north of the stables, west of the mine-head | the Royal Bird Office, paperwork-minded pigeons, and a forecast clerk |
| Cuckoo Observatory | north of the crag, west of the rookery | an improbable telescope, three constellation lenses, and a ropeway plot |
| Soggy Bottom Strand | east of the Causeway | shell-strewn sand, tidal parcels, and Chamber-Pot Put |
| Saltspite Works | north of the Strand | salt pans, unionized crabs, and a harbour plot |
| The Bickering Beacon | west of the saltworks, north of the Causeway | a lighthouse with a missing oil delivery and a keeper who blames the moon |
| Splinter's Timberyard | west of Gullible Cliffs | a working log yard, wheelbarrow BMX course and a forester who measures dignity by the plank |
| The Hedge of Grievances | south of the timberyard, west of Wheyward Downs | a walkable hedge maze and three legally dubious mushrooms |
| Badger's Bothy | south of the hedge, west of Squabblefen Marsh | a badger soup cook and a Board-and-Breakfast wagon plot |
| The Royal Dog-Paddle | west of Splinter's Timberyard | a millpond, a dry inspection pier and shield surfing under a monk's disapproving eye |
| The Sluice of Excuses | south of the millpond, west of the hedge | walkable valve gardens; the king's bath has flooded an abbey |
| The Sunken Abbey | south of the sluice, west of Badger's Bothy | flooded cloisters, a missing lunch bell and a permanent punt-house plot |
| The Bashful Belfry | east of Soggy Bottom Strand | shield-paddle Bat-and-Ball, four tax bells, and the lost Bell Tax minutes |
| The Unnecessary Aqueduct | south of the belfry, east of Parliament | stone arches, walkable inspections, and a Water Tax on water that never arrived |
| Pigeon Parliament | east of Puddingstone Priory, south of the Strand | the Speaker, lost Seed Tax minutes, and a buildable Air-Mail Monstrosity |
| Mummers' Hollow | north of the mine-head | a walkable fair stage, The Jester's Half-Pipe and barrel-board acrobatics |
| The Royal Spin Cycle | east of the Hollow | four oversized wash vats, a cross-shaped inspection road and the king's embarrassing laundry |
| The Caravan of Questionable Comfort | south of the laundry, east of the mine-head | tent lanes, lost property and The Wheely Inn wagon plot |
| The Rusty Hippodrome | east of the caravan, north of the lighthouse | free Cabbage and Roll chariot practice, a finite spectator race card, and a buildable coach |
| The Wonky Wheelwright | north of the Hippodrome, east of the laundry | square wheels, a mechanic with opinions, and a goat-proof axle commission |
| The Compliment Toll Bridge | east of the Hippodrome, north of the saltworks | a free walkable bridge, suspicious paperwork and scenery with no legal authority |
| Sir Honks-a-Lot Picnic Green | west of the millpond | a flying-trencher event, goose referee and missing picnic sail |
| The Knotty Kitewright | north of Goose Green | questionable kites, a signal commission and a buildable kite-post |
| The Hill of Extremely Loud Flags | east of the kitewright, north of the millpond | three separated signal levers and a sandwich-minded lookout |


A stair or a door you stand on for a moment takes you through it, as the
Winding Stair does: a ring fills round it, and its board says where it goes.
The Undercroft has a stair of its own back up to the gate, in its north wall
beside the Training Yard's door. `/dungeon settlement` still opens in the
PLAYER HALL below.

### A first ramble around the kingdom

Ordinary walking is free. The plots below buy **optional two-way shortcuts**,
not permission to explore. From the Undercroft's stair, try these little tours;
reverse any road to return, or follow the named neighbouring-door board:

- **A very bad paper round:** Gate → Forest → Village → Windmill. Try the
  Broadsheet Boy—our medieval *Paperboy* nod—then return through the forest to
  the Fairground for the barrel and turnip courses. The orchard and old watch
  offer quieter errands and building plots.
- **Eight-bit seaside holidays:** Fairground → Causeway → Strand → Saltworks
  → Lighthouse. There are pots to put, tubs to paddle back at Washbucket Quay
  (the Riverbank), and
  shields to surf at the millpond: the kingdom's decidedly unsafe
  *California Games*-style holidays.
- **The cabbage circuit:** Mine-head → Caravan → Hippodrome → Wheelwright.
  Return through the Hippodrome to Toll Bridge → Saltworks. Race three rivals
  yourself, or visit the ticket booth for the separately paid spectator heats.
  The Compliment Toll Bridge is free; its paperwork only *sounds* expensive.
  The workshop commission and coach construction are separate transactions,
  not road tolls.
- **A picnic with air traffic control:** Windmill → Downs → Cliffs → Timberyard
  → Millpond → Goose Green → Kitewright → Signal Hill → Millpond. Toss plates
  for Sir Honks-a-Lot, take the kite commission, and send the lookout FISH /
  BOOT / CROWN. The kite-post is a purchasable shortcut; that entire loop can
  be walked first.

There are also woodland, snowy, abbey, marsh and rookery roads to wander. The
King's Hall is the separate constructed great-door destination, not a free
ordinary-road stop. No sport starts until a knight stands on its start plate;
you can travel through the kingdom without entering a contest or placing a bet.

## Surface rambles and the kingdom's very serious games

The original countryside circuits are now groups of enterable places on
the continuous kingdom. Visit the village, grove, fair, windmill, old watch,
quay, meadow, orchard, marsh, downs, cliffs, causeway, priory and coast
through their surface doors. Their dry approaches are free before any
quest or construction. The tide gates a parcel pickup, not exploration.
The built ferry and goat lift remain direct shortcuts between visits.

The **woodland places** include the Timberyard, Hedge of Grievances and
Badger's Bothy. The hedge walls retain real walkable collision. At the
bothy, press **E** beside the cook, collect **BOOT → HAT → BEARD** mushrooms
at their hedge rings, then return to the cook. The soup is now legally a
wizard. This pays **70 gold + 2 wax once** and unlocks the
**Badger's Board-and-Breakfast**: **95 gold + 3 bone + 2 wax**, then a two-way
wagon between Bothy and Orchard of Minor Disputes. New quests and visit bits
never revoke the original six-place ramble or inn eligibility.

The **flooded abbey places** include Royal Dog-Paddle, Sluice of Excuses
and Sunken Abbey. Their inspection pier and dry causeways remain walkable.
Talk with **E** to **Brother Bucket** at the millpond, turn the sluice's **FISH → BOOT → DUCK** valves in that order,
collect the abbey bell, and return it to the monk. He offers **65 gold + 2 ore
once**, and permission to build **The Abbey Punt House** for **105 gold + 4 bone
+ 2 ore**. Hold E at its gold building plate to pay; it opens a two-way boat
between the Sunken Abbey and Apple Apology Orchard. No earlier visit reward,
inn requirement, building or game record is revoked.

The **high-road places** include Bashful Belfry, Unnecessary Aqueduct
and Pigeon Parliament. Every surface door is free to approach.
Press **E** beside Parliament's **Speaker** to accept the lost-minutes errand,
then find the **Bell Tax**, **Water Tax**, and **Seed Tax** papers in **any
order**. Return all three to the Speaker for **75 gold + 2 wax once**. This
unlocks **The Air-Mail Monstrosity**: **100 gold + 4 bone + 2 wax**, paid by
holding E on the gold plate, opening a two-way mail cart between **Parliament
and Rookery Pass**. The pigeons call it air mail; the road does the work.
Building is a separate purchase, not an automatic deduction from the reward.

The **mummers' places** are Mummers' Hollow, Royal Spin Cycle and
Caravan of Questionable Comfort. All three are free to explore.
The laundry's four wash vats have inspection aisles; the caravan has tent lanes, lost property and a construction plate. Land
four tricks at the Hollow's **J** ring for **80 gold + 2 wax once**. Explore
all three places as well to unlock **The Wheely Inn**: **110 gold + 3 bone
+ 2 wax**. Hold E at its construction plate to buy it; the reward does not
automatically spend itself. The wagon opens two-way travel between the
caravan camp and **Fool's Green**. Previously saved exploration can count
before winning the medal; the original six-place ramble and inn requirements
remain unchanged. No fetch quest or admission fee is required.

The **racing places** are The Rusty Hippodrome, The Wonky Wheelwright
and The Compliment Toll Bridge. Enter their doors from the kingdom.

**Cabbage and Roll Racing** is a medieval *Rock n' Roll Racing*-style event,
not a delivery or timing-pad game. Step onto **C** in the Hippodrome. The
first knight on the ring drives a chariot; their knight stays safely on foot.
**Up arrow** whips the horse, **down arrow** brakes, **left/right arrows**
steer between three lanes, and **tap and release F** throws one of your three
cabbages at the nearest rival just ahead in the same lane. WASD is the
movement-control fallback; arrows let you race without walking off the ring.
Complete **three laps within 35 seconds**, ahead of **all three** rivals:
Sir Rattle, Goat Meatloaf and Abbot Overdrive. Barrel obstacles slow your
axle, so steer clear rather than merely holding the whip. Practice is free
and saves bests, plays and wins, not repeatable gold. Step off C and back on
to retry. Leaving, retiring or becoming a statue cancels that attempt.

**Spectator betting:** at the arena's **B** booth, choose **left / up / right**
for Sir Rattle / Goat Meatloaf / Abbot Overdrive, then **tap E** to buy the
shared party's ticket and watch a **10-second heat**. There are **three heats**,
with deterministic, varied lead changes. A ticket costs **10 gold**; a winner
returns **25 gold total** (15 net), while a loser loses the stake. Each heat
settles only once, through the ordinary atomic realm save. A paid unfinished
ticket keeps its selection when you leave, restart or rejoin: return to B and
tap E to resume without buying another. An in-progress checkpoint also keeps
its animation. Saving must succeed before the next heat opens; a failed save
keeps its pending intents for retry, rather than paying twice. When all three
heats finish, the card is closed. No real-money currency, paywall or admission
fee is involved; foot travel and chariot practice remain free.

**The Suspiciously Fast Coach:** press **E** beside the wheelwright's **W**
mechanic to accept the commission, collect the **square wheel** at the arena,
**goat-proof axle** at the wheelwright, and **toll receipt** at the bridge in
any order, then return to W and press E. This pays **85 gold + 2 ore once**
and unlocks the separate coach construction plate at the Hippodrome. Hold E
there to buy it for **120 gold + 4 bone + 2 ore**. It opens two-way
**Hippodrome ↔ Stables** travel. The reward does not buy the coach for you;
walk onto the travel plate and wait for its ring to fill. Old saves acquire
the new roads without revoking the original ramble, inn or festival records.

The festival is a medieval nod to *Paperboy* and *California Games*: petty
courier disasters, barrel racing, turnip keep-up, royal washtub sport, and a
cheese-wheel answer to flying-disc contests, hobbyhorse hurdle athletics, and
downhill shield-sled slalom, a wind-judged chamber-pot athletics contest,
wheelbarrow BMX in the timberyard, shield surfing in the millpond, and
a shield-paddle answer to old cartridge bat-and-ball arcades in the belfry,
and a barrel-board half-pipe with a jester very much attached to his helmet —
and, when a dimensional round is live, the whole room view hands itself to
the attraction: a fixed-camera 3D stage for the duck range's booth and the
half-pipe's channel, painted in the realm's own palette.
Walk into the gold lettered start ring; friends share one timed round and its
score. The boards show the objective, remaining seconds, best score and wins.

**Humphrey the Risen** haunts the festival once it is properly underway — three
rounds played and a first victory won. Stand about too long (eight seconds of
the whole party doing nothing, never during a live round) and he claws up
through the compost at the far side of the room and comes lurching: a slow
shamble, a rising wind-up, then a two-handed shove that sends a knight
tumbling. He is a real foe — swings and shots fell him, paying a bounty and
leaving a body the hand drags under — but he reassembles after about
forty-five quiet seconds, each return a little larger. Leaving a room always
buys the party a clean start; he is never more than a room behind, and he
never follows into the Delve. The Herald keeps his page in the Bestiary.

| Game | How to play | Kingdom complication |
|---|---|---|
| Broadsheet delivery (P, village) | Within 35 seconds, **tap F** near each of the three gold-flag subscriber mailboxes. Their flags rotate between rounds. | Non-subscribers throw the sheet back; the goose eats your editorial. Each wrong delivery is attempted only once. |
| Barrel derby (B, fair) | Walk or roll through flags **1–6 in order** within 25 seconds. The next flag glows gold. | The jester charges a toll in dignity. Out-of-order flags give no progress. |
| Turnip keep-up (T, fair) | Stay by the turnip's ring and **tap F as it lands gold**, once per beat, for eight kicks within 20 seconds. | Holding F does not juggle; mistimed taps lower the score. |
| Washtub wake (W, quay) | Walk the six moving wake rings **in order** within 30 seconds. The next ring glows gold. | The royal duck disapproves of your navigation. Keep to the bank and bridge: the northern water is hazardous. |
| Cheese Chuck (C, Downs) | Start at the C ring. From its tee, use **arrows to aim** and **tap F** to throw a cheese wheel. Lead the moving goat basket: five catches, twelve wheels, forty seconds. You can shift around the tee to line up a throw. | Basket-bearing goats pace up and down. Walking into the basket cannot score. Only one wheel flies at a time; release F between throws. Twelve misses end the round with a refund demand. |
| Hobbyhorse Hurdles (H, Causeway) | Start at the H ring, then **walk east along the broad central lane**. **Tap F just before crossing** each of the six numbered hurdles in order, within 35 seconds. Green hurdles are cleared; gold is next. | Your knight rides a wooden horse with wildly exaggerated jumping arcs. Walking across without jumping costs 25 points; go back west of the missed hurdle and retry. Holding F does not auto-jump. Each friend's F lifts only their own hobbyhorse; successful crossings contribute to the shared round. |
| Squire Sled Slalom (S, Crag) | Step onto S to send a volunteer squire downhill on a borrowed shield. Use **up/down aim or movement** to steer its momentum; release to coast. **Hold F to brake** and reduce sideways drift. Thread six numbered gates in order within 45 seconds. | You remain on foot; the shield-mounted squire is the racing piece. Only the knight who starts it steers or brakes. Snowbank scrapes cost 25 points. Missing a gate ends the round with a written complaint; step off S and back to retry. |

| Carnival Duck Range (K, fair) | Step onto **K** at the booth. The fairground lends a BB rifle: **arrows swing the gun**, **tap F** fires. Three rails of ducks slide at very different speeds; the near rail pays in speed, the far in patience. | The camera is bolted to the counter, so what you see is where you hit. Forty shots, forty seconds, ten ducks. Knocked ducks stand back up, dignity intact. |

| Chamber-Pot Put (P, Strand) | Start on P. **Up/down aim or movement** changes the launch angle. **Hold F to charge**, then **release F to throw** a clean pot. Read the windsock and land within each of the three numbered target brackets: 45 seconds, eight pots. | The starting knight is the thrower; an idle host does not prevent a friend playing. Only a landing scores, not flying through a target or walking over it. Wind changes between targets, misses cost 25 points, and moving away from the tee cancels unthrown power. The keeper invoices the sea for eight lost pots. |
| Wheelbarrow Cross (X, Timberyard) | Step onto X to send a volunteer forester through four numbered logs. **Left/right aim or movement** brakes/accelerates; **up/down** steers with momentum. **Tap F just before each log** to hop it. Four logs, 40 seconds. | Only the starting knight drives. Ruts and scraping the lane cost 25 points and slow the cart; grounded log crossings end the attempt. Held F cannot bunny-hop forever. The knight stays on foot, so use aim keys to drive without walking off the ring. Step off X and back to retry. No treasury payouts. |
| Shield Surf (U, Royal Dog-Paddle) | Step onto **U** to send a volunteer squire surfing on a borrowed shield. Use **left/right aim** (arrow keys) to move the gold balance pointer into the green zone. Release the arrow to keep that lean. When the moving wave turns gold, **tap F, then release it**. Keep the pointer green until the wave clears. Six waves in 20 seconds. | Only the starting knight controls the board; the knight stays safely on foot. Targets change after every wave. Movement keys can steer too, but aim lets you stand still. An early tap or held F cannot catch the next crest. A missed crest or bad balance causes a wipeout, not damage or a treasury charge. Step off U and back to retry. Plays, wins and best score are saved separately from older games. |

| Belfry Bat-and-Ball (V, Bashful Belfry) | Step onto **V**. Use **left/right arrows** to slide the shield paddle; **tap F** to serve a turnip. Catch its return and shift the paddle to change its ricochet. Ring **four different tax bells**, three turnips, **40 seconds**. | Only the starting knight controls the shield; the knight remains safely on foot. A missed catch costs a life and 25 points. Release F before serving a replacement; held F cannot auto-serve after a miss. Three lost turnips end the round. Step off V and back to retry. No treasury charge or farming reward; best, wins, plays and an airborne turnip survive checkpoints. |
| Jester's Half-Pipe (J, Mummers' Hollow) | Step onto **J**. Hold the **right/left arrow shown on the board** to pump toward the lip. **Tap and release F when the lip turns gold**, hold the same arrow to spin, then **release arrows when the spin turns green** to land. Four tricks, **30 seconds**, three helmets. The pumping direction alternates after each pass. | Only the starting knight controls the barrel-board; the knight stays safely on foot. Early F, a missed lip or an unstraightened landing dents a helmet. Holding F cannot auto-launch. Three dents or timeout end the round; step off J and return to retry. Medal payout is once-only, while best/wins/plays and airborne rotation survive checkpoints. |
| Trencher Toss (D, Goose Green) | Tap/release F to toss; left/right steer goose to the gold landing ring. 3 catches, 30 seconds, 3 misses. Fixed-point plate drift and separate saved records; free score-only practice. |
| Cabbage and Roll Racing (C, Rusty Hippodrome) | Three laps, three rivals, 35 seconds. Up whips / down brakes / left-right steers; tap F throws a lane-targeted cabbage. Avoid barrels. Free practice saves records, not gold; B nearby runs the separate 10-gold spectator card. |

Mistakes subtract 25 points; the time left provides a bonus. A completed round
adds **25–1,000 team score**, never repeatable treasury spoils. Bests, plays and
wins are saved with the realm and mirrored to native friends; browser friends
see the host's same course and boards. Leaving a room cancels the current round
but keeps its records. After a round ends, **step off the start ring and back
on** to replay. During a round F is the game's action rather than ordinary
weapon fire.

**The Goat's Western Survey:** walk to the gold E stakes in **Wonky Windmill**,
**Wheyward Downs**, and **Gullible Cliffs**; press **E** beside each one. Signed
stakes turn green. All three pay **45 gold and 2 ore once**, then allow the goat
lift to be built on the cliffs' northern plot. The lift itself is a paid work;
a survey does not spend your treasury automatically. Western visits, survey
signatures and Cheese Chuck records save independently of the original visit
bits and four-game arrays, so older saves keep their ramble/inn progress.

**The Abbot's Lost Picnic:** this is an ordered, cooperative errand rather than
three interchangeable flags. Press **E** beside the **Causeway troll** to collect
the claim ticket; press **E** beside the **Priory hamper** to recover lunch; then
press **E** beside the **hungry abbot at Washbucket Quay** to return it. Visiting
the hamper or abbot early cannot skip a step. Any living, unstunned friend can
carry out the next step. Delivery pays **60 gold and 3 wax once** and unlocks the
priory's Pigeon Post plot. Quest boards and green rings show shared progress.

**The King's Missing Forecast:** a mountain adventure with an ordered lens
puzzle. From the stables, walk west to **Candlewick Crag**, north to **Cuckoo
Observatory**, east to **Rookery Pass**, and south back to the stables. The crag
also connects south to Gullible Cliffs; the rookery connects east to the
mine-head. All roads are walkable without construction.

1. Press **E** by the **Rookery clerk** to accept the royal weather job.
2. Retrieve the **brass rooster** with **E** at Candlewick Crag.
3. At Cuckoo Observatory, align the **BOOT**, **GOOSE**, then **CROWN** lenses
   with **E**. Wrong-order lenses cannot skip a step; holding E at one lens
   cannot finish the others.
4. Return the forecast to the **Rookery clerk**: *“Mostly knights, scattered
   embarrassment.”* Delivery pays **55 gold + 3 ore once** and unlocks the
   observatory's Fair-Weather Ropeway plot.

Living friends share the errand. Its progress, northern visits, construction,
and sled records save separately from the earlier ramble and arcade records.

**The Bickering Beacon:** walk north from the Causeway to the keeper and press
**E** to accept the missing lamp-oil errand. Go east through the saltworks and
south to the Strand's crab-guarded parcel. Press **E** there at **low tide**; the
board shows the tide and seconds remaining. Low and high tides each last six
seconds. Wait on the dry path: all routes stay open. Early, high-tide, or
nonliving-player interactions cannot skip the errand. Return to the keeper and
press **E**: the beacon lights, paying **65 gold + 3 bone once** and unlocking
the Crab-Approved Harbour plot at Saltspite Works. Coastal visits, quest
progress and Pot Put records are separate defaulted save fields; previous
ramble/inn and arcade records retain their original requirements.

**Wind-country ramble:** visit Sir Honks-a-Lot Picnic Green, the Knotty
Kitewright, the Hill of Extremely Loud Flags and the Millpond through their
kingdom doors. All are free to explore.

**Trencher Toss** is a medieval flying-disc event inspired by the old
multi-sport cartridges. Step onto **D** in Goose Green, **tap and release F**
to toss a wooden dinner plate, then use **left/right arrows** to steer the
goose under its **gold landing ring** before touchdown. The plate drifts in
the wind and each catch has a different target. **Three catches in 30
seconds** win; **three misses** end the picnic. Only the starting knight
steers the goose; friends can watch and explore. The round, airborne plate,
goose and separate best/plays/wins survive checkpoints. Holding F never
auto-tosses another plate. Step off D and back to retry. Free practice gives
score, never treasury gold.

**The Kitewright's Questionable Signal Job:** press **E at K** to accept,
collect the **picnic sail** at Goose Green, then use the hill's marked levers
in **FISH → BOOT → CROWN** order. Wrong flags do not advance the job;
completed flags remain lit. Return to K for **60 gold + 3 wax once**. Hold E
at the separate kite-post plot to build **The Extremely Unwise Kite-Post**
for **90 gold + 3 bone + 3 wax**. It opens two-way **Kitewright ↔ Windmill**
travel. The commission does not purchase the building for you, and walking
remains free. Earlier save masks and records are unchanged.

### Wren's ramble, lost property, and building plots

- **Grand Ramble:** visit the original six ramble places (Broadsheet Row,
  Fool's Green, Wonky Windmill, Old Watch, Washbucket Quay, Wayfarer Meadow)
  for a one-time **150 treasury gold**.
- **Royal lost property:** walk into the sparks to recover the windmill's cog,
  the old watch's bell, and the quay's heroic left sock. Finding all three pays
  **100 gold and 4 ore**, once per realm.
- On a building plot's gold E plate, **hold E** to request construction from the
  realm treasury. A passing tap buys nothing. Release E before another request;
  insufficient materials never charge you. These are permanent surface works,
  not changes to the saved Undercroft settlement.

| Plot | Price | Requirement / use |
|---|---|---|
| Goose-Proof Bakery, windmill | 80 gold, 2 wax | No visit requirement. Feeds the goose so courier rounds no longer incur its nuisance penalty. |
| Wayfarer's Beacon, old watch | 140 gold, 4 ore | Visit three of the original six ramble places. Casts a pool of light; with the recovered bell, starts the Lamplighter's Walk. |
| The Bent Spoon Inn, meadow | 180 gold, 4 bone, 3 wax | Visit all original six. Opens coach plates at both the meadow and gate for two-way travel. |
| Sir Damp's Ferry Office, marsh | 90 gold, 6 bone, 2 ore | Light all three road lanterns. Opens two-way ferry plates between the marsh and Washbucket Quay. |
| Counterweight Goat Lift, cliffs | 70 gold, 3 bone, 2 ore | Sign the three western survey stakes. Opens two-way lift plates between Gullible Cliffs and Wonky Windmill. |
| The Overqualified Pigeon Post, priory | 100 gold, 4 bone, 2 wax | Return the abbot's picnic. Opens two-way post plates between Puddingstone Priory and the Jousting Lists. The pigeon considers you oversized mail. |
| The Fair-Weather Ropeway, observatory | 110 gold, 3 ore, 2 wax | Deliver the king's forecast. Opens two-way travel between Cuckoo Observatory and the mine-head; the cart operates regardless of the forecast's accuracy. |
| The Crab-Approved Harbour, saltworks | 90 gold, 3 bone, 3 ore | Deliver the beacon's lamp oil. Opens a two-way barge between Saltspite Works and Washbucket Quay; the crabs negotiated weekends but misunderstood the calendar. |
| Badger's Board-and-Breakfast, bothy | 95 gold, 3 bone, 2 wax | Deliver BOOT, HAT, BEARD mushroom soup to the badger cook. Opens two-way Bothy–Orchard wagon travel. |
| The Abbey Punt House, Sunken Abbey | 105 gold, 4 bone, 2 ore | Return Brother Bucket's bell after FISH, BOOT, DUCK sluice work. Opens two-way Abbey–Orchard boat travel. |
| The Air-Mail Monstrosity, Pigeon Parliament | 100 gold, 4 bone, 2 wax | Return the three lost tax minutes to the Speaker, in any collection order. Opens two-way Parliament–Rookery mail-cart travel. |
| The Wheely Inn, caravan camp | 110 gold, 3 bone, 2 wax | Explore Hollow, Laundry and Caravan, and win a half-pipe medal. Opens two-way camp–Fool's Green caravan travel. The king calls a mattress on wheels mobile infrastructure. |
| The Suspiciously Fast Coach, Hippodrome | 120 gold, 4 bone, 2 ore | Accept W's commission, collect the wheel / axle / receipt, then return to W for 85 gold + 2 ore once. Buy this separately for two-way Hippodrome–Stables travel. The goat signed the safety certificate. |
| The Extremely Unwise Kite-Post, Kitewright | 90 gold, 3 bone, 3 wax | Accept K's job, find the picnic sail, signal FISH / BOOT / CROWN, then return for 60 gold + 3 wax once. Buy separately for two-way Kitewright–Windmill basket travel. No safety officer was consulted. |

**The Lamplighter's Walk:** recover the lost bell in the Old Watch and build its
beacon. Walk beside the lantern posts in the **Old Watch**, **Apple Apology
Orchard**, and **Sir Damp's Marsh**, then press **E** to ring the bell and light
each one. The frogs applaud politely. All three lights pay **75 gold and 4 wax
once**, then permit the ferry office to be built. The lights and reward survive
saves; co-op friends share progress. The orchard and marsh add exploration, not
two extra requirements to an older save's six-place ramble or inn unlock.

Stand on a coach, ferry, goat lift, Pigeon Post, ropeway, or harbour barge's marked travel plate for a moment to take the whole
party across. Locked plates say which building is missing. Walk off a plate
before using it again; arrival positions sit clear of the return trip, so the
party cannot bounce endlessly between stops.

Old checkpoints gain the continuous kingdom and missing visit maps. A
legacy countryside or College visit gains its appropriate surface return
door without moving the current knight. Occupied settlement cells are not
excavated or replaced; the Undercroft and existing home wings stay where they were.
This migration also happens immediately when a private host checkpoint is
restored. Pending country discoveries and building requests survive a failed
realm save followed by a checkpoint/restart; retrying commits the reward or
purchase once, without putting private transaction requests on a guest mirror.

## The horse ranch and the flower fields

Enter **Horse Ranch** on the kingdom, or use the retained gate in the
stables yard's south-west corner. The ranch has a red barn, Old Barley the
black draft braided at the hitching rail with the groom's brush going, and a paddock of war horses. North of it lie
**the flower fields**: wide open grass full of small yellow flowers, a few
mares at grass, and the morning sun low over the trees.

The pride of the ranch is **Helios**, a Belgian draft, brown with a white
star and a yellow mane. Nobody rides Helios and nobody buys him. He runs
the fields in long loops, and at the gallop, with the sun through it, his
mane and tail burn gold to white. Walk up and press **E** and he lets you
see the star, then runs.

**The stud book.** In front of his stall, hold **E** with Mist saddled at
the stables (she is the realm's mare) to bring her to him. It costs 400
gold from the treasury every time, and he is particular: about one try in
25 gives a foal, Phaethon or Aurora, with his star and his yellow mane. The
foal grows up in the fields as the realm wins delves (a yearling after two,
grown after five).

## The stables and the lists

**The stables** keep the realm's three mounts, each over its half-door, the
saddled one out in the yard. Stand on a stall's plate and **hold E** to
saddle that mount; at the trough's plate, to tend it (brushed, watered, the
tack checked: it carries its rider a knock longer, for one bout). Wat the
groom has opinions about all three.

| Mount | Rides |
|---|---|
| Bramble | Steady: the most balance and the widest brace |
| Cinder | Fast and heavy: the soonest meeting and the hardest hit, the narrowest strike |
| Mist | Light and true: the widest strike, and the rival's guard is read early |

**The lists:** stand on the plate by the red pavilion and **hold E** to
mount. Then a course at a time:

- **F** spurs. The horses run at each other down either side of the tilt,
  faster all the way.
- **W** aims high, at the helm; **S** aims low, at the shield. The rival's own
  shield shows where he guards a moment before you meet: strike where it
  isn't.
- **F** strikes as the lances meet. A ring closes over the tilt where you
  will meet, and burns gold while a strike would land.
- **Space** braces as his lance comes in, and takes it a grade lighter.

On his shield is a point (two if your lance breaks on it at the very moment);
where his shield isn't is two (three at the very moment). Every hit knocks
the rider struck; a rider out of balance goes over his horse's tail and onto
the sand, and the bout is over. Otherwise three courses are counted. Hold
Space at the end to withdraw. Your rivals, in order: **Sir Kay** (who has
opinions about it), **Sir Palamedes**, and **Sir Lancelot**, the realm's
champion. The sidebar keeps the score.

## King Brannoc and the forges of Caer Dwfn

Under the realm lie the halls of Caer Dwfn, a dwarf-kingdom the goblin clans
took a hundred years ago. Every hall had a forge, and every forge went cold.
Once the party has cleared a floor of the Mines, **King Brannoc Onehorn**,
the last of its kings, comes up the Mines' shaft and camps at the mine-head,
by the ruin of his gate-hall. Walk up to him.

- **His missions** come one at a time, read at the plate before him: first
  **The Upper Workings** (clear eight halls of the Mines: 150 gold, 10 ore),
  then, once the Ore-Forge burns, **The First Fire** (fell Cinderjaw, who has
  been chewing it for a century: 4 gems).
- **His works** the realm pays for, and pays a great deal for, and his
  dwarves build them while you play, at home or below (the sidebar counts
  them up):

  | Work | Where | Price | Takes |
  |---|---|---|---|
  | The King's Hall | the ruin's plate at the mine-head | 3,000 gold, 40 ore, 20 bone, 6 gems | three dwarves about half an hour |
  | The Ore-Forge | its plate in the Mines (below) | 2,000 gold, 30 ore, 4 gems | three dwarves about twenty minutes |

  More missions done means more dwarves home, and faster work. The hall
  rises out of its ruin a quarter at a time: the rubble carried off, the
  walls, the roof, the great door. When it stands, the King kneels in it and
  swears Caer Dwfn to the crown (**A Baron Sworn**), and its door opens.
- **The forges.** Once the Upper Workings are his, every floor of the Mines
  has a forge-hall off its first room: the Ore-Forge, in ruin. Pay at its
  plate to rebuild it; once it stands, relight it there with **8 embers**
  from Dragon Keep (**The Hammers Ring**). A burning forge has smiths at its
  anvils, adds to the tribute, and makes **dwarf-forged mail** for every
  knight (one piece of armor). The war table in his hall shows every forge
  of Caer Dwfn and how it stands; the Great Forge, the crown of them all,
  waits cold at the end of his hall for the first fire.
- **Tribute:** his miners follow you down. Every floor the party clears pays
  3 ore and 30 gold once the Upper Workings are his, and 1 gem and 60 gold
  more for every forge burning. The **ledger** on the lectern in his hall
  keeps what the realm has paid in and what he has paid back.

His people are about: his guards, his miners going down the shaft with their
picks and coming up with sacks of ore, his builders hauling stone, a carter
taking the realm's share to market through the gate courtyard, and his man
waiting by the signpost there. The design and what comes next are in
[The Barony](BARONY.md).

## The Undercroft: home

The Undercroft is down the gate's stair: the company's cellar under the
Delve's gate. The **Winding Stair** in its middle goes down to the first
floor (or to a landing further down; see Wren's map). A finished delve
(**R** after a victory or a fall) begins again at the gate.

The Undercroft is built out with the spoils the party carries up. Each
station has an engraved plate in front of it: stand on it to read its
ledger (the next rung, what it does, and its price), and **hold E** to buy it
from the realm's treasury. A tap in passing buys nothing. What is built
stays built for every delve after, and friends see the same cellar.

| Station | Keeper | What each rung does |
|---|---|---|
| Tobbin's forge | Tobbin | +8% damage for every knight, up to +40% (a *Tobbin's Edge* card in each deck) |
| Blaise's hearth | Old Blaise | +10 health for every knight, up to +40 |
| Tobbin's rack | — | Knights take a potion down; then a bomb more; a second potion; a thunder scroll |
| Chapel of Bonds | — | A fallen knight rises again at 40% health, once a delve; then once a floor. Paid in bonds |
| Wren's map table | Wren | A landing at floor 2, then floor 3, once the party has reached them. Tap E at the stair's mouth to choose |

Deeper rungs need the party to have reached a floor first. The forge is
cold ash until its first rung; the rack fills with what it hands out; the
chapel's candles light; Wren's map gains a mark for each landing. The keepers
speak when a knight walks up to them.

**Merlin** stands west of the stair: the realm's guide. Walk up to him and he tells you the one thing most worth
doing next that the realm hasn't done yet: build something, spin Fortune's
wheel, open the coffer, take Sir Ector's lesson, free the people in the
cages, bomb the walls that breathe, chase a rune, try the Pit, mind the
witches (he was a toad once, for a fortnight). With nothing pressing, he
reminisces.

## Rescues

Now and then a floor's treasure room holds a cage, and someone in it, who
calls out as the party comes in. Stand by the cage and hold E to work its
lock: they step out, go home up the stair, and stay. Each delve keeps its
own prisoner, found on one of its floors until they're home:

| Who | Found in | At home, they give |
|---|---|---|
| Mabel, the cook | the Mines | her stew: 15 more health for every knight (a home card) |
| Brother Anselm | the Crypt | his blessing: one more second wind a delve |
| Pip, the scribe | the Drowned Archive | his maps: every floor drawn whole on arrival |
| Maud, the alewife | the Fungal Deep | her ale: a potion for every knight on the way down |

The people at home stand about the Undercroft and have a word for whoever
walks up. The first rescue is **Out of the Cage**; all four home is **A
Full House**.

## Lady Tallow

Blaise's cat: a fluffy grey
cat with a very small crown and a very large opinion of herself. Once
Blaise's hearth is built to its second rung she sits by the fire at home,
and comes down the Winding Stair with the party.

In the delve she keeps to her knight's heel (the first knight standing).
A monster that comes too close gets hissed at, and freezes, startled, for a
second (she rests her voice for six). Gold and spoils lying about she
fetches, trots home, and drops at her knight's feet, into what they carry;
potions and the like she leaves for the knights. No monster can touch her.
Ten fetches in one delve is **Cat Person**.

## The Training Yard, and Grubbins

North of the Undercroft is the Training Yard: three quintains to strike with
anything you carry, ultimates included. Each shows the damage it took in
the last second. They never fall and never strike back, and the blows still
charge your ultimate, so the yard is where to try one before the stair.

In the yard's corner stands a stall, shut, with a sign. The first time a
loot goblin gets away from the party, he comes back: **Grubbins** opens the
stall and sells three cards a delve for gold (60 for a common, 150 for a
rare, 300 for a relic), each landing at your feet. Stand on a ware's plate
to read it; hold E to buy. His stock is, he insists, legitimately acquired.

## Sir Ector's lessons

Knights learn by fighting. Everything the party fells is experience for
each knight in it, kept by the realm under the knight's name, and every
level a knight reaches is a lesson waiting with **Sir Ector**, Arthur's
foster father, who keeps the south-west corner of the Training Yard.

Two lecterns stand before him, a talent on each, a choice at every
level. Stand at one to read its lesson and hold E to learn
it. A lesson learned goes down the stair with that knight as a held card,
every delve after.

| Lesson | Level (experience) | Left | Right |
|---|---|---|---|
| I | 2 (600) | Iron Hide: one more piece of mail | Keen Edge: 12% more damage |
| II | 3 (1,800) | Quickdraw: strike 15% faster | Fleet Foot: walk 12% faster |
| III | 4 (3,600) | Morningstar: a morningstar circles you | Arc Lightning: each hit sparks to another |
| IV | 5 (6,000) | Bloodthirst: health back from kills, at most 2 a second | Lady's Veil: your hits mend a friend, at most 2 a second |

A monster is worth a twenty-fifth of its bounty in experience (a skeleton
4, a demon 12, the dragon 80). The home's sidebar shows each knight's level
and any lessons waiting. The first lesson is **Ector's Pupil**; all four,
**Master of Arms**.

## The Trophy Hall, and Sir Kay

South of the Undercroft, down the runner, is the Trophy Hall: plinths of
pale stone along its walls, the dragon's in the middle, and the Grail's
dais at the far end between two candles. Sir Kay, Arthur's seneschal,
keeps it, and has opinions about everything in it.

Each guardian the realm fells stands on its plinth as a bronze statuette,
its own art at half size; felled ten times, it is gilded. The dragon's
statuette lies on the big plinth in the middle, and the Grail, once found,
stands on its dais. An empty plinth wears a dust sheet. Walk up to a
plinth to read its plaque: how often its guardian has fallen. Every plinth
filled is **A Full Hall**.

## The west wing: Maud's tavern

The Trophy Hall's west doorway is full of rubble, planks nailed across it.
Once the party has been down the stair, Tobbin's crew will dig it out: stand
on the plate before it and **hold E** (**The West Wing**, 300 gold, 20 ore and
10 bone). The wall opens on the realm's tavern, the **Siege Perilous**.

- **The bar** runs along the north wall under shelves of bottles. Maud keeps it
  once she has been rescued from the Fungal Deep (she moves from the
  Undercroft to her taps); until then the taps are dusty and the Herald drops
  hints about a cage.
- **Maud's rounds:** once she's home, her three taps before the bar each pour
  a round for the next delve, paid from the treasury; stand at one and **hold
  F**. One round waits at a time (the sidebar says which), and it's drunk at
  the top of the stair:

  | Tap | Round | Does | Gold |
  |---|---|---|---|
  | West | Maud's Mushroom Ale | A second wind for every knight | 40 |
  | Middle | Dragon's Breath Stout | +15% damage for every knight | 80 |
  | East | The Herald's Reserve | Fortune's audience starts warm (300K) | 60 |
- **Sir Dinadan's songs:** the Round Table's jester-knight (who, in Malory,
  wrote a mocking song about King Mark) keeps a little stage east of the
  tables. For gold he'll sing for the next delve, one song at a time, an aura:
  a knight alone always hears it; in company, a knight hears it
  only within nine steps of another (a note floats over whoever does). Stand
  together for Dinadan; stand apart for the Lich.

  | Plate | Song | A knight who hears it | Gold |
  |---|---|---|---|
  | West | The Lay of Haste | runs 15% faster | 50 |
  | Middle | The Iron Hymn | fills up to 2 armour, sharing the mail cap | 70 |
  | East | The Red Ballad | mends 3 more as monsters fall (within the leech cap of 3, and 2 a second) | 90 |
- **Beaumains, the kitchen knight,** waits by the west table. Sir Kay named
  him for his soft kitchen hands; in Malory he was Sir Gareth all along and
  told nobody. Stand on his plate and **hold E**: for **120 gold** he goes
  down the stair with the party for the next delve (once hired, he waits by
  the Winding Stair, and the sidebar says so).
  - In a fight he keeps five to nine steps off whatever is nearest and throws
    carving knives at it (8 damage on the first floor, 2 more a floor down),
    sidesteps the shot coming at him, and follows the party through the
    doors.
  - Monsters aim at him as they do at knights. Felled, he lies where he fell
    until the fight is over, then gets up at half his health.
  - He is not one of the party's knights: a guardian doesn't grow for him, he
    can't lose the delve for anyone, and he isn't company for Dinadan's songs.
  - With one knight over the same eight delves, he cut the wipes on the second
    floor from 6 to 3, on Dragon Keep from 28 to 16, and in the Archive from
    14 to 9.
- **The rumour board**, east of the bar: walk up to read the next floor's
  supported political structure and checked chiefs, what Fortune's wheel
  said, and what they say in the tavern. Clan floors advertise an **ANY**
  route rather than promising a sole guardian.
- **The Siege Perilous** stands in the corner on its step: the chair at the
  Round Table that only the worthiest knight may sit in. Stand on it for a
  second. A realm of 24 achievements or more is found worthy, and earns one;
  anyone else is found wanting, stung, and put out of the chair. Its plaque
  counts the realm's deeds.

## Dame Fortune's Wheel: how the delve goes

East of the Undercroft is Dame Fortune's hall, with her great wheel, red
drapes and an audience on the benches. Stand at the lever and **hold E**:
the wheel spins once a delve, and the wedge it stops on is the delve's mode.
Fortune's own plate sells more wedges: her third rung (800 gold, 6 gems and
a dragon's scale, once the party has been three floors down) opens three
wedges (Turbo, Rune Rush, All Random), and her fourth (1,200 gold, 8 gems, 2 scales, from the fourth floor)
three more.

| Wedge | The delve | Spoils |
|---|---|---|
| The Long Way Down | As it always was | 1x |
| Lights Out | Dark but for each knight's lantern and the fires; eyes and embers still glow | 1.5x |
| Giant's Feast | Every ordinary monster is a giant: bigger, 1.8x health, twice the drops | 1.5x |
| Mimic Fair | Every won fight room leaves a chest, and half of all chests bite | 1x |
| The Collapse *(Fortune I)* | Each floor stands 2½ minutes (+30 s a floor), then the ceiling comes down until you take the stairs | 2x |
| Glass Jaw *(Fortune I)* | Every hit lands twice as hard, both ways | 2x |
| Hold the Stair *(Fortune II)* | One great hall a floor, a horde coming in waves, the stairs in its middle | 2x |
| Gauntlet of Guardians *(Fortune II)* | A passage, then the guardian, every floor | 1x |
| Turbo *(Fortune III)* | Down in the delve everything runs a third faster, knights, monsters and clocks alike | 2x |
| Rune Rush *(Fortune III)* | A power rune in every fight, and another within about ten seconds of the last one taken, while the fight lasts | 1x |
| All Random *(Fortune III)* | Each knight carries another knight's ultimate for the delve (the sidebar names it), and every floor starts it half charged | 1x |
| Ironman *(Fortune IV)* | No second winds at all: not the Chapel's, not Brother Anselm's, not Maud's ale. Fall and stay fallen (the Pit's Talisman, the Pit Tyrant's own, still saves) | 3x |
| Hollow Walls *(Fortune IV)* | Every floor hides a vault behind a cracked wall: bring bombs | 1x |
| Sponsors' Night *(Fortune IV)* | Every viewer brings a friend, so the audience counts double and the fan boxes rain | 1x |

The spoils a delve banks (whole at the stairs and on victory, half on a
fall) are multiplied by its wedge. The HUD names the mode, and in the
Collapse it counts down the floor's time.

## Achievements and the Herald's coffer

The Herald notices things. A whole slime family pruned, a keg that takes
three monsters with it, a goblin caught (or let go), a guardian down, the
dragon, the floor below the dragon, the Grail, a lesson learned, a cage
opened, a full Trophy Hall, a baron sworn, a forge relit, a champion
unhorsed: forty-two moments in all,
each earned once per realm. A new one drops a banner across the view
(**NEW ACHIEVEMENT!**, its name, the Herald's line about it, and its reward)
and puts a box in the **Herald's coffer**, on its pedestal in Dame Fortune's
hall.

Stand on the coffer's plate and hold E to open the next box before the
audience. Its contents go straight into the realm's treasury:

| Box | Holds |
|---|---|
| Bronze | 40–90 gold, a common material or two |
| Silver | 100–180 gold, materials, a gem |
| Gold | 220–350 gold, materials, gems, an ember, a bond |
| Legendary | 500 gold, five gems, three embers, a dragon's scale, two bonds |

The sidebar at home counts the feats earned and the boxes waiting.

## Fortune's audience

Every delve is a show, and somebody is watching. The header counts
**Viewers**, and the count climbs with spectacle: the audience reacts to
the moments the Herald calls, and a little to every kill.

| Moment | Viewers |
|---|---|
| A kill (a dangerous kind) | 2K (8K) |
| First blood, a double slay, a room cleared | 5K |
| A triple slay, a guardian rising | 15K |
| An ultimate, a mimic, a new floor | 20K |
| A flawless room, a relic, a knight getting up | 30K |
| Unstoppable, a knight going down or rising on a second wind, the floor collapsing | 40K |
| A goblin caught, the stairs into the deep | 50K |
| A guardian falling | 90K |
| The dragon slain | 120K |
| The Grail | 300K |

At each milestone a fan throws a **fan box** down to the party: a crate on
a parachute in Fortune's red and cream. A gold ring marks where it will
land, and it bursts there into a card and two piles of gold. The first comes
at 250K viewers, then at 1M, 2.25M, 4M: each needs more than the last. A
million viewers in one delve earns **Must-See TV**; ten million, a delve
into the deep, **Prime Time**.

The Undercroft is off the air: the count starts at nothing on the stair,
every delve. The realm keeps its **best show**, and the home's header
shows it. Invited friends see the same count.

## Fortune's dares

On every floor below the Undercroft, Dame Fortune dares the party, the way
the Delve's sponsors do. Her red board drops in as the party arrives, and
the sidebar keeps the dare and how it stands.

| Dare | What it asks |
|---|---|
| Not a Scratch | two rooms on this floor without a scratch |
| Stand Firm | no rolling, the whole floor (judged at the stairs) |
| Against the Clock | the stairs within four minutes (judged at the stairs) |
| Show-Off | 300K more viewers on this floor |
| Make It Quick | the guardian felled within 45 seconds of it rising |
| Unstoppable | four in a breath, twice |

A kept dare opens her purse: gold (60, and 20 more a floor down) and a gem
for every knight standing, carried like any spoils, and 100K viewers. The
Unknown, with no stairs and no guardian, only dares what it can. The first
kept dare is **Daredevil**; five in one delve, **Fortune's Favourite**.

## The Herald's Bestiary

The card screen's last page (Tab, then down past the book) is the Herald's
Bestiary: every kind of monster the realm has felled, how many, and what
the Herald thinks of it ("Loot goblin: has your gold. Is leaving. Is
gone."). Kinds not yet met are only rumours. The realm keeps the count
across every delve; guardians have the Trophy Hall instead.

## Wren's bounties

Wren pins three bounties to the board on its posts by the stair, the way
the Delve's quest-givers do. Walk up to the board to read them: what each
asks, how far along it is, and what it pays. The home's sidebar lists them
too.

A bounty asks for something a delve can do: so many of a monster (25
skeletons, 4 warboars), a moment (three flawless rooms, a goblin caught,
two guardians), a floor (Dragon Keep, the deep), or a show (a million and a
half viewers). Progress carries from delve to delve. The moment one is done
it is paid into the realm's treasury, wherever the party is, and Wren pins
the next in its place.

She only pins what the party can fairly do: the board grows with the floors
it has reached, from bats and skeletons on a new realm to the dragon, the
Fungal Deep and, at the last, the Grail itself.

The night's newer wonders are on her board too: a vault behind a cracked
wall (Through the Wall), Snibbet's hush money (Hush Money), five runes
(Glow-Chaser), being hexed into a frog twice (Frog Prince: Tobbin is running
a book on it), and five hexers (Witch Hunt).

## The realm we build

The harness's work grows the town. The party's adventures win the wild.

- **Spoils.** Each delve drops its own material:
  - gold anywhere;
  - bone and candle-wax in the Crypt;
  - ore and gems in the Mines;
  - embers in Dragon Keep;
  - a dragon scale for slaying the dragon.

  A room cleared with two knights standing earns the party one **bond**, so
  some wishes can only be paid for by playing together.
- **Banking.** What you carry is banked into the island's treasury at the
  stairs and on victory. A party that falls, or turns back with
  `/dungeon off`, keeps half. The sidebar shows what each knight carries.
- **Wishes.** Anyone makes one in plain words: `/dungeon wish <words>`. Then:
  1. `/dungeon grant <n>` hands an unwritten wish to angelX in the composer.
     angelX drafts it as `.angel/realm/wishes/<id>.wish`: pixel art in realm
     inks, a price in spoils, and a place to stand near.
  2. `/dungeon wishes reload` reads the draft in.
  3. Once the treasury covers the price, `/dungeon grant <n>` raises it. So
     does **Enter** on the card screen's realm page.

  The wish then stands in the mini-viz wild, with its name on a plaque. The
  realm keeps who wished and who paid. A draft may use `sheet art <id>.png`
  from `.angel/realm/sheets/` instead of ink rows; it is checked and reduced to
  32×32. A crowded realm tries a closer lot without a margin; if no free
  ground remains, the grant reports the landmarks waiting for ground.
- **Saving.** The treasury and wishes are saved beside the island's world
  rewards (`~/.angelX/world-rewards/<island>.realm.json`). Wishes are data,
  checked like cards, never code. A large landmark can't be cheap: the price
  is raised to at least two gold per inked pixel.

## Roll or shield

**Space** is the guard key. Every knight starts with the **Dodge Roll** card.
A quick tumble carries you through anything, untouchable. You can't shoot
while rolling, and it needs a moment to rearm. The rare **Kite Shield** card
swaps the roll for a shield. Hold Space to raise it toward your aim: shots
from the front glance off, but you walk at under half speed and can't shoot.
One guard at a time; taking the other drops yours. **F** fires, and so do
the arrow keys. Shift+WASD still rolls.

The raised shield is the one on the knight's arm, brought round to face
the way you aim: held out to the side, across the front, or ahead of you
when you face away. The dwarf, Dame Thrudi, carries none; her guard is the
**Axe Spin** below.

### Dame Thrudi, the dwarf

A dwarf of the deep halls, with a braided beard and an axe in each fist,
chosen on the intro like the rest of the company. Her arm is the **Twin Axes**
(a wide, hard sweep up close) and her guard is **Axe Spin**: press Space and
she turns on the spot, both axes out, cutting everything within three tiles
twice (each sweep in full on the nearest three, a quarter on the rest) and
cutting down every shot in the circle. She walks on while it lasts,
slowly, then takes a breath (two and a half seconds) before the next. Her
ultimate is the **Axe Storm**: the spin, over and over, for three seconds.

## Arms, and how they feel

Your shots are fast and theirs are slow. Each of the three arms is a shape,
not a number:

| Arm | Shape | Shot speed | Rest | Shots a second | Damage | Extra |
|---|---|---|---|---|---|---|
| Knight (moving) | | 13 u/s | | | | |
| **Bow** | a stream: hold to keep firing | 40 u/s | 10 ticks | 3 | 24 | a small stray, so the stream looks alive |
| **Crossbow** | a rail | 70 u/s | 23 ticks | 1.3 | 55 | passes through everything in its line, leaving a tracer that fades |
| **Handgonne** | a fan: five balls in 30° | 34 u/s | 25 ticks | 1.2 | 14 each | kicks the knight back 0.6, shoves what it hits, smokes; its balls fall after 20 ticks |

A card's extra shot is another arrow or bolt beside the first, or another
whole fan for the handgonne. A forged arm's bolt flies at least 36 u/s.

**Hits land.** A hit sparks in the shot's colour and jolts an ordinary
monster still for 2 ticks (no more often than every 15, so a stream never
pins one) and knocks it back: a nudge from an arrow (0.1), a push from a
bolt (0.5), a shove from a ball (0.9), never through stone. Guardians, the
dragon and the Pit Lord are never jolted or moved. A death holds a white
frame for 4 ticks, then bursts. The handgonne shakes the view (2) and so does
a kill by anything heavy (3). Arrows and bolts that meet stone stick there
until the party leaves the room.

**The room's end pays.** Every monster shot still in the air when the last
foe falls becomes a gold coin (1 gold, at most 24 a room) and flies to the
nearest knight.

**Tempering.** Take (P) the arm already in your hand and it is tempered,
up to three times; past that the twin stays on the floor. The sidebar names
the tier (Bow II, III, IV) and the tray shows it under the weapon's plate.

| Arm | Once | Twice | Three times |
|---|---|---|---|
| Bow | a second stream | a third | the outer streams home |
| Crossbow | a thicker rail | it splits on its first kill | it chains |
| Handgonne | seven balls | they bounce once | they burst where they hit |

**Synergies**, named on the notice line when they go off:

- **Fire Arrows:** a Fire Pit thrown with a bow in hand sets your arrows
  burning for four seconds; what they hit burns.
- **Ricochet:** a crossbow rail that meets a flying shield turns off it to
  the nearest monster.
- **Powder Keg:** a handgonne ball through a cloud of gas lights it.
- Flame meeting gas still flashes it off.

**The feel ledger.** With `ANGEL_DELVE_FEEL=1`, each room writes a line per
knight to `feel.jsonl` beside the delve's save: shots, hit rate, kills,
seconds to kill, dead air (seconds of the fight with no damage dealt),
skills used and grazes.

Same-seed party-size clear times and damage taken, test commands and render
paths are recorded in [the pressure and arms review](DELVE-FEEL-REVIEW.md).

## Skills

Beside the weapon and the guard, every knight carries five **skills**, each on
its own key, shown in the sidebar under the spells. Nothing a knight raises
hurts a knight.

A skill is paid for in **valour**: three pips, filled by the hits you land
(five landed hits a pip, 120 valour each, 600 a pip) and by **grazes** (a
monster shot passing within half a unit of you without hitting, once a shot,
120 each; not while untouchable). Each use
spends a pip, then the skill rests for a moment. A knight who fights close
and dodges close gets skills; one who hides does not. The sidebar, the tray
(three pips along the weapon's plate) and the browser page show the pips.

| Skill | Key (first knight) | Second knight | Gamepad (hold LT) | What it does | Rest |
|---|---|---|---|---|---|
| **Fart Cloud** | **M** | **8** | A | Throws a canister six units ahead. It bursts into a cloud that swells, drifts a little and thins over seven seconds, and poisons whatever breathes it, for a few seconds after too. Bosses shrug off half. | a pip, 3 s |
| **Fire Pit** | **,** | **9** | B | A firebomb: a small burst, then burning ground for five seconds. Foes in it burn. Flame that reaches a cloud of gas flashes it off in one blast. | a pip, 3 s |
| **Mines** | **.** | **0** | X | Lays a mine at your feet, up to three at a time. It arms after a second and a half (it blinks while it does) and blows when a foe walks on it; a blast lights its neighbours. Knights never set one off. | a pip, 3 s between |
| **Barrier** | **;** | **-** | Y | Raises a palisade four units wide ahead of you, across your aim (a plank wall, side-on). Foes cannot walk through it and their shots break against it. It wears down under shots and the weight of foes, and crumbles when spent or after twelve seconds. Two at a time. | a pip, 3 s |
| **Shield Throw** | **'** | **=** | RB | You hurl your own shield along your aim. It strikes what it meets (a short stun each time), glances off stone up to twice, jumps to one or two near foes, knocks shots from the air, and comes back to your hand. While it is out you have no shield to raise. Every strike clangs. | a pip, 3 s |

Keys are read as pressed, not held: one press, one use. Hold **LT** on a
gamepad and the face buttons become skills (without it they are the roll,
sword, bomb and interact), and the d-pad's up, left, right and down cast the four
spells; Start keeps vigil and R3 quaffs. LT alone does nothing. RT fires, as F
does. The numbers behind them (damage, reach, rests, how many) are the named constants at the top of `together_shooter/skills.rs`, so
they can be tuned in one place. Side-on halls (see below) add one thing:
**S** held on firm ground is a **crouch**, and shots at the chest go over you.

## Health, armour and the take key

- **Bars.** Every monster that has been hurt shows its health over its head;
  a guardian, the dragon and the Pit Lord always do. Your own is under your
  feet. A hit flashes the monster cream, then red.
- **HUD.** The sidebar (in the terminal, a friend's angelX, and the browser
  page) always shows, for each knight: health, the weapon and guard in hand,
  each spell with its recharge, the cards in hand, bombs, the ultimate's
  charge and the weapon lying beside you.
- **Armour.** At two-fifths of your health the helmet pops off and you fight
  bare-headed; at fifteen in the hundred the rest follows and you stand in
  your smallclothes, shield and axes kept. Mended above a mark, the gear is
  back on at once. Every knight does it, the dwarf with her beard.
- **Weapons are swapped, not walked into.** Walking over a weapon card leaves
  it where it is, with its name and "P TAKE" over it. Press **P** beside it
  (a couch friend's key is **H**, and their sword is on **[**) to swap: your old
  weapon is set down where the new one lay, so you can swap back. Everything
  else (gold, potions, hearts, bombs, mail, shields) is still taken by
  walking over it.

## Ultimates

Every knight of the company carries one signature power. Fighting fills its charge, a little every second and a lot for
every hit the knight lands. A full charge glows gold at the knight's feet;
**R** casts it (a couch friend's key is **Y**). The sidebar shows each
knight's ultimate and its charge.

| Knight | Ultimate | What it does |
|---|---|---|
| Sir Percival | Pilgrim's Arrow | One great arrow through everything in its line, stunning each monster it hits |
| Dame Lynette | Lady's Veil | The whole party is untouchable for four seconds, and mends a little |
| Sir Gareth | Trebuchet | Two volleys of shells land where he aims, after their red rings close |
| Sir Galahad | Bladewind | He blinks from monster to monster, cutting six times, untouchable while he does |
| the Composer | Grand Chord | Every monster near is pulled into a knot before him, hurt and stunned |
| the Dispatcher | Sealed Writs | Six missiles, each turning after its own monster, bursting where they land |
| the Loop Knight | Stillhour | Time stops in a sphere ahead for four seconds: monsters and their shots hang still |
| the Money Knight | Midas Touch | The three nearest monsters turn to gold (guardians just lose a share) |
| the Competition Knight | Assassinate | A second's aim on the room's toughest monster, then one shot down the line |
| the Scryglass Knight | Phantasm | Two images stand beside the knight for eight seconds, fire with them, and draw fire |
| Dame Thrudi | Axe Storm | The axe spin, over and over, for three seconds, and half a second untouchable to begin |

The three party-wide holds don't chain: a Grand Chord, Stillhour or Lady's
Veil cast within eight seconds of another holds only for the short time
(Grand Chord 30 ticks instead of 75, Stillhour and Lady's Veil one and a half
seconds instead of four). The Herald notes the echo.

## How hard the delve pushes back

A party that has built well should still be in danger. Five rules keep it so;
`balance_sim` (an ignored test, `cargo test balance_sim -- --ignored --nocapture`)
plays bots through rooms and bosses on every floor and holds the numbers to
their targets: a deep room takes about twenty-five seconds, a deep boss
forty-five to ninety, most deep fights cost somebody the helmet, most boss
fights cost somebody the clothes, and now and then a knight falls.

- **Held cards add up with limits.** The first copy of a card is worth all of
  it, the second half, the third a quarter, and no more after. Every sum has a
  ceiling: damage +40%, rate +30%, speed +25%, pierce 2, one more arrow, mail
  2, a leech of 3, a volley no oftener than every 5th shot, 2 bounces, 2
  homing, 1 chain, 2 mends, 2 morningstars and a burst of a quarter. A knight's
  health stops at 150 (a little more for the sturdier kits). **Mail** turns
  aside a share of every hit (6, 11 in the hundred), not a fixed piece. Mail
  counts whole pieces: duplicate half/quarter pieces round down, so copies of
  one shirt give one piece; two different shirts can reach two. Dinadan's
  Iron Hymn fills missing mail within the same two-piece ceiling.
- **Monsters grow.** Past the second floor their health follows the floor, the
  number of knights and the party's strength (what the held cards add up to),
  and a guardian is tougher still; what they deal follows the floor and the
  head count, and a lone knight meets lighter blows. Deep rooms hold more,
  including gunners, now and then an **elite** (larger, two and a half times
  the health, thicker volleys), and the monsters there fire in wider volleys.
  A monster's bullet pattern grows
  thicker by fixed steps: one for floors 3, two for 4 and 5, three for 6,
  one for each knight standing past the first (up to three), and one for an
  elite, never more than five (`patterns::DEPTH_STEP`, `HEAD_STEP`). A
  bigger party meets denser rings, more arms and more drops; health and
  damage are never lowered for it. A guardian's patterns keep their own
  numbers (its floor and the party are already in its health and blows)
  until a full party of four, which adds one step (`BOSS_HEAD_STEP`).
  For a lone knight the first two floors are as they were.
- **A party is never an easier delve.** Monster health is never cut for a
  party; it meets more pressure instead (`together_shooter/press.rs`, the
  head-count tables in `stakes.rs`):

  | Knights | 1 | 2 | 3 | 4 |
  |---|---|---|---|---|
  | What monsters deal, floors 1–2 (`EARLY_PARTY_HIT`) | 100% | 115% | 130% | 145% |
  | What monsters deal, floor 3 down (`PARTY_HIT`) | 60% | 100% | 120% | 135% |
  | More gunners in every room (`EXTRA_PER_KNIGHT`) | 0 | 2 | 4 | 6 |
  | A wave through a door | 2 + floor (floors 1–3) | +2 | +4 | +6 |
  | Extra shot pairs beside each shot of a volley, guardians too (`PARTY_FURY`) | 0 | 1 | 2 | 3 |
  | A wave's cap | 6 | 8 | 10 | 12 |
  | Extra bodies a floor may raise past its provision (`HEAD_PROVISION`) | 0 | 32 | 64 | 96 |

  - **Aggro spreads.** Each monster hunts its own knight (its share of the
    party) and turns on whoever last hurt it. It still sees Phantasm's
    images and a hireling, and any knight within four units of it.
  - **Holds wear off.** A monster held still for two seconds in a row
    shrugs off any long hold (a stun, a Grand Chord, a Stillhour) for a
    second and a half after it is free. A hit's jolt still lands.
  - **A bomb clears a circle.** The monsters' shots within nine units of
    the knight who threw it, not the whole room.
  - **Rising costs more.** A fallen knight rises at the stairs with a
    quarter of their health, not half.
  - **One bond a room**, for the party, not one each.
  - **No vigil mid-fight.** A knight wakes from one any time, but kneels to
    it only when no fight is on.
- **Healing is cut by the moment.** In a fight a mend lands at a half or less,
  at a guardian at a third or less: a draught 50%/40%, a censer or a linnet
  35%/20%, the mends that fire on their own (a kill's leech, a friend's blow, a
  vigil, a rune) 30%/15% and no more than one a second. Draughts one on
  another do less, a second ward soon after the first holds a third as long,
  and a Sanctuary or the stairs mend only a little. Potions are rarer.
- **One blow lands in full on three.** Every blow that reaches more than one
  monster lands whole on the three nearest where it struck and a quarter on
  each beyond: a sword's or axes' sweep, each sweep of the axe spin, the
  morningstars, a ball's or a burst card's burst, a firebomb, a mine, a gas
  flash, the poison and burning ground of gas and fire, the Grand Chord, a
  Trebuchet shell, and a shot that passes through (counting monsters in the
  order it meets them). Its stuns, pulls and parries still reach them all.
  In an ordinary fight almost every blow (96 in the hundred) reaches three
  or fewer, so only a crowd notices. Morningstars strike on a beat, three
  times a second, for 12 a strike (about half what they did when they struck
  on every tick they touched). A bomb and a Thunder Scroll still reach the
  whole room: each is spent when used and hits for a fixed sum, whatever the
  knight holds.
- **Lifesteal is a trickle.** A kill's leech, a blow's mend, the Red Ballad
  and the marrow sip together give a knight at most **2 health a second**,
  on every floor; the kills or hits of one moment mend once, however many
  one blow took; and the Red Ballad never lifts a leech past the cap of 3.
  A knight who stands beside a boss and only swings goes down.

## Power runes

About a third of fights well up a rune a few seconds in,
somewhere open in the room. Motes gather first; then a small glowing orb
hovers over a ring on the floor, and the first knight to reach it has it. The
Herald calls each one by name the moment it is taken, as a ringside
announcer would. A carried rune's motes circle its knight's feet, and the
sidebar counts it down.

| Rune | Orb | What it does |
|---|---|---|
| Haste | rust, a bolt | Half again as fast, for twelve seconds |
| Double Damage | blue, crossed blades | Every hit twice as hard, for fifteen seconds |
| Regeneration | green, a cross | Ten health a second until whole, for up to fifteen; a hit ends it |
| Arcane | teal, a star | For fifteen seconds the ultimate fills in about eleven seconds of fighting, and spells and the roll come back twice as quick |
| Invisibility | ghost-grey, an eye | Monsters cannot see the knight for ten seconds (it is drawn half there) |
| Illusion | brass, four squares | Two images of the knight at once, as Phantasm makes |
| Bounty | gold, a gem | Gold for every knight standing: 15, and 10 more a floor down |
| Wisdom | parchment, an open book | Experience for the party, toward Sir Ector's lessons: 60, and 20 more a floor down |

Monsters with no one in sight (everyone fallen, stone, or invisible) hold
their aimed shots; rings and wandering go on. Four runes in one delve is an
achievement. Leaving a room leaves its rune behind.

## Vigil

**G** is for stepping away. Your knight turns to stone where it stands:
nothing can hurt it, it travels with the party, and living knights within
seven steps mend three health a second inside its ring of light. The vigil
holds with no keys at all — through Esc, the composer, and a dropped
connection — until **G** wakes the knight where it stood. It is kept between
fights: **G** does nothing while a fight is on (waking always works), and the
key rests five seconds after waking. A statue fights nothing:
monsters still count it in the party's size.

## Hallways

Each floor has up to three stone passages between its chambers. A passage
runs from each doorway to a small hub. It may hold a bat or two, and
sometimes a spike plate laid across the way.

## Monster doors and traps

- **Finite camps on new standard floors.** Terrain/resource support funds a
  camp's initial troops and a small one-shot reserve. A chief's death cuts
  future support; returning never replenishes spent troop slots. These camps
  have no timed reinforcement waves. Extra bodies (summons, slime descendants,
  incidental goblins/mimics and any indirect emissions, including in the Pit)
  share a separate floor provision funded by resource support. Only successful
  bodies spend it; deaths, wipes and reentry never refill it. Required encounters
  and surviving roster slots are not charged again. Traps and ceilings still
  work normally.
- **Legacy and special-mode waves.** Floors without the new political state
  keep their original waves: fight rooms hold back one per floor of depth,
  and guardian rooms hold an escort wave. Each comes through a doorway that
  burns for a second first, never near a knight. The room stays sealed until
  the last wave is down, and a thinned room calls the next wave early.
- **The Crypt's spike plates** rattle, then strike knights and walking
  monsters alike. A dash passes through untouched.
- **The Mines' loose ceiling** sheds rocks near the knights. A red ring closes
  where each one will land. A landing rock crushes monsters too, so lure them
  under it.
- **Dragon Keep's wall vents** glow, then breathe a line of fire across the
  room. They never fire across a doorway's lane.
- **Room perils.** Rooms hold dangers of their own, each a slow cycle with a
  warning you can read, and each hurts monsters as gladly as knights, so lure
  the company into them. A **fire jet** hisses and spits sparks for a second,
  then roars with a column of flame. A **gas seep** bubbles, then breathes out a
  cloud (the same bad air the Gas skill throws, and flame flashes it too). An
  **ember crack** glows along a seam of the floor, then bursts into a line of
  fire. A **crumbling floor** sheds dust and shows hairline cracks, then falls
  away under whoever stands on it. The first two floors keep to the gentlest:
  about one fight room in three holds one of them (one at most); the deeper
  floors hold more of them and more kinds (up to three a room), and the
  hotter packs add fire. They never lie at a doorway. Their numbers are the
  constants at the top of `together_shooter/perils.rs`.
- **Mimics.** About one treasure chest in four has teeth. It hops, bites and
  spits coins, then coughs up the treasure it was pretending to be.

## The deep

Slaying the dragon no longer ends the delve. Its floor is reclaimed and its
hoard banked whole on the spot, and the lair opens two ways:

- **the light home:** step into it and the delve is won, with everything
  carried coming home whole;
- **the stairs down,** into the deep below Dragon Keep, where the spoils are
  richer and the way back is only by winning or falling.

| Floor | Delve | What lives there | Guardian |
|---|---|---|---|
| 4 | The Drowned Archive | Rows of shelves, flooded aisles, pages underfoot; wraiths, slimes, sappers, a necromancer | **The Index**, a librarian who read until it became the catalogue: hovering, a spiral of pages, thrown tomes; or **the Late-Fee Leviathan**: chasing, and its Ravage throws the floor up in rings |
| 5 | The Fungal Deep | A forest of mushrooms, moss, glowing spore pools; slimes, hobs, warboars, a shaman | **The Mother of Spores**, rooted and in no hurry: spore rings, thorn fans, burning spores; or **The Big Ripper**, a swollen bog lord carving gas lanes through the room |
| 6 | The Unknown | Eye-pillars on a lattice of faint light; echoes of everything met on the way down | **The Shoggoth's threshold** (below) |

The deep's native rosters include worse company. New standard floors also
vary density by their resource ecology: the Fungal Deep's brood-supported
hives have more lesser troops but fewer leaders than the rival mining clans.

The deep sheds its kin's materials (the Archive the Crypt's bone and wax,
the Fungal Deep the Mines' ore and gems, the Unknown the Keep's embers).
Wren's map table can draw landings at floors 4 and 5 once the party has
reached them; they cost a dragon's scale.

At the bottom of the Unknown the Shoggoth fills the north of its hall:
vast, many-eyed, curious, not an enemy. On new standard floors its echo-brood
prerequisite must fall before the threshold encounter appears; that encounter
has a finite escort. Legacy floors retain their four returning waves. Clearing
the unlocked threshold wins **the Grail**: every knight standing carries home
three gems and a scale, and the realm counts the Grail.

## Secret rooms

The old dungeons kept them, and so does the Delve: on a little under half the
floors, one fight room has a **cracked wall**. Lit fissures run across two of
its stones, rubble lies at its foot, and a draught breathes pale dust out of
it. The Herald notices the first time you walk in ("Nothing a bomb couldn't
fix, anyway"), and Wren feels the draught.

A knight's **bomb** anywhere in that room brings the wall down; so does a
sapper's keg or a hob's bomb going off beside the crack. Behind it is a vault
no map shows (Pip's maps excepted): a chest in its middle, gold heaped round
it, nobody guarding it. The Herald did not know it was there, and checks the
plans. There are no plans. The first one found is an achievement.

About one vault in three keeps **Snibbet** instead: a retired loot goblin
sitting on his sack. Walk up to him and he heaps eight piles of gold at your
feet: "It's a secret to everybody." Once. The Herald saw that.

## The Pit

On about half the floors from the second to the
fifth, a dead end off a fight room opens on the Pit (the floor map marks it
☠), and the **Pit Tyrant** sleeps in it. Nobody has to go in, and once in,
the doors bar like any fight's.

He is huge, slow and patient: a fan of embers every few seconds, and a
slam. A red ring gathers round him for a second, filling in as it comes;
anyone still inside when it lands is struck hard and thrown clear. Below a
third of his health he slams more often.

Felled, he leaves a ring of gold, a prize, and **the Talisman**. The knight who
picks it up is mended whole, and the next time they fall they rise again at
once, whole, before any of the Chapel's second winds is spent. Felling him
is **Into the Pit**.

By bot, with a well-built Undercroft, the Pit is won about one fight in
three alone and two in three with a friend.

## The second company

New monsters join the old ones, each with something to read and something
to do about it. The first floor meets only the gentlest of them (slimes in
the Crypt, sappers in the Mines); the rest come in from the second floor
down, and a room never holds two of a dangerous kind: one necromancer,
one shaman, one warboar, one slime family at a time. A wave that would
bring a second brings the floor's plainest monster instead.

| Monster | Where | What it does | What to do |
|---|---|---|---|
| Sapper | Mines | Creeps close, plants a keg, runs. The keg's fuse blinks, then turns red: armed. It blows on its own, or when a knight steps by | Shoot the keg: it goes off early and hurts monsters as gladly as knights |
| Slime | Crypt | Hops. Big ones split into two, and those into two | Clear the pieces before they surround you |
| Necromancer | Crypt | Keeps its distance, spits orbs, and raises two skeletons every few seconds, three times; then it has no bones left | Kill it first |
| Warboar | Mines, Keep | Paws the ground, marks a red line, then charges down it and throws aside whoever it hits | Step off the line; a boar that hits a wall stands dazed: punish it |
| Hob | Mines, Keep | Lobs bombs at where you stand; a red ring marks where it will land | Keep moving |
| Shaman | Keep | Boxes the nearest knight in with four serpent wards that spit, then crumble; between wards calls crossfire from the side walls | Step out of the box, or break a ward; step between the rows |
| Hexer | Crypt (floor 2 down), the Fungal Deep, the Unknown | Keeps her distance, spits, and every four seconds looses a slow green bolt. A knight it touches is a frog in a tiny helmet for two and a half seconds: it hops at seven-tenths pace and can do nothing else. Then no hex takes for three seconds | Roll through the bolt, raise a shield to it, or let a friend cover the frog |
| Loot goblin | about one fight room in three, more on the deep floors, now and then two | Takes loot off the floor, then a share of a knight's gold, gems, ember or ore or a card from their hand on a touch (THIEF!), runs for a door and is through it in eight to twelve seconds, loot and all. It does not fight | Catch it: everything it took comes back, with a pile of gold and now and then a prize |

## The deep's hunters

Below Dragon Keep a third company hunts. A deep
room holds up to three dangerous kinds where the floors above hold two.

| Monster | Where | What it does | What to do |
|---|---|---|---|
| The Flesher | the Fungal Deep, the Unknown | Shambles after the party in a cloud of rot that wears at whoever stands close (it never fells a knight on its own). Every few seconds he stops and stares down a line, marked in red dashes on the floor, then throws his meat hook down it. A knight it catches is reeled in to his cleaver | Step off the line, or roll through the hook |
| The Silkmother | the Drowned Archive, the Fungal Deep | Keeps her distance, spins webs under the knights (a knight in a web walks at a little over half pace), and hatches spiderlings, up to six at a time | Get out of the web; reach her through the brood |
| Spiderling | wherever the Silkmother is | Small, quick, and bites | Sweep them up |
| The Lich | the Drowned Archive, the Unknown | Keeps his distance and throws cold bolts. Every six seconds, Rimeleap: a slow pale orb that chills whoever it strikes (a chilled knight walks at six-tenths pace for a second and a half) and leaps from the knight it strikes to the nearest other knight within ten steps, five times, never straight back. Alone, you take it once | Spread out; a raised shield ends the chain |
| The Hollow One | the Unknown | Stones floating round a void. It keeps its distance and throws cold bolts, and every eleven seconds opens a Black Hole where a knight stands. A ring warns for a second; then for three it drags every knight within seven steps toward its middle, and its core burns. It holds still while it keeps the hole open | Walk out (a knight beats the pull), or hit it hard while it channels (120 damage breaks the hole shut, and earns Event Horizon) |

## Monster patterns

The monsters' shots come from a small vocabulary of bullet patterns
(`crates/delve/src/rules/patterns.rs`). Every pattern is a set of
fixed numbers and throws the same shots from the same seed. Shots stay
slower than a knight (most fly 4 to 9 units a second, none faster than 10,
against a knight's 13), and
everything that strikes from somewhere unexpected is marked first: a laser
shows a dashed line for 0.7 s before its 0.3 s burn. A sweep warns for the
same 0.7 s, showing the whole arc, then crosses it during a 0.8 s burn.
Rain warns for 0.8 s or longer, crossfire for 0.6 s: a blinking diamond and
an arrow mark where each shot will come from.

| Pattern | Shape | Thrown by |
|---|---|---|
| `burst` | three bones in a line at you, one after another | Crypt skeletons |
| `lead` | a fan aimed where you are going, not where you are | Mines imps, the Mimic, Dragon Keep demons, the Pit Tyrant |
| `gap_ring` | a full ring with one wide gap (some 71 degrees) to slip through, near your line | Crypt wraiths, Ringers, Unknown wraiths and demons |
| `flower` | petals of three, the flower turned a little each volley, petals leaning in flight | elite wraiths, elite demons |
| `twin_spiral` | two pairs of spiral arms turning against each other | Dragon Keep demons, the Dragon, Oathbound |
| `weave` | a stream at you, its shots weaving either side of the line | Archive wraiths, elite Archive skeletons |
| `curve` | a fan whose outer shots hook back in across your line | Archive skeletons, Unknown imps |
| `surge` | a ring that leaves slow and quickens to 9 | Dragon Keep imps, Unknown demons, the Pit Tyrant |
| `reaim` | a ring that slows to a stop three steps out, hangs, then flies at the nearest knight | Crypt necromancers, Dragon Keep wraiths |
| `split` | a slow heavy shot, ringed, that bursts into six | Fungal imps and spitters |
| `bounce` | a fan that comes off the first wall it meets | Mines skeletons, the Mimic |
| `laser` | a dashed warning line, then a beam down it | Unknown wraiths, the Dragon |
| `sweep` | a warning line, then a beam that sweeps a 90-degree arc | demons, the Dragon, the Hollow One |
| `mines` | shots that slow to a stop, hang blinking, and pop into rings of six | Silkmothers (egg sacs), Fungal slimes and elite imps, Unknown necromancers |
| `gap_wall` | a wall of shots across your line with one gap, a second wall behind it with the gap moved | Pavises, the Pit Tyrant |
| `crossfire` | rows of shots from the left and right walls at your row, marked first | Shamans |
| `rain` | drops from the top of the room, marked first | elite demons, the Dragon |
| `seeker` | slow orbs that turn after the nearest knight, and fade | Archive necromancers |

Each kind walks its own chain, one pattern a volley, chosen by its floor, so
the floors throw differently: the Crypt slow ghost rings with gaps and orbs
that stop and come again; the Mines shotgun leads, ricocheting ore and
mines; Dragon Keep spirals, quickening rings, sweeping fire and rain; the
Drowned Archive torn pages that curve and weave and seek; the Fungal Deep
spores that hang and burst; the Unknown eyes, lasers and mixtures of all of
it. Elites throw denser volleys, and several kinds walk longer chains.
Each delve's shots have their own look:
the Crypt's pale wisps, the Mines' ore, Dragon Keep's flames, the Archive's
torn pages, the Fungal Deep's spores, the Unknown's eyes. Monsters whose
threat is something else (bats, boars, hobs, sappers, hunters, the Hexer,
the Lich, the swarm) keep it.

Pattern shots are held to the room's cap of 384 shots like every other, and
a beam strikes each knight and hireling once. A raised shield facing a beam
blocks its hit. Beams stop at stone; bouncing ore reflects once, then falls
on its next wall. Seekers turn gently at 1.1 radians a second and expire
after five seconds. Splitting shots pop after 27 ticks into six shots at
4.5 units a second; mines stop, hang, and pop after 51 ticks into six at 4.
Re-aiming rings stop before their 45-tick fuse, then launch at 7.5. These
motion traits default to empty when loading an older shot.

Review pictures are written without a live game or any API calls:

```sh
CARGO_TARGET_DIR=/work/build-cache/angelx-patterns TMPDIR=/work/tmp \
  ANGEL_ARENA_SHOTS=/work/tmp/review-shots/patterns \
  cargo test --manifest-path cockpit/Cargo.toml --bin angel write_pattern_shots -- --ignored
```

This writes one mid-volley PNG for each of the 18 patterns, warning PNGs
for lasers, sweeps, rain and crossfire, a stopped re-aim ring, and four
successive boss-phase PNGs. The browser guest receives this same arena
raster, including beam lines and warning marks.

## Bosses

### Floor ecology: chiefs, resources and access

New **standard generated floors** have a bounded economy/politics encounter
slice, not a fixed two-warden checklist. The existing **six-floor limit** and
native terrain remain. Interior pools, blocks and usable floor area contribute
to a site's resource capacity; pack, depth and seed determine its support and
placement. Resource changes actually change troop budgets, supported leader
counts and, for a hive, whether a rare apex can exist.

| Structure | Packs / support | Leaders and route |
|---|---|---|
| **Hive** | Fungal brood beds; Unknown echo brood | One required brood leader, with an optional second at high support; many lesser troops. Rich Fungal profiles can support a rare apex (one of four seed salts), unlocked by the core brood's death. The Unknown preserves its native Grail threshold encounter. |
| **Rival clans** | Mines ore seams; Archive salvage stacks | Three or four independent checked chiefs. **ANY** one chief opens an alternative route; surviving chiefs need not all die. Productive contested camps can contain a rival's troops. A standard clan floor has no mandated sole exit boss. |
| **Tribute stronghold** | Crypt tomb tribute; Hellforge fuel | Two or three gate-house chiefs protect an apex through **ALL** prerequisites. When support permits, another independent captain is optional, outside the gate pact. Dragon Keep still has its native Dragon, not a replacement catalog boss. |

All chiefs are real checked `.boss` encounters with boss health bars, attacks
and drops, including the Mines' **Iron Tusk Chief** and **Red Pick Chief**.
They command productive fight rooms reachable from entry **without using the
sealed apex room as transit**. Ordinary kills or visiting a chief do not count
as political deaths. The optional Pit Tyrant is separate from both the route
and economy.

The map marks a visible living chief **`C`**, not `W`. Status/guest notices
show Hive / Rival clans / Tribute stronghold, **ANY** or **ALL**, route progress,
checked encounter names and faction claims. A separate support line identifies
the resource, remaining local support/reserve and how many factions lost
control. Home's rumour board forecasts the supported chiefs. Native guests
receive the same mirror/state; a browser seat gets the same picture as its
own camera and the same table. No client can submit a political transition or
defeat assertion.

A camp provisions **once**. Its base budgets are 8–20 lesser troops for a hive,
3–9 for clans, and 3–6 for a stronghold, plus at most two reserve troops on
first entry. Reentry preserves living slots, not dead ones. A chief's actual
death removes control once, cuts the budgets of its undeployed camps, and cuts
future apex support in hives/strongholds. In clans only, the next living rival
inherits at most three unused reserve units, capped at nine. Deployed escorts
do not disappear when their leader dies. An interrupted fight can therefore
resume without respawning its fallen boss or awarding its drops again.

This is **rival composition and finite aftermath**, not NPC-on-NPC gang-war
combat, expanding territories or a permanent world simulation. There are no
new economy timers, off-floor simulation or LLM/paid-token calls. Active
ecological rooms are capped at **96 enemies**, including summoned/splitting
children; the serialized catalog remains at most **64** entries (below the
`u8` index limit). Hive rosters allow at most one splitting slime per camp.

Extra bodies share **min(128, 8 + 4 × floor resource support)** provision per
floor, separate from the finite camp slots. Each successful summon, slime
half, incidental goblin/mimic or indirect emitted body costs one; blocked tiles,
invalid requests and full live caps cost nothing. A bounded identity journal
persists provision/spent and retired bodies across saves, wipes and reentry;
corpse replay cannot produce more drops or descendants. Abandoned/escaped bodies
are retired without refund. When provision runs out producers keep their other
attacks, slime splitting stops, and a mimic attempt leaves its chest usable;
roster troops and required/native bosses still spawn and can be completed.
These provisions apply only to gated standard floors, including the optional
Pit; legacy ungated floors and special-mode spawns/waves retain their behavior.

Approaching a sealed apex is safe: its boss is **absent**, so bombs and AoE
cannot hurt or credit it. The room gives no clear, chest, light, victory or
stairs, and doors allow retreat. Once prerequisites fall, return for the
unlocked encounter. A clan route can have a finite exit escort without a sole
boss. Starving a hive reduces future support but does **not** add a resource
threshold to its unlock condition; killing the brood prerequisite always
opens the route.

A legacy checkpoint with no `boss_gates` field keeps its current floor's old
population active. Its **next** standard floor generates the new finite state.
Gated saves and native mirror deltas serialize encounter identity, support,
deployments, spent troop slots, provision/spent, emission identities and actual
leader deaths. Admission reconstructs
genesis and replays the bounded journal, rejecting invalid changes atomically.
The superseded two-warden experimental graph schema and unintegrated pre-repair
economies lacking the required provision fields are not migrated. Older ungated
saves remain unchanged.

**Mode limits:** Gauntlet and Hold the Stair deliberately keep their existing
short-route/horde encounters without economic gates. Home and layouts with
insufficient reciprocal prerequisite sites also use the legacy path. Other
standard-layout modes use the supported profiles. Empty catalogs are filled
with distinct checked native definitions as needed; custom pack bosses remain
eligible encounters. This does not grow the classic dungeon or change special
modes into an infinite world.

### Mini-bosses and the Boss Key

Every floor's route chiefs are its **mini-bosses**: each in a room of its own,
with a name over its health bar. The chief whose fall opens the route drops
the **Boss Key**, a gold key that lies where it fell (a chief still holding
it shows a key beside its name). Walk over it to carry it; the carrier's HUD
says so. If that knight falls, the key drops where they went down; if they
leave the party, where they last stood. The boss room's door, in the room
beside it, is shut and chained (a padlock on the gate) until the knight with
the key stands at it; the key is spent opening it, and the door stays open.
Without the key the party is turned back at the door, with the Herald's
reminder. Floors without the political state (Gauntlet, Hold the Stair) have
no boss door to lock, and a run saved before keys existed keeps its door open.

### The Wanted hall

Every standard floor has one more room, hung off it like the Pit: the **Wanted
hall**, a side-on showdown with an outlaw of the pack's own roster (not that
floor's chiefs or apex, if the pack has another). It is the floor's showdown
for those who go looking, off the road to the stairs, marked on the map with a
star (hollow once it is done). Walk in and the **WANTED poster** goes up: the
outlaw's likeness, its name, and the reward in gold, while the knights stand
untouched. Then it moves between its posts: the ground, either balcony, the
beam over the middle, and the cover of a crate. It takes its shots on the
beats its own file gives it, each preceded by a **tell** (a flash at the
muzzle and a dotted sight to the knight it has marked), calls minions down on
ropes (a shadow marks where each lands), and taunts when it has spent itself,
the moment to strike. Its rage and its last stand are the ordinary ones. Run,
jump (twice in the air), drop through planks with **S**, and crouch under its
fire on the ground with **S**.

When it falls it falls hard: it flashes, tips and comes apart, its company runs,
the coins fly, the **bounty** is counted at the foot of the screen and paid
(the score, and two gold in each standing knight's pack), and its own spoils
drop as a guardian's do. The hall stays quiet after. The bounty is
`400 + 300 a floor + a quarter of its health`, to the nearest fifty.
The hall's numbers are the constants at the top of `together_shooter/outlaw.rs`.

### Checked encounter definitions

Supported chiefs/apices keep their normal combat and scatter spoils as cards.
The Crypt's **Waxen Warden** and
the Mines' **Cinderjaw** were designed and drawn by Muse
(`muse-spark-1.3-contributor`) from a brief. Muse wrote the stats, the attack
patterns, the lore and the pixel art; its first draft had uneven art rows,
and it fixed them itself in a second pass after seeing its render. Dragon Keep
keeps the dragon.

The first two floors, the ones a realm plays most, each have a second
guardian: in the Crypt **the Bone Choir**, three skulls on
one spine singing in a key that hurts (rings of bone, aimed orbs), and in
the Mines **the Foreman**, a goblin overseer with a lamp on his helmet and a
whip in each hand (fans of embers, aimed bones). On new standard floors these
checked definitions can lead supported factions; tribute floors also protect
a separate apex. Legacy/special-mode floors keep their guardian choice.
A workspace's own `.boss` for the same pack stays eligible. Checked bosses
retain their normal plinths in the Trophy Hall.

| Boss | Floor / ecology | Fight |
|---|---|---|
| **The Big Ripper** | Fungal Deep chief, or a supported rare hive apex; legacy guardian option | Three great breaths pull gently and swell his belly and hitbox. A flashing arrow gives 0.7 seconds before a 40-unit/s wall-bouncing rip, leaving green-brown poison walls for seven seconds. Walk against the breath or roll clear; fire flashes the gas off and hurts him inside. Walls occupy at most 160 square units, about one eighth of a normal room. He lands dazed: strikes from behind deal half again as much. Below 40% health he chains three individually warned rips and leaves wider walls. |
| **Grinwick, the Last Service** | Mines checked clan chief; legacy guardian option | A soot-black spider engine with a cracked porcelain grin, red lantern eyes and brass knees. Skitters, spits coal from its funnel, blows brief steam veils and stamps shockwave rings. Three rails cross the floor; the active rails light up and a scream-whistle gives a full second before a charge. Step off the sleepers. Below half health four spider coal tenders uncouple, using the floor's finite summon provision, and he jumps between three separately warned charges. |

Both have 900 base health before the ordinary floor, guardian and party
scaling. Full parties thicken their pattern sequences through the existing
head-count pressure rules. The Herald keeps individual pages for them beside
the ordinary monster entries, counting each guardian felled.

The Drowned Archive has a second guardian too:
**the Late-Fee Leviathan**, a barnacled sea-thing with a ship's anchor that
came up with the flood to collect everything the Archive ever lent. It
chases, throws water and barnacles, and every seven seconds (sooner below a
third of its health) it **Ravages**: ripples run out round it in five rings,
one lane left still, and then the rings burst outward one after another into
tentacles. A knight on a ring as it bursts takes 16 and is thrown into the
air, helpless for half a second. The rings overlap, so there is no standing
between them: step into the still lane, roll as your ring comes, or be
farther off than the last ring reaches. A Pendragon Sceptre does not help.

A boss is a checked `.boss` file in `cockpit/assets/dungeon/bosses/`. It has
a movement (chase, drift, hover or anchor), up to three attacks (aimed, fan,
ring, spiral, ravage, or any monster pattern by name, e.g. `attack gap_ring
every=90 damage=12`), a rage threshold, spoils it drops, and art up to 32
by 32 inks. A `phases` line chains two to four patterns, walked in turn a
few volleys each (`phases gap_ring sweep rain twin_spiral every=45
volleys=2 damage=12`); every built-in guardian and chief has one. A chain
can stand alone or accompany attacks. It starts with the first named
pattern; rage and desperation speed its beat without skipping or repeating
phases. Its volley counter is saved with the monster. `shot=`
picks a look (bone, orb, ember, wisp, ore, flame, page, spore, eye); a named
pattern defaults to its delve's own. A name the vocabulary lacks is
rejected. The whole fight is held under 14 hostile shots a second (a
phases line counts its average volley); a ravage throws up the floor, not
shots, so it is not counted.

Named patterns keep their fixed counts, speeds and motion; `shots=`,
`speed=` and `arc=` on such an attack do not override them. For `phases`,
`every=` accepts 20–150 ticks, `volleys=` 1–6 per pattern and `damage=`
8–20. Unknown attack, phase and shot names are rejected, as are duplicate
chains and chains shorter than two or longer than four patterns. The
checker may lengthen beats to keep the shared bullet budget.

### A guardian near its end

A boss (every guardian, the dragon, the Pit Lord) under a third of its
health turns desperate. It flashes, slowly at first and faster and faster
toward the end; its attacks come up to a third sooner; and it begins a last
stand on a five-second cycle, each part shown on the boss a second before it
lands. A **ring** of embers closes round the party with one lane left open
toward the nearest knight, marked in gold: stand in the lane. Under 18 in the hundred
of its health, **three spiralling arms** of orbs follow it, shown as spokes
before they turn.

## Cards

Every pickup is a card, and a card is a short text file anyone can write:
`/dungeon cards new <name>`, edit `.angel/dungeon/cards/<name>.card`, then
`/dungeon cards reload`. **Tab** opens the card screen; **Z** quaffs the first
card from your hand, then the rest shift left.

Every knight also has **four reusable spell slots**, shown above the hand.
**1 / 2 / 3 / 4** cast them; a couch friend uses **5 / 6 / 7 / backslash**.
Spells are never consumed: each slot recharges independently for the card's `cooldown <seconds>`
(whole seconds, 1–120). They reuse **nova, ward, heal and bombs**. The new
`cast bolt|ring|meteor` effect launches a straight bolt, a twelve-bolt ring, or
a heavy bursting meteor from the knight, carrying the knight's own shot traits.

Write a `kind spell` card into workspace `.angel/dungeon/cards/` while playing:
it hot-loads within half a second into the host's **first empty spell slot**,
with a sound, notice and burst flourish. Full slots leave it as a pickup at the
host's feet. Partial/invalid files are retried; unchanged files never duplicate
spells, and editing an equipped spell preserves its recharge. Solo pauses also
pause cooldowns, but not the file scan. Friends see spell slots, timers, casts
and card-rule edits through the mirror, in their own cockpit.
Three of the delve's own play cards are items, rare drops and chest
prizes, each with an effect word a card file can use too:

| Card | Effect word | Does |
|---|---|---|
| Fae Dagger | `blink N` (4–14) | You are N steps along your aim at once; walls stop it |
| Pendragon Sceptre | `immune N` (2–8) | N seconds that no hex, chill, web or hook takes (a gold ring turns round you) |
| Censer | `censer N` (10–80) | A green pulse mends every knight within eight steps by N |

The format, kinds, effects, caps and inks are in
[`cockpit/assets/dungeon/cards/README.md`](../cockpit/assets/dungeon/cards/README.md).
Arm cards may carry Forge runes, checked against the same tier-I caps below.

Cards can also draw what they put on the field. A `draw <slot>` block holds
ink rows; `.` is transparent and `|` separates 1–4 frames on every row. Frames
advance every six ticks (five frames a second). End a block with a blank line.

| Slot | Maximum frame | Where it appears |
|---|---|---|
| `worn` | 24×24 | Over the knight's 16×16 body, including an uploaded avatar; use for shields, armour, pauldrons and helms |
| `shot` | 12×12 | Plain/forged shots and spell projectiles, drawn at twice size |
| `orbit` | 12×12 | Replaces the morningstar when a held card grants `orbit` |
| `summon` (or `foe`) | 16×16 | The creatures called by a `summon` effect |
| `aura` | 32×24 | A glow under the knight's feet |

Worn and aura looks belong to hold, arm or guard cards; shot looks belong to
hold, arm or spell cards. Up to three worn layers draw oldest under newest.
A small worn piece may use `draw worn at=left` for the shield arm, or `head`,
`shoulders`, `body`, `right`, `feet`. Its centre is placed on a 24×24 canvas;
the body remains underneath. Gear wishes use **worn**, never an orbit as a
stand-in. A hold or spell card can also use the Forge's `look <colour> <shape>`.
The newest held shot look wins; a spell's own look overrides it for that cast.

Instead of rows, put a PNG horizontal strip in `.angel/dungeon/sheets/`:

```text
name Golden Devils
kind spell
cooldown 16
summon 4 swarm
sheet summon golden-devils.png frames=2 size=12
```

`sheet <slot> <file.png> [frames=N] [size=N] [at=place]` cuts equal-width cells
into a checked sprite. Use a plain filename, a flat background or transparency,
1–4 frames, at most 2 MiB and 2048 pixels per side. `frames=` is recommended;
omitting it estimates the count from the strip's width/height. Width must divide
evenly into the count. `size=` sets the normalized frame size within the slot's
limit; `at=` places a worn piece. The loader shares the avatar background remover
and 16-colour reduction, crops every frame with one common box to keep animation
steady, and maps pixels to realm inks. PNG references become checked ink rows
before sharing: saves and friends carry the frames, with no local file dependency.
A missing or malformed sheet rejects that wish and reports why.

`kind spell` and `kind play` can use `summon N <behaviour>`:

| Behaviour | Count cap | Fixed combat behaviour | Minimum spell cooldown |
|---|---|---|---|
| `swarm` | 6 | Hunts nearby monsters and bursts once: 30 base damage, radius 1.8, expires after 6s | 4s per creature |
| `guard` | 4 | Circles its knight; each catches one hostile shot, expires after 12s | 5s per creature |
| `turret` | 2 | Stands and fires 14-base-damage bolts every 24 ticks, range 12, expires after 8s | 10s per creature |

The checker bounds the count and raises a spell's cooldown to that floor. Stats
come from the fixed menu. A knight keeps at most six creatures; a new room clears
them. Swarm bursts use the usual three-full-target area rule, extra targets take
25%, and kills use the existing leech cap. Turrets share the projectile ceiling.
Looks change no combat numbers. Card text remains limited to 4 KiB; sprites may
bring the whole file to 24 KiB.

## Your AI makes data, not executable game code

Use `/dungeon forge` for the weapon instructions. Give them to your AI, save its
answer in a workspace file, then `/dungeon forge path/to/weapon.runes` to equip it.
`/dungeon avatar` gives image instructions; `/dungeon avatar path/to/knight.png`
checks and equips an image. The game does not call an image model or obtain API
keys. Host files must be regular files inside the current workspace.

Runes have a fixed vocabulary. Unknown behavior is rejected; numeric values are
clamped with feedback. Tier I has 6 LOC, a **70 DPS** ceiling and **100 damage**
volley-plus-swing ceiling. Name and look are free; each behavior costs 1 LOC.

| Rune | Bounds |
|---|---|
| `name <text>` | Up to 32 characters |
| `look <colour> <shape>` | ember/ice/gold/venom/void/rose/steel; orb/shard/arrow/star/blade/wave |
| `bolt damage=D speed=S every=E range=R` | D 4–100; S 8–20; E 15–60 ticks; R 6–48 |
| `spread shots=N arc=A` | N 2–3; A 5–90 degrees; requires bolt |
| `melee damage=D reach=R arc=A every=E` | D 10–60; R 1–4; A 30–360 degrees; E 8–45 ticks |
| `throw damage=D range=R every=E` | D 8–50; R 4–14; E 15–60 ticks; a blade that flies out and comes back, cutting both ways; not with `bolt` |

30 ticks = one second. Paper DPS counts all spread shots as hits. Hybrids swing
when a monster is in reach, otherwise shoot (or throw). Every swing cuts monsters'
shots out of the air in its arc, and a knight with a blade and no bow takes a
third less damage: melee has to stand in it. A swing applies once, respects its
cooldown, and has a visible arc. Forged projectiles carry their checked colour
and shape, or a card's `draw shot` frames. Picking up another ranged arm replaces a forged weapon.

The delve's ordinary arms are the bow's stream **24 every 10 ticks at 40 u/s**,
the crossbow's rail **55 every 23 ticks at 70 u/s, through everything**, and
the handgonne's fan **five balls of 14 every 25 ticks at 34 u/s** (see "Arms,
and how they feel"). A forged bolt flies at least 36 u/s, whatever its
`speed` says. Example runes:

```text
name Bow
look gold arrow
bolt damage=12 speed=40 every=5 range=48
```

```text
name Knight's Blade
look steel blade
melee damage=30 reach=3.5 arc=150 every=16
throw damage=18 range=9 every=40
```

At a Sanctuary, **T** can reforge an offensive or defensive kit piece. The
brief asks for its play-field look as well as its checked rules: offense can draw
shots and a weapon on the right hand; defense draws a shield, helm or armour
as a worn overlay. It may use ink frames or a PNG sheet in the same sheets folder.

## Wishes

Win a room and an **Overclass window** opens: each knight standing gets one wish
until the next fight. A Sanctuary opens one too. **T** opens the Scroll of One
Wish. Pick a known wish with **↑/↓**, or say it in your own words ("triple wide
fire my shots", "fire faster", "heal my friend when I hit"). The ONE WISH box reads
it back; sealed, a known wish is yours at once, in the running game, and every
friend's screen shows it with the next tick. The same wish granted again climbs
its ladder: Triple Volley goes from every 8th shot three wide, to every 6th, to
every 5th. Fully granted ladders are skipped. Known phrases must cover the
wish's content words (all, or all but one when there are at least four):
"golden spiky armor that explodes" and "bigger spiked shield" go to learning.
A matching learned entry wins a tie with a stock phrase.

Known wishes live in the **phrasebook**, plain data anyone can grow:
`cockpit/assets/dungeon/phrasebook.txt` (the delve's own) and
`.angel/dungeon/phrasebook.txt` in your workspace (yours; an entry with the same
id replaces the delve's). The game reads both again within a quarter second of a
change, while it runs.

```text
wish volley | Triple Volley
say triple wide fire my shots | triple shot | fire three | spread my shots
tier volley 8 :: Every 8th shot flies three wide.
tier volley 6 :: Every 6th shot flies three wide.
tier volley 5 :: Every 5th shot flies three wide.
art
<optional icon: up to 16 rows of inks, `.` is black paper, then a blank line>
```

Each `tier` line is card lines (`;` between) and the line the scroll shows.
Wish ids are at most 24 characters, with at most five tiers, so every tier's
card id fits without cutting off its ladder marker.
`kind hold` is the default; `kind spell` offers active casts and summons, every
tier including `cooldown`; `kind guard` offers a guard skill. A `draw` block
applies to all tiers, or add a tier number (`draw worn at=left 3`) to grow the
look from that tier. PNG references may use `tier=3` the same way. For example:

```text
wish golden-devils | Golden Devils
kind spell
say tiny golden devils that explode | golden imps | exploding devils
tier summon 2 swarm; cooldown 8 :: Two golden devils hunt and burst.
tier summon 3 swarm; cooldown 12 :: Three golden devils hunt and burst.
tier summon 4 swarm; cooldown 16 :: Four golden devils hunt and burst.
sheet summon golden-devils.png frames=2
```

Armour and shield ladders use `draw worn`, with `armor` and/or `max_hp` lines.
Iron Skin now shows iron pauldrons, with spikes at the last tier. Stock wish
icons are embedded in the game and appear in shipped binaries.

Effects a held card may carry, with their ranges: `damage`, `rate`, `speed`,
`pierce`, `shots`, `armor`, `vamp`, `max_hp`, and the wish primitives `volley N` (every Nth
shot three wide), `bounce N` (off walls N times), `homing N` (curves toward
monsters), `chain N` (a hit sparks on to N more), `mend N` (each hit mends the
most hurt friend, at most 2 a second), `orbit N` (morningstars spin round you)
and `burst N` (shots burst, N% to the three nearest). The card checker bounds
every tier, including the deck cap: armor 2, chain 1, orbit 2, burst 25%,
volley no faster than every fifth shot. A knight's held cards add up with limits
(see "How hard the delve pushes back"): each further copy of a card is worth
half the one before, and
every sum has a ceiling.

A wish the phrasebook doesn't know is **learned while you play**: angelX gets a
short brief to append one entry to the workspace's phrasebook, the game never
leaves the screen, and the reload grants the newly learned entry to whoever
asked. The brief offers ink frames or a PNG sheet made with the drafting tool's
image tool. Invalid art and sheets report their checker errors back for one
repair turn; other valid entries remain available. Updating a sheet also reloads
its wish. A friend's wish goes to the host's angelX the same way. Workspace
`.card` files hot-load the same
way: write one while the game runs and it joins the book.

## Avatars

Uploads are bounded to 2 MiB and 2048 pixels per side. The checker requires one
readable figure, removes a flat background and stray specks, reduces the palette,
and generates a 32×32 sprite plus 128×128 portrait. Only normalized data is stored;
avatars do not affect combat. The shared arena renderer draws the sprite.

## Vote on 0.2.2

Press **V** on the surface, in the Undercroft or during a delve. **V** or
**Esc** closes your ballot. The other knights keep playing; your game inputs
are ignored while you vote. Up/down chooses a feature, left/right adjusts
its stars, **1–5** sets a rating, **0** clears it, and **N** marks “haven't
tried it.” Changes save immediately. The party average appears after two
players have rated an item; untried marks do not count toward the average.
New seats see `V: VOTE ON WHAT'S NEW` for seven seconds, once per realm.

A couch second knight uses **B** to open or close, **I/K** to choose,
**J/L** to rate, **5–9** for one to five stars, **0** to clear and **U** for
untried. Esc closes an open ballot before returning to the composer. On the
shared terminal, the last local voter to use their ballot gets the panel.

A friend's angelX and browser seats use **V** and the first knight's ballot
keys. Browser gamepads use **L3** to open, d-pad or left stick to choose and
rate, **A** for untried, **X** to clear and **B** to close.

The host saves votes by player name with the realm. `/dungeon votes` reads
each feature's average and rating count. Every change also appends a JSON
line to `~/.angelX/playtest/votes-0.2.2.jsonl`:

```json
{"time":1791820800,"player":"Friend","item":"bow","stars":4}
```

`time` is Unix seconds; `item` is a stable ID. `stars` is 1–5, `"untried"`,
or `null` when cleared. Labels, groups and IDs live together in
`crates/delve/src/rules/votes.rs`.

## Playing with friends

The host types:

```
/dungeon_host --3
```

and gets one link for everyone (up to three friends), with its QR code:

```
One link for everyone: open it in a browser (nothing to install), or paste it into angelX as /dungeon join <link>.
  http://100.x.y.z:8767/#<key>
```

Opened in a browser, the link is a playable seat with nothing to install:
the Delve's own client runs in the page. Pasted into a friend's angelX
(`/dungeon join <link>`), it plays from their terminal. Either way the
friend's client keeps a copy of the host's run (the tree once, then each
tick's changes), draws it itself at the display's rate, and answers its own
keys at once; the host's next tick corrects it. The host's own screen is a
client of the same server. In angelX friends play with the same keys, hear
the same knights, effects and music, make wishes with **T**, and see their
own cards and the host's book with **Tab**. Esc returns them to their
composer; `/dungeon` goes back in; `/dungeon leave` goes home.

On Kitty (or Ghostty, WezTerm) the room is a picture the terminal places one to
one; other terminals get half blocks. macOS Terminal.app works, coarser, and
says so on the status line.

- Everyone opens the same link; each newcomer takes the next free knight. A
  dropped connection gets its own knight back within two minutes.
- A friend's knight joins the party when they arrive, dressed as the next
  knight of the company, mid-delve or at the entrance.
- The host listens on its tailnet address when Tailscale is up, else on the
  local network; nothing is opened to the internet. `/dungeon_host --2
  <address:port>` picks the address (`ANGEL_DUNGEON_LISTEN` sets it too).
- A wish the phrasebook doesn't know becomes a background angelX turn on the
  host's machine, in the friend's words. Invite people you would let type into
  your angelX.
- `/dungeon invite` shows the link again; `/dungeon invite new` makes a new
  one (the old stops letting anyone new in); `/dungeon lock` and `unlock`
  close and open the door to newcomers; `/dungeon kick <knight>` frees one
  seat; `/dungeon kick` ends them all.
- In the browser: WASD moves, the arrows aim, F fires, Space rolls, Q is the
  sword, E interacts, X throws a bomb, 1–4 cast spells, Z quaffs, R is the
  ultimate, and a gamepad works.
- Behind a public `https` door (a tunnel or a relay), set
  `ANGEL_DUNGEON_PUBLIC_URL` to it and the link names it, so a friend
  anywhere needs nothing installed; the page and the socket share the
  host's one port.
- The link survives a host restart: the checkpoint keeps its door and key.


### Woodland games, travelling shops, and browser skills

The kingdom's grove door leads to Woodland Games: a shared 30-second Firefly Hunt
and a four-note Woodland Memory round. Walk onto the gold or blue ring to begin;
leave the grove to return to the kingdom. The Undercroft's stair still comes
up to the gate.

Seeded travelling peddlers set up in safe surface or treasure rooms. Their plates
show their offers and prices. Hold **E** on a marked plate to buy.
Below, a knight pays with **carried gold**; on the surface, where nothing is
carried, the realm's **treasury** pays. Step off before buying again.
Full packs or insufficient gold never take payment.

In the browser the five skills are **M** Fart Cloud, **,** Fire Pit,
**.** Mines, **;** Barrier, **'** Shield Throw.
Practice at the gate or in the forest. Gas is a visible swirling poison cloud;
fire can ignite it. **1–4** use equipped spells and **Z** quaffs the first card
in hand. Click or press a key once to unlock browser audio. Quiet event sounds accompany gas, fire, barriers, spells, and wishes;
footsteps remain disabled to avoid crowding combat sounds.

### The Sanctuary privy

Approach the privy for **Privy Dash**. Each seat gets a large framed pixel
pop-up on its own screen, including browser guests. The window eases open
and shut; the arena shows only the stalls rattling and their occupied signs.
Two stalls run independently where clear floor permits. Other knights can
undo their armour while waiting, hopping in separate queue spots; the oldest
queued visit gets the next free stall, with seat order breaking ties.

**Hit green** with **F** (or the other action keys, including **E**). Three
straps fling the helm, pauldron and breastplate into tumbling, bouncing arcs.
Misses tangle the knight's hands and cost time. The knight waddle-sprints in;
**Pick a side** chooses **Quick pee** on the left or **Big poop** on the right.
Release the direction first. The longer job pays more. The door slams, its
moon window glows, the sign swings, flies buzz and little sound puffs escape.

The return crosses a chequered line between waving pennants, with a burst and
split time. Three more timed straps fly the armour back on. The final clock
includes dressing; the carved party board keeps each knight's best. A short
animation grades the time and accuracy: **Royal Flush** gets a daisy crown,
sparkles and a trumpet; **Fresh Start** a thumbs-up and flower; **Bit Whiffy**
an orbiting fly and stink curls; **Paper Trail** parchment caught on a boot.
The board updates before the grade appears, sorted fastest first. A new party
best flashes **PARTY RECORD!** for two seconds. Finish pennants leave the grade
scene clear for the knight's props.
All animation frames use simulation ticks and the realm palette.

The fixed grades use the existing quick limits (pee 8 seconds, poop 11):
Royal Flush is at or under that limit, no misses and at most one loose hit;
Fresh Start permits three extra seconds, two misses and four loose hits;
Bit Whiffy permits eight extra seconds and five misses; otherwise Paper Trail.
A strap left unanswered for eight seconds finishes with a disaster penalty.

The stakes are unchanged: a visit mends 10 health and refills mana; a poop
mends 10 more and grants 15% speed for 12 seconds; a quick stop grants 15%
damage for 10 seconds. The result animation uses none of that buff time.
Leaving a floor without relief costs a quarter of maximum health, never the
last point. Winning without a single visit remains
**The Great Unwashed**. Existing saved stops and guest snapshots remain readable.

Offline visual review: set `ANGEL_ARENA_SHOTS` and run the ignored
`write_privy_popup_shots` test. It writes the whole visit, each grade, both
seat views, the opening/closing transitions and `contact_sheet.png`.


## Floor transition paintings

Each arrival below the surface has a 384×224 dark-deco intertitle, fitted to
both terminal pixel transports and the browser guest frame. The gate's stair
shows **The Undercroft / Floor 0** on arrival in the Undercroft. The Winding
Stair (including Wren's selected landing), the mine-head shaft, ordinary
floor stairs and the circling hell gate show the destination's name and depth.
The intro begins in the gate courtyard and reaches the same stair path.

The two upper-floor orders are Crypt 1 → Mines 2 or Mines 1 → Crypt 2,
followed by Dragon Keep 3, Drowned Archive 4, Fungal Deep 5 and Unknown 6.
Crypt and Mines have separate compositions at each reachable depth. The Pit
and Wanted hall are rooms on those floors, not extra descents. The Labyrinth
routing below the approach floors and Fortune's variant layouts retain their
floor's painting; Wren's map uses those same destinations.

Playback lasts 54 outer simulation steps (1.8 seconds at 30 Hz, including
Turbo), with a fast iris from/to black, searchlight sweeps, lamp flicker,
forge embers, spores and water reflections. Any fresh key press dismisses the
picture on that seat. A shared keyboard dismisses the shared terminal view;
invited terminal and browser guests dismiss their own seat. Playback never
changes `Phase`, blocks the shared simulation or waits for another seat.
The hero, monsters and controls retain the existing simulation rules.

Far haze, middle silhouettes, near architecture, dithered bloom and surface
reflections are composed once. Outdoor plates cache eight soft, crossing
searchlight phases; interior plates cache one painting. Playback clones a
cached painting and adds only a few flicker, ember, spore or reflection pixels.
Crypt 1 shows the chapel graveyard, Crypt 2 its candlelit ossuary; Mines 1 shows
the pit-head winding gear, Mines 2 the mountain's great forge hall.
Frames are pure and deterministic; no runtime asset loading is involved. Three
inks are appended to the master palette: crimson **#741017**, navy **#293e69**
and mustard **#c9ac3a**. These live outside the five material/signal banks and
are excluded from `nearest_plain`, so room and world quantization is unchanged.
The corresponding sprite ink characters are χ, ν and μ, used only by plates.
All remaining plate colors come from the existing palette.

Offline review (from `cockpit/`):

```sh
CARGO_TARGET_DIR=/work/build-cache/angelx-plates TMPDIR=/work/tmp \
ANGEL_PLATE_SHOTS=/work/tmp/review-shots/plates-v2 \
ANGEL_PLATE_MOODBOARD=/work/tmp/plates-moodboard.png \
cargo test --bin angel write_floor_plate_shots -- --ignored
```

The ignored writer emits each painting, terminal-sized samples, three iris
frames and `contact-sheet.png` with the reference board beside the paintings.
