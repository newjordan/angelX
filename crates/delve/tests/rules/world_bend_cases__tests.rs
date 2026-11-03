//! Shared by the world group's Bend case recorders (feature `bend-cases-world`):
//! each case is a line `{"fn", "in", "out"}` appended to
//! `$DELVE_CASES/<fn>.jsonl` (`DELVE_CASES` is required), replayed
//! by `python3 delve/rules/world/wb.py <fn>...`.
#![allow(dead_code)]

use serde::Serialize;
use serde_json::{Value, json};
use std::path::PathBuf;

pub fn dir() -> PathBuf {
    std::env::var_os("DELVE_CASES")
        .map(PathBuf::from)
        .expect("set DELVE_CASES to the Bend case directory")
}

/// The cases of one Bend def, written whole (one writer per name) on `save`.
pub struct Cases {
    name: String,
    lines: Vec<String>,
}

impl Cases {
    pub fn new(name: &str) -> Self {
        Self { name: name.to_string(), lines: Vec::new() }
    }

    pub fn add(&mut self, input: Value, output: Value) {
        let line = json!({"fn": self.name, "in": input, "out": output});
        self.lines.push(serde_json::to_string(&line).unwrap());
    }

    pub fn len(&self) -> usize {
        self.lines.len()
    }

    pub fn save(&self) {
        let dir = dir();
        std::fs::create_dir_all(&dir).unwrap();
        let mut text = self.lines.join("\n");
        text.push('\n');
        std::fs::write(dir.join(format!("{}.jsonl", self.name)), text).unwrap();
        eprintln!("{}: {} cases", self.name, self.lines.len());
    }
}

/// A value as the host's state tree holds it.
pub fn to<T: Serialize>(t: &T) -> Value {
    serde_json::to_value(t).unwrap()
}

/// A splitmix64 stream: seeds for the cases, the same on every run.
pub struct Mix(pub u64);

impl Mix {
    pub fn next(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9e37_79b9_7f4a_7c15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
        z ^ (z >> 31)
    }

    pub fn below(&mut self, n: u64) -> u64 {
        self.next() % n.max(1)
    }
}
