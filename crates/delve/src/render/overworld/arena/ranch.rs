//! The horse ranch and the flower fields, drawn: morning light from the
//! east, small yellow flowers in the grass, the red barn, Old Barley at the
//! hitching rail with the groom's brush going, war horses in their paddock,
//! and Helios.
//!
//! Helios is a Belgian draft, brown with a white star and a yellow mane. He
//! is drawn from shapes like the destrier (`joust::destrier`), heavier, with
//! feathered legs, and his mane and tail are a layer of their own: when he
//! gallops through the low sun they are drawn as fire.
use super::super::ink::{Img, bayer, hash, vnoise};
use super::super::material as m;
use super::{TILE, at, home, stand};
use crate::rules::ranch::{
    self as r, Age, BARN, DRAFT_AT, FIELDS_GATE, FIELDS_OUT, GROOM_AT, HAY, LECTERN, OAK,
    PADDOCK, RAIL, RANCH_OUT, STALL, STUD_BOOK, TROUGH,
};
use crate::rules::{RoomKind, Run, TILE_UNITS};

/// A horse's colours: hide, its shade, its sunlit top, mane, the feather
/// on the legs, and the hooves.
#[derive(Clone, Copy)]
pub(super) struct Hide {
    hide: char,
    dark: char,
    light: char,
    mane: [char; 3],
    feather: char,
    hoof: char,
    star: bool,
}

/// Helios: brown, a white star, a yellow mane.
const HELIOS: Hide = Hide {
    hide: 'R',
    dark: 'r',
    light: 'o',
    mane: ['T', 't', 'O'],
    feather: 'c',
    hoof: 'X',
    star: true,
};
/// Old Barley: a black draft, brushed till he shines.
const BARLEY: Hide = Hide {
    hide: 'X',
    dark: 'K',
    light: 'j',
    mane: ['K', 'K', 'k'],
    feather: '$',
    hoof: 'k',
    star: false,
};
/// The mares at grass, and the war horses.
const MARES: [Hide; 3] = [
    Hide { hide: 'R', dark: 'p', light: 'r', mane: ['n', 'n', 'b'], feather: 'n', hoof: 'k', star: false },
    Hide { hide: 'i', dark: 'h', light: 'H', mane: ['H', 'i', 'h'], feather: 'H', hoof: 'X', star: false },
    Hide { hide: 'O', dark: 'o', light: 't', mane: ['n', 'b', 'b'], feather: 'b', hoof: 'k', star: true },
];
const WAR: [Hide; 3] = [
    Hide { hide: 'K', dark: 'k', light: 'X', mane: ['k', 'k', 'K'], feather: 'K', hoof: 'k', star: false },
    Hide { hide: 'h', dark: 'G', light: 'i', mane: ['G', 'J', 'j'], feather: 'i', hoof: 'X', star: false },
    Hide { hide: 'p', dark: 'B', light: 'R', mane: ['n', 'n', 'b'], feather: 'n', hoof: 'k', star: true },
];

/// Each leg's (upper, lower) angle for the four frames of the gallop, as
/// the destrier's.
const GAIT: [[(f32, f32); 4]; 4] = [
    [(35.0, 10.0), (20.0, -40.0), (-30.0, 30.0), (-10.0, 60.0)],
    [(10.0, -45.0), (40.0, 20.0), (-40.0, 10.0), (-25.0, 40.0)],
    [(-15.0, -60.0), (10.0, -20.0), (20.0, 50.0), (-35.0, 15.0)],
    [(25.0, -10.0), (-5.0, -50.0), (0.0, 70.0), (10.0, 40.0)],
];
const STAND: [(f32, f32); 4] = [(4.0, 0.0), (-4.0, 0.0), (-4.0, 6.0), (4.0, 4.0)];

/// How a horse is posed.
#[derive(Clone, Copy, PartialEq)]
pub(super) enum Pose {
    Stand,
    Graze,
    Gallop(u32),
}

fn thick(im: &mut Img, (x0, y0): (f32, f32), (x1, y1): (f32, f32), ink: char, w: i32) {
    let n = ((x1 - x0).abs().max((y1 - y0).abs()) * 2.0) as i32 + 1;
    for i in 0..=n {
        let t = i as f32 / n as f32;
        let (x, y) = (x0 + (x1 - x0) * t, y0 + (y1 - y0) * t);
        for ox in 0..w {
            for oy in 0..w {
                im.put(
                    (x + ox as f32 - (w - 1) as f32 / 2.0).floor() as i32,
                    (y + oy as f32 - (w - 1) as f32 / 2.0).floor() as i32,
                    ink,
                );
            }
        }
    }
}

/// The fire ramp, root to tip: gold at the roots burning to white at the
/// ends, the way his mane looks with the morning sun through it.
const FIRE: [char; 6] = ['4', '5', '6', 'T', '9', 'w'];

/// One lock of hair, root to tip: tapering from `w` to a point, waving,
/// inked by how far along it is (`ramp`).
fn lock(
    hair: &mut Img,
    k: f32,
    (x0, y0): (f32, f32),
    (dx, dy): (f32, f32),
    len: f32,
    w: f32,
    wave: (f32, f32),
    ramp: &dyn Fn(f32) -> char,
) {
    let steps = ((len * k) * 2.5) as i32 + 2;
    let norm = (dx * dx + dy * dy).sqrt().max(0.01);
    let (ux, uy) = (dx / norm, dy / norm);
    let (nx, ny) = (-uy, ux);
    for s in 0..=steps {
        let t = s as f32 / steps as f32;
        let sway = (t * 4.2 + wave.1).sin() * wave.0 * t;
        let (x, y) = (
            (x0 + ux * len * t + nx * sway) * k,
            (y0 + uy * len * t + ny * sway) * k,
        );
        let r = ((w * (1.0 - t * 0.85)) * k / 2.0).max(0.5);
        let ink = ramp(t);
        let ri = r.ceil() as i32;
        for oy in -ri..=ri {
            for ox in -ri..=ri {
                if (ox * ox + oy * oy) as f32 <= r * r + 0.3 {
                    hair.put((x + ox as f32) as i32, (y + oy as f32) as i32, ink);
                }
            }
        }
    }
}

