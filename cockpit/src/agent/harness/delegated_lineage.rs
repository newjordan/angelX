//! Producer-owned delegate artifacts. These describe observed native execution
//! and committed output, not semantic authorship or training eligibility.
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct DelegateArtifact {
    pub(crate) schema: String,
    pub(crate) parent_workspace_sha256: String,
    pub(crate) task_sha256: String,
    pub(crate) answer_sha256: String,
    pub(crate) mode: String,
    pub(crate) base_oid: String,
    pub(crate) tip_oid: String,
    pub(crate) tree_oid: String,
    pub(crate) diff_sha256: String,
    pub(crate) child_rollout_id: String,
    pub(crate) child_audit: Value,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct DelegateApplication {
    pub(crate) before_commit: String,
    pub(crate) before_tree: String,
    pub(crate) after_commit: String,
    pub(crate) after_tree: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct LinkedOperation {
    pub(crate) operation_id: String,
    pub(crate) tool: String,
    pub(crate) arguments_sha256: String,
    pub(crate) artifact: DelegateArtifact,
    pub(crate) application: Option<DelegateApplication>,
}

fn digest(value: &str, lengths: &[usize]) -> bool {
    lengths.contains(&value.len())
        && value
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
}

impl DelegateArtifact {
    pub(crate) fn validate(&self) -> Result<(), String> {
        let audit = &self.child_audit;
        let termination = &audit["termination"];
        let routes = &audit["resolved_actions"];
        let coverage: super::auxiliary::AuxiliaryCoverage =
            serde_json::from_value(audit["auxiliary_coverage"].clone())
                .map_err(|_| "delegate child has no valid auxiliary coverage")?;
        // The implemented delegate registry is a leaf (shell/Cargo). Nested
        // delegate receipts need a separately reviewed linkage contract.
        if coverage.schema != "angel-native-auxiliary-coverage/v1"
            || !coverage.linked_operations.is_empty()
        {
            return Err("nested delegate contribution is not linked by this contract".into());
        }
        coverage.validate()?;
        if self.schema != "angel-delegate-artifact/v1"
            || !["write", "read_only"].contains(&self.mode.as_str())
            || [
                &self.parent_workspace_sha256,
                &self.task_sha256,
                &self.answer_sha256,
                &self.diff_sha256,
            ]
            .iter()
            .any(|s| !digest(s, &[64]))
            || [&self.base_oid, &self.tip_oid, &self.tree_oid]
                .iter()
                .any(|s| !digest(s, &[40, 64]))
            || audit["schema"] != "angel-harness-rollout-audit/v1"
            || audit["rollout_id"].as_str() != Some(self.child_rollout_id.as_str())
            || !self.child_rollout_id.starts_with("rol-")
            || self.child_rollout_id.len() > 128
            || !self
                .child_rollout_id
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || b == b'-')
            || audit["status"] != "finalized"
            || termination["stop_reason"] != "answer"
            || termination["interrupted"] != false
            || termination["deadline_reached"] != false
            || termination["max_hops_reached"] != false
            || termination["final_answer_sha256"].as_str() != Some(self.answer_sha256.as_str())
            || audit["task_binding"]["prompt_sha256"].as_str() != Some(self.task_sha256.as_str())
            || routes["completed_actions"].as_u64().unwrap_or(0) == 0
            || routes["unresolved_actions"] != 0
            || !coverage.complete
        {
            return Err("delegate artifact lacks complete bound native execution".into());
        }
        Ok(())
    }
}

impl LinkedOperation {
    pub(crate) fn validate(&self) -> Result<(), String> {
        self.artifact.validate()?;
        if self.operation_id.is_empty()
            || self.operation_id.len() > 128
            || !self
                .operation_id
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || b == b'-')
            || !digest(&self.arguments_sha256, &[64])
            || !matches!(
                (self.tool.as_str(), self.application.is_some()),
                ("delegate", false) | ("integrate", true)
            )
            || self.application.as_ref().is_some_and(|a| {
                [
                    &a.before_commit,
                    &a.before_tree,
                    &a.after_commit,
                    &a.after_tree,
                ]
                .iter()
                .any(|s| !digest(s, &[40, 64]))
            })
        {
            return Err("invalid linked native operation".into());
        }
        Ok(())
    }
}

/// Passed only by the registry at actual tool entry. The model cannot supply
/// these artifacts in tool arguments or fabricate them through result text.
#[derive(Default)]
pub struct NativeToolContext {
    artifacts: Arc<Mutex<HashMap<String, DelegateArtifact>>>,
}

impl NativeToolContext {
    pub(crate) fn artifact(&self, branch: &str) -> Option<DelegateArtifact> {
        self.artifacts
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .get(branch)
            .cloned()
    }
    pub(crate) fn remember(&self, branch: String, artifact: DelegateArtifact) {
        let mut artifacts = self.artifacts.lock().unwrap_or_else(|p| p.into_inner());
        // This is a bounded process-local handoff. Eviction loses attribution,
        // never permission to keep working or integrate a selected commit.
        if artifacts.len() >= 64 {
            artifacts.clear();
        }
        artifacts.insert(branch, artifact);
    }
}

pub struct NativeToolResult {
    pub(crate) text: String,
    pub(crate) artifact: Option<DelegateArtifact>,
    pub(crate) branch: Option<String>,
    pub(crate) application: Option<DelegateApplication>,
}

impl NativeToolResult {
    pub(crate) fn plain(text: String) -> Self {
        Self {
            text,
            artifact: None,
            branch: None,
            application: None,
        }
    }
}
