use super::*;
use std::path::Path;

fn repo(name: &str, stars: u64, pushed: &str, archived: bool) -> Repo {
    Repo {
        full_name: name.to_string(),
        description: String::new(),
        language: Some("Rust".to_string()),
        stars,
        forks: 0,
        open_issues: 0,
        pushed_at: Some(pushed.to_string()),
        topics: vec![],
        archived,
        url: Some(format!("https://github.com/{name}")),
    }
}

#[test]
fn pushed_ordinal_parses_iso_and_orders_chronologically() {
    let a = pushed_ordinal("2020-01-01T00:00:00Z").unwrap();
    let b = pushed_ordinal("2026-07-19T12:00:00Z").unwrap();
    assert!(b > a, "later push has the larger ordinal");
    // Reference epoch: 1970-01-01 is day 0.
    assert_eq!(pushed_ordinal("1970-01-01T00:00:00Z"), Some(0));
    assert_eq!(pushed_ordinal("bogus"), None);
    assert_eq!(pushed_ordinal("2026-13-40T00:00:00Z"), None);
    assert_eq!(pushed_ordinal("2026/07/19"), None);
}

#[test]
fn best_mode_ranks_stars_first() {
    let _lock = crate::tests::env_lock();
    let repos = vec![
        repo("a/low-stars-fresh", 5, "2026-07-19T00:00:00Z", false),
        repo("b/high-stars-stale", 50_000, "2024-01-01T00:00:00Z", false),
    ];
    let folded = fold(repos, Mode::Best).unwrap();
    assert_eq!(
        folded[0].full_name, "b/high-stars-stale",
        "stars dominate best mode"
    );
}

#[test]
fn latest_mode_ranks_recency_first() {
    let _lock = crate::tests::env_lock();
    let repos = vec![
        repo("a/low-stars-fresh", 5, "2026-07-19T00:00:00Z", false),
        repo("b/high-stars-stale", 50_000, "2020-01-01T00:00:00Z", false),
    ];
    let folded = fold(repos, Mode::Latest).unwrap();
    assert_eq!(
        folded[0].full_name, "a/low-stars-fresh",
        "recency dominates latest mode"
    );
}

#[test]
fn archived_repos_sink_beneath_live_peers() {
    let _lock = crate::tests::env_lock();
    let repos = vec![
        repo("a/archived", 10_000, "2026-07-19T00:00:00Z", true),
        repo("b/live", 9_000, "2026-07-19T00:00:00Z", false),
    ];
    let folded = fold(repos, Mode::Best).unwrap();
    assert_eq!(
        folded[0].full_name, "b/live",
        "a live repo outranks a more-starred archived one"
    );
}

#[test]
fn fold_dedupes_by_full_name_keeping_the_better_starred() {
    let _lock = crate::tests::env_lock();
    let repos = vec![
        repo("Owner/Repo", 100, "2026-01-01T00:00:00Z", false),
        repo("owner/repo", 5_000, "2026-01-01T00:00:00Z", false),
    ];
    let folded = fold(repos, Mode::Best).unwrap();
    assert_eq!(folded.len(), 1, "case-insensitive dedupe by owner/name");
    assert_eq!(folded[0].stars, 5_000, "kept the better-starred copy");
}

#[test]
fn parse_github_is_tolerant_of_missing_fields() {
    let body = serde_json::json!({
        "items": [
            {
                "full_name": "rust-lang/rust",
                "description": "Empowering everyone",
                "language": "Rust",
                "stargazers_count": 90_000,
                "forks_count": 12_000,
                "open_issues_count": 9_000,
                "pushed_at": "2026-07-19T10:00:00Z",
                "topics": ["compiler", "language"],
                "archived": false,
                "html_url": "https://github.com/rust-lang/rust"
            },
            { "full_name": "" },                       // dropped: empty name
            { "description": "no name" },              // dropped: missing name
            { "full_name": "only/name" }               // kept: everything else defaults
        ]
    });
    let repos = parse_github(&body);
    assert_eq!(repos.len(), 2, "empty/nameless records are skipped");
    let top = &repos[0];
    assert_eq!(top.full_name, "rust-lang/rust");
    assert_eq!(top.stars, 90_000);
    assert_eq!(
        top.url.as_deref(),
        Some("https://github.com/rust-lang/rust")
    );
    let bare = &repos[1];
    assert_eq!(bare.stars, 0, "missing count defaults to zero");
    assert!(!bare.archived);
}

