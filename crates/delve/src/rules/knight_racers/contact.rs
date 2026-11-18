//! Pod against pod: the wake a pod rides, and what happens when two touch.
//!
//! A pod is two boxes on the road, the engines and the chariot on its tethers behind them,
//! and every box has a mass from the pod's spec. Where two overlap the pair is pushed apart
//! (the lighter pod gives more ground), their speeds trade (the lighter pod loses more) and,
//! when it was a hard hit, the engine nearest the blow takes heat and wear. A side slam also
//! throws the pods sideways on a `kick` that the steering cannot cancel, so a shove is felt.
//! A pod ahead is never passed through: the boxes are longer than a pod can move in a tick
//! at the fastest closing speed, and a pair that did swap places is put back (see `swept`).
use super::engines::{dump, near};
use super::{Car, HZ};
use std::collections::BTreeMap;

pub const SLAM_SPEED: f32 = 4.5;
const BOUNCE: f32 = 0.30;
pub const WAKE_REACH: f32 = 26.;
const WAKE_PEAK: (f32, f32) = (3., 11.);
const CHARIOT_HALF_W: f32 = 1.15;

#[derive(Clone, Copy, Debug)]
pub struct Body { pub back: f32, pub front: f32, pub half_w: f32 }

pub fn bodies(c: &Car) -> [Body; 2] {
    let s = c.spec();
    [Body { back: -s.len * 0.5, front: s.len * 0.5 + 0.6, half_w: s.half_w }, Body { back: -s.tail, front: -s.len * 0.5, half_w: CHARIOT_HALF_W }]
}

pub fn nose(c: &Car) -> f32 { bodies(c)[0].front }
pub fn tail(c: &Car) -> f32 { -bodies(c)[1].back }

pub fn ahead(lap: f32, a: f32, b: f32) -> f32 {
    let d = (b - a).rem_euclid(lap);
    if d > lap * 0.5 { d - lap } else { d }
}

pub fn wake(lap: f32, me: &Car, lead: &Car) -> (f32, f32) {
    if lead.finish.is_some() || lead.blown() || me.route != lead.route || me.flying || lead.flying { return (0., f32::MAX); }
    if lead.v < 22. || me.v < 16. { return (0., f32::MAX); }
    let gap = ahead(lap, me.d, lead.d) - nose(me) - tail(lead);
    if gap < 0. || gap > WAKE_REACH { return (0., gap); }
    let half = lead.spec().half_w * 0.55 + 0.8 + gap * 0.06;
    let off = (me.y - lead.y).abs();
    let across = (1. - (off / half).powi(2)).clamp(0., 1.);
    let along = if gap < WAKE_PEAK.0 { 0.55 + 0.45 * gap / WAKE_PEAK.0 } else if gap <= WAKE_PEAK.1 { 1. } else { 1. - (gap - WAKE_PEAK.1) / (WAKE_REACH - WAKE_PEAK.1) };
    (across * along, gap)
}

pub fn wake_of(lap: f32, cars: &BTreeMap<u32, Car>, id: u32) -> (f32, u32) {
    let Some(me) = cars.get(&id) else { return (0., 0) };
    if me.finish.is_some() { return (0., 0); }
    let mut best = (0., 0);
    for (&o, lead) in cars {
        if o == id { continue; }
        let (w, _) = wake(lap, me, lead);
        if w > best.0 { best = (w, o); }
    }
    best
}

pub const PAINT_KICK: f32 = 1.2;
pub const PAINT_SHED: f32 = 0.985;
pub const PAINT_TICKS: u32 = 36;
pub const SCUFFS: usize = 4;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Touch { pub a: u32, pub b: u32, pub side: bool, pub speed: f32, pub slam: bool }

fn trade_paint(c: &mut Car, other: u32, toward: i8) {
    if c.paint == 0 {
        c.rubs += 1;
        if c.scuffs.len() == SCUFFS { c.scuffs.remove(0); }
        c.scuffs.push((other as u8, toward));
        c.kick -= f32::from(toward) * PAINT_KICK;
        c.v *= PAINT_SHED;
    }
    c.paint = PAINT_TICKS;
    c.paint_side = toward;
    c.paint_with = other as u8;
}

#[derive(Clone, Copy)]
struct Overlap { along: f32, across: f32, a: Body, b: Body }

fn overlap(lap: f32, a: &Car, b: &Car) -> Option<Overlap> {
    let fg = ahead(lap, a.d, b.d);
    let dy = b.y - a.y;
    let mut best: Option<Overlap> = None;
    for ba in bodies(a) {
        for bb in bodies(b) {
            let along = (ba.front).min(fg + bb.front) - (ba.back).max(fg + bb.back);
            let across = (ba.half_w + bb.half_w) - dy.abs();
            if along > 0. && across > 0. && best.is_none_or(|o| along.min(across) > o.along.min(o.across)) { best = Some(Overlap { along, across, a: ba, b: bb }); }
        }
    }
    best
}

fn weight(c: &Car) -> f32 { c.spec().mass * if c.blown() { 4. } else { 1. } }

