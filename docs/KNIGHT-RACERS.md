# Knight Racers

Podracing for knights: two engines tow a chariot, six seats race, and a cup is
won over several courses. It is a Round Table adventure at the race gate east
of Lanternward (kingdom tile 128, 22): walk to it and press **E**. Hold **Esc**
to leave.

A lone knight gets five CPU rivals. Two or more knights race each other and the
rivals go home. Every seat sees its own race: its own camera, its own place,
its own lap clock.

## The game around the race

Walking into the gate opens the **title** over a CPU race. Any key goes on to the
**modes**. In the realm the menus use W/S and A/D, **E**, **F** or **Space** to pick,
**Q** (or a short tap of **Esc**) to step back; a long hold of **Esc** still leaves the
gate. In Glass Enter and Backspace work too.

| Mode | What it is |
|---|---|
| **Cup** | The three courses in turn, points 10-7-5-3-2-1, a purse for every place, a **podium** and a gold cup at the end with a prize and what winning opens. |
| **Single Race** | One race on any course (the Proving Run too), for two thirds of the cup's purse. |
| **Time Trial** | Alone on the road (no rivals) against the **ghost** of your best lap on that course and pod, drawn as a see-through pod with the gap in the corner. A first lap pays 400, a new best 200. Solo only. |
| **Practice** | The Proving Run, easy rivals, no prize. |

**Classes** set how hard the rivals race and what the money is worth: **Squire**
(rivals at 93% of their pods' top speed, and slower off the line), **Knight** (the
pods as they stand) and **Champion** (2% over, harder off the line and in a bend). The
pilots themselves run at 97.5% of their pod and draft, heat and slam on top of that, so
these numbers sit near one. Against a clean human run (`flow_tests::class_balance_table`:
the pilot with no slips in seat 1, six pod rotations on each course, 18 races) a Squire cup
is won 18 of 18 by 27 s, a Knight cup 13 of 18 and a Champion cup 7 of 18: a knight needs
the parts and the hands. The purses are
900/1500/2400 for first and fall away to sixth (`career::PURSE`), and the cup's own
prize is 3000/5000/8000 for the winner (`career::CUP_PURSE`).

**Winning a cup opens things** (the first time for each class): the Squire cup the
Knight class and the Ringwright, the Knight cup the Champion class and the
Bellfounder, the Champion cup the Lancewain. Winning all three races of any cup opens
the Gatehouse. The Warden and the Wyvern are yours from the start.

**Several knights** pick together: the host (the lowest seat) chooses the mode, the
class and the course; every seat picks its own pod (from the ones *it* has opened) and
the race starts when all are ready. The garage and the records are each seat's own
screens, opened from the mode list, and hold nobody up.

### The garage

Parts tune any pod you race: six slots (engine, chariot, running gear, exhaust, crest,
armour; paint and trim are not sold here) from the garage's catalogue
(`garage::parts`), each with a price (`career::WARES`). The racer every knight starts
with, Excalibur, is the stock: it changes nothing. A better engine adds a few percent
of top speed, plate makes a pod harder to stop and heavier (slower), a hellhound
engine leaps off the line. The numbers are the garage's own (speed, accel, handling,
armour, weight, each 0 to 100) turned into multiples of the pod's spec
(`career::tune_of`, `Car::tune`), so they apply on top of whatever the pods are.

### What is kept

Money, parts bought, pods and classes opened, cups won and the best race and lap on
each course (and by pod) are a knight's **career**, kept by name in the garage's ledger
(`garage::ledger`, `.angel/dungeon/garage.json`); the best time-trial lap on each
course and pod is kept as a ghost beside it (`garage::ghost`, five points a second).
The rules never touch the file: the host carries it through a `garage::Bank` that
sits a knight down with their career and writes it back the moment a race, a purse or a
purchase changes it. The Glass example keeps its own ledger in `--save FILE`
(default `.angel/podracer/garage.json`).

## Controls

| Key | Does |
|---|---|
| **W** / Up | gas |
| **A** / **D** (Left / Right) | steer across the road |
| **S** / Down | brake; held at a standstill with **A** or **D**, the pod turns about on the spot |
| **Space** | boost: more speed and thrust while the bar lasts |
| **E** | at the flag: the next race of the cup (or the podium after the last) |
| hold **Esc** | leave the race |

