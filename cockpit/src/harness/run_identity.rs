//! Immutable identity of the first request in this process. Subsequent route
//! changes remain in the existing per-turn route/rollout receipts.
use crate::sandbox::process_owner::OwnedCommandExt;
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
    static ANSWER_ROUTES: std::cell::RefCell<std::collections::HashMap<usize, crate::club::RouteIdentity>> = std::cell::RefCell::new(std::collections::HashMap::new());
}

pub(crate) fn publish_answer_route(key: usize, route: crate::club::RouteIdentity) {
    ANSWER_ROUTES.with(|routes| {
        routes.borrow_mut().insert(key, route);
    });
}

pub(crate) fn answer_route(key: usize) -> Option<crate::club::RouteIdentity> {
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
    Ok(crate::cut::sha256_hex(&bytes))
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
                .map(|p| std::fs::read(p).map(|bytes| crate::cut::sha256_hex(&bytes)))
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
        let plan = crate::workspace_lang::plan_tests(
            workspace,
            &crate::workspace_lang::detect(workspace),
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
        json!(crate::turn::configured_turn_idle_timeout_secs().unwrap_or(0));
    budgets["stream_stall_secs"] = json!(crate::club::identity_stream_stall_secs());
    budgets["codex_stream_stall_secs"] =
        json!(crate::club::env_secs("ANGEL_CODEX_STREAM_STALL_SECS", 120).as_secs());
    let defaults = MODEL_DEFAULTS
        .with(|slot| slot.borrow_mut().take())
        .unwrap_or_else(|| crate::club::model_defaults::budgets(&model.id, &model.club));
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
        sandbox: crate::sandbox::sealed::identity(),
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
    })
}

pub(crate) fn bind(
    model: Model,
    effort: Value,
    output_tokens: Value,
    codex_stall_secs: Option<u64>,
) -> Result<(), String> {
    IDENTITY
        .get_or_init(|| {
            let mut identity = capture(model, effort, output_tokens)?;
            if let Some(seconds) = codex_stall_secs {
                identity.budgets["codex_stream_stall_secs"] = json!(seconds);
            }
            Ok(identity)
        })
        .as_ref()
        .map(|_| ())
        .map_err(Clone::clone)
}

