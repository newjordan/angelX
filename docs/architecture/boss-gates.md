# Delve floor-local encounter economy and boss access

This is the bounded first playable slice over the **existing six-floor**
generator. It supersedes the experimental fixed-two-warden graph. It is not
Undernetwork terrain expansion, NPC-on-NPC warfare, a permanent world economy
or an LLM service.

## Ownership and generation workflow

- `boss_population.rs`: deterministic terrain/resource profile, capacity,
  leader count and placement, initial finite camps/factions and access shape.
- `boss_ecology.rs`: serialized finite economy, one-shot transition journal,
  roster generation, spent troop slots, control loss and capped succession.
- `boss_gates.rs`: access predicate, immutable encounter/geography identity,
  actual boss-death evidence, status projection and admission validation.
- `encounter_catalog.rs`: checked pack-appropriate leaders/apex definitions,
  distinct indices, compatible custom definitions and the 64-entry cap.
- Top-level `Run`: populates on new floors, deploys on entry, associates enemy
  faction/slot identity, settles casualties/deaths before clear or wipe, and
  refuses rewards/exits from locked rooms. `hazards.rs` suppresses legacy
  timed waves only for economic camps. Existing traps and boss combat remain.
- `mirror.rs`: host-to-view finite state, monotonic identity/progress admission
  and atomic live updates. `together_guest.rs`, `shooter_viz.rs`, `joined_viz.rs`
  and the optional `assets/dungeon/guest.html`: status/map/resource projection.

Standard floors populate before entering room zero. Home/depth zero, depths
beyond `DEEPEST = 6`, empty/malformed room geometry, insufficient reachable
fight sites, full catalogs lacking required definitions, and Gauntlet/Hold the
Stair decline this policy and keep the safe legacy path. Population stages
catalog edits on a clone so failure cannot partially extend it. No combat RNG
is consumed by profile/leader selection.

## Causal resource profile, not a random policy name

The salt is `mix(seed ^ raid.rotate_left(17) ^ depth.rotate_left(33))`.
For each ordinary fight room, **interior** counts of hazard (`wet`), block
(`works`) and floor (`open`) tiles measure support. Boundary door opening does
not change these counts. Integer yields are:

| Pack | Structure / resource | Capacity units before depth/seed |
|---|---|---|
| Fungal | Hive / brood | `wet/8 + open/180` |
| Unknown | Hive / echo brood | `wet/8 + open/180` |
| Cavern | Clans / ore | `works/8 + open/280` |
| Archive | Clans / salvage | `works/12 + wet/12 + open/280` |
| Crypt | Stronghold / relic tribute | `works/8 + open/280` |
| Hellforge | Stronghold / fuel | `wet/8 + works/16` |

Add `mix(salt ^ room) % 2` and `depth/3`, cap capacity at six, and sum for floor
support. Integer mean capacity drives extra chiefs/apex eligibility. This is
a coarse terrain-backed **support proxy**, not a mining interaction or a
simulation of trade. Tests replace terrain with seed/depth/pack fixed and
assert actual encounter changes.

Base budgets are Hive `8 + 2*capacity`, Clans `3 + capacity`, Stronghold
`3 + capacity/2`. Pools differ by pack: hives favor bats/imps with at most one
splitting slime; clans include a bounded sapper/hob specialist and, on eligible
contested sites, one troop from another living chief's claim; tribute courts
use pack-appropriate skeleton/wraith or forge defenders. Initial faction
reserve is its productive capacity sum capped at six. A camp gets at most two
reserve units on its single deployment; the finite roster budget caps at 24.
The guardian camp uses mean capacity and is a separate final deployment.

Leader sites are ranked by productive capacity with seeded ties. Prerequisite
sites are reachable from entry in the reciprocal room graph **without crossing
the guardian**. No graph edge points from a prerequisite through the sealed
apex. There are no arbitrary dynamic dependency edges:

- **Hive:** one required brood chief; a second optional chief if mean capacity
  >= 4. Rich Fungal beds support a rare separate apex only when `salt % 4 == 1`.
  Unknown preserves its native Grail threshold boss. Actual chief death is the
  prerequisite, never a post-starvation resource threshold.
- **Clans:** three independent chiefs (four at mean >= 4), bounded by sites.
  **ANY** chief death opens an alternative route, without a universal sole
  exit boss. Rival roster composition is real; rival AI fighting one another
  is intentionally **not implemented**.
- **Stronghold:** two required chiefs (three at mean >= 4), and an optional
  captain if mean >= 2 and sites/count cap permit. **ALL** required chiefs
  must die. A separate apex guards the exit; native Dragon Keep retains
  `EnemyKind::Dragon`, and the bottom retains Grail behavior.

