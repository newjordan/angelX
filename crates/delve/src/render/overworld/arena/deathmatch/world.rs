//! The arena's own life: braziers that flicker, throw embers and light the
//! stone (the pit paints that light; see `pit`), pickups that bob and twinkle
//! and pop when taken.
use super::super::pfx::{Pen, ramps};
use super::super::race3d;
use crate::render::overworld::ink::{Img, bayer, hash};
use crate::render::world3d::{math::v3, raster::{self, View3}};
use crate::rules::deathmatch as dm;

/// Where the braziers stand: open flags beside the pillars.
pub(super) const BRAZIERS: [(f32, f32); 4] = [(7., 13.), (41., 13.), (7., 23.), (41., 23.)];

const FLAMES: [[&str; 7]; 3] = [
    ["....6....", "...656...", "..65556..", "..@555@..", "..@@5@@..", "...@@@...", "....@...."],
    [".....6...", "...6656..", "..65556..", "..@555@..", "..@@5@@..", "...@@@...", "....@...."],
    ["...6.....", "...656...", "..65556..", "..@555@..", "..@@5@@..", "...@@@...", "....@...."],
];
const BOWL: [&str; 7] = [".hhhhhhh.", ".JJJJJJJ.", "..jjjjj..", "....j....", "....j....", "...jjj...", "..jjjjj.."];

fn brazier(ticks: u32, k: usize) -> Img {
    let mut rows: Vec<&str> = FLAMES[((ticks / 3) as usize + k) % 3].to_vec();
    rows.extend_from_slice(&BOWL);
    Img::from_rows(&rows)
}

/// The braziers: a halo dithered round each flame, embers climbing off it,
/// then the brazier itself, standing in the stone's depth like any figure.
pub(super) fn braziers(small: &mut Img, depth: &[f32], view: &View3, ticks: u32) {
    let (w, h) = (small.w, small.h);
    for (k, &(x, y)) in BRAZIERS.iter().enumerate() {
        let Some((fx, fy, fd)) = raster::project_point(view, w as usize, h as usize, v3(x, y, 0.)) else { continue };
        let tall = raster::dots_tall(view, w as usize, h as usize, 1.0, fd);
        let flick = 0.8 + 0.2 * ((hash(k as i32, (ticks / 2) as i32, 7) & 0xff) as f32 / 255.0);
        // The halo sits on the flame's heart, and only over what is behind it.
        let (cx, cy) = (fx, fy - tall * 1.25);
        let reach = tall * 1.15 * flick;
        for py in (cy - reach) as i32..=(cy + reach) as i32 {
            for px in (cx - reach) as i32..=(cx + reach) as i32 {
                if px < 0 || py < 0 || px >= w || py >= h {
                    continue;
                }
                let d = ((px as f32 - cx).powi(2) + (py as f32 - cy).powi(2)).sqrt() / reach.max(1.0);
                if d >= 1.0 || depth[(py * w + px) as usize] < fd - 0.3 {
                    continue;
                }
                if bayer(px, py) < (1.0 - d).powi(2) * 0.55 {
                    if let Some(c) = small.get(px, py) {
                        small.set(px, py, [c[0].saturating_add(46), c[1].saturating_add(24), c[2].saturating_add(6)]);
                    }
                }
            }
        }
        // Embers: a handful of motes climbing off the bowl, each on its own clock.
        for n in 0..5u32 {
            let life = 44 + (hash(k as i32, n as i32, 0xe1) & 15);
            let age = (ticks + n * 13 + k as u32 * 7) % life;
            let seed = hash(k as i32, (ticks + n * 13 + k as u32 * 7) as i32 / life as i32, 0xe2 + n);
            let (jx, jy) = (((seed & 0xff) as f32 / 255.0 - 0.5) * 0.5, (((seed >> 8) & 0xff) as f32 / 255.0 - 0.5) * 0.5);
            let rise = age as f32 / life as f32;
            let p = v3(x + jx + 0.25 * (rise * 6.0 + n as f32).sin() * rise, y + jy, 1.1 + rise * 2.4);
            let Some((ex, ey, ed)) = raster::project_point(view, w as usize, h as usize, p) else { continue };
            let (ex, ey) = (ex as i32, ey as i32);
            if ex < 0 || ey < 0 || ex >= w || ey >= h || depth[(ey * w + ex) as usize] < ed - 0.05 {
                continue;
            }
            small.put(ex, ey, if rise < 0.45 { '6' } else if rise < 0.8 { 'o' } else { 'J' });
        }
        if fd > 2.6 {
            race3d::stand(small, depth, view, &brazier(ticks, k), v3(x, y, 0.), v3(x, y, 0.5), 1.1, w, h);
        }
    }
}

/// Where a pickup hangs: it bobs a hand's breadth and turns on its place.
pub(super) fn bob(ticks: u32, k: usize) -> f32 {
    0.12 + 0.07 * ((ticks as f32 * 0.14) + k as f32 * 1.7).sin()
}

/// A pickup's sparkles: a twinkle that comes and goes beside it, drawn on
/// the grey picture at its projected `(px, py)`.
pub(super) fn sparkle(small: &mut Img, px: f32, py: f32, ticks: u32, k: usize) {
    let mut pen = Pen::new(small, 60);
    for n in 0..2u32 {
        let t = (ticks + n * 17 + k as u32 * 5) % 34;
        let a = (hash(k as i32, (ticks / 34 + n) as i32, 3) & 0xff) as f32 / 255.0 * std::f32::consts::TAU;
        pen.twinkle((px + a.cos() * 6.0, py - 4.0 + a.sin() * 5.0), 2.0, t as f32 / 34.0, ramps::GOLD);
    }
}

/// How long ago pickup `k` was taken, if it was just now: a pop for the
/// first dozen ticks, and a glint in the last few before it is back.
pub(super) fn taken(s: &dm::Session, k: usize) -> Option<u32> {
    let wait = s.pickups[k];
    (wait > dm::PICKUP_REST.saturating_sub(14)).then(|| dm::PICKUP_REST - wait)
}
