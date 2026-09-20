use super::*;
use std::collections::VecDeque;
use std::sync::Mutex;

#[test]
fn knobs_have_protocol_defaults() {
    let k = Knobs::default();
    assert_eq!(k.rounds, 6);
    assert_eq!(k.pivot, 2);
    assert_eq!(k.stall_stop, 0);
    assert_eq!(k.min_findings, 0);
    assert!(k.state_dir.is_empty());
}

/// Returns the next scripted worker output per iteration (repeating the last
/// when the script runs dry, to simulate a stall), and a fixed synthesis. Also
/// records every iteration prompt so tests can assert on the injected state.
struct ScriptClub {
    iters: Mutex<VecDeque<String>>,
    last: Mutex<String>,
    prompts: Mutex<Vec<String>>,
    worker_calls: Mutex<usize>,
}
impl ScriptClub {
    fn new(iters: &[&str]) -> Arc<Self> {
        Arc::new(Self {
            iters: Mutex::new(iters.iter().map(|s| s.to_string()).collect()),
            last: Mutex::new(String::new()),
            prompts: Mutex::new(Vec::new()),
            worker_calls: Mutex::new(0),
        })
    }
}
impl Club for ScriptClub {
    fn label(&self) -> &str {
        "script"
    }
    fn respond(&self, _p: &str) -> Result<String, String> {
        Ok("x".to_string())
    }
    fn chat(&self, messages: &[ChatMsg], _t: &[ToolDef]) -> Result<ClubReply, String> {
        let sys = messages
            .iter()
            .find(|m| m.role == ChatRole::System)
            .map(|m| m.content.as_ref())
            .unwrap_or("");
        if sys.starts_with("You synthesize the accumulated") {
            return Ok(ClubReply::Text("SYNTHESIZED".to_string()));
        }
        // Worker iteration.
        *self.worker_calls.lock().unwrap() += 1;
        if let Some(u) = messages.iter().rev().find(|m| m.role == ChatRole::User) {
            self.prompts.lock().unwrap().push(u.content.to_string());
        }
        let mut q = self.iters.lock().unwrap();
        let out = q
            .pop_front()
            .unwrap_or_else(|| self.last.lock().unwrap().clone());
        *self.last.lock().unwrap() = out.clone();
        Ok(ClubReply::Text(out))
    }
}

/// A raw block: bullets exactly as given, no evidence tags added. Use when
/// the test is *about* citation quality.
fn block(dir: &str, findings: &[&str]) -> String {
    let mut s = format!("DIRECTION: {dir}\nFINDINGS:\n");
    for f in findings {
        s.push_str(&format!("- {f}\n"));
    }
    s
}

/// A compliant block for deli's *reasoning* regime: every finding carries a
/// `premise:` tag, the kind a tool-less worker can honestly supply. Use for
/// tests about loop *mechanics* (accumulation, stall, pivot, persistence)
/// rather than citation policy.
fn block_ev(dir: &str, findings: &[&str]) -> String {
    let mut s = format!("DIRECTION: {dir}\nFINDINGS:\n");
    for f in findings {
        s.push_str(&format!(
            "- {f} [evidence: premise:the problem statement]\n"
        ));
    }
    s
}

#[test]
fn iterate_accumulates_distinct_findings() {
    let inner = ScriptClub::new(&[
        &block_ev("a", &["f1", "f2"]),
        &block_ev("b", &["f3"]),
        &block_ev("c", &["f4", "f5"]),
    ]);
    let k = Knobs {
        rounds: 3,
        ..Knobs::default()
    };
    let deli = DeliClub::with_knobs("deli", inner.clone(), k);
    let st = deli.iterate("solve it", "", &AtomicBool::new(false));
    assert_eq!(st.iteration, 3);
    assert_eq!(st.findings.len(), 5, "all distinct findings kept");
    assert_eq!(st.directions_tried, vec!["a", "b", "c"]);
    assert_eq!(st.stale_count, 0);
    assert_eq!(*inner.worker_calls.lock().unwrap(), 3);
}

#[test]
fn default_deli_keeps_working_after_repeated_findings() {
    let _guard = crate::tests::env_lock();
    let _rounds = crate::tests::TestEnvGuard::unset("ANGEL_DELI_ROUNDS");
    let _stall = crate::tests::TestEnvGuard::unset("ANGEL_DELI_STALL_STOP");
    let _findings = crate::tests::TestEnvGuard::unset("ANGEL_DELI_MIN_FINDINGS");
    let _state = crate::tests::TestEnvGuard::unset("ANGEL_DELI_STATE_DIR");
    let inner = ScriptClub::new(&[&block_ev("a", &["same"])]);
    let deli = DeliClub::from_env("deli", inner.clone());
    let st = deli.iterate("solve it", "", &AtomicBool::new(false));
    assert_eq!(
        st.iteration, 6,
        "repetition must not shorten the chosen round count"
    );
    assert_eq!(st.stale_count, 5);
    assert_eq!(
        st.findings.len(),
        1,
        "repetition still earns no new evidence"
    );
}

