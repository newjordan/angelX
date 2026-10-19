//! Set dressing drawn (`crate::world::dressing`): the small stories of the
//! open country, in the realm's dusk inks. Every thing is scenery, so
//! nothing here is lit and nothing is brighter than a quiet grey: a
//! woodcutter's stump and axe, a cairn, bleached bones, an upturned boat,
//! a sunken punt, hay stooks, a hitching rail, a packed-up wagon, a
//! milestone. Each sprite is drawn once and kept; a wild camp's clearing is
//! trodden bare ground under the things round it ([`bay`]), and the
//! footpaths to it are the ground's own (`ground::trail`).

use super::ink::{Img, hash, vnoise};
use super::kit;
use super::light::at_dusk;
use super::{TILE, View};
use crate::world::dressing::{self, Item};
use std::sync::OnceLock;

/// A sprite from its rows of inks.
fn rows(r: &[&str]) -> Img {
    Img::from_rows(r)
}

/// Dark where the thing turns from the light, a coloured contour.
fn rim(im: Img) -> Img {
    kit::shaded_rim(&im)
}

/// A shadow of `w` pixels along the foot of a thing.
fn shadow(im: &mut Img, x: i32, y: i32, w: i32) {
    for dx in 0..w {
        if hash(x + dx, y, 61) % 4 != 0 {
            im.put(x + dx, y, 'K');
        }
    }
}

/// A ragged fill of one thing: ink `a` mostly, `b` and `c` now and then.
fn speckle(im: &mut Img, (x0, y0, w, h): (i32, i32, i32, i32), (a, b, c): (char, char, char), salt: u32) {
    for y in y0..y0 + h {
        for x in x0..x0 + w {
            let n = hash(x, y, salt) % 10;
            im.put(x, y, if n < 6 { a } else if n < 9 { b } else { c });
        }
    }
}

/// Stones set round an ellipse, each by `stone(x, y, k)`.
fn round(cx: f32, cy: f32, rx: f32, ry: f32, n: i32, mut stone: impl FnMut(i32, i32, i32)) {
    let mut at: Vec<(i32, i32, i32)> = (0..n)
        .map(|k| {
            let a = k as f32 * std::f32::consts::TAU / n as f32;
            ((cx + rx * a.cos()) as i32, (cy + ry * a.sin()) as i32, k)
        })
        .collect();
    at.sort_by_key(|s| s.1);
    for (x, y, k) in at {
        stone(x, y, k);
    }
}

fn fallen_log() -> Img {
    let mut im = Img::new(40, 13);
    // The trunk: lit along its back, shadowed underneath, broken at the
    // left, its cut end showing its rings at the right.
    for x in 2..34 {
        let top = 2 + i32::from(x < 6) * 2 + i32::from(x < 4) - i32::from(hash(x, 0, 62) % 9 == 0);
        let bottom = 9 - i32::from(x < 4);
        for y in top..=bottom {
            im.put(x, y, if y == top { 'r' } else if y == top + 1 { 'P' } else if y >= bottom - 1 { 'b' } else { 'B' });
        }
        if hash(x, 1, 63) % 4 == 0 {
            im.put(x, top + 3, 'b');
        }
    }
    im.ellipse(34.0, 6.0, 3.0, 4.4, 'P');
    im.ellipse(34.0, 6.0, 2.0, 3.2, 'r');
    im.ellipse(34.0, 6.0, 1.0, 1.6, 'o');
    // Moss along the back, snapped branch stubs.
    for x in [8, 9, 10, 17, 18, 24, 25, 26] {
        im.put(x, 2, 'l');
        if x % 2 == 0 {
            im.put(x, 1, 'E');
        }
    }
    for (x, y) in [(14, 2), (27, 2)] {
        im.rect(x, y - 2, 2, 3, 'B');
        im.put(x, y - 2, 'P');
    }
    shadow(&mut im, 3, 11, 32);
    rim(im)
}

fn stump_axe() -> Img {
    let mut im = Img::new(24, 20);
    im.rect(2, 9, 12, 8, 'B');
    for x in [4, 7, 10, 13] {
        im.line(x, 10, x, 16, 'b');
    }
    for x in 1..15 {
        im.put(x, 17, 'b');
    }
    im.put(1, 16, 'B');
    im.put(14, 16, 'B');
    im.ellipse(8.0, 9.0, 7.0, 3.2, 'P');
    im.ellipse(8.0, 9.0, 5.8, 2.6, 'r');
    im.ellipse(8.0, 9.0, 3.6, 1.6, 'o');
    im.ellipse(8.0, 9.0, 1.6, 0.8, 'r');
    // The axe, sunk in the top: a haft and a dull blade.
    im.line(9, 8, 17, 1, 'P');
    im.line(10, 8, 18, 1, 'r');
    im.rect(15, 0, 6, 4, 'G');
    im.rect(15, 0, 6, 1, 'J');
    im.rect(19, 1, 2, 3, 'j');
    im.put(20, 3, 'h');
    // Chips round the foot.
    for (x, y) in [(17, 16), (19, 17), (15, 18), (21, 18)] {
        im.rect(x, y, 2, 1, 'r');
    }
    shadow(&mut im, 2, 19, 14);
    rim(im)
}

