//! The Kernel's V8 backend: warm pooled isolates, cached module setups, and
//! the shipped policy generations compiled in from cockpit/live.
#![cfg(feature = "v8")]
use angel_live::{Registry, kernel::Kind, shipped, v8 as backend};
use serde_json::{Value, json};
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

fn cockpit_live() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../cockpit/live")
}

fn effort_probe() -> Value {
    json!({"seat":"probe","native":"low","allowed":["low","high"],"hop":0,"friction":false,
        "tool_errors":0,"failed_checks":0,"task":"fix the failing test","task_len":20})
}

#[test]
fn the_shipped_engine_answers_a_policy_module_with_the_directory_generation() {
    let mut registry = Registry::validated(cockpit_live(), backend::evaluate, |_, _| Ok(()));
    registry.refresh(true);
    let snapshot = registry.snapshot();
    let ids: Vec<_> = shipped::ids().collect();
    assert!(ids.contains(&"effort"), "{ids:?}");
    for id in &ids {
        let loaded = snapshot
            .policies
            .get(*id)
            .unwrap_or_else(|| panic!("{id} is not active"));
        assert_eq!(
            shipped::program(id).unwrap().digest(),
            loaded.digest(),
            "{id}"
        );
    }
    let answer = shipped::engine("effort", &effort_probe()).unwrap();
    assert!(answer.get("effort").is_some(), "{answer}");
    assert_eq!(
        answer,
        snapshot.policies["effort"].render(&effort_probe()).unwrap()
    );
    assert!(
        shipped::engine("no-such-module", &json!({}))
            .unwrap_err()
            .contains("no shipped module")
    );
    assert!(shipped::program("no-such-module").is_none());
}

#[test]
fn the_pool_runs_a_script_with_a_fresh_context_and_no_tools() {
    assert_eq!(
        backend::evaluate("return {value: (await Promise.resolve(41)) + 1}").unwrap(),
        r#"{"value":42}"#
    );
    // Nothing a call defines survives into the next one.
    backend::evaluate("globalThis.leak = 7; return 1").unwrap();
    assert_eq!(
        backend::evaluate("return typeof globalThis.leak").unwrap(),
        "undefined"
    );
    let tool = backend::evaluate("return tool('read_file', {})").unwrap_err();
    assert!(
        tool.contains("live projections do not execute tools"),
        "{tool}"
    );
    let batch = backend::evaluate("return batch([{tool:'a',args:{}},{tool:'b'}])").unwrap();
    let batch: Value = serde_json::from_str(&batch).unwrap();
    assert_eq!(batch.as_array().unwrap().len(), 2);
    assert!(batch.as_array().unwrap().iter().all(|r| r["ok"] == false));
    let bad = backend::evaluate("return batch([{args:{}}])").unwrap_err();
    assert!(bad.contains("string `tool` field"), "{bad}");
}

#[test]
fn a_runaway_script_is_stopped_at_the_deadline_and_the_pool_keeps_serving() {
    let started = Instant::now();
    let error = backend::evaluate("for(;;){}").unwrap_err();
    assert!(error.contains("timed out"), "{error}");
    assert!(
        started.elapsed() < Duration::from_secs(3),
        "{:?}",
        started.elapsed()
    );
    for _ in 0..8 {
        assert_eq!(backend::evaluate("return 'ok'").unwrap(), "ok");
    }
}

#[test]
fn a_cached_setup_runs_once_per_key_and_a_failed_call_forgets_it() {
    let deadline = Duration::from_millis(200);
    let mut warm = backend::Warm::new(32);
    let setup = "const feature={n:0}; ";
    let bump = "return ++feature.n";
    assert_eq!(warm.run_cached("a", setup, bump, deadline).unwrap(), "1");
    assert_eq!(warm.run_cached("a", setup, bump, deadline).unwrap(), "2");
    // Another key is another context.
    assert_eq!(warm.run_cached("b", setup, bump, deadline).unwrap(), "1");
    assert_eq!(warm.run_cached("a", setup, bump, deadline).unwrap(), "3");
    // A failing call drops its context; the next call builds the setup again.
    assert!(
        warm.run_cached("a", setup, "throw Error('boom')", deadline)
            .is_err()
    );
    assert_eq!(warm.run_cached("a", setup, bump, deadline).unwrap(), "1");
    // A call's own locals are scoped to that call.
    warm.run_cached("a", setup, "const mine = 5; return mine", deadline)
        .unwrap();
    assert_eq!(
        warm.run_cached("a", setup, "return typeof mine", deadline)
            .unwrap(),
        "undefined"
    );
    // A runaway call terminates its isolate; the next call rebuilds and runs.
    let error = warm
        .run_cached("a", setup, "for(;;){}", deadline)
        .unwrap_err();
    assert!(error.contains("timed out"), "{error}");
    assert_eq!(warm.run_cached("a", setup, bump, deadline).unwrap(), "1");
}

