use super::super::super::ink::Img;
use super::super::class_looks::Look;
use super::{kit, paint::*};
use crate::rules::{Enemy, Hero};

/// Whether this canvas draws knight `id`: every knight but those in a
/// state of their own (crouched at the works, veiled, in their own picture
/// or a card's dress, a frog, stone, rimed, rolling, tossed, in the privy
/// or at the pit stop), which go to v1 whole.
pub fn knight_supported(
    run: &crate::rules::Run,
    h: &Hero,
    id: u32,
) -> bool {
    !run.works.kits.get(&id).is_some_and(|kit| kit.duck)
        && !h.has_rune(crate::rules::runes::RuneKind::Invisibility)
        && h.avatar.is_none()
        && h.worn(&run.book).is_empty()
        && !h.frog()
        && h.possessed.is_none()
        && !h.stone
        && h.chilled == 0
        && h.rolling().is_none()
        && h.tossed == 0
        && h.privy == 0
        && h.pit.stop.is_none()
}

/// The seats the kit's pose studies are painted for: red, blue, green, gold.
const STUDIED_SEATS: u32 = 4;

/// Whether the kit's own pose studies show this knight: the realm's knight
/// whole, in no company's colours, the shield on the arm or raised across.
/// Any other knight (a class, a stage of dress, a shield thrown or raised
/// toward or away, the dwarf, a kit's colours, or one of the two seats past
/// the four the studies are painted for) is the arena's own drawing brought
/// up to this canvas.
fn studied(h: &Hero, thrown: bool, id: u32) -> bool {
    Look::of(h).is_none()
        && id <= STUDIED_SEATS
        && !thrown
        && !(h.shielding && h.aim_y.abs() > h.aim_x.abs() * 1.25)
        && h.knight.as_deref() != Some(super::super::knights::DWARF)
        && h.gear == 0
        && h.colours().is_empty()
}

pub fn knight(cv: &mut Canvas, x: f32, y: f32, h: &Hero, _tick: u64, id: u32, thrown: bool) {
    if !studied(h, thrown, id) {
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
            material('c'),
            if h.struck > 6 { 0.40 } else { 0.15 },
        );
    }
}
pub fn skeleton(cv: &mut Canvas, x: f32, y: f32, e: &Enemy, tick: u64) {
    let im = foe_hit_sprite("skeleton", e.hurt);
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

/// A knight as the arena draws them (a class as its class; the realm's knight
/// at their stage of dress, shield and colours; facing, stride, strike, seat
/// trim), brought up to this canvas at three samples a pixel with smoothed
/// edges, lit from the upper left and shaded at the lower right, inside a
/// dark coloured edge.
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
    let im = hd(&im);
    cv.shadow(x, y + 1.0, 9.0, 2.4, 0.65);
    kit::sprite(
        cv,
        &im,
        x - im.w as f32 * 0.25,
        y - im.h as f32 * 0.5,
        true,
        1.0,
        None,
        0.0,
    );
    if h.struck > 0 {
        cv.flat_ellipse(
            x,
            y - 10.0,
            6.5,
            8.0,
            material('c'),
            if h.struck > 6 { 0.40 } else { 0.15 },
        );
    }
}

/// `lit(scale3x(im))`, kept: a knight's figure is one of a few dozen
/// drawings, so each is brought up to this canvas once per thread.
fn hd(im: &Img) -> Img {
    use std::hash::{Hash, Hasher};
    thread_local! {
        static HD: std::cell::RefCell<std::collections::HashMap<u64, Img>> =
            std::cell::RefCell::new(std::collections::HashMap::new());
    }
    let mut key = std::collections::hash_map::DefaultHasher::new();
    (im.w, im.h, im.words()).hash(&mut key);
    let key = key.finish();
    if let Some(done) = HD.with(|m| m.borrow().get(&key).cloned()) {
        return done;
    }
    let done = lit(&scale3x(im));
    HD.with(|m| {
        let mut m = m.borrow_mut();
        if m.len() >= 512 {
            m.clear();
        }
        m.insert(key, done.clone());
    });
    done
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
                    if (d == b && e != c) || (b == f && e != a) {
                        b
                    } else {
                        e
                    },
                    if b == f { f } else { e },
                    if (d == b && e != g) || (d == hh && e != a) {
                        d
                    } else {
                        e
                    },
                    e,
                    if (b == f && e != i) || (hh == f && e != c) {
                        f
                    } else {
                        e
                    },
                    if d == hh { d } else { e },
                    if (d == hh && e != i) || (hh == f && e != g) {
                        hh
                    } else {
                        e
                    },
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

/// Creature-only finishing of the local pose studies. Quantised material
/// planes tint the source's material detail; source alpha and pivots are exact.
/// Knights and mounts continue to use their own unmodified kit samples.
pub(super) fn foe_sprite(name: &str) -> &'static Img {
    use std::sync::OnceLock;
    macro_rules! pose {
        ($($n:literal),*) => { $(if name == $n {
            static POSE: OnceLock<Img> = OnceLock::new();
            return POSE.get_or_init(|| finish_foe(kit::get($n), $n));
        })* };
    }
    pose!(
        "ghost",
        "slime",
        "skeleton",
        "imp",
        "ripper",
        "engine-boiler"
    );
    panic!("unknown creature pose {name}");
}

