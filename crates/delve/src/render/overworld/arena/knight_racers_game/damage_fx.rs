//! Damage you can see. A worn or hot engine smokes, a burning one throws flame and sparks,
//! a blown one bursts, and a pod sitting out its blowout steams while the squires patch it.
//!
//! This file is where the marks *are*: for a car and a tick it gives the puffs that hang off its
//! engines (`puffs`, in the world, with a size, an age and a kind). Drawing them is the feel kit's
//! (`fx::damage`): every puff is a lumpy cluster of dots laid on the picture through the 4x4
//! dither, tinted from what is already there, so the road and the pods show through, and it thins
//! as it rises and ages. They used to be fans of lit mesh triangles, which could not be thin: a pale
//! flat hexagon in a warm tunnel read as a boulder.
//! All of it is a function of the race tick and the car (`rules::Car`: `heat`, `wear`, `crash`,
//! `blown_side`), never of the wall clock, so every seat sees the same smoke.
use super::*;
use super::pods::{look_of, pod, SCALE};

const LIFE: f32 = 40.;
const PUFFS: u32 = 12;
const GONE: f32 = 6.;
const FULL: f32 = 18.;
const TRAIL: f32 = 9.;

#[derive(Clone, Copy, PartialEq, Debug)]
pub enum Stuff {
    Smoke,
    Steam,
    Flame,
    Spark,
    Shard(u8),
}

#[derive(Clone, Copy, Debug)]
pub struct Puff {
    pub at: V3,
    pub r: f32,
    pub age: f32,
    pub stuff: Stuff,
    pub weight: f32,
}

fn roll(a: u32, b: u32, c: u32) -> f32 {
    let mut x = a.wrapping_mul(0x9E37_79B1) ^ b.wrapping_mul(0x85EB_CA6B) ^ c.wrapping_mul(0xC2B2_AE35) ^ 0x1B87_3593;
    x ^= x >> 15; x = x.wrapping_mul(0x2C1B_3C6D); x ^= x >> 12; x = x.wrapping_mul(0x297A_2D39); x ^= x >> 15;
    (x >> 8) as f32 / (1u32 << 24) as f32
}

const PUFF_GONE: f32 = 3.;
const PUFF_FULL: f32 = 11.;

pub fn near_lens(p: V3, eye: V3, r: f32) -> f32 {
    let dist = (eye - p).length();
    let near = ((dist - PUFF_GONE) / (PUFF_FULL - PUFF_GONE)).clamp(0., 1.);
    r * near * near * (3. - 2. * near)
}

