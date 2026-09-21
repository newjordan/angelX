use super::*;

#[test]
fn module_purpose_extracts_first_sentence_of_doc_block() {
    let src = "//! The agent harness: per-agent tool-loop + the orchestrator.\n\
                   //! More detail on the second line.\n\nuse std::fs;\n";
    assert_eq!(
        module_purpose(src),
        "The agent harness: per-agent tool-loop + the orchestrator."
    );
}

#[test]
fn module_purpose_empty_when_no_doc_comment() {
    let src = "use std::fs;\n\npub fn main() {}\n";
    assert_eq!(module_purpose(src), "");
}

#[test]
fn module_purpose_joins_wrapped_lines_into_one_sentence() {
    let src = "//! A tiny CPU raytracer rendered into a ratatui buffer — a rotating\n\
                   //! cube. Second sentence here.\n";
    assert_eq!(
        module_purpose(src),
        "A tiny CPU raytracer rendered into a ratatui buffer — a rotating cube."
    );
}

#[test]
fn pub_type_name_picks_public_types_only() {
    assert_eq!(pub_type_name("pub struct Foo {"), Some("Foo".to_string()));
    assert_eq!(
        pub_type_name("pub(crate) trait Bar:"),
        Some("Bar".to_string())
    );
    assert_eq!(pub_type_name("pub enum E {"), Some("E".to_string()));
    assert_eq!(pub_type_name("struct Private {"), None); // not pub
    assert_eq!(pub_type_name("pub fn run() {"), None); // fn, not a type
}

#[test]
fn module_symbols_counts_and_collects_types() {
    let src = "pub struct A;\nfn helper() {}\npub trait T {}\nstruct B;\nimpl A {}\n";
    let sym = module_symbols(src);
    assert_eq!(sym.count, 5, "5 top-level decls");
    assert_eq!(sym.key_types, vec!["A".to_string(), "T".to_string()]);
}

#[test]
fn parse_crate_meta_reads_package_and_bin() {
    let toml = "[package]\nname = \"angelX-cockpit\"\nversion = \"0.1.0\"\nedition = \"2021\"\n\n\
                    [[bin]]\nname = \"angel\"\npath = \"src/main.rs\"\n";
    let m = parse_crate_meta(toml);
    assert_eq!(m.name, "angelX-cockpit");
    assert_eq!(m.version, "0.1.0");
    assert_eq!(m.edition, "2021");
    assert_eq!(m.bin, "angel");
}

#[test]
fn group_of_buckets_known_and_unknown() {
    assert_eq!(group_of("harness"), "Agent harness & tooling");
    assert_eq!(group_of("swarm"), "Drivers & orchestration");
    assert_eq!(group_of("memory_store"), "Memory & compaction");
    assert_eq!(group_of("brand_new_module"), "Other");
}

#[test]
fn module_source_stamp_changes_with_scanned_source() {
    let root = std::env::temp_dir().join(format!(
        "angel-self-context-stamp-{}-{:?}",
        std::process::id(),
        std::thread::current().id()
    ));
    let src = root.join("src");
    std::fs::create_dir_all(src.join("tools")).unwrap();
    let module = src.join("sample.rs");
    std::fs::write(&module, "//! first\npub struct A;\n").unwrap();
    let before = module_source_stamp(&root);
    std::fs::write(&module, "//! changed and longer\npub struct A;\n").unwrap();
    let after = module_source_stamp(&root);
    assert_ne!(before, after, "content metadata invalidates the cache");
    let _ = std::fs::remove_dir_all(root);
}

// --- Against the real tree (CARGO_MANIFEST_DIR is always set in tests). ---

#[test]
fn source_root_finds_this_crate() {
    let _guard = crate::tests::env_lock();
    let root = source_root().expect("must locate own crate during cargo test");
    assert!(root.join("Cargo.toml").is_file());
    assert!(root.join("src/main.rs").is_file());
    assert!(is_cockpit_root(&root));
}

#[test]
fn generate_self_model_describes_real_modules() {
    let _guard = crate::tests::env_lock();
    let map = generate_self_model();
    assert!(map.contains("angelX-cockpit"), "names the crate");
    assert!(map.contains("cargo test"), "has the build/test contract");
    assert!(map.contains("harness/mod.rs"), "lists the harness module");
    assert!(map.contains("swarm/mod.rs"), "lists the swarm driver");
    assert!(
        map.contains("src/agent/tools/"),
        "covers the tools submodule"
    );
    assert!(
        map.contains("ToolRegistry") || map.contains("Tool"),
        "surfaces a key public type from harness"
    );
}

