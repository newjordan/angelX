//! code_mode registry orchestration, budgets, hooks, recipes, and receipts.
//!
//! Extracted from `harness/tests.rs` without behavior change so the monolith can
//! shrink while preserving the full harness test inventory. Shared helpers
//! (`EnvGuard`) and utility tools (`ReverseTool`, `WordCountTool`) remain in the
//! parent / harness crate surface.

use super::*;

// --- code_mode suite --------------------------------------------------------

struct CodeModeEffectProbe;

impl Tool for CodeModeEffectProbe {
    fn name(&self) -> &str {
        "effect_probe"
    }

    fn def(&self) -> ToolDef {
        ToolDef {
            name: self.name().to_string(),
            description: "test-only effect probe".to_string(),
            params: serde_json::json!({"type":"object"}),
        }
    }

    fn call(&self, args: &Value) -> Result<String, String> {
        match args.get("bytes").and_then(Value::as_u64) {
            Some(bytes) => Ok("x".repeat(bytes as usize)),
            None => Ok("effect-ran".to_string()),
        }
    }
}

struct CodeModeNamedProbe(&'static str);

impl Tool for CodeModeNamedProbe {
    fn name(&self) -> &str {
        self.0
    }

    fn def(&self) -> ToolDef {
        ToolDef {
            name: self.name().to_string(),
            description: "test-only named capability probe".to_string(),
            params: serde_json::json!({"type":"object"}),
        }
    }

    fn call(&self, _args: &Value) -> Result<String, String> {
        Ok("external-probe-ran".to_string())
    }
}

struct CodeModeBatchState {
    active: [std::sync::atomic::AtomicUsize; 2],
    max_active: [std::sync::atomic::AtomicUsize; 2],
    barrier_seen: std::sync::atomic::AtomicBool,
}

impl Default for CodeModeBatchState {
    fn default() -> Self {
        Self {
            active: std::array::from_fn(|_| std::sync::atomic::AtomicUsize::new(0)),
            max_active: std::array::from_fn(|_| std::sync::atomic::AtomicUsize::new(0)),
            barrier_seen: std::sync::atomic::AtomicBool::new(false),
        }
    }
}

struct CodeModeBatchProbe {
    name: &'static str,
    barrier: bool,
    state: std::sync::Arc<CodeModeBatchState>,
}

impl Tool for CodeModeBatchProbe {
    fn name(&self) -> &str {
        self.name
    }

    fn def(&self) -> ToolDef {
        ToolDef {
            name: self.name().to_string(),
            description: "test-only segmented batch probe".to_string(),
            params: serde_json::json!({"type":"object"}),
        }
    }

    fn call(&self, args: &Value) -> Result<String, String> {
        use std::sync::atomic::Ordering;

        if self.barrier {
            if self.state.active[0].load(Ordering::SeqCst) != 0 {
                return Err("effect barrier crossed the first safe run".to_string());
            }
            self.state.barrier_seen.store(true, Ordering::SeqCst);
            return Ok("barrier".to_string());
        }
        let phase = args
            .get("phase")
            .and_then(Value::as_u64)
            .unwrap_or_default() as usize;
        if phase == 1 && !self.state.barrier_seen.load(Ordering::SeqCst) {
            return Err("second safe run crossed the effect barrier".to_string());
        }
        let now = self.state.active[phase].fetch_add(1, Ordering::SeqCst) + 1;
        self.state.max_active[phase].fetch_max(now, Ordering::SeqCst);
        std::thread::sleep(std::time::Duration::from_millis(30));
        self.state.active[phase].fetch_sub(1, Ordering::SeqCst);
        Ok(args
            .get("label")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_string())
    }
}

#[test]
fn code_mode_orchestrates_real_registry_tools() {
    // End-to-end: a script that loops over the REAL `reverse` tool, proving
    // the dispatch interception + V8 host-binding + registry re-entry all
    // connect (the code_mode.rs unit tests only use a mock invoke).
    let mut reg = ToolRegistry::new();
    reg.register(Box::new(ReverseTool));
    reg.register(Box::new(WordCountTool));
    let names = reg.bindable_tool_names();
    reg.register(Box::new(CodeModeTool::new(&names)));
    let empty = Hooks::default();

    let script = "const ws=['ab','cd','ef']; const out=[]; \
                      for (const w of ws) { out.push(reverse({text:w})); } \
                      return out.join(',');";
    let args = serde_json::json!({ "script": script });
    let r = dispatch_with_hooks(&reg, &empty, "code_mode", &args);
    assert!(
        r.starts_with("ba,dc,fe\n--- code_mode receipt:"),
        "got: {r}"
    );
    assert!(r.contains("nested_calls=3"), "got: {r}");
    assert!(r.contains("allow_effects=false"), "got: {r}");
}

#[test]
fn code_mode_mixed_batch_parallelizes_safe_segments_around_effect_barrier() {
    use std::sync::atomic::Ordering;

    let _lock = crate::tests::env_lock();
    let _effects = EnvGuard::set("ANGEL_CODE_MODE_EFFECTS", "1");
    let state = std::sync::Arc::new(CodeModeBatchState::default());
    let mut reg = ToolRegistry::new();
    for (name, barrier) in [
        ("reverse", false),
        ("word_count", false),
        ("effect_probe", true),
    ] {
        reg.register(Box::new(CodeModeBatchProbe {
            name,
            barrier,
            state: std::sync::Arc::clone(&state),
        }));
    }
    let names = reg.bindable_tool_names();
    reg.register(Box::new(CodeModeTool::new(&names)));

    let result = dispatch_with_hooks(
        &reg,
        &Hooks::default(),
        "code_mode",
        &serde_json::json!({
            "allow_effects": true,
            "script": "const r=batch([
                {tool:'reverse',args:{phase:0,label:'a'}},
                {tool:'word_count',args:{phase:0,label:'b'}},
                {tool:'effect_probe',args:{}},
                {tool:'reverse',args:{phase:1,label:'c'}},
                {tool:'word_count',args:{phase:1,label:'d'}}
            ]); return r.map(x=>x.ok?x.output:'ERR:'+x.error).join(',');"
        }),
    );

    assert!(
        result.starts_with("a,b,barrier,c,d\n--- code_mode receipt:"),
        "{result}"
    );
    assert_eq!(state.max_active[0].load(Ordering::SeqCst), 2);
    assert_eq!(state.max_active[1].load(Ordering::SeqCst), 2);
}

