//! Responses-API stream event decoding (module-breakup: event decoding,
//! extracted from `openai_codex.rs`).
//!
//! One SSE line → one [`ResponseEvent`], plus the partial tool-call assembler
//! that folds start/delta/done frames across output-item and call-id keys.
//! Pure over `serde_json::Value` / strings — directly unit-tested; the
//! streaming path in the parent consumes these through re-exports.

use crate::agent::club::ToolCall;
use std::collections::BTreeMap;

use super::{Usage, parse_usage};

pub(crate) enum ResponseEvent {
    Text(String),
    Reasoning(String),
    ReasoningSummary(String),
    ToolCallStart {
        key: String,
        call_id: Option<String>,
        name: Option<String>,
    },
    ToolArgumentsDelta { key: String, delta: String },
    ToolArgumentsDone { key: String, arguments: String },
    ToolCallDone { key: String, call: ToolCall },
    Done(Option<Usage>),
    Failed(String, Option<Usage>),
    Incomplete(String, Option<Usage>),
    Usage(Usage),
    Ignore,
}

pub(super) struct ParsedResponsesLine<'a> {
    pub(super) line: &'a str,
    pub(super) payload: &'a str,
    pub(super) value: Option<serde_json::Value>,
    done: bool,
}

impl<'a> ParsedResponsesLine<'a> {
    pub(super) fn new(line: &'a str) -> Self {
        let trimmed = line.trim();
        let payload = trimmed
            .strip_prefix("data:")
            .map(str::trim)
            .unwrap_or(trimmed);
        let ignored =
            trimmed.is_empty() || trimmed.starts_with(':') || trimmed.starts_with("event:");
        let done = !ignored && payload == "[DONE]";
        let value = if ignored || done {
            None
        } else {
            serde_json::from_str(payload).ok()
        };
        Self {
            line,
            payload,
            value,
            done,
        }
    }

    pub(super) fn event(&self) -> ResponseEvent {
        if self.done {
            return ResponseEvent::Done(None);
        }
        self.value
            .as_ref()
            .map(parse_responses_value)
            .unwrap_or(ResponseEvent::Ignore)
    }

    pub(super) fn wire_payload(&self) -> Option<&str> {
        let data = self.line.strip_prefix("data:")?.trim();
        (!data.is_empty() && data != "[DONE]").then_some(data)
    }
}

#[cfg(test)]
pub(crate) fn parse_responses_event(line: &str) -> ResponseEvent {
    ParsedResponsesLine::new(line).event()
}

fn parse_responses_value(v: &serde_json::Value) -> ResponseEvent {
    let kind = v.get("type").and_then(|t| t.as_str()).unwrap_or("");
    match kind {
        "response.output_text.delta" => v
            .get("delta")
            .and_then(|d| d.as_str())
            .map(|s| ResponseEvent::Text(s.to_string()))
            .unwrap_or(ResponseEvent::Ignore),
        "response.reasoning_text.delta" => v
            .get("delta")
            .and_then(|d| d.as_str())
            .map(|s| ResponseEvent::Reasoning(s.to_string()))
            .unwrap_or(ResponseEvent::Ignore),
        "response.reasoning_summary_text.delta" => v
            .get("delta")
            .and_then(|d| d.as_str())
            .filter(|delta| !delta.is_empty())
            .map(|s| ResponseEvent::ReasoningSummary(s.to_string()))
            .unwrap_or(ResponseEvent::Ignore),
        "response.reasoning_summary_part.done" => {
            let text = v
                .get("part")
                .filter(|part| {
                    part.get("type").and_then(|kind| kind.as_str()) == Some("summary_text")
                })
                .and_then(|part| part.get("text"))
                .and_then(|text| text.as_str())
                .filter(|text| !text.trim().is_empty());
            match text {
                Some(text) if text.ends_with("\n\n") => ResponseEvent::Ignore,
                Some(text) if text.ends_with('\n') => ResponseEvent::ReasoningSummary("\n".into()),
                Some(_) => ResponseEvent::ReasoningSummary("\n\n".into()),
                None => ResponseEvent::Ignore,
            }
        }
        "response.output_item.added" => parse_tool_start(v.get("item")),
        "response.output_item.done" => {
            parse_tool_done(v.get("item")).unwrap_or_else(|| parse_tool_start(v.get("item")))
        }
        "response.function_call_arguments.delta" => {
            let key = response_item_key(v);
            let delta = v
                .get("delta")
                .and_then(|d| d.as_str())
                .unwrap_or_default()
                .to_string();
            if key.is_empty() || delta.is_empty() {
                ResponseEvent::Ignore
            } else {
                ResponseEvent::ToolArgumentsDelta { key, delta }
            }
        }
        "response.function_call_arguments.done" => {
            let key = response_item_key(v);
            let arguments = v
                .get("arguments")
                .and_then(|d| d.as_str())
                .unwrap_or_default()
                .to_string();
            if key.is_empty() {
                ResponseEvent::Ignore
            } else {
                ResponseEvent::ToolArgumentsDone { key, arguments }
            }
        }
        "response.completed" => match incomplete_response_message(v) {
            Some(msg) => ResponseEvent::Incomplete(msg, parse_usage(v.pointer("/response/usage"))),
            None => ResponseEvent::Done(parse_usage(v.pointer("/response/usage"))),
        },
        "response.incomplete" => ResponseEvent::Incomplete(
            incomplete_response_message(v).unwrap_or_else(|| {
                crate::agent::club::OutputBudgetPolicy::EndpointManaged.incomplete_message()
            }),
            parse_usage(v.pointer("/response/usage")),
        ),
        "response.failed" | "error" => {
            let msg = v
                .pointer("/response/error/message")
                .or_else(|| v.pointer("/error/message"))
                .or_else(|| v.get("message"))
                .and_then(|m| m.as_str())
                .unwrap_or("request failed")
                .to_string();
            ResponseEvent::Failed(msg, parse_usage(v.pointer("/response/usage")))
        }
        _ => parse_usage(v.pointer("/response/usage"))
            .map(ResponseEvent::Usage)
            .unwrap_or(ResponseEvent::Ignore),
    }
}

