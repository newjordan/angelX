//! The sounds the rules make, as the wire and a seat's page hear them: every
//! name plays a shipped file, and a busy fight never machine-guns one.

use super::*;
use crate::render::overworld::arena::testing;
use crate::rules::{HZ, Input, Phase, bosses};
use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

/// The shipped effects, as the bundle names them (`crates/web/delve/sounds.list`):
/// `sfx/<name>.mp3` is `<name>`, `sfx/knights/<clip>.mp3` is `knights-<clip>`.
fn shipped() -> BTreeSet<String> {
    let sfx = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../cockpit/assets/dungeon/sfx");
    let mut names = BTreeSet::new();
    for (dir, prefix) in [(sfx.clone(), ""), (sfx.join("knights"), "knights-")] {
        for entry in std::fs::read_dir(&dir).expect("the Delve's sound folder") {
            let path = entry.unwrap().path();
            if path.extension().is_some_and(|e| e == "mp3") {
                names.insert(format!("{prefix}{}", path.file_stem().unwrap().to_string_lossy()));
            }
        }
    }
    names
}

/// Every Rust file of the rules.
fn rules_files() -> Vec<PathBuf> {
    let src = Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
    let mut files = vec![src.join("rules.rs")];
    let mut dirs = vec![src.join("rules")];
    while let Some(dir) = dirs.pop() {
        for entry in std::fs::read_dir(&dir).unwrap() {
            let path = entry.unwrap().path();
            if path.is_dir() {
                dirs.push(path);
            } else if path.extension().is_some_and(|e| e == "rs")
                // A test module beside the rules plays stand-in names, not sounds.
                && !path.file_stem().is_some_and(|s| s == "tests" || s.to_string_lossy().ends_with("_tests"))
            {
                files.push(path);
            }
        }
    }
    files
}

/// The text from just after an open bracket to its close.
fn bracketed(text: &str) -> &str {
    let mut depth = 1;
    let mut quoted = false;
    let mut escaped = false;
    for (i, c) in text.char_indices() {
        match c {
            _ if escaped => escaped = false,
            '\\' if quoted => escaped = true,
            '"' => quoted = !quoted,
            '(' | '[' | '{' if !quoted => depth += 1,
            ')' | ']' | '}' if !quoted => {
                depth -= 1;
                if depth == 0 {
                    return &text[..i];
                }
            }
            _ => {}
        }
    }
    text
}

/// The quoted names in `text` that could be a sound's.
fn quoted_names(text: &str) -> impl Iterator<Item = &str> {
    text.split('"').skip(1).step_by(2).filter(|s| {
        !s.is_empty() && s.bytes().all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'_' || b == b'/')
    })
}

/// Where the rules hand a sound over: a run's or a session's sound list, a
/// sound helper, a side-scroller's `say`, Moonfall's own `cues`, and the
/// named hooks (`SOUND_*`, `CALL_SOUND`, `CHAIN_PITCH`).
const SINKS: [&str; 8] = [
    "sounds.push(",
    "sounds.extend(",
    "sounds.insert(",
    "snd.push(",
    "say.push(",
    "noises.push(",
    ".sound(",
    "cues.push(",
];

/// Every sound name the rules' source hands over at a sink, with where.
fn named_sounds() -> BTreeMap<String, String> {
    let mut found = BTreeMap::new();
    for file in rules_files() {
        let raw = std::fs::read_to_string(&file).unwrap();
        // Comments say what a sound is for; only code names one.
        let text: String = raw.lines().map(|l| l.split("//").next().unwrap_or("")).collect::<Vec<_>>().join("\n");
        let place = file.file_name().unwrap().to_string_lossy().to_string();
        let mut spans: Vec<&str> = Vec::new();
        for sink in SINKS {
            // A run's `cues` are the chorus's moments; only Moonfall's are sounds.
            if sink == "cues.push(" && !file.to_string_lossy().contains("moonfall") {
                continue;
            }
            spans.extend(text.match_indices(sink).map(|(i, _)| bracketed(&text[i + sink.len()..])));
        }
        for line in text.lines().filter(|l| l.contains("const ") && l.contains("&str")) {
            let name = line.split("const ").nth(1).unwrap_or("").split(':').next().unwrap_or("");
            if name.contains("SOUND") || name.contains("PITCH") {
                spans.push(line);
            }
        }
        for span in spans {
            // A knight's clip is named by its moment (`chatter::clip`): those
            // are checked whole below.
            if span.contains("clip(") {
                continue;
            }
            for name in quoted_names(span) {
                found.entry(name.to_string()).or_insert_with(|| place.clone());
            }
        }
    }
    found
}

#[test]
fn every_sound_the_rules_name_plays_a_shipped_file() {
    let shipped = shipped();
    let named = named_sounds();
    assert!(named.len() > 80, "the scan found {} names", named.len());
    for (name, place) in &named {
        assert!(shipped.contains(&sound_file(name)), "{place}: `{name}` plays nothing (no sfx/{}.mp3)", sound_file(name));
    }
    for name in crate::rules::chatter::names() {
        assert!(shipped.contains(&sound_file(name)), "the knights' `{name}` has no file");
    }
    // The boss fights' own cues (`furnace`, `court`, `rubble`).
    for name in ["clang", "crack_open", "vent_fire", "armour_fall", "shield_block", "rock_land"] {
        assert!(named.contains_key(name) && shipped.contains(name), "{name}");
    }
}

