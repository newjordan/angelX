//! A village of the Deep, drawn (`rules::deep::village`): one great cavern
//! in its people's own stone and light. Its back wall is the dark the star
//! falls out of, stalactites hanging and glowworms for stars; along the
//! walls stand the people's houses (mushroom cottages, crystal-hewn halls,
//! basalt forges, stilt-houses over black water), the great hall at the
//! top, the heart in the plaza, the wishing place by the well.
//!
//! The village is drawn dim and lights up as hope grows: its lamps take
//! fire one by one, windows glow, the heart wakes, and the dark lifts off
//! the stone a step at a time (`gloom`). What moves (the folk, the falling
//! star, spores and smoke, water and glints, the city's wishes rising) is
//! drawn over the light, every frame (`figures`); the boards are the
//! screen's (`screen`).
//!
//! The effects here (the star's streak, its burst, the wishes' sparkle)
//! are small and local so they can move onto the shared kit when it lands.

use super::super::ink::{BLACK, Img, hash, vnoise};
use super::super::light::{Light, step_down};
use super::super::material as mat;
use super::{TILE, at, fire};
use crate::rules::{
    Run,
    deep::{Culture, Village, village as v},
};

mod bells;
mod dread;
mod folk;
mod gifts;
mod houses;
mod panel;
mod road;
mod roost;
mod roots;
mod salt;
mod sky;

#[cfg(any(test, feature = "testing"))]
pub(super) use dread::rows_for as dread_rows;
pub(super) use dread::{figure as dread_figure, lantern_beam as dread_lantern, under as dread_under};
pub(super) use panel::{screen, signs};
pub(super) use road::meeting;

/// A people's stone and light, as inks.
pub(super) struct Look {
    /// The cavern floor, dark to light.
    pub ground: [char; 4],
    /// The cavern's rock, dark to light.
    pub rock: [char; 6],
    /// What the dark glows with: the people's own cool or warm light,
    /// dim to bright. The last two are lamplight (signal inks).
    pub glow: [char; 4],
    /// A lit lamp's flame, core and halo.
    pub flame: (char, char),
    /// The dusk's pools on bare paper.
    pub pool: (char, char),
    /// Whether its light is fire (warms what it touches).
    pub warm: bool,
}

pub(super) fn look(culture: Culture) -> Look {
    match culture {
        Culture::Capfolk => Look {
            ground: ['f', 'D', 'F', 'e'],
            rock: ['n', 'b', 'I', 'B', 'P', 'r'],
            glow: ['q', 'z', '2', '3'],
            flame: ('3', '2'),
            pool: ('F', 'e'),
            warm: false,
        },
        Culture::Glimmerkin => Look {
            ground: ['s', 'x', 'S', 'u'],
            rock: ['s', 'x', 'S', 'u', 'U', 'v'],
            glow: ['Q', 'z', '3', 'w'],
            flame: ('w', '3'),
            pool: ('x', 'u'),
            warm: false,
        },
        Culture::Emberfolk => Look {
            ground: ['k', 'K', 'X', 'g'],
            rock: ['k', 'K', 'X', 'g', 'j', 'G'],
            glow: ['B', 'R', '@', '6'],
            flame: ('6', '@'),
            pool: ('X', 'r'),
            warm: true,
        },
        Culture::Lampwick => Look {
            ground: ['n', 'b', 'I', 'B'],
            rock: ['s', 'x', 'S', 'q', 'u', 'U'],
            glow: ['q', 'Q', '5', '6'],
            flame: ('6', '5'),
            pool: ('x', 'q'),
            warm: true,
        },
        // The roost's light is the red of its lanterns, and its stars are
        // the bats' eyes.
        Culture::Roostfolk => Look {
            ground: ['K', 'X', 'X', 'g'],
            rock: ['k', 'K', 'X', 'g', 'j', 'G'],
            glow: ['8', 'R', '7', '4'],
            flame: ('@', '7'),
            pool: ('X', 'g'),
            warm: true,
        },
        Culture::Rootkin => Look {
            ground: ['f', 'D', 'n', 'b'],
            rock: ['n', 'b', 'I', 'B', 'p', 'P'],
            glow: ['a', 'o', '4', '5'],
            flame: ('5', '4'),
            pool: ('b', 'I'),
            warm: true,
        },
        Culture::Chimefolk => Look {
            ground: ['s', 'x', 'S', 'u'],
            rock: ['K', 'g', 'j', 'G', 'J', 'h'],
            glow: ['e', 'C', '2', '3'],
            flame: ('3', '2'),
            pool: ('x', 'S'),
            warm: false,
        },
        // Salt-light is dawn-coloured: a warm rose over the white.
        Culture::Saltkin => Look {
            ground: ['g', 'G', 'h', 'i'],
            rock: ['G', 'J', 'h', 'i', 'H', 'W'],
            glow: ['$', 'c', '@', '9'],
            flame: ('9', '@'),
            pool: ('J', 'h'),
            warm: true,
        },
    }
}