fn mushroom_ring() -> Img {
    let mut im = Img::new(34, 15);
    round(17.0, 8.0, 14.0, 4.0, 10, |x, y, k| {
        let big = hash(k, 3, 64) % 3 == 0;
        let h = 2 + i32::from(big);
        im.rect(x, y - h, 1, h + 1, 'J');
        let cap = if k % 2 == 0 { 'R' } else { 'r' };
        im.rect(x - 2, y - h - 2, 5, 2, cap);
        im.rect(x - 1, y - h - 3, 3, 1, cap);
        im.put(x - 2, y - h - 1, 'p');
        im.put(x + 2, y - h - 1, 'p');
        im.put(x, y - h - 2, 'h');
    });
    im
}

fn hide() -> Img {
    let mut im = Img::new(30, 26);
    // Forked posts, a ridge pole, and a screen of cut brush with a slit to
    // watch through.
    im.rect(2, 3, 3, 22, 'P');
    im.rect(25, 3, 3, 22, 'P');
    im.rect(2, 3, 1, 22, 'r');
    im.rect(25, 3, 1, 22, 'r');
    im.line(1, 3, 28, 2, 'r');
    im.line(1, 4, 28, 3, 'B');
    for y in 5..19 {
        for x in 5..25 {
            let n = hash(x, y, 65) % 9;
            let leaf = if n < 2 { 'F' } else if n < 5 { 'E' } else if n < 8 { 'l' } else { 'N' };
            if y > 16 && hash(x, y, 66) % 3 == 0 {
                continue;
            }
            im.put(x, y, leaf);
        }
    }
    for x in (7..24).step_by(4) {
        im.line(x, 5, x + 1, 18, 'b');
    }
    im.rect(9, 10, 12, 2, 'k');
    im.put(8, 10, 'F');
    im.put(21, 11, 'F');
    shadow(&mut im, 2, 25, 26);
    rim(im)
}

fn woodpile() -> Img {
    let mut im = Img::new(26, 16);
    for (row, count) in [(11, 5), (7, 4), (3, 3)] {
        let x0 = (26 - count * 5) / 2;
        for k in 0..count {
            let x = x0 + k * 5;
            im.rect(x, row, 5, 5, 'B');
            im.rect(x + 1, row + 1, 3, 3, 'r');
            im.rect(x + 1, row + 1, 2, 2, 'o');
            im.put(x + 2, row + 2, 'P');
        }
    }
    im.rect(2, 1, 22, 2, 'b');
    im.rect(2, 1, 22, 1, 'P');
    shadow(&mut im, 1, 15, 24);
    rim(im)
}

fn cairn() -> Img {
    let mut im = Img::new(17, 21);
    for (cx, cy, rx, ry, c) in [
        (8.5, 16.5, 7.0, 3.8, 'G'),
        (8.5, 12.5, 5.4, 3.0, 'J'),
        (8.0, 9.2, 4.2, 2.6, 'G'),
        (8.5, 6.4, 3.0, 2.2, 'J'),
        (8.5, 4.0, 1.9, 1.6, 'h'),
    ] {
        im.ellipse(cx, cy, rx, ry, c);
        im.line((cx - rx + 1.0) as i32, cy as i32 + 1, (cx + rx - 1.0) as i32, cy as i32 + 1, 'g');
        im.line((cx - rx + 1.0) as i32, cy as i32 - 1, (cx - 1.0) as i32, cy as i32 - 1, 'h');
    }
    shadow(&mut im, 1, 20, 15);
    rim(im)
}

fn fold() -> Img {
    let mut im = Img::new(56, 28);
    // A dry-stone ring seen from the south: the far wall behind, the near
    // wall low with a gap for the flock, a couple of sheep within.
    let (cx, cy, rx, ry) = (28.0f32, 14.0f32, 24.0f32, 8.0f32);
    let mut stones: Vec<(i32, i32, u32)> = Vec::new();
    for k in 0..40 {
        let a = k as f32 * std::f32::consts::TAU / 40.0;
        if (a - std::f32::consts::FRAC_PI_2).abs() < 0.3 {
            continue;
        }
        stones.push(((cx + rx * a.cos()) as i32, (cy + ry * a.sin()) as i32, hash(k, 5, 67)));
    }
    stones.sort_by_key(|s| s.1);
    let mut sheep_done = false;
    for (x, y, h) in stones {
        if y > 14 && !sheep_done {
            sheep_done = true;
            for (sx, sy) in [(17, 12), (33, 15)] {
                im.rect(sx, sy, 7, 4, 'h');
                im.rect(sx + 1, sy - 1, 5, 1, 'J');
                im.rect(sx + 7, sy, 2, 2, 'g');
                im.put(sx + 1, sy + 4, 'g');
                im.put(sx + 5, sy + 4, 'g');
            }
        }
        let tall = if y < 14 { 5 } else { 4 };
        im.rect(x - 2, y - tall, 4, tall, if h % 3 == 0 { 'J' } else { 'G' });
        im.rect(x - 2, y - tall, 4, 1, 'h');
        im.put(x + 1, y - 1, 'g');
    }
    rim(im)
}

fn scree() -> Img {
    let mut im = Img::new(36, 11);
    for k in 0..12 {
        let x = 1 + (hash(k, 1, 68) % 31) as i32;
        let y = 2 + (hash(k, 2, 68) % 6) as i32;
        let big = hash(k, 3, 68) % 3;
        let c = ['G', 'J', 'j', 'G'][(hash(k, 4, 68) % 4) as usize];
        im.rect(x, y, 3 + big as i32, 2 + (big as i32 + 1) / 2, c);
        im.rect(x, y, 3 + big as i32, 1, 'h');
        im.put(x + 2 + big as i32, y + 1, 'g');
    }
    rim(im)
}

