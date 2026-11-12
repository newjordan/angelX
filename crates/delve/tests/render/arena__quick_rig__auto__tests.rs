//! Parity tests for the in-process quick-rig auto-rigger (Python port).
//!
//! Fixtures: `tests/fixtures/quick-rig/` — `frog.txt`/`squire.txt` ink rows and
//! the Python-generated `*.game.json` (venv `tools/sprite-rig` `quick fit` +
//! `quick build`). The Rust port must reproduce the Python game JSON exactly.

use crate::render::overworld::arena::quick_rig::{
    auto::{auto_rig, auto_rig_json, auto_rig_plan, auto_rig_smart_json},
    smart::{fit_smart, rig_json_smart},
    Clip, Rig,
};
use serde_json::Value;
use std::time::Instant;

const FROG_ROWS: &str = include_str!("../fixtures/quick-rig/frog.txt");
const FROG_GAME: &str = include_str!("../fixtures/quick-rig/frog.game.json");
const SQUIRE_ROWS: &str = include_str!("../fixtures/quick-rig/squire.txt");
const SQUIRE_GAME: &str = include_str!("../fixtures/quick-rig/squire.game.json");

fn rows(text: &str) -> Vec<&str> {
    text.lines().filter(|l| !l.is_empty()).collect()
}

/// Compare semantic JSON; print the first differences frame by frame.
fn assert_game_parity(produced: &str, expected: &str, label: &str) {
    let got: Value = serde_json::from_str(produced).expect("produced parses as JSON");
    let want: Value = serde_json::from_str(expected).expect("fixture parses as JSON");
    assert_eq!(got["version"], want["version"], "{label}: version");
    assert_eq!(got["canvas"], want["canvas"], "{label}: canvas");
    assert_eq!(got["pivot"], want["pivot"], "{label}: pivot");
    let got_clips = got["clips"].as_object().expect("produced clips");
    let want_clips = want["clips"].as_object().expect("fixture clips");
    assert_eq!(
        got_clips.len(),
        want_clips.len(),
        "{label}: clip count {} vs {}",
        got_clips.len(),
        want_clips.len()
    );
    for (name, want_frames) in want_clips {
        let got_frames = got_clips
            .get(name)
            .unwrap_or_else(|| panic!("{label}: missing clip {name}"));
        let gf = got_frames.as_array().expect("frames array");
        let wf = want_frames.as_array().expect("frames array");
        assert_eq!(gf.len(), wf.len(), "{label}/{name}: frame count");
        for (i, (g, w)) in gf.iter().zip(wf).enumerate() {
            assert_eq!(g["ticks"], w["ticks"], "{label}/{name}[{i}]: ticks");
            let gr = g["rows"].as_array().unwrap();
            let wr = w["rows"].as_array().unwrap();
            assert_eq!(gr.len(), wr.len(), "{label}/{name}[{i}]: row count");
            for (y, (a, b)) in gr.iter().zip(wr).enumerate() {
                assert_eq!(
                    a.as_str().unwrap(),
                    b.as_str().unwrap(),
                    "{label}/{name}[{i}] row {y}"
                );
            }
            let ga = g["anchors"].as_object().unwrap();
            let wa = w["anchors"].as_object().unwrap();
            assert_eq!(ga.len(), wa.len(), "{label}/{name}[{i}]: anchor count");
            for (k, v) in wa {
                assert_eq!(
                    ga.get(k).unwrap(),
                    v,
                    "{label}/{name}[{i}] anchor {k}"
                );
            }
        }
    }
}

#[test]
fn auto_rig_frog_matches_python_game_json() {
    let started = Instant::now();
    let json = auto_rig_json(&rows(FROG_ROWS)).expect("frog rigs");
    let elapsed = started.elapsed();
    assert_game_parity(&json, FROG_GAME, "frog");
    // The output must be loadable by the runtime parser itself.
    Rig::parse(&json).expect("frog game.json parses as Rig");
    println!("auto_rig frog: {elapsed:?}");
}

#[test]
fn auto_rig_squire_matches_python_game_json() {
    let started = Instant::now();
    let json = auto_rig_json(&rows(SQUIRE_ROWS)).expect("squire rigs");
    let elapsed = started.elapsed();
    assert_game_parity(&json, SQUIRE_GAME, "squire");
    Rig::parse(&json).expect("squire game.json parses as Rig");
    println!("auto_rig squire: {elapsed:?}");
}

#[test]
fn auto_rig_returns_parsed_rig_with_all_clips() {
    let rig = auto_rig(&rows(FROG_ROWS)).expect("frog rigs");
    for clip in Clip::ALL {
        assert!(rig.has(clip), "rig missing {clip:?}");
    }
    let rig2 = auto_rig(&rows(SQUIRE_ROWS)).expect("squire rigs");
    for clip in Clip::ALL {
        assert!(rig2.has(clip), "squire rig missing {clip:?}");
    }
}

#[test]
fn auto_rig_blank_sprite_is_an_error_not_a_panic() {
    let blank = vec!["...."; 4];
    match auto_rig(&blank) {
        Ok(rig) => {
            // A valid rig must still parse.
            let json = auto_rig_json(&blank).expect("json matches rig result");
            Rig::parse(&json).expect("blank-sprite rig parses");
            let _ = rig;
        }
        Err(message) => {
            assert!(!message.is_empty(), "error must carry a message");
        }
    }
}

#[test]
fn auto_rig_one_pixel_sprite_is_an_error_not_a_panic() {
    let one = vec!["..."; 3];
    let mut one = one;
    one[1] = ".W.";
    match auto_rig(&one) {
        Ok(rig) => {
            let json = auto_rig_json(&one).expect("json matches rig result");
            Rig::parse(&json).expect("one-pixel rig parses");
            let _ = rig;
        }
        Err(message) => assert!(!message.is_empty(), "error must carry a message"),
    }
}

