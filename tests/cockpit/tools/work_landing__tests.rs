use super::*;

#[cfg(unix)]
#[test]
fn live_detection_capture_uses_the_in_process_deadline() {
    let started = std::time::Instant::now();
    assert!(run_capture("sh", &["-c", "sleep 30"], Path::new("."), 0).is_none());
    assert!(
        started.elapsed() < std::time::Duration::from_secs(2),
        "work landing must not fall back to an unbounded child"
    );
}

/// A unique throwaway dir under the system temp dir (no tempfile dep, no env).
fn temp_dir(tag: &str) -> PathBuf {
    static N: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
    let n = N.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
    let pid = std::process::id();
    let dir = std::env::temp_dir().join(format!("angel-worklanding-{tag}-{pid}-{n}"));
    let _ = std::fs::remove_dir_all(&dir);
    dir
}

// --- git remote normalization (ssh + https → owner/repo). ---

#[test]
fn normalize_remote_ssh_scp_form() {
    let s = normalize_remote("git@github.com:newjordan/angelX.git").unwrap();
    assert_eq!(s.slug(), "newjordan/angelX");
    assert_eq!(s.host, "github.com");
    assert!(s.is_github());
}

#[test]
fn normalize_remote_https_form_with_and_without_dot_git() {
    assert_eq!(
        normalize_remote("https://github.com/newjordan/angelX.git")
            .unwrap()
            .slug(),
        "newjordan/angelX"
    );
    assert_eq!(
        normalize_remote("https://github.com/newjordan/angelX")
            .unwrap()
            .slug(),
        "newjordan/angelX"
    );
}

#[test]
fn normalize_remote_ssh_url_form_and_trailing_slash() {
    assert_eq!(
        normalize_remote("ssh://git@github.com/acme/Tool.git")
            .unwrap()
            .slug(),
        "acme/Tool"
    );
    assert_eq!(
        normalize_remote("https://github.com/acme/Tool/")
            .unwrap()
            .slug(),
        "acme/Tool"
    );
}

#[test]
fn normalize_remote_enterprise_host_is_not_github() {
    let s = normalize_remote("git@git.corp.internal:team/svc.git").unwrap();
    assert_eq!(s.slug(), "team/svc");
    assert_eq!(s.host, "git.corp.internal");
    assert!(!s.is_github());
}

#[test]
fn normalize_remote_rejects_garbage_and_local_paths() {
    assert!(normalize_remote("").is_none());
    assert!(normalize_remote("   ").is_none());
    assert!(normalize_remote("not a url").is_none());
    assert!(normalize_remote("/srv/git/repo.git").is_none()); // local path, no host
    assert!(normalize_remote("https://github.com/only-owner").is_none()); // no repo segment
}

// --- gh visibility parsing. ---

#[test]
fn parse_gh_visibility_maps_known_values() {
    assert_eq!(parse_gh_visibility("PUBLIC\n"), Visibility::Public);
    assert_eq!(parse_gh_visibility("private"), Visibility::Private);
    assert_eq!(parse_gh_visibility("INTERNAL"), Visibility::Private);
    assert_eq!(parse_gh_visibility(""), Visibility::Unknown);
    assert_eq!(parse_gh_visibility("huh?"), Visibility::Unknown);
}

// --- visibility → mode inference. ---

#[test]
fn infer_mode_covers_all_cases() {
    assert_eq!(infer_mode(Visibility::Public, true), Mode::PublicFacing);
    assert_eq!(infer_mode(Visibility::Private, true), Mode::InternalDev);
    // unknown but a remote exists → ask (unset)
    assert_eq!(infer_mode(Visibility::Unknown, true), Mode::Unset);
    // unknown and purely local → internal-dev (nothing is exposed)
    assert_eq!(infer_mode(Visibility::Unknown, false), Mode::InternalDev);
}

// --- assemble_context graceful tiers (mock git/gh as strings). ---

#[test]
fn assemble_public_repo_is_public_facing() {
    let ctx = assemble_context(
        "/w",
        Some("git@github.com:org/app.git"),
        Some("PUBLIC"),
        None,
    );
    assert_eq!(ctx.repo.as_deref(), Some("org/app"));
    assert_eq!(ctx.visibility, Visibility::Public);
    assert_eq!(ctx.mode, Mode::PublicFacing);
    assert!(!ctx.confirmed);
}

#[test]
fn assemble_private_repo_is_internal_dev() {
    let ctx = assemble_context(
        "/w",
        Some("https://github.com/org/secret.git"),
        Some("PRIVATE"),
        None,
    );
    assert_eq!(ctx.visibility, Visibility::Private);
    assert_eq!(ctx.mode, Mode::InternalDev);
}

#[test]
fn assemble_missing_gh_with_remote_is_unset() {
    // Remote exists but gh is unavailable → can't classify → ask.
    let ctx = assemble_context("/w", Some("git@github.com:org/app.git"), None, None);
    assert_eq!(ctx.repo.as_deref(), Some("org/app"));
    assert_eq!(ctx.visibility, Visibility::Unknown);
    assert_eq!(ctx.mode, Mode::Unset);
}

#[test]
fn assemble_no_remote_is_local_internal_dev() {
    // No remote at all (not a repo / no origin) → folder-only, local internal-dev.
    let ctx = assemble_context("/w", None, None, None);
    assert!(ctx.repo.is_none());
    assert_eq!(ctx.visibility, Visibility::Unknown);
    assert_eq!(ctx.mode, Mode::InternalDev);
}

