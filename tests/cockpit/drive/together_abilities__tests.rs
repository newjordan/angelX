use angel_delve::rules::abilities::{Command, Origin};
use angel_delve::rules::{Input, RoomKind, Run};
use angel_live::Registry;
use angel_live::kernel::Resource;
use angel_realm_host::together_abilities::*;
use serde_json::{Value, json};
use std::collections::BTreeMap;
use std::path::PathBuf;
use std::sync::Arc;

struct Fixture {
    root: PathBuf,
    _retirement: crate::drive::together_media::RetirementGuard,
}
impl Fixture {
    fn new() -> Self {
        static SERIAL: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
        let root = std::env::temp_dir().join(format!(
            "angel-abilities-{}-{}",
            std::process::id(),
            SERIAL.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
        ));
        for module in [
            "abilities",
            "ability-code",
            "ability-sounds",
            "ability-effects",
            "characters",
        ] {
            std::fs::create_dir_all(root.join(module)).unwrap();
            for entry in std::fs::read_dir(
                std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
                    .join("live")
                    .join(module),
            )
            .unwrap()
            {
                let entry = entry.unwrap();
                std::fs::copy(entry.path(), root.join(module).join(entry.file_name())).unwrap();
            }
        }
        let mut manifest: Value =
            serde_json::from_slice(&std::fs::read(root.join("ability-code/module.json")).unwrap())
                .unwrap();
        manifest["assets"]
            .as_array_mut()
            .unwrap()
            .push(json!("sound.bin"));
        std::fs::write(root.join("ability-code/module.json"), manifest.to_string()).unwrap();
        std::fs::write(root.join("ability-code/sound.bin"), b"old sound").unwrap();
        Self {
            root,
            _retirement: crate::drive::together_media::RetirementGuard,
        }
    }
    fn registry(&self) -> Registry {
        let mut registry = Registry::with_resources(
            self.root.clone(),
            crate::agent::hotload::evaluate,
            |_, _| Ok(()),
            crate::agent::hotload::bind_resource,
        );
        registry.refresh(true);
        registry
    }
    fn source(&self) -> String {
        std::fs::read_to_string(self.root.join("ability-code/spells.js")).unwrap()
    }
    fn edit(&self, source: &str) {
        std::fs::write(self.root.join("ability-code/spells.js"), source).unwrap();
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.root);
    }
}
fn select(registry: &mut Registry, args: &str) {
    registry
        .dispatch(
            angel_live::kernel::Kind::Command,
            "ability",
            &json!({"type":"command","arguments":args}),
        )
        .unwrap();
}
fn sources(registry: &Registry) -> Vec<Resource> {
    registry
        .snapshot()
        .resources
        .into_values()
        .filter(|resource| resource.binding::<Loadout>().is_some())
        .collect()
}
fn fight() -> Run {
    let mut run = Run::new(3, 9, None);
    let room = run
        .dungeon
        .rooms
        .iter()
        .position(|room| room.kind == RoomKind::Fight)
        .unwrap();
    run.enter_for_test(room);
    run.calm_for_test();
    run
}
fn tick(runtime: &mut Runtime, run: &mut Run, cast: u8) {
    runtime.before_step(run);
    run.step(&BTreeMap::from([(
        1,
        Input {
            cast,
            ..Default::default()
        },
    )]));
    runtime.after_step(run);
}

#[test]
fn executable_swap_finishes_old_casts_and_uses_new_code_for_future_casts() {
    let fixture = Fixture::new();
    let mut registry = fixture.registry();
    select(&mut registry, "spark");
    let mut runtime = Runtime::default();
    let mut run = fight();
    runtime.set_sources(sources(&registry), &mut run);
    tick(&mut runtime, &mut run, 1);
    assert_eq!(
        runtime.started,
        1,
        "rules did not admit the module cast: {}",
        runtime.status()
    );
    assert_eq!(runtime.effects, 1);
    assert!(run.projectiles.iter().any(|p| {
        p.damage == 28
            && p.traits
                .module_origin
                .as_ref()
                .is_some_and(|origin| origin.generation == 1)
    }));
    let weak = {
        let snapshot = registry.snapshot();
        Arc::downgrade(
            snapshot
                .assets
                .values()
                .find(|asset| asset.bytes == b"old sound")
                .unwrap(),
        )
    };
    let hp = run.players[&1].hp;
    let mana = run.players[&1].mana;
    let cooldown = run.players[&1].spell_cooldowns;
    fixture.edit(&fixture.source().replace("damage: 28", "damage: 77"));
    std::fs::write(fixture.root.join("ability-code/sound.bin"), b"new sound").unwrap();
    registry.refresh(true);
    assert_eq!(
        registry.snapshot().statuses[0].generation,
        2,
        "{:?}",
        registry.snapshot().statuses
    );
    runtime.set_sources(sources(&registry), &mut run);
    assert_eq!(
        (
            run.players[&1].hp,
            run.players[&1].mana,
            run.players[&1].spell_cooldowns
        ),
        (hp, mana, cooldown)
    );
    assert!(
        weak.upgrade().is_some(),
        "active cast released its old source/assets"
    );
    for _ in 0..5 {
        tick(&mut runtime, &mut run, 0);
    }
    assert_eq!(runtime.effects, 2);
    assert!(run.projectiles.iter().any(|p| {
        p.damage == 28
            && p.traits
                .module_origin
                .as_ref()
                .is_some_and(|origin| origin.generation == 1)
    }));
    assert!(runtime.groups.is_empty());
    assert!(
        weak.upgrade().is_some(),
        "finished cast released a still-playing effect's source"
    );
    while run
        .module_visuals
        .effects
        .iter()
        .any(|effect| effect.origin.generation == 1)
    {
        tick(&mut runtime, &mut run, 0);
    }
    assert!(
        weak.upgrade().is_none(),
        "retired cast/effect still holds old assets"
    );
    while run.players[&1].spell_cooldowns[0] > 0 {
        tick(&mut runtime, &mut run, 0);
    }
    tick(&mut runtime, &mut run, 1);
    assert_eq!(runtime.started, 2);
    assert!(run.projectiles.iter().any(|p| {
        p.damage == 77
            && p.traits
                .module_origin
                .as_ref()
                .is_some_and(|origin| origin.generation == 2)
    }));
}

