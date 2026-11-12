//! The ways goods travel, and what travels on them: the laid line itself,
//! a tile at a time, per kind; and the ox cart, the wagon on its rails, the
//! ropeway's bucket, the chute's sled, the river barge, each carrying its
//! load drawn from [`Load`] so stone, lime and timber read differently.

use std::collections::HashMap;
use std::sync::{Arc, Mutex, OnceLock};

use super::{FRAMES, Load, paint};
use crate::render::overworld::ink::{Img, hash};

/// The kinds of line (`LINES` in the plan catalogue).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Kind {
    /// An ox cart on a cart road.
    Cart,
    /// A wagon on a wooden tramway.
    Tram,
    /// Buckets on a cable between pylons.
    Rope,
    /// A timber chute, downhill only.
    Chute,
    /// A barge on the river.
    Barge,
    /// A string of pack mules on a trodden way.
    Mule,
}

impl Kind {
    pub const ALL: [Kind; 6] = [Kind::Cart, Kind::Tram, Kind::Rope, Kind::Chute, Kind::Barge, Kind::Mule];

    pub fn from_id(id: &str) -> Option<Kind> {
        Some(match id {
            "cart" => Kind::Cart,
            "tram" => Kind::Tram,
            "rope" => Kind::Rope,
            "chute" => Kind::Chute,
            "barge" => Kind::Barge,
            "mule" => Kind::Mule,
            _ => return None,
        })
    }

    pub fn id(self) -> &'static str {
        match self {
            Kind::Cart => "cart",
            Kind::Tram => "tram",
            Kind::Rope => "rope",
            Kind::Chute => "chute",
            Kind::Barge => "barge",
            Kind::Mule => "mule",
        }
    }
}

/// Which way a thing on a line faces.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Dir {
    East,
    West,
    North,
    South,
}

/// The neighbours a laid tile joins: `N | E | S | W` bits.
pub const N: u8 = 1;
pub const E: u8 = 2;
pub const S: u8 = 4;
pub const W: u8 = 8;

/// A tile of laid line is sixteen wide; a rope's pylon or a chute's trestle
/// stands over it, so the picture is taller (its foot on the bottom row).
pub const TILE_W: i32 = 16;
pub fn tile_h(kind: Kind) -> i32 {
    match kind {
        Kind::Rope => 36,
        Kind::Chute => 22,
        _ => 16,
    }
}

type Made<K> = OnceLock<Mutex<HashMap<K, Arc<Img>>>>;

fn once<K: std::hash::Hash + Eq + Copy>(memory: &'static Made<K>, key: K, make: impl FnOnce() -> Img) -> Arc<Img> {
    let memory = memory.get_or_init(Default::default);
    if let Some(hit) = memory.lock().ok().and_then(|m| m.get(&key).cloned()) {
        return hit;
    }
    let made = Arc::new(make());
    if let Ok(mut m) = memory.lock() {
        m.insert(key, made.clone());
    }
    made
}

/// One tile of laid line of `kind` joining the neighbours `links`; a rope's
/// tile with `post` stands its pylon. `frame` animates what moves in it
/// (water, a cable's shiver).
pub fn line_flat(kind: Kind, links: u8, post: bool, frame: u32) -> Arc<Img> {
    static MADE: Made<(Kind, u8, bool, u32)> = OnceLock::new();
    let frame = if matches!(kind, Kind::Barge) { frame % FRAMES } else { 0 };
    once(&MADE, (kind, links & 15, post, frame), || draw_line(kind, links & 15, post, frame))
}

/// A thing running on a line: `kind`'s carrier facing `dir`, `load` on it,
/// step `frame` of its eight-step motion (wheels turning, legs walking).
pub fn vehicle_flat(kind: Kind, dir: Dir, load: Option<Load>, frame: u32) -> Arc<Img> {
    static MADE: Made<(Kind, Dir, Option<Load>, u32)> = OnceLock::new();
    once(&MADE, (kind, dir, load, frame % FRAMES), || draw_vehicle(kind, dir, load, frame % FRAMES))
}

/// The arms of a laid tile as rectangles `(x, y, w, h)` of width `wide`
/// about the tile's middle, one per link, and the middle square.
fn arms(links: u8, wide: i32, h: i32) -> Vec<(i32, i32, i32, i32)> {
    let (c, base) = (TILE_W / 2, h - TILE_W / 2);
    let half = wide / 2;
    let mut v = vec![(c - half, base - half, wide, wide)];
    if links & N != 0 {
        v.push((c - half, base - TILE_W / 2, wide, TILE_W / 2));
    }
    if links & S != 0 {
        v.push((c - half, base, wide, TILE_W / 2));
    }
    if links & E != 0 {
        v.push((c, base - half, TILE_W / 2, wide));
    }
    if links & W != 0 {
        v.push((0, base - half, TILE_W / 2, wide));
    }
    v
}

