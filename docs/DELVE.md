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
   - **where to begin**: the Crypt or the Mines;
   - **who comes along**: alone, a friend on this keyboard, or friends on
     their own angelX (see *Playing with friends*);
   - **voices**: on or off.

   Enter begins the run, and the game expands.

`/dungeon start` skips the walk and the intro.

The host plays **inside the expanded cockpit mini-viz**. No browser is needed.

- **F4** or clicking the mini-viz expands a run; **F6** is an alias.
- **Esc** returns to the harness composer, preserving its draft and cursor.
- While coding, the mini-viz shows the current room. Solo play pauses.
- While a friend is seated, Esc turns the host into an invulnerable grey
  statue and the friends play on. Returning brings the host back to the party.
- The expanded view shows harness working/queued/done/approval status.
- Private workspace checkpoints under `.angel/dungeon/session.json` preserve the
  run, checked gear, and invite across orderly exits/rebuilds. Checkpoints are
  also written every three seconds. `/dungeon off` removes the checkpoint.

## The Undercroft: home

Every delve begins at home, in the Undercroft: the company's cellar under the
Delve's gate. The **Winding Stair** in its middle goes down to the first
floor (or to a landing further down; see Wren's map). A finished delve
(**R** after a victory or a fall) comes home again.

The Undercroft is built out with the spoils the party carries up. Each
station has an engraved plate in front of it: stand on it to read its
ledger (the next rung, what it does, and its price), and **hold F** to buy it
from the realm's treasury. A tap in passing buys nothing. What is built
stays built for every delve after, and friends see the same cellar.

| Station | Keeper | What each rung does |
|---|---|---|
| Tobbin's forge | Tobbin | +8% damage for every knight, up to +40% (a *Tobbin's Edge* card in each deck) |
| Blaise's hearth | Old Blaise | +10 health for every knight, up to +40 |
| Tobbin's rack | — | Knights take a potion down; then a bomb more; a second potion; a thunder scroll |
| Chapel of Bonds | — | A fallen knight rises again at 40% health, once a delve; then once a floor. Paid in bonds |
| Wren's map table | Wren | A landing at floor 2, then floor 3, once the party has reached them. Tap F at the stair's mouth to choose |

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
calls out as the party comes in. Stand by the cage and hold F to work its
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
to read it; hold F to buy. His stock is, he insists, legitimately acquired.

## Sir Ector's lessons

Knights learn by fighting. Everything the party fells is experience for
each knight in it, kept by the realm under the knight's name, and every
level a knight reaches is a lesson waiting with **Sir Ector**, Arthur's
foster father, who keeps the south-west corner of the Training Yard.

Two lecterns stand before him, a talent on each, a choice at every
level. Stand at one to read its lesson and hold F to learn
it. A lesson learned goes down the stair with that knight as a held card,
every delve after.

