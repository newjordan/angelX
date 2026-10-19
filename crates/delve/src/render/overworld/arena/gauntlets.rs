//! The gauntlets drawn in the dungeon's own hand: a gorge black as the
//! floor's own dark, a plank bridge over it lit by a lantern at each end,
//! pendulum axes that glint as they come down, iron stampers that shudder
//! and glow underneath before they fall, a boulder grinding sparks out of
//! its lane, and a saw spitting them off its rail.
//!
//! Danger is warm: everything that can hurt carries its own warm light onto
//! the black floor (`pool`), and every warning is warm light growing where
//! the blow will land. Everything reads the run's own state
//! (`Run::works`), so a friend's mirror shows what the host's does; the
//! frame is pure.

use super::super::ink::{Img, Rgb, bayer, hash};
use super::{UNIT, at, dungeon_art};
use crate::rules::gauntlets::{
    AXE_REACH, BOULDER_RADIUS, BOULDER_SPEED, BOULDER_WARN, BRIDGE_HALF, Gauntlet, HAMMER_HALF, HAMMER_SLAM,
    SAW_RADIUS,
};
use crate::rules::{Run, TILE_UNITS, Tile};
use std::f32::consts::TAU;

/// Pixels a tile.
const T: i32 = 16;
/// How high over a bridge along the screen its axes' beam runs, in pixels.
const GANTRY: i32 = 58;
/// A lantern's light, and a danger's.
const LANTERN: Rgb = [196, 128, 52];
const HOT: Rgb = [210, 82, 30];
/// How long a pendulum's arm is, in units, and how high its pivot hangs.
const ARM: f32 = 4.2;

/// The baked floor under the room, to light: the dungeon kit's still
/// picture, where it draws this room.
fn floor_of(run: &Run) -> Option<std::sync::Arc<dungeon_art::Scene>> {
    dungeon_art::supported(run).then(|| dungeon_art::scene(run, 1))
}

/// Warm light on the black: the stone shows where it falls, the gorge
/// stays dark. `power` is how strong it is at its middle.
fn pool(cv: &mut Img, run: &Run, floor: Option<&dungeon_art::Scene>, (cx, cy): (i32, i32), radius: i32, warm: Rgb, power: f32) {
    if radius <= 0 || power <= 0.0 {
        return;
    }
    let room = run.room();
    let r2 = (radius * radius) as f32;
    for y in cy - radius..=cy + radius {
        for x in cx - radius..=cx + radius {
            let d2 = ((x - cx) * (x - cx) + (y - cy) * (y - cy)) as f32;
            if d2 >= r2 {
                continue;
            }
            let (c, r) = (x.div_euclid(T), y.div_euclid(T));
            if !room.tile(c, r).ground() || plated(run, c, r) {
                continue;
            }
            let a = (1.0 - d2 / r2).powi(2) * power;
            let Some(under) = cv.get(x, y).or_else(|| floor.and_then(|f| f.base.get(x, y))) else {
                // No kit picture under it: a sparse warm dither stands in.
                if a > bayer(x, y) * 1.6 {
                    cv.set(x, y, warm.map(|v| (v as f32 * 0.22 * a.min(1.0)) as u8));
                }
                continue;
            };
            let lit = std::array::from_fn(|k| (under[k] as f32 * (1.0 + 1.8 * a) + warm[k] as f32 * a * 0.3).min(118.0) as u8);
            cv.set(x, y, lit);
        }
    }
}

/// A spike plate lies on tile `(c, r)`: its own drawing stays as it is.
fn plated(run: &Run, c: i32, r: i32) -> bool {
    run.traps.iter().any(|t| matches!(*t, crate::rules::Trap::Spikes { col, row, .. } if (col..col + 2).contains(&c) && (row..row + 2).contains(&r)))
}

/// Darken what is under a soft ellipse: a shadow cast down from above.
fn shade(cv: &mut Img, floor: Option<&dungeon_art::Scene>, (cx, cy): (i32, i32), rx: i32, ry: i32, depth: f32) {
    for y in cy - ry..=cy + ry {
        for x in cx - rx..=cx + rx {
            let (dx, dy) = ((x - cx) as f32 / rx.max(1) as f32, (y - cy) as f32 / ry.max(1) as f32);
            let d = dx * dx + dy * dy;
            if d > 1.0 {
                continue;
            }
            if let Some(c) = cv.get(x, y).or_else(|| floor.and_then(|f| f.base.get(x, y))) {
                let k = 1.0 - depth * (1.0 - d * 0.6);
                cv.set(x, y, c.map(|v| (v as f32 * k.clamp(0.0, 1.0)) as u8));
            }
        }
    }
}

/// Warm dither: `amount` (0 to 1) of the pixels of a rectangle glow, the
/// hottest in its middle.
fn ember_rect(cv: &mut Img, x0: i32, y0: i32, w: i32, h: i32, amount: f32, tick: u32) {
    for y in y0..y0 + h {
        for x in x0..x0 + w {
            let n = (hash(x, y, tick / 3) % 1000) as f32 / 1000.0;
            if n < amount * 0.55 {
                let ink = if amount > 0.85 && n < 0.18 {
                    '@'
                } else if amount > 0.5 && n < amount * 0.3 {
                    '7'
                } else {
                    '8'
                };
                cv.put(x, y, ink);
            }
        }
    }
}

// ─── the gorge and the bridge ────────────────────────────────────────────────

fn span(g: &Gauntlet) -> Option<(f32, f32, f32, bool)> {
    match *g {
        Gauntlet::Bridge { from, to, line, upright } => Some((from, to, line, upright)),
        _ => None,
    }
}

