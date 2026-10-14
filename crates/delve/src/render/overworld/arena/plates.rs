//! Dark-deco intertitles, painted on black at 384×224.
//! Atmospheric depth, material detail and light spill are baked once. The
//! cached searchlight phases leave only a few flicker/particle pixels per frame.

use super::super::ink::{BLACK, Img, Rgb, bayer, hash, ink};
use crate::rules::floor_plates::{DESTINATIONS, DURATION, Kind, Playing};
use std::sync::OnceLock;

pub const W: i32 = 384;
pub const H: i32 = 224;
const DARK: char = 'k';
const SLATE: char = 's';
const CRIMSON: char = 'χ';
const NAVY: char = 'ν';
const MUSTARD: char = 'μ';
const HORIZON: i32 = 188;

struct Layers {
    sky: Img,
    architecture: Img,
    light: (i32, i32),
}

struct Scene {
    sweeps: Vec<Img>,
    light: (i32, i32),
}

fn scene(sky: Img, architecture: Img, light: (i32, i32)) -> Layers {
    Layers {
        sky,
        architecture,
        light,
    }
}

fn poly(im: &mut Img, pts: &[(i32, i32)], c: char) {
    let lo = pts.iter().map(|p| p.1).min().unwrap_or(0).max(0);
    let hi = pts.iter().map(|p| p.1).max().unwrap_or(0).min(H);
    for y in lo..hi {
        let mut xs = Vec::with_capacity(pts.len());
        for i in 0..pts.len() {
            let (a, b) = (pts[i], pts[(i + 1) % pts.len()]);
            if (a.1 <= y && b.1 > y) || (b.1 <= y && a.1 > y) {
                xs.push(a.0 + (y - a.1) * (b.0 - a.0) / (b.1 - a.1));
            }
        }
        xs.sort_unstable();
        for pair in xs.chunks_exact(2) {
            im.rect(pair[0], y, pair[1] - pair[0] + 1, 1, c);
        }
    }
}

// A restricted selection of existing master inks for paletted light mixing.
// Quantization happens only while building the cache, never during playback.
const TONES: [char; 25] = [
    'k', 's', 'x', 'S', 'u', 'U', 'v', 'ν', 'χ', '7', '8', 'b', 'I', 'B', 'P', 'r', 'R', 'o', 't',
    '@', '6', 'X', 'g', 'μ', 'n',
];
fn tint(im: &mut Img, x: i32, y: i32, light: Rgb, amount: f32) {
    let Some(base) = im.get(x, y) else { return };
    let a = amount.clamp(0.0, 1.0);
    if a < 0.005 {
        return;
    }
    let wanted =
        std::array::from_fn::<_, 3, _>(|i| base[i] as f32 * (1.0 - a) + light[i] as f32 * a);
    let mut best = [(f32::MAX, BLACK); 2];
    for ch in TONES {
        let c = ink(ch).unwrap();
        let d = (0..3)
            .map(|i| (c[i] as f32 - wanted[i]).powi(2))
            .sum::<f32>();
        if d < best[0].0 {
            best[1] = best[0];
            best[0] = (d, c);
        } else if d < best[1].0 {
            best[1] = (d, c);
        }
    }
    let (c0, c1) = (best[0].1, best[1].1);
    let denom = (0..3)
        .map(|i| (c1[i] as f32 - c0[i] as f32).powi(2))
        .sum::<f32>();
    let fraction = (0..3)
        .map(|i| (wanted[i] - c0[i] as f32) * (c1[i] as f32 - c0[i] as f32))
        .sum::<f32>()
        / denom.max(1.0);
    im.set(
        x,
        y,
        if bayer(x, y) < fraction.clamp(0.0, 1.0) {
            c1
        } else {
            c0
        },
    );
}

fn glow(im: &mut Img, cx: i32, cy: i32, rx: i32, ry: i32, strength: f32) {
    for y in (cy - ry).max(0)..(cy + ry).min(HORIZON) {
        for x in (cx - rx).max(0)..(cx + rx).min(W) {
            let d = ((x - cx) as f32 / rx as f32).powi(2) + ((y - cy) as f32 / ry as f32).powi(2);
            if d < 1.0 {
                tint(im, x, y, ink('@').unwrap(), strength * (1.0 - d).powi(2));
            }
        }
    }
}

fn sky(im: &mut Img, deep: bool) {
    let bands: &[(i32, char)] = if deep {
        &[(0, 's'), (92, 'x'), (136, NAVY), (178, 'u')]
    } else {
        &[(0, CRIMSON), (49, '7'), (109, 'R'), (157, 'o')]
    };
    for y in 0..HORIZON {
        for x in 0..W {
            let mut c = bands[0].1;
            for pair in bands.windows(2) {
                let edge = pair[1].0 + (x as f32 / 109.0).sin() as i32 * 2;
                let t = ((y - edge + 9) as f32 / 18.0).clamp(0.0, 1.0);
                if bayer(x, y) < t {
                    c = pair[1].1;
                }
            }
            im.put(x, y, c);
        }
    }
    if !deep {
        // Long sunset ribbons feather into the gradient, with no hard stripe edges.
        for (cy, slope, thick) in [(37.0, 0.04, 7.0), (82.0, -0.05, 6.0), (116.0, 0.025, 4.0)] {
            for y in 0..150 {
                for x in 0..W {
                    let centre = cy + slope * x as f32 + (x as f32 / 63.0).sin() * 2.0;
                    let a = (1.0 - (y as f32 - centre).abs() / thick).max(0.0) * 0.42;
                    tint(im, x, y, ink(CRIMSON).unwrap(), a);
                }
            }
        }
    }
}

fn masonry(im: &mut Img, x: i32, y: i32, w: i32, h: i32, c: char) {
    for yy in (y..y + h).step_by(9) {
        for xx in (x..x + w).step_by(2) {
            if hash(xx / 2, yy, 41) % 5 != 0 {
                im.put(xx, yy, c);
            }
        }
        for xx in (x + ((yy / 9) % 2) * 9..x + w).step_by(18) {
            im.line(xx, yy + 1, xx, (yy + 8).min(y + h), c);
        }
    }
}

fn window(im: &mut Img, x: i32, y: i32, w: i32, h: i32, lit: char) {
    im.rect(x, y + w / 2, w, h - w / 2, lit);
    poly(
        im,
        &[(x, y + w / 2), (x + w / 2, y), (x + w - 1, y + w / 2)],
        lit,
    );
    im.line(x + w / 2, y + 2, x + w / 2, y + h, DARK);
    if h > 15 {
        im.line(x, y + h * 2 / 3, x + w, y + h * 2 / 3, DARK);
    }
}

fn spire(im: &mut Img, x: i32, y: i32, w: i32, bottom: i32, c: char) {
    let roof = w * 2;
    im.rect(x - w / 2, y + roof, w, bottom - y - roof, c);
    poly(
        im,
        &[(x, y), (x - w / 2 - 2, y + roof), (x + w / 2 + 2, y + roof)],
        c,
    );
    im.line(x, y - 6, x, y + 5, c);
    im.line(x - 2, y - 3, x + 2, y - 3, c);
    if c == DARK {
        poly(
            im,
            &[
                (x, y + 5),
                (x - w / 2 + 1, y + roof - 2),
                (x - 1, y + roof - 2),
            ],
            SLATE,
        );
        im.rect(x - w / 2 - 2, y + roof, w + 4, 3, 'x');
        im.line(x + w / 2 - 1, y + roof + 4, x + w / 2 - 1, bottom, SLATE);
        if w > 10 {
            window(im, x - 3, y + roof + 12, 5, 17, 'o');
        }
    }
}

fn battlement(im: &mut Img, x: i32, y: i32, w: i32, bottom: i32, c: char) {
    im.rect(x, y + 5, w, bottom - y - 5, c);
    for xx in (x..x + w - 3).step_by(10) {
        im.rect(xx, y, 6, 8, c);
    }
    if c == DARK {
        im.rect(x, y + 12, w, 2, SLATE);
    }
}

