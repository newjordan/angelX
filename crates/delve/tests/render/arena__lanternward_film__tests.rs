//! Lanternward's moments as films. The rules are a Bend module this crate's
//! tests cannot run, so a film poses the recorded moments: it moves the
//! knights, foes and effects the way the rules would and draws every tick with
//! the real stage. `ANGEL_ARENA_SHOTS=<dir> cargo test -p angel-delve --lib
//! write_lanternward_films -- --ignored` writes `<dir>/films/<name>/f0000.png`...
//! (`LW_FILM=a,b` picks some). What it shows is the picture, not the play.
use super::super::super::ink::Img;
use super::super::frame_for;
use crate::rules::{
    Run,
    lanternward::{
        self as lw, Act, Atk, Bk, Bolt, Danger, Fs, Fx, Fxk, Lock, Ws,
        world::{Q, TILE, plan},
    },
};
use std::path::PathBuf;

fn px(v: i32) -> i32 {
    v * Q
}

struct Reel {
    dir: PathBuf,
    n: u32,
}

impl Reel {
    fn new(name: &str) -> Option<Reel> {
        let root = std::env::var_os("ANGEL_ARENA_SHOTS")?;
        if let Ok(only) = std::env::var("LW_FILM")
            && !only.split(',').any(|o| o == name)
        {
            return None;
        }
        let dir = PathBuf::from(root).join("films").join(name);
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        Some(Reel { dir, n: 0 })
    }

    fn shot(&mut self, r: &Run) {
        let img = frame_for(r, 384, 224, Some(1));
        image::RgbaImage::from_raw(img.w as u32, img.h as u32, img.rgba_bytes())
            .unwrap()
            .save(self.dir.join(format!("f{:04}.png", self.n)))
            .unwrap();
        self.n += 1;
    }
}

fn s(r: &mut Run) -> &mut lw::Session {
    r.lanternward.as_mut().unwrap()
}

/// A unit vector in `Q` pixels from `(ax, ay)` to `(bx, by)`.
fn toward(ax: i32, ay: i32, bx: i32, by: i32) -> (i32, i32) {
    let (dx, dy) = (f64::from(bx - ax), f64::from(by - ay));
    let d = dx.hypot(dy).max(1.0);
    ((dx / d * f64::from(Q)).round() as i32, (dy / d * f64::from(Q)).round() as i32)
}

/// Age every counter one tick, as the rules would.
fn age(r: &mut Run) {
    r.hitstop.left = r.hitstop.left.saturating_sub(1);
    let s = s(r);
    s.elapsed += 1;
    s.stage_t += 1;
    s.shake = s.shake.saturating_sub(1);
    s.flash = s.flash.saturating_sub(1);
    s.wipe = s.wipe.saturating_sub(1);
    for f in &mut s.fx {
        f.t += 1;
    }
    s.fx.retain(|f| f.t < f.kind.life());
    for f in &mut s.foes {
        f.flinch = f.flinch.saturating_sub(1);
        if f.st == Fs::Dying {
            f.dead = (f.dead + 1).min(lw::DEATH_TICKS);
        }
        f.t = f.t.saturating_sub(1);
    }
    if let Some(w) = &mut s.boss {
        w.flinch = w.flinch.saturating_sub(1);
        if w.st == Ws::Dying {
            w.dead = (w.dead + 1).min(600);
        }
        w.t = w.t.saturating_sub(1);
    }
    for k in s.seats.values_mut() {
        k.ward = k.ward.saturating_sub(1);
        k.freeze = k.freeze.saturating_sub(1);
        if k.lock.is_some() {
            k.lock_t += 1;
        }
        if k.act != Act::Free {
            k.t += 1;
        }
        if k.shield > 0 {
            k.shield += 1;
        }
    }
}

/// The view leans toward the lock and eases to where it wants to be.
fn camera(r: &mut Run) {
    let s = s(r);
    let p = plan(usize::from(s.room));
    let (w, h) = (p.w() * TILE * Q, p.h() * TILE * Q);
    let clamp = |v: i32, half: i32, whole: i32| if whole <= 2 * half { whole / 2 } else { v.clamp(half, whole - half) };
    let ids: Vec<u32> = s.seats.keys().copied().collect();
    for id in ids {
        let k = &s.seats[&id];
        let (mut tx, mut ty) = (k.x, k.y);
        if let Some(l) = k.lock
            && let Some((lx, ly)) = s.lock_point(l)
        {
            tx += (lx - k.x) * lw::LOCK_LEAN / 100;
            ty += (ly - k.y) * lw::LOCK_LEAN / 100;
        }
        // The Warden pulls the view toward it (up to 52 px) so its whole body stays on screen.
        if let Some(b) = &s.boss
            && b.hp > 0
            && k.lock != Some(Lock::Boss)
        {
            let (dx, dy) = (f64::from(b.x - k.x), f64::from(b.y - k.y));
            let d = dx.hypot(dy).max(1.0);
            let pull = (d / 2.0).min(f64::from(px(52)));
            tx += (dx / d * pull) as i32;
            ty += (dy / d * pull) as i32;
        }
        let (tx, ty) = (clamp(tx, px(192), w), clamp(ty, px(112), h));
        let k = s.seats.get_mut(&id).unwrap();
        k.cam = (k.cam.0 + (tx - k.cam.0) / 4, k.cam.1 + (ty - k.cam.1) / 4);
    }
}

