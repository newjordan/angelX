//! The Colosseum fight pit, in true 3D and painted by where each dot lands.
//!
//! The race raster gives the pit its shape and its depth: a sand floor and
//! walls that rise out of it, no roof. Everything a player reads comes from
//! here, a dot at a time, from the world point the dot sees: ashlar with
//! age, arched cells, banners and sconces, a gate with its portcullis up,
//! pillars with capitals, a gallery of crowd above the cornice, sand raked
//! and scuffed and stained, pools of torchlight that stop at a pillar, haze
//! with depth, a smoke-dusk sky. Every colour is one of the realm's inks,
//! reached through an ordered dither.
use super::super::super::ink::Img;
use crate::render::world3d::{
    math::{V3, v3},
    mesh::{Mesh, mat},
    raster::View3,
};
use crate::rules::deathmatch as dm;
use std::sync::OnceLock;

mod light;
mod palette;
mod sand;
mod sky;
mod stone;

pub(super) use light::Flash;
const COLS: usize = 24;
const ROWS: usize = 18;
const CELL: f32 = 2.0;
/// How tall the ring wall stands (gallery, parapet and pennants), and the pillars inside it.
const OUTER_H: f32 = 6.4;
const PILLAR_H: f32 = 3.4;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Kind {
    /// The ring wall: cells, banners, sconces, the gallery above.
    Outer,
    /// A free-standing block: ashlar and a capital.
    Pillar,
    /// A gate in a notch of the ring, portcullis raised.
    Gate,
    /// The box that juts out of the north and south walls.
    Box,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Centre {
    None,
    Banner,
    Wide,
    Sconce,
}

/// One flat run of wall facing the open pit.
struct Face {
    /// 0: the plane is `x = at`, running along y; 1: `y = at`, along x.
    axis: u8,
    at: f32,
    lo: f32,
    hi: f32,
    /// Which way the open pit lies from the plane: `1.0` or `-1.0`.
    open: f32,
    h: f32,
    kind: Kind,
    /// How high the cornice runs (the gallery sits on it).
    gal: f32,
    seed: u32,
    centre: Centre,
    /// What hangs from the wall, by how far along it: `(u, what)`.
    posts: Vec<(f32, Centre)>,
    /// Where the arched cells open, by how far along the wall.
    alcoves: Vec<f32>,
    tex: stone::Tex,
    /// By tenths along the face: 1 where a post hangs, 2 where a lit cell glows.
    feat: Vec<u8>,
}

impl Face {
    fn post_at(&self, u: f32) -> Option<(f32, Centre, u32)> {
        self.posts.iter().enumerate().find(|(_, p)| (u - p.0).abs() < 0.95).map(|(i, p)| (u - p.0, p.1, self.seed.wrapping_add(i as u32 * 31)))
    }
    fn alcove_at(&self, u: f32) -> Option<(f32, u32)> {
        self.alcoves.iter().enumerate().find(|(_, c)| (u - **c).abs() < 1.25).map(|(i, c)| (u - c, self.seed.wrapping_add(i as u32 * 17)))
    }
    fn pole_posts(&self) -> impl Iterator<Item = f32> + '_ {
        self.posts.iter().filter(|p| p.1 == Centre::Banner).map(|p| p.0)
    }
}

/// A dot's worth of a surface before the light reaches it: the colour it
/// would be in an even light, and what it gives off itself.
struct Texel {
    a: [f32; 3],
    e: [f32; 3],
}

/// The view's size in dots, for what is laid on the sky.
struct Frame {
    px_per_rad: f32,
}

struct Pit {
    faces: Vec<Face>,
    xf: Vec<u16>,
    yf: Vec<u16>,
    lights: light::Lights,
    floor: sand::Floor,
    mesh: Mesh,
}

const NONE: u16 = u16::MAX;

type Job<'a> = Box<dyn FnOnce() + Send + 'a>;

/// Runs the jobs side by side, on threads wherever the platform has them.
fn together(jobs: Vec<Job<'_>>) {
    #[cfg(not(target_arch = "wasm32"))]
    std::thread::scope(|scope| {
        for job in jobs {
            scope.spawn(job);
        }
    });
    #[cfg(target_arch = "wasm32")]
    for job in jobs {
        job();
    }
}

fn raw_wall(c: i32, r: i32) -> bool {
    if !(0..COLS as i32).contains(&c) || !(0..ROWS as i32).contains(&r) {
        return true;
    }
    dm::MAP[r as usize].as_bytes()[c as usize] == b'#'
}


