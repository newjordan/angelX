use super::*;
use serde_json::json;

fn call(name: &str, args: serde_json::Value) -> ToolCall {
    ToolCall {
        id: "c".into(),
        name: name.into(),
        args,
    }
}

fn shell(command: &str) -> ToolCall {
    call("shell", json!({"command": command}))
}

fn replace(old: &str, new: &str) -> ToolCall {
    call(
        "str_replace",
        json!({"path": "src/config.rs", "old": old, "new": new}),
    )
}

fn retries(from: usize) -> ToolCall {
    replace(
        &format!("retries = {from}"),
        &format!("retries = {}", from + 1),
    )
}

fn hop(sweep: &mut NumberSweep, calls: &[ToolCall]) -> Option<String> {
    sweep.observe(calls.iter().map(|call| (call, ExecutionOutcome::Succeeded)))
}

/// One round: a numbers-only edit in one hop and its check in the next.
fn round(sweep: &mut NumberSweep, n: usize) -> Option<String> {
    assert_eq!(hop(sweep, &[retries(n)]), None, "edit {n}");
    hop(sweep, &[shell("cargo test --release")])
}

#[test]
fn sed_that_moves_only_numbers_is_a_numbers_only_change() {
    for command in [
        "sed -i 's/TIMEOUT_SECS = 30/TIMEOUT_SECS = 45/' src/config.rs",
        "sed -i.bak 's|tolerance: 0.05|tolerance: 0.02|g' settings.yaml",
        "sed -i 's#retries=3#retries=5#' run.sh",
        "sed -i 's,\"workers\": 8,\"workers\": 6,' config.json",
        // An escaped delimiter inside the pattern and the replacement.
        "sed -i 's/ratio = 1\\/4/ratio = 1\\/8/' src/config.rs",
        "sed -i -e 's/retries = 3/retries = 4/' -e 's/backoff_ms = 100/backoff_ms = 250/' src/config.rs",
        "sed -i 's/retries = 3/retries = 4/; s/backoff_ms = 100/backoff_ms = 250/' src/config.rs",
        "sed --in-place=.orig --expression='s/retries = 3/retries = 4/' src/config.rs",
        "sed -i '42s/0.5/0.6/' src/config.rs",
        "sed -i '/^tolerance/s/[0-9.]\\+/0.02/' settings.yaml",
        // The pattern is a regular expression that can only match the old number.
        "sed -i 's/^TIMEOUT_SECS = [0-9]*/TIMEOUT_SECS = 45/' src/config.rs",
        "sed -i 's/^tolerance: .*$/tolerance: 0.02/' settings.yaml",
        "sed -i -E 's/(retries *= *)[0-9]+/\\15/' src/config.rs",
        "sed -i \"s/\\(backoff_ms = \\)[[:digit:]]*/\\1250/\" src/config.rs",
        "perl -pi -e 's/retries = \\d+/retries = 5/' src/config.rs",
        "cd /w/repo && sed -i 's/limit = 10/limit = 12/' a.py && cargo test",
        "find . -name '*.toml' | xargs sed -i 's/workers = 8/workers = 6/'",
    ] {
        assert_eq!(shell_change(command), Some(Change::Numbers), "{command}");
    }
}

#[test]
fn sed_that_changes_text_or_cannot_be_read_is_structural() {
    for command in [
        "sed -i 's/retry_once(/retry_with_backoff(/' src/client.rs",
        // Every substitution in the call has to be numbers-only.
        "sed -i -e 's/retries = 3/retries = 4/' -e 's/linear/exponential/' src/config.rs",
        "sed -i 's/retries = 3/retries = 4/; s/linear/exponential/' src/config.rs",
        "sed -i '/legacy_retry/d' src/client.rs",
        "sed -i '12s/.*/    return cached/' src/client.rs",
        "sed -i -f tweak.sed src/config.rs",
        "perl -pi -e 's/(\\d+)/$1 + 1/e' src/config.rs",
    ] {
        assert_eq!(shell_change(command), Some(Change::Structural), "{command}");
    }
    for command in [
        // Not in place: nothing is edited.
        "sed -n '10,40p' src/config.rs",
        "sed 's/retries = 3/retries = 4/' src/config.rs",
        // The same text back is not a change.
        "sed -i 's/retries = 3/retries = 3/' src/config.rs",
        "grep -n 'sed -i' notes.md",
        "cat > notes.md <<'EOF'\nsed -i 's/retries = 3/retries = 4/' src/config.rs\nEOF",
    ] {
        assert_eq!(shell_change(command), None, "{command}");
    }
}

