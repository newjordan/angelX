//! The repo-surfacing bench — the "latest/best repositories" support layer for
//! reports, using the native host and a replaceable shortlist policy.
//!
//! When a report needs living software as evidence — the reference
//! implementation, the maintained library, the freshest project in a space —
//! the academic fan-out in [`crate::drive::science`] is the wrong index and a raw
//! `web_search` snippet is too thin to rank. This module gives the agent one
//! query fanned at GitHub's code search, folded into a ranked, deduplicated,
//! source-cited shortlist it can cite directly in a briefing.
//!
//! It talks to GitHub's free, key-less REST search API with the cockpit's
//! blocking `ureq` client — no async runtime, no token required (an optional
//! `ANGEL_GITHUB_TOKEN`/`GITHUB_TOKEN` only raises the rate limit). Parsing is
//! driven off `serde_json::Value` so an upstream schema tweak degrades to a
//! missing field, never a panic; a rate-limited or unreachable API degrades to
//! a noted skip, never a failed tool call.
//!
//! Networked calls and metadata admission live behind `search`; the ranking
//! and briefing policy runs in a bounded, disposable JavaScript worker.

#![allow(dead_code)]

use serde::{Deserialize, Serialize};
use std::io::Read;
use std::time::Duration;

const REPO_RESPONSE_MAX_BYTES: usize = 4 * 1024 * 1024;
const MAX_DESC_CHARS: usize = 300;
const MAX_NAME_CHARS: usize = 160;
const MAX_TOPIC_CHARS: usize = 40;
const MAX_TOPICS: usize = 6;
pub(crate) const MAX_QUERY_CHARS: usize = 256;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) enum Mode {
    Best,
    Latest,
}

impl Mode {
    pub(crate) fn parse(raw: &str) -> Option<Self> {
        match raw.trim().to_ascii_lowercase().as_str() {
            "best" | "top" | "stars" | "popular" => Some(Self::Best),
            "latest" | "recent" | "new" | "fresh" | "updated" => Some(Self::Latest),
            _ => None,
        }
    }

