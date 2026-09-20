use super::*;

fn turn(
    model: &str,
    effort: &str,
    ok: bool,
    failover: bool,
    latency_ms: u64,
    tokens: u64,
) -> String {
    serde_json::json!({
        "kind": "turn",
        "v": 2,
        "ts": latency_ms,
        "driver": "openai",
        "route": {
            "driver": "openai",
            "model": model,
            "reasoning_effort": effort,
        },
        "outcome": {
            "ok": ok,
            "stop": if ok { "answer" } else { "error_stop" },
            "latency_ms": latency_ms,
            "tokens": { "openai": { "in": tokens, "out": 0 } },
            "failovers": if failover { vec![serde_json::json!({"to":"practice"})] } else { vec![] },
        }
    })
    .to_string()
}

fn verdict(model: &str, effort: &str, value: &str, ts: u64) -> String {
    serde_json::json!({
        "kind": "route_verdict",
        "v": 2,
        "ts": ts,
        "source": "user",
        "route": {
            "driver": "openai",
            "model": model,
            "reasoning_effort": effort,
        },
        "verdict": value,
    })
    .to_string()
}

#[test]
fn parser_groups_exact_model_effort_and_labels_recovery_separately() {
    let raw = [
        turn("gpt-5.6-sol", "ultra", true, false, 100, 1000),
        turn("gpt-5.6-sol", "ultra", true, true, 300, 3000),
        turn("gpt-5.6-sol", "ultra", false, false, 200, 2000),
        turn("gpt-5.6-sol", "low", true, false, 50, 500),
    ]
    .join("\n");
    let snapshot = parse_recent_jsonl(&raw, false);
    let ultra = snapshot
        .find("openai", "gpt-5.6-sol", Some("ultra"))
        .unwrap();
    assert_eq!(ultra.samples, 3);
    assert_eq!(ultra.clean_answers, 1);
    assert_eq!(ultra.recovered_answers, 1);
    assert_eq!(ultra.failures, 1);
    assert_eq!(ultra.clean_percent(), 33);
    assert_eq!(ultra.p50_latency_ms, 200);
    assert_eq!(ultra.mean_tokens, 2000);
    assert_eq!(
        snapshot
            .find("openai", "gpt-5.6-sol", Some("low"))
            .unwrap()
            .samples,
        1
    );
}

#[test]
fn parser_skips_malformed_and_never_smears_legacy_openai_across_models() {
    let legacy = serde_json::json!({
        "kind":"turn", "driver":"openai",
        "outcome":{"ok":true,"stop":"answer","latency_ms":1,"failovers":[]}
    });
    let raw = format!("{{broken\n{legacy}\n");
    let snapshot = parse_recent_jsonl(&raw, false);
    assert_eq!(snapshot.malformed_lines, 1);
    assert_eq!(snapshot.scanned_lines, 2);
    assert!(
        snapshot
            .find("openai", "gpt-5.6-sol", Some("ultra"))
            .is_none()
    );
}

#[test]
fn parser_keeps_user_quality_separate_and_rejects_fake_provenance() {
    let automated = serde_json::json!({
        "kind":"route_verdict", "v":2, "ts":4, "source":"grader",
        "route":{"driver":"openai","model":"gpt-5.6-sol","reasoning_effort":"ultra"},
        "verdict":"useful"
    });
    let raw = [
        turn("gpt-5.6-sol", "ultra", true, false, 100, 1000),
        verdict("gpt-5.6-sol", "ultra", "useful", 2),
        verdict("gpt-5.6-sol", "ultra", "miss", 3),
        automated.to_string(),
    ]
    .join("\n");
    let snapshot = parse_recent_jsonl(&raw, false);
    let route = snapshot
        .find("openai", "gpt-5.6-sol", Some("ultra"))
        .unwrap();
    assert_eq!(route.samples, 1, "labels are not operational samples");
    assert_eq!(route.clean_answers, 1);
    assert_eq!(route.user_useful, 1);
    assert_eq!(route.user_miss, 1);
    assert_eq!(route.user_quality_samples(), 2);
    assert_eq!(route.useful_percent(), 50);
    assert_eq!(route.explicit_quality_score(), None);
    assert_eq!(route.quality_last_ts, 3);
}