/// Wall cells joined to the border by wall: the ring (and what hangs off it).
fn ring() -> [[bool; COLS]; ROWS] {
    let mut out = [[false; COLS]; ROWS];
    let mut todo: Vec<(i32, i32)> = Vec::new();
    for c in 0..COLS as i32 {
        todo.extend([(c, 0), (c, ROWS as i32 - 1)]);
    }
    for r in 0..ROWS as i32 {
        todo.extend([(0, r), (COLS as i32 - 1, r)]);
    }
    while let Some((c, r)) = todo.pop() {
        if !raw_wall(c, r) || !(0..COLS as i32).contains(&c) || !(0..ROWS as i32).contains(&r) || out[r as usize][c as usize] {
            continue;
        }
        out[r as usize][c as usize] = true;
        todo.extend([(c + 1, r), (c - 1, r), (c, r + 1), (c, r - 1)]);
    }
    out
}

fn u01(h: u32) -> f32 {
    (h & 0xffff) as f32 / 65535.0
}

fn dress(f: &mut Face) {
    let len = f.hi - f.lo;
    match f.kind {
        Kind::Pillar => {
            f.centre = if u01(f.seed >> 3) < 0.7 { Centre::Banner } else { Centre::None };
        }
        Kind::Gate => {}
        Kind::Box => {
            f.centre = Centre::Wide;
            f.posts = vec![(0.45, Centre::Sconce), (len * 0.5, Centre::Wide), (len - 0.45, Centre::Sconce)];
        }
        Kind::Outer => {
            let bays = ((len / 6.0).round() as usize).max(1);
            if bays >= 2 && len >= 8.0 {
                f.alcoves = (0..bays).map(|k| len * (k as f32 + 0.5) / bays as f32).collect();
                f.posts = (1..bays).map(|k| (len * k as f32 / bays as f32, if k % 2 == 1 { Centre::Sconce } else { Centre::Banner })).collect();
            } else {
                f.centre = Centre::Sconce;
                f.posts = vec![(len * 0.5, Centre::Sconce)];
            }
        }
    }
    let n = (len * 10.0).ceil() as usize + 1;
    f.feat = (0..n)
        .map(|k| {
            let u = k as f32 * 0.1;
            let mut m = 0u8;
            if f.kind == Kind::Pillar {
                m |= u8::from(f.centre == Centre::Banner);
            } else {
                m |= u8::from(f.post_at(u).is_some());
                m |= 2 * u8::from(f.alcove_at(u).is_some_and(|(_, seed)| seed % 3 == 0));
            }
            m
        })
        .collect();
}

