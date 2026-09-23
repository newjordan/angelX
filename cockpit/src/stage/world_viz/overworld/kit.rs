//! The sprite kit: trees and rocks, the building kit, the realm's landmarks,
//! figures, icons and small effects.
//!
//! Organic shapes (canopies, boulders, table tops) are shaded blobs lit from
//! the top-left and quantized onto a ramp with ordered dither; buildings are
//! composed from a wall/roof kit; figures are authored ink rows. Every sprite
//! is a pure function of its arguments.

use std::f32::consts::PI;
use std::sync::OnceLock;

use super::ink::{Img, bayer, hash, norm3, shade, vnoise};
use super::scene::{Soldier, SoldierState};

// ─── organic shapes ──────────────────────────────────────────────────────────

/// Ellipsoid blobs `(cx, cy, rx, ry)` shaded onto `ramp` (dark → light).
pub(crate) fn blob(
    w: i32,
    h: i32,
    blobs: &[(f32, f32, f32, f32)],
    ramp: &[char],
    tex: f32,
    seed: u32,
    grain: f32,
) -> Img {
    let light = norm3(-0.55, -0.75, 0.6);
    let mut im = Img::new(w, h);
    for y in 0..h {
        for x in 0..w {
            let (px, py) = (x as f32 + 0.5, y as f32 + 0.5);
            let mut best: Option<(f32, [f32; 3])> = None;
            for &(cx, cy, rx, ry) in blobs {
                let (dx, dy) = ((px - cx) / rx, (py - cy) / ry);
                let d2 = dx * dx + dy * dy;
                if d2 < 1.0 {
                    let z = (1.0 - d2).sqrt();
                    // Lower blobs sit in front.
                    let score = z + cy / h as f32 * 0.4;
                    if best.is_none_or(|(s, _)| score > s) {
                        best = Some((score, norm3(dx, dy, z)));
                    }
                }
            }
            if let Some((_, n)) = best {
                let mut i = (n[0] * light[0] + n[1] * light[1] + n[2] * light[2]) * 0.5 + 0.5;
                i += (vnoise(px / grain, py / grain, seed) - 0.5) * tex;
                i += (bayer(x, y) - 0.5) * 0.28;
                let idx = ((i * ramp.len() as f32).floor() as i32).clamp(0, ramp.len() as i32 - 1);
                im.put(x, y, ramp[idx as usize]);
            }
        }
    }
    im
}

fn tree(v: u32) -> Img {
    let j = |k: i32| (hash(v as i32, k, 7) % 100) as f32 / 100.0 - 0.5;
    let blobs = [
        (8.0 + j(1), 6.2 + j(2), 6.6, 5.9),
        (4.4 + j(3), 9.4 + j(4), 4.3, 3.9),
        (11.6 + j(5), 9.4 + j(6), 4.3, 3.9),
        (8.0, 10.2, 5.0, 3.5),
    ];
    let ramp: &[char] = if v % 3 == 2 {
        &['f', 'D', 'F', 'F', 'e', 'e', 'A']
    } else {
        &['f', 'D', 'F', 'F', 'E', 'l', 'L']
    };
    let mut canopy = blob(16, 15, &blobs, ramp, 0.6, v, 2.0);
    canopy.outline_inside('f');
    let mut im = Img::new(16, 16);
    im.rect(7, 12, 2, 4, 'B');
    im.put(8, 12, 'b');
    im.put(8, 13, 'b');
    im.stamp(&canopy, 0, 0);
    im
}

fn rock(v: u32, ramp: &[char]) -> Img {
    let j = |k: i32| (hash(v as i32, k, 8) % 100) as f32 / 100.0 - 0.5;
    let blobs = [
        (5.0 + j(1), 10.0 + j(2), 5.4, 5.2),
        (11.0 + j(3), 10.0 + j(4), 5.2, 5.4),
        (8.0 + j(5), 5.0 + j(6), 5.6, 4.6),
    ];
    let mut im = blob(16, 16, &blobs, ramp, 0.75, v, 1.3);
    for k in 0..2 {
        let x = 3 + (hash(v as i32, k, 9) % 10) as i32;
        let y = 5 + (hash(v as i32, k, 10) % 7) as i32;
        im.put(x, y, ramp[0]);
        im.put(x + 1, y + 1, ramp[0]);
    }
    im.outline_inside('k');
    im
}

fn dead_tree() -> Img {
    let mut im = Img::new(16, 16);
    im.line(7, 15, 7, 6, 'j');
    im.line(8, 15, 8, 8, 'G');
    im.line(7, 9, 3, 5, 'j');
    im.line(8, 8, 12, 3, 'G');
    im.line(4, 6, 4, 3, 'j');
    im.line(11, 4, 13, 4, 'j');
    im.line(7, 6, 6, 2, 'G');
    im.outline_outside('k');
    im
}

/// Rock ramps by region: timber-brown in the Mines, charcoal around the
/// Dragon Keep, slate everywhere else.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum RockKind {
    Slate,
    Ore,
    Char,
}

/// Terrain sprites, built once.
pub(crate) struct Tiles {
    pub(crate) trees: Vec<Img>,
    pub(crate) rocks: Vec<Vec<Img>>,
    pub(crate) dead: Img,
}

const ROCK_VARIANTS: u32 = 8;

impl Tiles {
    pub(crate) fn get() -> &'static Tiles {
        static TILES: OnceLock<Tiles> = OnceLock::new();
        TILES.get_or_init(|| Tiles {
            trees: (0..4).map(tree).collect(),
            rocks: [
                &['s', 'x', 'S', 'u', 'U'][..],
                &['n', 'b', 'I', 'B', 'P', 'r'][..],
                &['K', 'X', 'g', 'j', 'G'][..],
            ]
            .iter()
            .map(|ramp| (0..ROCK_VARIANTS).map(|v| rock(v * 7 + 3, ramp)).collect())
            .collect(),
            dead: dead_tree(),
        })
    }

    pub(crate) fn tree(&self, v: u32) -> &Img {
        &self.trees[(v % self.trees.len() as u32) as usize]
    }

    pub(crate) fn rock(&self, kind: RockKind, v: u32) -> &Img {
        let bank = match kind {
            RockKind::Slate => 0,
            RockKind::Ore => 1,
            RockKind::Char => 2,
        };
        &self.rocks[bank][(v % ROCK_VARIANTS) as usize]
    }
}