#[test]
fn auto_rig_timing_per_sprite() {
    // Measures the full fit+build pipeline on both fixtures.
    for (label, text) in [("frog", FROG_ROWS), ("squire", SQUIRE_ROWS)] {
        let started = Instant::now();
        let json = auto_rig_json(&rows(text)).expect("rigs");
        let elapsed = started.elapsed();
        println!("auto_rig_json {label}: {elapsed:?} ({} bytes)", json.len());
        assert!(
            elapsed.as_secs() < 30,
            "{label}: auto_rig took {elapsed:?}, over the 30s budget"
        );
    }
}

const SMART_CASES: [(&str, &str); 7] = [
    ("frog", include_str!("../fixtures/quick-rig/smart/frog.txt")),
    ("mage", include_str!("../fixtures/quick-rig/smart/mage.txt")),
    ("ogre", include_str!("../fixtures/quick-rig/smart/ogre.txt")),
    (
        "skeleton",
        include_str!("../fixtures/quick-rig/smart/skeleton.txt"),
    ),
    ("imp", include_str!("../fixtures/quick-rig/smart/imp.txt")),
    ("knight", include_str!("../fixtures/quick-rig/smart/knight.txt")),
    ("witch", include_str!("../fixtures/quick-rig/smart/witch.txt")),
];

fn smart_fixture(name: &str, kind: &str) -> String {
    std::fs::read_to_string(format!(
        "{}/tests/fixtures/quick-rig/smart/{name}.{kind}",
        env!("CARGO_MANIFEST_DIR")
    ))
    .expect("fixture")
}

fn delve_rig_json_smart(r: &[&str]) -> Result<String, String> {
    let grid: Vec<Vec<char>> = r.iter().map(|l| l.chars().collect()).collect();
    let fitted = fit_smart(&grid)?;
    rig_json_smart(&fitted)
}

fn delve_smart_json(r: &[&str]) -> Result<Value, String> {
    let text = auto_rig_smart_json(r)?;
    serde_json::from_str(&text).map_err(|e| e.to_string())
}

#[test]
fn auto_rig_smart_matches_python_fixtures() {
    for (name, text) in SMART_CASES {
        let r = rows(text);
        // rig.json parity: masks, joints, anchors and plan.
        let want_rig: serde_json::Value =
            serde_json::from_str(&smart_fixture(name, "rig.json")).unwrap();
        assert_eq!(
            auto_rig_plan(&r).unwrap(),
            want_rig["plan"].as_str().unwrap(),
            "{name}: auto_rig_plan disagrees with the fixture"
        );
        let smart = delve_rig_json_smart(&r).expect("smart fit");
        let got_rig: serde_json::Value = serde_json::from_str(&smart).unwrap();
        assert_eq!(got_rig["plan"], want_rig["plan"], "{name}: plan mismatch");
        assert_eq!(
            got_rig["joints"], want_rig["joints"],
            "{name}: joints mismatch"
        );
        assert_eq!(
            got_rig["anchors"], want_rig["anchors"],
            "{name}: anchors mismatch"
        );
        for (part, want) in want_rig["parts"].as_object().unwrap() {
            assert_eq!(
                got_rig["parts"][part], *want,
                "{name}: part {part} mask mismatch"
            );
        }
        // game.json parity: rows, ticks and anchors, clip by clip.
        let want_game: serde_json::Value =
            serde_json::from_str(&smart_fixture(name, "game.json")).unwrap();
        let got = delve_smart_json(&r).expect("smart build");
        assert_eq!(got["canvas"], want_game["canvas"], "{name}: canvas");
        assert_eq!(got["pivot"], want_game["pivot"], "{name}: pivot");
        let want_clips = want_game["clips"].as_object().unwrap();
        let got_clips = got["clips"].as_object().unwrap();
        for (clip, frames) in want_clips {
            let got_frames = got_clips[clip].as_array().unwrap();
            for (i, want_frame) in frames.as_array().unwrap().iter().enumerate() {
                assert_eq!(
                    got_frames[i]["rows"], want_frame["rows"],
                    "{name}/{clip}[{i}]: {}",
                    first_row_diff(&got_frames[i]["rows"], &want_frame["rows"])
                );
                assert_eq!(
                    got_frames[i]["ticks"], want_frame["ticks"],
                    "{name}/{clip}[{i}]: ticks mismatch"
                );
                assert_eq!(
                    got_frames[i]["anchors"], want_frame["anchors"],
                    "{name}/{clip}[{i}]: anchors mismatch"
                );
            }
        }
    }
}

fn first_row_diff(got: &serde_json::Value, want: &serde_json::Value) -> String {
    let g = got.as_array().cloned().unwrap_or_default();
    let w = want.as_array().cloned().unwrap_or_default();
    for y in 0..g.len().min(w.len()) {
        let gs = g[y].as_str().unwrap_or("");
        let ws = w[y].as_str().unwrap_or("");
        if gs != ws {
            let cols: Vec<usize> = gs.chars().zip(ws.chars()).enumerate().filter(|(_, (a,b))| a!=b).map(|(i,_)| i).collect();
            let c = cols.first().copied().unwrap_or(0);
            let lo = c.saturating_sub(4);
            return format!(
                "row {y} col {c} ({} cols) got {:?} want {:?}",
                cols.len(),
                gs.chars().skip(lo).take(12).collect::<String>(),
                ws.chars().skip(lo).take(12).collect::<String>()
            );
        }
    }
    format!("row count got {} want {}", g.len(), w.len())
}
