//! The Delve's rock faces: what a seam looks like where a knight finds it.
//! Each stone has its own face, drawn in code on the realm's palette: a
//! limestone face in pale beds, a flint bank banded with chalk, a basalt
//! column, slate in slanting sheets, soft pitted tufa, a clay and peat bank
//! with reeds, a vein of voidstone. A face the party has won is chalked and
//! pegged; one with a mine on it has a ladder and a windlass working. Only
//! plain inks and the violet dye are used: the signal bank is for the claim
//! pennant and nothing else.
use super::*;
use crate::rules::geology::{Face, Grade};

/// A face is this wide and tall, in pixels, standing on its foot.
pub const W: i32 = 42;
pub const H: i32 = 30;

/// What the picture shows of the seam's state.
#[derive(Clone, Copy, Debug, Default)]
pub struct State {
    /// The party has won the seam: chalked and pegged.
    pub secured: bool,
    /// A mine is linked to it: a ladder and a windlass work the face.
    pub camp: bool,
    /// Blocks cut from it, as scars, at most six shown.
    pub cut: u8,
    /// Ticks since a swing landed on it, while the chips fly.
    pub struck: Option<u64>,
    /// The Deep's sentries are back.
    pub raided: bool,
    pub tick: u64,
}

fn n(x: i32, y: i32, seed: u32) -> u32 {
    hash(x, y, seed)
}
fn pick(ramp: &[char], t: f32) -> char {
    let i = ((t.clamp(0.0, 0.999)) * ramp.len() as f32) as usize;
    ramp[i.min(ramp.len() - 1)]
}

/// Where the rock's top edge is at column `x`: smaller is taller.
fn top(face: Face, x: i32, seed: u32) -> i32 {
    let t = x as f32 / (W - 1) as f32;
    let wob = |k: f32, a: f32| vnoise(x as f32 / k, 1.7, seed) * a;
    match face {
        // Terraced beds: two or three steps.
        Face::Limestone => {
            let step = [9, 6, 3, 6, 10][((t * 4.99) as usize).min(4)];
            step + wob(2.5, 1.5) as i32
        }
        // A low banked mound, the flint thrown up in a ridge.
        Face::Flint => (10.0 + (t * 2.0 - 1.0).powi(2) * 10.0 + wob(3.0, 2.0)) as i32,
        // Columns of different heights, a pixel gap between.
        Face::Basalt => {
            let col = x / 6;
            [7, 2, 9, 3, 11, 5, 8][(col as usize) % 7] + (n(col, 0, seed) % 3) as i32
        }
        // Sheets slanting down to the right.
        Face::Slate => (2 + (x / 3) * 8 / 11 + (n(x / 3, 4, seed) % 2) as i32).min(14),
        // A lumpy, rounded heap.
        Face::Tufa => (6.0 + (t * 2.0 - 1.0).powi(2) * 10.0 + wob(2.0, 3.0)) as i32,
        // A low bank, mostly flat.
        Face::Clay => (13.0 + wob(4.0, 3.0)) as i32,
        // Spires of crystal.
        Face::Void => {
            let c = [4.0, 15.0, 27.0, 37.0];
            let h = [12.0, 3.0, 10.0, 17.0];
            let mut best = 20.0f32;
            for (cx, ch) in c.iter().zip(h) {
                let d = (x as f32 - cx).abs();
                best = best.min(ch + d * 2.2);
            }
            best as i32
        }
    }
}