#[test]
fn split_call_separates_setup_key_and_call() {
    let script = format!("setup;\n{}key:1*/\ncall;", angel_live::CALL_MARKER);
    assert_eq!(
        backend::split_call(&script),
        Some(("key:1", "setup;\n", "call;"))
    );
    assert_eq!(backend::split_call("no marker"), None);
}

struct Staged(PathBuf);
impl Staged {
    fn new(source: &str) -> Self {
        static SERIAL: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
        let root = std::env::temp_dir().join(format!(
            "angel-live-v8-{}-{}",
            std::process::id(),
            SERIAL.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
        ));
        std::fs::create_dir_all(root.join("report")).unwrap();
        std::fs::write(
            root.join("report/module.json"),
            r#"{"api_version":2,"id":"report","kind":"system","entry":"index.js","state_schema":1,"exports":[{"kind":"command","name":"count","contract":"command/1"},{"kind":"surface","name":"scoreboard","contract":"surface/1"}]}"#,
        )
        .unwrap();
        let staged = Self(root);
        staged.source(source);
        staged
    }
    fn source(&self, text: &str) {
        let path = self.0.join("report/index.js");
        std::fs::write(path.with_extension("tmp"), text).unwrap();
        std::fs::rename(path.with_extension("tmp"), path).unwrap();
    }
}
impl Drop for Staged {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

#[test]
fn a_system_module_calls_reuse_the_generation_setup_and_a_new_generation_replaces_it() {
    let staged = Staged::new(
        "module.exports={apiVersion:2, initialize:()=>({count:0}), \
         reduce:(s,e)=>({state:{count:s.count+1},output:'count='+(s.count+1)}), \
         view:s=>({scoreboard:{title:'Score',lines:['value '+s.count]}})}",
    );
    let mut registry = Registry::new(staged.0.clone(), backend::evaluate);
    registry.refresh(true);
    for n in 1..=50 {
        let output = registry
            .dispatch(Kind::Command, "count", &json!({}))
            .unwrap();
        assert_eq!(output["output"], format!("count={n}"));
    }
    assert_eq!(
        registry.snapshot().surfaces["scoreboard"].lines,
        vec!["value 50"]
    );
    // A new generation of the same module is a new key: its code answers at once,
    // and the state carries over through the module's own `migrate` default.
    staged.source(
        "module.exports={apiVersion:2, initialize:()=>({count:0}), \
         reduce:(s,e)=>({state:{count:s.count+10},output:'new='+(s.count+10)}), \
         view:s=>({scoreboard:{title:'New',lines:['value '+s.count]}})}",
    );
    registry.refresh(true);
    assert_eq!(
        registry
            .dispatch(Kind::Command, "count", &json!({}))
            .unwrap()["output"],
        "new=60"
    );
    // A module whose reducer throws reports it and keeps serving afterwards.
    staged.source(
        "module.exports={apiVersion:2, migrate:()=>{throw Error('refused')}, reduce:()=>{}}",
    );
    registry.refresh(true);
    assert!(
        registry.snapshot().statuses[0]
            .error
            .as_ref()
            .unwrap()
            .contains("refused")
    );
    assert_eq!(
        registry
            .dispatch(Kind::Command, "count", &json!({}))
            .unwrap()["output"],
        "new=70"
    );
}

/// The pool without its cache: the call marker becomes a plain comment, so
/// every call evaluates the whole script in a fresh context.
fn whole_script(script: &str) -> Result<String, String> {
    backend::evaluate(&script.replace(angel_live::CALL_MARKER, "//"))
}

#[test]
#[ignore = "local runtime measurement; run explicitly with --ignored --nocapture"]
fn system_call_cost_with_and_without_the_cached_setup() {
    let staged = Staged::new(
        "module.exports={apiVersion:2, initialize:()=>({count:0}), \
         reduce:(s,e)=>({state:{count:s.count+1},output:'c'}), view:s=>({scoreboard:{title:'t',lines:[]}})}",
    );
    for (label, evaluator) in [
        ("cached setup", backend::evaluate as angel_live::Evaluator),
        ("whole script", whole_script as angel_live::Evaluator),
    ] {
        let mut registry = Registry::new(staged.0.clone(), evaluator);
        registry.refresh(true);
        let mut samples = Vec::new();
        for _ in 0..2000 {
            let started = Instant::now();
            registry
                .dispatch(Kind::Command, "count", &json!({}))
                .unwrap();
            samples.push(started.elapsed().as_micros());
        }
        samples.sort_unstable();
        eprintln!(
            "system dispatch, {label}: p50={}us p95={}us max={}us",
            samples[1000], samples[1900], samples[1999]
        );
    }
}
