//! `ANGEL_SYSTEM_RECORD`. Its own test binary: the environment is process-wide.
use angel_live::system::System;
use serde_json::{Value, json};
use std::{fs, os::unix::fs::PermissionsExt, path::Path, time::Duration};

fn host(root: &Path) -> System {
    let journal = root.join("journal");
    let code = include_str!("fixtures/resident-worker.py.in")
        .replace("__MODE__", "resident")
        .replace("__JOURNAL__", &serde_json::to_string(&journal).unwrap());
    fs::write(root.join("worker"), code).unwrap();
    fs::set_permissions(root.join("worker"), fs::Permissions::from_mode(0o700)).unwrap();
    System::watch("resident", root.join("worker"), Duration::from_secs(5)).unwrap()
}
fn ok(_: &Value) -> Result<(), String> {
    Ok(())
}
fn apply(host: &mut System, state: &Value, event: Value) -> Result<Value, String> {
    let deadline = std::time::Instant::now() + Duration::from_secs(8);
    loop {
        match host.apply(state.clone(), event.clone(), ok) {
            Err(e) if e.contains("preparing") && std::time::Instant::now() < deadline => {
                std::thread::sleep(Duration::from_millis(20))
            }
            other => return other.map(|(state, _)| state),
        }
    }
}

#[test]
fn the_recorder_writes_a_replayable_tape_and_never_fails_an_apply() {
    let root = std::env::temp_dir().join(format!("angel-record-test-{}", std::process::id()));
    let tapes = root.join("tapes");
    fs::create_dir_all(&root).unwrap();

    unsafe { std::env::remove_var("ANGEL_SYSTEM_RECORD") };
    let mut off = host(&root);
    apply(
        &mut off,
        &json!({"count":0,"tick":0,"pos":{"x":0.5},"items":[]}),
        json!({"nothing":1}),
    )
    .unwrap();
    assert!(!tapes.exists());

    unsafe { std::env::set_var("ANGEL_SYSTEM_RECORD", &tapes) };
    let mut on = host(&root);
    let first = json!({"count":0,"tick":4294967296u64,"pos":{"x":0.1f32,"y":-0.0},"items":[1]});
    let a = apply(&mut on, &first, json!({"nothing":1})).unwrap();
    let b = apply(&mut on, &a, json!({"add":1})).unwrap();
    assert!(apply(&mut on, &b, json!({"bad":1})).is_err());
    let mut moved = b.clone();
    moved["count"] = json!(40);
    let c = apply(&mut on, &moved, json!({"push":3})).unwrap();

    let files: Vec<_> = fs::read_dir(&tapes)
        .unwrap()
        .map(|e| e.unwrap().path())
        .collect();
    assert_eq!(files.len(), 1);
    let name = files[0].file_name().unwrap().to_str().unwrap();
    assert!(
        name.starts_with(&format!("resident-{}-", std::process::id())) && name.ends_with(".jsonl"),
        "{name}"
    );
    let lines: Vec<Value> = fs::read_to_string(&files[0])
        .unwrap()
        .lines()
        .map(|l| serde_json::from_str(l).unwrap())
        .collect();
    assert_eq!(
        lines,
        [
            json!({"kind":"restore","state":first}),
            json!({"kind":"apply","before":null,"event":{"nothing":1},"after":a}),
            json!({"kind":"apply","before":null,"event":{"add":1},"after":b}),
            json!({"kind":"apply","before":moved,"event":{"push":3},"after":c}),
        ]
    );

    // A tape that cannot be written costs the host nothing.
    fs::write(root.join("file"), "x").unwrap();
    unsafe { std::env::set_var("ANGEL_SYSTEM_RECORD", root.join("file/under")) };
    let mut broken = host(&root);
    let a = apply(&mut broken, &first, json!({"nothing":1})).unwrap();
    apply(&mut broken, &a, json!({"add":1})).unwrap();
    drop((off, on, broken));
    let _ = fs::remove_dir_all(&root);
}