pub fn puffs(race: &rules::Session, seat: u32, c: &rules::Car, eye: V3) -> Vec<Puff> {
    let mut out = Vec::new();
    let sick = c.heat.iter().any(|h| *h > 0.42) || c.wear.iter().any(|w| *w > 0.12) || c.blown();
    if !sick || c.finish.is_some() { return out; }
    let p = pod(look_of(seat, c));
    let lap = race.course().lap();
    let o = point(c.d, c.y, c.lift.max(-30.));
    let dist = (o - eye).length();
    let k = ((dist - GONE) / (FULL - GONE)).clamp(0., 1.);
    let scale = SCALE * k * k * (3. - 2. * k);
    if scale < 0.04 { return out; }
    let (yaw_s, yaw_c) = (pose(c.d).2 + c.yaw - c.vy * 0.012).sin_cos();
    let road = race.course().frame(c.route, c.d.rem_euclid(lap));
    let (roll_s, roll_c) = (-road.bank - c.vy * 0.018).clamp(-0.5, 0.5).sin_cos();
    let (pitch_s, pitch_c) = road.g.atan().sin_cos();
    let rotate = |q: V3| {
        let (x, z) = (q.x * pitch_c - q.z * pitch_s, q.x * pitch_s + q.z * pitch_c);
        let (y, z) = (q.y * roll_c - z * roll_s, q.y * roll_s + z * roll_c);
        v3(x * yaw_c - y * yaw_s, x * yaw_s + y * yaw_c, z)
    };
    let t = race.ticks as f32;
    let back = rotate(v3(-1., 0., 0.));
    let into_trail = (c.v / 40.).clamp(0.15, 1.) * TRAIL;
    for i in 0..2usize {
        let side = if i == 0 { -1. } else { 1. };
        let tail = o + rotate(v3(-p.len * 0.5, side * p.gap, p.z + 0.1)) * scale;
        let (heat, wear) = (c.heat[i], c.wear[i]);
        let blowing = c.blown() && usize::from(c.blown_side) == i + 1;
        let age_blow = (rules::race::engines::BLOWOUT_TICKS.saturating_sub(c.crash)) as f32;
        let mut smoke = (((heat - 0.42) / 0.33).clamp(0., 1.)).max(((wear - 0.12) / 0.3).clamp(0., 1.) * 0.7);
        let fire = ((heat - 0.72) / 0.28).clamp(0., 1.);
        if blowing { smoke = smoke.max(1. - age_blow / rules::race::engines::BLOWOUT_TICKS as f32 * 0.6); }
        let n = (smoke * PUFFS as f32).ceil() as u32;
        for j in 0..n {
            let age = (t / LIFE + j as f32 / PUFFS as f32 + roll(seat, i as u32, 3)).fract();
            let jx = roll(seat + j, i as u32, 11) - 0.5;
            let jy = roll(seat + j, i as u32, 12) - 0.5;
            let sit = if blowing { 1. } else { 0. };
            let pos = tail + back * (into_trail * age * (1. - 0.8 * sit)) + v3(jx * 0.9 * age, jy * 0.9 * age, (1.2 + 2.6 * sit) * age * scale);
            let r = (0.3 + 1.0 * age) * (0.6 + 0.4 * smoke) * scale;
            let r = near_lens(pos, eye, r);
            if r < 0.04 { continue; }
            out.push(Puff { at: pos, r, age, stuff: if blowing && age > 0.35 { Stuff::Steam } else { Stuff::Smoke }, weight: smoke });
        }
        if fire > 0. && !blowing {
            let flick = |s: f32| 0.75 + 0.5 * (t * 0.9 + s).sin().abs();
            for q in 0..2 {
                let len = (1.1 + 2.4 * fire) * flick(q as f32 * 2.1 + i as f32) * scale;
                let lift = rotate(v3(0., (q as f32 - 0.5) * 0.35, 0.18 * q as f32)) * scale;
                let base = tail + lift;
                for b in 0..5 {
                    let u = b as f32 / 4.;
                    let at = base + back * (len * u) + v3(0., 0., 0.3 * scale * u * u);
                    let r = near_lens(at, eye, 0.36 * (0.6 + 0.4 * fire) * scale * (1. - 0.7 * u));
                    if r < 0.04 { continue; }
                    out.push(Puff { at, r, age: u, stuff: Stuff::Flame, weight: fire });
                }
            }
            for q in 0..3u32 {
                let age = (t / 14. + q as f32 / 3. + roll(seat, i as u32, 21 + q)).fract();
                let pos = tail + back * (age * 5.5 * scale) + v3((roll(seat, q, 31) - 0.5) * 1.6 * age, (roll(seat, q, 32) - 0.5) * 1.6 * age, (age * (1. - age) * 3.2 + 0.2) * scale);
                let r = near_lens(pos, eye, 0.2 * (1. - age * 0.6) * scale);
                if r < 0.04 { continue; }
                out.push(Puff { at: pos, r, age, stuff: Stuff::Spark, weight: fire });
            }
        }
        if blowing && age_blow < 18. {
            let u = age_blow / 18.;
            for q in 0..14u32 {
                let (a, b) = (roll(seat, q, 41) * TAU, roll(seat, q, 42) * 1.4 - 0.2);
                let dir = v3(a.cos() * b.cos(), a.sin() * b.cos(), 0.35 + b.sin().abs());
                let pos = tail + dir * (age_blow * 0.34 * (0.5 + roll(seat, q, 43)) * scale) + v3(0., 0., 0.4 * scale);
                let r = near_lens(pos, eye, (0.7 * (1. - u) + 0.2) * (0.55 + roll(seat, q, 44) * 0.6) * scale);
                if r < 0.04 { continue; }
                out.push(Puff { at: pos, r, age: u, stuff: Stuff::Shard((q % 3) as u8), weight: 1. });
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn staged(heat: [f32; 2], wear: [f32; 2], crash: u32, ticks: u32) -> (rules::Session, rules::Car) {
        let mut race = rules::Session::blank_on(rules::cup()[0]);
        race.join_ids(1..=6);
        race.ticks = rules::race::COUNTDOWN + ticks;
        let mut car = race.cars[&1].clone();
        car.d = 300.; car.y = 0.; car.v = 44.;
        car.heat = heat; car.wear = wear; car.crash = crash; car.blown_side = if crash > 0 { 1 } else { 0 };
        (race, car)
    }

    fn fx(race: &rules::Session, car: &rules::Car) -> Vec<Puff> {
        let eye = point(car.d - 24., car.y * 0.4, 6.);
        puffs(race, 1, car, eye)
    }

    #[test]
    fn a_sound_pod_throws_nothing_and_a_hurt_one_does() {
        let (race, car) = staged([0.1, 0.1], [0.; 2], 0, 200);
        assert!(fx(&race, &car).is_empty());
        let (race, car) = staged([0.5, 0.1], [0.; 2], 0, 200);
        let smoke = fx(&race, &car).len();
        assert!(smoke > 0, "a warm engine smokes");
        let (race, car) = staged([0.95, 0.1], [0.; 2], 0, 200);
        let burning = fx(&race, &car);
        assert!(burning.len() > smoke, "a burning one throws more: smoke, flame and sparks");
        assert!(burning.iter().any(|p| p.stuff == Stuff::Flame) && burning.iter().any(|p| p.stuff == Stuff::Spark));
        let (race, car) = staged([0.2, 0.2], [0.3, 0.], 0, 200);
        assert!(!fx(&race, &car).is_empty(), "a worn engine smokes even when cool");
    }

    #[test]
    fn the_smoke_comes_off_the_engine_that_is_hurt() {
        let side = |heat: [f32; 2]| {
            let (race, car) = staged(heat, [0.; 2], 0, 200);
            let m = fx(&race, &car);
            m.iter().map(|p| p.at.y).sum::<f32>() / m.len() as f32 - point(car.d, car.y, 0.).y
        };
        let (a, b) = (side([0.7, 0.1]), side([0.1, 0.7]));
        assert!((a - b).abs() > 1., "the two engines smoke from different sides: {a} {b}");
    }

    #[test]
    fn a_blowout_bursts_then_steams_and_everything_is_finite_and_repeatable() {
        let fresh = staged([1., 0.3], [0.; 2], rules::race::engines::BLOWOUT_TICKS - 3, 200);
        let later = staged([0.4, 0.3], [0.; 2], 50, 200);
        let (a, b) = (fx(&fresh.0, &fresh.1), fx(&later.0, &later.1));
        assert!(a.len() > b.len() && !b.is_empty(), "a burst of {} then steam of {}", a.len(), b.len());
        assert!(a.iter().any(|p| matches!(p.stuff, Stuff::Shard(_))) && !b.iter().any(|p| matches!(p.stuff, Stuff::Shard(_))));
        assert!(b.iter().any(|p| p.stuff == Stuff::Steam), "a pod sitting out its blowout steams");
        for p in a.iter().chain(&b) { assert!(p.at.x.is_finite() && p.at.y.is_finite() && p.at.z.is_finite() && p.r > 0. && (0. ..=1.).contains(&p.age)); }
        let again = fx(&fresh.0, &fresh.1);
        assert_eq!(a.len(), again.len());
        assert!(a.iter().zip(&again).all(|(x, y)| x.at == y.at), "the same tick draws the same smoke");
    }

    #[test]
    fn a_puff_shrinks_away_as_it_nears_the_lens() {
        let size = |dist: f32| near_lens(point(300., 0., 2.), point(300. - dist, 0., 2.), 1.0);
        assert_eq!(size(2.), 0., "a puff at the lens is gone");
        assert!(size(6.) > 0. && size(6.) < size(20.) * 0.7, "a puff 6 m off is small: {} against {}", size(6.), size(20.));
        assert!((size(12.) - size(30.)).abs() < 1e-3, "and full size from 11 m");
    }

    #[test]
    fn effects_shrink_away_with_a_pod_at_the_lens() {
        let (race, car) = staged([0.95, 0.1], [0.; 2], 0, 200);
        assert!(puffs(&race, 1, &car, point(car.d, car.y, 3.)).is_empty());
    }
}
