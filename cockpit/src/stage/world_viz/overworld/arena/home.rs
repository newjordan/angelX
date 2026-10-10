//! The Undercroft, drawn: the company's cellar under the Delve's gate, in the
//! overworld's own hand. Its stations show what the party has built out:
//! the forge is cold until Tobbin's edge is bought, the rack fills, the
//! chapel's candles catch, Wren's map grows landings. As everywhere in the
//! realm, light means activity.

use super::super::ink::{Img, hash, text_width};
use super::super::kit::district_sign;
use super::super::kit::{self, RockKind, Tiles};
use super::{TILE, at, dusk, fire, flags, room_px, stand};
use crate::drive::together_realm::Spoil;
use crate::drive::together_shooter::home::{
    self, BUY_HOLD, LADDERS, SPOTS, STAIR_MOUTH, STAIRWELL, Station,
};
use crate::drive::together_shooter::{RoomKind, Run, Tile};
use crate::stage::world_viz::overworld::light::Light;

/// The Winding Stair's middle, in native pixels.
pub(super) fn stair_centre() -> (i32, i32) {
    let (c, r, w, h) = STAIRWELL;
    ((c * 2 + w) * TILE / 2, (r * 2 + h) * TILE / 2)
}

/// The cellar at rest: flags and the rug, the walls and their braziers,
/// each station as built, the plates, and the stair.
pub(super) fn scenery(run: &Run) -> Img {
    let room = run.room();
    let tick = run.tick as u32;
    let tiles = Tiles::get();
    let (pw, ph) = (room.cols as i32 * TILE, room.rows as i32 * TILE);
    let mut cv = Img::black(pw, ph);
    let mut lights: Vec<Light> = Vec::new();
    for y in 0..ph {
        for x in 0..pw {
            if room.tile(x / TILE, y / TILE) == Tile::Floor
                && let Some(ch) = flags(x, y)
            {
                cv.put(x, y, ch);
            }
        }
    }
    rug(&mut cv);
    for row in 0..room.rows as i32 {
        for col in 0..room.cols as i32 {
            if room.tile(col, row) != Tile::Wall {
                continue;
            }
            let (x, y) = (col * TILE, row * TILE);
            let corner = (col == 0 || col == room.cols as i32 - 1)
                && (row == 0 || row == room.rows as i32 - 1);
            if corner {
                let im = kit::brazier(tick / 4, (col + row) as u32);
                cv.stamp(&im, x + (TILE - im.w) / 2, y + TILE - im.h);
                lights.push(fire(x + TILE / 2, y + 4, 56.0, 0.32));
            } else {
                cv.stamp(tiles.rock(RockKind::Slate, hash(col, row, 7)), x, y);
            }
        }
    }
    banners(&mut cv);
    let levels = |s: Station| run.home.level(s);
    forge(&mut cv, &mut lights, levels(Station::Forge), tick);
    map_table(&mut cv, &mut lights, levels(Station::Map), tick);
    hearth(&mut cv, &mut lights, levels(Station::Hearth), tick);
    rack(&mut cv, levels(Station::Rack));
    chapel(&mut cv, &mut lights, levels(Station::Chapel), tick);
    if room.doors[3] {
        // Match the loop miners' real access cut through the chapel screen.
        // A home without the wing keeps its existing painting unchanged.
        for y in (room.rows as i32 / 2 - 1) * TILE..(room.rows as i32 / 2 + 1) * TILE {
            for x in TILE..2 * TILE {
                if let Some(ch) = flags(x, y) {
                    cv.put(x, y, ch);
                }
            }
        }
    }
    super::bounties::furniture(&mut cv);
    for spot in &SPOTS {
        plate(&mut cv, spot.plate, false, spot.station);
    }
    stair(&mut cv);
    dusk(&mut cv, &lights, ('x', 'I'));
    cv
}

/// Loop-built wing: furniture/collision comes from the saved shared Floor.
/// Do not accidentally draw the home shop's purchase plates in every room.
pub(super) fn settlement_scenery(run: &Run) -> Img {
    let room = run.room();
    let (pw, ph) = room_px(room);
    let mut cv = Img::black(pw, ph);
    let tiles = Tiles::get();
    for row in 0..room.rows as i32 {
        for col in 0..room.cols as i32 {
            let (x, y) = (col * TILE, row * TILE);
            match room.tile(col, row) {
                Tile::Floor | Tile::Door => {
                    for dy in 0..TILE {
                        for dx in 0..TILE {
                            if let Some(ch) = flags(x + dx, y + dy) {
                                cv.put(x + dx, y + dy, ch);
                            }
                        }
                    }
                }
                Tile::Block => {
                    let (base, trim) = match room.kind {
                        RoomKind::Workshop => ('g', 'Y'),
                        RoomKind::Quarters => ('B', 'c'),
                        _ => ('r', 'u'),
                    };
                    cv.rect(x + 1, y + 1, TILE - 2, TILE - 2, base);
                    cv.line(x + 1, y + 1, x + TILE - 2, y + 1, trim);
                }
                _ => cv.stamp(tiles.rock(RockKind::Slate, hash(col, row, 7)), x, y),
            }
        }
    }
    let sign = district_sign(room.kind.settlement_label());
    cv.stamp(&sign, (pw - sign.w) / 2, 0);
    dusk(&mut cv, &[fire(pw / 2, ph / 2, 110.0, 0.4)], ('x', 'I'));
    cv
}

