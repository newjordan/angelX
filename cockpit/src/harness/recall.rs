//! Memory recall and BM25 tool search.

use super::coeffect::{Change, Classification, CoeffectStore, Requirement};
use super::*;
use std::collections::{HashMap, HashSet};

/// `recall` — pull relevant notes from long-form memory (the palace) on demand.
/// The agentic counterpart to auto-compaction's deposit: rather than auto-
/// injecting (which would bloat history and feed back into the next compaction),
/// the model asks for what it needs, when it needs it — the same pull pattern as
/// `tool_search`. Scoped to the current project's wing by default.
pub struct RecallTool {
    store: Arc<dyn crate::memory::store::MemoryStore>,
    wing: String,
}

impl RecallTool {
    pub fn new(
        store: Arc<dyn crate::memory::store::MemoryStore>,
        workspace: &std::path::Path,
    ) -> Self {
        Self {
            store,
            wing: crate::compaction::project_wing_for(workspace),
        }
    }
}

impl Tool for RecallTool {
    fn name(&self) -> &str {
        "recall"
    }
    fn def(&self) -> ToolDef {
        ToolDef {
            name: "recall".to_string(),
            description: "Search long-term memory (notes distilled from this and earlier \
                          sessions: decisions, files changed, durable facts, open threads) and \
                          return the most relevant. Use it when prior context would help and \
                          isn't in the current conversation. Args: `query` (string, required), \
                          `limit` (int, optional, default 5)."
                .to_string(),
            params: serde_json::json!({
                "type": "object",
                "properties": {
                    "query": { "type": "string", "description": "What to recall." },
                    "limit": { "type": "integer", "description": "Max notes (default 5)." }
                },
                "required": ["query"]
            }),
        }
    }
    fn call(&self, args: &Value) -> Result<String, String> {
        let query = args
            .get("query")
            .and_then(|q| q.as_str())
            .unwrap_or("")
            .trim();
        if query.is_empty() {
            return Err("recall needs a non-empty `query`".to_string());
        }
        let limit = args
            .get("limit")
            .and_then(|l| l.as_u64())
            .map(|l| l as usize)
            .unwrap_or(5)
            .clamp(1, 20);
        match self.store.search(query, limit, Some(&self.wing)) {
            Ok(blocks) if !blocks.is_empty() => Ok(blocks.join("\n\n")),
            Ok(_) => Ok("(no relevant notes in long-term memory)".to_string()),
            Err(e) => Err(format!("recall failed: {e}")),
        }
    }
}

// ---------------------------------------------------------------------------
// tool_search — BM25 discovery over deferred tools (the Codex deferral pattern),
// so a large tool set (e.g. many MCP tools) doesn't bloat every request.
// ---------------------------------------------------------------------------

/// Lowercase alphanumeric tokens.
pub(crate) fn tokenize(s: &str) -> Vec<String> {
    s.split(|c: char| !c.is_alphanumeric())
        .filter(|t| !t.is_empty())
        .map(|t| t.to_lowercase())
        .collect()
}

