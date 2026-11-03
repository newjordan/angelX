//! The muster tent drawn (`rules::muster_tent`): a great striped pavilion on
//! the mine-head, and inside it a trodden floor strewn with straw, canvas
//! walls in bands of madder and undyed cloth, the ridge pole's two posts, a
//! bar of trestles and casks, and three rows of long tables with the recruits
//! on their benches. Dark as the tavern: lit by lanterns hung from the poles.

use super::super::ink::{Img, hash};
use super::super::kit;
use super::super::light::Light;
use super::{TILE, alehouse, at, dusk, fire, stand};
use crate::rules::muster_tent::{BAR, CASKS, FLAP, KEEPER_AT, OUTER_FLAP, PITCH, POLES, TABLES};
use crate::rules::{RoomKind, Run, Tile};

const T: i32 = TILE;
/// The canvas's two cloths, light and shade of each.
const CLOTH: [(char, char); 2] = [('c', '$'), ('7', '?')];

/// The tent inside at rest.
pub(super) fn scenery(run: &Run) -> Img {
    let room = run.room();
    let (pw, ph) = (room.cols as i32 * T, room.rows as i32 * T);
    let mut cv = Img::black(pw, ph);
    let mut lights: Vec<Light> = Vec::new();
    floor(&mut cv, room.cols as i32, room.rows as i32);
    walls(&mut cv, run);
    bar(&mut cv);
    casks(&mut cv);
    for (i, &rect) in TABLES.iter().enumerate() {
        alehouse::table(&mut cv, rect, i as i32 + 11, &mut lights);
    }
    for &(c, r) in &POLES {
        pole(&mut cv, c, r, &mut lights);
    }
    flap(&mut cv);
    // Lanterns at the bar and on the canvas by the flap.
    for (x, y) in [(BAR.0 * T - 6, BAR.1 * T - 2), ((BAR.0 + BAR.2) * T + 2, BAR.1 * T - 2), (FLAP.0 * T - 14, FLAP.1 * T - 8), ((FLAP.0 + FLAP.2) * T + 8, FLAP.1 * T - 8)] {
        let lamp = kit::lantern(true);
        cv.stamp(&lamp, x, y);
        lights.push(fire(x + 3, y + 3, 70.0, 0.5));
    }
    dusk(&mut cv, &lights, ('j', 'o'));
    cv
}

/// Trodden earth in broad worn patches, a little straw strewn over it.
fn floor(cv: &mut Img, cols: i32, rows: i32) {
    for y in 0..rows * T {
        for x in 0..cols * T {
            let n = hash(x / 10, y / 6, 41) % 10;
            cv.put(x, y, match n { 0 => 'n', 1 | 2 => 'I', _ => 'b' });
        }
    }
    for k in 0..(cols * rows / 2) {
        let x = (hash(k, 1, 77) % (cols * T) as u32) as i32;
        let y = (hash(k, 2, 77) % (rows * T) as u32) as i32;
        cv.line(x, y, x + 2, y + (k % 2) as i32, 'r');
    }
}

/// The canvas: the back wall in bands under its eaves, the side walls in
/// shade, the south wall with its flap.
fn walls(cv: &mut Img, run: &Run) {
    let room = run.room();
    for row in 0..room.rows as i32 {
        for col in 0..room.cols as i32 {
            if room.tile(col, row) != Tile::Wall {
                continue;
            }
            for y in row * T..(row + 1) * T {
                for x in col * T..(col + 1) * T {
                    let (light, shade) = CLOTH[((x / 12) % 2) as usize];
                    // The back wall catches the light; the sides and the
                    // south wall fall into shade, folds showing.
                    let back = row == 0;
                    let fold = (x % 12 == 0) || (y % 16 == 15);
                    cv.put(x, y, if fold { 'n' } else if back && (y % T) > 3 { light } else { shade });
                }
            }
        }
    }
    // The eaves: a scalloped valance along the back wall.
    for x in (0..room.cols as i32 * T).step_by(12) {
        let (light, _) = CLOTH[((x / 12) % 2) as usize];
        cv.rect(x + 1, T - 2, 10, 3, light);
        cv.line(x + 3, T + 1, x + 8, T + 1, light);
    }
}

