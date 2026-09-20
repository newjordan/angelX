use super::*;

#[test]
fn empty_query_is_rejected_not_fanned() {
    let t = RepoSearchTool;
    assert!(t.call(&serde_json::json!({ "query": "  " })).is_err());
    assert!(t.call(&serde_json::json!({})).is_err());
}

#[test]
fn oversized_query_is_rejected_before_network_dispatch() {
    let t = RepoSearchTool;
    let query = "x".repeat(crate::repos::MAX_QUERY_CHARS + 1);
    let err = t.call(&serde_json::json!({ "query": query })).unwrap_err();
    assert!(err.contains("at most 256 characters"), "{err}");
}

#[test]
fn def_advertises_the_name_query_and_mode() {
    let d = RepoSearchTool.def();
    assert_eq!(d.name, "repo_search");
    assert_eq!(RepoSearchTool.name(), d.name);
    assert!(d.params["properties"]["query"].is_object());
    assert_eq!(d.params["properties"]["query"]["maxLength"], 256);
    assert!(d.params["properties"]["mode"].is_object());
}
