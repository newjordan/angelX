use super::*;

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
    let repos = vec![
        repo("a/low-stars-fresh", 5, "2026-07-19T00:00:00Z", false),
        repo("b/high-stars-stale", 50_000, "2024-01-01T00:00:00Z", false),
    ];
    let folded = fold(repos, Mode::Best);
    assert_eq!(
        folded[0].full_name, "b/high-stars-stale",
        "stars dominate best mode"
    );
}

#[test]
fn latest_mode_ranks_recency_first() {
    let repos = vec![
        repo("a/low-stars-fresh", 5, "2026-07-19T00:00:00Z", false),
        repo("b/high-stars-stale", 50_000, "2020-01-01T00:00:00Z", false),
    ];
    let folded = fold(repos, Mode::Latest);
    assert_eq!(
        folded[0].full_name, "a/low-stars-fresh",
        "recency dominates latest mode"
    );
}

#[test]
fn archived_repos_sink_beneath_live_peers() {
    let repos = vec![
        repo("a/archived", 10_000, "2026-07-19T00:00:00Z", true),
        repo("b/live", 9_000, "2026-07-19T00:00:00Z", false),
    ];
    let folded = fold(repos, Mode::Best);
    assert_eq!(
        folded[0].full_name, "b/live",
        "a live repo outranks a more-starred archived one"
    );
}

#[test]
fn fold_dedupes_by_full_name_keeping_the_better_starred() {
    let repos = vec![
        repo("Owner/Repo", 100, "2026-01-01T00:00:00Z", false),
        repo("owner/repo", 5_000, "2026-01-01T00:00:00Z", false),
    ];
    let folded = fold(repos, Mode::Best);
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
