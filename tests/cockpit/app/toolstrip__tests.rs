use super::*;

#[test]
#[ignore = "explicit repeated note construction and steady rendering measurement"]
fn rolling_note_large_payload_measurement() {
    for repeat in 1..=5 {
        for bytes in [128, 4_096, 65_536, 1_048_576] {
            let note = "a".repeat(bytes);
            let mut strip = ToolStrip::default();
            let started = Instant::now();
            strip.note_event(&note);
            let setup_us = started.elapsed().as_micros();
            let now = Instant::now();
            let mut samples = Vec::new();
            for frame in 0..256 {
                let started = Instant::now();
                let row =
                    strip.rolling_note(80, now + Duration::from_millis(frame * 33), true, false);
                std::hint::black_box(row);
                samples.push(started.elapsed().as_micros());
            }
            let mut sorted = samples.clone();
            sorted.sort_unstable();
            eprintln!(
                "ROLLING_NOTE_WORK {}",
                serde_json::json!({
                    "repeat": repeat, "note_bytes": bytes, "width": 80, "frames": 256,
                    "setup_us": setup_us, "p50_us": sorted[128], "p95_us": sorted[243],
                    "max_us": sorted[255], "ordered_us": samples,
                })
            );
        }
    }
}

fn compose(row: &StatusRow) -> String {
    let stall = row
        .stall
        .as_ref()
        .map(|readout| format!(" {}", readout.text))
        .unwrap_or_default();
    format!(
        "{}{}{}{}",
        row.left,
        " ".repeat(row.padding),
        row.right,
        stall
    )
}

fn status_text(strip: &ToolStrip, width: usize) -> String {
    status_row_parts(strip, width)
        .map(|row| compose(&row))
        .unwrap_or_default()
}

/// Backdate the newest entry's start so age fragments are deterministic.
fn backdate_current(strip: &mut ToolStrip, ago: Duration) {
    if let Some(entry) = strip.entries.last_mut() {
        entry.started = Instant::now().checked_sub(ago).unwrap_or_else(Instant::now);
    }
}

#[test]
fn strip_tracks_calls_results_and_summary() {
    let mut s = ToolStrip::default();
    assert!(s.take_summary().is_none());
    s.call("shell", "cmd=ls");
    s.call("read_file", "path=a.rs");
    assert_eq!(s.count(), 2);
    assert_eq!(s.current().unwrap().name, "read_file");
    s.result("shell", "ok");
    s.result("read_file", "tool error: nope");
    let cur = s.current().unwrap();
    assert!(cur.done && cur.err, "newest entry finished with an error");
    s.call("shell", "cmd=make");
    s.result("shell", "done");
    let summary = s.take_summary().unwrap();
    assert!(summary.contains("3 tools"), "{summary}");
    assert!(summary.contains("shell\u{00d7}2"), "{summary}");
    assert!(summary.contains("1 err"), "{summary}");
    assert!(s.is_empty(), "summary resets the strip");
}

