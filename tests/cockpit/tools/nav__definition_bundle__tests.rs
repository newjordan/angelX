use super::*;
use crate::agent::harness::Tool;
use crate::agent::tools::nav::DefsTool;

fn invoke(args: Value, source: &str) -> Value {
    serde_json::from_str(
        &collect_with(Path::new("/owned"), &args, &["solve".into()], |_| {
            Ok(Some(source.as_bytes().to_vec()))
        })
        .unwrap(),
    )
    .unwrap()
}

#[test]
fn definition_source_bundle_reads_each_file_once_and_binds_definition_and_reference() {
    let source = format!(
        "fn solve() {{\n    important_constraint();\n}}\n{}fn caller() {{ solve(); }}\n",
        "// spacer\n".repeat(90)
    );
    let mut reads = Vec::new();
    let raw = collect_with(
        Path::new("/owned"),
        &json!({"paths": ["src/a.rs", "src/./a.rs", "src/b.rs"]}),
        &["solve".into()],
        |path| {
            reads.push(path.to_path_buf());
            Ok(Some(source.as_bytes().to_vec()))
        },
    )
    .unwrap();
    assert_eq!(
        reads,
        [PathBuf::from("src/a.rs"), PathBuf::from("src/b.rs")]
    );
    let result: Value = serde_json::from_str(&raw).unwrap();
    let file = &result["files"][0];
    assert_eq!(file["sha256"], sha256_hex(source.as_bytes()));
    assert_eq!(file["matches"][0]["definition_lines"], 1);
    assert_eq!(file["matches"][0]["reference_lines"], 1);
    assert!(
        file["windows"][0]["source"]
            .as_str()
            .unwrap()
            .contains("important_constraint")
    );
    assert!(
        file["windows"][1]["source"]
            .as_str()
            .unwrap()
            .contains("caller() { solve(); }")
    );
    assert_eq!(result["truncated"], false);
}

#[test]
fn definition_source_bundle_validates_whole_request_before_access() {
    for args in [
        json!({"paths": ["src/a.rs", "off-limits/denied.rs"]}),
        json!({"paths": ["src/a.rs", "../escape.rs"]}),
        json!({"paths": ["src/a.rs", ".secrets/value.rs"]}),
        json!({"paths": ["src/a.rs"], "max_source_bytes": 0}),
        json!({"paths": ["src/a.rs"], "ignore_case": "yes"}),
    ] {
        let mut reads = 0;
        assert!(
            collect_with(Path::new("/owned"), &args, &["solve".into()], |_| {
                reads += 1;
                Ok(Some(Vec::new()))
            })
            .is_err()
        );
        assert_eq!(reads, 0);
    }
    let tool = DefsTool {
        root: PathBuf::from("/owned"),
    };
    assert!(
        tool.call(&json!({"name": "solve", "paths": ["a.rs"]}))
            .is_err()
    );
    assert!(
        tool.call(&json!({"name": "solve", "include_source": "true"}))
            .is_err()
    );
}

#[test]
fn definition_source_bundle_utf8_budget_and_omitted_hits_are_explicit() {
    let source = format!("fn solve() {{ {} }}\n", "🙂".repeat(1000));
    let result = invoke(
        json!({"paths": ["a.rs"], "max_source_bytes": 1024}),
        &source,
    );
    let returned = result["files"][0]["windows"][0]["source"].as_str().unwrap();
    assert!(returned.len() <= 1024 && returned.len() > 1018);
    assert_eq!(result["source_bytes"], returned.len());
    assert_eq!(result["truncated"], true);
    assert_eq!(result["files"][0]["windows"][0]["window_truncated"], true);
    assert_eq!(result["files"][0]["windows"][0]["end_line"], 1);
    let many = invoke(json!({"paths": ["a.rs"]}), &"solve();\n".repeat(100));
    assert_eq!(many["files"][0]["matches"][0]["reference_lines"], 100);
    assert_eq!(many["files"][0]["truncated"], true);
    let unicode = invoke(
        json!({"paths": ["a.rs"]}),
        "é solve();\néSolve();\nésolve();\n",
    );
    assert_eq!(unicode["files"][0]["matches"][0]["reference_lines"], 1);
}