#[test]
fn bad_candidate_keeps_the_entire_working_ability_generation() {
    let fixture = Fixture::new();
    let mut registry = fixture.registry();
    select(&mut registry, "spark");
    fixture.edit(&fixture.source().replace("damage: 28", "damage: -1"));
    registry.refresh(true);
    assert_eq!(registry.snapshot().statuses[0].generation, 1);
    assert!(
        registry
            .snapshot()
            .statuses
            .iter()
            .any(|status| status.error.is_some())
    );
    let mut runtime = Runtime::default();
    let mut run = fight();
    runtime.set_sources(sources(&registry), &mut run);
    tick(&mut runtime, &mut run, 1);
    assert_eq!(runtime.started, 1);
    assert_eq!(runtime.effects, 1);
}

#[test]
fn invalid_actual_batch_does_not_partially_apply_and_cancel_does_not_replay() {
    let fixture = Fixture::new();
    let good = fixture.source();
    fixture.edit(&good.replace(
        "damage: cast.parameters.damage",
        "damage: player.hp===7 ? -1 : cast.parameters.damage",
    ));
    let mut registry = fixture.registry();
    select(&mut registry, "spark");
    let mut runtime = Runtime::default();
    let mut run = fight();
    run.players.get_mut(&1).unwrap().hp = 7;
    runtime.set_sources(sources(&registry), &mut run);
    tick(&mut runtime, &mut run, 1);
    assert_eq!(runtime.started, 0);
    assert_eq!(runtime.effects, 0);
    assert_eq!(run.players[&1].mana, 100.0);
    assert!(runtime.groups.values().any(|group| group.fault.is_some()));
    // A resource update in the same generation cannot clear quarantine.
    select(&mut registry, "bloom 1 2");
    runtime.set_sources(sources(&registry), &mut run);
    assert!(runtime.groups.values().any(|group| group.fault.is_some()));
    for _ in 0..3 {
        tick(&mut runtime, &mut run, 0);
    }
    assert_eq!(runtime.effects, 0);
    fixture.edit(&good);
    registry.refresh(true);
    select(&mut registry, "cancel 1");
    runtime.set_sources(sources(&registry), &mut run);
    assert!(runtime.pending.is_empty());
    assert!(runtime.groups.is_empty());
    tick(&mut runtime, &mut run, 1);
    assert_eq!(runtime.started, 1);
    assert_eq!(runtime.effects, 1);
}

#[test]
fn bounded_capacity_defers_once_and_unaffordable_starts_do_not_stall_active_casts() {
    let fixture = Fixture::new();
    let mut registry = fixture.registry();
    select(&mut registry, "spark 1 1");
    select(&mut registry, "spark 1 2");
    let mut runtime = Runtime::default();
    let mut run = fight();
    run.players.get_mut(&1).unwrap().mana = 12.0;
    runtime.set_sources(sources(&registry), &mut run);
    run.apply_ability_commands(
        (0..angel_delve::rules::MAX_PROJECTILES)
            .map(|_| {
                (
                    1,
                    Origin {
                        module: "fixture".into(),
                        generation: 0,
                        cast: "capacity".into(),
                    },
                    Command::Projectile {
                        offset: [0.0, 0.0],
                        velocity: [0.0, 0.0],
                        damage: 0,
                        ttl: 3600,
                        pierce: 0,
                        kind: angel_delve::rules::Shot::Bolt,
                    },
                )
            })
            .collect(),
    );
    tick(&mut runtime, &mut run, 1);
    tick(&mut runtime, &mut run, 2);
    assert_eq!(runtime.pending.len(), 2);
    assert_eq!(runtime.started, 0);
    assert_eq!(runtime.effects, 0);
    assert_eq!(run.players[&1].mana, 12.0);
    run.projectiles.clear();
    tick(&mut runtime, &mut run, 0);
    assert_eq!(runtime.started, 1);
    assert_eq!(runtime.pending.len(), 1);
    assert_eq!(run.players[&1].mana, 4.0);
    for _ in 0..5 {
        tick(&mut runtime, &mut run, 0);
    }
    assert_eq!(
        runtime.effects, 2,
        "the unaffordable start stalled an already paid cast"
    );
    assert_eq!(runtime.started, 1);
    assert!(
        runtime
            .groups
            .values()
            .all(|group| group.casts.is_empty() && group.fault.is_none())
    );
    assert_eq!(runtime.pending.len(), 1);
    // Re-observing a committed tick cannot issue the projectile again.
    runtime.after_step(&mut run);
    assert_eq!(runtime.effects, 2);
    select(&mut registry, "cancel 1");
    runtime.set_sources(sources(&registry), &mut run);
    assert!(runtime.pending.is_empty());
    assert!(runtime.groups.is_empty());
}

#[test]
fn unsupported_ability_contract_cannot_remove_the_working_binding() {
    let fixture = Fixture::new();
    let mut registry = fixture.registry();
    select(&mut registry, "spark");
    let manifest = fixture.root.join("abilities/module.json");
    let old = std::fs::read_to_string(&manifest).unwrap();
    std::fs::write(
        manifest,
        old.replace("character-loadout/1", "character-loadout/2"),
    )
    .unwrap();
    registry.refresh(true);
    assert_eq!(registry.snapshot().statuses[0].generation, 1);
    assert!(
        registry
            .snapshot()
            .statuses
            .iter()
            .find_map(|status| status.error.as_deref())
            .unwrap()
            .contains("unsupported host resource contract")
    );
    assert!(sources(&registry)[0].binding::<Loadout>().is_some());
}

#[test]
fn adding_an_authored_ability_while_a_cast_runs_needs_no_new_native_branch() {
    let fixture = Fixture::new();
    let mut registry = fixture.registry();
    select(&mut registry, "spark");
    let mut runtime = Runtime::default();
    let mut run = fight();
    runtime.set_sources(sources(&registry), &mut run);
    tick(&mut runtime, &mut run, 1);
    let source = fixture.source()
        .replace("exports.definitions = {", "exports.definitions = { blink: {name:'Blink',mana:2,cooldown:10,icon:'P'},")
        .replace("} else if (cast.ability === 'bloom') {", "} else if (cast.ability === 'blink') { emit({type:'move',delta:[0.25,0]}); finished.push(id); delete casts[id]; continue } else if (cast.ability === 'bloom') {");
    fixture.edit(&source);
    registry.refresh(true);
    select(&mut registry, "blink 1 2");
    runtime.set_sources(sources(&registry), &mut run);
    let position = run.players[&1].x;
    tick(&mut runtime, &mut run, 2);
    assert!((run.players[&1].x - position - 0.25).abs() < 0.001);
    assert_eq!(runtime.started, 2);
    assert_eq!(
        runtime.groups.len(),
        1,
        "the original spark cast lost its timeline"
    );
    assert_eq!(
        runtime
            .groups
            .values()
            .next()
            .unwrap()
            .instance
            .generation(),
        1
    );
}