#[test]
fn snapshot_retains_error_counts_and_dedupes_costly_command_shape() {
    let mut first = ToolStrip::default();
    first.call(
        "shell",
        "popcorn submit --mode benchmark -o /tmp/first.txt candidate.py 2>&1 | head -80",
    );
    first.result("shell", "ok");
    first.call("read_file", "path=candidate.py");
    first.result("read_file", "tool error: missing");
    let a = first.snapshot();
    assert_eq!(a.calls, 2);
    assert_eq!(a.errors, 1);
    assert_eq!(a.incomplete, 0);
    assert_eq!(a.costly_actions.len(), 1);
    assert_eq!(a.outcome_actions.len(), 1);

    let mut second = ToolStrip::default();
    second.call(
        "shell",
        "popcorn submit --mode benchmark -o /tmp/renamed.txt candidate.py 2>&1 | tail -60",
    );
    second.result("shell", "ok");
    assert_eq!(
        a.costly_actions,
        second.snapshot().costly_actions,
        "output filenames and presentation pipes must not disguise a repeated benchmark"
    );

    // Treebeard Hi/Q wrapper + popcorn-cli must collapse to the same fingerprint.
    let mut wrap = ToolStrip::default();
    wrap.call(
        "shell",
        "./scripts/popcorn-submit-hiq.sh --mode benchmark --leaderboard cholesky cand.py",
    );
    wrap.result("shell", "ok");
    let mut cli = ToolStrip::default();
    cli.call(
            "shell",
            "popcorn-cli submit --no-tui --leaderboard cholesky --gpu B200 --mode benchmark -o /tmp/x.txt /tmp/submission.py",
        );
    cli.result("shell", "ok");
    assert_eq!(
        wrap.snapshot().costly_actions,
        cli.snapshot().costly_actions,
        "popcorn-submit-hiq and popcorn-cli must share one costly fingerprint"
    );
    assert_eq!(
        wrap.snapshot().costly_actions,
        a.costly_actions,
        "legacy popcorn submit and hiq wrapper must also match"
    );

    let mut mutation = ToolStrip::default();
    mutation.call("apply_patch", "path=candidate.py");
    mutation.result("apply_patch", "ok");
    let mutation_outcomes = mutation.snapshot().outcome_actions;
    assert_eq!(mutation_outcomes.len(), 1);
    assert!(
        mutation_outcomes[0].starts_with("mutation:apply_patch:result="),
        "{}",
        mutation_outcomes[0]
    );

    let mut changed_status = ToolStrip::default();
    changed_status.call("shell", "hilbert status candidate-1");
    changed_status.result("shell", "score=1.2");
    let first_status = changed_status.snapshot().outcome_actions;
    let mut changed_status = ToolStrip::default();
    changed_status.call("shell", "hilbert status candidate-1");
    changed_status.result("shell", "score=1.3");
    assert_ne!(
        first_status,
        changed_status.snapshot().outcome_actions,
        "the same query returning a new external state is a new receipt"
    );
}

#[test]
fn verified_receipts_require_a_real_measurement_or_submission() {
    // Status polls and listings are outcome activity but never a verified
    // receipt; a failed benchmark is a blocker diagnostic, not progress.
    let mut churn = ToolStrip::default();
    churn.call("shell", "git status --short");
    churn.result("shell", "M src/kernel.cu");
    churn.call("shell", "yukon submissions --all");
    churn.result("shell", "829: scored, 827: rejected");
    churn.call("shell", "./benchmark.sh --local-iterate");
    churn.result(
        "shell",
        "tool error: benchctl measure-job: missing required --golden",
    );
    let snap = churn.snapshot();
    assert_eq!(
        snap.outcome_actions.len(),
        2,
        "status polling stays outcome activity"
    );
    assert!(
        snap.verified_outcome_actions.is_empty(),
        "{:?}",
        snap.verified_outcome_actions
    );
    assert_eq!(snap.verifier_failures.len(), 1);
    assert!(
        snap.verifier_failures[0]
            .1
            .contains("benchctl measure-job: missing required --golden"),
        "{:?}",
        snap.verifier_failures[0]
    );
    assert!(
        snap.verifier_failures[0]
            .0
            .starts_with("shell:./benchmark.sh"),
        "{:?}",
        snap.verifier_failures[0]
    );

    // A succeeded local benchmark script is a measured candidate.
    let mut bench = ToolStrip::default();
    bench.call("shell", "./benchmark.sh --local-iterate 2>&1 | tail -5");
    bench.result("shell", "score=0.42");
    let snap = bench.snapshot();
    assert_eq!(snap.verified_outcome_actions.len(), 1, "{:?}", snap);
    assert!(
        snap.verified_outcome_actions[0]
            .starts_with("measured:shell:./benchmark.sh --local-iterate:result="),
        "{}",
        snap.verified_outcome_actions[0]
    );
    // Output redirection must not disguise a repeated measurement.
    let mut again = ToolStrip::default();
    again.call("shell", "./benchmark.sh --local-iterate -o /tmp/other.txt");
    again.result("shell", "score=0.42");
    assert_eq!(
        bench.snapshot().verified_outcome_actions,
        again.snapshot().verified_outcome_actions,
        "the same measurement cannot look novel under a new output path"
    );

    // Reads that merely *name* a benchmark or submit are not receipts
    // (the matrices run credited `cat …/benchmark.json` 18 times).
    let mut reads = ToolStrip::default();
    reads.call(
        "shell",
        "cat matrices-leader/benchmark.json; git -C matrices-leader status --short",
    );
    reads.result("shell", "{\"score\": 0.8651}");
    reads.call("shell", "grep -n score recon/benchmark.log | tail -3");
    reads.result("shell", "score=0.83");
    reads.call("shell", "hilbert submit --help > recon/submit-help.txt");
    reads.result("shell", "usage: hilbert submit [--note-file] …");
    reads.call(
        "code_mode",
        "allow_effects=false, query=plan: search benchmark contract",
    );
    reads.result("code_mode", "plan: …");
    let snap = reads.snapshot();
    assert!(
        snap.verified_outcome_actions.is_empty(),
        "reads are not receipts: {:?}",
        snap.verified_outcome_actions
    );
    // Launchers and directory hops do not hide a real measurement.
    let mut launched = ToolStrip::default();
    launched.call(
        "shell",
        "cd wt-rcm-lane && timeout 900 ./benchmark.sh --official 2>&1 | tail -2",
    );
    launched.result("shell", "score 0.833148");
    launched.call(
        "proc_run",
        "RUST_LOG=info python3 scripts/measure.py --rows 300",
    );
    launched.result("proc_run", "geomean 0.86");
    let snap = launched.snapshot();
    assert_eq!(
        snap.verified_outcome_actions.len(),
        2,
        "{:?}",
        snap.verified_outcome_actions
    );
    assert!(
        snap.verified_outcome_actions
            .iter()
            .all(|r| r.starts_with("measured:"))
    );

    // A real submit is a submission receipt.
    let mut submit = ToolStrip::default();
    submit.call("shell", "yukon submit cand.py --note 'v2 kernel'");
    submit.result("shell", "submission 829 accepted");
    let snap = submit.snapshot();
    assert_eq!(snap.verified_outcome_actions.len(), 1);
    assert!(
        snap.verified_outcome_actions[0].starts_with("submitted:shell:yukon submit"),
        "{}",
        snap.verified_outcome_actions[0]
    );
}

