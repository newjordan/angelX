# Squires

## Location and keys

The **Training Yard**, north of the Undercroft, now has three separately
upgradeable stations along its north side. Walk onto a labelled plate and
**hold E**; release E before using another action. Each seat gets only its
own contextual paper board, with price, treasury balance and hold progress.

- **Recruit** (west): **Next** browses cadenced named offers; **Hire** pays
  treasury gold; **Build** upgrades the recruiting board.
- **Hire** (centre): **Select** cycles your roster's active squire; **Rest**
  selects nobody; **Build** upgrades the hiring hall.
- **Train** (east): **Train** gives the selected squire an instant paid
  lesson; **Roster** cycles the selection; **Build** upgrades the yard.

The roster board shows the active entry and up to three entries starting
there. Cycling exposes every entry without a tall panel covering the room.
Selection is a town action, not an in-combat replacement. Hiring the first
squire selects it automatically. Existing weapon/card/lesson stations stay
where they are.

## Design

- Permanent roster per named knight, using `Home::knight_key`, as talents
  and other realm records do. Three places initially, up to nine. One
  optional active ID, never a list of active choices.
- Three initial offers, refreshed once per delve after first visiting the
  yard. Empty boards stay empty until that cadence advances. Every type
  appears over the cadence. Recruitment upgrades give more offers (up to
  six) and better innate talent. Offers have stable IDs, names, type,
  level, health and power. Names/commands safely support punctuation.
- Shared treasury funds hiring, lessons and all three building upgrades.
  Hiring upgrades add roster places and reduce wages. Training upgrades
  reduce lesson cost and raise the level ceiling from 3 to 9. Each station
  has three improvements; displayed levels run 1–4. Training is an instant
  gold-paid lesson; no real-time wait, offline timer or new dependency.
- Level increases health and power. Level 2/3 unlock type abilities:
  - **Ninja**: fast flanking and thrown stars; level 2 backstabs from the
    opposite side of the foe to the owner; level 3 timed smoke/vanish.
  - **Healer**: keeps distance, heals the lowest health fraction in range;
    level 2 cleanses hex/chill/mire; level 3 approaches a fallen knight's
    lying skull and raises them with limited health and a long cooldown.
  - **Ranger**: keeps range and fires arrows; level 2 marks a target,
    visibly indicating it and strengthening the ranger's arrows at it.
  - **Shield**: stands between owner and foe, catches ordinary shots at
    reduced damage, bashes nearby foes; level 2 gives brief, cadenced guard.
  - **Alchemist**: ranged splash bombs with an existing explosion effect;
    level 2 adds lowest-health potions. Healing and blasts respect walls.
- Reuses `Hireling` bodies, health/down handling, `beside` placement and
  grid sliding; fits the existing minion/decoy aggro and projectile systems.
  Squires have separate slots, so Beaumains and summoned minions still work.
  Smoke removes the companion from enemy sights. Beams can hurt companions
  but cannot be erased by a shield as though they were ordinary arrows.
- Downed squires recover at half health after combat or on a room/floor
  transition. No permadeath and no inventory or loot pickup logic. They do
  not count as party knights for defeat, loot or encounter scaling.
- Small figures sample the knight sprite pipeline, retain the existing
  owner's palette, and add five distinct equipment silhouettes. Downed
  figures lie sideways; wounded figures have small health bars. No names
  floating over every companion in a six-knight fight.

## Persistence and authority

`Home.squires` is the persistent company book, included in the existing host
realm save. `Run.squires` is the transient active party, saved in checkpoints.
Both default for old saves and omit empty state, preserving existing unused
save/wire/determinism snapshots byte-for-byte.

Like town sales and holdings, hold-E actions produce ordered marks; the
existing `activity::settle_all` registry applies them to the authoritative
realm and treasury. The host's normal save/rollback and `rebuild_home` path
handles them without cockpit changes. Offers cannot be hired twice and
insufficient gold never partially mutates a purchase. No new network input
fields. The incremental guest decoder explicitly recognises `squires`.

## Files

Owned implementation:
- `crates/delve/src/realm/squires.rs`
- `crates/delve/src/rules/squires.rs`
- `crates/delve/src/render/overworld/arena/squires.rs`

Focused tests:
- `crates/delve/tests/realm/squires__tests.rs`
- `crates/delve/tests/rules/squires__tests.rs`
- `crates/delve/tests/render/arena__squires__tests.rs`

Small registrations/hooks: `realm.rs`, `rules.rs`, `rules/home.rs`,
`rules/activity.rs`, `rules/yard.rs`, `rules/floors.rs`, `rules/hireling.rs`,
`rules/aftermath.rs`, `rules/hunt.rs`, `rules/press.rs`, `wire/guest.rs`,
`render/overworld/arena.rs`, `arena/room_paint.rs`, `arena/figures.rs`.
No tavern or alehouse rules/render files changed.

## Tests and evidence

TDD checks included explicit failing tests before adding decoy support,
wall-limited healing/blasts and cadenced shield guard. An initial full-suite
failure exposed changed empty-state serialization; this was corrected, not
accepted by updating golden baselines.

Focused command:

```sh
CARGO_TARGET_DIR=<build-cache>/qsquires cargo test --manifest-path crates/Cargo.toml -p angel-delve --profile fast squires --lib
```

