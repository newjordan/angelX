//! Live git tooling and porcelain summary helpers.
//!
//! Extracted from `harness/tests.rs` without behavior change so the monolith can
//! shrink while preserving the full harness test inventory.

use super::*;

// --- git tools suite ---

#[test]
fn git_tools_parse_live_repo() {
    let ws = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let status = GitStatusTool {
        workspace: ws.clone(),
    }
    .call(&serde_json::json!({}))
    .expect("git_status on the real repo");
    // Derive the current branch rather than hardcoding one — otherwise this
    // test fails on every branch but the one it was written on.
    let branch = Command::new("git")
        .args(["rev-parse", "--abbrev-ref", "HEAD"])
        .current_dir(&ws)
        .output()
        .expect("git rev-parse --abbrev-ref HEAD");
    let branch = String::from_utf8_lossy(&branch.stdout).trim().to_string();
    assert!(
        status.contains(&branch),
        "status should name the current branch {branch}:\n{status}"
    );
    let head = Command::new("git")
        .args(["rev-parse", "--short", "HEAD"])
        .current_dir(&ws)
        .output()
        .expect("git rev-parse HEAD");
    assert!(head.status.success(), "git rev-parse failed");
    let head = String::from_utf8_lossy(&head.stdout).trim().to_string();
    let log = GitLogTool {
        workspace: ws.clone(),
    }
    .call(&serde_json::json!({ "count": 20 }))
    .expect("git_log on the real repo");
    assert!(
        log.contains(&head),
        "log should include the current HEAD {head}:\n{log}"
    );

    let cargo_toml = ws.join("Cargo.toml").to_string_lossy().into_owned();
    GitLogTool {
        workspace: ws.clone(),
    }
    .call(&serde_json::json!({ "count": 1, "path": cargo_toml }))
    .expect("git_log accepts an absolute path inside the workspace");
    GitDiffTool {
        workspace: ws.clone(),
    }
    .call(&serde_json::json!({ "path": ws.join("Cargo.toml").to_string_lossy() }))
    .expect("git_diff accepts an absolute path inside the workspace");
    GitDiffTool {
        workspace: ws.clone(),
    }
    .call(&serde_json::json!({ "base": "HEAD", "stat": true }))
    .expect("git_diff compares the worktree against a verified commit");
    assert!(
        GitDiffTool {
            workspace: ws.clone(),
        }
        .call(&serde_json::json!({ "base": "definitely-not-an-angel0-revision" }))
        .is_err()
    );
    assert!(
        GitDiffTool {
            workspace: ws.clone(),
        }
        .call(&serde_json::json!({ "base": "HEAD", "staged": true }))
        .is_err()
    );
    assert!(
        GitDiffTool {
            workspace: ws.clone(),
        }
        .call(&serde_json::json!({ "path": ws.join("../README.md").to_string_lossy() }))
        .is_err()
    );
}

#[test]
fn git_diff_argv_is_explicit_and_rejects_option_or_range_bases() {
    assert_eq!(
        git_diff_argv(false, None, false, None).unwrap(),
        vec!["diff", "--no-ext-diff", "--no-textconv"]
    );
    assert_eq!(
        git_diff_argv(true, None, true, Some("src/main.rs")).unwrap(),
        vec![
            "diff",
            "--no-ext-diff",
            "--no-textconv",
            "--stat",
            "--cached",
            "--",
            "src/main.rs",
        ]
    );
    assert_eq!(
        git_diff_argv(false, Some("origin/main"), false, None).unwrap(),
        vec!["diff", "--no-ext-diff", "--no-textconv", "origin/main"]
    );
    assert!(git_diff_argv(true, Some("HEAD"), false, None).is_err());
    for invalid in ["", "-p", "HEAD..main", "HEAD main", "HEAD:secret"] {
        assert!(validate_git_revision(invalid).is_err(), "{invalid:?}");
    }
    assert!(validate_git_revision(&"a".repeat(129)).is_err());
    for valid in ["HEAD", "HEAD~2", "origin/main", "v1.2.3", "abc123^"] {
        assert_eq!(validate_git_revision(valid).unwrap(), valid);
    }
}