/// The gorge: black falling away below a lit lip, a cliff face under each
/// near edge, the bridge's timbers where it spans, a mote of mist below.
fn gorge(cv: &mut Img, run: &Run, from: f32, to: f32, line: f32, upright: bool) {
    let room = run.room();
    let t = run.tick as u32;
    let (a0, a1) = ((from / TILE_UNITS) as i32, (to / TILE_UNITS) as i32);
    let bridge = |c: i32, r: i32| {
        let (along, across) = if upright { (r, c) } else { (c, r) };
        let mid = (line / TILE_UNITS) as i32;
        (a0..a1).contains(&along) && (across == mid - 1 || across == mid)
    };
    let pit = |c: i32, r: i32| room.tile(c, r) == Tile::Hazard;
    for r in 0..room.rows as i32 {
        for c in 0..room.cols as i32 {
            if !pit(c, r) {
                continue;
            }
            let (x0, y0) = (c * T, r * T);
            // What stands at each side: firm floor, the bridge, more gorge.
            let north = !pit(c, r - 1) && room.tile(c, r - 1) != Tile::Wall;
            let north_bridge = bridge(c, r - 1);
            let south = !pit(c, r + 1) && room.tile(c, r + 1) != Tile::Wall;
            let west = !pit(c - 1, r) && room.tile(c - 1, r) != Tile::Wall;
            let east = !pit(c + 1, r) && room.tile(c + 1, r) != Tile::Wall;
            let west_bridge = bridge(c - 1, r);
            let east_bridge = bridge(c + 1, r);
            for yy in 0..T {
                for xx in 0..T {
                    let (x, y) = (x0 + xx, y0 + yy);
                    // The void: near black, a breath of cold far down.
                    let deep = hash(x / 2, y / 2, 17) % 97 == 0;
                    let mut c3: Rgb = if deep { [9, 10, 16] } else { [2, 2, 4] };
                    if north_bridge && yy < 4 {
                        // The bridge's own timber, its stringer seen from the side.
                        c3 = if yy == 0 { [74, 44, 22] } else if yy < 3 { [40, 22, 12] } else { [18, 10, 6] };
                    } else if north && yy < 7 {
                        // The cliff under the near edge, darkening as it falls.
                        let k = 1.0 - yy as f32 / 7.0;
                        let grain = (hash(x, y / 2, 5) % 3) as f32;
                        c3 = if yy == 0 { [92, 90, 82] } else { [((26.0 + grain * 3.0) * k) as u8 + 2, ((24.0 + grain * 3.0) * k) as u8 + 2, ((24.0 + grain * 2.0) * k) as u8 + 4] };
                    }
                    if (west_bridge && xx < 2) || (east_bridge && xx >= T - 2) {
                        c3 = [40, 22, 12];
                    } else if (west && xx == 0) || (east && xx == T - 1) {
                        c3 = [44, 44, 40];
                    }
                    if south && yy == T - 1 {
                        // The far lip, lit from above.
                        c3 = [60, 60, 54];
                    }
                    cv.set(x, y, c3);
                }
            }
            // A drifting mote of mist, now and then.
            let m = hash(c, r, 3);
            if m % 5 == 0 {
                let drift = ((t / 7 + m) % (T as u32)) as i32;
                cv.set(x0 + (m >> 8) as i32 % T, y0 + (drift + 8) % T, [16, 18, 26]);
            }
        }
    }
}

/// The planks across the bridge, a rope rail each side on its posts.
fn planks(cv: &mut Img, run: &Run, from: f32, to: f32, line: f32, upright: bool) {
    let (a0, a1) = ((from * UNIT) as i32, (to * UNIT) as i32);
    let mid = (line * UNIT) as i32;
    let half = (BRIDGE_HALF * UNIT) as i32;
    let _ = run;
    for along in a0..a1 {
        let plank = along.div_euclid(4);
        let gap = along.rem_euclid(4) == 3;
        // Old timber in the dark: the lanterns show its grain.
        let wood = match hash(plank, 7, 21) % 4 {
            0 => [50, 29, 13],
            1 => [58, 34, 16],
            2 => [44, 26, 12],
            _ => [62, 38, 18],
        };
        // Some planks sit a little proud or sag; a few are missing a corner.
        for k in -half..half {
            let (x, y) = if upright { (mid + k, along) } else { (along, mid + k) };
            let edge = k == -half || k == half - 1;
            let grain = hash(along / 2, k, plank as u32) % 9 == 0;
            let c3: Rgb = if gap {
                [14, 8, 5]
            } else if edge {
                [40, 24, 12]
            } else if grain {
                wood.map(|v| v * 3 / 4)
            } else {
                wood
            };
            cv.set(x, y, c3);
        }
    }
    // The rope rails, and a post every two tiles.
    for side in [-1, 1] {
        let rail = mid + side * (half - 1);
        for along in a0..a1 {
            let sag = ((along - a0) as f32 / 32.0 * std::f32::consts::PI).sin().abs() as i32;
            let (x, y) = if upright { (rail + side * sag, along) } else { (along, rail - 3 + sag) };
            cv.set(x, y, [150, 112, 60]);
        }
        let mut along = a0;
        while along <= a1 {
            for h in 0..5 {
                let (x, y) = if upright { (rail, along - h) } else { (along, rail - h) };
                cv.set(x, y, if h == 4 { [120, 80, 40] } else { [56, 32, 14] });
                let (x, y) = if upright { (rail + 1, along - h) } else { (along + 1, rail - h) };
                cv.set(x, y, [34, 20, 9]);
            }
            along += 32;
        }
    }
}

