//! Actual chart-data mutations; one point call corresponds to one world sprite.
use crate::agent::{
    club::ToolDef,
    harness::{Tool, ToolEventId, ToolOutcome, TurnEvent},
};
use crate::knowledge::graph_crop::{GraphEvent, GraphReceipt, GraphRequest, GraphStore};
use serde_json::{Value, json};
use std::sync::{Mutex, mpsc::Sender};

#[derive(Default)]
pub(crate) struct GraphTool {
    store: Mutex<GraphStore>,
}
impl Tool for GraphTool {
    fn name(&self) -> &str {
        "graph"
    }
    fn def(&self) -> ToolDef {
        ToolDef {
            name: "graph".into(),
            description: "Create a real bar, line, or scatter chart in the world's graph garden. Begin a plot with explicit axes and expected_points (1-32) in spec; use the returned generation for point calls (generation, index and point: one actual data point per call), then finish with the generation. Farmers coordinate each call and a sprite delivers its successful value. Begin again on the same id or clear to turn the old graph back into dirt. Use measured or user-supplied data; do not invent observations. Data is in-memory for this workspace session. Discover graph with tool_search when needed.".into(),
            // Flat on purpose: a root `oneOf` without `properties` reaches some
            // providers (GLM on Z.ai) as a function with no parameters, and
            // the model's arguments arrive as `{}`. The op's own fields are
            // checked when the request is parsed. Every property names its
            // type: Moonshot's schema check rejects an enum without one.
            params: json!({"type":"object","properties":{
                "op":{"type":"string","enum":["begin","point","finish","clear"]},
                "plot":{"type":"string"},
                "spec":{"type":"object","properties":{
                    "title":{"type":"string"},"kind":{"type":"string","enum":["bar","line","scatter"]},"x_label":{"type":"string"},"y_label":{"type":"string"},"x_min":{"type":"number"},"x_max":{"type":"number"},"y_min":{"type":"number"},"y_max":{"type":"number"},"expected_points":{"type":"integer","minimum":1,"maximum":32}
                },"required":["title","kind","x_label","y_label","x_min","x_max","y_min","y_max","expected_points"],"additionalProperties":false},
                "generation":{"type":"integer"},
                "index":{"type":"integer","minimum":0,"maximum":31},
                "point":{"type":"object","properties":{"label":{"type":"string"},"x":{"type":"number"},"y":{"type":"number"}},"required":["label","x","y"],"additionalProperties":false}
            },"required":["op","plot"],"additionalProperties":false}),
        }
    }
    fn call(&self, args: &Value) -> Result<String, String> {
        let request = serde_json::from_value::<GraphRequest>(without_nulls(args))
            .map_err(|e| format!("invalid graph request: {e}"))?;
        let receipt = self
            .store
            .lock()
            .map_err(|_| "graph store unavailable")?
            .apply(request)?;
        serde_json::to_string(&receipt).map_err(|e| e.to_string())
    }
}

/// The flat schema lets a model send every field; the ones its op does not
/// take arrive as null and would trip `deny_unknown_fields`.
fn without_nulls(args: &Value) -> Value {
    match args {
        Value::Object(map) => Value::Object(
            map.iter()
                .filter(|(_, v)| !v.is_null())
                .map(|(k, v)| (k.clone(), v.clone()))
                .collect(),
        ),
        other => other.clone(),
    }
}

pub(crate) fn emit_requested(tx: &Sender<TurnEvent>, id: &ToolEventId, name: &str, args: &Value) {
    if name == "graph"
        && let Ok(request) = serde_json::from_value::<GraphRequest>(without_nulls(args))
        && request.validate().is_ok()
    {
        let _ = tx.send(TurnEvent::GraphCrop {
            id: id.clone(),
            event: Box::new(GraphEvent::Requested(request)),
        });
    }
}
pub(crate) fn emit_returned(
    tx: &Sender<TurnEvent>,
    id: &ToolEventId,
    name: &str,
    result: &str,
    outcome: ToolOutcome,
) {
    if name == "graph"
        && outcome.attributable_success()
        && result.len() <= 16_384
        && let Ok(receipt) = serde_json::from_str::<GraphReceipt>(result)
    {
        let _ = tx.send(TurnEvent::GraphCrop {
            id: id.clone(),
            event: Box::new(GraphEvent::Returned(receipt)),
        });
    }
}

#[cfg(test)]
#[path = "../../../../tests/cockpit/tools/graph__tests.rs"]
mod tests;
