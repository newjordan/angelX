//! Task-scoped observation of native auxiliary operations. This is deliberately
//! not semantic authorship, external shell-process tracking, or a model label.
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::sync::{Arc, Mutex, Weak};

const SAFE_COUNT: u64 = (1u64 << 53) - 1;
const SOURCES: [&str; 8] = [
    "model_tool",
    "loop_recovery_context",
    "advisor",
    "teacher_watch",
    "prior_policy_turn",
    "tool_bubble",
    "sync_compaction",
    "background_compaction",
];

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct AuxiliaryCoverage {
    pub(crate) schema: String,
    pub(crate) scope: String,
    pub(crate) observed_tool_calls: u64,
    pub(crate) unlinked_auxiliary_operations: u64,
    pub(crate) sources: BTreeMap<String, u64>,
    pub(crate) overlapping_turns: bool,
    pub(crate) complete: bool,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub(crate) linked_operations: Vec<super::delegated_lineage::LinkedOperation>,
}

impl Default for AuxiliaryCoverage {
    fn default() -> Self {
        Self {
            schema: "angel-native-auxiliary-coverage/v1".into(),
            scope: "observed-native-dispatch".into(),
            observed_tool_calls: 0,
            unlinked_auxiliary_operations: 0,
            sources: BTreeMap::new(),
            overlapping_turns: false,
            complete: true,
            linked_operations: Vec::new(),
        }
    }
}

impl AuxiliaryCoverage {
    pub(crate) fn validate(&self) -> Result<(), String> {
        let sum = self.sources.iter().try_fold(0u64, |sum, (key, count)| {
            if !SOURCES.contains(&key.as_str()) || *count == 0 || *count > SAFE_COUNT {
                return None;
            }
            sum.checked_add(*count)
        });
        if ![
            "angel-native-auxiliary-coverage/v1",
            "angel-native-auxiliary-coverage/v2",
        ]
        .contains(&self.schema.as_str())
            || (self.schema == "angel-native-auxiliary-coverage/v1"
                && !self.linked_operations.is_empty())
            || (self.schema == "angel-native-auxiliary-coverage/v2"
                && self.linked_operations.is_empty())
            || self.scope != "observed-native-dispatch"
            || self.observed_tool_calls > SAFE_COUNT
            || self.unlinked_auxiliary_operations > SAFE_COUNT
            || sum != Some(self.unlinked_auxiliary_operations)
            || self.complete != (self.unlinked_auxiliary_operations == 0 && !self.overlapping_turns)
        {
            return Err("invalid native auxiliary coverage".into());
        }
        if self.linked_operations.len() > 32
            || self.linked_operations.len() as u64 > self.observed_tool_calls
            || (self.schema == "angel-native-auxiliary-coverage/v2"
                && self
                    .sources
                    .get("model_tool")
                    .copied()
                    .unwrap_or(0)
                    .checked_add(self.linked_operations.len() as u64)
                    .is_none_or(|count| count > self.observed_tool_calls))
            || serde_json::to_vec(&self.linked_operations)
                .map_err(|e| e.to_string())?
                .len()
                > 1024 * 1024
        {
            return Err("native linked operation bound exceeded".into());
        }
        let mut seen = std::collections::HashSet::new();
        for link in &self.linked_operations {
            if !seen.insert(&link.operation_id) {
                return Err("duplicate native linked operation".into());
            }
            link.validate()?;
        }
        Ok(())
    }
}

#[derive(Default)]
pub(crate) struct AuxiliaryTracker {
    active: Mutex<Vec<Weak<Mutex<AuxiliaryCoverage>>>>,
}

pub(crate) struct AuxiliaryScope(Arc<Mutex<AuxiliaryCoverage>>);

/// Single-use ownership of precisely the scopes charged at this dispatch.
pub(crate) struct AuxiliaryOperation {
    scopes: Vec<Weak<Mutex<AuxiliaryCoverage>>>,
    operation_id: String,
    tool: String,
    arguments_sha256: String,
}

impl AuxiliaryOperation {
    pub(crate) fn resolve(
        self,
        artifact: super::delegated_lineage::DelegateArtifact,
        application: Option<super::delegated_lineage::DelegateApplication>,
    ) {
        let link = super::delegated_lineage::LinkedOperation {
            operation_id: self.operation_id,
            tool: self.tool,
            arguments_sha256: self.arguments_sha256,
            artifact,
            application,
        };
        if link.validate().is_err() {
            return;
        }
        for scope in self.scopes.iter().filter_map(Weak::upgrade) {
            let mut value = scope.lock().unwrap_or_else(|p| p.into_inner());
            if value.overlapping_turns {
                continue;
            }
            let mut candidate = value.clone();
            let Some(count) = candidate.sources.get_mut("model_tool") else {
                continue;
            };
            *count -= 1;
            if *count == 0 {
                candidate.sources.remove("model_tool");
            }
            candidate.unlinked_auxiliary_operations -= 1;
            candidate.complete = candidate.unlinked_auxiliary_operations == 0;
            candidate.schema = "angel-native-auxiliary-coverage/v2".into();
            candidate.linked_operations.push(link.clone());
            if candidate.validate().is_ok() {
                *value = candidate;
            }
        }
    }
}

