//! The Bard in a fight, drawn tick by tick with the sounds each tick made:
//! the review clip (`ANGEL_BARD_LAB=<dir> cargo test ... write_bard_lab --
//! --ignored`), and the checks that every sound the Bard names is a file.
use super::*;
use crate::rules::{EnemyKind, Input, Phase, RoomKind, TELEGRAPH, bard as sim, knights, ults};
use std::collections::BTreeMap;

/// Floor one's first fight (seed 21): the Bard at (10, 14) facing right, a
/// training dummy 4.5 units off, two skeleton archers either side of it.
fn stage() -> Run {
    let mut r = Run::new(21, 1, None);
    let fight = r.dungeon.rooms.iter().position(|r| r.kind == RoomKind::Fight && !r.great()).unwrap();
    r.enter_for_test(fight);
    r.calm_for_test();
    r.enemies.clear();
    r.projectiles.clear();
    r.outfit(1, knights::knight("bard").unwrap());
    let h = r.players.get_mut(&1).unwrap();
    (h.x, h.y, h.aim_x, h.aim_y) = (10.0, 14.0, 1.0, 0.0);
    h.invulnerable = 100_000;
    h.ult_charge = ults::ULT_FULL;
    for (kind, x, y) in [(EnemyKind::Dummy, 14.5, 14.0), (EnemyKind::Skeleton, 14.0, 9.5), (EnemyKind::Skeleton, 14.5, 18.5)] {
        r.spawn_at_for_test(kind, x, y);
    }
    for e in &mut r.enemies {
        (e.hp, e.max_hp, e.seen_hp, e.age) = (10_000, 10_000, 10_000, TELEGRAPH);
    }
    r.phase = Phase::Fighting;
    r
}

/// What the Bard does on tick `t` of the clip: a riff and a half held (the
/// wub at two seconds, the overload at four), a breath, the Lullaby, then
/// playing on inside it.
fn script(t: u32) -> Input {
    let fire = Input { fire: true, aim_x: 1, ..Default::default() };
    match t {
        10..=200 => fire,
        212 => Input { ult: true, aim_x: 1, ..Default::default() },
        240..=290 => fire,
        _ => Input::default(),
    }
}

#[test]
#[ignore = "writes the Bard's review frames; run with ANGEL_BARD_LAB set"]
fn write_bard_lab() {
    let dir = std::path::PathBuf::from(std::env::var_os("ANGEL_BARD_LAB").expect("ANGEL_BARD_LAB"));
    let frames = dir.join("frames");
    std::fs::create_dir_all(&frames).unwrap();
    let mut r = stage();
    let (w, h) = (super::super::super::NATIVE_W, super::super::super::NATIVE_H);
    let (cw, ch) = (256, 144);
    let mut heard = Vec::new();
    for t in 0..330u32 {
        r.step(&BTreeMap::from([(1, script(t))]));
        for name in std::mem::take(&mut r.sounds) {
            heard.push(serde_json::json!([t, name]));
        }
        let frame = super::super::super::frame_for(&r, w, h, Some(1));
        let (bx, by) = super::super::super::frame_px(&r, w, h, Some(1), 10.0, 14.0);
        let (x0, y0) = ((bx - 96).clamp(0, frame.w - cw), (by - ch / 2 - 6).clamp(0, frame.h - ch));
        let mut crop = Img::new(cw, ch);
        for y in 0..ch {
            for x in 0..cw {
                if let Some(c) = frame.get(x0 + x, y0 + y) {
                    crop.set(x, y, c);
                }
            }
        }
        image::RgbaImage::from_raw(cw as u32 * 4, ch as u32 * 4, crop.rgba_scaled(4))
            .unwrap()
            .save(frames.join(format!("{t:04}.png")))
            .unwrap();
    }
    let groove = r.players[&1].groove;
    std::fs::write(
        dir.join("sounds.json"),
        serde_json::to_string(&serde_json::json!({ "hz": crate::rules::HZ, "sounds": heard, "groove_at_end": groove })).unwrap(),
    )
    .unwrap();
}

#[test]
fn every_sound_the_bard_names_is_a_shipped_file() {
    let sfx = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../cockpit/assets/dungeon/sfx");
    let names = sim::RIFF
        .iter()
        .chain(sim::SAX.iter())
        .chain(sim::GLITCHES.iter())
        .chain([sim::KICK, sim::HIT, sim::LULLABY, sim::WUB, sim::OVERLOAD].iter());
    for name in names {
        assert!(sfx.join(format!("{name}.mp3")).is_file(), "{name}.mp3 is missing");
    }
    for name in sim::RIFF {
        assert!(sim::SAX.contains(&name), "{name} is not in the mixers' list");
    }
}

#[test]
fn a_held_stream_draws_its_horn_its_beat_and_its_glitch() {
    let mut r = stage();
    let quiet = super::super::super::frame_for(&r, 384, 216, Some(1));
    let fire = Input { fire: true, aim_x: 1, ..Default::default() };
    r.step(&BTreeMap::from([(1, fire)]));
    let first = super::super::super::frame_for(&r, 384, 216, Some(1));
    assert_ne!(first.rgba_scaled(1), quiet.rgba_scaled(1), "the first volley shows");
    for _ in 0..(4 * sim::GLITCH_AT) {
        r.step(&BTreeMap::from([(1, fire)]));
    }
    assert!(sim::stage(r.players[&1].groove) == 2, "a held riff overloads");
    // A glitching shot is torn: red and blue either side of its gold.
    let mut mark = Img::new(25, 25);
    note_shot(&mut mark, (12, 12), (18.0, 0.0), 0);
    let clean = mark.clone();
    glitch_mark(&mut mark, 3);
    let red = super::super::super::super::ink::ink('7').unwrap();
    let blue = super::super::super::super::ink::ink('1').unwrap();
    let has = |im: &Img, c| (0..im.h).any(|y| (0..im.w).any(|x| im.get(x, y) == Some(c)));
    assert!(!has(&clean, red) && has(&mark, red) && has(&mark, blue));
    r.step(&BTreeMap::from([(1, Input::default())]));
    r.step(&BTreeMap::from([(1, Input::default())]));
    r.step(&BTreeMap::from([(1, Input::default())]));
    r.step(&BTreeMap::from([(1, Input::default())]));
    r.step(&BTreeMap::from([(1, Input::default())]));
    assert_eq!(r.players[&1].groove, 0, "letting go ends the groove");
}
