# Stables and the lists

[Architecture index](README.md) · [Commands](../COMMANDS.md) · [The Barony](../BARONY.md)

The stables and the lists are **rooms of the world** beside the Delve's gate
(`drive/together_shooter/world.rs`), walked with the Delve's keys and
mirrored to friends like any home room. The joust is simulated in the run
(`drive/together_shooter/joust.rs`) and drawn by
`stage/world_viz/overworld/arena/joust.rs`. The realm keeps the stable in its
home (`Home::stable`: the saddled mount, the tending, bouts, wins,
unhorsings), decoded on its own so a bad subtree never costs the realm. It is
not the `/tourney` benchmark system and has no model calls.

The earlier 3D side stage (a practice overlay with three guard, aim or charge
picks) was removed: its dot render was unreadable at 1:1, a pass had no
charge or hit, and its key presses cleared the screen.

## Commands

| Command | Effect |
|---|---|
| `/dungeon stable`, `/world visit stables` | Open the Delve (a new delve begins at the gate) and walk the party into the stables. |
| `/dungeon stable select Bramble\|Cinder\|Mist`, `/dungeon stable tend` | The same marks a stall's plate and the trough's leave; settled by the cockpit into the realm. |
| `/dungeon tournament`, `/world visit lists` | Walk the party onto the lists' mount plate. (`/world visit tournament` is still a camera visit on the map.) |
| `/dungeon stable status`, `/dungeon tournament status` | The stable and the next rival, as text. |

## The joust

Hold F on the mount plate to mount (`Run::mount_up`): the knight rides the
saddled mount against the next rival (Sir Kay, Sir Palamedes, Sir Lancelot,
then Lancelot again). While mounted the knight's keys go only to the joust
(`joust::reins`).

- A course: F spurs; the riders accelerate down either side of the tilt and
  meet in the middle at `Steed::charge` ticks. W or S aims high (the helm) or
  low (the shield) until the meeting; F strikes, Space braces. A press counts
  inside its window around the meeting (`Steed::strike`, `Steed::brace`); the
  riders hold at the clash until the latest window closes, then the lances
  are resolved (`Run::meet`).
- The rival guards high or low each course (from the seed, the bout and the
  course: no dice). Striking his shield is a point (two if the lance breaks
  at the very moment); striking where it isn't is two (three at the very
  moment, plus the mount's weight in knocks). His lance follows a pattern
  that moves along from bout to bout; a brace in time takes it a grade
  lighter.
- Knocks come off each rider's balance (the mount's, one more if tended; the
  rival's own). At none, the rider goes over his horse's tail and the bout is
  over. Otherwise three courses are counted.
- Holding Space at the end withdraws. The verdict stands four seconds, then
  the knight dismounts at the plate. The run marks `joust:start`,
  `joust:<rival>:<won|lost|drawn>` and `joust:unhorsed`; the cockpit settles
  them (`joust::settle_stable`).

Tests: `tests/cockpit/app/together_joust__tests.rs`,
`tests/cockpit/drive/chivalry__tests.rs`,
`tests/cockpit/app/dungeon_chivalry__tests.rs`. Renders:
`write_joust_shots` in `tests/cockpit/world_viz/overworld__arena_tests.rs`.