/// Rank `docs` (id, text) against `query` by BM25 (k1=1.5, b=0.75); return the
/// ids of the top `limit`, best first. Only positive-scoring docs are returned.
pub(crate) fn bm25_rank(query: &str, docs: &[(usize, String)], limit: usize) -> Vec<usize> {
    let q = tokenize(query);
    if q.is_empty() || docs.is_empty() {
        return Vec::new();
    }
    let toks: Vec<(usize, Vec<String>)> = docs.iter().map(|(id, t)| (*id, tokenize(t))).collect();
    let n = toks.len() as f64;
    let avgdl = (toks.iter().map(|(_, t)| t.len()).sum::<usize>() as f64 / n).max(1.0);
    let (k1, b) = (1.5_f64, 0.75_f64);
    let query_terms = q.iter().map(String::as_str).collect::<HashSet<_>>();
    let mut document_frequencies = HashMap::<&str, usize>::with_capacity(query_terms.len());
    let mut term_frequencies = Vec::with_capacity(toks.len());
    for (_, doc) in &toks {
        record_bm25_index_token_visits(doc.len());
        let mut frequencies = HashMap::<&str, usize>::with_capacity(query_terms.len());
        for word in doc {
            if query_terms.contains(word.as_str()) {
                *frequencies.entry(word.as_str()).or_default() += 1;
            }
        }
        for term in frequencies.keys() {
            *document_frequencies.entry(term).or_default() += 1;
        }
        term_frequencies.push(frequencies);
    }

    let mut scored: Vec<(usize, f64)> = Vec::new();
    for ((id, doc), frequencies) in toks.iter().zip(&term_frequencies) {
        let dl = doc.len() as f64;
        let mut score = 0.0;
        for term in &q {
            let tf = frequencies.get(term.as_str()).copied().unwrap_or(0) as f64;
            if tf == 0.0 {
                continue;
            }
            let df = document_frequencies
                .get(term.as_str())
                .copied()
                .unwrap_or(0) as f64;
            let idf = (((n - df + 0.5) / (df + 0.5)) + 1.0).ln();
            score += idf * (tf * (k1 + 1.0)) / (tf + k1 * (1.0 - b + b * dl / avgdl));
        }
        if score > 0.0 {
            scored.push((*id, score));
        }
    }
    scored.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
    scored.into_iter().take(limit).map(|(id, _)| id).collect()
}

#[cfg(test)]
std::thread_local! {
    static BM25_INDEX_TOKEN_VISITS: std::cell::Cell<Option<usize>> = const {
        std::cell::Cell::new(None)
    };
}

#[cfg(test)]
fn record_bm25_index_token_visits(visits: usize) {
    BM25_INDEX_TOKEN_VISITS.with(|probe| {
        if let Some(total) = probe.get() {
            probe.set(Some(total.saturating_add(visits)));
        }
    });
}

#[cfg(not(test))]
fn record_bm25_index_token_visits(_visits: usize) {}

#[cfg(test)]
pub(crate) fn count_bm25_index_token_visits<T>(run: impl FnOnce() -> T) -> (T, usize) {
    BM25_INDEX_TOKEN_VISITS.with(|probe| {
        let prior = probe.replace(Some(0));
        let result = run();
        let visits = probe.replace(prior).unwrap_or(0);
        (result, visits)
    })
}

/// The searchable text for a tool: name (also de-underscored), description, and
/// each parameter's name + description.
pub(crate) fn tool_search_text(def: &ToolDef) -> String {
    let mut s = format!(
        "{} {} {}",
        def.name,
        def.name.replace('_', " "),
        def.description
    );
    if let Some(props) = def.params.get("properties").and_then(|p| p.as_object()) {
        for (k, v) in props {
            s.push(' ');
            s.push_str(k);
            if let Some(d) = v.get("description").and_then(|d| d.as_str()) {
                s.push(' ');
                s.push_str(d);
            }
        }
    }
    s
}

/// One-line, bounded tool summary for discovery and local routing prompts. The
/// provider receives the exact native schema on the following request, so
/// copying that schema into tool-result history only bloats every later hop.
pub(crate) fn compact_tool_summary(description: &str) -> String {
    const MAX_CHARS: usize = 220;
    let one_line = description.split_whitespace().collect::<Vec<_>>().join(" ");
    if one_line.chars().count() <= MAX_CHARS {
        return one_line;
    }
    let mut compact = one_line.chars().take(MAX_CHARS - 1).collect::<String>();
    compact.push('…');
    compact
}