fn inside(rects: &[(i32, i32, i32, i32)], x: i32, y: i32) -> bool {
    rects.iter().any(|&(rx, ry, rw, rh)| x >= rx && y >= ry && x < rx + rw && y < ry + rh)
}

fn draw_line(kind: Kind, links: u8, post: bool, frame: u32) -> Img {
    let h = tile_h(kind);
    let mut im = Img::new(TILE_W, h);
    let base = h - TILE_W / 2;
    let along_x = links & (E | W) != 0 || links == 0;
    match kind {
        Kind::Cart => {
            // A road of packed earth, two ruts worn into it, gravel in its crown.
            let road = arms(links, 9, h);
            for y in 0..h {
                for x in 0..TILE_W {
                    if inside(&road, x, y) {
                        let n = hash(x, y, 31) % 9;
                        let edge = !inside(&road, x - 1, y) || !inside(&road, x + 1, y) || !inside(&road, x, y - 1) || !inside(&road, x, y + 1);
                        im.put(x, y, if edge { 'B' } else if n == 0 { 'h' } else if n == 1 { 'P' } else { 'o' });
                    }
                }
            }
            let ruts = arms(links, 6, h);
            for y in 0..h {
                for x in 0..TILE_W {
                    if inside(&ruts, x, y) && !inside(&arms(links, 2, h), x, y) {
                        let (cx, cy) = (x - TILE_W / 2, y - base);
                        let on = if along_x { cy.abs() == 2 } else { cx.abs() == 2 };
                        if on {
                            im.put(x, y, 'B');
                        }
                    }
                }
            }
        }
        Kind::Tram => {
            // Sleepers across, two iron rails along: wooden tramway.
            let bed = arms(links, 10, h);
            for y in 0..h {
                for x in 0..TILE_W {
                    if !inside(&bed, x, y) {
                        continue;
                    }
                    let (cx, cy) = (x - TILE_W / 2, y - base);
                    let (u, v) = if along_x { (cx, cy) } else { (cy, cx) };
                    let sleeper = (u + 16).rem_euclid(4) < 2;
                    let rail = v == -3 || v == 2;
                    let ch = if rail {
                        if (u + 16).rem_euclid(4) == 0 { 'W' } else { 'v' }
                    } else if sleeper {
                        'O'
                    } else {
                        'B'
                    };
                    im.put(x, y, ch);
                }
            }
        }
        Kind::Chute => {
            // A trough of planks on trestles, sloping: stilts down to the ground.
            let lift = 6;
            let trough = arms(links, 8, h - 6);
            for y in 0..h {
                for x in 0..TILE_W {
                    if inside(&trough, x, y) {
                        let (cx, cy) = (x - TILE_W / 2, y - (base - lift));
                        let v = if along_x { cy } else { cx };
                        im.put(x, y + 0, if v == -4 || v == 3 { 'O' } else if v == -3 { 'o' } else if v == 2 { 'B' } else { 'b' });
                    }
                }
            }
            // The legs.
            let (cx, cy) = (TILE_W / 2, base);
            for (lx, ly) in [(cx - 3, cy - lift + 4), (cx + 2, cy - lift + 4)] {
                for y in ly..h - 1 {
                    im.put(lx, y, 'o');
                    im.put(lx + 1, y, 'B');
                }
            }
            let brace = if along_x { 4 } else { 3 };
            im.line(cx - 3, h - 2, cx + 3, cy - lift + 4, 'B');
            let _ = brace;
        }
        Kind::Rope => {
            // A cable through the tile's head height; a pylon where it is anchored.
            let top = 8;
            let cable = |im: &mut Img| {
                let (cx, cy) = (TILE_W / 2, top);
                let mut put = |x: i32, y: i32| {
                    im.put(x, y, 'U');
                    im.put(x, y + 1, 'u');
                };
                if links & W != 0 { for x in 0..cx { put(x, cy + x / 6 - 1) } }
                if links & E != 0 { for x in cx..TILE_W { put(x, cy + (TILE_W - x) / 6 - 1) } }
                if links & N != 0 { for y in 0..cy { put(cx, y) } }
                if links & S != 0 { for y in cy..h { put(cx, y) } }
                put(cx, cy);
            };
            // The pylon first, so the cable crosses its arm.
            if post {
                for y in top..h - 2 {
                    im.put(7, y, 'o');
                    im.put(8, y, 'O');
                    im.put(9, y, 'B');
                }
                im.line(3, top + 3, 7, top + 9, 'B');
                im.line(12, top + 3, 9, top + 9, 'B');
                im.rect(4, top - 1, 9, 2, 'O');
                im.rect(4, top, 9, 1, 'B');
                im.rect(2, h - 4, 12, 3, 'G');
                im.rect(2, h - 4, 12, 1, 'h');
                paint::wheel(&mut im, 8.0, top as f32, 2.5, 0.4, 4, false, ('V', 'U', 'v'));
            }
            cable(&mut im);
        }
        Kind::Barge => {
            // The river lane: a ribbon of moving water, reeds on its banks, a mooring post now and then.
            let water = arms(links, 12, h);
            for y in 0..h {
                for x in 0..TILE_W {
                    if inside(&water, x, y) {
                        let (u, v) = (x, y);
                        let shift = (frame % FRAMES) as i32;
                        let band = (v + hash((u + shift).div_euclid(8), v, 17) as i32 % 2) % 4;
                        let g = (u + shift).rem_euclid(8);
                        let ch = match (band, g) {
                            (0, 2..=5) => '3',
                            (3, _) => 'q',
                            _ => 'Q',
                        };
                        im.put(x, y, ch);
                    }
                }
            }
            for y in 0..h {
                for x in 0..TILE_W {
                    if inside(&water, x, y) && (!inside(&water, x - 1, y) || !inside(&water, x + 1, y) || !inside(&water, x, y - 1) || !inside(&water, x, y + 1)) {
                        im.put(x, y, 'u');
                    }
                }
            }
            if post {
                paint::post(&mut im, 2, base - 5, base + 1);
                im.rect(1, base - 6, 4, 1, 'O');
            }
        }
        Kind::Mule => {
            // A trodden way: no road, no ruts. A narrow strip of pale scuffed earth through the grass,
            // its edge ragged, hoof-prints along it and a tuft or a stone either side.
            let way = arms(links, 5, h);
            for y in 0..h {
                for x in 0..TILE_W {
                    if !inside(&way, x, y) {
                        continue;
                    }
                    let n = hash(x, y, 53) % 11;
                    let edge = !inside(&way, x - 1, y) || !inside(&way, x + 1, y) || !inside(&way, x, y - 1) || !inside(&way, x, y + 1);
                    let ch = if edge && n < 4 { 'P' } else if n == 0 { 'B' } else if n < 3 { 'o' } else { 'O' };
                    im.put(x, y, ch);
                }
            }
            // Hoof-prints, two dark flecks side by side, every few pixels along the way's middle.
            for k in 0..4 {
                let (u, side) = (2 + k * 4, if k % 2 == 0 { -1 } else { 1 });
                let (x, y) = if along_x { (u, base + side) } else { (TILE_W / 2 + side, base - TILE_W / 2 + u) };
                if inside(&way, x, y) {
                    im.put(x, y, 'B');
                    if along_x { im.put(x + 1, y, 'B') } else { im.put(x, y + 1, 'B') }
                }
            }
            for k in 0..3 {
                let v = hash(k, links as i32, 59);
                let (x, y) = (1 + (v % 14) as i32, 1 + ((v >> 4) % 14) as i32);
                if !inside(&way, x, y) && !inside(&way, x + 1, y) {
                    im.put(x, y, if v % 3 == 0 { 'l' } else { 'J' });
                }
            }
        }
    }
    im
}