#[test]
fn status_row_fits_width_and_shows_state() {
    let mut s = ToolStrip::default();
    s.call(
        "shell",
        "cd /very/long/path && cargo build --release --quiet",
    );
    for width in [1, 8, 20, 40, 100] {
        let row = status_text(&s, width);
        assert!(row.chars().count() <= width, "width={width}: {row:?}");
    }
    assert!(status_text(&s, 40).starts_with('\u{25b8}'));
    assert!(status_text(&s, 40).contains("#1"));
    s.result("shell", "ok");
    assert!(status_text(&s, 40).starts_with('\u{2713}'));
}

#[test]
fn status_row_shows_per_call_age_for_unfinished_entry() {
    let mut s = ToolStrip::default();
    s.call("shell", "cargo build");
    backdate_current(&mut s, Duration::from_secs(43));
    // Also backdate the turn-level started clock so the total elapsed
    // matches the call (otherwise right-rail budget squeezes the fragment).
    s.started = Some(
        Instant::now()
            .checked_sub(Duration::from_secs(43))
            .unwrap_or_else(Instant::now),
    );
    let row = status_text(&s, 80);
    assert!(
        row.contains("call 43s"),
        "unfinished call must surface its age: {row}"
    );
    assert!(row.contains("#1"), "{row}");
}

#[test]
fn take_summary_names_the_slowest_tool() {
    let mut s = ToolStrip::default();
    s.call("shell", "cargo test");
    backdate_current(&mut s, Duration::from_secs(21));
    s.result("shell", "ok");
    s.call("read_file", "path=a.rs");
    backdate_current(&mut s, Duration::from_secs(2));
    s.result("read_file", "ok");
    let summary = s.take_summary().unwrap();
    assert!(
        summary.contains("slowest shell 21s"),
        "tally must name the critical path: {summary}"
    );
}

#[test]
fn interrupted_summary_keeps_the_long_delegate_without_inventing_a_result() {
    let mut strip = ToolStrip::default();
    strip.call("vision_look", "image");
    backdate_current(&mut strip, Duration::from_secs(56));
    strip.result("vision_look", "ok");
    strip.call("delegate", "work");
    backdate_current(&mut strip, Duration::from_secs(3600));
    assert_eq!(strip.snapshot().incomplete, 1);
    let summary = strip.take_summary().unwrap();
    assert!(summary.contains("1 unfinished"), "{summary}");
    assert!(
        summary.contains("slowest delegate 1h00m (unfinished)"),
        "{summary}"
    );
    assert!(!summary.contains("verified"), "{summary}");
}

