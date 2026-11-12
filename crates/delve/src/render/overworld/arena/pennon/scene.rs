//! The siege by night: a moon over a camp of fires and engines, three
//! planes sliding at their own pace behind the rampart, a moat that laps the
//! wall, torches that flicker and pennants that wave. The planes are drawn
//! once and kept (`scene`); a frame lays windows of them down and adds what
//! moves (fires, stars, water, flags, embers).

use super::super::super::ink::{Img, bayer, hash};
use super::super::pfx::{self, Pen, Ramp, Shape, Spray, ramps};
use super::hud::{OY, VIEW_BOTTOM, VIEW_TOP};
use super::{Art, art};
use crate::rules::pennon::{FLOOR, WIDTH, ramparts};
use crate::render::overworld::arena::{NATIVE_H, NATIVE_W};
use std::sync::OnceLock;

/// The rows a layer keeps: the world's window.
const VH: i32 = VIEW_BOTTOM - VIEW_TOP;
/// How fast each plane slides against the rampart (percent of its pace).
const FAR: i32 = 12;
const MID: i32 = 36;
const WATER: i32 = 80;
/// Where the walkway stands in a layer's own rows.
pub(super) const WALK_ROW: i32 = FLOOR + OY - VIEW_TOP;
/// The moat's surface, in layer rows.
const MOAT_ROW: i32 = 176;

pub(super) struct Scene {
    sky: Img,
    clouds: Img,
    far: Img,
    mid: Img,
    near: [Img; 3],
}

/// Fires of the far camp and of the engines: layer column and row.
const FAR_FIRES: [i32; 8] = [30, 98, 176, 247, 318, 389, 440, 478];
/// The far ridge: its row and how far it rises and falls.
const FAR_BASE: f32 = 108.0;
const FAR_AMP: f32 = 13.0;
const MID_FIRES: [(i32, i32); 5] = [(88, 140), (214, 118), (338, 144), (460, 138), (590, 142)];
/// Torches on the rampart: world column, and the row they burn at (layer
/// rows).
const TORCHES: [i32; 7] = [96, 214, 372, 498, 640, 786, 1010];
/// Pennant poles along the walk.
pub(super) const POLES: [i32; 5] = [36, 302, 576, 856, 1172];

fn stop(stops: &[(f32, char)], t: f32, x: i32, y: i32) -> char {
    let t = t.clamp(0.0, 1.0);
    for w in stops.windows(2) {
        let ((t0, a), (t1, b)) = (w[0], w[1]);
        if t <= t1 {
            let k = (t - t0) / (t1 - t0).max(1e-4);
            return if bayer(x, y) < k { b } else { a };
        }
    }
    stops[stops.len() - 1].1
}

fn sky() -> Img {
    let mut im = Img::black(NATIVE_W, NATIVE_H);
    for y in VIEW_TOP..VIEW_BOTTOM {
        let t = (y - VIEW_TOP) as f32 / VH as f32;
        for x in 0..NATIVE_W {
            let ch = stop(&[(0.0, 'k'), (0.3, 'k'), (0.52, 'K'), (0.7, 's'), (0.85, 'x'), (1.0, 'S')], t, x, y);
            im.put(x, y, ch);
        }
    }
    // A warm breath on the horizon, from the fires beyond.
    for y in 118..150 {
        let k = 1.0 - ((y - 134) as f32 / 16.0).abs();
        for x in 0..NATIVE_W {
            if k > 0.0 && bayer(x, y) < k * 0.28 {
                im.put(x, y, 'X');
            }
        }
    }
    // Stars, and the moon's halo first.
    let moon = (338.0f32, 98.0f32);
    for y in VIEW_TOP..150 {
        for x in 0..NATIVE_W {
            let d = ((x as f32 - moon.0).powi(2) + (y as f32 - moon.1).powi(2)).sqrt();
            if d > 14.0 && d < 40.0 {
                let k = (1.0 - (d - 14.0) / 26.0).powi(2) * 0.55;
                if bayer(x, y) < k {
                    im.put(x, y, if k > 0.3 { 'S' } else { 'x' });
                }
            }
        }
    }
    for i in 0..64 {
        let x = (hash(i, 1, 0x51) % NATIVE_W as u32) as i32;
        let y = VIEW_TOP + 2 + (hash(i, 2, 0x51) % 96) as i32;
        let d = ((x as f32 - moon.0).powi(2) + (y as f32 - moon.1).powi(2)).sqrt();
        if d < 40.0 {
            continue;
        }
        im.put(x, y, ['u', 'U', 'v', 'u'][(hash(i, 3, 0x51) % 4) as usize]);
    }
    // The moon, a crescent lit from the upper left.
    for y in 78..118 {
        for x in 318..360 {
            let (dx, dy) = (x as f32 - moon.0, y as f32 - moon.1);
            let d = (dx * dx + dy * dy).sqrt();
            if d > 13.0 {
                continue;
            }
            let bite = ((x as f32 - (moon.0 + 7.0)).powi(2) + (y as f32 - (moon.1 - 3.0)).powi(2)).sqrt();
            if bite < 11.0 {
                continue;
            }
            let u = (dx * -0.6 + dy * -0.8) / 13.0;
            let ch = if u > 0.45 { 'H' } else if u > 0.0 { 'i' } else if u > -0.5 { 'h' } else { 'J' };
            im.put(x, y, ch);
            if hash(x, y, 0x77) % 23 == 0 && ch != 'H' {
                im.put(x, y, 'J');
            }
        }
    }
    im
}