    fn github_sort(self) -> &'static str {
        match self {
            Self::Best => "stars",
            Self::Latest => "updated",
        }
    }

    fn label(self) -> &'static str {
        match self {
            Self::Best => "best",
            Self::Latest => "latest",
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct Repo {
    pub(crate) full_name: String,
    pub(crate) description: String,
    pub(crate) language: Option<String>,
    pub(crate) stars: u64,
    pub(crate) forks: u64,
    pub(crate) open_issues: u64,
    pub(crate) pushed_at: Option<String>,
    pub(crate) topics: Vec<String>,
    pub(crate) archived: bool,
    pub(crate) url: Option<String>,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub(crate) struct RepoSet {
    pub(crate) query: String,
    pub(crate) repos: Vec<Repo>,
    pub(crate) notes: Vec<String>,
}

impl RepoSet {
    pub(crate) fn brief(&self, mode: Mode, limit: usize) -> Result<String, String> {
        let repos: Vec<_> = self.repos.iter().map(|r| {
            let name = crate::drive::science::sanitize_metadata(&r.full_name, MAX_NAME_CHARS);
            let language = r.language.as_deref()
                .map(|s| crate::drive::science::sanitize_metadata(s, 40))
                .filter(|s| !s.is_empty());
            serde_json::json!({
                "full_name": r.full_name,
                "name": if name.is_empty() { "unknown/repo" } else { &name },
                "language": language,
                "raw_language": language,
                "description": crate::drive::science::sanitize_metadata(&r.description, MAX_DESC_CHARS),
                "stars": r.stars.to_string(),
                "archived": r.archived,
                "pushed_at": r.pushed_at,
                "pushed": r.pushed_at.as_deref().map(pushed_date),
                "url": r.url.as_deref().and_then(trusted_repo_url),
            })
        }).collect();
        let notes: Vec<_> = self
            .notes
            .iter()
            .map(|s| crate::drive::science::sanitize_metadata(s, 240))
            .collect();
        let request = serde_json::json!({"version":1, "op":"project", "mode":mode.label(),
            "query":crate::drive::science::sanitize_metadata(&self.query, 200),
            "limit":limit, "repos":repos, "notes":notes});
        repo_policy(&request)
            .and_then(|value| {
                validate_indices(&value, &self.repos)?;
                value
                    .get("brief")
                    .and_then(serde_json::Value::as_str)
                    .map(str::to_owned)
                    .ok_or_else(|| "invalid briefing response".to_string())
            })
            .map_err(|error| format!("repository briefing unavailable: {error}"))
    }
}

fn repo_snapshot() -> Result<crate::platform::worker_snapshot::Snapshot, String> {
    crate::platform::worker_snapshot::Snapshot::capture(
        &crate::platform::runtime_paths::root(),
        &[
            "scripts/runtime/repo-search.mjs",
            "lib/repo-search/rules.mjs",
        ],
        "repository",
    )
}

fn repo_policy(request: &serde_json::Value) -> Result<serde_json::Value, String> {
    let input = serde_json::to_vec(request).map_err(|_| "invalid policy request".to_string())?;
    let node = crate::agent::tools::runtime_missing::resolve("node")
        .map_err(|_| "Node.js unavailable".to_string())?;
    let context = crate::agent::tools::http_transport::context();
    if context.interrupted() {
        return Err("worker request interrupted".into());
    }
    let snapshot = repo_snapshot()?;
    let mut command = std::process::Command::new(node);
    command.arg(snapshot.dir.join("scripts/runtime/repo-search.mjs"));
    command.env_clear();
    let output = crate::platform::worker::run_cancellable(
        command,
        &input,
        Duration::from_secs(5),
        4 * 1024 * 1024,
        256 * 1024,
        || context.interrupted(),
    )?;
    let value: serde_json::Value =
        serde_json::from_slice(&output).map_err(|_| "invalid policy response".to_string())?;
    if value.get("version").and_then(serde_json::Value::as_u64) != Some(1) {
        return Err("unsupported policy response version".to_string());
    }
    Ok(value)
}

fn agent() -> ureq::Agent {
    ureq::AgentBuilder::new()
        .redirects(0)
        .timeout_connect(Duration::from_secs(5))
        .timeout_read(Duration::from_secs(10))
        .timeout(Duration::from_secs(12))
        .user_agent("angelX-repos/0.1 (+https://github.com/newjordan/angelX)")
        .build()
}

fn github_token() -> Option<String> {
    ["ANGEL_GITHUB_TOKEN", "GITHUB_TOKEN", "GH_TOKEN"]
        .iter()
        .find_map(|k| std::env::var(k).ok())
        .map(|t| t.trim().to_string())
        .filter(|t| !t.is_empty())
}

pub(crate) fn surface(query: &str, mode: Mode, limit: usize) -> RepoSet {
    surface_with(query, mode, limit, &search)
}

fn surface_with<F>(query: &str, mode: Mode, limit: usize, searcher: &F) -> RepoSet
where
    F: Fn(&str, Mode, usize) -> Result<Vec<Repo>, String>,
{
    let (repos, notes) = match searcher(query, mode, limit) {
        Ok(repos) => (repos, Vec::new()),
        Err(error) => (Vec::new(), vec![error]),
    };
    RepoSet {
        query: query.to_string(),
        repos,
        notes,
    }
}

fn validate_indices(value: &serde_json::Value, repos: &[Repo]) -> Result<Vec<usize>, String> {
    let indices = value
        .get("indices")
        .and_then(serde_json::Value::as_array)
        .ok_or_else(|| "invalid ranking response".to_string())?;
    if indices.len() > repos.len() {
        return Err("invalid ranking length".into());
    }
    let expected: std::collections::HashSet<_> = repos
        .iter()
        .map(|repo| repo.full_name.to_ascii_lowercase())
        .collect();
    let mut seen_indices = std::collections::HashSet::new();
    let mut seen_keys = std::collections::HashSet::new();
    let selected: Result<Vec<_>, String> = indices
        .iter()
        .map(|value| {
            let index = value
                .as_u64()
                .and_then(|n| usize::try_from(n).ok())
                .filter(|&index| index < repos.len())
                .ok_or_else(|| "invalid ranking index".to_string())?;
            if !seen_indices.insert(index) {
                return Err("duplicate ranking index".into());
            }
            if !seen_keys.insert(repos[index].full_name.to_ascii_lowercase()) {
                return Err("duplicate repository identity".into());
            }
            Ok(index)
        })
        .collect();
    let selected = selected?;
    if seen_keys != expected {
        return Err("repository policy omitted an identity".into());
    }
    Ok(selected)
}

#[cfg(test)]
fn fold(repos: Vec<Repo>, mode: Mode) -> Result<Vec<Repo>, String> {
    let input: Vec<_> = repos
        .iter()
        .map(|r| {
            serde_json::json!({
                "full_name": r.full_name, "stars": r.stars.to_string(),
                "pushed_at": r.pushed_at, "archived": r.archived,
            })
        })
        .collect();
    let response = repo_policy(&serde_json::json!({"version":1, "op":"rank",
        "mode":mode.label(), "repos":input}))?;
    validate_indices(&response, &repos)
        .map(|indices| indices.into_iter().map(|i| repos[i].clone()).collect())
}

pub(crate) fn search(query: &str, mode: Mode, limit: usize) -> Result<Vec<Repo>, String> {
    let per_page = limit.clamp(1, 100);
    let url = format!(
        "https://api.github.com/search/repositories?q={}&sort={}&order=desc&per_page={per_page}",
        urlencode(query),
        mode.github_sort(),
    );
    let mut req = agent()
        .get(&url)
        .set("Accept", "application/vnd.github+json")
        .set("X-GitHub-Api-Version", "2022-11-28");
    if let Some(token) = github_token() {
        req = req.set("Authorization", &format!("Bearer {token}"));
    }
    let response = match req.call() {
        Ok(r) => r,
        Err(ureq::Error::Status(403, r)) => {
            let exhausted = r.header("x-ratelimit-remaining") == Some("0");
            if exhausted {
                return Err(
                    "GitHub search rate limit reached (10/min unauthenticated); set \
                     ANGEL_GITHUB_TOKEN for 30/min, or retry shortly"
                        .to_string(),
                );
            }
            return Err(format!("GitHub search refused (403): {}", status_reason(r)));
        }
        Err(ureq::Error::Status(422, r)) => {
            return Err(format!(
                "GitHub rejected the query (422): {}",
                status_reason(r)
            ));
        }
        Err(ureq::Error::Status(code, r)) => {
            return Err(format!("GitHub search HTTP {code}: {}", status_reason(r)));
        }
        Err(e) => return Err(format!("GitHub search unreachable: {e}")),
    };
    let body = read_bounded_json(response.into_reader(), REPO_RESPONSE_MAX_BYTES)?;
    Ok(parse_github(&body))
}

fn status_reason(resp: ureq::Response) -> String {
    let raw = resp
        .into_string()
        .unwrap_or_else(|_| "unreadable response".to_string());
    let message = serde_json::from_str::<serde_json::Value>(&raw)
        .ok()
        .and_then(|v| {
            v.get("message")
                .and_then(|m| m.as_str())
                .map(str::to_string)
        })
        .unwrap_or(raw);
    crate::drive::science::sanitize_metadata(&message, 200)
}

fn parse_github(v: &serde_json::Value) -> Vec<Repo> {
    let mut out = Vec::new();
    let Some(items) = v.get("items").and_then(|i| i.as_array()) else {
        return out;
    };
    for w in items {
        let full_name = crate::drive::science::sanitize_metadata(
            w.get("full_name").and_then(|n| n.as_str()).unwrap_or(""),
            MAX_NAME_CHARS,
        );
        if full_name.is_empty() {
            continue;
        }
        let description = crate::drive::science::sanitize_metadata(
            w.get("description").and_then(|d| d.as_str()).unwrap_or(""),
            MAX_DESC_CHARS,
        );
        let language = w
            .get("language")
            .and_then(|l| l.as_str())
            .map(|l| crate::drive::science::sanitize_metadata(l, 40))
            .filter(|l| !l.is_empty());
        let topics = w
            .get("topics")
            .and_then(|t| t.as_array())
            .map(|a| {
                a.iter()
                    .filter_map(|t| t.as_str())
                    .map(|t| crate::drive::science::sanitize_metadata(t, MAX_TOPIC_CHARS))
                    .filter(|t| !t.is_empty())
                    .take(MAX_TOPICS)
                    .collect()
            })
            .unwrap_or_default();
        out.push(Repo {
            full_name,
            description,
            language,
            stars: w
                .get("stargazers_count")
                .and_then(serde_json::Value::as_u64)
                .unwrap_or(0),
            forks: w
                .get("forks_count")
                .and_then(serde_json::Value::as_u64)
                .unwrap_or(0),
            open_issues: w
                .get("open_issues_count")
                .and_then(serde_json::Value::as_u64)
                .unwrap_or(0),
            pushed_at: w
                .get("pushed_at")
                .and_then(|p| p.as_str())
                .filter(|p| pushed_ordinal(p).is_some())
                .map(str::to_string),
            topics,
            archived: w
                .get("archived")
                .and_then(serde_json::Value::as_bool)
                .unwrap_or(false),
            url: w
                .get("html_url")
                .and_then(|u| u.as_str())
                .and_then(trusted_repo_url),
        });
    }
    out
}

fn trusted_repo_url(raw: &str) -> Option<String> {
    let parsed = url::Url::parse(raw.trim()).ok()?;
    if parsed.scheme() != "https"
        || !parsed.username().is_empty()
        || parsed.password().is_some()
        || parsed.port().is_some()
        || parsed.query().is_some()
        || parsed.fragment().is_some()
        || parsed.host_str() != Some("github.com")
    {
        return None;
    }
    let mut segments = parsed.path().trim_start_matches('/').split('/');
    match (segments.next(), segments.next(), segments.next()) {
        (Some(owner), Some(repo), None)
            if valid_path_component(owner) && valid_path_component(repo) =>
        {
            Some(parsed.to_string())
        }
        _ => None,
    }
}

fn valid_path_component(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 128
        && value
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'-' | b'_' | b'.'))
}

