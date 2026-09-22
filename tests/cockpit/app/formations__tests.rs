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
    assert_eq!(formation_alias("math-god"), Some(FormationId::MathGod));
    assert_eq!(formation_alias("gpu-comp"), Some(FormationId::GpuComp));
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
            route_id: crate::agent::backplane::RouteId::chat(agent, model, None),
            expected_revision: crate::agent::backplane::ModelRevision::chat(model),
            metered: true,
        },
        available: true,
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
fn math_god_rosters_sol_glm_deepseek_with_grok_weigh_in() {
    let model = |agent: &str, model: &str, metered: bool| MoaModelChoice {
        route: MoaModelRef {
            agent_index: 0,
            slot_index: 0,
            agent: agent.into(),
            driver: model.into(),
            model: model.into(),
            route_id: crate::agent::backplane::RouteId::chat(agent, model, None),
            expected_revision: crate::agent::backplane::ModelRevision::chat(model),
            metered,
        },
        available: true,
    };
    let models = vec![
        model("glm", "glm-5.3", true),
        model("openai", "gpt-5.6-sol", true),
        model("grok", "grok-4.6", true),
        model("spark", "leanstral-24b", false),
        model("deepseek", "deepseek-v4-pro", true),
    ];
    let roster = FormationRoster::new(FormationId::MathGod, &models);
    let slots = formation(FormationId::MathGod).slots();
    assert_eq!(slots.len(), 6); // scout + 3 propose + judge + aggregate
    let labels: Vec<_> = roster
        .assignments()
        .iter()
        .map(|a| a.as_ref().map(|r| r.model.as_str()).unwrap_or("?"))
        .collect();
    assert_eq!(
        labels,
        vec![
            "grok-4.6",        // scout weigh-in
            "gpt-5.6-sol",     // P1 — Sol head
            "glm-5.3",         // P2 — GLM mix
            "deepseek-v4-pro", // P3 — DeepSeek mix
            "grok-4.6",        // judge weigh-in
            "gpt-5.6-sol",     // aggregate
        ]
    );
    assert!(!labels.iter().any(|label| label.contains("leanstral")));
    assert!(!labels.iter().any(|label| label.contains("flash")));
    assert_eq!(
        labels.iter().filter(|label| label.contains("sol")).count(),
        2
    );
    assert_eq!(roster.role_effort(FormationRole::Propose), Some("ultra"));
    assert_eq!(roster.role_effort(FormationRole::Aggregate), Some("ultra"));
    assert_eq!(roster.role_effort(FormationRole::Judge), Some("xhigh"));
    assert!(roster.is_ready());
}

#[test]
fn math_god_is_ready_without_leanstral_on_the_roster() {
    let model = |agent: &str, model: &str, metered: bool| MoaModelChoice {
        route: MoaModelRef {
            agent_index: 0,
            slot_index: 0,
            agent: agent.into(),
            driver: model.into(),
            model: model.into(),
            route_id: crate::agent::backplane::RouteId::chat(agent, model, None),
            expected_revision: crate::agent::backplane::ModelRevision::chat(model),
            metered,
        },
        available: true,
    };
    let models = vec![
        model("glm", "glm-5.3", true),
        model("openai", "gpt-5.6-sol", true),
        model("grok", "grok-4.6", true),
        model("deepseek", "deepseek-v4-pro", true),
    ];
    let roster = FormationRoster::new(FormationId::MathGod, &models);
    let assigned: Vec<_> = roster
        .assignments()
        .iter()
        .map(|a| a.as_ref().map(|r| r.model.clone()))
        .collect();
    assert_eq!(
        assigned,
        vec![
            Some("grok-4.6".into()),
            Some("gpt-5.6-sol".into()),
            Some("glm-5.3".into()),
            Some("deepseek-v4-pro".into()),
            Some("grok-4.6".into()),
            Some("gpt-5.6-sol".into()),
        ]
    );
    assert!(roster.is_ready());
}

