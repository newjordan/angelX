# The heart questline

The kingdom has one questline that no knight can walk alone. Its steps are
real work: progress on the heart boards of the Virtual Embryo Challenge
(NeurIPS 2026), which asks how the mouse heart is built, from single-cell
and 3D spatial transcriptomics. The kingdom keeps a mirror of that work.
When the work moves, a lantern is lit in the Lantern Hall, and every knight
who calls in hears about it.

## Elspet the Heartwright

Elspet keeps the Lantern Hall, a small stone hall on the town green east of
the Court of Deeds, across the North Road. She wears a leather apron stuck
with pins and keeps her spectacles pushed up into her grey hair. She never
quite puts down her lantern. Six lanterns stand on her bench: one for each
step, and the trial's on a post above the others. Each one is dark until
the work reaches it.

The steps follow a heart as it is first built. Each one is tied to the
board that models that moment:

| Step | Counter, need | What it waits on |
|---|---|---|
| The Crescent | `vec_data_staged` 1 | the stages gathered: every training file on disk and checked |
| The Tube | `vec_heart_proxy_runs` 3 | a heart board measured three times at home before any is sent |
| The Loop | `vec_heart_interp_above_floor` 1 | the heart board at E8.5 above its floor |
| The Chambers | `vec_heart_extrap_above_floor` 1 | the heart board at E10.5 above its floor |
| The Quiet Gene | `vec_gata4_above_floor` 1 | the Gata4 board above its floor |
| **Trial: The Whole Heart** | `vec_heart_top10` 1 | a heart board among the VE top ten |

The reward is a title, **the Heartwright's Apprentice**, and a keepsake, **a
red thread, tied in a loop**: the heart tube's first fold. No power is
taught. The file is `cockpit/assets/dungeon/questlines/heart.txt`, in the
same format as the Fellowship's (`docs/DELVE-TEACHERS.md`). Its ties are to
Dame Alyce (Mending Strike), Goodwife Barley (Stout Heart) and Gil (Chain
Spark).

## A kingdom questline

Every other questline teaches a power. Its teacher opens only to a knight
holding that power's second tier, and its counters are deeds done in the
realm. The heart line is the other way round. It is the kingdom's, not a
power's: every knight may walk it, and nothing a knight does in the realm
moves it. The questline system expresses this in one word and two checks:

- **`power kingdom:heart`.** The kind `kingdom` (beside `wish` and `skill`)
  marks a questline that teaches no power. Its gate (`Run::quest_gate`) is
  open to every knight. The tier and level rules never ask about it: they
  look up `wish:` and `skill:` lines only.
- **The campaign's counters are a kingdom questline's, and only its.** The
  counters prefixed `vec_` belong to the campaign. The parser holds every
  file to this, with line numbers. A `vec_` word that is not one of the
  seven counters is refused at its line. A power's line that names a `vec_`
  counter is refused, and so is a kingdom line that names a counter of the
  realm. No deed can walk the heart line, and no outside work can walk a
  power's.
- **`room realm-hall` with `hall Lantern Hall`.** Every landmark's and
  castle's door opens on one shared stone hall, which shows the visited
  place's name. A teacher in that room must name which hall is theirs
  (`Run::teacher_here`, keyed on the door the party came in by). Only a
  realm-hall teacher may name a hall. Elspet is drawn, heard and marked in
  the Lantern Hall only.

This reuses the questline's file, parser, sheet, missions and drawing. The
other route would have been a separate quest type, or a new room kind for
the hall. A new quest type would duplicate all of that. A new room kind
would touch every exhaustive match on rooms, the 3D looks and the save
masks. Neither buys anything here.

## The Lantern Hall

`world::kingdom::HALLS` lists the kingdom's own halls: a name, a region and
a spot. Each hall is built by the rule a countryside place follows. It takes
the free 4 x 3 lot nearest its spot in its region, with three tiles of ground
from its neighbours, off set pieces and region names. It gets a lane to the
nearest road. Its door opens on the shared stone hall. The halls are built
after every countryside place has its lot, so no existing place moves. The
Lantern Hall's spot is (82, 70) in Castle Town. Its lot comes out at
(85, 70), with the door at (86, 73), six tiles of ground east of the open plot (74 to
78, 68 to 72). On the map it is a small stone hall with a bench by the door
and its six lanterns dark under the eaves. On the map, light means activity,
so the lanterns there stay dark. They burn only inside.

A save from before the hall loads as any save from before a change of
ground does (`docs/KINGDOM-OPEN.md`, "Saves"). The new ground replaces the
saved one. A knight standing where the hall now stands steps onto the
nearest open ground.