/// Draw the tick; `hold` extra ticks of hit-stop, the world frozen but for
/// the shake the realm shakes it with.
fn frame(r: &mut Run, reel: &mut Reel, hold: u32) {
    camera(r);
    reel.shot(r);
    age(r);
    for h in (0..hold).rev() {
        r.hitstop.left = h + 1;
        camera(r);
        reel.shot(r);
    }
    r.hitstop.left = 0;
}

fn fx(s: &mut lw::Session, kind: Fxk, x: i32, y: i32, a: i16) {
    s.fx.push(Fx { kind, x, y, t: 0, a });
}

fn seat_at(s: &mut lw::Session, id: u32, x: i32, y: i32, face: (i32, i32)) {
    let k = s.seats.get_mut(&id).unwrap();
    k.x = px(x);
    k.y = px(y);
    k.face = face;
}

/// The knight cuts: slash `n` starts, its blade lands on the foe a few ticks on.
fn start_cut(s: &mut lw::Session, id: u32, n: u8) {
    let k = s.seats.get_mut(&id).unwrap();
    k.act = Act::Slash;
    k.n = n;
    k.t = 0;
}

fn free(s: &mut lw::Session, id: u32) {
    let k = s.seats.get_mut(&id).unwrap();
    k.act = Act::Free;
    k.t = 0;
    k.n = 0;
    k.z = 0;
}

#[test]
#[ignore = "writes Lanternward's films"]
fn write_lanternward_films() {
    if std::env::var_os("ANGEL_ARENA_SHOTS").is_none() {
        return;
    }
    title();
    duel();
    parry_film();
    lantern();
    warden();
    finale();
}

// ----- the title -----------------------------------------------------------------

fn title() {
    let Some(mut reel) = Reel::new("title") else { return };
    let mut r = Run::lanternward_at_for_test("stair", 2);
    {
        let s = s(&mut r);
        s.elapsed = 0;
        s.stage_t = 0;
        s.foes.clear();
        seat_at(s, 1, 40, 46, (0, Q));
        seat_at(s, 2, 70, 46, (0, Q));
        for k in s.seats.values_mut() {
            k.cam = (px(176), px(112));
        }
    }
    for f in 0..130 {
        {
            let s = s(&mut r);
            // The knights wake and step down toward the sign.
            if f > 40 {
                let y = 46 + (f as i32 - 40) / 3;
                seat_at(s, 1, 40, y.min(66), (0, Q));
                seat_at(s, 2, 70, (y - 4).min(62), (0, Q));
            }
        }
        frame(&mut r, &mut reel, 0);
    }
}

// ----- lock on, hop, three cuts ----------------------------------------------------

