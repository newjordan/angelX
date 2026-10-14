//! Materials: the realm's one way of drawing stuff.
//!
//! Black paper stays the ground of every picture. What stands on it (trees,
//! water, stone, timber, thatch) is a lit mass with a ramp of its own; what
//! lies flat on it (grass, sand, snow, a trodden road) is a texture of dots
//! laid in two-by-four cells, the same cell a braille glyph fills, with the
//! paper showing between them. One light, from the north-west and a little
//! above, shades every mass: lit rims face up and left, the lower and right
//! edges fall a step into their own bank's shadow, and a mass leaves a dark
//! contact shadow on the ground to its south-east. Texture is grouped marks
//! (leaf clumps, wood grain, stone courses, ripples) keyed on fixed seeds and
//! world pixels, never a wall-clock or a random generator. Every pixel stays
//! on the realm palette; signal inks are never used for material.
//!
//! See `docs/ART-V2.md`, "The material system".

use std::sync::OnceLock;

use super::ink::{BANK, BLACK, Img, PALETTE, Rgb, RgbMap, SIGNAL_BANK, bank_of, hash, ink, rgb, vnoise};

// ─── ramps ───────────────────────────────────────────────────────────────────

/// Material ramps, dark to light. Shadows lean cool, lights lean warm.
pub mod ramp {
    /// Summer oak and ash: navy-green shadow to sunlit leaf.
    pub const OAK: &[char] = &['f', 'D', 'F', 'E', 'l', 'm', 'A', 'y'];
    /// The wood that closes a room: background, so dark and calm.
    pub const EDGE: &[char] = &['f', 'f', 'D', 'F', 'F', 'E', 'e'];
    /// The pine wood that closes a room.
    pub const EDGE_PINE: &[char] = &['s', 'f', 'D', 'F', 'e', 'e'];
    /// An orchard's or a landmark's tree: the focal one, so brighter.
    pub const FOCAL: &[char] = &['D', 'F', 'E', 'l', 'm', 'A', 'M', 'y', 'Y'];
    /// Dusky far canopy and box hedge.
    pub const HEDGE: &[char] = &['f', 'D', 'F', 'E', 'N', 'm', 'L'];
    /// Pine and yew: cold teal.
    pub const PINE: &[char] = &['s', 'D', 'F', 'e', 'e', 'l', 'C'];
    /// Autumn beech and orchard copper.
    pub const COPPER: &[char] = &['n', 'b', 'I', 'p', 'R', 'o', 'O'];
    /// Dressed and field stone: warm greys.
    pub const STONE: &[char] = &['K', 'X', 'g', 'j', 'G', 'J', 'h', 'i'];
    /// Slate, cliff and roof stone: cool blue-greys.
    pub const SLATE: &[char] = &['s', 'x', 'S', 'u', 'U', 'v', 'V'];
    /// Oak boards and beams.
    pub const TIMBER: &[char] = &['n', 'b', 'I', 'B', 'P', 'r', 'R', 'o'];
    /// Thatch and straw.
    pub const THATCH: &[char] = &['b', 'I', 'P', 'r', 'o', 'O', 't'];
    /// Fired roof tile.
    pub const TILE: &[char] = &['n', 'b', 'B', 'p', 'R', 'r', 'o'];
}

/// The realm's one light: from the north-west and a little above.
pub fn sun() -> [f32; 3] {
    super::ink::norm3(-0.55, -0.75, 0.6)
}

pub fn pick(ramp: &[char], t: f32) -> char {
    let i = (t * ramp.len() as f32).floor() as i32;
    ramp[i.clamp(0, ramp.len() as i32 - 1) as usize]
}

fn lum(c: Rgb) -> f32 {
    0.299 * c[0] as f32 + 0.587 * c[1] as f32 + 0.114 * c[2] as f32
}

fn d2(p: Rgb, t: [f32; 3]) -> f32 {
    (0..3).map(|i| (p[i] as f32 - t[i]).powi(2)).sum()
}

/// `(darker, lighter)` for every material ink: one step inside its own bank.
fn steps() -> &'static RgbMap<(Rgb, Rgb)> {
    static STEPS: OnceLock<RgbMap<(Rgb, Rgb)>> = OnceLock::new();
    STEPS.get_or_init(|| {
        let mut out = RgbMap::default();
        for &v in PALETTE.iter().take(BANK * SIGNAL_BANK) {
            let c = rgb(v);
            if c == BLACK {
                continue;
            }
            let bank = bank_of(c).unwrap_or(0);
            let ramp = || PALETTE[bank * BANK..(bank + 1) * BANK].iter().map(|&v| rgb(v));
            let up = [c[0] as f32 * 1.3 + 6.0, c[1] as f32 * 1.3 + 6.0, c[2] as f32 * 1.3 + 6.0];
            let lighter = ramp()
                .filter(|&p| lum(p) > lum(c) + 1.0)
                .min_by(|a, b| d2(*a, up).total_cmp(&d2(*b, up)))
                .unwrap_or(c);
            out.insert(c, (super::light::darker(c), lighter));
        }
        out
    })
}

/// One step lighter in the ink's own bank. Signal and black never move.
pub fn lighter(c: Rgb) -> Rgb {
    steps().get(&c).map_or(c, |s| s.1)
}

/// One step darker in the ink's own bank. Signal and black never move.
pub fn darker(c: Rgb) -> Rgb {
    steps().get(&c).map_or(c, |s| s.0)
}

// ─── lit masses ──────────────────────────────────────────────────────────────

/// A shaded ellipsoid with grouped texture: a leaf clump, a boulder, a bale.
/// `grain` is the clump size in pixels; `rag` roughens the silhouette.
#[allow(clippy::too_many_arguments)]
pub fn lobe(
    cv: &mut Img,
    (cx, cy): (f32, f32),
    (rx, ry): (f32, f32),
    ramp: &[char],
    seed: u32,
    grain: f32,
    rag: f32,
    keep: &dyn Fn(i32, i32) -> bool,
) {
    let light = sun();
    for y in (cy - ry - 1.0).floor() as i32..=(cy + ry + 1.0).ceil() as i32 {
        for x in (cx - rx - 1.0).floor() as i32..=(cx + rx + 1.0).ceil() as i32 {
            if !keep(x, y) {
                continue;
            }
            let (px, py) = (x as f32 + 0.5, y as f32 + 0.5);
            let (dx, dy) = ((px - cx) / rx, (py - cy) / ry);
            let edge = 1.0 - rag * vnoise(px / 2.0, py / 2.0, seed ^ 0x51);
            let d2 = dx * dx + dy * dy;
            if d2 >= edge {
                continue;
            }
            let z = (1.0 - d2 / edge).max(0.0).sqrt();
            let n = super::ink::norm3(dx, dy, z);
            let mut i = (n[0] * light[0] + n[1] * light[1] + n[2] * light[2]) * 0.5 + 0.5;
            // Grouped marks: each clump is lit on its own north-west.
            let c = vnoise(px / grain, py / grain, seed);
            let cl = vnoise((px + 1.0) / grain, (py + 1.0) / grain, seed);
            i += (c - 0.5) * 0.22 + (c - cl) * 0.9;
            // The rim turns away from the light into the darkest step.
            if d2 > edge * 0.86 && dx + dy > -0.2 {
                i -= 0.35;
            }
            cv.put(x, y, pick(ramp, i));
        }
    }
}

