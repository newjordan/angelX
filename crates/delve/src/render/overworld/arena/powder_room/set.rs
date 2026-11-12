//! The store's moving parts, in the room's pixels, under the knights: kegs
//! with their burning fuses (hot as they near the end), the red lanes a
//! lit keg will burn in its last second and a half, the soot a blast left
//! on the stones and the embers still cooling in it, lanterns that flicker,
//! the dust of a keg set down and the motes in the air. The still stone is
//! `walls`; the blasts themselves are `boom`.

use super::super::super::ink::{Img, bayer, hash};
use super::super::knights::seat_mark;
use super::super::pfx::{self, Pen, Shape, Spray, ramps};
use super::super::TILE;
use super::{bits, boom, walls};
use crate::rules::powder::{FUSE, Keg, Powder};
use crate::rules::powder_show::{self, DROP_LIFE};
use crate::rules::{Room, Run};

/// Ticks before a keg goes that the lanes it will burn begin to glow.
pub(super) const WARN: u32 = 54;

/// Whether room pixel `(x, y)` is near enough the view `view` to draw.
pub(super) fn near((x, y): (i32, i32), (vx, vy): (i32, i32), pad: i32) -> bool {
    x >= vx - pad && x < vx + 384 + pad && y >= vy - pad && y < vy + 224 + pad
}

/// The tile columns and rows a view covers, with a margin.
fn tiles(view: (i32, i32), room: &Room, pad: i32) -> (std::ops::Range<i32>, std::ops::Range<i32>) {
    let c0 = ((view.0 - pad) / TILE).max(0);
    let c1 = ((view.0 + 384 + pad) / TILE + 1).min(room.cols as i32);
    let r0 = ((view.1 - pad) / TILE).max(0);
    let r1 = ((view.1 + 224 + pad) / TILE + 1).min(room.rows as i32);
    (c0..c1, r0..r1)
}

/// What lies on the floor and under the knights: soot and its embers, the
/// lanes a keg will burn, the keg itself, a keg's landing, the lanterns'
/// pools of light and the blasts' own floor (`boom::under`).
pub(super) fn under(cv: &mut Img, run: &Run, p: &Powder, view: (i32, i32)) {
    let now = run.tick as u32;
    let room = run.room();
    scorch(cv, run, p, view, now);
    lanterns(cv, run, view);
    for k in &p.kegs {
        if near(boom::centre(k.at), view, 70) {
            lanes(cv, run, p, k, now);
        }
    }
    boom::under(cv, &p.show.blasts, now, view);
    for b in &p.show.blasts {
        if b.depth > 0 && near(boom::centre(b.at), view, 24) {
            boom::primed(cv, b, now);
        }
    }
    for k in &p.kegs {
        if near(boom::centre(k.at), view, 24) {
            keg(cv, run, p, k, now);
        }
    }
    for d in &p.show.drops {
        if near(boom::centre(d.at), view, 24) {
            landing(cv, run, d, now);
        }
    }
    let _ = room;
}

/// The soot a blast leaves, one of eight kept: the stone blackened in a
/// dither that thickens to the tile's middle, and specks of char.
fn soot_img(variant: usize) -> &'static Img {
    static SOOT: std::sync::OnceLock<Vec<Img>> = std::sync::OnceLock::new();
    &SOOT.get_or_init(|| {
        (0..8)
            .map(|v| {
                let mut im = Img::new(TILE, TILE);
                for py in 0..TILE {
                    for px in 0..TILE {
                        let (x, y) = (px + v * 16, py + v * 5);
                        let d = (((px - 8) * (px - 8) + (py - 8) * (py - 8)) as f32).sqrt() / 11.0;
                        let h = hash(x, y, 77);
                        let thr = (0.62 - d * 0.5).max(0.0);
                        if bayer(x, y) < thr && h % 6 != 0 {
                            im.put(px, py, if h % 5 == 0 { 'K' } else { 'k' });
                        } else if bayer(x + 1, y) < thr * 0.5 && h % 4 == 0 {
                            im.put(px, py, 'X');
                        }
                    }
                }
                im
            })
            .collect()
    })[variant % 8]
}

