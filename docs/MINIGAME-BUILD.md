# Mini-games and follies: how to build one

Standing orders for the agent developing a surface pastime, a folly, or the
feel of a delve floor. The method is the one in [dunik's guide](https://x.com/dunik_7/status/2108538374650736660):
write the feeling down first, build the verb before anything else, and play
the result before showing it. The stack in that guide (one `index.html`,
Three.js, Supabase, a public link) is a different product. This game already
has a world, a 30 Hz sim, and a host.

Read this before every change to a game. Also read the rules already in
force:

- [DELVE-TASKS.md](DELVE-TASKS.md) — the operator's working rules, and the
  list of surface games that are not finished.
- [DELVE-NEXT.md](DELVE-NEXT.md) Part 1 — arms and hits. Part 2 — follies.
- `crates/delve/src/rules/pastimes.rs` — the pastime contract:
  medieval homages, host-authoritative, rounds pay a bounded purse.

The privy is the model for a finished mini-game: short, funny, tied to the
rest of the realm, and readable at a glance.

---

## The one file

A game that starts from "make me a surf game" becomes a grey box on a grey
ramp. Before the first line of code, write the brief and keep it at the top
of the new module. Re-read it before every later change. When the code and
the brief disagree, fix the brief first, then the code.

```
# <Our name for the game>

## The feeling
One sentence: the memory this borrows, told the way you would tell a friend.
The moment a knight must feel in the first 10 seconds:
<one physical moment, e.g. "the shield tips and you catch it at the crest">

## The loop (one sentence)
<what you do, what the game does back, how a try ends>

## Words on the title card
<two or three plain words. nothing else.>

## Who plays
Every seated knight, at once, on their own screen.

## Controls
Move, F, E. Spells stay on 1 to 4. Name only the buttons this game uses.

## The five numbers
Each one is a fixed integer in ticks or room-units. HZ is 30.
1. <name> = <start>  — what it does to the feeling
2. ...
5. ...

## Fail and retry
What a miss looks like. How a knight is back in the loop in under 1 second.

## Version 1 is only
One room. One verb. All seated knights. A start and a finish.
No second mechanic, no new art pass, no ante screen, until this is fun
with simple shapes.

## Pays
<which bounded purse: a pot, a laurel, a feat, a mark. name the cap.>
```

Then, before writing the implementation, answer in the module comment:

1. What the knight feels in the first 10 seconds.
2. The five numbers, with starting values and units.
3. What version 1 leaves out.

Build that. Step the sim. Open the render. Fix what is stuck, unreadable,
unwinnable, or leaving a seated knight as a spectator. Show the third draft.

---

## Feel is the product

Name the memory, then ask for the mechanics that create it. Write ten lines:

| # | What the knight does | What the game does back | The number that controls it |

Rank them. Version 1 builds the top three. The other seven wait.

A few memories already named for this realm:

| Memory | The part that is the product | Ours |
|---|---|---|
| A marble run | weight, a lean, a drop you chose | the Shaft Run, a knight inside their own ball |
| A barrel pile | the jump you just make, the thing rolling at you | the Keg Gantry |
| A maze chase | one hunter on you, one cutting you off, pellets running out | the Wax Maze, wax and wraiths |
| A mine cart | the beat of the jumps, the switch you hit | the Ore Cart |
| Air on a board | speed you build by steering, not by the ramp art | Shield Surf |
| A round-based survival | round 1 is easy, the late round is panic | a delve floor: more bodies, more shooters |
| A trick line | the flick, then the score | the Jester's Half-Pipe |

Borrow the trope. Use our knights, our rooms, our names.

---

## Build order

Do these in order. A game with a podium and a bad verb is a dead game. A
game with a good verb and no podium is still a game.

### 1. The verb, in an empty room

Build the player body and an empty test room. Every seated knight has the
verb on their own screen. Movement, balance, the jump, the roll, the aim:
whatever the top-ranked mechanic is, it has to read within 10 seconds.

Put the five numbers where a playtester can change them without a rewrite:
named constants in the module, one comment each saying what sliding it does.
Starting values are proposals. Once the operator picks a number from play,
leave it. Report a number that plays wrong. Change it when told.

Build nothing else in this step.

### 2. One room

One course, one shaft, one maze, one shore. It starts easy and ends hard.
Simple shapes, original textures, a start and a finish, a clock in ticks.

Play it five times, including with two knights and with a full table. Write
down where a new player gets stuck, and which knight had nothing to do.
Fix those before adding anything.

### 3. The challenge

Add the thing that makes a failed try want another try.

| Kind of game | The challenge |
|---|---|
| A folly or a pastime | a harder beat, a tighter window, a longer line, a rival who is also a knight |
| A delve floor | more foes and more shooters as the floor goes deeper |

Round 1 stays readable. The late try is the panic. Co-op that feels easy
gets more pressure. Monster health and damage stay where they are.

A miss returns to the loop in under a second. No menu between tries. A
folly lasts 60 to 90 seconds, then the podium, then the delve.

### 4. Juice

Do this only after step 2 is fun with simple shapes. Art on a dull loop
stays a dull loop.

Add feedback and nothing else. The list already sized for the 30-tick sim
is in DELVE-NEXT Part 1 ("Hits that land"):

- hit-freeze on the foe, per foe, so the shared sim never pauses
- knockback sized to the hit
- a spark in the shot's colour
- a shake on a heavy hit and on a kill
- a burst and a small gold fountain on a death
- a payoff when the room's last foe falls: hostile shots in the air become
  coins and fly to the nearest knight

A pastime uses the same idea at its own scale: a crest flash, a wipeout
that reads, a cheer you can hear, dust when a ball hits. Generate short
cues from the game's existing sounds. Keep the frame rate. Show a before
and an after of the same moment.

### 5. A reason to come back

This is the leaderboard step, in the realm's terms. Add it after the verb
is good.

- A result the table can see: place, time, or pot.
- A bounded purse into the shared economy (gold, bone, marks, horses, a
  feat). Surface rounds stay a purse with a cap. They are not a faucet.
- For a folly: each knight antes 10% of the gold they carry. First takes
  half, second a third, third the rest, plus the floor bonus. The winner
  takes a Laurel. At the end of the delve the most Laurels picks the
  hoard's cards first.
- A saved best on the host, so the next try has a number to beat.
- A line after the run that says where you placed.

A score the guest invents is not a score. The host computes the finish,
the pot, and the best. A "beat this" between friends is the next seated
knight, or that saved best, on the same room.

### 6. The next room

A second map, a second folly, a harder shaft. Same verb. A new mechanic is
a new brief and starts again at step 1.

Fifteen small games people return to beat a single game with fifteen
systems. The surface list in DELVE-TASKS §2 and the folly table in
DELVE-NEXT Part 2 are that set. Finish one before opening the next.

---

## Systems

Use these. A new game gets a file and a brief. It does not get a new
engine, a new clock, or a new economy.

### Surface pastimes

Live under `crates/delve/src/rules/pastimes/`. One file, integer
state, `Serialize` / `Deserialize`, a `valid` check, a `step` that takes
`Input`. Register the game on `pastimes::Game` (`name`, `room`, `start`,
`time`, `need`) and give it a `Score` inside `Records`. Drawing lives with
the arena stage, in `crates/delve/src/render/overworld/arena/pastimes.rs`. The module header is the
contract: host-authoritative, our own names, a bounded purse.

Current games and the rooms they belong to: Broadsheet Boy, Barrel-Board
Derby, Turnip Keepy-Uppy, Washtub Regatta, Cheese Chuck, Hobbyhorse
Hurdles, Squire Sled Slalom, Chamber-Pot Put, Wheelbarrow Cross, Shield
Surf, Belfry Bat-and-Ball, Jester's Half-Pipe, Cabbage and Roll Racing,
Trencher Toss, Carnival Duck Range. DELVE-TASKS §2 says which of these
still play as one knight plus a crowd. Finishing one means the whole table
plays.

### Follies

A folly is a room inside a delve that plays like another game for 60 to 90
seconds. The door wears a mummers' arch so the floor announces it.

The shared shell, once, owns:

- the arch
- the title card (the two or three words)
- the ante and the pot
- the podium
- the Laurel
- the return to the delve

Each folly owns its state, its input mapping, its draw, and its tests.
One file in `crates/delve/src/rules/follies/`. The planned set and the flagship
(the Shaft Run, one themed ball per knight) are in DELVE-NEXT Part 2.

World3d steps at the delve's viewport. A marble race needs 30 frames a
second. If the shaft cannot hold that, draw it as a tilted floor and keep
true 3D for the slower Griffin Run.

### The delve around them

A mini-game is a room in a world, so the surrounding rules apply:

- One goal line, plain, and an arrow. "To the Gate." "Down the stairs."
- No walk between places is a long stretch of nothing.
- A sign at the door: the place's name and a picture of what happens there.
- E interacts, F fires, spells on 1 to 4.
- Instructions on a card are two or three words.
- Each knight has their own screen. Races follow each knight. The podium
  is the shared view.
- What you win above helps below, and what you carry up opens something
  above. Gold, bone, marks, and horses already have sinks: the home, the
  barony, the stable.

### Arms, when the game is the delve itself

The same order: five numbers, one room, then pressure, then juice. The
measured problems and the proposed shapes (stream, rail, fan) are in
DELVE-NEXT Part 1. Shots are fast. Theirs stay slow. Each arm is a shape.
Power is something you can see. A Valour pip is filled by hitting and by
grazing, not by waiting out a long rest.

Judge "not fun" with the feel ledger (shots, hit rate, time to kill, dead
air, skills per minute, each knight's share of the kills) and the A/B rack
in the Training Yard. The operator picks the number.

### Tests

Every game ships with tests that step the sim:

- `valid` rejects a broken save and `repair` mends one.
- A full try can be won, and a miss can be lost, with scripted input.
- Two knights both have a move that changes the outcome. A spectator is a
  failing test.
- The purse pays once, and a repeated finish does not pay again.
- Restart lands back in the loop inside a second of ticks.

Open the render before calling the art done. A test that passes on numbers
and a screen that was never looked at is half done.

---

## The line

| Build this | Leave this out |
|---|---|
| The mechanic: a wave, a strafe, a rolling keg | The original game's name on our screen |
| The feeling, described in the brief | The original maps |
| Our rooms, our knights, our balls and crests | Ripped models, sprites, or sounds |
| Our name for the game | Their logo |
| "A room in the spirit of" inside the brief | Charging anyone for someone else's game |
| Our own engine and our own files | Their game files, their UI, their music |

A homage that plays well and uses their assets is one letter away from a
takedown. The version we can keep growing is the one in our realm.

---

## The contract

This can:

- turn one remembered mechanic into a room the table can play
- give that room a purse, a Laurel, and a best the host remembers
- make the room feel right once the five numbers are picked from play
- sit in the realm so a win upstairs changes the next delve

This cannot:

- rebuild a long commercial game. Scope is one verb and one room.
- pick the final feel numbers. Propose them, play the loop, and stop for
  the operator on the hour of tuning.
- make a dull verb fun by drawing it better. Stop at step 2 and say so.
- protect a room that uses someone else's names, maps, models, or sounds.
