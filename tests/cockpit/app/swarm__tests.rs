use super::*;
use crate::club::{CacheUsage, EffortGateUsage, ToolCall};
use std::collections::VecDeque;
use std::ffi::OsString;
use std::sync::Mutex;

struct EnvGuard {
    key: &'static str,
    old: Option<OsString>,
}

impl EnvGuard {
    fn set(key: &'static str, value: &str) -> Self {
        let old = std::env::var_os(key);
        // TODO: Audit that the environment access only happens in single-threaded code.
        unsafe { std::env::set_var(key, value) };
        Self { key, old }
    }

    fn unset(key: &'static str) -> Self {
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

/// Records how many chat calls it served and answers deterministically so we
/// can assert the exact pipeline shape. The classifier prompt is recognized by
/// its leading word so the gate can be steered per test.
struct CountingClub {
    calls: Mutex<usize>,
    classify_as: &'static str,
}

impl CountingClub {
    fn new(classify_as: &'static str) -> Self {
        Self {
            calls: Mutex::new(0),
            classify_as,
        }
    }
    fn count(&self) -> usize {
        *self.calls.lock().unwrap()
    }
}

impl Club for CountingClub {
    fn label(&self) -> &str {
        "counting"
    }
    fn respond(&self, p: &str) -> Result<String, String> {
        Ok(format!("echo:{p}"))
    }
    fn chat(&self, messages: &[ChatMsg], _tools: &[ToolDef]) -> Result<ClubReply, String> {
        *self.calls.lock().unwrap() += 1;
        let sys = messages
            .iter()
            .find(|m| m.role == ChatRole::System)
            .map(|m| m.content.as_ref())
            .unwrap_or("");
        if sys.starts_with("Classify") {
            return Ok(ClubReply::Text(self.classify_as.to_string()));
        }
        Ok(ClubReply::Text("draft".to_string()))
    }
}

/// Like [`CountingClub`] but every reply is a *distinct* multi-word draft
/// (digit-free, so judge/chooser parses on it still fail cleanly), so the
/// local near-duplicate collapse never fires and tests can assert the full
/// ladder's call shape.
struct DistinctClub {
    calls: Mutex<usize>,
}

impl DistinctClub {
    fn new() -> Self {
        Self {
            calls: Mutex::new(0),
        }
    }
    fn count(&self) -> usize {
        *self.calls.lock().unwrap()
    }
}

impl Club for DistinctClub {
    fn label(&self) -> &str {
        "distinct"
    }
    fn respond(&self, p: &str) -> Result<String, String> {
        Ok(format!("echo:{p}"))
    }
    fn chat(&self, _messages: &[ChatMsg], _tools: &[ToolDef]) -> Result<ClubReply, String> {
        let mut calls = self.calls.lock().unwrap();
        *calls += 1;
        // Every 3-word shingle contains the per-call tag, so drafts from
        // different calls share zero shingles — never near-identical.
        let tag = "z".repeat(*calls);
        Ok(ClubReply::Text(format!(
            "{tag} draft {tag} angle {tag} path {tag} claim {tag}"
        )))
    }
}

struct ScriptedClub {
    label: &'static str,
    calls: Mutex<usize>,
    replies: Mutex<VecDeque<Result<String, String>>>,
}

impl ScriptedClub {
    fn new(label: &'static str, replies: Vec<Result<&'static str, &'static str>>) -> Self {
        Self {
            label,
            calls: Mutex::new(0),
            replies: Mutex::new(
                replies
                    .into_iter()
                    .map(|r| r.map(str::to_string).map_err(str::to_string))
                    .collect(),
            ),
        }
    }

    fn count(&self) -> usize {
        *self.calls.lock().unwrap()
    }
}

impl Club for ScriptedClub {
    fn label(&self) -> &str {
        self.label
    }

    fn respond(&self, p: &str) -> Result<String, String> {
        Ok(format!("echo:{p}"))
    }

    fn chat(&self, _messages: &[ChatMsg], _tools: &[ToolDef]) -> Result<ClubReply, String> {
        *self.calls.lock().unwrap() += 1;
        match self
            .replies
            .lock()
            .unwrap()
            .pop_front()
            .unwrap_or_else(|| Ok("draft".to_string()))
        {
            Ok(text) => Ok(ClubReply::Text(text)),
            Err(err) => Err(err),
        }
    }
}

struct ToolCallingClub {
    label: &'static str,
    calls: Mutex<usize>,
}

struct PlanningToolClub {
    text_calls: Mutex<usize>,
    tool_calls: Mutex<usize>,
    tool_history: Mutex<Vec<ChatMsg>>,
}

struct PlanningThenAnswerClub {
    text_calls: Mutex<usize>,
    tool_calls: Mutex<usize>,
}

impl PlanningThenAnswerClub {
    fn new() -> Self {
        Self {
            text_calls: Mutex::new(0),
            tool_calls: Mutex::new(0),
        }
    }
}

impl Club for PlanningThenAnswerClub {
    fn label(&self) -> &str {
        "glm-5.3-flash"
    }

    fn respond(&self, prompt: &str) -> Result<String, String> {
        Ok(format!("plan:{prompt}"))
    }

    fn chat(&self, messages: &[ChatMsg], tools: &[ToolDef]) -> Result<ClubReply, String> {
        if tools.is_empty() {
            *self.text_calls.lock().unwrap() += 1;
            return Ok(ClubReply::Text(
                "Inspect the contract, compare one bounded candidate, and verify the edit."
                    .to_string(),
            ));
        }
        *self.tool_calls.lock().unwrap() += 1;
        if messages
            .iter()
            .any(|message| message.role == ChatRole::Tool)
        {
            return Ok(ClubReply::Text(
                "The targeted checks passed and the workspace contains the verified candidate. "
                    .repeat(6),
            ));
        }
        Ok(ClubReply::Calls(vec![ToolCall {
            id: "preflight-only-tool-call".to_string(),
            name: "shell".to_string(),
            args: serde_json::json!({ "command": "pwd" }),
        }]))
    }
}

impl PlanningToolClub {
    fn new() -> Self {
        Self {
            text_calls: Mutex::new(0),
            tool_calls: Mutex::new(0),
            tool_history: Mutex::new(Vec::new()),
        }
    }
}

impl Club for PlanningToolClub {
    fn label(&self) -> &str {
        "glm-5.3-flash"
    }

    fn respond(&self, prompt: &str) -> Result<String, String> {
        Ok(format!("plan:{prompt}"))
    }

    fn chat(&self, messages: &[ChatMsg], tools: &[ToolDef]) -> Result<ClubReply, String> {
        if tools.is_empty() {
            *self.text_calls.lock().unwrap() += 1;
            return Ok(ClubReply::Text(
                "Inspect the benchmark contract, establish a baseline, then test one bounded hypothesis."
                    .to_string(),
            ));
        }
        *self.tool_calls.lock().unwrap() += 1;
        *self.tool_history.lock().unwrap() = messages.to_vec();
        Ok(ClubReply::Calls(vec![ToolCall {
            id: "preflight-tool-call".to_string(),
            name: "shell".to_string(),
            args: serde_json::json!({ "command": "pwd" }),
        }]))
    }
}

impl ToolCallingClub {
    fn new(label: &'static str) -> Self {
        Self {
            label,
            calls: Mutex::new(0),
        }
    }

    fn count(&self) -> usize {
        *self.calls.lock().unwrap()
    }
}

impl Club for ToolCallingClub {
    fn label(&self) -> &str {
        self.label
    }

    fn respond(&self, p: &str) -> Result<String, String> {
        Ok(format!("echo:{p}"))
    }

    fn chat(&self, _messages: &[ChatMsg], _tools: &[ToolDef]) -> Result<ClubReply, String> {
        *self.calls.lock().unwrap() += 1;
        Ok(ClubReply::Calls(vec![ToolCall {
            id: "bad-tool-call".to_string(),
            name: "shell".to_string(),
            args: serde_json::json!({ "cmd": "pwd" }),
        }]))
    }
}

struct TranscriptRecordingClub {
    label: &'static str,
    seen: Mutex<Vec<Vec<ChatMsg>>>,
}

impl TranscriptRecordingClub {
    fn new(label: &'static str) -> Self {
        Self {
            label,
            seen: Mutex::new(Vec::new()),
        }
    }

    fn count(&self) -> usize {
        self.seen.lock().unwrap().len()
    }

    fn last_messages(&self) -> Vec<ChatMsg> {
        self.seen
            .lock()
            .unwrap()
            .last()
            .cloned()
            .unwrap_or_default()
    }
}

impl Club for TranscriptRecordingClub {
    fn label(&self) -> &str {
        self.label
    }

    fn respond(&self, p: &str) -> Result<String, String> {
        Ok(format!("echo:{p}"))
    }

    fn chat(&self, messages: &[ChatMsg], tools: &[ToolDef]) -> Result<ClubReply, String> {
        assert!(tools.is_empty(), "swarm workers must be text-only");
        self.seen.lock().unwrap().push(messages.to_vec());
        Ok(ClubReply::Text("draft".to_string()))
    }
}

struct ResearchScoutClub {
    calls: Mutex<Vec<String>>,
    reply: &'static str,
}

impl ResearchScoutClub {
    fn new(reply: &'static str) -> Self {
        Self {
            calls: Mutex::new(Vec::new()),
            reply,
        }
    }

    fn count(&self) -> usize {
        self.calls.lock().unwrap().len()
    }

    fn last_prompt(&self) -> String {
        self.calls
            .lock()
            .unwrap()
            .last()
            .cloned()
            .unwrap_or_default()
    }
}

impl Club for ResearchScoutClub {
    fn label(&self) -> &str {
        "grok-research"
    }

    fn respond(&self, prompt: &str) -> Result<String, String> {
        self.calls.lock().unwrap().push(prompt.to_string());
        Ok(self.reply.to_string())
    }
}

fn sota_test_swarm(
    propose: Arc<CountingClub>,
    aggregate: Arc<CountingClub>,
    always: bool,
) -> SwarmClub {
    SwarmClub {
        name: "sota-moa".to_string(),
        clubs: RoleClubs {
            propose,
            propose_extra: Vec::new(),
            judge: aggregate.clone(),
            judge_extra: Vec::new(),
            verify: aggregate.clone(),
            verify_extra: Vec::new(),
            aggregate,
            aggregate_extra: Vec::new(),
            research: None,
        },
        k: Knobs {
            width: 2,
            layers: 1,
            always,
            reflect: false,
            judge: false,
            judge_panel: 1,
            dims: false,
            keep: 0,
            samples: 1,
            verify: 0,
            verify_guard: false,
            research: false,
            cite: false,
            hedge: false,
            search_url: default_search_url(),
            delegate: false,
            max_tests: 2,
            dissent_gate: false,
            ..Knobs::default()
        },
        fallbacks: Vec::new(),
        tool_preflight: Default::default(),
    }
}

#[derive(Default)]
struct UsageClub {
    label: &'static str,
    usage: TokenUsage,
    effort_gate: EffortGateUsage,
    cache: CacheUsage,
}

impl Club for UsageClub {
    fn label(&self) -> &str {
        self.label
    }
    fn respond(&self, p: &str) -> Result<String, String> {
        Ok(p.to_string())
    }
    fn token_usage(&self) -> Option<TokenUsage> {
        Some(self.usage)
    }
    fn effort_gate_usage(&self) -> EffortGateUsage {
        self.effort_gate.clone()
    }
    fn cache_usage(&self) -> CacheUsage {
        self.cache
    }
}

#[test]
fn swarm_opts_into_palace_reports() {
    let inner = Arc::new(CountingClub::new("OPEN"));
    // The swarm's synthesis is report-worthy; a plain club's turn is not.
    assert!(SwarmClub::with_config("swarm", inner.clone(), 2, 1, false).reports_to_palace());
    assert!(!CountingClub::new("OPEN").reports_to_palace());
}

#[test]
fn swarm_token_usage_sums_unique_role_clubs() {
    let a: Arc<dyn Club> = Arc::new(UsageClub {
        label: "deepseek-v4-pro",
        usage: TokenUsage {
            turns: 2,
            last_input: 10,
            last_output: 4,
            last_reasoning: 1,
            total_input: 20,
            total_output: 8,
            total_reasoning: 2,
        },
        ..Default::default()
    });
    let b: Arc<dyn Club> = Arc::new(UsageClub {
        label: "openai",
        usage: TokenUsage {
            turns: 1,
            last_input: 7,
            last_output: 3,
            last_reasoning: 0,
            total_input: 7,
            total_output: 3,
            total_reasoning: 0,
        },
        ..Default::default()
    });
    let swarm = SwarmClub::from_env_roles(
        "sota-moa",
        Arc::clone(&a),
        Arc::clone(&b),
        Arc::clone(&a),
        Arc::clone(&b),
    );
    assert_eq!(
        swarm.token_usage(),
        Some(TokenUsage {
            turns: 3,
            last_input: 17,
            last_output: 7,
            last_reasoning: 1,
            total_input: 27,
            total_output: 11,
            total_reasoning: 2,
        })
    );
}

#[test]
fn swarm_token_usage_does_not_multiply_shared_fallback_chains() {
    let longcat: Arc<dyn Club> = Arc::new(UsageClub {
        label: "longcat",
        usage: TokenUsage {
            turns: 1,
            total_input: 100,
            total_output: 20,
            ..TokenUsage::default()
        },
        ..Default::default()
    });
    let gemma: Arc<dyn Club> = Arc::new(UsageClub {
        label: "gemma",
        usage: TokenUsage {
            turns: 1,
            total_input: 80,
            total_output: 10,
            ..TokenUsage::default()
        },
        ..Default::default()
    });
    let longcat_role: Arc<dyn Club> = Arc::new(crate::club::FallbackClub::new(vec![
        Arc::clone(&longcat),
        Arc::clone(&gemma),
    ]));
    let gemma_role: Arc<dyn Club> = Arc::new(crate::club::FallbackClub::new(vec![
        Arc::clone(&gemma),
        Arc::clone(&longcat),
    ]));
    let swarm = SwarmClub::from_env_roles(
        "sota-moa",
        Arc::clone(&longcat_role),
        Arc::clone(&gemma_role),
        Arc::clone(&longcat_role),
        gemma_role,
    )
    .with_quota_fallbacks(vec![longcat, gemma]);

    let usage = swarm.token_usage().expect("raw provider usage");
    assert_eq!(usage.turns, 2);
    assert_eq!(usage.total_input, 180);
    assert_eq!(usage.total_output, 30);
}

#[test]
fn swarm_effort_gate_usage_speaks_for_the_whole_roster() {
    let quiet: Arc<dyn Club> = Arc::new(UsageClub {
        label: "quiet-seat",
        ..Default::default()
    });
    // A seat whose requested effort was withheld/rejected deep inside a stage
    // fan-out: the swarm is the driver club the turn loop deltas, so the
    // seat's gate events must surface through the roster sum.
    let gated: Arc<dyn Club> = Arc::new(UsageClub {
        label: "gated-seat",
        effort_gate: EffortGateUsage {
            withheld: 2,
            rejections: 1,
            last: Some("gated-seat: effort 'high' withheld — the backend rejected it".into()),
        },
        ..Default::default()
    });
    let swarm = SwarmClub::from_env_roles(
        "sota-moa",
        Arc::clone(&quiet),
        Arc::clone(&gated),
        Arc::clone(&quiet),
        Arc::clone(&quiet),
    );
    let gate = swarm.effort_gate_usage();
    assert_eq!(gate.withheld, 2);
    assert_eq!(gate.rejections, 1);
    assert!(
        gate.last
            .expect("roster reason surfaces")
            .contains("gated-seat")
    );
}

#[test]
fn swarm_cache_usage_sums_unique_role_clubs() {
    let a: Arc<dyn Club> = Arc::new(UsageClub {
        label: "seat-a",
        cache: CacheUsage {
            control_requests: 1,
            read_input_tokens: 100,
            write_input_tokens: 40,
            read_accounting_responses: 2,
            write_accounting_responses: 1,
        },
        ..Default::default()
    });
    let b: Arc<dyn Club> = Arc::new(UsageClub {
        label: "seat-b",
        cache: CacheUsage {
            control_requests: 3,
            read_input_tokens: 10,
            write_input_tokens: 4,
            read_accounting_responses: 1,
            write_accounting_responses: 2,
        },
        ..Default::default()
    });
    // Duplicate wrapper seats must not double-count the same provider link.
    let swarm = SwarmClub::from_env_roles(
        "sota-moa",
        Arc::clone(&a),
        Arc::clone(&b),
        Arc::clone(&a),
        Arc::clone(&b),
    );
    assert_eq!(
        swarm.cache_usage(),
        CacheUsage {
            control_requests: 4,
            read_input_tokens: 110,
            write_input_tokens: 44,
            read_accounting_responses: 3,
            write_accounting_responses: 3,
        }
    );
}

#[test]
fn swarm_streaming_emits_progress_breadcrumbs() {
    let inner = Arc::new(CountingClub::new("OPEN"));
    let swarm = SwarmClub::with_config("swarm", inner, 2, 1, true);
    let cancel = AtomicBool::new(false);
    let mut reasoning = String::new();
    let mut content = String::new();
    let reply = swarm
        .chat_streaming(
            &[ChatMsg::user("design a robust cockpit")],
            &[],
            &cancel,
            &mut |d| match d {
                StreamDelta::Reasoning(r) => reasoning.push_str(r),
                StreamDelta::Content(c) => content.push_str(c),
                StreamDelta::Heartbeat => {}
            },
        )
        .expect("swarm streams");
    assert!(reasoning.contains("MOA engaged"), "{reasoning}");
    assert!(reasoning.contains("MOA proposers"), "{reasoning}");
    assert!(reasoning.contains("MOA synthesis"), "{reasoning}");
    assert!(!content.trim().is_empty());
    assert!(matches!(reply, ClubReply::Text(_)));
}

#[test]
fn sota_moa_forced_is_two_wave_fast_path() {
    let propose = Arc::new(CountingClub::new("OPEN"));
    let aggregate = Arc::new(CountingClub::new("OPEN"));
    let swarm = sota_test_swarm(propose.clone(), aggregate.clone(), true);
    let cancel = AtomicBool::new(false);
    let mut content = String::new();
    swarm
        .chat_streaming(
            &[ChatMsg::user("compare two implementation plans")],
            &[],
            &cancel,
            &mut |d| {
                if let StreamDelta::Content(c) = d {
                    content.push_str(c);
                }
            },
        )
        .expect("fast SOTA-MOA streams");
    assert_eq!(
        propose.count(),
        2,
        "forced SOTA-MOA does two proposer calls"
    );
    assert_eq!(
        aggregate.count(),
        1,
        "forced SOTA-MOA streams one synthesis call"
    );
    assert!(!content.trim().is_empty());
}

#[test]
fn sota_moa_round_robins_live_extra_proposer_and_skips_it_when_down() {
    let primary = Arc::new(CountingClub::new("OPEN"));
    let aggregate = Arc::new(CountingClub::new("OPEN"));
    let gemma = Arc::new(ScriptedClub::new("gemma", vec![]));
    let gemma_club: Arc<dyn Club> = gemma.clone();
    let gate = Arc::new(AtomicBool::new(true));
    let swarm = sota_test_swarm(primary.clone(), aggregate.clone(), true)
        .with_extra_proposers(vec![(gemma_club, Some(Arc::clone(&gate)))]);

    swarm
        .respond("compare two implementation plans")
        .expect("live heterogeneous proposer wave succeeds");
    assert_eq!(primary.count(), 1, "primary owns one width-2 seat");
    assert_eq!(gemma.count(), 1, "live Gemma owns the additional seat");

    gate.store(false, Ordering::Relaxed);
    swarm
        .respond("compare two more implementation plans")
        .expect("down optional proposer degrades to the primary");
    assert_eq!(
        primary.count(),
        3,
        "primary absorbs both seats when Gemma is down"
    );
    assert_eq!(gemma.count(), 1, "down Gemma is not called or failed over");
}

#[test]
fn explicit_proposer_roster_preserves_duplicate_seat_order() {
    let first = Arc::new(CountingClub::new("model-a"));
    let aggregate = Arc::new(CountingClub::new("aggregate"));
    let second: Arc<dyn Club> = Arc::new(CountingClub::new("OPEN"));
    let third: Arc<dyn Club> = Arc::new(ScriptedClub::new("model-b", vec![]));
    let swarm = sota_test_swarm(first, aggregate, true)
        .with_extra_proposers(vec![(second, None), (third, None)]);
    let labels = swarm
        .available_proposers()
        .iter()
        .map(|club| club.label().to_string())
        .collect::<Vec<_>>();
    assert_eq!(labels, vec!["counting", "counting", "model-b"]);
}

#[test]
fn swarm_text_only_calls_strip_tool_protocol_history() {
    let recorder = Arc::new(TranscriptRecordingClub::new("recorder"));
    let inner: Arc<dyn Club> = recorder.clone();
    let swarm = SwarmClub::with_config("sota-moa", inner, 2, 1, true);
    let history = vec![
        ChatMsg::user("fix the failing SOTA-MOA turn"),
        ChatMsg::assistant_calls(vec![crate::club::ToolCall {
            id: "call_dup".to_string(),
            name: "shell".to_string(),
            args: serde_json::json!({ "cmd": "date" }),
        }]),
        ChatMsg::tool("call_dup", "first result"),
        ChatMsg::tool("call_dup", "duplicate result"),
    ];

    swarm
        .call_on(&*recorder, "worker system", &history)
        .expect("text-only worker call succeeds");

    let seen = recorder.last_messages();
    assert_eq!(seen[0].role, ChatRole::System);
    assert!(
        seen.iter().all(|m| m.role != ChatRole::Tool),
        "provider payload must not contain tool-role messages"
    );
    assert!(
        seen.iter()
            .all(|m| m.tool_call_id.is_none() && m.tool_calls.is_empty()),
        "provider payload must not contain tool protocol fields"
    );
    assert!(
        seen.iter()
            .filter(|message| message.content.starts_with("[tool result for "))
            .all(|message| message.role == ChatRole::Harness),
        "flattened tool evidence must retain Harness provenance"
    );
    let joined = seen
        .iter()
        .map(|m| m.content.as_ref())
        .collect::<Vec<_>>()
        .join("\n");
    assert!(joined.contains("Assistant requested tools:"), "{joined}");
    assert!(joined.contains("[tool result for call_dup]"), "{joined}");
    assert!(joined.contains("first result"), "{joined}");
    assert!(joined.contains("duplicate result"), "{joined}");
}

#[test]
fn sota_moa_fails_closed_when_proposer_quorum_collapses() {
    // Serialized: this test mutates process-global environment that other
    // tests also read or write. Without the lock they interleave and each
    // observes the other's value between its own set and restore.
    let _guard = crate::tests::env_lock();
    let _guard = EnvGuard::unset("ANGEL_SOTA_MOA_ALLOW_DEGRADED");
    let propose = Arc::new(ScriptedClub::new(
        "propose",
        vec![
            Err("club output was cut off by finish_reason=length"),
            Ok("surviving raw proposer draft"),
        ],
    ));
    let aggregate = Arc::new(CountingClub::new("OPEN"));
    let propose_club: Arc<dyn Club> = propose.clone();
    let aggregate_club: Arc<dyn Club> = aggregate.clone();
    let swarm = SwarmClub {
        name: "sota-moa".to_string(),
        clubs: RoleClubs {
            propose: propose_club,
            propose_extra: Vec::new(),
            judge: aggregate_club.clone(),
            judge_extra: Vec::new(),
            verify: aggregate_club.clone(),
            verify_extra: Vec::new(),
            aggregate: aggregate_club,
            aggregate_extra: Vec::new(),
            research: None,
        },
        k: Knobs {
            width: 2,
            layers: 1,
            always: true,
            reflect: false,
            judge: false,
            judge_panel: 1,
            dims: false,
            keep: 0,
            samples: 1,
            verify: 0,
            verify_guard: false,
            research: false,
            cite: false,
            hedge: false,
            search_url: default_search_url(),
            delegate: false,
            max_tests: 2,
            dissent_gate: false,
            ..Knobs::default()
        },
        fallbacks: Vec::new(),
        tool_preflight: Default::default(),
    };

    let err = swarm
        .chat_streaming(
            &[ChatMsg::user("debug the early MOA breakdown")],
            &[],
            &AtomicBool::new(false),
            &mut |_| {},
        )
        .expect_err("SOTA-MOA must not silently synthesize with one proposer");

    assert!(err.contains("SOTA-MOA proposer quorum failed"), "{err}");
    assert!(err.contains("finish_reason=length"), "{err}");
    assert_eq!(propose.count(), 2);
    assert_eq!(
        aggregate.count(),
        0,
        "failed proposer quorum must stop before synthesis"
    );
}

const QUOTA_429: &str = r#"HTTP 429: {"error":{"code":"1310","message":"Weekly/Monthly Limit Exhausted. Your limit will reset at 2026-07-03 11:33:39"}}"#;
const AUTH_401: &str = r#"HTTP 401: {"error":{"code":"1000","message":"身 份 验 证 失 败 。 "}}"#;

/// The failure that motivated the bench: one link's weekly/monthly cap runs
/// out mid-session. The role call must reroute to the next spendable link and
/// the turn must still deliver — not die with `agent error · HTTP 429`.
#[test]
fn sota_moa_reroutes_quota_exhausted_role_calls_to_fallback_link() {
    // Serialized: this test mutates process-global environment that other
    // tests also read or write. Without the lock they interleave and each
    // observes the other's value between its own set and restore.
    let _guard = crate::tests::env_lock();
    let _guard = EnvGuard::unset("ANGEL_SOTA_MOA_ALLOW_DEGRADED");
    let propose = Arc::new(ScriptedClub::new(
        "longcat",
        vec![Err(QUOTA_429), Err(QUOTA_429)],
    ));
    let backup = Arc::new(ScriptedClub::new("deepseek", vec![]));
    let aggregate = Arc::new(CountingClub::new("OPEN"));
    let propose_club: Arc<dyn Club> = propose.clone();
    let backup_club: Arc<dyn Club> = backup.clone();
    let mut swarm = sota_test_swarm(Arc::new(CountingClub::new("OPEN")), aggregate.clone(), true);
    swarm.clubs.propose = propose_club.clone();
    swarm.fallbacks = vec![propose_club, backup_club];

    let out = swarm
        .chat_streaming(
            &[ChatMsg::user("fix this race condition")],
            &[],
            &AtomicBool::new(false),
            &mut |_| {},
        )
        .expect("an exhausted proposer link fails over instead of killing the turn");
    assert!(matches!(out, ClubReply::Text(t) if !t.trim().is_empty()));
    assert_eq!(propose.count(), 2, "both proposer calls hit the primary");
    assert_eq!(
        backup.count(),
        2,
        "both proposer calls must reroute to the spendable link"
    );
    assert_eq!(
        aggregate.count(),
        1,
        "synthesis still runs after the reroute"
    );
}

#[test]
fn sota_moa_reroutes_auth_failed_role_calls_to_fallback_link() {
    let glm = Arc::new(ScriptedClub::new("glm", vec![Err(AUTH_401), Err(AUTH_401)]));
    let backup = Arc::new(ScriptedClub::new("openai", vec![]));
    let aggregate = Arc::new(CountingClub::new("OPEN"));
    let glm_club: Arc<dyn Club> = glm.clone();
    let backup_club: Arc<dyn Club> = backup.clone();
    let mut swarm = sota_test_swarm(Arc::new(CountingClub::new("OPEN")), aggregate.clone(), true);
    swarm.clubs.propose = glm_club.clone();
    swarm.fallbacks = vec![glm_club, backup_club];

    let out = swarm
        .chat_streaming(
            &[ChatMsg::user("compete and optimize this submission")],
            &[],
            &AtomicBool::new(false),
            &mut |_| {},
        )
        .expect("an auth-failed role link should hand off instead of killing the turn");
    assert!(matches!(out, ClubReply::Text(t) if !t.trim().is_empty()));
    assert_eq!(
        glm.count(),
        2,
        "both proposer calls hit the auth-failed link"
    );
    assert_eq!(
        backup.count(),
        2,
        "both proposer calls must reroute to the capable fallback"
    );
    assert_eq!(
        aggregate.count(),
        1,
        "synthesis still runs after the auth handoff"
    );
}

/// A transient failure (5xx, timeout) is the retry loop's business, not the
/// bench's: rerouting those would double-spend metered tokens on hiccups.
#[test]
fn quota_failover_ignores_non_quota_errors() {
    let primary = Arc::new(ScriptedClub::new("longcat", vec![Err("HTTP 500: boom")]));
    let backup = Arc::new(ScriptedClub::new("deepseek", vec![]));
    let primary_club: Arc<dyn Club> = primary.clone();
    let backup_club: Arc<dyn Club> = backup.clone();
    let mut swarm = SwarmClub::with_knobs("sota-moa", primary_club.clone(), Knobs::default());
    swarm.fallbacks = vec![backup_club];

    let err = swarm
        .chat_with_failover(&*primary_club, &[ChatMsg::user("q")])
        .expect_err("a non-quota error surfaces unchanged");
    assert!(err.contains("HTTP 500"), "{err}");
    assert_eq!(backup.count(), 0, "no failover on transient errors");
}

/// When the whole bench is exhausted the last quota error stands — and each
/// link is tried at most once, so two gate-less links can never ping-pong.
#[test]
fn quota_failover_stops_after_each_link_tried_once() {
    let primary = Arc::new(ScriptedClub::new("longcat", vec![Err(QUOTA_429)]));
    let second = Arc::new(ScriptedClub::new("deepseek", vec![Err(QUOTA_429)]));
    let primary_club: Arc<dyn Club> = primary.clone();
    let second_club: Arc<dyn Club> = second.clone();
    let mut swarm = SwarmClub::with_knobs("sota-moa", primary_club.clone(), Knobs::default());
    swarm.fallbacks = vec![primary_club.clone(), second_club];

    let err = swarm
        .chat_with_failover(&*primary_club, &[ChatMsg::user("q")])
        .expect_err("an all-exhausted bench surfaces the last quota error");
    assert!(crate::club::error_indicates_quota_exhausted(&err), "{err}");
    assert_eq!(primary.count(), 1, "the failed primary is never re-tried");
    assert_eq!(second.count(), 1, "each bench link gets exactly one shot");
}

/// Emits a delta, then dies with a quota error — the shape that must NOT fail
/// over, or the caller's transcript would carry the text twice.
struct StreamsThenDies;

impl Club for StreamsThenDies {
    fn label(&self) -> &str {
        "longcat"
    }
    fn respond(&self, _p: &str) -> Result<String, String> {
        Err("unused".to_string())
    }
    fn chat_streaming(
        &self,
        _messages: &[ChatMsg],
        _tools: &[ToolDef],
        _cancel: &AtomicBool,
        on_delta: &mut dyn FnMut(StreamDelta),
    ) -> Result<ClubReply, String> {
        on_delta(StreamDelta::Content("partial "));
        Err(QUOTA_429.to_string())
    }
}

#[test]
fn streaming_failover_never_duplicates_already_streamed_text() {
    let backup = Arc::new(ScriptedClub::new("deepseek", vec![]));
    let backup_club: Arc<dyn Club> = backup.clone();
    let primary: Arc<dyn Club> = Arc::new(StreamsThenDies);
    let mut swarm = SwarmClub::with_knobs("sota-moa", primary.clone(), Knobs::default());
    swarm.fallbacks = vec![backup_club];

    let mut content = String::new();
    let err = swarm
        .stream_with_failover(
            &*primary,
            &[ChatMsg::user("q")],
            &AtomicBool::new(false),
            &mut |d| {
                if let StreamDelta::Content(c) = d {
                    content.push_str(c);
                }
            },
            false,
        )
        .expect_err("mid-stream quota death must surface, not retry");
    assert!(crate::club::error_indicates_quota_exhausted(&err), "{err}");
    assert_eq!(content, "partial ", "streamed text arrives exactly once");
    assert_eq!(backup.count(), 0, "no failover after content has flowed");
}

/// The happy streaming case: the quota gate trips before the first byte, so
/// failover is safe and the backup's answer streams through untouched.
#[test]
fn streaming_failover_reroutes_when_nothing_streamed_yet() {
    let primary = Arc::new(ScriptedClub::new("longcat", vec![Err(QUOTA_429)]));
    let backup = Arc::new(ScriptedClub::new("deepseek", vec![Ok("rescued answer")]));
    let primary_club: Arc<dyn Club> = primary.clone();
    let backup_club: Arc<dyn Club> = backup.clone();
    let mut swarm = SwarmClub::with_knobs("sota-moa", primary_club.clone(), Knobs::default());
    swarm.fallbacks = vec![backup_club];

    let mut content = String::new();
    let out = swarm
        .stream_with_failover(
            &*primary_club,
            &[ChatMsg::user("q")],
            &AtomicBool::new(false),
            &mut |d| {
                if let StreamDelta::Content(c) = d {
                    content.push_str(c);
                }
            },
            false,
        )
        .expect("a pre-stream quota error fails over cleanly");
    assert!(matches!(out, ClubReply::Text(t) if t == "rescued answer"));
    assert_eq!(content, "rescued answer");
    assert_eq!(primary.count(), 1);
    assert_eq!(backup.count(), 1);
}

#[test]
fn sota_moa_adapts_simple_questions_fast_and_work_requests_slow() {
    let propose = Arc::new(CountingClub::new("OPEN"));
    let aggregate = Arc::new(CountingClub::new("OPEN"));
    let swarm = sota_test_swarm(propose.clone(), aggregate.clone(), false);
    let cancel = AtomicBool::new(false);
    let mut content = String::new();
    swarm
        .chat_streaming(&[ChatMsg::user("what is 2 + 2?")], &[], &cancel, &mut |d| {
            if let StreamDelta::Content(c) = d {
                content.push_str(c);
            }
        })
        .expect("simple question streams");
    assert_eq!(
        propose.count(),
        0,
        "simple questions must not pay proposer fan-out latency"
    );
    assert_eq!(
        aggregate.count(),
        1,
        "simple questions should take one direct aggregate call"
    );
    assert!(!content.trim().is_empty());

    let propose = Arc::new(CountingClub::new("OPEN"));
    let aggregate = Arc::new(CountingClub::new("OPEN"));
    let swarm = sota_test_swarm(propose.clone(), aggregate.clone(), false);
    let mut content = String::new();
    swarm
        .chat_streaming(
            &[ChatMsg::user("fix this null deref")],
            &[],
            &cancel,
            &mut |d| {
                if let StreamDelta::Content(c) = d {
                    content.push_str(c);
                }
            },
        )
        .expect("work request streams");
    assert_eq!(
        propose.count(),
        2,
        "work requests should use the SOTA-MOA proposer wave"
    );
    assert_eq!(
        aggregate.count(),
        1,
        "work requests should stream one synthesis call after fan-out"
    );
    assert!(!content.trim().is_empty());
}

// --- tool-capable driver (passthrough) --------------------------------------

/// A driver that always produces a long, distinct final answer (over the
/// amplify threshold) and counts every chat call — so a test can tell a single
/// direct pass (one call) from an amplified fan-out (many calls).
struct LongAnswerClub {
    calls: Mutex<usize>,
}

impl LongAnswerClub {
    fn new() -> Self {
        Self {
            calls: Mutex::new(0),
        }
    }
    fn count(&self) -> usize {
        *self.calls.lock().unwrap()
    }
}

impl Club for LongAnswerClub {
    fn label(&self) -> &str {
        "longcat"
    }
    fn respond(&self, p: &str) -> Result<String, String> {
        Ok(format!("echo:{p}"))
    }
    fn chat(&self, _messages: &[ChatMsg], _tools: &[ToolDef]) -> Result<ClubReply, String> {
        let mut calls = self.calls.lock().unwrap();
        *calls += 1;
        let n = *calls;
        // Distinct per call (no near-duplicate collapse) and comfortably over
        // MIN_AMPLIFY_CHARS so the amplify path isn't short-circuited.
        Ok(ClubReply::Text(format!(
            "answer variant {n}: {}",
            "the synthesis reasons carefully about the tradeoffs at some length ".repeat(6)
        )))
    }
}

struct ShortAnswerClub {
    calls: Mutex<usize>,
}

impl ShortAnswerClub {
    fn new() -> Self {
        Self {
            calls: Mutex::new(0),
        }
    }

    fn count(&self) -> usize {
        *self.calls.lock().unwrap()
    }
}

impl Club for ShortAnswerClub {
    fn label(&self) -> &str {
        "short"
    }

    fn respond(&self, p: &str) -> Result<String, String> {
        Ok(format!("brief:{p}"))
    }

    fn chat(&self, _messages: &[ChatMsg], _tools: &[ToolDef]) -> Result<ClubReply, String> {
        let mut calls = self.calls.lock().unwrap();
        *calls += 1;
        Ok(ClubReply::Text(format!("brief answer {}", *calls)))
    }
}

fn tool_def() -> ToolDef {
    ToolDef {
        name: "shell".to_string(),
        description: "run a shell command".to_string(),
        params: serde_json::json!({ "type": "object" }),
    }
}

/// The core fix: with tools offered, the MoA driver is a real agent — a tool
/// call is handed straight back to the harness, not swallowed by a text-only
/// think. Exactly one driver pass; no fan-out on a working hop.
#[test]
fn tool_capable_driver_passes_tool_calls_straight_through() {
    let tools = Arc::new(ToolCallingClub::new("longcat"));
    let club: Arc<dyn Club> = tools.clone();
    let swarm = SwarmClub::with_knobs("sota-moa", club, Knobs::default());
    let reply = swarm
        .chat(
            &[ChatMsg::user("investigate the failing build")],
            &[tool_def()],
        )
        .expect("the tool-capable driver hop returns");
    match reply {
        ClubReply::Calls(calls) => {
            assert_eq!(calls.len(), 1);
            assert_eq!(calls[0].name, "shell");
        }
        ClubReply::Text(t) => panic!("a tool call must pass through, got text: {t}"),
    }
    assert_eq!(
        tools.count(),
        1,
        "one driver pass, no MoA fan-out on a tool-calling hop"
    );
}

/// The harness drives via `chat_streaming` with the tool schemas — the real
/// entry point. A tool call still passes straight through (no MoA fan-out, no
/// stray answer content), so the agent actually gets to run its tools.
#[test]
fn tool_capable_driver_streams_tool_calls_through() {
    let tools = Arc::new(ToolCallingClub::new("longcat"));
    let club: Arc<dyn Club> = tools.clone();
    let swarm = SwarmClub::with_knobs("sota-moa", club, Knobs::default());
    let mut content = String::new();
    let reply = swarm
        .chat_streaming(
            &[ChatMsg::user("read foo.rs and fix the failing build")],
            &[tool_def()],
            &AtomicBool::new(false),
            &mut |d| {
                if let StreamDelta::Content(c) = d {
                    content.push_str(c);
                }
            },
        )
        .expect("the streamed driver hop returns");
    assert!(
        matches!(reply, ClubReply::Calls(ref c) if c[0].name == "shell"),
        "the tool call must reach the harness"
    );
    assert!(
        content.is_empty(),
        "a tool-calling hop streams no answer content, got: {content}"
    );
    assert_eq!(tools.count(), 1, "one driver pass, no fan-out");
}

/// An ordinary action turn works through tools and its answer is already
/// grounded in real output — return it directly, never re-synthesize it
/// text-only (which could only regress it). Zero MoA overhead.
#[test]
fn tool_capable_driver_answers_action_turns_directly() {
    let driver = Arc::new(LongAnswerClub::new());
    let club: Arc<dyn Club> = driver.clone();
    let swarm = SwarmClub::with_knobs("sota-moa", club, Knobs::default());
    let reply = swarm
        .chat(
            &[ChatMsg::user(
                "fix the failing test in foo.rs and update the assertions",
            )],
            &[tool_def()],
        )
        .expect("the driver hop returns");
    match reply {
        ClubReply::Text(t) => assert!(t.starts_with("answer variant 1"), "{t}"),
        ClubReply::Calls(_) => panic!("expected a direct text answer"),
    }
    assert_eq!(
        driver.count(),
        1,
        "an action answer is tool-grounded — no fan-out rewrite"
    );
}

/// A reasoning-dominant (synthesis) question is exactly where multi-agent
/// deliberation pays off: the final answer is amplified by the MoA fan-out.
#[test]
fn tool_capable_driver_amplifies_synthesis_turns() {
    let driver = Arc::new(LongAnswerClub::new());
    let club: Arc<dyn Club> = driver.clone();
    let swarm = SwarmClub::with_knobs("sota-moa", club, Knobs::default());
    let reply = swarm
        .chat(
            &[ChatMsg::user(
                "design a caching architecture and compare the tradeoffs",
            )],
            &[tool_def()],
        )
        .expect("the driver hop returns");
    assert!(
        matches!(reply, ClubReply::Text(_)),
        "a synthesis answer is text"
    );
    assert!(
        driver.count() > 1,
        "a synthesis turn fans the MoA out (many calls), got {}",
        driver.count()
    );
}

/// Manual mode — the sota-moa default since the token blowouts: even a
/// synthesis-flavored prompt that trips the auto heuristics stays
/// coordinator-only. Standard single-model spend until the user summons.
#[test]
fn manual_mode_keeps_the_coordinator_solo_until_summoned() {
    let driver = Arc::new(LongAnswerClub::new());
    let club: Arc<dyn Club> = driver.clone();
    let swarm = SwarmClub::with_knobs(
        "sota-moa",
        club,
        Knobs {
            mode: MoaMode::Manual,
            ..Knobs::default()
        },
    );
    let reply = swarm
        .chat(
            &[ChatMsg::user(
                "design a caching architecture and compare the tradeoffs",
            )],
            &[tool_def()],
        )
        .expect("the driver hop returns");
    assert!(matches!(reply, ClubReply::Text(_)));
    assert_eq!(
        driver.count(),
        1,
        "manual mode must never fan out unsummoned"
    );
}

/// The summon: a turn opening with `moa:` engages the full mixture for exactly
/// that turn — fan-out on demand.
#[test]
fn manual_mode_moa_prefix_summons_the_mixture() {
    let driver = Arc::new(LongAnswerClub::new());
    let club: Arc<dyn Club> = driver.clone();
    let swarm = SwarmClub::with_knobs(
        "sota-moa",
        club,
        Knobs {
            mode: MoaMode::Manual,
            ..Knobs::default()
        },
    );
    let reply = swarm
        .chat(
            &[ChatMsg::user(
                "moa: design a caching architecture and compare the tradeoffs",
            )],
            &[tool_def()],
        )
        .expect("the driver hop returns");
    assert!(matches!(reply, ClubReply::Text(_)));
    assert!(
        driver.count() > 1,
        "a summoned turn fans the MoA out, got {}",
        driver.count()
    );
}

#[test]
fn engaged_formation_state_summons_without_rewriting_operator_text() {
    let driver = Arc::new(LongAnswerClub::new());
    let club: Arc<dyn Club> = driver.clone();
    let swarm = SwarmClub::with_knobs(
        "sota-moa",
        club,
        Knobs {
            mode: MoaMode::Manual,
            ..Knobs::default()
        },
    )
    .with_engaged_formation();
    let operator_text = "design a caching architecture and compare the tradeoffs";
    let messages = [ChatMsg::user(operator_text)];
    let reply = swarm
        .chat(&messages, &[tool_def()])
        .expect("the staged MoA turn returns");
    assert!(matches!(reply, ClubReply::Text(_)));
    assert_eq!(&*messages[0].content, operator_text);
    assert!(
        driver.count() > 1,
        "typed formation state must engage the full formation"
    );
}

/// Engagement ≠ `always`: an armed formation performs its one planning
/// preflight before tools, but a short coordinator answer is returned as-is
/// rather than paying for a second, final-answer amplification wave.
#[test]
fn engaged_formation_does_not_re_amplify_a_short_ack() {
    let driver = Arc::new(ShortAnswerClub::new());
    let club: Arc<dyn Club> = driver.clone();
    let swarm = SwarmClub::with_knobs(
        "sota-moa",
        club,
        Knobs {
            mode: MoaMode::Manual,
            ..Knobs::default()
        },
    )
    .with_engaged_formation();
    let reply = swarm
        .chat(
            &[ChatMsg::user(
                "design a caching architecture and compare the tradeoffs",
            )],
            &[tool_def()],
        )
        .expect("the staged MoA turn returns");
    assert!(matches!(reply, ClubReply::Text(_)));
    assert!(
        driver.count() > 1,
        "the armed formation must plan before the coordinator can acknowledge"
    );
}

/// `ANGEL_SOTA_MOA_ALWAYS=1` stays the explicit "amplify everything" override:
/// `always` sets `explicitly_requested`, so even a short answer amplifies.
#[test]
fn always_override_amplifies_even_a_short_answer() {
    let driver = Arc::new(ShortAnswerClub::new());
    let club: Arc<dyn Club> = driver.clone();
    let swarm = SwarmClub::with_knobs(
        "sota-moa",
        club,
        Knobs {
            mode: MoaMode::Manual,
            always: true,
            ..Knobs::default()
        },
    );
    let reply = swarm
        .chat(&[ChatMsg::user("compare these two options")], &[tool_def()])
        .expect("the forced MoA turn returns");
    assert!(matches!(reply, ClubReply::Text(_)));
    assert!(
        driver.count() > 1,
        "`always` must amplify every turn, even a short one"
    );
}

#[test]
fn sealed_tool_preflight_runs_formation_once_before_coordinator_actions() {
    let _guard = crate::tests::env_lock();
    let _preflight = crate::tests::TestEnvGuard::set("ANGEL_SOTA_MOA_TOOL_PREFLIGHT", "1");
    let _required = crate::tests::TestEnvGuard::set("ANGEL_SOTA_MOA_TOOL_PREFLIGHT_REQUIRED", "1");
    let driver = Arc::new(PlanningToolClub::new());
    let club: Arc<dyn Club> = driver.clone();
    let swarm = SwarmClub::with_knobs(
        "sota-moa",
        club,
        Knobs {
            width: 2,
            max_width: 2,
            layers: 1,
            always: true,
            judge: false,
            verify: 0,
            samples: 1,
            ..Knobs::default()
        },
    );

    let first = swarm
        .chat(
            &[ChatMsg::user("optimize this real benchmark")],
            &[tool_def()],
        )
        .expect("preflight and coordinator call succeed");
    assert!(matches!(first, ClubReply::Calls(_)));
    assert!(
        *driver.text_calls.lock().unwrap() >= 3,
        "two proposers plus synthesis must run before the tool call"
    );
    assert_eq!(*driver.tool_calls.lock().unwrap(), 1);
    let history = driver.tool_history.lock().unwrap();
    assert!(history.iter().any(|message| {
        message.role == ChatRole::Harness
            && message.content.contains("Formation preflight advisory")
            && message.content.contains("no command")
            && message.content.contains("has run yet")
    }));
    drop(history);

    let calls_before = *driver.text_calls.lock().unwrap();
    let second_history = vec![
        ChatMsg::user("optimize this real benchmark"),
        ChatMsg::assistant_calls(vec![ToolCall {
            id: "preflight-tool-call".to_string(),
            name: "shell".to_string(),
            args: serde_json::json!({ "command": "pwd" }),
        }]),
        ChatMsg::tool("preflight-tool-call", "/workspace"),
    ];
    let second = swarm
        .chat(&second_history, &[tool_def()])
        .expect("later tool hop succeeds");
    assert!(matches!(second, ClubReply::Calls(_)));
    assert_eq!(
        *driver.text_calls.lock().unwrap(),
        calls_before,
        "the existing tool transcript suppresses duplicate preflight"
    );
    let replayed = driver.tool_history.lock().unwrap();
    assert_eq!(replayed[0].role, ChatRole::User);
    assert_eq!(replayed[1].role, ChatRole::Harness);
    assert!(replayed[1].content.contains("Formation preflight advisory"));
    assert_eq!(replayed[2].role, ChatRole::Assistant);
    assert_eq!(replayed[3].role, ChatRole::Tool);
}

#[test]
fn required_tool_preflight_failure_is_terminal_for_the_same_user_turn() {
    let _guard = crate::tests::env_lock();
    let _preflight = crate::tests::TestEnvGuard::set("ANGEL_SOTA_MOA_TOOL_PREFLIGHT", "1");
    let _required = crate::tests::TestEnvGuard::set("ANGEL_SOTA_MOA_TOOL_PREFLIGHT_REQUIRED", "1");
    let _full = crate::tests::TestEnvGuard::set("ANGEL_SOTA_MOA_REQUIRE_FULL_WIDTH", "1");
    let driver = Arc::new(ScriptedClub::new(
        "glm-5.3-flash",
        vec![
            Err("transient HTTP 429 on seat one"),
            Err("transient HTTP 429 on seat two"),
        ],
    ));
    let club: Arc<dyn Club> = driver.clone();
    let swarm = SwarmClub::with_config("sota-moa", club, 2, 1, true);
    let messages = [ChatMsg::user("optimize this real benchmark")];

    let first = swarm
        .chat(&messages, &[tool_def()])
        .expect_err("the required partial formation must fail closed");
    assert!(first.contains("SOTA-MOA tool preflight failed"), "{first}");
    assert!(first.contains("need at least 2"), "{first}");
    let calls_after_first = driver.count();
    assert_eq!(
        calls_after_first, 2,
        "the first attempt buys one width-2 wave"
    );

    let second = swarm
        .chat(&messages, &[tool_def()])
        .expect_err("an identical outer retry must keep the terminal failure");
    assert_eq!(second, first);
    assert_eq!(
        driver.count(),
        calls_after_first,
        "retrying the same user turn must not buy another formation wave",
    );

    let _ = swarm.chat(
        &[ChatMsg::user("optimize a different real benchmark")],
        &[tool_def()],
    );
    assert!(
        driver.count() > calls_after_first,
        "a genuinely new user turn must receive a fresh preflight attempt",
    );
}

#[test]
fn preflight_only_returns_the_tool_grounded_answer_without_a_second_moa_wave() {
    let _guard = crate::tests::env_lock();
    let _preflight = crate::tests::TestEnvGuard::set("ANGEL_SOTA_MOA_TOOL_PREFLIGHT", "1");
    let _required = crate::tests::TestEnvGuard::set("ANGEL_SOTA_MOA_TOOL_PREFLIGHT_REQUIRED", "1");
    let _preflight_only = crate::tests::TestEnvGuard::set("ANGEL_SOTA_MOA_PREFLIGHT_ONLY", "1");
    let driver = Arc::new(PlanningThenAnswerClub::new());
    let club: Arc<dyn Club> = driver.clone();
    let swarm = SwarmClub::with_knobs(
        "sota-moa",
        club,
        Knobs {
            width: 2,
            max_width: 2,
            layers: 1,
            always: true,
            judge: false,
            verify: 0,
            samples: 1,
            ..Knobs::default()
        },
    );

    let first = swarm
        .chat(
            &[ChatMsg::user("optimize this real benchmark")],
            &[tool_def()],
        )
        .expect("preflight and coordinator call succeed");
    assert!(matches!(first, ClubReply::Calls(_)));
    let preflight_text_calls = *driver.text_calls.lock().unwrap();
    assert!(preflight_text_calls >= 3, "the planning formation must run");

    let second = swarm
        .chat(
            &[
                ChatMsg::user("optimize this real benchmark"),
                ChatMsg::assistant_calls(vec![ToolCall {
                    id: "preflight-only-tool-call".to_string(),
                    name: "shell".to_string(),
                    args: serde_json::json!({ "command": "pwd" }),
                }]),
                ChatMsg::tool("preflight-only-tool-call", "/workspace"),
            ],
            &[tool_def()],
        )
        .expect("the grounded final answer succeeds");
    assert!(matches!(second, ClubReply::Text(_)));
    assert_eq!(
        *driver.text_calls.lock().unwrap(),
        preflight_text_calls,
        "the final answer must not buy a second text-only formation",
    );
    assert_eq!(*driver.tool_calls.lock().unwrap(), 2);
}

#[test]
fn armed_formation_preflights_tool_work_by_default() {
    let _guard = crate::tests::env_lock();
    let _preflight = crate::tests::TestEnvGuard::unset("ANGEL_SOTA_MOA_TOOL_PREFLIGHT");
    let _required = crate::tests::TestEnvGuard::unset("ANGEL_SOTA_MOA_TOOL_PREFLIGHT_REQUIRED");
    let driver = Arc::new(PlanningToolClub::new());
    let club: Arc<dyn Club> = driver.clone();
    let swarm = SwarmClub::with_knobs(
        "sota-moa",
        club,
        Knobs {
            width: 2,
            max_width: 2,
            mode: MoaMode::Manual,
            judge: false,
            verify: 0,
            samples: 1,
            ..Knobs::default()
        },
    )
    .with_engaged_formation();

    let reply = swarm
        .chat(&[ChatMsg::user("optimize this repository")], &[tool_def()])
        .expect("armed formation and coordinator call succeed");

    assert!(matches!(reply, ClubReply::Calls(_)));
    assert!(
        *driver.text_calls.lock().unwrap() >= 3,
        "the armed formation must run before repository tools"
    );
    assert!(driver.tool_history.lock().unwrap().iter().any(|message| {
        message.role == ChatRole::Harness
            && message.content.contains("Formation preflight advisory")
    }));
}

#[test]
fn strict_formation_width_rejects_a_partial_proposer_wave() {
    let _guard = crate::tests::env_lock();
    let _full = crate::tests::TestEnvGuard::set("ANGEL_SOTA_MOA_REQUIRE_FULL_WIDTH", "1");
    let _degraded = crate::tests::TestEnvGuard::set("ANGEL_SOTA_MOA_ALLOW_DEGRADED", "1");
    let inner = Arc::new(ScriptedClub::new(
        "glm-5.3-flash",
        vec![Ok("draft one"), Err("seat failed"), Ok("draft three")],
    ));
    let swarm = SwarmClub::with_config("sota-moa", inner, 3, 1, true);
    let error = swarm
        .run(
            &[ChatMsg::user("design the benchmark optimization")],
            &AtomicBool::new(false),
            &mut |_| {},
            false,
        )
        .expect_err("strict width must not launder a partial wave");
    assert!(error.contains("need at least 3"), "{error}");
}

#[test]
fn explicit_moa_summon_is_not_vetoed_by_a_short_driver_answer() {
    let driver = Arc::new(ShortAnswerClub::new());
    let club: Arc<dyn Club> = driver.clone();
    let swarm = SwarmClub::with_knobs(
        "sota-moa",
        club,
        Knobs {
            mode: MoaMode::Manual,
            ..Knobs::default()
        },
    );
    let reply = swarm
        .chat(
            &[ChatMsg::user("moa: compare these two options")],
            &[tool_def()],
        )
        .expect("explicit MoA turn returns");
    assert!(matches!(reply, ClubReply::Text(_)));
    assert!(
        driver.count() > 1,
        "explicit engagement must run the formation even when the coordinator is concise"
    );
}

/// Driver hop, raw markup on the assigned link: fail over across the bench and
/// pass the fallback's *structured* tool call straight through. Each link gets
/// exactly one shot — bounded, no ping-pong.
#[test]
fn tool_stage_reroutes_raw_markup_to_link_with_structured_call() {
    let raw = r#"<longcat_tool_call>shell
<longcat_arg_key>command</longcat_arg_key>
<longcat_arg_value>cargo test</longcat_arg_value>
</longcat_tool_call>"#;
    let driver = Arc::new(ScriptedClub::new("openai", vec![Ok(raw)]));
    let capable = Arc::new(ToolCallingClub::new("deepseek"));
    let driver_club: Arc<dyn Club> = driver.clone();
    let capable_club: Arc<dyn Club> = capable.clone();
    let mut swarm = SwarmClub::with_knobs("sota-moa", driver_club.clone(), Knobs::default());
    swarm.fallbacks = vec![driver_club, capable_club];

    let reply = swarm
        .drive_with_tools(
            &[ChatMsg::user("run the failing test and report")],
            &[tool_def()],
            &AtomicBool::new(false),
            &mut |_| {},
            false,
        )
        .expect("a capable fallback link recovers the botched call");
    assert!(
        matches!(reply, ClubReply::Calls(ref c) if c[0].name == "shell"),
        "the structured tool call must reach the harness"
    );
    assert_eq!(driver.count(), 1, "the botched link gets exactly one shot");
    assert_eq!(capable.count(), 1);
}

/// The whole bench botches the tool call as raw markup: the markup must surface
/// to the harness (whose false-start guard forces a bounded structured retry) —
/// NOT be swallowed into a text-only MoA answer that describes work that never
/// ran. This was the "agent finishes without ever testing" hole.
#[test]
fn tool_stage_bench_exhausted_on_raw_markup_surfaces_markup_not_moa_answer() {
    let _guard = crate::tests::env_lock();
    let _preflight = crate::tests::TestEnvGuard::set("ANGEL_SOTA_MOA_TOOL_PREFLIGHT", "0");
    let raw = r#"<longcat_tool_call>shell
<longcat_arg_key>command</longcat_arg_key>
<longcat_arg_value>cargo test</longcat_arg_value>
</longcat_tool_call>"#;
    let driver = Arc::new(ScriptedClub::new("openai", vec![Ok(raw)]));
    let fallback = Arc::new(ScriptedClub::new("deepseek", vec![Ok(raw)]));
    let driver_club: Arc<dyn Club> = driver.clone();
    let fallback_club: Arc<dyn Club> = fallback.clone();
    // `always` forces the amplify intent — exactly the configuration that used
    // to recover the swallowed markup error with a fabricated text answer.
    let mut swarm = SwarmClub::with_knobs(
        "sota-moa",
        driver_club.clone(),
        Knobs {
            always: true,
            ..Knobs::default()
        },
    );
    swarm.fallbacks = vec![driver_club, fallback_club];

    let reply = swarm
        .drive_with_tools(
            &[ChatMsg::user("run the failing test and report")],
            &[tool_def()],
            &AtomicBool::new(false),
            &mut |_| {},
            false,
        )
        .expect("exhausted bench surfaces the markup, not an error");
    match reply {
        ClubReply::Text(t) => {
            assert!(
                crate::club::contains_raw_tool_markup(&t),
                "the raw markup must survive to the harness guard, got: {t}"
            );
            assert!(t.contains("cargo test"), "{t}");
        }
        ClubReply::Calls(_) => panic!("no structured call existed to pass through"),
    }
    // One shot per link, then stop — no MoA amplification pass on top.
    assert_eq!(driver.count(), 1, "bounded fail-over: one shot per link");
    assert_eq!(fallback.count(), 1, "no MoA fan-out laundered the markup");
}

/// `run_seeded` folds the driver's pre-computed answer into the draft pool:
/// a downstream stage (refine/judge/synthesis) sees the seed text alongside
/// the proposer drafts instead of the generation being discarded.
#[test]
fn seeded_draft_reaches_the_synthesis_pool() {
    let inner = Arc::new(TranscriptRecordingClub::new("swarm"));
    let swarm = SwarmClub::with_config("swarm", inner.clone(), 3, 2, false);
    let seed = "SEEDMARK: the driver already reasoned through the tradeoffs here";
    let out = swarm
        .run_seeded(
            &[ChatMsg::user("design a resilient distributed system")],
            &AtomicBool::new(false),
            &mut |_| {},
            false,
            Some(seed),
        )
        .expect("seeded run returns");
    assert!(!out.is_empty());
    let saw_seed = inner
        .seen
        .lock()
        .unwrap()
        .iter()
        .flatten()
        .any(|m| m.content.contains("SEEDMARK"));
    assert!(
        saw_seed,
        "the seed must ride into a downstream stage's transcript"
    );
}

#[test]
fn open_problem_fans_out_across_layers() {
    // Distinct drafts, so the near-duplicate collapse stays out of the way
    // and the full ladder shape is visible.
    let inner = Arc::new(DistinctClub::new());
    let swarm = SwarmClub::with_config("swarm", inner.clone(), 3, 2, false);
    let out = swarm
        .respond("design a resilient distributed system")
        .unwrap();
    assert!(!out.is_empty());
    // Default fast router: NO classifier round-trip. "design ..." trips the
    // deliberation heuristic → 3 proposers + 3 layer-1 aggregators + 1 synthesis.
    assert_eq!(inner.count(), 7);
}

#[test]
fn convergent_proposers_collapse_locally_and_skip_the_ladder() {
    let _guard = crate::tests::env_lock();
    let _dedup = EnvGuard::set("ANGEL_MOA_DEDUP_SIM", "0.9");
    // Identical drafts (the CountingClub always says "draft") collapse via
    // the local similarity check — no refine layer, no judge — and the one
    // surviving draft still gets a real synthesis pass so the user never
    // sees raw proposer material: 3 proposers + 1 synthesis, nothing else.
    let inner = Arc::new(CountingClub::new("OPEN"));
    let swarm = SwarmClub::with_config("swarm", inner.clone(), 3, 2, false);
    let out = swarm
        .respond("design a resilient distributed system")
        .unwrap();
    assert!(!out.is_empty());
    assert_eq!(
        inner.count(),
        4,
        "convergent drafts must skip the refine layer (3 propose + 1 synthesis)"
    );
}

#[test]
fn tight_directive_single_passes() {
    let inner = Arc::new(CountingClub::new("TIGHT"));
    let swarm = SwarmClub::with_config("swarm", inner.clone(), 4, 2, false);
    let out = swarm.respond("what is 2 + 2?").unwrap();
    assert_eq!(out, "draft"); // the single direct pass
    // Default fast router: the heuristic sends a narrow directive straight to one
    // direct pass with NO classifier round-trip — the broad win (was 2, now 1).
    assert_eq!(inner.count(), 1);
}

#[test]
fn default_router_costs_no_classifier_round_trip() {
    // Every non-deliberation turn answers in ONE call — no routing model call.
    for prompt in [
        "thanks!",
        "what is 2 + 2?",
        "read the config file",
        "rename x to y",
    ] {
        let inner = Arc::new(CountingClub::new("TIGHT"));
        let swarm = SwarmClub::with_config("swarm", inner.clone(), 4, 2, false);
        let out = swarm.respond(prompt).unwrap();
        assert_eq!(out, "draft");
        assert_eq!(inner.count(), 1, "{prompt:?} must answer in one call");
    }
}

/// The driver's tool instructions must not reach a seat that has no tools.
#[test]
fn text_only_system_drops_tool_protocol_but_keeps_context() {
    let base = "You are Angel — the Driver for this workspace.\n\n\
        Tool protocol: tools are available through the structured tool-call interface advertised \
        by the host. Use built-in tools for local work, `skill(name)` for reusable playbooks.\n\n\
        Default posture: work only on the user's current task.";
    let out = text_only_system(base);
    assert!(out.contains("You are Angel"), "{out}");
    assert!(out.contains("Default posture"), "{out}");
    assert!(!out.contains("Tool protocol"), "{out}");
    assert!(!out.contains("skill(name)"), "{out}");
    assert!(out.contains("no tools here"), "{out}");
    // An empty driver prompt still gets the directive, never a stray blank line.
    assert!(text_only_system("").starts_with("This is an analysis-only stage"));
}

/// The live failure this fixes: Leanstral answered every Tag Team turn with
/// `[TOOL_CALLS]quantize-model[TOOL_CALLS]{"model": "4bit"}` — 55 characters, no
/// analysis — so its draft was discarded and the two-corner formation silently
/// became one model. Correcting the seat in place recovers its angle.
#[test]
fn swarm_worker_recovers_after_a_text_only_correction() {
    const CALL: &str = r#"[TOOL_CALLS]quantize-model[TOOL_CALLS]{"model": "4bit"}"#;
    const PROSE: &str = "- Router precision loss dominates: expert logits are close together, so \
        4-bit rounding reorders the top-k and traffic collapses onto a few experts.";
    let worker = Arc::new(ScriptedClub::new("leanstral", vec![Ok(CALL), Ok(PROSE)]));
    let aggregate = Arc::new(CountingClub::new("OPEN"));
    let worker_club: Arc<dyn Club> = worker.clone();
    let aggregate_club: Arc<dyn Club> = aggregate.clone();
    let swarm = SwarmClub {
        name: "sota-moa".to_string(),
        clubs: RoleClubs {
            propose: worker_club.clone(),
            propose_extra: Vec::new(),
            judge: aggregate_club.clone(),
            judge_extra: Vec::new(),
            verify: aggregate_club.clone(),
            verify_extra: Vec::new(),
            aggregate: aggregate_club,
            aggregate_extra: Vec::new(),
            research: None,
        },
        k: Knobs::default(),
        fallbacks: Vec::new(),
        tool_preflight: Default::default(),
    };

    let draft = swarm
        .call_on(&*worker_club, "worker system", &[ChatMsg::user("analyze")])
        .expect("the corrected retry is a usable draft");
    assert!(draft.contains("Router precision loss"), "{draft}");
    // Exactly one corrective retry — not a ladder.
    assert_eq!(worker.count(), 2);
}

/// A seat that writes real analysis and *then* slips into tool markup keeps the
/// analysis. Discarding the whole reply is what quietly reduced a two-corner
/// formation to a single model, because the tail is the only bad part.
#[test]
fn swarm_worker_salvages_analysis_written_before_tool_markup() {
    const RAW: &str = "- Routing collapse: 4-bit quantization flattens expert logits, so the \
        router loses the fine-grained differences it discriminates on and folds traffic onto a \
        few robust experts, negating sparsity.\n- Memory is not the binding constraint: weights \
        shrink ~4x but activations and KV stay wide, so a bandwidth-bound decode barely moves.\n\n\
        <SHELL>{\"cmd\":\"nvidia-smi\"}";
    let worker = Arc::new(ScriptedClub::new("leanstral", vec![Ok(RAW)]));
    let aggregate = Arc::new(CountingClub::new("OPEN"));
    let worker_club: Arc<dyn Club> = worker.clone();
    let aggregate_club: Arc<dyn Club> = aggregate.clone();
    let swarm = SwarmClub {
        name: "sota-moa".to_string(),
        clubs: RoleClubs {
            propose: worker_club.clone(),
            propose_extra: Vec::new(),
            judge: aggregate_club.clone(),
            judge_extra: Vec::new(),
            verify: aggregate_club.clone(),
            verify_extra: Vec::new(),
            aggregate: aggregate_club,
            aggregate_extra: Vec::new(),
            research: None,
        },
        k: Knobs::default(),
        fallbacks: Vec::new(),
        tool_preflight: Default::default(),
    };

    let draft = swarm
        .call_on(&*worker_club, "worker system", &[ChatMsg::user("analyze")])
        .expect("analysis before the markup is a usable draft");
    assert!(draft.contains("Routing collapse"), "{draft}");
    // The call itself is still never honored, and no markup survives into the
    // material the aggregator sees.
    assert!(!draft.contains("<SHELL>"), "{draft}");
    assert!(!draft.contains("nvidia-smi"), "{draft}");
}

#[test]
fn swarm_worker_rejects_raw_longcat_tool_markup_text() {
    let raw = r#"angel <longcat_tool_call>shell
<parameter=command>cd /workspace/fixture-overworld && find . -maxdepth 4 -type f ( -name "gauntlet" -o -name "harness" )</parameter>
</function>"#;
    // Scripted twice: the seat repeats the markup even after the text-only
    // correction, which is the case that must still fail closed.
    let worker = Arc::new(ScriptedClub::new("longcat", vec![Ok(raw), Ok(raw)]));
    let aggregate = Arc::new(CountingClub::new("OPEN"));
    let worker_club: Arc<dyn Club> = worker.clone();
    let aggregate_club: Arc<dyn Club> = aggregate.clone();
    let swarm = SwarmClub {
        name: "sota-moa".to_string(),
        clubs: RoleClubs {
            propose: worker_club.clone(),
            propose_extra: Vec::new(),
            judge: aggregate_club.clone(),
            judge_extra: Vec::new(),
            verify: aggregate_club.clone(),
            verify_extra: Vec::new(),
            aggregate: aggregate_club,
            aggregate_extra: Vec::new(),
            research: None,
        },
        k: Knobs::default(),
        fallbacks: Vec::new(),
        tool_preflight: Default::default(),
    };

    let err = swarm
        .call_on(&*worker_club, "worker system", &[ChatMsg::user("inspect")])
        .expect_err("raw provider tool markup must not become a usable MOA draft");
    assert!(err.contains("raw tool markup"), "{err}");
    // One corrective retry on the seat, then it is out of chances.
    assert_eq!(worker.count(), 2);
}

#[test]
fn sota_text_stage_reroutes_raw_markup_to_fallback_link() {
    let raw = r#"<longcat_tool_call>shell
<longcat_arg_key>command</longcat_arg_key>
<longcat_arg_value>pwd</longcat_arg_value>
</longcat_tool_call>"#;
    // A link that emits markup *again* after the correction is genuinely stuck,
    // and only then is routing around it the right move.
    let longcat = Arc::new(ScriptedClub::new("longcat", vec![Ok(raw), Ok(raw)]));
    let glm = Arc::new(ScriptedClub::new("glm", vec![Ok("clean aggregate")]));
    let longcat_club: Arc<dyn Club> = longcat.clone();
    let glm_club: Arc<dyn Club> = glm.clone();
    let mut swarm = SwarmClub::with_knobs("sota-moa", longcat_club.clone(), Knobs::default());
    swarm.fallbacks = vec![longcat_club.clone(), glm_club];

    let out = swarm
        .call_on(&*longcat_club, "aggregate", &[ChatMsg::user("merge")])
        .expect("raw text-stage markup should fail over to a clean SOTA link");
    assert_eq!(out, "clean aggregate");
    assert_eq!(longcat.count(), 2); // original + corrective retry
    assert_eq!(glm.count(), 1);
}

#[test]
fn sota_text_stage_hands_failed_tool_call_to_capable_fallback() {
    let longcat = Arc::new(ToolCallingClub::new("longcat"));
    let codex = Arc::new(ScriptedClub::new(
        "openai",
        vec![Ok("clean capable handoff")],
    ));
    let longcat_club: Arc<dyn Club> = longcat.clone();
    let codex_club: Arc<dyn Club> = codex.clone();
    let mut swarm = SwarmClub::with_knobs("sota-moa", longcat_club.clone(), Knobs::default());
    swarm.fallbacks = vec![longcat_club.clone(), codex_club];

    let out = swarm
        .call_on(&*longcat_club, "aggregate", &[ChatMsg::user("merge")])
        .expect("tool-call attempt in a text-only stage should hand off to fallback");
    assert_eq!(out, "clean capable handoff");
    assert_eq!(longcat.count(), 2); // original + corrective retry
    assert_eq!(codex.count(), 1);
}

#[test]
fn swarm_streamed_final_rejects_raw_markup_before_user_content() {
    let raw = r#"<longcat_tool_call>shell
<longcat_arg_key>command</longcat_arg_key>
<longcat_arg_value>ls -la /workspace/fixture-overworld/</longcat_arg_value>
</longcat_tool_call>"#;
    let aggregate = Arc::new(ScriptedClub::new("longcat", vec![Ok(raw)]));
    let aggregate_club: Arc<dyn Club> = aggregate.clone();
    let swarm = SwarmClub {
        name: "sota-moa".to_string(),
        clubs: RoleClubs {
            propose: aggregate_club.clone(),
            propose_extra: Vec::new(),
            judge: aggregate_club.clone(),
            judge_extra: Vec::new(),
            verify: aggregate_club.clone(),
            verify_extra: Vec::new(),
            aggregate: aggregate_club,
            aggregate_extra: Vec::new(),
            research: None,
        },
        k: Knobs::default(),
        fallbacks: Vec::new(),
        tool_preflight: Default::default(),
    };

    let mut visible = String::new();
    let err = swarm
        .single_pass(
            &[ChatMsg::user("inspect")],
            &AtomicBool::new(false),
            &mut |d| {
                if let StreamDelta::Content(t) = d {
                    visible.push_str(t);
                }
            },
            true,
        )
        .expect_err("streamed raw provider tool markup must fail before user-visible content");
    assert!(err.contains("raw tool markup"), "{err}");
    assert!(visible.is_empty(), "raw markup leaked to stream: {visible}");
    assert_eq!(aggregate.count(), 1);
}

#[test]
fn buffered_streamed_aggregate_reroutes_raw_markup_before_user_content() {
    let raw = r#"<longcat_tool_call>shell
<longcat_arg_key>command</longcat_arg_key>
<longcat_arg_value>pwd</longcat_arg_value>
</longcat_tool_call>"#;
    let longcat = Arc::new(ScriptedClub::new("longcat", vec![Ok(raw)]));
    let glm = Arc::new(ScriptedClub::new(
        "glm",
        vec![Ok("clean streamed aggregate")],
    ));
    let longcat_club: Arc<dyn Club> = longcat.clone();
    let glm_club: Arc<dyn Club> = glm.clone();
    let mut swarm = SwarmClub::with_knobs("sota-moa", longcat_club.clone(), Knobs::default());
    swarm.fallbacks = vec![longcat_club.clone(), glm_club];

    let mut visible = String::new();
    let out = swarm
        .checked_stream_text_reply(
            &*longcat_club,
            &[ChatMsg::user("merge")],
            &AtomicBool::new(false),
            &mut |d| {
                if let StreamDelta::Content(t) = d {
                    visible.push_str(t);
                }
            },
        )
        .expect("buffered raw aggregate text should fail over before display");
    assert_eq!(out, "clean streamed aggregate");
    assert_eq!(visible, "clean streamed aggregate");
    assert_eq!(longcat.count(), 1);
    assert_eq!(glm.count(), 1);
}

#[test]
fn grok_research_scout_adds_context_without_replacing_role_seats() {
    // Serialized: this test mutates process-global environment that other
    // tests also read or write. Without the lock they interleave and each
    // observes the other's value between its own set and restore.
    let _guard = crate::tests::env_lock();
    let _guard = EnvGuard::unset("ANGEL_GROK_RESEARCH_ALWAYS");
    let propose = Arc::new(TranscriptRecordingClub::new("propose"));
    let aggregate = Arc::new(CountingClub::new("OPEN"));
    let research = Arc::new(ResearchScoutClub::new(
        "fresh finding from school: xAI posted a launch note today. https://x.ai",
    ));
    let propose_club: Arc<dyn Club> = propose.clone();
    let aggregate_club: Arc<dyn Club> = aggregate.clone();
    let research_club: Arc<dyn Club> = research.clone();
    let swarm = SwarmClub {
        name: "sota-moa".to_string(),
        clubs: RoleClubs {
            propose: propose_club,
            propose_extra: Vec::new(),
            judge: aggregate_club.clone(),
            judge_extra: Vec::new(),
            verify: aggregate_club.clone(),
            verify_extra: Vec::new(),
            aggregate: aggregate_club,
            aggregate_extra: Vec::new(),
            research: Some(research_club),
        },
        k: Knobs {
            width: 2,
            max_width: 2,
            refine_width: 2,
            layers: 1,
            always: false,
            reflect: false,
            judge: false,
            judge_panel: 1,
            dims: false,
            keep: 0,
            samples: 1,
            verify: 0,
            verify_guard: false,
            research: false,
            cite: false,
            hedge: false,
            search_url: default_search_url(),
            delegate: false,
            max_tests: 2,
            dissent_gate: false,
            ..Knobs::default()
        },
        fallbacks: Vec::new(),
        tool_preflight: Default::default(),
    };

    let out = swarm
        .respond("what is the latest xAI release today?")
        .expect("live prompt should run MoA with research scout");
    assert!(!out.trim().is_empty());
    assert_eq!(research.count(), 1, "live prompt should wake the scout");
    assert_eq!(
        propose.count(),
        2,
        "research must not replace proposer seats"
    );
    assert_eq!(aggregate.count(), 1, "normal synthesis seat still runs");
    assert!(
        research.last_prompt().contains("live research scout"),
        "scout prompt should define the role"
    );

    let joined = propose
        .last_messages()
        .iter()
        .map(|m| m.content.as_ref())
        .collect::<Vec<_>>()
        .join("\n");
    assert!(joined.contains("Grok research scout"), "{joined}");
    assert!(joined.contains("fresh finding from school"), "{joined}");
}

#[test]
fn grok_research_scout_stays_off_for_non_live_direct_turns() {
    // Serialized: this test mutates process-global environment that other
    // tests also read or write. Without the lock they interleave and each
    // observes the other's value between its own set and restore.
    let _guard = crate::tests::env_lock();
    let _guard = EnvGuard::unset("ANGEL_GROK_RESEARCH_ALWAYS");
    let propose = Arc::new(TranscriptRecordingClub::new("propose"));
    let aggregate = Arc::new(CountingClub::new("OPEN"));
    let research = Arc::new(ResearchScoutClub::new("fresh finding"));
    let propose_club: Arc<dyn Club> = propose.clone();
    let aggregate_club: Arc<dyn Club> = aggregate.clone();
    let research_club: Arc<dyn Club> = research.clone();
    let swarm = SwarmClub {
        name: "sota-moa".to_string(),
        clubs: RoleClubs {
            propose: propose_club,
            propose_extra: Vec::new(),
            judge: aggregate_club.clone(),
            judge_extra: Vec::new(),
            verify: aggregate_club.clone(),
            verify_extra: Vec::new(),
            aggregate: aggregate_club,
            aggregate_extra: Vec::new(),
            research: Some(research_club),
        },
        k: Knobs {
            width: 2,
            max_width: 2,
            layers: 1,
            always: false,
            research: false,
            dissent_gate: false,
            ..Knobs::default()
        },
        fallbacks: Vec::new(),
        tool_preflight: Default::default(),
    };

    let out = swarm.respond("what is a prefix?").unwrap();
    assert!(!out.trim().is_empty());
    assert_eq!(research.count(), 0, "ordinary direct turns skip the scout");
    assert_eq!(
        propose.count(),
        0,
        "ordinary direct turns skip proposer fan-out"
    );
    assert_eq!(aggregate.count(), 1, "ordinary direct turns stay one-pass");
}

#[test]
fn sota_moa_grok_researcher_is_explicit_on_deliberate_turns() {
    // Serialized: this test mutates process-global environment that other
    // tests also read or write. Without the lock they interleave and each
    // observes the other's value between its own set and restore.
    let _guard = crate::tests::env_lock();
    let _guard = EnvGuard::unset("ANGEL_GROK_RESEARCH_ALWAYS");
    let _sota_guard = EnvGuard::unset("ANGEL_SOTA_MOA_GROK_RESEARCH");
    let propose = Arc::new(TranscriptRecordingClub::new("propose"));
    let aggregate = Arc::new(CountingClub::new("OPEN"));
    let research = Arc::new(ResearchScoutClub::new("current outside context"));
    let propose_club: Arc<dyn Club> = propose.clone();
    let aggregate_club: Arc<dyn Club> = aggregate.clone();
    let research_club: Arc<dyn Club> = research.clone();
    let swarm = SwarmClub {
        name: "sota-moa".to_string(),
        clubs: RoleClubs {
            propose: propose_club,
            propose_extra: Vec::new(),
            judge: aggregate_club.clone(),
            judge_extra: Vec::new(),
            verify: aggregate_club.clone(),
            verify_extra: Vec::new(),
            aggregate: aggregate_club,
            aggregate_extra: Vec::new(),
            research: Some(research_club),
        },
        k: Knobs {
            width: 2,
            max_width: 2,
            layers: 1,
            always: false,
            research: false,
            dissent_gate: false,
            ..Knobs::default()
        },
        fallbacks: Vec::new(),
        tool_preflight: Default::default(),
    };

    let out = swarm
        .respond("design a durable routing calibration pass")
        .unwrap();
    assert!(!out.trim().is_empty());
    assert_eq!(
        research.count(),
        0,
        "deliberation alone must not put a CLI scout in front of the turn"
    );
    assert_eq!(
        propose.count(),
        2,
        "researcher must not replace proposer seats"
    );

    let _sota_enabled = EnvGuard::set("ANGEL_SOTA_MOA_GROK_RESEARCH", "1");
    let out = swarm
        .respond("design a durable routing calibration pass")
        .unwrap();
    assert!(!out.trim().is_empty());
    assert_eq!(research.count(), 1, "explicit SOTA scout should run once");
    assert_eq!(propose.count(), 4, "scout remains additive to proposers");
    let joined = propose
        .last_messages()
        .iter()
        .map(|m| m.content.as_ref())
        .collect::<Vec<_>>()
        .join("\n");
    assert!(joined.contains("Grok research scout"), "{joined}");
}

#[test]
fn moa_workers_receive_compaction_checkpoint_contract() {
    // Serialized: this test mutates process-global environment that other
    // tests also read or write. Without the lock they interleave and each
    // observes the other's value between its own set and restore.
    let _guard = crate::tests::env_lock();
    let _guard = EnvGuard::set("ANGEL_MOA_COMPACT_EVERY_LOOPS", "3");
    let propose = Arc::new(TranscriptRecordingClub::new("propose"));
    let aggregate = Arc::new(CountingClub::new("OPEN"));
    let propose_club: Arc<dyn Club> = propose.clone();
    let aggregate_club: Arc<dyn Club> = aggregate.clone();
    let swarm = SwarmClub {
        name: "sota-moa".to_string(),
        clubs: RoleClubs {
            propose: propose_club,
            propose_extra: Vec::new(),
            judge: aggregate_club.clone(),
            judge_extra: Vec::new(),
            verify: aggregate_club.clone(),
            verify_extra: Vec::new(),
            aggregate: aggregate_club,
            aggregate_extra: Vec::new(),
            research: None,
        },
        k: Knobs {
            width: 2,
            max_width: 2,
            layers: 1,
            always: true,
            dissent_gate: false,
            ..Knobs::default()
        },
        fallbacks: Vec::new(),
        tool_preflight: Default::default(),
    };

    let _ = swarm.respond("design a durable MoA run loop").unwrap();
    let joined = propose
        .last_messages()
        .iter()
        .map(|m| m.content.as_ref())
        .collect::<Vec<_>>()
        .join("\n");
    assert!(
        joined.contains("compacted after every 3 loop(s)"),
        "{joined}"
    );
    assert!(joined.contains("checkpoint-ready"), "{joined}");
}

#[test]
fn wants_deliberation_is_focused() {
    for d in [
        "design a resilient cache",
        "compare gRPC and REST here",
        "what are the tradeoffs?",
        "brainstorm approaches to sharding",
        "fix this null deref",
        "implement compact input composer",
        "debug the moa output",
        "can you fix this null deref",
        "please update the docs",
    ] {
        assert!(super::wants_deliberation(d), "{d:?} should fan out");
    }
    for s in [
        "what is 2 + 2?",
        "read the file src/main.rs",
        "thanks",
        "what is the latest version?",
        "what is a prefix?",
        "what does debug mean?",
        "is planet a noun?",
    ] {
        assert!(
            !super::wants_deliberation(s),
            "{s:?} should answer directly"
        );
    }
}

#[test]
fn wants_live_research_is_focused() {
    for d in [
        "what is the latest version?",
        "what is trending on X today?",
        "find current news about Grok",
        "look up the recent release notes",
        "search the web for breaking updates",
    ] {
        assert!(super::wants_live_research(d), "{d:?} should wake research");
    }
    for s in [
        "what is a prefix?",
        "read the current file",
        "fix the parser bug",
        "compare these local implementations",
    ] {
        assert!(
            !super::wants_live_research(s),
            "{s:?} should not wake research"
        );
    }
}

#[test]
fn obviously_tight_is_narrow() {
    for t in [
        "hi", "Thanks!", "ok", "continue", "yes", "LGTM", "got it", "perfect.",
    ] {
        assert!(super::obviously_tight(t), "{t:?} should be obviously tight");
    }
    for s in [
        "design a distributed rate limiter",
        "why does this deadlock?",
        "okay so explain the tradeoffs",
        "fix the parser bug",
    ] {
        assert!(
            !super::obviously_tight(s),
            "{s:?} must fall through to the classifier"
        );
    }
}

#[test]
fn always_skips_the_classifier() {
    let inner = Arc::new(CountingClub::new("TIGHT")); // would say TIGHT, but ignored
    let swarm = SwarmClub::with_config("swarm", inner.clone(), 2, 1, true);
    let _ = swarm.respond("anything at all").unwrap();
    // No classifier; layers=1 → 2 proposers + 1 final synthesis.
    assert_eq!(inner.count(), 3);
}

#[test]
fn all_knobs_compose() {
    // Distinct drafts so no stage is skipped by the local convergence
    // checks; keep=1 so the judge panel can actually prune (a no-op panel —
    // keep ≥ drafts — is now skipped without a call).
    let inner = Arc::new(DistinctClub::new());
    let k = Knobs {
        width: 2,
        refine_width: 2,
        layers: 2,
        always: true,
        reflect: true,
        judge: true,
        judge_panel: 1,
        dims: false,
        keep: 1,
        samples: 2,
        verify: 1,
        verify_guard: false,
        research: false,
        cite: false,
        hedge: false,
        search_url: String::new(),
        delegate: false,
        max_tests: 3,
        dissent_gate: false,
        ..Knobs::default()
    };
    let swarm = SwarmClub::with_knobs("swarm", inner.clone(), k);
    let out = swarm.respond("how should we design this?").unwrap();
    assert!(!out.is_empty());
    // 2 proposers + 1 critique + 2 aggregators + 1 judge + (2 synth + 1 chooser)
    // + (1 verify + 1 revise) = 11. (judge/chooser parses fail on the mock and
    // fall through, but the calls still happen.)
    assert_eq!(inner.count(), 11);
}

#[test]
fn noop_judge_panel_is_skipped_without_a_call() {
    // keep=0 → auto keep = max(2, len/2) = 2 with two drafts: the panel
    // couldn't prune anything, so no reviewer call is spent.
    let inner = Arc::new(DistinctClub::new());
    let k = Knobs {
        width: 2,
        layers: 1,
        always: true,
        judge: true,
        judge_panel: 3,
        ..Knobs::default()
    };
    let swarm = SwarmClub::with_knobs("swarm", inner.clone(), k);
    let out = swarm.respond("how should we design this?").unwrap();
    assert!(!out.is_empty());
    assert_eq!(
        inner.count(),
        3,
        "2 proposers + 1 synthesis — the no-op judge panel must cost zero calls"
    );
}

#[test]
fn identical_candidates_skip_the_chooser_call() {
    let _guard = crate::tests::env_lock();
    let _dedup = EnvGuard::set("ANGEL_MOA_DEDUP_SIM", "0.9");
    // Self-consistency with a mock that always answers "draft": both
    // syntheses converge, so the chooser round-trip is skipped locally.
    let inner = Arc::new(CountingClub::new("OPEN"));
    let k = Knobs {
        width: 2,
        layers: 1,
        always: true,
        samples: 2,
        ..Knobs::default()
    };
    let swarm = SwarmClub::with_knobs("swarm", inner.clone(), k);
    let out = swarm.respond("how should we design this?").unwrap();
    assert_eq!(out, "draft");
    // 2 proposers (collapse to 1 convergent draft) + 2 syntheses; the
    // chooser over two identical candidates costs nothing.
    assert_eq!(inner.count(), 4);
}

#[test]
fn verify_loop_breaks_once_the_revision_stops_changing() {
    let _guard = crate::tests::env_lock();
    let _dedup = EnvGuard::set("ANGEL_MOA_DEDUP_SIM", "0.9");
    // The mock's verifier never says OK and its reviser returns the same
    // text every round — without the local convergence check verify=3 would
    // burn 3×(verify+revise) = 6 calls; with it, one round settles it.
    let inner = Arc::new(CountingClub::new("OPEN"));
    let k = Knobs {
        width: 2,
        layers: 1,
        always: true,
        verify: 3,
        ..Knobs::default()
    };
    let swarm = SwarmClub::with_knobs("swarm", inner.clone(), k);
    let out = swarm.respond("how should we design this?").unwrap();
    assert_eq!(out, "draft");
    // 2 proposers (collapse) + 1 synthesis + 1 verify + 1 revise = 5.
    assert_eq!(inner.count(), 5);
}

// --- local near-duplicate detection --------------------------------------

#[test]
fn near_identical_matches_same_and_rejects_distinct_text() {
    let a = "Use a token bucket per client keyed by account id, refill at the plan rate, \
                 and reject with 429 plus Retry-After when the bucket is empty.";
    let b = "Use a token bucket per client keyed by account id, refill at the plan rate, \
                 and reject with 429 plus Retry-After when the bucket is empty!";
    assert!(near_identical_at(a, b, 0.9), "same text modulo punctuation");
    let c = "Shard the counter across Redis with a sliding window and a Lua script for \
                 atomic increment-and-expire; fall back to local buckets on Redis loss.";
    assert!(!near_identical_at(a, c, 0.9), "different approaches");
    // Threshold 0 disables the check outright.
    assert!(!near_identical_at(a, b, 0.0));
    // Too short to shingle → normalized whole-text equality.
    assert!(near_identical_at("ok then", " ok  then ", 0.9));
    assert!(!near_identical_at("ok then", "not really", 0.9));
}

#[test]
fn collapse_keeps_first_of_each_convergent_group() {
    let _guard = crate::tests::env_lock();
    let _dedup = EnvGuard::set("ANGEL_MOA_DEDUP_SIM", "0.9");
    let a = "Use a token bucket per client keyed by account id, refill at the plan rate, \
                  and reject with 429 plus Retry-After when the bucket is empty.";
    let a2 = format!("{a} "); // same draft, trivially different
    let c = "Shard the counter across Redis with a sliding window and a Lua script for \
                  atomic increment-and-expire; fall back to local buckets on Redis loss.";
    let out = collapse_near_duplicates(vec![a.to_string(), a2, c.to_string()]);
    assert_eq!(out.len(), 2);
    assert_eq!(out[0], a, "first of the convergent group survives");
    assert_eq!(out[1], c);
}

/// Dedup is on at the default threshold: two drafts differing only in
/// whitespace are the same answer, so one collapses away without any model
/// call. Explicit `ANGEL_MOA_DEDUP_SIM=0` keeps every draft.
#[test]
fn whitespace_variants_collapse_to_one_at_the_default_threshold() {
    let _guard = crate::tests::env_lock();
    let _dedup = EnvGuard::unset("ANGEL_MOA_DEDUP_SIM");
    assert!((dedup_similarity() - 0.92).abs() < 1e-9);
    let a = "Use a token bucket per client keyed by account id, refill at the plan rate, \
                  and reject with 429 plus Retry-After when the bucket is empty.";
    let a2 = format!("{a}  \n\t");
    let out = collapse_near_duplicates(vec![a.to_string(), a2.clone()]);
    assert_eq!(out.len(), 1, "whitespace-only variants are one answer");
    let _off = EnvGuard::set("ANGEL_MOA_DEDUP_SIM", "0");
    let out = collapse_near_duplicates(vec![a.to_string(), a2]);
    assert_eq!(out.len(), 2, "0 disables the collapse");
}

#[test]
fn moa_draft_bounding_marks_omitted_text_without_splitting_chars() {
    let draft = format!("{}{}", "α".repeat(40), "tail");
    let bounded = bound_moa_draft(&draft, 24);
    assert!(bounded.starts_with('α'));
    assert!(bounded.contains("MOA draft truncated before synthesis"));
    assert!(!bounded.contains("tail"));

    let huge = "x".repeat(7000);
    let body = agg_task_with_cap(&[huge], false, 6000);
    assert!(body.contains("### Response 1"));
    assert!(body.contains("MOA draft truncated before synthesis"));
}

/// Each draft an aggregator receives is bounded by default: a 30k-char draft
/// is tail-trimmed to the 12k ceiling with a marker (the fan-out multiplies
/// every draft by every seat, so an unbounded draft dominates input spend),
/// and an explicit `0` via env restores the unbounded behavior.
#[test]
fn moa_draft_cap_defaults_to_twelve_k_chars() {
    let _guard = crate::tests::env_lock();
    let _max = EnvGuard::unset("ANGEL_MOA_DRAFT_MAX_CHARS");
    let _sota = EnvGuard::unset("ANGEL_SOTA_MOA_DRAFT_MAX_CHARS");
    let _swarm = EnvGuard::unset("ANGEL_SWARM_DRAFT_MAX_CHARS");
    let tail = "UNIQUE_TAIL";
    let body = agg_task(&[format!("{}{tail}", "x".repeat(30_000))], false);
    assert!(
        body.contains("MOA draft truncated before synthesis"),
        "the default ceiling marks what it trimmed"
    );
    assert!(!body.contains(tail), "the tail past 12k is trimmed");
    let _off = EnvGuard::set("ANGEL_MOA_DRAFT_MAX_CHARS", "0");
    let body = agg_task(&[format!("{}{tail}", "x".repeat(30_000))], false);
    assert!(body.contains(tail), "explicit 0 = unbounded");
    assert!(!body.contains("MOA draft truncated"));
}

#[test]
fn moa_defaults_preserve_panel_work_and_shape_only_the_final_answer() {
    let _guard = crate::tests::env_lock();
    let _max = EnvGuard::unset("ANGEL_MOA_DRAFT_MAX_CHARS");
    let _sota = EnvGuard::unset("ANGEL_SOTA_MOA_DRAFT_MAX_CHARS");
    let _swarm = EnvGuard::unset("ANGEL_SWARM_DRAFT_MAX_CHARS");
    let _target = EnvGuard::unset("ANGEL_MOA_FINAL_TARGET_CHARS");
    let _aux = EnvGuard::unset("ANGEL_MOA_AUX_CONTEXT_CHARS");
    let _dedup = EnvGuard::unset("ANGEL_MOA_DEDUP_SIM");
    let tail = "UNIQUE_TAIL";
    let draft = format!("{}{tail}", "x".repeat(7000));
    let body = agg_task(&[draft], false);
    assert!(body.contains(tail));
    assert!(!body.contains("MOA draft truncated before synthesis"));
    assert!(!body.contains("Aim for at most"));
    assert!(body.contains("Do not shorten merely to meet an unstated length"));
    assert_eq!(aux_context_chars(), 24_000);
    assert_eq!(dedup_similarity(), 0.92);
}

#[test]
fn moa_quality_first_env_defaults_do_not_relax_or_compress_the_formation() {
    let _guard = crate::tests::env_lock();
    let _dissent = EnvGuard::unset("ANGEL_MOA_DISSENT_GATE");
    let _sota_dissent = EnvGuard::unset("ANGEL_SOTA_MOA_DISSENT_GATE");
    let _cluster = EnvGuard::unset("ANGEL_MOA_CLUSTER");
    let _fanin = EnvGuard::unset("ANGEL_MOA_AGG_FANIN");
    let _redirect = EnvGuard::unset("ANGEL_SOTA_MOA_USE_SWARM_KNOBS");

    let local = SwarmClub::knobs_from_env();
    let sota = SwarmClub::sota_knobs_from_env();
    for knobs in [&local, &sota] {
        assert!(!knobs.dissent_gate);
        assert!(!knobs.cluster);
        assert_eq!(knobs.agg_fanin, usize::MAX);
    }
}

#[test]
fn angle_roster_preserves_first_six_and_expands_deterministically() {
    let _guard = crate::tests::env_lock();
    assert_eq!(roster_len(), 36);
    for (idx, (key, sys)) in PERSONAS.iter().enumerate() {
        let a = angle(idx);
        assert_eq!(a.key, *key);
        assert_eq!(a.sys, *sys);
    }
    let mut pairs = std::collections::HashSet::new();
    let mut prev: Option<(String, String)> = None;
    for idx in PERSONAS.len()..roster_len() {
        let a = angle(idx);
        let mut parts = a.key.split('+');
        let base = parts.next().unwrap().to_string();
        let stance = parts.next().unwrap().to_string();
        assert!(
            pairs.insert((base.clone(), stance.clone())),
            "{idx}: {}",
            a.key
        );
        if let Some((prev_base, prev_stance)) = prev {
            assert_ne!(prev_base, base, "consecutive bases should differ");
            assert_ne!(prev_stance, stance, "consecutive stances should differ");
        }
        prev = Some((base, stance));
    }
    assert_eq!(pairs.len(), 30);
}

#[test]
fn angel_moa_personas_selects_and_orders_case_insensitively() {
    let _guard = crate::tests::env_lock();
    let key = "ANGEL_MOA_PERSONAS";
    let previous = std::env::var_os(key);
    // TODO: Audit that the environment access only happens in single-threaded code.
    unsafe { std::env::set_var(key, "red-team , SYSTEMS") };

    let roster = selected_personas();
    let keys: Vec<&str> = roster.iter().map(|(k, _)| *k).collect();
    assert_eq!(
        keys,
        vec!["red-team", "systems"],
        "case-insensitive match, trim whitespace, preserve declared order"
    );
    assert_eq!(roster_len(), 2 + 2 * STANCES.len());
    assert_eq!(angle(0).key, "red-team");
    assert_eq!(angle(1).key, "systems");

    match previous {
        // TODO: Audit that the environment access only happens in single-threaded code.
        Some(v) => unsafe { std::env::set_var(key, v) },
        // TODO: Audit that the environment access only happens in single-threaded code.
        None => unsafe { std::env::remove_var(key) },
    }
}

#[test]
fn angel_moa_personas_skips_unknown_keys_keeping_valid_ones() {
    let _guard = crate::tests::env_lock();
    let key = "ANGEL_MOA_PERSONAS";
    let previous = std::env::var_os(key);
    // TODO: Audit that the environment access only happens in single-threaded code.
    unsafe { std::env::set_var(key, "red-team,invented") };

    let roster = selected_personas();
    let keys: Vec<&str> = roster.iter().map(|(k, _)| *k).collect();
    assert_eq!(
        keys,
        vec!["red-team"],
        "unknown key skipped, valid key kept"
    );

    match previous {
        // TODO: Audit that the environment access only happens in single-threaded code.
        Some(v) => unsafe { std::env::set_var(key, v) },
        // TODO: Audit that the environment access only happens in single-threaded code.
        None => unsafe { std::env::remove_var(key) },
    }
}

#[test]
fn angel_moa_personas_all_unknown_falls_back_to_full_roster() {
    let _guard = crate::tests::env_lock();
    let key = "ANGEL_MOA_PERSONAS";
    let previous = std::env::var_os(key);
    // TODO: Audit that the environment access only happens in single-threaded code.
    unsafe { std::env::set_var(key, "nope") };

    let roster = selected_personas();
    assert_eq!(
        roster.len(),
        PERSONAS.len(),
        "nothing valid ⇒ full built-in roster fallback"
    );
    assert_eq!(
        roster_len(),
        PERSONAS.len() + PERSONAS.len() * STANCES.len()
    );

    match previous {
        // TODO: Audit that the environment access only happens in single-threaded code.
        Some(v) => unsafe { std::env::set_var(key, v) },
        // TODO: Audit that the environment access only happens in single-threaded code.
        None => unsafe { std::env::remove_var(key) },
    }
}

#[test]
fn decide_wave_stops_in_documented_order_then_expands() {
    let base = WaveInputs {
        wave_index: 0,
        max_waves: 3,
        launched_total: 4,
        max_width: 16,
        returned: 4,
        novel: 4,
        wave_size: 4,
        dissent: Some(0.8),
        prev_dissent: None,
        novelty_floor: 0.35,
        dissent_lo: 0.45,
        dissent_epsilon: 0.05,
        growth: 2.0,
    };
    assert_eq!(
        decide_wave(WaveInputs {
            launched_total: 16,
            returned: 0,
            ..base
        }),
        Decision::Stop(StopReason::MaxWidth)
    );
    assert_eq!(
        decide_wave(WaveInputs {
            wave_index: 2,
            ..base
        }),
        Decision::Stop(StopReason::MaxWaves)
    );
    assert_eq!(
        decide_wave(WaveInputs {
            returned: 0,
            ..base
        }),
        Decision::Stop(StopReason::Failed)
    );
    assert_eq!(
        decide_wave(WaveInputs { novel: 1, ..base }),
        Decision::Stop(StopReason::LowNovelty)
    );
    assert_eq!(
        decide_wave(WaveInputs {
            dissent: Some(0.2),
            ..base
        }),
        Decision::Stop(StopReason::Agree)
    );
    assert_eq!(
        decide_wave(WaveInputs {
            dissent: Some(0.62),
            prev_dissent: Some(0.60),
            ..base
        }),
        Decision::Stop(StopReason::Stable)
    );
    assert_eq!(decide_wave(base), Decision::Expand { next_wave: 8 });
    assert_eq!(next_wave_size(8, 2.0, 6), 6);
}

fn family(name: &str, n: usize) -> Vec<String> {
    (0..n)
        .map(|i| format!("{name}a {name}b {name}c {name}d unique{i}"))
        .collect()
}

#[test]
fn scale_clusters_and_gate_signals_track_pool_shape() {
    let mut dominant = family("alpha", 28);
    dominant.extend(family("bravo", 2));
    dominant.extend(family("charlie", 1));
    dominant.extend(family("delta", 1));
    let clusters = cluster_drafts(&dominant, 0.55);
    let signal = scale_signal_at(&clusters, dominant.len(), 0.67, 4.0).unwrap();
    assert_eq!(signal.gate, GateAction::Relax);
    assert!(signal.dominant >= 0.87, "{signal:?}");

    let mut balanced = Vec::new();
    for name in ["alpha", "bravo", "charlie", "delta"] {
        balanced.extend(family(name, 8));
    }
    let clusters = cluster_drafts(&balanced, 0.55);
    let signal = scale_signal_at(&clusters, balanced.len(), 0.67, 4.0).unwrap();
    assert_eq!(clusters.len(), 4);
    assert_eq!(signal.gate, GateAction::Escalate);

    let mut bimodal = family("alpha", 16);
    bimodal.extend(family("bravo", 16));
    let clusters = cluster_drafts(&bimodal, 0.55);
    let signal = scale_signal_at(&clusters, bimodal.len(), 0.67, 4.0).unwrap();
    assert_eq!(clusters.len(), 2);
    assert_eq!(signal.gate, GateAction::Escalate);
}

#[test]
fn seed_pods_are_contiguous_and_caps_are_budgeted() {
    // Serialized: this test mutates process-global environment that other
    // tests also read or write. Without the lock they interleave and each
    // observes the other's value between its own set and restore.
    let _guard = crate::tests::env_lock();
    let _min = EnvGuard::unset("ANGEL_MOA_DRAFT_MIN_CHARS");
    let _max = EnvGuard::unset("ANGEL_MOA_DRAFT_MAX_CHARS");
    let _sota = EnvGuard::unset("ANGEL_SOTA_MOA_DRAFT_MAX_CHARS");
    let _swarm = EnvGuard::unset("ANGEL_SWARM_DRAFT_MAX_CHARS");
    assert_eq!(
        seed_pods(13, 5),
        vec![vec![0, 1, 2, 3, 4], vec![5, 6, 7, 8, 9], vec![10, 11, 12]]
    );
    assert_eq!(moa_draft_max_chars(), 12_000);
    assert_eq!(per_draft_cap(4, 4_000), Some(1_000));
    assert_eq!(per_draft_cap(10, 6_000), None);
    assert_eq!(per_draft_cap(2, 20_000), Some(10_000));
    // A budget above the default ceiling still clips to the ceiling.
    assert_eq!(per_draft_cap(2, 40_000), Some(12_000));
}

// --- JUDGE panel (peer-review median scoring) ---------------------------

/// Returns a fixed score sheet for the judge prompt (recognized by `JUDGE_SYS`)
/// and a plain draft for everything else, counting every chat call.
struct ScoringClub {
    calls: Mutex<usize>,
    sheet: &'static str,
}

impl Club for ScoringClub {
    fn label(&self) -> &str {
        "scoring"
    }
    fn respond(&self, _p: &str) -> Result<String, String> {
        Ok("x".to_string())
    }
    fn chat(&self, messages: &[ChatMsg], _tools: &[ToolDef]) -> Result<ClubReply, String> {
        *self.calls.lock().unwrap() += 1;
        // The judge instruction sits in the system slot (legacy shape) or at
        // the head of the trailing task message (cache-aligned shape) — scan
        // the whole call for its marker so both shapes are recognized.
        if messages
            .iter()
            .any(|m| m.content.contains("impartial, calibrated judge"))
        {
            return Ok(ClubReply::Text(self.sheet.to_string()));
        }
        Ok(ClubReply::Text("draft".to_string()))
    }
}

#[test]
fn judge_panel_defaults_to_one_reviewer() {
    assert_eq!(Knobs::default().judge_panel, 1);
}

#[test]
fn judge_panel_one_is_a_single_reviewer() {
    let inner = Arc::new(ScoringClub {
        calls: Mutex::new(0),
        sheet: "1: 9\n2: 1\n3: 5",
    });
    let k = Knobs {
        judge: true,
        judge_panel: 1,
        ..Knobs::default()
    };
    let swarm = SwarmClub::with_knobs("swarm", inner.clone(), k);
    let kept = swarm.judge_select("", &[], vec!["a".into(), "b".into(), "c".into()]);
    // keep = ((3+1)/2).max(2) = 2; strongest are a(9) then c(5).
    assert_eq!(kept, vec!["a".to_string(), "c".to_string()]);
    assert_eq!(
        *inner.calls.lock().unwrap(),
        1,
        "panel of one = one judge call"
    );
}

#[test]
fn judge_panel_votes_and_prunes_to_top_k() {
    let inner = Arc::new(ScoringClub {
        calls: Mutex::new(0),
        sheet: "1: 2\n2: 9\n3: 8\n4: 1",
    });
    let k = Knobs {
        judge: true,
        judge_panel: 3,
        ..Knobs::default()
    };
    let swarm = SwarmClub::with_knobs("swarm", inner.clone(), k);
    let drafts = vec!["a".into(), "b".into(), "c".into(), "d".into()];
    let kept = swarm.judge_select("", &[], drafts);
    // keep = ((4+1)/2).max(2) = 2; medians rank b(9) > c(8) > a(2) > d(1).
    assert_eq!(kept, vec!["b".to_string(), "c".to_string()]);
    assert_eq!(
        *inner.calls.lock().unwrap(),
        3,
        "three reviewers, three calls"
    );
}

#[test]
fn median_of_scores() {
    assert_eq!(median(vec![5.0]), 5.0);
    assert_eq!(median(vec![1.0, 2.0, 3.0]), 2.0);
    assert_eq!(median(vec![1.0, 2.0, 3.0, 4.0]), 2.5);
    assert_eq!(median(vec![9.0, 1.0, 5.0]), 5.0); // unsorted input
}

#[test]
fn median_rank_orders_by_per_draft_median() {
    // draft0: [9,1,1] → 1 ; draft1: [5,5,6] → 5 ; draft2: [8,8] → 8
    let reviews = vec![
        vec![(0, 9.0), (1, 5.0), (2, 8.0)],
        vec![(0, 1.0), (1, 5.0), (2, 8.0)],
        vec![(0, 1.0), (1, 6.0)],
    ];
    let order: Vec<usize> = median_rank(&reviews).into_iter().map(|(i, _)| i).collect();
    assert_eq!(order, vec![2, 1, 0]);
}

#[test]
fn parse_scores_filters_range_and_dedups() {
    // Out-of-range index 9 dropped; the second score for draft 1 ignored.
    assert_eq!(
        parse_scores("1: 7\n2: 8\n9: 3\n1: 99", 3),
        vec![(0, 7.0), (1, 8.0)]
    );
}

// --- HEDGE ladder (calibrated claims) ----------------------------------

/// Records the system prompt of every chat call so we can assert what each
/// stage was told.
struct RecordingClub {
    systems: Mutex<Vec<String>>,
}

impl Club for RecordingClub {
    fn label(&self) -> &str {
        "recording"
    }
    fn respond(&self, p: &str) -> Result<String, String> {
        Ok(format!("echo:{p}"))
    }
    fn chat(&self, messages: &[ChatMsg], _tools: &[ToolDef]) -> Result<ClubReply, String> {
        let sys = messages
            .iter()
            .find(|m| m.role == ChatRole::System)
            .map(|m| m.content.to_string())
            .unwrap_or_default();
        self.systems.lock().unwrap().push(sys);
        Ok(ClubReply::Text("draft".to_string()))
    }
}

const HEDGE_MARK: &str = "Calibrate every claim's strength to its evidence";

#[test]
fn hedge_off_by_default() {
    assert!(!Knobs::default().hedge);
}

#[test]
fn hedge_folds_ladder_into_every_stage() {
    let inner = Arc::new(RecordingClub {
        systems: Mutex::new(Vec::new()),
    });
    let k = Knobs {
        width: 2,
        layers: 1,
        always: true,
        hedge: true,
        ..Knobs::default()
    };
    let swarm = SwarmClub::with_knobs("swarm", inner.clone(), k);
    let _ = swarm.respond("how should we build this?").unwrap();
    let systems = inner.systems.lock().unwrap();
    assert!(!systems.is_empty());
    assert!(
        systems.iter().all(|s| s.contains(HEDGE_MARK)),
        "every worker's system prompt should carry the hedge ladder"
    );
}

#[test]
fn hedge_off_leaves_prompts_clean() {
    let inner = Arc::new(RecordingClub {
        systems: Mutex::new(Vec::new()),
    });
    let k = Knobs {
        width: 2,
        layers: 1,
        always: true,
        ..Knobs::default()
    };
    let swarm = SwarmClub::with_knobs("swarm", inner.clone(), k);
    let _ = swarm.respond("how should we build this?").unwrap();
    let systems = inner.systems.lock().unwrap();
    assert!(!systems.is_empty());
    assert!(
        systems.iter().all(|s| !s.contains(HEDGE_MARK)),
        "the hedge ladder must not appear when HEDGE is off"
    );
}

// --- convo pipeline shape (cache alignment + bounded score context) -----

#[test]
fn stage_parts_shapes_aligned_and_legacy_calls() {
    let ctx = vec![ChatMsg::user("the problem")];
    // Aligned answer stage: the base voice holds the system slot; the
    // stage/persona instruction rides at the end as the task message.
    let (sys, msgs) = stage_parts_at(true, "You are a persona.", "BASE", &ctx, None);
    assert_eq!(sys, "BASE");
    assert_eq!(msgs.len(), 2);
    assert_eq!(msgs[1].role, ChatRole::Harness);
    assert_eq!(&*msgs[1].content, "You are a persona.");
    // Aligned score stage: no system slot at all; instruction + task fold
    // into one trailing message.
    let (sys, msgs) = stage_parts_at(
        true,
        "You are a judge.",
        "",
        &ctx,
        Some("Score them.".into()),
    );
    assert_eq!(sys, "");
    assert_eq!(msgs[1].role, ChatRole::Harness);
    assert_eq!(&*msgs[1].content, "You are a judge.\n\nScore them.");
    // Legacy shape: the stage prompt leads the system slot — byte-identical
    // to the historical behavior.
    let (sys, msgs) = stage_parts_at(
        false,
        "You are a judge.",
        "BASE",
        &ctx,
        Some("Score them.".into()),
    );
    assert_eq!(sys, "You are a judge.\n\nBASE");
    assert_eq!(msgs[1].role, ChatRole::Harness);
    assert_eq!(&*msgs[1].content, "Score them.");
}

#[test]
fn aligned_run_reuses_one_system_prefix_for_every_call() {
    // The whole point of cache alignment: every call in a deliberate turn
    // opens with the same [system][conversation] bytes, so provider prefix
    // caches hit across the fan-out, the synthesis, and future turns.
    let inner = Arc::new(RecordingClub {
        systems: Mutex::new(Vec::new()),
    });
    let swarm = SwarmClub::with_config("swarm", inner.clone(), 2, 1, true);
    let history = [
        ChatMsg::system("BASE VOICE"),
        ChatMsg::user("how should we build this?"),
    ];
    let out = swarm.chat(&history, &[]).unwrap();
    assert!(matches!(out, ClubReply::Text(t) if !t.is_empty()));
    let systems = inner.systems.lock().unwrap();
    assert!(systems.len() >= 2, "proposers + synthesis expected");
    assert!(
        systems.iter().all(|s| s == "BASE VOICE"),
        "every stage call must reuse the conversation's own system prefix: {systems:?}"
    );
}

#[test]
fn judge_reads_bounded_context_over_the_shared_system_prefix() {
    let inner = Arc::new(RecordingClub {
        systems: Mutex::new(Vec::new()),
    });
    let k = Knobs {
        judge: true,
        keep: 1,
        ..Knobs::default()
    };
    let swarm = SwarmClub::with_knobs("swarm", inner.clone(), k);
    let rest = vec![ChatMsg::user("problem statement")];
    let kept = swarm.judge_select(
        "BASE VOICE",
        &rest,
        vec!["alpha".into(), "beta".into(), "gamma".into()],
    );
    assert_eq!(kept.len(), 3, "unparseable scores keep every draft");
    let systems = inner.systems.lock().unwrap();
    assert_eq!(
        systems.as_slice(),
        ["BASE VOICE".to_string()],
        "the judge seat must reuse the conversation's own system slot so its \
         calls share the propose/aggregate cache prefix"
    );
}

/// Every seat of a turn must open with the same `[system][conversation]` bytes:
/// judge and propose `stage_parts` calls put the identical `base_sys` in the
/// system slot and the identical conversation in front, so provider prefix
/// caches hit across the fan-out and the stages.
#[test]
fn judge_and_propose_stage_parts_share_the_system_slot() {
    let ctx = vec![ChatMsg::user("the problem")];
    let (propose_sys, propose_msgs) =
        stage_parts_at(true, "You are a persona.", "BASE VOICE", &ctx, None);
    let (judge_sys, judge_msgs) = stage_parts_at(
        true,
        "You are a judge.",
        "BASE VOICE",
        &ctx,
        Some("Score them.".into()),
    );
    let (verify_sys, verify_msgs) = stage_parts_at(
        true,
        "You are a verifier.",
        "BASE VOICE",
        &ctx,
        Some("Check it.".into()),
    );
    assert_eq!(propose_sys, "BASE VOICE");
    assert_eq!(judge_sys, propose_sys);
    assert_eq!(verify_sys, propose_sys);
    // The conversation prefix ahead of each seat's trailing task message is
    // byte-identical too (same messages, same order).
    let convo = |msgs: &[ChatMsg]| -> Vec<(u8, String)> {
        msgs[..ctx.len()]
            .iter()
            .map(|m| {
                (
                    match m.role {
                        ChatRole::System => 0,
                        ChatRole::User => 1,
                        ChatRole::Harness => 2,
                        ChatRole::Assistant => 3,
                        ChatRole::Tool => 4,
                    },
                    m.content.to_string(),
                )
            })
            .collect()
    };
    assert_eq!(convo(&judge_msgs), convo(&propose_msgs));
    assert_eq!(convo(&verify_msgs), convo(&propose_msgs));
}

#[test]
fn aux_context_keeps_the_newest_messages_within_budget() {
    let rest = vec![
        ChatMsg::user("a".repeat(50)),
        ChatMsg::assistant("b".repeat(50)),
        ChatMsg::user("the actual problem"),
    ];
    // The problem and the reply before it fit; the oldest message doesn't.
    let out = aux_context_at(&rest, 80);
    assert_eq!(out.len(), 2);
    assert_eq!(&*out[0].content, "b".repeat(50));
    assert_eq!(&*out[1].content, "the actual problem");
    // 0 = full history.
    assert_eq!(aux_context_at(&rest, 0).len(), 3);
}

#[test]
fn aux_context_bounds_a_giant_final_message() {
    let marker = "END-OF-THE-ASK";
    let rest = vec![ChatMsg::user(format!("{}{marker}", "x".repeat(9000)))];
    let out = aux_context_at(&rest, 200);
    assert_eq!(out.len(), 1);
    assert!(out[0].content.chars().count() < 400);
    // The newest end of an over-long message is the part a scorer needs.
    assert!(
        out[0].content.starts_with("[context trimmed:"),
        "leading trim marker, got: {}",
        &*out[0].content
    );
    assert!(out[0].content.contains(marker), "tail is kept");
}

/// The measured failure this bounds: score-only stages re-sent the entire
/// conversation on every call, so a 100k-char turn made each judge/verify seat
/// a 100k-input-token call. The default budget keeps the newest ~24k chars
/// (plus one marker line), and `0` is the explicit full-history opt-in.
#[test]
fn aux_context_default_bounds_a_hundred_k_message() {
    let _guard = crate::tests::env_lock();
    let _aux = EnvGuard::unset("ANGEL_MOA_AUX_CONTEXT_CHARS");
    assert_eq!(aux_context_chars(), 24_000);
    let marker = "THE-FINAL-ASK";
    let rest = vec![ChatMsg::user(format!("{}{marker}", "x".repeat(100_000)))];
    let out = aux_context(&rest);
    assert_eq!(out.len(), 1);
    let len = out[0].content.chars().count();
    assert!(
        len <= 24_000 + 64,
        "bounded tail + marker line only, got {len}"
    );
    assert!(out[0].content.contains(marker), "the newest ask survives");
    assert!(out[0].content.contains("chars omitted]"));
}

// --- JUDGE_DIMS (per-dimension / weighted "LQS" scoring) ----------------

#[test]
fn dims_off_by_default() {
    assert!(!Knobs::default().dims);
}

#[test]
fn judge_dims_weights_sum_to_one() {
    let total: f64 = JUDGE_DIMS.iter().map(|(_, w)| w).sum();
    assert!(
        (total - 1.0).abs() < 1e-9,
        "dimension weights must sum to 1.0"
    );
}

#[test]
fn parse_dim_scores_weights_and_filters() {
    // correctness .40, insight .25, rigor .20, novelty .15.
    // 1: 8 6 7 5 → 3.2+1.5+1.4+.75 = 6.85
    // 2: 10 0 0 0 → 4.0 ; 9: out of range (dropped) ; 1: dup (ignored)
    // 4: 5 5 → too few dimensions (skipped)
    let got = parse_dim_scores("1: 8 6 7 5\n2: 10 0 0 0\n9: 9 9 9 9\n1: 1 1 1 1\n4: 5 5", 4);
    assert_eq!(got.len(), 2);
    assert_eq!(got[0].0, 0);
    assert!((got[0].1 - 6.85).abs() < 1e-9, "got {}", got[0].1);
    assert_eq!(got[1].0, 1);
    assert!((got[1].1 - 4.0).abs() < 1e-9, "got {}", got[1].1);
}

#[test]
fn judge_dims_scores_and_prunes_to_top_k() {
    // Composites: a=10 (all 10s), b=0, c=3.2 (correctness 8, rest 0).
    let inner = Arc::new(ScoringClub {
        calls: Mutex::new(0),
        sheet: "1: 10 10 10 10\n2: 0 0 0 0\n3: 8 0 0 0",
    });
    let k = Knobs {
        judge: true,
        dims: true,
        ..Knobs::default()
    };
    let swarm = SwarmClub::with_knobs("swarm", inner.clone(), k);
    let kept = swarm.judge_select("", &[], vec!["a".into(), "b".into(), "c".into()]);
    // keep = ((3+1)/2).max(2) = 2; strongest are a(10) then c(3.2).
    assert_eq!(kept, vec!["a".to_string(), "c".to_string()]);
}

// --- VERIFY regression guard -------------------------------------------

/// Replies by stage: the verifier always finds a problem (forcing a revise),
/// the aggregator returns a fixed revision, and the chooser returns `pick` —
/// the candidate number it deems best (1 = the prior best, 2 = the revision).
struct VerifyClub {
    revision: &'static str,
    pick: &'static str,
    chooser_calls: Mutex<usize>,
}
impl Club for VerifyClub {
    fn label(&self) -> &str {
        "verify"
    }
    fn respond(&self, _p: &str) -> Result<String, String> {
        Ok("x".to_string())
    }
    fn chat(&self, messages: &[ChatMsg], _tools: &[ToolDef]) -> Result<ClubReply, String> {
        // Stage instructions live in the system slot (legacy shape) or in the
        // trailing task message (cache-aligned shape) — scan the whole call.
        let hit = |marker: &str| messages.iter().any(|m| m.content.contains(marker));
        if hit("adversarial verifier") {
            return Ok(ClubReply::Text("Problem: be more precise.".to_string()));
        }
        if hit("select the single best") {
            *self.chooser_calls.lock().unwrap() += 1;
            return Ok(ClubReply::Text(self.pick.to_string()));
        }
        // The aggregator (revise) path.
        Ok(ClubReply::Text(self.revision.to_string()))
    }
}

#[test]
fn verify_guard_off_by_default() {
    assert!(!Knobs::default().verify_guard);
}

#[test]
fn verify_without_guard_takes_latest_revision() {
    let inner = Arc::new(VerifyClub {
        revision: "REVISED",
        pick: "1",
        chooser_calls: Mutex::new(0),
    });
    let k = Knobs {
        verify: 1,
        verify_guard: false,
        ..Knobs::default()
    };
    let swarm = SwarmClub::with_knobs("swarm", inner.clone(), k);
    let out = swarm.verify_revise("", &[], "ORIGINAL".into(), &AtomicBool::new(false));
    assert_eq!(out, "REVISED", "default path keeps the latest revision");
    assert_eq!(
        *inner.chooser_calls.lock().unwrap(),
        0,
        "no scoring call without the guard"
    );
}

#[test]
fn verify_guard_keeps_best_when_revision_loses() {
    // Chooser picks candidate 1 (the prior best) → the revision is rejected.
    let inner = Arc::new(VerifyClub {
        revision: "REVISED",
        pick: "1",
        chooser_calls: Mutex::new(0),
    });
    let k = Knobs {
        verify: 1,
        verify_guard: true,
        ..Knobs::default()
    };
    let swarm = SwarmClub::with_knobs("swarm", inner.clone(), k);
    let out = swarm.verify_revise("", &[], "ORIGINAL".into(), &AtomicBool::new(false));
    assert_eq!(out, "ORIGINAL", "a worse revision can't regress the answer");
    assert_eq!(
        *inner.chooser_calls.lock().unwrap(),
        1,
        "the guard costs one score call"
    );
}

#[test]
fn verify_guard_adopts_better_revision() {
    // Chooser picks candidate 2 (the revision) → it wins.
    let inner = Arc::new(VerifyClub {
        revision: "REVISED",
        pick: "2",
        chooser_calls: Mutex::new(0),
    });
    let k = Knobs {
        verify: 1,
        verify_guard: true,
        ..Knobs::default()
    };
    let swarm = SwarmClub::with_knobs("swarm", inner.clone(), k);
    let out = swarm.verify_revise("", &[], "ORIGINAL".into(), &AtomicBool::new(false));
    assert_eq!(out, "REVISED", "a better revision is adopted");
}

// --- CITE (citation/source verification) -------------------------------

/// Returns a fixed reply for every chat call (used to drive `cite_check`).
struct ConstClub {
    reply: &'static str,
}
impl Club for ConstClub {
    fn label(&self) -> &str {
        "const"
    }
    fn respond(&self, _p: &str) -> Result<String, String> {
        Ok(self.reply.to_string())
    }
    fn chat(&self, _m: &[ChatMsg], _t: &[ToolDef]) -> Result<ClubReply, String> {
        Ok(ClubReply::Text(self.reply.to_string()))
    }
}

#[test]
fn cite_off_by_default() {
    assert!(!Knobs::default().cite);
}

#[test]
fn cite_check_rewrites_against_sources() {
    let inner = Arc::new(ConstClub {
        reply: "CITED ANSWER",
    });
    let swarm = SwarmClub::with_knobs("swarm", inner, Knobs::default());
    let out = swarm.cite_check("", &[], "raw answer".into(), "- src one (http://x)");
    assert_eq!(out, "CITED ANSWER");
}

#[test]
fn cite_check_keeps_answer_on_empty_rewrite() {
    let inner = Arc::new(ConstClub { reply: "   " });
    let swarm = SwarmClub::with_knobs("swarm", inner, Knobs::default());
    let out = swarm.cite_check("", &[], "raw answer".into(), "- src one (http://x)");
    assert_eq!(
        out, "raw answer",
        "an empty rewrite must keep the original answer"
    );
}

/// Live end-to-end run against the real gemma4 vLLM endpoint. Ignored by
/// default (needs the DGX up); honors all ANGEL_SWARM_* knobs, so run e.g.:
///   ANGEL_SWARM_MAX=1 cargo test --bin angel swarm_live -- --ignored --nocapture
/// Override the target via ANGEL_GEMMA_URL / ANGEL_GEMMA_MODEL.
#[test]
#[ignore = "hits the live gemma4 endpoint on the DGX Spark"]
fn swarm_live_gemma() {
    use crate::club::HttpClub;
    let url = std::env::var("ANGEL_GEMMA_URL")
        .expect("set ANGEL_GEMMA_URL to an explicitly trusted live endpoint");
    let model = std::env::var("ANGEL_GEMMA_MODEL").unwrap_or_else(|_| "gemma4".to_string());
    let inner = HttpClub::new("gemma", url.clone(), model, None);
    assert!(
        inner.is_ready(),
        "gemma is not reachable at {url}; the explicit live swarm contract did not run"
    );
    let swarm = SwarmClub::from_env_with("swarm", Arc::new(inner), |_| None);
    let t = std::time::Instant::now();
    let out = swarm
        .respond(
            "A small team must choose between a monolith and microservices for a new \
                 product. Give a decisive recommendation, attacking it from multiple angles.",
        )
        .expect("swarm produced an answer");
    eprintln!(
        "\n=== SWARM ({} ms) ===\n{out}\n=== END ===\n",
        t.elapsed().as_millis()
    );
    assert!(out.len() > 100, "expected a substantive synthesized answer");
}

/// Live delegation loop: a delegator should emit a `swarm-test` block, angel
/// approves + runs it locally, and the evidence grounds the answer. Run with:
///   ANGEL_SWARM_DELEGATE=1 ANGEL_SWARM_ALWAYS=1 ANGEL_SWARM_DEBUG=1 \
///     cargo test --bin angel swarm_live_delegate -- --ignored --nocapture
#[test]
#[ignore = "hits the live gemma4 endpoint + runs a sandboxed test"]
fn swarm_live_delegate() {
    let _guard = crate::tests::env_lock();
    use crate::club::HttpClub;
    let url = std::env::var("ANGEL_GEMMA_URL")
        .expect("set ANGEL_GEMMA_URL to an explicitly trusted live endpoint");
    let model = std::env::var("ANGEL_GEMMA_MODEL").unwrap_or_else(|_| "gemma4".to_string());
    let inner = HttpClub::new("gemma", url.clone(), model, None);
    assert!(
        inner.is_ready(),
        "gemma is not reachable at {url}; the explicit live delegation contract did not run"
    );
    // TODO: Audit that the environment access only happens in single-threaded code.
    unsafe { std::env::set_var("ANGEL_SWARM_DELEGATE", "1") };
    // TODO: Audit that the environment access only happens in single-threaded code.
    unsafe { std::env::set_var("ANGEL_SWARM_ALWAYS", "1") };
    let swarm = SwarmClub::from_env_with("swarm", Arc::new(inner), |_| None);
    let out = swarm
        .respond(
            "How many logical CPU cores does this machine have, and is that enough to run a \
                 4-wide agent swarm comfortably? Verify the core count empirically first.",
        )
        .expect("swarm produced an answer");
    eprintln!("\n=== DELEGATE OUTPUT ===\n{out}\n=== END ===\n");
    assert!(out.len() > 50);
}

// -----------------------------------------------------------------------
// Dissent gate + MoA ledger
// -----------------------------------------------------------------------

#[test]
fn draft_similarity_and_dissent_track_vocabulary_overlap() {
    let same = "the reactor design should favor passive cooling loops".to_string();
    let reworded = "passive cooling loops should favor the reactor design".to_string();
    let divergent = "invest everything into modular turbine manufacturing lines".to_string();

    assert_eq!(draft_similarity(&same, &same), 1.0);
    // Same conclusion, different word order: identical content vocabulary.
    assert_eq!(draft_similarity(&same, &reworded), 1.0);
    // Disjoint content vocabulary → no overlap at all.
    assert_eq!(draft_similarity(&same, &divergent), 0.0);

    assert_eq!(
        draft_dissent(std::slice::from_ref(&same)),
        None,
        "one draft can't disagree"
    );
    assert_eq!(draft_dissent(&[same.clone(), reworded]), Some(0.0));
    assert_eq!(draft_dissent(&[same, divergent]), Some(1.0));
}

#[test]
fn contradictory_drafts_do_not_read_as_agreement() {
    // Pure vocabulary overlap scored these pairs at similarity 1.0 — perfect
    // agreement — because every negation that flips the conclusion is either
    // under the 4-char content-word floor (`not`) or mangled by the alphanumeric
    // split (`isn't` → `isn` + `t`). The gate then *relaxed* the pipeline on
    // precisely the turns where the proposers had reached opposite conclusions.
    let contradictions = [
        (
            "We should ship the change.",
            "We should not ship the change.",
        ),
        (
            "This approach is safe and will work correctly under concurrent load.",
            "This approach is not safe and will not work correctly under concurrent load.",
        ),
        (
            "The connection pool is the bottleneck; raising its size reduces tail latency.",
            "The connection pool isn't the bottleneck; raising its size doesn't reduce tail latency.",
        ),
    ];
    for (affirm, deny) in contradictions {
        let sim = draft_similarity(affirm, deny);
        assert!(
            sim < 0.5,
            "contradictory drafts must not read as agreement (got {sim:.3}): {affirm:?} vs {deny:?}"
        );
        let dissent = draft_dissent(&[affirm.to_string(), deny.to_string()]).expect("two drafts");
        assert!(
            dissent > dissent_lo(),
            "a flipped conclusion must not fall in the relax band (dissent {dissent:.3})"
        );
    }
}

#[test]
fn polarity_adjustment_only_ever_raises_dissent() {
    // The correction is fail-safe by construction: it can escalate verification
    // but never skip verification the unadjusted metric would have triggered.
    // Agreeing drafts that share polarity keep scoring as agreement.
    let a = "the reactor design should favor passive cooling loops";
    let b = "passive cooling loops should favor the reactor design";
    assert_eq!(draft_similarity(a, b), 1.0, "no penalty without a gap");
    // Both negating equally → still agreement, not a spurious escalation.
    let neg_a = "the reactor design should not favor passive cooling loops";
    let neg_b = "passive cooling loops should not favor the reactor design";
    assert_eq!(
        draft_similarity(neg_a, neg_b),
        1.0,
        "matched polarity is agreement"
    );
    // A long draft with one incidental negation is nudged, not flipped.
    let long_plain = "the pipeline stages each contribute measurable latency and the aggregate \
                      budget determines whether the service meets its target under load"
        .to_string();
    let long_one_neg = format!("{long_plain} although this is not the only factor");
    let sim = draft_similarity(&long_plain, &long_one_neg);
    assert!(
        sim > 0.5,
        "one incidental negation in a long draft is not a contradiction (got {sim:.3})"
    );
}

#[test]
fn gate_knobs_at_escalates_relaxes_and_holds() {
    let base = Knobs {
        layers: 2,
        reflect: true,
        judge: false,
        samples: 2,
        verify: 0,
        ..Knobs::default()
    };

    // High dissent buys scrutiny: judge on, a verify round granted.
    let (k, action) = gate_knobs_at(&base, Some(0.9), 0.7, 0.45, 1);
    assert_eq!(action, Some(GateAction::Escalate));
    assert!(k.judge);
    assert_eq!(k.verify, 1);
    // Escalation never takes away configured rounds.
    let configured = Knobs {
        verify: 3,
        ..base.clone()
    };
    let (k, _) = gate_knobs_at(&configured, Some(0.9), 0.7, 0.45, 1);
    assert_eq!(k.verify, 3);

    // Low dissent releases the optional stages.
    let heavy = Knobs {
        layers: 3,
        reflect: true,
        judge: true,
        samples: 3,
        verify: 2,
        ..Knobs::default()
    };
    let (k, action) = gate_knobs_at(&heavy, Some(0.2), 0.7, 0.45, 1);
    assert_eq!(action, Some(GateAction::Relax));
    assert_eq!(k.layers, 1);
    assert!(!k.reflect);
    assert!(!k.judge);
    assert_eq!(k.samples, 1);
    assert_eq!(k.verify, 0);

    // Between the thresholds: hands off.
    let (k, action) = gate_knobs_at(&heavy, Some(0.55), 0.7, 0.45, 1);
    assert_eq!(action, Some(GateAction::Hold));
    assert_eq!(k.layers, 3);
    assert!(k.judge);

    // No dissent measured (gate off / single draft): untouched, no action.
    let (k, action) = gate_knobs_at(&heavy, None, 0.7, 0.45, 1);
    assert_eq!(action, None);
    assert_eq!(k.samples, 3);

    // Misconfigured lo > hi: escalate wins (over-scrutiny is the safe failure).
    let (_, action) = gate_knobs_at(&base, Some(0.5), 0.4, 0.6, 1);
    assert_eq!(action, Some(GateAction::Escalate));
}

/// A mock whose two proposers answer with disjoint vocabularies (dissent 1.0)
/// and which recognizes the downstream stages by their marker text — scanned
/// across ALL messages, so it works in aligned and legacy shapes alike.
struct DissentClub {
    proposer_calls: Mutex<usize>,
    verify_calls: Mutex<usize>,
    transcripts: Mutex<Vec<String>>,
    agree: bool,
}

impl DissentClub {
    fn new(agree: bool) -> Self {
        Self {
            proposer_calls: Mutex::new(0),
            verify_calls: Mutex::new(0),
            transcripts: Mutex::new(Vec::new()),
            agree,
        }
    }
    fn verify_count(&self) -> usize {
        *self.verify_calls.lock().unwrap()
    }
    fn saw_marker(&self, marker: &str) -> bool {
        self.transcripts
            .lock()
            .unwrap()
            .iter()
            .any(|t| t.contains(marker))
    }
}

impl Club for DissentClub {
    fn label(&self) -> &str {
        "dissent-mock"
    }
    fn respond(&self, p: &str) -> Result<String, String> {
        Ok(format!("echo:{p}"))
    }
    fn chat(&self, messages: &[ChatMsg], _tools: &[ToolDef]) -> Result<ClubReply, String> {
        let joined = messages
            .iter()
            .map(|m| m.content.as_ref())
            .collect::<Vec<_>>()
            .join("\n");
        self.transcripts.lock().unwrap().push(joined.clone());
        let hit = |marker: &str| joined.contains(marker);
        if hit("adversarial verifier") {
            *self.verify_calls.lock().unwrap() += 1;
            return Ok(ClubReply::Text("OK".to_string()));
        }
        if hit("impartial, calibrated judge") {
            return Ok(ClubReply::Text("1: 8\n2: 7".to_string()));
        }
        if hit("Synthesize the single strongest answer") || hit("Merge the responses below") {
            return Ok(ClubReply::Text("the synthesized final answer".to_string()));
        }
        // A proposer. Agreeing drafts share every content word; diverging
        // drafts share none.
        let mut n = self.proposer_calls.lock().unwrap();
        *n += 1;
        let text = if self.agree || *n % 2 == 1 {
            "alpha bravo charlie delta echo foxtrot"
        } else {
            "golf hotel india juliet kilo lima november"
        };
        Ok(ClubReply::Text(text.to_string()))
    }
}

#[test]
fn high_dissent_turn_buys_a_verify_round() {
    let inner = Arc::new(DissentClub::new(false));
    let k = Knobs {
        width: 2,
        layers: 1,
        always: true,
        dissent_gate: true,
        ..Knobs::default()
    };
    let swarm = SwarmClub::with_knobs("swarm", inner.clone(), k);
    let out = swarm.respond("which architecture should we pick?").unwrap();
    assert_eq!(out, "the synthesized final answer");
    // verify=0 configured, but total disagreement escalated: one round ran.
    assert_eq!(inner.verify_count(), 1);
}

#[test]
fn gate_off_leaves_a_high_dissent_turn_unescalated() {
    let inner = Arc::new(DissentClub::new(false));
    let k = Knobs {
        width: 2,
        layers: 1,
        always: true,
        dissent_gate: false,
        ..Knobs::default()
    };
    let swarm = SwarmClub::with_knobs("swarm", inner.clone(), k);
    swarm.respond("which architecture should we pick?").unwrap();
    assert_eq!(inner.verify_count(), 0);
}

#[test]
fn agreeing_drafts_relax_the_configured_pipeline() {
    let inner = Arc::new(DissentClub::new(true));
    // A deliberately heavy configuration: without the gate this would run
    // reflect + a refine layer + judge + 2 synthesis samples + 2 verify rounds.
    let k = Knobs {
        width: 2,
        layers: 2,
        always: true,
        reflect: true,
        judge: true,
        keep: 1,
        samples: 2,
        verify: 2,
        dissent_gate: true,
        ..Knobs::default()
    };
    let swarm = SwarmClub::with_knobs("swarm", inner.clone(), k);
    let out = swarm.respond("which architecture should we pick?").unwrap();
    // Identical drafts (dissent 0.0) relax everything optional; dedup then
    // collapses them to one, and the synthesis still runs so the user gets a
    // polished answer rather than raw proposer bullets.
    assert_eq!(out, "the synthesized final answer");
    assert_eq!(inner.verify_count(), 0);
    assert!(!inner.saw_marker("impartial, calibrated judge"));
    assert!(!inner.saw_marker("rigorous critic"));
    // 2 proposers + 1 synthesis and nothing else.
    assert_eq!(inner.transcripts.lock().unwrap().len(), 3);
}

#[test]
fn ledger_turn_record_and_summary_carry_the_turn_economics() {
    let trace = ledger::TurnTrace {
        route: "deliberate",
        dissent: Some(0.82),
        gate: Some("escalate"),
        proposed: 2,
        kept: 2,
        effective: Some((1, true, 1, 1)),
        ..Default::default()
    };
    let tokens = vec![
        ("deepseek".to_string(), 12_000u64, 800u64),
        ("longcat".to_string(), 30_000u64, 2_500u64),
    ];
    let rec = ledger::turn_record("sota-moa", &trace, 4200, true, &tokens);
    assert_eq!(rec["route"], "deliberate");
    assert_eq!(rec["gate"], "escalate");
    assert!(rec["formation"].is_null(), "no formation owns this turn");

    let mustered = ledger::TurnTrace {
        route: "deliberate",
        formation: Some("Full Muster".to_string()),
        ..Default::default()
    };
    let rec3 = ledger::turn_record("sota-moa", &mustered, 5100, true, &[]);
    assert_eq!(
        rec3["formation"], "Full Muster",
        "an engaged formation's turn carries its name"
    );
    assert_eq!(rec["drafts"]["proposed"], 2);
    assert_eq!(rec["knobs"]["verify"], 1);
    assert_eq!(rec["tokens"]["longcat"]["in"], 30_000);

    let direct = ledger::TurnTrace {
        route: "direct",
        ..Default::default()
    };
    let rec2 = ledger::turn_record("sota-moa", &direct, 900, true, &[]);
    assert!(rec2["dissent"].is_null());
    assert!(rec2["knobs"].is_null());

    let summary = ledger::summarize(&[rec, rec2]);
    assert!(summary.contains("2 turn(s)"), "{summary}");
    assert!(summary.contains("ok 2/2"), "{summary}");
    assert!(summary.contains("deliberate 1"), "{summary}");
    assert!(summary.contains("direct 1"), "{summary}");
    assert!(summary.contains("escalate 1"), "{summary}");
    assert!(summary.contains("mean 0.82"), "{summary}");
    assert!(summary.contains("longcat"), "{summary}");
    assert!(summary.contains("30.0k / 2500"), "{summary}");
}

#[test]
fn ledger_token_report_exposes_latest_turn_and_session_totals() {
    let trace = ledger::TurnTrace {
        route: "deliberate",
        ..Default::default()
    };
    let first = ledger::turn_record(
        "sota-moa",
        &trace,
        1000,
        true,
        &[
            ("deepseek".to_string(), 100, 20),
            ("longcat".to_string(), 500, 50),
        ],
    );
    let second = ledger::turn_record(
        "sota-moa",
        &trace,
        2400,
        true,
        &[
            ("deepseek".to_string(), 300, 30),
            ("openai".to_string(), 700, 70),
        ],
    );
    let report = ledger::token_report_from_records(&[first, second]).expect("token report");
    assert_eq!(report.turns, 2);
    assert_eq!(report.latest_route, "deliberate");
    assert_eq!(report.latest_ms, 2400);
    assert_eq!(report.rows[0].label, "openai");
    assert_eq!(report.rows[0].last_total(), 770);
    let deepseek = report
        .rows
        .iter()
        .find(|row| row.label == "deepseek")
        .expect("deepseek row");
    assert_eq!(deepseek.last_input, 300);
    assert_eq!(deepseek.total_input, 400);
    let longcat = report
        .rows
        .iter()
        .find(|row| row.label == "longcat")
        .expect("longcat row");
    assert_eq!(longcat.last_total(), 0);
    assert_eq!(longcat.session_total(), 550);
}

#[test]
fn ledger_usage_delta_subtracts_snapshots_and_drops_idle_clubs() {
    let before = vec![
        ("deepseek".to_string(), 100u64, 50u64),
        ("longcat".to_string(), 900u64, 200u64),
    ];
    let after = vec![
        ("deepseek".to_string(), 150u64, 70u64),
        ("longcat".to_string(), 900u64, 200u64),
        ("cerebras".to_string(), 40u64, 10u64),
    ];
    let delta = ledger::usage_delta(&before, &after);
    assert_eq!(
        delta,
        vec![
            ("deepseek".to_string(), 50, 20),
            ("cerebras".to_string(), 40, 10),
        ],
        "idle longcat dropped; unseen-before cerebras counted from zero"
    );
}

#[test]
fn sota_seat_efforts_parse_from_env_and_default_off() {
    let _guard = crate::tests::env_lock();
    // Scrub the knob-redirect too: under USE_SWARM_KNOBS the sota builder
    // returns swarm knobs, whose seat efforts are always default-off.
    let keys = [
        "ANGEL_SOTA_MOA_PROPOSE_EFFORT",
        "ANGEL_SOTA_MOA_JUDGE_EFFORT",
        "ANGEL_SOTA_MOA_VERIFY_EFFORT",
        "ANGEL_SOTA_MOA_AGG_EFFORT",
        "ANGEL_SOTA_MOA_USE_SWARM_KNOBS",
    ];
    let previous = keys.map(std::env::var_os);
    for key in keys {
        // TODO: Audit that the environment access only happens in single-threaded code.
        unsafe { std::env::remove_var(key) };
    }
    let unset = SwarmClub::sota_knobs_from_env().seat_efforts;
    assert!(unset.propose.is_none(), "seat efforts default off");
    assert!(unset.judge.is_none());
    assert!(unset.verify.is_none());
    assert!(unset.aggregate.is_none());

    // TODO: Audit that the environment access only happens in single-threaded code.
    unsafe { std::env::set_var(keys[0], "LOW") };
    // TODO: Audit that the environment access only happens in single-threaded code.
    unsafe { std::env::set_var(keys[1], " none ") };
    // TODO: Audit that the environment access only happens in single-threaded code.
    unsafe { std::env::set_var(keys[2], "") };
    // TODO: Audit that the environment access only happens in single-threaded code.
    unsafe { std::env::set_var(keys[3], "high") };
    let set = SwarmClub::sota_knobs_from_env().seat_efforts;
    assert_eq!(set.propose.as_deref(), Some("low"), "trim + lowercase");
    assert_eq!(set.judge.as_deref(), Some("none"));
    assert!(set.verify.is_none(), "empty value stays unset");
    assert_eq!(set.aggregate.as_deref(), Some("high"));
    for (key, value) in keys.into_iter().zip(previous) {
        match value {
            // TODO: Audit that the environment access only happens in single-threaded code.
            Some(value) => unsafe { std::env::set_var(key, value) },
            // TODO: Audit that the environment access only happens in single-threaded code.
            None => unsafe { std::env::remove_var(key) },
        }
    }
}

#[test]
fn role_effort_overrides_merge_over_env_seat_policy() {
    let club: Arc<dyn Club> = Arc::new(CountingClub::new("OPEN"));
    let k = Knobs {
        seat_efforts: SeatEfforts {
            propose: Some("low".to_string()),
            judge: None,
            verify: Some("none".to_string()),
            aggregate: Some("medium".to_string()),
        },
        ..Knobs::default()
    };
    let swarm = SwarmClub {
        name: "sota-moa".to_string(),
        clubs: RoleClubs {
            propose: club.clone(),
            propose_extra: Vec::new(),
            judge: club.clone(),
            judge_extra: Vec::new(),
            verify: club.clone(),
            verify_extra: Vec::new(),
            aggregate: club.clone(),
            aggregate_extra: Vec::new(),
            research: None,
        },
        k,
        fallbacks: Vec::new(),
        tool_preflight: Default::default(),
    }
    .with_role_effort_overrides(SeatEfforts {
        propose: None,
        judge: Some("high".to_string()),
        verify: None,
        aggregate: Some("high".to_string()),
    });
    let seats = &swarm.k.seat_efforts;
    assert_eq!(seats.propose.as_deref(), Some("low"), "None keeps env");
    assert_eq!(
        seats.judge.as_deref(),
        Some("high"),
        "Some fills empty seat"
    );
    assert_eq!(seats.verify.as_deref(), Some("none"), "None keeps env");
    assert_eq!(
        seats.aggregate.as_deref(),
        Some("high"),
        "Some wins over env"
    );
}
