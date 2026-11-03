//! The rules worker evaluates the shipped Bend modules: the same engine the
//! cockpit installs, over the worker's own V8 backend.
use serde_json::{Value, json};

fn probe() -> Value {
    json!({
        "seat": "probe", "native": "low", "allowed": ["low", "high"], "hop": 0,
        "friction": false, "tool_errors": 0, "failed_checks": 0,
        "task": "fix the failing test", "task_len": 20
    })
}

#[test]
fn the_installed_engine_answers_a_shipped_module_and_refuses_an_unknown_one() {
    angel_delve_system::install_rules_engine();
    assert!(angel_delve::scripted::installed());
    let answer: Value = angel_delve::scripted::step("effort", &probe()).unwrap();
    assert!(answer.get("effort").is_some(), "{answer}");
    let missing = angel_delve::scripted::step::<_, Value>("no-such-module", &json!({}));
    assert!(missing.unwrap_err().contains("no shipped module named"));
}

#[cfg(feature = "swap-test")]
#[test]
fn a_scripted_call_runs_inside_the_worker_process() {
    use angel_delve::rules::{Run, runtime_state};
    use angel_live::system::System;
    use std::time::Duration;

    let root = std::env::temp_dir().join(format!("delve-scripted-{}", std::process::id()));
    std::fs::create_dir_all(&root).unwrap();
    let artifact = root.join("engine");
    std::fs::copy(env!("CARGO_BIN_EXE_angel-delve-system-scripted"), &artifact).unwrap();
    let mut system = System::watch("delve", artifact, Duration::from_secs(5)).unwrap();
    let run = Run::new(470, 103, None);
    let state = runtime_state::capture(&run).unwrap().into_value();
    let event = json!({
        "inputs": {},
        "settings": angel_delve::settings::get(),
        "phrasebook": angel_delve::rules::phrasebook::snapshot()
    });
    // The first implementation prepares in the background; ask until it serves.
    let started = std::time::Instant::now();
    let (next, _) = loop {
        match system.apply(state.clone(), event.clone(), |_| Ok(())) {
            Ok(reply) => break reply,
            Err(_) if started.elapsed() < Duration::from_secs(10) => {
                std::thread::sleep(Duration::from_millis(10))
            }
            Err(error) => panic!("rules request failed: {error}; {:?}", system.status()),
        }
    };
    let next = runtime_state::restore_value(&next).unwrap();
    // The worker's own process answered; this test process installed nothing.
    let scripted: Vec<_> = next
        .cues
        .iter()
        .filter(|cue| cue.starts_with("scripted:"))
        .collect();
    assert_eq!(scripted.len(), 1, "{:?}", next.cues);
    assert!(scripted[0].starts_with("scripted:Ok("), "{}", scripted[0]);
    assert!(scripted[0].contains("effort"), "{}", scripted[0]);
    drop(system);
    std::fs::remove_dir_all(root).unwrap();
}
