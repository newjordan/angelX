//! What the realm host asks of the cockpit it is linked into.
//!
//! The game's stage reads facts only the cockpit knows: whether the ambient
//! stage sims may run (Lean / Comp / Turbo mode), the agent-activity stage a
//! swarm publishes, the clearing of that stage when a turn ends, a workspace's
//! repository identity (it needs the cockpit's git sandbox), and the village's
//! HTTP (the cockpit's client, so the host links none). The cockpit lends them
//! once at startup with [`install`].
//!
//! Without them (a test, or an example that never starts the cockpit) the host
//! behaves as the cockpit does under test: the posture is read from the
//! environment, no stage is published (so the world never musters an army), a
//! workspace is its own identity, and the village's lights stay dark.

use angel_live::telemetry::StageSnapshot;
use std::path::Path;
use std::sync::OnceLock;

pub use angel_plug::screen::Identity;

#[derive(Clone, Copy)]
pub struct Hooks {
    pub ambient_stage_sim_allowed: fn() -> bool,
    pub stage_current: fn() -> Option<StageSnapshot>,
    pub stage_clear: fn(),
    pub repo_identity: fn(&Path) -> Identity,
    pub http_probe: fn(url: &str) -> bool,
    pub http_get_json: fn(url: &str) -> Option<serde_json::Value>,
    pub http_post_json: fn(url: &str, body: serde_json::Value) -> Option<serde_json::Value>,
}

static HOOKS: OnceLock<Hooks> = OnceLock::new();

pub fn install(hooks: Hooks) -> bool {
    HOOKS.set(hooks).is_ok()
}

pub fn ambient_stage_sim_allowed() -> bool {
    match HOOKS.get() {
        Some(hooks) => (hooks.ambient_stage_sim_allowed)(),
        None => !angel_live::host::posture::from_env_uncached(),
    }
}

pub fn stage_lent() -> bool {
    HOOKS.get().is_some()
}

pub fn stage_current() -> Option<StageSnapshot> {
    HOOKS.get().and_then(|hooks| (hooks.stage_current)())
}

pub fn stage_clear() {
    if let Some(hooks) = HOOKS.get() {
        (hooks.stage_clear)();
    }
}

pub fn repo_identity(workspace: &Path) -> Identity {
    match HOOKS.get() {
        Some(hooks) => (hooks.repo_identity)(workspace),
        None => Identity {
            root: workspace.to_path_buf(),
            key: angel_live::host::workspace_store::workspace_key(workspace),
        },
    }
}

pub fn http_probe(url: &str) -> bool {
    HOOKS.get().is_some_and(|hooks| (hooks.http_probe)(url))
}

pub fn http_get_json(url: &str) -> Option<serde_json::Value> {
    HOOKS.get().and_then(|hooks| (hooks.http_get_json)(url))
}

pub fn http_post_json(url: &str, body: serde_json::Value) -> Option<serde_json::Value> {
    HOOKS
        .get()
        .and_then(|hooks| (hooks.http_post_json)(url, body))
}
