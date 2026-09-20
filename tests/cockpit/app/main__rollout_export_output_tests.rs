use super::write_private_export;

#[test]
fn explicit_export_path_is_private_and_never_overwritten() {
    let path = std::env::temp_dir().join(format!(
        "angel-rollout-export-{}-{}.json",
        std::process::id(),
        crate::cut::sha256_hex(b"private-export-fixture")
    ));
    let _ = std::fs::remove_file(&path);
    write_private_export(
        path.to_str().unwrap(),
        &serde_json::json!({"schema": "fixture/v1"}),
    )
    .unwrap();
    assert!(write_private_export(path.to_str().unwrap(), &serde_json::json!({})).is_err());
    assert_eq!(
        serde_json::from_slice::<serde_json::Value>(&std::fs::read(&path).unwrap()).unwrap()["schema"],
        "fixture/v1"
    );
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        assert_eq!(
            std::fs::metadata(&path).unwrap().permissions().mode() & 0o777,
            0o600
        );
    }
    let _ = std::fs::remove_file(path);
}