/// Dark dots on the ground: a mass's shadow cast south-east on black paper.
pub fn shadow(cv: &mut Img, (cx, cy): (f32, f32), (rx, ry): (f32, f32), keep: &dyn Fn(i32, i32) -> bool) {
    for y in (cy - ry).floor() as i32..=(cy + ry).ceil() as i32 {
        for x in (cx - rx).floor() as i32..=(cx + rx).ceil() as i32 {
            let (dx, dy) = ((x as f32 + 0.5 - cx) / rx, (y as f32 + 0.5 - cy) / ry);
            if dx * dx + dy * dy < 1.0 && keep(x, y) {
                // Anything already on the ground goes dark; bare paper keeps
                // a sparse cool dot so the shadow reads as a shape.
                match cv.get(x, y) {
                    Some(c) if c != BLACK => cv.put(x, y, if (x + y) % 2 == 0 { 'f' } else { 'K' }),
                    _ if (x + 2 * y).rem_euclid(4) == 0 => cv.put(x, y, 'f'),
                    _ => {}
                }
            }
        }
    }
}

/// Kinds of tree.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Leaf {
    Oak,
    /// The dark, quiet wood at a room's edge.
    Edge,
    /// The dark pine wood at a room's edge.
    EdgePine,
    /// A focal tree (orchard, landmark): brighter, outlined.
    Focal,
    Hedge,
    Pine,
    Copper,
}

/// A tree standing with its trunk's foot at `(x, foot)`: a cast shadow, a
/// trunk lit on its left, a canopy of overlapping clumps, back ones first.
pub fn tree(cv: &mut Img, x: i32, foot: i32, r: f32, leaf: Leaf, seed: u32, keep: &dyn Fn(i32, i32) -> bool) {
    let (fx, fy) = (x as f32, foot as f32);
    shadow(cv, (fx + r * 0.45, fy - 1.0), (r * 0.95, r * 0.42), keep);
    if matches!(leaf, Leaf::Pine | Leaf::EdgePine) {
        let tiers_ramp = if leaf == Leaf::Pine { ramp::PINE } else { ramp::EDGE_PINE };
        // Tiers, narrow at the top; each tier a flat lobe.
        let h = r * 2.4;
        cv_trunk(cv, x, foot, 3, keep);
        let tiers = 4;
        for k in 0..tiers {
            let t = k as f32 / (tiers - 1) as f32;
            let cy = fy - h + 3.0 + t * (h - 7.0);
            let rx = r * (0.35 + 0.65 * t);
            lobe(cv, (fx, cy), (rx, r * 0.42), tiers_ramp, seed + k, 2.0, 0.28, keep);
        }
        return;
    }
    let ramp = match leaf {
        Leaf::Oak => ramp::OAK,
        Leaf::Edge => ramp::EDGE,
        Leaf::Focal => ramp::FOCAL,
        Leaf::Hedge => ramp::HEDGE,
        Leaf::Copper => ramp::COPPER,
        Leaf::Pine | Leaf::EdgePine => ramp::PINE,
    };
    // The crown sits over a short visible trunk.
    let cy = fy - r * 1.2;
    cv_trunk(cv, x, foot, (r * 0.6) as i32, keep);
    let j = |k: u32| ((hash(seed as i32, k as i32, 401) % 100) as f32 / 100.0 - 0.5) * r * 0.25;
    // Back clumps, then the crown, then the low front clumps.
    let clumps = [
        (-0.5, -0.3, 0.55),
        (0.5, -0.35, 0.55),
        (0.0, -0.55, 0.6),
        (0.0, 0.05, 0.8),
        (-0.45, 0.35, 0.5),
        (0.48, 0.38, 0.48),
    ];
    for (k, &(ox, oy, s)) in clumps.iter().enumerate() {
        let k = k as u32;
        lobe(
            cv,
            (fx + ox * r + j(k), cy + oy * r + j(k + 9)),
            (r * s, r * s * 0.9),
            ramp,
            seed.wrapping_mul(31) + k,
            2.4,
            0.32,
            keep,
        );
    }
}

fn cv_trunk(cv: &mut Img, x: i32, foot: i32, h: i32, keep: &dyn Fn(i32, i32) -> bool) {
    for y in foot - h.max(2)..foot {
        for (dx, ch) in [(-1, 'P'), (0, 'B'), (1, 'b')] {
            if keep(x + dx, y) {
                cv.put(x + dx, y, ch);
            }
        }
    }
    // Roots spread a pixel each side.
    for (dx, ch) in [(-2, 'I'), (2, 'n')] {
        if keep(x + dx, foot - 1) {
            cv.put(x + dx, foot - 1, ch);
        }
    }
}

/// A low bush.
pub fn bush(cv: &mut Img, x: i32, foot: i32, r: f32, leaf: Leaf, seed: u32) {
    let keep = |_: i32, _: i32| true;
    let ramp = match leaf {
        Leaf::Copper => ramp::COPPER,
        Leaf::Pine | Leaf::EdgePine => ramp::PINE,
        Leaf::Hedge | Leaf::Edge => ramp::HEDGE,
        Leaf::Oak | Leaf::Focal => ramp::OAK,
    };
    shadow(cv, (x as f32 + r * 0.4, foot as f32), (r, r * 0.4), &keep);
    let (fx, fy) = (x as f32, foot as f32 - r * 0.6);
    lobe(cv, (fx - r * 0.45, fy + 1.0), (r * 0.6, r * 0.5), ramp, seed, 1.8, 0.3, &keep);
    lobe(cv, (fx + r * 0.45, fy + 1.0), (r * 0.6, r * 0.5), ramp, seed + 1, 1.8, 0.3, &keep);
    lobe(cv, (fx, fy - r * 0.15), (r * 0.7, r * 0.6), ramp, seed + 2, 1.8, 0.3, &keep);
}

