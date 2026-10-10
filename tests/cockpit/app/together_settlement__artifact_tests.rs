use super::tests::{Temp, facts, value};
use super::*;
use crate::drive::{
    loop_ctl::{self, LoopStatus},
    together_shooter::{Run, TILE_UNITS, home::Home},
};
use crate::stage::world_viz::{
    crawl::{dungeon::Dungeon, party, settlement::Scene},
    overworld::arena,
};
use serde_json::json;

struct Fixture {
    dir: Temp,
    store: Store,
    st: LoopState,
}
impl Fixture {
    fn new() -> Self {
        let dir = Temp::new();
        std::fs::create_dir(dir.path().join("reports")).unwrap();
        std::fs::create_dir(dir.path().join(".angelX")).unwrap();
        let mut st = facts("real-research-loop", &(1..=14).collect::<Vec<_>>());
        st.workspace = Some(dir.path().to_path_buf());
        st.confirm_settlement_checkpoint_for_test();
        let mut store = Store::beside(Some(&dir.path().join("island.json")), 77, "Miner").unwrap();
        store.sync(&st, false).unwrap();
        std::fs::write(dir.path().join("reports/result.txt"), b"Measured result: 37 samples; median 12 ms. Baseline 15 ms.\nUncertainty: local fixture only, not official evaluation.\n").unwrap();
        let fixture = Self { dir, store, st };
        fixture.manifest(json!([{"key":"median-v1", "title":"PRIVATE measured latency", "source":"reports/result.txt", "status":"reported"}]));
        fixture
    }
    fn manifest(&self, artifacts: serde_json::Value) {
        self.write(json!({"schema":"angel.settlement-exhibits/v1", "site_id":self.store.site.id, "loop_id":self.st.id, "iteration":14, "artifacts":artifacts}));
    }
    fn write(&self, manifest: serde_json::Value) {
        std::fs::write(
            self.dir.path().join(".angelX/settlement-exhibits.json"),
            serde_json::to_vec(&manifest).unwrap(),
        )
        .unwrap();
    }
    fn named(&self, name: &str, iteration: usize, artifacts: serde_json::Value) {
        self.write_named(
            name,
            json!({"schema":"angel.settlement-exhibits/v1", "site_id":self.store.site.id,
            "loop_id":self.st.id, "iteration":iteration, "artifacts":artifacts}),
        );
    }
    fn write_named(&self, name: &str, manifest: serde_json::Value) {
        let path = self
            .dir
            .path()
            .join(exhibits::manifest_path(Some(name)).unwrap());
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, serde_json::to_vec(&manifest).unwrap()).unwrap();
    }
    fn deposit_named(&mut self, name: &str) -> Result<usize, String> {
        self.store.deposit(&self.st, self.dir.path(), Some(name))
    }
    fn disk(&self) -> Vec<u8> {
        std::fs::read(self.store.path.as_ref().unwrap()).unwrap()
    }
    fn deposit(&mut self) -> Result<usize, String> {
        self.store.deposit(&self.st, self.dir.path(), None)
    }
    fn run(&self) -> Run {
        let mut run = Run::at_home(77, 91, None, Home::default(), Default::default());
        run.enter_settlement(&self.store.site).unwrap();
        run
    }
    fn approach(&self, run: &mut Run) {
        let m = &self.store.site.exhibits[0].marker;
        run.at = m.room;
        let hero = run.players.get_mut(&1).unwrap();
        hero.x = (m.stand.0 as f32 + 0.5) * TILE_UNITS;
        hero.y = (m.stand.1 as f32 + 0.5) * TILE_UNITS;
    }
}

#[test]
fn settlement_durable_receipts_require_canonical_save_not_watchdog_cursor() {
    let _env = crate::tests::env_lock();
    let dir = Temp::new();
    let path = dir.path().join("loop.json");
    let _file = crate::tests::TestEnvGuard::set("ANGEL_LOOP_FILE", path.to_str().unwrap());
    let _mirror = crate::tests::TestEnvGuard::set("ANGEL_LOOP_STATE_DIR", "");
    let mut st = facts("durable", &[1]);
    st.workspace = Some(dir.path().to_path_buf());
    st.settlement_checkpoint.borrow_mut().take();
    st.persisted_log.set(st.log.len()); // Watchdog does NOT establish durability.
    let mut store = Store::beside(Some(&dir.path().join("island.json")), 77, "Miner").unwrap();
    assert!(!store.sync(&st, false).unwrap());
    assert!(!store.site.associated());
    loop_ctl::save(&st); // Idle writes nothing.
    assert!(!path.exists());
    assert!(!store.sync(&st, false).unwrap());
    st.status = LoopStatus::Stopped;
    std::fs::write(&path, b"corrupt checkpoint").unwrap();
    loop_ctl::save(&st);
    assert!(st.durable_settlement_events().is_none());
    assert!(!store.sync(&st, false).unwrap());
    std::fs::remove_file(&path).unwrap();
    loop_ctl::save(&st);
    assert_eq!(st.persisted_log.get(), 1);
    assert!(store.sync(&st, false).unwrap());
    assert_eq!(store.site.work, 1);
    let mut resumed = loop_ctl::load_for(dir.path()).unwrap();
    assert!(!store.sync(&resumed, false).unwrap());
    let new = facts("durable", &[2]).log.remove(0);
    resumed.log.push(new.clone());
    // Save failure cannot credit the new row (keep old confirmed snapshot).
    std::fs::remove_file(&path).unwrap();
    std::fs::create_dir(&path).unwrap();
    loop_ctl::save(&resumed);
    assert!(!store.sync(&resumed, false).unwrap());
    assert_eq!(store.site.work, 1);
    std::fs::remove_dir(&path).unwrap();
    loop_ctl::save(&resumed);
    assert!(store.sync(&resumed, false).unwrap());
    assert_eq!(store.site.work, 2);
    // Truncate the history: preserve old site receipts, do NOT use length/high-water.
    resumed.log = vec![new];
    loop_ctl::save(&resumed);
    assert!(!store.sync(&resumed, false).unwrap());
    let mut next = facts("durable", &[8]);
    next.workspace = resumed.workspace.clone();
    next.status = LoopStatus::Stopped;
    next.settlement_checkpoint.borrow_mut().take();
    loop_ctl::save(&next);
    assert!(store.sync(&next, false).unwrap());
    assert_eq!(
        store.site.work, 3,
        "only persisted rows 1,2,8, not iteration gaps"
    );
    let restored = loop_ctl::load_for(dir.path()).unwrap();
    assert!(!store.sync(&restored, false).unwrap());
    assert_eq!(store.site.work, 3);
    next.id = "unsaved-new-loop".into();
    assert!(!store.sync(&next, false).unwrap());
    assert!(!store.site.loops.contains_key("unsaved-new-loop"));
    // Deserializing model/file JSON is not by itself persistence authority.
    let unconfirmed: LoopState =
        serde_json::from_value(serde_json::to_value(&restored).unwrap()).unwrap();
    assert!(unconfirmed.durable_settlement_events().is_none());
}

