//! Behavioral tests for the RL campaign: real attempts on the selected club,
//! the operator's own verifier as the measurement, the receipt-backed gate, the
//! audited release path, honest no-promotion, and cancellation.
//!
//! Every fixture here is an explicitly synthetic club and a throwaway Git
//! workspace. Nothing in this module is production fallback data: the campaign
//! itself requires the operator's own objective and verifier, and installs a
//! learned note only when an independently authored audit case approves it.

use super::*;
use crate::agent::club::{ChatMsg, ChatRole, ClubReply, StreamDelta, ToolDef};
use crate::tests::{TestEnvGuard, TestGitWorkspace, env_lock};
use serde_json::json;
use std::sync::mpsc;

#[path = "rl_ctl__campaign_tests__loop_campaign_tests.rs"]
mod loop_campaign_tests;
#[path = "rl_ctl__campaign_tests__research_tests.rs"]
mod research_tests;

/// The note a promoted policy carries in these fixtures.
const NOTE: &str = "ALWAYS-WRITE-THE-ARTIFACT-BEFORE-ANSWERING";
/// A deliberately worse proposal, used to prove the gate rejects regressions.
const WORSE: &str = "NEVER-TOUCH-THE-FILESYSTEM";

/// The verifier every fixture objective uses. It is the case's own script, in
/// the operator-declared verifier scope, checking a real tool effect: it runs
/// `tests/check.sh`, which requires `result.txt` to carry "done".
const VERIFY: &str = "sh tests/check.sh";
/// Verifier-owned inputs declared for every fixture case.
const SCOPE: &str = "tests";

/// Scripted club used both as the campaign's selected route and as the
/// reflector's backend (the reflector is [`crate::drive::reinforce::ClubReflector`],
/// which calls `respond`).
///
/// Attempt behavior:
/// * system prompt carries [`WORSE`] -> no work (red);
/// * system prompt carries [`NOTE`] -> always writes the artifact (green);
/// * otherwise -> alternates green/red, which is what gives a real training
///   batch an advantage spread to reflect on.
struct ObjectiveFixtureClub {
    proposal: &'static str,
    attempts: AtomicUsize,
    applied_notes: Mutex<Vec<String>>,
    /// Every reflection prompt the club was asked, so a test can see what the
    /// reflector actually received.
    reflection_prompts: Mutex<Vec<String>>,
    /// When set, the attempt rewrites the verifier's own script instead of
    /// doing the work.
    tamper_verifier: bool,
    /// When set, a reflection call blocks until the campaign is cancelled.
    block_reflection: bool,
}

impl ObjectiveFixtureClub {
    fn new(proposal: &'static str) -> Arc<Self> {
        Arc::new(Self {
            proposal,
            attempts: AtomicUsize::new(0),
            applied_notes: Mutex::new(Vec::new()),
            reflection_prompts: Mutex::new(Vec::new()),
            tamper_verifier: false,
            block_reflection: false,
        })
    }

    fn configured(
        proposal: &'static str,
        tamper_verifier: bool,
        block_reflection: bool,
    ) -> Arc<Self> {
        Arc::new(Self {
            proposal,
            attempts: AtomicUsize::new(0),
            applied_notes: Mutex::new(Vec::new()),
            reflection_prompts: Mutex::new(Vec::new()),
            tamper_verifier,
            block_reflection,
        })
    }

    fn reflections(&self) -> Vec<String> {
        self.reflection_prompts
            .lock()
            .map(|prompts| prompts.clone())
            .unwrap_or_default()
    }

    fn attempt_index(&self) -> usize {
        self.attempts.fetch_add(1, Ordering::AcqRel)
    }

    fn applied_notes(&self) -> Vec<String> {
        self.applied_notes
            .lock()
            .map(|notes| notes.clone())
            .unwrap_or_default()
    }

    fn should_work(&self, system: &str, index: usize) -> bool {
        if system.contains(WORSE) {
            return false;
        }
        if system.contains(NOTE) {
            return true;
        }
        index.is_multiple_of(2)
    }
}