fn watch_stub() -> Img {
    // A slate tower broken down to a ragged stump: courses of stone with
    // offset joints, a dark doorway, rubble round the foot.
    let (w, h) = (26, 38);
    let mut im = Img::new(w + 12, h + 4);
    for x in 0..w {
        let top = 4 + (vnoise(x as f32 / 3.0, 0.5, 69) * 13.0) as i32;
        for y in top..h {
            if hash(x, y, 70) % 31 == 0 {
                continue;
            }
            let course = y / 4;
            let joint = (x + course * 5 + (hash(0, course, 71) % 4) as i32) % 8 == 0;
            let ink = if y % 4 == 3 || joint {
                'S'
            } else if x < 3 {
                'U'
            } else if x > w - 5 {
                'x'
            } else if hash(x, y, 72) % 7 == 0 {
                'U'
            } else {
                'u'
            };
            im.put(x + 6, y, ink);
        }
        im.put(x + 6, top, 'U');
    }
    // The doorway and a window slit.
    im.rect(6 + 9, h - 14, 8, 14, 'k');
    im.rect(6 + 9, h - 15, 8, 1, 'S');
    im.rect(6 + 12, h - 28, 2, 5, 'k');
    // Rubble.
    for (x, y) in [(1, 38), (4, 36), (31, 38), (34, 36), (8, 39), (27, 40)] {
        im.rect(x, y - 1, 4, 3, if (x + y) % 2 == 0 { 'u' } else { 'S' });
        im.rect(x, y - 1, 4, 1, 'U');
    }
    shadow(&mut im, 4, 41, 30);
    im
}

fn bones() -> Img {
    let mut im = Img::new(40, 19);
    // The ribs of something large half under the ground, a skull beside.
    im.line(2, 12, 28, 12, 'J');
    im.line(2, 13, 28, 13, 'G');
    im.line(2, 14, 28, 14, 'g');
    for (i, x) in [6, 10, 14, 18, 22].into_iter().enumerate() {
        let h = [7, 10, 12, 10, 7][i];
        for k in 0..h {
            let bend = (k as f32 / h as f32 * 4.0) as i32;
            im.put(x + bend, 12 - k, if k >= h - 2 { 'h' } else { 'J' });
            im.put(x + bend + 1, 12 - k, 'G');
        }
    }
    // The skull, jaw open, and a few long bones.
    im.ellipse(33.0, 11.0, 4.0, 3.4, 'J');
    im.rect(30, 12, 8, 3, 'J');
    im.rect(35, 14, 4, 2, 'G');
    im.rect(31, 10, 2, 2, 'k');
    im.rect(35, 10, 2, 2, 'k');
    im.rect(32, 7, 3, 1, 'h');
    for k in [33, 35, 37] {
        im.put(k, 14, 'h');
    }
    im.rect(0, 16, 6, 2, 'J');
    im.rect(0, 16, 6, 1, 'h');
    im.rect(8, 17, 5, 1, 'J');
    im.rect(26, 16, 4, 2, 'J');
    shadow(&mut im, 2, 18, 30);
    rim(im)
}

fn burnt_cart() -> Img {
    let mut im = Img::new(42, 26);
    // A cart burnt to its frame: charred bed, hoops with the canvas gone,
    // one wheel standing, one down, a broken shaft.
    im.rect(5, 13, 28, 5, 'g');
    im.rect(5, 13, 28, 1, 'j');
    im.rect(5, 17, 28, 1, 'Z');
    for (k, x) in [8, 15, 22, 29].into_iter().enumerate() {
        let top = 3 + (hash(x, 1, 71) % 5) as i32;
        im.line(x, 13, x, top, 'j');
        im.line(x + 1, 13, x + 1, top + 1, 'g');
        if k % 2 == 0 {
            im.line(x, top, x + 5, top + 1, 'j');
        }
    }
    im.ellipse(13.0, 20.0, 6.0, 6.0, 'g');
    im.ellipse(13.0, 20.0, 4.6, 4.6, 'Z');
    for (dx, dy) in [(0, -5), (0, 5), (-5, 0), (5, 0), (3, 3), (-3, -3), (3, -3), (-3, 3)] {
        im.line(13, 20, 13 + dx, 20 + dy, 'j');
    }
    im.put(13, 20, 'G');
    im.ellipse(31.0, 23.0, 7.0, 2.0, 'g');
    im.ellipse(31.0, 23.0, 4.0, 1.0, 'Z');
    im.line(33, 15, 41, 21, 'j');
    im.line(33, 16, 40, 21, 'g');
    for (x, y) in [(1, 22), (17, 24), (23, 20), (38, 24), (21, 14), (3, 18)] {
        im.rect(x, y, 3, 1, 'g');
    }
    rim(im)
}

fn ash_pit() -> Img {
    let mut im = Img::new(28, 14);
    im.ellipse(14.0, 8.0, 12.0, 4.6, 'g');
    im.ellipse(14.0, 8.0, 9.6, 3.4, 'k');
    // Streaks of ash and a half-burnt log across it.
    for (x, y, w) in [(7, 8, 5), (15, 9, 4), (12, 6, 3), (19, 7, 3)] {
        im.rect(x, y, w, 1, 'X');
    }
    im.line(9, 9, 19, 6, 'b');
    im.line(9, 8, 19, 5, 'Z');
    round(14.0, 8.0, 12.0, 4.6, 11, |x, y, k| {
        im.rect(x - 2, y - 2, 4, 3, if k % 2 == 0 { 'J' } else { 'G' });
        im.rect(x - 2, y - 2, 4, 1, 'h');
        im.put(x + 1, y, 'g');
    });
    rim(im)
}

