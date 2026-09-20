//! The `repo_search` agent tool — the repo-surfacing bench (`crate::repos`),
//! callable mid-turn. Fans one query at GitHub repository search and hands back
//! a ranked, deduplicated, source-cited shortlist so a report can cite the
//! latest or most-established projects in a space. Key-less (an optional
//! `ANGEL_GITHUB_TOKEN` only raises the rate limit) and a rate-limited or down
//! API degrades to a note rather than an error, so the tool is safe to offer.

use crate::club::ToolDef;
use crate::harness::{Tool, ToolRegistry, env_flag};
use crate::repos::Mode;
use serde_json::Value;

pub(crate) struct RepoSearchTool;

impl Tool for RepoSearchTool {
    fn name(&self) -> &str {
        "repo_search"
    }
    fn def(&self) -> ToolDef {
        ToolDef {
            name: "repo_search".to_string(),
            description: "Surface the latest/best code repositories for a topic via GitHub \
                          repository search. Returns a cited shortlist — owner/name, stars, \
                          last-push date, primary language, description, and a validated \
                          github.com link — ranked by reputation (stars) or recency. Use when a \
                          report or answer should point at real, maintained projects: reference \
                          implementations, libraries, tools, or the freshest work in a fast-moving \
                          area. `mode:\"latest\"` favours recently-pushed repos; `mode:\"best\"` \
                          (default) favours established, high-star ones."
                .to_string(),
            params: serde_json::json!({
                "type": "object",
                "properties": {
                    "query": {
                        "type": "string",
                        "minLength": 1,
                        "maxLength": crate::repos::MAX_QUERY_CHARS,
                        "description": "topic or GitHub search query (supports GitHub qualifiers \
                                        like language:rust, stars:>1000)"
                    },
                    "mode": {
                        "type": "string",
                        "enum": ["best", "latest"],
                        "description": "best = most-starred (default); latest = most-recently-pushed"
                    },
                    "limit": {
                        "type": "integer",
                        "description": "repositories to return (default 10, max 25)"
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
        if query.chars().count() > crate::repos::MAX_QUERY_CHARS {
            return Err(format!(
                "'query' must be at most {} characters",
                crate::repos::MAX_QUERY_CHARS
            ));
        }
        let mode = args["mode"]
            .as_str()
            .and_then(Mode::parse)
            .unwrap_or(Mode::Best);
        let limit = args["limit"].as_u64().unwrap_or(10).clamp(1, 25) as usize;
        let set = crate::repos::surface_cached(query.trim(), mode, limit, 1800);
        if set.repos.is_empty() {
            let why = if set.notes.is_empty() {
                "no repositories found".to_string()
            } else {
                set.notes.join("; ")
            };
            return Ok(format!(
                "repo_search \"{}\": {why}",
                crate::science::sanitize_metadata(query, crate::repos::MAX_QUERY_CHARS)
            ));
        }
        Ok(set.brief(mode, limit.min(15)))
    }
}

/// Register `repo_search`, gated on `ANGEL_REPO_TOOL` (default on). GitHub
/// search is free + key-less, so it advertises everywhere without config.
pub(crate) fn maybe_register_repos(r: &mut ToolRegistry) {
    if env_flag("ANGEL_REPO_TOOL", true) {
        r.register(Box::new(RepoSearchTool));
    }
}

#[cfg(test)]
#[path = "../../../tests/cockpit/tools/repos__tests.rs"]
mod tests;
