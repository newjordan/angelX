//! The Roostfolk's cavern: a roost-city of woven nests. The nests along
//! the back hang from the roof of the dark on long ropes, those lower down
//! sit up on crooked poles like storks' nests, and rope bridges sag between
//! them. Red lanterns hang everywhere, the light the bats steer by; the
//! bats themselves roost in clumps under the stalactites and wheel about
//! the cavern, a rider on the bigger ones once the village is bright.

use super::super::super::ink::{Img, hash, vnoise};
use super::super::super::light::Light;
use super::super::super::material as mat;
use super::super::fire;
use super::{Look, all, px_rect};
use crate::rules::{
    Run,
    deep::{Village, village as v},
};

/// The woven ramp, dark to light: reeds and withies.
const WEAVE: [char; 7] = ['n', 'b', 'I', 'B', 'p', 'P', 'r'];

/// The floor: bare rock, twigs and feathers dropped from the nests.
pub(super) fn ground_marks(cv: &mut Img, k: i32, (x, y): (i32, i32)) {
    match k % 6 {
        0 => {
            // A twig.
            let lean = (hash(k, 1, 9601) % 5) as i32 - 2;
            cv.line(x, y, x + 4, y + lean / 2, 'I');
            cv.put(x + 2, y - 1, 'b');
        }
        1 => {
            // A dropped feather, pale on the rock.
            cv.line(x, y, x + 3, y - 1, 'h');
            cv.put(x + 1, y, 'J');
        }
        _ => cv.put(x, y, if k % 3 == 0 { 'g' } else { 'X' }),
    }
}

/// The roof of the dark: bats roosting in clumps under the stalactites,
/// red eyes here and there, and far nests hanging on their long ropes.
pub(super) fn back(cv: &mut Img, vl: &Village, lights: &mut Vec<Light>) {
    // Far nests, small and high, a rope each up into the dark.
    for k in 0..7 {
        let h = hash(k, 3, 9610);
        let x = 30 + (h % 516) as i32;
        let y = 20 + ((h >> 10) % 40) as i32;
        cv.line(x, 0, x, y - 8, 'b');
        far_nest(cv, x, y, 5 + (h >> 20) as i32 % 4, (k as u8) < vl.hope, lights);
    }
    // Clumps of roosting bats.
    for k in 0..9 {
        let h = hash(k, 4, 9611);
        let x = 20 + (h % 536) as i32;
        let y = 6 + ((h >> 10) % 30) as i32;
        let n = 3 + (h >> 18) as i32 % 4;
        for b in 0..n {
            let bx = x + (b - n / 2) * 4 + (hash(k, b, 9612) % 3) as i32 - 1;
            let by = y + (hash(b, k, 9613) % 4) as i32;
            hanging_bat(cv, bx, by, hash(bx, by, 9614) % 5 == 0);
        }
    }
}

const TILE_Y: i32 = super::super::TILE;

/// A small far nest at `(x, y)`, its door lit or dark.
fn far_nest(cv: &mut Img, x: i32, y: i32, r: i32, lit: bool, lights: &mut Vec<Light>) {
    for yy in -r - 2..=r {
        let half = if yy < -r / 2 { r * (yy + r + 2) / (r / 2 + 2).max(1) } else { r - (yy + r / 2).max(0) / 3 };
        for xx in -half..=half {
            let ink = if (xx + yy).rem_euclid(3) == 0 { 'n' } else if xx < 0 { 'I' } else { 'b' };
            cv.put(x + xx, y + yy, ink);
        }
    }
    cv.put(x, y + 1, if lit { '7' } else { 'k' });
    cv.put(x + 1, y + 1, if lit { '4' } else { 'k' });
    if lit {
        lights.push(Light { x: x as f32, y: (y + 1) as f32, r: 16.0, s: 0.25, fire: true });
    }
}