/// A tile rect in pixels.
pub(super) fn px_rect((c, r, w, h): (i32, i32, i32, i32)) -> (i32, i32, i32, i32) {
    (c * TILE, r * TILE, w * TILE, h * TILE)
}

/// A tile point in pixels.
pub(super) fn px((x, y): (f32, f32)) -> (i32, i32) {
    at(x * 2.0, y * 2.0)
}

fn all(_: i32, _: i32) -> bool {
    true
}

/// The village the party stands in, while it does.
pub(super) fn village(run: &Run) -> Option<&Village> {
    run.deep.as_ref()?.village.as_ref().filter(|_| run.in_deep_village())
}

/// What the kept drawing of a village depends on beyond its room: how lit
/// it is, and what the realm has built in it. Nothing outside a village.
pub(super) fn print(run: &Run) -> u64 {
    let Some(vl) = village(run) else {
        return 0;
    };
    let town = run.home.deep.town(vl.floor);
    let built = town.map_or(0, |t| t.wishes.len() as u64 * 31 + t.deeds.len() as u64 * 7 + u64::from(t.lit));
    (u64::from(vl.hope) << 8 | built << 16 | vl.culture as u64) << 1 | 1
}

/// The village's still art, lit for how much hope it holds.
pub(super) fn scenery(run: &Run) -> Img {
    let room = run.room();
    let (pw, ph) = (room.cols as i32 * TILE, room.rows as i32 * TILE);
    let mut cv = Img::black(pw, ph);
    let Some(vl) = village(run) else {
        return cv;
    };
    let lk = look(vl.culture);
    let tick = run.tick as u32;
    let mut lights: Vec<Light> = Vec::new();
    ground(&mut cv, vl.culture, &lk);
    match vl.culture {
        Culture::Emberfolk => lava_channels(&mut cv, &mut lights),
        Culture::Rootkin => roots::floor_roots(&mut cv, &mut lights),
        _ => {}
    }
    sky::back_wall(&mut cv, vl, &lk, &mut lights);
    sky::far_dwellings(&mut cv, vl, &lk, &mut lights);
    side_walls(&mut cv, &lk);
    plaza(&mut cv, vl.culture, &lk);
    let town = run.home.deep.town(vl.floor);
    houses::all(&mut cv, vl, town, &lk, &mut lights, tick);
    if vl.culture == Culture::Roostfolk {
        roost::bridges(&mut cv);
    }
    let deeds: Vec<crate::rules::deep::Errand> = town.map_or_else(Vec::new, |t| t.deeds.clone());
    lamp_posts(&mut cv, vl, &lk, &mut lights, deeds.contains(&crate::rules::deep::Errand::Beacons));
    // The stair on down, and the lift's foot by the light home.
    let (sx, sy) = (v::STAIRS.0 * TILE, v::STAIRS.1 * TILE);
    cv.stamp(&super::sprites::stairs(true), sx, sy);
    lift(&mut cv, &lk, &mut lights);
    // Masses lifted toward the light as the world above draws them: the
    // ground stays dark and calm, the houses read first.
    super::dusk_with(&mut cv, &lights, lk.pool, 1.0);
    gloom(&mut cv, &lights, vl.light());
    cv
}