fn skyline(im: &mut Img, near: bool) {
    let c = if near { 's' } else { 'x' };
    let base = if near { 148 } else { 142 };
    battlement(im, 0, base, W, 177, c);
    for (i, x) in [12, 46, 80, 120, 151, 192, 227, 264, 303, 337, 370]
        .into_iter()
        .enumerate()
    {
        let y = if near {
            86 + (i as i32 * 17 % 47)
        } else {
            105 + (i as i32 * 13 % 30)
        };
        spire(
            im,
            x + if near { 10 } else { 0 },
            y,
            if near { 9 } else { 6 },
            174,
            c,
        );
        if i % 3 == 1 {
            im.rect(x, y + 27, 2, 5, if near { 'o' } else { 't' });
        }
    }
}

fn horizon_haze(im: &mut Img) {
    for y in 117..177 {
        let amount = (y - 117) as f32 / 60.0 * 0.23;
        for x in 0..W {
            if [ink('s'), ink('x'), ink(NAVY)].contains(&im.get(x, y)) {
                tint(im, x, y, ink('u').unwrap(), amount);
            }
        }
    }
}

fn rose(im: &mut Img, x: i32, y: i32, r: i32) {
    im.ellipse(x as f32, y as f32, (r + 4) as f32, (r + 4) as f32, 'x');
    im.ellipse(x as f32, y as f32, (r + 2) as f32, (r + 2) as f32, DARK);
    im.ellipse(x as f32, y as f32, r as f32, r as f32, 'o');
    im.ellipse(x as f32, y as f32, (r - 2) as f32, (r - 2) as f32, '@');
    for i in 0..8 {
        let a = i as f32 * std::f32::consts::TAU / 8.0;
        let (dx, dy) = (
            (a.cos() * r as f32 * 0.61) as i32,
            (a.sin() * r as f32 * 0.61) as i32,
        );
        im.ellipse(
            (x + dx) as f32,
            (y + dy) as f32,
            (r / 4 + 1) as f32,
            (r / 4 + 1) as f32,
            DARK,
        );
        im.ellipse(
            (x + dx) as f32,
            (y + dy) as f32,
            (r / 4 - 1).max(1) as f32,
            (r / 4 - 1).max(1) as f32,
            't',
        );
        im.line(
            x,
            y,
            x + (a.cos() * r as f32) as i32,
            y + (a.sin() * r as f32) as i32,
            DARK,
        );
    }
    im.ellipse(x as f32, y as f32, 4.0, 4.0, DARK);
    im.ellipse(x as f32, y as f32, 2.0, 2.0, '6');
}

fn flame(im: &mut Img, x: i32, y: i32, size: i32) {
    poly(
        im,
        &[
            (x - size, y + 2),
            (x - size, y - size / 2),
            (x - size * 2 / 3, y - size),
            (x - size / 2, y - size * 2),
            (x - size / 3, y - size * 3 / 2),
            (x, y - size * 3),
            (x + size / 3, y - size * 2),
            (x + size / 6, y - size),
            (x + size / 2, y - size * 3 / 2),
            (x + size, y - size * 2),
            (x + size * 3 / 4, y - size / 3),
            (x + size, y + 2),
        ],
        'R',
    );
    poly(
        im,
        &[
            (x - size / 2, y + 1),
            (x - size * 2 / 5, y - size / 2),
            (x - size / 4, y - size),
            (x - size / 3, y - size * 3 / 2),
            (x, y - size * 2),
            (x + size / 5, y - size * 5 / 4),
            (x + size / 6, y - size / 2),
            (x + size / 2, y - size),
            (x + size * 2 / 3, y - size / 2),
            (x + size / 2, y),
            (x, y + 3),
        ],
        '@',
    );
    if size >= 6 {
        poly(
            im,
            &[
                (x - 2, y + 1),
                (x - size / 5, y - size / 2),
                (x + 1, y - size),
                (x + size / 5, y - size / 3),
                (x + 2, y + 2),
            ],
            '6',
        );
    } else {
        im.line(x, y - size, x, y + 1, '6');
    }
}

fn torch(im: &mut Img, x: i32, y: i32) {
    glow(im, x, y - 3, 24, 29, 0.38);
    im.line(x - 3, y + 15, x, y + 6, 'r');
    im.rect(x - 5, y + 2, 10, 3, DARK);
    flame(im, x, y, 3);
}

fn courtyard(im: &mut Img, vanish: i32, y: i32) {
    im.rect(0, y, W, HORIZON - y, DARK);
    for xx in (-180..600).step_by(54) {
        im.line(vanish, y, xx, HORIZON, 's');
    }
    for yy in [y + 5, y + 13, y + 26, 185] {
        im.line(0, yy, W, yy, 's');
    }
}

fn near_wall(im: &mut Img, left: bool) {
    let (a, b) = if left { (0, 70) } else { (383, 316) };
    poly(im, &[(a, 95), (b, 137), (b, 188), (a, 188)], DARK);
    for i in 0..6 {
        let x = a + (b - a) * i / 6;
        let y = 88 + i * 7;
        let dx = if left { 8 } else { -8 };
        poly(
            im,
            &[(x, y), (x + dx, y + 5), (x + dx, y + 18), (x, y + 13)],
            DARK,
        );
        im.line(x, y + 21, x, HORIZON, 's');
    }
    im.line(a, 118, b, 155, 'x');
    im.line(a, 149, b, 172, 's');
}

fn chapel(im: &mut Img, x: i32) {
    im.rect(x - 34, 60, 68, 115, DARK);
    poly(im, &[(x, 17), (x - 38, 63), (x + 38, 63)], DARK);
    poly(im, &[(x, 24), (x - 29, 59), (x - 4, 51)], 's');
    spire(im, x - 39, 6, 16, 178, DARK);
    spire(im, x + 39, 28, 12, 178, DARK);
    im.line(x, 6, x, 20, DARK);
    im.line(x - 4, 11, x + 4, 11, DARK);
    masonry(im, x - 27, 100, 54, 70, 's');
    glow(im, x, 78, 38, 45, 0.17);
    rose(im, x, 78, 16);
    window(im, x - 17, 108, 9, 31, 't');
    window(im, x + 8, 108, 9, 31, 't');
    window(im, x - 13, 145, 26, 32, 'x');
    window(im, x - 9, 149, 18, 28, 'o');
    window(im, x - 5, 152, 10, 25, '@');
    for dx in [-30, 27] {
        im.rect(x + dx, 68, 3, 110, 'x');
    }
    for y in [100, 142] {
        im.line(x - 30, y, x + 30, y, 'x');
    }
}

fn crypt_exterior() -> Layers {
    let mut back = Img::black(W, H);
    sky(&mut back, false);
    skyline(&mut back, false);
    skyline(&mut back, true);
    horizon_haze(&mut back);
    let mut front = Img::new(W, H);
    battlement(&mut front, 93, 146, 290, 173, DARK);
    poly(
        &mut front,
        &[(192, 176), (192, 125), (214, 102), (240, 126), (240, 176)],
        DARK,
    );
    spire(&mut front, 211, 44, 18, 178, DARK);
    window(&mut front, 207, 129, 7, 23, 'o');
    chapel(&mut front, 282);
    courtyard(&mut front, 282, 176);
    glow(&mut front, 282, 174, 65, 24, 0.30);
    // Graveyard hill and ironwork in the near plane.
    poly(
        &mut front,
        &[
            (0, 148),
            (58, 143),
            (121, 160),
            (178, 182),
            (196, 188),
            (0, 188),
        ],
        DARK,
    );
    for (i, x) in [17, 43, 77, 115, 152].into_iter().enumerate() {
        let y = 149 + i as i32 * 5;
        if i % 2 == 0 {
            front.rect(x, y - 19, 4, 30, 'x');
            front.rect(x - 6, y - 12, 16, 4, 'x');
            front.line(x + 1, y - 18, x + 1, y + 9, 's');
        } else {
            window(&mut front, x, y - 13, 13, 23, 's');
            front.rect(x + 4, y - 5, 5, 14, DARK);
        }
    }
    for x in (7..192).step_by(9) {
        let y = 166 + x / 15;
        front.line(x, y - 12, x, y + 8, 's');
        poly(
            &mut front,
            &[(x, y - 16), (x - 2, y - 12), (x + 2, y - 12)],
            's',
        );
    }
    front.line(3, 165, 193, 179, 's');
    poly(
        &mut front,
        &[(6, 186), (22, 169), (54, 158), (60, 169), (24, 188)],
        'x',
    );
    poly(
        &mut front,
        &[(18, 179), (52, 161), (56, 167), (22, 182)],
        DARK,
    );
    torch(&mut front, 244, 171);
    torch(&mut front, 333, 164);
    scene(back, front, (282, 78))
}

