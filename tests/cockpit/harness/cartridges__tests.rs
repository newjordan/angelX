use super::config::{Config, folders, has_source};
use super::fleet::{FleetState, FleetSnapshot, FleetSubmission, SubmissionPhase, sweep_command};
use super::*;

fn temp(name: &str) -> std::path::PathBuf {
    let dir = std::env::temp_dir().join(format!("angel-cartridges-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

#[test]
fn a_cartridge_toml_needs_an_id_and_a_label_and_known_fields() {
    let config = Config::parse(
        "id = \"kb\"\nlabel = \"KernelBench\"\nboards = [\"kb\"]\nmeasures = [\"run\"]\n",
    )
    .unwrap();
    assert_eq!(config.id, "kb");
    assert_eq!(config.boards, ["kb"]);
    assert!(Config::parse("label = \"no id\"\n").is_err());
    assert!(Config::parse("id = \"kb\"\nlabel = \"KB\"\nsecret_mode = true\n").is_err());
}

#[test]
fn folders_find_cartridges_and_folders_of_them_in_order() {
    let root = temp("folders");
    for (name, source) in [("zeta", false), ("alpha", true)] {
        let dir = root.join(name);
        std::fs::create_dir_all(dir.join("src")).unwrap();
        std::fs::write(dir.join("cartridge.toml"), format!("id = \"{name}\"\nlabel = \"{name}\"\n")).unwrap();
        if source {
            std::fs::write(dir.join("src").join("mod.rs"), "").unwrap();
        }
    }
    std::fs::create_dir_all(root.join("not-a-cartridge")).unwrap();
    let found = folders(std::slice::from_ref(&root));
    assert_eq!(found, [root.join("alpha"), root.join("zeta")]);
    assert!(has_source(&found[0]) && !has_source(&found[1]));
    // A cartridge named directly is taken as itself.
    assert_eq!(folders(&[root.join("zeta")]), [root.join("zeta")]);
    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn tests_play_the_fixture_board_and_its_measurements() {
    let _env = crate::tests::env_lock();
    let board = active().expect("the fixture board is always plugged in under test");
    assert_eq!(board.id(), "board");
    assert!(board.measures("board", Some("run")));
    assert!(board.measures("board", Some("validate")));
    assert!(!board.measures("board", Some("submit")));
    assert!(!board.measures("bench", Some("run")));
    assert!(boards().any(|name| name == "board"));
    assert_eq!(status_check(), "the platform CLI");
}

#[test]
fn an_unknown_cartridge_says_so_and_falls_back() {
    let _env = crate::tests::env_lock();
    let _pick = crate::tests::TestEnvGuard::set("ANGEL_CARTRIDGE", "no-such-board");
    assert_eq!(active().map(Cartridge::id), Some("board"));
    assert_eq!(
        selection_error().as_deref(),
        Some("unknown cartridge `no-such-board`; using `board`")
    );
}

#[test]
fn a_config_only_fleet_is_one_command_printing_one_snapshot() {
    let snapshot = sweep_command(
        r#"printf '%s' '{"benchmarks": 2, "failed": 1, "submissions": [{"benchmark": "b/one", "id": "abc", "status": "validating"}, {"benchmark": "b/two", "id": "def", "status": "accepted", "score": "1.5"}]}'"#,
        None,
    )
    .unwrap();
    assert_eq!(snapshot.benchmark_count, 2);
    assert_eq!(snapshot.failed_benchmarks, 1);
    assert_eq!(snapshot.entries[0].phase, SubmissionPhase::Running);
    assert_eq!(snapshot.entries[1].phase, SubmissionPhase::Accepted);
    assert_eq!(snapshot.entries[1].score.as_deref(), Some("1.5"));
    assert!(sweep_command("printf 'not json'", None).is_err());
    assert!(sweep_command("exit 3", None).is_err());
    // The command finds its own cartridge's folder.
    let folder = temp("fleet-folder");
    std::fs::write(folder.join("fleet.sh"), "printf '{\"benchmarks\": 1, \"failed\": 0, \"submissions\": []}'").unwrap();
    let own = sweep_command("bash \"$ANGEL_CARTRIDGE_DIR/fleet.sh\"", Some(&folder)).unwrap();
    assert_eq!(own.benchmark_count, 1);
    let _ = std::fs::remove_dir_all(&folder);
}

#[test]
fn a_fleet_failure_keeps_the_last_good_rows_as_stale() {
    let mut state = FleetState::default();
    state.apply(FleetSnapshot {
        entries: vec![FleetSubmission {
            benchmark: "b/one".into(),
            id: "abc".into(),
            status: "accepted".into(),
            score: Some("1.0".into()),
            phase: SubmissionPhase::Accepted,
        }],
        benchmark_count: 1,
        failed_benchmarks: 0,
    });
    state.fail("offline".into());
    assert_eq!(state.entries().len(), 1);
    assert!(state.stale());
    assert!(state.has_error());
}

#[test]
fn the_book_without_a_cartridge_names_no_family_tool() {
    let _env = crate::tests::env_lock();
    use crate::agent::harness::book::{k_competition, primary, subs};
    let competition = primary(k_competition::CELL).unwrap();
    for sub in subs(competition) {
        for page in sub.pages {
            assert!(!page.contains("_status"), "{page}");
        }
    }
    assert!(
        k_competition::RAPID
            .sub()
            .pages
            .iter()
            .any(|page| page.starts_with("Use the platform CLI for status"))
    );
}

#[test]
fn a_config_only_cartridge_speaks_in_pages() {
    let config = Config::parse(
        r#"
id = "lean-kernel"
label = "Lean Kernel"
worker = "⠻⠊"

[[pages]]
route = "⠻⠊"
name = "lean-kernel"
signal = "the Lean Kernel loop worker"
pages = ["Write impl and impl_correct.", "Spec.lean stays unchanged."]
"#,
    )
    .unwrap();
    let cartridge = Cartridge::new(config, None);
    let worker = cartridge
        .pages()
        .iter()
        .find(|sub| sub.route == crate::agent::harness::book::er_loop::COMPETITION_WORKER)
        .unwrap();
    assert_eq!(worker.name, "lean-kernel");
    assert_eq!(worker.pages, ["Write impl and impl_correct.", "Spec.lean stays unchanged."]);
    assert!(Config::parse("id = \"x\"\nlabel = \"X\"\n[[pages]]\nroute = \"ab\"\nname = \"n\"\nsignal = \"s\"\npages = [\"p\"]\n").is_err());
    assert!(Config::parse("id = \"x\"\nlabel = \"X\"\n[[pages]]\nroute = \"⠻⠊\"\nname = \"n\"\nsignal = \"s\"\npages = []\n").is_err());
}

#[test]
fn the_lean_kernel_example_is_a_valid_cartridge() {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../docs/examples/cartridges/lean-kernel/cartridge.toml");
    let config = Config::parse(&std::fs::read_to_string(path).unwrap()).unwrap();
    assert_eq!(config.id, "lean-kernel");
    assert!(config.boards.is_empty());
    let cartridge = Cartridge::new(config, None);
    assert!(cartridge.has_fleet());
    assert!(
        cartridge
            .pages()
            .iter()
            .any(|sub| sub.route == crate::agent::harness::book::er_loop::COMPETITION_WORKER)
    );
}
