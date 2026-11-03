# Templates and the content checker

New content in the Delve comes from many hands: the overnight lanes, Codex,
the genie writing a learned wish while a party plays, and the owner. So that
quality does not depend on who made a piece, every new wish, card, foe, room,
effect or pastime starts from a reviewed template and goes through one
checker that says, in plain words, what is wrong and how to mend it.

- The checker: `crates/delve/src/rules/content.rs` (`rules::content`).
- The templates: `cockpit/assets/dungeon/templates/`.
- From a terminal: `cargo run -p angel-delve --example content_check -- <file>...`
  (run it under `systemd-run` as the build rules say).
- In the suite: `tests/rules/content__tests.rs`.

## How each kind is made, and what checked it before

| kind | written as | checked by the game | what was missing |
|---|---|---|---|
| wish | a phrasebook entry (`phrasebook.txt`, a workspace's `.angel/dungeon/phrasebook.txt`) | `phrasebook::read_checked`: every tier through `cards::check`, the ladder rule (`ladder_fault`), safe for work; a learned wish's pictures graded (`artistry`) | nothing graded the delve's own wishes; nothing said a `say` was heard as another wish; a worn look bigger than its place was squeezed to fit without a word |
| card | a `.card` file (`cards/`, `cards::BUILTIN`) | `cards::check`: the vocabulary, numbers clamped *with notes nobody read* | texts cut at 64 characters in play; clamped numbers; no art grade |
| foe | a `.boss` file (`bosses/`) | `bosses::check`: numbers clamped, a stage over its shot budget slowed, with notes; guardians' suite tests (caps, charge-ups, phases) | nothing checked a file's art against the stature it stands at; notes unread |
| room | a quadrant in `rules/rooms/<pack>.rs` or a pack's own templates (`layout.rs`) | the hand-drawn rooms' rule, in a test only (`together_rooms__tests`) | the rule was not callable; the packs' own templates were never held to it |
| effect | `vfx` lines on a card or a wish's tiers (`rules/vfx.rs`) | `Layer::read`: bounds, a line that does not read set aside | nothing checked an effect against the moment the run keeps for it |
| pastime | code: `rules/pastimes.rs` and its module | the game's own tests per pastime | no spec to check before the code; nothing on its banner's width |

## One door: `rules::content`

`content::check(kind, id, text)` checks one piece and returns a `Report`:
its faults, each said plainly with how to mend it, and its pictures' grade.
`content::check_file(name, text)` takes the kind from the extension
(`.wish .card .boss .room .vfx .pastime`). Faults come in two levels:

- **refused**: the game turns it away (or, for a `vfx` line, sets the line
  aside). It does nothing until mended.
- **bar**: the game takes it, but a built-in piece is not let under it.

What it checks, on top of the game's own checkers:

- **Pictures**: graded by `artistry` (the same marks a learned wish's power
  comes from). Fine (58) or better for any piece; Masterwork (78) for a
  template. A fault names the weakest picture and its two weakest marks
  ("edge it darker than its inside; shade from a dark ink to a light one").
- **A worn look** no bigger than its place on the knight (head 10x8, face 8x4,
  body 12x6, shoulders 16x5, back 14x10, left 8x12, right 8x16, feet 12x3,
  `looks/fit.rs`): bigger is squeezed nearest-neighbour and loses pixels.
- **A foe's stature** (`content::stature`, the renderer's own numbers): its
  art blown up at most twice to stand as a chief (24 px) or a guardian
  (32 px), and no wider than its strike by half again. Skipped for foes the
  field draws from art of their own (the library's, the Ripper's, the Mire
  King's, the Mother's).
- **A foe's fight**: a charge-up (`windup`) anyone can read, a `phases`
  chain, each stage inside its shot budget as written.
- **An effect's timing** on the motion foundation (docs/MOTION.md): a layer
  ends inside the moment the run keeps for it (`vfx::kept`, `vfx::fits`): a
  loosing and a hit 20 ticks (life 16 at most for a hit, at a tier-five
  scale), a kill 14 (life 9). One layer a moment and form.
