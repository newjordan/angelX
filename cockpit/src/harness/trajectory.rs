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

/// Brief one-line summary of tool-call args for the activity trace.
/// Default hops (`read_file {path}`) skip the parts Vec and per-key format!.
pub(crate) fn summarize_args(args: &Value) -> String {
    if let Some(command) = args.get("command").and_then(Value::as_str) {
        return crate::secrets::redact_str(command)
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

/// Brief one-line summary of a tool result for the activity trace.
pub(crate) fn summarize_result(result: &str) -> String {
    result
        .lines()
        .next()
        .unwrap_or("(empty)")
        .chars()
        .take(80)
        .collect()
}

/// Tool-call hop budget for one agent turn. This is a *runaway guard*, not a
/// reasoning limit: nex2-style extended-reasoning clubs loop through many tool
/// calls by design, and agentic drivers (spark) are multi-step too — so the
/// default is generous. Override with `ANGEL_MAX_HOPS`; set it to `0` to run
/// unbounded (loop until the club produces a text answer).
pub fn default_max_hops() -> usize {
    match std::env::var("ANGEL_MAX_HOPS")
        .ok()
        .and_then(|s| s.trim().parse::<usize>().ok())
    {
        Some(0) => usize::MAX, // unbounded
        Some(n) => n,
        None => usize::MAX,
    }
}

// ---------------------------------------------------------------------------
// Trajectory logging — persist each run_turn rollout as JSONL for offline RL /
// analysis. The raw training data the reinforce loop feeds on. Opt-in via
// `ANGEL_TRAJECTORY_LOG`; dir overridable via `ANGEL_TRAJECTORY_DIR` (default
// ~/.angel0/trajectories; under cfg(test) a per-process temp dir, so cockpit
// test suites never feed the live trainer inbox). One file per process,
// appended.
// ---------------------------------------------------------------------------

fn trajectory_messages(history: &[ChatMsg]) -> Vec<Value> {
    crate::club::messages_to_json(history, true)
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

/// Last-turn root Hi/Q snapshot for the agent strip (process-local, no disk).
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

/// Serialize the root-only LID view of a rollout for RL analysis and forge
/// metadata. Bulk tool bodies are classified, not embedded — this is what the
/// root model actually conditions on under Hi/Q offload.
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
        // Strategy token *list* (call:/tool:handle/…) plus a scalar length for
        // short↔long ablation without re-counting the array in forge.
        "tokens": root.tokens,
        "token_n": root.tokens.len(),
        "root_chars": root.root_chars,
        "tool_hops": root.tool_hops,
        "handle_receipts": root.handle_receipts,
        "aged_receipts": root.aged_receipts,
        "bulk_tool_results": root.bulk_tool_results,
        // LID mass ratio: share of tool results that left root as strategy
        // receipts rather than bulk. Useful for short↔long trajectory pairing.
        "offload_ratio": ratio,
        // Forge curriculum priority in [0.5, 2.0]: high offload + treebeard
        // lane preferred for Zhang/Khattab-style generalization training.
        "hiq_priority": prio,
    })
}

/// LID mass ratio: handle+aged receipts over all classified tool results.
pub(crate) fn offload_ratio(handle: usize, aged: usize, bulk: usize) -> f64 {
    let offloaded = handle.saturating_add(aged) as f64;
    let total = offloaded + bulk as f64;
    if total <= 0.0 { 0.0 } else { offloaded / total }
}

/// Relative forge/curriculum weight for a trajectory under Hi/Q training.
///
/// `0.5` bulk-stuffed ReAct → `1.75` fully offloaded default → `2.1875`
/// Treebeard before competition bonuses. Cap matches forge
/// `FORGE_HIQ_WEIGHT_MAX` default (3.0) so PRIMARY open-lever stacks on the
/// forge side remain distinguishable (old hard clamp 2.5 flat-lined PRIMARY).
/// Does not change The Cut's compile reward.
pub(crate) fn hiq_priority(offload_ratio: f64, lane: Lane) -> f64 {
    let ratio = offload_ratio.clamp(0.0, 1.0);
    // Linear map: ratio 0 → 0.5, ratio 1 → 1.75 before lane bonus.
    let mut w = 0.5 + 1.25 * ratio;
    if lane == Lane::Treebeard {
        w *= 1.25;
    }
    // Env override keeps cockpit ↔ forge contract single-sourced for night ops.
    let wmax = std::env::var("FORGE_HIQ_WEIGHT_MAX")
        .ok()
        .and_then(|s| s.parse::<f64>().ok())
        .unwrap_or(3.0)
        .clamp(1.0, 5.0);
    w.clamp(0.5, wmax)
}

