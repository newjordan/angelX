//! The course's country, baked once: a dark field, the stands in tiers either side of the road,
//! the wall and the rumble strips, the dirt with its ruts, the line across it, the trees beyond.
//! What moves (the crowd, the torches, the pennants) is listed here and drawn each frame by
//! `draw_life` over a window of the baked picture, so the frame is a copy and a few hundred figures
//! and not the whole course painted again.
use super::super::super::ink::{Img, bayer, hash, ink, vnoise};
use super::super::TILE;
use super::art;
use crate::rules::chariot::{LANE_TILES, TRACKS, Track, pos_on};
use std::sync::OnceLock;

/// Pixels of road either side of the centre line, and where the wall, the first tier and the
/// last tier of the stands lie from it.
const TREAD: f32 = 28.0;
const WALL: f32 = 36.0;
const STANDS: f32 = 64.0;
/// Rows of spectators stand this far out from the road's middle.
const ROWS: [f32; 3] = [43.0, 51.0, 59.0];

/// Black paper round the course's country: the camera may ride out past its edge, and what it
/// finds there is dark field and trees that fall away to nothing.
pub(super) const PAD: i32 = 200;

/// One spectator's place in the stands.
#[derive(Clone, Copy)]
pub(super) struct Seat {
    pub x: i32,
    pub y: i32,
    pub seed: u32,
    /// Paces along the course the seat is nearest.
    pub at: f32,
}

/// A post on the wall: a torch or a pennant.
#[derive(Clone, Copy)]
pub(super) struct Post {
    pub x: i32,
    pub y: i32,
    pub torch: bool,
    pub seed: u32,
}

pub(super) struct World {
    pub map: Img,
    /// The whole course on a small plate, and where the map's origin and scale fall on it.
    pub mini: Img,
    pub mini_xf: (f32, f32, f32),
    pub crowd: Vec<Seat>,
    pub posts: Vec<Post>,
    /// The line: its centre, and the way the course runs there.
    pub line: (f32, f32, f32, f32),
}

pub(super) fn world(index: usize) -> &'static World {
    static WORLDS: [OnceLock<World>; 5] = [const { OnceLock::new() }; 5];
    let i = index.min(TRACKS.len() - 1);
    WORLDS[i].get_or_init(|| bake(&TRACKS[i]))
}

fn rgb(c: char) -> [u8; 3] {
    ink(c).unwrap_or([0, 0, 0])
}

/// The field: darkest at the map's edge, in dithered patches of two greens.
fn ground(map: &mut Img) {
    let (w, h) = (map.w, map.h);
    let tones = ['f', 'D', 'F', 'E'].map(rgb);
    for y in 0..h {
        for x in 0..w {
            let n = vnoise(x as f32 * 0.09, y as f32 * 0.12, 311) * 0.7 + vnoise(x as f32 * 0.31, y as f32 * 0.37, 312) * 0.3;
            let edge = x.min(y).min(w - 1 - x).min(h - 1 - y) as f32 / 70.0;
            let level = (n * 2.0 + 0.4 - (1.0 - edge.min(1.0)) * 2.4).clamp(0.0, 2.999);
            let i = level.floor() as usize;
            let up = usize::from(bayer(x, y) < level - level.floor());
            map.set(x, y, tones[i + up]);
            let rim = x.min(y).min(w - 1 - x).min(h - 1 - y);
            if rim < 48 && bayer(x, y) < 1.0 - rim as f32 / 48.0 {
                map.put(x, y, 'k');
            }
        }
    }
    // Tufts of grass, here and there.
    for y in (0..h).step_by(3) {
        for x in (0..w).step_by(3) {
            let j = hash(x, y, 313);
            if j % 37 == 0 {
                map.put(x, y, 'l');
                map.put(x + 1, y - 1, 'E');
                map.put(x - 1, y - 2, 'E');
            }
        }
    }
}