/// A bat hanging upside down: folded wings, ears down, now and then eyes.
fn hanging_bat(cv: &mut Img, x: i32, y: i32, eyes: bool) {
    cv.put(x, y, 'g');
    cv.rect(x - 1, y + 1, 3, 4, 'K');
    cv.line(x - 2, y + 1, x - 2, y + 4, 'g');
    cv.line(x + 2, y + 1, x + 2, y + 4, 'X');
    cv.put(x - 1, y + 5, 'K');
    cv.put(x + 1, y + 5, 'K');
    if eyes {
        cv.put(x - 1, y + 4, '7');
        cv.put(x + 1, y + 4, '7');
    }
}

/// A nest house on its footprint: a woven gourd, hung from the dark on
/// ropes along the back of the cavern, up on crooked poles lower down; a
/// round door with a rope ladder, round windows lit red.
pub(super) fn house(cv: &mut Img, (x, y, w, h): (i32, i32, i32, i32), seed: u32, lit: bool, lights: &mut Vec<Light>) {
    let cx = x + w / 2;
    let hung = y <= v::BACK as i32 * TILE_Y + 4;
    let rx = (w / 2 - 3) as f32;
    let (top, bottom) = if hung { (y - 48, y + h - 16) } else { (y - 34, y + h - 22) };
    let foot = y + h;
    // Its shadow on the floor under it, wherever it hangs.
    mat::shadow(cv, ((cx + 6) as f32, (foot + 2) as f32), (rx * 0.75, 3.5), &all);
    if hung {
        // Ropes up into the dark from its neck, splayed to the roof.
        for (k, dx) in [-(rx as i32) / 2 - 4, -2, rx as i32 / 2 + 4].into_iter().enumerate() {
            cv.line(cx + dx, 0, cx + dx / 6, top + 1, if k == 1 { 'r' } else { 'b' });
        }
    } else {
        // Crooked poles, the nest up on them.
        for (k, dx) in [-(rx as i32) * 2 / 3, 1, rx as i32 * 2 / 3].into_iter().enumerate() {
            let pole_foot = foot + 1 - k as i32 % 2;
            let lean = (hash(k as i32, seed as i32, 9620) % 5) as i32 - 2;
            cv.line(cx + dx + lean, pole_foot, cx + dx / 2, bottom - 6, 'b');
            cv.line(cx + dx + lean + 1, pole_foot, cx + dx / 2 + 1, bottom - 6, 'P');
            cv.put(cx + dx + lean, pole_foot + 1, 'n');
        }
        // A cross-brace between them.
        cv.line(cx - rx as i32 * 2 / 3, foot - 8, cx + rx as i32 * 2 / 3, foot - 10, 'b');
    }
    gourd(cv, cx, (top, bottom), rx, seed);
    // The binding at its neck, where the weave gathers.
    cv.rect(cx - 4, top - 2, 9, 5, 'b');
    cv.line(cx - 4, top - 2, cx + 4, top - 2, 'r');
    cv.line(cx - 4, top + 1, cx + 4, top + 1, 'n');
    cv.put(cx, top - 3, 'P');
    // The round door, low on its belly, with a rope ladder to the floor.
    let (dx, dy) = (cx - 2 + (seed % 3) as i32 * 2, bottom - (bottom - top) / 4);
    cv.ellipse(dx as f32, dy as f32, 7.0, 6.5, 'n');
    cv.ellipse(dx as f32, (dy + 1) as f32, 5.5, 5.0, if lit { '8' } else { 'k' });
    if lit {
        cv.ellipse(dx as f32, (dy + 2) as f32, 3.5, 3.0, '7');
        cv.rect(dx - 1, dy, 3, 3, '4');
        cv.put(dx, dy + 1, '@');
        lights.push(fire(dx, dy + 1, 54.0, 0.44));
    }
    for side in [-4, 4] {
        cv.line(dx + side, dy + 6, dx + side + side / 4, foot, 'r');
    }
    for ly in (dy + 9..foot).step_by(4) {
        cv.line(dx - 4, ly, dx + 4, ly, 'P');
    }
    // Round windows either side, red with the lanterns inside.
    let wy = (top + bottom) / 2 - 2;
    for side in [-1, 1] {
        let wx = cx + side * (rx as i32 / 2 + 2);
        cv.ellipse(wx as f32, wy as f32, 3.5, 3.5, 'n');
        cv.ellipse(wx as f32, wy as f32, 2.5, 2.5, if lit { '7' } else { 'k' });
        if lit {
            cv.put(wx, wy, '4');
            lights.push(Light { x: wx as f32, y: wy as f32, r: 30.0, s: 0.3, fire: true });
        }
    }
    // A lantern hung off its side on a crook.
    let (lx, ly) = (cx + rx as i32 - 3, wy + 6);
    cv.line(lx, ly, lx + 7, ly - 3, 'b');
    lantern(cv, lx + 7, ly - 2, lit, lights);
    // Bats asleep under its belly.
    for b in 0..(seed % 3 + 1) as i32 {
        hanging_bat(cv, cx - 12 + b * 9, bottom - 1, (seed + b as u32).is_multiple_of(3));
    }
}

