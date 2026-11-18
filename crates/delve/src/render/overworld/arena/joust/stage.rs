//! The lists at dusk, painted once: a night sky with flag poles, striped
//! pavilions at either end, timber stands under a scalloped awning with a
//! painted rail, the sand in its two lanes, the tilt down the middle and the
//! fence in front. The crowd, the flames, the pennants and everything that
//! moves is drawn over it each frame (`live`).
//!
//! Black paper, the realm's timber and stone inks, an ordered dither, one
//! warm accent: the torches.

use super::super::super::ink::{Img, bayer, hash};
use std::sync::OnceLock;

pub(super) const FAR_Y: i32 = 132;
pub(super) const NEAR_Y: i32 = 176;
pub(super) const TILT_Y: i32 = 134;
pub(super) const TILT_H: i32 = 11;
pub(super) const RAIL_Y: i32 = 86;
pub(super) const FENCE_Y: i32 = 193;
pub(super) const TIERS: [i32; 4] = [47, 59, 71, 83];
pub(super) const STANDS: (i32, i32) = (46, 338);
pub(super) const TORCHES: [(i32, i32); 4] = [(44, 96), (340, 96), (12, 150), (372, 150)];

pub(super) fn backdrop() -> &'static Img {
    static B: OnceLock<Img> = OnceLock::new();
    B.get_or_init(paint)
}

pub(super) fn tilt() -> &'static Img {
    static T: OnceLock<Img> = OnceLock::new();
    T.get_or_init(|| {
        let mut im = Img::new(384, TILT_H + 6);
        let y = 3;
        for x in 10..374 {
            let stripe = (x / 12) % 2 == 0;
            im.put(x, y, 'o');
            im.put(x, y + 1, 'O');
            for dy in 2..7 {
                im.put(x, y + dy, if stripe { 'R' } else { 'P' });
            }
            im.put(x, y + 7, if stripe { 'p' } else { 'r' });
            im.put(x, y + 8, 'b');
            if (x + y) % 7 == 0 {
                im.put(x, y + 4, 'B');
            }
        }
        for x in (10..=370).step_by(48) {
            im.rect(x, 0, 3, TILT_H + 5, 'I');
            im.line(x, 0, x + 2, 0, 'o');
            im.put(x, 1, 'r');
            im.line(x + 2, 1, x + 2, TILT_H + 4, 'b');
        }
        im
    })
}

pub(super) fn fence() -> &'static Img {
    static F: OnceLock<Img> = OnceLock::new();
    F.get_or_init(|| {
        let mut im = Img::new(384, 8);
        for x in 0..384 {
            im.put(x, 1, 'o');
            im.put(x, 2, 'P');
            im.put(x, 3, 'P');
            im.put(x, 4, 'I');
            im.put(x, 5, 'b');
            if x % 3 == 0 {
                im.put(x, 6, 'n');
            }
        }
        for x in (4..384).step_by(40) {
            im.rect(x, 0, 4, 8, 'B');
            im.line(x, 0, x + 3, 0, 'O');
            im.line(x + 3, 1, x + 3, 7, 'b');
        }
        im
    })
}

fn sky(im: &mut Img) {
    for y in 0..40 {
        for x in 0..384 {
            let k = y as f32 / 40.0;
            let cold = if bayer(x, y) < k * 0.5 { 'K' } else { 'k' };
            let ink = if bayer(x + 2, y + 1) < (k - 0.55).max(0.0) * 1.2 { 's' } else { cold };
            im.put(x, y, ink);
        }
    }
    for i in 0..46 {
        let (x, y) = ((hash(i, 3, 91) % 384) as i32, (hash(i, 9, 92) % 26) as i32);
        im.put(x, y, if i % 5 == 0 { 'h' } else { 'G' });
    }
}