/// An iron lantern on a post, burning.
fn lantern(cv: &mut Img, (x, y): (i32, i32), tick: u32, seed: i32) {
    for h in 0..9 {
        cv.set(x, y - h, [30, 22, 16]);
    }
    let flick = (hash(seed, (tick / 4) as i32, 9) % 3) as u8;
    let cage: [&str; 6] = [".kXk.", "kX6Xk", "X5@6X", "X@4@X", "kX@Xk", ".kXk."];
    for (r, row) in cage.iter().enumerate() {
        for (c, ch) in row.chars().enumerate() {
            if ch == '.' {
                continue;
            }
            let ch = if ch == '6' && flick == 0 { '5' } else if ch == '4' && flick == 2 { '@' } else { ch };
            cv.put(x - 2 + c as i32, y - 15 + r as i32, ch);
        }
    }
}

/// Where a bridge's lanterns stand: on the landings, at its corners.
fn lanterns(from: f32, to: f32, line: f32, upright: bool) -> [(f32, f32); 2] {
    let side = BRIDGE_HALF + 0.5;
    if upright {
        [(line - side, from - 0.4), (line + side, to + 1.2)]
    } else {
        [(from - 0.6, line - side + 0.4), (to + 0.6, line + side)]
    }
}

// ─── the axe ─────────────────────────────────────────────────────────────────

/// Where the pendulum hangs from and where its blade is, on screen.
fn pendulum(g: &Gauntlet, tick: u64) -> ((i32, i32), (i32, i32), f32) {
    let Gauntlet::Axe { x, y, upright, .. } = *g else { return ((0, 0), (0, 0), 0.0) };
    let off = AXE_REACH * g.swing(tick);
    let height = ARM - (ARM * ARM - off * off).max(0.0).sqrt();
    let (bx, by) = g.blade(tick);
    let (gx, gy) = at(bx, by);
    let (px, py) = at(x, y);
    if upright {
        // Across the screen: a true arc, rising at either end.
        let lift = ((height + 1.0) * UNIT) as i32;
        ((px, py - ((ARM + 1.0) * UNIT) as i32), (gx, gy - lift), height)
    } else {
        // Toward and away from the eye: it rises as it swings, and its
        // shadow tells where it is.
        let lift = ((height * 0.6 + 1.0) * UNIT) as i32;
        ((px, py - GANTRY), (gx, gy - lift), height)
    }
}

/// What the axes hang from. Over a bridge across the screen: one beam the
/// length of the gorge, high over it, on a post at each end. Over a bridge
/// up the screen: a gallows over the bridge at each axe.
fn gantry(cv: &mut Img, run: &Run) {
    let beam = |cv: &mut Img, x0: i32, x1: i32, y: i32| {
        for x in x0..=x1 {
            cv.put(x, y - 1, 'P');
            cv.put(x, y, if (x - x0).rem_euclid(9) == 0 { 'b' } else { 'B' });
            cv.put(x, y + 1, 'I');
            cv.put(x, y + 2, 'n');
        }
    };
    let post = |cv: &mut Img, x: i32, y0: i32, y1: i32| {
        for y in y0..=y1 {
            cv.put(x, y, 'B');
            cv.put(x + 1, y, 'I');
            cv.put(x + 2, y, 'n');
        }
        // An iron band near its foot.
        cv.put(x, y1 - 3, 'g');
        cv.put(x + 1, y1 - 3, 'X');
        cv.put(x + 2, y1 - 3, 'X');
    };
    for g in &run.works.gauntlets {
        let Gauntlet::Bridge { from, to, line, upright } = *g else { continue };
        if upright {
            for a in run.works.gauntlets.iter().filter(|a| matches!(a, Gauntlet::Axe { .. })) {
                let ((px, py), _, _) = pendulum(a, run.tick);
                let (_, gy) = at(a.at().0, a.at().1);
                let half = (BRIDGE_HALF * UNIT) as i32 + 3;
                post(cv, px - half - 2, py - 2, gy + 2);
                post(cv, px + half, py - 2, gy + 2);
                beam(cv, px - half - 3, px + half + 3, py - 2);
            }
        } else {
            let (x0, gy) = at(from, line);
            let (x1, _) = at(to, line);
            let y = gy - GANTRY - 2;
            let foot = gy - (BRIDGE_HALF * UNIT) as i32 - 2;
            post(cv, x0 - 5, y, foot);
            post(cv, x1 + 2, y, foot);
            beam(cv, x0 - 6, x1 + 5, y);
        }
    }
}

/// The shadow under a blade, and the warm strip across the bridge where it
/// is about to come down.
fn axe_floor(cv: &mut Img, run: &Run, floor: Option<&dungeon_art::Scene>, g: &Gauntlet) {
    let Gauntlet::Axe { x, y, upright, .. } = *g else { return };
    let tick = run.tick;
    let t = tick as u32;
    let half = (BRIDGE_HALF * UNIT) as i32;
    let (px, py) = at(x, y);
    // The track across the bridge: warm as the blade comes, hot as it cuts.
    let heat = if g.low(tick) { 1.0 } else { g.warning(tick).unwrap_or(0.0) * 0.8 };
    if heat > 0.0 {
        let (x0, y0, w, h) = if upright { (px - half, py - 2, 2 * half, 5) } else { (px - 2, py - half, 5, 2 * half) };
        ember_rect(cv, x0, y0, w, h, heat, t);
    }
    let (_, (bx, by), height) = pendulum(g, tick);
    let (gx, gy) = at(g.blade(tick).0, g.blade(tick).1);
    let _ = (bx, by);
    if !run.room().tile(gx.div_euclid(T), gy.div_euclid(T)).ground() {
        return;
    }
    let size = (1.0 - height / ARM).clamp(0.3, 1.0);
    shade(cv, floor, (gx, gy), (9.0 * size) as i32 + 2, (3.0 * size) as i32 + 1, 0.75 * size);
}

