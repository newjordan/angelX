use super::*;
use crate::club::{ChatRole, ClubReply, Metadata, ToolCall};
use crate::tools::file::{FileOp, locate_replacement, parse_freeform_patch};
use crate::tools::git::{git_diff_argv, git_log_argv, summarize_git_status, validate_git_revision};
use crate::tools::nav::{
    fuzzy_score, glob_match, grep_fallback, line_defines, rank_paths, word_present,
};
use crate::tools::web::{WebSearchTool, format_searxng};
use std::ffi::OsString;
use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};

pub(crate) struct EnvGuard {
    key: &'static str,
    old: Option<OsString>,
}

impl EnvGuard {
    pub(crate) fn set(key: &'static str, value: &str) -> Self {
        let old = std::env::var_os(key);
        // TODO: Audit that the environment access only happens in single-threaded code.
        unsafe { std::env::set_var(key, value) };
        Self { key, old }
    }

    pub(crate) fn unset(key: &'static str) -> Self {
        let old = std::env::var_os(key);
        // TODO: Audit that the environment access only happens in single-threaded code.
        unsafe { std::env::remove_var(key) };
        Self { key, old }
    }
}

impl Drop for EnvGuard {
    fn drop(&mut self) {
        if let Some(old) = &self.old {
            // TODO: Audit that the environment access only happens in single-threaded code.
            unsafe { std::env::set_var(self.key, old) };
        } else {
            // TODO: Audit that the environment access only happens in single-threaded code.
            unsafe { std::env::remove_var(self.key) };
        }
    }
}

/// A scripted club: first chat → call `reverse`, second chat → final text.
struct ScriptedClub {
    hops: AtomicUsize,
}
impl Club for ScriptedClub {
    fn respond(&self, _p: &str) -> Result<String, String> {
        Ok("unused".into())
    }
    fn label(&self) -> &str {
        "scripted"
    }
    fn chat(&self, messages: &[ChatMsg], _tools: &[ToolDef]) -> Result<ClubReply, String> {
        if self.hops.fetch_add(1, Ordering::SeqCst) == 0 {
            Ok(ClubReply::Calls(vec![ToolCall {
                id: "c1".into(),
                name: "reverse".into(),
                args: serde_json::json!({ "text": "hello" }),
            }]))
        } else {
            let last_tool = messages
                .iter()
                .rev()
                .find(|m| m.role == ChatRole::Tool)
                .map(|m| m.content.clone())
                .unwrap_or_default();
            Ok(ClubReply::Text(format!("done: {last_tool}")))
        }
    }
}

/// Temp dir for harness tests.
///
/// Unique per call so parallel tests that share a tag cannot collide under the
/// same process id (which previously flaked rollout capture inventory).
fn scratch(tag: &str) -> PathBuf {
    use std::sync::atomic::{AtomicU64, Ordering};
    static SCRATCH_SEQ: AtomicU64 = AtomicU64::new(0);
    let seq = SCRATCH_SEQ.fetch_add(1, Ordering::Relaxed);
    let d = std::env::temp_dir().join(format!("angel_sc_{tag}_{}_{seq}", std::process::id()));
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).unwrap();
    d
}

/// Tiny ToolCall builder for schedule fixtures (shared).
fn tc(name: &str, args: Value) -> ToolCall {
    ToolCall {
        id: "x".into(),
        name: name.into(),
        args,
    }
}