**18 passed, 0 failed, 1 ignored.** Coverage includes cadence, exhausted
boards, every type, atomic treasury costs, roster cap/upgrades, training and
unlock gates, one active per owner, six owners, all type behaviours,
down/recovery, no loot consumption, old serde defaults, save round trips,
full wire and incremental browser guest decoding, hold-E through purchase,
upgrade, selection and descent, walkable plates and distinct sprites.

Required full crate command:

```sh
CARGO_TARGET_DIR=<build-cache>/qsquires cargo test --manifest-path crates/Cargo.toml -p angel-delve --profile fast
```

**2,848 passed, 0 failed, 303 ignored**, plus doc tests (0). Existing unrelated
unused-variable warnings remain in gore/alehouse/open-floor tests.
Log: `<scratch>/tavern-quality/squires-full.log`.

Ignored local art exporter, explicitly run and passed:

```sh
ANGEL_SQUIRE_SHOTS=<scratch>/tavern-quality/squire-shots CARGO_TARGET_DIR=<build-cache>/qsquires cargo test --manifest-path crates/Cargo.toml -p angel-delve --profile fast write_squire_shots --lib -- --ignored
```

Artifacts: `squires-yard.png`, `squires-yard-seat.png`, and
`squires-types-owners.png` under that directory. These are offline rendered
review images, not claims of a live browser/playtest session. No host was
started or accessed; no port 8840 access or cockpit tests. No dependencies.

## Integration scope

Tavern quality and weapon-wall work belongs to the main driver and is not
included. Squire gear/armory, dismissal and opt-in permadeath are not added;
none is needed for the default recruit/hire/train/select/delve flow. Balance
numbers are deliberately bounded, deterministic starting values; live party
playtesting is still useful for tuning, but not performed on the live host.

## Integrated review fixes

All nine actionable findings in the integrated-squires review are addressed:

1. Leaving removes the owner's companion immediately. Missing, absent and
   outside owners are excluded from squire movement, enemy sights (including
   warning previews), projectile guards, room reconstruction and rendering.
2. Hostile flying shots meet companions along the swept segment, ordered
   against knight hits and skeleton-wall entries. Shields absorb the shot at
   half damage; ordinary companions take swept damage without erasing volleys.
   A companion behind the knight cannot intercept an earlier knight hit.
3. Non-waiting beams now hurt companions after the warning. Each beam records
   companion owners struck independently of its knight-hit mask, so a long
   burn cannot hit the same companion again after its invulnerability expires.
   Strike records survive checkpoint and wire round trips; shields do not
   erase beams.
4. Healers pursue skulls while their healing/revival ability cools down, so
   repeated ordinary healing cannot continuously abandon revival movement.
5. Revival chooses the nearest line-reachable skull with an owner-ID tie
   break, excluding absent/outside/carried skulls rather than repeatedly
   pursuing a blocked lower-ID skull.
6. Contact uses `hurts_by_touch`; targeting and direct/splash damage reject
   buried enemies, including underground moles and submerged bog hands.
7. Companion projectiles retain damage-credit ownership but carry separate
   squire provenance. They do not consume knight weapon-shot budgets or enter
   owner prediction/ghost handover. The new fields default away for old/plain
   projectiles, preserving existing empty-state wire shapes.
8. Partial hold-E progress belongs to its specific squire action. Release,
   leaving or changing plates cancels it; room transitions clear it. A spent
   press still requires release before another action. No treasury action can
   inherit an almost-completed hold from another plate.
9. Reusable alchemist blasts use shared `aoe::reached` nearest-first ordering
   and `aoe::part` crowd falloff, retaining wall checks and owner damage credit.

### Regression evidence and diagnostics

Added **17 regression tests** using real `Run::step` combat/yard execution,
plus offline renderer and client prediction checks after real simulation
steps. Coverage includes multiplayer departure; absent/outside/missing-owner
sights and rendering; high-speed shield interception with/without a knight;
knight/shield/skeleton ordering; all five types under warning/burning beams;
per-companion beam strike persistence; sustained healing during skull pursuit;
blocked/nearest/tied revival targets; buried contact, attacks and splash;
owner/companion same-kind prediction handover; knight shot allowance;
release/exit/action-change hold cancellation and actual realm settlement;
and crowd falloff.

TDD diagnostics:

- The first nine full-step review regressions failed on the integrated code.
- The corrected yard regression was also run with the original hold handler:
  it failed when off-plate release carried a 21/22 hold into an immediate
  action, rather than depending on a missing host settlement in the fixture.
- Removing provenance and crowd-falloff fixes made the shot-budget,
  same-kind client prediction and crowd-damage regressions fail.
- Intermediate fixture/build diagnostics were corrected (private room hook,
  borrow scopes, and a skeleton fixture lacking its required active wall).

Fresh final offline library suite:

```sh
CARGO_TARGET_DIR=<build-cache>/qsquires-fix cargo test --manifest-path crates/Cargo.toml -p angel-delve --profile fast --lib
```

**2,865 passed, 0 failed, 303 ignored** (25.54 seconds test execution).
This includes **35 passing squire-focused tests**; the existing squire art
exporter remains ignored. Final log: `/tmp/qsquires-full.log`. Target directory:
`<build-cache>/qsquires-fix`.

Only the existing unused-variable warnings in `gore.rs` (`cut`),
`alehouse__tests.rs` (`b`) and `together_open_floors__tests.rs` (`w`) were
reported. `git diff --check` is clean. No tavern implementation files were
changed. No live host, browser/cockpit tests or screenshot export was run;
the client regression is an offline in-process wire/draw test. The main driver
still owns the final integrated-main suite.
