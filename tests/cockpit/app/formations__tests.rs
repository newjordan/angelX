use super::*;

#[test]
fn selection_change_starts_250ms_dissolve_and_increments_epoch() {
    let now = Instant::now();
    let mut deck = MoaDeckState::new(Vec::new());
    deck.move_selection_at(1, now);
    assert_eq!(deck.selected().id, FormationId::Recon);
    assert_eq!(
        deck.transition_at(now),
        Some((FormationId::SoloStrike, 0.0))
    );
    let halfway = now + SELECTION_DISSOLVE / 2;
    let (_, progress) = deck.transition_at(halfway).unwrap();
    assert!((progress - 0.5).abs() < 0.01);
    assert!(deck.transition_at(now + SELECTION_DISSOLVE).is_none());
}

#[test]
fn formation_alias_routes_every_formation_to_its_own_id() {
    assert_eq!(formation_alias("tag-team"), Some(FormationId::TagTeam));
    assert_eq!(formation_alias("tag team"), Some(FormationId::TagTeam));
    assert_eq!(formation_alias("council"), Some(FormationId::Council));
    assert_eq!(formation_alias("auto-moa"), Some(FormationId::AutoMoa));
    assert_eq!(formation_alias("grok-war"), Some(FormationId::GrokWar));
    assert_eq!(formation_alias("gpu-comp"), None);
    assert_eq!(
        formation_alias("solo-strike"),
        Some(FormationId::SoloStrike)
    );
    assert_eq!(formation_alias("recon"), Some(FormationId::Recon));
    assert_eq!(formation_alias("duel"), Some(FormationId::Duel));
    assert_eq!(formation_alias("all-in"), Some(FormationId::AllIn));
}

#[test]
fn selection_wrap_preserves_formation_data_behavior() {
    let mut deck = MoaDeckState::new(Vec::new());
    let original = *deck.selected();
    deck.move_selection_at(-1, Instant::now());
    // Wrapping backwards from the first card lands on the last one in the
    // built-in deck order (Auto MoA was appended last).
    assert_eq!(deck.selected().id, FormationId::AutoMoa);
    assert_eq!(original.width, formation(FormationId::SoloStrike).width);
    assert_eq!(
        original.stage_summary(),
        formation(FormationId::SoloStrike).stage_summary()
    );
}

#[test]
fn grok_war_rosters_trio_under_grok_command() {
    let model = |agent: &str, model: &str| MoaModelChoice {
        route: MoaModelRef {
            agent_index: 0,
            slot_index: 0,
            agent: agent.into(),
            driver: model.into(),
            model: model.into(),
            route_id: crate::agent::backplane::RouteId::chat(agent, model, model, None),
            expected_revision: crate::agent::backplane::ModelRevision::chat(model),
            metered: true,
        },
        available: true,
        in_hand: false,
    };
    let models = vec![
        model("glm", "glm-5.2"),
        model("deepseek", "deepseek-v4-pro"),
        model("openai", "gpt-5.6-sol"),
        model("grok", "grok-4.5"),
    ];
    let roster = FormationRoster::new(FormationId::GrokWar, &models);
    let slots = formation(FormationId::GrokWar).slots();
    assert_eq!(slots.len(), 6); // scout + 3 propose + judge + aggregate
    let labels: Vec<_> = roster
        .assignments()
        .iter()
        .map(|a| a.as_ref().map(|r| r.model.as_str()).unwrap_or("?"))
        .collect();
    assert_eq!(
        labels,
        vec![
            "grok-4.5",        // scout
            "glm-5.2",         // P1
            "deepseek-v4-pro", // P2
            "gpt-5.6-sol",     // P3
            "grok-4.5",        // judge
            "grok-4.5",        // aggregate / manager
        ]
    );
    assert!(roster.is_ready());
}