#[test]
fn trusted_repo_url_rejects_anything_but_a_clean_github_record() {
    assert!(trusted_repo_url("https://github.com/rust-lang/rust").is_some());
    assert!(trusted_repo_url("https://github.com/rust-lang/rust/issues").is_none());
    assert!(trusted_repo_url("http://github.com/a/b").is_none()); // not https
    assert!(trusted_repo_url("https://evil.com/a/b").is_none()); // wrong host
    assert!(trusted_repo_url("https://github.com/a/b?x=1").is_none()); // query
    assert!(trusted_repo_url("https://user@github.com/a/b").is_none()); // creds
}

#[test]
fn surface_with_notes_a_degraded_fetch_instead_of_failing() {
    let set = surface_with("anything", Mode::Best, 5, &|_, _, _| {
        Err("GitHub search unreachable".to_string())
    });
    assert!(set.repos.is_empty());
    assert_eq!(set.notes, vec!["GitHub search unreachable".to_string()]);
}

#[test]
fn mode_parse_maps_synonyms() {
    assert_eq!(Mode::parse("best"), Some(Mode::Best));
    assert_eq!(Mode::parse("Popular"), Some(Mode::Best));
    assert_eq!(Mode::parse("latest"), Some(Mode::Latest));
    assert_eq!(Mode::parse("RECENT"), Some(Mode::Latest));
    assert_eq!(Mode::parse("sideways"), None);
}

#[test]
fn run_without_query_shows_usage() {
    assert!(run(None).contains("usage: /repos"));
    assert!(run(Some("   ")).contains("usage: /repos"));
}

#[test]
#[cfg(unix)]
fn repo_policy_reloads_between_requests_and_reports_invalid_generation() {
    let _lock = crate::tests::env_lock();
    let root = crate::tests::TestGitWorkspace::new("repo-policy-reload");
    let script_dir = root.path().join("scripts/runtime");
    let lib_dir = root.path().join("lib/repo-search");
    std::fs::create_dir_all(&script_dir).unwrap();
    std::fs::create_dir_all(&lib_dir).unwrap();
    let source = Path::new(env!("CARGO_MANIFEST_DIR")).join("..");
    std::fs::copy(
        source.join("scripts/runtime/repo-search.mjs"),
        script_dir.join("repo-search.mjs"),
    )
    .unwrap();
    let rules = lib_dir.join("rules.mjs");
    let original = std::fs::read_to_string(source.join("lib/repo-search/rules.mjs")).unwrap();
    std::fs::write(&rules, &original).unwrap();
    let _root =
        crate::tests::TestEnvGuard::set("ANGEL_RESOURCE_DIR", root.path().to_str().unwrap());
    let item = repo("a/b", 10, "2026-01-01", false);
    assert_eq!(fold(vec![item.clone()], Mode::Best).unwrap().len(), 1);
    std::fs::write(&rules, original.replace("if (request.op === 'rank') return { version: 1, indices: rank(request.repos, request.mode) }", "if (request.op === 'rank') return { version: 1, indices: [] }")).unwrap();
    assert!(fold(vec![item.clone()], Mode::Best).is_err());
    std::fs::write(&rules, "invalid javascript {").unwrap();
    assert!(fold(vec![item], Mode::Best).is_err());
}

