//! Trajectory logging and activity-trace summaries.

mod cost;
mod parking;
pub(crate) use parking::{park_compaction_window, park_tool_result};
pub(crate) mod trace_schema;
pub(crate) use trace_schema::WorkspaceStartScope;

mod rotation;

use super::*;

fn push_truncated_chars(out: &mut String, text: &str, max_chars: usize) {
    for (count, ch) in text.chars().enumerate() {
        if count >= max_chars {
            break;
        }
        out.push(ch);
    }
}

pub(crate) fn summarize_args(args: &Value) -> String {
    if let Some(command) = args.get("command").and_then(Value::as_str) {
        return crate::platform::secrets::redact_str(command)
            .chars()
            .take(200)
            .collect();
    }
    match args {
        Value::Object(map) => {
            let mut out = String::new();
            for (index, (key, value)) in map.iter().take(3).enumerate() {
                if index > 0 {
                    out.push_str(", ");
                }
                out.push_str(key);
                out.push('=');
                match value {
                    Value::String(s) => push_truncated_chars(&mut out, s, 60),
                    other => {
                        let rendered = other.to_string();
                        push_truncated_chars(&mut out, &rendered, 60);
                    }
                }
            }
            out
        }
        other => other.to_string().chars().take(80).collect(),
    }
}

pub(crate) fn summarize_result(result: &str) -> String {
    result
        .lines()
        .next()
        .unwrap_or("(empty)")
        .chars()
        .take(80)
        .collect()
}

pub fn default_max_hops() -> usize {
    match std::env::var("ANGEL_MAX_HOPS")
        .ok()
        .and_then(|s| s.trim().parse::<usize>().ok())
    {
        Some(0) => usize::MAX,
        Some(n) => n,
        None => usize::MAX,
    }
}