#[test]
fn math_god_does_not_fill_deepseek_seat_from_intelligence_order() {
    let model = |agent: &str, model: &str, metered: bool| MoaModelChoice {
        route: MoaModelRef {
            agent_index: 0,
            slot_index: 0,
            agent: agent.into(),
            driver: model.into(),
            model: model.into(),
            route_id: crate::agent::backplane::RouteId::chat(agent, model, None),
            expected_revision: crate::agent::backplane::ModelRevision::chat(model),
            metered,
        },
        available: true,
    };
    let models = vec![
        model("glm", "glm-5.3", true),
        model("openai", "gpt-5.6-sol", true),
        model("grok", "grok-4.6", true),
        model("kimi", "kimi-k3", true),
        model("deepseek-flash", "deepseek-v4-flash", true),
    ];
    let roster = FormationRoster::new(FormationId::MathGod, &models);
    let assigned: Vec<_> = roster
        .assignments()
        .iter()
        .map(|a| a.as_ref().map(|r| r.model.clone()))
        .collect();
    assert_eq!(
        assigned,
        vec![
            Some("grok-4.6".into()),
            Some("gpt-5.6-sol".into()),
            Some("glm-5.3".into()),
            None,
            Some("grok-4.6".into()),
            Some("gpt-5.6-sol".into()),
        ]
    );
    assert!(!roster.is_ready());
}