/// Strictly parse a local router response. Models may wrap the JSON in a code
/// fence, but only allow-listed candidate names survive; invented/core tool
/// names never alter the provider schema set.
pub(crate) fn parse_tool_bubble_reply(
    reply: &str,
    allowed: &[String],
    max: usize,
) -> Option<Vec<String>> {
    if max == 0 {
        return Some(Vec::new());
    }
    let trimmed = reply.trim();
    let candidate = serde_json::from_str::<Value>(trimmed).ok().or_else(|| {
        let object = trimmed
            .find('{')
            .zip(trimmed.rfind('}'))
            .filter(|(start, end)| start <= end)
            .and_then(|(start, end)| serde_json::from_str(&trimmed[start..=end]).ok());
        object.or_else(|| {
            trimmed
                .find('[')
                .zip(trimmed.rfind(']'))
                .filter(|(start, end)| start <= end)
                .and_then(|(start, end)| serde_json::from_str(&trimmed[start..=end]).ok())
        })
    })?;
    let names = match &candidate {
        Value::Array(names) => names,
        Value::Object(object) => object.get("tools")?.as_array()?,
        _ => return None,
    };
    let allowed = allowed
        .iter()
        .map(String::as_str)
        .collect::<std::collections::HashSet<_>>();
    let mut seen = std::collections::HashSet::new();
    let mut selected = Vec::new();
    let mut named = 0usize;
    for name in names.iter().filter_map(Value::as_str) {
        named += 1;
        if allowed.contains(name) && seen.insert(name) {
            selected.push(name.to_string());
            if selected.len() == max {
                break;
            }
        }
    }
    (names.is_empty() || (named > 0 && !selected.is_empty())).then_some(selected)
}

/// Compact prompt for a cheap local model to rerank a deterministic shortlist.
/// Parameter schemas are intentionally absent: this classifier needs capability
/// semantics, not the wire contract later sent to the root model.
pub(crate) fn tool_bubble_router_prompt(task: &str, candidates: &[&ToolDef], max: usize) -> String {
    let task = task.chars().take(8_000).collect::<String>();
    let mut prompt = format!(
        "Select 0 to {max} tools that the coding agent is likely to need for the current task.\n\
         Return ONLY JSON in this exact shape: {{\"tools\":[\"tool_name\"]}}.\n\
         Use only names from the candidate list. Prefer fewer tools; the base read/edit/shell/search tools are already present.\n\n\
         TASK:\n{task}\n\nCANDIDATES:\n"
    );
    for definition in candidates {
        prompt.push_str("- ");
        prompt.push_str(&definition.name);
        prompt.push_str(": ");
        prompt.push_str(&compact_tool_summary(&definition.description));
        prompt.push('\n');
    }
    prompt
}

/// `tool_search(query)` — BM25-rank deferred tools and activate the best native
/// schemas for the next provider request. Results stay compact because the
/// structured definitions themselves arrive out-of-band on that request.
///
/// Activation is **derived**, not maintained (§3.2 Def 22). `activate(..)` records
/// what the model asked for; `snapshot()` is that intent intersected with what is
/// live, computed against the coeffect store at read time. A provider that goes away
/// reclassifies every dependent as deactivating, so its schema leaves the
/// advertisement without anyone remembering to retract it — and a neutral change (an
/// unrelated key, a retune under a live provider) leaves the generation untouched,
/// which is what lets the turn loop reuse the previous hop's definitions verbatim.
pub(crate) struct ToolActivations {
    /// Intent: the names `tool_search` surfaced for the next request.
    names: std::sync::Mutex<std::collections::BTreeSet<String>>,
    /// Declared specs `d ⊆ K` per tool name (§3.2 Def 21).
    requirements: std::sync::Mutex<std::collections::BTreeMap<String, Requirement>>,
    /// Σ — what is live right now.
    store: Arc<std::sync::Mutex<CoeffectStore>>,
    /// Bumped when the *advertised* set can have moved. Deliberately not bumped by
    /// neutral changes: stability here is the "no reload" property.
    generation: std::sync::atomic::AtomicU64,
}

impl Default for ToolActivations {
    fn default() -> Self {
        Self {
            names: std::sync::Mutex::new(std::collections::BTreeSet::new()),
            requirements: std::sync::Mutex::new(std::collections::BTreeMap::new()),
            store: Arc::new(std::sync::Mutex::new(CoeffectStore::default())),
            generation: std::sync::atomic::AtomicU64::new(0),
        }
    }
}

