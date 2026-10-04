use super::*;
use crate::agent::harness::{ExecutionOutcome, VerificationOutcome};
fn begin() -> Value {
    json!({"op":"begin","plot":"harvest","spec":{"title":"Harvest","kind":"bar","x_label":"Day","y_label":"Bushels","x_min":0,"x_max":3,"y_min":0,"y_max":10,"expected_points":1}})
}
fn outcome(execution: ExecutionOutcome) -> ToolOutcome {
    ToolOutcome {
        execution,
        verification: VerificationOutcome::NotApplicable,
    }
}

#[test]
fn graph_crop_tool_receipts_keep_full_data_and_failed_outcomes_emit_no_crops() {
    let tool = GraphTool::default();
    let args = begin();
    let result = tool.call(&args).unwrap();
    let receipt: GraphReceipt = serde_json::from_str(&result).unwrap();
    assert_eq!(receipt.chart.unwrap().generation, 1);
    let (tx, rx) = std::sync::mpsc::channel();
    let id = ToolEventId("actual-point".into());
    emit_requested(&tx, &id, "graph", &args);
    emit_returned(
        &tx,
        &id,
        "graph",
        &result,
        outcome(ExecutionOutcome::Failed),
    );
    assert_eq!(rx.try_iter().count(), 1);
    emit_returned(
        &tx,
        &id,
        "graph",
        &result,
        outcome(ExecutionOutcome::Succeeded),
    );
    assert!(
        matches!(rx.try_recv().unwrap(),TurnEvent::GraphCrop { event,.. } if matches!(*event,GraphEvent::Returned(_)))
    );
    let args = json!({"op":"point","plot":"harvest","generation":1,"index":0,"point":{"label":"measured","x":1,"y":7}});
    let result = tool.call(&args).unwrap();
    assert_eq!(
        serde_json::from_str::<GraphReceipt>(&result)
            .unwrap()
            .chart
            .unwrap()
            .points[&0]
            .y,
        7.0
    );
}

#[test]
fn graph_crop_harness_dispatch_emits_one_sprite_per_actual_point_call() {
    use crate::agent::club::{ChatMsg, Club, ClubReply, ToolCall};
    use crate::agent::harness::{ToolRegistry, run_turn};
    use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
    let _guard = crate::tests::env_lock();
    let _skill = crate::tests::TestEnvGuard::set("ANGEL_SKILL_HINT", "0");
    let _advisor = crate::tests::TestEnvGuard::set("ANGEL_ADVISOR", "0");
    struct GraphClub(AtomicUsize);
    impl Club for GraphClub {
        fn label(&self) -> &str {
            "graph-fixture"
        }
        fn respond(&self, _: &str) -> Result<String, String> {
            Ok("unused".into())
        }
        fn chat(&self, _: &[ChatMsg], _: &[ToolDef]) -> Result<ClubReply, String> {
            let n = self.0.fetch_add(1, Ordering::SeqCst);
            let args = match n {
                0 => begin(),
                1 => {
                    json!({"op":"point","plot":"harvest","generation":1,"index":0,"point":{"label":"measured","x":1,"y":7}})
                }
                2 => json!({"op":"finish","plot":"harvest","generation":1}),
                _ => {
                    return Ok(ClubReply::Text(
                        "The measured harvest chart is complete.".into(),
                    ));
                }
            };
            Ok(ClubReply::Calls(vec![ToolCall {
                id: format!("graph-{n}"),
                name: "graph".into(),
                args,
            }]))
        }
    }
    let mut registry = ToolRegistry::new();
    registry.register(Box::new(GraphTool::default()));
    let (tx, rx) = std::sync::mpsc::channel();
    run_turn(
        &GraphClub(AtomicUsize::new(0)),
        &registry,
        &mut vec![ChatMsg::user("Chart the measured harvest")],
        &AtomicBool::new(false),
        Some(8),
        &tx,
    )
    .unwrap();
    let events: Vec<_> = rx.try_iter().collect();
    assert_eq!(events.iter().filter(|e|matches!(e,TurnEvent::GraphCrop { event,.. } if matches!(event.as_ref(),GraphEvent::Requested(GraphRequest::Point { .. })))).count(),1);
    let mut world = crate::stage::world_viz::World::new(7);
    for event in events {
        match event {
            TurnEvent::GraphCrop { id, event } => world.note_graph_event(&id, &event),
            TurnEvent::ToolResult {
                id, name, outcome, ..
            } if name == "graph" => world.settle_graph_call(&id, outcome),
            _ => {}
        }
    }
    let report = world.graph_report(None);
    assert!(report.contains("finished"));
    assert!(report.contains("x=1 y=7"));
}