impl Club for ObjectiveFixtureClub {
    fn label(&self) -> &str {
        "objective-fixture"
    }

    fn respond(&self, _prompt: &str) -> Result<String, String> {
        Ok(self.proposal.to_string())
    }

    fn chat_streaming(
        &self,
        messages: &[ChatMsg],
        _tools: &[ToolDef],
        _cancel: &AtomicBool,
        _on_delta: &mut dyn FnMut(StreamDelta),
    ) -> Result<ClubReply, String> {
        let system = messages
            .iter()
            .find(|message| message.role == ChatRole::System)
            .map(|message| message.content.clone())
            .unwrap_or_default();
        if !system.contains("[LOOP RECOVERY EXPERIMENT]") {
            // Reflection: record the prompt (including the evidence the
            // reflector was given), then answer. A blocking fixture holds the
            // request open until the campaign is cancelled.
            let prompt = messages
                .iter()
                .map(|message| message.content.to_string())
                .collect::<Vec<_>>()
                .join("\n");
            if let Ok(mut prompts) = self.reflection_prompts.lock() {
                prompts.push(prompt);
            }
            if self.block_reflection {
                let deadline = Instant::now() + Duration::from_secs(60);
                while !_cancel.load(Ordering::Acquire) && Instant::now() < deadline {
                    std::thread::sleep(Duration::from_millis(10));
                }
                if _cancel.load(Ordering::Acquire) {
                    return Err("reflection cancelled".into());
                }
            }
            return Ok(ClubReply::Text(self.proposal.to_string()));
        }
        // The first hop of an attempt carries exactly the system prompt and the
        // task: decide there whether this attempt does the work, then answer.
        if messages.len() != 2 {
            return Ok(ClubReply::Text("finished".to_string()));
        }
        let index = self.attempt_index();
        let note = system
            .lines()
            .skip_while(|line| !line.starts_with("[applied policy note"))
            .nth(1)
            .unwrap_or("")
            .to_string();
        if !note.is_empty()
            && let Ok(mut notes) = self.applied_notes.lock()
        {
            notes.push(note);
        }
        if self.tamper_verifier {
            return Ok(ClubReply::Calls(vec![crate::agent::club::ToolCall {
                id: format!("tamper-{index}"),
                name: "write_file".into(),
                args: json!({"path":"tests/check.sh","content":"#!/bin/sh\nexit 0\n"}),
            }]));
        }
        if self.should_work(&system, index) {
            return Ok(ClubReply::Calls(vec![crate::agent::club::ToolCall {
                id: format!("objective-{index}"),
                name: "write_file".into(),
                args: json!({"path":"result.txt","content":"done"}),
            }]));
        }
        Ok(ClubReply::Text(format!("attempt {index} finished")))
    }
}

/// A club that never performs the work: the objective stays red for every
/// policy, so no training batch can produce an advantage spread.
struct RedFixtureClub;

impl Club for RedFixtureClub {
    fn label(&self) -> &str {
        "objective-red-fixture"
    }
    fn respond(&self, _prompt: &str) -> Result<String, String> {
        Ok("no policy change proposed".to_string())
    }
    fn chat_streaming(
        &self,
        _messages: &[ChatMsg],
        _tools: &[ToolDef],
        _cancel: &AtomicBool,
        _on_delta: &mut dyn FnMut(StreamDelta),
    ) -> Result<ClubReply, String> {
        Ok(ClubReply::Text("nothing to change".to_string()))
    }
}

/// A club that blocks inside its first attempt until the campaign is cancelled,
/// so `/rl stop` has real in-flight work to stop.
struct BlockingFixtureClub {
    entered: Mutex<Option<mpsc::Sender<()>>>,
}