/// The declaration half: what each tool needs, and whether it holds right now.
impl ToolActivations {
    /// The coeffect store behind these activations — `Σ`.
    pub(crate) fn store(&self) -> Arc<std::sync::Mutex<CoeffectStore>> {
        Arc::clone(&self.store)
    }

    /// Record a tool's declared spec. The empty spec behaves exactly as every tool
    /// did before this existed: always satisfiable.
    pub(crate) fn declare(&self, name: &str, requirement: Requirement) {
        self.requirements
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .insert(name.to_string(), requirement);
    }

    /// Whether `name`'s declared spec holds right now. An undeclared name is
    /// admitted — no declaration, no precondition.
    pub(crate) fn admits(&self, name: &str) -> bool {
        let requirement = self
            .requirements
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .get(name)
            .cloned();
        match requirement {
            None => true,
            Some(requirement) => self
                .store
                .lock()
                .unwrap_or_else(|e| e.into_inner())
                .satisfied(&requirement),
        }
    }
}

/// The reaction half: intent, the derived advertisement, and the classification
/// log a caller (or a test) reads to see *why* something moved.
impl ToolActivations {
    /// Forget a tool entirely: its declaration, its intent, its advertisement.
    #[allow(dead_code)]
    pub(crate) fn withdraw(&self, name: &str) -> bool {
        self.requirements
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .remove(name);
        self.retract(name)
    }

    pub(crate) fn reset(&self) {
        self.names.lock().unwrap_or_else(|e| e.into_inner()).clear();
        self.bump();
    }

    /// Record intent. A name whose declared spec is unsatisfied is not admitted:
    /// a stale search index must not advertise a tool whose provider has left.
    pub(crate) fn activate<'a>(
        &self,
        names: impl IntoIterator<Item = &'a str>,
        max: usize,
    ) -> usize {
        if max == 0 {
            return 0;
        }
        let mut active = self.names.lock().unwrap_or_else(|e| e.into_inner());
        let before = active.len();
        for name in names {
            if active.len() >= max {
                break;
            }
            if self.admits(name) {
                active.insert(name.to_string());
            }
        }
        let added = active.len().saturating_sub(before);
        drop(active);
        if added > 0 {
            self.bump();
        }
        added
    }

    /// Retract one name from the active set. A provider that departs takes its
    /// schema with it: leaving the name behind would keep
    /// `activated_schema_failures` non-zero on the next request.
    #[allow(dead_code)]
    pub(crate) fn retract(&self, name: &str) -> bool {
        let removed = self
            .names
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .remove(name);
        if removed {
            self.bump();
        }
        removed
    }

    /// What may be advertised right now: intent ∩ `σ ⊧ d`, computed at read time.
    /// This is the whole point of the derivation — nobody has to remember to
    /// retract a name when its provider leaves, and a name whose spec holds again
    /// comes back on its own, without re-asking.
    pub(crate) fn snapshot(&self) -> std::collections::BTreeSet<String> {
        let intent = self.names.lock().unwrap_or_else(|e| e.into_inner()).clone();
        let requirements = self
            .requirements
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .clone();
        let store = self.store.lock().unwrap_or_else(|e| e.into_inner());
        intent
            .into_iter()
            .filter(|name| {
                requirements
                    .get(name)
                    .is_none_or(|requirement| store.satisfied(requirement))
            })
            .collect()
    }

    /// Def 22 reaction of every declared tool to one change, moving the generation
    /// only when something must actually be re-derived. Two neutral changes in a
    /// row therefore leave every downstream cache untouched — the "no reload" half
    /// of the classification.
    pub(crate) fn observe(&self, change: &Change) -> Vec<(String, Classification)> {
        let described: Vec<(String, Requirement)> = self
            .requirements
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .iter()
            .map(|(name, requirement)| (name.clone(), requirement.clone()))
            .collect();
        let mut reactions = Vec::with_capacity(described.len());
        let mut reloads = false;
        for (name, requirement) in described {
            let reaction = change.react(&requirement);
            reloads |= reaction.reloads();
            reactions.push((name, reaction.classification));
        }
        if reloads {
            self.bump();
        }
        reactions
    }

    pub(crate) fn generation(&self) -> u64 {
        self.generation.load(std::sync::atomic::Ordering::Relaxed)
    }

    fn bump(&self) {
        self.generation
            .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    }
}

