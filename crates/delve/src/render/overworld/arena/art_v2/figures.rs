use super::super::class_looks::Look;
use super::super::super::ink::Img;
use super::{kit, paint::*};
use crate::rules::{Enemy, Hero};

pub fn knight_supported(
    run: &crate::rules::Run,
    h: &Hero,
    id: u32,
) -> bool {
    // A knight of a class is drawn as the class here, whatever their kit's
    // colours, dress or shield; the authored states below still go to v1.
    if Look::of(h).is_some() {
        return !run.works.kits.get(&id).is_some_and(|kit| kit.duck)
            && !h.has_rune(crate::rules::runes::RuneKind::Invisibility)
            && h.avatar.is_none()
            && h.worn(&run.book).is_empty()
            && !h.frog()
            && !h.stone
            && h.chilled == 0
            && h.rolling().is_none()
            && h.tossed == 0
            && h.privy == 0
            && h.pit.stop.is_none();
    }
    !run.shield_out(id)
        && !run.works.kits.get(&id).is_some_and(|kit| kit.duck)
        && !h.has_rune(crate::rules::runes::RuneKind::Invisibility)
        && !(h.shielding && h.aim_y.abs() > h.aim_x.abs() * 1.25)
        && h.avatar.is_none()
        && h.knight.as_deref() != Some(super::super::knights::DWARF)
        && h.gear == 0
        && h.colours().is_empty()
        && h.worn(&run.book).is_empty()
        && !h.frog()
        && !h.stone
        && h.chilled == 0
        && h.rolling().is_none()
        && h.tossed == 0
        && h.privy == 0
        && h.pit.stop.is_none()
}

