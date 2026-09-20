#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AgentKey {
    Turbo,
    Atlas,
    Sparky,
    Apollo,
    Codex,
    GpuComp,
    MathGod,
    Unknown,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct AgentProfile {
    pub key: AgentKey,
    pub name: &'static str,
    pub role: &'static str,
    pub detail: &'static str,
    standard_asset: Option<&'static str>,
    high_effort_asset: Option<&'static str>,
}

impl AgentProfile {
    pub fn asset(self, high_effort: bool) -> Option<&'static str> {
        if high_effort {
            self.high_effort_asset.or(self.standard_asset)
        } else {
            self.standard_asset.or(self.high_effort_asset)
        }
    }
}

/// Collapse provider-specific reasoning ladders into the cockpit's two
/// intentionally distinct portrait states. Unknown and unconfigured efforts
/// stay on the calmer standard portrait; only an explicit upper rung lights
/// the high-effort companion.
pub fn portrait_uses_high_effort(effort: Option<&str>) -> bool {
    let Some(effort) = effort.map(str::trim).filter(|effort| !effort.is_empty()) else {
        return false;
    };
    // Portrait paint asks this every frame; matching must not allocate.
    effort.eq_ignore_ascii_case("high")
        || effort.eq_ignore_ascii_case("xhigh")
        || effort.eq_ignore_ascii_case("extra-high")
        || effort.eq_ignore_ascii_case("extra_high")
        || effort.eq_ignore_ascii_case("very-high")
        || effort.eq_ignore_ascii_case("very_high")
        || effort.eq_ignore_ascii_case("ultra")
        || effort.eq_ignore_ascii_case("max")
        || effort.eq_ignore_ascii_case("maximum")
        || effort.eq_ignore_ascii_case("deep")
}

pub fn profile_for(label: &str, apollo_specialist: bool) -> AgentProfile {
    apply_specialist_override(base_profile_for(label), apollo_specialist)
}

/// Resolve the portrait for one concrete route. Named local boxes keep their
/// authored identities even when the checkpoint they serve changes; logical
/// provider boxes such as `sota` instead derive an identity from the route that
/// is actually selected. This keeps several provider models housed in the same
/// Bag agent from sharing a stale generic portrait.
pub fn profile_for_route(
    agent_label: &str,
    driver: &str,
    model: Option<&str>,
    apollo_specialist: bool,
) -> AgentProfile {
    let agent = base_profile_for(agent_label);
    let base = if agent.key != AgentKey::Unknown {
        // Physical/local named boxes are the stable visual identity. A Spark
        // serving DeepSeek remains Sparky; a Turbo serving another checkpoint
        // remains Turbo.
        agent
    } else {
        route_profile_for(driver, model).unwrap_or(agent)
    };
    apply_specialist_override(base, apollo_specialist)
}

fn apply_specialist_override(base: AgentProfile, apollo_specialist: bool) -> AgentProfile {
    // Apollo is one of the SWARM's specialist personas — it runs *on the Spark
    // box*. When apollo-class specialist work is live it may transiently recolour
    // the moa driver's portrait, but ONLY the moa host (Sparky, or a generic /
    // unknown moa driver). It must never repaint a *different* in-hand agent
    // such as codex, turbo, or atlas: doing so pinned the Apollo avatar onto the
    // OpenAI/Codex agent and stuck there (the reported stuck-avatar bug — asking
    // codex "who are you" surfaces the word "apollo"/"kernel" and used to hijack
    // the portrait permanently). The avatar must stay a function of the in-hand
    // agent, so the specialist override is scoped to the moa host alone.
    if apollo_specialist && matches!(base.key, AgentKey::Sparky | AgentKey::Unknown) {
        return APOLLO_PROFILE;
    }
    base
}

fn route_profile_for(driver: &str, model: Option<&str>) -> Option<AgentProfile> {
    let model = model.unwrap_or_default();
    let fields = [driver, model];

    // Provider-family precedence is deterministic. A provider-qualified model
    // (for example `openai/gpt-*`) retains its authored family rather than
    // falling through to the generic SOTA portrait. Matching must not allocate:
    // Brain Route cursoring calls this on the UI thread.
    if fields.iter().any(|field| {
        let field = field.trim();
        contains_ascii_case(field, b"openai")
            || contains_ascii_case(field, b"codex")
            || contains_ascii_case(field, b"gpt-")
    }) {
        return Some(base_profile_for("codex"));
    }
    if fields.iter().any(|field| {
        let field = field.trim();
        contains_ascii_case(field, b"deepseek")
            || field.eq_ignore_ascii_case("ds4")
            || field.eq_ignore_ascii_case("dsflash")
            || starts_with_ascii_case(field, b"dsflash-")
    }) {
        return Some(base_profile_for("spark"));
    }
    if fields.iter().any(|field| {
        let field = field.trim();
        contains_ascii_case(field, b"grok")
            || contains_ascii_case(field, b"xai")
            || contains_ascii_case(field, b"x.ai")
    }) {
        return Some(base_profile_for("turbo"));
    }
    if fields
        .iter()
        .any(|field| crate::club::is_sota_label(field.trim()))
    {
        return Some(base_profile_for("atlas"));
    }
    None
}

