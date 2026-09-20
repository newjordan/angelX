//! The science synthesis bench — the good parts of a research workbench,
//! ripped down to Rust and native to the cockpit.
//!
//! `OpenScience` (github.com/newjordan/openscience) exposes ~30 scientific
//! databases as agent tools. The valuable, load-bearing part of that is not the
//! browser workspace or the 290 skills — it is *the literature layer*: one query
//! fanned across the major indices, deduplicated, and ranked into a reading
//! list the agent can act on. That is what this module ports.
//!
//! It talks to the free, key-less JSON APIs with the cockpit's existing
//! blocking `ureq` client — no async runtime, no vendored TypeScript, no npm.
//! `synthesize` fans a query concurrently across every wired [`Source`], folds
//! the hits into one deduplicated, citation-ranked list, and hands back a [`Synthesis`] the
//! rest of the cockpit (tool surface, MCP, the miniworld's future Observatory)
//! can render. Parsing is driven off `serde_json::Value` so a schema tweak
//! upstream degrades to a missing field, never a panic.
//!
//! Networked calls live behind `search_*`; the pure `parse_*` folders are unit
//! tested offline so the synthesis logic is covered without touching the wire.

#![allow(dead_code)] // wired incrementally: parsers + synthesis land before the tool/MCP surface.

use serde::{Deserialize, Serialize};
use std::io::Read;
#[cfg(test)]
use std::time::Duration;

/// Public literature responses are metadata, never an unbounded download.
/// Twenty-five records from any supported index fit comfortably beneath this
/// ceiling, while a broken or hostile endpoint cannot consume arbitrary RAM.
const SCIENCE_RESPONSE_MAX_BYTES: usize = 2 * 1024 * 1024;
const MAX_TITLE_CHARS: usize = 500;
const MAX_AUTHOR_CHARS: usize = 160;
pub(crate) const MAX_QUERY_CHARS: usize = 500;

/// A scientific literature index we can query. Each maps to one free JSON API.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) enum Source {
    /// OpenAlex — 250M+ works, open catalog, no key. The spine of the fan-out.
    OpenAlex,
    /// Crossref — DOI registration metadata, strong on venues and references.
    Crossref,
    /// Semantic Scholar — CS/ML-strong graph, carries influential-citation signal.
    SemanticScholar,
    /// Europe PMC — biomedical + life sciences, key-less JSON. Broadens the fan
    /// beyond the ML-heavy indices into biology, chemistry and clinical work.
    EuropePmc,
}

impl Source {
    pub(crate) fn label(self) -> &'static str {
        match self {
            Source::OpenAlex => "OpenAlex",
            Source::Crossref => "Crossref",
            Source::SemanticScholar => "Semantic Scholar",
            Source::EuropePmc => "Europe PMC",
        }
    }

    /// Every source the bench fans a query across.
    pub(crate) fn all() -> [Source; 4] {
        [
            Source::OpenAlex,
            Source::Crossref,
            Source::SemanticScholar,
            Source::EuropePmc,
        ]
    }
}

/// One work, normalized across sources so the folder can dedupe and rank them.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct Paper {
    pub(crate) title: String,
    pub(crate) authors: Vec<String>,
    pub(crate) year: Option<u32>,
    /// Bare DOI, lowercased, no `https://doi.org/` prefix — the dedupe key.
    pub(crate) doi: Option<String>,
    pub(crate) url: Option<String>,
    pub(crate) citations: u64,
    pub(crate) source: Source,
}

impl Paper {
    /// Dedupe identity: the DOI when we have one; otherwise a composite of
    /// title + first author + year. A bare-title key silently merges distinct
    /// works that share a generic or degenerate title, so DOI-less records key
    /// on more than the title, and a title that squashes to nothing keys
    /// uniquely rather than collapsing every empty-title record into one.
    fn key(&self) -> String {
        if let Some(doi) = self.doi.as_deref().and_then(normalize_doi) {
            return format!("doi:{doi}");
        }
        let title = normalize_title(&self.title);
        if title.is_empty() {
            return format!(
                "u:{}:{}",
                self.source.label(),
                self.url.as_deref().unwrap_or(&self.title)
            );
        }
        let first_author = self
            .authors
            .first()
            .map(|a| normalize_title(a))
            .unwrap_or_default();
        format!("t:{title}|a:{first_author}|y:{}", self.year.unwrap_or(0))
    }

    /// A citation-safe locator. Cached records are revalidated here as well as
    /// at ingestion so an older or manually edited cache cannot inject a link.
    fn citation_locator(&self) -> Option<CitationLocator> {
        if let Some(doi) = self.doi.as_deref().and_then(normalize_doi) {
            return Some(CitationLocator::Doi(format!("https://doi.org/{doi}")));
        }
        self.url
            .as_deref()
            .and_then(|url| trusted_source_url(self.source, url))
            .map(CitationLocator::Source)
    }

    /// Ranking score — citations carry it (log-damped so a megahit doesn't bury
    /// everything), with a gentle recency nudge so a strong recent paper can
    /// edge an older one. Deterministic; no wall clock.
    fn score(&self, newest_year: u32) -> f64 {
        let cites = (self.citations as f64 + 1.0).ln();
        let recency = match self.year {
            Some(y) if newest_year >= y => 1.0 - ((newest_year - y) as f64 * 0.05).min(1.0),
            _ => 0.0,
        };
        cites + recency * 1.5
    }

