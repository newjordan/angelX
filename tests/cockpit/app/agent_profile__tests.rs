use super::*;

#[test]
fn profile_aliases_pick_expected_agents() {
    // Machines are not personas: a host label with no model family is the
    // standby agent, whichever box it names.
    for label in ["turbo", "atlas", "spark", "apollo", "gpu-comp", "local"] {
        assert_eq!(profile_for(label).key, AgentKey::Unknown, "{label}");
    }
    assert_eq!(profile_for(" Codex ").key, AgentKey::Codex);
    assert_eq!(profile_for("practice").name, "Practice");
}

#[test]
fn route_profiles_distinguish_provider_models_on_one_logical_agent() {
    let openai = profile_for_route("sota", "openai", Some("gpt-5.6-sol"));
    let deepseek = profile_for_route("sota", "deepseek", Some("deepseek-v4-pro"));
    let grok = profile_for_route("sota", "xai", Some("grok-4.6"));
    let kimi = profile_for_route("sota", "kimi", Some("kimi-k3"));

    assert_eq!(openai.key, AgentKey::Codex);
    assert_eq!(deepseek.key, AgentKey::DeepSeek);
    assert_eq!(grok.key, AgentKey::Grok);
    assert_eq!(kimi.key, AgentKey::Kimi);
    assert_eq!(
        profile_for_route("sota", "openai", Some("GPT-5.6-SOL")).key,
        AgentKey::Codex
    );
    assert_eq!(
        profile_for_route("sota", "openai-api", Some("deepseek-v4-pro")).key,
        AgentKey::DeepSeek,
        "the served model family wins over a generic OpenAI-compatible driver"
    );
    assert_eq!(
        profile_for_route("sota", "openai-api", Some("openai/gpt-5.6-luna")).key,
        AgentKey::Luna,
        "an explicitly provider-qualified model keeps its authored family"
    );
    assert_ne!(openai.asset(false), deepseek.asset(false));
    assert_ne!(deepseek.asset(false), grok.asset(false));
    let src = include_str!("../../../cockpit/src/ui/agent_panel/profile.rs");
    let start = src
        .find("fn route_profile_for(")
        .expect("route_profile_for");
    let body = src[start..]
        .split("fn base_profile_for(")
        .next()
        .expect("route_profile_for body");
    assert!(
        !body.contains("to_ascii_lowercase"),
        "route portrait matching must not lowercase driver/model: {body}"
    );
}

#[test]
fn every_model_family_wears_its_own_knight() {
    let route = |driver: &str, model: &str| profile_for_route("sota", driver, Some(model)).key;
    assert_eq!(route("openai", "gpt-5.6-luna"), AgentKey::Luna);
    assert_eq!(route("openai", "gpt-6-astra"), AgentKey::Astra);
    assert_eq!(route("openai-api", "openai/gpt-5.6-luna"), AgentKey::Luna);
    assert_eq!(route("openai-api", "openai/gpt-6-astra"), AgentKey::Astra);
    assert_eq!(route("openai", "gpt-5.6-sol"), AgentKey::Codex);
    assert_eq!(route("xai", "grok-4.7"), AgentKey::Grok);
    assert_eq!(route("deepseek", "deepseek-v4-pro"), AgentKey::DeepSeek);
    assert_eq!(route("glm", "glm-5.3"), AgentKey::Glm);
    assert_eq!(route("kimi", "kimi-k3"), AgentKey::Kimi);
    assert_eq!(route("qwen", "qwen3.7-plus"), AgentKey::Qwen);
    assert_eq!(route("longcat", "LongCat-Flash"), AgentKey::LongCat);
    assert_eq!(route("meta", "muse-spark"), AgentKey::Muse);
    assert_eq!(route("hy", "tencent/hy3-preview"), AgentKey::Hy);
    assert_eq!(
        route("openrouter", "nvidia/nemotron-3-super:free"),
        AgentKey::Nemotron
    );
    assert_eq!(
        route("openrouter", "google/gemma-4-31b-it"),
        AgentKey::Gemma
    );
    assert_eq!(
        route("openrouter", "thinkingmachines/inkling"),
        AgentKey::Inkling
    );
    assert_eq!(
        route("openrouter", "poolside/laguna-s-2.1"),
        AgentKey::Laguna
    );
    assert_eq!(
        route("openrouter", "cohere/north-mini-code:free"),
        AgentKey::North
    );
    // Providers serve families; they are not characters themselves.
    assert_eq!(route("cerebras", "zai-glm-4.7"), AgentKey::Glm);
    assert_eq!(route("openrouter", "moonshotai/kimi-k3"), AgentKey::Kimi);
    assert_eq!(route("cerebras", "cerebras-native"), AgentKey::Unknown);
    assert_eq!(
        route("openrouter", "some-lab/model:free"),
        AgentKey::Unknown
    );
    let sheets: std::collections::HashSet<_> = [
        AgentKey::Codex,
        AgentKey::Luna,
        AgentKey::Astra,
        AgentKey::Grok,
        AgentKey::DeepSeek,
        AgentKey::Glm,
        AgentKey::Kimi,
        AgentKey::Qwen,
        AgentKey::LongCat,
        AgentKey::Muse,
        AgentKey::Hy,
        AgentKey::Nemotron,
        AgentKey::Gemma,
        AgentKey::Inkling,
        AgentKey::Laguna,
        AgentKey::North,
    ]
    .into_iter()
    .map(crate::ui::helm::sheet)
    .collect();
    assert_eq!(sheets.len(), 16, "each seat has its own sheet");
}