/// The face's own pixel: its character at `(x, y)` of the picture, `depth`
/// rows under its top edge, lit `lit` (0 dark .. 1 bright).
fn pixel(face: Face, x: i32, y: i32, depth: i32, lit: f32, seed: u32) -> char {
    let grain = (n(x, y, seed) & 0xff) as f32 / 255.0;
    let blur = vnoise(x as f32 / 3.0, y as f32 / 3.0, seed ^ 0x55);
    let t = (lit * 0.7 + blur * 0.3 + (grain - 0.5) * 0.12).clamp(0.0, 1.0);
    match face {
        Face::Limestone => {
            // Beds: a dark joint every five or six rows, broken in places,
            // and block joints stepped between them.
            let bed = (y + (n(0, 0, seed) % 3) as i32) % 6 == 0 && n(x / 3, y, seed) % 5 != 0;
            let joint_row = (y + 3) / 6;
            let joint = (x + joint_row * 5 + (n(joint_row, 1, seed) % 7) as i32) % 11 == 0
                && (y + 3) % 6 != 0
                && depth > 1;
            if bed {
                return if t > 0.5 { 'J' } else { 'G' };
            }
            if joint {
                return 'J';
            }
            // A fossil: a little coil, now and then.
            if (x + 3 * y) % 23 == 7 && grain > 0.5 {
                return 'G';
            }
            if grain > 0.97 {
                return 'G';
            }
            pick(&['G', 'J', 'h', 'i', 'i', 'H'], t)
        }
        Face::Flint => {
            // Chalk bands with dark flint between, nodules in the chalk.
            let band = ((y + (x / 9) as i32) / 4) % 2 == 0;
            let nodule = {
                let (cx, cy) = (x / 5, y / 4);
                let (lx, ly) = (x - cx * 5, y - cy * 4);
                n(cx, cy, seed) % 3 == 0 && lx > 0 && lx < 4 && ly > 0 && ly < 3
            };
            if band {
                if nodule {
                    return if lit > 0.6 && n(x, y, seed) % 4 == 0 { 'U' } else { 'k' };
                }
                pick(&['j', 'G', 'h', 'i', 'H'], t)
            } else {
                if grain > 0.9 {
                    return 'v';
                }
                pick(&['k', 'K', 's', 'x', 'S'], t)
            }
        }
        Face::Basalt => {
            // Columns six wide, edged dark, lit on the left of each, a cap
            // on top, a rust crack low down.
            let col = x / 6;
            let cx = x - col * 6;
            if cx == 5 || (cx == 0 && x > 0 && depth > 0 && false) {
                return 'k';
            }
            if depth <= 1 {
                return pick(&['X', 'g', 'j', 'G'], lit + 0.2);
            }
            if y > H - 8 && n(x, y / 2, seed) % 11 == 0 {
                return if n(x, y, seed) % 2 == 0 { 'p' } else { 'r' };
            }
            let side = match cx {
                0 | 1 => 0.75,
                2 | 3 => 0.5,
                _ => 0.25,
            };
            pick(&['K', 'X', 'g', 'j', 'u', 'S'], (side + t * 0.3 - 0.1).clamp(0.0, 1.0))
        }
        Face::Slate => {
            // Sheets: slanting courses of three rows, a dark gap, a sheen
            // on the lit edge, each sheet its own blue-grey, a cleavage
            // crack now and then.
            let k = (y * 11 - x * 3).rem_euclid(33) / 11;
            let sheet = (y * 11 - x * 3).div_euclid(33);
            let tone = (n(sheet, 7, seed) % 5) as f32 / 5.0;
            if k == 0 {
                return 'x';
            }
            if (x + sheet * 3) % 9 == 0 && k == 2 && n(x, sheet, seed) % 3 == 0 {
                return 'x';
            }
            if k == 1 && lit > 0.4 {
                return if tone > 0.5 { 'V' } else { 'v' };
            }
            pick(&['S', 'u', 'u', 'U', 'v'], (tone * 0.7 + t * 0.4).clamp(0.0, 1.0))
        }
        Face::Tufa => {
            // Pale, porous: the stone pitted with holes, moss in the lee.
            let pit = n(x / 2, y / 2, seed) % 6 == 0 && grain > 0.35 && depth > 1;
            if pit {
                return if lit > 0.5 { 'G' } else { 'j' };
            }
            if depth <= 2 && grain > 0.4 {
                return if n(x, 0, seed) % 2 == 0 { 'C' } else { 'y' };
            }
            if grain > 0.88 {
                return 'Y';
            }
            pick(&['G', 'J', 'h', 'h', 'i', 'H'], t)
        }
        Face::Clay => {
            // Turf, then orange clay in layers, a black peat seam, reeds.
            if depth <= 1 {
                return if grain > 0.5 { 'm' } else { 'A' };
            }
            let layer = depth - 2;
            if (6..=9).contains(&layer) {
                return pick(&['n', 'b', 'I', 'b'], grain);
            }
            if layer % 5 == 4 {
                return 'P';
            }
            pick(&['p', 'r', 'R', 'o', 'O'], t * 0.8 + (layer as f32 * 0.05))
        }
        Face::Void => {
            // Violet crystal, dark in its heart, a glint on the lit facets.
            let facet = ((x * 3 + y * 2 + (n(0, 1, seed) % 5) as i32) / 5) % 3;
            if grain > 0.96 && lit > 0.5 {
                return '^';
            }
            match facet {
                0 => pick(&['d', '(', ')', ')', '['], t),
                1 => pick(&['(', ')', '[', '[', ']'], t),
                _ => pick(&['d', 'd', '(', ')', '['], t * 0.7),
            }
        }
    }
}