// ─── building kit ────────────────────────────────────────────────────────────

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Wall {
    Stone,
    Timber,
    Plaster,
    Dark,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Roof {
    Slate,
    Tile,
    Thatch,
}

fn roof_ramp(r: Roof) -> [char; 4] {
    match r {
        Roof::Slate => ['s', 'x', 'S', 'u'],
        Roof::Tile => ['I', 'p', 'r', 'R'],
        Roof::Thatch => ['B', 'r', 'o', 'O'],
    }
}

fn wall_px(kind: Wall, x: i32, y: i32, ww: i32) -> char {
    match kind {
        Wall::Stone | Wall::Dark => {
            let row = y / 4;
            let off = if row % 2 == 1 { 3 } else { 0 };
            let (lx, ly) = ((x + off) % 6, y % 4);
            let (mortar, brick, hi) = if kind == Wall::Dark {
                ('s', 'x', 'S')
            } else {
                ('S', 'u', 'U')
            };
            if ly == 3 || lx == 0 {
                mortar
            } else if ly == 0 && hash((x + off) / 6, row, 51) % 2 == 0 {
                hi
            } else {
                brick
            }
        }
        Wall::Timber => {
            let lx = x % 4;
            if lx == 3 {
                'b'
            } else if lx == 0 {
                'r'
            } else if hash(x, y / 3, 52) % 9 == 0 {
                'B'
            } else {
                'p'
            }
        }
        Wall::Plaster => {
            let beam = x < 2 || x >= ww - 2 || x == ww / 2 || x == ww / 2 - 1 || y < 2;
            if beam {
                if x == 0 || y == 0 { 'r' } else { 'B' }
            } else if hash(x, y, 53) % 9 == 0 {
                'o'
            } else {
                't'
            }
        }
    }
}

/// A house from the kit: front wall with door and windows under a roof.
#[derive(Clone, Copy, Debug)]
pub(crate) struct House {
    pub(crate) w: i32,
    pub(crate) h: i32,
    pub(crate) roof_h: i32,
    pub(crate) wall: Wall,
    pub(crate) roof: Roof,
    /// Firelight in the doorway — a forge at work.
    pub(crate) door_glow: bool,
    pub(crate) windows: i32,
    /// Lamplight in the windows — someone is home and working.
    pub(crate) lit: bool,
    pub(crate) chimney: bool,
}

pub(crate) fn house(hs: &House) -> Img {
    let top = if hs.chimney { 6 } else { 0 };
    let (w, h, rh) = (hs.w, hs.h + top, hs.roof_h);
    let mut im = Img::new(w, h);
    let wall_y0 = top + rh - 1;
    let wall_h = h - wall_y0;
    for y in wall_y0..h {
        for x in 1..w - 1 {
            let mut c = wall_px(hs.wall, x - 1, y - wall_y0, w - 2);
            if x >= w - 3 {
                c = match c {
                    't' => 'o',
                    'o' => 'r',
                    'U' => 'u',
                    'u' => 'S',
                    'p' => 'B',
                    other => other,
                };
            }
            im.put(x, y, c);
        }
    }
    for x in 1..w - 1 {
        for (dy, k) in [(0, 0.45), (1, 0.7)] {
            if let Some(c) = im.get(x, wall_y0 + dy) {
                im.set(x, wall_y0 + dy, shade(c, k));
            }
        }
    }
    let r = roof_ramp(hs.roof);
    for y in 0..rh {
        for x in 0..w {
            let (course, cy) = (y / 3, y % 3);
            let off = if course % 2 == 1 { 2 } else { 0 };
            let mut i: i32 = 2;
            if hs.roof == Roof::Thatch {
                if hash(x, 0, 41) % 3 == 0 {
                    i -= 1;
                }
                if cy == 2 && hash(x, course, 42) % 2 == 0 {
                    i -= 1;
                }
            } else {
                if cy == 2 {
                    i = 1;
                }
                if (x + off) % 4 == 0 && cy != 2 {
                    i = 1;
                }
            }
            if x < 3 {
                i += 1;
            }
            if x >= w - 3 {
                i -= 1;
            }
            if y < rh / 3 && x < w / 2 {
                i += 1;
            }
            if y == 0 {
                i = 3;
            }
            if y == rh - 1 {
                i = 0;
            }
            im.put(x, top + y, r[i.clamp(0, 3) as usize]);
        }
    }
    let (dw, dh) = (8, (wall_h - 3).min(10));
    let (dx, dy) = ((w - dw) / 2, h - dh);
    for y in dy..h {
        for x in dx..dx + dw {
            if y == dy && (x == dx || x == dx + dw - 1) {
                continue;
            }
            let frame = x == dx
                || x == dx + dw - 1
                || y == dy
                || (y == dy + 1 && (x == dx + 1 || x == dx + dw - 2));
            let c = if frame {
                'b'
            } else if hs.door_glow {
                if y <= dy + 2 {
                    'a'
                } else if y >= h - 3 {
                    '5'
                } else {
                    '@'
                }
            } else if y == dy + 1 || x == dx + 1 {
                'n'
            } else {
                'K'
            };
            im.put(x, y, c);
        }
    }
    let slots: &[i32] = match hs.windows {
        1 => &[dx - 7],
        2 => &[dx - 7, dx + dw + 3],
        3 => &[dx - 7, dx + dw + 3, 3],
        _ => &[],
    };
    let wy = wall_y0 + 3;
    for &sx in slots {
        if sx < 2 || sx + 4 > w - 2 {
            continue;
        }
        im.rect(sx, wy, 4, 4, 'b');
        if hs.lit {
            im.rect(sx + 1, wy + 1, 2, 2, '5');
            im.put(sx + 1, wy + 1, '6');
        } else {
            im.rect(sx + 1, wy + 1, 2, 2, 's');
        }
    }
    if hs.chimney {
        let cx = w - 9;
        for y in 0..top + 4 {
            for x in cx..cx + 4 {
                im.put(
                    x,
                    y,
                    if y == 0 {
                        'j'
                    } else if (y + x) % 3 == 0 {
                        'u'
                    } else {
                        'U'
                    },
                );
            }
        }
        im.put(cx + 3, 1, 'S');
    }
    im.outline_inside('k');
    im
}

#[derive(Clone, Copy, Debug)]
pub(crate) enum Top {
    Crenel,
    Cone(Roof),
    /// An observatory dome; `true` while the bench is lit.
    Dome(bool),
}

pub(crate) fn tower(w: i32, h: i32, top_h: i32, top: Top, wall: Wall) -> Img {
    let mut im = Img::new(w, h);
    for y in top_h..h {
        for x in 0..w {
            let mut c = wall_px(wall, x, y - top_h, w);
            if x >= w - 2 {
                c = match c {
                    'U' => 'u',
                    'u' => 'S',
                    'x' => 's',
                    't' => 'o',
                    other => other,
                };
            }
            im.put(x, y, c);
        }
    }
    match top {
        Top::Crenel => {
            let (floor, joint, hi) = if wall == Wall::Dark {
                ('x', 's', 'S')
            } else {
                ('u', 'S', 'U')
            };
            for y in 0..top_h {
                for x in 0..w {
                    let c = if y % 4 == 3 || (x + (y / 4) * 2) % 5 == 0 {
                        joint
                    } else {
                        floor
                    };
                    im.put(x, y, c);
                }
            }
            for x in 0..w {
                if (x / 3) % 2 == 0 {
                    im.put(x, 0, hi);
                    im.put(x, 1, floor);
                } else {
                    im.clear(x, 0);
                }
            }
            for y in top_h - 3..top_h {
                for x in 0..w {
                    im.put(x, y, if y == top_h - 3 { hi } else { floor });
                }
            }
            for x in 0..w {
                if (x / 3) % 2 == 1 {
                    im.put(x, top_h - 5, hi);
                    im.put(x, top_h - 4, floor);
                }
                im.put(x, top_h - 6, joint);
            }
        }
        Top::Cone(roof) => {
            let r = roof_ramp(roof);
            let span = top_h + 2;
            for y in 0..span {
                let hw = (y as f32 + 1.0) / span as f32 * (w as f32 / 2.0 + 1.0);
                for x in -1..w + 1 {
                    let d = x as f32 + 0.5 - w as f32 / 2.0;
                    if d.abs() < hw {
                        let mut i = if d < -hw * 0.35 {
                            3
                        } else if d < hw * 0.15 {
                            2
                        } else if d < hw * 0.65 {
                            1
                        } else {
                            0
                        };
                        if y % 3 == 2 && i > 0 {
                            i -= 1;
                        }
                        im.put(x, y, r[i]);
                    }
                }
            }
        }
        Top::Dome(active) => {
            let (cx, rx, ry) = (w as f32 / 2.0, w as f32 / 2.0, top_h as f32 + 2.0);
            let dome = blob(
                w,
                top_h + 2,
                &[(cx, top_h as f32 + 2.0, rx, ry)],
                &['s', 'x', 'S', 'Q', 'z'],
                0.1,
                5,
                3.0,
            );
            im.stamp(&dome, 0, 0);
            let slit = if active { '2' } else { 's' };
            for y in 1..top_h + 1 {
                im.put(w / 2 - 1, y, slit);
                im.put(w / 2, y, slit);
            }
            if active {
                im.put(w / 2 - 1, 2, '3');
            }
            im.line(w / 2 + 1, 5, w / 2 + 6, 0, 'O');
            im.line(w / 2 + 1, 6, w / 2 + 6, 1, 'o');
        }
    }
    im.outline_inside('k');
    im
}

fn arch(im: &mut Img, x: i32, y: i32, w: i32, h: i32, portcullis: bool) {
    for yy in y..y + h {
        for xx in x..x + w {
            let corner = yy == y && (xx < x + 2 || xx >= x + w - 2)
                || yy == y + 1 && (xx == x || xx == x + w - 1);
            if corner {
                continue;
            }
            let rim = xx == x
                || xx == x + w - 1
                || yy == y
                || (yy == y + 1 && (xx == x + 1 || xx == x + w - 2));
            let c = if rim {
                'k'
            } else if portcullis && ((xx - x) % 2 == 0 || (yy - y) % 3 == 0) && yy < y + h - 3 {
                'G'
            } else {
                'K'
            };
            im.put(xx, yy, c);
        }
    }
    for xx in x - 1..x + w + 1 {
        if !(xx >= x + 2 && xx < x + w - 2) {
            im.put(xx, y - 1, 'U');
        }
    }
}

/// A banner on a pole; `field` and `charge` are signal inks — a flag only
/// flies for something real (an earned tier, a repo's colours, a house).
pub(crate) fn flag(field: char, charge: char) -> Img {
    let mut im = Img::new(9, 12);
    im.line(1, 1, 1, 11, 'h');
    im.put(1, 0, '4');
    im.rect(2, 1, 6, 4, field);
    im.clear(7, 2);
    im.put(4, 2, charge);
    im.put(5, 2, charge);
    im
}

// ─── landmarks ───────────────────────────────────────────────────────────────

/// The keep; banners fly from tier 4.
pub(crate) fn keep(tier: u32) -> Img {
    let mut im = Img::new(48, 64);
    im.stamp(&tower(20, 30, 10, Top::Crenel, Wall::Stone), 14, 2);
    im.stamp(&tower(40, 44, 14, Top::Crenel, Wall::Stone), 4, 20);
    let corner = tower(13, 48, 9, Top::Crenel, Wall::Stone);
    im.stamp(&corner, 0, 16);
    im.stamp(&corner, 35, 16);
    arch(&mut im, 19, 50, 10, 14, true);
    for (x, y) in [(5, 32), (41, 32), (22, 15), (25, 15)] {
        im.rect(x, y, 1, 3, 'K');
    }
    if tier >= 4 {
        for (x, y) in [(3, 5), (38, 5), (22, 0)] {
            im.stamp(&flag('7', '5'), x, y);
        }
    }
    im
}

pub(crate) fn keep_turret() -> Img {
    tower(14, 44, 12, Top::Cone(Roof::Slate), Wall::Stone)
}

pub(crate) fn rookery() -> Img {
    let mut im = tower(14, 40, 14, Top::Cone(Roof::Slate), Wall::Stone);
    im.rect(6, 20, 2, 4, 'K');
    im.rect(6, 29, 2, 3, 'K');
    im
}

pub(crate) fn chapel(lit: bool) -> Img {
    let mut im = Img::new(32, 44);
    let body = house(&House {
        w: 32,
        h: 34,
        roof_h: 17,
        wall: Wall::Plaster,
        roof: Roof::Slate,
        door_glow: false,
        windows: 2,
        lit,
        chimney: false,
    });
    im.stamp(&body, 0, 10);
    im.stamp(
        &tower(8, 16, 7, Top::Cone(Roof::Slate), Wall::Plaster),
        12,
        0,
    );
    im.rect(15, 10, 2, 2, 'o');
    im.put(16, 10, 'O');
    im.ellipse(16.0, 31.0, 2.6, 2.6, 'b');
    im.ellipse(16.0, 31.0, 1.6, 1.6, if lit { '5' } else { 'Q' });
    im
}

/// The Round Table; `seated` are `(seat 0..6, robe ink)` for council members.
pub(crate) fn round_table(seated: &[(usize, char)]) -> Img {
    let mut im = Img::new(32, 32);
    for y in 0..32 {
        for x in 0..32 {
            let (dx, dy) = (
                (x as f32 + 0.5 - 16.0) / 15.5,
                (y as f32 + 0.5 - 17.0) / 13.5,
            );
            if dx * dx + dy * dy <= 1.0 && x % 5 != 0 && y % 4 != 0 {
                im.put(
                    x,
                    y,
                    if hash(x / 5, y / 4, 61) % 3 == 0 {
                        'g'
                    } else {
                        'X'
                    },
                );
            }
        }
    }
    let seats: Vec<(f32, f32)> = (0..6)
        .map(|i| {
            let a = i as f32 / 6.0 * 2.0 * PI - PI / 2.0;
            (16.0 + a.cos() * 11.5, 17.0 + a.sin() * 9.5)
        })
        .collect();
    for &(sx, sy) in &seats {
        im.ellipse(sx, sy, 2.2, 1.6, 'B');
    }
    im.ellipse(16.0, 18.5, 8.5, 6.5, 'b');
    im.stamp(
        &blob(
            32,
            32,
            &[(16.0, 17.0, 8.5, 6.5)],
            &['p', 'r', 'R', 'O'],
            0.15,
            3,
            3.0,
        ),
        0,
        0,
    );
    im.ellipse(16.0, 17.0, 3.0, 2.0, 'B');
    for &(seat, robe) in seated {
        let (sx, sy) = seats[seat % seats.len()];
        let mut fig = Img::new(7, 7);
        fig.ellipse(3.5, 3.5, 3.2, 3.2, robe);
        fig.put(2, 2, 'c');
        fig.outline_inside('k');
        im.stamp(&fig, sx as i32 - 3, sy as i32 - 5);
    }
    im
}

pub(crate) fn observatory(active: bool) -> Img {
    let mut im = tower(28, 38, 15, Top::Dome(active), Wall::Stone);
    arch(&mut im, 10, 30, 8, 8, false);
    im.rect(4, 22, 2, 4, 'K');
    im.rect(22, 22, 2, 4, 'K');
    im
}

pub(crate) fn gatehouse() -> Img {
    let mut im = Img::new(32, 42);
    im.stamp(&tower(22, 30, 9, Top::Crenel, Wall::Stone), 5, 12);
    let t = tower(10, 40, 9, Top::Crenel, Wall::Stone);
    im.stamp(&t, 0, 2);
    im.stamp(&t, 22, 2);
    arch(&mut im, 11, 28, 10, 14, true);
    im
}

pub(crate) fn well() -> Img {
    let mut im = Img::new(16, 18);
    im.line(2, 3, 2, 12, 'B');
    im.line(13, 3, 13, 12, 'B');
    im.line(1, 3, 14, 3, 'r');
    im.ellipse(8.0, 12.5, 6.4, 4.4, 'U');
    im.ellipse(8.0, 14.0, 6.4, 3.4, 'u');
    im.ellipse(8.0, 12.0, 3.9, 2.4, 'q');
    im.ellipse(8.0, 11.5, 3.2, 1.4, 's');
    im.line(8, 4, 8, 10, 'h');
    im.rect(7, 10, 2, 1, 'o');
    im.outline_inside('k');
    im
}

pub(crate) fn garden() -> Img {
    let mut im = Img::new(32, 16);
    for y in 1..15 {
        for x in 1..31 {
            let c = if y % 3 == 0 {
                'I'
            } else if y % 3 == 1 && hash(x, y, 71) % 3 == 0 {
                'L'
            } else if y % 3 == 1 && x % 2 == 0 {
                'l'
            } else {
                'b'
            };
            im.put(x, y, c);
        }
    }
    for x in (0..32).step_by(5) {
        im.put(x, 0, 'r');
        im.put(x, 15, 'r');
    }
    for y in (0..16).step_by(4) {
        im.put(0, y, 'r');
        im.put(31, y, 'r');
    }
    im
}

pub(crate) fn lantern(lit: bool) -> Img {
    let mut im = Img::new(7, 16);
    im.line(3, 6, 3, 15, 'B');
    im.rect(1, 1, 5, 5, 'k');
    if lit {
        im.rect(2, 2, 3, 3, '5');
        im.put(3, 3, '6');
    } else {
        im.rect(2, 2, 3, 3, 'g');
    }
    im.rect(1, 0, 5, 1, 'K');
    im
}

pub(crate) fn market_stall(v: u32) -> Img {
    let mut im = Img::new(16, 16);
    let stripes = if v % 2 == 0 { ('T', 'R') } else { ('T', 'u') };
    for y in 0..6 {
        for x in 0..16 {
            im.put(
                x,
                y,
                if (x / 2) % 2 == 0 {
                    stripes.0
                } else {
                    stripes.1
                },
            );
        }
    }
    im.rect(1, 6, 1, 10, 'B');
    im.rect(14, 6, 1, 10, 'B');
    im.rect(2, 9, 12, 4, 'r');
    for (i, g) in ['L', 'o', 'Y', 't', 'L'].iter().enumerate() {
        im.put(3 + i as i32 * 2, 9, *g);
    }
    im.outline_inside('k');
    im
}

pub(crate) fn docks() -> Img {
    let mut im = Img::new(48, 16);
    for y in 3..13 {
        for x in 0..48 {
            let c = if x % 4 == 3 {
                'b'
            } else if hash(x / 4, y, 81) % 5 == 0 {
                'r'
            } else {
                'R'
            };
            im.put(x, y, c);
        }
    }
    for x in [0, 15, 31] {
        im.rect(x, 12, 2, 4, 'B');
    }
    im.outline_inside('k');
    im
}

pub(crate) fn windmill(tick: u32) -> Img {
    let mut im = Img::new(32, 42);
    im.stamp(
        &tower(14, 28, 9, Top::Cone(Roof::Thatch), Wall::Plaster),
        9,
        14,
    );
    arch(&mut im, 13, 35, 6, 7, false);
    let (hx, hy) = (16.0f32, 17.0f32);
    let spin = tick as f32 * 0.15;
    for k in 0..4 {
        let a = spin + k as f32 * PI / 2.0 + PI / 4.0;
        im.line(
            hx as i32,
            hy as i32,
            (hx + a.cos() * 14.0) as i32,
            (hy + a.sin() * 14.0) as i32,
            'r',
        );
        for s in 4..14 {
            let (px, py) = (hx + a.cos() * s as f32, hy + a.sin() * s as f32);
            let (nx, ny) = (-a.sin(), a.cos());
            for t in 1..3 {
                im.put(
                    (px + nx * t as f32) as i32,
                    (py + ny * t as f32) as i32,
                    if s % 3 == 0 { 't' } else { 'T' },
                );
            }
        }
    }
    im.ellipse(16.0, 17.0, 1.6, 1.6, 'B');
    im
}

pub(crate) fn mine_mouth() -> Img {
    let mut im = blob(
        32,
        32,
        &[
            (9.0, 16.0, 10.0, 12.0),
            (23.0, 16.0, 10.0, 12.0),
            (16.0, 10.0, 12.0, 9.0),
        ],
        &['I', 'B', 'p', 'r', 'R'],
        0.5,
        11,
        2.0,
    );
    im.outline_inside('k');
    for y in 16..32 {
        for x in 10..22 {
            im.put(x, y, if y < 18 { 'n' } else { 'k' });
        }
    }
    im.rect(8, 15, 3, 17, 'B');
    im.rect(21, 15, 3, 17, 'B');
    im.rect(8, 13, 16, 3, 'r');
    im.line(12, 31, 12, 24, 'h');
    im.line(19, 31, 19, 24, 'h');
    im
}

pub(crate) fn ruins() -> Img {
    let mut im = Img::new(48, 46);
    im.stamp(&tower(30, 32, 10, Top::Crenel, Wall::Dark), 9, 0);
    im.stamp(&tower(48, 30, 10, Top::Crenel, Wall::Dark), 0, 16);
    for x in 0..48 {
        let cut = (vnoise(x as f32 / 4.0, 0.0, 91) * 14.0) as i32;
        for y in 0..cut.min(16 + cut / 2) {
            im.clear(x, y);
        }
    }
    for y in 0..46 {
        for x in 0..48 {
            if hash(x, y, 92) % 23 == 0 {
                im.clear(x, y);
            }
        }
    }
    arch(&mut im, 19, 32, 10, 14, false);
    im.outline_inside('k');
    im
}

/// The tilting yard: stands, raked sand, fence and the tilt barrier.
pub(crate) fn lists_ground() -> Img {
    let (w, h) = (192, 80);
    let mut im = Img::new(w, h);
    for x in 16..176 {
        for y in 0..12 {
            let c = if y < 4 {
                if (x / 4) % 2 == 0 { 'T' } else { 'R' }
            } else if y < 10 {
                match hash(x / 2, y / 2, 101) % 7 {
                    0 => 'c',
                    1 => 'u',
                    2 => 'R',
                    3 => 'h',
                    4 => 'o',
                    _ => 'b',
                }
            } else {
                'B'
            };
            im.put(x, y, c);
        }
    }
    for y in 16..76 {
        for x in 20..172 {
            if hash(x, y, 102) % 11 == 0 {
                im.put(x, y, 'I');
            }
        }
    }
    for x in 18..174 {
        im.put(x, 14, 'r');
        im.put(x, 77, 'r');
    }
    for y in 14..78 {
        im.put(18, y, 'r');
        im.put(173, y, 'r');
    }
    for x in (18..174).step_by(8) {
        im.rect(x, 13, 2, 3, 'B');
        im.rect(x, 76, 2, 3, 'B');
    }
    for x in 34..158 {
        im.put(x, 45, if (x / 6) % 2 == 0 { 'T' } else { 'R' });
        im.put(x, 46, 'B');
    }
    for x in (34..158).step_by(12) {
        im.rect(x, 44, 2, 5, 'b');
    }
    im
}

/// A pavilion for a house of the tournament.
pub(crate) fn tent(house: Heraldry) -> Img {
    let (c1, c2) = house.inks();
    let mut im = Img::new(24, 30);
    for y in 0..24 {
        for x in 0..24 {
            let (dx, dy) = (x as f32 + 0.5 - 12.0, y as f32 + 0.5 - 16.0);
            if (dx * dx) / 121.0 + (dy * dy) / 64.0 <= 1.0 {
                let a = dy.atan2(dx);
                let stripe = (((a + PI) / (2.0 * PI) * 12.0).floor() as i32) % 2;
                let dark = dx + dy > 3.0;
                im.put(
                    x,
                    y + 4,
                    match (stripe, dark) {
                        (0, false) => c1,
                        (0, true) => c2,
                        (_, false) => '9',
                        _ => '$',
                    },
                );
            }
        }
    }
    im.line(12, 0, 12, 19, 'h');
    im.rect(13, 0, 5, 3, c1);
    im.put(12, 19, '4');
    im.outline_inside('k');
    im
}

pub(crate) fn quintain(active: bool, tick: u32) -> Img {
    let mut im = Img::new(24, 28);
    im.rect(11, 8, 2, 18, 'r');
    im.line(8, 27, 11, 25, 'B');
    im.line(15, 27, 12, 25, 'B');
    im.rect(2, 8, 20, 2, 'R');
    im.rect(10, 6, 4, 3, 'o');
    im.rect(1, 4, 7, 9, 'T');
    im.rect(2, 5, 5, 7, 'R');
    im.rect(4, 5, 1, 7, 'T');
    im.rect(2, 7, 5, 1, 'T');
    im.line(20, 10, 20, 13, 'h');
    im.ellipse(20.0, 16.5, 3.0, 3.6, 'o');
    im.ellipse(19.3, 15.6, 1.4, 1.6, 'O');
    im.outline_outside('k');
    if active {
        for k in 0..9 {
            let a = PI * (1.05 + k as f32 * 0.1) + tick as f32 * 0.35;
            im.put(
                (12.0 + a.cos() * 11.0) as i32,
                (9.0 + a.sin() * 6.5) as i32,
                if k % 2 == 0 { '2' } else { '3' },
            );
        }
    }
    im
}

/// The standings board: `(name, score, heraldry)` rows.
pub(crate) fn board(rows: &[(String, u32, Heraldry)]) -> Img {
    let mut im = Img::new(54, 32);
    im.rect(4, 20, 2, 12, 'B');
    im.rect(48, 20, 2, 12, 'B');
    im.rect(0, 0, 54, 22, 'r');
    im.frame(0, 0, 54, 22, 'k');
    im.frame(1, 1, 52, 20, 'B');
    for (i, (name, score, house)) in rows.iter().take(2).enumerate() {
        let y = 3 + i as i32 * 9;
        im.rect(4, y + 2, 3, 3, house.inks().0);
        let name: String = name.chars().take(4).collect();
        im.text(9, y, &name.to_uppercase(), '9');
        let s = (*score).min(99).to_string();
        im.text(50 - super::ink::text_width(&s), y, &s, '9');
    }
    im
}

pub(crate) fn brazier(tick: u32, seed: u32) -> Img {
    let mut im = Img::new(8, 14);
    im.line(3, 8, 3, 13, 'g');
    im.line(4, 8, 4, 13, 'j');
    im.line(1, 13, 6, 13, 'g');
    im.rect(1, 6, 6, 2, 'j');
    im.put(0, 5, 'g');
    im.put(7, 5, 'g');
    let f = (hash(tick as i32, seed as i32, 99) % 3) as i32;
    im.rect(2, 3, 4, 3, '@');
    im.put(3 - f % 2, 1 + f / 2, '5');
    im.put(3, 2, '5');
    im.put(4, 2, '6');
    im.put(4 - f % 2, 0, '6');
    im
}

pub(crate) fn torch(tick: u32, seed: u32) -> Img {
    let mut im = Img::new(4, 9);
    im.line(1, 4, 1, 8, 'B');
    im.line(2, 4, 2, 8, 'r');
    let f = (hash(tick as i32, seed as i32, 98) % 2) as i32;
    im.rect(1, 2, 2, 2, '@');
    im.put(1 + f, 1, '5');
    im.put(2 - f, 0, '6');
    im
}

// ─── figures ─────────────────────────────────────────────────────────────────

/// The two houses of the lists.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Heraldry {
    Red,
    Blue,
}