// ----------------------------------------------------------- vehicles ----

fn draw_vehicle(kind: Kind, dir: Dir, load: Option<Load>, t: u32) -> Img {
    let side = matches!(dir, Dir::East | Dir::West);
    let im = match (kind, side) {
        (Kind::Cart, true) => ox_cart(load, t),
        (Kind::Cart, false) => ox_cart_end(load, t, dir == Dir::South),
        (Kind::Tram, true) => wagon(load, t),
        (Kind::Tram, false) => wagon_end(load, t),
        (Kind::Rope, _) => bucket(load, t),
        (Kind::Chute, true) => sled(load, t),
        (Kind::Chute, false) => sled_end(load, t),
        (Kind::Barge, true) => barge(load, t),
        (Kind::Barge, false) => barge_end(load, t),
        (Kind::Mule, true) => mule(load, t),
        (Kind::Mule, false) => mule_end(load, t, dir == Dir::South),
    };
    if dir == Dir::West { im.flip_h() } else { im }
}

fn pile_on(im: &mut Img, cx: i32, foot: i32, w: i32, h: i32, load: Option<Load>) {
    if let Some(load) = load {
        paint::pile(im, cx, foot, w, h, load, 11);
    }
}

fn ox_cart(load: Option<Load>, t: u32) -> Img {
    let mut im = Img::new(34, 22);
    let stride = (t % 4 / 2) as i32;
    // The ox: heavy shoulders, a yoke, horns, legs in pairs swinging.
    let ox = 19;
    im.rect(ox, 9, 13, 7, 'P');
    im.rect(ox + 1, 8, 11, 1, 'o');
    im.rect(ox, 15, 13, 1, 'B');
    im.rect(ox + 9, 6, 4, 4, 'P');
    im.rect(ox + 12, 6, 3, 3, 'o');
    im.put(ox + 14, 7, 'k');
    im.line(ox + 10, 5, ox + 9, 3, 'T');
    im.line(ox + 13, 5, ox + 14, 3, 'T');
    im.put(ox + 14, 8, 'T');
    for (lx, ph) in [(ox + 1, 0), (ox + 3, 1), (ox + 9, 1), (ox + 11, 0)] {
        let swing = if (ph + stride) % 2 == 0 { 1 } else { 0 };
        im.rect(lx, 16, 2, 5 - swing, 'B');
        im.rect(lx, 20 - swing, 2, 1, 'k');
    }
    im.line(ox - 1, 10, ox - 2, 15, 'b');
    // The yoke bar over its shoulders, the pole back to the cart.
    im.rect(ox + 6, 7, 2, 4, 'O');
    im.line(12, 14, ox + 7, 10, 'o');
    im.line(12, 15, ox + 7, 11, 'B');
    // The cart: a bed with slatted sides on two big wheels.
    im.rect(1, 10, 13, 5, 'o');
    im.rect(1, 10, 13, 1, 'O');
    for x in [3, 6, 9, 12] {
        im.line(x, 10, x, 14, 'B');
    }
    im.rect(1, 15, 13, 1, 'b');
    pile_on(&mut im, 7, 10, 12, 8, load);
    let a = paint::turn(t) * 2.0 / 8.0;
    paint::wheel(&mut im, 6.0, 17.0, 4.0, a, 8, false, ('B', 'o', 'U'));
    im
}

