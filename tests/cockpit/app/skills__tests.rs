use super::*;

#[test]
fn list_and_load_from_a_skills_dir() {
    // Mutates ANGEL_SKILLS_DIR (process-global env) — serialize with every
    // other env-touching test so the parallel runner can't race the environ
    // table (harness/tests/skills.rs sets the same var under this lock).
    let _guard = crate::tests::env_lock();
    let tmp = std::env::temp_dir().join(format!("angel_skills_{}", std::process::id()));
    let _ = std::fs::create_dir_all(&tmp);
    std::fs::write(
        tmp.join("unique-test-review-skill.md"),
        "Be a strict reviewer.",
    )
    .unwrap();
    std::fs::write(
        tmp.join("unique-test-rust-debug.md"),
        "Diagnose zqxcompilerneedle failures without guessing.\u{1b}[31m",
    )
    .unwrap();
    for index in 0..30 {
        std::fs::write(
            tmp.join(format!("unique-test-search-cap-{index:02}.md")),
            "Apply zqxcompilerneedle verification.",
        )
        .unwrap();
    }
    std::fs::write(tmp.join("notes.txt"), "ignored").unwrap();
    let old_user = std::env::var_os("ANGEL_SKILLS_DIR");
    // TODO: Audit that the environment access only happens in single-threaded code.
    unsafe { std::env::set_var("ANGEL_SKILLS_DIR", &tmp) };
    let workspace = std::env::current_dir().unwrap();
    let names = list_for(&workspace);
    let by_name = search_for(&workspace, "rust debug");
    let by_description = search_for(&workspace, "zqxcompilerneedle");
    let no_match = search_for(&workspace, "definitely-absent-query");
    restore_env("ANGEL_SKILLS_DIR", old_user);
    let _ = std::fs::remove_dir_all(&tmp);
    assert!(
        names.contains(&"unique-test-review-skill".to_string()),
        "test skill missing from catalog: {names:?}"
    );
    assert!(by_name.contains("unique-test-rust-debug"), "{by_name}");
    assert!(
        by_description.contains("unique-test-rust-debug"),
        "{by_description}"
    );
    assert!(!by_description.contains('\u{1b}'), "{by_description:?}");
    assert!(by_description.contains("31 match(es)"), "{by_description}");
    assert!(
        by_description.contains("7 additional match(es) omitted"),
        "{by_description}"
    );
    assert_eq!(by_description.lines().count(), 27, "{by_description}");
    assert!(
        no_match.contains("no installed skill matched"),
        "{no_match}"
    );
    assert_eq!(search_for(&workspace, ""), "usage: /skills search <query>");
}

fn restore_env(key: &str, old: Option<std::ffi::OsString>) {
    match old {
        // TODO: Audit that the environment access only happens in single-threaded code.
        Some(v) => unsafe { std::env::set_var(key, v) },
        // TODO: Audit that the environment access only happens in single-threaded code.
        None => unsafe { std::env::remove_var(key) },
    }
}
