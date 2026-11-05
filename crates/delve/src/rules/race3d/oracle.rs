//! The Shaft Run's, the Ore Cart's and the Griffin Run's tick as it ran in Rust
//! before it moved into Bend: the oracle the leg is held to.
//!
//! This is the code that used to run in `race3d.rs` (`roll_ball`, `bump_balls`,
//! `ride_cart`, `bump_carts`, `fly_griffin`, `fly_bolts` and the course arms of
//! `tick_race3d`), with the three private helpers it leant on (`wreck`,
//! `rope_back`, `pick_up`) copied beside it, and one change: see `hyp`. It runs
//! only in tests: `crates/delve`'s own, through an engine that answers
//! from here, and the cockpit's parity test, which ticks the shipped leg and
//! this side by side and compares every bit of the state.
//!
//! It is included by `#[path]` from both trees, so it may only name what a
//! `super::` can supply: the race's types and course, and `Input` and `HZ`.
use super::{
    BALL_R, BOLT_LIFE, BOLT_PACE, BOLT_REST, BURST, Bolt, CRASH, Course, Ground, HZ, Input,
    ItemKind, LANES, ROLL, ROLL_REST, ROT, RUFFLE, Race3d, Rider, SKY_X, SKY_Z, TIMBER_H, TITLE,
    Track, Wreck, broken, crossing_carts, laid, rotten, track,
};
use serde::Deserialize;
use std::collections::BTreeMap;

const DT: f32 = 1.0 / HZ as f32;
const FALL_G: f32 = 9.0;
const PUSH: f32 = 3.2;
const BRAKE: f32 = 7.0;
const DRAG: f32 = 0.12;
const TOP: f32 = 11.0;
const STEER: f32 = 7.0;
const HOP: f32 = 4.6;
const HOP_G: f32 = 14.0;
const RUN_UP: f32 = 6.0;

struct Noise {
    sounds: Vec<&'static str>,
}

impl Noise {
    fn new() -> Noise {
        Noise { sounds: Vec::new() }
    }
}

/// The one change from the code that moved: a distance is the root of the sum
/// of the squares, each rounded as an f32, where the old code asked libm's
/// `hypot`. libm differs by platform and by an ulp from this, and the leg
/// cannot reproduce it; this is what the module computes, everywhere.
fn hyp(a: f32, b: f32) -> f32 {
    (a * a + b * b).sqrt()
}

fn wreck(rider: &mut Rider, what: Wreck, noise: &mut Noise) {
    rider.crash = CRASH;
    rider.wreck = what;
    rider.crashes = rider.crashes.saturating_add(1);
    rider.burst = 0;
    noise.sounds.push(match what {
        Wreck::Fall | Wreck::Gap => "knight_fall",
        Wreck::Bonk | Wreck::Timber => "helmet_pop",
        Wreck::Buffer => "clang",
        _ => "rock_land",
    });
}

fn rope_back(track: &Track, rider: &mut Rider, x: f32) {
    rider.s = (track.post(rider.s) - RUN_UP).max(1.0);
    (rider.x, rider.z, rider.vs, rider.vx, rider.vz) = (x, 0.0, 0.0, 0.0, 0.0);
    rider.wreck = Wreck::None;
}

fn pick_up(race: &mut Race3d, rider: &mut Rider, reach: f32, noise: &mut Noise) {
    let track = track(race.course);
    for (i, item) in track.items.iter().enumerate() {
        if item.kind != ItemKind::Gem || (item.s - rider.s).abs() > reach {
            continue;
        }
        let i = i as u16;
        if race.taken.contains(&i) {
            continue;
        }
        let dz = item.z - (rider.z + 0.5);
        if hyp(item.x - rider.x, dz) <= reach + 0.2 {
            race.taken.insert(i);
            rider.haul = rider.haul.saturating_add(1);
            noise.sounds.push("gold");
        }
    }
}