/// The bare face, standing on its foot, in a transparent picture.
pub fn face_img(face: Face, grade: Grade, seed: u32) -> Img {
    let mut im = Img::new(W, H);
    let mut tops: Vec<i32> = (0..W).map(|x| top(face, x, seed).clamp(0, H - 6)).collect();
    // Rough rock sheds more at its foot, prime rock stands clean.
    let skirt = match grade {
        Grade::Rough => 4,
        Grade::Sound => 2,
        Grade::Prime => 0,
    };
    // Rough rock is ragged along its top; prime rock stands clean.
    if grade == Grade::Rough {
        for (x, t) in tops.iter_mut().enumerate() {
            *t = (*t + (n(x as i32, 21, seed) % 3) as i32).min(H - 6);
        }
    }
    for x in 0..W {
        let from = tops[x as usize];
        for y in from..H - 1 {
            // Lit from the upper left: the left edge and the top are bright,
            // the right and the foot go dark.
            let depth = y - from;
            let across = 1.0 - x as f32 / (W - 1) as f32;
            let down = 1.0 - (y as f32 / (H - 1) as f32) * 0.8;
            let rim = if depth <= 1 { 0.18 } else { 0.0 };
            let tone = match grade {
                Grade::Rough => -0.12,
                Grade::Sound => 0.0,
                Grade::Prime => 0.1,
            };
            let lit = (across * 0.45 + down * 0.45 + rim + 0.1 + tone).clamp(0.0, 1.0);
            // The outline: a dark edge where the rock meets the air.
            let left = x > 0 && tops[x as usize - 1] > y;
            let right = x < W - 1 && tops[x as usize + 1] > y;
            let ch = if (depth == 0 && !matches!(face, Face::Clay)) || ((left || right) && !matches!(face, Face::Clay)) {
                'k'
            } else {
                pixel(face, x, y, depth, lit, seed)
            };
            im.put(x, y, ch);
        }
    }
    // Rubble at the foot.
    let (rub, rub2) = match face {
        Face::Limestone => ('h', 'J'),
        Face::Flint => ('j', 'k'),
        Face::Basalt => ('g', 'K'),
        Face::Slate => ('U', 'S'),
        Face::Tufa => ('G', 'j'),
        Face::Clay => ('o', 'r'),
        Face::Void => (')', '('),
    };
    for k in 0..(3 + skirt * 2) {
        let x = (n(k, 3, seed) % (W as u32 - 2)) as i32 + 1;
        im.put(x, H - 1, if k % 2 == 0 { rub } else { rub2 });
        if k % 3 == 0 {
            im.put(x + 1, H - 1, rub);
            im.put(x, H - 2, rub2);
        }
    }
    // Prime rock glints.
    if grade == Grade::Prime {
        for k in 0..7 {
            let x = (n(k, 9, seed) % (W as u32 - 8)) as i32 + 4;
            let from = tops[x as usize];
            let y = from + 3 + (n(k, 10, seed) % 9) as i32;
            if y < H - 2 {
                im.put(x, y, match face {
                    Face::Limestone | Face::Flint | Face::Tufa => 'H',
                    Face::Basalt => 'S',
                    Face::Slate => 'W',
                    Face::Clay => 'O',
                    Face::Void => '^',
                });
            }
        }
    }
    // Reeds on a clay bank: tall blades at the right.
    if face == Face::Clay {
        for k in 0..6 {
            let x = W - 9 + k * 2 + (n(k, 11, seed) % 2) as i32;
            let hgt = 6 + (n(k, 12, seed) % 5) as i32;
            let base = tops[(x as usize).min(W as usize - 1)];
            for j in 0..hgt {
                im.put(x, base - j, if j == hgt - 1 { 'Y' } else if j % 3 == 0 { 'M' } else { 'y' });
            }
        }
    }
    im
}

