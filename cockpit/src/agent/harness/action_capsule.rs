//! Action capsules — a lean, terminal-native preview and receipt for side effects.
//!
//! The cockpit already has a rich tool loop. This module deliberately does not
//! add a planning model, a preflight subprocess, a git scan, or text to the
//! model's history. It only classifies arguments the model has already supplied,
//! renders a bounded human preview, and records local timing at the existing
//! turn boundary. That makes the feature measurable without slowing ordinary
//! read-heavy work.

use super::*;

/// Runtime posture for action capsules. `Off` is a true fast path: callers do
/// not classify calls or allocate previews. `Observe` shows UI-only previews and
/// receipts; `Approve` additionally pauses the interactive root turn once per
/// batch through the existing token-free approval broker.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) enum ActionCapsuleMode {
    Off,
    Observe,
    #[default]
    Approve,
}

impl ActionCapsuleMode {
    /// Parse the one public knob. Keep this pure so the policy has exhaustive
    /// tests without mutating process-wide environment state.
    pub(crate) fn parse(raw: Option<&str>) -> Self {
        match raw
            .unwrap_or("approve")
            .trim()
            .to_ascii_lowercase()
            .as_str()
        {
            "0" | "false" | "off" | "no" | "disable" | "disabled" => Self::Off,
            "observe" | "preview" => Self::Observe,
            "1" | "true" | "on" | "yes" | "approve" => Self::Approve,
            _ => Self::Approve,
        }
    }

    pub(crate) fn active(self) -> bool {
        !matches!(self, Self::Off)
    }

    pub(crate) fn needs_approval(self) -> bool {
        matches!(self, Self::Approve)
    }
}

/// The mode for one harness turn. Only the interactive root registry is opted
/// in; task mode, delegates, swarm seats, and evaluators retain their existing
/// unattended behavior even if the environment happens to contain the knob.
pub(crate) fn mode_for(registry: &ToolRegistry) -> ActionCapsuleMode {
    // Full YOLO and Smart YOLO both skip interactive action-capsule modals so
    // coding turns are not blocked on y/a/n for every shell/write batch. Smart
    // keeps sandbox/timeouts/hooks; only the modal path is suppressed here.
    if crate::platform::yolo::workspace_power() || !registry.action_capsules_enabled() {
        return ActionCapsuleMode::Off;
    }
    ActionCapsuleMode::parse(std::env::var("ANGEL_ACTION_CAPSULES").ok().as_deref())
}

/// Preserve the existing footprint scheduler except while an interactive
/// approval can block. Observation is deliberately non-blocking, so disjoint
/// writes retain the harness's existing parallel speedup.
pub(crate) fn parallel_allowed(
    mode: ActionCapsuleMode,
    boundary: &WorkspaceBoundary,
    calls: &[ToolCall],
) -> bool {
    !mode.needs_approval() && batch_parallelizable_in(boundary, calls)
}

/// A declarative, no-I/O description of one operation the current batch may
/// change or execute. It intentionally contains no file preimage, diff, or raw
/// payload: generating those would add a read/subprocess to the command chain.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct ActionPreview {
    pub(crate) tool: String,
    pub(crate) scope: String,
    command_like: bool,
}