/// The cavern floor: the people's ground in soft patches, worn paler along
/// the ways folk walk.
fn ground(cv: &mut Img, culture: Culture, lk: &Look) {
    let (w, h) = (cv.w, cv.h);
    match culture {
        Culture::Lampwick => return lake_floor(cv),
        Culture::Chimefolk => return bells::water_floor(cv),
        Culture::Saltkin => return salt::salt_floor(cv),
        _ => {}
    }
    for y in (v::BACK as i32 * TILE)..h - TILE {
        for x in TILE..w - TILE {
            let n = vnoise(x as f32 / 11.0, y as f32 / 9.0, 9101 + culture as u32);
            let fine = (hash(x, y, 9102) % 100) as f32 / 100.0;
            let t = n * 0.8 + fine * 0.2;
            let i = if t > 0.72 { 3 } else if t > 0.5 { 2 } else if t > 0.3 { 1 } else { 0 };
            // Sparse marks only: black paper between, as every room keeps.
            if fine < 0.55 || i >= 2 {
                cv.put(x, y, lk.ground[i]);
            }
        }
    }
    // The people's own ground: moss and caps, crystal grit, cinders, planks.
    for k in 0..140 {
        let hk = hash(k, 3, 9103 + culture as u32);
        let (x, y) = (TILE + (hk % (w - 2 * TILE) as u32) as i32, v::BACK as i32 * TILE + 4 + ((hk >> 10) % (h as u32 - 6 * TILE as u32)) as i32);
        match culture {
            Culture::Capfolk => {
                cv.put(x, y, 'E');
                if k % 5 == 0 {
                    cv.put(x, y - 1, 'T');
                    cv.put(x - 1, y - 2, 'R');
                    cv.put(x, y - 2, 'R');
                    cv.put(x + 1, y - 2, 'R');
                }
            }
            Culture::Glimmerkin => {
                cv.put(x, y, if k % 3 == 0 { 'z' } else { 'u' });
                if k % 7 == 0 {
                    cv.line(x, y, x + 1, y - 3, 'Q');
                }
            }
            Culture::Emberfolk => cv.put(x, y, if k % 4 == 0 { 'B' } else { 'X' }),
            Culture::Lampwick => cv.put(x, y, if k % 3 == 0 { 'I' } else { 'b' }),
            Culture::Roostfolk => roost::ground_marks(cv, k, (x, y)),
            Culture::Rootkin => roots::ground_marks(cv, k, (x, y)),
            Culture::Chimefolk | Culture::Saltkin => {}
        }
    }
}

/// The Emberfolk's floor: channels of slow fire cut across it, from the
/// falls on the back wall down to the forge-heart, crusted at the edges.
fn lava_channels(cv: &mut Img, lights: &mut Vec<Light>) {
    let paths: [&[(i32, i32)]; 2] = [&[(96, 114), (110, 168), (150, 198), (230, 234)], &[(410, 114), (400, 168), (360, 198), (340, 234)]];
    for path in paths {
        for seg in path.windows(2) {
            let ((x0, y0), (x1, y1)) = (seg[0], seg[1]);
            let steps = (x1 - x0).abs().max((y1 - y0).abs()).max(1);
            for s in 0..=steps {
                let (x, y) = (x0 + (x1 - x0) * s / steps, y0 + (y1 - y0) * s / steps);
                let wide = 3 + (vnoise(x as f32 / 9.0, y as f32 / 9.0, 9107) * 3.0) as i32;
                for o in -wide - 2..=wide + 2 {
                    let ink = if o.abs() > wide { 'g' } else if o.abs() == wide { 'B' } else if o.abs() < wide / 2 + 1 { '@' } else { '7' };
                    cv.put(x + o, y, ink);
                }
                if s % 24 == 0 {
                    lights.push(fire(x, y, 40.0, 0.3));
                }
            }
        }
    }
}

