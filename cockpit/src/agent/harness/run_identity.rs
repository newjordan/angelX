//! Immutable first-request provenance plus caller-local effective request identity.
use crate::agent::sandbox::process_owner::OwnedCommandExt;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::path::Path;
use std::sync::OnceLock;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub(crate) struct StaticIdentity {
    pub executable_sha256: String,
    pub executable_path: String,
    pub cockpit_source_sha256: String,
    pub toolchain: Toolchain,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub(crate) struct Toolchain {
    pub rustc: String,
    pub host: String,
    pub profile: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub(crate) struct Dataset {
    pub kind: String,
    pub path: Option<String>,
    pub sha256: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub(crate) struct RunIdentity {
    #[serde(flatten)]
    pub build: StaticIdentity,
    /// Sealed sandbox posture bound to the run (`None` on ordinary runs).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sandbox: Option<Value>,
    pub model: Model,
    /// Exact wire controls, including an explicit `none` when no control is sent.
    pub effort: Value,
    pub dataset: Dataset,
    pub budgets: Value,
    pub verifier: Value,
    pub bound_at_ms: u64,
    /// Original process provenance, retained when emitting the effective request.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub initial_request: Option<Box<RunIdentity>>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub(crate) struct Model {
    pub club: String,
    pub id: String,
    /// Host, optional port, and path only: credentials/query/fragment omitted.
    pub base_url: String,
    pub driver: String,
}

static BUILD: OnceLock<Result<StaticIdentity, String>> = OnceLock::new();
static IDENTITY: OnceLock<Result<RunIdentity, String>> = OnceLock::new();
static DATASET: OnceLock<Dataset> = OnceLock::new();
static VERIFIER: OnceLock<Value> = OnceLock::new();
thread_local! {
    static LIVE_MODEL: std::cell::RefCell<Option<String>> = const { std::cell::RefCell::new(None) };
    static MODEL_DEFAULTS: std::cell::RefCell<Option<Value>> = const { std::cell::RefCell::new(None) };
    static TURN_BUDGETS: std::cell::RefCell<Option<Value>> = const { std::cell::RefCell::new(None) };
    static REQUEST_IDENTITY: std::cell::RefCell<Option<RunIdentity>> = const { std::cell::RefCell::new(None) };
    static TURN_IDENTITY: std::cell::RefCell<Option<RunIdentity>> = const { std::cell::RefCell::new(None) };
    static TURN_ACTIVE: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
}

pub(crate) fn reset_turn_identity() {
    TURN_ACTIVE.with(|slot| slot.set(true));
    REQUEST_IDENTITY.with(|slot| slot.borrow_mut().take());
    TURN_IDENTITY.with(|slot| slot.borrow_mut().take());
}

/// Exact latest request on this caller, without borrowing another worker's seat.
pub(crate) fn request_identity() -> Option<RunIdentity> {
    REQUEST_IDENTITY.with(|slot| slot.borrow().clone())
}

pub(crate) fn begin_request() {
    REQUEST_IDENTITY.with(|slot| slot.borrow_mut().take());
}

/// A wrapper may return a different seat, or a synthesized answer with no one
/// certified seat. Never attach the last attempted seat's endpoint in that case.
pub(crate) fn select_answer_route(route: &crate::agent::club::RouteIdentity) {
    let request = request_identity().filter(|identity| {
        identity.model.driver == route.driver
            && Some(identity.model.id.as_str()) == route.model.as_deref()
    });
    let identity = request.or_else(|| {
        capture(
            Model {
                club: route.driver.clone(),
                driver: route.driver.clone(),
                id: route.model.clone().unwrap_or_else(|| "unbound".into()),
                base_url: "unbound".into(),
            },
            json!("unbound"),
            json!("unbound"),
        )
        .ok()
    });
    TURN_IDENTITY.with(|slot| *slot.borrow_mut() = identity);
}

/// Executing seat identity, independent of immutable first-request provenance.
pub(crate) struct LiveModelScope(Option<String>);

impl LiveModelScope {
    pub(crate) fn enter(model: Option<String>) -> Self {
        if model.is_none() {
            ANSWER_ROUTES.with(|routes| routes.borrow_mut().clear());
        }
        Self(LIVE_MODEL.with(|slot| slot.replace(model.filter(|m| !m.trim().is_empty()))))
    }
}

impl Drop for LiveModelScope {
    fn drop(&mut self) {
        LIVE_MODEL.with(|slot| slot.replace(self.0.take()));
    }
}

pub(crate) fn live_model() -> Option<String> {
    LIVE_MODEL.with(|slot| slot.borrow().clone())
}

thread_local! {
    static LIVE_TURN: std::cell::RefCell<Option<String>> = const { std::cell::RefCell::new(None) };
}

pub(crate) struct LiveTurnScope(Option<String>);
impl LiveTurnScope {
    pub(crate) fn enter(turn: Option<String>) -> Self {
        Self(LIVE_TURN.with(|slot| slot.replace(turn)))
    }
}
impl Drop for LiveTurnScope {
    fn drop(&mut self) {
        LIVE_TURN.with(|slot| slot.replace(self.0.take()));
    }
}
pub(crate) fn live_turn() -> Option<String> {
    LIVE_TURN.with(|slot| slot.borrow().clone())
}

// Answer receipts are caller-thread-local, not shared "last winner" state:
// concurrent users of one logical club must not borrow one another's seat.
thread_local! {
    static ANSWER_ROUTES: std::cell::RefCell<std::collections::HashMap<usize, crate::agent::club::RouteIdentity>> = std::cell::RefCell::new(std::collections::HashMap::new());
}

pub(crate) fn publish_answer_route(key: usize, route: crate::agent::club::RouteIdentity) {
    ANSWER_ROUTES.with(|routes| {
        routes.borrow_mut().insert(key, route);
    });
}

pub(crate) fn answer_route(key: usize) -> Option<crate::agent::club::RouteIdentity> {
    ANSWER_ROUTES.with(|routes| routes.borrow().get(&key).cloned())
}

pub(crate) fn prepare_model_defaults(budgets: Value) {
    MODEL_DEFAULTS.with(|slot| *slot.borrow_mut() = Some(budgets));
}

pub(crate) fn source_sha256() -> &'static str {
    option_env!("ANGEL_BUILD_SOURCE_SHA256").unwrap_or("unbound")
}

/// Hash the running executable. The system hasher is preferred: it is C-fast
/// under every profile, while the in-process hasher (`cut::sha256_hex`) is pure
/// Rust and takes tens of seconds on a debug-profile test binary. The fallback
/// keeps the identity bindable on hosts without coreutils.
fn executable_sha256(path: &std::path::Path) -> Result<String, String> {
    if let Ok(out) = std::process::Command::new("sha256sum")
        .arg(path)
        .output_owned()
        && out.status.success()
        && let Some(hex) = String::from_utf8_lossy(&out.stdout)
            .split_whitespace()
            .next()
        && hex.len() == 64
        && hex.bytes().all(|b| b.is_ascii_hexdigit())
    {
        return Ok(hex.to_string());
    }
    let bytes = std::fs::read(path).map_err(|e| format!("run identity executable hash: {e}"))?;
    Ok(crate::knowledge::cut::sha256_hex(&bytes))
}

/// Start hashing the executable off the request path so the first model
/// request never waits on it (the `OnceLock` serializes any concurrent caller
/// behind this thread instead of hashing twice).
pub(crate) fn prewarm() {
    std::thread::Builder::new()
        .name("run-identity-prewarm".into())
        .spawn(|| {
            let _ = static_identity();
        })
        .ok();
}

pub(crate) fn static_identity() -> Result<&'static StaticIdentity, String> {
    BUILD
        .get_or_init(|| {
            let path =
                std::env::current_exe().map_err(|e| format!("run identity executable: {e}"))?;
            let executable_sha256 = executable_sha256(&path)?;
            let rustc = option_env!("ANGEL_BUILD_RUSTC").unwrap_or("unbound");
            let host = rustc
                .lines()
                .find_map(|line| line.strip_prefix("host: "))
                .unwrap_or("unbound");
            Ok(StaticIdentity {
                executable_sha256,
                executable_path: path.to_string_lossy().into_owned(),
                cockpit_source_sha256: source_sha256().into(),
                toolchain: Toolchain {
                    rustc: rustc.into(),
                    host: host.into(),
                    profile: option_env!("ANGEL_BUILD_PROFILE")
                        .unwrap_or("unbound")
                        .into(),
                },
            })
        })
        .as_ref()
        .map_err(Clone::clone)
}

impl Dataset {
    pub(crate) fn new(kind: &str, path: Option<&Path>) -> Result<Self, String> {
        Ok(Self {
            kind: kind.into(),
            path: path.map(|p| p.to_string_lossy().into_owned()),
            sha256: path
                .map(|p| std::fs::read(p).map(|bytes| crate::knowledge::cut::sha256_hex(&bytes)))
                .transpose()
                .map_err(|e| format!("run identity dataset hash: {e}"))?,
        })
    }
}

pub(crate) fn configure_dataset(dataset: Dataset) {
    let _ = DATASET.set(dataset);
}

pub(crate) fn configure_verifier(verifier: Value) {
    let _ = VERIFIER.set(verifier);
}

pub(crate) fn prepare_verifier(workspace: &Path, external_only: bool, accept_cmd: Option<&str>) {
    if VERIFIER.get().is_some() {
        return;
    }
    let verifier = if external_only {
        json!({"plan_kind":"external-only", "command":"none", "accept_cmd":"none", "evaluator_id":"none"})
    } else {
        let plan = crate::platform::workspace_lang::plan_tests(
            workspace,
            &crate::platform::workspace_lang::detect(workspace),
            None,
        );
        json!({
            "plan_kind": if accept_cmd.is_some() { "accept_cmd" } else if plan.is_some() { "workspace-language" } else { "none" },
            "command": plan.map(|p| json!({"program":p.program, "args":p.args, "directory":p.dir})).unwrap_or(json!("none")),
            "accept_cmd": accept_cmd.unwrap_or("none"), "evaluator_id":"none"
        })
    };
    configure_verifier(verifier);
}

pub(crate) fn prepare_turn(
    max_hops: Option<usize>,
    compaction: usize,
    keep: usize,
    keep_tokens: usize,
) {
    TURN_BUDGETS.with(|slot| {
        let mut budgets = json!({
            "max_hops": max_hops.filter(|n| *n != usize::MAX).unwrap_or(0),
            "compaction_budget_tokens": compaction,
            "compact_keep_recent": keep,
            "compact_keep_recent_tokens": keep_tokens,
        });
        super::formation_budget::identity(&mut budgets);
        *slot.borrow_mut() = Some(budgets);
    });
}

pub(crate) fn endpoint_identity(endpoint: &str) -> String {
    let Ok(url) = url::Url::parse(endpoint) else {
        return "unbound".into();
    };
    let Some(host) = url.host_str() else {
        return "unbound".into();
    };
    let identity = format!(
        "{host}{}{}",
        url.port().map(|p| format!(":{p}")).unwrap_or_default(),
        url.path()
    );
    if url.scheme() == "http" && is_loopback_host(host) {
        format!("{identity} insecure-local")
    } else {
        identity
    }
}

fn is_loopback_host(host: &str) -> bool {
    host.eq_ignore_ascii_case("localhost")
        || host
            .parse::<std::net::IpAddr>()
            .is_ok_and(|ip| ip.is_loopback())
}

pub(crate) fn wire_effort(body: &Value) -> Value {
    let mut controls = serde_json::Map::new();
    for key in ["reasoning_effort", "thinking", "reasoning"] {
        if let Some(value) = body.get(key) {
            controls.insert(key.into(), value.clone());
        }
    }
    if let Some(value) = body.pointer("/chat_template_kwargs/enable_thinking") {
        controls.insert("enable_thinking".into(), value.clone());
    }
    if controls.is_empty() {
        json!("none")
    } else {
        Value::Object(controls)
    }
}

/// Parse just request controls; skip potentially large message/tool payloads
/// without allocating them. The returned object contains no prompt or secrets.
pub(crate) fn wire_identity_controls(bytes: &[u8]) -> Result<Value, serde_json::Error> {
    struct Controls;
    impl<'de> serde::de::Visitor<'de> for Controls {
        type Value = Value;
        fn expecting(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            formatter.write_str("a request object")
        }
        fn visit_map<A: serde::de::MapAccess<'de>>(self, mut map: A) -> Result<Value, A::Error> {
            let mut controls = serde_json::Map::new();
            while let Some(key) = map.next_key::<String>()? {
                if matches!(
                    key.as_str(),
                    "model"
                        | "reasoning_effort"
                        | "thinking"
                        | "reasoning"
                        | "chat_template_kwargs"
                        | "max_tokens"
                        | "max_completion_tokens"
                        | "max_output_tokens"
                ) {
                    controls.insert(key, map.next_value()?);
                } else {
                    map.next_value::<serde::de::IgnoredAny>()?;
                }
            }
            Ok(Value::Object(controls))
        }
    }
    let mut deserializer = serde_json::Deserializer::from_slice(bytes);
    let controls = serde::Deserializer::deserialize_map(&mut deserializer, Controls)?;
    deserializer.end()?;
    Ok(controls)
}

