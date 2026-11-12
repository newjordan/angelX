//! One seat's whole screen at the lists while a bout is ridden: the stage,
//! the crowd, the riders at the gallop with their lances, and everything a
//! meeting throws up; then the strips, the calls and the words over it, in
//! that order, the words last and each claimed in one ledger (`hud`).
//!
//! The world is shaken as one by the blow's weight; the strips and calls
//! stay still. Everything is a function of the run's state: the bout
//! (`Joust`) and its show (`show`), so every seat draws the same moment.

use super::super::super::ink::{Img, bayer};
use super::super::juice;
use super::super::{enlarge, knights};
use super::super::{NATIVE_H, NATIVE_W};
use super::hud::{self, Ledger};
use super::riders::{self, Pose};
use super::stage::{self, FAR_Y, NEAR_Y};
use super::{Coat, Colours, RIVAL_COAT, coat, colours_of, rival_colours};
use super::{crowd, finish, fx, moments};
use crate::motion::ease;
use crate::rules::Run;
use crate::rules::joust::show::{Meet, weight_of};
use crate::rules::joust::{Aim, Hit, Joust, Stage};
use crate::rules::juice::Weight;

/// Pixels to an arena unit.
pub(super) const UNIT: f32 = 8.0;
/// How far each rider is drawn back along his run at the meeting, so the
/// lances touch across the tilt while the riders' bodies are still apart.
const PULL: f32 = 26.0;

/// Everything a draw needs to know, from one seat's view.
pub(super) struct Cx<'a> {
    pub run: &'a Run,
    pub j: &'a Joust,
    pub me: Option<u32>,
    /// The show's clock.
    pub t: u32,
    pub tick: u32,
}

impl<'a> Cx<'a> {
    pub fn new(run: &'a Run, j: &'a Joust, me: Option<u32>) -> Cx<'a> {
        Cx { run, j, me, t: j.show.t, tick: run.tick as u32 }
    }

    /// The rider the viewer is: 0 the knight, 1 the other; none in the gallery.
    pub fn side(&self) -> Option<u8> {
        let me = self.me?;
        if me == self.j.knight {
            Some(0)
        } else if self.j.foe.as_ref().is_some_and(|f| f.id == me) {
            Some(1)
        } else {
            None
        }
    }

    /// A rider's name as this seat reads it.
    pub fn name(&self, side: u8, take: usize) -> String {
        match self.j.rider(side) {
            Some(id) => hud::called(self.run, id, self.me, take),
            None => self.j.rival().name.to_uppercase().chars().take(take).collect(),
        }
    }

    /// The last meeting and how long ago it was.
    pub fn meet(&self) -> Option<(&'a Meet, u32)> {
        self.j.show.last().map(|m| (m, m.age(self.t)))
    }

    /// How hard the room is cheering: 0 hushed, 1 on its feet.
    pub fn fever(&self) -> f32 {
        let j = self.j;
        let base = match j.stage {
            Stage::Ready => 0.2 + 0.08 * f32::from(j.course),
            Stage::Charge => j.closing().map_or(0.5, |k| 0.3 + 0.5 * k * k),
            Stage::Done => 1.0,
        };
        let hot = self.meet().map_or(0.0, |(m, age)| {
            let peak = match m.heaviest() {
                Weight::Graze if !m.landed() => 0.45,
                Weight::Graze | Weight::Solid => 0.7,
                Weight::Crushing => 0.85,
                Weight::Kill | Weight::Finale => 1.0,
            };
            let fall = if age < 70 { 1.0 } else { 1.0 - ((age - 70) as f32 / 90.0).min(1.0) * 0.6 };
            if m.down.0 || m.down.1 { 1.0 } else { peak * fall }
        });
        base.max(hot)
    }

    /// How far a charge has built, 0 at the spur to 1 at the meeting.
    pub fn speed(&self) -> f32 {
        let j = self.j;
        if j.stage != Stage::Charge {
            return 0.0;
        }
        let m = j.meeting().max(1) as f32;
        let t = j.t as f32;
        if t <= m {
            (t / m).powf(0.7)
        } else if j.clashing() {
            0.0
        } else {
            let after = j.t - j.meeting() - j.clash();
            (0.65 - after as f32 / (m * 0.6) * 0.65).max(0.0)
        }
    }
}