#[test]
fn warp_drive_eases_with_event_rate_and_respects_reduced() {
    let mut s = ToolStrip::default();
    assert!(
        (s.warp_drive(false) - WARP_DRIVE_MIN).abs() < 0.01,
        "silent strip must glide at min drive"
    );
    assert_eq!(s.warp_drive(true), 1.0, "Reduced clamps drive to 1.0");

    // Burst of result events → drive approaches the ceiling.
    for i in 0..16 {
        s.call(&format!("t{i}"), "");
        s.result(&format!("t{i}"), "ok");
    }
    let drive = s.warp_drive(false);
    assert!(
        drive > 2.0 && drive <= WARP_DRIVE_MAX + 0.01,
        "burst must push drive toward max, got {drive}"
    );
    assert_eq!(
        s.warp_drive(true),
        1.0,
        "Reduced still clamps after a burst"
    );

    // Stall cue wins over warp: the flatline style ignores time entirely,
    // so a stalled lane stays flat regardless of drive.
    let stalled = bar_row_with_silence(&s, 40, s.warp_time(false), true);
    let flat = bar_row_with_silence(&s, 40, 0.0, true);
    assert_eq!(
        stalled, flat,
        "stalled lane must ignore warp clock and stay flat"
    );
}

#[test]
fn agent_wait_uses_semantic_two_stage_motion_without_false_stall() {
    let mut strip = ToolStrip::default();
    strip.call("wait_agent", "workers=proof,benchmark");

    assert!(strip.is_waiting_on_agents());
    assert_eq!(motion_cue(&strip), MotionCue::AgentDispatch);
    let status = status_text(&strip, 88);
    assert!(status.contains("COUNCIL · awaiting agents"), "{status}");
    let dispatch_a = bar_row(&strip, 60, 0.3);
    let dispatch_b = bar_row(&strip, 60, 1.1);
    assert!(!dispatch_a.trim().is_empty());
    assert_ne!(
        dispatch_a, dispatch_b,
        "dispatch packets must visibly advance"
    );
    assert!(
        effective_stall_readout(&strip, 301, 20, Some(600)).is_none(),
        "a live agent wait owns its own deadline and is not a provider stall"
    );

    backdate_current(&mut strip, Duration::from_secs(61));
    assert_eq!(motion_cue(&strip), MotionCue::AgentRendezvous);
    let rendezvous_a = bar_row(&strip, 60, 0.3);
    let rendezvous_b = bar_row(&strip, 60, 1.1);
    assert!(!rendezvous_a.trim().is_empty());
    assert_ne!(
        rendezvous_a, rendezvous_b,
        "long-wait rendezvous pings must remain visibly alive"
    );
    assert_ne!(
        dispatch_a, rendezvous_a,
        "long waits should calm into a distinct rendezvous pattern"
    );

    strip.result("wait_agent", "worker complete");
    assert!(!strip.is_waiting_on_agents());
    assert!(
        effective_stall_readout(&strip, 301, 20, Some(600)).is_some(),
        "provider-stall reporting resumes after the wait tool settles"
    );
}

#[test]
fn status_row_fits_terminal_cells_for_wide_unicode() {
    let mut strip = ToolStrip::default();
    strip.call("read_file", "資料/設計🧪/実装.md");
    for width in [1, 8, 20, 40] {
        let row = status_text(&strip, width);
        assert!(
            unicode_width::UnicodeWidthStr::width(row.as_str()) <= width,
            "width={width}: {row:?}"
        );
    }
    assert!(status_text(&strip, 40).contains("#1"));
}

#[test]
fn verifier_status_is_truthful_and_keeps_structured_result_detail() {
    let mut s = ToolStrip::default();
    s.call("functions.run_tests", "args=--no-default-features");
    let running = status_text(&s, 80);
    assert!(running.contains("VERIFY · TEST"), "{running}");
    assert!(running.contains("RUNNING"), "{running}");

    s.result(
        "functions.run_tests",
        "tests: 10 passed, 2 failed; reward: 0.4",
    );
    let failed = status_text(&s, 80);
    assert!(failed.starts_with('\u{2717}'), "{failed}");
    assert!(failed.contains("10 passed · 2 failed"), "{failed}");
    assert!(failed.contains("FAIL"), "{failed}");
    assert_eq!(s.snapshot().errors, 1);
}

