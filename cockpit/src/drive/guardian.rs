//! The sleeping giant: `angel --guardian` watches peer heartbeats, and
//! when an instance dies mid-session it resurrects that session on this
//! rig — recovering the thread (locally or from the latest backup
//! snapshot), handing it to this rig's configured model with an
//! assess-repair-continue brief, and leaving a durable record of what
//! happened. `angel --resurrect <session>` is the one-shot form the
//! guardian spawns (and an operator may run directly).
//!
//! Fully automatic by invocation: running `--guardian` *is* the consent.
//! `--watch-only` demotes it to detection + logging. Fencing lives in
//! `platform::guardian` (stale horizon, live-session check, atomic
//! claim, once-per-death record); a guardian only ever acts on sessions
//! whose workspace exists on its own rig, so a fleet of guardians
//! divides the dead by checkout locality and the claim breaks ties.

use crate::agent::club::{Bag, ChatMsg};
use crate::agent::harness;
use crate::knowledge::session;
use crate::platform::guardian as guard;
use crate::platform::peer_watch::{PeerBeat, peer_dir, read_peers};
use crate::platform::workspace_store::angel_dir;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc;
use std::time::Duration;

fn now_ms() -> u128 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis())
        .unwrap_or(0)
}

fn host_name() -> String {
    std::fs::read_to_string("/proc/sys/kernel/hostname")
        .map(|s| s.trim().to_string())
        .ok()
        .filter(|s| !s.is_empty())
        .or_else(|| std::env::var("HOSTNAME").ok())
        .unwrap_or_else(|| "unknown".to_string())
}

fn take_flag(args: &[String], name: &str) -> Option<String> {
    args.windows(2)
        .find(|pair| pair[0] == name)
        .map(|pair| pair[1].clone())
}

/// The mandate handed to the backup model. Deliberately blunt about
/// uncertainty: the previous instance may have died mid-tool-call, and
/// the recovered thread marks those outcomes unknown.
fn recovery_brief(
    dead_host: &str,
    dead_instance: &str,
    old_session_id: &str,
    new_session_id: &str,
) -> String {
    format!(
        "[guardian resurrection] The angelX instance that owned this session \
         (instance {dead_instance} on {dead_host}, session {old_session_id}) \
         has gone silent and is presumed dead. You are its backup, running \
         on {} in a fresh session ({new_session_id}) seeded with its \
         recovered thread. Tool calls recorded without a result were \
         interrupted mid-flight: their outcomes are UNKNOWN — check before \
         assuming any of them happened, and do not blindly replay them.\n\n\
         Your mandate, in order:\n\
         1. Assess: establish where the work stopped. Inspect git status/diff \
         and any half-finished state in the workspace.\n\
         2. Repair: make the workspace consistent — finish or safely undo \
         anything left half-applied, and get the project's own checks \
         (build/tests) to a known state.\n\
         3. Continue: carry the original task in this thread forward to a \
         checked stopping point, verifying as the thread's own standards \
         require.\n\
         4. Report: end with a concise account of what you found, what you \
         repaired, what you completed, and what (if anything) remains. \
         Do not commit or push unless the original task explicitly asked \
         for it.",
        host_name()
    )
}

/// Ensure `sessions/<id>.json` exists locally, fetching it (hash-verified)
/// from the latest backup snapshot when this rig never held the session.
fn ensure_session_local(state_root: &Path, session_id: &str) -> Result<(), String> {
    if session_id.is_empty()
        || session_id.contains('/')
        || session_id.contains("..")
        || !session_id
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_'))
    {
        return Err(format!("unsafe session id {session_id:?}"));
    }
    let destination = session::sessions_dir().join(format!("{session_id}.json"));
    if destination.is_file() {
        return Ok(());
    }
    let staging = state_root
        .join("guardian")
        .join(format!("fetch-{}", std::process::id()));
    let fetched = crate::platform::backup::fetch_latest_file(
        state_root,
        &format!("sessions/{session_id}.json"),
        &staging,
    )?;
    let bytes = std::fs::read(&fetched).map_err(|e| format!("read fetched session: {e}"))?;
    // Publish only into absence: a live local session file always wins.
    if !destination.exists() {
        crate::platform::workspace_store::write_atomic(&destination, &bytes)?;
    }
    let _ = std::fs::remove_dir_all(&staging);
    Ok(())
}

