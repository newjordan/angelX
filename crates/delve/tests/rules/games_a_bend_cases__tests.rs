//! The games_a group's case recorder kit (Bend port; delve/PORTING.md).
//!
//! `rec` appends `{"fn", "in", "out"}` to `$DELVE_CASES/<fn>.jsonl`. The first
//! write of a name in a process truncates its file, so a rerun replaces the old
//! cases. `rec_cap` also stops after `cap` cases of that name.
use serde::Serialize;
use std::cell::RefCell;
use std::collections::BTreeMap;
use std::io::Write;

thread_local! {
    static SEEN: RefCell<BTreeMap<String, usize>> = const { RefCell::new(BTreeMap::new()) };
}

pub fn dir() -> std::path::PathBuf {
    std::env::var("DELVE_CASES")
        .map(Into::into)
        .expect("set DELVE_CASES to the Bend case directory")
}

/// How many cases of `name` this process has written.
pub fn count(name: &str) -> usize {
    SEEN.with(|s| s.borrow().get(name).copied().unwrap_or(0))
}

/// Append a case; false when `cap` cases of the name are already written.
pub fn rec_cap<I: Serialize, O: Serialize>(name: &str, input: &I, out: &O, cap: usize) -> bool {
    let n = count(name);
    if n >= cap {
        return false;
    }
    let path = dir().join(format!("{name}.jsonl"));
    std::fs::create_dir_all(dir()).unwrap();
    let mut f = std::fs::OpenOptions::new()
        .create(true)
        .write(true)
        .append(n > 0)
        .truncate(n == 0)
        .open(path)
        .unwrap();
    let line = serde_json::json!({"fn": name, "in": input, "out": out});
    writeln!(f, "{}", serde_json::to_string(&line).unwrap()).unwrap();
    SEEN.with(|s| {
        s.borrow_mut().insert(name.to_string(), n + 1);
    });
    true
}

pub fn rec<I: Serialize, O: Serialize>(name: &str, input: &I, out: &O) {
    rec_cap(name, input, out, usize::MAX);
}