impl Heraldry {
    /// `(field, shadow)` inks.
    pub(crate) fn inks(self) -> (char, char) {
        match self {
            Heraldry::Red => ('7', '8'),
            Heraldry::Blue => ('1', '0'),
        }
    }

    fn charge(self) -> char {
        match self {
            Heraldry::Red => '5',
            Heraldry::Blue => '9',
        }
    }
}

/// The hero: silver helm, red plume, the sun-star shield. No black inner
/// lines — on black paper they would read as holes.
pub(crate) fn knight() -> Img {
    Img::from_rows(&[
        ".......77.......",
        "......7778......",
        ".....iH778i.....",
        "....iHHhhhhJ....",
        "....iHhhhhhJ....",
        "....hggggggG....",
        "....hhJhhJhG....",
        "....JhhhhhhG....",
        "..iHh777777hHi..",
        ".4444477577hJ...",
        ".4757475557hJ...",
        ".4555477577GJ...",
        ".47574BB4BBG....",
        "..474.87778.....",
        "...4..hJ.hJ.....",
        "......GG.GG.....",
    ])
}

/// A mounted knight in profile, facing right, lance couched.
pub(crate) fn rider(house: Heraldry) -> Img {
    let (c1, c2) = house.inks();
    let star = house.charge();
    let mut im = Img::new(34, 26);
    im.line(5, 12, 1, 20, 'n');
    im.line(6, 12, 2, 21, 'n');
    im.line(9, 18, 8, 24, 'b');
    im.line(21, 18, 22, 24, 'b');
    im.ellipse(14.0, 15.0, 9.0, 4.6, 'P');
    im.line(11, 18, 11, 24, 'P');
    im.line(19, 18, 19, 24, 'P');
    for t in 0..8 {
        im.ellipse(20.0 + t as f32 * 0.8, 13.0 - t as f32 * 0.9, 2.4, 2.4, 'P');
    }
    im.ellipse(28.0, 6.5, 3.4, 2.3, 'P');
    im.line(24, 4, 27, 3, 'n');
    im.put(31, 7, 'b');
    im.put(27, 5, 'k');
    for x in 5..24 {
        let drop = if (x / 3) % 2 == 0 { 21 } else { 20 };
        for y in 11..drop {
            im.put(x, y, c1);
        }
        im.put(x, drop - 1, '4');
    }
    for (x, y) in [(14, 15), (13, 15), (14, 14), (14, 16)] {
        im.put(x, y, star);
    }
    im.rect(12, 5, 6, 7, 'h');
    im.rect(12, 7, 6, 4, c1);
    im.rect(12, 0, 6, 5, 'H');
    im.rect(14, 2, 4, 1, 'k');
    im.put(11, 0, c1);
    im.put(10, 1, c1);
    im.put(11, 1, c2);
    im.rect(9, 7, 5, 6, c1);
    im.rect(9, 7, 5, 1, '4');
    im.put(11, 9, star);
    im.put(11, 10, star);
    im.put(9, 12, 'k');
    im.put(13, 12, 'k');
    im.outline_inside('k');
    im.line(16, 9, 33, 7, 'T');
    im.line(16, 10, 33, 8, 'o');
    im.put(33, 7, 'W');
    im.put(32, 7, 'V');
    im
}