Boost burns the bar in about three and a half seconds and refills slowly. Empty
it and the engines **overheat**: no boost until they have cooled past a third.
A wall takes some speed and shakes the screen; slag, rockfalls, jets and slick
are real hits. Gold boost strips on the road push you past your top speed. Sit in a
rival's wake, a few lengths behind its chariot in its lane, and you ride its **draft**.
Every engine has its own heat, so there is more to a race than the bar: see
**The racer's law** below.

## The race

1. **The course card.** The name of the course, its laps and the length of a
   lap, written in the clock plate at the top (where the lap and the time go once
   the lights are out), over a high view of the whole grid. All six pods are in frame.
2. **The lights.** The light tree at the left of the grid fills one lamp a second and the
   count runs 3, 2, 1 while the camera swoops down behind your pod. On **GO** the tree
   goes green and you drive. Stand on the gas in the last half second, as the last lamp
   burns white, for a **rocket start** (twice the push for a second); stand on it earlier
   and you **flood** the engines (a third of the push and some heat).
3. **The laps.** A banner announces each new lap with the last lap's time. The
   last one is **FINAL LAP** in the header and a flashing banner.
4. **The flag.** Your place and time, and the rest of the field drives in.
5. **The results.** Every seat in order with its pod, time, gap to the winner
   and best lap, and beside it the cup points. **E** starts the next race.

## The screen

- **Top left** your place out of six. Everything that shows a place (this, the
  ladder, the tags over the pods, the minimap's blips) reads the same running
  order: finishers by time, then everyone by distance run.
- **Top centre** the course, the lap, the race clock, this lap's clock and your
  best lap. The name of a section of road shows under it as you enter it.
- **Top right** the minimap, drawn from the real road (forks too) with a blip
  in each seat's colour. Yours is ringed. Below it the **ladder**: the six
  seats in running order.
- **Over the pods** a tag with each rival's place and its gap in metres.
- **Left and right edges** an arrow in a rival's colour when one is alongside
  but out of the picture.
- **Bottom left** speed. The notch on the bar is your cruising top speed.
- **Bottom centre-left** the two engine gauges, **L** and **R**: each nacelle's heat, the
  amber notch at half and the white notch where it catches fire, and a brown scar for wear
  the squires have yet to patch. **Next to it** the rear cue: rivals within 70 m behind,
  in their lanes. The frame flashes when one is closer than 22 m.
- **Bottom right** the boost bar and the engine's state: BOOST, BOOSTING or
  OVERHEAT.
- **In the middle of the picture**, one call at a time by how much it matters: BLOWOUT with the
  squires' progress bar, an engine BURNING with its heat and "lift off the gas to cool it",
  SLAM! (and the side it came from lights up), ROCKET START or FLOODED, PASSED A RIVAL!, and
  an engine HOT warning. While you ride a wake a **DRAFT** meter shows over the pod and
  chevrons run along the road from your nose to the pod you are tucked behind.
- **WRONG WAY** flashes across the screen when your pod's nose has been held against
  the road's direction (or you are seen running back along the course). It clears
  once the nose has been along the road again for a moment. Under way a pod
  straightens to the road by itself; only a pod held still on the brake turns about.
- **The progress strip** above the bottom plates: every seat along the whole race.

The lens sits low and close behind the engines and tucks in as they run up. Its field
widens hard with speed and is kicked wider by a boost, easing back; it lags a steer and a
bend, so the pod swings in the frame; it rolls into a bend, dips and bounces on a landing,
rumbles at speed and shakes on a wall, a hazard and a near miss. The engines trail plumes
that lengthen and brighten with the burn, a boost lights with a flash, a shock ring and
a burst of flame, the road is thrown up behind them, walls throw sparks, a landing throws
dust, speed streaks stream past and the edges of the picture dim as the boost bites. The
kit is `knight_racers_game/fx.rs`; every mark is an ink of the realm on the 4x4 dither.

## The pods

A pod is a knight in a chariot towed by two engines, with a tether and a chain
to each engine and an arc crackling across the gap between them. The pod is the
garage's choice; the **colours are the seat's**: gold, red, blue, green, violet
and rose for seats 1 to 6, on the pod, its pennant, its tag and its minimap blip.

| Pod | Looks like | Drives like | Top / boost |
|---|---|---|---|
| **Warden** | twin iron tubes, a plumed knight | even in every way: the numbers every course was tuned on | 46 / 58 |
| **Bellfounder** | flared bells, horned helm | the biggest burst of boost, and it runs hot for it | 45 / 63 |
| **Ringwright** | standing hoops, a hooded rider | grips the road in a bend and vents heat well | 45 / 56 |
| **Lancewain** | long needles and a lance | fast on a straight, nervous in a bend, brittle at a wall, hot | 48 / 62 |
| **Gatehouse** | battlement blocks on a yoke, a crowned helm | heavy and steady: a wall costs it little, it runs cool | 45 / 56 |
| **Wyvern** | swept wings | light and quick off the line | 46 / 57 |

The tops sit within a few metres a second of each other on purpose: a pod's edge is how its
pilot uses its engines, and a pod in a wake gains more than the whole spread. The sim's `pods()` list sets the numbers (top speed and boost speed in metres a
second, thrust, grip, mass); seat 1 starts in the first pod, seat 2 in the
second, and so on. In the Glass example you choose your own.