#[test]
fn tag_team_is_two_seats_the_operator_fills() {
    let model = |agent: &str, model: &str, metered: bool| MoaModelRef {
        agent_index: 0,
        slot_index: 0,
        agent: agent.into(),
        driver: model.into(),
        model: model.into(),
        route_id: crate::agent::backplane::RouteId::chat(agent, model, model, None),
        expected_revision: crate::agent::backplane::ModelRevision::chat(model),
        metered,
    };
    let models: Vec<_> = [
        model("local", "qwen3-30b-a3b", false),
        model("openai", "gpt-5.6-sol", true),
        model("glm", "glm-5.2", true),
    ]
    .into_iter()
    .map(|route| MoaModelChoice {
        route,
        available: true,
        in_hand: false,
    })
    .collect();
    let slots = formation(FormationId::TagTeam).slots();
    assert_eq!(
        slots.len(),
        2,
        "Tag Team is two models, not the council panel"
    );
    // Nothing is pre-filled: the operator picks both models.
    let mut roster = FormationRoster::new(FormationId::TagTeam, &models);
    assert_eq!(roster.assignments(), &[None, None]);
    assert!(!roster.is_ready());
    // Any two models share the workload — local, frontier, or one of each.
    assert!(roster.assign(0, models[0].route.clone()));
    assert!(roster.assign(1, models[1].route.clone()));
    assert!(roster.is_ready());
}

/// Refinement layers must follow the formation's own width: Full Muster
/// (layers=2, width=3) pins `ANGEL_SWARM_REFINE_WIDTH` to 3 — each refine
/// layer carries the full history plus every draft, so the generic 6-seat
/// default doubles the formation's per-layer input spend. Single-layer
/// formations never refine; the disengage path (SoloStrike) clears it.
#[test]
fn apply_sota_env_pins_refine_width_to_the_formation_width() {
    let _lock = crate::tests::env_lock();
    const TOUCHED: &[&str] = &[
        "ANGEL_SOTA_MOA_MODE",
        "ANGEL_SOTA_MOA_ALWAYS",
        "ANGEL_SOTA_MOA_MAX",
        "ANGEL_SOTA_MOA_WIDTH",
        "ANGEL_SOTA_MOA_MAX_WIDTH",
        "ANGEL_SOTA_MOA_LAYERS",
        "ANGEL_SOTA_MOA_SAMPLES",
        "ANGEL_SOTA_MOA_JUDGE_PANEL",
        "ANGEL_SOTA_MOA_VERIFY",
        "ANGEL_SOTA_MOA_JUDGE",
        "ANGEL_SOTA_MOA_VERIFY_GUARD",
        "ANGEL_SOTA_MOA_REFLECT",
        "ANGEL_SOTA_MOA_GROK_RESEARCH",
        "ANGEL_SOTA_MOA_RESEARCH",
        "ANGEL_SWARM_REFINE_WIDTH",
        "ANGEL_SOTA_MOA_FORMATION",
    ];
    let _restore: Vec<_> = TOUCHED
        .iter()
        .map(|key| crate::tests::TestEnvGuard::unset(key))
        .collect();
    let full_muster = formation(FormationId::AllIn);
    assert!(full_muster.layers > 1, "Full Muster refines");
    full_muster.apply_sota_env();
    assert_eq!(
        std::env::var("ANGEL_SWARM_REFINE_WIDTH"),
        Ok(full_muster.width.to_string())
    );
    assert_eq!(
        std::env::var("ANGEL_SOTA_MOA_FORMATION").as_deref(),
        Ok("Full Muster"),
        "the engaged formation names itself for the ledgers"
    );
    // Disengage: the resting formation owns no refine width and no name.
    formation(FormationId::SoloStrike).apply_sota_env();
    assert!(std::env::var_os("ANGEL_SWARM_REFINE_WIDTH").is_none());
    assert!(std::env::var_os("ANGEL_SOTA_MOA_FORMATION").is_none());
}

