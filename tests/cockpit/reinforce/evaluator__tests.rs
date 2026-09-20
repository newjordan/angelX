use super::*;

fn git_fixture(name: &str, contents: &[u8]) -> PathBuf {
    let root = std::env::temp_dir().join(format!(
        "angel-evaluator-spec-{name}-{}",
        std::process::id()
    ));
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir_all(&root).unwrap();
    assert!(
        std::process::Command::new("git")
            .args(["init", "-q"])
            .current_dir(&root)
            .status()
            .unwrap()
            .success()
    );
    std::fs::write(root.join("fixture.txt"), contents).unwrap();
    assert!(
        std::process::Command::new("git")
            .args(["add", "fixture.txt"])
            .current_dir(&root)
            .status()
            .unwrap()
            .success()
    );
    root
}

#[test]
fn evaluator_execution_spec_is_structured_and_drift_sensitive() {
    let fixture_root = git_fixture("drift", b"fixture-v1");
    let inventory = crate::knowledge::cut::sha256_hex(b"two-tests");
    let true_tool = std::fs::canonicalize("/usr/bin/true").unwrap();
    let command = format!("{} outcome", true_tool.display());
    let inventory_command = format!("{} inventory", true_tool.display());
    let first = EvaluatorSpec::new(
        &fixture_root,
        &inventory,
        "technical-pass/v1",
        &command,
        &inventory_command,
        &[true_tool.as_path()],
    )
    .unwrap();
    let changed = EvaluatorSpec::new(
        &fixture_root,
        &inventory,
        "technical-pass/v2",
        &command,
        &inventory_command,
        &[true_tool.as_path()],
    )
    .unwrap();
    assert_ne!(first.manifest_sha256(), changed.manifest_sha256());
    let inventory_command_changed = EvaluatorSpec::new(
        &fixture_root,
        &inventory,
        "technical-pass/v1",
        &command,
        &format!("{} changed-inventory", true_tool.display()),
        &[true_tool.as_path()],
    )
    .unwrap();
    assert_ne!(
        first.manifest_sha256(),
        inventory_command_changed.manifest_sha256()
    );
    assert!(
        EvaluatorSpec::new(
            &fixture_root,
            "claimed",
            "parser",
            &command,
            &inventory_command,
            &[true_tool.as_path()],
        )
        .is_err()
    );
    assert_eq!(
        first.fixture_sha256,
        crate::agent::harness::workspace_evidence_sha256(&fixture_root).unwrap()
    );
    assert_eq!(first.expected_inventory_sha256, inventory);
    assert_eq!(first.parser_contract, "technical-pass/v1");
    assert!(
        EvaluatorSpec::new(
            &fixture_root,
            &inventory,
            "technical-pass/v1",
            "tool=/usr/bin/true $tool",
            &inventory_command,
            &[true_tool.as_path()],
        )
        .unwrap_err()
        .contains("simple, non-expanding")
    );
    let shell = std::fs::canonicalize("/bin/sh").unwrap();
    assert!(
        EvaluatorSpec::new(
            &fixture_root,
            &inventory,
            "technical-pass/v1",
            &format!("{} -c /usr/bin/tr\\ue", shell.display()),
            &inventory_command,
            &[shell.as_path(), true_tool.as_path()],
        )
        .unwrap_err()
        .contains("simple, non-expanding")
    );

    let root =
        std::env::temp_dir().join(format!("angel-evaluator-tool-spec-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir_all(&root).unwrap();
    let tool = root.join("tool");
    std::fs::copy("/usr/bin/true", &tool).unwrap();
    let command = tool.to_string_lossy().into_owned();
    #[cfg(unix)]
    {
        let alias = root.join("tool-alias");
        std::os::unix::fs::symlink(&tool, &alias).unwrap();
        let alias_command = alias.to_string_lossy().into_owned();
        assert!(
            EvaluatorSpec::new(
                &fixture_root,
                &inventory,
                "technical-pass/v1",
                &alias_command,
                &alias_command,
                &[alias.as_path()],
            )
            .unwrap_err()
            .contains("canonical path")
        );
    }
    assert!(
        EvaluatorSpec::new(
            &fixture_root,
            &inventory,
            "technical-pass/v1",
            &command,
            &command,
            &[],
        )
        .unwrap_err()
        .contains("not a declared tool")
    );
    let pinned = EvaluatorSpec::new(
        &fixture_root,
        &inventory,
        "technical-pass/v1",
        &command,
        &command,
        &[tool.as_path()],
    )
    .unwrap();
    pinned.validate_runtime().unwrap();
    std::fs::write(&tool, b"replacement").unwrap();
    assert!(
        pinned
            .validate_runtime()
            .unwrap_err()
            .contains("resolved tool drift")
    );
    assert!(
        EvaluatorSpec::new(
            &fixture_root,
            &inventory,
            "technical-pass/v1",
            "tool",
            "tool",
            &[Path::new("tool")],
        )
        .is_err()
    );
    std::fs::write(fixture_root.join("fixture.txt"), b"drifted").unwrap();
    assert!(
        first
            .validate_runtime()
            .unwrap_err()
            .contains("fixture drift")
    );
    let _ = std::fs::remove_dir_all(root);
    let _ = std::fs::remove_dir_all(fixture_root);
}

#[test]
fn evaluator_inventory_requires_exact_sorted_test_ids() {
    let canonical = b"case-a::test_one\ncase-b::test_two\n";
    assert_eq!(
        canonical_inventory(canonical).unwrap().sha256,
        crate::knowledge::cut::sha256_hex(canonical)
    );
    for changed in [
        b"case-a::test_one\n".as_slice(),
        b"case-a::test_one\ncase-b::renamed\n".as_slice(),
        b"case-a::test_one\ncase-b::test_two\ncase-c::test_three\n".as_slice(),
    ] {
        assert_ne!(
            canonical_inventory(changed).unwrap().sha256,
            canonical_inventory(canonical).unwrap().sha256
        );
    }
    for invalid in [
        b"case-a::test_one".as_slice(),
        b"case-b::test_two\ncase-a::test_one\n".as_slice(),
        b"case-a::test_one\ncase-a::test_one\n".as_slice(),
        b" case-a::test_one\n".as_slice(),
        b"case-a::test_one\r\n".as_slice(),
    ] {
        assert!(canonical_inventory(invalid).is_err());
    }
    assert!(canonical_inventory("case-a::\u{202e}test\n".as_bytes()).is_err());
}

#[test]
fn evaluator_outcome_requires_exact_executed_test_ids() {
    let outcome = canonical_test_outcome(
        b"angel.rlvr.test-outcome/v1\ncase-a::one\tpass\ncase-b::two\tfail\n",
    )
    .unwrap();
    assert_eq!(outcome.passed, 1);
    assert_eq!(outcome.failed, 1);
    assert_eq!(
        outcome.inventory_sha256,
        crate::knowledge::cut::sha256_hex(b"case-a::one\ncase-b::two\n")
    );

    for invalid in [
        b"test result: ok. 999 passed; 0 failed;\n".as_slice(),
        b"angel.rlvr.test-outcome/v1\ncase-a::one\tpass\ntest result: ok. 999 passed; 0 failed;\n"
            .as_slice(),
        b"angel.rlvr.test-outcome/v1\ncase-a::one\tpass\ncase-a::one\tpass\n".as_slice(),
        b"angel.rlvr.test-outcome/v1\ncase-a::one\tskipped\n".as_slice(),
        b"angel.rlvr.test-outcome/v1\ncase-b::two\tpass\ncase-a::one\tpass\n".as_slice(),
        b"angel.rlvr.test-outcome/v1\n".as_slice(),
    ] {
        assert!(canonical_test_outcome(invalid).is_err());
    }
}