pub fn knight(cv: &mut Canvas, x: f32, y: f32, h: &Hero, _tick: u64, id: u32, thrown: bool) {
    if Look::of(h).is_some() {
        return class_figure(cv, x, y, h, id, thrown);
    }
    let steel = Ramp::inks('s', 'u', 'i');
    let gold = Ramp::inks('b', 'o', 'T');
    let gait = (h.stride * std::f32::consts::TAU / 8.0).sin();
    let py = y - gait.abs() * 0.3;
    let side = if h.aim_x < -0.3 { -1.0 } else { 1.0 };
    let (name, tint) = if h.aim_y < -h.aim_x.abs() {
        ("knight-back", Some(id))
    } else if gait.abs() > 0.55 {
        ("knight-run", Some(id))
    } else {
        (
            ["knight-gold", "knight-red", "knight-blue", "knight-green"][id as usize % 4],
            None,
        )
    };
    let im = kit::get(name);
    cv.shadow(x, y + 1.0, 9.0, 2.4, 0.65);
    kit::sprite(
        cv,
        im,
        x - im.w as f32 * 0.25,
        py - im.h as f32 * 0.5,
        side > 0.0,
        1.0,
        tint,
        gait * 1.5,
    );
    // The weapon pivots independently of the gait, matching the live aim.
    let aim_len = h.aim_x.hypot(h.aim_y).max(0.01);
    let aim = if aim_len < 0.1 {
        (side, 0.0)
    } else {
        (h.aim_x / aim_len, h.aim_y / aim_len)
    };
    let hand = (x + aim.0 * 5.0, py - 9.0 + aim.1 * 3.0);
    let muzzle = (hand.0 + aim.0 * 8.0, hand.1 + aim.1 * 8.0);
    match h.weapon {
        crate::rules::Weapon::Handgonne => {
            cv.line(hand, muzzle, 2.0, gold.at(0.43));
            cv.line(
                (hand.0, hand.1 - 0.5),
                (muzzle.0, muzzle.1 - 0.5),
                0.7,
                steel.at(0.9),
            );
            cv.ellipse(muzzle.0, muzzle.1, 1.0, 1.3, gold, 11);
        }
        crate::rules::Weapon::Crossbow => {
            cv.line(hand, muzzle, 1.2, gold.at(0.67));
            cv.line(
                (muzzle.0 - aim.1 * 3.5, muzzle.1 + aim.0 * 3.5),
                (muzzle.0 + aim.1 * 3.5, muzzle.1 - aim.0 * 3.5),
                1.0,
                steel.at(0.74),
            );
        }
        _ => {
            for i in 0..8 {
                let a = -1.2 + i as f32 * 0.3;
                let c = (
                    hand.0 + aim.0 * a.cos() * 3.0 - aim.1 * a.sin() * 5.0,
                    hand.1 + aim.1 * a.cos() * 3.0 + aim.0 * a.sin() * 5.0,
                );
                let b = a + 0.3;
                cv.line(
                    c,
                    (
                        hand.0 + aim.0 * b.cos() * 3.0 - aim.1 * b.sin() * 5.0,
                        hand.1 + aim.1 * b.cos() * 3.0 + aim.0 * b.sin() * 5.0,
                    ),
                    0.6,
                    gold.at(0.8),
                );
            }
        }
    }
    cv.ellipse(hand.0, hand.1, 1.7, 1.5, steel, 47);
    if h.struck > 0 {
        cv.flat_ellipse(
            x,
            py - 10.0,
            6.5,
            8.0,
            material('w'),
            if h.struck > 6 { 0.78 } else { 0.28 },
        );
    }
}
pub fn skeleton(cv: &mut Canvas, x: f32, y: f32, e: &Enemy, tick: u64) {
    let im = kit::get("skeleton");
    let stride = (tick as f32 * 0.17 + e.id as f32).sin();
    cv.shadow(x, y + 1.0, 8.0, 2.5, 0.62);
    kit::sprite(
        cv,
        im,
        x - im.w as f32 * 0.25,
        y - im.h as f32 * 0.5,
        true,
        1.0,
        None,
        stride * 1.5,
    );
}
pub(super) fn horse(
    cv: &mut Canvas,
    x: f32,
    y: f32,
    east: bool,
    tick: u64,
    helios: bool,
    gallop: bool,
    variant: u32,
) {
    let name = if helios {
        "helios-calm"
    } else {
        [
            "horse-brown",
            "horse-grey",
            "horse-tan",
            "horse-black",
            "horse-bay",
            "horse-ranch-grey",
        ][variant as usize % 6]
    };
    let im = kit::get(name);
    let scale = if helios {
        1.0
    } else if variant == 3 {
        1.2
    } else if variant >= 4 {
        1.0
    } else {
        0.85
    };
    let phase = (tick % 24) as f32 / 24.0 * std::f32::consts::TAU;
    let stride = if gallop { phase.sin() * 2.0 } else { 0.0 };
    let bob = if gallop { phase.cos() * 0.6 } else { 0.0 };
    cv.shadow(x, y + 1.0, im.w as f32 * scale * 0.22, 4.0, 0.62);
    kit::sprite(
        cv,
        im,
        x - im.w as f32 * scale * 0.25,
        y - im.h as f32 * scale * 0.5 - bob,
        east,
        scale,
        None,
        stride,
    );
    if helios && gallop {
        kit::emission(
            cv,
            kit::get("helios"),
            kit::get("helios-calm"),
            x - im.w as f32 * scale * 0.25,
            y - im.h as f32 * scale * 0.5 - bob,
            east,
            scale,
        );
        let dir = if east { 1.0 } else { -1.0 };
        cv.glow(Point::new(x + dir * 14.0, y - 22.0, 27.0, '6', 0.6));
        for i in 0..12 {
            let age = ((tick + i * 5) % 23) as f32 / 23.0;
            cv.mist(
                x - dir * (17.0 + age * 23.0),
                y - 19.0 - age * 9.0 + (i as f32 * 2.0).sin() * 2.0,
                0.6,
                0.4,
                mix(material('6'), material('w'), age),
                0.7 * (1.0 - age),
            );
        }
    }
}