## The cup

Win points by place in each race: **10, 7, 5, 3, 2, 1**. The points carry from
race to race and the results board shows the table beside the race result: each
seat's points so far and what this race added. **E** at the flag runs the next
course of the cup; after the last one the champion is named and the cup starts
over. The proving run is practice: it is not in the cup.

## The racer's law

Two engines tow every pod, and each has **its own heat**.

- **Gas heats a little, boost heats a lot.** Flat out for a minute and a half cooks an engine;
  a full boost bar is a third of its margin. A pod in a wake runs cooler.
- **Lifting off the gas cools the engines** (coasting fastest, braking a little less) **and the
  squires patch their wear.** An engine cannot cool below its wear.
- **Walls, slag, flame jets and hard slams dump heat and wear into the engine nearest the blow.**
  A light pod takes more of it than a heavy one.
- **A hot engine limps.** From 60% it gives less push, and a burning engine (75%) also lowers
  the pod's top speed and drags it toward its own side while under power: steering holds it, but
  the wall is that way.
- **At full heat an engine blows.** The pod skids to a stop and sits out about three and a half
  seconds while its squires patch the nacelle, then goes again on cooled engines. A blown pod is a
  wall in the road: the rivals steer round it.

**The draft.** Behind a pod that is moving, in the cone that opens behind its chariot (strongest
three to eleven metres behind its tail, gone by twenty-six), a pod builds a draft in under a second:
more push, up to 5.5 m/s over its top speed, and cooler engines. Pull out and the draft fades over
two seconds: that is the slingshot that carries the pass.

**Contact.** A pod is its engines and its chariot, each a box with the pod's mass. Boxes meeting
are pushed apart (the lighter pod gives more ground) and trade speed. A **side slam** (closing at
4.5 m/s or more) throws both pods on a kick that steering cannot cancel and heats and scars the
engines that met; a **nose into a tail** shocks the leader's pair of engines through the tethers. A
rub is racing: no heat, no shake. Nothing goes through anything: the boxes are longer than a pod can
move in a tick.

**The rivals.** Each CPU pilot reads the whole field: it takes a racing line (wide in, apex, wide
out), tucks into a wake, pulls out and boosts by on a straight, covers its line from a pod behind
it, lifts to cool a hot engine and spends its boost where it pays. Each pod's pilot has a character:
Lancewain is brave, late on the brakes and forever hot; Gatehouse is a door that shuts on whoever is
behind it; Bellfounder burns its boost; Ringwright is smooth and cool; Wyvern takes every gap. Every
pilot slips now and then (a late brake, a wide line, a twitch, a lift, a boost on a hot engine), and a
jumpy one may flood the engines on the grid. A rival runs at 97.5% of its pod's speed; one far
behind the pod the race is run against may run up to 4% faster, one far ahead 2.5% slower: a mild
band, capped, so a clean human run still wins clearly and the field still stays in sight of itself.

## Courses

The cup runs three big courses, two laps each, about six and a half minutes a
race. The road is authored, not looped: it climbs, crests, drops and banks, it
narrows, it forks, and there is a jump on every one. Beyond the road there is
real land (canyon walls, mountains, chasms, a moat), not a corridor.

| Course | Lap | Forks | The road |
|---|---|---|---|
| **Emberfall Foundry Run** (cup 1) | 9.5 km | 1 | Forge canyons, the Slagworks, a lava lake on chain bridges and a molten leap. |
| **The Glassway** (cup 2) | 9.1 km | 1 | Down the Mineshaft, through the crystal gallery, over the Broken Span and up behind Whisper Falls. |
| **Moatwalk** (cup 3) | 9.1 km | 2 | Lanternward by night: the harbour wall, the castle rampart over the moat, and a leap across the drawbridge. |
| **Lanternward Proving Run** (practice) | 2.8 km | 0 | The old circuit: one long straight, a cavern and a bowl. Not in the cup. |

