//! Checked negative attempts remain research data. They close a perspective,
//! not an unanswered question, and never grant proof or reward authority.

use serde_json::{Value, json};
use std::path::{Component, Path};

const MAX_ATTEMPTS: usize = 128;

fn text(value: &Value, max: usize) -> bool {
    value
        .as_str()
        .is_some_and(|s| !s.trim().is_empty() && s.len() <= max && !s.contains('\0'))
}

fn digest(value: &Value) -> bool {
    let path = value["path"].as_str().unwrap_or_default();
    !path.is_empty()
        && path.len() <= 1024
        && !path.contains('\\')
        && !path.chars().any(char::is_control)
        && path.split('/').all(|part| !matches!(part, "" | "." | ".."))
        && Path::new(path)
            .components()
            .all(|part| matches!(part, Component::Normal(_)))
        && value["sha256"].as_str().is_some_and(|hash| {
            hash.len() == 64 && hash.bytes().all(|byte| byte.is_ascii_hexdigit())
        })
}

fn check(value: &Value) -> bool {
    if digest(value) {
        return true;
    }
    // Earlier structured check descriptions identify the retained own code
    // and output separately instead of pointing at the complete receipt.
    text(&value["id"], 240)
        && [
            ("code_path", "code_sha256"),
            ("output_path", "output_sha256"),
        ]
        .into_iter()
        .all(|(path, hash)| digest(&json!({"path":value[path],"sha256":value[hash]})))
        && ["source_sha256", "command_sha256"].into_iter().all(|key| {
            value[key].as_str().is_some_and(|hash| {
                hash.len() == 64 && hash.bytes().all(|byte| byte.is_ascii_hexdigit())
            })
        })
}

fn valid(attempt: &Value) -> bool {
    attempt.is_object()
        && matches!(attempt["verdict"].as_str(), Some("GAP" | "FALSE"))
        && text(&attempt["route"], 16_000)
        && text(&attempt["claim_id"], 240)
        && (text(&attempt["lesson"], 16_000) || text(&attempt["reason"], 16_000))
        && digest(&attempt["author_report"])
        && digest(&attempt["referee_report"])
        && (check(&attempt["check"]) || check(&attempt["referee_check"]))
        && attempt
            .get("counterexample")
            .is_none_or(|value| value.is_null() || digest(value))
        && ["perspective", "next_test"].into_iter().all(|key| {
            attempt
                .get(key)
                .is_none_or(|value| value.is_null() || text(value, 16_000))
        })
}

pub(super) fn validate(node: &Value, id: &str) -> Result<(), String> {
    if let Some(attempts) = node.get("failed_routes") {
        if attempts.as_array().is_none_or(|items| {
            items.len() > MAX_ATTEMPTS || items.iter().any(|attempt| !valid(attempt))
        }) {
            return Err(format!(
                "{id}: failed routes need bounded negative attempts and retained report/check identities"
            ));
        }
    }
    Ok(())
}

pub(crate) fn has_negative_attempt(node: &Value) -> bool {
    node["failed_routes"]
        .as_array()
        .is_some_and(|attempts| attempts.len() <= MAX_ATTEMPTS && attempts.iter().any(valid))
}

fn shortened(value: &Value, max: usize) -> Value {
    value
        .as_str()
        .map(|s| json!(s.chars().take(max).collect::<String>()))
        .unwrap_or(Value::Null)
}

pub(super) fn projection(node: &Value, limit: usize, compact: bool) -> Vec<Value> {
    let max = if compact { 160 } else { 800 };
    let path_max = if compact { 160 } else { 1024 };
    node["failed_routes"].as_array().into_iter().flatten().rev()
        .filter(|attempt| valid(attempt)).take(limit.min(MAX_ATTEMPTS))
        .map(|attempt| {
            let lesson = if text(&attempt["lesson"], 16_000) { &attempt["lesson"] } else { &attempt["reason"] };
            let mut view = json!({"claim_id":attempt["claim_id"],
                "verdict":attempt["verdict"],"route":shortened(&attempt["route"],max),
                "lesson":shortened(lesson,max)});
            for key in ["id", "campaign", "integration", "perspective", "next_test"] {
                if let Some(value) = attempt.get(key).filter(|value| !value.is_null()) {
                    view[key] = shortened(value, max.min(240));
                }
            }
            if let Some(round) = attempt["round"].as_u64() {
                view["round"] = json!(round);
            }
            for key in ["author_report", "referee_report", "counterexample"] {
                if let Some(value) = attempt.get(key).filter(|value| digest(value)) {
                    view[key] = json!({"path":shortened(&value["path"],path_max),"sha256":value["sha256"]});
                }
            }
            let receipt = if digest(&attempt["check"]) { &attempt["check"] } else { &attempt["referee_check"] };
            if digest(receipt) {
                view["check"] = json!({"path":shortened(&receipt["path"],path_max),"sha256":receipt["sha256"]});
            }
            let receipt = if check(&attempt["referee_check"]) && !digest(&attempt["referee_check"]) {
                &attempt["referee_check"]
            } else { &attempt["check"] };
            if check(receipt) && !digest(receipt) {
                view["referee_check"] = json!({"id":shortened(&receipt["id"],160),
                    "code_path":shortened(&receipt["code_path"],path_max),"code_sha256":receipt["code_sha256"],
                    "output_path":shortened(&receipt["output_path"],path_max),"output_sha256":receipt["output_sha256"],
                    "source_sha256":receipt["source_sha256"],"command_sha256":receipt["command_sha256"]});
            }
            view
        }).collect()
}

#[cfg(test)]
#[path = "../../../../tests/cockpit/labyrinth/failed_routes__tests.rs"]
mod tests;
