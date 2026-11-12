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

/// Closing speed, metres a second, from which a touch is a slam: it hurts, and it is told.
pub const SLAM_SPEED: f32 = 4.5;
/// How much of a closing speed a bounce gives back.
const BOUNCE: f32 = 0.30;
/// The tail of the draft: how far behind a pod's chariot its wake still pulls, metres.
pub const WAKE_REACH: f32 = 26.;
/// Where the wake is strongest: from this far behind the chariot to this far.
const WAKE_PEAK: (f32, f32) = (3., 11.);
/// The chariot's half width, metres.
const CHARIOT_HALF_W: f32 = 1.15;

/// A box on the road in (distance, lane): `back..front` ahead of the pod's position, `half_w` either side.
#[derive(Clone, Copy, Debug)]
pub struct Body { pub back: f32, pub front: f32, pub half_w: f32 }

/// The two boxes of a car's pod: the engines, then the chariot behind them.
pub fn bodies(c: &Car) -> [Body; 2] {
    let s = c.spec();
    [Body { back: -s.len * 0.5, front: s.len * 0.5 + 0.6, half_w: s.half_w }, Body { back: -s.tail, front: -s.len * 0.5, half_w: CHARIOT_HALF_W }]
}

/// The distance from a pod's position to the front of its nose and to the end of its tail.
pub fn nose(c: &Car) -> f32 { bodies(c)[0].front }
pub fn tail(c: &Car) -> f32 { -bodies(c)[1].back }

/// The signed distance along the lap from `a` to `b`: positive when `b` is ahead, folded to
/// half a lap either way so the line is no seam.
pub fn ahead(lap: f32, a: f32, b: f32) -> f32 {
    let d = (b - a).rem_euclid(lap);
    if d > lap * 0.5 { d - lap } else { d }
}

/// How much the pod `lead` ahead of `me` pulls it along, 0..1, and the gap that decides it:
/// the metres between `me`'s nose and `lead`'s tail. The wake is a cone that opens behind a pod
/// that is moving: straightest and strongest a few lengths back, gone past `WAKE_REACH`.
pub fn wake(lap: f32, me: &Car, lead: &Car) -> (f32, f32) {
    if lead.finish.is_some() || lead.blown() || me.route != lead.route || me.flying || lead.flying { return (0., f32::MAX); }
    if lead.v < 22. || me.v < 16. { return (0., f32::MAX); }
    let gap = ahead(lap, me.d, lead.d) - nose(me) - tail(lead);
    if gap < 0. || gap > WAKE_REACH { return (0., gap); }
    // The cone opens as it goes back; its edge is soft.
    let half = lead.spec().half_w * 0.55 + 0.8 + gap * 0.06;
    let off = (me.y - lead.y).abs();
    let across = (1. - (off / half).powi(2)).clamp(0., 1.);
    let along = if gap < WAKE_PEAK.0 { 0.55 + 0.45 * gap / WAKE_PEAK.0 } else if gap <= WAKE_PEAK.1 { 1. } else { 1. - (gap - WAKE_PEAK.1) / (WAKE_REACH - WAKE_PEAK.1) };
    (across * along, gap)
}

/// What a pod rides, this tick: the strongest wake any pod ahead throws on it, and whose it is.
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

/// One touch between two pods, for the tests and the feel of it.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Touch { pub a: u32, pub b: u32, pub side: bool, pub speed: f32, pub slam: bool }

/// Where two pods' boxes overlap: the deepest pair of boxes, how far they overlap along the road
/// and across it, and the pair itself (a's box, b's box in a's frame).
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

/// A pod sitting out a blowout is a block in the road: it takes a shove like something four times its weight.
fn weight(c: &Car) -> f32 { c.spec().mass * if c.blown() { 4. } else { 1. } }

/// Resolve every touch between the pods this tick. `before` holds where each pod was before it
/// moved, so the way a pair met is known (from the side, or nose into tail) and a pair that swapped
/// places in one step is caught too. Returns what happened.
pub fn resolve(lap: f32, cars: &mut BTreeMap<u32, Car>, before: &BTreeMap<u32, (f32, f32)>) -> Vec<Touch> {
    let ids: Vec<u32> = cars.iter().filter(|(_, c)| c.finish.is_none() && !c.flying).map(|(id, _)| *id).collect();
    let mut touches = Vec::new();
    for (i, &ia) in ids.iter().enumerate() {
        for &ib in &ids[i + 1..] {
            let (ca, cb) = (&cars[&ia], &cars[&ib]);
            if ca.route != cb.route { continue; }
            // Which is behind now, and was it behind a tick ago? A pair that crossed on the road
            // in one tick went through each other: put the one that was behind back behind.
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
            // The way they met: from the side if they were apart sideways a tick ago, nose into tail if
            // they were apart along the road, and if they were already touching, the shallower way out.
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
                // A nose into a tail: whoever is behind runs into the other.
                let (rear, front) = if fg >= 0. { (ia, ib) } else { (ib, ia) };
                let (mr, mf) = (weight(&cars[&rear]), weight(&cars[&front]));
                let closing = (cars[&rear].v - cars[&front].v).max(0.);
                speed = closing;
                slam = closing >= SLAM_SPEED;
                let j = (1. + BOUNCE * 0.7) * closing / (1. / mr + 1. / mf);
                // Put the pair back apart along the road, the lighter giving the most.
                let r = cars.get_mut(&rear).unwrap();
                r.d -= along * (mf / (mr + mf));
                r.v = (r.v - j / mr).max(0.);
                if slam { r.impact = r.impact.max(10); }
                let f = cars.get_mut(&front).unwrap();
                f.d += along * (mr / (mr + mf));
                f.v += j / mf * 0.7;
                if slam { f.impact = f.impact.max(10); }
                if slam {
                    // The leader's engines take the shove through the tethers; the rear pod's nose takes the rest.
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

/// Ticks a slam keeps its marker up (the HUD flash and the shake).
pub const SLAM_TICKS: u32 = 24;
/// What a kick sheds each tick.
pub const KICK_DECAY: f32 = 0.88;

/// The kick, spent: lateral metres this tick, and the new kick.
pub fn kick_step(kick: f32) -> (f32, f32) { (kick / HZ, kick * KICK_DECAY) }
