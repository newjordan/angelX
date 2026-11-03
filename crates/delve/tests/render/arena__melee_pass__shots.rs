//! The melee pass, looked at: each knight's arm at rest and in its swing,
//! the slash at its widest, a head taken, a body cut in two, and a bullet
//! turned back off the blade.
//!
//! `ANGEL_ARENA_SHOTS=<dir> cargo test write_melee_pass_shots -- --ignored`
//! writes whole frames (`frame_for`, as a seat sees them) and close crops
//! about the knight, as PPM.

use super::*;
use crate::rules::{EnemyKind, Input, Phase};
use std::collections::BTreeMap;

/// Where the knight stands: the middle of a bare fight room.
const KX: f32 = 22.0;
const KY: f32 = 14.0;

/// A quiet bare fight room: knight 1 at its middle with `kit`, facing east.
fn room(kit: &str) -> Run {
    let mut r = crate::rules::feel::testing::quiet();
    crate::rules::feel::testing::bare(&mut r);
    match kit {
        "sword" => {}
        "galahad" | "composer" | "ironbraid" | "hammerer" | "butcher" | "gravecaller" | "percival" => {
            assert!(r.take_up(1, kit));
        }
        arm => {
            let card = r.book.get(arm).unwrap();
            let crate::rules::cards::Arm::Forged(w) = card.arm.as_ref().unwrap() else {
                panic!("melee arm")
            };
            let h = r.players.get_mut(&1).unwrap();
            h.arm = Some(arm.into());
            h.forged = Some(w.clone());
        }
    }
    let h = r.players.get_mut(&1).unwrap();
    (h.x, h.y, h.aim_x, h.aim_y) = (KX, KY, 1.0, 0.0);
    h.invulnerable = 100_000;
    r.phase = Phase::Fighting;
    r
}

/// A foe of `kind` at `(x, y)`, awake but holding still, with `hp`.
fn foe(r: &mut Run, kind: EnemyKind, x: f32, y: f32, hp: u32) -> u32 {
    r.spawn_at_for_test(kind, x, y);
    let e = r.enemies.last_mut().unwrap();
    (e.hp, e.max_hp, e.seen_hp) = (hp, hp, hp);
    e.frozen = 100_000;
    e.sleep_until = 0;
    e.dormant = false;
    e.age = e.age.max(crate::rules::TELEGRAPH);
    e.id
}

fn attack(kit: &str) -> Input {
    let sword = matches!(kit, "sword" | "percival");
    Input { swing: sword, fire: !sword, aim_x: 1, ..Default::default() }
}

/// The seat's frame, and where knight 1 stands in it.
fn shot(r: &Run) -> (Img, (i32, i32)) {
    let img = frame_for(r, 768, 448, Some(1));
    let (ox, oy) = camera::view_origin(r, Some(1));
    let h = &r.players[&1];
    let (x, y) = at(h.x, h.y);
    (img, ((x - ox) * 2, (y - oy) * 2))
}

/// A `w` by `h` window of `img` about `(cx, cy)`.
fn crop(img: &Img, (cx, cy): (i32, i32), w: i32, h: i32) -> Img {
    let mut out = Img::new(w, h);
    for y in 0..h {
        for x in 0..w {
            if let Some(c) = img.get(cx - w / 2 + x, cy - h / 2 + y) {
                out.set(x, y, c);
            }
        }
    }
    out
}

fn ppm(dir: &std::path::Path, name: &str, img: &Img) {
    let mut out = format!("P6\n{} {}\n255\n", img.w, img.h).into_bytes();
    out.extend(img.rgb_bytes());
    std::fs::write(dir.join(format!("{name}.ppm")), out).unwrap();
}

/// Frames side by side in one strip.
fn strip(frames: &[Img]) -> Img {
    let (w, h) = (frames[0].w, frames[0].h);
    let mut out = Img::new(w * frames.len() as i32, h);
    for (i, f) in frames.iter().enumerate() {
        out.stamp(f, i as i32 * w, 0);
    }
    out
}

/// `frames` ticks of `r` from now, a crop about the knight (shifted by
/// `off` frame pixels) every `every` ticks, stepping as a seat sees it:
/// a hold of the room's time shows as frames that repeat.
fn film(r: &mut Run, frames: usize, every: usize, off: (i32, i32), size: (i32, i32)) -> Vec<Img> {
    let mut out = Vec::new();
    for k in 0..frames * every {
        // What a kill drops, and the notice it calls, are another
        // picture's business.
        let at = r.at;
        r.dungeon.rooms[at].items.clear();
        r.found = None;
        if k % every == 0 {
            let (img, c) = shot(r);
            out.push(crop(&img, (c.0 + off.0, c.1 + off.1), size.0, size.1));
        }
        r.step(&BTreeMap::new());
    }
    out
}

/// Frames in rows of `per`.
fn sheet(frames: &[Img], per: usize) -> Img {
    let (w, h) = (frames[0].w, frames[0].h);
    let rows = frames.len().div_ceil(per);
    let mut out = Img::new(w * per as i32, h * rows as i32);
    for (i, f) in frames.iter().enumerate() {
        out.stamp(f, (i % per) as i32 * w, (i / per) as i32 * h);
    }
    out
}

/// A far foe asleep at the room's edge, so a kill does not clear the room
/// and bring its rewards in over the picture.
fn witness(r: &mut Run) {
    foe(r, EnemyKind::Goblin, KX - 14.0, KY - 8.0, 10_000);
    r.enemies.last_mut().unwrap().sleep_until = u64::MAX;
}