#[test]
fn local_slots_wear_the_served_models_knight_and_the_swarm_keeps_its_runner() {
    assert_eq!(
        profile_for_route("local", "local", Some("qwen3.6-27b")).key,
        AgentKey::Qwen
    );
    assert_eq!(
        profile_for_route("local", "local", Some("mystery-7b")).key,
        AgentKey::Unknown
    );
    assert_eq!(
        profile_for_route("local", "swarm", Some("qwen3.6-27b")).key,
        AgentKey::Unknown
    );
    assert_eq!(
        profile_for_route("local", "local", Some("deepseek-v4-flash")).key,
        AgentKey::DeepSeek
    );
}

#[test]
fn the_served_model_wins_over_the_machine_that_serves_it() {
    assert_eq!(
        profile_for_route("turbo", "deepseek", Some("deepseek-v4-pro")).key,
        AgentKey::DeepSeek
    );
    assert_eq!(
        profile_for_route("spark", "openai", Some("gpt-5.6-sol")).key,
        AgentKey::Codex
    );
    assert_eq!(
        profile_for_route("apollo", "xai", Some("grok-4.6")).key,
        AgentKey::Grok
    );
    assert_eq!(
        profile_for_route("sota", "openai-api", Some("mystery-model")).key,
        AgentKey::Codex,
        "an unknown model retains the original OpenAI-compatible driver fallback"
    );
    // A machine serving a model with no family shows no machine persona.
    for machine in ["turbo", "atlas", "spark", "apollo"] {
        assert_eq!(
            profile_for_route(machine, machine, Some("mystery-7b")).key,
            AgentKey::Unknown,
            "{machine}"
        );
    }
}

#[test]
fn codex_agent_uses_distinct_standard_and_high_effort_masters() {
    for label in ["openai", "codex", "GPT", " OpenAI "] {
        let p = profile_for(label);
        assert_eq!(p.key, AgentKey::Codex, "label {label:?}");
    }
    let p = profile_for("openai");
    assert_eq!(p.name, "Codex");
    assert_eq!(p.asset(false), Some("assets/agents/codex-champion.png"));
    assert_eq!(p.asset(true), Some("assets/agents/codex-champion-high.png"));
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let neutral = std::fs::read(root.join(p.asset(false).unwrap())).unwrap();
    assert!(neutral.starts_with(b"\x89PNG\r\n\x1a\n"));
}

#[test]
fn every_supported_agent_has_exactly_two_square_decodable_masters() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let route = |driver: &str, model: &str| profile_for_route("sota", driver, Some(model));
    for (label, profile) in [
        ("codex", profile_for("codex")),
        ("grok", route("xai", "grok-4.7")),
        ("deepseek", route("deepseek", "deepseek-v4-pro")),
        ("glm", route("glm", "glm-5.3")),
        ("standby", profile_for("local")),
    ] {
        let standard = profile.asset(false).unwrap();
        let high = profile.asset(true).unwrap();
        assert_ne!(standard, high, "{label} must have two distinct states");
        assert!(standard.contains("-champion.png"), "{label}: {standard}");
        assert!(high.contains("-champion-high.png"), "{label}: {high}");
        for asset in [standard, high] {
            let image = image::ImageReader::open(root.join(asset))
                .unwrap_or_else(|error| panic!("{label}: could not open {asset}: {error}"))
                .decode()
                .unwrap_or_else(|error| panic!("{label}: could not decode {asset}: {error}"));
            assert_eq!(
                (image.width(), image.height()),
                (128, 128),
                "{label}: {asset}"
            );
        }
    }
}

