use angel_delve::rules::abilities::{Command, Origin};
use angel_delve::rules::{Input, Run, RoomKind};
use angel_live::kernel::Resource;
use angel_realm_host::together_abilities::*;
use serde_json::{Value, json};
use std::collections::BTreeMap;
use std::sync::Arc;
use angel_live::Registry;
use std::path::PathBuf;

struct Fixture {
    root: PathBuf,
    _retirement: crate::delve::together_media::RetirementGuard,
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
            "character-motion",
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
            _retirement: crate::delve::together_media::RetirementGuard,
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
fn animated_loadout_reaches_the_live_sim_and_rebinds_after_edit() {
    let fixture = Fixture::new();
    let mut registry = fixture.registry();
    for arguments in ["gold", "animation breathe"] {
        registry.dispatch(
            angel_live::kernel::Kind::Command,
            "character",
            &json!({"type":"command","name":"character","arguments":arguments}),
        ).unwrap();
    }
    select(&mut registry, "spark");
    let mut sim = angel_realm_host::together_sim::Sim::default();
    sim.prefer_thread(false);
    let mut run = fight();
    run.enemies.clear();
    sim.set(Some(run));
    let snapshot = registry.snapshot();
    sim.set_live_characters(snapshot.resources.values());
    let hero = &sim.as_ref().unwrap().players[&1];
    assert_eq!(hero.avatar.as_ref().unwrap().animation.as_ref().unwrap().period(), 16);
    assert!(hero.module_spells[0].is_some(), "the live Sim discarded the executable loadout");

    let path = fixture.root.join("character-motion/index.js");
    let source = std::fs::read_to_string(&path).unwrap();
    std::fs::write(&path, source.replace(
        "breathe: [{ frame: 0, ticks: 8 }, { frame: 1, ticks: 8 }]",
        "breathe: [{ frame: 0, ticks: 24 }, { frame: 1, ticks: 8 }]",
    )).unwrap();
    registry.refresh(true);
    assert!(registry.snapshot().statuses.iter().all(|status| status.error.is_none()));
    let snapshot = registry.snapshot();
    sim.set_live_characters(snapshot.resources.values());
    let hero = &sim.as_ref().unwrap().players[&1];
    assert_eq!(hero.avatar.as_ref().unwrap().animation.as_ref().unwrap().period(), 32);
    assert!(hero.module_spells[0].is_some(), "animation reload removed the executable loadout");

    sim.set_controls(angel_realm_host::together_sim::Controls {
        active: true,
        keyboard: true,
        ..Default::default()
    });
    let pump = |sim: &mut angel_realm_host::together_sim::Sim| {
        std::thread::sleep(std::time::Duration::from_millis(35));
        let now = std::time::Instant::now();
        if let Some(seat) = sim.seat(1) {
            seat.tick(now);
        }
        sim.pump(now);
        sim.latch();
    };
    // A local projection is available before Link finishes connecting. Wait
    // until the seat is playing and the authoritative hero has returned.
    let ready_deadline = std::time::Instant::now() + std::time::Duration::from_secs(10);
    loop {
        pump(&mut sim);
        let ready = sim.seat(1).is_some_and(|seat| {
            seat.status() == angel_serve::client::LinkStatus::Playing
                && seat.knight() == Some(1) && seat.run_arc().is_some()
        }) && sim.as_ref().is_some_and(|run| {
            run.tick > 0 && run.active() && run.players.get(&1).is_some_and(|h| h.hp > 0 && !h.away)
        });
        if ready { break; }
        assert!(std::time::Instant::now() < ready_deadline, "live seat did not enter play");
    }
    sim.seat(1).unwrap().tap(angel_realm_host::together_seat::bits(&Input {
        cast: 1,
        ..Default::default()
    }));
    let cast_deadline = std::time::Instant::now() + std::time::Duration::from_secs(10);
    while std::time::Instant::now() < cast_deadline {
        pump(&mut sim);
        if sim.system_status().contains("started=1") {
            break;
        }
    }
    assert!(
        sim.system_status().contains("started=1"),
        "the installed live-Sim ability did not execute at tick {}: {}; player={:?}; seat={:?}; news={:?}",
        sim.as_ref().unwrap().tick, sim.system_status(),
        sim.as_ref().unwrap().players.get(&1).map(|h| (h.hp,h.mana,h.idle,h.away,h.stone,h.module_spells[0].is_some(),h.spell_cooldowns[0])),
        sim.seat(1).unwrap().status(), sim.drain_news()
    );
}

#[test]
fn independent_animation_provider_replaces_live_pixels_and_preserves_phase_and_seat() {
    let fixture = Fixture::new();
    let mut registry = fixture.registry();
    registry
        .dispatch(
            angel_live::kernel::Kind::Command,
            "character",
            &json!({"type":"command","name":"character","arguments":"gold 2"}),
        )
        .unwrap();
    registry
        .dispatch(
            angel_live::kernel::Kind::Command,
            "character",
            &json!({"type":"command","name":"character","arguments":"animation breathe 2"}),
        )
        .unwrap();
    select(&mut registry, "spark 2 1");
    let mut run = fight();
    run.join_seat(2, "Animated");
    run.tick = 27;
    let mut runtime = Runtime::default();
    runtime.set_sources(sources(&registry), &mut run);
    let art = registry.snapshot().resources["characters/pack"].clone();
    let code = registry.snapshot().resources["ability-code/pack"].clone();
    let before = run.players[&2].clone();
    let epoch = run.seat_epoch(2);
    let avatar = before.avatar.as_ref().unwrap();
    let phase = avatar.animation.as_ref().unwrap().phase(run.tick);
    let period = avatar.animation.as_ref().unwrap().period();
    let old_id = avatar.source_id.clone();
    let old_pixels = avatar.pixels_at(run.tick).to_vec();
    let (_, _, old_picture) = crate::delve::together_seat::guest_view(&run, 2);
    let path = fixture.root.join("character-motion/index.js");
    let bytes = std::fs::read_to_string(&path).unwrap();
    std::fs::write(
        &path,
        bytes.replace(
            "breathe: [{ frame: 0, ticks: 8 }, { frame: 1, ticks: 8 }]",
            "breathe: [{ frame: 0, ticks: 24 }, { frame: 1, ticks: 8 }]",
        ),
    )
    .unwrap();
    let activation = std::time::Instant::now();
    registry.refresh(true);
    assert!(
        registry
            .snapshot()
            .statuses
            .iter()
            .all(|status| status.error.is_none()),
        "{:?}",
        registry.snapshot().statuses
    );
    runtime.set_sources(sources(&registry), &mut run);
    let elapsed = activation.elapsed();
    assert_eq!(
        registry.snapshot().resources["characters/pack"],
        art,
        "art provider changed during animation-only replacement"
    );
    assert_eq!(registry.snapshot().resources["ability-code/pack"], code);
    let hero = &run.players[&2];
    assert_eq!(
        (hero.hp, hero.mana, hero.x, hero.y, hero.spell_cooldowns),
        (
            before.hp,
            before.mana,
            before.x,
            before.y,
            before.spell_cooldowns
        )
    );
    assert_eq!(run.seat_epoch(2), epoch);
    let avatar = hero.avatar.as_ref().unwrap();
    let animation = avatar.animation.as_ref().unwrap();
    assert_ne!(
        avatar.source_id, old_id,
        "animation-only replacement must invalidate appearance matching"
    );
    assert_eq!(avatar.pixels, before.avatar.as_ref().unwrap().pixels);
    assert_eq!(
        animation.phase(run.tick),
        phase * animation.period() / period
    );
    assert_ne!(avatar.pixels_at(run.tick), old_pixels);
    #[derive(serde::Deserialize)]
    #[serde(deny_unknown_fields)]
    struct LegacyAnimation {
        frames: Vec<Vec<[u8; 4]>>,
        frame_ticks: u32,
        phase_offset: u32,
    }
    let legacy: LegacyAnimation =
        serde_json::from_value(serde_json::to_value(animation).unwrap()).unwrap();
    assert!(legacy.frames.len() <= 8);
    let legacy_period = legacy.frame_ticks * legacy.frames.len() as u32;
    for tick in 0..u64::from(legacy_period * 2) {
        let phase = ((tick % u64::from(legacy_period) + u64::from(legacy.phase_offset))
            % u64::from(legacy_period)) as u32;
        assert_eq!(
            legacy.frames[(phase / legacy.frame_ticks) as usize],
            animation.pixels(tick).unwrap()
        );
    }
    let (wire, guest, picture) = crate::delve::together_seat::guest_view(&run, 2);
    assert!(!wire.is_empty());
    assert_eq!(
        guest.players[&2]
            .avatar
            .as_ref()
            .unwrap()
            .pixels_at(run.tick),
        avatar.pixels_at(run.tick)
    );
    let edit_to_guest_pixels = activation.elapsed();
    assert_ne!(
        picture, old_picture,
        "the production remote/terminal pixel consumer did not replace its animation"
    );
    let restored = angel_delve::rules::runtime_state::restore(
        angel_delve::rules::runtime_state::capture(&run).unwrap(),
    )
    .unwrap();
    assert_eq!(
        restored.players[&2]
            .avatar
            .as_ref()
            .unwrap()
            .animation
            .as_ref()
            .unwrap()
            .phase(run.tick),
        animation.phase(run.tick)
    );
    let offset = animation.phase_offset;
    runtime.set_sources(sources(&registry), &mut run);
    assert_eq!(
        run.players[&2]
            .avatar
            .as_ref()
            .unwrap()
            .animation
            .as_ref()
            .unwrap()
            .phase_offset,
        offset
    );
    eprintln!(
        "animation-only edit -> validated activation/install: {elapsed:?}; edit -> published guest pixels: {edit_to_guest_pixels:?}; phase {phase}/{period} -> {}/{}; wire {} bytes",
        run.players[&2]
            .avatar
            .as_ref()
            .unwrap()
            .animation
            .as_ref()
            .unwrap()
            .phase(run.tick),
        run.players[&2]
            .avatar
            .as_ref()
            .unwrap()
            .animation
            .as_ref()
            .unwrap()
            .period(),
        wire.len()
    );
    let accepted = sources(&registry);
    std::fs::write(&path, bytes.replace("ticks: 8", "ticks: 0")).unwrap();
    registry.refresh(true);
    assert!(
        registry
            .snapshot()
            .statuses
            .iter()
            .any(|status| status.error.is_some())
    );
    assert_eq!(
        sources(&registry),
        accepted,
        "invalid clip partially replaced a resolved loadout"
    );
    std::fs::write(
        &path,
        bytes.replace(
            "breathe: [{ frame: 0, ticks: 8 }, { frame: 1, ticks: 8 }]",
            "breathe: [{ frame: 0, ticks: 7 }, { frame: 1, ticks: 11 }]",
        ),
    )
    .unwrap();
    registry.refresh(true);
    assert!(registry.snapshot().statuses.iter().any(|status| {
        status
            .error
            .as_deref()
            .is_some_and(|error| error.contains("eight legacy playback frames"))
    }));
    assert_eq!(
        sources(&registry),
        accepted,
        "oversized exact lowering replaced last good generation"
    );
    std::fs::write(
        &path,
        bytes.replace(
            "breathe: [{ frame: 0, ticks: 8 }, { frame: 1, ticks: 8 }]",
            "breathe: [{ frame: 0, ticks: 24 }, { frame: 1, ticks: 8 }]",
        ),
    )
    .unwrap();
    registry.refresh(true);
    let selector = fixture.root.join("abilities/index.js");
    let selector_bytes = std::fs::read_to_string(&selector).unwrap();
    std::fs::write(&selector, selector_bytes.replace("appearance: state.artChoices[player] ? reference('characters', state.artChoices[player]) : null", "appearance: reference('characters', 'off')")).unwrap();
    registry.refresh(true);
    assert!(registry.snapshot().statuses.iter().any(|status| {
        status
            .error
            .as_deref()
            .is_some_and(|error| error.contains("animation requires enabled character art"))
    }));
    assert_eq!(
        sources(&registry),
        accepted,
        "disabled art plus animation replaced the working publication"
    );
    std::fs::write(&selector, &selector_bytes).unwrap();
    registry.refresh(true);
    let active = sources(&registry);
    let manifest = fixture.root.join("character-motion/module.json");
    let manifest_bytes = std::fs::read(&manifest).unwrap();
    std::fs::remove_file(&manifest).unwrap();
    registry.refresh(true);
    assert_eq!(
        sources(&registry),
        active,
        "removing required animation pack revoked the active loadout"
    );
    runtime.set_sources(sources(&registry), &mut run);
    assert_eq!(
        run.players[&2].avatar.as_ref().unwrap().source_id,
        active
            .iter()
            .find_map(|resource| resource
                .binding::<Loadout>()
                .filter(|loadout| loadout.player == 2)
                .and_then(|loadout| loadout.appearance.as_ref())
                .map(|appearance| appearance.fingerprint.clone()))
            .unwrap()
    );
    let mut cold = fixture.registry();
    select(&mut cold, "spark");
    assert_eq!(
        sources(&cold).len(),
        1,
        "absent optional animation pack must not block unrelated abilities"
    );
    std::fs::write(&manifest, manifest_bytes).unwrap();
    registry.refresh(true);
    cold.refresh(true);
    assert_eq!(sources(&registry).len(), active.len());
    assert!(
        registry
            .snapshot()
            .statuses
            .iter()
            .all(|status| status.error.is_none())
    );
    // A re-added provider permits a fresh cold selector, without creating seats.
    assert!(
        cold.snapshot()
            .statuses
            .iter()
            .all(|status| status.error.is_none())
    );
}

#[test]
fn optional_animation_pack_cold_start_and_selected_unavailability_fail_closed() {
    let fixture = Fixture::new();
    let motion_manifest = fixture.root.join("character-motion/module.json");
    let manifest_bytes = std::fs::read(&motion_manifest).unwrap();
    let read_state = |registry: &mut Registry| -> Value {
        let response = registry
            .dispatch(
                angel_live::kernel::Kind::Command,
                "loadout",
                &json!({"type":"command","name":"loadout","arguments":""}),
            )
            .unwrap();
        let output = response["output"].as_str().unwrap();
        serde_json::from_str(
            output
                .strip_prefix("Character loadouts: ")
                .unwrap()
                .split(" · ")
                .next()
                .unwrap(),
        )
        .unwrap()
    };
    let mut saved_registry = fixture.registry();
    saved_registry
        .dispatch(
            angel_live::kernel::Kind::Command,
            "character",
            &json!({"type":"command","name":"character","arguments":"gold 2"}),
        )
        .unwrap();
    saved_registry
        .dispatch(
            angel_live::kernel::Kind::Command,
            "character",
            &json!({"type":"command","name":"character","arguments":"animation breathe 2"}),
        )
        .unwrap();
    select(&mut saved_registry, "spark");
    let saved_choice = read_state(&mut saved_registry);
    assert_eq!(saved_choice["animationChoices"]["2"], "breathe");
    let checkpoint_file = fixture.root.join("abilities/saved-selection.json");
    let checkpoint_bytes = serde_json::to_vec_pretty(&saved_choice).unwrap();
    std::fs::write(&checkpoint_file, &checkpoint_bytes).unwrap();
    let mut saved_run = fight();
    saved_run.join_seat(2, "Restored animation");
    saved_run.tick = 27;
    let hero = saved_run.players.get_mut(&2).unwrap();
    hero.hp = 7;
    hero.mana = 41.0;
    hero.spell_cooldowns[0] = 9;
    let mut saved_runtime = Runtime::default();
    saved_runtime.set_sources(sources(&saved_registry), &mut saved_run);
    let snapshot_file = fixture.root.join("saved-run.json");
    std::fs::write(
        &snapshot_file,
        serde_json::to_vec(&angel_delve::rules::runtime_state::capture(&saved_run).unwrap())
            .unwrap(),
    )
    .unwrap();
    std::fs::remove_file(&motion_manifest).unwrap();
    let mut registry = fixture.registry();
    select(&mut registry, "spark");
    let mut runtime = Runtime::default();
    let mut run = fight();
    runtime.set_sources(sources(&registry), &mut run);
    tick(&mut runtime, &mut run, 1);
    assert_eq!(
        runtime.started,
        1,
        "unselected optional pack blocked live executable ability: {}",
        runtime.status()
    );
    assert_eq!(runtime.effects, 1);
    let selector = fixture.root.join("abilities/index.js");
    let selector_bytes = std::fs::read_to_string(&selector).unwrap();
    let retained_choice = selector_bytes.replace("const initial = () => ({ choices: {}, cancelled: {}, soundBanks: {}, effectBanks: {}, artChoices: {}, animationChoices: {} })", "const initial = () => require('./saved-selection.json')");
    assert_ne!(retained_choice, selector_bytes);
    std::fs::write(&selector, &retained_choice).unwrap();
    let mut selected_cold = fixture.registry();
    assert_eq!(
        sources(&selected_cold).len(),
        2,
        "a retained unavailable clip must permit default art and unrelated abilities"
    );
    assert!(
        selected_cold.snapshot().surfaces["characters"]
            .lines
            .iter()
            .any(|line| line.contains("breathe unavailable; showing art defaults"))
    );
    assert_eq!(
        read_state(&mut selected_cold),
        saved_choice,
        "the actual saved selector checkpoint lost its requested clip while its provider was absent"
    );
    let pending = sources(&selected_cold);
    let pending_art = pending
        .iter()
        .find(|resource| resource.value["player"] == 2)
        .unwrap();
    assert!(pending_art.value["animation"].is_null());
    assert_eq!(
        pending_art
            .binding::<Loadout>()
            .unwrap()
            .appearance
            .as_ref()
            .unwrap()
            .avatar
            .as_ref()
            .unwrap()
            .animation
            .as_ref()
            .unwrap()
            .period(),
        12
    );
    select(&mut selected_cold, "spark");
    let mut restored_run = angel_delve::rules::runtime_state::restore(
        serde_json::from_slice(&std::fs::read(&snapshot_file).unwrap()).unwrap(),
    )
    .unwrap();
    let restored_animation = restored_run.players[&2]
        .avatar
        .as_ref()
        .unwrap()
        .animation
        .as_ref()
        .unwrap();
    let (saved_phase, saved_period) = (
        restored_animation.phase(restored_run.tick),
        restored_animation.period(),
    );
    let mut restored_runtime = Runtime::default();
    restored_runtime.set_sources(sources(&selected_cold), &mut restored_run);
    let default_animation = restored_run.players[&2]
        .avatar
        .as_ref()
        .unwrap()
        .animation
        .as_ref()
        .unwrap();
    assert_eq!(
        default_animation.phase(restored_run.tick),
        saved_phase * default_animation.period() / saved_period
    );
    assert_eq!(
        (
            restored_run.players[&2].hp,
            restored_run.players[&2].mana,
            restored_run.players[&2].spell_cooldowns[0]
        ),
        (7, 41.0, 9)
    );
    tick(&mut restored_runtime, &mut restored_run, 1);
    assert_eq!((restored_runtime.started, restored_runtime.effects), (1, 1));
    let hero_before = restored_run.players[&2].clone();
    let old_animation = hero_before
        .avatar
        .as_ref()
        .unwrap()
        .animation
        .as_ref()
        .unwrap();
    let (old_phase, old_period) = (
        old_animation.phase(restored_run.tick),
        old_animation.period(),
    );
    let epoch = restored_run.seat_epoch(2);
    assert_eq!(
        std::fs::read_to_string(&selector).unwrap(),
        retained_choice,
        "pending admission rewrote persisted selection policy"
    );
    std::fs::write(&motion_manifest, &manifest_bytes).unwrap();
    selected_cold.refresh(true);
    restored_runtime.set_sources(sources(&selected_cold), &mut restored_run);
    let reactivated = restored_run.players[&2]
        .avatar
        .as_ref()
        .unwrap()
        .animation
        .as_ref()
        .unwrap();
    assert_eq!(
        reactivated.phase(restored_run.tick),
        old_phase * reactivated.period() / old_period
    );
    assert_eq!(restored_run.seat_epoch(2), epoch);
    assert_eq!(
        (
            restored_run.players[&2].hp,
            restored_run.players[&2].mana,
            restored_runtime.started
        ),
        (hero_before.hp, hero_before.mana, 1)
    );
    assert!(
        selected_cold.snapshot().surfaces["characters"]
            .lines
            .iter()
            .all(|line| !line.contains("unavailable"))
    );
    assert_eq!(
        read_state(&mut selected_cold),
        saved_choice,
        "provider return changed the restored selector checkpoint"
    );
    assert_eq!(
        std::fs::read(&checkpoint_file).unwrap(),
        checkpoint_bytes,
        "pending/default/reactivation rewrote the saved choice file"
    );
    eprintln!(
        "saved selector JSON cold restore: requested breathe retained; default art plus live ability while absent; provider return phase {old_phase}/{old_period} -> {}/{}",
        reactivated.phase(restored_run.tick),
        reactivated.period()
    );
    let active = sources(&selected_cold);
    let choice = active
        .iter()
        .find(|resource| resource.value["player"] == 2)
        .unwrap();
    assert_eq!(choice.value["animation"]["option"], "breathe");
    let animation = choice
        .binding::<Loadout>()
        .unwrap()
        .appearance
        .as_ref()
        .unwrap()
        .avatar
        .as_ref()
        .unwrap()
        .animation
        .as_ref()
        .unwrap();
    assert_eq!(animation.period(), 16);
    let path = fixture.root.join("character-motion/index.js");
    let motion_bytes = std::fs::read_to_string(&path).unwrap();
    std::fs::write(&path, motion_bytes.replace("breathe:", "removed_breathe:")).unwrap();
    selected_cold.refresh(true);
    assert_eq!(
        sources(&selected_cold),
        active,
        "selected option removal deleted the active generation or choice"
    );
    assert!(selected_cold.snapshot().statuses.iter().any(|status| {
        status
            .error
            .as_deref()
            .is_some_and(|error| error.contains("selected animation is missing"))
    }));
    std::fs::write(&path, &motion_bytes).unwrap();
    let wrong = String::from_utf8(manifest_bytes.clone())
        .unwrap()
        .replace("character-animation-pack/1", "character-animation-pack/999");
    std::fs::write(&motion_manifest, wrong).unwrap();
    selected_cold.refresh(true);
    assert_eq!(
        sources(&selected_cold),
        active,
        "present incompatible optional provider was silently ignored"
    );
    assert!(selected_cold.snapshot().statuses.iter().any(|status| {
        status
            .error
            .as_deref()
            .is_some_and(|error| error.contains("requires character-motion contract"))
    }));
    std::fs::write(&motion_manifest, manifest_bytes).unwrap();
    selected_cold.refresh(true);
    assert_eq!(
        sources(&selected_cold)
            .iter()
            .find(|resource| resource.value["player"] == 2)
            .unwrap()
            .value["animation"]["option"],
        "breathe"
    );
}

#[test]
fn animation_off_restores_art_default_phase_and_survives_art_disable_reenable() {
    let fixture = Fixture::new();
    let mut registry = fixture.registry();
    let choose = |registry: &mut Registry, arguments: &str| {
        registry
            .dispatch(
                angel_live::kernel::Kind::Command,
                "character",
                &json!({"type":"command","name":"character","arguments":arguments}),
            )
            .unwrap();
    };
    choose(&mut registry, "gold 2");
    choose(&mut registry, "animation breathe 2");
    let mut run = fight();
    run.join_seat(2, "Animated");
    run.tick = 27;
    let epoch = run.seat_epoch(2);
    let mut runtime = Runtime::default();
    runtime.set_sources(sources(&registry), &mut run);
    let animation = run.players[&2]
        .avatar
        .as_ref()
        .unwrap()
        .animation
        .as_ref()
        .unwrap();
    let (phase, period) = (animation.phase(run.tick), animation.period());
    choose(&mut registry, "animation off 2");
    runtime.set_sources(sources(&registry), &mut run);
    let avatar = run.players[&2].avatar.as_ref().unwrap();
    let default_id = avatar.source_id.clone();
    let animation = avatar.animation.as_ref().unwrap();
    assert_eq!(animation.period(), 12);
    assert_eq!(animation.phase(run.tick), phase * 12 / period);
    let old_phase = animation.phase(run.tick);
    choose(&mut registry, "animation staccato 2");
    runtime.set_sources(sources(&registry), &mut run);
    let animation = run.players[&2]
        .avatar
        .as_ref()
        .unwrap()
        .animation
        .as_ref()
        .unwrap();
    assert_eq!(animation.period(), 15);
    assert_eq!(animation.phase(run.tick), old_phase * 15 / 12);
    choose(&mut registry, "off 2");
    runtime.set_sources(sources(&registry), &mut run);
    assert!(run.players[&2].avatar.is_none());
    choose(&mut registry, "gold 2");
    runtime.set_sources(sources(&registry), &mut run);
    assert_eq!(
        run.players[&2]
            .avatar
            .as_ref()
            .unwrap()
            .animation
            .as_ref()
            .unwrap()
            .period(),
        15,
        "art toggle retains selected playback"
    );
    choose(&mut registry, "off 2");
    choose(&mut registry, "animation off 2");
    runtime.set_sources(sources(&registry), &mut run);
    assert!(run.players[&2].avatar.is_none());
    choose(&mut registry, "gold 2");
    runtime.set_sources(sources(&registry), &mut run);
    assert_eq!(
        run.players[&2].avatar.as_ref().unwrap().source_id,
        default_id
    );
    assert_eq!(
        run.players[&2]
            .avatar
            .as_ref()
            .unwrap()
            .animation
            .as_ref()
            .unwrap()
            .period(),
        12
    );
    assert_eq!(run.seat_epoch(2), epoch);
    let path = fixture.root.join("character-motion/index.js");
    let bytes = std::fs::read_to_string(&path).unwrap();
    std::fs::write(
        &path,
        bytes.replace(
            "const options = {",
            "const options = {
  newly_authored: [{frame: 1, ticks: 5}, {frame: 0, ticks: 15}],",
        ),
    )
    .unwrap();
    registry.refresh(true);
    choose(&mut registry, "animation newly_authored 2");
    runtime.set_sources(sources(&registry), &mut run);
    assert_eq!(
        run.players[&2]
            .avatar
            .as_ref()
            .unwrap()
            .animation
            .as_ref()
            .unwrap()
            .period(),
        20
    );
    let active = sources(&registry);
    std::fs::write(
        &path,
        bytes.replace(
            "const options = {",
            "const options = {
  off: [{frame: 0, ticks: 1}],",
        ),
    )
    .unwrap();
    registry.refresh(true);
    assert!(registry.snapshot().statuses.iter().any(|status| {
        status
            .error
            .as_deref()
            .is_some_and(|error| error.contains("off is reserved"))
    }));
    assert_eq!(sources(&registry), active);
}

#[test]
fn replaced_seat_cannot_admit_previous_occupants_queued_cast() {
    let fixture = Fixture::new();
    let mut registry = fixture.registry();
    select(&mut registry, "spark 2 1");
    let mut runtime = Runtime::default();
    let mut run = fight();
    run.join_seat(2, "Original");
    runtime.set_sources(sources(&registry), &mut run);
    runtime.before_step(&mut run);
    run.step(&BTreeMap::from([(2, Input { cast: 1, ..Default::default() })]));
    assert_eq!(run.module_casts.len(), 1);
    let old_epoch = run.module_casts[0].epoch;
    run.leave(2);
    run.join_seat(2, "Replacement");
    assert_ne!(run.seat_epoch(2), old_epoch);
    let mana = run.players[&2].mana;
    runtime.after_step(&mut run);
    assert_eq!(runtime.started, 0, "stale request started: {}", runtime.status());
    assert!(runtime.pending.is_empty());
    assert_eq!(run.players[&2].mana, mana);
    assert_eq!(runtime.effects, 0);
}

#[test]
fn replacement_between_steps_retires_cast_but_keeps_already_issued_effects() {
    let fixture = Fixture::new();
    let mut registry = fixture.registry();
    select(&mut registry, "spark 2 1");
    let mut runtime = Runtime::default();
    let mut run = fight();
    run.join_seat(2, "Original");
    runtime.set_sources(sources(&registry), &mut run);
    runtime.before_step(&mut run);
    run.step(&BTreeMap::from([(2, Input { cast: 1, ..Default::default() })]));
    runtime.after_step(&mut run);
    assert_eq!(runtime.started, 1);
    assert_eq!(runtime.effects, 1);
    assert_eq!(runtime.groups.values().map(|g| g.casts.len()).sum::<usize>(), 1);
    let projectiles = run.projectiles.len();
    let visuals = serde_json::to_value(&run.module_visuals).unwrap();
    let voices = runtime.media.take();
    assert!(projectiles > 0 && !voices.is_empty());
    let old_epoch = run.seat_epoch(2);
    run.leave(2);
    run.join_seat(2, "Replacement");
    assert_ne!(run.seat_epoch(2), old_epoch);
    runtime.before_step(&mut run);
    assert!(runtime.groups.values().all(|g| g.casts.is_empty()));
    assert_eq!(run.projectiles.len(), projectiles);
    assert_eq!(serde_json::to_value(&run.module_visuals).unwrap(), visuals);
    assert!(!voices[0].name().is_empty());
    for _ in 0..6 { tick(&mut runtime, &mut run, 0); }
    assert_eq!(runtime.effects, 1, "retired cast emitted a new effect");
}

#[test]
fn same_tick_replacement_fences_cast_before_after_step_returns_early() {
    let fixture = Fixture::new();
    let mut registry = fixture.registry();
    select(&mut registry, "spark 2 1");
    let mut runtime = Runtime::default();
    let mut run = fight();
    run.join_seat(2, "Original");
    runtime.set_sources(sources(&registry), &mut run);
    runtime.before_step(&mut run);
    run.step(&BTreeMap::from([(2, Input { cast: 1, ..Default::default() })]));
    runtime.after_step(&mut run);
    assert_eq!(runtime.groups.values().map(|g| g.casts.len()).sum::<usize>(), 1);
    run.leave(2);
    run.join_seat(2, "Replacement");
    // Re-entrant same-tick processing normally returns immediately. It must
    // still retire the prior owner's work before the return.
    runtime.after_step(&mut run);
    assert!(runtime.groups.values().all(|group| group.casts.is_empty()));
    assert_eq!(runtime.effects, 1);
}

#[test]
fn queued_cast_from_another_raid_is_not_admitted() {
    let fixture = Fixture::new();
    let mut registry = fixture.registry();
    select(&mut registry, "spark");
    let mut runtime = Runtime::default();
    let mut run = fight();
    runtime.set_sources(sources(&registry), &mut run);
    runtime.before_step(&mut run);
    run.step(&BTreeMap::from([(1, Input { cast: 1, ..Default::default() })]));
    assert_eq!(run.module_casts.len(), 1);
    run.module_casts[0].raid = run.raid_id.wrapping_add(1);
    let mana = run.players[&1].mana;
    runtime.after_step(&mut run);
    assert_eq!(runtime.started, 0);
    assert!(runtime.pending.is_empty());
    assert_eq!(run.players[&1].mana, mana);
}

#[test]
fn six_seat_character_choices_and_live_casts_survive_reload() {
    assert_eq!(
        angel_delve::rules::MAX_PLAYERS,
        6,
        "update the shipped character module's seat limit"
    );
    let fixture = Fixture::new();
    let mut registry = fixture.registry();
    for player in [5, 6] {
        registry
            .dispatch(
                angel_live::kernel::Kind::Command,
                "character",
                &json!({"type":"command","name":"character","arguments":format!("gold {player}")}),
            )
            .unwrap();
        select(&mut registry, &format!("spark {player} 1"));
        select(&mut registry, &format!("sound warm {player}"));
        select(&mut registry, &format!("effect ember {player}"));
        let snapshot = registry.snapshot();
        let loadout = snapshot.resources[&format!("abilities/character-seat-{player}")]
            .binding::<Loadout>()
            .unwrap();
        assert_eq!(loadout.player, player);
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
        assert_eq!(loadout.media["spark"].level, 30);
        assert!(loadout.vfx.contains_key("spark"));
    }
    let before = registry.snapshot().resources;
    select(&mut registry, "spark 7 1");
    select(&mut registry, "spark 5 5");
    assert_eq!(registry.snapshot().resources, before);
    let mut run = fight();
    for player in 2..=angel_delve::rules::MAX_PLAYERS {
        run.join_seat(player, &format!("Player {player}"));
    }
    let mut runtime = Runtime::default();
    runtime.set_sources(sources(&registry), &mut run);
    let tick_start = std::time::Instant::now();
    runtime.before_step(&mut run);
    run.step(
        &[5, 6]
            .into_iter()
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
    let two_cast_tick = tick_start.elapsed();
    assert_eq!(runtime.started, 2, "{}", runtime.status());
    assert!(
        run.module_visuals
            .effects
            .iter()
            .any(|effect| effect.player == 5)
    );
    assert!(
        run.module_visuals
            .effects
            .iter()
            .any(|effect| effect.player == 6)
    );
    run.module_visuals.validate().unwrap();
    for invalid_player in [0, angel_delve::rules::MAX_PLAYERS + 1] {
        let mut invalid = run.module_visuals.clone();
        invalid.effects[0].player = invalid_player;
        assert!(invalid.validate().is_err());
    }
    let issued_effects = serde_json::to_value(&run.module_visuals).unwrap();
    let issued_voices = runtime.media.take();
    assert_eq!(issued_voices.len(), 2);
    let voice_names: Vec<_> = issued_voices.iter().map(|voice| voice.name()).collect();
    let before = [5, 6].map(|player| {
        (
            run.players[&player].hp,
            run.players[&player].mana,
            run.players[&player].spell_cooldowns,
        )
    });
    let active = runtime
        .groups
        .values()
        .map(|group| group.casts.len())
        .sum::<usize>();
    assert_eq!(active, 2);
    fixture.edit(&format!("{}\n", fixture.source()));
    let activation_start = std::time::Instant::now();
    registry.refresh(true);
    for status in &registry.snapshot().statuses {
        assert!(status.error.is_none(), "{status:?}");
    }
    runtime.set_sources(sources(&registry), &mut run);
    eprintln!(
        "six-seat sample: two-cast rules/ability tick {two_cast_tick:?}; edit detection, validation and source activation {:?}",
        activation_start.elapsed()
    );
    assert_eq!(
        serde_json::to_value(&run.module_visuals).unwrap(),
        issued_effects
    );
    assert_eq!(
        issued_voices
            .iter()
            .map(|voice| voice.name())
            .collect::<Vec<_>>(),
        voice_names
    );
    assert!(
        issued_voices
            .iter()
            .all(|voice| voice.sound.path().is_file())
    );
    assert_eq!(
        runtime
            .groups
            .values()
            .map(|group| group.casts.len())
            .sum::<usize>(),
        active
    );
    for (index, player) in [5, 6].into_iter().enumerate() {
        assert_eq!(
            (
                run.players[&player].hp,
                run.players[&player].mana,
                run.players[&player].spell_cooldowns
            ),
            before[index]
        );
        assert_eq!(
            runtime.sources[&player]
                .binding::<Loadout>()
                .unwrap()
                .executables["slot1"]
                .ability,
            "spark"
        );
    }
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
        old.replace("character-graph-loadout/1", "character-graph-loadout/999"),
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
    for name in ["fairies.bend", "LAWS.bend", "PROOF.bend", "fairies.compiled.js", "bend-build.json"] {
        std::fs::copy(fixture.root.join("ability-code").join(name), other.join(name)).unwrap();
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
    fixture.edit(&fixture.source().replace("parameters: fairyParameters", "parameters: { ...fairyParameters, hold: 0 }"));
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

fn graph_fixture() -> Fixture {
    let fixture = Fixture::new();
    let dir = fixture.root.join("character-graphs");
    std::fs::create_dir_all(&dir).unwrap();
    for file in ["module.json", "index.js"] {
        std::fs::copy(
            std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("live/character-graphs")
                .join(file),
            dir.join(file),
        )
        .unwrap();
    }
    fixture
}
fn character(registry: &mut Registry, args: &str) {
    registry
        .dispatch(
            angel_live::kernel::Kind::Command,
            "character",
            &json!({"type":"command","name":"character","arguments":args}),
        )
        .unwrap();
}
fn graph_state(run: &Run, player: u32) -> &str {
    run.players[&player]
        .avatar
        .as_ref()
        .unwrap()
        .source_id
        .rsplit(':')
        .next()
        .unwrap()
}

#[test]
fn authored_graph_drives_real_gameplay_pixels_reload_phase_and_isolates_seats() {
    let fixture = graph_fixture();
    let mut registry = fixture.registry();
    for args in ["gold 1", "gold 2", "graph adventure 1", "graph adventure 2"] {
        character(&mut registry, args);
    }
    assert!(
        registry
            .snapshot()
            .statuses
            .iter()
            .all(|s| s.error.is_none()),
        "{:?}",
        registry.snapshot().statuses
    );
    let mut run = fight();
    run.join_seat(2, "Other");
    let mut runtime = Runtime::default();
    runtime.set_sources(sources(&registry), &mut run);
    assert_eq!(graph_state(&run, 1), "rest");
    let resting = run.players[&1]
        .avatar
        .as_ref()
        .unwrap()
        .pixels_at(run.tick)
        .to_vec();
    // Actual rule admission, not a manually assigned animation fact.
    runtime.before_step(&mut run);
    run.step(&BTreeMap::from([(
        1,
        Input {
            swing: true,
            ..Default::default()
        },
    )]));
    runtime.after_step(&mut run);
    assert!(run.players[&1].swing > 0);
    assert_eq!(graph_state(&run, 1), "attack");
    assert_eq!(graph_state(&run, 2), "rest");
    let avatar = run.players[&1].avatar.as_ref().unwrap();
    assert_eq!(avatar.animation.as_ref().unwrap().phase(run.tick), 0);
    assert_ne!(avatar.pixels_at(run.tick), resting);
    let (_, guest, picture) = crate::delve::together_seat::guest_view(&run, 1);
    assert_eq!(
        guest.players[&1]
            .avatar
            .as_ref()
            .unwrap()
            .pixels_at(run.tick),
        avatar.pixels_at(run.tick)
    );
    assert!(picture.w > 0 && picture.h > 0);
    #[derive(serde::Deserialize)]
    #[serde(deny_unknown_fields)]
    struct Legacy {
        frames: Vec<Vec<[u8; 4]>>,
        frame_ticks: u32,
        phase_offset: u32,
    }
    let legacy: Legacy =
        serde_json::from_value(serde_json::to_value(avatar.animation.as_ref().unwrap()).unwrap())
            .unwrap();
    for tick in 0..20 {
        assert_eq!(
            legacy.frames[((tick + u64::from(legacy.phase_offset)) / u64::from(legacy.frame_ticks))
                as usize
                % legacy.frames.len()],
            avatar.pixels_at(tick)
        );
    }
    for _ in 0..20 {
        runtime.before_step(&mut run);
        run.step(&BTreeMap::new());
        runtime.after_step(&mut run);
    }
    assert_eq!(graph_state(&run, 1), "rest");
    runtime.before_step(&mut run);
    run.step(&BTreeMap::from([(
        1,
        Input {
            move_x: 1,
            ..Default::default()
        },
    )]));
    runtime.after_step(&mut run);
    assert!(run.players[&1].stride > 0.0);
    assert_eq!(graph_state(&run, 1), "walk");
    let path = fixture.root.join("character-graphs/index.js");
    let original = std::fs::read_to_string(&path).unwrap();
    let before = run.players[&1].clone();
    let old = before.avatar.as_ref().unwrap().animation.as_ref().unwrap();
    let old_phase = old.phase(run.tick);
    let started = std::time::Instant::now();
    std::fs::write(
        &path,
        original
            .replace("frame: 0, ticks: 4", "frame: 0, ticks: 12")
            .replace("frame: 1, ticks: 4", "frame: 1, ticks: 12"),
    )
    .unwrap();
    registry.refresh(true);
    runtime.set_sources(sources(&registry), &mut run);
    assert_eq!(graph_state(&run, 1), "walk");
    let next = run.players[&1]
        .avatar
        .as_ref()
        .unwrap()
        .animation
        .as_ref()
        .unwrap();
    assert_eq!(
        next.phase(run.tick),
        old_phase * next.period() / old.period()
    );
    assert_eq!(
        (run.players[&1].hp, run.players[&1].mana, run.players[&1].x),
        (before.hp, before.mana, before.x)
    );
    let (_, _, changed_picture) = crate::delve::together_seat::guest_view(&run, 1);
    eprintln!(
        "graph reload-to-guest {:?}, image pixels {}",
        started.elapsed(),
        changed_picture.w * changed_picture.h
    );
    let accepted = sources(&registry);
    std::fs::write(
        &path,
        original.replace(
            "=> ({ to, when, phase })",
            "=> ({ to: 'missing', when, phase })",
        ),
    )
    .unwrap();
    registry.refresh(true);
    assert!(
        registry
            .snapshot()
            .statuses
            .iter()
            .any(|s| s.error.is_some())
    );
    assert_eq!(
        sources(&registry),
        accepted,
        "bad graph must retain whole last-good loadout"
    );
    // Held shielding is a neutral gameplay fact, and release routes back through authored policy.
    run.players.get_mut(&1).unwrap().guard = angel_delve::rules::cards::Guard::Shield;
    runtime.before_step(&mut run);
    run.step(&BTreeMap::from([(
        1,
        Input {
            dash: true,
            ..Default::default()
        },
    )]));
    runtime.after_step(&mut run);
    assert!(run.players[&1].shielding);
    assert_eq!(graph_state(&run, 1), "guard");
    runtime.before_step(&mut run);
    run.step(&BTreeMap::new());
    runtime.after_step(&mut run);
    assert!(!run.players[&1].shielding);
    assert_eq!(graph_state(&run, 1), "rest");
    // Repeated same-tick observers cannot consume a second edge or reset phase.
    let phase = run.players[&1]
        .avatar
        .as_ref()
        .unwrap()
        .animation
        .as_ref()
        .unwrap()
        .phase(run.tick);
    runtime.after_step(&mut run);
    assert_eq!(
        run.players[&1]
            .avatar
            .as_ref()
            .unwrap()
            .animation
            .as_ref()
            .unwrap()
            .phase(run.tick),
        phase
    );
    runtime.set_sources(vec![], &mut run);
    assert!(run.players[&1].avatar.is_none());
    assert!(run.players[&2].avatar.is_none());
    character(&mut registry, "graph adventure 2");
    runtime.set_sources(sources(&registry), &mut run);
    run.players
        .get_mut(&2)
        .unwrap()
        .avatar
        .as_mut()
        .unwrap()
        .source_id = "independent-takeover".into();
    run.players.get_mut(&2).unwrap().swing = 4;
    run.tick += 1;
    runtime.after_step(&mut run);
    runtime.set_sources(sources(&registry), &mut run);
    assert_eq!(
        run.players[&2].avatar.as_ref().unwrap().source_id,
        "independent-takeover",
        "graph transition/reload overwrote independent art"
    );
    runtime.set_sources(vec![], &mut run);
    assert_eq!(
        run.players[&2].avatar.as_ref().unwrap().source_id,
        "independent-takeover",
        "graph cleanup erased independently installed art"
    );
}

#[test]
fn graph_missing_provider_retains_live_generation_cold_defaults_reactivates_and_fences_seat() {
    let fixture = graph_fixture();
    let mut registry = fixture.registry();
    character(&mut registry, "gold 2");
    character(&mut registry, "graph adventure 2");
    select(&mut registry, "spark 2 1");
    let mut run = fight();
    run.join_seat(2, "First");
    let mut runtime = Runtime::default();
    runtime.set_sources(sources(&registry), &mut run);
    let accepted = sources(&registry);
    let manifest = fixture.root.join("character-graphs/module.json");
    let bytes = std::fs::read(&manifest).unwrap();
    std::fs::remove_file(&manifest).unwrap();
    registry.refresh(true);
    assert_eq!(
        sources(&registry),
        accepted,
        "live dependency removal must retain last-good whole generation"
    );
    // Cold restored selector fixture: captured state, not a production durable profile.
    let output = registry
        .dispatch(
            angel_live::kernel::Kind::Command,
            "loadout",
            &json!({"type":"command","name":"loadout","arguments":""}),
        )
        .unwrap();
    let output = output["output"].as_str().unwrap();
    let state: Value = serde_json::from_str(
        output
            .strip_prefix("Character loadouts: ")
            .unwrap()
            .split(" · ")
            .next()
            .unwrap(),
    )
    .unwrap();
    let selector = fixture.root.join("abilities/index.js");
    std::fs::write(
        fixture.root.join("abilities/saved-graph.json"),
        state.to_string(),
    )
    .unwrap();
    let source = std::fs::read_to_string(&selector).unwrap();
    std::fs::write(&selector, source.replace("const initial = () => ({ choices: {}, cancelled: {}, soundBanks: {}, effectBanks: {}, artChoices: {}, animationChoices: {} })", "const initial = () => require('./saved-graph.json')")).unwrap();
    let mut cold = fixture.registry();
    assert!(
        cold.snapshot()
            .statuses
            .iter()
            .filter(|status| status.id != "character-graphs")
            .all(|status| status.error.is_none()),
        "{:?}",
        cold.snapshot().statuses
    );
    assert!(
        cold.snapshot()
            .statuses
            .iter()
            .any(|status| status.id == "character-graphs"
                && status.error.as_deref() == Some("missing module.json")),
        "the absent optional provider remains visibly diagnosed"
    );
    let mut cold_runtime = Runtime::default();
    cold_runtime.set_sources(sources(&cold), &mut run);
    assert!(run.players[&2].module_spells[0].is_some());
    assert_ne!(
        graph_state(&run, 2),
        "rest",
        "missing graph uses art default identity"
    );
    std::fs::write(&manifest, bytes).unwrap();
    cold.refresh(true);
    cold_runtime.set_sources(sources(&cold), &mut run);
    assert_eq!(graph_state(&run, 2), "rest");
    let pointer = run.players[&2]
        .avatar
        .as_ref()
        .unwrap()
        .animation
        .as_ref()
        .unwrap()
        .frames[0]
        .as_ptr();
    let start = std::time::Instant::now();
    for _ in 0..100_000 {
        cold_runtime.before_step(&mut run);
        run.tick += 1;
        cold_runtime.after_step(&mut run);
    }
    eprintln!(
        "graph steady 100k paired runtime ticks {:?}",
        start.elapsed()
    );
    assert_eq!(
        run.players[&2]
            .avatar
            .as_ref()
            .unwrap()
            .animation
            .as_ref()
            .unwrap()
            .frames[0]
            .as_ptr(),
        pointer,
        "steady graph ticks copied frames"
    );
    // Visibility and ownership are separate: an absent/away seat can return
    // without a new epoch, including a provider replacement while hidden.
    let visibility_epoch = run.seat_epoch(2);
    let before_hidden = run.players[&2]
        .avatar
        .as_ref()
        .unwrap()
        .animation
        .as_ref()
        .unwrap()
        .clone();
    run.set_absent(2, true);
    cold_runtime.after_step(&mut run);
    assert!(run.players[&2].avatar.is_some());
    std::fs::write(fixture.root.join("character-graphs/index.js"), "module.exports = {apiVersion:2, initialize:()=>({}), reduce:state=>({state}), resources:()=>({pack:{options:{adventure:{initial:'replacement',states:{replacement:{steps:[{frame:0,ticks:24}],transitions:[]}}}}}})}").unwrap();
    cold.refresh(true);
    cold_runtime.set_sources(sources(&cold), &mut run);
    let hidden_animation = run.players[&2]
        .avatar
        .as_ref()
        .unwrap()
        .animation
        .as_ref()
        .unwrap();
    assert_eq!(
        hidden_animation.phase(run.tick),
        before_hidden.phase(run.tick) * hidden_animation.period() / before_hidden.period()
    );
    run.set_absent(2, false);
    run.set_away(2, false);
    run.tick += 1;
    cold_runtime.after_step(&mut run);
    assert_eq!(run.seat_epoch(2), visibility_epoch);
    assert_eq!(
        graph_state(&run, 2),
        "replacement",
        "same-epoch visibility change retired graph"
    );
    let mut new_owner_avatar = run.players[&2].avatar.clone().unwrap();
    new_owner_avatar.animation.as_mut().unwrap().phase_offset = 7;
    let old_epoch = run.seat_epoch(2);
    run.leave(2);
    run.join_seat(2, "New");
    assert_ne!(run.seat_epoch(2), old_epoch);
    run.players.get_mut(&2).unwrap().avatar = Some(new_owner_avatar.clone());
    let expected = serde_json::to_value(&new_owner_avatar).unwrap();
    cold_runtime.after_step(&mut run);
    assert_eq!(
        serde_json::to_value(run.players[&2].avatar.as_ref().unwrap()).unwrap(),
        expected,
        "stale graph cleanup mutated new owner with identical fingerprint"
    );
    cold_runtime.set_sources(sources(&cold), &mut run);
    assert_eq!(
        serde_json::to_value(run.players[&2].avatar.as_ref().unwrap()).unwrap(),
        expected,
        "stale activation adopted new occupant"
    );
    cold_runtime.set_sources(vec![], &mut run);
    assert_eq!(
        serde_json::to_value(run.players[&2].avatar.as_ref().unwrap()).unwrap(),
        expected,
        "stale removal mutated replacement avatar"
    );
}

#[test]
fn graph_validation_fails_closed_and_old_selection_contracts_remain_strict() {
    let fixture = graph_fixture();
    let mut registry = fixture.registry();
    character(&mut registry, "gold 1");
    character(&mut registry, "graph adventure 1");
    let accepted = sources(&registry);
    let path = fixture.root.join("character-graphs/index.js");
    let original = std::fs::read_to_string(&path).unwrap();
    for broken in [
        original.replace("moving: true", "\"not-a-fact\": true"),
        original.replace("ticks: 12", "ticks: 301"),
        original
            .replace("frame: 0, ticks: 4", "frame: 0, ticks: 7")
            .replace("frame: 1, ticks: 4", "frame: 1, ticks: 11"),
        original.replace("frame: 1, ticks: 3", "frame: 7, ticks: 3"),
        original.replace("adventure:", "off:"),
    ] {
        std::fs::write(&path, broken).unwrap();
        registry.refresh(true);
        assert!(
            registry
                .snapshot()
                .statuses
                .iter()
                .any(|s| s.error.is_some())
        );
        assert_eq!(sources(&registry), accepted);
    }
    std::fs::write(&path, original).unwrap();
    registry.refresh(true);
    let manifest_path = fixture.root.join("abilities/module.json");
    let manifest = std::fs::read_to_string(&manifest_path).unwrap();
    for contract in ["character-loadout/1", "character-animated-loadout/1"] {
        std::fs::write(
            &manifest_path,
            manifest.replace("character-graph-loadout/1", contract),
        )
        .unwrap();
        registry.refresh(true);
        assert!(registry.snapshot().statuses.iter().any(|s| {
            s.error
                .as_ref()
                .is_some_and(|error| error.contains("graph requires"))
        }));
        assert_eq!(sources(&registry), accepted);
    }
}

#[test]
fn graph_loadout_activates_through_production_sim_filter_and_reloads() {
    let fixture = graph_fixture();
    let mut registry = fixture.registry();
    character(&mut registry, "gold 1");
    character(&mut registry, "graph adventure 1");
    select(&mut registry, "spark 1 1");
    let mut sim = crate::delve::together_sim::Sim::default();
    sim.prefer_thread(false);
    sim.set(Some(fight()));
    sim.set_live_characters(registry.snapshot().resources.values());
    assert_eq!(graph_state(sim.as_ref().unwrap(), 1), "rest");
    assert!(sim.as_ref().unwrap().players[&1].module_spells[0].is_some());
    let before = sim.as_ref().unwrap().players[&1]
        .avatar
        .as_ref()
        .unwrap()
        .source_id
        .clone();
    let path = fixture.root.join("character-graphs/index.js");
    let source = std::fs::read_to_string(&path).unwrap();
    std::fs::write(
        &path,
        source.replace("frame: 0, ticks: 12", "frame: 1, ticks: 12"),
    )
    .unwrap();
    registry.refresh(true);
    sim.set_live_characters(registry.snapshot().resources.values());
    let run = sim.as_ref().unwrap();
    assert_eq!(graph_state(run, 1), "rest");
    assert_ne!(run.players[&1].avatar.as_ref().unwrap().source_id, before);
    let (_, guest, pixels) = crate::delve::together_seat::guest_view(run, 1);
    assert_eq!(
        guest.players[&1]
            .avatar
            .as_ref()
            .unwrap()
            .pixels_at(run.tick),
        run.players[&1].avatar.as_ref().unwrap().pixels_at(run.tick)
    );
    assert!(pixels.w > 0 && pixels.h > 0);
    sim.set(None);
    assert!(sim.as_ref().is_none());
}

#[test]
fn explicit_sim_avatar_intents_retire_graph_even_when_fingerprint_is_identical() {
    let fixture = graph_fixture();
    let mut registry = fixture.registry();
    character(&mut registry, "gold 1");
    character(&mut registry, "graph adventure 1");
    let path = fixture.root.join("character-graphs/index.js");
    let original = std::fs::read_to_string(&path).unwrap();
    for (iteration, (through_command, empty_id)) in
        [(false, false), (true, false), (false, true), (true, true)]
            .into_iter()
            .enumerate()
    {
        let mut sim = crate::delve::together_sim::Sim::default();
        sim.prefer_thread(false);
        sim.set(Some(fight()));
        sim.set_live_characters(registry.snapshot().resources.values());
        let mut independent = sim.as_ref().unwrap().players[&1].avatar.clone().unwrap();
        independent.animation.as_mut().unwrap().phase_offset = 7;
        if empty_id {
            independent.source_id.clear();
        }
        let expected = serde_json::to_value(&independent).unwrap();
        if through_command {
            sim.commands_with_avatars(
                vec![angel_delve::rules::host_commands::Command::MirrorName {
                    player: 1,
                    words: "Transferred".into(),
                }],
                vec![(1, Some(independent))],
            )
            .unwrap();
        } else {
            assert!(sim.set_avatar(1, Some(independent)));
        }
        std::fs::write(
            &path,
            original.replace("ticks: 12", &format!("ticks: {}", 13 + iteration)),
        )
        .unwrap();
        registry.refresh(true);
        sim.set_live_characters(registry.snapshot().resources.values());
        assert_eq!(
            serde_json::to_value(sim.as_ref().unwrap().players[&1].avatar.as_ref().unwrap())
                .unwrap(),
            expected,
            "graph reload stole explicit same-fingerprint avatar assignment"
        );
        sim.set_live_characters(std::iter::empty::<&Resource>());
        assert_eq!(
            serde_json::to_value(sim.as_ref().unwrap().players[&1].avatar.as_ref().unwrap())
                .unwrap(),
            expected,
            "graph retirement erased explicit same-fingerprint assignment"
        );
        sim.set(None);
    }
}

struct GraphPeer {
    ws: angel_serve::client::WsClient,
    seat: Option<u32>,
    token: String,
    newest: u32,
    seq: u32,
}
impl GraphPeer {
    fn open(sim: &crate::delve::together_sim::Sim, resume: &str) -> Self {
        let addr = sim.server().unwrap().addr();
        let mut ws = angel_serve::client::WsClient::connect(
            &format!("ws://{addr}/ws"),
            std::time::Duration::from_secs(5),
        )
        .unwrap();
        ws.send(
            &angel_net::proto::ToServer::Hello {
                protocol: angel_net::proto::PROTOCOL,
                game: angel_delve::wire::GAME,
                join: "GRAPHKEY",
                resume,
                name: "GraphPeer",
                caps: 0,
            }
            .to_vec(),
        )
        .unwrap();
        Self {
            ws,
            seat: None,
            token: String::new(),
            newest: 0,
            seq: 0,
        }
    }
    fn read(&mut self) {
        while let Ok(Some(message)) = self.ws.recv(std::time::Duration::from_millis(2)) {
            if let angel_serve::ws::Message::Binary(bytes) = message {
                match angel_net::proto::ToClient::decode(&bytes).unwrap() {
                    angel_net::proto::ToClient::Welcome(w) => {
                        self.seat = w.seat;
                        self.token = w.seat_token.to_owned();
                    }
                    angel_net::proto::ToClient::Snapshot { tick, .. } => {
                        self.newest = self.newest.max(tick)
                    }
                    _ => {}
                }
            }
        }
    }
}
fn graph_peer_play(
    sim: &mut crate::delve::together_sim::Sim,
    mut peer: Option<&mut GraphPeer>,
    ticks: u32,
    move_x: i8,
) {
    for _ in 0..ticks {
        if let Some(peer) = peer.as_mut() {
            peer.read();
            if peer.seat.is_some() {
                peer.seq += 1;
                let mut body = Vec::new();
                angel_delve::rules::command::DelveCmd {
                    held: Input {
                        move_x,
                        ..Default::default()
                    },
                    pressed: Default::default(),
                }
                .encode(&mut body);
                peer.ws
                    .send(
                        &angel_net::proto::ToServer::Cmds {
                            applied: peer.newest,
                            cmds: vec![(peer.seq, body.as_slice())],
                        }
                        .to_vec(),
                    )
                    .unwrap();
            }
        }
        std::thread::sleep(std::time::Duration::from_millis(34));
        let now = std::time::Instant::now();
        if let Some(seat) = sim.seat(1) {
            seat.tick(now);
        }
        sim.pump(now);
        for news in sim.drain_news() {
            if let crate::delve::together_sim::News::Joined { knight, name } = news {
                sim.as_mut().unwrap().join_seat(knight, &name);
            }
        }
        if let Some(peer) = peer.as_mut() {
            peer.read();
        }
        sim.latch();
    }
}
#[test]
fn graph_survives_actual_same_token_socket_disconnect_and_resume() {
    let fixture = graph_fixture();
    let mut registry = fixture.registry();
    let mut sim = crate::delve::together_sim::Sim::default();
    sim.prefer_thread(false);
    sim.set(Some(fight()));
    sim.host(crate::delve::together_sim::Door {
        bind: "127.0.0.1:0".parse().unwrap(),
        seats: 3,
        key: Some("GRAPHKEY".into()),
        bundle: None,
    })
    .unwrap();
    sim.set_controls(crate::delve::together_sim::Controls {
        active: true,
        keyboard: true,
        hosted: true,
        ..Default::default()
    });
    let mut peer = GraphPeer::open(&sim, "");
    graph_peer_play(&mut sim, Some(&mut peer), 15, 0);
    let seat = peer.seat.unwrap();
    let player = seat + 1;
    assert_eq!(player, 2);
    character(&mut registry, "gold 2");
    character(&mut registry, "graph adventure 2");
    sim.set_live_characters(registry.snapshot().resources.values());
    let epoch = sim.as_ref().unwrap().seat_epoch(player);
    graph_peer_play(&mut sim, Some(&mut peer), 4, 1);
    assert_eq!(graph_state(sim.as_ref().unwrap(), player), "walk");
    let token = peer.token.clone();
    assert!(!token.is_empty());
    drop(peer);
    graph_peer_play(&mut sim, None, 10, 0);
    assert!(sim.as_ref().unwrap().players[&player].away);
    assert!(sim.as_ref().unwrap().players[&player].avatar.is_some());
    let mut resumed = GraphPeer::open(&sim, &token);
    graph_peer_play(&mut sim, Some(&mut resumed), 15, -1);
    assert_eq!(resumed.seat, Some(seat));
    assert_eq!(resumed.token, token);
    let run = sim.as_ref().unwrap();
    assert_eq!(run.seat_epoch(player), epoch);
    assert!(!run.players[&player].away);
    assert_eq!(
        graph_state(run, player),
        "walk",
        "same-token resume lost graph ownership"
    );
    let (_, guest, _) = crate::delve::together_seat::guest_view(run, player);
    assert_eq!(
        guest.players[&player]
            .avatar
            .as_ref()
            .unwrap()
            .pixels_at(run.tick),
        run.players[&player]
            .avatar
            .as_ref()
            .unwrap()
            .pixels_at(run.tick)
    );
    drop(resumed);
    sim.set(None);
}

#[test]
fn graph_movement_remains_walking_when_real_consecutive_steps_wrap_footstep_phase() {
    let fixture = graph_fixture();
    let mut registry = fixture.registry();
    character(&mut registry, "gold 1");
    character(&mut registry, "graph adventure 1");
    let mut run = fight();
    run.enemies.clear();
    run.works.perils.clear();
    let mut runtime = Runtime::default();
    runtime.set_sources(sources(&registry), &mut run);
    let input = BTreeMap::from([(
        1,
        Input {
            move_x: 1,
            ..Default::default()
        },
    )]);
    runtime.before_step(&mut run);
    run.step(&input);
    runtime.after_step(&mut run);
    assert_eq!(graph_state(&run, 1), "walk");
    // Probe the next real step's exact f32 displacement. Seed only the
    // unrelated footstep phase so this continuously held input lands exactly
    // on spectacle's 3.4 threshold, without moving/teleporting the hero.
    let mut probe = run.clone();
    let (x, y) = (run.players[&1].x, run.players[&1].y);
    probe.step(&input);
    let moved = (probe.players[&1].x - x).hypot(probe.players[&1].y - y);
    assert!(moved > 0.02 && moved < 2.5);
    run.players.get_mut(&1).unwrap().stride = 3.4 - moved;
    runtime.before_step(&mut run);
    run.step(&input);
    assert_eq!(
        run.players[&1].stride, 0.0,
        "fixture did not hit the footstep wrap"
    );
    assert_eq!(
        (run.players[&1].x, run.players[&1].y),
        (probe.players[&1].x, probe.players[&1].y)
    );
    runtime.after_step(&mut run);
    assert_eq!(
        graph_state(&run, 1),
        "walk",
        "continuous movement flickered idle at the footstep threshold"
    );
    runtime.before_step(&mut run);
    run.step(&input);
    runtime.after_step(&mut run);
    assert_eq!(graph_state(&run, 1), "walk");
    runtime.before_step(&mut run);
    run.step(&BTreeMap::new());
    runtime.after_step(&mut run);
    assert_eq!(
        graph_state(&run, 1),
        "rest",
        "stopping must clear the per-tick movement fact"
    );
    // Reused room indices on another floor/topology are not locomotion.
    for topology_edit in [false, true] {
        runtime.before_step(&mut run);
        let x = run.players[&1].x;
        run.step(&input);
        assert!(run.players[&1].x > x);
        if topology_edit {
            run.dungeon.world_revision += 1;
        } else {
            run.dungeon.depth += 1;
        }
        runtime.after_step(&mut run);
        assert_eq!(
            graph_state(&run, 1),
            "rest",
            "reused room index crossed a floor/topology boundary"
        );
    }
}

#[test]
fn graph_movement_sampling_rejects_pause_repeats_teleports_room_changes_and_owner_transfer() {
    let fixture = graph_fixture();
    let mut registry = fixture.registry();
    character(&mut registry, "gold 2");
    character(&mut registry, "graph adventure 2");
    let mut run = fight();
    run.join_seat(2, "MotionOwner");
    run.enemies.clear();
    run.works.perils.clear();
    let mut runtime = Runtime::default();
    runtime.set_sources(sources(&registry), &mut run);
    // Repeated observers and a pause cannot interpret between-frame edits as a step.
    runtime.before_step(&mut run);
    run.players.get_mut(&2).unwrap().x += 0.1;
    runtime.after_step(&mut run);
    runtime.after_step(&mut run);
    assert_eq!(graph_state(&run, 2), "rest");
    let input = BTreeMap::from([(
        2,
        Input {
            move_x: 1,
            ..Default::default()
        },
    )]);
    runtime.before_step(&mut run);
    run.step(&input);
    runtime.after_step(&mut run);
    assert_eq!(graph_state(&run, 2), "walk");
    let identity = run.players[&2].avatar.as_ref().unwrap().source_id.clone();
    let offset = run.players[&2]
        .avatar
        .as_ref()
        .unwrap()
        .animation
        .as_ref()
        .unwrap()
        .phase_offset;
    // No actual step, even if position changes: don't consume walk->rest or reset phase.
    runtime.before_step(&mut run);
    run.players.get_mut(&2).unwrap().x += 0.1;
    runtime.after_step(&mut run);
    assert_eq!(run.players[&2].avatar.as_ref().unwrap().source_id, identity);
    assert_eq!(
        run.players[&2]
            .avatar
            .as_ref()
            .unwrap()
            .animation
            .as_ref()
            .unwrap()
            .phase_offset,
        offset
    );
    // Fresh consecutive real step, then an in-step teleport-sized discontinuity.
    runtime.before_step(&mut run);
    run.step(&BTreeMap::new());
    run.players.get_mut(&2).unwrap().x += 10.0;
    runtime.after_step(&mut run);
    assert_eq!(graph_state(&run, 2), "rest");
    runtime.before_step(&mut run);
    run.step(&input);
    run.at = (run.at + 1) % run.dungeon.rooms.len();
    runtime.after_step(&mut run);
    assert_eq!(
        graph_state(&run, 2),
        "rest",
        "room transitions are not locomotion samples"
    );
    // A sample admitted for one epoch cannot animate or clear the replacement.
    runtime.before_step(&mut run);
    let mut replacement = run.players[&2].avatar.clone().unwrap();
    replacement.animation.as_mut().unwrap().phase_offset = 5;
    run.leave(2);
    run.join_seat(2, "Replacement");
    run.players.get_mut(&2).unwrap().avatar = Some(replacement.clone());
    run.step(&input);
    runtime.after_step(&mut run);
    assert_eq!(
        serde_json::to_value(run.players[&2].avatar.as_ref().unwrap()).unwrap(),
        serde_json::to_value(&replacement).unwrap()
    );
}

#[test]
fn graph_rehost_retains_owner_phase_and_does_not_invent_movement() {
    let fixture = graph_fixture();
    let mut registry = fixture.registry();
    character(&mut registry, "gold 1");
    character(&mut registry, "graph adventure 1");
    let mut sim = crate::delve::together_sim::Sim::default();
    sim.prefer_thread(false);
    let mut run = fight();
    run.enemies.clear();
    run.works.perils.clear();
    // An old footstep accumulator must not invent motion on a new server clock.
    run.players.get_mut(&1).unwrap().stride = 2.0;
    sim.set(Some(run));
    sim.set_live_characters(registry.snapshot().resources.values());
    let epoch = sim.as_ref().unwrap().seat_epoch(1);
    let avatar = sim.as_ref().unwrap().players[&1].avatar.clone().unwrap();
    for _ in 0..2 {
        sim.host(crate::delve::together_sim::Door {
            bind: "127.0.0.1:0".parse().unwrap(),
            seats: 3,
            key: Some("GRAPHKEY".into()),
            bundle: None,
        })
        .unwrap();
        assert_eq!(sim.as_ref().unwrap().seat_epoch(1), epoch);
        assert_eq!(
            serde_json::to_value(sim.as_ref().unwrap().players[&1].avatar.as_ref().unwrap())
                .unwrap(),
            serde_json::to_value(&avatar).unwrap(),
            "rehost replaced graph owner/phase before stepping"
        );
        graph_peer_play(&mut sim, None, 3, 0);
        assert_eq!(graph_state(sim.as_ref().unwrap(), 1), "rest");
        assert_eq!(
            sim.as_ref().unwrap().players[&1]
                .avatar
                .as_ref()
                .unwrap()
                .animation
                .as_ref()
                .unwrap()
                .phase_offset,
            avatar.animation.as_ref().unwrap().phase_offset
        );
    }
    sim.set(None);
}

fn v12_frame(sim: &mut crate::delve::together_sim::Sim) {
    std::thread::sleep(std::time::Duration::from_millis(35));
    let now = std::time::Instant::now();
    if let Some(seat) = sim.seat(1) {
        seat.tick(now);
    }
    sim.pump(now);
    sim.latch();
}
fn v12_sim(registry: &Registry) -> crate::delve::together_sim::Sim {
    let mut sim = crate::delve::together_sim::Sim::default();
    sim.prefer_thread(false);
    let mut run = fight();
    run.enemies.clear();
    run.works.perils.clear();
    sim.set(Some(run));
    // Selection precedes the actual Link handshake, matching real startup.
    sim.set_live_characters(registry.snapshot().resources.values());
    sim.set_controls(crate::delve::together_sim::Controls {
        active: true,
        keyboard: true,
        ..Default::default()
    });
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(10);
    loop {
        v12_frame(&mut sim);
        if sim.seat(1).is_some_and(|seat| {
            seat.status() == angel_serve::client::LinkStatus::Playing
                && seat.knight() == Some(1)
                && seat.run_arc().is_some()
        }) && sim
            .as_ref()
            .is_some_and(|run| run.active() && !run.players[&1].away)
        {
            break;
        }
        assert!(
            std::time::Instant::now() < deadline,
            "Link never entered play: {}",
            sim.system_status()
        );
    }
    sim
}
fn v12_independent(sim: &mut crate::delve::together_sim::Sim, player: u32) -> Value {
    let mut avatar = sim.as_ref().unwrap().players[&player]
        .avatar
        .clone()
        .unwrap();
    avatar.source_id = "explicit-independent-avatar".into();
    avatar.animation.as_mut().unwrap().phase_offset = 7;
    let expected = serde_json::to_value(&avatar).unwrap();
    assert!(sim.set_avatar(player, Some(avatar)));
    expected
}
fn v12_avatar(sim: &crate::delve::together_sim::Sim, player: u32) -> Value {
    serde_json::to_value(
        sim.as_ref().unwrap().players[&player]
            .avatar
            .as_ref()
            .unwrap(),
    )
    .unwrap()
}

#[test]
fn v12_same_option_reselect_reclaims_after_independent_assignment_but_plain_reload_does_not() {
    let fixture = graph_fixture();
    let mut registry = fixture.registry();
    character(&mut registry, "gold 1");
    character(&mut registry, "graph adventure 1");
    select(&mut registry, "spark 1 1");
    let stale = sources(&registry);
    let mut sim = v12_sim(&registry);
    assert_eq!(graph_state(sim.as_ref().unwrap(), 1), "rest");
    let epoch = sim.as_ref().unwrap().seat_epoch(1);
    let independent = v12_independent(&mut sim, 1);
    let path = fixture.root.join("character-graphs/index.js");
    let source = std::fs::read_to_string(&path).unwrap();
    std::fs::write(
        &path,
        source.replace("frame: 0, ticks: 12", "frame: 0, ticks: 24"),
    )
    .unwrap();
    registry.refresh(true);
    sim.set_live_characters(registry.snapshot().resources.values());
    assert_eq!(
        v12_avatar(&sim, 1),
        independent,
        "plain reload reclaimed retired cursor"
    );
    character(&mut registry, "graph adventure 1");
    sim.set_live_characters(registry.snapshot().resources.values());
    assert_eq!(graph_state(sim.as_ref().unwrap(), 1), "rest");
    assert_eq!(sim.as_ref().unwrap().seat_epoch(1), epoch);
    let reclaimed = v12_avatar(&sim, 1);
    assert_ne!(reclaimed, independent);
    // The admission revision is not part of animation content identity/phase.
    character(&mut registry, "graph adventure 1");
    sim.set_live_characters(registry.snapshot().resources.values());
    assert_eq!(v12_avatar(&sim, 1), reclaimed);
    sim.set_live_characters(stale.iter());
    assert_eq!(
        v12_avatar(&sim, 1),
        reclaimed,
        "lower intent rolled back current presentation"
    );
    assert!(
        sim.as_ref()
            .unwrap()
            .seat_notices
            .iter()
            .any(|(_, text)| text.contains("stale graph selection intent"))
    );
    sim.seat(1)
        .unwrap()
        .set_held(angel_net::input::button::RIGHT);
    let x = sim.as_ref().unwrap().players[&1].x;
    for _ in 0..4 {
        v12_frame(&mut sim);
    }
    assert!(sim.as_ref().unwrap().players[&1].x > x);
    assert_eq!(
        graph_state(sim.as_ref().unwrap(), 1),
        "walk",
        "pre-handshake graph did not survive actual playing seat"
    );
    sim.set(None);
}

#[test]
fn v12_retired_graph_does_not_skip_running_cast_cancel() {
    let fixture = graph_fixture();
    fixture.edit(&fixture.source().replace("delay: 5", "delay: 500"));
    let mut registry = fixture.registry();
    character(&mut registry, "gold 1");
    character(&mut registry, "graph adventure 1");
    select(&mut registry, "spark 1 1");
    let mut sim = v12_sim(&registry);
    sim.seat(1)
        .unwrap()
        .tap(crate::delve::together_seat::bits(&Input {
            cast: 1,
            ..Default::default()
        }));
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(10);
    while !sim.system_status().contains("started=1") {
        v12_frame(&mut sim);
        assert!(std::time::Instant::now() < deadline);
    }
    assert!(
        sim.system_status().contains("abilities: active=1 "),
        "{}",
        sim.system_status()
    );
    let independent = v12_independent(&mut sim, 1);
    select(&mut registry, "cancel 1");
    sim.set_live_characters(registry.snapshot().resources.values());
    assert!(
        sim.system_status().contains("abilities: active=0 "),
        "cancel skipped on inactive graph: {}",
        sim.system_status()
    );
    assert_eq!(v12_avatar(&sim, 1), independent);
    for _ in 0..3 {
        v12_frame(&mut sim);
    }
    assert_eq!(v12_avatar(&sim, 1), independent);
    sim.set(None);
}

#[test]
fn v12_replacement_seat_graph_tombstone_survives_off_clip_art_and_ability_changes() {
    let fixture = graph_fixture();
    let mut registry = fixture.registry();
    character(&mut registry, "gold 2");
    character(&mut registry, "graph adventure 2");
    select(&mut registry, "spark 2 1");
    let mut run = fight();
    run.join_seat(2, "First");
    let mut runtime = Runtime::default();
    runtime.set_sources(sources(&registry), &mut run);
    let mut new_avatar = run.players[&2].avatar.clone().unwrap();
    new_avatar.source_id = "replacement-owner".into();
    run.leave(2);
    run.join_seat(2, "Replacement");
    run.players.get_mut(&2).unwrap().avatar = Some(new_avatar.clone());
    runtime.before_step(&mut run);
    runtime.after_step(&mut run);
    // A genuinely new occupant can issue an explicit command, but the graph
    // contract lacks the native context needed to distinguish delayed old work.
    character(&mut registry, "graph adventure 2");
    runtime.set_sources(sources(&registry), &mut run);
    assert_eq!(
        run.players[&2].avatar.as_ref().unwrap().source_id,
        "replacement-owner"
    );
    assert!(
        run.seat_notices
            .iter()
            .any(|(_, text)| text.contains("leased current-seat context"))
    );
    select(&mut registry, "bloom 2 2");
    runtime.set_sources(sources(&registry), &mut run);
    assert_eq!(
        run.players[&2].module_spells[1].as_ref().unwrap().name,
        "Bloom",
        "graph fence blocked existing unleased ability update"
    );
    for args in [
        "graph off 2",
        "graph adventure 2",
        "animation breathe 2",
        "graph adventure 2",
        "off 2",
        "gold 2",
        "graph adventure 2",
    ] {
        character(&mut registry, args);
        runtime.set_sources(sources(&registry), &mut run);
        assert!(
            !run.players[&2]
                .avatar
                .as_ref()
                .is_some_and(|a| a.source_id.contains(':')),
            "ordinary transition erased old graph owner tombstone: {args}"
        );
    }
    // Even deleting all sources/cursors cannot erase retained graph ownership.
    runtime.set_sources(vec![], &mut run);
    character(&mut registry, "graph adventure 2");
    runtime.set_sources(sources(&registry), &mut run);
    assert!(
        !run.players[&2]
            .avatar
            .as_ref()
            .is_some_and(|a| a.source_id.contains(':'))
    );
    // Existing unleased cast/cancel still operates for the replacement actor.
    runtime.before_step(&mut run);
    run.step(&BTreeMap::from([(
        2,
        Input {
            cast: 1,
            ..Default::default()
        },
    )]));
    runtime.after_step(&mut run);
    assert!(runtime.started > 0);
    select(&mut registry, "cancel 2");
    runtime.set_sources(sources(&registry), &mut run);
    assert!(runtime.groups.values().all(|group| group.casts.is_empty()));
}

fn v12_saved_state(registry: &mut Registry) -> Value {
    let response = registry
        .dispatch(
            angel_live::kernel::Kind::Command,
            "loadout",
            &json!({"type":"command","name":"loadout","arguments":""}),
        )
        .unwrap();
    serde_json::from_str(
        response["output"]
            .as_str()
            .unwrap()
            .strip_prefix("Character loadouts: ")
            .unwrap()
            .split(" · ")
            .next()
            .unwrap(),
    )
    .unwrap()
}
#[test]
fn v12_retired_independent_avatar_survives_graphless_fallback_return_and_art_reload() {
    let fixture = graph_fixture();
    let mut registry = fixture.registry();
    character(&mut registry, "gold 1");
    character(&mut registry, "graph adventure 1");
    let state = v12_saved_state(&mut registry);
    let mut sim = v12_sim(&registry);
    let independent = v12_independent(&mut sim, 1);
    let manifest = fixture.root.join("character-graphs/module.json");
    let bytes = std::fs::read(&manifest).unwrap();
    std::fs::remove_file(&manifest).unwrap();
    let selector = fixture.root.join("abilities/index.js");
    let code = std::fs::read_to_string(&selector).unwrap();
    std::fs::write(
        fixture.root.join("abilities/v12-captured.json"),
        state.to_string(),
    )
    .unwrap();
    std::fs::write(&selector, code.replace("const initial = () => ({ choices: {}, cancelled: {}, soundBanks: {}, effectBanks: {}, artChoices: {}, animationChoices: {} })", "const initial = () => require('./v12-captured.json')")).unwrap();
    // A fresh registry models the actual graph-less pending/default publication,
    // then feeds that publication to the existing native owner (not a new owner).
    let mut fallback = fixture.registry();
    assert!(
        sources(&fallback).iter().all(|resource| resource
            .binding::<Loadout>()
            .unwrap()
            .motion
            .is_none())
    );
    sim.set_live_characters(fallback.snapshot().resources.values());
    assert_eq!(v12_avatar(&sim, 1), independent);
    let art = fixture.root.join("characters/index.js");
    let art_code = std::fs::read_to_string(&art).unwrap();
    std::fs::write(
        &art,
        art_code.replace(
            "looks.resolve(name)",
            "({...looks.resolve(name), colour:[1,2,3]})",
        ),
    )
    .unwrap();
    fallback.refresh(true);
    sim.set_live_characters(fallback.snapshot().resources.values());
    assert_eq!(
        v12_avatar(&sim, 1),
        independent,
        "ordinary art reload installed a graph-less fallback"
    );
    std::fs::write(&manifest, bytes).unwrap();
    fallback.refresh(true);
    sim.set_live_characters(fallback.snapshot().resources.values());
    assert_eq!(
        v12_avatar(&sim, 1),
        independent,
        "provider return reclaimed without selection intent"
    );
    character(&mut fallback, "graph adventure 1");
    sim.set_live_characters(fallback.snapshot().resources.values());
    assert_eq!(graph_state(sim.as_ref().unwrap(), 1), "rest");
    assert_ne!(v12_avatar(&sim, 1), independent);
    sim.set(None);
}
#[test]
fn v12_selection_intent_overflow_refuses_without_changing_working_publication() {
    let fixture = graph_fixture();
    let selector = fixture.root.join("abilities/index.js");
    let original = std::fs::read_to_string(&selector).unwrap();
    std::fs::write(fixture.root.join("abilities/v12-max.json"), json!({"choices":{},"cancelled":{},"soundBanks":{},"effectBanks":{},"artChoices":{"1":"gold"},"animationChoices":{},"graphChoices":{"1":"adventure"},"graphIntents":{"1":9007199254740991u64}}).to_string()).unwrap();
    let code = original.replace("const initial = () => ({ choices: {}, cancelled: {}, soundBanks: {}, effectBanks: {}, artChoices: {}, animationChoices: {} })", "const initial = () => require('./v12-max.json')");
    std::fs::write(&selector, &code).unwrap();
    let mut registry = fixture.registry();
    assert!(
        registry
            .snapshot()
            .statuses
            .iter()
            .all(|status| status.error.is_none()),
        "{:?}",
        registry.snapshot().statuses
    );
    let accepted = sources(&registry);
    for args in [
        "graph adventure 1",
        "graph off 1",
        "animation breathe 1",
        "off 1",
    ] {
        let reply = registry
            .dispatch(
                angel_live::kernel::Kind::Command,
                "character",
                &json!({"type":"command","name":"character","arguments":args}),
            )
            .unwrap();
        assert!(
            reply["output"]
                .as_str()
                .unwrap()
                .contains("revision exhausted")
        );
        assert_eq!(sources(&registry), accepted);
    }
    std::fs::write(
        &selector,
        code.replace(
            "graph_intent: state.graphIntents?.[player] ?? 0",
            "graph_intent: 9007199254740992",
        ),
    )
    .unwrap();
    registry.refresh(true);
    assert!(registry.snapshot().statuses.iter().any(|status| {
        status
            .error
            .as_ref()
            .is_some_and(|error| error.contains("safe-integer"))
    }));
    assert_eq!(
        sources(&registry),
        accepted,
        "unsafe numeric revision replaced last-good loadout"
    );
}

#[test]
fn v12_unseated_startup_keeps_legacy_abilities_but_graph_needs_current_seat_context() {
    let fixture = graph_fixture();
    let mut registry = fixture.registry();
    character(&mut registry, "gold 2");
    character(&mut registry, "graph adventure 2");
    select(&mut registry, "spark 2 1");
    let mut run = fight();
    assert!(!run.players.contains_key(&2));
    let mut runtime = Runtime::default();
    runtime.set_sources(sources(&registry), &mut run);
    assert!(
        runtime.sources.contains_key(&2),
        "graph pending state dropped the legacy executable source"
    );
    run.join_seat(2, "First attachment");
    runtime.before_step(&mut run);
    assert!(run.players[&2].module_spells[0].is_some());
    assert!(
        run.players[&2].avatar.is_none(),
        "pre-attachment resource silently established a graph owner"
    );
    character(&mut registry, "graph adventure 2");
    select(&mut registry, "bloom 2 2");
    runtime.set_sources(sources(&registry), &mut run);
    assert!(run.players[&2].avatar.is_none());
    assert_eq!(
        run.players[&2].module_spells[1].as_ref().unwrap().name,
        "Bloom"
    );
    assert!(
        run.seat_notices
            .iter()
            .any(|(_, text)| text.contains("leased current-seat context"))
    );
}