#[test]
fn a_shell_variable_where_the_number_goes_is_a_numbers_only_change() {
    for command in [
        // The pattern has a number, or an expression that matches one, where the variable is.
        "sed -i \"s/const MAX_RETRIES: usize = [0-9]*;/const MAX_RETRIES: usize = $n;/\" src/config.rs",
        "sed -i \"s/^const TIMEOUT_SECS: u64 = .*/const TIMEOUT_SECS: u64 = $T;/; \
         s/^const MAX_RETRIES: usize = .*/const MAX_RETRIES: usize = $R;/\" src/config.rs",
        "sed -i \"s/retries = 3/retries = ${RETRIES}/\" src/config.rs",
        "sed -i \"s/timeout_secs: [0-9]\\+/timeout_secs: $1/\" settings.yaml",
        "sed -i 's/^tolerance: .*/tolerance: '\"$TOL\"'/' settings.yaml",
        // A zero in the pattern is still replaced by whatever the variable holds.
        "sed -i \"s/backoff_ms = 0/backoff_ms = $B/\" src/config.rs",
    ] {
        assert_eq!(shell_change(command), Some(Change::Numbers), "{command}");
    }
    for command in [
        // The variable stands where the pattern has text.
        "sed -i \"s/mode = fast/mode = $MODE/\" src/config.rs",
        "sed -i \"s/retry(3)/$CALL(3)/\" src/client.rs",
        // Single quotes or a backslash: the shell expands nothing and the name is written.
        "sed -i 's/timeout 30/timeout $TIMEOUT/' run.sh",
        "sed -i \"s/timeout 30/timeout \\$TIMEOUT/\" run.sh",
        // A pattern that takes the rest of the line has to get a number back, not words.
        "sed -i \"s/^timeout = .*/timeout = compute($T)/\" src/config.rs",
    ] {
        assert_eq!(shell_change(command), Some(Change::Structural), "{command}");
    }

    // A command that edits through a variable and runs the checker is one round a hop,
    // however many values its loop walks.
    let step = |values: &str| {
        shell(&format!(
            "for n in {values}; do sed -i \"s/const MAX_RETRIES: usize = [0-9]*;/const \
             MAX_RETRIES: usize = $n;/\" src/config.rs; timeout 900 api-benchmark \"RETRIES=$n \
             LIMIT=60\" 2>&1 | grep -E \"^score|run #|NEW\"; done"
        ))
    };
    let mut sweep = NumberSweep::default();
    assert_eq!(hop(&mut sweep, &[step("9")]), None);
    assert_eq!(hop(&mut sweep, &[step("8 7 6 5")]), None);
    let note = hop(&mut sweep, &[step("4")]).expect("the third hop of the sweep");
    assert!(note.starts_with("The last 3 changes"), "{note}");
}

