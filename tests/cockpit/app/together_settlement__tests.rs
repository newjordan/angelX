use super::*;
pub(super) struct Temp(std::path::PathBuf);
impl Temp {
    pub(super) fn new() -> Self {
        static SERIAL: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
        let path = std::env::temp_dir().canonicalize().unwrap().join(format!(
            "angel-settlement-{}-{}",
            std::process::id(),
            SERIAL.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
        ));
        std::fs::create_dir(&path).unwrap();
        Self(path)
    }
    pub(super) fn path(&self) -> &std::path::Path {
        &self.0
    }
}
impl Drop for Temp {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

use crate::{
    drive::together_shooter::{Run, home::Home},
    stage::world_viz::crawl::{dungeon::Dungeon, party},
};

pub(super) fn facts(id: &str, iterations: &[usize]) -> LoopState {
    let mut st = LoopState {
        id: id.into(),
        iteration: iterations.last().copied().unwrap_or(200),
        ..LoopState::default()
    };
    for &iteration in iterations {
        st.log.push(crate::drive::loop_ctl::LoopIterLog {
            iteration,
            direction: String::new(),
            new_findings: 0,
            reported_findings: 0,
            unverified_findings: 0,
            tool_calls: 1,
            tool_errors: 0,
            duplicate_costly_actions: 0,
            outcome_progress: 0,
            novel_outcome_actions: 0,
            verified_outcome_actions: 0,
            workspace_changed: true,
            evidence_review: false,
            stale_count: 0,
            ts_ms: 0,
        });
    }
    st.workspace = Some(std::env::temp_dir());
    st.confirm_settlement_checkpoint_for_test();
    st
}

pub(super) fn value(site: &Site) -> serde_json::Value {
    serde_json::to_value(site).unwrap()
}

#[test]
fn settlement_starts_in_player_hall_and_never_infers_missing_iterations() {
    let mut store = Store::beside(None, 77, "Miner").unwrap();
    let hall = layout::undercroft(Pack::Cavern);
    assert_eq!(
        value(&store.site)["floor"],
        serde_json::to_value(&hall).unwrap()
    );
    assert_eq!(store.site.focus(), 0);
    assert_eq!(store.site.floor.rooms[0].kind, RoomKind::Home);
    assert!(store.sync(&facts("loop-A", &[]), false).unwrap());
    assert_eq!(store.site.work, 0, "iteration 200 is not 200 receipts");
    assert_eq!(
        store.site.loops.len(),
        1,
        "even an empty loop is associated"
    );
    assert!(store.sync(&facts("loop-A", &[200]), false).unwrap());
    assert_eq!(store.site.work, 1);
    assert_eq!(store.site.floor.rooms.len(), 5);
}

#[test]
fn settlement_expands_reachable_rooms_with_shared_vocabulary() {
    let mut site = Site::new(77, "Miner");
    for n in 1..=30 {
        site.apply("loop-A", n, true).unwrap();
        let d = Dungeon::of(&site.floor);
        let start = party::spot(&d, 0);
        for (i, room) in site.floor.rooms.iter().enumerate() {
            assert!(room.roster.is_empty());
            assert!(
                d.path(start, party::spot(&d, i)).is_some(),
                "room {i} unreachable at receipt {n}"
            );
            for dir in 0..4 {
                if room.doors[dir] {
                    let next = site.floor.neighbour(i, dir).expect("door has neighbour");
                    assert_eq!(site.floor.neighbour(next, (dir + 2) % 4), Some(i));
                }
            }
        }
        let focus = party::spot(&d, site.focus());
        assert!(d.walkable((focus.0 + 3, focus.1)), "camera on solid rock");
        assert!(site.valid(&Site::new(77, "Miner")));
    }
    assert_eq!(site.built, 6);
    for (i, &(_, _, kind)) in ROOMS.iter().enumerate() {
        assert_eq!(site.floor.rooms[i + 4].kind, kind);
    }
    assert_eq!(site.floor.rooms[0].kind, RoomKind::Home);
}

#[test]
fn settlement_receipts_are_idempotent_across_resume_reorder_and_loops() {
    let mut site = Site::new(77, "Miner");
    assert!(site.apply("A", 7, false).unwrap());
    let snapshot = value(&site);
    assert!(
        !site.apply("A", 7, true).unwrap(),
        "later flags cannot recraft old receipt"
    );
    assert_eq!(value(&site), snapshot);
    site.apply("A", 3, true).unwrap();
    site.apply("B", 7, true).unwrap();
    assert_eq!(
        site.work, 3,
        "only receipts, not high-water iteration delta"
    );
    assert!(site.valid(&Site::new(77, "Miner")));
    let mut resumed: Site = serde_json::from_value(value(&site)).unwrap();
    assert!(!resumed.apply("B", 7, true).unwrap());
    assert_eq!(value(&site), value(&resumed));
    assert!(resumed.apply("B", 8, true).unwrap());
    assert_eq!(site.id, resumed.id);
    assert_ne!(site.id, Site::new(78, "Miner").id);
    assert_ne!(site.id, Site::new(77, "Other").id);
}

#[test]
fn settlement_crafting_and_building_conserve_mined_resources() {
    let mut site = Site::new(77, "Miner");
    assert!(!site.pay(1, 0));
    site.craft();
    assert_eq!(site.resources, Resources::default());
    for n in 1..=14 {
        site.apply("A", n, true).unwrap();
    }
    assert_eq!(site.built, 3);
    let before = site.resources.clone();
    site.apply("A", 15, true).unwrap();
    assert_eq!(site.resources.tools, 1);
    assert_eq!(before.stone - site.resources.stone, 2);
    assert_eq!(before.ore - site.resources.ore, 3);
    for n in 16..=30 {
        site.apply("A", n, false).unwrap();
    }
    assert_eq!(site.resources.tools, 1, "unchanged iterations cannot craft");
    for n in 31..=300 {
        site.apply("A", n, true).unwrap();
    }
    let r = &site.resources;
    assert_eq!(r.stone + r.spent_stone, r.mined_stone);
    assert_eq!(r.ore + r.spent_ore, r.mined_ore);
    assert_eq!(r.spent_stone, 12 * 6 + 2 * r.tools);
    assert_eq!(r.spent_ore, 2 * 6 + 3 * r.tools);
    assert!(r.ore < 3, "no infinite crafting once ore is spent");
    assert_eq!(
        site.floor.rooms.len(),
        10,
        "bounded slice never invents more rock"
    );
    assert!(site.valid(&Site::new(77, "Miner")));
}

#[test]
fn settlement_disk_resume_and_interleaved_writers_share_one_site() {
    let dir = Temp::new();
    let rewards = dir.path().join("island.json");
    let realm_path = rewards.with_extension("realm.json");
    let run_path = dir.path().join("dungeon.json");
    std::fs::write(&realm_path, b"existing treasury").unwrap();
    std::fs::write(&run_path, b"existing playable checkpoint").unwrap();
    let mut a = Store::beside(Some(&rewards), 77, "Miner").unwrap();
    let mut b = Store::beside(Some(&rewards), 77, "Miner").unwrap();
    a.sync(&facts("A", &[1, 2]), false).unwrap();
    b.sync(&facts("B", &[1]), true).unwrap();
    a.sync(&facts("A", &[1, 2, 3]), false).unwrap();
    let resumed = Store::beside(Some(&rewards), 77, "Miner").unwrap();
    assert_eq!(resumed.site.work, 4);
    assert_eq!(resumed.site.id, a.site.id);
    assert_eq!(resumed.site.loops.len(), 2);
    assert_eq!(
        resumed
            .site
            .floor
            .rooms
            .iter()
            .filter(|r| r.kind == RoomKind::Tavern)
            .count(),
        1
    );
    assert_eq!(std::fs::read(&realm_path).unwrap(), b"existing treasury");
    assert_eq!(
        std::fs::read(&run_path).unwrap(),
        b"existing playable checkpoint"
    );
    b.refresh().unwrap();
    assert_eq!(value(&b.site), value(&resumed.site));
}

#[test]
fn settlement_bad_save_and_failed_transaction_never_reset_or_overwrite() {
    let dir = Temp::new();
    let mut store = Store::beside(Some(&dir.path().join("island.json")), 77, "Miner").unwrap();
    store.sync(&facts("A", &[1]), false).unwrap();
    let path = store.path.clone().unwrap();
    let original = std::fs::read(&path).unwrap();
    // Existing temp is not removed; a failed save publishes no state in memory.
    let temp = path.with_extension(format!("{}.next", std::process::id()));
    std::fs::write(&temp, b"stale owned temp").unwrap();
    assert!(store.sync(&facts("A", &[1, 2]), false).is_err());
    assert_eq!(store.site.work, 1);
    assert_eq!(std::fs::read(&path).unwrap(), original);
    assert_eq!(std::fs::read(&temp).unwrap(), b"stale owned temp");
    std::fs::remove_file(&temp).unwrap();
    assert!(store.sync(&facts("A", &[1, 2]), false).unwrap());
    assert_eq!(store.site.work, 2);
    std::fs::write(&path, b"broken save").unwrap();
    assert!(Store::beside(Some(&dir.path().join("island.json")), 77, "Miner").is_err());
    assert!(store.sync(&facts("A", &[1, 2, 3]), false).is_err());
    assert_eq!(std::fs::read(&path).unwrap(), b"broken save");
    assert_eq!(store.site.work, 2);
    std::fs::remove_file(&path).unwrap();
    assert!(
        store.sync(&facts("A", &[1, 2, 3]), false).is_err(),
        "missing established ledger is not reset"
    );
}

#[test]
fn settlement_replay_rejects_tampered_resources_maps_and_receipts() {
    let mut site = Site::new(77, "Miner");
    site.apply("A", 1, true).unwrap();
    let expected = Site::new(77, "Miner");
    let mut bad = site.clone();
    bad.resources.stone += 1;
    assert!(!bad.valid(&expected));
    let mut bad = site.clone();
    bad.floor.rooms[0].set(1, 1, Tile::Hazard);
    assert!(!bad.valid(&expected));
    let mut bad = site.clone();
    bad.loops.get_mut("A").unwrap().push(Receipt {
        iteration: 1,
        changed: true,
        sequence: 1,
        plan: None,
    });
    assert!(!bad.valid(&expected));
    let mut bad = site.clone();
    bad.version += 1;
    assert!(!bad.valid(&expected));
    assert!(!site.valid(&Site::new(77, "Other")));
    assert!(!site.valid(&Site::new(78, "Miner")));
}

#[test]
fn settlement_passive_and_playable_share_saved_floor_not_just_generator() {
    let dir = Temp::new();
    let rewards = dir.path().join("island.json");
    let mut store = Store::beside(Some(&rewards), 77, "Miner").unwrap();
    store
        .sync(&facts("A", &(1..=14).collect::<Vec<_>>()), true)
        .unwrap();
    let saved = Store::beside(Some(&rewards), 77, "Miner").unwrap();
    let mut run = Run::at_home(77, 3, None, Home::default(), Default::default());
    run.enter_settlement(&saved.site).unwrap();
    assert_eq!(run.settlement_site.as_deref(), Some(saved.site.id.as_str()));
    assert_eq!(run.players[&1].name, "Miner");
    assert_eq!(
        serde_json::to_value(crate::drive::together_shooter::world::below_the_world(
            &run.dungeon
        ))
        .unwrap(),
        value(&saved.site)["floor"]
    );
    assert_eq!(run.at, 0);
    assert!(run.valid_snapshot());
    let passive = Dungeon::of(&saved.site.floor);
    let playable = Dungeon::of(&crate::drive::together_shooter::world::below_the_world(
        &run.dungeon,
    ));
    assert_eq!(
        (passive.grid.w, passive.grid.h),
        (playable.grid.w, playable.grid.h)
    );
    for y in 0..passive.grid.h as i32 {
        for x in 0..passive.grid.w as i32 {
            assert_eq!(passive.grid.at(x, y), playable.grid.at(x, y));
        }
    }
    store
        .sync(&facts("A", &(1..=20).collect::<Vec<_>>()), true)
        .unwrap();
    assert_eq!(
        crate::drive::together_shooter::world::below_the_world(&run.dungeon)
            .rooms
            .len(),
        saved.site.floor.rooms.len(),
        "live map is not updated by loop work"
    );
    assert!(
        run.enter_settlement(&store.site).is_err(),
        "no readmission can overwrite an active snapshot"
    );
    let checkpoint = serde_json::to_vec(&run).unwrap();
    let resumed: Run = serde_json::from_slice(&checkpoint).unwrap();
    assert_eq!(resumed.settlement_site, run.settlement_site);
    assert_eq!(
        serde_json::to_value(crate::drive::together_shooter::world::below_the_world(
            &resumed.dungeon
        ))
        .unwrap(),
        value(&saved.site)["floor"]
    );
    for i in 4..run.dungeon.rooms.len() {
        if run.dungeon.rooms[i].kind == RoomKind::Tavern {
            continue;
        }
        run.enter_for_test(i);
        assert!(run.enemies.is_empty(), "no routine settlement fights");
    }
    run.enter_for_test(0);
    run.descend_for_test();
    assert_eq!(run.floor(), 1, "the existing Delve route still works");
}

#[test]
fn settlement_rendering_time_never_awards_work_and_motion_off_is_stable() {
    use crate::{
        stage::world_viz::{World, crawl::settlement::Scene},
        ui::viz::lifecycle_viz::MotionMode,
    };
    let mut world = World::new(77);
    world.sync_settlement(&facts("A", &[]), "Miner", false);
    let before = value(world.settlement_site().unwrap().unwrap());
    for elapsed in [0, 40, 1200, 3600, 12000, u32::MAX] {
        let mut clock = world.crawl_clock(MotionMode::Full);
        clock.elapsed = elapsed;
        let img = world.crawl_frame(&clock, (144, 80));
        assert_eq!(img.dimensions(), (144, 80));
        assert_eq!(value(world.settlement_site().unwrap().unwrap()), before);
    }
    assert_eq!(
        world.crawl_log(MotionMode::Off)[0],
        "Miner · PLAYER HALL · miners muster at the west wall"
    );
    let clock = world.crawl_clock(MotionMode::Off);
    assert_eq!(
        world.crawl_frame(&clock, (144, 80)),
        world.crawl_frame(&clock, (144, 80))
    );
    world.sync_settlement(&facts("A", &[1, 2, 3, 4]), "Miner", false);
    let site = world.settlement_site().unwrap().unwrap();
    let before = value(site);
    let scene = Scene::of(site);
    for elapsed in [0, 400, 3600, 12000] {
        scene.frame(elapsed, (144, 80));
    }
    assert_eq!(value(world.settlement_site().unwrap().unwrap()), before);
}

#[test]
fn settlement_entry_route_uses_host_identity_and_does_not_replace_a_live_run() {
    use crate::app::App;
    let mut app = App::preview(crate::ui::viewer::Viewer::static_preview());
    let player = app.host_name();
    app.loop_ctl = facts("A", &[1, 2, 3, 4, 5, 6, 7, 8, 9, 10]);
    app.adventure_mirror_drain();
    let site = app.world.settlement_site().unwrap().unwrap().clone();
    assert_eq!(site.player, player);
    assert!(
        app.dungeon_command(Some("settlement"))
            .contains("PLAYER HALL")
    );
    let run = app.dungeon.shooter.as_ref().unwrap();
    assert_eq!(run.settlement_site.as_deref(), Some(site.id.as_str()));
    assert_eq!(
        serde_json::to_value(crate::drive::together_shooter::world::below_the_world(
            &run.dungeon
        ))
        .unwrap(),
        value(&site)["floor"]
    );
    let before = serde_json::to_value(run).unwrap();
    app.loop_ctl = facts("A", &(1..=20).collect::<Vec<_>>());
    app.adventure_mirror_drain();
    assert_eq!(
        serde_json::to_value(app.dungeon.shooter.as_ref().unwrap()).unwrap(),
        before
    );
    assert!(
        app.dungeon_command(Some("settlement"))
            .contains("map is unchanged")
    );
    assert_eq!(
        serde_json::to_value(app.dungeon.shooter.as_ref().unwrap()).unwrap(),
        before
    );
}

#[test]
fn settlement_different_rooms_have_visible_shared_furniture() {
    use crate::stage::world_viz::crawl::{dungeon::Prop, settlement::Scene};
    let mut site = Site::new(77, "Miner");
    let mut shots = Vec::new();
    for n in 1..=19 {
        site.apply("visual", n, true).unwrap();
        if [9, 14, 19].contains(&n) {
            let dungeon = Dungeon::of(&site.floor);
            let prop = match n {
                9 => Prop::Crate,
                14 => Prop::Workbench,
                _ => Prop::Bed,
            };
            assert!(dungeon.props.iter().any(|p| p.2 == prop));
            shots.push(Scene::of(&site).frame(3600, (288, 160)));
        }
    }
    assert_ne!(
        shots[0], shots[1],
        "stockpile and workshop must be visibly distinct"
    );
    assert_ne!(
        shots[1], shots[2],
        "workshop and quarters must be visibly distinct"
    );
}

#[test]
fn settlement_busy_lock_is_retryable_without_losing_a_receipt() {
    let dir = Temp::new();
    let mut store = Store::beside(Some(&dir.path().join("island.json")), 77, "Miner").unwrap();
    store.sync(&facts("A", &[1]), false).unwrap();
    let lock = std::fs::OpenOptions::new()
        .read(true)
        .write(true)
        .open(store.path.as_ref().unwrap().with_extension("lock"))
        .unwrap();
    lock.lock().unwrap();
    assert!(
        store
            .sync(&facts("A", &[1, 2]), false)
            .unwrap_err()
            .contains("busy")
    );
    assert_eq!(store.site.work, 1);
    drop(lock);
    assert!(store.sync(&facts("A", &[1, 2]), false).unwrap());
    assert_eq!(store.site.work, 2);
}

#[test]
fn settlement_entry_requires_a_real_loop_association_even_before_work() {
    let mut app = crate::app::App::preview(crate::ui::viewer::Viewer::static_preview());
    assert!(
        app.dungeon_command(Some("settlement"))
            .contains("Start a loop first")
    );
    assert!(app.dungeon.shooter.is_none());
    app.loop_ctl = facts("real-empty-loop", &[]);
    app.adventure_mirror_drain();
    let site = app.world.settlement_site().unwrap().unwrap().clone();
    assert!(site.associated());
    assert_eq!(site.work, 0);
    assert!(
        app.dungeon_command(Some("settlement"))
            .contains("PLAYER HALL")
    );
    assert_eq!(
        app.dungeon
            .shooter
            .as_ref()
            .unwrap()
            .settlement_site
            .as_deref(),
        Some(site.id.as_str())
    );
}

#[test]
fn settlement_measurement_and_stall_ceremonies_cannot_mint_resources() {
    use crate::{
        stage::world_viz::{AdventureEvent, World},
        ui::viz::lifecycle_viz::MotionMode,
    };
    let mut world = World::new(77);
    world.sync_settlement(&facts("A", &[1]), "Miner", false);
    let before = value(world.settlement_site().unwrap().unwrap());
    world.note_adventure(AdventureEvent::LoopStarted {
        kind: crate::stage::world_viz::LoopKind::Coding,
        task: "build".into(),
    });
    world.note_adventure(AdventureEvent::MeasurementObserved);
    assert!(world.crawl_log(MotionMode::Full)[0].contains("chest"));
    let mut clock = world.crawl_clock(MotionMode::Full);
    clock.elapsed = 3600;
    clock.found = None;
    let calm = world.crawl_frame(&clock, (144, 80));
    clock.danger = 2;
    let dark = world.crawl_frame(&clock, (144, 80));
    assert_ne!(calm, dark);
    clock.danger = 0;
    clock.found = Some(200);
    assert_ne!(world.crawl_frame(&clock, (144, 80)), calm);
    assert_eq!(value(world.settlement_site().unwrap().unwrap()), before);
}

#[test]
fn settlement_playable_knight_can_walk_out_of_the_hall_into_the_first_cut() {
    use crate::drive::together_shooter::{Input, TILE_UNITS};
    let mut site = Site::new(77, "Miner");
    site.apply("A", 1, true).unwrap();
    let mut run = Run::at_home(77, 1, None, Home::default(), Default::default());
    run.enter_settlement(&site).unwrap();
    let hero = run.players.get_mut(&1).unwrap();
    // Approach the west wall from inside the hall, crossing the actual
    // renovated chapel screen. Run::step performs real collision and exits.
    hero.x = 4.5 * TILE_UNITS;
    hero.y = (ROWS / 2) as f32 * TILE_UNITS;
    let input = BTreeMap::from([(
        1,
        Input {
            move_x: -1,
            ..Default::default()
        },
    )]);
    for _ in 0..200 {
        run.step(&input);
        if run.at != 0 {
            break;
        }
    }
    assert_eq!(
        run.at, 4,
        "actual movement must cross the hall's access cut"
    );
    assert!(run.enemies.is_empty());
    assert!(
        run.bank.is_empty(),
        "walking into safe rooms cannot mint Delve spoils"
    );
    let before = serde_json::to_value(&run.dungeon).unwrap();
    for _ in 0..600 {
        run.step(&BTreeMap::new());
    }
    assert_eq!(serde_json::to_value(&run.dungeon).unwrap(), before);
    assert_eq!(
        site.work, 1,
        "a thousand playable ticks are not excavation receipts"
    );
}

#[test]
fn settlement_saved_site_can_be_admitted_with_no_active_loop() {
    let dir = Temp::new();
    let rewards = dir.path().join("island.json");
    let mut app = crate::app::App::preview(crate::ui::viewer::Viewer::static_preview());
    let seed = app.world.realm_seed();
    let player = app.host_name();
    let mut store = Store::beside(Some(&rewards), seed, &player).unwrap();
    store
        .sync(&facts("finished-loop", &[1, 2, 3, 4]), false)
        .unwrap();
    // Test production admission, not the in-memory generator route. A fresh
    // loaded Store is installed in this disk-free App preview at the narrow
    // World's test seam, then /dungeon start does its ordinary refresh/admit.
    let saved = Store::beside(Some(&rewards), seed, &player).unwrap();
    app.world.settlement_restore_for_test(saved);
    assert!(app.loop_ctl.id.is_empty());
    app.dungeon_command(Some("start"));
    let run = app.dungeon.shooter.as_ref().unwrap();
    assert_eq!(run.settlement_site.as_deref(), Some(store.site.id.as_str()));
    assert_eq!(
        serde_json::to_value(crate::drive::together_shooter::world::below_the_world(
            &run.dungeon
        ))
        .unwrap(),
        value(&store.site)["floor"]
    );
    assert_eq!(run.players[&1].name, player);
}

#[test]
fn settlement_paid_realm_wing_is_not_lost_on_admission_between_loops() {
    use crate::drive::together_shooter::home::Station;
    let dir = Temp::new();
    let rewards = dir.path().join("island.json");
    let mut app = crate::app::App::preview(crate::ui::viewer::Viewer::static_preview());
    let player = app.host_name();
    let mut store = Store::beside(Some(&rewards), app.world.realm_seed(), &player).unwrap();
    store.sync(&facts("completed-loop", &[1]), false).unwrap();
    let resources = store.site.resources.clone();
    app.world.settlement_restore_for_test(store);
    // Existing realm construction, not a loop reward or free purchase.
    app.realm().home.levels.insert(Station::Wing, 1);
    assert!(app.loop_ctl.id.is_empty());
    app.dungeon_command(Some("start"));
    let run = app.dungeon.shooter.as_ref().unwrap();
    assert!(run.dungeon.rooms.iter().any(|r| r.kind == RoomKind::Tavern));
    let saved = Store::beside(Some(&rewards), app.world.realm_seed(), &player).unwrap();
    assert!(saved.site.tavern);
    assert_eq!(saved.site.work, 1);
    assert_eq!(saved.site.resources, resources);
    assert_eq!(saved.site.loops.len(), 1, "no invented loop identity");
}

#[test]
fn settlement_failed_refresh_cannot_admit_a_cached_site_through_noop_sync() {
    let dir = Temp::new();
    let rewards = dir.path().join("island.json");
    let mut app = crate::app::App::preview(crate::ui::viewer::Viewer::static_preview());
    let player = app.host_name();
    let mut store = Store::beside(Some(&rewards), app.world.realm_seed(), &player).unwrap();
    store.sync(&facts("completed-loop", &[1]), false).unwrap();
    let site_id = store.site.id.clone();
    let path = store.path.clone().unwrap();
    let original = std::fs::read(&path).unwrap();
    app.world.settlement_restore_for_test(store);
    assert!(app.loop_ctl.id.is_empty());
    std::fs::write(&path, b"broken save").unwrap();

    assert!(
        app.dungeon_command(Some("settlement"))
            .contains("Settlement not opened")
    );
    assert!(app.dungeon.shooter.is_none());
    assert!(app.world.settlement_site().is_err());
    // The ordinary entry can fall back to its standard home, but must not
    // advertise success or admit cached settlement tiles after a disk failure.
    app.dungeon_command(Some("start"));
    assert!(
        app.dungeon
            .shooter
            .as_ref()
            .unwrap()
            .settlement_site
            .is_none()
    );
    assert!(app.dungeon.notice.contains("Settlement unavailable"));
    assert!(app.world.settlement_site().is_err());
    assert_eq!(std::fs::read(&path).unwrap(), b"broken save");

    app.dungeon_command(Some("off"));
    std::fs::write(&path, original).unwrap();
    assert!(
        app.dungeon_command(Some("settlement"))
            .contains("PLAYER HALL")
    );
    assert_eq!(
        app.dungeon
            .shooter
            .as_ref()
            .unwrap()
            .settlement_site
            .as_deref(),
        Some(site_id.as_str())
    );
}
