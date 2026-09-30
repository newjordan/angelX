use super::*;
use crate::agent::{
    harness::{ExecutionOutcome, Tool, ToolEventId, ToolOutcome, VerificationOutcome},
    tools::graph::{GraphTool, emit_requested, emit_returned},
};

#[test]
fn graph_crop_native_events_keep_data_when_the_world_stage_is_hidden() {
    let _guard = crate::tests::env_lock();
    let mut app = App::preview(crate::ui::viewer::Viewer::static_preview());
    app.world_pane_visible = false;
    let mut thinking = Thinking::pending_for_test("graph-fixture");
    let (_worker_tx, worker_rx) = mpsc::channel();
    thinking.rx = worker_rx;
    let (events, event_rx) = mpsc::channel();
    thinking.event_rx = event_rx;
    app.thinking = Some(thinking);
    let tool = GraphTool::default();
    let outcome = ToolOutcome {
        execution: ExecutionOutcome::Succeeded,
        verification: VerificationOutcome::NotApplicable,
    };
    for (n,args) in [
        serde_json::json!({"op":"begin","plot":"harvest","spec":{"title":"Harvest","kind":"bar","x_label":"Day","y_label":"Bushels","x_min":0,"x_max":3,"y_min":0,"y_max":10,"expected_points":1}}),
        serde_json::json!({"op":"point","plot":"harvest","generation":1,"index":0,"point":{"label":"measured","x":1,"y":7}}),
    ].into_iter().enumerate() {
        let id=ToolEventId(format!("hidden-{n}"));
        events.send(TurnEvent::ToolCall { id:id.clone(),name:"graph".into(),args_summary:"truncated; no numerical authority".into() }).unwrap();
        emit_requested(&events,&id,"graph",&args);
        let result=tool.call(&args).unwrap(); emit_returned(&events,&id,"graph",&result,outcome);
        events.send(TurnEvent::ToolResult { id,name:"graph".into(),summary:"truncated".into(),outcome }).unwrap();
    }
    app.advance();
    assert!(app.world.graph_report(None).contains("x=1 y=7"));
    app.input = "/world crops".into();
    app.submit();
    assert_eq!(app.world.overworld_view_label(), Some("GRAPH GARDEN"));
    assert!(app.messages.iter().any(|m| m.text.contains("x=1 y=7")));
}

#[test]
fn graph_crop_payload_bytes_and_visibility_include_actual_chart_data() {
    use crate::knowledge::graph_crop::{GraphEvent, GraphRequest};
    let request = GraphRequest::Clear {
        plot: "harvest".into(),
    };
    let event = TurnEvent::GraphCrop {
        id: ToolEventId("point".into()),
        event: Box::new(GraphEvent::Requested(request)),
    };
    assert!(stream_event_payload_bytes(&event) > "harvest".len());
    assert!(stream_event_is_operator_visible(&event));
}