#[test]
fn tag_team_rosters_two_fleet_boxes_with_sol_as_the_advice_seat() {
    let model = |agent: &str, model: &str, metered: bool| MoaModelChoice {
        route: MoaModelRef {
            agent_index: 0,
            slot_index: 0,
            agent: agent.into(),
            driver: model.into(),
            model: model.into(),
            route_id: crate::agent::backplane::RouteId::chat(agent, model, None),
            expected_revision: crate::agent::backplane::ModelRevision::chat(model),
            metered,
        },
        available: true,
    };
    let models = vec![
        model("turbo", "qwen3-30b-a3b", false),
        model("spark", "leanstral-24b", false),
        model("openai", "gpt-5.6-sol", true),
        model("glm", "glm-5.2", true),
    ];
    let roster = FormationRoster::new(FormationId::TagTeam, &models);
    let slots = formation(FormationId::TagTeam).slots();
    assert_eq!(slots.len(), 2, "Tag Team is two models, not the council panel");
    let labels: Vec<_> = roster
        .assignments()
        .iter()
        .map(|a| a.as_ref().map(|r| r.model.as_str()).unwrap_or("?"))
        .collect();
    assert_eq!(labels, vec!["leanstral-24b", "qwen3-30b-a3b"]);
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
fn tag_team_pins_the_dissent_gate_and_leaving_clears_it() {
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
        "ANGEL_RL_REWARD",
        "ANGEL_GPU_COMP_LOCAL_MOA",
        "GPU_COMP_TURBO_MAX_AGENTS",
        "GPU_COMP_TURBO_ROLE",
        "GPU_COMP_LEANSTRAL_ROLE",
        "GPU_COMP_DICE_ROLE",
        "GPU_COMP_GROK_ROLE",
        "GPU_COMP_OPENAI_INTERVAL_MIN",
        "GPU_COMP_MOA_PROFILE",
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

/// Draw-path effort env is seed-once. Engaging Math God must still move
/// the observed OpenAI / GLM effort after the cache has been seeded, and
/// leaving must drop the unconditional OpenAI pin.
#[test]
fn apply_sota_env_math_god_pins_live_after_effort_env_seed() {
    use crate::agent::club::{Club, HttpClub};
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
        "ANGEL_GLM_MODEL",
        "ANGEL_GLM_REASONING_EFFORT",
        "ANGEL_DEEPSEEK_MODEL",
        "ANGEL_DEEPSEEK_REASONING_EFFORT",
        "ANGEL_REASONING_EFFORT",
        "ANGEL_RL_REWARD",
        "ANGEL_GPU_COMP_LOCAL_MOA",
        "GPU_COMP_TURBO_MAX_AGENTS",
        "GPU_COMP_TURBO_ROLE",
        "GPU_COMP_LEANSTRAL_ROLE",
        "GPU_COMP_DICE_ROLE",
        "GPU_COMP_GROK_ROLE",
        "GPU_COMP_OPENAI_INTERVAL_MIN",
        "GPU_COMP_MOA_PROFILE",
    ];
    let _restore: Vec<_> = TOUCHED
        .iter()
        .map(|key| crate::tests::TestEnvGuard::unset(key))
        .collect();
    crate::agent::club::resync_reasoning_effort_env_from_env();

    let glm = HttpClub::new(
        "glm",
        "https://api.z.ai/api/coding/paas/v4",
        "glm-5.3",
        None,
    );
    let openai = HttpClub::new("openai", "https://api.openai.com/v1", "gpt-5.6-sol", None);

    assert!(openai.reasoning_effort().is_none());
    assert_eq!(glm.reasoning_effort().as_deref(), Some("low"));
    assert_eq!(
        crate::agent::club::reasoning_env_var("ANGEL_OPENAI_REASONING_EFFORT"),
        None
    );
    assert_eq!(
        crate::agent::club::reasoning_env_var("ANGEL_GROK_REASONING_EFFORT"),
        None
    );

    formation(FormationId::MathGod).apply_sota_env();
    assert_eq!(
        crate::agent::club::reasoning_env_var("ANGEL_OPENAI_REASONING_EFFORT").as_deref(),
        Some("ultra")
    );
    assert_eq!(
        crate::agent::club::reasoning_env_var("ANGEL_GROK_REASONING_EFFORT").as_deref(),
        Some("xhigh")
    );
    assert_eq!(
        openai.reasoning_effort().as_deref(),
        Some("ultra"),
        "Math God OpenAI pin must land after the draw-path cache has seeded"
    );
    assert_eq!(
        glm.reasoning_effort().as_deref(),
        Some("high"),
        "Math God GLM pin must land after the draw-path cache has seeded"
    );

    formation(FormationId::SoloStrike).apply_sota_env();
    assert_eq!(
        crate::agent::club::reasoning_env_var("ANGEL_OPENAI_REASONING_EFFORT"),
        None
    );
    assert_eq!(
        crate::agent::club::reasoning_env_var("ANGEL_GROK_REASONING_EFFORT"),
        None
    );
    assert!(openai.reasoning_effort().is_none());
    crate::agent::club::resync_reasoning_effort_env_from_env();
}

/// A tag team needs two corners. With only one fleet box online the partner
/// seat must stay empty and say so, rather than nominating the lead's own
/// box again — that renders as a pair in the deck while actually being one
/// model arguing with itself.
#[test]
fn tag_team_partner_seat_never_doubles_up_on_the_lead_box() {
    let turbo = MoaModelChoice {
        route: MoaModelRef {
            agent_index: 3,
            slot_index: 0,
            agent: "turbo".into(),
            driver: "turbo".into(),
            model: "qwen3-30b-a3b".into(),
            route_id: crate::agent::backplane::RouteId::chat("turbo", "turbo", None),
            expected_revision: crate::agent::backplane::ModelRevision::chat("qwen3-30b-a3b"),
            metered: false,
        },
        available: true,
    };
    let roster = FormationRoster::new(FormationId::TagTeam, &[turbo]);
    let assigned: Vec<_> = roster
        .assignments()
        .iter()
        .map(|a| a.as_ref().map(|r| r.model.clone()))
        .collect();
    assert_eq!(
        assigned,
        vec![
            Some("qwen3-30b-a3b".to_string()),
            None,
        ]
    );
    // With Leanstral down, turbo inherits the lead rather than the roster
    // stalling — the chain degrades, it does not pin one box.
    assert!(!roster.is_ready());
}

/// The failure this guards is silent, expensive, and easy to ship: with no
/// fleet box online the generic fallback would fill the "local" seats from
/// the intelligence order, turning a local mode into a frontier one.
#[test]
fn tag_team_leaves_local_seats_empty_rather_than_billing_a_frontier_model() {
    let frontier = MoaModelChoice {
        route: MoaModelRef {
            agent_index: 0,
            slot_index: 0,
            agent: "openai".into(),
            driver: "gpt-5.6-sol".into(),
            model: "gpt-5.6-sol".into(),
            route_id: crate::agent::backplane::RouteId::chat("openai", "gpt-5.6-sol", None),
            expected_revision: crate::agent::backplane::ModelRevision::chat("gpt-5.6-sol"),
            metered: true,
        },
        available: true,
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
            route_id: crate::agent::backplane::RouteId::chat("spark", "qwen", None),
            expected_revision: crate::agent::backplane::ModelRevision::chat("qwen-32b"),
            metered: false,
        },
        available: true,
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
            route_id: crate::agent::backplane::RouteId::chat(agent, model, None),
            expected_revision: crate::agent::backplane::ModelRevision::chat(model),
            metered: false,
        },
        available: true,
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
            route_id: crate::agent::backplane::RouteId::chat("alpha", "model-a", None),
            expected_revision: crate::agent::backplane::ModelRevision::chat("model-a"),
            metered: false,
        },
        available: true,
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
            route_id: crate::agent::backplane::RouteId::chat(agent, model, None),
            expected_revision: crate::agent::backplane::ModelRevision::chat(model),
            metered: false,
        },
        available: true,
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