fn pushed_ordinal(s: &str) -> Option<i64> {
    let s = s.trim();
    if s.len() < 10 {
        return None;
    }
    let year: i64 = s.get(0..4)?.parse().ok()?;
    if s.as_bytes().get(4) != Some(&b'-') || s.as_bytes().get(7) != Some(&b'-') {
        return None;
    }
    let month: i64 = s.get(5..7)?.parse().ok()?;
    let day: i64 = s.get(8..10)?.parse().ok()?;
    if !(1..=12).contains(&month) || !(1..=31).contains(&day) {
        return None;
    }
    let y = if month <= 2 { year - 1 } else { year };
    let era = (if y >= 0 { y } else { y - 399 }) / 400;
    let yoe = y - era * 400;
    let doy = (153 * (if month > 2 { month - 3 } else { month + 9 }) + 2) / 5 + day - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    Some(era * 146097 + doe - 719468)
}

fn pushed_date(s: &str) -> String {
    s.get(0..10).unwrap_or(s).to_string()
}

fn read_bounded_json(reader: impl Read, max_bytes: usize) -> Result<serde_json::Value, String> {
    let mut bytes = Vec::new();
    reader
        .take(max_bytes.saturating_add(1) as u64)
        .read_to_end(&mut bytes)
        .map_err(|e| format!("response read failed: {e}"))?;
    if bytes.len() > max_bytes {
        return Err(format!(
            "response exceeded {max_bytes}-byte repository metadata limit"
        ));
    }
    serde_json::from_slice(&bytes).map_err(|e| format!("invalid JSON response: {e}"))
}