fn ox_cart_end(load: Option<Load>, t: u32, toward: bool) -> Img {
    let mut im = Img::new(16, 26);
    let stride = (t % 4 / 2) as i32;
    if toward {
        // The ox coming at you, its horns wide, the cart behind.
        im.rect(3, 12, 10, 7, 'P');
        im.rect(4, 10, 8, 3, 'o');
        im.rect(5, 8, 6, 5, 'P');
        im.rect(6, 6, 4, 3, 'o');
        im.put(6, 9, 'k');
        im.put(9, 9, 'k');
        im.line(4, 6, 2, 3, 'T');
        im.line(11, 6, 13, 3, 'T');
        im.rect(7, 11, 2, 2, 'T');
        im.rect(3 + stride, 19, 3, 5, 'B');
        im.rect(10 - stride, 19, 3, 5, 'B');
    } else {
        // The cart going away, its back and load, the ox ahead past it.
        im.rect(2, 12, 12, 8, 'o');
        im.rect(2, 12, 12, 1, 'O');
        for x in [4, 7, 10] {
            im.line(x, 12, x, 19, 'B');
        }
        pile_on(&mut im, 8, 12, 11, 8, load);
        im.rect(1 + stride, 20, 3, 5, 'b');
        im.rect(12 - stride, 20, 3, 5, 'b');
    }
    im
}

