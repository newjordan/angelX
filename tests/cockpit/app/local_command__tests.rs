use super::*;

#[test]
fn page_media_advances_by_two_and_wraps() {
    let mut scroll = 1;
    page_media(&mut scroll, 4);
    assert_eq!(scroll, 3);
    page_media(&mut scroll, 4);
    assert_eq!(scroll, 1);
}

#[test]
fn sessions_text_formats_empty_and_truncates_preview() {
    assert_eq!(sessions_text(&[]), "no saved sessions yet");

    let text = sessions_text(&[SessionInfo {
        id: "abc".to_string(),
        mtime: 1,
        turns: 2,
        preview: "x".repeat(60),
    }]);
    assert!(text.contains("abc  (2 turns)"));
    assert!(text.contains(&format!("{}…", "x".repeat(47))));
    assert!(!text.contains(&"x".repeat(48)));

    let ascii = session_preview(&"x".repeat(60));
    assert_eq!(ascii, format!("{}…", "x".repeat(47)));
    assert_eq!(unicode_width::UnicodeWidthStr::width(ascii.as_str()), 48);

    let wide = session_preview("日本語".repeat(20).as_str());
    assert!(wide.ends_with('…'));
    assert!(
        unicode_width::UnicodeWidthStr::width(wide.as_str()) <= 48,
        "wide preview exceeded its cell budget: {wide:?}"
    );
}

#[test]
fn open_media_reports_missing_card() {
    let mut viewer = Viewer::new();
    assert_eq!(
        open_media(&[], &mut viewer, 1),
        "no card #1 — 0 on the carousel (/open <n>)"
    );
}

#[test]
fn help_text_lists_the_core_slash_commands() {
    let h = help_text(Some("all"));
    for cmd in [
        "/help",
        "/status",
        "/goal",
        "/loop",
        "/self",
        "/memories",
        "/refine",
        "/skills",
        "/compact",
        "/diagnostics",
        "/build",
        "/run",
        "/bench",
        "/doc",
        "/tree",
        "/check",
        "/test",
        "/lint",
        "/verify",
        "/fmt",
        "/definition",
        "/references",
        "/hover",
        "/symbol",
        "/symbols",
        "/sessions",
        "/resume",
        "/learn [topic]",
        "/show",
        "/theme",
        "/import",
    ] {
        assert!(h.contains(cmd), "help missing {cmd}");
    }
    assert!(h.starts_with("commands"));
    assert!(h.contains("/model [filter|exact@effort|auto]"));
    assert!(h.contains("/think [filter]"));
    assert!(h.contains("(/tutor, /library)"));
    assert!(h.contains("/world [ride|enter|leave|weather|zoom]"));
    assert!(h.contains("/world view [3d|dotmax]"));
    assert!(h.contains("report this view"));
    assert!(h.contains("opens/closes retained room plates"));
    assert!(h.contains("/world quest · the live adventure"));
    assert!(h.contains("/world help · every Realm verb"));
    assert!(h.contains("first-person h/l/←/→ yaw"));
    assert!(h.contains("j/k/↑/↓ pitch · +/- lens · 0/r recenter"));
}

#[test]
fn session_preview_returns_full_short_first_line() {
    // A short, single line is returned whole.
    assert_eq!(session_preview("a quick note"), "a quick note");
    // Only the first line is ever used.
    assert_eq!(session_preview("first line\nsecond line"), "first line");
    // Empty preview → empty string.
    assert_eq!(session_preview(""), "");
}