#[test]
fn settlement_exhibits_deposit_duplicate_reload_and_source_inspection() {
    let mut f = Fixture::new();
    let before = f.store.site.resources.clone();
    let work = f.store.site.work;
    let old_run = f.run();
    assert_eq!(f.deposit().unwrap(), 1);
    assert_eq!(f.store.site.resources, before);
    assert_eq!(f.store.site.work, work);
    assert_eq!(
        f.store.site.exhibits[0].marker.room, 5,
        "real stockpile room, not loot"
    );
    assert_eq!(f.deposit().unwrap(), 0);
    let bytes = std::fs::read(f.store.path.as_ref().unwrap()).unwrap();
    assert_eq!(f.deposit().unwrap(), 0);
    assert_eq!(
        std::fs::read(f.store.path.as_ref().unwrap()).unwrap(),
        bytes
    );
    assert!(
        old_run.settlement_exhibits.is_empty(),
        "live run not silently replaced"
    );
    assert!(f.store.site.inspect(&old_run, f.dir.path()).is_err());
    f.store = Store::beside(Some(&f.dir.path().join("island.json")), 77, "Miner").unwrap();
    let mut run = f.run();
    assert!(run.valid_snapshot());
    assert!(
        f.store.site.inspect(&run, f.dir.path()).is_err(),
        "must walk to stand"
    );
    f.approach(&mut run);
    let text = f.store.site.inspect(&run, f.dir.path()).unwrap();
    assert!(text.contains("37 samples"));
    assert!(text.contains("reports/result.txt"));
    assert!(text.contains("NOT verifier-confirmed"));
    assert!(text.contains("real-research-loop"));
    assert!(
        text.contains(
            value(&f.store.site)["exhibits"][0]["sha256"]
                .as_str()
                .unwrap()
        )
    );
    assert!(
        f.store
            .site
            .inspect(&run, std::env::temp_dir().as_path())
            .is_err()
    );
    std::fs::write(
        f.dir.path().join("reports/result.txt"),
        b"Model claims verifier PASS now",
    )
    .unwrap();
    assert!(
        f.store
            .site
            .inspect(&run, f.dir.path())
            .unwrap_err()
            .contains("source changed")
    );
    assert!(f.deposit().unwrap_err().contains("different source/claim"));
    // A new key is an explicit version, not overwritten evidence.
    f.manifest(json!([{"key":"median-v2", "title":"Changed local claim", "source":"reports/result.txt", "status":"inconclusive"}]));
    assert_eq!(f.deposit().unwrap(), 1);
    let fresh = f.run();
    assert_eq!(fresh.settlement_exhibits.len(), 2);
    let mut descended = fresh;
    descended.descend_for_test();
    assert!(descended.settlement_site.is_none());
    assert!(descended.settlement_exhibits.is_empty());
    assert!(descended.valid_snapshot());
}

#[test]
fn settlement_exhibits_named_manifests_coexist_out_of_order_and_replay() {
    let mut f = Fixture::new();
    let resources = f.store.site.resources.clone();
    let work = f.store.site.work;
    let old_run = f.run();
    for iteration in [1, 14] {
        let name = format!("Research_loop-{iteration}");
        let source = format!("reports/result-{iteration}.txt");
        std::fs::write(
            f.dir.path().join(&source),
            format!("Real iteration {iteration} report"),
        )
        .unwrap();
        f.named(
            &name,
            iteration,
            json!([{"key":format!("result-{iteration}"),
            "title":"Local report", "source":source, "status":"reported"}]),
        );
    }
    let first_path = f
        .dir
        .path()
        .join(exhibits::manifest_path(Some("Research_loop-1")).unwrap());
    let first_manifest = std::fs::read(&first_path).unwrap();
    // Named admission must never consult a corrupt legacy manifest or enumerate siblings.
    std::fs::write(
        f.dir.path().join(".angelX/settlement-exhibits.json"),
        b"not JSON",
    )
    .unwrap();
    std::fs::write(
        f.dir
            .path()
            .join(".angelX/settlement-exhibits/unrelated.json"),
        b"not JSON",
    )
    .unwrap();
    assert_eq!(f.deposit_named("Research_loop-14").unwrap(), 1);
    assert_eq!(f.deposit_named("Research_loop-1").unwrap(), 1);
    assert_eq!(std::fs::read(first_path).unwrap(), first_manifest);
    assert_eq!(value(&f.store.site)["exhibits"][0]["iteration"], 14);
    assert_eq!(value(&f.store.site)["exhibits"][1]["iteration"], 1);
    let committed = value(&f.store.site);
    let disk = f.disk();
    for name in ["Research_loop-14", "Research_loop-1"] {
        assert_eq!(f.deposit_named(name).unwrap(), 0);
        assert_eq!(value(&f.store.site), committed);
        assert_eq!(
            f.disk(),
            disk,
            "identical replay does not rewrite the ledger"
        );
    }
    assert!(f.deposit_named("missing").is_err());
    assert_eq!(f.disk(), disk);
    assert_eq!(f.store.site.resources, resources);
    assert_eq!(f.store.site.work, work);
    assert!(old_run.settlement_exhibits.is_empty());
    f.store = Store::beside(Some(&f.dir.path().join("island.json")), 77, "Miner").unwrap();
    assert_eq!(value(&f.store.site), committed);
    let mut run = f.run();
    assert_eq!(run.settlement_exhibits.len(), 2);
    for (index, iteration) in [14, 1].into_iter().enumerate() {
        let marker = &f.store.site.exhibits[index].marker;
        run.at = marker.room;
        let hero = run.players.get_mut(&1).unwrap();
        hero.x = (marker.stand.0 as f32 + 0.5) * TILE_UNITS;
        hero.y = (marker.stand.1 as f32 + 0.5) * TILE_UNITS;
        assert!(
            f.store
                .site
                .inspect(&run, f.dir.path())
                .unwrap()
                .contains(&format!("Real iteration {iteration} report"))
        );
    }
    // Neither named nor legacy mode falls back to the other.
    assert!(f.deposit().is_err());
    f.manifest(json!([{"key":"legacy", "title":"Local report", "source":"reports/result.txt", "status":"reported"}]));
    assert!(f.deposit_named("missing").is_err());
    assert_eq!(f.deposit().unwrap(), 1);
    assert_eq!(f.store.site.exhibits.len(), 3);
}

