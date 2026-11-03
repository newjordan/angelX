//! The VFX lab: every arm a knight can hold, fired once in a quiet fight room
//! beside a control run that holds nothing, the two pictures differenced tick
//! by tick. Frames, the arm's tray icon and one metrics file per arm land in
//! `ANGEL_LAB_DIR/<arm>/`.

use super::*;
use crate::rules::{Input, Phase, RoomKind, Weapon};
use std::collections::BTreeMap;

const TICKS: i32 = 45;
const CROP_W: i32 = 192;
const CROP_H: i32 = 96;
const KNIGHT: (f32, f32) = (10.0, 14.0);

/// How an arm is held for the lab: the kit or arm, its attack input, and how
/// far the dummy stands.
struct Arm {
    name: &'static str,
    class: &'static str,
    setup: fn(&mut Run),
    input: fn() -> Input,
    reach: f32,
}

/// A plain arm as the rack hands it over: the weapon and its card, so the
/// tray shows the card's own art (`Hero::equip`).
fn plain(w: Weapon, card: Option<&'static str>) -> impl Fn(&mut Run) {
    move |r: &mut Run| {
        let h = r.players.get_mut(&1).unwrap();
        h.weapon = w;
        h.forged = None;
        h.arm = card.map(Into::into);
    }
}

fn forged(r: &mut Run, arm: &str) {
    let card = r.book.get(arm).unwrap();
    let crate::rules::cards::Arm::Forged(w) = card.arm.as_ref().unwrap() else { panic!("forged arm") };
    let w = w.clone();
    let h = r.players.get_mut(&1).unwrap();
    h.arm = Some(arm.into());
    h.forged = Some(w);
}

fn fire() -> Input {
    Input { fire: true, aim_x: 1, ..Default::default() }
}

/// Q alone: an aim would also loose the held arm (aiming fires, `arms`).
fn swing() -> Input {
    Input { swing: true, ..Default::default() }
}

macro_rules! kit {
    ($name:literal, $class:literal, $reach:expr) => {
        Arm { name: $name, class: $class, setup: |r| assert!(r.take_up(1, $name)), input: fire, reach: $reach }
    };
}

macro_rules! arm {
    ($name:literal, $class:literal) => {
        Arm { name: $name, class: $class, setup: |r| forged(r, $name), input: fire, reach: 2.0 }
    };
}

fn arms() -> Vec<Arm> {
    vec![
        Arm { name: "fists", class: "melee", setup: |r| plain(Weapon::Fists, None)(r), input: fire, reach: 2.0 },
        Arm { name: "sword", class: "melee", setup: |_| {}, input: swing, reach: 2.0 },
        arm!("knights-blade", "melee"),
        arm!("round-table-blade", "melee"),
        arm!("twin-axes", "melee"),
        arm!("grave-axes", "melee"),
        arm!("grave-staff", "melee"),
        kit!("hammerer", "melee", 2.0),
        kit!("butcher", "melee", 2.0),
        kit!("ironbraid", "melee", 2.0),
        Arm { name: "bow", class: "ranged", setup: |r| plain(Weapon::Bow, Some("bow"))(r), input: fire, reach: 9.0 },
        Arm { name: "crossbow", class: "ranged", setup: |r| plain(Weapon::Crossbow, Some("crossbow"))(r), input: fire, reach: 9.0 },
        Arm { name: "handgonne", class: "ranged", setup: |r| plain(Weapon::Handgonne, Some("handgonne"))(r), input: fire, reach: 9.0 },
        Arm { name: "notes", class: "ranged", setup: |r| plain(Weapon::Notes, Some("note-stream"))(r), input: fire, reach: 9.0 },
        Arm { name: "stones", class: "ranged", setup: |r| plain(Weapon::Stones, Some("thrown-stones"))(r), input: fire, reach: 9.0 },
        kit!("farwatch", "ranged", 9.0),
        kit!("galeweaver", "ranged", 9.0),
        kit!("cinderheart", "ranged", 9.0),
        kit!("gravecaller", "ranged", 9.0),
        kit!("alchemist", "ranged", 9.0),
        kit!("bard", "ranged", 9.0),
        kit!("scryglass", "ranged", 9.0),
        // Notes fly 6 units (speed 18 for 10 ticks): a second trial in range.
        Arm { name: "notes-close", class: "ranged", setup: |r| plain(Weapon::Notes, Some("note-stream"))(r), input: fire, reach: 4.5 },
        Arm { name: "bard-close", class: "ranged", setup: |r| assert!(r.take_up(1, "bard")), input: fire, reach: 4.5 },
    ]
}

