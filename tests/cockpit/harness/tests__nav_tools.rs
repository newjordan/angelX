//! Navigation helpers: glob, defs, find, grep, outline, fuzzy rank, git_diff.
//!
//! Extracted from `harness/tests.rs` without behavior change so the monolith can
//! shrink while preserving the full harness test inventory.

use super::*;

// --- nav tools suite --------------------------------------------------------

#[test]
fn truncate_to_char_boundary_handles_multibyte_edges() {
    let mut s = format!("{}{}", "x".repeat(3999), "ើtail");
    truncate_to_char_boundary(&mut s, 4000);
    assert_eq!(s, "x".repeat(3999));

    let mut ascii = "abcdef".to_string();
    truncate_to_char_boundary(&mut ascii, 4);
    assert_eq!(ascii, "abcd");
}

#[test]
fn glob_match_handles_wildcards_and_double_star() {
    assert!(glob_match("**/*.rs", "src/sub/c.rs"));
    assert!(glob_match("**/*.rs", "a.rs"));
    assert!(glob_match("src/*.rs", "src/a.rs"));
    assert!(!glob_match("src/*.rs", "src/sub/a.rs")); // * doesn't cross /
    assert!(!glob_match("*.rs", "src/a.rs")); // top-level only
    assert!(glob_match("*.toml", "Cargo.toml"));
    assert!(glob_match("?.txt", "a.txt"));
    assert!(!glob_match("?.txt", "ab.txt"));
}