#[test]
fn iterate_detects_stall_and_stops() {
    // Every iteration returns the same single finding → no new ground.
    let inner = ScriptClub::new(&[&block_ev("a", &["same"])]);
    let k = Knobs {
        rounds: 10,
        pivot: 99, // never pivot — isolate the stall-stop
        stall_stop: 2,
        ..Knobs::default()
    };
    let deli = DeliClub::with_knobs("deli", inner.clone(), k);
    let st = deli.iterate("solve it", "", &AtomicBool::new(false));
    // round0: "same" is new (stale 0); round1: 0 new (stale 1); round2: 0 new
    // (stale 2 == stall_stop → break). Three iterations, one finding.
    assert_eq!(st.iteration, 3);
    assert_eq!(st.stale_count, 2);
    assert_eq!(st.findings.len(), 1);
}

#[test]
fn iterate_forces_pivot_after_stalling() {
    let inner = ScriptClub::new(&[&block_ev("a", &["same"])]);
    let k = Knobs {
        rounds: 4,
        pivot: 2,
        stall_stop: 0, // disabled, so we run the full 4 rounds
        ..Knobs::default()
    };
    let deli = DeliClub::with_knobs("deli", inner.clone(), k);
    let _ = deli.iterate("solve it", "", &AtomicBool::new(false));
    let prompts = inner.prompts.lock().unwrap();
    assert_eq!(prompts.len(), 4);
    // Stale climbs 0,1,2,3 over rounds; the pivot fires when stale >= 2, i.e.
    // the 4th iteration (index 3). The first never pivots.
    assert!(!prompts[0].contains("PIVOT"), "round 0 must not pivot");
    assert!(
        prompts.iter().any(|p| p.contains("PIVOT")),
        "a structural pivot must be injected once stalled"
    );
}

#[test]
fn iterate_stops_at_min_findings() {
    let inner = ScriptClub::new(&[
        &block_ev("a", &["f1"]),
        &block_ev("b", &["f2"]),
        &block_ev("c", &["f3"]),
    ]);
    let k = Knobs {
        rounds: 10,
        min_findings: 2,
        ..Knobs::default()
    };
    let deli = DeliClub::with_knobs("deli", inner.clone(), k);
    let st = deli.iterate("solve it", "", &AtomicBool::new(false));
    assert_eq!(st.iteration, 2, "stops as soon as the findings goal is met");
    assert_eq!(st.findings.len(), 2);
}

#[test]
fn run_iterates_then_synthesizes() {
    let inner = ScriptClub::new(&[&block_ev("a", &["f1"]), &block_ev("b", &["f2"])]);
    let k = Knobs {
        rounds: 2,
        ..Knobs::default()
    };
    let deli = DeliClub::with_knobs("deli", inner, k);
    let out = deli.respond("solve it").unwrap();
    assert_eq!(out, "SYNTHESIZED");
}

#[test]
fn split_system_merges_system_messages_and_keeps_the_rest() {
    let hist = [
        ChatMsg::system("rule one"),
        ChatMsg::user("do it"),
        ChatMsg::system("rule two"),
        ChatMsg::assistant("ok"),
    ];
    let (sys, rest) = split_system(&hist);
    assert_eq!(sys, "rule one\n\nrule two");
    assert_eq!(rest.len(), 2);
    assert_eq!(&*rest[0].content, "do it");
    assert_eq!(&*rest[1].content, "ok");
}

#[test]
fn compose_prepends_primary_and_folds_base() {
    assert_eq!(compose("PRIMARY", ""), "PRIMARY");
    assert_eq!(compose("PRIMARY", "   "), "PRIMARY"); // whitespace base ignored
    assert_eq!(compose("PRIMARY", "extra"), "PRIMARY\n\nextra");
}

