use super::*;

fn fixture_registry() -> (PathBuf, ToolRegistry) {
    let unique = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let root = std::env::temp_dir().join(format!(
        "angel_task_recon_{}_{}",
        std::process::id(),
        unique
    ));
    std::fs::create_dir_all(root.join("src")).unwrap();
    std::fs::write(
        root.join("src/needlewidget.rs"),
        "fn needlewidget_contract() -> bool { true }\n",
    )
    .unwrap();
    let mut registry = ToolRegistry::new();
    register_file_tools(&mut registry, root.clone());
    let names = registry.bindable_tool_names();
    registry.register(Box::new(CodeModeTool::new(&names)));
    (root, registry)
}

#[test]
fn placebo_mask_preserves_every_utf8_byte() {
    let original = "alpha Ωmega 四 / punctuation:{}";
    let masked = mask_alphanumeric_same_bytes(original);
    assert_eq!(masked.len(), original.len());
    assert_ne!(masked, original);
    assert_eq!(masked.matches('/').count(), 1);
}

#[test]
fn truncation_never_splits_utf8() {
    assert_eq!(truncate_utf8("aΩz", 2), "a");
    assert_eq!(truncate_utf8("aΩz", 3), "aΩ");
}

#[test]
fn code_mode_task_recon_real_and_placebo_do_identical_bounded_work() {
    let (root, registry) = fixture_registry();
    let hooks = Hooks::default();
    let prompt = "repair the NeedleWidget contract";
    let real =
        task_recon_context_with(&registry, prompt, TaskReconMode::Repo, 12 * 1024, &hooks).unwrap();
    let real_calls = registry
        .gauge
        .preturn_code_mode_nested_calls
        .load(Ordering::Relaxed);
    let real_bytes = registry
        .gauge
        .preturn_code_mode_nested_output_bytes
        .load(Ordering::Relaxed);
    let placebo =
        task_recon_context_with(&registry, prompt, TaskReconMode::Placebo, 12 * 1024, &hooks)
            .unwrap();
    let placebo_calls = registry
        .gauge
        .preturn_code_mode_nested_calls
        .load(Ordering::Relaxed)
        .saturating_sub(real_calls);
    let placebo_bytes = registry
        .gauge
        .preturn_code_mode_nested_output_bytes
        .load(Ordering::Relaxed)
        .saturating_sub(real_bytes);
    assert_eq!(real.len(), placebo.len());
    assert_eq!(real_calls, placebo_calls);
    assert_eq!(real_bytes, placebo_bytes);
    assert!(real.starts_with(TASK_RECON_PREFIX));
    assert!(placebo.starts_with(TASK_RECON_PREFIX));
    assert!(real.contains("angel-repo-recon/v2"), "{real}");
    assert!(real.contains("symbol_map"), "{real}");
    assert!(real.contains("src/needlewidget.rs"), "{real}");
    assert!(!placebo.contains("needlewidget"), "{placebo}");
    assert_eq!(
        registry
            .gauge
            .preturn_code_mode_recipe_calls
            .load(Ordering::Relaxed),
        2
    );
    assert!(
        registry
            .gauge
            .preturn_code_mode_nested_calls
            .load(Ordering::Relaxed)
            >= 6
    );
    std::fs::remove_dir_all(root).ok();
}