/// Background heartbeat for the resurrecting child, so peers (and the
/// world's giant) can see the resurrection while it runs.
fn spawn_beater(
    state_root: &Path,
    workspace: &Path,
    session_id: String,
    activity: &'static str,
) -> (Arc<AtomicBool>, std::thread::JoinHandle<()>) {
    let stop = Arc::new(AtomicBool::new(false));
    let flag = Arc::clone(&stop);
    let state_root = state_root.to_path_buf();
    let workspace = workspace.to_path_buf();
    let handle = std::thread::spawn(move || {
        let mut beat = PeerBeat::new(&state_root, &workspace, Some(session_id));
        beat.set_activity(activity, None);
        while !flag.load(Ordering::Acquire) {
            beat.beat(0.0, 0.0, 0.0);
            std::thread::sleep(Duration::from_secs(10));
        }
        beat.retire();
    });
    (stop, handle)
}

struct ResurrectArgs {
    session_id: String,
    workspace: PathBuf,
    driver: Option<String>,
    dead_instance: String,
    dead_host: String,
}

fn parse_resurrect_args(args: &[String]) -> Result<ResurrectArgs, String> {
    let session_id = args
        .first()
        .filter(|arg| !arg.starts_with('-'))
        .cloned()
        .ok_or_else(|| "usage: angel --resurrect <session-id> --workspace DIR".to_string())?;
    let workspace = take_flag(args, "--workspace")
        .map(PathBuf::from)
        .ok_or_else(|| "--resurrect requires --workspace DIR".to_string())?;
    Ok(ResurrectArgs {
        session_id,
        workspace,
        driver: take_flag(args, "--driver")
            .or_else(|| std::env::var("ANGEL_GUARDIAN_DRIVER").ok())
            .filter(|d| !d.trim().is_empty()),
        dead_instance: take_flag(args, "--dead-instance").unwrap_or_else(|| "unknown".into()),
        dead_host: take_flag(args, "--dead-host").unwrap_or_else(|| "unknown host".into()),
    })
}

/// `angel --resurrect <session-id> --workspace DIR` — one-shot recovery.
pub(crate) fn cli_resurrect(args: Vec<String>) -> std::io::Result<()> {
    let parsed = parse_resurrect_args(&args).map_err(std::io::Error::other)?;
    let state_root = angel_dir();
    let started_ms = now_ms();
    let outcome = run_resurrect(&state_root, &parsed);
    match outcome {
        Ok(record) => {
            let peers = peer_dir(&state_root);
            let _ = guard::write_record(&peers, &record);
            guard::log_line(
                &state_root,
                &format!(
                    "resurrect {} (was {}) -> {} success={}",
                    record.old_session_id,
                    record.dead_instance,
                    record.new_session_id.as_deref().unwrap_or("-"),
                    record.success
                ),
            );
            println!(
                "resurrection {}: session {} -> {} ({})",
                if record.success {
                    "complete"
                } else {
                    "stopped"
                },
                record.old_session_id,
                record.new_session_id.as_deref().unwrap_or("none"),
                record.summary.chars().take(200).collect::<String>()
            );
            if record.success {
                Ok(())
            } else {
                Err(std::io::Error::other(record.summary))
            }
        }
        Err(error) => {
            let record = guard::ResurrectionRecord {
                dead_instance: parsed.dead_instance.clone(),
                dead_host: parsed.dead_host.clone(),
                old_session_id: parsed.session_id.clone(),
                new_session_id: None,
                guardian_host: host_name(),
                started_ms,
                finished_ms: now_ms(),
                success: false,
                summary: error.clone(),
            };
            let _ = guard::write_record(&peer_dir(&state_root), &record);
            guard::log_line(&state_root, &format!("resurrect failed: {error}"));
            Err(std::io::Error::other(error))
        }
    }
}

