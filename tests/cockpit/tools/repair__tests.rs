use super::*;

#[test]
fn token_masking_hides_command_credentials() {
    let masked = mask_token("username=newjordan\npassword=gho_secret\n  - Token: gho_secret");
    assert!(masked.contains("username=newjordan"));
    assert!(masked.contains("password=<hidden>"));
    assert!(masked.contains("Token: <hidden>"));
    assert!(!masked.contains("gho_secret"));
}

#[cfg(unix)]
#[test]
fn repair_command_text_has_a_fixed_hung_tree_deadline() {
    let started = std::time::Instant::now();
    let error = command_text_with_timeout(
        Path::new("sh"),
        &["-c", "sleep 30 & wait"],
        Path::new("/tmp"),
        Duration::from_millis(50),
    )
    .unwrap_err();

    assert!(error.contains("timed out after 50ms"), "{error}");
    assert!(
        started.elapsed() < Duration::from_secs(2),
        "repair probe did not clean up its hung tree: {:?}",
        started.elapsed()
    );
}

#[test]
fn snap_launcher_detection_handles_real_snap_symlink_shape() {
    let dir = std::env::temp_dir().join(format!("angel_snap_{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let gh = dir.join("gh");
    std::os::unix::fs::symlink("/usr/bin/snap", &gh).unwrap();
    assert!(is_snap_launcher(&gh));
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn repaired_path_puts_user_bins_first() {
    let home = std::env::temp_dir().join(format!("angel_home_{}", std::process::id()));
    let current = std::env::join_paths([Path::new("/snap/bin"), Path::new("/usr/bin")]).unwrap();
    let path = repaired_path_from(&home, current);
    let parts: Vec<PathBuf> = std::env::split_paths(&path).collect();
    assert_eq!(parts.first(), Some(&home.join(".local/bin")));
    assert_eq!(parts.get(1), Some(&home.join("bin")));
    assert!(parts.contains(&PathBuf::from("/snap/bin")));
}

#[test]
fn normalizes_empty_tool_to_auto() {
    assert_eq!(normalize_tool(""), "auto");
    assert_eq!(normalize_tool("/gh"), "gh");
}

#[test]
fn default_call_is_diagnose_only() {
    // No repair arg → diagnose only: the generic report says repairs were
    // skipped, and no mutation branch runs.
    let tool = ToolRepairTool::new(std::env::temp_dir());
    let out = tool
        .call(&serde_json::json!({ "tool": "gh" }))
        .expect("diagnose runs");
    assert!(
        out.contains("repair: skipped (repair=false)"),
        "default must be diagnose-only:\n{out}"
    );
}

#[test]
fn session_budget_stops_repeated_calls() {
    // Default limit is 2: calls 1 and 2 run, the 3rd is refused. Set the env
    // explicitly (save/restore) so an external value can't skew the count.
    let _guard = crate::tests::env_lock();
    let prev = std::env::var_os("ANGEL_TOOL_REPAIR_LIMIT");
    // TODO: Audit that the environment access only happens in single-threaded code.
    unsafe { std::env::set_var("ANGEL_TOOL_REPAIR_LIMIT", "2") };
    let tool = ToolRepairTool::new(std::env::temp_dir());
    let args = serde_json::json!({ "tool": "gh" });
    assert!(tool.call(&args).is_ok(), "call 1 within budget");
    assert!(tool.call(&args).is_ok(), "call 2 within budget");
    let third = tool.call(&args);
    match prev {
        // TODO: Audit that the environment access only happens in single-threaded code.
        Some(v) => unsafe { std::env::set_var("ANGEL_TOOL_REPAIR_LIMIT", v) },
        // TODO: Audit that the environment access only happens in single-threaded code.
        None => unsafe { std::env::remove_var("ANGEL_TOOL_REPAIR_LIMIT") },
    }
    let err = third.expect_err("call 3 exhausts the session budget");
    assert!(
        err.contains("session budget exhausted"),
        "budget error text: {err}"
    );
}