/// A heavy horse side-on, facing east, drawn at `k` times its 64 by 60
/// design: short-coupled and deep, a crested neck, feathered legs. `fire`,
/// the tick, sets mane and tail alight in the sun; `braided` plaits the
/// mane with ribbons.
pub(super) fn draft(h: Hide, pose: Pose, k: f32, fire: Option<u32>, braided: bool) -> Img {
    // Room on the left for the tail to stream.
    const OX: f32 = 18.0;
    const W: f32 = 64.0 + OX;
    const H: f32 = 60.0;
    let (iw, ih) = ((W * k).ceil() as i32 + 2, (H * k).ceil() as i32 + 2);
    let mut body = Img::new(iw, ih);
    let mut hair = Img::new(iw, ih);
    let p = |x: f32, y: f32| ((x + OX) * k, y * k);
    let px = |w: f32| ((w * k).round() as i32).max(1);
    let gait = match pose {
        Pose::Gallop(f) => Some(f),
        _ => None,
    };
    let bob = gait.map_or(0.0, |f| [0.0, -1.5, -0.5, 1.0][(f % 4) as usize]);
    // The body's centre line; legs hang from it.
    let by = 34.0 + bob;
    let legs = gait.map_or(STAND, |f| GAIT[(f % 4) as usize]);
    let leg = |im: &mut Img, x: f32, (up, lo): (f32, f32), ink: char, near: bool| {
        let (a, b) = (up.to_radians(), (up + lo).to_radians());
        let (kx, ky) = (x + a.sin() * 7.5, by + 2.0 + a.cos() * 7.5);
        let (fx, fy) = (kx + b.sin() * 8.0, ky + b.cos() * 8.0);
        // The forearm tapers from the body to the knee, the cannon below
        // it is lean: a leg, not a post.
        let (mx, my) = ((x + kx) / 2.0, (by + ky) / 2.0 + 1.0);
        thick(im, p(x, by), p(mx, my), ink, px(if near { 6.0 } else { 5.0 }));
        thick(im, p(mx, my), p(kx, ky), ink, px(if near { 4.4 } else { 3.8 }));
        thick(im, p(kx, ky), p(fx, fy), ink, px(3.0));
        // Feather: long hair from mid-cannon over the hoof, flaring, ragged.
        let n = 10;
        for i in 0..=n {
            let t = i as f32 / n as f32;
            let (x, y) = (kx + (fx - kx) * (0.45 + t * 0.55), ky + (fy - ky) * (0.45 + t * 0.55));
            let half = (1.6 + t * 2.4) * k;
            let (cx, cy) = p(x, y);
            for dx in -(half.ceil() as i32)..=(half.ceil() as i32) {
                let ragged = hash(cx as i32 + dx, cy as i32, 71) % 3 == 0;
                if (dx as f32).abs() <= half - if ragged { 1.0 } else { 0.0 } {
                    let ink = if hash(cx as i32 + dx, cy as i32, 72) % 4 == 0 { 'h' } else { h.feather };
                    im.put(cx as i32 + dx, cy as i32, ink);
                }
            }
        }
        let (hx, hy) = p(fx, fy + 1.0);
        for dx in -px(2.0)..=px(2.0) {
            im.put(hx as i32 + dx, hy as i32, h.hoof);
        }
    };
    leg(&mut body, 40.0, legs[1], h.dark, false);
    leg(&mut body, 17.0, legs[3], h.dark, false);
    leg(&mut body, 38.0, legs[0], h.hide, true);
    leg(&mut body, 15.0, legs[2], h.hide, true);
    // Rump, barrel and chest: round and deep, about as long as he is tall.
    let mut shape = Img::new(iw, ih);
    for (cx, cy, rx, ry) in [
        (16.0, by - 9.0, 10.5, 11.0),
        (27.0, by - 8.0, 11.0, 10.5),
        (37.0, by - 9.0, 9.0, 11.0),
        (26.5, by - 5.0, 15.0, 7.5),
    ] {
        let (x, y) = p(cx, cy);
        shape.ellipse(x, y, rx * k, ry * k, 'x');
    }
    // The neck: massive, crested; low to the grass when grazing.
    let (lift, reach, head) = match pose {
        Pose::Graze => (-16.0, 10.0, (52.0, by + 3.0)),
        Pose::Gallop(_) => (12.0, 9.0, (53.0, by - 24.0)),
        Pose::Stand => (15.0, 7.0, (51.0, by - 27.0)),
    };
    for i in 0..16 {
        let t = i as f32 / 15.0;
        let (x, y) = p(40.0 + t * reach, by - 13.0 - t * lift);
        shape.ellipse(x, y, (8.0 - t * 3.6) * k, (7.5 - t * 3.4) * k, 'x');
        // The crest rising over the top line of the neck.
        if pose != Pose::Graze && (0.2..0.85).contains(&t) {
            let (x, y) = p(40.0 + t * reach - 1.5, by - 17.5 - t * lift);
            shape.ellipse(x, y, 4.5 * k, 3.0 * k, 'x');
        }
    }
    let (hx, hy) = head;
    // A big, kind draft head: broad, a straight face, a wide muzzle.
    let tilt = if pose == Pose::Graze { 2.0 } else { 0.0 };
    let (x, y) = p(hx, hy);
    shape.ellipse(x, y, 6.0 * k, 4.4 * k, 'x');
    let (x, y) = p(hx + 4.8, hy + 2.6 + tilt);
    shape.ellipse(x, y, 3.8 * k, 3.4 * k, 'x');
    // Shade: sunlit along the top line, dark beneath, a highlight on the
    // round of the hip.
    for y in 0..ih {
        for x in 0..iw {
            if !shape.solid(x, y) {
                continue;
            }
            let top = (0..y).rev().take_while(|&yy| shape.solid(x, yy)).count() as i32;
            let below = (y + 1..ih).take_while(|&yy| shape.solid(x, yy)).count() as i32;
            let (hipx, hipy) = p(14.0, by - 14.0);
            let hip = ((x as f32 - hipx).powi(2) + (y as f32 - hipy).powi(2)).sqrt() < 4.0 * k
                && bayer(x, y) < 0.35;
            let ink = if top < px(2.0) || hip {
                h.light
            } else if below < px(3.5) {
                h.dark
            } else if top < px(4.0) && hash(x, y, 5) % 3 != 0 {
                h.light
            } else {
                h.hide
            };
            body.put(x, y, ink);
        }
    }
    // Two small ears, the eye, a nostril, and the star.
    for (ox, len) in [(-3.0f32, 4.0f32), (-1.5, 3.5)] {
        let (ex, ey) = (hx + ox, hy - 3.6);
        let tip = if pose == Pose::Graze { (ex - 1.5, ey - len + 1.0) } else { (ex - 0.5, ey - len) };
        thick(&mut body, p(ex, ey), p(tip.0, tip.1), h.dark, px(2.0));
    }
    let (ex, ey) = p(hx + 0.5, hy - 1.0);
    body.put(ex as i32, ey as i32, 'k');
    body.put(ex as i32 - 1, ey as i32, h.dark);
    let (nx, ny) = p(hx + 7.0, hy + 3.0 + tilt);
    body.put(nx as i32, ny as i32, 'k');
    if h.star {
        let (sx, sy) = p(hx + 3.0, hy - 2.3);
        let r = (1.2 * k).max(1.0);
        for dy in -(r.ceil() as i32)..=(r.ceil() as i32) {
            for dx in -(r.ceil() as i32)..=(r.ceil() as i32) {
                if (dx.abs() + dy.abs()) as f32 <= r + 0.5 {
                    body.put(sx as i32 + dx, sy as i32 + dy, '9');
                }
            }
        }
    }
    // Parted from the ground by a rim a step under its own shade, never
    // by a black line.
    let rim = super::super::ink::ink(h.dark).map(m::darker);
    if let Some(rim) = rim {
        let edge: Vec<(i32, i32)> = (0..ih)
            .flat_map(|y| (0..iw).map(move |x| (x, y)))
            .filter(|&(x, y)| !body.solid(x, y) && [(1, 0), (-1, 0), (0, 1), (0, -1)].iter().any(|&(dx, dy)| body.solid(x + dx, y + dy)))
            .collect();
        for (x, y) in edge {
            body.set(x, y, rim);
        }
    }
    // Mane and tail: thick locks, root to tip.
    let tick = fire.unwrap_or(0);
    let calm = h.mane;
    let shade = |i: u32| i % 3 == 1;
    let wind = gait.map_or(0.0, |f| f as f32 * 0.9);
    let crest_a = (38.0, by - 18.0);
    let crest_b = match pose {
        Pose::Graze => (hx - 4.0, hy - 3.0),
        _ => (hx - 3.5, hy - 4.0),
    };
    let locks = 11u32;
    for i in 0..locks {
        let t = i as f32 / (locks - 1) as f32;
        let root = (
            crest_a.0 + (crest_b.0 - crest_a.0) * t,
            crest_a.1 + (crest_b.1 - crest_a.1) * t,
        );
        if braided {
            // Plaited into knots along the crest, each with a ribbon.
            let (x, y) = p(root.0, root.1 + 1.5);
            body.put(x as i32, y as i32, calm[0]);
            hair.put(x as i32, y as i32 + 1, if i % 2 == 0 { '7' } else { '2' });
            continue;
        }
        let (dir, len) = match pose {
            Pose::Gallop(_) => ((-1.0, -0.25), 9.0 + (i % 3) as f32 * 1.5),
            Pose::Graze => ((-0.2, 1.0), 7.0 + (i % 2) as f32),
            Pose::Stand => ((-0.2, 1.0), 8.5 + (i % 3) as f32 * 1.2),
        };
        let flick = (tick as f32 / 3.0 + i as f32 * 1.7).sin() * 0.08;
        let dark = shade(i);
        let ramp = move |s: f32| -> char {
            match fire {
                Some(_) => FIRE[(((s + flick).clamp(0.0, 0.999)) * 6.0) as usize],
                None => {
                    if dark { calm[2] } else { calm[((s * 2.2) as usize).min(1)] }
                }
            }
        };
        lock(&mut hair, k, (root.0 + OX, root.1), dir, len, 3.0, (0.9, wind + i as f32), &ramp);
    }
    if !braided {
        // The forelock, falling over the brow beside the star.
        let ramp = |s: f32| match fire {
            Some(_) => FIRE[((s * 4.0) as usize).min(5)],
            None => calm[0],
        };
        lock(&mut hair, k, (hx - 3.0 + OX, hy - 3.8), (0.7, 0.6), 3.5, 2.0, (0.3, 0.0), &ramp);
    }
    // The tail: full and long; streaming at the gallop, falling at rest.
    for i in 0..9u32 {
        let root = (6.5 + OX, by - 16.0 + i as f32 * 0.5);
        let (dir, len) = if gait.is_some() {
            ((-1.0, -0.12 + i as f32 * 0.07), 18.0 + (i % 3) as f32 * 2.5)
        } else {
            ((-0.22, 1.0), 20.0 + (i % 3) as f32)
        };
        let flick = (tick as f32 / 3.0 + i as f32 * 2.3).sin() * 0.08;
        let dark = shade(i);
        let ramp = move |s: f32| -> char {
            match fire {
                Some(_) => FIRE[(((s + flick).clamp(0.0, 0.999)) * 6.0) as usize],
                None => {
                    if dark { calm[2] } else { calm[((s * 2.2) as usize).min(1)] }
                }
            }
        };
        lock(&mut hair, k, root, dir, len, 3.2, (1.8, wind * 0.8 + i as f32 * 0.7), &ramp);
    }
    let mut im = body;
    im.stamp(&hair, 0, 0);
    // In the sun, the hair glows past its own edge, and white sparks drift
    // off the ends of the tail.
    if let Some(tick) = fire {
        let mut glow = Img::new(iw, ih);
        for y in 0..ih {
            for x in 0..iw {
                if im.solid(x, y) {
                    continue;
                }
                let lit = [(1, 0), (-1, 0), (0, 1), (0, -1), (1, 1), (-1, -1)]
                    .iter()
                    .any(|&(dx, dy)| hair.solid(x + dx, y + dy));
                if lit && bayer(x, y) < 0.55 {
                    glow.put(x, y, if bayer(x + 1, y) < 0.3 { '9' } else { '6' });
                }
            }
        }
        im.stamp(&glow, 0, 0);
        for i in 0..5u32 {
            let age = (tick / 2 + i * 7) % 16;
            let (x, y) = p(4.0 - age as f32 * 0.9, by - 14.0 + i as f32 * 1.6 - age as f32 * 0.35);
            if x >= 0.0 {
                im.put(x as i32, y as i32, if age < 8 { 'w' } else { '9' });
            }
        }
    }
    im
}