#[test]
fn definition_source_bundle_keeps_matching_line_ahead_of_long_leading_comment() {
    let source = format!(
        "// {}\n// b\n// c\n// d\nfn solve() {{}}\n",
        "x".repeat(1200)
    );
    let result = invoke(
        json!({"paths": ["a.rs"], "max_source_bytes": 1024}),
        &source,
    );
    let window = &result["files"][0]["windows"][0];
    assert!(window["source"].as_str().unwrap().contains("fn solve() {}"));
    assert_eq!(window["anchor_line"], 5);
    assert_eq!(window["leading_context_omitted"], true);
    assert_eq!(result["truncated"], true);
}

#[test]
fn definition_source_bundle_preserves_disjoint_symbols_under_shared_budget() {
    let source = format!(
        "fn solve() {{ {} }}\n{}fn verify() {{ assert_invariant(); }}\n",
        "x".repeat(2000),
        "// gap\n".repeat(90)
    );
    let result: Value = serde_json::from_str(
        &collect_with(
            Path::new("/owned"),
            &json!({"paths": ["a.rs"], "max_source_bytes": 1024}),
            &["solve".into(), "verify".into()],
            |_| Ok(Some(source.as_bytes().to_vec())),
        )
        .unwrap(),
    )
    .unwrap();
    let windows = result["files"][0]["windows"].as_array().unwrap();
    assert_eq!(windows.len(), 2);
    assert!(
        windows[0]["source"]
            .as_str()
            .unwrap()
            .contains("fn solve()")
    );
    assert!(
        windows[1]["source"]
            .as_str()
            .unwrap()
            .contains("assert_invariant()")
    );
    assert!(result["source_bytes"].as_u64().unwrap() <= 1024);
    assert_eq!(result["truncated"], true);
}

#[test]
fn definition_source_bundle_distinguishes_unread_files_from_no_matches() {
    let result: Value = serde_json::from_str(
        &collect_with(
            Path::new("/owned"),
            &json!({"paths": ["large", "invalid", "missing", "clean"]}),
            &["solve".into()],
            |path| match path.to_str().unwrap() {
                "large" => Ok(None),
                "invalid" => Ok(Some(vec![255])),
                "missing" => Err("missing".into()),
                _ => Ok(Some(b"unrelated source".to_vec())),
            },
        )
        .unwrap(),
    )
    .unwrap();
    assert_eq!(result["unread_files"], 3);
    assert_eq!(result["files"][0]["status"], "file_limit_exceeded");
    assert_eq!(result["files"][1]["status"], "invalid_utf8");
    assert_eq!(result["files"][2]["status"], "unreadable");
    assert_eq!(result["requested_names"], json!(["solve"]));
    assert_eq!(result["files"][3]["matches"], json!([]));
}

#[cfg(unix)]
#[test]
fn definition_source_bundle_actual_tool_refuses_symlink_aliases_and_directories() {
    use std::os::unix::fs::symlink;
    struct OwnedDir(PathBuf);
    impl Drop for OwnedDir {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }
    let path = std::env::temp_dir().join(format!(
        "angel-definition-source-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    std::fs::create_dir(&path).unwrap();
    let root = OwnedDir(path);
    std::fs::create_dir(root.0.join("private")).unwrap();
    std::fs::create_dir(root.0.join(".private")).unwrap();
    std::fs::write(root.0.join(".private/a.rs"), "hidden content").unwrap();
    std::fs::write(root.0.join("private/a.rs"), "fn solve() {}\n").unwrap();
    symlink("private/a.rs", root.0.join("alias.rs")).unwrap();
    symlink("private", root.0.join("alias-dir")).unwrap();
    symlink(".private/a.rs", root.0.join("excluded-alias.rs")).unwrap();
    let tool = DefsTool {
        root: root.0.clone(),
    };
    let result: Value = serde_json::from_str(
        &tool
            .call(&json!({
                "name": "solve", "include_source": true,
            "paths": ["alias.rs", "alias-dir/a.rs", "private", "private/a.rs", "excluded-alias.rs"]
            }))
            .unwrap(),
    )
    .unwrap();
    assert_eq!(result["unread_files"], 4);
    assert_eq!(result["files"][3]["status"], "read");
    assert_eq!(
        result["files"][3]["windows"][0]["source"],
        "fn solve() {}\n"
    );
}
