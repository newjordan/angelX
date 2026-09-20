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
        "cd 'suite space' && TEST_MODE='fixture value' python -m unittest -v test_prefix.py".into(),
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