fn urlencode(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for b in s.bytes() {
        match b {
            b'a'..=b'z' | b'A'..=b'Z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                out.push(b as char)
            }
            b' ' => out.push('+'),
            _ => out.push_str(&format!("%{b:02X}")),
        }
    }
    out
}

fn cache_dir() -> Option<std::path::PathBuf> {
    std::env::var_os("HOME").map(|h| std::path::Path::new(&h).join(".angelX").join("repos"))
}

pub(crate) fn surface_cached(query: &str, mode: Mode, limit: usize, ttl_secs: u64) -> RepoSet {
    let key = format!(
        "{:016x}",
        fnv1a_str(&format!("{}:{limit}:{query}", mode.label()))
    );
    let path = cache_dir().map(|d| d.join(format!("{key}.json")));
    surface_cached_in(path.as_deref(), query, mode, limit, ttl_secs, &search)
}

fn surface_cached_in<F>(
    path: Option<&std::path::Path>,
    query: &str,
    mode: Mode,
    limit: usize,
    ttl_secs: u64,
    searcher: &F,
) -> RepoSet
where
    F: Fn(&str, Mode, usize) -> Result<Vec<Repo>, String>,
{
    let hit = path.and_then(|p| read_fresh(p, ttl_secs));
    let from_cache = hit.is_some();
    let raw = match hit {
        Some(hit) => hit,
        None => match searcher(query, mode, limit) {
            Ok(repos) => RepoSet {
                query: query.to_string(),
                repos,
                notes: Vec::new(),
            },
            Err(error) => {
                return RepoSet {
                    query: query.to_string(),
                    repos: Vec::new(),
                    notes: vec![error],
                };
            }
        },
    };
    if raw.repos.is_empty() {
        return raw;
    }
    if !from_cache && let Some(path) = path {
        write_cache(path, &raw);
    }
    raw
}