fn duel() {
    let Some(mut reel) = Reel::new("duel") else { return };
    let mut r = Run::lanternward_at_for_test("stair", 2);
    {
        let s = s(&mut r);
        s.elapsed = 600;
        s.stage_t = 120;
        s.foes[0].id = 1;
        s.foes[0].x = px(205);
        s.foes[0].y = px(140);
        s.foes[0].hp = 6;
        s.foes[0].full = 6;
        s.foes[0].st = Fs::Drift;
        s.foes[0].t = 40;
        s.foes[0].prov = 1;
        seat_at(s, 1, 120, 150, (Q, 0));
        seat_at(s, 2, 96, 176, (Q, 0));
        for k in s.seats.values_mut() {
            k.cam = (px(176), px(140));
        }
    }
    let mut dir = (Q, 0);
    for f in 0..140 {
        let mut hold = 0;
        {
            let s = s(&mut r);
            let (fx_, fy_) = (s.foes[0].x, s.foes[0].y);
            let (kx, ky) = (s.seats[&1].x, s.seats[&1].y);
            // frame 6: seat 1 takes the lock; seat 2 locks on a moment later
            if f == 6 {
                let k = s.seats.get_mut(&1).unwrap();
                k.lock = Some(Lock::Foe(1));
                k.lock_t = 0;
                fx(s, Fxk::Tick, fx_, fy_, 0);
            }
            if f == 14 {
                let k = s.seats.get_mut(&2).unwrap();
                k.lock = Some(Lock::Foe(1));
                k.lock_t = 0;
            }
            if f >= 6 {
                dir = toward(kx, ky, fx_, fy_);
                s.seats.get_mut(&1).unwrap().face = dir;
            }
            if let Some(k) = s.seats.get_mut(&2) {
                k.face = toward(k.x, k.y, fx_, fy_);
            }
            // The snuffer drifts in, tells, lunges along a line the knight hops out of, recovers.
            match f {
                0..=22 => {
                    let (dx, dy) = toward(fx_, fy_, kx, ky);
                    s.foes[0].x += dx * 3 / 4;
                    s.foes[0].y += dy * 3 / 4;
                    s.foes[0].fx = dx;
                    s.foes[0].fy = dy;
                    // the knight circles
                    let k = s.seats.get_mut(&1).unwrap();
                    k.y -= px(1) / 2;
                    k.x += px(1) / 3;
                }
                23 => {
                    let (dx, dy) = toward(fx_, fy_, kx, ky);
                    let f0 = &mut s.foes[0];
                    f0.st = Fs::Tell;
                    f0.t = lw::SNUFFER_TELL;
                    f0.fx = dx;
                    f0.fy = dy;
                    f0.danger = Danger::Capsule { ax: fx_, ay: fy_, bx: fx_ + dx * 52 / 1, by: fy_ + dy * 52 / 1, r: px(7) };
                }
                47 => {
                    s.foes[0].st = Fs::Strike;
                    s.foes[0].t = lw::LUNGE_TICKS;
                }
                55 => {
                    s.foes[0].st = Fs::Recover;
                    s.foes[0].t = 22;
                    s.foes[0].danger = Danger::None;
                }
                _ => {}
            }
            if (47..55).contains(&f) {
                let (fx2, fy2) = (s.foes[0].fx, s.foes[0].fy);
                s.foes[0].x += fx2 * 22 / 4;
                s.foes[0].y += fy2 * 22 / 4;
            }
            // frame 38: the knight hops out of the line
            if f == 40 {
                let k = s.seats.get_mut(&1).unwrap();
                k.act = Act::Hop;
                k.n = 0;
                k.t = 0;
            }
            if let Some(k) = s.seats.get_mut(&1)
                && k.act == Act::Hop
            {
                let t = k.t as f32;
                k.z = (px(lw::HOP_HEIGHT) as f32 * (t / lw::HOP_TICKS as f32 * std::f32::consts::PI).sin()) as i32;
                k.x += px(2) / 1 * 0;
                k.y += px(2);
                if k.t >= lw::HOP_TICKS {
                    k.act = Act::Free;
                    k.z = 0;
                    k.t = 0;
                }
            }
            // the cuts, one by one: the knight steps in behind each
            let reach = 21;
            let step_in = |s: &mut lw::Session| {
                let (fx2, fy2) = (s.foes[0].x, s.foes[0].y);
                let k = s.seats.get_mut(&1).unwrap();
                let d = toward(k.x, k.y, fx2, fy2);
                let dist = (((fx2 - k.x) as f64).hypot((fy2 - k.y) as f64) / Q as f64) as i32;
                if dist > reach {
                    k.x += d.0 * (dist - reach).min(6);
                    k.y += d.1 * (dist - reach).min(6);
                }
                k.face = d;
            };
            if f >= 58 && f < 100 {
                step_in(s);
            }
            let hit = |s: &mut lw::Session, dmg: u32, hold: &mut u32, heavy: bool| {
                let (x2, y2) = (s.foes[0].x, s.foes[0].y);
                let f0 = &mut s.foes[0];
                f0.hp = f0.hp.saturating_sub(dmg);
                f0.flinch = lw::FLINCH_TICKS;
                f0.open = false;
                let (kx2, ky2) = (s.seats[&1].x, s.seats[&1].y);
                let d = toward(kx2, ky2, x2, y2);
                f0.kb = (d.0 * 2, d.1 * 2);
                f0.kbt = 3;
                let k = s.seats.get_mut(&1).unwrap();
                k.freeze = if heavy { 4 } else { 2 };
                fx(s, Fxk::Spark, x2, y2, 0);
                if heavy {
                    s.shake = 12;
                    *hold = 3;
                }
            };
            if f == 58 { start_cut(s, 1, 0); }
            if f == 61 { hit(s, 2, &mut hold, false); }
            if f == 64 { free(s, 1); }
            if f == 68 { start_cut(s, 1, 1); }
            if f == 71 { hit(s, 2, &mut hold, false); }
            if f == 76 { free(s, 1); }
            if f == 80 { start_cut(s, 1, 2); }
            if f == 85 {
                let (x2, y2) = (s.foes[0].x, s.foes[0].y);
                hit(s, 2, &mut hold, true);
                s.foes[0].hp = 0;
                s.foes[0].st = Fs::Dying;
                s.foes[0].dead = 0;
                fx(s, Fxk::Puff, x2, y2, 0);
                fx(s, Fxk::Light, x2, y2, 0);
                hold = 3;
            }
            if f == 92 { free(s, 1); }
            // a foe pushed back moves while the push lasts
            let f0 = &mut s.foes[0];
            if f0.kbt > 0 {
                f0.x += f0.kb.0;
                f0.y += f0.kb.1;
                f0.kbt -= 1;
            }
            if f0.st == Fs::Recover && f0.t == 0 {
                f0.st = Fs::Drift;
                f0.t = 20;
            }
        }
        frame(&mut r, &mut reel, hold);
    }
}