/// The pendulum: its iron mount, the haft, and the crescent blade, a warm
/// glint along its edge as it comes down.
fn axe_air(cv: &mut Img, run: &Run, g: &Gauntlet) {
    let tick = run.tick;
    let ((px, py), (bx, by), _) = pendulum(g, tick);
    let low = g.low(tick);
    let warn = g.warning(tick).unwrap_or(0.0);
    // The blade's afterimage while it cuts: two faint arcs behind it.
    if low {
        for back in [2u64, 1] {
            let (_, (ox, oy), _) = pendulum(g, tick.saturating_sub(back));
            crescent(cv, (px, py), (ox, oy), if back == 2 { Some('8') } else { Some('7') }, false);
        }
    }
    // The mount: a dark iron bracket, a ring through it.
    for (dx, dy, ch) in [(-3, -1, 'X'), (-2, -1, 'g'), (-1, -1, 'g'), (0, -1, 'g'), (1, -1, 'g'), (2, -1, 'g'), (3, -1, 'X'), (-2, 0, 'K'), (-1, 0, 'X'), (0, 0, 'j'), (1, 0, 'X'), (2, 0, 'K'), (-1, 1, 'K'), (0, 1, 'G'), (1, 1, 'K')] {
        cv.put(px + dx, py + dy, ch);
    }
    // The haft, two pixels, lit on one side.
    let (dx, dy) = ((bx - px) as f32, (by - py) as f32);
    let len = dx.hypot(dy).max(1.0);
    let (nx, ny) = (-dy / len, dx / len);
    cv.line(px, py + 1, bx, by, 'X');
    cv.line(px + nx.round() as i32, py + 1 + ny.round() as i32, bx + nx.round() as i32, by + ny.round() as i32, 'j');
    crescent(cv, (px, py), (bx, by), None, low || warn > 0.5);
}

/// A crescent blade at `blade`, hung from `pivot`, its edge away from it:
/// solid steel, or only its edge in `ghost` ink.
fn crescent(cv: &mut Img, pivot: (i32, i32), blade: (i32, i32), ghost: Option<char>, glint: bool) {
    let (dx, dy) = ((blade.0 - pivot.0) as f32, (blade.1 - pivot.1) as f32);
    let len = dx.hypot(dy).max(1.0);
    let (ux, uy) = (dx / len, dy / len);
    let (vx, vy) = (-uy, ux);
    let inside = |px: f32, py: f32| {
        let (ox, oy) = (blade.0 as f32 - ux * 6.0, blade.1 as f32 - uy * 6.0);
        let (ix, iy) = (blade.0 as f32 - ux * 12.0, blade.1 as f32 - uy * 12.0);
        (px - ox).hypot(py - oy) <= 11.0 && (px - ix).hypot(py - iy) >= 12.5
    };
    for y in blade.1 - 13..=blade.1 + 13 {
        for x in blade.0 - 13..=blade.0 + 13 {
            let (fx, fy) = (x as f32, y as f32);
            if !inside(fx, fy) {
                // A dark rim round the steel, so it stands off any floor.
                if ghost.is_none() && [(1.0, 0.0), (-1.0, 0.0), (0.0, 1.0), (0.0, -1.0)].iter().any(|(ox, oy)| inside(fx + ox, fy + oy)) {
                    cv.put(x, y, 'k');
                }
                continue;
            }
            let (rx, ry) = (fx - blade.0 as f32, fy - blade.1 as f32);
            let along = rx * ux + ry * uy;
            let across = rx * vx + ry * vy;
            let (ox, oy) = (blade.0 as f32 - ux * 6.0, blade.1 as f32 - uy * 6.0);
            let edge = (fx - ox).hypot(fy - oy) > 9.6;
            if let Some(ink) = ghost {
                if edge {
                    cv.put(x, y, ink);
                }
                continue;
            }
            let ink = if edge {
                if glint && (across.abs() < 5.0 || (along as i32 + across as i32).rem_euclid(3) == 0) { '6' } else { 'W' }
            } else if across < -2.0 {
                'v'
            } else if across > 3.0 {
                'u'
            } else {
                'U'
            };
            cv.put(x, y, ink);
        }
    }
    if ghost.is_none() {
        // The socket the haft goes into.
        cv.put(blade.0 - (ux * 2.0) as i32, blade.1 - (uy * 2.0) as i32, 'g');
        cv.put(blade.0 - (ux * 3.0) as i32, blade.1 - (uy * 3.0) as i32, 'X');
    }
}

// ─── the hammer ──────────────────────────────────────────────────────────────