/// Shared pixels are deliberately generic: no worker title, path or prose.
pub(super) fn research_exhibits(cv: &mut Img, run: &Run) {
    for m in run.settlement_exhibits.iter().filter(|m| m.room == run.at) {
        let (x, y) = (m.col as i32 * TILE, m.row as i32 * TILE);
        cv.rect(x + 2, y + 3, TILE - 4, TILE - 4, 'r');
        cv.rect(x + 3, y + 2, TILE - 6, 5, 'H');
        let sign = district_sign("LOCAL RESEARCH · E");
        cv.stamp(&sign, x + (TILE - sign.w) / 2, y - sign.h);
    }
}

/// The runner from the foot of the room up to the stair's mouth.
fn rug(cv: &mut Img) {
    let (mc, mr, mw, _) = STAIR_MOUTH;
    let (x0, x1) = (mc * TILE + 2, (mc + mw) * TILE - 2);
    let (y0, y1) = ((mr + 1) * TILE + 2, 14 * TILE);
    let cx = (x0 + x1) / 2;
    for y in y0..y1 {
        for x in x0..x1 {
            let edge = x == x0 || x == x1 - 1;
            let inner = x == x0 + 2 || x == x1 - 3;
            let lattice = ((x - cx).abs() + (y - y0).rem_euclid(14) - 7).abs() % 7 == 0;
            let ink = if edge {
                'p'
            } else if inner {
                'o'
            } else if lattice {
                'r'
            } else {
                'B'
            };
            cv.put(x, y, ink);
        }
    }
    for x in (x0..x1).step_by(2) {
        cv.put(x, y1, 'O');
        cv.put(x, y1 + 1, 'o');
    }
}

/// Two long banners of the company on the north wall, either side of the
/// stair: the realm's red, worked in timber inks so they never glow.
fn banners(cv: &mut Img) {
    for x0 in [124i32, 248] {
        for y in 2i32..30 {
            for x in x0..x0 + 10 {
                let tail = y > 24 && (x - x0 - 5).abs() < y - 24;
                if tail {
                    continue;
                }
                let ink = if x == x0 || x == x0 + 9 {
                    'b'
                } else if ((y - 12).abs() < 4 && (x - x0 - 5).abs() < 2)
                    || ((y - 12).abs() < 2 && (x - x0 - 5).abs() < 4)
                {
                    'O'
                } else {
                    'p'
                };
                cv.put(x, y, ink);
            }
        }
        cv.line(x0 - 1, 1, x0 + 10, 1, 'I');
    }
}

/// Tobbin's forge: a stone hood into the north wall, the firebox under it
/// (cold ash until the edge is bought), the anvil, and his blade rack.
fn forge(cv: &mut Img, lights: &mut Vec<Light>, level: u8, tick: u32) {
    // The hood, narrowing into the wall.
    for y in 0..20 {
        let inset = (20 - y) / 5;
        for x in 36 + inset..68 - inset {
            let joint = (y % 5 == 0) || ((x + (y / 5) * 3) % 8 == 0);
            cv.put(x, y, if joint { 'x' } else { 'S' });
        }
    }
    cv.rect(36, 20, 32, 26, 'S');
    cv.frame(36, 20, 32, 26, 'x');
    cv.line(37, 20, 66, 20, 'v');
    cv.rect(42, 28, 20, 15, 'k');
    let lit = level > 0;
    for y in 34..43 {
        for x in 43..61 {
            let h = hash(x, y + (tick / 3) as i32, 61);
            let ink = if lit {
                match h % 7 {
                    0 => '6',
                    1 | 2 => '@',
                    3 => '7',
                    4 => '8',
                    _ if y > 38 => '7',
                    _ => continue,
                }
            } else {
                match h % 5 {
                    0 => 'j',
                    1 => 'g',
                    _ if y > 39 => 'X',
                    _ => continue,
                }
            };
            cv.put(x, y, ink);
        }
    }
    if lit {
        lights.push(fire(52, 38, 60.0 + f32::from(level) * 6.0, 0.62));
    }
    // The anvil.
    let (ax, ay) = (72, 33);
    cv.line(ax, ay, ax + 18, ay, 'h');
    cv.line(ax - 3, ay + 1, ax + 19, ay + 1, 'J');
    cv.line(ax - 2, ay + 2, ax + 18, ay + 2, 'G');
    cv.rect(ax + 5, ay + 3, 9, 5, 'G');
    cv.rect(ax + 3, ay + 8, 13, 3, 'j');
    cv.line(ax + 2, ay + 11, ax + 16, ay + 11, 'g');
    if lit {
        // A blade on the anvil, still glowing.
        cv.line(ax + 4, ay - 1, ax + 14, ay - 1, '@');
    }
    let rack = kit::blade_rack(u32::from(level).clamp(1, 5));
    cv.stamp(&rack, 74, 4);
}

