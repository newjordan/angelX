# Knights Wish village production

The user expanded the active development goal to **48 hours of sustained work** on 2026-11-05 at approximately 20:49 UTC. This supersedes the earlier eight-hour scope. A successful milestone is a reason to advance to the next useful system, not to mark the whole goal complete. Actual elapsed active work and completed evidence belong in this log; parallel agent time must not be represented as elapsed hours.

## Playable target

Build a comfortable, intricate medieval magical village into a town and castle. Mine, cut, farm, process, transport and construct. Residents have individual names, jobs and needs. Adventurers discover dangerous ore, secure claims, equip themselves using the settlement's output, and bring new possibilities home. Village growth and expedition progress should continually give each other purpose.

The selected working visual direction is [village-vision.png](../cockpit/assets/kingdom/concepts/village-vision.png), generated with built-in ImageGen. It is concept art, not a runtime screenshot. Warm woodland greens, ochre timber, cream plaster, terracotta bricks, slate roofs, amber fires and restrained teal magic. Small readable workers and animated machinery give the settlement life. Orthogonal gameplay placement and tile readability take priority over the concept image's decorative perspective.

## Work sequence and evidence

1. Clay and brick construction: clay bank → clay pit → fired bricks → masonry homes, wall segments and gates. Physical deliveries, construction stages and readable shortages. Verify a fresh settlement can complete the chain without debug stock.
2. Residents: persistent individuals, migration, beds, jobs, food and recoverable shortages. Empty housing does not manufacture instant workers. Verify production and hunger balance over a sustained session.
3. Landscape: persistent 72×54 terrain, safe migration of old 24×18 saves, resource-bearing parcels, river sites and useful expansion objectives.
4. Power and transport controls: waterwheel/mechanical and magical supply networks, local shortages, player priorities and routes. Breaking a connection must have understandable consequences and repair must restore operation.
5. Building a village: useful homes, food services, workshops, stores, courtyards, modular walls and entrances, visible inhabitants and equipment. Test the experience through actual inputs, inspect runtime art and animation.
6. Expedition purpose: new ore types signalled by enemies, claim objectives, material-driven equipment and settlement progression. Preserve the existing campaigns and combat.
7. Continue from the largest remaining player-facing gap, including city growth, construction ergonomics, richer logistics, simulation consequences, sound, polish and long-session verification.

## Ownership and architecture

The Rust `angel-kingdom` crate owns deterministic simulation and validated commands. Content stays in `delve/kingdom/frontier.json`. Delve's industry adapter owns real input, campaigns and persistent Home integration. Render modules display simulation state; animation never decides production or damage. New subsystems get separate modules and focused scenario tests.

Root: integration, production chains, power/logistics, input and UI, runtime acceptance. `kingdom_graphics`: construction and machinery concepts, generated modular pixel assets, animation and import helpers. `item_art`: population simulation, worker concept and animated NPC assets. `monster_art`: terrain, expansion, resource geography and expedition consequences. Each agent owns isolated files until an explicit integration handoff.