fn read_fresh(path: &std::path::Path, ttl_secs: u64) -> Option<RepoSet> {
    let age = std::fs::metadata(path)
        .ok()?
        .modified()
        .ok()?
        .elapsed()
        .ok()?;
    if age.as_secs() > ttl_secs {
        return None;
    }
    let raw = std::fs::read_to_string(path).ok()?;
    serde_json::from_str(&raw).ok()
}

fn write_cache(path: &std::path::Path, set: &RepoSet) {
    if let Some(parent) = path.parent()
        && std::fs::create_dir_all(parent).is_err()
    {
        return;
    }
    if let Ok(json) = serde_json::to_string(set) {
        let _ = crate::platform::workspace_store::write_atomic(path, json.as_bytes());
    }
}

fn fnv1a_str(s: &str) -> u64 {
    let mut hash = 0xcbf2_9ce4_8422_2325u64;
    for b in s.bytes() {
        hash ^= b as u64;
        hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
    }
    hash
}

pub(crate) fn run(arg: Option<&str>) -> String {
    let Some(raw) = arg.map(str::trim).filter(|q| !q.is_empty()) else {
        return "usage: /repos [best|latest] <query>\n  fans the query at GitHub repository \
                search, ranks by stars + recency, and returns a cited shortlist (cached 30m \
                under ~/.angelX/repos/). Prefix `latest` for recency-first, `best` (default) \
                for reputation-first."
            .to_string();
    };
    let (mode, query) = match raw.split_once(char::is_whitespace) {
        Some((head, rest)) if Mode::parse(head).is_some() && !rest.trim().is_empty() => {
            (Mode::parse(head).unwrap(), rest.trim())
        }
        _ => (Mode::Best, raw),
    };
    if query.chars().count() > MAX_QUERY_CHARS {
        return format!("/repos query must be at most {MAX_QUERY_CHARS} characters");
    }
    let set = surface_cached(query, mode, 12, 1800);
    if set.repos.is_empty() {
        let why = if set.notes.is_empty() {
            "no repositories found".to_string()
        } else {
            set.notes.join("; ")
        };
        return format!(
            "repos \"{}\": {why}",
            crate::drive::science::sanitize_metadata(query, MAX_QUERY_CHARS)
        );
    }
    set.brief(mode, 10).unwrap_or_else(|error| error)
}

#[cfg(test)]
#[path = "../../../tests/cockpit/app/repos__tests.rs"]
mod tests;