/// Wren's map table: a timber table under a parchment map of the Winding
/// Stair, a mark for every landing drawn, and her candles.
fn map_table(cv: &mut Img, lights: &mut Vec<Light>, level: u8, tick: u32) {
    let (x0, y0) = (290, 22);
    cv.rect(x0, y0, 60, 16, 'P');
    cv.frame(x0, y0, 60, 16, 'b');
    cv.line(x0 + 1, y0 + 16, x0 + 58, y0 + 16, 'I');
    for lx in [x0 + 2, x0 + 56] {
        cv.line(lx, y0 + 17, lx, y0 + 24, 'b');
    }
    // The parchment, a little askew.
    cv.rect(x0 + 6, y0 + 2, 46, 12, 'T');
    cv.line(x0 + 6, y0 + 13, x0 + 51, y0 + 13, 't');
    cv.line(x0 + 51, y0 + 2, x0 + 51, y0 + 13, 't');
    // The stair drawn on it: a spiral, and a mark for each floor reached.
    let (sx, sy) = (x0 + 14, y0 + 8);
    for k in 0..28 {
        let a = k as f32 * 0.42;
        let r = 1.0 + k as f32 * 0.17;
        cv.put(
            sx + (a.cos() * r) as i32,
            sy + (a.sin() * r * 0.8) as i32,
            'b',
        );
    }
    for landing in 0..=i32::from(level) {
        let lx = x0 + 24 + landing * 7;
        cv.line(sx + 5, sy, lx - 1, sy, 'r');
        cv.rect(lx, sy - 1, 3, 3, if landing == 0 { 'b' } else { '7' });
    }
    let candles = kit::candles(2, tick);
    cv.stamp(&candles, x0 + 1, y0 - 3);
    lights.push(fire(x0 + 3, y0 - 2, 40.0, 0.3));
    let lantern = kit::lantern(true);
    cv.stamp(&lantern, 354, 18);
    lights.push(fire(357, 21, 38.0, 0.3));
}

/// Old Blaise's hearth against the south wall: always lit, and bigger for
/// every rung of warmth bought.
fn hearth(cv: &mut Img, lights: &mut Vec<Light>, level: u8, tick: u32) {
    let (x0, y0) = (34, 176);
    cv.rect(x0, y0, 60, 32, 'S');
    for y in y0..y0 + 32 {
        for x in x0..x0 + 60 {
            if (y - y0) % 6 == 0 || (x + ((y - y0) / 6) * 5) % 10 == 0 {
                cv.put(x, y, 'x');
            }
        }
    }
    cv.line(x0, y0, x0 + 59, y0, 'v');
    cv.rect(x0 + 10, y0 + 6, 40, 22, 'k');
    // Logs, more of them as the hearth is built up.
    for i in 0..=i32::from(level).min(3) {
        let y = y0 + 24 - i * 2;
        cv.line(
            x0 + 14 + i * 2,
            y,
            x0 + 45 - i * 2,
            y,
            if i % 2 == 0 { 'b' } else { 'B' },
        );
    }
    let tall = 6 + i32::from(level) * 2;
    for y in y0 + 22 - tall..y0 + 23 {
        for x in x0 + 14..x0 + 46 {
            let rise = y0 + 22 - y;
            let width = (tall - rise).max(0) * 16 / tall.max(1);
            if (x - (x0 + 30)).abs() > width {
                continue;
            }
            let h = hash(x, y + (tick / 2) as i32, 62);
            if h.is_multiple_of(3) {
                continue;
            }
            let ink = match (rise * 4 / tall.max(1), h % 4) {
                (0, _) => '7',
                (1, 0) => '7',
                (1, _) => '@',
                (2, 0) => '@',
                (2, _) => '5',
                _ => '6',
            };
            cv.put(x, y, ink);
        }
    }
    lights.push(fire(x0 + 30, y0 + 14, 70.0 + f32::from(level) * 8.0, 0.7));
    // Blaise's stool.
    cv.rect(100, 200, 10, 3, 'r');
    cv.line(101, 203, 101, 206, 'b');
    cv.line(108, 203, 108, 206, 'b');
}

