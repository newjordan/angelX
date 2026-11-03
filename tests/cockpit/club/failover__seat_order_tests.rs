use super::*;
use crate::agent::harness::live_policy::{LegSource, decide_with, shipped_leg};

/// A club fixture with the only properties the seating and the walk read: its
/// label, its live model name, its detected metadata and whether it answers.
struct SeatClub {
    label: &'static str,
    model: Option<&'static str>,
    detected: Option<Metadata>,
    answers: bool,
}

impl Club for SeatClub {
    fn respond(&self, _prompt: &str) -> Result<String, String> {
        if self.answers {
            Ok(self.label.to_string())
        } else {
            Err("endpoint down".to_string())
        }
    }

    fn label(&self) -> &str {
        self.label
    }

    fn live_model_name(&self) -> Option<String> {
        self.model.map(str::to_string)
    }

    fn metadata(&self) -> Option<Metadata> {
        self.detected
    }
}

type Link = (String, Arc<dyn Club>, Arc<AtomicBool>);

fn link(
    alias: &str,
    label: &'static str,
    model: Option<&'static str>,
    answers: bool,
    up: bool,
) -> Link {
    (
        alias.to_string(),
        Arc::new(SeatClub {
            label,
            model,
            detected: None,
            answers,
        }),
        Arc::new(AtomicBool::new(up)),
    )
}

fn detected(window: usize, supports_tools: bool) -> Metadata {
    Metadata {
        context_window: window,
        supports_tools,
        ..Metadata::default()
    }
}

fn gated(
    label: &'static str,
    detected: Option<Metadata>,
    up: bool,
    answers: bool,
) -> (Arc<dyn Club>, Option<Arc<AtomicBool>>) {
    (
        Arc::new(SeatClub {
            label,
            model: None,
            detected,
            answers,
        }),
        Some(Arc::new(AtomicBool::new(up))),
    )
}

/// Realistic link tables, including the automatic GLM seat and its built-in
/// catalog siblings that a Z.ai key seats beside it.
fn seating_tables() -> Vec<Vec<Link>> {
    let glm = || {
        vec![
            link("glm", "glm-5.3", Some("glm-5.3"), true, true),
            link("glm-5.3-flash", "glm-5.3-flash", Some("glm-5.3-flash"), true, true),
            link("glm-5.2", "glm-5.2", Some("glm-5.2"), true, true),
        ]
    };
    let breadth = || {
        vec![
            link("codex-run", "openai", None, true, true),
            link("deepseek", "deepseek-v4-pro", None, true, true),
            link("kimi", "kimi-k3", None, true, true),
            link("longcat", "LongCat-2.0", None, true, true),
            link("openrouter", "stealth/ox-alpha", None, true, true),
        ]
    };
    let mut tables = vec![
        Vec::new(),
        vec![link("glm", "glm-5.3", Some("glm-5.3"), true, true)],
        glm(),
        breadth(),
        // The configured GLM seat beside every catalog sibling and a frontier
        // link that only the full profile seats.
        {
            let mut table = glm();
            table.extend(breadth());
            table
        },
        // Unknown providers match no cheap order slot at all.
        vec![
            link("solar", "solar-pro", None, true, true),
            link("glm", "glm-5.3", Some("glm-5.3"), true, true),
        ],
        // Two links sharing one alias: the seating keeps the first.
        vec![
            link("longcat", "LongCat-2.0", None, true, true),
            link("longcat", "longcat-backup", None, true, true),
            link("deepseek", "deepseek-v4-pro", None, true, true),
        ],
        vec![
            link("LongCat", "LongCat-2.0", None, true, true),
            link("longcat", "longcat-backup", None, true, true),
        ],
        // The alias matches one order slot while the label matches another.
        vec![
            link("glm", "glm-5.3-flash", None, true, true),
            link("glm-5.3-flash", "glm-5.2", None, true, true),
        ],
        // A live model name is enough to seat a link whose alias is unrelated.
        vec![
            link("house-model", "house", Some("glm-5.2"), true, true),
            link("deepseek", "deepseek-v4-pro", None, true, true),
        ],
        // Availability does not gate the seating; only admission does.
        vec![
            link("codex-run", "openai", None, true, false),
            link("glm", "glm-5.3", None, true, true),
        ],
        // Cheap-pool order is preference order, not configuration order.
        vec![
            link("longcat", "LongCat-2.0", None, true, true),
            link("openrouter", "stealth/ox-alpha", None, true, true),
            link("deepseek", "deepseek-flash", None, true, true),
            link("luna", "chatgpt-luna-on-high", None, true, true),
        ],
        // A frontier-only link, alone.
        vec![link("codex-run", "openai", None, true, true)],
        // Both GLM flash spellings: the alias and the provider id.
        vec![
            link("glm", "glm-5.3-flash", Some("glm-5.3-flash"), true, true),
            link("glm-5.3", "glm-5.3", Some("glm-5.3"), true, true),
        ],
        // One link matched by three different cheap order names.
        vec![link("deepseek-v4-flash", "deepseek-flash", Some("deepseek-flash"), true, true)],
        // The same club offered twice under two different aliases.
        {
            let club: Arc<dyn Club> = Arc::new(SeatClub {
                label: "glm-5.3-flash",
                model: Some("glm-5.3-flash"),
                detected: None,
                answers: true,
            });
            vec![
                ("glm-flash".to_string(), Arc::clone(&club), Arc::new(AtomicBool::new(true))),
                ("glm-5.3-flash".to_string(), club, Arc::new(AtomicBool::new(true))),
            ]
        },
        // An empty-but-present alias seats nothing under that alias alone.
        vec![
            link("", "nameless", None, true, true),
            link("kimi", "kimi-k3", None, true, true),
        ],
    ];
    // Duplicate every table with each link also offered under its label, so the
    // match-by-label path is exercised across the whole corpus.
    let mut doubled = Vec::new();
    for table in &tables {
        let mut variant: Vec<Link> = table.clone();
        for (alias, club, gate) in table {
            variant.push((club.label().to_string(), Arc::clone(club), Arc::clone(gate)));
            let _ = alias;
        }
        doubled.push(variant);
    }
    tables.extend(doubled);
    tables
}