/// Long thin cloud, dithered, a wrap of 768 columns.
fn clouds() -> Img {
    let mut im = Img::new(768, 70);
    for c in 0..9 {
        let cx = (hash(c, 7, 0xc1) % 768) as f32;
        let cy = 8.0 + (hash(c, 8, 0xc1) % 52) as f32;
        let (rx, ry) = (46.0 + (hash(c, 9, 0xc1) % 50) as f32, 3.0 + (hash(c, 10, 0xc1) % 5) as f32);
        for y in 0..70 {
            for dx in -(rx as i32) - 1..=(rx as i32) + 1 {
                let x = (cx as i32 + dx).rem_euclid(768);
                let (u, v) = (dx as f32 / rx, (y as f32 - cy) / ry);
                let d = u * u + v * v;
                if d < 1.0 && bayer(x, y) < (1.0 - d) * 0.8 {
                    im.put(x, y, if d < 0.35 { 'S' } else { 'x' });
                }
            }
        }
    }
    im
}

fn ridge(x: i32, base: f32, amp: f32, s: f32) -> i32 {
    let x = x as f32;
    (base - amp * ((x * 0.013 + s).sin() * 0.55 + (x * 0.031 + s * 2.1).sin() * 0.3 + (x * 0.071 + s * 3.7).sin() * 0.15)) as i32
}

fn far() -> Img {
    let w = NATIVE_W + 100;
    let mut im = Img::new(w, VH);
    for x in 0..w {
        let top = ridge(x, FAR_BASE, FAR_AMP, 1.0);
        for y in top..VH {
            let ch = if y == top { 'S' } else if y < top + 3 { 'x' } else { 's' };
            im.put(x, y, ch);
        }
    }
    // Tents and standards along the ridge.
    for i in 0..14 {
        let x = 10 + i * 36 + (hash(i, 1, 0xf0) % 14) as i32;
        let base = ridge(x, FAR_BASE, FAR_AMP, 1.0) + 3;
        let h = 6 + (hash(i, 2, 0xf0) % 5) as i32;
        for k in 0..h {
            let half = (h - k) * 6 / h;
            for dx in -half..=half {
                im.put(x + dx, base - k, if dx == -half { 'S' } else if dx > half / 2 { 'K' } else { 'X' });
            }
        }
        im.put(x, base - h - 1, 'g');
        im.put(x, base - h - 2, 'R');
        im.put(x + 1, base - h - 2, 'R');
        // A lit door.
        im.put(x, base, 'a');
        im.put(x, base - 1, 'o');
    }
    im
}

