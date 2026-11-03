//! Scientific literature synthesis: native network and citation authority,
//! with provider field mapping, ranking and briefing in a reloadable module.
//!
//! `OpenScience` (github.com/newjordan/openscience) exposes ~30 scientific
//! databases as agent tools. The valuable, load-bearing part of that is not the
//! browser workspace or the 290 skills — it is *the literature layer*: one query
//! fanned across the major indices, deduplicated, and ranked into a reading
//! list the agent can act on. That is what this module ports.
//!
//! It talks to the free, key-less JSON APIs with the cockpit's existing
//! blocking transport. It fans out across sources while the host owns network
//! authority, bounded responses, URL validation and cache persistence. The
//! replaceable analysis module is invoked per request; it receives no ambient
//! credentials and its implementation does not make network calls.

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

/// A private copy of the complete currently declared JS dependency set.
/// All workers for one synthesis use these exact bytes, even when the source
/// files are edited while requests are running. Drop removes the generation.
#[derive(Debug)]
struct ScienceSnapshot {
    generation: crate::platform::worker_snapshot::Snapshot,
    protocol: ExtractionProtocol,
}

impl std::ops::Deref for ScienceSnapshot {
    type Target = crate::platform::worker_snapshot::Snapshot;
    fn deref(&self) -> &Self::Target {
        &self.generation
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum ExtractionProtocol {
    Legacy,
    Batch,
}

/// Identity of the shipped v1 entry point, before capability negotiation was
/// introduced. Unknown entry points must identify themselves explicitly; a
/// generic worker failure is never mistaken for the older protocol.
const LEGACY_ENTRY_SHA256: &str =
    "2e6ed05d3a64b2f2c0a6a568595ee1063a297fdf2fb75fbcd8e5312e8eeb3f59";

impl ScienceSnapshot {
    fn load() -> Result<std::sync::Arc<Self>, String> {
        const FILES: &[&str] = &["scripts/runtime/science-brief.mjs", "lib/science/brief.mjs"];
        let generation = crate::platform::worker_snapshot::Snapshot::capture(
            &crate::platform::runtime_paths::root(),
            FILES,
            "science",
        )?;
        let entry =
            crate::platform::worker_snapshot::read(&generation.dir.join(FILES[0]), "science")?;
        let protocol = if crate::knowledge::cut::sha256_hex(&entry) == LEGACY_ENTRY_SHA256 {
            ExtractionProtocol::Legacy
        } else {
            ExtractionProtocol::Batch
        };
        let snapshot = std::sync::Arc::new(Self {
            generation,
            protocol,
        });
        // Import the staged dependency set and exercise a side-effect-free
        // empty fold before admitting network work. This is a readiness probe,
        // not a guarantee about arbitrary future inputs.
        match snapshot.protocol {
            ExtractionProtocol::Legacy => {
                let probe: Vec<usize> =
                    science_worker(&snapshot, &serde_json::json!({"op":"fold", "papers":[]}))
                        .map_err(|e| format!("science module preparation failed: {e}"))?;
                if !probe.is_empty() {
                    return Err("science module preparation returned unexpected data".into());
                }
            }
            ExtractionProtocol::Batch => {
                let probe: serde_json::Value =
                    science_worker(&snapshot, &serde_json::json!({"op":"capabilities"})).map_err(
                        |e| format!("science module capability negotiation failed: {e}"),
                    )?;
                if probe.get("protocol").and_then(serde_json::Value::as_u64) != Some(2)
                    || probe
                        .get("extract_batch")
                        .and_then(serde_json::Value::as_bool)
                        != Some(true)
                    || probe
                        .get("batch_source_identity")
                        .and_then(serde_json::Value::as_bool)
                        != Some(true)
                    || probe
                        .get("empty_fold")
                        .and_then(serde_json::Value::as_array)
                        .is_none_or(|v| !v.is_empty())
                {
                    return Err("science module has an unsupported capability contract".into());
                }
            }
        }
        Ok(snapshot)
    }
}

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
    fn protocol_id(self) -> &'static str {
        match self {
            Source::OpenAlex => "OpenAlex",
            Source::Crossref => "Crossref",
            Source::SemanticScholar => "SemanticScholar",
            Source::EuropePmc => "EuropePmc",
        }
    }

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
    /// Pre-deduplication candidates retain enough evidence to re-rank a cached
    /// search under the next policy generation. Old cache files omit them.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    candidates: Option<Vec<Paper>>,
    /// Per-request immutable code generation; never persisted in the cache.
    #[serde(skip)]
    snapshot: Option<std::sync::Arc<ScienceSnapshot>>,
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
    synthesize_impl(query, per_source, &|source, query, limit, snapshot| {
        search(source, query, limit, snapshot).map(SourcePayload::Raw)
    })
}