fn build() -> Pit {
    let outer = ring();
    let mut faces: Vec<Face> = Vec::new();
    let mut xf = vec![NONE; (COLS + 1) * ROWS];
    let mut yf = vec![NONE; (ROWS + 1) * COLS];
    let cell_outer = |c: i32, r: i32| outer[r as usize][c as usize];
    // x planes, running along y.
    for i in 0..=COLS as i32 {
        let mut run: Option<(i32, i32, f32, bool)> = None;
        let flush = |faces: &mut Vec<Face>, xf: &mut Vec<u16>, run: Option<(i32, i32, f32, bool)>| {
            let Some((a, b, open, out)) = run else { return };
            let kind = if !out {
                Kind::Pillar
            } else if (i == 3 || i == 21) && (7..10).contains(&a) {
                Kind::Gate
            } else {
                Kind::Outer
            };
            let id = faces.len() as u16;
            for r in a..b {
                xf[i as usize * ROWS + r as usize] = id;
            }
            let h = if out { OUTER_H } else { PILLAR_H };
            let mut f = Face {
                axis: 0,
                at: i as f32 * CELL,
                lo: a as f32 * CELL,
                hi: b as f32 * CELL,
                open,
                h,
                kind,
                seed: crate::render::overworld::ink::hash(i, a, 0xf1),
                gal: if kind == Kind::Gate { 3.95 } else { 2.78 },
                centre: Centre::None,
                posts: Vec::new(),
                alcoves: Vec::new(),
                tex: stone::Tex::empty(),
                feat: Vec::new(),
            };
            dress(&mut f);
            faces.push(f);
        };
        for r in 0..ROWS as i32 {
            let (wl, wr) = (raw_wall(i - 1, r), raw_wall(i, r));
            if wl == wr {
                flush(&mut faces, &mut xf, run.take());
                continue;
            }
            let (open, out) = if wl { (1.0, cell_outer(i - 1, r)) } else { (-1.0, cell_outer(i, r)) };
            match run {
                Some((a, b, o, ou)) if b == r && o == open && ou == out => run = Some((a, r + 1, o, ou)),
                _ => {
                    flush(&mut faces, &mut xf, run.take());
                    run = Some((r, r + 1, open, out));
                }
            }
        }
        flush(&mut faces, &mut xf, run.take());
    }
    // y planes, running along x.
    for j in 0..=ROWS as i32 {
        let mut run: Option<(i32, i32, f32, bool)> = None;
        let flush = |faces: &mut Vec<Face>, yf: &mut Vec<u16>, run: Option<(i32, i32, f32, bool)>| {
            let Some((a, b, open, out)) = run else { return };
            let kind = if !out {
                Kind::Pillar
            } else if (j == 3 || j == 15) && (11..13).contains(&a) {
                Kind::Box
            } else {
                Kind::Outer
            };
            let id = faces.len() as u16;
            for c in a..b {
                yf[j as usize * COLS + c as usize] = id;
            }
            let h = if out { OUTER_H } else { PILLAR_H };
            let mut f = Face {
                axis: 1,
                at: j as f32 * CELL,
                lo: a as f32 * CELL,
                hi: b as f32 * CELL,
                open,
                h,
                kind,
                seed: crate::render::overworld::ink::hash(j, a, 0xf2),
                gal: if kind == Kind::Gate { 3.95 } else { 2.78 },
                centre: Centre::None,
                posts: Vec::new(),
                alcoves: Vec::new(),
                tex: stone::Tex::empty(),
                feat: Vec::new(),
            };
            dress(&mut f);
            faces.push(f);
        };
        for c in 0..COLS as i32 {
            let (wt, wb) = (raw_wall(c, j - 1), raw_wall(c, j));
            if wt == wb {
                flush(&mut faces, &mut yf, run.take());
                continue;
            }
            let (open, out) = if wt { (1.0, cell_outer(c, j - 1)) } else { (-1.0, cell_outer(c, j)) };
            match run {
                Some((a, b, o, ou)) if b == c && o == open && ou == out => run = Some((a, c + 1, o, ou)),
                _ => {
                    flush(&mut faces, &mut yf, run.take());
                    run = Some((c, c + 1, open, out));
                }
            }
        }
        flush(&mut faces, &mut yf, run.take());
    }

    let solid = |c: i32, r: i32| raw_wall(c, r);

    // The mesh: the floor in one sheet, every face a quad. No tops, no roof.
    let mut mesh = Mesh::new();
    let (w, d) = (COLS as f32 * CELL, ROWS as f32 * CELL);
    mesh.push_quad(v3(0., 0., 0.), v3(w, 0., 0.), v3(w, d, 0.), v3(0., d, 0.), mat::FLOOR);
    for f in &faces {
        let (a, b) = if f.axis == 0 { (v3(f.at, f.lo, 0.), v3(f.at, f.hi, 0.)) } else { (v3(f.lo, f.at, 0.), v3(f.hi, f.at, 0.)) };
        mesh.push_quad(a, b, v3(b.x, b.y, f.h), v3(a.x, a.y, f.h), mat::STONE);
    }

    // The lights: braziers on the sand, a sconce wherever one hangs.
    let mut list: Vec<light::Light> = Vec::new();
    for &(x, y) in &super::world::BRAZIERS {
        list.push(light::Light { x, y, z: 1.2, r2: 289.0, inv_r2: 1.0 / 289.0, k: 0.95, tint: [1.0, 0.74, 0.48], brazier: true });
    }
    for f in &faces {
        for &(u, what) in &f.posts {
            if what != Centre::Sconce {
                continue;
            }
            let (x, y) = if f.axis == 0 { (f.at + f.open * 0.26, f.lo + u) } else { (f.lo + u, f.at + f.open * 0.26) };
            list.push(light::Light { x, y, z: 2.15, r2: 196.0, inv_r2: 1.0 / 196.0, k: 0.85, tint: [1.0, 0.76, 0.52], brazier: false });
        }
    }
    // The slow part, once: the quantizing cube, every light's reach, the floor and every wall baked.
    let open = |x: f32, y: f32| !raw_wall((x / CELL).floor() as i32, (y / CELL).floor() as i32);
    let (mut lights, mut floor) = (None, None);
    let per = faces.len().div_ceil(4);
    let mut jobs: Vec<Job> = vec![Box::new(palette::warm), Box::new(|| lights = Some(light::Lights::build(list, &solid))), Box::new(|| floor = Some(sand::Floor::bake(&solid, &sand::Decals::build(&open))))];
    for chunk in faces.chunks_mut(per) {
        jobs.push(Box::new(move || chunk.iter_mut().for_each(|f| f.tex = stone::Tex::bake(f))));
    }
    together(jobs);
    let (lights, floor) = (lights.expect("lights"), floor.expect("floor"));
    Pit { faces, xf, yf, lights, floor, mesh }
}

