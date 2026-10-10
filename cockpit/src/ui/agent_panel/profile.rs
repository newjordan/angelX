#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AgentKey {
    Codex,
    Luna,
    Astra,
    Glm,
    Kimi,
    Qwen,
    LongCat,
    Muse,
    Hy,
    Nemotron,
    Grok,
    DeepSeek,
    Gemma,
    Inkling,
    Laguna,
    North,
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

pub fn profile_for(label: &str) -> AgentProfile {
    base_profile_for(label)
}

/// Resolve the portrait for one concrete route. The portrait is the model's
/// own knight: the served model family decides it, whichever host or provider
/// serves it; a route whose model has no family falls back to its label.
pub fn profile_for_route(agent_label: &str, driver: &str, model: Option<&str>) -> AgentProfile {
    route_profile_for(driver, model).unwrap_or_else(|| base_profile_for(agent_label))
}

fn route_profile_for(driver: &str, model: Option<&str>) -> Option<AgentProfile> {
    let model = model.unwrap_or_default();
    let fields = [driver, model];

    // An explicit OpenAI/Codex model ID keeps its authored family before the
    // served-model check below.
    if contains_ascii_case(model, b"openai")
        || contains_ascii_case(model, b"codex")
        || contains_ascii_case(model, b"gpt-")
    {
        return Some(codex_profile(&fields));
    }

    // A known served-model family outranks a generic OpenAI-compatible driver.
    // Keep the model-only check scoped to that transport; every other driver
    // retains the original precedence below.
    let driver = driver.trim();
    if contains_ascii_case(driver, b"openai")
        && !driver.eq_ignore_ascii_case("openai")
        && let Some(profile) = model_profile(model)
    {
        return Some(profile);
    }

    if fields.iter().any(|field| {
        let field = field.trim();
        contains_ascii_case(field, b"openai")
            || contains_ascii_case(field, b"codex")
            || contains_ascii_case(field, b"gpt-")
    }) {
        return Some(codex_profile(&fields));
    }
    if fields.iter().any(|field| {
        let field = field.trim();
        contains_ascii_case(field, b"deepseek")
    }) {
        return Some(DEEPSEEK_PROFILE);
    }
    if fields.iter().any(|field| {
        let field = field.trim();
        contains_ascii_case(field, b"grok")
            || contains_ascii_case(field, b"xai")
            || contains_ascii_case(field, b"x.ai")
    }) {
        return Some(GROK_PROFILE);
    }
    if let Some(knight) = family_profile_for(driver, &fields) {
        return Some(knight);
    }
    None
}

fn codex_profile(fields: &[&str; 2]) -> AgentProfile {
    if fields
        .iter()
        .any(|field| contains_ascii_case(field, b"luna"))
    {
        return LUNA_PROFILE;
    }
    if fields
        .iter()
        .any(|field| contains_ascii_case(field, b"astra"))
    {
        return ASTRA_PROFILE;
    }
    base_profile_for("codex")
}

fn model_profile(model: &str) -> Option<AgentProfile> {
    let model_fields = [model, model];
    if contains_ascii_case(model, b"deepseek") {
        return Some(DEEPSEEK_PROFILE);
    }
    if contains_ascii_case(model, b"grok")
        || contains_ascii_case(model, b"xai")
        || contains_ascii_case(model, b"x.ai")
    {
        return Some(GROK_PROFILE);
    }
    family_profile_for(model, &model_fields)
}

/// Every other model family wears its own knight, whichever machine or
/// provider serves it. The swarm keeps its runner portrait whatever checkpoint
/// it serves.
fn family_profile_for(driver: &str, fields: &[&str; 2]) -> Option<AgentProfile> {
    if driver.eq_ignore_ascii_case("swarm") || driver.eq_ignore_ascii_case("local-swarm") {
        return None;
    }
    let any = |test: fn(&str) -> bool| fields.iter().any(|field| test(field.trim()));
    let knight = if any(|f| contains_ascii_case(f, b"glm") || contains_ascii_case(f, b"zhipu")) {
        GLM_PROFILE
    } else if any(|f| contains_ascii_case(f, b"kimi") || contains_ascii_case(f, b"moonshot")) {
        KIMI_PROFILE
    } else if any(|f| contains_ascii_case(f, b"qwen")) {
        QWEN_PROFILE
    } else if any(|f| contains_ascii_case(f, b"longcat")) {
        LONGCAT_PROFILE
    } else if any(|f| f.eq_ignore_ascii_case("meta") || starts_with_ascii_case(f, b"muse-")) {
        MUSE_PROFILE
    } else if any(|f| {
        f.eq_ignore_ascii_case("hy")
            || contains_ascii_case(f, b"hy3")
            || contains_ascii_case(f, b"hunyuan")
    }) {
        HY_PROFILE
    } else if any(|f| contains_ascii_case(f, b"nemotron")) {
        NEMOTRON_PROFILE
    } else if any(|f| contains_ascii_case(f, b"gemma")) {
        GEMMA_PROFILE
    } else if any(|f| {
        contains_ascii_case(f, b"inkling") || contains_ascii_case(f, b"thinkingmachines")
    }) {
        INKLING_PROFILE
    } else if any(|f| contains_ascii_case(f, b"laguna") || contains_ascii_case(f, b"poolside")) {
        LAGUNA_PROFILE
    } else if any(|f| {
        contains_ascii_case(f, b"north-mini") || contains_ascii_case(f, b"cohere/north")
    }) {
        NORTH_PROFILE
    } else {
        return None;
    };
    Some(knight)
}

