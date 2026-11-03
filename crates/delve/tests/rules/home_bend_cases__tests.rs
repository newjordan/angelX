//! Shared by the home group's Bend case recorders (feature `bend-cases-home`):
//! each case is a line `{"fn", "in", "out"}` written to
//! `$DELVE_CASES/<fn>.jsonl` (`DELVE_CASES` is required), replayed
//! by `python3 delve/tests/cases.py home`.
#![allow(dead_code)]

use serde::Serialize;
use serde_json::{Map, Value, json};
use std::path::PathBuf;

pub fn dir() -> PathBuf {
    std::env::var_os("DELVE_CASES")
        .map(PathBuf::from)
        .expect("set DELVE_CASES to the Bend case directory")
}

/// The cases of one Bend def, written whole on `save`.
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
    }
}

/// A value as the host's state tree holds it.
pub fn to<T: Serialize>(t: &T) -> Value {
    serde_json::to_value(t).unwrap()
}

/// The Run as `capture_for_rules` writes it.
pub fn state(run: &crate::rules::Run) -> Value {
    crate::rules::runtime_state::capture_for_rules(run).unwrap().run
}

/// An object cut to the keys a case needs.
pub fn keep(v: &Value, keys: &[&str]) -> Value {
    let mut out = Map::new();
    if let Value::Object(m) = v {
        for k in keys {
            if let Some(x) = m.get(*k) {
                out.insert((*k).to_string(), x.clone());
            }
        }
    }
    Value::Object(out)
}

/// xorshift64*: seeded variety for the recorders.
pub struct Mix(pub u64);
impl Mix {
    pub fn next(&mut self) -> u64 {
        self.0 ^= self.0 >> 12;
        self.0 ^= self.0 << 25;
        self.0 ^= self.0 >> 27;
        self.0.wrapping_mul(0x2545_f491_4f6c_dd1d)
    }
    pub fn below(&mut self, n: u64) -> u64 {
        self.next() % n.max(1)
    }
    pub fn pick<'a, T>(&mut self, xs: &'a [T]) -> &'a T {
        &xs[self.below(xs.len() as u64) as usize]
    }
    pub fn chance(&mut self, percent: u64) -> bool {
        self.below(100) < percent
    }
}

/// What the rules worker answers a compute request, by contract: the
/// `angel_delve_system` flow of deserialize, validate, evaluate, check the
/// result, serialize. `{"Ok": value}` or `{"Err": text}`.
pub fn worker(contract: &str, input: &Value) -> Value {
    use crate::rules::{brew_jobs, card_paint, home_settlement, reforge_jobs, scrolls, wish_jobs};
    fn go<T: serde::de::DeserializeOwned, R: Serialize>(
        input: &Value,
        validate: impl Fn(&T) -> Result<(), String>,
        evaluate: impl Fn(&T) -> Result<R, String>,
        check: impl Fn(&T, &R) -> Result<(), String>,
    ) -> Result<Value, String> {
        let request: T = serde_json::from_value(input.clone()).map_err(|e| e.to_string())?;
        validate(&request)?;
        let result = evaluate(&request)?;
        check(&request, &result)?;
        serde_json::to_value(result).map_err(|e| e.to_string())
    }
    let out = match contract {
        "scroll" => go::<scrolls::Request, _>(input, |r| r.validate(), |r| r.evaluate(), |r, x| r.validate_result(x)),
        "card_paint" => go::<card_paint::Request, _>(input, |r| r.validate(), |r| r.evaluate(), |r, x| r.validate_result(x)),
        "wish" => go::<wish_jobs::PolicyRequest, _>(input, |r| r.validate(), |r| r.evaluate(), |r, x| r.validate_result(x)),
        "reforge" => go::<reforge_jobs::PolicyRequest, _>(input, |r| r.validate(), |r| r.evaluate(), |r, x| r.validate_result(x)),
        "brew" => go::<brew_jobs::PolicyRequest, _>(input, |r| r.validate(), |r| r.evaluate(), |r, x| r.validate_result(x)),
        "home" => go::<home_settlement::HomePolicyRequest, _>(input, |r| r.validate(), |r| r.evaluate(), |r, x| r.validate_result(x)),
        other => panic!("no contract {other}"),
    };
    match out {
        Ok(v) => json!({"Ok": v}),
        Err(e) => json!({"Err": e}),
    }
}

/// The replay corpus's states, sampled by `scripts` into plain JSON files
/// (`$HOME_STATES`, required), restored as
/// Runs in name order. `step` keeps every `step`th.
pub fn states(step: usize) -> Vec<(String, crate::rules::Run)> {
    let dir = std::env::var_os("HOME_STATES")
        .map(PathBuf::from)
        .expect("set HOME_STATES to the sampled home states directory");
    let mut files: Vec<PathBuf> = std::fs::read_dir(&dir)
        .unwrap_or_else(|e| panic!("{}: {e}", dir.display()))
        .filter_map(|e| e.ok().map(|e| e.path()))
        .filter(|p| p.extension().is_some_and(|x| x == "json"))
        .collect();
    files.sort();
    files
        .into_iter()
        .enumerate()
        .filter(|(i, _)| i % step.max(1) == 0)
        .filter_map(|(_, p)| {
            let text = std::fs::read_to_string(&p).ok()?;
            let value: Value = serde_json::from_str(&text).ok()?;
            let run = crate::rules::runtime_state::restore_value(&value).ok()?;
            Some((p.file_stem()?.to_string_lossy().into_owned(), run))
        })
        .collect()
}
