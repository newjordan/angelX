//! The Keep's site in the flat view (the Works, `world::works`): the same
//! stones at the same progress as the 3D view, drawn course by course in the
//! kingdom's own pixel inks.
//!
//! The castle is one picture of the site, built as the masons lay: each stone
//! drawn into it with a depth per pixel (how far south its surface stands on
//! the floor, as the oblique view tells nearness), so a stone laid later
//! behind one already drawn is hidden by it, and the picture only grows. The
//! newest stones' joints are dark mortar that dries as the courses go on. A
//! knight walking in front of a wall is drawn over it, one behind it under
//! it: the picture is cut by the depth of the knight's feet when it is laid
//! on the view (`back` before the knights, `front` after).
//!
//! The falsework (the scaffolds, the crane's frame) and the working folk are
//! drawn each time into a picture of their own over it.

use std::sync::{Arc, Mutex};

use super::ink::{Img, Rgb, ink};
use crate::world::works::{Castle, PX_PER_M, Work};
use crate::world::works_life as wl;
use crate::world3d::works_mesh::{self, Covers};
use crate::world3d::works_site::WET_STONES;

/// How much of a stone's own colour each of the six levels shows, dark to
/// light: shaded face, face, lit face, the sun on top. The colour is the
/// castle's own, by its material, put on the flat map's inks.
const LEVELS: [f32; 6] = [0.20, 0.32, 0.46, 0.62, 0.78, 0.95];

/// A material's colour in the flat map, 0 to 1: the great tower's pale warm
/// limestone (`materials` of the export) as the 3D view's stone is, the
/// dressed work a little paler, the roofs' lead and slate cool and dark;
/// `None` for timber, which is drawn in the map's own browns.
fn material_colour(name: &str) -> Option<[f32; 3]> {
    match name {
        "ashlar-pale-warm" => Some([0.90, 0.80, 0.63]),
        "ashlar-pale" => Some([0.84, 0.79, 0.69]),
        "dressing-pale" => Some([0.94, 0.88, 0.76]),
        "lead" => Some([0.58, 0.58, 0.62]),
        "slate" => Some([0.46, 0.47, 0.52]),
        "iron" => Some([0.38, 0.36, 0.35]),
        "oak" | "oak-weathered" => None,
        _ => Some([0.80, 0.77, 0.68]),
    }
}

/// The picture of the castle as built so far and the depth of each pixel.
pub struct Flat {
    plan: &'static str,
    /// How many stones are drawn into it.
    laid: u32,
    /// Where its top left stands in the kingdom's pixels.
    pub at: (i32, i32),
    pub img: Img,
    depth: Vec<f32>,
    /// How many earth lifts are painted into it.
    dug: usize,
}

/// Where the picture lies: the castle's bounds, a margin round them.
fn frame(castle: &Castle) -> ((i32, i32), (i32, i32)) {
    let b = castle.bounds;
    let (ox, oz) = crate::world::works::origin_px();
    let (left, right) = (ox + b[0] * PX_PER_M - 8.0, ox + b[3] * PX_PER_M + 8.0);
    // The top of the tallest stone, as high up the picture as it stands; the
    // foot of the nearest.
    let top = oz - b[4] * PX_PER_M - b[5] * PX_PER_M - 8.0;
    let bottom = oz - b[1] * PX_PER_M + 12.0;
    ((left.floor() as i32, top.floor() as i32), ((right - left).ceil() as i32, (bottom - top).ceil() as i32))
}

impl Flat {
    fn new(castle: &'static Castle) -> Flat {
        let (at, (w, h)) = frame(castle);
        Flat { plan: castle.id.as_str(), laid: 0, at, img: Img::new(w, h), depth: vec![f32::MIN; (w * h) as usize], dug: 0 }
    }