/// Morning ground: grass lit from the east, with small yellow flowers in
/// drifts. `field` is how much of it is open meadow (the yard keeps only its
/// verges).
fn grass(cv: &mut Img, x: i32, y: i32, w: i32) {
    let n = vnoise(x as f32 / 22.0, y as f32 / 16.0, 41);
    let sun = x as f32 / w as f32 * 0.45 + (1.0 - y as f32 / 224.0) * 0.15;
    // Clumps of blades a few pixels across, each lit on its north-west,
    // where an ordered dither once laid an even checker.
    let c = vnoise(x as f32 / 3.0, y as f32 / 2.2, 42);
    let cl = vnoise((x + 1) as f32 / 3.0, (y + 1) as f32 / 2.2, 42);
    let light = (n * 0.45 + sun + 0.06 + (c - 0.5) * 0.14 + (c - cl) * 0.35).clamp(0.0, 0.999);
    const RAMP: [char; 8] = ['E', 'l', 'N', 'm', 'L', 'A', 'M', 'y'];
    let mut ink = RAMP[(light * RAMP.len() as f32) as usize];
    // Blades: a darker stroke here and there.
    if hash(x, y, 17) % 37 == 0 || hash(x, y - 1, 17) % 37 == 0 {
        ink = 'E';
    }
    // Dew catching the sun on the eastern side.
    if x > w / 2 && hash(x, y, 23) % 97 == 0 {
        ink = 'Y';
    }
    cv.put(x, y, ink);
}

