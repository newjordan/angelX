//! Default tool kit, reward parsers, env-knob docs, and live daemon lifecycle.
//!
//! Extracted from `harness/tests.rs` without behavior change so the monolith can
//! shrink while preserving the full harness test inventory.

use super::*;

// --- kit / registry suite ---

#[test]
fn default_kit_registers_expected_tools() {
    let _guard = crate::tests::env_lock();
    // Wiring guard: the new tools must actually be in the default kit.
    let defs = ToolRegistry::with_defaults().defs();
    let names: Vec<&str> = defs.iter().map(|d| d.name.as_str()).collect();
    for expected in [
        "shell",
        "cargo",
        "run_tests",
        "lint",
        "check",
        "fmt",
        "todo",
        "notes",
        "read_file",
        "write_file",
        "str_replace",
        "multi_edit",
        "apply_patch",
        "outline",
        "git_diff",
        "git_status",
        "git_log",
        "tool_repair",
        "grep",
        "find_files",
        "defs",
        "list_dir",
        "http_request",
        "proc_run",
        "proc_status",
        "proc_stop",
        "gpu_stat",
        "fleet_status",
        "vast_instances",
        "llm_probe",
        "llm_bench",
    ] {
        assert!(
            names.contains(&expected),
            "missing tool {expected:?}; have {names:?}"
        );
    }
}

#[test]
fn parse_lint_counts_and_rewards() {
    let warns = "warning: unused variable: `x`\n --> src/m.rs:1:5\n\
            warning: unused import: `Foo`\n\
            warning: `cockpit` (bin \"angel\") generated 2 warnings";
    let o = parse_lint(warns);
    assert_eq!((o.warnings, o.errors), (2, 0));
    assert!(!o.clean());
    assert!(
        (o.reward() - 1.0 / 3.0).abs() < 1e-6,
        "reward {}",
        o.reward()
    );

    let errs = "error[E0425]: cannot find value `foo`\n\
            error: aborting due to 1 previous error\n\
            error: could not compile `cockpit`";
    let e = parse_lint(errs);
    assert_eq!(e.errors, 1, "summary lines must not be counted");
    assert_eq!(e.reward(), 0.0);

    let clean = parse_lint("    Checking cockpit v0.1.0\n    Finished in 0.3s");
    assert!(clean.clean());
    assert_eq!(clean.reward(), 1.0);
}

#[test]
fn parse_test_result_sums_and_rewards() {
    let mixed = "running 13 tests\n\
            test result: ok. 12 passed; 0 failed; 1 ignored; 0 measured; 0 filtered out; finished in 0.10s\n\
            test result: FAILED. 2 passed; 3 failed; 0 ignored; 0 measured; 0 filtered out;";
    let o = parse_test_result(mixed);
    assert_eq!((o.passed, o.failed, o.ignored), (14, 3, 1));
    assert!(!o.all_passed());
    assert!(
        (o.reward() - 14.0 / 17.0).abs() < 1e-6,
        "reward {}",
        o.reward()
    );

    let clean = "test result: ok. 5 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out;";
    let c = parse_test_result(clean);
    assert!(c.all_passed());
    assert_eq!(c.reward(), 1.0);

    // Build error → nothing ran → zero reward.
    let none = parse_test_result("error[E0432]: unresolved import `foo`");
    assert_eq!(none.ran(), 0);
    assert_eq!(none.reward(), 0.0);
}

#[test]
#[ignore = "spawns a real python http.server; run with --ignored"]
fn live_daemon_lifecycle_proc_run_http_request_proc_stop() {
    let _guard = crate::tests::env_lock();
    // The serving loop end-to-end, through registry dispatch exactly as the
    // model drives it: launch a daemon detached, talk to it over HTTP,
    // stop the whole tree.
    let reg = ToolRegistry::with_defaults();
    let port = 18471;
    let started = reg
        .dispatch(
            "proc_run",
            &serde_json::json!({
                "command": format!("python3 -m http.server {port} --bind 127.0.0.1"),
                "name": "mock-daemon",
            }),
        )
        .expect("proc_run");
    let id: u64 = started
        .split('[')
        .nth(1)
        .and_then(|s| s.split(']').next())
        .and_then(|s| s.parse().ok())
        .expect("handle id");
    let resp = (0..50).find_map(|_| {
        std::thread::sleep(Duration::from_millis(100));
        reg.dispatch(
            "http_request",
            &serde_json::json!({ "url": format!("http://127.0.0.1:{port}/") }),
        )
        .ok()
        .filter(|r| r.starts_with("[200"))
    });
    let status = reg
        .dispatch("proc_status", &serde_json::json!({ "id": id }))
        .expect("proc_status");
    let stopped = reg.dispatch("proc_stop", &serde_json::json!({ "id": id }));
    let resp = resp.expect("daemon never answered on its port");
    println!("http_request → {}", resp.lines().next().unwrap_or(""));
    assert!(status.contains("running"), "status: {status}");
    let stopped = stopped.expect("proc_stop");
    assert!(
        stopped.contains("stopped") || stopped.contains("killed"),
        "stop: {stopped}"
    );
}