/// One rider as the picture has him.
pub(super) struct Placed {
    pub side: u8,
    pub east: bool,
    /// The body's middle (pixels) and the sand under its hooves.
    pub x: i32,
    pub ground: i32,
    pub pose: Pose,
    pub coat: Coat,
    pub barding: (char, char),
    pub col: Colours,
    pub down: bool,
}

/// How far the riders are drawn back (0 to 1 of `PULL`).
fn pull(j: &Joust) -> f32 {
    match j.stage {
        Stage::Charge => {
            let m = j.meeting().max(1) as f32;
            let t = j.t as f32;
            let c = j.clash() as f32;
            if t <= m {
                ease::in_quad(t / m)
            } else if t <= m + c {
                1.0
            } else {
                1.0 - ease::out(((t - m - c) / (m * 0.6)).min(1.0))
            }
        }
        _ => 0.0,
    }
}

/// A lance's height 0 to 4 as it turns from one aim to the other.
fn lance_q(now: Aim, turned: Option<u32>, t: u32) -> u8 {
    let to = if now == Aim::High { 4.0 } else { 0.0 };
    let q = match turned {
        Some(at) if t.saturating_sub(at) < 7 => {
            let k = ease::back_by((t - at) as f32 / 7.0, 1.4).min(1.25);
            (4.0 - to) + (to - (4.0 - to)) * k
        }
        _ => to,
    };
    q.round().clamp(0.0, 4.0) as u8
}

fn guard_q(now: Aim, turned: Option<u32>, t: u32) -> u8 {
    let to = if now == Aim::High { 2 } else { 0 };
    match turned {
        Some(at) if t.saturating_sub(at) < 4 => 1,
        _ => to,
    }
}

/// Both riders as they stand now.
pub(super) fn placed(cx: &Cx) -> [Placed; 2] {
    let j = cx.j;
    let (a, b) = j.riders();
    let k = pull(j);
    let charging = j.stage == Stage::Charge;
    let met = charging && j.hits.is_some();
    let done = j.stage == Stage::Done;
    let (mine, theirs) = j.hits.unwrap_or_default();
    let me_col = colours_of(cx.run, j.knight);
    let (him_col, him_coat) = match &j.foe {
        Some(f) => (colours_of(cx.run, f.id), coat(f.mount)),
        None => (rival_colours(usize::from(j.rival)), RIVAL_COAT),
    };
    let stride = if j.clashing() { j.meeting() } else { j.t };
    // Spurred riders paw the ground, a few strides, until the flag drops.
    let paw = |side: usize, spurred: bool| -> Option<u32> {
        let at = j.show.spurred[side]?;
        (spurred && j.stage == Stage::Ready).then(|| (cx.t.saturating_sub(at)) / 3 % 4)
    };
    let foe_spurred = j.foe.as_ref().is_some_and(|f| f.spurred);
    let gait = |seed: u32, paw: Option<u32>| charging.then_some((stride / 3 + seed) % 4).or(paw);
    let waver = (j.t / 6).is_multiple_of(2);
    let guard_far = match j.guard_shown() {
        Some(g) => guard_q(g, j.show.shielded[1], cx.t),
        None if charging => if waver { 2 } else { 0 },
        None => 2,
    };
    let lance_far = match j.lance_shown() {
        Some(l) => lance_q(l, j.show.aimed[1], cx.t),
        None if charging => (2.0 + 2.0 * (j.t as f32 * 0.55).sin()).round() as u8,
        None => 0,
    };
    let winner = j.winner();
    let wind = ((cx.tick / 3) % 4) as u8;
    let mk = |side: u8, (x, y, east): (f32, f32, bool), col: Colours, coat: Coat, hit: Hit, gait: Option<u32>, aim: u8, guard: u8| {
        let _ = y;
        let dir = if east { 1.0 } else { -1.0 };
        let fallen = if side == 0 { j.fallen.0 } else { j.fallen.1 };
        let down = fallen && (met || done);
        let px = ((x * UNIT - dir * PULL * k) as i32).clamp(46, 338);
        Placed {
            side,
            east,
            x: px,
            ground: if side == 0 { NEAR_Y } else { FAR_Y },
            pose: Pose {
                gait,
                aim,
                guard,
                broken: matches!(hit, Hit::Broke | Hit::Smite) && met,
                crowned: done && winner == Some(side),
                riderless: down,
                far: side == 1,
                wind,
                salute: done && winner == Some(side),
                couched: charging && !j.unset() && !met,
            },
            coat,
            barding: (col.tabard, col.field),
            col,
            down,
        }
    };
    [
        mk(
            0,
            a,
            me_col,
            coat_of(j),
            mine,
            gait(0, paw(0, j.spurred)),
            lance_q(j.aim, j.show.aimed[0], cx.t),
            guard_q(j.shield, j.show.shielded[0], cx.t),
        ),
        mk(1, b, him_col, him_coat, theirs, gait(1, paw(1, foe_spurred)), lance_far, guard_far),
    ]
}