impl ActionPreview {
    /// Classify only operations for which an honest preview is available from
    /// the arguments already parsed by the tool loop. Reads and verifiers are
    /// intentionally absent, preserving their zero-extra-work path.
    pub(crate) fn from_call(call: &ToolCall) -> Option<Self> {
        let path = || call.args.get("path").and_then(Value::as_str).unwrap_or("?");
        let text_len = |key: &str| {
            call.args
                .get(key)
                .and_then(Value::as_str)
                .map(str::len)
                .unwrap_or(0)
        };
        let preview = match call.name.as_str() {
            "write_file" => Self {
                tool: call.name.clone(),
                scope: format!("write {} ({} B payload)", path(), text_len("content")),
                command_like: false,
            },
            "str_replace" => Self {
                tool: call.name.clone(),
                scope: format!(
                    "replace once in {} ({} B → {} B)",
                    path(),
                    text_len("old"),
                    text_len("new")
                ),
                command_like: false,
            },
            "multi_edit" => Self {
                tool: call.name.clone(),
                scope: format!(
                    "apply {} atomic edit(s) to {}",
                    call.args
                        .get("edits")
                        .and_then(Value::as_array)
                        .map(Vec::len)
                        .unwrap_or(0),
                    path()
                ),
                command_like: false,
            },
            "apply_patch" => Self {
                tool: call.name.clone(),
                scope: format!("apply multi-file patch ({} B payload)", text_len("diff")),
                command_like: false,
            },
            "fmt" if call.args.get("check").and_then(Value::as_bool) != Some(true) => Self {
                tool: call.name.clone(),
                scope: "format the workspace in place".to_string(),
                command_like: false,
            },
            "shell" => Self {
                tool: call.name.clone(),
                scope: format!(
                    "run shell command: {}",
                    bounded_command(
                        crate::agent::tools::shell::shell_command_arg(&call.args).unwrap_or("")
                    )
                ),
                command_like: true,
            },
            "cargo" => Self {
                tool: call.name.clone(),
                scope: format!(
                    "run cargo {}",
                    bounded_command(call.args.get("args").and_then(Value::as_str).unwrap_or(""))
                ),
                command_like: true,
            },
            "machine_test" => Self {
                tool: call.name.clone(),
                scope: format!(
                    "queue remote machine test: {}",
                    bounded_command(
                        call.args
                            .get("command")
                            .and_then(Value::as_str)
                            .unwrap_or("")
                    )
                ),
                command_like: true,
            },
            "http_request"
                if !matches!(
                    call.args
                        .get("method")
                        .and_then(Value::as_str)
                        .unwrap_or("GET")
                        .to_ascii_uppercase()
                        .as_str(),
                    "GET" | "HEAD"
                ) =>
            {
                Self {
                    tool: call.name.clone(),
                    scope: format!(
                        "send mutating HTTP request to {}",
                        bounded_command(
                            call.args.get("url").and_then(Value::as_str).unwrap_or("?")
                        )
                    ),
                    command_like: true,
                }
            }
            // Read-only code-mode is bounded reconnaissance, not an action.
            // Effectful mode requires an explicit outer capability and gets one
            // conservative capsule, avoiding a modal storm from nested calls.
            "code_mode"
                if call.args.get("allow_effects").and_then(Value::as_bool) == Some(true) =>
            {
                Self {
                    tool: call.name.clone(),
                    scope: format!(
                        "run effectful programmatic tool batch ({} B script)",
                        text_len("script"),
                    ),
                    command_like: true,
                }
            }
            "code_mode" => return None,
            "integrate" => Self {
                tool: call.name.clone(),
                scope: "merge a parked delegate branch into the workspace".to_string(),
                command_like: false,
            },
            _ => return None,
        };
        Some(preview)
    }

    pub(crate) fn receipt(&self, result: &str, elapsed_ms: u128) -> String {
        let verb = if result.starts_with("tool error:") {
            "dispatch error"
        } else if self.command_like {
            // Shell/cargo text is not a verifier protocol; a successful tool
            // dispatch only proves the command ran, not that its task passed.
            "ran"
        } else {
            "applied"
        };
        let reason = failure_reason(result);
        let reason = reason
            .map(|reason| format!(" · {reason}"))
            .unwrap_or_default();
        format!(
            "action receipt · {} {verb} · {elapsed_ms} ms{reason}",
            self.tool
        )
    }
}

/// A fixed reason class for a failed tool result. Fixed classes cannot leak
/// command bodies, paths or credentials, so receipts and the tool strip can
/// both show them.
pub(crate) fn failure_reason(result: &str) -> Option<&'static str> {
    let error = result.strip_prefix("tool error:")?;
    let first = error
        .split(['\n', ';'])
        .next()
        .unwrap_or("")
        .trim()
        .to_ascii_lowercase();
    Some(
        if first.contains("mount") || first.contains("not confined") {
            "sandbox: mount not confined"
        } else if first.contains("enoent") || first.contains("no such file") {
            "helper: ENOENT"
        } else if first.contains("timeout") || first.contains("timed out") {
            "timeout"
        } else if first.contains("sandbox") || first.contains("confinement") {
            "sandbox: confinement failed"
        } else if first.contains("denied") || first.contains("permission") {
            "policy: denied"
        } else {
            "dispatch: failed"
        },
    )
}

/// All action previews in one model-emitted tool batch. A single batch is the
/// approval unit so an agent does not incur one UI round trip per file.
#[derive(Clone, Debug)]
pub(crate) struct ActionBatch {
    items: Vec<(usize, ActionPreview)>,
    key: String,
}