#[test]
fn defs_finds_definitions() {
    let root = std::env::temp_dir().join(format!("angel_defs_{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir_all(root.join("src")).unwrap();
    std::fs::write(
        root.join("src/lib.rs"),
        "pub fn alpha() {}\nstruct Beta;\nfn uses_alpha() { alpha(); }\n",
    )
    .unwrap();
    let mut reg = ToolRegistry::new();
    register_file_tools(&mut reg, root.clone());

    let d = reg
        .dispatch("defs", &serde_json::json!({"name": "alpha"}))
        .unwrap();
    assert!(d.contains("fn alpha"), "should find the definition: {d}");
    let b = reg
        .dispatch("defs", &serde_json::json!({"name": "Beta"}))
        .unwrap();
    assert!(b.contains("struct Beta"), "got: {b}");
    // a non-identifier name is rejected (no regex injection)
    assert!(
        reg.dispatch("defs", &serde_json::json!({"name": "a b"}))
            .is_err()
    );

    // helper precision (the dependency-free fallback path)
    assert!(line_defines("pub fn alpha() {", "alpha"));
    assert!(!line_defines("    alpha();", "alpha"));
    assert!(word_present("alpha()", "alpha"));
    assert!(!word_present("uses_alpha", "alpha"));
    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn find_files_lists_matches() {
    let root = std::env::temp_dir().join(format!("angel_ff_{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir_all(root.join("src/sub")).unwrap();
    std::fs::write(root.join("src/a.rs"), "").unwrap();
    std::fs::write(root.join("src/sub/c.rs"), "").unwrap();
    std::fs::write(root.join("Cargo.toml"), "").unwrap();
    let mut reg = ToolRegistry::new();
    register_file_tools(&mut reg, root.clone());

    let rs = reg
        .dispatch("find_files", &serde_json::json!({"pattern": "**/*.rs"}))
        .unwrap();
    assert!(
        rs.contains("src/a.rs") && rs.contains("src/sub/c.rs"),
        "got: {rs}"
    );
    assert!(!rs.contains("Cargo.toml"), "got: {rs}");

    let toml = reg
        .dispatch("find_files", &serde_json::json!({"pattern": "*.toml"}))
        .unwrap();
    assert!(
        toml.contains("Cargo.toml") && !toml.contains("a.rs"),
        "got: {toml}"
    );
    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn grep_finds_matches_and_handles_misses() {
    let root = std::env::temp_dir().join(format!("angel_grep_{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir_all(root.join("src")).unwrap();
    std::fs::write(root.join("src/a.rs"), "fn alpha() {}\nfn beta() {}\n").unwrap();
    std::fs::write(root.join("src/b.rs"), "let gamma = 1;\n").unwrap();
    let mut reg = ToolRegistry::new();
    register_file_tools(&mut reg, root.clone());

    let hit = reg
        .dispatch("grep", &serde_json::json!({"pattern": "beta"}))
        .unwrap();
    assert!(hit.contains("a.rs") && hit.contains("beta"), "got: {hit}");

    let miss = reg
        .dispatch("grep", &serde_json::json!({"pattern": "zzz_no_such_token"}))
        .unwrap();
    assert_eq!(miss, "no matches");

    // Exercise the dependency-free fallback directly (independent of `rg`).
    let fb = grep_fallback(&root, "gamma", false).unwrap();
    assert!(fb.contains("b.rs") && fb.contains("gamma"), "got: {fb}");
    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn grep_rejects_oversized_sparse_files_before_reading() {
    let root = scratch("grep_sparse");
    std::fs::write(root.join("visible.txt"), "bounded_needle\n").unwrap();
    let sparse = root.join("huge.txt");
    std::fs::write(&sparse, "bounded_needle\n").unwrap();
    std::fs::OpenOptions::new()
        .write(true)
        .open(&sparse)
        .unwrap()
        .set_len(4 * 1024 * 1024 * 1024)
        .unwrap();

    assert!(
        confined_read_limited(&root, Path::new("huge.txt"), 2 * 1024 * 1024)
            .unwrap()
            .is_none()
    );
    let result = GrepTool { root: root.clone() }
        .call(&serde_json::json!({"pattern":"bounded_needle"}))
        .unwrap();
    assert!(result.contains("visible.txt"), "{result}");
    assert!(!result.contains("huge.txt"), "{result}");
    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn confined_prefix_read_is_bounded_and_reports_binary_and_total_size() {
    let root = scratch("confined_prefix");
    std::fs::write(root.join("large.txt"), b"abcdefghij").unwrap();
    std::fs::write(root.join("binary.bin"), b"abc\0def").unwrap();

    let prefix = confined_read_prefix(&root, Path::new("large.txt"), 4).unwrap();
    assert_eq!(prefix.bytes, b"abcd");
    assert_eq!(prefix.total_bytes, 10);
    assert!(prefix.truncated);
    assert!(!prefix.binary);

    let binary = confined_read_prefix(&root, Path::new("binary.bin"), 4).unwrap();
    assert_eq!(binary.bytes, b"abc\0");
    assert_eq!(binary.total_bytes, 7);
    assert!(binary.truncated);
    assert!(binary.binary);

    #[cfg(unix)]
    {
        let outside = root.with_extension("outside.txt");
        std::fs::write(&outside, "outside").unwrap();
        std::os::unix::fs::symlink(&outside, root.join("escape.txt")).unwrap();
        assert!(
            confined_read_prefix(&root, Path::new("escape.txt"), 64).is_err(),
            "descriptor-confined preview must reject outbound symlinks"
        );
        let _ = std::fs::remove_file(outside);
    }
    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn grep_respects_git_ignores_and_filters_hidden_or_secret_files() {
    let root = scratch("grep_ignore");
    if run_git(&root, &["init", "-q"]).is_err() {
        eprintln!("git unavailable; skipping ignore-aware grep test");
        return;
    }
    std::fs::create_dir_all(root.join("src/.hidden")).unwrap();
    std::fs::write(root.join(".gitignore"), "ignored.txt\n").unwrap();
    std::fs::write(root.join("visible.txt"), "private_needle\n").unwrap();
    std::fs::write(root.join("ignored.txt"), "private_needle\n").unwrap();
    std::fs::write(root.join(".env"), "private_needle\n").unwrap();
    std::fs::write(root.join("credentials.json"), "private_needle\n").unwrap();
    std::fs::write(root.join("private.pem"), "private_needle\n").unwrap();
    std::fs::write(root.join("src/.hidden/data.txt"), "private_needle\n").unwrap();

    let result = GrepTool { root: root.clone() }
        .call(&serde_json::json!({"pattern":"private_needle"}))
        .unwrap();
    assert!(result.contains("visible.txt"), "{result}");
    for excluded in [
        "ignored.txt",
        ".env",
        "credentials.json",
        "private.pem",
        ".hidden",
    ] {
        assert!(
            !result.contains(excluded),
            "{excluded} leaked into:\n{result}"
        );
    }
    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn git_diff_shows_uncommitted_changes() {
    let root = std::env::temp_dir().join(format!("angel_gd_{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir_all(&root).unwrap();
    // Init a repo with one committed file.
    if run_git(&root, &["init", "-q"]).is_err() {
        eprintln!("git unavailable; skipping git_diff test");
        return;
    }
    std::fs::write(root.join("g.txt"), "one\n").unwrap();
    run_git(&root, &["add", "."]).unwrap();
    run_git(&root, &["commit", "-q", "-m", "init"]).unwrap();

    let mut reg = ToolRegistry::new();
    register_file_tools(&mut reg, root.clone());

    // Clean tree → no changes.
    let clean = reg.dispatch("git_diff", &serde_json::json!({})).unwrap();
    assert!(clean.contains("no uncommitted"), "got: {clean}");

    // Modify → diff shows the added line.
    std::fs::write(root.join("g.txt"), "one\ntwo\n").unwrap();
    let d = reg.dispatch("git_diff", &serde_json::json!({})).unwrap();
    assert!(d.contains("g.txt") && d.contains("+two"), "got: {d}");

    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn outline_lists_symbols_and_skips_comments() {
    let root = std::env::temp_dir().join(format!("angel_ol_{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir_all(&root).unwrap();
    let mut reg = ToolRegistry::new();
    register_file_tools(&mut reg, root.clone());
    std::fs::write(
        root.join("o.rs"),
        "pub fn alpha() {}\n\
             struct Beta;\n\
             impl Beta {\n    fn method(&self) {}\n}\n\
             // fn not_a_real_decl\n\
             const K: u32 = 1;\n",
    )
    .unwrap();
    std::fs::write(
        root.join("o.go"),
        "package sample\n\nfunc Helper() {}\n\ntype Service interface {\n\tRun()\n}\n",
    )
    .unwrap();
    std::fs::write(
        root.join("O.java"),
        "public class O {\n  public void run() {}\n  private int count;\n}\n",
    )
    .unwrap();

    let o = reg
        .dispatch("outline", &serde_json::json!({"path": "o.rs"}))
        .unwrap();
    assert!(o.contains("alpha"), "got: {o}");
    assert!(o.contains("struct Beta"), "got: {o}");
    assert!(o.contains("fn method"), "indented methods should show: {o}");
    assert!(o.contains("const K"), "got: {o}");
    assert!(
        !o.contains("not_a_real_decl"),
        "commented decls excluded: {o}"
    );
    let go = reg
        .dispatch("outline", &serde_json::json!({"path": "o.go"}))
        .unwrap();
    assert!(go.contains("package sample"), "got: {go}");
    assert!(go.contains("func Helper"), "got: {go}");
    assert!(go.contains("interface"), "got: {go}");
    let java = reg
        .dispatch("outline", &serde_json::json!({"path": "O.java"}))
        .unwrap();
    assert!(java.contains("public class O"), "got: {java}");
    assert!(java.contains("public void run"), "got: {java}");
    assert!(
        !java.contains("private int count"),
        "field without ( should not match method heuristic: {java}"
    );
    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn fuzzy_score_requires_subsequence() {
    assert!(fuzzy_score("clb", "src/club.rs").is_some()); // c,l,b ⊂ club
    assert!(fuzzy_score("hrs", "src/harness.rs").is_some()); // non-contiguous ok
    assert!(fuzzy_score("xyz", "src/club.rs").is_none());
    assert!(fuzzy_score("swarm", "src/harness.rs").is_none()); // no 'w' after 's'
}

#[test]
fn fuzzy_basename_prefix_outranks_incidental_match() {
    // "harness" is a subsequence of both, but only the first is a basename
    // prefix — it must win.
    let paths = vec![
        "src/harness.rs".to_string(),
        "docs/handler_arch_notes_rss.md".to_string(),
    ];
    let ranked = rank_paths("harness", &paths, 10);
    assert_eq!(ranked.first().unwrap(), "src/harness.rs");
}

#[test]
fn rank_paths_best_match_first_and_excludes_nonmatches() {
    let paths = vec![
        "src/harness.rs".to_string(),
        "src/swarm.rs".to_string(),
        "README.md".to_string(),
    ];
    let ranked = rank_paths("swarm", &paths, 5);
    assert_eq!(ranked, vec!["src/swarm.rs".to_string()]);
}

#[test]
fn rank_paths_limits_and_tiebreaks_shorter_first() {
    let paths = vec!["aa/x.rs".to_string(), "a/x.rs".to_string()];
    let ranked = rank_paths("xrs", &paths, 5);
    assert_eq!(ranked[0], "a/x.rs", "equal score → shorter path wins");
    let two = rank_paths(
        "rs",
        &["a.rs".to_string(), "b.rs".to_string(), "c.rs".to_string()],
        2,
    );
    assert_eq!(two.len(), 2, "limit honored");
}
