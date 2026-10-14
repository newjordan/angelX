# College of the Labyrinth

A separate school of magic stands beside the Delve entrance. Its wizard is
**Bartosz the Wizard**, a respectful fictional portrayal of Professor Bartosz
Naskręcki, creator of [Labyrinth Exploration](LABYRINTH.md). He studies the
Labyrinth and the Delve's creatures. His portrait and world sprite are original
pixel art: a blue robe, pointed hat, spectacles, staff and a welcoming face.

On the realm map, walk to **College of the Labyrinth** and press **E or Enter**.
A short woodland approach and stream footbridge join the existing map road.
Inside a Delve, the free **College Gate** stair is in the east of the gate
courtyard, beside the entrance into the Undercroft. Every room uses the same
room engine, input, party travel, checkpoint and per-seat renderer as the world.
Stand on a door or stair until its ring fills; no admission fee or building
purchase is needed. All seated knights travel together and have their own views.

The fountain courtyard has wandering student folk. The Great Hall connects to
the Chalk Classroom, Floating Library and Alchemy Lab. The courtyard also opens
into the Wizard's Tower: Tower Entry → Spiral Landing → Star Observatory →
Bartosz's Study. Reverse the stairs to come back out. Furniture has collision;
the moving shelf actually opens its passage. Floating books, candles, bubbling
cauldrons, window stars and the fountain animate in the realm palette.
Investigate the fountain rim, woven rug, chalk mark and jar lid with E for
small hidden discoveries; the library's curious book also has a lesson.

Press **E** beside Bartosz. Use **up/down** to select a topic or quest, **E** to
read or accept, and **left** to return to his menu. Walk away to dismiss it.
A shared speech box, with his portrait, is owned by the knight who opened it;
the other knights can keep exploring. The study's Bestiary lectern reads the
Herald's existing bat, skeleton and slime kill counts alongside the College's
separate observations of living creatures.

His brief lessons explain the research map, uncertain versus established routes,
independent checks and retained failures, then the Delve's connected seeded
Gallery/Warrens/Karst growth from floor three. Creature lessons describe actual
bats, skeleton archers, splitting slimes and the Fungal Deep's splitting imp
spores. Claims come from [LABYRINTH.md](LABYRINTH.md),
[the Rust research adapter](../cockpit/src/drive/labyrinth/mod.rs), and the
Delve's `layout.rs`, `monsters.rs`, `foes.rs`, `patterns.rs` and `bestiary.rs`.
Research evidence and game quest rewards remain separate systems.

Accept any of his six quests; return and select its entry to hand in completed
notes. These are shared realm missions, using the existing pinned bounty record.
They never take a slot on Wren's board or replace King Brannoc's missions.

| Quest | Fieldwork | Fixed reward, once |
|---|---|---|
| Chart the Tunnels | Visit four distinct rooms at depth 3 or deeper after accepting | 60 gold; unlock Hedge Maze spell |
| Creature Notes | Press E within three tiles of a living bat, skeleton and slime | 45 gold |
| The Lost Tome | Clear a Drowned Archive treasure room; E at its field marker | 50 gold |
| Spore Specimens | Clear three distinct Fungal Deep rooms; E at their patches | 55 gold |
| Study Together | Two living, active knights near one field marker in a cleared room at depth 3 or deeper; one presses E | 40 gold + 1 bond |
| Three Hidden Sigils | Investigate the tower shelf, star map and familiar; collect the revealed sigils with E | 75 gold |

The map mission permanently unlocks one existing **Hedge Maze** spell card for
each knight. It enters a free spell slot on the next home-card deal (including
new seats and new delves); it never replaces a spell already equipped. A knight
with four spells receives it when a slot becomes available and home cards are
dealt again. Rooms, samples and creature kinds deduplicate. A statue, dead knight
or absent seat cannot count as a study partner. Notes collected before accepting
a field quest do not count. The three puzzles can be investigated freely.

Progress defaults to empty in old realm saves. Acceptances, notes, hand-ins,
rewards and completion flags use the realm's ordinary atomic save; a failed save
keeps bounded private intents in the session checkpoint for retry. Replaying a
hand-in cannot pay twice. School dialogue and puzzles are mirrored to guests;
research-map files and unpublished claims are not multiplayer game state.

The Sloptomizer School of Magic remains its own separate landmark by the market
gardens; its existing commands and rooms keep their original meaning.

## Later classes

Future class sessions can bring students in as browser seats through the
existing multiplayer invitation, let them take field trips together, and add
shared notebooks with explicit workspace grants. The College's rooms, shared
quest records and Bestiary lectern provide places for those activities. No class
scheduler, notebook synchronization or research campaign execution is added here.

## Offline review

```sh
CARGO_TARGET_DIR=/work/build-cache/angelx-school TMPDIR=/work/tmp \
  cargo test --manifest-path cockpit/Cargo.toml -p angelX-cockpit \
  --bin angel --no-default-features college
ANGEL_ARENA_SHOTS=/work/tmp/review-shots/school \
  CARGO_TARGET_DIR=/work/build-cache/angelx-school TMPDIR=/work/tmp \
  cargo test --manifest-path cockpit/Cargo.toml -p angelX-cockpit \
  --bin angel --no-default-features write_college_shots -- --ignored
```

The writer covers the gate-side map, gate courtyard, all nine College rooms,
Bartosz's quest dialogue, topic menu, tunnel lesson and Bestiary, and the opened
bookshelf. The review contact sheet is kept with these local shots.
