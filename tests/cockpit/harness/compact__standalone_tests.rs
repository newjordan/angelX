#[test]
fn tool_aging_excerpt_preserves_durable_digest() {
    let digest = "a".repeat(64);
    let receipt = format!(
        "{TOOL_AGED_MARK}: shell|check (3000 bytes){TOOL_EXCERPT_MARK}head … tail: tail — re-run the tool if needed] [trajectory-sha256:{digest}]"
    );
    let stripped = strip_excerpt_receipt(&receipt).unwrap();
    assert!(!stripped.contains(TOOL_EXCERPT_MARK));
    assert!(stripped.contains(&format!(" [trajectory-sha256:{digest}]")));
    assert!(stripped.ends_with(" — re-run the tool if needed]"));
    assert!(stripped.contains("(3000 bytes)"));
    assert!(strip_excerpt_receipt(&stripped).is_none());
    assert!(strip_excerpt_receipt(&receipt.replace(&digest, "invalid")).is_none());
}
