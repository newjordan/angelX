//! Display-only local lesson with optional STEM/computing reference enrichment.
//!
//! The local lesson is immediate and useful without a network. When the
//! existing `ANGEL_QUICK_LOOKUP` policy permits it, a small worker may add a
//! bounded Wikipedia excerpt. Neither path enters conversation/private-reasoning
//! history or invokes a model.

use std::io::Read as _;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc;
use std::time::Duration;

const LOOKUP_RESPONSE_MAX_BYTES: usize = 256 * 1024;
const LOOKUP_SUMMARY_MAX_CHARS: usize = 520;
#[cfg_attr(not(test), allow(dead_code))]
const LOOKUP_TERM_MAX_CHARS: usize = crate::library::LOCAL_TOPIC_MAX_CHARS;
const ROLL_STEP: usize = 12;
static LOOKUP_IN_FLIGHT: AtomicBool = AtomicBool::new(false);

struct LookupLease {
    in_flight: &'static AtomicBool,
}

impl Drop for LookupLease {
    fn drop(&mut self) {
        self.in_flight.store(false, Ordering::Release);
    }
}

fn acquire_lookup(in_flight: &'static AtomicBool) -> Option<LookupLease> {
    in_flight
        .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
        .ok()
        .map(|_| LookupLease { in_flight })
}

