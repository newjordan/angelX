//! The knight's blow, drawn (`rules::melee`): the arm in its sweep, the
//! slash's crescent running out past the blade to the stroke's full reach,
//! the sparks of contact, the floor cracks of a heavy blow, a bullet turned
//! off the blade with its ting and its word, and the bullet flying home.
//!
//! Every picture here is a pure function of `Run::feel.melee` and the
//! run's shots, so the host, a friend's mirror and a browser guest draw the
//! same pixels from the same tick. The crescent is the stroke's reach: what
//! it covers at its widest is what the stroke struck.
use super::*;
use super::super::ink::text_width;
use super::blades::{self, Arm};
use crate::rules::melee::{Hit, SWING_TICKS, Stroke, Swing, Tier};
use std::f32::consts::{FRAC_PI_2, PI, TAU};

fn line(cv: &mut Img, a: (i32, i32), b: (i32, i32), ink: char) {
    let n = (a.0 - b.0).abs().max((a.1 - b.1).abs()).max(1);
    for k in 0..=n {
        cv.put(a.0 + (b.0 - a.0) * k / n, a.1 + (b.1 - a.1) * k / n, ink);
    }
}

fn age(s: &Swing) -> u8 {
    SWING_TICKS.saturating_sub(s.ttl)
}

pub(super) fn step(run: &Run, id: u32) -> (f32, f32) {
    let Some(s) = run
        .feel
        .melee
        .swings
        .iter()
        .find(|s| s.owner == id && s.ttl > 0)
    else {
        return (0.0, 0.0);
    };
    // A visual step, not free movement or extra reach. Return on recovery.
    let n = match age(s) {
        0..=2 => 0.12,
        3..=4 => 0.16,
        5..=6 => 0.08,
        _ => 0.0,
    };
    let weight = if s.combo == 3 { 1.35 } else { 1.0 };
    (s.dx * n * weight, s.dy * n * weight)
}

pub(super) fn recoil(run: &Run, foe: u32) -> (f32, f32) {
    let Some(h) = run
        .feel
        .melee
        .hits
        .iter()
        .rev()
        .find(|h| h.foe == foe && matches!(h.tier, Tier::Heavy | Tier::Medium))
    else {
        return (0.0, 0.0);
    };
    // A heavy blow throws it back further; both shudder as they land.
    let heavy = if h.tier == Tier::Heavy { 1.0 } else { 0.6 };
    let n = match h.age() {
        0..=1 => 0.14,
        2..=3 => 0.2,
        4..=5 => 0.08,
        _ => 0.0,
    };
    let shudder = if h.age() <= 2 { [0.06, -0.06, 0.03][usize::from(h.age())] } else { 0.0 };
    (h.dx * n * heavy - h.dy * shudder, h.dy * n * heavy + h.dx * shudder)
}

pub(super) fn tracked(run: &Run, foe: u32) -> bool {
    run.feel
        .melee
        .hits
        .iter()
        .any(|h| h.foe == foe && h.ttl > 0)
}

/// A monster just struck solid flashes pale for the frame of contact.
pub(super) fn body(run: &Run, foe: u32, im: &Img) -> Img {
    if !run.feel.melee.hits.iter().any(|h| {
        h.foe == foe && matches!(h.tier, Tier::Heavy | Tier::Medium) && h.age() <= 1
    }) {
        return im.clone();
    }
    let mut out = im.clone();
    for y in 0..im.h {
        for x in 0..im.w {
            if im.get(x, y).is_some_and(|c| c != BLACK) {
                out.put(x, y, if (x + y) % 4 == 0 { 'v' } else { 'W' });
            }
        }
    }
    out
}

/// The inks of a crescent, brightest first: cold steel, or a finisher's
/// gold.
const STEEL: [char; 5] = ['W', 'V', 'z', 'Q', 'q'];
const GOLD: [char; 5] = ['W', 'T', 't', 'O', 'R'];
const STONE: [char; 5] = ['V', 'v', 'U', 'u', 'S'];