fn room(arm: &Arm) -> Run {
    let mut r = Run::new(21, 1, None);
    let room = r.dungeon.rooms.iter().position(|r| r.kind == RoomKind::Fight && !r.great()).unwrap();
    r.enter_for_test(room);
    r.calm_for_test();
    r.enemies.clear();
    r.projectiles.clear();
    (arm.setup)(&mut r);
    let h = r.players.get_mut(&1).unwrap();
    (h.x, h.y, h.aim_x, h.aim_y) = (KNIGHT.0, KNIGHT.1, 1.0, 0.0);
    h.invulnerable = 1000;
    r.spawn_at_for_test(crate::rules::EnemyKind::Demon, KNIGHT.0 + arm.reach, KNIGHT.1);
    for e in &mut r.enemies {
        (e.hp, e.max_hp, e.seen_hp) = (10000, 10000, 10000);
        e.sleep_until = 100000;
    }
    r.phase = Phase::Fighting;
    r.sounds.clear();
    r
}

fn crop(r: &Run) -> Img {
    let mut cv = (*scenery_cached(r)).clone();
    cv.stamp(&room_layer(r, None, true), 0, 0);
    let (x, y) = at(KNIGHT.0 + 4.5, KNIGHT.1);
    let mut out = Img::new(CROP_W, CROP_H);
    for dy in 0..CROP_H {
        for dx in 0..CROP_W {
            if let Some(c) = cv.get(x - CROP_W / 2 + dx, y - CROP_H * 3 / 5 + dy) {
                out.set(dx, dy, c);
            }
        }
    }
    out
}

fn png(path: &std::path::Path, im: &Img, k: u32) {
    image::RgbaImage::from_raw(im.w as u32 * k, im.h as u32 * k, im.rgba_scaled(k)).unwrap().save(path).unwrap();
}

fn luma(c: [u8; 3]) -> f32 {
    0.2126 * c[0] as f32 + 0.7152 * c[1] as f32 + 0.0722 * c[2] as f32
}

/// Warm: red clearly over blue, and bright enough to read as lit.
fn warm(c: [u8; 3]) -> bool {
    c[0] as i32 > c[2] as i32 + 24 && luma(c) > 40.0
}

/// One tick's difference from the control picture.
fn diff(a: &Img, control: &Img) -> (u32, f32, u32, u32, f32) {
    let (mut px, mut energy, mut warm_px, mut peak) = (0u32, 0f32, 0u32, 0f32);
    let mut colours = std::collections::BTreeSet::new();
    for y in 0..a.h {
        for x in 0..a.w {
            let (p, q) = (a.get(x, y).unwrap(), control.get(x, y).unwrap());
            if p != q {
                px += 1;
                let d = (luma(p) - luma(q)).abs();
                energy += d;
                peak = peak.max(luma(p));
                colours.insert(p);
                warm_px += u32::from(warm(p));
            }
        }
    }
    (px, energy, colours.len() as u32, warm_px, peak)
}

#[test]
#[ignore]
fn write_vfx_lab() {
    let dir = std::path::PathBuf::from(std::env::var_os("ANGEL_LAB_DIR").expect("lab directory"));
    for arm in arms() {
        let out = dir.join(arm.name);
        std::fs::create_dir_all(out.join("frames")).unwrap();
        let mut lit = room(&arm);
        let mut control = room(&arm);
        let keys = crate::rules::hud::Keys::FIRST;
        let icon = lit.tray(1, &keys).into_iter().find(|i| i.slot == crate::rules::tray::Slot::Weapon);
        if let Some(icon) = &icon {
            let cell = tray::cell(icon);
            png(&out.join("icon-1x.png"), &cell, 1);
            png(&out.join("icon-8x.png"), &cell, 8);
        }
        let foe = lit.enemies[0].id;
        let mut ticks = Vec::new();
        for t in 0..TICKS {
            let input = if t == 1 { (arm.input)() } else { Input::default() };
            lit.step(&BTreeMap::from([(1, input)]));
            control.step(&BTreeMap::from([(1, Input::default())]));
            let (a, c) = (crop(&lit), crop(&control));
            png(&out.join(format!("frames/t{t:03}.png")), &a, 1);
            if t == 0 {
                png(&out.join("control.png"), &c, 1);
            }
            let (px, energy, colours, warm_px, peak) = diff(&a, &c);
            let hp = lit.enemies.iter().find(|e| e.id == foe).map_or(0, |e| e.hp);
            let sounds: Vec<&str> = std::mem::take(&mut lit.sounds);
            control.sounds.clear();
            ticks.push(serde_json::json!({
                "t": t, "px": px, "energy": energy, "colours": colours, "warm_px": warm_px,
                "peak_luma": peak, "foe_hp": hp, "hitstop": lit.hitstop.left, "shake": lit.shake,
                "shots": lit.projectiles.len(), "sounds": sounds,
            }));
        }
        let meta = serde_json::json!({
            "arm": arm.name, "class": arm.class, "reach": arm.reach, "hz": crate::rules::HZ,
            "crop": [CROP_W, CROP_H], "icon": icon.is_some(), "ticks": ticks,
        });
        std::fs::write(out.join("metrics.json"), serde_json::to_string_pretty(&meta).unwrap()).unwrap();
    }
}