/// A boulder on the ground.
pub fn boulder(cv: &mut Img, x: i32, foot: i32, r: f32, ramp: &[char], seed: u32) {
    let keep = |_: i32, _: i32| true;
    shadow(cv, (x as f32 + r * 0.5, foot as f32), (r * 1.1, r * 0.45), &keep);
    lobe(cv, (x as f32, foot as f32 - r * 0.7), (r, r * 0.75), ramp, seed, 3.2, 0.18, &keep);
    // A crack, dark with a lit lip.
    let (cx, cy) = (x + (hash(seed as i32, 1, 402) % 3) as i32 - 1, foot - (r * 0.9) as i32);
    for k in 0..(r as i32 / 2).max(1) {
        cv.put(cx + k / 2, cy + k, ramp[1]);
    }
}

/// Reeds and rushes: a clump of stems with dark heads.
pub fn reeds(cv: &mut Img, x: i32, foot: i32, seed: u32) {
    let n = 3 + (hash(seed as i32, 0, 403) % 3) as i32;
    for k in 0..n {
        let h = 7 + (hash(seed as i32, k, 404) % 6) as i32;
        let lean = (hash(seed as i32, k, 405) % 3) as i32 - 1;
        let bx = x + k * 2 - n;
        for t in 0..h {
            let sx = bx + lean * t / 5;
            cv.put(sx, foot - t, if t < 2 { 'F' } else if t % 3 == 0 { 'm' } else { 'l' });
        }
        if k % 2 == 0 {
            let tx = bx + lean * h / 5;
            cv.put(tx, foot - h, 'B');
            cv.put(tx, foot - h - 1, 'B');
            cv.put(tx, foot - h - 2, 'I');
        }
    }
}

/// Wild flowers: a few heads on stems, muted.
pub fn flowers(cv: &mut Img, x: i32, y: i32, seed: u32) {
    for k in 0..4 {
        let h = hash(seed as i32, k, 406);
        let (dx, dy) = ((h % 9) as i32 - 4, ((h >> 4) % 5) as i32 - 2);
        cv.put(x + dx, y + dy + 1, 'E');
        cv.put(x + dx, y + dy, ['T', 'V', 'z', 'O'][(h >> 8) as usize % 4]);
    }
}

// ─── water ───────────────────────────────────────────────────────────────────

/// Open water inside the pixel rectangle `(x, y, w, h)`: see `water_field`.
pub fn water(cv: &mut Img, (x0, y0, w, h): (i32, i32, i32, i32), seed: u32, step: u32) {
    let wet = |x: i32, y: i32| x >= x0 && y >= y0 && x < x0 + w && y < y0 + h;
    water_field(cv, (x0, y0, w, h), &wet, seed, step);
}

/// Open water wherever `wet` admits a pixel inside `bounds`: an irregular
/// bank that never leaves the wet pixels, depth from the true distance to
/// the nearest dry pixel, slow swells, ripple glints that drift with `step`,
/// a dark earth lip along each north bank and a pale foam line where the
/// water meets a south bank.
pub fn water_field(cv: &mut Img, (x0, y0, w, h): (i32, i32, i32, i32), wet: &dyn Fn(i32, i32) -> bool, seed: u32, step: u32) {
    if w <= 0 || h <= 0 {
        return;
    }
    // Chamfer distance (thirds of a pixel) to the nearest dry pixel.
    let (uw, uh) = (w as usize, h as usize);
    let far = u32::MAX / 4;
    let mut d: Vec<u32> = (0..uw * uh)
        .map(|i| if wet(x0 + (i % uw) as i32, y0 + (i / uw) as i32) { far } else { 0 })
        .collect();
    let at = |x: i32, y: i32| (y as usize) * uw + x as usize;
    for y in 0..h {
        for x in 0..w {
            let mut v = d[at(x, y)];
            if v == 0 {
                continue;
            }
            for (dx, dy, k) in [(-1, 0, 3), (0, -1, 3), (-1, -1, 4), (1, -1, 4)] {
                let (nx, ny) = (x + dx, y + dy);
                let n = if nx < 0 || ny < 0 || nx >= w { if wet(x0 + nx, y0 + ny) { far } else { 0 } } else { d[at(nx, ny)] };
                v = v.min(n.saturating_add(k));
            }
            d[at(x, y)] = v;
        }
    }
    for y in (0..h).rev() {
        for x in (0..w).rev() {
            let mut v = d[at(x, y)];
            if v == 0 {
                continue;
            }
            for (dx, dy, k) in [(1, 0, 3), (0, 1, 3), (1, 1, 4), (-1, 1, 4)] {
                let (nx, ny) = (x + dx, y + dy);
                let n = if nx < 0 || nx >= w || ny >= h { if wet(x0 + nx, y0 + ny) { far } else { 0 } } else { d[at(nx, ny)] };
                v = v.min(n.saturating_add(k));
            }
            d[at(x, y)] = v;
        }
    }
    let depth = |x: i32, y: i32| -> Option<f32> {
        if x < 0 || y < 0 || x >= w || y >= h {
            return None;
        }
        let v = d[at(x, y)];
        if v == 0 {
            return None;
        }
        let (gx, gy) = (x0 + x, y0 + y);
        let wander = vnoise(gx as f32 / 7.0, gy as f32 / 7.0, seed) * 3.2;
        let depth = v as f32 / 3.0 - 1.0 - wander;
        (depth >= 0.0).then_some(depth)
    };
    for y in 0..h {
        for x in 0..w {
            let Some(dp) = depth(x, y) else { continue };
            let (gx, gy) = (x0 + x, y0 + y);
            if depth(x, y - 1).is_none() {
                // The north bank drops to the water: a dark lip of earth.
                cv.put(gx, gy, 'b');
                continue;
            }
            if depth(x, y + 1).is_none() {
                cv.put(gx, gy, if (gx + seed as i32) % 3 == 0 { 'v' } else { 'U' });
                continue;
            }
            let t = (dp / 10.0).min(1.0);
            let mut ch = if t < 0.25 {
                'u'
            } else if t < 0.6 {
                'q'
            } else {
                'S'
            };
            // Slow swells: broad soft bands a step lighter.
            let swell = vnoise(gx as f32 / 22.0 + step as f32 * 0.03, gy as f32 / 5.0, seed ^ 7);
            if swell > 0.7 && t > 0.3 {
                ch = 'q';
            } else if swell < 0.22 && t > 0.6 {
                ch = 'x';
            }
            // Ripple glints: short dashes on every fourth row, drifting
            // east, the braille cell's pair of dots.
            let row = (gy + (seed % 4) as i32).rem_euclid(4) == 0;
            let lane = (gx + step as i32 / 2 + (hash(0, gy, seed) % 16) as i32).rem_euclid(16);
            if row && lane < 3 && t > 0.15 {
                ch = if lane == 1 && t > 0.4 && hash(gx / 16, gy, seed ^ 3) % 3 == 0 { 'z' } else { 'Q' };
            }
            cv.put(gx, gy, ch);
        }
    }
}