/// A woven gourd from `top` to `bottom`, `rx` wide each side at its belly:
/// gathered tight at its neck, swelling low, round underneath; the weave
/// crossing in diagonals, lit from the upper left, darker underneath.
fn gourd(cv: &mut Img, cx: i32, (top, bottom): (i32, i32), rx: f32, seed: u32) {
    let tall = (bottom - top).max(1) as f32;
    for y in top..=bottom {
        let t = (y - top) as f32 / tall;
        // A teardrop: the neck, then a belly broadest two thirds down.
        let belly = (std::f32::consts::PI * t).sin().max(0.0).powf(0.6) * (0.45 + 0.55 * t);
        let width = belly.max(if t < 0.14 { 0.13 } else { 0.0 });
        let half = (rx * width) as i32;
        for x in cx - half..=cx + half {
            let u = (x - cx) as f32 / half.max(1) as f32;
            // Round: lit upper left, falling away to the rim and the base.
            let z = (1.0 - u * u).max(0.0).sqrt();
            let light = (z * 0.7 - u * 0.45 + (0.5 - t) * 0.5).clamp(0.0, 1.0);
            let mut i = (light * 5.5) as i32 + 1;
            // The weave: two sets of diagonals, the over-strand lit.
            let a = (x + y * 2).rem_euclid(7);
            let b = (x - y * 2).rem_euclid(7);
            if a == 0 || b == 0 {
                i -= 2;
            } else if a == 1 || b == 3 {
                i += 1;
            }
            // Bands of rope round it, every so often.
            if (y - top) % 13 == 6 {
                i = (i - 1).max(1);
            }
            if hash(x / 3, y / 2, 9621 + seed) % 15 == 0 {
                i -= 1;
            }
            if u.abs() > 0.88 {
                i -= 2;
            }
            cv.put(x, y, WEAVE[i.clamp(0, WEAVE.len() as i32 - 1) as usize]);
        }
    }
    // Loose withies poking out round its belly and under it.
    for k in 0..14 {
        let a = std::f32::consts::PI * (-0.1 + k as f32 * 0.09);
        let t = 0.55 + a.sin() * 0.4;
        let half = rx * (std::f32::consts::PI * t).sin().max(0.0).powf(0.6) * (0.45 + 0.55 * t);
        let (ex, ey) = (cx as f32 + a.cos() * half, top as f32 + t * tall);
        let out = 2 + (hash(k, seed as i32, 9622) % 3) as i32;
        cv.line(ex as i32, ey as i32, ex as i32 + (a.cos() * out as f32) as i32, ey as i32 + out / 2, 'b');
    }
}

