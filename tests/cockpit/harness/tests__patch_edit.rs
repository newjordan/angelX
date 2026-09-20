//! Apply-patch, multi-edit, str_replace, locate_replacement, and freeform patch coverage.
//!
//! Extracted from `harness/tests.rs` without behavior change so the monolith can
//! shrink while preserving the full harness test inventory.

use super::*;

// --- patch / edit suite -----------------------------------------------------

#[test]
fn apply_patch_applies_unified_diff_and_confines_paths() {
    let root = std::env::temp_dir().join(format!("angel_ap_{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir_all(&root).unwrap();
    let mut reg = ToolRegistry::new();
    register_file_tools(&mut reg, root.clone());
    std::fs::write(root.join("f.txt"), "line one\nline two\n").unwrap();

    let diff = "--- a/f.txt\n+++ b/f.txt\n@@ -1,2 +1,2 @@\n line one\n-line two\n+line TWO\n";
    let r = reg.dispatch("apply_patch", &serde_json::json!({"diff": diff}));
    // git/patch may be absent in some CI; only assert the effect when it applied.
    if let Ok(msg) = r {
        assert!(msg.contains("f.txt"), "got: {msg}");
        let after = std::fs::read_to_string(root.join("f.txt")).unwrap();
        assert_eq!(after, "line one\nline TWO\n");
    }

    // A diff that escapes the workspace is rejected before any apply.
    let bad = "--- a/../../etc/x\n+++ b/../../etc/x\n@@ -1 +1 @@\n-a\n+b\n";
    assert!(
        reg.dispatch("apply_patch", &serde_json::json!({"diff": bad}))
            .is_err()
    );
    // A diff with no file headers is rejected.
    assert!(
        reg.dispatch("apply_patch", &serde_json::json!({"diff": "garbage"}))
            .is_err()
    );

    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn apply_patch_hashline_edits_by_line_and_guards_stale_tag() {
    let root = std::env::temp_dir().join(format!("angel_hl_{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir_all(&root).unwrap();
    let mut reg = ToolRegistry::new();
    register_file_tools(&mut reg, root.clone());

    let content = "alpha\nbeta\ngamma\n";
    std::fs::write(root.join("h.rs"), content).unwrap();
    let tag = crate::agent::hashline::content_tag(content);

    // SWAP line 2 + INS.POST line 3: the model emits only the NEW rows, no
    // retyping of the old text.
    let patch = format!(
        "*** Begin Patch\n[h.rs#{tag}]\nSWAP 2:\n+BETA\nINS.POST 3:\n+delta\n*** End Patch\n"
    );
    let msg = reg
        .dispatch("apply_patch", &serde_json::json!({ "diff": patch }))
        .unwrap();
    assert!(msg.contains("hashline"), "got: {msg}");
    assert!(
        msg.contains("new tag #"),
        "receipt must mint a fresh tag: {msg}"
    );
    assert_eq!(
        std::fs::read_to_string(root.join("h.rs")).unwrap(),
        "alpha\nBETA\ngamma\ndelta\n"
    );
    let new_tag = msg
        .rsplit_once("new tag #")
        .map(|(_, t)| t.trim())
        .expect("new tag in receipt");
    assert_eq!(
        new_tag,
        crate::agent::hashline::content_tag("alpha\nBETA\ngamma\ndelta\n")
    );

    // A patch carrying the OLD tag no longer matches the changed file and is
    // rejected before any write — the stale-patch guard.
    let stale = format!("*** Begin Patch\n[h.rs#{tag}]\nSWAP 1:\n+X\n*** End Patch\n");
    let err = reg
        .dispatch("apply_patch", &serde_json::json!({ "diff": stale }))
        .unwrap_err();
    assert!(err.contains("stale"), "got: {err}");
    assert_eq!(
        std::fs::read_to_string(root.join("h.rs")).unwrap(),
        "alpha\nBETA\ngamma\ndelta\n",
        "rejected patch must not mutate the file"
    );

    // Path confinement applies to hashline sections too.
    let escape = format!("*** Begin Patch\n[../../etc/x#{tag}]\nDEL 1\n*** End Patch\n");
    assert!(
        reg.dispatch("apply_patch", &serde_json::json!({ "diff": escape }))
            .is_err()
    );

    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn apply_patch_hashline_rem_and_mv() {
    let root = std::env::temp_dir().join(format!("angel_hl_rm_{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir_all(&root).unwrap();
    let mut reg = ToolRegistry::new();
    register_file_tools(&mut reg, root.clone());

    let content = "fn a() {\n    1\n}\nfn b() {\n    2\n}\n";
    std::fs::write(root.join("m.rs"), content).unwrap();
    let tag = crate::agent::hashline::content_tag(content);

    // DEL.BLK the first function, then MV to a new path.
    let patch = format!("*** Begin Patch\n[m.rs#{tag}]\nDEL.BLK 1\nMV nest/m2.rs\n*** End Patch\n");
    let msg = reg
        .dispatch("apply_patch", &serde_json::json!({ "diff": patch }))
        .unwrap();
    assert!(msg.contains("m.rs → nest/m2.rs"), "got: {msg}");
    assert!(!root.join("m.rs").exists(), "source must be gone");
    assert_eq!(
        std::fs::read_to_string(root.join("nest/m2.rs")).unwrap(),
        "fn b() {\n    2\n}\n"
    );

    // REM the moved file.
    let tag2 = crate::agent::hashline::content_tag("fn b() {\n    2\n}\n");
    let rem = format!("*** Begin Patch\n[nest/m2.rs#{tag2}]\nREM\n*** End Patch\n");
    let msg2 = reg
        .dispatch("apply_patch", &serde_json::json!({ "diff": rem }))
        .unwrap();
    assert!(msg2.contains("removed"), "got: {msg2}");
    assert!(!root.join("nest/m2.rs").exists());

    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn read_file_hashline_anchors_on_by_default() {
    let _env = crate::tests::env_lock();
    let _anchors = crate::tests::TestEnvGuard::unset("ANGEL_HASHLINE_ANCHORS");
    let root = std::env::temp_dir().join(format!("angel_hl_rd_{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir_all(&root).unwrap();
    let mut reg = ToolRegistry::new();
    register_file_tools(&mut reg, root.clone());

    let content = "alpha\nbeta\n";
    std::fs::write(root.join("a.rs"), content).unwrap();
    let tag = crate::agent::hashline::content_tag(content);
    let page = reg
        .dispatch("read_file", &serde_json::json!({"path": "a.rs"}))
        .unwrap();
    assert!(
        page.starts_with(&format!("[a.rs#{tag}]\n")),
        "default read must carry the hashline header: {page}"
    );
    assert!(
        page.contains("1  alpha") || page.contains("1 alpha"),
        "{page}"
    );
    assert!(
        page.contains("2  beta") || page.contains("2 beta"),
        "{page}"
    );

    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn conflict_uri_registers_on_read_and_resolves_via_write() {
    let _env = crate::tests::env_lock();
    let _anchors = crate::tests::TestEnvGuard::set("ANGEL_HASHLINE_ANCHORS", "0");
    crate::agent::conflict::clear_conflicts_for_test();
    let root = std::env::temp_dir().join(format!("angel_cflt_{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir_all(&root).unwrap();
    let mut reg = ToolRegistry::new();
    register_file_tools(&mut reg, root.clone());

    let body = "head\n<<<<<<< HEAD\nours\n=======\ntheirs\n>>>>>>> branch\ntail\n";
    std::fs::write(root.join("c.rs"), body).unwrap();
    let page = reg
        .dispatch("read_file", &serde_json::json!({"path": "c.rs"}))
        .unwrap();
    assert!(page.contains("conflict://"), "footer missing: {page}");
    assert!(page.contains("merge conflict"), "{page}");

    let list = reg
        .dispatch("read_file", &serde_json::json!({"path": "conflict://"}))
        .unwrap();
    assert!(list.contains("conflict://"), "{list}");

    // Extract first id.
    let id = list
        .lines()
        .find_map(|l| {
            l.trim()
                .strip_prefix("conflict://")
                .and_then(|rest| rest.split_whitespace().next())
                .and_then(|n| n.parse::<u32>().ok())
        })
        .or_else(|| {
            page.lines().find_map(|l| {
                l.trim()
                    .strip_prefix("conflict://")
                    .and_then(|rest| rest.split_whitespace().next())
                    .and_then(|n| n.parse::<u32>().ok())
            })
        })
        .expect("conflict id");

    let detail = reg
        .dispatch(
            "read_file",
            &serde_json::json!({"path": format!("conflict://{id}/theirs")}),
        )
        .unwrap();
    assert!(detail.contains("theirs"), "{detail}");

    let msg = reg
        .dispatch(
            "write_file",
            &serde_json::json!({
                "path": format!("conflict://{id}"),
                "content": "@theirs"
            }),
        )
        .unwrap();
    assert!(msg.contains("resolved"), "{msg}");
    assert_eq!(
        std::fs::read_to_string(root.join("c.rs")).unwrap(),
        "head\ntheirs\ntail\n"
    );

    crate::agent::conflict::clear_conflicts_for_test();
    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn skill_uri_lists_catalog() {
    let root = std::env::temp_dir().join(format!("angel_skilluri_{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir_all(&root).unwrap();
    let mut reg = ToolRegistry::new();
    register_file_tools(&mut reg, root.clone());
    let catalog = reg
        .dispatch("read_file", &serde_json::json!({"path": "skill://"}))
        .unwrap();
    // Workspace may have zero project skills but shipped skills usually exist.
    assert!(
        catalog.contains("skill://") || catalog.contains("no skills"),
        "{catalog}"
    );
    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn outline_uri_maps_symbols() {
    let _env = crate::tests::env_lock();
    let _anchors = crate::tests::TestEnvGuard::set("ANGEL_HASHLINE_ANCHORS", "0");
    let root = std::env::temp_dir().join(format!("angel_outline_uri_{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir_all(&root).unwrap();
    let mut reg = ToolRegistry::new();
    register_file_tools(&mut reg, root.clone());
    std::fs::write(
        root.join("lib.rs"),
        "pub fn alpha() {}\nconst X: u8 = 1;\nstruct S;\nfn beta() {}\n",
    )
    .unwrap();
    let out = reg
        .dispatch(
            "read_file",
            &serde_json::json!({"path": "outline://lib.rs"}),
        )
        .unwrap();
    assert!(out.contains("[outline://lib.rs]"), "{out}");
    assert!(out.contains("alpha") && out.contains("beta"), "{out}");
    assert!(out.contains("struct S") || out.contains("const X"), "{out}");
    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn github_uri_parse_is_wired_through_read_file_errors() {
    // Without network/auth we still validate parse + missing-number errors
    // via the virtual path; a clean list may succeed if gh is authed.
    let root = std::env::temp_dir().join(format!("angel_ghuri_{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir_all(&root).unwrap();
    let mut reg = ToolRegistry::new();
    register_file_tools(&mut reg, root.clone());
    let err = reg
        .dispatch(
            "read_file",
            &serde_json::json!({"path": "pr://not-a-number"}),
        )
        .unwrap_err();
    assert!(
        err.contains("invalid") || err.contains("not a") || err.contains("expected"),
        "{err}"
    );
    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn agent_uri_lists_and_discloses_handles() {
    use crate::agent::harness::{HandleKind, PutMeta, session_clear, session_put};

    let _env = crate::tests::env_lock();
    let _store = crate::tests::TestEnvGuard::set("ANGEL_HANDLE_STORE", "1");
    session_clear();

    let root = std::env::temp_dir().join(format!("angel_agenturi_{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir_all(&root).unwrap();
    let mut reg = ToolRegistry::new();
    register_file_tools(&mut reg, root.clone());

    let empty = reg
        .dispatch("read_file", &serde_json::json!({"path": "agent://"}))
        .unwrap();
    assert!(
        empty.contains("no live") || empty.contains("handle"),
        "{empty}"
    );

    let body = r#"{"findings":[{"path":"src/a.rs","ok":true}],"summary":"all good"}"#;
    let receipt = session_put(
        body,
        PutMeta {
            kind: HandleKind::Subcall,
            producer: "spawn",
            identity: Some("spawn|reviewer"),
            paths: &[],
            include_preview: true,
        },
    )
    .expect("put");
    let id = receipt.handle.as_str().to_string();

    let catalog = reg
        .dispatch("read_file", &serde_json::json!({"path": "agent://"}))
        .unwrap();
    assert!(catalog.contains(&format!("agent://{id}")), "{catalog}");
    assert!(catalog.contains("subcall"), "{catalog}");

    let full = reg
        .dispatch(
            "read_file",
            &serde_json::json!({"path": format!("agent://{id}")}),
        )
        .unwrap();
    assert!(full.contains("findings"), "{full}");

    let extracted = reg
        .dispatch(
            "read_file",
            &serde_json::json!({"path": format!("agent://{id}/findings.0.path")}),
        )
        .unwrap();
    assert!(extracted.contains("src/a.rs"), "{extracted}");

    session_clear();
    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn apply_patch_hashline_recovers_stale_tag_via_session_snapshot() {
    let _env = crate::tests::env_lock();
    let _anchors = crate::tests::TestEnvGuard::unset("ANGEL_HASHLINE_ANCHORS");
    let root = std::env::temp_dir().join(format!("angel_hl_rec_{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir_all(&root).unwrap();
    let mut reg = ToolRegistry::new();
    register_file_tools(&mut reg, root.clone());

    // Model reads the original file (records a session snapshot).
    let original = "alpha\nbeta\ngamma\n";
    std::fs::write(root.join("r.rs"), original).unwrap();
    let page = reg
        .dispatch("read_file", &serde_json::json!({"path": "r.rs"}))
        .unwrap();
    let tag = crate::agent::hashline::content_tag(original);
    assert!(page.contains(&format!("[r.rs#{tag}]")), "got: {page}");

    // External drift: insert a line above the anchored region.
    std::fs::write(root.join("r.rs"), "HEADER\nalpha\nbeta\ngamma\n").unwrap();

    // Patch still carries the pre-drift tag; recovery must remap SWAP 2 → line 3.
    let patch = format!("*** Begin Patch\n[r.rs#{tag}]\nSWAP 2:\n+BETA\n*** End Patch\n");
    let msg = reg
        .dispatch("apply_patch", &serde_json::json!({ "diff": patch }))
        .unwrap();
    assert!(msg.contains("new tag #"), "got: {msg}");
    assert!(
        msg.contains("recover") || msg.contains("remap") || msg.contains("snapshot"),
        "receipt should note recovery: {msg}"
    );
    assert_eq!(
        std::fs::read_to_string(root.join("r.rs")).unwrap(),
        "HEADER\nalpha\nBETA\ngamma\n"
    );

    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn str_replace_expect_tag_guards_stale_edit() {
    let root = std::env::temp_dir().join(format!("angel_et_{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir_all(&root).unwrap();
    let mut reg = ToolRegistry::new();
    register_file_tools(&mut reg, root.clone());
    std::fs::write(root.join("g.rs"), "let a = 1;\n").unwrap();
    let tag = crate::agent::hashline::content_tag("let a = 1;\n");

    // Matching tag → edit applies.
    reg.dispatch(
        "str_replace",
        &serde_json::json!({ "path": "g.rs", "old": "a = 1", "new": "a = 2", "expect_tag": tag }),
    )
    .unwrap();
    assert_eq!(
        std::fs::read_to_string(root.join("g.rs")).unwrap(),
        "let a = 2;\n"
    );

    // The file has since changed, so the OLD tag is stale → rejected, untouched.
    let err = reg
        .dispatch(
            "str_replace",
            &serde_json::json!({ "path": "g.rs", "old": "a = 2", "new": "a = 3", "expect_tag": tag }),
        )
        .unwrap_err();
    assert!(err.contains("stale edit"), "got: {err}");
    assert_eq!(
        std::fs::read_to_string(root.join("g.rs")).unwrap(),
        "let a = 2;\n"
    );

    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn mutation_receipts_chain_guarded_edits_without_reread() {
    fn receipt_tag(receipt: &str) -> &str {
        let tag = receipt
            .rsplit_once("new tag #")
            .map(|(_, tag)| tag)
            .unwrap_or_else(|| panic!("missing new content tag in receipt: {receipt}"));
        assert_eq!(tag.len(), 8, "unexpected content tag: {tag}");
        assert!(
            tag.bytes().all(|byte| byte.is_ascii_hexdigit()),
            "content tag is not hexadecimal: {tag}"
        );
        tag
    }

    let root = std::env::temp_dir().join(format!("angel_mutation_tag_{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir_all(&root).unwrap();
    let mut reg = ToolRegistry::new();
    register_file_tools(&mut reg, root.clone());

    let write_receipt = reg
        .dispatch(
            "write_file",
            &serde_json::json!({"path": "chain.txt", "content": "alpha beta\n"}),
        )
        .unwrap();
    let write_tag = receipt_tag(&write_receipt).to_string();
    assert_eq!(
        write_tag,
        crate::agent::hashline::content_tag("alpha beta\n")
    );

    // The returned tag is sufficient authority for the next guarded mutation;
    // no read_file call is needed between these tool calls.
    let replace_receipt = reg
        .dispatch(
            "str_replace",
            &serde_json::json!({
                "path": "chain.txt",
                "old": "alpha",
                "new": "gamma",
                "expect_tag": write_tag,
            }),
        )
        .unwrap();
    let replace_tag = receipt_tag(&replace_receipt).to_string();
    assert_eq!(
        replace_tag,
        crate::agent::hashline::content_tag("gamma beta\n")
    );

    let multi_receipt = reg
        .dispatch(
            "multi_edit",
            &serde_json::json!({
                "path": "chain.txt",
                "expect_tag": replace_tag,
                "edits": [
                    {"old": "gamma", "new": "omega"},
                    {"old": "beta", "new": "delta"},
                ],
            }),
        )
        .unwrap();
    let multi_tag = receipt_tag(&multi_receipt);
    assert_eq!(
        multi_tag,
        crate::agent::hashline::content_tag("omega delta\n")
    );
    assert_eq!(
        std::fs::read_to_string(root.join("chain.txt")).unwrap(),
        "omega delta\n"
    );

    // The first receipt is now stale and must not authorize a later edit.
    let err = reg
        .dispatch(
            "str_replace",
            &serde_json::json!({
                "path": "chain.txt",
                "old": "omega",
                "new": "stale",
                "expect_tag": write_tag,
            }),
        )
        .unwrap_err();
    assert!(err.contains("stale edit"), "got: {err}");
    assert_eq!(
        std::fs::read_to_string(root.join("chain.txt")).unwrap(),
        "omega delta\n"
    );

    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn multi_edit_applies_atomically() {
    let _env = crate::tests::env_lock();
    let _anchors = crate::tests::TestEnvGuard::set("ANGEL_HASHLINE_ANCHORS", "0");
    let root = std::env::temp_dir().join(format!("angel_me_{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir_all(&root).unwrap();
    let mut reg = ToolRegistry::new();
    register_file_tools(&mut reg, root.clone());
    reg.dispatch(
        "write_file",
        &serde_json::json!({"path": "m.rs", "content": "let a = 1;\nlet b = 2;\n"}),
    )
    .unwrap();

    let r = reg
        .dispatch(
            "multi_edit",
            &serde_json::json!({"path": "m.rs", "edits": [
                {"old": "a = 1", "new": "a = 10"},
                {"old": "b = 2", "new": "b = 20"}
            ]}),
        )
        .unwrap();
    assert!(r.contains("2 edits"), "got: {r}");
    let after = reg
        .dispatch("read_file", &serde_json::json!({"path": "m.rs"}))
        .unwrap();
    assert_eq!(after, "let a = 10;\nlet b = 20;\n");

    // Atomicity: the 2nd edit can't match, so NEITHER edit should land.
    let e = reg.dispatch(
        "multi_edit",
        &serde_json::json!({"path": "m.rs", "edits": [
            {"old": "a = 10", "new": "a = 99"},
            {"old": "NOPE_not_present", "new": "x"}
        ]}),
    );
    assert!(e.is_err(), "expected failure, got: {e:?}");
    let after2 = reg
        .dispatch("read_file", &serde_json::json!({"path": "m.rs"}))
        .unwrap();
    assert_eq!(
        after2, "let a = 10;\nlet b = 20;\n",
        "file must be unchanged after a failed multi_edit"
    );
    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn str_replace_rejects_ambiguous_match() {
    let root = std::env::temp_dir().join(format!("angel_ft2_{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir_all(&root).unwrap();
    let mut reg = ToolRegistry::new();
    register_file_tools(&mut reg, root.clone());
    reg.dispatch(
        "write_file",
        &serde_json::json!({"path": "d.txt", "content": "x x x"}),
    )
    .unwrap();
    let e = reg.dispatch(
        "str_replace",
        &serde_json::json!({"path": "d.txt", "old": "x", "new": "y"}),
    );
    assert!(e.is_err(), "ambiguous replace must fail, got: {e:?}");
    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn locate_replacement_exact_paths() {
    let c = "let a = 1;\nlet b = 2;\n";
    // Mid-line exact substring still works, no fuzzy flag.
    assert_eq!(locate_replacement(c, "a = 1"), Ok((4, 9, false)));
    // Ambiguous exact match fails loudly.
    assert!(locate_replacement("x x x", "x").is_err());
    // Absent text fails.
    assert!(locate_replacement(c, "nope").is_err());
}

#[test]
fn locate_replacement_ambiguous_names_line_numbers() {
    // Same parse site in validFrom and expiresOn — the Roll 07 poco failure mode.
    let c = "\
Poco::DateTime X509Certificate::validFrom() const\n\
{\n\
\treturn DateTimeParser::parse(\"%y%m%d%H%M%S\", dateTime, tzd);\n\
}\n\
Poco::DateTime X509Certificate::expiresOn() const\n\
{\n\
\treturn DateTimeParser::parse(\"%y%m%d%H%M%S\", dateTime, tzd);\n\
}\n";
    let err = locate_replacement(
        c,
        "return DateTimeParser::parse(\"%y%m%d%H%M%S\", dateTime, tzd);",
    )
    .expect_err("duplicate sites must stay ambiguous");
    assert!(err.contains("2 matches"), "count present: {err}");
    assert!(
        err.contains("L3") && err.contains("L7"),
        "line numbers of both sites: {err}"
    );
    assert!(
        err.contains("enclosing function") || err.contains("surrounding unique"),
        "disambiguation guidance: {err}"
    );
}

#[test]
fn locate_replacement_tolerates_interior_trailing_ws() {
    // Trailing spaces on a non-final line break the exact substring; the
    // whole-line fuzzy tier recovers it and reports fuzzy=true.
    let c = "fn a() {   \n    body();\n}\n";
    let old = "fn a() {\n    body();\n}";
    let (s, e, fuzzy) = locate_replacement(c, old).unwrap();
    assert!(fuzzy, "should need the fuzzy tier");
    assert_eq!(&c[s..e], "fn a() {   \n    body();\n}");
}

#[test]
fn locate_replacement_tolerates_crlf_and_typography() {
    // CRLF line endings on a multi-line block.
    let c = "let x = 1;\r\nlet y = 2;\r\n";
    let (s, e, fuzzy) = locate_replacement(c, "let x = 1;\nlet y = 2;").unwrap();
    assert!(fuzzy);
    assert_eq!(&c[s..e], "let x = 1;\r\nlet y = 2;");
    // Smart quotes fold to ASCII.
    let c = "  println!(“hi”);\n";
    let (s, e, fuzzy) = locate_replacement(c, "  println!(\"hi\");").unwrap();
    assert!(fuzzy);
    assert!(
        c[s..e].contains('\u{201C}'),
        "kept the original curly quotes"
    );
}

#[test]
fn locate_replacement_never_silently_reindents() {
    // File indents the interior line with 4 spaces; old uses 2. There is no
    // exact substring, and the fuzzy tiers must REFUSE (leading whitespace
    // differs) rather than re-indent the code.
    let c = "fn a() {   \n    body();\n}\n";
    let old = "fn a() {\n  body();\n}";
    assert!(locate_replacement(c, old).is_err());
}

#[test]
fn locate_replacement_fuzzy_requires_uniqueness() {
    // Two 2-line blocks identical modulo trailing whitespace; only a fuzzy
    // tier matches and it finds both — must fail as ambiguous, never guess.
    let c = "foo\t\nbar\nfoo \nbar\n";
    let r = locate_replacement(c, "foo\nbar");
    assert!(r.is_err(), "ambiguous fuzzy match must fail, got {r:?}");
    let err = r.unwrap_err();
    assert!(
        err.contains("whitespace-tolerant"),
        "fuzzy path labels the tier: {err}"
    );
    assert!(
        err.contains("L1") && err.contains("L3"),
        "fuzzy ambiguity also names lines: {err}"
    );
}

#[test]
fn str_replace_uses_fuzzy_fallback() {
    let _env = crate::tests::env_lock();
    let _anchors = crate::tests::TestEnvGuard::set("ANGEL_HASHLINE_ANCHORS", "0");
    let root = std::env::temp_dir().join(format!("angel_fz_{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir_all(&root).unwrap();
    let mut reg = ToolRegistry::new();
    register_file_tools(&mut reg, root.clone());
    // Interior trailing whitespace defeats an exact match.
    reg.dispatch(
        "write_file",
        &serde_json::json!({"path": "f.rs", "content": "fn a() {   \n    body();\n}\n"}),
    )
    .unwrap();
    let r = reg
        .dispatch(
            "str_replace",
            &serde_json::json!({
                "path": "f.rs",
                "old": "fn a() {\n    body();\n}",
                "new": "fn a() {\n    body2();\n}"
            }),
        )
        .unwrap();
    assert!(r.contains("whitespace-tolerant"), "got: {r}");
    let after = reg
        .dispatch("read_file", &serde_json::json!({"path": "f.rs"}))
        .unwrap();
    assert_eq!(after, "fn a() {\n    body2();\n}\n");
    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn parse_freeform_patch_handles_all_ops() {
    let patch = "*** Begin Patch\n\
*** Add File: a.txt\n\
+hello\n\
+world\n\
*** Delete File: gone.txt\n\
*** Update File: b.rs\n\
*** Move to: c.rs\n\
@@ fn f()\n\
-    old();\n\
+    new();\n\
*** End Patch\n";
    let ops = parse_freeform_patch(patch).unwrap();
    assert_eq!(ops.len(), 3);
    assert_eq!(
        ops[0],
        FileOp::Add {
            path: "a.txt".into(),
            content: "hello\nworld".into()
        }
    );
    assert_eq!(
        ops[1],
        FileOp::Delete {
            path: "gone.txt".into()
        }
    );
    match &ops[2] {
        FileOp::Update {
            path,
            move_to,
            hunks,
        } => {
            assert_eq!(path, "b.rs");
            assert_eq!(move_to.as_deref(), Some("c.rs"));
            assert_eq!(hunks.len(), 1);
            assert_eq!(hunks[0].old, "    old();");
            assert_eq!(hunks[0].new, "    new();");
        }
        _ => panic!("expected an update op"),
    }
}

#[test]
fn parse_freeform_patch_rejects_malformed() {
    let bad = "*** Begin Patch\n*** Update File: x\n@@\nbogus line\n*** End Patch\n";
    assert!(
        parse_freeform_patch(bad).is_err(),
        "bad hunk line must error"
    );
    assert!(
        parse_freeform_patch("no markers at all").is_err(),
        "needs begin marker"
    );
    let no_end = "*** Begin Patch\n*** Delete File: x\n";
    assert!(parse_freeform_patch(no_end).is_err(), "needs end marker");
}

#[test]
fn apply_patch_freeform_end_to_end() {
    let _env = crate::tests::env_lock();
    let _anchors = crate::tests::TestEnvGuard::set("ANGEL_HASHLINE_ANCHORS", "0");
    let root = std::env::temp_dir().join(format!("angel_ffpatch_{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir_all(&root).unwrap();
    let mut reg = ToolRegistry::new();
    register_file_tools(&mut reg, root.clone());
    reg.dispatch(
        "write_file",
        &serde_json::json!({"path": "b.rs", "content": "fn f() {\n    old();\n}\n"}),
    )
    .unwrap();
    let patch = "*** Begin Patch\n\
*** Add File: a.txt\n\
+hi\n\
*** Update File: b.rs\n\
@@ fn f()\n\
-    old();\n\
+    new();\n\
*** End Patch\n";
    let r = reg
        .dispatch("apply_patch", &serde_json::json!({ "diff": patch }))
        .unwrap();
    assert!(r.contains("1 added, 1 updated"), "got: {r}");
    let a = reg
        .dispatch("read_file", &serde_json::json!({"path": "a.txt"}))
        .unwrap();
    assert_eq!(a, "hi\n");
    let b = reg
        .dispatch("read_file", &serde_json::json!({"path": "b.rs"}))
        .unwrap();
    assert_eq!(b, "fn f() {\n    new();\n}\n");
    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn apply_patch_freeform_preflights_every_hunk_before_mutation() {
    let root = std::env::temp_dir().join(format!("angel_ffpatch_preflight_{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir_all(&root).unwrap();
    std::fs::write(root.join("first.txt"), "old first\n").unwrap();
    std::fs::write(root.join("second.txt"), "old second\n").unwrap();
    let mut reg = ToolRegistry::new();
    register_file_tools(&mut reg, root.clone());
    let patch = "*** Begin Patch\n\
*** Update File: first.txt\n\
@@\n\
-old first\n\
+new first\n\
*** Update File: second.txt\n\
@@\n\
-missing context\n\
+new second\n\
*** End Patch\n";
    let error = reg
        .dispatch("apply_patch", &serde_json::json!({ "diff": patch }))
        .expect_err("the second hunk must fail preflight");
    assert!(error.contains("second.txt hunk 1"), "got: {error}");
    assert_eq!(
        std::fs::read_to_string(root.join("first.txt")).unwrap(),
        "old first\n"
    );
    assert_eq!(
        std::fs::read_to_string(root.join("second.txt")).unwrap(),
        "old second\n"
    );
    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn apply_patch_freeform_rejects_duplicate_targets_and_move_overwrite() {
    let root = std::env::temp_dir().join(format!("angel_ffpatch_conflict_{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir_all(&root).unwrap();
    std::fs::write(root.join("source.txt"), "source\n").unwrap();
    std::fs::write(root.join("destination.txt"), "destination\n").unwrap();
    let mut reg = ToolRegistry::new();
    register_file_tools(&mut reg, root.clone());

    let duplicate = "*** Begin Patch\n\
*** Update File: source.txt\n\
@@\n\
-source\n\
+one\n\
*** Delete File: source.txt\n\
*** End Patch\n";
    let error = reg
        .dispatch("apply_patch", &serde_json::json!({ "diff": duplicate }))
        .expect_err("one path cannot have ambiguous operation ordering");
    assert!(error.contains("more than once"), "got: {error}");

    let overwrite = "*** Begin Patch\n\
*** Update File: source.txt\n\
*** Move to: destination.txt\n\
@@\n\
-source\n\
+moved\n\
*** End Patch\n";
    let error = reg
        .dispatch("apply_patch", &serde_json::json!({ "diff": overwrite }))
        .expect_err("a move must not silently overwrite its destination");
    assert!(error.contains("destination already exists"), "got: {error}");
    assert_eq!(
        std::fs::read_to_string(root.join("source.txt")).unwrap(),
        "source\n"
    );
    assert_eq!(
        std::fs::read_to_string(root.join("destination.txt")).unwrap(),
        "destination\n"
    );
    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn apply_patch_freeform_rolls_back_an_earlier_commit_when_later_add_fails() {
    let root = std::env::temp_dir().join(format!("angel_ffpatch_rollback_{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir_all(&root).unwrap();
    std::fs::write(root.join("first.txt"), "old first\n").unwrap();
    std::fs::write(root.join("occupied.txt"), "keep me\n").unwrap();
    let mut reg = ToolRegistry::new();
    register_file_tools(&mut reg, root.clone());
    let patch = "*** Begin Patch\n\
*** Update File: first.txt\n\
@@\n\
-old first\n\
+new first\n\
*** Add File: occupied.txt\n\
+replace me\n\
*** End Patch\n";
    let error = reg
        .dispatch("apply_patch", &serde_json::json!({ "diff": patch }))
        .expect_err("create-new failure must roll back the earlier update");
    assert!(
        error.contains("earlier patch operations rolled back"),
        "got: {error}"
    );
    assert_eq!(
        std::fs::read_to_string(root.join("first.txt")).unwrap(),
        "old first\n"
    );
    assert_eq!(
        std::fs::read_to_string(root.join("occupied.txt")).unwrap(),
        "keep me\n"
    );
    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn apply_patch_freeform_commits_add_delete_and_move_together() {
    let root = std::env::temp_dir().join(format!("angel_ffpatch_commit_{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir_all(&root).unwrap();
    std::fs::write(root.join("delete.txt"), "gone\n").unwrap();
    std::fs::write(root.join("source.txt"), "old\n").unwrap();
    let mut reg = ToolRegistry::new();
    register_file_tools(&mut reg, root.clone());
    let patch = "*** Begin Patch\n\
*** Add File: added.txt\n\
+added\n\
*** Delete File: delete.txt\n\
*** Update File: source.txt\n\
*** Move to: moved.txt\n\
@@\n\
-old\n\
+new\n\
*** End Patch\n";
    let result = reg
        .dispatch("apply_patch", &serde_json::json!({ "diff": patch }))
        .expect("the complete preflighted transaction should commit");
    assert!(
        result.contains("1 added, 1 updated, 1 deleted"),
        "got: {result}"
    );
    assert_eq!(
        std::fs::read_to_string(root.join("added.txt")).unwrap(),
        "added\n"
    );
    assert!(!root.join("delete.txt").exists());
    assert!(!root.join("source.txt").exists());
    assert_eq!(
        std::fs::read_to_string(root.join("moved.txt")).unwrap(),
        "new\n"
    );
    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn apply_patch_unified_preflight_prevents_partial_multi_file_change() {
    let root = std::env::temp_dir().join(format!(
        "angel_unified_patch_preflight_{}",
        std::process::id()
    ));
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir_all(&root).unwrap();
    std::fs::write(root.join("first.txt"), "old first\n").unwrap();
    std::fs::write(root.join("second.txt"), "old second\n").unwrap();
    let mut reg = ToolRegistry::new();
    register_file_tools(&mut reg, root.clone());
    let diff = "--- a/first.txt\n\
+++ b/first.txt\n\
@@ -1 +1 @@\n\
-old first\n\
+new first\n\
--- a/second.txt\n\
+++ b/second.txt\n\
@@ -1 +1 @@\n\
-missing second\n\
+new second\n";
    let error = reg
        .dispatch("apply_patch", &serde_json::json!({ "diff": diff }))
        .expect_err("both unified-diff backends must preflight every file");
    assert!(error.contains("preflight failed"), "got: {error}");
    assert_eq!(
        std::fs::read_to_string(root.join("first.txt")).unwrap(),
        "old first\n"
    );
    assert_eq!(
        std::fs::read_to_string(root.join("second.txt")).unwrap(),
        "old second\n"
    );
    let _ = std::fs::remove_dir_all(&root);
}

/// The model quotes the tag in every shape read_file printed it — `tag`,
/// `#tag`, `path#tag`, `[path#tag]` — and all of them must mean the same guard.
/// Measured 2026-09-07: glm-5.3-flash passed the whole `path#tag` and lost two
/// hops to "stale edit" with identical tags on both sides.
#[test]
fn expect_tag_accepts_every_header_form() {
    let content = "fn main() {}\n";
    let tag = crate::agent::hashline::content_tag(content);
    for form in [
        tag.clone(),
        format!("#{tag}"),
        format!("src/main.rs#{tag}"),
        format!("[src/main.rs#{tag}]"),
    ] {
        crate::agent::tools::file::guard_expected_tag("src/main.rs", content, Some(&form))
            .unwrap_or_else(|e| panic!("{form}: {e}"));
    }
    assert!(
        crate::agent::tools::file::guard_expected_tag(
            "src/main.rs",
            content,
            Some("src/main.rs#deadbeef")
        )
        .is_err()
    );
    assert!(
        crate::agent::tools::file::guard_expected_tag("src/main.rs", content, Some("")).is_ok(),
        "empty tag = no guard"
    );
}