#[test]
fn assemble_non_github_remote_skips_visibility_and_asks() {
    // A local-path remote doesn't parse to a slug → repo unknown, but a remote
    // string is present → mode unset (we shouldn't assume internal).
    let ctx = assemble_context("/w", Some("/srv/git/repo.git"), None, None);
    assert!(ctx.repo.is_none());
    assert_eq!(ctx.mode, Mode::Unset);
}

#[test]
fn assemble_preserves_confirmed_prior_when_unchanged() {
    let prior = WorkContext {
        folder: "/old".into(),
        repo: Some("org/app".into()),
        visibility: Visibility::Public,
        mode: Mode::PublicFacing,
        confirmed: true,
        updated_at: 123,
    };
    let ctx = assemble_context(
        "/w",
        Some("git@github.com:org/app.git"),
        Some("PUBLIC"),
        Some(&prior),
    );
    assert!(
        ctx.confirmed,
        "unchanged repo+visibility keeps confirmation"
    );
    assert_eq!(ctx.mode, Mode::PublicFacing);
    assert_eq!(ctx.folder, "/w", "folder still refreshed to the live path");
}

#[test]
fn assemble_resets_confirmation_when_repo_changes() {
    let prior = WorkContext {
        folder: "/old".into(),
        repo: Some("org/old".into()),
        visibility: Visibility::Public,
        mode: Mode::PublicFacing,
        confirmed: true,
        updated_at: 1,
    };
    // /cd into a different repo → must re-land.
    let ctx = assemble_context(
        "/w",
        Some("git@github.com:org/new.git"),
        Some("PRIVATE"),
        Some(&prior),
    );
    assert!(!ctx.confirmed, "a different repo re-triggers onboarding");
    assert_eq!(ctx.repo.as_deref(), Some("org/new"));
    assert_eq!(ctx.mode, Mode::InternalDev);
}

// --- persistence roundtrip (explicit dir → no env mutation). ---

#[test]
fn save_load_roundtrip() {
    let dir = temp_dir("roundtrip");
    let ws = Path::new("/home/u/projects/widget");
    let ctx = WorkContext {
        folder: ws.to_string_lossy().to_string(),
        repo: Some("u/widget".into()),
        visibility: Visibility::Private,
        mode: Mode::InternalDev,
        confirmed: true,
        updated_at: 42,
    };
    let path = save_context_in(&dir, ws, &ctx).expect("save");
    assert!(path.exists());
    let loaded = load_context_in(&dir, ws).expect("load");
    assert_eq!(loaded, ctx);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn load_missing_context_is_none() {
    let dir = temp_dir("missing");
    assert!(load_context_in(&dir, Path::new("/no/such/ws")).is_none());
}

#[test]
fn workspace_key_is_deterministic_and_distinct() {
    let a = workspace_key(Path::new("/home/u/alpha"));
    let a2 = workspace_key(Path::new("/home/u/alpha"));
    let b = workspace_key(Path::new("/home/u/beta"));
    assert_eq!(a, a2, "same path → same key (stable file)");
    assert_ne!(a, b, "different paths → different keys (no collision)");
    assert!(!a.contains('/'), "key is filesystem-safe");
}

// --- system-prompt block rendering. ---

#[test]
fn render_block_active_for_confirmed_context() {
    let ctx = WorkContext {
        folder: "/w".into(),
        repo: Some("org/app".into()),
        visibility: Visibility::Public,
        mode: Mode::PublicFacing,
        confirmed: true,
        updated_at: 0,
    };
    let block = render_block(Some(&ctx));
    assert!(block.contains("Active work context"));
    assert!(block.contains("mode=public-facing"));
    assert!(
        block.contains("NEVER commit secrets"),
        "injects mode guidance"
    );
}

#[test]
fn render_block_internal_dev_guidance() {
    let ctx = WorkContext {
        folder: "/w".into(),
        repo: None,
        visibility: Visibility::Private,
        mode: Mode::InternalDev,
        confirmed: true,
        updated_at: 0,
    };
    let block = render_block(Some(&ctx));
    assert!(block.contains("mode=internal-dev"));
    assert!(block.contains("velocity"), "internal-dev guidance present");
}

#[test]
fn render_block_not_established_for_none() {
    let block = render_block(None);
    assert!(block.contains("not established"));
    assert!(
        block.contains("work_landing"),
        "tells the agent to self-direct"
    );
    assert!(block.contains("confirm=true"));
}

#[test]
fn render_block_unconfirmed_shows_detected_hint() {
    let ctx = WorkContext {
        folder: "/w".into(),
        repo: Some("org/app".into()),
        visibility: Visibility::Unknown,
        mode: Mode::Unset,
        confirmed: false,
        updated_at: 0,
    };
    let block = render_block(Some(&ctx));
    assert!(block.contains("not established"));
    assert!(block.contains("Detected so far"));
    assert!(block.contains("org/app"));
}

#[test]
fn mode_and_visibility_label_parse_roundtrip() {
    assert_eq!(Mode::parse_label("public-facing"), Some(Mode::PublicFacing));
    assert_eq!(Mode::parse_label("internal_dev"), Some(Mode::InternalDev));
    assert_eq!(Mode::parse_label("nonsense"), None);
    assert_eq!(
        Visibility::parse_label("Private"),
        Some(Visibility::Private)
    );
    assert_eq!(
        Visibility::parse_label("internal"),
        Some(Visibility::Private)
    );
    assert_eq!(Visibility::parse_label("weird"), None);
}
