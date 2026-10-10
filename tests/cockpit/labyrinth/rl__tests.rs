use super::*;
use crate::agent::harness::{
    DescendantBudgetScope, current_descendant_budget, reserve_descendant_calls,
};

struct BadReportClub(Arc<AtomicUsize>);
impl Club for BadReportClub {
    fn label(&self) -> &str {
        "labyrinth-budget-fixture"
    }
    fn respond(&self, _: &str) -> Result<String, String> {
        self.0.fetch_add(1, Ordering::AcqRel);
        Ok("{}".into())
    }
}

#[test]
fn labyrinth_rl_campaign_mutations_require_workspace_write_scope() {
    use crate::agent::harness::Tool;
    let tool = crate::agent::tools::loop_research::LoopResearchTool::new(
        std::env::temp_dir(),
        Arc::new(Mutex::new(RlState::default())),
    );
    for action in ["start", "run", "cancel", "recover", "recheck"] {
        assert!(tool.workspace_write_scope_is_opaque(
            &json!({"action":"campaign","campaign_action":action})
        ));
    }
    for action in ["status", "check"] {
        assert!(!tool.workspace_write_scope_is_opaque(
            &json!({"action":"campaign","campaign_action":action})
        ));
    }
    assert!(
        tool.def().params["properties"]["campaign_action"]["enum"]
            .as_array()
            .unwrap()
            .contains(&json!("recheck"))
    );
    assert!(!tool.workspace_write_scope_is_opaque(&json!({"action":"campaign","campaign_action":"check","campaign_bundle":"labyrinth/angel/campaigns/c/integrations/i.json"})));
}

#[test]
fn labyrinth_rl_preserves_incomplete_campaign_outcomes() {
    for status in [
        "budget_exhausted",
        "deadline",
        "execution_blocked",
        "saturated",
    ] {
        assert_eq!(
            campaign_outcome_status(&json!({"status":status,"integrations":[]})),
            status
        );
    }
    assert_eq!(
        campaign_outcome_status(&json!({"status":"cancelled"})),
        "stopped"
    );
    assert_eq!(
        campaign_outcome_status(&json!({"status":"completed","integrations":[]})),
        "unintegrated"
    );
    assert_eq!(
        campaign_outcome_status(&json!({"status":"completed","integrations":[{"id":"checked"}]})),
        "completed"
    );
}

#[test]
fn labyrinth_rl_worker_inherits_remaining_root_descendant_budget() {
    let root =
        std::env::temp_dir().join(format!("angel-labyrinth-rl-budget-{}", std::process::id()));
    std::fs::create_dir_all(&root).unwrap();
    std::fs::write(root.join("problem.json"), b"{\"values\":[0,1,2]}").unwrap();
    std::fs::write(root.join("notes.md"), b"TODO").unwrap();
    let scope = DescendantBudgetScope::for_test(10);
    let budget = current_descendant_budget().unwrap();
    reserve_descendant_calls(&budget, 8, "parent formations").unwrap();
    let calls = Arc::new(AtomicUsize::new(0));
    let club = Arc::new(BadReportClub(calls.clone()));
    let mut state = RlState::default();
    state.bind_loop(LoopCampaignContext {
        loop_id: "budget-fixture".into(),
        task: "bounded research".into(),
        verify: None,
        club,
        deadline: None,
        remaining_tokens: None,
    });
    let spec = json!({"id":"bounded-workflow","task":"bounded research",
        "doors":[{"id":"q.bounded","statement":"small integer classification","missing":"independent checks","perspectives":["parity","tiny cases","enumeration","counting","primary definitions"]}],
        "inputs":["problem.json"],"referee_inputs":["problem.json"],"canonical_documents":["notes.md"],
        "checks":[{"argv":["python3","-c","print('pinned check')"],"timeout_secs":5}],
        "spot_checks":[{"argv":["python3","-c","print('pinned spot check')"],"timeout_secs":5}],
        "role_timeout_secs":5,"campaign_timeout_secs":10,"max_hops":2,"max_rounds":2});
    state
        .research_call(
            &root,
            &json!({"action":"campaign","campaign_action":"start","campaign_spec":spec}),
            &AtomicBool::new(false),
        )
        .unwrap();
    state
        .research_call(
            &root,
            &json!({"action":"campaign","campaign_action":"run","campaign_id":"bounded-workflow"}),
            &AtomicBool::new(false),
        )
        .unwrap();
    let started = Instant::now();
    while state.research_running() && started.elapsed() < Duration::from_secs(15) {
        std::thread::sleep(Duration::from_millis(20));
    }
    assert!(
        !state.research_running(),
        "worker must finish and retain its failure"
    );
    let receipt = reserve_descendant_calls(&budget, 0, "inspect shared allowance").unwrap();
    assert!(receipt.spent <= 10 && receipt.spent > 8, "{receipt:?}");
    assert!(calls.load(Ordering::Acquire) <= 2);
    let record: Value = serde_json::from_str(
        &state
            .research_call(&root, &json!({"action":"status"}), &AtomicBool::new(false))
            .unwrap(),
    )
    .unwrap();
    assert_ne!(record["status"], "completed", "{record}");
    assert!(record["measurements"].is_null());
    drop(scope);
    std::fs::remove_dir_all(root).unwrap();
}
