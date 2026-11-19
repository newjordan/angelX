use super::*;

#[test]
fn bounded_keeps_short_text_whole() {
    assert_eq!(bounded("hello", 100), "hello");
    assert_eq!(bounded("hello", 5), "hello");
}

#[test]
fn bounded_elides_the_middle_and_marks_it() {
    let text: String = (0..1000).map(|n| format!("{n:04}")).collect();
    let cut = bounded(&text, 200);
    assert!(cut.chars().count() <= 200, "{}", cut.chars().count());
    assert!(cut.starts_with("0000"), "{cut}");
    assert!(cut.ends_with("0999"), "{cut}");
    assert!(cut.contains("elided"), "{cut}");
}

#[test]
fn bounded_never_splits_a_character() {
    let text = "é".repeat(500);
    let cut = bounded(&text, 100);
    assert!(cut.chars().count() <= 100);
    assert!(cut.chars().all(|c| c == 'é' || ELISION.contains(c)));
}

#[test]
fn standing_context_inlines_named_files_and_skips_missing_ones() {
    let _guard = crate::tests::env_lock();
    let _files = crate::tests::TestEnvGuard::set(
        "ANGEL_LOOP_CONTEXT_FILES",
        "NOTES.md, MISSING.md",
    );
    let _chars = crate::tests::TestEnvGuard::unset("ANGEL_LOOP_CONTEXT_CHARS");
    let dir = std::env::temp_dir().join(format!("angel-loop-context-{}", std::process::id()));
    std::fs::create_dir_all(&dir).expect("temp dir");
    std::fs::write(dir.join("NOTES.md"), "journal line one\njournal line two\n").expect("journal");
    let block = standing_context(&dir);
    std::fs::remove_dir_all(&dir).ok();
    assert!(block.contains("[standing context: NOTES.md]"), "{block}");
    assert!(block.contains("journal line two"), "{block}");
    assert!(!block.contains("MISSING"), "{block}");
}

#[test]
fn standing_context_is_empty_when_unconfigured() {
    let _guard = crate::tests::env_lock();
    let _files = crate::tests::TestEnvGuard::unset("ANGEL_LOOP_CONTEXT_FILES");
    assert_eq!(standing_context(Path::new("/nonexistent")), "");
}