/// Tobbin's rack against the south wall: a plank shelf that fills with
/// potions, bombs and a scroll as its rungs are bought.
fn rack(cv: &mut Img, level: u8) {
    let (x0, y0) = (288, 180);
    cv.rect(x0, y0 + 18, 64, 4, 'I');
    cv.line(x0, y0 + 18, x0 + 63, y0 + 18, 'P');
    cv.rect(x0, y0 + 6, 64, 3, 'I');
    cv.line(x0, y0 + 6, x0 + 63, y0 + 6, 'P');
    for lx in [x0, x0 + 63] {
        cv.line(lx, y0 + 6, lx, y0 + 26, 'b');
    }
    let blades = kit::blade_rack(2);
    cv.stamp(&blades, x0 + 2, y0 - 6);
    let barrels = kit::barrels();
    cv.stamp(&barrels, x0 + 46, y0 + 9);
    let potion = |cv: &mut Img, x: i32, y: i32| {
        cv.put(x + 1, y, 'h');
        cv.rect(x, y + 1, 3, 3, '7');
        cv.put(x + 1, y + 1, '8');
    };
    if level >= 1 {
        for i in 0..3 {
            potion(cv, x0 + 20 + i * 5, y0 + 2);
        }
    }
    if level >= 2 {
        for i in 0..3 {
            let (bx, by) = (x0 + 6 + i * 7, y0 + 12);
            cv.ellipse(bx as f32 + 2.5, by as f32 + 3.0, 2.6, 2.6, 'K');
            cv.put(bx + 3, by, 'o');
            cv.put(bx + 4, by - 1, '6');
        }
    }
    if level >= 3 {
        for i in 0..3 {
            potion(cv, x0 + 36 + i * 5, y0 + 2);
        }
    }
    if level >= 4 {
        cv.rect(x0 + 27, y0 + 12, 12, 4, 'T');
        cv.line(x0 + 27, y0 + 12, x0 + 27, y0 + 15, 'O');
        cv.line(x0 + 38, y0 + 12, x0 + 38, y0 + 15, 'O');
        cv.put(x0 + 32, y0 + 13, '3');
    }
}

/// The Chapel of Bonds: an alcove in the west wall, its altar, its candles
/// lit and its two rings bright once the second wind is bought.
fn chapel(cv: &mut Img, lights: &mut Vec<Light>, level: u8, tick: u32) {
    let (x0, y0) = (17, 82);
    cv.rect(x0, y0, 14, 60, 'U');
    cv.frame(x0, y0, 14, 60, 'u');
    cv.line(x0 + 1, y0 + 1, x0 + 12, y0 + 1, 'V');
    let lit = level > 0;
    let (rx, ry) = (x0 + 7, y0 + 30);
    for (dx, ink) in [
        (-2, if lit { '2' } else { 'u' }),
        (2, if lit { '3' } else { 'v' }),
    ] {
        for step in 0..28 {
            let a = step as f32 / 28.0 * std::f32::consts::TAU;
            cv.put(
                rx + dx + (a.cos() * 3.6).round() as i32,
                ry + (a.sin() * 3.6).round() as i32,
                ink,
            );
        }
    }
    for cy in [y0 + 6, y0 + 52] {
        if lit {
            let candles = kit::candles(3, tick + cy as u32);
            cv.stamp(&candles, x0 + 2, cy);
            lights.push(fire(x0 + 7, cy, 30.0, 0.3));
        } else {
            for i in 0..3 {
                cv.line(x0 + 3 + i * 3, cy + 2, x0 + 3 + i * 3, cy + 3, 'c');
            }
        }
    }
    if lit {
        lights.push(fire(rx, ry, 22.0 + f32::from(level) * 8.0, 0.25));
    }
}