/// A seat of the muster: helm over a tabard in the stage's ink. A returned
/// seat raises its pennant, a failed seat goes dark red, a cut seat grey.
pub(crate) fn soldier(s: Soldier) -> Img {
    let tabard = match s.state {
        SoldierState::Failed => '8',
        SoldierState::Cut => 'G',
        _ => s.kind.ink(),
    };
    let rows = [
        "..iHi..", ".iHHhJ.", "..JhJ..", ".TTTTT.", "hTTTTTh", ".TTTTT.", "..T.T..", "..J.J..",
    ];
    let rows: Vec<String> = rows
        .iter()
        .map(|r| r.replace('T', &tabard.to_string()))
        .collect();
    let refs: Vec<&str> = rows.iter().map(String::as_str).collect();
    let mut body = Img::from_rows(&refs);
    if s.state == SoldierState::Cut {
        body = body.recolor(&[('i', 'j'), ('H', 'G'), ('h', 'g'), ('J', 'g')]);
    }
    let mut im = Img::new(10, 12);
    im.stamp(&body, 0, 4);
    if s.state == SoldierState::Returned {
        im.line(8, 0, 8, 11, 'h');
        im.rect(9, 0, 1, 3, '5');
    }
    im
}

/// A companion on the quest, robed in the party's ink.
pub(crate) fn squire(robe: char) -> Img {
    let rows = [
        "...hhh....",
        "..hHHhJ...",
        "..JhhhJ...",
        "...JhJ....",
        "..RRRRR...",
        ".hRRRRRh..",
        "..RRRRR...",
        "..RR.RR...",
        "..J...J...",
    ];
    let rows: Vec<String> = rows
        .iter()
        .map(|r| r.replace('R', &robe.to_string()))
        .collect();
    let refs: Vec<&str> = rows.iter().map(String::as_str).collect();
    Img::from_rows(&refs)
}