fn boat() -> Img {
    let mut im = Img::new(54, 22);
    // An upturned clinker hull on the sand: its strakes lapped, a keel
    // along its back, both ends lifted, an oar lying beside.
    let (cx, rx, base) = (27.0f32, 24.0f32, 19);
    for x in 2..52 {
        let t = (x as f32 + 0.5 - cx) / rx;
        let dome = 13.0 * (1.0 - t * t).max(0.0).powf(0.7);
        let sheer = 4.0 * t.abs().powi(4);
        let h = (dome + sheer) as i32;
        if h < 1 {
            continue;
        }
        for k in 0..h {
            let y = base - k;
            let strake = k % 4;
            im.put(x, y, if k >= h - 1 { 'r' } else if strake == 3 { 'b' } else if strake == 2 { 'P' } else if k < 2 { 'I' } else { 'B' });
        }
    }
    im.line(5, base - 2, 49, base - 2, 'k');
    for x in [10, 17, 24, 31, 38, 45] {
        im.put(x, base - 7, 'b');
    }
    im.line(3, 21, 22, 20, 'P');
    im.rect(1, 20, 4, 2, 'r');
    shadow(&mut im, 3, 20, 48);
    rim(im)
}

fn nets() -> Img {
    let mut im = Img::new(44, 30);
    // Two poles and a net hung between them to dry, sagging, with cork
    // floats along its head and weights along its foot.
    for x in [3, 38] {
        im.rect(x, 2, 3, 26, 'P');
        im.rect(x, 2, 1, 26, 'r');
        im.rect(x - 1, 1, 5, 1, 'B');
        im.put(x + 1, 0, 'r');
    }
    for x in 6..38 {
        let sag = (5.0 * (1.0 - ((x as f32 - 21.5) / 16.0).powi(2))) as i32;
        let top = 5 + 5 - sag;
        let bottom = top + 13 + (hash(x / 3, 1, 73) % 3) as i32;
        im.put(x, top, 'P');
        for y in top + 1..bottom {
            let diamond = (x + y) % 4 == 0 || (x - y).rem_euclid(4) == 0;
            if diamond {
                im.put(x, y, if y % 3 == 0 { 'J' } else { 'j' });
            }
        }
        if x % 5 == 0 {
            im.rect(x, top - 1, 2, 2, 'R');
            im.rect(x, bottom - 1, 2, 2, 'G');
        }
    }
    shadow(&mut im, 3, 29, 38);
    rim(im)
}

fn driftwood() -> Img {
    let mut im = Img::new(34, 12);
    for (x0, y0, x1, y1) in [(2, 9, 22, 6), (12, 8, 18, 2), (16, 10, 32, 9)] {
        im.line(x0, y0, x1, y1, 'J');
        im.line(x0, y0 + 1, x1, y1 + 1, 'j');
        im.line(x0, y0 - 1, x1, y1 - 1, 'P');
    }
    im.put(24, 6, 'G');
    im.put(5, 7, 'G');
    im.line(18, 3, 15, 0, 'J');
    rim(im)
}

fn tide_pool() -> Img {
    let mut im = Img::new(36, 15);
    im.ellipse(18.0, 8.5, 16.0, 5.6, 'g');
    im.ellipse(18.0, 8.5, 14.4, 4.7, 's');
    im.ellipse(18.0, 8.8, 11.4, 3.5, 'x');
    for (x, y, w) in [(9, 7, 5), (19, 9, 6), (25, 7, 4), (14, 10, 3)] {
        im.rect(x, y, w, 1, 'S');
    }
    round(18.0, 9.0, 16.0, 5.6, 13, |x, y, k| {
        im.rect(x - 1, y - 1, 3, 2, if k % 3 == 0 { 'J' } else { 'G' });
    });
    for (x, y) in [(14, 8), (22, 10)] {
        im.put(x, y, 'u');
        im.put(x + 1, y, 'R');
    }
    im
}

fn punt() -> Img {
    let mut im = Img::new(46, 19);
    // A flat-bottomed punt gone down in the marsh to its gunwales: water
    // about it, its bow lifted, a pole still standing in the mud.
    im.ellipse(23.0, 13.0, 22.0, 5.2, 'x');
    im.ellipse(23.0, 13.5, 17.5, 3.6, 's');
    for x in 6..40 {
        let lift = if x > 31 { (x - 31) * 2 / 3 } else { 0 };
        let y = 9 - lift;
        im.put(x, y, 'o');
        im.put(x, y + 1, 'r');
        im.put(x, y + 2, 'B');
        if hash(x, 3, 72) % 4 != 0 {
            im.put(x, y + 3, 'b');
        }
    }
    for x in [11, 19, 27] {
        im.rect(x, 7, 1, 3, 'r');
    }
    for x in 8..34 {
        if hash(x, 7, 73) % 3 == 0 {
            im.put(x, 14, 'S');
        }
    }
    im.line(5, 14, 10, 0, 'P');
    im.line(6, 14, 11, 0, 'r');
    im
}

fn eel_traps() -> Img {
    let mut im = Img::new(30, 15);
    // Two wicker pots lying on their sides, their mouths narrowing to a
    // dark funnel, roped to a stake.
    for (x, y, salt) in [(2, 3, 74u32), (15, 5, 75)] {
        im.ellipse((x + 6) as f32, (y + 4) as f32, 6.5, 4.2, 'P');
        for k in 0..30 {
            let (wx, wy) = (x + (hash(k, 1, salt) % 12) as i32, y + (hash(k, 2, salt) % 8) as i32);
            if im.get(wx, wy).is_some() {
                im.put(wx, wy, 'r');
            }
        }
        for dx in [3, 6, 9] {
            im.line(x + dx, y, x + dx, y + 8, 'B');
        }
        im.ellipse((x + 11) as f32, (y + 4) as f32, 2.0, 2.6, 'k');
        im.put(x + 1, y + 2, 'o');
    }
    im.line(13, 11, 20, 10, 'g');
    im.line(27, 3, 27, 13, 'P');
    im.rect(26, 2, 3, 1, 'r');
    shadow(&mut im, 2, 14, 26);
    rim(im)
}