#[test]
fn structured_zero_and_nonzero_diagnostics_drive_pass_fail() {
    let mut clean = ToolStrip::default();
    clean.call("check", "--all-targets");
    clean.result("check", "check: 0 warnings, 0 errors");
    assert!(status_text(&clean, 72).contains("PASS"));
    assert_eq!(clean.snapshot().errors, 0);

    let mut broken = ToolStrip::default();
    broken.call("lint", "--all-targets");
    broken.result("lint", "lint: warnings: 3, errors: 1");
    let row = status_text(&broken, 72);
    assert!(row.contains("3 warnings · 1 errors"), "{row}");
    assert!(row.contains("FAIL"), "{row}");
    assert_eq!(broken.snapshot().errors, 1);
}

#[test]
fn cargo_subcommands_receive_specific_verifier_labels() {
    for (args, label) in [
        ("test --workspace", "TEST"),
        ("check", "CHECK"),
        ("clippy --all-targets", "LINT"),
        ("fmt --check", "FORMAT"),
    ] {
        let mut s = ToolStrip::default();
        s.call("cargo", args);
        let row = status_text(&s, 60);
        assert!(row.contains(&format!("VERIFY · {label}")), "{row}");
        assert_eq!(motion_cue(&s), MotionCue::VerifierLock);
    }
}

#[test]
fn stall_readout_thresholds_escalate_and_disarm() {
    // Below the pulse threshold: no readout at all.
    assert!(stall_readout(0, 20, Some(600)).is_none());
    assert!(stall_readout(19, 20, Some(600)).is_none());
    // Past it: the silence report.
    let warn = stall_readout(25, 20, Some(600)).unwrap();
    assert_eq!(warn.text, "\u{00b7} silent 25s");
    assert_eq!(warn.severity, StallSeverity::Warn);
    // Past half the watchdog deadline: countdown to abandonment.
    let watchdog = stall_readout(301, 20, Some(600)).unwrap();
    assert_eq!(watchdog.text, "\u{00b7} watchdog 299s");
    assert_eq!(watchdog.severity, StallSeverity::Watchdog);
    // The countdown floors at zero rather than going negative.
    assert_eq!(
        stall_readout(700, 20, Some(600)).unwrap().text,
        "\u{00b7} watchdog 0s"
    );
    // 0 disarms the pulse entirely, however silent the stream.
    assert!(stall_readout(9_999, 0, Some(600)).is_none());
    // No watchdog configured: silence keeps reporting without a countdown.
    assert_eq!(
        stall_readout(301, 20, None).unwrap().severity,
        StallSeverity::Warn
    );
}

#[test]
fn status_row_with_silence_appends_fragment_and_flattens_the_lane() {
    let mut s = ToolStrip::default();
    s.call("shell", "cmd=cargo build");

    // 0s silent: byte-identical to the plain row, lane keeps its motion.
    let quiet = status_row_parts_with_silence(&s, 80, stall_readout(0, 20, Some(600))).unwrap();
    assert!(quiet.stall.is_none());
    assert_eq!(compose(&quiet), status_text(&s, 80));
    assert_eq!(motion_cue_with_silence(&s, false), MotionCue::Work);

    // 25s: the silence fragment rides the right rail and still fits.
    let warn = status_row_parts_with_silence(&s, 80, stall_readout(25, 20, Some(600))).unwrap();
    assert_eq!(warn.stall.as_ref().unwrap().severity, StallSeverity::Warn);
    let row = compose(&warn);
    assert!(row.ends_with("\u{00b7} silent 25s"), "{row:?}");
    assert_eq!(UnicodeWidthStr::width(row.as_str()), 80, "{row:?}");
    assert_eq!(motion_cue_with_silence(&s, true), MotionCue::Stalled);

    // 301s of a 600s deadline: the watchdog countdown takes the rail.
    let hot = status_row_parts_with_silence(&s, 80, stall_readout(301, 20, Some(600))).unwrap();
    assert_eq!(
        hot.stall.as_ref().unwrap().severity,
        StallSeverity::Watchdog
    );
    assert!(compose(&hot).ends_with("\u{00b7} watchdog 299s"));

    // Narrow panes drop the fragment with the rest of the rail.
    for width in [1, 8, 14] {
        let narrow =
            status_row_parts_with_silence(&s, width, stall_readout(25, 20, Some(600))).unwrap();
        assert!(narrow.stall.is_none(), "width={width}");
        let text = compose(&narrow);
        assert!(text.chars().count() <= width, "width={width}: {text:?}");
    }
}