pub(crate) fn incomplete_response_message(v: &serde_json::Value) -> Option<String> {
    let response = v.get("response").unwrap_or(v);
    let status = response.get("status").and_then(|s| s.as_str());
    if status != Some("incomplete") {
        return None;
    }
    let reason = response
        .pointer("/incomplete_details/reason")
        .and_then(|r| r.as_str())
        .unwrap_or("unknown");
    Some(if reason == "max_output_tokens" {
        crate::agent::club::OutputBudgetPolicy::EndpointManaged.incomplete_message()
    } else {
        format!("response incomplete: endpoint reported {reason}")
    })
}

pub(crate) fn parse_tool_start(item: Option<&serde_json::Value>) -> ResponseEvent {
    let Some(item) =
        item.filter(|item| item.get("type").and_then(|t| t.as_str()) == Some("function_call"))
    else {
        return ResponseEvent::Ignore;
    };
    let key = tool_item_key(item);
    if key.is_empty() {
        return ResponseEvent::Ignore;
    }
    ResponseEvent::ToolCallStart {
        key,
        call_id: item
            .get("call_id")
            .and_then(|v| v.as_str())
            .map(str::to_string),
        name: item
            .get("name")
            .and_then(|v| v.as_str())
            .map(str::to_string),
    }
}

pub(crate) fn parse_tool_done(item: Option<&serde_json::Value>) -> Option<ResponseEvent> {
    let item = item?;
    if item.get("type").and_then(|t| t.as_str()) != Some("function_call") {
        return None;
    }
    let key = tool_item_key(item);
    let name = item.get("name").and_then(|v| v.as_str())?;
    let id = item
        .get("call_id")
        .or_else(|| item.get("id"))
        .and_then(|v| v.as_str())
        .unwrap_or("tool_call")
        .to_string();
    let args = item
        .get("arguments")
        .and_then(|v| v.as_str())
        .map(parse_tool_args)
        .unwrap_or_else(|| serde_json::json!({}));
    Some(ResponseEvent::ToolCallDone {
        key: if key.is_empty() { id.clone() } else { key },
        call: ToolCall {
            id,
            name: name.to_string(),
            args,
        },
    })
}

pub(crate) fn response_item_key(v: &serde_json::Value) -> String {
    v.get("item_id")
        .or_else(|| v.get("call_id"))
        .and_then(|x| x.as_str())
        .map(str::to_string)
        .or_else(|| {
            v.get("output_index")
                .and_then(|x| x.as_u64())
                .map(|idx| format!("output:{idx}"))
        })
        .unwrap_or_default()
}

pub(crate) fn tool_item_key(item: &serde_json::Value) -> String {
    item.get("id")
        .or_else(|| item.get("call_id"))
        .and_then(|x| x.as_str())
        .map(str::to_string)
        .or_else(|| {
            item.get("output_index")
                .and_then(|x| x.as_u64())
                .map(|idx| format!("output:{idx}"))
        })
        .unwrap_or_default()
}

pub(crate) fn parse_tool_args(raw: &str) -> serde_json::Value {
    if raw.trim().is_empty() {
        return serde_json::json!({});
    }
    serde_json::from_str(raw).unwrap_or_else(|_| serde_json::json!({ "_raw": raw }))
}