// ─── built surfaces ──────────────────────────────────────────────────────────

/// Coursed stone: blocks of varied tone, dark mortar, each block lit along
/// its top, the bottom course dark with grime.
pub fn ashlar(cv: &mut Img, (x0, y0, w, h): (i32, i32, i32, i32), ramp: &[char], seed: u32) {
    let mid = ramp.len() as i32 / 2;
    for y in y0..y0 + h {
        let course = (y - y0) / 4;
        let ly = (y - y0) % 4;
        let off = if course % 2 == 1 { 4 } else { 0 };
        for x in x0..x0 + w {
            let len = 7;
            let block = (x - x0 + off) / len;
            let lx = (x - x0 + off) % len;
            let tone = hash(block, course, seed) % 5;
            let mut i = mid + match tone {
                0 => 1,
                1 => -1,
                _ => 0,
            };
            if ly == 3 || lx == 0 {
                i = 1;
            } else if ly == 0 {
                i += 1;
            }
            if y >= y0 + h - 2 {
                i -= 1;
            }
            if y == y0 {
                i = ramp.len() as i32 - 1;
            }
            cv.put(x, y, ramp[i.clamp(0, ramp.len() as i32 - 1) as usize]);
        }
    }
}

/// Boards: planks with dark seams, grain streaks and a lit top edge.
pub fn boards(cv: &mut Img, (x0, y0, w, h): (i32, i32, i32, i32), vertical: bool, seed: u32) {
    let ramp = ramp::TIMBER;
    for y in y0..y0 + h {
        for x in x0..x0 + w {
            let (across, along) = if vertical { (x - x0, y - y0) } else { (y - y0, x - x0) };
            let board = across / 4;
            let seam = across % 4 == 3;
            let tone = (hash(board, 0, seed) % 3) as i32;
            let grain = vnoise(along as f32 / 7.0, across as f32 * 0.9 + board as f32 * 5.0, seed ^ 11);
            let mut i = 3 + tone;
            if grain > 0.7 {
                i += 1;
            } else if grain < 0.25 {
                i -= 1;
            }
            if seam {
                i = 1;
            }
            if (!vertical && across == 0) || (vertical && y == y0) {
                i = ramp.len() as i32 - 1;
            }
            if y >= y0 + h - 1 {
                i = 1;
            }
            cv.put(x, y, ramp[i.clamp(0, ramp.len() as i32 - 1) as usize]);
        }
    }
}

/// Roof covering on the pixel rectangle: rows of thatch, tile or slate,
/// the ridge catching the light, the eave in shadow.
pub fn roof(cv: &mut Img, (x0, y0, w, h): (i32, i32, i32, i32), ramp: &[char], seed: u32) {
    roof_hip(cv, (x0, y0, w, h), ramp, seed, 0);
}

/// A hipped roof: its ends slope in by `hip` pixels at the ridge, the hip
/// ends a step darker than the front plane.
pub fn roof_hip(cv: &mut Img, (x0, y0, w, h): (i32, i32, i32, i32), ramp: &[char], seed: u32, hip: i32) {
    let top = ramp.len() as i32 - 1;
    for y in y0..y0 + h {
        let row = (y - y0) / 3;
        let ly = (y - y0) % 3;
        let off = if row % 2 == 1 { 2 } else { 0 };
        // Lighter toward the ridge, darker toward the eave.
        let fall = (y - y0) * 3 / h.max(1);
        let inset = hip * (h - (y - y0)) / h.max(1);
        for x in x0..x0 + w {
            if x < x0 + inset || x >= x0 + w - inset {
                continue;
            }
            let end = x < x0 + inset + hip / 2 || x >= x0 + w - inset - hip / 2;
            let lx = (x - x0 + off) % 5;
            let mut i = top - 1 - fall;
            if hash((x - x0 + off) / 5, row, seed) % 4 == 0 {
                i -= 1;
            }
            if ly == 2 || lx == 0 {
                i -= 1;
            }
            if end && hip > 0 {
                i -= if x < x0 + w / 2 { 1 } else { 2 };
            }
            if y == y0 {
                i = top;
            }
            if y == y0 + h - 1 {
                i = 0;
            }
            cv.put(x, y, ramp[i.clamp(0, top) as usize]);
        }
    }
}

/// A small building seen from the south: a coursed or boarded front with a
/// door and windows, under a roof, its shadow cast south-east.
pub struct House {
    pub roof: &'static [char],
    pub stone: bool,
    pub windows: i32,
}

pub fn house(cv: &mut Img, (x, y, w, h): (i32, i32, i32, i32), hs: &House, seed: u32) {
    let keep = |_: i32, _: i32| true;
    let roof_h = h * 11 / 20;
    let wall_h = h - roof_h;
    shadow(cv, ((x + w) as f32 - 2.0, (y + h) as f32), (w as f32 * 0.35, 4.0), &keep);
    for k in 0..w {
        cv.put(x + k + 2, y + h, if k % 2 == 0 { 'f' } else { 'K' });
    }
    if hs.stone {
        ashlar(cv, (x, y + roof_h, w, wall_h), ramp::STONE, seed);
    } else {
        boards(cv, (x, y + roof_h, w, wall_h), true, seed);
    }
    roof_hip(cv, (x - 2, y, w + 4, roof_h), hs.roof, seed, (roof_h * 2 / 3).min(w / 4));
    // A chimney on the ridge's east end.
    if w > 30 {
        let cx = x + w - 10;
        for yy in y - 4..y + 3 {
            cv.put(cx, yy, 'J');
            cv.put(cx + 1, yy, 'G');
            cv.put(cx + 2, yy, 'g');
            cv.put(cx + 3, yy, 'X');
        }
        cv.line(cx - 1, y - 5, cx + 4, y - 5, 'h');
    }
    // The eave's shadow on the wall.
    for k in 0..w {
        let c = cv.get(x + k, y + roof_h).unwrap_or(BLACK);
        cv.set(x + k, y + roof_h, darker(darker(c)));
    }
    let door_w = 6.min(w / 3);
    let dx = x + w / 2 - door_w / 2;
    cv.rect(dx, y + h - wall_h * 3 / 4, door_w, wall_h * 3 / 4, 'n');
    cv.rect(dx + 1, y + h - wall_h * 3 / 4 + 1, door_w - 2, wall_h * 3 / 4 - 1, 'b');
    cv.line(dx, y + h - wall_h * 3 / 4, dx + door_w - 1, y + h - wall_h * 3 / 4, 'r');
    for k in 0..hs.windows {
        let wx = x + 3 + k * (w - 10) / hs.windows.max(1);
        let wx = if (wx..wx + 4).contains(&dx) || (dx..dx + door_w).contains(&wx) { wx.max(dx + door_w + 2) } else { wx };
        if wx + 4 > x + w - 1 {
            continue;
        }
        let wy = y + roof_h + 2;
        cv.rect(wx, wy, 4, 4, 'k');
        cv.put(wx + 1, wy + 1, 'g');
        cv.line(wx, wy + 4, wx + 3, wy + 4, 'h');
    }
}