#[cfg(test)]
#[path = "../../../tests/cockpit/harness/trajectory__hiq_tests.rs"]
mod hiq_tests;

/// Ablatable harness treatment tags for RL splits (Hi/Q on vs discard-only,
/// handle_read on/off). Always cheap; no model call.
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
    map.insert(
        "gpu_comp".into(),
        env_flag("ANGEL_GPU_COMP_LOCAL_MOA", false).into(),
    );
    // Phase-4 RL scorer label (GpuComp pins popcorn_peer). Stamps Cut/Forge
    // goldens so preference densify can join the same reward contract.
    {
        let rl = std::env::var("ANGEL_RL_REWARD")
            .ok()
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
            .unwrap_or_else(|| {
                if env_flag("ANGEL_GPU_COMP_LOCAL_MOA", false) {
                    "popcorn_peer".into()
                } else {
                    String::new()
                }
            });
        if !rl.is_empty() {
            map.insert("rl_reward".into(), Value::String(rl));
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
    // Living B200 peer frontier — stamps Cut/Forge goldens with the competition
    // subject so curriculum can filter/join peer-relative rewards later.
    if let Some((geo, name, path)) = load_living_peer_snapshot() {
        map.insert(
            "living_peer_us".into(),
            serde_json::json!((geo * 100.0).round() / 100.0),
        );
        map.insert("living_peer_name".into(), Value::String(name));
        if let Some(p) = path {
            map.insert("living_peer_path".into(), Value::String(p));
        }
    }
    if let Some(p1) = load_living_peer_p1_us() {
        map.insert(
            "living_peer_p1_us".into(),
            serde_json::json!((p1 * 10.0).round() / 10.0),
        );
    }
    let holds = load_living_peer_shape_holds(8);
    if !holds.is_empty() {
        map.insert(
            "living_peer_shape_holds_n".into(),
            serde_json::json!(holds.len()),
        );
        // Compact top keys for Hi/Q join without stuffing full shape tables.
        let keys: Vec<String> = holds.iter().take(5).map(|h| h.key.clone()).collect();
        map.insert(
            "living_peer_shape_hold_keys".into(),
            serde_json::json!(keys),
        );
    }
    let open = load_living_peer_open_levers(5);
    if !open.is_empty() {
        map.insert(
            "living_peer_open_levers_n".into(),
            serde_json::json!(open.len()),
        );
        let keys: Vec<String> = open.iter().map(|l| l.key.clone()).collect();
        map.insert(
            "living_peer_open_lever_keys".into(),
            serde_json::json!(keys),
        );
        // Top absolute board µs (primary geomean lever) for curriculum join.
        if let Some(top) = open.first() {
            let top_us = (top.board_us * 10.0).round() / 10.0;
            map.insert("living_peer_top_open_us".into(), serde_json::json!(top_us));
            map.insert(
                "living_peer_top_open_key".into(),
                Value::String(top.key.clone()),
            );
            // Primary attack = rank-1 board µs (usually huge 32k). Half-cut impact
            // teaches equal-weight geomean: any shape 2× faster multiplies geo by 0.5^(1/n).
            // Canonical forge names (`living_peer_primary_key` / `_us`) match
            // strategy_student + free-train Hi/Q curriculum; keep `_attack` alias.
            map.insert(
                "living_peer_primary_key".into(),
                Value::String(top.key.clone()),
            );
            map.insert("living_peer_primary_us".into(), serde_json::json!(top_us));
            map.insert(
                "living_peer_primary_attack".into(),
                Value::String(top.key.clone()),
            );
            map.insert(
                "living_peer_primary_geo_drop_half_pct".into(),
                serde_json::json!((top.geo_drop_if_half_pct * 1000.0).round() / 1000.0),
            );
            if let Some(b) = top.best_us {
                map.insert(
                    "living_peer_primary_best_us".into(),
                    serde_json::json!((b * 10.0).round() / 10.0),
                );
            }
            if let Some(ref n) = top.best_name {
                map.insert(
                    "living_peer_primary_best_name".into(),
                    Value::String(n.clone()),
                );
            }
        }
    }
    if let Ok(peer_log) = std::env::var("POPCORN_PEER_LOG")
        && !peer_log.is_empty()
    {
        map.insert("popcorn_peer_log".into(), Value::String(peer_log));
    }
    // Free-train / last LoRA cycle handoff — stamps Cut goldens so curriculum
    // can join adapter version + gate without re-reading forge logs.
    if let Some(snap) = load_forge_train_snap()
        && snap.state == "done"
    {
        if let Some(ref ver) = snap.version {
            map.insert("forge_adapter_version".into(), Value::String(ver.clone()));
        }
        if let Some(g) = snap.gate_pass {
            map.insert("forge_gate_pass".into(), Value::Bool(g));
        }
        if snap.adapter_local {
            map.insert("forge_adapter_local".into(), Value::Bool(true));
        }
        if let Some(p) = snap.promoted {
            map.insert("forge_promoted".into(), Value::Bool(p));
        }
        if let Some(loss) = snap.train_loss
            && loss.is_finite()
            && loss > 0.0
        {
            map.insert(
                "forge_train_loss".into(),
                serde_json::json!((loss * 10000.0).round() / 10000.0),
            );
        }
        if let Some(lo) = snap.train_loss_min
            && lo.is_finite()
            && lo > 0.0
        {
            map.insert(
                "forge_train_loss_min".into(),
                serde_json::json!((lo * 10000.0).round() / 10000.0),
            );
        }
        if let Some(hi) = snap.train_loss_max
            && hi.is_finite()
            && hi > 0.0
        {
            map.insert(
                "forge_train_loss_max".into(),
                serde_json::json!((hi * 10000.0).round() / 10000.0),
            );
        }
    }
    Value::Object(map)
}

/// Whether this record should carry root-trajectory / treatment metadata.
/// Reward-labeled rows are training data and always get it. Unlabeled logs
/// keep the lighter shape unless `ANGEL_ROOT_TRAJECTORY=1` requests analysis.
pub(crate) fn should_attach_root_trajectory(reward: Option<f32>) -> bool {
    reward.is_some() || env_flag("ANGEL_ROOT_TRAJECTORY", false)
}

/// Build the JSON record for one rollout. Pure w.r.t. club/history/answer; may
/// read handle-store session stats and env for treatment tags. `log_trajectory`
/// wraps it with the remaining side effects.
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
    // A reward-labeled record is training data; an unlabeled one is just a log.
    if let Some(r) = reward {
        rec["reward"] = serde_json::json!(r);
    }
    // Always refresh the process-local strip snapshot (last_root_hiq) so the
    // agent bay shows offload% even when the trajectory file omits root meta
    // (unlabeled + ANGEL_ROOT_TRAJECTORY off). Cheap classify; no disk I/O.
    let root_json = root_trajectory_json(history);
    // Hi/Q ↔ RL bridge: labeled rollouts always carry the root-only trajectory
    // fingerprint and the ablatable harness treatment so forge/promotion can
    // compare Hi/Q vs baseline and train on strategy-level isomorphism.
    if should_attach_root_trajectory(reward) {
        rec["root_trajectory"] = root_json;
        rec["harness_treatment"] = harness_treatment_json();
    }
    trace_schema::defaults(&mut rec);
    // Standalone evaluation rows also need the current cost schema. No usage
    // observation means explicitly unknown cost, never a fabricated zero.
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
        // Cockpit test suites must never feed the live trainer inbox: a
        // practice-club rollout written to the real ~/.angel0/trajectories is
        // rsynced to the Spark trainer within ten minutes. Unit-test binaries
        // default to a per-process temp directory instead.
        return std::env::temp_dir()
            .join(format!("angel-trajectories-test-{}", std::process::id()));
    }
    std::env::var_os("HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("/tmp"))
        .join(".angel0/trajectories")
}