#[test]
fn operational_score_requires_evidence_and_shrinks_small_samples() {
    let sparse = RouteEvidence {
        samples: 1,
        clean_answers: 1,
        ..RouteEvidence::default()
    };
    assert_eq!(sparse.operational_score(), None);
    let reliable = RouteEvidence {
        samples: 8,
        clean_answers: 7,
        ..RouteEvidence::default()
    };
    let shaky = RouteEvidence {
        samples: 8,
        clean_answers: 4,
        ..RouteEvidence::default()
    };
    assert!(reliable.operational_score() > shaky.operational_score());
}

#[test]
fn quality_pick_requires_labels_and_rewards_deeper_equal_evidence() {
    let snapshot = RouteEvidenceSnapshot {
        routes: vec![
            RouteEvidence {
                driver: "openai".to_string(),
                model: Some("small-sample".to_string()),
                effort: Some("high".to_string()),
                user_useful: 5,
                ..RouteEvidence::default()
            },
            RouteEvidence {
                driver: "openai".to_string(),
                model: Some("deep-sample".to_string()),
                effort: Some("high".to_string()),
                user_useful: 20,
                ..RouteEvidence::default()
            },
            RouteEvidence {
                driver: "openai".to_string(),
                model: Some("too-sparse".to_string()),
                effort: Some("high".to_string()),
                user_useful: 4,
                ..RouteEvidence::default()
            },
        ],
        ..RouteEvidenceSnapshot::default()
    };
    let choices = ["small-sample", "deep-sample", "too-sparse"]
        .into_iter()
        .enumerate()
        .map(|(index, model)| RouteChoice {
            agent_index: index,
            slot_index: 0,
            agent: model.to_string(),
            driver: "openai".to_string(),
            model: model.to_string(),
            available: true,
            selected: false,
            reasoning_effort: Some("high".to_string()),
            reasoning_levels: std::sync::Arc::from(vec!["high".to_string()]),
            metadata: crate::club::RouteMetadata::default(),
        })
        .collect::<Vec<_>>();
    assert_eq!(snapshot.routes[2].explicit_quality_score(), None);
    assert!(
        snapshot.routes[1].explicit_quality_score() > snapshot.routes[0].explicit_quality_score()
    );
    assert_eq!(quality_pick_for_context(&snapshot, &choices, 0), Some(1));
}

#[test]
fn context_aware_picks_preserve_reserve_over_stronger_history() {
    let snapshot = RouteEvidenceSnapshot {
        routes: vec![
            RouteEvidence {
                driver: "openai".to_string(),
                model: Some("small-window".to_string()),
                effort: Some("high".to_string()),
                samples: 20,
                clean_answers: 20,
                p50_latency_ms: 50,
                user_useful: 20,
                ..RouteEvidence::default()
            },
            RouteEvidence {
                driver: "openai".to_string(),
                model: Some("large-window".to_string()),
                effort: Some("high".to_string()),
                samples: 20,
                clean_answers: 16,
                p50_latency_ms: 100,
                user_useful: 15,
                user_miss: 5,
                ..RouteEvidence::default()
            },
        ],
        ..RouteEvidenceSnapshot::default()
    };
    let choices = [("small-window", 100), ("large-window", 1_000)]
        .into_iter()
        .enumerate()
        .map(|(index, (model, window))| RouteChoice {
            agent_index: index,
            slot_index: 0,
            agent: model.to_string(),
            driver: "openai".to_string(),
            model: model.to_string(),
            available: true,
            selected: false,
            reasoning_effort: Some("high".to_string()),
            reasoning_levels: std::sync::Arc::from(vec!["high".to_string()]),
            metadata: crate::club::RouteMetadata {
                context_window: Some(window),
                ..crate::club::RouteMetadata::default()
            },
        })
        .collect::<Vec<_>>();

    assert_eq!(
        operational_pick_for_context(&snapshot, &choices, 0),
        Some(0)
    );
    assert_eq!(quality_pick_for_context(&snapshot, &choices, 0), Some(0));
    assert_eq!(
        operational_pick_for_context(&snapshot, &choices, 85),
        Some(1),
        "the stronger historical route is already 85% occupied"
    );
    assert_eq!(
        quality_pick_for_context(&snapshot, &choices, 85),
        Some(1),
        "explicit quality must not override inadequate context reserve"
    );
}

