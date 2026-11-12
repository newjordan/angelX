//! Who stands on the rampart: the pennons and the captain with their
//! tells, the bolts, the knight, and the other knights as ghosts running the
//! same wall in their own lanes.

use super::super::super::ink::{Img, bayer, ink};
use super::super::juice;
use super::super::rafterwake::{attack_stamp, danger_colour};
use super::hud::OY;
use super::{Art, art, foe_pose, knight};
use crate::rules::Run;
use crate::rules::pennon::{Area, Beat, FLOOR, Foe, Lane, Q, Session};

/// What a draw knows: the knight looked through, the camera and the clock.
pub(super) struct Cx<'a> {
    pub run: &'a Run,
    pub s: &'a Session,
    pub d: &'a Lane,
    pub id: u32,
    pub cam: i32,
    /// The siege's own tick (the show's).
    pub t: u32,
    /// The run's tick, for the figures' own cycles.
    pub tick: u64,
}

impl Cx<'_> {
    pub fn x(&self, wx: i32) -> i32 {
        wx - self.cam
    }
    pub fn y(&self, wy: i32) -> i32 {
        wy + OY
    }
}

/// The sprite a foe stands in.
pub(super) fn foe_img<'a>(a: &'a Art, f: &Foe, tick: u64) -> &'a Img {
    let pose = foe_pose(f, tick);
    let prefix = if f.captain { "captain" } else { "sentry" };
    let mut key = format!(
        "{prefix}-{}",
        [
            "idle", "walk-1", "walk-2", "drop-tell", "drop", "hook-tell", "hook", "flinch", "damage-1", "damage-2",
            "defeat", "recover"
        ][pose]
    );
    if f.captain && f.beat == Beat::Tell && f.hp > 0 && f.flinch == 0 {
        key = format!("captain-{}-tell-{}", if f.hook { "hook" } else { "drop" }, f.tell_stage());
    }
    if f.captain && f.stage() > 0 && !matches!(pose, 8..=10) {
        key = format!("{key}-stage-{}", f.stage());
    }
    if !f.captain && f.hp > 0 && f.stage() > 0 {
        let state = if f.flinch > 0 {
            "flinch"
        } else {
            match f.beat {
                Beat::Walk => {
                    if f.stage() == 2 {
                        if tick / 7 % 2 == 0 { "limp-1" } else { "limp-2" }
                    } else if tick / 7 % 2 == 0 {
                        "walk-1"
                    } else {
                        "walk-2"
                    }
                }
                Beat::Tell => "tell",
                Beat::Attack => "attack",
                Beat::Rest if f.ticks < 12 => "recover",
                _ => {
                    if f.stage() == 2 { "last-stand" } else { "idle" }
                }
            }
        };
        let damaged = format!("sentry-damaged-{state}");
        if a.all.contains_key(&damaged) {
            key = damaged;
        }
    }
    &a.all[&key].body
}

/// A soft dark ellipse on the walkway under a body `rx` wide.
pub(super) fn shadow(world: &mut Img, cx: i32, rx: i32) {
    let y = FLOOR + OY + 2;
    for dy in 0..3 {
        let half = rx * (3 - dy) / 3;
        for dx in -half..=half {
            if bayer(cx + dx, y + dy) < 0.75 - dy as f32 * 0.2 {
                world.put(cx + dx, y + dy, 'k');
            }
        }
    }
}

/// The hazard of the captain's blow: a taped-off box that blinks faster.
fn danger(im: &mut Img, z: Area, cx: &Cx, stage: usize) {
    let (on, period) = match stage {
        0 => (1, 5),
        1 | 2 => (2, 5),
        _ => (3, 4),
    };
    let colour = danger_colour(if stage < 2 { 2 } else { 3 });
    let (x, y) = (cx.x(z.x), cx.y(z.y));
    for xx in 0..z.w {
        if xx / 4 % period < on {
            for dy in 0..3 {
                im.set(x + xx, y + z.h - 3 + dy, colour);
            }
            im.set(x + xx, y, colour);
        }
    }
    for yy in 0..z.h {
        if yy / 4 % period < on {
            for dx in 0..2 {
                im.set(x + dx, y + yy, colour);
                im.set(x + z.w - 2 + dx, y + yy, colour);
            }
        }
    }
}