pub(crate) fn now_ms() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

static TRAJECTORY_WRITE_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

/// Append one complete JSON value as one line. Parallel tool/agent workers can
/// finish turns on the same process simultaneously, so serialize the append
/// and encode before opening the file; otherwise separate `write_fmt` calls can
/// interleave into corrupt JSONL.
pub(crate) fn append_trajectory(path: &Path, record: &Value) -> std::io::Result<()> {
    let _guard = TRAJECTORY_WRITE_LOCK
        .lock()
        .unwrap_or_else(|e| e.into_inner());
    rotation::append(
        path,
        record,
        crate::store_caps::value("trajectories", "max_bytes"),
        std::time::Duration::from_secs(
            crate::store_caps::value("trajectories", "max_age_days") * 86400,
        ),
    )
}

/// Append one record to the per-process JSONL log, if `ANGEL_TRAJECTORY_LOG`
/// is set. Best-effort: any IO error is silently ignored (logging must never
/// break a turn).
pub(crate) fn write_trajectory(record: &Value) {
    if std::env::var_os("ANGEL_TRAJECTORY_LOG").is_none() {
        return;
    }
    let dir = trajectory_dir();
    if crate::workspace_store::private_io::PrivateDirectory::open(&dir).is_err() {
        return;
    }
    ensure_private_store(&dir);
    let path = dir.join(format!("session-{}.jsonl", std::process::id()));
    let _ = append_trajectory(&path, record);
}