fn coat_of(j: &Joust) -> Coat {
    coat(j.mount)
}

/// Where a rider's lance tip is in the picture (the unbroken lance).
pub(super) fn tip_of(p: &Placed) -> (i32, i32) {
    let reach = 30;
    let a = i32::from(p.pose.aim.min(4));
    let ty = if p.pose.far { 8 - a * 3 } else { -6 - a * 3 };
    let (sx, sy) = (21 + reach, 26 + ty);
    let x = if p.east { sx - riders::BODY_X } else { riders::BODY_X - sx };
    (p.x + x, p.ground - riders::HOOF_Y + sy)
}

/// A rider's sprite in the picture: facing, shaken by a blow it took.
fn stamp_rider(world: &mut Img, cx: &Cx, p: &Placed) {
    let built = riders::mounted(p.coat, p.barding, p.col, p.pose);
    let body = if p.east { riders::BODY_X } else { riders::W - 1 - riders::BODY_X };
    // A standing horse breathes.
    let breathe = i32::from(p.pose.gait.is_none() && (cx.tick / 14) % 3 == 0);
    let base_y = p.ground + (riders::H - riders::HOOF_Y) + breathe;
    let (x0, dir) = (p.x - body, if p.east { 1 } else { -1 });
    // The blur of speed: echoes behind, dimmer and thinner.
    if cx.j.stage == Stage::Charge && !p.down {
        let v = cx.speed();
        for (k, (off, need)) in [(7, 0.45), (15, 0.62), (24, 0.8)].into_iter().enumerate() {
            if v >= need {
                let g = &built.ghosts[usize::from(!p.east)][k];
                world.stamp(g, x0 - dir * off, base_y - g.h);
            }
        }
    }
    let mut sprite = None;
    let mut dx = 0;
    // A blow taken: the white frame, then the squash, and the lurch back.
    if let Some((m, age)) = cx.meet() {
        let taken = if p.side == 0 { m.taken.0 } else { m.taken.1 };
        let by = if p.side == 0 { m.hits.1 } else { m.hits.0 };
        if by != Hit::Miss && !p.down && age < 24 {
            let mut im = built.facing(p.east).clone();
            if age < taken.flash() {
                im = riders::tint(&im, 'H');
            }
            sprite = Some(juice::squash_stretch(&im, age, taken.squash() * 0.45).0);
            dx = (-(dir as f32) * juice::lurch(taken, age) * 3.0 * UNIT) as i32;
        }
    }
    match sprite {
        Some(s) => world.stamp(&s, x0 + dx, base_y - s.h),
        None => {
            let s = built.facing(p.east);
            world.stamp(s, x0, base_y - s.h);
        }
    }
}