/// The dragon curled on the ruined keep; `breathing` while a gate judges.
/// Its scales are half signal red: a judging gate is live state and must
/// read at a glance.
pub(crate) fn dragon(breathing: bool, tick: u32) -> Img {
    let mut im = Img::new(72, 46);
    // wings behind, ribbed
    for (x0, dir) in [(24, -1), (42, 1)] {
        for i in 0..17 {
            let h = 20 - i;
            let ink = if i % 4 == 0 {
                '8'
            } else if i % 2 == 0 {
                'p'
            } else {
                'b'
            };
            im.line(x0 + dir * i, 20 - h / 2, x0 + dir * i, 20 + h / 3, ink);
        }
    }
    let body = blob(
        72,
        46,
        &[
            (31.0, 31.0, 15.0, 9.0),
            (44.0, 23.0, 7.0, 7.0),
            (51.0, 14.0, 7.5, 5.5),
            (15.0, 34.0, 8.0, 4.5),
        ],
        &['n', '8', 'p', '7', 'R', 'o'],
        0.3,
        17,
        3.0,
    );
    im.stamp(&body, 0, 0);
    for t in 0..18 {
        let a = t as f32 * 0.3;
        im.ellipse(
            15.0 - a.cos() * 10.0,
            39.0 - a.sin() * 4.0,
            2.0,
            1.7,
            if t % 2 == 0 { '7' } else { 'p' },
        );
    }
    im.outline_inside('n');
    // horns, eye and jaw
    im.line(49, 9, 46, 5, 'h');
    im.line(52, 9, 51, 4, 'h');
    im.rect(54, 12, 2, 2, '6');
    im.put(55, 12, '5');
    im.line(55, 17, 58, 17, 'n');
    if breathing {
        for k in 0..30 {
            let spread = k / 4;
            let jitter = (hash(k, tick as i32, 231) % 5) as i32 - 2;
            let (x, y) = (59 + k * 2 / 3, 16 + jitter * spread / 3);
            let ink = if k < 8 {
                '6'
            } else if k < 18 {
                '5'
            } else {
                '@'
            };
            im.put(x, y, ink);
            im.put(x, y + 1, if k < 12 { '5' } else { '@' });
            if spread > 1 {
                im.put(x, y + spread / 2 + 1, '@');
                im.put(x, y - spread / 2, 'a');
            }
        }
    }
    im
}

