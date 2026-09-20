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
    // Apollo is the swarm's specialist persona. An active specialist may
    // change the swarm portrait, while other selected agent portraits retain
    // their own identity. Persona names do not require particular machines.
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
#[path = "../../tests/cockpit/app/agent_profile__tests.rs"]
mod tests;