/// The conditional-advice posture lives in one env knob, and it must not
/// outlive the engagement that set it.
#[test]
fn tag_team_clears_the_dissent_gate_and_council_keeps_it_off() {
    let _lock = crate::tests::env_lock();
    // `apply_sota_env` writes the whole MoA knob set as a side effect, and
    // tests that never take this lock still read those knobs. Snapshot every
    // key either formation can touch so this test restores the process
    // environment exactly as it found it.
    const TOUCHED: &[&str] = &[
        "ANGEL_SOTA_MOA_MODE",
        "ANGEL_SOTA_MOA_ALWAYS",
        "ANGEL_SOTA_MOA_MAX",
        "ANGEL_SOTA_MOA_WIDTH",
        "ANGEL_SOTA_MOA_MAX_WIDTH",
        "ANGEL_SOTA_MOA_LAYERS",
        "ANGEL_SOTA_MOA_SAMPLES",
        "ANGEL_SOTA_MOA_JUDGE_PANEL",
        "ANGEL_SOTA_MOA_VERIFY",
        "ANGEL_SOTA_MOA_JUDGE",
        "ANGEL_SOTA_MOA_VERIFY_GUARD",
        "ANGEL_SOTA_MOA_REFLECT",
        "ANGEL_SOTA_MOA_GROK_RESEARCH",
        "ANGEL_SOTA_MOA_RESEARCH",
        "ANGEL_SWARM_REFINE_WIDTH",
        "ANGEL_SOTA_MOA_FORMATION",
        "ANGEL_SOTA_MOA_DISSENT_GATE",
        "ANGEL_SOTA_MOA_ALLOW_DEGRADED",
        "ANGEL_SOTA_MOA_PROPOSE_CLUB",
        "ANGEL_SOTA_MOA_EXTRA_PROPOSERS",
        "ANGEL_SOTA_MOA_JUDGE_CLUB",
        "ANGEL_SOTA_MOA_AGG_CLUB",
        "ANGEL_SOTA_MOA_VERIFY_CLUB",
        "ANGEL_OPENAI_MODEL",
        "ANGEL_OPENAI_REASONING_EFFORT",
    ];
    let _restore: Vec<_> = TOUCHED
        .iter()
        .map(|key| crate::tests::TestEnvGuard::unset(key))
        .collect();

    formation(FormationId::TagTeam).apply_sota_env();
    assert!(
        std::env::var_os("ANGEL_SOTA_MOA_DISSENT_GATE").is_none(),
        "dissent escalation would turn Tag Team into the council judge"
    );
    assert_eq!(
        std::env::var("ANGEL_SOTA_MOA_ALLOW_DEGRADED").as_deref(),
        Ok("1")
    );
    assert!(
        std::env::var_os("ANGEL_OPENAI_MODEL").is_none(),
        "Tag Team must not pin Sol"
    );
    // A frontier formation must get its strict quorum back.
    formation(FormationId::Council).apply_sota_env();
    assert!(std::env::var_os("ANGEL_SOTA_MOA_DISSENT_GATE").is_none());
    assert!(std::env::var_os("ANGEL_SOTA_MOA_ALLOW_DEGRADED").is_none());
}

/// The failure this guards is silent, expensive, and easy to ship: the generic
/// fallback would fill Tag Team's seats from the intelligence order, billing a
/// frontier model the operator never picked.
#[test]
fn tag_team_never_fills_a_seat_with_a_frontier_model_on_its_own() {
    let frontier = MoaModelChoice {
        route: MoaModelRef {
            agent_index: 0,
            slot_index: 0,
            agent: "openai".into(),
            driver: "gpt-5.6-sol".into(),
            model: "gpt-5.6-sol".into(),
            route_id: crate::agent::backplane::RouteId::chat(
                "openai",
                "gpt-5.6-sol",
                "gpt-5.6-sol",
                None,
            ),
            expected_revision: crate::agent::backplane::ModelRevision::chat("gpt-5.6-sol"),
            metered: true,
        },
        available: true,
        in_hand: false,
    };
    let roster = FormationRoster::new(FormationId::TagTeam, &[frontier]);
    let assigned: Vec<_> = roster
        .assignments()
        .iter()
        .map(|a| a.as_ref().map(|r| r.model.clone()))
        .collect();
    // No frontier model may occupy a tag-team seat.
    assert_eq!(assigned, vec![None, None]);
    assert!(!roster.is_ready());
}

#[test]
fn all_in_rosters_every_adaptive_execution_slot() {
    let formation = *formation(FormationId::AllIn);
    let slots = formation.slots();
    assert_eq!(
        slots
            .iter()
            .filter(|slot| slot.role == FormationRole::Propose)
            .count(),
        formation.max_width
    );
    assert_eq!(
        slots
            .iter()
            .filter(|slot| slot.role == FormationRole::Aggregate)
            .count(),
        formation.samples
    );
    assert!(slots.iter().any(|slot| slot.reserve));
}