#[test]
fn code_mode_task_recon_bounds_wide_results_and_excludes_outbound_symlinks() {
    let _guard = crate::tests::env_lock();
    let _handles = crate::tests::TestEnvGuard::set("ANGEL_HANDLE_STORE", "0");
    let (root, registry) = fixture_registry();
    for index in 0..20 {
        std::fs::write(
            root.join("src").join(format!("candidate_{index}.rs")),
            format!("fn needlewidget_{index}() {{}}\n"),
        )
        .unwrap();
    }
    std::fs::create_dir_all(root.join("off-limits/legacy-product")).unwrap();
    std::fs::write(
        root.join("off-limits/legacy-product/forbidden.rs"),
        "QUARANTINED_NEEDLEWIDGET_MARKER",
    )
    .unwrap();
    let outside = std::env::temp_dir().join(format!(
        "angel_task_recon_outside_{}_{}",
        std::process::id(),
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    std::fs::write(&outside, "EXTERNAL_NEEDLEWIDGET_SECRET").unwrap();
    #[cfg(unix)]
    std::os::unix::fs::symlink(&outside, root.join("src/leak.rs")).unwrap();

    let context = task_recon_context_with(
        &registry,
        "NeedleWidget",
        TaskReconMode::Repo,
        12 * 1024,
        &Hooks::default(),
    )
    .unwrap();
    let evidence: Value = serde_json::from_str(context.strip_prefix(TASK_RECON_PREFIX).unwrap())
        .expect("recipe output is structured JSON");
    assert!(evidence["candidates"].as_array().unwrap().len() <= 8);
    assert!(!context.contains("EXTERNAL_NEEDLEWIDGET_SECRET"));
    assert!(!context.contains("QUARANTINED_NEEDLEWIDGET_MARKER"));
    assert!(!context.contains("off-limits"));
    std::fs::remove_dir_all(root).ok();
    std::fs::remove_file(outside).ok();
}

#[test]
fn code_mode_task_recon_message_keeps_harness_origin() {
    let (root, registry) = fixture_registry();
    let message = task_recon_message_with(
        &registry,
        "NeedleWidget",
        TaskReconMode::Repo,
        1024,
        &Hooks::default(),
    )
    .unwrap();
    assert_eq!(message.role, ChatRole::Harness);
    assert_ne!(message.role, ChatRole::System);
    assert_ne!(message.role, ChatRole::User);
    std::fs::remove_dir_all(root).ok();
}

#[test]
fn code_mode_task_recon_respects_context_cap_and_disabled_tool() {
    let (root, registry) = fixture_registry();
    let context = task_recon_context_with(
        &registry,
        "NeedleWidget",
        TaskReconMode::Repo,
        256,
        &Hooks::default(),
    )
    .unwrap();
    assert!(context.len() <= 256);
    let empty = ToolRegistry::new();
    assert!(
        task_recon_context_with(
            &empty,
            "NeedleWidget",
            TaskReconMode::Repo,
            256,
            &Hooks::default(),
        )
        .is_none()
    );
    std::fs::remove_dir_all(root).ok();
}

#[test]
fn code_mode_task_recon_metrics_transfer_exactly_once() {
    let (root, registry) = fixture_registry();
    let context = task_recon_context_with(
        &registry,
        "NeedleWidget",
        TaskReconMode::Repo,
        4096,
        &Hooks::default(),
    )
    .unwrap();
    let first = registry.take_preturn_code_mode_metrics();
    assert_eq!(first.calls, 1);
    assert_eq!(first.recipe_calls, 1);
    assert!(first.nested_calls >= 2);
    assert!(first.nested_output_bytes > 0);
    assert_eq!(first.policy_rejections, 0);
    assert_eq!(first.task_recon_context_bytes, context.len());
    assert_eq!(
        registry.take_preturn_code_mode_metrics(),
        PreturnCodeModeMetrics::default()
    );
    std::fs::remove_dir_all(root).ok();
}

#[test]
fn code_mode_task_recon_dense_fixture_nested_work_baseline() {
    let _guard = crate::tests::env_lock();
    let _handles = crate::tests::TestEnvGuard::set("ANGEL_HANDLE_STORE", "0");
    let (root, registry) = fixture_registry();
    for index in 0..8 {
        std::fs::write(
            root.join("src").join(format!("dense_{index}.rs")),
            format!("fn alpha_beta_gamma_delta_{index}() {{}}\n"),
        )
        .unwrap();
    }
    let context = task_recon_context_with(
        &registry,
        "Alpha Beta Gamma Delta",
        TaskReconMode::Repo,
        12 * 1024,
        &Hooks::default(),
    )
    .unwrap();
    let metrics = registry.take_preturn_code_mode_metrics();
    // v2: list+grep (2) + enough direct candidates to skip the sparse-only
    // definition fallback + outlines for the top hits (8) = 10.
    assert_eq!(metrics.nested_calls, 10);
    assert!(context.contains("\"filename_fallback\":false"), "{context}");
    assert!(context.contains("\"symbol_map\""), "{context}");
    assert!(metrics.nested_calls <= 24);
    std::fs::remove_dir_all(root).ok();
}

#[test]
fn code_mode_task_recon_sparse_fixture_uses_bounded_filename_fallback() {
    let (root, registry) = fixture_registry();
    let context = task_recon_context_with(
        &registry,
        "AbsentWidget AbsentProtocol",
        TaskReconMode::Repo,
        12 * 1024,
        &Hooks::default(),
    )
    .unwrap();
    let metrics = registry.take_preturn_code_mode_metrics();
    // v2: list+grep (2) + one multi-symbol definition scan (1) + bounded
    // file_search fallback for the two terms (2) = 5.
    assert_eq!(metrics.nested_calls, 5);
    assert!(context.contains("\"filename_fallback\":true"), "{context}");
    assert!(context.contains("\"symbol_map\""), "{context}");
    assert!(metrics.nested_output_bytes <= 4 * 1024 * 1024);
    std::fs::remove_dir_all(root).ok();
}