fn gate() -> Layers {
    let mut back = Img::black(W, H);
    sky(&mut back, false);
    skyline(&mut back, false);
    skyline(&mut back, true);
    horizon_haze(&mut back);
    let mut front = Img::new(W, H);
    battlement(&mut front, 58, 112, 326, 157, DARK);
    masonry(&mut front, 60, 128, 320, 29, 's');
    // A gatehouse within the curtain wall, viewed across the arrival court.
    for (x, y, w) in [(150, 39, 36), (269, 31, 40)] {
        battlement(&mut front, x, y, w, 170, DARK);
        front.rect(x - 3, y + 17, w + 6, 4, 'x');
        masonry(&mut front, x + 3, y + 30, w - 6, 94, 's');
        window(&mut front, x + w / 2 - 4, y + 36, 9, 24, 'o');
    }
    poly(
        &mut front,
        &[(175, 170), (175, 91), (226, 51), (277, 90), (277, 170)],
        DARK,
    );
    poly(
        &mut front,
        &[(185, 169), (185, 101), (226, 65), (267, 101), (267, 169)],
        'x',
    );
    poly(
        &mut front,
        &[(194, 169), (194, 105), (226, 78), (258, 105), (258, 169)],
        DARK,
    );
    // Successive inner arches and raised teeth show the descent beyond.
    window(&mut front, 211, 108, 30, 61, 's');
    window(&mut front, 217, 119, 18, 49, DARK);
    front.rect(223, 132, 7, 30, 'o');
    glow(&mut front, 227, 146, 33, 38, 0.23);
    for x in (197..258).step_by(7) {
        front.rect(x, 94 + (x - 227).abs() / 2, 2, 16, 's');
    }
    front.line(196, 115, 257, 115, 's');
    rose(&mut front, 226, 87, 7);
    courtyard(&mut front, 226, 163);
    glow(&mut front, 226, 173, 49, 24, 0.29);
    for y in [165, 170, 178] {
        front.rect(197 - (y - 165), y, 59 + (y - 165) * 2, 2, 'x');
    }
    torch(&mut front, 188, 133);
    torch(&mut front, 266, 133);
    near_wall(&mut front, true);
    // Near left brazier and empty courtyard benches establish human scale.
    front.rect(76, 141, 6, 38, DARK);
    front.rect(68, 177, 21, 4, DARK);
    glow(&mut front, 79, 140, 30, 36, 0.28);
    flame(&mut front, 79, 141, 5);
    front.rect(70, 144, 19, 4, 'x');
    poly(
        &mut front,
        &[(303, 174), (348, 163), (366, 172), (321, 184)],
        DARK,
    );
    front.line(305, 173, 348, 163, 's');
    front.rect(316, 179, 4, 9, DARK);
    front.rect(357, 170, 4, 13, DARK);
    scene(back, front, (227, 147))
}

fn dragon(im: &mut Img, x: i32, y: i32) {
    // Curved scallops between long wing fingers, rather than triangular bat fins.
    for side in [-1, 1] {
        let p = |dx, dy| (x + dx * side, y + dy);
        let pts = [
            p(0, 14),
            p(18, -13),
            p(44, -24),
            p(81, -16),
            p(64, -14),
            p(55, -9),
            p(48, -1),
            p(45, 9),
            p(42, 5),
            p(37, 3),
            p(31, 6),
            p(27, 18),
            p(23, 13),
            p(18, 12),
            p(13, 18),
            p(8, 28),
            p(1, 26),
        ];
        poly(im, &pts, DARK);
        for (dx, dy) in [(79, -16), (45, 7), (27, 16), (9, 26)] {
            im.line(x + side * 8, y + 15, x + side * dx, y + dy, 's');
        }
        im.line(x + side * 8, y + 14, x + side * 39, y - 16, 's');
    }
    poly(
        im,
        &[
            (x - 5, y + 13),
            (x + 5, y + 12),
            (x + 10, y + 31),
            (x + 4, y + 40),
            (x - 5, y + 37),
            (x - 15, y + 51),
            (x - 31, y + 55),
            (x - 19, y + 48),
            (x - 9, y + 30),
        ],
        DARK,
    );
    poly(
        im,
        &[
            (x + 3, y + 20),
            (x + 14, y + 14),
            (x + 17, y + 6),
            (x + 20, y + 14),
            (x + 26, y + 12),
            (x + 29, y + 18),
            (x + 24, y + 23),
            (x + 13, y + 24),
            (x + 8, y + 33),
        ],
        DARK,
    );
    im.line(x + 6, y + 35, x + 18, y + 42, DARK);
    im.line(x + 18, y + 42, x + 22, y + 37, DARK);
    im.line(x - 1, y + 37, x - 8, y + 44, DARK);
    im.put(x + 23, y + 17, 'R');
}

fn keep() -> Layers {
    let mut back = Img::black(W, H);
    sky(&mut back, false);
    skyline(&mut back, false);
    skyline(&mut back, true);
    horizon_haze(&mut back);
    dragon(&mut back, 205, 43);
    let mut front = Img::new(W, H);
    for (x, y, w) in [(96, 57, 14), (152, 108, 8), (253, 79, 11), (301, 41, 15)] {
        spire(&mut front, x, y, w, 179, DARK);
    }
    battlement(&mut front, 0, 145, W, 183, DARK);
    for (x, y, w) in [(-7, -54, 53), (342, -45, 49)] {
        spire(&mut front, x + w / 2, y, w, 188, DARK);
        masonry(&mut front, x + 3, y + w * 2 + 9, w - 6, 95, 's');
        glow(&mut front, x + 30, 97, 28, 36, 0.22);
        window(&mut front, x + 26, 81, 11, 30, 't');
        front.rect(x - 5, 147, w + 10, 4, 'x');
    }
    window(&mut front, 296, 130, 8, 25, 'o');
    window(&mut front, 246, 132, 5, 17, 'o');
    courtyard(&mut front, 194, 175);
    near_wall(&mut front, true);
    near_wall(&mut front, false);
    torch(&mut front, 68, 155);
    torch(&mut front, 323, 151);
    scene(back, front, (24, 97))
}

fn arch(im: &mut Img, cx: i32, top: i32, half: i32, bottom: i32, rim: char, inside: char) {
    poly(
        im,
        &[
            (cx - half, bottom),
            (cx - half, top + half),
            (cx - half * 3 / 4, top + half / 2),
            (cx, top),
            (cx + half * 3 / 4, top + half / 2),
            (cx + half, top + half),
            (cx + half, bottom),
        ],
        rim,
    );
    let inner = half - (half / 7).max(3);
    poly(
        im,
        &[
            (cx - inner, bottom),
            (cx - inner, top + half + 3),
            (cx - inner * 3 / 4, top + half / 2 + 5),
            (cx, top + 7),
            (cx + inner * 3 / 4, top + half / 2 + 5),
            (cx + inner, top + half + 3),
            (cx + inner, bottom),
        ],
        inside,
    );
}

fn candle(im: &mut Img, x: i32, y: i32, h: i32) {
    glow(im, x, y - h, 15, 19, 0.24);
    im.rect(x - 1, y - h, 3, h, 'o');
    im.line(x, y - h, x, y, 't');
    im.line(x, y - h - 4, x, y - h - 1, '@');
    im.put(x, y - h - 2, '6');
    im.rect(x - 3, y + 1, 7, 2, DARK);
}