/// A sweep of the slash: about `centre`, starting at angle `from` and
/// turning `turn` radians (signed) over the whole stroke, the band between
/// `inner` and `outer` pixels out.
#[derive(Clone, Copy)]
struct Sweep {
    centre: (f32, f32),
    from: f32,
    turn: f32,
    inner: f32,
    outer: f32,
}

impl Sweep {
    fn angle(&self, k: f32) -> f32 {
        self.from + self.turn * k
    }
}

/// The crescent of `sweep` from `tail` to `head` (shares of the stroke):
/// thin at the tail, full and bright toward the head, its front rounded,
/// its inner edge breaking into air. `fade` (0 fresh, 1 gone) thins it.
fn crescent(cv: &mut Img, sweep: &Sweep, tail: f32, head: f32, fade: f32, inks: &[char; 5]) {
    let span = head - tail;
    if span <= 0.0 || fade >= 1.0 {
        return;
    }
    // Fading, the band thins to its bright rim and darkens, rather than
    // breaking into a sieve.
    let band = ((sweep.outer - sweep.inner) * (1.0 - fade * 0.75)).max(1.0);
    let n = (sweep.turn.abs() * span * sweep.outer * 1.8).ceil() as i32 + 2;
    for i in 0..=n {
        let u = i as f32 / n as f32;
        let theta = sweep.angle(tail + span * u);
        let body = u.powf(0.8);
        let cap = ((1.0 - u) / 0.1).min(1.0).sqrt();
        let thick = (band * body * cap).max(1.0);
        let (c, s) = (theta.cos(), theta.sin());
        for d in 0..thick.ceil() as i32 {
            let q = d as f32 / thick;
            let r = sweep.outer - d as f32;
            let (x, y) = ((sweep.centre.0 + c * r).round() as i32, (sweep.centre.1 + s * r).round() as i32);
            // The inner edge breaks into air.
            if q > 0.5 && bayer(x, y) < (q - 0.5) * 2.2 {
                continue;
            }
            if fade > 0.6 && bayer(x, y) < (fade - 0.6) * 2.0 {
                continue;
            }
            let light = (1.0 - q) * (0.3 + 0.7 * u) - fade * 0.4;
            let ink = if light > 0.7 {
                inks[0]
            } else if light > 0.5 {
                inks[1]
            } else if light > 0.34 {
                inks[2]
            } else if light > 0.2 {
                inks[3]
            } else {
                inks[4]
            };
            cv.put(x, y, ink);
        }
    }
}

/// A short bright streak along `theta` from `a` to `b` pixels out: the
/// blade's own blur where it passes.
fn streak(cv: &mut Img, centre: (f32, f32), theta: f32, a: f32, b: f32, ink: char) {
    let (c, s) = (theta.cos(), theta.sin());
    let n = (b - a).ceil().max(1.0) as i32;
    for k in 0..=n {
        let r = a + (b - a) * k as f32 / n as f32;
        cv.put((centre.0 + c * r).round() as i32, (centre.1 + s * r).round() as i32, ink);
    }
}

/// An angle the short way round, within a half turn of none.
fn wrap(a: f32) -> f32 {
    a.sin().atan2(a.cos())
}

/// How a stroke's slash stands at each tick of its life: how far the head
/// has come, how far the tail, how far out (a share of the way from the
/// blade to the full reach), and how faded. The first tick a seat sees is
/// the stroke's second (its first is the tick it struck), and that is the
/// slash at its widest: the frame a hold of the room's time stands on.
fn timeline(a: u8) -> Option<(f32, f32, f32, f32)> {
    Some(match a {
        0 => (0.6, 0.0, 0.6, 0.0),
        1 => (1.0, 0.0, 1.0, 0.0),
        2 => (1.0, 0.3, 1.05, 0.15),
        3 => (1.0, 0.6, 1.1, 0.45),
        4 => (1.0, 0.85, 1.14, 0.75),
        _ => return None,
    })
}

/// Where the blade itself is along its sweep at each tick: a little behind
/// the slash's head while it runs, then carried through.
fn steel_at(a: u8) -> f32 {
    match a {
        0 => 0.45,
        1 => 0.72,
        2 => 0.9,
        _ => 1.0,
    }
}