/// A red paper lantern hanging at `(x, y)` (its hook): ribbed paper, a
/// cap and a tassel, lit from inside.
pub(super) fn lantern(cv: &mut Img, x: i32, y: i32, lit: bool, lights: &mut Vec<Light>) {
    cv.line(x, y, x, y + 2, 'b');
    cv.rect(x - 2, y + 2, 5, 1, 'K');
    cv.ellipse(x as f32, (y + 6) as f32, 3.5, 3.5, if lit { '7' } else { 'p' });
    cv.line(x - 2, y + 6, x + 2, y + 6, if lit { '8' } else { 'B' });
    if lit {
        cv.put(x, y + 5, '4');
        cv.put(x, y + 6, '@');
        lights.push(fire(x, y + 6, 34.0, 0.38));
    }
    cv.rect(x - 1, y + 10, 3, 1, 'K');
    cv.put(x, y + 11, 'R');
    cv.put(x, y + 12, 'p');
}

/// The rope bridges between the back nests, sagging, slats on two ropes,
/// from one nest's flank to the next's.
pub(super) fn bridges(cv: &mut Img) {
    let side = |i: usize, right: bool| {
        let (x, y, w, h) = px_rect(v::HOUSES[i]);
        (if right { x + w - 8 } else { x + 8 }, y + h - 26)
    };
    let (hx, hy, hw, hh) = px_rect(v::HALL);
    let hall_l = (hx + 8, hy + hh - 26);
    let hall_r = (hx + hw - 8, hy + hh - 26);
    for (a, b) in [(side(0, true), side(1, false)), (side(1, true), hall_l), (hall_r, side(2, false)), (side(2, true), side(3, false))] {
        bridge(cv, a, b);
    }
}

fn bridge(cv: &mut Img, (x0, y0): (i32, i32), (x1, y1): (i32, i32)) {
    let span = (x1 - x0).abs().max(1);
    let sag = (span as f32 * 0.22).min(16.0);
    let at = |t: f32| (x0 as f32 + (x1 - x0) as f32 * t, y0 as f32 + (y1 - y0) as f32 * t + sag * 4.0 * t * (1.0 - t));
    for s in 0..=span {
        let t = s as f32 / span as f32;
        let (x, y) = at(t);
        let (x, y) = (x as i32, y as i32);
        // A hand-rope over, the foot-rope under, slats laid across it.
        cv.put(x, y - 7 + (sag * 0.3 * (4.0 * t * (1.0 - t))) as i32, 'r');
        cv.put(x, y + 1, 'n');
        if s % 3 == 0 {
            cv.line(x, y - 1, x, y, 'P');
        } else {
            cv.put(x, y, 'B');
        }
        if s % 8 == 0 {
            cv.line(x, y - 7 + (sag * 0.3 * (4.0 * t * (1.0 - t))) as i32, x, y - 1, 'b');
        }
    }
}