#[test]
fn self_context_is_bounded_and_self_referential() {
    let _guard = crate::tests::env_lock();
    let ctx = self_context(Path::new(env!("CARGO_MANIFEST_DIR")));
    assert!(!ctx.is_empty(), "source is present during tests");
    assert!(ctx.contains("Self-model"));
    assert!(ctx.contains("self_map"));
    assert!(ctx.contains("self-modification gate"));
    assert!(ctx.contains("not a standing objective"));
    // No self-repair priming: naming the gauntlet/health-checkup is what let a
    // degraded model chase the forbidden artifact. The self-model must not carry
    // those nouns.
    let lower = ctx.to_ascii_lowercase();
    assert!(
        !lower.contains("gauntlet"),
        "self-model must not name a gauntlet"
    );
    assert!(
        !lower.contains("health checkup"),
        "self-model must not name a health checkup"
    );
    // Bounded: the compact injection must not balloon the preamble.
    assert!(
        ctx.len() < 16 * 1024,
        "injection stays bounded: {}",
        ctx.len()
    );
}

#[test]
fn self_context_disabled_by_env_flag() {
    // Saved/restored to avoid leaking into other tests in the same process.
    let _guard = crate::tests::env_lock();
    let prev = std::env::var_os("ANGEL_SELF_MODEL");
    // TODO: Audit that the environment access only happens in single-threaded code.
    unsafe { std::env::set_var("ANGEL_SELF_MODEL", "0") };
    let ctx = self_context(Path::new(env!("CARGO_MANIFEST_DIR")));
    match prev {
        // TODO: Audit that the environment access only happens in single-threaded code.
        Some(v) => unsafe { std::env::set_var("ANGEL_SELF_MODEL", v) },
        // TODO: Audit that the environment access only happens in single-threaded code.
        None => unsafe { std::env::remove_var("ANGEL_SELF_MODEL") },
    }
    assert_eq!(ctx, "", "ANGEL_SELF_MODEL=0 disables injection");
}

