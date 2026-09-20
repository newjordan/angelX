//! Realistic-scenario streak coverage: confinement, grep scopes, multi_edit,
//! task accept, and happy-path turn/outline contracts.
//!
//! Extracted from `harness/tests.rs` without behavior change so the monolith can
//! shrink while preserving the full harness test inventory.

use super::*;

// --- realistic-scenario sweep (streak coverage) --------------------------

#[test]
fn task_accept_gate_requires_red_then_green_and_rejects_zero_tests() {
    let root = scratch("task_accept");
    std::fs::create_dir_all(&root).unwrap();
    assert!(!run_task_accept("test -f ready", &root).passed);
    std::fs::write(root.join("ready"), "ok\n").unwrap();
    assert!(run_task_accept("test -f ready", &root).passed);
    assert!(!run_task_accept("printf 'test result: ok. 0 passed; 0 failed\\n'", &root).passed);
    let _ = std::fs::remove_dir_all(root);
}

/// Scenario: path traversal out of the workspace is refused (confinement).
#[test]
fn scenario_safe_path_rejects_escape() {
    let root = scratch("esc");
    let r = ReadFileTool { root: root.clone() };
    assert!(
        r.call(&serde_json::json!({"path":"../../../../etc/passwd"}))
            .is_err()
    );
    assert!(r.call(&serde_json::json!({"path":"/etc/passwd"})).is_err());
    let _ = std::fs::remove_dir_all(&root);
}

/// Models commonly echo the absolute workspace path printed by shell tools.
/// Accept that spelling without weakening the workspace boundary or charging a
/// retry hop merely to remove the root prefix.
#[test]
fn scenario_file_tools_accept_absolute_paths_only_inside_workspace() {
    // Exact-byte read contract; hashline anchors are on by default.
    let _env = crate::tests::env_lock();
    let _anchors = crate::tests::TestEnvGuard::set("ANGEL_HASHLINE_ANCHORS", "0");
    let root = scratch("absolute_in_workspace");
    std::fs::write(root.join("source.txt"), "before\n").unwrap();

    let source = root.join("source.txt").to_string_lossy().into_owned();
    assert_eq!(
        ReadFileTool { root: root.clone() }
            .call(&serde_json::json!({"path": source}))
            .unwrap(),
        "before\n"
    );
    StrReplaceTool { root: root.clone() }
        .call(&serde_json::json!({
            "path": root.join("source.txt").to_string_lossy(),
            "old": "before",
            "new": "after"
        }))
        .unwrap();
    WriteFileTool { root: root.clone() }
        .call(&serde_json::json!({
            "path": root.join("nested/new.txt").to_string_lossy(),
            "content": "created"
        }))
        .unwrap();
    assert_eq!(
        std::fs::read_to_string(root.join("source.txt")).unwrap(),
        "after\n"
    );
    assert_eq!(
        std::fs::read_to_string(root.join("nested/new.txt")).unwrap(),
        "created"
    );

    let outside = scratch("absolute_outside_workspace");
    std::fs::write(outside.join("secret.txt"), "outside").unwrap();
    assert!(
        ReadFileTool { root: root.clone() }
            .call(&serde_json::json!({
                "path": outside.join("secret.txt").to_string_lossy()
            }))
            .is_err()
    );
    assert!(
        WriteFileTool { root: root.clone() }
            .call(&serde_json::json!({
                "path": outside.join("new.txt").to_string_lossy(),
                "content": "escaped"
            }))
            .is_err()
    );
    assert!(!outside.join("new.txt").exists());

    let _ = std::fs::remove_dir_all(root);
    let _ = std::fs::remove_dir_all(outside);
}