#[test]
#[cfg(unix)]
fn repo_brief_keeps_metadata_bounded_and_rejects_untrusted_cached_links() {
    let _lock = crate::tests::env_lock();
    let set = RepoSet {
        query: "query\nnext".into(),
        repos: vec![Repo {
            full_name: "a/b".into(),
            description: "description\nforged".into(),
            language: Some("Rust\nspoof".into()),
            stars: 10,
            forks: 0,
            open_issues: 0,
            pushed_at: Some("2026-01-01T00:00:00Z".into()),
            topics: vec![],
            archived: false,
            url: Some("https://evil.example/a/b".into()),
        }],
        notes: vec![],
    };
    let result = set.brief(Mode::Best, 10).unwrap();
    assert!(result.contains("query next"), "{result}");
    assert!(result.contains("description forged"), "{result}");
    assert!(result.contains("Rust spoof"), "{result}");
    assert!(result.contains("link unavailable"), "{result}");
    assert!(!result.contains("evil.example"), "{result}");
}

#[test]
fn empty_search_does_not_require_policy_worker() {
    let _lock = crate::tests::env_lock();
    let _missing = crate::tests::TestEnvGuard::set("ANGEL_NODE_BIN", "/nonexistent/angel-node");
    let set = surface_with("nothing", Mode::Best, 5, &|_, _, _| Ok(Vec::new()));
    assert!(set.repos.is_empty());
    assert!(set.notes.is_empty());
}

#[test]
#[cfg(unix)]
fn fresh_cache_preserves_source_candidates_and_uses_current_policy() {
    let _lock = crate::tests::env_lock();
    let root = crate::tests::TestGitWorkspace::new("repo-policy-cached");
    let script_dir = root.path().join("scripts/runtime");
    let lib_dir = root.path().join("lib/repo-search");
    std::fs::create_dir_all(&script_dir).unwrap();
    std::fs::create_dir_all(&lib_dir).unwrap();
    let source = Path::new(env!("CARGO_MANIFEST_DIR")).join("..");
    std::fs::copy(
        source.join("scripts/runtime/repo-search.mjs"),
        script_dir.join("repo-search.mjs"),
    )
    .unwrap();
    let rules = lib_dir.join("rules.mjs");
    let original = std::fs::read_to_string(source.join("lib/repo-search/rules.mjs")).unwrap();
    std::fs::write(&rules, &original).unwrap();
    let _root =
        crate::tests::TestEnvGuard::set("ANGEL_RESOURCE_DIR", root.path().to_str().unwrap());
    let cache = root.path().join("cache.json");
    let first = surface_cached_in(Some(&cache), "q", Mode::Best, 2, 3600, &|_, _, _| {
        Ok(vec![
            repo("a/low", 1, "2026-01-01", false),
            repo("b/high", 10000, "2026-01-01", false),
            repo("B/HIGH", 10, "2026-01-01", false),
        ])
    });
    let initial_brief = first.brief(Mode::Best, 2).unwrap();
    assert!(initial_brief.contains("  1. b/high"), "{initial_brief}");
    let cached: RepoSet = serde_json::from_slice(&std::fs::read(&cache).unwrap()).unwrap();
    assert_eq!(
        cached.repos.len(),
        3,
        "cache retains pre-deduplication input"
    );
    std::fs::write(
        &rules,
        original.replace(
            "const indices = rank(request.repos, request.mode)",
            "const indices = [0, 2]",
        ),
    )
    .unwrap();
    let next = surface_cached_in(Some(&cache), "q", Mode::Best, 2, 3600, &|_, _, _| {
        panic!("fresh cache should not fetch")
    });
    let next_brief = next.brief(Mode::Best, 2).unwrap();
    assert!(next_brief.contains("  1. a/low"), "{next_brief}");
    assert!(next_brief.contains("  2. B/HIGH"), "{next_brief}");
}