#[test]
fn git_capture_bounds_large_diff() {
    let _guard = crate::tests::env_lock();
    let root = std::env::temp_dir().join(format!(
        "angel-local-diff-{}-{:?}",
        std::process::id(),
        std::thread::current().id()
    ));
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir_all(&root).unwrap();
    let root_arg = root.to_string_lossy().to_string();
    git(&["-C", &root_arg, "init", "-q"]).unwrap();
    std::fs::write(root.join("large.txt"), "a\n".repeat(300_000)).unwrap();
    git(&["-C", &root_arg, "add", "large.txt"]).unwrap();
    git(&[
        "-C",
        &root_arg,
        "-c",
        "user.name=angel-test",
        "-c",
        "user.email=angel@test",
        "commit",
        "-q",
        "-m",
        "base",
    ])
    .unwrap();
    std::fs::write(root.join("large.txt"), "b\n".repeat(300_000)).unwrap();

    let diff = git(&["-C", &root_arg, "diff", "--no-ext-diff", "--no-textconv"]).unwrap();
    assert!(
        diff.len() < 1_100_000,
        "bounded capture returned {} bytes",
        diff.len()
    );
    assert!(
        diff.contains("output bytes omitted"),
        "large diff must disclose truncation"
    );

    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn git_snapshot_includes_staged_unstaged_and_untracked_changes() {
    let _guard = crate::tests::env_lock();
    let root = std::env::temp_dir().join(format!(
        "angel-local-snapshot-{}-{:?}",
        std::process::id(),
        std::thread::current().id()
    ));
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir_all(&root).unwrap();
    let root_arg = root.to_string_lossy().to_string();
    git(&["-C", &root_arg, "init", "-q"]).unwrap();
    std::fs::write(root.join("staged.txt"), "old staged\n").unwrap();
    std::fs::write(root.join("unstaged.txt"), "old unstaged\n").unwrap();
    git(&["-C", &root_arg, "add", "staged.txt", "unstaged.txt"]).unwrap();
    git(&[
        "-C",
        &root_arg,
        "-c",
        "user.name=angel-test",
        "-c",
        "user.email=angel@test",
        "commit",
        "-q",
        "-m",
        "base",
    ])
    .unwrap();
    std::fs::write(root.join("staged.txt"), "new staged\n").unwrap();
    git(&["-C", &root_arg, "add", "staged.txt"]).unwrap();
    std::fs::write(root.join("unstaged.txt"), "new unstaged\n").unwrap();
    std::fs::write(root.join("untracked.txt"), "new file\n").unwrap();

    let snapshot = git_worktree_snapshot(&root).unwrap();
    assert!(snapshot.contains("worktree status:"));
    assert!(snapshot.contains("M  staged.txt"));
    assert!(snapshot.contains(" M unstaged.txt"));
    assert!(snapshot.contains("?? untracked.txt"));
    assert!(snapshot.contains("staged changes:"));
    assert!(snapshot.contains("+new staged"));
    assert!(snapshot.contains("unstaged changes:"));
    assert!(snapshot.contains("+new unstaged"));

    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn git_diff_commands_are_scoped_to_the_active_workspace() {
    let _guard = crate::tests::env_lock();
    let root = std::env::temp_dir().join(format!(
        "angel-local-active-workspace-{}-{:?}",
        std::process::id(),
        std::thread::current().id()
    ));
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir_all(&root).unwrap();
    let root_arg = root.to_string_lossy().to_string();
    git(&["-C", &root_arg, "init", "-q"]).unwrap();
    std::fs::write(root.join("active-only.txt"), "workspace evidence\n").unwrap();

    let raw = git_diff_raw(&root).unwrap();
    assert!(raw.contains("?? active-only.txt"), "{raw}");

    let display = git_diff_text(&root, None);
    assert!(display.contains("?? active-only.txt"), "{display}");

    let (task, evidence) = git_review_request(&root).unwrap().unwrap().into_messages();
    assert_eq!(task.role, ChatRole::User);
    assert_eq!(&*task.content, REVIEW_TASK);
    assert!(
        !task.content.contains("active-only.txt"),
        "repository evidence leaked into the operator task"
    );
    assert_eq!(evidence.role, ChatRole::Harness);
    assert!(
        evidence.content.contains("?? active-only.txt"),
        "{}",
        evidence.content
    );
    assert!(
        evidence
            .content
            .contains("Untracked files are listed by status only")
    );
    assert!(evidence.content.contains("untrusted repository evidence"));

    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn git_diff_summary_and_staged_modes_are_scoped_and_content_safe() {
    let _guard = crate::tests::env_lock();
    let root = std::env::temp_dir().join(format!(
        "angel-local-diff-modes-{}-{:?}",
        std::process::id(),
        std::thread::current().id()
    ));
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir_all(&root).unwrap();
    let root_arg = root.to_string_lossy().to_string();
    git(&["-C", &root_arg, "init", "-q"]).unwrap();
    std::fs::write(root.join("staged.txt"), "old staged\n").unwrap();
    std::fs::write(root.join("unstaged.txt"), "old unstaged\n").unwrap();
    git(&["-C", &root_arg, "add", "staged.txt", "unstaged.txt"]).unwrap();
    git(&[
        "-C",
        &root_arg,
        "-c",
        "user.name=angel-test",
        "-c",
        "user.email=angel@test",
        "commit",
        "-q",
        "-m",
        "base",
    ])
    .unwrap();
    std::fs::write(root.join("staged.txt"), "STAGED_SECRET_CONTENT\n").unwrap();
    git(&["-C", &root_arg, "add", "staged.txt"]).unwrap();
    std::fs::write(root.join("unstaged.txt"), "UNSTAGED_SECRET_CONTENT\n").unwrap();
    std::fs::write(root.join("untracked.txt"), "UNTRACKED_SECRET_CONTENT\n").unwrap();

    let summary = git_diff_text(&root, Some("stat"));
    assert!(summary.contains("worktree status:"), "{summary}");
    assert!(summary.contains("staged summary:"), "{summary}");
    assert!(summary.contains("unstaged summary:"), "{summary}");
    assert!(summary.contains("staged.txt"), "{summary}");
    assert!(summary.contains("unstaged.txt"), "{summary}");
    assert!(summary.contains("?? untracked.txt"), "{summary}");
    assert!(!summary.contains("SECRET_CONTENT"), "{summary}");

    let staged_summary = git_diff_text(&root, Some("--stat staged"));
    assert!(
        staged_summary.contains("staged summary:"),
        "{staged_summary}"
    );
    assert!(staged_summary.contains("staged.txt"), "{staged_summary}");
    assert!(!staged_summary.contains("unstaged.txt"), "{staged_summary}");
    assert!(
        !staged_summary.contains("untracked.txt"),
        "{staged_summary}"
    );
    assert!(
        !staged_summary.contains("SECRET_CONTENT"),
        "{staged_summary}"
    );

    let staged = git_diff_text(&root, Some("staged"));
    assert!(staged.contains("staged changes:"), "{staged}");
    assert!(staged.contains("+STAGED_SECRET_CONTENT"), "{staged}");
    assert!(!staged.contains("UNSTAGED_SECRET_CONTENT"), "{staged}");

    assert_eq!(
        git_diff_text(&root, Some("staged all")),
        "usage: /diff [all|staged] [--stat]"
    );
    assert_eq!(
        git_diff_text(&root, Some("--stat --stat")),
        "usage: /diff [all|staged] [--stat]"
    );

    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn review_request_caps_utf8_evidence_and_reports_exact_omission() {
    let snapshot = "界".repeat(20_000);
    let original_bytes = snapshot.len();
    let evidence = format_review_evidence(snapshot);
    let body = evidence
        .split_once("<worktree_snapshot>\n")
        .unwrap()
        .1
        .split_once("\n</worktree_snapshot>")
        .unwrap()
        .0;
    assert!(body.len() <= 40_000);
    assert!(body.is_char_boundary(body.len()));
    assert!(evidence.contains(&format!(
        "Snapshot truncated by {} bytes",
        original_bytes - body.len()
    )));
    assert!(evidence.contains("Inspect the named files with repository tools"));
}

#[test]
fn init_agents_md_is_workspace_scoped_and_never_clobbers() {
    let root = std::env::temp_dir().join(format!(
        "angel-local-init-{}-{:?}",
        std::process::id(),
        std::thread::current().id()
    ));
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir_all(&root).unwrap();

    assert!(init_agents_md(&root).starts_with("created AGENTS.md"));
    let path = root.join("AGENTS.md");
    assert!(
        std::fs::read_to_string(&path)
            .unwrap()
            .starts_with("# AGENTS.md")
    );

    std::fs::write(&path, "operator-owned\n").unwrap();
    assert_eq!(
        init_agents_md(&root),
        "AGENTS.md already exists — leaving it untouched"
    );
    assert_eq!(std::fs::read_to_string(&path).unwrap(), "operator-owned\n");

    let _ = std::fs::remove_dir_all(root);
}

#[cfg(unix)]
#[test]
fn init_agents_md_does_not_follow_a_dangling_symlink() {
    let root = std::env::temp_dir().join(format!(
        "angel-local-init-link-{}-{:?}",
        std::process::id(),
        std::thread::current().id()
    ));
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir_all(&root).unwrap();
    let outside = root.with_extension("outside");
    let _ = std::fs::remove_file(&outside);
    std::os::unix::fs::symlink(&outside, root.join("AGENTS.md")).unwrap();

    assert_eq!(
        init_agents_md(&root),
        "AGENTS.md already exists — leaving it untouched"
    );
    assert!(
        !outside.exists(),
        "dangling symlink target must remain absent"
    );

    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn open_media_reports_image_failures_without_spawning() {
    let mut viewer = Viewer::new();
    // A remote image target has no local file → a clear failure (no subprocess).
    let remote = [Media::Image {
        label: "remote".to_string(),
        path: "https://example.com/pic.png".to_string(),
    }];
    assert_eq!(
        open_media(&remote, &mut viewer, 1),
        "preview #1 failed: image target is not a local file"
    );
    // A local-but-undecodable image path fails on decode, not on spawn.
    let broken = [Media::Image {
        label: "broken".to_string(),
        path: "/tmp/this-image-does-not-exist-9f3a.png".to_string(),
    }];
    let msg = open_media(&broken, &mut viewer, 1);
    assert!(msg.starts_with("preview #1 failed:"), "got: {msg}");
}