/// A station's engraved plate: a dark slab with a bronze edge and the
/// station's mark, bright while a knight stands on it.
pub(super) fn plate(cv: &mut Img, (c, r, w, h): (i32, i32, i32, i32), lit: bool, station: Station) {
    let (x, y, pw, ph) = (c * TILE + 1, r * TILE + 1, w * TILE - 2, h * TILE - 2);
    cv.rect(x, y, pw, ph, 'X');
    cv.frame(x, y, pw, ph, if lit { '5' } else { 'o' });
    for (cx, cy) in [
        (x, y),
        (x + pw - 1, y),
        (x, y + ph - 1),
        (x + pw - 1, y + ph - 1),
    ] {
        cv.put(cx, cy, if lit { '6' } else { 'O' });
    }
    let mark = mark(station);
    let ink = if lit { '6' } else { 'O' };
    let mark = mark.recolor(&[('O', ink)]);
    cv.stamp(&mark, x + (pw - mark.w) / 2, y + (ph - mark.h) / 2);
}

/// Each station's mark, engraved on its plate.
fn mark(station: Station) -> Img {
    Img::from_rows(match station {
        Station::Forge => &["OOOO..", "OOOOO.", "..O...", "..O...", "..O..."],
        Station::Hearth => &["..O..", ".OO..", ".OOO.", "OOOOO", ".OOO."],
        Station::Rack => &[".O.", ".O.", "OOO", "OOO", "OOO"],
        Station::Chapel => &[".O.O.", "O.O.O", "O.O.O", ".O.O."],
        Station::Map => &["OOOOO", "O.O.O", "OOOOO", "O.O.O", "OOOOO"],
        Station::Wheel => &[".OOO.", "O.O.O", "OOOOO", "O.O.O", ".OOO."],
        Station::Coffer => &["OOOOO", "O.O.O", "OOOOO", "O...O", "OOOOO"],
        Station::StallA | Station::StallB | Station::StallC => {
            &[".OOO.", "O.O.O", "O.OOO", "O.O.O", ".OOO."]
        }
        Station::LessonA | Station::LessonB => &["..O..", "..O..", "OOOOO", "O.O.O", "OOOOO"],
        Station::Wing => &[".OOO.", ".OOO.", "..O..", "..O..", ".OOO."],
        Station::TapA | Station::TapB | Station::TapC => {
            &["OOOO..", "O..OO.", "O..O.O", "O..OO.", "OOOO.."]
        }
        Station::SongA | Station::SongB | Station::SongC => {
            &["..OOO", "..O.O", "..O..", "OOO..", "OOO.."]
        }
        Station::Hire => &["....O", "...OO", "..OO.", ".O...", "O...."],
    })
}

/// The Winding Stair from above: a stone parapet round a spiral of steps
/// going down clockwise into the dark, open to the south where the party
/// walks in, a newel post in its middle.
fn stair(cv: &mut Img) {
    use std::f32::consts::{FRAC_PI_2, TAU};
    const RAMP: [char; 9] = ['V', 'v', 'U', 'u', 'S', 'x', 's', 'K', 'k'];
    const STEPS: f32 = 14.0;
    let (cx, cy) = stair_centre();
    for y in cy - 33..cy + 33 {
        for x in cx - 33..cx + 33 {
            let (dx, dy) = (x as f32 + 0.5 - cx as f32, y as f32 + 0.5 - cy as f32);
            let r = dx.hypot(dy);
            let theta = dy.atan2(dx);
            let off_mouth = (theta - FRAC_PI_2).sin().atan2((theta - FRAC_PI_2).cos());
            let mouth = off_mouth.abs() < 0.5;
            if (25.5..31.5).contains(&r) {
                if mouth {
                    continue;
                }
                let joint = ((theta + 7.0) / 0.45).fract() < 0.09 || r < 26.3;
                let ink = if joint {
                    'x'
                } else if r > 30.4 {
                    'S'
                } else if dy < -8.0 {
                    'V'
                } else if dy < 6.0 {
                    'v'
                } else {
                    'U'
                };
                cv.put(x, y, ink);
            } else if r < 25.5 {
                let ink = if r < 3.2 {
                    'i'
                } else if r < 4.4 {
                    'h'
                } else if r < 6.2 {
                    'k'
                } else {
                    // Clockwise from the mouth, each step a little lower: a
                    // lit lip at its front, a shadow under the next.
                    let twist = (r - 6.0) / 19.0 * 0.35;
                    let a = (theta - FRAC_PI_2 - twist).rem_euclid(TAU);
                    let along = a / TAU * STEPS;
                    let step = along.floor();
                    let depth = (step / STEPS * 9.0) as usize;
                    let lip = along.fract() < 0.13;
                    let shadow = along.fract() > 0.8;
                    let inner = usize::from(r < 8.5);
                    let shade = depth + inner + usize::from(shadow) * 2;
                    if lip && depth < 6 {
                        ['W', 'V', 'V', 'v', 'U', 'u'][depth]
                    } else {
                        *RAMP.get(shade).unwrap_or(&'k')
                    }
                };
                cv.put(x, y, ink);
            }
        }
    }
    // The parapet's ends either side of the mouth: two posts.
    for side in [-1.0f32, 1.0] {
        let a = FRAC_PI_2 + side * 0.5;
        let (px, py) = (
            cx + (a.cos() * 28.5).round() as i32,
            cy + (a.sin() * 28.5).round() as i32,
        );
        cv.rect(px - 2, py - 2, 5, 5, 'v');
        cv.frame(px - 2, py - 2, 5, 5, 'x');
        cv.put(px, py - 1, 'V');
    }
}