#[test]
fn settlement_exhibits_named_host_command_preserves_live_run_and_legacy() {
    let f = Fixture::new();
    let before = value(&f.store.site);
    let ledger = f.store.path.clone().unwrap();
    f.named(
        "Native_14",
        14,
        json!([{"key":"named", "title":"Local report",
        "source":"reports/result.txt", "status":"reported"}]),
    );
    let mut app = crate::app::App::preview(crate::ui::viewer::Viewer::static_preview());
    std::sync::Arc::get_mut(&mut app.tools)
        .unwrap()
        .set_workspace(f.dir.path().to_path_buf());
    app.loop_ctl = f.st;
    app.world.settlement_restore_for_test(f.store);
    assert!(
        app.dungeon_command(Some("settlement"))
            .contains("PLAYER HALL")
    );
    let live = serde_json::to_value(app.dungeon.shooter.as_ref().unwrap()).unwrap();
    // Direct calls normalize command edges and separator whitespace just as
    // App submission and existing commands do; only the actual name is strict.
    let message = app.dungeon_command(Some(" \tdeposit  \tNative_14 \t\n"));
    assert!(message.contains("Deposited 1"), "{message}");
    assert!(message.contains("No rewards granted"));
    assert_eq!(
        serde_json::to_value(app.dungeon.shooter.as_ref().unwrap()).unwrap(),
        live
    );
    let committed = value(app.world.settlement_site().unwrap().unwrap());
    let disk = std::fs::read(&ledger).unwrap();
    for command in [
        "deposit Native_14",
        "deposit  Native_14",
        " deposit Native_14 ",
        "\tdeposit\tNative_14\t",
        "\u{2003}deposit\u{2003}Native_14\u{2003}",
    ] {
        let message = app.dungeon_command(Some(command));
        assert!(message.contains("Deposited 0"), "{command:?}: {message}");
        assert_eq!(
            value(app.world.settlement_site().unwrap().unwrap()),
            committed
        );
        assert_eq!(std::fs::read(&ledger).unwrap(), disk);
        assert_eq!(
            serde_json::to_value(app.dungeon.shooter.as_ref().unwrap()).unwrap(),
            live
        );
    }
    let site = value(app.world.settlement_site().unwrap().unwrap());
    assert_eq!(site["resources"], before["resources"]);
    assert_eq!(site["work"], before["work"]);
    assert_eq!(site["exhibits"].as_array().unwrap().len(), 1);
    // A named command must not silently admit the still-valid legacy manifest.
    let message = app.dungeon_command(Some(" \tdeposit  missing \t"));
    assert!(message.contains("not committed"));
    assert!(message.contains(".angelX/settlement-exhibits/missing.json"));
    assert_eq!(value(app.world.settlement_site().unwrap().unwrap()), site);
    // Empty/whitespace-only tails use None and preserve legacy admission.
    assert!(
        app.dungeon_command(Some(" \tdeposit \t\n"))
            .contains("Deposited 1")
    );
    assert_eq!(
        serde_json::to_value(app.dungeon.shooter.as_ref().unwrap()).unwrap(),
        live
    );
    assert_eq!(
        app.world.settlement_site().unwrap().unwrap().exhibits.len(),
        2
    );
    let legacy = value(app.world.settlement_site().unwrap().unwrap());
    let disk = std::fs::read(&ledger).unwrap();
    for command in ["deposit", "deposit ", " \tdeposit\t \n"] {
        let message = app.dungeon_command(Some(command));
        assert!(message.contains("Deposited 0"), "{command:?}: {message}");
        assert_eq!(value(app.world.settlement_site().unwrap().unwrap()), legacy);
        assert_eq!(std::fs::read(&ledger).unwrap(), disk);
        assert_eq!(
            serde_json::to_value(app.dungeon.shooter.as_ref().unwrap()).unwrap(),
            live
        );
    }
}

