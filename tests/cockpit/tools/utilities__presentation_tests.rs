use super::*;

#[test]
fn show_work_present_preserves_identity_and_only_acknowledges_queueing() {
    let workspace = std::env::temp_dir().join("angel-present-project");
    std::fs::create_dir_all(&workspace).unwrap();
    let tool = PresentTool::new(&workspace);
    let result = tool
        .call(&serde_json::json!({
            "kind": "report", "label": "Measured result", "url": "reports/result.md"
        }))
        .unwrap();
    let (kind, label, url) = crate::ui::media::presentation_from_result(&result).unwrap();
    assert_eq!(kind, "resource");
    assert_eq!(label, "Measured result");
    assert_eq!(url, workspace.join("reports/result.md").to_string_lossy());
    assert!(!result.contains("presented"));
    assert!(
        tool.call(&serde_json::json!({ "kind": "invalid", "label": "x", "url": "x" }))
            .is_err()
    );
    assert!(
        tool.call(&serde_json::json!({ "kind": "resource", "label": "x", "url": "" }))
            .is_err()
    );
    assert!(crate::ui::media::presentation_from_result("error: missing target").is_none());
    assert!(
        crate::ui::media::presentation_from_result(
            r#"{"status":"queued","kind":"invalid","label":"x","url":"x"}"#
        )
        .is_none()
    );
}