#[test]
fn code_mode_large_results_offload_to_handle_receipts() {
    let _guard = crate::tests::env_lock();
    let _store = EnvGuard::set("ANGEL_HANDLE_STORE", "1");
    let _min = EnvGuard::set("ANGEL_HANDLE_CODE_MODE_MIN_BYTES", "64");
    session_clear();

    let mut reg = ToolRegistry::new();
    reg.register(Box::new(ReverseTool));
    let names = reg.bindable_tool_names();
    reg.register(Box::new(CodeModeTool::new(&names)));
    let empty = Hooks::default();

    // Build a return value well above the temporary 64-byte offload floor.
    let script = "return 'BULK_MARKER_' + 'z'.repeat(200);";
    let args = serde_json::json!({ "script": script });
    let r = dispatch_with_hooks(&reg, &empty, "code_mode", &args);
    assert!(
        r.contains(HANDLE_RECEIPT_MARK) || r.contains("handle=hnd_"),
        "large code_mode body must become a handle receipt: {r}"
    );
    assert!(
        !r.contains("BULK_MARKER_"),
        "bulk must not enter root-visible code_mode result: {r}"
    );
    assert!(
        r.contains("code_mode receipt:"),
        "strategy receipt retained: {r}"
    );

    // Recover via explicit disclosure.
    let handle = r
        .split("hnd_")
        .nth(1)
        .and_then(|s| s.split(|c: char| !c.is_ascii_alphanumeric()).next())
        .map(|s| format!("hnd_{s}"))
        .expect("handle id");
    let slice = session_disclose(&handle, 0, 512).expect("disclose");
    assert!(slice.content.contains("BULK_MARKER_"));
    session_clear();
}