fn skull(im: &mut Img, x: i32, y: i32, c: char) {
    im.ellipse(x as f32, y as f32, 3.0, 3.0, c);
    im.rect(x - 2, y + 2, 4, 2, c);
    im.put(x - 2, y, DARK);
    im.put(x + 1, y, DARK);
    im.put(x, y + 2, DARK);
}

fn crypt_interior() -> Layers {
    let mut back = Img::black(W, H);
    sky(&mut back, true);
    // A chain of pale vaults recedes to an amber funerary window.
    for (x, top, half) in [(204, 40, 70), (204, 58, 50), (204, 73, 34), (204, 88, 21)] {
        arch(&mut back, x, top, half, 158, 'S', 's');
    }
    rose(&mut back, 204, 113, 9);
    glow(&mut back, 204, 119, 44, 42, 0.24);
    for x in [157, 244] {
        back.rect(x, 111, 5, 44, 'x');
    }
    let mut front = Img::new(W, H);
    // Side walls angle toward the far sanctuary; tiers of ossuary niches.
    poly(&mut front, &[(0, 0), (117, 41), (140, 160), (0, 188)], 's');
    poly(
        &mut front,
        &[(384, 0), (291, 42), (265, 160), (384, 188)],
        's',
    );
    masonry(&mut front, 0, 20, 110, 152, 'x');
    masonry(&mut front, 292, 21, 92, 152, 'x');
    for (side, columns) in [(0, [23, 64, 102]), (1, [279, 314, 353])] {
        for (i, x) in columns.into_iter().enumerate() {
            let inset = if side == 0 { i as i32 } else { 2 - i as i32 };
            let w = 28 - inset * 5;
            for row in 0..3 {
                let y = 37 + inset * 12 + row * (36 - inset * 4);
                window(&mut front, x, y, w, 29 - inset * 4, 'x');
                window(&mut front, x + 3, y + 4, w - 6, 24 - inset * 4, DARK);
                for dx in [8, w - 8] {
                    skull(&mut front, x + dx, y + 18 - inset, 'g');
                }
                front.line(
                    x + 3,
                    y + 24 - inset * 2,
                    x + w - 3,
                    y + 24 - inset * 2,
                    'r',
                );
                if (row + i as i32 + side) % 2 == 0 {
                    candle(&mut front, x + w / 2, y + 26 - inset * 2, 7);
                }
            }
        }
    }
    courtyard(&mut front, 204, 157);
    glow(&mut front, 204, 157, 57, 30, 0.16);
    // The stone sarcophagus and its clustered candles are the near hero.
    poly(
        &mut front,
        &[(126, 157), (187, 139), (259, 155), (202, 177)],
        'x',
    );
    poly(
        &mut front,
        &[(126, 157), (202, 177), (202, 188), (126, 175)],
        DARK,
    );
    poly(
        &mut front,
        &[(202, 177), (259, 155), (259, 174), (202, 188)],
        's',
    );
    front.line(128, 156, 187, 140, 'u');
    front.line(187, 140, 258, 155, 'u');
    front.line(131, 162, 196, 180, 's');
    front.line(131, 172, 196, 187, 's');
    for (x, y) in [(146, 168), (168, 174), (188, 180)] {
        skull(&mut front, x, y, 's');
    }
    front.line(209, 179, 249, 162, 'x');
    front.line(209, 184, 249, 168, 'x');
    glow(&mut front, 182, 131, 55, 51, 0.28);
    for (x, y, h) in [
        (164, 151, 19),
        (179, 148, 25),
        (193, 151, 15),
        (215, 157, 20),
        (229, 156, 12),
    ] {
        candle(&mut front, x, y, h);
    }
    skull(&mut front, 201, 152, 'o');
    for (x, top) in [(121, 54), (282, 49)] {
        front.rect(x, top, 9, 106, DARK);
        front.line(x + 7, top + 3, x + 7, 158, 'x');
        front.rect(x - 4, top, 16, 5, 'x');
        front.rect(x - 3, 156, 15, 6, DARK);
    }
    // Near black columns frame all three depth planes, with a ribbed ceiling.
    for x in [-8, 366] {
        front.rect(x, 0, 25, 188, DARK);
        front.rect(x + 6, 0, 3, 170, 's');
        front.rect(x - 4, 154, 33, 8, DARK);
    }
    poly(
        &mut front,
        &[
            (0, 0),
            (384, 0),
            (384, 23),
            (293, 16),
            (205, 49),
            (110, 13),
            (0, 32),
        ],
        DARK,
    );
    front.line(42, 0, 205, 49, 's');
    front.line(348, 0, 205, 49, 's');
    scene(back, front, (179, 121))
}

fn crypt(depth: u32) -> Layers {
    if depth == 1 {
        crypt_exterior()
    } else {
        crypt_interior()
    }
}

fn wheel(im: &mut Img, x: i32, y: i32, r: i32) {
    im.ellipse(x as f32, y as f32, (r + 3) as f32, (r + 3) as f32, DARK);
    im.ellipse(x as f32, y as f32, r as f32, r as f32, 'x');
    im.ellipse(x as f32, y as f32, (r - 2) as f32, (r - 2) as f32, DARK);
    for i in 0..8 {
        let a = i as f32 * std::f32::consts::TAU / 8.0;
        im.line(
            x,
            y,
            x + (a.cos() * r as f32) as i32,
            y + (a.sin() * r as f32) as i32,
            's',
        );
    }
    im.ellipse(x as f32, y as f32, 3.0, 3.0, 'r');
}

fn chain(im: &mut Img, x: i32, top: i32, bottom: i32) {
    for y in (top..bottom).step_by(5) {
        im.frame(x - 1, y, 3, 4, 'x');
        im.put(x, y + 2, 'u');
    }
}

