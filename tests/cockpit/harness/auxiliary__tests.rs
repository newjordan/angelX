use super::*;
use crate::agent::club::ToolDef;
use crate::agent::harness::{Hooks, Tool, ToolRegistry, run_code_mode_tool};
use serde_json::{Value, json};

struct OwnedTool(&'static str);
impl Tool for OwnedTool {
    fn name(&self) -> &str {
        self.0
    }
    fn def(&self) -> ToolDef {
        ToolDef {
            name: self.0.into(),
            description: "owned fixture".into(),
            params: json!({"type":"object"}),
        }
    }
    fn call(&self, _: &Value) -> Result<String, String> {
        if self.0 == "spawn" {
            Err("owned entered capability failed before linked child receipt".into())
        } else {
            Ok("owned read result".into())
        }
    }
}

#[test]
fn auxiliary_actual_registry_entry_distinguishes_unknown_rejected_and_failed_model_tool() {
    let mut registry = ToolRegistry::new();
    registry.register(Box::new(OwnedTool("read_file")));
    registry.register(Box::new(OwnedTool("spawn")));
    let scope = registry.auxiliary.enter();
    assert!(registry.dispatch("missing", &json!({})).is_err());
    assert_eq!(scope.snapshot().observed_tool_calls, 0);
    registry.dispatch("read_file", &json!({})).unwrap();
    assert!(scope.snapshot().complete);
    assert!(registry.dispatch("spawn", &json!({})).is_err());
    let actual = scope.snapshot();
    assert_eq!(actual.observed_tool_calls, 2);
    assert_eq!(actual.sources["model_tool"], 1);
    assert!(!actual.complete);
    actual.validate().unwrap();
}

#[test]
fn auxiliary_nested_readonly_code_mode_and_denied_nested_model_stay_complete() {
    let _env = crate::tests::env_lock();
    let _effects = crate::tests::TestEnvGuard::set("ANGEL_CODE_MODE_EFFECTS", "0");
    let _yolo = crate::tests::TestEnvGuard::set("ANGEL_YOLO", "0");
    let mut registry = ToolRegistry::new();
    registry.register(Box::new(OwnedTool("read_file")));
    registry.register(Box::new(OwnedTool("spawn")));
    let scope = registry.auxiliary.enter();
    let result = run_code_mode_tool(
        &registry,
        &Hooks::default(),
        &json!({
            "script":"return read_file({path:'owned'});", "allow_effects":false
        }),
        None,
        None,
    );
    assert!(result.contains("owned read result"), "{result}");
    assert_eq!(scope.snapshot().observed_tool_calls, 1);
    let result = run_code_mode_tool(
        &registry,
        &Hooks::default(),
        &json!({
            "script":"return spawn({task:'denied before entry'});", "allow_effects":false
        }),
        None,
        None,
    );
    assert!(result.contains("allow_effects=true"), "{result}");
    let actual = scope.snapshot();
    assert_eq!(actual.observed_tool_calls, 1);
    assert!(actual.complete);
    assert!(actual.sources.is_empty());
}

#[test]
fn auxiliary_parallel_shared_registry_overlap_is_unknown_then_fresh_turn_is_clear() {
    let mut registry = ToolRegistry::new();
    registry.register(Box::new(OwnedTool("read_file")));
    let first = registry.auxiliary.enter();
    std::thread::scope(|threads| {
        let registry = &registry;
        threads
            .spawn(move || {
                let second = registry.auxiliary.enter();
                registry.dispatch("read_file", &json!({})).unwrap();
                assert!(second.snapshot().overlapping_turns);
                assert!(!second.snapshot().complete);
            })
            .join()
            .unwrap();
    });
    assert!(first.snapshot().overlapping_turns);
    assert!(!first.snapshot().complete);
    drop(first);
    let fresh = registry.auxiliary.enter();
    registry.dispatch("read_file", &json!({})).unwrap();
    assert!(fresh.snapshot().complete);
    assert_eq!(fresh.snapshot().observed_tool_calls, 1);
}

#[test]
fn auxiliary_utility_partial_work_and_late_background_splice_remain_unlinked() {
    let tracker = AuxiliaryTracker::default();
    {
        let old = tracker.enter();
        tracker.utility_entered("background_compaction");
        assert!(!old.snapshot().complete);
    }
    let current = tracker.enter();
    assert!(current.snapshot().complete);
    tracker.utility_entered("background_compaction");
    for source in [
        "tool_bubble",
        "advisor",
        "teacher_watch",
        "prior_policy_turn",
        "sync_compaction",
    ] {
        tracker.utility_entered(source);
    }
    let coverage = current.snapshot();
    assert_eq!(coverage.unlinked_auxiliary_operations, 6);
    assert_eq!(coverage.observed_tool_calls, 0);
    assert!(!coverage.complete);
    coverage.validate().unwrap();
}

#[test]
fn auxiliary_schema_rejects_false_completion_unknown_keys_and_unsafe_counts() {
    let valid = AuxiliaryCoverage::default();
    valid.validate().unwrap();
    let mut invalid = valid.clone();
    invalid.sources.insert("advisor".into(), 1);
    assert!(invalid.validate().is_err());
    invalid.unlinked_auxiliary_operations = 1;
    assert!(invalid.validate().is_err());
    invalid.complete = false;
    invalid.validate().unwrap();
    invalid.sources.insert("invented".into(), 1);
    assert!(invalid.validate().is_err());
    let mut invalid = valid;
    invalid.observed_tool_calls = SAFE_COUNT + 1;
    assert!(invalid.validate().is_err());
}
