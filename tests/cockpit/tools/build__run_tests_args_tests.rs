use super::*;
use crate::platform::workspace_lang;
use serde_json::json;

fn scratch(name: &str) -> PathBuf {
    let root = std::env::temp_dir().join(format!(
        "angel-run-tests-args-{name}-{}",
        std::process::id()
    ));
    std::fs::create_dir_all(root.join("react")).unwrap();
    root
}

#[test]
fn runners_written_as_commands_reach_their_language_and_entrypoint() {
    let ws = std::env::temp_dir();
    for (runner, lang, entrypoint) in [
        ("npm test", "js", None),
        ("npm run test", "js", None),
        ("npx jest", "js", None),
        ("jest", "js", None),
        ("cargo test", "rust", None),
        ("python3 -m pytest", "python", Some("pytest")),
        ("python -m unittest", "python", Some("unittest")),
        ("node --test", "js", Some("node")),
        ("pytest", "pytest", Some("pytest")),
        ("unittest", "unittest", Some("unittest")),
        ("cargo", "cargo", None),
        ("npm", "npm", None),
    ] {
        let out = normalize_run_tests_args(&json!({ "runner": runner }), &ws);
        assert_eq!(out["runner"], lang, "{runner}");
        assert_eq!(out["entrypoint"].as_str(), entrypoint, "{runner}");
        assert!(
            workspace_lang::parse_lang(out["runner"].as_str().unwrap()).is_some(),
            "{runner}"
        );
    }
}

#[test]
fn canonical_arguments_pass_through_byte_for_byte() {
    let ws = std::env::temp_dir();
    for args in [
        json!({}),
        json!({"runtime": "rust", "args": "acronym"}),
        json!({"runner": "js"}),
        json!({"runtime": "python", "entrypoint": "unittest", "args": "'-q'"}),
        json!({"runtime": "node", "args": "./*", "runner": "unknown-thing"}),
        json!({"dir": "cockpit"}),
        json!("not an object"),
    ] {
        assert_eq!(normalize_run_tests_args(&args, &ws), args, "{args}");
    }
}

#[test]
fn args_that_repeat_the_runner_command_keep_what_follows() {
    let ws = std::env::temp_dir();
    let out = normalize_run_tests_args(&json!({"args": "python3 -m pytest -q"}), &ws);
    assert_eq!(out["args"], "'-q'");
    assert_eq!(out["entrypoint"], "pytest");
    assert_eq!(out["runner"], "python");
    let out = normalize_run_tests_args(&json!({"runner": "npx jest connect"}), &ws);
    assert_eq!(out["runner"], "js");
    assert_eq!(out["args"], "'connect'");
    let out = normalize_run_tests_args(
        &json!({"runner": "npm test", "args": "--watchAll=false"}),
        &ws,
    );
    assert_eq!(out["runner"], "js");
    assert_eq!(out["args"], "--watchAll=false");
    // An explicit entrypoint always wins; a runtime already given is kept.
    let out = normalize_run_tests_args(
        &json!({"runner": "python3 -m pytest", "entrypoint": "unittest"}),
        &ws,
    );
    assert_eq!(out["entrypoint"], "unittest");
    let out = normalize_run_tests_args(&json!({"runtime": "python", "args": "pytest -k x"}), &ws);
    assert_eq!(out["runtime"], "python");
    assert!(out.get("runner").is_none());
    assert_eq!(out["args"], "'-k' 'x'");
    // Cargo strips its own repeated subcommand (`cargo_argv`).
    let out = normalize_run_tests_args(&json!({"runtime": "rust", "args": "test"}), &ws);
    assert_eq!(out["args"], "test");
}

#[test]
fn cargo_arguments_that_repeat_the_subcommand_are_the_subcommand() {
    assert_eq!(cargo_argv("test", "test").unwrap(), ["test"]);
    assert_eq!(
        cargo_argv("test", "cargo test --features exponentials").unwrap(),
        ["test", "--features", "exponentials"]
    );
    assert_eq!(cargo_argv("test", "acronym").unwrap(), ["test", "acronym"]);
    assert_eq!(cargo_argv("test", "").unwrap(), ["test"]);
    assert_eq!(
        cargo_argv("check", "check --all-targets").unwrap(),
        ["check", "--all-targets"]
    );
    assert_eq!(
        cargo_argv("test", "-- test").unwrap(),
        ["test", "--", "test"]
    );
}

#[test]
fn an_absolute_dir_inside_the_workspace_becomes_workspace_relative() {
    let root = scratch("dir");
    let canonical = root.canonicalize().unwrap();
    let out = normalize_run_tests_args(
        &json!({"dir": canonical.join("react").to_string_lossy()}),
        &root,
    );
    assert_eq!(out["dir"], "react");
    let out = normalize_run_tests_args(&json!({"crate": canonical.to_string_lossy()}), &root);
    assert_eq!(out["crate"], ".");
    // Outside the workspace it stays as given, and the tool reports it.
    let out = normalize_run_tests_args(&json!({"dir": "/etc"}), &root);
    assert_eq!(out["dir"], "/etc");
    let _ = std::fs::remove_dir_all(&root);
}