pub(super) fn swing(cv: &mut Img, run: &Run, id: u32) {
    let Some(s) = run
        .feel
        .melee
        .swings
        .iter()
        .find(|s| s.owner == id && s.ttl > 0)
    else {
        if let Some(h) = run.players.get(&id).filter(|h| h.champ.windup > 0) {
            let (x, y) = at(h.x, h.y);
            let pull = 3 + (crate::rules::champions::SLAM_WINDUP - h.champ.windup).min(8) as i32;
            line(cv, (x, y - 7), (x - 4, y - 17 - pull), 'u');
            cv.rect(x - 7, y - 19 - pull, 7, 4, 'V');
            line(cv, (x - 3, y - 16 - pull), (x - 3, y - 12 - pull), 'S');
        }
        return;
    };
    let a = age(s);
    let hero = run.players.get(&id);
    // The figure's own strike carries some arms; the rest are drawn here,
    // in the sweep.
    let arm = hero.map_or(Arm::of_stroke(s.stroke), Arm::of);
    let steel = !hero.is_some_and(blades::drawn_by_body);
    let way = s.dy.atan2(s.dx);
    let (ox, oy) = at(s.x, s.y);
    let origin = (ox as f32, oy as f32 - 7.0);
    let reach = s.reach * UNIT;
    let blade = crate::rules::melee::blade(s.reach) * UNIT;
    // The steel as drawn is the blade's visible part; the slash takes it
    // from there to the stroke's full reach.
    let held = (blade * 0.62).clamp(9.0, 17.0);
    let finish = s.combo == 3;
    match s.stroke {
        Stroke::Slam => slam(cv, s, a, (ox, oy - 7)),
        Stroke::Fist => fist(cv, s, a, (ox, oy - 7), reach),
        Stroke::Overhead => overhead(cv, s, a, origin, held, reach, arm, steel),
        Stroke::Thrust if s.arc <= 100.0 => thrust(cv, s, a, origin, held, reach, arm, steel),
        _ => {
            let Some((head, tail, out, fade)) = timeline(a) else {
                super::meleestrips::over_swing(cv, run, s, a, (ox, oy - 7), reach);
                return;
            };
            let arc = s.arc.to_radians();
            let ring = s.arc >= 300.0;
            let inks = if finish { &GOLD } else { &STEEL };
            let outer = held + 2.0 + (reach - held - 2.0) * out;
            // A band no wider than the stroke deserves: a finisher's the
            // fullest.
            let inner = (held * 0.82).max(outer - if finish { 12.0 } else { 9.0 });
            // Two axes come round together from either side; a lone blade
            // sweeps one way, the backhand the other.
            let signs: &[f32] = if s.stroke == Stroke::Chain && !ring {
                &[1.0, -1.0]
            } else if s.combo == 2 {
                &[-1.0]
            } else {
                &[1.0]
            };
            for (k, &sign) in signs.iter().enumerate() {
                let sweep = Sweep {
                    centre: origin,
                    from: if ring { way } else { way - sign * arc / 2.0 },
                    turn: if ring { sign * TAU } else { sign * arc },
                    inner,
                    outer: outer - k as f32 * 3.0,
                };
                crescent(cv, &sweep, tail, head, fade, inks);
                let theta = sweep.angle(steel_at(a));
                if steel && a <= 3 {
                    let hand = (origin.0 + theta.cos() * 2.0, origin.1 + theta.sin() * 2.0);
                    if a >= 1 && arm == Arm::Sword && signs.len() == 1 {
                        // Where the blade was a tick ago, dim.
                        let was = sweep.angle(steel_at(a - 1));
                        blades::draw(cv, arm, hand, (was.cos(), was.sin()), held, false);
                    }
                    blades::draw(cv, arm, hand, (theta.cos(), theta.sin()), held, true);
                }
                if ring && steel && a <= 3 {
                    // The second of the pair, opposite.
                    let back = theta + PI;
                    let hand = (origin.0 + back.cos() * 2.0, origin.1 + back.sin() * 2.0);
                    blades::draw(cv, arm, hand, (back.cos(), back.sin()), held, true);
                }
            }
            if finish && (1..=3).contains(&a) {
                // The finisher's glint, out on the slash's edge.
                let p = (origin.0 + way.cos() * outer, origin.1 + way.sin() * outer);
                let r = 4 - i32::from(a.saturating_sub(1));
                for (dx, dy) in [(1, 0), (-1, 0), (0, 1), (0, -1)] {
                    for k in 1..=r {
                        cv.put(p.0 as i32 + dx * k, p.1 as i32 + dy * k, if k == 1 { 'W' } else { 'T' });
                    }
                }
                cv.put(p.0 as i32, p.1 as i32, 'W');
            }
        }
    }
    super::meleestrips::over_swing(cv, run, s, a, (ox, oy - 7), reach);
}