impl Club for BlockingFixtureClub {
    fn label(&self) -> &str {
        "objective-blocking-fixture"
    }
    fn respond(&self, _prompt: &str) -> Result<String, String> {
        Ok("blocked fixture proposal".to_string())
    }
    fn chat_streaming(
        &self,
        _messages: &[ChatMsg],
        _tools: &[ToolDef],
        cancel: &AtomicBool,
        _on_delta: &mut dyn FnMut(StreamDelta),
    ) -> Result<ClubReply, String> {
        if let Some(sender) = self.entered.lock().ok().and_then(|mut slot| slot.take()) {
            let _ = sender.send(());
        }
        let deadline = Instant::now() + Duration::from_secs(60);
        while !cancel.load(Ordering::Acquire) && Instant::now() < deadline {
            std::thread::sleep(Duration::from_millis(10));
        }
        Err("cancelled blocking fixture".to_string())
    }
}

struct FixtureEnv {
    _authority: TestEnvGuard,
    _harness: TestEnvGuard,
    _rl_dir: TestEnvGuard,
    _authority_dir: TestEnvGuard,
    root: PathBuf,
    // Struct fields drop in declaration order: restore the environment before
    // releasing the shared lock to another test.
    _lock: std::sync::MutexGuard<'static, ()>,
}

impl FixtureEnv {
    fn new(tag: &str) -> Self {
        let lock = env_lock();
        let root = std::env::temp_dir().join(format!(
            "angel-rl-campaign-{tag}-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|duration| duration.as_nanos())
                .unwrap_or(0)
        ));
        std::fs::create_dir_all(&root).expect("create campaign fixture root");
        let harness = root.join("continual-harness");
        let runs = root.join("rl-runs");
        let authority = root.join("authority");
        for dir in [&harness, &runs, &authority] {
            std::fs::create_dir_all(dir).expect("create fixture store");
        }
        Self {
            _lock: lock,
            _authority: TestEnvGuard::unset("ANGEL_CODING_TRAINING_AUTHORITY_DIR"),
            _harness: TestEnvGuard::set(
                "ANGEL_CONTINUAL_HARNESS_DIR",
                harness.to_str().expect("utf-8 harness dir"),
            ),
            _rl_dir: TestEnvGuard::set("ANGEL_RL_DIR", runs.to_str().expect("utf-8 run dir")),
            _authority_dir: TestEnvGuard::set(
                "ANGEL_REINFORCE_AUTHORITY_DIR",
                authority.to_str().expect("utf-8 authority dir"),
            ),
            root,
        }
    }

    fn workspace(&self) -> TestGitWorkspace {
        let workspace = TestGitWorkspace::new("rl-campaign");
        seed_workspace(&workspace, "source.txt", "operator source\n");
        workspace
    }
}

/// Seed a workspace with an operator-owned verifier script in the declared
/// scope plus a source file the candidate is free to edit.
fn seed_workspace(workspace: &TestGitWorkspace, source: &str, body: &str) {
    let root = workspace.path();
    std::fs::create_dir_all(root.join("tests")).expect("create verifier scope");
    std::fs::write(root.join(source), body).expect("seed operator source");
    std::fs::write(
        root.join("tests/check.sh"),
        "#!/bin/sh\ntest \"$(cat result.txt)\" = done\n",
    )
    .expect("seed selection verifier");
    std::fs::write(
        root.join("tests/audit.sh"),
        "#!/bin/sh\ngrep -qx done result.txt\n",
    )
    .expect("seed audit verifier");
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        for script in ["tests/check.sh", "tests/audit.sh"] {
            std::fs::set_permissions(root.join(script), std::fs::Permissions::from_mode(0o755))
                .expect("mark verifier executable");
        }
    }
}

/// An independently authored audit objective over its own source scope. The
/// verifier differs from the selection verifier on purpose: the release gate
/// refuses an audit that shares a manifest, fixture, verifier or inventory
/// identity with the selection cohort, so a relabelled repeat of the same
/// objective can never be presented as independent generalization.
fn audit_args(audit_source: &Path) -> Vec<String> {
    vec![
        "--audit".to_string(),
        format!(
            "independent review objective :: sh tests/audit.sh :: {}",
            audit_source.display()
        ),
    ]
}