#[test]
fn code_mode_nested_calls_receive_stable_outer_derived_event_ids() {
    let mut reg = ToolRegistry::new();
    reg.register(Box::new(ReverseTool));
    let names = reg.bindable_tool_names();
    reg.register(Box::new(CodeModeTool::new(&names)));
    let hooks = Hooks::default();
    let (event_tx, event_rx) = std::sync::mpsc::channel();
    let outer_id = ToolEventId("provider-call-7".to_string());
    let args = serde_json::json!({
        "script": "return [reverse({text:'ab'}), reverse({text:'cd'})].join(',');"
    });

    let result = dispatch_with_hooks_events(
        &reg,
        &hooks,
        "code_mode",
        &args,
        Some((&outer_id, &event_tx)),
    );
    assert!(
        result.starts_with("ba,dc\n--- code_mode receipt:"),
        "{result}"
    );

    let events = event_rx.try_iter().collect::<Vec<_>>();
    assert_eq!(
        events.len(),
        4,
        "nested calls must emit paired start/result events"
    );
    for (pair_index, pair) in events.chunks_exact(2).enumerate() {
        let expected = ToolEventId(format!("provider-call-7:{}", pair_index + 1));
        assert!(matches!(
            &pair[0],
            TurnEvent::ToolCall { id, name, .. }
                if id == &expected && name == "reverse"
        ));
        assert!(matches!(
            &pair[1],
            TurnEvent::ToolResult { id, name, outcome, .. }
                if id == &expected
                    && name == "reverse"
                    && outcome.execution == ExecutionOutcome::Succeeded
        ));
    }
}

#[test]
fn code_mode_requires_explicit_effect_capability() {
    let _lock = crate::tests::env_lock();
    let _effects = EnvGuard::set("ANGEL_CODE_MODE_EFFECTS", "1");
    let mut reg = ToolRegistry::new();
    reg.register(Box::new(CodeModeEffectProbe));
    let names = reg.bindable_tool_names();
    reg.register(Box::new(CodeModeTool::new(&names)));
    let hooks = Hooks::default();
    let script = "return effect_probe({});";

    let blocked = dispatch_with_hooks(
        &reg,
        &hooks,
        "code_mode",
        &serde_json::json!({"script": script}),
    );
    assert!(
        blocked.contains("requires allow_effects=true"),
        "got: {blocked}"
    );
    assert!(blocked.contains("nested_calls=0"), "got: {blocked}");

    let allowed = dispatch_with_hooks(
        &reg,
        &hooks,
        "code_mode",
        &serde_json::json!({"script": script, "allow_effects": true}),
    );
    assert!(
        allowed.starts_with("effect-ran\n--- code_mode receipt:"),
        "got: {allowed}"
    );
    assert!(allowed.contains("nested_calls=1"), "got: {allowed}");
    assert!(allowed.contains("allow_effects=true"), "got: {allowed}");
}

#[test]
fn yolo_code_mode_defaults_to_effectful_and_removes_nested_call_budget() {
    let _lock = crate::tests::env_lock();
    let _yolo = EnvGuard::set("ANGEL_YOLO", "1");
    let _effects = EnvGuard::set("ANGEL_CODE_MODE_EFFECTS", "0");
    let mut reg = ToolRegistry::new();
    reg.register(Box::new(CodeModeEffectProbe));
    let names = reg.bindable_tool_names();
    reg.register(Box::new(CodeModeTool::new(&names)));
    let result = dispatch_with_hooks(
        &reg,
        &Hooks::default(),
        "code_mode",
        &serde_json::json!({
            "script": "let out=''; for(let i=0;i<49;i++){ out=effect_probe({}); } return out;"
        }),
    );
    assert!(
        result.starts_with("effect-ran\n--- code_mode receipt:"),
        "got: {result}"
    );
    assert!(result.contains("nested_calls=49"), "got: {result}");
    assert!(result.contains("allow_effects=true"), "got: {result}");
    assert!(!result.contains("budget exhausted"), "got: {result}");
}

#[test]
fn code_mode_enforces_nested_call_and_output_budgets() {
    let _lock = crate::tests::env_lock();
    let _effects = EnvGuard::set("ANGEL_CODE_MODE_EFFECTS", "1");
    let mut reg = ToolRegistry::new();
    reg.register(Box::new(ReverseTool));
    reg.register(Box::new(CodeModeEffectProbe));
    let names = reg.bindable_tool_names();
    reg.register(Box::new(CodeModeTool::new(&names)));
    let hooks = Hooks::default();

    let call_limited = dispatch_with_hooks(
        &reg,
        &hooks,
        "code_mode",
        &serde_json::json!({
            "script": "for(let i=0;i<49;i++){ reverse({text:'x'}); } return 'unreached';"
        }),
    );
    assert!(
        call_limited.contains("nested-call budget exhausted (48 calls)"),
        "got: {call_limited}"
    );
    assert!(
        call_limited.contains("nested_calls=48"),
        "got: {call_limited}"
    );

    let output_limited = dispatch_with_hooks(
        &reg,
        &hooks,
        "code_mode",
        &serde_json::json!({
            "script": "return effect_probe({bytes:8388609});",
            "allow_effects": true
        }),
    );
    assert!(
        output_limited.contains("nested-output budget exhausted (8388608 B)"),
        "got: {}",
        &output_limited[..output_limited.len().min(512)]
    );
    assert!(
        output_limited.contains("nested_output_bytes=8388609"),
        "missing receipt"
    );

    let oversized_script = " ".repeat(65_537);
    let script_limited = dispatch_with_hooks(
        &reg,
        &hooks,
        "code_mode",
        &serde_json::json!({"script": oversized_script}),
    );
    assert!(
        script_limited.contains("script is 65537 B; limit is 65536 B"),
        "got: {script_limited}"
    );
}