pub fn resolve(lap: f32, cars: &mut BTreeMap<u32, Car>, before: &BTreeMap<u32, (f32, f32)>) -> Vec<Touch> {
    let ids: Vec<u32> = cars.iter().filter(|(_, c)| c.finish.is_none() && !c.flying).map(|(id, _)| *id).collect();
    let mut touches = Vec::new();
    for (i, &ia) in ids.iter().enumerate() {
        for &ib in &ids[i + 1..] {
            let (ca, cb) = (&cars[&ia], &cars[&ib]);
            if ca.route != cb.route { continue; }
            let fg = ahead(lap, ca.d, cb.d);
            let was = before.get(&ia).zip(before.get(&ib)).map(|(pa, pb)| (ahead(lap, pa.0, pb.0), pb.1 - pa.1));
            let crossed = was.is_some_and(|(wfg, wdy)| wfg * fg < 0. && wfg.abs() < lap * 0.25 && wdy.abs().min((cb.y - ca.y).abs()) < bodies(ca)[0].half_w + bodies(cb)[0].half_w);
            let hit = overlap(lap, ca, cb);
            if hit.is_none() && !crossed { continue; }
            let (ma, mb) = (weight(ca), weight(cb));
            let total = ma + mb;
            if crossed {
                let (rear, front) = if was.is_some_and(|(wfg, _)| wfg >= 0.) { (ia, ib) } else { (ib, ia) };
                let room = nose(&cars[&rear]) + tail(&cars[&front]) + 0.05;
                let at = cars[&front].d - room;
                let closing = (cars[&rear].v - cars[&front].v).max(0.);
                let r = cars.get_mut(&rear).unwrap();
                r.d = at;
                r.v = (r.v - closing * 0.8).max(0.);
                r.impact = r.impact.max(10);
                let f = cars.get_mut(&front).unwrap();
                f.v += closing * 0.2;
                f.impact = f.impact.max(10);
                touches.push(Touch { a: ia, b: ib, side: false, speed: closing, slam: true });
                continue;
            }
            let hit = hit.unwrap();
            let side = match was {
                Some((wfg, wdy)) => {
                    let lat_apart = wdy.abs() >= hit.a.half_w + hit.b.half_w;
                    let along_apart = wfg + hit.b.back >= hit.a.front || wfg + hit.b.front <= hit.a.back;
                    if lat_apart && !along_apart { true } else if along_apart && !lat_apart { false } else { hit.across < hit.along }
                }
                None => hit.across < hit.along,
            };
            let (along, across) = (hit.along, hit.across);
            let (speed, slam);
            if side {
                let n = if cb.y > ca.y { 1. } else if cb.y < ca.y { -1. } else if ia < ib { 1. } else { -1. };
                let rel = ((cb.vy + cb.kick) - (ca.vy + ca.kick)) * n;
                let closing = (-rel).max(0.);
                speed = closing;
                slam = closing >= SLAM_SPEED;
                let j = (1. + BOUNCE) * closing / (1. / ma + 1. / mb);
                let (fa, fb) = ((mb / total).min(1.), (ma / total).min(1.));
                let a = cars.get_mut(&ia).unwrap();
                a.y -= n * across * fa;
                a.kick -= n * j / ma;
                a.vy -= n * j / ma * 0.5;
                a.v = (a.v - 0.25 * closing * fa).max(0.);
                if slam { a.impact = a.impact.max(10); }
                let b = cars.get_mut(&ib).unwrap();
                b.y += n * across * fb;
                b.kick += n * j / mb;
                b.vy += n * j / mb * 0.5;
                b.v = (b.v - 0.25 * closing * fb).max(0.);
                if slam { b.impact = b.impact.max(10); }
                if !slam {
                    trade_paint(cars.get_mut(&ia).unwrap(), ib, n as i8);
                    trade_paint(cars.get_mut(&ib).unwrap(), ia, -n as i8);
                }
                if slam {
                    let (fa_h, fb_h) = ((mb / ma).sqrt().clamp(0.6, 1.6), (ma / mb).sqrt().clamp(0.6, 1.6));
                    let a = cars.get_mut(&ia).unwrap();
                    dump(a, Some(near(a.y, a.y + n)), 0.010 * closing * fa_h, 0.005 * closing * fa_h);
                    a.slam = SLAM_TICKS; a.slam_side = n as i8; a.slams += 1;
                    let b = cars.get_mut(&ib).unwrap();
                    dump(b, Some(near(b.y, b.y - n)), 0.010 * closing * fb_h, 0.005 * closing * fb_h);
                    b.slam = SLAM_TICKS; b.slam_side = -n as i8; b.slams += 1;
                }
            } else {
                let (rear, front) = if fg >= 0. { (ia, ib) } else { (ib, ia) };
                let (mr, mf) = (weight(&cars[&rear]), weight(&cars[&front]));
                let closing = (cars[&rear].v - cars[&front].v).max(0.);
                speed = closing;
                slam = closing >= SLAM_SPEED;
                let j = (1. + BOUNCE * 0.7) * closing / (1. / mr + 1. / mf);
                let r = cars.get_mut(&rear).unwrap();
                r.d -= along * (mf / (mr + mf));
                r.v = (r.v - j / mr).max(0.);
                if slam { r.impact = r.impact.max(10); }
                let f = cars.get_mut(&front).unwrap();
                f.d += along * (mr / (mr + mf));
                f.v += j / mf * 0.7;
                if slam { f.impact = f.impact.max(10); }
                if slam {
                    let (sr, sf) = ((mf / mr).sqrt().clamp(0.6, 1.6), (mr / mf).sqrt().clamp(0.6, 1.6));
                    let f = cars.get_mut(&front).unwrap();
                    dump(f, None, 0.006 * closing * sf, 0.003 * closing);
                    f.slam = SLAM_TICKS; f.slam_side = 0; f.slams += 1;
                    let r = cars.get_mut(&rear).unwrap();
                    dump(r, None, 0.004 * closing * sr, 0.002 * closing);
                    r.slam = SLAM_TICKS; r.slam_side = 0; r.slams += 1;
                }
            }
            touches.push(Touch { a: ia, b: ib, side, speed, slam });
        }
    }
    touches
}

pub const SLAM_TICKS: u32 = 24;
pub const KICK_DECAY: f32 = 0.88;

pub fn kick_step(kick: f32) -> (f32, f32) { (kick / HZ, kick * KICK_DECAY) }