#[test]
fn persist_writes_protocol_state_files() {
    let dir = std::env::temp_dir().join(format!("deli_persist_{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    let workspace = dir.join("workspace");
    crate::experience::note_turn_workspace(&workspace);
    let inner = ScriptClub::new(&[&block_ev("a", &["f1"]), &block_ev("b", &["f2"])]);
    let k = Knobs {
        rounds: 2,
        state_dir: dir.to_string_lossy().into_owned(),
        ..Knobs::default()
    };
    let deli = DeliClub::with_knobs("deli", inner, k);
    let st = deli.iterate("solve it", "", &AtomicBool::new(false));
    assert_eq!(st.findings.len(), 2);

    let state = dir
        .join(crate::workspace_store::repo_identity(&workspace).key)
        .join("state");
    let progress: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(state.join("progress.json")).unwrap())
            .unwrap();
    assert_eq!(progress["iteration"], 2);
    assert_eq!(progress["total_findings"], 2);
    assert_eq!(progress["status"], "running");
    // directions_tried is a JSON array of the two directions.
    let dirs: Vec<String> = serde_json::from_str(
        &std::fs::read_to_string(state.join("directions_tried.json")).unwrap(),
    )
    .unwrap();
    assert_eq!(dirs, vec!["a".to_string(), "b".to_string()]);
    // findings.jsonl has one object per finding, citation included so an
    // external watchdog can audit the evidence, not just the claim.
    let findings = std::fs::read_to_string(state.join("findings.jsonl")).unwrap();
    assert_eq!(findings.lines().count(), 2);
    assert!(
        findings.contains("\\\"finding\\\":\\\"f1 [evidence:")
            || findings.contains("\"finding\":\"f1 [evidence:")
    );
    // Open leads get their own spool alongside the findings.
    assert!(state.join("hypotheses.jsonl").exists());
    // iteration_log.jsonl has one line per round.
    let log = std::fs::read_to_string(state.join("iteration_log.jsonl")).unwrap();
    assert_eq!(log.lines().count(), 2);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn persist_marks_status_stuck_once_stalled_out() {
    let dir = std::env::temp_dir().join(format!("deli_stuck_{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    let workspace = dir.join("workspace");
    crate::experience::note_turn_workspace(&workspace);
    let inner = ScriptClub::new(&[&block_ev("a", &["same"])]); // then repeats → stalls
    let k = Knobs {
        rounds: 10,
        pivot: 99,
        stall_stop: 2,
        state_dir: dir.to_string_lossy().into_owned(),
        ..Knobs::default()
    };
    let deli = DeliClub::with_knobs("deli", inner, k);
    deli.iterate("solve it", "", &AtomicBool::new(false));
    let progress: serde_json::Value = serde_json::from_str(
        &std::fs::read_to_string(
            dir.join(crate::workspace_store::repo_identity(&workspace).key)
                .join("state")
                .join("progress.json"),
        )
        .unwrap(),
    )
    .unwrap();
    assert_eq!(progress["status"], "stuck");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn cancel_before_first_round_yields_empty_state() {
    let inner = ScriptClub::new(&[&block("a", &["f1"])]);
    let deli = DeliClub::with_knobs("deli", inner.clone(), Knobs::default());
    let cancel = AtomicBool::new(true);
    let st = deli.iterate("solve it", "", &cancel);
    assert_eq!(st.iteration, 0, "cancel short-circuits before any work");
    assert!(st.findings.is_empty());
    assert_eq!(*inner.worker_calls.lock().unwrap(), 0);
}

#[test]
fn synthesize_falls_back_to_direct_pass_when_no_findings() {
    // Nothing surfaced at all → synthesize does a single direct pass through
    // the synth system prompt (ScriptClub recognizes it → "SYNTHESIZED").
    let inner = ScriptClub::new(&[]);
    let deli = DeliClub::with_knobs("deli", inner, Knobs::default());
    let out = deli.synthesize("", "the problem", &[], &[]).unwrap();
    assert_eq!(out, "SYNTHESIZED");
}

#[test]
fn synthesize_uses_open_leads_when_nothing_could_be_evidenced() {
    // A run that raised leads but evidenced none must still synthesize from
    // them. Falling through to a cold direct pass would discard the loop.
    let inner = ScriptClub::new(&[]);
    let deli = DeliClub::with_knobs("deli", inner.clone(), Knobs::default());
    let _ = deli
        .synthesize("", "the problem", &[], &["the allocator may be hot".into()])
        .unwrap();
    let prompts = inner.prompts.lock().unwrap();
    assert!(
        prompts.is_empty(),
        "synth prompt goes down the synth branch, not the worker branch"
    );
}

#[test]
fn synthesis_prompt_separates_evidenced_findings_from_open_leads() {
    let inner = ScriptClub::new(&[]);
    let deli = DeliClub::with_knobs("deli", inner, Knobs::default());
    // Reach the message builder directly by asserting on its shape via a
    // club that captures the synth user message.
    let captured = Arc::new(Mutex::new(String::new()));
    struct Capture(Arc<Mutex<String>>);
    impl Club for Capture {
        fn label(&self) -> &str {
            "cap"
        }
        fn respond(&self, _p: &str) -> Result<String, String> {
            Ok(String::new())
        }
        fn chat(&self, messages: &[ChatMsg], _t: &[ToolDef]) -> Result<ClubReply, String> {
            if let Some(u) = messages.iter().rev().find(|m| m.role == ChatRole::User) {
                *self.0.lock().unwrap() = u.content.to_string();
            }
            Ok(ClubReply::Text("ok".into()))
        }
    }
    let deli2 = DeliClub::with_knobs(
        "deli",
        Arc::new(Capture(captured.clone())),
        Knobs::default(),
    );
    let _ = deli2
        .synthesize(
            "",
            "the problem",
            &["settled [evidence: premise:the stated goal]".into()],
            &["speculative lead".into()],
        )
        .unwrap();
    let msg = captured.lock().unwrap().clone();
    assert!(msg.contains("Evidenced findings"));
    assert!(msg.contains("Open leads"));
    assert!(msg.contains("NOT evidenced"));
    assert!(msg.contains("never state an open lead as fact"));
    let _ = deli;
}

#[test]
fn streaming_run_emits_the_final_answer_as_one_delta() {
    let inner = ScriptClub::new(&[&block_ev("a", &["f1"])]);
    let k = Knobs {
        rounds: 1,
        ..Knobs::default()
    };
    let deli = DeliClub::with_knobs("deli", inner, k);
    let mut chunks: Vec<String> = Vec::new();
    let out = deli
        .run(
            &[ChatMsg::user("solve it")],
            &AtomicBool::new(false),
            &mut |d| {
                if let StreamDelta::Content(c) = d {
                    chunks.push(c.to_string());
                }
            },
            true,
        )
        .unwrap();
    assert_eq!(out, "SYNTHESIZED");
    assert_eq!(chunks, vec!["SYNTHESIZED".to_string()]);
}

#[test]
fn deli_keeps_rl_tools_available_without_restarting_deliberation_after_a_tool_result() {
    use crate::club::ToolCall;
    use std::sync::atomic::AtomicUsize;
    struct Actions {
        rounds: AtomicUsize,
        actions: AtomicUsize,
    }
    impl Club for Actions {
        fn label(&self) -> &str {
            "deli-action-fixture"
        }
        fn respond(&self, _: &str) -> Result<String, String> {
            unreachable!()
        }
        fn chat(&self, messages: &[ChatMsg], tools: &[ToolDef]) -> Result<ClubReply, String> {
            if tools.is_empty() {
                self.rounds.fetch_add(1, Ordering::AcqRel);
                return Ok(ClubReply::Text(block_ev(
                    "inspect approach",
                    &["a supported premise"],
                )));
            }
            assert_eq!(tools[0].name, "rl_campaign");
            let hop = self.actions.fetch_add(1, Ordering::AcqRel);
            if hop == 0 {
                assert!(messages.iter().any(
                    |m| m.role == ChatRole::Harness && m.content.contains("Deli deliberation")
                ));
                return Ok(ClubReply::Calls(vec![ToolCall {
                    id: "rl-start".into(),
                    name: "rl_campaign".into(),
                    args: serde_json::json!({"action":"run"}),
                }]));
            }
            assert!(
                messages
                    .iter()
                    .any(|m| m.tool_call_id.as_deref() == Some("rl-start"))
            );
            Ok(ClubReply::Text(
                "Continue work while the campaign runs".into(),
            ))
        }
    }
    let inner = Arc::new(Actions {
        rounds: AtomicUsize::new(0),
        actions: AtomicUsize::new(0),
    });
    let deli = DeliClub::with_knobs(
        "deli",
        inner.clone(),
        Knobs {
            rounds: 2,
            ..Knobs::default()
        },
    );
    let tools = [ToolDef {
        name: "rl_campaign".into(),
        description: "real campaign".into(),
        params: serde_json::json!({"type":"object"}),
    }];
    let mut history = vec![
        ChatMsg::system("operator instructions"),
        ChatMsg::user("improve the objective"),
    ];
    let cancel = AtomicBool::new(false);
    let reply = deli
        .chat_streaming(&history, &tools, &cancel, &mut |_| {})
        .unwrap();
    let ClubReply::Calls(calls) = reply else {
        panic!("Deli must preserve native tool calls")
    };
    assert_eq!(calls[0].name, "rl_campaign");
    history.push(ChatMsg::assistant_calls(calls));
    history.push(ChatMsg::tool("rl-start", "campaign started"));
    assert!(matches!(
        deli.chat_streaming(&history, &tools, &cancel, &mut |_| {})
            .unwrap(),
        ClubReply::Text(_)
    ));
    assert_eq!(
        inner.rounds.load(Ordering::Acquire),
        2,
        "one prelude for the whole action phase"
    );
    assert_eq!(inner.actions.load(Ordering::Acquire), 2);
    cancel.store(true, Ordering::Release);
    assert!(
        deli.chat_streaming(&history, &tools, &cancel, &mut |_| {})
            .is_err()
    );
    assert_eq!(
        inner.actions.load(Ordering::Acquire),
        2,
        "cancellation sends no further request"
    );
}

#[test]
fn cancelling_deli_reflection_never_starts_an_uncancelled_synthesis_request() {
    use std::sync::atomic::AtomicUsize;
    struct CancelRound(AtomicUsize);
    impl Club for CancelRound {
        fn label(&self) -> &str {
            "cancel-deli-round"
        }
        fn respond(&self, _: &str) -> Result<String, String> {
            unreachable!()
        }
        fn chat_streaming(
            &self,
            _: &[ChatMsg],
            _: &[ToolDef],
            cancel: &AtomicBool,
            _: &mut dyn FnMut(StreamDelta),
        ) -> Result<ClubReply, String> {
            self.0.fetch_add(1, Ordering::AcqRel);
            cancel.store(true, Ordering::Release);
            Err("operator cancelled during reflection".into())
        }
    }
    let inner = Arc::new(CancelRound(AtomicUsize::new(0)));
    let deli = DeliClub::with_knobs(
        "deli",
        inner.clone(),
        Knobs {
            rounds: 3,
            ..Knobs::default()
        },
    );
    let cancel = AtomicBool::new(false);
    let result = deli.chat_streaming(
        &[ChatMsg::user("reflect on these measured results")],
        &[],
        &cancel,
        &mut |_| {},
    );
    assert!(result.unwrap_err().contains("cancelled"));
    assert_eq!(
        inner.0.load(Ordering::Acquire),
        1,
        "no synthesis or later round after cancellation"
    );
}

/// A club that always asks for a tool — text-only deliberation offers none.
struct ToolHungry;
impl Club for ToolHungry {
    fn label(&self) -> &str {
        "hungry"
    }
    fn is_available(&self) -> bool {
        false
    }
    fn respond(&self, _p: &str) -> Result<String, String> {
        Ok(String::new())
    }
    fn chat(&self, _m: &[ChatMsg], _t: &[ToolDef]) -> Result<ClubReply, String> {
        Ok(ClubReply::Calls(Vec::new()))
    }
}

#[test]
fn worker_tool_request_surfaces_as_an_error_at_synthesis() {
    // The iteration loop swallows a tool-requesting worker as a stall, but the
    // final synthesis pass propagates the "(none offered)" error.
    let deli = DeliClub::with_knobs("deli", Arc::new(ToolHungry), Knobs::default());
    let err = deli.respond("solve it").unwrap_err();
    assert!(err.contains("requested a tool"), "got: {err}");
}

#[test]
fn is_available_delegates_to_the_inner_club() {
    let deli = DeliClub::with_knobs("deli", Arc::new(ToolHungry), Knobs::default());
    assert!(
        !deli.is_available(),
        "delegates the readiness probe to inner"
    );
}

/// A block carrying the evidence tags and HYPOTHESES section the curated
/// prompt actually asks for — the shape a compliant worker emits.
fn block_full(dir: &str, findings: &[&str], hypotheses: &[&str]) -> String {
    let mut s = format!("DIRECTION: {dir}\nFINDINGS:\n");
    for f in findings {
        s.push_str(&format!("- {f}\n"));
    }
    if !hypotheses.is_empty() {
        s.push_str("HYPOTHESES:\n");
        for h in hypotheses {
            s.push_str(&format!("- {h}\n"));
        }
    }
    s
}

#[test]
fn hypotheses_are_retained_not_discarded() {
    // A worker that honestly labels an untested idea must not lose it. The
    // curated prompt asks for a HYPOTHESES section; discarding it punishes
    // exactly the honest labeling the protocol is trying to elicit.
    let inner = ScriptClub::new(&[&block_full(
        "a",
        &["fact one [evidence: premise:the stated goal]"],
        &["fusion might help", "the cache may be cold"],
    )]);
    let k = Knobs {
        rounds: 1,
        ..Knobs::default()
    };
    let deli = DeliClub::with_knobs("deli", inner.clone(), k);
    let st = deli.iterate("solve it", "", &AtomicBool::new(false));
    assert_eq!(st.findings.len(), 1, "the evidenced fact is a finding");
    assert_eq!(
        st.hypotheses.len(),
        2,
        "labeled hypotheses are retained for a later iteration to validate"
    );
}

#[test]
fn unevidenced_findings_are_demoted_to_hypotheses_not_counted_as_progress() {
    // The curated prompt states: "Only FINDINGS with a concrete, checkable
    // evidence tag are admitted as progress." A bare assertion must not
    // reset stall detection, or the loop never pivots off a confabulating
    // worker.
    let inner = ScriptClub::new(&[&block("a", &["bare assertion with no evidence tag"])]);
    let k = Knobs {
        rounds: 1,
        ..Knobs::default()
    };
    let deli = DeliClub::with_knobs("deli", inner.clone(), k);
    let st = deli.iterate("solve it", "", &AtomicBool::new(false));
    assert_eq!(
        st.findings.len(),
        0,
        "an unevidenced claim is not an admitted finding"
    );
    assert_eq!(
        st.hypotheses.len(),
        1,
        "it is retained as an unverified lead"
    );
    assert_eq!(st.stale_count, 1, "unevidenced output does not reset stall");
}

#[test]
fn fabricated_filesystem_citations_are_rejected_in_the_reasoning_regime() {
    // Reproduces what nemotron-3-super-120b, gpt-oss-20b and ling-3.0-flash
    // all did when the old prompt demanded `file:`/`benchmark:` evidence
    // from this tool-less worker: they invented it. One of them stated the
    // exploit directly — "the system might not actually check the existence
    // of the file; it's just a format". deli has no filesystem, so a
    // citation of that kind is proof of fabrication, not weak evidence.
    let inner = ScriptClub::new(&[&block(
        "pretend to have profiled",
        &[
            "the handler opens a fresh connection per request \
             [evidence: file:src/handlers.rs:27]",
            "pooling cuts p99 from 180ms to 34ms \
             [evidence: benchmark:target/criterion/http_p99/report.json]",
        ],
    )]);
    let k = Knobs {
        rounds: 1,
        ..Knobs::default()
    };
    let deli = DeliClub::with_knobs("deli", inner.clone(), k);
    let st = deli.iterate("make it faster", "", &AtomicBool::new(false));
    assert_eq!(
        st.findings.len(),
        0,
        "a tool-less worker cannot have read a file or run a benchmark"
    );
    assert_eq!(st.hypotheses.len(), 2, "both are retained as unverified");
    assert_eq!(
        st.stale_count, 1,
        "fabricated citations must not read as progress"
    );
}

#[test]
fn the_reasoning_contract_forbids_the_citations_the_worker_cannot_obtain() {
    let inner = ScriptClub::new(&[&block_ev("a", &["f1"])]);
    let deli = DeliClub::with_knobs(
        "deli",
        inner.clone(),
        Knobs {
            rounds: 1,
            ..Knobs::default()
        },
    );
    let _ = deli.iterate("solve it", "", &AtomicBool::new(false));
    let prompt = inner.prompts.lock().unwrap()[0].clone();
    // It must offer the kinds this worker can supply...
    assert!(prompt.contains("premise:"), "{prompt}");
    assert!(prompt.contains("derivation:"), "{prompt}");
    // ...name the ones it cannot, so the model does not infer them...
    assert!(prompt.contains("NO repository"), "{prompt}");
    assert!(prompt.contains("would be fabricated"), "{prompt}");
    // ...and make honest labeling the cheap option.
    assert!(prompt.contains("costs you nothing"), "{prompt}");
    // The grounded example shapes must not leak into this regime.
    assert!(
        !prompt.contains("[evidence: file:<path>:<line>]"),
        "the grounded contract must not be shown to a tool-less worker: {prompt}"
    );
}

#[test]
fn placeholder_evidence_is_not_admissible() {
    let inner = ScriptClub::new(&[&block("a", &["claim [evidence: none]"])]);
    let k = Knobs {
        rounds: 1,
        ..Knobs::default()
    };
    let deli = DeliClub::with_knobs("deli", inner.clone(), k);
    let st = deli.iterate("solve it", "", &AtomicBool::new(false));
    assert_eq!(st.findings.len(), 0, "'none' is not evidence");
    assert_eq!(st.stale_count, 1);
}

#[test]
fn repeated_direction_does_not_inflate_the_tried_list() {
    // directions_tried rides in every subsequent curated prompt. A worker
    // that repeats its DIRECTION line must not grow that list without
    // bound — it burns context and misreports the search as broader than
    // it was.
    let inner = ScriptClub::new(&[&block_full(
        "the same angle",
        &["f1 [evidence: premise:the stated goal]"],
        &[],
    )]);
    let k = Knobs {
        rounds: 4,
        pivot: 99,
        stall_stop: 0,
        ..Knobs::default()
    };
    let deli = DeliClub::with_knobs("deli", inner.clone(), k);
    let st = deli.iterate("solve it", "", &AtomicBool::new(false));
    assert_eq!(st.iteration, 4);
    assert_eq!(
        st.directions_tried,
        vec!["the same angle"],
        "one distinct angle was tried, so the record shows one"
    );
}

#[test]
fn hypotheses_ride_into_the_curated_prompt_for_validation() {
    // An unverified lead is only useful if a later iteration can see it.
    let inner = ScriptClub::new(&[
        &block_full("a", &[], &["the allocator may be the bottleneck"]),
        &block_full("b", &["f [evidence: premise:the stated goal]"], &[]),
    ]);
    let k = Knobs {
        rounds: 2,
        stall_stop: 0,
        ..Knobs::default()
    };
    let deli = DeliClub::with_knobs("deli", inner.clone(), k);
    let _ = deli.iterate("solve it", "", &AtomicBool::new(false));
    let prompts = inner.prompts.lock().unwrap();
    assert!(
        prompts[1].contains("the allocator may be the bottleneck"),
        "round 2 must see round 1's unverified lead so it can validate it"
    );
}

/// Cheap workers for the live calibrator, in the order they're tried.
/// LongCat exposes one model; GLM's roster supplies the rest of the spread.
/// Both are subscription/flat-rate links per the project's test-budget rule,
/// so a calibration pass costs no metered spend.
fn calibration_workers() -> Vec<(String, Arc<dyn Club>)> {
    let mut out: Vec<(String, Arc<dyn Club>)> = Vec::new();
    let longcat_key = std::env::var("ANGEL_LONGCAT_KEY")
        .ok()
        .or_else(|| std::env::var("LONGCAT_API_KEY").ok());
    if let Some(key) = longcat_key {
        let url = std::env::var("ANGEL_LONGCAT_URL")
            .unwrap_or_else(|_| "https://api.longcat.chat/openai/v1".to_string());
        let model =
            std::env::var("ANGEL_LONGCAT_MODEL").unwrap_or_else(|_| "LongCat-2.0".to_string());
        out.push((
            format!("longcat/{model}"),
            Arc::new(crate::club::HttpClub::new("longcat", url, model, Some(key)).sota_tuned()),
        ));
    }
    let glm_key = [
        "ANGEL_GLM_KEY",
        "GLM_API_KEY",
        "ZHIPU_API_KEY",
        "ZAI_API_KEY",
    ]
    .iter()
    .find_map(|k| std::env::var(k).ok());
    if let Some(key) = glm_key {
        let url = std::env::var("ANGEL_GLM_URL")
            .unwrap_or_else(|_| crate::club::default_glm_url().to_string());
        // A couple of tiers, so the calibration is not one model's quirk.
        let models = std::env::var("ANGEL_CALIBRATE_GLM_MODELS")
            .unwrap_or_else(|_| "glm-5.3,glm-5.3-flash,glm-5.2".to_string());
        for model in models.split(',').map(str::trim).filter(|m| !m.is_empty()) {
            out.push((
                format!("glm/{model}"),
                Arc::new(
                    crate::club::HttpClub::new(
                        "glm",
                        url.clone(),
                        model.to_string(),
                        Some(key.clone()),
                    )
                    .sota_tuned(),
                ),
            ));
        }
    }
    out
}

/// Live evidence-regime calibrator. Opt-in (`ANGEL_LIVE_CALIBRATE=1`); skips
/// when no cheap link is configured.
///
/// This is the end-to-end check the unit contracts cannot make. Those pin
/// what *this code* admits; only a live worker shows what a model actually
/// emits when handed the shipped prompt. The regression it guards is real
/// and was measured: under the old grounded-only contract, tool-less workers
/// invented `file:` and `benchmark:` citations — and invented the
/// measurements to go with them — rather than admit they had no evidence.
///
/// Run it after touching `iterate.rs`, the deli contract, or the regime
/// split, and whenever a worker model changes underneath us:
/// `ANGEL_LIVE_CALIBRATE=1 cargo test --no-default-features \
///   live_deli_evidence_calibration -- --ignored --nocapture`
#[test]
#[ignore = "needs live server: ANGEL_LIVE_CALIBRATE=1 and a reachable deli provider"]
fn live_deli_evidence_calibration() {
    if std::env::var("ANGEL_LIVE_CALIBRATE").is_err() {
        eprintln!("set ANGEL_LIVE_CALIBRATE=1 to run the live deli calibrator; skipping");
        return;
    }
    let workers = calibration_workers();
    if workers.is_empty() {
        eprintln!("no cheap link configured (LONGCAT_API_KEY / ANGEL_GLM_KEY); skipping");
        return;
    }
    // Problems a reasoning-only worker genuinely cannot settle from the text
    // alone — exactly the pressure that used to produce invented citations.
    let problems = [
        "Reduce p99 latency of a Rust HTTP service from 180ms to under 50ms. \
         The service does one Postgres query per request and serializes to JSON.",
        "A CI suite of 2000 tests fails intermittently about once every 20 runs, \
         always in a different test. Find the cause.",
    ];
    let knobs = Knobs {
        rounds: 2,
        pivot: 2,
        stall_stop: 0,
        min_findings: 0,
        state_dir: String::new(),
    };

    let mut runs: Vec<CalibrationRun> = Vec::new();
    for (name, club) in &workers {
        for problem in problems {
            let deli = DeliClub::with_knobs("deli", club.clone(), knobs.clone());
            let st = deli.iterate(problem, "", &AtomicBool::new(false));
            runs.push(score_run(name, problem, &st, DELI_REGIME));
        }
    }

    println!("\n=== deli evidence-regime calibration ({DELI_REGIME:?}) ===");
    println!(
        "{:<22} {:>4} {:>6} {:>6} {:>6} {:>8} {:>6}",
        "worker", "iter", "admit", "leads", "untag", "FABRIC", "compl"
    );
    for r in &runs {
        println!(
            "{:<22} {:>4} {:>6} {:>6} {:>6} {:>8} {:>5.0}%",
            r.worker,
            r.iterations,
            r.admitted,
            r.leads,
            r.untagged,
            r.fabricated,
            r.compliance() * 100.0
        );
        // A silent zero row is uninterpretable, so never print one silently:
        // say whether the worker failed or simply broke no new ground.
        if r.admitted + r.leads == 0 {
            match &r.error {
                Some(e) => println!("{:<22}   \\_ worker call failed: {e}", ""),
                None => println!(
                    "{:<22}   \\_ no parseable bullet (worker replied, contract not followed)",
                    ""
                ),
            }
        }
    }
    let fabricated: usize = runs.iter().map(|r| r.fabricated).sum();
    let admitted: usize = runs.iter().map(|r| r.admitted).sum();
    let leads: usize = runs.iter().map(|r| r.leads).sum();
    println!("--- totals: admitted={admitted} leads={leads} fabricated={fabricated}");

    let worker_errors = runs
        .iter()
        .filter_map(|run| {
            run.error
                .as_ref()
                .map(|error| format!("{}: {error}", run.worker))
        })
        .collect::<Vec<_>>();
    assert!(
        worker_errors.is_empty(),
        "configured live calibration worker call(s) failed: {}",
        worker_errors.join("; ")
    );
    // The load-bearing assertion. Everything else is calibration data the
    // operator reads; this one is a contract. A tool-less worker that cites
    // a file it cannot open has fabricated, and the gate must have caught
    // it — a non-zero count here means the regime split has regressed.
    assert_eq!(
        fabricated, 0,
        "a reasoning-regime worker emitted {fabricated} citation(s) naming a source kind it \
         cannot obtain; the evidence gate is no longer rejecting fabrication"
    );
    // A run that admits nothing at all across every worker and problem would
    // mean the contract is unusable in practice, not merely strict.
    assert!(
        admitted + leads > 0,
        "no worker produced any parseable bullet; the output contract may be malformed"
    );
}

#[test]
fn empty_problem_falls_through_to_direct_pass() {
    // No user content → a single inner pass, no iteration. The synth system
    // prompt path returns the fixed string.
    let inner = ScriptClub::new(&[]);
    let deli = DeliClub::with_knobs("deli", inner.clone(), Knobs::default());
    let out = deli
        .run(&[], &AtomicBool::new(false), &mut |_| {}, false)
        .unwrap();
    // The direct pass goes through chat() with no system match → worker branch
    // returns the (empty script → last) default; assert it didn't iterate.
    assert_eq!(
        *inner.worker_calls.lock().unwrap(),
        1,
        "one direct pass, no loop"
    );
    let _ = out;
}