    /// A one-line digest with an attributable, validated citation locator.
    pub(crate) fn line(&self) -> String {
        let safe_authors = self
            .authors
            .iter()
            .map(|author| sanitize_metadata(author, MAX_AUTHOR_CHARS))
            .filter(|author| !author.is_empty())
            .collect::<Vec<_>>();
        let who = match safe_authors.first() {
            Some(a) if safe_authors.len() > 1 => format!("{a} et al."),
            Some(a) => a.clone(),
            None => "—".into(),
        };
        let title = sanitize_metadata(&self.title, MAX_TITLE_CHARS);
        let title = if title.is_empty() { "Untitled" } else { &title };
        let year = self.year.map_or_else(|| "n.d.".into(), |y| y.to_string());
        let citation = match self.citation_locator() {
            Some(CitationLocator::Doi(url)) => format!("doi: {url}"),
            Some(CitationLocator::Source(url)) => format!("source: {url}"),
            None => "citation unavailable".to_string(),
        };
        format!(
            "{} ({}) — {} · {} cites [{}] · {}",
            title,
            year,
            who,
            self.citations,
            self.source.label(),
            citation
        )
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
enum CitationLocator {
    Doi(String),
    Source(String),
}

/// The folded result of a fan-out: the ranked reading list plus which sources
/// answered, ready to render or hand to the model.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub(crate) struct Synthesis {
    pub(crate) query: String,
    pub(crate) papers: Vec<Paper>,
    pub(crate) sources_hit: Vec<Source>,
    pub(crate) notes: Vec<String>,
}

impl Synthesis {
    /// A compact plaintext digest — the reading list, one paper per line.
    pub(crate) fn digest(&self, limit: usize) -> String {
        let mut out = format!(
            "synthesis · \"{}\" · {} works\n",
            sanitize_metadata(&self.query, 240),
            self.papers.len()
        );
        for (i, p) in self.papers.iter().take(limit).enumerate() {
            out.push_str(&format!("{:>2}. {}\n", i + 1, p.line()));
        }
        out
    }
}

/// Fan `query` across every [`Source`], fold into one deduplicated, ranked
/// reading list. A source that errors is noted and skipped — a down index
/// degrades the result, it never fails the call.
pub(crate) fn synthesize(query: &str, per_source: usize) -> Synthesis {
    synthesize_with(query, per_source, &search)
}

enum SourceWorkerOutcome {
    Completed(Result<Vec<Paper>, String>),
    Panicked,
}

/// Injected fan-out core. Exactly one scoped worker owns each fixed source;
/// every worker is joined, then outcomes are reduced in [`Source::all`] order.
/// That keeps diagnostics and source provenance deterministic even when the
/// network completes in reverse order. A worker panic is source-attributed and
/// degraded like any other unavailable index rather than unwinding the tool.
fn synthesize_with<F>(query: &str, per_source: usize, searcher: &F) -> Synthesis
where
    F: Fn(Source, &str, usize) -> Result<Vec<Paper>, String> + Sync,
{
    let outcomes = std::thread::scope(|scope| {
        let handles = Source::all()
            .into_iter()
            .map(|source| {
                let http_context = crate::tools::http_transport::context();
                (
                    source,
                    scope.spawn(move || {
                        match crate::term::catch_background_unwind(|| {
                            crate::tools::http_transport::with_context(http_context, || {
                                searcher(source, query, per_source)
                            })
                        }) {
                            Ok(result) => SourceWorkerOutcome::Completed(result),
                            Err(_) => SourceWorkerOutcome::Panicked,
                        }
                    }),
                )
            })
            .collect::<Vec<_>>();
        handles
            .into_iter()
            .map(|(source, handle)| (source, handle.join()))
            .collect::<Vec<_>>()
    });

    let mut all: Vec<Paper> = Vec::new();
    let mut hit = Vec::new();
    let mut notes = Vec::new();
    for (source, outcome) in outcomes {
        match outcome {
            Ok(SourceWorkerOutcome::Completed(Ok(mut papers))) => {
                if !papers.is_empty() {
                    hit.push(source);
                }
                all.append(&mut papers);
            }
            Ok(SourceWorkerOutcome::Completed(Err(error))) => {
                notes.push(format!("{}: {error}", source.label()));
            }
            Ok(SourceWorkerOutcome::Panicked) | Err(_) => {
                notes.push(format!("{}: source worker panicked", source.label()));
            }
        }
    }
    let papers = fold(all);
    Synthesis {
        query: query.to_string(),
        papers,
        sources_hit: hit,
        notes,
    }
}

/// Dedupe by [`Paper::key`] (keeping the better-cited copy) and rank. Pure — the
/// unit tests drive it straight, no network.
fn fold(papers: Vec<Paper>) -> Vec<Paper> {
    use std::collections::HashMap;
    let newest = papers.iter().filter_map(|p| p.year).max().unwrap_or(0);
    let mut by_key: HashMap<String, Paper> = HashMap::new();
    for p in papers {
        by_key
            .entry(p.key())
            .and_modify(|existing| {
                // Keep the richer record: more citations wins; ties keep the one
                // with a syntactically valid DOI.
                let existing_has_doi = existing.doi.as_deref().and_then(normalize_doi).is_some();
                let candidate_has_doi = p.doi.as_deref().and_then(normalize_doi).is_some();
                if p.citations > existing.citations
                    || (p.citations == existing.citations && !existing_has_doi && candidate_has_doi)
                {
                    *existing = p.clone();
                }
            })
            .or_insert(p);
    }
    let mut out: Vec<Paper> = by_key.into_values().collect();
    out.sort_by(|a, b| {
        b.score(newest)
            .partial_cmp(&a.score(newest))
            .unwrap_or(std::cmp::Ordering::Equal)
            // Stable tie-break so the ranking is deterministic across runs.
            .then_with(|| a.title.cmp(&b.title))
    });
    out
}

/// Query one source. Networked; the pure `parse_*` folders below carry the
/// logic and are what the tests exercise.
pub(crate) fn search(src: Source, query: &str, limit: usize) -> Result<Vec<Paper>, String> {
    let enc = urlencode(query);
    let url = match src {
        Source::OpenAlex => format!(
            "https://api.openalex.org/works?search={enc}&per-page={limit}&sort=cited_by_count:desc"
        ),
        Source::Crossref => {
            format!(
                "https://api.crossref.org/works?query={enc}&rows={limit}&select=title,author,DOI,URL,is-referenced-by-count,issued"
            )
        }
        Source::SemanticScholar => format!(
            "https://api.semanticscholar.org/graph/v1/paper/search?query={enc}&limit={limit}&fields=title,year,authors,externalIds,citationCount,url"
        ),
        Source::EuropePmc => format!(
            "https://www.ebi.ac.uk/europepmc/webservices/rest/search?query={enc}&format=json&pageSize={limit}"
        ),
    };
    let response = crate::tools::http_transport::request("GET", &url, true, 5)
        .set(
            "User-Agent",
            "angel0-science/0.1 (+https://github.com/newjordan/angel0)",
        )
        .call()
        .map_err(|e| e.to_string())?;
    let body = read_bounded_json(response.into_reader(), SCIENCE_RESPONSE_MAX_BYTES)?;
    let papers = match src {
        Source::OpenAlex => parse_openalex(&body),
        Source::Crossref => parse_crossref(&body),
        Source::SemanticScholar => parse_semantic_scholar(&body),
        Source::EuropePmc => parse_europepmc(&body),
    };
    // Distinguish schema drift or a 200 error-envelope (the expected result
    // container is absent) from a legitimate empty result set (container present
    // but empty). The former is a degraded source worth a note, not a silent
    // "no matches" — [`synthesize_with`] folds the Err into the briefing's
    // skipped-sources line.
    if papers.is_empty() && !result_container_present(src, &body) {
        return Err(format!(
            "unexpected response shape (missing `{}`)",
            container_key(src)
        ));
    }
    Ok(papers)
}

/// The JSON path each source's result array lives at — used to explain a
/// schema-drift skip and to tell "container missing" from "container empty".
fn container_key(src: Source) -> &'static str {
    match src {
        Source::OpenAlex => "results",
        Source::Crossref => "message.items",
        Source::SemanticScholar => "data",
        Source::EuropePmc => "resultList.result",
    }
}

fn result_container_present(src: Source, v: &serde_json::Value) -> bool {
    let container = match src {
        Source::OpenAlex => v.get("results"),
        Source::Crossref => v.get("message").and_then(|m| m.get("items")),
        Source::SemanticScholar => v.get("data"),
        Source::EuropePmc => v.get("resultList").and_then(|r| r.get("result")),
    };
    container.is_some_and(serde_json::Value::is_array)
}

/// OpenAlex `works` → [`Paper`]s. Tolerant: any missing field is skipped, never
/// fatal, so an upstream schema drift degrades gracefully.
fn parse_openalex(v: &serde_json::Value) -> Vec<Paper> {
    let mut out = Vec::new();
    let Some(results) = v.get("results").and_then(|r| r.as_array()) else {
        return out;
    };
    for w in results {
        let title = sanitize_metadata(
            w.get("display_name").and_then(|t| t.as_str()).unwrap_or(""),
            MAX_TITLE_CHARS,
        );
        if title.is_empty() {
            continue;
        }
        let authors = w
            .get("authorships")
            .and_then(|a| a.as_array())
            .map(|a| {
                a.iter()
                    .filter_map(|au| {
                        au.get("author")
                            .and_then(|x| x.get("display_name"))
                            .and_then(|n| n.as_str())
                            .map(|name| sanitize_metadata(name, MAX_AUTHOR_CHARS))
                    })
                    .filter(|name| !name.is_empty())
                    .take(4)
                    .collect()
            })
            .unwrap_or_default();
        out.push(Paper {
            title,
            authors,
            year: w
                .get("publication_year")
                .and_then(serde_json::Value::as_u64)
                .and_then(sane_year),
            doi: w
                .get("doi")
                .and_then(|d| d.as_str())
                .and_then(normalize_doi),
            url: w
                .get("id")
                .and_then(|i| i.as_str())
                .and_then(|url| trusted_source_url(Source::OpenAlex, url)),
            citations: w
                .get("cited_by_count")
                .and_then(serde_json::Value::as_u64)
                .unwrap_or(0),
            source: Source::OpenAlex,
        });
    }
    out
}

