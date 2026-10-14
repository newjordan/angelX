# The Delve, next: arms that feel good, follies, and holdings

Three asks from 2026-10-12, in one place:

1. **Arms.** Friends say the weapons and skills aren't as fun as they need to
   be. What's wrong, measured from the code, and what the great bullet hells
   do instead.
2. **Follies.** Dungeons feel generic. Add rooms that play like a different
   game: NES/SNES tropes (never Atari), original takes with our knights, so a
   delve feels a little like Mario Party. The mineshaft marble run, with a
   knight inside a themed metal ball, is the flagship.
3. **Holdings.** Clear a whole dungeon and you can buy its floors. Buying is
   a Crusader Kings decision tree. Building is a SimTower cross-section. The
   rooms come from Dwarf Fortress. Each floor has one focus and two side
   focuses and holds ten rooms; deeper floors are better floors.

---

## Part 1: Arms

### What the code says today

All numbers are from `together_shooter.rs`, `loot.rs`, `monsters.rs` and
`stakes.rs`. The sim runs at 30 ticks a second; a room is 48 × 28 units.

| | Shot speed | Rest | Shots a second | Damage | Damage a second |
|---|---|---|---|---|---|
| Knight (moving) | 13 u/s | | | | |
| Bow | 17 u/s | 15 ticks | 2.0 | 32 | 64 |
| Crossbow | 20 u/s | 32 ticks | 0.94 | 75, pierces 1 | 70 |
| Handgonne | 27 u/s | 45 ticks | 0.67 | 95 + 47 burst | 63 + splash |
| Knight's Blade (melee) | | 16 ticks | 1.9 | 30 | 56 |
| Monster shots | 4.5 to 9 u/s | | | | |

`loot.rs` states the intent: "Shots are slow and heavy: one well-placed arrow
matters more than a stream of them." The feedback says that intent isn't
landing. Four reasons, in order of how much they matter.

**1. Our arrows barely outrun the knight.** An arrow is 1.3 times as fast as
the knight carrying it (a handgonne ball, 2.1 times). Shots don't inherit the
knight's motion, so a knight running forward while shooting sees the arrow
creep ahead at 4 u/s. A bat flies at 6.5 u/s with a body 0.6 across; at a
typical 12-unit range the bow's arrow takes 0.7 s to arrive, and the bat has
moved 4.6 units, about four bodies. Unled shots at moving foes mostly miss.
Every shooter people love does the opposite: **your shots are fast, theirs
are slow.** Our monster shots are slow, which is right. Ours are slow too,
which is the problem.

**2. Co-op makes every shot feel like a pea.** A monster's health is
multiplied by the head count (`hp = base * players`). With four knights a
skeleton has 280 health, so one arrow is 11% of it, and nobody's shot ever
visibly kills anything. On top of that, from floor 3 down monsters gain 25%
health for every point of party strength, up to +120% (`POWER_HEALTH`). That
rubber band quietly cancels the cards you picked up. On floor 6 with four
knights at strength 2, a skeleton has about 520 health and a demon about
1,600. The power fantasy is taxed twice.

**3. All three arms are the same weapon.** They all deal about 65 damage a
second in a single line straight ahead. The difference is only the cadence,
and every cadence is slow. Nothing sprays, nothing beams, nothing bounces,
nothing knocks a foe back.

**4. Hits are quiet.** A hit flashes the monster for 6 ticks and plays "hit".
There is no knockback from a normal shot, no hit-freeze, no recoil, no muzzle
flash, no impact spark, no shake below a bomb, nothing left behind on the
floor, and no payoff when a room's last foe falls.

**Skills** have the same shape. Five keys (gas, fire, mines, barricade,
shield throw) with 9 to 14 second rests, and four of the five are zone
control: you place something and wait. On top of that sit three spells, a
bomb, the sword, vigil, roll, shield and the ultimate. That is a lot of
buttons for powers you use a few times a minute, and few of them reward
aggression.

### The bullet hells people remember, and their arms

