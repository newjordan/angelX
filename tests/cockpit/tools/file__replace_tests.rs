use super::*;

struct RecoveryFixture(PathBuf);

impl RecoveryFixture {
    fn new() -> Self {
        static NEXT: AtomicU64 = AtomicU64::new(0);
        let root = std::env::temp_dir().join(format!(
            "angel-edit-recovery-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed),
        ));
        std::fs::create_dir(&root).unwrap();
        Self(root)
    }
}

impl Drop for RecoveryFixture {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn recovery_fields(error: &str) -> Value {
    let line = error
        .lines()
        .find_map(|line| line.strip_prefix("[edit recovery] "))
        .unwrap_or_else(|| panic!("missing recovery: {error}"));
    serde_json::from_str(line).unwrap()
}

#[test]
fn indentation_recovery_supplies_exact_bytes_for_one_guarded_retry() {
    let _env = crate::tests::env_lock();
    let fixture = RecoveryFixture::new();
    let tool = StrReplaceTool {
        root: fixture.0.clone(),
    };
    let rust_block = (0..25)
        .map(|n| format!("    call_before_{n}();"))
        .collect::<Vec<_>>()
        .join("\n");
    let cases = vec![
        (
            "long.rs",
            format!("fn f() {{\n{rust_block}\n}}\n"),
            rust_block.replace("    ", "      "),
            rust_block,
        ),
        (
            "README.md",
            "# before\ntext before\n".into(),
            "   # before\n   text before".into(),
            "# before\ntext before".into(),
        ),
        (
            "single.rs",
            "    before();\n".into(),
            "        before();".into(),
            "    before();".into(),
        ),
        (
            "tabs.rs",
            "\tbefore();\n\tother();\n".into(),
            "    before();\n    other();".into(),
            "\tbefore();\n\tother();".into(),
        ),
        (
            "mixed.rs",
            "\t  before();\n  \tother();\n".into(),
            "      before();\n      other();".into(),
            "\t  before();\n  \tother();".into(),
        ),
        (
            "suite.py",
            "if True:\n    before()\n".into(),
            "        before()".into(),
            "    before()".into(),
        ),
        (
            "config.yaml",
            "jobs:\n  before: value\n".into(),
            "      before: value".into(),
            "  before: value".into(),
        ),
        (
            "crlf.rs",
            "\tbefore();\r\n\tother();\r\n".into(),
            "    before();\n    other();\n".into(),
            "\tbefore();\r\n\tother();\r\n".into(),
        ),
    ];
    for (path, original, old, exact) in cases {
        std::fs::write(fixture.0.join(path), &original).unwrap();
        let error = tool
            .call(&serde_json::json!({
                "path": path, "old": old, "new": "must not be written",
            }))
            .unwrap_err();
        assert_eq!(
            std::fs::read_to_string(fixture.0.join(path)).unwrap(),
            original
        );
        let fields = recovery_fields(&error);
        assert_eq!(fields["old"], exact, "{path}: {error}");
        assert_eq!(
            fields["expect_tag"],
            crate::hashline::content_tag(&original)
        );
        let new = exact.replace("before", "after");
        tool.call(&serde_json::json!({
            "path": path, "old": fields["old"], "expect_tag": fields["expect_tag"], "new": new,
        }))
        .unwrap();
        assert_eq!(
            std::fs::read_to_string(fixture.0.join(path)).unwrap(),
            original.replacen(&exact, &new, 1)
        );
    }
}

#[test]
fn edit_recovery_preserves_ambiguity_and_stale_guards() {
    let _env = crate::tests::env_lock();
    let fixture = RecoveryFixture::new();
    let tool = StrReplaceTool {
        root: fixture.0.clone(),
    };
    let path = fixture.0.join("source.rs");
    for original in [
        "    before();\n    before();\n",
        "    before();\n\tbefore();\n",
    ] {
        std::fs::write(&path, original).unwrap();
        let error = tool
            .call(
                &serde_json::json!({"path":"source.rs","old":"        before();","new":"after();"}),
            )
            .unwrap_err();
        assert!(!error.contains("[edit recovery]"), "{error}");
        assert_eq!(std::fs::read_to_string(&path).unwrap(), original);
    }
    std::fs::write(&path, "    before();\n").unwrap();
    let error = tool
        .call(&serde_json::json!({"path":"source.rs","old":"        before();","new":"after();"}))
        .unwrap_err();
    let fields = recovery_fields(&error);
    let changed = "    before();\n// external edit\n";
    std::fs::write(&path, changed).unwrap();
    let error = tool.call(&serde_json::json!({"path":"source.rs","old":fields["old"],"expect_tag":fields["expect_tag"],"new":"    after();"})).unwrap_err();
    assert!(error.contains("stale edit"), "{error}");
    assert_eq!(std::fs::read_to_string(&path).unwrap(), changed);
}

#[test]
fn atomic_recovery_retries_the_list_against_the_original_live_tag() {
    let _env = crate::tests::env_lock();
    let fixture = RecoveryFixture::new();
    let original = "    before();\n    finish_before();\n";
    std::fs::write(fixture.0.join("source.rs"), original).unwrap();
    let tool = MultiEditTool {
        root: fixture.0.clone(),
    };
    let mut args = serde_json::json!({"path":"source.rs","edits":[
        {"old":"    before();","new":"    middle();"},
        {"old":"        finish_before();","new":"    finish_after();"},
    ]});
    let error = tool.call(&args).unwrap_err();
    assert_eq!(
        std::fs::read_to_string(fixture.0.join("source.rs")).unwrap(),
        original
    );
    let fields = recovery_fields(&error);
    assert_eq!(fields["edit_index"], 2);
    assert_eq!(fields["expect_tag"], crate::hashline::content_tag(original));
    assert!(error.contains("ENTIRE multi_edit list"), "{error}");
    args["expect_tag"] = fields["expect_tag"].clone();
    args["edits"][1]["old"] = fields["old"].clone();
    tool.call(&args).unwrap();
    assert_eq!(
        std::fs::read_to_string(fixture.0.join("source.rs")).unwrap(),
        "    middle();\n    finish_after();\n"
    );
}

#[test]
fn edit_recovery_does_not_clip_authoritative_unicode_or_large_regions() {
    let exact = format!("    before_{}();", "界".repeat(100));
    let wrong = format!("    {exact}");
    let error = replacement_recovery(
        &exact,
        &wrong,
        locate_replacement(&exact, &wrong).unwrap_err(),
        None,
    );
    assert_eq!(recovery_fields(&error)["old"], exact);
    let huge = (0..1000)
        .map(|n| format!("    before_{n}();"))
        .collect::<Vec<_>>()
        .join("\n");
    let wrong = huge.replace("    ", "        ");
    let error = replacement_recovery(
        &huge,
        &wrong,
        locate_replacement(&huge, &wrong).unwrap_err(),
        None,
    );
    assert!(!error.contains("[edit recovery]"), "no clipped JSON");
    assert!(error.contains("no clipped edit supplied"));
    assert!(
        error.len() < 4096,
        "bounded diagnostic: {} bytes",
        error.len()
    );
}

#[test]
fn outline_handle_mistake_points_to_handle_disclosure() {
    let error = read_outline_uri(Path::new("."), "/hnd_bs").unwrap_err();
    assert!(error.contains("agent://hnd_bs"), "got: {error}");
    assert!(error.contains("handle_read"), "got: {error}");
}

#[test]
fn locate_exact_then_fuzzy_tiers() {
    let content = "fn f() {\n    let x =  1;\n    a();\n    b();\n    c();\n}\n";

    // Tier 1: exact substring.
    let (s, e, fuzzy) = locate_replacement(content, "    a();").unwrap();
    assert_eq!(&content[s..e], "    a();");
    assert!(!fuzzy);

    // Tier 2: internal whitespace collapsed ("= 1" vs "=  1"), leading indent
    // still matched so no re-indent.
    let (s, e, fuzzy) = locate_replacement(content, "    let x = 1;").unwrap();
    assert_eq!(&content[s..e], "    let x =  1;");
    assert!(fuzzy);

    // Genuinely absent → error.
    assert!(locate_replacement(content, "    zzz();").is_err());

    // Different leading indent must NOT match (no silent re-indent): the line
    // exists at 4-space indent; asking at 8 spaces must refuse.
    assert!(locate_replacement(content, "        a();").is_err());
}

#[test]
fn miss_errors_carry_a_near_miss_report() {
    let content = "fn f() {\n    let x = 1;\n    a();\n    b();\n    c();\n}\n";

    // Indentation-only miss: every line matches after trim → the error
    // names leading whitespace and shows the authoritative region.
    let err = locate_replacement(content, "	a();\n	b();").unwrap_err();
    assert!(err.contains("LEADING WHITESPACE"), "got: {err}");
    assert!(err.contains("    a();"), "region shown: {err}");

    // Partial-content miss: one line right, one line stale → closest
    // region with the real lines, and a changed-since-read hint.
    let err = locate_replacement(content, "    b();\n    stale_line();").unwrap_err();
    assert!(err.contains("Closest region"), "got: {err}");
    assert!(err.contains("    c();"), "authoritative line shown: {err}");

    // Nothing matches anywhere → tell the model to re-read, no fake region.
    let err = locate_replacement(content, "    zzz();\n    qqq();").unwrap_err();
    assert!(err.contains("Re-read the file"), "got: {err}");
}