/// A solid block seen from the south and above: a lit top face, a front
/// face of coursed stone or boards, a contact shadow at its foot.
pub fn block(cv: &mut Img, (x, y, w, h): (i32, i32, i32, i32), ramp: &[char], boarded: bool, seed: u32) {
    let keep = |_: i32, _: i32| true;
    shadow(cv, ((x + w) as f32 - w as f32 * 0.3, (y + h) as f32 + 1.0), (w as f32 * 0.45, 3.5), &keep);
    let top = (h / 3).clamp(3, 7);
    let n = ramp.len();
    for yy in y..y + top {
        for xx in x..x + w {
            let v = vnoise(xx as f32 / 4.0, yy as f32 / 3.0, seed ^ 0x77);
            let mut i = n as i32 - 2 + if v > 0.65 { 1 } else if v < 0.3 { -1 } else { 0 };
            if yy == y {
                i = n as i32 - 1;
            }
            if xx == x + w - 1 || yy == y + top - 1 {
                i -= 1;
            }
            cv.put(xx, yy, ramp[i.clamp(0, n as i32 - 1) as usize]);
        }
    }
    if boarded {
        boards(cv, (x, y + top, w, h - top), true, seed);
    } else {
        ashlar(cv, (x, y + top, w, h - top), ramp, seed);
    }
    // The front face's top edge sits in the top face's shade.
    for xx in x..x + w {
        if let Some(c) = cv.get(xx, y + top) {
            cv.set(xx, y + top, darker(c));
        }
    }
}

/// A clipped hedge on the pixel rectangle: a lit, leafy top face over a
/// darker side face toward the viewer, a contact shadow below.
pub fn hedge(cv: &mut Img, (x, y, w, h): (i32, i32, i32, i32), seed: u32) {
    let face = 5.min(h / 2);
    for yy in y..y + h {
        for xx in x..x + w {
            let n = vnoise(xx as f32 / 2.8, yy as f32 / 2.8, 73 + seed % 4);
            let nl = vnoise((xx + 1) as f32 / 2.8, (yy + 1) as f32 / 2.8, 73 + seed % 4);
            let side = yy >= y + h - face;
            let mut t = if side { 0.1 + n * 0.3 } else { 0.3 + n * 0.45 + (n - nl) * 1.6 };
            if yy == y || xx == x {
                t += 0.2;
            }
            if xx == x + w - 1 {
                t -= 0.15;
            }
            cv.put(xx, yy, pick(ramp::HEDGE, t));
        }
    }
    for xx in x + 1..x + w + 1 {
        cv.put(xx, y + h, if xx % 2 == 0 { 'f' } else { 'K' });
    }
}

/// A round tower rising from `foot`: a cylinder lit on its left, coursed,
/// with a lantern room and cap at the top.
pub fn tower(cv: &mut Img, cx: i32, foot: i32, r: i32, h: i32, seed: u32) {
    let keep = |_: i32, _: i32| true;
    shadow(cv, ((cx + r) as f32, foot as f32), (r as f32 * 1.6, 4.0), &keep);
    for y in foot - h..foot {
        let course = (y - foot).rem_euclid(5) == 0;
        for dx in -r..=r {
            let n = dx as f32 / r as f32;
            // Cylinder shading: the light from the west and above.
            let t = 0.62 - n * 0.42 - n * n * 0.25 + if hash(dx / 3, y / 5, seed) % 5 == 0 { 0.08 } else { 0.0 };
            let t = if course { t - 0.18 } else { t };
            cv.put(cx + dx, y, pick(ramp::STONE, t));
        }
    }
    // The lantern room: dark glass between posts, a slate cap, a gallery.
    let top = foot - h;
    cv.rect(cx - r - 2, top - 2, 2 * r + 5, 3, 'g');
    cv.line(cx - r - 2, top - 2, cx + r + 2, top - 2, 'J');
    for y in top - 11..top - 2 {
        for dx in -r + 2..=r - 2 {
            cv.put(cx + dx, y, if dx % 4 == 0 { 'X' } else { 'k' });
        }
    }
    for k in 0..6 {
        cv.line(cx - k - 1, top - 12 - (5 - k), cx + k + 1, top - 12 - (5 - k), if k < 2 { 'U' } else { 'u' });
        cv.line(cx + k - 1, top - 12 - (5 - k), cx + k + 1, top - 12 - (5 - k), 'S');
    }
}

/// Which sides of a wall tile face open floor: north, east, south, west.
pub type Open = [bool; 4];