#[test]
#[ignore = "local runtime measurement; run explicitly with --ignored --nocapture"]
fn four_player_executable_ability_tick_cost() {
    let fixture = Fixture::new();
    let effects = fixture.root.join("ability-effects/effects.js");
    let mut graphs = std::fs::read_to_string(&effects).unwrap();
    // Continuous four-player emissions exercise the adapter, with one-tick
    // graph lives so the fixture does not intentionally fill its VFX budget.
    graphs.push_str("\nfor(const bank of Object.values(exports.banks)) for(const graph of Object.values(bank)) {graph.ticks=1; for(const node of graph.nodes) node.keys=node.keys.slice(0,1)}\n");
    std::fs::write(effects, graphs).unwrap();
    fixture.edit(
        &fixture
            .source()
            .replace("delay: 5", "delay: 600")
            .replace("damage: 28", "damage: 0")
            .replace("ttl: 90", "ttl: 3")
            .replace(
                "if (age === 0 || age >= cast.parameters.delay)",
                "if (true)",
            ),
    );
    let mut registry = fixture.registry();
    for player in 1..=4 {
        select(&mut registry, &format!("spark {player}"));
    }
    let mut runtime = Runtime::default();
    let mut run = fight();
    run.enemies.clear();
    run.join_seat(2, "Two");
    run.join_seat(3, "Three");
    run.join_seat(4, "Four");
    runtime.set_sources(sources(&registry), &mut run);
    let mut samples = Vec::new();
    for tick in 0..180 {
        runtime.before_step(&mut run);
        run.step(
            &(1..=4)
                .map(|id| {
                    (
                        id,
                        Input {
                            cast: if tick == 0 { 1 } else { 0 },
                            ..Default::default()
                        },
                    )
                })
                .collect(),
        );
        let started = std::time::Instant::now();
        runtime.after_step(&mut run);
        samples.push(started.elapsed().as_micros());
    }
    assert_eq!(runtime.started, 4);
    assert_eq!(runtime.effects, 4 * 180);
    assert_eq!(runtime.visuals.emitted, 4 * 180);
    assert_eq!(runtime.groups.len(), 1);
    samples.sort_unstable();
    eprintln!(
        "four-player executable ability batch: p50={}us p95={}us max={}us, {}",
        samples[90],
        samples[171],
        samples[179],
        runtime.status()
    );
}

#[test]
fn accelerated_rules_time_coalesces_admission_and_does_not_skip_delayed_release() {
    let fixture = Fixture::new();
    let mut registry = fixture.registry();
    select(&mut registry, "spark");
    let mut runtime = Runtime::default();
    let mut run = fight();
    run.mode = angel_delve::rules::fortune::Mode::Turbo;
    run.dungeon.depth = 1;
    run.tick = 2;
    runtime.set_sources(sources(&registry), &mut run);
    tick(&mut runtime, &mut run, 1);
    assert_eq!(run.tick, 4, "fixture did not exercise two rules advances");
    assert_eq!(runtime.started, 1);
    assert_eq!(runtime.effects, 1);
    while run.tick < 10 {
        tick(&mut runtime, &mut run, 0);
    }
    assert_eq!(runtime.effects, 2);
    assert!(runtime.groups.is_empty());
}

#[test]
fn sound_asset_swap_keeps_delayed_release_on_original_bytes_and_new_casts_use_new_bank() {
    let fixture = Fixture::new();
    let mut registry = fixture.registry();
    select(&mut registry, "spark");
    let mut runtime = Runtime::default();
    let mut run = fight();
    runtime.set_sources(sources(&registry), &mut run);
    tick(&mut runtime, &mut run, 1);
    let old = runtime.media.take().pop().unwrap();
    let old_bytes = old.sound.file.bytes().to_vec();
    let path = fixture.root.join("ability-sounds/spark.wav");
    let mut replacement = std::fs::read(&path).unwrap();
    replacement[80] ^= 0x40;
    std::fs::write(path, &replacement).unwrap();
    registry.refresh(true);
    assert_eq!(registry.snapshot().statuses[0].generation, 2);
    runtime.set_sources(sources(&registry), &mut run);
    for _ in 0..5 {
        tick(&mut runtime, &mut run, 0);
    }
    let delayed = runtime.media.take().pop().unwrap();
    assert_eq!(delayed.sound.file.name, old.sound.file.name);
    assert_eq!(delayed.sound.file.bytes(), old_bytes);
    select(&mut registry, "spark 1 2");
    runtime.set_sources(sources(&registry), &mut run);
    tick(&mut runtime, &mut run, 2);
    let next = runtime.media.take().pop().unwrap();
    assert_ne!(next.sound.file.name, old.sound.file.name);
    assert_eq!(next.sound.file.bytes(), replacement);
    assert_eq!(runtime.media.emitted, 3);
}

#[test]
fn malformed_sound_candidate_keeps_the_complete_working_bank_and_runtime_batch_cannot_emit_partial_audio()
 {
    let fixture = Fixture::new();
    let mut registry = fixture.registry();
    select(&mut registry, "spark");
    let path = fixture.root.join("ability-sounds/spark.wav");
    let good = std::fs::read(&path).unwrap();
    std::fs::write(&path, b"not a wave").unwrap();
    registry.refresh(true);
    assert_eq!(registry.snapshot().statuses[0].generation, 1);
    assert!(
        registry
            .snapshot()
            .statuses
            .iter()
            .find_map(|status| status.error.as_deref())
            .unwrap()
            .contains("PCM16")
    );
    std::fs::write(path, good).unwrap();
    fixture.edit(
        &fixture
            .source()
            .replace("cue: 'spark'", "cue: player.hp===7 ? 'absent' : 'spark'"),
    );
    registry.refresh(true);
    assert_eq!(registry.snapshot().statuses[0].generation, 2);
    let mut runtime = Runtime::default();
    let mut run = fight();
    run.players.get_mut(&1).unwrap().hp = 7;
    runtime.set_sources(sources(&registry), &mut run);
    tick(&mut runtime, &mut run, 1);
    assert_eq!(runtime.started, 0);
    assert_eq!(runtime.effects, 0);
    assert_eq!(runtime.media.emitted, 0);
    assert!(runtime.media.take().is_empty());
    assert_eq!(run.players[&1].mana, 100.0);
    assert!(runtime.groups.values().any(|group| {
        group
            .fault
            .as_deref()
            .is_some_and(|error| error.contains("sound cue"))
    }));
}