fn base_profile_for(label: &str) -> AgentProfile {
    let label = label.trim();
    if label.eq_ignore_ascii_case("openai")
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

/// A model-family knight. Its helm sheet is the portrait; the legacy
/// champion stills (used only when portrait states are off) stay those the
/// family showed before it had a knight of its own.
const fn knight(
    key: AgentKey,
    name: &'static str,
    role: &'static str,
    detail: &'static str,
    champion: (&'static str, &'static str),
) -> AgentProfile {
    AgentProfile {
        key,
        name,
        role,
        detail,
        standard_asset: Some(champion.0),
        high_effort_asset: Some(champion.1),
    }
}

const ATLAS_STILLS: (&str, &str) = (
    "assets/agents/atlas-champion.png",
    "assets/agents/atlas-champion-high.png",
);

const LUNA_PROFILE: AgentProfile = knight(
    AgentKey::Luna,
    "Luna",
    "SOTA escalation",
    "ChatGPT-OAuth Luna route",
    (
        "assets/agents/codex-champion.png",
        "assets/agents/codex-champion-high.png",
    ),
);
const ASTRA_PROFILE: AgentProfile = knight(
    AgentKey::Astra,
    "Astra",
    "SOTA seat",
    "ChatGPT-OAuth Astra route",
    (
        "assets/agents/codex-champion.png",
        "assets/agents/codex-champion-high.png",
    ),
);
const GLM_PROFILE: AgentProfile =
    knight(AgentKey::Glm, "GLM", "SOTA seat", "GLM route", ATLAS_STILLS);
const KIMI_PROFILE: AgentProfile = knight(
    AgentKey::Kimi,
    "Kimi",
    "SOTA seat",
    "Moonshot Kimi route",
    ATLAS_STILLS,
);
const QWEN_PROFILE: AgentProfile = knight(
    AgentKey::Qwen,
    "Qwen",
    "SOTA seat",
    "hosted Qwen route",
    ATLAS_STILLS,
);
const LONGCAT_PROFILE: AgentProfile = knight(
    AgentKey::LongCat,
    "LongCat",
    "SOTA seat",
    "LongCat route",
    ATLAS_STILLS,
);
const MUSE_PROFILE: AgentProfile = knight(
    AgentKey::Muse,
    "Muse",
    "SOTA seat",
    "Meta Muse route",
    ATLAS_STILLS,
);
const HY_PROFILE: AgentProfile = knight(
    AgentKey::Hy,
    "Hy",
    "SOTA seat",
    "Tencent Hy route",
    ATLAS_STILLS,
);
const NEMOTRON_PROFILE: AgentProfile = knight(
    AgentKey::Nemotron,
    "Nemotron",
    "SOTA seat",
    "Nemotron route",
    ATLAS_STILLS,
);
const GROK_PROFILE: AgentProfile = knight(
    AgentKey::Grok,
    "Grok",
    "SOTA seat",
    "Grok route",
    (
        "assets/agents/turbo-champion.png",
        "assets/agents/turbo-champion-high.png",
    ),
);
const DEEPSEEK_PROFILE: AgentProfile = knight(
    AgentKey::DeepSeek,
    "DeepSeek",
    "SOTA seat",
    "DeepSeek route",
    (
        "assets/agents/sparky-champion.png",
        "assets/agents/sparky-champion-high.png",
    ),
);
const GEMMA_PROFILE: AgentProfile = knight(
    AgentKey::Gemma,
    "Gemma",
    "open model",
    "Gemma route",
    ATLAS_STILLS,
);
const INKLING_PROFILE: AgentProfile = knight(
    AgentKey::Inkling,
    "Inkling",
    "SOTA seat",
    "Inkling route",
    ATLAS_STILLS,
);
const LAGUNA_PROFILE: AgentProfile = knight(
    AgentKey::Laguna,
    "Laguna",
    "SOTA seat",
    "Laguna route",
    ATLAS_STILLS,
);
const NORTH_PROFILE: AgentProfile = knight(
    AgentKey::North,
    "North",
    "SOTA seat",
    "North route",
    ATLAS_STILLS,
);

#[cfg(test)]
#[path = "../../../../tests/cockpit/app/agent_profile__tests.rs"]
mod tests;