fn trajectory_messages(history: &[ChatMsg]) -> Vec<Value> {
    crate::agent::club::messages_to_json(history, true)
        .into_iter()
        .zip(history)
        .map(|(mut wire, message)| {
            let origin = match message.role {
                ChatRole::User => Some("operator"),
                ChatRole::Harness => Some("harness"),
                _ => None,
            };
            if let (Some(origin), Some(object)) = (origin, wire.as_object_mut()) {
                object.insert("origin".to_string(), Value::String(origin.to_string()));
            }
            wire
        })
        .collect()
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub(crate) struct LastRootHiq {
    pub offload_ratio: f64,
    pub hiq_priority: f64,
    pub handle_receipts: usize,
    pub aged_receipts: usize,
    pub bulk_tool_results: usize,
    pub strategy_token_n: usize,
}

static LAST_ROOT_HIQ: std::sync::Mutex<Option<LastRootHiq>> = std::sync::Mutex::new(None);

pub(crate) fn note_last_root_hiq(snap: LastRootHiq) {
    if let Ok(mut g) = LAST_ROOT_HIQ.lock() {
        *g = Some(snap);
    }
}

pub(crate) fn last_root_hiq() -> Option<LastRootHiq> {
    LAST_ROOT_HIQ.lock().ok().and_then(|g| *g)
}

pub(crate) fn root_trajectory_json(history: &[ChatMsg]) -> Value {
    let root = root_trajectory(history);
    let ratio = offload_ratio(
        root.handle_receipts,
        root.aged_receipts,
        root.bulk_tool_results,
    );
    let prio = hiq_priority(ratio, active_lane());
    note_last_root_hiq(LastRootHiq {
        offload_ratio: ratio,
        hiq_priority: prio,
        handle_receipts: root.handle_receipts,
        aged_receipts: root.aged_receipts,
        bulk_tool_results: root.bulk_tool_results,
        strategy_token_n: root.tokens.len(),
    });
    serde_json::json!({
        "fingerprint": root.fingerprint(),
        "tokens": root.tokens,
        "token_n": root.tokens.len(),
        "root_chars": root.root_chars,
        "tool_hops": root.tool_hops,
        "handle_receipts": root.handle_receipts,
        "aged_receipts": root.aged_receipts,
        "bulk_tool_results": root.bulk_tool_results,
        "offload_ratio": ratio,
        "hiq_priority": prio,
    })
}

pub(crate) fn offload_ratio(handle: usize, aged: usize, bulk: usize) -> f64 {
    let offloaded = handle.saturating_add(aged) as f64;
    let total = offloaded + bulk as f64;
    if total <= 0.0 { 0.0 } else { offloaded / total }
}

pub(crate) fn hiq_priority(offload_ratio: f64, lane: Lane) -> f64 {
    let ratio = offload_ratio.clamp(0.0, 1.0);
    let mut w = 0.5 + 1.25 * ratio;
    if lane == Lane::Treebeard {
        w *= 1.25;
    }
    let wmax = std::env::var("FORGE_HIQ_WEIGHT_MAX")
        .ok()
        .and_then(|s| s.parse::<f64>().ok())
        .unwrap_or(3.0)
        .clamp(1.0, 5.0);
    w.clamp(0.5, wmax)
}

#[cfg(test)]
#[path = "../../../../tests/cockpit/harness/trajectory__hiq_tests.rs"]
mod hiq_tests;

pub(crate) fn harness_treatment_json() -> Value {
    let stats = session_stats();
    let mut map = serde_json::Map::new();
    map.insert(
        "lane".into(),
        Value::String(active_lane().as_str().to_string()),
    );
    map.insert("handle_store".into(), handle_store_enabled().into());
    map.insert("handle_read_tool".into(), handle_read_tool_enabled().into());
    map.insert(
        "handle_code_mode_min_bytes".into(),
        code_mode_offload_min_bytes().into(),
    );
    map.insert(
        "handle_subcall_min_bytes".into(),
        subcall_offload_min_bytes().into(),
    );
    map.insert(
        "handle_eager_tool_min_bytes".into(),
        eager_tool_offload_min_bytes().into(),
    );
    map.insert("treebeard_max_depth".into(), treebeard_max_depth().into());
    map.insert("subcall_depth".into(), subcall_depth().into());
    map.insert(
        "tool_aging".into(),
        env_flag("ANGEL_TOOL_AGING", true).into(),
    );
    if let Some(requested) = std::env::var("ANGEL_RL_REWARD")
        .ok()
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
    {
        let live = crate::drive::reinforce::live_reward_label();
        map.insert("rl_reward".into(), Value::String(live.into()));
        if crate::drive::reinforce::reward_label_from_env() != live {
            map.insert("rl_reward_requested".into(), Value::String(requested));
        }
    }
    map.insert(
        "handle_stats".into(),
        serde_json::json!({
            "entries": stats.entries,
            "total_bytes": stats.total_bytes,
            "puts": stats.puts,
            "discloses": stats.discloses,
            "evictions": stats.evictions,
        }),
    );
    Value::Object(map)
}

pub(crate) fn should_attach_root_trajectory(reward: Option<f32>) -> bool {
    reward.is_some() || env_flag("ANGEL_ROOT_TRAJECTORY", false)
}

pub(crate) fn trajectory_record(
    club_label: &str,
    history: &[ChatMsg],
    answer: &str,
    hops: usize,
    interrupted: bool,
    reward: Option<f32>,
    ts_ms: u64,
) -> Value {
    let mut rec = serde_json::json!({
        "schema": "angel-trajectory/v2",
        "identity": super::run_identity::current_for_turn(),
        "ts_ms": ts_ms,
        "club": club_label,
        "hops": hops,
        "interrupted": interrupted,
        "answer": answer,
        "messages": trajectory_messages(history),
    });
    if let Some(r) = reward {
        rec["reward"] = serde_json::json!(r);
    }
    let root_json = root_trajectory_json(history);
    if should_attach_root_trajectory(reward) {
        rec["root_trajectory"] = root_json;
        rec["harness_treatment"] = harness_treatment_json();
    }
    trace_schema::defaults(&mut rec);
    rec["cost"] = cost::cost_for(rec["identity"]["model"]["id"].as_str(), None);
    if let Some(budget) = super::formation_budget::snapshot() {
        rec["formation_budget"] = budget;
    }
    rec
}

pub(crate) fn trajectory_dir() -> PathBuf {
    if let Some(dir) = std::env::var_os("ANGEL_TRAJECTORY_DIR") {
        return PathBuf::from(dir);
    }
    if cfg!(test) {
        return std::env::temp_dir()
            .join(format!("angel-trajectories-test-{}", std::process::id()));
    }
    std::env::var_os("HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("/tmp"))
        .join(".angelX/trajectories")
}

pub(crate) fn now_ms() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

static TRAJECTORY_WRITE_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

pub(crate) fn append_trajectory(path: &Path, record: &Value) -> std::io::Result<()> {
    let _guard = TRAJECTORY_WRITE_LOCK
        .lock()
        .unwrap_or_else(|e| e.into_inner());
    rotation::append(
        path,
        record,
        crate::platform::store_caps::value("trajectories", "max_bytes"),
        std::time::Duration::from_secs(
            crate::platform::store_caps::value("trajectories", "max_age_days") * 86400,
        ),
    )
}

pub(crate) fn write_trajectory(record: &Value) {
    if std::env::var_os("ANGEL_TRAJECTORY_LOG").is_none() {
        return;
    }
    let dir = trajectory_dir();
    if crate::platform::workspace_store::private_io::PrivateDirectory::open(&dir).is_err() {
        return;
    }
    ensure_private_store(&dir);
    let path = dir.join(format!("session-{}.jsonl", std::process::id()));
    let _ = append_trajectory(&path, record);
}

pub(crate) fn ensure_private_store(dir: &Path) {
    #[cfg(unix)]
    {
        let mut targets = vec![dir.to_path_buf()];
        if let Some(parent) = dir.parent()
            && parent.file_name().is_some_and(|n| n == ".angelX")
        {
            targets.push(parent.to_path_buf());
        }
        for target in targets {
            if let Ok(directory) =
                crate::platform::workspace_store::private_io::PrivateDirectory::open_existing(
                    &target,
                )
            {
                let _ = directory.secure_owner_only();
            }
        }
    }
    #[cfg(not(unix))]
    {
        let _ = dir;
    }
}

pub(crate) fn log_trajectory(
    club: &dyn Club,
    history: &[ChatMsg],
    answer: &str,
    hops: usize,
    interrupted: bool,
    reward: Option<f32>,
) {
    if std::env::var_os("ANGEL_TRAJECTORY_LOG").is_none() {
        return;
    }
    let Some(repo) = crate::knowledge::experience::current_turn_repo_value() else {
        return;
    };
    let mut record = trajectory_record(
        club.label(),
        history,
        answer,
        hops,
        interrupted,
        if eval_owns_label() { None } else { reward },
        now_ms(),
    );
    record["repo"] = repo;
    attach_turn_ledger(&mut record, Some(club));
    if eval_owns_label() {
        record["data_class"] = serde_json::json!("coding_eval_observation");
        record["evaluator_decision_sha256"] = serde_json::Value::Null;
        record["training_capture"] = serde_json::json!({"status":"pending_evaluator"});
    }
    if let Some((hop, ms)) = take_first_action() {
        record["first_action_hop"] = hop.into();
        record["first_action_ms"] = ms.into();
    }
    write_trajectory(&record);
}

thread_local! {
    static EVAL_OWNS_LABEL: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };

    static FIRST_ACTION: std::cell::Cell<Option<(usize, u64)>> = const { std::cell::Cell::new(None) };
}

pub(crate) fn note_first_action(hop: usize, ms_since_turn_start: u64) {
    FIRST_ACTION.with(|cell| {
        if cell.get().is_none() {
            cell.set(Some((hop, ms_since_turn_start)));
        }
    });
}

pub(crate) fn reset_first_action() {
    FIRST_ACTION.with(|cell| cell.set(None));
}

#[derive(Default)]
struct TurnLedger {
    tools_output: super::turn::background::ToolsOutput,
    store_rotations: Vec<Value>,
    tools: Vec<Value>,
    verifier: Vec<Value>,
    artifacts: Vec<Value>,
    turn_id: Option<String>,
    session: Option<String>,
    stop_reason: Option<super::turn::TurnStopReason>,
    timing_origin: Option<std::time::Instant>,
    active_model_call: Option<usize>,
    next_model_call: usize,
    effort_leg: Option<(String, Option<String>)>,
    provider_samples: Vec<Value>,
    parking_events: Vec<Value>,
    timing: Option<Value>,
    task_timing: Option<super::turn::TaskTimingTelemetry>,
    usage_before: Option<crate::agent::club::AccountingView>,
    signatures: std::collections::HashMap<String, usize>,
    read_paths: std::collections::HashSet<String>,
    verifier_runs: std::collections::HashSet<String>,
    remote_receipts: std::collections::HashSet<String>,
    failing_tests: std::collections::HashSet<String>,
    completed_hops: usize,
    last_progress: Option<Value>,
    hop_credit: Option<&'static str>,
    stream_cuts: std::collections::BTreeMap<usize, usize>,
    research_turn: bool,
    research_sources: std::collections::HashSet<String>,
    research_answer_delivered: bool,
    first_verified_at_ms: Option<u64>,
    unproductive_streak: usize,
    unproductive_streak_max: usize,
    hop_stamps: std::collections::VecDeque<char>,
    hop_stamp_counts: std::collections::BTreeMap<char, usize>,
    routes_raised: std::collections::BTreeMap<String, usize>,
    ledger_reads: usize,
    hop_progress: bool,
    hop_pending: bool,
    escalations: Vec<Value>,
    streak_escalated: bool,
    last_streak_evaluation_hop: Option<usize>,
    last_verifier: Option<String>,
}

thread_local! {
    static TURN_LEDGER: std::cell::RefCell<TurnLedger> =
        std::cell::RefCell::new(TurnLedger::default());
}

pub(crate) fn reset_turn_ledger(club: &dyn Club) {
    super::run_identity::reset_turn_identity();
    LAST_TASK_TIMING.with(|cell| *cell.borrow_mut() = None);
    TURN_LEDGER.with(|cell| {
        let mut ledger = cell.borrow_mut();
        *ledger = TurnLedger::default();
        ledger.usage_before = Some(club.usage_accounting());
        static NEXT_TURN: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
        ledger.turn_id = Some(format!(
            "{}-{}-{}",
            std::process::id(),
            now_ms(),
            NEXT_TURN.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
        ));
    });
}

pub(crate) fn set_research_turn(selected: bool) {
    TURN_LEDGER.with(|cell| cell.borrow_mut().research_turn = selected);
}

pub(crate) fn note_research_answer(answer: &str) {
    TURN_LEDGER.with(|cell| {
        let mut ledger = cell.borrow_mut();
        if ledger.research_turn && !answer.trim().is_empty() {
            ledger.research_answer_delivered = true;
            ledger.hop_progress = true;
        }
    });
}

pub(crate) fn note_turn_session(session: &str) {
    TURN_LEDGER.with(|cell| {
        cell.borrow_mut().session = Some(if session.is_empty() {
            format!("session-{}", std::process::id())
        } else {
            session.to_owned()
        });
    });
}

pub(crate) fn note_stop_reason(reason: super::turn::TurnStopReason) {
    TURN_LEDGER.with(|cell| cell.borrow_mut().stop_reason = Some(reason));
}

#[derive(Default)]
struct TaskLifecycle {
    origin: Option<std::time::Instant>,
    phases: serde_json::Map<String, Value>,
    first_request: Option<u128>,
    answer: Option<u128>,
}
thread_local! {
    static TASK_LIFECYCLE: std::cell::RefCell<TaskLifecycle> = Default::default();
    static LAST_TASK_TIMING: std::cell::RefCell<Option<super::turn::TaskTimingTelemetry>> = Default::default();
}

pub(crate) fn retain_task_timing(timing: &super::turn::TaskTimingTelemetry) {
    LAST_TASK_TIMING.with(|cell| *cell.borrow_mut() = Some(timing.clone()));
}

pub(crate) fn task_timing_snapshot() -> Option<super::turn::TaskTimingTelemetry> {
    LAST_TASK_TIMING.with(|cell| cell.borrow().clone())
}

#[cfg(test)]
pub(crate) fn clear_task_lifecycle() {
    TASK_LIFECYCLE.with(|cell| *cell.borrow_mut() = TaskLifecycle::default());
}

pub(crate) fn begin_task_lifecycle(origin: std::time::Instant) {
    crate::agent::harness::clear_graph_turn_episodes();
    TASK_LIFECYCLE.with(|cell| {
        *cell.borrow_mut() = TaskLifecycle {
            origin: Some(origin),
            ..Default::default()
        }
    });
}

pub(crate) fn note_task_startup_phase(name: &str, ms: u128) {
    TASK_LIFECYCLE.with(|cell| {
        cell.borrow_mut()
            .phases
            .insert(name.into(), serde_json::json!(ms));
    });
}

pub(crate) fn note_task_answer() {
    TASK_LIFECYCLE.with(|cell| {
        let mut state = cell.borrow_mut();
        if state.answer.is_none() {
            state.answer = state.origin.map(|t| t.elapsed().as_millis());
        }
    });
}

pub(crate) fn task_lifecycle_snapshot() -> Option<(u128, u128, u128, Value)> {
    TASK_LIFECYCLE.with(|cell| {
        let state = cell.borrow();
        let wall = state.origin?.elapsed().as_millis();
        let startup = state.first_request.unwrap_or(wall);
        let shutdown = state.answer.map_or(0, |answer| wall.saturating_sub(answer));
        let mut phases = state.phases.clone();
        let measured: u128 = phases
            .values()
            .filter_map(Value::as_u64)
            .map(u128::from)
            .sum();
        phases.insert(
            "pre_request_ms".into(),
            serde_json::json!(startup.saturating_sub(measured)),
        );
        Some((wall, startup, shutdown, Value::Object(phases)))
    })
}

pub(crate) fn note_last_tool_attribution(ms: u128) {
    TURN_LEDGER.with(|cell| {
        if let Some(tool) = cell.borrow_mut().tools.last_mut() {
            tool["execution_ms"] = tool["ms"].clone();
            tool["ms"] = serde_json::json!(ms);
        }
    });
}

pub(crate) fn note_timing_origin(origin: std::time::Instant) {
    TURN_LEDGER.with(|cell| cell.borrow_mut().timing_origin = Some(origin));
}

pub(crate) fn begin_model_request() {
    super::run_identity::begin_request();
    TASK_LIFECYCLE.with(|cell| {
        let mut state = cell.borrow_mut();
        if state.first_request.is_none() {
            state.first_request = state.origin.map(|t| t.elapsed().as_millis());
        }
    });
    TURN_LEDGER.with(|cell| {
        let mut ledger = cell.borrow_mut();
        ledger.active_model_call = Some(ledger.next_model_call);
        ledger.next_model_call += 1;
    });
}

pub(crate) fn note_effort_leg(leg: Option<(String, Option<String>)>) {
    TURN_LEDGER.with(|cell| cell.borrow_mut().effort_leg = leg);
}

pub(crate) fn end_model_request() {
    TURN_LEDGER.with(|cell| cell.borrow_mut().active_model_call = None);
}

pub(crate) fn model_request_active() -> bool {
    TURN_LEDGER.with(|cell| cell.borrow().active_model_call.is_some())
}

pub(crate) fn begin_provider_attempt(source_id: u64) -> Option<usize> {
    let identity = super::run_identity::request_identity().map(|identity| {
        serde_json::json!({"model":identity.model,"effort":identity.effort,
            "output_tokens":identity.budgets["output_tokens"]})
    });
    TURN_LEDGER.with(|cell| {
        let mut ledger = cell.borrow_mut();
        let parent = ledger.active_model_call?;
        let start = ledger.timing_origin?.elapsed().as_millis() as u64;
        let id = ledger.provider_samples.len();
        let retry_of = ledger
            .provider_samples
            .iter()
            .rposition(|sample| sample["model_call"] == parent && sample["source_id"] == source_id);
        let mut sample = serde_json::json!({"id":id,"start":start,
            "end":null,"retry_of":retry_of,"model_call":parent,"source_id":source_id,
            "identity":identity,"usage":null,"usage_accounting":null,
            "request_bytes":null,"response_bytes":null});
        if let Some((leg, error)) = &ledger.effort_leg {
            sample["effort_leg"] = serde_json::json!(leg);
            if let Some(error) = error {
                sample["effort_leg_error"] = serde_json::json!(error);
            }
        }
        ledger.provider_samples.push(sample);
        Some(id)
    })
}

pub(crate) fn note_provider_bytes(id: Option<usize>, request: Option<u64>, response: Option<u64>) {
    TURN_LEDGER.with(|cell| {
        let mut ledger = cell.borrow_mut();
        if let Some(sample) = id.and_then(|id| ledger.provider_samples.get_mut(id)) {
            sample["request_bytes"] = serde_json::json!(request);
            sample["response_bytes"] = serde_json::json!(response);
        }
    });
}

pub(crate) fn end_provider_attempt(
    id: Option<usize>,
    observation: Option<crate::agent::club::UsageObservation>,
) {
    TURN_LEDGER.with(|cell| {
        let mut ledger = cell.borrow_mut();
        let end = ledger
            .timing_origin
            .map(|origin| origin.elapsed().as_millis() as u64);
        if let Some(sample) = id.and_then(|id| ledger.provider_samples.get_mut(id)) {
            sample["end"] = serde_json::json!(end);
            sample["usage"] = serde_json::json!(observation.map(|o| o.raw));
            let mut snapshot = crate::agent::club::AccountingSnapshot::default();
            snapshot.record(observation);
            sample["usage_accounting"] = complete_ledger_usage(
                &crate::agent::club::AccountingReport::from_snapshot(snapshot, false),
            );
            let [input, output, _, cached, _] = observation.map(|o| o.raw).unwrap_or([None; 5]);
            let included = observation
                .is_some_and(|o| o.contract.cache == crate::agent::club::CacheConvention::Included);
            let paid = if included {
                input.zip(cached).and_then(|(i, c)| i.checked_sub(c))
            } else {
                None
            };
            let generation = observation.and_then(|o| o.generation_output());
            let total = if included {
                input.zip(generation).and_then(|(i, o)| i.checked_add(o))
            } else {
                None
            };
            let counters = serde_json::json!({"raw_input":input,"output":output,
                "cached_input":cached,"paid_input":paid,"total_tokens":total,
                "generation_output":generation,
                "response_bytes":sample["response_bytes"]});
            let status = if observation.is_none() {
                "unreported"
            } else if counters.as_object().unwrap().values().all(|v| !v.is_null()) {
                "reported"
            } else {
                "partial"
            };
            sample["accounting_status"] = serde_json::json!(status);
            sample["counters"] = counters;
        }
    });
}

pub(crate) fn provider_call_samples() -> Vec<Value> {
    TURN_LEDGER.with(|cell| cell.borrow().provider_samples.clone())
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn note_tool_outcome(
    hop: usize,
    name: &str,
    args: &Value,
    result: &str,
    execution: &str,
    errored: bool,
    error_class: Option<&str>,
    verify: Option<&str>,
    elapsed_ms: Option<u128>,
    result_bytes: usize,
) {
    let mut entry = serde_json::json!({
        "hop": hop,
        "tool": name,
        "exec": execution,
        "status": execution,
        "err": errored,
        "bytes": result_bytes,
        "ms": null, "class": null, "verify": null,
        "args_digest": canonical_signature(name, args),
    });
    if let Some(receipt) = super::exec::sandbox_receipt() {
        entry["sandbox_profile"] = receipt["sandbox_profile"].clone();
        entry["sandbox"] = receipt;
    }
    if name == "proc_run"
        && !errored
        && let Some(id) = result
            .strip_prefix("started [")
            .and_then(|s| s.split_once(']'))
            .and_then(|(id, _)| id.parse::<u64>().ok())
    {
        entry["proc_id"] = serde_json::json!(id);
    }
    if name == "shell"
        && errored
        && let Some(kill) = crate::agent::sandbox::process_owner::KillReceipt::from_error(result)
    {
        kill.apply(&mut entry);
    }
    if let Some(ms) = elapsed_ms {
        entry["ms"] = serde_json::json!(ms as u64);
    }
    if let Some(class) = error_class {
        entry["class"] = serde_json::json!(class);
    }
    if let Some(v) = verify {
        entry["verify"] = serde_json::json!(v);
    }
    let failure = (errored
        || matches!(execution, "denied" | "failed" | "cancelled" | "panicked")
        || matches!(verify, Some("failed" | "inconclusive")))
    .then_some(result);
    if result.starts_with("tool error:") {
        entry["error"] = serde_json::json!(result);
    }
    let class = super::tool_errors::classify_tool_error(name, args, failure);
    entry["error_class"] = serde_json::json!(class);
    entry["avoidable"] = serde_json::json!(
        (result.starts_with("tool error:")
            && matches!(
                class,
                super::tool_errors::ToolErrorClass::Environment
                    | super::tool_errors::ToolErrorClass::Policy
            ))
            || super::tool_errors::avoidable(class, name, args, failure)
    );
    TURN_LEDGER.with(|cell| {
        let mut ledger = cell.borrow_mut();
        let attempt_id = format!("{}:{}", ledger.turn_id.as_deref().unwrap_or("unbound"), ledger.tools.len());
        entry["attempt_id"] = serde_json::json!(attempt_id);
        if verify.is_some() {
            ledger.verifier.push(serde_json::json!({"attempt_id":attempt_id,
                "plan": name, "command":args.get("command").and_then(Value::as_str).map(crate::knowledge::experience::scrub_secrets), "exit":null, "ms":elapsed_ms.map(|v| v as u64),
                "coverage_gap":"numeric_exit_not_captured; command_is_scrubbed_when_available"}));
        }
        if name == "present" {
            let path = args.get("url").and_then(Value::as_str);
            ledger.artifacts.push(serde_json::json!({"attempt_id":attempt_id,
                "kind":args.get("kind").and_then(Value::as_str), "path_digest":path.map(|p| crate::knowledge::cut::sha256_hex(p.as_bytes())),
                "shown":null}));
        }
        if verify.is_some() {
            ledger.research_turn = false;
        }
        if ledger.research_turn && execution == "ok" && class == super::tool_errors::ToolErrorClass::None {
            for source in super::turn::research::sources(name, args, result) {
                let added = ledger.research_sources.insert(source);
                ledger.hop_progress |= added;
            }
        }
        if ledger.research_turn && super::turn::research::off_surface(name, args) {
            entry["research_note"] = serde_json::json!("Research/OffSurface");
            entry["avoidable"] = serde_json::json!(true);
        }
        if verify.is_some() {
            ledger.last_verifier = Some(if error_class == Some("timeout")
                || result.to_ascii_lowercase().contains("timed out") {
                entry["class"] = serde_json::json!("timeout");
                "timeout".to_string()
            } else {
                verify.unwrap_or("inconclusive").to_string()
            });
        }
        let signature = canonical_signature(name, args);
        let new_signature = !ledger.signatures.contains_key(&signature);
        entry["duplicate_of"] = serde_json::json!(ledger.signatures.get(&signature));
        let index = ledger.tools.len();
        ledger.signatures.entry(signature).or_insert(index);
        if execution == "ok"
            && name == "read_file"
            && let Some(path) = args.get("path").and_then(Value::as_str)
        {
            ledger.read_paths.insert(normalized_read_path(path));
            if super::book::ledger::is_ledger_path(path) {
                ledger.ledger_reads += 1;
            }
        }
        let remote_receipt = execution == "ok"
            && !errored
            && matches!(name, "shell" | "proc_status")
            && is_remote_verification_receipt(result)
            && ledger.remote_receipts.insert(crate::knowledge::cut::sha256_hex(result.as_bytes()));
        if (verify.is_some() || super::turn::is_progress_verifier_call(name, args) || remote_receipt)
            && execution == "ok" && !errored {
            let first_run = ledger.verifier_runs.insert(canonical_signature(name, args));
            let mut new_failure = false;
            if verify == Some("failed") {
                for signature in failing_test_signatures(result) {
                    new_failure |= ledger.failing_tests.insert(signature);
                }
            }
            if first_run || new_failure || remote_receipt {
                ledger.hop_progress = true;
                ledger.hop_credit = Some(if new_failure {
                    "new_failing_test"
                } else if remote_receipt {
                    "remote_verification_receipt"
                } else {
                    "verifier_run"
                });
            }
        }
        if name == "graph" && execution == "ok" && !errored && new_signature {
            ledger.hop_progress = true;
            ledger.hop_credit = Some("graph_receipt");
        }
        ledger.tools.push(entry);
    });
}

fn is_remote_verification_receipt(result: &str) -> bool {
    result.contains("pass=True")
        || result.contains("passed_correctness")
        || result.contains("GOLDEN pass")
        || result.contains("FOLD_AB_DONE")
        || result.contains("STACK_AB_DONE")
        || result.contains("test result: ok")
        || result.contains("officialScore")
}

fn normalized_read_path(path: &str) -> String {
    let mut normalized = std::path::PathBuf::new();
    for component in std::path::Path::new(path).components() {
        match component {
            std::path::Component::CurDir => {}
            std::path::Component::ParentDir
                if normalized.file_name().is_some_and(|name| name != "..") =>
            {
                normalized.pop();
            }
            _ => normalized.push(component.as_os_str()),
        }
    }
    normalized.to_string_lossy().into_owned()
}

fn canonical_signature(name: &str, args: &Value) -> String {
    fn canonical(value: &Value) -> Value {
        match value {
            Value::Object(map) => {
                let sorted: std::collections::BTreeMap<_, _> = map
                    .iter()
                    .map(|(key, value)| (key.clone(), canonical(value)))
                    .collect();
                serde_json::to_value(sorted).expect("JSON object")
            }
            Value::Array(values) => Value::Array(values.iter().map(canonical).collect()),
            _ => value.clone(),
        }
    }
    crate::knowledge::cut::sha256_hex(format!("{name}\0{}", canonical(args)).as_bytes())
}

pub(crate) fn note_verified(elapsed_ms: u64) {
    TURN_LEDGER.with(|cell| {
        let mut ledger = cell.borrow_mut();
        if ledger.first_verified_at_ms.is_none() {
            ledger.first_verified_at_ms = Some(elapsed_ms);
            ledger.hop_progress = true;
        }
    });
}

fn failing_test_signatures(result: &str) -> Vec<String> {
    result
        .lines()
        .filter_map(|line| {
            let line = line.trim();
            let identity = line
                .strip_prefix("test ")
                .and_then(|s| s.strip_suffix(" ... FAILED"))
                .or_else(|| {
                    line.strip_prefix("FAILED ")
                        .map(|s| s.split(" - ").next().unwrap_or(s))
                })
                .or_else(|| {
                    line.strip_prefix("--- FAIL: ")
                        .map(|s| s.split_whitespace().next().unwrap_or(s))
                })
                .or_else(|| {
                    line.strip_prefix("thread '")
                        .and_then(|s| s.split_once("' panicked at").map(|(name, _)| name))
                });
            identity
                .filter(|s| !s.is_empty())
                .map(|s| crate::knowledge::cut::sha256_hex(s.as_bytes()))
        })
        .collect()
}

pub(crate) fn note_stream_cut(hop: usize) {
    TURN_LEDGER.with(|cell| *cell.borrow_mut().stream_cuts.entry(hop).or_default() += 1);
}

pub(crate) fn mutation_byte_snapshot(
    root: &Path,
    calls: &[ToolCall],
) -> Vec<(std::path::PathBuf, Option<String>)> {
    let mut paths = std::collections::BTreeSet::new();
    for call in calls {
        crate::knowledge::cut::for_each_mutation_target_path(&call.name, &call.args, |path| {
            paths.insert(std::path::PathBuf::from(path));
            false
        });
    }
    paths
        .into_iter()
        .map(|path| {
            let hash = confined_read(root, &path)
                .ok()
                .map(|bytes| crate::knowledge::cut::sha256_hex(&bytes));
            (path, hash)
        })
        .collect()
}

pub(crate) struct ProgressHop;

pub(crate) fn begin_progress_hop() -> ProgressHop {
    TURN_LEDGER.with(|cell| cell.borrow_mut().hop_pending = true);
    ProgressHop
}

impl Drop for ProgressHop {
    fn drop(&mut self) {
        if TURN_LEDGER.with(|cell| cell.borrow().hop_pending) {
            note_progress_hop(false);
        }
    }
}

pub(crate) fn note_progress_hop(mutated: bool) {
    TURN_LEDGER.with(|cell| {
        let mut ledger = cell.borrow_mut();
        ledger.completed_hops += 1;
        let live_background_jobs = crate::agent::tools::proc::live_job_count();
        if mutated {
            ledger.research_turn = false;
            ledger.hop_progress = true;
            ledger.hop_credit = Some("workspace_bytes_changed");
            ledger.verifier_runs.clear();
        }
        if ledger.hop_progress {
            ledger.last_progress = Some(serde_json::json!({
                "hop": ledger.completed_hops,
                "kind": ledger.hop_credit.unwrap_or("verified_or_research_evidence"),
            }));
        }
        ledger.hop_credit = None;
        ledger.unproductive_streak = if ledger.hop_progress {
            0
        } else if live_background_jobs > 0 {
            ledger.unproductive_streak
        } else {
            ledger.unproductive_streak + 1
        };
        ledger.unproductive_streak_max = ledger
            .unproductive_streak_max
            .max(ledger.unproductive_streak);
        if ledger.unproductive_streak == 0 {
            ledger.streak_escalated = false;
        }
        ledger.hop_progress = false;
        ledger.hop_pending = false;
    });
}

pub(crate) fn note_hop_stamps(raised: &[super::book::Route]) {
    use super::book::{PASS_GLYPH, WAIT_GLYPH};
    let glyph = match raised.first() {
        Some(route) => route.primary,
        None if crate::agent::tools::proc::live_job_count() > 0 => WAIT_GLYPH,
        None => PASS_GLYPH,
    };
    TURN_LEDGER.with(|cell| {
        let mut ledger = cell.borrow_mut();
        for route in raised {
            *ledger.routes_raised.entry(route.name()).or_default() += 1;
        }
        if ledger.hop_stamps.len() == 48
            && let Some(old) = ledger.hop_stamps.pop_front()
            && let Some(slot) = ledger.hop_stamp_counts.get_mut(&old)
        {
            *slot = slot.saturating_sub(1);
        }
        ledger.hop_stamps.push_back(glyph);
        *ledger.hop_stamp_counts.entry(glyph).or_default() += 1;
    });
}

pub(crate) fn unproductive_escalation(
    hop: usize,
    escalate: usize,
    redirect: usize,
) -> (Option<String>, Option<String>) {
    TURN_LEDGER.with(|cell| {
        let mut ledger = cell.borrow_mut();
        if ledger.last_streak_evaluation_hop == Some(hop) {
            return (None, None);
        }
        ledger.last_streak_evaluation_hop = Some(hop);
        let streak = ledger.unproductive_streak;
        let last = ledger
            .last_verifier
            .as_deref()
            .unwrap_or("not_run")
            .to_string();
        let already_escalated = ledger.streak_escalated;
        let notice = if escalate > 0
            && streak >= escalate
            && (!already_escalated || streak.is_multiple_of(escalate))
        {
            let elapsed_ms = ledger.timing_origin.map(|t| t.elapsed().as_millis() as u64);
            ledger.escalations.push(serde_json::json!({
                "kind": "unproductive_streak", "hop": hop, "streak": streak,
                "last_verifier": last, "elapsed_ms": elapsed_ms,
            }));
            ledger.streak_escalated = true;
            let streak_route = crate::agent::harness::book::l_loops::STREAK;
            if ledger.research_turn {
                Some(format!("{}⠁ streak={streak}", streak_route.cells()))
            } else {
                Some(format!(
                    "{}⠃ streak={streak} last={last}",
                    streak_route.cells()
                ))
            }
        } else {
            None
        };
        let redirected = redirect > 0
            && !ledger.research_turn
            && streak >= redirect
            && (already_escalated || ledger.streak_escalated);
        let redirect_note = redirected.then(|| {
            let streak_route = crate::agent::harness::book::l_loops::STREAK;
            ledger.escalations.push(serde_json::json!({
                "kind": "unproductive_redirect", "hop": hop, "streak": streak,
                "last_verifier": last,
            }));
            format!("{}⠉ streak={streak} last={last}", streak_route.cells())
        });
        if redirected {
            ledger.unproductive_streak = 0;
            ledger.streak_escalated = false;
            ledger.last_streak_evaluation_hop = Some(hop);
        }
        (notice, redirect_note)
    })
}

pub(crate) fn note_escalation(hop: usize, kind: &str) {
    TURN_LEDGER.with(|cell| {
        cell.borrow_mut()
            .escalations
            .push(serde_json::json!({"hop": hop, "kind": kind}))
    });
}

pub(crate) fn note_provider_unavailable(hop: usize, class: &str, leg: &str, digest: Option<&str>) {
    TURN_LEDGER.with(|cell| {
        cell.borrow_mut().escalations.push(serde_json::json!({
            "hop": hop, "kind": "provider_unavailable", "class": class, "leg": leg, "digest": digest,
        }))
    });
}

pub(crate) fn note_route_switch(
    hop: usize,
    from: &crate::agent::club::RouteIdentity,
    to: &crate::agent::club::RouteIdentity,
) {
    TURN_LEDGER.with(|cell| {
        cell.borrow_mut().escalations.push(serde_json::json!({
            "hop": hop, "kind": "route_switch", "from": from, "to": to,
        }));
    });
}

pub(crate) fn last_route_switch() -> Option<crate::agent::club::RouteIdentity> {
    TURN_LEDGER.with(|cell| {
        cell.borrow()
            .escalations
            .iter()
            .rev()
            .find(|event| event["kind"] == "route_switch")
            .and_then(|event| serde_json::from_value(event["to"].clone()).ok())
    })
}

pub(crate) fn turn_identity() -> Option<super::run_identity::RunIdentity> {
    super::run_identity::current_for_turn()
}

pub(crate) fn progress_ledger_snapshot() -> Value {
    TURN_LEDGER.with(|cell| {
        let ledger = cell.borrow();
        serde_json::json!({
            "research_turn": ledger.research_turn,
            "research_sources": ledger.research_sources.len(),
            "research_answer_delivered": ledger.research_answer_delivered,
            "first_verified_at_ms": ledger.first_verified_at_ms,
            "last_credited_progress": ledger.last_progress,
            "hop_stream_cuts": ledger.stream_cuts.iter().map(|(hop, count)| serde_json::json!({"hop":hop,"stream_cut":count})).collect::<Vec<_>>(),
            "hop_stamps": ledger.hop_stamps.iter().collect::<String>(),
            "hop_stamp_counts": ledger.hop_stamp_counts,
            "routes_raised": ledger.routes_raised,
            "ledger_reads": ledger.ledger_reads,
            "unproductive_streak_max": ledger.unproductive_streak_max.max(
                if ledger.hop_pending && !ledger.hop_progress { ledger.unproductive_streak + 1 } else { 0 }
            ),
            "escalations": ledger.escalations,
        })
    })
}

pub(crate) fn tool_ledger_snapshot() -> Vec<Value> {
    TURN_LEDGER.with(|cell| cell.borrow().tools.clone())
}

pub(crate) fn note_proc_kills(kills: &[(u64, crate::agent::sandbox::process_owner::KillReceipt)]) {
    TURN_LEDGER.with(|cell| {
        let mut ledger = cell.borrow_mut();
        for (id, kill) in kills {
            if let Some(entry) = ledger
                .tools
                .iter_mut()
                .find(|entry| entry["tool"] == "proc_run" && entry["proc_id"].as_u64() == Some(*id))
            {
                kill.apply(entry);
            }
        }
    });
}

#[cfg_attr(not(test), allow(dead_code))]
pub(crate) fn note_timing<T: serde::Serialize>(timing: &T) {
    if let Ok(v) = serde_json::to_value(timing) {
        TURN_LEDGER.with(|cell| {
            let mut ledger = cell.borrow_mut();
            ledger.timing = Some(v);
            ledger.task_timing = None;
        });
    }
}

pub(crate) fn note_task_timing(timing: &super::turn::TaskTimingTelemetry) {
    TURN_LEDGER.with(|cell| {
        let mut ledger = cell.borrow_mut();
        ledger.task_timing = Some(timing.clone());
        ledger.timing = None;
    });
}

pub(crate) fn ledger_status_text(arg: &str) -> String {
    format!(
        "{}\n{}",
        super::run_identity::summary(),
        ledger_rows_text(arg)
    )
}

fn ledger_rows_text(arg: &str) -> String {
    let want: usize = arg
        .split_whitespace()
        .next()
        .and_then(|w| w.parse().ok())
        .unwrap_or(12)
        .max(1);
    let dir = trajectory_dir();
    if std::env::var_os("ANGEL_TRAJECTORY_LOG").is_none() {
        return format!(
            "turn ledger — off: set ANGEL_TRAJECTORY_LOG=1 to record turns (dir {})",
            dir.display()
        );
    }
    let mut files: Vec<(std::time::SystemTime, PathBuf)> = std::fs::read_dir(&dir)
        .map(|rd| {
            rd.flatten()
                .filter(|e| e.path().extension().is_some_and(|x| x == "jsonl"))
                .filter_map(|e| Some((e.metadata().ok()?.modified().ok()?, e.path())))
                .collect()
        })
        .unwrap_or_default();
    files.sort_by_key(|entry| std::cmp::Reverse(entry.0));
    const TAIL_BYTES: u64 = 1 << 20;
    const LEDGERLESS_SESSIONS: usize = 24;
    let mut rows: Vec<Value> = Vec::new();
    let mut scanned_files = 0usize;
    let mut scanned_records = 0usize;
    let mut misses = 0usize;
    for (_, path) in files {
        let Some(text) = read_tail(&path, TAIL_BYTES) else {
            continue;
        };
        scanned_files += 1;
        let mut found = 0usize;
        let mut here: Vec<Value> = text
            .lines()
            .rev()
            .filter_map(|l| serde_json::from_str::<Value>(l).ok())
            .inspect(|_| scanned_records += 1)
            .filter(|r| r.get("timing").is_some() || r.get("tools").is_some())
            .inspect(|_| found += 1)
            .collect();
        rows.append(&mut here);
        if rows.len() >= want {
            break;
        }
        misses = if found == 0 { misses + 1 } else { 0 };
        if misses >= LEDGERLESS_SESSIONS {
            break;
        }
    }
    if rows.is_empty() {
        return format!(
            "turn ledger — no recorded turns with a ledger in the {scanned_files} newest session file(s) \
             ({scanned_records} records) under {} — records land at each turn's end; a binary older than \
             2026-09-07 writes none",
            dir.display()
        );
    }
    rows.truncate(want);
    rows.sort_by_key(|r| r["ts_ms"].as_u64().unwrap_or(0));
    let mut out = format!(
        "turn ledger — last {} turn(s) from {}\n{:<8} {:<14} {:>4} {:>5} {:>4} {:>7} {:>6} {:>6} {:>6} {:<12} {:>6}\n",
        rows.len(),
        dir.display(),
        "time",
        "club",
        "hops",
        "calls",
        "errs",
        "model_s",
        "tool_s",
        "out",
        "reas",
        "verify",
        "reward"
    );
    let (mut sum_hops, mut sum_errs, mut sum_model, mut sum_out) = (0u64, 0u64, 0f64, 0u64);
    for r in &rows {
        let timing = &r["timing"];
        let tools = r["tools"].as_array().cloned().unwrap_or_default();
        let errs = tools.iter().filter(|t| t["err"] == true).count() as u64;
        let verify = tools
            .iter()
            .rev()
            .find_map(|t| t["verify"].as_str().map(str::to_string))
            .unwrap_or_else(|| "-".to_string());
        let ts = r["ts_ms"].as_u64().unwrap_or(0) / 1000;
        let time = {
            let secs = ts % 86_400;
            format!(
                "{:02}:{:02}:{:02}",
                secs / 3600,
                (secs / 60) % 60,
                secs % 60
            )
        };
        let hops = r["hops"].as_u64().unwrap_or(0);
        let model_s = timing["model_ms"].as_u64().unwrap_or(0) as f64 / 1000.0;
        let tool_s = timing["tool_ms"].as_u64().unwrap_or(0) as f64 / 1000.0;
        let out_tok = r["usage"]["output"].as_u64();
        let reas = r["usage"]["reasoning"].as_u64();
        sum_hops += hops;
        sum_errs += errs;
        sum_model += model_s;
        sum_out += out_tok.unwrap_or(0);
        out.push_str(&format!(
            "{:<8} {:<14} {:>4} {:>5} {:>4} {:>7.1} {:>6.1} {:>6} {:>6} {:<12} {:>6}\n",
            time,
            r["club"]
                .as_str()
                .unwrap_or("?")
                .chars()
                .take(14)
                .collect::<String>(),
            hops,
            tools
                .len()
                .max(timing["tool_calls"].as_u64().unwrap_or(0) as usize),
            errs,
            model_s,
            tool_s,
            out_tok.map(|v| v.to_string()).unwrap_or_else(|| "-".into()),
            reas.map(|v| v.to_string()).unwrap_or_else(|| "-".into()),
            verify,
            r["reward"]
                .as_f64()
                .map(|v| format!("{v:.2}"))
                .unwrap_or_else(|| "-".into()),
        ));
    }
    let n = rows.len() as f64;
    out.push_str(&format!(
        "mean: hops {:.1} · tool errors {:.2}/turn · model {:.1} s/turn · out {:.0} tok/turn",
        sum_hops as f64 / n,
        sum_errs as f64 / n,
        sum_model / n,
        sum_out as f64 / n
    ));
    out
}

fn read_tail(path: &Path, max: u64) -> Option<String> {
    use std::io::{Read, Seek, SeekFrom};
    let mut f = std::fs::File::open(path).ok()?;
    let len = f.metadata().ok()?.len();
    let start = len.saturating_sub(max);
    f.seek(SeekFrom::Start(start)).ok()?;
    let mut buf = Vec::with_capacity((len - start) as usize);
    f.read_to_end(&mut buf).ok()?;
    let mut text = String::from_utf8_lossy(&buf).into_owned();
    if start > 0
        && let Some(nl) = text.find('\n')
    {
        text.drain(..=nl);
    }
    Some(text)
}

pub(crate) fn complete_ledger_usage(report: &crate::agent::club::AccountingReport) -> Value {
    let mut value = serde_json::to_value(report).expect("accounting report serializes");
    for key in [
        "input",
        "output",
        "reasoning",
        "cache_read",
        "cache_write",
        "uncached_input",
        "total_prompt",
        "generation_output",
    ] {
        let complete = report.attempts > 0
            && value["reported_attempts"][key].as_u64() == Some(report.attempts)
            && !report.untracked_sources
            && !report.overflowed;
        if !complete {
            value[key] = Value::Null;
        }
    }
    let any_reported = value["reported_attempts"]
        .as_object()
        .unwrap()
        .values()
        .any(|count| count.as_u64().is_some_and(|n| n > 0));
    let complete = [
        "input",
        "output",
        "cache_read",
        "total_prompt",
        "generation_output",
    ]
    .iter()
    .all(|key| !value[*key].is_null())
        && report.inconsistent_attempts == 0;
    value["accounting_status"] = serde_json::json!(if !any_reported {
        "unreported"
    } else if complete {
        "reported"
    } else {
        "partial"
    });
    value
}

fn attach_turn_ledger(record: &mut Value, club: Option<&dyn Club>) {
    record["identity"] = serde_json::to_value(turn_identity()).expect("identity serializes");
    if let Some(fields) = progress_ledger_snapshot().as_object() {
        for (key, value) in fields {
            record[key] = value.clone();
        }
    }
    TURN_LEDGER.with(|cell| {
        let ledger = cell.borrow();
        if let (Some(before), Some(club)) = (ledger.usage_before.as_ref(), club) {
            let report = club.usage_accounting().delta(before);
            if report.attempts > 0 || report.untracked_sources {
                record["usage"] = complete_ledger_usage(&report);
            }
        }
        if let Some(timing) = &ledger.task_timing {
            if let Ok(timing) = serde_json::to_value(timing) {
                record["timing"] = timing;
            }
        } else if let Some(timing) = &ledger.timing {
            record["timing"] = timing.clone();
        }
        if let Some(budget) = super::formation_budget::snapshot() {
            record["formation_budget"] = budget;
        }
        record["tools_output"] = serde_json::json!(ledger.tools_output);
        record["store_rotations"] = serde_json::json!(ledger.store_rotations);
        record["parking_events"] = serde_json::json!(ledger.parking_events);
        record["tools"] = Value::Array(ledger.tools.clone());
        record["verifier"] = serde_json::json!(ledger.verifier);
        record["artifacts"] = serde_json::json!(ledger.artifacts);
        record["lease"] =
            serde_json::json!({"kind":"local", "host":null,"seat":null,"lease_id":null});
        record["turn"]["operator_turn_id"] = serde_json::json!(ledger.turn_id);
        record["turn"]["session"] = serde_json::json!(ledger.session);
        record["turn"]["stop_reason"] =
            serde_json::json!(ledger.stop_reason.map(super::turn::TurnStopReason::as_str));
    });
    record["workspace_state"] = trace_schema::workspace_state(
        crate::knowledge::experience::current_turn_workspace().as_deref(),
    );
    trace_schema::attach_start(&mut record["workspace_state"]);
    record["cost"] = cost::cost_for_attempts(
        record["identity"]["model"]["id"].as_str(),
        record["usage"].as_object().map(|_| &record["usage"]),
        &provider_call_samples(),
    );
}

fn take_first_action() -> Option<(usize, u64)> {
    FIRST_ACTION.with(|cell| cell.take())
}

pub(crate) fn eval_owns_label() -> bool {
    EVAL_OWNS_LABEL.with(|f| f.get())
}

pub(crate) struct EvalLabelScope(bool);

impl EvalLabelScope {
    pub(crate) fn new() -> Self {
        Self(EVAL_OWNS_LABEL.with(|f| f.replace(true)))
    }
}

impl Drop for EvalLabelScope {
    fn drop(&mut self) {
        EVAL_OWNS_LABEL.with(|f| f.set(self.0));
    }
}

#[allow(dead_code)]
pub fn log_eval_trajectory(
    club_label: &str,
    history: &[ChatMsg],
    answer: &str,
    reward: f32,
    evaluator_evidence_manifest_sha256: &str,
) {
    if std::env::var_os("ANGEL_TRAJECTORY_LOG").is_none() {
        return;
    }
    let Some(repo) = crate::knowledge::experience::current_turn_repo_value() else {
        return;
    };
    let mut record = eval_trajectory_record(
        club_label,
        history,
        answer,
        reward,
        evaluator_evidence_manifest_sha256,
        now_ms(),
    );
    record["repo"] = repo;
    attach_turn_ledger(&mut record, None);
    write_trajectory(&record);
}

pub(crate) fn log_verified_coding_eval_trajectory(
    club_label: &str,
    history: &[ChatMsg],
    answer: &str,
    task: &str,
    decision: &crate::drive::reinforce::training::PublishedDecision,
) -> Result<Value, String> {
    let repo = crate::knowledge::experience::current_turn_repo_value()
        .ok_or("coding producer has no exact workspace scope")?;
    let mut record = eval_trajectory_record(
        club_label,
        history,
        answer,
        decision.reward,
        &decision.manifest_sha256,
        now_ms(),
    );
    record["data_class"] = serde_json::json!("measured_coding_eval");
    record["evaluator_task"] = serde_json::json!(task);
    record["evaluator_decision_sha256"] = serde_json::json!(decision.decision_sha256);
    record["evaluator_artifact_sha256"] = serde_json::json!(decision.artifact_sha256);
    record["competition"] = Value::Null;
    record["repo"] = repo;
    write_trajectory(&record);
    Ok(record)
}

pub(crate) fn eval_trajectory_record(
    club_label: &str,
    history: &[ChatMsg],
    answer: &str,
    reward: f32,
    evaluator_evidence_manifest_sha256: &str,
    ts_ms: u64,
) -> Value {
    let mut record = trajectory_record(club_label, history, answer, 0, false, Some(reward), ts_ms);
    record["evaluator_evidence_manifest_sha256"] =
        serde_json::json!(evaluator_evidence_manifest_sha256);
    record
}

#[cfg(test)]
#[path = "../../../../tests/cockpit/harness/trajectory__progress_tests.rs"]
mod progress_tests;

pub(crate) fn note_tool_routing(receipt: &super::shell_verifier::RoutingReceipt) {
    TURN_LEDGER.with(|cell| {
        if let Some(entry) = cell.borrow_mut().tools.last_mut() {
            entry["routed_to"] = serde_json::json!(
                receipt
                    .routed_call
                    .as_ref()
                    .map(|call| call.name.as_str())
                    .unwrap_or("none")
            );
            entry["routed_cwd"] = serde_json::json!(receipt.routed_cwd);
            entry["routing_reason"] = serde_json::json!(receipt.reason);
            entry["routed_args"] =
                serde_json::json!(receipt.routed_call.as_ref().map(|call| &call.args));
        }
    });
}

#[cfg(test)]
#[path = "../../../../tests/cockpit/harness/trajectory__process_kill_tests.rs"]
mod process_kill_tests;

pub(crate) fn retain_tools_output(value: super::turn::background::ToolsOutput) {
    TURN_LEDGER.with(|cell| cell.borrow_mut().tools_output = value);
}
pub(crate) fn tools_output_snapshot() -> super::turn::background::ToolsOutput {
    TURN_LEDGER.with(|cell| cell.borrow().tools_output.clone())
}
pub(crate) fn store_rotations_snapshot() -> Vec<Value> {
    TURN_LEDGER.with(|cell| cell.borrow().store_rotations.clone())
}

#[cfg(test)]
#[path = "../../../../tests/cockpit/harness/trajectory__research_tests.rs"]
mod research_tests;

#[cfg(test)]
#[path = "../../../../tests/cockpit/harness/trajectory__r06_progress_tests.rs"]
mod r06_progress_tests;

#[cfg(test)]
#[path = "../../../../tests/cockpit/harness/trajectory__mutation_tests.rs"]
mod mutation_tests;