#[test]
fn player_sound_choices_change_future_casts_without_rebinding_in_flight_cues() {
    let fixture = Fixture::new();
    let mut registry = fixture.registry();
    select(&mut registry, "spark");
    let mut runtime = Runtime::default();
    let mut run = fight();
    runtime.set_sources(sources(&registry), &mut run);
    tick(&mut runtime, &mut run, 1);
    let first = runtime.media.take().pop().unwrap();
    let hp = run.players[&1].hp;
    let mana = run.players[&1].mana;
    let cooldown = run.players[&1].spell_cooldowns;
    select(&mut registry, "sound warm 1");
    runtime.set_sources(sources(&registry), &mut run);
    assert_eq!(
        (
            run.players[&1].hp,
            run.players[&1].mana,
            run.players[&1].spell_cooldowns
        ),
        (hp, mana, cooldown)
    );
    assert_eq!(registry.snapshot().statuses[0].generation, 1);
    for _ in 0..5 {
        tick(&mut runtime, &mut run, 0);
    }
    assert_eq!(runtime.media.take().pop().unwrap().name(), first.name());
    select(&mut registry, "spark 1 2");
    runtime.set_sources(sources(&registry), &mut run);
    tick(&mut runtime, &mut run, 2);
    let future = runtime.media.take().pop().unwrap();
    assert_ne!(future.sound.file.name, first.sound.file.name);
    assert_eq!(future.level, 30);
}

#[test]
fn an_invalid_unselected_sound_bank_rejects_the_whole_advertised_catalogue() {
    let fixture = Fixture::new();
    let mut registry = fixture.registry();
    select(&mut registry, "spark");
    let path = fixture.root.join("ability-sounds/sounds.js");
    std::fs::write(
        &path,
        std::fs::read_to_string(&path)
            .unwrap()
            .replace("level: 0.3 }", "level: 2 }"),
    )
    .unwrap();
    registry.refresh(true);
    assert_eq!(registry.snapshot().statuses[0].generation, 1);
    assert!(
        registry
            .snapshot()
            .statuses
            .iter()
            .find_map(|status| status.error.as_deref())
            .unwrap()
            .contains("sound cue/level")
    );
    select(&mut registry, "sound warm 1");
    let loadout = sources(&registry).pop().unwrap();
    assert_eq!(
        loadout.binding::<Loadout>().unwrap().media["spark"].level,
        30
    );
}

#[test]
fn effect_asset_swap_keeps_finished_cast_visuals_alive_until_their_own_expiry() {
    let fixture = Fixture::new();
    fixture.edit(&fixture.source().replace("delay: 5", "delay: 0"));
    let mut registry = fixture.registry();
    select(&mut registry, "spark");
    let mut runtime = Runtime::default();
    let mut run = fight();
    runtime.set_sources(sources(&registry), &mut run);
    tick(&mut runtime, &mut run, 1);
    assert!(runtime.groups.is_empty());
    assert_eq!(runtime.visuals.active(), 1);
    let first = run.module_visuals.effects[0].clone();
    let old_pixels = serde_json::to_value(&run.module_visuals.graphs[&first.graph]).unwrap();
    let old_asset = {
        let snapshot = registry.snapshot();
        Arc::downgrade(
            snapshot
                .assets
                .values()
                .find(|asset| asset.bytes.starts_with(b"\x89PNG"))
                .unwrap(),
        )
    };
    let image = image::RgbaImage::from_pixel(32, 32, image::Rgba([255, 40, 80, 255]));
    image
        .save(fixture.root.join("ability-effects/sigil.png"))
        .unwrap();
    registry.refresh(true);
    assert_eq!(registry.snapshot().statuses[0].generation, 2);
    let state = (
        run.players[&1].hp,
        run.players[&1].mana,
        run.players[&1].spell_cooldowns,
    );
    select(&mut registry, "spark 1 2");
    runtime.set_sources(sources(&registry), &mut run);
    assert_eq!(
        (
            run.players[&1].hp,
            run.players[&1].mana,
            run.players[&1].spell_cooldowns
        ),
        state
    );
    tick(&mut runtime, &mut run, 2);
    assert!(runtime.groups.is_empty());
    assert_eq!(runtime.visuals.active(), 2);
    assert_eq!(runtime.visuals.generations(), 2);
    let next = run.module_visuals.effects.last().unwrap().clone();
    assert_ne!(first.graph, next.graph);
    assert_eq!(next.origin.generation, 2);
    assert_eq!(
        serde_json::to_value(&run.module_visuals.graphs[&first.graph]).unwrap(),
        old_pixels
    );
    assert!(old_asset.upgrade().is_some());
    run.tick = first.born + 90;
    runtime.before_step(&mut run);
    assert_eq!(runtime.visuals.active(), 1);
    assert_eq!(runtime.visuals.generations(), 1);
    assert!(
        old_asset.upgrade().is_none(),
        "old effect asset survived its last owner"
    );
    run.tick = next.born + 90;
    runtime.before_step(&mut run);
    assert!(run.module_visuals.is_empty());
    assert_eq!(runtime.visuals.active(), 0);
}