#[test]
fn roster_requires_every_slot_and_assigns_per_slot() {
    let model = MoaModelChoice {
        route: MoaModelRef {
            agent_index: 1,
            slot_index: 2,
            agent: "spark".into(),
            driver: "qwen".into(),
            model: "qwen-32b".into(),
            route_id: crate::agent::backplane::RouteId::chat("spark", "qwen", "qwen-32b", None),
            expected_revision: crate::agent::backplane::ModelRevision::chat("qwen-32b"),
            metered: false,
        },
        available: true,
        in_hand: false,
    };
    let roster = FormationRoster::new(FormationId::Council, &[model]);
    assert!(roster.is_ready());
    assert_eq!(
        roster.assigned_count(),
        formation(FormationId::Council).slots().len()
    );
    assert_eq!(roster.local_count(), roster.slot_count());
}

#[test]
fn think_picker_stages_and_clears_the_focused_seats_role_effort() {
    let model = |agent_index, agent: &str, model: &str| MoaModelChoice {
        route: MoaModelRef {
            agent_index,
            slot_index: 0,
            agent: agent.into(),
            driver: model.into(),
            model: model.into(),
            route_id: crate::agent::backplane::RouteId::chat(agent, model, model, None),
            expected_revision: crate::agent::backplane::ModelRevision::chat(model),
            metered: false,
        },
        available: true,
        in_hand: false,
    };
    let mut deck = MoaDeckState::new(vec![
        model(0, "alpha", "model-a"),
        model(1, "beta", "model-b"),
    ]);
    deck.select(FormationId::Duel);
    assert!(deck.select_slot(2)); // Duel graph order: P1 P2 J1 A1
    let ladder = || vec!["low".to_string(), "medium".to_string(), "high".to_string()];
    assert!(deck.open_effort_picker(ladder()));
    assert_eq!(deck.focus(), MoaDeckFocus::Efforts);
    // Nothing staged yet: the cursor rests on the env-default row.
    assert_eq!(deck.selected_effort_index(), 3);
    deck.move_effort(-1); // wraps up onto "high"
    assert_eq!(deck.selected_effort_index(), 2);
    assert_eq!(
        deck.stage_selected_effort(),
        Some((FormationRole::Judge, Some("high".to_string())))
    );
    assert_eq!(deck.focus(), MoaDeckFocus::Slots);
    assert_eq!(
        deck.selected_roster().role_effort(FormationRole::Judge),
        Some("high")
    );
    // The engagement snapshot carries the staged THINK column.
    let engagement = deck.engagement().expect("duel roster is ready");
    assert_eq!(
        engagement.roster.role_effort(FormationRole::Judge),
        Some("high")
    );
    // Reopening lands the cursor on the staged level; the env-default row
    // clears the seat back to env policy.
    assert!(deck.open_effort_picker(ladder()));
    assert_eq!(deck.selected_effort_index(), 2);
    deck.move_effort(1);
    assert_eq!(
        deck.stage_selected_effort(),
        Some((FormationRole::Judge, None))
    );
    assert!(
        deck.selected_roster()
            .role_effort(FormationRole::Judge)
            .is_none()
    );
}

/// The verification chain of the roster THINK column: stage an effort on a
/// seat through the picker, activate the roster on a real Bag, and the
/// staged value is exactly what [`FormationRoster::seat_efforts`] hands the
/// engaged wrapper (Bag activation consumes this same roster surface).
#[test]
fn staged_think_effort_rides_activation_into_seat_efforts() {
    let _lock = crate::tests::env_lock();
    let mut bag = crate::agent::club::Bag::for_reasoning_render_test();
    let mut deck = MoaDeckState::new(bag.moa_model_choices());
    deck.select(FormationId::Duel);
    assert!(deck.select_slot(2)); // J1
    let sol = deck
        .models()
        .iter()
        .position(|choice| choice.route.model == "gpt-5.6-sol")
        .expect("ladder-declaring route");
    assert!(deck.assign_model(sol));
    // The ladder exactly as the deck opener resolves it from the Bag.
    let route = deck
        .selected_roster()
        .assignment(2)
        .expect("J1 assigned")
        .clone();
    let levels = bag
        .route_choices()
        .iter()
        .find(|choice| {
            choice.agent_index == route.agent_index && choice.slot_index == route.slot_index
        })
        .map(|choice| choice.reasoning_levels.to_vec())
        .expect("route row in the deck");
    assert_eq!(levels, vec!["low", "medium", "high"]);
    assert!(deck.open_effort_picker(levels));
    deck.move_effort(-1); // env-default row -> "high"
    assert_eq!(
        deck.stage_selected_effort(),
        Some((FormationRole::Judge, Some("high".to_string())))
    );
    let engagement = deck.engagement().expect("duel roster is ready");
    bag.activate_sota_moa_with_roster(&engagement.roster)
        .expect("staged roster activates");
    assert_eq!(bag.in_hand_label(), "sota");
    let seats = engagement.roster.seat_efforts();
    assert_eq!(seats.judge.as_deref(), Some("high"));
    assert!(seats.propose.is_none());
    assert!(seats.verify.is_none());
    assert!(seats.aggregate.is_none());
}

