//! `System::apply_discarding` (the host's step: it reads no state back) leaves a
//! resident worker where `apply` does: the same tick after tick, the same runs.
use angel_delve::rules::{Input, Run, phrasebook, runtime_state};
use angel_live::system::System;
use serde_json::{Value, json};
use std::{collections::BTreeMap, fs, thread, time::{Duration, Instant, SystemTime}};

fn state(run: &Run) -> Value {
    runtime_state::capture(run).unwrap().into_value()
}
fn event(inputs: &BTreeMap<u32, Input>) -> Value {
    json!({"inputs":inputs,"settings":angel_delve::settings::get(),"phrasebook":phrasebook::snapshot(),"step":true})
}
fn engine(name: &str) -> System {
    let root = std::env::temp_dir().join(format!(
        "delve-discarding-{name}-{}-{}",
        std::process::id(),
        SystemTime::now().duration_since(SystemTime::UNIX_EPOCH).unwrap().as_nanos()
    ));
    fs::create_dir(&root).unwrap();
    let artifact = root.join("engine");
    fs::copy(env!("CARGO_BIN_EXE_angel-delve-system"), &artifact).unwrap();
    System::watch("delve", artifact, Duration::from_secs(5)).unwrap()
}

#[test]
fn a_state_the_caller_does_not_read_back_is_not_cloned_and_changes_nothing() {
    let (mut keeps, mut drops) = (engine("keeps"), engine("drops"));
    let mut a = Run::new(470, 103, None);
    let mut b = a.clone();
    let input = BTreeMap::from([(1, Input { move_x: 1, fire: true, ..Default::default() })]);
    let began = Instant::now();
    for tick in 0..60 {
        // The host puts a thing of its own on the run between ticks, so the worker is sent a delta.
        if tick % 7 == 3 {
            a.cues.push(format!("host says {tick}"));
            b.cues.push(format!("host says {tick}"));
        }
        let parsed = std::cell::RefCell::new(None);
        let (next, _) = loop {
            match keeps.apply(state(&a), event(&input), |v| {
                runtime_state::restore_value(v).map(|run| {
                    parsed.replace(Some(run));
                })
            }) {
                Ok(reply) => break reply,
                Err(_) if keeps.status().swaps == 0 && began.elapsed() < Duration::from_secs(20) => thread::sleep(Duration::from_millis(10)),
                Err(e) => panic!("apply failed: {e}"),
            }
        };
        assert!(next.get("run").is_some(), "apply hands back the state it validated");
        a = parsed.take().unwrap();
        let parsed = std::cell::RefCell::new(None);
        loop {
            match drops.apply_discarding(state(&b), event(&input), |v| {
                runtime_state::restore_value(v).map(|run| {
                    parsed.replace(Some(run));
                })
            }) {
                Ok(_) => break,
                Err(_) if drops.status().swaps == 0 && began.elapsed() < Duration::from_secs(20) => thread::sleep(Duration::from_millis(10)),
                Err(e) => panic!("apply_discarding failed: {e}"),
            }
        }
        b = parsed.take().unwrap();
        assert_eq!(state(&a), state(&b), "tick {tick}: the two runs are the same");
    }
    assert_eq!(a.tick, b.tick);
    assert!(a.tick >= 60, "the worker stepped the run ({})", a.tick);
}