fn roll_ball(
    race: &mut Race3d,
    rider: &mut Rider,
    input: Input,
    pressed: (bool, bool),
    noise: &mut Noise,
) {
    let track = track(race.course);
    if rider.finish.is_some() {
        rider.vs *= 0.9;
        rider.s = (rider.s + rider.vs * DT).min(track.len() - 1.0);
        rider.spin += rider.vs * DT / BALL_R;
        return;
    }
    if rider.crash > 0 {
        rider.crash -= 1;
        if rider.wreck == Wreck::Fall {
            rider.vz -= HOP_G * DT;
            rider.z = (rider.z + rider.vz * DT).max(-14.0);
            rider.s += rider.vs * DT * 0.5;
            rider.x += rider.vx * DT * 0.5;
            rider.spin += 0.5;
        } else {
            rider.vs *= 0.85;
            rider.s = (rider.s + rider.vs * DT).max(0.5);
            rider.x += rider.vx * DT;
            rider.vx *= 0.85;
        }
        if rider.crash == 0 {
            if rider.wreck == Wreck::Fall {
                rope_back(track, rider, 0.0);
            }
            rider.wreck = Wreck::None;
        }
        return;
    }
    let at = track.at(rider.s);
    let grounded = rider.z <= 0.0;
    let mut a = FALL_G * at.slope;
    if rider.stun == 0 {
        if input.move_y < 0 {
            a += PUSH;
        }
        if input.move_y > 0 || input.interact {
            a -= BRAKE * rider.vs.signum();
        }
    }
    if at.ground == Ground::Sluice {
        a += 2.0;
        if race.t % 20 == 0 {
            noise.sounds.push("step_wet");
        }
    }
    let rail = 1.3;
    let mut ax = 0.0;
    if at.ground == Ground::Lodestone && grounded {
        let off = rail - rider.x;
        if off.abs() < 1.8 {
            ax += 3.2 * off.signum();
        }
        if off.abs() < 0.35 {
            a += 3.0;
        }
    }
    if at.ground == Ground::Sluice {
        ax += 4.0 * (rider.s * 0.35).sin();
    }
    a -= DRAG * rider.vs;
    rider.vs = (rider.vs + a * DT).clamp(-4.0, TOP);
    if rider.stun == 0 {
        ax += STEER * f32::from(input.move_x);
    }
    rider.stun = rider.stun.saturating_sub(1);
    ax -= rider.vs * rider.vs * at.bend;
    rider.vx = (rider.vx + ax * DT) * if grounded { 0.97 } else { 1.0 };
    if pressed.0 && grounded {
        rider.vz = HOP;
        noise.sounds.push("roll");
    }
    if !grounded || rider.vz > 0.0 {
        rider.vz -= HOP_G * DT;
        rider.z += rider.vz * DT;
        if rider.z <= 0.0 {
            rider.z = 0.0;
            rider.vz = 0.0;
            noise.sounds.push("wheel_land");
        }
    }
    rider.s += rider.vs * DT;
    rider.x += rider.vx * DT;
    rider.spin += rider.vs * DT / BALL_R;
    if rider.s < 0.5 {
        rider.s = 0.5;
        rider.vs = rider.vs.abs() * 0.4;
    }
    let at = track.at(rider.s);
    let half = at.width * 0.5;
    if at.walls {
        if rider.x.abs() > half - BALL_R {
            rider.x = rider.x.signum() * (half - BALL_R);
            rider.vx = if at.funnel { 0.0 } else { -rider.vx * 0.45 };
            rider.vs *= 0.94;
            if rider.vx.abs() > 1.5 {
                noise.sounds.push("clang");
            }
        }
    } else if rider.x.abs() > half && rider.z <= 0.05 {
        rider.vx = rider.x.signum() * 2.0;
        rider.vz = 0.0;
        wreck(rider, Wreck::Fall, noise);
        return;
    }
    if at.ground == Ground::Collapse {
        let plank = (rider.s / 2.0) as u16;
        if rotten(plank) {
            let began = *race.planks.entry(plank).or_insert(race.t);
            if race.t >= began + ROT && rider.z <= 0.05 {
                rider.vz = 0.0;
                wreck(rider, Wreck::Fall, noise);
                return;
            }
        }
    }
    rider.rest = rider.rest.saturating_sub(1);
    if at.ground == Ground::Crossing && rider.z < 1.0 && rider.rest == 0 {
        for (cs, cx) in crossing_carts(track, race.t) {
            if (cs - rider.s).abs() < 1.3 && (cx - rider.x).abs() < 1.6 {
                rider.vs = -3.0;
                rider.vx = if cx < rider.x { 5.0 } else { -5.0 };
                wreck(rider, Wreck::Bonk, noise);
                rider.crash = 20;
                rider.rest = ROLL_REST;
                return;
            }
        }
    }
    pick_up(race, rider, 0.9, noise);
}