/// Trestles and a plank, mugs along it, a cask tapped at its end.
fn bar(cv: &mut Img) {
    let (c, r, w, h) = BAR;
    let (x, y, bw, bh) = (c * T, r * T + 4, w * T, h * T - 6);
    for k in 0..4 {
        let lx = x + 6 + k * (bw - 12) / 3;
        cv.rect(lx, y + 6, 3, bh - 6, 'b');
        cv.line(lx - 2, y + bh - 1, lx + 4, y + bh - 1, 'n');
    }
    cv.rect(x, y, bw, 7, 'r');
    cv.line(x, y, x + bw - 1, y, 'O');
    cv.line(x, y + 6, x + bw - 1, y + 6, 'B');
    for k in 0..7 {
        let mx = x + 10 + k * 26;
        cv.rect(mx, y - 4, 4, 4, 'o');
        cv.put(mx + 1, y - 4, 't');
    }
}

/// Casks on their side, stacked by the bar, hoops dark.
fn casks(cv: &mut Img) {
    let (c, r, w, _) = CASKS;
    for k in 0..w {
        let (x, y) = ((c + k) * T + 1, r * T + 10);
        cv.ellipse((x + 7) as f32, (y + 8) as f32, 7.0, 8.0, 'B');
        cv.ellipse((x + 7) as f32, (y + 8) as f32, 4.0, 5.0, 'r');
        cv.line(x + 1, y + 3, x + 13, y + 3, 'n');
        cv.line(x + 1, y + 13, x + 13, y + 13, 'n');
        cv.put(x + 7, y + 8, 'O');
    }
}

/// A tent pole: a timber post from the floor up, a lantern hung on it.
fn pole(cv: &mut Img, c: i32, r: i32, lights: &mut Vec<Light>) {
    let (x, y) = (c * T + T / 2 - 2, r * T + T - 2);
    cv.ellipse((x + 2) as f32, y as f32, 6.0, 2.0, 'n');
    cv.rect(x, y - 44, 4, 44, 'B');
    cv.line(x, y - 44, x, y - 1, 'r');
    cv.line(x + 3, y - 44, x + 3, y - 1, 'b');
    let lamp = kit::lantern(true);
    cv.stamp(&lamp, x - 1, y - 36);
    lights.push(fire(x + 2, y - 33, 88.0, 0.55));
}

/// The flap in the south wall, tied back, the night beyond it.
fn flap(cv: &mut Img) {
    let (c, r, w, h) = FLAP;
    let (x, y, fw, fh) = (c * T, r * T, w * T, h * T);
    cv.rect(x, y, fw, fh, 'k');
    for (side, dir) in [(x - 4, 1), (x + fw - 3, -1)] {
        for k in 0..fh {
            cv.line(side, y + k, side + 6 * dir.max(0) + 2, y + k, if k % 4 == 0 { '8' } else { '7' });
        }
    }
    cv.line(x - 4, y + 6, x + fw + 3, y + 6, 'o');
}

/// The tent's living things: the keeper, and the recruits at their tables.
pub(super) fn figures(cv: &mut Img, run: &Run) {
    let tick = run.tick as u32;
    let (kx, ky) = at(KEEPER_AT.0 * 2.0, KEEPER_AT.1 * 2.0);
    stand(cv, &Img::from_rows(&alehouse::KEEPER), kx, ky);
    let swing = ((tick / 6) % 4) as i32;
    cv.rect(kx - 8 + swing, ky - 6, 4, 4, 'V');
    alehouse::patrons(cv, run, RoomKind::MusterTent, &TABLES);
}