fn reeds() -> Img {
    let mut im = Img::new(18, 24);
    for i in 0..12 {
        let x = 2 + i;
        let lean = (i - 6) / 2;
        let top = 3 + (hash(i, 1, 74) % 5) as i32;
        let ink = ['N', 'E', 'F', 'N', 'E'][(hash(i, 2, 74) % 5) as usize];
        im.line(x, 22, x + lean, top, ink);
    }
    im.rect(3, 15, 12, 1, 'b');
    im.rect(3, 16, 12, 1, 'B');
    for x in [4, 8, 12] {
        im.rect(x, 0 + (x % 3), 2, 4, 'B');
        im.put(x, 0 + (x % 3), 'r');
    }
    shadow(&mut im, 2, 23, 14);
    im
}

fn stilt_hut() -> Img {
    let mut im = Img::new(34, 42);
    // Water about the stilts.
    im.ellipse(17.0, 37.0, 16.0, 4.2, 'x');
    im.ellipse(17.0, 37.5, 12.0, 2.6, 's');
    // Four stilts, the hut leaning a little to the right, cross-braced.
    for (x, lean) in [(7, 0), (11, 0), (23, 1), (27, 1)] {
        im.line(x, 24, x + lean, 38, 'P');
        im.line(x + 1, 24, x + 1 + lean, 38, 'I');
    }
    im.line(8, 30, 24, 33, 'b');
    im.line(24, 30, 8, 34, 'b');
    // The platform and the hut, each row of the wall a step further over.
    im.rect(4, 22, 28, 3, 'B');
    im.rect(4, 22, 28, 1, 'r');
    for row in 0..14 {
        let shift = (14 - row) / 5;
        im.rect(8 + shift, 8 + row, 18, 1, if row % 3 == 2 { 'b' } else { 'I' });
    }
    im.rect(14 + 1, 14, 5, 8, 'k');
    // The thatch.
    for row in 0..9 {
        let w = 6 + row * 2;
        let x = 17 + 1 - w / 2 + (9 - row) / 4;
        for dx in 0..w {
            let n = hash(x + dx, row, 75) % 5;
            im.put(x + dx, row + 1, if n == 0 { 'm' } else if n < 3 { 'N' } else { 'E' });
        }
    }
    // A ladder up to it, a fish rack at the side.
    im.line(2, 25, 5, 37, 'P');
    im.line(5, 25, 8, 37, 'P');
    for y in (27..37).step_by(3) {
        im.line(3 + (y - 25) / 4, y, 6 + (y - 25) / 4, y, 'r');
    }
    im.line(28, 18, 32, 18, 'P');
    for x in [29, 31] {
        im.line(x, 18, x, 21, 'g');
    }
    rim(im)
}

fn stooks() -> Img {
    let mut im = Img::new(30, 16);
    for (x0, h, salt) in [(1, 13, 76u32), (14, 15, 77)] {
        let w = 12;
        for y in 0..h {
            let half = ((y as f32 + 3.0) / (h as f32 + 3.0) * (w as f32 / 2.0)) as i32;
            let cx = x0 + w / 2;
            for dx in -half..=half {
                let n = hash(cx + dx, y, salt) % 6;
                let ink = if dx > half - 2 { 'P' } else if n == 0 { 'o' } else if n < 4 { 'r' } else { 'P' };
                im.put(cx + dx, 16 - h + y - 1, ink);
            }
        }
        let cx = x0 + w / 2;
        im.rect(cx - 3, 16 - h + 5, 7, 1, 'b');
        im.put(cx, 16 - h - 2, 'r');
        im.put(cx - 1, 16 - h - 1, 'o');
        im.put(cx + 1, 16 - h - 1, 'r');
    }
    for (x, y) in [(10, 14), (13, 15), (26, 14), (3, 15)] {
        im.rect(x, y, 3, 1, 'r');
    }
    shadow(&mut im, 1, 15, 26);
    rim(im)
}

fn scarecrow() -> Img {
    let mut im = Img::new(22, 32);
    im.line(11, 9, 11, 30, 'P');
    im.line(12, 9, 12, 30, 'I');
    // Arms along a cross-pole in a patched coat, straw at the cuffs.
    im.rect(1, 12, 20, 2, 'P');
    im.rect(5, 12, 12, 10, 'j');
    for y in 13..21 {
        if hash(y, 1, 78) % 2 == 0 {
            im.rect(6 + (y % 4), y, 3, 2, 'p');
        }
    }
    for x in [6, 8, 10, 13, 15] {
        im.put(x, 22, 'j');
        if x % 2 == 1 {
            im.put(x, 23, 'g');
        }
    }
    im.rect(5, 12, 1, 10, 'G');
    for y in [12, 13] {
        im.put(0, y, 'o');
        im.put(21, y, 'r');
    }
    im.put(1, 14, 'r');
    im.put(20, 14, 'o');
    // A sack head, stitched, under a floppy hat.
    im.ellipse(11.5, 7.0, 3.6, 3.6, 'J');
    im.put(10, 6, 'k');
    im.put(13, 6, 'k');
    im.line(10, 9, 13, 9, 'b');
    im.rect(6, 4, 12, 1, 'b');
    im.rect(8, 0, 8, 4, 'B');
    im.put(14, 3, 'b');
    // A crow on one arm.
    im.rect(16, 9, 4, 3, 'k');
    im.put(20, 9, 'k');
    im.put(21, 10, 'o');
    im.put(15, 11, 'K');
    shadow(&mut im, 7, 31, 9);
    rim(im)
}