fn pit() -> &'static Pit {
    static PIT: OnceLock<Pit> = OnceLock::new();
    PIT.get_or_init(build)
}

/// The pit's shape for the raster.
pub(super) fn mesh() -> &'static Mesh {
    &pit().mesh
}

/// The camera the raster looks through, rebuilt here to read depth back out.
struct Cam {
    pos: V3,
    right: V3,
    up: V3,
    fwd: V3,
    tan_h: f32,
    tan_v: f32,
    half_w: f32,
    half_h: f32,
}

impl Cam {
    fn new(view: &View3, w: usize, h: usize) -> Cam {
        let (sin_h, cos_h) = view.heading_rad.sin_cos();
        let (sin_p, cos_p) = view.pitch.clamp(-1.35, 1.35).sin_cos();
        let right = v3(-sin_h, cos_h, 0.0);
        let fwd = v3(cos_h * cos_p, sin_h * cos_p, sin_p);
        let up = fwd.cross(right).normalize();
        let tan_h = (view.fov_rad.clamp(0.20, 2.40) * 0.5).tan();
        Cam { pos: view.pos, right, up, fwd, tan_h, tan_v: tan_h * h as f32 / w.max(1) as f32, half_w: w as f32 * 0.5, half_h: h as f32 * 0.5 }
    }
}

fn mix(a: [f32; 3], b: [f32; 3], t: f32) -> [f32; 3] {
    [a[0] + (b[0] - a[0]) * t, a[1] + (b[1] - a[1]) * t, a[2] + (b[2] - a[2]) * t]
}

const AMBIENT_COOL: [f32; 3] = [0.25, 0.27, 0.34];
const AMBIENT_WARM: [f32; 3] = [0.42, 0.31, 0.23];

/// The pit as the raster saw it (`rgba` and `depth`, `w` by `h` dots),
/// painted: `fever` is how loudly the crowd is up (`0..=1`) and `feet` are
/// where the knights stand, each in a small shadow.
#[allow(clippy::too_many_arguments)]
pub(super) fn paint(rgba: &[u8], depth: &[f32], view: &View3, ticks: u32, fever: f32, feet: &[(f32, f32)], flashes: &[Flash], w: usize, h: usize) -> Img {
    let pit = pit();
    let cam = Cam::new(view, w, h);
    let flick = pit.lights.flicker(ticks);
    let q = palette::quant();
    let frame = Frame { px_per_rad: w as f32 / view.fov_rad.max(0.2) };
    let col_dir: Vec<V3> = (0..w).map(|x| cam.right * (((x as f32 + 0.5) / cam.half_w - 1.0) * cam.tan_h)).collect();
    // Rows are independent, so the frame is painted in four bands side by side, as the raster fills it.
    let mut px = vec![[0u8; 3]; w * h];
    let rows_per = h.div_ceil(4).max(1);
    let (cam, col_dir, flick, frame) = (&cam, &col_dir, &flick, &frame);
    let jobs: Vec<Job> = px
        .chunks_mut(rows_per * w)
        .enumerate()
        .map(|(k, band)| {
            Box::new(move || {
                for (r, out) in band.chunks_mut(w).enumerate() {
                    let py = k * rows_per + r;
                    let row = cam.fwd + cam.up * ((1.0 - (py as f32 + 0.5) / cam.half_h) * cam.tan_v);
                    // Light changes slowly: a run of dots on one surface shares it.
                    let mut cached: (u32, i32, i32, i32, [f32; 3]) = (u32::MAX, 0, 0, 0, [0.0; 3]);
                    for (x, dot) in out.iter_mut().enumerate() {
                        let i = py * w + x;
                        let d = depth[i];
                        let dir = row + col_dir[x];
                        let rgb = if !d.is_finite() {
                            sky::color([dir.x, dir.y, dir.z], ticks, frame)
                        } else {
                            let p = v3(cam.pos.x + dir.x * d, cam.pos.y + dir.y * d, cam.pos.z + dir.z * d);
                            match surface(pit, p, d, ticks, fever, feet, flashes, &rgba[i * 4..i * 4 + 3], flick, &mut cached) {
                                Some(c) => c,
                                None => sky::color([dir.x, dir.y, dir.z], ticks, frame),
                            }
                        };
                        *dot = q.dot(rgb, x as i32, py as i32, 22.0);
                    }
                }
            }) as Job
        })
        .collect();
    together(jobs);
    Img::from_rgb(w as i32, h as i32, &px)
}