/// The soot a blast leaves on the stone, and while it is fresh a few
/// embers flickering in it.
fn scorch(cv: &mut Img, run: &Run, p: &Powder, view: (i32, i32), now: u32) {
    let cols = run.room().cols as i32;
    let (cs, rs) = tiles(view, run.room(), 0);
    for &i in &p.show.scorch {
        let (c, r) = (i32::from(i) % cols, i32::from(i) / cols);
        if !cs.contains(&c) || !rs.contains(&r) {
            continue;
        }
        let (x0, y0) = (c * TILE, r * TILE);
        cv.stamp(soot_img(hash(c, r, 3) as usize), x0, y0);
    }
    // Embers glow in the newest blasts' tiles while they cool.
    for b in p.show.blasts.iter().filter(|b| b.age(now) < 100 && b.age(now) >= u32::from(b.depth) * crate::rules::powder_show::LINK) {
        let fresh = 1.0 - b.age(now) as f32 / 100.0;
        for (c, r) in b.tiles() {
            if !cs.contains(&c) || !rs.contains(&r) {
                continue;
            }
            for j in 0..7 {
                let h = hash(c * 7 + j, r, 91);
                if (h >> 8) as f32 / 16777216.0 * 1.2 > fresh {
                    continue;
                }
                let (px, py) = ((h % 13) as i32 + 1, ((h >> 4) % 13) as i32 + 1);
                let flick = (now / 3 + h) % 3;
                if flick != 0 {
                    cv.put(c * TILE + px, r * TILE + py, if flick == 1 { '7' } else { 'R' });
                }
            }
        }
    }
}

/// The lanterns' flames and the pools they throw on the floor below.
fn lanterns(cv: &mut Img, run: &Run, view: (i32, i32)) {
    let room = run.room();
    let (cs, rs) = tiles(view, room, 24);
    let tick = run.tick as u32;
    for r in rs {
        for c in cs.clone() {
            if !walls::lantern(room, c, r) {
                continue;
            }
            let (x0, y0) = (c * TILE, r * TILE);
            let flick = (tick / 4 + hash(c, r, 3) % 5) % 2;
            cv.stamp(bits::flame(flick), x0 + 7, y0 + 7);
            let breath = 0.34 + 0.08 * ((tick / 5 + hash(c, r, 9) % 7) % 3) as f32;
            let mut pen = Pen::new(cv, 500);
            pen.glow(((x0 + 8) as f32, (y0 + TILE + 12) as f32), 40.0, 0.7, ramps::EMBER, breath);
        }
    }
}

/// The lanes a lit keg will burn, glowing red on the stone as its last
/// second and a half runs out: the same walk the rules take, so what
/// glows is what burns. The crate at the end of each lane is bracketed.
fn lanes(cv: &mut Img, run: &Run, p: &Powder, k: &Keg, now: u32) {
    if k.fuse > WARN {
        return;
    }
    let room = run.room();
    let (arms, hit) = powder_show::cross(room, &p.crates, k.at);
    let heat = 1.0 - k.fuse as f32 / WARN as f32;
    let tick = run.tick as u32;
    let last = k.fuse < 14;
    let mut lit = vec![k.at];
    for d in 0..4 {
        let (dx, dy) = [(1, 0), (-1, 0), (0, 1), (0, -1)][d];
        lit.extend((1..=i32::from(arms[d])).map(|i| (k.at.0 + dx * i, k.at.1 + dy * i)));
    }
    for (tx, ty) in lit {
        if hit.contains(&(tx, ty)) {
            continue;
        }
        let (x0, y0) = (tx * TILE, ty * TILE);
        for py in 1..TILE - 1 {
            for px in 1..TILE - 1 {
                let (x, y) = (x0 + px, y0 + py);
                let stripe = (x + y + (tick as i32 / 2)) % 6 < 2;
                let thr = 0.1 + 0.5 * heat;
                if bayer(x, y) < thr && (stripe || last) {
                    cv.put(x, y, if last && tick / 2 % 2 == 0 { '@' } else { '7' });
                }
            }
        }
    }
    // The crates at the lanes' ends: four corner brackets, blinking.
    for &(tx, ty) in &hit {
        if heat > 0.35 && tick / 3 % 2 == 0 {
            let (x0, y0) = (tx * TILE, ty * TILE);
            for (bx, by, sx, sy) in [(0, 0, 1, 1), (15, 0, -1, 1), (0, 15, 1, -1), (15, 15, -1, -1)] {
                for i in 0..4 {
                    cv.put(x0 + bx + sx * i, y0 + by, '@');
                    cv.put(x0 + bx, y0 + by + sy * i, '@');
                }
            }
        }
    }
    let _ = now;
}

