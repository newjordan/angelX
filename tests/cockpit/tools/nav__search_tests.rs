use super::*;
use std::sync::atomic::{AtomicUsize, Ordering};

pub(super) struct TestWorkspace(pub(super) PathBuf);

impl TestWorkspace {
    pub(super) fn new(label: &str) -> Self {
        static NEXT: AtomicUsize = AtomicUsize::new(0);
        let root = std::env::temp_dir().join(format!(
            "angel-nav-{label}-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(&root).unwrap();
        Self(root)
    }

    pub(super) fn write(&self, relative: &str, content: &str) {
        let path = self.0.join(relative);
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).unwrap();
        }
        std::fs::write(path, content).unwrap();
    }
}

impl Drop for TestWorkspace {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

#[test]
fn planted_cohort_tokens_are_found_in_an_ignored_nested_workspace() {
    let _guard = crate::tests::env_lock();
    let workspace = TestWorkspace::new("cohort-parent");
    workspace.write(".gitignore", "work/\n");
    assert!(
        std::process::Command::new("git")
            .args(["init", "--quiet"])
            .arg(&workspace.0)
            .status()
            .unwrap()
            .success()
    );
    let fixtures = workspace.0.join("work");
    let script = Path::new(env!("CARGO_MANIFEST_DIR")).join("../scripts/repo-search-cohort.py");
    assert!(
        std::process::Command::new("python3")
            .arg(script)
            .args(["--fixtures-only", "--fixture-dir"])
            .arg(&fixtures)
            .status()
            .unwrap()
            .success()
    );
    let manifest: Value =
        serde_json::from_slice(&std::fs::read(fixtures.join("manifest.json")).unwrap()).unwrap();
    assert!(manifest["large"]["file_count"].as_u64().unwrap() >= 5000);
    // Reproduce the old empty Git name source independently of the tool.
    assert_eq!(
        run_git(&fixtures.join("large"), &git_listing_args(None)).unwrap(),
        ""
    );
    let mut visible = 0;
    let mut ignored = 0;
    for pack in manifest.as_object().unwrap().values() {
        let root = PathBuf::from(pack["root"].as_str().unwrap());
        let grep = GrepTool { root: root.clone() };
        for target in pack["targets"].as_array().unwrap() {
            let path = target["path"].as_str().unwrap();
            let pattern = regex::escape(target["identifier"].as_str().unwrap());
            if target["escaping"].as_bool().unwrap() {
                for no_ignore in [false, true] {
                    assert!(
                        grep.call(&serde_json::json!({"pattern": pattern, "path": path,
                            "no_ignore": no_ignore, "hidden": true}))
                            .is_err()
                    );
                }
                continue;
            }
            let output = grep.call(&serde_json::json!({"pattern": pattern})).unwrap();
            if target["ignored"].as_bool().unwrap() {
                assert_eq!(output, "no matches", "{path}");
                let output = grep
                    .call(&serde_json::json!({"pattern": pattern, "no_ignore": true}))
                    .unwrap();
                assert!(grep_hits(&output).contains(path), "{path}: {output}");
                let ff = FindFilesTool { root: root.clone() };
                let fs = FileSearchTool { root: root.clone() };
                let ld = ListDirTool { root: root.clone() };
                assert!(
                    ff.call(&serde_json::json!({"pattern": path}))
                        .unwrap()
                        .starts_with("no files")
                );
                assert_eq!(
                    ff.call(&serde_json::json!({"pattern": path, "no_ignore": true}))
                        .unwrap(),
                    path
                );
                let name = Path::new(path).file_name().unwrap().to_str().unwrap();
                assert!(
                    fs.call(&serde_json::json!({"query": name}))
                        .unwrap()
                        .starts_with("no files")
                );
                assert!(
                    fs.call(&serde_json::json!({"query": name, "no_ignore": true}))
                        .unwrap()
                        .contains(path)
                );
                let parent = Path::new(path).parent().unwrap().to_str().unwrap();
                assert!(ld.call(&serde_json::json!({"path": parent})).is_err());
                assert!(
                    ld.call(&serde_json::json!({"path": parent, "no_ignore": true}))
                        .unwrap()
                        .contains(name)
                );
                ignored += 1;
            } else {
                assert!(
                    output
                        .lines()
                        .take(5)
                        .any(|line| line.starts_with(&format!("{path}:"))),
                    "{path}: {output}"
                );
                visible += 1;
            }
        }
    }
    // Greater than the cohort's two directory levels, with 5,012 peers.
    let root = fixtures.join("large");
    std::fs::create_dir_all(root.join("deep/three/levels")).unwrap();
    std::fs::write(
        root.join("deep/three/levels/token.txt"),
        "literal[a].b+ token\n",
    )
    .unwrap();
    let tool = GrepTool { root };
    let output = tool
        .call(&serde_json::json!({"pattern": r"literal\[a\]\.b\+"}))
        .unwrap();
    assert!(output.contains("deep/three/levels/token.txt:1:"));
    assert_eq!(
        tool.call(&serde_json::json!({"pattern": "literal[a].b+"}))
            .unwrap(),
        "no matches"
    );
    assert!(tool.call(&serde_json::json!({"pattern": "["})).is_err());
    assert_eq!((visible, ignored), (63, 20));
    println!(
        "direct cohort: visible grep recall@5 {visible}/{visible}; ignored default 0/{ignored}; ignored no_ignore {ignored}/{ignored}; escaping scope refused in both modes"
    );
}

#[test]
fn discovery_flags_are_independent_and_keep_sensitive_paths_excluded() {
    let _guard = crate::tests::env_lock();
    let workspace = TestWorkspace::new("options");
    for path in [
        "visible.txt",
        ".hidden/token.txt",
        "build/token.txt",
        "node_modules/token.txt",
        ".cache/token.txt",
        ".env",
        ".env.local",
        "credentials.json",
        "private.key",
    ] {
        workspace.write(path, "needle\n");
    }
    workspace.write(".gitignore", "build/\n.cache/\n*.log\n");
    workspace.write("visible.log", "needle\n");
    for hidden in [false, true] {
        for no_ignore in [false, true] {
            let args =
                serde_json::json!({"pattern": "needle", "hidden": hidden, "no_ignore": no_ignore});
            let output = GrepTool {
                root: workspace.0.clone(),
            }
            .call(&args)
            .unwrap();
            let hits = grep_hits(&output);
            assert!(hits.contains("visible.txt"));
            assert_eq!(hits.contains(".hidden/token.txt"), hidden);
            assert_eq!(hits.contains(".cache/token.txt"), hidden && no_ignore);
            assert_eq!(hits.contains("build/token.txt"), no_ignore);
            assert_eq!(hits.contains("node_modules/token.txt"), no_ignore);
            assert_eq!(hits.contains("visible.log"), no_ignore);
            for path in [".env", ".env.local", "credentials.json", "private.key"] {
                assert!(!hits.contains(path));
            }
            let ff = FindFilesTool {
                root: workspace.0.clone(),
            };
            let fs = FileSearchTool {
                root: workspace.0.clone(),
            };
            let ld = ListDirTool {
                root: workspace.0.clone(),
            };
            for (path, expected) in [
                (".hidden/token.txt", hidden),
                (".cache/token.txt", hidden && no_ignore),
                ("build/token.txt", no_ignore),
                ("node_modules/token.txt", no_ignore),
            ] {
                let query = serde_json::json!({"pattern": path, "query": path, "hidden": hidden, "no_ignore": no_ignore});
                assert_eq!(
                    ff.call(&query).unwrap().contains(path)
                        && !ff.call(&query).unwrap().starts_with("no files"),
                    expected
                );
                assert_eq!(
                    fs.call(&query).unwrap().lines().any(|line| line == path),
                    expected
                );
            }
            let listing = ld
                .call(&serde_json::json!({"hidden": hidden, "no_ignore": no_ignore}))
                .unwrap();
            assert_eq!(listing.lines().any(|line| line == "build/"), no_ignore);
            assert_eq!(listing.lines().any(|line| line == ".hidden/"), hidden);
            for tool in [&ff as &dyn Tool, &fs, &ld] {
                assert_eq!(
                    tool.def().params["properties"]["no_ignore"]["type"],
                    "boolean"
                );
                assert_eq!(tool.def().params["properties"]["hidden"]["type"], "boolean");
            }
        }
    }
    let grep = GrepTool {
        root: workspace.0.clone(),
    };
    assert_eq!(grep.def().params["properties"]["hidden"]["type"], "boolean");
    assert!(
        grep.call(&serde_json::json!({"pattern":"needle", "hidden":"true"}))
            .is_err()
    );
    assert!(
        grep.call(&serde_json::json!({"pattern":"needle", "no_ignore":1}))
            .is_err()
    );
}

fn grep_hits(output: &str) -> BTreeSet<String> {
    output
        .lines()
        .filter(|line| !line.starts_with('[') && *line != "--" && *line != "no matches")
        .filter_map(|line| line.split(':').next().map(str::to_string))
        .collect()
}

#[test]
fn git_listing_parser_rejects_capped_or_partial_output() {
    assert_eq!(
        parse_git_file_listing("src/a.rs\0src/b.rs\0").unwrap(),
        vec![PathBuf::from("src/a.rs"), PathBuf::from("src/b.rs")]
    );
    assert!(parse_git_file_listing("src/a.rs").is_none());
    assert!(parse_git_file_listing(&"x".repeat(GIT_OUTPUT_CAP_BYTES)).is_none());
}

#[test]
fn git_listing_pathspec_stays_narrow_when_search_is_scoped() {
    assert_eq!(
        git_listing_args(Some("src/harness")),
        vec![
            "ls-files",
            "--cached",
            "--others",
            "--exclude-standard",
            "-z",
            "--",
            "src/harness",
        ]
    );
    assert_eq!(
        git_listing_args(None),
        vec![
            "ls-files",
            "--cached",
            "--others",
            "--exclude-standard",
            "-z"
        ]
    );
}

#[test]
fn repository_search_policy_excludes_quarantined_directories() {
    assert!(!search_path_allowed(Path::new(
        "off-limits/legacy-product/source.rs"
    )));
    assert!(!search_path_allowed(Path::new("src/target/generated.rs")));
    assert!(!search_path_allowed(Path::new("src/.private/secret.rs")));
    assert!(search_path_allowed(Path::new("src/harness/turn.rs")));
}

#[test]
fn grep_diversity_keeps_a_hot_first_file_from_hiding_later_files() {
    let workspace = TestWorkspace::new("diverse");
    workspace.write("a-hot.txt", &"needle\n".repeat(1000));
    workspace.write("z-unique.txt", "one unique needle\n");

    let output = grep_confined_many(
        &workspace.0,
        &[PathBuf::from(".")],
        &BTreeSet::new(),
        None,
        "needle",
        false,
        true,
        0,
    )
    .unwrap();

    assert!(output.contains("z-unique.txt:1:one unique needle"));
    assert_eq!(
        output
            .lines()
            .filter(|line| line.starts_with("a-hot.txt:"))
            .count(),
        GREP_MAX_MATCHES_PER_FILE
    );
    assert!(output.contains("per-file caps omitted"));
    assert!(output.contains("a-hot.txt"));
}

#[test]
fn grep_continuation_is_deterministic_and_has_no_duplicate_files() {
    let workspace = TestWorkspace::new("paging");
    for index in 0..30 {
        workspace.write(
            &format!("file-{index:02}.txt"),
            &format!("needle {index}\n").repeat(10),
        );
    }
    let first = grep_confined_many(
        &workspace.0,
        &[PathBuf::from(".")],
        &BTreeSet::new(),
        None,
        "needle",
        false,
        true,
        0,
    )
    .unwrap();
    assert!(first.contains("grep continuation"));
    let first_files = grep_hits(&first);
    let cursor = first_files.iter().next_back().unwrap().clone();

    let second = grep_confined_many(
        &workspace.0,
        &[PathBuf::from(".")],
        &BTreeSet::new(),
        Some(Path::new(&cursor)),
        "needle",
        false,
        true,
        0,
    )
    .unwrap();
    let second_repeat = grep_confined_many(
        &workspace.0,
        &[PathBuf::from(".")],
        &BTreeSet::new(),
        Some(Path::new(&cursor)),
        "needle",
        false,
        true,
        0,
    )
    .unwrap();
    assert_eq!(second, second_repeat);
    let second_files = grep_hits(&second);
    assert!(first_files.is_disjoint(&second_files));
    assert_eq!(first_files.len() + second_files.len(), 30);
}

#[test]
fn grep_multi_path_and_skip_inputs_stay_confined() {
    let workspace = TestWorkspace::new("multipath");
    workspace.write("one/a.txt", "needle one\n");
    workspace.write("two/b.txt", "needle two\n");
    workspace.write("three/c.txt", "needle three\n");
    let skipped = BTreeSet::from([PathBuf::from("two/b.txt")]);
    let output = grep_confined_many(
        &workspace.0,
        &[
            PathBuf::from("one"),
            PathBuf::from("two"),
            PathBuf::from("three"),
        ],
        &skipped,
        None,
        "needle",
        false,
        true,
        0,
    )
    .unwrap();
    assert!(output.contains("one/a.txt:1:needle one"));
    assert!(!output.contains("two/b.txt:1:"));
    assert!(output.contains("three/c.txt:1:needle three"));
    assert!(output.contains("skipped 1 requested file"));
    assert!(output.contains("two/b.txt"));

    // Both `path` and `paths` supplied: one merged, deduplicated search.
    let merged = GrepTool {
        root: workspace.0.clone(),
    }
    .call(&serde_json::json!({
        "pattern": "needle",
        "path": "one",
        "paths": ["one", "three"],
        "context": 0,
    }))
    .unwrap();
    assert!(merged.contains("one/a.txt:1:needle one"), "{merged}");
    assert!(merged.contains("three/c.txt:1:needle three"), "{merged}");
    assert!(!merged.contains("two/b.txt"), "{merged}");

    assert!(
        grep_confined_many(
            &workspace.0,
            &[PathBuf::from("../escape")],
            &BTreeSet::new(),
            None,
            "needle",
            false,
            true,
            0,
        )
        .is_err()
    );
    assert!(
        grep_skip_files(
            &workspace.0,
            Some(&serde_json::json!(["../escape"])),
            SearchOptions::default()
        )
        .is_err()
    );
    assert_eq!(
        grep_start_paths(&serde_json::json!({"path":"one", "paths":["two"]})).unwrap(),
        vec![PathBuf::from("one"), PathBuf::from("two")]
    );
    assert_eq!(
        grep_start_paths(&serde_json::json!({"path":"one", "paths":["one", "two", "one"]}))
            .unwrap(),
        vec![PathBuf::from("one"), PathBuf::from("two")]
    );
    assert!(
        grep_start_paths(&serde_json::json!({
            "path": "one",
            "paths": ["a", "b", "c", "d", "e", "f", "g", "h"]
        }))
        .is_err()
    );
    assert_eq!(
        grep_start_paths(&serde_json::json!({"path":"", "paths":["two"]})).unwrap(),
        vec![PathBuf::from("two")]
    );
    assert_eq!(
        grep_start_paths(&serde_json::json!({"path":"one", "paths":[]})).unwrap(),
        vec![PathBuf::from("one")]
    );
    assert_eq!(
        grep_start_paths(&serde_json::json!({"path":null, "paths":["two"]})).unwrap(),
        vec![PathBuf::from("two")]
    );
    assert_eq!(
        grep_start_paths(&serde_json::json!({"path":"", "paths":[]})).unwrap(),
        vec![PathBuf::from(".")]
    );
}

#[test]
fn grep_exact_empty_continuation_placeholder_is_absent() {
    let workspace = TestWorkspace::new("empty-continuation");
    workspace.write("src/example.rs", "needle\n");
    let tool = GrepTool {
        root: workspace.0.clone(),
    };

    for after_file in [Value::Null, Value::String(String::new())] {
        let output = tool
            .call(&serde_json::json!({
                "pattern": "needle",
                "after_file": after_file,
                "context": 0,
            }))
            .unwrap();
        assert!(output.contains("src/example.rs:1:needle"));
    }

    assert!(
        tool.call(&serde_json::json!({"pattern":"needle", "after_file":" "}))
            .is_err()
    );
    assert!(
        tool.call(&serde_json::json!({"pattern":"needle", "after_file":[]}))
            .is_err()
    );
}

#[test]
fn grep_output_is_line_byte_and_utf8_bounded() {
    let workspace = TestWorkspace::new("bounded");
    let long_match = format!("needle {}\n", "λ".repeat(1800));
    for index in 0..12 {
        workspace.write(
            &format!("long-{index:02}.txt"),
            &long_match.repeat(GREP_MAX_MATCHES_PER_FILE),
        );
    }
    let output = grep_confined_many(
        &workspace.0,
        &[PathBuf::from(".")],
        &BTreeSet::new(),
        None,
        "needle",
        false,
        true,
        0,
    )
    .unwrap();
    assert!(output.len() <= GREP_MAX_OUTPUT_BYTES);
    assert!(output.lines().count() <= GREP_MAX_LINES);
    assert!(output.contains("line truncated"));
    assert!(output.contains("grep continuation"));
    assert!(std::str::from_utf8(output.as_bytes()).is_ok());
}

#[test]
fn definition_batch_builds_one_regex_and_validates_arguments() {
    let regex = regex::RegexBuilder::new(&defs_regex_many(&["NeedleWidget", "run"]))
        .case_insensitive(true)
        .build()
        .unwrap();
    assert!(regex.is_match("pub struct NeedleWidget {"));
    assert!(regex.is_match("func run() {"));
    assert!(!regex.is_match("let run = widget.run();"));

    assert_eq!(
        definition_names(&serde_json::json!({"names":["NeedleWidget", "run", "run"]})).unwrap(),
        vec!["NeedleWidget", "run"]
    );
    assert!(definition_names(&serde_json::json!({"name":"run", "names":["run"]})).is_err());
    assert_eq!(
        definition_names(&serde_json::json!({"name":"", "names":["run"]})).unwrap(),
        vec!["run"]
    );
    assert_eq!(
        definition_names(&serde_json::json!({"name":"run", "names":[]})).unwrap(),
        vec!["run"]
    );
    assert!(definition_names(&serde_json::json!({"names":[]})).is_err());
    assert!(definition_names(&serde_json::json!({"name":"run-now"})).is_err());
}

#[test]
fn nav_tools_refuse_outside_escape_nul_and_overlong_targets() {
    let ws = TestWorkspace::new("confine");
    ws.write("src/lib.rs", "pub fn main() {}\n");
    let root = ws.0.clone();
    let over = "x".repeat(4000) + ".txt";
    let nul = "bad\0name.txt";
    let file_search = FileSearchTool { root: root.clone() };
    let defs = DefsTool { root: root.clone() };
    let outline = OutlineTool { root: root.clone() };
    let grep = GrepTool { root: root.clone() };
    for (tool, args) in [
        (
            "file_search",
            file_search.call(&serde_json::json!({"query": "/etc/passwd"})),
        ),
        (
            "file_search_dotdot",
            file_search.call(&serde_json::json!({"query": "../outside/secret.txt"})),
        ),
        (
            "file_search_nul",
            file_search.call(&serde_json::json!({"query": nul})),
        ),
        (
            "file_search_overlong",
            file_search.call(&serde_json::json!({"query": over})),
        ),
        (
            "defs",
            defs.call(&serde_json::json!({"name": "main", "path": "../outside/secret.txt"})),
        ),
        (
            "outline",
            outline.call(&serde_json::json!({"path": "/etc/passwd"})),
        ),
        (
            "grep",
            grep.call(&serde_json::json!({"pattern": "main", "path": "../outside/secret.txt"})),
        ),
    ] {
        let err = args.expect_err(tool);
        assert!(
            err.contains("outside the workspace") || err.contains("escapes the workspace"),
            "{tool}: {err}"
        );
    }
    let scoped = defs
        .call(&serde_json::json!({"name": "main", "path": "src/lib.rs"}))
        .unwrap();
    assert!(scoped.contains("src/lib.rs"), "{scoped}");
    let listed = file_search
        .call(&serde_json::json!({"query": "lib.rs"}))
        .unwrap();
    assert!(listed.contains("src/lib.rs"), "{listed}");
}