/// Crossref `message.items` → [`Paper`]s.
fn parse_crossref(v: &serde_json::Value) -> Vec<Paper> {
    let mut out = Vec::new();
    let Some(items) = v
        .get("message")
        .and_then(|m| m.get("items"))
        .and_then(|i| i.as_array())
    else {
        return out;
    };
    for w in items {
        let title = sanitize_metadata(
            w.get("title")
                .and_then(|t| t.as_array())
                .and_then(|a| a.first())
                .and_then(|t| t.as_str())
                .unwrap_or(""),
            MAX_TITLE_CHARS,
        );
        if title.is_empty() {
            continue;
        }
        let authors = w
            .get("author")
            .and_then(|a| a.as_array())
            .map(|a| {
                a.iter()
                    .filter_map(|au| {
                        let fam = au.get("family").and_then(|f| f.as_str());
                        let given = au.get("given").and_then(|g| g.as_str());
                        match (given, fam) {
                            (Some(g), Some(f)) => {
                                Some(sanitize_metadata(&format!("{g} {f}"), MAX_AUTHOR_CHARS))
                            }
                            (None, Some(f)) => Some(sanitize_metadata(f, MAX_AUTHOR_CHARS)),
                            _ => None,
                        }
                    })
                    .filter(|name| !name.is_empty())
                    .take(4)
                    .collect()
            })
            .unwrap_or_default();
        let year = w
            .get("issued")
            .and_then(|i| i.get("date-parts"))
            .and_then(|d| d.as_array())
            .and_then(|a| a.first())
            .and_then(|a| a.as_array())
            .and_then(|a| a.first())
            .and_then(serde_json::Value::as_u64)
            .and_then(sane_year);
        out.push(Paper {
            title,
            authors,
            year,
            doi: w
                .get("DOI")
                .and_then(|d| d.as_str())
                .and_then(normalize_doi),
            url: w
                .get("URL")
                .and_then(|u| u.as_str())
                .and_then(|url| trusted_source_url(Source::Crossref, url)),
            citations: w
                .get("is-referenced-by-count")
                .and_then(serde_json::Value::as_u64)
                .unwrap_or(0),
            source: Source::Crossref,
        });
    }
    out
}

/// Semantic Scholar `paper/search` → [`Paper`]s. Carries the DOI out of the
/// `externalIds` bag when present.
fn parse_semantic_scholar(v: &serde_json::Value) -> Vec<Paper> {
    let mut out = Vec::new();
    let Some(data) = v.get("data").and_then(|d| d.as_array()) else {
        return out;
    };
    for w in data {
        let title = sanitize_metadata(
            w.get("title").and_then(|t| t.as_str()).unwrap_or(""),
            MAX_TITLE_CHARS,
        );
        if title.is_empty() {
            continue;
        }
        let authors = w
            .get("authors")
            .and_then(|a| a.as_array())
            .map(|a| {
                a.iter()
                    .filter_map(|au| {
                        au.get("name")
                            .and_then(|n| n.as_str())
                            .map(|name| sanitize_metadata(name, MAX_AUTHOR_CHARS))
                    })
                    .filter(|name| !name.is_empty())
                    .take(4)
                    .collect()
            })
            .unwrap_or_default();
        out.push(Paper {
            title,
            authors,
            year: w
                .get("year")
                .and_then(serde_json::Value::as_u64)
                .and_then(sane_year),
            doi: w
                .get("externalIds")
                .and_then(|e| e.get("DOI"))
                .and_then(|d| d.as_str())
                .and_then(normalize_doi),
            url: w
                .get("url")
                .and_then(|u| u.as_str())
                .and_then(|url| trusted_source_url(Source::SemanticScholar, url)),
            citations: w
                .get("citationCount")
                .and_then(serde_json::Value::as_u64)
                .unwrap_or(0),
            source: Source::SemanticScholar,
        });
    }
    out
}

/// Europe PMC `resultList.result` → [`Paper`]s. Its authors arrive as one
/// comma-joined `authorString` and the year as a string, so both are split /
/// parsed here.
fn parse_europepmc(v: &serde_json::Value) -> Vec<Paper> {
    let mut out = Vec::new();
    let Some(results) = v
        .get("resultList")
        .and_then(|r| r.get("result"))
        .and_then(|r| r.as_array())
    else {
        return out;
    };
    for w in results {
        let title = sanitize_metadata(
            w.get("title")
                .and_then(|t| t.as_str())
                .unwrap_or("")
                .trim_end_matches('.'),
            MAX_TITLE_CHARS,
        );
        if title.is_empty() {
            continue;
        }
        let authors = w
            .get("authorString")
            .and_then(|a| a.as_str())
            .map(|s| {
                s.trim_end_matches('.')
                    .split(", ")
                    .filter(|a| !a.is_empty())
                    .take(4)
                    .map(|name| sanitize_metadata(name, MAX_AUTHOR_CHARS))
                    .filter(|name| !name.is_empty())
                    .collect()
            })
            .unwrap_or_default();
        let url = match (
            w.get("source").and_then(|s| s.as_str()),
            w.get("id").and_then(|i| i.as_str()),
        ) {
            (Some(s), Some(id)) if valid_record_component(s) && valid_record_component(id) => {
                trusted_source_url(
                    Source::EuropePmc,
                    &format!("https://europepmc.org/article/{s}/{id}"),
                )
            }
            _ => None,
        };
        out.push(Paper {
            title,
            authors,
            year: w
                .get("pubYear")
                .and_then(|y| y.as_str())
                .and_then(|y| y.parse::<u64>().ok())
                .and_then(sane_year),
            doi: w
                .get("doi")
                .and_then(|d| d.as_str())
                .and_then(normalize_doi),
            url,
            citations: w
                .get("citedByCount")
                .and_then(serde_json::Value::as_u64)
                .unwrap_or(0),
            source: Source::EuropePmc,
        });
    }
    out
}

/// Plausible publication-year band. The module deliberately avoids the wall
/// clock (see [`Paper::score`]), so this is a fixed band, not `now()`: it only
/// has to reject a corrupt or hostile future/ancient year that would otherwise
/// collapse the recency signal and hijack the "freshest" anchor.
const MIN_PLAUSIBLE_YEAR: u32 = 1500;
const MAX_PLAUSIBLE_YEAR: u32 = 2100;

/// Accept a year only within the plausible band; anything else is dropped to
/// `None` (unknown) rather than trusted.
fn sane_year(y: u64) -> Option<u32> {
    u32::try_from(y)
        .ok()
        .filter(|y| (MIN_PLAUSIBLE_YEAR..=MAX_PLAUSIBLE_YEAR).contains(y))
}

/// Normalize a DOI only when it follows the DOI handbook's practical ASCII
/// shape: `10.` plus a 4–9 digit registrant and a non-empty safe suffix.
fn normalize_doi(raw: &str) -> Option<String> {
    let raw = raw.trim();
    let lower = raw.to_ascii_lowercase();
    let doi = [
        "https://doi.org/",
        "http://doi.org/",
        "https://dx.doi.org/",
        "http://dx.doi.org/",
        "doi:",
    ]
    .iter()
    .find_map(|prefix| lower.strip_prefix(prefix))
    .unwrap_or(&lower);
    if !(7..=255).contains(&doi.len()) || !doi.is_ascii() {
        return None;
    }
    let (registrant, suffix) = doi.strip_prefix("10.")?.split_once('/')?;
    if !(4..=9).contains(&registrant.len())
        || !registrant.bytes().all(|b| b.is_ascii_digit())
        || suffix.is_empty()
        || !suffix.bytes().all(|b| {
            b.is_ascii_alphanumeric()
                || matches!(b, b'-' | b'.' | b'_' | b';' | b'(' | b')' | b'/' | b':')
        })
    {
        return None;
    }
    Some(doi.to_string())
}