fn stile() -> Img {
    let mut im = Img::new(38, 17);
    // A dry-stone wall with a gap, steps of through-stones stepping up its
    // face on each side.
    for (x0, w) in [(0, 14), (24, 14)] {
        for y in 4..15 {
            for x in x0..x0 + w {
                let course = (y + hash(x0, y / 3, 79) as i32) % 4;
                let seam = (x + course * 3) % 6 == 0;
                im.put(x, y, if seam { 'g' } else if y < 7 { 'J' } else if course == 0 { 'j' } else { 'G' });
            }
        }
        im.rect(x0, 3, w, 1, 'h');
    }
    im.rect(11, 9, 6, 2, 'J');
    im.rect(11, 11, 6, 1, 'g');
    im.rect(21, 7, 6, 2, 'J');
    im.rect(21, 9, 6, 1, 'g');
    // The step in the gap and a post to hold.
    im.rect(15, 13, 8, 2, 'G');
    im.rect(18, 1, 3, 13, 'P');
    im.rect(18, 1, 1, 13, 'r');
    im.rect(18, 0, 3, 1, 'B');
    shadow(&mut im, 0, 16, 38);
    rim(im)
}

fn hives() -> Img {
    let mut im = Img::new(32, 20);
    // Three skeps on a plank bench.
    im.rect(1, 11, 30, 2, 'B');
    im.rect(1, 11, 30, 1, 'r');
    for x in [3, 27] {
        im.rect(x, 13, 2, 6, 'b');
    }
    for (cx, h) in [(7, 9), (16, 10), (25, 9)] {
        for y in 0..h {
            let half = (((y as f32 + 1.5) / h as f32).sqrt() * 5.0) as i32;
            for dx in -half..=half {
                im.put(cx + dx, 11 - h + y, if (y + 1) % 3 == 0 { 'P' } else if dx < 0 && y % 3 == 1 { 'o' } else { 'r' });
            }
        }
        im.rect(cx - 1, 9, 2, 2, 'k');
    }
    shadow(&mut im, 2, 19, 28);
    rim(im)
}

fn rail() -> Img {
    let mut im = Img::new(38, 20);
    for x in [3, 32] {
        im.rect(x, 3, 3, 16, 'P');
        im.rect(x, 3, 1, 16, 'r');
        im.rect(x, 2, 3, 1, 'B');
    }
    for y in [5, 11] {
        im.rect(1, y, 36, 2, 'B');
        im.rect(1, y, 36, 1, 'r');
    }
    // A horseshoe nailed up, a rope looped through.
    im.put(3, 8, 'J');
    im.put(5, 8, 'J');
    im.put(4, 9, 'J');
    im.line(16, 13, 16, 16, 'J');
    im.line(17, 16, 20, 16, 'J');
    im.line(21, 13, 21, 16, 'J');
    for (x, y) in [(9, 19), (22, 19), (27, 18)] {
        im.rect(x, y, 3, 1, 'r');
    }
    shadow(&mut im, 2, 19, 34);
    rim(im)
}

fn lance_rack() -> Img {
    let mut im = Img::new(34, 32);
    for x in [4, 27] {
        im.rect(x, 9, 3, 22, 'B');
        im.rect(x, 9, 1, 22, 'r');
    }
    im.rect(3, 14, 28, 2, 'B');
    im.rect(3, 14, 28, 1, 'r');
    // Lances in the rack, one snapped, one with a torn pennon.
    for (x0, x1, top) in [(9, 13, 0), (16, 19, 11), (23, 25, 2)] {
        im.line(x0, 30, x1, top, 'P');
        im.line(x0 + 1, 30, x1 + 1, top, 'r');
        im.line(x0 + 2, 30, x1 + 2, top, 'I');
    }
    im.rect(12, 0, 3, 2, 'J');
    im.rect(24, 1, 3, 2, 'J');
    for k in 0..7 {
        im.rect(14 + k, 2 + k / 2, 1, 5 - k / 2, 'p');
    }
    im.line(18, 29, 31, 26, 'P');
    im.line(18, 30, 31, 27, 'I');
    im.rect(31, 26, 2, 2, 'r');
    shadow(&mut im, 3, 31, 28);
    rim(im)
}