Each course has a road surface of its own, laid in the road's own frame (`roadtex*.rs`: a
mark made for an edge stays at the edge on every bend and width) and lit by its world. Emberfall's
is black basalt in plates seven metres long with cracks that glow and thicken toward the
edges and a pool of light under each pylon, and the air over its lava and hot straights wavers
(`knight_racers_game/shimmer.rs`, a pixel or two, far dots only). The Glassway's is dark glass with sheens
that rush past, violet down the left edge and cyan down the right. Moatwalk's is setts under the
moon with a torch pool under every lamp, and over it a night sky: a moon in two halos, and a castle
with its keep and towers on the skyline. Where the land falls away beside the road the rail is
half a metre high and the lava, the moat water or the Glassway's mist lies a couple of metres
under the road, so the chase lens sees what a fall would be into.

A fork is a second road with its own hazards and strips; the minimap draws it
dimmer beside the main road, and a car on it shows on its own line. A jump
(a gap in the road) wants speed: too slow and the car drops into the chasm,
is set back up the road for another run at it, and loses a few seconds.

## Glass example

```
cargo run -p angel-glass --example podracer
cargo run -p angel-glass --example podracer -- --course emberfall --pod stag
cargo run -p angel-glass --example podracer -- --demo --seconds 12 --capture reports/podracer/glass.png
```

The example opens on the **title** and the whole game above: the modes, the
classes, the courses, the pods, the garage, the records and the podium. `--mode`,
`--class`, `--course` and `--pod` take part of a name and skip the menus. **N** in
the race skips ahead 15 seconds with the CPU pilot at the wheel, so the laps,
the results board, the cup table and the podium are a few key presses away (a cup
race is six and a half minutes). `--demo` drives the seat with the CPU pilot, shows
each board for six seconds and rolls the cup on. `--save FILE` keeps the career.

## Where it lives

| What | Where |
|---|---|
| the sim, the cup, the pod specs | `crates/delve/src/rules/knight_racers/` |
| the world around the road | `crates/delve/src/render/overworld/arena/knight_racers_world/` |
| pods, camera, HUD, race flow, lap clocks | `crates/delve/src/render/overworld/arena/knight_racers_game/` |
| the game around the race: modes, classes, menus, purses | `crates/delve/src/rules/knight_racers/{flow,career,tune}.rs` |
| the screens | `.../arena/knight_racers_game/ui/` |
| the garage's ledger, bank and ghosts | `crates/delve/src/garage/{ledger,store,bank,ghost}.rs` |
| the Glass example | `crates/glass/examples/podracer.rs` |

The walkthrough GIF (title, cup, race, podium, garage, the next cup open) and the
time-trial GIF come from `knight_racers_game/flow_proof.rs`
(`ANGEL_FLOW_PROOF=/dir cargo test -p angel-delve --lib flow_walkthrough` and `flow_trial`).

The proof reels (the grid to go, a boost, a pass, the final lap, the results)
come from the `podracer_reels` test; see `proof.rs` beside the pods for how to run it.

The polish proofs (a jump at close range, each fork's split and rejoin, a lap, a pod
turned about) come from `worldkit/polish.rs`:

```
ANGEL_PODRACER_POLISH=/dir ANGEL_PODRACER_COURSE=1 cargo test -p angel-delve --lib \
    worldkit::polish::jump_close -- --nocapture      # also fork_close, lap_chase, wrong_way_turn
```

The road, its edges and the sky alone (no pod, no HUD) come from `worldkit::shots` (`ANGEL_PODRACER_WORLD_SHOTS=/dir`, distances
in `ANGEL_PODRACER_AT`): `road_look`, `sky_look`, `zone_shots`, and `zone_list` (prints where each named
stretch, gap and piece is, with `ANGEL_PODRACER_TALLY=1`).

`ANGEL_PODRACER_COURSE` is 0 for the proving run, 1 Emberfall, 2 the Glassway, 3 Moatwalk.
The interleaved frame-time comparison is `worldkit::frame_time::frame_time_compared`
(`ANGEL_PODRACER_COMPARE=0,2`): each course timed at eight places round the lap, round
after round, so a busy machine slows them alike.