fn run_resurrect(
    state_root: &Path,
    parsed: &ResurrectArgs,
) -> Result<guard::ResurrectionRecord, String> {
    let started_ms = now_ms();
    let workspace = parsed
        .workspace
        .canonicalize()
        .map_err(|e| format!("workspace {}: {e}", parsed.workspace.display()))?;
    if !workspace.is_dir() {
        return Err(format!(
            "workspace {} is not a directory",
            workspace.display()
        ));
    }
    ensure_session_local(state_root, &parsed.session_id)?;
    let recovered = session::load_for(&parsed.session_id, &workspace).map_err(|e| {
        format!(
            "cannot load session {} for {}: {e} (resurrection needs the same \
             checkout path the dead instance used)",
            parsed.session_id,
            workspace.display()
        )
    })?;

    // The resurrection runs in a NEW session seeded with the recovered
    // thread; the dead instance's session file is never written again.
    let mut session = session::Session::new();
    session.bind(&workspace);
    let new_session_id = session.id.clone();
    let brief = recovery_brief(
        &parsed.dead_host,
        &parsed.dead_instance,
        &parsed.session_id,
        &new_session_id,
    );
    let mut history = recovered;
    history.push(ChatMsg::user(brief.clone()));
    session
        .save(&history)
        .map_err(|e| format!("checkpoint resurrected session: {e}"))?;

    let (stop_beat, beater) = spawn_beater(
        state_root,
        &workspace,
        new_session_id.clone(),
        "resurrecting",
    );
    let result = run_recovery_turn(&workspace, &mut history, &brief, parsed.driver.as_deref());
    stop_beat.store(true, Ordering::Release);
    let _ = beater.join();

    let (success, summary) = match result {
        Ok(answer) => {
            let _ = session.save(&history);
            (true, answer)
        }
        Err(error) => {
            let _ = session.save(&history);
            (false, error)
        }
    };
    Ok(guard::ResurrectionRecord {
        dead_instance: parsed.dead_instance.clone(),
        dead_host: parsed.dead_host.clone(),
        old_session_id: parsed.session_id.clone(),
        new_session_id: Some(new_session_id),
        guardian_host: host_name(),
        started_ms,
        finished_ms: now_ms(),
        success,
        summary: summary.chars().take(2000).collect(),
    })
}

