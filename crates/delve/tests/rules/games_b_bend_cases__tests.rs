//! Case recorder helpers for the Bend port of games_b (delve/GROUPS.md). Test
//! only; compiled under `bend-cases-games-b`. Each recorded line is
//! `{"fn": <bend name>, "in": <json>, "out": <json>}` in
//! `$DELVE_CASES/<bend name>.jsonl`.
use crate::rules::{Input, Run, runtime_state};
use serde_json::{Map, Value, json};
use std::collections::{BTreeMap, BTreeSet};
use std::io::Write;
use std::sync::Mutex;

static OPEN: Mutex<Option<BTreeSet<String>>> = Mutex::new(None);

pub fn dir() -> std::path::PathBuf {
    std::env::var_os("DELVE_CASES")
        .map(Into::into)
        .expect("set DELVE_CASES to the Bend case directory")
}

/// The Run as the rules worker holds it.
pub fn state(run: &Run) -> Value {
    runtime_state::capture_for_rules(run).unwrap().run
}

pub fn inputs(inputs: &BTreeMap<u32, Input>) -> Value {
    serde_json::to_value(inputs).unwrap()
}

/// Append one case; the first write of a name in this process truncates it.
pub fn record(name: &str, input: Value, output: Value) {
    let dir = dir();
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join(format!("{name}.jsonl"));
    let mut open = OPEN.lock().unwrap();
    let fresh = open.get_or_insert_with(Default::default).insert(name.to_string());
    let mut file = std::fs::OpenOptions::new()
        .create(true)
        .write(true)
        .append(!fresh)
        .truncate(fresh)
        .open(path)
        .unwrap();
    let line = json!({"fn": name, "in": input, "out": output});
    writeln!(file, "{}", serde_json::to_string(&line).unwrap()).unwrap();
}

/// A method of Run: the Run (and the named extra arguments) go in, the Run
/// comes out beside the method's return value, when it has one (`ret`).
pub fn call<R: serde::Serialize>(name: &str, run: &mut Run, extra: Value, f: impl FnOnce(&mut Run) -> R) {
    let before = state(run);
    let ret = serde_json::to_value(f(run)).unwrap();
    let mut input = Map::new();
    input.insert("run".into(), before);
    if let Value::Object(extra) = extra {
        input.extend(extra);
    }
    let mut output = Map::new();
    output.insert("run".into(), state(run));
    if !ret.is_null() {
        output.insert("ret".into(), ret);
    }
    record(name, Value::Object(input), Value::Object(output));
}

/// A method of Run that takes the tick's inputs.
pub fn call_inputs<R: serde::Serialize>(name: &str, run: &mut Run, keys: &BTreeMap<u32, Input>, f: impl FnOnce(&mut Run, &BTreeMap<u32, Input>) -> R) {
    call(name, run, json!({"inputs": inputs(keys)}), |r| f(r, keys));
}

pub fn obj(pairs: Vec<(&str, Value)>) -> Value {
    Value::Object(pairs.into_iter().map(|(k, v)| (k.to_string(), v)).collect::<Map<_, _>>())
}