/// The Hammerer's ground slam: the maul up and then down on the floor.
fn slam(cv: &mut Img, s: &Swing, a: u8, origin: (i32, i32)) {
    if a < 5 {
        let end = (origin.0 + (s.dx * 9.0) as i32, origin.1 + 7);
        line(cv, (origin.0, origin.1 - 11), end, 'u');
        line(cv, (origin.0 + 2, origin.1 - 9), (end.0 + 2, end.1), 'S');
        cv.rect(end.0 - 4, end.1 - 2, 8, 4, 'u');
        cv.rect(end.0 - 3, end.1 - 2, 6, 1, 'V');
    }
}

/// A punch: the fist out and back.
fn fist(cv: &mut Img, s: &Swing, a: u8, origin: (i32, i32), r: f32) {
    if a >= 7 {
        return;
    }
    let d = if a < 4 { 1.0 } else { 0.65 };
    let end = (origin.0 + (s.dx * r * d) as i32, origin.1 + (s.dy * r * d) as i32);
    if a <= 3 {
        for n in [-2, 2] {
            line(cv, (origin.0, origin.1 + n), (end.0, end.1 + n), 'S');
        }
    }
    line(cv, origin, end, if a >= 6 { 'u' } else { 'V' });
    cv.rect(end.0 - 2, end.1 - 2, 4, 3, 'T');
}

/// A heavy stroke from above: the arm comes over and down onto the floor
/// ahead, and the blow runs out across the ground in a crescent of dust and
/// grit to the stroke's full reach and width.
#[allow(clippy::too_many_arguments)]
fn overhead(cv: &mut Img, s: &Swing, a: u8, origin: (f32, f32), held: f32, reach: f32, arm: Arm, steel: bool) {
    let way = s.dy.atan2(s.dx);
    let side = if s.dx < 0.0 { -1.0 } else { 1.0 };
    // Over the top: from straight up, round to the way it was aimed.
    let up = -FRAC_PI_2;
    let turn = {
        let t = wrap(way - up);
        if t.abs() > 2.9 { side * PI } else { t }
    };
    let k = match a {
        0 => 0.15,
        1 => 0.55,
        _ => 1.0,
    };
    if a <= 3 {
        let chop = Sweep { centre: origin, from: up, turn, inner: held * 0.7, outer: held + 3.0 };
        crescent(cv, &chop, 0.0, k, if a == 3 { 0.5 } else { 0.0 }, &STEEL);
        if steel {
            let theta = chop.angle(k);
            let hand = (origin.0 + theta.cos() * 2.0, origin.1 + theta.sin() * 2.0);
            blades::draw(cv, arm, hand, (theta.cos(), theta.sin()), held, true);
        }
    }
    // The blow across the ground, from where the head came down.
    if (1..=6).contains(&a) {
        let feet = (origin.0, origin.1 + 7.0);
        let arc = s.arc.to_radians();
        let out = [0.0, 0.55, 0.85, 1.0, 1.06, 1.1, 1.13][usize::from(a)];
        let fade = [0.0, 0.0, 0.0, 0.1, 0.35, 0.6, 0.82][usize::from(a)];
        let outer = held + (reach - held) * out;
        // Two rings of the shock, the second a step behind the first.
        for (sign, back, band) in [(1.0, 0.0, 5.0), (-1.0, 6.0, 3.0)] {
            let r = (outer - back).max(held);
            let wave = Sweep { centre: feet, from: way - sign * arc / 2.0, turn: sign * arc, inner: r - band, outer: r };
            crescent(cv, &wave, 0.0, 1.0, fade, &STONE);
        }
        // Grit thrown up off the front of it.
        for g in 0..9 {
            let t = way + (g as f32 / 8.0 - 0.5) * arc;
            let r = outer + 2.0 + (hash(g, i32::from(a), s.owner) % 3) as f32;
            if bayer(g, i32::from(a)) >= fade {
                cv.put((feet.0 + t.cos() * r).round() as i32, (feet.1 + t.sin() * r).round() as i32 - i32::from(a), 'v');
            }
        }
    }
}