/// Build identity stays immutable, but an emitted turn must show its own opt-in
/// formation limits, including their removal on a later unbounded turn.
pub(crate) fn current_for_turn() -> Option<RunIdentity> {
    let mut identity = current()?.clone();
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
    current()
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
mod tests {
    use super::*;

    #[test]
    fn run_identity_executable_and_unbound() {
        let _env = crate::tests::env_lock();
        let build = static_identity().unwrap();
        let output = std::process::Command::new("sha256sum")
            .arg(std::env::current_exe().unwrap())
            .output()
            .unwrap();
        assert!(output.status.success());
        assert_eq!(
            build.executable_sha256,
            String::from_utf8(output.stdout)
                .unwrap()
                .split_whitespace()
                .next()
                .unwrap()
        );
        assert_eq!(
            build.cockpit_source_sha256,
            option_env!("ANGEL_BUILD_SOURCE_SHA256").unwrap_or("unbound")
        );
        assert_eq!(
            build.toolchain.profile,
            option_env!("ANGEL_BUILD_PROFILE").unwrap_or("unbound")
        );
    }

    #[test]
    fn model_defaults_identity_capture() {
        let _env = crate::tests::env_lock();
        let _idle = crate::tests::TestEnvGuard::unset("ANGEL_TURN_IDLE_TIMEOUT_SECS");
        let _stall = crate::tests::TestEnvGuard::unset("ANGEL_STREAM_STALL_SECS");
        let _effort = crate::tests::TestEnvGuard::unset("ANGEL_REASONING_EFFORT");
        let _grok = crate::tests::TestEnvGuard::unset("ANGEL_GROK_REASONING_EFFORT");
        let identity = capture(
            Model {
                club: "grok".into(),
                id: "grok-4.6".into(),
                base_url: "127.0.0.1".into(),
                driver: "grok".into(),
            },
            json!({"reasoning_effort":"low"}),
            json!(123),
        )
        .unwrap();
        assert_eq!(identity.budgets["reasoning_effort"], "low");
        assert_eq!(
            identity.budgets["reasoning_effort_source"],
            "table 2026-09-09"
        );
        assert_eq!(identity.budgets["stream_stall_secs"], 240);
        assert_eq!(identity.budgets["turn_idle_timeout_secs"], 0);
    }

    #[test]
    fn run_identity_effective_budgets_and_wire_controls() {
        let _env = crate::tests::env_lock();
        let _idle = crate::tests::TestEnvGuard::set("ANGEL_TURN_IDLE_TIMEOUT_SECS", "17");
        prepare_turn(Some(3), 4096, 2, 1024);
        let wire = json!({"reasoning_effort": "low", "thinking": {"type":"enabled"}});
        let id = capture(
            Model {
                club: "scripted".into(),
                id: "fixture".into(),
                base_url: "none".into(),
                driver: "scripted".into(),
            },
            wire_effort(&wire),
            json!(123),
        )
        .unwrap();
        assert_eq!(id.budgets["max_hops"], 3);
        assert_eq!(id.budgets["turn_idle_timeout_secs"], 17);
        assert_eq!(id.budgets["compaction_budget_tokens"], 4096);
        assert_eq!(id.budgets["output_tokens"], 123);
        assert_eq!(id.effort["reasoning_effort"], wire["reasoning_effort"]);
        assert_eq!(wire_effort(&json!({})), "none");
        assert_eq!(
            endpoint_identity("https://user:password@example.test:8443/v1?key=secret#secret"),
            "example.test:8443/v1"
        );
        assert_eq!(
            endpoint_identity("http://127.0.0.1:8080/v1"),
            "127.0.0.1:8080/v1 insecure-local"
        );
        assert_eq!(
            endpoint_identity("http://localhost:11434/v1"),
            "localhost:11434/v1 insecure-local"
        );
    }

    #[test]
    fn run_identity_carries_sealed_sandbox_when_active() {
        let _env = crate::tests::env_lock();
        // Sealed activation is process-permanent (OnceLock), so the active
        // branch runs in a forked test binary of exactly this test.
        if !cfg!(target_os = "linux") {
            eprintln!(
                "SKIP run_identity_carries_sealed_sandbox_when_active: sealed requires Linux Landlock + bwrap; unsupported on {}",
                std::env::consts::OS
            );
            return;
        }
        if std::env::var("ANGEL_T_SEALED_CHILD").as_deref() != Ok("1") {
            let mut cmd = std::process::Command::new(std::env::current_exe().unwrap());
            cmd.args([
                "--exact",
                "harness::run_identity::tests::run_identity_carries_sealed_sandbox_when_active",
                "--nocapture",
            ])
            .env("ANGEL_T_SEALED_CHILD", "1");
            let output = cmd.output().unwrap();
            assert!(
                output.status.success(),
                "{}\n{}",
                String::from_utf8_lossy(&output.stdout),
                String::from_utf8_lossy(&output.stderr)
            );
            return;
        }
        assert!(
            capture(
                Model {
                    club: "scripted".into(),
                    id: "fixture".into(),
                    base_url: "none".into(),
                    driver: "scripted".into(),
                },
                json!("none"),
                json!(123),
            )
            .unwrap()
            .sandbox
            .is_none()
        );
        let profile = crate::sandbox::sealed::build(
            std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
                .parent()
                .unwrap(),
            None,
        );
        crate::sandbox::sealed::tests::test_activate(profile);
        let id = capture(
            Model {
                club: "scripted".into(),
                id: "fixture".into(),
                base_url: "none".into(),
                driver: "scripted".into(),
            },
            json!("none"),
            json!(123),
        )
        .unwrap();
        assert_eq!(id.sandbox.as_ref().unwrap()["name"], "sealed");
        let digest = id.sandbox.as_ref().unwrap()["digest"].as_str().unwrap();
        assert_eq!(digest.len(), 64);
        assert!(digest.bytes().all(|b| b.is_ascii_hexdigit()));
    }

    #[test]
    fn run_identity_http_resolves_requested_none_to_wire_low() {
        let _env = crate::tests::env_lock();
        let club =
            crate::club::HttpClub::new("glm", "https://identity.invalid/v1", "glm-5.3", None);
        let body = club
            .build_body_with_effort(
                &[crate::club::ChatMsg::user("fixture")],
                &[],
                false,
                Some("none"),
            )
            .unwrap();
        // Exercise the real request builder without sending a request or opening a socket.
        assert_eq!(body["reasoning_effort"], "low");
        let id = capture(
            Model {
                club: "glm".into(),
                id: body["model"].as_str().unwrap().into(),
                base_url: endpoint_identity("https://identity.invalid/v1"),
                driver: "glm".into(),
            },
            wire_effort(&body),
            json!("provider-native"),
        )
        .unwrap();
        assert_eq!(id.effort["reasoning_effort"], body["reasoning_effort"]);
        assert_eq!(id.effort["thinking"], body["thinking"]);
    }

    #[test]
    fn run_identity_scripted_task_json() {
        use crate::club::{ChatMsg, Club};
        use crate::harness::{TaskJsonContext, TaskJsonEnvelope, ToolRegistry, TurnEvent};
        let _env = crate::tests::env_lock();
        if std::env::var("ANGEL_T_IDENTITY_CHILD").as_deref() != Ok("1") {
            let root =
                std::env::temp_dir().join(format!("angel-identity-fixture-{}", std::process::id()));
            std::fs::create_dir_all(&root).unwrap();
            let mut cmd = std::process::Command::new(std::env::current_exe().unwrap());
            cmd.args([
                "--exact",
                "harness::run_identity::tests::run_identity_scripted_task_json",
                "--nocapture",
            ])
            .current_dir(&root)
            .env("ANGEL_T_IDENTITY_CHILD", "1")
            .env("ANGEL_EXPERIENCE", "0")
            .env("ANGEL_SKILL_HINT", "0")
            .env("ANGEL_AUTO_RECALL", "0")
            .env("ANGEL_AUTO_COMPACT", "0")
            .env("ANGEL_HARNESS_ROLLOUTS", "off")
            .env("ANGEL_CONTEXT_BUDGET_TOKENS", "4096")
            .env("ANGEL_TRAJECTORY_LOG", "1")
            .env("ANGEL_NEEDS_PRO", "0")
            .env("ANGEL_TURN_IDLE_TIMEOUT_SECS", "17")
            .env_remove("ANGEL_TASK_ACCEPT_CMD")
            .env_remove("ANGEL_FIRST_WRITE_CALLS");
            for key in [
                "ANGEL_ATLAS_DIR",
                "ANGEL_CADDY_DIR",
                "ANGEL_DOSSIER_DIR",
                "ANGEL_TRAJECTORY_DIR",
            ] {
                cmd.env(key, root.join(key));
            }
            for profile in ["ordinary", "sealed"] {
                cmd.env("ANGEL_T_IDENTITY_PROFILE", profile);
                cmd.env("ANGEL_TRAJECTORY_DIR", root.join(profile));
                let output = cmd.output().unwrap();
                assert!(
                    output.status.success(),
                    "{}\n{}",
                    String::from_utf8_lossy(&output.stdout),
                    String::from_utf8_lossy(&output.stderr)
                );
            }
            return;
        }
        let sealed = std::env::var("ANGEL_T_IDENTITY_PROFILE").as_deref() == Ok("sealed");
        if sealed {
            crate::sandbox::sealed::tests::test_activate(crate::sandbox::sealed::build(
                &std::env::current_dir().unwrap(),
                None,
            ));
        }
        struct Scripted;
        impl Club for Scripted {
            fn respond(&self, _: &str) -> Result<String, String> {
                assert!(
                    current().is_some(),
                    "identity must precede the first model request"
                );
                Ok("scripted answer".into())
            }
            fn label(&self) -> &str {
                "identity-scripted"
            }
            fn model_identity(&self) -> Option<String> {
                Some("identity-fixture-v1".into())
            }
        }
        configure_dataset(Dataset::new("task_json", None).unwrap());
        let mut registry = ToolRegistry::new();
        registry.external_evaluator_only = true;
        let mut history = vec![ChatMsg::user("Say scripted answer.")];
        let outcome = crate::harness::run_turn_observed(
            &Scripted,
            &registry,
            &mut history,
            &std::sync::atomic::AtomicBool::new(false),
            Some(3),
            &std::sync::mpsc::channel::<TurnEvent>().0,
        )
        .unwrap();
        let envelope = TaskJsonEnvelope::from_outcome(
            TaskJsonContext {
                task_id: Some("identity-fixture".into()),
                run_id: None,
                workspace: std::env::current_dir().unwrap(),
                club: Some("identity-scripted".into()),
                model: Some("identity-fixture-v1".into()),
                reasoning_effort: None,
                output_budget: None,
                elapsed_ms: 0,
                timing: None,
                tools: vec![],
                usage: None,
                runtime: None,
                session_id: None,
                artifacts: vec![],
                memory_health: crate::caddy::StoreHealthSummary::default(),
            },
            outcome,
            &history,
        );
        let value = serde_json::to_value(&envelope).unwrap();
        let id = &value["identity"];
        assert_eq!(
            value["authority_profile"],
            serde_json::to_value(crate::authority_profile::active(true)).unwrap()
        );
        if sealed {
            assert_eq!(id["sandbox"], crate::sandbox::sealed::identity().unwrap());
        } else {
            assert!(id.get("sandbox").is_none());
        }
        for key in [
            "executable_sha256",
            "executable_path",
            "cockpit_source_sha256",
            "toolchain",
            "model",
            "effort",
            "dataset",
            "budgets",
            "verifier",
            "bound_at_ms",
        ] {
            assert!(!id[key].is_null(), "missing {key}");
        }
        assert_eq!(id["dataset"]["kind"], "task_json");
        assert_eq!(id["budgets"]["max_hops"], 3);
        assert_eq!(id["budgets"]["turn_idle_timeout_secs"], 17);
        assert_eq!(id["budgets"]["compaction_budget_tokens"], 4096);
        assert_eq!(id["verifier"]["plan_kind"], "external-only");
        assert_eq!(
            crate::harness::eval_trajectory_record(
                "identity-scripted",
                &history,
                "scripted answer",
                1.0,
                "fixture-manifest",
                1,
            )["identity"],
            *id
        );
        assert!(crate::harness::ledger_status_text("").contains("model=identity-fixture-v1"));
        let log_dir = std::path::PathBuf::from(std::env::var_os("ANGEL_TRAJECTORY_DIR").unwrap());
        let mut rows = Vec::new();
        for entry in std::fs::read_dir(&log_dir).unwrap().flatten() {
            if entry.path().extension().is_some_and(|ext| ext == "jsonl") {
                for line in std::fs::read_to_string(entry.path()).unwrap().lines() {
                    let row: Value = serde_json::from_str(line).unwrap();
                    assert_eq!(&row["identity"], id);
                    rows.push(row);
                }
            }
        }
        assert!(!rows.is_empty());
        let trace_fixture = std::env::current_dir().unwrap().join("schema-records.json");
        let mut schema_rows = rows.clone();
        schema_rows.push(crate::harness::eval_trajectory_record(
            "identity-scripted",
            &history,
            "scripted answer",
            1.0,
            "fixture-manifest",
            1,
        ));
        std::fs::write(&trace_fixture, serde_json::to_vec(&schema_rows).unwrap()).unwrap();
        let validator =
            std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../scripts/trace_schema.py");
        let checked = std::process::Command::new("python3")
            .arg(validator)
            .arg(&trace_fixture)
            .output()
            .unwrap();
        assert!(
            checked.status.success(),
            "stdout:\n{}\nstderr:\n{}",
            String::from_utf8_lossy(&checked.stdout),
            String::from_utf8_lossy(&checked.stderr)
        );
        if let Some(dir) = std::env::var_os("ANGEL_T_IDENTITY_RECEIPT_DIR") {
            let dir = std::path::PathBuf::from(dir);
            std::fs::write(
                dir.join("example-identity.json"),
                serde_json::to_vec_pretty(id).unwrap(),
            )
            .unwrap();
            std::fs::write(
                dir.join("example-task-result.json"),
                serde_json::to_vec_pretty(&value).unwrap(),
            )
            .unwrap();
            std::fs::write(
                dir.join("example-trajectory.json"),
                serde_json::to_vec_pretty(&rows).unwrap(),
            )
            .unwrap();
        }
    }
    #[test]
    fn run_identity_dataset_file_digest_and_read_failure() {
        let _env = crate::tests::env_lock();
        let path =
            std::env::temp_dir().join(format!("angel-identity-dataset-{}.txt", std::process::id()));
        std::fs::write(&path, b"sealed fixture\n").unwrap();
        let dataset = Dataset::new("arena", Some(&path)).unwrap();
        assert_eq!(
            dataset.sha256.as_deref(),
            Some(crate::cut::sha256_hex(b"sealed fixture\n").as_str())
        );
        assert_eq!(dataset.path.as_deref(), path.to_str());
        std::fs::remove_file(&path).unwrap();
        assert!(Dataset::new("arena", Some(&path)).is_err());
    }
}
