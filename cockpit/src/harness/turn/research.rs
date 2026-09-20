//! Answer-only contract selected by the shared skill router, never by a
//! separate task-keyword detector. The bundled playbook declares no verifier.
use super::*;

pub(super) const GUIDANCE: &str = "Research completion: deliver the answer with supporting citations. If evidence is missing or insufficient, say so explicitly and cite nothing (NO citations, background links, document identifiers, or sources appendix). Do not invent a verifier run. Answer as soon as the requested claims are supported; batch independent source reads when possible. Source tracking is automatic and requires no extra tool calls or searches. Missing-evidence replies are rendered as a citation-free disclosure.";

pub(crate) const COMPOSE: &str = "Research compose step: answer now from the fetched evidence with citations, or decline citing nothing. If the evidence is insufficient, say 'Evidence is missing' and cite nothing (NO citations, background links, or sources appendix). Your next reply is the candidate; do not search, fetch, or call tools.";

pub(crate) const DISCLOSURE: &str =
    "No evidence in the corpus supports an answer to this question.";

/// Enforce the cite-nothing contract when the candidate explicitly declines the
/// whole question. This is disclosure rendering, not a semantic support scorer.
/// Only an opening, unqualified declaration is recognized: a later caveat or a
/// quoted missing-evidence phrase must not erase a supported answer.
pub(crate) fn finalize(answer: &str) -> String {
    let opening = answer.trim_start_matches(|c: char| c.is_whitespace() || c == '*');
    let lower = opening.to_lowercase();
    let declaration = [
        "no evidence exists to answer this question",
        "the corpus contains no evidence answering this question",
        "no evidence in the corpus supports an answer to this question",
    ]
    .iter()
    .any(|prefix| lower.starts_with(prefix));
    // Require a sentence/clause boundary. "Evidence is missing for the second
    // part, but ..." is a partial answer, not a whole-question decline.
    let missing = ["evidence is missing", "evidence is insufficient"]
        .iter()
        .any(|prefix| {
            lower.strip_prefix(prefix).is_some_and(|rest| {
                rest.is_empty()
                    || rest.starts_with(['.', ';', '!', '\n', '*'])
                    || rest.starts_with(" —")
                    || rest.starts_with(" / not found")
            })
        });
    if declaration || missing {
        DISCLOSURE.into()
    } else {
        answer.into()
    }
}

/// Task declarations are explicit labelled URLs, not arbitrary URLs in source text.
pub(super) fn origin(history: &[ChatMsg]) -> Option<String> {
    crate::tools::web::configured_research_origin().or_else(|| {
        history
            .iter()
            .rev()
            .find(|m| m.role == ChatRole::User)?
            .content
            .lines()
            .find_map(|line| {
                let raw = line
                    .strip_prefix("Research origin: ")
                    .or_else(|| line.strip_prefix("Corpus origin: "))?;
                crate::tools::web::research_origin(raw)
            })
    })
}

pub(super) fn preamble(origin: Option<&str>) -> String {
    let surface = match origin {
        Some(origin) => format!("Search with web_search (origin {origin}); fetch documents with web_fetch; do not probe the host with shell."),
        None => "Search with web_search; fetch documents with web_fetch; do not probe the host with shell. No research origin was declared; do not invent one.".into(),
    };
    format!("{surface}\n{GUIDANCE}")
}

pub(super) fn ensure_tools(defs: &mut Vec<ToolDef>, registry: &ToolRegistry) {
    for def in registry.defs() {
        if matches!(def.name.as_str(), "web_search" | "web_fetch")
            && !defs.iter().any(|existing| existing.name == def.name)
        {
            defs.push(def);
        }
    }
}

pub(super) fn describe_surface(defs: &mut [ToolDef], origin: Option<&str>) {
    if let Some(origin) = origin {
        for def in defs {
            if matches!(def.name.as_str(), "web_search" | "web_fetch")
                && !def.description.contains(origin)
            {
                def.description.push_str(&format!(
                    " Research search origin: {origin}; fetch document URLs with web_fetch."
                ));
            }
        }
    }
}