/// A club that returns a fixed summary from `respond` (empty string => error),
/// for exercising auto-compaction without a live model.
struct SummarizerClub(&'static str);
impl Club for SummarizerClub {
    fn respond(&self, _p: &str) -> Result<String, String> {
        if self.0.is_empty() {
            Err("summarizer unavailable".to_string())
        } else {
            Ok(self.0.to_string())
        }
    }
    fn label(&self) -> &str {
        "summarizer"
    }
}

struct PanickingSummarizerClub;
impl Club for PanickingSummarizerClub {
    fn respond(&self, _p: &str) -> Result<String, String> {
        panic!("synthetic summarizer panic")
    }
    fn label(&self) -> &str {
        "panicking-summarizer"
    }
}

fn long_history() -> Vec<ChatMsg> {
    let mut h = vec![ChatMsg::system(
        "you are a helpful agent with a fairly long preamble",
    )];
    for i in 0..8 {
        h.push(ChatMsg::user(format!(
            "user message number {i} with some length to it"
        )));
        h.push(ChatMsg::assistant_calls(vec![ToolCall {
            id: format!("c{i}"),
            name: "read_file".into(),
            args: serde_json::json!({ "path": "x" }),
        }]));
        h.push(ChatMsg::tool(
            format!("c{i}"),
            format!("tool result number {i} goes here"),
        ));
        h.push(ChatMsg::assistant(format!(
            "assistant answer number {i} text here"
        )));
    }
    h
}

/// A bare registry carrying `store` — what `maybe_compact` reads its store,
/// session id, and (empty) aux-club list from in these tests.
fn registry_with_store(
    store: std::sync::Arc<dyn crate::memory_store::MemoryStore>,
) -> ToolRegistry {
    let mut reg = ToolRegistry::new();
    reg.set_memory_store(store);
    reg
}

fn tool_hop_history(hops: usize, payload: &str) -> Vec<ChatMsg> {
    let mut history = vec![ChatMsg::system("sys"), ChatMsg::user("task")];
    for i in 0..hops {
        history.push(ChatMsg::assistant_calls(vec![ToolCall {
            id: format!("c{i}"),
            name: "read_file".to_string(),
            args: serde_json::json!({ "path": format!("src/file-{i}.rs") }),
        }]));
        history.push(ChatMsg::tool(format!("c{i}"), payload));
    }
    history
}

mod adversarial_io;
mod bg_compact;
mod cargo_tool;
mod code_mode;
mod comp_watch;
mod context_compact;
mod context_recall;
mod delegate;
mod delegated_lineage;
mod descendant_budget;
mod file_tools;
mod git_tools;
mod hooks;
mod hop_progress;
mod independence_gate;
mod interception_seats;
mod k5_cargo_yolo;
mod kit_registry;
mod mcp_provider_lifecycle;
mod mutation_signatures;
mod nav_tools;
mod patch_edit;
mod presentation;
mod project_docs;
mod reactive_activation;
mod registration_inverse;
mod run_turn;
mod sandbox_exec;
mod sandbox_policy;
mod scenarios;
mod schedule;
mod skills;
mod telemetry;
mod tool_aging;
mod trajectory_moa;
mod verification;
mod verification_targets;
mod web;

#[test]
fn lifecycle_idle_timeout_has_truthful_stop_and_cancelled_tool_receipt() {
    let _lock = crate::tests::env_lock();
    let _env = [
        EnvGuard::set("ANGEL_YOLO", "1"),
        EnvGuard::set("ANGEL_BACKPLANE", "0"),
        EnvGuard::set("ANGEL_TURN_IDLE_TIMEOUT_SECS", "1"),
        EnvGuard::set("ANGEL_TOOL_IDLE_SECS", "30"),
        EnvGuard::set("ANGEL_SKILL_HINT", "0"),
        EnvGuard::set("ANGEL_CADDY", "0"),
        EnvGuard::set("ANGEL_ATLAS", "0"),
        EnvGuard::set("ANGEL_EXPERIENCE", "0"),
        EnvGuard::set("ANGEL_PROJECT_DOC", "0"),
        EnvGuard::set("ANGEL_TASK_RECON", "0"),
        EnvGuard::set("ANGEL_MEMPALACE", "0"),
        EnvGuard::set("ANGEL_TRAJECTORY_LOG", "0"),
        EnvGuard::set("ANGEL_ROLLOUT_CAPTURE", "0"),
        EnvGuard::set("ANGEL_POLL_GUARD", "0"),
    ];
    struct LocalScript;
    impl Club for LocalScript {
        fn label(&self) -> &str {
            "model-free-lifecycle"
        }
        fn respond(&self, _: &str) -> Result<String, String> {
            Err("no model".into())
        }
        fn chat(&self, _: &[ChatMsg], _: &[ToolDef]) -> Result<ClubReply, String> {
            Ok(ClubReply::Calls(vec![ToolCall {
                id: "silent".into(),
                name: "shell".into(),
                args: serde_json::json!({"command":"echo $$ > worker.pid; sleep 8"}),
            }]))
        }
    }
    let dir = scratch("idle-process");
    // Initialize the process helper before measuring a one-second idle window.
    ShellTool::in_dir(dir.clone())
        .call(&serde_json::json!({"command": "true"}))
        .unwrap();
    let mut registry = ToolRegistry::new();
    registry.set_workspace(dir.clone());
    registry.register(Box::new(ShellTool::in_dir(dir.clone())));
    let (tx, rx) = mpsc::channel();
    let outcome = run_turn_observed(
        &LocalScript,
        &registry,
        &mut vec![ChatMsg::user("silent local fixture")],
        &AtomicBool::new(false),
        Some(2),
        &tx,
    )
    .unwrap();
    assert_eq!(outcome.stop_reason, TurnStopReason::IdleTimeout);
    assert_eq!(outcome.stop_reason.as_str(), "idle_timeout");
    assert!(outcome.answer.contains("1s without progress"));
    assert!(rx.try_iter().any(|event| matches!(
        event,
        TurnEvent::ToolResult {
            outcome: ToolOutcome {
                execution: ExecutionOutcome::Cancelled,
                ..
            },
            ..
        }
    )));
    let worker_pid: u32 = std::fs::read_to_string(dir.join("worker.pid"))
        .unwrap()
        .trim()
        .parse()
        .unwrap();
    assert!(
        !Path::new(&format!("/proc/{worker_pid}")).exists(),
        "idle-timeout worker was not reaped"
    );
    eprintln!(
        "LIFECYCLE_IDLE_RECEIPT stop_reason=idle_timeout tool=Cancelled worker_pid={worker_pid} reaped=true"
    );
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn t06c_tool_idle_escalation_is_failed_and_model_can_recover() {
    let _lock = crate::tests::env_lock();
    let _env = [
        EnvGuard::set("ANGEL_YOLO", "1"),
        EnvGuard::set("ANGEL_BACKPLANE", "0"),
        EnvGuard::set("ANGEL_TOOL_IDLE_SECS", "1"),
        EnvGuard::set("ANGEL_TURN_IDLE_TIMEOUT_SECS", "1"),
        EnvGuard::set("ANGEL_TOOL_IDLE_FLOOR_SECS", "0"),
        EnvGuard::set("ANGEL_SKILL_HINT", "0"),
        EnvGuard::set("ANGEL_CADDY", "0"),
        EnvGuard::set("ANGEL_ATLAS", "0"),
        EnvGuard::set("ANGEL_EXPERIENCE", "0"),
        EnvGuard::set("ANGEL_PROJECT_DOC", "0"),
        EnvGuard::set("ANGEL_TASK_RECON", "0"),
        EnvGuard::set("ANGEL_MEMPALACE", "0"),
        EnvGuard::set("ANGEL_TRAJECTORY_LOG", "0"),
        EnvGuard::set("ANGEL_ROLLOUT_CAPTURE", "0"),
    ];
    struct LocalScript;
    impl Club for LocalScript {
        fn label(&self) -> &str {
            "model-free-stdin"
        }
        fn respond(&self, _: &str) -> Result<String, String> {
            Err("no model".into())
        }
        fn chat(&self, history: &[ChatMsg], _: &[ToolDef]) -> Result<ClubReply, String> {
            if history
                .iter()
                .any(|m| m.content.contains("tool error: tool_idle:"))
            {
                return Ok(ClubReply::Text(
                    "Recovered from the tool idle escalation.".into(),
                ));
            }
            Ok(ClubReply::Calls(vec![ToolCall {
                id: "stdin-idle".into(),
                name: "shell".into(),
                // Plain cat must see EOF; a deliberately supplied pipe recreates
                // the observed stdin wait without regressing the null default.
                args: serde_json::json!({"command":"echo $$ > worker.pid; cat < <(sleep 8)", "read_only":false}),
            }]))
        }
    }
    let dir = scratch("t06c-idle");
    // Measure idle recovery after process-helper initialization, not cold startup.
    ShellTool::in_dir(dir.clone())
        .call(&serde_json::json!({"command": "true"}))
        .unwrap();
    let mut registry = ToolRegistry::new();
    registry.set_workspace(dir.clone());
    registry.register(Box::new(ShellTool::in_dir(dir.clone())));
    let (tx, rx) = mpsc::channel();
    let started = Instant::now();
    let outcome = run_turn_observed(
        &LocalScript,
        &registry,
        &mut vec![ChatMsg::user("Exercise the silent stdin fixture")],
        &AtomicBool::new(false),
        Some(3),
        &tx,
    )
    .unwrap();
    assert!(outcome.answer.contains("Recovered"), "{}", outcome.answer);
    assert!(started.elapsed() < Duration::from_secs(3));
    let ledger = trajectory::progress_ledger_snapshot();
    assert!(
        ledger["escalations"]
            .as_array()
            .unwrap()
            .iter()
            .any(|row| row["kind"] == "tool_idle" && row["hop"] == 1)
    );
    let events: Vec<_> = rx.try_iter().collect();
    assert!(events.iter().any(|event| matches!(
        event,
        TurnEvent::ToolResult {
            outcome: ToolOutcome {
                execution: ExecutionOutcome::Failed,
                ..
            },
            ..
        }
    )));
    assert!(
        events
            .iter()
            .any(|event| matches!(event, TurnEvent::Notice(note) if note.contains("tool_idle")))
    );
    let pid = std::fs::read_to_string(dir.join("worker.pid")).unwrap();
    assert!(!Path::new(&format!("/proc/{}", pid.trim())).exists());
    eprintln!(
        "T06C_IDLE_RECEIPT elapsed_ms={} execution=Failed recovered=true reaped=true escalations={}",
        started.elapsed().as_millis(),
        ledger["escalations"]
    );
    std::fs::remove_dir_all(dir).unwrap();
}

// Exercise the real turn/deadline/dispatch/envelope seam, not a fabricated row.
fn assert_lifecycle_proc_envelope(cancel_after_launch: bool, provider_failure: bool) {
    let _lock = crate::tests::env_lock();
    let dir = scratch("proc-envelope");
    let _env = [
        EnvGuard::set("ANGEL_YOLO", "1"),
        EnvGuard::set("ANGEL_BACKPLANE", "0"),
        EnvGuard::set("ANGEL_TURN_DEADLINE_SECS", "60"),
        EnvGuard::set("ANGEL_TURN_IDLE_TIMEOUT_SECS", "0"),
        EnvGuard::set(
            "ANGEL_PROC_DIR",
            dir.join("process-store").to_str().unwrap(),
        ),
        EnvGuard::set("ANGEL_SKILL_HINT", "0"),
        EnvGuard::set("ANGEL_CADDY", "0"),
        EnvGuard::set("ANGEL_ATLAS", "0"),
        EnvGuard::set("ANGEL_EXPERIENCE", "0"),
        EnvGuard::set("ANGEL_PROJECT_DOC", "0"),
        EnvGuard::set("ANGEL_TASK_RECON", "0"),
        EnvGuard::set("ANGEL_MEMPALACE", "0"),
        EnvGuard::unset("ANGEL_TRAJECTORY_LOG"),
        EnvGuard::set("ANGEL_ROLLOUT_CAPTURE", "0"),
    ];
    let cancel = AtomicBool::new(false);
    struct Script<'a> {
        cancel: &'a AtomicBool,
        cancel_after_launch: bool,
        provider_failure: bool,
        hops: AtomicUsize,
    }
    impl Club for Script<'_> {
        fn label(&self) -> &str {
            "model-free-proc-envelope"
        }
        fn respond(&self, _: &str) -> Result<String, String> {
            unreachable!()
        }
        fn chat(&self, _: &[ChatMsg], _: &[ToolDef]) -> Result<ClubReply, String> {
            if self.hops.fetch_add(1, Ordering::SeqCst) == 0 {
                return Ok(ClubReply::Calls(vec![ToolCall {
                    id: "launch".into(),
                    name: "proc_run".into(),
                    args: serde_json::json!({"command":"sleep 300", "name":"envelope-fixture"}),
                }]));
            }
            if self.cancel_after_launch {
                self.cancel.store(true, Ordering::Release);
            }
            if self.provider_failure {
                return Err("scripted cancellation".into());
            }
            Ok(ClubReply::Text("fixture answer".into()))
        }
    }
    let club = Script {
        cancel: &cancel,
        cancel_after_launch,
        provider_failure,
        hops: AtomicUsize::new(0),
    };
    let mut registry = ToolRegistry::new();
    registry.set_workspace(dir.clone());
    registry.register(Box::new(crate::tools::proc::ProcRunTool::in_dir(
        dir.clone(),
    )));
    let mut history = vec![ChatMsg::user(
        "launch the local background fixture and answer",
    )];
    let (tx, _rx) = mpsc::channel();
    let result = run_turn_observed(&club, &registry, &mut history, &cancel, Some(3), &tx);
    let ctx = TaskJsonContext {
        task_id: None,
        run_id: None,
        workspace: dir.clone(),
        club: None,
        model: None,
        reasoning_effort: None,
        output_budget: None,
        elapsed_ms: 1,
        timing: None,
        tools: tool_ledger_snapshot(),
        usage: None,
        runtime: None,
        session_id: None,
        artifacts: Vec::new(),
        memory_health: crate::caddy::StoreHealthSummary::default(),
    };
    let envelope = match result {
        Ok(outcome) => TaskJsonEnvelope::from_outcome(ctx, outcome, &history),
        Err(failure) => TaskJsonEnvelope::from_failure(ctx, failure),
    };
    let value = serde_json::to_value(envelope).unwrap();
    let rows = value["tools"].as_array().unwrap();
    assert_eq!(rows.len(), 1, "one row for the actual launch: {value}");
    assert_eq!(rows[0]["tool"], "proc_run");
    if cancel_after_launch {
        assert_eq!(rows[0]["status"], "killed");
        assert_eq!(rows[0]["kill"]["signal"], libc::SIGTERM);
        assert_eq!(rows[0]["kill"]["owner"], "turn_owner");
        assert_eq!(rows[0]["kill"]["owner_pid"], std::process::id());
        assert_eq!(rows[0]["kill"]["reason"], "cancelled");
    } else {
        assert_eq!(rows[0]["status"], "ok");
        assert!(rows[0]["kill"].is_null());
        crate::tools::proc::ProcStopTool::new(dir.clone())
            .call(&serde_json::json!({"id": rows[0]["proc_id"]}))
            .unwrap();
    }
    assert_eq!(
        value["stop_reason"],
        if cancel_after_launch {
            "cancelled"
        } else {
            "answer"
        }
    );
    if !cancel_after_launch {
        assert_eq!(value["status"], "completed");
        assert_eq!(value["answer"], "fixture answer");
    }
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn lifecycle_proc_deadline_wrapper_answer_preserves_job() {
    assert_lifecycle_proc_envelope(false, false);
}

#[test]
fn lifecycle_proc_deadline_wrapper_cancel_reaches_envelope() {
    assert_lifecycle_proc_envelope(true, false);
}

#[test]
fn lifecycle_proc_deadline_wrapper_provider_failure_keeps_kill() {
    assert_lifecycle_proc_envelope(true, true);
}