/// A thrust: the arm drives straight out along the aim, and its slash runs
/// on past the point as a narrow fan to the stroke's full reach.
#[allow(clippy::too_many_arguments)]
fn thrust(cv: &mut Img, s: &Swing, a: u8, origin: (f32, f32), held: f32, reach: f32, arm: Arm, steel: bool) {
    let way = s.dy.atan2(s.dx);
    let Some((_, _, out, fade)) = timeline(a) else { return };
    let arc = s.arc.to_radians().max(0.35);
    let drive = match a {
        0 => 0.5,
        1..=3 => 1.0,
        _ => 0.7,
    };
    let tip = held * drive + 3.0;
    let outer = tip + (reach - tip) * out;
    // The fan, swept across in an instant, from one side to the other.
    let fan = Sweep { centre: origin, from: way - arc / 2.0, turn: arc, inner: tip, outer };
    crescent(cv, &fan, 0.0, 1.0, fade, &STEEL);
    if a <= 3 {
        streak(cv, origin, way, held * 0.6, outer, 'W');
    }
    if steel && a <= 4 {
        let lead = (held * (drive - 0.6)).max(1.0);
        let hand = (origin.0 + way.cos() * lead, origin.1 + way.sin() * lead);
        blades::draw(cv, arm, hand, (s.dx, s.dy), held, a <= 3);
    }
}

pub(super) fn under(cv: &mut Img, run: &Run) {
    for (&id, hero) in &run.players {
        if hero.hp == 0 || hero.bonus.orbit == 0 {
            continue;
        }
        for back in (1..=4).rev() {
            for (x, y) in run.orbits_at(id, run.tick.saturating_sub(back)) {
                let (x, y) = at(x, y);
                cv.put(x, y - 3, if back > 2 { 'S' } else { 'u' });
                if back == 1 {
                    cv.put(x + 1, y - 3, 'u');
                }
            }
        }
    }
    for h in run.feel.melee.hits.iter().filter(|h| h.ttl > 0) {
        let (x, y) = at(h.x, h.y);
        let age = h.age();
        if h.crack {
            for branch in 0..5 {
                let theta = branch as f32 * 1.256 + h.dx.atan2(h.dy) * 0.3;
                let mut p = (x, y);
                for k in 1..=4 {
                    let bend = if (k + branch) % 2 == 0 { 0.18 } else { -0.18 };
                    let r = k as f32 * 4.0;
                    let q = (
                        x + ((theta + bend).cos() * r) as i32,
                        y + ((theta + bend).sin() * r * 0.65) as i32,
                    );
                    line(cv, p, q, if age < 6 { 's' } else { 'S' });
                    if age < 13 {
                        line(cv, (p.0, p.1 + 1), (q.0, q.1 + 1), 'u');
                    }
                    p = q;
                }
            }
        }
        // Directional dust and stone chips, without moving a hitbox.
        if !matches!(h.tier, Tier::Blocked | Tier::Turned) && age < 6 {
            for k in 0..4 {
                let t = age as f32;
                let dx = h.dx * (3.0 + t) + h.dy * (k as f32 - 1.5) * 2.0;
                let dy = h.dy * (3.0 + t) - h.dx * (k as f32 - 1.5) * 2.0;
                cv.rect(
                    x + dx as i32,
                    y + dy as i32,
                    if age < 3 { 2 } else { 1 },
                    1,
                    'u',
                );
            }
        }
    }
}

