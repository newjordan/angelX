//! The model houses as the cockpit knows them: which house each portrait
//! key, and so each route, serves from. The houses themselves (their
//! castles, knights and banners, and who is serving) are the Delve's
//! (`angel_delve::world::houses`).

pub use angel_delve::world::houses::*;

use angel_plug::feed::AgentKey;

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
pub fn of_agent(key: AgentKey) -> Option<HouseId> {
    AGENTS
        .iter()
        .find(|&&(_, agent)| agent == key && key != AgentKey::Unknown)
        .and_then(|&(house, _)| by_key(house))
}