fn audit_workspace() -> TestGitWorkspace {
    let workspace = TestGitWorkspace::new("rl-campaign-audit");
    seed_workspace(
        &workspace,
        "reviewed.txt",
        "independently authored review source\n",
    );
    workspace
}

fn wait_until(mut condition: impl FnMut() -> bool, timeout: Duration) -> bool {
    let deadline = Instant::now() + timeout;
    while Instant::now() < deadline {
        if condition() {
            return true;
        }
        std::thread::sleep(Duration::from_millis(20));
    }
    condition()
}

fn args(extra: &[&str]) -> Vec<String> {
    let mut args: Vec<String> = vec![
        "--rounds".into(),
        "1".into(),
        "--group".into(),
        "4".into(),
        // Two samples can never clear the confidence bound on a half-red
        // incumbent; the operator's sample count is what makes the gate decisive.
        "--samples".into(),
        "4".into(),
        "--task".into(),
        "write the artifact this objective asks for".into(),
        "--verify".into(),
        VERIFY.into(),
        "--verify-scope".into(),
        SCOPE.into(),
    ];
    args.extend(extra.iter().map(|value| value.to_string()));
    args
}

fn run_to_completion(state: &RlState, timeout: Duration) -> Result<CampaignOutcome, String> {
    assert!(
        wait_until(|| !state.running(), timeout),
        "campaign did not finish within {timeout:?}"
    );
    let progress = state.progress_snapshot();
    let log: Vec<String> = progress.log_tail.iter().cloned().collect();
    match progress.outcome {
        Some(outcome) => outcome.map_err(|error| format!("{error}\nlog: {log:?}")),
        None => panic!("a finished campaign records its outcome; log: {log:?}"),
    }
}

#[test]
fn audited_campaign_releases_a_measured_note_that_later_work_consumes() {
    let env = FixtureEnv::new("release");
    let workspace = env.workspace();
    let audit = audit_workspace();
    let club = ObjectiveFixtureClub::new(NOTE);
    let mut state = RlState::default();

    let mut campaign_args = args(&[]);
    campaign_args.extend(audit_args(audit.path()));
    state
        .start_campaign(workspace.path(), club.clone(), &campaign_args)
        .expect("campaign launches on the selected club");
    assert_eq!(state.mode, RlMode::Campaign);

    let outcome = run_to_completion(&state, Duration::from_secs(600))
        .expect("an audited improvement releases a policy");
    eprintln!("attempt log: {:?}", state.progress_snapshot().log_tail);
    assert!(outcome.validated, "audit approved the release: {outcome:?}");
    assert_eq!(outcome.decision, "promoted", "{outcome:?}");
    assert!(outcome.attempted >= 6, "real attempts ran: {outcome:?}");
    assert!(outcome.passed >= 1, "{outcome:?}");
    assert!(outcome.release_sha256.is_some(), "{outcome:?}");
    assert_eq!(
        outcome.accepted_entry.as_deref(),
        Some(RL_POLICY_ENTRY_ID),
        "the released note lands in the continual harness"
    );
    let event = outcome
        .accepted_event
        .clone()
        .expect("the release is recorded as a refinement event");

    // The candidate attempts really ran with the note applied: only those
    // attempts write the artifact the verifier reads.
    let notes = club.applied_notes();
    assert!(
        notes.iter().any(|note| note.contains(NOTE)),
        "the policy note must reach the attempt's system prompt: {notes:?}"
    );

    // Durable, workspace-scoped consumption: the same block ordinary
    // interactive and headless turns inject now carries the learned note.
    let block = crate::drive::continual_harness::context_block(workspace.path());
    assert!(
        block.contains(NOTE),
        "later work consumes the note: {block}"
    );
    assert_eq!(current_policy_note(workspace.path()).as_deref(), Some(NOTE));

    // Real work and its artifacts survive in the run record.
    let run_dir = state.run_dir().expect("run dir").to_path_buf();
    assert!(run_dir.join("report.json").is_file());
    let attempts = std::fs::read_dir(run_dir.join("attempts"))
        .expect("attempts recorded")
        .filter_map(|entry| entry.ok())
        .filter(|entry| entry.path().join("candidate.patch").is_file())
        .count();
    assert!(attempts >= 4, "each attempt left real work: {attempts}");

    // Rollback through the established continual-harness API.
    crate::drive::continual_harness::rollback(workspace.path(), Scope::Project, &event)
        .expect("the release event rolls back");
    assert!(current_policy_note(workspace.path()).is_none());
    assert!(!crate::drive::continual_harness::context_block(workspace.path()).contains(NOTE));
    let _ = std::fs::remove_dir_all(&env.root);
}