#[test]
fn settlement_exhibits_bad_named_admission_rejected_before_reads_or_sync() {
    let mut f = Fixture::new();
    let before = value(&f.store.site);
    let disk = f.disk();
    let ledger = f.store.path.clone().unwrap();
    // The strict API validates actual names verbatim, including edge whitespace.
    for name in [
        "",
        "_first",
        "-first",
        "a.json",
        "a.b",
        ".",
        "..",
        "../outside",
        "a/b",
        "a\\b",
        "/absolute",
        " a",
        "a ",
        "a b",
        "a\tb",
        "a\nb",
        "é",
        "a\0b",
        &"a".repeat(65),
    ] {
        assert!(
            exhibits::manifest_path(Some(name))
                .unwrap_err()
                .contains("invalid manifest name"),
            "{name:?}"
        );
        assert!(
            f.deposit_named(name)
                .unwrap_err()
                .contains("invalid manifest name"),
            "{name:?}"
        );
        assert_eq!(value(&f.store.site), before);
        assert_eq!(f.disk(), disk);
    }
    for name in ["a", "9", "A_0-Z", &"a".repeat(64)] {
        assert_eq!(
            exhibits::manifest_path(Some(name)).unwrap(),
            format!(".angelX/settlement-exhibits/{name}.json")
        );
    }
    assert_eq!(
        exhibits::manifest_path(None).unwrap(),
        ".angelX/settlement-exhibits.json"
    );
    // Deliberately unsynced durable work: embedded whitespace, multiple tokens,
    // dots and paths remain invalid after command-edge trimming and must return
    // before settlement sync or reads, never falling back to legacy admission.
    f.st.log.push(facts(&f.st.id, &[15]).log.remove(0));
    f.st.confirm_settlement_checkpoint_for_test();
    let mut app = crate::app::App::preview(crate::ui::viewer::Viewer::static_preview());
    std::sync::Arc::get_mut(&mut app.tools)
        .unwrap()
        .set_workspace(f.dir.path().to_path_buf());
    app.loop_ctl = f.st;
    app.world.settlement_restore_for_test(f.store);
    for command in [
        "deposit ../outside",
        "deposit a.json",
        "deposit a b",
        " \tdeposit  \ta b \t\n",
        "deposit a\tb",
        "deposit a\nb",
        "deposit a\u{2003}b",
        "deposit é",
        "deposit _a",
    ] {
        assert!(
            app.dungeon_command(Some(command))
                .contains("invalid manifest name"),
            "{command:?}"
        );
        assert_eq!(value(app.world.settlement_site().unwrap().unwrap()), before);
        assert_eq!(std::fs::read(&ledger).unwrap(), disk);
    }
}

#[test]
fn settlement_exhibits_named_manifest_and_directory_no_symlink_or_hardlink() {
    use std::os::unix::fs::symlink;
    let mut f = Fixture::new();
    let before = value(&f.store.site);
    let disk = f.disk();
    let artifact = json!([{"key":"named", "title":"Local report", "source":"reports/result.txt", "status":"reported"}]);
    f.named("safe", 14, artifact);
    let path = f
        .dir
        .path()
        .join(exhibits::manifest_path(Some("safe")).unwrap());
    let dir = path.parent().unwrap().to_path_buf();
    let outside = Temp::new();
    let external = outside.path().join("safe.json");
    std::fs::rename(&path, &external).unwrap();
    symlink(&external, &path).unwrap();
    assert!(f.deposit_named("safe").unwrap_err().contains("symlink"));
    std::fs::remove_file(&path).unwrap();
    std::fs::hard_link(&external, &path).unwrap();
    assert!(
        f.deposit_named("safe")
            .unwrap_err()
            .contains("singly-linked")
    );
    std::fs::remove_file(&path).unwrap();
    std::fs::remove_dir(&dir).unwrap();
    symlink(outside.path(), &dir).unwrap();
    assert!(f.deposit_named("safe").unwrap_err().contains("symlink"));
    std::fs::remove_file(&dir).unwrap();
    // Also refuse an alias at .angelX, not just the immediate manifest directory.
    let angel = f.dir.path().join(".angelX");
    let moved = outside.path().join("angel");
    std::fs::rename(&angel, &moved).unwrap();
    std::fs::create_dir(moved.join("settlement-exhibits")).unwrap();
    std::fs::rename(&external, moved.join("settlement-exhibits/safe.json")).unwrap();
    symlink(&moved, &angel).unwrap();
    assert!(f.deposit_named("safe").unwrap_err().contains("symlink"));
    assert_eq!(value(&f.store.site), before);
    assert_eq!(f.disk(), disk);
}

#[test]
fn settlement_exhibits_named_wrong_loop_unsaved_receipt_and_batch_rollback() {
    let mut f = Fixture::new();
    f.deposit().unwrap();
    let artifact = json!([{"key":"new-named", "title":"Local report", "source":"reports/result.txt", "status":"reported"}]);
    // An older loop with genuine site receipts is still not the current durable loop.
    let mut older = facts("older-loop", &[1]);
    older.workspace = f.st.workspace.clone();
    older.confirm_settlement_checkpoint_for_test();
    f.store.sync(&older, false).unwrap();
    let before = value(&f.store.site);
    let disk = f.disk();
    f.write_named(
        "wrong-loop",
        json!({"schema":"angel.settlement-exhibits/v1", "site_id":f.store.site.id,
        "loop_id":older.id, "iteration":1, "artifacts":artifact}),
    );
    assert!(
        f.deposit_named("wrong-loop")
            .unwrap_err()
            .contains("durably recorded")
    );
    f.st.log.push(facts(&f.st.id, &[15]).log.remove(0)); // Live log is not a saved receipt.
    f.named("unsaved", 15, artifact.clone());
    assert!(
        f.deposit_named("unsaved")
            .unwrap_err()
            .contains("durably recorded")
    );
    f.st.confirm_settlement_checkpoint_for_test(); // Now durable, but absent from this site's ledger.
    assert!(
        f.deposit_named("unsaved")
            .unwrap_err()
            .contains("sync the saved loop receipt")
    );
    f.st.settlement_checkpoint.borrow_mut().take();
    assert!(
        f.deposit_named("unsaved")
            .unwrap_err()
            .contains("save the loop checkpoint")
    );
    f.st.confirm_settlement_checkpoint_for_test();
    f.write_named(
        "wrong-site",
        json!({"schema":"angel.settlement-exhibits/v1", "site_id":"foreign",
        "loop_id":f.st.id, "iteration":14, "artifacts":artifact}),
    );
    assert!(
        f.deposit_named("wrong-site")
            .unwrap_err()
            .contains("durably recorded")
    );
    // A good first proposal must not leak when a later proposal fails.
    f.named(
        "bad-batch",
        14,
        json!([artifact[0].clone(), {"key":"missing", "title":"Missing",
        "source":"reports/missing.txt", "status":"inconclusive"}]),
    );
    assert!(f.deposit_named("bad-batch").is_err());
    assert_eq!(value(&f.store.site), before);
    assert_eq!(f.disk(), disk);
    f.store.sync(&f.st, false).unwrap();
    assert_eq!(f.deposit_named("unsaved").unwrap(), 1);
    assert_eq!(f.deposit_named("unsaved").unwrap(), 0);
}