fn base_profile_for(label: &str) -> AgentProfile {
    let label = label.trim();
    if label.eq_ignore_ascii_case("turbo") {
        AgentProfile {
            key: AgentKey::Turbo,
            name: "Turbo",
            role: "fastest brain",
            detail: "high-speed strategy agent",
            standard_asset: Some("assets/agents/turbo-champion.png"),
            high_effort_asset: Some("assets/agents/turbo-champion-high.png"),
        }
    } else if label.eq_ignore_ascii_case("atlas") {
        AgentProfile {
            key: AgentKey::Atlas,
            name: "Atlas",
            role: "general agent",
            detail: "reliable baseline operator",
            standard_asset: Some("assets/agents/atlas-champion.png"),
            high_effort_asset: Some("assets/agents/atlas-champion-high.png"),
        }
    } else if label.eq_ignore_ascii_case("spark")
        || label.eq_ignore_ascii_case("spark-r1")
        || label.eq_ignore_ascii_case("spark-v4")
        || label.eq_ignore_ascii_case("dsflash")
        || label.eq_ignore_ascii_case("ds4")
    {
        AgentProfile {
            key: AgentKey::Sparky,
            name: "Sparky",
            role: "powerhouse",
            detail: "multi-agent runner",
            standard_asset: Some("assets/agents/sparky-champion.png"),
            high_effort_asset: Some("assets/agents/sparky-champion-high.png"),
        }
    } else if label.eq_ignore_ascii_case("apollo") {
        APOLLO_PROFILE
    } else if label.eq_ignore_ascii_case("openai")
        || label.eq_ignore_ascii_case("codex")
        || label.eq_ignore_ascii_case("gpt")
    {
        AgentProfile {
            key: AgentKey::Codex,
            name: "Codex",
            role: "SOTA escalation",
            detail: "ChatGPT-OAuth reasoning agent",
            standard_asset: Some("assets/agents/codex-champion.png"),
            high_effort_asset: Some("assets/agents/codex-champion-high.png"),
        }
    } else if label.eq_ignore_ascii_case("gpu-comp")
        || label.eq_ignore_ascii_case("gpu-comp-local-moa")
        || label.eq_ignore_ascii_case("gpu comp")
    {
        AgentProfile {
            key: AgentKey::GpuComp,
            name: "GPU Comp",
            role: "agent configuration",
            detail: "Turbo x12, Leanstral math, DICE advisor",
            standard_asset: Some("assets/agents/turbo-champion.png"),
            high_effort_asset: Some("assets/agents/turbo-champion-high.png"),
        }
    } else if label.eq_ignore_ascii_case("mathgod")
        || label.eq_ignore_ascii_case("math-god")
        || label.eq_ignore_ascii_case("math god")
    {
        AgentProfile {
            key: AgentKey::MathGod,
            name: "Math God",
            role: "Lean/math solver",
            detail: "Sol@ultra head + GLM-5.3 + DeepSeek v4 Pro, Grok xhigh; Leanstral send-to",
            standard_asset: Some("assets/agents/codex-champion.png"),
            high_effort_asset: Some("assets/agents/codex-champion-high.png"),
        }
    } else if label.eq_ignore_ascii_case("practice") {
        AgentProfile {
            key: AgentKey::Unknown,
            name: "Practice",
            role: "standby",
            detail: "offline echo — no live model",
            standard_asset: Some("assets/agents/sparky-champion.png"),
            high_effort_asset: Some("assets/agents/sparky-champion-high.png"),
        }
    } else {
        AgentProfile {
            key: AgentKey::Unknown,
            name: "Agent",
            role: "standby",
            detail: "unnamed route",
            standard_asset: Some("assets/agents/sparky-champion.png"),
            high_effort_asset: Some("assets/agents/sparky-champion-high.png"),
        }
    }
}

pub fn specialist_text(text: &str) -> bool {
    contains_ascii_case(text, b"apollo")
        || contains_ascii_case(text, b"dice")
        || contains_ascii_case(text, b"head-bridge")
        || contains_ascii_case(text, b"kernel")
}

fn starts_with_ascii_case(text: &str, prefix: &[u8]) -> bool {
    let haystack = text.as_bytes();
    haystack.len() >= prefix.len() && haystack[..prefix.len()].eq_ignore_ascii_case(prefix)
}

fn contains_ascii_case(text: &str, needle: &[u8]) -> bool {
    let haystack = text.as_bytes();
    if needle.is_empty() || haystack.len() < needle.len() {
        return false;
    }
    let first = needle[0];
    for start in 0..=haystack.len() - needle.len() {
        if !haystack[start].eq_ignore_ascii_case(&first) {
            continue;
        }
        if haystack[start + 1..start + needle.len()]
            .iter()
            .zip(&needle[1..])
            .all(|(a, b)| a.eq_ignore_ascii_case(b))
        {
            return true;
        }
    }
    false
}

const APOLLO_PROFILE: AgentProfile = AgentProfile {
    key: AgentKey::Apollo,
    name: "Apollo",
    role: "specialist",
    detail: "RTX 4080 micro-lab agent",
    standard_asset: Some("assets/agents/apollo-champion.png"),
    high_effort_asset: Some("assets/agents/apollo-champion-high.png"),
};

#[cfg(test)]
mod tests {
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
        let src = include_str!("agent_profile.rs");
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
        let src = include_str!("agent_profile.rs");
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
}
