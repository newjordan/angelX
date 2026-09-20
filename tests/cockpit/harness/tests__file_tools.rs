//! Core tool dispatch and file/notes/todo coverage.
//!
//! Extracted from `harness/tests.rs` without behavior change so the monolith can
//! shrink while preserving the full harness test inventory.

use super::*;

// --- file tools / notes / todo suite ----------------------------------------

#[test]
fn reverse_tool_works() {
    let out = ReverseTool
        .call(&serde_json::json!({ "text": "abc" }))
        .unwrap();
    assert_eq!(out, "cba");
}

#[test]
fn shell_accepts_recovered_cmd_alias_without_an_extra_tool_hop() {
    let root = scratch("shell_cmd_alias");
    let tool = ShellTool::in_dir(root.clone());
    let result = tool
        .call(&serde_json::json!({"cmd":"printf recovered-cmd"}))
        .expect("legacy cmd alias should execute through the canonical shell tool");
    assert!(result.contains("recovered-cmd"), "{result}");
    // `script` is the code_mode-vocabulary alias for the same command slot:
    // a nested `shell({script})` call must run, not error on `command`.
    let result = tool
        .call(&serde_json::json!({"script":"printf recovered-script"}))
        .expect("script alias should execute through the canonical shell tool");
    assert!(result.contains("recovered-script"), "{result}");
    // `command` stays authoritative when several aliases appear.
    let result = tool
        .call(&serde_json::json!({
            "command":"printf authoritative",
            "script":"printf shadowed",
            "cmd":"printf shadowed-too",
        }))
        .expect("command wins over aliases");
    assert!(result.contains("authoritative"), "{result}");
    assert!(!result.contains("shadowed"), "{result}");
    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn dispatch_unknown_tool_errors() {
    let _guard = crate::tests::env_lock();
    let reg = ToolRegistry::with_defaults();
    assert!(reg.dispatch("nope", &serde_json::json!({})).is_err());
}

#[test]
fn unrecoverable_tool_arguments_fail_closed_without_dispatch() {
    let mut reg = ToolRegistry::new();
    reg.register(Box::new(ReverseTool));
    let raw = "{ this is not json at all !!!";
    let args = crate::agent::club::parsed_tool_args("bad-call", raw);
    let error = reg
        .dispatch("reverse", &args)
        .expect_err("an unrecoverable blob must never become an empty object");
    assert!(error.contains("unrecoverable tool arguments"), "{error}");
    assert!(
        error.contains("reissue `reverse` with valid JSON"),
        "{error}"
    );
    let record = crate::agent::club::tool_arg_repair_records()
        .into_iter()
        .find(|record| record.call_id == "bad-call")
        .expect("repair diagnostic");
    assert_eq!(record.kind, "Unrecoverable");
    assert!(record.digest.starts_with("fnv64:"));
    assert_eq!(record.length, raw.len());
}

#[test]
fn file_tools_write_read_replace_list_roundtrip() {
    // Exact-byte read assertions need plain pages; hashline anchors are on by
    // default and prefix `[path#tag]` + line numbers.
    let _env = crate::tests::env_lock();
    let _anchors = crate::tests::TestEnvGuard::set("ANGEL_HASHLINE_ANCHORS", "0");
    let root = std::env::temp_dir().join(format!("angel_ft_{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir_all(&root).unwrap();
    let mut reg = ToolRegistry::new();
    register_file_tools(&mut reg, root.clone());

    let w = reg
        .dispatch(
            "write_file",
            &serde_json::json!({"path": "a/b.txt", "content": "hello world"}),
        )
        .unwrap();
    assert!(w.contains("11 bytes"), "got: {w}");

    let r = reg
        .dispatch("read_file", &serde_json::json!({"path": "a/b.txt"}))
        .unwrap();
    assert_eq!(r, "hello world");

    reg.dispatch(
        "str_replace",
        &serde_json::json!({"path": "a/b.txt", "old": "world", "new": "angel"}),
    )
    .unwrap();
    let r2 = reg
        .dispatch("read_file", &serde_json::json!({"path": "a/b.txt"}))
        .unwrap();
    assert_eq!(r2, "hello angel");

    let l = reg
        .dispatch("list_dir", &serde_json::json!({"path": "a"}))
        .unwrap();
    assert!(l.contains("b.txt"), "got: {l}");

    // Confinement: no climbing out and no absolute paths outside this workspace.
    assert!(
        reg.dispatch("read_file", &serde_json::json!({"path": "../escape"}))
            .is_err()
    );
    assert!(
        reg.dispatch("read_file", &serde_json::json!({"path": "/etc/passwd"}))
            .is_err()
    );
    assert!(
        reg.dispatch(
            "write_file",
            &serde_json::json!({"path": "../../x", "content": "x"})
        )
        .is_err()
    );

    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn read_file_pages_are_contiguous_bounded_and_schema_documented() {
    // Page sizing and exact next-offset teaching are independent of hashline
    // anchors; pin them off so this contract stays byte-stable.
    let _env = crate::tests::env_lock();
    let _anchors = crate::tests::TestEnvGuard::set("ANGEL_HASHLINE_ANCHORS", "0");
    let root = scratch("read_pages");
    let source = (1..=5000)
        .map(|line| format!("line-{line:04}: bounded paging payload\n"))
        .collect::<String>();
    std::fs::write(root.join("large.txt"), &source).unwrap();
    let tool = ReadFileTool { root: root.clone() };

    let def = tool.def();
    assert_eq!(def.params["required"], serde_json::json!(["path"]));
    assert_eq!(def.params["properties"]["offset"]["type"], "integer");
    assert_eq!(def.params["properties"]["offset"]["minimum"], 1);
    assert_eq!(def.params["properties"]["limit"]["type"], "integer");
    assert_eq!(def.params["properties"]["limit"]["minimum"], 1);
    assert_eq!(def.params["properties"]["limit"]["maximum"], 400);

    let first = tool.call(&serde_json::json!({"path":"large.txt"})).unwrap();
    assert!(
        first.contains("line-0001: bounded paging payload"),
        "{first}"
    );
    assert!(
        first.contains("line-0200: bounded paging payload"),
        "{first}"
    );
    assert!(
        !first.contains("line-0201: bounded paging payload"),
        "{first}"
    );
    assert!(
        first.ends_with("offset=201]"),
        "default page must teach the next contiguous offset: {first}"
    );
    assert!(
        first.len() * 10 < source.len(),
        "default page must be materially smaller than the full source: {} vs {} bytes",
        first.len(),
        source.len()
    );

    let ranged = tool
        .call(&serde_json::json!({"path":"large.txt","offset":1201,"limit":24}))
        .unwrap();
    assert!(
        ranged.contains("line-1201: bounded paging payload"),
        "{ranged}"
    );
    assert!(
        ranged.contains("line-1224: bounded paging payload"),
        "{ranged}"
    );
    assert!(
        !ranged.contains("line-1200: bounded paging payload"),
        "{ranged}"
    );
    assert!(
        !ranged.contains("line-1225: bounded paging payload"),
        "{ranged}"
    );
    assert!(ranged.ends_with("offset=1225]"), "{ranged}");
    assert_eq!(
        cap_tool_output(&ranged, None),
        ranged,
        "a page must enter history unchanged rather than suffer generic head/tail elision"
    );
    let page_tokens = estimate_tokens(&[ChatMsg::tool("page", ranged.clone())]);
    let full_tokens = source.len() / 4;
    assert!(
        page_tokens * 20 < full_tokens,
        "the range page should remove the recurring context payload: {page_tokens} vs {full_tokens} tokens"
    );

    let next = tool
        .call(&serde_json::json!({"path":"large.txt","offset":1225,"limit":24}))
        .unwrap();
    assert!(next.contains("line-1225: bounded paging payload"), "{next}");
    assert!(
        !next.contains("line-1224: bounded paging payload"),
        "{next}"
    );

    for args in [
        serde_json::json!({"path":"large.txt","offset":0}),
        serde_json::json!({"path":"large.txt","offset":"2"}),
        serde_json::json!({"path":"large.txt","limit":0}),
        serde_json::json!({"path":"large.txt","limit":401}),
    ] {
        assert!(
            tool.call(&args).is_err(),
            "invalid page args must fail: {args}"
        );
    }
    assert_eq!(
        tool.call(&serde_json::json!({"path":"large.txt","offset":5001}))
            .unwrap(),
        "[end of file before line 5001]"
    );

    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn read_file_pages_preserve_crlf_unicode_and_final_line() {
    let _env = crate::tests::env_lock();
    let _anchors = crate::tests::TestEnvGuard::set("ANGEL_HASHLINE_ANCHORS", "0");
    let root = scratch("read_page_text");
    std::fs::write(root.join("text.txt"), "first\r\nβeta\r\nlast").unwrap();
    let tool = ReadFileTool { root: root.clone() };

    assert_eq!(
        tool.call(&serde_json::json!({"path":"text.txt","offset":2,"limit":1}))
            .unwrap(),
        "βeta\r\n…[more content; re-call read_file with offset=3]"
    );
    assert_eq!(
        tool.call(&serde_json::json!({"path":"text.txt","offset":3,"limit":1}))
            .unwrap(),
        "last"
    );
    assert_eq!(
        tool.call(&serde_json::json!({"path":"text.txt","offset":4}))
            .unwrap(),
        "[end of file before line 4]"
    );

    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn read_file_pages_bound_deep_scans_and_giant_lines() {
    let _env = crate::tests::env_lock();
    let _anchors = crate::tests::TestEnvGuard::set("ANGEL_HASHLINE_ANCHORS", "0");
    let root = scratch("read_page_bounds");
    let tool = ReadFileTool { root: root.clone() };

    std::fs::write(root.join("deep.txt"), "skip\n".repeat(500_000)).unwrap();
    let deep_error = tool
        .call(&serde_json::json!({"path":"deep.txt","offset":500_000,"limit":1}))
        .unwrap_err();
    assert!(
        deep_error.contains("bounded") && deep_error.contains("scan"),
        "deep line offsets must not turn into an unbounded whole-file read: {deep_error}"
    );

    // A giant single line self-corrects: largest prefix that fits, a truncation
    // marker naming the skipped bytes, and an offset that advances past the
    // unpageable line — success instead of a retried error.
    std::fs::write(root.join("giant.txt"), "x".repeat(49 * 1024)).unwrap();
    let giant = tool
        .call(&serde_json::json!({"path":"giant.txt","limit":1}))
        .unwrap();
    assert!(
        giant.ends_with("[truncated: 29696 more bytes; next offset 2]"),
        "giant line must return the bounded prefix plus a continuation marker: {giant}"
    );
    assert!(
        giant.len() < 21 * 1024,
        "giant-line page stays byte-bounded: {}",
        giant.len()
    );
    assert_eq!(
        tool.call(&serde_json::json!({"path":"giant.txt","offset":2,"limit":1}))
            .unwrap(),
        "[end of file before line 2]"
    );

    // Byte-cap truncation across many lines: the partial line is resumed from
    // its start on the next page, so no content is lost.
    let fat_source = (1..=200)
        .map(|line| format!("l{:04}:{}\n", line, "x".repeat(193)))
        .collect::<String>();
    assert_eq!(fat_source.len(), 40_000);
    std::fs::write(root.join("fat.txt"), &fat_source).unwrap();
    let fat = tool.call(&serde_json::json!({"path":"fat.txt"})).unwrap();
    assert!(
        fat.ends_with("[truncated: 19520 more bytes; next offset 103]"),
        "byte-capped page names the remaining bytes and resume offset: {fat}"
    );
    let resumed = tool
        .call(&serde_json::json!({"path":"fat.txt","offset":103}))
        .unwrap();
    assert!(resumed.contains("l0103:"), "{resumed}");
    assert!(resumed.contains("l0200:"), "{resumed}");
    assert!(!resumed.contains("[truncated"), "{resumed}");

    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn notes_tool_persists_across_instances() {
    let root = std::env::temp_dir().join(format!("angel_notes_{}", std::process::id()));
    let alpha = root.join("alpha");
    let beta = root.join("beta");
    std::fs::create_dir_all(&alpha).unwrap();
    std::fs::create_dir_all(&beta).unwrap();
    let p = root.join("notes.json");
    let _ = std::fs::remove_file(&p);

    let mut reg = ToolRegistry::new();
    reg.register(Box::new(NotesTool::at(p.clone(), &alpha)));
    assert_eq!(
        reg.dispatch("notes", &serde_json::json!({})).unwrap(),
        "(no notes)"
    );
    reg.dispatch(
        "notes",
        &serde_json::json!({"action": "add", "text": "alpha finding"}),
    )
    .unwrap();
    reg.dispatch(
        "notes",
        &serde_json::json!({"action": "add", "text": "beta decision"}),
    )
    .unwrap();
    let list = reg
        .dispatch("notes", &serde_json::json!({"action": "list"}))
        .unwrap();
    assert!(
        list.contains("- alpha finding") && list.contains("- beta decision"),
        "got: {list}"
    );

    // Cross-session: a fresh tool instance on the same file sees prior notes.
    let mut reg2 = ToolRegistry::new();
    reg2.register(Box::new(NotesTool::at(p.clone(), &alpha)));
    let seen = reg2
        .dispatch("notes", &serde_json::json!({"action": "list"}))
        .unwrap();
    assert!(
        seen.contains("- alpha finding"),
        "memory must survive restart: {seen}"
    );

    let mut foreign = ToolRegistry::new();
    foreign.register(Box::new(NotesTool::at(p.clone(), &beta)));
    assert_eq!(
        foreign
            .dispatch("notes", &serde_json::json!({"action":"list"}))
            .unwrap(),
        "(no notes)",
        "alpha notes crossed into beta"
    );
    assert!(
        foreign
            .dispatch(
                "notes",
                &serde_json::json!({"action":"add", "text":"beta overwrite"}),
            )
            .is_err()
    );

    reg2.dispatch("notes", &serde_json::json!({"action": "clear"}))
        .unwrap();
    assert_eq!(
        reg2.dispatch("notes", &serde_json::json!({"action": "list"}))
            .unwrap(),
        "(no notes)"
    );
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn todo_tool_add_complete_set_persist() {
    let mut reg = ToolRegistry::new();
    reg.register(Box::new(TodoTool::new()));
    let state = |result: &str| {
        serde_json::from_str::<Value>(
            result
                .lines()
                .last()
                .and_then(|line| line.strip_prefix(crate::agent::tools::plan::TODO_STATE_PREFIX))
                .expect("canonical todo state"),
        )
        .expect("valid todo JSON")
    };

    let a = reg
        .dispatch(
            "todo",
            &serde_json::json!({"action": "add", "text": "write parser"}),
        )
        .unwrap();
    assert!(a.contains("#1") && a.contains("write parser"), "got: {a}");
    reg.dispatch(
        "todo",
        &serde_json::json!({"action": "add", "text": "add tests"}),
    )
    .unwrap();

    let done = reg
        .dispatch("todo", &serde_json::json!({"action": "complete", "id": 1}))
        .unwrap();
    let done_state = state(&done);
    assert_eq!(done_state["items"][0]["id"], 1);
    assert_eq!(done_state["items"][0]["text"], "write parser");
    assert_eq!(done_state["items"][0]["done"], true);

    // State persists across dispatch calls (same registry instance).
    let list = reg
        .dispatch("todo", &serde_json::json!({"action": "list"}))
        .unwrap();
    let list_state = state(&list);
    assert_eq!(list_state["items"][0]["done"], true);
    assert_eq!(list_state["items"][1]["id"], 2);
    assert_eq!(list_state["items"][1]["text"], "add tests");
    assert_eq!(list_state["items"][1]["done"], false);

    // set replaces the whole list.
    reg.dispatch(
        "todo",
        &serde_json::json!({"action": "set", "items": ["only one"]}),
    )
    .unwrap();
    let list2 = reg
        .dispatch("todo", &serde_json::json!({"action": "list"}))
        .unwrap();
    assert!(
        list2.contains("only one") && !list2.contains("write parser"),
        "got: {list2}"
    );

    // completing a missing id errors.
    assert!(
        reg.dispatch("todo", &serde_json::json!({"action": "complete", "id": 99}))
            .is_err()
    );
}

// --- residual binary read coverage (folded from parent) -------------------

#[test]
fn read_file_detects_binary() {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let tool = ReadFileTool { root };
    let out = tool
        .call(&serde_json::json!({ "path": "assets/agents/apollo-neutral.png" }))
        .expect("read");
    assert!(
        out.contains("[binary file"),
        "expected a binary notice, got {} chars",
        out.len()
    );
    let replacement = out.chars().filter(|&c| c == '\u{FFFD}').count();
    assert!(
        replacement < 4,
        "binary notice must not dump replacement chars (got {replacement})"
    );
}

/// Benchmark: detecting a binary short-circuits the whole-file lossy decode,
/// so even a large asset returns fast.
#[test]
fn read_file_binary_is_fast() {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let tool = ReadFileTool { root };
    let args = serde_json::json!({ "path": "assets/agents/apollo-neutral.png" });
    let t0 = std::time::Instant::now();
    for _ in 0..20 {
        let _ = tool.call(&args).expect("read");
    }
    let per = t0.elapsed() / 20;
    assert!(
        per < Duration::from_millis(25),
        "binary read should be fast, took {per:?}/call"
    );
}

#[cfg(target_os = "linux")]
#[test]
#[ignore = "explicit repeated I/O/timing measurement; not a wall-clock unit gate"]
fn read_file_hashline_large_page_measurement() {
    let _env = crate::tests::env_lock();
    let root = scratch("hashline_large_measurement");
    let bytes = b"bounded\n".repeat(4 * 1024 * 1024);
    std::fs::write(root.join("large.txt"), &bytes).unwrap();
    let tool = ReadFileTool { root: root.clone() };
    let rchar = || -> u64 {
        std::fs::read_to_string("/proc/self/io")
            .unwrap()
            .lines()
            .find_map(|line| line.strip_prefix("rchar: "))
            .unwrap()
            .parse()
            .unwrap()
    };
    for sample in 1..=5 {
        for anchors in ["1", "0"] {
            let _anchors = crate::tests::TestEnvGuard::set("ANGEL_HASHLINE_ANCHORS", anchors);
            let before = rchar();
            let started = std::time::Instant::now();
            let page = tool
                .call(&serde_json::json!({"path":"large.txt","offset":1,"limit":1}))
                .unwrap();
            let elapsed = started.elapsed();
            let read_bytes = rchar().saturating_sub(before);
            assert_eq!(
                page,
                "bounded\n…[more content; re-call read_file with offset=2]"
            );
            eprintln!(
                "HASHLINE_PAGE_IO sample={sample} anchors={anchors} file_bytes={} returned_bytes={} rchar_delta={read_bytes} elapsed_us={:.3}",
                bytes.len(),
                page.len(),
                elapsed.as_secs_f64() * 1_000_000.0
            );
        }
    }
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn read_file_hashline_anchors_keep_huge_sparse_text_paged() {
    let _env = crate::tests::env_lock();
    let _anchors = crate::tests::TestEnvGuard::set("ANGEL_HASHLINE_ANCHORS", "1");
    let root = scratch("hashline_sparse_page");
    // The first probe is text, so the binary shortcut cannot mask the optional
    // whole-file enrichment bug. No 4 GiB allocation or read is needed.
    let path = root.join("huge.txt");
    std::fs::write(&path, "bounded\n".repeat(2048)).unwrap();
    std::fs::OpenOptions::new()
        .write(true)
        .open(&path)
        .unwrap()
        .set_len(4 * 1024 * 1024 * 1024)
        .unwrap();
    let tool = ReadFileTool { root: root.clone() };
    let page = tool
        .call(&serde_json::json!({"path":"huge.txt","limit":1}))
        .unwrap();
    assert_eq!(
        page,
        "bounded\n…[more content; re-call read_file with offset=2]"
    );
    std::fs::remove_dir_all(root).unwrap();
}