/// A treasure chest: gold-banded and shut, or open and empty.
pub(crate) fn chest(full: bool) -> Img {
    if full {
        Img::from_rows(&[
            ".kkkkkkkkkk.",
            "k4rRRRRRRr4k",
            "k4RRRRRRRR4k",
            "k4444664444k",
            "k4BBBBBBBB4k",
            "k4BBB55BBB4k",
            "k4BBBBBBBB4k",
            ".kkkkkkkkkk.",
        ])
    } else {
        Img::from_rows(&[
            ".kkkkkkkkkk.",
            "krRRRRRRRRrk",
            ".kkkkkkkkkk.",
            "kBnnnnnnnnBk",
            "kBnnnnnnnnBk",
            "kBBBBBBBBBBk",
            "kBBBBBBBBBBk",
            ".kkkkkkkkkk.",
        ])
    }
}

/// A will-o'-wisp over the swamp.
pub(crate) fn wisp() -> Img {
    Img::from_rows(&["...2...", "..232..", ".23w32.", "..232..", "...2..."])
}

/// A name plaque: readable text on a dark board.
pub(crate) fn plaque(name: &str) -> Img {
    let text: String = name.to_uppercase().chars().take(7).collect();
    let w = super::ink::text_width(&text) + 6;
    let mut im = Img::new(w, 11);
    im.rect(0, 0, w, 11, 'K');
    im.frame(0, 0, w, 11, 'B');
    im.text(3, 2, &text, '9');
    im
}