/// The plate it lands on, its shadow growing, and the glow under it before
/// it falls; cracks and dust after.
fn hammer_floor(cv: &mut Img, run: &Run, floor: Option<&dungeon_art::Scene>, g: &Gauntlet) {
    let Gauntlet::Hammer { x, y, .. } = *g else { return };
    let tick = run.tick;
    let t = tick as u32;
    let (x0, y0) = at(x - HAMMER_HALF, y - HAMMER_HALF);
    let w = (2.0 * HAMMER_HALF * UNIT) as i32;
    // The strike plate: dark iron, its rim and its rivets.
    for yy in 0..w {
        for xx in 0..w {
            let rim = xx == 0 || yy == 0 || xx == w - 1 || yy == w - 1;
            let inner = xx == 2 || yy == 2 || xx == w - 3 || yy == w - 3;
            let c: Rgb = if rim { [30, 28, 26] } else if inner { [20, 19, 18] } else { [12, 11, 11] };
            cv.set(x0 + xx, y0 + yy, c);
        }
    }
    for (rx, ry) in [(4, 4), (w - 5, 4), (4, w - 5), (w - 5, w - 5)] {
        cv.set(x0 + rx, y0 + ry, [70, 68, 62]);
    }
    // Its edge is marked hot, always: this square of floor is not to be
    // stood on lightly.
    for k in (1..w - 1).step_by(4) {
        for (ex, ey) in [(k, 1), (k, w - 2), (1, k), (w - 2, k)] {
            cv.put(x0 + ex, y0 + ey, '7');
        }
    }
    let drop = g.drop(tick);
    shade(cv, floor, (x0 + w / 2, y0 + w / 2), w / 2 - 2 + (drop * 3.0) as i32, w / 2 - 4 + (drop * 3.0) as i32, 0.25 + 0.6 * drop);
    if let Some(heat) = g.warning(tick) {
        ember_rect(cv, x0 + 3, y0 + 3, w - 6, w - 6, 0.25 + 0.75 * heat, t);
    }
    let since = g.beat(tick).wrapping_sub(HAMMER_SLAM);
    if since < 10 {
        // Cracks out from under it, and dust kicked out of its sides.
        for k in 0..8 {
            let a = k as f32 / 8.0 * TAU + 0.3;
            let reach = 3 + (hash(k, x0, 3) % 5) as i32;
            for s in 0..reach {
                let (cx, cy) = ((a.cos() * (w as f32 / 2.0 + s as f32)) as i32, (a.sin() * (w as f32 / 2.0 + s as f32)) as i32);
                cv.put(x0 + w / 2 + cx, y0 + w / 2 + cy, 'k');
            }
            let puff = since as i32 * 2 + 3;
            let (dx, dy) = ((a.cos() * (w as f32 / 2.0 + puff as f32)) as i32, (a.sin() * (w as f32 / 3.0 + puff as f32 * 0.5)) as i32);
            if since < 7 {
                cv.put(x0 + w / 2 + dx, y0 + w / 2 + dy, if since < 3 { 'h' } else { 'G' });
            }
        }
    }
}

/// The stamper itself: an iron block on chains, high in the dark (a ghost
/// of itself) until it comes down; shuddering and hot at the seam as it
/// warns.
fn hammer_air(cv: &mut Img, run: &Run, g: &Gauntlet) {
    let Gauntlet::Hammer { x, y, .. } = *g else { return };
    let tick = run.tick;
    let t = tick as u32;
    let (x0, y0) = at(x - HAMMER_HALF, y - HAMMER_HALF);
    let w = (2.0 * HAMMER_HALF * UNIT) as i32;
    let drop = g.drop(tick);
    let warn = g.warning(tick);
    let lift = ((1.0 - drop) * 40.0) as i32;
    let jitter = if warn.is_some() { if (t / 2).is_multiple_of(2) { 1 } else { -1 } } else { 0 };
    let bottom = y0 + w - lift;
    let (front, top) = (10, 18);
    let left = x0 + jitter;
    // The chains up into the dark.
    for cx in [left + 6, left + w - 7] {
        for cy in (0..bottom - front - top).rev().take(30) {
            cv.put(cx, cy, if cy.rem_euclid(4) < 2 { 'g' } else { 'X' });
        }
    }
    if drop <= 0.0 && warn.is_none() {
        // Up in the dark: only the iron yoke it hangs from shows.
        let y = bottom - front - top;
        for xx in 3..w - 3 {
            cv.put(left + xx, y, 'X');
            cv.put(left + xx, y + 1, if xx % 5 == 0 { 'j' } else { 'g' });
            cv.put(left + xx, y + 2, 'k');
        }
        return;
    }
    let ghost = false;
    for yy in 0..front + top {
        for xx in 0..w {
            let (px, py) = (left + xx, bottom - front - top + yy);
            if ghost && (px + py).rem_euclid(2) == 0 {
                continue;
            }
            let ink = if yy < top {
                // The top face: worn iron, lighter at its near edge.
                if xx == 0 || xx == w - 1 || yy == 0 {
                    'X'
                } else if yy == top - 1 {
                    'J'
                } else if (xx == 4 || xx == w - 5) && (yy == 4 || yy == top - 5) {
                    'h'
                } else if (xx + yy * 3).rem_euclid(11) == 0 {
                    'j'
                } else if xx == w / 2 || yy == top / 2 {
                    'X'
                } else {
                    'g'
                }
            } else {
                // The front face, and its hot seam as it warns.
                let seam = yy == top + front / 2;
                if xx == 0 || xx == w - 1 || yy == front + top - 1 {
                    'k'
                } else if seam && warn.is_some_and(|w| w > 0.0) {
                    let h = warn.unwrap_or(0.0);
                    if h > 0.7 { '@' } else if h > 0.35 { '7' } else { '8' }
                } else if seam {
                    'K'
                } else {
                    'X'
                }
            };
            cv.put(px, py, ink);
        }
    }
}

// ─── the boulder ─────────────────────────────────────────────────────────────