fn bump_balls(race: &mut Race3d, noise: &mut Noise) {
    let ids: Vec<u32> = race.riders.keys().copied().collect();
    for (k, &a) in ids.iter().enumerate() {
        for &b in &ids[k + 1..] {
            let (ra, rb) = (&race.riders[&a], &race.riders[&b]);
            if !ra.racing() || !rb.racing() || (ra.z - rb.z).abs() > 0.8 {
                continue;
            }
            let (ds, dx) = (rb.s - ra.s, rb.x - ra.x);
            let d = hyp(ds, dx);
            if d >= 2.0 * BALL_R || d < 1e-4 {
                continue;
            }
            let (ns, nx) = (ds / d, dx / d);
            let push = (2.0 * BALL_R - d) * 0.5;
            let along = (rb.vs - ra.vs) * ns + (rb.vx - ra.vx) * nx;
            let mut ra = ra.clone();
            let mut rb = rb.clone();
            ra.s -= ns * push;
            ra.x -= nx * push;
            rb.s += ns * push;
            rb.x += nx * push;
            if along < 0.0 {
                let j = -along * 0.9;
                ra.vs -= j * ns;
                ra.vx -= j * nx;
                rb.vs += j * ns;
                rb.vx += j * nx;
                noise.sounds.push("clang");
            }
            race.riders.insert(a, ra);
            race.riders.insert(b, rb);
        }
    }
}

/// A cart's cruising pace, pumped and braked, how fast it gets there, the
/// jump, and how quick a cart crosses to another line.
const CRUISE: f32 = 8.5;
const PUMPED: f32 = 11.0;
const BRAKED: f32 = 4.0;
const SPUR: f32 = 4.0;
const JUMP: f32 = 5.6;
const JUMP_G: f32 = 15.0;
const SWITCH: f32 = 11.0;

fn ride_cart(
    race: &mut Race3d,
    rider: &mut Rider,
    input: Input,
    pressed: (bool, bool),
    noise: &mut Noise,
) {
    let track = track(race.course);
    if rider.finish.is_some() {
        rider.vs *= 0.92;
        rider.s = (rider.s + rider.vs * DT).min(track.len() - 1.0);
        rider.spin += rider.vs * DT;
        return;
    }
    if rider.crash > 0 {
        rider.crash -= 1;
        // The cart goes over and the knight goes up; then the rope.
        rider.vz -= JUMP_G * DT;
        rider.z = (rider.z + rider.vz * DT).max(if rider.wreck == Wreck::Gap {
            -12.0
        } else {
            0.0
        });
        rider.s += rider.vs * DT;
        rider.vs *= 0.9;
        rider.spin += 0.4;
        if rider.crash == 0 {
            let s = (track.post(rider.s) - RUN_UP).max(1.0);
            // Back on a line that is laid there.
            let lane = (0..3u8)
                .min_by_key(|&l| (!laid(track, s, l), (i32::from(l) - 1).abs()))
                .unwrap_or(1);
            rider.lane = lane;
            rope_back(track, rider, LANES[usize::from(lane)]);
            // Set back on the rails rolling.
            rider.vs = BRAKED;
        }
        return;
    }
    let at = track.at(rider.s);
    let grounded = rider.z <= 0.0 && rider.vz <= 0.0;
    let goal = if input.move_y < 0 {
        PUMPED
    } else if input.move_y > 0 {
        BRAKED
    } else {
        CRUISE
    };
    let pull = (goal - rider.vs).clamp(-SPUR * DT * 2.0, SPUR * DT);
    rider.vs = (rider.vs + pull + FALL_G * at.slope * DT * 0.5).clamp(0.0, PUMPED + 2.0);
    // Switch lines: toward the arrow held, or else to the nearest other
    // line laid ahead.
    if pressed.1 {
        let ahead = rider.s + 2.0;
        let dir = i32::from(input.move_x);
        let options: Vec<i32> = if dir != 0 { vec![dir] } else { vec![1, -1] };
        for d in options {
            let to = i32::from(rider.lane) + d;
            if (0..3).contains(&to) && laid(track, ahead, to as u8) {
                rider.lane = to as u8;
                noise.sounds.push("wheel_spin");
                break;
            }
        }
    }
    let target = LANES[usize::from(rider.lane)];
    let step = SWITCH * DT;
    rider.x += (target - rider.x).clamp(-step, step);
    if pressed.0 && grounded {
        rider.vz = JUMP;
        noise.sounds.push("roll");
    }
    if !grounded || rider.vz > 0.0 {
        rider.vz -= JUMP_G * DT;
        rider.z += rider.vz * DT;
        if rider.z <= 0.0 {
            rider.z = 0.0;
            rider.vz = 0.0;
            noise.sounds.push("wheel_land");
        }
    }
    let before = rider.s;
    rider.s += rider.vs * DT;
    rider.spin += rider.vs * DT;
    let on_rail = (rider.x - target).abs() < 0.3;
    // A line that ends at a buffer stop with the cart still on it.
    if !laid(track, rider.s, rider.lane) && laid(track, before, rider.lane) {
        rider.s = before;
        rider.vz = 4.0;
        wreck(rider, Wreck::Buffer, noise);
        return;
    }
    if rider.z <= 0.0 && on_rail && broken(track, rider.s, rider.lane) {
        rider.vz = 0.0;
        wreck(rider, Wreck::Gap, noise);
        return;
    }
    // A timber across the line: over it or into it.
    for item in &track.items {
        if item.kind == ItemKind::Timber
            && (item.x - rider.x).abs() < 1.0
            && before <= item.s
            && rider.s > item.s - 0.6
            && rider.z < TIMBER_H
        {
            rider.vz = 5.0;
            rider.vs *= 0.5;
            wreck(rider, Wreck::Timber, noise);
            return;
        }
    }
    pick_up(race, rider, 0.8, noise);
}

