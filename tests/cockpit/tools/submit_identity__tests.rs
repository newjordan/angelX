use super::*;

#[test]
fn non_submissions_pass_untouched() {
    let _env = crate::tests::env_lock();
    assert!(stamp("hilbert submissions --all", None).unwrap().is_none());
    assert!(stamp("cargo build --release", None).unwrap().is_none());
    for command in [
        "echo 'yukon submit'",
        "rg 'hilbert submit' README.md",
        "cat <<'EOF'\nyukon submit\nEOF",
    ] {
        assert!(stamp(command, None).unwrap().is_none(), "{command}");
    }
    assert!(stamp("yukon submit", None).is_err());
}

#[test]
fn quoted_nested_heredocs_are_literal_data_with_stable_attribution_offsets() {
    let _env = crate::tests::env_lock();
    let _model = crate::harness::run_identity::LiveModelScope::enter(Some("fixture-model".into()));
    for (header, body, delimiter) in [
        (
            "cat <<'MSG'",
            "fixture\nLiteral: yukon submit --model copied\nMSG_prefix\n",
            "MSG",
        ),
        ("cat <<\"MSG\"", "$(yukon submit) `hilbert submit`\n", "MSG"),
        ("cat <<-'MSG'", "\tfixture\n\tyukon submit\n", "\tMSG"),
    ] {
        let command = format!("git commit -m \"$({header}\n{body}{delimiter}\n)\"");
        assert!(stamp(&command, None).unwrap().is_none(), "{command}");
        let combined = format!("{command}; yukon submit --model copied --harness other");
        let stamped = stamp(&combined, None).unwrap().unwrap();
        assert!(
            stamped.command.starts_with(&command),
            "literal message changed: {}",
            stamped.command
        );
        assert!(
            stamped.command[command.len()..].contains("--model 'fixture-model' --harness 'angel0'")
        );
        assert!(!stamped.command[command.len()..].contains("copied"));
    }
}

#[test]
fn nested_executable_submissions_and_unsupported_heredocs_fail_closed() {
    let _env = crate::tests::env_lock();
    let _model = crate::harness::run_identity::LiveModelScope::enter(None);
    for command in [
        "echo \"$(yukon submit --model copied)\"",
        "echo \"`hilbert submit`\"",
        "git commit -m \"$(cat <<'MSG'\ntext\nMSG\nyukon submit\n)\"",
        "git commit -m \"$(cat <<'MSG' | yukon submit\ntext\nMSG\n)\"",
        "git commit -m \"$(cat <<MSG\n$(yukon submit)\nMSG\n)\"",
        "cat <<MSG\n$(yukon submit)\nMSG\n",
        "git commit -m \"$(cat <<'MSG' <<'NEXT'\ntext\nMSG\nNEXT\n)\"",
        "git commit -m \"$(cat <<'MSG\ntext\nMSG\n)\"",
        "git commit -m \"$(cat <<'MSG'\ntext\nMSG_prefix\n)\"",
    ] {
        assert!(
            stamp(command, None).is_err(),
            "must refuse ambiguous execution: {command}"
        );
    }
    // Data in either quote style remains data, not an executable scan hit.
    assert!(stamp("echo '$(yukon submit)'", None).unwrap().is_none());
    assert!(
        stamp("cat <<'MSG'\n$(yukon submit)\nMSG\n", None)
            .unwrap()
            .is_none()
    );
    let outside = "git commit -m \"$(cat <<'MSG'\nliteral\nMSG\n)\"; yukon submit";
    assert!(
        stamp(outside, None).is_err(),
        "a later submit still needs live identity"
    );
}