enum SourcePayload {
    /// The native HTTP worker returns parsed but otherwise untrusted metadata.
    Raw(serde_json::Value),
    /// Offline test injection retains the same fan-out seam without network IO.
    #[cfg(test)]
    Papers(Vec<Paper>),
}

enum SourceWorkerOutcome {
    Completed(Result<SourcePayload, String>),
    Panicked,
}

/// Injected fan-out core. Exactly one scoped worker owns each fixed source;
/// every worker is joined, then outcomes are reduced in [`Source::all`] order.
/// That keeps diagnostics and source provenance deterministic even when the
/// network completes in reverse order. A worker panic is source-attributed and
/// degraded like any other unavailable index rather than unwinding the tool.
#[cfg(test)]
fn synthesize_with<F>(query: &str, per_source: usize, searcher: &F) -> Synthesis
where
    F: Fn(Source, &str, usize, &ScienceSnapshot) -> Result<Vec<Paper>, String> + Sync,
{
    synthesize_impl(query, per_source, &|source, query, limit, snapshot| {
        searcher(source, query, limit, snapshot).map(SourcePayload::Papers)
    })
}

fn synthesize_impl<F>(query: &str, per_source: usize, searcher: &F) -> Synthesis
where
    F: Fn(Source, &str, usize, &ScienceSnapshot) -> Result<SourcePayload, String> + Sync,
{
    let snapshot = match ScienceSnapshot::load() {
        Ok(s) => s,
        Err(e) => {
            return Synthesis {
                query: query.to_string(),
                notes: vec![format!("science analysis: {e}")],
                ..Synthesis::default()
            };
        }
    };
    let outcomes = std::thread::scope(|scope| {
        let handles = Source::all()
            .into_iter()
            .map(|source| {
                let snapshot = &snapshot;
                let http_context = crate::agent::tools::http_transport::context();
                (
                    source,
                    scope.spawn(move || {
                        match crate::ui::term::catch_background_unwind(|| {
                            crate::agent::tools::http_transport::with_context(http_context, || {
                                searcher(source, query, per_source, snapshot)
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

    // Keep outcomes in source order. Raw bodies are mapped together after all
    // native HTTP workers have completed; this module performs no HTTP calls.
    let mut ordered = Vec::with_capacity(outcomes.len());
    let mut batch = Vec::new();
    for (source, outcome) in outcomes {
        let index = ordered.len();
        let result = match outcome {
            #[cfg(test)]
            Ok(SourceWorkerOutcome::Completed(Ok(SourcePayload::Papers(papers)))) => {
                Some(Ok(papers))
            }
            Ok(SourceWorkerOutcome::Completed(Ok(SourcePayload::Raw(mut body)))) => {
                match prepare_provider_body(source, &mut body) {
                    Some(count) => {
                        batch.push((index, source, count, body));
                        None
                    }
                    None => Some(Err(format!(
                        "unexpected response shape (missing `{}`)",
                        container_key(source)
                    ))),
                }
            }
            Ok(SourceWorkerOutcome::Completed(Err(error))) => Some(Err(error)),
            Ok(SourceWorkerOutcome::Panicked) | Err(_) => {
                Some(Err("source worker panicked".to_string()))
            }
        };
        ordered.push((source, result));
    }
    if !batch.is_empty() && snapshot.protocol == ExtractionProtocol::Legacy {
        // The exact previously shipped entry point is known to support the
        // single-source protocol. Do not retry generic batch failures here.
        for (index, source, count, body) in batch.drain(..) {
            let result = science_worker(
                &snapshot,
                &serde_json::json!({"op":"extract", "source":source, "body":body}),
            )
            .and_then(|items| admit_extracted(source, items, count));
            ordered[index].1 = Some(result);
        }
    }
    if !batch.is_empty() {
        let requests: Vec<_> = batch
            .iter()
            .map(|(_, source, _, body)| serde_json::json!({"source":source, "body":body}))
            .collect();
        let responses: Result<Vec<serde_json::Value>, String> = science_worker_limited(
            &snapshot,
            &serde_json::json!({"op":"extract-batch", "items":requests}),
            32 * 1024 * 1024,
            16 * 1024 * 1024,
        );
        match responses {
            Ok(responses) if responses.len() == batch.len() => {
                for ((index, source, count, _), response) in batch.into_iter().zip(responses) {
                    let result = if response.get("source").and_then(serde_json::Value::as_str)
                        != Some(source.protocol_id())
                    {
                        Err("science module returned a mismatched batch source".into())
                    } else {
                        match response.get("papers") {
                            Some(value) => {
                                serde_json::from_value::<Vec<ExtractedPaper>>(value.clone())
                                    .map_err(|e| format!("invalid science module result: {e}"))
                                    .and_then(|items| admit_extracted(source, items, count))
                            }
                            None => Err("science module could not extract source records".into()),
                        }
                    };
                    ordered[index].1 = Some(result);
                }
            }
            Ok(_) => {
                for (index, _, _, _) in batch {
                    ordered[index].1 =
                        Some(Err("science module returned an invalid batch length".into()));
                }
            }
            Err(error) => {
                for (index, _, _, _) in batch {
                    ordered[index].1 = Some(Err(error.clone()));
                }
            }
        }
    }
    let mut all: Vec<Paper> = Vec::new();
    let mut hit = Vec::new();
    let mut notes = Vec::new();
    for (source, result) in ordered {
        match result.expect("each source was resolved") {
            Ok(mut papers) => {
                if !papers.is_empty() {
                    hit.push(source);
                }
                all.append(&mut papers);
            }
            Err(error) => notes.push(format!("{}: {error}", source.label())),
        }
    }
    let candidates = all.clone();
    let papers = match fold_with(&snapshot, all) {
        Ok(papers) => papers,
        Err(error) => {
            notes.push(format!("science analysis: {error}"));
            Vec::new()
        }
    };
    Synthesis {
        query: query.to_string(),
        papers,
        sources_hit: hit,
        notes,
        candidates: Some(candidates),
        snapshot: Some(snapshot),
    }
}

/// Fold via a replaceable policy module. The native host owns each paper,
/// its citation validation and identity; the worker returns indices only.
#[cfg(test)]
fn fold(papers: Vec<Paper>) -> Result<Vec<Paper>, String> {
    let snapshot = ScienceSnapshot::load()?;
    fold_with(snapshot.as_ref(), papers)
}

fn fold_with(snapshot: &ScienceSnapshot, papers: Vec<Paper>) -> Result<Vec<Paper>, String> {
    use std::collections::HashSet;
    if papers.is_empty() {
        return Ok(Vec::new());
    }
    let rows: Vec<_> = papers
        .iter()
        .enumerate()
        .map(|(index, p)| {
            serde_json::json!({
                "index": index, "key": p.key(), "title": p.title, "year": p.year,
                "citations": p.citations.to_string(),
                "valid_doi": p.doi.as_deref().and_then(normalize_doi).is_some()
            })
        })
        .collect();
    let indices: Vec<usize> =
        science_worker(snapshot, &serde_json::json!({"op":"fold", "papers":rows}))?;
    let expected: HashSet<String> = papers.iter().map(Paper::key).collect();
    let mut seen = HashSet::new();
    let mut selected = Vec::with_capacity(indices.len().min(papers.len()));
    for index in indices {
        let paper = papers
            .get(index)
            .ok_or("science module returned an invalid paper index")?;
        if !seen.insert(paper.key()) {
            return Err("science module returned duplicate paper identities".into());
        }
        selected.push(paper.clone());
    }
    if seen != expected {
        return Err("science module omitted a paper identity".into());
    }
    Ok(selected)
}

fn science_worker<T: serde::de::DeserializeOwned>(
    snapshot: &ScienceSnapshot,
    request: &serde_json::Value,
) -> Result<T, String> {
    science_worker_limited(snapshot, request, 8 * 1024 * 1024, 4 * 1024 * 1024)
}

fn science_worker_limited<T: serde::de::DeserializeOwned>(
    snapshot: &ScienceSnapshot,
    request: &serde_json::Value,
    input_limit: usize,
    output_limit: usize,
) -> Result<T, String> {
    let node = crate::agent::tools::runtime_missing::resolve("node")
        .map_err(|_| "Node.js unavailable (install Node or set ANGEL_NODE_BIN)".to_string())?;
    let mut command = std::process::Command::new(node);
    command
        .env_clear()
        .arg(snapshot.dir.join("scripts/runtime/science-brief.mjs"));
    let input = serde_json::to_vec(request).map_err(|e| format!("science request: {e}"))?;
    let context = crate::agent::tools::http_transport::context();
    let output = crate::platform::worker::run_cancellable(
        command,
        &input,
        std::time::Duration::from_secs(10),
        input_limit,
        output_limit,
        || context.interrupted(),
    )?;
    serde_json::from_slice(&output).map_err(|e| format!("invalid science module result: {e}"))
}

/// Query one source. Networked; the pure `parse_*` folders below carry the
/// logic and are what the tests exercise.
fn search(
    src: Source,
    query: &str,
    limit: usize,
    _snapshot: &ScienceSnapshot,
) -> Result<serde_json::Value, String> {
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
    let response = crate::agent::tools::http_transport::request("GET", &url, true, 5)
        .set(
            "User-Agent",
            "angelX-science/0.1 (+https://github.com/newjordan/angelX)",
        )
        .call()
        .map_err(|e| e.to_string())?;
    let body = read_bounded_json(response.into_reader(), SCIENCE_RESPONSE_MAX_BYTES)?;
    // Distinguish schema drift or a 200 error-envelope (the expected result
    // container is absent) from a legitimate empty result set (container present
    // but empty). The former is a degraded source worth a note, not a silent
    // "no matches" — [`synthesize_with`] folds the Err into the briefing's
    // skipped-sources line.
    if !result_container_present(src, &body) {
        return Err(format!(
            "unexpected response shape (missing `{}`)",
            container_key(src)
        ));
    }
    Ok(body)
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

/// A module can select source fields but never creates a trusted citation:
/// the native host validates every returned identifier and sanitizes all text.
#[derive(Deserialize)]
struct ExtractedPaper {
    title: Option<String>,
    authors: Vec<String>,
    year: Option<String>,
    doi: Option<String>,
    url: Option<String>,
    citations: Option<String>,
    record_source: Option<String>,
    record_id: Option<String>,
}

fn source_records_mut(
    source: Source,
    body: &mut serde_json::Value,
) -> Option<&mut Vec<serde_json::Value>> {
    match source {
        Source::OpenAlex => body.get_mut("results"),
        Source::Crossref => body.get_mut("message").and_then(|v| v.get_mut("items")),
        Source::SemanticScholar => body.get_mut("data"),
        Source::EuropePmc => body.get_mut("resultList").and_then(|v| v.get_mut("result")),
    }
    .and_then(serde_json::Value::as_array_mut)
}

fn preserve_unsigned(value: Option<&mut serde_json::Value>) {
    if let Some(v) = value {
        *v = v
            .as_u64()
            .map(|n| serde_json::Value::String(n.to_string()))
            .unwrap_or(serde_json::Value::Null);
    }
}

#[cfg(test)]
fn parse_papers(source: Source, body: &serde_json::Value) -> Result<Vec<Paper>, String> {
    let snapshot = ScienceSnapshot::load()?;
    parse_papers_with(snapshot.as_ref(), source, body)
}

fn parse_papers_with(
    snapshot: &ScienceSnapshot,
    source: Source,
    body: &serde_json::Value,
) -> Result<Vec<Paper>, String> {
    let mut body = body.clone();
    let Some(record_count) = prepare_provider_body(source, &mut body) else {
        return Ok(Vec::new());
    };
    let extracted: Vec<ExtractedPaper> = science_worker(
        snapshot,
        &serde_json::json!({"op":"extract", "source":source, "body":body}),
    )?;
    admit_extracted(source, extracted, record_count)
}

/// Preserve integer identities before the JavaScript boundary. Provider years
/// and citation counts stay exact even above JavaScript's safe integer range.
fn prepare_provider_body(source: Source, body: &mut serde_json::Value) -> Option<usize> {
    let records = source_records_mut(source, body)?;
    let record_count = records.len();
    for row in records {
        let (year, cites) = match source {
            Source::OpenAlex => ("publication_year", "cited_by_count"),
            Source::Crossref => ("", "is-referenced-by-count"),
            Source::SemanticScholar => ("year", "citationCount"),
            Source::EuropePmc => ("", "citedByCount"),
        };
        if !year.is_empty() {
            preserve_unsigned(row.get_mut(year));
        }
        preserve_unsigned(row.get_mut(cites));
        if source == Source::Crossref {
            preserve_unsigned(
                row.get_mut("issued")
                    .and_then(|v| v.get_mut("date-parts"))
                    .and_then(|v| v.as_array_mut())
                    .and_then(|v| v.first_mut())
                    .and_then(|v| v.as_array_mut())
                    .and_then(|v| v.first_mut()),
            );
        }
    }
    Some(record_count)
}

/// Convert untrusted module output using the same native limits and citation
/// validation as the single-provider path.
fn admit_extracted(
    source: Source,
    extracted: Vec<ExtractedPaper>,
    record_count: usize,
) -> Result<Vec<Paper>, String> {
    if extracted.len() != record_count {
        return Err("science module returned an invalid record count".into());
    }
    let mut papers = Vec::new();
    for item in extracted {
        let title = sanitize_metadata(item.title.as_deref().unwrap_or(""), MAX_TITLE_CHARS);
        if title.is_empty() {
            continue;
        }
        let authors = item
            .authors
            .iter()
            .map(|a| sanitize_metadata(a, MAX_AUTHOR_CHARS))
            .filter(|a| !a.is_empty())
            .take(4)
            .collect();
        let url = if source == Source::EuropePmc {
            match (item.record_source.as_deref(), item.record_id.as_deref()) {
                (Some(s), Some(id)) if valid_record_component(s) && valid_record_component(id) => {
                    trusted_source_url(source, &format!("https://europepmc.org/article/{s}/{id}"))
                }
                _ => None,
            }
        } else {
            item.url
                .as_deref()
                .and_then(|u| trusted_source_url(source, u))
        };
        papers.push(Paper {
            title,
            authors,
            year: item
                .year
                .as_deref()
                .and_then(|y| y.parse::<u64>().ok())
                .and_then(sane_year),
            doi: item.doi.as_deref().and_then(normalize_doi),
            url,
            citations: item
                .citations
                .as_deref()
                .and_then(|n| n.parse::<u64>().ok())
                .unwrap_or(0),
            source,
        });
    }
    Ok(papers)
}

// Existing offline parser fixtures use these compatibility names.
#[cfg(test)]
fn parse_openalex(v: &serde_json::Value) -> Vec<Paper> {
    parse_papers(Source::OpenAlex, v).expect("science extraction")
}
#[cfg(test)]
fn parse_crossref(v: &serde_json::Value) -> Vec<Paper> {
    parse_papers(Source::Crossref, v).expect("science extraction")
}
#[cfg(test)]
fn parse_semantic_scholar(v: &serde_json::Value) -> Vec<Paper> {
    parse_papers(Source::SemanticScholar, v).expect("science extraction")
}
#[cfg(test)]
fn parse_europepmc(v: &serde_json::Value) -> Vec<Paper> {
    parse_papers(Source::EuropePmc, v).expect("science extraction")
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

/// Where synthesis results memoize (`~/.angelX/science/<hash>.json`). `None`
/// outside a HOME, so unit-test worlds never touch the disk.
fn cache_dir() -> Option<std::path::PathBuf> {
    std::env::var_os("HOME").map(|h| std::path::Path::new(&h).join(".angelX").join("science"))
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
        let snapshot = match ScienceSnapshot::load() {
            Ok(s) => s,
            Err(e) => {
                return Synthesis {
                    query: query.to_string(),
                    notes: vec![format!("science analysis: {e}")],
                    ..Synthesis::default()
                };
            }
        };
        let mut hit = hit;
        let candidates = hit.candidates.clone().unwrap_or_else(|| hit.papers.clone());
        match fold_with(&snapshot, candidates) {
            Ok(papers) => hit.papers = papers,
            Err(e) => {
                hit.papers.clear();
                hit.notes.push(format!("science analysis: {e}"));
            }
        }
        hit.snapshot = Some(snapshot);
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
    if let Ok(raw) = crate::platform::secrets::to_redacted_vec(syn) {
        let _ = crate::platform::workspace_store::write_atomic(path, &raw);
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

// Analysis and briefing are loaded per invocation. The host formats each
// citation first so a worker cannot bypass the native URL/DOI checks.
#[derive(Deserialize)]
struct ScienceAnalysis {
    brief: String,
    #[cfg(test)]
    themes: Vec<(String, usize)>,
    #[cfg(test)]
    clusters: Vec<TopicCluster>,
    #[cfg(test)]
    cues: Vec<String>,
}

#[cfg(test)]
#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
pub(crate) struct TopicCluster {
    pub(crate) theme: String,
    pub(crate) paper_count: usize,
    pub(crate) citation_sum: String,
    pub(crate) anchors: Vec<String>,
}

impl Synthesis {
    fn analysis(
        &self,
        limit: usize,
        theme_limit: usize,
        cluster_limit: usize,
        anchor_limit: usize,
        cue_limit: usize,
    ) -> Result<ScienceAnalysis, String> {
        let papers: Vec<_> = self.papers.iter().map(|p| serde_json::json!({
            "title": p.title, "year": p.year, "citations": p.citations.to_string(), "line": p.line()
        })).collect();
        let notes: Vec<_> = self
            .notes
            .iter()
            .map(|n| sanitize_metadata(n, 240))
            .collect();
        let snapshot = match &self.snapshot {
            Some(s) => s.clone(),
            None => ScienceSnapshot::load()?,
        };
        science_worker(
            &snapshot,
            &serde_json::json!({"op":"analyze", "query":sanitize_metadata(&self.query, 240),
            "papers":papers, "notes":notes, "source_count":self.sources_hit.len(), "limit":limit,
            "theme_limit":theme_limit, "cluster_limit":cluster_limit, "anchor_limit":anchor_limit, "cue_limit":cue_limit}),
        )
    }

    #[cfg(test)]
    pub(crate) fn themes(&self, top_n: usize) -> Vec<(String, usize)> {
        self.analysis(0, top_n, 0, 0, 0)
            .expect("science analysis")
            .themes
    }

    #[cfg(test)]
    pub(crate) fn topic_clusters(
        &self,
        max_clusters: usize,
        anchors_per_cluster: usize,
    ) -> Vec<TopicCluster> {
        self.analysis(0, 0, max_clusters, anchors_per_cluster, 0)
            .expect("science analysis")
            .clusters
    }

    #[cfg(test)]
    pub(crate) fn metadata_cues(&self, limit: usize) -> Vec<String> {
        self.analysis(0, 0, 0, 0, limit)
            .expect("science analysis")
            .cues
    }

    pub(crate) fn brief(&self, limit: usize) -> Result<String, String> {
        self.analysis(limit, 5, 4, 2, 3)
            .map(|v| v.brief)
            .map_err(|e| format!("science report unavailable: {e}"))
    }
}

/// The `/science <query>` command body: synthesize (cached, 1h TTL) and return a
/// briefing. No query → usage. Mirrors `crate::drive::habits::run`.
pub(crate) fn run(arg: Option<&str>) -> String {
    let Some(query) = arg.map(str::trim).filter(|q| !q.is_empty()) else {
        return "usage: /science <query>\n  fans the query across OpenAlex, Crossref, Semantic \
                Scholar and Europe PMC, then folds the hits into a ranked, deduplicated briefing \
                (cached 1h under ~/.angelX/science/)."
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
    syn.brief(12).unwrap_or_else(|e| e)
}

#[cfg(test)]
#[path = "../../../tests/cockpit/app/science__tests.rs"]
mod tests;