/// The yard's gravel: larger patches of light and shadow with stones in
/// loose clusters on them, each stone lit along its top over a darker body
/// and a shadowed foot; never an even speckle.
fn gravel(x: i32, y: i32) -> char {
    let patch = vnoise(x as f32 / 12.0, y as f32 / 9.0, 35);
    let base = if patch > 0.62 { 'X' } else { 'g' };
    let (cx, cy) = (x.div_euclid(4), y.div_euclid(3));
    let (px, py) = (x.rem_euclid(4), y.rem_euclid(3));
    let h = hash(cx, cy, 4);
    if h % 3 == 0 {
        let ox = ((h >> 5) % 2) as i32;
        if py == 0 && (px == ox || px == ox + 1) {
            return if (h >> 8).is_multiple_of(5) { 'h' } else { 'J' };
        }
        if py == 1 && (px == ox || px == ox + 1) {
            return 'G';
        }
        if py == 1 && px == ox + 2 {
            return 'K';
        }
    }
    if h % 17 == 1 && px == 2 && py == 1 {
        return 'P';
    }
    base
}

/// Small yellow flowers, thicker in drifts.
fn flowers(cv: &mut Img, x0: i32, y0: i32, w: i32, h: i32, keep: impl Fn(i32, i32) -> bool) {
    for cy in (y0..y0 + h).step_by(3) {
        for cx in (x0..x0 + w).step_by(3) {
            let drift = vnoise(cx as f32 / 46.0, cy as f32 / 34.0, 77);
            let odds = if drift > 0.58 { 6 } else if drift > 0.45 { 17 } else { 47 };
            let roll = hash(cx, cy, 31);
            if roll % odds != 0 {
                continue;
            }
            let (x, y) = (cx + (roll >> 8) as i32 % 3, cy + (roll >> 12) as i32 % 3);
            if !keep(x, y) {
                continue;
            }
            if roll % 5 == 0 {
                // An open flower: petals round a darker eye.
                for (dx, dy) in [(-1, 0), (1, 0), (0, -1), (0, 1)] {
                    cv.put(x + dx, y + dy, '5');
                }
                cv.put(x, y, '4');
                cv.put(x, y + 2, 'm');
            } else if roll % 11 == 0 {
                cv.put(x, y, 'c');
            } else {
                cv.put(x, y, if roll % 2 == 0 { '6' } else { '5' });
                cv.put(x, y + 1, 'L');
            }
        }
    }
}

/// The low sun in the north-east: its disc over the tree line and a soft
/// glow into the sky (the grass is lit by `grass`).
fn sun(cv: &mut Img) {
    use super::super::ink::ink;
    let (sx, sy) = (cv.w as f32 - 34.0, 2.0);
    let sky = [ink('V'), ink('W'), ink('c'), ink('9')];
    for y in 0..TILE + 2 {
        for x in 0..cv.w {
            let d = ((x as f32 - sx).powi(2) + ((y as f32 - sy) * 1.4).powi(2)).sqrt();
            if d < 9.0 {
                cv.put(x, y, if d < 6.0 { '9' } else { '6' });
                continue;
            }
            if d >= 70.0 {
                continue;
            }
            // The glow falls off in soft rings whose edges wander in blots:
            // the sky warms toward the disc, and the crowns and grass in
            // it take the light a step or two lighter, keeping their marks.
            let g = 1.0 - d / 70.0 + (super::super::light::seam(x, y) - 0.5) * 0.1;
            let Some(c) = cv.get(x, y) else { continue };
            if sky.contains(&Some(c)) {
                if g > 0.62 {
                    cv.put(x, y, '6');
                } else if g > 0.4 {
                    cv.put(x, y, 'T');
                } else if g > 0.22 {
                    cv.put(x, y, 'c');
                }
            } else if g > 0.3 {
                let lit = m::lighter(c);
                cv.set(x, y, if g > 0.6 { m::lighter(lit) } else { lit });
            }
        }
    }
}

/// A tree line along the north edge, misty in the morning.
fn tree_line(cv: &mut Img) {
    // A pale morning sky, warmer toward the sun.
    for y in 0..TILE {
        for x in 0..cv.w {
            // Soft bands warming toward the sun, their edges wandering in
            // blots rather than a checker.
            let warm = x as f32 / cv.w as f32 + (super::super::light::seam(x, y) - 0.5) * 0.12 + 0.12;
            cv.put(x, y, if warm > 0.95 { '9' } else if warm > 0.6 { 'c' } else if y < 4 { 'V' } else { 'W' });
        }
    }
    for x in 0..cv.w {
        let top = 5 + (vnoise(x as f32 / 9.0, 0.0, 5) * 7.0) as i32;
        for y in top..TILE {
            let lit = x > cv.w * 2 / 3 && y < top + 3;
            let ink = if lit { 'A' } else if (y + x / 3) % 4 == 0 { 'F' } else { 'E' };
            cv.put(x, y, ink);
        }
        // Mist along the foot of the trees, in wisps.
        for y in TILE - 3..TILE {
            if super::super::light::seam(x * 2, y * 4) < 0.24 - (TILE - 1 - y) as f32 * 0.04 {
                cv.put(x, y, 'V');
            }
        }
    }
}

