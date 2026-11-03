//! The `science_search` agent tool — the synthesis bench (`crate::drive::science`),
//! callable mid-turn. Fans one query across the free scientific indices and
//! hands back a ranked, deduplicated briefing. All sources are key-less and a
//! down index degrades rather than errors, so the tool is always safe to offer.

use crate::agent::club::ToolDef;
use crate::agent::harness::{Tool, ToolRegistry, env_flag};
use serde_json::Value;

pub(crate) struct ScienceTool;

impl Tool for ScienceTool {
    fn name(&self) -> &str {
        "science_search"
    }
    fn def(&self) -> ToolDef {
        let mut definition = crate::agent::harness::core_catalog::baseline_def("science_search");
        *definition.params.pointer_mut("/properties/query/maxLength").expect("embedded tool schema") = serde_json::json!(crate::drive::science::MAX_QUERY_CHARS);
        definition
    }
    fn call(&self, args: &Value) -> Result<String, String> {
        let query = args["query"].as_str().ok_or("missing 'query'")?;
        if query.trim().is_empty() {
            return Err("'query' must not be empty".to_string());
        }
        if query.chars().count() > crate::drive::science::MAX_QUERY_CHARS {
            return Err(format!(
                "'query' must be at most {} characters",
                crate::drive::science::MAX_QUERY_CHARS
            ));
        }
        let limit = args["limit"].as_u64().unwrap_or(8).clamp(1, 25) as usize;
        let syn = crate::drive::science::synthesize_cached(query, limit, 3600);
        if syn.notes.iter().any(|note| {
            note.contains("stalled {")
                || note.contains("worker request interrupted")
                || note.starts_with("science analysis:")
        }) {
            return Err(syn.notes.join("; "));
        }
        if syn.papers.is_empty() {
            let why = if syn.notes.is_empty() {
                "no results".to_string()
            } else {
                syn.notes.join("; ")
            };
            return Ok(format!(
                "science_search \"{}\": {why}",
                crate::drive::science::sanitize_metadata(
                    query,
                    crate::drive::science::MAX_QUERY_CHARS
                )
            ));
        }
        syn.brief(limit.min(15))
    }
}

/// Register `science_search`, gated on `ANGEL_SCIENCE_TOOL` (default on). The
/// sources are free + key-less, so it advertises everywhere without config.
pub(crate) fn maybe_register_science(r: &mut ToolRegistry) {
    if env_flag("ANGEL_SCIENCE_TOOL", true) {
        r.register(Box::new(ScienceTool));
    }
}

#[cfg(test)]
#[path = "../../../../tests/cockpit/tools/science__tests.rs"]
mod tests;