fn mine_head() -> Layers {
    let mut back = Img::black(W, H);
    sky(&mut back, false);
    // Distant ridges sink into the sunset glow before the nearer rock face.
    poly(
        &mut back,
        &[
            (0, 126),
            (50, 75),
            (90, 99),
            (155, 58),
            (216, 103),
            (280, 64),
            (337, 94),
            (384, 68),
            (384, 188),
            (0, 188),
        ],
        NAVY,
    );
    poly(
        &mut back,
        &[
            (0, 145),
            (48, 110),
            (97, 137),
            (174, 80),
            (204, 111),
            (252, 101),
            (309, 68),
            (384, 118),
            (384, 188),
            (0, 188),
        ],
        's',
    );
    poly(
        &mut back,
        &[(309, 68), (317, 120), (357, 143), (332, 112)],
        'x',
    );
    horizon_haze(&mut back);
    let mut front = Img::new(W, H);
    // An open pit and tall timber winding gear, with no outdoor forge.
    courtyard(&mut front, 279, 165);
    poly(
        &mut front,
        &[(43, 159), (131, 145), (170, 166), (90, 186), (25, 173)],
        DARK,
    );
    front.line(43, 158, 131, 144, 'x');
    front.line(131, 144, 171, 165, 'x');
    for (a, b) in [(52, 31), (115, 137)] {
        poly(
            &mut front,
            &[(a, 53), (a + 8, 53), (b + 9, 169), (b - 2, 169)],
            DARK,
        );
        front.line(a + 1, 54, b, 166, 'x');
    }
    front.rect(35, 52, 102, 9, DARK);
    front.rect(41, 78, 86, 7, DARK);
    front.line(55, 88, 123, 142, 'x');
    front.line(111, 88, 43, 145, 'x');
    front.line(56, 89, 123, 143, DARK);
    wheel(&mut front, 82, 43, 19);
    front.line(83, 25, 289, 113, 'x');
    front.line(83, 27, 289, 115, DARK);
    chain(&mut front, 82, 49, 148);
    poly(
        &mut front,
        &[(69, 137), (94, 137), (90, 156), (73, 156)],
        's',
    );
    front.line(69, 136, 94, 136, 'u');
    // The adit has its own lantern, inset within angular dwarven stonework.
    poly(
        &mut front,
        &[
            (223, 168),
            (223, 119),
            (249, 93),
            (306, 93),
            (332, 119),
            (332, 168),
        ],
        DARK,
    );
    poly(
        &mut front,
        &[
            (233, 169),
            (233, 122),
            (253, 102),
            (302, 102),
            (321, 122),
            (321, 169),
        ],
        'x',
    );
    poly(
        &mut front,
        &[
            (241, 169),
            (241, 125),
            (259, 110),
            (296, 110),
            (313, 125),
            (313, 169),
        ],
        DARK,
    );
    for y in [121, 133, 145] {
        front.line(234, y, 242, y, 'r');
        front.line(312, y, 320, y, 'r');
    }
    glow(&mut front, 278, 137, 34, 41, 0.34);
    front.line(278, 113, 278, 128, 'r');
    front.rect(273, 128, 10, 15, 't');
    front.rect(276, 130, 4, 10, '6');
    front.rect(271, 126, 14, 3, DARK);
    front.rect(271, 142, 14, 3, DARK);
    for (end, start) in [(127, 267), (230, 284)] {
        front.line(start, 167, end, 188, 'u');
    }
    for y in [171, 176, 183] {
        front.line(123 + (188 - y) * 7, y, 239 + (188 - y) * 3, y, 's');
    }
    // Cart, track points, fence and loose foreground picks.
    poly(
        &mut front,
        &[(306, 157), (350, 149), (364, 164), (318, 172)],
        DARK,
    );
    front.line(306, 157, 350, 149, 'x');
    front.rect(317, 159, 44, 14, DARK);
    for x in [326, 353] {
        wheel(&mut front, x, 175, 5);
    }
    for x in (5..186).step_by(22) {
        front.rect(x, 151 + x / 15, 3, 26, DARK);
    }
    front.line(0, 157, 183, 171, 's');
    front.line(0, 165, 182, 179, 's');
    front.line(13, 174, 48, 188, 'r');
    poly(
        &mut front,
        &[
            (7, 173),
            (20, 167),
            (36, 171),
            (43, 178),
            (27, 173),
            (18, 173),
        ],
        'x',
    );
    glow(&mut front, 278, 167, 61, 22, 0.21);
    scene(back, front, (278, 136))
}

fn anvil(im: &mut Img, x: i32, y: i32, scale: i32) {
    let p = |dx, dy| (x + dx * scale, y + dy * scale);
    poly(
        im,
        &[
            p(-27, -9),
            p(8, -9),
            p(26, -14),
            p(20, -5),
            p(10, -2),
            p(6, 1),
            p(5, 9),
            p(13, 13),
            p(-17, 13),
            p(-8, 8),
            p(-8, 1),
            p(-19, -2),
        ],
        DARK,
    );
    im.line(
        x - 26 * scale,
        y - 9 * scale,
        x + 8 * scale,
        y - 9 * scale,
        'r',
    );
    im.line(
        x + 8 * scale,
        y - 9 * scale,
        x + 24 * scale,
        y - 13 * scale,
        'o',
    );
    im.rect(x - 19 * scale, y + 13 * scale, 33 * scale, 4 * scale, DARK);
}

fn forge_hall() -> Layers {
    let mut back = Img::black(W, H);
    sky(&mut back, true);
    // Pale far galleries and their small hearths, under the mountain vault.
    for (x, half, top) in [(225, 27, 66), (286, 31, 48), (358, 35, 29)] {
        arch(&mut back, x, top, half, 154, 'x', 's');
        back.rect(x - 12, 119, 24, 21, 'x');
        back.rect(x - 7, 127, 14, 11, 'r');
        glow(&mut back, x, 134, 23, 28, 0.26);
        flame(&mut back, x, 137, 3);
    }
    back.line(184, 102, 384, 79, 'x');
    back.line(184, 107, 384, 84, 'x');
    for x in (199..384).step_by(17) {
        back.line(x, 83 + (384 - x) / 9, x, 105 - (x - 184) / 9, 'x');
    }
    let mut front = Img::new(W, H);
    front.rect(0, 143, W, 45, DARK);
    for y in [149, 163, 181] {
        front.line(175, y, 384, y + 3, 's');
    }
    // Monumental hood and masonry hearth, offset left from the galleries.
    front.rect(53, 61, 107, 89, 's');
    masonry(&mut front, 56, 63, 99, 79, 'x');
    arch(&mut front, 108, 72, 41, 145, 'r', '8');
    glow(&mut front, 108, 123, 75, 65, 0.38);
    front.rect(77, 99, 62, 45, '7');
    front.rect(86, 116, 45, 25, 'R');
    flame(&mut front, 107, 138, 13);
    flame(&mut front, 89, 141, 6);
    flame(&mut front, 126, 141, 7);
    for x in (77..143).step_by(8) {
        front.ellipse(x as f32, 144.0, 4.0, 3.0, DARK);
        front.put(x - 1, 143, 'R');
    }
    poly(
        &mut front,
        &[(50, 70), (68, 39), (79, 0), (139, 0), (149, 39), (170, 70)],
        DARK,
    );
    poly(&mut front, &[(58, 64), (74, 38), (143, 38), (161, 64)], 's');
    front.line(74, 37, 142, 37, 'x');
    front.rect(47, 67, 126, 7, DARK);
    for x in (52..169).step_by(12) {
        front.put(x, 70, 'r');
    }
    front.rect(56, 149, 109, 7, 'x');
    glow(&mut front, 108, 153, 108, 42, 0.32);
    // Midground bench, tools, water barrel and a smaller anvil.
    front.rect(246, 149, 112, 5, 'x');
    for x in [253, 341] {
        front.rect(x, 154, 7, 27, DARK);
    }
    for (x, y) in [(268, 115), (286, 114), (303, 109)] {
        front.line(x, y, x + 2, 144, DARK);
        front.rect(x - 5, y - 2, 11, 4, 'x');
    }
    anvil(&mut front, 286, 141, 1);
    front.ellipse(28.0, 159.0, 16.0, 8.0, 'x');
    front.rect(12, 156, 32, 31, DARK);
    for x in [16, 24, 34, 40] {
        front.line(x, 159, x, 184, 's');
    }
    front.line(12, 163, 44, 163, 'x');
    front.line(12, 180, 44, 180, 'x');
    // The foreground anvil catches the hearth along its upper edge.
    anvil(&mut front, 156, 161, 2);
    poly(
        &mut front,
        &[(120, 151), (161, 151), (172, 158), (165, 160), (141, 158)],
        'b',
    );
    front.line(119, 148, 171, 148, 'r');
    front.line(133, 171, 140, 176, 's');
    front.line(166, 174, 185, 183, 's');
    front.rect(194, 166, 20, 6, DARK);
    front.line(205, 170, 224, 188, 'r');
    chain(&mut front, 198, 0, 111);
    front.line(195, 108, 202, 119, 'x');
    front.line(202, 119, 208, 114, 'x');
    // Near pillars, roof beams and hanging tongs close the interior frame.
    for x in [-3, 370] {
        front.rect(x, 0, 16, 188, DARK);
        front.line(x + 12, 5, x + 12, 170, 's');
    }
    poly(&mut front, &[(8, 0), (44, 0), (78, 32), (61, 32)], DARK);
    poly(
        &mut front,
        &[(323, 0), (365, 0), (239, 52), (227, 44)],
        DARK,
    );
    front.line(357, 0, 232, 49, 's');
    front.line(35, 0, 67, 30, 's');
    front.line(23, 48, 29, 108, 'r');
    front.line(36, 48, 31, 108, 's');
    scene(back, front, (108, 123))
}

fn mines(depth: u32) -> Layers {
    if depth == 1 {
        mine_head()
    } else {
        forge_hall()
    }
}

