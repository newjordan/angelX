//! ⡪ research: the Sloptomizer's and rl_campaign's lifecycle as routes.
use super::*;
use serde_json::json;

#[test]
fn a_research_record_raises_the_route_its_state_calls_for() {
    let cases = [
        (json!({"status": "running", "running": true}), vec![LIVE]),
        (
            json!({"status": "completed", "running": false,
                   "measurements": {"candidate_passed": true, "baseline_passed": false, "paired_delta": 1}}),
            vec![WON, PASSED],
        ),
        (
            json!({"status": "completed", "running": false,
                   "measurements": {"candidate_passed": true, "baseline_passed": null, "paired_delta": null}}),
            vec![PASSED],
        ),
        (
            json!({"status": "completed", "running": false,
                   "measurements": {"candidate_passed": false, "baseline_passed": true, "paired_delta": -1}}),
            vec![FAILED],
        ),
        (
            json!({"status": "completed", "running": false,
                   "measurements": {"candidate_passed": null, "baseline_passed": null, "paired_delta": null}}),
            vec![UNVERIFIED],
        ),
        (
            json!({"status": "completed_with_learning_error", "running": false,
                   "measurements": {"candidate_passed": true}}),
            vec![PASSED, LEARNING_ERROR],
        ),
        (
            json!({"status": "failed", "running": false, "baseline": {"stop_reason": "max_hops"}}),
            vec![BASELINE_RED],
        ),
        (
            json!({"status": "failed", "running": false, "baseline": {}, "candidate": {}}),
            vec![],
        ),
        (json!({"status": "stopped", "running": false}), vec![]),
        (json!({"status": "idle"}), vec![]),
    ];
    for (record, routes) in cases {
        assert_eq!(research(&record), routes, "{record}");
    }
}

#[test]
fn a_campaign_raises_live_then_its_verdict() {
    assert_eq!(campaign(&json!({"status": "running"})), vec![LIVE]);
    assert_eq!(campaign(&json!({"status": "settled"})), vec![CAMPAIGN]);
    assert!(campaign(&json!({"status": "idle"})).is_empty());
    assert!(no_spread(&json!({"advantage_variance": 0.0})));
    assert!(!no_spread(&json!({"advantage_variance": 0.16})));
    assert!(!no_spread(&json!({"advantage_variance": null})));
    // As a campaign's status and history rows carry it: serde's `Result`.
    assert!(no_spread(&json!({"Ok": {"advantage_variance": 0.0}})));
    assert!(!no_spread(&json!({"Ok": {"advantage_variance": 0.16}})));
    assert!(!no_spread(&json!({"Err": "campaign cancelled"})));
}

#[test]
fn advice_is_cold_with_no_observations() {
    assert!(cold_advice(&json!({"observations": 0})));
    assert!(!cold_advice(&json!({"observations": 3})));
}

#[test]
fn every_research_route_is_introduced_as_an_action() {
    let root = std::env::temp_dir().join(format!("angel-book-research-{}", std::process::id()));
    std::fs::create_dir_all(&root).unwrap();
    for sub in PRIMARY.subs {
        assert!(
            super::super::ledger::read(&root, &sub.route.cells()).is_ok(),
            "{}",
            sub.name
        );
        // A verdict the model must act on carries its action.
        if sub.pages.is_empty() {
            assert!(!sub.action.is_empty(), "{} has no action", sub.name);
        }
    }
    // A stamp in a tool's JSON `warpath` field is introduced in English.
    let record = crate::agent::club::ChatMsg::tool(
        "r",
        r#"{"status":"completed","warpath":"⡪⠃⡪⠁"}"#.to_string(),
    );
    let intros = super::super::introduction::introductions(&[record], None);
    assert_eq!(intros.len(), 1, "{intros:?}");
    assert!(
        intros[0]
            .1
            .contains("⡪⠃ the candidate beat its paired baseline")
    );
    assert!(intros[0].1.contains("⡪⠁ a loop_research candidate passed"));
    assert!(intros[0].1.contains("→ apply its patch from results"));
    let _ = std::fs::remove_dir_all(root);
}