pub(crate) fn dust(k: i32) -> Img {
    let mut im = Img::new(5, 4);
    let r = 1.0 + (3 - k) as f32 * 0.5;
    im.ellipse(2.5, 2.0, r, r * 0.8, if k == 1 { 'P' } else { 'I' });
    im
}

pub(crate) fn smoke(tick: u32) -> Img {
    let mut im = Img::new(20, 20);
    for i in 0..4 {
        let r = 1.4 + i as f32 * 0.7;
        let (x, y) = (
            4.0 + i as f32 * 3.2 + (tick as f32 * 0.7 + i as f32).sin(),
            16.0 - i as f32 * 4.2,
        );
        im.ellipse(x, y, r, r * 0.85, if i < 2 { 'i' } else { 'h' });
        im.ellipse(x - 0.4, y - 0.4, r * 0.5, r * 0.4, 'H');
    }
    im
}

// ─── icons & cues ────────────────────────────────────────────────────────────

/// What the knight is doing, as a HUD item and an emote.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub(crate) enum Tool {
    Hammer,
    Book,
    Quill,
    Candle,
    Lens,
    Scroll,
    Sword,
}

pub(crate) fn icon(tool: Tool) -> Img {
    match tool {
        Tool::Hammer => Img::from_rows(&[
            "kkkkkkk", "kHhhhJk", "kGJJJGk", "kkkBkkk", "..kBk..", "..kBk..", "..kBk..", "..kBk..",
            "..kkk..",
        ]),
        Tool::Book => Img::from_rows(&[
            "kkkkkkkk", "k7cccc9k", "k7c$$c9k", "k7cccc9k", "k7c$$c9k", "k7cccc9k", "k8kkkkkk",
            "kk......",
        ]),
        Tool::Quill => Img::from_rows(&[
            "......kk", ".....k9k", "....k9ck", "...k9ck.", "..k9ck..", ".kcck...", ".kkk....",
            "k.......",
        ]),
        Tool::Candle => Img::from_rows(&[
            "...5...", "..565..", "...@...", "..k9k..", "..k9k..", "..k9k..", ".kkkkk.", "kkkkkkk",
        ]),
        Tool::Lens => Img::from_rows(&[
            ".kkkk...", "k3223k..", "k2332k..", "k2222k..", ".kkkkk..", "....kok.", ".....kok",
            "......kk",
        ]),
        Tool::Scroll => {
            Img::from_rows(&["kkkkkk", "k9$$9k", ".k$$k.", ".k$$k.", "k9$$9k", "kkkkkk"])
        }
        Tool::Sword => Img::from_rows(&[
            "...k...", "..kHk..", "..kHk..", "..kHk..", "..kHk..", "..kHk..", "..kHk..", "..kHk..",
            "kkkkkkk", "k44444k", "kkkBkkk", "..kBk..", "..k4k..", "..kkk..",
        ]),
    }
}

