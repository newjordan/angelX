#![cfg(unix)]
use angel_live::system::System;
use serde_json::{Value, json};
use std::os::unix::fs::PermissionsExt;
use std::{
    fs,
    path::PathBuf,
    thread,
    time::{Duration, Instant, SystemTime},
};

struct Fixture {
    root: PathBuf,
    artifact: PathBuf,
    journal: PathBuf,
}
impl Fixture {
    fn new() -> Self {
        static SERIAL: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
        let root = std::env::temp_dir().join(format!(
            "angel-system-test-{}-{}-{}",
            std::process::id(),
            SERIAL.fetch_add(1, std::sync::atomic::Ordering::Relaxed),
            SystemTime::now()
                .duration_since(SystemTime::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        fs::create_dir(&root).unwrap();
        Self {
            artifact: root.join("worker"),
            journal: root.join("journal"),
            root,
        }
    }
    fn publish(&self, version: u32, mode: &str) {
        let code = include_str!("fixtures/system-worker.py.in")
            .replace("__VERSION__", &version.to_string())
            .replace("__MODE__", mode)
            .replace(
                "__JOURNAL__",
                &serde_json::to_string(&self.journal).unwrap(),
            );
        let temp = self.root.join("candidate");
        fs::write(&temp, code).unwrap();
        fs::set_permissions(&temp, fs::Permissions::from_mode(0o700)).unwrap();
        fs::rename(temp, &self.artifact).unwrap();
    }
    fn host(&self) -> System {
        System::watch("counter", self.artifact.clone(), Duration::from_millis(700)).unwrap()
    }
    fn lines(&self) -> String {
        fs::read_to_string(&self.journal).unwrap_or_default()
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.root);
    }
}
fn valid(state: &Value) -> Result<(), String> {
    if matches!(state["schema"].as_u64(), Some(1 | 2))
        && state["count"].as_i64().is_some_and(|n| n >= 0)
    {
        Ok(())
    } else {
        Err("bad checkpoint".into())
    }
}
fn await_version(host: &mut System, state: &mut Value, version: u32) -> Value {
    let deadline = Instant::now() + Duration::from_secs(8);
    loop {
        if let Ok((next, output)) = host.apply(state.clone(), json!({"delta":0}), valid) {
            *state = next;
            if output["version"] == version {
                return output;
            }
        }
        assert!(
            Instant::now() < deadline,
            "candidate never activated: {:?}",
            host.status()
        );
        thread::sleep(Duration::from_millis(20));
    }
}
fn dead(pid: u32) -> bool {
    unsafe { libc::kill(pid as i32, 0) != 0 }
}

#[test]
fn code_swap_migrates_state_and_retires_the_previous_process() {
    let fixture = Fixture::new();
    fixture.publish(1, "ok");
    let mut host = fixture.host();
    let mut state = json!({"schema":1,"count":0});
    let first = await_version(&mut host, &mut state, 1);
    state = host.apply(state, json!({"delta":3}), valid).unwrap().0;
    fixture.publish(2, "ok");
    let second = await_version(&mut host, &mut state, 2);
    assert_eq!(state, json!({"schema":2,"count":3}));
    state = host.apply(state, json!({"delta":3}), valid).unwrap().0;
    assert_eq!(state["count"], 33);
    assert_ne!(first["pid"], second["pid"]);
    assert!(dead(first["pid"].as_u64().unwrap() as u32));
    let pid = second["pid"].as_u64().unwrap() as u32;
    drop(host);
    assert!(dead(pid));
    assert!(fixture.lines().contains("1 dispose"));
}

#[test]
fn bad_api_migration_and_state_keep_the_original_serving() {
    let fixture = Fixture::new();
    fixture.publish(1, "ok");
    let mut host = fixture.host();
    let mut state = json!({"schema":1,"count":7});
    let first = await_version(&mut host, &mut state, 1);
    for (version, mode) in [(2, "bad-api"), (3, "bad-migration"), (4, "bad-activation")] {
        let before = host.status().rejected;
        fixture.publish(version, mode);
        let deadline = Instant::now() + Duration::from_secs(5);
        while host.status().rejected == before {
            let (next, out) = host
                .apply(state.clone(), json!({"delta":0}), valid)
                .unwrap();
            state = next;
            assert_eq!(out["pid"], first["pid"]);
            assert_eq!(state["count"], 7);
            assert!(Instant::now() < deadline);
            thread::sleep(Duration::from_millis(20));
        }
        assert_eq!(host.status().swaps, 1);
    }
    fixture.publish(5, "ok");
    // Host validation also rejects the candidate's copied checkpoint.
    let deadline = Instant::now() + Duration::from_secs(5);
    while host.status().rejected < 4 {
        let _ = host.apply(state.clone(), json!({"delta":0}), |s| {
            if s["schema"] == 1 {
                Ok(())
            } else {
                Err("host cannot read schema 2".into())
            }
        });
        assert!(Instant::now() < deadline);
        thread::sleep(Duration::from_millis(20));
    }
    assert_eq!(host.status().swaps, 1);
    assert_eq!(
        host.apply(state, json!({"delta":1}), valid).unwrap().0["count"],
        8
    );
}

#[test]
fn in_flight_request_finishes_once_before_handoff() {
    let fixture = Fixture::new();
    fixture.publish(1, "ok");
    let mut host = fixture.host();
    let mut state = json!({"schema":1,"count":0});
    await_version(&mut host, &mut state, 1);
    let running = thread::spawn(move || {
        let (next, out) = host
            .apply(state, json!({"delta":1,"delay":0.5}), valid)
            .unwrap();
        (host, next, out)
    });
    thread::sleep(Duration::from_millis(80));
    fixture.publish(2, "ok");
    let (mut host, mut state, out) = running.join().unwrap();
    assert_eq!(out["version"], 1);
    assert_eq!(state["count"], 1);
    await_version(&mut host, &mut state, 2);
    assert_eq!(
        host.apply(state, json!({"delta":1}), valid).unwrap().0["count"],
        11
    );
}

#[test]
fn failed_request_is_not_replayed_and_new_code_can_recover_the_committed_state() {
    for mode in ["crash", "hang"] {
        let fixture = Fixture::new();
        fixture.publish(1, mode);
        let mut host = fixture.host();
        let state = json!({"schema":1,"count":12});
        let deadline = Instant::now() + Duration::from_secs(5);
        while host.status().swaps == 0 {
            let _ = host.apply(state.clone(), json!({"delta":1}), valid);
            assert!(Instant::now() < deadline);
            thread::sleep(Duration::from_millis(20));
        }
        let start = Instant::now();
        assert!(
            host.apply(state.clone(), json!({"delta":1}), valid)
                .is_err()
        );
        assert!(start.elapsed() < Duration::from_millis(100));
        assert_eq!(
            fixture
                .lines()
                .lines()
                .filter(|l| l.ends_with("apply"))
                .count(),
            1
        );
        fixture.publish(2, "ok");
        let mut recovered = state.clone();
        await_version(&mut host, &mut recovered, 2);
        assert_eq!(recovered["count"], 12);
    }
}

#[test]
fn repeated_reloads_reap_processes_and_artifacts() {
    let fixture = Fixture::new();
    fixture.publish(1, "ok");
    let mut host = fixture.host();
    let mut state = json!({"schema":1,"count":1});
    let mut previous = None;
    for version in 1..=8 {
        fixture.publish(version, "ok");
        let output = await_version(&mut host, &mut state, version);
        if let Some(pid) = previous {
            assert!(dead(pid));
        }
        previous = Some(output["pid"].as_u64().unwrap() as u32);
    }
    assert_eq!(host.status().swaps, 8);
    drop(host);
    assert!(dead(previous.unwrap()));
}

#[test]
fn required_contracts_reject_legacy_candidates_without_retiring_the_working_process() {
    let fixture = Fixture::new();
    fixture.publish(1, "ok");
    let mut host = System::watch_with_contracts(
        "counter",
        fixture.artifact.clone(),
        Duration::from_millis(700),
        std::collections::BTreeMap::from([("counter.quote".into(), 1)]),
    )
    .unwrap();
    let mut state = json!({"schema":1,"count":7});
    let first = await_version(&mut host, &mut state, 1);
    for (version, mode) in [(2, "missing-contract"), (3, "bad-contract")] {
        let rejected = host.status().rejected;
        fixture.publish(version, mode);
        let deadline = Instant::now() + Duration::from_secs(5);
        while host.status().rejected == rejected {
            let (next, output) = host
                .apply(state.clone(), json!({"delta":0}), valid)
                .unwrap();
            state = next;
            assert_eq!(output["pid"], first["pid"]);
            assert!(Instant::now() < deadline);
            thread::sleep(Duration::from_millis(20));
        }
        assert_eq!(host.status().swaps, 1);
        let output = host
            .compute("counter.quote", 1, json!({"base":4}), |_| Ok(()))
            .unwrap();
        assert_eq!(output["value"], 4);
        assert_eq!(state["count"], 7);
    }
}

#[test]
fn pure_computations_finish_before_handoff_and_validate_outputs_without_a_checkpoint() {
    let fixture = Fixture::new();
    fixture.publish(1, "ok");
    let mut host = fixture.host();
    let mut state = json!({"schema":1,"count":9});
    let first = await_version(&mut host, &mut state, 1);
    let running = thread::spawn(move || {
        let output = host
            .compute(
                "counter.quote",
                1,
                json!({"base":4,"delay":0.5}),
                |_| Ok(()),
            )
            .unwrap();
        (host, output)
    });
    thread::sleep(Duration::from_millis(80));
    fixture.publish(2, "ok");
    let (mut host, output) = running.join().unwrap();
    assert_eq!(output["value"], 4);
    assert_eq!(output["pid"], first["pid"]);
    // Even a fully prepared image waits for a validated checkpoint boundary.
    assert_eq!(
        host.compute("counter.quote", 1, json!({"base":4}), |_| Ok(()))
            .unwrap()["pid"],
        first["pid"]
    );
    await_version(&mut host, &mut state, 2);
    assert_eq!(state["count"], 9);
    assert_eq!(
        host.compute("counter.quote", 1, json!({"base":4}), |_| Ok(()))
            .unwrap()["value"],
        8
    );
    assert!(dead(first["pid"].as_u64().unwrap() as u32));
    assert!(
        host.compute("counter.quote", 2, Value::Null, |_| Ok(()))
            .is_err()
    );
    fixture.publish(3, "bad-output");
    await_version(&mut host, &mut state, 3);
    assert!(
        host.compute("counter.quote", 1, json!({"base":4}), |value| {
            if value["value"].as_i64().is_some_and(|v| v >= 0) {
                Ok(())
            } else {
                Err("invalid quote".into())
            }
        })
        .unwrap_err()
        .contains("invalid quote")
    );
    assert_eq!(state["count"], 9);
    assert_eq!(host.status().error.as_deref(), Some("invalid quote"));
}