/// One headless recovery turn on this rig's configured model, mirroring
/// the `--task` path's registry and turn invocation.
fn run_recovery_turn(
    workspace: &Path,
    history: &mut Vec<ChatMsg>,
    brief: &str,
    driver: Option<&str>,
) -> Result<String, String> {
    let mut bag = Bag::standard();
    let club = match driver {
        Some(requested) => bag
            .require_task_driver(requested)
            .map_err(|reason| format!("guardian driver unavailable: {reason:?}"))?,
        None => bag.in_hand(),
    };
    Arc::clone(&club).prewarm();
    let roster = crate::app::bootstrap::delegation_roster(&bag);
    let skills = harness::load_skills_for(workspace);
    let mut registry = harness::ToolRegistry::with_team_self_in_run(
        workspace.to_path_buf(),
        roster,
        Some(Arc::clone(&club)),
        bag.codex_startup.as_deref(),
    );
    registry.set_skill_index(&skills);
    registry.enable_tool_search();
    // A resurrected session is the operator's live work, not a sealed
    // benchmark task: keep it free of the headless per-call default unless the
    // operator set a cap.
    if std::env::var_os("ANGEL_TASK_CALL_TIMEOUT_SECS").is_none()
        && std::env::var_os("ANGEL_TEST_RUN_TIMEOUT_SECS").is_none()
    {
        // TODO: Audit that the environment access only happens in single-threaded code.
        unsafe { std::env::set_var("ANGEL_TASK_CALL_TIMEOUT_SECS", "0") };
    }
    let task_pace = harness::apply_task_runtime_defaults(brief);
    let max_hops = harness::configured_max_hops();
    let competition = harness::competition_mode_active(history);
    let deadline_secs = harness::configured_turn_deadline_secs_for(competition);
    let runtime = harness::TaskRuntimeConfig::new(
        crate::knowledge::cut::sha256_hex(brief.as_bytes()),
        driver.map(str::to_string),
        None,
        task_pace,
        max_hops,
        deadline_secs,
        "auto".to_string(),
        "off".to_string(),
        false,
        crate::platform::yolo::enabled(),
    );
    let binding = runtime.rollout_binding(None, None);
    let (event_tx, event_rx) = mpsc::channel::<harness::TurnEvent>();
    let drain = std::thread::spawn(move || {
        for event in event_rx.iter() {
            match event {
                harness::TurnEvent::ToolCall { name, .. } => {
                    eprintln!("[resurrect] call {name}")
                }
                harness::TurnEvent::Notice(note) => eprintln!("[resurrect] {note}"),
                _ => {}
            }
        }
    });
    let cancel = AtomicBool::new(false);
    let result = harness::run_task_turn_observed(
        &*club, &registry, history, &cancel, max_hops, &event_tx, &binding, driver,
    );
    drop(event_tx);
    let _ = drain.join();
    match result {
        Ok(outcome) => {
            if outcome.stop_reason == harness::TurnStopReason::Answer {
                Ok(outcome.answer)
            } else {
                Err(format!("recovery turn stopped: {:?}", outcome.stop_reason))
            }
        }
        Err(failure) => Err(format!("recovery turn failed: {failure:?}")),
    }
}

// ---------------------------------------------------------------------------
// The watcher loop.
// ---------------------------------------------------------------------------

const GUARDIAN_USAGE: &str = "\
usage: angel --guardian [--watch-only] [--once] [--stale-secs N] [--driver ROUTE]
       angel --resurrect <session-id> --workspace DIR [--driver ROUTE]

--guardian watches peer heartbeats; a peer silent past the stale horizon
(default 180s) whose workspace exists on this rig is claimed and its
session resurrected by this rig's model (assess, repair, continue).
--watch-only detects and logs without acting. --once scans a single time
(cron-friendly) instead of looping. Running --guardian at all is the
consent for automatic resurrection; fencing (atomic claim, live-session
check, once-per-death record) is always on.
";

pub(crate) fn cli_guardian(args: Vec<String>) -> std::io::Result<()> {
    if args.iter().any(|arg| arg == "--help" || arg == "-h") {
        println!("{GUARDIAN_USAGE}");
        return Ok(());
    }
    let watch_only = args.iter().any(|arg| arg == "--watch-only");
    let once = args.iter().any(|arg| arg == "--once");
    let stale_ms: u128 = take_flag(&args, "--stale-secs")
        .and_then(|raw| raw.parse::<u128>().ok())
        .map(|secs| secs.saturating_mul(1000))
        .unwrap_or(guard::DEFAULT_STALE_MS);
    let driver = take_flag(&args, "--driver")
        .or_else(|| std::env::var("ANGEL_GUARDIAN_DRIVER").ok())
        .filter(|d| !d.trim().is_empty());
    let state_root = angel_dir();
    let peers_dir = peer_dir(&state_root);
    let workspace = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
    let mut beat = PeerBeat::new(&state_root, &workspace, None);
    beat.set_activity(GuardianPhase::Sleeping.label(), None);
    let own_id = beat.heartbeat.instance_id.clone();
    guard::log_line(
        &state_root,
        &format!(
            "guardian up instance={own_id} peers={} stale_ms={stale_ms} watch_only={watch_only} once={once}",
            peers_dir.display()
        ),
    );
    let mut guardian = Guardian::new(state_root, peers_dir, stale_ms, watch_only, driver, beat);
    loop {
        guardian.turn();
        if once {
            // Single scan; if we spawned a resurrection, stay its parent
            // until it lands so cron sees the real outcome.
            if let Some((mut running, _, _)) = guardian.child.take() {
                let status = running.wait().map_err(std::io::Error::other)?;
                guardian.beat.retire();
                if !status.success() {
                    return Err(std::io::Error::other(format!(
                        "resurrect child exited {status}"
                    )));
                }
            }
            guardian.beat.retire();
            return Ok(());
        }
        std::thread::sleep(Duration::from_secs(10));
    }
}