impl AuxiliaryScope {
    /// Charge this exact parent only when its retained messages reach a native
    /// provider dispatch. Rewrites later in the turn cannot erase consumption.
    pub(crate) fn observe_recovery_context(
        &self,
        messages: &[crate::club::ChatMsg],
        seen: &mut std::collections::HashSet<String>,
    ) {
        let refs = crate::club::recovery_context_refs(messages);
        let mut value = self.0.lock().unwrap_or_else(|p| p.into_inner());
        for reference in refs {
            if seen.insert(reference.import_id) {
                value.unlinked_auxiliary_operations =
                    value.unlinked_auxiliary_operations.saturating_add(1);
                *value
                    .sources
                    .entry("loop_recovery_context".into())
                    .or_default() += 1;
                value.complete = false;
            }
        }
    }

    pub(crate) fn snapshot(&self) -> AuxiliaryCoverage {
        self.0.lock().unwrap_or_else(|p| p.into_inner()).clone()
    }
}

impl AuxiliaryTracker {
    pub(crate) fn enter(&self) -> AuxiliaryScope {
        let mut active = self.active.lock().unwrap_or_else(|p| p.into_inner());
        active.retain(|scope| scope.strong_count() > 0);
        let mut value = AuxiliaryCoverage::default();
        if !active.is_empty() {
            value.overlapping_turns = true;
            value.complete = false;
            for prior in active.iter().filter_map(Weak::upgrade) {
                let mut prior = prior.lock().unwrap_or_else(|p| p.into_inner());
                prior.overlapping_turns = true;
                prior.complete = false;
            }
        }
        let scope = AuxiliaryScope(Arc::new(Mutex::new(value)));
        active.push(Arc::downgrade(&scope.0));
        scope
    }

    fn observe(&self, tool: bool, source: Option<&str>) {
        let mut active = self.active.lock().unwrap_or_else(|p| p.into_inner());
        active.retain(|scope| scope.strong_count() > 0);
        for scope in active.iter().filter_map(Weak::upgrade) {
            let mut value = scope.lock().unwrap_or_else(|p| p.into_inner());
            if tool {
                value.observed_tool_calls = value.observed_tool_calls.saturating_add(1);
            }
            if let Some(source) = source {
                value.unlinked_auxiliary_operations =
                    value.unlinked_auxiliary_operations.saturating_add(1);
                let count = value.sources.entry(source.into()).or_default();
                *count = count.saturating_add(1);
                value.complete = false;
            }
        }
    }

    pub(crate) fn tool_entered(&self, name: &str, args: &serde_json::Value) -> AuxiliaryOperation {
        // These capabilities can invoke a native model or import delegated work.
        // An entered capability may subsequently fail before or after model work;
        // absent a child receipt, neither failure nor success clears its origin.
        let model = matches!(
            name,
            "spawn"
                | "delegate"
                | "integrate"
                | "agent_graph"
                | "swarm_compile"
                | "consult_model"
                | "code_review"
                | "leanstral"
                | "llm_probe"
                | "llm_bench"
                | "vision_look"
                | "video_look"
                | "grok_research"
        );
        let model = model
            || (name == "knowledge_graph"
                && args.get("op").and_then(serde_json::Value::as_str) != Some("stats"));
        // Charge and capture the same scope set atomically: a concurrently
        // entering turn cannot steal a completion from an earlier dispatch.
        let mut active = self.active.lock().unwrap_or_else(|p| p.into_inner());
        active.retain(|scope| scope.strong_count() > 0);
        for scope in active.iter().filter_map(Weak::upgrade) {
            let mut value = scope.lock().unwrap_or_else(|p| p.into_inner());
            value.observed_tool_calls = value.observed_tool_calls.saturating_add(1);
            if model {
                value.unlinked_auxiliary_operations =
                    value.unlinked_auxiliary_operations.saturating_add(1);
                *value.sources.entry("model_tool".into()).or_default() += 1;
                value.complete = false;
            }
        }
        // Only these two implementations can return a linked artifact. Keep
        // ordinary reads/edits free of added argument serialization and hashes.
        if !matches!(name, "delegate" | "integrate") {
            return AuxiliaryOperation {
                scopes: Vec::new(),
                operation_id: String::new(),
                tool: String::new(),
                arguments_sha256: String::new(),
            };
        }
        static NEXT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
        let serial = NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        AuxiliaryOperation {
            scopes: if model { active.clone() } else { Vec::new() },
            operation_id: format!("dispatch-{}-{serial}", std::process::id()),
            tool: name.into(),
            arguments_sha256: crate::cut::sha256_hex(
                &serde_json::to_vec(args).expect("tool args serialize"),
            ),
        }
    }

    pub(crate) fn utility_entered(&self, source: &'static str) {
        debug_assert!(SOURCES.contains(&source));
        self.observe(false, Some(source));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::club::ToolDef;
    use crate::harness::{Hooks, Tool, ToolRegistry, run_code_mode_tool};
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
}
