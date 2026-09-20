use super::*;

struct ParallelBudgetClub {
    hops: AtomicUsize,
}

impl Club for ParallelBudgetClub {
    fn respond(&self, _prompt: &str) -> Result<String, String> {
        Ok("unused".to_string())
    }

    fn label(&self) -> &str {
        "parallel-budget"
    }

    fn chat(&self, _messages: &[ChatMsg], _tools: &[ToolDef]) -> Result<ClubReply, String> {
        if self.hops.fetch_add(1, Ordering::AcqRel) == 0 {
            Ok(ClubReply::Calls(vec![
                ToolCall {
                    id: "budget-a".to_string(),
                    // `reverse` is intentionally classified parallel-safe; the
                    // test registry supplies this accounting probe under that
                    // name to exercise the real scoped-worker boundary.
                    name: "reverse".to_string(),
                    args: serde_json::json!({"probe": 1}),
                },
                ToolCall {
                    id: "budget-b".to_string(),
                    name: "reverse".to_string(),
                    args: serde_json::json!({"probe": 2}),
                },
            ]))
        } else {
            Ok(ClubReply::Text("done".to_string()))
        }
    }
}

struct ParallelBudgetProbe {
    successful: Arc<AtomicUsize>,
}

impl Tool for ParallelBudgetProbe {
    fn name(&self) -> &str {
        "reverse"
    }

    fn def(&self) -> ToolDef {
        ToolDef {
            name: self.name().to_string(),
            description: "test-only descendant admission probe".to_string(),
            params: serde_json::json!({"type": "object"}),
        }
    }

    fn call(&self, _args: &Value) -> Result<String, String> {
        let budget = current_descendant_budget()?;
        let receipt = reserve_descendant_calls(&budget, 2, "parallel probe")?;
        self.successful.fetch_add(1, Ordering::AcqRel);
        Ok(receipt.fields())
    }
}

#[test]
fn parallel_tool_workers_inherit_one_root_descendant_budget() {
    let _budget_scope = DescendantBudgetScope::for_test(3);
    let budget = current_descendant_budget().unwrap();
    let successful = Arc::new(AtomicUsize::new(0));
    let mut registry = ToolRegistry::new();
    registry.register(Box::new(ParallelBudgetProbe {
        successful: Arc::clone(&successful),
    }));
    let club = ParallelBudgetClub {
        hops: AtomicUsize::new(0),
    };
    let mut history = vec![ChatMsg::user("probe parallel admission")];
    let (events, _) = mpsc::channel();

    let answer = run_turn(
        &club,
        &registry,
        &mut history,
        &AtomicBool::new(false),
        Some(3),
        &events,
    )
    .unwrap();

    assert_eq!(answer, "done");
    assert_eq!(successful.load(Ordering::Acquire), 1);
    assert_eq!(
        descendant_budget_status(&budget).unwrap(),
        DescendantBudgetReceipt {
            requested: 0,
            total: 3,
            spent: 2,
            remaining: 1,
        }
    );
    let tool_results = history
        .iter()
        .filter(|message| message.role == ChatRole::Tool)
        .map(|message| message.content.as_ref())
        .collect::<Vec<_>>();
    assert_eq!(tool_results.len(), 2);
    assert_eq!(
        tool_results
            .iter()
            .filter(|result| result.contains("zero calls admitted"))
            .count(),
        1
    );
}
