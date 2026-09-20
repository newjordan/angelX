//! Compiles the ordinary binary and exercises the loopback-only research fixture.
mod tests {
    pub fn env_lock() -> std::sync::MutexGuard<'static, ()> {
        static LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());
        LOCK.lock().unwrap_or_else(|e| e.into_inner())
    }
}

#[test]
fn research_compose_scripted_loop_smoke() {
    let _guard = crate::tests::env_lock();
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap();
    let scratch = root
        .join(".astra-tmp")
        .join(format!("research-smoke-{}", std::process::id()));
    std::fs::create_dir_all(&scratch).unwrap();
    let output = std::process::Command::new("python3")
        .arg(root.join("scripts/research-grounding-cohort.py"))
        .args(["--provider", "scripted-loop", "--smoke", "--angel-bin"])
        .arg(env!("CARGO_BIN_EXE_angel"))
        .arg("--out")
        .arg(scratch.join("receipts"))
        .env("TMPDIR", &scratch)
        .env("PYTHONDONTWRITEBYTECODE", "1")
        .output()
        .unwrap();
    println!("{}", String::from_utf8_lossy(&output.stdout));
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(String::from_utf8_lossy(&output.stdout).contains("\"smoke_ok\": true"));
}

#[test]
fn research_compose_scripted_disclosure_and_supported_inverse() {
    let _guard = crate::tests::env_lock();
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap();
    let scratch = root
        .join(".astra-tmp")
        .join(format!("research-disclosure-{}", std::process::id()));
    std::fs::create_dir_all(&scratch).unwrap();
    let output = std::process::Command::new("python3")
        .arg(root.join("scripts/research-grounding-cohort.py"))
        .args([
            "--provider",
            "scripted-disclosure",
            "--smoke",
            "--angel-bin",
        ])
        .arg(env!("CARGO_BIN_EXE_angel"))
        .arg("--out")
        .arg(scratch.join("receipts"))
        .env("TMPDIR", &scratch)
        .env("PYTHONDONTWRITEBYTECODE", "1")
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "stdout={} stderr={}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    let rows = std::fs::read_to_string(scratch.join("receipts/rows.jsonl")).unwrap();
    let rows: Vec<serde_json::Value> = rows
        .lines()
        .map(|line| serde_json::from_str(line).unwrap())
        .collect();
    assert_eq!(rows.len(), 3);
    for row in rows {
        assert_eq!(row["correct"], true);
        if row["id"] == "q01" {
            assert_eq!(row["cited"], serde_json::json!(["nx-410"]));
            assert!(
                row["answer_excerpt"]
                    .as_str()
                    .unwrap()
                    .starts_with("17.4 kHz (source: ")
            );
        } else {
            assert_eq!(row["cited"], serde_json::json!([]));
            assert_eq!(
                row["answer_excerpt"],
                "No evidence in the corpus supports an answer to this question."
            );
            assert_eq!(row["fetched"].as_array().unwrap().len(), 2);
        }
    }
}
