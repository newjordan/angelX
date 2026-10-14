//! The model houses as the cockpit knows them: which house each portrait
//! key, and so each route, serves from. The houses themselves (their
//! castles, knights and banners, and who is serving) are the Delve's
//! (`angel_delve::world::houses`).

pub(crate) use angel_delve::world::houses::*;

use crate::ui::agent_panel::profile::AgentKey;

/// The profile key each house answers for, by the house's key.
const AGENTS: [(&str, AgentKey); 16] = [
    ("sol", AgentKey::Codex),
    ("luna", AgentKey::Luna),
    ("grok", AgentKey::Grok),
    ("deepseek", AgentKey::DeepSeek),
    ("glm", AgentKey::Glm),
    ("kimi", AgentKey::Kimi),
    ("qwen", AgentKey::Qwen),
    ("muse", AgentKey::Muse),
    ("longcat", AgentKey::LongCat),
    ("hy", AgentKey::Hy),
    ("nemotron", AgentKey::Nemotron),
    ("gemma", AgentKey::Gemma),
    ("inkling", AgentKey::Inkling),
    ("laguna", AgentKey::Laguna),
    ("north", AgentKey::North),
    ("astra", AgentKey::Astra),
];

/// The house a portrait key belongs to; the stub and unknown routes have none
/// (their knight is the Keep's own).
pub(crate) fn of_agent(key: AgentKey) -> Option<HouseId> {
    AGENTS
        .iter()
        .find(|&&(_, agent)| agent == key && key != AgentKey::Unknown)
        .and_then(|&(house, _)| by_key(house))
}

/// The house a route serves from: its model family, whichever machine or
/// provider serves it.
pub(crate) fn of_route(agent: &str, driver: &str, model: Option<&str>) -> Option<HouseId> {
    of_agent(crate::ui::agent_panel::profile::profile_for_route(agent, driver, model).key)
}

#[cfg(test)]
#[path = "../../../tests/cockpit/app/houses__tests.rs"]
mod tests;