/// The knight flung from the saddle: up and over, turning as he goes, down
/// into the sand, a little bounce, and he lies on his back, the helm rolling
/// off on its own.
fn tumble(world: &mut Img, cx: &Cx, p: &Placed, age: u32) {
    const LAND: u32 = 24;
    let dir = if p.east { 1 } else { -1 };
    let back = -dir;
    let a = age as f32;
    let (x0, y0) = (p.x - dir * 6, p.ground - 24);
    let flown = ease::out((a / (LAND as f32 + 6.0)).min(1.0));
    let x = x0 + back * (flown * 46.0) as i32;
    let ground = p.ground - 16;
    let up = if age < LAND {
        let s = a / LAND as f32;
        (s * (1.0 - s) * 4.0 * 34.0) as i32
    } else {
        let s = ((a - LAND as f32) / 8.0).min(1.0);
        (s * (1.0 - s) * 4.0 * 5.0) as i32
    };
    let y = (y0 + ((ground - y0) as f32 * (a / LAND as f32).min(1.0)) as i32) - up;
    // Turning head over heels, slowing into a flat landing.
    let turns = 630.0 * ease::out((a / (LAND as f32 + 2.0)).min(1.0));
    let kick = age >= LAND && (cx.tick / 4).is_multiple_of(2);
    let body = enlarge(&riders::flung(p.col, age < 7, kick), 2.0);
    let body = if p.east { body } else { body.flip_h() };
    let body = riders::turned(&body, -turns * dir as f32);
    world.stamp(&body, x - body.w / 2, y - body.h / 2);
    if age >= 7 {
        // The helm: off at the top of the arc, two bounces, a roll.
        let t = (age - 7).min(70) as i32;
        let hx0 = x0 + back * 8;
        let hx = hx0 + back * ((t * 2).min(34) + (t - 16).clamp(0, 12) / 2);
        let gnd = ground + 12;
        let hy = match t {
            0..=11 => y0 - 14 - (t * (11 - t)) / 2 + t * 3,
            12..=17 => gnd - ((t - 12) * (17 - t)) / 2,
            _ => gnd,
        };
        let h = enlarge(&riders::helm_img(if t < 28 { (t / 2) as u32 } else { 1 }), 2.0);
        world.stamp(&h, hx - 5, hy.min(gnd) - 5);
    }
    if age == LAND || age > LAND {
        fx::landing(world, (x as f32, (p.ground - 6) as f32), age - LAND, p.side as u32 * 31 + 7, cx.tick as u64);
    }
    if age >= LAND + 4 {
        fx::dizzy(world, (x, y - 22), cx.tick);
    }
}

/// The gallery on the stands' front tier: each knight at the lists in their
/// own figure with a pennant in the colours of the rider they back.
pub(super) fn gallery_spots(cx: &Cx) -> Vec<(u32, i32)> {
    let slots: &[i32] = match cx.j.gallery.len() {
        0 | 1 => &[96],
        2 => &[96, 288],
        3 => &[58, 192, 326],
        _ => &[58, 148, 238, 328],
    };
    cx.j.gallery.keys().enumerate().map(|(i, &id)| (id, slots[i.min(slots.len() - 1)])).collect()
}

/// A gallery knight's figure, at twice its size so the seat is seen.
pub(super) fn gallery_figure(cx: &Cx, id: u32) -> Option<Img> {
    let hero = cx.run.players.get(&id)?;
    let mut hero = hero.clone();
    (hero.aim_x, hero.aim_y) = (0.0, 1.0);
    Some(enlarge(&knights::figure(&hero, id), 2.0))
}

fn gallery(world: &mut Img, cx: &Cx, placed: &[Placed; 2]) {
    let hot = cx.fever();
    for (id, x) in gallery_spots(cx) {
        let Some(w) = cx.j.gallery.get(&id) else { continue };
        let Some(fig) = gallery_figure(cx, id) else { continue };
        let stand = hot > 0.5 && (cx.tick / 4 + id) % 2 == 0;
        let y = stage::RAIL_Y - 1 - fig.h - i32::from(stand) * 2;
        world.stamp(&fig, x - fig.w / 2, y);
        // The pennant in the backed rider's colours, waving.
        if let Some(side) = w.side {
            let p = &placed[usize::from(side.min(1))];
            let (top, flag) = (y - 6, p.col);
            let (px, wave) = (x + fig.w / 2 + 1, (cx.tick / 4 + id) % 2);
            world.line(px, top, px, y + fig.h - 4, 'o');
            for k in 0..7 {
                let len = 11 - k;
                let ripple = i32::from(wave == 1 && k > 2);
                world.line(px + 1, top + k + ripple, px + len, top + k + ripple, if k == 3 { flag.field } else { flag.tabard });
            }
        }
    }
}

