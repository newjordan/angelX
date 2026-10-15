//! Props drawn in the shared materials (`overworld::material`), for the
//! rooms of the rooms stream: bronze bells, bone slabs, the ghost
//! choirmaster, straw butts, market stalls, tubs, a copper, a hearth, an
//! armour rack, a fletcher's shed and wheat stooks. Lit from the north-west
//! like everything else, each with its contact shadow.
use super::super::ink::{Img, hash};
use super::super::material::{self as mat, ramp};
use super::nooks::{IRON, LINEN, all};

/// Bell bronze, dark to lit.
pub(super) const BRONZE: &[char] = &['n', 'b', 'I', 'B', 'R', 'o', 'O', 't'];
/// Old bone, ivory.
pub(super) const BONE: &[char] = &['g', 'G', 'J', 'h', 'i', 'T'];
/// A ghost's pallor.
pub(super) const PALE: &[char] = &['x', 'S', 'u', 'U', 'v', 'V'];
/// Copper, for the bathhouse's copper.
pub(super) const COPPER: &[char] = &['b', 'I', 'p', 'R', 'r', 'o', 'O'];

/// A shaded vertical body: each row `half` wide about `cx`, lit across its
/// width from the west.
fn body(cv: &mut Img, cx: i32, rows: impl Iterator<Item = (i32, i32)>, ramp: &[char], seed: u32) {
    for (y, half) in rows {
        for x in cx - half..=cx + half {
            let t = 0.85 - (x - (cx - half)) as f32 / (2 * half + 1) as f32 * 0.75;
            let n = (hash(x, y, seed) % 7) as f32 / 60.0;
            cv.put(x, y, mat::pick(ramp, t + n));
        }
    }
}

/// A bronze bell hung by its yoke at `(x, top)`, its rope down to `rope`
/// (pixels below its mouth). Lit while it sounds; `swing` leans it.
pub(super) fn bell(cv: &mut Img, x: i32, top: i32, lit: bool, swing: i32, rope: i32) {
    // The strap up to the beam.
    cv.rect(x - 1, top - 16, 3, 13, 'B');
    cv.line(x - 1, top - 16, x - 1, top - 4, 'r');
    let x = x + swing;
    // The yoke and its wheel.
    cv.rect(x - 4, top - 3, 9, 3, 'X');
    cv.line(x - 4, top - 3, x + 4, top - 3, 'j');
    // The bell: a crown, a waist, a flared lip.
    let rows = (0..14).map(|k| {
        let half = match k {
            0..=2 => 3 + k,
            3..=9 => 5 + (k - 3) / 3,
            _ => 7 + (k - 9),
        };
        (top + k, half)
    });
    body(cv, x, rows, BRONZE, 71);
    // The lit rim at the mouth, and the clapper.
    let rim = if lit { '6' } else { 'O' };
    cv.line(x - 11, top + 13, x + 11, top + 13, rim);
    cv.line(x - 10, top + 14, x + 10, top + 14, 'B');
    cv.rect(x - 1, top + 14, 3, 3, 'X');
    // A sound-bow band.
    cv.line(x - 7, top + 9, x + 7, top + 9, if lit { 't' } else { 'R' });
    // The rope, down from the wheel to a woollen grip.
    cv.line(x + 5, top - 2, x + 6, top + 14 + rope, 'o');
    cv.line(x + 6, top - 2, x + 7, top + 14 + rope, 'P');
    cv.rect(x + 5, top + 10 + rope, 3, 5, 'R');
    if lit {
        for s in [-1, 1] {
            cv.line(x + s * 14, top + 4, x + s * 17, top + 1, '6');
            cv.line(x + s * 14, top + 9, x + s * 18, top + 9, '6');
        }
    }
}

/// A bone domino stood on end: an ivory slab, a groove across its middle,
/// pips each side. `pips` is the two halves' counts.
pub(super) fn slab(cv: &mut Img, x: i32, foot: i32, pips: (u8, u8)) {
    mat::shadow(cv, ((x + 3) as f32, foot as f32), (5.0, 1.5), &all);
    let (w, h) = (7, 13);
    let (x0, y0) = (x - w / 2, foot - h);
    for yy in y0..foot {
        for xx in x0..x0 + w {
            let t = 0.8 - (xx - x0) as f32 / w as f32 * 0.5 - if yy == foot - 1 { 0.3 } else { 0.0 };
            cv.put(xx, yy, mat::pick(BONE, t));
        }
    }
    cv.line(x0, y0, x0 + w - 1, y0, 'T');
    cv.line(x0, y0 + h / 2, x0 + w - 1, y0 + h / 2, 'G');
    for (half, n) in [(0, pips.0), (1, pips.1)] {
        let cy = y0 + 3 + half * 6;
        let spots: &[(i32, i32)] = match n {
            1 => &[(0, 0)],
            2 => &[(-2, -1), (2, 1)],
            _ => &[(-2, -1), (0, 0), (2, 1)],
        };
        for &(dx, dy) in spots {
            cv.put(x + dx, cy + dy, 'b');
        }
    }
}

