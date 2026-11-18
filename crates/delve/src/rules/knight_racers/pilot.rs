//! The rival pilots. A pilot reads the whole field, not just its own pod: it takes a racing
//! line (wide into a bend, in to the apex, wide out), steps round the road's hazards, tucks into
//! a pod's wake to build a draft, pulls out and boosts past it on a straight, covers its own line
//! from a pod that is trying the same on it, lifts to cool a hot engine, spends its boost where it
//! pays and makes the odd mistake. Each pod has a character (`persona`): Lancewain is brave and
//! brittle, Gatehouse is a blocker, Bellfounder burns its boost, Ringwright is smooth.
//!
//! A pilot is a pure function of the race: the same field and tick make the same hands, so a race
//! replays exactly. Nothing is remembered between ticks; the choices that need a memory (which
//! side to pass on, whether a pass is on) read it off where the pod already is.
use super::*;
use super::contact::{ahead, nose, tail};
use super::engines::FIRE_HEAT;
use std::collections::BTreeMap;

pub const CPU_SLOP: f32 = 1.;

#[derive(Clone, Copy, Debug)]
pub struct Persona {
    pub brave: f32,
    pub block: f32,
    pub boost: f32,
    pub steady: f32,
    pub caution: f32,
}

pub fn persona(pod: u8) -> Persona {
    match pod % 6 {
        0 => Persona { brave: 0.50, block: 0.40, boost: 0.55, steady: 0.80, caution: 0.60 },
        1 => Persona { brave: 0.50, block: 0.30, boost: 0.95, steady: 0.60, caution: 0.45 },
        2 => Persona { brave: 0.35, block: 0.50, boost: 0.45, steady: 0.95, caution: 0.80 },
        3 => Persona { brave: 1.00, block: 0.10, boost: 0.75, steady: 0.35, caution: 0.35 },
        4 => Persona { brave: 0.55, block: 1.00, boost: 0.35, steady: 0.85, caution: 0.70 },
        _ => Persona { brave: 0.70, block: 0.25, boost: 0.60, steady: 0.55, caution: 0.55 },
    }
}

fn smooth(a: f32, b: f32, x: f32) -> f32 { let t = ((x - a) / (b - a)).clamp(0., 1.); t * t * (3. - 2. * t) }

fn roll(a: u32, b: u32, c: u32) -> f32 {
    let mut x = a.wrapping_mul(0x9E37_79B1) ^ b.wrapping_mul(0x85EB_CA6B) ^ c.wrapping_mul(0xC2B2_AE35) ^ 0x27D4_EB2F;
    x ^= x >> 15; x = x.wrapping_mul(0x2C1B_3C6D); x ^= x >> 12; x = x.wrapping_mul(0x297A_2D39); x ^= x >> 15;
    (x >> 8) as f32 / (1u32 << 24) as f32
}

#[derive(Clone, Copy)]
struct Near<'a> { id: u32, car: &'a Car, dy: f32, gap: f32 }

#[derive(Clone, Copy, PartialEq)]
enum Slip { None, LateBrake, Wide(f32), Twitch, Lift, HotBoost }

fn slip(id: u32, course: &Course, ticks: u32, p: &Persona, slop: f32) -> Slip {
    let window = (ticks.saturating_sub(COUNTDOWN)) / 40;
    let rate = knob("SLIP", 0.011) * slop * (1.7 - p.steady);
    let key = course.lap() as u32;
    if roll(id, window, key) >= rate { return Slip::None; }
    match (roll(id, window, key ^ 0x55) * 5.) as u32 {
        0 => Slip::LateBrake,
        1 => Slip::Wide(if roll(id, window, key ^ 0x99) < 0.5 { 3.2 } else { -3.2 }),
        2 => Slip::Twitch,
        3 => Slip::Lift,
        _ => Slip::HotBoost,
    }
}

fn launch(id: u32, car: &Car, ticks: u32, slop: f32) -> Drive {
    let p = persona(car.pod);
    let early = roll(id, 1, 0xA1) < 0.12 * slop * (1.6 - p.steady);
    let lead = if early { LAUNCH_WINDOW + 4 + (roll(id, 2, 0xA1) * 8.) as u32 } else { 2 + (roll(id, 2, 0xA1) * (LAUNCH_WINDOW - 3) as f32) as u32 };
    Drive { gas: ticks + lead >= COUNTDOWN, ..Drive::default() }
}