fn bake(course: &Track) -> World {
    let (mw, mh) = ((course.map.0 * TILE as f32) as i32 + 2 * PAD, (course.map.1 * TILE as f32) as i32 + 2 * PAD);
    let mut map = Img::black(mw, mh);
    ground(&mut map);
    let px = |d: f32, lane: f32| {
        let (x, y) = pos_on(course, d, lane);
        (x * TILE as f32 + PAD as f32, y * TILE as f32 + PAD as f32)
    };
    // How far every pixel is from the road's middle, by the nearest of the points along it.
    let step = 4.0;
    let n = (course.lap / step) as i32;
    let centres: Vec<(f32, f32)> = (0..n).map(|k| px(k as f32 * step, 0.0)).collect();
    let mut dist = vec![f32::MAX; (mw * mh) as usize];
    let reach = STANDS.ceil() as i32 + 2;
    for &(cx, cy) in &centres {
        let (ix, iy) = (cx as i32, cy as i32);
        for y in (iy - reach).max(0)..(iy + reach).min(mh) {
            let row = (y * mw) as usize;
            let dy = y as f32 + 0.5 - cy;
            for x in (ix - reach).max(0)..(ix + reach).min(mw) {
                let dx = x as f32 + 0.5 - cx;
                let d = dx * dx + dy * dy;
                if d < dist[row + x as usize] * dist[row + x as usize] {
                    dist[row + x as usize] = d.sqrt();
                }
            }
        }
    }
    // Bands out from the middle: the dirt, its packed edge, the rumble strip's bed, the wall, and
    // the stands in tiers going up and away.
    let (dirt, packed, edge, rutted) = (rgb('I'), rgb('B'), rgb('b'), rgb('n'));
    let (pebble, pale) = (rgb('P'), rgb('r'));
    let (tier_a, tier_b, tier_lip, tier_dark) = (rgb('b'), rgb('n'), rgb('I'), rgb('Z'));
    let (wall, wall_lip, wall_dark) = (rgb('g'), rgb('j'), rgb('X'));
    for y in 0..mh {
        for x in 0..mw {
            let d = dist[(y * mw + x) as usize];
            if d >= STANDS {
                continue;
            }
            let c = if d < TREAD {
                let n = vnoise(x as f32 * 0.35, y as f32 * 0.25, 211);
                let grit = hash(x, y, 212) % 97;
                if grit == 0 {
                    pebble
                } else if grit == 1 {
                    pale
                } else if n > 0.7 && bayer(x, y) < 0.5 {
                    packed
                } else if n < 0.26 && bayer(x, y) < 0.6 {
                    rutted
                } else {
                    dirt
                }
            } else if d < TREAD + 2.0 {
                if bayer(x, y) < 0.5 { packed } else { dirt }
            } else if d < TREAD + 5.0 {
                edge
            } else if d < WALL - 1.5 {
                wall_dark
            } else if d < WALL - 0.5 {
                wall_lip
            } else if d < WALL + 1.5 {
                wall
            } else {
                // Tiers three pixels deep, each lit along its lip.
                let t = (d - WALL - 1.5) / 3.0;
                let k = t.floor() as i32;
                let frac = t - t.floor();
                if frac < 0.34 && k % 2 == 0 {
                    tier_lip
                } else if k % 2 == 0 {
                    if bayer(x, y) < 0.2 { tier_dark } else { tier_a }
                } else {
                    tier_b
                }
            };
            map.set(x, y, c);
        }
    }
    // The rumble strips: blocks in red and tan along both edges of the dirt.
    let kerb = TREAD + 2.0;
    let mut d = 0.0f32;
    let mut k = 0;
    while d < course.lap {
        let ink_of = if k % 2 == 0 { '7' } else { 'o' };
        for side in [-1.0f32, 1.0] {
            let (x, y) = px(d, side * kerb / (LANE_TILES * TILE as f32));
            map.rect(x as i32 - 1, y as i32 - 1, 3, 3, ink_of);
        }
        d += 6.0;
        k += 1;
    }
    // The ruts the wheels have worn, and the dashes down the middle.
    let mut d = 0.0f32;
    while d < course.lap {
        for lane in [-0.62f32, 0.4] {
            let (x, y) = px(d, lane);
            map.put(x as i32, y as i32, 'n');
        }
        if (d / 12.0) as i32 % 3 != 0 {
            let (x, y) = px(d, 0.0);
            map.put(x as i32, y as i32, 'P');
        }
        d += 1.5;
    }
    // The line across the road, in squares.
    let (lx, ly) = px(0.0, 0.0);
    let (tx, ty) = {
        let (ax, ay) = px(-3.0, 0.0);
        let (bx, by) = px(3.0, 0.0);
        let l = (bx - ax).hypot(by - ay).max(1e-3);
        ((bx - ax) / l, (by - ay) / l)
    };
    let (nx, ny) = (-ty, tx);
    for u in -28..28 {
        for v in -4..4 {
            let sq = ((u + 28) / 4 + (v + 4) / 4) % 2 == 0;
            let (x, y) = (lx + nx * u as f32 + tx * v as f32, ly + ny * u as f32 + ty * v as f32);
            for (ox, oy) in [(0, 0), (1, 0), (0, 1)] {
                map.put(x as i32 + ox, y as i32 + oy, if sq { 'i' } else { 'K' });
            }
        }
    }
    // Trees and rocks beyond the stands, wherever the hash puts them.
    for ty in 0..(mh / TILE) {
        for tx in 0..(mw / TILE) {
            let j = hash(tx * 3, ty * 5, 97);
            let (fx, fy) = (tx * TILE + (j % 11) as i32 + 2, ty * TILE + (j / 11 % 11) as i32 + 4);
            let near = dist[(fy.clamp(0, mh - 1) * mw + fx.clamp(0, mw - 1)) as usize];
            if near < STANDS + 14.0 {
                continue;
            }
            if j % 9 == 0 {
                map.ellipse(fx as f32, fy as f32 - 1.0, 8.0, 3.0, 'f');
                map.rect(fx - 1, fy - 5, 3, 6, 'n');
                for (i, (rx, ry, c)) in [(8.0f32, 5.0f32, 'D'), (6.5, 4.5, 'F'), (4.5, 3.5, 'E')].into_iter().enumerate() {
                    map.ellipse(fx as f32, fy as f32 - 9.0 - i as f32 * 3.0, rx, ry, c);
                }
                map.put(fx - 2, fy - 15, 'l');
            } else if j % 41 == 1 {
                map.ellipse(fx as f32, fy as f32, 4.0, 2.5, 'g');
                map.ellipse(fx as f32 - 0.5, fy as f32 - 1.0, 3.0, 1.8, 'j');
                map.put(fx - 2, fy - 2, 'G');
            }
        }
    }
    // The stands' people: three rows either side, wherever the nearest road is this one.
    let mut crowd = Vec::new();
    let mut posts = Vec::new();
    let mut d = 0.0f32;
    let mut idx = 0u32;
    while d < course.lap {
        for side in [-1.0f32, 1.0] {
            for (row, &off) in ROWS.iter().enumerate() {
                let (x, y) = px(d, side * off / (LANE_TILES * TILE as f32));
                let (ix, iy) = (x as i32, y as i32);
                if ix < 8 || iy < 12 || ix >= mw - 8 || iy >= mh - 4 {
                    continue;
                }
                let here = dist[(iy * mw + ix) as usize];
                if (here - off).abs() > 3.0 {
                    continue;
                }
                let jitter = hash(idx as i32, row as i32 * 2 + side as i32, 17);
                if jitter % 7 == 0 {
                    continue;
                }
                crowd.push(Seat { x: ix + (jitter % 3) as i32 - 1, y: iy + (jitter / 3 % 3) as i32 - 1, seed: jitter, at: d });
            }
            if idx % 11 == 0 {
                let (x, y) = px(d, side * (WALL + 2.5) / (LANE_TILES * TILE as f32));
                let (ix, iy) = (x as i32, y as i32);
                if ix > 6 && iy > 18 && ix < mw - 6 && iy < mh - 2 && (dist[(iy * mw + ix) as usize] - (WALL + 2.5)).abs() < 3.0 {
                    posts.push(Post { x: ix, y: iy, torch: (idx / 11 + u32::from(side > 0.0)) % 2 == 0, seed: hash(ix, iy, 5) });
                }
            }
        }
        d += 8.0;
        idx += 1;
    }
    // Far rows first, so a nearer figure covers the one behind it.
    crowd.sort_by_key(|s| (s.y, s.x));
    // The minimap's plate, with the course on it in dots.
    let (bw, bh) = (92, 54);
    let scale = ((bw - 8) as f32 / course.map.0).min((bh - 8) as f32 / course.map.1);
    let (ox, oy) = ((bw as f32 - course.map.0 * scale) / 2.0, (bh as f32 - course.map.1 * scale) / 2.0);
    let mut mini = Img::new(bw, bh);
    crate::render::overworld::chrome::board(&mut mini, (0, 0, bw, bh), 'B');
    for k in 0..(course.lap / 12.0) as i32 {
        let (x, y) = pos_on(course, k as f32 * 12.0, 0.0);
        mini.put((ox + x * scale) as i32, (oy + y * scale) as i32, 'j');
    }
    World { map, mini, mini_xf: (ox, oy, scale), crowd, posts, line: (lx, ly, tx, ty) }
}