/// The favours in the air: from each thrower toward the rider.
fn favours(world: &mut Img, cx: &Cx, placed: &[Placed; 2]) {
    let spots = gallery_spots(cx);
    for f in &cx.j.show.favours {
        let Some(&(_, x)) = spots.iter().find(|(id, _)| *id == f.by) else { continue };
        let p = &placed[usize::from(f.side.min(1))];
        let to = (p.x, p.ground - 34);
        // The throw: a glint where it leaves his hand.
        fx::glint(world, (x + 8, stage::RAIL_Y - 26), f.age(cx.t), 9);
        fx::favour(world, (x, stage::RAIL_Y - 24), to, f.age(cx.t), crate::rules::joust::show::FAVOUR_LIFE, (p.col.tabard, p.col.field));
        if f.age(cx.t) >= crate::rules::joust::show::FAVOUR_LIFE - 8 {
            fx::glint(world, to, f.age(cx.t) - (crate::rules::joust::show::FAVOUR_LIFE - 8), 8);
        }
    }
}

/// The scenery, the crowd, the riders and what they throw up: the world,
/// before it is shaken.
fn world(cx: &Cx) -> Img {
    let j = cx.j;
    let mut w = stage::backdrop().clone();
    stage::live(&mut w, cx.tick);
    let placed = placed(cx);
    let met_x = (j.meeting_x() * UNIT) as i32;
    crowd::draw(&mut w, cx.tick, cx.fever(), met_x);
    gallery(&mut w, cx, &placed);
    let speed = cx.speed();
    fx::wind(&mut w, speed, cx.tick);
    let meet = cx.meet();
    // Far lane first, behind the tilt.
    for (n, p) in placed.iter().rev().enumerate() {
        let dir = if p.east { 1 } else { -1 };
        let v = if j.stage == Stage::Charge { speed } else { 0.0 };
        if v > 0.0 {
            fx::speed_lines(&mut w, p.x - dir * 22, p.ground, dir, v, cx.tick, 11 + p.side as u32 * 5);
            hoofbeats(&mut w, cx, p, v);
        } else if p.pose.gait.is_some() && j.stage == Stage::Ready {
            // Pawing the ground: a puff each stride.
            let age = (cx.t % 9) as u32;
            fx::puff(&mut w, ((p.x - dir * 18) as f32, p.ground as f32 - 2.0), dir, age, 5 + p.side as u32, 3, cx.tick as u64);
        }
        if p.down && meet.is_some() {
            // The knight's horse runs on without him.
        }
        stamp_rider(&mut w, cx, p);
        if n == 0 {
            w.stamp(stage::tilt(), 0, stage::TILT_Y - 3);
        }
    }
    // The riders who went over.
    if let Some((m, age)) = meet {
        for p in placed.iter().filter(|p| p.down) {
            let _ = m;
            let fallen_age = if j.stage == Stage::Done { 120 + j.t } else { age };
            // The fall begins where the riders met, at the moment they did.
            let at = Placed { x: ((m.x * UNIT) as i32 + if p.east { -PULL as i32 } else { PULL as i32 }).clamp(46, 338), ..clone_placed(p) };
            tumble(&mut w, cx, &at, fallen_age);
        }
        impact(&mut w, cx, m, age, &placed);
    }
    // The clash: lances grinding.
    if j.clashing() && j.hits.is_none() {
        let (a, b) = (tip_of(&placed[0]), tip_of(&placed[1]));
        let mid = ((a.0 + b.0) as f32 / 2.0, (a.1 + b.1) as f32 / 2.0);
        let hot = ((j.t - j.meeting()) as f32 / j.clash().max(1) as f32).min(1.0);
        fx::grind(&mut w, mid, cx.tick as u64, hot);
        // The horses dig in: dust skids out from under the hooves.
        for p in &placed {
            let dir = if p.east { 1.0 } else { -1.0 };
            fx::puff(&mut w, (p.x as f32 + dir * 16.0, p.ground as f32 - 2.0), if p.east { -1 } else { 1 }, (cx.tick % 8) + 2, 21 + p.side as u32, 4, cx.tick as u64);
        }
    }
    favours(&mut w, cx, &placed);
    if let Some((m, age)) = meet {
        if m.down.0 || m.down.1 || m.heaviest() >= Weight::Kill {
            crowd::thrown(&mut w, age, 17 + u32::from(m.course) * 13, if m.down.0 || m.down.1 { 22 } else { 10 });
        }
    }
    w.stamp(stage::fence(), 0, stage::FENCE_Y);
    w
}

