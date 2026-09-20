//! Shell argv routing. Only dispatch-owned records authenticate the typed result.
use crate::club::ToolCall;
use crate::harness::{ToolOutcome, ToolRegistry};
use serde_json::{Value, json};
use std::path::Path;

#[derive(Clone, Debug)]
pub(crate) struct RoutingReceipt {
    pub(crate) routed_call: Option<ToolCall>,
    pub(crate) reason: String,
    pub(crate) routed_cwd: Option<std::path::PathBuf>,
}

#[derive(Clone)]
pub(crate) struct RoutedExecution {
    pub(crate) text: String,
    pub(crate) outcome: ToolOutcome,
    pub(crate) receipt: RoutingReceipt,
}

#[derive(Debug)]
pub(crate) struct Route {
    pub(crate) call: ToolCall,
    tail: Option<usize>,
    echo: Option<String>,
}

pub(crate) fn valid_env_key(key: &str) -> bool {
    let mut chars = key.chars();
    chars
        .next()
        .is_some_and(|c| c.is_ascii_alphabetic() || c == '_')
        && chars.all(|c| c.is_ascii_alphanumeric() || c == '_')
}

pub(crate) fn protected_env_key(key: &str) -> bool {
    matches!(
        key,
        "PATH"
            | "HOME"
            | "ENV"
            | "BASH_ENV"
            | "CI"
            | "NO_COLOR"
            | "FORCE_COLOR"
            | "TMPDIR"
            | "TMP"
            | "TEMP"
            | "XDG_CACHE_HOME"
    ) || [
        "ANGEL_",
        "CARGO",
        "RUST",
        "NODE_",
        "PYTHON",
        "PYTEST",
        "LD_",
        "DYLD_",
        "GO",
        "npm_config_",
    ]
    .iter()
    .any(|prefix| key.starts_with(prefix))
}

fn quoted(words: &[String]) -> String {
    words
        .iter()
        .map(|s| format!("'{}'", s.replace('\'', "'\\''")))
        .collect::<Vec<_>>()
        .join(" ")
}

pub(crate) fn plan(name: &str, args: &Value, root: &Path) -> Result<Route, String> {
    if !matches!(name, "shell" | "proc_run") {
        return Err("not a shell tool".into());
    }
    if args.get("cwd").is_some() || args.get("env").is_some() || args.get("dir").is_some() {
        return Err("shell cwd/env overrides require plain shell semantics".into());
    }
    let raw = crate::tools::shell::shell_command_arg(args).ok_or("missing command")?;
    command(raw, root, false)
}