/// Every `ANGEL_*` knob the source reads must appear in docs/ENV.md, so the
/// operator reference can't silently drift from the code.
#[test]
fn env_knob_doc_completeness() {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let doc = std::fs::read_to_string(root.join("docs/ENV.md")).expect("docs/ENV.md exists");
    // Collect every ANGEL_ token from the source (directory modules included).
    let mut knobs: std::collections::BTreeSet<String> = std::collections::BTreeSet::new();
    let mut files = Vec::new();
    let mut dirs = vec![root.join("src")];
    while let Some(dir) = dirs.pop() {
        for entry in std::fs::read_dir(dir).unwrap() {
            let path = entry.unwrap().path();
            if path.is_dir() {
                dirs.push(path);
            } else if path.extension().and_then(|e| e.to_str()) == Some("rs") {
                files.push(path);
            }
        }
    }
    for path in files {
        let src = std::fs::read_to_string(&path).unwrap();
        let bytes = src.as_bytes();
        let mut i = 0;
        while let Some(rel) = src[i..].find("ANGEL_") {
            let start = i + rel;
            let mut end = start + "ANGEL_".len();
            while end < bytes.len()
                && (bytes[end].is_ascii_uppercase()
                    || bytes[end].is_ascii_digit()
                    || bytes[end] == b'_')
            {
                end += 1;
            }
            knobs.insert(src[start..end].to_string());
            i = end;
        }
    }
    // `ANGEL_T_*` names are synthetic namespaces used only by unit fixtures
    // that exercise dynamically constructed per-club controls. They are not
    // process knobs and must not pollute the operator reference.
    knobs.retain(|knob| !knob.starts_with("ANGEL_T_"));
    // Hook-injected vars are documented in prose; the rest are tracked here.
    // Doc-completeness is a *heads-up*, not a hard gate — it never blocks
    // interactive work. Set ANGEL_DOC_STRICT=1 to enforce it (e.g. a release /
    // CI step) and turn drift back into a failure.
    let missing: Vec<&String> = knobs.iter().filter(|k| !doc.contains(k.as_str())).collect();
    if !missing.is_empty() {
        if std::env::var_os("ANGEL_DOC_STRICT").is_some() {
            panic!("undocumented ANGEL_* knobs in docs/ENV.md: {missing:?} (ANGEL_DOC_STRICT)");
        }
        eprintln!(
            "note: undocumented ANGEL_* knobs in docs/ENV.md: {missing:?} (set ANGEL_DOC_STRICT=1 to enforce)"
        );
    }
    assert!(
        knobs.len() > 80,
        "sanity: expected the full knob set, found {}",
        knobs.len()
    );
}

#[test]
fn tool_panic_is_isolated_into_error_result() {
    struct PanicTool;
    impl Tool for PanicTool {
        fn name(&self) -> &str {
            "panic_probe"
        }
        fn def(&self) -> ToolDef {
            ToolDef {
                name: "panic_probe".into(),
                description: "fixture that panics".into(),
                params: serde_json::json!({"type": "object"}),
            }
        }
        fn call(&self, _args: &Value) -> Result<String, String> {
            panic!("boom")
        }
    }

    let mut registry = ToolRegistry::with_defaults();
    registry.register(Box::new(PanicTool));

    // A panicking tool must fold into a `tool error:` result — never unwind
    // through the turn loop and take down the cockpit.
    let err = registry
        .dispatch("panic_probe", &serde_json::json!({}))
        .expect_err("a panicking tool must become an Err, not unwind");
    assert!(err.starts_with("tool error:"), "got: {err}");
    assert!(err.contains("panicked"), "got: {err}");
    assert!(err.contains("panic_probe"), "got: {err}");
}