#[cfg(test)]
pub(crate) enum TestLookupOutcome {
    Success {
        title: &'static str,
        summary: &'static str,
        source_url: &'static str,
    },
    Empty,
    Error(&'static str),
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct Definition {
    title: String,
    summary: String,
    source_url: String,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum ReferenceStatus {
    Offline,
    Loading,
    Ready,
    NotFound,
    Unavailable,
}

/// A local-first lesson shown by the world-owned Scryglass. Wikipedia is an
/// optional enrichment, never the object that makes the lesson useful.
pub(crate) struct QuickLookup {
    term: String,
    local: crate::library::LocalLesson,
    text: String,
    shown: usize,
    reference_status: ReferenceStatus,
    receiver: Option<mpsc::Receiver<Result<Option<Definition>, String>>>,
}

impl QuickLookup {
    pub(crate) fn spawn(term: String) -> Self {
        let local = lesson_for_lookup(&term);
        Self::spawn_local(local)
    }

    pub(crate) fn for_shelf(shelf_id: &str) -> Option<Self> {
        crate::library::local_lesson_for_shelf(shelf_id).map(Self::spawn_local)
    }

    fn spawn_local(local: crate::library::LocalLesson) -> Self {
        if !crate::harness::env_flag("ANGEL_QUICK_LOOKUP", true) {
            return Self::from_local(local);
        }
        Self::spawn_local_with_admission(local, &LOOKUP_IN_FLIGHT)
    }

    fn spawn_local_with_admission(
        local: crate::library::LocalLesson,
        in_flight: &'static AtomicBool,
    ) -> Self {
        let Some(lease) = acquire_lookup(in_flight) else {
            return Self::from_local(local);
        };
        let (sender, receiver) = mpsc::channel();
        let worker_term = local.topic.clone();
        let worker = std::thread::Builder::new()
            .name("angel-quick-lookup".to_string())
            .spawn(move || {
                let result = match crate::term::catch_background_unwind(|| {
                    lookup_definition(&worker_term)
                }) {
                    Ok(result) => result,
                    Err(_) => Err("lookup worker stopped unexpectedly".to_string()),
                };
                drop(lease);
                let _ = sender.send(result);
            });
        let receiver = match worker {
            Ok(_) => receiver,
            Err(error) => {
                let text = format!(
                    "Quick lookup unavailable · optional Wikipedia enrichment · {}\nLocal lesson remains available offline.\n\n{}",
                    truncate_chars(&collapse_whitespace(&error.to_string()), 160),
                    local.render_offline()
                );
                let shown = text.len();
                return Self {
                    term: local.topic.clone(),
                    local,
                    text,
                    shown,
                    reference_status: ReferenceStatus::Unavailable,
                    receiver: None,
                };
            }
        };
        Self::loading(local, receiver)
    }

    /// Construct an explicitly offline lesson. This is also the stable fallback
    /// when external reference enrichment is disabled.
    #[cfg(test)]
    pub(crate) fn offline(term: String) -> Self {
        Self::from_local(lesson_for_lookup(&term))
    }

    fn from_local(local: crate::library::LocalLesson) -> Self {
        let text = local.render_offline();
        let shown = text.len();
        Self {
            term: local.topic.clone(),
            local,
            text,
            shown,
            reference_status: ReferenceStatus::Offline,
            receiver: None,
        }
    }

    fn loading(
        local: crate::library::LocalLesson,
        receiver: mpsc::Receiver<Result<Option<Definition>, String>>,
    ) -> Self {
        let text = format!(
            "Checking “{}” for an optional STEM or computing reference…\n\n{}",
            local.topic,
            local.render_offline()
        );
        let shown = text.len();
        Self {
            term: local.topic.clone(),
            local,
            text,
            shown,
            reference_status: ReferenceStatus::Loading,
            receiver: Some(receiver),
        }
    }

    pub(crate) fn term(&self) -> &str {
        &self.term
    }

    pub(crate) fn visible_text(&self) -> &str {
        &self.text[..self.shown.min(self.text.len())]
    }

    pub(crate) fn is_loading(&self) -> bool {
        self.receiver.is_some()
    }

    pub(crate) fn roll_pending(&self) -> bool {
        self.shown < self.text.len()
    }

    /// The resident tutor delivering this lesson (for status surfaces).
    pub(crate) fn tutor_name(&self) -> &str {
        self.local.tutor.name
    }

    /// The lesson's Recall prompts (for the `/recall` review surface).
    pub(crate) fn recall_prompt(&self) -> String {
        self.local.recall_prompt()
    }

    /// Editable handoff for the ordinary composer. This returns text only; it
    /// cannot submit a turn or invoke a provider.
    pub(crate) fn ask_prompt(&self) -> String {
        self.local.ask_tutor_prompt()
    }

    /// The inspectable curriculum pointer is always available, including while
    /// Wikipedia is disabled, loading, or unavailable.
    pub(crate) fn source_url(&self) -> &str {
        self.local.source_url
    }

    pub(crate) const fn reference_status(&self) -> ReferenceStatus {
        self.reference_status
    }

    pub(crate) fn poll_and_roll(&mut self, animate: bool) {
        let outcome = self.receiver.as_ref().and_then(|receiver| {
            use mpsc::TryRecvError;
            match receiver.try_recv() {
                Ok(result) => Some(result),
                Err(TryRecvError::Empty) => None,
                Err(TryRecvError::Disconnected) => {
                    Some(Err("lookup worker disconnected".to_string()))
                }
            }
        });
        if let Some(outcome) = outcome {
            self.receiver = None;
            let mut immediate_fallback = false;
            self.text = match outcome {
                Ok(Some(definition)) => {
                    let context =
                        format!("{} {} {}", self.term, definition.title, definition.summary);
                    if let Some(local) =
                        crate::library::local_lesson_with_context(&self.term, &context)
                    {
                        self.local = local;
                    }
                    self.reference_status = ReferenceStatus::Ready;
                    format!(
                        "{}\n\n{}\n\nSource · Wikipedia · {}\n\n{}",
                        definition.title,
                        definition.summary,
                        definition.source_url,
                        self.local.render_offline()
                    )
                }
                Ok(None) => {
                    self.reference_status = ReferenceStatus::NotFound;
                    immediate_fallback = true;
                    format!(
                        "No concise STEM or computing entry was found on Wikipedia for “{}”.\nLocal lesson remains available offline.\n\n{}",
                        self.term,
                        self.local.render_offline()
                    )
                }
                Err(error) => {
                    self.reference_status = ReferenceStatus::Unavailable;
                    immediate_fallback = true;
                    format!(
                        "Quick lookup unavailable · optional Wikipedia enrichment · {}\nLocal lesson remains available offline.\n\n{}",
                        truncate_chars(&collapse_whitespace(&error), 160),
                        self.local.render_offline()
                    )
                }
            };
            self.shown = if animate && immediate_fallback {
                self.text.len()
            } else {
                0
            };
        }
        if animate {
            self.roll();
        } else {
            self.shown = self.text.len();
        }
    }

    fn roll(&mut self) {
        let len = self.text.len();
        if self.shown >= len {
            self.shown = len;
            return;
        }
        let mut target = (self.shown + ROLL_STEP).min(len);
        while target < len && !self.text.is_char_boundary(target) {
            target += 1;
        }
        self.shown = target;
    }

    #[cfg(test)]
    pub(crate) fn ready(term: &str, title: &str, summary: &str) -> Self {
        let context = format!("{term} {title} {summary}");
        let local = crate::library::local_lesson_with_context(term, &context)
            .unwrap_or_else(|| lesson_for_lookup(term));
        let text = format!(
            "{title}\n\n{summary}\n\nSource · Wikipedia · https://en.wikipedia.org/\n\n{}",
            local.render_offline()
        );
        let shown = text.len();
        Self {
            term: local.topic.clone(),
            local,
            text,
            shown,
            reference_status: ReferenceStatus::Ready,
            receiver: None,
        }
    }

    /// Like [`ready`], but with the roll-in still pending (shown = 0).
    #[cfg(test)]
    pub(crate) fn ready_unrolled(term: &str, title: &str, summary: &str) -> Self {
        let mut lookup = Self::ready(term, title, summary);
        lookup.shown = 0;
        lookup
    }

    #[cfg(test)]
    pub(crate) fn shown_chars(&self) -> usize {
        self.shown
    }

    #[cfg(test)]
    pub(crate) fn queued(term: &str, outcome: TestLookupOutcome) -> Self {
        let (sender, receiver) = mpsc::channel();
        let outcome = match outcome {
            TestLookupOutcome::Success {
                title,
                summary,
                source_url,
            } => Ok(Some(Definition {
                title: title.to_string(),
                summary: summary.to_string(),
                source_url: source_url.to_string(),
            })),
            TestLookupOutcome::Empty => Ok(None),
            TestLookupOutcome::Error(error) => Err(error.to_string()),
        };
        sender.send(outcome).expect("test lookup receiver is live");
        Self::loading(lesson_for_lookup(term), receiver)
    }
}

fn lesson_for_lookup(term: &str) -> crate::library::LocalLesson {
    crate::library::local_lesson(term)
        .or_else(|| crate::library::local_lesson("concept"))
        .expect("the built-in fallback topic always produces a local lesson")
}

/// Turn rendered transcript selection into a local lesson topic. Surrounding
/// prose punctuation is harmless and ordinary multiword subjects are welcome;
/// control-bearing, overlong, and punctuation-only selections stay ordinary
/// copy operations.
#[cfg_attr(not(test), allow(dead_code))]
pub(crate) fn selected_term(text: &str) -> Option<String> {
    let mut term = text.trim().trim_matches(is_edge_punctuation);
    for (open, close) in [('(', ')'), ('[', ']'), ('{', '}')] {
        if term.starts_with(open) && term.ends_with(close) {
            term = term[open.len_utf8()..term.len() - close.len_utf8()].trim();
            break;
        }
    }
    if term.is_empty()
        || term.chars().count() > LOOKUP_TERM_MAX_CHARS
        || term.chars().any(char::is_control)
    {
        return None;
    }
    crate::library::normalize_topic(term)
}

#[cfg_attr(not(test), allow(dead_code))]
fn is_edge_punctuation(character: char) -> bool {
    matches!(
        character,
        '.' | ',' | ';' | ':' | '!' | '?' | '"' | '\'' | '’' | '“' | '”'
    )
}

fn lookup_definition(term: &str) -> Result<Option<Definition>, String> {
    let query = url::form_urlencoded::Serializer::new(String::new())
        .append_pair("action", "query")
        .append_pair("generator", "search")
        .append_pair("gsrsearch", term)
        .append_pair("gsrnamespace", "0")
        .append_pair("gsrlimit", "8")
        .append_pair("prop", "extracts|info")
        .append_pair("exintro", "1")
        .append_pair("explaintext", "1")
        .append_pair("exchars", "1200")
        .append_pair("inprop", "url")
        .append_pair("redirects", "1")
        .append_pair("format", "json")
        .append_pair("formatversion", "2")
        .finish();
    let url = format!("https://en.wikipedia.org/w/api.php?{query}");
    let response = http_agent()
        .get(&url)
        .set("Accept", "application/json")
        .call()
        .map_err(|error| error.to_string())?;
    let mut body = Vec::new();
    response
        .into_reader()
        .take((LOOKUP_RESPONSE_MAX_BYTES + 1) as u64)
        .read_to_end(&mut body)
        .map_err(|error| error.to_string())?;
    if body.len() > LOOKUP_RESPONSE_MAX_BYTES {
        return Err("Wikipedia response exceeded the safety limit".to_string());
    }
    parse_definition(term, &body)
}

fn http_agent() -> ureq::Agent {
    let mut builder = ureq::AgentBuilder::new()
        .timeout_connect(Duration::from_secs(3))
        .timeout_read(Duration::from_secs(6))
        .timeout(Duration::from_secs(6))
        .user_agent("angel0-quick-lookup/0.1 (+https://github.com/newjordan/angel0)");
    if !crate::yolo::enabled() {
        let allow_private = crate::harness::env_flag("ANGEL_HTTP_ALLOW_PRIVATE_NETWORK", false);
        builder = builder.resolver(move |netloc: &str| {
            crate::tools::web::resolve_http_target(netloc, allow_private)
        });
    }
    builder.build()
}

fn parse_definition(term: &str, body: &[u8]) -> Result<Option<Definition>, String> {
    let parsed: serde_json::Value = serde_json::from_slice(body)
        .map_err(|error| format!("invalid Wikipedia response: {error}"))?;
    if parsed.get("error").is_some() {
        return Err("Wikipedia rejected the lookup".to_string());
    }
    let Some(pages) = parsed
        .get("query")
        .and_then(|query| query.get("pages"))
        .and_then(serde_json::Value::as_array)
    else {
        return Ok(None);
    };

    let needle = term.to_lowercase();
    let mut candidates = pages
        .iter()
        .filter_map(|page| {
            let title = page.get("title")?.as_str()?.trim();
            let extract = page.get("extract")?.as_str()?.trim();
            if title.is_empty() || extract.is_empty() {
                return None;
            }
            let score = stem_score(&needle, title, extract);
            (score >= 2).then(|| {
                (
                    score,
                    page.get("index")
                        .and_then(serde_json::Value::as_i64)
                        .unwrap_or(i64::MAX),
                    Definition {
                        title: title.to_string(),
                        summary: concise_summary(extract),
                        source_url: source_url(page),
                    },
                )
            })
        })
        .collect::<Vec<_>>();
    candidates.sort_by(|left, right| right.0.cmp(&left.0).then(left.1.cmp(&right.1)));
    Ok(candidates
        .into_iter()
        .next()
        .map(|(_, _, definition)| definition))
}

fn source_url(page: &serde_json::Value) -> String {
    if let Some(page_id) = page.get("pageid").and_then(serde_json::Value::as_u64)
        && page_id > 0
    {
        return format!("https://en.wikipedia.org/?curid={page_id}");
    }
    page.get("fullurl")
        .and_then(serde_json::Value::as_str)
        .filter(|url| {
            url.len() <= 512
                && (url.starts_with("https://en.wikipedia.org/wiki/")
                    || url.starts_with("https://en.wikipedia.org/w/"))
        })
        .unwrap_or("https://en.wikipedia.org/")
        .to_string()
}

fn stem_score(needle: &str, title: &str, extract: &str) -> i32 {
    let title = title.to_lowercase();
    let extract = extract.to_lowercase();
    if extract.contains("may refer to:") || extract.contains("may refer to either") {
        return -100;
    }
    // MediaWiki search is fuzzy. A scientifically relevant near-name (for
    // example “Bevy” matching scientist Isabel Bevier) is still unrelated to
    // the selected word and must not become a confident teaching lesson.
    if !title.contains(needle) && !extract.contains(needle) {
        return -100;
    }
    let mut score = 0;
    for marker in [
        "mathematic",
        "probability",
        "statistic",
        "theorem",
        "equation",
        "distribution",
        "geometry",
        "calculus",
        "algebra",
        "number theory",
        "computer science",
        "computer graphics",
        "computer program",
        "programming",
        "software engineering",
        "graphics pipeline",
        "graphics shader",
        "rendering",
        "ray tracing",
        "graphics processing unit",
        "gpu",
        "cuda",
        "parallel computing",
        "compute kernel",
        "webgpu",
        "algorithm",
        "data structure",
        "video game",
        "game engine",
        "entity component system",
        "machine learning",
        "deep learning",
        "neural network",
        "language model",
        "transformer",
        "tokenization",
        "matrix",
        "vector",
        "linear algebra",
        "physics",
        "physicist",
        "particle",
        "quantum",
        "thermodynamic",
        "chemical",
        "chemistry",
        "molecule",
        "atomic",
        "biology",
        "biological",
        "species",
        "genus",
        "protein",
        "genetic",
        "cell division",
        "cellular",
        "membrane",
        "medicine",
        "medical",
        "disease",
        "syndrome",
        "astronom",
        "geolog",
        "ecolog",
        "neuroscience",
        "engineering",
        "scientific",
        "unit of measurement",
    ] {
        if extract.contains(marker) {
            score += 2;
        }
    }
    // Title proximity ranks candidates only after the extract itself establishes
    // STEM relevance; otherwise an exact music, company, or placename result
    // would pass merely because MediaWiki matched the selected word.
    if score == 0 {
        return -100;
    }
    if title == needle {
        score += 2;
    } else if title.starts_with(&format!("{needle} ")) || title.contains(&format!(" {needle} ")) {
        score += 3;
    }
    for marker in [
        " distribution",
        " equation",
        " theorem",
        " process",
        " formula",
        " function",
        " space",
        " algorithm",
        " law",
        " constant",
        " number",
    ] {
        if title.contains(marker) {
            score += 8;
        }
    }
    score
}

fn concise_summary(extract: &str) -> String {
    let normalized = collapse_whitespace(extract);
    let mut sentences = 0;
    let mut sentence_cut = None;
    for (index, character) in normalized.char_indices() {
        if matches!(character, '.' | '!' | '?')
            && normalized[index + character.len_utf8()..]
                .chars()
                .next()
                .is_none_or(char::is_whitespace)
        {
            sentences += 1;
            if sentences == 2 {
                sentence_cut = Some(index + character.len_utf8());
                break;
            }
        }
    }
    let summary = sentence_cut
        .map(|cut| normalized[..cut].trim())
        .unwrap_or(normalized.as_str());
    truncate_chars(summary, LOOKUP_SUMMARY_MAX_CHARS)
}

fn collapse_whitespace(text: &str) -> String {
    text.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn truncate_chars(text: &str, max_chars: usize) -> String {
    if text.chars().count() <= max_chars {
        return text.to_string();
    }
    let mut clipped = text
        .chars()
        .take(max_chars.saturating_sub(1))
        .collect::<String>();
    while clipped.ends_with(char::is_whitespace) {
        clipped.pop();
    }
    clipped.push('…');
    clipped
}

#[cfg(test)]
#[path = "../../../tests/cockpit/app/term_lookup__tests.rs"]
mod tests;