/// The live part of the cellar under the knights: the keepers, and the
/// plates knights stand on.
pub(super) fn figures(cv: &mut Img, run: &Run) {
    let tick = run.tick as u32;
    for spot in &SPOTS {
        let Some((who, (c, r))) = spot.keeper else {
            continue;
        };
        let (kx, ky) = (c * 2.0, r * 2.0);
        let (x, y) = at(kx, ky);
        let im = keeper(who, tick, near(run, kx, ky, 5.0));
        stand(cv, &im, x, y);
    }
    // Merlin by the stair, the realm's guide.
    {
        use crate::drive::together_shooter::merlin::MERLIN_AT;
        let (mx, my) = (MERLIN_AT.0 * 2.0, MERLIN_AT.1 * 2.0);
        let (x, y) = at(mx, my);
        stand(cv, &Img::from_rows(&MERLIN), x, y);
    }
    // Lady Tallow by the fire, once the hearth is warm enough for her.
    if run.home.level(Station::Hearth) >= crate::drive::together_shooter::cat::HEARTH_FOR_TALLOW {
        let (x, y) = at(8.4 * 2.0, 12.6 * 2.0);
        let im = super::sprites::tallow(false, false, false, tick);
        cv.stamp(&im, x - im.w / 2, y - im.h + 2);
    }
    // The stair's ring fills while a knight stands on it.
    if run.descending > 0 {
        let (cx, cy) = stair_centre();
        let k = run.descending as f32 / home::DESCEND_HOLD as f32;
        let n = 64;
        for i in 0..(n as f32 * k.min(1.0)) as i32 {
            let a = -std::f32::consts::FRAC_PI_2 + i as f32 / n as f32 * std::f32::consts::TAU;
            for r in [22.0f32, 23.0] {
                cv.put(
                    cx + (a.cos() * r) as i32,
                    cy + (a.sin() * r) as i32,
                    if i % 2 == 0 { '6' } else { '5' },
                );
            }
        }
    }
    for hero in run.players.values().filter(|h| h.hp > 0) {
        if let Some(station) = home::plate_at(run.room().kind, hero.x, hero.y)
            && let Some(spot) = SPOTS.iter().find(|s| s.station == station)
        {
            plate(cv, spot.plate, true, station);
        }
    }
}

fn near(run: &Run, x: f32, y: f32, reach: f32) -> bool {
    run.players
        .values()
        .any(|h| h.hp > 0 && (h.x - x).hypot(h.y - y) < reach)
}

/// Over everything: the ledger of the station a knight is reading, or
/// else where the stair goes when a knight stands at its mouth.
pub(super) fn boards(cv: &mut Img, run: &Run) {
    let reading = run
        .players
        .iter()
        .filter(|(_, h)| h.hp > 0)
        .find_map(|(&id, h)| {
            let station = home::plate_at(run.room().kind, h.x, h.y)?;
            Some((id, SPOTS.iter().find(|s| s.station == station)?))
        });
    if let Some((id, spot)) = reading {
        let buying = run.players.get(&id).map_or(0, |h| h.buying);
        let board = ledger_board(run, spot.station, buying);
        let (c, r, w, h) = spot.plate;
        let (px, py) = (c * TILE + w * TILE / 2, r * TILE + h * TILE / 2);
        let (bx, by) = if spot.station == Station::Chapel {
            (px + 14, py - board.h / 2)
        } else if r < 7 {
            (px - board.w / 2, py + 12)
        } else {
            (px - board.w / 2, py - 12 - board.h)
        };
        let bx = bx.clamp(2, cv.w - board.w - 2);
        cv.stamp(&board, bx, by);
        return;
    }
    // Wren's board, to a knight standing at it.
    let (fx, fy) = super::bounties::foot();
    if near(run, fx, fy, 3.4) {
        let sheet = super::bounties::sheet(run);
        let (c, r, w, _) = home::BOUNTY_BOARD;
        let x = (c * TILE + w * TILE / 2 - sheet.w / 2).clamp(2, cv.w - sheet.w - 2);
        let y = (r * TILE - 26 - sheet.h).max(2);
        cv.stamp(&sheet, x, y);
        return;
    }
    let (mc, mr, mw, _) = STAIR_MOUTH;
    let (mx, my) = ((mc as f32 + mw as f32 / 2.0) * 2.0, (mr as f32 + 0.5) * 2.0);
    if near(run, mx, my, 3.2) {
        let landing = run.home.landing();
        let mut lines = vec![(format!("DOWN TO FLOOR {landing}"), '9')];
        if run.home.landings().len() > 1 {
            lines.push(("TAP F: ANOTHER LANDING".to_string(), 'O'));
        }
        let board = board(&lines);
        let (cx, cy) = stair_centre();
        cv.stamp(&board, cx - board.w / 2, cy - 33 - board.h - 2);
    }
}