fn command(raw: &str, root: &Path, nested: bool) -> Result<Route, String> {
    let mut inner = raw.trim();
    let mut routed_dir = None;
    if let Some((prefix, rest)) = inner.split_once("&&") {
        if nested
            || prefix.contains(['$', '`', '|', '&', ';', '<', '>', '\n', '\r'])
            || rest.contains(['$', '`', '|', '&', ';', '<', '>', '\n', '\r'])
        {
            return Err(
                "cd prefix requires one cd and one && without other shell operators".into(),
            );
        }
        let prefix = crate::tools::build::parse_direct_argv(prefix.trim())?;
        let [cd, path] = prefix.as_slice() else {
            return Err("prefix must be cd <directory> && <test command>".into());
        };
        if cd != "cd" || path.starts_with('-') || path.starts_with('~') {
            return Err("prefix must be a literal cd directory".into());
        }
        if Path::new(path)
            .components()
            .any(|p| p.as_os_str() == "off-limits")
        {
            return Err("cd directory is quarantined".into());
        }
        let root = root
            .canonicalize()
            .map_err(|e| format!("resolve workspace: {e}"))?;
        let cwd = root
            .join(path)
            .canonicalize()
            .map_err(|e| format!("resolve cd directory: {e}"))?;
        if !cwd.starts_with(&root)
            || !cwd.is_dir()
            || cwd.components().any(|p| p.as_os_str() == "off-limits")
        {
            return Err("cd directory escapes the workspace or is not a directory".into());
        }
        routed_dir = Some(cwd.strip_prefix(&root).unwrap().to_path_buf());
        inner = rest.trim();
    }
    let mut tail = None;
    let mut echo = None;
    if let Some((head, suffix)) = inner.split_once('|') {
        if suffix.starts_with('|') && suffix[1..].trim() == "true" {
            inner = head.trim();
        } else {
            let words = suffix.split_whitespace().collect::<Vec<_>>();
            let count = match words.as_slice() {
                ["tail"] => Some(10),
                ["tail", count] => count
                    .strip_prefix('-')
                    .and_then(|n| n.parse::<usize>().ok()),
                ["tail", "-n", count] => count.parse::<usize>().ok(),
                _ => None,
            }
            .ok_or("unsupported pipeline; only a final tail of the typed output is routable")?;
            tail = Some(count);
            inner = head.trim();
        }
    } else if let Some((head, suffix)) = inner.split_once(';') {
        let words = crate::tools::build::parse_direct_argv(suffix.trim())?;
        if words.first().map(String::as_str) != Some("echo")
            || words.iter().skip(1).any(|s| s.starts_with('-'))
        {
            return Err(
                "unsupported command list; only literal echo after the verifier is routable".into(),
            );
        }
        echo = Some(words[1..].join(" "));
        inner = head.trim();
    }
    if let Some(head) = inner.strip_suffix("2>&1") {
        inner = head.trim_end();
    }
    if inner.contains(['$', '`', '|', '&', ';', '<', '>', '\n', '\r'])
        || echo
            .as_ref()
            .is_some_and(|s| s.contains(['$', '`', '|', '&', ';', '<', '>', '\n', '\r']))
    {
        return Err("shell expansion or compound shape cannot be routed safely".into());
    }
    let words = crate::tools::build::parse_direct_argv(inner)?;
    let mut offset = usize::from(words.first().is_some_and(|w| w == "env"));
    let mut env = serde_json::Map::new();
    while let Some((key, value)) = words.get(offset).and_then(|w| w.split_once('=')) {
        if !valid_env_key(key) {
            return Err("invalid environment assignment name".into());
        }
        if protected_env_key(key) {
            return Err(format!(
                "environment override {key} controls the trusted verifier"
            ));
        }
        env.insert(key.to_string(), Value::String(value.to_string()));
        offset += 1;
    }
    let words = &words[offset..];
    let w = words.iter().map(String::as_str).collect::<Vec<_>>();
    let (tool, runtime, skip, entrypoint) = match w.as_slice() {
        ["cargo", "test", ..] => ("run_tests", "rust", 2, ""),
        ["cargo", "check", ..] => ("check", "rust", 2, ""),
        ["cargo", "clippy", ..] => ("lint", "rust", 2, ""),
        ["node", "--test", ..] => ("run_tests", "node", 2, "node"),
        ["npm" | "pnpm" | "yarn", "test", ..] => ("run_tests", "node", 2, ""),
        ["pytest", ..] => ("run_tests", "python", 1, "pytest"),
        ["python" | "python3", "-m", "pytest", ..] => ("run_tests", "python", 3, "pytest"),
        ["python" | "python3", "-m", "unittest", ..] => ("run_tests", "python", 3, "unittest"),
        ["go", "test", ..] => ("run_tests", "go", 2, ""),
        ["make", "test"] if !nested => {
            let make_root = routed_dir
                .as_ref()
                .map(|dir| root.join(dir))
                .unwrap_or_else(|| root.to_path_buf());
            let path = make_root.join("Makefile");
            let meta = std::fs::symlink_metadata(&path).map_err(|e| e.to_string())?;
            if !meta.is_file() || meta.len() > 128 * 1024 {
                return Err("Makefile is not a bounded regular file".into());
            }
            let text = std::fs::read_to_string(path).map_err(|e| e.to_string())?;
            if text
                .lines()
                .any(|line| line.trim_start().starts_with("test:") && line.trim() != "test:")
            {
                return Err(
                    "Makefile test target has prerequisites or target-specific configuration"
                        .into(),
                );
            }
            let mut lines = text.lines();
            let mut recipes = Vec::new();
            while let Some(line) = lines.next() {
                if line.trim() == "test:" {
                    for line in lines.by_ref() {
                        if !line.starts_with('\t') {
                            break;
                        }
                        recipes.push(line.trim().trim_start_matches('@'));
                    }
                }
            }
            if recipes.len() != 1
                || text.lines().any(|line| {
                    line.contains("include ")
                        || line.contains('=')
                        || line.trim_start().starts_with(".ONESHELL")
                })
            {
                return Err("Makefile test target requires exactly one direct recipe without dependencies or configuration".into());
            }
            let mut route = command(recipes[0], &make_root, true)?;
            if route.tail.is_some()
                || route.echo.is_some()
                || ((routed_dir.is_some() || !env.is_empty()) && route.call.name != "run_tests")
            {
                return Err("compound Makefile recipes are not routable".into());
            }
            if let Some(dir) = &routed_dir {
                route.call.args["dir"] = json!(if dir.as_os_str().is_empty() {
                    Path::new(".")
                } else {
                    dir
                });
            }
            if !env.is_empty() {
                let recipe_env = route.call.args["env"]
                    .as_object()
                    .cloned()
                    .unwrap_or_default();
                env.extend(recipe_env);
                route.call.args["env"] = json!(env);
            }
            route.tail = tail;
            route.echo = echo;
            return Ok(route);
        }
        _ => return Err("argv head is not a supported verifier invocation".into()),
    };
    let mut extra = &words[skip..];
    if matches!(w.first(), Some(&"npm" | &"pnpm" | &"yarn"))
        && extra.first().is_some_and(|s| s == "--")
    {
        extra = &extra[1..];
    }
    if (routed_dir.is_some() || !env.is_empty()) && tool != "run_tests" {
        return Err("cd/environment prefixes require a test verifier".into());
    }
    let mut args = json!({"runtime":runtime,"args":quoted(extra),"entrypoint":entrypoint});
    if let Some(dir) = routed_dir {
        args["dir"] = json!(if dir.as_os_str().is_empty() {
            Path::new(".")
        } else {
            &dir
        });
    }
    if !env.is_empty() {
        args["env"] = json!(env);
    }
    Ok(Route {
        call: ToolCall {
            id: String::new(),
            name: tool.into(),
            args,
        },
        tail,
        echo,
    })
}