#[test]
fn note_row_carries_the_stall_fragment_on_its_right_edge() {
    let mut s = ToolStrip::default();
    s.note_event("recall: warming 3 capsules");
    let row = status_row_parts_with_silence(&s, 60, stall_readout(25, 20, Some(600))).unwrap();
    assert!(row.stall.is_some());
    let text = compose(&row);
    assert!(text.starts_with('\u{25b8}'), "{text:?}");
    assert!(text.ends_with("silent 25s"), "{text:?}");
    assert_eq!(UnicodeWidthStr::width(text.as_str()), 60, "{text:?}");
}

#[test]
fn note_prefixes_keep_distinct_families_in_arrival_order() {
    let mut s = ToolStrip::default();
    s.note_event("trimmed 3 recent tool result(s) to fit the active context window");
    s.note_event("storm: suppressed duplicate shell call (x3)");
    s.note_event("storm: suppressed duplicate grep call (x2)");
    s.note_event("context compacted");
    assert_eq!(s.note_count(), 4);
    assert_eq!(s.note(), Some("context compacted"));
    assert_eq!(s.note_prefixes(), ["trimmed", "storm", "context"]);
    s.begin_turn();
    assert!(s.note_prefixes().is_empty());
    assert_eq!(s.note_count(), 0);
}

#[test]
fn take_summary_keeps_mixed_note_prefixes() {
    let mut s = ToolStrip::default();
    s.call("shell", "cmd=ls");
    s.result("shell", "ok");
    s.note_event("trimmed 3 recent tool result(s) to fit the active context window");
    s.note_event("storm: suppressed duplicate shell call (x3)");
    s.note_event("context compacted");
    let summary = s.take_summary().unwrap();
    assert!(
        summary.contains("3 notes (trimmed, storm, context)"),
        "mixed prefixes must survive the tally: {summary}"
    );
    assert_eq!(summary.lines().count(), 1, "{summary}");
    assert!(
        !summary.contains("suppressed") && !summary.contains("compacted"),
        "tally must not dump live note sentences: {summary}"
    );
    assert!(s.is_empty());

    let mut notes_only = ToolStrip::default();
    notes_only.note_event("trimmed 3 recent tool result(s) to fit the active context window");
    notes_only.note_event("storm: suppressed duplicate shell call (x3)");
    notes_only.note_event("context compacted");
    let summary = notes_only.take_summary().unwrap();
    assert!(
        summary.contains("3 notes (trimmed, storm, context)"),
        "notes-only tally must keep prefixes: {summary}"
    );
    assert!(summary.contains("(/trace for detail)"), "{summary}");
    assert_eq!(summary.lines().count(), 1, "{summary}");
    assert!(
        !summary.contains("suppressed") && !summary.contains("compacted"),
        "notes-only tally must not dump live note sentences: {summary}"
    );
}

#[test]
fn take_summary_same_family_notes_keep_one_prefix() {
    let mut s = ToolStrip::default();
    s.note_event("storm: suppressed duplicate shell call (x2)");
    s.note_event("storm: suppressed duplicate grep call (x3)");
    assert_eq!(s.note_prefixes(), ["storm"]);
    let summary = s.take_summary().unwrap();
    assert!(
        summary.contains("2 notes (storm)"),
        "same-family repeats keep the one prefix: {summary}"
    );
    assert!(
        !summary.contains("trimmed") && !summary.contains("context"),
        "same-family repeats must not invent extra prefixes: {summary}"
    );
    assert!(
        !summary.contains("suppressed"),
        "tally must not dump live note sentences: {summary}"
    );
    assert_eq!(summary.lines().count(), 1, "{summary}");
}

#[test]
fn stalled_lane_is_flat_static_and_distinct() {
    let strip = ToolStrip::default();
    let early = bar_row_with_silence(&strip, 30, 1.7, true);
    let late = bar_row_with_silence(&strip, 30, 9.9, true);
    assert_eq!(early.chars().count(), 30);
    assert_eq!(early, late, "a parked stream must not animate");
    assert_ne!(
        early,
        bar_row(&strip, 30, 1.7),
        "flatline differs from the work lane"
    );
    assert!(
        early
            .chars()
            .any(|c| ('\u{2800}'..='\u{28FF}').contains(&c) && c != '\u{2800}'),
        "flat rail still draws its dots: {early}"
    );
}