/// A keeper's figure: Tobbin hammers while someone is near; Wren's lantern
/// flickers; Blaise sits by his fire.
fn keeper(who: &str, tick: u32, busy: bool) -> Img {
    match who {
        "tobbin" if busy && (tick / 9).is_multiple_of(2) => Img::from_rows(&TOBBIN_UP),
        "tobbin" => Img::from_rows(&TOBBIN),
        "wren" => {
            let im = Img::from_rows(&WREN);
            if hash(tick as i32 / 4, 3, 44).is_multiple_of(3) {
                im.recolor(&[('6', '@'), ('5', '6')])
            } else {
                im
            }
        }
        _ => Img::from_rows(&BLAISE),
    }
}

/// A board of lines in the 5x7 hand: dark, timber-edged, like the realm's
/// signs.
pub(super) fn board(lines: &[(String, char)]) -> Img {
    let w = lines.iter().map(|(s, _)| text_width(s)).max().unwrap_or(0) + 8;
    let h = lines.len() as i32 * 9 + 5;
    let mut im = Img::new(w, h);
    im.rect(0, 0, w, h, 'K');
    im.frame(0, 0, w, h, 'B');
    for (i, (text, ink)) in lines.iter().enumerate() {
        let tw = text_width(text);
        im.text((w - tw) / 2, 3 + i as i32 * 9, text, *ink);
    }
    im
}

/// A station's ledger: its next rung, what it does, its price in spoils
/// (short ones in red), and the hold that buys it.
pub(super) fn ledger_board(run: &Run, station: Station, buying: u32) -> Img {
    let ladder = LADDERS
        .iter()
        .find(|l| l.station == station)
        .expect("a ladder per station");
    let Some((level, rung)) = run.home.next(station) else {
        let built = format!("{} {}", ladder.name, home::numeral(run.home.level(station)));
        return board(&[
            (built.to_uppercase(), '9'),
            ("BUILT IN FULL".to_string(), 'h'),
        ]);
    };
    let title = format!("{} {}", ladder.name, home::numeral(level)).to_uppercase();
    let says = rung.says.to_uppercase();
    let locked = run.home.deepest < rung.needs;
    let mut lines = vec![(title, '9'), (says, 'h')];
    if locked {
        lines.push((format!("REACH FLOOR {} FIRST", rung.needs), '7'));
    }
    lines.push((String::new(), 'h'));
    if !locked {
        lines.push((String::new(), 'h'));
    }
    let mut im = board(&lines);
    // The price, as each spoil's mark and its count.
    let row_y = 3 + 2 * 9 + if locked { 9 } else { 0 };
    let items: Vec<(Img, String, bool)> = rung
        .price
        .iter()
        .map(|&(spoil, n)| {
            (
                spoil_mark(spoil),
                n.to_string(),
                run.treasury.get(spoil) >= n,
            )
        })
        .collect();
    let width: i32 = items
        .iter()
        .map(|(mark, n, _)| mark.w + 2 + text_width(n) + 7)
        .sum::<i32>()
        - 7;
    let mut x = (im.w - width) / 2;
    for (mark, n, have) in &items {
        im.stamp(mark, x, row_y + (7 - mark.h) / 2);
        x += mark.w + 2;
        x = im.text(x, row_y, n, if *have { 'H' } else { '7' }) + 7;
    }
    if !locked {
        // HOLD F, and the bar it fills.
        let bar_y = row_y + 9;
        let label = "HOLD F";
        let lw = text_width(label);
        let bar_w = 40;
        let start = (im.w - (lw + 4 + bar_w)) / 2;
        let after = im.text(start, bar_y, label, 'O');
        im.frame(after + 4, bar_y + 1, bar_w, 5, 'b');
        let fill = (buying.min(BUY_HOLD) * (bar_w as u32 - 2) / BUY_HOLD) as i32;
        if fill > 0 {
            im.rect(after + 5, bar_y + 2, fill, 3, '5');
        }
    }
    im
}

