# The Barony, and the world above the Delve

A design for a living realm around the Delve: rooms of the world the party
walks between delves, folk who work in them and go down into the dungeons
and come back up, places the realm pays a great deal to build, a dwarf king
who takes back his halls with the realm's help, and the forges of those
halls, cleared, rebuilt and relit one by one.

The player's guide is [The Delve](DELVE.md); the cast is in
[the lore](DELVE-LORE.md). This document is for whoever builds the next
phase.

## What was asked

More of the world's folk going to and from the dungeons as a general system;
folk waiting in the main area; costly expansions of the home (taverns,
throne rooms, chapels, workshops); building the dungeons back out; and a
dwarf king, grateful for help reclaiming the underground, who runs the
mines, gives the party missions and is paid and governed as a baron of the
realm.

Three follow-ups came while it was being built:

- **The king's arc is his forge.** A dwarf king's reclaiming of his hall
  and its forge is the inspiration. Relighting the king's great forge is
  the emotional milestone: lost, cleared, rebuilt, relit, the hammers
  ringing again, and then a working part of the barony.
- **Forges, plural.** Every reclaimed hall or floor has its own forge to
  clear, rebuild and relight, each with its own output. The king's
  ancestral great forge is the crown of the chain. The relit forges together
  are what make the underground visibly the kingdom's.
- **The main world is the world, not the dungeon.** The places the realm
  builds (taverns, throne rooms, chapels, workshops, the stables, the lists)
  are rooms in the world, and a room of the world can hold a dungeon
  entrance. The joust and horse games did not work well enough to keep as
  they were: they move into world rooms and have to actually play.

## What was wrong: why things weren't building out

From the code, the renders and a live run of the real binary (in a scratch
home and workspace):

- **Buying a rung changes almost nothing you can see, and changes it at
  once.** The Undercroft's ladders (`home.rs`) pay from the treasury and set a
  level. The forge's fire lights, the rack gains a bottle; the room's
  geometry never changes. The one exception, the West Wing, opens the instant
  it is paid (`layout::dig_tavern`, a shake and a sound). Nothing is ever
  under construction.
- **Nobody moves.** Every keeper and resident stands on a fixed tile
  (`SPOTS`, `RESIDENTS.home_at`, `MERLIN_AT`, `BEAUMAINS_AT`). Tobbin's hammer
  and Wren's lantern are the only animation. There is no traffic anywhere,
  least of all to or from the stair.
- **The loop settlement is fed only by /loop, and gives play nothing.**
  `together_settlement` buys work only with completed, durable loop
  iterations. It cuts six rooms of a few block tiles, each with a sign and
  nobody in it (`settlement-local-research-delve.png` in the audit renders:
  a dark room, a STOCKPILE sign, six crates). Its stone, ore and tools can't
  be spent in the realm by design.
- **The dungeons never change.** Every delve lays out every floor afresh
  from its seed. `Realm::reclaimed`, the count of floors cleared per delve,
  is written by `settle_realm` and read by nothing: no drawing, no rule.
- **There is nothing big to aim at.** Tobbin's dearest rung is 1,200 gold. A
  bot that clears floors one to three whole banks about 2,200 gold, 14 ore,
  7 gems and 46 embers a delve (`haul_probe`, below). A few delves buy
  everything the Undercroft sells.
- **The stables and the lists were a side stage, not a game.** A capture run
  found the 3D dot render murky and unreadable at 1:1, with the kitty
  background showing through it; a pass had no charge or hit, only a status
  line; some key presses flashed a blank frame (the command set
  `redraw_requested`, which clears the terminal); and the game was three
  guard, aim or charge picks.
- **The Labyrinth's lower tunnels** (floor three down) are laid out by the
  research router, freshly each delve, like every other floor; nothing there
  is kept either. They have not been playtested yet.

## Where the world is drawn, and why

The overworld map (`stage/world_viz/overworld`) is walked by the harness: its
knight goes where the agent's work is (`World::tick_overworld`,
`overworld_goal`), it takes no movement keys, and it has no doors or
interiors. The 3D mini-world is the murky render the capture run rejected.
The Delve's room engine (`drive/together_shooter`, drawn by
`overworld/arena.rs`) already draws in the overworld's own hand (its kit,
its ground marks, black paper, dusk and firelight), takes keys, and carries
co-op, the mirror and checkpoints.