These are real checked `.boss` definitions, not ordinary mob kills renamed as
leaders. Supplemental native definitions include two orc claim chiefs for the
Mines, appropriate brood/tribute/salvage leaders and distinct apices. Checked
workspace bosses remain eligible for their pack. A custom apex is not also
recycled as a prerequisite. The complete boss catalog is immutable for an
active floor and serialized with `Run`.

## Finite state and transition review

`Run.boss_gates: Option<Gates>` serializes:

- depth, guardian room/optional catalog boss, native optional Pit room and
  their actual-death flags;
- immutable room `Location` identities (cells, doors, room kinds);
- `access: any|all`, up to four `Leader`s (room, catalog index, faction,
  prerequisite membership, defeated flag);
- `Economy { profile, factions, camps, events, spawn_allowance, spawn_spent }`: measured capacities,
  immutable base budgets/controller identities, mutable reserve/control,
  deployment rosters and a `u32` remaining-slot bitset.

Events are `Deploy(camp)`, `Casualty { camp, slot }` and `Loss(leader)`. Each
deployment, slot death and controller loss occurs at most once. There is no
replenishment event. Real leader death marks defeat/removes control before
room-clear/wipe handling. It empties reserve; its **undeployed** camps lose six
support units for a hive, two otherwise, floored at two troops. Tribute/hive
supply loss also weakens an undeployed apex escort. In clans the next living
rival receives at most three unused reserve units, capped at nine. Already
deployed troops remain physically present; their admitted budget does not
rewind when a chief dies.

Entry reuses the admitted deployment and spawns only living slots. Actual
zero-HP settlement spends a slot even if the party wipes that tick. Replaying
a spent camp corpse/defeated boss awards no loot, score, bounty, shock or
political credit. A boss cannot respawn while its escorts survive. Optional
native Pit Tyrant death is independent and once-only, giving no route/economy
credit. Ordinary visits/kills/clears and removing a living boss cannot claim
a leader death.

Clear requires economic slots and the encounter boss to be settled. A sealed
active room has no enemies (including an AoE target), hostile shots, traps,
rocks, waves, rewards, light, stairs or victory. Doors permit retreat. Native
first-floor Sanctuary conversion remains allowed after clear. Dragon's
light-home victory and the final Grail victory remain admitted; an empty
locked room cannot produce either.

## Extra-body provision (repair)

Camp deployment is not the entire population budget. Every dynamic `spawn_at`
or `spawn_staged` on a gated floor spends one shared provision, initially
`min(128, 8 + 4 * profile.support)`. This immutable resource-derived allowance
and cumulative spent count live in `Economy`. Its existing bounded journal adds
`Emit { room, id, kind, stage }` and `Retire(id)`, rather than a second roster or
producer-lifetime counters. Emit IDs are increasing, floor-local body identities;
slime stage is part of identity, while mutable native combat stages are not.

The debit follows successful raw insertion, after tile/kind/stage/ID/cap checks.
`apply_deeds`, slime splitting, incidental goblins, mimic wakes and indirect
wave emission and non-camp initial rosters (entrance/ledge) all converge here,
including the optional Pit arena. Death is
retired before rewards/descendants, and escape/reentry retires discarded bodies
without refund. Journal identity prevents duplicate corpse drops/splits and
admits active extra bodies after serialization. Camp slots and required/native
encounter bodies use raw admitted insertion, outside this allowance: exhaustion
cannot prevent them appearing or clearing. Empty provision suppresses only extra
bodies; a failed mimic wake preserves its chest. All ungated legacy/special-mode
spawning remains on its previous path. Checked `.boss` attack vocabulary has no
summon opcode; native necromancer/shaman/brood emissions use the same provision.

Bounds: 64 sites/camps/geography rooms, 4 factions/leaders, 24 mutable roster
entries per camp, 128 lifetime emitted bodies and 96 simultaneous bodies.
The journal is at most `25 * camps + leaders + 2 * allowance` events and never
more than **1,860** (64 deployments, 1,536 casualties, 4 losses, 128 emissions,
128 retirements). These bounds, faction-name bounds and indices are checked
before cloning/replay/indexing, both for stored mirror state and incoming Live.
Replay clones independently reconstructed bounded genesis, never received mutable
rosters. Serde decoding/HTTP transport allocation is not itself a streaming
bounded parser; admission rejects oversized decoded state before mirror cloning.

## Admission, persistence and guest projection