/// A resurrection child this guardian is parenting: the process, the death it
/// answers, and when it started.
type Resurrection = (std::process::Child, guard::Candidate, u128);

/// What the guardian is doing, as its peers read it off the heartbeat. It is
/// derived from what the guardian holds (see [`Guardian::phase`]), never set
/// on its own, so a label cannot outlive its cause.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum GuardianPhase {
    Sleeping,
    /// Watch-only found a death it will not answer.
    Watching,
    /// A resurrection child is running.
    Resurrecting,
}

impl GuardianPhase {
    /// The heartbeat activity strings peers match on; a wire contract.
    fn label(self) -> &'static str {
        match self {
            GuardianPhase::Sleeping => "guardian-sleeping",
            GuardianPhase::Watching => "guardian-watching",
            GuardianPhase::Resurrecting => "guardian-resurrecting",
        }
    }
}

/// One running guardian: what it was started with, its heartbeat, and the
/// resurrection it is parenting, if any.
struct Guardian {
    state_root: PathBuf,
    peers_dir: PathBuf,
    stale_ms: u128,
    watch_only: bool,
    driver: Option<String>,
    beat: PeerBeat,
    child: Option<Resurrection>,
    /// The last scan found a death and this guardian is watch-only.
    watching: bool,
}

impl Guardian {
    fn new(
        state_root: PathBuf,
        peers_dir: PathBuf,
        stale_ms: u128,
        watch_only: bool,
        driver: Option<String>,
        beat: PeerBeat,
    ) -> Self {
        Self {
            state_root,
            peers_dir,
            stale_ms,
            watch_only,
            driver,
            beat,
            child: None,
            watching: false,
        }
    }

    fn phase(&self) -> GuardianPhase {
        if self.child.is_some() {
            GuardianPhase::Resurrecting
        } else if self.watching {
            GuardianPhase::Watching
        } else {
            GuardianPhase::Sleeping
        }
    }

    /// Put the current phase on the heartbeat. Called wherever the phase may
    /// have moved, so no branch has to remember to set or reset a label; a
    /// change is beaten out promptly.
    fn sync_activity(&mut self) {
        let label = self.phase().label();
        if self.beat.heartbeat.activity != label {
            self.beat.set_activity(label, None);
        }
    }