/// A dungeon wall tile seen from above and the south. Its top is a dark,
/// rough cap; a side facing open floor gets a lit lip; a wall with floor to
/// its south shows its coursed front face (or a rough rock face, `rough`),
/// and drops a contact shadow on the floor below it.
pub fn wall_tile(cv: &mut Img, (x, y): (i32, i32), tile: i32, open: Open, ramp: &[char], rough: bool, seed: u32) {
    let n = ramp.len() as i32;
    let face = if open[2] { 9 } else { 0 };
    if rough {
        // A cave wall: packed boulders and fissures, no courses. The stones
        // keyed on world pixels run on across tiles; the face toward open
        // floor is a step darker, its lip lit.
        rubble(cv, (x, y, tile, tile), ramp, 7, seed);
        for yy in 0..tile {
            for xx in 0..tile {
                let (wx, wy) = (x + xx, y + yy);
                let Some(mut c) = cv.get(wx, wy) else { continue };
                if face > 0 && yy >= tile - face {
                    c = darker(c);
                }
                let lip = (open[0] && yy == 0) || (open[3] && xx == 0) || (face > 0 && yy == tile - face - 1);
                if lip && lum(c) > 12.0 {
                    c = lighter(lighter(c));
                }
                cv.set(wx, wy, c);
            }
        }
        if face > 0 {
            for xx in x..x + tile {
                cv.put(xx, y + tile, if xx % 2 == 0 { 'K' } else { 'k' });
            }
        }
        return;
    }
    for yy in y..y + tile - face {
        for xx in x..x + tile {
            // Rubble set in the cap: irregular stones, dark joints, each
            // stone lit along its top.
            let row = yy.div_euclid(5);
            let off = (hash(row, 0, seed) % 6) as i32;
            let stone = (xx + off).div_euclid(6 + (hash(row, 1, seed) % 3) as i32);
            let (lx, ly) = ((xx + off).rem_euclid(6 + (hash(row, 1, seed) % 3) as i32), yy.rem_euclid(5));
            let tone = (hash(stone, row, seed) % 4) as i32;
            let mut i = if lx == 0 || ly == 4 { 0 } else if ly == 0 { 2 } else { 1 + (tone == 0) as i32 };
            // Lit lips where the cap meets open floor.
            if (open[0] && yy == y) || (open[3] && xx == x) {
                i = n - 2;
            } else if (open[0] && yy == y + 1) || (open[3] && xx == x + 1) {
                i = n / 2;
            } else if open[1] && xx == x + tile - 1 {
                i = n / 2 - 1;
            }
            if open[2] && yy == y + tile - face - 1 {
                i = n - 2;
            }
            cv.put(xx, yy, ramp[i.clamp(0, n - 1) as usize]);
        }
    }
    if face > 0 {
        let fy = y + tile - face;
        if rough {
            for yy in fy..y + tile {
                for xx in x..x + tile {
                    let v = vnoise(xx as f32 / 4.0, yy as f32 / 2.5, seed ^ 0x33);
                    let vl = vnoise((xx + 1) as f32 / 4.0, (yy + 1) as f32 / 2.5, seed ^ 0x33);
                    let i = n / 2 + if v - vl > 0.06 { 1 } else if v - vl < -0.06 { -1 } else { 0 } - (yy - fy) / 4;
                    cv.put(xx, yy, ramp[i.clamp(1, n - 1) as usize]);
                }
            }
        } else {
            ashlar(cv, (x, fy, tile, face), ramp, seed);
        }
        // Contact shadow on the floor below the face.
        for xx in x..x + tile {
            cv.put(xx, y + tile, if xx % 2 == 0 { 'K' } else { 'k' });
            if xx % 3 == 0 {
                cv.put(xx, y + tile + 1, 'k');
            }
        }
    }
}

/// Rough rock: packed, irregular stones with black fissures between them,
/// each stone lit on its own north-west and toned on its own. No courses.
/// `size` is the typical stone across, in pixels.
pub fn rubble(cv: &mut Img, (x0, y0, w, h): (i32, i32, i32, i32), ramp: &[char], size: i32, seed: u32) {
    let n = ramp.len() as i32;
    let point = |gx: i32, gy: i32| -> (f32, f32) {
        let v = hash(gx, gy, seed);
        (
            (gx * size) as f32 + (v % 1000) as f32 / 1000.0 * size as f32 * 0.8 + size as f32 * 0.1,
            (gy * size) as f32 + ((v >> 10) % 1000) as f32 / 1000.0 * size as f32 * 0.8 + size as f32 * 0.1,
        )
    };
    for y in y0..y0 + h {
        for x in x0..x0 + w {
            // Keyed on the picture's own pixels, so neighbouring calls join.
            let (px, py) = (x as f32 + 0.5, y as f32 + 0.5);
            let (gx, gy) = (x.div_euclid(size), y.div_euclid(size));
            let (mut d1, mut d2, mut best) = (f32::MAX, f32::MAX, (0, 0, 0.0f32, 0.0f32));
            for dy in -1..=1 {
                for dx in -1..=1 {
                    let (cx, cy) = point(gx + dx, gy + dy);
                    // Stones are a little wider than tall.
                    let d = ((px - cx) * 0.85).hypot(py - cy);
                    if d < d1 {
                        d2 = d1;
                        d1 = d;
                        best = (gx + dx, gy + dy, cx, cy);
                    } else if d < d2 {
                        d2 = d;
                    }
                }
            }
            if d2 - d1 < 1.1 {
                cv.put(x, y, ramp[0]);
                continue;
            }
            let (sx, sy, cx, cy) = best;
            let tone = (hash(sx, sy, seed ^ 0x5a) % 3) as i32 - 1;
            // Lit toward the north-west, falling off to the south-east;
            // the rim nearest a fissure a step darker.
            let lean = ((px - cx) + (py - cy)) / size as f32;
            let mut i = n / 2 + tone - (lean * 3.0).round() as i32;
            if d2 - d1 < 2.2 {
                i -= 1;
            }
            cv.put(x, y, ramp[i.clamp(1, n - 1) as usize]);
        }
    }
}

/// Courses of large blocks for the braille eye: black mortar, a face tone
/// per block, a lit top row and a dark bottom row. Big shapes and black
/// gaps are what survive two-by-four dot cells.
pub fn blocks(cv: &mut Img, (x0, y0, w, h): (i32, i32, i32, i32), ramp: &[char], (bw, bh): (i32, i32), seed: u32) {
    let n = ramp.len() as i32;
    for y in y0..y0 + h {
        let course = (y - y0) / bh;
        let ly = (y - y0) % bh;
        let off = if course % 2 == 1 { bw / 2 } else { 0 };
        for x in x0..x0 + w {
            let block = (x - x0 + off).div_euclid(bw);
            let lx = (x - x0 + off).rem_euclid(bw);
            if ly == bh - 1 || lx == bw - 1 {
                cv.put(x, y, ramp[0]);
                continue;
            }
            let tone = (hash(block, course, seed) % 3) as i32 - 1;
            let mut i = n / 2 + tone;
            if ly == 0 {
                i = n - 1;
            } else if ly == bh - 2 || lx == bw - 2 {
                i -= 2;
            } else if hash(x, y, seed ^ 9) % 11 == 0 {
                i -= 1;
            }
            cv.put(x, y, ramp[i.clamp(1, n - 1) as usize]);
        }
    }
}

/// A post-and-rail fence from `x0` to `x1` with its foot on row `y`.
pub fn fence(cv: &mut Img, x0: i32, x1: i32, y: i32) {
    for x in x0..=x1 {
        cv.put(x + 1, y + 1, if x % 2 == 0 { 'f' } else { 'K' });
        cv.put(x, y - 5, if x % 3 == 0 { 'r' } else { 'B' });
        cv.put(x, y - 2, if x % 3 == 0 { 'r' } else { 'B' });
        cv.put(x, y - 4, 'b');
    }
    let mut x = x0;
    while x <= x1 {
        for k in 0..8 {
            cv.put(x, y - k, if k == 7 { 'o' } else { 'P' });
            cv.put(x + 1, y - k, if k == 7 { 'r' } else { 'b' });
        }
        x += 12;
    }
}

