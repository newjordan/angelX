//! The guardians' polish, frame by frame: the Lawn Flamingo in every one
//! of its moves (standing on one leg, strutting, the wing-gust, the
//! beak-stab, the honk, the spin of its rage, a hit, and its fall), and
//! the Bard's song (a riff held through its wub to the overload, then the
//! Lullaby).
//!
//! `ANGEL_ARENA_SHOTS=<dir> cargo test write_boss_polish_shots -- --ignored`
//! writes a folder a scene, a PNG a frame: a window round the subject at
//! three times the room's size. `POLISH_ONLY` (a comma list of scene
//! prefixes) draws only those.

use super::*;
use crate::rules::{EnemyKind, Input, Phase, RoomKind, Run, TELEGRAPH, knights, signatures as fight, ults};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

/// How much bigger a window is saved.
const K: u32 = 3;

fn wanted(scene: &str) -> bool {
    std::env::var("POLISH_ONLY").map_or(true, |list| list.split(',').any(|p| scene.starts_with(p.trim())))
}

fn save(img: &Img, k: u32, path: &Path) {
    let rgba = img.rgba_scaled(k);
    image::RgbaImage::from_raw(img.w as u32 * k, img.h as u32 * k, rgba).unwrap().save(path).unwrap();
}

/// The `size` window round arena point `at`, raised `lift` pixels, as
/// knight 1's screen shows it.
fn window(run: &Run, at: (f32, f32), size: (i32, i32), lift: i32) -> Img {
    let img = frame_for(run, NATIVE_W, NATIVE_H, Some(1));
    let (cx, cy) = frame_px(run, NATIVE_W, NATIVE_H, Some(1), at.0, at.1);
    let x0 = (cx - size.0 / 2).clamp(0, (img.w - size.0).max(0));
    let y0 = (cy - lift - size.1 / 2).clamp(0, (img.h - size.1).max(0));
    Img::from_view(&img, x0, y0, size.0, size.1)
}

struct Film {
    dir: PathBuf,
}

impl Film {
    fn frame(&self, scene: &str, n: u32, img: &Img) {
        let dir = self.dir.join(scene);
        std::fs::create_dir_all(&dir).unwrap();
        save(img, K, &dir.join(format!("{n:03}.png")));
    }
}

// ─── the Lawn Flamingo ───────────────────────────────────────────────────────

/// Floor one's guardian hall, emptied, the Lawn Flamingo standing in its
/// middle and knight 1 `off` from it; and the flamingo's place among the
/// run's bosses.
fn lawn(off: (f32, f32)) -> (Run, u8, (f32, f32)) {
    let mut run = Run::new(41, 1, None);
    let hall = run.dungeon.rooms.iter().position(|r| r.kind == RoomKind::Stairs).unwrap();
    run.enter_for_test(hall);
    run.calm_for_test();
    run.enemies.clear();
    run.projectiles.clear();
    let i = run.flamingo().unwrap();
    let mid = (run.room().width() * 0.5, run.room().height() * 0.55);
    run.spawn_boss_at_for_test(i, mid.0, mid.1);
    let hero = run.players.get_mut(&1).unwrap();
    (hero.x, hero.y) = (mid.0 + off.0, mid.1 + off.1);
    hero.invulnerable = 100_000;
    run.phase = Phase::Fighting;
    (run, i as u8, mid)
}

fn the_flamingo(run: &Run, i: u8) -> Option<usize> {
    run.enemies.iter().position(|e| e.boss == Some(i))
}

/// The knight whole and out of harm's way; and, `pin`, the flamingo back
/// on `mid` with no charge-up due.
fn keep(run: &mut Run, i: u8, mid: (f32, f32), off: (f32, f32), pin: bool) {
    if let Some(h) = run.players.get_mut(&1) {
        h.hp = h.max_hp;
        h.invulnerable = 100_000;
        (h.x, h.y) = (mid.0 + off.0, mid.1 + off.1);
    }
    if let Some(k) = the_flamingo(run, i) {
        let e = &mut run.enemies[k];
        e.age = e.age.max(TELEGRAPH);
        if pin {
            (e.x, e.y) = mid;
            if e.stage == 0 {
                e.timer = 0;
            }
        }
    }
}