#[test]
#[cfg(unix)]
fn repo_policy_honors_owning_http_deadline() {
    let _lock = crate::tests::env_lock();
    let items = vec![repo("a/b", 1, "2026-01-01", false)];
    let error = crate::agent::tools::http_transport::with_timeout(Duration::ZERO, || {
        fold(items, Mode::Best).unwrap_err()
    });
    assert!(error.contains("interrupted"), "{error}");
}

#[test]
#[cfg(unix)]
fn repository_projection_uses_one_generation_during_a_live_edit() {
    let _lock = crate::tests::env_lock();
    let root = crate::tests::TestGitWorkspace::new("repo-policy-generation");
    let script_dir = root.path().join("scripts/runtime");
    let lib_dir = root.path().join("lib/repo-search");
    std::fs::create_dir_all(&script_dir).unwrap();
    std::fs::create_dir_all(&lib_dir).unwrap();
    let source = Path::new(env!("CARGO_MANIFEST_DIR")).join("..");
    std::fs::copy(
        source.join("scripts/runtime/repo-search.mjs"),
        script_dir.join("repo-search.mjs"),
    )
    .unwrap();
    let rules = lib_dir.join("rules.mjs");
    let original = std::fs::read_to_string(source.join("lib/repo-search/rules.mjs")).unwrap();
    let newer = original.replace("◇ repos", "◇ generation B");
    let injected = format!(
        "import {{ writeFileSync }} from 'node:fs'\n{}",
        original.replace(
            "const indices = rank(request.repos, request.mode)",
            &format!(
                "const indices = rank(request.repos, request.mode)\n    writeFileSync({}, {})",
                serde_json::to_string(&rules.to_string_lossy()).unwrap(),
                serde_json::to_string(&newer).unwrap()
            ),
        )
    );
    std::fs::write(&rules, injected).unwrap();
    let _root =
        crate::tests::TestEnvGuard::set("ANGEL_RESOURCE_DIR", root.path().to_str().unwrap());
    let set = RepoSet {
        query: "q".into(),
        repos: vec![repo("a/b", 10, "2026-01-01", false)],
        notes: vec![],
    };
    let first = set.brief(Mode::Best, 10).unwrap();
    assert!(first.contains("◇ repos"), "{first}");
    assert!(!first.contains("generation B"), "{first}");
    let next = set.brief(Mode::Best, 10).unwrap();
    assert!(next.contains("◇ generation B"), "{next}");
}