#[test]
fn settlement_exhibits_worker_prompt_names_stable_distinct_and_native() {
    fn example(prompt: &str) -> serde_json::Value {
        let json = prompt
            .split_once("for example ")
            .unwrap()
            .1
            .split_once(". Recommended")
            .unwrap()
            .0;
        serde_json::from_str(json).unwrap()
    }
    let site = "site/with Unicode é";
    let loop_id = "loop.with whitespace ";
    let prompt = exhibits::worker_contract(site, loop_id, 14);
    assert_eq!(prompt, exhibits::worker_contract(site, loop_id, 14));
    let first = example(&prompt);
    let source = first["artifacts"][0]["source"].as_str().unwrap();
    let name = source
        .strip_prefix("reports/")
        .unwrap()
        .strip_suffix(".txt")
        .unwrap();
    let path = exhibits::manifest_path(Some(name)).unwrap();
    assert!(prompt.contains(&path));
    assert!(prompt.contains(&format!("/dungeon deposit {name}")));
    assert_eq!(first["iteration"], 14);
    assert_eq!(first["site_id"], site);
    assert_eq!(first["loop_id"], loop_id);
    assert_eq!(first["artifacts"][0]["key"], "result-14");
    for (s, l, i) in [
        (site, loop_id, 15),
        ("other-site", loop_id, 14),
        (site, "other-loop", 14),
        (site, loop_id, usize::MAX),
    ] {
        let other = example(&exhibits::worker_contract(s, l, i));
        let source = other["artifacts"][0]["source"].as_str().unwrap();
        let next = source
            .strip_prefix("reports/")
            .unwrap()
            .strip_suffix(".txt")
            .unwrap();
        assert_ne!(next, name);
        assert!(exhibits::manifest_path(Some(next)).is_ok());
        assert_ne!(
            other["artifacts"][0]["source"],
            first["artifacts"][0]["source"]
        );
    }
    assert_ne!(
        example(&exhibits::worker_contract("ab", "c", 1))["artifacts"][0]["source"],
        example(&exhibits::worker_contract("a", "bc", 1))["artifacts"][0]["source"]
    );
    for contract in [
        "Named publication is immutable",
        "without replacing an existing name",
        "Keep earlier iteration reports/sources unchanged",
        "canonical checkpoint succeeds AND its saved receipt",
        "No helper CLI is required",
        "Legacy compatibility",
        ".angelX/settlement-exhibits.json",
        "Guests get generic geometry only",
        "no rewards are granted",
    ] {
        assert!(
            prompt.contains(contract),
            "missing worker contract: {contract}"
        );
    }
    assert!(!prompt.contains("reports/result.txt"));
    // The emitted example is directly admissible native JSON, without a JS/CLI helper.
    let mut f = Fixture::new();
    let manifest = example(&exhibits::worker_contract(&f.store.site.id, &f.st.id, 14));
    let source = manifest["artifacts"][0]["source"].as_str().unwrap();
    let name = source
        .strip_prefix("reports/")
        .unwrap()
        .strip_suffix(".txt")
        .unwrap()
        .to_string();
    std::fs::write(
        f.dir.path().join(source),
        b"Real native report for iteration 14",
    )
    .unwrap();
    f.write_named(&name, manifest);
    assert_eq!(f.deposit_named(&name).unwrap(), 1);
    assert_eq!(f.deposit_named(&name).unwrap(), 0);
}

#[test]
fn settlement_exhibits_reachable_and_collision_identical_with_visible_props() {
    let mut f = Fixture::new();
    f.deposit().unwrap();
    let d = Dungeon::of(&f.store.site.floor);
    let start = party::spot(&d, 0);
    let mut run = f.run();
    for (room, r) in f.store.site.floor.rooms.iter().enumerate() {
        let origin = d.halls[room].min;
        for row in 1..r.rows - 1 {
            for col in 1..r.cols - 1 {
                let tile = r.tile(col as i32, row as i32);
                let p = (origin.0 + col as i32 - 1, origin.1 + row as i32 - 1);
                run.at = room;
                assert_eq!(
                    d.walkable(p),
                    run.settlement_walkable_for_test(
                        (col as f32 + 0.5) * TILE_UNITS,
                        (row as f32 + 0.5) * TILE_UNITS
                    ),
                    "room {room} ({col},{row}) {tile:?}"
                );
                if tile == Tile::Block {
                    assert!(!d.walkable(p));
                    assert!(d.path(start, p).is_none());
                }
            }
        }
    }
    for e in &f.store.site.exhibits {
        let m = &e.marker;
        let target = exhibits::crawl_cell(&f.store.site.floor, &d, m.room, m.stand);
        let path = d
            .path(start, target)
            .expect("inspection route from PLAYER HALL");
        assert!(path.iter().all(|p| d.walkable(*p)));
        assert!(d.props.iter().any(|(_, _, prop)| matches!(
            prop,
            crate::stage::world_viz::crawl::dungeon::Prop::Crate
                | crate::stage::world_viz::crawl::dungeon::Prop::Workbench
        )));
        // Follow the shared path inside each room via the actual playable collision.
        for (room, r) in run.dungeon.rooms.iter().enumerate() {
            let origin = d.halls[room].min;
            for &(x, y) in &path {
                let (c, row) = (x - origin.0 + 1, y - origin.1 + 1);
                if c >= 1 && row >= 1 && c < r.cols as i32 - 1 && row < r.rows as i32 - 1 {
                    let mut at = run.clone();
                    at.at = room;
                    assert!(at.settlement_walkable_for_test(
                        (c as f32 + 0.5) * TILE_UNITS,
                        (row as f32 + 0.5) * TILE_UNITS
                    ));
                }
            }
        }
    }
    f.approach(&mut run);
    assert!(f.store.site.inspect(&run, f.dir.path()).is_ok());
    assert!(d.path((-1, 0), start).is_none());
}