/// A cart that runs into the back of another on its line shunts it on and
/// stops dead itself.
fn bump_carts(race: &mut Race3d, noise: &mut Noise) {
    let ids: Vec<u32> = race.riders.keys().copied().collect();
    for (k, &a) in ids.iter().enumerate() {
        for &b in &ids[k + 1..] {
            let (ra, rb) = (&race.riders[&a], &race.riders[&b]);
            if !ra.racing()
                || !rb.racing()
                || (ra.x - rb.x).abs() > 0.8
                || (ra.z - rb.z).abs() > 0.8
            {
                continue;
            }
            if (ra.s - rb.s).abs() >= 1.4 {
                continue;
            }
            let (back, front) = if ra.s < rb.s { (a, b) } else { (b, a) };
            let mut rear = race.riders[&back].clone();
            let mut lead = race.riders[&front].clone();
            if rear.vs <= lead.vs {
                continue;
            }
            lead.vs = rear.vs + 1.0;
            rear.vs = (lead.vs - 4.0).max(1.0);
            rear.s = lead.s - 1.4;
            noise.sounds.push("clang");
            race.riders.insert(back, rear);
            race.riders.insert(front, lead);
        }
    }
}

/// The gorge's pace, a burst on top, a ruffled griffin's, how quick a
/// griffin answers the reins, and how fast a roll throws it aside.
const WING: f32 = 10.0;
const WING_BURST: f32 = 5.0;
const WING_RUFFLED: f32 = 5.0;
const REIN: f32 = 16.0;
const ROLL_THROW: f32 = 7.0;