/// Keep the trajectory store, and the `~/.angel` root above it, readable by
/// the owner only. The launcher's umask left the store at 775 (audit S08);
/// a shared box must not let another account read rollouts.
pub(crate) fn ensure_private_store(dir: &Path) {
    #[cfg(unix)]
    {
        let mut targets = vec![dir.to_path_buf()];
        if let Some(parent) = dir.parent()
            && parent.file_name().is_some_and(|n| n == ".angel0")
        {
            targets.push(parent.to_path_buf());
        }
        for target in targets {
            if let Ok(directory) =
                crate::workspace_store::private_io::PrivateDirectory::open_existing(&target)
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

/// Log a rollout from `run_turn`.
///
/// `reward` is The Cut's machine verdict for the turn
/// ([`crate::cut::TurnVerdicts::reward`]): `Some` when angel wrote source and a
/// compiler answered, `None` — an unlabeled log, exactly as before — when it
/// wrote no code. When the eval path owns this rollout's label
/// ([`EvalLabelScope`]), preserve it as an explicit observation that cannot be
/// admitted through the unlabeled imitation path.
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
    let Some(repo) = crate::experience::current_turn_repo_value() else {
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
    /// Set while a coding eval owns the label of the rollout running on this
    /// thread. See [`EvalLabelScope`].
    static EVAL_OWNS_LABEL: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };

    /// `(hop, ms since turn start)` of this turn's FIRST mutating tool call —
    /// the RESULT-speed telemetry: how long recon lasted before action. Reset
    /// at turn start, stamped once, consumed by [`log_trajectory`].
    static FIRST_ACTION: std::cell::Cell<Option<(usize, u64)>> = const { std::cell::Cell::new(None) };
}

/// Record the first mutating action of the running turn (later calls no-op).
pub(crate) fn note_first_action(hop: usize, ms_since_turn_start: u64) {
    FIRST_ACTION.with(|cell| {
        if cell.get().is_none() {
            cell.set(Some((hop, ms_since_turn_start)));
        }
    });
}

/// Clear the first-action stamp at turn start (worker threads are reused).
pub(crate) fn reset_first_action() {
    FIRST_ACTION.with(|cell| cell.set(None));
}

/// Per-turn telemetry ledger attached to the trajectory record: provider usage
/// for the turn, the model/tool timing split, and one entry per dispatched
/// tool call (name, execution outcome, error class, verifier verdict, wall
/// time, result bytes). Until 2026-09-07 a record carried only the wire
/// messages and one scalar reward, so the trainer could not see *how fast* or
/// *how cleanly* a rollout got its answer — the signal every efficiency
/// reward needs. Thread-local like [`FIRST_ACTION`]: it scopes to the one
/// rollout running on this worker thread.
#[derive(Default)]
struct TurnLedger {
    tools_output: super::turn::background::ToolsOutput,
    store_rotations: Vec<Value>,
    tools: Vec<Value>,
    verifier: Vec<Value>,
    artifacts: Vec<Value>,
    lease: Option<Value>,
    turn_id: Option<String>,
    session: Option<String>,
    stop_reason: Option<&'static str>,
    timing_origin: Option<std::time::Instant>,
    active_model_call: Option<usize>,
    next_model_call: usize,
    provider_samples: Vec<Value>,
    parking_events: Vec<Value>,
    timing: Option<Value>,
    usage_before: Option<crate::club::AccountingView>,
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

/// Open the ledger at turn start: clear the previous turn's entries and
/// snapshot the driver's cumulative counters so usage lands as a delta.
pub(crate) fn reset_turn_ledger(club: &dyn Club) {
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
            // Delivery is the candidate, not a correctness or citation grade.
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

pub(crate) fn note_stop_reason(reason: &'static str) {
    TURN_LEDGER.with(|cell| cell.borrow_mut().stop_reason = Some(reason));
}

// Task lifecycle is separate from the per-turn ledger: resetting a turn must
// not discard process startup, and interactive turns have no process scope.
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
    crate::harness::clear_graph_turn_episodes();
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

pub(crate) fn end_model_request() {
    TURN_LEDGER.with(|cell| cell.borrow_mut().active_model_call = None);
}

/// Provider attempts nest inside harness calls; these samples never add to totals.
pub(crate) fn begin_provider_attempt(source_id: u64) -> Option<usize> {
    TURN_LEDGER.with(|cell| {
        let mut ledger = cell.borrow_mut();
        let parent = ledger.active_model_call?;
        let start = ledger.timing_origin?.elapsed().as_millis() as u64;
        let id = ledger.provider_samples.len();
        let retry_of = ledger
            .provider_samples
            .iter()
            .rposition(|sample| sample["model_call"] == parent && sample["source_id"] == source_id);
        ledger
            .provider_samples
            .push(serde_json::json!({"id":id,"start":start,
            "end":null,"retry_of":retry_of,"model_call":parent,"source_id":source_id,
            "usage":null,"request_bytes":null,"response_bytes":null}));
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
    observation: Option<crate::club::UsageObservation>,
) {
    TURN_LEDGER.with(|cell| {
        let mut ledger = cell.borrow_mut();
        let end = ledger
            .timing_origin
            .map(|origin| origin.elapsed().as_millis() as u64);
        if let Some(sample) = id.and_then(|id| ledger.provider_samples.get_mut(id)) {
            sample["end"] = serde_json::json!(end);
            sample["usage"] = serde_json::json!(observation.map(|o| o.raw));
            // Publish each field on every attempt, including retries without a
            // response. Unknown provider counters must never masquerade as zero.
            let [input, output, _, cached, _] = observation.map(|o| o.raw).unwrap_or([None; 5]);
            let included = observation
                .is_some_and(|o| o.contract.cache == crate::club::CacheConvention::Included);
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

/// One dispatched tool call, recorded before its result is folded into history.
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
        // Stable status projection for task-envelope consumers.
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
        && let Some(kill) = crate::sandbox::process_owner::KillReceipt::from_error(result)
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
            // Verdict is known, but it does not imply a numeric process exit.
            ledger.verifier.push(serde_json::json!({"attempt_id":attempt_id,
                "plan": name, "command":args.get("command").and_then(Value::as_str).map(crate::experience::scrub_secrets), "exit":null, "ms":elapsed_ms.map(|v| v as u64),
                "coverage_gap":"numeric_exit_not_captured; command_is_scrubbed_when_available"}));
        }
        if name == "present" {
            let path = args.get("url").and_then(Value::as_str);
            ledger.artifacts.push(serde_json::json!({"attempt_id":attempt_id,
                "kind":args.get("kind").and_then(Value::as_str), "path_digest":path.map(|p| crate::cut::sha256_hex(p.as_bytes())),
                "shown":null})); // successful dispatch does not prove terminal rendering
        }
        if name == "machine_test" {
            ledger.lease = Some(serde_json::json!({"kind":"fleet", "host":args.get("host").and_then(Value::as_str),
                "seat":null,"lease_id":null}));
        }
        if verify.is_some() {
            // A real verification obligation restores the coding contract.
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
                // Shell timeouts may be successful dispatches with a timeout
                // footer rather than a tool error. Retain the timeout class.
                entry["class"] = serde_json::json!("timeout");
                "timeout".to_string()
            } else {
                verify.unwrap_or("inconclusive").to_string()
            });
        }
        let signature = canonical_signature(name, args);
        entry["duplicate_of"] = serde_json::json!(ledger.signatures.get(&signature));
        let index = ledger.tools.len();
        ledger.signatures.entry(signature).or_insert(index);
        if execution == "ok"
            && name == "read_file"
            && let Some(path) = args.get("path").and_then(Value::as_str)
        {
            ledger.read_paths.insert(normalized_read_path(path));
        }
        // Execution credit is independent of green/completion credit. Repeating
        // the same verifier on unchanged bytes is not another experiment.
        let remote_receipt = execution == "ok"
            && !errored
            && matches!(name, "shell" | "proc_status")
            && is_remote_verification_receipt(result)
            && ledger.remote_receipts.insert(crate::cut::sha256_hex(result.as_bytes()));
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

// Lexical normalization prevents ./path aliases from manufacturing new reads.
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

// Explicitly sort object keys recursively even if serde_json enables preserve_order.
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
    crate::cut::sha256_hex(format!("{name}\0{}", canonical(args)).as_bytes())
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

/// Only stable, named failure lines earn new-failure credit. Timing summaries,
/// progress chatter, and arbitrary changing stdout are deliberately excluded.
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
                .map(|s| crate::cut::sha256_hex(s.as_bytes()))
        })
        .collect()
}

pub(crate) fn note_stream_cut(hop: usize) {
    TURN_LEDGER.with(|cell| *cell.borrow_mut().stream_cuts.entry(hop).or_default() += 1);
}

/// Direct byte snapshots also work in Git-free workspaces and for ignored files.
/// Failed confined reads cannot manufacture edit credit.
pub(crate) fn mutation_byte_snapshot(
    root: &Path,
    calls: &[ToolCall],
) -> Vec<(std::path::PathBuf, Option<String>)> {
    let mut paths = std::collections::BTreeSet::new();
    for call in calls {
        crate::cut::for_each_mutation_target_path(&call.name, &call.args, |path| {
            paths.insert(std::path::PathBuf::from(path));
            false
        });
    }
    paths
        .into_iter()
        .map(|path| {
            let hash = confined_read(root, &path)
                .ok()
                .map(|bytes| crate::cut::sha256_hex(&bytes));
            (path, hash)
        })
        .collect()
}

/// Finalize even model-only or pre-dispatch exit hops without changing control flow.
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

/// Evaluate completed hops only; the turn loop calls this before another request.
/// Progress and reset semantics are owned by the existing progress ledger.
pub(crate) fn unproductive_escalation(
    hop: usize,
    escalate: usize,
    stop: usize,
) -> (Option<String>, Option<String>) {
    TURN_LEDGER.with(|cell| {
        let mut ledger = cell.borrow_mut();
        if ledger.last_streak_evaluation_hop == Some(hop) {
            return (None, None);
        }
        ledger.last_streak_evaluation_hop = Some(hop);
        let streak = ledger.unproductive_streak;
        let last = ledger.last_verifier.as_deref().unwrap_or("not_run").to_string();
        let already_escalated = ledger.streak_escalated;
        let notice = if escalate > 0 && streak >= escalate && streak.is_multiple_of(escalate) {
            let elapsed_ms = ledger.timing_origin.map(|t| t.elapsed().as_millis() as u64);
            ledger.escalations.push(serde_json::json!({
                "kind": "unproductive_streak", "hop": hop, "streak": streak,
                "last_verifier": last, "elapsed_ms": elapsed_ms,
            }));
            ledger.streak_escalated = true;
            if ledger.research_turn {
                Some(format!("unproductive streak: {streak} consecutive actions added no new sources; deliver the answer with supporting citations or an explicit missing-evidence statement with NO citations"))
            } else {
            Some(format!("unproductive streak: {streak} consecutive actions changed nothing verifiable; the verifier's last outcome was {last}; produce a verified candidate, run the verifier with an explicit result, or report the blocker as your answer"))
            }
        } else {
            None
        };
        let diagnosis = if stop > 0 && streak >= stop && already_escalated {
            let mut digests = Vec::new();
            for tool in ledger.tools.iter().rev() {
                if let Some(digest) = tool["args_digest"].as_str()
                    && !digests.contains(&digest) {
                    digests.push(digest);
                    if digests.len() == 3 { break; }
                }
            }
            if ledger.research_turn {
                Some(format!("Escalated research turn: {streak} consecutive unproductive hops added no new sources; deliver the answer with supporting citations or an explicit missing-evidence statement with NO citations"))
            } else {
            Some(format!("Escalated unproductive turn: {streak} consecutive unproductive hops ({}–{hop}), with no progress since escalation; last verifier outcome: {last}; last credited progress: {}; last 3 distinct action digests (newest first): {}", hop.saturating_sub(streak) + 1, ledger.last_progress.as_ref().map(Value::to_string).unwrap_or_else(|| "none (hop 0)".into()), digests.join(", ")))
            }
        } else {
            None
        };
        (notice, diagnosis)
    })
}

pub(crate) fn note_escalation(hop: usize, kind: &str) {
    TURN_LEDGER.with(|cell| {
        cell.borrow_mut()
            .escalations
            .push(serde_json::json!({"hop": hop, "kind": kind}))
    });
}

/// Record the route that actually supplied a successful policy reply.
pub(crate) fn note_route_switch(
    hop: usize,
    from: &crate::club::RouteIdentity,
    to: &crate::club::RouteIdentity,
) {
    TURN_LEDGER.with(|cell| {
        cell.borrow_mut().escalations.push(serde_json::json!({
            "hop": hop, "kind": "route_switch", "from": from, "to": to,
        }));
    });
}

pub(crate) fn last_route_switch() -> Option<crate::club::RouteIdentity> {
    TURN_LEDGER.with(|cell| {
        cell.borrow()
            .escalations
            .iter()
            .rev()
            .find(|event| event["kind"] == "route_switch")
            .and_then(|event| serde_json::from_value(event["to"].clone()).ok())
    })
}

/// Keep the immutable first-request binding, but project the answering route
/// into each terminal receipt. Unknown endpoint/wire controls stay unbound.
pub(crate) fn turn_identity() -> Option<super::run_identity::RunIdentity> {
    let mut identity = super::run_identity::current()?.clone();
    if let Some(route) = last_route_switch() {
        identity.model.club = route.driver.clone();
        identity.model.driver = route.driver;
        identity.model.id = route.model.unwrap_or_else(|| "unbound".into());
        identity.model.base_url = "unbound".into();
        identity.effort = serde_json::json!("unbound");
        identity.budgets["output_tokens"] = serde_json::json!("unbound");
    }
    Some(identity)
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
            "unproductive_streak_max": ledger.unproductive_streak_max.max(
                if ledger.hop_pending && !ledger.hop_progress { ledger.unproductive_streak + 1 } else { 0 }
            ),
            "escalations": ledger.escalations,
        })
    })
}