fn mid() -> Img {
    let w = NATIVE_W + (WIDTH - NATIVE_W) * MID / 100 + 8;
    let mut im = Img::new(w, VH);
    let ground = 178;
    // A low rampart of earth and stakes.
    for x in 0..w {
        let top = ground - 7 - ((x as f32 * 0.09).sin() * 2.0) as i32;
        for y in top..VH {
            im.put(x, y, if y == top { 'Z' } else if y < top + 3 { 'X' } else { 'K' });
        }
        if x % 5 == 0 {
            let h = 5 + (hash(x, 3, 0xe1) % 3) as i32;
            for k in 0..h {
                im.put(x, top - k, if k == h - 1 { 'o' } else { 'X' });
            }
        }
    }
    // A siege tower, wheels and a lowered bridge.
    for (base, x, h) in [(ground - 6, 84, 74), (ground - 6, 330, 58), (ground - 6, 520, 66)] {
        let (tw, top) = (26, base - h);
        for y in top..base {
            for dx in 0..tw {
                let edge = dx == 0 || dx == tw - 1;
                let beam = (y - top) % 12 == 0 || ((dx + (y - top)) % 13 == 0 && !edge);
                im.put(x + dx, y, if edge { 'B' } else if beam { 'p' } else { 'I' });
            }
        }
        for dx in 0..tw {
            im.put(x + dx, top, 'o');
            im.put(x + dx, top + 1, 'R');
        }
        // Slits and a lit loft.
        for k in 0..3 {
            im.rect(x + 6 + (k % 2) * 8, top + 8 + k * 14, 3, 6, 'n');
        }
        im.rect(x + 4, top + 3, 4, 3, 'a');
        // The drawbridge, down toward the wall.
        for k in 0..16 {
            im.put(x + tw + k, top + 8 + k / 3, 'P');
            im.put(x + tw + k, top + 9 + k / 3, 'b');
        }
        // Wheels.
        for wx in [x + 3, x + tw - 8] {
            im.ellipse(wx as f32 + 3.0, base as f32 - 1.0, 4.0, 4.0, 'K');
            im.ellipse(wx as f32 + 3.0, base as f32 - 1.0, 2.0, 2.0, 'B');
        }
        // A black banner on the top.
        for k in 0..9 {
            im.put(x + 4 + k, top - 12 + (k / 3), 'K');
            im.put(x + 4 + k, top - 11 + (k / 3), 'K');
            im.put(x + 4 + k, top - 10 + (k / 3), if k < 6 { '7' } else { 'K' });
        }
        im.rect(x + 3, top - 14, 1, 14, 'g');
    }
    // Trebuchets: an A-frame, a long arm and a hanging weight.
    for x in [210, 430, 640] {
        let base = ground - 6;
        im.line(x, base, x + 10, base - 32, 'B');
        im.line(x + 22, base, x + 12, base - 32, 'B');
        im.line(x + 3, base - 12, x + 19, base - 12, 'p');
        // The arm tilts back, the sling trails.
        im.line(x - 14, base - 22, x + 30, base - 40, 'o');
        im.line(x - 14, base - 23, x + 30, base - 41, 'R');
        im.rect(x + 27, base - 40, 6, 7, 'g');
        im.rect(x + 28, base - 39, 4, 5, 'K');
        im.line(x - 14, base - 22, x - 22, base - 12, 'J');
        im.ellipse((x - 22) as f32, base as f32 - 10.0, 3.0, 3.0, 'G');
    }
    // Ladders against nothing, leaning.
    for x in [150, 400, 590] {
        let base = ground - 6;
        im.line(x, base, x + 18, base - 40, 'P');
        im.line(x + 4, base, x + 22, base - 40, 'P');
        for r in 0..9 {
            let k = r as f32 / 9.0;
            im.line(x + (k * 18.0) as i32, base - (k * 40.0) as i32, x + 4 + (k * 18.0) as i32, base - (k * 40.0) as i32, 'o');
        }
    }
    im
}