fn wagon() -> Img {
    let mut im = Img::new(50, 34);
    // A fair's wagon packed up: its hood laced shut in grey canvas, a band
    // of faded stripes along it, shafts down on the ground.
    let (cx, rx) = (26.0f32, 20.0f32);
    for x in 6..46 {
        let t = (x as f32 + 0.5 - cx) / rx;
        if t.abs() > 1.0 {
            continue;
        }
        let top = 23 - (15.0 * (1.0 - t * t).sqrt()) as i32;
        for y in top..23 {
            let band = (y - top) >= 6 && (y - top) < 10;
            im.put(
                x,
                y,
                if band && x % 4 != 0 {
                    'p'
                } else if (x - 6) % 8 == 0 {
                    'g'
                } else if y == top {
                    'J'
                } else if t > 0.6 {
                    'g'
                } else {
                    'j'
                },
            );
        }
    }
    im.rect(4, 23, 42, 3, 'I');
    im.rect(4, 23, 42, 1, 'r');
    for (cx, cy) in [(14, 27), (37, 27)] {
        im.ellipse(cx as f32, cy as f32, 6.5, 6.5, 'B');
        im.ellipse(cx as f32, cy as f32, 5.0, 5.0, 'b');
        for (dx, dy) in [(0, -5), (0, 5), (-5, 0), (5, 0), (4, 4), (-4, -4), (4, -4), (-4, 4)] {
            im.line(cx, cy, cx + dx, cy + dy, 'r');
        }
        im.put(cx, cy, 'o');
    }
    im.line(5, 25, 0, 32, 'P');
    im.line(7, 26, 2, 33, 'I');
    im.rect(0, 32, 3, 1, 'r');
    shadow(&mut im, 7, 33, 36);
    rim(im)
}

fn bunting() -> Img {
    let mut im = Img::new(42, 44);
    for x in [3, 37] {
        im.rect(x, 3, 3, 40, 'P');
        im.rect(x, 3, 1, 40, 'r');
        im.rect(x - 1, 2, 5, 1, 'B');
    }
    // Dark flags on a sagging line.
    let flags = ['p', 'N', 'S', 'r', 'p', 'E', 'u'];
    for x in 6..37 {
        let sag = (6.0 * (1.0 - ((x as f32 - 21.0) / 15.0).powi(2))) as i32;
        let y = 5 + 6 - sag;
        im.put(x, y, 'J');
        if x % 5 == 1 {
            let c = flags[((x - 6) / 5) as usize % flags.len()];
            for k in 0..5 {
                im.rect(x + i32::from(k == 4), y + 1 + k, 4 - (k + 1) / 2, 1, c);
            }
        }
    }
    shadow(&mut im, 2, 43, 38);
    rim(im)
}

fn burnt_post() -> Img {
    let mut im = Img::new(24, 42);
    // A watch post burnt to a split pole: its platform gone to stubs, the
    // rungs of the ladder charred, ash heaped at its foot.
    im.rect(9, 5, 5, 35, 'g');
    im.rect(9, 5, 1, 35, 'j');
    im.rect(13, 6, 1, 34, 'Z');
    im.line(9, 5, 7, 1, 'j');
    im.line(13, 5, 15, 0, 'j');
    for y in (12..38).step_by(4) {
        im.line(4, y, 9, y, 'g');
    }
    im.rect(3, 12, 2, 26, 'g');
    im.rect(3, 12, 1, 26, 'j');
    im.rect(5, 17, 4, 2, 'j');
    im.rect(14, 15, 7, 2, 'g');
    im.rect(19, 15, 2, 4, 'Z');
    im.ellipse(11.5, 40.0, 9.0, 2.2, 'X');
    for (x, y) in [(5, 39), (14, 40), (17, 38), (3, 41)] {
        im.rect(x, y, 3, 1, 'g');
    }
    shadow(&mut im, 3, 41, 16);
    rim(im)
}

fn milestone() -> Img {
    let mut im = Img::new(20, 16);
    // A low waymarker, its face cut slanting back, a plate let into it
    // with the miles counted in notches.
    for x in 3..17 {
        let top = 3 + (17 - x) / 4;
        for y in top..14 {
            im.put(x, y, if y == top { 'h' } else if x < 5 { 'J' } else if x > 14 { 'g' } else { 'G' });
        }
    }
    im.rect(6, 8, 8, 4, 'g');
    im.rect(6, 8, 8, 1, 'j');
    for x in [8, 10, 12] {
        im.line(x, 9, x, 11, 'h');
    }
    for x in [1, 2, 17, 18] {
        im.put(x, 14, 'l');
    }
    im.put(2, 13, 'E');
    shadow(&mut im, 2, 15, 16);
    rim(im)
}

fn cross() -> Img {
    let mut im = Img::new(18, 30);
    for (y, w) in [(26, 16), (23, 12), (20, 8)] {
        let x = (18 - w) / 2;
        im.rect(x, y, w, 3, if w == 8 { 'J' } else { 'G' });
        im.rect(x, y, w, 1, 'J');
        im.rect(x, y + 2, w, 1, 'g');
    }
    im.rect(8, 6, 3, 15, 'G');
    im.rect(8, 6, 1, 15, 'J');
    im.rect(3, 9, 13, 3, 'G');
    im.rect(3, 9, 13, 1, 'J');
    im.rect(8, 3, 3, 3, 'G');
    im.put(8, 3, 'J');
    // Weathering and ivy up the foot.
    im.line(8, 14, 10, 16, 'X');
    for (x, y) in [(8, 19), (9, 18), (8, 17), (7, 22), (10, 21), (9, 20)] {
        im.put(x, y, if (x + y) % 2 == 0 { 'E' } else { 'F' });
    }
    shadow(&mut im, 1, 29, 16);
    rim(im)
}

fn trough() -> Img {
    let mut im = Img::new(36, 16);
    im.rect(1, 5, 34, 8, 'I');
    im.rect(1, 5, 34, 1, 'r');
    im.rect(1, 12, 34, 1, 'b');
    im.rect(3, 6, 30, 3, 'x');
    for x in [6, 15, 24] {
        im.rect(x, 6, 5, 1, 'S');
    }
    for x in [3, 31] {
        im.rect(x, 13, 3, 3, 'b');
    }
    for x in [12, 23] {
        im.line(x, 9, x, 12, 'b');
    }
    shadow(&mut im, 2, 15, 32);
    rim(im)
}