/// The Lampwick's floor is the lake: black water, its ripples catching
/// what light there is.
fn lake_floor(cv: &mut Img) {
    let (w, h) = (cv.w, cv.h);
    for y in (v::BACK as i32 * TILE)..h - TILE {
        for x in TILE..w - TILE {
            let n = vnoise(x as f32 / 17.0, y as f32 / 5.0, 9105);
            let ripple = ((x as f32 / 9.0 + n * 6.0).sin() * (y as f32 / 3.0).cos()) > 0.86;
            let ink = if ripple { 'Q' } else if n > 0.62 { 'x' } else if n > 0.25 { 's' } else { 'k' };
            if ink != 'k' || hash(x, y, 9106) % 3 == 0 {
                cv.put(x, y, ink);
            }
        }
    }
}

/// The rock round the cavern's sides and south: dark stone, shaded off
/// into the black.
fn side_walls(cv: &mut Img, lk: &Look) {
    let (w, h) = (cv.w, cv.h);
    let top = v::BACK as i32 * TILE;
    for y in top..h {
        for x in 0..w {
            let edge = x.min(w - 1 - x).min(h - 1 - y);
            let ragged = (vnoise(x as f32 / 5.0, y as f32 / 5.0, 9111) * 9.0) as i32;
            if edge < TILE - 6 + ragged / 2 {
                let n = vnoise(x as f32 / 4.0, y as f32 / 3.0, 9112);
                let i = ((n * 3.0) as usize + (edge as usize * 3 / TILE as usize)).min(4);
                cv.put(x, y, lk.rock[i]);
            }
        }
    }
}

/// The plaza round the heart: paving in rings, and the ways out to the
/// houses, the well and the stair.
fn plaza(cv: &mut Img, culture: Culture, lk: &Look) {
    let (cx, cy) = (18 * TILE, 15 * TILE + 8);
    let stone = match culture {
        Culture::Capfolk => ['b', 'I', 'P', 'r'],
        Culture::Glimmerkin => ['x', 'S', 'u', 'U'],
        Culture::Emberfolk => ['K', 'X', 'g', 'j'],
        Culture::Lampwick => ['b', 'I', 'B', 'P'],
        Culture::Roostfolk => ['K', 'X', 'g', 'j'],
        Culture::Rootkin => ['n', 'b', 'I', 'B'],
        Culture::Chimefolk => ['K', 'g', 'j', 'G'],
        Culture::Saltkin => ['G', 'J', 'h', 'i'],
    };
    for y in cy - 76..cy + 76 {
        for x in cx - 120..cx + 120 {
            let (dx, dy) = ((x - cx) as f32 / 118.0, (y - cy) as f32 / 74.0);
            let d = dx * dx + dy * dy;
            if d >= 1.0 {
                continue;
            }
            if culture == Culture::Chimefolk {
                // A dais of dry flagstones above the water, worn at its edge.
                let (fx, fy) = ((x - cx).div_euclid(18), (y - cy).div_euclid(12));
                let seam = (x - cx).rem_euclid(18) == 0 || (y - cy).rem_euclid(12) == 0;
                let tone = (hash(fx, fy, 9126) % 3) as usize;
                let ink = if d > 0.92 { stone[3] } else if seam { stone[0] } else { stone[1 + tone.min(1)] };
                if seam || d > 0.92 || super::super::ink::bayer(x, y) < 0.4 + tone as f32 * 0.08 {
                    cv.put(x, y, ink);
                }
                continue;
            }
            if culture == Culture::Lampwick {
                // A deck of boards over the water, its edge on posts.
                let seam = (y - cy).rem_euclid(5) == 0;
                let n = hash(x / 23 + (y - cy).div_euclid(5) * 7, 0, 9121) % 3;
                let edge = d > 0.9;
                cv.put(x, y, if seam || (x / 23 + (y - cy).div_euclid(5)) % 2 == 0 && x % 23 == 0 { 'n' } else if edge { 'b' } else { stone[1 + n as usize] });
                continue;
            }
            let ring = ((d.sqrt() * 9.0) as i32) % 2 == 0;
            let a = (dy.atan2(dx) * 8.0).floor() as i32;
            let seam = ((d.sqrt() * 9.0).fract() < 0.12) || (hash(a, (d.sqrt() * 9.0) as i32, 9122) % 4 == 0 && (dy.atan2(dx) * 8.0).fract() < 0.08);
            let ink = if seam { stone[0] } else if ring { stone[2] } else { stone[1] };
            // The root-hall's plaza is trodden earth: its rings show less.
            let sparse = if culture == Culture::Rootkin { hash(x, y, 9123) % 3 == 0 } else { (hash(x, y, 9123) % 5) != 0 };
            if sparse || seam {
                cv.put(x, y, ink);
            }
        }
    }
    // Ways out: to the stair, the well, the hall's door.
    let ways: [((i32, i32), (i32, i32)); 4] = [
        ((cx, cy + 60), (cx, 20 * TILE)),
        ((cx - 100, cy + 20), (12 * TILE, 17 * TILE)),
        ((cx, cy - 70), (cx, 10 * TILE + 8)),
        ((cx + 100, cy + 20), (v::LIFT.0 as i32 * TILE, v::LIFT.1 as i32 * TILE)),
    ];
    for ((x0, y0), (x1, y1)) in ways.into_iter().chain(boardwalks(culture)) {
        let steps = ((x1 - x0).abs().max((y1 - y0).abs())).max(1);
        let flat = (x1 - x0).abs() > (y1 - y0).abs();
        for s in 0..=steps {
            let (x, y) = (x0 + (x1 - x0) * s / steps, y0 + (y1 - y0) * s / steps);
            for o in -7..=7 {
                let (px_, py_) = if flat { (x, y + o) } else { (x + o, y) };
                if culture == Culture::Lampwick {
                    // Boardwalk planks across the water.
                    let along = if flat { px_ } else { py_ };
                    let ink = if along.rem_euclid(5) == 0 { 'n' } else if o.abs() == 7 { 'b' } else { ['I', 'B', 'P'][(hash(along / 5, 0, 9125) % 3) as usize] };
                    cv.put(px_, py_, ink);
                } else if hash(px_, py_, 9124) % 3 == 0 {
                    cv.put(px_, py_, stone[1 + (o.abs() < 4) as usize]);
                }
            }
        }
    }
    let _ = lk;
}

