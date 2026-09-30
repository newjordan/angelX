//! The 0.1.6 advisories in the book: each is its own words as pages, spoken in
//! full by the legend, given under the condition it had, and never a stop.
use crate::agent::club::{ChatMsg, ToolCall};
use crate::agent::harness::book::{
    self, DIGITS, Raise, Route, introduction, k_competition, l_loops, q_stop, v_verification,
    x_execution,
};
use crate::agent::harness::{TaskPace, book::ledger};

/// The 0.1.6 advisories as they stood at 9d73e92 (`nudges.rs`, the turn loop,
/// the reply notes), the `[harness-telemetry]` tag aside, each with the
/// address whose pages must rebuild it verbatim. A page dropped, merged or
/// paraphrased fails here. What is missing from a text is named beside it.
const ORIGINAL_ADVISORIES: &[(&str, &str)] = &[
    (
        "⠇⠁",
        r##"You've repeated the same tool call several times with no new result — you're stuck in a loop, not converging. Break the pattern deliberately: (1) state the key assumption your current approach depends on, then test the OPPOSITE hypothesis; (2) if that doesn't fit, reframe the problem by analogy to a different domain and see what that suggests; (3) or attack it with a different tool entirely. Do not repeat the previous tool call. If you genuinely cannot make progress, give your best final answer and flag what's unresolved."##,
    ),
    (
        "⠭⠁",
        r##"Every tool call in your last several turns failed. Stop and read the actual error messages — they usually name the cause (wrong path, missing file, bad arguments, wrong state). Verify the precondition first (does the file/dir exist? `list_dir`/`find_files` before reading; check the working state) and then retry differently. Repeating failing calls won't help."##,
    ),
    (
        "⠭⠉",
        r##"Your last reply was cut off at the output token limit before it finished, so nothing in it ran. Usually one tool call carried too much text. Split the work: write a large file in parts (create it with the first part, then add the rest with further edits), keep each tool call well under the limit, and do not restate large content."##,
    ),
    (
        "⠭⠙",
        r##"Your last reply spent its whole output limit on private reasoning and was cut off before it said or did anything, so nothing ran. Do not work the problem out in your head: take the next concrete step now with one tool call (run the tests, read the failing case, or make one small edit) and keep your reasoning short."##,
    ),
    (
        "⠭⠑",
        r##"Your previous reply arrived empty — no text and no tool calls were received. Respond now with either structured tool calls or answer text."##,
    ),
    (
        "⠭⠃",
        r##"Your previous message printed raw tool markup as plain text — no tool was executed, and any results it described were invented. Re-issue the action through the structured tool-call interface now (no tool markup in chat text), then answer from the real output."##,
    ),
    (
        "⠧⠁",
        r##"Your last test run on this exact code failed, so the task is not finished, and there is budget left to fix it. Read the failure below, change the code, and run the tests again. If something outside the code blocks you, such as a missing tool or a broken environment, say what it is and answer again."##,
    ),
    // the stop clause is gone with the stop
    (
        "⠧⠃",
        r##"The task's operator-pinned acceptance command is still RED. This is a hard completion contract, not an advisory verifier: fix the reported failure before claiming completion. Do not redefine, bypass, mask, or replace the command. If the contract cannot be satisfied, report the concrete blocker."##,
    ),
    (
        "⠧⠙",
        r##"VERIFICATION RECOVERY: 3 consecutive verification failures detected. Pause speculative edits and inspect the first failing diagnostic. If errors span multiple functions, types, or borrow lifetimes, stop micro-patching with str_replace and use write_file to rewrite the module cleanly. Do not re-run tests without changing code. Report infrastructure failures honestly; never discard unrelated changes or assume a clean baseline exists."##,
    ),
    (
        "⠧⠑",
        r##"Your last passing test run did not hold: angelX re-ran it on the same code and it failed. The solution passes by luck; something depends on randomness, timing, iteration order or state shared between tests or runs. Find that and fix it so the tests pass every run. Re-running until green is not a fix."##,
    ),
    // the denial threat is gone with the denial
    (
        "⠧⠋",
        r##"You edited the workspace but have not run a verifier since the latest edit. Before claiming completion, run the smallest relevant `check`, `run_tests`, `lint`, `fmt --check`, or equivalent repository command. One relevant green verifier is sufficient; do not follow it with broader or overlapping checks unless the task explicitly requires them. If verification cannot run, state the concrete blocker and the unverified risk in your final answer. A real verifier attempt, even when red or unavailable, is sufficient evidence for an honest blocker report."##,
    ),
    (
        "⠧⠛",
        r##"You changed test files that came with the task. Those tests are the task's contract: leave them as they were. Restore them (for example `git checkout -- <file>`) and keep your fix in the source. To run tests that are skipped, run a copy or restore the file afterwards. If the task asked you to change these tests, say so and answer again."##,
    ),
    (
        "⠼⠁",
        r##"Review the edit logically before testing: trace the state transitions, invariants, cleanup/empty cases, and error paths implied by the task. Prefer the code that *implements or emits* the behavior (library/pkg/src machinery) over generated testdata, docs, or example trees. If the bug names multiple surfaces (code + config/markdown/model file), check whether each still needs a change. Respect explicit no-build and externally delegated verification instructions; in those cases finish with the candidate and a truthful pending-verification note. Otherwise, if the edit already covers them, run one smallest relevant verifier from a *pre-existing* project test entry point and finish. Do not invent new tests as proof, stack broader checks without a concrete diagnostic, or thrash the same edit."##,
    ),
    (
        "⠼⠃",
        r##"GREEN VERIFIER. A full project verifier just passed on this workspace. Use this result as evidence. Complete every remaining requested deliverable, including additional edits or distinct checks when needed, then give the final answer summarizing the work."##,
    ),
    (
        "⠼⠉",
        r##"FINAL-MILE BUDGET ACTIVE. The workspace has changed and the bounded turn is near its horizon. Stop broad inspection. Follow the operator's verification arrangement: if local builds/checks are prohibited or verification is delegated to another host or evaluator, preserve the candidate and report local verification as pending; do not create build manifests or run setup to bypass that arrangement. Otherwise run the smallest relevant verifier now; if it fails, make only the concrete fix supported by its diagnostics, verify again, then return an honest final answer. Do not spend the remaining calls re-reading known context."##,
    ),
    (
        "⠼⠙",
        r##"ACTIONABLE CANDIDATE PROGRESS. Inspection has not yet produced candidate progress. The next tool must mutate the candidate, run the narrow validation implied by current evidence, or report the concrete blocker. Do not wrap another source read in a build/check. Submission is never implied by this hop guard."##,
    ),
    (
        "⠼⠑",
        r##"NO WORKSPACE MUTATION. You claimed progress or completion but no source file was changed this turn. If the bug is real, make the smallest edit (or a dependency bump in go.mod / package.json / Cargo.toml when the fix is an upstream library version). If you truly cannot edit, name the concrete blocker. A bare claim that it is already fixed is not enough."##,
    ),
    (
        "⠼⠋",
        r##"WEAK VERIFICATION. The green check only ran tests or files you created or heavily rewrote this turn. That is not evidence the task's real acceptance tests pass. Prefer pre-existing project test entry points (package test suites, named cases already in the tree). Keep verifying against those, or disclose the residual risk honestly."##,
    ),
    (
        "⠼⠛",
        r##"MUTATION THRASH. You re-issued the same edit signature multiple times (same path and old/new payload, or the same non-unique short snippet). Stop replaying it. If the tool said 'old is not unique', include the enclosing function or more unique context in `old`. If the edit already applied, run a verifier or change approach. Do not spend the remaining horizon re-applying an identical patch."##,
    ),
    (
        "⠼⠓",
        r##"PERIPHERAL FAN-OUT. Several edits landed under docs/, testdata/, fixtures, or generated examples without a corresponding change in the implementing library (src/, pkg/, lib/, core/). Find the code that *produces* those artifacts and fix it at the source instead of hand-patching every generated copy."##,
    ),
    (
        "⡅⠁",
        r##"COMPETITION CANDIDATE VERIFIED. A local check passed on this candidate. Preserve the passing candidate and its evidence before speculative edits. If the operator has authorized submission and all required checks pass, submit through the agreed workflow and record the receipt. A local pass is not proof of a leaderboard win; complete remaining requested checks first."##,
    ),
    (
        "⡅⠉",
        r##"RAPID COMPETITION CANDIDATE PROGRESS. Inspection has not yet produced candidate progress. The next tool must mutate the candidate or follow the explicitly armed submission contract; a board receipt remains legal. Do not wrap another source read in a build/check. If no defensible edit exists, report the concrete blocker."##,
    ),
    (
        "⠅⠁",
        r##"COMPETITION CHALLENGE PACE — RAPID. ALWAYS BE IMPROVING. Revolving door: mutate → local preflight → the current BEST goes up to bat (SUBMIT, receipt/ID) → immediately improve the next best. Once a submission is in play the harness watcher owns that slot. A WATCHER NOTIFY arrives with id + status + score/reason; do not poll-wait. Sitting on a prepped submission is a competition failure. Constant output: one bat in flight, the next best being prepped. After notify, one receipt check is optional; then submit-next (best to bat) or improve-candidate. Long monologues or passive waiting do not count. Never wrap builds, engine boots, or benchmarks in `timeout`: the harness has no tool ceiling and reports long tools live; a self-imposed timeout that kills a load mid-flight is the most common cause of "no verifier result"."##,
    ),
    (
        "⠅⠃",
        r##"COMPETITION CHALLENGE PACE — DEEP. This is a slow-burn solve: build a coherent evidence chain, test distinct hypotheses, and converge only when the candidate is defensible. Hop count, first-write pressure, and watcher state are never instructions to submit — but a candidate that passes the local gate IS submitted (receipt/ID) and then improved; depth is a reason to measure more, never a reason to withhold a measured candidate. Never wrap builds, engine boots, or benchmarks in `timeout`: the harness has no tool ceiling, long tools are reported live, and a self-imposed timeout that kills a load mid-flight is the most common cause of "no verifier result"."##,
    ),
    // {error_stop} is 6, the 0.1.6 task error limit
    (
        "⠼⠊",
        r##"ERROR CASCADE REDIRECTION: Every tool call in the last 6 hops failed. Stop repeating failing commands. Read the compiler diagnostics above and rewrite the file cleanly using `write_file` instead of accumulating micro-patches."##,
    ),
    (
        "⡅⠃",
        r##"VERIFIED CANDIDATE CHANGED: The workspace changed after a passing check. Preserve the prior candidate if available and verify the new bytes before claiming success. Follow the operator-authorized submission plan; a local pass alone does not prove a competitive win."##,
    ),
    (
        "⠇⠓⠁",
        r##"MANDATORY REDIRECTION: You have repeated the same tool call multiple times without making progress. You are caught in a deterministic loop. Break this loop immediately: you MUST NOT repeat this call or run another inspection. Step back and use `write_file` to rewrite the implementing file cleanly from first principles, or use `str_replace` to apply a completely different fix. State your new hypothesis and edit the code now."##,
    ),
    // the escalation diagnosis's progress json and action digests stay in the trajectory
    (
        "⠇⠛⠉",
        r##"MANDATORY PROGRESS REDIRECTION: escalated unproductive turn: {streak} consecutive unproductive hops with no progress since escalation; last verifier outcome: {last}. You must stop inspecting and stop running unchanged commands. You MUST edit the target source code using `write_file` or `str_replace` before executing any more tools. State your concrete fix and modify the file now."##,
    ),
    (
        "⠏⠁",
        r##"Your final answer was deferred because this task still owned running background work. Inspect proc_status and finish from its actual outcome. If a job is no longer needed, explicitly stop it with proc_stop before answering. Do not report an in-flight build as complete; task exit stops remaining jobs."##,
    ),
    // The job's id, name and state (`background process [id] name state`) ride
    // beside the stamp as data.
    (
        "⠏⠃",
        r##"Inspect proc_status id={id} for captured output. Process exit is not benchmark acceptance or a verified solve."##,
    ),
    (
        "⠏⠉⠁⠏⠉⠃⠏⠉⠉⠏⠉⠙",
        r##"Inspect proc_status id={id} for captured output. Process exit is not benchmark acceptance or a verified solve. Background work finished while your answer was being generated. Inspect its proc_status outcome and incorporate it before finishing."##,
    ),
    (
        "⠗⠁",
        r##"[relentless execution active] Relentless execution to the details: keep taking concrete tool-backed actions until the user's request is actually advanced; ensure every action benefits the user; produce logical, evidence-grounded output. Do not stop at status prose. Deliver a useful final answer, then this mode can turn off."##,
    ),
];