fn wagon(load: Option<Load>, t: u32) -> Img {
    let mut im = Img::new(26, 18);
    // A flat-bottomed tub of boards on four small iron wheels.
    im.rect(1, 8, 24, 5, 'o');
    im.rect(1, 8, 24, 1, 'O');
    im.rect(1, 12, 24, 1, 'b');
    for x in [4, 9, 14, 19] {
        im.line(x, 8, x, 12, 'B');
    }
    im.rect(0, 7, 1, 6, 'U');
    im.rect(25, 7, 1, 6, 'U');
    pile_on(&mut im, 13, 8, 22, 9, load);
    // A coupling hook at each end, an axle box over each wheel pair.
    im.rect(-1, 11, 2, 1, 'u');
    let a = paint::turn(t) * 2.0 / 6.0;
    for x in [6.0f32, 19.0] {
        paint::wheel(&mut im, x, 14.5, 3.0, a, 6, false, ('U', 'u', 'V'));
        im.rect(x as i32 - 1, 12, 3, 2, 'u');
    }
    // The rail under it.
    im.rect(0, 17, 26, 1, 'v');
    im
}

fn wagon_end(load: Option<Load>, t: u32) -> Img {
    let mut im = Img::new(14, 22);
    im.rect(2, 10, 10, 8, 'o');
    im.rect(2, 10, 10, 1, 'O');
    for x in [4, 7, 10] {
        im.line(x, 10, x, 17, 'B');
    }
    pile_on(&mut im, 7, 10, 9, 8, load);
    im.rect(1, 18, 3, 3, 'U');
    im.rect(10, 18, 3, 3, 'U');
    if t % 2 == 0 {
        im.put(2, 19, 'V');
        im.put(11, 19, 'V');
    }
    im
}

fn bucket(load: Option<Load>, t: u32) -> Img {
    let mut im = Img::new(14, 24);
    // The hanger: a sheave on the cable, a bar down, a bucket swinging below.
    let sway = ((paint::turn(t).sin()) * 1.5).round() as i32;
    paint::wheel(&mut im, 7.0, 2.0, 2.2, paint::turn(t) * 2.0, 4, false, ('V', 'U', 'v'));
    im.line(7, 4, 7 + sway, 9, 'U');
    im.line(8, 4, 8 + sway, 9, 'u');
    let x = 2 + sway;
    // A bucket of staved wood, iron-hooped, wider at the lip.
    im.rect(x, 9, 11, 1, 'v');
    for y in 10..19 {
        let inset = (y - 10) / 4;
        im.rect(x + inset, y, 11 - 2 * inset, 1, if y == 14 || y == 18 { 'u' } else { 'o' });
        im.put(x + inset, y, 'O');
        im.put(x + 10 - inset, y, 'B');
    }
    pile_on(&mut im, x + 5, 10, 10, 6, load);
    im
}

fn sled(load: Option<Load>, t: u32) -> Img {
    let mut im = Img::new(22, 16);
    let tilt = (t % 2) as i32;
    // A plank sled with runners, sliding down the trough, load heaped.
    im.rect(1, 10, 20, 2, 'o');
    im.rect(1, 12, 20, 1, 'B');
    im.line(0, 12 - tilt, 3, 13, 'b');
    im.line(21, 12 - tilt, 18, 13, 'b');
    im.rect(2, 9, 1, 2, 'B');
    im.rect(19, 9, 1, 2, 'B');
    pile_on(&mut im, 11, 10, 17, 9, load);
    // Spray of chips behind it.
    if t % 2 == 0 {
        im.put(0, 9, 'h');
    }
    im
}

fn sled_end(load: Option<Load>, t: u32) -> Img {
    let mut im = Img::new(12, 18);
    im.rect(1, 12, 10, 3, 'o');
    im.rect(1, 12, 10, 1, 'O');
    pile_on(&mut im, 6, 12, 9, 9, load);
    if t % 2 == 0 {
        im.put(5, 16, 'h');
    }
    im
}

fn barge(load: Option<Load>, t: u32) -> Img {
    let mut im = Img::new(52, 24);
    let bob = (paint::turn(t).sin().round()) as i32;
    // A long flat river barge: planked hull, raked bow and stern, a hold of load.
    let y = 11 + bob;
    for x in 1..51 {
        let rake = if x < 5 { 5 - x } else if x > 46 { x - 46 } else { 0 };
        im.rect(x, y - rake, 1, 8 + rake.min(4), 'o');
        im.put(x, y - rake, 'O');
        im.put(x, y + 7, 'b');
    }
    im.rect(1, y + 3, 50, 1, 'B');
    im.rect(4, y + 5, 44, 1, 'b');
    im.rect(10, y - 1, 30, 1, 'O');
    pile_on(&mut im, 25, y, 34, 11, load);
    // The pole man at the stern, pushing off; a rope coiled at the bow.
    im.line(46, y - 12, 49, y + 12, 'o');
    im.rect(3, y - 3, 4, 3, 't');
    // Ripples at the waterline.
    for x in (0..52).step_by(6) {
        let g = (x + t as i32 * 2) % 12;
        im.put(x, y + 9, if g < 4 { '3' } else { 'z' });
        im.put(x + 1, y + 9, if g < 4 { '3' } else { 'z' });
    }
    im
}