impl Route {
    pub(super) fn present(&self, text: String) -> String {
        // Preserve the authoritative header even when tail would hide it.
        let header = text.lines().next().unwrap_or("");
        let mut output = if let Some(count) = self.tail {
            let lines = text.lines().collect::<Vec<_>>();
            let start = lines.len().saturating_sub(count);
            if start > 0 {
                format!(
                    "{header}\n[typed output tail]\n{}",
                    lines[start..].join("\n")
                )
            } else {
                text.clone()
            }
        } else {
            text
        };
        if let Some(echo) = &self.echo {
            output.push('\n');
            output.push_str(echo);
        }
        output
    }
}

impl ToolRegistry {
    pub(crate) fn routed_execution(
        &self,
        call: &ToolCall,
        result: &str,
    ) -> Option<RoutedExecution> {
        self.routed_verifications
            .lock()
            .ok()?
            .get(&serde_json::to_string(&(call.name.as_str(), &call.args)).ok()?)
            .filter(|entry| entry.text == result)
            .cloned()
    }

    pub(crate) fn executed_outcome(
        &self,
        call: &ToolCall,
        result: &str,
        denied: bool,
    ) -> ToolOutcome {
        if !denied && let Some(entry) = self.routed_execution(call, result) {
            return entry.outcome;
        }
        super::turn_event_outcome(call, result, denied)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::harness::{Tool, VerificationOutcome};

    #[test]
    fn t04b_routing_table_and_safe_presentation() {
        let root = std::env::temp_dir();
        for (command, tool, runtime) in [
            ("npm test", "run_tests", "node"),
            ("pnpm test -- -k", "run_tests", "node"),
            ("yarn test", "run_tests", "node"),
            ("node --test test.mjs", "run_tests", "node"),
            ("pytest test_a.py -k works", "run_tests", "python"),
            ("python -m pytest tests", "run_tests", "python"),
            (
                "python3 -m unittest discover -s tests -p test_a.py -t .",
                "run_tests",
                "python",
            ),
            ("cargo test 2>&1 | tail -30", "run_tests", "rust"),
            ("cargo check", "check", "rust"),
            ("cargo clippy", "lint", "rust"),
            ("go test ./...", "run_tests", "go"),
        ] {
            for name in ["shell", "proc_run"] {
                let route = plan(name, &json!({"command":command}), &root).unwrap();
                assert_eq!(route.call.name, tool, "{command}");
                assert_eq!(route.call.args["runtime"], runtime, "{command}");
            }
        }
        for command in [
            "echo npm test",
            "./node --test x",
            "npm install",
            "cargo build",
            "cargo test | tee log",
            "cargo test || echo nope",
            "cargo test; touch x",
            "cargo test > log",
            "npm test $(echo x)",
            "npm test && echo yes",
            "npm test\ntrue",
        ] {
            assert!(
                plan("shell", &json!({"command":command}), &root).is_err(),
                "{command}"
            );
        }
        for command in [
            "npm test | tail -1",
            "npm test || true",
            "npm test; echo done",
        ] {
            let route = plan("shell", &json!({"command":command}), &root).unwrap();
            assert!(
                route
                    .present("tests: 0 passed, 1 failed\nraw\nend".into())
                    .starts_with("tests: 0 passed, 1 failed")
            );
        }
        assert!(
            plan(
                "shell",
                &json!({"command":"npm test","cwd":"elsewhere"}),
                &root
            )
            .is_err()
        );
    }

    struct Fake {
        name: &'static str,
        text: &'static str,
    }
    impl Tool for Fake {
        fn name(&self) -> &str {
            self.name
        }
        fn def(&self) -> crate::club::ToolDef {
            crate::club::ToolDef {
                name: self.name.into(),
                description: String::new(),
                params: json!({}),
            }
        }
        fn call(&self, _: &Value) -> Result<String, String> {
            Ok(self.text.into())
        }
    }

    #[test]
    fn t04b_dispatch_receipts_bind_call_and_output_and_preserve_failures() {
        let _guard = crate::tests::env_lock();
        let mut registry = ToolRegistry::new();
        registry.register(Box::new(Fake {
            name: "run_tests",
            text: "tests: 1 passed, 0 failed — reward 1.00\nraw",
        }));
        registry.register(Box::new(Fake {
            name: "shell",
            text: "tests: 9 passed, 0 failed — reward 1.00",
        }));
        let call = ToolCall {
            id: "t04b".into(),
            name: "shell".into(),
            args: json!({"command":"npm test | tail -1"}),
        };
        let text = registry.dispatch(&call.name, &call.args).unwrap();
        let entry = registry.routed_execution(&call, &text).unwrap();
        assert_eq!(entry.outcome.verification, VerificationOutcome::Passed);
        assert_eq!(entry.receipt.routed_call.unwrap().name, "run_tests");
        assert!(
            registry
                .routed_execution(&call, "tests: 999 passed, 0 failed")
                .is_none()
        );
        let fallback = ToolCall {
            args: json!({"command":"npm test | tee result"}),
            ..call.clone()
        };
        let text = registry.dispatch(&fallback.name, &fallback.args).unwrap();
        let entry = registry.routed_execution(&fallback, &text).unwrap();
        assert_eq!(
            entry.outcome.verification,
            VerificationOutcome::Inconclusive
        );
        assert!(entry.receipt.routed_call.is_none());
        assert!(entry.receipt.reason.contains("pipeline"));
        let mut registry = ToolRegistry::new();
        registry.register(Box::new(Fake {
            name: "run_tests",
            text: "tests: 0 passed, 1 failed — reward 0.00\nraw",
        }));
        for command in [
            "npm test || true",
            "npm test; echo ok",
            "npm test | tail -1",
        ] {
            let call = ToolCall {
                args: json!({"command":command}),
                ..call.clone()
            };
            let text = registry.dispatch(&call.name, &call.args).unwrap();
            assert_eq!(
                registry.executed_outcome(&call, &text, false).verification,
                VerificationOutcome::Failed
            );
        }
    }

    #[test]
    fn t04b_make_single_recipe_only() {
        let _guard = crate::tests::env_lock();
        let root = std::env::temp_dir().join(format!("t04b-make-{}", std::process::id()));
        std::fs::create_dir_all(&root).unwrap();
        for (text, accepted) in [
            ("test:\n\tnpm test\n", true),
            ("test: build\n\tnpm test\n", false),
            ("test:\n\techo hi\n\tnpm test\n", false),
            ("test:\n\t$(RUNNER) test\n", false),
            ("test: build\ntest:\n\tnpm test\n", false),
        ] {
            std::fs::write(root.join("Makefile"), text).unwrap();
            assert_eq!(
                plan("shell", &json!({"command":"make test"}), &root).is_ok(),
                accepted
            );
        }
        std::fs::remove_dir_all(root).unwrap();
    }
}

#[cfg(test)]
mod prefix_tests {
    use super::*;
    use crate::harness::VerificationOutcome;