/// The heart of the roost: the Great Perch, a stout mast braced and
/// barred where the bats come in to land, a big lantern at each bar's
/// end, and the first nest at its top with the oldest bat on its lip.
pub(super) fn heart(cv: &mut Img, vl: &Village, lights: &mut Vec<Light>, tick: u32) {
    let (x, y, w, h) = px_rect(v::HEART);
    let (cx, foot) = (x + w / 2, y + h);
    let awake = vl.hope >= 1;
    let light = vl.light();
    mat::shadow(cv, ((cx + 10) as f32, (foot + 2) as f32), (34.0, 6.0), &all);
    // Its foot: a ring of stones round the mast.
    for k in 0..11 {
        let a = std::f32::consts::PI * (0.02 + k as f32 * 0.96 / 10.0);
        let (sx, sy) = (cx + (a.cos() * 20.0) as i32, foot - 2 + (a.sin() * 4.0) as i32);
        cv.ellipse(sx as f32, sy as f32, 3.5, 2.5, 'j');
        cv.put(sx - 1, sy - 1, 'G');
        cv.put(sx + 1, sy + 1, 'X');
    }
    // The mast, stout and a little crooked, its grain lit on the left.
    let top = foot - 74;
    let bend = |yy: i32| (((yy - top) as f32 / 16.0).sin() * 1.5) as i32;
    for yy in top..foot {
        let half = 4 + (foot - yy) / 20;
        for xx in -half..=half {
            let ink = match xx {
                d if d == -half => 'n',
                d if d == -half + 1 => 'P',
                d if d < 0 => 'B',
                d if d == half => 'n',
                _ => 'b',
            };
            cv.put(cx + xx + bend(yy), yy, ink);
        }
        if (yy - top) % 9 == 4 {
            cv.put(cx - 1 + bend(yy), yy, 'r');
        }
    }
    // The crossbars, braced, a lantern hung from each end and bats beneath.
    for (k, (bar, reach)) in [(foot - 60, 30), (foot - 40, 40), (foot - 22, 24)].into_iter().enumerate() {
        let bx = cx + bend(bar);
        for side in [-1, 1] {
            cv.line(bx + side * 4, bar + 10, bx + side * reach / 2, bar + 1, 'b');
        }
        cv.rect(bx - reach, bar - 1, reach * 2 + 1, 3, 'b');
        cv.line(bx - reach, bar - 1, bx + reach, bar - 1, 'P');
        cv.line(bx - reach, bar + 1, bx + reach, bar + 1, 'n');
        for side in [-1, 1] {
            let ex = bx + side * reach;
            cv.line(ex, bar + 1, ex, bar + 4, 'b');
            big_lantern(cv, ex, bar + 4, vl.hope as usize > k, lights);
        }
        for b in 0..3 {
            let px_ = bx - reach + 12 + b * (reach * 2 - 24) / 2;
            hanging_bat(cv, px_, bar + 2, awake && (b + k as i32) % 2 == 0);
        }
    }
    // The first nest, up at the top of the mast.
    gourd(cv, cx, (top - 24, top + 4), 18.0, 77);
    cv.ellipse(cx as f32, (top - 4) as f32, 4.5, 4.0, 'n');
    cv.ellipse(cx as f32, (top - 3) as f32, 3.0, 2.5, if awake { '7' } else { 'k' });
    if awake {
        cv.put(cx, top - 3, '4');
        // A great bat on the nest's lip, wings half open, keeping watch.
        let flap = ((tick / 8) % 2) as i32;
        let (ux, uy) = (cx, top - 28);
        for side in [-1, 1] {
            cv.line(ux + side * 3, uy, ux + side * (12 + flap), uy - 5 + flap, 'K');
            cv.line(ux + side * 3, uy + 1, ux + side * (11 + flap), uy - 2, 'X');
            cv.line(ux + side * (12 + flap), uy - 5 + flap, ux + side * 8, uy + 2, 'K');
        }
        cv.rect(ux - 2, uy - 2, 5, 5, 'K');
        cv.put(ux - 2, uy - 3, 'K');
        cv.put(ux + 2, uy - 3, 'K');
        cv.put(ux - 1, uy - 1, '7');
        cv.put(ux + 1, uy - 1, '7');
        lights.push(fire(cx, top - 10, 90.0 + light * 60.0, 0.4 + light * 0.3));
    }
}

/// A big red lantern hanging from `(x, y)`.
fn big_lantern(cv: &mut Img, x: i32, y: i32, lit: bool, lights: &mut Vec<Light>) {
    cv.rect(x - 3, y, 7, 2, 'K');
    cv.ellipse(x as f32, (y + 8) as f32, 5.5, 6.5, if lit { '7' } else { 'p' });
    for r in [y + 5, y + 8, y + 11] {
        cv.line(x - 4, r, x + 4, r, if lit { '8' } else { 'B' });
    }
    if lit {
        cv.rect(x - 1, y + 5, 3, 6, '4');
        cv.put(x, y + 7, '6');
        lights.push(fire(x, y + 8, 46.0, 0.42));
    }
    cv.rect(x - 2, y + 14, 5, 2, 'K');
    cv.line(x, y + 16, x, y + 19, 'R');
}

