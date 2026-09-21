//! The repo-surfacing bench — the "latest/best repositories" support layer for
//! reports, native to the cockpit and modelled on [`crate::drive::science`].
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
//! Networked calls live behind `search`; the pure `parse_github`/`fold` folders
//! are unit tested offline so the ranking logic is covered without the wire.

#![allow(dead_code)] // wired incrementally: the parse/fold core lands with the tool/command surface.

use serde::{Deserialize, Serialize};
use std::io::Read;
use std::time::Duration;

/// GitHub search returns metadata for at most `per_page` (≤100) records; a page
/// of repositories fits comfortably under this ceiling, while a broken or
/// hostile endpoint cannot consume arbitrary RAM.
const REPO_RESPONSE_MAX_BYTES: usize = 4 * 1024 * 1024;
const MAX_DESC_CHARS: usize = 300;
const MAX_NAME_CHARS: usize = 160;
const MAX_TOPIC_CHARS: usize = 40;
const MAX_TOPICS: usize = 6;
pub(crate) const MAX_QUERY_CHARS: usize = 256;

/// How the shortlist is ordered. Both re-rank locally after the fetch so the
/// combined signal (stars + recency) is deterministic regardless of which
/// server-side sort GitHub applied.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) enum Mode {
    /// Reputation-first: most-starred, with a recency nudge. The default a
    /// report wants when citing "the" library for a topic.
    Best,
    /// Recency-first: most-recently-pushed, with a reputation nudge. What a
    /// report wants when the topic is moving and freshness beats stars.
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

    /// The GitHub `sort=` value that gets the right records into the page before
    /// the local re-rank refines the order.
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

/// One repository, normalized so the folder can dedupe and rank.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct Repo {
    /// `owner/name`, the dedupe key (lowercased when compared).
    pub(crate) full_name: String,
    pub(crate) description: String,
    pub(crate) language: Option<String>,
    pub(crate) stars: u64,
    pub(crate) forks: u64,
    pub(crate) open_issues: u64,
    /// ISO-8601 timestamp of the last push, verbatim from GitHub. ISO-8601 sorts
    /// lexicographically = chronologically, so it is compared as a string.
    pub(crate) pushed_at: Option<String>,
    pub(crate) topics: Vec<String>,
    pub(crate) archived: bool,
    /// Validated `https://github.com/owner/name` locator, or `None` when the
    /// upstream `html_url` did not match that exact shape.
    pub(crate) url: Option<String>,
}

impl Repo {
    /// Dedupe identity: the lowercased `owner/name`.
    fn key(&self) -> String {
        self.full_name.to_ascii_lowercase()
    }

    /// A day ordinal for the last push (days from civil epoch), or `None` when
    /// the timestamp is missing or malformed. Deterministic; no wall clock.
    fn pushed_ordinal(&self) -> Option<i64> {
        pushed_ordinal(self.pushed_at.as_deref()?)
    }

    /// Ranking score. `stars` are log-damped so a megastar repo does not bury
    /// everything; `recency` is measured against the freshest push in the set
    /// (no wall clock) so ranking is stable across runs. The two modes weight
    /// the same signals differently. Deterministic.
    fn score(&self, mode: Mode, newest_push: i64) -> f64 {
        let stars = (self.stars as f64 + 1.0).ln();
        let recency = match self.pushed_ordinal() {
            Some(o) if newest_push >= o => {
                // Decay to zero across ~2 years relative to the freshest push.
                (1.0 - ((newest_push - o) as f64 / 730.0)).max(0.0)
            }
            _ => 0.0,
        };
        // Archived repos are still citable but should sink beneath live ones.
        let live = if self.archived { 0.4 } else { 1.0 };
        let raw = match mode {
            Mode::Best => stars + recency * 1.5,
            Mode::Latest => recency * 6.0 + stars * 0.5,
        };
        raw * live
    }

    /// A one-line digest with a validated citation locator.
    pub(crate) fn line(&self) -> String {
        let name = crate::drive::science::sanitize_metadata(&self.full_name, MAX_NAME_CHARS);
        let name = if name.is_empty() {
            "unknown/repo"
        } else {
            &name
        };
        let lang = self
            .language
            .as_deref()
            .map(|l| crate::drive::science::sanitize_metadata(l, 40))
            .filter(|l| !l.is_empty())
            .unwrap_or_else(|| "—".to_string());
        let pushed = self
            .pushed_at
            .as_deref()
            .map(pushed_date)
            .unwrap_or_else(|| "n.d.".to_string());
        let desc = crate::drive::science::sanitize_metadata(&self.description, MAX_DESC_CHARS);
        let desc = if desc.is_empty() {
            "(no description)"
        } else {
            &desc
        };
        let flags = if self.archived { " [archived]" } else { "" };
        let citation = match &self.url {
            Some(url) => format!("source: {url}"),
            None => "link unavailable".to_string(),
        };
        format!(
            "{name}{flags} — ★{} · {lang} · pushed {pushed} · {}",
            self.stars, citation
        ) + &format!("\n      {desc}")
    }
}