#[test]
fn code_mode_effects_are_operator_quarantined_by_default() {
    let _lock = crate::tests::env_lock();
    let _effects = EnvGuard::set("ANGEL_CODE_MODE_EFFECTS", "0");
    let mut reg = ToolRegistry::new();
    reg.register(Box::new(CodeModeEffectProbe));
    let names = reg.bindable_tool_names();
    reg.register(Box::new(CodeModeTool::new(&names)));
    let result = dispatch_with_hooks(
        &reg,
        &Hooks::default(),
        "code_mode",
        &serde_json::json!({
            "script": "return effect_probe({});",
            "allow_effects": true
        }),
    );
    assert!(
        result.contains("effectful code_mode is quarantined"),
        "got: {result}"
    );
    assert!(!result.contains("effect-ran"), "effect executed: {result}");
}

#[test]
fn code_mode_default_boundary_excludes_network_and_lsp_tools() {
    let _lock = crate::tests::env_lock();
    let _yolo = EnvGuard::unset("ANGEL_YOLO");
    let _smart = EnvGuard::unset("ANGEL_YOLO_SMART");
    let _effects = EnvGuard::set("ANGEL_CODE_MODE_EFFECTS", "0");
    let mut reg = ToolRegistry::new();
    reg.register(Box::new(CodeModeNamedProbe("web_search")));
    reg.register(Box::new(CodeModeNamedProbe("lsp_hover")));
    let names = reg.bindable_tool_names();
    reg.register(Box::new(CodeModeTool::new(&names)));
    for name in ["web_search", "lsp_hover"] {
        let result = dispatch_with_hooks(
            &reg,
            &Hooks::default(),
            "code_mode",
            &serde_json::json!({"script": format!("return tool('{name}', {{}});")}),
        );
        assert!(
            result.contains(&format!(
                "effectful tool `{name}` requires allow_effects=true"
            )),
            "got: {result}"
        );
        assert!(
            !result.contains("external-probe-ran"),
            "tool executed: {result}"
        );
    }
}

#[test]
fn code_mode_nested_call_respects_blocking_hook() {
    // A PreToolUse hook that blocks `reverse` must also block it when the
    // call originates from inside a code_mode script (nested dispatch goes
    // through the same hook gate, surfaced to JS as a throw → run error).
    let mut reg = ToolRegistry::new();
    reg.register(Box::new(ReverseTool));
    let names = reg.bindable_tool_names();
    reg.register(Box::new(CodeModeTool::new(&names)));
    let block = Hooks {
        pre: vec![HookRule {
            matcher: "reverse".into(),
            command: "echo denied; exit 1".into(),
        }],
        post: vec![],
    };
    let args = serde_json::json!({ "script": "return reverse({text:'ab'});" });
    let r = dispatch_with_hooks(&reg, &block, "code_mode", &args);
    assert!(
        r.contains("tool error"),
        "blocked nested call should error: {r}"
    );
}

#[test]
fn code_mode_tool_call_is_fallback_only() {
    // Direct .call() (bypassing the loop) must not silently no-op.
    let tool = CodeModeTool::new(&["reverse".to_string()]);
    let err = tool
        .call(&serde_json::json!({ "script": "return 1;" }))
        .unwrap_err();
    assert!(err.contains("dispatch_with_hooks"), "got: {err}");
    // And the advertised description names the bound tools.
    assert!(tool.def().description.contains("reverse"));
}