#[test]
fn edit_tools_are_read_by_their_old_and_new_text() {
    let numbers = Some(Change::Numbers);
    let structural = Some(Change::Structural);
    assert_eq!(
        tool_change(&replace(
            "const MAX_RETRIES: u32 = 3;",
            "const MAX_RETRIES: u32 = 5;"
        )),
        numbers
    );
    assert_eq!(
        tool_change(&replace("retry(3)", "retry_with_backoff(3)")),
        structural
    );
    assert_eq!(tool_change(&replace("retries = 3", "retries = 3")), None);

    let multi = |tolerance: &str| {
        call(
            "multi_edit",
            json!({"path": "src/config.rs", "edits": [
                {"old": "timeout_secs: 30", "new": "timeout_secs: 45"},
                {"old": "tolerance: 0.05", "new": tolerance},
            ]}),
        )
    };
    assert_eq!(tool_change(&multi("tolerance: 0.02")), numbers);
    assert_eq!(tool_change(&multi("tolerance: None")), structural);

    let patch = |diff: &str| call("apply_patch", json!({"diff": diff}));
    let unified = "--- a/src/config.rs\n+++ b/src/config.rs\n@@ -4,3 +4,3 @@\n fn defaults() {\n\
                   -    retries: 3,\n+    retries: 5,\n }\n";
    assert_eq!(tool_change(&patch(unified)), numbers);
    let freeform = "*** Begin Patch\n*** Update File: src/config.rs\n@@\n\
                    -const WINDOW: usize = 0x40;\n+const WINDOW: usize = 0x80;\n*** End Patch";
    assert_eq!(tool_change(&patch(freeform)), numbers);
    let added_line = "*** Begin Patch\n*** Update File: src/config.rs\n@@\n-    retries: 3,\n\
                      +    retries: 5,\n+    jitter: true,\n*** End Patch";
    assert_eq!(tool_change(&patch(added_line)), structural);
    let new_file = "*** Begin Patch\n*** Add File: src/limits.rs\n+pub const LIMIT: usize = 8;\n\
                    *** End Patch";
    assert_eq!(tool_change(&patch(new_file)), structural);

    // A whole-file write shows no old text, so it cannot be called numbers-only.
    let rewrite = call(
        "write_file",
        json!({"path": "src/config.rs", "content": "retries = 5\n"}),
    );
    assert_eq!(tool_change(&rewrite), structural);
    assert_eq!(
        tool_change(&call("read_file", json!({"path": "src/config.rs"}))),
        None
    );
}

#[test]
fn hex_and_decimal_literals_each_mask_as_one_number() {
    for (old, new) in [
        ("mask = 0x1F", "mask = 0xff"),
        ("tolerance = 0.05", "tolerance = 1.5"),
        ("budget = 1_000_000", "budget = 250"),
        ("epsilon = 1e-3", "epsilon = 5e-4"),
        ("scale = 2.5e6", "scale = 3e6"),
        ("timeout: 30s", "timeout: 45s"),
        ("flags = 0b1010", "flags = 0b0110"),
    ] {
        assert_eq!(mask_numbers(old), mask_numbers(new), "{old} -> {new}");
        assert_eq!(
            tool_change(&replace(old, new)),
            Some(Change::Numbers),
            "{old} -> {new}"
        );
    }
    assert_eq!(mask_numbers("0x1F + 2.50 * 1_000"), "\0 + \0 * \0");
    // One more number, or one fewer, is a different shape.
    assert_ne!(
        mask_numbers("limits = [8, 16]"),
        mask_numbers("limits = [8]")
    );
}

/// The false positive that matters most: a digit inside a name is part of the name. Widening
/// a type or swapping an algorithm is a structural change and must never be counted as a
/// constant being nudged.
#[test]
fn digits_inside_a_name_are_not_numbers() {
    let renames = [
        ("let count: u8 = 0;", "let count: u16 = 0;"),
        ("fn checksum() -> Sha1", "fn checksum() -> Sha256"),
        ("total as int32", "total as int64"),
        ("decode_utf8(input)", "decode_utf16(input)"),
        ("mod parser_v1;", "mod parser_v2;"),
    ];
    let mut sweep = NumberSweep::default();
    for (old, new) in renames {
        assert_eq!(
            tool_change(&replace(old, new)),
            Some(Change::Structural),
            "{old} -> {new}"
        );
        assert_eq!(hop(&mut sweep, &[replace(old, new)]), None);
        assert_eq!(hop(&mut sweep, &[shell("cargo test")]), None);
    }
    assert_eq!(
        shell_change("sed -i 's/f32/f64/g' src/math.rs"),
        Some(Change::Structural)
    );
}

#[test]
fn the_notice_comes_at_three_rounds_then_seven_and_not_between() {
    let mut sweep = NumberSweep::default();
    let mut raised = Vec::new();
    for n in 1..=11 {
        if let Some(note) = round(&mut sweep, n) {
            raised.push((n, note));
        }
    }
    let rounds: Vec<usize> = raised.iter().map(|(n, _)| *n).collect();
    assert_eq!(rounds, [3, 7, 11]);
    assert_eq!(
        raised[0].1,
        "The last 3 changes altered only numbers, each followed by a check. If that check \
         samples its inputs, the lowest setting that passes once is not a result: it will fail \
         on another draw. Keep the last setting that passed with room to spare, record it, and \
         make the next change a structural one."
    );
    assert!(
        raised[1].1.starts_with("The last 7 changes"),
        "{}",
        raised[1].1
    );
}