#[test]
#[cfg(unix)]
fn repository_worker_does_not_inherit_github_credentials() {
    let _lock = crate::tests::env_lock();
    let root = crate::tests::TestGitWorkspace::new("repo-policy-env");
    let script_dir = root.path().join("scripts/runtime");
    let lib_dir = root.path().join("lib/repo-search");
    std::fs::create_dir_all(&script_dir).unwrap();
    std::fs::create_dir_all(&lib_dir).unwrap();
    std::fs::write(
        lib_dir.join("rules.mjs"),
        "// placeholder for the fake entry point\n",
    )
    .unwrap();
    std::fs::write(script_dir.join("repo-search.mjs"), r#"
        import { readFileSync, writeSync } from 'node:fs';
        readFileSync(0);
        if (process.env.GITHUB_TOKEN || process.env.ANGEL_GITHUB_TOKEN || process.env.GH_TOKEN) throw Error('inherited token');
        writeSync(1, JSON.stringify({version:1, indices:[0], brief:'OK'}));
    "#).unwrap();
    let _root =
        crate::tests::TestEnvGuard::set("ANGEL_RESOURCE_DIR", root.path().to_str().unwrap());
    let _token = crate::tests::TestEnvGuard::set("GITHUB_TOKEN", "test-secret");
    let set = RepoSet {
        query: "q".into(),
        repos: vec![repo("a/b", 10, "2026-01-01", false)],
        notes: vec![],
    };
    assert_eq!(set.brief(Mode::Best, 10).unwrap(), "OK");
}

#[test]
#[cfg(unix)]
fn repo_search_tool_returns_error_when_policy_request_is_interrupted() {
    use crate::agent::harness::Tool;
    let _lock = crate::tests::env_lock();
    let root = crate::tests::TestGitWorkspace::new("repo-tool-interruption");
    let _home = crate::tests::TestEnvGuard::set("HOME", root.path().to_str().unwrap());
    let dir = root.path().join(".angelX/repos");
    std::fs::create_dir_all(&dir).unwrap();
    let key = format!("{:016x}", fnv1a_str("best:10:query"));
    let data = RepoSet {
        query: "query".into(),
        repos: vec![repo("a/b", 10, "2026-01-01", false)],
        notes: vec![],
    };
    std::fs::write(
        dir.join(format!("{key}.json")),
        serde_json::to_vec(&data).unwrap(),
    )
    .unwrap();
    let result = crate::agent::tools::http_transport::with_timeout(Duration::ZERO, || {
        crate::agent::tools::repos::RepoSearchTool.call(&serde_json::json!({"query":"query"}))
    });
    assert!(result.unwrap_err().contains("interrupted"));
}

#[test]
#[cfg(unix)]
fn repository_snapshot_rejects_oversized_and_nonregular_files_and_cleans_up() {
    use std::os::unix::ffi::OsStrExt;
    let _lock = crate::tests::env_lock();
    let root = crate::tests::TestGitWorkspace::new("repo-policy-snapshot");
    let script_dir = root.path().join("scripts/runtime");
    let lib_dir = root.path().join("lib/repo-search");
    let tmp = root.path().join("tmp");
    std::fs::create_dir_all(&script_dir).unwrap();
    std::fs::create_dir_all(&lib_dir).unwrap();
    std::fs::create_dir_all(&tmp).unwrap();
    let script = script_dir.join("repo-search.mjs");
    let rules = lib_dir.join("rules.mjs");
    std::fs::write(&script, "entry").unwrap();
    std::fs::write(&rules, "rules").unwrap();
    let _root =
        crate::tests::TestEnvGuard::set("ANGEL_RESOURCE_DIR", root.path().to_str().unwrap());
    let _tmp = crate::tests::TestEnvGuard::set("TMPDIR", tmp.to_str().unwrap());
    let snapshot = repo_snapshot().unwrap();
    assert_eq!(
        std::fs::read(snapshot.dir.join("lib/repo-search/rules.mjs")).unwrap(),
        b"rules"
    );
    let snapshot_dir = snapshot.dir.clone();
    drop(snapshot);
    assert!(!snapshot_dir.exists());
    std::fs::write(&rules, vec![b'x'; 1024 * 1024 + 1]).unwrap();
    assert!(repo_snapshot().err().unwrap().contains("byte limit"));
    std::fs::remove_file(&rules).unwrap();
    let path = std::ffi::CString::new(rules.as_os_str().as_bytes()).unwrap();
    // SAFETY: the CString is a valid, NUL-terminated path to a new FIFO.
    assert_eq!(unsafe { libc::mkfifo(path.as_ptr(), 0o600) }, 0);
    assert!(repo_snapshot().err().unwrap().contains("regular file"));
    assert_eq!(std::fs::read_dir(tmp).unwrap().count(), 0);
}

#[test]
fn repository_projection_rejects_missing_and_duplicate_identities() {
    let repos = vec![
        repo("a/repo", 1, "2026-01-01", false),
        repo("A/REPO", 2, "2026-01-01", false),
        repo("b/repo", 3, "2026-01-01", false),
    ];
    assert!(validate_indices(&serde_json::json!({"indices":[0]}), &repos).is_err());
    assert!(validate_indices(&serde_json::json!({"indices":[0,1,2]}), &repos).is_err());
    assert_eq!(
        validate_indices(&serde_json::json!({"indices":[1,2]}), &repos).unwrap(),
        vec![1, 2]
    );
}