fn flamingo(film: &Film) {
    let size = (176, 112);
    // Every pose, on the dark, a frame a pose.
    if wanted("fl-sheet") {
        let dir = film.dir.join("fl-sheet");
        std::fs::create_dir_all(&dir).unwrap();
        save(&super::garden::flamingo_sheet(), 4, &dir.join("sheet.png"));
    }
    let idle = BTreeMap::new();
    // On one leg, a knight close by: its head bobs and now and then it
    // ruffles.
    if wanted("fl-idle") {
        let off = (-3.2, 0.6);
        let (mut run, i, mid) = lawn(off);
        for t in 0..96u32 {
            keep(&mut run, i, mid, off, true);
            run.projectiles.clear();
            if t % 2 == 0 {
                film.frame("fl-idle", t / 2, &window(&run, mid, size, 14));
            }
            run.step(&idle);
        }
    }
    // Strutting after a knight across the hall.
    if wanted("fl-strut") {
        let off = (-15.0, 0.0);
        let (mut run, i, mid) = lawn(off);
        for t in 0..72u32 {
            keep(&mut run, i, mid, off, false);
            run.projectiles.clear();
            if let Some(k) = the_flamingo(&run, i) {
                run.enemies[k].timer = 0;
                if t % 2 == 0 {
                    let (x, y) = (run.enemies[k].x, run.enemies[k].y);
                    film.frame("fl-strut", t / 2, &window(&run, (x, y), size, 14));
                }
            }
            run.step(&idle);
        }
    }
    // The wing-gust: its fan, on its beat.
    if wanted("fl-gust") {
        let off = (-9.0, 0.5);
        let (mut run, i, mid) = lawn(off);
        if let Some(k) = the_flamingo(&run, i) {
            // The fan looses when its age comes round to its beat.
            let every = run.bosses[usize::from(i)].attacks[0].every;
            let e = &mut run.enemies[k];
            e.age = (e.age / every + 2) * every - 14;
        }
        for t in 0..44u32 {
            keep(&mut run, i, mid, off, true);
            film.frame("fl-gust", t, &window(&run, (mid.0 - 3.0, mid.1), size, 14));
            run.step(&idle);
        }
    }
    // Its charge-ups, from the middle of its fight: the beak-stab (its
    // first) and the honk (its second).
    for (scene, which, off) in [("fl-stab", 0u32, (-14.0f32, 0.0f32)), ("fl-honk", 1, (-6.0, 1.0))] {
        if !wanted(scene) {
            continue;
        }
        let (mut run, i, mid) = lawn(off);
        let mid = (mid.0 + 6.0, mid.1);
        if let Some(k) = the_flamingo(&run, i) {
            let boss = run.bosses[usize::from(i)].clone();
            let e = &mut run.enemies[k];
            (e.x, e.y) = mid;
            e.hp = e.max_hp * 6 / 10;
            e.stage = 0;
            e.signature.cycle = which;
            e.timer = boss.windups[which as usize].every;
        }
        keep(&mut run, i, mid, off, false);
        let mut n = 0;
        let mut started = false;
        for _ in 0..170u32 {
            keep(&mut run, i, mid, off, false);
            run.step(&idle);
            let Some(k) = the_flamingo(&run, i) else { break };
            let stage = run.enemies[k].stage;
            if stage != 0 {
                started = true;
            }
            if started {
                let at = if scene == "fl-stab" { (run.enemies[k].x - 3.0, run.enemies[k].y) } else { mid };
                film.frame(scene, n, &window(&run, at, (224, 128), 14));
                n += 1;
            }
            if started && stage == 0 && n > 8 {
                // A breath after, to see it settle.
                if n > 80 {
                    break;
                }
            }
        }
    }
    // Its rage: the spiral, spun on one leg.
    if wanted("fl-spin") {
        let off = (-9.0, 0.5);
        let (mut run, i, mid) = lawn(off);
        if let Some(k) = the_flamingo(&run, i) {
            let e = &mut run.enemies[k];
            e.hp = e.max_hp * 3 / 10;
        }
        for t in 0..120u32 {
            keep(&mut run, i, mid, off, true);
            if t % 2 == 0 {
                film.frame("fl-spin", t / 2, &window(&run, (mid.0 - 2.0, mid.1), size, 14));
            }
            run.step(&idle);
        }
    }
    // Struck, twice.
    if wanted("fl-hit") {
        let off = (-3.4, 0.6);
        let (mut run, i, mid) = lawn(off);
        for t in 0..40u32 {
            keep(&mut run, i, mid, off, true);
            run.projectiles.clear();
            if (t == 2 || t == 22)
                && let Some(k) = the_flamingo(&run, i)
            {
                run.enemies[k].hp -= 30;
            }
            film.frame("fl-hit", t, &window(&run, mid, size, 14));
            run.step(&idle);
        }
    }
    // Felled: it topples into its feathers, lies there a lawn ornament,
    // and the hand comes for it.
    if wanted("fl-death") {
        let off = (-4.0, 1.2);
        let (mut run, i, mid) = lawn(off);
        for _ in 0..4 {
            keep(&mut run, i, mid, off, true);
            run.step(&idle);
        }
        if let Some(k) = the_flamingo(&run, i) {
            run.enemies[k].hp = 0;
        }
        run.projectiles.clear();
        let mut n = 0;
        for t in 0..330u32 {
            if let Some(h) = run.players.get_mut(&1) {
                h.hp = h.max_hp;
                (h.x, h.y) = (mid.0 + off.0, mid.1 + off.1);
            }
            run.projectiles.clear();
            if t < 60 || t % 4 == 0 {
                film.frame("fl-death", n, &window(&run, mid, size, 14));
                n += 1;
            }
            run.step(&idle);
        }
    }
}

// ─── the Bard ────────────────────────────────────────────────────────────────

/// Floor one's first fight (seed 21): the Bard at (10, 14) facing right, a
/// training dummy 4.5 units off, two skeletons either side of it.
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

fn bard(film: &Film) {
    if !wanted("bard") {
        return;
    }
    let fire = Input { fire: true, aim_x: 1, ..Default::default() };
    let mut r = stage();
    // The riff: its first beats, the wub coming in, the overload; then a
    // breath and the Lullaby.
    let windows = [("bard-a-riff", 8..40), ("bard-b-wub", 64..96), ("bard-c-overload", 150..200), ("bard-d-lullaby", 210..290)];
    for t in 0..290u32 {
        let input = match t {
            10..=200 => fire,
            212 => Input { ult: true, aim_x: 1, ..Default::default() },
            240..=290 => fire,
            _ => Input::default(),
        };
        r.step(&BTreeMap::from([(1, input)]));
        for (scene, span) in &windows {
            if span.contains(&t) {
                film.frame(scene, t - span.start, &window(&r, (12.0, 14.0), (192, 120), 8));
            }
        }
    }
}

#[test]
#[ignore]
fn write_boss_polish_shots() {
    let Some(dir) = std::env::var_os("ANGEL_ARENA_SHOTS") else {
        return;
    };
    let film = Film { dir: PathBuf::from(dir) };
    flamingo(&film);
    bard(&film);
    let _ = fight::WIND;
}
