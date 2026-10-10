use super::*;
use crate::drive::together_settlement::{self, planning};
use crate::stage::world_viz::World;
use serde_json::{Value, json};

struct Fixture(PathBuf);
impl Fixture {
    fn new() -> Self {
        static NEXT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
        let root = std::env::temp_dir().canonicalize().unwrap().join(format!(
            "angel-loop-settlement-planning-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
        ));
        std::fs::create_dir(&root).unwrap();
        Self(root)
    }

    fn map(&self, task: &str, door: &str, refereed: bool) {
        let task_id = format!(
            "q.angel-{}",
            &crate::knowledge::cut::sha256_hex(task.as_bytes())[..24]
        );
        std::fs::create_dir_all(self.0.join("labyrinth")).unwrap();
        let review = if refereed {
            json!({"state":"refereed","by":["independent checker"],"verdict":"ESTABLISHED"})
        } else {
            json!({"state":"under-review"})
        };
        std::fs::write(
            self.0.join("labyrinth/knowledge.json"),
            serde_json::to_vec(&json!({"schema":1,"nodes":[
                {"id":task_id,"kind":"question","status":"open","links":[{"to":"m.check","rel":"suggests"}]},
                {"id":"m.check","kind":"method","links":[]},
                {"id":door,"kind":"question","status":"open","review":review,"links":[{"to":"m.check","rel":"uses"}]}
            ]})).unwrap(),
        ).unwrap();
    }