    /// Draw stones `from..to` as the state `to` shows them.
    fn lay(&mut self, castle: &'static Castle, covers: &Covers, from: u32, to: u32, laid: u32, lost: &[u32], bands: &[(u32, u8)]) {
        for i in from..to {
            // A stone a raid knocked out is a gap in the wall.
            if lost.binary_search(&i).is_ok() {
                continue;
            }
            let stone = &castle.stones[i as usize];
            let c = stone.corners().map(|p| castle.world(p));
            // Mortar is dark while it is wet.
            let wet = (1.0 - laid.saturating_sub(i + 1) as f32 / WET_STONES as f32).clamp(0.0, 1.0);
            let tone = works_mesh::albedo(castle, stone, i);
            let kind = crate::rules::site::Site::stone_kind(bands, i);
            let base = if kind == 0 { material_colour(castle.material(stone.mat)) } else { Some(crate::rules::site::Site::stone_rgb(kind)) };
            let centre = [0, 1, 2].map(|k| c.iter().map(|p| p[k]).sum::<f32>() / 8.0);
            const RINGS: [[usize; 4]; 6] = [[0, 1, 2, 3], [4, 5, 6, 7], [0, 1, 5, 4], [1, 2, 6, 5], [2, 3, 7, 6], [3, 0, 4, 7]];
            for (f, ring) in RINGS.iter().enumerate() {
                if covers.faces[i as usize][f] <= laid {
                    continue;
                }
                let q = ring.map(|k| c[k]);
                // Outward normal, in the kingdom's frame (x east, y up, z south).
                let mut n = cross(sub(q[1], q[0]), sub(q[3], q[0]));
                let mid = [0, 1, 2].map(|k| (q[0][k] + q[2][k]) / 2.0);
                if dot(n, sub(mid, centre)) < 0.0 {
                    n = n.map(|v| -v);
                }
                let len = dot(n, n).sqrt();
                if len < 1e-4 {
                    continue;
                }
                let n = n.map(|v| v / len);
                // Toward the viewer: south and up, equally.
                let facing = (n[2] + n[1]) * std::f32::consts::FRAC_1_SQRT_2;
                if facing <= 0.05 {
                    continue;
                }
                let top = n[1] > 0.7;
                let shade = if top { 1.18 } else { 0.92 + 0.12 * (n[2]).max(0.0) };
                let level = (tone * shade * 5.6 - 0.4).clamp(0.0, 5.0);
                self.quad(q, level, base, wet, top);
            }
        }
        self.laid = to;
    }

    /// Paint the ditch dug by the earth lifts done with `laid` stones: dark
    /// earth under everything, the deeper cut darker. Lifts already painted
    /// are left.
    fn dig(&mut self, castle: &Castle, laid: u32) {
        let done = castle.earth_done(laid);
        if done <= self.dug {
            return;
        }
        let (ox, oz) = crate::world::works::origin_px();
        for y in 0..self.img.h {
            for x in 0..self.img.w {
                // The picture's pixel as the castle's frame, on the floor.
                let (mx, my) = ((x as f32 + 0.5 + self.at.0 as f32 - ox) / PX_PER_M, -((y as f32 + 0.5 + self.at.1 as f32) - oz) / PX_PER_M);
                // The causeway before the gate is undug.
                if mx.abs() < 1.5 && my < 0.0 {
                    continue;
                }
                let mut deep = 0;
                for e in castle.earth.iter().filter(|e| e.at <= laid) {
                    if crate::world::works::dug(e, mx, my) {
                        deep += 1;
                    }
                }
                if deep > 0 {
                    // Under everything the stones draw.
                    let at = (y * self.img.w + x) as usize;
                    if self.depth[at] == f32::MIN {
                        self.img.set(x, y, ink(if deep > 1 { 'n' } else { 'Z' }).unwrap_or([20, 14, 8]));
                        self.depth[at] = -1.0e6;
                    }
                }
            }
        }
        self.dug = done;
    }