The user explicitly authorized implementation and autonomous iteration. No extra approval gate is inferred from the consulted [Game Dev Studio workflow](https://github.com/D1124423017/game-dev-studio/blob/main/SKILL.md). Its useful requirements are explicit visual direction, modular implementation, inspected runtime evidence and honest testing.

## Active work log

- 20:49 UTC: scope extended to 48 hours; three agents reactivated with concrete file ownership. Read Game Dev Studio skill and production/art/architecture references. Main target image generated and retained with prompt. Current implementation still has a small founding map, abstract labour and basic road carriers; these are the next implementation gaps.

- 21:31 UTC: inspected the actual 347-second village run: clay, fired bricks, staged cottage, modular walls/archway, seven named residents, food and moving deliveries. The scenario used normal `Input` actions and full authoritative `Run` ticks, with no added inventory or claims. Runtime frames are in `/tmp/village-review`; these are distinct from the concept images. Core population, terrain, food reserve and locomotion scenarios were green (53 tests before power integration).
- 21:39 UTC: all 62 pure kingdom tests pass with mechanical shafts and arcane leylines integrated into the real factory. Optional paid drives, road-connected staffed sources, local fuel deliveries, whole-machine power allocation, manual fallback, bounded faster production, cut/repair and save validation are implemented. The production catalogue adds river waterwheels, ley generators and both connector plans. Full game input/render verification is underway; the core test result alone does not prove the current native or browser executable.
- Latest user steering: continue backlog work and general fixes. Current priorities are mouse building, actual village power progression, clearer workshop diagnostics, knight scale, depth ordering and readable terrain transitions. Subagents remain assigned to concrete implementation and review tasks.

## Model-switch handoff — 2026-11-05, 21:54 UTC

The user requested a wind-down to switch models and reduce reasoning effort. Stop new feature work. All subagents have frozen their source and finished. Wind-down verification is complete; this is the pause checkpoint for the model switch. Resume only when the user requests it. Do not mark the overall village/game goal complete. The user also asked to leave the prompt/quant visualizer aside; none of this work depends on it.

### Current implementation and evidence

- The real-input village scenario reaches 519 simulated seconds and nine residents from normal founding stock. It pays for brick construction, Stonecraft, a mason, a river waterwheel, a shaft route and a powered sawmill drive. Cutting the shaft stops plank production; repair resumes it. A complete Run snapshot round-trips. No stock or claims are injected into this scenario.
- All 67 `angel-kingdom` tests pass. This includes population/food/terrain/pathing, real economy progression, separate power graphs, source fuel delivery, per-batch recipe costs, cut/repair, claim-cap recovery, cargo preservation, earned resource history and unpowered worker animation.
- Before the latest claim fixes, engine checks passed: industry 15, builder helpers/codec 5, actual client input 3, command regressions 8, frontier objectives 4. The actual client checks cover pointer-to-command-to-host placement, closed desk gating and C/Space/E mappings.
- Final engine verification: PASS at 21:59 UTC. 56 targeted checks passed: industry/Claims UI 16, claim retirement 3, ore sentries 8, claim bosses 9, expansion objectives 4, builder helpers/codec 5, actual client controls 3, command regressions 8. Three visual tests were intentionally ignored in the ordinary encounter filters; the actual powered-village visual scenario was run separately and passed. The final build has the existing 15 unrelated warnings. Logs: `/tmp/angelx-winddown-engine.log`, `/tmp/angelx-winddown-checks/`, `/tmp/angelx-winddown-capture.log`. A missing BTreeSet import and two incorrect new test-fixture assumptions were corrected during this verification. No failing targeted checks remain.
- Generated art is retained, not temporary: `cockpit/assets/kingdom/concepts/village-vision.png`, `village/`, `citizens/`, `world/`, with prompts and manifests. Village art has 11 masters, 27 groups and 49 frames. Use existing assets before requesting more images.
- Final runtime capture: PASS. `write_powered_village_review` was rerun against the final gameplay/render source and reached 519 seconds with nine residents. Root inspected the full-width walking scene and unobstructed shaft route. Six verified captures and checksums are retained in `docs/images/kingdom-works/village/`; the complete frames and latest snapshot remain in `/tmp/powered-village-review/`. The only subsequent changes were corrections to two test fixtures, not gameplay or rendering.

### Files and ownership

- `crates/kingdom/src/`: deterministic economy, transport, construction, population, citizen motion, geography, power and claims. `delve/kingdom/frontier.json` is the production catalogue.
- `crates/delve/src/rules/industry.rs`: surface adapter, Build/People/Land/Works/Claims controls and per-seat Board state. Browser actions go through the same authoritative commands. Releasing an unused claim requires two separate X presses; 2 unlinks its mine first.
- `rules/frontier_claims.rs`: safe retirement and bounded ledger-full notices. Retain encounter death records after releasing a claim to prevent duplicate loot. `secured_resources` preserves historical charter evidence; old saves derive it from existing secured claims.
- `rules/ore_threats.rs`: crowded seams must admit/defeat their sentry roster before guardian challenge; already-challenged legacy guardians remain compatible.
- `rules/builder_input.rs`, `client.rs`, `rules/command.rs`, `crates/net/src/client.rs`, `crates/web/src/lib.rs`, `crates/web/shell/boot.js`: bounded one-shot pointer command, hit testing, queueing, wire and browser dispatch. Builder map header clicks are ignored; mouse placement freezes the camera until keyboard panning resumes.
- `render/overworld/arena/frontier_scene.rs`: village world draw. Build viewport stays 448×308 with exact hit mapping; walking is 744×308 and uses continuous world-position camera coordinates. Structures, actors and cargo share depth order. Decorative rocks/trees do not obscure power corridors.
- `arena/industry.rs`, `arena/village_ui.rs`: ledgers, costs, shortages, power allocation and claim controls. Global power header reports allocated/requested units; local controls distinguish connection and capacity failures.
- `arena/village_art.rs` and `crates/delve/village_art_build.rs`: retained generated sprites, construction stages and machinery/citizen animation.
- `crates/delve/tests/render/village__shots.rs`: `write_powered_village_review` creates the real-input scenario and all proof frames. `docs/KINGDOM-WORKS.md` is the gameplay/architecture guide.

### Known unfinished work — resume in this order

1. **Local browser preview has not launched.** Native dev-host and browser bundle compiled, but startup panics with `rooms-rules … no rules engine is installed` because the dev-host dependency lacks the `bend-engine` feature. No Cargo fix or feature rebuild has been performed. Build it with the command below, then launch the helper and review real browser clicks, tab/list selection and power controls. The existing bundle predates the final claim/viewport fixes and should be rebuilt after those are verified.
2. **Native desktop mouse dispatch is not implemented.** Keyboard and browser pointer paths work. Investigation only: cockpit's `on_mouse` returns early for active dungeon views. Record the exact rectangle from `game_placement()` in `shooter_viz::render_seat`; queue validated terminal-cell-centre to frame-pixel hits on the displayed `Seat`; forward via `NativeLink` to `Runtime::pointer`; dispatch inside that existing dungeon mouse branch while preserving modal gating. Do not guess cell aspect/letterboxing. Add a real Sim→Seat input test and pure coordinate tests. No edits in NativeLink/Seat/RealmHost/mouse were started.
3. **Walking hero stride/facing still needs a small fix.** `step_frontier` updates position but bypasses normal `Hero::own_keys` and `Run::watch`, so it does not update aim toward movement or advance/reset stride. Inspect `hero.rs` around `own_keys` and `spectacle.rs` footsteps; update only frontier locomotion, preserving equipped gear and collision. Do not accidentally enable combat or duplicate normal combat ticks. NPC movement and work animations are already implemented.
4. Broaden gameplay after those fixes: larger settlement logistics, readable building controls and shortages, ore/claim progression and long-session checks. Citizen walking is visual; commute time does not yet gate production credit. Do not claim relationships, a full daily schedule, conveyors or fluid networks exist.
5. Rebuild and verify the production worker/native app after the final source settles. Prior native app and installed worker artifacts are from the earlier pass; do not present them as containing this latest village work.

### Exact commands and artifacts

Pure core:

```bash
CARGO_TARGET_DIR=/tmp/angelx-economy-target cargo test --manifest-path crates/Cargo.toml -p angel-kingdom --offline
```

The full engine test harness is large. Avoid default debug symbols and optimized test codegen (an earlier run exhausted its quota). Use:

```bash
CARGO_TARGET_DIR=/tmp/angelx-final-test-target CARGO_INCREMENTAL=0 CARGO_PROFILE_TEST_DEBUG=0 cargo test --manifest-path crates/Cargo.toml -p angel-delve --offline --config 'profile.test.package.angel-delve.opt-level=0' --lib rules::industry::tests -- --test-threads=1
```

After that build, run other filters directly with `/tmp/angelx-final-test-target/debug/deps/angel_delve-5c27e69908e71370` to avoid unnecessary recompilation. Relevant filters: `frontier_claims`, `ore_threats`, `claim_boss`, `frontier_objectives`, `builder_input`, `client::tests::builder_tests`, `rules::command::tests`. Visual proof: `write_powered_village_review --ignored --nocapture --test-threads=1`. Its default destination is `/tmp/powered-village-review`; `ANGEL_POWER_SHOTS` overrides it. The generated `powered-run.json` is real scenario state.

Preview host (cached V8 avoids network download):

```bash
CARGO_TARGET_DIR=~/build-cache/angelX/cockpit-target \
RUSTY_V8_ARCHIVE=~/build-cache/angelX/crates-target/systems/fast/gn_out/obj/librusty_v8.a \
CARGO_NET_OFFLINE=true \
cargo build --manifest-path crates/Cargo.toml --profile fast -p angel-delve-dev-host --bin delve-dev-host --features angel-delve/bend-engine

VILLAGE_HOST=~/build-cache/angelX/cockpit-target/fast/delve-dev-host \
scripts/dev/village-preview.sh /tmp/powered-village-review/powered-run.json
```

The launch helper is syntax checked. It binds localhost:8820 and keeps its isolated state in `/tmp/angelx-village-session`; it resumes an existing checkpoint without overwriting it. This is a preview, not the user's live realm. The initial host binary `/tmp/angelx-preview-native-target/debug/delve-dev-host` compiled but cannot start without Bend. Its startup failure is in `/tmp/angelx-village-preview.log`. Build logs: `/tmp/angelx-village-native-build.log`, `/tmp/angelx-village-web-build.log`. Completed browser bundle: `crates/web/dist/delve-village/` (game WASM and both GPU presenters).

Production native build uses the same cached target/archive and `cargo build --manifest-path cockpit/Cargo.toml --profile fast --bin angel`. Writing the `~/build-cache` target needs sandbox escalation; auto-review approved this authorized project build in the preceding pass. No commits or publication were requested. Preserve the shared working tree and its untracked art/source files; do not reset or clean it.