fn barge_end(load: Option<Load>, t: u32) -> Img {
    let mut im = Img::new(18, 30);
    let bob = (paint::turn(t).sin().round()) as i32;
    // Bow-on: the narrow prow, the hold behind it.
    let y = 8 + bob;
    im.rect(3, y + 4, 12, 14, 'o');
    for k in 0..6 {
        im.rect(3 + k, y + 18 + k, 12 - 2 * k, 1, if k % 2 == 0 { 'B' } else { 'b' });
    }
    im.rect(3, y + 4, 12, 1, 'O');
    im.rect(8, y + 5, 2, 18, 'B');
    pile_on(&mut im, 9, y + 5, 11, 9, load);
    for x in 0..18 {
        if (x + t as i32) % 5 < 2 {
            im.put(x, y + 25, 'z');
        }
    }
    im
}

/// A pack mule walking, ears up, a blanket and a pannier of wicker slung on it, the load piled over:
/// slighter than the ox, grey where the ox is brown, with no cart behind it.
fn mule(load: Option<Load>, t: u32) -> Img {
    let mut im = Img::new(26, 22);
    let stride = (t % 4 / 2) as i32;
    // The body, the neck, the head with its long ears and pale muzzle, the tail.
    im.rect(5, 10, 13, 6, 'G');
    im.rect(6, 9, 11, 1, 'J');
    im.rect(5, 15, 13, 1, 'g');
    im.rect(16, 6, 4, 6, 'G');
    im.rect(17, 6, 6, 4, 'J');
    im.rect(21, 8, 3, 3, 'h');
    im.put(21, 7, 'k');
    im.line(17, 5, 17, 2, 'G');
    im.line(19, 5, 20, 2, 'J');
    im.line(5, 11, 3, 16, 'g');
    // Four slim legs in diagonal pairs, hooves dark.
    for (lx, ph) in [(6, 0), (8, 1), (14, 1), (16, 0)] {
        let swing = if (ph + stride) % 2 == 0 { 1 } else { 0 };
        im.rect(lx, 16, 1, 5 - swing, 'g');
        im.put(lx, 20 - swing, 'k');
    }
    // The blanket and the pannier on the near flank, its wicker barred, a girth strap under.
    im.rect(7, 9, 8, 2, '7');
    im.rect(8, 11, 6, 5, 'o');
    im.rect(8, 11, 6, 1, 'O');
    for x in [10, 12] {
        im.line(x, 11, x, 15, 'B');
    }
    im.rect(8, 15, 6, 1, 'b');
    if let Some(load) = load {
        paint::pile(&mut im, 11, 9, 9, 7, load, 11);
    }
    im
}

fn mule_end(load: Option<Load>, t: u32, toward: bool) -> Img {
    let mut im = Img::new(14, 24);
    let stride = (t % 4 / 2) as i32;
    if toward {
        // Coming at you: the long face, the ears up, the baskets wide at its sides.
        im.rect(4, 6, 6, 8, 'J');
        im.rect(5, 12, 4, 3, 'h');
        im.put(5, 9, 'k');
        im.put(8, 9, 'k');
        im.line(4, 5, 3, 1, 'G');
        im.line(9, 5, 10, 1, 'G');
        im.rect(1, 14, 12, 3, 'o');
        im.rect(1, 14, 12, 1, 'O');
        im.rect(4 + stride, 17, 2, 5, 'g');
        im.rect(8 - stride, 17, 2, 5, 'g');
    } else {
        // Going away: the rump, the tail, the baskets either side and the load over them.
        im.rect(3, 9, 8, 7, 'G');
        im.rect(3, 9, 8, 1, 'J');
        im.line(7, 10, 7, 16, 'g');
        im.rect(0, 11, 3, 5, 'o');
        im.rect(11, 11, 3, 5, 'o');
        im.rect(0, 11, 3, 1, 'O');
        im.rect(11, 11, 3, 1, 'O');
        if let Some(load) = load {
            paint::pile(&mut im, 7, 9, 8, 7, load, 11);
        }
        im.rect(4 + stride, 16, 2, 6, 'g');
        im.rect(8 - stride, 16, 2, 6, 'g');
    }
    im
}