So **rooms of the world run on the Delve's room engine**, painted as outdoor
places: meadow tufts and pebbled roads (the overworld's `ground` marks),
trees and the mountain's rock for edges, the overworld's buildings at room
scale. They are rooms of the home floor (depth 0), in their own corner of
its grid (row -8 and above, beyond the six cells a loop settlement can dig),
so nothing in the Delve's plumbing had to learn a new kind of floor. Each
names the overworld place it belongs to (the Delve's gate near the Mines,
the lists, the mine-head).

**Entrances** are walk-on stairs, held for a moment like the Winding Stair
(`world::ENTRANCES`): to another room (the gate's stair down to the
Undercroft, a new stair in the Undercroft's north wall back up, the King's
Hall's great door) or straight into a delve (the mine-head's shaft into the
Mines). An entrance can wait on a work the realm has paid for (the great
door opens once the hall stands).

A delve now **begins at the gate**, in the world. The Undercroft is down its
stair. `layout::undercroft` is untouched, because a saved loop settlement
replays it byte for byte; the world and the Undercroft's stair up are added
to the run's own copy of the floor (`raise_world`), after a settlement is
admitted. `/dungeon settlement` still visits the hall below.

## Folk: the general activity system

`together_shooter/folk.rs`. Everyone about follows a **route**: walk a path
through a room, stay somewhere at a task, or be away (down the shaft, out to
market) for a while, and round again. Where anyone is, is a **pure function
of the realm's state and the tick**: nothing about them is saved or sent, a
friend's mirror shows the same traffic, and a test can ask where everyone is
at any moment. That makes them cheap (a few stamps a frame), persistent (who
is about follows from the saved barony), deterministic and shared.

Who is about comes from what the realm has done:

| When | Who | Where |
|---|---|---|
| King Brannoc has come | the King, two guards | camped by the ruin at the mine-head |
| | the King's man | waiting by the signpost in the gate courtyard |
| The Upper Workings are his | miners, three, two more for every forge lit | down the shaft with picks, up with sacks of ore to the stockpile |
| | a miner off his shift | resting by the well in the gate courtyard |
| A work goes up | his builders, three and one more for every mission done | stone up from the shaft to the ruin; or down the shaft to a forge-hall, working round its furnace |
| A forge burns | a carter | the realm's share of the ore, out through the gate courtyard to the realm's road, and back |
| | two smiths | at its anvils below, sparks flying |
| His hall stands | the King on his seat, guards either side, a sentry at the door | inside the King's Hall, and at its door |

Phase 2 extends the same rosters to the realm's other folk (see the plan).

## The stables and the lists

The 3D side stage is gone; the stables and the lists are rooms of the
world, either side of the gate courtyard, and the realm keeps the stable in
its home (`Home::stable`).

- **The stables:** three mounts over their half-doors, the saddled one out
  in the yard. Hold F at a stall's plate to saddle it, at the trough's to
  tend it (brushed, watered, tack checked: a knock more of balance for the
  next bout). Wat the groom has a word.
- **The lists:** hold F at the plate by the red pavilion to mount. A course is
  a charge down the tilt, faster all the way; the rider aims (W high at the
  helm, S low at the shield), strikes (F, as the lances meet; a ring closes
  over the tilt where they will, and burns gold while a strike would land)
  and braces (Space, as his lance comes in). The rival's shield shows where
  he guards a moment before the meeting: strike where it isn't. The riders
  hold at the clash while a late press can land, then the impact: points,
  knocks, a broken lance and splinters, and a rider who loses his balance
  goes over his horse's tail and onto the sand while the horse runs on.
  Three courses, or an unhorsing.
- **The mounts ride differently** (`Mount::steed`): Bramble is steady (more
  balance, a wide brace), Cinder is fast and heavy (the soonest meeting, the
  hardest hit, the narrowest strike), Mist is light and true (a wide strike,
  and the rival's guard is readable early).
- **The rivals** are a ladder: Sir Kay, then Sir Palamedes, then Sir
  Lancelot. Each has his own balance and lance pattern, varied from bout to
  bout. The realm keeps bouts, wins and unhorsings; two achievements (Over
  the Tail, Champion of the Lists).
- **No purse yet.** A bout pays honour and achievements, not gold, so the
  lists can't be farmed. Purses for tournaments are a Phase 2 question.

## King Brannoc Onehorn of Caer Dwfn

An original character and a loving nod to Bruenor Battlehammer: a red beard
braided to his belt, a steel helm with one bone horn (the other snapped to a
stub), a notched axe (every notch a goblin, he says, and every goblin a
notch), a round shield with a foaming tankard on it, gruff, fiercely loyal,
homesick for his lost halls. Every name is the realm's own: Caer Dwfn is
Welsh for the deep fort.

**The story.** Caer Dwfn was a dwarf-kingdom. Its halls ran from a gate-hall
at the mine-head down into the Mines and below, and every hall had a forge.
A hundred years ago the goblin clans came up from below (the Mines' rival
clans are their heirs), Cinderjaw, the furnace golem the Mines already have,
took the first fire, the forges went cold one by one, and the gate-hall fell
in. Brannoc, the last of its kings, comes up the Mines' shaft once the party
has cleared a floor of the Mines (or at once, for a realm already past it),
and camps by the ruin.

**His arc is the forges.** Every hall had one, each with its own work:

| Forge | Hall | Makes | Phase |
|---|---|---|---|
| The Ore-Forge | the Mines | smelts the Mines' ore; dwarf-forged mail | 1 |
| The Bell-Forge | the Crypt | bells and candles for the dead | 2 |
| The Ember-Forge | Dragon Keep | fire-arms from the dragon's coals | 2 |
| The Type-Foundry | the Drowned Archive | type and plates for Pip's maps | 2 |
| The Spore-Kiln | the Fungal Deep | Maud's draughts, fired | 2 |
| **The Great Forge** | the King's Hall | the crown: lit only with the first fire | 3 |

Each is **lost** (the hall goblin-held), **cleared** (his mission frees the
hall; its forge-hall is found off the floor's first room, in ruin),
**rebuilt** (the realm pays; his dwarves build it over realm time, course by
course), **relit** (with fire carried up from Dragon Keep: embers), and then
**works** (smiths at its anvils, its output in the tribute, its specialty
for the realm). The Great Forge stands cold in his hall from the day it is
rebuilt, a chain across its mouth; the first fire, won back from Cinderjaw,
hangs in a lantern beside it until the chain is lit.

**He is a baron of the realm.** He gives missions; the realm funds his works
(and pays a great deal); he pays back as a vassal (ore, gems, gold, the
realm's share carted to market, dwarf-forged mail, more workers). The
**ledger** on the lectern in his hall keeps what went in and what came back,
in totals and its last lines. When his hall stands he kneels in it and
swears Caer Dwfn to the crown (A Baron Sworn).

## The economy, with its evidence

Every number is one fixed number, picked from evidence, not a formula. The
evidence is `haul_probe` (in `together_balance__tests.rs`): a bot with a
well-built Undercroft fights every room of floors one to three, opens the
chests, walks over everything dropped and takes the stairs, eight seeds.
One knight banks about 562 gold on floor one, 807 on floor two and 845 on
Dragon Keep, about 14 ore and 7 gems from the Crypt and the Mines, and 38 to
53 embers from the Keep: about 2,200 gold a delve to Dragon Keep, perfectly
collected. A floor takes a bot 4 to 8 minutes of fighting.

| What | Price | Labour | Why |
|---|---|---|---|
| The King's Hall | 3,000 gold, 40 ore, 20 bone, 6 gems | three dwarves, 30 minutes | Most of two perfect delves; 2.5 times Tobbin's dearest rung |
| The Ore-Forge | 2,000 gold, 30 ore, 4 gems | three dwarves, 20 minutes | Most of a perfect delve |
| Relighting it | 8 embers | | A fifth of Dragon Keep's embers |
| The Upper Workings (mission) | clear 8 halls of the Mines | | About two Mines floors |
| The First Fire (mission) | fell Cinderjaw | | The Mines' guardian, chief of a clan |

**Labour** is realm time: while a work goes up the run marks ten seconds at a
time (`barony:labour`) wherever the party is, at home or below, and each
dwarf at work adds a crew-second a second. Half an hour is about one delve to
Dragon Keep. More missions done means more dwarves home and faster works.
The crew builds one work at a time, in the order paid for.

**Tribute** is paid for every floor the party clears (takes its stairs),
because his miners follow the knights down: 3 ore and 30 gold a floor once
the Upper Workings are his (a third again on a Mines floor's ore), and 1 gem
and 60 gold more for every forge burning. **Mission rewards:** 150 gold and
10 ore for the Upper Workings, 4 gems for the First Fire. **Dwarf-forged
mail** (one piece of armor, three damage turned from every hit) is a home
card for every knight once the Ore-Forge burns.

## How it ties into what exists

- **The Undercroft** stays home: its rungs, keepers, Merlin, the tavern, the
  Trophy Hall and Fortune's hall are one stair down from the gate.
  Merlin's first tip, once the King has come, sends you to him.
- **Floor politics:** the Mines are rival clans of goblin chiefs; they are
  the heirs of the clans that took Caer Dwfn. Cinderjaw, the Mines' own
  guardian, chews the first fire. A reclaimed Mines floor gets its
  forge-hall off its first room before its politics are drawn, so the camps
  and chiefs see it; it is chosen without dice, so every other room stays as
  the seed laid it.
- **Achievements:** A Baron Sworn, The Hammers Ring, Over the Tail,
  Champion of the Lists (forty-two in all).
- **The chorus:** the King, Wat the groom, Sir Palamedes and Sir Lancelot
  have lines; the Herald calls the joust and the barony's moments.
- **The settlement** (loop-built wing) is unchanged; Phase 3 lets completed
  loop iterations add labour to the barony's works (the harness builds the
  town).

## Save format, migration, co-op

- The realm keeps the barony and the stable inside `Home`
  (`Home::barony`, `Home::stable`), each **decoded on its own**: a malformed
  or newer subtree resets to its default instead of failing the whole realm
  (`Realm::beside` falls back to an empty realm on any decode error).
- **Migration:** an older realm has neither, and loads as a realm the King
  has not come to yet; he arrives at once if it has been below the Mines.
  The old practice game's mount and tending move into the stable once.
- **Older builds** reading a new realm ignore the two new fields (they lose
  the barony and the stable, never the realm). No new `Station` was added
  to `Home::levels`, whose keys an older build must parse.
- A Delve **checkpoint** now contains new room kinds (the world, the King's
  Hall, a forge-hall): an older build can't load such a checkpoint and starts
  a new run. Friends need the same build, as before.
- **Co-op:** the world, the folk, the stable, the barony and a bout are all
  in the run that friends mirror. What anyone's knight does at a plate is a
  mark the host's cockpit settles into the host's realm, like every other
  purchase. One knight rides a bout at a time; the others watch from the
  sand.

## The plan

### Phase 1 (this branch)

- The world above the Delve: the gate courtyard, the stables, the lists, the
  mine-head, the King's Hall; entrances; delves begin at the gate.
- The general folk system, with the barony's rosters.
- The stables and the lists rebuilt as a real joust, the old side stage
  removed.
- King Brannoc: his coming, camp, meeting, voice lines, sprites.
- Two missions (The Upper Workings, The First Fire), the King's Hall and the
  Ore-Forge as works built over time, relighting, tribute, dwarf-forged mail,
  the ledger, the war table of the forges, the cold Great Forge.
- Tests for the economy, the joust, the folk, the world; staged renders of
  every new visual; a live run of the binary.

### Phase 2

- **The rest of the chain:** the Bell-Forge (Crypt), the Ember-Forge (Dragon
  Keep), the Type-Foundry (the Archive), the Spore-Kiln (the Fungal Deep),
  each with a clearing mission (its floor's chiefs, a guardian), a rebuilding
  work, a relighting cost from its own delve's spoils, and a specialty: the
  Ember-Forge's fire-arms as cards, the Type-Foundry making Wren's landings
  cheaper, the Spore-Kiln firing Maud's rounds stronger, the Bell-Forge
  ringing a second wind for the Chapel of Bonds.
- **Reclaimed floors rebuilt:** a waystation in the first room of a held
  floor (dwarf guards, a brazier, the tankard banner), lit sconces along the
  Labyrinth's lower tunnels once their halls are held, bridges over the Mines'
  chasms, dwarven patrols that take a room's fight off a held floor.
- **More works in the world** at the same scale of price: the Great Chapel,
  a Workshop Row (Tobbin's apprentices; card-making), a Brewery for Maud,
  Barracks (hirelings beyond Beaumains), a Market (merchants waiting in the
  courtyard), and the West Wing's tavern brought up into the world.
- **More folk:** the Undercroft's residents on schedules (Mabel to the
  hearth, Pip to the map table), adventurers for hire waiting in the
  courtyard, merchants; folk with a word when walked up to.
- **The overworld map** shows the barony: the gate-hall at the Mines' mouth,
  smoke from relit forges, the carter on the road.
- **Tournament purses**, and knights jousting each other in co-op.

### Phase 3: the crown

- **The Great Forge relit** with the first fire once the chain's forges
  burn: the barony's crowning moment, its own film; what it makes
  (legendary arms, the realm's crown work).
- **The harness builds the town:** completed loop iterations add labour to
  the barony's works, and the settlement's miners join the King's.
- The King in the loop's crawl, with the realm's other folk.

## Trying it

From the branch's worktree: `bin/angelX` (or the built binary), then
`/dungeon start`. The party stands in the gate courtyard: the stables are
west, the lists east, the mine-head north, the Undercroft down the gate's
stair. `/dungeon stable`, `/dungeon tournament` and `/world visit
stables|lists` walk straight there. King Brannoc comes once a floor of the
Mines has been cleared (or at once for a realm already past it).

Renders of every new visual are written by the ignored shot writers
`write_world_shots`, `write_joust_shots` and `write_barony_shots` in
`tests/cockpit/world_viz/overworld__arena_tests.rs`
(`ANGEL_ARENA_SHOTS=<dir>`).