/// A split-rail fence along the room's other edges.
fn fence(cv: &mut Img, gaps: &[(i32, i32, i32, i32)]) {
    let (w, h) = (cv.w, cv.h);
    let gap = |x: i32, y: i32| {
        gaps.iter().any(|&(c, r, gw, gh)| {
            x >= c * TILE && x < (c + gw) * TILE && y >= (r - 1) * TILE && y < (r + gh + 1) * TILE
        })
    };
    // South rail: a run of split rails between each gap.
    let mut x = 0;
    while x < w {
        if gap(x, h - 8) {
            x += 1;
            continue;
        }
        let start = x;
        while x < w && !gap(x, h - 8) {
            x += 1;
        }
        m::rail_fence(cv, (start - 1, x), h - 2, &[h - 11, h - 7], 22, ('o', 'r'), start as u32);
    }
    // West and east rails, seen end-on: posts only.
    for y in (TILE + 6..h - 12).step_by(20) {
        if !gap(4, y) {
            cv.rect(3, y, 3, 9, 'P');
            cv.put(4, y, 'o');
        }
        if !gap(w - 5, y) {
            cv.rect(w - 6, y, 3, 9, 'P');
            cv.put(w - 5, y, 'o');
        }
    }
}

/// A farm gate standing open over a gap: two posts and trodden ground.
fn gate(cv: &mut Img, (c, r, w, _): (i32, i32, i32, i32), south: bool) {
    let (x0, x1) = (c * TILE, (c + w) * TILE);
    for yy in r * TILE..(r + 1) * TILE + if south { 8 } else { 0 } {
        for x in x0..x1 {
            let edge = (x - x0).min(x1 - 1 - x) as f32 / 8.0;
            if super::super::light::seam(x, yy) < edge.min(1.0) * 0.8 {
                cv.put(x, yy, if hash(x, yy, 3) % 4 == 0 { 'B' } else { 'P' });
            }
        }
    }
    let y = if south { cv.h - 2 } else { r * TILE + 10 };
    for x in [x0 - 4, x1 + 1] {
        cv.rect(x, y - 14, 4, 15, 'r');
        cv.line(x, y - 14, x + 3, y - 14, 'o');
        cv.line(x + 3, y - 13, x + 3, y, 'P');
    }
}

fn px((c, r, w, h): (i32, i32, i32, i32)) -> (i32, i32, i32, i32) {
    (c * TILE, r * TILE, w * TILE, h * TILE)
}

fn oak(cv: &mut Img) {
    let (x, y, w, h) = px(OAK);
    let (cx, cy) = (x + w / 2, y + h / 2 - 6);
    // Its morning shadow, long to the south-west.
    for yy in cy..cy + h {
        for xx in x - 30..x + w {
            // Solid in its core, its edge breaking up in soft blots.
            let (dx, dy) = ((xx - (cx - 22)) as f32 / 34.0, (yy - (cy + 26)) as f32 / 14.0);
            let d = dx * dx + dy * dy;
            if d < 1.0 && (d < 0.55 || super::super::light::seam(xx, yy) > (d - 0.55) / 0.45) {
                if let Some(c) = cv.get(xx, yy) {
                    let _ = c;
                    cv.put(xx, yy, 'E');
                }
            }
        }
    }
    cv.rect(cx - 3, cy + 8, 7, h / 2 + 4, 'I');
    cv.line(cx + 2, cy + 8, cx + 2, cy + h / 2 + 10, 'P');
    for yy in y - 10..y + h - 4 {
        for xx in x - 8..x + w + 8 {
            let d = ((xx - cx) as f32 / 34.0).powi(2) + ((yy - cy) as f32 / 26.0).powi(2);
            let n = vnoise(xx as f32 / 6.0, yy as f32 / 6.0, 12);
            if d + (n - 0.5) * 0.5 < 1.0 {
                let lit = (xx - cx) as f32 * 0.7 - (yy - cy) as f32 + n * 20.0;
                let ink = if lit > 18.0 { 'M' } else if lit > 6.0 { 'A' } else if lit > -8.0 { 'l' } else { 'E' };
                cv.put(xx, yy, ink);
            }
        }
    }
}

fn trough(cv: &mut Img, (c, r, w, _): (i32, i32, i32, i32), tick: u32) {
    let (x, y) = (c * TILE + 2, r * TILE + 3);
    let bw = w * TILE - 4;
    cv.rect(x, y, bw, 10, 'P');
    cv.rect(x + 2, y + 2, bw - 4, 4, 'q');
    let glint = (tick / 9) as i32 % (bw - 6).max(1);
    cv.put(x + 3 + glint, y + 3, '3');
    cv.line(x, y, x + bw - 1, y, 'o');
}

/// The flower fields at morning.
/// What of a room's picture never moves, drawn once and kept: the ground,
/// the flowers, the tree line and what stands first on them. Each bake
/// starts from a copy and draws the rest in the same order.
fn still(cv: &mut Img, slot: &'static std::sync::OnceLock<Img>, draw: fn(&mut Img)) {
    let base = slot.get_or_init(|| {
        let mut base = Img::black(cv.w, cv.h);
        draw(&mut base);
        base
    });
    if base.w == cv.w && base.h == cv.h {
        cv.stamp(base, 0, 0);
    } else {
        draw(cv);
    }
}

fn pasture_ground(cv: &mut Img) {
    let w = cv.w;
    for y in TILE..cv.h {
        for x in 0..w {
            grass(cv, x, y, w);
        }
    }
    flowers(cv, 0, TILE, w, cv.h - TILE, |_, _| true);
    tree_line(cv);
    oak(cv);
}