/// A fallen domino, lying along `(dx, dy)`.
pub(super) fn slab_down(cv: &mut Img, x: i32, y: i32, (dx, dy): (i32, i32)) {
    let (len, wide) = (12, 3);
    for k in 0..len {
        for s in -wide / 2..=wide / 2 {
            let (px, py) = (x + dx * k - dy * s, y + dy * k + dx * s);
            cv.put(px, py, mat::pick(BONE, 0.45 - s as f32 * 0.15));
        }
    }
    cv.put(x + dx * 3, y + dy * 3, 'b');
    cv.put(x + dx * 9, y + dy * 9, 'b');
}

/// The ghost choirmaster: a pale shade with a round head and a scalloped
/// hem, hollow eyes under raised brows, a mouth open in song, a ruff, and
/// a baton out toward `aim` (pixels), or raised while he waits.
pub(super) fn ghost(cv: &mut Img, x: i32, y: i32, aim: Option<(i32, i32)>, tick: u32) {
    let bob = (tick / 8 % 2) as i32;
    let y = y - bob;
    mat::shadow(cv, ((x + 4) as f32, (y + 8) as f32), (8.0, 2.0), &all);
    // Head and body as one smooth shape, widening to the hem.
    let rows = (0..30).map(|k| {
        let half = match k {
            0..=1 => 3 + k * 2,
            2..=9 => 7,
            10..=12 => 6,
            _ => 6 + (k - 12) / 4,
        };
        (y - 28 + k, half)
    });
    body(cv, x, rows, PALE, 133);
    // The hem: three scallops that wave.
    let wave = (tick / 6 % 2) as i32;
    for k in 0..3 {
        let hx = x - 8 + k * 6 + wave;
        cv.rect(hx, y + 2, 4, 2 + (k + wave) % 2, mat::pick(PALE, 0.35));
    }
    // The face.
    cv.line(x - 5, y - 24, x - 2, y - 25, 'S');
    cv.line(x + 2, y - 25, x + 5, y - 24, 'S');
    cv.rect(x - 4, y - 23, 3, 3, 'K');
    cv.rect(x + 2, y - 23, 3, 3, 'K');
    cv.ellipse(x as f32, (y - 17) as f32, 1.6, 2.2, 'K');
    // A ruff.
    for k in -6..=6 {
        cv.put(x + k, y - 13 + (k.abs() % 2), 'V');
        cv.put(x + k, y - 12, 'U');
    }
    // An arm and the baton.
    let (bx, by) = aim.map_or((x + 13, y - 30), |(ax, ay)| {
        let (dx, dy) = ((ax - x) as f32, (ay - (y - 10)) as f32);
        let l = dx.hypot(dy).max(1.0);
        (x + 8 + (dx / l * 13.0) as i32, y - 10 + (dy / l * 13.0) as i32)
    });
    cv.line(x + 5, y - 10, x + 8, y - 10, mat::pick(PALE, 0.7));
    cv.rect(x + 7, y - 11, 3, 3, mat::pick(PALE, 0.85));
    cv.line(x + 8, y - 10, bx, by, 'T');
    cv.put(bx, by, 'i');
}

/// A straw butt on its stand, rings painted on.
pub(super) fn butt(cv: &mut Img, x: i32, y: i32) {
    mat::shadow(cv, ((x + 5) as f32, (y + 14) as f32), (11.0, 2.5), &all);
    cv.line(x - 7, y + 7, x - 10, y + 15, 'B');
    cv.line(x + 7, y + 7, x + 10, y + 15, 'B');
    mat::lobe(cv, (x as f32, y as f32), (11.0, 10.0), ramp::THATCH, 141, 1.5, 0.08, &all);
    for (r, ink) in [(8.0, 'T'), (6.5, 'R'), (5.0, 'T'), (3.0, 'R'), (1.5, 'O')] {
        cv.ellipse(x as f32 - 0.5, y as f32 - 0.5, r, r * 0.9, ink);
    }
}