fn clone_placed(p: &Placed) -> Placed {
    Placed { side: p.side, east: p.east, x: p.x, ground: p.ground, pose: p.pose, coat: p.coat, barding: p.barding, col: p.col, down: p.down }
}

/// The dust each hoofbeat kicked up behind a rider, a few strides back.
fn hoofbeats(w: &mut Img, cx: &Cx, p: &Placed, v: f32) {
    let j = cx.j;
    let mut beats = Vec::new();
    let now = j.t;
    for k in 0..5u32 {
        let tb = (now / 3).saturating_sub(k) * 3;
        if tb == 0 && k > 0 || (j.stage == Stage::Charge && tb > j.meeting() && !j.clashing() && tb > now) {
            continue;
        }
        let (a, b) = j.riders_at(tb);
        let r = if p.side == 0 { a } else { b };
        let age = now - tb;
        if age > 15 {
            continue;
        }
        beats.push((age, r.0 * UNIT - if p.east { 1.0 } else { -1.0 } * PULL * pull_at(j, tb)));
    }
    fx::hoofbeats(w, &beats, p.ground as f32, if p.east { 1 } else { -1 }, v, 3 + p.side as u32 * 17, cx.tick as u64);
}

fn pull_at(j: &Joust, t: u32) -> f32 {
    let m = j.meeting().max(1) as f32;
    ease::in_quad((t as f32 / m).min(1.0))
}

/// The crack of the lances meeting and what follows: flare, star, ring,
/// sparks, splinters, the dust of the blow.
fn impact(w: &mut Img, cx: &Cx, m: &Meet, age: u32, placed: &[Placed; 2]) {
    if !m.landed() || age > 60 {
        return;
    }
    let (a, b) = (tip_of(&placed[0]), tip_of(&placed[1]));
    let at = (((a.0 + b.0) / 2) as f32, ((a.1 + b.1) / 2) as f32);
    let heavy = m.heaviest();
    fx::crack(w, at, age, heavy, 5 + u32::from(m.course) * 3, cx.tick as u64);
    // A lance that broke on a shield, or a mighty one: splinters.
    for (i, (hit, p)) in [(m.hits.0, &placed[0]), (m.hits.1, &placed[1])].into_iter().enumerate() {
        if matches!(hit, Hit::Broke | Hit::Smite) {
            let tip = tip_of(p);
            fx::splinters(w, (tip.0 as f32, tip.1 as f32), age, 3 + i as u32 * 11 + u32::from(m.course) * 5, 12, stage::NEAR_Y as f32 - 4.0);
        } else if hit == Hit::Clean {
            let tip = tip_of(p);
            fx::splinters(w, (tip.0 as f32, tip.1 as f32), age, 9 + i as u32 * 7, 4, stage::NEAR_Y as f32 - 4.0);
        }
    }
    if heavy >= Weight::Crushing && age < 6 {
        fx::flare(w, (at.0 as i32, stage::NEAR_Y - 6), 1.0 - age as f32 / 6.0, 70, 'B');
    }
}

