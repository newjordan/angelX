# The Round Table adventures

Five co-op adventures for the whole table, registered end to end as stubs so
five builders can fill them in parallel, each in its own worktree, each
editing only its own files. A stub has a door on the kingdom map, a bounded
session that seats every knight, a labelled block scene, five placeholder
knobs and its tests. It has no mechanics. Read
[MINIGAME-BUILD.md](MINIGAME-BUILD.md) and the working rules at the top of
[DELVE-TASKS.md](DELVE-TASKS.md) first; this file is the recipe on top of them.

| Slug | Name | The idea (context for the brief, not a spec) |
|---|---|---|
| `lanternward` | Lanternward | A lock-on sword, shield and lantern dungeon adventure. |
| `moonfall` | Moonfall | A three-day town time loop; the party shares one clock and rewinds together. |
| `bannerpeak` | Bannerpeak | A side-view exploration platformer (side-view, not 3D): a hub below a mountain course, banners to collect. |
| `griffinwing` | Griffinwing | An on-rails co-op flight shooter; reuse the griffin flight in `race3d` where it fits. |
| `lightgallop` | Lightgallop | A momentum runner; reuse the race hall's seats, pot and podium. |

The template is the **Forest Road** (`rules/forest_road.rs`, commit
`b751cc2f1`): shared foes and checkpoints, health scaled per seat, a knight
who joins mid-run, entry from the kingdom map, a campaign mark saved to the
Home. Pennon Rush (`ef42711a8`) is the second example.

## The doors

Each adventure is entered from a tile of the walking kingdom
([KINGDOM.md](KINGDOM.md)): walk up, press E. Any seated knight can open the
door and the whole party goes in, as it does at any other door. Holding Esc
for 15 ticks leaves (`exits.rs`); the way back is the same tile.

| Slug | Tile (x, y) | Where | Nearest existing door |
|---|---|---|---|
| `lanternward` | (104, 23) | The Mines' yard, west of the mine mouth, in the Highlands | Mines (108, 21), 6 away |
| `moonfall` | (78, 92) | The Village's south-west edge, in Castle Town | Village (83, 87), 10 away |
| `bannerpeak` | (174, 23) | The Wastes, west of the Dragon Keep's gate | Dragon Keep (180, 23), 6 away |
| `griffinwing` | (105, 9) | West of the Griffin Eyrie, on the high ground of the Highlands | Griffin Eyrie (109, 11), 6 away |
| `lightgallop` | (175, 71) | The Tilt-Yards, west of the Horse Ranch | Horse Ranch (183, 71), 8 away |

The stubs' doors first stood on the kingdom's 112 by 121 map, at (36, 5),
(46, 48), (66, 5), (77, 24) and (98, 70). When the kingdom opened into eleven
biomes on a 240 by 176 map, Lanternward's tile became rock and the other four
no longer stood where this table put them (the nearest door to Bannerpeak's
was 25 tiles away), so the integrator moved all five to the same places on the
new map and re-ran the test below.

Each tile was picked beside a road on open meadow or ash, and
`together_roundtable__tests` holds the rest: open ground with at least six of
its nine surrounding tiles open, reachable on foot from the Keep, on an
authored screen (the party's wishes are built in the wild screens between; only
a crowded realm falls back to anywhere), and at least six tiles (Manhattan) from
every site's door, class banner stone, kingdom thing (bench, well, bell...),
herald post, the Forest Road's tile (8, 1) and Old Oak (13, 8), and from each
other. The countryside's errands, relics and lamps stand inside their own
rooms, not on this map, so none overlaps. A builder does not move a door; the
integrator does, with the test.

The run is hosted in the Dark Forest's room, exactly as the Forest Road's is:
the door calls `enter_kingdom_place` for the forest site, then the
adventure's `start`. The surface return is overwritten to this tile and this
adventure's name, so a seat that leaves, a seat that joins late and the "waiting
at" card all name the right place. Nothing else about the Dark Forest runs
while an adventure is up (`tick_forest` stands aside).

## Known side effects of the hosting

The adventures borrow the Dark Forest's room and its kingdom door, as the
Forest Road does, so opening one marks the Dark Forest "seen" for the goal
arrow's next stop (`kingdom.seen`), and a knight who leaves returns to the
adventure's tile with the kingdom's door-held guard set, so a held E does not
re-open it. None of this is saved. A builder who needs a room of their own
asks the integrator.

## How a stub is wired