// ----- a parry --------------------------------------------------------------------

fn parry_film() {
    let Some(mut reel) = Reel::new("parry") else { return };
    let mut r = Run::lanternward_at_for_test("stair", 4);
    {
        let s = s(&mut r);
        s.elapsed = 700;
        s.stage_t = 200;
        s.foes[0].id = 1;
        s.foes[0].x = px(184);
        s.foes[0].y = px(140);
        s.foes[0].hp = 18;
        s.foes[0].full = 18;
        s.foes[0].st = Fs::Drift;
        s.foes[0].prov = 3;
        seat_at(s, 1, 140, 140, (Q, 0));
        seat_at(s, 2, 120, 168, (Q, 0));
        seat_at(s, 3, 118, 112, (Q, 0));
        let k = s.seats.get_mut(&1).unwrap();
        k.lock = Some(Lock::Foe(1));
        k.lock_t = 12;
        for k in s.seats.values_mut() {
            k.cam = (px(176), px(140));
        }
    }
    for f in 0..120 {
        let mut hold = 0;
        {
            let s = s(&mut r);
            let (fx_, fy_) = (s.foes[0].x, s.foes[0].y);
            for id in [1, 2, 3] {
                let k = s.seats.get_mut(&id).unwrap();
                k.face = toward(k.x, k.y, fx_, fy_);
            }
            if f < 4 {
                // the snuffer reaches striking distance
            }
            if f == 4 {
                let d = toward(fx_, fy_, s.seats[&1].x, s.seats[&1].y);
                let f0 = &mut s.foes[0];
                f0.st = Fs::Tell;
                f0.t = lw::SNUFFER_TELL;
                f0.fx = d.0;
                f0.fy = d.1;
                f0.danger = Danger::Capsule { ax: fx_, ay: fy_, bx: fx_ + d.0 * 40, by: fy_ + d.1 * 40, r: px(7) };
            }
            // the shield goes up three ticks before the blow
            if f == 21 {
                s.seats.get_mut(&1).unwrap().shield = 1;
            }
            if f == 28 {
                s.foes[0].st = Fs::Strike;
                s.foes[0].t = lw::LUNGE_TICKS;
            }
            if (28..31).contains(&f) {
                let (dx, dy) = (s.foes[0].fx, s.foes[0].fy);
                s.foes[0].x += dx * 9;
                s.foes[0].y += dy * 9;
            }
            if f == 31 {
                // the blow meets the shield inside the parry window: a ring, a flash, a hold
                let (kx, ky) = (s.seats[&1].x, s.seats[&1].y);
                let d = toward(kx, ky, s.foes[0].x, s.foes[0].y);
                fx(s, Fxk::Parry, kx + d.0 * 10, ky + d.1 * 10 - px(2), 0);
                fx(s, Fxk::Ring, kx + d.0 * 9, ky + d.1 * 9, 0);
                s.flash = 6;
                s.shake = 8;
                let f0 = &mut s.foes[0];
                f0.st = Fs::Stagger;
                f0.t = lw::STAGGER_TICKS;
                f0.open = true;
                f0.danger = Danger::None;
                f0.flinch = 5;
                s.seats.get_mut(&1).unwrap().freeze = 4;
                hold = 5;
            }
            if f == 38 {
                s.seats.get_mut(&1).unwrap().shield = 0;
            }
            // the friends step in on the stunned foe, each to a side of their own
            if f >= 40 {
                for (id, off) in [(1u32, (-21, 0)), (2, (-15, 17)), (3, (-14, -17))] {
                    let (fx2, fy2) = (s.foes[0].x, s.foes[0].y);
                    let k = s.seats.get_mut(&id).unwrap();
                    let (tx, ty) = (fx2 + px(off.0), fy2 + px(off.1));
                    let d = toward(k.x, k.y, tx, ty);
                    let dist = (((tx - k.x) as f64).hypot((ty - k.y) as f64) / Q as f64) as i32;
                    if dist > 2 {
                        k.x += d.0 * dist.min(3);
                        k.y += d.1 * dist.min(3);
                    }
                }
            }
            let blade = |s: &mut lw::Session, id: u32, n: u8| {
                start_cut(s, id, n);
            };
            if f == 52 { blade(s, 1, 0); }
            if f == 55 {
                // a parried foe takes a double blow
                let (x2, y2) = (s.foes[0].x, s.foes[0].y);
                let f0 = &mut s.foes[0];
                f0.hp -= 8;
                f0.flinch = lw::FLINCH_TICKS;
                f0.open = false;
                fx(s, Fxk::Spark, x2, y2, 0);
                s.seats.get_mut(&1).unwrap().freeze = 3;
                s.shake = 8;
                hold = 2;
            }
            if f == 59 { free(s, 1); }
            if f == 64 { start_cut(s, 2, 0); }
            if f == 67 {
                let (x2, y2) = (s.foes[0].x, s.foes[0].y);
                let f0 = &mut s.foes[0];
                f0.hp = f0.hp.saturating_sub(4);
                f0.flinch = lw::FLINCH_TICKS;
                fx(s, Fxk::Spark, x2, y2, 0);
                s.seats.get_mut(&2).unwrap().freeze = 2;
            }
            if f == 71 { free(s, 2); }
            if f == 76 { start_cut(s, 3, 2); }
            if f == 80 {
                let (x2, y2) = (s.foes[0].x, s.foes[0].y);
                let f0 = &mut s.foes[0];
                f0.hp = 0;
                f0.st = Fs::Dying;
                f0.dead = 0;
                f0.flinch = lw::FLINCH_TICKS;
                fx(s, Fxk::Puff, x2, y2, 0);
                fx(s, Fxk::Light, x2, y2, 0);
                s.shake = 12;
                s.seats.get_mut(&3).unwrap().freeze = 4;
                hold = 3;
            }
            if f == 86 { free(s, 3); }
        }
        frame(&mut r, &mut reel, hold);
    }
}