fn finish_foe(src: &Img, name: &str) -> Img {
    let bone = [
        [43, 48, 66],
        [104, 116, 137],
        [169, 177, 174],
        [225, 217, 191],
    ];
    let steel = [[27, 39, 54], [61, 80, 97], [110, 137, 149], [175, 192, 192]];
    let brass = [[61, 38, 50], [124, 76, 50], [185, 128, 65], [228, 184, 109]];
    let green = [[27, 48, 51], [59, 88, 54], [105, 139, 64], [171, 190, 105]];
    let cyan = [
        [25, 48, 65],
        [59, 111, 132],
        [114, 173, 182],
        [196, 222, 216],
    ];
    let mut out = Img::new(src.w, src.h);
    for y in 0..src.h {
        for x in 0..src.w {
            let Some(c) = src.get(x, y) else {
                continue;
            };
            let [r, g, b] = c.map(u32::from);
            let l = (r * 3 + g * 6 + b) / 10;
            let ramp = match name {
                "ghost" => &cyan,
                "slime" => &green,
                "imp" => &brass,
                "ripper" if (18..66).contains(&y) || (y >= 66 && g * 10 > r * 9) => &green,
                "skeleton" if r > b + 25 && r * 10 > g * 12 => &brass,
                "skeleton" => &bone,
                "engine-boiler" if r.abs_diff(g) < 25 && r > 125 && b < g + 20 => &bone,
                "ripper" => &brass,
                "engine-boiler" if r > b + 28 => &brass,
                _ => &steel,
            };
            let stops = if std::ptr::eq(ramp, &green) {
                [40, 92, 148]
            } else {
                [48, 112, 183]
            };
            let mut step = stops.iter().filter(|&&stop| l > stop).count();
            let edge_top = src.get(x, y - 1).is_none();
            let edge_left = src.get(x - 1, y).is_none();
            let edge_down = src.get(x, y + 1).is_none();
            let edge_right = src.get(x + 1, y).is_none();
            if (edge_top || edge_left) && l > 80 {
                step = (step + 1).min(3);
            } else if (edge_down || edge_right) && l < 160 {
                step = step.saturating_sub(1);
            }
            let mut c: [u8; 3] = ramp[step];
            // Preserve the little furnace cores, with a toned warm peak.
            if name == "engine-boiler"
                && (40..63).contains(&y)
                && ((17..30).contains(&x) || (34..48).contains(&x))
                && r * 10 > g * 12
                && r > 160
            {
                c = if g > 105 {
                    [238, 198, 124]
                } else {
                    [176, 77, 45]
                };
            }
            // Keep the source's ridges, pores, specular gel, cloth and warm
            // furnace falloff within the new ramp. Quantisation alone erased
            // those differences even when the silhouette stayed identical.
            let source_weight = if name == "ripper" { 1 } else { 2 };
            let c = std::array::from_fn(|i| {
                ((u32::from(src.get(x, y).unwrap()[i]) * source_weight
                    + u32::from(c[i]) * (4 - source_weight))
                    / 4) as u8
            });
            out.set(x, y, c);
        }
    }
    out
}

/// Three cached views of each authored creature: rest, settling hit, held hit.
/// Coverage, pose detail and every pivot come directly from the rest picture.
pub(super) fn foe_hit_sprite(name: &str, hurt: u8) -> &'static Img {
    use std::sync::OnceLock;
    if hurt == 0 {
        return foe_sprite(name);
    }
    let make = || {
        [false, true].map(|strong| {
            let src = foe_sprite(name);
            let mut out = Img::new(src.w, src.h);
            for y in 0..src.h {
                for x in 0..src.w {
                    if let Some(c) = src.get(x, y) {
                        out.set(
                            x,
                            y,
                            if name == "engine-boiler" && strong {
                                super::super::marks::hit_metal_rgb(c)
                            } else {
                                super::super::marks::hit_rgb(c, strong)
                            },
                        );
                    }
                }
            }
            out
        })
    };
    macro_rules! hit { ($($n:literal),*) => { $(if name == $n {
        static POSES: OnceLock<[Img; 2]> = OnceLock::new();
        return &POSES.get_or_init(make)[usize::from(hurt >= 4)];
    })* }; }
    hit!(
        "ghost",
        "slime",
        "skeleton",
        "imp",
        "ripper",
        "engine-boiler"
    );
    panic!("unknown creature pose {name}");
}