/// The per-call tool ledger of the running turn (for the task envelope, which
/// consumers read without trajectory logging).
pub(crate) fn tool_ledger_snapshot() -> Vec<Value> {
    TURN_LEDGER.with(|cell| cell.borrow().tools.clone())
}

/// Merge the terminal outcome into its launch call; never append an unrelated
/// synthetic call or depend on the completion-notice queue being undrained.
pub(crate) fn note_proc_kills(kills: &[(u64, crate::sandbox::process_owner::KillReceipt)]) {
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

/// Latest timing split for the running turn (refreshed after every model wait
/// and tool batch, so the record written at any exit seam is current).
pub(crate) fn note_timing<T: serde::Serialize>(timing: &T) {
    if let Ok(v) = serde_json::to_value(timing) {
        TURN_LEDGER.with(|cell| cell.borrow_mut().timing = Some(v));
    }
}

/// `/ledger [N]` — the last N (default 12) recorded turns with their ledger,
/// newest last: hops, tool calls/errors, model and tool seconds, output and
/// reasoning tokens, the last verifier verdict and The Cut's reward. Reads the
/// trajectory dir this process writes to, so the operator sees the efficiency
/// of their own recent turns without leaving the cockpit.
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
    // Session files are append-only, so the newest records sit at the tail:
    // read the last TAIL_BYTES of each file instead of the whole thing (the live
    // corpus is ~700 MB across ~1,000 sessions; a whole-file scan froze the UI
    // for 8.7 s on 2026-09-07). Records without a ledger predate this build; once
    // the newest sessions have none, older ones will not either, so the search
    // ends after LEDGERLESS_SESSIONS consecutive misses instead of reading on.
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

/// The last `max` bytes of a file as text, starting at the first whole line.
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

/// Shared UI accounting retains reported subtotals and coverage. A durable
/// turn ledger must not present those subtotals as complete run measurements.
/// The raw per-attempt samples retain every actual provider observation.
pub(crate) fn complete_ledger_usage(report: &crate::club::AccountingReport) -> Value {
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

/// Attach the ledger (usage delta, timing, tools) to a record. Fields are
/// explicit-null when unknown; every existing legacy key remains available.
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
            // Same gate as task_mode::task_usage_delta, on borrowed views.
            let report = club.usage_accounting().delta(before);
            if report.attempts > 0 || report.untracked_sources {
                record["usage"] = complete_ledger_usage(&report);
            }
        }
        if let Some(timing) = &ledger.timing {
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
        record["lease"] = ledger.lease.clone().unwrap_or_else(|| {
            serde_json::json!({
            "kind":"local", "host":null,"seat":null,"lease_id":null})
        });
        record["turn"]["operator_turn_id"] = serde_json::json!(ledger.turn_id);
        record["turn"]["session"] = serde_json::json!(ledger.session);
        record["turn"]["stop_reason"] = serde_json::json!(ledger.stop_reason);
    });
    record["workspace_state"] =
        trace_schema::workspace_state(crate::experience::current_turn_workspace().as_deref());
    trace_schema::attach_start(&mut record["workspace_state"]);
    // Cost binds from the record's own usage totals against the list-price
    // table; the earlier explicit-null placeholder is always explained.
    record["cost"] = cost::cost_for(
        record["identity"]["model"]["id"].as_str(),
        record["usage"].as_object().map(|_| &record["usage"]),
    );
}

