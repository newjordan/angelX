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

- **Waves.** Fight rooms hold back one wave per floor of depth. The guardian
  rooms hold one escort wave. Each wave comes through a doorway that burns
  for a second first, and never one that a knight is standing near. The room
  stays sealed until the last wave is down, and a thinned room calls the next
  wave early.
- **The Crypt's spike plates** rattle, then strike knights and walking
  monsters alike. A dash passes through untouched.
- **The Mines' loose ceiling** sheds rocks near the knights. A red ring closes
  where each one will land. A landing rock crushes monsters too, so lure them
  under it.
- **Dragon Keep's wall vents** glow, then breathe a line of fire across the
  room. They never fire across a doorway's lane.
- **Mimics.** About one treasure chest in four has teeth. It hops, bites and
  spits coins, then coughs up the treasure it was pretending to be.

## Bosses

Each floor's stairs room has a guardian; the way down stays barred until it
falls, and it scatters its spoils as cards. The Crypt's **Waxen Warden** and
the Mines' **Cinderjaw** were designed and drawn by Muse
(`muse-spark-1.3-contributor`) from a brief. Muse wrote the stats, the attack
patterns, the lore and the pixel art; its first draft had uneven art rows,
and it fixed them itself in a second pass after seeing its render. Dragon Keep
keeps the dragon.

A boss is a checked `.boss` file in `cockpit/assets/dungeon/bosses/`. It has
a movement (chase, drift, hover or anchor), up to three attacks (aimed, fan,
ring or spiral), a rage threshold, spoils it drops, and art up to 32 by 32
inks. The whole fight is held under 14 hostile shots a second.

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
- Codes survive a host restart: the checkpoint keeps them.