    /// A face as two triangles on the picture, nearer over farther.
    fn quad(&mut self, q: [[f32; 3]; 4], level: f32, base: Option<[f32; 3]>, wet: f32, top: bool) {
        let p = q.map(|v| [v[0] - self.at.0 as f32, v[2] - v[1] - self.at.1 as f32, v[2]]);
        let pick = |l: f32, edge: bool| -> Rgb {
            let k = (l.round() as usize).min(5);
            let Some(base) = base else {
                // Timber, in the map's browns; a joint a step darker.
                let ch = if edge { 'B' } else { ['B', 'P', 'r', 'o', 'O', 't'][k] };
                return ink(ch).unwrap_or([0, 0, 0]);
            };
            // A joint: a step darker on a face's low and right edge, darker
            // still while the mortar is wet.
            let shown = if edge { if wet > 0.3 { 0.14 } else { LEVELS[k.saturating_sub(1)] * 0.8 } } else { LEVELS[k] };
            crate::render::overworld::ink::nearest_plain(base.map(|c| (c * shown * 0.82 * 255.0).clamp(0.0, 255.0)))
        };
        let (xs, ys): (Vec<f32>, Vec<f32>) = (p.iter().map(|v| v[0]).collect(), p.iter().map(|v| v[1]).collect());
        let (x0, x1) = (xs.iter().cloned().fold(f32::MAX, f32::min), xs.iter().cloned().fold(f32::MIN, f32::max));
        let (y0, y1) = (ys.iter().cloned().fold(f32::MAX, f32::min), ys.iter().cloned().fold(f32::MIN, f32::max));
        let (ix0, ix1) = ((x0.round() as i32).max(0), (x1.round() as i32).min(self.img.w));
        let (iy0, iy1) = ((y0.round() as i32).max(0), (y1.round() as i32).min(self.img.h));
        for y in iy0..iy1 {
            for x in ix0..ix1 {
                let (fx, fy) = (x as f32 + 0.5, y as f32 + 0.5);
                // Inside either triangle of the quad (0 1 2) (0 2 3).
                let Some(depth) = depth_in(&p, fx, fy) else { continue };
                let at = (y * self.img.w + x) as usize;
                if depth + 0.001 < self.depth[at] {
                    continue;
                }
                // An edge: the face's last row or column (south and east).
                let edge = !top && (fy + 1.0 > y1.round() || fx + 1.0 > x1.round());
                let c = pick(level + if top { 0.0 } else { (fy - y0) / (y1 - y0).max(1.0) * -0.5 }, edge);
                self.depth[at] = depth;
                self.img.set(x, y, c);
            }
        }
    }
}

fn sub(a: [f32; 3], b: [f32; 3]) -> [f32; 3] {
    [a[0] - b[0], a[1] - b[1], a[2] - b[2]]
}
fn cross(a: [f32; 3], b: [f32; 3]) -> [f32; 3] {
    [a[1] * b[2] - a[2] * b[1], a[2] * b[0] - a[0] * b[2], a[0] * b[1] - a[1] * b[0]]
}
fn dot(a: [f32; 3], b: [f32; 3]) -> f32 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}

/// The depth (the floor's `z`) at picture point `(x, y)` if it is inside the
/// quad `p` (each corner `[x, y, depth]`), else `None`.
fn depth_in(p: &[[f32; 3]; 4], x: f32, y: f32) -> Option<f32> {
    for tri in [[0, 1, 2], [0, 2, 3]] {
        let (a, b, c) = (p[tri[0]], p[tri[1]], p[tri[2]]);
        let d = (b[1] - c[1]) * (a[0] - c[0]) + (c[0] - b[0]) * (a[1] - c[1]);
        if d.abs() < 1e-6 {
            continue;
        }
        let l1 = ((b[1] - c[1]) * (x - c[0]) + (c[0] - b[0]) * (y - c[1])) / d;
        let l2 = ((c[1] - a[1]) * (x - c[0]) + (a[0] - c[0]) * (y - c[1])) / d;
        let l3 = 1.0 - l1 - l2;
        if l1 >= -0.001 && l2 >= -0.001 && l3 >= -0.001 {
            return Some(l1 * a[2] + l2 * b[2] + l3 * c[2]);
        }
    }
    None
}