#[test]
fn seat_order_leg_reproduces_the_native_cheap_seating() {
    let leg = shipped_leg(SEAT_ORDER_LEG);
    let tables = seating_tables();
    let mut checked = 0;
    for links in &tables {
        let native = ordered_cheap_plan_native(links);
        let input = seat_order_input(links);
        let limit = links.len();
        let (plan, source) = decide_with(
            Some(leg.clone()),
            SEAT_ORDER_LEG,
            &input,
            move |plan: &SeatPlan| seat_plan_check(&plan.plan, limit, false),
            || SeatPlan {
                plan: native.clone(),
            },
        );
        assert_eq!(plan.plan, native, "seating differs for {:?}", aliases(links));
        assert!(matches!(source, LegSource::Live { .. }), "{source:?}");
        checked += 1;
    }
    assert!(checked >= 30, "only {checked} seating tables ran");
}

fn aliases(links: &[Link]) -> Vec<&str> {
    links.iter().map(|(alias, _, _)| alias.as_str()).collect()
}

/// The seating's own two rules, driven straight from a feature record: a refused
/// link is never seated, and one alias seats once however often it appears.
#[test]
fn a_refused_link_is_never_seated_and_an_alias_seats_once() {
    let leg = shipped_leg(SEAT_ORDER_LEG);
    let input = serde_json::json!({
        "mode": "order",
        "order": (0..SOTA_MOA_CHEAP_FALLBACK_ORDER.len()).collect::<Vec<usize>>(),
        "links": [
            {"index": 0, "group": 0, "allowed": false, "matches": [0]},
            {"index": 1, "group": 1, "allowed": true, "matches": [1]},
            {"index": 2, "group": 1, "allowed": true, "matches": [1]},
            {"index": 3, "group": 3, "allowed": true, "matches": [0, 1]},
        ],
    });
    let (plan, source) = decide_with(
        Some(leg),
        SEAT_ORDER_LEG,
        &input,
        |plan: &SeatPlan| seat_plan_check(&plan.plan, 4, false),
        || SeatPlan { plan: vec![3, 3, 3] },
    );
    assert!(matches!(source, LegSource::Live { .. }), "{source:?}");
    // Slot 0 seats link 3 (link 0 is refused); slot 1 then seats link 1, and
    // link 2 shares that alias group, so it never seats.
    assert_eq!(plan.plan, vec![3, 1]);
}