fn boulder_floor(cv: &mut Img, run: &Run, floor: Option<&dungeon_art::Scene>, g: &Gauntlet) {
    let Gauntlet::Boulder { from, to, y, .. } = *g else { return };
    let tick = run.tick;
    let t = tick as u32;
    let west = from < to;
    let (xa, xb) = (from.min(to), from.max(to));
    let (x0, x1) = (((xa - BOULDER_RADIUS) * UNIT) as i32, ((xb + BOULDER_RADIUS) * UNIT) as i32);
    let cy = (y * UNIT) as i32;
    let r = (BOULDER_RADIUS * UNIT) as i32;
    let rumble = g.warning(tick);
    // The worn lane: two ruts, polished where the light finds them.
    for x in x0..=x1 {
        for side in [-1, 1] {
            let ry = cy + side * (r - 2);
            cv.set(x, ry, [6, 5, 5]);
            cv.set(x, ry + side, [10, 9, 8]);
            if hash(x, side, 4) % 3 == 0 {
                cv.set(x, ry - side, [40, 37, 32]);
            }
        }
    }
    // Chevrons down its length, pointing the way it rolls: dim embers at
    // rest, lit one after another from the mouth as it rumbles.
    let lit_to = rumble.map_or(-1.0, |w| w * 1.15);
    let count = ((x1 - x0) / 16).max(1);
    for k in 0..count {
        let along = k as f32 / count as f32;
        let cx = if west { x0 + 8 + k * 16 } else { x1 - 8 - k * 16 };
        let hot = along <= lit_to;
        let dir = if west { 1 } else { -1 };
        for d in 0..4 {
            let ink = if hot { if d == 0 { '@' } else { '7' } } else { '8' };
            if hot || (hash(k, d, 2) % 2 == 0) {
                cv.put(cx + dir * (2 - d), cy - d, ink);
                cv.put(cx + dir * (2 - d), cy + d, ink);
            }
        }
    }
    // The mouth in the wall it comes out of: a dark arch, its keystones
    // pale, the boulder's face just showing inside; hot as it rumbles.
    let wx = if west { 0 } else { (run.room().cols as i32 - 1) * T };
    let (top, bot) = (cy - r - 4, cy + r + 1);
    for yy in top..=bot {
        for xx in 0..T {
            let x = wx + xx;
            let rim = yy == top || yy == bot || xx == 0 || xx == T - 1;
            let arch = yy < top + 3 && (xx - T / 2).abs() > 3 + (yy - top) * 2;
            let c: Rgb = if arch || rim {
                [54, 50, 44]
            } else {
                match rumble {
                    Some(w) => {
                        let flick = (hash(xx, yy, t / 2) % 4) as f32 / 4.0;
                        let k = w * (0.55 + 0.45 * flick);
                        let face = (xx - if west { 3 } else { T - 4 }).abs() < 4 && (yy - cy).abs() < r - 2;
                        if face { [(70.0 * k) as u8 + 30, (56.0 * k) as u8 + 26, (44.0 * k) as u8 + 22] } else { [(150.0 * k) as u8 + 6, (58.0 * k) as u8 + 4, (16.0 * k) as u8 + 3] }
                    }
                    None => [4, 3, 3],
                }
            };
            cv.set(x, yy, c);
        }
    }
    if rumble.is_some() {
        // Grit trickling from the lintel.
        for p in 0..4u32 {
            let fall = ((t + p * 5) % 10) as i32;
            let x = wx + if west { T + 1 + p as i32 } else { -2 - p as i32 };
            cv.put(x, top + fall, 'G');
        }
    }
    if let Some((bx, by)) = g.boulder(tick) {
        let (sx, sy) = at(bx, by);
        shade(cv, floor, (sx, sy + r - 3), r + 2, 4, 0.85);
        // Grit kicked up behind it.
        let back = if west { -1 } else { 1 };
        for k in 0..12 {
            let d = (hash(k, t as i32, 6) % 20) as i32;
            let up = (hash(k, t as i32, 7) % 7) as i32;
            cv.put(sx + back * (r - 2 + d), sy + r - 3 - up, if k % 3 == 0 { 'G' } else { 'j' });
        }
    }
}

fn boulder_air(cv: &mut Img, run: &Run, g: &Gauntlet) {
    let Gauntlet::Boulder { from, to, .. } = *g else { return };
    let tick = run.tick;
    let t = tick as u32;
    let Some((bx, by)) = g.boulder(tick) else { return };
    let (cx, cy) = at(bx, by);
    let r = BOULDER_RADIUS * UNIT;
    let gone = (g.beat(tick).saturating_sub(BOULDER_WARN)) as f32 * BOULDER_SPEED;
    let roll = gone / BOULDER_RADIUS * if from < to { 1.0 } else { -1.0 };
    let ri = r as i32 + 1;
    for y in -ri..=ri {
        for x in -ri..=ri {
            let (fx, fy) = (x as f32 + 0.5, y as f32 + 0.5);
            let d = fx.hypot(fy);
            // A rough stone, not a ball: its outline is knobbled.
            let ang = fy.atan2(fx);
            let knob = ((hash(((ang + 3.2) * 2.2) as i32, 1, 77) % 3) as f32 - 1.0) * 0.8;
            let edge = r + knob;
            if d > edge {
                continue;
            }
            let light = (-fx * 0.5 - fy * 0.85) / r + (1.0 - d / edge) * 0.5;
            let w = (edge * edge - fy * fy).max(0.01).sqrt();
            let lon = (fx / w).clamp(-1.0, 1.0).asin() - roll;
            let (u, v) = ((lon * 3.0).floor() as i32, (fy / 3.5).floor() as i32);
            let cell = hash(u.rem_euclid(19), v, 31);
            let pit = cell % 5 == 0 && (lon * 3.0).fract().abs() < 0.35;
            let seam = cell % 4 == 1 && ((fy / 3.5).fract().abs() < 0.2);
            let grain = (hash(x, y, 5) % 3) as f32 * 0.05;
            let l = light + grain;
            let ink = if d > edge - 1.0 {
                'k'
            } else if pit || seam {
                if l > 0.5 { 'X' } else { 'K' }
            } else if l > 0.78 {
                'h'
            } else if l > 0.5 {
                'J'
            } else if l > 0.22 {
                'G'
            } else if l > -0.1 {
                'j'
            } else if l > -0.4 {
                'g'
            } else {
                'X'
            };
            cv.put(cx + x, cy + y - 3, ink);
        }
    }
    // Sparks where it grinds the floor, thrown back from its foot.
    let back = if from < to { -1 } else { 1 };
    for k in 0..8 {
        let n = hash(k, t as i32, 13);
        let dx = back * ((n % 7) as i32);
        let up = ((n >> 4) % 5) as i32;
        cv.put(cx + dx + (n >> 8) as i32 % 3 - 1, cy + ri - 4 - up, if n % 3 == 0 { '6' } else { '@' });
    }
}

