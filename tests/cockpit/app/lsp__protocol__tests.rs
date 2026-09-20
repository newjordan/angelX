use super::*;
use serde_json::json;

#[test]
fn pull_report_normalizes_full_and_unchanged() {
    let full = json!({ "kind": "full", "items": [ { "severity": 1, "message": "x" } ] });
    let p = pull_report_to_params("file:///a.rs", &full);
    assert_eq!(p["uri"], "file:///a.rs");
    assert_eq!(p["diagnostics"].as_array().unwrap().len(), 1);
    let unchanged = json!({ "kind": "unchanged", "resultId": "1" });
    let p2 = pull_report_to_params("file:///a.rs", &unchanged);
    assert_eq!(p2["diagnostics"].as_array().unwrap().len(), 0); // no items -> clean
}
