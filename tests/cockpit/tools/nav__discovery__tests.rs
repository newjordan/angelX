use super::super::search_tests::TestWorkspace;
use super::*;

#[test]
fn list_lean_schema_preserves_ranking_and_paging_controls() {
    let full = ListDirTool {
        root: PathBuf::from("."),
    }
    .def()
    .params;
    let lean = crate::agent::harness::lean_list_dir_params();
    for key in [
        "path",
        "hint",
        "pattern",
        "offset",
        "limit",
        "no_ignore",
        "hidden",
    ] {
        assert_eq!(
            full["properties"][key]["type"],
            lean["properties"][key]["type"]
        );
    }
    assert_eq!(
        full["properties"]["limit"]["maximum"],
        lean["properties"]["limit"]["maximum"]
    );
}

#[test]
fn list_late_hint_is_ranked_before_entry_window() {
    let mut entries = (0..900).map(|i| format!("a{i:04}.rs")).collect::<Vec<_>>();
    entries.push("z_target.rs".into());
    let plain = list_window(entries.clone(), ".", &serde_json::json!({})).unwrap();
    assert!(plain.contains("shown=700, omitted_before=0, omitted_after=201"));
    assert!(plain.contains("offset=700, limit=700"));
    let hinted = list_window(entries, ".", &serde_json::json!({"hint":"z_target.rs"})).unwrap();
    assert_eq!(hinted.lines().next(), Some("z_target.rs"));
    assert!(hinted.contains("shown=700, omitted_before=0, omitted_after=201"));
}

#[test]
fn list_ordering_and_paging_preserve_complete_membership() {
    let entries = [
        "aaa.txt",
        "zebra.py",
        "lib/",
        "zebra_copy.py",
        "zebra/",
        "README.md",
    ]
    .map(str::to_string)
    .to_vec();
    let plain = list_window(entries.clone(), ".", &serde_json::json!({})).unwrap();
    assert_eq!(
        plain,
        "lib/\nzebra/\nREADME.md\nzebra.py\nzebra_copy.py\naaa.txt"
    );
    let hinted = list_window(
        entries.clone(),
        ".",
        &serde_json::json!({"hint":"ZEBRA.PY"}),
    )
    .unwrap();
    assert_eq!(hinted.lines().next(), Some("zebra.py"));
    let globbed = list_window(
        entries.clone(),
        ".",
        &serde_json::json!({"pattern":"*.txt"}),
    )
    .unwrap();
    assert_eq!(globbed.lines().next(), Some("aaa.txt"));
    let page = list_window(
        entries.clone(),
        ".",
        &serde_json::json!({"hint":"zebra.py", "limit":2}),
    )
    .unwrap();
    assert!(page.contains("total=6, shown=2, omitted_before=0, omitted_after=4"));
    assert!(page.contains("offset=2, limit=2"));
    let mut collected = Vec::new();
    for offset in [0, 2, 4] {
        let page = list_window(
            entries.clone(),
            ".",
            &serde_json::json!({"hint":"zebra.py", "limit":2, "offset":offset}),
        )
        .unwrap();
        collected.extend(
            page.lines()
                .filter(|s| !s.starts_with('['))
                .map(str::to_string),
        );
    }
    assert_eq!(collected.join("\n"), hinted);
    let past = list_window(entries, ".", &serde_json::json!({"offset":99})).unwrap();
    assert!(past.contains("shown=0, omitted_before=6, omitted_after=0"));
    assert!(past.contains("end of listing"));
}

#[test]
fn list_window_reports_byte_cut_and_rejects_invalid_arguments() {
    let entries = (0..300)
        .map(|i| format!("{i:04}_{}.rs", "é".repeat(100)))
        .collect::<Vec<_>>();
    let first = list_window(entries.clone(), ".", &serde_json::json!({})).unwrap();
    let count = first.lines().filter(|s| !s.starts_with('[')).count();
    assert!(count > 0 && count < entries.len());
    assert!(first.contains(&format!("omitted_after={}", entries.len() - count)));
    assert!(first.contains(&format!("offset={count}, limit=700")));
    assert!(first.contains("byte_limit=24000"));
    let second = list_window(entries.clone(), ".", &serde_json::json!({"offset":count})).unwrap();
    assert_eq!(second.lines().next(), Some(entries[count].as_str()));
    for args in [
        serde_json::json!({"limit":0}),
        serde_json::json!({"limit":701}),
        serde_json::json!({"offset":-1}),
        serde_json::json!({"hint":true}),
        serde_json::json!({"pattern":3}),
    ] {
        assert!(list_window(entries.clone(), ".", &args).is_err());
    }
}

