//! The crawl's eye: a grid raycaster in the old way. Walls are whole cells,
//! cast column by column with their textures; the floor and the vault are
//! cast row by row; whatever stands in the dungeon (monsters, folk, a fire)
//! stands up as a picture facing the eye, hidden by any nearer wall. A torch
//! lights the near cells warm and the dark takes the rest.
//!
//! Map x runs east, y south; heading 0 looks east and a quarter turn
//! clockwise looks south. Everything is plain arithmetic on its inputs, so a
//! frame is the same every time it is drawn.

use super::super::overworld::ink::Img;

/// A texture: inks as colours, `None` where it is see-through.
#[derive(Clone, Debug)]
pub(crate) struct Tex {
    pub(crate) w: usize,
    pub(crate) h: usize,
    px: Vec<Option<[u8; 3]>>,
}

impl Tex {
    /// From rows of realm inks (`.` is see-through).
    pub(crate) fn from_rows(rows: &[&str]) -> Tex {
        let w = rows
            .iter()
            .map(|r| r.chars().count())
            .max()
            .unwrap_or(1)
            .max(1);
        let h = rows.len().max(1);
        let mut px = vec![None; w * h];
        for (y, row) in rows.iter().enumerate() {
            for (x, ch) in row.chars().enumerate() {
                px[y * w + x] = super::super::overworld::ink::ink(ch);
            }
        }
        Tex { w, h, px }
    }

    /// From a sprite drawn for the Delve.
    pub(crate) fn from_img(img: &Img) -> Tex {
        let (w, h) = (img.w.max(1) as usize, img.h.max(1) as usize);
        let mut px = vec![None; w * h];
        for y in 0..h {
            for x in 0..w {
                px[y * w + x] = img.get(x as i32, y as i32);
            }
        }
        Tex { w, h, px }
    }

    /// The texel at `(u, v)` in 0..1, wrapping.
    pub(crate) fn at(&self, u: f32, v: f32) -> Option<[u8; 3]> {
        let x = ((u.rem_euclid(1.0) * self.w as f32) as usize).min(self.w - 1);
        let y = ((v.rem_euclid(1.0) * self.h as f32) as usize).min(self.h - 1);
        self.px[y * self.w + x]
    }
}

/// What a cell is to the eye.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Cell {
    /// Open floor, of a kind of ground.
    Open(Ground),
    /// A wall, faced with a texture (an index into the wall textures).
    Wall(u8),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Ground {
    Flags,
    Water,
    Lava,
    Stairs,
}

/// The dungeon as the eye sees it.
#[derive(Clone, Debug)]
pub(crate) struct Grid {
    pub(crate) w: usize,
    pub(crate) h: usize,
    pub(crate) cells: Vec<Cell>,
    /// What hangs on a wall's face: by the wall cell and the way the face
    /// looks (toward the open cell), a decal (an index into the kit's).
    pub(crate) decals: std::collections::HashMap<(i32, i32, i8, i8), u8>,
}

impl Grid {
    pub(crate) fn at(&self, x: i32, y: i32) -> Cell {
        if x < 0 || y < 0 || x as usize >= self.w || y as usize >= self.h {
            return Cell::Wall(0);
        }
        self.cells[y as usize * self.w + x as usize]
    }

    #[cfg(test)]
    pub(crate) fn open(&self, x: i32, y: i32) -> bool {
        matches!(self.at(x, y), Cell::Open(_))
    }
}

/// Where the eye stands and looks.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct Eye {
    pub(crate) x: f32,
    pub(crate) y: f32,
    pub(crate) heading: f32,
    /// Horizontal field of view, radians.
    pub(crate) fov: f32,
    /// Eye height above the floor (a wall is 1 tall), with the walk's bob.
    pub(crate) z: f32,
    /// Looking up (+) or down (-), in screen heights.
    pub(crate) pitch: f32,
}