pub fn slipping(course: &Course, id: u32, pod: u8, ticks: u32, slop: f32) -> bool {
    slip(id, course, ticks, &persona(pod), slop) != Slip::None
}

pub fn drive(course: &Course, cars: &BTreeMap<u32, Car>, id: u32, ticks: u32, slop: f32) -> Drive {
    let Some(car) = cars.get(&id) else { return Drive::default() };
    if car.finish.is_some() || car.blown() { return Drive::default(); }
    if ticks <= COUNTDOWN { return launch(id, car, ticks, slop); }
    let lap = course.lap();
    let p = persona(car.pod);
    let spec = car.spec();
    let s = car.d.rem_euclid(lap);
    let fr = course.frame(car.route, car.d);
    let hw = spec.half_w;
    let mistake = slip(id, course, ticks, &p, slop);

    let mut lead: Option<Near> = None;
    let mut chaser: Option<Near> = None;
    let mut beside: Vec<Near> = Vec::new();
    let mut human_best = f32::MIN;
    let mut field_best = f32::MIN;
    for (&o, oc) in cars {
        if oc.finish.is_none() { field_best = field_best.max(oc.d); if !oc.cpu { human_best = human_best.max(oc.d); } }
        if o == id || oc.route != car.route || oc.finish.is_some() { continue; }
        let fg = ahead(lap, car.d, oc.d);
        let dy = oc.y - car.y;
        let gap = if fg >= 0. { fg - nose(car) - tail(oc) } else { -fg - nose(oc) - tail(car) };
        let near = Near { id: o, car: oc, dy, gap };
        let lane = hw + oc.spec().half_w + 3.5;
        if gap < 1.5 && fg.abs() < 14. && dy.abs() < lane { beside.push(near); }
        if fg > 0. && gap < 45. && dy.abs() < lane && lead.is_none_or(|l| gap < l.gap) { lead = Some(near); }
        if fg < 0. && gap < 26. && dy.abs() < lane && chaser.is_none_or(|c| gap < c.gap) { chaser = Some(near); }
    }
    let reference = if human_best > f32::MIN && !car.cpu { car.d } else if human_best > f32::MIN { human_best } else { field_best };
    let rb = ((reference - car.d) / 320.).clamp(-1., 1.);

    let at = |x: f32| course.frame(car.route, car.d + x);
    let (f1, f2, f3, f4) = (at(14.), at(34.), at(62.), at(95.));
    let reach = at(26.);
    let lim_l = (fr.wl.min(reach.wl) - hw * 0.75 - 0.5).max(0.8);
    let lim_r = (fr.wr.min(reach.wr) - hw * 0.75 - 0.5).max(0.8);
    let band = |y: f32| y.clamp(-lim_r, lim_l);
    let inside = |k: f32| if k >= 0. { lim_l * 0.85 } else { -lim_r * 0.85 };
    let near_k = if f1.k.abs() > fr.k.abs() { f1.k } else { fr.k };
    let far_k = if f3.k.abs() > f4.k.abs() { f3.k } else { f4.k };
    let (s_near, s_far) = ((near_k.abs() * 240.).min(1.), (far_k.abs() * 240.).min(1.));
    let w_in = smooth(0.12, 0.6, s_near);
    let w_out = smooth(0.12, 0.6, s_far) * (1. - w_in);
    let spread = [0., 1.5, -1.3, 2.1, -1.9, 0.7][(id as usize - 1) % 6] * (f2.wl.min(f2.wr) / 12.).min(1.);
    let lane = spread * (1. - w_in) * (1. - w_out);
    let mut target = lane + w_in * inside(near_k) * 0.8 + w_out * -inside(far_k) * 0.55;
    for (i, f) in course.forks().iter().enumerate() {
        let to_gore = (f.s0 - s).rem_euclid(lap);
        if car.route == 0 && to_gore < 90. {
            let main = course.frame(0, f.s0);
            let m = main.wl.min(main.wr) * 0.55;
            target = if fork_taken(id, i, f) { f.side * m } else { -f.side * m };
        }
        if car.route as usize == i + 1 {
            let to_join = (f.s1 - s).rem_euclid(lap);
            if to_join < 40. { target = (target - f.side * f.off1).clamp(-2., 2.); }
        }
    }
    for h in hazards_on(course, car.route) {
        let gap = (h.s - s + lap * 0.5).rem_euclid(lap) - lap * 0.5;
        if gap > -(h.half_l + 3.2) && gap < 26. + car.v * 0.4 && (h.period == 0 || h.on * 2 > h.period || h.kind == HazardKind::Jet || h.kind == HazardKind::Rockfall) {
            let clear = h.half_w + 1.05 + 0.45;
            if (target - h.y).abs() < clear || (car.y - h.y).abs() < clear + 0.6 && gap < 14. + car.v * 0.2 {
                let (l, r) = (h.y + clear, h.y - clear);
                let (l_ok, r_ok) = (l <= lim_l, r >= -lim_r);
                let mine = if car.y >= h.y { l } else { r };
                target = match (l_ok, r_ok) {
                    (true, true) if (car.y - h.y).abs() < clear => mine,
                    (true, true) => if (l - target).abs() < (r - target).abs() { l } else { r },
                    (true, false) => l, (false, true) => r, _ => target,
                };
            }
        }
    }

    let k_ahead = [fr.k, f1.k, f2.k, f3.k].iter().fold(0f32, |m, k| m.max(k.abs()));
    let straight = k_ahead < 0.008;
    let clear_of = 0.9 - 0.7 * p.brave;
    let mut attacking = false;
    let mut defending = false;
    let mut boost_help = false;
    if let Some(l) = lead {
        let need = hw + l.car.spec().half_w + clear_of;
        let closing = (car.v - l.car.v).max(0.);
        let stopped = l.car.v < 12.;
        let react = 12. + closing * 0.6 + if stopped { 10. } else { 0. };
        let in_lane = l.dy.abs() < need + 0.4;
        if in_lane || l.dy.abs() < need + 2. {
            let out_of_it = l.dy.abs() > 1.5;
            let pass_on = straight || p.brave > 0.65 || stopped || k_ahead < 0.012 && out_of_it;
            if l.gap > react && l.gap < 30. && straight && l.car.v > 24. && !stopped {
                target = band(l.car.y + 0.2 * (target - l.car.y).signum());
                attacking = true;
            } else if l.gap <= react.max(14.) && pass_on || out_of_it && l.gap < 24. && pass_on {
                let room_l = lim_l - (l.car.y + need);
                let room_r = (l.car.y - need) + lim_r;
                let want = if l.dy.abs() > 1.2 { -l.dy.signum() } else if room_l > 0. && room_r > 0. {
                    if near_k > 0.004 { 1. } else if near_k < -0.004 { -1. } else if room_l >= room_r { 1. } else { -1. }
                } else if room_l > 0. { 1. } else if room_r > 0. { -1. } else { 0. };
                let taken = |side: f32| beside.iter().any(|b| ((l.car.y + side * (need + 0.4)) - b.car.y).abs() < need);
                let want = if want != 0. && taken(want) && !taken(-want) { -want } else { want };
                if want != 0. {
                    target = band(l.car.y + want * (need + 0.4));
                    attacking = true;
                    boost_help = l.gap < 22. && !stopped || stopped;
                }
            }
        }
    }
    if let Some(c) = chaser {
        if straight && c.gap < 16. && c.dy.abs() < 3.2 && p.block > 0.35 && !attacking {
            let w = (p.block - 0.2).clamp(0., 0.9);
            target = band(target + (c.car.y - target) * w);
            defending = true;
        }
    }
    for b in &beside {
        let need = hw + b.car.spec().half_w + clear_of;
        if b.dy.abs() < need {
            let away = if b.dy.abs() < 0.3 { if id < b.id { -1. } else { 1. } } else { -b.dy.signum() };
            let yield_to = band(b.car.y + away * (need + 0.15));
            if (target - b.car.y).abs() < need && !(p.brave > 0.8 && b.dy.abs() > need * 0.6) { target = yield_to; }
        }
    }
    if let Slip::Wide(w) = mistake { target += w; }
    let target = band(target);

    let err = target - car.y - car.vy * 0.07;
    let dead = if defending { 0.8 } else { 0.4 };
    let mut steer = if err > dead { 1 } else if err < -dead { -1 } else { 0 };
    if mistake == Slip::Twitch && ticks % 8 < 4 { steer = -steer; }
    let mut squeezed = false;
    for b in &beside {
        let need = hw + b.car.spec().half_w + 0.35 + 0.9 * (1. - p.brave);
        if b.dy.abs() < need + 1.0 + car.vy.abs() * 0.12 {
            if (steer > 0 && b.dy > 0.) || (steer < 0 && b.dy < 0.) { steer = 0; }
            if b.dy.abs() < need && p.brave < 0.95 {
                let away = if b.dy > 0. { -1 } else { 1 };
                let room = if away > 0 { lim_l - car.y } else { car.y + lim_r };
                if room > 0.6 { steer = away; } else { squeezed = true; }
            }
        }
    }

    let kk = (0..3).map(|n| course.frame(car.route, car.d + 10. + car.v * (0.15 + 0.1 * n as f32)).k.abs()).fold(0., f32::max);
    let near_gap = course.gaps().iter().any(|g| (g.s0 - s).rem_euclid(lap) < 150.);
    let bold = (p.brave * (0.5 + 0.5 * rb.max(0.))).clamp(0., 1.);
    let thr = 0.018 * (1. + knob("BRAVE_BRAKE", 0.45) * bold);
    let mut brake = kk > thr && car.v > 30. && !near_gap && mistake != Slip::LateBrake;
    let mut gas = !brake;
    if let Some(l) = lead {
        let closing = car.v - l.car.v;
        if closing > 1.5 && l.dy.abs() < hw + l.car.spec().half_w + 0.6 {
            if l.gap < 2. + closing * 0.3 { gas = false; }
            if l.gap < 0.8 + closing * 0.15 { brake = true; }
        }
        if l.car.blown() && l.dy.abs() < hw + l.car.spec().half_w + 1.2 && l.gap < 5. + closing * 0.35 { gas = false; brake = l.gap < 3.; }
    }
    let hot = car.hottest();
    let lo = knob("LIFT_LO", 0.33) + knob("LIFT_CAUTION", 0.30) * (1. - p.caution) + 0.05 * rb.max(0.) + if attacking { 0.06 } else { 0. };
    let hi = lo + knob("LIFT_BAND", 0.20);
    if !near_gap || hot >= 0.9 {
        if hot >= FIRE_HEAT { gas = false; }
        else if hot > lo { let duty = 1. - (hot - lo) / (hi - lo); if roll(id, ticks, 7) > duty { gas = false; } }
    }
    if squeezed && car.v > 12. { gas = false; }
    if car.v < 9. { brake = false; }
    if car.v < 6. && hot < 0.95 && !brake { gas = true; }
    if mistake == Slip::Lift { gas = false; }
    let final_lap = car.lap(course) == course.laps();
    let reserve = knob("RESERVE", 0.22) + (1. - p.boost) * 0.22 - 0.10 * rb.max(0.) - if final_lap || attacking || defending { 0.15 } else { 0. };
    let cool_enough = hot < lo + 0.04 || (attacking || defending) && hot < FIRE_HEAT - 0.1 || mistake == Slip::HotBoost && hot < 0.9;
    let easing = rb < -0.5 && !attacking && !defending;
    let mut boost = !brake && gas && car.boost > reserve.max(0.04) && !car.boost_locked && car.v > 16. && cool_enough && !easing
        && ((straight && k_ahead < 0.009) || boost_help || (defending && chaser.is_some_and(|c| c.gap < 6.)));
    if near_gap && car.v < 48. && car.boost > 0.15 && !car.boost_locked { boost = true; }
    if hot >= FIRE_HEAT && !near_gap { boost = false; }
    if brake { boost = false; }
    Drive { steer, gas, brake, boost, interact: false }
}