#[test]
fn local_git_commit_accepts_a_quoted_heredoc_message_without_execution() {
    let _env = crate::tests::env_lock();
    let _model = crate::harness::run_identity::LiveModelScope::enter(None);
    static NEXT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
    let dir = std::env::temp_dir().join(format!(
        "angel-heredoc-commit-{}-{}",
        std::process::id(),
        NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed),
    ));
    std::fs::create_dir(&dir).unwrap();
    struct Fixture(PathBuf);
    impl Drop for Fixture {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }
    let _fixture = Fixture(dir.clone());
    let run = |program: &str, args: &[&str]| {
        std::process::Command::new(program)
            .args(args)
            .current_dir(&dir)
            .env_remove("GIT_DIR")
            .env_remove("GIT_WORK_TREE")
            .env_remove("GIT_INDEX_FILE")
            .env("GIT_CONFIG_NOSYSTEM", "1")
            .env("GIT_CONFIG_GLOBAL", "/dev/null")
            .output()
            .unwrap()
    };
    assert!(run("git", &["init", "--quiet"]).status.success());
    let command = "git -c user.name=Fixture -c user.email=fixture@example.invalid -c core.hooksPath=/dev/null -c commit.gpgsign=false commit --quiet --allow-empty -m \"$(cat <<'MSG'\nfixture change\n\nLiteral: yukon submit --model copied\n$(touch must-not-exist)\nMSG\n)\"";
    assert!(stamp(command, Some(&dir)).unwrap().is_none());
    let output = run(
        "bash",
        &["--noprofile", "--norc", "-o", "pipefail", "-c", command],
    );
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(!dir.join("must-not-exist").exists());
    let message = run("git", &["log", "-1", "--format=%B"]);
    assert!(message.status.success());
    assert_eq!(
        String::from_utf8(message.stdout).unwrap().trim_end(),
        "fixture change\n\nLiteral: yukon submit --model copied\n$(touch must-not-exist)"
    );
}

#[test]
fn copied_attribution_flags_are_replaced_with_the_live_identity() {
    let _env = crate::tests::env_lock();
    let _model =
        crate::harness::run_identity::LiveModelScope::enter(Some("deepseek-v4-flash".into()));
    let cmd = r#"cd /w && hilbert submit --note-file prof/out/note.md --model "GPT 5.6 Sol" --harness "Codex" && echo done"#;
    let stamped = stamp(cmd, None).unwrap().expect("submission");
    assert!(
        !stamped.command.contains("GPT 5.6 Sol"),
        "{}",
        stamped.command
    );
    assert!(!stamped.command.contains("Codex"), "{}", stamped.command);
    let expected_flags = "hilbert submit --model 'deepseek-v4-flash' --harness 'angel0'";
    assert!(
        stamped.command.contains(expected_flags),
        "{}",
        stamped.command
    );
    assert!(
        stamped.command.ends_with("&& echo done"),
        "{}",
        stamped.command
    );
    assert!(
        stamped.notice.contains("model=deepseek-v4-flash"),
        "{}",
        stamped.notice
    );
    // Flags absent entirely: added, since the CLI requires them.
    let bare = stamp("yukon submit --note 'v2'", None)
        .unwrap()
        .expect("submission");
    assert!(
        bare.command.starts_with("yukon submit --model "),
        "{}",
        bare.command
    );
    // `--flag=value` spelling and repeated flags are all removed.
    let eq = stamp(
        "hilbert submit --model=Claude --harness='Other Harness' --model x",
        None,
    )
    .unwrap()
    .unwrap();
    assert_eq!(eq.command.matches("--model").count(), 1, "{}", eq.command);
    assert_eq!(eq.command.matches("--harness").count(), 1, "{}", eq.command);
    let compound = stamp(
        "echo --model keep && yukon submit --model copied; echo --harness keep",
        None,
    )
    .unwrap()
    .unwrap();
    assert!(
        compound
            .command
            .starts_with("echo --model keep && yukon submit --model 'deepseek-v4-flash'")
    );
    assert!(compound.command.ends_with("; echo --harness keep"));
    assert!(stamp("yukon submit --model $MODEL", None).is_err());
    assert!(stamp("bash -c 'yukon submit'", None).is_err());
}

