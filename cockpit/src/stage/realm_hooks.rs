//! What the cockpit lends the realm host (`angel_realm_host::hooks`): its
//! Lean / Comp / Turbo posture, the agent-activity stage a swarm publishes, a
//! workspace's repository identity and the HTTP client the village probes its
//! forge and heads with. Installed once at startup; without it (tests) the host
//! behaves as it does under test.

use angel_realm_host::hooks::{Hooks, Identity};
use std::path::Path;
use std::sync::OnceLock;
use std::time::Duration;

/// Short, hard-capped probe client — a wedged head can cost one cycle at most.
fn probe_agent() -> &'static ureq::Agent {
    static AGENT: OnceLock<ureq::Agent> = OnceLock::new();
    AGENT.get_or_init(|| {
        ureq::AgentBuilder::new()
            .timeout_connect(Duration::from_secs(4))
            .timeout(Duration::from_secs(8))
            .user_agent("angelX-village/0.1")
            .build()
    })
}

/// The chatter client gets more read headroom — a 4B takes a moment to speak —
/// but still a hard total cap. Background thread only; the UI never waits.
fn voice_agent() -> &'static ureq::Agent {
    static AGENT: OnceLock<ureq::Agent> = OnceLock::new();
    AGENT.get_or_init(|| {
        ureq::AgentBuilder::new()
            .timeout_connect(Duration::from_secs(4))
            .timeout(Duration::from_secs(30))
            .user_agent("angelX-village/0.1")
            .build()
    })
}

fn http_probe(url: &str) -> bool {
    match probe_agent().get(url).call() {
        Ok(_) => true,
        Err(ureq::Error::Status(_, _)) => true,
        Err(_) => false,
    }
}

fn http_get_json(url: &str) -> Option<serde_json::Value> {
    probe_agent()
        .get(url)
        .call()
        .ok()
        .and_then(|r| r.into_json::<serde_json::Value>().ok())
}

fn http_post_json(url: &str, body: serde_json::Value) -> Option<serde_json::Value> {
    voice_agent()
        .post(url)
        .send_json(body)
        .ok()?
        .into_json()
        .ok()
}

fn repo_identity(workspace: &Path) -> Identity {
    let identity = crate::platform::workspace_store::repo_identity(workspace);
    Identity {
        root: identity.root,
        key: identity.key,
    }
}

/// Lend the realm host the cockpit's answers. The first call wins.
pub(crate) fn install() {
    angel_realm_host::hooks::install(Hooks {
        ambient_stage_sim_allowed: crate::drive::comp_mode::ambient_stage_sim_allowed,
        stage_current: crate::ui::viz::agentviz::current,
        stage_clear: crate::ui::viz::agentviz::clear,
        repo_identity,
        http_probe,
        http_get_json,
        http_post_json,
    });
}