// ----- the lantern's light -----------------------------------------------------------

fn lantern() {
    // The Lamp Hall at rest: braziers and the ember breathe, a flare swells.
    if let Some(mut reel) = Reel::new("lamp") {
        let mut r = Run::lanternward_at_for_test("lamp-wave", 2);
        {
            let s = s(&mut r);
            s.stage_t = 200;
            s.foes.clear();
            seat_at(s, 1, 150, 118, (Q, 0));
            seat_at(s, 2, 118, 130, (Q, 0));
            for p in s.props.iter_mut().filter(|p| p.kind == lw::Pk::Brazier) {
                p.on = 1;
            }
            for k in s.seats.values_mut() {
                k.cam = (px(150), px(112));
            }
        }
        for f in 0..110 {
            {
                let s = s(&mut r);
                if f > 20 {
                    let k = s.seats.get_mut(&1).unwrap();
                    k.lamp = (f - 20).min(255);
                    k.flame = 120;
                    k.x += px(1) / 2;
                }
                if f == 50 {
                    s.seats.get_mut(&2).unwrap().lamp = 0;
                }
                if f > 50 {
                    let k = s.seats.get_mut(&2).unwrap();
                    k.lamp = (f - 50).min(255);
                    k.flame = 120;
                }
            }
            frame(&mut r, &mut reel, 0);
        }
    }
    // The Vault: dark, one flare burns a snuffer's hood off, the blade then counts.
    if let Some(mut reel) = Reel::new("vault") {
        let mut r = Run::lanternward_at_for_test("vault-flare", 1);
        {
            let s = s(&mut r);
            s.stage_t = 200;
            s.foes[0].hooded = true;
            s.foes[0].lit = 0;
            s.foes[0].st = Fs::Dormant;
            s.foes[0].x = px(190);
            s.foes[0].y = px(130);
            s.foes[1].x = px(300);
            s.foes[1].y = px(90);
            seat_at(s, 1, 140, 150, (Q, 0));
            for k in s.seats.values_mut() {
                k.cam = (px(190), px(140));
            }
        }
        for f in 0..120 {
            {
                let s = s(&mut r);
                s.seats.get_mut(&1).unwrap().face = toward(s.seats[&1].x, s.seats[&1].y, s.foes[0].x, s.foes[0].y);
                if f > 12 {
                    let k = s.seats.get_mut(&1).unwrap();
                    k.lamp = (f - 12).min(255);
                    k.flame = 100;
                }
                if f > 40 && f < 100 {
                    s.foes[0].lit = 60;
                }
                if f == 40 {
                    let (x2, y2) = (s.foes[0].x, s.foes[0].y);
                    fx(s, Fxk::Light, x2, y2, 0);
                    s.foes[0].flinch = 6;
                }
                if f == 60 {
                    s.foes[0].st = Fs::Drift;
                }
            }
            frame(&mut r, &mut reel, 0);
        }
    }
}

// ----- the Warden -----------------------------------------------------------------------