#[test]
fn effect_bank_selection_preserves_old_casts_and_quiet_affects_only_future_visuals() {
    let fixture = Fixture::new();
    let mut registry = fixture.registry();
    select(&mut registry, "spark");
    let mut runtime = Runtime::default();
    let mut run = fight();
    runtime.set_sources(sources(&registry), &mut run);
    tick(&mut runtime, &mut run, 1);
    let first = run.module_visuals.effects[0].graph.clone();
    select(&mut registry, "effect ember 1");
    select(&mut registry, "spark 1 2");
    runtime.set_sources(sources(&registry), &mut run);
    for _ in 0..5 {
        tick(&mut runtime, &mut run, 0);
    }
    assert_eq!(run.module_visuals.effects.last().unwrap().graph, first);
    tick(&mut runtime, &mut run, 2);
    assert_ne!(run.module_visuals.effects.last().unwrap().graph, first);
    select(&mut registry, "effect quiet 1");
    select(&mut registry, "bloom 1 3");
    runtime.set_sources(sources(&registry), &mut run);
    let emitted = runtime.visuals.emitted;
    let physical = runtime.effects;
    tick(&mut runtime, &mut run, 3);
    assert_eq!(runtime.visuals.emitted, emitted);
    assert_eq!(runtime.effects, physical + 1);
    select(&mut registry, "cancel 1");
    runtime.set_sources(sources(&registry), &mut run);
    assert!(run.module_visuals.is_empty());
    assert_eq!(runtime.visuals.active(), 0);
}

#[test]
fn effect_pressure_defers_the_entire_cast_without_costs_sound_or_replay() {
    use angel_realm_host::together_visuals::Emission;
    let fixture = Fixture::new();
    let mut registry = fixture.registry();
    select(&mut registry, "spark");
    let resource = sources(&registry).pop().unwrap();
    let prepared = resource.binding::<Loadout>().unwrap().vfx["spark"].clone();
    let mut runtime = Runtime::default();
    let mut run = fight();
    runtime.set_sources(vec![resource.clone()], &mut run);
    runtime.before_step(&mut run);
    let emission = Emission {
        player: 1,
        origin: Origin {
            module: resource.module.clone(),
            generation: resource.generation,
            cast: "fixture".into(),
        },
        prepared,
        offset: [0.0, 0.0],
        follow: false,
        resource: Some(resource),
    };
    let staged = runtime
        .visuals
        .stage(
            &run,
            vec![emission; angel_delve::realm::effects::MAX_ACTIVE],
        )
        .unwrap()
        .unwrap();
    runtime.visuals.commit(staged, &mut run);
    tick(&mut runtime, &mut run, 1);
    assert_eq!(runtime.started, 0);
    assert_eq!(runtime.effects, 0);
    assert_eq!(runtime.media.emitted, 0);
    assert_eq!(run.players[&1].mana, 100.0);
    assert_eq!(runtime.pending.len(), 1);
    run.tick += 90;
    tick(&mut runtime, &mut run, 0);
    assert_eq!(runtime.started, 1);
    assert_eq!(runtime.effects, 1);
    assert_eq!(runtime.media.emitted, 1);
    assert_eq!(runtime.visuals.emitted, 65);
    let mana = run.players[&1].mana;
    runtime.after_step(&mut run);
    assert_eq!(runtime.started, 1);
    assert_eq!(runtime.effects, 1);
    assert_eq!(run.players[&1].mana, mana);
}

#[test]
fn invalid_effect_candidates_and_runtime_cues_cannot_partially_commit() {
    let fixture = Fixture::new();
    let mut registry = fixture.registry();
    select(&mut registry, "spark");
    let path = fixture.root.join("ability-effects/effects.js");
    let good = std::fs::read_to_string(&path).unwrap();
    std::fs::write(
        &path,
        good.replace("ember: {", "ember: { broken: { ticks: 0, nodes: [] },"),
    )
    .unwrap();
    registry.refresh(true);
    assert_eq!(registry.snapshot().statuses[0].generation, 1);
    assert!(
        registry
            .snapshot()
            .statuses
            .iter()
            .any(|status| status.error.is_some())
    );
    std::fs::write(path, good).unwrap();
    fixture.edit(&fixture.source().replace(
        "type: 'visual', cue: 'spark'",
        "type: 'visual', cue: player.hp===7 ? 'absent' : 'spark'",
    ));
    registry.refresh(true);
    assert_eq!(registry.snapshot().statuses[0].generation, 2);
    let mut runtime = Runtime::default();
    let mut run = fight();
    run.players.get_mut(&1).unwrap().hp = 7;
    runtime.set_sources(sources(&registry), &mut run);
    tick(&mut runtime, &mut run, 1);
    assert_eq!(runtime.started, 0);
    assert_eq!(runtime.effects, 0);
    assert_eq!(runtime.media.emitted, 0);
    assert_eq!(runtime.visuals.emitted, 0);
    assert_eq!(run.players[&1].mana, 100.0);
    assert!(run.module_visuals.is_empty());
    assert!(runtime.groups.values().any(|group| {
        group
            .fault
            .as_deref()
            .is_some_and(|error| error.contains("effect cue"))
    }));
}

#[test]
#[ignore = "explicit actual-room visual review artifact"]
fn render_authored_effect_review_frame() {
    let fixture = Fixture::new();
    let mut registry = fixture.registry();
    let fairies = std::env::var_os("ANGEL_EFFECT_REVIEW_FAIRIES").is_some();
    select(&mut registry, if fairies { "fairies" } else { "spark" });
    if !fairies {
        select(&mut registry, "bloom 1 2");
    }
    let mut runtime = Runtime::default();
    let mut run = if fairies { field_fight() } else { fight() };
    runtime.set_sources(sources(&registry), &mut run);
    tick(&mut runtime, &mut run, 1);
    if !fairies {
        tick(&mut runtime, &mut run, 2);
    }
    for _ in 0..12 {
        tick(&mut runtime, &mut run, 0);
    }
    let frame = angel_delve::render::overworld::arena::frame_for(&run, 768, 448, Some(1));
    image::save_buffer(
        if fairies {
            "/tmp/angelx-fairy-field-review.png"
        } else {
            "/tmp/angelx-vfx-review.png"
        },
        &frame.rgba_bytes(),
        frame.w as u32,
        frame.h as u32,
        image::ColorType::Rgba8,
    )
    .unwrap();
    eprintln!(
        "{} · {}",
        if fairies {
            "/tmp/angelx-fairy-field-review.png"
        } else {
            "/tmp/angelx-vfx-review.png"
        },
        runtime.status()
    );
}

