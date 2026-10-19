use super::*;

#[test]
fn retired_viewers_are_not_loaded_or_published() {
    let mut saved = serde_json::to_value(delve()).unwrap();
    saved["audience"] = serde_json::json!(10000);
    saved["fans"] = serde_json::json!(6);
    saved["fan_boxes"] = serde_json::json!([{"x": 20.0, "y": 10.0, "fall": 1}]);
    let mut run: Run = serde_json::from_value(saved).unwrap();
    let before = run.room().items.len();
    run.cues.push("victory".into());
    run.step(&BTreeMap::new());
    let published = serde_json::to_value(run.wire_copy()).unwrap();
    for key in ["audience", "fans", "fan_boxes"] {
        assert!(published.get(key).is_none(), "retired field {key} was published");
    }
    assert_eq!(run.room().items.len(), before, "legacy fan boxes must not drop loot");
    assert!(!run.cues.iter().any(|cue| cue.starts_with("fan_box") || cue.starts_with("audience_")));
}

#[test]
fn retired_sponsor_mode_is_not_offered() {
    assert!(!fortune::Mode::WHEEL.iter().any(|mode| mode.word() == "sponsors"));
}

#[test]
fn retired_sponsor_spin_loads_on_the_normal_wedge() {
    for wedge in [14, 255] {
        let spin: fortune::Spin = serde_json::from_value(serde_json::json!({
            "started": 0, "from": 0.0, "wedge": wedge, "turns": 3, "by": 1
        })).unwrap();
        assert_eq!(spin.wedge, 0, "removed or invalid saved wedge must become the normal mode");
        assert_eq!(fortune::Mode::WHEEL[usize::from(spin.wedge)], fortune::Mode::LongWayDown);
        assert!(spin.angle(1000).is_finite());
    }
}

/// Floor one's entrance, its monsters gone, the party at ease.
fn delve() -> Run {
    let mut run = Run::new(5, 1, None);
    run.enter_for_test(0);
    run.calm_for_test();
    run.enemies.clear();
    run.phase = Phase::Exploring;
    run
}

#[test]
fn retired_sponsor_mode_loads_as_an_ordinary_delve() {
    let mode: fortune::Mode = serde_json::from_str("\"sponsors\"").unwrap();
    assert_eq!(mode, fortune::Mode::LongWayDown);
}

#[test]
fn retired_viewer_bounty_scores_cannot_complete_replacement_jobs() {
    let mut home: home::Home = serde_json::from_value(serde_json::json!({
        "bounties": [{"id": "ratings", "have": 1400}, {"id": "prime", "have": 4900}],
        "best_show": 10000
    })).unwrap();
    assert!(home.work_bounties(&BTreeMap::new(), 1).is_empty());
    assert!(home.bounties.is_empty());
    assert!(serde_json::to_value(home).unwrap().get("best_show").is_none());
}