/// The folded result of a fan-out: the ranked shortlist plus any notes about a
/// degraded fetch, ready to render or hand to the model.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub(crate) struct RepoSet {
    pub(crate) query: String,
    pub(crate) repos: Vec<Repo>,
    pub(crate) notes: Vec<String>,
}

impl RepoSet {
    /// A briefing: orientation (dominant language, freshest + most-starred
    /// anchors) then the ranked shortlist with cited links.
    pub(crate) fn brief(&self, mode: Mode, limit: usize) -> String {
        let mut out = format!(
            "◇ repos · \"{}\" · {} mode\n  {} repositor{} after dedupe\n",
            crate::drive::science::sanitize_metadata(&self.query, 200),
            mode.label(),
            self.repos.len(),
            if self.repos.len() == 1 { "y" } else { "ies" },
        );
        let langs = self.top_languages(4);
        if !langs.is_empty() {
            let l: Vec<String> = langs.iter().map(|(k, c)| format!("{k}×{c}")).collect();
            out.push_str(&format!("  languages: {}\n", l.join(", ")));
        }
        if let Some(top) = self.repos.iter().max_by_key(|r| r.stars) {
            out.push_str(&format!(
                "  most-starred: {}\n",
                top.line().replace("\n      ", " · ")
            ));
        }
        if let Some(fresh) = self
            .repos
            .iter()
            .filter(|r| r.pushed_ordinal().is_some())
            .max_by_key(|r| r.pushed_ordinal().unwrap_or(i64::MIN))
        {
            out.push_str(&format!(
                "  freshest: {}\n",
                fresh.line().replace("\n      ", " · ")
            ));
        }
        if !self.notes.is_empty() {
            let notes = self
                .notes
                .iter()
                .map(|n| crate::drive::science::sanitize_metadata(n, 240))
                .collect::<Vec<_>>();
            out.push_str(&format!("  (notes: {})\n", notes.join("; ")));
        }
        out.push_str("  ── shortlist ──\n");
        for (i, r) in self.repos.iter().take(limit).enumerate() {
            out.push_str(&format!("  {:>2}. {}\n", i + 1, r.line()));
        }
        out
    }

    /// Count languages across the shortlist, most-common first. Ties broken by
    /// name so the orientation line is deterministic.
    fn top_languages(&self, limit: usize) -> Vec<(String, usize)> {
        use std::collections::HashMap;
        let mut counts: HashMap<String, usize> = HashMap::new();
        for r in &self.repos {
            if let Some(lang) = r.language.as_deref().filter(|l| !l.is_empty()) {
                *counts.entry(lang.to_string()).or_default() += 1;
            }
        }
        let mut v: Vec<(String, usize)> = counts.into_iter().collect();
        v.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.cmp(&b.0)));
        v.truncate(limit);
        v
    }
}

/// The shared blocking agent — short timeouts so a slow GitHub can never wedge
/// the caller. `timeout` is the hard cap on the WHOLE request; a report writer
/// gets a fast degrade rather than a hang.
fn agent() -> ureq::Agent {
    ureq::AgentBuilder::new()
        // The request may carry the operator's GitHub bearer token. Keep it
        // pinned to the configured API origin.
        .redirects(0)
        .timeout_connect(Duration::from_secs(5))
        .timeout_read(Duration::from_secs(10))
        .timeout(Duration::from_secs(12))
        .user_agent("angelX-repos/0.1 (+https://github.com/newjordan/angelX)")
        .build()
}

/// An optional token from the environment. Absent → key-less (10 req/min);
/// present → authenticated (30 req/min for search). Never required.
fn github_token() -> Option<String> {
    ["ANGEL_GITHUB_TOKEN", "GITHUB_TOKEN", "GH_TOKEN"]
        .iter()
        .find_map(|k| std::env::var(k).ok())
        .map(|t| t.trim().to_string())
        .filter(|t| !t.is_empty())
}

/// Fan `query` at GitHub repository search, fold into a ranked, deduplicated
/// shortlist. A rate-limited or unreachable API is noted and skipped — it
/// degrades the result, it never fails the call.
pub(crate) fn surface(query: &str, mode: Mode, limit: usize) -> RepoSet {
    surface_with(query, mode, limit, &search)
}

/// Injected fan-out core so the tests drive folding without the wire.
fn surface_with<F>(query: &str, mode: Mode, limit: usize, searcher: &F) -> RepoSet
where
    F: Fn(&str, Mode, usize) -> Result<Vec<Repo>, String>,
{
    let (repos, notes) = match searcher(query, mode, limit) {
        Ok(repos) => (fold(repos, mode), Vec::new()),
        Err(error) => (Vec::new(), vec![error]),
    };
    RepoSet {
        query: query.to_string(),
        repos,
        notes,
    }
}