/// A market stall: a boarded counter, two posts, a striped awning shaded
/// toward its scalloped edge. `front` is the counter's top (pixels).
pub(super) fn stall(cv: &mut Img, (x, y, w, h): (i32, i32, i32, i32), stripes: (char, char), front: i32, awning: i32, seed: u32) {
    mat::shadow(cv, ((x + w / 2 + 5) as f32, (y + h + 2) as f32), (w as f32 * 0.5, 3.0), &all);
    for px in [x + 1, x + w - 3] {
        cv.rect(px, y, 2, h, 'B');
        cv.line(px, y, px, y + h - 1, 'r');
    }
    mat::boards(cv, (x, front, w, 12), true, seed);
    cv.line(x, front, x + w - 1, front, 'o');
    // Each stall keeps its own canvas: a flat awning with a scalloped
    // hem, a peaked canopy lit on its west slope, or a cloth sagging
    // between its poles. The canvas is lit along its top and goes a step
    // darker toward its hem.
    let (x0, x1) = (x - 2, x + w + 2);
    let (mid, half) = ((x0 + x1) as f32 / 2.0, (x1 - x0) as f32 / 2.0);
    let shape = seed % 3;
    for xx in x0..x1 {
        let band = ((xx - x + 2) / 5) % 2 == 0;
        let ink = if band { stripes.0 } else { stripes.1 };
        let t = (xx - x0) as f32 / (x1 - x0 - 1).max(1) as f32;
        let (rise, sag, scallop) = match shape {
            1 => ((4.0 * (1.0 - ((xx as f32 + 0.5 - mid) / half).abs())).round() as i32, 0, 0),
            2 => (0, (2.0 * 4.0 * t * (1.0 - t)).round() as i32, 0),
            _ => (0, 0, ((xx - x + 2) % 5 == 2) as i32),
        };
        let west = shape == 1 && (xx as f32) < mid;
        let top = awning - rise;
        let bottom = awning + 7 + sag + scallop;
        for yy in top..bottom {
            let k = yy - top;
            let base = super::super::ink::ink(ink);
            let c = if yy >= bottom - 2 || (shape == 1 && !west && k > 0) {
                base.map(mat::darker)
            } else if k == 0 || (west && k < 2) {
                base.map(mat::lighter)
            } else {
                base
            };
            if let Some(c) = c {
                cv.set(xx, yy, c);
            }
        }
        // The ridge or the top edge, a thin dark seam where the canvas
        // meets its frame.
        cv.put(xx, top - 1, 'b');
    }
}

/// A bathing tub of staves and hoops, full of water.
pub(super) fn tub(cv: &mut Img, x: i32, y: i32, seed: u32) {
    mat::shadow(cv, ((x + 6) as f32, (y + 13) as f32), (18.0, 3.0), &all);
    mat::lobe(cv, (x as f32, (y + 5) as f32), (18.0, 10.0), ramp::TIMBER, seed, 2.0, 0.0, &all);
    for k in (-16..=16).step_by(5) {
        cv.line(x + k, y + 3, x + k, y + 13 - (k * k) / 60, 'b');
    }
    for hy in [y + 6, y + 11] {
        cv.line(x - 16, hy, x + 16, hy, 'g');
    }
    cv.ellipse(x as f32, y as f32, 17.0, 7.0, 'P');
    cv.ellipse(x as f32, y as f32, 15.0, 6.0, 'q');
    for k in 0..7 {
        let gx = x - 11 + (hash(k, seed as i32, 151) % 22) as i32;
        cv.line(gx, y - 2 + (k % 4) as i32, gx + 2, y - 2 + (k % 4) as i32, 'Q');
    }
}

/// The copper: a round copper on a brick firebox, a chimney pipe.
pub(super) fn copper(cv: &mut Img, (x, y, w, h): (i32, i32, i32, i32), tick: u32) {
    mat::shadow(cv, ((x + w / 2 + 6) as f32, (y + h + 1) as f32), (w as f32 * 0.6, 3.0), &all);
    mat::block(cv, (x + 2, y + h - 14, w - 4, 14), ramp::STONE, false, 161);
    cv.rect(x + 8, y + h - 8, w - 16, 5, 'k');
    super::nooks::logs(cv, x + w / 2, y + h - 4, tick);
    mat::lobe(cv, ((x + w / 2) as f32, (y + h - 20) as f32), (13.0, 9.0), COPPER, 162, 2.5, 0.0, &all);
    cv.ellipse((x + w / 2) as f32, (y + h - 27) as f32, 11.0, 3.0, 'O');
    cv.ellipse((x + w / 2) as f32, (y + h - 27) as f32, 9.0, 2.0, 'q');
    cv.rect(x + w - 6, y, 3, h - 18, 'X');
    cv.line(x + w - 6, y, x + w - 6, y + h - 19, 'j');
}