#[test]
fn list_hint_does_not_override_ignore_or_shallow_scope() {
    let _guard = crate::tests::env_lock();
    let workspace = TestWorkspace::new("discovery-hint-policy");
    workspace.write(".gitignore", "secret-output.py\n");
    workspace.write("secret-output.py", "ignored");
    workspace.write("nested/deep.py", "deep");
    workspace.write("visible.py", "visible");
    let tool = ListDirTool {
        root: workspace.0.clone(),
    };
    let listed = tool
        .call(&serde_json::json!({"hint":"secret-output.py"}))
        .unwrap();
    assert!(!listed.contains("secret-output.py"));
    assert!(!listed.contains("deep.py"));
    let included = tool
        .call(&serde_json::json!({"hint":"secret-output.py", "no_ignore":true}))
        .unwrap();
    assert_eq!(included.lines().next(), Some("secret-output.py"));
}

#[test]
fn list_dir_is_shallow_and_keeps_late_entries_in_large_directories() {
    let _guard = crate::tests::env_lock();
    let workspace = TestWorkspace::new("discovery-list-large");
    for i in 0..600 {
        workspace.write(&format!("wide/f{i:04}.txt"), "filler");
    }
    workspace.write("wide/zz_target.txt", "needle");
    workspace.write("wide/nested/deeper/target.txt", "deep needle");
    let listed = ListDirTool {
        root: workspace.0.clone(),
    }
    .call(&serde_json::json!({"path": "wide"}))
    .unwrap();
    let entries = listed.lines().collect::<Vec<_>>();
    assert_eq!(entries.len(), 602);
    assert_eq!(entries.last(), Some(&"zz_target.txt"));
    assert!(entries.contains(&"nested/"));
    assert!(!listed.contains("deeper"));
    let nested = ListDirTool {
        root: workspace.0.clone(),
    }
    .call(&serde_json::json!({"path": "wide/nested"}))
    .unwrap();
    assert_eq!(nested, "deeper/");
    let files = walk(
        &workspace.0,
        Path::new("wide"),
        700,
        SearchOptions::default(),
    )
    .unwrap();
    assert!(files.contains(&PathBuf::from("wide/nested/deeper/target.txt")));
    let capped = walk(
        &workspace.0,
        Path::new("wide"),
        10,
        SearchOptions::default(),
    )
    .unwrap();
    assert_eq!(capped.len(), 10);
    assert!(!capped.contains(&PathBuf::from("wide/zz_target.txt")));
}

#[cfg(unix)]
#[test]
fn list_dir_can_scope_to_an_in_tree_symlink_directory() {
    let _guard = crate::tests::env_lock();
    let workspace = TestWorkspace::new("discovery-directory-link");
    workspace.write("src/nested/token.txt", "needle");
    std::os::unix::fs::symlink("src", workspace.0.join("alias")).unwrap();
    let tool = ListDirTool {
        root: workspace.0.clone(),
    };
    assert_eq!(
        tool.call(&serde_json::json!({"path": "alias"})).unwrap(),
        "nested/"
    );
    assert_eq!(
        tool.call(&serde_json::json!({"path": "alias/nested"}))
            .unwrap(),
        "token.txt"
    );
}