```
rules/roundtable.rs                       the shared shell (integrator's)
  ADVENTURES: [Adventure; 5]              the one list, one line each
  enter, tick_roundtable_entry            the E at the tile
  valid / clear / sessions / knobs / marks  sums over the five
rules/<slug>.rs                           the adventure (builder's)
  NAME, ENTRY, SLOT, CAMPAIGN_MARK, MARKS, NUMBERS
  ADVENTURE (its row in the list), ACTIVITY (its row in activity::RIDES)
  Session { seats, elapsed }  new / step / valid / repair / bot_input
  Run::<slug>_showing, start_<slug>, tick_<slug>, <slug>_for_test
render/overworld/arena/roundtable.rs      stage dispatch + the block scene (integrator's)
render/overworld/arena/<slug>.rs          layers, terminal_layers, kingdom (builder's)
cockpit/.../art/roundtable.rs             terminal text dispatch (integrator's)
cockpit/.../art/roundtable/<slug>.rs      paint (builder's)
```

The shell reaches an adventure only through those names. A builder replaces
what is inside them freely and keeps the names. The shell's registrations in
the shared files were made once, in the prep commit, and never need a line
changed for any of the five.

The stub's session: `seats` (who is at the table and the tick each sat down)
and a clock. `step` seats whoever is present and awake each tick, so a knight
who leaves stands up and a knight who joins late sits down. `valid` bounds
everything (it is what a save, the wire and `valid_snapshot` ask), and
`repair` mends a bad save. The run is a `Run` field that is written only
while it is open (`skip_serializing_if`), so a closed run is byte-for-byte
today's.