/// The rampart, three seeds (the breaches shift a little with the seed).
fn near(seed: usize, a: &Art) -> Img {
    let mut im = Img::new(WIDTH, VH);
    for p in ramparts(seed as u64) {
        let (x0, x1) = (p.x, p.x + p.w);
        // The back wall: dark, crenelled, with a lit coping.
        for x in x0..x1 {
            let crenel = (x - x0) % 24 < 15;
            let h = if crenel { 24 } else { 15 };
            for dy in 0..h {
                let y = WALK_ROW - 1 - dy;
                let ch = if dy == h - 1 { 'j' } else if dy >= h - 3 { 'g' } else if (x + y) % 11 == 0 { 'X' } else { 'K' };
                im.put(x, y, ch);
            }
            // A seam between blocks.
            if (x - x0) % 24 == 15 || (x - x0) % 24 == 0 {
                for dy in 0..15 {
                    im.put(x, WALK_ROW - 1 - dy, 'k');
                }
            }
        }
        // The walkway's lit edge, then the brick of the wall face.
        for x in x0..x1 {
            im.put(x, WALK_ROW, 'h');
            im.put(x, WALK_ROW + 1, 'J');
        }
        for dx in 0..p.w {
            for dy in 0..34 {
                if let Some(c) = a.all["brick"].body.get(dx % 32, dy % 16) {
                    im.set(p.x + dx, WALK_ROW + 1 + dy, c);
                }
            }
        }
        // The wall's foot in the water, darker.
        for x in x0..x1 {
            for dy in 35..(VH - WALK_ROW - 1).min(60) {
                im.put(x, WALK_ROW + 1 + dy, if dy > 38 { 'k' } else { 'K' });
            }
            // A shadow under the lip.
            im.put(x, WALK_ROW + 2, 'K');
            if bayer(x, 1) < 0.5 {
                im.put(x, WALK_ROW + 3, 'K');
            }
        }
        for x in (p.x + 10..p.x + p.w - 22).step_by(64) {
            im.stamp(&a.all["merlon"].body, x, WALK_ROW + 3);
        }
    }
    // The breaches: broken teeth, rubble and an iron tie hanging.
    let ps = ramparts(seed as u64);
    for (i, p) in ps.iter().enumerate() {
        // The right edge of each plank but the last: a ragged end.
        if i + 1 < ps.len() {
            let edge = p.x + p.w;
            for k in 0..9 {
                let bite = (hash(edge + k, i as i32, 0xb1) % 5) as i32 + k / 2;
                for dy in 0..bite.min(10) {
                    im.clear(edge - 1 - k, WALK_ROW + 1 + dy);
                }
            }
            // A fallen tie rod, dangling.
            for dy in 0..12 {
                im.put(edge - 4 - (dy / 5), WALK_ROW + 6 + dy, if dy % 3 == 0 { 'J' } else { 'G' });
            }
            // Rubble heaped on the near foot.
            for k in 0..14i32 {
                let h = (7 - (k - 7).abs()).max(0) / 2 + (hash(k, i as i32, 0xb2) % 2) as i32;
                for dy in 0..h {
                    im.put(edge - 18 + k, WALK_ROW + 48 - dy, if dy == h - 1 { 'J' } else { 'g' });
                }
            }
        }
        // The left end of each plank but the first.
        if i > 0 {
            let edge = p.x;
            for k in 0..9 {
                let bite = (hash(edge + k, i as i32, 0xb3) % 5) as i32 + k / 2;
                for dy in 0..bite.min(10) {
                    im.clear(edge + k, WALK_ROW + 1 + dy);
                }
            }
        }
    }
    // Torches' brackets on the back wall, and a seat of soot above.
    for &x in &TORCHES {
        if ps.iter().any(|p| x > p.x + 6 && x < p.x + p.w - 6) {
            im.rect(x - 1, WALK_ROW - 17, 3, 6, 'g');
            im.rect(x - 2, WALK_ROW - 12, 5, 2, 'j');
            im.rect(x, WALK_ROW - 11, 1, 5, 'G');
            im.rect(x - 3, WALK_ROW - 19, 7, 2, 'k');
        }
    }
    im
}

pub(super) fn scene() -> &'static Scene {
    static SCENE: OnceLock<Scene> = OnceLock::new();
    SCENE.get_or_init(|| {
        let a = art();
        Scene { sky: sky(), clouds: clouds(), far: far(), mid: mid(), near: std::array::from_fn(|i| near(i, a)) }
    })
}

