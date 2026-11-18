use super::*;

#[test]
fn pathological_double_star_pattern_terminates_and_is_correct() {
    let pattern = format!("{}nope.rs", "**/".repeat(24));
    assert!(!glob_match(&pattern, "a/b/c/d/e/f/g/h/i/j/k/l/m/n/o/p.rs"));

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
    let pattern = format!("{}b", "a*".repeat(30));
    let text = "a".repeat(60);
    assert!(!glob_match(&pattern, &text));
}