    fn state(&self) -> LoopState {
        LoopState {
            id: "planned-loop".into(),
            workspace: Some(self.0.clone()),
            task: "measure settlement geometry".into(),
            status: LoopStatus::Paused,
            ..Default::default()
        }
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn finish(st: &mut LoopState) {
    evidence::apply_reply_with_tools(
        st,
        "DIRECTION:\nmeasure geometry\nFINDINGS:\nHYPOTHESES:\n",
        &ToolStripSnapshot::default(),
    );
}

fn site(world: &World) -> Value {
    serde_json::to_value(world.settlement_site().unwrap().unwrap()).unwrap()
}

#[test]
fn settlement_planning_persists_frontiers_before_restart_and_catches_up_from_committed_inputs() {
    let _lock = crate::tests::env_lock();
    let fixture = Fixture::new();
    let _file = crate::tests::TestEnvGuard::set(
        "ANGEL_LOOP_FILE",
        fixture.0.join("loop.json").to_str().unwrap(),
    );
    let _watchdog = crate::tests::TestEnvGuard::unset("ANGEL_LOOP_STATE_DIR");
    let mut st = fixture.state();
    fixture.map(&st.task, "q.original", false);
    for _ in 0..6 {
        finish(&mut st);
    }
    assert_eq!(st.settlement_plans.len(), st.log.len());
    let frozen = st.settlement_plans.clone();
    assert!(frozen.values().all(|input| input.door_id == "q.original"));
    save_checkpoint(&st).unwrap();
    assert_eq!(st.durable_settlement_events().unwrap().plans, frozen);

    fixture.map(&st.task, "q.changed", true);
    let current = planning::input(&fixture.0, &st.task).unwrap().unwrap();
    assert_ne!(current.map_sha256, frozen[&1].map_sha256);
    // Even an accidental second call cannot rewrite a captured iteration.
    st.snapshot_settlement_plan();
    assert_eq!(st.settlement_plans, frozen);
    save_checkpoint(&st).unwrap();
    let loaded = load_for(&fixture.0).unwrap();
    assert_eq!(loaded.settlement_plans, frozen);
    assert_eq!(loaded.durable_settlement_events().unwrap().plans, frozen);

    // A mutable live snapshot is not the confirmed restart checkpoint.
    st.settlement_plans = (1..=6).map(|n| (n, current.clone())).collect();
    let mut world = World::new(77);
    world.sync_settlement(&st, "Miner", true);
    let caught_up = site(&world);
    assert_eq!(caught_up["work"], 6);
    assert_eq!(caught_up["floor"]["rooms"][4]["kind"], "workshop");
    for receipt in caught_up["loops"]["planned-loop"].as_array().unwrap() {
        assert_eq!(receipt["plan"]["input"]["door_id"], "q.original");
        assert_eq!(
            receipt["plan"]["input"]["map_sha256"],
            frozen[&1].map_sha256
        );
    }
    let mut restarted = World::new(77);
    restarted.sync_settlement(&loaded, "Miner", true);
    assert_eq!(site(&restarted), caught_up);

    let mut loaded = loaded;
    for _ in 6..11 {
        finish(&mut loaded);
    }
    assert_eq!(loaded.settlement_plans[&11].door_id, "q.changed");
    save_checkpoint(&loaded).unwrap();
    restarted.sync_settlement(&loaded, "Miner", true);
    let next = site(&restarted);
    assert_eq!(next["work"], 11);
    assert_eq!(
        next["loops"]["planned-loop"][10]["plan"]["input"]["door_id"],
        "q.changed"
    );
    assert_eq!(
        next["loops"]["planned-loop"][10]["plan"]["kind"],
        "quarters"
    );
    assert_eq!(
        next["loops"]["planned-loop"][0],
        caught_up["loops"]["planned-loop"][0]
    );
}

#[test]
fn settlement_planning_legacy_checkpoint_is_not_relabelled_when_a_map_appears() {
    let _lock = crate::tests::env_lock();
    let fixture = Fixture::new();
    let _file = crate::tests::TestEnvGuard::set(
        "ANGEL_LOOP_FILE",
        fixture.0.join("loop.json").to_str().unwrap(),
    );
    let _watchdog = crate::tests::TestEnvGuard::unset("ANGEL_LOOP_STATE_DIR");
    let mut st = fixture.state();
    for _ in 0..5 {
        finish(&mut st);
    }
    assert!(st.settlement_plans.is_empty());
    save_checkpoint(&st).unwrap();
    fixture.map(&st.task, "q.later", true);
    // Routine checkpoint publication and load never derive historical plans.
    save_checkpoint(&st).unwrap();
    let mut loaded = load_for(&fixture.0).unwrap();
    assert!(loaded.settlement_plans.is_empty());
    assert!(loaded.durable_settlement_events().unwrap().plans.is_empty());
    let mut world = World::new(77);
    world.sync_settlement(&loaded, "Miner", false);
    let before = site(&world);
    assert_eq!(before["floor"]["rooms"][4]["kind"], "hall");
    assert!(
        before["loops"]["planned-loop"]
            .as_array()
            .unwrap()
            .iter()
            .all(|row| row.get("plan").is_none())
    );
    finish(&mut loaded);
    save_checkpoint(&loaded).unwrap();
    world.sync_settlement(&loaded, "Miner", false);
    let after = site(&world);
    assert_eq!(loaded.settlement_plans.len(), 1);
    assert_eq!(
        after["loops"]["planned-loop"][5]["plan"]["input"]["door_id"],
        "q.later"
    );
    assert_eq!(
        after["loops"]["planned-loop"][0],
        before["loops"]["planned-loop"][0]
    );
}

#[test]
fn settlement_planning_error_harvest_snapshots_without_granting_extra_work() {
    let _lock = crate::tests::env_lock();
    let fixture = Fixture::new();
    let _file = crate::tests::TestEnvGuard::set(
        "ANGEL_LOOP_FILE",
        fixture.0.join("loop.json").to_str().unwrap(),
    );
    let _watchdog = crate::tests::TestEnvGuard::unset("ANGEL_LOOP_STATE_DIR");
    let mut app = crate::seed_preview_app();
    app.loop_ctl = fixture.state();
    app.loop_ctl.max_iters = 1;
    app.loop_ctl.operator_caps = true;
    fixture.map(&app.loop_ctl.task, "q.error-frontier", false);
    app.loop_harvest_error_with_tools("test worker failed".into(), ToolStripSnapshot::default());
    assert_eq!(app.loop_ctl.log.len(), 1);
    assert_eq!(app.loop_ctl.settlement_plans.len(), 1);
    let durable = app.loop_ctl.durable_settlement_events().unwrap();
    assert_eq!(durable.rows.len(), 1);
    assert_eq!(durable.plans[&1].door_id, "q.error-frontier");
    let mut world = World::new(77);
    world.sync_settlement(&app.loop_ctl, "Miner", false);
    assert_eq!(site(&world)["work"], 1);
    let before = site(&world);
    world.sync_settlement(&app.loop_ctl, "Miner", false);
    assert_eq!(site(&world), before);
}

#[test]
fn settlement_planning_unavailable_map_and_invalid_provenance_cannot_expand_authority() {
    let _lock = crate::tests::env_lock();
    let fixture = Fixture::new();
    let mut st = fixture.state();
    fixture.map(&st.task, "q.valid", false);
    finish(&mut st);
    let valid = st.settlement_plans[&1].clone();
    fixture.map(&st.task, "q.next", false);
    finish(&mut st);
    st.settlement_plans.insert(99, valid.clone());
    st.settlement_plans.get_mut(&1).unwrap().map_sha256.clear();
    st.confirm_settlement_checkpoint_for_test();
    let durable = st.durable_settlement_events().unwrap();
    assert_eq!(durable.rows.len(), 2);
    assert_eq!(durable.plans.keys().copied().collect::<Vec<_>>(), vec![2]);

    std::fs::write(fixture.0.join("labyrinth/knowledge.json"), b"invalid map").unwrap();
    finish(&mut st);
    assert_eq!(st.iteration, 3);
    assert_eq!(st.log.len(), 3);
    assert!(!st.settlement_plans.contains_key(&3));
    assert!(!st.settlement_plans.contains_key(&99));
    assert!(!st.settlement_plans.contains_key(&1));
    let mut store = together_settlement::Store::beside(None, 77, "Miner").unwrap();
    // The third in-memory row is uncommitted; only the confirmed two rows work.
    store.sync_with_plans(&st, false, &durable.plans).unwrap();
    assert_eq!(store.site.work, 2);
}