#[test]
fn the_change_and_its_check_can_share_a_hop_or_a_command() {
    let mut sweep = NumberSweep::default();
    let one_command = shell("sed -i 's/workers = 8/workers = 7/' app.toml && ./benchmark.sh");
    assert_eq!(hop(&mut sweep, &[one_command]), None);
    let one_hop = [
        replace("workers = 7", "workers = 6"),
        call("run_tests", json!({})),
    ];
    assert_eq!(hop(&mut sweep, &one_hop), None);
    let two_calls = [
        shell("sed -i 's/workers = 6/workers = 5/' app.toml"),
        shell("make verify"),
    ];
    let note = hop(&mut sweep, &two_calls).expect("third round");
    assert!(note.starts_with("The last 3 changes"), "{note}");
}

#[test]
fn several_rounds_in_one_hop_raise_one_notice() {
    let batch = |from: usize| {
        let step = |n: usize| {
            shell(&format!(
                "sed -i 's/workers = {n}/workers = {}/' app.toml",
                n - 1
            ))
        };
        [
            step(from),
            shell("./benchmark.sh"),
            step(from - 1),
            shell("./benchmark.sh"),
        ]
    };
    let mut sweep = NumberSweep::default();
    assert_eq!(hop(&mut sweep, &batch(9)), None);
    let note = hop(&mut sweep, &batch(7)).expect("the third round is inside this hop");
    assert!(note.starts_with("The last 4 changes"), "{note}");
}

#[test]
fn a_structural_edit_starts_the_count_again() {
    let structural = [
        replace("retry(n)", "retry_with_backoff(n)"),
        shell("sed -i 's/linear/exponential/' src/config.rs"),
        call(
            "write_file",
            json!({"path": "src/backoff.rs", "content": "pub fn delay() {}\n"}),
        ),
    ];
    for edit in structural {
        let mut sweep = NumberSweep::default();
        assert_eq!(round(&mut sweep, 1), None);
        assert_eq!(round(&mut sweep, 2), None);
        assert_eq!(hop(&mut sweep, std::slice::from_ref(&edit)), None);
        assert_eq!(round(&mut sweep, 3), None, "{}", edit.name);
        assert_eq!(round(&mut sweep, 4), None, "{}", edit.name);
        assert!(round(&mut sweep, 5).is_some(), "{}", edit.name);
    }
}

#[test]
fn reads_searches_notes_and_bare_checks_neither_count_nor_restart() {
    let quiet = [
        call("read_file", json!({"path": "src/config.rs"})),
        call("grep", json!({"pattern": "retries"})),
        call("list_dir", json!({"path": "src"})),
        shell("git diff --stat"),
        shell("cat benchmark.sh"),
        // Running the check again with nothing changed is a re-draw, not a round.
        shell("cargo test --release"),
        // Notes are not the thing under check.
        call(
            "write_file",
            json!({"path": "notes.md", "content": "retries = 2 passed\n"}),
        ),
    ];
    let mut sweep = NumberSweep::default();
    for n in 1..=2 {
        assert_eq!(round(&mut sweep, n), None);
        for call in &quiet {
            assert_eq!(hop(&mut sweep, std::slice::from_ref(call)), None);
        }
    }
    assert!(round(&mut sweep, 3).is_some());
}

#[test]
fn a_change_with_no_check_after_it_is_not_a_round() {
    // Edits and never a check: nothing is counted, however many there are.
    let mut sweep = NumberSweep::default();
    for n in 1..=9 {
        assert_eq!(hop(&mut sweep, &[retries(n)]), None);
    }
    // One check after them answers for one change, so this is the first round, not the tenth.
    assert_eq!(hop(&mut sweep, &[shell("cargo test")]), None);
    assert_eq!(round(&mut sweep, 10), None);
    assert!(round(&mut sweep, 11).is_some());
}