/// The Lampwick's boardwalks out to their houses' doors over the water.
fn boardwalks(culture: Culture) -> Vec<((i32, i32), (i32, i32))> {
    if culture != Culture::Lampwick {
        return Vec::new();
    }
    let mut walks = Vec::new();
    for &(c, r, w, h) in &v::HOUSES {
        let door = ((c * 2 + w) * TILE / 2, (r + h) * TILE + 4);
        let to = (door.0, 15 * TILE + 8);
        walks.push((door, to));
        walks.push(((door.0, to.1), (18 * TILE, to.1)));
    }
    walks.push(((12 * TILE, 18 * TILE), (12 * TILE, 15 * TILE)));
    walks
}

/// The lamp posts: each takes fire in its turn as hope grows.
/// A village whose beacons the party lit below keeps every lamp lit.
fn lamp_posts(cv: &mut Img, vl: &Village, lk: &Look, lights: &mut Vec<Light>, beacons: bool) {
    let lit = if beacons { v::LAMPS.len() } else { vl.lamps_lit() };
    for (i, &spot) in v::LAMPS.iter().enumerate() {
        let (x, y) = px(spot);
        let on = i < lit;
        mat::shadow(cv, ((x + 2) as f32, (y + 1) as f32), (4.0, 1.5), &all);
        let post = match vl.culture {
            Culture::Capfolk => 'B',
            Culture::Glimmerkin => 'u',
            Culture::Emberfolk => 'g',
            Culture::Lampwick | Culture::Roostfolk => 'P',
            Culture::Rootkin => 'b',
            Culture::Chimefolk => 'G',
            Culture::Saltkin => 'J',
        };
        cv.line(x, y, x, y - 20, post);
        cv.line(x + 1, y, x + 1, y - 20, 'n');
        match vl.culture {
            // A spore-lamp: a little glowing cap on a stalk.
            Culture::Capfolk => {
                cv.ellipse(x as f32 + 0.5, (y - 22) as f32, 5.0, 3.0, if on { lk.glow[2] } else { 'q' });
                cv.line(x - 4, y - 21, x + 5, y - 21, if on { lk.glow[3] } else { 'x' });
            }
            // A lodestone in a cage of iron.
            Culture::Glimmerkin => {
                cv.frame(x - 3, y - 27, 8, 8, 'G');
                cv.rect(x - 2, y - 26, 6, 6, if on { lk.glow[2] } else { 'S' });
                if on {
                    cv.put(x, y - 24, 'w');
                    cv.put(x + 1, y - 24, 'w');
                }
            }
            // A brazier bowl on its post.
            Culture::Emberfolk => {
                cv.rect(x - 4, y - 23, 10, 3, 'X');
                cv.line(x - 5, y - 24, x + 6, y - 24, 'g');
                if on {
                    cv.rect(x - 3, y - 26, 8, 2, '@');
                    cv.put(x, y - 27, '6');
                    cv.put(x + 2, y - 27, '5');
                }
            }
            // A lamplighter's lamp on a crook.
            Culture::Lampwick => {
                cv.line(x + 1, y - 20, x + 5, y - 22, post);
                let im = super::super::kit::lantern(on);
                cv.stamp(&im, x + 2, y - 22);
            }
            Culture::Roostfolk => roost::lamp_head(cv, x, y, on, lights),
            Culture::Rootkin => roots::lamp_head(cv, x, y, on, lights),
            Culture::Chimefolk => bells::lamp_head(cv, x, y, on, lights),
            Culture::Saltkin => salt::lamp_head(cv, x, y, on, lights),
        }
        if on {
            lights.push(Light { x: x as f32, y: (y - 22) as f32, r: 64.0, s: 0.5, fire: lk.warm });
        }
    }
}