/// Capability truth: a ladder-less route opens the picker with no invented
/// levels — only the env-default clear row remains actionable.
#[test]
fn think_picker_without_a_ladder_offers_only_the_env_default_row() {
    let model = MoaModelChoice {
        route: MoaModelRef {
            agent_index: 0,
            slot_index: 0,
            agent: "alpha".into(),
            driver: "model-a".into(),
            model: "model-a".into(),
            route_id: crate::agent::backplane::RouteId::chat("alpha", "model-a", "model-a", None),
            expected_revision: crate::agent::backplane::ModelRevision::chat("model-a"),
            metered: false,
        },
        available: true,
        in_hand: false,
    };
    let mut deck = MoaDeckState::new(vec![model]);
    deck.select(FormationId::Duel);
    assert!(deck.select_slot(0));
    assert!(deck.open_effort_picker(Vec::new()));
    assert!(deck.effort_options().is_empty());
    assert_eq!(deck.selected_effort_index(), 0); // the env-default row
    assert_eq!(
        deck.stage_selected_effort(),
        Some((FormationRole::Propose, None))
    );
}

/// The swarm carries no scout seat effort, so the picker refuses the seat
/// instead of staging a value activation would silently drop.
#[test]
fn think_picker_refuses_the_scout_seat() {
    let mut deck = MoaDeckState::new(Vec::new());
    deck.select(FormationId::Recon);
    assert!(deck.select_slot(0)); // Recon's S1
    assert!(!deck.open_effort_picker(vec!["low".to_string()]));
    assert_ne!(deck.focus(), MoaDeckFocus::Efforts);
    assert!(deck.stage_effort(0).is_none());
}

#[test]
fn deck_assignment_changes_only_the_focused_slot() {
    let model = |agent_index, agent: &str, model: &str| MoaModelChoice {
        route: MoaModelRef {
            agent_index,
            slot_index: 0,
            agent: agent.into(),
            driver: model.into(),
            model: model.into(),
            route_id: crate::agent::backplane::RouteId::chat(agent, model, model, None),
            expected_revision: crate::agent::backplane::ModelRevision::chat(model),
            metered: false,
        },
        available: true,
        in_hand: false,
    };
    let mut deck = MoaDeckState::new(vec![
        model(0, "alpha", "model-a"),
        model(1, "beta", "model-b"),
    ]);
    deck.select(FormationId::Duel);
    assert!(deck.select_slot(0));
    assert!(deck.assign_model(1));
    assert_eq!(
        deck.selected_roster()
            .assignment(0)
            .map(|route| route.model.as_str()),
        Some("model-b")
    );
    assert_eq!(
        deck.selected_roster()
            .assignment(1)
            .map(|route| route.model.as_str()),
        Some("model-a")
    );
}

#[test]
fn self_seats_fill_every_formation_with_the_model_in_hand() {
    let _env = crate::tests::env_lock();
    let model = |agent: &str, model: &str, in_hand: bool| MoaModelChoice {
        route: MoaModelRef {
            agent_index: 0,
            slot_index: 0,
            agent: agent.into(),
            driver: model.into(),
            model: model.into(),
            route_id: crate::agent::backplane::RouteId::chat(agent, model, model, None),
            expected_revision: crate::agent::backplane::ModelRevision::chat(model),
            metered: true,
        },
        available: true,
        in_hand,
    };
    let models = vec![
        model("glm", "glm-5.2", false),
        model("deepseek", "deepseek-v4-pro", false),
        model("openai", "gpt-6.1-sol", true),
        model("grok", "grok-4.5", false),
    ];
    let seated = |formation| {
        FormationRoster::new(formation, &models)
            .assignments()
            .iter()
            .map(|a| a.as_ref().map_or("?".to_string(), |r| r.model.clone()))
            .collect::<Vec<_>>()
    };
    for formation in [
        FormationId::GrokWar,
        FormationId::Council,
        FormationId::AutoMoa,
    ] {
        let seats = seated(formation);
        assert!(!seats.is_empty());
        assert!(
            seats.iter().all(|m| m == "gpt-6.1-sol"),
            "{formation:?}: {seats:?}"
        );
    }
    // Tag Team still waits for the operator's two picks.
    assert!(seated(FormationId::TagTeam).iter().all(|m| m == "?"));
    // `mixed` brings back the intelligence-order picks.
    let _mixed = crate::tests::TestEnvGuard::set("ANGEL_MOA_SEATS", "mixed");
    assert_eq!(seated(FormationId::GrokWar)[1], "glm-5.2");
}