/// A spoil's little mark for a price.
pub(super) fn spoil_mark(spoil: Spoil) -> Img {
    Img::from_rows(match spoil {
        Spoil::Gold => &[".444.", "45654", "46564", "45654", ".444."],
        Spoil::Bone => &["H...", "HiH.", ".HiH", "..iH", "...H"],
        Spoil::Wax => &["..6..", "..c..", ".c9c.", ".c9c.", ".$$$."],
        Spoil::Ore => &[".hJ..", "hJRJ.", "JRjJh", ".JjJ.", "..j.."],
        Spoil::Gem => &["..3..", ".323.", "32w23", ".323.", "..2.."],
        Spoil::Ember => &["..7..", ".7@7.", "7@6@7", ".7@7.", "..8.."],
        Spoil::Scale => &[".CYC.", "CYmYC", "CmAmC", ".CmC.", "..C.."],
        Spoil::Bond => &[".2..3.", "2.23.3", "2.23.3", ".2..3."],
    })
}

/// Merlin: a tall hat with a star on it, a beard to his belt, a robe of
/// stars, his staff's head glowing.
pub(super) const MERLIN: [&str; 19] = [
    "......Q.......3.",
    ".....QQ......323",
    ".....QQq......b.",
    "....QQ6Qq.....b.",
    "....QQQQq.....b.",
    "...QQQQQQq....b.",
    "..qQQQQQQQq...b.",
    "...OOKOKOO....b.",
    "...WOOrOOW....b.",
    "...WWWWWWW....b.",
    "..zWWWWWWWz...b.",
    ".zzWWWWWWWzzOOb.",
    ".zzzWWWWWzzz..b.",
    ".zz6zWWWzz6z..b.",
    "..zzzzWzzzzz..b.",
    "..zzz6zzzzzz..b.",
    "..zzzzzzz6zz..b.",
    "..qqqqqqqqqq..b.",
    "...nn....nn...b.",
];

pub(super) const TOBBIN: [&str; 17] = [
    "....tOOOo......",
    "...tOOOOOo.....",
    "..hOOOOOOOh....",
    "..hbbOOObbh....",
    "..hOKOOOKOh....",
    "...oOOrROo.....",
    "..iHHOOOHHi....",
    ".pHHHiHiHHHp...",
    "ppHHHHHHHHHpp..",
    "pp.HHHHHHH.pp..",
    "pp.bBBBBBb.pp..",
    "Oo.bBBBBBb.oOG.",
    "Oo.bBBtBBb.oJhG",
    "...bBBBBBb..JhG",
    "...bBBBBBb...b.",
    "...nnn.nnn...b.",
    "..Knnn.nnnK....",
];

const TOBBIN_UP: [&str; 17] = [
    "............GhG",
    "....tOOOo...hJG",
    "...tOOOOOo..Jb.",
    "..hOOOOOOOh.b..",
    "..hbbOOObbh.b..",
    "..hOKOOOKOhOo..",
    "...oOOrROo.Oo..",
    "..iHHOOOHHipp..",
    ".pHHHiHiHHHp...",
    "ppHHHHHHHHHp...",
    "pp.HHHHHHH.....",
    "pp.bBBBBBb.....",
    "Oo.bBBBBBb.....",
    "Oo.bBBtBBb.....",
    "...bBBBBBb.....",
    "...nnn.nnn.....",
    "..Knnn.nnnK....",
];

pub(super) const BLAISE: [&str; 16] = [
    ".....uuuu......",
    "....uUUUUu.....",
    "...uUvvvvUu....",
    "...uvOOOOvu....",
    "...uOKOOKOu..b.",
    "...uOOrOOOu..b.",
    "...uHOOOOHu..b.",
    "..uUHHHHHHUu.b.",
    "..uUHHHHHHUu.b.",
    ".uUUUHHHHUUUuO.",
    ".uUUUUHHUUUUuO.",
    "uUUUUUUUUUUUUb.",
    "uUUUUUUUUUUUUb.",
    ".uUUUUUUUUUUub.",
    ".uuuUUUUUUuuub.",
    "..KKK....KKK.b.",
];

pub(super) const WREN: [&str; 16] = [
    "....rrrr.....",
    "...rRRRRrr...",
    "..rRRRRRRRr..",
    "...pOOOOp....",
    "...OKOOKO....",
    "...oOOOOo....",
    "....oOOo.....",
    "...eEEEEe.o..",
    "..eEEEEEEeo..",
    "..eEtEEEEo...",
    "..OEEtEEE.a..",
    "...EEEtEE.656",
    "...ePPPPe.565",
    "....n..n...a.",
    "....n..n.....",
    "...KK..KK....",
];