/// Admit only the fixed HTTPS record family owned by the declared source.
/// Query strings, fragments, credentials, ports, and ambiguous paths are not
/// useful citations and are rejected rather than echoed into a tool result.
fn trusted_source_url(source: Source, raw: &str) -> Option<String> {
    let parsed = url::Url::parse(raw.trim()).ok()?;
    if parsed.scheme() != "https"
        || !parsed.username().is_empty()
        || parsed.password().is_some()
        || parsed.port().is_some()
        || parsed.query().is_some()
        || parsed.fragment().is_some()
    {
        return None;
    }
    let host = parsed.host_str()?;
    let path = parsed.path();
    let valid = match source {
        Source::OpenAlex => {
            host == "openalex.org"
                && path
                    .strip_prefix("/W")
                    .is_some_and(|id| !id.is_empty() && id.bytes().all(|b| b.is_ascii_digit()))
        }
        Source::Crossref => {
            host == "doi.org" && path.strip_prefix('/').and_then(normalize_doi).is_some()
        }
        Source::SemanticScholar => {
            host == "www.semanticscholar.org"
                && path.strip_prefix("/paper/").is_some_and(valid_record_path)
        }
        Source::EuropePmc => {
            host == "europepmc.org"
                && path.strip_prefix("/article/").is_some_and(|rest| {
                    let mut parts = rest.split('/');
                    matches!(
                        (parts.next(), parts.next(), parts.next()),
                        (Some(source), Some(id), None)
                            if valid_record_component(source) && valid_record_component(id)
                    )
                })
        }
    };
    valid.then(|| parsed.to_string())
}

fn valid_record_component(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 128
        && value
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'-' | b'_' | b'.'))
}

fn valid_record_path(value: &str) -> bool {
    !value.is_empty() && value.len() <= 512 && value.split('/').all(valid_record_component)
}

/// Keep one terminal row per record even when an upstream index contains
/// control characters, bidi overrides, zero-width separators, or huge fields.
pub(crate) fn sanitize_metadata(raw: &str, max_chars: usize) -> String {
    let without_controls: String = raw
        .chars()
        .map(|c| {
            let formatting_control = matches!(
                c,
                '\u{200b}'..='\u{200f}' | '\u{202a}'..='\u{202e}' | '\u{2060}'..='\u{206f}' | '\u{feff}'
            );
            if c.is_control() || formatting_control {
                ' '
            } else {
                c
            }
        })
        .collect();
    let compact = without_controls
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ");
    if compact.chars().count() <= max_chars {
        return compact;
    }
    if max_chars == 0 {
        return String::new();
    }
    if max_chars == 1 {
        return "…".to_string();
    }
    let mut out: String = compact.chars().take(max_chars - 1).collect();
    out.push('…');
    out
}

fn read_bounded_json(reader: impl Read, max_bytes: usize) -> Result<serde_json::Value, String> {
    let mut bytes = Vec::new();
    reader
        .take(max_bytes.saturating_add(1) as u64)
        .read_to_end(&mut bytes)
        .map_err(|e| format!("response read failed: {e}"))?;
    if bytes.len() > max_bytes {
        return Err(format!(
            "response exceeded {max_bytes}-byte scientific metadata limit"
        ));
    }
    serde_json::from_slice(&bytes).map_err(|e| format!("invalid JSON response: {e}"))
}

/// Squash a title to a dedupe key: lowercase alphanumerics only, no spaces.
fn normalize_title(t: &str) -> String {
    t.chars()
        .filter(|c| c.is_alphanumeric())
        .flat_map(char::to_lowercase)
        .collect()
}

/// Minimal percent-encoding for a query string — enough for the fan-out URLs
/// without pulling in a URL crate. Spaces → `+`, anything non-unreserved → `%XX`.
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

// ─── persistence: a TTL cache so repeat queries are instant and offline ──────

/// Where synthesis results memoize (`~/.angel0/science/<hash>.json`). `None`
/// outside a HOME, so unit-test worlds never touch the disk.
fn cache_dir() -> Option<std::path::PathBuf> {
    std::env::var_os("HOME").map(|h| std::path::Path::new(&h).join(".angel0").join("science"))
}

/// Synthesize with a disk cache: a fresh hit (younger than `ttl_secs`) returns
/// without touching the network. Falls straight through to [`synthesize`] when
/// there is no HOME or the cache is stale/missing. Only a *complete* fan-out is
/// memoized (non-empty AND no skipped/errored source): a total whiff never
/// sticks, and a partial outage (some indices momentarily down) is not pinned as
/// a thin reading list for the full TTL — it is re-tried next call.
pub(crate) fn synthesize_cached(query: &str, per_source: usize, ttl_secs: u64) -> Synthesis {
    let key = format!("{:016x}", fnv1a_str(&format!("{per_source}:{query}")));
    let path = cache_dir().map(|d| d.join(format!("{key}.json")));
    if let Some(p) = &path
        && let Some(hit) = read_fresh(p, ttl_secs)
    {
        return hit;
    }
    let syn = synthesize(query, per_source);
    if let Some(p) = &path
        && !syn.papers.is_empty()
        && syn.notes.is_empty()
    {
        write_cache(p, &syn);
    }
    syn
}

/// A cached [`Synthesis`] iff the file exists and its mtime is within `ttl_secs`.
fn read_fresh(path: &std::path::Path, ttl_secs: u64) -> Option<Synthesis> {
    let age = std::fs::metadata(path)
        .ok()?
        .modified()
        .ok()?
        .elapsed()
        .ok()?;
    if age.as_secs() > ttl_secs {
        return None;
    }
    serde_json::from_str(&std::fs::read_to_string(path).ok()?).ok()
}

/// How long a cache entry may sit before the opportunistic prune removes it.
/// Far past the read TTL (1h) — this is a disk-hygiene horizon, not freshness.
const CACHE_PRUNE_SECS: u64 = 7 * 24 * 3600;

fn write_cache(path: &std::path::Path, syn: &Synthesis) {
    if let Some(dir) = path.parent() {
        let _ = std::fs::create_dir_all(dir);
        // Piggyback disk hygiene on the write (never on the hot read path):
        // entries older than the horizon are dead — the TTL expired days ago —
        // so the cache dir can't accumulate one file per query forever.
        prune_stale(dir, CACHE_PRUNE_SECS);
    }
    if let Ok(raw) = crate::secrets::to_redacted_vec(syn) {
        let _ = std::fs::write(path, raw);
    }
}

/// Delete `.json` cache entries in `dir` older than `horizon_secs`. Best-effort:
/// any IO error skips the file — hygiene must never fail a synthesis.
fn prune_stale(dir: &std::path::Path, horizon_secs: u64) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let p = entry.path();
        if p.extension().is_none_or(|x| x != "json") {
            continue;
        }
        let stale = std::fs::metadata(&p)
            .ok()
            .and_then(|m| m.modified().ok())
            .and_then(|t| t.elapsed().ok())
            .is_some_and(|age| age.as_secs() > horizon_secs);
        if stale {
            let _ = std::fs::remove_file(&p);
        }
    }
}

fn fnv1a_str(s: &str) -> u64 {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for b in s.bytes() {
        h ^= u64::from(b);
        h = h.wrapping_mul(0x0000_0100_0000_01b3);
    }
    h
}

// ─── S4: analysis — a briefing, not just a bibliography ──────────────────────

/// Generic words that don't discriminate a research theme, dropped before the
/// keyword vote so `themes()` surfaces the domain terms that actually cluster.
const STOPWORDS: &[&str] = &[
    "the",
    "and",
    "for",
    "with",
    "without",
    "via",
    "using",
    "from",
    "into",
    "onto",
    "between",
    "across",
    "over",
    "under",
    "that",
    "this",
    "these",
    "those",
    "study",
    "studies",
    "analysis",
    "approach",
    "approaches",
    "method",
    "methods",
    "based",
    "novel",
    "toward",
    "towards",
    "case",
    "review",
    "paper",
    "results",
    "effects",
    "role",
    "evidence",
    "data",
    "their",
    "have",
    "been",
];

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct TopicCluster {
    pub(crate) theme: String,
    pub(crate) paper_count: usize,
    pub(crate) citation_sum: u64,
    pub(crate) anchors: Vec<String>,
}