| Game | Arms | What makes them feel good |
|---|---|---|
| **Contra** (NES) | The Spread gun (S), machine gun, laser, fire | The Spread is the most loved weapon of the NES: five shots in a fan, fast, forgiving. Pick-ups are letters in flying capsules: you see the upgrade coming. |
| **Gradius** (NES) | The power-up bar: speed, missile, double, laser, option, shield | You choose your upgrade. Options are glowing orbs that trail you and copy every shot, so power is something you can see. |
| **Raiden** | Red vulcan spread, blue laser, purple homing "toothpaste" | Each colour levels up visibly and is a different geometry, not a different number. |
| **DoDonPachi** | Tap for a wide shot, hold for a laser; a hyper meter | Hold-to-laser also slows you and tightens control; tap spreads wide and moves fast. The combo chain turns killing into a rhythm. Boss kills turn every bullet on screen into points. |
| **Touhou** | Per-character shot types, focus mode, bombs (spell cards), graze | Your shot is near instant; theirs are slow and pretty. Holding focus slows you and shows your tiny hitbox. Grazing a bullet pays you. Items vacuum to you at the top. |
| **Ikaruga** | Polarity: absorb bullets of your colour | Absorbed bullets charge a homing-laser release. Defence becomes ammo. |
| **Radiant Silvergun** | Seven weapons from three buttons, plus a sword that eats pink bullets | Every weapon is one geometry: forward, homing, spread, rear. The sword turns danger into power. |
| **Nuclear Throne** | Shotguns, crossbows, launchers, energy swords | The Vlambeer "Art of Screenshake" list: bigger bullets, faster bullets, hit-freeze, knockback, recoil, shake, muzzle flash, impact effects, corpses and casings that stay. |
| **Enter the Gungeon** | Hundreds of guns, each a joke with a mechanic (a mailbox, a bee hive, a bat that bats bullets back) | The dodge roll is untouchable; the blank clears the screen; guns combine into synergies with names. Active reload rewards timing. |
| **The Binding of Isaac** | One tear, transformed by items: Brimstone, Mom's Knife, Tech X, Polyphemus | Every item visibly changes your shot. Stacks get absurd, and absurd is the point. |
| **Vampire Survivors** | Whip, garlic aura, orbiting bible, bouncing runes, lightning | Each weapon covers a different shape of space. Weapon + matching passive evolves. Kill counts climb into the thousands; gems vacuum in. |
| **Cuphead** | Peashooter, Spread, Chaser, Charge; EX moves and supers | Parrying pink things fills the super meter. The meter, not a timer, gates the big moves. |
| **Geometry Wars** | Shot pattern upgrades with score | Every death is a firework; the grid warps under explosions. Feedback is the whole game. |

What they share, as rules for us:

1. **Your shots are fast, theirs are slow.** Ours should be at least three
   times the knight's speed. Theirs stay readable.
2. **Each arm is a shape, not a number.** Stream, fan, rail, bounce,
   boomerang, orbit, beam, homing.
3. **Power you can see.** Options that trail you, shots that grow, arrows
   that split, a ball that turns gold.
4. **Hits land.** Knockback, hit-freeze on the foe, sparks, recoil, shake on
   big shots, deaths that burst, a payoff at the room's end.
5. **Big moves come from a meter you fill by playing well,** not a timer you
   wait out.
6. **Danger pays.** Graze, parry, absorb.

### Proposed changes

Fixed numbers, picked to feel, then checked in play
(one number each, no formulas).

**Three arms, three shapes**

| Arm | Shape | Shot speed | Shots a second | Damage | Extra |
|---|---|---|---|---|---|
| Bow | Stream: hold to keep firing | 40 u/s | 6 | 12 | A small jitter so a stream looks alive |
| Crossbow | Rail: a near-instant line through everything | 70 u/s | 1.3 | 55 | Pierces all, leaves a tracer that fades |
| Handgonne | Fan: five balls in 30° | 34 u/s | 1.2 | 14 each | Recoil pushes the knight back; strong knockback on foes; smoke puff |