/// A sentry's aim: dashes that fill in as the tell runs, ending in an arrow
/// at the height the bolt will keep.
fn aim_line(im: &mut Img, from: (i32, i32), to: (i32, i32), stage: usize) {
    let (dx, dy) = (to.0 - from.0, to.1 - from.1);
    let length = dx.abs().max(dy.abs()).max(1);
    for n in 0..=length {
        if n / 5 % 4 <= stage as i32 {
            let (x, y) = (from.0 + dx * n / length, from.1 + dy * n / length);
            im.set(x, y, danger_colour(3));
            im.set(x, y + 1, danger_colour(2));
        }
    }
    im.line(to.0 - 4, to.1 - 4, to.0, to.1, '2');
    im.line(to.0, to.1, to.0 - 4, to.1 + 4, '2');
}

/// A pennon's bar of three pips over its head once it has been struck.
fn pips(world: &mut Img, f: &Foe, x: i32, top: i32) {
    if f.captain || f.hp == 0 || f.hp == 3 {
        return;
    }
    for k in 0..3 {
        let on = k < i32::from(f.hp);
        world.rect(x - 7 + k * 5, top, 4, 3, 'k');
        world.rect(x - 6 + k * 5, top + 1, 2, 1, if on { 'O' } else { 's' });
    }
}

/// The wards lying on the walk, bobbing, until they are taken.
fn wards(world: &mut Img, cx: &Cx, a: &Art) {
    for (n, x) in [340, 848].into_iter().enumerate() {
        if cx.d.pickups[n] || !(-30..420).contains(&cx.x(x)) {
            continue;
        }
        let bob = (((cx.tick as f32) * 0.12 + n as f32).sin() * 2.0).round() as i32;
        let (sx, sy) = (cx.x(x), cx.y(FLOOR));
        // A warm ring on the stone beneath, breathing.
        let r = 11 + ((cx.tick / 6 + n as u64) % 3) as i32;
        for dx in -r..=r {
            if bayer(sx + dx, sy) < 0.55 && dx.abs() > r - 3 {
                world.put(sx + dx, sy + 1, 'o');
            }
        }
        world.stamp(&a.all["repair"].body, sx - 12, sy - 26 + bob);
        let tw = ((cx.tick / 5 + n as u64 * 3) % 8) as i32;
        let (gx, gy) = [(-8, -22), (9, -16), (-4, -10), (7, -26), (0, -20), (-9, -14), (6, -6), (-3, -24)][tw as usize];
        world.put(sx + gx, sy + gy + bob, '9');
        world.put(sx + gx - 1, sy + gy + bob, 'T');
        world.put(sx + gx + 1, sy + gy + bob, 'T');
        world.put(sx + gx, sy + gy + bob - 1, 'T');
        world.put(sx + gx, sy + gy + bob + 1, 'T');
    }
}

/// A pennant on the walk, its flag waving down the wind.
fn poles(world: &mut Img, cx: &Cx, a: &Art) {
    let flag = &a.all["pennant"].body;
    for (i, &x) in super::scene::POLES.iter().enumerate() {
        let sx = cx.x(x);
        if !(-40..430).contains(&sx) {
            continue;
        }
        let top = cx.y(FLOOR) - flag.h + 1;
        for xx in 0..flag.w {
            // The pole stays; the cloth ripples from its hoist.
            let k = (xx as f32 - 6.0).max(0.0) / 14.0;
            let ripple = ((cx.t as f32 * 0.22 + xx as f32 * 0.5 + i as f32 * 2.0).sin() * 1.6 * k).round() as i32;
            for yy in 0..flag.h {
                if let Some(c) = flag.get(xx, yy) {
                    world.set(sx + xx, top + yy + ripple, c);
                }
            }
        }
    }
}

