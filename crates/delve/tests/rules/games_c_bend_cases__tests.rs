//! The games_c group's case recorder kit (Bend port; delve/PORTING.md). Test only; compiled under
//! `bend-cases-games-c`. A case is `{"fn", "in", "out"}` in `$DELVE_CASES/<fn>.jsonl`; the first
//! write of a name in a process truncates its file, so a rerun replaces the old cases.
use crate::rules::{Run, runtime_state};
use serde::Serialize;
use serde_json::{Map, Value, json};
use std::collections::{BTreeMap, BTreeSet};
use std::io::Write;
use std::sync::Mutex;

static OPEN: Mutex<Option<BTreeSet<String>>> = Mutex::new(None);
static COUNT: Mutex<Option<BTreeMap<String, usize>>> = Mutex::new(None);

pub fn dir() -> std::path::PathBuf {
    std::env::var_os("DELVE_CASES").map(Into::into).expect("set DELVE_CASES to the Bend case directory")
}

/// How many cases of `name` this process has written.
pub fn count(name: &str) -> usize {
    COUNT.lock().unwrap().get_or_insert_with(Default::default).get(name).copied().unwrap_or(0)
}

/// Append one case.
pub fn rec(name: &str, input: Value, output: Value) {
    let dir = dir();
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join(format!("{name}.jsonl"));
    let mut open = OPEN.lock().unwrap();
    let fresh = open.get_or_insert_with(Default::default).insert(name.to_string());
    let mut file = std::fs::OpenOptions::new().create(true).write(true).append(!fresh).truncate(fresh).open(path).unwrap();
    let line = json!({"fn": name, "in": input, "out": output});
    writeln!(file, "{}", serde_json::to_string(&line).unwrap()).unwrap();
    *COUNT.lock().unwrap().get_or_insert_with(Default::default).entry(name.to_string()).or_insert(0) += 1;
}

/// A value as JSON.
pub fn js<T: Serialize>(v: &T) -> Value {
    serde_json::to_value(v).unwrap()
}

/// The Run as the rules worker holds it.
pub fn state(run: &Run) -> Value {
    runtime_state::capture_for_rules(run).unwrap().run
}

/// The named top-level keys of an object.
pub fn pick(v: &Value, keys: &BTreeSet<String>) -> Value {
    let mut out = Map::new();
    if let Value::Object(m) = v {
        for k in keys {
            if let Some(x) = m.get(k) {
                out.insert(k.clone(), x.clone());
            }
        }
    }
    Value::Object(out)
}

/// The top-level keys whose values differ (or appear in one only).
pub fn changed(a: &Value, b: &Value) -> BTreeSet<String> {
    let (Value::Object(a), Value::Object(b)) = (a, b) else { return BTreeSet::new() };
    a.keys().chain(b.keys()).filter(|k| a.get(*k) != b.get(*k)).cloned().collect()
}

/// A method of Run: the Run (only the keys it reads and the keys it changes, so a case stays small)
/// and the named extra arguments go in; the Run comes out beside the return value when there is one.
pub fn call<R: Serialize>(name: &str, run: &mut Run, reads: &[&str], extra: Value, f: impl FnOnce(&mut Run) -> R) {
    let before = state(run);
    let ret = serde_json::to_value(f(run)).unwrap();
    let after = state(run);
    let mut keys: BTreeSet<String> = reads.iter().map(|k| k.to_string()).collect();
    keys.extend(changed(&before, &after));
    let mut input = Map::new();
    input.insert("run".into(), pick(&before, &keys));
    if let Value::Object(extra) = extra {
        input.extend(extra);
    }
    let mut output = Map::new();
    output.insert("run".into(), pick(&after, &keys));
    if !ret.is_null() {
        output.insert("ret".into(), ret);
    }
    rec(name, Value::Object(input), Value::Object(output));
}

/// A tiny deterministic generator for the recorders' choices.
pub struct Dice(pub u64);
impl Dice {
    pub fn next(&mut self) -> u64 {
        let mut x = self.0.max(1);
        x ^= x >> 12;
        x ^= x << 25;
        x ^= x >> 27;
        self.0 = x;
        x.wrapping_mul(0x2545_f491_4f6c_dd1d)
    }
    pub fn below(&mut self, n: u64) -> u64 {
        self.next() % n.max(1)
    }
    pub fn chance(&mut self, percent: u64) -> bool {
        self.below(100) < percent
    }
    /// A key axis: -1, 0 or 1.
    pub fn axis(&mut self) -> i8 {
        self.below(3) as i8 - 1
    }
}
