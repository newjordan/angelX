# Room hit-stop

The host's `Run::hitstop` holds simulation time on large impacts. Network
commands and snapshots continue at 30 Hz; the room clock, all actors, RNG,
projectiles, cameras and effect timers stand still. The existing boss hit
flash, sparks and shake hold with the picture. Client prediction and pose
blending stop during the hold, including its last frozen frame.

The F2 server group **hit-stop** exposes these defaults:

| Knob | Default |
| --- | ---: |
| `hitstop` | 1 (on; 0 disables) |
| `hitstop_boss_kill` | 6 ticks |
| `hitstop_elite_kill` | 3 ticks |
| `hitstop_phase` | 4 ticks |
| `hitstop_heavy` | 2 ticks |
| `hitstop_cooldown` | 14 transport ticks (0.467 seconds) |

Boss kills include guardians, the dragon and Pit Tyrant. Large kills use
an authored radius of at least 1 unit, excluding training dummies and
wards; party/depth health scaling does not change that classification.
Phase events include rage, a change in a chained attack phase, and entering
a stagger. Heavy hits include bombs/Thunder Scrolls, empowered shots and
damaging ultimates, including delayed shells, missiles, cuts and spins.
Ordinary boss shots do not request a hold. Simultaneous requests take the
longest duration; kills override cooldown. Cooldown runs on transport ticks.

Valid action presses during the hold coalesce per knight, as queued engine
commands do. The next advancing step overlays current held controls on those
presses, retaining a tapped dash's direction. Buffered controls survive saves
and snapshots. Disabling hit-stop releases them on the next step.

`hitstop-review OUT [--off] [--capture] [--check]` in `angel-delve-smoke`
runs 1,800 transport ticks for each of Warden, Bone Choir and the busy crypt,
with four deterministic shooters. It replays the inputs independently,
writes every save snapshot and checks the committed 60-tick save/wire hashes.
Only tune metadata is normalized out of the comparison. The `off-*` fixtures
were recorded on the original simulation at `69d7e40a`; the `on-*` fixtures
intentionally record the changed timing. The older two-knight tune golden
is unchanged with either setting. Ordinary test fixtures use
`step_sim_for_test` when they mean an advancing tick after an impact;
transport-timing and hit-stop tests use `step` directly.

`parity BOSS_ID [--off] [--party]` and `smoke/parity.mjs WASM BOSS_ID`
(with the same flags) compare native and wasm over 60 ticks, both art kits,
rage/normal and painted/plain variants. Hash part 6 normalizes tune metadata
for the original/off comparison. The JavaScript scene table follows the
current native scene order, including the six shelf games.

For terminal review, the ignored `write_hitstop_terminal_shots` test in
`scripts/boss-display-review` reads `ANGEL_HITSTOP_SNAPSHOTS` and writes
RGB plus production 96x28 cell buffers under `ANGEL_HITSTOP_CELLS`.
The dev host prints per-impact durations and cumulative minute counts;
it waits through a kill hold before refilling a review room.

Cross-platform evidence retains whole-wire and pixel hashes, including the
existing differences: Warden/Bone Choir have a few platform-specific rendered
frames. The crypt's whole-wire difference is four one-ulp `feel.stuck` arrow
direction values, also present with hit-stop off; actors, projectile physics
and crypt pixels match. Native and wasm pixel goldens remain separate.
`parity BOSS_ID --dump` and the wasm `state_json_ptr`/`state_json_len` exports
provide an untimed JSON state dump for checking a mismatch by field.