fn warden() {
    let Some(mut reel) = Reel::new("warden") else { return };
    let mut r = Run::lanternward_at_for_test("warden-open", 2);
    {
        let s = s(&mut r);
        s.stage_t = 20;
        s.elapsed = 600;
        let w = s.boss.as_mut().unwrap();
        w.st = Ws::Rest;
        w.t = 70;
        w.n = 0;
        w.hp = w.full;
        w.phase = 0;
        w.danger = Danger::None;
        w.lit = [0; 6];
        w.face = (0, Q);
        seat_at(s, 1, 214, 248, (0, -Q));
        seat_at(s, 2, 182, 258, (0, -Q));
        for k in s.seats.values_mut() {
            k.cam = (px(230), px(170));
        }
    }
    let boss_xy = |s: &lw::Session| {
        let w = s.boss.as_ref().unwrap();
        (w.x, w.y)
    };
    for f in 0..325 {
        let mut hold = 0;
        {
            let s = s(&mut r);
            let (bx, by) = boss_xy(s);
            let elapsed = s.elapsed;
            if f == 30 {
                // the Warden wakes, takes the lock
                s.seats.get_mut(&1).unwrap().lock = Some(Lock::Boss);
                s.seats.get_mut(&1).unwrap().lock_t = 0;
            }
            for id in [1, 2] {
                let k = s.seats.get_mut(&id).unwrap();
                k.face = toward(k.x, k.y, bx, by);
            }
            let full = s.boss.as_ref().unwrap().full;
            let w = s.boss.as_mut().unwrap();
            match f {
                30 => {
                    // a short rest, then a beam
                    w.t = 30;
                }
                60 => {
                    let d = toward(w.x, w.y, px(214), px(248));
                    w.face = d;
                    w.st = Ws::Tell;
                    w.atk = Atk::Beam;
                    w.t = 53;
                    w.aim = 114;
                    w.danger = Danger::Fan { x: w.x, y: w.y, deg: 100, half: 56, r: px(150) };
                }
                113 => {
                    w.st = Ws::Strike;
                    w.t = lw::BEAM_TICKS;
                }
                _ => {}
            }
            // the shield goes up late in the tell; the beam meets it inside the window
            if f == 108 {
                s.seats.get_mut(&1).unwrap().shield = 1;
            }
            if f == 119 {
                // parried: the beam turns back, the shutter flies open
                let (kx, ky) = (s.seats[&1].x, s.seats[&1].y);
                fx(s, Fxk::Parry, kx, ky - px(14), 0);
                fx(s, Fxk::Ring, kx, ky - px(8), 0);
                s.flash = 6;
                s.shake = 10;
                let w = s.boss.as_mut().unwrap();
                w.st = Ws::Stagger;
                w.t = 75;
                w.danger = Danger::None;
                w.lit = [elapsed, elapsed, 0, 0, 0, 0];
                s.seats.get_mut(&1).unwrap().freeze = 4;
                hold = 5;
            }
            if f == 130 {
                s.seats.get_mut(&1).unwrap().shield = 0;
            }
            // the knights charge in on the stunned Warden and cut its heart
            if (126..190).contains(&f) {
                for id in [1u32, 2] {
                    let k = s.seats.get_mut(&id).unwrap();
                    let d = toward(k.x, k.y, bx, by + px(8));
                    let dist = (((bx - k.x) as f64).hypot((by + px(8) - k.y) as f64) / Q as f64) as i32;
                    if dist > 40 {
                        k.x += d.0 * 4;
                        k.y += d.1 * 4;
                    }
                    if id == 1 {
                        k.face = d;
                    }
                }
            }
            let cut = |s: &mut lw::Session, id: u32, n: u8| start_cut(s, id, n);
            let strike = |s: &mut lw::Session, id: u32, hp: u32, heavy: bool, hold: &mut u32| {
                let (bx2, by2) = (s.boss.as_ref().unwrap().x, s.boss.as_ref().unwrap().y);
                let w = s.boss.as_mut().unwrap();
                w.hp = w.hp.saturating_sub(hp);
                w.flinch = lw::FLINCH_TICKS;
                fx(s, Fxk::Spark, bx2, by2 + px(2), 0);
                s.seats.get_mut(&id).unwrap().freeze = if heavy { 4 } else { 2 };
                if heavy {
                    s.shake = 12;
                    *hold = 3;
                }
            };
            if f == 150 { cut(s, 1, 0); }
            if f == 153 { strike(s, 1, 8, false, &mut hold); }
            if f == 157 { free(s, 1); }
            if f == 160 { cut(s, 2, 0); }
            if f == 163 { strike(s, 2, 8, false, &mut hold); }
            if f == 167 { free(s, 2); }
            if f == 168 { cut(s, 1, 1); }
            if f == 171 { strike(s, 1, 8, false, &mut hold); }
            if f == 176 { free(s, 1); }
            if f == 178 { cut(s, 1, 2); }
            if f == 182 { strike(s, 1, 16, true, &mut hold); }
            if f == 190 { free(s, 1); }
            if f == 195 {
                let w = s.boss.as_mut().unwrap();
                w.st = Ws::Recover;
                w.t = 20;
                w.lit = [0; 6];
            }
            // a friend's thrown light finds the shutter: the heart opens
            if f == 200 {
                let (hx, hy) = s.boss.as_ref().unwrap().heart();
                fx(s, Fxk::Light, hx, hy, 0);
                let w = s.boss.as_mut().unwrap();
                w.lit = [0, elapsed, 0, 0, 0, 0];
            }
            if f == 206 { cut(s, 2, 0); }
            if f == 209 { strike(s, 2, 8, false, &mut hold); }
            if f == 214 { free(s, 2); }
            // a slam: the fists rise, the footprint shows, the ring runs out, the knights hop it
            if f == 222 {
                let w = s.boss.as_mut().unwrap();
                w.st = Ws::Tell;
                w.atk = Atk::Slam;
                w.t = 53;
                w.danger = Danger::Circle { x: w.x, y: w.y, r: px(lw::SLAM_REACH) };
                w.phase = 1;
            }
            if f == 275 {
                let w = s.boss.as_mut().unwrap();
                w.st = Ws::Strike;
                w.t = lw::SLAM_TICKS;
                fx(s, Fxk::Wave, bx, by + px(20), 0);
                s.shake = 12;
                hold = 3;
            }
            if f == 283 {
                for id in [1u32, 2] {
                    let k = s.seats.get_mut(&id).unwrap();
                    k.act = Act::Hop;
                    k.n = 2;
                    k.t = 0;
                }
            }
            for id in [1u32, 2] {
                let k = s.seats.get_mut(&id).unwrap();
                if k.act == Act::Hop {
                    let t = k.t as f32;
                    k.z = (px(lw::HOP_HEIGHT) as f32 * (t / lw::HOP_TICKS as f32 * std::f32::consts::PI).sin()) as i32;
                    if k.t >= lw::HOP_TICKS {
                        k.act = Act::Free;
                        k.z = 0;
                        k.t = 0;
                    }
                }
            }
            if f == 297 {
                let w = s.boss.as_mut().unwrap();
                w.st = Ws::Recover;
                w.danger = Danger::None;
                w.t = 30;
            }
            let _ = (elapsed, full);
        }
        frame(&mut r, &mut reel, hold);
    }
    let _ = (Bk::Iron, Bolt { kind: Bk::Iron, x: 0, y: 0, vx: 0, vy: 0, ttl: 0, owner: 0 });
}

