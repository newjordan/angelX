//! The server's knobs (`angel_net::knob`): the engine's own and the game's,
//! set between ticks on the sim, told to every client, and kept in a tune
//! file beside the session so a restart plays the same numbers.
//!
//! Who may set one: the host, through its handle (`Server::tune`) or an
//! in-process connection, always; anyone else only in a session started in
//! dev mode (`ServeConfig::dev`). Everyone is told the table either way, so
//! a panel shows what the game is running at.

use angel_net::knob::{self, Knob, Side};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

/// The engine's server knobs: how many ticks apart snapshots go out.
///
/// **The input queue's seam.** The per-seat command queue's numbers
/// (`angel_net::cmd`) are not knobs yet: when they become fields a queue
/// can be given, they are listed here beside `snapshot_every`, and
/// `Engine::set` hands a new value to every seat's queue
/// (`sim::Stepper::set_engine_knob`), as it hands `snapshot_every` to the
/// sim and the writers.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct Engine {
    /// Ticks between snapshots: `tick_hz / snapshot_hz` until moved.
    pub snapshot_every: u32,
    /// What the game's rates said, the knob's default.
    pub snapshot_every_default: u32,
}

/// The most ticks between two snapshots a slider reaches: a tenth of a
/// second at a 60 Hz tick, a fifth at 30.
const MAX_SNAPSHOT_EVERY: u32 = 6;

impl Engine {
    pub fn new(tick_hz: u32, snapshot_hz: u32) -> Engine {
        let every = (tick_hz / snapshot_hz.max(1)).max(1);
        Engine { snapshot_every: every, snapshot_every_default: every }
    }

    pub fn knobs(&self) -> Vec<Knob> {
        vec![
            Knob::new(
                "snapshot_every",
                "net",
                "Ticks between snapshots",
                "Rates::snapshot_hz (the game's rates(), as tick_hz / it)",
                Side::Server,
            )
            .range(1.0, f64::from(MAX_SNAPSHOT_EVERY.max(self.snapshot_every_default)), 1.0)
            .default(f64::from(self.snapshot_every_default))
            .at(f64::from(self.snapshot_every)),
        ]
    }

    /// Set one of the engine's knobs (already fitted): whether it is one.
    pub fn set(&mut self, name: &str, value: f64) -> bool {
        match name {
            "snapshot_every" => {
                self.snapshot_every = (value as u32).max(1);
                true
            }
            _ => false,
        }
    }
}

/// A tune file: `{"name": value, ...}`, the knobs moved from their defaults.
pub(crate) fn file_json(knobs: &[Knob]) -> String {
    let mut out = String::from("{");
    let mut first = true;
    for k in knobs.iter().filter(|k| k.side == Side::Server && k.value != k.default) {
        if !first {
            out.push(',');
        }
        first = false;
        out.push_str("\n  ");
        knob::json_str(k.name, &mut out);
        out.push_str(": ");
        knob::json_num(k.value, &mut out);
    }
    out.push_str(if first { "}\n" } else { "\n}\n" });
    out
}

/// A tune file's numbers, by name: a flat JSON object of names and numbers.
/// Anything else in it is skipped, never an error: a file from another
/// build keeps what still reads.
pub(crate) fn read_file(text: &str) -> Vec<(String, f64)> {
    let body = text.trim();
    let Some(body) = body.strip_prefix('{').and_then(|b| b.strip_suffix('}')) else {
        return Vec::new();
    };
    body.split(',')
        .filter_map(|pair| {
            let (name, value) = pair.split_once(':')?;
            let name = name.trim().strip_prefix('"')?.strip_suffix('"')?;
            let value: f64 = value.trim().parse().ok()?;
            (knob::valid_name(name) && value.is_finite()).then(|| (name.to_string(), value))
        })
        .collect()
}

/// Read a tune file; nothing when there is none or it does not read.
pub(crate) fn load(path: &Path) -> Vec<(String, f64)> {
    std::fs::read_to_string(path).map(|t| read_file(&t)).unwrap_or_default()
}

/// Write a tune file off the sim's thread (a knob moves seldom, and a disk
/// may be slow), whole or not at all. Writes take turns under `written`,
/// which holds the newest `generation` on disk: a write that lost the race
/// to a newer one is dropped, so the file ends as the last move left it.
pub(crate) fn save(path: PathBuf, json: String, generation: u64, written: Arc<Mutex<u64>>) {
    let _ = std::thread::Builder::new().name("angel-tune".into()).spawn(move || {
        let mut newest = written.lock().unwrap_or_else(|p| p.into_inner());
        if generation <= *newest {
            return;
        }
        if let Some(dir) = path.parent() {
            let _ = std::fs::create_dir_all(dir);
        }
        let temp = path.with_extension("json.tmp");
        if std::fs::write(&temp, json.as_bytes()).is_ok() && std::fs::rename(&temp, &path).is_err() {
            let _ = std::fs::remove_file(&temp);
        }
        *newest = generation;
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_tune_file_keeps_only_what_moved_and_reads_back() {
        let mut engine = Engine::new(30, 30);
        let mut knobs = engine.knobs();
        assert_eq!(file_json(&knobs), "{}\n");
        engine.set("snapshot_every", 2.0);
        knobs = engine.knobs();
        knobs.push(Knob::new("pace", "g", "Pace", "PACE", Side::Server).range(0.0, 10.0, 0.5).default(4.0).at(5.5));
        knobs.push(Knob::new("art", "g", "Art", "ART", Side::Client).default(0.0).at(1.0));
        let text = file_json(&knobs);
        assert_eq!(text, "{\n  \"snapshot_every\": 2,\n  \"pace\": 5.5\n}\n");
        assert_eq!(read_file(&text), vec![("snapshot_every".to_string(), 2.0), ("pace".to_string(), 5.5)]);
        assert_eq!(read_file("{\"ok\": 1, \"bad name\": 2, \"nan\": \"x\"}"), vec![("ok".to_string(), 1.0)]);
        assert!(read_file("not json").is_empty());
    }

    #[test]
    fn the_snapshot_knob_starts_at_the_games_rates() {
        let e = Engine::new(30, 15);
        let k = &e.knobs()[0];
        assert_eq!((k.default, k.value, k.min, k.max), (2.0, 2.0, 1.0, 6.0));
    }
}