#[test]
fn code_mode_receipt_parser_is_strict_and_policy_rejections_are_typed() {
    assert_eq!(
        code_mode_receipt_metrics(
            "ok\n--- code_mode receipt: nested_calls=7 nested_output_bytes=123 allow_effects=false"
        ),
        Some((7, 123))
    );
    assert_eq!(code_mode_receipt_metrics("nested_calls=7"), None);
    assert!(code_mode_receipt_is_recipe(
        "ok\n--- code_mode receipt: nested_calls=7 nested_output_bytes=123 allow_effects=false recipe=repo_recon",
        "repo_recon"
    ));
    assert!(!code_mode_receipt_is_recipe(
        "ok\n--- code_mode receipt: nested_calls=7 nested_output_bytes=123 allow_effects=false recipe=script",
        "repo_recon"
    ));
    assert!(is_code_mode_policy_rejection(
        "tool error: code_mode nested-call budget exhausted (48 calls)"
    ));
    assert!(!is_code_mode_policy_rejection(
        "tool error: code_mode: compile error"
    ));
}

#[test]
fn code_mode_recipe_contract_rejects_ambiguous_or_effectful_calls() {
    let mut reg = ToolRegistry::new();
    reg.register(Box::new(ReverseTool));
    let names = reg.bindable_tool_names();
    reg.register(Box::new(CodeModeTool::new(&names)));
    let hooks = Hooks::default();
    // Both `script` and `recipe` given: the script runs (unambiguous), with a
    // one-line notice, instead of the retried exclusivity error.
    let both = dispatch_with_hooks(
        &reg,
        &hooks,
        "code_mode",
        &serde_json::json!({"script":"return 1", "recipe":"repo_recon", "query":"x"}),
    );
    assert!(
        both.starts_with("[code_mode: both script and recipe given; ran script]\n"),
        "{both}"
    );
    assert!(both.contains("recipe=script"), "{both}");
    let neither = dispatch_with_hooks(&reg, &hooks, "code_mode", &serde_json::json!({"query":"x"}));
    assert!(neither.contains("exactly one"), "{neither}");
    let placeholder_script = dispatch_with_hooks(
        &reg,
        &hooks,
        "code_mode",
        &serde_json::json!({"script":"", "recipe":"repo_recon", "query":"needle"}),
    );
    assert!(
        code_mode_receipt_is_recipe(&placeholder_script, "repo_recon"),
        "{placeholder_script}"
    );
    let effectful = dispatch_with_hooks(
        &reg,
        &hooks,
        "code_mode",
        &serde_json::json!({"recipe":"repo_recon", "query":"needle", "allow_effects":true}),
    );
    assert!(effectful.contains("recipe is read-only"), "{effectful}");
}

#[test]
fn code_mode_repo_recon_has_a_tighter_resource_receipt() {
    let _lock = crate::tests::env_lock();
    let _generic_calls = EnvGuard::set("ANGEL_CODE_MODE_MAX_CALLS", "48");
    let _recipe_calls = EnvGuard::set("ANGEL_TASK_RECON_MAX_CALLS", "24");
    let _generic_bytes = EnvGuard::set("ANGEL_CODE_MODE_MAX_NESTED_OUTPUT_BYTES", "8388608");
    let _recipe_bytes = EnvGuard::set("ANGEL_TASK_RECON_MAX_NESTED_OUTPUT_BYTES", "4194304");
    let _recipe_timeout = EnvGuard::set("ANGEL_TASK_RECON_TIMEOUT_MS", "15000");
    let root = std::env::temp_dir().join(format!(
        "angel_repo_recon_limits_{}_{}",
        std::process::id(),
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    std::fs::create_dir_all(&root).unwrap();
    let mut reg = ToolRegistry::new();
    register_file_tools(&mut reg, root.clone());
    let names = reg.bindable_tool_names();
    reg.register(Box::new(CodeModeTool::new(&names)));
    let result = dispatch_with_hooks(
        &reg,
        &Hooks::default(),
        "code_mode",
        &serde_json::json!({"recipe":"repo_recon", "query":"AbsentWidget"}),
    );
    assert!(result.contains("recipe=repo_recon"), "{result}");
    assert!(result.contains("max_calls=24"), "{result}");
    assert!(
        result.contains("max_nested_output_bytes=4194304"),
        "{result}"
    );
    assert!(result.contains("timeout_ms=15000"), "{result}");
    std::fs::remove_dir_all(root).ok();
}