/// A speech bubble holding an icon, tail at the bottom left.
pub(crate) fn bubble(tool: Tool) -> Img {
    let icon = icon(tool);
    let mut im = Img::new(16, 17);
    im.rect(1, 0, 14, 13, '9');
    im.rect(0, 1, 16, 11, '9');
    im.rect(4, 13, 3, 1, '9');
    im.rect(4, 14, 2, 1, '9');
    im.put(4, 15, '9');
    im.outline_outside('k');
    im.stamp(&icon, (16 - icon.w) / 2, (13 - icon.h) / 2 + 1);
    im
}

/// Beacon brackets and a bobbing arrow over a live place (world pixels).
pub(crate) fn beacon(cv: &mut Img, (x, y, w, h): (i32, i32, i32, i32), tick: u32) {
    let (x0, y0, x1, y1) = (x - 3, y - 3, x + w + 2, y + h + 2);
    for (cx, cy, sx, sy) in [
        (x0, y0, 1, 1),
        (x1, y0, -1, 1),
        (x0, y1, 1, -1),
        (x1, y1, -1, -1),
    ] {
        for k in 0..5 {
            cv.put(cx + sx * k, cy, '2');
            cv.put(cx, cy + sy * k, '2');
        }
        cv.put(cx + sx, cy + sy, '3');
    }
    let ax = x + w / 2;
    let bob = ((tick / 2) % 2) as i32;
    for (dy, half) in [(0, 3), (1, 2), (2, 1), (3, 0)] {
        for dx in -half..=half {
            cv.put(ax + dx, y0 - 8 + dy - bob, if dy == 0 { '3' } else { '2' });
        }
    }
}
