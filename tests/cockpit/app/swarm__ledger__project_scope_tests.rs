use super::*;

#[test]
fn legacy_and_foreign_rows_are_inert_for_project_reports() {
    let alpha = std::path::Path::new("/tmp/moa-alpha/repo");
    let beta = std::path::Path::new("/tmp/moa-beta/repo");
    let identity = crate::workspace_store::repo_identity(alpha);
    let bound = serde_json::json!({
        "repo": {"root": identity.root, "key": identity.key},
        "tokens": {},
    });
    assert!(record_matches_workspace(&bound, alpha));
    assert!(!record_matches_workspace(&bound, beta));
    assert!(!record_matches_workspace(
        &serde_json::json!({"tokens": {}}),
        alpha
    ));
}