#[test]
fn headless_self_map_requires_explicit_pin_and_is_read_only() {
    let _guard = crate::tests::env_lock();
    struct RestorePin(Option<std::ffi::OsString>);
    impl Drop for RestorePin {
        fn drop(&mut self) {
            unsafe {
                match &self.0 {
                    Some(pin) => std::env::set_var("ANGEL_SELF_SRC", pin),
                    None => std::env::remove_var("ANGEL_SELF_SRC"),
                }
            }
        }
    }
    let _restore = RestorePin(std::env::var_os("ANGEL_SELF_SRC"));
    for pin in [None, Some(""), Some("  ")] {
        unsafe {
            match pin {
                Some(pin) => std::env::set_var("ANGEL_SELF_SRC", pin),
                None => std::env::remove_var("ANGEL_SELF_SRC"),
            }
        }
        let mut registry = crate::agent::harness::ToolRegistry::new();
        SelfMapTool::register_headless(&mut registry);
        assert!(!registry.defs().iter().any(|def| def.name == "self_map"));
    }

    let root = std::env::temp_dir().join(format!(
        "angel-headless-self-map-{}-{:?}",
        std::process::id(),
        std::thread::current().id()
    ));
    std::fs::create_dir_all(root.join("src")).unwrap();
    std::fs::write(
        root.join("Cargo.toml"),
        "[package]\nname = \"angelX-cockpit\"\nversion = \"9.8.7\"\n",
    )
    .unwrap();
    std::fs::write(
        root.join("src/main.rs"),
        "//! Pinned fixture source.\npub struct PinnedFixture;\n",
    )
    .unwrap();
    std::fs::write(root.join("SELF.md"), "operator sentinel").unwrap();
    let bound = SelfMapTool::read_only(root.clone());
    assert!(
        bound
            .read_only_root
            .as_ref()
            .unwrap()
            .as_ref()
            .unwrap()
            .is_absolute()
    );
    unsafe { std::env::set_var("ANGEL_SELF_SRC", &root) };
    let mut registry = crate::agent::harness::ToolRegistry::new();
    SelfMapTool::register_headless(&mut registry);
    let def = registry
        .defs()
        .into_iter()
        .find(|def| def.name == "self_map")
        .unwrap();
    assert!(def.description.contains("Read-only"));
    assert!(def.params["properties"].get("write").is_none());
    // Registration captures the explicit pin; later ambient changes cannot redirect it.
    unsafe { std::env::remove_var("ANGEL_SELF_SRC") };
    let outline = registry
        .dispatch("self_map", &serde_json::json!({"module": "main"}))
        .unwrap();
    assert!(outline.contains("PinnedFixture"), "{outline}");
    let map = registry
        .dispatch("self_map", &serde_json::json!({}))
        .unwrap();
    assert!(map.contains("9.8.7"), "map must come from the pinned root");
    for args in [
        serde_json::json!({"write": true}),
        serde_json::json!({"module": "main", "write": true}),
    ] {
        let error = registry.dispatch("self_map", &args).unwrap_err();
        assert!(error.contains("read-only"), "{error}");
    }
    assert_eq!(
        std::fs::read_to_string(root.join("SELF.md")).unwrap(),
        "operator sentinel"
    );
    std::fs::remove_file(root.join("SELF.md")).unwrap();
    assert!(
        registry
            .dispatch("self_map", &serde_json::json!({"write": true}))
            .is_err()
    );
    assert!(!root.join("SELF.md").exists());

    unsafe { std::env::set_var("ANGEL_SELF_SRC", root.join("missing")) };
    let mut invalid = crate::agent::harness::ToolRegistry::new();
    SelfMapTool::register_headless(&mut invalid);
    let error = invalid
        .dispatch("self_map", &serde_json::json!({}))
        .unwrap_err();
    assert!(error.contains("invalid ANGEL_SELF_SRC pin"), "{error}");
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn self_map_tool_returns_module_outline() {
    let _guard = crate::tests::env_lock();
    let tool = SelfMapTool::new();
    let out = tool
        .call(&serde_json::json!({"module": "harness/registry"}))
        .expect("module outline");
    assert!(out.contains("harness"));
    assert!(out.contains("Symbols:"));
    assert!(
        out.contains("ToolRegistry"),
        "outline surfaces a known symbol"
    );
}

#[test]
fn self_map_tool_full_map_by_default() {
    let _guard = crate::tests::env_lock();
    let tool = SelfMapTool::new();
    let out = tool.call(&serde_json::json!({})).expect("full map");
    assert!(out.contains("self-model"));
    assert!(out.contains("Build · test · run"));
}

#[test]
fn self_map_tool_rejects_path_escape() {
    let tool = SelfMapTool::new();
    assert!(
        tool.call(&serde_json::json!({"module": "../Cargo"}))
            .is_err()
    );
}

#[test]
fn module_outline_never_falls_back_outside_src() {
    let root = std::env::temp_dir().join(format!(
        "angel-self-map-boundary-{}-{:?}",
        std::process::id(),
        std::thread::current().id()
    ));
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir_all(root.join("src")).unwrap();
    std::fs::write(root.join("outside.rs"), "pub struct Outside;\n").unwrap();

    let error = module_outline(&root, "outside").expect_err("root-level file must be inert");
    assert!(error.contains("no such module under src/"), "{error}");

    #[cfg(unix)]
    {
        std::os::unix::fs::symlink(root.join("outside.rs"), root.join("src/escape.rs")).unwrap();
        let error = module_outline(&root, "escape").expect_err("outbound symlink must be inert");
        assert!(error.contains("outside src/"), "{error}");
    }

    let _ = std::fs::remove_dir_all(root);
}

// --- The Layer-2 gate (pure logic). ---

#[test]
fn gate_rejects_broken_build() {
    let v = evaluate_gate(
        false,
        &TestOutcome {
            passed: 10,
            failed: 0,
            ignored: 0,
        },
    );
    assert!(!v.passed);
    assert_eq!(v.reward, 0.0);
    assert!(v.summary.contains("does not build"));
}

#[test]
fn gate_green_when_builds_and_all_pass() {
    let v = evaluate_gate(
        true,
        &TestOutcome {
            passed: 42,
            failed: 0,
            ignored: 1,
        },
    );
    assert!(v.passed);
    assert_eq!(v.reward, 1.0);
    assert!(v.summary.contains("GREEN"));
}

#[test]
fn gate_rejects_when_a_test_fails() {
    let v = evaluate_gate(
        true,
        &TestOutcome {
            passed: 40,
            failed: 2,
            ignored: 0,
        },
    );
    assert!(!v.passed);
    assert!(v.reward < 1.0 && v.reward > 0.0);
    assert!(v.summary.contains("failed"));
}

#[test]
fn gate_rejects_when_no_tests_ran() {
    let v = evaluate_gate(true, &TestOutcome::default());
    assert!(
        !v.passed,
        "a build with zero tests is not a confirmed green"
    );
    assert!(v.summary.contains("no tests ran"));
}

#[test]
fn gate_baseline_guard_rejects_pass_count_regression() {
    let green = TestOutcome {
        passed: 3,
        failed: 0,
        ignored: 0,
    };
    // All green but fewer passes than the pre-edit baseline → tests were
    // deleted/disabled to fake a pass; rejected despite reward 1.0.
    let v = evaluate_gate_with_baseline(true, &green, Some(5));
    assert!(
        !v.passed,
        "pass-count regression must reject: {}",
        v.summary
    );
    assert!(v.summary.contains("baseline"));
    // Meeting or beating the baseline is a pass.
    assert!(evaluate_gate_with_baseline(true, &green, Some(3)).passed);
    assert!(evaluate_gate_with_baseline(true, &green, Some(2)).passed);
    // No baseline → plain gate semantics.
    assert!(evaluate_gate_with_baseline(true, &green, None).passed);
    // The guard never rescues a red gate.
    let red = TestOutcome {
        passed: 9,
        failed: 1,
        ignored: 0,
    };
    assert!(!evaluate_gate_with_baseline(true, &red, Some(1)).passed);
    assert!(!evaluate_gate_with_baseline(false, &green, None).passed);
}

#[cfg(unix)]
fn fake_cargo_workspace(name: &str, script: &str) -> (PathBuf, String) {
    use std::os::unix::fs::PermissionsExt;

    let root = std::env::temp_dir().join(format!(
        "angel-self-gate-{name}-{}-{:?}",
        std::process::id(),
        std::thread::current().id()
    ));
    let _ = std::fs::remove_dir_all(&root);
    let bin = root.join("bin");
    let workspace = root.join("workspace");
    std::fs::create_dir_all(&bin).unwrap();
    std::fs::create_dir_all(&workspace).unwrap();
    let cargo = bin.join("cargo");
    std::fs::write(&cargo, script).unwrap();
    std::fs::set_permissions(&cargo, std::fs::Permissions::from_mode(0o755)).unwrap();
    let path = format!(
        "{}:{}",
        bin.display(),
        std::env::var("PATH").unwrap_or_default()
    );
    (root, path)
}

#[cfg(unix)]
#[test]
fn self_gate_drains_noisy_build_before_running_tests() {
    let _guard = crate::tests::env_lock();
    let (root, path) = fake_cargo_workspace(
        "noisy",
        "#!/bin/sh\n\
             if [ \"$1\" = build ]; then\n\
               head -c 2097152 /dev/zero | tr '\\000' e >&2\n\
               exit 0\n\
             fi\n\
             printf 'test result: ok. 9 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s\\n'\n",
    );
    let _path = crate::tests::TestEnvGuard::set("PATH", &path);

    let (verdict, detail) = run_self_gate_with_baseline(&root.join("workspace"), Some(9));
    assert!(verdict.passed, "{}: {detail}", verdict.summary);
    assert!(
        detail.is_empty(),
        "green gate detail must be empty: {detail}"
    );
    let _ = std::fs::remove_dir_all(root);
}

#[cfg(unix)]
#[test]
fn self_gate_reaps_a_hung_build_tree_and_returns_red() {
    let _guard = crate::tests::env_lock();
    let (root, path) = fake_cargo_workspace(
        "hung",
        "#!/bin/sh\n\
             if [ \"$1\" = build ]; then\n\
               sleep 30 &\n\
               wait\n\
             fi\n",
    );
    let _path = crate::tests::TestEnvGuard::set("PATH", &path);
    let _idle = crate::tests::TestEnvGuard::set("ANGEL_TOOL_IDLE_FLOOR_SECS", "1");
    let _hard = crate::tests::TestEnvGuard::set("ANGEL_TOOL_HARD_TIMEOUT", "3");
    let started = std::time::Instant::now();

    let (verdict, detail) = run_self_gate_with_baseline(&root.join("workspace"), None);
    assert!(!verdict.passed, "hung build must not pass");
    assert!(
        verdict.summary.contains("does not build"),
        "{}",
        verdict.summary
    );
    assert!(
        detail.contains("execution deadline"),
        "timeout should be actionable: {detail:?}"
    );
    assert!(
        started.elapsed() < std::time::Duration::from_secs(5),
        "hung verifier was not bounded: {:?}",
        started.elapsed()
    );
    let _ = std::fs::remove_dir_all(root);
}