/// Everything behind the actors for a knight at world column `cam`, `t`
/// ticks into the siege. Returns the canvas with the world laid on it.
pub(super) fn paint(world: &mut Img, seed: u64, cam: i32, t: u32) {
    let sc = scene();
    world.stamp(&sc.sky, 0, 0);
    // Twinkling stars.
    {
        let mut pen = Pen::new(world, 400);
        for i in 0..22 {
            let x = (hash(i, 1, 0x51) % NATIVE_W as u32) as i32;
            let y = VIEW_TOP + 2 + (hash(i, 2, 0x51) % 96) as i32;
            if ((x - 338).pow(2) + (y - 98).pow(2)) < 40 * 40 {
                continue;
            }
            let phase = ((t as f32 * 0.05 + hash(i, 4, 0x51) as f32 * 0.001).sin() * 0.5 + 0.5).powi(3);
            if phase > 0.4 {
                pen.twinkle((x as f32, y as f32), 1.0 + phase, 0.5, ramps::BONE);
            }
        }
    }
    // The clouds drift on their own.
    let drift = (cam * 5 / 100 + (t / 11) as i32).rem_euclid(768);
    for k in [-1, 0] {
        world.stamp(&sc.clouds, k * 768 - drift + 768, 22);
    }
    // The far camp, its fires and their smoke.
    let fx = cam * FAR / 100;
    world.stamp(&sc.far, -fx, VIEW_TOP);
    for (i, &x) in FAR_FIRES.iter().enumerate() {
        let y = ridge(x, FAR_BASE, FAR_AMP, 1.0) + 2;
        flame(world, (x - fx) as f32, (y + VIEW_TOP) as f32, t, i as u32, 1);
    }
    // The engines.
    let mx = cam * MID / 100;
    world.stamp(&sc.mid, -mx, VIEW_TOP);
    for (i, &(x, y)) in MID_FIRES.iter().enumerate() {
        let (sx, sy) = ((x - mx) as f32, (y + VIEW_TOP) as f32);
        flame(world, sx, sy, t, 10 + i as u32, 2);
        if i % 2 == 0 && sx > -20.0 && sx < NATIVE_W as f32 + 20.0 {
            smoke(world, (sx, sy), t, i as u32);
        }
    }
    volleys(world, cam, t);
    moat(world, cam, t);
    let near = &sc.near[(seed % 3) as usize];
    world.stamp(near, -cam, VIEW_TOP);
    // Torches burn on the back wall and throw a little light on it.
    for &x in &TORCHES {
        let sx = x - cam;
        if !(-30..NATIVE_W + 30).contains(&sx) {
            continue;
        }
        let ps = ramparts(seed);
        if !ps.iter().any(|p| x > p.x + 6 && x < p.x + p.w - 6) {
            continue;
        }
        let sy = WALK_ROW + VIEW_TOP - 21;
        flame(world, sx as f32, sy as f32, t, 40 + x as u32, 2);
        pool(world, (sx, sy + 4), 30, t, x as u32);
    }
}

/// Fire arrows and the odd boulder, lobbed over the camp toward the wall: an
/// arc each, a bright head and a streak that cools behind it.
fn volleys(im: &mut Img, cam: i32, t: u32) {
    let mut pen = Pen::new(im, 200);
    for k in 0..4u32 {
        let life = 70 + k * 9;
        let age = (t + k * 53) % (life + 90);
        if age >= life {
            continue;
        }
        let round = (t + k * 53) / (life + 90);
        let x0 = 20.0 + (hash(round as i32, k as i32, 0x71) % 320) as f32 - (cam as f32 * 0.2) % 40.0;
        let dir = if (hash(round as i32, k as i32, 0x72) & 1) == 0 { 1.0 } else { -1.0 };
        let reach = 150.0 + (hash(round as i32, k as i32, 0x73) % 80) as f32;
        let (u, v) = (age as f32 / life as f32, (age as f32 - 1.0).max(0.0) / life as f32);
        let at = |u: f32| (x0 + dir * reach * u, 150.0 - 26.0 * k as f32 * 0.4 - 52.0 * (1.0 - (2.0 * u - 1.0).powi(2)) + 28.0 * u);
        let (p, q) = (at(u), at(v));
        pen.streak(p, (p.0 - q.0, p.1 - q.1), 12.0, ramps::FIRE, 0.1);
        pen.light(p.0.round() as i32, p.1.round() as i32, '9');
    }
}

/// A tendril of smoke rising off a fire: puffs let go every few ticks,
/// swelling and thinning as they climb and sway.
fn smoke(im: &mut Img, base: (f32, f32), t: u32, seed: u32) {
    let mut pen = Pen::new(im, 260);
    for e in 0..7u32 {
        let age = ((t + e * 9) % 63) as f32;
        let k = age / 63.0;
        let sway = (age * 0.12 + e as f32 * 1.9 + seed as f32).sin() * 5.0 * k;
        let (x, y) = ((base.0 + sway).round() as i32, (base.1 - 4.0 - 38.0 * k).round() as i32);
        let r = 1.2 + k * 3.2;
        let ri = r.ceil() as i32;
        for dy in -ri..=ri {
            for dx in -ri..=ri {
                if (dx * dx + dy * dy) as f32 <= r * r && bayer(x + dx, y + dy) < (1.0 - k) * 0.65 {
                    pen.put(x + dx, y + dy, ramps::SMOKE.at(0.45 + k * 0.5));
                }
            }
        }
    }
}