#[test]
fn seat_order_leg_reproduces_the_native_walk() {
    let leg = shipped_leg(SEAT_ORDER_LEG);
    let tools = vec![ToolDef {
        name: "shell".to_string(),
        description: "run a command".to_string(),
        params: serde_json::json!({"type": "object"}),
    }];
    let shapes: Vec<Vec<(Option<Metadata>, bool, bool)>> = vec![
        vec![(None, false, false)],
        vec![(None, true, true)],
        vec![(None, true, false)],
        vec![(Some(detected(131_072, true)), true, true)],
        vec![
            (Some(detected(131_072, true)), true, false),
            (None, true, true),
        ],
        vec![
            (Some(detected(131_072, true)), false, true),
            (None, true, true),
            (Some(detected(8, true)), true, true),
        ],
        vec![
            (Some(detected(0, false)), true, true),
            (Some(detected(4096, true)), true, true),
        ],
        vec![
            (Some(detected(4096, true)), true, true),
            (Some(detected(0, true)), true, true),
        ],
        vec![
            (None, false, false),
            (None, true, true),
            (Some(detected(1024, false)), true, true),
        ],
        vec![
            (Some(detected(200, true)), false, true),
            (Some(detected(0, false)), true, true),
        ],
        vec![
            (None, true, true),
            (None, true, true),
            (None, true, true),
        ],
        vec![(Some(detected(131_072, false)), true, true)],
        vec![
            (Some(detected(131_072, true)), true, true),
            (Some(detected(131_072, true)), false, true),
            (Some(detected(131_072, true)), true, true),
        ],
        vec![
            (Some(detected(0, false)), true, true),
            (Some(detected(0, false)), false, true),
        ],
        vec![
            (Some(detected(65_536, true)), true, true),
            (None, false, true),
            (Some(detected(65_536, false)), true, true),
            (None, true, true),
        ],
    ];
    let mut checked = 0;
    for shape in &shapes {
        for chat in [false, true] {
            let chain: Vec<(Arc<dyn Club>, Option<Arc<AtomicBool>>)> = shape
                .iter()
                .map(|&(metadata, up, answers)| gated("seat", metadata, up, answers))
                .collect();
            let gates = chain.iter().map(|(_, gate)| gate.clone()).collect();
            let clubs: Vec<Arc<dyn Club>> = chain.into_iter().map(|(club, _)| club).collect();
            let fallback = FallbackClub::with_gates(clubs, gates);
            let messages: Vec<ChatMsg> = Vec::new();
            let offered = if chat { &tools[..] } else { &[][..] };
            let native = fallback.walk_plan_native(chat, &messages, offered);
            let input = fallback.walk_input(chat, &messages, offered);
            let limit = fallback.chain.len();
            let (plan, source) = decide_with(
                Some(leg.clone()),
                SEAT_ORDER_LEG,
                &input,
                move |plan: &SeatPlan| seat_plan_check(&plan.plan, limit, true),
                || SeatPlan {
                    plan: native.clone(),
                },
            );
            assert_eq!(plan.plan, native, "walk differs for chat={chat} {shape:?}");
            assert!(matches!(source, LegSource::Live { .. }), "{source:?}");
            checked += 1;
        }
    }
    assert!(checked >= 30, "only {checked} walk shapes ran");
}

#[test]
fn seat_order_leg_reproduces_the_native_step_after_a_failure() {
    let leg = shipped_leg(SEAT_ORDER_LEG);
    let mut checked = 0;
    for plan_len in 1..=4usize {
        for step in 0..plan_len {
            for streaming in [false, true] {
                for emitted in [false, true] {
                    for cancelled in [false, true] {
                        let native = next_step_native(
                            step,
                            plan_len,
                            streaming,
                            emitted,
                            cancelled,
                        );
                        let input = serde_json::json!({
                            "mode": "advance",
                            "step": step,
                            "plan_len": plan_len,
                            "streaming": streaming,
                            "emitted": emitted,
                            "cancelled": cancelled,
                        });
                        let (decision, source) = decide_with(
                            Some(leg.clone()),
                            SEAT_ORDER_LEG,
                            &input,
                            move |decision: &AdvanceStep| {
                                advance_check(decision, step, plan_len)
                            },
                            || native.clone(),
                        );
                        assert_eq!(
                            (decision.proceed, decision.next, decision.abort),
                            (native.proceed, native.next, native.abort),
                            "step differs for step={step} plan_len={plan_len} \
                             streaming={streaming} emitted={emitted} cancelled={cancelled}"
                        );
                        assert!(matches!(source, LegSource::Live { .. }), "{source:?}");
                        checked += 1;
                    }
                }
            }
        }
    }
    assert!(checked >= 30, "only {checked} step cases ran");
}

fn next_step_native(
    step: usize,
    plan_len: usize,
    streaming: bool,
    emitted: bool,
    cancelled: bool,
) -> AdvanceStep {
    if streaming && (emitted || cancelled) {
        AdvanceStep { proceed: false, next: 0, abort: true }
    } else if step + 1 >= plan_len {
        AdvanceStep { proceed: false, next: 0, abort: false }
    } else {
        AdvanceStep { proceed: true, next: step + 1, abort: false }
    }
}