fn spark(cv: &mut Img, run: &Run, h: &Hit) {
    if h.tier == Tier::Turned {
        ting(cv, h);
        return;
    }
    let a = h.age();
    if a >= crate::rules::melee::HIT_TICKS {
        return;
    }
    let (x, y) = at(h.x, h.y);
    let y = y - 7;
    let strength = match h.tier {
        Tier::Light => 1,
        Tier::Medium => 2,
        Tier::Heavy => 3,
        Tier::Blocked | Tier::Turned => 2,
    };
    let way = h.dy.atan2(h.dx);
    if a <= 1 {
        cv.rect(x - 1, y - 1, 3, 3, 'V');
        cv.put(x, y, '$');
        if h.tier == Tier::Light {
            line(cv, (x - 3, y), (x + 3, y), 'u');
            line(cv, (x, y - 3), (x, y + 3), 'u');
            cv.put(x, y, '$');
        }
    }
    // A heavy blow throws a spray of steel-bright chips out the far side.
    if a <= 3 && strength == 3 {
        for k in 0..(strength * 2) {
            let t = way + (k as f32 - strength as f32 + 0.5) * 0.32;
            let r = 4.0 + a as f32 * 3.0 + (k % 2) as f32 * 2.0;
            let (px, py) = (x + (t.cos() * r) as i32, y + (t.sin() * r) as i32 + i32::from(a) / 2);
            cv.put(px, py, if a <= 1 { 'W' } else { 'v' });
            if strength == 3 && a <= 1 {
                cv.put(px - (t.cos() * 1.5) as i32, py - (t.sin() * 1.5) as i32, 'V');
            }
        }
    }
    // Thin rings lose segments as they dissolve; blocked contact is a half-ring.
    if a <= 4 && strength > 1 {
        for k in 0..24 {
            if a > 2 && k % 3 == 0 {
                continue;
            }
            let t = way + k as f32 * TAU / 24.0;
            if h.tier == Tier::Blocked && (t - way).cos() < 0.0 {
                continue;
            }
            let r = 3 + strength + a as i32;
            cv.put(
                x + (t.cos() * r as f32) as i32,
                y + (t.sin() * r as f32) as i32,
                'u',
            );
        }
    }
    let rays = if h.tier == Tier::Blocked {
        3
    } else {
        2 + strength * 2
    };
    for k in 0..rays {
        let theta = if h.tier == Tier::Blocked {
            way + PI + (k as f32 - 1.0) * 0.5
        } else {
            way + k as f32 * TAU / rays as f32 + 0.2
        };
        let inner = 2 + a as i32;
        let outer = (4 + strength * 2 - a as i32).max(inner);
        let point = |r: i32| {
            (
                x + (theta.cos() * r as f32) as i32,
                y + (theta.sin() * r as f32) as i32,
            )
        };
        line(
            cv,
            point(inner),
            point(outer),
            if a < 2 && k % 2 == 0 {
                '$'
            } else if a < 4 {
                'V'
            } else {
                'u'
            },
        );
        if strength == 3 && a < 3 {
            let (px, py) = point(outer + 2);
            cv.put(px + 1, py, 'u');
        }
    }
    let (hx, hy) = at(h.x, h.y);
    super::meleestrips::over_hit(cv, run, h, (hx, hy - 7));
}

/// The word a turned bullet calls.
const PARRY: &str = "PARRY!";

/// A bullet turned off the blade: a bright star where steel met it, a ring
/// opening out of it, and the word rising over it. A cue, so the signal's
/// own white and sea-glass.
fn ting(cv: &mut Img, h: &Hit) {
    let a = i32::from(h.age());
    let (x, y) = at(h.x, h.y);
    if a <= 2 {
        let r = 7 - a * 2;
        for k in -r..=r {
            let ink = if k.abs() <= 1 { 'w' } else if k.abs() <= r / 2 + 1 { '3' } else { '2' };
            cv.put(x + k, y, ink);
            cv.put(x, y + k, ink);
        }
        for k in 1..=(r / 2) {
            for (dx, dy) in [(1, 1), (-1, 1), (1, -1), (-1, -1)] {
                cv.put(x + dx * k, y + dy * k, if k == 1 { 'w' } else { '3' });
            }
        }
        cv.rect(x - 1, y - 1, 3, 3, 'w');
    }
    if a <= 5 {
        let r = 4.0 + a as f32 * 2.2;
        for k in 0..20 {
            if a > 3 && k % 2 == 0 {
                continue;
            }
            let t = k as f32 * TAU / 20.0;
            cv.put(x + (t.cos() * r).round() as i32, y + (t.sin() * r).round() as i32, if a <= 2 { '3' } else { '2' });
        }
    }
    word(cv, h, (x, y));
}