#[test]
fn stall_pulse_env_overrides_and_zero_disarms() {
    let _env = crate::tests::env_lock();
    {
        let _unset = crate::tests::TestEnvGuard::unset("ANGEL_STALL_PULSE_SECS");
        assert_eq!(configured_stall_pulse_secs(), 20);
    }
    {
        let _set = crate::tests::TestEnvGuard::set("ANGEL_STALL_PULSE_SECS", "45");
        assert_eq!(configured_stall_pulse_secs(), 45);
    }
    {
        let _off = crate::tests::TestEnvGuard::set("ANGEL_STALL_PULSE_SECS", "0");
        assert_eq!(configured_stall_pulse_secs(), 0);
        assert!(stall_readout(10_000, configured_stall_pulse_secs(), Some(600)).is_none());
    }
    {
        let _junk = crate::tests::TestEnvGuard::set("ANGEL_STALL_PULSE_SECS", "not-seconds");
        assert_eq!(configured_stall_pulse_secs(), 20);
    }
}

#[test]
fn bar_row_renders_braille_at_width() {
    let row = bar_row(&ToolStrip::default(), 30, 1.7);
    assert_eq!(row.chars().count(), 30);
    assert!(
        row.chars()
            .any(|c| ('\u{2800}'..='\u{28FF}').contains(&c) && c != '\u{2800}'),
        "bar has lit dots: {row}"
    );
}

#[test]
fn verifier_tools_select_the_sync_lock_motion_cue() {
    let mut strip = ToolStrip::default();
    strip.call("ui_verify", "open_control=model");
    assert_eq!(motion_cue(&strip), MotionCue::VerifierLock);
    strip.begin_turn();
    strip.call("run_tests", "--no-default-features");
    assert_eq!(motion_cue(&strip), MotionCue::VerifierLock);
    let verifier = bar_row(&strip, 30, 1.7);
    let ordinary = bar_row(&ToolStrip::default(), 30, 1.7);
    assert_eq!(verifier.chars().count(), 30);
    assert_ne!(verifier, ordinary);
}

#[test]
fn terminal_result_cues_are_static_and_require_correlated_evidence() {
    for (summary, marker) in [("ok", '✓'), ("tool error: failed", '✗')] {
        let mut strip = ToolStrip::default();
        strip.call("shell", "cargo test");
        assert_ne!(bar_row(&strip, 40, 0.3), bar_row(&strip, 40, 1.1));
        strip.result("shell", summary);
        let row = bar_row(&strip, 40, 0.3);
        assert!(row.starts_with(marker), "{row}");
        assert_eq!(row, bar_row(&strip, 40, 9.9));
        assert_eq!(UnicodeWidthStr::width(row.as_str()), 40);
        assert_eq!(bar_row(&strip, 0, 0.3), "");
        assert_eq!(bar_row(&strip, 1, 0.3), marker.to_string());
    }
}

#[test]
fn dotmax_live_v2_catalog_is_bundled() {
    assert_eq!(dotmax::progress::themes().len(), 57);
    assert_eq!(dotmax::progress::all_styles().len(), 644);
    for theme in ["matrix", "aurora", "inferno", "glitch", "fireworks"] {
        let styles = dotmax::progress::styles_for_theme(theme);
        assert_eq!(styles.len(), 10, "{theme}");
        for style in styles {
            let ctx = BarContext::new(0.61, 1.7, 30, 1);
            let row = dotmax::progress::render_lines(style.as_ref(), &ctx).unwrap();
            assert_eq!(row.len(), 1, "{theme}/{}", style.name());
            assert_eq!(row[0].chars().count(), 30, "{theme}/{}", style.name());
        }
    }
}

#[test]
fn parallel_same_name_results_pair_by_id_in_reverse_order() {
    let mut s = ToolStrip::default();
    let first = ToolEventId("read-a".to_string());
    let second = ToolEventId("read-b".to_string());
    s.call_event(first.clone(), "read_file", "path=a.rs");
    s.call_event(second.clone(), "read_file", "path=b.rs");
    let success = ToolOutcome {
        execution: ExecutionOutcome::Succeeded,
        verification: VerificationOutcome::NotApplicable,
    };
    s.result_event(&second, "read_file", "b ok", success);
    assert!(s.has_running_calls(), "call a still owns work");
    assert!(!s.entries[0].done && s.entries[1].done, "id b closes b");
    s.result_event(&first, "read_file", "a ok", success);
    assert!(!s.has_running_calls(), "all calls have terminal results");
    assert!(s.entries.iter().all(|entry| entry.done));
}