fn books(im: &mut Img, x: i32, y: i32, w: i32, h: i32, near: bool) {
    im.rect(x, y, w, h, DARK);
    im.rect(x + 3, y + 3, w - 6, h - 6, 's');
    let step = if near { 18 } else { 13 };
    for yy in (y + step..y + h).step_by(step as usize) {
        im.rect(x + 2, yy, w - 4, 3, DARK);
        let mut xx = x + 5;
        while xx < x + w - 6 {
            let n = hash(xx, yy, 73);
            let bw = 2 + (n % 4) as i32;
            let bh = step - 4 - ((n >> 5) % 4) as i32;
            let c = match n % 7 {
                0 => 'r',
                1 => CRIMSON,
                2 => 'u',
                3 => NAVY,
                _ => 'x',
            };
            im.rect(xx, yy - bh, bw, bh, c);
            if near {
                im.line(
                    xx,
                    yy - bh + 3,
                    xx + bw - 1,
                    yy - bh + 3,
                    if n % 3 == 0 { 'o' } else { 's' },
                );
                im.line(xx, yy - 3, xx + bw - 1, yy - 3, 's');
                im.line(
                    xx,
                    yy - bh + 1,
                    xx,
                    yy - 1,
                    if n % 3 == 0 { 'r' } else { 'u' },
                );
            }
            xx += bw + 2;
        }
    }
    im.rect(x, y, 3, h, 'x');
    im.rect(x + w - 3, y, 3, h, 's');
}

fn water(im: &mut Img, source: &Img, line: i32) {
    // Broken, compressed mirror silhouettes; the water stays in the cool inks.
    im.rect(0, line, W, HORIZON - line, DARK);
    for y in line..HORIZON {
        let sy = line - (y - line) * 2 - 1;
        for x in 0..W {
            let n = hash(x / 3, y, 97);
            if n % 5 < 2 && y % 3 != 0 {
                if let Some(c) = source.get(x + ((y - line) % 5 - 2), sy) {
                    im.set(x, y, c);
                    tint(im, x, y, ink(DARK).unwrap(), 0.67);
                }
            }
        }
    }
    for i in 0..23 {
        let x = (hash(i, 0, 23) % 376) as i32;
        let y = line + (hash(i, 1, 23) % (188 - line) as u32) as i32;
        im.rect(x, y, 3 + i % 11, 1, 's');
    }
}

fn reflection(im: &mut Img, x: i32, line: i32, width: i32) {
    for y in line..HORIZON {
        let spread = width + (y - line) / 3;
        for xx in x - spread..x + spread {
            let falloff = (1.0 - (xx - x).abs() as f32 / spread as f32).max(0.0);
            if hash(xx / 3, y, 33) % 7 < 3 && y % 3 != 0 {
                tint(im, xx, y, ink('@').unwrap(), falloff * 0.46);
            }
        }
    }
}

fn archive() -> Layers {
    let mut back = Img::black(W, H);
    sky(&mut back, true);
    // A drowned cloister and far folios are visible through the reading hall.
    for (x, half, top) in [(148, 32, 47), (204, 26, 65), (251, 23, 74)] {
        arch(&mut back, x, top, half, 155, 'x', 's');
        books(
            &mut back,
            x - half + 11,
            top + half + 15,
            half * 2 - 22,
            53,
            false,
        );
    }
    let mut front = Img::new(W, H);
    books(&mut front, 18, 32, 63, 122, true);
    books(&mut front, 95, 56, 42, 95, true);
    books(&mut front, 272, 48, 42, 105, true);
    books(&mut front, 327, 23, 53, 132, true);
    for (x, top, half) in [(43, 5, 43), (114, 26, 29), (291, 21, 30), (358, -13, 43)] {
        // Crown the cases with gothic gables and a little finial.
        poly(
            &mut front,
            &[
                (x - half, top + 27),
                (x, top),
                (x + half, top + 27),
                (x + half - 5, top + 31),
                (x, top + 9),
                (x - half + 5, top + 31),
            ],
            DARK,
        );
        front.line(x, top - 6, x, top + 5, 's');
    }
    for x in [0, 145, 258, 380] {
        front.rect(x, 0, 7, 157, DARK);
        front.line(x + 5, 29, x + 5, 150, 'x');
    }
    poly(
        &mut front,
        &[
            (0, 0),
            (384, 0),
            (384, 11),
            (304, 9),
            (204, 43),
            (112, 9),
            (0, 16),
        ],
        DARK,
    );
    front.line(91, 0, 204, 43, 's');
    front.line(327, 0, 204, 43, 's');
    chain(&mut front, 195, 0, 79);
    glow(&mut back, 195, 102, 48, 50, 0.25);
    glow(&mut front, 195, 102, 92, 62, 0.32);
    poly(&mut front, &[(195, 76), (180, 87), (210, 87)], DARK);
    front.rect(183, 88, 24, 24, 'o');
    front.rect(187, 90, 16, 19, '@');
    front.rect(191, 92, 7, 16, '6');
    for x in [184, 195, 205] {
        front.line(x, 86, x, 112, DARK);
    }
    front.rect(179, 112, 32, 4, DARK);
    let mut source = back.clone();
    source.stamp(&front, 0, 0);
    water(&mut front, &source, 151);
    reflection(&mut front, 195, 153, 16);
    // A half-submerged lectern holds an open folio, with warm page edges.
    poly(
        &mut front,
        &[(90, 151), (120, 137), (146, 148), (116, 164)],
        DARK,
    );
    poly(
        &mut front,
        &[(93, 149), (118, 139), (120, 151), (108, 157)],
        'r',
    );
    poly(
        &mut front,
        &[(120, 139), (141, 148), (122, 158), (120, 151)],
        'o',
    );
    front.line(120, 141, 120, 155, DARK);
    front.line(98, 149, 111, 145, 't');
    front.line(126, 146, 137, 150, 't');
    front.rect(113, 160, 5, 24, DARK);
    // Near shelf edges and drifting folios sit in front of the reflections.
    poly(&mut front, &[(0, 79), (15, 87), (15, 179), (0, 188)], DARK);
    poly(
        &mut front,
        &[(384, 73), (369, 86), (367, 187), (384, 188)],
        DARK,
    );
    front.line(375, 97, 372, 173, 's');
    for (x, y) in [(60, 173), (273, 180), (311, 164)] {
        poly(
            &mut front,
            &[(x, y), (x + 17, y - 3), (x + 28, y + 2), (x + 11, y + 5)],
            DARK,
        );
        front.line(x + 3, y, x + 15, y - 2, 'r');
    }
    scene(back, front, (195, 102))
}

fn mushroom(im: &mut Img, x: i32, y: i32, r: i32, foot: i32, c: char) {
    poly(
        im,
        &[(x - 4, y), (x + 5, y), (x + 9, foot), (x - 9, foot)],
        c,
    );
    poly(
        im,
        &[
            (x - r, y + 5),
            (x - r + 6, y - 6),
            (x - r / 2, y - r / 3),
            (x + 2, y - r / 2),
            (x + r / 2, y - r / 3),
            (x + r - 4, y - 3),
            (x + r, y + 7),
        ],
        c,
    );
    if c == DARK {
        poly(
            im,
            &[
                (x - r + 3, y + 5),
                (x + r - 3, y + 5),
                (x + r / 2, y + 13),
                (x + 4, y + 15),
                (x - r / 2, y + 12),
            ],
            's',
        );
        for dx in (-r + 6..r - 5).step_by(5) {
            im.line(x + dx, y + 6, x + dx / 4, y + 14, 'u');
        }
        im.line(x - r + 4, y + 4, x + r - 4, y + 5, 'x');
        im.line(x + 3, y + 17, x + 5, foot - 3, 's');
        for i in 0..7 {
            let dx = (hash(x, i, 22) % (r * 3 / 2) as u32) as i32 - r * 3 / 4;
            let dy = (hash(x, i, 28) % (r / 3).max(1) as u32) as i32;
            im.rect(x + dx, y - dy - 1, 2, 1, 'x');
        }
    }
}

