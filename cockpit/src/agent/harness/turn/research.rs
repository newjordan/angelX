//! Answer-only contract selected by the shared skill router, never by a
//! separate task-keyword detector. The bundled playbook declares no verifier.
use super::*;

/// The whole-question decline: `⠟⠛⠉`'s page, which the answer carries itself
/// (its reader, an evaluator, cannot read the ledger).
pub(crate) const DISCLOSURE: &str = super::book::q_stop::DISCLOSURE;

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
    crate::agent::tools::web::configured_research_origin().or_else(|| {
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
                crate::agent::tools::web::research_origin(raw)
            })
    })
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

/// A research turn's origin rides after the tool's route as data, behind the
/// page (in that tool's own section) whose `{origin}` it fills.
pub(super) fn describe_surface(defs: &mut [ToolDef], origin: Option<&str>) {
    if let Some(origin) = origin {
        for def in defs {
            let page = match def.name.as_str() {
                "web_search" => "⠱⠁⠙",
                "web_fetch" => "⠱⠃⠙",
                _ => continue,
            };
            if !def.description.contains(origin) {
                def.description.push_str(&format!(" {page} {origin}"));
            }
        }
    }
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
                .and_then(crate::agent::tools::web::research_origin)
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
                && !crate::agent::club::contains_raw_tool_markup(&m.content)
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
            Some(crate::knowledge::cut::sha256_hex(url.as_str().as_bytes()))
        })
        .collect()
}

#[cfg(test)]
#[path = "../../../../../tests/cockpit/harness/turn__research__tests.rs"]
mod tests;