// ─── the saw ─────────────────────────────────────────────────────────────────

fn saw_floor(cv: &mut Img, run: &Run, g: &Gauntlet) {
    let Gauntlet::Saw { a, b, .. } = *g else { return };
    let tick = run.tick;
    let t = tick as u32;
    let (ax, ay) = at(a.0, a.1);
    let (bx, by) = at(b.0, b.1);
    let upright = (ax - bx).abs() < (ay - by).abs();
    // The slot: a black groove between two iron lips, a dull ember down
    // its middle, an iron cap at each end.
    let steps = (ax - bx).abs().max((ay - by).abs()).max(1);
    for k in -5..=steps + 5 {
        let s = k as f32 / steps as f32;
        let (x, y) = (ax + ((bx - ax) as f32 * s) as i32, ay + ((by - ay) as f32 * s) as i32);
        for w in -3..=3 {
            let (px, py) = if upright { (x + w, y) } else { (x, y + w) };
            let c: Rgb = match w {
                -3 => [16, 15, 14],
                3 => [62, 60, 54],
                -2 | 2 => [34, 32, 30],
                0 if hash(k, 0, t / 3) % 3 == 0 => [70, 20, 10],
                _ => [3, 3, 3],
            };
            cv.set(px, py, c);
        }
    }
    for (ex, ey) in [(ax, ay), (bx, by)] {
        let (ox, oy) = if upright { (ex - 4, ey - 6) } else { (ex - 6, ey - 4) };
        let (w, h) = if upright { (9, 12) } else { (12, 9) };
        for yy in 0..h {
            for xx in 0..w {
                let rim = xx == 0 || yy == 0 || xx == w - 1 || yy == h - 1;
                cv.put(ox + xx, oy + yy, if rim { 'X' } else if (xx + yy) % 4 == 0 { 'j' } else { 'g' });
            }
        }
    }
    let ((sx, sy), rest) = g.saw_at(tick);
    let (cx, cy) = at(sx, sy);
    // The slot ahead of it glows where it is about to run.
    let ((nx, ny), _) = g.saw_at(tick + 10);
    let (tx, ty) = at(nx, ny);
    let heat = if rest.is_some() { 0.45 + 0.5 * rest.unwrap_or(0.0) } else { 0.85 };
    let reach = (tx - cx).abs().max((ty - cy).abs());
    for k in 0..=reach {
        let s = k as f32 / reach.max(1) as f32;
        let (x, y) = (cx + ((tx - cx) as f32 * s) as i32, cy + ((ty - cy) as f32 * s) as i32);
        for w in -1..=1 {
            let (px, py) = if upright { (x + w, y) } else { (x, y + w) };
            if (hash(px, py, t / 2) % 100) as f32 / 100.0 < heat * (1.0 - s * 0.5) {
                cv.put(px, py, if w == 0 && s < 0.5 { '@' } else { '7' });
            }
        }
    }
}

fn saw_air(cv: &mut Img, run: &Run, g: &Gauntlet) {
    let Gauntlet::Saw { .. } = *g else { return };
    let tick = run.tick;
    let t = tick as u32;
    let ((sx, sy), rest) = g.saw_at(tick);
    let (cx, cy) = at(sx, sy);
    let cy = cy - 3;
    let r = SAW_RADIUS * UNIT + 1.0;
    let spin = t as f32 * 0.7;
    let ri = r as i32 + 2;
    // The whirling disc, seen from above: steel teeth round a blurred
    // face, a dark hub.
    for y in -ri..=ri {
        for x in -ri..=ri {
            let (fx, fy) = (x as f32 + 0.5, (y as f32 + 0.5) * 1.15);
            let d = fx.hypot(fy);
            if d > r + 1.6 {
                continue;
            }
            let ang = fy.atan2(fx);
            let tooth = ((ang * 12.0 / TAU + spin).floor() as i32).rem_euclid(2) == 0;
            let ink = if d > r {
                if tooth { 'V' } else { continue }
            } else if d > r - 1.5 {
                if tooth { 'W' } else { 'v' }
            } else if d > r - 2.5 {
                'u'
            } else if d < 2.2 {
                if d < 1.0 { 'j' } else { 'X' }
            } else if ((ang * 3.0 / TAU + spin * 0.5).floor() as i32).rem_euclid(2) == 0 {
                'U'
            } else {
                'u'
            };
            cv.put(cx + x, cy + y, ink);
        }
    }
    // Sparks thrown off where it bites the slot: a fan of them, a burst
    // while it revs.
    let ((nx, ny), _) = g.saw_at(tick + 1);
    let (dx, dy) = (nx - sx, ny - sy);
    let moving = dx.abs() + dy.abs() > 1.0e-4;
    let back = if moving { (-dx.signum() * f32::from(dx.abs() > 1.0e-4), -dy.signum() * f32::from(dy.abs() > 1.0e-4)) } else { (0.0, 0.0) };
    let n = if rest.is_some() { 14 } else { 9 };
    for k in 0..n {
        let h = hash(k, t as i32, 41);
        let reach = ri + (h % 9) as i32;
        let side = ((h >> 9) % 7) as i32 - 3;
        let up = ((h >> 5) % 6) as i32;
        let (px, py) = if moving {
            (cx + (back.0 * reach as f32) as i32 + if back.0 == 0.0 { side } else { 0 }, cy + 3 + (back.1 * reach as f32) as i32 - up + if back.1 == 0.0 { side / 2 } else { 0 })
        } else {
            (cx + side * 3, cy + ri - up * 2)
        };
        cv.put(px, py, if h % 3 == 0 { '6' } else if h % 3 == 1 { '@' } else { '4' });
    }
}

