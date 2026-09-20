use super::*;

#[test]
fn empty_query_is_rejected_not_fanned() {
    let t = ScienceTool;
    assert!(t.call(&serde_json::json!({ "query": "  " })).is_err());
    assert!(t.call(&serde_json::json!({})).is_err());
}

#[test]
fn oversized_query_is_rejected_before_network_dispatch() {
    let t = ScienceTool;
    let query = "x".repeat(crate::drive::science::MAX_QUERY_CHARS + 1);
    let err = t.call(&serde_json::json!({ "query": query })).unwrap_err();
    assert!(err.contains("at most 500 characters"), "{err}");
}

#[test]
fn def_advertises_the_name_and_query_param() {
    let d = ScienceTool.def();
    assert_eq!(d.name, "science_search");
    assert_eq!(ScienceTool.name(), d.name);
    assert!(d.params["properties"]["query"].is_object());
    assert_eq!(d.params["properties"]["query"]["maxLength"], 500);
}