#[test]
fn campaign_rejects_a_proposal_that_measures_worse_than_the_incumbent() {
    let env = FixtureEnv::new("reject");
    let workspace = env.workspace();
    let audit = audit_workspace();
    let club = ObjectiveFixtureClub::new(WORSE);
    let mut state = RlState::default();

    let mut campaign_args = args(&[]);
    campaign_args.extend(audit_args(audit.path()));
    state
        .start_campaign(workspace.path(), club.clone(), &campaign_args)
        .expect("campaign launches");
    let outcome =
        run_to_completion(&state, Duration::from_secs(600)).expect("the campaign itself completes");
    assert!(outcome.reflection, "a reflection was proposed: {outcome:?}");
    assert_eq!(outcome.promoted_rounds, 0, "{outcome:?}");
    assert!(!outcome.validated, "{outcome:?}");
    assert_ne!(outcome.decision, "promoted", "{outcome:?}");
    assert!(outcome.release_sha256.is_none());
    assert!(
        current_policy_note(workspace.path()).is_none(),
        "a rejected proposal never reaches later work"
    );
    assert!(crate::drive::continual_harness::context_block(workspace.path()).is_empty());
    let record = std::fs::read_to_string(state.run_dir().expect("run dir").join("report.json"))
        .expect("run record written");
    assert!(record.contains("\"validated\": false"), "{record}");
    let _ = std::fs::remove_dir_all(&env.root);
}

#[test]
fn exploration_without_an_audit_case_measures_but_installs_nothing() {
    let env = FixtureEnv::new("explore");
    let workspace = env.workspace();
    let club = ObjectiveFixtureClub::new(NOTE);
    let mut state = RlState::default();

    state
        .start_campaign(workspace.path(), club.clone(), &args(&[]))
        .expect("campaign launches without an audit case");
    let outcome = run_to_completion(&state, Duration::from_secs(600))
        .expect("exploration reports its measured verdict");
    assert!(!outcome.audit_supplied, "{outcome:?}");
    assert!(!outcome.validated, "nothing is installed without an audit");
    assert_eq!(outcome.decision, "promoted", "{outcome:?}");
    assert!(outcome.promoted_rounds > 0, "{outcome:?}");
    assert!(outcome.accepted_entry.is_none());
    assert!(
        current_policy_note(workspace.path()).is_none(),
        "measured exploration is not validated learning"
    );
    let record = std::fs::read_to_string(state.run_dir().expect("run dir").join("report.json"))
        .expect("run record written");
    assert!(record.contains("\"audit_supplied\": false"), "{record}");
    let _ = std::fs::remove_dir_all(&env.root);
}

#[test]
fn campaign_without_any_advantage_spread_proposes_nothing() {
    let env = FixtureEnv::new("no-signal");
    let workspace = env.workspace();
    let club: Arc<dyn Club> = Arc::new(RedFixtureClub);
    let mut state = RlState::default();

    state
        .start_campaign(workspace.path(), club, &args(&[]))
        .expect("campaign launches");
    let outcome = run_to_completion(&state, Duration::from_secs(600))
        .expect("a campaign with no signal still reports plainly");
    assert!(!outcome.reflection, "{outcome:?}");
    assert_eq!(outcome.decision, "no-reflection", "{outcome:?}");
    assert_eq!(outcome.promoted_rounds, 0);
    assert_eq!(outcome.solve_rate, Some(0.0));
    assert!(outcome.red >= 2, "every attempt measured red: {outcome:?}");
    assert!(current_policy_note(workspace.path()).is_none());
    let _ = std::fs::remove_dir_all(&env.root);
}