/// The wishing perch: a post and its one crossbar, where old Dusk hangs
/// and takes a visitor's wish up to the stars.
pub(super) fn wishing_place(cv: &mut Img, vl: &Village, lights: &mut Vec<Light>, tick: u32) {
    let (x, y, w, h) = px_rect(v::WELL);
    let (cx, foot) = (x + w / 2, y + h);
    let glow = vl.hope >= 2;
    mat::shadow(cv, ((cx + 4) as f32, (foot + 1) as f32), (14.0, 3.0), &all);
    cv.rect(cx - 6, foot - 4, 13, 4, 'g');
    cv.line(cx - 6, foot - 4, cx + 6, foot - 4, 'G');
    cv.rect(cx - 1, foot - 34, 3, 30, 'b');
    cv.line(cx - 1, foot - 34, cx - 1, foot - 5, 'P');
    cv.line(cx - 12, foot - 33, cx + 12, foot - 34, 'B');
    cv.line(cx - 12, foot - 32, cx + 12, foot - 33, 'b');
    // Old Dusk: grey about the ears, wrapped in his wings.
    let (bx, by) = (cx + 6, foot - 32);
    cv.put(bx, by, 'J');
    cv.rect(bx - 2, by + 1, 5, 7, 'g');
    cv.line(bx - 3, by + 2, bx - 3, by + 6, 'G');
    cv.line(bx + 3, by + 2, bx + 3, by + 6, 'j');
    cv.put(bx - 2, by + 8, 'h');
    cv.put(bx + 2, by + 8, 'h');
    let blink = (tick / 50) % 7 == 0;
    let eye = if glow && !blink { '7' } else { 'K' };
    cv.put(bx - 1, by + 6, eye);
    cv.put(bx + 1, by + 6, eye);
    // A string of feathers and a little bell off the bar's other end.
    let (sx, sy) = (cx - 10, foot - 33);
    cv.line(sx, sy, sx, sy + 8, 'r');
    for k in 0..3 {
        cv.line(sx - 1, sy + 3 + k * 2, sx - 3, sy + 4 + k * 2, 'i');
    }
    cv.rect(sx - 1, sy + 9, 3, 3, 'O');
    cv.put(sx, sy + 12, 'o');
    if glow {
        lights.push(fire(bx, by + 6, 40.0, 0.3));
    }
}

/// A lamp post's head: a red lantern on a crook.
pub(super) fn lamp_head(cv: &mut Img, x: i32, y: i32, on: bool, lights: &mut Vec<Light>) {
    cv.line(x + 1, y - 20, x + 6, y - 22, 'b');
    lantern(cv, x + 6, y - 22, on, lights);
}

/// The bats about the cavern: wheeling in loops, more of them as the
/// village wakes, riders on the big ones once it is bright; moths round
/// every lit lantern.
pub(super) fn ambient(cv: &mut Img, run: &Run, vl: &Village, lk: &Look) {
    let tick = run.tick as f32;
    let lit = vl.light();
    let n = 6 + (lit * 10.0) as i32;
    for k in 0..n {
        let h = hash(k, 11, 9630);
        let (ox, oy) = (60.0 + (h % 460) as f32, 30.0 + ((h >> 9) % 150) as f32);
        let (rx, ry) = (30.0 + ((h >> 3) % 60) as f32, 10.0 + ((h >> 13) % 20) as f32);
        let speed = 0.012 + ((h >> 17) % 10) as f32 * 0.002;
        let a = tick * speed * if k % 2 == 0 { 1.0 } else { -1.0 } + k as f32;
        let (x, y) = ((ox + a.cos() * rx) as i32, (oy + a.sin() * ry + (a * 3.0).sin() * 3.0) as i32);
        let big = k % 4 == 0;
        let flap = ((run.tick as u32 / 3 + h) % 4) < 2;
        flying_bat(cv, x, y, flap, big, big && lit > 0.5);
    }
    // Moths about the lamps that burn.
    for (i, &spot) in v::LAMPS.iter().enumerate().take(vl.lamps_lit()) {
        let (x, y) = super::px(spot);
        for m in 0..2 {
            let a = tick * 0.21 + (i * 3 + m) as f32;
            cv.put(x + 6 + (a.cos() * 5.0) as i32, y - 18 + (a.sin() * 3.0) as i32, if m == 0 { 'c' } else { lk.glow[1] });
        }
    }
    // Dust drifting down through the lantern light.
    for k in 0..24 {
        let h = hash(k, 12, 9631);
        let t = ((run.tick as u32 + h % 300) % 300) as i32;
        let x = (h % 576) as i32 + (vnoise(t as f32 / 30.0, k as f32, 9632) * 8.0) as i32;
        cv.put(x, 60 + t, if k % 3 == 0 { 'j' } else { 'X' });
    }
}