/// Something standing in the dungeon, drawn as a picture facing the eye.
pub(crate) struct Thing<'a> {
    pub(crate) x: f32,
    pub(crate) y: f32,
    pub(crate) tex: &'a Tex,
    /// How tall it stands (a wall is 1), and how far its feet are off the
    /// floor.
    pub(crate) height: f32,
    pub(crate) lift: f32,
    /// Its own light (a fire, a glowing eye): the dark doesn't take it.
    pub(crate) lit: bool,
    /// Turned to face left.
    pub(crate) flip: bool,
}

/// The textures the dungeon is faced with.
pub(crate) struct Kit<'a> {
    pub(crate) walls: &'a [Tex],
    /// What hangs on the walls: a sconce (in this moment's flicker), a banner.
    pub(crate) decals: &'a [&'a Tex],
    pub(crate) floor: &'a Tex,
    pub(crate) water: &'a Tex,
    pub(crate) lava: &'a Tex,
    pub(crate) stairs: &'a Tex,
    pub(crate) ceiling: &'a Tex,
}

/// The torch: how far its light carries and its flicker (1 is steady); and
/// a second, warmer light standing somewhere in the dungeon (a campfire, a
/// candle): where, and how far it carries.
#[derive(Clone, Copy, Debug)]
pub(crate) struct Torch {
    pub(crate) reach: f32,
    pub(crate) gain: f32,
    pub(crate) glow: Option<(f32, f32, f32)>,
    /// The nearest wall sconces, each a small warm light of its own.
    pub(crate) sconces: [Option<(f32, f32)>; 3],
}

/// How far a sconce's light carries.
const SCONCE_REACH: f32 = 2.2;

/// How bright a texel at distance `d` from the eye, standing at `(x, y)`,
/// is under the torch (and any fire), as a warm tint.
fn lit(c: [u8; 3], d: f32, (x, y): (f32, f32), torch: Torch) -> [u8; 3] {
    let mut fall = torch.gain / (1.0 + d * d / (torch.reach * torch.reach) * 4.0);
    if let Some((gx, gy, reach)) = torch.glow {
        let g = (x - gx).hypot(y - gy);
        fall += torch.gain * 0.9 / (1.0 + g * g / (reach * reach) * 4.0);
    }
    for (sx, sy) in torch.sconces.iter().flatten() {
        let g = (x - sx).hypot(y - sy);
        fall += 0.5 / (1.0 + g * g / (SCONCE_REACH * SCONCE_REACH) * 4.0);
    }
    let fall = fall.clamp(0.0, 1.25);
    // Near is warm firelight; far cools to the dark.
    let warm = (1.0 - d / (torch.reach * 1.6)).clamp(0.0, 1.0);
    let tint = [1.0, 0.86 + 0.06 * (1.0 - warm), 0.70 + 0.18 * (1.0 - warm)];
    [
        (c[0] as f32 * fall * tint[0]).min(255.0) as u8,
        (c[1] as f32 * fall * tint[1]).min(255.0) as u8,
        (c[2] as f32 * fall * tint[2]).min(255.0) as u8,
    ]
}