/// Significant keywords in a title: lowercased alphanumeric words ≥4 chars that
/// aren't stopwords. Cheap thematic signal for the briefing.
fn keywords(title: &str) -> Vec<String> {
    title
        .split(|c: char| !c.is_alphanumeric())
        .map(str::to_lowercase)
        .filter(|w| w.len() >= 4 && !STOPWORDS.contains(&w.as_str()))
        .collect()
}

impl Synthesis {
    /// The dominant themes across the reading list — the most common significant
    /// title keywords (one vote per paper each), with how many papers carry each.
    /// Deterministic order (count desc, then alpha) so a briefing reproduces.
    pub(crate) fn themes(&self, top_n: usize) -> Vec<(String, usize)> {
        use std::collections::{HashMap, HashSet};
        let mut counts: HashMap<String, usize> = HashMap::new();
        for p in &self.papers {
            let mut seen = HashSet::new();
            for k in keywords(&p.title) {
                if seen.insert(k.clone()) {
                    *counts.entry(k).or_insert(0) += 1;
                }
            }
        }
        let mut v: Vec<(String, usize)> = counts.into_iter().filter(|(_, c)| *c >= 2).collect();
        v.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.cmp(&b.0)));
        v.truncate(top_n);
        v
    }

    /// Cheap topical clusters from shared title keywords. This is deliberately
    /// a proxy, not a claim of true co-citation clustering: the public search
    /// APIs in the fast path do not all return reference lists. Once refs are
    /// wired, this method is the seam to upgrade from keyword votes to shared-
    /// reference / co-citation clusters without changing the briefing contract.
    pub(crate) fn topic_clusters(
        &self,
        max_clusters: usize,
        anchors_per_cluster: usize,
    ) -> Vec<TopicCluster> {
        let mut clusters = Vec::new();
        for (theme, _) in self.themes(max_clusters.saturating_mul(3).max(max_clusters)) {
            let mut papers: Vec<&Paper> = self
                .papers
                .iter()
                .filter(|p| keywords(&p.title).iter().any(|k| k == &theme))
                .collect();
            if papers.len() < 2 {
                continue;
            }
            papers.sort_by(|a, b| {
                b.citations
                    .cmp(&a.citations)
                    .then_with(|| a.title.cmp(&b.title))
            });
            clusters.push(TopicCluster {
                theme,
                paper_count: papers.len(),
                citation_sum: papers.iter().map(|p| p.citations).sum(),
                anchors: papers
                    .iter()
                    .take(anchors_per_cluster.max(1))
                    .map(|p| p.line())
                    .collect(),
            });
        }
        clusters.sort_by(|a, b| {
            b.paper_count
                .cmp(&a.paper_count)
                .then_with(|| b.citation_sum.cmp(&a.citation_sum))
                .then_with(|| a.theme.cmp(&b.theme))
        });
        clusters.truncate(max_clusters);
        clusters
    }

    /// The seminal work (most cited) — the anchor a reader wants first.
    fn seminal(&self) -> Option<&Paper> {
        self.papers.iter().max_by_key(|p| p.citations)
    }

    /// The freshest work (latest year) — the current edge of the field.
    fn freshest(&self) -> Option<&Paper> {
        self.papers
            .iter()
            .filter_map(|p| p.year.map(|y| (y, p)))
            .max_by_key(|(y, _)| *y)
            .map(|(_, p)| p)
    }

    /// Metadata-only reading cues. No abstract or full text is fetched, so these
    /// labels deliberately avoid claim/evidence language.
    pub(crate) fn metadata_cues(&self, limit: usize) -> Vec<String> {
        self.papers
            .iter()
            .take(limit)
            .map(|p| {
                let title_l = p.title.to_ascii_lowercase();
                let kind = if ["survey", "review", "meta-analysis", "systematic"]
                    .iter()
                    .any(|m| title_l.contains(m))
                {
                    "survey/review"
                } else if ["benchmark", "evaluation", "comparison", "comparative"]
                    .iter()
                    .any(|m| title_l.contains(m))
                {
                    "evaluation"
                } else if [
                    "improve",
                    "improves",
                    "improving",
                    "increase",
                    "increases",
                    "reduce",
                    "reduces",
                    "enhance",
                    "enhances",
                ]
                .iter()
                .any(|m| title_l.contains(m))
                {
                    "effect/intervention"
                } else {
                    "method/finding"
                };
                let year = p.year.map_or_else(|| "n.d.".to_string(), |y| y.to_string());
                format!("{kind} cue: {} ({year}, {} cites)", p.title, p.citations)
            })
            .collect()
    }

    /// A briefing: themes, the seminal + freshest anchors, then the ranked list.
    /// This is the "synthesis" — orientation before the bibliography.
    pub(crate) fn brief(&self, limit: usize) -> String {
        let mut out = format!(
            "◇ synthesis · \"{}\"\n  {} works across {} source(s)\n",
            sanitize_metadata(&self.query, 240),
            self.papers.len(),
            self.sources_hit.len()
        );
        let themes = self.themes(5);
        if !themes.is_empty() {
            let t: Vec<String> = themes.iter().map(|(k, c)| format!("{k}×{c}")).collect();
            out.push_str(&format!("  themes: {}\n", t.join(", ")));
        }
        if let Some(s) = self.seminal() {
            out.push_str(&format!("  seminal: {}\n", s.line()));
        }
        if let Some(f) = self.freshest() {
            out.push_str(&format!("  freshest: {}\n", f.line()));
        }
        if !self.notes.is_empty() {
            let notes = self
                .notes
                .iter()
                .map(|note| sanitize_metadata(note, 240))
                .collect::<Vec<_>>();
            out.push_str(&format!("  (skipped: {})\n", notes.join("; ")));
        }
        let clusters = self.topic_clusters(4, 2);
        if !clusters.is_empty() {
            out.push_str(
                "  metadata clusters (shared title keywords only; not citation clusters):\n",
            );
            for c in clusters {
                out.push_str(&format!(
                    "    - {}: {} papers, {} citations\n",
                    c.theme, c.paper_count, c.citation_sum
                ));
                for anchor in c.anchors {
                    out.push_str(&format!("      * {anchor}\n"));
                }
            }
        }
        let cues = self.metadata_cues(3);
        if !cues.is_empty() {
            out.push_str(
                "  metadata cues (title text only; not evidence—read the paper before relying):\n",
            );
            for c in cues {
                out.push_str(&format!("    - {c}\n"));
            }
        }
        out.push_str("  ── reading list ──\n");
        for (i, p) in self.papers.iter().take(limit).enumerate() {
            out.push_str(&format!("  {:>2}. {}\n", i + 1, p.line()));
        }
        out
    }
}