/// A knight of a class, as the class: the arena's own drawing of them (facing,
/// stride, strike, seat trim), brought up to this canvas at three samples a
/// pixel with smoothed edges, lit from the upper left and shaded at the lower
/// right, inside a dark coloured edge.
fn class_figure(
    cv: &mut Canvas,
    x: f32,
    y: f32,
    h: &Hero,
    id: u32,
    thrown: bool,
) {
    let mut im = super::super::knights::figure_with(h, id, thrown);
    if h.aim_x < -0.3 {
        im = im.flip_h();
    }
    let im = lit(&scale3x(&im));
    cv.shadow(x, y + 1.0, 9.0, 2.4, 0.65);
    kit::sprite(cv, &im, x - im.w as f32 * 0.25, y - im.h as f32 * 0.5, true, 1.0, None, 0.0);
    if h.struck > 0 {
        cv.flat_ellipse(x, y - 10.0, 6.5, 8.0, material('w'), if h.struck > 6 { 0.78 } else { 0.28 });
    }
}

/// Scale3x: each pixel becomes nine, corners taking a neighbour's colour
/// where two neighbours agree, so diagonals stay diagonals.
pub(super) fn scale3x(src: &Img) -> Img {
    let mut out = Img::new(src.w * 3, src.h * 3);
    let at = |x: i32, y: i32| src.get(x, y);
    for y in 0..src.h {
        for x in 0..src.w {
            let (a, b, c) = (at(x - 1, y - 1), at(x, y - 1), at(x + 1, y - 1));
            let (d, e, f) = (at(x - 1, y), at(x, y), at(x + 1, y));
            let (g, hh, i) = (at(x - 1, y + 1), at(x, y + 1), at(x + 1, y + 1));
            let px = if b != hh && d != f {
                [
                    if d == b { d } else { e },
                    if (d == b && e != c) || (b == f && e != a) { b } else { e },
                    if b == f { f } else { e },
                    if (d == b && e != g) || (d == hh && e != a) { d } else { e },
                    e,
                    if (b == f && e != i) || (hh == f && e != c) { f } else { e },
                    if d == hh { d } else { e },
                    if (d == hh && e != i) || (hh == f && e != g) { hh } else { e },
                    if hh == f { f } else { e },
                ]
            } else {
                [e; 9]
            };
            for (k, p) in px.into_iter().enumerate() {
                if let Some(p) = p {
                    out.set(x * 3 + k as i32 % 3, y * 3 + k as i32 / 3, p);
                }
            }
        }
    }
    out
}

/// Light from the upper left: an edge facing it warms toward ivory, an edge
/// facing away cools toward the stone shadow, and the silhouette gets a
/// dark coloured edge so it parts from the ground. Near-black inks become
/// coloured occlusion, never holes.
pub(super) fn lit(src: &Img) -> Img {
    let ivory = [226, 222, 204];
    let edge = mix(material('s'), material('q'), 0.25);
    let mut out = Img::new(src.w + 2, src.h + 2);
    let filled = |x: i32, y: i32| src.get(x, y).is_some();
    for y in -1..=src.h {
        for x in -1..=src.w {
            match src.get(x, y) {
                Some(c) => {
                    let c = crate::render::art::outline(c);
                    let c = if !filled(x - 1, y - 2) || !filled(x - 2, y - 1) {
                        mix(c, ivory, 0.16)
                    } else if !filled(x + 1, y + 2) || !filled(x + 2, y + 1) {
                        mix(c, material('s'), 0.38)
                    } else {
                        c
                    };
                    out.set(x + 1, y + 1, c);
                }
                None if (-1..=1).any(|dy| (-1..=1).any(|dx| filled(x + dx, y + dy))) => {
                    out.set(x + 1, y + 1, edge);
                }
                None => {}
            }
        }
    }
    out
}
