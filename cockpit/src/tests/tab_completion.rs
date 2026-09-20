//! Tab-completion suites (module-breakup: extracted from the `tests.rs`
//! monolith). Unique-prefix commit, bounded ambiguity reports, ordered skill
//! selector stacks, confined quarantine-blind mention completion, UTF-8-safe
//! prefixes, and the agent-only cycle outside slash input.

use super::seed_preview_app;
use crate::app_control;
use crate::club::Bag;
use crate::harness;
use crate::tests::{TestEnvGuard, env_lock};
use crate::turn::Thinking;
use std::sync::Arc;

#[test]
fn tab_completes_unique_slash_prefixes_even_while_a_turn_runs() {
    use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

    let mut app = seed_preview_app();
    app.thinking = Some(Thinking::pending_for_test("practice"));
    app.input = "/sav".to_string();
    app.cursor = app.input.chars().count();
    let messages = app.messages.len();

    app.on_key(KeyEvent::new(KeyCode::Tab, KeyModifiers::NONE));

    assert_eq!(app.input, "/save");
    assert_eq!(app.cursor, app.input.chars().count());
    assert_eq!(app.messages.len(), messages);
    assert!(app.thinking.is_some());
}

#[test]
fn tab_reports_ambiguous_and_unknown_slash_prefixes_boundedly() {
    use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

    let mut ambiguous = seed_preview_app();
    ambiguous.input = "/red".to_string();
    ambiguous.cursor = ambiguous.input.chars().count();
    ambiguous.on_key(KeyEvent::new(KeyCode::Tab, KeyModifiers::NONE));
    assert_eq!(ambiguous.input, "/red");
    let receipt = &ambiguous.messages.last().unwrap().text;
    assert!(receipt.contains("/redo"), "{receipt}");
    assert!(receipt.contains("/redraw"), "{receipt}");
    assert!(receipt.chars().count() < 180, "{receipt}");
    let messages = ambiguous.messages.len();
    ambiguous.on_key(KeyEvent::new(KeyCode::Tab, KeyModifiers::NONE));
    assert_eq!(
        ambiguous.messages.len(),
        messages,
        "repeated Tab must not spam an identical ambiguity receipt"
    );

    let mut unknown = seed_preview_app();
    unknown.input = "/definitely-not-a-command".to_string();
    unknown.cursor = unknown.input.chars().count();
    unknown.on_key(KeyEvent::new(KeyCode::Tab, KeyModifiers::NONE));
    assert!(
        unknown
            .messages
            .last()
            .unwrap()
            .text
            .contains("no command matches")
    );
}