// ─── layers ──────────────────────────────────────────────────────────────────

/// The warm lights the gauntlets carry onto the floor, in room pixels:
/// where, how far, how strong.
pub(super) fn lights(run: &Run) -> Vec<(i32, i32, i32, f32, Rgb)> {
    let tick = run.tick;
    let t = tick as u32;
    let mut out = Vec::new();
    for g in &run.works.gauntlets {
        match *g {
            Gauntlet::Bridge { from, to, line, upright } => {
                for (i, (x, y)) in lanterns(from, to, line, upright).into_iter().enumerate() {
                    let (px, py) = at(x, y);
                    let flick = (hash(i as i32, (t / 4) as i32, 9) % 3) as f32 * 0.04;
                    out.push((px, py - 4, 46, 0.62 + flick, LANTERN));
                }
            }
            Gauntlet::Axe { .. } => {
                if let Some(w) = g.warning(tick).or(g.low(tick).then_some(1.0)) {
                    let (gx, gy) = at(g.blade(tick).0, g.blade(tick).1);
                    out.push((gx, gy, 18, 0.25 * w, HOT));
                }
            }
            Gauntlet::Hammer { x, y, .. } => {
                let (px, py) = at(x, y);
                if let Some(w) = g.warning(tick) {
                    out.push((px, py, 26, 0.2 + 0.5 * w, HOT));
                } else {
                    out.push((px, py, 28, 0.3, LANTERN));
                }
            }
            Gauntlet::Boulder { from, y, .. } => {
                let (mx, my) = at(from, y);
                if let Some(w) = g.warning(tick) {
                    out.push((mx, my, 40, 0.3 + 0.5 * w, HOT));
                } else {
                    out.push((mx, my, 24, 0.18, LANTERN));
                }
                if let Some((bx, by)) = g.boulder(tick) {
                    let (sx, sy) = at(bx, by);
                    out.push((sx, sy + 6, 34, 0.45, LANTERN));
                }
            }
            Gauntlet::Saw { .. } => {
                let ((sx, sy), rest) = g.saw_at(tick);
                let (px, py) = at(sx, sy);
                out.push((px, py, 30, if rest.is_some() { 0.55 } else { 0.4 }, HOT));
            }
        }
    }
    out
}

/// Under the figures: the gorge and the bridge, lanterns' and dangers'
/// light, shadows, plates, lanes, slots and every warning.
pub(super) fn under(cv: &mut Img, run: &Run) {
    if run.works.gauntlets.is_empty() {
        return;
    }
    let floor = floor_of(run);
    let floor = floor.as_deref();
    let t = run.tick as u32;
    for g in &run.works.gauntlets {
        if let Some((from, to, line, upright)) = span(g) {
            gorge(cv, run, from, to, line, upright);
        }
    }
    for (x, y, r, power, warm) in lights(run) {
        pool(cv, run, floor, (x, y), r, warm, power);
    }
    for g in &run.works.gauntlets {
        match g {
            Gauntlet::Bridge { from, to, line, upright } => {
                planks(cv, run, *from, *to, *line, *upright);
                // Light the planks themselves from the lanterns.
                for (x, y) in lanterns(*from, *to, *line, *upright) {
                    pool(cv, run, floor, at(x, y), 40, LANTERN, 0.35);
                }
            }
            Gauntlet::Axe { .. } => axe_floor(cv, run, floor, g),
            Gauntlet::Hammer { .. } => hammer_floor(cv, run, floor, g),
            Gauntlet::Boulder { .. } => boulder_floor(cv, run, floor, g),
            Gauntlet::Saw { .. } => saw_floor(cv, run, g),
        }
    }
    for g in &run.works.gauntlets {
        if let Gauntlet::Bridge { from, to, line, upright } = *g {
            for (i, (x, y)) in lanterns(from, to, line, upright).into_iter().enumerate() {
                let (px, py) = at(x, y);
                lantern(cv, (px, py), t, i as i32);
            }
        }
    }
}

/// Over the figures: the axes, the hammers, the boulder and the saws, the
/// nearer ones over the farther.
pub(super) fn over(cv: &mut Img, run: &Run) {
    let mut order: Vec<&Gauntlet> = run.works.gauntlets.iter().filter(|g| !matches!(g, Gauntlet::Bridge { .. })).collect();
    order.sort_by(|a, b| a.at().1.total_cmp(&b.at().1));
    gantry(cv, run);
    for g in order {
        match g {
            Gauntlet::Axe { .. } => axe_air(cv, run, g),
            Gauntlet::Hammer { .. } => hammer_air(cv, run, g),
            Gauntlet::Boulder { .. } => boulder_air(cv, run, g),
            Gauntlet::Saw { .. } => saw_air(cv, run, g),
            Gauntlet::Bridge { .. } => {}
        }
    }
}

#[cfg(test)]
#[path = "../../../../tests/render/arena__gauntlets__tests.rs"]
mod tests;