/// Every page of an address, joined, as the legend and the ledger read it.
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
fn every_ported_advisory_rebuilds_its_0_1_6_text_from_its_pages() {
    for (cells, original) in ORIGINAL_ADVISORIES {
        assert_eq!(
            words(&pages_of(cells)),
            words(original),
            "{cells} lost or changed text"
        );
    }
}

#[test]
fn the_storm_advice_keeps_its_instruction_and_drops_only_the_suppression() {
    // 0.1.6: "tool error: [duplicate call suppressed: {count}×] You have issued
    // this exact `{name}` call {count} times with identical arguments in the
    // observation window; it was not executed again. No fresh result was read
    // because workspace files have not changed. Do not repeat this call. You
    // must edit the code using write_file or str_replace to fix the issue, or
    // run a different command." The call runs now, so what says it did not is gone.
    assert_eq!(
        words(&pages_of("⠇⠃")),
        "You have issued this exact call several times with identical arguments in the \
         observation window. Do not repeat this call. You must edit the code using write_file \
         or str_replace to fix the issue, or run a different command."
    );
}

#[test]
fn the_poll_advice_keeps_the_passive_wait_instruction_and_drops_the_suppression() {
    let poll = words(&pages_of("⠇⠉"));
    for sentence in [
        "Status snapshots and shell sleeps are observations, not candidate progress.",
        "If a submission is in flight, the harness watcher already owns its status and will inject WATCHER NOTIFY.",
        "Mutate the candidate, run a local preflight/benchmark, submit the current best, or report a concrete blocker before requesting another status snapshot.",
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
            "A WATCHER NOTIFY arrives with id + status + score/reason; do not poll-wait."
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