/// A smith's hearth: a stone forge, its hood and flue, coals as hot as
/// `glow` (0 dull to 3 white-hot), the bellows' nozzle in its side.
pub(super) fn hearth(cv: &mut Img, (x, y, w, h): (i32, i32, i32, i32), glow: u32, seed: u32) {
    mat::block(cv, (x, y + 10, w, h - 10), ramp::STONE, false, seed);
    for k in 0..12 {
        let inset = 6 + k;
        for xx in x + inset..x + w - inset {
            cv.put(xx, y + 11 - k, mat::pick(ramp::STONE, 0.3 + ((xx - x) as f32 / w as f32) * 0.35));
        }
    }
    cv.rect(x + w / 2 - 7, y - 1, 14, 3, 'X');
    cv.rect(x + 12, y + 20, w - 24, 13, 'k');
    cv.line(x + 12, y + 20, x + w - 13, y + 20, 'J');
    for k in 0..12 {
        let cx = x + 14 + (hash(k, 9, seed) % (w as u32 - 30)) as i32;
        let ink = match glow {
            0 => 'g',
            1 => 'a',
            2 => '@',
            _ => '6',
        };
        cv.rect(cx, y + 27 + (k % 3) as i32 * 2, 3, 2, ink);
    }
}

/// An armour rack of boards: a helm, a mail shirt and a kite shield.
pub(super) fn rack(cv: &mut Img, (x, y, w, h): (i32, i32, i32, i32)) {
    mat::shadow(cv, ((x + w / 2 + 5) as f32, (y + h + 1) as f32), (w as f32 * 0.5, 2.5), &all);
    mat::boards(cv, (x, y + 1, w, 4), false, 171);
    for px in [x + 2, x + w - 4] {
        cv.rect(px, y + 1, 2, h, 'B');
    }
    // The helm.
    mat::lobe(cv, ((x + 13) as f32, (y + 12) as f32), (6.0, 6.0), IRON, 172, 1.5, 0.0, &all);
    cv.line(x + 9, y + 13, x + 17, y + 13, 'K');
    cv.line(x + 13, y + 13, x + 13, y + 17, 'K');
    // The mail shirt: rings of light and shade.
    for yy in y + 6..y + 26 {
        for xx in x + 26..x + 40 {
            let lit = (xx + yy) % 2 == 0;
            cv.put(xx, yy, mat::pick(IRON, if lit { 0.75 } else { 0.4 } - (xx - x - 26) as f32 * 0.02));
        }
    }
    cv.rect(x + 22, y + 6, 4, 8, 'G');
    cv.rect(x + 40, y + 6, 4, 8, 'g');
    // The shield.
    for k in 0..17 {
        let half = 8 - k / 2;
        for dx in -half..=half {
            cv.put(x + 54 + dx, y + 6 + k, mat::pick(ramp::SLATE, 0.75 - (dx + half) as f32 / (2 * half + 1) as f32 * 0.45));
        }
    }
    cv.line(x + 54, y + 7, x + 54, y + 20, 'R');
    cv.line(x + 48, y + 11, x + 60, y + 11, 'R');
}

/// A lean-to shed: a shingled roof on posts, boards at its back.
pub(super) fn shed(cv: &mut Img, (x, y, w, h): (i32, i32, i32, i32), seed: u32) {
    mat::shadow(cv, ((x + w / 2 + 6) as f32, (y + h + 1) as f32), (w as f32 * 0.55, 3.0), &all);
    mat::boards(cv, (x + 2, y + 10, w - 4, h - 12), true, seed);
    mat::roof(cv, (x - 2, y, w + 4, 13), ramp::TIMBER, seed ^ 3);
    for px in [x + 2, x + w - 4] {
        cv.rect(px, y + 12, 3, h - 12, 'b');
        cv.line(px, y + 12, px, y + h - 1, 'r');
    }
}

/// A stook of wheat sheaves.
pub(super) fn stook(cv: &mut Img, x: i32, foot: i32, seed: u32) {
    mat::hay(cv, x, foot, 5.0, seed);
    for k in -2..=2 {
        cv.line(x + k, foot - 9, x + k * 2, foot - 13, 'O');
    }
    cv.line(x - 4, foot - 5, x + 4, foot - 5, 'I');
}

/// A stallholder's face: skin, eyes, a mouth, red as their temper.
pub(super) fn face(cv: &mut Img, x: i32, y: i32, temper: u32) {
    let skin = ['t', 'R', '7'][temper.min(2) as usize];
    mat::lobe(cv, (x as f32, y as f32), (4.5, 4.5), &['o', skin, skin, 't'], 181, 3.0, 0.0, &all);
    cv.put(x - 2, y - 1, 'k');
    cv.put(x + 2, y - 1, 'k');
    cv.put(x, y + 1, 'o');
    cv.line(x - 1, y + 3, x + 1, y + 3, if temper >= 2 { 'k' } else { 'b' });
    let _ = LINEN;
}