## The counters

The `vec` cartridge is the only writer. It rewrites
`$ANGEL_VEC_HOME/board.json` (default `~/.angelX/vec/board.json`, schema
`vec.board/1`, the cartridge's `CONTRACT.md`) atomically after every event.
The board's `quest` object holds monotone, non-negative integer counters.
The realm reads seven of them and nothing else from that folder.

- **Reading** (`rules::campaign::Campaign::parse`, in the Delve, pure): a
  board over 256 KiB, nested deeper than `serde_json` follows (128 levels),
  of another schema, or with any of the seven that is not a whole number
  from 0 to a million (a negative, a fraction, `1.0`, a string, a bool, a
  null) reads as all zeros. The board is taken whole or not at all. An
  unknown counter is ignored and a missing one is 0.
- **Polling** (`together_files::CampaignWatch`, in the cockpit): the delve's
  service thread looks at the board at most once a second (`CAMPAIGN_POLL`).
  It reads the board again only when its modified time, length or file has
  changed. It opens the file non-blocking, checks it is a plain file, and
  reads no more than the limit. The reading is done with the run let go.
  Under the run's lock, between ticks, the run takes the counts only when
  they differ (`Run::campaign`, marked edited so the next snapshot carries
  them). A new or restored run has them again within a second. A missing,
  unreadable or malformed board reads as zeros. It never panics, never
  writes, and never changes a save.
- **Read-only in the game.** `Campaign` has no setter and no increment. It
  is made only by parsing a board, or by the wire carrying the host's copy
  to a client for drawing. `Run::campaign` is `serde(skip)`, so a
  checkpoint never carries it: a save that claims one is ignored. The wire
  sends it beside the run, like the marks and the pot shards
  (`wire::DRAWN`). A client's copy flows nowhere. `questline::Board::note`
  drops any `vec_` counter, so `note_quest` cannot pay one in.
  `flush_questlines` never touches a kingdom line. A knight's saved walk of
  the heart line keeps no counts.
- **Test builds read no operator's board.** `campaign_board()` is `None`
  under `cfg(test)`. A test hands a board's path to the service through
  `Controls::campaign`. A friend's delve (joined) reads none either: its
  host's campaign arrives on the wire.

## Progress is the campaign's

The campaign is one, so the heart line's progress is one for the whole
realm (`questline::campaign_walk`). Every step and the trial are worked out
from the counts each time they are asked for. A step is done once its count
has reached its need. The trial is passed once every step is done and the
top-ten count has reached 1. Steps are done independently and told in order.
If the E10.5 board clears its floor before the E8.5 one, the Chambers is
done, but Elspet still speaks of the Loop as the step at hand. When the
board reads zeros (missing or broken), the hall shows zeros. That is the
honest mirror.

What each knight keeps, by name, in the realm save beside their other
questlines (`home.questlines`, key `kingdom:heart`), is only what Elspet has
told them:

- the steps she has told them were reached,
- whether she told them the trial was passed,
- whether she gave them the keepsake.

These change only when that knight presses E at her bench. Pressing E
gives one line each time:

- the hook, on the first meeting;
- then, oldest first, one step the campaign has reached since that knight
  last heard;
- then the closing line, with the keepsake and the title, the first time
  after the trial;
- otherwise, the step the campaign is on, how far it has come, and that
  step's waiting line.

She is marked on a knight's map, at the hall's door (`missions_held`), only
while she has news for that knight: a reached step they have not heard, or
the keepsake not yet given. A realm with no board read never shows the
mark, so frames of a kingdom without the campaign are unchanged.

## Also fixed

A teacher's spot is in tiles and a knight stands in units (two to a tile).
`tick_teachers` compared one with the other. Standing at Stilt's drawn spot
at the millpond heard nothing, and a spot about half as far from the room's
corner did. It now compares tiles with tiles, within two tiles of the spot
(`TEACHER_REACH`).

## Proof

Every command here ran in the worktree on 2026-10-20, with
`CARGO_TARGET_DIR=<build-cache>/heart-quest TMPDIR=<scratch>`, on a
heavily shared machine.

- `cd crates && cargo test -p angel-delve -- heart campaign questline`:
  - the board reader (`campaign__tests`): a missing, empty, cut-off or
    other-schema board reads as zeros, and so does a negative, fractional,
    `1.0`, string, null or over-a-million count, a board over 256 KiB, one
    nested 100,000 deep, and random bytes;
  - the parser and the line (`heart_quest__tests`): the file parses, and a
    bad file answers at its line;
  - nothing raises the counts: `note_quest`, `flush_questlines`, a forged
    saved walk and talking to her leave them as they were;
  - a run's save never carries the campaign, and a forged one is ignored;
  - as the counts rise, the steps complete and are told in order, and
    still in order when the board reaches a later step first;
  - two knights see the same campaign, and each hears it for themselves;
  - her mark shows only when she has news;
  - the hall's door is on a road the Keep's roads reach, and E at her spot
    in the Lantern Hall answers;
  - in another landmark's hall she is not there;
  - Stilt is heard at his drawn spot;
  - an old save loads and saves again byte for byte.

  The Fellowship's own questline tests pass beside these.
- The whole Delve suite, `cd crates && cargo test -p angel-delve`: 2,378
  passed, 269 ignored and 1 failed. The failure was
  `successive_hippodrome_frames_move_and_render_fast` (a frame-time budget
  on the loaded machine, 81 ms). It passed when run alone. An earlier full
  run also failed `upgraded_panels_stay_local_to_their_seat` under the same
  load, and it passed alone too.
- The two expected changes the hall makes:
  - **`TODAY`** (`together_tune__tests`), the kingdom walk's tick-for-tick
    hash, has moved: the run carries the kingdom's map, and the hall is new
    ground in it. With the kingdom's tiles left out, the walk hashed the
    same with and without the hall, and the two knights ended in the same
    places.
  - **`crates/delve/assets/world3d/chunk-3-4/chunk-3-4.set`** was
    regenerated (`ANGEL_WORLD3D_CHUNKS=1 ... write_world3d_chunks`). Only
    that chunk changed: the hall as a 3D stone hall under slate
    (`chunks::kit_of`), its lane, and the garden dressing its lot cleared.
- `cd cockpit && cargo test --bin angel` (the binary run directly), with
  the filters `docs/KINGDOM.md` names:
  - `together`: 155 passed, 5 ignored;
  - `world_viz`: 376 passed, 57 ignored;
  - `overworld_walk`: 7 passed;
  - `app::control::dungeon::tests`: 66 passed, 6 ignored.

  The new cockpit tests:
  - `together_files`: the bounded read (missing, too heavy, malformed, a
    folder, an unwritten pipe that must not hang) and the once-a-second
    watch;
  - `the_service_brings_the_campaigns_board_into_the_run`: a board reaches
    a hand-stepped delve through its service, and goes back to zeros when
    removed.

  Two `together_sim` tests (`the_hosts_own_knight_walks...` and
  `the_runs_sounds_reach_the_seats...`) fail when run under the filter
  `drive::together_sim::tests` alone. They fail the same way on `dev`
  (`d0d909766`, built separately), and they pass inside the `together` run.
- `cd crates && cargo check --tests -p angel-glass -p angel-delve-dev-host
  -p angel-delve-smoke -p angel-web-delve`: clean.
- Shots: `ANGEL_ARENA_SHOTS=<scratch>/review-shots/heart-quest cargo test
  -p angel-delve -- --ignored write_heart_shots` writes:
  - `heart-hall-dark.png`, `heart-hall-three-lit.png` (staged, measured
    three times, E10.5 above its floor: lanterns 1, 2 and 4) and
    `heart-hall-all-lit.png`: Elspet at her bench;
  - `heart-hall-door.png` and `heart-hall-door-news.png`: the hall on the
    green, and her mark by its door when she has news;
  - `heart-hall-town.png`: the town round it, from the baked kingdom.

  The writer refuses to write if any questline file failed to load: a file
  that does not load draws no teacher.

## Open

- Teacher figures are 9 by 7 ink cells (`ART_W`, `ART_H`). Elspet is as
  small beside a knight as every other teacher is.
- A spoken line shows for three seconds in the cockpit's footer, the same
  as every teacher's. The hook is the longest line.
- `crates/delve/dev-host` hosts a run without the cockpit's service, so it
  reads no board. The board is read on the machine that hosts the delve:
  a host without the `vec` cartridge's folder shows a dark hall.
- The live cockpit reading the real board has not been watched end to end.
  The path is tested with a temporary board in a hand-stepped delve.
- A painted plate for the kingdom's chunk 3-4, wherever an asset pack holds
  one, was painted from the old set. It is stale now and that chunk draws
  flat until it is painted again.
- The Heartglass panel (`glass/heartglass`, not yet on `dev`) is the
  contract's other reader. If it reads `board.json` too, it should use this
  bounded reader rather than a second one.
