use super::*;

fn apply(content: &str, patch_body: &str) -> Result<String, String> {
    let tag = content_tag(content);
    let patch = format!("*** Begin Patch\n[f.rs#{tag}]\n{patch_body}\n*** End Patch\n");
    let sections = parse(&patch)?;
    match plan_section(content, &sections[0])? {
        SectionPlan::Update { content, .. } => Ok(content),
        other => Err(format!("expected Update, got {other:?}")),
    }
}

fn plan(content: &str, patch_body: &str) -> Result<SectionPlan, String> {
    let tag = content_tag(content);
    let patch = format!("*** Begin Patch\n[f.rs#{tag}]\n{patch_body}\n*** End Patch\n");
    let sections = parse(&patch)?;
    plan_section(content, &sections[0])
}

#[test]
fn content_tag_is_8_hex_and_newline_insensitive() {
    let t = content_tag("a\nb\n");
    assert_eq!(t.len(), 8);
    assert!(t.bytes().all(|b| b.is_ascii_hexdigit()));
    assert_eq!(content_tag("a\r\nb\r\n"), content_tag("a\nb\n"));
}

#[test]
fn swap_single_line() {
    let out = apply("one\ntwo\nthree\n", "SWAP 2:\n+TWO").unwrap();
    assert_eq!(out, "one\nTWO\nthree\n");
}

#[test]
fn swap_range_with_multiple_body_rows() {
    let out = apply("a\nb\nc\nd\n", "SWAP 2.=3:\n+B\n+B2\n+B3").unwrap();
    assert_eq!(out, "a\nB\nB2\nB3\nd\n");
}

#[test]
fn delete_range() {
    let out = apply("a\nb\nc\nd\n", "DEL 2.=3").unwrap();
    assert_eq!(out, "a\nd\n");
}

#[test]
fn insert_pre_post_head_tail() {
    assert_eq!(apply("a\nb\n", "INS.PRE 1:\n+x").unwrap(), "x\na\nb\n");
    assert_eq!(apply("a\nb\n", "INS.POST 1:\n+x").unwrap(), "a\nx\nb\n");
    assert_eq!(apply("a\nb\n", "INS.HEAD:\n+top").unwrap(), "top\na\nb\n");
    assert_eq!(apply("a\nb\n", "INS.TAIL:\n+bot").unwrap(), "a\nb\nbot\n");
}

#[test]
fn blank_body_row_via_bare_plus() {
    let out = apply("a\nb\n", "INS.POST 1:\n+\n+c").unwrap();
    assert_eq!(out, "a\n\nc\nb\n");
}

#[test]
fn multiple_ops_use_original_numbering() {
    // Insert at 1 and swap 3 refer to ORIGINAL lines; order-independent.
    let out = apply("a\nb\nc\n", "INS.PRE 1:\n+HEAD\nSWAP 3:\n+C").unwrap();
    assert_eq!(out, "HEAD\na\nb\nC\n");
}

#[test]
fn stale_tag_is_rejected() {
    let path = format!("stale_reject_{}.rs", std::process::id());
    invalidate_snapshot(&path);
    let patch = format!("*** Begin Patch\n[{path}#deadbeef]\nSWAP 1:\n+x\n*** End Patch\n");
    let sections = parse(&patch).unwrap();
    let err = plan_section("a\nb\n", &sections[0]).unwrap_err();
    assert!(err.contains("stale patch"), "got: {err}");
    assert!(
        err.contains("No session snapshot") || err.contains("re-read"),
        "got: {err}"
    );
    invalidate_snapshot(&path);
}

#[test]
fn stale_tag_recovers_when_snapshot_and_anchors_map() {
    // Unique path so parallel unit tests do not share the process store.
    let path = format!("recover_ok_{}.rs", std::process::id());
    invalidate_snapshot(&path);
    // Model read "a\nb\nc\n". External agent inserted a header line.
    let snapshot = "a\nb\nc\n";
    let live = "HEAD\na\nb\nc\n";
    let tag = record_snapshot(&path, snapshot);
    let patch = format!("*** Begin Patch\n[{path}#{tag}]\nSWAP 2:\n+B\n*** End Patch\n");
    let sections = parse(&patch).unwrap();
    let outcome = plan_section_detailed(live, &sections[0]).unwrap();
    assert!(
        outcome.recovery_note.is_some(),
        "expected recovery note, got {:?}",
        outcome.recovery_note
    );
    match outcome.plan {
        SectionPlan::Update { content, .. } => {
            assert_eq!(content, "HEAD\na\nB\nc\n");
        }
        other => panic!("expected Update, got {other:?}"),
    }
    invalidate_snapshot(&path);
}