#[test]
fn a_change_waits_for_its_check_through_hops_that_only_read() {
    let looks = [
        call("read_file", json!({"path": "src/config.rs"})),
        call("list_dir", json!({"path": "src"})),
        shell("grep -n retries src/config.rs"),
    ];
    let mut sweep = NumberSweep::default();
    for n in 1..=3 {
        assert_eq!(hop(&mut sweep, &[retries(n)]), None);
        for look in &looks {
            assert_eq!(hop(&mut sweep, std::slice::from_ref(look)), None);
        }
        let note = hop(&mut sweep, &[shell("cargo test")]);
        assert_eq!(note.is_some(), n == 3, "round {n}");
    }

    // A structural edit between the change and its check still starts the count again.
    let mut sweep = NumberSweep::default();
    for n in 1..=4 {
        assert_eq!(hop(&mut sweep, &[retries(n)]), None);
        assert_eq!(hop(&mut sweep, std::slice::from_ref(&looks[0])), None);
        let rewrite = replace("retry(n)", "retry_with_backoff(n)");
        assert_eq!(hop(&mut sweep, &[rewrite]), None);
        assert_eq!(hop(&mut sweep, &[shell("cargo test")]), None);
    }
    assert_eq!(round(&mut sweep, 5), None);
    assert_eq!(round(&mut sweep, 6), None);
    assert!(round(&mut sweep, 7).is_some());
}

#[test]
fn a_heredoc_that_only_names_the_benchmark_is_not_a_check() {
    let note = shell("cat >> notes.md <<'EOF'\nnext: rerun ./benchmark.sh and make verify\nEOF");
    let mut sweep = NumberSweep::default();
    for n in 1..=5 {
        assert_eq!(hop(&mut sweep, &[retries(n)]), None);
        assert_eq!(hop(&mut sweep, std::slice::from_ref(&note)), None);
    }
    assert_eq!(round(&mut sweep, 6), None);
    assert_eq!(round(&mut sweep, 7), None);
    assert!(round(&mut sweep, 8).is_some());
}

#[test]
fn only_a_command_that_runs_a_checker_is_a_check() {
    for command in [
        "./benchmark.sh",
        "timeout 600 ./bench.sh --rounds 3",
        "cd build && LIMIT=4 ../tools/api-benchmark --quick",
        "make verify",
        "python3 run_eval.py --seed 7",
        "uv run python scripts/evaluate.py",
        "npm run validate",
        "cargo bench",
        "cargo run --release --bin measure",
        "for n in 1 2 3; do LIMIT=$n ./score.sh; done",
        "bash -lc 'cd /w/repo && ./submit.sh'",
        "out=$(./benchmark.sh 2>&1); echo \"$out\" | tail -3",
        "cargo test --release",
    ] {
        assert!(mentions_check_run(command), "{command}");
    }
    for command in [
        "echo benchmark",
        "echo 'next: ./benchmark.sh'",
        "cat benchmark.sh",
        "vim run_eval.py",
        "grep -n score results.txt",
        "git add verify.py && git commit -m 'bench'",
        "python3 plot.py eval.csv",
        "python3 -c \"print(open('out/score.json').read())\"",
        // The shell builtin, not a checker.
        "eval \"$(tool env)\"",
        "ls # ./benchmark.sh",
        "cat > notes.md <<'EOF'\nran ./benchmark.sh, then make verify\nEOF",
        "python3 - <<'EOF'\nimport benchmark\nbenchmark.run()\nEOF",
    ] {
        assert!(!mentions_check_run(command), "{command}");
    }
}

#[test]
fn a_call_that_did_not_land_is_not_a_change_but_a_red_check_is_a_check() {
    let mut sweep = NumberSweep::default();
    let check = shell("cargo test");
    let denied = shell("sed -i 's/retries = 3/retries = 4/' src/config.rs && ./benchmark.sh");
    // An edit that returned an error wrote nothing; a denied command did not run.
    for n in 1..=4 {
        let failed = [(&retries(n), ExecutionOutcome::Failed)];
        assert_eq!(sweep.observe(failed), None);
        assert_eq!(hop(&mut sweep, std::slice::from_ref(&check)), None);
        assert_eq!(sweep.observe([(&denied, ExecutionOutcome::Denied)]), None);
    }
    // A check that ran and came back red is still a check.
    for n in 1..=3 {
        assert_eq!(hop(&mut sweep, &[retries(n)]), None);
        let red = sweep.observe([(&check, ExecutionOutcome::Failed)]);
        assert_eq!(red.is_some(), n == 3, "round {n}");
    }
}