Validation checks bounds/indices/geometry. It normalizes only the declared
found-secret doorway and first-floor cleared Sanctuary, then reconstructs
population/genesis against immutable identity. It replays the finite economy
journal, verifies every derived reserve/roster/control/budget and links loss to
leader defeat/deployment. Cleared camps must have deployed/spent all slots,
including nonactive rooms. Expected active bosses, native Dragon/Pit,
faction/slot assignments, duplicate IDs/slots, locks and victory are checked.

`Live` carries the whole finite state. Within a floor `same_encounters` forbids
identity changes; `permits_progress` permits monotonic defeat and an append-only
valid journal. Atomic clone-validate-commit rejects bad deltas without poisoning
the view. Legitimate room/secret/clear progress is projected before admission.
`Live.light` is serde-default `Option<(f32, f32)>`, matching Run. The native
Dragon's clear creates the canonical marker; light-home travel retains it as
consumed Won evidence, whose exact position, floor (FLOORS = 3), cleared guardian
and actual defeat are validated. A mirror initialized before Dragon death accepts
clear → light → Won in sequence without relaxing victory invariants.

Secret admission is not conditional on gates: snapshots validate host/side/vault,
fight-host/vault adjacency, dimensions, door relation and payment state. Immutable
Snibbet flavor is recomputed from pre-vault geography, without copying room data. Deltas
must preserve host/side/vault/snibbet identity and advance found/paid monotonically.
Stored secret metadata is validated before `open_secret` as well; ungated and
gated deltas both stage atomically, preserving discovery and door traversal.

A new-floor full snapshot replaces the graph/economy. Clients never author
this state: HTTP inputs are controls/allowed intents, not defeat assertions.

Absent `boss_gates` in a legacy save leaves the **current** floor on its old
population; the next standard floor gets the new state. Empty catalogs are
supplemented safely. The superseded experimental two-warden schema is not
migrated: it rejects rather than guessing new progress.

Host/native-guest HUD/map show living chiefs as `C`, access shape, checked
names/factions, resource and changed control/local support. Compact/normal
native captures are deterministic tests. By default an invitation opened in a
browser only says to paste it into angelX. With the browser view on
(`/dungeon_host --view`, or `ANGEL_DUNGEON_BROWSER_VIEW=1`) it is read-only:
bearer-authenticated HUD/PNG, bounded four-chief rows, 2 Hz sequential polling,
`textContent` for data, revoked PNG object URLs and a same-origin/blob image
CSP. It shows actual checked names and route/control/reserve changes; playing
still requires native join. No private forge provenance is added.

## Bounds and verification workflow

- Classic depth <= 6; <= 64 serialized rooms; <= 4 political leaders/factions.
- <= 64 catalog entries, below the `u8` index-256 ceiling. Appended native
  templates pass the checker and have distinct pack-appropriate IDs.
- <= 24 troop slots/camp; <= 96 live enemies on economic floors including
  summons/children; spawning refuses overflow.
- <= 64 camps and <= 128 lifetime extra bodies, hence <= **1,860** journal
  events: four losses, one deployment and at most 24 casualties/camp, plus
  one Emit/Retire pair per provisioned extra body. No per-frame economy timers
  or journal append on repeated settled-corpse replay; room entry can retire
  abandoned active bodies but never refund provision. Ungated legacy combat
  and special-mode waves are not a persistent finite-population model.

Existing `together_shooter` regressions in
`tests/cockpit/app/together_boss_gates__tests.rs` cover terrain-backed profiles,
changed resources, distinct density/rosters/graphs, chief death/capped aftermath
once, ANY/ALL order, starvation, reciprocal reachability across both starting
packs/seeds/depths, legacy saves/roundtrips/reset, invalid graph/economy, atomic
mirror deltas, transactional catalog/malformed-geometry rejection, no loss
before camp admission, empty catalog/geometry and special modes, finite
deployment and provision exhaustion through real native raising/brood paths,
wipes/reentry/corpse replay, active descendant save/mirror admission, exhausted
Pit completion, oversized rosters/journals, poisoned secrets/badthenfound/door
traversal, production Dragon→light→Won mirrors, real guest HTTP/CSP and
80x24 / 120x40 renderer/guest projections. Unrelated combat assertions remain;
fixture helpers (`enter_for_test`/`spawn_at_for_test`) explicitly select the
retained legacy roster/wave path, while
ecological regressions use **production entry**.

The focused verifier:

```sh
cargo test --locked --manifest-path cockpit/Cargo.toml --no-default-features \
  --bin angel together_shooter -- --nocapture
```