/// Count repeated searches since the last document fetch in the current task.
/// Search URLs are supported for providers that use the generic HTTP tool.
pub(super) fn repeated_search(history: &[ChatMsg]) -> bool {
    let mut searches = std::collections::HashSet::new();
    for message in history
        .iter()
        .rev()
        .take_while(|m| m.role != ChatRole::User)
    {
        for call in message.tool_calls.iter().rev() {
            let query = match call.name.as_str() {
                "web_search" => call
                    .args
                    .get("query")
                    .and_then(Value::as_str)
                    .map(str::to_string),
                "http_request" | "web_fetch" => {
                    let Some(url) = call
                        .args
                        .get("url")
                        .and_then(Value::as_str)
                        .and_then(|raw| url::Url::parse(raw).ok())
                    else {
                        continue;
                    };
                    if url.path().trim_end_matches('/') != "/search" {
                        return false;
                    }
                    url.query_pairs()
                        .find(|(key, _)| key == "q")
                        .map(|(_, value)| value.into_owned())
                }
                _ => None,
            };
            if let Some(query) = query.filter(|q| !q.trim().is_empty())
                && !searches.insert(
                    query
                        .split_whitespace()
                        .collect::<Vec<_>>()
                        .join(" ")
                        .to_lowercase(),
                )
            {
                return true;
            }
        }
    }
    false
}

pub(crate) fn off_surface(name: &str, args: &Value) -> bool {
    if !matches!(name, "shell" | "proc_run") {
        return false;
    }
    let command = args
        .get("command")
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_lowercase();
    command.contains("/proc/net/")
        || command.contains("ss -")
        || command.contains("netstat")
        || command.contains("lsof -i")
        || (command.contains("ps ") && (command.contains("corpus") || command.contains("searx")))
        || ((command.contains("curl ") || command.contains("wget "))
            && (command.contains("127.0.0.1") || command.contains("localhost")))
}

pub(super) fn selected(history: &[ChatMsg]) -> bool {
    if std::env::var("ANGEL_TASK_ACCEPT_CMD").is_ok_and(|s| !s.trim().is_empty()) {
        return false;
    }
    let Some(task) = history.iter().rev().find(|m| m.role == ChatRole::User) else {
        return false;
    };
    let skills = skill_summaries(&embedded_skills());
    relevant_skill_name(&skills, &task.content) == Some("research-answer")
        || task.content.lines().any(|line| {
            line.strip_prefix("Corpus origin: ")
                .or_else(|| line.strip_prefix("Research origin: "))
                .and_then(crate::tools::web::research_origin)
                .is_some()
        })
}

/// Preserve the latest usable draft in this user turn across tool-call messages.
/// Never revive a prior user turn or expose harness metadata as an answer.
pub(super) fn draft(history: &[ChatMsg]) -> Option<String> {
    history
        .iter()
        .rev()
        .take_while(|m| m.role != ChatRole::User)
        .find(|m| {
            m.role == ChatRole::Assistant
                && !m.content.trim().is_empty()
                && !m
                    .content
                    .trim_start()
                    .strip_prefix('{')
                    .is_some_and(|body| body.trim_start().starts_with("\"authority_profile\""))
                && !crate::club::contains_raw_tool_markup(&m.content)
        })
        .map(|m| finalize(&m.content))
}