#[test]
fn stopping_a_campaign_cancels_work_and_installs_nothing() {
    let env = FixtureEnv::new("stop");
    let workspace = env.workspace();
    let (entered_tx, entered_rx) = mpsc::channel();
    let club: Arc<dyn Club> = Arc::new(BlockingFixtureClub {
        entered: Mutex::new(Some(entered_tx)),
    });
    let mut state = RlState::default();

    state
        .start_campaign(workspace.path(), club, &args(&[]))
        .expect("campaign launches");
    entered_rx
        .recv_timeout(Duration::from_secs(180))
        .expect("the campaign reached a real in-flight attempt");
    assert!(state.stop(), "stop reaches the running campaign");
    assert!(
        wait_until(
            || matches!(state.progress_snapshot().outcome, Some(Err(_))),
            Duration::from_secs(180)
        ),
        "the stopped campaign settles"
    );
    match state.progress_snapshot().outcome {
        Some(Err(error)) => assert!(error.contains("stopped by operator"), "{error}"),
        other => panic!("expected the operator stop to be recorded, got {other:?}"),
    }
    assert!(current_policy_note(workspace.path()).is_none());
    assert!(!state.stop(), "a settled campaign has nothing left to stop");
    let _ = std::fs::remove_dir_all(&env.root);
}

#[test]
fn reflection_receives_the_attempt_work_and_the_physical_verdict() {
    let env = FixtureEnv::new("reflection-evidence");
    let workspace = env.workspace();
    let club = ObjectiveFixtureClub::new(NOTE);
    let mut state = RlState::default();

    state
        .start_campaign(workspace.path(), club.clone(), &args(&[]))
        .expect("campaign launches");
    let outcome =
        run_to_completion(&state, Duration::from_secs(600)).expect("the campaign completes");
    assert!(outcome.reflection, "{outcome:?}");

    let reflections = club.reflections();
    assert_eq!(reflections.len(), 1, "one reflection ran: {reflections:?}");
    let prompt = &reflections[0];
    // The reflector sees what the attempts did, not just where they live.
    assert!(prompt.contains("patch ·"), "{prompt}");
    assert!(prompt.contains("patch excerpt"), "{prompt}");
    assert!(prompt.contains("tools ·"), "{prompt}");
    assert!(prompt.contains("work · stop answer"), "{prompt}");
    // And the physical verdict the evaluator measured for each arm, so the
    // successful and failed work are distinguishable by evidence rather than by
    // the receipt alone.
    assert!(
        prompt.contains("physical verifier: verifier exit 0"),
        "{prompt}"
    );
    assert!(
        prompt.contains("physical verifier: verifier exit 1"),
        "the failed attempt's verdict is visible too: {prompt}"
    );
    assert!(
        prompt.contains("HIGH-scoring") && prompt.contains("LOW-scoring"),
        "{prompt}"
    );
    let _ = std::fs::remove_dir_all(&env.root);
}

#[test]
fn a_candidate_that_rewrites_the_verifier_never_releases() {
    let env = FixtureEnv::new("verifier-scope");
    let workspace = env.workspace();
    let audit = audit_workspace();
    let club = ObjectiveFixtureClub::configured(NOTE, true, false);
    let mut state = RlState::default();

    let mut campaign_args = args(&[]);
    campaign_args.extend(audit_args(audit.path()));
    state
        .start_campaign(workspace.path(), club.clone(), &campaign_args)
        .expect("campaign launches");
    let outcome = run_to_completion(&state, Duration::from_secs(600))
        .expect("the campaign completes with a verdict");
    assert!(!outcome.validated, "{outcome:?}");
    assert!(outcome.release_sha256.is_none(), "{outcome:?}");
    assert!(current_policy_note(workspace.path()).is_none());
    let _ = std::fs::remove_dir_all(&env.root);
}

