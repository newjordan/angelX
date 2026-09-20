//! Protocol/effect fixtures, not live-model quality measurements.
use super::*;
use crate::club::{ChatRole, StreamDelta, ToolCall};
use crate::tests::{TestEnvGuard, TestGitWorkspace, env_lock};
use serde_json::json;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};

struct ExcursionClub {
    root_hops: AtomicUsize,
    rounds: AtomicUsize,
    syntheses: AtomicUsize,
    cancel_in_round: bool,
}

impl ExcursionClub {
    fn new(cancel_in_round: bool) -> Arc<Self> {
        Arc::new(Self {
            root_hops: AtomicUsize::new(0),
            rounds: AtomicUsize::new(0),
            syntheses: AtomicUsize::new(0),
            cancel_in_round,
        })
    }
}

impl Club for ExcursionClub {
    fn label(&self) -> &str {
        "deli-excursion-fixture"
    }
    fn respond(&self, _: &str) -> Result<String, String> {
        Err("fixture requires structured chat".into())
    }
    fn chat_streaming(
        &self,
        messages: &[ChatMsg],
        tools: &[ToolDef],
        cancel: &AtomicBool,
        _: &mut dyn FnMut(StreamDelta),
    ) -> Result<ClubReply, String> {
        if tools.is_empty() {
            let system = messages
                .iter()
                .filter(|m| m.role == ChatRole::System)
                .map(|m| m.content.as_ref())
                .collect::<Vec<_>>()
                .join("\n");
            if system.starts_with("You synthesize the accumulated findings") {
                self.syntheses.fetch_add(1, Ordering::AcqRel);
                assert!(messages.iter().any(|m| m.content.contains("amber")));
                return Ok(ClubReply::Text(
                    "Write amber to chosen.txt, then check the file.".into(),
                ));
            }
            let round = self.rounds.fetch_add(1, Ordering::AcqRel);
            if self.cancel_in_round {
                cancel.store(true, Ordering::Release);
                return Err("fixture cancelled during deliberation".into());
            }
            return Ok(ClubReply::Text(format!(
                "DIRECTION: alternative {round}\nFINDINGS:\n- Use amber [evidence: premise:the supplied candidates include amber]\n"
            )));
        }
        match self.root_hops.fetch_add(1, Ordering::AcqRel) {
            0 => {
                let tool = tools
                    .iter()
                    .find(|t| t.name == "consult_model")
                    .expect("Deli entry advertised");
                assert!(tool.description.contains("method=deli"));
                Ok(ClubReply::Calls(vec![ToolCall {
                    id: "deli-choice".into(),
                    name: "consult_model".into(),
                    args: json!({"method":"deli","club":"self","rounds":2,
                        "prompt":"Choose amber from the supplied candidates and propose the next file action."}),
                }]))
            }
            1 => {
                let result = messages
                    .iter()
                    .find(|m| m.tool_call_id.as_deref() == Some("deli-choice"))
                    .expect("Deli result returns to the original turn");
                assert!(
                    result.content.contains("Write amber to chosen.txt"),
                    "{}",
                    result.content
                );
                Ok(ClubReply::Calls(vec![ToolCall {
                    id: "apply-choice".into(),
                    name: "write_file".into(),
                    args: json!({"path":"chosen.txt","content":"amber\n"}),
                }]))
            }
            _ => Ok(ClubReply::Text(
                "Applied the Deli proposal in the current workspace.".into(),
            )),
        }
    }
}

#[test]
fn model_enters_deli_returns_to_normal_tools_and_applies_its_result() {
    let _lock = env_lock();
    let workspace = TestGitWorkspace::new("deli-tool-continuity");
    let _advisor = TestEnvGuard::set("ANGEL_ADVISOR", "0");
    let _skill = TestEnvGuard::set("ANGEL_SKILL_HINT", "0");
    let _profile = TestEnvGuard::set("ANGEL_TOOL_SCHEMA_PROFILE", "full");
    let _findings = TestEnvGuard::set("ANGEL_DELI_MIN_FINDINGS", "0");
    let _stop = TestEnvGuard::set("ANGEL_DELI_STALL_STOP", "0");
    let club = ExcursionClub::new(false);
    let registry = crate::harness::ToolRegistry::with_team_self(
        workspace.path().into(),
        vec![],
        Some(club.clone()),
    );
    let mut history = vec![ChatMsg::user(
        "Use a Deli excursion to decide the next artifact, then apply it here.",
    )];
    let (events, _received) = std::sync::mpsc::channel();
    crate::harness::run_turn(
        club.as_ref(),
        &registry,
        &mut history,
        &AtomicBool::new(false),
        Some(4),
        &events,
    )
    .unwrap();
    assert_eq!(
        std::fs::read_to_string(workspace.path().join("chosen.txt")).unwrap(),
        "amber\n"
    );
    assert_eq!(club.rounds.load(Ordering::Acquire), 2);
    assert_eq!(club.syntheses.load(Ordering::Acquire), 1);
    assert_eq!(
        club.root_hops.load(Ordering::Acquire),
        3,
        "ordinary tool continuation does not restart Deli"
    );
}

#[test]
fn cancelling_deli_consult_stops_before_synthesis_and_keeps_self_route() {
    let _lock = env_lock();
    let club = ExcursionClub::new(true);
    let tool = ConsultModelTool::new(vec![], Some(club.clone()));
    let cancel = AtomicBool::new(false);
    let error = tool
        .call_with_cancel(
            &json!({"method":"deli","rounds":2,"prompt":"Inspect this premise"}),
            Some(&cancel),
        )
        .unwrap_err();
    assert!(error.contains("cancel"), "{error}");
    assert_eq!(club.rounds.load(Ordering::Acquire), 1);
    assert_eq!(club.syntheses.load(Ordering::Acquire), 0);
    assert!(
        tool.call_with_cancel(
            &json!({"method":"direct","prompt":"do not run"}),
            Some(&cancel)
        )
        .is_err()
    );
    assert_eq!(club.rounds.load(Ordering::Acquire), 1);
}

#[test]
fn deli_consult_keeps_pinned_effort_through_all_rounds_and_synthesis() {
    let _lock = env_lock();
    struct EffortProbe(AtomicUsize);
    impl Club for EffortProbe {
        fn label(&self) -> &str {
            "effort-fixture"
        }
        fn respond(&self, _: &str) -> Result<String, String> {
            Err("must use effort-aware chat".into())
        }
        fn chat_streaming_with_effort(
            &self,
            _: &[ChatMsg],
            _: &[ToolDef],
            effort: Option<&str>,
            _: &AtomicBool,
            _: &mut dyn FnMut(StreamDelta),
        ) -> Result<ClubReply, String> {
            assert_eq!(effort, Some("high"));
            self.0.fetch_add(1, Ordering::AcqRel);
            Ok(ClubReply::Text("DIRECTION: inspect premise\nFINDINGS:\n- There is a premise [evidence: premise:the supplied task]\n".into()))
        }
    }
    let _findings = TestEnvGuard::set("ANGEL_DELI_MIN_FINDINGS", "0");
    let _stop = TestEnvGuard::set("ANGEL_DELI_STALL_STOP", "0");
    let club = Arc::new(EffortProbe(AtomicUsize::new(0)));
    let deli = crate::deli::DeliClub::for_consult(club.clone(), Some(2), Some("high".into()));
    deli.chat_streaming(
        &[ChatMsg::user("Inspect this premise")],
        &[],
        &AtomicBool::new(false),
        &mut |_| {},
    )
    .unwrap();
    assert_eq!(club.0.load(Ordering::Acquire), 3);
}