/// The seam's picture on `cv` with its foot at `(x, foot)`: the face, its
/// chalk and pennant once won, its camp once a mine works it, flying chips
/// while it is struck.
pub fn paint(cv: &mut Img, face: Face, grade: Grade, seed: u32, x: i32, foot: i32, st: State) {
    let camp = st.camp;
    // With a camp the face stands left of its middle and the windlass right.
    let fx = if camp { x - 10 } else { x };
    let left = fx - W / 2;
    let top_y = foot - H + 1;
    // Ground shadow.
    cv.ellipse(fx as f32, foot as f32 + 0.5, (W / 2 + 2) as f32, 3.0, 'k');
    let img = face_img(face, grade, seed);
    cv.stamp(&img, left, top_y);
    // Scars where blocks were cut.
    for k in 0..st.cut.min(6) {
        let cx = left + 6 + (k as i32 % 3) * 11;
        let cy = foot - 10 + (k as i32 / 3) * 5;
        cv.rect(cx, cy, 5, 3, 'k');
        cv.rect(cx, cy, 5, 1, 'K');
    }
    if st.secured {
        // Chalk on a dark face, red ochre on a pale one.
        let ink = match face {
            Face::Limestone | Face::Tufa => 'R',
            Face::Clay => 'H',
            _ => 'H',
        };
        let (mx, my) = (left + 9, foot - 18);
        cv.line(mx, my, mx + 4, my + 4, ink);
        cv.line(mx + 4, my, mx, my + 4, ink);
        for k in 0..4 {
            cv.rect(left + 21 + k * 2, foot - 17, 1, 4, ink);
        }
        cv.line(left + 20, foot - 16, left + 29, foot - 14, ink);
        // The claim pennant on its peg, planted at the foot.
        let px = left + W - 4;
        cv.rect(px, foot - 15, 1, 15, 'B');
        let wave = [0, 1, 1, 0][((st.tick / 8) % 4) as usize];
        for j in 0..5 {
            cv.rect(px + 1, foot - 15 + j + if j > 2 { wave } else { 0 }, 5 - j, 1, '5');
        }
        cv.put(px, foot - 16, '6');
    }
    if camp {
        let wx = x + 15;
        // A ladder leaning on the face.
        let lx = left + 15;
        cv.line(lx, foot - 1, lx + 5, foot - 20, 'o');
        cv.line(lx + 4, foot - 1, lx + 9, foot - 20, 'o');
        for r in 0..6 {
            let ry = foot - 3 - r * 3;
            let off = (foot - 1 - ry) * 5 / 19;
            cv.line(lx + off, ry, lx + off + 4, ry, 'O');
        }
        // The windlass: two posts, a drum, a crank, a rope and a bucket.
        cv.rect(wx - 7, foot - 16, 2, 16, 'B');
        cv.rect(wx + 6, foot - 16, 2, 16, 'B');
        cv.rect(wx - 8, foot - 17, 17, 2, 'r');
        let spin = (st.tick / 5) % 4;
        cv.rect(wx - 5, foot - 14, 11, 4, 'P');
        for k in 0..4 {
            let sx = wx - 5 + ((k + spin) % 4) as i32 * 3;
            cv.rect(sx, foot - 14, 1, 4, 'b');
        }
        cv.rect(wx + 8, foot - 12, 4, 1, 'O');
        cv.rect(wx + 11, foot - 12 + [0, 1, 2, 1][spin as usize] as i32, 1, 3, 'O');
        // The rope down to a bucket that bobs.
        let bob = [0, 1, 2, 1][spin as usize] as i32;
        cv.rect(wx - 1, foot - 10, 1, 6 + bob, 't');
        cv.rect(wx - 3, foot - 4 + bob, 5, 4, 'o');
        cv.rect(wx - 3, foot - 4 + bob, 5, 1, 'O');
        // A barrel and a pick against the post.
        cv.rect(wx + 9, foot - 5, 5, 5, 'P');
        cv.rect(wx + 9, foot - 4, 5, 1, 'g');
        cv.rect(wx + 9, foot - 1, 5, 1, 'g');
    }
    // Chips flying from the last swing.
    if let Some(age) = st.struck.filter(|&a| a < 10) {
        let (ox, oy) = (left + 11, foot - 13);
        let chip = match face {
            Face::Limestone | Face::Tufa | Face::Flint => 'H',
            Face::Basalt => 'j',
            Face::Slate => 'V',
            Face::Clay => 'O',
            Face::Void => '^',
        };
        for k in 0..4i32 {
            let vx = (k - 2) as f32 * 1.1 - 0.6;
            let a = age as f32;
            let px = ox + (vx * a) as i32;
            let py = oy - (3.0 * a - 0.5 * a * a * 0.55) as i32 - (k % 2);
            cv.put(px, py, chip);
            if k % 2 == 0 {
                cv.put(px + 1, py, 'g');
            }
        }
    }
    if st.raided {
        // A warning mark over the seam while the Deep holds it.
        let (rx, ry) = (fx, foot - H - 3);
        cv.rect(rx - 1, ry - 3, 3, 5, '7');
        cv.rect(rx - 1, ry + 3, 3, 2, '7');
    }
}

#[cfg(all(test, feature = "renderer"))]
#[path = "../../../../tests/render/seams__shots.rs"]
mod shots;