fn take_first_action() -> Option<(usize, u64)> {
    FIRST_ACTION.with(|cell| cell.take())
}

pub(crate) fn eval_owns_label() -> bool {
    EVAL_OWNS_LABEL.with(|f| f.get())
}

/// **Do not double-count.** The coding-eval path drives `run_turn` and *then*
/// scores the workspace with the test suite ([`log_eval_trajectory`]) — a
/// strictly stronger verdict than a compile check, since it says the code is
/// *right*, not merely that it builds. Without this guard the same rollout would
/// be written twice, both times labeled, with two different rewards, and the
/// forge would train on it as two samples.
///
/// While this guard is alive, `run_turn`'s own row is an explicit coding-eval
/// observation. Only the later authoritative eval row can be training data.
/// Thread-local, so it scopes to the
/// one rollout the eval is driving — a subagent spawned on another thread still
/// earns its own label for its own rollout.
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

/// Log a **reward-labeled** rollout — actual training data for the reinforce
/// loop. Called from the coding-eval path once a verifiable reward is known.
#[allow(dead_code)]
pub fn log_eval_trajectory(
    club_label: &str,
    history: &[ChatMsg],
    answer: &str,
    reward: f32,
    evaluator_evidence_manifest_sha256: &str,
) {
    log_eval_trajectory_ex(
        club_label,
        history,
        answer,
        reward,
        evaluator_evidence_manifest_sha256,
        None,
    );
}