/// The sword key, pressed for one tick.
fn swing(r: &mut Run) {
    r.step_sim_for_test(&BTreeMap::from([(1, Input { swing: true, ..Default::default() })]));
}

/// The sword's rearm, waited out.
fn rearm(r: &mut Run) {
    while r.players[&1].sword_cooldown > 0 || r.hitstop.frozen() {
        r.step(&BTreeMap::new());
    }
}

#[test]
#[ignore]
fn write_melee_pass_shots() {
    let Some(dir) = std::env::var_os("ANGEL_ARENA_SHOTS") else { return };
    let dir = std::path::PathBuf::from(dir);
    std::fs::create_dir_all(&dir).unwrap();
    // Each knight's arm: at rest, then the swing a tick at a time.
    for kit in ["sword", "percival", "galahad", "ironbraid", "hammerer", "butcher", "gravecaller", "knights-blade", "twin-axes", "great-maul", "cleaver", "grave-staff"] {
        let mut r = room(kit);
        foe(&mut r, EnemyKind::Goblin, KX + 3.0, KY, 10_000);
        for _ in 0..3 {
            r.step_sim_for_test(&BTreeMap::new());
        }
        let (img, c) = shot(&r);
        let mut frames = vec![crop(&img, c, 160, 120)];
        r.step_sim_for_test(&BTreeMap::from([(1, attack(kit))]));
        for _ in 0..7 {
            let (img, c) = shot(&r);
            frames.push(crop(&img, c, 160, 120));
            r.step(&BTreeMap::new());
        }
        ppm(&dir, &format!("arm-{kit}"), &strip(&frames));
    }

    // At rest and walking: the arm in hand.
    let mut rest = Vec::new();
    for kit in ["sword", "knights-blade", "twin-axes", "great-maul", "cleaver", "grave-staff", "galahad", "ironbraid"] {
        let mut r = room(kit);
        for _ in 0..3 {
            r.step_sim_for_test(&BTreeMap::new());
        }
        let (img, c) = shot(&r);
        rest.push(crop(&img, c, 96, 96));
        for _ in 0..5 {
            r.step_sim_for_test(&BTreeMap::from([(1, Input { move_x: 1, ..Default::default() })]));
        }
        let (img, c) = shot(&r);
        rest.push(crop(&img, c, 96, 96));
    }
    ppm(&dir, "a-idle-arms", &sheet(&rest, 4));

    // The slash at its widest: one goblin in the blade's reach, one only
    // the slash reaches, one past it.
    let mut r = room("sword");
    // In the blade's reach, in the slash's only, and past it.
    for (x, y) in [(1.9, -1.2), (3.2, 0.7), (5.0, 0.0)] {
        foe(&mut r, EnemyKind::Goblin, KX + x, KY + y, 10_000);
    }
    swing(&mut r);
    r.step(&BTreeMap::new());
    let struck: Vec<u32> = r.enemies.iter().map(|e| 10_000 - e.hp).collect();
    println!("slash struck {struck:?}");
    let (img, c) = shot(&r);
    ppm(&dir, "b-slash-widest-frame", &img);
    ppm(&dir, "b-slash-widest", &crop(&img, (c.0 + 40, c.1 - 10), 240, 160));

    // A head taken: a level sweep across a goblin.
    let mut r = room("sword");
    witness(&mut r);
    foe(&mut r, EnemyKind::Goblin, KX + 2.6, KY, 1);
    swing(&mut r);
    let frames = film(&mut r, 16, 2, (40, -10), (200, 120));
    println!("behead {:?}", r.gore.last().map(|g| (g.cut, g.slant)));
    ppm(&dir, "c-behead", &sheet(&frames, 4));

    // Cut in two: the backhand through a goblin's middle.
    let mut r = room("sword");
    witness(&mut r);
    swing(&mut r);
    rearm(&mut r);
    foe(&mut r, EnemyKind::Goblin, KX + 2.6, KY, 1);
    swing(&mut r);
    let frames = film(&mut r, 16, 2, (40, -10), (200, 120));
    println!("bisect {:?}", r.gore.last().map(|g| (g.cut, g.slant)));
    ppm(&dir, "d-bisect", &sheet(&frames, 4));

    // Split from above: the great maul down through a hob.
    let mut r = room("hammerer");
    witness(&mut r);
    foe(&mut r, EnemyKind::Hob, KX + 3.0, KY, 1);
    r.step_sim_for_test(&BTreeMap::from([(1, attack("hammerer"))]));
    let frames = film(&mut r, 16, 2, (40, -10), (200, 120));
    println!("split {:?}", r.gore.last().map(|g| (g.cut, g.slant)));
    ppm(&dir, "e-split", &sheet(&frames, 4));

    // A bullet turned: a goblin's shot meets the slash, flies home and
    // strikes it.
    let mut r = room("sword");
    let shooter = foe(&mut r, EnemyKind::Marksman, KX + 9.0, KY, 60);
    r.projectiles.push(crate::rules::Projectile::bullet_for_test((KX + 3.6, KY), (-9.0, 0.0), crate::rules::Shot::Orb, 12));
    let mut frames = Vec::new();
    let (img, c) = shot(&r);
    frames.push(crop(&img, (c.0 + 80, c.1 - 10), 280, 120));
    swing(&mut r);
    frames.extend(film(&mut r, 11, 2, (80, -10), (280, 120)));
    let hp = r.enemies.iter().find(|e| e.id == shooter).map_or(0, |e| e.hp);
    println!("parry: shooter hp {hp}");
    ppm(&dir, "f-parry", &sheet(&frames, 3));
}