/// A keg standing: its shadow, the barrel (warming as the fuse shortens),
/// its owner's colour on the lid, the fuse burning down with its spark, a
/// small light of its own. A keg just set down drops in and lands.
fn keg(cv: &mut Img, run: &Run, p: &Powder, k: &Keg, now: u32) {
    let (x0, y0) = (k.at.0 * TILE, k.at.1 * TILE);
    let tick = run.tick as u32;
    let since = p.show.drops.iter().find(|d| d.at == k.at).map_or(DROP_LIFE, |d| now.saturating_sub(d.born));
    let fall = if since < 5 { (5 - since as i32) * 3 } else { 0 };
    let bounce = if (5..9).contains(&since) { [2, 3, 2, 1][(since - 5) as usize] } else { 0 };
    let heat = match k.fuse {
        0..=5 => 3,
        6..=17 if tick / 2 % 2 == 0 => 3,
        6..=35 => 2,
        36..=63 if tick / 4 % 2 == 0 => 1,
        _ => 0,
    };
    let tremble = if k.fuse < 26 { [0, 1, 0, -1][(tick / 2 % 4) as usize] } else { 0 };
    // Its shadow on the stone, small as it lands.
    let wide = if fall > 0 { 3 } else { 6 };
    for dx in -wide..=wide {
        for dy in 0..3 {
            if dx * dx + dy * dy * 9 <= wide * wide + 2 && bayer(x0 + 8 + dx, y0 + 13 + dy) < 0.75 {
                cv.put(x0 + 8 + dx, y0 + 13 + dy, 'k');
            }
        }
    }
    let (kx, ky) = (x0 + 2 + tremble, y0 + 2 - fall - bounce + (if since < 5 { 0 } else { 0 }));
    cv.stamp(bits::keg(heat), kx, ky);
    // The owner's colour on the lid.
    let mark = seat_mark(k.owner);
    cv.rect(kx + 4, ky + 2, 4, 1, mark);
    // The fuse: a cord rising from the lid, shorter as it burns.
    const CORD: [(i32, i32); 8] = [(8, 1), (8, 0), (9, -1), (9, -2), (10, -3), (10, -4), (11, -4), (11, -5)];
    let len = (1 + (k.fuse as usize * 7) / FUSE as usize).min(8);
    for &(cx, cy) in &CORD[..len] {
        cv.put(kx + cx - 2, ky + cy, if cy % 2 == 0 { '$' } else { 'c' });
    }
    let (sx, sy) = CORD[len - 1];
    let (sx, sy) = (kx + sx - 2, ky + sy - 1);
    let flick = (tick + k.at.0 as u32 * 3) % 3;
    cv.put(sx, sy, ['9', 'w', '6'][flick as usize]);
    cv.put(sx - 1, sy - 1 + i32::from(flick == 1), '@');
    cv.put(sx + 1, sy - 1, if flick == 2 { '6' } else { '4' });
    let mut pen = Pen::new(cv, 120);
    let spit = Spray { gravity: 0.08, ..Spray::round(k.at.0 as u32 * 7 + k.at.1 as u32 + tick / 3, 2, (sx as f32, sy as f32), (0.5, 1.6), (4.0, 9.0)) };
    spit.each((tick % 3) as f32 * 1.5, |m| pfx::mote(&mut pen, &m, Shape::Spark, ramps::GOLD, 1, run.tick));
    if k.fuse < 64 {
        let power = 0.28 + 0.4 * (1.0 - k.fuse as f32 / 64.0);
        pen.glow(((x0 + 8) as f32, (y0 + 9) as f32), 18.0 + (64 - k.fuse.min(64)) as f32 * 0.25, 0.8, ramps::EMBER, power);
    }
}

/// A keg set down: a small ring of dust leaves its foot, and grit.
fn landing(cv: &mut Img, run: &Run, d: &powder_show::Drop, now: u32) {
    let age = now.saturating_sub(d.born);
    let (cx, cy) = boom::centre(d.at);
    if !(4..DROP_LIFE).contains(&age) {
        return;
    }
    let a = (age - 4) as f32;
    let mut pen = Pen::new(cv, 160);
    pen.ring((cx as f32, cy as f32 + 5.0), 3.0 + a * 1.1, 1.0, 0.55, ramps::SMOKE, a / 12.0);
    let grit = Spray { gravity: 0.05, ..Spray::round(d.born ^ d.at.0 as u32, 5, (cx as f32, cy as f32 + 5.0), (0.4, 1.3), (6.0, 12.0)) };
    grit.each(a, |m| pfx::mote(&mut pen, &m, Shape::Dot, ramps::SMOKE, 1, run.tick));
}

/// Whether `tile` lies in the lanes keg `k` will burn (its own tile too).
pub(super) fn in_lane(run: &Run, p: &Powder, k: &Keg, tile: (i32, i32)) -> bool {
    let (arms, _) = powder_show::cross(run.room(), &p.crates, k.at);
    tile == k.at
        || (0..4).any(|d| {
            let (dx, dy) = [(1, 0), (-1, 0), (0, 1), (0, -1)][d];
            (1..=i32::from(arms[d])).any(|i| (k.at.0 + dx * i, k.at.1 + dy * i) == tile)
        })
}