/// Like [`log_eval_trajectory`], with optional competition meta for GpuComp /
/// popcorn coding seats (`score_us`, shape, reward contract). Forge Hi/Q
/// curriculum + preference pairs join on these fields.
pub fn log_eval_trajectory_ex(
    club_label: &str,
    history: &[ChatMsg],
    answer: &str,
    reward: f32,
    evaluator_evidence_manifest_sha256: &str,
    competition: Option<Value>,
) {
    if std::env::var_os("ANGEL_TRAJECTORY_LOG").is_none() {
        return;
    }
    let Some(repo) = crate::experience::current_turn_repo_value() else {
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
    if let Some(comp) = competition {
        record["competition"] = comp;
    }
    record["repo"] = repo;
    // The eval row is the reward-labeled training sample: it carries the same
    // turn ledger as the run_turn row (timing + per-call tool outcomes; usage
    // needs the club and is left to the run_turn row).
    attach_turn_ledger(&mut record, None);
    write_trajectory(&record);
}

/// Only the coding evaluator producer can attach this immutable decision.
/// The consumer independently audits it and emits the exact task/answer pair.
pub(crate) fn log_verified_coding_eval_trajectory(
    club_label: &str,
    history: &[ChatMsg],
    answer: &str,
    task: &str,
    decision: &crate::reinforce::training::PublishedDecision,
) -> Result<Value, String> {
    let repo = crate::experience::current_turn_repo_value()
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
    record["competition"] = serde_json::json!(decision.competition);
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
#[path = "../../../tests/cockpit/harness/trajectory__progress_tests.rs"]
mod progress_tests;

/// Adds dispatch-authenticated routing to the just-recorded tool entry.
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
#[path = "../../../tests/cockpit/harness/trajectory__process_kill_tests.rs"]
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
#[path = "../../../tests/cockpit/harness/trajectory__research_tests.rs"]
mod research_tests;

#[cfg(test)]
#[path = "../../../tests/cockpit/harness/trajectory__r06_progress_tests.rs"]
mod r06_progress_tests;

#[cfg(test)]
#[path = "../../../tests/cockpit/harness/trajectory__mutation_tests.rs"]
mod mutation_tests;
