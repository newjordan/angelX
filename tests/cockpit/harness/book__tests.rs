use super::*;

fn scratch(tag: &str) -> std::path::PathBuf {
    let path = std::env::temp_dir().join(format!(
        "angel-book-{tag}-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    std::fs::create_dir_all(&path).unwrap();
    path
}

/// Braille numerals: the subcategory alphabet (⠁1 … ⠚0).
const DIGITS: [char; 10] = ['⠁', '⠃', '⠉', '⠙', '⠑', '⠋', '⠛', '⠓', '⠊', '⠚'];

#[test]
fn the_first_layer_is_cells_that_are_neither_digits_nor_tray_glyphs() {
    // Volume I is the letters k–z; later volumes open on the contractions.
    const LETTERS: &str = "⠅⠇⠍⠝⠕⠏⠟⠗⠎⠞⠥⠧⠺⠭⠽⠵";
    // Lowered cells read as digits; a primary never uses them.
    const LOWERED: &str = "⠂⠆⠒⠲⠢⠖⠶⠦⠔⠴";
    let mut seen = std::collections::HashSet::new();
    for primary in TOC {
        assert!(
            ledger::is_cell(primary.cell),
            "{} is not a braille cell",
            primary.name
        );
        assert!(!DIGITS.contains(&primary.cell), "{}", primary.name);
        assert!(
            ![PASS_GLYPH, WAIT_GLYPH].contains(&primary.cell),
            "{}",
            primary.name
        );
        assert!(
            !LOWERED.contains(primary.cell),
            "{} reads as a digit",
            primary.name
        );
        assert!(
            seen.insert(primary.cell),
            "{} reuses {}",
            primary.name,
            primary.cell
        );
        assert!(
            !primary.subs.is_empty(),
            "{} has no subcategories",
            primary.name
        );
    }
    let letters = TOC
        .iter()
        .take(LETTERS.chars().count())
        .map(|primary| primary.cell);
    assert!(
        letters.eq(LETTERS.chars()),
        "Volume I is k–z, in order, first"
    );
    // Volume I is in chapter order: the file listing reads as it.
    let names = TOC
        .iter()
        .take(16)
        .map(|primary| primary.name)
        .collect::<Vec<_>>();
    assert_eq!(
        names,
        [
            "competition",
            "loops",
            "method",
            "environment",
            "orchestration",
            "processes",
            "stop",
            "relentless",
            "sources",
            "personality",
            "skills",
            "verification",
            "workflow",
            "execution",
            "types",
            "brevity"
        ]
    );
}

#[test]
fn every_subcategory_is_attached_to_its_primary_as_a_braille_digit() {
    for primary in TOC {
        let mut seen = std::collections::HashSet::new();
        for (index, sub) in primary.subs.iter().enumerate() {
            assert_eq!(
                sub.route.primary, primary.cell,
                "{}.{}",
                primary.name, sub.name
            );
            assert_eq!(
                sub.route.sub, DIGITS[index],
                "{}.{} out of order",
                primary.name, sub.name
            );
            assert!(seen.insert(sub.route.sub));
            assert_eq!(
                sub.route.sub().name,
                sub.name,
                "{}.{} resolves",
                primary.name,
                sub.name
            );
            assert!(
                !sub.action.is_empty() || !sub.pages.is_empty(),
                "{}.{} routes nowhere",
                primary.name,
                sub.name
            );
            for banned in [
                "MUST",
                "MANDATORY",
                "Do not",
                "REQUIRED",
                "[harness-telemetry]",
            ] {
                assert!(
                    !sub.signal.contains(banned) && !sub.action.contains(banned),
                    "{}.{} reads as an order: {banned}",
                    primary.name,
                    sub.name
                );
            }
        }
    }
}

#[test]
fn every_route_a_detector_throws_resolves_on_the_tree() {
    for route in [
        k_competition::RAPID,
        k_competition::DEEP,
        k_competition::WATCHER,
        k_competition::SUBMIT,
        k_competition::IMPROVE,
        k_competition::NO_TIMEOUT,
        l_loops::SAME_BATCH,
        l_loops::STORM,
        l_loops::POLL,
        l_loops::CYCLE,
        p_processes::LIVE,
        p_processes::DONE,
        p_processes::FAILED,
        q_stop::DELI,
        q_stop::FINISH,
        r_relentless::ARMED,
        s_sources::CONTRACT,
        m_method::RAPID,
        m_method::DEEP,
        m_method::MAP_NOTE,
        m_method::RECON,
        o_orchestration::ROUTES,
        t_personality::IDENTITY,
        t_personality::PROVENANCE,
        y_types::DRIVER,
        y_types::DRIVER_TEAM,
        y_types::COMPACT,
        y_types::COMPACT_TEAM,
        y_types::REPAIR,
        z_brevity::MARKER,
        z_brevity::RULES,
        x_execution::ANSWER_DIRECTLY,
        x_execution::EMIT_TOOL_CALL,
        v_verification::POSTURE,
        v_verification::RED,
        v_verification::ACCEPTANCE,
        v_verification::POST_WRITE,
        v_verification::RED_STREAK,
        v_verification::FLAKY,
        v_verification::UNTESTED,
        v_verification::TESTS_EDITED,
        v_verification::OUT_OF_SCOPE,
        w_workflow::PROTOCOL,
        w_workflow::BATCHING,
        w_workflow::BATCH,
        w_workflow::FINISH,
        x_execution::ERRORS,
        x_execution::MARKUP,
        x_execution::OUTPUT_CAP,
        x_execution::REASONING_CAP,
        x_execution::EMPTY,
        x_execution::SESSION,
    ] {
        assert_eq!(route.sub().route, route);
        assert!(route.name().contains('.'));
        assert_eq!(route.cells().chars().count(), 2);
    }
}

#[test]
fn a_warpath_is_at_most_six_cells_and_evidence_reaches_the_ledger() {
    let root = scratch("warpath");
    let raises = [
        Raise::new(v_verification::RED, "cargo test: 1 failed".to_string()),
        Raise::new(v_verification::UNTESTED, None),
        Raise::new(v_verification::RED, None),
        Raise::new(l_loops::SAME_BATCH, "grep x".to_string()),
        Raise::new(x_execution::ERRORS, "third hop".to_string()),
    ];
    let cells = warpath(&root, &raises);
    assert_eq!(cells, "⠧⠁⠧⠋⠇⠁", "in order, deduplicated, three routes");
    assert_eq!(cells.chars().count(), 6);
    assert!(ledger::is_warpath_line(&cells));
    // The route past the cap still has its evidence in the ledger.
    assert!(ledger::read(&root, "⠭⠁").unwrap().contains("third hop"));
    assert!(
        ledger::read(&root, "⠧⠁")
            .unwrap()
            .contains("cargo test: 1 failed")
    );
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn the_competition_loop_engages_as_a_double_encoded_warpath() {
    let root = scratch("comp");
    let rapid = warpath(
        &root,
        &k_competition::engage(crate::agent::harness::TaskPace::Rapid),
    );
    assert_eq!(rapid, "⠅⠁⠅⠙⠅⠑", "engage rapid → submit → improve");
    let deep = warpath(
        &root,
        &k_competition::engage(crate::agent::harness::TaskPace::Deep),
    );
    assert_eq!(deep, "⠅⠃⠅⠑⠅⠙", "engage deep → improve → submit");
    let notify = crate::agent::harness::WatchNotify {
        id: "s_4821".into(),
        status: "scored".into(),
        score: Some("948.94M".into()),
        rejection_reason: None,
        source_note: None,
        receipt: None,
    };
    assert_eq!(warpath(&root, &k_competition::watcher(&notify)), "⠅⠉⠅⠙");
    let decoded = ledger::read(&root, "⠅⠉").unwrap();
    assert!(
        decoded.contains("s_4821") && decoded.contains("948.94M"),
        "{decoded}"
    );
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn the_ledger_decodes_the_toc_a_primary_a_route_and_a_warpath() {
    let root = scratch("ledger");
    let toc = ledger::read(&root, "").unwrap();
    for primary in TOC {
        assert!(toc.contains(primary.cell) && toc.contains(primary.name));
    }
    let chapter = ledger::read(&root, "⠟").unwrap();
    for sub in q_stop::PRIMARY.subs {
        assert!(chapter.contains(&sub.route.cells()), "{}", sub.name);
    }
    let deli = ledger::read(&root, "⠟⠁").unwrap();
    assert!(deli.contains("→ ") && deli.contains("consult_model") && deli.contains("\"deli\""));
    let path = ledger::read(&root, "⠧⠁⠧⠋⠟⠁").unwrap();
    assert!(path.contains("failed") && path.contains("`shell`") && path.contains("deli"));
    // The first free sub on the first layer: a digit past a chapter's last
    // section.
    let unattached = TOC
        .iter()
        .find(|primary| primary.subs.len() < DIGITS.len())
        .map(|primary| format!("{}{}", primary.cell, DIGITS[primary.subs.len()]))
        .expect("some chapter has room");
    assert!(
        ledger::read(&root, &unattached).is_err(),
        "an unattached sub is not a route: {unattached}"
    );
    assert!(ledger::read(&root, "nope").unwrap_err().contains("⠅"));
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn the_stop_checkpoint_leads_with_the_most_pressing_facts_then_deli() {
    let root = scratch("checkpoint");
    let facts = [
        Raise::new(v_verification::UNTESTED, None),
        Raise::new(l_loops::SAME_BATCH, None),
        Raise::new(v_verification::RED, "cargo test: 2 failed".to_string()),
    ];
    let mut checkpoint = q_stop::Checkpoint::default();
    assert!(
        checkpoint.engage(&root, &[], true).is_none(),
        "no facts, no checkpoint"
    );
    assert!(
        checkpoint.engage(&root, &facts, false).is_none(),
        "no budget, no checkpoint"
    );
    let (cells, shown) = checkpoint.engage(&root, &facts, true).unwrap();
    assert_eq!(cells, "⠧⠁⠧⠋⠟⠁");
    assert_eq!(shown.last().unwrap().route, q_stop::DELI);
    assert!(checkpoint.spent());
    assert!(
        checkpoint.engage(&root, &facts, true).is_none(),
        "once per turn"
    );
    let deli = ledger::read(&root, "⠟⠁").unwrap();
    assert!(
        deli.contains("⠇⠁ loops.same-batch"),
        "every fact is listed: {deli}"
    );
    let _ = std::fs::remove_dir_all(root);
}

/// polyglot-v1 rust-robot-name (DeepSeek V4.1 Flash, 2026-11-01): a prompt
/// paste-back ending in DSML tool markup, nothing changed, stood on its second
/// answer. With the workspace unchanged it is a failed call: it stops again,
/// a bounded number of times; once the workspace changed, the answer stands.
#[test]
fn unexecuted_tool_markup_with_no_change_does_not_stand() {
    let paste = "You are angel …\n<functions>…</functions>\n<｜DSML｜invoke name=\"read_file\">\
                 </｜｜DSML｜｜ parameter>";
    assert!(crate::agent::club::contains_raw_tool_markup(paste));
    assert!(crate::agent::club::contains_raw_tool_markup(
        "<｜tool▁calls▁begin｜><｜tool▁call▁begin｜>function<｜tool▁sep｜>shell"
    ));
    let root = scratch("checkpoint-markup");
    let facts = [
        Raise::new(d3456_advisories::NO_EDIT, None),
        Raise::new(x_execution::MARKUP, None),
    ];
    let mut checkpoint = q_stop::Checkpoint::default();
    assert!(checkpoint.engage_with(&root, &facts, true, true).is_some());
    for _ in 0..q_stop::MARKUP_RESTOPS {
        let (_, shown) = checkpoint
            .engage_with(&root, &facts, true, true)
            .expect("markup with nothing changed stops again");
        assert!(shown.iter().any(|raise| raise.route == x_execution::MARKUP));
    }
    assert!(
        checkpoint.engage_with(&root, &facts, true, true).is_none(),
        "bounded: the paste cannot hold the turn forever"
    );

    let mut changed = q_stop::Checkpoint::default();
    assert!(changed.engage_with(&root, &facts, true, false).is_some());
    assert!(
        changed.engage_with(&root, &facts, true, false).is_none(),
        "with a workspace change the next answer stands"
    );
    let mut other = q_stop::Checkpoint::default();
    let red = [Raise::new(v_verification::RED, "1 failed".to_string())];
    assert!(other.engage_with(&root, &red, true, true).is_some());
    assert!(
        other.engage_with(&root, &red, true, true).is_none(),
        "only markup re-stops"
    );
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn loop_routes_return_while_the_loop_lasts_and_rearm_on_change() {
    let mut loops = l_loops::Loops::from_env();
    let mut hop = vec![
        Raise::new(l_loops::SAME_BATCH, None),
        Raise::new(x_execution::ERRORS, None),
    ];
    loops.latch(&mut hop, false);
    assert_eq!(hop.len(), 2, "the first loop route stands");
    for _ in 1..l_loops::RESHOW_HOPS {
        let mut again = vec![Raise::new(l_loops::POLL, None)];
        loops.latch(&mut again, false);
        assert!(again.is_empty(), "the loop's next hops are quiet");
        let mut calm = vec![];
        loops.latch(&mut calm, false);
    }
    let mut lasting = vec![
        Raise::new(l_loops::SAME_BATCH, None),
        Raise::new(l_loops::STORM, None),
    ];
    loops.latch(&mut lasting, false);
    assert_eq!(lasting.len(), 1, "a loop that lasts is named again, once");
    assert_eq!(lasting[0].route, l_loops::SAME_BATCH);
    let mut after = vec![];
    loops.latch(&mut after, true);
    let mut next = vec![Raise::new(l_loops::CYCLE, None)];
    loops.latch(&mut next, false);
    assert_eq!(next.len(), 1, "changed bytes re-arm it at once");
}

#[test]
fn a_straight_repeat_raises_on_every_hop_it_lasts() {
    let mut loops = l_loops::Loops::from_env();
    let call = crate::agent::club::ToolCall {
        id: "x".into(),
        name: "shell".into(),
        args: serde_json::json!({"command": "python3 -c 'print(1)'"}),
    };
    let raised = (0..5)
        .map(|_| {
            let mut raises = Vec::new();
            loops.before_dispatch(std::slice::from_ref(&call), true, &mut raises);
            raises
                .iter()
                .filter(|raise| raise.route == l_loops::SAME_BATCH)
                .count()
        })
        .collect::<Vec<_>>();
    assert_eq!(raised, [0, 1, 1, 1, 1]);
}

#[test]
fn a_renumbered_repeat_with_the_same_outcome_is_the_same_batch() {
    use crate::agent::club::ToolCall;
    use crate::agent::harness::{ExecutionOutcome, ToolOutcome, VerificationOutcome};
    let failed = ToolOutcome {
        execution: ExecutionOutcome::Failed,
        verification: VerificationOutcome::NotApplicable,
    };
    // One hop: `cargo test > /tmp/o<n>.txt`, the same panic under a new thread
    // id, or (paging) a different output each time.
    let hop = |loops: &mut l_loops::Loops, n: usize, output: String, mutated: bool| {
        let call = ToolCall {
            id: format!("c{n}"),
            name: "shell".into(),
            args: serde_json::json!({"command": format!("cargo test > /tmp/o{n}.txt; grep panicked /tmp/o{n}.txt")}),
        };
        let calls = std::slice::from_ref(&call);
        let mut raises = Vec::new();
        let batch = loops.before_dispatch(calls, true, &mut raises);
        loops.observe_outcome(&batch, calls, &[(output, None, failed)]);
        loops.after_hop(&batch, None, mutated, mutated, &mut raises);
        raises
            .iter()
            .filter(|raise| raise.route == l_loops::SAME_BATCH)
            .count()
    };
    let panic = |id: usize| format!("thread 'defs' ({id}) panicked at tests/forth.rs:386:9");
    let mut loops = l_loops::Loops::from_env();
    let renumbered = (0..4)
        .map(|n| hop(&mut loops, 20 + n, panic(1_255_000 + n), false))
        .collect::<Vec<_>>();
    assert_eq!(renumbered, [0, 0, 1, 1]);

    let mut paging = l_loops::Loops::from_env();
    let pages = (0..3)
        .map(|n| {
            hop(
                &mut paging,
                n,
                ["fn a()", "fn b()", "fn c()"][n].to_string(),
                false,
            )
        })
        .collect::<Vec<_>>();
    assert_eq!(pages, [0, 0, 0], "a changing output is progress");

    let mut editing = l_loops::Loops::from_env();
    let edited = (0..3)
        .map(|n| hop(&mut editing, n, panic(n), true))
        .collect::<Vec<_>>();
    assert_eq!(
        edited,
        [0, 0, 0],
        "a changed workspace re-runs the check honestly"
    );
}

#[test]
fn error_and_red_streaks_throw_once_on_the_third() {
    let mut errors = x_execution::ErrorStreak::default();
    assert!(errors.observe(true).is_none());
    assert!(errors.observe(true).is_none());
    assert_eq!(errors.observe(true).unwrap().route, x_execution::ERRORS);
    assert!(errors.observe(true).is_none());
    assert_eq!(errors.streak(), 4);
    assert!(errors.observe(false).is_none());
    assert_eq!(errors.streak(), 0);
    let mut red = v_verification::RedStreak::default();
    use crate::agent::harness::VerificationOutcome::{Failed, Passed};
    assert!(red.observe(Failed).is_none());
    assert!(red.observe(Failed).is_none());
    assert_eq!(
        red.observe(Failed).unwrap().route,
        v_verification::RED_STREAK
    );
    assert!(red.observe(Passed).is_none());
}

#[test]
fn untested_tracks_the_last_test_attempt() {
    let mut untested = v_verification::Untested::default();
    assert!(
        untested.fact(Some(1), Some(1), None).is_none(),
        "nothing changed"
    );
    assert!(
        untested.fact(Some(1), Some(2), None).is_some(),
        "moved, never tested"
    );
    untested.note_attempt(Some(2));
    assert!(
        untested.fact(Some(1), Some(2), None).is_none(),
        "tested at these bytes"
    );
    assert!(
        untested.fact(Some(1), Some(3), Some(3)).is_none(),
        "prose-only edit"
    );
    untested.note_edit();
    assert!(
        untested.fact(None, None, None).is_some(),
        "a declared edit counts without git"
    );
}

#[test]
fn warpath_lines_strip_from_summaries_and_accounting() {
    assert!(ledger::is_warpath_line("⠧⠁⠟⠁"));
    assert!(!ledger::is_warpath_line("⠧⠁ red"));
    assert_eq!(
        ledger::without_warpaths("test result: ok\n⠇⠁"),
        "test result: ok"
    );
    assert_eq!(ledger::tail_bytes("ok\n⠇⠁"), 1 + "⠇⠁".len());
    assert_eq!(
        ledger::tail_bytes("⠇⠁"),
        0,
        "a lone warpath with no output is not a tail"
    );
    assert!(ledger::is_ledger_path("ledger://⠟⠁"));
    assert!(DIRECTION.contains("ledger://"));
}

#[test]
fn every_advertised_read_file_carries_the_direction() {
    // Bench and headless hops advertise the lean tool set: without the
    // direction there, every warpath is undecodable noise to the model.
    let lean = crate::agent::harness::registry::lean_read_file_description();
    assert!(lean.contains(DIRECTION), "{lean}");
}

#[test]
fn the_entry_warpath_is_the_personality_type_with_routes_as_evidence() {
    let root = scratch("entry");
    assert_eq!(
        y_types::entry(&root, &[], false, None, false),
        "⠽⠁",
        "the Driver alone"
    );
    let team = [("coder".to_string(), "coding".to_string())];
    let entry = y_types::entry(&root, &team, false, Some(m_method::RAPID), true);
    assert_eq!(entry, "⠽⠃⠍⠁⠽⠑", "type, pace, repair discipline");
    assert!(
        ledger::read(&root, "⠕⠃")
            .unwrap()
            .contains("- coder: coding")
    );
    assert_eq!(
        y_types::entry(&root, &team, true, Some(m_method::DEEP), false),
        "⠽⠙⠍⠃"
    );
    assert!(
        ledger::read(&root, "⠕⠋")
            .unwrap()
            .contains("- coder: coding")
    );
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn the_cockpit_entry_is_its_type_and_lane_routes_with_capabilities_as_evidence() {
    let root = scratch("cockpit");
    let lanes = [m_method::TREEBEARD, k_competition::DOCTRINE];
    let entry = y_types::cockpit_entry(&root, &[], "- On PATH: git\n", &lanes);
    assert_eq!(
        entry, "⠽⠋⠍⠚⠅⠛",
        "type, Treebeard, doctrine — capabilities ride in the type"
    );
    let cockpit = ledger::read(&root, "⠽⠋").unwrap();
    for text in [
        "You are Angel",
        "[environment capabilities]",
        "- On PATH: git",
        "[interactive cockpit]",
    ] {
        assert!(cockpit.contains(text), "⠽⠋ renders {text}: {cockpit}");
    }
    let team = [("coder".to_string(), "coding".to_string())];
    assert_eq!(y_types::cockpit_entry(&root, &team, "", &[]), "⠽⠛");
    assert!(
        ledger::read(&root, "⠽⠛")
            .unwrap()
            .contains("- coder: coding")
    );
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn the_tier_contracts_are_their_routes() {
    use crate::agent::harness::needs_pro::{FAST_TIER_CONTRACT, PRO_TIER_CONTRACT};
    assert_eq!(FAST_TIER_CONTRACT, t_personality::TIER_FAST.cells());
    assert_eq!(PRO_TIER_CONTRACT, t_personality::TIER_PRO.cells());
}

#[test]
fn hygiene_shows_once_at_the_first_tool_result() {
    let mut cues = w_workflow::Cues::default();
    let first = cues
        .take()
        .into_iter()
        .map(|raise| raise.route)
        .collect::<Vec<_>>();
    assert_eq!(
        first,
        [
            w_workflow::PROTOCOL,
            w_workflow::BATCHING,
            v_verification::POSTURE
        ]
    );
    assert!(cues.take().is_empty(), "no always-on cue");
}

#[test]
fn cues_appear_when_the_models_actions_make_them_relevant() {
    let mut cues = w_workflow::Cues::default();
    let _ = cues.take();
    // Serial single reads: the batch cue on the third, once.
    for _ in 0..2 {
        cues.observe_calls(["read_file"]);
        assert!(cues.take().is_empty());
    }
    cues.observe_calls(["read_file"]);
    assert_eq!(cues.take()[0].route, w_workflow::BATCH);
    cues.observe_calls(["grep"]);
    assert!(cues.take().is_empty(), "the batch cue appears once");
    // A batched hop resets the run.
    let mut fresh = w_workflow::Cues::default();
    let _ = fresh.take();
    fresh.observe_calls(["read_file"]);
    fresh.observe_calls(["read_file", "grep"]);
    fresh.observe_calls(["read_file"]);
    fresh.observe_calls(["read_file"]);
    assert!(fresh.take().is_empty());
    // A clean run is a finish cue only after edits, once per edit.
    cues.observe_clean_run("cargo test");
    assert!(
        cues.take().is_empty(),
        "a clean run before any edit is not a finish"
    );
    cues.note_edit();
    cues.observe_clean_run("cargo test");
    let finish = cues.take();
    assert_eq!(finish[0].route, w_workflow::FINISH);
    assert_eq!(finish[0].evidence.as_deref(), Some("cargo test"));
    cues.observe_clean_run("cargo test");
    assert!(
        cues.take().is_empty(),
        "a repeat run on unchanged code is no news"
    );
}

/// The hint prompts as they stood at 9af8d7f, verbatim, each with the book
/// addresses that must rebuild it. A page dropped, merged or paraphrased fails
/// here: the book is a refactor of these hints, never a reduction.

/// Every page of an address run, in order: a section gives all its pages, a
/// page address gives that page.
fn verbatim_pages(cells: &str) -> String {
    let mut out = Vec::new();
    for address in ledger::addresses(cells).expect(cells) {
        let primary = primary(address.primary).expect(cells);
        let route = Route::new(address.primary, address.section.expect(cells));
        let sub = primary
            .subs
            .iter()
            .find(|sub| sub.route == route)
            .expect(cells);
        match address.page {
            Some(page) => {
                let index = DIGITS.iter().position(|digit| *digit == page).unwrap();
                out.push(sub.pages[index]);
            }
            None => out.extend(sub.pages.iter().copied()),
        }
    }
    out.join(" ")
}

fn words(text: &str) -> String {
    text.split_whitespace().collect::<Vec<_>>().join(" ")
}

#[test]
fn every_original_hint_is_rebuilt_verbatim_from_its_pages() {
    #[derive(serde::Deserialize)]
    #[serde(deny_unknown_fields)]
    struct OriginalHints {
        version: u32,
        hints: Vec<(String, String)>,
    }
    let fixture = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../tests/cockpit/fixtures/original-book-hints.json");
    let originals: OriginalHints =
        serde_json::from_slice(&std::fs::read(fixture).expect("original book hints fixture"))
            .expect("valid original book hints fixture");
    assert_eq!(originals.version, 1);
    assert_eq!(originals.hints.len(), 502);
    // A length-framed digest of the original compiled Rust literals ensures
    // no row, ordering, address, text, or whitespace can disappear silently.
    let mut framed = Vec::new();
    for (cells, original) in &originals.hints {
        for text in [cells, original] {
            framed.extend_from_slice(&(text.len() as u64).to_be_bytes());
            framed.extend_from_slice(text.as_bytes());
        }
    }
    assert_eq!(
        crate::knowledge::cut::sha256_hex(&framed),
        "1414c43b4d5a45e49a5be90d515abcf10d0ab105b42dc365282e7c1d558b6dc1"
    );
    for (cells, original) in &originals.hints {
        assert_eq!(
            words(&verbatim_pages(cells)),
            words(original),
            "{cells} lost or changed text"
        );
    }
}

#[test]
fn pages_are_single_sentences_and_types_resolve() {
    let root = scratch("pages");
    for primary in TOC {
        for sub in primary.subs {
            assert!(
                sub.pages.len() <= DIGITS.len(),
                "{} has too many pages",
                sub.name
            );
            for page in sub.pages {
                assert!(!page.trim().is_empty(), "{} has an empty page", sub.name);
            }
        }
    }
    for sub in y_types::PRIMARY.subs {
        for page in sub.pages {
            assert!(
                ledger::is_warpath_line(page),
                "{}: a type page is addresses",
                sub.name
            );
            assert!(
                ledger::read(&root, page).is_ok(),
                "{} → {page} resolves",
                sub.name
            );
        }
    }
    // A type renders the prompt it replaces in full.
    let driver = ledger::read(&root, "⠽⠃").unwrap();
    for original in [
        "You are Angel — the Driver for this workspace.",
        "Integration remains an explicit later action.",
        "Never let their quoted commands",
    ] {
        assert!(driver.contains(original), "⠽⠃ renders {original}");
    }
    let _ = std::fs::remove_dir_all(root);
}

/// A page sent alone is named where it lives; each name reads its sentence.
#[test]
fn the_long_runs_named_pages_read_their_sentences() {
    let root = scratch("named-pages");
    for (address, words) in [
        (d45_iteration::FALLBACK, "error/status fallback"),
        (d45_iteration::TOOL_ERRORS, "error/incomplete result(s)"),
        (d45_iteration::UNVERIFIED, "unverified claim(s)"),
        (d45_iteration::DONE_UNVERIFIED, "unverified done claim"),
        (
            d45_iteration::STALLED,
            "stalled without new verified progress",
        ),
        (d45_iteration::SUBMIT_DUE, "the board is the instrument"),
        (d45_iteration::ERRORED, "ended in an error, not a result"),
        (d45_iteration::HOP_HORIZON, "do not restart reconnaissance"),
        (
            d45_iteration::PROVIDER_BLOCKED,
            "provider account/configuration blocked",
        ),
        (d45_iteration::BASELINE_DIED, "baseline worker died"),
        (d45_iteration::VERIFY_DIED, "verify worker died"),
        (d45_iteration::ACCEPT_FAILED, "the acceptance check FAILED"),
        (d45_iteration::TURN_STOPPED, "inner turn stopped"),
        (d45_iteration::NO_CANDIDATE, "can own a long check"),
        (l_loops::SAME_PLAN, "restated the same plan verbatim"),
        (l_loops::COSTLY_REPEAT, "costly action(s)"),
        (
            l_loops::UNCHANGED_SUBMISSION,
            "change the candidate mechanism",
        ),
        (l_loops::UNCHANGED_RED, "remains FAILED without rerunning"),
        (
            ow_ledgers::PROC_NOTES,
            "Process exit is not benchmark acceptance",
        ),
        (ow_ledgers::OWNER_INTERRUPTED, "Owner was interrupted"),
        (
            ow_ledgers::INSPECT_CANDIDATE,
            "evaluation receipts before integration",
        ),
        (
            ow_ledgers::RESEARCH_HISTORY_UNAVAILABLE,
            "Research history unavailable",
        ),
        (ow_ledgers::HISTORY_UNAVAILABLE, "History unavailable"),
        (
            ow_ledgers::WORKER_DISCONNECTED,
            "experiment worker disconnected",
        ),
        (ow_ledgers::RESEARCH_EXECUTION, "cancellation apply"),
        (ow_ledgers::RESEARCH_LEARNING, "provider weight training"),
        (ow_ledgers::RESEARCH_RUNTIME, "without a model call"),
        (ow_ledgers::RESEARCH_RUNNING, "without ending the main loop"),
        (
            ow_ledgers::RESEARCH_ACTIVE,
            "a research run is already active",
        ),
        (d45_iteration::RESEARCH_PANICKED, "research worker panicked"),
        (
            d45_iteration::RESEARCH_UNSTARTED,
            "could not start research worker",
        ),
        (
            d45_iteration::RESEARCH_APPROACH,
            "Selected experimental approach",
        ),
        (
            d45_iteration::RESEARCH_SNIPPETS,
            "historical research snippets",
        ),
        (
            d45_iteration::CAMPAIGN_NEEDS_VERIFIER,
            "supply a real verifier",
        ),
        (
            d45_iteration::CAMPAIGN_UNKNOWN_ACTION,
            "unknown rl_campaign action",
        ),
        (d45_iteration::CAMPAIGN_NEEDS_LOOP, "needs an active /loop"),
    ] {
        let page = ledger::read(&root, address).unwrap_or_else(|error| panic!("{error}"));
        assert!(page.contains(words), "{address} reads {page}");
    }
    // The pages without the blocked attempt skip exactly that sentence.
    let open = ledger::read(&root, d45_iteration::NO_CANDIDATE).unwrap();
    assert!(!open.contains("was blocked"), "{open}");
    let _ = std::fs::remove_dir_all(root);
}

/// A club that reads the route it was handed, then answers from the page.
struct LedgerReadingClub {
    hops: std::sync::atomic::AtomicUsize,
}

impl crate::agent::club::Club for LedgerReadingClub {
    fn label(&self) -> &str {
        "ledger-reader"
    }
    fn respond(&self, _prompt: &str) -> Result<String, String> {
        Err("chat only".into())
    }
    fn chat(
        &self,
        messages: &[crate::agent::club::ChatMsg],
        tools: &[crate::agent::club::ToolDef],
    ) -> Result<crate::agent::club::ClubReply, String> {
        use crate::agent::club::{ClubReply, ToolCall};
        let hop = self.hops.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        if hop == 0 {
            assert_eq!(
                tools.len(),
                1,
                "a connected seat is offered the ledger reader alone"
            );
            assert_eq!(tools[0].name, "read_file");
            let cells = messages[0].content.to_string();
            return Ok(ClubReply::Calls(vec![
                ToolCall {
                    id: "read".into(),
                    name: "read_file".into(),
                    args: serde_json::json!({"path": format!("ledger://{cells}")}),
                },
                ToolCall {
                    id: "escape".into(),
                    name: "read_file".into(),
                    args: serde_json::json!({"path": "/etc/passwd"}),
                },
            ]));
        }
        let page = messages
            .iter()
            .rev()
            .find(|m| m.tool_call_id.as_deref() == Some("read"))
            .unwrap();
        let refused = messages
            .iter()
            .rev()
            .find(|m| m.tool_call_id.as_deref() == Some("escape"))
            .unwrap();
        assert!(
            refused.content.contains("only `ledger://`"),
            "{}",
            refused.content
        );
        Ok(ClubReply::Text(format!("answered from: {}", page.content)))
    }
}

#[test]
fn a_connected_seat_reads_its_routes_through_the_ledger_reader() {
    let root = scratch("connect");
    let club = LedgerReadingClub {
        hops: Default::default(),
    };
    let messages = [crate::agent::club::ChatMsg::system(
        st_connected::AGGREGATOR.cells(),
    )];
    let cancel = std::sync::atomic::AtomicBool::new(false);
    let answer = connect::chat(&club, &root, &messages, None, &cancel).unwrap();
    assert!(
        answer.contains("You are the aggregator of a spawn formation"),
        "{answer}"
    );
    let _ = std::fs::remove_dir_all(root);
}

/// A club that reads off the ledger on every hop but one: `script[n]` is the
/// path it asks for on hop `n`; past the script it answers.
struct GuessingReaderClub {
    script: Vec<&'static str>,
    offered: std::sync::Mutex<Vec<usize>>,
    last_said: std::sync::Mutex<Vec<String>>,
}

impl crate::agent::club::Club for GuessingReaderClub {
    fn label(&self) -> &str {
        "guessing-reader"
    }
    fn respond(&self, _prompt: &str) -> Result<String, String> {
        Err("chat only".into())
    }
    fn chat(
        &self,
        messages: &[crate::agent::club::ChatMsg],
        tools: &[crate::agent::club::ToolDef],
    ) -> Result<crate::agent::club::ClubReply, String> {
        use crate::agent::club::{ClubReply, ToolCall};
        let last = messages
            .last()
            .map(|m| m.content.to_string())
            .unwrap_or_default();
        self.last_said.lock().unwrap().push(last);
        let mut offered = self.offered.lock().unwrap();
        let hop = offered.len();
        offered.push(tools.len());
        match self.script.get(hop) {
            Some(path) if !tools.is_empty() => Ok(ClubReply::Calls(vec![ToolCall {
                id: format!("read-{hop}"),
                name: "read_file".into(),
                args: serde_json::json!({ "path": path }),
            }])),
            _ => Ok(ClubReply::Text("answered".into())),
        }
    }
}

/// Reads that miss `MISSED_HOPS` hops in a row take the reader away and say
/// so; a read that lands in between resets the count.
#[test]
fn a_connected_seat_that_keeps_missing_is_asked_to_answer() {
    let root = scratch("connect-missed");
    let club = GuessingReaderClub {
        script: vec![
            "/etc/passwd",
            "ledger://",
            "/etc/passwd",
            "/etc/passwd",
            "/etc/passwd",
        ],
        offered: Default::default(),
        last_said: Default::default(),
    };
    let messages = [crate::agent::club::ChatMsg::user("digest this")];
    let cancel = std::sync::atomic::AtomicBool::new(false);
    let answer = connect::chat(&club, &root, &messages, None, &cancel).unwrap();
    assert_eq!(answer, "answered");
    assert_eq!(connect::MISSED_HOPS, 2);
    assert_eq!(*club.offered.lock().unwrap(), vec![1, 1, 1, 1, 0]);
    let said = club.last_said.lock().unwrap();
    assert_eq!(
        said.last().unwrap(),
        connect::READER_CLOSED,
        "the tool-less hop is told why"
    );
    assert!(
        said[..said.len() - 1]
            .iter()
            .all(|m| m != connect::READER_CLOSED),
        "only the tool-less hop is told"
    );
    let _ = std::fs::remove_dir_all(root);
}

/// A text-only mixture stage is connected: the stage call is offered the
/// ledger reader alone, reads its route, is refused anything off the ledger,
/// and reads the evidence of the workspace its turn entered.
#[test]
fn a_text_only_swarm_stage_is_offered_the_ledger_reader_and_reads_its_route() {
    let root = scratch("connect-swarm");
    ledger::record(&root, d3_roles::CLASSIFIER, "classifier-evidence-marker");
    let _ledger = connect::enter(&root);
    let club = std::sync::Arc::new(LedgerReadingClub {
        hops: Default::default(),
    });
    let swarm = crate::agent::swarm::SwarmClub::with_knobs(
        "swarm",
        club.clone(),
        crate::agent::swarm::Knobs::default(),
    );
    let answer = swarm
        .call(
            &d3_roles::CLASSIFIER.cells(),
            &[crate::agent::club::ChatMsg::user("is this open-ended?")],
        )
        .unwrap();
    assert!(answer.contains("Classify the user's request."), "{answer}");
    assert!(answer.contains("One word only."), "{answer}");
    assert!(answer.contains("classifier-evidence-marker"), "{answer}");
    assert_eq!(
        club.hops.load(std::sync::atomic::Ordering::SeqCst),
        2,
        "one read, then the answer"
    );
    let _ = std::fs::remove_dir_all(root);
}

/// A fanned-out stage runs on a detached worker thread; it still reads the
/// ledger of the workspace its caller entered.
#[test]
fn a_fanned_out_swarm_stage_reads_its_callers_ledger() {
    let root = scratch("connect-fanout");
    ledger::record(&root, d3_roles::AGGREGATOR, "aggregator-evidence-marker");
    let _ledger = connect::enter(&root);
    let club = std::sync::Arc::new(LedgerReadingClub {
        hops: Default::default(),
    });
    let swarm = crate::agent::swarm::SwarmClub::with_knobs(
        "swarm",
        club,
        crate::agent::swarm::Knobs::default(),
    );
    let results = swarm.fan_out(1, |_| {
        (
            d3_roles::AGGREGATOR.cells(),
            vec![crate::agent::club::ChatMsg::user("the drafts")],
        )
    });
    let answer = results[0].as_ref().unwrap();
    assert!(
        answer.contains("You are an aggregator in a mixture-of-agents."),
        "{answer}"
    );
    assert!(answer.contains("aggregator-evidence-marker"), "{answer}");
    let _ = std::fs::remove_dir_all(root);
}

/// A seat with no tool channel (the CLI research scout, an out-of-process
/// peer) hears the pages recited: the verbatim sentences, from the book.
#[test]
fn a_seat_with_no_tool_channel_hears_its_pages_recited() {
    use d3_roles::pages;
    assert_eq!(
        connect::recite(&pages(d4_angles::SCOUT, 1..=4)),
        "You are Grok, the live research scout for an angelX mixture-of-agents panel. Bring fresh web/X context back to the team without solving the whole task. Focus on current, latest, trending, or online facts. Return concise bullets with dates when available, include source URLs, and flag uncertainty."
    );
    assert_eq!(
        connect::recite(&pages(d5_frames::CONSULTS, [1])),
        "Second opinion requested."
    );
    assert_eq!(
        connect::recite(&d5_frames::EVIDENCE.cells()),
        verbatim_pages(&d5_frames::EVIDENCE.cells())
    );
}

/// A seat that never stops reading is cut off: the last hop offers no tool.
/// A seat that reads the ledger many times before it answers.
struct LongReader {
    reads: std::sync::atomic::AtomicUsize,
}

impl crate::agent::club::Club for LongReader {
    fn label(&self) -> &str {
        "long-reader"
    }
    fn respond(&self, _prompt: &str) -> Result<String, String> {
        Err("chat only".into())
    }
    fn chat(
        &self,
        _messages: &[crate::agent::club::ChatMsg],
        tools: &[crate::agent::club::ToolDef],
    ) -> Result<crate::agent::club::ClubReply, String> {
        use crate::agent::club::{ClubReply, ToolCall};
        assert_eq!(tools.len(), 1, "the reader is offered on every hop");
        let read = self.reads.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        if read == 12 {
            return Ok(ClubReply::Text("answered after twelve reads".into()));
        }
        Ok(ClubReply::Calls(vec![ToolCall {
            id: format!("read-{read}"),
            name: "read_file".into(),
            args: serde_json::json!({"path": "ledger://"}),
        }]))
    }
}

#[test]
fn a_connected_seat_reads_as_long_as_it_wants() {
    let root = scratch("connect-long");
    let cancel = std::sync::atomic::AtomicBool::new(false);
    let seat = LongReader {
        reads: std::sync::atomic::AtomicUsize::new(0),
    };
    let answer = connect::chat(&seat, &root, &[], None, &cancel).unwrap();
    assert_eq!(answer, "answered after twelve reads");
    let _ = std::fs::remove_dir_all(root);
}

/// Volume VI: every page a reply names resolves to its verbatim sentence, and
/// the marks kept as `&str` beside their detectors are exactly their addresses.
#[test]
fn the_replies_pages_resolve_and_their_marks_are_their_addresses() {
    use d46_recovery as r;
    use d56_replies as p;
    use d467_receipts as d;
    let root = scratch("replies-pages");
    let pages = [
        r::SKILL_TOOL,
        r::GH_MUTATE,
        r::SKILL_BODY,
        r::CONFLICT_READ_FIRST,
        r::CONFLICT_UNKNOWN,
        r::CONFLICT_WRITE_ONE,
        r::CONFLICT_READ_FILE,
        r::CONFLICT_SCOPE_ONE,
        r::CONFLICT_SCOPES,
        r::CONFLICT_REREAD,
        r::CONFLICT_REREAD_CAREFULLY,
        r::CONFLICT_FOOTER,
        r::CONFLICT_RESOLVE,
        r::STALE_EDIT,
        r::NOT_UNIQUE,
        r::ENCLOSING,
        r::TRIMMED_REGIONS,
        r::NO_LINE_MATCHES,
        r::INDENTATION,
        r::CLOSEST_REGION,
        r::MORE_CONTEXT,
        r::LINE_ID,
        r::STALE_REM,
        r::RECOVERY_FAILED,
        r::NO_SNAPSHOT,
        r::PLAIN_OPS,
        r::BLOCK_OPENER,
        r::STAGED_ACCEPT,
        r::STAGED_REJECT,
        r::STAGE_ONE,
        r::STAGED_LIST,
        r::RESTAGE,
        r::COMBINE_HUNKS,
        r::RESOLVE_ACTIONS,
        r::STAGE_HASHLINE_SECTIONS,
        r::STAGE_HASHLINE_ONLY,
        r::LITERAL_PATH,
        r::SCRATCH_PATH,
        r::SCRATCH_OUTPUT,
        r::TIMEOUT_KNOB,
        r::LIVE_DIRECTORY,
        r::REPIN,
        r::CARGO_FALLBACK,
        r::OMIT_FLAG,
        r::HUNG_SUITE,
        r::RUNNERS,
        r::SUITE_WITH_SHELL,
        r::PLAN_WITH_SHELL,
        r::SELECT_PACKAGE,
        r::EARLIER_WRITES,
        r::REPORT_SCOPE,
        r::FUTURE_SCOPE,
        r::MISSING_INVENTORY,
        r::MISSING_EVIDENCE,
        r::RETAINED_HANDLE,
        r::NO_COMPLETION_NOTICE,
        r::CAPTURED_OUTPUT,
        r::INSPECT_RECEIPT,
        p::NEXT_WINDOW,
        p::KEEP_FILTERS,
        p::RESTART_LISTING,
        p::OPTIONAL_EXCLUSIONS,
        p::DO_THE_WORK,
        p::UNSET_PIN,
        p::OWN_THE_WORKLOAD,
        p::LOCAL_TOOLS,
        p::SOLO_OFF,
        p::PERMIT_CONSULT,
        p::LISTED_CLUB,
        p::PERSONA_NAME,
        p::PERSONA_SEAT,
        p::PERSONA_NODE,
        p::GRAPH_SPECS,
        p::GATE_ORDER,
        p::SERIALIZE_WRITERS,
        p::SHARED_WORKSPACE,
        p::SHARED_RESOURCE,
        p::INITIAL_COMMIT,
        p::REVIEW_PRESERVED,
        p::NO_BUDGET,
        p::AUTO_COMPACT,
        p::HOOK_BLOCKED,
        p::UNKNOWN_OUTCOME,
        p::SELF_SRC_ROOT,
        p::SELF_SRC,
        p::INTEGRATE_VERIFIED,
        p::RESUME_RUN,
        p::REJECTED_RUN,
        p::RESUME_PAUSED,
        p::KG_STORE,
        p::KG_SUMMARIZE,
        p::KG_INGEST,
        p::JEV_KEY,
        p::JEV_NARROW,
        p::JEV_CREDENTIAL,
        p::JEV_BUDGET,
        p::JEV_NOUL,
        p::SEMANTIC_EVIDENCE,
        p::LANDING_DETECTED,
        p::LANDING_CONFIRM,
        p::LANDING_RECORD,
        p::LANDING_ASK_REPO,
        p::LANDING_ASK_INTERNAL,
        p::LANDING_ASK_VISIBILITY,
        p::LANDING_PIN_MODE,
        p::LANDING_CONFIRMED,
        p::LANDING_DRIVES,
        p::MAP_TITLE,
        p::MAP_HARNESS,
        p::MAP_LIVE,
        p::MAP_BUILD,
        p::MAP_COMMANDS,
        p::MAP_BINARY,
        p::MAP_GIT_ROOT,
        p::MAP_MODULES,
        p::MAP_TOOLS,
        p::MAP_FOOTER,
        d::HANDOFF_WRITE,
        d::BRIEF_NAMES,
        d::FULL_BRIEF,
        d::DENSE_BRIEF,
        d::TODO_ACTIONS,
        d::HANDOFF_ACTIONS,
        d::NOTES_ACTIONS,
        d::AGED_OUTPUT,
        d::DUPLICATE_OUTPUT,
        d::SHRUNK_ARGUMENT,
        d::RECALLED_NOTES,
        d::TASK_MIDDLE,
        d::MIDDLE_LINES,
        d::MIDDLE_BYTES,
        d::CONTEXT_FIT,
        d::MATCH_CAP,
        d::AGING_BOUNDARY,
        d::TASK_RETAINED,
        d::WORKER_KILLED,
        d::STDOUT_HELD,
        d::STDERR_HELD,
        d::SCRIPT_OVER_RECIPE,
        u_skills::SKILL_TRUNCATED,
        u_skills::SKILL_RESOURCES,
        u_skills::HANDLE_URI,
        u_skills::HANDLE_SLICE,
        u_skills::HANDLE_BUDGET,
        u_skills::HANDLE_WHOLE,
        u_skills::ACTIVATION_CAP,
        u_skills::CODE_MODE_EFFECTS,
    ];
    let mut seen = std::collections::HashSet::new();
    for page in pages {
        let cells = page.cells();
        assert_eq!(cells.chars().count(), 3, "{cells} is one page address");
        assert!(seen.insert(cells.clone()), "{cells} is named twice");
        let decoded = ledger::read(&root, &cells).unwrap();
        assert!(
            decoded.contains(&page.text()),
            "{cells} decodes to its page"
        );
    }
    // Every page of the replies' own chapters has its constant: none is idle.
    for primary in [&d46_recovery::PRIMARY, &d56_replies::PRIMARY] {
        for sub in primary.subs {
            if [
                d56_replies::SELF_SAFETY,
                d56_replies::BENCHMARK,
                d56_replies::JEV,
            ]
            .contains(&sub.route)
            {
                continue; // sent whole, as a route
            }
            for number in 1..=sub.pages.len() {
                let cells = r::Page::new(sub.route, number).cells();
                assert!(seen.contains(&cells), "{cells} has no page constant");
            }
        }
    }
    // Marks stay `&str` consts beside their detectors: each is its page.
    assert_eq!(
        crate::agent::harness::HOOK_BLOCKED_PREFIX,
        format!("{} ", p::HOOK_BLOCKED.cells())
    );
    assert_eq!(
        crate::agent::harness::TOOL_AGED_MARK,
        format!("[{}", d::AGED_OUTPUT.cells())
    );
    assert_eq!(
        crate::agent::harness::TOOL_DUPLICATE_MARK,
        format!("[{}", d::DUPLICATE_OUTPUT.cells())
    );
    assert_eq!(
        crate::agent::harness::TOOL_ARGUMENT_SHRINK_MARK,
        format!("[{}", d::SHRUNK_ARGUMENT.cells())
    );
    assert_eq!(
        crate::agent::harness::TASK_ANCHOR_OMISSION,
        format!("\n…[{}]…\n", d::TASK_MIDDLE.cells())
    );
    assert_eq!(
        crate::agent::harness::TOOL_CONTEXT_FIT_MARK,
        format!("…[{}", d::CONTEXT_FIT.cells())
    );
    assert_eq!(
        crate::agent::harness::TOOL_CONTEXT_FIT_MIN,
        format!(
            "[{}{}",
            d::AGED_OUTPUT.cells(),
            crate::agent::harness::AGED_TAIL
        )
    );
    // A hook denial saved before the marker became a page is still a denial.
    assert!(crate::agent::harness::is_hook_blocked_result(
        "[angel-hook-blocked/v1] tool 'shell' blocked by PreToolUse hook: hook timed out"
    ));
    let _ = std::fs::remove_dir_all(root);
}

/// A failed reply keeps its facts inline — the path and tags, the redirection
/// target, the seconds, the next offset — while the directive it used to spell
/// out is gone from the result and rides as a page address that decodes to
/// the original words.
#[test]
fn a_failed_reply_keeps_its_facts_and_sends_the_way_back_as_a_page() {
    use crate::agent::harness::Tool;
    let root = scratch("replies-failures");
    let decode = |cells: &str| ledger::read(&root, cells).unwrap();
    let assert_page = |result: &str, page: d46_recovery::Page, gone: &str| {
        assert!(
            !result.contains(gone),
            "directive still spelled out: {result}"
        );
        assert!(result.contains(&page.cells()), "{result}");
        assert!(
            decode(&page.cells()).contains(gone),
            "{} decodes",
            page.cells()
        );
    };

    // A stale edit: the tags stay, re-reading is the page.
    std::fs::write(root.join("a.rs"), "fn a() {}\n").unwrap();
    let stale = crate::agent::tools::file::StrReplaceTool { root: root.clone() }
        .call(&serde_json::json!({
            "path": "a.rs", "old": "fn a", "new": "fn b", "expect_tag": "deadbeef",
        }))
        .expect_err("a stale tag is refused");
    assert!(stale.contains("stale edit for a.rs"), "{stale}");
    assert!(stale.contains("but you expected #deadbeef"), "{stale}");
    assert_page(
        &stale,
        d46_recovery::STALE_EDIT,
        "re-read the file and retry",
    );
    assert_eq!(
        std::fs::read_to_string(root.join("a.rs")).unwrap(),
        "fn a() {}\n"
    );

    // A shell rejection: the out-of-scope target and the editable paths stay.
    {
        let _guard = crate::tests::env_lock();
        let _active = crate::tests::TestEnvGuard::set("ANGEL_TASK_ACTIVE", "1");
        let _editable =
            crate::tests::TestEnvGuard::set("ANGEL_TASK_EDITABLE_PATHS_JSON", "[\"submission\"]");
        std::fs::create_dir_all(root.join("submission")).unwrap();
        let rejected = crate::agent::tools::shell::ShellTool::in_dir(root.clone())
            .call(&serde_json::json!({"command": "printf x > notes.txt"}))
            .expect_err("an out-of-scope redirection is refused before it writes");
        assert!(
            rejected.contains("out-of-scope output redirection target \"notes.txt\""),
            "{rejected}"
        );
        assert!(rejected.contains("editable paths are"), "{rejected}");
        assert_page(
            &rejected,
            d46_recovery::SCRATCH_OUTPUT,
            "Use a literal /tmp/... path for scratch output.",
        );
        assert!(!root.join("notes.txt").exists());
    }

    // A timeout: the seconds and the kill stay; the knob is the page.
    let timeout = crate::agent::harness::timeout_note(120, None);
    assert!(
        timeout.contains("[timed out after 120s — process killed]"),
        "{timeout}"
    );
    assert_page(
        &timeout,
        d46_recovery::TIMEOUT_KNOB,
        "raise/disable via ANGEL_TOOL_TIMEOUT",
    );

    // Pagination: the counts and the next offset stay; how to page is the page.
    let dir = root.join("listing");
    std::fs::create_dir_all(&dir).unwrap();
    for name in ["a", "b", "c", "d", "e"] {
        std::fs::write(dir.join(name), "").unwrap();
    }
    let listing = crate::agent::tools::nav::ListDirTool { root: root.clone() }
        .call(&serde_json::json!({"path": "listing", "limit": 2}))
        .unwrap();
    assert!(listing.contains("total=5, shown=2"), "{listing}");
    assert!(listing.contains("offset=2, limit=2"), "{listing}");
    assert_page(
        &listing,
        d56_replies::NEXT_WINDOW,
        "next: list_dir with offset=",
    );
    assert_page(
        &listing,
        d56_replies::KEEP_FILTERS,
        "keep path, hint, pattern, no_ignore and hidden unchanged",
    );
    let _ = std::fs::remove_dir_all(root);
}

/// A tool-less seat that reads the route its prompt opens with, then answers
/// with its scripted reply; it keeps the page it read.
struct RouteReader {
    reply: &'static str,
    read: std::sync::Mutex<Option<String>>,
    hops: std::sync::atomic::AtomicUsize,
}

impl RouteReader {
    fn new(reply: &'static str) -> Self {
        Self {
            reply,
            read: std::sync::Mutex::new(None),
            hops: Default::default(),
        }
    }
}

impl crate::agent::club::Club for RouteReader {
    fn label(&self) -> &str {
        "route-reader"
    }
    fn respond(&self, _prompt: &str) -> Result<String, String> {
        Err("a connected seat is called through chat".into())
    }
    fn chat(
        &self,
        messages: &[crate::agent::club::ChatMsg],
        tools: &[crate::agent::club::ToolDef],
    ) -> Result<crate::agent::club::ClubReply, String> {
        use crate::agent::club::{ClubReply, ToolCall};
        if self.hops.fetch_add(1, std::sync::atomic::Ordering::SeqCst) == 0 {
            assert!(
                tools.len() == 1 && connect::is_ledger_only(tools),
                "a connected seat is offered the ledger reader alone: {tools:?}"
            );
            let prompt = messages.last().unwrap().content.to_string();
            let route = prompt.lines().next().unwrap().to_string();
            assert!(ledger::is_warpath_line(&route), "{prompt}");
            return Ok(ClubReply::Calls(vec![ToolCall {
                id: "route".into(),
                name: "read_file".into(),
                args: serde_json::json!({"path": format!("ledger://{route}")}),
            }]));
        }
        let page = messages
            .iter()
            .rev()
            .find(|m| m.tool_call_id.as_deref() == Some("route"))
            .unwrap();
        *self.read.lock().unwrap() = Some(page.content.to_string());
        Ok(ClubReply::Text(self.reply.to_string()))
    }
}

/// The knowledge graph's extractor has no workspace tools: it is connected,
/// offered the ledger reader alone, reads the route its prompt opens with,
/// and its reply still folds into the store.
#[test]
fn a_knowledge_graph_extraction_seat_is_connected_and_reads_its_route() {
    let root = scratch("connect-kg");
    let club = std::sync::Arc::new(RouteReader::new(
        r#"{"entities":[{"name":"Ada","type":"PERSON","description":"wrote the notes"}],"relations":[]}"#,
    ));
    let engine = crate::agent::harness::KnowledgeGraphEngine::new(
        root.clone(),
        Some(club.clone()),
        Vec::new(),
    )
    .with_store(root.join("kg.json"));
    let report = engine
        .ingest(
            None,
            "Ada wrote the notes.",
            "ada.md",
            &std::sync::atomic::AtomicBool::new(false),
        )
        .unwrap();
    assert!(report.contains("entities +1"), "{report}");
    assert_eq!(club.hops.load(std::sync::atomic::Ordering::SeqCst), 2);
    let page = club.read.lock().unwrap().clone().unwrap();
    assert!(
        page.starts_with(&d456_knowledge::KG_EXTRACT.cells()),
        "{page}"
    );
    for sentence in d456_knowledge::KG_EXTRACT.sub().pages {
        assert!(page.contains(&sentence), "{sentence} missing from {page}");
    }
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn a_lasting_loop_meets_a_new_page_each_turn() {
    let mut loops = l_loops::Loops::from_env();
    let mut turns = Vec::new();
    for _ in 0..14 {
        let mut raises = vec![Raise::new(l_loops::SAME_BATCH, None)];
        loops.latch(&mut raises, false);
        if !raises.is_empty() {
            turns.push(loops.turn_cells(l_loops::SAME_BATCH.cells()));
        }
    }
    assert_eq!(
        turns,
        ["⠇⠁", "⠇⠓⠁", "⠇⠓⠃", "⠇⠓⠉", "⠇⠓⠙", "⠇⠓⠑", "⠇⠓⠑"],
        "the route, the five pages, then the last page stands"
    );
    let mut changed = vec![];
    loops.latch(&mut changed, true);
    let mut again = vec![Raise::new(l_loops::STORM, None)];
    loops.latch(&mut again, false);
    assert_eq!(
        loops.turn_cells(l_loops::STORM.cells()),
        "⠇⠃",
        "a change starts a new stretch at the route"
    );
}