#[test]
fn note_file_model_and_harness_lines_are_corrected() {
    let _env = crate::tests::env_lock();
    let _model =
        crate::harness::run_identity::LiveModelScope::enter(Some("deepseek-v4-flash".into()));
    let dir = std::env::temp_dir().join(format!("angel_submit_identity_{}", std::process::id()));
    std::fs::create_dir_all(dir.join("prof")).unwrap();
    let note = dir.join("prof").join("note.md");
    std::fs::write(
        &note,
        "Model: GPT 5.6 Sol\n**Harness:** Codex\n\n# Row tile\nThe model: unchanged prose here.\n",
    )
    .unwrap();
    let cmd = format!(
        "cd {} && hilbert submit --note-file prof/note.md",
        dir.display()
    );
    let stamped = stamp(&cmd, None).unwrap().expect("submission");
    let text = std::fs::read_to_string(&note).unwrap();
    assert_eq!(
        text,
        "Model: deepseek-v4-flash\n**Harness:** angel0\n\n# Row tile\nThe model: unchanged prose here.\n"
    );
    assert!(
        stamped.notice.contains("2 Model/Harness line(s) corrected"),
        "{}",
        stamped.notice
    );
    // Idempotent: a second pass corrects nothing.
    let again = stamp(&cmd, None).unwrap().expect("submission");
    assert!(!again.notice.contains("corrected"), "{}", again.notice);
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn journal_execution_records_refused_submit_without_network() {
    let _env = crate::tests::env_lock();
    let _model =
        crate::harness::run_identity::LiveModelScope::enter(Some("deepseek-v4-flash".into()));
    let _ = drain_journal();
    journal_execution(
        "shell",
        "yukon submit --model grok-4.6 --harness angel0 --note-file /tmp/nope.md",
        None,
        Some(2),
        "rejected: byte-gate refused this payload",
    );
    let rows = drain_journal();
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].tool, "shell");
    assert_eq!(rows[0].exit_code, 2);
    assert_eq!(rows[0].outcome, "refused");
    assert!(rows[0].platform_response_excerpt.contains("byte-gate"));
    assert_eq!(rows[0].harness, HARNESS_LABEL);
    assert_eq!(rows[0].model, "deepseek-v4-flash");
}

#[test]
fn model_switches_and_concurrent_seats_restore_identity() {
    let _env = crate::tests::env_lock();
    use crate::harness::run_identity::LiveModelScope;
    let _empty = LiveModelScope::enter(None);
    assert!(model_label().is_err());
    for model in ["gpt-6-astra", "deepseek-v4-flash"] {
        let _seat = LiveModelScope::enter(Some(model.into()));
        assert!(
            stamp("yukon submit", None)
                .unwrap()
                .unwrap()
                .command
                .contains(model)
        );
        std::thread::scope(|scope| {
            let worker = scope.spawn(|| {
                assert!(model_label().is_err());
                let _delegate = LiveModelScope::enter(Some("grok".into()));
                assert_eq!(model_label().unwrap(), "grok");
            });
            assert_eq!(model_label().unwrap(), model);
            worker.join().unwrap();
        });
        {
            let _unknown = LiveModelScope::enter(None);
            assert!(stamp("yukon submit", None).is_err());
        }
        assert_eq!(model_label().unwrap(), model);
    }
    assert!(model_label().is_err());
}

#[test]
fn output_prose_and_enqueue_are_not_terminal_acceptance() {
    for text in [
        "accepted",
        "submitted",
        "not accepted",
        "not submitted",
        "submission was never accepted",
        "exit zero",
        "Submission queued but not dispatched",
    ] {
        assert_eq!(classify_outcome(0, text), "unknown", "{text}");
    }
    assert_eq!(classify_outcome(0, "rejected"), "rejected");
    assert_eq!(classify_outcome(2, "accepted"), "refused");
    let receipt = "Submission queued\n11111111-2222-3333-4444-555555555555";
    assert_eq!(classify_outcome(0, receipt), "dispatched");
    // Even a status table in arbitrary shell output cannot certify acceptance.
    let table = "7871bd4 newjordan accepted 519469.35 {} +1% 380b04d yesterday";
    assert_eq!(classify_outcome(0, table), "unknown");
    // The established Yukon status contract retains the terminal distinction.
    let rows = crate::harness::comp_packages::yukon::fleet::parse_submission_table("test/board", table);
    assert_eq!(
        rows[0].phase,
        crate::harness::comp_packages::yukon::fleet::YukonSubmissionPhase::Accepted
    );
}