#[test]
fn ignored_directories_require_no_ignore_even_with_explicit_scope() {
    let _guard = crate::tests::env_lock();
    let workspace = TestWorkspace::new("discovery-ignored");
    workspace.write(".gitignore", "build/\n");
    for dir in ["build/out", "node_modules/pkg", "target/debug"] {
        let path = format!("{dir}/token.txt");
        workspace.write(&path, "needle\n");
        for no_ignore in [false, true] {
            let options = SearchOptions {
                no_ignore,
                hidden: false,
            };
            let files = walk(&workspace.0, Path::new(""), 100, options).unwrap();
            assert_eq!(files.contains(&PathBuf::from(&path)), no_ignore, "{path}");
            let scoped = walk(&workspace.0, Path::new(dir), 100, options);
            if no_ignore {
                assert_eq!(scoped.unwrap(), vec![PathBuf::from(&path)]);
            } else {
                assert!(scoped.is_err());
            }
            let found = FindFilesTool {
                root: workspace.0.clone(),
            }
            .call(&serde_json::json!({"pattern": path, "no_ignore": no_ignore}))
            .unwrap();
            assert_eq!(found.lines().any(|line| line == path), no_ignore);
            let listed = ListDirTool {
                root: workspace.0.clone(),
            }
            .call(&serde_json::json!({"path": dir, "no_ignore": no_ignore}));
            if no_ignore {
                assert_eq!(listed.unwrap(), "token.txt");
            } else {
                assert!(listed.is_err());
            }
        }
    }
}

#[test]
fn hidden_files_and_directories_require_hidden_independently_of_no_ignore() {
    let _guard = crate::tests::env_lock();
    let workspace = TestWorkspace::new("discovery-hidden");
    workspace.write(".note.txt", "needle\n");
    workspace.write(".notes/token.txt", "needle\n");
    workspace.write(".cache/token.txt", "needle\n");
    workspace.write(".gitignore", ".cache/\n");
    for hidden in [false, true] {
        for no_ignore in [false, true] {
            let files = walk(
                &workspace.0,
                Path::new(""),
                100,
                SearchOptions { no_ignore, hidden },
            )
            .unwrap();
            assert_eq!(files.contains(&PathBuf::from(".note.txt")), hidden);
            assert_eq!(files.contains(&PathBuf::from(".notes/token.txt")), hidden);
            assert_eq!(
                files.contains(&PathBuf::from(".cache/token.txt")),
                hidden && no_ignore
            );
            let listed = ListDirTool {
                root: workspace.0.clone(),
            }
            .call(&serde_json::json!({"hidden": hidden, "no_ignore": no_ignore}))
            .unwrap();
            assert_eq!(listed.lines().any(|line| line == ".note.txt"), hidden);
            assert_eq!(listed.lines().any(|line| line == ".notes/"), hidden);
            assert_eq!(
                listed.lines().any(|line| line == ".cache/"),
                hidden && no_ignore
            );
        }
    }
}

#[cfg(unix)]
#[test]
fn in_tree_file_symlink_is_discovered_and_followed() {
    let _guard = crate::tests::env_lock();
    let workspace = TestWorkspace::new("discovery-in-tree");
    workspace.write("src/token.txt", "in_tree_marker\n");
    std::fs::create_dir(workspace.0.join("links")).unwrap();
    std::os::unix::fs::symlink("../src/token.txt", workspace.0.join("links/alias.txt")).unwrap();
    for no_ignore in [false, true] {
        let files = walk(
            &workspace.0,
            Path::new(""),
            100,
            SearchOptions {
                no_ignore,
                hidden: false,
            },
        )
        .unwrap();
        assert!(files.contains(&PathBuf::from("links/alias.txt")));
        assert_eq!(
            FindFilesTool {
                root: workspace.0.clone()
            }
            .call(&serde_json::json!({"pattern": "links/alias.txt", "no_ignore": no_ignore}))
            .unwrap(),
            "links/alias.txt"
        );
        assert_eq!(
            ListDirTool {
                root: workspace.0.clone()
            }
            .call(&serde_json::json!({"path": "links", "no_ignore": no_ignore}))
            .unwrap(),
            "alias.txt"
        );
        let hits = GrepTool {
            root: workspace.0.clone(),
        }
        .call(&serde_json::json!({"pattern": "in_tree_marker", "no_ignore": no_ignore}))
        .unwrap();
        assert!(
            hits.lines()
                .any(|line| line.starts_with("links/alias.txt:1:")),
            "{hits}"
        );
    }
}