fn pasture(cv: &mut Img, tick: u32) {
    static STILL: std::sync::OnceLock<Img> = std::sync::OnceLock::new();
    let w = cv.w;
    still(cv, &STILL, pasture_ground);
    trough(cv, TROUGH, tick);
    fence(cv, &[FIELDS_OUT]);
    gate(cv, FIELDS_OUT, true);
    sun(cv);
    // Pollen and motes drifting in the light.
    for i in 0..8 {
        let seed = hash(i, 7, 99);
        let x = (seed % w as u32) as i32 + ((tick / 6 + i as u32 * 13) % 40) as i32 - 20;
        let y = TILE + (seed >> 9) as i32 % (cv.h - 2 * TILE) - ((tick / 9 + i as u32 * 7) % 24) as i32;
        if x > w / 3 {
            cv.put(x, y, '6');
        }
    }
}

fn barn(cv: &mut Img) {
    let (x, y, w, h) = px(BARN);
    let roof_h = h * 9 / 20;
    // A gambrel roof in dark red shingles, its ridge catching the sun.
    for yy in y - 6..y + roof_h {
        let inset = ((y + roof_h - yy) as f32 * 0.35) as i32;
        let (west, east) = (x + inset - 4, x + w - inset + 3);
        for xx in west..=east {
            let course = (yy - y) % 4 == 0;
            let mut ink = if course { '8' } else if (xx + (yy / 4) * 3) % 7 == 0 { 'p' } else { 'B' };
            // The roof as a volume in the low sun: its west slope lit, its
            // east slope a step into shade, the eave's last course darkest.
            if xx < west + 5 && !course {
                ink = if ink == 'B' { 'p' } else { 'R' };
            } else if xx > east - 5 {
                ink = if course { 'n' } else { 'b' };
            }
            if yy == y + roof_h - 1 {
                ink = 'n';
            }
            cv.put(xx, yy, ink);
        }
        cv.put(west, yy, 'R');
    }
    cv.line(x + 20, y - 6, x + w - 20, y - 6, 'R');
    // Red board walls, white trim: the eave's shadow across the top, the
    // ground's along the foot and the east corner in shade, where each
    // board's seam widens.
    for yy in y + roof_h..y + h {
        for xx in x..x + w {
            let k = (xx - x) % 5;
            let shade = yy < y + roof_h + 2 || yy >= y + h - 2 || xx >= x + w - 3;
            cv.put(xx, yy, if k == 0 || (shade && k == 4) { '8' } else { '7' });
        }
    }
    cv.line(x, y + roof_h, x + w - 1, y + roof_h, 'c');
    cv.line(x, y + roof_h, x, y + h - 1, 'c');
    cv.line(x + w - 1, y + roof_h, x + w - 1, y + h - 1, 'c');
    // The hayloft door with hay spilling, and the big doors with white
    // braces.
    let (lx, ly) = (x + w / 2 - 7, y + 4);
    cv.rect(lx, ly, 14, 12, 'k');
    cv.frame(lx - 1, ly - 1, 16, 14, 'c');
    for i in 0..10 {
        cv.put(lx + 2 + i, ly + 9 + (i % 3), if i % 2 == 0 { 'O' } else { 't' });
    }
    let (dx, dy, dw, dh) = (x + w / 2 - 18, y + h - 26, 36, 26);
    cv.rect(dx, dy, dw, dh, '7');
    cv.frame(dx, dy, dw, dh, 'c');
    cv.line(dx + dw / 2, dy, dx + dw / 2, dy + dh - 1, 'c');
    cv.line(dx, dy, dx + dw / 2, dy + dh - 1, 'c');
    cv.line(dx + dw / 2, dy, dx, dy + dh - 1, 'c');
    cv.line(dx + dw / 2, dy, dx + dw - 1, dy + dh - 1, 'c');
    cv.line(dx + dw - 1, dy, dx + dw / 2, dy + dh - 1, 'c');
    // A weathervane horse on the ridge.
    let (vx, vy) = (x + w / 2, y - 7);
    cv.line(vx, vy - 9, vx, vy, 'g');
    let vane = Img::from_rows(&["..gg.....", ".gggggg..", "g.gggggg.", "..g...g..", "..g...g.."]);
    cv.stamp(&vane, vx - 4, vy - 14);
}

fn stall(cv: &mut Img, tick: u32) {
    let (x, y, w, h) = px(STALL);
    cv.rect(x, y, w, h, 'P');
    for xx in x..x + w {
        if (xx - x) % 6 == 0 {
            cv.line(xx, y + 8, xx, y + h - 1, 'I');
        }
    }
    // A short roof.
    cv.rect(x - 3, y, w + 6, 8, 'B');
    cv.line(x - 3, y, x + w + 2, y, 'o');
    // A Dutch door: top open on the dark, bottom shut and braced.
    let (dx, dy) = (x + w / 2 - 10, y + 16);
    cv.rect(dx, dy, 20, 13, 'k');
    cv.rect(dx, dy + 13, 20, h - 29, 'r');
    cv.line(dx, dy + 13, dx + 19, y + h - 1, 'o');
    cv.frame(dx - 1, dy - 1, 22, h - 15, 'o');
    // His brass plate, and a rosette.
    let (bx, by) = (x + w / 2 - 19, y + 9);
    let tw = super::super::ink::text_width("HELIOS");
    super::super::chrome::inset_panel(cv, (bx, by - 2, tw + 8, 10), super::super::chrome::Tone::NAMEPLATE);
    cv.text(bx + 4, by, "HELIOS", '6');
    let (rx, ry) = (x + w - 9, y + 20);
    cv.ellipse(rx as f32, ry as f32, 3.5, 3.5, '1');
    cv.ellipse(rx as f32, ry as f32, 1.5, 1.5, '6');
    cv.line(rx - 1, ry + 3, rx - 3, ry + 9, '1');
    cv.line(rx + 1, ry + 3, rx + 3, ry + 9, '1');
    // A horseshoe over the door, points up for luck.
    let (hx, hy) = (x + w / 2, y + 11 + (tick / 60 % 2) as i32 * 0);
    let shoe = Img::from_rows(&["v...v", "v...v", ".vvv."]);
    cv.stamp(&shoe, hx - 2 + 24, hy - 1);
}