| Lesson | Level (experience) | Left | Right |
|---|---|---|---|
| I | 2 (600) | Iron Hide: one more piece of mail | Keen Edge: 12% more damage |
| II | 3 (1,800) | Quickdraw: strike 15% faster | Fleet Foot: walk 12% faster |
| III | 4 (3,600) | Morningstar: a morningstar circles you | Arc Lightning: each hit sparks to another |
| IV | 5 (6,000) | Bloodthirst: 3 health back for each kill | Lady's Veil: each hit mends a friend by 2 |

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
on the plate before it and **hold F** (**The West Wing**, 300 gold, 20 ore and
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
  | Middle | The Iron Hymn | 2 more armour | 70 |
  | East | The Red Ballad | mends 3 more for every monster slain | 90 |
- **Beaumains, the kitchen knight,** waits by the west table. Sir Kay named
  him for his soft kitchen hands; in Malory he was Sir Gareth all along and
  told nobody. Stand on his plate and **hold F**: for **120 gold** he goes
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
drapes and an audience on the benches. Stand at the lever and **hold F**:
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
opened, a full Trophy Hall: thirty-eight moments in all,
each earned once per realm. A new one drops a banner across the view
(**NEW ACHIEVEMENT!**, its name, the Herald's line about it, and its reward)
and puts a box in the **Herald's coffer**, on its pedestal in Dame Fortune's
hall.

Stand on the coffer's plate and hold F to open the next box before the
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

  Rooms cleared with two knights standing earn each of them a **bond**, so
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
  realm keeps who wished and who paid.
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
connection — until **G** wakes the knight where it stood. The key rests five
seconds after waking, so it is no escape in a fight. A statue fights nothing:
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
| 5 | The Fungal Deep | A forest of mushrooms, moss, glowing spore pools; slimes, hobs, warboars, a shaman | **The Mother of Spores**, rooted and in no hurry: spore rings, thorn fans, burning spores |
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
| Shaman | Keep | Boxes the nearest knight in with four serpent wards that spit, then crumble | Step out of the box, or break a ward |
| Hexer | Crypt (floor 2 down), the Fungal Deep, the Unknown | Keeps her distance, spits, and every four seconds looses a slow green bolt. A knight it touches is a frog in a tiny helmet for two and a half seconds: it hops at seven-tenths pace and can do nothing else. Then no hex takes for three seconds | Roll through the bolt, raise a shield to it, or let a friend cover the frog |
| Loot goblin | anywhere, now and then | Runs with a sack of gold and slips away after twelve seconds | Catch it: it spills four gold and a prize |

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
receive the same mirror/state; an invitation opened in a browser now has an
**authenticated read-only** PNG and faction/resource table. Browser play still
requires `/dungeon join <link>` in angelX. No client can submit a political
transition or defeat assertion.

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
ring, spiral or ravage), a rage threshold, spoils it drops, and art up to 32
by 32 inks. The whole fight is held under 14 hostile shots a second; a
ravage throws up the floor, not shots, so it is not counted.

## Cards

Every pickup is a card, and a card is a short text file anyone can write:
`/dungeon cards new <name>`, edit `.angel/dungeon/cards/<name>.card`, then
`/dungeon cards reload`. **Tab** opens the card screen; keys **1–4** play
cards from your hand.

Every knight also has **three reusable spell slots**, shown under the hand.
**Z / B / N** cast them; a couch friend uses **5 / 6 / 7**. Spells are never
consumed: each slot recharges independently for the card's `cooldown <seconds>`
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
and shape. Picking up another ranged arm replaces a forged weapon.

The delve's ordinary arms remain bow **32 / 0.5s**, crossbow **75 / 1.1s**,
and handgonne **95 / 1.5s** (its ball bursts where it hits, half its damage to
all within 2.5). Example runes:

```text
name Bow
look gold arrow
bolt damage=32 speed=12 every=15 range=48
```

```text
name Knight's Blade
look steel blade
melee damage=30 reach=3.5 arc=150 every=16
throw damage=18 range=9 every=40
```

## Wishes

Win a room and an **Overclass window** opens: each knight standing gets one wish
until the next fight. A Sanctuary opens one too. **T** opens the Scroll of One
Wish. Pick a known wish with **↑/↓**, or say it in your own words ("triple wide
fire my shots", "fire faster", "heal my friend when I hit"). The ONE WISH box reads
it back; sealed, a known wish is yours at once, in the running game, and every
friend's screen shows it with the next tick. The same wish granted again climbs
its ladder: Triple Volley goes from every 5th shot three wide, to every 3rd, to
every shot.

Known wishes live in the **phrasebook**, plain data anyone can grow:
`cockpit/assets/dungeon/phrasebook.txt` (the delve's own) and
`.angel/dungeon/phrasebook.txt` in your workspace (yours; an entry with the same
id replaces the delve's). The game reads both again within a quarter second of a
change, while it runs.

```text
wish volley | Triple Volley
say triple wide fire my shots | triple shot | fire three | spread my shots
tier volley 5 :: Every 5th shot flies three wide.
tier volley 3 :: Every 3rd shot flies three wide.
tier volley 1 :: Every shot flies three wide.
art
<optional icon: up to 16 rows of inks, `.` is black paper, then a blank line>
```

Each `tier` line is card effects (`;` between) and the line the scroll shows.
Effects a held card may carry, with their ranges: `damage`, `rate`, `speed`,
`pierce`, `shots`, `armor`, `vamp`, and the wish primitives `volley N` (every Nth
shot three wide), `bounce N` (off walls N times), `homing N` (curves toward
monsters), `chain N` (a hit sparks on to N more), `mend N` (each hit mends the
most hurt friend), `orbit N` (morningstars spin round you) and `burst N` (shots
burst, N% to all around). The card checker bounds every tier.

A wish the phrasebook doesn't know is **learned while you play**: angelX gets a
short brief to append one entry to the workspace's phrasebook, the game never
leaves the screen, and the reload grants it to whoever asked. A friend's wish
goes to the host's angelX the same way. Workspace `.card` files hot-load the same
way: write one while the game runs and it joins the book.

## Avatars

Uploads are bounded to 2 MiB and 2048 pixels per side. The checker requires one
readable figure, removes a flat background and stray specks, reduces the palette,
and generates a 32×32 sprite plus 128×128 portrait. Only normalized data is stored;
avatars do not affect combat. The shared arena renderer draws the sprite.

## Playing with friends

Each friend runs their own (private) angelX. The host types:

```
/dungeon_host --3
```

and gets one line per friend (up to three):

```
  friend 1:  /dungeon join http://100.x.y.z:8767/#<code>
  friend 2:  /dungeon join http://100.x.y.z:8767/#<code>
  friend 3:  /dungeon join http://100.x.y.z:8767/#<code>
```

Each friend pastes their line into their own angelX composer, and the Delve's
menu opens on the host over the entrance room. The host's game is the only
game; each friend's angelX holds a **live mirror** of it, streamed over one open
connection (the whole run when the floor changes, then each tick's changes,
about 2 KB), and draws it itself at 60 frames a second, blending motion between
the 30 ticks. A friend's own knight answers their keys at once (it is drawn
where their keys take it, and the host's next tick corrects it). They play with
the same keys, hear the same chorus and music, make wishes with **T**, and see
their own cards and the host's book with **Tab**. Esc returns them to their
composer; `/dungeon` goes back in; `/dungeon leave` goes home.

On Kitty (or Ghostty, WezTerm) the room is a picture the terminal places one to
one; other terminals get half blocks. macOS Terminal.app works, coarser, and
says so on the status line.

- Every seat has its own code: a line plays only its own knight.
- A friend's knight joins the party when they arrive, dressed as the next
  knight of the company, mid-delve or at the entrance.
- The host listens on its tailnet address when Tailscale is up, else on the
  local network; nothing is opened to the internet. `/dungeon_host --2
  <address:port>` picks the address (`ANGEL_DUNGEON_LISTEN` sets it too).
- A wish the phrasebook doesn't know becomes a background angelX turn on the
  host's machine, in the friend's words. Invite people you would let type into
  your angelX.
- `/dungeon invite` shows the lines again; `/dungeon kick` ends them all.
- Opened in a browser, a line only says to paste it into angelX. With
  `/dungeon_host --3 --view` (or `ANGEL_DUNGEON_BROWSER_VIEW=1`) it opens a
  read-only view instead: the host's current room, the floor and its chiefs.
  Playing still takes angelX.
- Codes survive a host restart: the checkpoint keeps them.
