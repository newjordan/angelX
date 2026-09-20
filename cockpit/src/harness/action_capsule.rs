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
    if crate::yolo::workspace_power() || !registry.action_capsules_enabled() {
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
                        crate::tools::shell::shell_command_arg(&call.args).unwrap_or("")
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
        let reason = result.strip_prefix("tool error:").map(|error| {
            let first = error
                .split(['\n', ';'])
                .next()
                .unwrap_or("")
                .trim()
                .to_ascii_lowercase();
            // Fixed reason classes cannot leak command bodies, paths, or credentials.
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
            }
        });
        let reason = reason
            .map(|reason| format!(" · {reason}"))
            .unwrap_or_default();
        format!(
            "action receipt · {} {verb} · {elapsed_ms} ms{reason}",
            self.tool
        )
    }
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
                crate::cut::sha256_hex(
                    format!(
                        "{:?}\n{}\n{material}",
                        workspace,
                        crate::yolo::profile().label()
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
    let scrubbed = crate::experience::scrub_secrets(raw);
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
mod tests {
    fn scope_for(call: ToolCall, workspace: &Path) -> crate::approval::ApprovalScope {
        crate::approval::ApprovalScope::ActionBatch(
            ActionBatch::from_calls(&[call], ActionCapsuleMode::Approve, workspace)
                .unwrap()
                .approval_key()
                .to_owned(),
        )
    }

    #[test]
    fn adversarial_approval_cargo_suffix_and_manifest_reuse_denied() {
        let _lock = crate::tests::env_lock();
        let workspace = Path::new("/fixture/workspace");
        let baseline = scope_for(
            call("shell", serde_json::json!({"command":"cargo test"})),
            workspace,
        );
        assert_eq!(
            crate::approval::test_reuse(baseline.clone(), baseline.clone()),
            crate::approval::Decision::Approve
        );
        for command in [
            "cargo test; rm -rf ../victim",
            "cargo test --manifest-path ../elsewhere/Cargo.toml",
        ] {
            let next = scope_for(
                call("shell", serde_json::json!({"command":command})),
                workspace,
            );
            assert_eq!(
                crate::approval::test_reuse(baseline.clone(), next),
                crate::approval::Decision::Deny
            );
        }
    }

    #[test]
    fn adversarial_approval_tool_name_and_payload_are_not_transferable() {
        let _lock = crate::tests::env_lock();
        let workspace = Path::new("/fixture/workspace");
        let shell = scope_for(
            call("shell", serde_json::json!({"command":"cargo test"})),
            workspace,
        );
        // Typed verifiers do not consume capsule grants at all. Their own
        // pinned-argv policy authorizes execution independently of shell results.
        assert!(
            ActionBatch::from_calls(
                &[call("run_tests", serde_json::json!({}))],
                ActionCapsuleMode::Approve,
                workspace
            )
            .is_none()
        );
        let patch = scope_for(
            call(
                "apply_patch",
                serde_json::json!({"diff":"*** Begin Patch\n*** Add File: src/a\n+x\n*** End Patch"}),
            ),
            workspace,
        );
        assert_eq!(
            crate::approval::test_reuse(shell.clone(), patch),
            crate::approval::Decision::Deny
        );
        let renamed = scope_for(
            call("cargo", serde_json::json!({"command":"cargo test"})),
            workspace,
        );
        assert_eq!(
            crate::approval::test_reuse(shell, renamed),
            crate::approval::Decision::Deny
        );
    }

    #[cfg(unix)]
    #[test]
    fn adversarial_approval_write_reuse_cannot_escape_via_parent_or_symlink() {
        use crate::harness::Tool;
        let _lock = crate::tests::env_lock();
        let _full = crate::tests::TestEnvGuard::set("ANGEL_YOLO", "0");
        let fixture = std::env::temp_dir().join(format!(
            "angel-s03-reuse-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let root = fixture.join("workspace");
        let outside = fixture.join("outside");
        std::fs::create_dir_all(root.join("src")).unwrap();
        std::fs::create_dir(&outside).unwrap();
        let tool = crate::tools::file::WriteFileTool { root: root.clone() };
        let args = serde_json::json!({"path":"src/a", "content":"allowed"});
        let baseline = scope_for(call("write_file", args.clone()), &root);
        assert!(tool.call(&args).is_ok());
        let escape = serde_json::json!({"path":"../outside/a", "content":"allowed"});
        assert_eq!(
            crate::approval::test_reuse(
                baseline.clone(),
                scope_for(call("write_file", escape.clone()), &root)
            ),
            crate::approval::Decision::Deny
        );
        assert!(tool.call(&escape).is_err());
        // Same lexical arguments and cached approval after an ancestor changes:
        // the actual descriptor-based write must still reject the escape.
        std::fs::rename(root.join("src"), root.join("original-src")).unwrap();
        std::os::unix::fs::symlink(&outside, root.join("src")).unwrap();
        assert_eq!(
            crate::approval::test_reuse(
                baseline.clone(),
                scope_for(call("write_file", args.clone()), &root)
            ),
            crate::approval::Decision::Approve
        );
        assert!(tool.call(&args).is_err());
        assert!(!outside.join("a").exists());
        assert_eq!(
            std::fs::read_to_string(root.join("original-src/a")).unwrap(),
            "allowed"
        );
        std::fs::remove_dir_all(fixture).unwrap();
    }

    #[test]
    fn adversarial_approval_command_workspace_target_and_effect_changes_invalidate_batch() {
        let _lock = crate::tests::env_lock();
        let original = call(
            "shell",
            serde_json::json!({"command":"printf local", "write_paths":["a.rs"]}),
        );
        let key = |calls: &[ToolCall], workspace: &Path| {
            ActionBatch::from_calls(calls, ActionCapsuleMode::Approve, workspace)
                .unwrap()
                .approval_key()
                .to_owned()
        };
        let workspace = Path::new("/fixture/one");
        let baseline = key(std::slice::from_ref(&original), workspace);
        assert_eq!(baseline, key(std::slice::from_ref(&original), workspace));
        assert_ne!(
            baseline,
            key(std::slice::from_ref(&original), Path::new("/fixture/two"))
        );
        for args in [
            serde_json::json!({"command":"printf changed", "write_paths":["a.rs"]}),
            serde_json::json!({"command":"printf local", "write_paths":["b.rs"]}),
            serde_json::json!({"command":"printf local", "write_paths":[]}),
            serde_json::json!({"command":"printf local"}),
        ] {
            assert_ne!(baseline, key(&[call("shell", args)], workspace));
        }
    }

    #[test]
    fn adversarial_approval_profile_change_invalidates_batch() {
        let _lock = crate::tests::env_lock();
        let calls = [call("shell", serde_json::json!({"command":"printf local"}))];
        let _guarded = crate::tests::TestEnvGuard::set("ANGEL_YOLO", "0");
        let _smart_off = crate::tests::TestEnvGuard::set("ANGEL_YOLO_SMART", "0");
        let guarded = ActionBatch::from_calls(
            &calls,
            ActionCapsuleMode::Approve,
            Path::new("/fixture/one"),
        )
        .unwrap();
        let _full = crate::tests::TestEnvGuard::set("ANGEL_YOLO", "1");
        let full = ActionBatch::from_calls(
            &calls,
            ActionCapsuleMode::Approve,
            Path::new("/fixture/one"),
        )
        .unwrap();
        assert_ne!(guarded.approval_key(), full.approval_key());
    }

    use super::*;

    fn call(name: &str, args: Value) -> ToolCall {
        ToolCall {
            id: "call_1".to_string(),
            name: name.to_string(),
            args,
        }
    }

    #[test]
    fn mode_defaults_to_approval_and_accepts_explicit_off_or_observe() {
        assert_eq!(ActionCapsuleMode::parse(None), ActionCapsuleMode::Approve);
        assert_eq!(ActionCapsuleMode::parse(Some("0")), ActionCapsuleMode::Off);
        assert_eq!(
            ActionCapsuleMode::parse(Some("observe")),
            ActionCapsuleMode::Observe
        );
        assert_eq!(
            ActionCapsuleMode::parse(Some("on")),
            ActionCapsuleMode::Approve
        );
        assert_eq!(
            ActionCapsuleMode::parse(Some("approve")),
            ActionCapsuleMode::Approve
        );
        assert_eq!(
            ActionCapsuleMode::parse(Some("unexpected")),
            ActionCapsuleMode::Approve
        );
    }

    #[test]
    fn yolo_disables_interactive_action_capsules() {
        let _guard = crate::tests::env_lock();
        let previous = std::env::var_os("ANGEL_YOLO");
        let previous_smart = std::env::var_os("ANGEL_YOLO_SMART");
        // TODO: Audit that the environment access only happens in single-threaded code.
        unsafe { std::env::set_var("ANGEL_YOLO", "1") };
        // TODO: Audit that the environment access only happens in single-threaded code.
        unsafe { std::env::remove_var("ANGEL_YOLO_SMART") };
        let mut registry = ToolRegistry::new();
        registry.enable_action_capsules();
        assert_eq!(mode_for(&registry), ActionCapsuleMode::Off);
        match previous {
            // TODO: Audit that the environment access only happens in single-threaded code.
            Some(value) => unsafe { std::env::set_var("ANGEL_YOLO", value) },
            // TODO: Audit that the environment access only happens in single-threaded code.
            None => unsafe { std::env::remove_var("ANGEL_YOLO") },
        }
        match previous_smart {
            // TODO: Audit that the environment access only happens in single-threaded code.
            Some(value) => unsafe { std::env::set_var("ANGEL_YOLO_SMART", value) },
            // TODO: Audit that the environment access only happens in single-threaded code.
            None => unsafe { std::env::remove_var("ANGEL_YOLO_SMART") },
        }
    }

    #[test]
    fn smart_yolo_also_disables_interactive_action_capsules() {
        let _guard = crate::tests::env_lock();
        let _full = crate::tests::TestEnvGuard::unset("ANGEL_YOLO");
        let _smart = crate::tests::TestEnvGuard::set("ANGEL_YOLO_SMART", "1");
        let mut registry = ToolRegistry::new();
        registry.enable_action_capsules();
        assert_eq!(mode_for(&registry), ActionCapsuleMode::Off);
        assert!(!crate::yolo::enabled());
        assert!(crate::yolo::smart_enabled());
    }

    #[test]
    fn previews_are_argument_only_and_reads_stay_unclassified() {
        let write = ActionPreview::from_call(&call(
            "write_file",
            serde_json::json!({"path":"src/lib.rs","content":"abc"}),
        ))
        .unwrap();
        assert_eq!(write.scope, "write src/lib.rs (3 B payload)");
        assert!(
            ActionPreview::from_call(&call("read_file", serde_json::json!({"path":"src/lib.rs"}),))
                .is_none()
        );

        let shell = ActionPreview::from_call(&call(
            "shell",
            serde_json::json!({"command":"API_KEY=secret cargo test"}),
        ))
        .unwrap();
        assert!(!shell.scope.contains("secret"));
        assert!(shell.scope.contains("API_KEY=…"));

        let legacy_shell =
            ActionPreview::from_call(&call("shell", serde_json::json!({"cmd":"cargo check"})))
                .unwrap();
        assert!(legacy_shell.scope.contains("cargo check"));

        let machine_test = ActionPreview::from_call(&call(
            "machine_test",
            serde_json::json!({"command":"swift test --filter SchedulerTests"}),
        ))
        .unwrap();
        assert!(machine_test.scope.contains("queue remote machine test"));
        assert!(machine_test.scope.contains("SchedulerTests"));

        assert!(
            ActionPreview::from_call(&call(
                "code_mode",
                serde_json::json!({"script":"return grep({pattern:'x'});"}),
            ))
            .is_none(),
            "bounded read-only reconnaissance must not request action approval"
        );
        let effectful_code = ActionPreview::from_call(&call(
            "code_mode",
            serde_json::json!({"script":"return shell({command:'true'});","allow_effects":true}),
        ))
        .unwrap();
        assert!(effectful_code.scope.contains("effectful programmatic"));

        assert!(
            ActionPreview::from_call(&call(
                "http_request",
                serde_json::json!({"method":"GET","url":"https://example.com"}),
            ))
            .is_none(),
            "read-only HTTP must not request mutation approval"
        );
        let mutating_http = ActionPreview::from_call(&call(
            "http_request",
            serde_json::json!({"method":"POST","url":"https://api.example.com/jobs"}),
        ))
        .unwrap();
        assert!(mutating_http.scope.contains("api.example.com/jobs"));
    }

    #[test]
    fn batch_is_scoped_to_exact_effectful_calls() {
        let _lock = crate::tests::env_lock();
        let calls = vec![
            call("read_file", serde_json::json!({"path":"a.rs"})),
            call(
                "str_replace",
                serde_json::json!({"path":"a.rs","old":"a","new":"b"}),
            ),
            call("shell", serde_json::json!({"command":"cargo test"})),
        ];
        let batch = ActionBatch::from_calls(
            &calls,
            ActionCapsuleMode::Approve,
            Path::new("/fixture/one"),
        )
        .unwrap();
        assert_eq!(batch.count(), 2);
        assert!(batch.contains(0).is_none());
        assert!(batch.contains(1).is_some());
        assert!(batch.contains(2).is_some());
        assert!(batch.approval_prompt().contains("replace once in a.rs"));
        assert!(batch.approval_prompt().contains("run shell command"));

        let changed = vec![
            calls[0].clone(),
            call(
                "str_replace",
                serde_json::json!({"path":"a.rs","old":"a","new":"c"}),
            ),
            calls[2].clone(),
        ];
        let changed_batch = ActionBatch::from_calls(
            &changed,
            ActionCapsuleMode::Approve,
            Path::new("/fixture/one"),
        )
        .unwrap();
        assert_ne!(batch.approval_key(), changed_batch.approval_key());
    }

    #[test]
    fn receipt_never_claims_command_success() {
        let shell =
            ActionPreview::from_call(&call("shell", serde_json::json!({"command":"false"})))
                .unwrap();
        assert!(shell.receipt("exit 1", 7).contains("ran"));
        assert!(
            shell
                .receipt("tool error: spawn failed", 7)
                .contains("dispatch error")
        );
    }

    #[test]
    fn metrics_aggregate_without_per_action_storage() {
        let mut metrics = ActionCapsuleMetrics::default();
        metrics.note_preflight(Duration::from_micros(12), 3);
        metrics.note_execution(Duration::from_millis(8));
        metrics.note_denied(2);
        metrics.note_approval_wait(Duration::from_millis(25));
        assert_eq!(metrics.previews, 1);
        assert_eq!(metrics.operations, 3);
        assert_eq!(metrics.preflight_us, 12);
        assert_eq!(metrics.action_exec_ms, 8);
        assert_eq!(metrics.denied, 2);
        assert_eq!(metrics.approval_wait_ms, 25);
    }

    #[test]
    fn observe_keeps_disjoint_write_parallelism_but_approve_serializes_modals() {
        let boundary = WorkspaceBoundary::new(Path::new("."));
        let calls = vec![
            call(
                "write_file",
                serde_json::json!({"path":"a.rs","content":"a"}),
            ),
            call(
                "write_file",
                serde_json::json!({"path":"b.rs","content":"b"}),
            ),
        ];
        assert!(parallel_allowed(ActionCapsuleMode::Off, &boundary, &calls));
        assert!(parallel_allowed(
            ActionCapsuleMode::Observe,
            &boundary,
            &calls
        ));
        assert!(!parallel_allowed(
            ActionCapsuleMode::Approve,
            &boundary,
            &calls
        ));
    }
}

#[cfg(test)]
mod receipt_reason_tests {
    use super::*;
    #[test]
    fn receipt_dispatch_short_reason() {
        let preview = ActionPreview {
            tool: "shell".into(),
            scope: String::new(),
            command_like: true,
        };
        for (error, reason) in [
            (
                "sandbox: mount not confined; private body",
                "sandbox: mount not confined",
            ),
            ("spawn failed: ENOENT\nprivate body", "helper: ENOENT"),
            ("timed out; private body", "timeout"),
        ] {
            let receipt = preview.receipt(&format!("tool error: {error}"), 12);
            assert_eq!(
                receipt,
                format!("action receipt · shell dispatch error · 12 ms · {reason}")
            );
        }
    }
}