/// Track source identities, not query wording or snippets. Search output puts
/// each URL on its own line; fetch supplies its source through the arguments.
pub(crate) fn sources(name: &str, args: &Value, result: &str) -> Vec<String> {
    let urls: Vec<&str> = match name {
        "web_search" => result.lines().map(str::trim).collect(),
        "web_fetch" if !result.trim().is_empty() => args
            .get("url")
            .and_then(Value::as_str)
            .into_iter()
            .collect(),
        _ => return Vec::new(),
    };
    urls.into_iter()
        .filter_map(|raw| {
            let mut url = url::Url::parse(raw).ok()?;
            if !matches!(url.scheme(), "http" | "https") {
                return None;
            }
            url.set_fragment(None);
            Some(crate::cut::sha256_hex(url.as_str().as_bytes()))
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn research_compose_disclosure_drops_background_and_keeps_supported_answers() {
        for answer in [
            "**No evidence exists to answer this question, so I cite nothing.**\n\nThe docs (pax-token, pax-upgrade) contain no such value.",
            "The corpus contains no evidence answering this question. Background: http://127.0.0.1:12345/doc/kestrel-m2",
            "Evidence is missing. [Background](https://example.test/doc/1)",
            "Evidence is insufficient; the answer is 123 [1].",
        ] {
            assert_eq!(finalize(answer), DISCLOSURE);
            assert_eq!(
                draft(&[ChatMsg::user("research"), ChatMsg::assistant(answer)]).as_deref(),
                Some(DISCLOSURE)
            );
        }
        for answer in [
            "The GlazeR R-series fires at **1185 °C**. Source: http://127.0.0.1:12345/doc/glaze-r",
            "30 requests per minute (http://127.0.0.1:12345/doc/sundial-api)",
            "42 [source](https://example.test/doc/1). Evidence is missing for a second question.",
            "Evidence is missing for the second part; the first is 42 [1].",
            "The passage says ‘Evidence is missing.’ [1]",
        ] {
            assert_eq!(finalize(answer), answer);
        }
    }

    #[test]
    fn research_origin_envelope_schema_and_preamble() {
        let _guard = crate::tests::env_lock();
        let _env =
            crate::tests::TestEnvGuard::set("ANGEL_SEARXNG_URL", "http://127.0.0.1:34251/search");
        let history = [ChatMsg::user("Research question: use the corpus.")];
        let configured = origin(&history);
        assert_eq!(configured.as_deref(), Some("http://127.0.0.1:34251/search"));
        let mut defs = vec![
            crate::tools::web::WebSearchTool.def(),
            crate::tools::web::WebFetchTool.def(),
        ];
        describe_surface(&mut defs, configured.as_deref());
        let envelope = serde_json::json!({"preamble":preamble(configured.as_deref()), "tools":defs.iter().map(|d| serde_json::json!({"name":d.name,"description":d.description,"parameters":d.params})).collect::<Vec<_>>()});
        assert!(
            envelope["preamble"]
                .as_str()
                .unwrap()
                .contains(configured.as_deref().unwrap())
        );
        for def in envelope["tools"].as_array().unwrap() {
            assert!(
                def["description"]
                    .as_str()
                    .unwrap()
                    .contains(configured.as_deref().unwrap())
            );
        }
        assert!(preamble(configured.as_deref()).contains("do not probe the host with shell"));
        drop(_env);
        let _env = crate::tests::TestEnvGuard::set("ANGEL_SEARXNG_URL", "");
        assert_eq!(origin(&history), None);
        assert!(!preamble(None).contains("http"));
        assert!(
            !crate::tools::web::WebSearchTool
                .def()
                .description
                .contains("127.0.0.1")
        );
        let declared = [ChatMsg::user(
            "Research question: use sources.\nCorpus origin: http://127.0.0.1:12345",
        )];
        assert_eq!(origin(&declared).as_deref(), Some("http://127.0.0.1:12345"));
        assert!(crate::tools::web::research_origin("https://user:secret@example.test").is_none());
        assert_eq!(
            crate::tools::web::research_search_endpoint("http://127.0.0.1:12345"),
            "http://127.0.0.1:12345/search"
        );
        assert_eq!(
            crate::tools::web::research_search_endpoint("https://example.test/custom/search"),
            "https://example.test/custom/search"
        );
    }

    #[test]
    fn research_repeated_search_resets_on_fetch_and_user_turn() {
        let search = || {
            ChatMsg::assistant_calls(vec![ToolCall {
                id: "s".into(),
                name: "web_search".into(),
                args: serde_json::json!({"query":"clock"}),
            }])
        };
        let mut history = vec![ChatMsg::user("research-answer"), search(), search()];
        assert!(repeated_search(&history));
        history.push(ChatMsg::assistant_calls(vec![ToolCall {
            id: "f".into(),
            name: "web_fetch".into(),
            args: serde_json::json!({"url":"https://example.test/doc/1"}),
        }]));
        history.push(search());
        assert!(!repeated_search(&history));
        history.push(search());
        assert!(repeated_search(&history));
        history.push(ChatMsg::user("new task"));
        assert!(!repeated_search(&history));
    }

    #[test]
    fn research_off_surface_discovery_is_not_document_work() {
        for command in [
            "cat /proc/net/tcp",
            "ss -tlnp",
            "ps aux | grep corpus",
            "curl http://127.0.0.1:8888/search",
        ] {
            assert!(off_surface(
                "shell",
                &serde_json::json!({"command":command})
            ));
        }
        assert!(!off_surface(
            "shell",
            &serde_json::json!({"command":"python3 analyze.py"})
        ));
        assert!(!off_surface(
            "web_fetch",
            &serde_json::json!({"url":"http://127.0.0.1:12345/doc/1"})
        ));
        assert!(!crate::club::final_response_requested(&[ChatMsg::user(
            COMPOSE
        )]));
        assert!(crate::club::final_response_requested(&[ChatMsg::harness(
            COMPOSE
        )]));
    }

    #[test]
    fn research_task_mode_uses_skill_selection_and_acceptance_contract() {
        let _guard = crate::tests::env_lock();
        let old = std::env::var_os("ANGEL_TASK_ACCEPT_CMD");
        unsafe {
            std::env::remove_var("ANGEL_TASK_ACCEPT_CMD");
        }
        let research = vec![ChatMsg::user(
            "Research question q04: answer using sources and citations.",
        )];
        assert!(selected(&research));
        assert!(selected(&[ChatMsg::user(
            "Use research-answer for this question."
        )]));
        assert!(!selected(&[ChatMsg::user(
            "Fix the research dashboard and run the tests."
        )]));
        assert!(!selected(&[ChatMsg::user(
            "Remove an unused renderer and preserve shared navigation. \
             Add regression tests and run local checks. \
             Use the existing source; do not perform network research."
        )]));
        assert!(!selected(&[ChatMsg::user(
            "Fix the research answer API and add regression tests."
        )]));

        assert!(!selected(&[ChatMsg::user(
            "Read the fixture URL using web_fetch and report what it returns."
        )]));
        unsafe {
            std::env::set_var("ANGEL_TASK_ACCEPT_CMD", "cargo test");
        }
        assert!(!selected(&research));
        unsafe {
            match old {
                Some(v) => std::env::set_var("ANGEL_TASK_ACCEPT_CMD", v),
                None => std::env::remove_var("ANGEL_TASK_ACCEPT_CMD"),
            }
        }
    }

    #[test]
    fn research_stopped_draft_is_current_verbatim_and_not_raw_markup() {
        let answer = "42 [source](https://example.test/a)";
        let mut history = vec![ChatMsg::user("research-answer"), ChatMsg::assistant(answer)];
        assert_eq!(draft(&history).as_deref(), Some(answer));
        history.push(ChatMsg::assistant(""));
        history.push(ChatMsg::assistant(
            r#"{"authority_profile": {"profile": "full"}}"#,
        ));
        assert_eq!(draft(&history).as_deref(), Some(answer));
        history.push(ChatMsg::user("new request"));
        assert_eq!(draft(&history), None);
        history.push(ChatMsg::assistant(""));
        assert_eq!(draft(&history), None);
    }

    #[test]
    fn research_sources_use_urls_not_queries_snippets_or_fragments() {
        let a = sources(
            "web_search",
            &serde_json::json!({"query":"a"}),
            "1. A\n https://example.test/a#first\n snippet",
        );
        let b = sources(
            "web_search",
            &serde_json::json!({"query":"b"}),
            "1. Different\n https://example.test/a#second\n other snippet",
        );
        assert_eq!(a, b);
        assert_eq!(a.len(), 1);
        assert!(sources("web_search", &Value::Null, "no results").is_empty());
        assert!(
            sources(
                "web_fetch",
                &serde_json::json!({"url":"https://example.test/a"}),
                ""
            )
            .is_empty()
        );
        assert_eq!(
            sources(
                "web_fetch",
                &serde_json::json!({"url":"https://example.test/a"}),
                "body"
            ),
            a
        );
    }
}