#[test]
fn provider_effort_ladders_map_to_two_portrait_tiers() {
    for effort in [
        None,
        Some("none"),
        Some("low"),
        Some("medium"),
        Some("shallow"),
    ] {
        assert!(!portrait_uses_high_effort(effort), "{effort:?}");
    }
    for effort in [
        "high",
        "HIGH",
        "xhigh",
        "extra-high",
        "very_high",
        "ultra",
        "max",
        "maximum",
        "deep",
    ] {
        assert!(portrait_uses_high_effort(Some(effort)), "{effort}");
    }
    assert!(!portrait_uses_high_effort(Some("invented")));
    let src = include_str!("../../../cockpit/src/ui/agent_panel/profile.rs");
    let start = src
        .find("pub fn portrait_uses_high_effort")
        .expect("portrait_uses_high_effort");
    let body = src[start..]
        .split("pub fn profile_for(")
        .next()
        .expect("effort body");
    assert!(
        !body.contains("to_ascii_lowercase"),
        "portrait paint must not lowercase effort: {body}"
    );
}

#[test]
fn unknown_profiles_still_have_real_image_assets() {
    let profile = profile_for("moa");
    assert_eq!(profile.key, AgentKey::Unknown);
    assert_eq!(
        profile.asset(false),
        Some("assets/agents/sparky-champion.png")
    );
    assert_eq!(
        profile.asset(true),
        Some("assets/agents/sparky-champion-high.png")
    );
}

#[test]
fn asset_falls_back_to_the_other_when_one_is_missing() {
    // No profile ships a half-filled asset pair, so exercise the `.or()`
    // fallback directly (the test module can see the private fields).
    let only_neutral = AgentProfile {
        key: AgentKey::Unknown,
        name: "n",
        role: "r",
        detail: "d",
        standard_asset: Some("neutral.png"),
        high_effort_asset: None,
    };
    // The high tier falls back to standard when its companion is absent.
    assert_eq!(only_neutral.asset(true), Some("neutral.png"));
    assert_eq!(only_neutral.asset(false), Some("neutral.png"));

    let only_active = AgentProfile {
        key: AgentKey::Unknown,
        name: "n",
        role: "r",
        detail: "d",
        standard_asset: None,
        high_effort_asset: Some("active.png"),
    };
    assert_eq!(only_active.asset(false), Some("active.png"));
    assert_eq!(only_active.asset(true), Some("active.png"));

    let none = AgentProfile {
        key: AgentKey::Unknown,
        name: "n",
        role: "r",
        detail: "d",
        standard_asset: None,
        high_effort_asset: None,
    };
    assert_eq!(none.asset(true), None);
    assert_eq!(none.asset(false), None);
}

#[test]
fn contains_ascii_case_guards_empty_needle_and_short_haystack() {
    // Empty needle → false (never "found").
    assert!(!contains_ascii_case("anything", b""));
    // Haystack shorter than the needle → false, no out-of-bounds.
    assert!(!contains_ascii_case("ab", b"abcdef"));
    // A match at the very end is found.
    assert!(contains_ascii_case("xxAPOLLO", b"apollo"));
    // First byte matches but the tail diverges → keep scanning, ultimately false.
    assert!(!contains_ascii_case("apXlomore", b"apollo"));
}

#[test]
fn base_profile_carries_role_and_detail_text() {
    let codex = profile_for("codex");
    assert_eq!(codex.name, "Codex");
    assert_eq!(codex.role, "SOTA escalation");
    let unknown = profile_for("totally-unheard-of");
    assert_eq!(unknown.key, AgentKey::Unknown);
    assert_eq!(unknown.name, "Agent");
    assert_eq!(unknown.role, "standby");
}