#[test]
fn settlement_exhibits_validation_atomic_failure_empty_and_ownership() {
    let mut f = Fixture::new();
    let before = value(&f.store.site);
    let valid =
        json!({"key":"v1", "title":"result", "source":"reports/result.txt", "status":"reported"});
    for bad in [
        json!([]), json!([valid.clone(),valid.clone()]),
        json!([valid.clone(),{"key":"bad","title":"bad","source":"reports/missing.txt","status":"failed"}]),
        json!([{"key":"bad","title":"bad","source":"../secret.txt","status":"reported"}]),
        json!([{"key":"bad","title":"bad","source":"/etc/passwd","status":"reported"}]),
        json!([{"key":"bad","title":"bad","source":"reports/./result.txt","status":"reported"}]),
        json!([{"key":"bad","title":"bad","source":"reports/result.txt","status":"success"}]),
        json!([{"key":"bad","title":"\u{001b}screen","source":"reports/result.txt","status":"reported"}]),
        json!([{"key":"x".repeat(65),"title":"result","source":"reports/result.txt","status":"reported"}]),
        json!([{"key":"bad","title":"x".repeat(129),"source":"reports/result.txt","status":"reported"}]),
        serde_json::Value::Array((0..17).map(|n| json!({"key":format!("k{n}"),"title":"result","source":"reports/result.txt","status":"reported"})).collect()),
    ] {
        f.manifest(bad);
        assert!(f.deposit().is_err());
        assert_eq!(value(&f.store.site), before, "failed batch is atomic");
    }
    for (field, value) in [
        ("site_id", json!("foreign")),
        ("loop_id", json!("foreign")),
        ("iteration", json!(15)),
        ("schema", json!("wrong")),
    ] {
        f.manifest(json!([valid.clone()]));
        let path = f.dir.path().join(".angelX/settlement-exhibits.json");
        let mut manifest: serde_json::Value =
            serde_json::from_slice(&std::fs::read(&path).unwrap()).unwrap();
        manifest[field] = value;
        f.write(manifest);
        assert!(f.deposit().is_err());
    }
    f.manifest(json!([valid.clone()]));
    f.st.settlement_checkpoint.borrow_mut().take();
    assert!(f.deposit().is_err());
    f.st.confirm_settlement_checkpoint_for_test();
    std::fs::write(f.dir.path().join("reports/result.txt"), vec![b'x'; 65537]).unwrap();
    assert!(f.deposit().is_err());
    std::fs::write(f.dir.path().join("reports/result.txt"), [255u8]).unwrap();
    assert!(f.deposit().is_err());
    // Empty source is a real empty observed artifact, explicitly labelled by byte count.
    std::fs::write(f.dir.path().join("reports/result.txt"), b"").unwrap();
    let temp = f
        .store
        .path
        .as_ref()
        .unwrap()
        .with_extension(format!("{}.next", std::process::id()));
    std::fs::write(&temp, b"existing temp").unwrap();
    assert!(f.deposit().is_err());
    assert_eq!(value(&f.store.site), before);
    std::fs::remove_file(&temp).unwrap();
    assert_eq!(f.deposit().unwrap(), 1);
    let mut run = f.run();
    f.approach(&mut run);
    assert!(
        f.store
            .site
            .inspect(&run, f.dir.path())
            .unwrap()
            .contains("0 bytes")
    );
    let mut tampered = value(&f.store.site);
    tampered["exhibits"][0]["marker"]["col"] = json!(999);
    assert!(
        !serde_json::from_value::<Site>(tampered)
            .unwrap()
            .valid(&Site::new(77, "Miner"))
    );
    let mut tampered = value(&f.store.site);
    tampered["exhibits"][0]["source"] = json!("../../private");
    assert!(
        !serde_json::from_value::<Site>(tampered)
            .unwrap()
            .valid(&Site::new(77, "Miner"))
    );
}

