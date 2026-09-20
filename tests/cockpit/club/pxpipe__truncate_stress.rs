use super::truncate_for_log;

#[test]
fn never_splits_a_multibyte_char() {
    // Subprocess stderr is arbitrary UTF-8; a byte budget landing inside a
    // multi-byte char used to panic. Try every budget across a run of 2- and
    // 3-byte chars — none may panic.
    let s: String = "é字".repeat(300); // é = 2 bytes, 字 = 3 bytes
    for max in 1..s.len() {
        let out = truncate_for_log(&s, max);
        assert!(out.is_empty() || out.ends_with("...") || out == s);
    }
    // Short input is returned verbatim.
    assert_eq!(truncate_for_log("hi", 100), "hi");
}
