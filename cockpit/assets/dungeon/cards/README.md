# Delve cards

Every pickup in the delve is a card. A card is a small text file, so anyone
can make one. Copy a file from this folder to see how it works.

## Make a card

1. In the cockpit, run `/dungeon cards new Frost Arrows`. This writes
   `.angel/dungeon/cards/frost-arrows.card` in your workspace.
2. Edit the file: change its name, kind, effects and art.
3. Run `/dungeon cards reload`. Errors come back with line numbers.
4. Your card now drops from monsters and chests. `/dungeon card <file>` also
   drops one copy at your feet straight away.

Press **Tab** in the delve to open the card screen. It shows your hand, your
weapon and your held cards, then every card in the book.

## The format

```text
name   Ember Quiver          # up to 22 characters
kind   hold                  # take | hold | play | arm
rarity rare                  # common | rare | relic
by     Jordan                # who made it
text   Arrows that smoulder in the dark.
drop   4                     # weight in monster drops (0 = never)
chest  3                     # weight as a chest prize
damage 25                    # up to three effects
art                          # up to 16 x 16 inks; `.` is black paper
..6...6.....
.565.565....
.pPPPPPPPp..
```

### Kinds

| Kind | When it works |
|---|---|
| `take` | The moment you walk over it. It stays on the floor while it would do nothing, such as a heal at full health. |
| `hold` | All run. It goes into your deck. |
| `play` | It goes into your hand (four slots). Keys 1–4 play it. |
| `spell` | First empty reusable spell slot (three slots). Z/B/N cast; its own cooldown recharges it. |
| `arm` | It becomes your weapon, and your old weapon drops. Use `arm bow\|crossbow\|handgonne`, or [Forge](../../../../docs/DELVE.md) rune lines (`bolt`, `spread`, `melee`, `look`). |

### Reusable spells

Each knight has **three spell slots**, shown on the HUD under the hand.
**Z / B / N** cast slots 1 / 2 / 3 in the host cockpit and a friend's cockpit. A second knight on the same keyboard uses **5 / 6 / 7**.
Spell slots start empty. Picking up a spell fills the first empty slot;
spells do not consume hand space and **are never used up**. Each slot recharges
independently in simulation time (cooldowns pause with solo play).

```text
name Ember Ring
kind spell
rarity rare
cooldown 4
cast ring
ward 1
text Fire remembers the hand that made it.
```

`cooldown <seconds>` is required on spells only, in whole seconds clamped to
**1–120**. A spell has up to three effects, using the same caps as play cards:
**heal, bombs, nova, ward**, and the spell-only **cast** effect. Reusable spells
cannot mint gold/materials or permanent health/held bonuses.

`cast bolt` launches a straight bolt; `cast ring` launches twelve bolts radially;
`cast meteor` launches one heavy, bursting ball along the knight's aim. Every
shape starts at the knight and inherits their held-card damage scaling, pierce,
bounce, homing, chain, mend and burst traits, including ownership. Knight
bonuses such as vamp and orbit continue to work normally. All shapes share the normal projectile cap.

While the game runs, write a checked `.card` file into workspace
`.angel/dungeon/cards/`. The host scans twice a second (even while solo play is
paused for editing); a new spell goes directly into the host's **first empty
spell slot**, with a pickup sound, notice and burst flourish. When all slots are
full it drops at the host's feet instead. Unchanged files do not duplicate cards;
editing an equipped spell updates its rules without resetting its recharge.
Malformed or incomplete writes are ignored and retried; use `/dungeon cards
reload` for line-numbered errors. Friends receive slots, cooldowns, shots and
changed card rules through the host mirror. Cards remain checked data, not code.

### Effects

`/dungeon cards` prints this table from the code.

| Effect | Range | Cards | What it does |
|---|---|---|---|
| `heal` | 5–100 | take, play, spell | Restores health. |
| `blink` | 4–14 | take, play | Moves the knight N steps along its aim at once; walls stop it (the Fae Dagger). |
| `immune` | 2–8 | take, play | N seconds in which no hex, chill, web or hook takes (the Pendragon Sceptre). |
| `censer` | 10–80 | take, play | Mends every knight within eight steps by N (the Censer). |
| `gold` | 10–500 | take, play | Adds to the score. |
| `bombs` | 1–3 | take, play, spell | Adds bomb charges. |
| `max_hp` | 5–40 | any | Adds maximum health for good. |
| `nova` | 10–120 | take, play, spell | Damages every monster in the room. |
| `ward` | 1–5 | take, play, spell | Makes you untouchable for that many seconds. |
| `damage` | 5–50 | hold | Shots and swings hit N% harder. The deck caps at 100. |
| `rate` | 5–40 | hold | You shoot and swing N% faster. The deck caps at 60. |
| `speed` | 5–30 | hold | You walk N% faster. The deck caps at 40. |
| `pierce` | 1–2 | hold | Shots pass through more monsters. The deck caps at 3. |
| `shots` | 1–2 | hold | Adds shots in a fan. The deck caps at 2. |
| `armor` | 1–2 | hold | Each piece turns aside 3 damage per hit. The deck caps at 3. |
| `vamp` | 1–10 | hold | Mends N health per monster slain. The deck caps at 10. |

Numbers outside a range are clamped, with a note. Unknown words are
rejected. Cards are data, never code.

### Inks

Art uses the realm palette, one character per colour:

- **Greys:** `k K Z X g j G J h i H`
- **Greens:** `f D F E e l N m L A M y C Y`
- **Wood:** `n b I B p P r R o O t T`
- **Stone:** `s x S u U v V W q Q z`
- **Signal:** `0 1 2 3 w` (blues), `a 4 @ 5 6 $ c 9` (golds), `8 7` (reds)

Each group runs from dark to light. Signal inks glow, so keep them for the
part that matters, such as a flame, a gem or a drop of blood.