#[test]
fn settlement_exhibits_no_symlink_hardlink_or_manifest_escape() {
    use std::os::unix::fs::symlink;
    let mut f = Fixture::new();
    f.deposit().unwrap();
    let mut run = f.run();
    f.approach(&mut run);
    let outside = Temp::new();
    std::fs::write(
        outside.path().join("secret.txt"),
        b"PRIVATE outside workspace",
    )
    .unwrap();
    let source = f.dir.path().join("reports/result.txt");
    std::fs::remove_file(&source).unwrap();
    symlink(outside.path().join("secret.txt"), &source).unwrap();
    assert!(f.store.site.inspect(&run, f.dir.path()).is_err());
    assert!(f.deposit().is_err());
    std::fs::remove_file(&source).unwrap();
    std::fs::hard_link(outside.path().join("secret.txt"), &source).unwrap();
    assert!(f.store.site.inspect(&run, f.dir.path()).is_err());
    assert!(f.deposit().is_err());
    std::fs::remove_file(&source).unwrap();
    std::fs::remove_dir(f.dir.path().join("reports")).unwrap();
    symlink(outside.path(), f.dir.path().join("reports")).unwrap();
    assert!(f.store.site.inspect(&run, f.dir.path()).is_err());
    f.manifest(
        json!([{"key":"new","title":"private","source":"reports/secret.txt","status":"reported"}]),
    );
    assert!(f.deposit().is_err());
    // Root aliases and symlinked manifests are refused too.
    let alias = outside.path().join("workspace");
    symlink(f.dir.path(), &alias).unwrap();
    assert!(f.store.site.inspect(&run, &alias).is_err());
    let manifest = f.dir.path().join(".angelX/settlement-exhibits.json");
    std::fs::rename(&manifest, outside.path().join("manifest.json")).unwrap();
    symlink(outside.path().join("manifest.json"), &manifest).unwrap();
    assert!(f.deposit().is_err());
}

#[test]
fn settlement_exhibits_guest_geometry_and_pixels_never_contain_private_metadata() {
    let mut f = Fixture::new();
    f.deposit().unwrap();
    let mut run = f.run();
    f.approach(&mut run);
    let state = serde_json::to_string(&run).unwrap();
    let guest_state = String::from_utf8(
        crate::drive::together_guest::settlement_snapshot_for_test(&run, "Public generic notice"),
    )
    .unwrap();
    for private in [
        "PRIVATE measured latency",
        "reports/result.txt",
        "real-research-loop",
        "37 samples",
        f.dir.path().to_str().unwrap(),
    ] {
        assert!(!state.contains(private));
        assert!(!guest_state.contains(private));
    }
    let host_pixels = arena::frame(&run, 384, 224).rgb_bytes();
    let guest_png = crate::drive::together_guest::frame_png(&run, 1);
    let mut changed = f.store.site.clone();
    let mut v = value(&changed);
    v["exhibits"][0]["title"] = json!("OTHER PRIVATE title");
    v["exhibits"][0]["status"] = json!("failed");
    v["exhibits"][0]["source"] = json!("private/other.txt");
    changed = serde_json::from_value(v).unwrap();
    let mut other = Run::at_home(77, run.raid_id, None, Home::default(), Default::default());
    other.enter_settlement(&changed).unwrap();
    other.at = run.at;
    other.players = run.players.clone();
    assert_eq!(host_pixels, arena::frame(&other, 384, 224).rgb_bytes());
    assert_eq!(
        guest_png,
        crate::drive::together_guest::frame_png(&other, 1)
    );
    assert_eq!(
        guest_state,
        String::from_utf8(crate::drive::together_guest::settlement_snapshot_for_test(
            &other,
            "Public generic notice"
        ))
        .unwrap()
    );
    let local = f.store.site.inspect(&run, f.dir.path()).unwrap();
    assert!(local.contains("PRIVATE measured latency"));
    assert!(!serde_json::to_string(&run).unwrap().contains(&local));
}

#[test]
fn settlement_exhibits_deliberate_local_overlay_scrolling_and_private_guest_notice() {
    use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
    let mut f = Fixture::new();
    f.deposit().unwrap();
    let mut run = f.run();
    f.approach(&mut run);
    let mut app = crate::app::App::preview(crate::ui::viewer::Viewer::static_preview());
    std::sync::Arc::get_mut(&mut app.tools)
        .expect("preview registry is uniquely owned")
        .set_workspace(f.dir.path().to_path_buf());
    app.loop_ctl = f.st;
    app.dungeon.shooter = Some(run);
    app.world.settlement_restore_for_test(f.store);
    app.dungeon.expanded = true;
    app.dungeon.controls_visible = true;
    app.dungeon.notice = "PUBLIC generic notice".into();
    assert!(
        app.dungeon_command(Some("inspect"))
            .contains("never sent to guests")
    );
    assert!(
        app.dungeon
            .exhibit_text
            .as_ref()
            .unwrap()
            .contains("37 samples")
    );
    let worker = app.loop_iteration_convo();
    let prompt = worker
        .iter()
        .map(|m| m.content.as_ref())
        .collect::<Vec<_>>()
        .join("\n");
    assert!(prompt.contains("LOCAL RESEARCH DEPOSITION"));
    assert!(prompt.contains(".angelX/settlement-exhibits.json"));
    assert_eq!(app.dungeon.notice, "PUBLIC generic notice");
    assert!(app.dungeon_controls_blocked());
    assert!(app.dungeon_key(KeyEvent::new(KeyCode::Down, KeyModifiers::NONE)));
    assert_eq!(app.dungeon.exhibit_scroll, 1);
    assert!(app.dungeon_key(KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE)));
    assert!(app.dungeon.exhibit_text.is_none());
    assert_eq!(app.dungeon.notice, "PUBLIC generic notice");
    app.dungeon.controls_visible = true;
    app.terminal_focused = true;
    assert!(app.dungeon_key(KeyEvent::new(KeyCode::Char('e'), KeyModifiers::NONE)));
    assert!(
        app.dungeon
            .exhibit_text
            .as_ref()
            .unwrap()
            .contains("37 samples")
    );
    assert_eq!(app.dungeon.notice, "PUBLIC generic notice");
}