pub(crate) struct ToolSearchTool {
    pub(crate) entries: Vec<ToolDef>,
    pub(crate) activations: Arc<ToolActivations>,
}

/// Discovery is open-ended unless the operator explicitly bounds it. Keep the
/// initial automatic shortlist small without making it a capability ceiling.
/// An explicit zero retains the text-only discovery control.
pub(crate) fn tool_search_activation_limit() -> usize {
    env_usize("ANGEL_TOOL_SEARCH_ACTIVE_MAX", usize::MAX)
}

impl ToolSearchTool {
    /// Entries whose declared spec holds right now. The catalogue is a snapshot,
    /// so availability is decided at read time — a provider that left after the
    /// catalogue was built cannot be surfaced by a search.
    fn available_len(&self) -> usize {
        self.entries
            .iter()
            .filter(|definition| self.activations.admits(&definition.name))
            .count()
    }
}

impl Tool for ToolSearchTool {
    fn name(&self) -> &str {
        "tool_search"
    }
    fn def(&self) -> ToolDef {
        ToolDef {
            name: "tool_search".to_string(),
            description: format!(
                "Search {} additional tools by capability (keywords). Returns matching tool \
                 names + summaries; then call the tool directly by name. Use this when you \
                 need a capability not in your base tool list.",
                self.available_len()
            ),
            params: serde_json::json!({
                "type": "object",
                "properties": {
                    "query": { "type": "string", "description": "capability keywords" },
                    "limit": { "type": "integer", "description": "max results (default 5)" }
                },
                "required": ["query"],
            }),
        }
    }
    fn call(&self, args: &Value) -> Result<String, String> {
        let query = args["query"].as_str().ok_or("missing 'query'")?;
        let limit = args
            .get("limit")
            .and_then(|l| l.as_u64())
            .unwrap_or(5)
            .clamp(1, 25) as usize;
        let docs: Vec<(usize, String)> = self
            .entries
            .iter()
            .enumerate()
            .filter(|(_, definition)| self.activations.admits(&definition.name))
            .map(|(i, d)| (i, tool_search_text(d)))
            .collect();
        let hits = bm25_rank(query, &docs, limit);
        if hits.is_empty() {
            return Ok(format!("no tools match '{query}'"));
        }
        let activation_max = tool_search_activation_limit();
        let activated = self.activations.activate(
            hits.iter().map(|index| self.entries[*index].name.as_str()),
            activation_max,
        );
        let active = self.activations.snapshot();
        let limit_note = if activation_max == usize::MAX {
            String::new()
        } else {
            format!(" (operator-set active cap {activation_max})")
        };
        let mut out = format!(
            "{} matching tool(s); {} new schema(s) activated for the next request{}. \
             Call a tool labelled active-next-request directly:\n",
            hits.len(),
            activated,
            limit_note,
        );
        if hits
            .iter()
            .any(|index| !active.contains(&self.entries[*index].name))
        {
            out.push_str(
                "The operator's ANGEL_TOOL_SEARCH_ACTIVE_MAX setting limits activation; \
                 repeating the search cannot raise it.\n",
            );
        }
        for i in hits {
            let d = &self.entries[i];
            let status = if active.contains(&d.name) {
                "active next request"
            } else {
                "not active: cap reached"
            };
            out.push_str(&format!(
                "\n- {} [{}]: {}\n",
                d.name,
                status,
                compact_tool_summary(&d.description)
            ));
        }
        Ok(out)
    }
}