fn preference_test_models() -> Vec<MoaModelChoice> {
    [
        "codex-run",
        "openai",
        "kimi",
        "deepseek",
        "grok",
        "cerebras",
    ]
    .into_iter()
    .enumerate()
    .map(|(i, name)| MoaModelChoice {
        route: MoaModelRef {
            agent_index: i,
            slot_index: 0,
            agent: name.into(),
            driver: name.into(),
            model: name.into(),
            route_id: crate::agent::backplane::RouteId::chat(name, name, name, None),
            expected_revision: crate::agent::backplane::ModelRevision::chat(name),
            metered: false,
        },
        available: true,
        in_hand: false,
    })
    .collect()
}

#[test]
fn roster_keeps_one_preference_generation_after_reload() {
    let dir = std::env::temp_dir().join(format!(
        "angel-moa-roster-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("preferences.json");
    let baseline = include_str!("../../../lib/club/moa-preferences.json");
    std::fs::write(&path, baseline).unwrap();
    let pinned = crate::agent::club::test_preferences(&path);
    let models = preference_test_models();
    let before = FormationRoster::new_with_preferences(FormationId::AutoMoa, &models, &pinned);
    let mut changed: serde_json::Value = serde_json::from_str(baseline).unwrap();
    changed["standard"].as_array_mut().unwrap().reverse();
    std::fs::write(&path, serde_json::to_vec(&changed).unwrap()).unwrap();
    let updated = crate::agent::club::test_preferences(&path);
    let again = FormationRoster::new_with_preferences(FormationId::AutoMoa, &models, &pinned);
    let next = FormationRoster::new_with_preferences(FormationId::AutoMoa, &models, &updated);
    assert_eq!(before, again);
    assert_ne!(before, next);
    std::fs::remove_file(path).unwrap();
    std::fs::remove_dir(dir).unwrap();
}

// --- a formation is an overlay on the operator's environment ------------------

/// Every env knob a formation can write.
const FORMATION_KNOBS: &[&str] = &[
    "ANGEL_SOTA_MOA_FORMATION",
    "ANGEL_SOTA_MOA_MODE",
    "ANGEL_SOTA_MOA_ALWAYS",
    "ANGEL_SOTA_MOA_MAX",
    "ANGEL_SOTA_MOA_WIDTH",
    "ANGEL_SOTA_MOA_MAX_WIDTH",
    "ANGEL_SOTA_MOA_LAYERS",
    "ANGEL_SWARM_REFINE_WIDTH",
    "ANGEL_SOTA_MOA_SAMPLES",
    "ANGEL_SOTA_MOA_JUDGE_PANEL",
    "ANGEL_SOTA_MOA_VERIFY",
    "ANGEL_SOTA_MOA_JUDGE",
    "ANGEL_SOTA_MOA_VERIFY_GUARD",
    "ANGEL_SOTA_MOA_REFLECT",
    "ANGEL_SOTA_MOA_GROK_RESEARCH",
    "ANGEL_SOTA_MOA_RESEARCH",
    "ANGEL_SOTA_MOA_DISSENT_GATE",
    "ANGEL_SOTA_MOA_ALLOW_DEGRADED",
    "ANGEL_SOTA_MOA_PROPOSE_CLUB",
    "ANGEL_SOTA_MOA_EXTRA_PROPOSERS",
    "ANGEL_SOTA_MOA_JUDGE_CLUB",
    "ANGEL_SOTA_MOA_AGG_CLUB",
    "ANGEL_SOTA_MOA_VERIFY_CLUB",
    "ANGEL_SOTA_MOA_COST_PROFILE",
    "ANGEL_GROK_TOOL",
    "ANGEL_GROK_REASONING_EFFORT",
    "ANGEL_OPENAI_MODEL",
    "ANGEL_OPENAI_REASONING_EFFORT",
    "ANGEL_LANE",
    "ANGEL_TASK_TREEBEARD",
    "ANGEL_TRAJECTORY_LOG",
];

/// Hand the process an operator environment: the listed knobs set, every other
/// formation knob unset. Dropping it puts the process back as it was and empties
/// the overlay, so one test's engagement never hands values back into another's.
struct OperatorEnv {
    _guards: Vec<crate::tests::TestEnvGuard>,
}

impl Drop for OperatorEnv {
    fn drop(&mut self) {
        forget_written_env();
    }
}

fn operator_env(operator: &[(&'static str, &str)]) -> OperatorEnv {
    forget_written_env();
    let guards = FORMATION_KNOBS
        .iter()
        .map(|key| match operator.iter().find(|(name, _)| name == key) {
            Some((_, value)) => crate::tests::TestEnvGuard::set(key, value),
            None => crate::tests::TestEnvGuard::unset(key),
        })
        .collect();
    OperatorEnv { _guards: guards }
}

fn knob(key: &str) -> Option<String> {
    std::env::var(key).ok()
}

/// Grok War leaves the cheap profile so its metered seats can fire. The profile
/// is the operator's, so it comes back when the war ends.
#[test]
fn grok_war_gives_the_operators_cost_profile_back() {
    let _lock = crate::tests::env_lock();
    let _env = operator_env(&[("ANGEL_SOTA_MOA_COST_PROFILE", "cheap")]);

    formation(FormationId::GrokWar).apply_sota_env();
    assert_eq!(knob("ANGEL_SOTA_MOA_COST_PROFILE"), None);
    formation(FormationId::Council).apply_sota_env();
    assert_eq!(
        knob("ANGEL_SOTA_MOA_COST_PROFILE").as_deref(),
        Some("cheap")
    );
}

/// The `high` Grok effort is the war's, not the operator's: it goes with the
/// war. An effort the operator set stays theirs the whole way through.
#[test]
fn grok_war_effort_pin_ends_with_the_war() {
    let _lock = crate::tests::env_lock();
    let _env = operator_env(&[]);

    formation(FormationId::GrokWar).apply_sota_env();
    assert_eq!(knob("ANGEL_GROK_REASONING_EFFORT").as_deref(), Some("high"));
    formation(FormationId::SoloStrike).apply_sota_env();
    assert_eq!(knob("ANGEL_GROK_REASONING_EFFORT"), None);
}

#[test]
fn an_effort_the_operator_set_survives_the_war() {
    let _lock = crate::tests::env_lock();
    let _env = operator_env(&[("ANGEL_GROK_REASONING_EFFORT", "xhigh")]);

    formation(FormationId::GrokWar).apply_sota_env();
    assert_eq!(
        knob("ANGEL_GROK_REASONING_EFFORT").as_deref(),
        Some("xhigh")
    );
    formation(FormationId::SoloStrike).apply_sota_env();
    assert_eq!(
        knob("ANGEL_GROK_REASONING_EFFORT").as_deref(),
        Some("xhigh")
    );
}

/// Every pin the war adds goes with it, and every setting it overrode comes
/// back: the Sol model and effort, the Grok tool switch, trajectory logging and
/// the lane.
#[test]
fn leaving_grok_war_restores_every_knob_it_wrote() {
    let _lock = crate::tests::env_lock();
    let _env = operator_env(&[
        ("ANGEL_OPENAI_MODEL", "operator-model"),
        ("ANGEL_OPENAI_REASONING_EFFORT", "xhigh"),
        ("ANGEL_GROK_TOOL", "0"),
        ("ANGEL_SOTA_MOA_COST_PROFILE", "cheap"),
    ]);

    formation(FormationId::GrokWar).apply_sota_env();
    assert_eq!(knob("ANGEL_OPENAI_MODEL").as_deref(), Some("gpt-5.6-sol"));
    assert_eq!(
        knob("ANGEL_OPENAI_REASONING_EFFORT").as_deref(),
        Some("max")
    );
    assert_eq!(knob("ANGEL_GROK_TOOL").as_deref(), Some("1"));
    assert_eq!(knob("ANGEL_TRAJECTORY_LOG").as_deref(), Some("1"));
    assert_eq!(knob("ANGEL_LANE").as_deref(), Some("treebeard"));

    formation(FormationId::Council).apply_sota_env();
    assert_eq!(
        knob("ANGEL_OPENAI_MODEL").as_deref(),
        Some("operator-model")
    );
    assert_eq!(
        knob("ANGEL_OPENAI_REASONING_EFFORT").as_deref(),
        Some("xhigh")
    );
    assert_eq!(knob("ANGEL_GROK_TOOL").as_deref(), Some("0"));
    assert_eq!(
        knob("ANGEL_SOTA_MOA_COST_PROFILE").as_deref(),
        Some("cheap")
    );
    assert_eq!(knob("ANGEL_TRAJECTORY_LOG"), None);
    assert_eq!(knob("ANGEL_LANE"), None);
    for pin in [
        "ANGEL_SOTA_MOA_PROPOSE_CLUB",
        "ANGEL_SOTA_MOA_EXTRA_PROPOSERS",
        "ANGEL_SOTA_MOA_JUDGE_CLUB",
        "ANGEL_SOTA_MOA_AGG_CLUB",
        "ANGEL_SOTA_MOA_VERIFY_CLUB",
    ] {
        assert_eq!(knob(pin), None, "{pin}");
    }
}

/// The cockpit applies the staged formation every turn and again on every
/// failure path. A repeat must not mistake its own earlier write for the
/// operator's setting.
#[test]
fn applying_the_same_formation_again_still_restores_the_operators_env() {
    let _lock = crate::tests::env_lock();
    let _env = operator_env(&[("ANGEL_SOTA_MOA_COST_PROFILE", "cheap")]);

    for _ in 0..3 {
        formation(FormationId::GrokWar).apply_sota_env();
    }
    assert_eq!(knob("ANGEL_GROK_REASONING_EFFORT").as_deref(), Some("high"));
    assert_eq!(knob("ANGEL_SOTA_MOA_COST_PROFILE"), None);
    formation(FormationId::SoloStrike).apply_sota_env();
    assert_eq!(knob("ANGEL_GROK_REASONING_EFFORT"), None);
    assert_eq!(
        knob("ANGEL_SOTA_MOA_COST_PROFILE").as_deref(),
        Some("cheap")
    );
}

/// Restoring runs only for a knob this module wrote and nobody has changed
/// since: an effort picked mid-war is the operator's new choice.
#[test]
fn a_knob_changed_during_the_war_is_not_overwritten_on_exit() {
    let _lock = crate::tests::env_lock();
    let _env = operator_env(&[]);

    formation(FormationId::GrokWar).apply_sota_env();
    // TODO: Audit that the environment access only happens in single-threaded code.
    unsafe { std::env::set_var("ANGEL_GROK_REASONING_EFFORT", "low") };
    formation(FormationId::SoloStrike).apply_sota_env();
    assert_eq!(knob("ANGEL_GROK_REASONING_EFFORT").as_deref(), Some("low"));
}

/// Tag Team drops the operator's Sol pin while it runs ("do not pin Sol") and
/// gives it back when it ends.
#[test]
fn tag_team_hands_the_operators_openai_pin_back_on_exit() {
    let _lock = crate::tests::env_lock();
    let _env = operator_env(&[
        ("ANGEL_OPENAI_MODEL", "operator-model"),
        ("ANGEL_SOTA_MOA_DISSENT_GATE", "1"),
    ]);

    formation(FormationId::TagTeam).apply_sota_env();
    assert_eq!(knob("ANGEL_OPENAI_MODEL"), None);
    formation(FormationId::Council).apply_sota_env();
    assert_eq!(
        knob("ANGEL_OPENAI_MODEL").as_deref(),
        Some("operator-model")
    );
    // The quorum pins are deliberately not handed back: a frontier formation
    // keeps its strict quorum even when the operator's env carried them.
    assert_eq!(knob("ANGEL_SOTA_MOA_ALLOW_DEGRADED"), None);
    assert_eq!(knob("ANGEL_SOTA_MOA_DISSENT_GATE"), None);
}