    /// One pass of the watcher loop: supervise the running resurrection, beat,
    /// and, when idle, look for a death to answer.
    fn turn(&mut self) {
        // Supervise the running resurrection, if any.
        if let Some((running, candidate, started)) = &mut self.child {
            match running.try_wait() {
                Ok(Some(status)) => {
                    if guard::read_record(&self.peers_dir, &candidate.instance_id).is_none() {
                        let _ = guard::write_record(
                            &self.peers_dir,
                            &guard::ResurrectionRecord {
                                dead_instance: candidate.instance_id.clone(),
                                dead_host: candidate.host.clone(),
                                old_session_id: candidate.session_id.clone(),
                                new_session_id: None,
                                guardian_host: host_name(),
                                started_ms: *started,
                                finished_ms: now_ms(),
                                success: false,
                                summary: format!(
                                    "resurrect child exited {status} without a record"
                                ),
                            },
                        );
                    }
                    guard::log_line(
                        &self.state_root,
                        &format!(
                            "resurrect child for {} exited {status}",
                            candidate.instance_id
                        ),
                    );
                    self.child = None;
                }
                Ok(None) => {}
                Err(error) => {
                    guard::log_line(&self.state_root, &format!("child wait failed: {error}"));
                    self.child = None;
                }
            }
        }
        self.sync_activity();
        self.beat.beat(0.0, 0.0, 0.0);
        if self.child.is_none() {
            self.watching = false;
            let peers = read_peers(&self.peers_dir, Some(&self.beat.heartbeat.instance_id));
            let candidates = guard::select_candidates(
                &peers,
                now_ms(),
                self.stale_ms,
                |path| path.is_dir() && path.join(".git").exists(),
                |instance, session| {
                    guard::read_record(&self.peers_dir, instance)
                        .is_some_and(|record| record.old_session_id == session)
                },
            );
            if let Some(candidate) = candidates.into_iter().next() {
                if self.watch_only {
                    guard::log_line(
                        &self.state_root,
                        &format!(
                            "watch-only: {} on {} is down (session {}), workspace {} present here",
                            candidate.instance_id,
                            candidate.host,
                            candidate.session_id,
                            candidate.workspace.display()
                        ),
                    );
                    self.watching = true;
                } else {
                    match guard::try_claim(
                        &self.peers_dir,
                        &candidate,
                        &self.beat.heartbeat.instance_id,
                        &host_name(),
                    ) {
                        Ok(true) => {
                            match spawn_resurrect(
                                &self.state_root,
                                &candidate,
                                self.driver.as_deref(),
                            ) {
                                Ok(spawned) => {
                                    guard::log_line(
                                        &self.state_root,
                                        &format!(
                                            "claimed {} (session {}), resurrecting",
                                            candidate.instance_id, candidate.session_id
                                        ),
                                    );
                                    self.child = Some((spawned, candidate, now_ms()));
                                }
                                Err(error) => {
                                    guard::log_line(
                                        &self.state_root,
                                        &format!("spawn resurrect failed: {error}"),
                                    );
                                    let _ = guard::write_record(
                                        &self.peers_dir,
                                        &guard::ResurrectionRecord {
                                            dead_instance: candidate.instance_id.clone(),
                                            dead_host: candidate.host.clone(),
                                            old_session_id: candidate.session_id.clone(),
                                            new_session_id: None,
                                            guardian_host: host_name(),
                                            started_ms: now_ms(),
                                            finished_ms: now_ms(),
                                            success: false,
                                            summary: format!("spawn failed: {error}"),
                                        },
                                    );
                                }
                            }
                        }
                        Ok(false) => {}
                        Err(error) => {
                            guard::log_line(&self.state_root, &format!("claim failed: {error}"))
                        }
                    }
                }
            }
        }
        self.sync_activity();
    }
}

fn spawn_resurrect(
    state_root: &Path,
    candidate: &guard::Candidate,
    driver: Option<&str>,
) -> Result<std::process::Child, String> {
    let exe = std::env::current_exe().map_err(|e| format!("locate own binary: {e}"))?;
    let log_dir = state_root.join("guardian");
    std::fs::create_dir_all(&log_dir).map_err(|e| format!("create {}: {e}", log_dir.display()))?;
    let log_path = log_dir.join(format!("resurrect-{}.log", candidate.instance_id));
    let log = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&log_path)
        .map_err(|e| format!("open {}: {e}", log_path.display()))?;
    let log_err = log.try_clone().map_err(|e| format!("clone log: {e}"))?;
    let mut command = std::process::Command::new(exe);
    command
        .arg("--resurrect")
        .arg(&candidate.session_id)
        .arg("--workspace")
        .arg(&candidate.workspace)
        .arg("--dead-instance")
        .arg(&candidate.instance_id)
        .arg("--dead-host")
        .arg(&candidate.host)
        .stdin(std::process::Stdio::null())
        .stdout(log)
        .stderr(log_err);
    if let Some(driver) = driver {
        command.arg("--driver").arg(driver);
    }
    command.spawn().map_err(|e| format!("spawn resurrect: {e}"))
}

#[cfg(test)]
#[path = "../../../tests/cockpit/app/drive_guardian__heartbeat_tests.rs"]
mod tests;
