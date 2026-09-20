//! The `science_search` agent tool — the synthesis bench (`crate::science`),
//! callable mid-turn. Fans one query across the free scientific indices and
//! hands back a ranked, deduplicated briefing. All sources are key-less and a
//! down index degrades rather than errors, so the tool is always safe to offer.

use crate::club::ToolDef;
use crate::harness::{Tool, ToolRegistry, env_flag};
use serde_json::Value;

pub(crate) struct ScienceTool;

impl Tool for ScienceTool {
    fn name(&self) -> &str {
        "science_search"
    }
    fn def(&self) -> ToolDef {
        ToolDef {
            name: "science_search".to_string(),
            description: "Search the scientific literature: fans one query across OpenAlex, \
                          Crossref, Semantic Scholar and Europe PMC, deduplicates by DOI and \
                          ranks by citations + recency. Returns a briefing — the dominant \
                          title-metadata cues, the seminal and freshest papers, then a ranked \
                          reading list with validated DOI or source-record links. It does not \
                          read abstracts or full text. Use for papers, prior work, citation counts, or a \
                          literature review in ML, biology, physics or chemistry."
                .to_string(),
            params: serde_json::json!({
                "type": "object",
                "properties": {
                    "query": {
                        "type": "string",
                        "minLength": 1,
                        "maxLength": crate::science::MAX_QUERY_CHARS,
                        "description": "the research question or topic"
                    },
                    "limit": {
                        "type": "integer",
                        "description": "papers per source (default 8, max 25)"
                    },
                },
                "required": ["query"],
            }),
        }
    }
    fn call(&self, args: &Value) -> Result<String, String> {
        let query = args["query"].as_str().ok_or("missing 'query'")?;
        if query.trim().is_empty() {
            return Err("'query' must not be empty".to_string());
        }
        if query.chars().count() > crate::science::MAX_QUERY_CHARS {
            return Err(format!(
                "'query' must be at most {} characters",
                crate::science::MAX_QUERY_CHARS
            ));
        }
        let limit = args["limit"].as_u64().unwrap_or(8).clamp(1, 25) as usize;
        let syn = crate::science::synthesize_cached(query, limit, 3600);
        if syn.notes.iter().any(|note| note.contains("stalled {")) {
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
                crate::science::sanitize_metadata(query, crate::science::MAX_QUERY_CHARS)
            ));
        }
        Ok(syn.brief(limit.min(15)))
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
mod tests {
    use super::*;

    #[test]
    fn empty_query_is_rejected_not_fanned() {
        let t = ScienceTool;
        assert!(t.call(&serde_json::json!({ "query": "  " })).is_err());
        assert!(t.call(&serde_json::json!({})).is_err());
    }

    #[test]
    fn oversized_query_is_rejected_before_network_dispatch() {
        let t = ScienceTool;
        let query = "x".repeat(crate::science::MAX_QUERY_CHARS + 1);
        let err = t.call(&serde_json::json!({ "query": query })).unwrap_err();
        assert!(err.contains("at most 500 characters"), "{err}");
    }

    #[test]
    fn def_advertises_the_name_and_query_param() {
        let d = ScienceTool.def();
        assert_eq!(d.name, "science_search");
        assert_eq!(ScienceTool.name(), d.name);
        assert!(d.params["properties"]["query"].is_object());
        assert_eq!(d.params["properties"]["query"]["maxLength"], 500);
    }
}
