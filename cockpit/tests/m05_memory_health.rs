//! Actual --task-json process with a loopback scripted provider, never a model API.
mod tests {
    pub fn env_lock() -> std::sync::MutexGuard<'static, ()> {
        static LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());
        LOCK.lock().unwrap_or_else(|e| e.into_inner())
    }
}

#[test]
fn m05_task_json_exposes_corruption_in_counts_stderr_and_ledger() {
    let _guard = crate::tests::env_lock();
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap();
    let output = std::process::Command::new("python3")
        .arg(root.join("scripts/m05-taskjson-health-receipt.py"))
        .args(["--angel-bin", env!("CARGO_BIN_EXE_angel")])
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "stdout={}\nstderr={}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    println!("{}", String::from_utf8_lossy(&output.stdout));
}
