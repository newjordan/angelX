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
use std::path::{Path, PathBuf};
use std::sync::OnceLock;

/// A workspace's identity: the root its island is seeded from and the key its
/// saved progress files under.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Identity {
    pub root: PathBuf,
    pub key: String,
}

/// The cockpit's answers, lent once.
#[derive(Clone, Copy)]
pub struct Hooks {
    /// Whether the ambient stage sims may run (false in Lean / Comp / Turbo).
    pub ambient_stage_sim_allowed: fn() -> bool,
    /// The latest published agent-activity stage, if any.
    pub stage_current: fn() -> Option<StageSnapshot>,
    /// Turn over: drop the published stage so the world disbands its muster.
    pub stage_clear: fn(),
    /// The repository identity of a workspace.
    pub repo_identity: fn(&Path) -> Identity,
    /// Reachability: any HTTP answer to a GET counts as "the light is on" (a
    /// 404 means the process is alive); only a transport failure darkens it.
    pub http_probe: fn(url: &str) -> bool,
    /// GET `url` and parse the JSON answer; `None` on any failure.
    pub http_get_json: fn(url: &str) -> Option<serde_json::Value>,
    /// POST a JSON body, with room for a slow model to speak, and parse the
    /// JSON answer; `None` on any failure.
    pub http_post_json: fn(url: &str, body: serde_json::Value) -> Option<serde_json::Value>,
}

static HOOKS: OnceLock<Hooks> = OnceLock::new();

/// Lend the cockpit's answers. The first call wins; returns whether this call
/// installed them.
pub fn install(hooks: Hooks) -> bool {
    HOOKS.set(hooks).is_ok()
}

/// Whether the ambient stage sims (loop loom, raytrace, MoA, the quest chrome)
/// may run.
pub fn ambient_stage_sim_allowed() -> bool {
    match HOOKS.get() {
        Some(hooks) => (hooks.ambient_stage_sim_allowed)(),
        None => !angel_live::host::posture::from_env_uncached(),
    }
}

/// Whether the cockpit lent a stage to read. Without one the world never
/// musters (the fixtures never see another test's published stage).
pub fn stage_lent() -> bool {
    HOOKS.get().is_some()
}

/// The latest published stage, if any.
pub fn stage_current() -> Option<StageSnapshot> {
    HOOKS.get().and_then(|hooks| (hooks.stage_current)())
}

/// Drop the published stage.
pub fn stage_clear() {
    if let Some(hooks) = HOOKS.get() {
        (hooks.stage_clear)();
    }
}

/// The identity of `workspace`.
pub fn repo_identity(workspace: &Path) -> Identity {
    match HOOKS.get() {
        Some(hooks) => (hooks.repo_identity)(workspace),
        None => Identity {
            root: workspace.to_path_buf(),
            key: angel_live::host::workspace_store::workspace_key(workspace),
        },
    }
}

/// Whether `url` answers at all (false without the cockpit's client).
pub fn http_probe(url: &str) -> bool {
    HOOKS.get().is_some_and(|hooks| (hooks.http_probe)(url))
}

/// GET `url` as JSON.
pub fn http_get_json(url: &str) -> Option<serde_json::Value> {
    HOOKS.get().and_then(|hooks| (hooks.http_get_json)(url))
}

/// POST `body` to `url` as JSON and read the JSON answer.
pub fn http_post_json(url: &str, body: serde_json::Value) -> Option<serde_json::Value> {
    HOOKS
        .get()
        .and_then(|hooks| (hooks.http_post_json)(url, body))
}