#[cfg(unix)]
#[test]
fn escaping_symlink_cannot_be_read_or_used_as_search_scope() {
    let _guard = crate::tests::env_lock();
    let workspace = TestWorkspace::new("discovery-escape");
    let outside = TestWorkspace::new("discovery-outside");
    outside.write("token.txt", "outside_marker\n");
    workspace.write("visible.txt", "inside_marker\n");
    std::os::unix::fs::symlink(outside.0.join("token.txt"), workspace.0.join("escape.txt"))
        .unwrap();
    std::os::unix::fs::symlink(&outside.0, workspace.0.join("escape-dir")).unwrap();
    for hidden in [false, true] {
        for no_ignore in [false, true] {
            let options = SearchOptions { no_ignore, hidden };
            assert!(confined_read(&workspace.0, Path::new("escape.txt")).is_err());
            assert!(walk(&workspace.0, Path::new("escape-dir"), 100, options).is_err());
            assert!(ListDirTool { root: workspace.0.clone() }
                    .call(&serde_json::json!({"path": "escape-dir", "no_ignore": no_ignore, "hidden": hidden})).is_err());
            let grep = GrepTool {
                root: workspace.0.clone(),
            };
            assert_eq!(grep.call(&serde_json::json!({"pattern": "outside_marker", "no_ignore": no_ignore, "hidden": hidden})).unwrap(), "no matches");
            for path in ["escape.txt", "escape-dir"] {
                assert!(grep.call(&serde_json::json!({"pattern": "outside_marker", "path": path, "no_ignore": no_ignore, "hidden": hidden})).is_err());
            }
        }
    }
}

#[test]
fn grep_keeps_workspace_local_scope_inside_ignored_ancestor_repository() {
    let _guard = crate::tests::env_lock();
    let parent = TestWorkspace::new("discovery-grep-parent");
    parent.write(".gitignore", "work/\n");
    parent.write("work/src/token.txt", "scope_marker\n");
    parent.write("work/build/token.txt", "scope_marker\n");
    parent.write("work/.gitignore", "build/\n");
    assert!(
        std::process::Command::new("git")
            .args(["init", "--quiet"])
            .arg(&parent.0)
            .status()
            .unwrap()
            .success()
    );
    let root = parent.0.join("work");
    assert_eq!(run_git(&root, &git_listing_args(None)).unwrap(), "");
    assert!(!workspace_is_git_root(&root));
    let grep = GrepTool { root };
    for path in [".", "src", "src/token.txt"] {
        let hits = grep
            .call(&serde_json::json!({"pattern": "scope_marker", "path": path}))
            .unwrap();
        assert!(
            hits.lines()
                .any(|line| line.starts_with("src/token.txt:1:")),
            "{hits}"
        );
        assert!(!hits.contains("build/token.txt"));
    }
    let hits = grep
        .call(&serde_json::json!({"pattern": "scope_marker", "no_ignore": true}))
        .unwrap();
    assert!(
        hits.lines()
            .any(|line| line.starts_with("build/token.txt:1:")),
        "{hits}"
    );
}

#[test]
fn ignore_patterns_cover_anchoring_negation_and_escaping() {
    for (pattern, path, expected) in [
        ("*.log", "a/b/output.log", true),
        ("/build", "nested/build", false),
        ("/build", "build", true),
        ("a/**/b", "a/b", true),
        ("a/**/b", "a/x/y/b", true),
        ("a/*/b", "a/x/y/b", false),
        (r"\#file", "#file", true),
        (r"\!file", "!file", true),
        ("file[0-9].txt", "file3.txt", true),
        ("file[!0-9].txt", "file3.txt", false),
        (r"with\ space\ ", "with space ", true),
    ] {
        assert_eq!(
            rule(pattern).unwrap().regex.is_match(path),
            expected,
            "{pattern}: {path}"
        );
    }
    assert!(rule("#comment").is_none());
    assert!(rule("!keep.log").unwrap().negate);
    assert!(rule("build/").unwrap().directory);
}

#[test]
fn nested_ignore_negation_and_excluded_parents_agree() {
    let workspace = super::super::search_tests::TestWorkspace::new("nested-ignore");
    workspace.write(
        ".gitignore",
        "*.log\n!keep.log\nblocked/\n!blocked/keep.txt\n",
    );
    workspace.write("src/.gitignore", "!nested.log\n");
    let policy = Policy::new(&workspace.0, SearchOptions::default());
    for (path, allowed) in [
        ("a.log", false),
        ("keep.log", true),
        ("src/nested.log", true),
        ("blocked/keep.txt", false),
    ] {
        assert_eq!(
            policy.allowed(Path::new(path), false).unwrap(),
            allowed,
            "{path}"
        );
    }
}