// ------------------------------------------------------------ the kept ----

/// The picture made so far (one a plan): grown while `laid` rises, made again
/// when it falls. Its stones' mortar is dried to the stone last drawn.
static KEPT: Mutex<Option<Flat>> = Mutex::new(None);

/// The Keep's picture as the run has it, a copy of what is built (cheap: the
/// picture is kept, a pointer to the last made).
pub fn flat(castle: &'static Castle, laid: u32) -> Arc<Flat> {
    static LAST: Mutex<Option<(&'static str, u32, Arc<Flat>)>> = Mutex::new(None);
    let laid = laid.min(castle.len());
    {
        let last = LAST.lock().unwrap_or_else(|e| e.into_inner());
        if let Some((plan, at, flat)) = last.as_ref()
            && *plan == castle.id
            && *at == laid
        {
            return Arc::clone(flat);
        }
    }
    let covers = works_mesh::covers(castle);
    let mut kept = KEPT.lock().unwrap_or_else(|e| e.into_inner());
    let rebuild = kept.as_ref().is_none_or(|f| f.plan != castle.id || f.laid > laid);
    if rebuild {
        *kept = Some(Flat::new(castle));
    }
    let flat = kept.as_mut().expect("made above");
    // Stones whose mortar was wet are drawn again as it dries: the last
    // stretch of what is drawn, then what is new.
    let from = flat.laid.saturating_sub(WET_STONES);
    flat.lay(castle, covers, from, laid, laid, &[], &[]);
    flat.dig(castle, laid);
    let out = Arc::new(Flat { plan: flat.plan, laid: flat.laid, at: flat.at, img: flat.img.clone(), depth: flat.depth.clone(), dug: flat.dug });
    *LAST.lock().unwrap_or_else(|e| e.into_inner()) = Some((castle.id.as_str(), laid, Arc::clone(&out)));
    out
}

/// [`flat`] with the stones `lost` (sorted) knocked out of it: made whole when the stones standing change,
/// kept while they do not.
pub fn flat_gapped(castle: &'static Castle, laid: u32, lost: &[u32]) -> Arc<Flat> {
    if lost.is_empty() {
        return flat(castle, laid);
    }
    type Gapped = Mutex<Option<(&'static str, u32, Vec<u32>, Arc<Flat>)>>;
    static GAPPED: Gapped = Mutex::new(None);
    let laid = laid.min(castle.len());
    {
        let kept = GAPPED.lock().unwrap_or_else(|e| e.into_inner());
        if let Some((plan, at, gone, flat)) = kept.as_ref()
            && *plan == castle.id
            && *at == laid
            && gone == lost
        {
            return Arc::clone(flat);
        }
    }
    let covers = works_mesh::covers(castle);
    let mut made = Flat::new(castle);
    made.lay(castle, covers, 0, laid, laid, lost, &[]);
    made.dig(castle, laid);
    let out = Arc::new(made);
    *GAPPED.lock().unwrap_or_else(|e| e.into_inner()) = Some((castle.id.as_str(), laid, lost.to_vec(), Arc::clone(&out)));
    out
}

impl Flat {
    /// The part of the castle that lies behind a knight whose feet are at
    /// kingdom pixel row `feet` (what is nearer or level with them is not
    /// in it), and the part in front of it, each as a picture of the view
    /// (`origin` its top left in the kingdom's pixels, `w` by `h`).
    ///
    /// What lies before the knight is opened round it by a stippled
    /// circle, as the 3D look keeps a knight in sight (`realm_look::REVEAL`):
    /// `knight` is where they stand on the view, if they are on it.
    pub fn split(&self, origin: (i32, i32), (w, h): (i32, i32), feet: f32, knight: Option<(i32, i32)>) -> (Img, Img) {
        let (mut back, mut front) = (Img::new(w, h), Img::new(w, h));
        let (dx, dy) = (self.at.0 - origin.0, self.at.1 - origin.1);
        let (x0, x1) = ((-dx).max(0), (self.img.w).min(w - dx));
        let (y0, y1) = ((-dy).max(0), (self.img.h).min(h - dy));
        for y in y0..y1 {
            for x in x0..x1 {
                let Some(c) = self.img.get(x, y) else { continue };
                if !self.img.solid(x, y) {
                    continue;
                }
                let d = self.depth[(y * self.img.w + x) as usize];
                if d > feet {
                    // Opened where it would hide the knight: a ring of dither.
                    if let Some((kx, ky)) = knight {
                        let (px, py) = (x + dx, y + dy);
                        let r = (((px - kx) as f32) / 17.0).hypot(((py - (ky - 9)) as f32) / 23.0);
                        if r < 0.65 || (r < 1.0 && super::ink::bayer(px, py) < (1.0 - r) / 0.35) {
                            continue;
                        }
                    }
                    front.set(x + dx, y + dy, c);
                } else {
                    back.set(x + dx, y + dy, c);
                }
            }
        }
        (back, front)
    }
}

// ----------------------------------------------------------- falsework ----

/// The falsework and the working folk at `laid`, as a picture of its own
/// with the same top left as the castle's; timber poles, decks and the
/// crane's frame, a stone on its rope.
pub fn flat_banded(castle: &'static Castle, laid: u32, lost: &[u32], bands: &[(u32, u8)]) -> Arc<Flat> {
    let laid = laid.min(castle.len());
    let covers = works_mesh::covers(castle);
    let mut made = Flat::new(castle);
    made.lay(castle, covers, 0, laid, laid, lost, bands);
    made.dig(castle, laid);
    Arc::new(made)
}

pub fn falsework(castle: &'static Castle, laid: u32, tick: u32) -> Img {
    falsework_built(castle, laid, laid, tick)
}

/// [`falsework`] with the carpenters' bays raised as far as stone `built`
/// (`rules::hands`); `tick` is the crane's clock in ticks.
pub fn falsework_built(castle: &'static Castle, laid: u32, built: u32, tick: u32) -> Img {
    let built = built.max(laid);
    let (at, (w, h)) = frame(castle);
    let mut im = Img::new(w, h);
    let project = |p: [f32; 3]| {
        let q = castle.world(p);
        ((q[0] - at.0 as f32).round() as i32, (q[2] - q[1] - at.1 as f32).round() as i32)
    };
    for f in castle.falsework.iter().filter(|f| f.appears <= built && laid < f.gone) {
        let [x0, y0, z0, x1, y1, z1] = f.bx.map(|v| f32::from(v) / 100.0);
        match f.kind {
            Work::Scaffold => {
                // Poles at the corners, a deck on top, a mid deck on tall bays.
                let near = [(x0, y0), (x1, y0)];
                for (x, y) in near {
                    let (px, py0) = project([x, y, z0]);
                    let (_, py1) = project([x, y, z1]);
                    im.line(px, py0, px, py1, 'P');
                }
                let (ax, ay) = project([x0, y0, z1]);
                let (bx, by) = project([x1, y0, z1]);
                im.line(ax, ay, bx, by, 'o');
                let (cx, cy) = project([x0, y1, z1]);
                im.line(ax, ay, cx, cy, 'r');
                if z1 - z0 > 2.0 {
                    let mid = (z0 + z1) / 2.0;
                    let (mx, my) = project([x0, y0, mid]);
                    let (nx, ny) = project([x1, y0, mid]);
                    im.line(mx, my, nx, ny, 'o');
                }
            }
            Work::Centring => {
                let (ax, ay) = project([x0, y0, z1]);
                let (bx, by) = project([x1, y0, z1]);
                im.line(ax, ay, bx, by, 'o');
                for (x, y) in [(x0, y0), (x1, y0)] {
                    let (px, py0) = project([x, y, z0]);
                    let (_, py1) = project([x, y, z1]);
                    im.line(px, py0, px, py1, 'P');
                }
            }
            // The crane stands and turns, drawn alive below.
            Work::Crane => {}
            Work::Other => {}
        }
    }
    // The crane: its mast to the top, the jib out, the stone on its rope.
    if let Some(c) = wl::crane(castle, laid, tick as f32 / crate::rules::HZ as f32) {
        let (mx0, my0) = project([c.at[0], c.at[1], 0.0]);
        let (mx1, my1) = project([c.at[0], c.at[1], c.top]);
        im.line(mx0 - 1, my0, mx1 - 1, my1, 'r');
        im.line(mx0, my0, mx1, my1, 'P');
        im.line(mx0 + 1, my0, mx1 + 1, my1, 'r');
        let tip = [c.at[0] + c.reach * c.yaw.cos(), c.at[1] + c.reach * c.yaw.sin(), c.top];
        let (jx, jy) = project(tip);
        im.line(mx1, my1, jx, jy, 'o');
        im.line(mx1, my1 + 1, jx, jy + 1, 'r');
        let (sx, sy) = project(c.stone);
        im.line(jx, jy, sx, sy - 3, 'b');
        im.rect(sx - 3, sy - 3, 7, 4, 'U');
        im.rect(sx - 3, sy - 3, 7, 1, 'v');
        // The treadwheel at the mast's foot.
        let (wx, wy) = project([c.at[0] - 0.9, c.at[1], 1.2]);
        for k in 0..4 {
            let a = c.wheel + k as f32 * std::f32::consts::FRAC_PI_2;
            im.line(wx, wy, wx + (a.cos() * 8.0) as i32, wy - (a.sin() * 8.0) as i32, 'o');
        }
    }
    // The benches the masons cut at, and the heap in the yard.
    if laid < castle.len() {
        let seconds = tick as f32 / crate::rules::HZ as f32;
        for b in wl::BANKERS {
            let (x, y) = project([b[0], b[1], 0.0]);
            im.rect(x - 8, y - 9, 16, 4, 'o');
            im.rect(x - 8, y - 5, 16, 5, 'P');
            im.rect(x - 4, y - 12, 8, 4, 'U');
            im.rect(x - 4, y - 12, 8, 1, 'v');
        }
        for (dx, dy, dz) in [(0.0f32, 0.0f32, 0.0f32), (1.3, 0.1, 0.0), (0.6, 0.0, 0.6), (-1.2, 0.5, 0.0)] {
            let (x, y) = project([wl::YARD[0] + dx, wl::YARD[1] + dy, dz]);
            im.rect(x - 7, y - 7, 14, 7, 'u');
            im.rect(x - 7, y - 7, 14, 2, 'U');
        }
        let _ = seconds;
    }
    // Dust where the last stones landed.
    for p in wl::dust(castle, laid) {
        let (x, y) = project(p.at);
        let r = (p.radius * PX_PER_M * 0.5).max(1.0);
        if p.thick > 0.18 {
            im.ellipse(x as f32, y as f32, r, r * 0.8, if p.thick > 0.4 { 'H' } else { 'i' });
        }
    }
    im
}

/// Whether the Keep's site is in the picture at all: its box touches the view.
pub fn in_view(castle: &Castle, origin: (i32, i32), (w, h): (i32, i32)) -> bool {
    let (at, (fw, fh)) = frame(castle);
    at.0 < origin.0 + w && at.0 + fw > origin.0 && at.1 < origin.1 + h && at.1 + fh > origin.1
}