fn stud_book(cv: &mut Img) {
    let (x, y, w, h) = px(STUD_BOOK);
    // Clean straw laid where the mare is led out.
    for yy in y + 2..y + h - 1 {
        for xx in x + 1..x + w - 1 {
            cv.put(xx, yy, if hash(xx, yy, 8) % 4 == 0 { 'O' } else { 't' });
        }
    }
    let (lx, ly, _, _) = px(LECTERN);
    cv.rect(lx + 6, ly + 6, 4, 10, 'P');
    cv.rect(lx + 2, ly + 2, 12, 5, 'r');
    cv.rect(lx + 3, ly + 1, 5, 4, 'c');
    cv.rect(lx + 8, ly + 1, 5, 4, '9');
    cv.line(lx + 12, ly - 3, lx + 10, ly + 1, 'H');
}

fn rail(cv: &mut Img) {
    let (x, y, w, _) = px(RAIL);
    for post in [x + 2, x + w - 5] {
        cv.rect(post, y + 4, 3, 22, 'P');
        cv.put(post + 1, y + 4, 'o');
    }
    cv.rect(x + 2, y + 8, w - 4, 3, 'r');
    cv.line(x + 2, y + 8, x + w - 3, y + 8, 'o');
    // A bucket and the grooming box.
    cv.rect(x + w + 2, y + 18, 7, 7, 'J');
    cv.rect(x + w + 3, y + 19, 5, 2, 'q');
    cv.rect(x - 10, y + 18, 9, 6, 'B');
    cv.line(x - 9, y + 17, x - 3, y + 17, 'o');
}

fn hay(cv: &mut Img) {
    let (x, y, w, _) = px(HAY);
    for (bx, by) in [(x, y + 4), (x + w / 2, y + 4), (x + w / 4, y - 4)] {
        cv.rect(bx, by, w / 2 - 1, 9, 'O');
        for xx in bx..bx + w / 2 - 1 {
            if hash(xx, by, 2) % 3 == 0 {
                cv.put(xx, by + 1 + (xx % 7), 't');
            }
        }
        cv.line(bx + 4, by, bx + 4, by + 8, 'P');
        cv.line(bx + w / 2 - 6, by, bx + w / 2 - 6, by + 8, 'P');
    }
}

fn paddock(cv: &mut Img) {
    let (x, y, w, h) = px(PADDOCK);
    for yy in y..y + h {
        for xx in x..x + w {
            let n = vnoise(xx as f32 / 10.0, yy as f32 / 8.0, 61);
            cv.put(xx, yy, if n > 0.62 { 'P' } else if n > 0.4 { 'B' } else { 'I' });
            if hash(xx, yy, 19) % 61 == 0 {
                cv.put(xx, yy, 'b');
            }
        }
    }
    // Split rails along the near and far sides.
    for (foot, seed) in [(y + 1, 1), (y + h, 2)] {
        m::rail_fence(cv, (x - 1, x + w + 1), foot, &[foot - 7, foot - 3], 16, ('o', 'r'), 60 + seed);
    }
    for yy in (y..y + h).step_by(16) {
        cv.rect(x - 1, yy, 3, 9, 'P');
        cv.rect(x + w - 1, yy, 3, 9, 'P');
    }
}

/// The ranch yard at morning.
fn ranch(cv: &mut Img, tick: u32) {
    static STILL: std::sync::OnceLock<Img> = std::sync::OnceLock::new();
    still(cv, &STILL, ranch_ground);
    stall(cv, tick);
    stud_book(cv);
    rail(cv);
    hay(cv);
    paddock(cv);
    fence(cv, &[RANCH_OUT]);
    gate(cv, RANCH_OUT, true);
    gate(cv, FIELDS_GATE, false);
    sun(cv);
}

fn ranch_ground(cv: &mut Img) {
    let w = cv.w;
    let paths = [(1, 5, 22, 2), (1, 6, 2, 7), (19, 1, 3, 5), (8, 7, 4, 3)];
    for y in TILE..cv.h {
        for x in 0..w {
            let (tx, ty) = (x / TILE, y / TILE);
            let verge = tx <= 1 || tx >= 22 || ty >= 12 || ty <= 1;
            let on_path = paths
                .iter()
                .any(|&(c, r, pw, ph)| tx >= c && tx < c + pw && ty >= r && ty < r + ph);
            if on_path {
                cv.put(x, y, gravel(x, y));
            } else if verge {
                grass(cv, x, y, w);
            } else {
                // Trodden mud in soft patches, wetter where it is darker.
                let n = vnoise(x as f32 / 20.0, y as f32 / 14.0, 33) + (vnoise(x as f32 / 4.0, y as f32 / 3.0, 34) - 0.5) * 0.1;
                cv.put(x, y, if n > 0.72 { 'P' } else { 'B' });
                if hash(x, y, 29) % 97 == 0 {
                    cv.put(x, y, if hash(x, y, 30) % 2 == 0 { 'O' } else { 't' });
                }
            }
        }
    }
    flowers(cv, 0, TILE, w, cv.h - TILE, |x, y| {
        let (tx, ty) = (x / TILE, y / TILE);
        tx <= 1 || tx >= 22 || ty >= 12 || (ty <= 1 && tx >= 17)
    });
    tree_line(cv);
    barn(cv);
}

/// The two rooms' scenery.
pub fn scenery(run: &Run) -> Img {
    let room = run.room();
    let mut cv = Img::black(room.cols as i32 * TILE, room.rows as i32 * TILE);
    let tick = run.tick as u32;
    match room.kind {
        RoomKind::Pasture => pasture(&mut cv, tick),
        _ => ranch(&mut cv, tick),
    }
    cv
}

/// The ranch gate in the stables yard: a timber arch with a horseshoe.
pub fn stables_gate(cv: &mut Img) {
    let (x, y, w, _) = px(r::STABLES_GATE);
    for post in [x - 2, x + w - 1] {
        cv.rect(post, y - 14, 3, 18, 'P');
        cv.put(post + 1, y - 14, 'o');
    }
    cv.rect(x - 2, y - 16, w + 4, 3, 'r');
    cv.line(x - 2, y - 16, x + w + 1, y - 16, 'o');
    let shoe = Img::from_rows(&["v...v", "v...v", ".vvv."]);
    cv.stamp(&shoe, x + w / 2 - 2, y - 12);
}

/// The ranch's groom, brush going.
fn groom(tick: u32) -> Img {
    let up = (tick / 7) % 2 == 0;
    Img::from_rows(&[
        "....bbbb......",
        "...bBBBBb.....",
        "..bbbbbbbb....",
        "...OOOOOO.....",
        "...OkOOkO.....",
        "....OOOO......",
        "...2222222....",
        "..222222222...",
        "..O2222222O...",
        if up { "...2222222Ott" } else { "...2222222O..." },
        if up { "...PPPPPPP...." } else { "...PPPPPPPOtt." },
        "...PPPPPPP....",
        "...PPP.PPP....",
        "....nn..nn....",
        "...nnn..nnn...",
    ])
}

