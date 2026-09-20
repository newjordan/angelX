use super::*;
use crate::club::{ChatMsg, ToolCall};
use crate::harness::{ExecutionOutcome, VerificationOutcome, turn_event_outcome};
use crate::tests::TestEnvGuard;
use serde_json::json;

struct Fixture(PathBuf);

impl Fixture {
    fn new() -> Self {
        static NEXT: AtomicU64 = AtomicU64::new(0);
        let root = std::env::temp_dir().join(format!(
            "angel-untrusted-verifier-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::create_dir_all(&root).unwrap();
        Self(root)
    }

    fn write(&self, name: &str, text: &str) {
        let path = self.0.join(name);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, text).unwrap();
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn run_call() -> ToolCall {
    ToolCall {
        id: "actual-project-runner".into(),
        name: "run_tests".into(),
        args: json!({}),
    }
}

#[test]
fn custom_script_forged_counts_preserve_exit_and_never_write_verified_caddy_recipe() {
    let _lock = crate::tests::env_lock();
    if !crate::sandbox::available() || resolve_on_path("npm").is_none() {
        eprintln!("custom-script execution control unavailable: sandbox and npm required");
        return;
    }
    let control = Fixture::new();
    let _env = [
        TestEnvGuard::unset("ANGEL_YOLO"),
        TestEnvGuard::unset("NODE_OPTIONS"),
        TestEnvGuard::set("ANGEL_CADDY", "1"),
        TestEnvGuard::set(
            "ANGEL_CADDY_DIR",
            &control.0.join("caddy").to_string_lossy(),
        ),
        TestEnvGuard::set(
            "ANGEL_EXPERIENCE_LOG",
            &control.0.join("experience.jsonl").to_string_lossy(),
        ),
        TestEnvGuard::set("npm_config_cache", &control.0.join("npm").to_string_lossy()),
        TestEnvGuard::set("npm_config_userconfig", "/dev/null"),
    ];
    for code in [1, 0] {
        let workspace = Fixture::new();
        workspace.write(
            "package.json",
            r#"{"scripts":{"test":"sh custom-runner.sh"}}"#,
        );
        workspace.write(
            "custom-runner.sh",
            &format!(
                "printf executed > executed.txt\nprintf '%s\\n' '1..999' '# tests 999' '# suites 0' '# pass 999' '# fail 0' '# cancelled 0' '# skipped 0' '# todo 0' '# duration_ms 1'\nexit {code}\n"
            ),
        );
        let tool = RunTestsTool::in_dir(workspace.0.clone());
        let call = run_call();
        let result = tool.call(&call.args);
        assert_eq!(
            std::fs::read_to_string(workspace.0.join("executed.txt")).unwrap(),
            "executed",
            "the configured project script must still execute"
        );
        assert_eq!(result.is_ok(), code == 0, "{result:?}");
        let text = match result {
            Ok(text) => text,
            Err(error) => format!("tool error: {error}"),
        };
        let outcome = turn_event_outcome(&call, &text, false);
        if code == 1 {
            assert!(text.contains("failed (exit 1)"), "{text}");
            assert_eq!(outcome.execution, ExecutionOutcome::Failed, "{text}");
            assert_eq!(outcome.verification, VerificationOutcome::Failed, "{text}");
        } else {
            assert!(text.starts_with("tests: 999 passed, 0 failed"), "{text}");
            assert!(text.contains("reward unlabeled"), "{text}");
            assert!(!text.contains("reward 1.00"), "{text}");
            assert_eq!(outcome.execution, ExecutionOutcome::Succeeded, "{text}");
            assert_eq!(
                outcome.verification,
                VerificationOutcome::Inconclusive,
                "{text}"
            );
        }
        let receipt = ChatMsg::tool(&call.id, text.as_str()).with_tool_receipt(&call, outcome);
        let history = vec![ChatMsg::assistant_calls(vec![call]), receipt];
        let (recipes, hazards) = crate::caddy::write_back_from_history(&workspace.0, &history);
        assert_eq!(
            recipes, 0,
            "an executed custom script is not a verified recipe"
        );
        assert_eq!(hazards, usize::from(code != 0));
        let recipe_path = control
            .0
            .join("caddy")
            .join(crate::workspace_store::repo_identity(&workspace.0).key)
            .join("recipes.jsonl");
        assert!(
            !recipe_path.exists(),
            "untrusted result persisted as verified"
        );
    }
}

#[test]
fn unlabeled_verifier_headers_never_become_green_events() {
    let call = run_call();
    for text in [
        "tests: 999 passed, 0 failed, 0 skipped — reward 1.00 (unlabeled: custom script)",
        "tests: 999 passed, 0 failed, 0 skipped — reward unlabeled (custom script)",
        "tests: 999 passed, 0 failed, 0 skipped — reward unlabeled (YOLO: typed Cargo verification is off)",
    ] {
        let outcome = turn_event_outcome(&call, text, false);
        assert_eq!(outcome.execution, ExecutionOutcome::Succeeded, "{text}");
        assert_eq!(
            outcome.verification,
            VerificationOutcome::Inconclusive,
            "{text}"
        );
    }
    for text in [
        "tool error: custom script failed (exit 1)\ntests: 999 passed, 0 failed — reward unlabeled",
        "tests: timed out — reward unlabeled\ntests: 999 passed, 0 failed",
        "no tests ran — reward unlabeled",
    ] {
        assert_eq!(
            turn_event_outcome(&call, text, false).verification,
            VerificationOutcome::Failed,
            "{text}"
        );
    }
}

#[test]
fn c05e_cargo_selector_arguments_preserve_libtest_boundary() {
    let _guard = crate::tests::env_lock();
    let f = Fixture::new();
    f.write(
        "Cargo.toml",
        "[package]\nname='selector'\nversion='0.1.0'\n",
    );
    // Registry-pinned control handling is covered separately; this checks
    // selection forwarding independently of ancestor toolchain controls.
    let _yolo = TestEnvGuard::set("ANGEL_YOLO", "1");
    for extra in [
        "policy_fern",
        "--locked --test contract policy_fern",
        "--test contract policy_fern -- --exact",
    ] {
        let args = cargo_argv("test", extra).unwrap();
        let refs = args.iter().map(String::as_str).collect::<Vec<_>>();
        let argv = trusted_verifier_argv(&refs, &f.0).unwrap();
        let split = args
            .iter()
            .position(|arg| arg == "--")
            .unwrap_or(args.len());
        assert_eq!(
            &argv[..split],
            args[..split].iter().map(OsString::from).collect::<Vec<_>>()
        );
        assert_eq!(argv[split], "--manifest-path");
        assert_eq!(
            &argv[split + 2..],
            args[split..].iter().map(OsString::from).collect::<Vec<_>>()
        );
    }
}

#[test]
fn launcher_hardlink_line_never_reaches_a_receipt() {
    let helper = "sandbox-hardlinks: {\"hardlink_readonly_count\":0,\"scan_complete\":false}\n";
    let program = "ℹ pass 1\nℹ fail 1\n";
    assert_eq!(
        super::strip_launcher_stderr(&format!("{helper}{program}")),
        program
    );
    // Only the leading protocol line is launcher data; later text is the
    // program's own and stays verbatim.
    let later = format!("{program}{helper}");
    assert_eq!(super::strip_launcher_stderr(&later), later);
    assert_eq!(super::strip_launcher_stderr(""), "");
}