#[test]
fn stopping_during_reflection_cancels_the_in_flight_request() {
    let env = FixtureEnv::new("stop-reflection");
    let workspace = env.workspace();
    let club = ObjectiveFixtureClub::configured(NOTE, false, true);
    let mut state = RlState::default();

    state
        .start_campaign(workspace.path(), club.clone(), &args(&[]))
        .expect("campaign launches");
    assert!(
        wait_until(|| !club.reflections().is_empty(), Duration::from_secs(240)),
        "the campaign reached in-flight reflection"
    );
    assert!(state.stop(), "stop reaches the campaign");
    assert!(
        wait_until(
            || matches!(state.progress_snapshot().outcome, Some(Err(_))),
            Duration::from_secs(120)
        ),
        "the stopped campaign settles"
    );
    match state.progress_snapshot().outcome {
        Some(Err(error)) => assert!(error.contains("stopped by operator"), "{error}"),
        other => panic!("expected an operator stop, got {other:?}"),
    }
    assert!(current_policy_note(workspace.path()).is_none());
    let _ = std::fs::remove_dir_all(&env.root);
}

#[test]
fn stopping_during_the_verifier_kills_the_running_process() {
    let env = FixtureEnv::new("stop-verifier");
    let workspace = env.workspace();
    // A verifier that announces itself and then hangs: the campaign must be able
    // to stop it rather than wait out a hidden deadline.
    std::fs::write(
        workspace.path().join("tests/check.sh"),
        "#!/bin/sh\ntouch verifier-entered\nsleep 120\n",
    )
    .expect("seed hanging verifier");
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(
            workspace.path().join("tests/check.sh"),
            std::fs::Permissions::from_mode(0o755),
        )
        .expect("mark verifier executable");
    }
    let club = ObjectiveFixtureClub::new(NOTE);
    let mut state = RlState::default();

    state
        .start_campaign(workspace.path(), club.clone(), &args(&[]))
        .expect("campaign launches");
    let run_dir = state.run_dir().expect("run dir").to_path_buf();
    assert!(
        wait_until(
            || {
                std::fs::read_dir(run_dir.join("attempts"))
                    .map(|entries| {
                        entries
                            .filter_map(|entry| entry.ok())
                            .any(|entry| entry.path().join("working/verifier-entered").is_file())
                    })
                    .unwrap_or(false)
            },
            Duration::from_secs(240)
        ),
        "a verifier process actually started"
    );
    let stopped_at = Instant::now();
    assert!(state.stop(), "stop reaches the running verifier");
    assert!(
        wait_until(
            || matches!(state.progress_snapshot().outcome, Some(Err(_))),
            Duration::from_secs(60)
        ),
        "the stopped campaign settles promptly"
    );
    assert!(
        stopped_at.elapsed() < Duration::from_secs(60),
        "the hung verifier was cancelled, not waited out"
    );
    match state.progress_snapshot().outcome {
        Some(Err(error)) => assert!(error.contains("stopped by operator"), "{error}"),
        other => panic!("expected an operator stop, got {other:?}"),
    }
    assert!(current_policy_note(workspace.path()).is_none());
    let _ = std::fs::remove_dir_all(&env.root);
}