/// A window `w` by `h` of the baked country at `(x, y)`.
pub(super) fn window(world: &World, x: i32, y: i32, w: i32, h: i32) -> Img {
    let (mw, mh) = (world.map.w, world.map.h);
    if x >= 0 && y >= 0 && x + w <= mw && y + h <= mh {
        return Img::from_view(&world.map, x, y, w, h);
    }
    // Past the map's edge is black paper.
    let (x0, y0, x1, y1) = (x.max(0), y.max(0), (x + w).min(mw), (y + h).min(mh));
    let mut out = Img::new(w, h);
    if x1 > x0 && y1 > y0 {
        out.stamp(&Img::from_view(&world.map, x0, y0, x1 - x0, y1 - y0), x0 - x, y0 - y);
    }
    out
}

/// What the crowd is doing: `roar` is where the roar began (paces along the course) and how many
/// ticks ago; `heat` is how much of the field is in a fight, 0 to 1, to lift every arm a little.
#[derive(Clone, Copy)]
pub(super) struct Mood {
    pub roar: Option<(f32, u32)>,
    pub heat: f32,
    pub lap: f32,
}

/// The crowd in the window (`cam` its corner), as the mood says: swaying in place, arms up when
/// the roar reaches them, bobbing at the top of the jump, a few waving a pennant in a yard's cloth.
pub(super) fn draw_crowd(layer: &mut Img, world: &World, cam: (i32, i32), now: u32, mood: &Mood, cloths: &[char]) {
    const COATS: [char; 8] = ['g', 'j', 'G', 'B', 'S', 'x', 'P', 'r'];
    const SKINS: [char; 4] = ['o', 't', 'r', 'P'];
    const CAPS: [char; 5] = ['n', 'b', 'g', 'X', 'I'];
    let (w, h) = (layer.w, layer.h);
    for s in &world.crowd {
        let (sx, sy) = (s.x - cam.0, s.y - cam.1);
        if sx < -8 || sy < -12 || sx > w + 8 || sy > h + 12 {
            continue;
        }
        let mut pose = if (now / 14 + s.seed % 5) % 2 == 0 { 0 } else { 1 };
        let mut lift = 0;
        if let Some((at, age)) = mood.roar {
            let gap = (s.at - at).abs();
            let gap = gap.min(mood.lap - gap);
            let t = age as f32 - gap / 14.0;
            if (0.0..60.0).contains(&t) {
                pose = 2;
                // The jump: up and down in a short wave, each figure out of step with the next.
                let beat = (t as u32 + s.seed % 4) % 8;
                if beat < 4 {
                    pose = 3;
                    lift = [2, 3, 3, 2][beat as usize];
                }
            }
        } else if mood.heat > 0.0 && (now / 9 + s.seed % 7) % 5 == 0 && f32::from((s.seed % 100) as u8) / 100.0 < mood.heat {
            pose = 2;
        }
        let warm = s.seed % 13 == 0;
        let coat = if warm { 'O' } else { COATS[(s.seed >> 3) as usize % COATS.len()] };
        art::paint_spectator(layer, sx - 3, sy - 9 - lift, coat, SKINS[(s.seed >> 7) as usize % SKINS.len()], CAPS[(s.seed >> 11) as usize % CAPS.len()], pose);
        // A few carry a pennant in a yard's cloth, waving as the arms go.
        if s.seed % 9 == 0 && !cloths.is_empty() {
            let cloth = cloths[(s.seed >> 5) as usize % cloths.len()];
            let up = pose >= 2;
            let top = sy - 9 - lift - if up { 7 } else { 3 };
            layer.rect(sx + 3, top, 1, 8, 'j');
            let wave = ((now / 4 + s.seed) % 3) as i32;
            layer.rect(sx + 4, top, 4 + wave, 2, cloth);
        }
    }
}