fn pavilion(im: &mut Img, left: i32, ground: i32, cloth: [char; 2], face: i32) {
    let (w, h) = (52, 40);
    let apex = (left + w / 2, ground - h - 8);
    for y in apex.1..ground - 12 {
        let t = (y - apex.1) as f32 / (ground - 12 - apex.1) as f32;
        let half = (t * (w as f32 / 2.0 + 3.0)) as i32;
        for x in apex.0 - half..=apex.0 + half {
            let stripe = ((x - apex.0 + 100) / 6) % 2 == 0;
            let shade = if (x - apex.0) * face > 0 { 1 } else { 0 };
            let c = cloth[usize::from(stripe)];
            let c = if shade == 1 { c } else { dim(c) };
            im.put(x, y, c);
        }
        if y >= ground - 16 {
            let mut x = apex.0 - half;
            while x < apex.0 + half {
                im.put(x + 1, y + 1, dim(cloth[0]));
                x += 6;
            }
        }
    }
    for y in ground - 12..ground {
        for x in left + 3..left + w - 3 {
            let stripe = ((x + 100) / 6) % 2 == 0;
            im.put(x, y, if stripe { dim(cloth[0]) } else { 'b' });
        }
    }
    im.rect(apex.0 - 7, ground - 12, 14, 12, 'k');
    im.rect(apex.0 - 8, ground - 12, 1, 12, 'I');
    im.rect(apex.0 + 7, ground - 12, 1, 12, 'I');
    im.line(apex.0, apex.1 - 8, apex.0, apex.1 + 2, 'o');
    im.put(apex.0, apex.1 - 9, '5');
}

fn dim(c: char) -> char {
    match c {
        'p' => 'B',
        'o' => 'r',
        'u' => 'S',
        'T' => 'o',
        'W' => 'u',
        'P' => 'I',
        'R' => 'p',
        _ => c,
    }
}

fn stands(im: &mut Img) {
    let (x0, x1) = STANDS;
    for y in 24..RAIL_Y {
        for x in x0..x1 {
            let plank = (x / 8) % 2 == 0;
            let ink = if (x % 8) == 0 {
                'k'
            } else if bayer(x, y) < 0.22 {
                'n'
            } else if plank {
                'b'
            } else {
                'n'
            };
            im.put(x, y, ink);
        }
    }
    for &t in &TIERS {
        for x in x0..x1 {
            im.put(x, t + 1, 'P');
            im.put(x, t + 2, 'I');
            if x % 2 == 0 {
                im.put(x, t + 3, 'b');
            }
        }
    }
    for y in 22..33 {
        for x in x0 - 6..x1 + 6 {
            let stripe = (x / 8) % 2 == 0;
            let scallop = y >= 30 && ((x % 8) as i32 - 4).abs() + (y - 30) > 4;
            if scallop {
                continue;
            }
            let c = if stripe { 'p' } else { 'o' };
            let c = if y < 24 { dim(c) } else { c };
            im.put(x, y, if y == 22 { 'k' } else { c });
        }
    }
    for x in [x0 - 4, x1 + 2] {
        im.rect(x, 22, 4, RAIL_Y - 20, 'I');
        im.rect(x, 22, 1, RAIL_Y - 20, 'o');
    }
    for x in x0 - 6..x1 + 8 {
        im.put(x, RAIL_Y, 'O');
        im.put(x, RAIL_Y + 1, 'o');
        for dy in 2..5 {
            im.put(x, RAIL_Y + dy, 'P');
        }
        im.put(x, RAIL_Y + 5, 'b');
    }
    const ARMS: [(char, char); 6] = [('p', 'T'), ('u', 'W'), ('E', 'Y'), ('R', 'c'), ('S', 'v'), ('B', 'O')];
    for (i, x) in (x0 + 2..x1 - 6).step_by(14).enumerate() {
        let (a, b) = ARMS[(hash(i as i32, 5, 71) % 6) as usize];
        for dy in 0..5 {
            for dx in 0..6 {
                let point = dy == 4 && (dx == 0 || dx == 5);
                if !point {
                    im.put(x + dx, RAIL_Y + 1 + dy, if dx == 2 || dy == 2 { b } else { a });
                }
            }
        }
    }
}