#[test]
fn effort_picks_keep_operational_and_user_quality_channels_separate() {
    let snapshot = RouteEvidenceSnapshot {
        routes: vec![
            RouteEvidence {
                driver: "openai".to_string(),
                model: Some("frontier".to_string()),
                effort: Some("low".to_string()),
                samples: 20,
                clean_answers: 20,
                p50_latency_ms: 100,
                user_useful: 5,
                user_miss: 15,
                ..RouteEvidence::default()
            },
            RouteEvidence {
                driver: "openai".to_string(),
                model: Some("frontier".to_string()),
                effort: Some("high".to_string()),
                samples: 20,
                clean_answers: 15,
                p50_latency_ms: 300,
                user_useful: 20,
                ..RouteEvidence::default()
            },
        ],
        ..RouteEvidenceSnapshot::default()
    };
    let choice = RouteChoice {
        agent_index: 0,
        slot_index: 0,
        agent: "openai".to_string(),
        driver: "openai".to_string(),
        model: "frontier".to_string(),
        available: true,
        selected: true,
        reasoning_effort: Some("low".to_string()),
        reasoning_levels: std::sync::Arc::from(vec![
            "low".to_string(),
            "medium".to_string(),
            "high".to_string(),
        ]),
        metadata: crate::club::RouteMetadata::default(),
    };

    assert_eq!(
        operational_effort_pick(&snapshot, &choice, &choice.reasoning_levels),
        Some(0)
    );
    assert_eq!(
        quality_effort_pick(&snapshot, &choice, &choice.reasoning_levels),
        Some(2)
    );
}

#[test]
fn in_memory_verdict_is_visible_without_disk_reload() {
    let mut snapshot = RouteEvidenceSnapshot::default();
    let route = crate::club::RouteIdentity {
        driver: "openai".to_string(),
        model: Some("gpt-5.6-sol".to_string()),
        reasoning_effort: Some("ultra".to_string()),
    };
    snapshot.apply_user_verdict(&route, crate::experience::RouteVerdict::Useful, 12_000);
    let evidence = snapshot
        .find("openai", "gpt-5.6-sol", Some("ultra"))
        .unwrap();
    assert_eq!(evidence.user_useful, 1);
    assert_eq!(evidence.quality_last_ts, 12);
}

#[test]
fn bounded_loader_drops_partial_first_line_and_keeps_recent_records() {
    let dir = std::env::temp_dir().join(format!(
        "angel-route-evidence-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("ledger.jsonl");
    let old = turn("old", "low", true, false, 1, 1);
    let recent = turn("gpt-5.6-sol", "ultra", true, false, 2, 2);
    std::fs::write(&path, format!("{old}\n{recent}\n")).unwrap();
    let snapshot = load_recent(&path, recent.len() as u64 + 2);
    assert!(snapshot.truncated);
    assert_eq!(snapshot.routes.len(), 1);
    assert!(
        snapshot
            .find("openai", "gpt-5.6-sol", Some("ultra"))
            .is_some()
    );
    let _ = std::fs::remove_dir_all(dir);
}