Forged arms (cards with `bolt`) get a floor of 36 u/s on `speed` in the card
checker, so a homemade arm can't be slow.

**Hits that land** (the Screenshake list, sized for our 30-tick sim)

- Foe hit-freeze: 2 ticks on a hit, 4 on a kill (`enemy.frozen`, per foe, so
  the shared sim never pauses).
- Knockback by arm: stream a nudge, rail a push, handgonne a shove.
- Impact sparks in the shot's colour; arrows stick in walls until the room
  ends.
- Recoil and a muzzle flash on the handgonne and the crossbow.
- Shake 2 on a handgonne shot, 3 on a kill by anything heavy.
- Deaths burst into parts and a little fountain of gold.
- **Room's end:** every hostile shot left in the air turns into a gold coin
  and flies to the nearest knight (DoDonPachi's bullet cancel).

**Co-op: harder, never thinner** (user, 2026-10-12)

Monster health stays as it is. The game is too easy with several knights,
so the fix adds pressure instead of taking health away: more foes and more
shooters per knight, monsters that split their attention across the party,
and fewer free answers (stacked stuns, stacked ultimates, cheap risings).
Faster, better-feeling arms land together with that pressure, checked in
the same-seed party-size harness. (Built: see `docs/DELVE.md`, "A party is
never an easier delve" and "Arms, and how they feel".)

**Skills on a meter**

- A **Valour** meter of three pips fills from hits landed and from grazes
  (a hostile shot passing within half a unit without hitting).
- A skill costs one pip and has a 3-second rest instead of 9 to 14.
- Playing aggressively and dodging close means using skills more often. A
  knight who hides doesn't get them.
- Synergies with names, shown when they happen (the Gungeon trick):
  - **Fire Arrows:** fire skill + bow; 4 seconds of burning shots.
  - **Ricochet:** shield throw + crossbow; a rail that hits the flying shield
    bounces to the next foe.
  - **Powder Keg:** gas + handgonne; a ball through gas detonates it.
  - Fire + gas already flashes, and keeps doing so.

**Tempering: power you can see** (Gradius / Contra)

Picking up the arm you already hold tempers it, up to three:

- **Bow:** a second stream, then a third, then the outer streams home.
- **Crossbow:** a thicker rail, then it splits on the first kill, then it
  chains.
- **Handgonne:** seven balls, then the balls bounce once, then they burst.

### Finding out what friends actually feel

"Not fun" needs detail, so measure while friends play:

- **Feel ledger**, one line per room per knight, written beside the run log:
  - shots fired and hit rate;
  - average time to kill by foe kind;
  - "dead air", the seconds a knight dealt no damage;
  - skills used per minute, grazes;
  - each knight's share of the kills.
- **The A/B rack** in the Training Yard. Today's three arms and the proposed
  three hang side by side on Grubbins's rack, with the Dummy and a few
  moving bats to shoot. Friends try both in thirty seconds and say which.
- **Three questions** after a session:
  1. Which arm did you drop on sight?
  2. When did you feel strongest?
  3. Which key did you never press?

---

## Part 2: Follies

A **folly** is a room in a delve that plays like a different game. Each
floor rolls one or two among its halls. The door has a painted mummers'
arch, so you know before you step in.

### Rules every folly keeps

These come from the 2026-10-11 verdict on the surface mini-games
(the surface games were one player plus spectators, with no stakes and walls of text):

- **Every seated knight plays at once.** No volunteers and no spectators.
- **Real stakes.** Each knight antes 10% of the gold they carry. The pot goes
  to the podium (first takes half, second a third, third the rest), and the
  realm adds a floor bonus. The winner also takes a **Laurel**. At the end of
  the delve, the knight with most Laurels takes first pick of the hoard's
  cards.
- **Two or three plain words** on the title card, and nothing else.
- **60 to 90 seconds,** then a podium, then back to the delve.
- **Original takes,** in our knights and our realm. The trope is borrowed;
  the names, art and twists are ours.
- **Every knight has their own screen.** Races follow each knight; the
  podium is shared.

### The follies

| Folly | The classic it nods to | Where | Words | Engine |
|---|---|---|---|---|
| **The Shaft Run** | Marble Madness (NES), Super Monkey Ball | Mines | ROLL. DON'T FALL. | 3D |
| **The Keg Gantry** | Donkey Kong | Dragon Keep | CLIMB. JUMP KEGS. | Side-on hall |
| **The Wax Maze** | Pac-Man | Crypt | EAT WAX. FLEE WRAITHS. | 2D top-down |
| **The Powder Room** | Super Bomberman (SNES) | Mines | BOMB CRATES. | 2D top-down |
| **The Ore Cart** | Donkey Kong Country mine carts (SNES) | Mines | JUMP. SWITCH. | 3D chase |
| **Shelving** | Tetris | Drowned Archive | STACK BOOKS. | 2D |
| **Book Hop** | Frogger | Drowned Archive | HOP. DON'T SINK. | 2D top-down |
| **Spore Bubbles** | Bubble Bobble (NES) | Fungal Deep | TRAP. POP. | Side-on hall |
| **The Stalk Climb** | Ice Climber (NES) | Fungal Deep | CLIMB UP. | Side-on hall |
| **The Belfry Shoot** | Duck Hunt | Crypt | SHOOT BATS. | 2D, arms |
| **The Griffin Run** | Star Fox (SNES) | Dragon Keep | FLY. SHOOT. | 3D on rails |

**The Shaft Run** (the flagship). Each knight is inside their own metal ball,
running like a gerbil. The ball has weight and momentum: the knight's feet
steer and push. A spiral mineshaft goes down through timbered ramps, plank
bridges over drops, crossing ore carts, a lodestone rail, a sluice and a
collapsing drift. Gem shards sit on the risky lines. Balls bump each other.

- **Keys:** move to steer. Roll is a hop: the knight kicks off the ball's
  wall. Shield is the brake.
- **Finish:** the shaft ends in Brannoc's ore-sorter, which ranks you.
- **The balls, one per knight:**

| Knight | Ball |
|---|---|
| Sir Percival | bronze cage with fletched fins |
| Dame Lynette | white enamel with lace filigree |
| Sir Gareth | riveted siege iron with a counterweight band |
| Sir Galahad | polished mirror silver |
| the Composer | brass, with organ-pipe ribs that whistle as it rolls |
| the Dispatcher | copper, wax-sealed and stamped |
| the Loop Knight | clockwork orrery rings |
| the Money Knight | gold, with a milled coin rim |
| the Competition Knight | black lacquer, checkered stripe |
| the Scryglass Knight | clear crystal: you see the knight running inside |
| Dame Thrudi | dwarven iron, rune bands, a braided tassel. Iron sticks to the lodestone rail, so her line is her own |

**The Keg Gantry.** A girder scaffold up the Keep's broken tower. A drunk
ogre at the top rolls kegs down the slopes, and a captive from the Rescues
waits beside him. Everyone climbs at once; jumping a keg scores; the
fire-iron pickup smashes kegs for a few seconds. The first knight to the top
frees the captive. It uses the side-on halls' gravity, jump and crouch.

**The Wax Maze.** A crypt maze strewn with candle-wax drops. Four wraiths
roam it: one chases, one cuts you off, one flanks, one wanders (the classic
four temperaments, our own spirits). Holy water turns them pale and fleeing,
and wraiths eaten bank bone. Every knight munches at once, and the pellets
are shared, so you race your friends for the corridors.

The rest follow the same pattern. Bomberman and Bubble Bobble are the two
best four-player games on this list; Shelving ties into Part 3, because
cleared rows mend a held floor's walls.

### How follies get built

A **folly engine**: one small Rust trait for a self-contained room game.
Each folly has its own state, its own input mapping, its own draw (pixel or
world3d), and its own tests. The engine owns the shared pieces:

- the arch;
- the title card;
- the ante and pot;
- the podium;
- the Laurels;
- returning to the delve.

Each folly is a file in `together_shooter/follies/`, and it gets a
hot-layer JS leg like the loops do.

**Risk:** world3d currently paces at a six-tick step, and a marble race
needs 30 frames a second. The first task of the Shaft Run is to time world3d
at the Delve's viewport. If it can't hold 30, the shaft renders as a
Mode-7-style tilted floor (SNES-true anyway), and true 3D stays for the
Griffin Run's slower vistas.

**Done (2026-10-13):** world3d now fills its frame in bands across threads
and paints a seat's race in 2 to 3.5 ms, so all three races are true 3D
(timings in DELVE-TASKS.md, "3D at frame rate").

---

## Part 3: Holdings

Today the Barony's phase 2 plans taverns, chapels and workshops as world
rooms above ground ([BARONY.md](BARONY.md)). Holdings take the same idea
underground: the dungeon you cleared becomes yours, floor by floor.

### When a dungeon is cleared: the King's court

Clearing every floor of a dungeon (the Crypt, the Mines, Dragon Keep, and
later the deep) calls **King Brannoc's court** at its mouth. Each floor is
offered as a holding. The choice is a Crusader Kings tree: each branch costs
something, and each changes what follows.

```
Floor cleared ─┬─ Claim it outright ──── full deed price; yours alone; the King's favour drops
               ├─ Hold it in fief ────── 40% of the price; the King takes a fifth of its income
               │                          and picks one side focus; his favour rises
               ├─ Grant it to a guild ── free; the guild picks the focus; you get a tenth;
               │                          buy it back later at 120%
               └─ Leave it wild ──────── monsters creep back; offered again next clear
        │
        ▼
   Charter: pick the focus ─── e.g. Tavern ─┬─ Alehouse  (cheap, rowdy: brawls, bruiser hirelings)
                                            ├─ Inn       (rest: every delve starts with a ward)
                                            └─ Mead Hall (floor 3 and down: feasts raise banners)
        │
        ▼
   Wings: pick two side focuses
        │
        ▼
   Steward: a rescued folk with traits (Greedy, Pious, Brave, Sot, Shrewd)
        │
        ▼
   The build: the cross-section, ten rooms
```

The King's favour is one number. High favour opens royal events (dwarven
masons at half price, a royal visit with a gift). Low favour brings tithe
demands and his men in your halls. Events come one at a time, CK-style, as
a card with two choices and a price on each. For example: "The brewers'
guild wants the lower drift. Grant it (+income, −a room slot) or refuse
(their next delivery is late)."

### The cross-section: SimTower in a dungeon

Each held floor is shown side-on as a long slice: ten room slots on a
lattice, the Winding Stair as the lift shaft at one end, and folk walking in
from the Barony's traffic to use what you built.

- **Ten rooms per floor.** The floor's depth sets the sizes it can hold and
  how good each room is:

| Floor | Room sizes | Quality |
|---|---|---|
| 1 | small | ×1.0 |
| 2 | small, medium | ×1.2 |
| 3 | small, medium, large | ×1.4 |
| 4 | small, medium, large | ×1.6 |
| 5 | all, including grand | ×1.8 |
| 6 | all, including grand | ×2.0 |

- **Focus and wings.** Rooms of the focus family are unlimited. Each wing
  family may take up to three rooms. Utilities (quarters, larder, store,
  stair landing) are always allowed.
- **Stars**, as in SimTower:
  - ★1: claimed.
  - ★2: five focus rooms.
  - ★3: a steward and both wings built.
  - ★4: no complaints for three delves.
  - ★5: a grand room of the focus.

  Each star opens new room types and raises the floor's income.
- **Neighbours matter.** A smithy beside an inn makes noise (inn income
  down). A brewery within two slots of a taproom feeds it (up). The
  complaint bubbles say which.
- **Income.** Each delve, every room earns a fixed sum by type, times the
  floor's quality, minus upkeep. Holdings pay back slowly, the SimTower
  loop.
- **Walk it.** A held floor is a real floor of the delve, built on the same
  room engine as the Barony's world rooms. Passing through on a delve, the
  party can use what's built: buy at your market, heal at your infirmary,
  hire at your tavern. Building is the management screen; walking your own
  halls is the payoff.
- **Raids.** A floor without a garrison can be raided by the dungeon's
  monsters (Dwarf Fortress sieges). A raid is a defence room in the next
  delve. Losing it closes a room until it's repaired.

### The rooms, by focus family

Drawn from Dwarf Fortress's buildings and workshops. Each room touches at
least one other part of the game, which is how holdings drive everything
else.

| Family | Rooms | What they change elsewhere |
|---|---|---|
| **Tavern** | Taproom, Inn rooms, Brewery, Kitchen, Bard's stage, Mead Hall (grand) | Hire a hireling; rumours mark a secret room on the next delve; Inn rooms start a delve with a ward |
| **Forge** | Smithy, Armory, Fletcher, Mason, Mechanic, Jeweller | Fletcher: +shot speed and more arm cards; Armory: start tempered; Mechanic: Shaft Run ball shells and Ore Cart parts; Mason: repair raided rooms |
| **Chapel** | Shrine, Chapel, Ossuary, Infirmary, Bell tower | An extra rising per delve; holy water drops; a fall keeps 75% of the haul instead of half |
| **Barracks** | Barracks, Archery range, Training hall, Gaol, Kennels | Garrison stops raids; the range adds the Belfry Shoot as free play; Kennels give a hound companion |
| **Scholarship** | Library, Scriptorium, Alchemist, Observatory (grand, deep only) | Skill rests shorter; Valour fills faster; maps show folly arches early; new spell cards |
| **Trade** | Market stall, Trade depot, Counting house, Warehouse, Mint (grand) | The travelling merchant carries one more card; treasury interest each delve |
| **Fare** | Mushroom farm, Still, Butcher, Larder, Fishery (Archive only) | Feeds taverns and barracks; without fare, those earn half |
| **Revels** | Games hall, Marble hall, Lists yard | Follies played at home as free play for Laurels, and ball shells shown off |

### Prices

Fixed numbers, set against the measured haul of about 2,200 gold per
Dragon Keep delve. All prices are draft; one probe will settle them.

| Floor | Deed (claim) | Fief (40%) | Small room | Medium | Large | Grand |
|---|---|---|---|---|---|---|
| 1 | 6,000 | 2,400 | 400 | | | |
| 2 | 9,000 | 3,600 | 400 | 900 | | |
| 3 | 14,000 | 5,600 | 450 | 1,000 | 2,000 | |
| 4 | 20,000 | 8,000 | 500 | 1,100 | 2,200 | |
| 5 | 28,000 | 11,200 | 550 | 1,200 | 2,400 | 5,000 |
| 6 | 40,000 | 16,000 | 600 | 1,300 | 2,600 | 6,000 |

A charter costs half the deed. A steward is free but costs upkeep. Bonds
pay for chapel rooms, as they do for the Chapel of Bonds.

### Co-op and saving

Holdings belong to the realm. Any knight can propose a choice at the court
or a room in the cross-section, and every knight present taps to agree
(Mario Party's vote). Holdings are saved beside the treasury in
`<island>.realm.json`, checked as data, never code.

---

## Order of work

1. **Arms feel pass.** Speeds, shapes, hits that land, co-op scaling, and
   the feel ledger. The A/B rack goes in the Training Yard. Small, and it
   answers the friends' complaint first.
2. **Folly engine,** with the Wax Maze (2D, cheapest) and the Keg Gantry
   (side-on halls already exist).
3. **The Shaft Run.** Time world3d first, then the balls.
4. **Holdings:**
   - the court tree;
   - the cross-section;
   - twelve rooms across four families;
   - stars and income;
   - then events, raids and the remaining families.