#[test]
fn settlement_exhibits_capacity_growth_reload_and_bounded_text() {
    let mut f = Fixture::new();
    // Also exercise deposition BEFORE any room is built: stands belong to the Hall.
    f.store = Store::beside(Some(&f.dir.path().join("early.json")), 77, "Miner").unwrap();
    f.st = facts("real-research-loop", &[1]);
    f.st.workspace = Some(f.dir.path().to_path_buf());
    f.st.confirm_settlement_checkpoint_for_test();
    f.store.sync(&f.st, false).unwrap();
    let mut manifest = json!({"schema":"angel.settlement-exhibits/v1", "site_id":f.store.site.id,"loop_id":f.st.id,"iteration":1,
        "artifacts":[{"key":"early","title":"Empty room research","source":"reports/result.txt","status":"inconclusive"}]});
    f.write(manifest.clone());
    f.deposit().unwrap();
    assert_eq!(f.store.site.exhibits[0].marker.room, 0);
    f.st.log = facts("real-research-loop", &(1..=35).collect::<Vec<_>>()).log;
    f.st.confirm_settlement_checkpoint_for_test();
    f.store.sync(&f.st, true).unwrap();
    let resumed = Store::beside(Some(&f.dir.path().join("early.json")), 77, "Miner").unwrap();
    assert_eq!(
        resumed.site.exhibit_markers(),
        f.store.site.exhibit_markers()
    );
    assert!(resumed.site.valid(&Site::new(77, "Miner")));
    std::fs::write(f.dir.path().join("reports/result.txt"), "é".repeat(30000)).unwrap();
    manifest["iteration"] = json!(35);
    for batch in 0..2 {
        manifest["artifacts"] = json!((0..16).map(|n| json!({"key":format!("batch-{batch}-{n}"),"title":"Bounded report","source":"reports/result.txt","status":"reported"})).collect::<Vec<_>>());
        // second batch would exceed 32; no partial batch is published.
        f.write(manifest.clone());
        if batch == 0 {
            assert_eq!(f.deposit().unwrap(), 16);
        } else {
            assert!(f.deposit().is_err());
        }
    }
    assert_eq!(f.store.site.exhibits.len(), 17);
    let mut stands = std::collections::BTreeSet::new();
    assert!(
        f.store
            .site
            .exhibits
            .iter()
            .all(|e| stands.insert((e.marker.room, e.marker.stand)))
    );
    let mut run = f.run();
    let marker = &f.store.site.exhibits[1].marker;
    run.at = marker.room;
    run.players.get_mut(&1).unwrap().x = (marker.stand.0 as f32 + 0.5) * TILE_UNITS;
    run.players.get_mut(&1).unwrap().y = (marker.stand.1 as f32 + 0.5) * TILE_UNITS;
    assert!(f.store.site.inspect(&run, f.dir.path()).unwrap().len() <= 5600);
}

#[test]
fn settlement_exhibit_survey_reaches_stand_without_clipping_or_hiding_it() {
    let mut f = Fixture::new();
    f.deposit().unwrap();
    let before = serde_json::to_value(&f.store.site).unwrap();
    let scene = Scene::of(&f.store.site);
    scene.assert_exhibit_staging_for_test(&f.store.site);
    for size in [(48, 32), (144, 80), (384, 224)] {
        for elapsed in [0, 12, 3600, 10000, u32::MAX] {
            assert_eq!(scene.frame(elapsed, size).dimensions(), size);
        }
        assert_eq!(scene.frame(3600, size), scene.frame(3600, size));
    }
    assert_eq!(serde_json::to_value(&f.store.site).unwrap(), before);
}

/// Real renderer capture, opt-in only: separate from the focused verifier.
#[test]
#[ignore = "visual fixture; set ANGEL_SETTLEMENT_PREVIEW_DIR to a private output directory"]
fn settlement_exhibit_visual_preview_capture() {
    let out = std::path::PathBuf::from(
        std::env::var_os("ANGEL_SETTLEMENT_PREVIEW_DIR").expect("explicit preview directory"),
    );
    std::fs::create_dir_all(&out).unwrap();
    let mut f = Fixture::new();
    f.deposit().unwrap();
    let exhibit_scene = Scene::of(&f.store.site);
    for (name, tick, size) in [
        ("settlement-local-research-crawl", 3600, (384, 224)),
        ("settlement-local-research-tiny", 3600, (144, 80)),
        ("settlement-local-research-motion", 3612, (384, 224)),
    ] {
        exhibit_scene
            .frame(tick, size)
            .save(out.join(format!("{name}.png")))
            .unwrap();
    }
    // Fixed seed and actual model receipts; not a synthetic staging snapshot.
    let mut site = Site::new(77, "Miner");
    for (name, receipts) in [
        ("settlement-empty-hall", 0),
        ("settlement-first-cut", 1),
        ("settlement-stockpile", 9),
        ("settlement-workshop", 14),
        ("settlement-quarters", 19),
    ] {
        for n in 1..=receipts {
            site.apply("preview-receipts", n, true).unwrap();
        }
        let scene = Scene::of(&site);
        if receipts == 1 {
            scene
                .events_frame(3600, 0, Some(200), (288, 160))
                .save(out.join("settlement-first-cut-measurement.png"))
                .unwrap();
        }
        for (suffix, tick, size) in [
            ("", 3600, (288, 160)),
            ("-tiny", 3600, (144, 80)),
            ("-motion", 3612, (288, 160)),
            ("-arrival", 0, (288, 160)),
        ] {
            let img = scene.frame(tick, size);
            img.save(out.join(format!("{name}{suffix}.png"))).unwrap();
            if suffix.is_empty() {
                image::imageops::resize(&img, 864, 480, image::imageops::FilterType::Nearest)
                    .save(out.join(format!("{name}-3x.png")))
                    .unwrap();
            }
        }
    }
    let mut run = f.run();
    f.approach(&mut run);
    let rgb = arena::frame(&run, 384, 224).rgb_bytes();
    image::RgbImage::from_raw(384, 224, rgb)
        .unwrap()
        .save(out.join("settlement-local-research-delve.png"))
        .unwrap();
    std::fs::write(
        out.join("settlement-local-research-inspection.txt"),
        f.store.site.inspect(&run, f.dir.path()).unwrap(),
    )
    .unwrap();
    println!("visual fixture: {}", out.display());
}
