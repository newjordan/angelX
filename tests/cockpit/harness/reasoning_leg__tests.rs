use super::*;
use crate::agent::club::ClubReply;
use crate::agent::harness::live_policy::{LegSource, decide_with};

struct Seat {
    levels: Vec<String>,
    effort: Option<String>,
    model: String,
}

impl Seat {
    fn new(levels: &[&str], effort: Option<&str>) -> Self {
        Self {
            levels: levels.iter().map(|level| level.to_string()).collect(),
            effort: effort.map(str::to_string),
            model: "muse-spark-1.3".into(),
        }
    }

    fn glm(levels: &[&str], effort: Option<&str>) -> Self {
        Self {
            model: "glm-5.3".into(),
            ..Self::new(levels, effort)
        }
    }
}

impl Club for Seat {
    fn respond(&self, _prompt: &str) -> Result<String, String> {
        Ok("done".into())
    }
    fn label(&self) -> &str {
        "muse"
    }
    fn model_identity(&self) -> Option<String> {
        Some(self.model.clone())
    }
    fn reasoning_levels(&self) -> &[String] {
        &self.levels
    }
    fn reasoning_effort(&self) -> Option<String> {
        self.effort.clone()
    }
    fn chat(&self, _messages: &[ChatMsg], _tools: &[ToolDef]) -> Result<ClubReply, String> {
        Ok(ClubReply::Text("done".into()))
    }
}

pub(crate) fn crossed_sota_input_milestone(previous: u64, current: u64) -> Option<u64> {
    let previous_millions = previous / SOTA_INPUT_MILESTONE_TOKENS;
    let current_millions = current / SOTA_INPUT_MILESTONE_TOKENS;
    (current_millions > previous_millions)
        .then(|| current_millions.saturating_mul(SOTA_INPUT_MILESTONE_TOKENS))
}

pub(crate) fn resolve_adaptive_reasoning_effort(
    club: &dyn Club,
    hop: usize,
    has_friction: bool,
) -> Option<String> {
    let levels = club.reasoning_levels();
    if levels.is_empty() {
        return None;
    }
    let base_effort = club.reasoning_effort().unwrap_or_else(|| {
        levels
            .iter()
            .find(|l| l.eq_ignore_ascii_case("high") || l.eq_ignore_ascii_case("max"))
            .cloned()
            .unwrap_or_else(|| levels.last().cloned().unwrap_or_default())
    });

    if hop == 0 || has_friction {
        Some(base_effort)
    } else if levels.iter().any(|l| l.eq_ignore_ascii_case("low")) {
        Some("low".to_string())
    } else if levels.iter().any(|l| l.eq_ignore_ascii_case("none")) {
        Some("none".to_string())
    } else {
        levels.first().cloned()
    }
}

pub(crate) fn glm_thinking_burn_demotion(
    club: &dyn Club,
    cumulative_reasoning: u64,
    submitted: bool,
    burn_limit: u64,
) -> Option<String> {
    if burn_limit == 0 || submitted || cumulative_reasoning < burn_limit {
        return None;
    }
    if !crate::agent::club::glm_thinking_locked_for_club(&club.model_identity().unwrap_or_default())
    {
        return None;
    }
    club.reasoning_levels()
        .iter()
        .find(|l| l.eq_ignore_ascii_case("low"))
        .cloned()
        .or_else(|| club.reasoning_levels().first().cloned())
}

pub(crate) fn turn_expired(elapsed_secs: u64, budget_secs: usize) -> bool {
    budget_secs > 0 && elapsed_secs >= budget_secs as u64
}

const LADDERS: [&[&str]; 6] = [
    &["low", "medium", "high", "xhigh"],
    &["none", "low", "high"],
    &["minimal", "low", "medium", "high", "xhigh"],
    &["low", "high"],
    &["High", "Low"],
    &[],
];

fn levels_of(ladder: &[&str]) -> Vec<String> {
    ladder.iter().map(|level| level.to_string()).collect()
}