/// The walk itself: a shut gate is skipped, an all-down chain is still walked,
/// and a stream that already spoke keeps its own error.
#[test]
fn a_failing_link_hands_the_turn_to_the_next_seated_link() {
    let fallback = FallbackClub::with_gates(
        vec![
            Arc::new(SeatClub {
                label: "closed",
                model: None,
                detected: None,
                answers: false,
            }),
            Arc::new(SeatClub {
                label: "down",
                model: None,
                detected: None,
                answers: false,
            }),
            Arc::new(SeatClub {
                label: "answers",
                model: None,
                detected: None,
                answers: true,
            }),
        ],
        vec![
            Some(Arc::new(AtomicBool::new(false))),
            Some(Arc::new(AtomicBool::new(true))),
            Some(Arc::new(AtomicBool::new(true))),
        ],
    );
    assert_eq!(fallback.respond("hello").unwrap(), "answers");

    let all_down = FallbackClub::with_gates(
        vec![
            Arc::new(SeatClub {
                label: "first",
                model: None,
                detected: None,
                answers: true,
            }),
            Arc::new(SeatClub {
                label: "second",
                model: None,
                detected: None,
                answers: false,
            }),
        ],
        vec![
            Some(Arc::new(AtomicBool::new(false))),
            Some(Arc::new(AtomicBool::new(false))),
        ],
    );
    assert_eq!(
        all_down.respond("hello").unwrap(),
        "first",
        "the availability bits are an optimization, never a brick"
    );

    let empty = FallbackClub::new(Vec::new());
    assert_eq!(empty.respond("hello").unwrap_err(), "fallback chain is empty");
}

/// A generation that breaks the leg's contract is rejected and the native
/// seating answers instead, so a bad hot edit never blocks a turn.
#[test]
fn a_broken_seat_order_generation_keeps_the_native_seating() {
    let root = std::env::temp_dir().join(format!("angel-seat-order-leg-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir_all(root.join("seat-order")).unwrap();
    std::fs::write(
        root.join("seat-order/module.json"),
        r#"{"api_version":1,"id":"seat-order","kind":"policy","entry":"index.js","probe":{"mode":"order"}}"#,
    )
    .unwrap();
    // Names the same link twice and an index outside the offered chain.
    std::fs::write(
        root.join("seat-order/index.js"),
        "module.exports={apiVersion:1,render:()=>({plan:[9,9]})}",
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
        .get("seat-order")
        .cloned()
        .expect("the broken generation must still publish");
    let links = seating_tables()
        .into_iter()
        .find(|links| links.len() >= 4)
        .expect("a seating table");
    let native = ordered_cheap_plan_native(&links);
    let input = seat_order_input(&links);
    let limit = links.len();
    let (plan, source) = decide_with(
        Some(broken),
        SEAT_ORDER_LEG,
        &input,
        move |plan: &SeatPlan| seat_plan_check(&plan.plan, limit, false),
        || SeatPlan {
            plan: native.clone(),
        },
    );
    assert_eq!(plan.plan, native, "the native seating must answer");
    assert!(
        matches!(source, LegSource::LiveFallback { .. }),
        "{source:?}"
    );
    let _ = std::fs::remove_dir_all(&root);
}

/// Reported, not asserted: a leg evaluation is a pure in-process call.
#[test]
fn seat_order_leg_decision_latency_is_reported() {
    let leg = shipped_leg(SEAT_ORDER_LEG);
    let links = seating_tables()
        .into_iter()
        .max_by_key(|links| links.len())
        .expect("a seating table");
    let input = seat_order_input(&links);
    let limit = links.len();
    let mut samples = Vec::with_capacity(200);
    let mut answered = 0;
    for _ in 0..200 {
        let started = std::time::Instant::now();
        let (plan, source) = decide_with(
            Some(leg.clone()),
            SEAT_ORDER_LEG,
            &input,
            move |plan: &SeatPlan| seat_plan_check(&plan.plan, limit, false),
            || SeatPlan { plan: Vec::new() },
        );
        samples.push(started.elapsed().as_micros());
        if matches!(source, LegSource::Live { .. }) && !plan.plan.is_empty() {
            answered += 1;
        }
    }
    assert_eq!(answered, 200, "every measured decision answered live");
    samples.sort_unstable();
    eprintln!(
        "seat-order live decision: {} links, median {} µs, p95 {} µs over 200 calls",
        links.len(),
        samples[samples.len() / 2],
        samples[samples.len() * 95 / 100],
    );
}

/// With no live generation offered, the shipped module still seats the walk and
/// the receipt says where the answer came from.
#[test]
fn a_walk_with_no_live_generation_uses_the_shipped_seat_order_leg() {
    let fallback = FallbackClub::new(vec![
        Arc::new(SeatClub {
            label: "a",
            model: None,
            detected: None,
            answers: true,
        }),
        Arc::new(SeatClub {
            label: "b",
            model: None,
            detected: None,
            answers: true,
        }),
    ]);
    let (plan, source) = fallback.walk_plan_decided(false, &[], &[]);
    assert!(
        matches!(source, LegSource::Shipped { .. }),
        "{source:?} did not name the shipped leg"
    );
    assert_eq!(plan, fallback.walk_plan_native(false, &[], &[]));
}