/// A board on two posts carrying `text`, lettered in pale ink. Returns the
/// board's width. `(x, y)` is where the lettering starts.
pub fn sign(cv: &mut Img, x: i32, y: i32, text: &str, ink: char) -> i32 {
    let w = super::ink::text_width(text) + 5;
    let (bx, by) = (x - 3, y - 2);
    for (px, ch) in [(bx + 3, 'P'), (bx + w - 4, 'P')] {
        for k in 0..5 {
            cv.put(px, by + 11 + k, ch);
            cv.put(px + 1, by + 11 + k, 'b');
        }
    }
    boards(cv, (bx, by, w, 11), false, hash(x, y, 407));
    cv.rect(bx + 1, by + 1, w - 2, 9, 'n');
    cv.text(x, y, text, ink);
    w
}

/// A haystack or round bale.
pub fn hay(cv: &mut Img, x: i32, foot: i32, r: f32, seed: u32) {
    let keep = |_: i32, _: i32| true;
    shadow(cv, (x as f32 + r * 0.5, foot as f32), (r * 1.1, r * 0.4), &keep);
    lobe(cv, (x as f32, foot as f32 - r * 0.75), (r, r * 0.8), ramp::THATCH, seed, 1.6, 0.12, &keep);
}

/// A barrel, end up.
pub fn barrel(cv: &mut Img, x: i32, foot: i32) {
    let keep = |_: i32, _: i32| true;
    shadow(cv, (x as f32 + 3.0, foot as f32), (5.0, 2.0), &keep);
    for k in 0..8 {
        let w = if k == 0 || k == 7 { 3 } else { 4 };
        for dx in -w..w {
            let ch = if k == 2 || k == 5 { 'g' } else if dx < -1 { 'r' } else if dx > 1 { 'b' } else { 'B' };
            cv.put(x + dx, foot - 1 - k, ch);
        }
    }
    cv.ellipse(x as f32 - 0.5, (foot - 9) as f32, 3.5, 1.5, 'P');
}

/// A crate.
pub fn crate_box(cv: &mut Img, x: i32, foot: i32, seed: u32) {
    block(cv, (x - 5, foot - 10, 10, 10), ramp::TIMBER, true, seed);
    cv.line(x - 5, foot - 6, x + 4, foot - 1, 'b');
}

// ─── the ground, in braille cells ────────────────────────────────────────────

/// Ground textures. Each two-by-four cell holds at most a small pattern of
/// dots, chosen by a patchy density field, so the ground reads as clumps
/// and bare paper rather than an even speckle.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Ground {
    Meadow,
    Sand,
    Snow,
    Earth,
    Mud,
    Scree,
}

fn cell(x: i32, y: i32) -> (i32, i32, i32, i32) {
    (x.div_euclid(2), y.div_euclid(4), x.rem_euclid(2), y.rem_euclid(4))
}

/// The ink of `ground` at world pixel `(x, y)`, or black paper.
pub fn ground(g: Ground, x: i32, y: i32) -> Option<char> {
    let (cx, cy, lx, ly) = cell(x, y);
    let h = hash(cx, cy, 601 + g as u32);
    let patch = vnoise(x as f32 / 38.0, y as f32 / 30.0, 602 + g as u32);
    let p = (h & 0xff) as f32 / 255.0;
    match g {
        Ground::Meadow => {
            // Tufts of three or four pixels in two or three close, dark
            // tones, gathered in clumps by a slow field; bare paper between.
            // Never a lone pixel.
            let (tx, ty) = (x.div_euclid(6), y.div_euclid(5));
            let (ox, oy) = (x.rem_euclid(6), y.rem_euclid(5));
            let t = hash(tx, ty, 601);
            let e = ((patch - 0.5) / 0.3).clamp(0.0, 1.0);
            if (t & 0xff) as f32 / 255.0 > 0.02 + e * 0.3 {
                return None;
            }
            let (bx, by) = (1 + (t >> 8) as i32 % 3, 1 + (t >> 10) as i32 % 2);
            let (dx, dy) = (ox - bx, oy - by);
            let lit = (t >> 12) % 3 == 0;
            let flower = (t >> 14) % 29 == 0;
            match (dx, dy) {
                (0, 0) if flower => Some(['T', 'V', 'O'][(t >> 16) as usize % 3]),
                (-1, 0) | (1, 0) => Some(if lit { 'l' } else { 'E' }),
                (0, 1) => Some('F'),
                (0, 0) => Some(if lit { 'm' } else { 'l' }),
                (1, -1) if (t >> 18) % 2 == 0 => Some('E'),
                _ => None,
            }
        }
        Ground::Sand => {
            // Wind ripples: dotted crests across the sand, pale on top.
            let wave = ((x as f32 * 0.05 + y as f32 * 0.5 + vnoise(x as f32 / 16.0, y as f32 / 12.0, 603) * 7.0).sin() + 1.0) * 0.5;
            let broken = vnoise(x as f32 / 9.0, y as f32 / 5.0, 604) > 0.42;
            if wave > 0.93 && broken && ly == 0 && lx == 0 && (h >> 3) % 2 != 0 {
                return Some(if (h >> 5) % 2 == 0 { 'O' } else { 'o' });
            }
            if p < 0.1 + patch * 0.12 && lx == ((h >> 11) & 1) as i32 && ly == 2 {
                return Some(if (h >> 9) % 3 == 0 { 'r' } else { 'P' });
            }
            if (h >> 16) % 307 == 0 && lx == 1 && ly == 1 {
                return Some('h');
            }
            None
        }
        Ground::Snow => {
            let drift = patch > 0.45;
            if p < if drift { 0.32 } else { 0.1 } && ly == ((h >> 8) % 4) as i32 && lx == ((h >> 10) & 1) as i32 {
                return Some(if drift { 'v' } else { 'U' });
            }
            if drift && p > 0.94 && ly == 3 && lx == 0 {
                return Some('u');
            }
            None
        }
        Ground::Earth => {
            // Trodden earth: fine grit, and pebbles lit on top.
            if p < 0.05 && lx == 0 && ly == 1 {
                return Some('o');
            }
            if p < 0.05 && lx == 0 && ly == 2 {
                return Some('I');
            }
            if p > 0.05 && p < 0.12 + patch * 0.12 && lx == ((h >> 8) & 1) as i32 && ly == ((h >> 9) % 4) as i32 {
                return Some(if patch > 0.6 { 'P' } else { 'B' });
            }
            None
        }
        Ground::Mud => {
            if patch > 0.62 && ly == 2 && (x + y / 4) % 5 < 2 {
                return Some(if p > 0.7 { 'Q' } else { 'q' });
            }
            if p < 0.14 + patch * 0.1 && lx == ((h >> 8) & 1) as i32 && ly == ((h >> 9) % 4) as i32 {
                return Some(if p < 0.05 { 'P' } else { 'b' });
            }
            None
        }
        Ground::Scree => {
            if p < 0.08 + patch * 0.1 && ly < 2 && lx == ((h >> 8) & 1) as i32 {
                return Some(if ly == 0 { 'J' } else { 'g' });
            }
            None
        }
    }
}