fn fly_griffin(
    race: &mut Race3d,
    id: u32,
    rider: &mut Rider,
    input: Input,
    rolled: bool,
    noise: &mut Noise,
) {
    let track = track(race.course);
    rider.spin += 0.35;
    if rider.finish.is_some() {
        rider.vs = (rider.vs - 0.2).max(4.0);
        rider.s = (rider.s + rider.vs * DT).min(track.len() - 1.0);
        rider.z += (1.5 - rider.z) * 0.05;
        return;
    }
    if rider.crash > 0 {
        // Tumbling: feathers everywhere, sinking to the line, slow.
        rider.crash -= 1;
        rider.vs = (rider.vs - 0.6).max(3.0);
        rider.s += rider.vs * DT;
        rider.x *= 0.95;
        rider.z += (-1.0 - rider.z) * 0.08;
        rider.spin += 0.6;
        if rider.crash == 0 {
            rider.wreck = Wreck::None;
        }
        return;
    }
    let goal = WING + if rider.burst > 0 { WING_BURST } else { 0.0 }
        - if rider.stun > 0 { WING_RUFFLED } else { 0.0 };
    rider.vs += (goal - rider.vs).clamp(-0.5, 0.4);
    rider.burst = rider.burst.saturating_sub(1);
    rider.stun = rider.stun.saturating_sub(1);
    rider.rest = rider.rest.saturating_sub(1);
    rider.reload = rider.reload.saturating_sub(1);
    // The reins: across and up, with the weight of a big bird.
    let (wx, wz) = (f32::from(input.move_x), -f32::from(input.move_y));
    rider.vx += (wx * 5.0 - rider.vx) * (REIN * DT).min(1.0) * 0.5;
    rider.vz += (wz * 4.0 - rider.vz) * (REIN * DT).min(1.0) * 0.5;
    if rolled && rider.rest == 0 {
        rider.rolling = ROLL;
        rider.rest = ROLL_REST;
        rider.vx = if wx != 0.0 { wx } else { 1.0 } * ROLL_THROW;
        noise.sounds.push("swing");
    }
    rider.rolling = rider.rolling.saturating_sub(1);
    // Held, F looses a bolt every rest.
    if input.fire && rider.reload == 0 {
        rider.reload = BOLT_REST;
        race.bolts.push(Bolt {
            owner: id,
            s: rider.s + 1.0,
            x: rider.x,
            z: rider.z,
            life: BOLT_LIFE,
        });
        noise.sounds.push("shot_bolt");
    }
    let before = rider.s;
    rider.s += rider.vs * DT;
    rider.x = (rider.x + rider.vx * DT).clamp(-SKY_X, SKY_X);
    rider.z = (rider.z + rider.vz * DT).clamp(-SKY_Z, SKY_Z);
    // Rings, each once for each griffin.
    for (i, item) in track.items.iter().enumerate().take(64) {
        let bit = 1u64 << i;
        if item.kind == ItemKind::Ring
            && rider.rings & bit == 0
            && before <= item.s
            && rider.s > item.s
            && hyp(item.x - rider.x, item.z - rider.z) < 1.3
        {
            rider.rings |= bit;
            rider.haul = rider.haul.saturating_add(1);
            rider.burst = BURST;
            noise.sounds.push("gold");
        }
    }
    // Stone and gargoyles met head-on.
    let hit_block = track.blocks.iter().any(|b| {
        rider.s + 0.5 > b.s
            && before - 0.5 < b.s + b.len
            && rider.x + 0.6 > b.x.0
            && rider.x - 0.6 < b.x.1
            && rider.z + 0.4 > b.z.0
            && rider.z - 0.4 < b.z.1
    });
    let hit_goyle = track.items.iter().enumerate().any(|(i, item)| {
        item.kind == ItemKind::Gargoyle
            && !race.taken.contains(&(i as u16))
            && before - 0.5 <= item.s
            && rider.s + 0.5 > item.s
            && hyp(item.x - rider.x, item.z - rider.z) < 1.0
    });
    if hit_block || hit_goyle {
        // Off the stone the way it came: beside a spire, under a rampart.
        if let Some(b) = track
            .blocks
            .iter()
            .find(|b| rider.s + 0.5 > b.s && before - 0.5 < b.s + b.len)
        {
            if b.x.1 - b.x.0 > 2.0 * SKY_X {
                rider.z = if b.z.0 - 0.6 > -SKY_Z {
                    b.z.0 - 0.6
                } else {
                    (b.z.1 + 0.6).min(SKY_Z)
                };
            } else {
                let left = b.x.0 - 0.7;
                let right = b.x.1 + 0.7;
                rider.x = if (rider.x - left).abs() < (rider.x - right).abs() && left > -SKY_X {
                    left
                } else {
                    right.min(SKY_X)
                };
            }
        }
        rider.vs = 4.0;
        wreck(rider, Wreck::Tumble, noise);
    }
}