#[test]
fn independent_media_and_art_updates_rebind_loadouts_without_restarting_the_executable_instance() {
    let fixture = Fixture::new();
    let mut registry = fixture.registry();
    select(&mut registry, "spark");
    registry
        .dispatch(
            angel_live::kernel::Kind::Command,
            "character",
            &json!({"type":"command","name":"character","arguments":"gold 1"}),
        )
        .unwrap();
    let mut runtime = Runtime::default();
    let mut run = fight();
    runtime.set_sources(sources(&registry), &mut run);
    tick(&mut runtime, &mut run, 1);
    let old_voice = runtime.media.take().pop().unwrap();
    let old_resource = sources(&registry).pop().unwrap();
    let old_code = old_resource.binding::<Loadout>().unwrap().executables["slot1"]
        .resource
        .clone();
    let owned = (
        run.players[&1].hp,
        run.players[&1].mana,
        run.players[&1].x,
        run.players[&1].y,
        run.players[&1].spell_cooldowns,
    );
    let wave = fixture.root.join("ability-sounds/spark.wav");
    let mut bytes = std::fs::read(&wave).unwrap();
    bytes[80] ^= 0x40;
    std::fs::write(wave, bytes).unwrap();
    let art = fixture.root.join("characters/looks.js");
    std::fs::write(
        &art,
        std::fs::read_to_string(&art)
            .unwrap()
            .replace("225, 165, 50", "10, 200, 90"),
    )
    .unwrap();
    registry.refresh(true);
    let next = sources(&registry).pop().unwrap();
    assert_eq!(next.generation, 2);
    let loadout = next.binding::<Loadout>().unwrap();
    assert_eq!(loadout.executables["slot1"].resource, old_code);
    assert_ne!(loadout.media["spark"].name(), old_voice.name());
    assert_eq!(
        loadout
            .appearance
            .as_ref()
            .unwrap()
            .avatar
            .as_ref()
            .unwrap()
            .colour,
        [10, 200, 90]
    );
    runtime.set_sources(sources(&registry), &mut run);
    assert_eq!(
        (
            run.players[&1].hp,
            run.players[&1].mana,
            run.players[&1].x,
            run.players[&1].y,
            run.players[&1].spell_cooldowns
        ),
        owned
    );
    assert_eq!(
        run.players[&1].avatar.as_ref().unwrap().colour,
        [10, 200, 90]
    );
    assert_eq!(
        runtime
            .groups
            .values()
            .next()
            .unwrap()
            .instance
            .generation(),
        1
    );
    select(&mut registry, "spark 1 2");
    runtime.set_sources(sources(&registry), &mut run);
    tick(&mut runtime, &mut run, 2);
    assert_eq!(
        runtime.groups.len(),
        1,
        "a media-only swap restarted the code instance"
    );
    assert_eq!(runtime.started, 2);
    let new_voice = runtime.media.take().pop().unwrap();
    assert_ne!(new_voice.name(), old_voice.name());
    for _ in 0..5 {
        tick(&mut runtime, &mut run, 0);
    }
    let voices = runtime.media.take();
    assert!(
        voices.iter().any(|voice| voice.name() == old_voice.name()),
        "old timeline lost its selected cue"
    );
    assert!(
        voices.iter().any(|voice| voice.name() == new_voice.name()),
        "new timeline lost its independently selected cue"
    );
    let before = registry.snapshot().resources;
    let sounds = fixture.root.join("ability-sounds/sounds.js");
    std::fs::write(
        &sounds,
        std::fs::read_to_string(&sounds)
            .unwrap()
            .replace("crystal: exports.sounds,", ""),
    )
    .unwrap();
    registry.refresh(true);
    assert_eq!(
        registry.snapshot().resources,
        before,
        "missing selected bank partially replaced the closure"
    );
    assert!(registry.snapshot().statuses.iter().any(|status| {
        status
            .error
            .as_deref()
            .is_some_and(|error| error.contains("selected sound bank is missing"))
    }));
}

#[test]
fn two_players_resolve_distinct_executable_providers_without_a_native_branch() {
    let fixture = Fixture::new();
    let other = fixture.root.join("other-code");
    std::fs::create_dir_all(&other).unwrap();
    let mut manifest: Value = serde_json::from_slice(
        &std::fs::read(fixture.root.join("ability-code/module.json")).unwrap(),
    )
    .unwrap();
    manifest["id"] = json!("other-code");
    manifest["assets"] = json!([]);
    std::fs::write(other.join("module.json"), manifest.to_string()).unwrap();
    std::fs::copy(
        fixture.root.join("ability-code/index.js"),
        other.join("index.js"),
    )
    .unwrap();
    std::fs::write(
        other.join("spells.js"),
        fixture.source().replace("damage: 28", "damage: 91"),
    )
    .unwrap();
    for name in [
        "fairies.bend",
        "LAWS.bend",
        "PROOF.bend",
        "fairies.compiled.js",
        "bend-build.json",
    ] {
        std::fs::copy(
            fixture.root.join("ability-code").join(name),
            other.join(name),
        )
        .unwrap();
    }
    let path = fixture.root.join("abilities/module.json");
    let mut manifest: Value = serde_json::from_slice(&std::fs::read(&path).unwrap()).unwrap();
    manifest["imports"]
        .as_array_mut()
        .unwrap()
        .push(json!({"module":"other-code","contract":"ability-pack/1"}));
    std::fs::write(path, manifest.to_string()).unwrap();
    let path = fixture.root.join("abilities/index.js");
    std::fs::write(
        &path,
        std::fs::read_to_string(&path).unwrap().replace(
            "reference('ability-code', id)",
            "reference(player === '2' ? 'other-code' : 'ability-code', id)",
        ),
    )
    .unwrap();
    let mut registry = fixture.registry();
    select(&mut registry, "spark 1");
    select(&mut registry, "spark 2");
    let mut run = fight();
    run.join_seat(2, "Two");
    run.enemies.clear();
    let mut runtime = Runtime::default();
    runtime.set_sources(sources(&registry), &mut run);
    runtime.before_step(&mut run);
    run.step(
        &(1..=2)
            .map(|player| {
                (
                    player,
                    Input {
                        cast: 1,
                        ..Default::default()
                    },
                )
            })
            .collect(),
    );
    runtime.after_step(&mut run);
    assert_eq!(runtime.started, 2, "{}", runtime.status());
    assert_eq!(runtime.groups.len(), 2);
    assert!(
        run.projectiles
            .iter()
            .any(|projectile| projectile.damage == 28)
    );
    assert!(
        run.projectiles
            .iter()
            .any(|projectile| projectile.damage == 91)
    );
    assert_eq!(
        runtime.sources[&1]
            .binding::<Loadout>()
            .unwrap()
            .executables["slot1"]
            .resource
            .module,
        "ability-code"
    );
    assert_eq!(
        runtime.sources[&2]
            .binding::<Loadout>()
            .unwrap()
            .executables["slot1"]
            .resource
            .module,
        "other-code"
    );
}