/// A bat in flight: wings up or down; a big one with a rider on it.
fn flying_bat(cv: &mut Img, x: i32, y: i32, up: bool, big: bool, rider: bool) {
    let span = if big { 7 } else { 4 };
    let tip = if up { -3 } else { 2 };
    for side in [-1, 1] {
        cv.line(x, y, x + side * span / 2, y + tip / 2 - 1, 'K');
        cv.line(x + side * span / 2, y + tip / 2 - 1, x + side * span, y + tip, 'K');
        if big {
            cv.line(x + side, y + 1, x + side * (span - 1), y + tip + 1, 'X');
        }
    }
    cv.rect(x - 1, y - 1, 2 + i32::from(big), 2 + i32::from(big), 'K');
    if big {
        cv.put(x - 1, y - 1, '7');
        cv.put(x + 1, y - 1, '7');
    }
    if rider {
        cv.rect(x - 1, y - 5, 3, 3, 'I');
        cv.put(x, y - 6, 'T');
        cv.put(x, y - 7, 'b');
    }
}

/// A floor below's moon-lantern, cold or lit: a crook on a post, a red
/// lantern from it.
pub(super) fn sign_lamp(cv: &mut Img, x: i32, head: i32, lit: bool) {
    cv.line(x, head + 2, x + 9, head - 2, 'b');
    cv.line(x, head + 3, x + 9, head - 1, 'n');
    let (lx, ly) = (x + 9, head - 2);
    cv.line(lx, ly, lx, ly + 3, 'b');
    cv.rect(lx - 3, ly + 3, 7, 1, 'K');
    cv.ellipse(lx as f32, (ly + 9) as f32, 5.0, 5.5, if lit { '7' } else { 'p' });
    for r in [ly + 7, ly + 11] {
        cv.line(lx - 4, r, lx + 4, r, if lit { '8' } else { 'B' });
    }
    if lit {
        cv.rect(lx - 1, ly + 7, 3, 4, '4');
        cv.put(lx, ly + 8, '6');
    }
    cv.rect(lx - 2, ly + 15, 5, 1, 'K');
    cv.line(lx, ly + 16, lx, ly + 19, 'R');
}

/// A skein of moth-silk on the floor below.
pub(super) fn relic(cv: &mut Img, x: i32, cy: i32) {
    cv.ellipse(x as f32, cy as f32, 5.0, 3.5, 'i');
    cv.ellipse(x as f32, cy as f32, 3.0, 2.0, 'h');
    for k in -4..=4 {
        if k % 2 == 0 {
            cv.put(x + k, cy + (k.abs() / 3) - 1, 'H');
        }
    }
    cv.line(x + 4, cy + 1, x + 8, cy + 4, 'i');
    cv.put(x - 2, cy - 2, '9');
}

/// Moth-silk over a door, as promised: a ribbon tied in a bow.
pub(super) fn over_the_door(cv: &mut Img, cx: i32, cy: i32) {
    cv.line(cx - 6, cy - 2, cx + 6, cy - 2, 'i');
    cv.line(cx - 6, cy - 1, cx - 8, cy + 3, 'h');
    cv.line(cx + 6, cy - 1, cx + 8, cy + 3, 'h');
    cv.rect(cx - 1, cy - 3, 3, 3, 'H');
    cv.put(cx, cy - 2, '9');
}