fn sand(im: &mut Img) {
    for y in RAIL_Y + 6..FENCE_Y + 8 {
        for x in 0..384 {
            let lane = |c: i32| {
                let d = ((y - c) as f32 / 18.0).abs();
                (1.0 - d).max(0.0)
            };
            let warm = lane(FAR_Y - 6).max(lane(NEAR_Y - 6));
            let v = bayer(x, y);
            let n = (hash(x / 2, y / 2, 17) & 255) as f32 / 255.0;
            let ink = if v < 0.07 + warm * 0.26 && n < 0.8 {
                'b'
            } else if v > 0.93 - warm * 0.07 && n > 0.55 {
                'I'
            } else if n > 0.985 {
                'B'
            } else if n < 0.03 {
                'n'
            } else {
                'k'
            };
            im.put(x, y, ink);
        }
    }
    for (lane, base) in [(FAR_Y, 3), (NEAR_Y, 4)] {
        for i in 0..52 {
            let x = (hash(i, base, 61) % 360) as i32 + 8;
            let y = lane - 2 + (hash(i, base, 62) % 5) as i32;
            im.put(x, y, 'I');
            im.put(x + 1, y, 'b');
        }
    }
    for y in RAIL_Y + 10..FENCE_Y {
        if y % 3 != 0 {
            im.put(5, y, 'j');
            im.put(378, y, 'j');
        }
    }
}

fn paint() -> Img {
    let mut im = Img::black(384, 224);
    sky(&mut im);
    for x in [8, 375] {
        im.rect(x, 4, 2, 60, 'I');
        im.put(x, 3, '5');
        im.put(x + 1, 3, '5');
    }
    pavilion(&mut im, -4, 124, ['p', 'o'], 1);
    pavilion(&mut im, 336, 124, ['u', 'T'], -1);
    stands(&mut im);
    sand(&mut im);
    for &(x, y) in &TORCHES {
        im.rect(x - 1, y, 3, 22, 'I');
        im.line(x - 1, y, x - 1, y + 21, 'o');
        im.rect(x - 3, y - 1, 7, 3, 'r');
        im.rect(x - 3, y - 1, 7, 1, 'O');
    }
    im
}

pub(super) fn live(im: &mut Img, tick: u32) {
    for i in 0..9 {
        let (x, y) = ((hash(i, 3, 91) % 384) as i32, (hash(i, 9, 92) % 24) as i32);
        if (tick / 9 + i as u32) % 5 == 0 {
            im.put(x, y, 'w');
        }
    }
    for (x, dir, cloth) in [(10, 1, ['7', 'T']), (375, -1, ['1', 'W'])] {
        for k in 0..9 {
            let ripple = ((tick / 4 + k as u32) % 4) as i32;
            let len = 18 - k * 2;
            let y = 6 + k + [0, 1, 1, 0][ripple as usize];
            for j in 0..len.max(2) {
                let xx = x + dir * j;
                im.put(xx, y, if k == 4 || j < 2 { cloth[1] } else { cloth[0] });
            }
        }
    }
    for (apex, dir) in [(22, 1), (362, -1)] {
        let y = 124 - 40 - 17;
        for k in 0..3 {
            let ripple = ((tick / 5 + k as u32) % 3) as i32 - 1;
            for j in 0..9 - k * 2 {
                im.put(apex + dir * j, y + k + if j > 3 { ripple } else { 0 }, if k == 1 { 'T' } else { '7' });
            }
        }
    }
    for (i, &(x, y)) in TORCHES.iter().enumerate() {
        let flick = (hash(i as i32, (tick / 2) as i32, 33) % 3) as i32;
        let r = 26 + flick * 2;
        for dy in -r / 3..=r / 2 {
            for dx in -r..=r {
                let d = ((dx as f32) / r as f32).powi(2) + ((dy as f32) / (r as f32 / 2.2)).powi(2);
                if d < 1.0 && bayer(x + dx, y + dy) > d * 0.95 + 0.1 {
                    let (px, py) = (x + dx, y + 20 + dy);
                    if py > RAIL_Y + 8 && py < FENCE_Y && im.get(px, py).is_some_and(|c| c[0] < 40) {
                        im.put(px, py, if d < 0.4 { 'B' } else { 'I' });
                    }
                }
            }
        }
        let h = 5 + flick;
        im.rect(x - 1, y - 2 - h, 3, h, '@');
        im.rect(x, y - 3 - h, 1, h, '6');
        im.put(x, y - 4 - h + flick % 2, '9');
        im.put(x - 2, y - 2 - h / 2, '4');
        im.put(x + 2, y - 3 - h / 2, '4');
        im.put(x, y - 5 - h, 'R');
    }
}