#[test]
fn git_status_summary_parses_porcelain() {
    // X (index) / Y (worktree): "M " staged, " M" modified, "MM" both, "??" untracked.
    let p =
        "## feat/x...origin/feat/x [ahead 2]\nM  src/a.rs\n M src/b.rs\n?? new.txt\nMM src/c.rs\n";
    let s = summarize_git_status(p);
    assert!(
        s.contains("branch: feat/x...origin/feat/x [ahead 2]"),
        "{s}"
    );
    assert!(
        s.contains("staged") && s.contains("src/a.rs") && s.contains("src/c.rs"),
        "{s}"
    );
    assert!(s.contains("modified") && s.contains("src/b.rs"), "{s}");
    assert!(s.contains("untracked") && s.contains("new.txt"), "{s}");
}

#[test]
fn git_log_argv_clamps_and_scopes() {
    assert_eq!(
        git_log_argv(15, None),
        vec!["log", "--oneline", "--no-color", "-n15"]
    );
    // count clamps to [1,100]
    assert_eq!(git_log_argv(0, None)[3], "-n1");
    assert_eq!(git_log_argv(9999, None)[3], "-n100");
    // path is scoped after a `--` separator
    let scoped = git_log_argv(5, Some("src/foo.rs"));
    assert_eq!(
        scoped,
        vec!["log", "--oneline", "--no-color", "-n5", "--", "src/foo.rs"]
    );
    // empty path is ignored (no separator)
    assert!(!git_log_argv(5, Some("")).contains(&"--".to_string()));
}

#[test]
fn git_status_summary_reports_clean_tree() {
    let s = summarize_git_status("## main...origin/main\n");
    assert!(s.contains("working tree clean"), "{s}");
    assert!(s.contains("branch: main"), "{s}");
}

#[test]
fn git_commit_plans_and_executes_atomic_splits() {
    use crate::agent::tools::git::GitCommitTool;
    use std::process::Command;

    let root = std::env::temp_dir().join(format!(
        "angel_git_commit_{}_{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir_all(root.join("src")).unwrap();
    std::fs::create_dir_all(root.join("tests")).unwrap();

    let git = |args: &[&str]| {
        let out = Command::new("git")
            .args(["-c", "user.name=test", "-c", "user.email=test@local"])
            .args(args)
            .current_dir(&root)
            .output()
            .expect("git");
        assert!(
            out.status.success(),
            "git {args:?} failed: {}",
            String::from_utf8_lossy(&out.stderr)
        );
    };
    git(&["init", "-q"]);
    // Default branch name varies; force main for readability.
    let _ = Command::new("git")
        .args(["branch", "-M", "main"])
        .current_dir(&root)
        .output();
    std::fs::write(root.join("src/lib.rs"), "pub fn a() {}\n").unwrap();
    git(&["add", "-A"]);
    git(&["commit", "-q", "-m", "init"]);

    // Dirty tree across classes.
    std::fs::write(root.join("src/lib.rs"), "pub fn a() { 1 }\n").unwrap();
    std::fs::write(root.join("tests/lib_test.rs"), "#[test] fn t() {}\n").unwrap();
    std::fs::write(root.join("README.md"), "# fixture\n").unwrap();

    let tool = GitCommitTool {
        workspace: root.clone(),
    };

    // Plan-only default.
    let plan = tool.call(&serde_json::json!({})).unwrap();
    assert!(plan.contains("atomic commit plan"), "{plan}");
    assert!(plan.contains("class=source"), "{plan}");
    assert!(plan.contains("class=test"), "{plan}");
    assert!(plan.contains("class=docs"), "{plan}");
    assert!(plan.contains("execute=true"), "{plan}");

    // Execute split.
    let done = tool
        .call(&serde_json::json!({ "execute": true, "split": true }))
        .unwrap();
    assert!(done.contains("atomic commit execute"), "{done}");
    assert!(done.contains("source"), "{done}");

    // Tree should be clean.
    let status = Command::new("git")
        .args(["status", "--porcelain"])
        .current_dir(&root)
        .output()
        .unwrap();
    assert!(
        String::from_utf8_lossy(&status.stdout).trim().is_empty(),
        "expected clean tree after execute: {}",
        String::from_utf8_lossy(&status.stdout)
    );

    // At least init + 3 class commits.
    let log = Command::new("git")
        .args(["log", "--oneline"])
        .current_dir(&root)
        .output()
        .unwrap();
    let lines: Vec<_> = String::from_utf8_lossy(&log.stdout)
        .lines()
        .map(str::to_string)
        .collect();
    assert!(
        lines.len() >= 4,
        "expected ≥4 commits, got {}: {lines:?}",
        lines.len()
    );

    let _ = std::fs::remove_dir_all(&root);
}