/// The torches and the pennants on the wall.
pub(super) fn draw_posts(layer: &mut Img, world: &World, cam: (i32, i32), now: u32, cloths: &[char]) {
    let (w, h) = (layer.w, layer.h);
    for p in &world.posts {
        let (sx, sy) = (p.x - cam.0, p.y - cam.1);
        if sx < -10 || sy < -20 || sx > w + 10 || sy > h + 30 {
            continue;
        }
        layer.rect(sx, sy - 14, 1, 15, 'b');
        layer.put(sx, sy - 14, 'B');
        if p.torch {
            // The flame: two tongues that gutter out of step, and an ember above.
            let f = hash(p.x, p.y, now / 2) % 5;
            layer.rect(sx - 1, sy - 19, 3, 4, 'R');
            layer.rect(sx - 1 + (f % 2) as i32, sy - 21, 2, 3, '@');
            layer.put(sx, sy - 18, '6');
            layer.put(sx - 1 + (f % 3) as i32, sy - 23 - (f / 3) as i32, 'o');
            // A pool of warm light on the stands.
            for dy in -5..=5i32 {
                for dx in -9..=9i32 {
                    let k = (dx * dx) as f32 / 81.0 + (dy * dy) as f32 / 25.0;
                    if k < 1.0 && bayer(sx + dx, sy + dy) < (1.0 - k) * 0.22 && layer.get(sx + dx, sy - 6 + dy).is_some() {
                        layer.put(sx + dx, sy - 6 + dy, 'o');
                    }
                }
            }
        } else if !cloths.is_empty() {
            let cloth = cloths[(p.seed >> 4) as usize % cloths.len()];
            let wave = ((now / 5 + p.seed) % 4) as i32;
            for row in 0..5 {
                let reach = (8 - row * 2 + (wave - 1)).max(2);
                layer.rect(sx + 1, sy - 14 + row, reach, 1, if row == 0 { 'O' } else { cloth });
            }
        }
    }
}