- **Words**: a wish's name short enough for `<Name> III` on a card (18); a
  card's text and every scroll line inside 64; three ways to say a wish, and
  each `say` heard as this wish and no other (`phrasebook::find_in`).
- **Numbers as the game uses them**: any note the game's checker would make
  (a number held to its bound, a volley slowed for its budget, a weapon's
  `every` left to its default) is a fault: write the number the game uses.
- **Rooms**: `rooms::fault` (doorway lanes open at their doors, the middle
  3x3 open, every floor tile reachable, 55% floor) on a quadrant of a
  room's size.
- **Pastimes**: a `.pastime` spec (`name how room start time need pays`):
  the banner and its seconds within 200 pixels, a round of 15 to 300 seconds,
  something to win, knights started on the floor, and score only (never
  gold). Every built-in game's descriptor is checked the same way
  (`Pastime::of`).

`content::builtin()` checks every piece the game carries: the phrasebook's
wishes, the book's cards and the recipes their words give them, every `.boss`
file (the population, the checked catalogue, the flamingo, the dreads), every
room quadrant (each pack's own and its hand-drawn), every pastime.

### Running it

    cargo run -p angel-delve --example content_check -- my-wish.wish my-foe.boss
    cargo run -p angel-delve --example content_check -- --builtin        # what is under the bar
    cargo run -p angel-delve --example content_check -- --builtin --all  # every report
    cargo run -p angel-delve --example content_check -- --template foe-chief.boss > my-foe.boss
    cargo run -p angel-delve --example content_check -- cockpit/assets/dungeon/phrasebook.txt

A report reads like this:

    wish blob: under the bar, drawn Crude (32 of 100)
      - [bar] 1 way to say it; give it at least 3 (the player's own words and others they might use)
      - [bar] its pictures grade Crude (32 of 100; Fine from 58); its shot look scores 32: fill more of its frame (12 pixels or more on its long side); shade in 4 to 6 inks

### In the suite

- `every_template_passes_with_nothing_to_say`: every template clean, its
  pictures Masterwork, one template of every kind.
- `every_built_in_piece_is_at_the_bar_or_flagged`: every built-in piece at the
  bar or in `FLAGGED` with why; nothing built in refused. A new piece under
  the bar fails it; mending a flagged one fails it until its line goes.
- `every_content_file_on_disk_is_checked`: every `.card` and `.boss` file is in
  a catalogue (an orphan is never checked), every template listed.
- `a_fault_is_said_plainly`, `every_recipe_the_generator_makes_ends_inside_its_moments`,
  `the_stature_numbers_are_the_renderers`, `a_templates_example_is_its_piece_alone_and_reads_the_same`,
  and the Drowned Reeve's own (`the_drowned_reeve_is_met_in_the_drowned_archive`).
- `ANGEL_CONTENT_CENSUS=<file> cargo test write_content_census -- --ignored`
  writes every report.

## The templates

Each is a real piece that passes with nothing to say, its pictures at
Masterwork, with `#` lines at its top: what to change, what to keep, the bar.

| file | what | start here for |
|---|---|---|
| `wish-worn.wish` | Spiked Pavise: a shield worn at=left, armour then health | gear: helms, shields, pauldrons, boots |
| `wish-shot.wish` | Bigger Arrows: a `draw shot`, size then a shove | anything that changes the shots |
| `wish-spell.wish` | Golden Devils: a summoning spell, cooldown rising with the count | spells, summons |
| `card-find.card` | Hunter's Whetstone: a found hold card | chest and drop cards |
| `foe-chief.boss` | The Brute Chief: a camp's leader, a Wanted outlaw (art by Codex Sol) | chiefs, outlaws |
| `foe-guardian.boss` | The Iron Warden: a floor's guardian, staged, two charge-ups (art by Codex Sol) | guardians |
| `room-small.room` | The Split Nave: a one-screen quadrant, 11 by 6 | fight rooms |
| `room-great.room` | The Colonnade Court: a party's hall, 15 by 9 | great halls |
| `effect-shot.vfx` | frost: trail, loosing, hit, ring, kill, aura | a shot wish's magic |
| `effect-spell.vfx` | holy: a cast layer first, rays and stars | a spell's magic |
| `effect-worn.vfx` | embers: a worn aura and a soft trail | a worn wish's glow |
| `pastime.pastime` | Bucket Brigade: a pastime's spec | a new pastime, before its code |

The wish templates are the three examples the genie's lesson brief teaches
by: the host reads them (`dungeon_reforge::lesson_examples`, through
`content::example`, the `#` notes aside), so a wish the genie learns while a
party plays starts from the same reviewed pieces, card art and all.

### The quality bar, kind by kind

- **Pictures** read at 1x first (docs/MOTION.md): one silhouette, a dark
  outline against the floor, lit from the upper left, 4 to 6 inks a material,
  one accent, no specks. The grade is the game's own (`artistry`).
- **Rungs** (docs/LADDER.md): everything here is drawn at rung 16, the game's
  own grain, and a foe stands at 1x by `stature`. A finer drawing of a piece
  is the ladder's (`figure_keys`), keyed by name; a foe's key would be
  `foe/<kind>/<clip>/<frame>/<colours>/<facing>`, not built yet. Draw the 16
  well first: it is what every seat sees until a pack says otherwise.
- **Motion** (docs/MOTION.md): an effect is timed in ticks and ends inside
  what the run keeps; overshoot then settle (`back`, `spring`); a burst at
  full on its first frames where it lands an impact.
- **Fights**: a foe telegraphs (a charge-up with a cry), opens gentle, builds
  by stage, stays inside its budget as written.
- **Words** fit where they are shown, the same everywhere: the game is played
  in public, everything safe for work.

## Adding a piece

1. `content_check -- --template <name> > mine.<ext>` (or copy the file).
2. Change what its `#` lines say to change; keep what they say to keep.
3. `content_check -- mine.<ext>` until it says *at the bar*, nothing else.
4. Look at it in the game, at 1x and zoomed (below), before calling it done.
5. Put it where the game reads it:
   - a wish: an entry in `phrasebook.txt`, its card art in `wish-art/<id>.art`
     and `phrasebook::WISH_ART` (every stock wish draws its own art);
   - a card: `cards/<id>.card` and `cards::BUILTIN`;
   - a foe: `bosses/<id>.boss`, `bosses::CHECKED`, and a place it is met
     (`encounter_catalog` natives for a pack's chiefs and outlaws; the foe
     art golden takes its entry through `write_approved_foe_hashes`);
   - a room: its pack's `SMALL` or `GREAT` in `rules/rooms/<pack>.rs`;
   - an effect: its lines on the card, or on every tier of a wish, `;` between;
   - a pastime: once its spec passes, its code: a `Game` variant and its
     arms in `rules/pastimes.rs` (`ALL`, `room`, `name`, `start`, `time`,
     `need`, a `Records` slot and its `valid`/`repair`/`merge`), its module in
     `rules/pastimes/`, its board line (`how`) and intro (`intro_board`) in
     `render/overworld/arena/pastimes.rs`, and its tests.
6. The suite holds it: `cargo test -p angel-delve rules::content`.

## What the checker found in the built-in content (2026-10-29)

The first census, of 378 built-in pieces (93 cards and the 93 recipes their
words give the ones that write none, 42 wishes, 36 foes, 95 room quadrants,
19 pastimes):

- **Effects**: every recipe the game makes from a card's words threw its kill
  burst for 18 to 23 ticks (up to 33 at a top tier) while a death is kept 14,
  so every kill burst was cut off in flight; the longest hits by a tick (122
  layers). Mended: the generator fits its hits and kills inside their moments
  and throws the kill harder and shorter, and the field fits any written
  layer inside its moment (`wishfx::play`).
- **Words**: 23 cards and 4 scroll lines were longer than a card keeps and
  showed cut mid-word in play ("Every knight's first less"). Rewritten.
- **A ladder that fell**: Bright Blade's scroll promised deeper cuts each
  tier, but its throw took the arm's 70 a second, so the swing was lowered as
  it climbed (30, 28, 24, 20, 16 by tier five). It now keeps 30 a swing and
  climbs in sweep, reach and range.
- **Numbers the game changed as it read them**: 8 guardians over a stage's
  budget, two spirals asking for less than a spiral fires, an inhale of 10
  for 8, four card numbers held to their bounds, four weapons' `every` left
  to default. Written as used (the same fights; the Flamingo's and the Memory
  Warden's spirals, which could not be, at the same pace inside the bound;
  Cinderjaw, over its rage budget by a rounding hair, flagged).
- **Rooms**: the Archive's stacks and the swamp's reed beds put a shelf and a
  clump in the room's middle 3x3, where the stairs and a guardian stand. Each
  moved a tile out.
- **Pictures**: 19 card icons and 2 wishes' card art graded Crude (22 to 57:
  specks, flat inks, no dark edge), and four worn looks drawn bigger than
  their place (Iron Skin's pauldrons 18 and 20 wide for 16, the breastplate
  7 rows for 6, the war crossbow and hand cannon 9 wide for 8), so squeezed
  nearest-neighbour as they were put on. Redrawn as the same things (Codex
  Sol to a brief carrying the bar, then reviewed at 1x): 86 to 100.
- **Still flagged** (`FLAGGED` in `content__tests.rs`): the Mire King's file
  has no charge-up (the swamp's last fight is its own: the owner's call), and
  Cinderjaw is over its rage budget by a rounding hair (written as the game
  uses it, its spiral would be past the spiral's bound, and every pinned
  trace would move).
- **Everything else** was at the bar: every hand-drawn room, every
  pastime's descriptor, every foe's stature, every wish's ladder.

After the mending, and with the two pieces made from the templates below,
378 of the 380 are at the bar.

## Made from the templates

- **Frost Fletching** (`phrasebook.txt`, from `wish-shot.wish`): "ice arrows",
  "frost shots" and their like, which the phrasebook did not know (they went
  to the genie every time). Chill to what wishes give by the second tier, then
  a little damage and bigger heads; an ice-headed arrow for its shot; on
  every tier the recipe of `effect-shot.vfx`. To see it: wish "ice arrows" in
  the Sanctuary, or film it: `ANGEL_MOTION_SHEET=<dir> ANGEL_MOTION_ONLY=wish-frost
  cargo test -p angel-delve --profile fast write_motion_sheet -- --ignored`.
- **The frost recipe** (`effect-shot.vfx`): flakes trailing, shards loosed, a
  shard burst and a ring eased back on a hit, a flake burst on a kill, flakes
  falling off a knight who wears it; every layer inside its moment.
- **The Drowned Reeve** (`bosses/archive-drowned-reeve.boss`, from
  `foe-chief.boss`, art by Codex Sol): the Drowned Archive's old toll-reeve,
  one of its natives: a camp fights under its name, and the Wanted hall puts
  up its poster. To see it: an Archive floor's Wanted hall (floor four), or
  `ANGEL_CONTENT_SHOTS=<dir> cargo test -p angel-delve --profile fast
  write_content_shots -- --ignored`.

## Not built

- The host does not run the checker on a learned wish yet: the genie's
  entries go through the phrasebook's reader (refusals) and the grade
  (power), not the bar. Its lesson brief could carry the bar's words.
- A foe's named rung keys (`foe/...`) for the ladder.
- Monsters (`EnemyKind`) are code with their sprites in code; a monster
  template would want them as data first.
- Pastimes are code; the spec template checks the descriptor, not the game.