#[test]
fn the_shipped_leg_reproduces_every_native_reasoning_decision() {
    let leg = crate::agent::harness::live_policy::shipped_leg("reasoning");
    let mut rows = 0;

    for ladder in LADDERS {
        for configured in [None, Some("low"), Some("high"), Some("max"), Some("auto")] {
            for hop in 0..4 {
                for friction in [false, true] {
                    let seat = Seat::new(ladder, configured);
                    let native = resolve_adaptive_reasoning_effort(&seat, hop, friction);
                    let input = serde_json::json!({
                        "op": "ladder",
                        "levels": ladder,
                        "effort": configured,
                        "hop": hop,
                        "friction": friction,
                    });
                    let levels = levels_of(ladder);
                    let (choice, source) = decide_with(
                        Some(leg.clone()),
                        "reasoning",
                        &input,
                        effort_check(&levels, native.as_deref()),
                        || Choice {
                            effort: native.clone(),
                        },
                    );
                    assert_eq!(
                        choice.effort, native,
                        "ladder {ladder:?} configured {configured:?} hop {hop} friction {friction}"
                    );
                    assert!(
                        matches!(source, LegSource::Live { .. }),
                        "ladder {ladder:?} kept the {} leg",
                        source.as_str()
                    );
                    rows += 1;
                }
            }
        }
    }

    for (ladder, locked) in LADDERS.iter().map(|l| (*l, false)).chain(
        LADDERS
            .iter()
            .map(|l| (*l, true)),
    ) {
        let seat = if locked {
            Seat::glm(ladder, Some("low"))
        } else {
            Seat::new(ladder, Some("low"))
        };
        for cumulative in [0u64, 999, 1_024, 1_000_000, 4_294_967_295] {
            for submitted in [false, true] {
                for limit in [0u64, 1_024, 2_048] {
                    let native =
                        glm_thinking_burn_demotion(&seat, cumulative, submitted, limit);
                    let input = serde_json::json!({
                        "op": "burn",
                        "levels": ladder,
                        "cumulative": cumulative,
                        "submitted": submitted,
                        "limit": limit,
                        "locked": crate::agent::club::glm_thinking_locked_for_club(
                            &seat.model_identity().unwrap_or_default(),
                        ),
                    });
                    let levels = levels_of(ladder);
                    let (choice, source) = decide_with(
                        Some(leg.clone()),
                        "reasoning",
                        &input,
                        effort_check(&levels, native.as_deref()),
                        || Choice {
                            effort: native.clone(),
                        },
                    );
                    assert_eq!(
                        choice.effort, native,
                        "burn {ladder:?} locked {locked} cumulative {cumulative} \
                         submitted {submitted} limit {limit}"
                    );
                    assert!(
                        matches!(source, LegSource::Live { .. }),
                        "burn {ladder:?} kept the {} leg",
                        source.as_str()
                    );
                    rows += 1;
                }
            }
        }
    }

    for (previous, current) in [
        (0u64, 0u64),
        (0, 999_999),
        (0, 1_000_000),
        (999_999, 1_000_000),
        (1_000_000, 1_999_999),
        (1_000_000, 2_000_000),
        (900_000, 2_100_000),
        (5_500_000, 5_500_001),
        (0, 4_294_967_295),
        (4_294_967_295, 4_294_967_296),
        (0, 120_000_000),
        (119_000_000, 120_000_000),
    ] {
        let native = crossed_sota_input_milestone(previous, current);
        let input = serde_json::json!({
            "op": "milestone",
            "previous": previous,
            "current": current,
        });
        let (milestone, source) = decide_with(
            Some(leg.clone()),
            "reasoning",
            &input,
            milestone_check,
            || Milestone { tokens: native },
        );
        assert_eq!(
            milestone.tokens, native,
            "milestone {previous} -> {current}"
        );
        assert!(
            matches!(source, LegSource::Live { .. }),
            "milestone {previous} -> {current} kept the {} leg",
            source.as_str()
        );
        rows += 1;
    }

    for (elapsed, budget) in [
        (0u64, 0usize),
        (9, 10),
        (10, 10),
        (11, 10),
        (99_999, 0),
        (3_600, 3_600),
        (1, 1),
        (7, 900),
    ] {
        let native = turn_expired(elapsed, budget);
        let input = serde_json::json!({
            "op": "budget",
            "elapsed": elapsed,
            "budget": budget,
        });
        let (answer, source) = decide_with(
            Some(leg.clone()),
            "reasoning",
            &input,
            |_: &Budget| Ok(()),
            || Budget { expired: native },
        );
        assert_eq!(answer.expired, native, "budget {elapsed}s of {budget}s");
        assert!(
            matches!(source, LegSource::Live { .. }),
            "budget {elapsed}s of {budget}s kept the {} leg",
            source.as_str()
        );
        rows += 1;
    }

    assert!(rows >= 30, "the parity table needs real coverage");
    eprintln!("reasoning leg parity: {rows} decisions");
}

#[test]
fn a_broken_reasoning_generation_keeps_the_native_decision() {
    let root = std::env::temp_dir().join(format!(
        "angel-reasoning-leg-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir_all(root.join("reasoning")).unwrap();
    std::fs::write(
        root.join("reasoning/module.json"),
        r#"{"api_version":1,"id":"reasoning","kind":"policy","entry":"index.js","probe":{"op":"ladder"}}"#,
    )
    .unwrap();
    std::fs::write(
        root.join("reasoning/index.js"),
        "module.exports={apiVersion:1,render:()=>({effort:'extreme'})}",
    )
    .unwrap();
    let mut registry = angel_live::Registry::validated(
        root.clone(),
        crate::agent::hotload::evaluate,
        |_, _| Ok(()),
    );
    registry.refresh(true);
    let broken = registry
        .snapshot()
        .policies
        .get("reasoning")
        .cloned()
        .expect("the broken generation must still publish");
    let ladder = ["low", "high"];
    let seat = Seat::new(&ladder, Some("high"));
    let native = resolve_adaptive_reasoning_effort(&seat, 1, false);
    let input = serde_json::json!({
        "op": "ladder",
        "levels": ladder,
        "effort": "high",
        "hop": 1,
        "friction": false,
    });
    let levels = levels_of(&ladder);
    let (choice, source) = decide_with(
        Some(broken),
        "reasoning",
        &input,
        effort_check(&levels, native.as_deref()),
        || Choice {
            effort: native.clone(),
        },
    );
    assert_eq!(choice.effort, native, "the native leg must answer");
    assert!(
        matches!(source, LegSource::LiveFallback { .. }),
        "{source:?} must be a fallback"
    );
    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn reasoning_leg_decision_latency_is_reported() {
    let leg = crate::agent::harness::live_policy::shipped_leg("reasoning");
    let input = serde_json::json!({
        "op": "ladder",
        "levels": ["none", "low", "high"],
        "effort": "high",
        "hop": 1,
        "friction": false,
    });
    let expected = serde_json::json!({"effort": "low"});
    let mut samples: Vec<f64> = Vec::new();
    for _ in 0..200 {
        let started = std::time::Instant::now();
        let decided = leg.render(&input).unwrap();
        samples.push(started.elapsed().as_secs_f64() * 1e6);
        assert_eq!(decided, expected);
    }
    samples.sort_by(f64::total_cmp);
    eprintln!(
        "reasoning leg: {:.1} µs median, {:.1} µs p95 over 200 decisions",
        samples[100], samples[190]
    );
}