    #[test]
    fn shell_verifier_cd_and_env_prefixes_execute_and_reach_ledger() {
        let _guard = crate::tests::env_lock();
        let root = std::env::temp_dir().join(format!("t04c-prefix-{}", std::process::id()));
        let suite = root.join("suite space");
        std::fs::create_dir_all(&suite).unwrap();
        std::fs::write(suite.join("test_prefix.py"), "import os, unittest\nfrom pathlib import Path\nclass Prefix(unittest.TestCase):\n def test_context(self):\n  self.assertEqual(Path.cwd().name, 'suite space')\n  self.assertEqual(os.environ.get('TEST_MODE'), 'fixture value')\n").unwrap();
        let parent_env = std::env::var_os("TEST_MODE");
        let mut registry = ToolRegistry::new();
        registry.set_workspace(root.clone());
        registry.register(Box::new(crate::tools::build::RunTestsTool::in_dir(
            root.clone(),
        )));
        for command in [
            format!(
                "cd '{}' && env TEST_MODE='fixture value' python -m unittest -v test_prefix.py",
                suite.display()
            ),
            "cd 'suite space' && TEST_MODE='fixture value' python -m unittest -v test_prefix.py"
                .into(),
        ] {
            let call = ToolCall {
                id: "prefix".into(),
                name: "shell".into(),
                args: json!({"command":command}),
            };
            let text = registry.dispatch(&call.name, &call.args).unwrap();
            let routed = registry.routed_execution(&call, &text).unwrap();
            assert_eq!(
                routed.outcome.verification,
                VerificationOutcome::Passed,
                "{text}"
            );
            assert_eq!(
                routed.receipt.routed_cwd,
                Some(suite.canonicalize().unwrap())
            );
            crate::harness::trajectory::note_tool_outcome(
                0,
                "shell",
                &call.args,
                &text,
                "ok",
                false,
                None,
                Some("passed"),
                None,
                text.len(),
            );
            crate::harness::trajectory::note_tool_routing(&routed.receipt);
            let ledger = crate::harness::trajectory::tool_ledger_snapshot();
            assert_eq!(
                ledger.last().unwrap()["routed_cwd"],
                json!(suite.canonicalize().unwrap())
            );
            assert_eq!(ledger.last().unwrap()["routed_to"], "run_tests");
            println!(
                "prefix child: cwd and environment assertions passed; ledger routed_cwd matched canonical suite directory"
            );
        }
        assert_eq!(std::env::var_os("TEST_MODE"), parent_env);
        let outside = format!("cd '{}' && npm test", root.parent().unwrap().display());
        assert!(
            plan("shell", &json!({"command":outside}), &root)
                .unwrap_err()
                .contains("escapes")
        );
        for command in [
            "env TEST_MODE=fixture npm test",
            "TEST_MODE=fixture npm test",
            "env npm test",
            "TEST_MODE=one TEST_MODE=two npm test",
            "cd . && npm test",
        ] {
            for tool in ["shell", "proc_run"] {
                assert!(
                    plan(tool, &json!({"command":command}), &root).is_ok(),
                    "{command}"
                );
            }
        }
        for command in [
            "cd .. && npm test",
            "cd 'suite space' && cd .. && npm test",
            "cd . && npm test | tail",
            "cd . && npm test; echo ok",
            "cd . && npm test 2>&1",
            "cd . && npm test || true",
            "cd . && npm test && echo ok",
            "cd missing && npm test",
            "env -i npm test",
            "env PATH=/tmp npm test",
            "1BAD=x npm test",
        ] {
            assert!(
                plan("shell", &json!({"command":command}), &root).is_err(),
                "{command}"
            );
        }
        #[cfg(unix)]
        {
            std::os::unix::fs::symlink(root.parent().unwrap(), root.join("escape")).unwrap();
            assert!(
                plan("shell", &json!({"command":"cd escape && npm test"}), &root)
                    .unwrap_err()
                    .contains("escapes")
            );
        }
        std::fs::remove_dir_all(root).unwrap();
    }
}
