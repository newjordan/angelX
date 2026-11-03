use super::*;

#[test]
fn code_mode_repo_recon_query_is_encoded_as_data() {
    let script =
        repo_recon_script("needle'); return shell({command:'touch escaped'}); // NeedleWidget")
            .unwrap();
    assert!(!script.contains("shell("));
    assert!(!script.contains("touch escaped"));
    assert!(script.contains("needlewidget"));
    assert!(script.len() < 16 * 1024);
}

#[test]
fn code_mode_repo_recon_terms_are_bounded_and_deterministic() {
    let query = "repair cockpit/src/agent/harness/turn.rs repeated read search telemetry telemetry";
    let first = repo_recon_terms(query);
    assert_eq!(first, repo_recon_terms(query));
    assert!(first.len() <= REPO_RECON_MAX_TERMS);
    assert!(first.contains(&"cockpit/src/agent/harness/turn.rs".to_string()));
}

#[test]
fn code_mode_repo_recon_drops_task_fluff_stopwords() {
    let terms = repo_recon_terms(
        "please help implement improve task only just something MutationThrashNudge",
    );
    for stop in [
        "please",
        "help",
        "implement",
        "improve",
        "task",
        "only",
        "just",
        "something",
    ] {
        assert!(
            !terms.iter().any(|t| t == stop),
            "stopword {stop} should not be a recon term, got {terms:?}"
        );
    }
    assert!(
        terms
            .iter()
            .any(|t| t.contains("mutation") || t.contains("thrash") || t.contains("nudge")),
        "identifier should survive fluff strip: {terms:?}"
    );
}

#[test]
fn code_mode_repo_recon_program_prefers_impl_paths() {
    let script = repo_recon_script("MutationThrashNudge thrash").unwrap();
    assert!(
        script.contains("implPathBonus"),
        "recon program should rank implementing libraries above docs/testdata"
    );
    assert!(script.contains("docs|testdata|fixtures"));
    assert!(script.contains("src|lib|pkg|core"));
}

#[test]
fn repo_recon_embedded_program_matches_previous_baseline() {
    assert_eq!(
        crate::knowledge::cut::sha256_hex(REPO_RECON_PROGRAM.as_bytes()),
        "4bcdb2518b3ff18be30f47c3655cc188ee8f72b6de7cae68809e5ce7b71bf7fc"
    );
}

#[test]
fn repo_recon_reload_is_pinned_and_read_only() {
    if std::env::var_os("ANGEL_RECON_RELOAD_TEST_CHILD").is_none() {
        let root = crate::tests::TestGitWorkspace::new("recon-reload");
        let output = std::process::Command::new(std::env::current_exe().unwrap())
            .arg("repo_recon_reload_is_pinned_and_read_only")
            .arg("--nocapture")
            .env("ANGEL_RECON_RELOAD_TEST_CHILD", "1")
            .env("ANGEL_RESOURCE_DIR", root.path())
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "reload subprocess failed: {}\n{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
        return;
    }
    let root = std::path::PathBuf::from(std::env::var_os("ANGEL_RESOURCE_DIR").unwrap());
    let path = root.join("scripts/runtime/repo-recon.js");
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(&path, REPO_RECON_PROGRAM).unwrap();
    let captured = repo_recon_script("needle").unwrap();
    std::fs::write(
        &path,
        REPO_RECON_PROGRAM.replace("angel-repo-recon/v2", "test-reloaded/v2"),
    )
    .unwrap();
    let reloaded = repo_recon_script("needle").unwrap();
    assert!(captured.contains("angel-repo-recon/v2"));
    assert!(reloaded.contains("test-reloaded/v2"));
    std::fs::write(&path, "return 1;").unwrap();
    assert!(repo_recon_script("needle").is_err());

    struct EffectProbe(std::sync::Arc<std::sync::atomic::AtomicBool>);
    impl Tool for EffectProbe {
        fn name(&self) -> &str {
            "effect_probe"
        }
        fn def(&self) -> ToolDef {
            ToolDef {
                name: self.name().into(),
                description: "effect probe".into(),
                params: serde_json::json!({"type":"object"}),
            }
        }
        fn call(&self, _: &Value) -> Result<String, String> {
            self.0.store(true, std::sync::atomic::Ordering::SeqCst);
            Ok("effect-ran".into())
        }
    }
    let touched = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
    let mut registry = ToolRegistry::new();
    registry.register(Box::new(EffectProbe(touched.clone())));
    std::fs::write(
        &path,
        "const terms=__TASK_RECON_TERMS__; const pattern=__TASK_RECON_PATTERN__; invalid javascript {",
    )
    .unwrap();
    let invalid = run_code_mode_tool(
        &registry,
        &Hooks::default(),
        &serde_json::json!({"recipe":"repo_recon","query":"needle"}),
        None,
        None,
    );
    assert!(invalid.contains("tool error:"), "{invalid}");
    std::fs::write(&path, "const terms=__TASK_RECON_TERMS__; const pattern=__TASK_RECON_PATTERN__; return batch([{tool:'effect_probe',args:{}}]);").unwrap();
    let result = run_code_mode_tool(
        &registry,
        &Hooks::default(),
        &serde_json::json!({"recipe":"repo_recon","query":"needle"}),
        None,
        None,
    );
    assert!(result.contains("recipe=repo_recon"), "{result}");
    assert!(!touched.load(std::sync::atomic::Ordering::SeqCst));
}