fn capture(model: Model, effort: Value, output_tokens: Value) -> Result<RunIdentity, String> {
    let mut budgets = TURN_BUDGETS
        .with(|slot| slot.borrow().clone())
        .unwrap_or_else(|| {
            json!({
                "max_hops": "unbound", "compaction_budget_tokens": "unbound",
                "compact_keep_recent": "unbound", "compact_keep_recent_tokens": "unbound",
            })
        });
    budgets["turn_idle_timeout_secs"] =
        json!(crate::agent::turn::configured_turn_idle_timeout_secs().unwrap_or(0));
    budgets["stream_stall_secs"] = json!(crate::agent::club::identity_stream_stall_secs());
    budgets["codex_stream_stall_secs"] =
        json!(crate::agent::club::env_secs("ANGEL_CODEX_STREAM_STALL_SECS", 120).as_secs());
    let defaults = MODEL_DEFAULTS
        .with(|slot| slot.borrow_mut().take())
        .unwrap_or_else(|| crate::agent::club::model_defaults::budgets(&model.id, &model.club));
    if let Some(fields) = defaults.as_object() {
        for (key, value) in fields {
            budgets[key] = value.clone();
        }
    }
    super::formation_budget::identity(&mut budgets);
    budgets["output_tokens"] = output_tokens;
    let dataset = DATASET
        .get()
        .cloned()
        .unwrap_or(Dataset::new("interactive", None)?);
    let plan_kind = if matches!(dataset.kind.as_str(), "task" | "task_json") {
        "external-only"
    } else {
        "none"
    };
    Ok(RunIdentity {
        build: static_identity()?.clone(),
        sandbox: crate::agent::sandbox::sealed::identity(),
        model,
        effort,
        dataset,
        budgets,
        verifier: VERIFIER.get().cloned().unwrap_or_else(|| json!({"plan_kind": plan_kind, "command": "none", "accept_cmd": "none", "evaluator_id": "none"})),
        bound_at_ms: std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_err(|e| e.to_string())?
            .as_millis()
            .try_into()
            .map_err(|_| "identity timestamp overflow")?,
        initial_request: None,
    })
}