/// The `/science <query>` command body: synthesize (cached, 1h TTL) and return a
/// briefing. No query → usage. Mirrors `crate::habits::run`.
pub(crate) fn run(arg: Option<&str>) -> String {
    let Some(query) = arg.map(str::trim).filter(|q| !q.is_empty()) else {
        return "usage: /science <query>\n  fans the query across OpenAlex, Crossref, Semantic \
                Scholar and Europe PMC, then folds the hits into a ranked, deduplicated briefing \
                (cached 1h under ~/.angel0/science/)."
            .to_string();
    };
    if query.chars().count() > MAX_QUERY_CHARS {
        return format!("/science query must be at most {MAX_QUERY_CHARS} characters");
    }
    let syn = synthesize_cached(query, 8, 3600);
    if syn.papers.is_empty() {
        let why = if syn.notes.is_empty() {
            "no results".to_string()
        } else {
            syn.notes.join("; ")
        };
        return format!(
            "/science \"{}\": {why}",
            sanitize_metadata(query, MAX_QUERY_CHARS)
        );
    }
    syn.brief(12)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn http_deadline_is_inherited_by_all_scoped_science_workers() {
        use std::io::{Read, Write};
        let _lock = crate::tests::env_lock();
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        listener.set_nonblocking(true).unwrap();
        let url = format!("http://{}/", listener.local_addr().unwrap());
        let server = std::thread::spawn(move || {
            // A worker may exhaust its inherited deadline before connecting.
            // Bound accept too: a socket read timeout cannot bound accept().
            let accept_deadline = std::time::Instant::now() + Duration::from_secs(2);
            std::thread::scope(|scope| {
                for _ in Source::all() {
                    let (mut socket, _) = loop {
                        match listener.accept() {
                            Ok(connection) => break connection,
                            Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                                if std::time::Instant::now() >= accept_deadline {
                                    return;
                                }
                                std::thread::sleep(Duration::from_millis(5));
                            }
                            Err(error) => panic!("science fixture accept failed: {error}"),
                        }
                    };
                    scope.spawn(move || {
                        socket
                            .set_read_timeout(Some(Duration::from_secs(5)))
                            .unwrap();
                        let mut request = Vec::new();
                        while !request.ends_with(b"\r\n\r\n") {
                            let mut byte = [0];
                            socket.read_exact(&mut byte).unwrap();
                            request.push(byte[0]);
                        }
                        socket.write_all(b"HTTP/1.1 200 OK\r\n\r\nseed").unwrap();
                        let mut byte = [0];
                        assert_eq!(socket.read(&mut byte).unwrap(), 0);
                    });
                }
            });
        });
        let start = std::time::Instant::now();
        let synthesis = crate::tools::http_transport::with_deadline(
            Some(start + Duration::from_millis(500)),
            || {
                synthesize_with("fixture", 1, &|_, _, _| {
                    crate::tools::http_transport::request("GET", &url, false, 0)
                        .call()
                        .map_err(|e| e.to_string())?
                        .into_string()
                        .map_err(|e| e.to_string())?;
                    Ok(Vec::new())
                })
            },
        );
        let request_elapsed = start.elapsed();
        server.join().unwrap();
        assert!(request_elapsed < Duration::from_secs(2));
        assert_eq!(synthesis.notes.len(), Source::all().len());
        for note in synthesis.notes {
            assert!(note.contains("stalled {"), "{note}");
            assert!(note.contains("\"bound\":\"deadline\""), "{note}");
            println!("joined science worker: {note}");
        }
    }

    #[test]
    fn openalex_parses_and_normalizes() {
        let v: serde_json::Value = serde_json::from_str(
            r#"{"results":[
                {"display_name":"Attention Is All You Need","publication_year":2017,
                 "doi":"https://doi.org/10.5555/ABC","cited_by_count":90000,
                 "id":"https://openalex.org/W1","authorships":[
                    {"author":{"display_name":"A Vaswani"}},
                    {"author":{"display_name":"N Shazeer"}}]},
                {"display_name":"","publication_year":2000}
            ]}"#,
        )
        .unwrap();
        let ps = parse_openalex(&v);
        assert_eq!(ps.len(), 1, "the empty-title record is dropped");
        assert_eq!(ps[0].doi.as_deref(), Some("10.5555/abc"));
        assert_eq!(ps[0].year, Some(2017));
        assert_eq!(ps[0].authors.len(), 2);
        assert_eq!(ps[0].citations, 90000);
        assert!(ps[0].line().contains("doi: https://doi.org/10.5555/abc"));
    }

    #[test]
    fn crossref_parses_authors_and_year() {
        let v: serde_json::Value = serde_json::from_str(
            r#"{"message":{"items":[
                {"title":["Deep Residual Learning"],"DOI":"10.1109/CVPR.2016.90",
                 "URL":"https://doi.org/10.1109/CVPR.2016.90","is-referenced-by-count":180000,
                 "issued":{"date-parts":[[2016,6]]},
                 "author":[{"given":"Kaiming","family":"He"},{"family":"Zhang"}]}
            ]}}"#,
        )
        .unwrap();
        let ps = parse_crossref(&v);
        assert_eq!(ps.len(), 1);
        assert_eq!(ps[0].authors[0], "Kaiming He");
        assert_eq!(ps[0].authors[1], "Zhang");
        assert_eq!(ps[0].year, Some(2016));
        assert_eq!(ps[0].doi.as_deref(), Some("10.1109/cvpr.2016.90"));
    }

    #[test]
    fn semantic_scholar_pulls_doi_from_external_ids() {
        let v: serde_json::Value = serde_json::from_str(
            r#"{"data":[
                {"title":"BERT","year":2019,"citationCount":70000,
                 "url":"https://www.semanticscholar.org/paper/x",
                 "externalIds":{"DOI":"10.18653/v1/N19-1423","ArXiv":"1810.04805"},
                 "authors":[{"name":"Jacob Devlin"},{"name":"Ming-Wei Chang"}]}
            ]}"#,
        )
        .unwrap();
        let ps = parse_semantic_scholar(&v);
        assert_eq!(ps.len(), 1);
        assert_eq!(ps[0].doi.as_deref(), Some("10.18653/v1/n19-1423"));
        assert_eq!(ps[0].year, Some(2019));
        assert_eq!(ps[0].authors[0], "Jacob Devlin");
    }

    #[test]
    fn europepmc_splits_authorstring_and_parses_year() {
        let v: serde_json::Value = serde_json::from_str(
            r#"{"resultList":{"result":[
                {"title":"CRISPR-Cas9 genome editing.","pubYear":"2014",
                 "authorString":"Doudna JA, Charpentier E.","doi":"10.1126/science.1258096",
                 "citedByCount":9000,"id":"25430774","source":"MED"}
            ]}}"#,
        )
        .unwrap();
        let ps = parse_europepmc(&v);
        assert_eq!(ps.len(), 1);
        assert_eq!(ps[0].title, "CRISPR-Cas9 genome editing");
        assert_eq!(ps[0].authors, vec!["Doudna JA", "Charpentier E"]);
        assert_eq!(ps[0].year, Some(2014));
        assert_eq!(ps[0].doi.as_deref(), Some("10.1126/science.1258096"));
        assert_eq!(
            ps[0].url.as_deref(),
            Some("https://europepmc.org/article/MED/25430774")
        );
    }

    #[test]
    fn all_four_sources_fan_out() {
        assert_eq!(Source::all().len(), 4);
    }

    #[test]
    fn fanout_is_concurrent_but_reduces_reverse_completion_canonically() {
        use std::sync::{Arc, Condvar, Mutex, mpsc};

        fn source_index(source: Source) -> usize {
            match source {
                Source::OpenAlex => 0,
                Source::Crossref => 1,
                Source::SemanticScholar => 2,
                Source::EuropePmc => 3,
            }
        }

        fn fixture(source: Source, citations: u64) -> Paper {
            Paper {
                title: format!("{} fixture", source.label()),
                authors: vec![],
                year: Some(2025),
                doi: Some(format!("10.1000/source{}", source_index(source))),
                url: None,
                citations,
                source,
            }
        }

        fn release_all(gates: &[(Mutex<bool>, Condvar)]) {
            for (released, gate) in gates {
                *released.lock().expect("gate lock") = true;
                gate.notify_all();
            }
        }

        let gates = Arc::new(
            (0..Source::all().len())
                .map(|_| (Mutex::new(false), Condvar::new()))
                .collect::<Vec<_>>(),
        );
        let completions = Arc::new(Mutex::new(Vec::new()));
        let worker_ids = Arc::new(Mutex::new(Vec::new()));
        let (started_tx, started_rx) = mpsc::channel();
        let (completed_tx, completed_rx) = mpsc::channel();

        let controller_gates = Arc::clone(&gates);
        let controller = std::thread::spawn(move || -> Result<(), String> {
            let mut started = Vec::new();
            for _ in Source::all() {
                match started_rx.recv_timeout(Duration::from_secs(2)) {
                    Ok(source) => started.push(source),
                    Err(error) => {
                        release_all(&controller_gates);
                        return Err(format!("not all source workers started: {error}"));
                    }
                }
            }
            started.sort_by_key(|source| source_index(*source));
            if started != Source::all() {
                release_all(&controller_gates);
                return Err(format!("unexpected source workers: {started:?}"));
            }

            for source in Source::all().into_iter().rev() {
                let (released, gate) = &controller_gates[source_index(source)];
                *released.lock().expect("gate lock") = true;
                gate.notify_one();
                match completed_rx.recv_timeout(Duration::from_secs(2)) {
                    Ok(completed) if completed == source => {}
                    Ok(completed) => {
                        release_all(&controller_gates);
                        return Err(format!("released {source:?}, but {completed:?} completed"));
                    }
                    Err(error) => {
                        release_all(&controller_gates);
                        return Err(format!("{source:?} did not complete: {error}"));
                    }
                }
            }
            Ok(())
        });

        let searcher = |source: Source, query: &str, per_source: usize| {
            assert_eq!(query, "bounded fanout");
            assert_eq!(per_source, 7);
            worker_ids
                .lock()
                .expect("worker id lock")
                .push(std::thread::current().id());
            started_tx.send(source).expect("announce worker");
            let (released, gate) = &gates[source_index(source)];
            let (released, timeout) = gate
                .wait_timeout_while(
                    released.lock().expect("gate lock"),
                    Duration::from_secs(2),
                    |released| !*released,
                )
                .expect("wait for release");
            assert!(
                *released && !timeout.timed_out(),
                "worker release timed out"
            );
            completions.lock().expect("completion lock").push(source);
            completed_tx.send(source).expect("announce completion");

            match source {
                Source::OpenAlex => Ok(vec![fixture(source, 10)]),
                Source::Crossref => Err("fixture unavailable".to_string()),
                Source::SemanticScholar => panic!("contained source panic"),
                Source::EuropePmc => Ok(vec![fixture(source, 20)]),
            }
        };

        let synthesis = synthesize_with("bounded fanout", 7, &searcher);
        controller
            .join()
            .expect("controller thread")
            .expect("controller result");

        assert_eq!(
            *completions.lock().expect("completion lock"),
            Source::all().into_iter().rev().collect::<Vec<_>>()
        );
        assert_eq!(
            synthesis.sources_hit,
            vec![Source::OpenAlex, Source::EuropePmc],
            "provenance follows canonical source order, not completion order"
        );
        assert_eq!(
            synthesis.notes,
            vec![
                "Crossref: fixture unavailable",
                "Semantic Scholar: source worker panicked",
            ]
        );
        assert_eq!(synthesis.papers.len(), 2);
        assert_eq!(synthesis.papers[0].source, Source::EuropePmc);

        let ids = worker_ids.lock().expect("worker id lock");
        assert_eq!(ids.len(), Source::all().len());
        let unique = ids.iter().collect::<std::collections::HashSet<_>>();
        assert_eq!(unique.len(), Source::all().len());
        assert!(ids.iter().all(|id| *id != std::thread::current().id()));
    }

    #[test]
    fn doi_normalization_accepts_canonical_shapes_and_rejects_injection() {
        assert_eq!(
            normalize_doi(" DOI:10.1234/ABC.def ").as_deref(),
            Some("10.1234/abc.def")
        );
        assert_eq!(
            normalize_doi("https://DX.DOI.org/10.123456789/a:b/c").as_deref(),
            Some("10.123456789/a:b/c")
        );
        for invalid in [
            "10.1/too-short",
            "11.1234/not-a-doi",
            "10.1234/",
            "10.1234/line\nbreak",
            "10.1234/x?redirect=https://evil.test",
            "javascript:alert(1)",
        ] {
            assert_eq!(normalize_doi(invalid), None, "accepted {invalid:?}");
        }
    }

    #[test]
    fn record_links_are_source_bound_https_urls() {
        assert!(trusted_source_url(Source::OpenAlex, "https://openalex.org/W123").is_some());
        assert!(
            trusted_source_url(
                Source::SemanticScholar,
                "https://www.semanticscholar.org/paper/title/abc-123"
            )
            .is_some()
        );
        assert!(
            trusted_source_url(
                Source::EuropePmc,
                "https://europepmc.org/article/MED/25430774"
            )
            .is_some()
        );
        for (source, url) in [
            (Source::OpenAlex, "http://openalex.org/W123"),
            (Source::OpenAlex, "https://evil.test/W123"),
            (Source::OpenAlex, "https://openalex.org/W123?next=evil"),
            (
                Source::SemanticScholar,
                "https://user@www.semanticscholar.org/paper/x",
            ),
            (
                Source::EuropePmc,
                "https://europepmc.org/article/MED/../secret",
            ),
        ] {
            assert_eq!(trusted_source_url(source, url), None, "accepted {url}");
        }
    }

    #[test]
    fn hostile_index_metadata_cannot_create_rows_or_links() {
        let v = serde_json::json!({
            "results": [{
                "display_name": "Paper\n  1. forged\u{001b}[31m\u{202e}spoof",
                "doi": "10.1234/line\nbreak",
                "id": "javascript:alert(1)",
                "authorships": [{"author": {"display_name": "A\nAuthor\u{200b}"}}]
            }]
        });
        let papers = parse_openalex(&v);
        assert_eq!(papers.len(), 1);
        assert_eq!(papers[0].title, "Paper 1. forged [31m spoof");
        assert_eq!(papers[0].authors, vec!["A Author"]);
        assert_eq!(papers[0].doi, None);
        assert_eq!(papers[0].url, None);
        let line = papers[0].line();
        assert_eq!(line.lines().count(), 1, "{line:?}");
        assert!(line.contains("citation unavailable"), "{line}");
        assert!(!line.chars().any(char::is_control), "{line:?}");
    }

    #[test]
    fn citation_falls_back_to_a_validated_source_record() {
        let paper = Paper {
            title: "No DOI record".into(),
            authors: vec![],
            year: None,
            doi: None,
            url: Some("https://www.semanticscholar.org/paper/title/abc-123".into()),
            citations: 0,
            source: Source::SemanticScholar,
        };
        assert!(
            paper
                .line()
                .contains("source: https://www.semanticscholar.org/paper/title/abc-123")
        );

        let cached_hostile = Paper {
            url: Some("https://evil.test/paper/fake".into()),
            ..paper
        };
        assert!(cached_hostile.line().contains("citation unavailable"));
    }

    #[test]
    fn decoded_json_body_is_bounded_before_parsing() {
        let parsed = read_bounded_json(std::io::Cursor::new(br#"{"ok":true}"#), 32).unwrap();
        assert_eq!(parsed["ok"], true);

        let err = read_bounded_json(std::io::Cursor::new(vec![b' '; 33]), 32).unwrap_err();
        assert!(err.contains("exceeded 32-byte"), "{err}");
    }

    #[test]
    fn fold_dedupes_by_doi_keeping_the_better_cited() {
        let mk = |src, cites| Paper {
            title: "Same Paper".into(),
            authors: vec![],
            year: Some(2020),
            doi: Some("10.1000/x".into()),
            url: None,
            citations: cites,
            source: src,
        };
        let folded = fold(vec![mk(Source::OpenAlex, 10), mk(Source::Crossref, 42)]);
        assert_eq!(folded.len(), 1, "one DOI → one entry");
        assert_eq!(folded[0].citations, 42, "the richer record survives");
    }

    #[test]
    fn fold_ranks_by_citations_then_recency() {
        let paper = |t: &str, y, c| Paper {
            title: t.into(),
            authors: vec![],
            year: Some(y),
            doi: Some(format!("10.1000/{t}")),
            url: None,
            citations: c,
            source: Source::OpenAlex,
        };
        let folded = fold(vec![
            paper("low", 2021, 5),
            paper("high", 2010, 5000),
            paper("mid", 2023, 50),
        ]);
        assert_eq!(folded[0].title, "high", "citations dominate the ranking");
    }

    #[test]
    fn sane_year_clamps_out_of_band_values() {
        assert_eq!(super::sane_year(2023), Some(2023));
        assert_eq!(super::sane_year(1600), Some(1600));
        assert_eq!(
            super::sane_year(9999),
            None,
            "a far-future year is dropped, not trusted"
        );
        assert_eq!(
            super::sane_year(300),
            None,
            "an implausibly ancient year is dropped"
        );
        assert_eq!(super::sane_year(0), None);
    }

    #[test]
    fn future_dated_record_does_not_hijack_the_freshest_anchor() {
        // A hostile 9999 year is dropped to None, so a genuine 2024 paper stays
        // the freshest rather than being buried by the corrupt record.
        let body = serde_json::json!({
            "results": [
                { "display_name": "corrupt future", "publication_year": 9999 },
                { "display_name": "genuine recent", "publication_year": 2024 },
            ]
        });
        let papers = parse_openalex(&body);
        let corrupt = papers.iter().find(|p| p.title == "corrupt future").unwrap();
        assert_eq!(corrupt.year, None, "9999 is rejected");
        let genuine = papers.iter().find(|p| p.title == "genuine recent").unwrap();
        assert_eq!(genuine.year, Some(2024));
    }

    #[test]
    fn result_container_present_distinguishes_missing_from_empty() {
        // Present-but-empty is a legitimate no-results (no note).
        assert!(result_container_present(
            Source::OpenAlex,
            &serde_json::json!({ "results": [] })
        ));
        // Missing/renamed container is schema drift (a noted skip).
        assert!(!result_container_present(
            Source::OpenAlex,
            &serde_json::json!({ "meta": { "count": 0 } })
        ));
        assert!(result_container_present(
            Source::Crossref,
            &serde_json::json!({ "message": { "items": [] } })
        ));
        assert!(!result_container_present(
            Source::Crossref,
            &serde_json::json!({ "message": {} })
        ));
        assert_eq!(container_key(Source::SemanticScholar), "data");
    }

    #[test]
    fn doi_less_records_do_not_merge_on_a_shared_generic_title() {
        let paper = |author: &str, year| Paper {
            title: "Introduction".into(),
            authors: vec![author.into()],
            year: Some(year),
            doi: None,
            url: None,
            citations: 1,
            source: Source::EuropePmc,
        };
        // Same title, different first author + year → two distinct works.
        let folded = fold(vec![paper("Alice Smith", 2019), paper("Bob Jones", 2024)]);
        assert_eq!(
            folded.len(),
            2,
            "distinct DOI-less works are not collapsed by title"
        );
    }

    #[test]
    fn empty_title_records_never_collapse_into_one() {
        let blank = |src, url: &str| Paper {
            title: "  ".into(), // squashes to an empty normalized title
            authors: vec![],
            year: None,
            doi: None,
            url: Some(url.into()),
            citations: 0,
            source: src,
        };
        let folded = fold(vec![
            blank(Source::OpenAlex, "https://openalex.org/W1"),
            blank(Source::EuropePmc, "https://europepmc.org/article/MED/2"),
        ]);
        assert_eq!(
            folded.len(),
            2,
            "empty-title records key uniquely, not into one row"
        );
    }

    #[test]
    fn urlencode_escapes_spaces_and_symbols() {
        assert_eq!(urlencode("graph neural"), "graph+neural");
        assert_eq!(urlencode("CRISPR/Cas9"), "CRISPR%2FCas9");
    }

    fn paper(title: &str, cites: u64, year: u32) -> Paper {
        Paper {
            title: title.into(),
            authors: vec!["Author One".into()],
            year: Some(year),
            doi: Some(format!("10.1000/{}", normalize_title(title))),
            url: None,
            citations: cites,
            source: Source::OpenAlex,
        }
    }

    #[test]
    fn themes_surface_shared_title_keywords() {
        let syn = Synthesis {
            query: "q".into(),
            papers: vec![
                paper("Graph Neural Networks for Molecules", 10, 2020),
                paper("Graph Neural Networks at Scale", 10, 2021),
                paper("Attention over Graph structures", 10, 2022),
            ],
            sources_hit: vec![Source::OpenAlex],
            notes: vec![],
        };
        let themes = syn.themes(5);
        // "graph" is in all three; the ≥2 filter keeps the shared terms only.
        assert_eq!(themes[0], ("graph".to_string(), 3));
        assert!(themes.iter().all(|(_, c)| *c >= 2));
    }

    #[test]
    fn brief_leads_with_seminal_and_freshest() {
        let syn = Synthesis {
            query: "transformers".into(),
            papers: vec![
                paper("small recent", 3, 2023),
                paper("Attention Is All You Need", 90000, 2017),
            ],
            sources_hit: vec![Source::OpenAlex],
            notes: vec![],
        };
        let b = syn.brief(10);
        assert!(b.contains("seminal: Attention Is All You Need"), "{b}");
        assert!(b.contains("freshest: small recent"), "{b}");
    }

    #[test]
    fn topic_clusters_group_shared_keywords_with_anchors() {
        let syn = Synthesis {
            query: "q".into(),
            papers: vec![
                paper("Graph Neural Networks for Molecules", 100, 2020),
                paper("Graph Neural Networks at Scale", 10, 2021),
                paper("Transformer Protein Folding", 50, 2022),
            ],
            sources_hit: vec![Source::OpenAlex],
            notes: vec![],
        };
        let clusters = syn.topic_clusters(3, 2);
        assert_eq!(clusters[0].theme, "graph");
        assert_eq!(clusters[0].paper_count, 2);
        assert_eq!(clusters[0].anchors.len(), 2);
        assert!(clusters[0].anchors[0].contains("Molecules"));
    }

    #[test]
    fn metadata_cues_never_claim_to_be_evidence() {
        let syn = Synthesis {
            query: "q".into(),
            papers: vec![
                paper("A Survey of Graph Neural Networks", 100, 2020),
                paper("Benchmark Evaluation for Graph Models", 10, 2021),
            ],
            sources_hit: vec![Source::OpenAlex],
            notes: vec![],
        };
        let cues = syn.metadata_cues(2);
        assert!(cues[0].starts_with("survey/review cue"));
        assert!(cues[1].starts_with("evaluation cue"));
        let brief = syn.brief(2);
        assert!(brief.contains("metadata cues (title text only; not evidence"));
        assert!(brief.contains("not citation clusters"));
        assert!(!brief.contains("claim signals"));
    }

    #[test]
    fn run_with_no_query_is_usage_not_a_crash() {
        assert!(run(None).starts_with("usage:"));
        assert!(run(Some("   ")).starts_with("usage:"));
    }

    #[test]
    fn prune_drops_stale_entries_and_keeps_fresh_ones() {
        let dir = std::env::temp_dir().join(format!("angel_sci_prune_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();

        let stale = dir.join("old.json");
        let fresh = dir.join("new.json");
        let other = dir.join("keep.txt");
        for p in [&stale, &fresh, &other] {
            std::fs::write(p, "{}").unwrap();
        }
        // Age the stale entry well past the horizon.
        let past = std::time::SystemTime::now() - Duration::from_secs(120);
        std::fs::File::options()
            .write(true)
            .open(&stale)
            .unwrap()
            .set_modified(past)
            .unwrap();

        prune_stale(&dir, 60);
        assert!(!stale.exists(), "stale .json entry is removed");
        assert!(fresh.exists(), "fresh entry survives");
        assert!(other.exists(), "non-json files are never touched");

        let _ = std::fs::remove_dir_all(&dir);
    }
}