/// The tent as the mine-head sees it: a striped pavilion with a pennant on
/// its ridge, the flap open on lantern light, a sign by it.
pub(super) fn pavilion(cv: &mut Img, lights: &mut Vec<Light>, tick: u32) {
    let (c, r, w, h) = PITCH;
    let (x0, y0, pw, ph) = (c * T, r * T, w * T, h * T);
    // It stands taller than the ground it covers, its roof up before the
    // spoil heaps behind it.
    let ridge = y0 - 26;
    let eave = y0 - 2;
    let base = y0 + ph;
    // The roof: a low gable of bands from the ridge out to the eaves.
    for y in ridge..eave {
        let k = (y - ridge) as f32 / (eave - ridge) as f32;
        let half = (pw as f32 * (0.18 + 0.36 * k)) as i32;
        let cx = x0 + pw / 2;
        for x in cx - half..cx + half {
            let band = (((x - cx) * 6) / pw.max(1)).rem_euclid(2) as usize;
            let (light, shade) = CLOTH[band];
            cv.put(x, y, if x < cx { light } else { shade });
        }
    }
    // The walls, banded, the scalloped valance where roof meets wall.
    for y in eave..base {
        for x in x0 + 4..x0 + pw - 4 {
            let band = (((x - x0) / 12) % 2) as usize;
            let (light, shade) = CLOTH[band];
            cv.put(x, y, if x < x0 + pw / 2 { light } else { shade });
        }
    }
    for x in (x0 + 4..x0 + pw - 4).step_by(8) {
        cv.rect(x, eave, 7, 3, 'n');
        cv.rect(x + 1, eave, 5, 2, '$');
    }
    // Guy ropes to pegs either side.
    for (ax, px) in [(x0 + 4, x0 - 10), (x0 + pw - 5, x0 + pw + 9)] {
        cv.line(ax, eave, px, base - 2, 'h');
        cv.rect(px - 1, base - 3, 3, 3, 'B');
    }
    // The ridge pole's head and its pennant, snapping in the wind.
    let cx = x0 + pw / 2;
    cv.line(cx, ridge - 14, cx, ridge, 'B');
    let snap = ((tick / 10) % 3) as i32;
    for k in 0..10 {
        cv.line(cx + 1, ridge - 14 + k / 3, cx + 12 - snap + (k % 3), ridge - 14 + k / 3 + snap / 2, if k % 2 == 0 { '7' } else { '8' });
    }
    // The flap: an opening in the canvas, dark, a little warm light low
    // in it, the cloth tied back either side.
    let (fc, _, fw, _) = OUTER_FLAP;
    let (fx, fwp) = (fc * T + 2, fw * T - 4);
    let mid = fx + fwp / 2;
    for y in eave + 2..base {
        let k = (y - eave - 2) as f32 / (base - eave - 2).max(1) as f32;
        let half = (fwp as f32 * (0.2 + 0.3 * k)) as i32;
        cv.line(mid - half, y, mid + half, y, if y > base - 5 { 'B' } else { 'k' });
        cv.put(mid - half - 1, y, 'p');
        cv.put(mid + half + 1, y, '$');
    }
    lights.push(fire(mid, base - 3, 40.0, 0.45));
    // The sign on its post, west of the tent.
    let word = "MUSTER";
    let tw = super::super::ink::text_width(word);
    let (sx, sy) = (x0 - tw - 6, base - 4);
    cv.line(sx + tw / 2 + 2, sy - 14, sx + tw / 2 + 2, sy + 2, 'B');
    cv.rect(sx - 1, sy - 22, tw + 6, 10, 'r');
    cv.frame(sx - 1, sy - 22, tw + 6, 10, 'b');
    cv.text(sx + 2, sy - 20, word, 'T');
}

#[cfg(test)]
#[path = "../../../../tests/render/arena__muster_tent__shots.rs"]
mod shots;
