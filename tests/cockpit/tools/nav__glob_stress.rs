use super::*;

#[test]
fn pathological_double_star_pattern_terminates_and_is_correct() {
    // 24 `**` segments + a non-matching literal against a deep path: the old
    // recursive matcher explored ~C(depth+24,24) failing combinations and
    // could wedge the turn for minutes. The iterative matcher is O(n*m) and
    // returns immediately. (If this test ever hangs, the blow-up is back.)
    let pattern = format!("{}nope.rs", "**/".repeat(24));
    assert!(!glob_match(&pattern, "a/b/c/d/e/f/g/h/i/j/k/l/m/n/o/p.rs"));

    // Ordinary glob semantics preserved.
    assert!(glob_match("**/*.rs", "src/a/b/main.rs"));
    assert!(glob_match("src/*.toml", "src/Cargo.toml"));
    assert!(!glob_match("src/*.toml", "src/a/Cargo.toml"));
    assert!(glob_match("a/**/z", "a/z"));
    assert!(glob_match("a/**/z", "a/b/c/z"));
    assert!(glob_match("a?c", "abc"));
    assert!(!glob_match("a?c", "ac"));
}

#[test]
fn pathological_star_segment_terminates() {
    // `a*a*...*b` against a long run of `a` with no trailing `b`: the old
    // `*` branch-recursion was exponential; the two-pointer form is linear.
    let pattern = format!("{}b", "a*".repeat(30));
    let text = "a".repeat(60);
    assert!(!glob_match(&pattern, &text));
}