#[test]
fn skill_selector_tab_completion_composes_ordered_local_stacks() {
    use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

    let _env = env_lock();
    let root = std::env::temp_dir().join(format!(
        "angel_skill_complete_{}_{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir_all(&root).unwrap();
    for name in ["zqx-stack-lint", "zqx-stack-review", "zqx-stack-repro"] {
        std::fs::write(root.join(format!("{name}.md")), "bounded instructions").unwrap();
    }
    let root_text = root.to_string_lossy();
    let _skills = TestEnvGuard::set("ANGEL_SKILLS_DIR", &root_text);

    let mut app = seed_preview_app();
    app.thinking = Some(Thinking::pending_for_test("practice"));
    app.input = "/skills zqx-stack-li".to_string();
    app.cursor = app.input.chars().count();
    app.on_key(KeyEvent::new(KeyCode::Tab, KeyModifiers::NONE));
    assert_eq!(app.input, "/skills zqx-stack-lint");
    assert!(app.skill_completion_catalog.is_some());

    std::fs::write(
        root.join("zqy-cache-new.md"),
        "appeared after the first completion scan",
    )
    .unwrap();
    app.input = "/skills zqy-cache-ne".to_string();
    app.cursor = app.input.chars().count();
    app.on_key(KeyEvent::new(KeyCode::Tab, KeyModifiers::NONE));
    assert_eq!(
        app.input, "/skills zqy-cache-ne",
        "consecutive Tabs reuse one catalog snapshot"
    );
    app.on_key(KeyEvent::new(KeyCode::Char('w'), KeyModifiers::NONE));
    assert_eq!(app.input, "/skills zqy-cache-new");
    assert!(
        app.skill_completion_catalog.is_none(),
        "a real composer edit invalidates the completion snapshot"
    );
    app.on_key(KeyEvent::new(KeyCode::Tab, KeyModifiers::NONE));
    assert!(
        app.skill_completion_catalog
            .as_ref()
            .is_some_and(|catalog| catalog.iter().any(|name| name == "zqy-cache-new"))
    );

    app.input = "/skills zqx-stack-lint,zqx-stack-".to_string();
    app.cursor = app.input.chars().count();
    app.on_key(KeyEvent::new(KeyCode::Tab, KeyModifiers::NONE));
    assert_eq!(app.input, "/skills zqx-stack-lint,zqx-stack-re");
    app.on_key(KeyEvent::new(KeyCode::Tab, KeyModifiers::NONE));
    let receipt = &app.messages.last().unwrap().text;
    assert!(receipt.contains("zqx-stack-review"), "{receipt}");
    assert!(receipt.contains("zqx-stack-repro"), "{receipt}");

    app.input = "/skills zqx-stack-lint,zqx-stack-review fix it".to_string();
    app.cursor = app.input.chars().count();
    app.on_key(KeyEvent::new(KeyCode::Tab, KeyModifiers::NONE));
    assert_eq!(app.input, "/skills zqx-stack-lint,zqx-stack-review fix it");
    assert!(
        app.thinking.is_some(),
        "skill completion must stay local mid-turn"
    );
    app.interrupt();
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn mention_tab_completion_is_confined_bounded_and_quarantine_blind() {
    use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

    let root = std::env::temp_dir().join(format!(
        "angel_mention_complete_{}_{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    let outside = root.with_extension("outside");
    let _ = std::fs::remove_dir_all(&root);
    let _ = std::fs::remove_dir_all(&outside);
    std::fs::create_dir_all(root.join("src")).unwrap();
    std::fs::create_dir_all(root.join("docs")).unwrap();
    std::fs::create_dir_all(root.join("off-limits")).unwrap();
    std::fs::write(root.join("src/main.rs"), "fn main() {}\n").unwrap();
    std::fs::write(root.join("src/markdown.rs"), "pub fn render() {}\n").unwrap();
    for index in 0..70 {
        std::fs::write(root.join(format!("cap-{index:02}.txt")), "bounded\n").unwrap();
    }
    #[cfg(unix)]
    std::fs::write(root.join("bad\nname.txt"), "hidden from completion\n").unwrap();
    std::fs::create_dir_all(&outside).unwrap();
    std::fs::write(outside.join("secret.rs"), "outside\n").unwrap();
    #[cfg(unix)]
    std::os::unix::fs::symlink(&outside, root.join("escape")).unwrap();

    let root_matches = app_control::mention_path_matches(&root, "").unwrap();
    assert!(root_matches.len() <= 64);
    assert_eq!(
        app_control::mention_path_matches(&root, "do").unwrap(),
        vec!["docs/".to_string()]
    );
    assert!(
        app_control::mention_path_matches(&root, "off")
            .unwrap()
            .is_empty(),
        "quarantined root must not be suggested"
    );
    assert!(
        app_control::mention_path_matches(&root, "bad")
            .unwrap()
            .is_empty(),
        "control-bearing names must not be suggested"
    );
    assert_eq!(
        app_control::mention_path_matches(&root, "cap-")
            .unwrap()
            .len(),
        64,
        "completion result set must stay bounded"
    );
    assert!(app_control::mention_path_matches(&root, "../").is_err());
    assert!(app_control::mention_path_matches(&root, "off-limits/").is_err());
    assert!(app_control::mention_path_matches(&root, "/tmp").is_err());
    #[cfg(unix)]
    assert!(
        app_control::mention_path_matches(&root, "escape/").is_err(),
        "outbound symlink directories must stay unavailable"
    );

    let mut app = seed_preview_app();
    app.tools = Arc::new(harness::ToolRegistry::with_team(root.clone(), Vec::new()));
    app.thinking = Some(Thinking::pending_for_test("practice"));
    app.input = "/mention src/main".to_string();
    app.cursor = app.input.chars().count();
    app.on_key(KeyEvent::new(KeyCode::Tab, KeyModifiers::NONE));
    assert_eq!(app.input, "/mention src/main.rs");
    assert!(
        app.thinking.is_some(),
        "completion must stay local mid-turn"
    );

    app.input = "/mention src/ma".to_string();
    app.cursor = app.input.chars().count();
    app.on_key(KeyEvent::new(KeyCode::Tab, KeyModifiers::NONE));
    assert_eq!(app.input, "/mention src/ma");
    let receipt = &app.messages.last().unwrap().text;
    assert!(receipt.contains("src/main.rs"), "{receipt}");
    assert!(receipt.contains("src/markdown.rs"), "{receipt}");
    assert!(app.workspace_completion_cache.is_some());

    std::fs::write(root.join("src/map.rs"), "pub fn map() {}\n").unwrap();
    app.on_key(KeyEvent::new(KeyCode::Tab, KeyModifiers::NONE));
    assert_eq!(app.input, "/mention src/ma");
    assert!(
        app.workspace_completion_cache
            .as_ref()
            .is_some_and(|cache| !cache.matches.iter().any(|path| path == "src/map.rs")),
        "consecutive Tabs must reuse one directory snapshot"
    );

    app.on_key(KeyEvent::new(KeyCode::Char('p'), KeyModifiers::NONE));
    assert_eq!(app.input, "/mention src/map");
    assert!(
        app.workspace_completion_cache.is_none(),
        "a real edit must invalidate the directory snapshot"
    );
    app.on_key(KeyEvent::new(KeyCode::Tab, KeyModifiers::NONE));
    assert_eq!(
        app.input, "/mention src/map.rs",
        "the next completion interaction must see the new file"
    );

    app.input = "/diagnostics src/main".to_string();
    app.cursor = app.input.chars().count();
    app.on_key(KeyEvent::new(KeyCode::Tab, KeyModifiers::NONE));
    assert_eq!(app.input, "/diagnostics src/main.rs");
    assert!(
        app.thinking.is_some(),
        "diagnostics completion must stay local mid-turn"
    );

    app.input = "/diagnostics src/ma".to_string();
    app.cursor = app.input.chars().count();
    app.on_key(KeyEvent::new(KeyCode::Tab, KeyModifiers::NONE));
    assert_eq!(app.input, "/diagnostics src/ma");
    let receipt = &app.messages.last().unwrap().text;
    assert!(receipt.starts_with("diagnostics matches"), "{receipt}");
    assert!(receipt.contains("src/main.rs"), "{receipt}");
    assert!(receipt.contains("src/markdown.rs"), "{receipt}");

    app.input = "/symbols src/main".to_string();
    app.cursor = app.input.chars().count();
    app.on_key(KeyEvent::new(KeyCode::Tab, KeyModifiers::NONE));
    assert_eq!(app.input, "/symbols src/main.rs");
    assert!(
        app.thinking.is_some(),
        "symbol completion must stay local mid-turn"
    );

    app.input = "/definition src/main".to_string();
    app.cursor = app.input.chars().count();
    app.on_key(KeyEvent::new(KeyCode::Tab, KeyModifiers::NONE));
    assert_eq!(app.input, "/definition src/main.rs");
    assert!(
        app.thinking.is_some(),
        "definition path completion must stay local mid-turn"
    );
    app.input.push_str(" main");
    app.cursor = app.input.chars().count();
    app.on_key(KeyEvent::new(KeyCode::Tab, KeyModifiers::NONE));
    assert_eq!(
        app.input, "/definition src/main.rs main",
        "Tab must leave the symbol argument untouched"
    );
    for command in ["references", "hover"] {
        app.input = format!("/{command} src/main");
        app.cursor = app.input.chars().count();
        app.on_key(KeyEvent::new(KeyCode::Tab, KeyModifiers::NONE));
        assert_eq!(app.input, format!("/{command} src/main.rs"));
        app.input.push_str(" main");
        app.cursor = app.input.chars().count();
        app.on_key(KeyEvent::new(KeyCode::Tab, KeyModifiers::NONE));
        assert_eq!(
            app.input,
            format!("/{command} src/main.rs main"),
            "Tab must leave {command}'s symbol argument untouched"
        );
    }
    assert!(app.workspace_completion_cache.is_some());
    app.on_paste("");
    assert!(
        app.workspace_completion_cache.is_none(),
        "paste boundaries invalidate the completion snapshot even when empty"
    );
    app.interrupt();

    let _ = std::fs::remove_dir_all(&root);
    let _ = std::fs::remove_dir_all(&outside);
}

#[test]
fn mention_path_common_prefix_is_utf8_safe() {
    assert_eq!(
        app_control::path_longest_common_prefix(&[
            "src/界面.rs".to_string(),
            "src/界限.rs".to_string(),
        ]),
        "src/界"
    );
}

#[test]
fn tab_cycles_agents_only_outside_slash_command_input() {
    use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

    let mut app = seed_preview_app();
    app.bag = Bag::for_render_test(&[
        ("alpha", &[("model-a", true)]),
        ("beta", &[("model-b", true)]),
    ]);
    assert_eq!(app.bag.in_hand_label(), "alpha");
    app.input = "/mention src/lib.rs".to_string();
    app.cursor = app.input.chars().count();
    app.on_key(KeyEvent::new(KeyCode::Tab, KeyModifiers::NONE));
    assert_eq!(app.bag.in_hand_label(), "alpha");
    assert_eq!(app.input, "/mention src/lib.rs");

    app.input.clear();
    app.cursor = 0;
    app.on_key(KeyEvent::new(KeyCode::Tab, KeyModifiers::NONE));
    assert_eq!(app.bag.in_hand_label(), "beta");
}