/// The lift up to the light home: a winch on two posts, its rope going up
/// into the dark.
fn lift(cv: &mut Img, lk: &Look, lights: &mut Vec<Light>) {
    let (x, y) = px(v::LIFT);
    for side in [-14, 14] {
        cv.line(x + side, y + 6, x + side, y - 30, lk.rock[4]);
        cv.line(x + side + 1, y + 6, x + side + 1, y - 30, lk.rock[1]);
    }
    cv.line(x - 15, y - 30, x + 16, y - 30, lk.rock[5]);
    cv.ellipse(x as f32, (y - 30) as f32, 4.0, 4.0, 'P');
    cv.line(x, y - 30, x, 0, 'r');
    mat::boards(cv, (x - 12, y - 2, 25, 8), false, 9131);
    lights.push(fire(x, y - 8, 40.0, 0.25));
}

/// The dark the village holds before hope comes: every still pixel taken
/// further down its bank, less where a light reaches it, none once the
/// village shines. Lamplight (the signal inks) never dims.
fn gloom(cv: &mut Img, lights: &[Light], light: f32) {
    let depth = (1.0 - light) * 2.6;
    if depth <= 0.05 {
        return;
    }
    for y in 0..cv.h {
        for x in 0..cv.w {
            let Some(c) = cv.get(x, y).filter(|&c| c != BLACK) else {
                continue;
            };
            let (pxf, pyf) = (x as f32 + 0.5, y as f32 + 0.5);
            let mut local = 0.0;
            for li in lights {
                let d = (pxf - li.x).hypot(pyf - li.y);
                if d < li.r {
                    let t = 1.0 - d / li.r;
                    local += li.s * t * t;
                }
            }
            let steps = (depth - local * 4.0 + super::super::ink::bayer(x, y) * 0.9 - 0.45).round();
            if steps >= 1.0 {
                cv.set(x, y, step_down(c, steps as usize));
            }
        }
    }
}

/// The village's moving parts over its light: the folk, the star, the
/// air, the city's wishes.
pub(super) fn figures(cv: &mut Img, run: &Run) {
    let Some(vl) = village(run) else {
        return;
    };
    let lk = look(vl.culture);
    sky::glowworms(cv, run, vl, &lk);
    sky::ambient(cv, run, vl, &lk);
    let town = run.home.deep.town(vl.floor);
    gifts::all(cv, run, vl, town);
    folk::all(cv, run, vl);
    sky::star(cv, run, vl);
}