/// A seat's screen at the lists while a bout is ridden.
pub(super) fn frame(run: &Run, focus: Option<u32>) -> Img {
    let Some(j) = run.joust.as_ref() else { return Img::black(NATIVE_W, NATIVE_H) };
    let cx = Cx::new(run, j, focus);
    let mut led = Ledger::default();
    let mut im = Img::black(NATIVE_W, NATIVE_H);
    if let Some(e) = finish::age(&cx) {
        finish::scene(&mut im, &cx, &mut led, e);
        led.keep();
        return im;
    }
    let mut world = world(&cx);
    if let Some((k, at)) = punch(&cx) {
        world = zoomed(&world, at, k);
    }
    let shake = shake_of(&cx);
    im.stamp(&world, shake.0, shake.1);
    // The speed of the charge closes in on the edges.
    let v = cx.speed();
    if v > 0.45 {
        fx::rim(&mut im, ((v - 0.4) * 1.7).min(0.95), 'k');
    }
    if let Some((m, age)) = cx.meet() {
        // A blow taken is a red rim for the knight who took it.
        let mine = cx.side().map(|s| if s == 0 { (m.taken.0, m.hits.1) } else { (m.taken.1, m.hits.0) });
        if let Some((w, by)) = mine && by != Hit::Miss && w >= Weight::Crushing && age < 10 {
            fx::rim(&mut im, (1.0 - age as f32 / 10.0) * 0.28, '7');
        }
    }
    moments::draw(&mut im, &cx, &mut led);
    led.keep();
    im
}

/// The camera's punch-in on a heavy blow: how far (1 is none) and on what.
/// It is at its most the instant of the blow, held through the hit-stop,
/// and eases home as time moves again.
fn punch(cx: &Cx) -> Option<(f32, (f32, f32))> {
    let (m, age) = cx.meet()?;
    let amp = match m.heaviest() {
        Weight::Finale => 0.2,
        Weight::Kill => 0.11,
        Weight::Crushing => 0.05,
        _ => return None,
    };
    if age > 26 || !m.landed() {
        return None;
    }
    let k = 1.0 + amp * crate::motion::math::exp(-(age as f32) * 0.22);
    let p = placed(cx);
    let (a, b) = (tip_of(&p[0]), tip_of(&p[1]));
    Some((k, (((a.0 + b.0) / 2) as f32, ((a.1 + b.1) / 2) as f32 + 10.0)))
}

/// `im` pushed in on `at` by `k`, nearest neighbour, the frame kept.
fn zoomed(im: &Img, at: (f32, f32), k: f32) -> Img {
    let mut out = Img::black(im.w, im.h);
    for y in 0..im.h {
        for x in 0..im.w {
            let (sx, sy) = (at.0 + (x as f32 - at.0) / k, at.1 + (y as f32 - at.1) / k);
            if let Some(c) = im.get(sx.round() as i32, sy.round() as i32) {
                out.set(x, y, c);
            }
        }
    }
    out
}

/// The world's shake: the rumble of the charge, the blow's own.
fn shake_of(cx: &Cx) -> (i32, i32) {
    let phase = cx.tick as u64 + u64::from(cx.run.hitstop.left) * 3;
    let mut s = (0, 0);
    if let Some((m, age)) = cx.meet() {
        if m.landed() {
            s = juice::shake([(m.heaviest(), age)], phase);
        }
    }
    let v = cx.speed();
    if s == (0, 0) && v > 0.55 {
        // The hooves on the sand: a shiver, harder the faster.
        let k = (v - 0.55) / 0.45;
        let up = if cx.tick % 2 == 0 && k > 0.2 { 1 } else { 0 };
        let side = if cx.tick % 3 == 0 && k > 0.6 { 1 } else { 0 };
        s = (side, up);
    }
    s
}

/// The bout's own `legible` use: the stage for a reduced frame is the same
/// picture, scaled by the field.
pub(super) fn layers(run: &Run, focus: Option<u32>) -> (Img, Img) {
    (frame(run, focus), Img::new(NATIVE_W, NATIVE_H))
}

#[allow(dead_code)]
fn _keep(_: u32) -> f32 {
    bayer(0, 0) + weight_of(Hit::Miss, false) as i32 as f32
}
