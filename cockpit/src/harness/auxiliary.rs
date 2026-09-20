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
#[path = "../../../tests/cockpit/harness/auxiliary__tests.rs"]
mod tests;