fn stocks() -> Img {
    let mut im = Img::new(32, 24);
    // Two posts and a hinged board bored for heads and hands; a few old
    // splats of rotten fruit on the ground, and no one in them.
    for x in [3, 24] {
        im.rect(x, 4, 5, 18, 'P');
        im.rect(x, 4, 1, 18, 'r');
        im.rect(x, 3, 5, 1, 'B');
    }
    im.rect(6, 9, 21, 4, 'B');
    im.rect(6, 9, 21, 1, 'r');
    im.rect(6, 13, 21, 4, 'I');
    im.rect(6, 13, 21, 1, 'k');
    for x in [11, 16, 21] {
        im.ellipse(x as f32, 13.0, 2.2, 2.2, 'k');
    }
    im.rect(2, 21, 7, 1, 'X');
    im.rect(23, 21, 7, 1, 'X');
    for (x, y, c) in [(11, 21, 'N'), (14, 22, 'p'), (19, 21, 'N'), (21, 22, 'm')] {
        im.rect(x, y, 3, 1, c);
    }
    shadow(&mut im, 2, 23, 28);
    rim(im)
}

fn build_sprite(item: Item) -> Img {
    match item {
        Item::FallenLog => fallen_log(),
        Item::StumpAxe => stump_axe(),
        Item::MushroomRing => mushroom_ring(),
        Item::Hide => hide(),
        Item::Woodpile => woodpile(),
        Item::Cairn => cairn(),
        Item::Fold => fold(),
        Item::Scree => scree(),
        Item::WatchStub => watch_stub(),
        Item::Bones => bones(),
        Item::BurntCart => burnt_cart(),
        Item::AshPit => ash_pit(),
        Item::Boat => boat(),
        Item::Nets => nets(),
        Item::Driftwood => driftwood(),
        Item::TidePool => tide_pool(),
        Item::Punt => punt(),
        Item::EelTraps => eel_traps(),
        Item::Reeds => reeds(),
        Item::StiltHut => stilt_hut(),
        Item::Stooks => stooks(),
        Item::Scarecrow => scarecrow(),
        Item::Stile => stile(),
        Item::Hives => hives(),
        Item::Rail => rail(),
        Item::LanceRack => lance_rack(),
        Item::Wagon => wagon(),
        Item::Bunting => bunting(),
        Item::Barrels => rim(at_dusk(&kit::barrels())),
        Item::BurntPost => burnt_post(),
        Item::Milestone => milestone(),
        Item::Cross => cross(),
        Item::Barrow => rim(at_dusk(&kit::handcart())),
        Item::Trough => trough(),
        Item::Stocks => stocks(),
        Item::Stall => rim(at_dusk(&kit::market_stall(0))),
        Item::Sacks => rim(at_dusk(&kit::sacks(7))),
        Item::Garden => rim(at_dusk(&kit::garden())),
    }
}

/// Every thing's sprite, plain and turned about, drawn once.
fn sprites() -> &'static Vec<(Item, Img, Img)> {
    static SPRITES: OnceLock<Vec<(Item, Img, Img)>> = OnceLock::new();
    SPRITES.get_or_init(|| {
        Item::ALL
            .iter()
            .map(|&item| {
                let plain = build_sprite(item);
                let turned = plain.flip_h();
                (item, plain, turned)
            })
            .collect()
    })
}

/// The sprite of `item`, turned about or not.
pub fn sprite(item: Item, flip: bool) -> &'static Img {
    let (_, plain, turned) = sprites().iter().find(|(i, _, _)| *i == item).expect("every item has a sprite");
    if flip { turned } else { plain }
}

/// The ink of the trodden clearing at world pixel `(wx, wy)` for the bay
/// centred on world pixel `(cx, cy)`, or `None` for bare paper: a loose oval
/// of worn ground, thinning to nothing at its edge.
pub fn bay(wx: i32, wy: i32, (cx, cy): (i32, i32)) -> Option<char> {
    let (dx, dy) = ((wx - cx) as f32 / 27.0, (wy - (cy + 3)) as f32 / 16.0);
    let wobble = (vnoise(wx as f32 / 9.0, wy as f32 / 7.0, 80) - 0.5) * 0.45;
    let d = (dx * dx + dy * dy).sqrt() + wobble;
    if d >= 1.0 {
        return None;
    }
    let n = hash(wx, wy, 81) % 100;
    let density = ((1.0 - d) * 80.0) as u32;
    if n >= density.min(55) {
        return None;
    }
    Some(match n % 7 {
        0 => 'X',
        1 | 2 => 'I',
        _ => 'b',
    })
}

/// Stamp every thing in `view` onto the landscape `cv`, back to front,
/// except those standing on ground the party has cleared for a wish.
pub fn paint(cv: &mut Img, view: View, hidden: &dyn Fn(i32, i32) -> bool) {
    for p in &dressing::get().pieces {
        let (x, y) = p.at;
        // Anything within a sprite's width of the view might show.
        let (px, base) = (x * TILE + TILE / 2, y * TILE + TILE - 2);
        if px < view.x - 56 || px > view.x + view.w + 56 || base < view.y || base > view.y + view.h + 48 {
            continue;
        }
        let reach = p.item.reach();
        if (x - reach..=x + reach).any(|tx| hidden(tx, y)) {
            continue;
        }
        let im = sprite(p.item, p.flip);
        cv.stamp(im, px - im.w / 2 - view.x, base - im.h - view.y);
    }
}

#[cfg(test)]
#[path = "../../../tests/render/overworld__dressing_tests.rs"]
mod tests;