/// Dedupe by [`Repo::key`] (keeping the better-starred copy) and rank. Pure.
fn fold(repos: Vec<Repo>, mode: Mode) -> Vec<Repo> {
    use std::collections::HashMap;
    let newest = repos
        .iter()
        .filter_map(Repo::pushed_ordinal)
        .max()
        .unwrap_or(0);
    let mut by_key: HashMap<String, Repo> = HashMap::new();
    for r in repos {
        by_key
            .entry(r.key())
            .and_modify(|existing| {
                if r.stars > existing.stars {
                    *existing = r.clone();
                }
            })
            .or_insert(r);
    }
    let mut out: Vec<Repo> = by_key.into_values().collect();
    out.sort_by(|a, b| {
        b.score(mode, newest)
            .partial_cmp(&a.score(mode, newest))
            .unwrap_or(std::cmp::Ordering::Equal)
            // Stable tie-break so the ranking is deterministic across runs.
            .then_with(|| b.stars.cmp(&a.stars))
            .then_with(|| a.full_name.cmp(&b.full_name))
    });
    out
}

/// Query GitHub repository search. Networked; the pure `parse_github` folder
/// below carries the logic the tests exercise.
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
        // A 403 with an exhausted rate limit is the common key-less failure; turn
        // it into an actionable note rather than a bare transport error. The
        // rate-limit headers are read before the (owned) body is consumed.
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
        // A 422 is a malformed query (GitHub's search grammar) — surface why.
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

/// Pull GitHub's own `message` out of a non-2xx JSON body so the note is
/// actionable, bounded so a hostile error page can't flood the result.
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

/// GitHub `search/repositories` → [`Repo`]s. Tolerant: any missing field is
/// skipped or defaulted, never fatal, so an upstream schema drift degrades.
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

/// Admit only the exact `https://github.com/owner/name` record shape. Query
/// strings, fragments, credentials, ports, and deeper paths are not clean
/// citations and are rejected rather than echoed into a tool result.
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

/// Days from the civil epoch for the `YYYY-MM-DD` prefix of an ISO-8601 stamp,
/// or `None` when malformed. Howard Hinnant's `days_from_civil`; pure, no clock.
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

/// The `YYYY-MM-DD` portion of an ISO-8601 push stamp for display.
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

/// Minimal percent-encoding for the query string — spaces → `+`, anything
/// non-unreserved → `%XX`. Enough for the search URL without a URL crate.
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

// ─── persistence: a short TTL cache so repeat queries are instant ────────────

/// Where shortlists memoize (`~/.angelX/repos/<hash>.json`). `None` outside a
/// HOME, so unit-test worlds never touch the disk.
fn cache_dir() -> Option<std::path::PathBuf> {
    std::env::var_os("HOME").map(|h| std::path::Path::new(&h).join(".angelX").join("repos"))
}

/// Surface with a disk cache: a fresh hit (younger than `ttl_secs`) returns
/// without the network. A total whiff (rate-limited/unreachable → zero repos)
/// is never memoized, so a transient outage can't stick.
pub(crate) fn surface_cached(query: &str, mode: Mode, limit: usize, ttl_secs: u64) -> RepoSet {
    let key = format!(
        "{:016x}",
        fnv1a_str(&format!("{}:{limit}:{query}", mode.label()))
    );
    let path = cache_dir().map(|d| d.join(format!("{key}.json")));
    if let Some(p) = &path
        && let Some(hit) = read_fresh(p, ttl_secs)
    {
        return hit;
    }
    let set = surface(query, mode, limit);
    if let Some(p) = &path
        && !set.repos.is_empty()
    {
        write_cache(p, &set);
    }
    set
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
        let _ = std::fs::write(path, json);
    }
}

/// FNV-1a over the query → a stable cache filename. Not cryptographic; only
/// needs to spread queries across distinct files.
fn fnv1a_str(s: &str) -> u64 {
    let mut hash = 0xcbf2_9ce4_8422_2325u64;
    for b in s.bytes() {
        hash ^= b as u64;
        hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
    }
    hash
}

/// The `/repos <query>` command body: surface (cached, 30-min TTL) and return a
/// briefing. No query → usage. Mirrors [`crate::drive::science::run`].
pub(crate) fn run(arg: Option<&str>) -> String {
    let Some(raw) = arg.map(str::trim).filter(|q| !q.is_empty()) else {
        return "usage: /repos [best|latest] <query>\n  fans the query at GitHub repository \
                search, ranks by stars + recency, and returns a cited shortlist (cached 30m \
                under ~/.angelX/repos/). Prefix `latest` for recency-first, `best` (default) \
                for reputation-first."
            .to_string();
    };
    // An optional leading mode word: "/repos latest rust http client".
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
    set.brief(mode, 10)
}

#[cfg(test)]
#[path = "../../../tests/cockpit/app/repos__tests.rs"]
mod tests;
