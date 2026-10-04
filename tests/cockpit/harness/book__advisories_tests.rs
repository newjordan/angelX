//! Advisory routing, rendering, deduplication, and live detector behavior.
use crate::agent::club::{ChatMsg, ToolCall};
use crate::agent::harness::book::{
    self, DIGITS, Raise, Route, introduction, k_competition, l_loops, q_stop, v_verification,
    x_execution,
};
use crate::agent::harness::{TaskPace, book::ledger};

fn pages_of(cells: &str) -> String {
    let mut out = Vec::new();
    for address in ledger::addresses(cells).expect(cells) {
        let primary = book::primary(address.primary).expect(cells);
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

fn legend_of(messages: &[ChatMsg]) -> Vec<String> {
    introduction::introductions(messages, None)
        .into_iter()
        .map(|(_, text)| text)
        .collect()
}

#[test]
fn the_poll_advice_keeps_the_passive_wait_instruction_and_drops_the_suppression() {
    let poll = words(&pages_of("⠇⠉"));
    for sentence in [
        "Status snapshots and shell sleeps are observations, not candidate progress.",
        "A submission without a terminal receipt still needs an explicit result check; never assume a watcher will deliver it.",
        "Use the next status check when it can inform a decision; avoid tight repeated polls with no intervening work.",
    ] {
        assert!(poll.contains(sentence), "{sentence}");
    }
    assert!(!poll.contains("not started"), "{poll}");
}

#[test]
fn voiced_routes_are_on_the_tree_and_speak_in_full() {
    for (route, _) in book::VOICED {
        let sub = route.sub();
        assert!(
            sub.action.is_empty(),
            "{} has an action the legend would hide",
            route.name()
        );
        assert!(!sub.pages.is_empty(), "{}", route.name());
        assert!(
            sub.pages
                .iter()
                .all(|page| !page.contains("[harness-telemetry]")),
            "{}",
            route.name()
        );
        // Met at first sight with every page, once.
        let messages = [
            ChatMsg::user("go".to_string()),
            ChatMsg::harness(route.cells()),
            ChatMsg::harness(route.cells()),
        ];
        let intros = legend_of(&messages);
        assert_eq!(intros.len(), 1, "{}: once", route.name());
        let intro = words(&intros[0]);
        assert!(intro.contains(&route.cells()), "{intro}");
        for page in sub.pages {
            let unslotted = page
                .split('{')
                .enumerate()
                .map(|(i, part)| {
                    if i == 0 {
                        part.to_string()
                    } else {
                        part.split_once('}')
                            .map_or(part.to_string(), |(_, rest)| format!("…{rest}"))
                    }
                })
                .collect::<String>();
            assert!(
                intro.contains(&words(&unslotted)),
                "{}: {page}",
                route.name()
            );
        }
        // A page of it met later is a new sentence, not a bare repeat.
        let again = [
            ChatMsg::harness(route.cells()),
            ChatMsg::harness(format!("{}{}", route.cells(), DIGITS[0])),
        ];
        let intros = legend_of(&again);
        assert_eq!(intros.len(), 2, "{}: {intros:?}", route.name());
        assert!(
            intros[1].starts_with(&format!("{}{} ", route.cells(), DIGITS[0])),
            "{intros:?}"
        );
    }
}

#[test]
fn a_warning_leads_with_the_sign_and_guidance_does_not() {
    for route in [
        l_loops::SAME_BATCH,
        x_execution::ERRORS,
        v_verification::RED_STREAK,
        book::d3456_advisories::THRASH,
        book::d3456_advisories::CASCADE,
    ] {
        assert!(book::is_warning(route), "{}", route.name());
    }
    for route in [
        book::d3456_advisories::POST_EDIT,
        book::d3456_advisories::GREEN,
        book::d3456_advisories::FINAL_MILE,
        v_verification::RED,
        v_verification::UNTESTED,
        k_competition::WINNER_BANK,
    ] {
        assert!(
            book::is_voiced(route) && !book::is_warning(route),
            "{}",
            route.name()
        );
    }
    assert!(
        !book::is_voiced(v_verification::POST_WRITE),
        "a check's diagnostics stay a tail"
    );
}

#[test]
fn ported_advisories_keep_their_0_1_6_names_for_the_analysis() {
    let names = [
        (l_loops::SAME_BATCH, "spin_advisory"),
        (x_execution::ERRORS, "error_advisory"),
        (v_verification::RED_STREAK, "verification_recovery"),
        (
            book::d3456_advisories::POST_EDIT,
            "post_edit_logic_advisory",
        ),
        (book::d3456_advisories::GREEN, "green_verify_advisory"),
        (k_competition::WINNER_BANK, "green_verify_advisory"),
        (book::d3456_advisories::FINAL_MILE, "final_mile_advisory"),
        (book::d3456_advisories::FIRST_WRITE, "first_write_advisory"),
        (k_competition::FIRST_WRITE_RAPID, "first_write_advisory"),
        (book::d3456_advisories::NO_EDIT, "no_edit_advisory"),
        (
            book::d3456_advisories::WEAK,
            "self_authored_verify_advisory",
        ),
        (l_loops::STORM, "duplicate_storm_advisory"),
    ];
    for (route, name) in names {
        assert_eq!(book::legacy_kind(route), Some(name), "{}", route.name());
    }
}

#[test]
fn a_raise_can_name_one_page_of_its_route() {
    let raise = Raise::page(book::d3456_advisories::FINAL_MILE, 4, None);
    assert_eq!(raise.cells(), "⠼⠉⠑");
    let root = std::env::temp_dir();
    assert_eq!(
        book::warpath(&root, &[raise, Raise::new(v_verification::UNTESTED, None)]),
        "⠼⠉⠑⠧⠋"
    );
}

#[test]
fn the_error_streak_advises_at_the_third_and_redirects_at_the_sixth_without_stopping() {
    let mut errors = x_execution::ErrorStreak::default();
    let mut given = Vec::new();
    for hop in 1..=13 {
        if let Some(raise) = errors.observe(true) {
            given.push((hop, raise.route));
        }
    }
    assert_eq!(
        given,
        [
            (3, x_execution::ERRORS),
            (6, book::d3456_advisories::CASCADE),
            (9, x_execution::ERRORS),
            (12, book::d3456_advisories::CASCADE),
        ],
        "the cascade starts the count again, as at 0.1.6; nothing here stops a turn"
    );
    assert_eq!(errors.observe(false), None);
    assert_eq!(errors.streak(), 0);
}

fn write(path: &str, content: &str) -> ToolCall {
    ToolCall {
        id: String::new(),
        name: "write_file".into(),
        args: serde_json::json!({"path": path, "content": content}),
    }
}

#[test]
fn a_thrashed_edit_is_the_third_issue_of_the_same_one() {
    let mut thrash = book::d3456_advisories::MutationThrash::default();
    let same = [write("lib.rs", "a")];
    assert!(thrash.observe(&same).is_none());
    assert!(thrash.observe(&[write("lib.rs", "b")]).is_none());
    assert!(thrash.observe(&same).is_none());
    let raise = thrash.observe(&same).expect("the third issue of one edit");
    assert_eq!(raise.route, book::d3456_advisories::THRASH);
    assert!(thrash.observe(&same).is_none(), "once per edit");
    // Reads are never thrash.
    let read = ToolCall {
        id: String::new(),
        name: "read_file".into(),
        args: serde_json::json!({"path": "lib.rs"}),
    };
    for _ in 0..5 {
        assert!(thrash.observe(std::slice::from_ref(&read)).is_none());
    }
}

#[test]
fn peripheral_fan_out_needs_four_paths_and_no_hit_in_the_code() {
    let mut fanout = book::d3456_advisories::PeripheralFanout::default();
    for n in 1..=3 {
        assert!(
            fanout
                .observe(&write(&format!("docs/p{n}.md"), "x"))
                .is_none()
        );
    }
    let raise = fanout
        .observe(&write("testdata/a.json", "x"))
        .expect("the fourth");
    assert_eq!(raise.route, book::d3456_advisories::FANOUT);
    assert!(fanout.observe(&write("docs/p5.md", "x")).is_none(), "once");
    let mut sourced = book::d3456_advisories::PeripheralFanout::default();
    assert!(sourced.observe(&write("src/lib.rs", "x")).is_none());
    for n in 1..=6 {
        assert!(
            sourced
                .observe(&write(&format!("docs/p{n}.md"), "x"))
                .is_none()
        );
    }
}

#[test]
fn the_final_mile_opens_for_a_bounded_turn_that_has_edited() {
    use book::d3456_advisories::{
        final_mile_reserve, rejects_inspection, should_activate_final_mile,
    };
    assert_eq!(final_mile_reserve(TaskPace::Rapid, true), 6);
    assert_eq!(final_mile_reserve(TaskPace::Rapid, false), 4);
    assert_eq!(final_mile_reserve(TaskPace::Deep, true), 4);
    assert!(should_activate_final_mile(Some(20), 16, 4, true, false));
    assert!(
        !should_activate_final_mile(Some(20), 15, 4, true, false),
        "not yet"
    );
    assert!(
        !should_activate_final_mile(Some(20), 16, 4, false, false),
        "nothing edited"
    );
    assert!(
        !should_activate_final_mile(None, 16, 4, true, false),
        "no horizon"
    );
    assert!(
        !should_activate_final_mile(Some(20), 16, 0, true, false),
        "no reserve"
    );
    assert!(
        !should_activate_final_mile(Some(20), 17, 4, true, true),
        "once"
    );
    let read = ToolCall {
        id: "r".into(),
        name: "read_file".into(),
        args: serde_json::json!({"path": "x"}),
    };
    let edit = write("x", "y");
    assert!(rejects_inspection(true, true, std::slice::from_ref(&read)));
    assert!(!rejects_inspection(true, true, &[read.clone(), edit]));
    assert!(!rejects_inspection(
        true,
        false,
        std::slice::from_ref(&read)
    ));
    assert!(!rejects_inspection(
        false,
        true,
        std::slice::from_ref(&read)
    ));
}

#[test]
fn the_stop_turn_carries_the_budget_and_each_facts_evidence_beside_the_stamps() {
    let root = std::env::temp_dir();
    let mut checkpoint = q_stop::Checkpoint::default();
    let facts = [
        Raise::new(
            v_verification::RED,
            "Last run, `run_tests`:\nsub_id failed".to_string(),
        ),
        Raise::new(v_verification::TESTS_EDITED, "spec/a.spec.js".to_string()),
        Raise::new(v_verification::UNTESTED, None),
    ];
    let (cells, shown) = checkpoint
        .engage(&root, &facts, true)
        .expect("facts and budget");
    let turn = q_stop::stop_turn(&cells, &shown, Some("About 300 s and 20 steps are left."));
    let lines = turn.lines().collect::<Vec<_>>();
    assert_eq!(lines[0], cells);
    assert!(cells.ends_with("⠟⠁"), "{cells}");
    assert_eq!(lines[1], "About 300 s and 20 steps are left.");
    assert!(
        turn.contains("sub_id failed") && turn.contains("spec/a.spec.js"),
        "{turn}"
    );
    assert!(
        !turn.contains("Facts at this stop"),
        "the deli's list stays in the ledger"
    );
    assert!(ledger::is_warpath_message(&turn));
}

#[test]
fn a_finished_slot_arrives_with_its_receipt_beside_the_stamps() {
    let notify = crate::agent::harness::WatchNotify {
        id: "slot-7".into(),
        status: "accepted".into(),
        score: Some("0.91".into()),
        rejection_reason: None,
        source_note: None,
        receipt: None,
    };
    let turn = k_competition::watcher_turn(&std::env::temp_dir(), &notify);
    let mut lines = turn.lines();
    assert_eq!(lines.next(), Some("⠅⠉⠅⠙"));
    assert_eq!(
        lines.next(),
        Some("submission slot-7 status=accepted score=0.91")
    );
}

#[test]
fn the_posture_speaks_in_full_and_the_world_card_stays_gone() {
    // Operator law: standing status is never a reply template.
    assert!(
        book::TOC
            .iter()
            .flat_map(|primary| primary.subs)
            .all(|sub| !sub.pages.iter().any(|page| page.contains("WORLD CARD"))),
        "no world card in the book"
    );
    let rapid = words(&pages_of("⠅⠁"));
    assert!(
        rapid.contains(
            "Use the platform CLI for status, score, rejection reason and metrics. A configured watcher is supplementary; a missing notification is not evidence that a job is pending."
        )
    );
    assert!(rapid.contains("Sitting on a prepped submission is a competition failure."));
}

#[test]
fn an_advice_turn_is_the_stamps_then_the_evidence_of_the_raises_that_carry_it() {
    let root = std::env::temp_dir();
    let raises = [
        Raise::inline(
            v_verification::FLAKY,
            "run 3 of 3 failed:\nnames.count(name) == 0".to_string(),
        ),
        Raise::new(v_verification::UNTESTED, "kept in the ledger".to_string()),
    ];
    let turn = book::advice_turn(&root, &raises, false);
    assert_eq!(turn, "⠧⠑⠧⠋\nrun 3 of 3 failed:\nnames.count(name) == 0");
    let warning = book::advice_turn(&root, &raises[..1], true);
    assert!(warning.starts_with("⛔⠧⠑\n"), "{warning}");
    assert!(ledger::is_warpath_message(&warning));
    assert_eq!(
        book::advice_turn(&root, &[Raise::new(x_execution::ERRORS, None)], true),
        "⛔⠭⠁"
    );
}