/// Bolts fly on down the gorge; a gargoyle hit shatters (a burst for the
/// shooter), a rival hit is ruffled.
fn fly_bolts(race: &mut Race3d, noise: &mut Noise) {
    let track = track(race.course);
    let mut bolts = std::mem::take(&mut race.bolts);
    bolts.retain_mut(|bolt| {
        let pace = race.riders.get(&bolt.owner).map_or(WING, |r| r.vs) + BOLT_PACE;
        let before = bolt.s;
        bolt.s += pace * DT;
        bolt.life = bolt.life.saturating_sub(1);
        for (i, item) in track.items.iter().enumerate() {
            let i = i as u16;
            if item.kind == ItemKind::Gargoyle
                && !race.taken.contains(&i)
                && before <= item.s + 0.5
                && bolt.s >= item.s - 0.5
                && hyp(item.x - bolt.x, item.z - bolt.z) < 1.3
            {
                race.taken.insert(i);
                if let Some(shooter) = race.riders.get_mut(&bolt.owner) {
                    shooter.haul = shooter.haul.saturating_add(1);
                    shooter.burst = BURST;
                }
                noise.sounds.push("rock_land");
                return false;
            }
        }
        for (&id, rival) in race.riders.iter_mut() {
            if id != bolt.owner
                && rival.racing()
                && rival.rolling == 0
                && before <= rival.s + 0.5
                && bolt.s >= rival.s - 0.5
                && hyp(rival.x - bolt.x, rival.z - bolt.z) < 1.1
            {
                rival.stun = RUFFLE;
                noise.sounds.push("hit_arrow");
                return false;
            }
        }
        bolt.life > 0 && bolt.s < track.len()
    });
    race.bolts = bolts;
}

/// The race's tick as it was for the three courses the leg answers: one rider
/// at a time, then what the field does to itself, then the finish line.
pub fn tick(
    race: &mut Race3d,
    inputs: &BTreeMap<u32, Input>,
    standing: &[u32],
) -> (bool, bool, Vec<&'static str>) {
    let mut noise = Noise::new();
    let ids: Vec<u32> = race.riders.keys().copied().collect();
    for &id in &ids {
        if !standing.contains(&id) {
            continue;
        }
        let input = inputs
            .get(&id)
            .copied()
            .filter(|i| i.valid() && standing.contains(&id))
            .unwrap_or_default();
        let mut rider = race.riders[&id].clone();
        let (f, e) = (input.fire, input.interact);
        let pressed = (f && rider.keys & 1 == 0, e && rider.keys & 2 == 0);
        rider.keys = u8::from(f) | u8::from(e) << 1;
        match race.course {
            Course::Shaft => roll_ball(race, &mut rider, input, pressed, &mut noise),
            Course::Cart => ride_cart(race, &mut rider, input, pressed, &mut noise),
            Course::Griffin => fly_griffin(race, id, &mut rider, input, pressed.1, &mut noise),
            Course::Touge => {}
        }
        if rider.finish.is_none() && rider.crash == 0 && rider.s >= race.finish_distance() {
            rider.finish = Some(race.t);
            noise.sounds.push("wheel_land");
        }
        race.riders.insert(id, rider);
    }
    match race.course {
        Course::Shaft => bump_balls(race, &mut noise),
        Course::Cart => bump_carts(race, &mut noise),
        Course::Griffin => fly_bolts(race, &mut noise),
        Course::Touge => {}
    }
    let all_home = !race.riders.is_empty() && race.riders.values().all(|r| r.finish.is_some());
    (all_home, race.t >= TITLE + race.time_limit(), noise.sounds)
}

/// What the leg is asked, as far as the oracle reads it.
#[derive(Deserialize)]
struct Ask {
    race: Race3d,
    inputs: BTreeMap<u32, Input>,
    standing: Vec<u32>,
}

/// The oracle's engine: the one op the leg answers, answered natively, in the
/// shape the leg answers it.
pub fn answer(id: &str, input: &serde_json::Value) -> Result<serde_json::Value, String> {
    if id != "race3d-rules" {
        return Err(format!("no oracle for {id}"));
    }
    let mut ask: Ask = serde_json::from_value(input.clone()).map_err(|e| e.to_string())?;
    if ask.race.course == Course::Touge {
        return Err("race3d-rules: the Touge's tick is not the leg's".into());
    }
    let (all_home, timed_out, sounds) = tick(&mut ask.race, &ask.inputs, &ask.standing);
    Ok(serde_json::json!({
        "riders": ask.race.riders,
        "taken": ask.race.taken,
        "planks": ask.race.planks,
        "bolts": ask.race.bolts,
        "sounds": sounds,
        "all_home": all_home,
        "timed_out": timed_out,
    }))
}