#[test]
fn stale_tag_recovery_fails_when_anchor_line_changed() {
    let path = format!("recover_fail_{}.rs", std::process::id());
    invalidate_snapshot(&path);
    let snapshot = "a\nb\nc\n";
    let live = "a\nCHANGED\nc\n";
    let tag = record_snapshot(&path, snapshot);
    let patch = format!("*** Begin Patch\n[{path}#{tag}]\nSWAP 2:\n+B\n*** End Patch\n");
    let sections = parse(&patch).unwrap();
    let err = plan_section(live, &sections[0]).unwrap_err();
    assert!(
        err.contains("stale patch") || err.contains("Recovery failed"),
        "got: {err}"
    );
    assert!(
        err.contains("Recovery failed") || err.contains("anchor"),
        "got: {err}"
    );
    invalidate_snapshot(&path);
}

#[test]
fn out_of_bounds_line_is_rejected() {
    let err = apply("a\nb\n", "SWAP 5:\n+x").unwrap_err();
    assert!(err.contains("out of bounds"), "got: {err}");
}

#[test]
fn overlapping_edits_are_rejected() {
    let err = apply("a\nb\nc\n", "SWAP 1.=2:\n+x\nDEL 2").unwrap_err();
    assert!(err.contains("overlapping"), "got: {err}");
}

#[test]
fn preserves_crlf_and_no_trailing_newline() {
    let out = apply("a\r\nb\r\nc", "SWAP 2:\n+B").unwrap();
    assert_eq!(out, "a\r\nB\r\nc");
}

#[test]
fn looks_like_hashline_discriminates() {
    assert!(looks_like_hashline(
        "*** Begin Patch\n[f.rs#a1b2c3d4]\nDEL 1\n*** End Patch"
    ));
    // A freeform Codex patch has the envelope but no [path#tag] header.
    assert!(!looks_like_hashline(
        "*** Begin Patch\n*** Update File: f.rs\n@@\n-a\n+b\n*** End Patch"
    ));
    assert!(!looks_like_hashline("just some prose"));
}

#[test]
fn rem_plans_remove() {
    let plan = plan("a\nb\n", "REM").unwrap();
    assert_eq!(
        plan,
        SectionPlan::Remove {
            path: "f.rs".into()
        }
    );
}

#[test]
fn mv_after_edit_plans_move() {
    let plan = plan("a\nb\n", "SWAP 1:\n+A\nMV g.rs").unwrap();
    match plan {
        SectionPlan::Move { from, to, content } => {
            assert_eq!(from, "f.rs");
            assert_eq!(to, "g.rs");
            assert_eq!(content, "A\nb\n");
        }
        other => panic!("expected Move, got {other:?}"),
    }
}

#[test]
fn pure_mv_keeps_content() {
    let plan = plan("hello\n", "MV nested/hello.rs").unwrap();
    match plan {
        SectionPlan::Move { from, to, content } => {
            assert_eq!(from, "f.rs");
            assert_eq!(to, "nested/hello.rs");
            assert_eq!(content, "hello\n");
        }
        other => panic!("expected Move, got {other:?}"),
    }
}

#[test]
fn rem_rejects_line_ops() {
    let tag = content_tag("a\n");
    let patch = format!("*** Begin Patch\n[f.rs#{tag}]\nSWAP 1:\n+x\nREM\n*** End Patch\n");
    let err = parse(&patch).unwrap_err();
    assert!(err.contains("REM"), "got: {err}");
}

#[test]
fn swap_blk_brace_function() {
    let src = "fn a() {\n    1\n}\nfn b() {\n    2\n}\n";
    let out = apply(src, "SWAP.BLK 1:\n+fn a() {\n+    9\n+}").unwrap();
    assert_eq!(out, "fn a() {\n    9\n}\nfn b() {\n    2\n}\n");
}

#[test]
fn del_blk_brace_function() {
    let src = "fn a() {\n    1\n}\nfn b() {\n    2\n}\n";
    let out = apply(src, "DEL.BLK 1").unwrap();
    assert_eq!(out, "fn b() {\n    2\n}\n");
}

#[test]
fn ins_blk_post_after_function() {
    let src = "fn a() {\n    1\n}\nfn b() {}\n";
    let out = apply(src, "INS.BLK.POST 1:\n+// after a").unwrap();
    assert_eq!(out, "fn a() {\n    1\n}\n// after a\nfn b() {}\n");
}

#[test]
fn swap_blk_markdown_section() {
    let src = "# Title\n\n## One\nbody\n## Two\nmore\n";
    let out = apply(src, "SWAP.BLK 3:\n+## One\n+new body\n").unwrap();
    assert_eq!(out, "# Title\n\n## One\nnew body\n## Two\nmore\n");
}

#[test]
fn swap_blk_indent_python() {
    let src = "def a():\n    x = 1\n    return x\n\ndef b():\n    pass\n";
    let out = apply(src, "SWAP.BLK 1:\n+def a():\n+    return 2\n").unwrap();
    assert_eq!(out, "def a():\n    return 2\n\ndef b():\n    pass\n");
}

#[test]
fn single_line_block_is_rejected() {
    let err = apply("let x = 1;\n", "SWAP.BLK 1:\n+let x = 2;\n").unwrap_err();
    assert!(
        err.contains("single-line") || err.contains("cannot resolve") || err.contains("plain"),
        "got: {err}"
    );
}