fn hut(im: &mut Img, x: i32, y: i32, lit: bool) {
    for dx in [1, 15, 38, 49] {
        im.rect(x + dx, y + 25, 3, 60, DARK);
    }
    im.rect(x, y, 53, 38, if lit { CRIMSON } else { DARK });
    poly(
        im,
        &[(x - 9, y + 2), (x + 22, y - 28), (x + 62, y + 4)],
        DARK,
    );
    poly(
        im,
        &[(x - 6, y + 2), (x + 22, y - 23), (x + 3, y + 2)],
        if lit { '7' } else { SLATE },
    );
    for yy in (y + 4..y + 38).step_by(7) {
        im.line(x, yy, x + 52, yy, DARK);
    }
    for xx in (x + 2..x + 52).step_by(9) {
        im.line(xx, y + 4, xx, y + 37, if lit { '8' } else { 's' });
    }
    glow(im, x + 20, y + 18, 36, 34, if lit { 0.37 } else { 0.17 });
    im.rect(x + 12, y + 8, 17, 20, if lit { '@' } else { 'o' });
    im.line(x + 20, y + 8, x + 20, y + 29, DARK);
    im.line(x + 12, y + 18, x + 28, y + 18, DARK);
    im.rect(x - 8, y + 39, 71, 4, DARK);
    im.line(x - 8, y + 31, x + 63, y + 31, 'x');
    for dx in [-5, 5, 38, 59] {
        im.rect(x + dx, y + 25, 2, 17, DARK);
    }
    im.line(x + 3, y + 44, x + 49, y + 71, DARK);
    im.line(x + 49, y + 44, x + 3, y + 71, DARK);
}

fn fungal() -> Layers {
    let mut back = Img::black(W, H);
    sky(&mut back, true);
    // A pale fungal forest disappears into the low waterline mist.
    for (x, y, r) in [
        (16, 104, 22),
        (68, 93, 26),
        (119, 108, 17),
        (168, 84, 22),
        (216, 100, 26),
        (260, 91, 20),
        (305, 106, 19),
        (356, 91, 29),
    ] {
        mushroom(&mut back, x, y, r, 160, 'x');
    }
    for (x, y, r) in [
        (12, 94, 25),
        (105, 76, 31),
        (191, 100, 24),
        (267, 62, 33),
        (377, 105, 30),
    ] {
        mushroom(&mut back, x, y, r, 163, 's');
    }
    let mut front = Img::new(W, H);
    mushroom(&mut front, 39, 53, 38, 181, DARK);
    mushroom(&mut front, 154, 71, 29, 168, DARK);
    mushroom(&mut front, 341, 32, 53, 182, DARK);
    hut(&mut front, 77, 119, false);
    hut(&mut front, 254, 101, true);
    let mut source = back.clone();
    source.stamp(&front, 0, 0);
    water(&mut front, &source, 162);
    reflection(&mut front, 274, 163, 14);
    reflection(&mut front, 97, 165, 8);
    // Boardwalk, ladder and branch rails, grounded in the dark water.
    poly(
        &mut front,
        &[(75, 163), (254, 146), (275, 151), (106, 177)],
        DARK,
    );
    front.line(105, 170, 256, 149, 'x');
    for i in 0..17 {
        front.line(99 + i * 9, 163 - i, 114 + i * 9, 170 - i, 's');
    }
    for x in [138, 174, 211, 246] {
        front.line(x, 156 - (x - 138) / 9, x, 176 - (x - 138) / 9, DARK);
    }
    front.line(128, 148, 251, 136, DARK);
    front.line(306, 141, 332, 181, 'x');
    front.line(312, 141, 338, 181, 'x');
    for i in 0..7 {
        front.line(309 + i * 4, 146 + i * 6, 315 + i * 4, 146 + i * 6, 'r');
    }
    // Near roots and reeds cut across the mirror plane.
    for x in [5, 372] {
        poly(
            &mut front,
            &[
                (x - 7, 0),
                (x + 7, 0),
                (x + 3, 138),
                (x + 21, 174),
                (x + 5, 163),
                (x - 6, 183),
                (x - 5, 138),
            ],
            DARK,
        );
        front.line(x + 2, 34, x + 35, 8, DARK);
        front.line(x + 1, 75, x - 29, 36, DARK);
        front.line(x + 1, 101, x + 29, 116, DARK);
    }
    for (x, y) in [(19, 188), (60, 188), (217, 188), (352, 188)] {
        for dx in [-15_i32, -7, 2, 11] {
            front.line(x, y, x + dx, y - 20 + dx.abs() / 2, 's');
        }
    }
    // Small clustered spores complement the few slowly drifting live motes.
    for i in 0..31 {
        let x = 25 + (hash(i, 0, 211) % 330) as i32;
        let y = 64 + (hash(i, 1, 211) % 93) as i32;
        glow(&mut back, x, y, 4, 5, 0.20);
        back.put(x, y, if i % 5 == 0 { 't' } else { 'o' });
    }
    scene(back, front, (274, 119))
}

fn eye(im: &mut Img, x: i32, y: i32, r: i32) {
    poly(
        im,
        &[
            (x - r, y),
            (x - r / 2, y - 5),
            (x + r / 2, y - 5),
            (x + r, y),
            (x + r / 2, y + 5),
            (x - r / 2, y + 5),
        ],
        MUSTARD,
    );
    im.ellipse(x as f32, y as f32, 4.0, 4.0, 't');
    im.rect(x - 1, y - 4, 3, 9, DARK);
}

fn unknown() -> Layers {
    let mut back = Img::black(W, H);
    sky(&mut back, true);
    // Remote monoliths and a second colonnade dissolve into slate haze.
    for (x, y, w) in [
        (22, 110, 12),
        (91, 123, 10),
        (156, 113, 9),
        (233, 119, 10),
        (299, 107, 13),
        (363, 105, 11),
    ] {
        back.rect(x, y, w, 60, 'x');
        back.rect(x - 3, y, w + 6, 3, 'x');
    }
    for (x, y) in [(58, 98), (132, 116), (259, 110), (324, 92)] {
        back.rect(x, y, 14, 70, 's');
        back.rect(x - 5, y - 4, 24, 6, 's');
    }
    courtyard(&mut back, 194, 146);
    glow(&mut back, 194, 132, 86, 51, 0.26);
    poly(
        &mut back,
        &[
            (188, 157),
            (188, 103),
            (191, 112),
            (194, 65),
            (198, 114),
            (198, 157),
        ],
        'o',
    );
    back.line(194, 84, 194, 158, '@');
    back.rect(193, 130, 3, 27, '6');
    glow(&mut back, 194, 163, 64, 22, 0.28);
    let mut front = Img::new(W, H);
    // The impossible threshold hangs over all three depth planes.
    poly(
        &mut front,
        &[
            (0, 0),
            (384, 0),
            (384, 35),
            (346, 28),
            (337, 56),
            (307, 47),
            (281, 71),
            (271, 109),
            (244, 86),
            (222, 125),
            (196, 61),
            (175, 107),
            (152, 95),
            (119, 119),
            (100, 63),
            (68, 75),
            (50, 41),
            (0, 51),
        ],
        DARK,
    );
    poly(
        &mut front,
        &[(46, 41), (66, 60), (86, 52), (101, 80), (95, 58), (71, 44)],
        's',
    );
    poly(
        &mut front,
        &[(278, 36), (269, 73), (282, 64), (306, 45)],
        's',
    );
    for (x, y, w) in [(35, 82, 22), (91, 108, 17), (281, 106, 17), (346, 70, 26)] {
        poly(
            &mut front,
            &[
                (x - w / 2, y),
                (x + w / 2, y),
                (x + w / 2 + 5, 183),
                (x - w / 2 - 5, 183),
            ],
            DARK,
        );
        front.line(x - w / 2 + 2, y + 14, x - w / 2 + 2, 181, 's');
        front.rect(x - w / 2 - 4, y + 17, w + 8, 4, 's');
        glow(&mut front, x, y + 7, 23, 28, 0.17);
        eye(&mut front, x, y + 7, w / 2 + 4);
        front.rect(x - w / 2 - 6, 181, w + 12, 7, DARK);
    }
    scene(back, front, (194, 139))
}