// ----- the Beacon ------------------------------------------------------------------------

fn finale() {
    // The Warden falls; the Beacon is lit; each knight takes a purse home.
    let Some(mut reel) = Reel::new("finale") else { return };
    let mut r = Run::lanternward_at_for_test("warden-dead", 1);
    {
        let s = s(&mut r);
        let w = s.boss.as_mut().unwrap();
        w.st = Ws::Dying;
        w.dead = 0;
        w.hp = 0;
        w.danger = Danger::None;
        s.elapsed = 5200;
        seat_at(s, 1, 225, 165, (0, -Q));
        for k in s.seats.values_mut() {
            k.cam = (px(235), px(140));
        }
    }
    for f in 0..90 {
        frame(&mut r, &mut reel, if f == 0 { 4 } else { 0 });
    }
    // the Beacon: the finale moment, three knights
    let mut r = Run::lanternward_at_for_test("finale", 2);
    {
        let s = s(&mut r);
        let done = s.done.unwrap_or(600);
        s.elapsed = done;
        s.done = Some(done);
    }
    for _ in 0..260 {
        frame(&mut r, &mut reel, 0);
    }
}

// ----- the words never meet -----------------------------------------------------------

type Rect = (i32, i32, i32, i32);

fn meets(a: Rect, b: Rect) -> bool {
    a.0 < b.0 + b.2 && b.0 < a.0 + a.2 && a.1 < b.1 + b.3 && b.1 < a.1 + a.3
}

/// Every box of words drawn for seat `id`'s screen.
fn boxes(r: &Run, id: u32) -> Vec<(&'static str, Rect)> {
    super::hud::CLAIMS.with(|c| c.borrow_mut().clear());
    let _ = super::layers(r, Some(id));
    super::hud::CLAIMS.with(|c| c.borrow().clone())
}

fn assert_clear(r: &Run, id: u32, what: &str) {
    let all = boxes(r, id);
    for (name, b) in &all {
        assert!(b.0 >= 0 && b.1 >= 0 && b.0 + b.2 <= 384 && b.1 + b.3 <= 224, "{what}: {name} {b:?} leaves 384x224");
    }
    for (i, (an, a)) in all.iter().enumerate() {
        for (bn, b) in &all[i + 1..] {
            assert!(!meets(*a, *b), "{what}: {an} {a:?} meets {bn} {b:?}");
        }
    }
}

#[test]
fn no_two_boxes_of_words_meet_on_any_moment() {
    for &(m, seats) in super::tests::MOMENTS {
        let r = Run::lanternward_at_for_test(m, seats);
        for id in 1..=seats {
            assert_clear(&r, id, &format!("{m}/{seats} seat {id}"));
        }
    }
}