/// Scenario: lexical confinement is not enough when an in-workspace symlink
/// points elsewhere. Existing reads and not-yet-created writes must both be
/// checked through the symlink's canonical target.
#[test]
fn scenario_safe_path_rejects_symlink_escape_for_reads_and_new_writes() {
    use std::os::unix::fs::symlink;

    let root = scratch("symlink_esc_root");
    let outside = scratch("symlink_esc_outside");
    std::fs::write(outside.join("secret.txt"), "outside").unwrap();
    symlink(&outside, root.join("escape")).unwrap();

    let reader = ReadFileTool { root: root.clone() };
    let writer = WriteFileTool { root: root.clone() };
    assert!(
        reader
            .call(&serde_json::json!({"path":"escape/secret.txt"}))
            .is_err(),
        "an existing file reached through an escaping symlink must be refused"
    );
    assert!(
        writer
            .call(&serde_json::json!({"path":"escape/new/deep.txt","content":"no"}))
            .is_err(),
        "a missing write target under an escaping symlink must be refused"
    );
    assert!(!outside.join("new/deep.txt").exists());

    // A symlink that remains inside the workspace is allowed, but the returned
    // path bypasses the original symlink and names its checked canonical target.
    let real = root.join("real");
    std::fs::create_dir_all(&real).unwrap();
    symlink(&real, root.join("inside")).unwrap();
    let resolved = safe_path(&root, "inside/new.txt").unwrap();
    assert_eq!(resolved, real.canonicalize().unwrap().join("new.txt"));
    writer
        .call(&serde_json::json!({"path":"inside/new.txt","content":"linked"}))
        .unwrap();
    assert_eq!(
        std::fs::read_to_string(real.join("new.txt")).unwrap(),
        "linked"
    );

    // A missing nested target under a real in-workspace directory remains valid.
    writer
        .call(&serde_json::json!({"path":"safe/new/deep.txt","content":"yes"}))
        .unwrap();
    assert_eq!(
        std::fs::read_to_string(root.join("safe/new/deep.txt")).unwrap(),
        "yes"
    );

    let _ = std::fs::remove_dir_all(&root);
    let _ = std::fs::remove_dir_all(&outside);
}

/// A canonicalized pathname is not a security boundary: its parent can be
/// replaced after validation. File tools must confine the actual open syscall.
#[cfg(target_os = "linux")]
#[test]
fn scenario_confined_file_ops_reject_post_validation_parent_swap() {
    use std::os::unix::fs::symlink;

    let root = scratch("confined_swap_root");
    let outside = scratch("confined_swap_outside");
    std::fs::create_dir_all(root.join("gate")).unwrap();
    std::fs::write(root.join("gate/data.txt"), "inside").unwrap();
    std::fs::write(outside.join("data.txt"), "outside").unwrap();

    // Reproduce the old check/use gap deterministically.
    let checked = safe_path(&root, "gate/data.txt").unwrap();
    std::fs::rename(root.join("gate"), root.join("held")).unwrap();
    symlink(&outside, root.join("gate")).unwrap();
    assert_eq!(std::fs::read_to_string(&checked).unwrap(), "outside");

    let reader = ReadFileTool { root: root.clone() };
    let writer = WriteFileTool { root: root.clone() };
    assert!(
        reader
            .call(&serde_json::json!({"path":"gate/data.txt"}))
            .is_err()
    );
    assert!(
        OutlineTool { root: root.clone() }
            .call(&serde_json::json!({"path":"gate/data.txt"}))
            .is_err()
    );
    assert!(
        ListDirTool { root: root.clone() }
            .call(&serde_json::json!({"path":"gate"}))
            .is_err()
    );
    assert!(
        GrepTool { root: root.clone() }
            .call(&serde_json::json!({"path":"gate","pattern":"outside"}))
            .is_err()
    );
    assert!(
        writer
            .call(&serde_json::json!({"path":"gate/data.txt","content":"escaped"}))
            .is_err()
    );
    assert_eq!(
        std::fs::read_to_string(outside.join("data.txt")).unwrap(),
        "outside"
    );

    let _ = std::fs::remove_dir_all(&root);
    let _ = std::fs::remove_dir_all(&outside);
}

