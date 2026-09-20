use super::*;
use crate::harness::GraphNodeSnap;

fn flatten(text: &Text<'static>) -> String {
    text.lines
        .iter()
        .map(|line| {
            line.spans
                .iter()
                .map(|span| span.content.as_ref())
                .collect::<String>()
        })
        .collect::<Vec<_>>()
        .join("\n")
}

fn snap(id: &str, deps: &[&str], phase: GraphNodePhase) -> GraphNodeSnap {
    GraphNodeSnap {
        id: id.to_string(),
        deps: deps.iter().map(|s| s.to_string()).collect(),
        persona: String::new(),
        club: "stub".to_string(),
        grant: "none".to_string(),
        pool: None,
        pool_limit: None,
        lease_id: None,
        phase,
        elapsed_ms: 0,
        retries: 0,
        gate: None,
        output_chars: 0,
    }
}

#[test]
fn layers_group_parallel_nodes() {
    let snapshot = GraphSnapshot {
        episode_id: "fixture".into(),
        workspace_key_sha256: "fixture".into(),
        graph: "fan".to_string(),
        task: "t".to_string(),
        phase: GraphRunPhase::Running,
        nodes: vec![
            snap("left", &[], GraphNodePhase::Done),
            snap("right", &[], GraphNodePhase::Running),
            snap("join", &["left", "right"], GraphNodePhase::Pending),
        ],
        events: vec!["0.1s · ▶ left (stub)".to_string()],
        final_answer: None,
        error: None,
        elapsed_ms: 100,
        fanin: Default::default(),
        incomplete_reason: None,
    };
    let grouped = layers(&snapshot);
    assert_eq!(grouped.len(), 2);
    assert_eq!(grouped[0], vec![0, 1]);
    assert_eq!(grouped[1], vec![2]);
    let flat = flatten(&Text::from(run_lines(&snapshot, 0.0, 60, 24)));
    assert!(flat.contains("[left]"), "{flat}");
    assert!(flat.contains("[join]"), "{flat}");
    assert!(flat.contains("timeline"), "{flat}");
}

#[test]
fn render_respects_height_budget_and_idle_hints() {
    for height in [1u16, 3, 8, 24] {
        let text = render(0.0, 48, height);
        assert!(text.lines.len() <= height as usize);
    }
}

#[test]
fn render_surfaces_role_pool_lease_and_capacity() {
    let mut worker = snap("research-a", &[], GraphNodePhase::Running);
    worker.pool = Some("research".to_string());
    worker.pool_limit = Some(2);
    worker.lease_id = Some(7);
    let snapshot = GraphSnapshot {
        episode_id: "fixture".into(),
        workspace_key_sha256: "fixture".into(),
        graph: "pooled".to_string(),
        task: "t".to_string(),
        phase: GraphRunPhase::Running,
        nodes: vec![worker],
        events: Vec::new(),
        final_answer: None,
        error: None,
        elapsed_ms: 1,
        fanin: Default::default(),
        incomplete_reason: None,
    };
    let flat = flatten(&Text::from(run_lines(&snapshot, 0.0, 80, 12)));
    assert!(flat.contains("pool:research · lease#7 · cap2"), "{flat}");
}