/// The word over a turned bullet, on the shared clock: each letter a fifth
/// of a tick behind the last (the whole word is up by the frame a hold of
/// the room's time stands on) springs up from under its line, past it and back
/// (`ease::back`), white as it lands and sea-glass after; the word drifts
/// up as it holds, then lifts away through the dither (never a blink). A
/// dark rim, so it reads over anything.
fn word(cv: &mut Img, h: &Hit, (x, y): (i32, i32)) {
    use super::super::ink::{GLYPH_ADVANCE, bayer};
    use crate::motion::{ease, share};
    let life = f32::from(crate::rules::melee::TURNED_TICKS) - 1.0;
    let a = f32::from(h.age());
    if a >= life {
        return;
    }
    let (tx, ty) = (x - text_width(PARRY) / 2, y - 16);
    let leave = share(a, life - 4.0, 4.0);
    let drift = -2.0 * share(a, 5.0, life - 9.0) - 4.0 * ease::inward(leave);
    for (i, ch) in PARRY.chars().enumerate() {
        let own = a - i as f32 * 0.2;
        if own < 0.0 {
            continue;
        }
        let rise = 5.0 * (1.0 - ease::back(share(own, 0.0, 5.0)));
        let (lx, ly) = (tx + i as i32 * GLYPH_ADVANCE, ty + (rise + drift).round() as i32);
        let mut glyph = Img::new(GLYPH_ADVANCE + 1, 9);
        let s = ch.to_string();
        for (dx, dy) in [(0, 1), (2, 1), (1, 0), (1, 2)] {
            glyph.text(dx, dy, &s, 'k');
        }
        glyph.text(1, 1, &s, if own < 2.0 { 'w' } else { '3' });
        for gy in 0..glyph.h {
            for gx in 0..glyph.w {
                let (px, py) = (lx - 1 + gx, ly - 1 + gy);
                if let Some(c) = glyph.get(gx, gy)
                    && bayer(px, py) >= leave
                {
                    cv.set(px, py, c);
                }
            }
        }
    }
}

/// A turned bullet on its way home: a gold wake behind it and a glint about
/// it, so a knight sees whose it is now.
fn turned(cv: &mut Img, run: &Run) {
    for shot in run.projectiles.iter().filter(|p| p.traits.turned && !p.hostile && p.ttl > 0) {
        let (x, y) = at(shot.x, shot.y);
        let (ux, uy) = {
            let n = shot.vx.hypot(shot.vy).max(1e-3);
            (shot.vx / n, shot.vy / n)
        };
        for k in 2..9 {
            let ink = match k {
                2..=3 => 'T',
                4..=5 => 't',
                6..=7 => 'O',
                _ => 'R',
            };
            let f = k as f32;
            cv.put(x - (ux * f).round() as i32, y - (uy * f).round() as i32, ink);
            if k <= 5 {
                cv.put(x - (ux * f - uy).round() as i32, y - (uy * f + ux).round() as i32, 'O');
            }
        }
        let spin = (run.tick % 4) as f32 * FRAC_PI_2 / 2.0;
        for k in 0..4 {
            let t = spin + k as f32 * FRAC_PI_2;
            cv.put(x + (t.cos() * 4.0).round() as i32, y + (t.sin() * 4.0).round() as i32, 'T');
        }
    }
}

pub(super) fn over(cv: &mut Img, run: &Run) {
    turned(cv, run);
    for h in &run.feel.melee.hits {
        spark(cv, run, h);
    }
}