#[test]
fn splitting_legacy_character_and_ability_modules_migrates_both_player_selections_together() {
    let fixture = Fixture::new();
    let mut originals = Vec::new();
    for module in ["abilities", "characters"] {
        for file in ["module.json", "index.js"] {
            let path = fixture.root.join(module).join(file);
            originals.push((path.clone(), std::fs::read(&path).unwrap()));
        }
        std::fs::write(fixture.root.join(module).join("module.json"), json!({
            "api_version":2,"id":module,"kind":"system","entry":"index.js","state_schema":1,
            "exports":[{"kind":"command","name":if module=="abilities" {"ability"}else{"character"},"contract":"command/1"}]
        }).to_string()).unwrap();
    }
    std::fs::write(fixture.root.join("abilities/index.js"), "module.exports={apiVersion:2,initialize:()=>({choices:{},cancelled:{},soundBanks:{},effectBanks:{}}),reduce:state=>({state:{...state,choices:{1:['spark',null,null,null]}}})}").unwrap();
    std::fs::write(fixture.root.join("characters/index.js"), "module.exports={apiVersion:2,initialize:()=>({choices:{}}),reduce:state=>({state:{choices:{1:'gold'}}})}").unwrap();
    let mut registry = fixture.registry();
    select(&mut registry, "spark");
    registry
        .dispatch(
            angel_live::kernel::Kind::Command,
            "character",
            &json!({"type":"command","name":"character","arguments":"gold 1"}),
        )
        .unwrap();
    for (path, bytes) in originals {
        std::fs::write(path, bytes).unwrap();
    }
    registry.refresh(true);
    let snapshot = registry.snapshot();
    for status in &snapshot.statuses {
        assert!(status.error.is_none(), "{status:?}");
    }
    let source = &snapshot.resources["abilities/character-seat-1"];
    assert_eq!(source.generation, 2);
    let loadout = source.binding::<Loadout>().unwrap();
    assert_eq!(loadout.executables["slot1"].ability, "spark");
    assert_eq!(
        loadout
            .appearance
            .as_ref()
            .unwrap()
            .avatar
            .as_ref()
            .unwrap()
            .colour,
        [225, 165, 50]
    );
    assert_eq!(
        registry.snapshot().surfaces["abilities"]
            .lines
            .iter()
            .filter(|line| line.contains("Player 1: spark"))
            .count(),
        1
    );
}

fn field_fight() -> Run {
    let mut run = fight();
    run.enemies.clear();
    let room = &mut run.dungeon.rooms[run.at];
    for y in 1..room.rows - 1 {
        for x in 1..room.cols - 1 {
            room.set(x, y, angel_delve::rules::Tile::Floor);
        }
    }
    let hero = run.players.get_mut(&1).unwrap();
    hero.x = 8.0;
    hero.y = 8.0;
    run.spawn_at_for_test(angel_delve::rules::EnemyKind::Skeleton, 10.0, 8.0);
    run.spawn_at_for_test(angel_delve::rules::EnemyKind::Skeleton, 14.0, 8.0);
    run
}
#[test]
#[ignore = "requires the pinned optional Bend developer compiler; run explicitly"]
fn fairy_field_stuns_real_enemies_and_hotswaps_radius_while_old_fields_keep_their_state() {
    let fixture = Fixture::new();
    let mut registry = fixture.registry();
    select(&mut registry, "fairies");
    let mut run = field_fight();
    run.join_seat(2, "Bend Guest");
    run.players.get_mut(&2).unwrap().spell_cooldowns[0] = 71;
    let mut runtime = Runtime::default();
    runtime.set_sources(sources(&registry), &mut run);
    tick(&mut runtime, &mut run, 1);
    assert_eq!(runtime.started, 1, "{}", runtime.status());
    assert_eq!(runtime.media.emitted, 1);
    assert_eq!(run.enemies[0].frozen, 12);
    assert_eq!(run.enemies[1].frozen, 0);
    assert_eq!(runtime.visuals.active(), 1);
    assert_eq!(run.players[&1].mana, 82.0);
    let position = (
        run.enemies[0].x,
        run.enemies[0].y,
        run.enemies[0].age,
        run.enemies[0].timer,
    );
    for _ in 0..4 {
        tick(&mut runtime, &mut run, 0);
    }
    assert_eq!(
        (
            run.enemies[0].x,
            run.enemies[0].y,
            run.enemies[0].age,
            run.enemies[0].timer
        ),
        position
    );
    let bend_path = fixture.root.join("ability-code/fairies.bend");
    let original = std::fs::read_to_string(&bend_path).unwrap();
    let resources = registry.snapshot().resources;
    let preserved = (
        run.players.len(),
        run.players[&1].mana,
        run.players[&1].spell_cooldowns,
        run.players[&2].spell_cooldowns,
    );
    std::fs::write(
        &bend_path,
        original.replace(
            "case False{} _:\n      End{}",
            "case False{} _:\n      Keep{Planted{0.0, 0.0, \"broken\", 0}, False{}, None{}}",
        ),
    )
    .unwrap();
    registry.refresh(true);
    assert_eq!(registry.snapshot().resources, resources);
    assert!(registry.snapshot().statuses.iter().any(|s| {
        s.error
            .as_deref()
            .is_some_and(|e| e.contains("SOME PROOFS FAIL"))
    }));
    let before_swap = std::time::Instant::now();
    std::fs::write(
        &bend_path,
        original.replace("Params{3.5, 150, 12, 12}", "Params{7.0, 210, 24, 12}"),
    )
    .unwrap();
    registry.refresh(true);
    eprintln!(
        "Bend capture/check/compile/activation {:?}",
        before_swap.elapsed()
    );
    select(&mut registry, "fairies 1 2");
    runtime.set_sources(sources(&registry), &mut run);
    assert_eq!(
        (
            run.players.len(),
            run.players[&1].mana,
            run.players[&1].spell_cooldowns,
            run.players[&2].spell_cooldowns
        ),
        preserved
    );
    tick(&mut runtime, &mut run, 2);
    assert!(
        run.enemies[1].frozen > 0,
        "the new field did not reach its independently authored radius"
    );
    assert_eq!(runtime.groups.len(), 2);
    assert_eq!(runtime.visuals.active(), 2);
    let radius = |generation| {
        runtime.groups[&("ability-code".into(), generation)]
            .casts
            .values()
            .next()
            .unwrap()
            .definition
            .parameters["radius"]
            .as_f64()
            .unwrap()
    };
    assert_eq!(radius(1), 3.5);
    assert_eq!(radius(2), 7.0);
    let hold = |generation| {
        runtime.groups[&("ability-code".into(), generation)]
            .casts
            .values()
            .next()
            .unwrap()
            .definition
            .parameters["hold"]
            .as_u64()
            .unwrap()
    };
    assert_eq!(hold(1), 12);
    assert_eq!(hold(2), 24);
    let mut graphs = run
        .module_visuals
        .graphs
        .values()
        .map(|graph| {
            let radius = graph
                .nodes
                .iter()
                .find_map(|node| match node.shape {
                    angel_delve::realm::effects::Shape::Ring { radius, .. } => Some(radius),
                    _ => None,
                })
                .unwrap();
            (radius, graph.ticks)
        })
        .collect::<Vec<_>>();
    graphs.sort_by(|a, b| a.0.total_cmp(&b.0));
    assert_eq!(
        graphs,
        vec![(3.5, 150), (7.0, 210)],
        "visual pack did not atomically derive the same radius/lifetime as its code provider"
    );
    assert_eq!(runtime.started, 2);
    assert_eq!(runtime.media.emitted, 2);
    let room = run
        .dungeon
        .rooms
        .iter()
        .enumerate()
        .find(|(at, _)| *at != run.at)
        .unwrap()
        .0;
    run.enter_for_test(room);
    tick(&mut runtime, &mut run, 0);
    assert!(runtime.groups.is_empty(), "field crossed its captured room");
    assert_eq!(runtime.visuals.active(), 0);
}

