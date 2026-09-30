//! Private, request-local Responses continuation. Never serialized or rendered.
use crate::agent::club::{ChatMsg, ToolCall};
use serde_json::Value;
use std::sync::Arc;

/// Hash the serializable message stream without allocating JSON or retaining
/// old payloads. Chained boundaries let each replay validate its full prefix in
/// O(1); the request computes the boundaries once in O(history bytes).
fn extend_digest(prior: &[u8; 32], message: &ChatMsg) -> [u8; 32] {
    struct DigestWriter(ring::digest::Context);
    impl std::io::Write for DigestWriter {
        fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
            self.0.update(bytes);
            Ok(bytes.len())
        }
        fn flush(&mut self) -> std::io::Result<()> {
            Ok(())
        }
    }
    let mut writer = DigestWriter(ring::digest::Context::new(&ring::digest::SHA256));
    writer.0.update(prior);
    serde_json::to_writer(&mut writer, message).expect("message JSON into infallible digest");
    writer.0.finish().as_ref().try_into().unwrap()
}

pub(super) fn prefix_digests(messages: &[ChatMsg]) -> Vec<[u8; 32]> {
    let mut digests = Vec::with_capacity(messages.len() + 1);
    digests.push([0; 32]);
    for message in messages {
        digests.push(extend_digest(digests.last().unwrap(), message));
    }
    digests
}

pub(crate) struct ResponseReplay {
    route: String,
    through_assistant: [u8; 32],
    items: Vec<Value>,
    extra_bytes: usize,
}

impl ResponseReplay {
    pub(super) fn new(
        route: &str,
        prefix: &[ChatMsg],
        calls: &[ToolCall],
        content: &str,
        items: Vec<Value>,
    ) -> Option<Arc<Self>> {
        // A partial/unsupported native item set must not replace valid generic
        // history. Require every returned function and all visible prose.
        if items.iter().any(|item| {
            item["type"] == "reasoning"
                && !item["encrypted_content"]
                    .as_str()
                    .is_some_and(|value| !value.is_empty())
        }) {
            return None;
        }
        let native_calls: Vec<_> = items
            .iter()
            .filter(|item| item["type"] == "function_call")
            .filter_map(|item| match super::events::parse_tool_done(Some(item)) {
                Some(super::ResponseEvent::ToolCallDone { call, .. }) => Some(call),
                _ => None,
            })
            .collect();
        let native_text = items
            .iter()
            .filter(|item| item["type"] == "message")
            .filter_map(|item| item["content"].as_array())
            .flatten()
            .filter(|part| part["type"] == "output_text")
            .filter_map(|part| part["text"].as_str())
            .collect::<String>();
        if calls.is_empty() || !same_calls(&native_calls, calls) || native_text != content {
            return None;
        }
        let extra_bytes = items
            .iter()
            .filter(|item| item["type"] == "reasoning")
            .map(|item| item.to_string().len())
            .sum();
        let assistant =
            ChatMsg::assistant_calls_full(native_calls, None, Some(content.to_string()));
        let through_assistant = extend_digest(prefix_digests(prefix).last().unwrap(), &assistant);
        Some(Arc::new(Self {
            route: route.into(),
            through_assistant,
            items,
            extra_bytes,
        }))
    }

    pub(super) fn items_for<'a>(
        &'a self,
        route: &str,
        through_assistant: &[u8; 32],
    ) -> Option<&'a [Value]> {
        (self.route == route && self.through_assistant == *through_assistant)
            .then_some(self.items.as_slice())
    }

    /// Conservative byte budget only; encrypted size is not an API token count.
    pub(crate) fn extra_bytes(&self) -> usize {
        self.extra_bytes
    }
}

fn same_calls(left: &[ToolCall], right: &[ToolCall]) -> bool {
    left.len() == right.len()
        && left.iter().zip(right).all(|(left, right)| {
            left.id == right.id && left.name == right.name && left.args == right.args
        })
}

/// Keep only completed items, indexed in provider order. Terminal output wins
/// over duplicate item-done frames. Never capture summaries from delta events.
#[derive(Default)]
pub(super) struct ReplayItems {
    items: std::collections::BTreeMap<u64, Value>,
    complete: Option<Vec<Value>>,
}

impl ReplayItems {
    pub(super) fn observe(&mut self, line: &str) {
        let payload = line
            .trim()
            .strip_prefix("data:")
            .map(str::trim)
            .unwrap_or(line.trim());
        // Most SSE frames are token deltas. Avoid a third JSON parse for those.
        if !payload.contains("response.output_item.done") && !payload.contains("response.completed")
        {
            return;
        }
        let Ok(value) = serde_json::from_str::<Value>(payload) else {
            return;
        };
        match value["type"].as_str() {
            Some("response.output_item.done") => {
                if let (Some(index), Some(item)) =
                    (value["output_index"].as_u64(), value.get("item"))
                {
                    self.items.insert(index, item.clone());
                }
            }
            Some("response.completed") => {
                self.complete = value
                    .pointer("/response/output")
                    .and_then(Value::as_array)
                    .cloned();
            }
            _ => {}
        }
    }

    pub(super) fn finish(self) -> Vec<Value> {
        let items = self
            .complete
            .unwrap_or_else(|| self.items.into_values().collect());
        if items.iter().any(|item| {
            item.get("status")
                .and_then(Value::as_str)
                .is_some_and(|status| status != "completed")
                || !matches!(
                    item["type"].as_str(),
                    Some("reasoning" | "message" | "function_call")
                )
        }) {
            return Vec::new();
        }
        items
    }
}