/// The advertised command line must survive the real entry point: this drives
/// `App::submit` with the exact quoted `/rl run …` string an operator types and
/// checks the launched campaign carries the objective, verifier command, audit
/// case and scope verbatim. The club is an in-process fixture, the workspace is
/// a private temp tree, and no provider request is made.
#[test]
fn app_entry_launches_the_advertised_quoted_command_intact() {
    let env = FixtureEnv::new("ui-entry");
    let workspace = env.workspace();
    // A real (private, disposable) Git source whose path contains spaces: the
    // frozen-source walker needs a repository, and quoting must preserve the
    // spaces all the way to the audit case.
    let audit_source = TestGitWorkspace::new("audit source with spaces");

    let club = ObjectiveFixtureClub::new(NOTE);
    let mut app = crate::seed_preview_app();
    app.tools = Arc::new(crate::agent::harness::ToolRegistry::with_team(
        workspace.path().to_path_buf(),
        Vec::new(),
    ));
    app.bag =
        crate::agent::club::Bag::for_render_test(&[("fixture", &[("objective-fixture", true)])]);
    app.bag.replace_in_hand_club_for_test(club.clone());

    // The task carries nested quotes and shell metacharacters; the verifier and
    // the audit source are multiword too. Nothing here may be rewritten,
    // expanded or split.
    let task = "keep \"nested\" quotes, $HOME, ; | > # literal";
    let quoted_task = task.replace('\\', "\\\\").replace('"', "\\\"");
    // A multiword verifier that announces itself and waits: it proves quoting is
    // preserved and keeps the campaign live until the operator stops it.
    let verify = "sh -c 'touch verifier-entered; sleep 120'";
    let audit_verify = "python -m unittest discover";
    app.input = format!(
        "/rl run --task \"{quoted_task}\" --verify \"{verify}\" --verify-scope tests \
         --audit \"independent audit :: {audit_verify} :: {}\"",
        audit_source.path().display()
    );
    app.cursor = app.input.chars().count();
    app.submit();

    let objective = app
        .tools
        .rl()
        .progress_snapshot()
        .objective
        .expect("a launched campaign records the objective it was given");
    assert_eq!(objective.cases.len(), 1);
    assert_eq!(objective.cases[0].id, "objective");
    assert_eq!(
        objective.cases[0].task, task,
        "the quoted task arrives intact"
    );
    assert_eq!(objective.cases[0].verify, verify);
    assert_eq!(objective.audit.len(), 1);
    assert_eq!(objective.audit[0].task, "independent audit");
    assert_eq!(objective.audit[0].verify, audit_verify);
    assert_eq!(
        objective.audit[0].source.as_deref(),
        Some(audit_source.path()),
        "an audit source path with spaces survives quoting"
    );
    assert_eq!(objective.verifier_scope, vec!["tests".to_string()]);
    assert!(
        app.tools.rl().running(),
        "the campaign is live after submit"
    );

    // Wait for a verifier to actually start, then stop: the message must claim
    // only what stop did, not what the campaign will conclude.
    let run_dir = app.tools.rl().run_dir().expect("run dir").to_path_buf();
    assert!(
        wait_until(
            || {
                std::fs::read_dir(run_dir.join("attempts"))
                    .map(|entries| {
                        entries
                            .filter_map(|entry| entry.ok())
                            .any(|entry| entry.path().join("working/verifier-entered").is_file())
                    })
                    .unwrap_or(false)
            },
            Duration::from_secs(240)
        ),
        "a verifier process started"
    );
    app.input = "/rl stop".to_string();
    app.cursor = app.input.chars().count();
    app.submit();
    let stopped = app
        .messages
        .last()
        .map(|message| message.text.to_string())
        .unwrap_or_default();
    assert!(stopped.contains("stop requested"), "{stopped}");
    assert!(
        !stopped.contains("installed") && !stopped.contains("stopped —"),
        "stop must not claim an outcome it does not know yet: {stopped}"
    );
    assert!(
        wait_until(
            || matches!(app.tools.rl().progress_snapshot().outcome, Some(Err(_))),
            Duration::from_secs(120)
        ),
        "the stopped campaign settles"
    );

    // A malformed quote is actionable and launches nothing.
    let settled_objective = app.tools.rl().progress_snapshot().objective.clone();
    app.input = "/rl run --task \"unterminated objective".to_string();
    app.cursor = app.input.chars().count();
    app.submit();
    let rejected = app
        .messages
        .last()
        .map(|message| message.text.to_string())
        .unwrap_or_default();
    assert!(rejected.contains("unterminated double quote"), "{rejected}");
    assert!(
        !app.tools.rl().running(),
        "a malformed quote starts no campaign"
    );
    assert_eq!(
        app.tools.rl().progress_snapshot().objective,
        settled_objective,
        "the rejected line does not replace the recorded objective"
    );
    let _ = std::fs::remove_dir_all(&env.root);
}