/// The colour, before the dither, of what is at `p` (seen from `d` away).
#[allow(clippy::too_many_arguments)]
fn surface(pit: &Pit, p: V3, d: f32, t: u32, fever: f32, feet: &[(f32, f32)], flashes: &[Flash], seen: &[u8], flick: &[f32], cached: &mut (u32, i32, i32, i32, [f32; 3])) -> Option<[f32; 3]> {
    let (texel, n, ground, amb_k, sid) = if p.z.abs() < 0.03 {
        let a = pit.floor.at(p.x, p.y, d);
        let k = sand::shadowed(p.x, p.y, feet);
        (Texel { a: [a[0] * k, a[1] * k, a[2] * k], e: [0.0; 3] }, [0.0, 0.0, 1.0], (p.x, p.y), 1.0, 0u32)
    } else {
        let found = find_face(pit, p);
        match found {
            Some((id, u)) => {
                let f = &pit.faces[id];
                let texel = stone::wall(f, u, p.z, d, t, fever)?;
                let n = if f.axis == 0 { [f.open, 0.0, 0.0] } else { [0.0, f.open, 0.0] };
                let ground = (p.x + n[0] * 0.3, p.y + n[1] * 0.3);
                (texel, n, ground, if f.axis == 0 { 1.0 } else { 0.85 }, 1 + id as u32)
            }
            None => {
                // Something the plan does not know (a hedge a knight raised): the raster's own tone.
                let l = (f32::from(seen[0]) * 0.3 + f32::from(seen[1]) * 0.5 + f32::from(seen[2]) * 0.2) / 255.0;
                (Texel { a: [0.46 * l * 2.2, 0.41 * l * 2.2, 0.34 * l * 2.2], e: [0.0; 3] }, [0.0, 0.0, 1.0], (p.x, p.y), 1.0, u32::MAX - 1)
            }
        }
    };
    let glow = ((p.z - 2.2) / 2.0).clamp(0.0, 1.0);
    let amb = mix(AMBIENT_COOL, AMBIENT_WARM, glow);
    // Far off, light is smooth: the coarser the farther.
    let q = 1.0 / (0.18 + 0.025 * d);
    let key = (sid, (p.x * q) as i32, (p.y * q) as i32, (p.z * q) as i32);
    if cached.0 != key.0 || cached.1 != key.1 || cached.2 != key.2 || cached.3 != key.3 {
        *cached = (key.0, key.1, key.2, key.3, pit.lights.at([p.x, p.y, p.z], n, ground, flick, flashes));
    }
    let lit = cached.4;
    // Lit stone is not allowed to blaze: a soft shoulder keeps every torch pool a tone the knights
    // stand out of. What a thing gives off itself (a flame) goes on after, and may.
    let shoulder = |v: f32| {
        let x = (v - 0.35).max(0.0);
        v.min(0.35) + 0.75 * x / (1.0 + 1.4 * x)
    };
    let mut c = [
        shoulder(texel.a[0] * (amb[0] * amb_k + lit[0])) + texel.e[0],
        shoulder(texel.a[1] * (amb[1] * amb_k + lit[1])) + texel.e[1],
        shoulder(texel.a[2] * (amb[2] * amb_k + lit[2])) + texel.e[2],
    ];
    let fog = ((d - 6.0) / 44.0).clamp(0.0, 1.0);
    let fog = fog * fog * (3.0 - 2.0 * fog) * 0.7;
    c = mix(c, sky::HAZE, fog);
    Some(c)
}

fn find_face(pit: &Pit, p: V3) -> Option<(usize, f32)> {
    let (fx, fy) = (p.x * 0.5, p.y * 0.5);
    let (ix, iy) = (fx.round(), fy.round());
    let (ex, ey) = ((fx - ix).abs() * 2.0, (fy - iy).abs() * 2.0);
    let cell = |v: f32, max: usize| (v.floor().max(0.0) as usize).min(max - 1);
    if ex < 0.025 && ex <= ey {
        let id = pit.xf[(ix.max(0.0) as usize).min(COLS) * ROWS + cell(fy, ROWS)];
        if id != NONE {
            let f = &pit.faces[usize::from(id)];
            return (p.z <= f.h).then_some((usize::from(id), p.y - f.lo));
        }
    }
    if ey < 0.025 {
        let id = pit.yf[(iy.max(0.0) as usize).min(ROWS) * COLS + cell(fx, COLS)];
        if id != NONE {
            let f = &pit.faces[usize::from(id)];
            return (p.z <= f.h).then_some((usize::from(id), p.x - f.lo));
        }
    }
    None
}

#[cfg(test)]
#[path = "../../../../../tests/render/arena__deathmatch_pit__tests.rs"]
mod tests;