/// One frame: the eye's view of the grid and the things in it, `w` by `h`.
pub(crate) fn render(
    grid: &Grid,
    eye: &Eye,
    things: &[Thing],
    kit: &Kit,
    torch: Torch,
    (w, h): (u32, u32),
) -> image::RgbaImage {
    let (wi, hi) = (w.max(1) as usize, h.max(1) as usize);
    let mut px = vec![[0u8; 3]; wi * hi];
    let (dx, dy) = (eye.heading.cos(), eye.heading.sin());
    let half = (eye.fov * 0.5).tan();
    // The camera plane: screen right, scaled to the field of view.
    let (px_, py_) = (-dy * half, dx * half);
    let focal = wi as f32 / (2.0 * half);
    let horizon = hi as f32 * (0.5 + eye.pitch);
    // The floor and the vault, row by row.
    for y in 0..hi {
        let row = y as f32 + 0.5 - horizon;
        let (dist, floor) = if row > 0.0 {
            (eye.z * focal / row, true)
        } else if row < 0.0 {
            ((1.0 - eye.z) * focal / -row, false)
        } else {
            continue;
        };
        if dist > torch.reach * 2.6 {
            continue;
        }
        let (lx, ly) = (dx - px_, dy - py_);
        let (sx, sy) = (2.0 * px_ / wi as f32, 2.0 * py_ / wi as f32);
        for x in 0..wi {
            let t = x as f32 + 0.5;
            let (fx, fy) = (eye.x + dist * (lx + sx * t), eye.y + dist * (ly + sy * t));
            let tex = if floor {
                match grid.at(fx.floor() as i32, fy.floor() as i32) {
                    Cell::Open(Ground::Water) => kit.water,
                    Cell::Open(Ground::Lava) => kit.lava,
                    Cell::Open(Ground::Stairs) => kit.stairs,
                    _ => kit.floor,
                }
            } else {
                kit.ceiling
            };
            if let Some(c) = tex.at(fx, fy) {
                let glow = floor && std::ptr::eq(tex, kit.lava);
                px[y * wi + x] = if glow {
                    c
                } else {
                    // The vault stays low, so the walls and whatever
                    // stands before them carry the picture.
                    let c = lit(c, dist, (fx, fy), torch);
                    if floor {
                        c
                    } else {
                        [
                            (c[0] as u16 * 5 / 8) as u8,
                            (c[1] as u16 * 5 / 8) as u8,
                            (c[2] as u16 * 5 / 8) as u8,
                        ]
                    }
                };
            }
        }
    }
    // The walls, column by column, remembering how near each one is.
    let mut depth = vec![f32::MAX; wi];
    for (x, near) in depth.iter_mut().enumerate() {
        let cam = 2.0 * (x as f32 + 0.5) / wi as f32 - 1.0;
        let (rx, ry) = (dx + px_ * cam, dy + py_ * cam);
        let (mut mx, mut my) = (eye.x.floor() as i32, eye.y.floor() as i32);
        let ddx = if rx == 0.0 {
            f32::MAX
        } else {
            (1.0 / rx).abs()
        };
        let ddy = if ry == 0.0 {
            f32::MAX
        } else {
            (1.0 / ry).abs()
        };
        let (step_x, mut side_x) = if rx < 0.0 {
            (-1, (eye.x - mx as f32) * ddx)
        } else {
            (1, (mx as f32 + 1.0 - eye.x) * ddx)
        };
        let (step_y, mut side_y) = if ry < 0.0 {
            (-1, (eye.y - my as f32) * ddy)
        } else {
            (1, (my as f32 + 1.0 - eye.y) * ddy)
        };
        let mut hit = None;
        for _ in 0..96 {
            let across = side_x < side_y;
            if across {
                side_x += ddx;
                mx += step_x;
            } else {
                side_y += ddy;
                my += step_y;
            }
            if let Cell::Wall(face) = grid.at(mx, my) {
                hit = Some((face, across));
                break;
            }
        }
        let Some((face, across)) = hit else {
            continue;
        };
        let perp = if across { side_x - ddx } else { side_y - ddy }.max(0.02);
        *near = perp;
        // Where along the wall's face the ray struck, for the texture.
        let mut u = if across {
            eye.y + perp * ry
        } else {
            eye.x + perp * rx
        };
        u -= u.floor();
        if (across && rx > 0.0) || (!across && ry < 0.0) {
            u = 1.0 - u;
        }
        let top = horizon - (1.0 - eye.z) * focal / perp;
        let bottom = horizon + eye.z * focal / perp;
        let tex = &kit.walls[usize::from(face).min(kit.walls.len() - 1)];
        // The face the ray struck looks back the way it came.
        let facing: (i8, i8) = if across {
            (-step_x as i8, 0)
        } else {
            (0, -step_y as i8)
        };
        let decal = grid
            .decals
            .get(&(mx, my, facing.0, facing.1))
            .and_then(|&d| kit.decals.get(usize::from(d)).copied());
        // The sides running north and south are a shade darker, as the old
        // games drew them, so corners read.
        let shade = if across { 0.78 } else { 1.0 };
        let (y0, y1) = (
            top.max(0.0) as usize,
            (bottom.ceil().max(0.0) as usize).min(hi),
        );
        for y in y0..y1 {
            let v = (y as f32 + 0.5 - top) / (bottom - top);
            // A decal hangs in the middle of the face, over the stone; its
            // bright parts (a flame) burn as bright as they are.
            if let Some(d) = decal {
                let (du, dv) = ((u - 0.3) / 0.4, (v - 0.22) / 0.6);
                if (0.0..1.0).contains(&du)
                    && (0.0..1.0).contains(&dv)
                    && let Some(c) = d.at(du, dv)
                {
                    let shaded = lit(c, perp, (eye.x + perp * rx, eye.y + perp * ry), torch);
                    let bright = c[0] as u32 + c[1] as u32 + c[2] as u32 > 480;
                    px[y * wi + x] = if bright { c } else { shaded };
                    continue;
                }
            }
            if let Some(c) = tex.at(u, v) {
                let c = lit(c, perp, (eye.x + perp * rx, eye.y + perp * ry), torch);
                px[y * wi + x] = [
                    (c[0] as f32 * shade) as u8,
                    (c[1] as f32 * shade) as u8,
                    (c[2] as f32 * shade) as u8,
                ];
            }
        }
    }
    // The things in the dungeon, farthest first, each column hidden by any
    // nearer wall.
    let (rx_, ry_) = (-dy, dx);
    let mut order: Vec<(f32, &Thing)> = things
        .iter()
        .map(|t| ((t.x - eye.x) * dx + (t.y - eye.y) * dy, t))
        .filter(|(d, _)| *d > 0.15)
        .collect();
    order.sort_by(|a, b| b.0.total_cmp(&a.0));
    for (dist, thing) in order {
        let lateral = (thing.x - eye.x) * rx_ + (thing.y - eye.y) * ry_;
        let cx = wi as f32 * 0.5 + focal * lateral / dist;
        let tall = focal * thing.height / dist;
        let wide = tall * thing.tex.w as f32 / thing.tex.h as f32;
        let feet = horizon + (eye.z - thing.lift) * focal / dist;
        let (left, top) = (cx - wide * 0.5, feet - tall);
        let x0 = left.max(0.0) as usize;
        let x1 = ((left + wide).ceil().max(0.0) as usize).min(wi);
        let y0 = top.max(0.0) as usize;
        let y1 = (feet.ceil().max(0.0) as usize).min(hi);
        for x in x0..x1 {
            if depth[x] < dist {
                continue;
            }
            let mut u = (x as f32 + 0.5 - left) / wide;
            if thing.flip {
                u = 1.0 - u;
            }
            if !(0.0..1.0).contains(&u) {
                continue;
            }
            for y in y0..y1 {
                let v = (y as f32 + 0.5 - top) / tall;
                if !(0.0..1.0).contains(&v) {
                    continue;
                }
                if let Some(c) = thing.tex.at(u, v) {
                    px[y * wi + x] = if thing.lit {
                        c
                    } else {
                        // Figures stand out a touch brighter than the stone.
                        let c = lit(c, dist, (thing.x, thing.y), torch);
                        [
                            (c[0] as u16 * 5 / 4).min(255) as u8,
                            (c[1] as u16 * 5 / 4).min(255) as u8,
                            (c[2] as u16 * 5 / 4).min(255) as u8,
                        ]
                    };
                }
            }
        }
    }
    let mut out = image::RgbaImage::new(w.max(1), h.max(1));
    for (i, c) in px.into_iter().enumerate() {
        out.put_pixel(
            (i % wi) as u32,
            (i / wi) as u32,
            image::Rgba([c[0], c[1], c[2], 255]),
        );
    }
    out
}

#[cfg(test)]
#[path = "../../../../../tests/cockpit/world_viz/crawl__render_tests.rs"]
mod tests;