#[test]
fn not_started_result_closes_call_as_non_success() {
    let mut s = ToolStrip::default();
    let id = ToolEventId("guard-rejected".to_string());
    s.call_event(id.clone(), "read_file", "path=src/lib.rs");
    s.result_event(
        &id,
        "read_file",
        "tool error: rejected before dispatch",
        ToolOutcome {
            execution: ExecutionOutcome::NotStarted,
            verification: VerificationOutcome::NotApplicable,
        },
    );

    assert!(s.entries[0].done);
    assert!(s.entries[0].err);
    assert_eq!(s.snapshot().errors, 0);
    let row = status_row_parts(&s, 80).unwrap();
    assert_eq!(row.state, ToolState::NotStarted);
    assert!(row.left.contains("NOT STARTED"), "{}", row.left);
    assert!(s.take_summary().unwrap().contains("1 not started"));
}

#[test]
fn unknown_and_duplicate_result_ids_are_neutral_diagnostics() {
    let mut s = ToolStrip::default();
    let call = ToolEventId("known".to_string());
    let unknown = ToolEventId("unknown".to_string());
    let success = ToolOutcome {
        execution: ExecutionOutcome::Succeeded,
        verification: VerificationOutcome::NotApplicable,
    };
    s.call_event(call.clone(), "write_file", "path=a.rs");
    s.result_event(&unknown, "write_file", "ok", success);
    assert!(!s.entries[0].done);
    s.result_event(&call, "write_file", "ok", success);
    s.result_event(&call, "write_file", "ok again", success);
    assert_eq!(s.snapshot().diagnostics, 2);
    assert_eq!(s.snapshot().calls, 1);
}
#[test]
fn measured_receipts_ignore_pipes_inside_quotes_and_heredoc_bodies() {
    // Operator finding 2026-09-11 (qwen38 loop): both of these minted "measured candidates".
    let grep = r#"cd /home/user/comps/qwen/qwen38-125b-a6b-cuda-v1 && grep -rn "verify_rows_top1\|read_logit_row\|defer_frontier\|verify_rows" ds4/ | head"#;
    assert!(
        measured_submission_fingerprint("shell", grep).is_none(),
        "grep pattern is a read"
    );
    let heredoc = r#"cd /home/user/comps/qwen/prof && python3 - <<'py'
import re
out=["probe case"]
measure = 1 | 2
benchmark_rows = []
verify = None
print(out)
py
"#;
    assert!(
        measured_submission_fingerprint("shell", heredoc).is_none(),
        "heredoc body is not a program"
    );
    // Real board measurements in program position still count.
    for cmd in [
        "cd /home/user/comps/qwen/qwen38-125b-a6b-cuda-v1 && benchd-bin/benchd iterate --engine .build/release/worker",
        "cd /home/user/comps/qwen/qwen38-125b-a6b-cuda-v1 && tools/local-baseline.sh",
        "CUDA_ENGINE_EXECUTABLE=/x/cuda-engine ./tools/qwen38-125b-a6b-measure-and-score.sh",
        "timeout 900 bash tools/benchmark.sh --local-iterate",
    ] {
        let fp = measured_submission_fingerprint("shell", cmd);
        assert!(
            fp.as_ref().is_some_and(|(_, sub)| !sub),
            "{cmd} should be a measurement: {fp:?}"
        );
    }
    assert!(
        measured_submission_fingerprint("shell", "cat score.json && grep bench notes.md").is_none()
    );
    // Exact line from the ripe loop that had minted "submitted:shell:ls experiments".
    let ls = "ls experiments/go* experiments/*go* challenge/ripemd160/submissions 2>/dev/null | head -40; ls challenge/ripemd160/submissions/ 2>/dev/null | head -20; cat challenge/ripemd160/submitting.md 2>/dev/null";
    assert!(
        measured_submission_fingerprint("shell", ls).is_none(),
        "reads of submission paths are not submissions"
    );
    assert!(
        measured_submission_fingerprint("shell", "hilbert submit --note-file note.md")
            .is_some_and(|(_, sub)| sub)
    );
    let segs = command_segments(r#"a | b 'x|y' && c "d;e" ; f"#);
    assert_eq!(
        segs.iter().map(|s| s.trim()).collect::<Vec<_>>(),
        vec!["a", "b 'x|y'", r#"c "d;e""#, "f"]
    );
}
