use super::*;

#[test]
fn profile_aliases_pick_expected_agents() {
    assert_eq!(profile_for("turbo", false).key, AgentKey::Turbo);
    assert_eq!(profile_for("atlas", false).key, AgentKey::Atlas);
    assert_eq!(profile_for("spark-r1", false).key, AgentKey::Sparky);
    assert_eq!(profile_for("spark-v4", false).key, AgentKey::Sparky);
    assert_eq!(profile_for("dsflash", false).key, AgentKey::Sparky);
    assert_eq!(profile_for("ds4", false).key, AgentKey::Sparky);
    assert_eq!(profile_for("apollo", false).key, AgentKey::Apollo);
    assert_eq!(profile_for("gpu-comp", false).key, AgentKey::GpuComp);
    assert_eq!(profile_for("mathgod", false).key, AgentKey::MathGod);
    assert_eq!(profile_for("math-god", false).key, AgentKey::MathGod);
    assert_eq!(
        profile_for("gpu-comp-local-moa", false).key,
        AgentKey::GpuComp
    );
    assert_eq!(profile_for(" Turbo ", false).key, AgentKey::Turbo);
    assert_eq!(profile_for("SPARK", false).key, AgentKey::Sparky);
    assert_eq!(profile_for("practice", false).name, "Practice");
    assert_eq!(profile_for("spark", false).name, "Sparky");
}

#[test]
fn route_profiles_distinguish_provider_models_on_one_logical_agent() {
    let openai = profile_for_route("sota", "openai", Some("gpt-5.6-sol"), false);
    let deepseek = profile_for_route("sota", "deepseek", Some("deepseek-v4-pro"), false);
    let grok = profile_for_route("sota", "xai", Some("grok-4.6"), false);
    let kimi = profile_for_route("sota", "kimi", Some("kimi-k3"), false);

    assert_eq!(openai.key, AgentKey::Codex);
    assert_eq!(deepseek.key, AgentKey::Sparky);
    assert_eq!(grok.key, AgentKey::Turbo);
    assert_eq!(kimi.key, AgentKey::Atlas);
    assert_eq!(
        profile_for_route("sota", "openai", Some("GPT-5.6-SOL"), false).key,
        AgentKey::Codex
    );
    assert_eq!(
        profile_for_route("sota", "local", Some("DSFLASH-local"), false).key,
        AgentKey::Sparky
    );
    assert_ne!(openai.asset(false), deepseek.asset(false));
    assert_ne!(deepseek.asset(false), grok.asset(false));
    let src = include_str!("../../../cockpit/src/agent/profile.rs");
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
fn named_local_box_identity_wins_over_the_served_model_family() {
    assert_eq!(
        profile_for_route("turbo", "deepseek", Some("deepseek-v4-pro"), false).key,
        AgentKey::Turbo
    );
    assert_eq!(
        profile_for_route("spark", "openai", Some("gpt-5.6-sol"), false).key,
        AgentKey::Sparky
    );
    assert_eq!(
        profile_for_route("apollo", "xai", Some("grok-4.6"), false).key,
        AgentKey::Apollo
    );
}

#[test]
fn codex_agent_uses_distinct_standard_and_high_effort_masters() {
    for label in ["openai", "codex", "GPT", " OpenAI "] {
        let p = profile_for(label, false);
        assert_eq!(p.key, AgentKey::Codex, "label {label:?}");
    }
    let p = profile_for("openai", false);
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
    for label in ["turbo", "atlas", "spark", "apollo", "codex"] {
        let profile = profile_for(label, false);
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
    let src = include_str!("../../../cockpit/src/agent/profile.rs");
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
fn apollo_specialist_override_wins_only_for_the_moa_host() {
    // The specialist persona may recolour the moa host (Sparky / a generic
    // unknown moa driver), since Apollo runs on the Spark box.
    let profile = profile_for("spark", true);
    assert_eq!(profile.key, AgentKey::Apollo);
    assert!(profile.detail.contains("RTX 4080"));
    assert_eq!(profile_for("moa", true).key, AgentKey::Apollo);
}

#[test]
fn apollo_specialist_never_repaints_other_in_hand_agents() {
    // Regression: asking the OpenAI/Codex agent "who are you" surfaced the word
    // "apollo"/"kernel" and the specialist flag used to hijack the portrait,
    // pinning Apollo onto codex permanently. A specialist signal must NEVER
    // override a distinct in-hand agent — the avatar follows the agent in hand.
    for label in ["openai", "codex", "gpt", "OpenAI"] {
        assert_eq!(
            profile_for(label, true).key,
            AgentKey::Codex,
            "specialist flag must not repaint codex (label {label:?})"
        );
    }
    assert_eq!(profile_for("turbo", true).key, AgentKey::Turbo);
    assert_eq!(profile_for("atlas", true).key, AgentKey::Atlas);
    assert_eq!(profile_for("gpu-comp", true).key, AgentKey::GpuComp);
}

#[test]
fn unknown_profiles_still_have_real_image_assets() {
    let profile = profile_for("moa", false);
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
fn specialist_text_detects_apollo_related_work() {
    assert!(specialist_text("route this to Apollo"));
    assert!(specialist_text("DICE kernel bridge"));
    assert!(specialist_text("日本語 KERNEL bridge"));
    assert!(!specialist_text("ordinary chat"));
}

#[test]
fn specialist_text_matches_head_bridge_token() {
    // The `head-bridge` alias is one of the specialist signals.
    assert!(specialist_text("dispatch over the HEAD-BRIDGE now"));
    assert!(!specialist_text("nothing special here"));
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
    let turbo = profile_for("turbo", false);
    assert_eq!(turbo.name, "Turbo");
    assert_eq!(turbo.role, "fastest brain");
    let unknown = profile_for("totally-unheard-of", false);
    assert_eq!(unknown.key, AgentKey::Unknown);
    assert_eq!(unknown.name, "Agent");
    assert_eq!(unknown.role, "standby");
}