#[derive(Default)]
pub(crate) struct PartialResponseToolCall {
    call_id: Option<String>,
    name: Option<String>,
    arguments: String,
    done: Option<ToolCall>,
}

#[derive(Default)]
pub(crate) struct ResponseToolCalls {
    order: Vec<String>,
    calls: BTreeMap<String, PartialResponseToolCall>,
    notes: Vec<(&'static str, String)>,
}

impl ResponseToolCalls {
    pub(crate) fn entry(&mut self, key: &str) -> &mut PartialResponseToolCall {
        if !self.calls.contains_key(key) {
            self.order.push(key.to_string());
        }
        self.calls.entry(key.to_string()).or_default()
    }

    pub(crate) fn existing_key_for(&self, candidate: &str) -> Option<String> {
        if self.calls.contains_key(candidate) {
            return Some(candidate.to_string());
        }
        self.calls.iter().find_map(|(key, partial)| {
            (partial.call_id.as_deref() == Some(candidate)).then(|| key.clone())
        })
    }

    pub(crate) fn entry_for_alias(&mut self, candidate: &str) -> &mut PartialResponseToolCall {
        let key = self
            .existing_key_for(candidate)
            .unwrap_or_else(|| candidate.to_string());
        self.entry(&key)
    }

    pub(crate) fn start(&mut self, key: String, call_id: Option<String>, name: Option<String>) {
        let canonical = self
            .existing_key_for(&key)
            .or_else(|| {
                call_id
                    .as_deref()
                    .and_then(|call_id| self.existing_key_for(call_id))
            })
            .unwrap_or(key);
        let entry = self.entry(&canonical);
        if call_id.is_some() {
            entry.call_id = call_id;
        }
        if name.is_some() {
            entry.name = name;
        }
    }

    pub(crate) fn push_args(&mut self, key: &str, delta: &str) {
        self.entry_for_alias(key).arguments.push_str(delta);
    }

    pub(crate) fn set_args(&mut self, key: &str, arguments: String) {
        let entry = self.entry_for_alias(key);
        let replaced = (!entry.arguments.is_empty() && entry.arguments != arguments)
            .then(|| (entry.name.clone(), entry.arguments.len()));
        entry.arguments = arguments;
        if let Some((name, streamed)) = replaced {
            let replacement = self.entry_for_alias(key).arguments.len();
            self.notes.push((
                "tool_args_replaced",
                format!(
                    "{} ({key}): final arguments ({replacement} B) differ from the streamed deltas ({streamed} B)",
                    name.as_deref().unwrap_or("unnamed")
                ),
            ));
        }
    }

    pub(crate) fn done(&mut self, key: String, call: ToolCall) {
        let canonical = self
            .existing_key_for(&key)
            .or_else(|| self.existing_key_for(&call.id))
            .unwrap_or(key);
        let entry = self.entry(&canonical);
        let renamed = entry
            .name
            .as_deref()
            .filter(|started| *started != call.name)
            .map(str::to_string);
        let reshaped =
            !entry.arguments.is_empty() && parse_tool_args(&entry.arguments) != call.args;
        entry.done = Some(call);
        let name = entry
            .done
            .as_ref()
            .map(|c| c.name.clone())
            .unwrap_or_default();
        if let Some(started) = renamed {
            self.notes.push((
                "tool_name_changed",
                format!("{canonical}: started as {started}, completed as {name}"),
            ));
        }
        if reshaped {
            self.notes.push((
                "tool_args_changed",
                format!("{name} ({canonical}): completed arguments differ from the streamed ones"),
            ));
        }
    }

    pub(crate) fn pending(&self) -> bool {
        !self.calls.is_empty()
    }

    pub(crate) fn into_calls_with_notes(self) -> (Vec<ToolCall>, Vec<(&'static str, String)>) {
        let mut out = Vec::new();
        let mut notes = self.notes;
        for key in self.order {
            let Some(partial) = self.calls.get(&key) else {
                continue;
            };
            if let Some(call) = &partial.done {
                out.push(call.clone());
                continue;
            }
            let Some(name) = partial.name.clone() else {
                notes.push((
                    "tool_entry_dropped",
                    format!(
                        "{key}: a tool entry with no name was dropped ({} B of arguments: {})",
                        partial.arguments.len(),
                        partial.arguments.chars().take(120).collect::<String>()
                    ),
                ));
                continue;
            };
            out.push(ToolCall {
                id: partial.call_id.clone().unwrap_or(key),
                name,
                args: parse_tool_args(&partial.arguments),
            });
        }
        (out, notes)
    }
}

#[cfg(test)]
#[path = "../../../../tests/cockpit/app/openai_codex__events__tests.rs"]
mod tests;