/// A flame of the camp's: a little teardrop that wavers; `size` 1 small,
/// 2 a torch's.
pub(super) fn flame(im: &mut Img, x: f32, y: f32, t: u32, seed: u32, size: i32) {
    let (xi, yi) = (x.round() as i32, y.round() as i32);
    if !(-6..NATIVE_W + 6).contains(&xi) {
        return;
    }
    let beat = (hash((t / 2) as i32, seed as i32, 0x3a) % 3) as i32;
    let h = 3 * size + beat;
    for dy in 0..h {
        let k = dy as f32 / h as f32;
        let half = ((1.0 - k) * (size as f32 + 0.6) * if dy == 0 { 0.5 } else { 1.0 }).round() as i32;
        let sway = ((t as f32 * 0.3 + seed as f32).sin() * k * 1.4) as i32;
        for dx in -half..=half {
            let ch = if k < 0.25 { '9' } else if k < 0.5 { '5' } else if k < 0.75 { '@' } else { 'R' };
            let core = if dx.abs() * 2 > half * 2 { 'R' } else { ch };
            im.put(xi + dx + sway, yi - dy, core);
        }
    }
}

/// A pool of warm light on whatever is there: it brightens, never dims.
fn pool(im: &mut Img, (cx, cy): (i32, i32), r: i32, t: u32, seed: u32) {
    let flick = 0.85 + 0.15 * ((t as f32 * 0.37 + seed as f32).sin());
    let mut pen = Pen::new(im, 1400);
    let ramp: Ramp = ramps::EMBER;
    for y in cy - r..=cy + r {
        for x in cx - r..=cx + r {
            let d = (((x - cx) * (x - cx) + (y - cy) * (y - cy)) as f32).sqrt() / r as f32;
            if d >= 1.0 {
                continue;
            }
            let i = (1.0 - d).powf(1.5) * flick;
            if bayer(x, y) < i * 0.7 {
                pen.light(x, y, ramp.at(1.0 - i * 0.8));
            }
        }
    }
}

/// The moat behind the wall: water in dithered rows sliding at their own
/// pace, with the camp's fires lying along it in warm strokes.
fn moat(im: &mut Img, cam: i32, t: u32) {
    let sx = cam * WATER / 100;
    for r in 0..(VH - MOAT_ROW) {
        let y = VIEW_TOP + MOAT_ROW + r;
        let speed = 0.12 + r as f32 * 0.03;
        let off = (t as f32 * speed) as i32 + sx;
        for x in 0..NATIVE_W {
            let u = x + off;
            let wave = ((u as f32 * 0.21 + r as f32 * 1.7).sin() + (u as f32 * 0.09 - t as f32 * 0.02).sin()) * 0.5;
            let k = 0.35 + 0.35 * wave + r as f32 * 0.03;
            let ch = if k > 0.78 { 'u' } else if k > 0.55 { 'q' } else if k > 0.35 { '0' } else { 's' };
            let ch = if bayer(u, y) < 0.5 { ch } else if k > 0.5 { '0' } else { 'k' };
            im.put(x, y, ch);
        }
    }
    // Fire on the water: short warm dashes under each fire.
    for x in MID_FIRES.iter().map(|f| f.0).chain(FAR_FIRES) {
        let px = x - sx * 3 / 4;
        let mut pen = Pen::new(im, 90);
        for r in 0..(VH - MOAT_ROW - 3) {
            let wob = ((t as f32 * 0.15 + r as f32 * 1.3 + x as f32).sin() * 3.0) as i32;
            if (r + (t / 4) as i32) % 3 != 0 {
                let ch = if r < 3 { '@' } else { 'R' };
                for dx in -2..=2i32 {
                    if dx.abs() * 2 < 5 - r.min(4) {
                        pen.light(px + wob + dx, VIEW_TOP + MOAT_ROW + 2 + r, ch);
                    }
                }
            }
        }
    }
}

/// Embers that drift up off the camp and the engines, over everything
/// behind the actors.
pub(super) fn embers(im: &mut Img, cam: i32, t: u32) {
    let mut pen = Pen::new(im, 220);
    for (i, &(x, y)) in MID_FIRES.iter().enumerate() {
        let sx = (x - cam * MID / 100) as f32;
        if !(-20.0..NATIVE_W as f32 + 20.0).contains(&sx) {
            continue;
        }
        let spray = Spray { gravity: -0.012, drag: 0.03, stagger: 6.0, ..Spray::round(0x90 + i as u32, 6, (sx, (y + VIEW_TOP) as f32 - 4.0), (0.2, 0.9), (30.0, 60.0)) };
        spray.each((t % 360) as f32, |m| pfx::mote(&mut pen, &m, Shape::Ember, ramps::EMBER, 1, t as u64));
    }
}