/// Feet at tile coordinates.
fn feet(tiles: (f32, f32)) -> (i32, i32) {
    at(tiles.0 * TILE_UNITS, tiles.1 * TILE_UNITS)
}

fn face(im: Img, east: bool) -> Img {
    if east { im } else { im.flip_h() }
}

/// The horses and the folk of the two rooms.
pub(super) fn figures_with(cv: &mut Img, run: &Run, modern: bool) {
    let kind = run.room().kind;
    if !r::is_room(kind) {
        return;
    }
    let tick = run.tick as u32;
    let near = |spot: (f32, f32), tiles: f32| {
        run.players.values().any(|h| {
            h.hp > 0
                && (h.x - spot.0 * TILE_UNITS).hypot(h.y - spot.1 * TILE_UNITS) < tiles * TILE_UNITS
        })
    };
    if kind == RoomKind::Pasture {
        let mut horses: Vec<((f32, f32), Img)> = Vec::new();
        if !modern {
            for (i, ((x, y), east)) in r::grazers(run.tick).into_iter().enumerate() {
                let pose = if (tick / 90 + i as u32) % 3 == 0 { Pose::Stand } else { Pose::Graze };
                horses.push(((x, y), face(draft(MARES[i], pose, 0.62, None, false), east)));
            }
        }
        if let Some(foal) = r::herd_foal(&run.home.stable) {
            let k = match r::age(foal, run.home.ranch.wins) {
                Age::Foal => 0.36,
                Age::Yearling => 0.5,
                Age::Grown => 0.7,
            };
            let ((gx, gy), _) = r::grazers(run.tick)[1];
            let skip = (tick / 120) % 2 == 0;
            let (pose, at) = if skip {
                (Pose::Gallop(tick / 3), (gx + 2.2 + ((tick / 4) % 30) as f32 / 10.0, gy + 1.2))
            } else {
                (Pose::Stand, (gx + 2.4, gy + 1.0))
            };
            let mut hide = HELIOS;
            hide.feather = 'T';
            horses.push((at, draft(hide, pose, k, None, false)));
        }
        let ((hx, hy), east, gait) = r::helios(run.tick);
        if !modern {
            let pose = gait.map_or(Pose::Stand, Pose::Gallop);
            let fire = gait.map(|_| tick);
            horses.push(((hx, hy), face(draft(HELIOS, pose, 0.82, fire, false), east)));
        }
        horses.sort_by(|a, b| a.0.1.total_cmp(&b.0.1));
        for ((x, y), im) in &horses {
            let (px, py) = feet((*x, *y));
            stand(cv, im, px, py);
        }
        if near((hx, hy), 4.0) {
            let board = home::board(&[("HELIOS".into(), '6'), ("PRIDE OF THE RANCH".into(), 'O')]);
            let (px, py) = feet((hx, hy));
            let top = py - 54 - board.h;
            let y = if top < 2 { py + 4 } else { top };
            cv.stamp(&board, (px - board.w / 2).clamp(2, cv.w - board.w - 2), y);
        }
        return;
    }
    // The ranch: Old Barley at the rail, his groom, the war horses.
    let barley = draft(BARLEY, Pose::Stand, 0.76, None, true);
    let (bx, by) = feet((DRAFT_AT.0, DRAFT_AT.1 + 1.2));
    if !modern { stand(cv, &barley, bx, by); }
    // He shines: a glint travels along his back.
    let glint = (tick / 3) as i32 % 50;
    let glint = glint * 3 / 5;
    cv.put(bx - 15 + glint, by - 33, '9');
    cv.put(bx - 14 + glint, by - 33, 'c');
    let (gx, gy) = feet((GROOM_AT.0, GROOM_AT.1 + 0.9));
    stand(cv, &groom(tick), gx, gy);
    let (px, py, pw, ph) = px(PADDOCK);
    let (cx, cy) = (px as f32 + pw as f32 / 2.0, py as f32 + ph as f32 / 2.0 + 8.0);
    if !modern { for (i, hide) in WAR.iter().enumerate() {
        let a = tick as f32 / 70.0 + i as f32 * std::f32::consts::PI / 2.0;
        let trotting = i != 1;
        let (x, y) = if trotting {
            (cx + 18.0 + 30.0 * a.cos(), cy + 12.0 * a.sin())
        } else {
            (cx - 46.0, cy + 4.0)
        };
        let east = a.sin() < 0.0;
        let pose = if trotting { Pose::Gallop(tick / 5 + i as u32) } else { Pose::Graze };
        let im = face(draft(*hide, pose, 0.58, None, false), east || !trotting);
        stand(cv, &im, x as i32, y as i32);
    }
    }
    let book = (
        STUD_BOOK.0 as f32 + STUD_BOOK.2 as f32 / 2.0,
        STUD_BOOK.1 as f32 + 0.5,
    );
    if near(book, 2.6) {
        let lines = match r::herd_foal(&run.home.stable) {
            Some(f) => vec![
                ("THE STUD BOOK".to_string(), '6'),
                (format!("{} IS IN THE FIELDS", f.name.to_uppercase()), 'O'),
            ],
            None => vec![
                ("THE STUD BOOK: HOLD E".to_string(), '6'),
                (format!("HERD MARE MEETS HELIOS, {} GOLD", r::FEE), 'O'),
            ],
        };
        let board = home::board(&lines);
        let (bx, _) = feet(book);
        cv.stamp(&board, (bx - board.w / 2).clamp(2, cv.w - board.w - 2), 1);
    }
}

#[cfg(test)]
#[path = "../../../../tests/render/overworld__ranch_shots.rs"]
mod tests;

/// The same gambrel barn at kingdom scale.
pub fn landmark() -> Img {
    let mut full = Img::new(384, 224);
    barn(&mut full);
    let mut small = Img::new(64, 36);
    for y in 0..36 {
        for x in 0..64 {
            if let Some(c) = full.get(32 + x * 2, 10 + y * 2) {
                small.set(x, y, c);
            }
        }
    }
    small
}