#[test]
fn fairy_field_stays_at_its_cast_position_and_cancel_stops_future_pulses() {
    let fixture = Fixture::new();
    let mut registry = fixture.registry();
    select(&mut registry, "fairies");
    let mut run = field_fight();
    let mut runtime = Runtime::default();
    runtime.set_sources(sources(&registry), &mut run);
    tick(&mut runtime, &mut run, 1);
    run.players.get_mut(&1).unwrap().x = 17.0;
    for _ in 0..12 {
        tick(&mut runtime, &mut run, 0);
    }
    assert!(
        run.enemies[0].frozen > 0,
        "field followed the caster instead of keeping its anchor"
    );
    assert_eq!(run.module_visuals.effects[0].position, [8.0, 8.0]);
    assert!(!run.module_visuals.effects[0].follow);
    select(&mut registry, "cancel 1");
    runtime.set_sources(sources(&registry), &mut run);
    assert!(runtime.groups.is_empty());
    assert_eq!(runtime.visuals.active(), 0);
    let effects = runtime.effects;
    for _ in 0..13 {
        tick(&mut runtime, &mut run, 0);
    }
    assert_eq!(runtime.effects, effects);
    assert_eq!(runtime.started, 1);
    assert_eq!(runtime.media.emitted, 1);
    assert_eq!(run.enemies[0].frozen, 0);
}
#[test]
fn invalid_fairy_art_or_status_contract_keeps_the_whole_working_loadout() {
    let fixture = Fixture::new();
    let mut registry = fixture.registry();
    select(&mut registry, "fairies");
    let before = registry.snapshot().resources;
    let art = fixture.root.join("ability-effects/fairies.js");
    let good = std::fs::read_to_string(&art).unwrap();
    std::fs::write(&art, good.replace("'..WBB...'", "'..ZBB...'")).unwrap();
    registry.refresh(true);
    assert_eq!(registry.snapshot().resources, before);
    assert!(registry.snapshot().statuses.iter().any(|status| {
        status
            .error
            .as_deref()
            .is_some_and(|error| error.contains("indexed sprite"))
    }));
    std::fs::write(art, good).unwrap();
    fixture.edit(&fixture.source().replace(
        "parameters: fairyParameters",
        "parameters: { ...fairyParameters, hold: 0 }",
    ));
    registry.refresh(true);
    assert_eq!(
        registry.snapshot().resources,
        before,
        "invalid field partially replaced code/media"
    );
}

#[test]
#[ignore = "local four-seat Bend/runtime measurement; run explicitly without compilation overlap"]
fn bend_four_seat_field_runtime_cost() {
    let fixture = Fixture::new();
    let mut registry = fixture.registry();
    let mut run = field_fight();
    for player in 2..=4 {
        run.join_seat(player, &format!("Bend {player}"));
    }
    for player in 1..=4 {
        select(&mut registry, &format!("fairies {player}"));
    }
    let mut runtime = Runtime::default();
    runtime.set_sources(sources(&registry), &mut run);
    runtime.before_step(&mut run);
    run.step(
        &(1..=4)
            .map(|id| {
                (
                    id,
                    Input {
                        cast: 1,
                        ..Default::default()
                    },
                )
            })
            .collect(),
    );
    runtime.after_step(&mut run);
    assert_eq!(runtime.started, 4, "{}", runtime.status());
    let mut samples = Vec::new();
    for _ in 0..120 {
        let before = std::time::Instant::now();
        tick(&mut runtime, &mut run, 0);
        samples.push(before.elapsed().as_micros());
    }
    assert_eq!(runtime.groups.len(), 1);
    assert_eq!(runtime.groups.values().next().unwrap().casts.len(), 4);
    assert!(runtime.groups.values().all(|g| g.fault.is_none()));
    samples.sort_unstable();
    eprintln!(
        "four-seat Bend fields: 120 complete rules/ability ticks p50={}us p95={}us max={}us; {} area actions, 4 captured fields; excludes rendering/compilation",
        samples[60], samples[114], samples[119], runtime.effects
    );
    for _ in 0..40 {
        tick(&mut runtime, &mut run, 0);
    }
    assert!(runtime.groups.is_empty());
    assert_eq!(runtime.visuals.active(), 0);
}