/// Draw everything that stands on the walk, back to front.
pub(super) fn draw(world: &mut Img, cx: &Cx) {
    let a = art();
    let d = cx.d;
    poles(world, cx, a);
    wards(world, cx, a);
    // Ghosts first: the other knights, behind this one.
    ghosts(world, cx);
    // The foes, their tells under them.
    for (i, f) in d.foes.iter().enumerate() {
        let fx = cx.x(f.x);
        if !(-80..460).contains(&fx) {
            continue;
        }
        if matches!(f.beat, Beat::Tell | Beat::Attack) && f.hp > 0 {
            if f.captain {
                let stage = if f.beat == Beat::Attack { 3 } else { f.tell_stage() };
                danger(world, f.danger, cx, stage);
            } else if f.beat == Beat::Tell {
                aim_line(
                    world,
                    (cx.x(f.x - 16), cx.y(FLOOR - if f.hook { 16 } else { 26 })),
                    (cx.x(f.aim.0), cx.y(f.aim.1)),
                    f.tell_stage(),
                );
            }
        }
        let sprite = foe_img(a, f, cx.tick);
        let hit = cx.s.show.struck(cx.id, i);
        let flash = hit.is_some_and(|h| h.age(cx.t) < h.weight.flash());
        let mut pic = if flash { juice::impact_frame(sprite, 'i') } else { sprite.clone() };
        let mut grow = 1.0;
        let mut dx = 0;
        if let Some(h) = hit.filter(|h| h.age(cx.t) < 26 && h.weight.squash() > 0.0) {
            (pic, grow) = juice::squash_stretch(&pic, h.age(cx.t), h.weight.squash());
            let away = if f.x >= d.x / Q { 1 } else { -1 };
            dx = (juice::lurch(h.weight, h.age(cx.t)) * 14.0) as i32 * away;
        }
        let _ = grow;
        let alive = f.hp > 0;
        let bob = i32::from(alive && f.beat == Beat::Rest && cx.tick / 10 % 2 == 0);
        let (x, y) = (fx - pic.w / 2 + dx, cx.y(FLOOR) - pic.h + 2 - bob);
        if alive {
            shadow(world, fx, if f.captain { 20 } else { 13 });
        }
        world.stamp(&pic, x, y);
        pips(world, f, fx, cx.y(FLOOR) - 56);
        if f.captain && f.beat == Beat::Attack && alive {
            let n = ((10 - f.ticks.min(10)) * 4 / 10).min(3);
            let effect = &a.all[&format!("fx-{}-{n}", if f.hook { "hook" } else { "drop" })].body;
            let area = (cx.x(f.danger.x), cx.y(f.danger.y), f.danger.w, f.danger.h);
            let mut spare = Img::new(1, 1);
            if f.hook {
                for x in (f.danger.x..f.danger.x + f.danger.w).step_by(40) {
                    attack_stamp(world, &mut spare, effect, cx.x(x), cx.y(FLOOR) - effect.h, area, false);
                }
            } else {
                let x = cx.x(f.danger.x + f.danger.w / 2) - effect.w / 2;
                attack_stamp(world, &mut spare, effect, x, cx.y(FLOOR) - effect.h, area, false);
            }
        }
    }
    shots(world, cx, a);
    knight_draw(world, cx);
}

/// Bolts, with a tail; the knight's gold, the sentries' red.
fn shots(world: &mut Img, cx: &Cx, a: &Art) {
    let mut pen = super::super::pfx::Pen::new(world, 600);
    for s in &cx.d.shots {
        let (x, y) = (cx.x(s.x / Q), cx.y(s.y / Q));
        let (ramp, len) = if s.enemy { (super::super::pfx::ramps::BLOOD, 7.0) } else { (super::super::pfx::ramps::GOLD, 11.0) };
        pen.streak((x as f32, y as f32), (-s.vx as f32, -s.vy as f32), len, ramp, 0.2);
    }
    for s in &cx.d.shots {
        let key = if s.enemy { "enemy-bolt" } else { "bolt" };
        let pic = &a.all[key].body;
        let (x, y) = (cx.x(s.x / Q), cx.y(s.y / Q));
        // The bolt points where it goes.
        let pic = if !s.enemy && s.vx < 0 && s.vy == 0 { super::super::rafterwake::flip(pic) } else { pic.clone() };
        pen.cv.stamp(&pic, x - pic.w / 2, y - pic.h / 2);
    }
}

