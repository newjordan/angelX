//! Real angel process; the provider and hung corpus server are loopback fixtures.
mod tests {
    pub fn env_lock() -> std::sync::MutexGuard<'static, ()> {
        static LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());
        LOCK.lock().unwrap_or_else(|e| e.into_inner())
    }
}

#[test]
fn task_mode_http_deadline_includes_partial_receipt() {
    let _guard = crate::tests::env_lock();
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap();
    let output = std::process::Command::new("python3")
        .arg(root.join("scripts/astra-w01-http-proof.py"))
        .args(["--angel-bin", env!("CARGO_BIN_EXE_angel")])
        .output()
        .unwrap();
    println!("{}", String::from_utf8_lossy(&output.stdout));
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
}
