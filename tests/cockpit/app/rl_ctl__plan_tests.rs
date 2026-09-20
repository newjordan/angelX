use super::*;

#[test]
fn plan_rejects_unknown_flags_and_unnamed_inputs() {
    let workspace = std::env::temp_dir().join("angel-rl-plan-missing");
    let _ = std::fs::create_dir_all(&workspace);
    let unknown = RlPlan::parse(&workspace, &["--steps".to_string()]).unwrap_err();
    assert!(unknown.contains("unknown /rl run argument"), "{unknown}");
    let half = RlPlan::parse(&workspace, &["--task".to_string(), "tidy".to_string()]).unwrap_err();
    assert!(half.contains("--verify"), "{half}");
    let no_objective = RlPlan::parse(&workspace, &[]).unwrap_err();
    assert!(no_objective.contains("/goal"), "{no_objective}");
    let bad_case =
        RlPlan::parse(&workspace, &["--case".to_string(), "no check".to_string()]).unwrap_err();
    assert!(bad_case.contains("::"), "{bad_case}");
    let zero = RlPlan::parse(&workspace, &["--rounds".to_string(), "0".to_string()]).unwrap_err();
    assert!(zero.contains("at least 1"), "{zero}");
    let _ = std::fs::remove_dir_all(&workspace);
}

#[test]
fn plan_keeps_operator_sizes_and_separates_audit_cases() {
    let workspace = std::env::temp_dir().join("angel-rl-plan-sizes");
    let audit_source = workspace.join("audit-source");
    let _ = std::fs::create_dir_all(&audit_source);
    let plan = RlPlan::parse(
        &workspace,
        &[
            "--rounds".to_string(),
            "3".to_string(),
            "--group".to_string(),
            "1".to_string(),
            "--samples".to_string(),
            "2".to_string(),
            "--task".to_string(),
            "fix the parser".to_string(),
            "--verify".to_string(),
            "cargo test".to_string(),
            "--case".to_string(),
            "other objective :: other check".to_string(),
            "--verify-scope".to_string(),
            "tests".to_string(),
            "--audit".to_string(),
            format!(
                "independent objective :: independent check :: {}",
                audit_source.display()
            ),
        ],
    )
    .unwrap();
    assert_eq!((plan.rounds, plan.group, plan.samples), (3, 1, 2));
    assert_eq!(plan.cases.len(), 2);
    assert_eq!(plan.cases[0].id, "objective");
    assert_eq!(plan.cases[1].id, "case-1");
    assert_eq!(plan.audit.len(), 1);
    assert_eq!(plan.audit[0].id, "audit-1");
    assert_eq!(
        plan.audit[0].source.as_deref(),
        Some(audit_source.as_path()),
        "an audit keeps its own source scope"
    );
    assert_eq!(
        plan.verifier_scope,
        vec!["tests".to_string()],
        "the verifier's own inputs travel with the campaign"
    );
    assert_eq!(
        plan.planned_attempts(),
        3 + 3 * 2 * 2 * 2 + 3 * 2 * 2,
        "3 rounds × 1 grouped attempt + 3 rounds × 2 policies × (2 samples × 2 selection + 2 × 1 audit)"
    );
    let _ = std::fs::remove_dir_all(&workspace);
}

/// Quoting is resolved at the command boundary without running a shell:
/// multiword values survive, escapes and metacharacters stay literal, and a
/// malformed quote is an actionable error rather than a silent split.
#[test]
fn command_line_splitting_is_lossless_and_reports_bad_quotes() {
    assert_eq!(
        split_command_args("--task \"multi word task\" --verify \"python -m unittest\"").unwrap(),
        vec![
            "--task",
            "multi word task",
            "--verify",
            "python -m unittest"
        ]
    );
    assert_eq!(
        split_command_args("--task 'single quoted task' --verify 'a | b > c'").unwrap(),
        vec!["--task", "single quoted task", "--verify", "a | b > c"],
        "single quotes hold shell syntax literally"
    );
    assert_eq!(
        split_command_args(r#"--task "keep \"nested\" quotes, $HOME, ; | > # and backslashes""#)
            .unwrap(),
        vec![
            "--task",
            "keep \"nested\" quotes, $HOME, ; | > # and backslashes"
        ],
        "no expansion, globbing, comment stripping or splitting inside quotes"
    );
    assert_eq!(
        split_command_args("--case audit :: sh tests/audit.sh :: /tmp/dir with spaces").unwrap(),
        vec![
            "--case",
            "audit",
            "::",
            "sh",
            "tests/audit.sh",
            "::",
            "/tmp/dir",
            "with",
            "spaces"
        ],
        "unquoted text still splits on whitespace"
    );
    assert_eq!(
        split_command_args("--verify /tmp/a\\ b/c.sh").unwrap(),
        vec!["--verify", "/tmp/a b/c.sh"],
        "an escaped space joins a path outside quotes"
    );
    assert!(split_command_args("").unwrap().is_empty());
    let unbalanced = split_command_args("--task \"unfinished").unwrap_err();
    assert!(
        unbalanced.contains("unterminated double quote"),
        "{unbalanced}"
    );
    let single = split_command_args("--task 'unfinished").unwrap_err();
    assert!(single.contains("unterminated single quote"), "{single}");
    let trailing = split_command_args("--task tail\\").unwrap_err();
    assert!(trailing.contains("backslash"), "{trailing}");
}

/// The stage's "attempt ms" is one definition everywhere: the recorded
/// generation time plus the recorded verifier wall time for the same
/// sample. No constant, floor or invented speed is involved.
#[test]
fn attempt_cost_is_the_sum_of_recorded_measurements() {
    assert_eq!(attempt_cost_ms(Some(1_500), Some(250)), 1_750);
    assert_eq!(attempt_cost_ms(Some(0), Some(12)), 12);
    assert_eq!(
        attempt_cost_ms(None, Some(12)),
        12,
        "an unrecorded generation time contributes nothing and is reported as missing"
    );
    assert_eq!(attempt_cost_ms(None, None), 0);
}

#[test]
fn audit_case_may_not_relabel_a_selection_task_or_omit_its_source() {
    let workspace = std::env::temp_dir().join("angel-rl-plan-relabel");
    let _ = std::fs::create_dir_all(&workspace);
    let relabelled = RlPlan::parse(
        &workspace,
        &[
            "--task".to_string(),
            "same task".to_string(),
            "--verify".to_string(),
            "check a".to_string(),
            "--audit".to_string(),
            format!("same task :: check b :: {}", workspace.display()),
        ],
    )
    .unwrap_err();
    assert!(
        relabelled.contains("independently authored"),
        "{relabelled}"
    );
    let no_source = RlPlan::parse(
        &workspace,
        &["--audit".to_string(), "task :: check".to_string()],
    )
    .unwrap_err();
    assert!(no_source.contains("source path"), "{no_source}");
    assert!(no_source.contains("independently authored"), "{no_source}");
    let _ = std::fs::remove_dir_all(&workspace);
}
