use super::truncate_for_log;

#[test]
fn never_splits_a_multibyte_char() {
    let s: String = "é字".repeat(300);
    for max in 1..s.len() {
        let out = truncate_for_log(&s, max);
        assert!(out.is_empty() || out.ends_with("...") || out == s);
    }
    assert_eq!(truncate_for_log("hi", 100), "hi");
}