/// The other knights, faint and dithered, at their own places on this wall.
fn ghosts(world: &mut Img, cx: &Cx) {
    for (&gid, g) in &cx.s.lanes {
        if gid == cx.id || g.finish.is_some() {
            continue;
        }
        // Standing in this knight's own footprints: the rail has them.
        if (g.x / Q - cx.d.x / Q).abs() < 22 && g.ground.is_some() == cx.d.ground.is_some() {
            continue;
        }
        let sx = cx.x(g.x / Q);
        if !(-50..440).contains(&sx) {
            continue;
        }
        // Down in a pit or crumpled: a blink.
        if g.down > 0 && (g.y / Q > FLOOR + 30 || cx.t / 3 % 2 == 0) {
            continue;
        }
        let pic = knight(cx.run, gid, usize::from(g.pose), g.dir, false);
        let (x, y) = (sx - 24, cx.y(g.y / Q) - 46);
        let mut faint = Img::new(pic.w, pic.h);
        for yy in 0..pic.h {
            for xx in 0..pic.w {
                if let Some(c) = pic.get(xx, yy)
                    && bayer(xx + 1, yy) < 0.5
                {
                    faint.set(xx, yy, c);
                }
            }
        }
        // A pip over the head in the knight's own ink.
        let mark = super::super::knights::seat_mark(gid);
        world.stamp(&faint, x, y);
        let (hx, hy) = (sx, y + 2);
        world.rect(hx - 2, hy - 7, 5, 4, 'k');
        world.rect(hx - 1, hy - 6, 3, 2, mark);
        world.put(hx, hy - 3, 'k');
    }
}

/// The knight looked through: its shadow, its sprite and what it wears.
fn knight_draw(world: &mut Img, cx: &Cx) {
    let d = cx.d;
    let sx = cx.x(d.x / Q);
    // Down in the moat: nothing stands there.
    if d.down > 0 && d.y / Q > FLOOR + 30 {
        return;
    }
    // Set back down after a fall: a few ticks of blinking.
    if d.hurt > 0 && d.down == 0 && cx.t / 2 % 2 == 0 {
        return;
    }
    let overlap = d.foes.iter().any(|f| (f.x - d.x / Q).abs() < 48);
    let mut pic = knight(cx.run, cx.id, usize::from(d.pose), d.dir, overlap);
    // Struck: a white frame, then the crumple.
    let down = cx.s.show.downs.iter().rev().find(|k| k.seat == cx.id && !k.ward).map(|k| cx.t.saturating_sub(k.born));
    if down.is_some_and(|a| a < 3) {
        pic = juice::impact_frame(&pic, 'i');
    }
    // A landing squashes the boot down a pixel or two.
    let land = cx.s.show.since(crate::rules::pennon::show::Puff::Land, cx.id).filter(|&a| a < 6);
    let (mut w, mut h) = (pic.w, pic.h);
    if let Some(a) = land {
        let k = [3, 2, 1, 1, 0, 0][a as usize];
        if k > 0 {
            let (nw, nh) = (pic.w + k, pic.h - k);
            let mut sq = Img::new(nw, nh);
            for y in 0..nh {
                for x in 0..nw {
                    if let Some(c) = pic.get(x * pic.w / nw, y * pic.h / nh) {
                        sq.set(x, y, c);
                    }
                }
            }
            (pic, w, h) = (sq, nw, nh);
        }
    }
    let ground = d.ground.is_some();
    if ground {
        shadow(world, sx, 11);
    }
    let y = cx.y(d.y / Q) - 46 + (48 - h) + 0;
    world.stamp(&pic, sx - w / 2, y);
    // The ward round it: a ring of light that turns.
    if d.ward > 0 {
        let c = (sx as f32, cx.y(d.y / Q) as f32 - 22.0);
        let mut pen = super::super::pfx::Pen::new(world, 300);
        pen.keep = true;
        let spin = cx.t as f32 * 0.09;
        pen.ring(c, 21.0, 1.0, 1.15, super::super::pfx::ramps::HOLY, 0.55);
        for k in 0..3 {
            let a = spin + k as f32 * 2.094;
            pen.twinkle((c.0 + a.cos() * 21.0, c.1 + a.sin() * 24.0), 1.5, (cx.t as f32 * 0.06 + k as f32 * 0.3).fract(), super::super::pfx::ramps::HOLY);
        }
    }
    let _ = ink;
}