pub(crate) fn bind(
    model: Model,
    effort: Value,
    output_tokens: Value,
    codex_stall_secs: Option<u64>,
) -> Result<(), String> {
    let mut identity = capture(model, effort, output_tokens)?;
    if let Some(seconds) = codex_stall_secs {
        identity.budgets["codex_stream_stall_secs"] = json!(seconds);
    }
    IDENTITY
        .get_or_init(|| Ok(identity.clone()))
        .as_ref()
        .map_err(Clone::clone)?;
    REQUEST_IDENTITY.with(|slot| *slot.borrow_mut() = Some(identity.clone()));
    // Compaction and other auxiliary requests can run on this thread between
    // policy calls. Their receipts must not replace the turn's answering seat
    // if a deadline or cancellation ends the turn before the next policy call.
    if !TURN_ACTIVE.with(|slot| slot.get()) || super::trajectory::model_request_active() {
        TURN_IDENTITY.with(|slot| *slot.borrow_mut() = Some(identity));
    }
    Ok(())
}

/// Build identity stays immutable, but an emitted turn must show its own opt-in
/// formation limits, including their removal on a later unbounded turn.
pub(crate) fn current_for_turn() -> Option<RunIdentity> {
    let mut identity = TURN_IDENTITY
        .with(|slot| slot.borrow().clone())
        .or_else(|| {
            (!TURN_ACTIVE.with(|slot| slot.get()))
                .then(|| current().cloned())
                .flatten()
        })?;
    identity.initial_request = current().cloned().map(Box::new);
    if let Some(object) = identity.budgets.as_object_mut() {
        object.remove("formation_token_budget");
        object.remove("formation_wall_secs");
    }
    if let Some(budget) = super::formation_budget::snapshot() {
        if let Some(tokens) = budget["token_budget"].as_u64() {
            identity.budgets["formation_token_budget"] = json!(tokens);
        }
        if let Some(wall) = budget["wall_secs"].as_u64() {
            identity.budgets["formation_wall_secs"] = json!(wall);
        }
    }
    Some(identity)
}

pub(crate) fn current() -> Option<&'static RunIdentity> {
    IDENTITY.get().and_then(|result| result.as_ref().ok())
}

pub(crate) fn summary() -> String {
    current_for_turn()
        .map(|id| {
            format!(
                "identity exe={} source={} model={} effort={}",
                id.build
                    .executable_sha256
                    .chars()
                    .take(12)
                    .collect::<String>(),
                id.build
                    .cockpit_source_sha256
                    .chars()
                    .take(12)
                    .collect::<String>(),
                id.model.id,
                id.effort
            )
        })
        .unwrap_or_else(|| "identity: not bound (no request launched)".into())
}

#[cfg(test)]
#[path = "../../../../tests/cockpit/harness/run_identity__tests.rs"]
mod tests;
