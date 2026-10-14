//! Every sound the Delve makes is one the cockpit's audio has.

use crate::drive::together_shooter::chatter::names;

#[test]
fn every_clip_is_a_short_file() {
    let dir = crate::drive::together_audio::sound_dir().expect("the Delve's sound folder");
    assert_eq!(names().len(), 4 * 12 * 2);
    for name in names() {
        let file = dir.join("sfx").join(format!("{name}.mp3"));
        let size = std::fs::metadata(&file).map(|m| m.len()).unwrap_or(0);
        assert!(size > 1_000, "{name}: {size} bytes");
        // 96 kbit/s: a few words at most, a grunt well under a second.
        assert!(size < 3 * 12_000, "{name} is short");
    }
}

#[test]
fn every_sound_the_delve_makes_is_one_the_audio_knows() {
    // The Delve's rules, in `angel_delve`.
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../crates/delve/src");
    let mut files = vec![root.join("rules.rs")];
    let mut dirs = vec![root.join("rules")];
    while let Some(dir) = dirs.pop() {
        for entry in std::fs::read_dir(&dir).unwrap() {
            let path = entry.unwrap().path();
            if path.is_dir() {
                dirs.push(path);
            } else if path.extension().is_some_and(|e| e == "rs") {
                files.push(path);
            }
        }
    }
    let known = crate::drive::together_audio::SFX;
    let mut heard = 0;
    for file in files {
        let text = std::fs::read_to_string(&file).unwrap();
        for (_, rest) in text.match_indices("sounds.push(\"").map(|(i, _)| text.split_at(i + 13)) {
            let name = &rest[..rest.find('"').unwrap()];
            assert!(known.contains(&name), "{}: {name}", file.display());
            heard += 1;
        }
    }
    assert!(heard > 50, "{heard}");
}

#[test]
fn every_new_sound_is_named_and_has_a_file() {
    let dir = crate::drive::together_audio::sound_dir().expect("the sound folder");
    for name in [
        "knight_hit",
        "knight_hit_big",
        "hit_blade",
        "hit_axe",
        "hit_arrow",
        "hit_bolt",
        "hit_blast",
        "hit_magic",
        "step_stone",
        "step_gravel",
        "step_ash",
        "step_wet",
        "step_moss",
        "step_wood",
        "door_pass",
        "gold",
        "potion",
        "holy_water",
        "bomb_toss",
        "shield_up",
        "axe_spin",
        "helmet_pop",
        "armour_fall",
        "boss_roar",
        "boss_rage",
        "boss_death",
        "knight_fall",
        "bones_clatter",
        "crack_open",
        "hand_rise",
        "hand_grab",
        "hand_drag",
        "ground_close",
        "hell_gate_rise",
        "hell_gate_enter",
        "circle_toll",
    ] {
        assert!(crate::drive::together_audio::SFX.contains(&name), "{name}");
        assert!(
            dir.join("sfx").join(format!("{name}.mp3")).is_file(),
            "{name}"
        );
    }
}
