use super::*;

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

#[test]
fn idle_renders_launch_hints() {
    let state = RlState::default();
    let flat = flatten(&render_view(&state, RlView::Branch, 0.0, 60, 14));
    assert!(flat.contains("/rl run"), "{flat}");
    assert!(flat.contains("--audit"), "{flat}");
    assert!(flat.contains("/rl status"), "{flat}");
}

fn measured_progress(rewards: &[f32]) -> RlState {
    let state = RlState::default();
    {
        let mut progress = state.progress.lock().unwrap();
        progress.planned_attempts = 12;
        progress.passed = rewards.iter().filter(|reward| **reward >= 1.0).count();
        progress.red = rewards.len() - progress.passed;
        progress.points = rewards
            .iter()
            .enumerate()
            .map(|(index, reward)| crate::drive::rl_ctl::RunPoint {
                step: index + 1,
                reward: *reward,
                latency_ms: 1_200 + index as u64 * 10,
            })
            .collect();
        progress
            .log_tail
            .push_back("attempt 1 · answer".to_string());
    }
    let mut state = state;
    state.mode = RlMode::Campaign;
    state
}

#[test]
fn campaign_renders_measured_reward_curve_and_attempt_status() {
    let rewards: Vec<f32> = (1..=8).map(|step| step as f32 / 8.0).collect();
    let state = measured_progress(&rewards);
    let flat = flatten(&render_view(&state, RlView::Branch, 1.0, 52, 20));
    assert!(flat.contains("attempt 8/12"), "{flat}");
    assert!(flat.contains("measured reward"), "{flat}");
    assert!(
        flat.contains("[attempt]") && flat.contains("[install]"),
        "{flat}"
    );
    assert!(
        flat.chars()
            .any(|ch| ('\u{2801}'..='\u{28FF}').contains(&ch)),
        "expected braille dots in the reward curve: {flat}"
    );
}

#[test]
fn released_campaign_banner_says_audited_and_installed() {
    let state = measured_progress(&[0.0, 1.0, 1.0, 1.0]);
    {
        let mut progress = state.progress.lock().unwrap();
        progress.promoted = true;
        progress.outcome = Some(Ok(CampaignOutcome {
            attempted: 4,
            passed: 3,
            red: 1,
            rounds: 1,
            promoted_rounds: 1,
            policy_version: 1,
            decision: "promoted".into(),
            mean_delta: Some(0.75),
            validated: true,
            audit_supplied: true,
            release_sha256: Some("a".repeat(64)),
            solve_rate: Some(0.5),
            advantage_variance: Some(0.25),
            reflection: true,
            accepted_entry: Some("rl-policy".into()),
            accepted_event: Some("r1".into()),
            report_path: "/tmp/rl/run-1".into(),
            route: crate::agent::club::RouteIdentity {
                driver: "fixture".into(),
                model: None,
                reasoning_effort: None,
            },
            wall_s: 42.0,
        }));
    }
    let flat = flatten(&render_view(&state, RlView::Branch, 0.0, 64, 12));
    assert!(flat.contains("RELEASED · audited"), "{flat}");
    assert!(title(&state).contains("released v1"), "{}", title(&state));
}

#[test]
fn measured_but_uninstalled_campaign_is_not_called_released() {
    let state = measured_progress(&[1.0, 1.0]);
    {
        let mut progress = state.progress.lock().unwrap();
        progress.outcome = Some(Ok(CampaignOutcome {
            attempted: 2,
            passed: 2,
            red: 0,
            rounds: 1,
            promoted_rounds: 1,
            policy_version: 1,
            decision: "promoted".into(),
            mean_delta: Some(1.0),
            validated: false,
            audit_supplied: false,
            release_sha256: None,
            solve_rate: Some(1.0),
            advantage_variance: Some(0.0),
            reflection: true,
            accepted_entry: None,
            accepted_event: None,
            report_path: "/tmp/rl/run-2".into(),
            route: crate::agent::club::RouteIdentity {
                driver: "fixture".into(),
                model: None,
                reasoning_effort: None,
            },
            wall_s: 9.0,
        }));
    }
    let flat = flatten(&render_view(&state, RlView::Branch, 0.0, 64, 12));
    assert!(flat.contains("MEASURED · not installed"), "{flat}");
    assert!(!flat.contains("RELEASED"), "{flat}");
    assert!(title(&state).contains("not installed"), "{}", title(&state));
}

#[test]
fn research_view_is_a_sampled_measured_ledger() {
    let rewards: Vec<f32> = (1..=120)
        .map(|step| if step % 3 == 0 { 1.0 } else { 0.0 })
        .collect();
    let state = measured_progress(&rewards);
    let flat = flatten(&render_view(&state, RlView::Research, 0.0, 64, 14));
    assert!(flat.contains("OPTIMIZATION LEDGER"), "{flat}");
    assert!(
        flat.contains("REWARD") && flat.contains("LATENCY"),
        "{flat}"
    );
    assert!(
        flat.contains("120"),
        "last attempt must survive sampling\n{flat}"
    );
    assert!(flat.contains("PASS") || flat.contains("RED"), "{flat}");
}

#[test]
fn sankey_view_has_no_flow_before_a_cohort_telemetry_snapshot() {
    let state = measured_progress(&[0.0, 1.0]);
    let flat = flatten(&render_view(&state, RlView::Sankey, 0.0, 64, 12));
    assert!(flat.contains("SANKEY"), "{flat}");
    assert!(!flat.contains("HELD-OUT"), "{flat}");
}

#[test]
fn rl_view_steps_wrap_in_both_directions() {
    assert_eq!(RlView::Branch.step(-1), RlView::Sankey);
    assert_eq!(RlView::Sankey.step(1), RlView::Branch);
}

#[test]
fn render_respects_height_budget() {
    let state = measured_progress(&[0.0, 1.0, 1.0]);
    state.progress.lock().unwrap().outcome = None;
    for height in [1u16, 3, 6, 24] {
        for view in RlView::ALL {
            let text = render_view(&state, view, 0.0, 40, height);
            assert!(text.lines.len() <= height as usize);
        }
    }
}

#[test]
fn campaign_row_marks_rejected_gate() {
    let snapshot = RlSnapshot {
        source: "nontechnical".into(),
        phase: RlPhase::Done,
        gate: Some(telemetry::CohortVerdict {
            role: "promotion".into(),
            decision: "rejected-below-floor".into(),
            promoted: false,
            mean_delta: Some(-0.5),
            delta_lower_bound: Some(-0.6),
        }),
        ..RlSnapshot::default()
    };
    let line = campaign_node_row(&snapshot, 0.0, 60);
    let flat: String = line
        .spans
        .iter()
        .map(|span| span.content.as_ref())
        .collect();
    assert!(flat.contains("[gate]"), "{flat}");
    assert!(
        !flat.contains("[audit]"),
        "nontechnical run has no audit node: {flat}"
    );
}