/// The busiest screens there are: a card over every sort of hint, the Warden's
/// bar under a held-up item, a sign being read, a six-seat finale.
#[test]
fn no_two_boxes_of_words_meet_on_the_busiest_screens() {
    use lw::{Banner, Ctx, Item};
    for (m, seats) in [("warden-open", 4), ("warden-dark", 4), ("gallery-tell", 1), ("vault-flare", 1), ("stair", 4), ("lamp-wave", 6), ("finale", 2)] {
        for age in [0, 4, 12, 30, 44, 52, 80, 130] {
            for banner in [None, Some(Item::Lantern), Some(Item::Container), Some(Item::BossKey)] {
                let mut r = Run::lanternward_at_for_test(m, seats);
                {
                    let s = s(&mut r);
                    s.stage_t = age;
                    s.banner = banner.map(|item| Banner { item, seat: 2, t: (60 - age.min(55)) as u16 });
                    for k in s.seats.values_mut() {
                        k.read = Some((100, 60));
                        k.ctx = Ctx::Throw;
                    }
                }
                for id in 1..=seats.min(2) {
                    assert_clear(&r, id, &format!("{m}/{seats} stage {age} {banner:?} seat {id}"));
                }
            }
        }
    }
    // The Beacon's finale, a full table, early and late.
    for since in [0, 20, 36, 50, 90, 140, 200, 400] {
        let mut r = Run::lanternward_at_for_test("lamp-wave", 6);
        {
            let s = s(&mut r);
            s.done = Some(100);
            s.elapsed = 100 + since;
            s.foes.clear();
            s.stage_t = 400;
        }
        for id in 1..=2 {
            assert_clear(&r, id, &format!("finale/6 {since} seat {id}"));
        }
    }
    // A parry's word among the cards and the bars, at every age.
    for (m, seats) in [("warden-open", 2), ("parry", 1), ("stair", 4)] {
        for age in 0..22u8 {
            for item in [None, Some(Item::Lantern)] {
                let mut r = Run::lanternward_at_for_test(m, seats);
                {
                    let s = s(&mut r);
                    s.stage_t = 20;
                    s.banner = item.map(|item| Banner { item, seat: 1, t: 30 });
                    let (x, y) = (s.seats[&1].x, s.seats[&1].y);
                    s.fx.push(Fx { kind: Fxk::Parry, x, y, t: age, a: 0 });
                    if let Some(w) = s.boss.as_mut() {
                        w.st = Ws::Stagger;
                        w.t = 70;
                    }
                }
                assert_clear(&r, 1, &format!("{m}/{seats} parry age {age} {item:?}"));
            }
        }
    }
    // The retry and the Warden waking and falling.
    for (name, tweak) in [("wipe", 0), ("woke", 1), ("falls", 2)] {
        let mut r = Run::lanternward_at_for_test("warden", 2);
        {
            let s = s(&mut r);
            match tweak {
                0 => s.wipe = 12,
                1 => {
                    let w = s.boss.as_mut().unwrap();
                    w.st = Ws::Rest;
                    w.t = 60;
                    w.hp = w.full;
                    w.n = 0;
                }
                _ => {
                    let w = s.boss.as_mut().unwrap();
                    w.st = Ws::Dying;
                    w.hp = 0;
                    w.dead = 40;
                }
            }
        }
        assert_clear(&r, 1, name);
    }
}

#[test]
fn a_frame_is_the_same_frame_every_time_it_is_drawn() {
    for &(m, seats) in &[("slash3", 1), ("parry", 1), ("warden-open", 2), ("finale", 2), ("vault-flare", 1)] {
        let r = Run::lanternward_at_for_test(m, seats);
        let a = super::layers(&r, Some(1)).0;
        let b = super::layers(&r, Some(1)).0;
        assert!(a == b, "{m} draws differently twice");
    }
}

/// Every art frame is the slot's size and made of the realm's inks (a frame
/// is inked once; this keeps the generated data honest).
#[test]
fn every_painted_slot_is_whole() {
    for slot in ["warden", "snuffer", "turret", "pot", "grass", "sign", "folk", "lever", "plate", "block", "cracked", "sigil", "lock", "boss-door"] {
        let states: Vec<&str> = match slot {
            "warden" => vec!["sleep", "idle", "walk", "beam-tell", "beam", "slam-tell", "slam", "summon", "hit", "rear", "stagger", "die", "heart-shut", "heart-open"],
            "snuffer" => vec!["idle", "drift", "tell", "lunge", "recover", "hit", "stagger", "die", "hooded"],
            "turret" => vec!["idle", "tell", "fire", "recover", "stun"],
            "pot" => vec!["whole", "broken"],
            "grass" => vec!["tuft", "cut"],
            "sign" => vec!["idle"],
            "folk" => vec!["idle", "talk"],
            "lever" => vec!["up", "pulled"],
            "plate" => vec!["up", "down"],
            "block" => vec!["idle"],
            "cracked" => vec!["intact", "blown"],
            "sigil" => vec!["hidden", "revealing", "found"],
            "lock" => vec!["locked", "open"],
            _ => vec!["chained", "open"],
        };
        for state in states {
            assert!(super::bank::painted(slot, state), "{slot}.{state} is still a labelled block");
        }
    }
}