impl ActionBatch {
    pub(crate) fn from_calls(
        calls: &[ToolCall],
        mode: ActionCapsuleMode,
        workspace: &Path,
    ) -> Option<Self> {
        if !mode.active() {
            return None;
        }
        let items: Vec<(usize, ActionPreview)> = calls
            .iter()
            .enumerate()
            .filter_map(|(index, call)| {
                ActionPreview::from_call(call).map(|preview| (index, preview))
            })
            .collect();
        if items.is_empty() {
            return None;
        }
        // The key includes the complete parsed arguments, not the bounded UI
        // summary. `approve all` is therefore limited to an identical batch in
        // this turn, never all writes of a broad tool kind.
        let material = calls
            .iter()
            .enumerate()
            .filter(|(index, _)| items.iter().any(|(item_index, _)| item_index == index))
            .map(|(_, call)| format!("{}\n{}", call.name, call.args))
            .collect::<Vec<_>>()
            .join("\n---\n");
        Some(Self {
            items,
            key: format!(
                "action-capsule:{}",
                crate::knowledge::cut::sha256_hex(
                    format!(
                        "{:?}\n{}\n{material}",
                        workspace,
                        crate::platform::yolo::profile().label()
                    )
                    .as_bytes()
                )
            ),
        })
    }

    pub(crate) fn contains(&self, index: usize) -> Option<&ActionPreview> {
        self.items
            .iter()
            .find_map(|(item_index, preview)| (*item_index == index).then_some(preview))
    }

    pub(crate) fn count(&self) -> usize {
        self.items.len()
    }

    pub(crate) fn approval_key(&self) -> &str {
        &self.key
    }

    pub(crate) fn preview_notice(&self) -> String {
        format!(
            "action capsule · {} scoped operation(s) ready · local preview only, no model call",
            self.count()
        )
    }

    pub(crate) fn approval_prompt(&self) -> String {
        const MAX_ROWS: usize = 4;
        let mut lines = Vec::new();
        for (_, item) in self.items.iter().take(MAX_ROWS) {
            lines.push(format!("• {}", item.scope));
        }
        if self.items.len() > MAX_ROWS {
            lines.push(format!(
                "• +{} additional scoped operation(s)",
                self.items.len() - MAX_ROWS
            ));
        }
        format!(
            "Action capsule — {} operation(s):\n{}\n\nApprove this bounded batch? No verifier or extra command will run automatically.",
            self.count(),
            lines.join("\n")
        )
    }
}

/// Turn-level, aggregate-only instrumentation. One existing turn-ledger write
/// carries these fields at completion; no per-action receipt file hits the inner
/// loop.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) struct ActionCapsuleMetrics {
    /// Number of scoped tool operations across previewed batches.
    pub(crate) operations: usize,
    pub(crate) previews: usize,
    pub(crate) denied: usize,
    pub(crate) preflight_us: u64,
    /// Human modal wait in explicit approve mode. Kept separate from local
    /// harness work so rollout analysis does not mistake operator choice time
    /// for command-chain overhead.
    pub(crate) approval_wait_ms: u64,
    pub(crate) action_exec_ms: u64,
}

impl ActionCapsuleMetrics {
    pub(crate) fn note_preflight(&mut self, elapsed: Duration, operations: usize) {
        self.previews = self.previews.saturating_add(1);
        self.operations = self.operations.saturating_add(operations);
        self.preflight_us = self
            .preflight_us
            .saturating_add(elapsed.as_micros().min(u128::from(u64::MAX)) as u64);
    }

    pub(crate) fn note_denied(&mut self, count: usize) {
        self.denied = self.denied.saturating_add(count);
    }

    pub(crate) fn note_approval_wait(&mut self, elapsed: Duration) {
        self.approval_wait_ms = self
            .approval_wait_ms
            .saturating_add(elapsed.as_millis().min(u128::from(u64::MAX)) as u64);
    }

    pub(crate) fn note_execution(&mut self, elapsed: Duration) {
        self.action_exec_ms = self
            .action_exec_ms
            .saturating_add(elapsed.as_millis().min(u128::from(u64::MAX)) as u64);
    }
}

fn bounded_command(raw: &str) -> String {
    const MAX: usize = 96;
    let scrubbed = crate::knowledge::experience::scrub_secrets(raw);
    if scrubbed.chars().count() <= MAX {
        return scrubbed;
    }
    let mut out = scrubbed
        .chars()
        .take(MAX.saturating_sub(1))
        .collect::<String>();
    out.push('…');
    out
}

#[cfg(test)]
#[path = "../../../../tests/cockpit/harness/action_capsule__tests.rs"]
mod tests;

#[cfg(test)]
#[path = "../../../../tests/cockpit/harness/action_capsule__receipt_reason_tests.rs"]
mod receipt_reason_tests;