#[test]
fn a_hook_without_its_own_take_plays_its_stand_in() {
    let shipped = shipped();
    for &(hook, take) in STAND_INS {
        assert!(!shipped.contains(hook), "{hook} has its own take now: drop its stand-in");
        assert!(shipped.contains(take), "{hook}'s stand-in {take} is not shipped");
        assert_eq!(sound_file(hook), take);
    }
    for hook in [
        crate::rules::shoulders::SOUND_CLIMB,
        crate::rules::shoulders::SOUND_HOP,
        crate::rules::shoulders::SOUND_THROW,
        crate::rules::shoulders::SOUND_SLAM,
    ] {
        assert!(shipped.contains(&sound_file(hook)), "{hook}");
    }
    // Everything else keeps its own name.
    assert_eq!(sound_file("hit_blade"), "hit_blade");
    assert_eq!(sound_file("knights/a_cheer_1"), "knights-a_cheer_1");
}

/// What one fight sounded like, a name at a time, at the seat's page: what
/// the rules pushed and what the page played after `client::sound_gap`.
#[derive(Default)]
struct Heard {
    pushed: u32,
    played: u32,
    /// The most played in any one second.
    peak: u32,
    /// The longest run of ticks, one after another, that pushed it.
    streak: u32,
    run: u32,
    last_push: Option<u32>,
    last_play: Option<u32>,
    window: Vec<u32>,
}

/// Four knights fight `run` for `seconds`, kept on their feet and aiming at
/// the nearest foe, the room filled again when it clears: each sound that
/// reached the wire, pushed and played.
fn listen(mut run: Run, seconds: u32) -> BTreeMap<String, Heard> {
    for seat in 2..=4 {
        run.join_seat(seat, &format!("Knight {seat}"));
    }
    let room = run.at;
    let mut heard: BTreeMap<String, Heard> = BTreeMap::new();
    for t in 0..seconds * HZ {
        let inputs: BTreeMap<u32, Input> = run
            .players
            .iter()
            .map(|(&id, h)| {
                let foe = run.enemies.iter().filter(|e| e.hp > 0).min_by(|a, b| {
                    (a.x - h.x).hypot(a.y - h.y).total_cmp(&(b.x - h.x).hypot(b.y - h.y))
                });
                let (ax, ay) = foe.map_or((1.0, 0.0), |e| (e.x - h.x, e.y - h.y));
                let sign = |v: f32| if v.abs() < 0.5 { 0 } else { v.signum() as i8 };
                let input = Input {
                    aim_x: sign(ax),
                    aim_y: sign(ay),
                    move_x: if (t / 40 + id) % 2 == 0 { 1 } else { -1 },
                    move_y: sign(-ay),
                    fire: true,
                    swing: (t + id * 7) % 20 < 10,
                    dash: (t + id * 17) % 70 == 0,
                    ..Default::default()
                };
                (id, input)
            })
            .collect();
        run.step(&inputs);
        for hero in run.players.values_mut() {
            hero.hp = hero.max_hp;
        }
        if run.phase != Phase::Fighting && run.at == room && !run.hitstop.frozen() {
            run.enter_for_test(room);
        }
        run.cues.clear();
        let _ = take_notices(&mut run);
        let mut this_tick = BTreeSet::new();
        for event in take_sounds(&mut run) {
            let Event::Sound { name, .. } = event else { continue };
            let h = heard.entry(name.clone()).or_default();
            h.pushed += 1;
            if this_tick.insert(name.clone()) {
                h.run = if h.last_push == Some(t.wrapping_sub(1)) { h.run + 1 } else { 1 };
                h.streak = h.streak.max(h.run);
                h.last_push = Some(t);
            }
            // The page's own gap (`DelveClient::events`).
            let gap = crate::client::sound_gap(&name) * HZ / 1000;
            if h.last_play.is_some_and(|at| t.wrapping_sub(at) < gap) {
                continue;
            }
            h.last_play = Some(t);
            h.played += 1;
            h.window.retain(|&at| t - at < HZ);
            h.window.push(t);
            h.peak = h.peak.max(h.window.len() as u32);
        }
    }
    heard
}

/// `cargo test sound_rates -- --ignored --nocapture`: each name's sounds a
/// second, pushed and played, its busiest second and its longest run of
/// ticks, in an ordinary fight and against every guardian.
#[test]
#[ignore]
fn sound_rates() {
    const SECONDS: u32 = 60;
    let mut scenes: Vec<(String, Run)> = vec![("fight".into(), testing::fight())];
    for (id, _) in bosses::BUILTIN.iter().chain(bosses::CHECKED) {
        if !scenes.iter().any(|(name, _)| name == id) {
            scenes.push((id.to_string(), testing::boss(id)));
        }
    }
    let mut worst: BTreeMap<String, (f32, u32, u32, String)> = BTreeMap::new();
    for (scene, run) in scenes {
        println!("== {scene}: four knights, {SECONDS} s");
        println!("  {:<18} {:>8} {:>8} {:>6} {:>7}", "sound", "pushed/s", "played/s", "peak/s", "streak");
        for (name, h) in listen(run, SECONDS) {
            let (pushed, played) = (h.pushed as f32 / SECONDS as f32, h.played as f32 / SECONDS as f32);
            println!("  {name:<18} {pushed:>8.2} {played:>8.2} {:>6} {:>7}", h.peak, h.streak);
            let w = worst.entry(name).or_insert((0.0, 0, 0, String::new()));
            if played > w.0 {
                w.0 = played;
                w.3 = scene.clone();
            }
            w.1 = w.1.max(h.peak);
            w.2 = w.2.max(h.streak);
        }
    }
    println!("== every scene: each name's worst");
    println!("  {:<18} {:>8} {:>6} {:>7}  scene", "sound", "played/s", "peak/s", "streak");
    for (name, (played, peak, streak, scene)) in worst {
        println!("  {name:<18} {played:>8.2} {peak:>6} {streak:>7}  {scene}");
    }
}