fn beam(im: &mut Img, origin: (i32, i32), end: (i32, i32), width: f32) {
    let (vx, vy) = ((end.0 - origin.0) as f32, (end.1 - origin.1) as f32);
    let length2 = vx * vx + vy * vy;
    let length = length2.sqrt();
    for y in 0..HORIZON {
        for x in 0..W {
            let (dx, dy) = ((x - origin.0) as f32, (y - origin.1) as f32);
            let t = (dx * vx + dy * vy) / length2;
            if !(0.0..1.0).contains(&t) {
                continue;
            }
            let cross = (dx * vy - dy * vx).abs() / length;
            let radius = 1.5 + t * width;
            let edge = (1.0 - cross / radius).max(0.0);
            let a = edge.powf(1.3) * (1.0 - t * 0.55) * 0.70;
            // Keep the cone cool even against the red sunset. The ordered
            // coverage supplies transparency and a feathered wedge edge.
            if bayer(x, y) < a {
                im.put(x, y, if edge > 0.65 { 'u' } else { 'S' });
            }
        }
    }
}

fn title(im: &mut Img, kind: Kind, depth: u32) {
    // Letterpress intertitle: double hairlines, stepped diamond and wide caps.
    im.rect(0, 188, W, 36, DARK);
    for (a, b) in [(28, 148), (235, 355)] {
        im.line(a, 192, b, 192, MUSTARD);
    }
    for x in [21, 362] {
        poly(
            im,
            &[(x, 190), (x + 3, 193), (x, 196), (x - 3, 193)],
            MUSTARD,
        );
    }
    let label = kind.title();
    let mut letters = Img::new(super::super::ink::text_width(label), 7);
    letters.text(0, 0, label, 't');
    let left = (W - letters.w * 2) / 2;
    for y in 0..14 {
        for x in 0..letters.w * 2 {
            if let Some(c) = letters.get(x / 2, y / 2) {
                im.set(left + x, 196 + y, c);
            }
        }
    }
    let depth = format!("FLOOR {depth}");
    let len = super::super::ink::text_width(&depth);
    im.text((W - len) / 2, 215, &depth, MUSTARD);
    im.line(125, 218, 155, 218, 'x');
    im.line(228, 218, 258, 218, 'x');
}

fn build(kind: Kind, depth: u32) -> Scene {
    let mut scene = match kind {
        Kind::Gate => gate(),
        Kind::Crypt => crypt(depth),
        Kind::Mines => mines(depth),
        Kind::Keep => keep(),
        Kind::Archive => archive(),
        Kind::Fungal => fungal(),
        Kind::Unknown => unknown(),
    };
    title(&mut scene.architecture, kind, depth);
    // Eight subtle beam positions are composed once behind the near masonry.
    // Interiors have a single cached painting, with no outdoor searchlights.
    let outdoors = kind == Kind::Gate || kind == Kind::Keep || (kind == Kind::Crypt && depth == 1);
    let mut sweeps = Vec::with_capacity(if outdoors { 8 } else { 1 });
    for phase in 0..if outdoors { 8 } else { 1 } {
        let mut im = scene.sky.clone();
        if outdoors {
            beam(&mut im, (318, 171), (-55 + phase * 4, -20), 42.0);
            beam(&mut im, (69, 176), (440 - phase * 4, 18), 35.0);
        }
        im.stamp(&scene.architecture, 0, 0);
        sweeps.push(im);
    }
    Scene {
        sweeps,
        light: scene.light,
    }
}

fn cached(kind: Kind, depth: u32) -> &'static Scene {
    static SCENES: [OnceLock<Scene>; DESTINATIONS.len()] =
        [const { OnceLock::new() }; DESTINATIONS.len()];
    let index = DESTINATIONS
        .iter()
        .position(|&(k, d)| k == kind && d == depth)
        .or_else(|| DESTINATIONS.iter().position(|&(k, _)| k == kind))
        .unwrap_or(0);
    let (kind, depth) = DESTINATIONS[index];
    SCENES[index].get_or_init(|| build(kind, depth))
}

/// Fully opened cached artwork, with only small seat-local light accents.
pub fn painting(kind: Kind, depth: u32, age: u8) -> Img {
    let scene = cached(kind, depth);
    let mut im = scene.sweeps[usize::from(age / 7) % scene.sweeps.len()].clone();
    let (x, y) = scene.light;
    im.rect(x - 1, y - 1, 2, 2, if age / 4 % 3 != 1 { '6' } else { 't' });
    if kind == Kind::Mines && depth == 2 {
        for i in 0..9 {
            let t = (i * 7 + i32::from(age)) % 31;
            im.put(
                x - 21 + i * 5 + t / 9,
                y - 7 - t,
                if i % 3 == 0 { '@' } else { 'o' },
            );
        }
    }
    if kind == Kind::Fungal {
        for (i, (sx, sy)) in [(73, 75), (167, 109), (234, 85), (305, 131), (40, 124)]
            .into_iter()
            .enumerate()
        {
            let drift = (i32::from(age) / 8 + i as i32) % 3;
            im.put(sx + drift, sy - drift, 't');
            if (usize::from(age) / 6 + i) % 3 == 0 {
                im.put(sx + drift, sy - drift - 1, MUSTARD);
            }
        }
    }
    if kind == Kind::Archive {
        let shift = i32::from(age) / 8 % 3;
        im.rect(188 - shift, 172, 13 + shift * 2, 1, 't');
    }
    im
}

pub fn frame(plate: Playing, w: i32, h: i32) -> Img {
    let mut native = painting(plate.kind, plate.depth, plate.age);
    // Quick horizontal iris: black -> painting in 7 ticks, close in 6.
    // No interpolated RGB: all frames remain exactly on the master palette.
    let aperture = if plate.age < 7 {
        i32::from(plate.age) * H / 7
    } else if plate.age > DURATION - 7 {
        i32::from(DURATION.saturating_sub(plate.age)) * H / 6
    } else {
        H
    };
    let edge = (H - aperture) / 2;
    for y in 0..H {
        if y < edge || y >= H - edge {
            for x in 0..W {
                native.set(x, y, BLACK);
            }
        }
    }
    if w == W && h == H {
        return native;
    }
    let mut out = Img::black(w.max(0), h.max(0));
    if w <= 0 || h <= 0 {
        return out;
    }
    // Fit with the arena's aspect ratio. Pure nearest-pixel enlargement and
    // reduction keep the large silhouettes and title shapes on their inks.
    let (tw, th) = if w * H <= h * W {
        (w, w * H / W)
    } else {
        (h * W / H, h)
    };
    if tw == 0 || th == 0 {
        return out;
    }
    let (ox, oy) = ((w - tw) / 2, (h - th) / 2);
    for yy in 0..th {
        for xx in 0..tw {
            if let Some(c) = native.get(xx * W / tw, yy * H / th) {
                out.set(ox + xx, oy + yy, c);
            }
        }
    }
    // Half-block terminals need an actual small font, rather than a 5×7
    // glyph reduced below one pixel. Keep this inside the fitted picture.
    if tw < 230 && tw >= 90 && th >= 40 && (7..=DURATION - 7).contains(&plate.age) {
        out.rect(ox, oy + th - 19, tw, 19, DARK);
        let label = plate.kind.title();
        out.text(
            ox + (tw - super::super::ink::text_width(label)) / 2,
            oy + th - 18,
            label,
            't',
        );
        let label = format!("FLOOR {}", plate.depth);
        out.text(
            ox + (tw - super::super::ink::text_width(&label)) / 2,
            oy + th - 8,
            &label,
            MUSTARD,
        );
    }
    out
}

#[cfg(test)]
#[path = "../../../../tests/render/overworld__plates_tests.rs"]
mod tests;