The scene is a solid backdrop, the adventure's name large, `STUB ADVENTURE`,
and one box per seat at the table (the viewer's own lit), with `HOLD ESC TO
LEAVE`. A slider along the rule shows the sim ticking. The door on the walking
map is a post and a board in the adventure's colour with its name over it
and `E` under it.

## Who may edit what

**A builder owns exactly these**, by slug, and may add new files whose names
begin with the slug (a data file, a sprite slot directory, a script):

| File | |
|---|---|
| `crates/delve/src/rules/<slug>.rs` | the rules |
| `crates/delve/tests/rules/together_<slug>__tests.rs` | its tests |
| `crates/delve/src/render/overworld/arena/<slug>.rs` | its stage |
| `cockpit/src/stage/world_viz/art/roundtable/<slug>.rs` | its terminal text |
| `crates/delve/assets/<slug>/` | `BRIEF.md` (write it first), data, `ART.md` |
| `crates/delve/assets/art-slots/<slug>.*/` | its sprite slots (new directories only) |

In `rules/<slug>.rs` keep these names and shapes, because the shell and the
shared tests call them: `NAME`, `ENTRY`, `SLOT`, `CAMPAIGN_MARK`, `MARKS`,
`NUMBERS`, `ADVENTURE`, `ACTIVITY`, `Session` (`Clone`, `PartialEq`, `Serialize`,
`Deserialize`, with `new`, `step`, `valid`, `repair`, `bot_input` and a
`seats: BTreeMap<u32, _>`), `Run::<slug>_showing`, `Run::start_<slug>`,
`Run::tick_<slug>`, `Run::<slug>_for_test`. Rename the five knobs, set their
bounds and starts, pick `LIMIT`, fill `settle`: all in your file. In
`arena/<slug>.rs` keep `layers`, `terminal_layers`, `kingdom`. In the cockpit
file keep `paint`. Put your own checks beside the stub's in your test file;
keep the stub's.

**Off limits to a builder**, and why:

| File | Why |
|---|---|
| `rules/roundtable.rs`, `render/overworld/arena/roundtable.rs`, `cockpit/src/stage/world_viz/art/roundtable.rs` | the shared shell; one list, so no branch ever edits it |
| `crates/delve/src/render/overworld/arena.rs`, `cockpit/src/stage/world_viz/art.rs` | **hash-pinned** by `assets/arcade/cell-proof.json`; any edit stales the pin and `build.rs` panics. Reach them through your own file |
| `crates/delve/assets/arcade/cell-proof.json` | the pins; the integrator recomputes them (below) |
| `rules.rs`, `rules/activity.rs`, `exits.rs`, `forest.rs`, `kingdom.rs`, `tune.rs`, `pastimes.rs`, `countryside.rs`, `home.rs`, `wire.rs` | registrations, made once; any adjacent edit conflicts across five branches |
| `render/overworld/arena/art_v2.rs`, `arena/kingdom.rs`, `cockpit/src/ui/viz/shooter_viz.rs` | the same, one line each |
| `crates/delve/smoke/`, `crates/delve/dev-host/` | the scenes call your `<slug>_for_test` and bots; you never need a line |
| `tests/rules/escape_hold__tests.rs`, `together_tune__tests.rs`, `together_activity__tests.rs`, `together_roundtable__tests.rs`, `tests/render/arena__roundtable__tests.rs`, `tests/cockpit/world_viz/roundtable__tests.rs`, `tests/cockpit/app/dungeon_controls__tests.rs`, `dungeon_northern_retry.rs` | shared checks over all five; they read each adventure's own `ADVENTURE` row, so a renamed knob or an added mark needs no line here |
| `assets/art-requests.json`, `docs/ART-REQUESTS.md`, `scripts/art-requests.py` | generated and shared; list your requests in `assets/<slug>/ART.md` and the integrator merges them |
| `Input`, `DelveCmd`, `wire.rs` enums, `Home`, `Records` | see the controls policy; a persisted need is a word of flags in `Home::roundtable[SLOT]` and a `Score` in `Records::roundtable[SLOT]`, both already there |

If you truly need a shared edit, ask the integrator: they make it on the
integration branch, once, in a block of its own. Do not make it on your branch.

## Building and testing

Always set `TMPDIR=/work/tmp`; builds are slow, so run them in the background
and do not poll. Name your target directory after your branch.

```
# the Delve crate's whole suite
cd crates && CARGO_TARGET_DIR=/work/build-cache/<name> TMPDIR=/work/tmp cargo test -p angel-delve

# just your adventure (and the shared shell's checks)
cd crates && CARGO_TARGET_DIR=/work/build-cache/<name> TMPDIR=/work/tmp \
  cargo test -p angel-delve --lib -- <slug> roundtable escape_hold

# look at your render: native, 768, terminal layers and the kingdom door, for
# 1, 2, 4 and 6 seats; open every picture before it counts as done
cd crates && ANGEL_ARENA_SHOTS=/work/tmp/<name>-shots CARGO_TARGET_DIR=/work/build-cache/<name> \
  TMPDIR=/work/tmp cargo test -p angel-delve --lib write_roundtable_review -- --ignored

# the cockpit's terminal route (a separate crate and target dir, a cold build
# of several minutes): the cells as `paint_picture_ui` draws them, the journal
# and the dungeon's controls
cd cockpit && CARGO_TARGET_DIR=/work/build-cache/<name>-cockpit TMPDIR=/work/tmp \
  ANGEL_CARTRIDGES=/work/tmp/empty-cartridges cargo test --bin angel -- roundtable dungeon world_viz
# and its pictures (a .png of the cells and a .txt of the text), open them too
cd cockpit && ANGEL_ARENA_SHOTS=/work/tmp/<name>-shots CARGO_TARGET_DIR=/work/build-cache/<name>-cockpit \
  TMPDIR=/work/tmp ANGEL_CARTRIDGES=/work/tmp/empty-cartridges \
  cargo test --bin angel write_roundtable_terminal_review -- --ignored

# the wasm smoke: the same renderer in wasm under node, scenes 39 to 43
cd crates && CARGO_TARGET_DIR=/work/build-cache/<name>-wasm TMPDIR=/work/tmp \
  cargo build -p angel-delve-smoke --target wasm32-unknown-unknown --release
node crates/delve/smoke/smoke.mjs /work/build-cache/<name>-wasm/wasm32-unknown-unknown/release/angel_delve_smoke.wasm
node crates/delve/smoke/parity.mjs /work/build-cache/<name>-wasm/wasm32-unknown-unknown/release/angel_delve_smoke.wasm <slug>
# the native run of the same scene must print the same lines as the wasm one
cd crates && CARGO_TARGET_DIR=/work/build-cache/<name> TMPDIR=/work/tmp cargo run -p angel-delve-smoke --bin parity -- <slug>

# the dev host: your scene in a browser or with bots (never the angel binary)
cd crates && cargo run -p angel-delve-dev-host --bin delve-dev-host -- --scene <slug> --bots 3
```

The Delve crate builds with the warnings `dev` already has (ten in the
library, all in `rules/pastimes.rs`, `rules/pastimes/gander.rs` and
`rules/pastimes/siege.rs`; five more in its test build, in `arena/rescues.rs`,
`arena/teachers.rs` and two rules tests). Add none.

Four tests on `dev` are load-sensitive and fail now and then with or without
this work, so a red run on one of them alone is not yours: in the Delve crate
`arena::tests::the_still_art_is_the_same_while_nothing_changes` ("one upload for
the room"), `art_v2::tests::v2_is_deterministic_rgb_and_small_frames_use_v1` and
`chariot::tests::successive_hippodrome_frames_move_and_render_fast` (a timing
check); in the cockpit `app::control::dungeon::tests::dungeon_n_held_lays_the_floor_over_the_first_knights_screen_and_moves_no_one`.
Each passes when run alone, or fails the same way on `dev`; rerun it alone before
you chase it.

### The pins (the hidden gate)

`crates/delve/build.rs` panics with `arcade cell gate: stale renderer
evidence: <file>` when any file listed under `"sources"` in
`crates/delve/assets/arcade/cell-proof.json` no longer matches its pin
(`arcade_gate::validate_cells`: FNV-1a 64 over the file's bytes). The list is
`render/arcade.rs`, `render/art.rs`, `render/braille.rs`,
`render/overworld/ink.rs`, `render/overworld/arena.rs`,
`render/overworld/arena/boss_art.rs`, `cockpit/src/stage/world_viz/art.rs`,
`cockpit/src/stage/world_viz/art/cells.rs`, `tests/cockpit/world_viz/arcade__tests.rs`
and the arcade's two manifests and palette. **Builders touch none of them.**

After the integrator merges anything that edits one (the prep commit did, for
`arena.rs` and `art.rs`), recompute and commit the proof file:

```
python3 scripts/cell-proof-pins.py          # rewrites the stale pins, prints each
python3 scripts/cell-proof-pins.py --check  # lists stale pins, changes nothing, exit 1 if any
```

It moves only the `"sources"` hashes. A change that alters how the arcade
knight or effect frames themselves draw needs the full evidence export instead
(`arcade-review`, [ART-ARCADE.md](ART-ARCADE.md)); never use that feature for a
release.

## The co-op rules (the Forest Road's)

- **Every seat plays at once**, on its own screen and camera. No volunteers, no
  spectators, nobody waiting their turn.
- **Foes and checkpoints are shared.** One set of foes, one latest checkpoint.
- **Health is scaled per seat**, as a fixed number per seat times the seats at
  the table. A knight who joins mid-run sits down at the shared checkpoint and
  scales the current encounter up, keeping its remaining share; a knight who
  rejoins a seat already provisioned does not scale it twice.
- **Harder co-op means more pressure, never weaker foes.** More bodies, more
  shooters, tighter windows. Monster health and damage never drop.
- **Restart in under a second.** A miss puts the knight back in the loop with
  no menu; leaving and coming back through the door is a fresh run.
- **Instructions are two or three plain words** on a card. No walls of text.
- **E interacts, F fires**, spells on 1 to 4. A knight leaves with Esc held.
- **Real stakes, bounded.** The campaign mark settles into the Home once
  (`settle`, idempotent); a purse has a cap; no score alone.
- **Host-authoritative, integer, 30 Hz.** Fixed-point positions and tick
  timers; the same seed and inputs replay byte-identically; `valid` bounds
  every number a save or the wire could carry.
- **One fixed number per limit**, picked from play, no formulas. Each
  adventure has its five (`NUMBERS`, knobs `<slug>_a` to `<slug>_e` until
  renamed), live on the F2 panel.
- **Finished-grade only.** Write the brief first (`assets/<slug>/BRIEF.md`),
  build the verb in an empty room, play it five times including with two knights
  and a full table, and open every render before it counts as done.

## The controls policy

Existing buttons only: move, aim, F, E, Esc held, spells on 1 to 4, and the
buttons the other games already use. **No new `Input` bit, no new command, no
wire enum change.** A builder who truly needs a button asks the integrator,
who adds it once on the integration branch. Name only the buttons the game
uses on its title card.

## Why five branches merge cleanly, and what could still collide

Everything shared was registered in the prep commit, as sums over the one list
in `rules/roundtable.rs` and as dispatch through `arena/roundtable.rs` and the
cockpit's `art/roundtable.rs`, so a builder's branch adds and edits files only
under its own slug. Every per-adventure value a shared file would otherwise
hold (the door, the marks, the five knobs, the activity row, the session type)
lives in the adventure's own file.

What can still collide, and what to do:

- **`Cargo.lock`, or a new dependency.** Do not add one; ask.
- **A builder who edits a shared file anyway.** Revert it and ask; the pin
  gate and the five-way merge both punish it.
- **A shared helper two builders both want** (a sprite loader, a shared HUD):
  the integrator adds it to the shell once; until then each copies the few
  lines into its own file.
- **Art requests** (`assets/art-requests.json`) are generated; list yours in
  `assets/<slug>/ART.md`.
- **After merging all five**, run `python3 scripts/cell-proof-pins.py --check`
  (nothing a builder touches is pinned, so it should be clean), the whole
  Delve suite, the cockpit's `world_viz` and `app::control::dungeon` tests, the
  wasm smoke and the parity script for each slug.