/// A dungeon floor dressed along its walls: flagstone joints (rubble in
/// the caves) that thin out with distance `d` from the wall and are gone a
/// tile away, so the room's edge has structure and its middle stays black.
/// `pack` is the floor's pack index (crypt, cavern, hellforge, archive,
/// fungal, unknown).
pub fn skirting(pack: u32, x: i32, y: i32, d: i32) -> Option<char> {
    if d >= 13 {
        return None;
    }
    let keep = hash(x / 8, y / 6, 620 + pack) % 13 > d as u32;
    if !keep {
        return None;
    }
    let (joint, lit) = match pack {
        0 => ('x', 'S'),
        3 => ('g', 'j'),
        2 => ('X', 'p'),
        5 => ('X', 'g'),
        _ => ('I', 'B'),
    };
    if matches!(pack, 1 | 4) {
        // Caves: rubble, each stone lit on top.
        let h = hash(x / 3, y / 3, 621);
        return match (h % 7, x.rem_euclid(3), y.rem_euclid(3)) {
            (0, 0, 0) => Some(lit),
            (0, 1, 0) | (0, 0, 1) => Some(joint),
            _ => None,
        };
    }
    let row = y.div_euclid(6);
    let off = if row % 2 == 1 { 4 } else { 0 };
    let (lx, ly) = ((x + off).rem_euclid(8), y.rem_euclid(6));
    if ly == 0 && lx != 0 {
        return Some(lit);
    }
    if lx == 0 && (y + x).rem_euclid(2) == 0 {
        return Some(joint);
    }
    None
}

/// A creature in the realm's one light: its silhouette's upper edge lit a
/// step, its lower and right edges a step into shadow, its signal inks
/// (eyes, cores, tells) and near-black outline untouched. Same size, same
/// footprint: only the inks move.
pub fn shade_sprite(im: &Img) -> Img {
    let mut out = im.clone();
    let solid = |x: i32, y: i32| im.get(x, y).is_some_and(|c| c != BLACK);
    for y in 0..im.h {
        for x in 0..im.w {
            let Some(c) = im.get(x, y) else { continue };
            if c == BLACK || lum(c) < 24.0 || bank_of(c).is_none_or(|b| b == SIGNAL_BANK) {
                continue;
            }
            let (up, left, down, right) = (!solid(x, y - 1), !solid(x - 1, y), !solid(x, y + 1), !solid(x + 1, y));
            let near_up = !solid(x, y - 2) && !up;
            out.set(
                x,
                y,
                if up && !down {
                    lighter(c)
                } else if down && !up && !solid(x, y + 1) && solid(x, y - 2) {
                    // Only a body's underside falls into shadow; thin
                    // limbs and spikes keep their ink.
                    darker(c)
                } else if near_up && left && !right {
                    lighter(c)
                } else {
                    c
                },
            );
        }
    }
    out
}

// ─── relief: the shared finish over flat fills ───────────────────────────────

/// Shade every flat fill in the scenery the one way: lit rims up and left,
/// a step of shadow down and right, and grouped texture inside by material
/// (stone courses, wood grain, leaf clumps, ripples). Authored sprites with
/// their own shading, thin lines, lettering and near-black fills are left
/// as drawn: only a pixel whose neighbours are all its own colour or paper
/// counts as a fill.
pub fn relief(cv: &mut Img) {
    let src = cv.clone();
    let water: [Rgb; 4] = ['q', 'Q', 'z', '1'].map(|c| ink(c).unwrap_or(BLACK));
    for y in 0..src.h {
        for x in 0..src.w {
            let Some(c) = src.get(x, y) else { continue };
            if c == BLACK || lum(c) < 22.0 {
                continue;
            }
            let Some(bank) = bank_of(c) else { continue };
            // Foliage and the stone bank (water, slate) are only ever drawn
            // by the material helpers, which shade them already.
            if bank == SIGNAL_BANK || bank == 1 || bank == 3 {
                continue;
            }
            let mut same = 0;
            let mut flat = true;
            for (dx, dy) in [(-1, -1), (0, -1), (1, -1), (-1, 0), (1, 0), (-1, 1), (0, 1), (1, 1)] {
                match src.get(x + dx, y + dy) {
                    Some(q) if q == c => same += 1,
                    Some(q) if q == BLACK => {}
                    None => {}
                    Some(_) => flat = false,
                }
            }
            if !flat || same < 3 {
                continue;
            }
            let paper = |dx: i32, dy: i32| src.get(x + dx, y + dy).is_none_or(|q| q == BLACK);
            let (up, left, down, right) = (paper(0, -1), paper(-1, 0), paper(0, 1), paper(1, 0));
            let out = if (up || left) && !down {
                lighter(c)
            } else if down || right {
                darker(c)
            } else if same == 8 {
                texture(c, bank, water.contains(&c), x, y)
            } else {
                c
            };
            cv.set(x, y, out);
        }
    }
}

fn texture(c: Rgb, bank: usize, wet: bool, x: i32, y: i32) -> Rgb {
    if wet {
        let ripple = y % 4 == 0 && vnoise(x as f32 / 5.0, y as f32, 611) > 0.6;
        return if ripple { lighter(c) } else if vnoise(x as f32 / 18.0, y as f32 / 5.0, 612) < 0.3 { darker(c) } else { c };
    }
    match bank {
        // Structure greys and cool stone: blocks, mortar, a lit top course.
        0 | 3 => {
            let off = if (y / 4) % 2 == 1 { 3 } else { 0 };
            if y % 4 == 3 || (x + off) % 7 == 0 {
                return darker(c);
            }
            match hash((x + off) / 7, y / 4, 613) % 6 {
                0 => lighter(c),
                1 => darker(c),
                _ if y % 4 == 0 => lighter(c),
                _ => c,
            }
        }
        // Foliage: leaf clumps, each lit on its north-west.
        1 => {
            let v = vnoise(x as f32 / 2.4, y as f32 / 2.4, 614);
            let w = vnoise((x + 1) as f32 / 2.4, (y + 1) as f32 / 2.4, 614);
            if v - w > 0.12 {
                lighter(c)
            } else if v < 0.3 {
                darker(c)
            } else {
                c
            }
        }
        // Timber: long grain.
        _ => {
            let v = vnoise(x as f32 / 9.0, y as f32 / 1.4, 615);
            if v > 0.7 {
                lighter(c)
            } else if v < 0.28 {
                darker(c)
            } else {
                c
            }
        }
    }
}

#[cfg(test)]
#[path = "../../../tests/render/overworld__material_tests.rs"]
mod tests;