#[cfg(target_os = "linux")]
#[test]
fn scenario_confined_writes_reject_final_symlinks_and_special_files() {
    use std::ffi::CString;
    use std::os::unix::ffi::OsStrExt;
    use std::os::unix::fs::symlink;

    let _env = crate::tests::env_lock();
    let _anchors = crate::tests::TestEnvGuard::set("ANGEL_HASHLINE_ANCHORS", "0");
    let root = scratch("confined_final");
    std::fs::write(root.join("target.txt"), "original").unwrap();
    symlink("target.txt", root.join("link.txt")).unwrap();

    // Reads may follow a safe in-workspace final symlink, but mutations may not.
    assert_eq!(
        ReadFileTool { root: root.clone() }
            .call(&serde_json::json!({"path":"link.txt"}))
            .unwrap(),
        "original"
    );
    assert!(
        WriteFileTool { root: root.clone() }
            .call(&serde_json::json!({"path":"link.txt","content":"changed"}))
            .is_err()
    );
    assert!(
        StrReplaceTool { root: root.clone() }
            .call(&serde_json::json!({"path":"link.txt","old":"original","new":"changed"}))
            .is_err()
    );
    assert!(
        StrReplaceTool { root: root.clone() }
            .call(&serde_json::json!({
                "path": root.join("link.txt").to_string_lossy(),
                "old": "original",
                "new": "changed"
            }))
            .is_err()
    );
    assert_eq!(
        std::fs::read_to_string(root.join("target.txt")).unwrap(),
        "original"
    );

    // O_NONBLOCK plus regular-file validation prevents a FIFO from wedging a
    // harness worker and refuses it as either a read or write target.
    let fifo = root.join("pipe");
    let fifo_c = CString::new(fifo.as_os_str().as_bytes()).unwrap();
    assert_eq!(unsafe { libc::mkfifo(fifo_c.as_ptr(), 0o600) }, 0);
    assert!(
        ReadFileTool { root: root.clone() }
            .call(&serde_json::json!({"path":"pipe"}))
            .is_err()
    );
    assert!(
        WriteFileTool { root: root.clone() }
            .call(&serde_json::json!({"path":"pipe","content":"no"}))
            .is_err()
    );

    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn scenario_atomic_replace_keeps_mode_and_leaves_no_temp() {
    use std::os::unix::fs::PermissionsExt;

    let root = scratch("atomic_replace");
    let script = root.join("run.sh");
    std::fs::write(&script, "#!/bin/sh\necho original\n").unwrap();
    std::fs::set_permissions(&script, std::fs::Permissions::from_mode(0o755)).unwrap();

    StrReplaceTool { root: root.clone() }
        .call(&serde_json::json!({"path":"run.sh","old":"original","new":"changed"}))
        .unwrap();
    assert_eq!(
        std::fs::read_to_string(&script).unwrap(),
        "#!/bin/sh\necho changed\n"
    );
    // The replace lands on a fresh inode via rename; an executable that lost
    // its mode bits there would break every script the agent touches.
    assert_eq!(
        std::fs::metadata(&script).unwrap().permissions().mode() & 0o7777,
        0o755
    );
    // A successful replace must not strand its temp sibling.
    assert!(
        !std::fs::read_dir(&root).unwrap().any(|entry| entry
            .unwrap()
            .file_name()
            .to_string_lossy()
            .contains("angel-tmp")),
        "temp sibling survived the rename"
    );

    // Full-file rewrite takes the same atomic path.
    WriteFileTool { root: root.clone() }
        .call(&serde_json::json!({"path":"run.sh","content":"#!/bin/sh\necho rewritten\n"}))
        .unwrap();
    assert_eq!(
        std::fs::metadata(&script).unwrap().permissions().mode() & 0o7777,
        0o755
    );

    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn scenario_missing_read_suggests_confined_suffix_match_without_auto_open() {
    let _lock = crate::tests::env_lock();
    let _enabled = EnvGuard::set("ANGEL_MISSING_PATH_HINTS", "1");
    let root = scratch("missing_read_hint");
    let candidate = root.join("plugins/codex/scripts/codex-companion.mjs");
    std::fs::create_dir_all(candidate.parent().unwrap()).unwrap();
    std::fs::write(&candidate, "candidate-content-must-not-be-returned\n").unwrap();

    let error = ReadFileTool { root: root.clone() }
        .call(&serde_json::json!({"path":"scripts/codex-companion.mjs"}))
        .unwrap_err();
    assert!(error.contains("No such file or directory"), "{error}");
    assert!(error.contains(crate::agent::tools::file::MISSING_PATH_HINT_MARKER));
    assert!(
        error.contains("- plugins/codex/scripts/codex-companion.mjs"),
        "{error}"
    );
    assert!(!error.contains("candidate-content-must-not-be-returned"));

    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn scenario_missing_read_hints_are_ranked_bounded_and_policy_filtered() {
    let _lock = crate::tests::env_lock();
    let _enabled = EnvGuard::set("ANGEL_MISSING_PATH_HINTS", "1");
    let root = scratch("missing_read_rank");
    for path in [
        "plugins/scripts/tool.mjs",
        "deep/plugins/scripts/tool.mjs",
        "examples/tool.mjs",
        "a/tool.mjs",
        "b/tool.mjs",
        "c/tool.mjs",
        "d/tool.mjs",
    ] {
        let path = root.join(path);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, "safe\n").unwrap();
    }
    std::fs::create_dir_all(root.join(".private/scripts")).unwrap();
    std::fs::write(root.join(".private/scripts/tool.mjs"), "hidden\n").unwrap();
    std::fs::create_dir_all(root.join("safe")).unwrap();
    std::fs::write(root.join("safe/credentials.json"), "secret\n").unwrap();

    let reader = ReadFileTool { root: root.clone() };
    let error = reader
        .call(&serde_json::json!({"path":"scripts/tool.mjs"}))
        .unwrap_err();
    let hints = error
        .split(crate::agent::tools::file::MISSING_PATH_HINT_MARKER)
        .nth(1)
        .unwrap()
        .lines()
        .filter(|line| line.starts_with("- "))
        .collect::<Vec<_>>();
    assert_eq!(hints.len(), 5, "{error}");
    assert_eq!(hints[0], "- plugins/scripts/tool.mjs");
    assert_eq!(hints[1], "- deep/plugins/scripts/tool.mjs");
    assert!(!error.contains(".private"), "{error}");
    let credential = reader
        .call(&serde_json::json!({"path":"credentials.json"}))
        .unwrap_err();
    assert!(!credential.contains(crate::agent::tools::file::MISSING_PATH_HINT_MARKER));
    let escape = reader
        .call(&serde_json::json!({"path":"../../outside/tool.mjs"}))
        .unwrap_err();
    assert!(!escape.contains(crate::agent::tools::file::MISSING_PATH_HINT_MARKER));

    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn scenario_missing_read_hint_has_an_executable_off_control() {
    let _lock = crate::tests::env_lock();
    let root = scratch("missing_read_off");
    std::fs::create_dir_all(root.join("nested")).unwrap();
    std::fs::write(root.join("nested/file.rs"), "content\n").unwrap();

    let _disabled = EnvGuard::set("ANGEL_MISSING_PATH_HINTS", "0");
    let error = ReadFileTool { root: root.clone() }
        .call(&serde_json::json!({"path":"file.rs"}))
        .unwrap_err();
    assert!(error.contains("No such file or directory"), "{error}");
    assert!(!error.contains(crate::agent::tools::file::MISSING_PATH_HINT_MARKER));

    let _ = std::fs::remove_dir_all(root);
}

/// Scenario: write→read roundtrip preserves unicode exactly, creates parent
/// dirs, and an empty file reads back empty.
#[test]
fn scenario_write_read_roundtrip_unicode_and_parents() {
    let _env = crate::tests::env_lock();
    let _anchors = crate::tests::TestEnvGuard::set("ANGEL_HASHLINE_ANCHORS", "0");
    let root = scratch("wr");
    let w = WriteFileTool { root: root.clone() };
    let r = ReadFileTool { root: root.clone() };
    let content = "日本語 🚀\nsecond café line\n";
    w.call(&serde_json::json!({"path":"nested/dir/f.txt","content":content}))
        .expect("write");
    assert_eq!(
        r.call(&serde_json::json!({"path":"nested/dir/f.txt"}))
            .unwrap(),
        content
    );
    w.call(&serde_json::json!({"path":"empty.txt","content":""}))
        .unwrap();
    assert_eq!(
        r.call(&serde_json::json!({"path":"empty.txt"})).unwrap(),
        ""
    );
    let _ = std::fs::remove_dir_all(&root);
}

/// Scenario: a huge tool result is capped to the line budget, keeps head and
/// tail, and marks the elision.
#[test]
fn scenario_cap_tool_output_keeps_ends_and_marks_elision() {
    let huge = (0..5000)
        .map(|i| format!("line {i}"))
        .collect::<Vec<_>>()
        .join("\n");
    let capped = cap_text(&huge, 0, 100);
    assert!(
        capped.lines().count() <= 110,
        "respects max_lines: {}",
        capped.lines().count()
    );
    assert!(capped.contains("line 0"), "keeps head");
    assert!(capped.contains("line 4999"), "keeps tail");
    assert!(capped.contains("elided"), "marks the elision");
}

/// Scenario: list_dir + grep over the real source tree return real results.
#[test]
fn scenario_list_dir_and_grep_real_tree() {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let ld = ListDirTool { root: root.clone() }
        .call(&serde_json::json!({"path":"src"}))
        .expect("list_dir");
    assert!(
        ld.contains("main.rs") && ld.contains("agent/"),
        "list_dir src:\n{ld}"
    );
    let g = GrepTool { root }
        .call(&serde_json::json!({"pattern":"pub fn run_turn\\(","path":"src"}))
        .expect("grep");
    assert!(
        g.contains("agent/harness/turn/mod.rs:"),
        "grep should find the production run_turn:\n{g}"
    );
}

#[test]
fn scenario_grep_accepts_relative_and_absolute_file_scope() {
    let _lock = crate::tests::env_lock();
    let root = scratch("grep_file_scope");
    std::fs::create_dir_all(root.join("src")).unwrap();
    std::fs::write(root.join("src/one.rs"), "first\nneedle one\n").unwrap();
    std::fs::write(root.join("src/two.rs"), "needle two\n").unwrap();
    let grep = GrepTool { root: root.clone() };

    {
        let _enabled = EnvGuard::set("ANGEL_GREP_FILE_SCOPE", "1");
        let relative = grep
            .call(&serde_json::json!({"pattern":"needle", "path":"src/one.rs"}))
            .unwrap();
        assert!(relative.contains("src/one.rs:2:needle one"), "{relative}");
        assert!(
            !relative.contains("two.rs"),
            "file scope leaked: {relative}"
        );

        let absolute = grep
            .call(&serde_json::json!({
                "pattern":"needle",
                "path": root.join("src/one.rs").to_string_lossy()
            }))
            .unwrap();
        assert_eq!(absolute, relative);

        let directory = grep
            .call(&serde_json::json!({"pattern":"needle", "path":"src"}))
            .unwrap();
        assert!(directory.contains("src/one.rs:2:needle one"), "{directory}");
        assert!(directory.contains("src/two.rs:1:needle two"), "{directory}");
    }

    {
        let _disabled = EnvGuard::set("ANGEL_GREP_FILE_SCOPE", "0");
        let error = grep
            .call(&serde_json::json!({"pattern":"needle", "path":"src/one.rs"}))
            .unwrap_err();
        assert!(
            error.contains("Not a directory"),
            "legacy control drifted: {error}"
        );
    }

    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn scenario_grep_file_scope_preserves_search_policy_and_size_bound() {
    let _lock = crate::tests::env_lock();
    let _enabled = EnvGuard::set("ANGEL_GREP_FILE_SCOPE", "1");
    let root = scratch("grep_file_policy");
    std::fs::write(root.join(".secret"), "needle\n").unwrap();
    std::fs::write(root.join("credentials.json"), "needle\n").unwrap();
    std::fs::write(root.join("binary.dat"), b"needle\0payload").unwrap();
    std::fs::write(
        root.join("large.txt"),
        vec![b'x'; crate::agent::tools::nav::SEARCH_FILE_MAX_BYTES + 1],
    )
    .unwrap();
    let grep = GrepTool { root: root.clone() };

    for path in [".secret", "credentials.json"] {
        let error = grep
            .call(&serde_json::json!({"pattern":"needle", "path":path}))
            .unwrap_err();
        assert!(
            error.contains("excluded by workspace search policy"),
            "{error}"
        );
    }
    let large = grep
        .call(&serde_json::json!({"pattern":"needle", "path":"large.txt"}))
        .unwrap_err();
    assert!(large.contains("search limit"), "{large}");
    assert_eq!(
        grep.call(&serde_json::json!({"pattern":"needle", "path":"binary.dat"}))
            .unwrap(),
        "no matches"
    );

    let _ = std::fs::remove_dir_all(root);
}

#[cfg(target_os = "linux")]
#[test]
fn scenario_grep_file_scope_rejects_outbound_symlink_and_absolute_escape() {
    use std::os::unix::fs::symlink;

    let _lock = crate::tests::env_lock();
    let _enabled = EnvGuard::set("ANGEL_GREP_FILE_SCOPE", "1");
    let root = scratch("grep_file_escape_root");
    let outside = scratch("grep_file_escape_outside");
    std::fs::write(root.join("inside.txt"), "needle inside\n").unwrap();
    symlink("inside.txt", root.join("inside-link.txt")).unwrap();
    std::fs::write(outside.join("secret.txt"), "needle\n").unwrap();
    symlink(outside.join("secret.txt"), root.join("escape.txt")).unwrap();
    let grep = GrepTool { root: root.clone() };

    let inside = grep
        .call(&serde_json::json!({"pattern":"needle", "path":"inside-link.txt"}))
        .unwrap();
    assert!(
        inside.contains("inside-link.txt:1:needle inside"),
        "{inside}"
    );
    assert!(
        grep.call(&serde_json::json!({"pattern":"needle", "path":"escape.txt"}))
            .is_err()
    );
    assert!(
        grep.call(&serde_json::json!({
            "pattern":"needle",
            "path": outside.join("secret.txt").to_string_lossy()
        }))
        .is_err()
    );

    let _ = std::fs::remove_dir_all(root);
    let _ = std::fs::remove_dir_all(outside);
}

#[test]
fn scenario_grep_context_merges_overlaps_and_preserves_zero_control() {
    let _lock = crate::tests::env_lock();
    let _file_scope = EnvGuard::set("ANGEL_GREP_FILE_SCOPE", "1");
    let root = scratch("grep_context_merge");
    std::fs::write(
        root.join("source.rs"),
        "line1\nline2\nline3\nneedle4\nline5\nneedle6\nline7\nline8\nline9\n",
    )
    .unwrap();
    let grep = GrepTool { root: root.clone() };

    let contextual = grep
        .call(&serde_json::json!({
            "pattern":"needle",
            "path":"source.rs",
            "context":2
        }))
        .unwrap();
    assert_eq!(contextual.lines().count(), 7, "{contextual}");
    assert!(contextual.contains("source.rs-2-line2"), "{contextual}");
    assert!(contextual.contains("source.rs:4:needle4"), "{contextual}");
    assert!(contextual.contains("source.rs-5-line5"), "{contextual}");
    assert!(contextual.contains("source.rs:6:needle6"), "{contextual}");
    assert_eq!(contextual.matches("source.rs-5-line5").count(), 1);
    assert!(
        !contextual.contains("\n--\n"),
        "overlap was not merged: {contextual}"
    );

    let zero = grep
        .call(&serde_json::json!({
            "pattern":"needle",
            "path":"source.rs",
            "context":0
        }))
        .unwrap();
    assert_eq!(zero, "source.rs:4:needle4\nsource.rs:6:needle6");

    let many = (1..=80)
        .map(|line| format!("needle {line}\n"))
        .collect::<String>();
    std::fs::write(root.join("many.rs"), many).unwrap();
    let legacy_capacity = grep
        .call(&serde_json::json!({
            "pattern":"needle",
            "path":"many.rs",
            "context":0
        }))
        .unwrap();
    assert_eq!(legacy_capacity.lines().count(), 80);

    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn scenario_grep_context_env_default_is_overridable_and_bounded() {
    let _lock = crate::tests::env_lock();
    let _file_scope = EnvGuard::set("ANGEL_GREP_FILE_SCOPE", "1");
    let _default = EnvGuard::set("ANGEL_GREP_CONTEXT_LINES", "1");
    let root = scratch("grep_context_default");
    let mut text = String::new();
    for line in 1..=1_300 {
        if line % 25 == 0 {
            text.push_str(&format!("needle {line}\n"));
        } else {
            text.push_str(&format!("line {line}\n"));
        }
    }
    std::fs::write(root.join("source.rs"), text).unwrap();
    let grep = GrepTool { root: root.clone() };

    let from_env = grep
        .call(&serde_json::json!({"pattern":"needle", "path":"source.rs"}))
        .unwrap();
    assert!(from_env.contains("source.rs-24-line 24"), "{from_env}");
    let explicit_zero = grep
        .call(&serde_json::json!({
            "pattern":"needle",
            "path":"source.rs",
            "context":0
        }))
        .unwrap();
    assert!(!explicit_zero.contains("source.rs-24-line 24"));
    let capped = grep
        .call(&serde_json::json!({
            "pattern":"needle",
            "path":"source.rs",
            "context":10
        }))
        .unwrap();
    assert!(capped.lines().count() <= crate::agent::tools::nav::GREP_MAX_LINES);
    assert_ne!(capped.lines().last(), Some("--"));

    for invalid in [
        serde_json::json!(-1),
        serde_json::json!(11),
        serde_json::json!(1.5),
    ] {
        assert!(
            grep.call(&serde_json::json!({
                "pattern":"needle",
                "path":"source.rs",
                "context":invalid
            }))
            .is_err()
        );
    }

    let _ = std::fs::remove_dir_all(root);
}

/// Scenario: an invalid regex must be handled gracefully — never a panic.
#[test]
fn scenario_grep_invalid_regex_is_graceful() {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    // Returns Ok (ripgrep prints nothing / fallback treats it as literal) or
    // Err — either is fine; the point is it doesn't panic.
    let _ = GrepTool { root }.call(&serde_json::json!({"pattern":"(unclosed[","path":"src"}));
}

/// Scenario: multi_edit is atomic — a failing edit in the batch leaves the
/// file untouched (no half-apply).
#[test]
fn scenario_multi_edit_is_atomic() {
    let root = scratch("me");
    std::fs::write(root.join("f.txt"), "alpha\nbeta\n").unwrap();
    let me = MultiEditTool { root: root.clone() };
    let res = me.call(&serde_json::json!({
        "path":"f.txt",
        "edits":[{"old":"alpha","new":"ALPHA"},{"old":"ZZZ_absent","new":"X"}],
    }));
    assert!(res.is_err(), "a missing 'old' should fail the batch");
    assert_eq!(
        std::fs::read_to_string(root.join("f.txt")).unwrap(),
        "alpha\nbeta\n",
        "atomic: a failed batch must not half-apply"
    );
    let _ = std::fs::remove_dir_all(&root);
}

/// Scenario: multi_edit happy path applies ordered edits to one file.
#[test]
fn scenario_multi_edit_applies_ordered() {
    let root = scratch("me2");
    std::fs::write(root.join("f.txt"), "alpha\nbeta\n").unwrap();
    MultiEditTool { root: root.clone() }
        .call(&serde_json::json!({
            "path":"f.txt",
            "edits":[{"old":"alpha","new":"ALPHA"},{"old":"beta","new":"BETA"}],
        }))
        .expect("multi_edit ok");
    assert_eq!(
        std::fs::read_to_string(root.join("f.txt")).unwrap(),
        "ALPHA\nBETA\n"
    );
    let _ = std::fs::remove_dir_all(&root);
}

/// Scenario: a realistic happy-path turn — the agent reads a real file, then
/// answers. run_turn returns the final text and the tool result is in history.
#[test]
fn scenario_run_turn_read_then_answer() {
    let _guard = crate::tests::env_lock();
    let _handles = crate::tests::TestEnvGuard::set("ANGEL_HANDLE_STORE", "0");
    struct ReadThenAnswer {
        hop: AtomicUsize,
    }
    impl Club for ReadThenAnswer {
        fn respond(&self, _p: &str) -> Result<String, String> {
            Ok(String::new())
        }
        fn label(&self) -> &str {
            "rta"
        }
        fn chat(&self, _m: &[ChatMsg], _t: &[ToolDef]) -> Result<ClubReply, String> {
            if self.hop.fetch_add(1, Ordering::Relaxed) == 0 {
                Ok(ClubReply::Calls(vec![ToolCall {
                    id: "r".into(),
                    name: "read_file".into(),
                    args: serde_json::json!({ "path": "Cargo.toml" }),
                }]))
            } else {
                Ok(ClubReply::Text("done — read the manifest".into()))
            }
        }
    }
    let mut history = vec![ChatMsg::user("what package is this?")];
    let answer = run_turn(
        &ReadThenAnswer {
            hop: AtomicUsize::new(0),
        },
        &ToolRegistry::with_defaults(),
        &mut history,
        &AtomicBool::new(false),
        Some(10),
        &mpsc::channel::<TurnEvent>().0,
    )
    .expect("turn completes");
    assert!(answer.contains("done"), "final answer: {answer}");
    let tool_msg = history
        .iter()
        .find(|m| m.role == ChatRole::Tool)
        .expect("a tool result is recorded");
    assert!(
        tool_msg.content.contains("angel0-cockpit") || tool_msg.content.contains("[package]"),
        "read_file should have returned the manifest:\n{}",
        tool_msg.content
    );
}

/// Scenario: a path-guess miss on `outline` carries did-you-mean hints, the
/// same as `read_file` — a wrong-root guess (src/lsp.rs vs cockpit/src/agent/lsp.rs
/// class) must cost one corrected call, not a listing round trip.
#[test]
fn scenario_outline_miss_suggests_paths() {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let err = OutlineTool { root }
        .call(&serde_json::json!({ "path": "wrongdir/lsp.rs" }))
        .expect_err("missing path must error");
    assert!(
        err.contains("src/agent/lsp.rs"),
        "outline miss should suggest the real path:\n{err}"
    );
}

/// Scenario: the regex `outline` tool maps a real source file's symbols.
#[test]
fn scenario_outline_real_source() {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let out = OutlineTool { root }
        .call(&serde_json::json!({ "path": "src/agent/lsp.rs" }))
        .expect("outline");
    assert!(
        out.contains("fn discover_lsp_tools") || out.contains("discover_lsp_tools"),
        "outline should surface a known fn:\n{out}"
    );
    assert!(
        out.contains("LspPool") || out.contains("LspClient"),
        "outline should surface a struct:\n{out}"
    );
}