#[test]
fn graph_crop_code_mode_keeps_executed_data_when_outer_output_budget_fails() {
    use crate::agent::harness::{
        CodeModeTool, Hooks, ToolRegistry, dispatch_with_hooks_events_cancel,
    };
    let _guard = crate::tests::env_lock();
    let _effects = crate::tests::TestEnvGuard::set("ANGEL_CODE_MODE_EFFECTS", "1");
    let _bytes = crate::tests::TestEnvGuard::set("ANGEL_CODE_MODE_MAX_NESTED_OUTPUT_BYTES", "16");
    let tool = GraphTool::default();
    tool.call(&begin()).unwrap();
    let mut registry = ToolRegistry::new();
    registry.register(Box::new(tool));
    registry.register(Box::new(CodeModeTool::new(&registry.bindable_tool_names())));
    let (tx, rx) = std::sync::mpsc::channel();
    let outer = ToolEventId("programmed-chart".into());
    let args = json!({"allow_effects":true,"script":"return graph({op:'point',plot:'harvest',generation:1,index:0,point:{label:'measured',x:1,y:7}});"});
    let result = dispatch_with_hooks_events_cancel(
        &registry,
        &Hooks::default(),
        "code_mode",
        &args,
        Some((&outer, &tx)),
        None,
    );
    assert!(
        result.contains("nested-output budget exhausted"),
        "{result}"
    );
    let mut world = crate::stage::world_viz::World::new(7);
    let mut ids = Vec::new();
    for event in rx.try_iter() {
        match event {
            TurnEvent::GraphCrop { id, event } => {
                ids.push(id.0.clone());
                world.note_graph_event(&id, &event);
            }
            TurnEvent::ToolResult {
                id, name, outcome, ..
            } if name == "graph" => world.settle_graph_call(&id, outcome),
            _ => {}
        }
    }
    assert_eq!(ids, vec!["programmed-chart:1", "programmed-chart:1"]);
    assert!(world.graph_report(None).contains("x=1 y=7"));
}

#[test]
fn graph_schema_is_flat_so_every_provider_sees_its_fields() {
    // GLM on Z.ai sent `{}` for every call while the root was a bare `oneOf`.
    let params = GraphTool::default().def().params;
    assert!(params.get("oneOf").is_none());
    let props = params["properties"]
        .as_object()
        .expect("top-level properties");
    for key in ["op", "plot", "spec", "generation", "index", "point"] {
        assert!(props.contains_key(key), "{key}");
    }
    assert_eq!(params["required"], json!(["op", "plot"]));
}

#[test]
fn graph_calls_ignore_the_null_fields_a_flat_schema_invites() {
    let tool = GraphTool::default();
    let mut args = begin();
    args["generation"] = Value::Null;
    args["index"] = Value::Null;
    args["point"] = Value::Null;
    let receipt: GraphReceipt = serde_json::from_str(&tool.call(&args).unwrap()).unwrap();
    assert_eq!(receipt.chart.unwrap().generation, 1);
    let point = json!({"op":"point","plot":"harvest","generation":1,"index":0,
        "point":{"label":"Day 1","x":1,"y":7},"spec":null});
    assert!(tool.call(&point).is_ok());
}

/// Every tool property names its type. Kimi (Moonshot) rejects a schema whose
/// property has none ("type is not defined") with HTTP 400 on hop 1, which
/// failed two polyglot tasks outright when the graph tool's `op` and
/// `spec.kind` were bare enums.
#[test]
fn every_default_tool_property_names_its_type() {
    fn walk(path: &str, schema: &Value, bad: &mut Vec<String>) {
        if let Some(props) = schema.get("properties").and_then(Value::as_object) {
            for (name, prop) in props {
                let here = format!("{path}.{name}");
                let typed = ["type", "anyOf", "oneOf", "allOf", "$ref"]
                    .iter()
                    .any(|key| prop.get(*key).is_some());
                if !typed {
                    bad.push(here.clone());
                }
                walk(&here, prop, bad);
            }
        }
        if let Some(items) = schema.get("items") {
            walk(&format!("{path}[]"), items, bad);
        }
        for key in ["anyOf", "oneOf", "allOf"] {
            for (i, branch) in schema
                .get(key)
                .and_then(Value::as_array)
                .into_iter()
                .flatten()
                .enumerate()
            {
                walk(&format!("{path}.{key}[{i}]"), branch, bad);
            }
        }
    }
    let mut bad = Vec::new();
    for def in crate::agent::harness::ToolRegistry::with_defaults().defs() {
        walk(&def.name, &def.params, &mut bad);
    }
    assert!(bad.is_empty(), "properties without a type: {bad:?}");
}
