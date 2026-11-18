//! Rasterizer core — OWNED BY THE RENDERER AGENT (docs/plans/world3d-spec.md).
//! Triangle rasterizer with z-buffer, shading to RGBA at braille-dot
//! resolution; frames flow through ride.rs's frame_to_braille_graded bridge.
//!
//! The pipeline is the classic one, kept deliberately small so it stays inside
//! the toy pane's budget (≤ ~3 ms at 144×104 dots, ≤ ~10 ms at 200×304):
//!
//! ```text
//! Mesh (world space, +z up, 1.0 = one map tile)
//!   └─ camera basis from View3          (yaw matches the raycaster's plane)
//!        └─ per-triangle → camera space (right/up/forward dot products)
//!             └─ near-plane clip at Z_NEAR, splitting into a fan
//!                  └─ screen-space edge functions + f32 z-buffer
//!                       └─ per-pixel shade: moon key + ambient + fog
//!   misses → the noir night sky (zenith→horizon, stars, moon, halo)
//! ```
//!
//! Reflected planes use cool shadows and warm keys; cached world-UV materials
//! keep the live shader small. The 3D braille bridge carries smooth value in
//! its coloured ink, retaining the black terminal around each silhouette.
//!
//! DETERMINISM LAW: byte-identical output for identical inputs. Triangles are
//! consumed in `Mesh::tris` order, every hash is SplitMix64 over integers, and
//! nothing here reads a clock, an env var, or an unordered collection.

use super::math::{V3, v3};
use super::mesh::{Mesh, mat};
use image::RgbaImage;

#[derive(Clone, Copy, Debug)]
pub struct View3 {
    pub pos: V3,
    pub heading_rad: f32,
    pub pitch: f32,
    pub fov_rad: f32,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Firelight {
    pub gain: f32,
    pub warm: f32,
}

impl Firelight {
    pub const STEADY: Firelight = Firelight {
        gain: 1.0,
        warm: 0.0,
    };

    fn is_steady(self) -> bool {
        self.gain == 1.0 && self.warm == 0.0
    }
}

#[derive(Clone, Debug)]
pub struct Painted {
    tables: Vec<Option<Mips>>,
    pub coarser_from: [f32; 3],
}

#[derive(Clone, Debug)]
struct Mips([Box<[[u8; 3]]>; 4]);

impl Default for Painted {
    fn default() -> Painted {
        Painted {
            tables: Vec::new(),
            coarser_from: [14.0, 28.0, 56.0],
        }
    }
}

impl Painted {
    pub const SIDE: usize = 256;

    pub fn new() -> Painted {
        Painted::default()
    }

    pub fn set(&mut self, material: u8, table: Vec<[u8; 3]>) {
        assert_eq!(
            table.len(),
            Self::SIDE * Self::SIDE,
            "a painted table is {0} by {0}",
            Self::SIDE
        );
        assert!(
            material != mat::MOONLIGHT && material != mat::TORCH_POOL,
            "material {material} is not a baked table"
        );
        let mut levels = vec![table.into_boxed_slice()];
        for k in 1..4 {
            let side = Self::SIDE >> k;
            let above = &levels[k - 1];
            let mut level = Vec::with_capacity(side * side);
            for y in 0..side {
                for x in 0..side {
                    let at = |dx: usize, dy: usize| above[(y * 2 + dy) * (side * 2) + x * 2 + dx];
                    let four = [at(0, 0), at(1, 0), at(0, 1), at(1, 1)];
                    level.push(std::array::from_fn(|c| {
                        ((four.iter().map(|p| u32::from(p[c])).sum::<u32>() + 2) / 4) as u8
                    }));
                }
            }
            levels.push(level.into_boxed_slice());
        }
        let at = usize::from(material);
        if self.tables.len() <= at {
            self.tables.resize(at + 1, None);
        }
        let [a, b, c, d]: [Box<[[u8; 3]]>; 4] = levels.try_into().expect("four levels");
        self.tables[at] = Some(Mips([a, b, c, d]));
    }

    pub fn patch(&mut self, material: u8, f: impl Fn(usize, usize) -> Option<[u8; 3]>) {
        let Some(Some(mips)) = self.tables.get(usize::from(material)) else {
            return;
        };
        let mut table = mips.0[0].to_vec();
        for (i, rgb) in table.iter_mut().enumerate() {
            if let Some(c) = f(i % Self::SIDE, i / Self::SIDE) {
                *rgb = c;
            }
        }
        self.set(material, table);
    }

    fn sample(&self, material: u8, x: usize, y: usize, depth: f32) -> Option<[u8; 3]> {
        let mips = self.tables.get(usize::from(material))?.as_ref()?;
        let level = self.coarser_from.iter().filter(|&&d| depth >= d).count();
        let side = Self::SIDE >> level;
        Some(mips.0[level][(y >> level) * side + (x >> level)])
    }
}

fn flickers(material: u8) -> bool {
    matches!(material, mat::FIRE | mat::FLAME | mat::EMBER | mat::TORCH_POOL)
}

const Z_NEAR: f32 = 0.05;
pub const FOG_DISTANCE: f32 = 52.0;
const LANTERN_REACH: f32 = 5.5;
const LANTERN_GAIN: f32 = 1.05;

const NIGHT_HAZE: [u8; 3] = super::NOIR_NIGHT_HAZE;
const SKY_ZENITH: [u8; 3] = super::NOIR_SKY_ZENITH;
const SKY_HORIZON: [u8; 3] = super::NOIR_SKY_HORIZON;
const MOON_SILVER: [u8; 3] = super::NOIR_MOON_SILVER;

const MOON_BEARING: f32 = super::MOON_BEARING;
const MOON_ELEVATION_RAD: f32 = 0.29;
const MOON_RADIUS_RAD: f32 = 0.052;
const MOON_HALO_RAD: f32 = 0.16;
const SKY_BAND_RAD: f32 = 0.55;
const HORIZON_GLOW_RAD: f32 = 0.17;
const HORIZON_GLOW: [u8; 3] = [32, 44, 66];
const HORIZON_GLOW_GAIN: f32 = 0.52;

const STAR_SEED: u64 = 0x574f_524c_4433_445f;

thread_local! {
    static ROLL: std::cell::Cell<f32> = const { std::cell::Cell::new(0.0) };
}

pub fn current_roll() -> f32 { ROLL.with(|r| r.get()) }

#[must_use]
pub struct RollGuard(f32);
pub fn roll(r: f32) -> RollGuard { RollGuard(ROLL.with(|c| c.replace(r))) }
impl Drop for RollGuard {
    fn drop(&mut self) { ROLL.with(|c| c.set(self.0)); }
}

pub fn rolled(right: V3, up: V3, roll: f32) -> (V3, V3) {
    if roll == 0.0 { return (right, up); }
    let (s, c) = roll.sin_cos();
    (right * c - up * s, up * c + right * s)
}

struct Camera {
    pos: V3,
    right: V3,
    up: V3,
    fwd: V3,
    tan_h: f32,
    tan_v: f32,
    half_w: f32,
    half_h: f32,
}

impl Camera {
    fn new(view: &View3, dot_w: usize, dot_h: usize) -> Camera {
        let (sin_h, cos_h) = view.heading_rad.sin_cos();
        let pitch = view.pitch.clamp(-1.35, 1.35);
        let (sin_p, cos_p) = pitch.sin_cos();
        let right = v3(-sin_h, cos_h, 0.0);
        let fwd = v3(cos_h * cos_p, sin_h * cos_p, sin_p);
        let up = fwd.cross(right).normalize();
        let (right, up) = rolled(right, up, current_roll());
        let tan_h = (view.fov_rad.clamp(0.20, 2.40) * 0.5).tan();
        let aspect = dot_h as f32 / dot_w.max(1) as f32;
        Camera {
            pos: view.pos,
            right,
            up,
            fwd,
            tan_h,
            tan_v: tan_h * aspect,
            half_w: dot_w as f32 * 0.5,
            half_h: dot_h as f32 * 0.5,
        }
    }

    fn to_camera(&self, p: V3) -> V3 {
        let d = p - self.pos;
        v3(d.dot(self.right), d.dot(self.up), d.dot(self.fwd))
    }

    fn project(&self, c: V3) -> (f32, f32) {
        let inv = 1.0 / c.z;
        (
            self.half_w * (1.0 + c.x * inv / self.tan_h),
            self.half_h * (1.0 - c.y * inv / self.tan_v),
        )
    }

    fn ray(&self, px: usize, py: usize) -> V3 {
        let ndc_x = (px as f32 + 0.5) / self.half_w - 1.0;
        let ndc_y = 1.0 - (py as f32 + 0.5) / self.half_h;
        (self.fwd + self.right * (ndc_x * self.tan_h) + self.up * (ndc_y * self.tan_v)).normalize()
    }
}

#[derive(Clone, Copy)]
struct ScreenVertex {
    x: f32,
    y: f32,
    inv_z: f32,
    u_over_z: f32,
    v_over_z: f32,
}

#[derive(Clone, Copy)]
struct ClipVertex {
    p: V3,
    uv: [f32; 2],
}

fn lerp_clip(a: ClipVertex, b: ClipVertex, t: f32) -> ClipVertex {
    ClipVertex {
        p: a.p.lerp(b.p, t),
        uv: [
            a.uv[0] + (b.uv[0] - a.uv[0]) * t,
            a.uv[1] + (b.uv[1] - a.uv[1]) * t,
        ],
    }
}

fn clip_near(tri: [ClipVertex; 3], out: &mut [ClipVertex; 4]) -> usize {
    let mut count = 0usize;
    for i in 0..3 {
        let current = tri[i];
        let next = tri[(i + 1) % 3];
        let current_in = current.p.z >= Z_NEAR;
        let next_in = next.p.z >= Z_NEAR;
        if current_in {
            out[count] = current;
            count += 1;
        }
        if current_in != next_in {
            let denominator = next.p.z - current.p.z;
            let t = if denominator.abs() > 1e-12 {
                (Z_NEAR - current.p.z) / denominator
            } else {
                0.0
            };
            out[count] = lerp_clip(current, next, t.clamp(0.0, 1.0));
            count += 1;
        }
        if count == 4 {
            break;
        }
    }
    count
}

pub fn render_scene(
    scene: &Mesh,
    view: &View3,
    dot_w: usize,
    dot_h: usize,
) -> image::RgbaImage {
    render_scene_lit(scene, view, dot_w, dot_h, Firelight::STEADY)
}

pub fn render_scene_fogged(
    scene: &Mesh,
    view: &View3,
    dot_w: usize,
    dot_h: usize,
    fire: Firelight,
    fog: f32,
) -> image::RgbaImage {
    render_fogged(scene, view, dot_w, dot_h, fire, fog)
}

pub fn render_scene_lit(
    scene: &Mesh,
    view: &View3,
    dot_w: usize,
    dot_h: usize,
    fire: Firelight,
) -> image::RgbaImage {
    render_fogged(scene, view, dot_w, dot_h, fire, FOG_DISTANCE)
}

fn render_fogged(
    scene: &Mesh,
    view: &View3,
    dot_w: usize,
    dot_h: usize,
    fire: Firelight,
    fog: f32,
) -> image::RgbaImage {
    render_meshes(&[scene], view, dot_w, dot_h, fire, fog, Backdrop::Night).0
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Backdrop {
    Night,
    Dark([u8; 3]),
    Haze([u8; 3]),
}

const BAND_ROWS: usize = 32;
const MAX_BANDS: usize = 8;

pub fn render_meshes(
    meshes: &[&Mesh],
    view: &View3,
    dot_w: usize,
    dot_h: usize,
    fire: Firelight,
    fog: f32,
    backdrop: Backdrop,
) -> (image::RgbaImage, Vec<f32>) {
    render_meshes_painted(meshes, view, dot_w, dot_h, fire, fog, backdrop, None)
}

#[allow(clippy::too_many_arguments)]
pub fn render_meshes_painted(
    meshes: &[&Mesh],
    view: &View3,
    dot_w: usize,
    dot_h: usize,
    fire: Firelight,
    fog: f32,
    backdrop: Backdrop,
    painted: Option<&Painted>,
) -> (image::RgbaImage, Vec<f32>) {
    if dot_w == 0 || dot_h == 0 {
        return (RgbaImage::new(dot_w as u32, dot_h as u32), Vec::new());
    }
    let camera = Camera::new(view, dot_w, dot_h);
    let haze = match backdrop { Backdrop::Haze(rgb) => rgb, _ => NIGHT_HAZE };
    let mut pixels = vec![0u8; dot_w * dot_h * 4];
    let mut depth = vec![f32::INFINITY; dot_w * dot_h];
    SCRATCH.with(|cell| {
        let mut scratch = cell.borrow_mut();
        let Scratch {
            prepared,
            surface,
            uv,
        } = &mut *scratch;
        prepare(meshes, &camera, dot_w, dot_h, prepared);
        surface.clear();
        surface.resize(dot_w * dot_h, Surface::SKY);
        uv.clear();
        uv.resize(dot_w * dot_h, [0.0; 2]);
        let prepared: &[Prepared] = prepared;
        let shade_band = |row0: usize,
                          depth: &mut [f32],
                          surface: &mut [Surface],
                          uv: &mut [[f32; 2]],
                          pixels: &mut [u8]| {
            let rows = depth.len() / dot_w;
            fill_band(prepared, dot_w, row0, rows, depth, surface, uv);
            for local in 0..rows {
                let py = row0 + local;
                for px in 0..dot_w {
                    let index = local * dot_w + px;
                    let color = if depth[index].is_finite() {
                        surface[index].shade_with(uv[index], depth[index], fire, fog, painted, haze)
                    } else {
                        match backdrop {
                            Backdrop::Night => sky_color(camera.ray(px, py)),
                            Backdrop::Dark(rgb) | Backdrop::Haze(rgb) => rgb,
                        }
                    };
                    let base = index * 4;
                    pixels[base] = color[0];
                    pixels[base + 1] = color[1];
                    pixels[base + 2] = color[2];
                    pixels[base + 3] = 255;
                }
            }
        };
        let band_rows = band_rows(dot_h);
        if band_rows >= dot_h {
            shade_band(0, &mut depth, surface, uv, &mut pixels);
        } else {
            let span = band_rows * dot_w;
            let bands = depth
                .chunks_mut(span)
                .zip(surface.chunks_mut(span))
                .zip(uv.chunks_mut(span))
                .zip(pixels.chunks_mut(span * 4))
                .enumerate();
            #[cfg(not(target_arch = "wasm32"))]
            std::thread::scope(|scope| {
                let shade_band = &shade_band;
                for (k, (((d, s), u), p)) in bands {
                    scope.spawn(move || shade_band(k * band_rows, d, s, u, p));
                }
            });
            #[cfg(target_arch = "wasm32")]
            for (k, (((d, s), u), p)) in bands {
                shade_band(k * band_rows, d, s, u, p);
            }
        }
    });
    (
        RgbaImage::from_raw(dot_w as u32, dot_h as u32, pixels)
            .expect("dot_w * dot_h * 4 bytes is exactly an RGBA buffer of that size"),
        depth,
    )
}

fn band_rows(dot_h: usize) -> usize {
    BAND_ROWS.max(dot_h.div_ceil(MAX_BANDS))
}

pub fn project_point(view: &View3, dot_w: usize, dot_h: usize, p: V3) -> Option<(f32, f32, f32)> {
    let camera = Camera::new(view, dot_w, dot_h);
    let c = camera.to_camera(p);
    if c.z < Z_NEAR {
        return None;
    }
    let (x, y) = camera.project(c);
    Some((x, y, c.z))
}

pub fn dots_tall(view: &View3, dot_w: usize, dot_h: usize, world_h: f32, depth: f32) -> f32 {
    let camera = Camera::new(view, dot_w, dot_h);
    world_h / depth.max(Z_NEAR) / camera.tan_v * camera.half_h
}

#[derive(Default)]
struct Scratch {
    prepared: Vec<Prepared>,
    surface: Vec<Surface>,
    uv: Vec<[f32; 2]>,
}

std::thread_local! {
    static SCRATCH: std::cell::RefCell<Scratch> = std::cell::RefCell::new(Scratch::default());
}

#[derive(Clone, Copy)]
struct Prepared {
    tri: [ScreenVertex; 3],
    surf: Surface,
    rows: (usize, usize),
}

fn prepare(meshes: &[&Mesh], camera: &Camera, dot_w: usize, dot_h: usize, out: &mut Vec<Prepared>) {
    out.clear();
    let mut clipped = [ClipVertex {
        p: V3::ZERO,
        uv: [0.0, 0.0],
    }; 4];
    let moon = moon_direction();
    for tri in meshes.iter().flat_map(|m| m.tris.iter()) {
        let camera_tri = [
            ClipVertex {
                p: camera.to_camera(tri.v[0]),
                uv: tri.uv[0],
            },
            ClipVertex {
                p: camera.to_camera(tri.v[1]),
                uv: tri.uv[1],
            },
            ClipVertex {
                p: camera.to_camera(tri.v[2]),
                uv: tri.uv[2],
            },
        ];
        if camera_tri.iter().all(|v| v.p.z < Z_NEAR)
            || camera_tri.iter().all(|v| v.p.x > v.p.z * camera.tan_h)
            || camera_tri.iter().all(|v| -v.p.x > v.p.z * camera.tan_h)
            || camera_tri.iter().all(|v| v.p.y > v.p.z * camera.tan_v)
            || camera_tri.iter().all(|v| -v.p.y > v.p.z * camera.tan_v)
        {
            continue;
        }
        let count = clip_near(camera_tri, &mut clipped);
        if count < 3 {
            continue;
        }
        let centroid = (tri.v[0] + tri.v[1] + tri.v[2]) / 3.0;
        let mut normal = tri.normal;
        if normal.dot(centroid - camera.pos) > 0.0 {
            normal = -normal;
        }
        let surf = Surface::lit(tri.mat, normal, (camera.pos - centroid).normalize(), moon);
        for i in 1..count - 1 {
            let projected = [
                project_vertex(camera, clipped[0]),
                project_vertex(camera, clipped[i]),
                project_vertex(camera, clipped[i + 1]),
            ];
            let min_y = projected[0].y.min(projected[1].y).min(projected[2].y).floor().max(0.0) as usize;
            let max_y = (projected[0].y.max(projected[1].y).max(projected[2].y).ceil() as i64)
                .clamp(0, dot_h as i64) as usize;
            let min_x = projected[0].x.min(projected[1].x).min(projected[2].x).floor().max(0.0) as usize;
            let max_x = (projected[0].x.max(projected[1].x).max(projected[2].x).ceil() as i64)
                .clamp(0, dot_w as i64) as usize;
            if min_y >= max_y || min_x >= max_x {
                continue;
            }
            out.push(Prepared {
                tri: projected,
                surf,
                rows: (min_y, max_y),
            });
        }
    }
}

fn fill_band(
    prepared: &[Prepared],
    dot_w: usize,
    row0: usize,
    rows: usize,
    depth: &mut [f32],
    surface: &mut [Surface],
    uv: &mut [[f32; 2]],
) {
    let row1 = row0 + rows;
    for p in prepared {
        if p.rows.1 <= row0 || p.rows.0 >= row1 {
            continue;
        }
        fill_triangle(p.tri, dot_w, (row0, row1), p.surf, depth, surface, uv);
    }
}

struct DepthPass {
    depth: Vec<f32>,
    surface: Vec<Surface>,
    uv: Vec<[f32; 2]>,
}

fn depth_pass(scene: &Mesh, camera: &Camera, dot_w: usize, dot_h: usize) -> DepthPass {
    let mut depth = vec![f32::INFINITY; dot_w * dot_h];
    let mut surface = vec![Surface::SKY; dot_w * dot_h];
    let mut uv_buffer = vec![[0.0f32; 2]; dot_w * dot_h];
    let mut prepared = Vec::new();
    prepare(&[scene], camera, dot_w, dot_h, &mut prepared);
    fill_band(&prepared, dot_w, 0, dot_h, &mut depth, &mut surface, &mut uv_buffer);
    DepthPass {
        depth,
        surface,
        uv: uv_buffer,
    }
}

pub fn composition_mix(
    scene: &Mesh,
    view: &View3,
    dot_w: usize,
    dot_h: usize,
) -> (f32, f32, f32) {
    if dot_w == 0 || dot_h == 0 {
        return (0.0, 0.0, 0.0);
    }
    let camera = Camera::new(view, dot_w, dot_h);
    let pass = depth_pass(scene, &camera, dot_w, dot_h);
    let (mut sky, mut wall, mut ground) = (0u32, 0u32, 0u32);
    for (index, depth) in pass.depth.iter().enumerate() {
        if !depth.is_finite() {
            sky += 1;
        } else if matches!(
            pass.surface[index].mat,
            mat::GRASS | mat::PATH | mat::WATER | mat::FLOOR | mat::TORCH_POOL
        ) {
            ground += 1;
        } else {
            wall += 1;
        }
    }
    let total = (dot_w * dot_h) as f32;
    (
        sky as f32 / total,
        wall as f32 / total,
        ground as f32 / total,
    )
}

fn project_vertex(camera: &Camera, vertex: ClipVertex) -> ScreenVertex {
    let (x, y) = camera.project(vertex.p);
    let inv_z = 1.0 / vertex.p.z;
    ScreenVertex {
        x,
        y,
        inv_z,
        u_over_z: vertex.uv[0] * inv_z,
        v_over_z: vertex.uv[1] * inv_z,
    }
}

fn edge(a: &ScreenVertex, b: &ScreenVertex, x: f32, y: f32) -> f32 {
    (b.x - a.x) * (y - a.y) - (b.y - a.y) * (x - a.x)
}

fn fill_triangle(
    mut tri: [ScreenVertex; 3],
    dot_w: usize,
    band: (usize, usize),
    surf: Surface,
    depth: &mut [f32],
    surface: &mut [Surface],
    uv_buffer: &mut [[f32; 2]],
) {
    let mut area = edge(&tri[0], &tri[1], tri[2].x, tri[2].y);
    if area.abs() < 1e-7 {
        return;
    }
    if area < 0.0 {
        tri.swap(1, 2);
        area = -area;
    }
    let min_x = tri[0].x.min(tri[1].x).min(tri[2].x).floor().max(0.0) as usize;
    let max_x =
        (tri[0].x.max(tri[1].x).max(tri[2].x).ceil() as i64).clamp(0, dot_w as i64) as usize;
    let min_y = (tri[0].y.min(tri[1].y).min(tri[2].y).floor().max(0.0) as usize).max(band.0);
    let max_y = ((tri[0].y.max(tri[1].y).max(tri[2].y).ceil() as i64).clamp(0, band.1 as i64)
        as usize)
        .min(band.1);
    if min_x >= max_x.min(dot_w) || min_y >= max_y {
        return;
    }
    let inv_area = 1.0 / area;
    for py in min_y..max_y {
        let y = py as f32 + 0.5;
        let row = (py - band.0) * dot_w;
        for px in min_x..max_x.min(dot_w) {
            let x = px as f32 + 0.5;
            let w0 = edge(&tri[1], &tri[2], x, y);
            if w0 < 0.0 {
                continue;
            }
            let w1 = edge(&tri[2], &tri[0], x, y);
            if w1 < 0.0 {
                continue;
            }
            let w2 = edge(&tri[0], &tri[1], x, y);
            if w2 < 0.0 {
                continue;
            }
            let (b0, b1, b2) = (w0 * inv_area, w1 * inv_area, w2 * inv_area);
            let inv_z = b0 * tri[0].inv_z + b1 * tri[1].inv_z + b2 * tri[2].inv_z;
            if inv_z <= 0.0 {
                continue;
            }
            let z = 1.0 / inv_z;
            let index = row + px;
            if z >= depth[index] {
                continue;
            }
            depth[index] = z;
            surface[index] = surf;
            uv_buffer[index] = [
                (b0 * tri[0].u_over_z + b1 * tri[1].u_over_z + b2 * tri[2].u_over_z) * z,
                (b0 * tri[0].v_over_z + b1 * tri[1].v_over_z + b2 * tri[2].v_over_z) * z,
            ];
        }
    }
}

fn moon_direction() -> V3 {
    let azimuth = MOON_BEARING * std::f32::consts::TAU;
    let (sin_a, cos_a) = azimuth.sin_cos();
    let (sin_e, cos_e) = MOON_ELEVATION_RAD.sin_cos();
    v3(cos_a * cos_e, sin_a * cos_e, sin_e)
}

#[derive(Clone, Copy)]
struct Surface {
    mat: u8,
    key: f32,
    ambient: f32,
    rim: f32,
    gain: [u16; 3],
}

impl Surface {
    const SKY: Surface = Surface {
        mat: 0,
        key: 0.0,
        ambient: 0.0,
        rim: 0.0,
        gain: [0; 3],
    };

    fn lit(mat: u8, normal: V3, to_eye: V3, moon: V3) -> Surface {
        let key = normal.dot(moon).max(0.0);
        let ambient = 0.16 + 0.34 * (normal.z * 0.5 + 0.5);
        let grazing = 1.0 - normal.dot(to_eye).abs().clamp(0.0, 1.0);
        let rim = if lantern_lit(mat)
            || matches!(
                mat,
                mat::HORSE_BAY
                    | mat::HORSE_CHESTNUT
                    | mat::HORSE_GREY
                    | mat::HAY
                    | mat::CLOTH_GOLD
                    | mat::CLOTH_BLUE
                    | mat::STABLE_TIMBER
                    | mat::STABLE_ROOF
                    | mat::STABLE_WALL
                    | mat::STABLE_FLOOR
                    | mat::TACK_LEATHER
            ) {
            0.0
        } else {
            grazing.powi(4) * (0.07 + 0.05 * key)
        };
        let plane = smoothstep(key * 0.82 + 0.12);
        let mut light = ambient * 0.68 + 0.66 * plane + 0.10;
        if mat == mat::FLOOR {
            light = 0.28 + ambient * 0.45 + key * 0.44;
        }
        if matches!(
            mat,
            mat::HORSE_BAY
                | mat::HORSE_CHESTNUT
                | mat::HORSE_GREY
                | mat::HAY
                | mat::CLOTH_GOLD
                | mat::CLOTH_BLUE
                | mat::STABLE_TIMBER
                | mat::STABLE_ROOF
                | mat::STABLE_WALL
                | mat::STABLE_FLOOR
                | mat::TACK_LEATHER
        ) {
            light = light.max(0.57 + key * 0.20);
        }
        if mat == mat::ARMOR {
            light = light.max(0.53 + key * 0.15);
        }
        if mat == mat::FOLIAGE {
            light = 0.56 + 0.72 * plane;
        }
        if mat == mat::CRYSTAL {
            light = 0.50 + ambient * 0.40 + plane * 0.18;
        }
        let gain = [
            0.84 + 0.16 * plane,
            0.94 + 0.04 * plane,
            1.08 - 0.15 * plane,
        ]
        .map(|hue| (light * hue * 256.0).round() as u16);
        Surface {
            mat,
            key,
            ambient,
            rim,
            gain,
        }
    }

    fn shade(self, uv: [f32; 2], depth: f32, fire: Firelight, fog: f32) -> [u8; 3] {
        self.shade_with(uv, depth, fire, fog, None, NIGHT_HAZE)
    }

    fn shade_with(
        self,
        uv: [f32; 2],
        depth: f32,
        fire: Firelight,
        fog: f32,
        painted: Option<&Painted>,
        haze: [u8; 3],
    ) -> [u8; 3] {
        let (base, fog_resistance, emissive) =
            material_sample_with(self.mat, uv, depth, painted);
        if emissive {
            let mut strength = emissive_strength(self.mat);
            let mut base = base;
            if !fire.is_steady() && flickers(self.mat) {
                strength *= fire.gain.clamp(0.5, 2.0);
                base = warm_swing(base, fire.warm);
            }
            let lit = scale_rgb(base, strength);
            return mix_rgb(lit, haze, fog_amount(depth, fog) * fog_resistance);
        }
        let lantern = if lantern_lit(self.mat) {
            lantern_gain(depth)
        } else {
            256
        };
        let lit = std::array::from_fn(|k| {
            ((u32::from(base[k]) * u32::from(self.gain[k]) * u32::from(lantern) + 32768) >> 16)
                .min(218) as u8
        });
        let rim_color = if self.mat == mat::FOLIAGE {
            [67, 109, 79]
        } else {
            MOON_SILVER
        };
        let lit = mix_rgb(lit, rim_color, self.rim);
        let resistance = fog_resistance * (1.0 - 0.40 * self.rim);
        mix_rgb(lit, haze, fog_amount(depth, fog) * resistance)
    }
}

fn emissive_strength(material: u8) -> f32 {
    match material {
        mat::EMBER => 1.02,
        mat::WINDOW | mat::FIRE | mat::FLAME => 0.96,
        mat::MOONLIGHT => 0.56,
        _ => 1.0,
    }
}

fn warm_swing(rgb: [u8; 3], warm: f32) -> [u8; 3] {
    let warm = warm.clamp(-0.5, 0.5);
    [
        (rgb[0] as f32 * (1.0 + 0.24 * warm))
            .round()
            .clamp(0.0, 255.0) as u8,
        (rgb[1] as f32 * (1.0 - 0.06 * warm))
            .round()
            .clamp(0.0, 255.0) as u8,
        (rgb[2] as f32 * (1.0 - 0.42 * warm))
            .round()
            .clamp(0.0, 255.0) as u8,
    ]
}

fn lantern_lit(material: u8) -> bool {
    matches!(material, mat::GRASS | mat::PATH | mat::FLOOR | mat::WATER)
}

fn fog_amount(depth: f32, distance: f32) -> f32 {
    if !depth.is_finite() {
        return 1.0;
    }
    (depth.max(0.0) / distance.max(1.0)).clamp(0.0, 1.0)
}

fn material_sample(material: u8, uv: [f32; 2]) -> ([u8; 3], f32, bool) {
    material_sample_with(material, uv, 0.0, None)
}

fn material_sample_with(
    material: u8,
    uv: [f32; 2],
    depth: f32,
    painted: Option<&Painted>,
) -> ([u8; 3], f32, bool) {
    const SIDE: usize = 256;
    const MATERIALS: usize = mat::FLAME as usize + 1;
    static TEXTURES: std::sync::OnceLock<Vec<Vec<[u8; 3]>>> = std::sync::OnceLock::new();
    if material == mat::MOONLIGHT {
        return material_base(material, uv);
    }
    let textures = TEXTURES.get_or_init(|| {
        (0..=mat::FLAME)
            .map(|m| {
                (0..SIDE * SIDE)
                    .map(|i| {
                        let offset = if m == mat::TORCH_POOL { 4.0 } else { 0.0 };
                        material_base(
                            m,
                            [
                                (i % SIDE) as f32 / 32.0 - offset,
                                (i / SIDE) as f32 / 32.0 - offset,
                            ],
                        )
                        .0
                    })
                    .collect()
            })
            .collect()
    });
    static INFO: std::sync::OnceLock<[(f32, bool); MATERIALS]> = std::sync::OnceLock::new();
    let info = INFO.get_or_init(|| {
        std::array::from_fn(|m| {
            let (_, resistance, emissive) = material_base(m as u8, [0.0; 2]);
            (resistance, emissive)
        })
    });
    let coordinate = |v: f32| {
        if material == mat::TORCH_POOL {
            ((v + 4.0) * 32.0).clamp(0.0, 255.0) as usize
        } else {
            (v * 32.0).floor() as i32 as usize & (SIDE - 1)
        }
    };
    let (x, y) = (coordinate(uv[0]), coordinate(uv[1]));
    let texture = textures.get(usize::from(material)).unwrap_or(&textures[0]);
    let &(resistance, emissive) = info.get(usize::from(material)).unwrap_or(&info[0]);
    if let Some(base) = painted.and_then(|p| p.sample(material, x, y, depth)) {
        return (base, resistance, emissive);
    }
    (texture[y * SIDE + x], resistance, emissive)
}

fn lantern_gain(depth: f32) -> u16 {
    static GAINS: std::sync::OnceLock<[u16; 512]> = std::sync::OnceLock::new();
    let gains = GAINS.get_or_init(|| {
        std::array::from_fn(|i| {
            let t = 1.0 - (i as f32 / 32.0 / LANTERN_REACH).clamp(0.0, 1.0);
            (256.0 * (1.0 + LANTERN_GAIN * smoothstep(t))).round() as u16
        })
    });
    gains[((depth.max(0.0) * 32.0) as usize).min(511)]
}

fn material_base(material: u8, uv: [f32; 2]) -> ([u8; 3], f32, bool) {
    let (u, v) = (uv[0], uv[1]);
    match material {
        mat::SCULPTED_STONE => ([108, 119, 135], 1.0, false),
        mat::BONE => ([195, 185, 158], 1.0, false),
        mat::STONE => (course_tint([116, 122, 138], u, v, 0.88), 1.0, false),
        mat::STONE_DARK => (course_tint([68, 74, 90], u, v, 0.90), 1.0, false),
        mat::ROOF => (shingle_tint([76, 68, 84], u, v), 1.0, false),
        mat::WOOD => (grain_tint([96, 70, 48], u, v), 1.0, false),
        mat::WINDOW => (window_tint(u, v), 0.22, true),
        mat::FIRE => (window_tint(u, v), 0.22, true),
        mat::FLAME => (flame_tint(u, v), 0.22, true),
        mat::EMBER => (ember_tint(u, v), 0.20, true),
        mat::MOONLIGHT => (moonlight_tint(u, v), 0.58, true),
        mat::DOOR => (grain_tint([124, 86, 48], u, v), 0.85, false),
        mat::GRASS => (speckle([68, 94, 58], [48, 70, 46], u, v, 1.7), 1.0, false),
        mat::PATH => (speckle([130, 120, 98], [98, 90, 74], u, v, 2.3), 1.0, false),
        mat::WATER => (speckle([26, 42, 66], [18, 30, 52], u, v, 1.1), 1.0, false),
        mat::FOLIAGE => (speckle([53, 94, 55], [31, 59, 40], u, v, 3.1), 1.0, false),
        mat::TRUNK => (grain_tint([62, 48, 36], u, v), 1.0, false),
        mat::HORSE_BAY => (
            speckle([176, 108, 57], [151, 85, 42], u, v, 4.0),
            0.55,
            false,
        ),
        mat::HORSE_CHESTNUT => (
            speckle([217, 132, 66], [183, 99, 43], u, v, 4.0),
            0.55,
            false,
        ),
        mat::HORSE_GREY => (
            speckle([207, 220, 228], [169, 187, 204], u, v, 4.0),
            0.55,
            false,
        ),
        mat::HAY => (
            speckle([244, 192, 67], [214, 163, 46], u, v, 6.0),
            0.5,
            false,
        ),
        mat::CLOTH_GOLD => ([211, 165, 69], 0.55, false),
        mat::CLOTH_BLUE => ([66, 142, 212], 0.55, false),
        mat::STABLE_TIMBER => (grain_tint([185, 127, 69], u, v), 0.55, false),
        mat::STABLE_ROOF => (shingle_tint([168, 94, 65], u, v), 0.6, false),
        mat::STABLE_WALL => (course_tint([151, 130, 100], u, v, 0.88), 0.6, false),
        mat::STABLE_FLOOR => (speckle([121, 96, 61], [105, 78, 47], u, v, 3.0), 0.6, false),
        mat::TACK_LEATHER => (grain_tint([91, 43, 26], u, v), 0.5, false),
        mat::BANNER => (speckle([146, 62, 58], [116, 46, 46], u, v, 2.0), 0.7, false),
        mat::BOOKS => (speckle([112, 78, 52], [72, 50, 36], u, v, 4.0), 0.6, false),
        mat::FLOOR => (course_tint([92, 88, 84], u, v, 0.84), 1.0, false),
        mat::BRASS => ([190, 145, 75], 0.85, false),
        mat::PARCHMENT => ([226, 203, 139], 1.0, false),
        mat::CRYSTAL => ([100, 180, 210], 0.35, false),
        mat::ROCK => (
            speckle([65, 81, 105], [42, 55, 79], u, v * 0.35, 1.5),
            1.0,
            false,
        ),
        mat::ARMOR => ([157, 174, 195], 0.85, false),
        mat::TORCH_POOL => {
            let falloff = (1.0 - (u * u + v * v).sqrt() / 1.65).clamp(0.0, 1.0);
            let stone = course_tint([62, 54, 44], u, v, 0.90);
            (
                mix_rgb(stone, [153, 102, 43], falloff * falloff),
                0.90,
                true,
            )
        }
        _ => ([96, 100, 110], 1.0, false),
    }
}

fn course_tint(base: [u8; 3], u: f32, v: f32, mortar: f32) -> [u8; 3] {
    let row = (v / 0.42).floor();
    let across = (u / 0.62 + (row as i32).rem_euclid(2) as f32 * 0.5).rem_euclid(1.0);
    let along = (v / 0.42).rem_euclid(1.0);
    let edge = across.min(1.0 - across).min(along.min(1.0 - along));
    let shoulder = smoothstep(edge / 0.14);
    let grain = soft_noise(u * 0.8, v * 0.8);
    scale_rgb(
        base,
        (mortar + (1.0 - mortar) * shoulder) * (0.97 + 0.06 * grain),
    )
}

fn shingle_tint(base: [u8; 3], u: f32, v: f32) -> [u8; 3] {
    let row = (v / 0.24).rem_euclid(1.0);
    let stagger = ((v / 0.24).floor() as i32).rem_euclid(2) as f32 * 0.5;
    let across = (u / 0.30 + stagger).rem_euclid(1.0);
    let edge = row.min(1.0 - row).min(across.min(1.0 - across));
    scale_rgb(base, 0.88 + 0.12 * smoothstep(edge / 0.20))
}

fn grain_tint(base: [u8; 3], u: f32, v: f32) -> [u8; 3] {
    let board = (u / 0.26).rem_euclid(1.0);
    let shoulder = smoothstep(board.min(1.0 - board) / 0.14);
    scale_rgb(
        base,
        (0.90 + 0.10 * shoulder) * (0.97 + 0.06 * soft_noise(u * 0.8, v * 1.5)),
    )
}

fn soft_noise(u: f32, v: f32) -> f32 {
    let x = u.floor() as i32;
    let y = v.floor() as i32;
    let a = smoothstep(u - u.floor());
    let b = smoothstep(v - v.floor());
    let h = |x, y| hash01(0x4752_4f55_4e44_3344, x, y);
    let lo = h(x, y) * (1.0 - a) + h(x + 1, y) * a;
    let hi = h(x, y + 1) * (1.0 - a) + h(x + 1, y + 1) * a;
    lo * (1.0 - b) + hi * b
}

fn ember_tint(u: f32, v: f32) -> [u8; 3] {
    let heat = smoothstep((soft_noise(u * 5.0, v * 5.0) - 0.22) / 0.56);
    mix_rgb([119, 45, 23], [228, 193, 113], heat)
}

fn flame_tint(u: f32, v: f32) -> [u8; 3] {
    let height = v.clamp(0.0, 1.0);
    let bend = 0.07 * (height * 5.0).sin();
    let width = 0.42 * (1.0 - 0.64 * height);
    let core = smoothstep(1.0 - (u - 0.5 - bend).abs() / width)
        * (1.0 - 0.78 * smoothstep(height));
    let outer = mix_rgb([181, 88, 37], [208, 127, 54], 1.0 - height);
    mix_rgb(outer, [228, 207, 156], core)
}

fn window_tint(u: f32, v: f32) -> [u8; 3] {
    let across = (u / 0.16).rem_euclid(1.0);
    let along = (v / 0.20).rem_euclid(1.0);
    let lead = across < 0.16 || along < 0.16;
    if lead {
        [76, 55, 35]
    } else {
        let dx = (across - 0.5) / 0.5;
        let dy = (along - 0.5) / 0.5;
        let dome = (1.0 - dx * dx) * (1.0 - dy * dy);
        let scale = 0.92 + 0.08 * dome;
        [
            (218.0 * scale) as u8,
            (174.0 * scale) as u8,
            (112.0 * scale) as u8,
        ]
    }
}

fn moonlight_tint(u: f32, v: f32) -> [u8; 3] {
    let fade = 1.0 / (1.0 + v.max(0.0) * 0.62);
    let grain = hash01(
        0x4D4F_4F4E_4C49_5445,
        (u * 0.72).floor() as i32,
        (v * 0.72).floor() as i32,
    );
    scale_rgb(
        [150, 176, 218],
        (0.30 + 0.70 * fade) * (0.90 + 0.16 * grain),
    )
}

fn speckle(a: [u8; 3], b: [u8; 3], u: f32, v: f32, scale: f32) -> [u8; 3] {
    let roll = 0.38 + 0.24 * soft_noise(u * scale * 0.4, v * scale * 0.4);
    mix_rgb(a, b, roll)
}

fn sky_color(dir: V3) -> [u8; 3] {
    if dir.z < 0.0 {
        return NIGHT_HAZE;
    }
    let elevation = dir.z.clamp(-1.0, 1.0).asin();
    let t = 1.0 - (elevation / SKY_BAND_RAD).clamp(0.0, 1.0);
    let bearing = if elevation > 0.07 {
        (dir.y.atan2(dir.x) / std::f32::consts::TAU).rem_euclid(1.0)
    } else {
        0.0
    };
    let mut color = mix_rgb(SKY_ZENITH, SKY_HORIZON, smoothstep(t));
    let glow = 1.0 - (elevation / HORIZON_GLOW_RAD).clamp(0.0, 1.0);
    color = mix_rgb(color, HORIZON_GLOW, glow * glow * HORIZON_GLOW_GAIN);

    let altitude = (elevation / std::f32::consts::FRAC_PI_2).clamp(0.0, 1.0);
    let star_col = (bearing * 144.0).floor() as i32;
    let star_row = (altitude * 30.0).floor() as i32;
    if elevation > 0.07 && hash01(STAR_SEED, star_col, star_row) > 0.93 {
        let local_x = (bearing * 144.0).fract();
        let local_y = (altitude * 30.0).fract();
        let at_x = hash01(STAR_SEED ^ 0x58, star_col, star_row);
        let at_y = hash01(STAR_SEED ^ 0x59, star_col, star_row);
        if (local_x - at_x).abs() < 0.13 && (local_y - at_y).abs() < 0.13 {
            let brightness = 0.72 + 0.28 * hash01(STAR_SEED ^ 0x42, star_col, star_row);
            color = mix_rgb(color, [171, 187, 209], brightness);
        }
    }

    let toward = dir.dot(moon_direction()).clamp(-1.0, 1.0);
    let angle = if toward > 0.98 { toward.acos() } else { MOON_HALO_RAD };
    if angle < MOON_RADIUS_RAD {
        let limb = smoothstep((angle / MOON_RADIUS_RAD).clamp(0.0, 1.0));
        let disc = mix_rgb([207, 214, 221], MOON_SILVER, limb);
        color = mix_rgb(color, disc, 1.0 - limb * 0.30);
    } else if angle < MOON_HALO_RAD {
        let halo = 1.0 - (angle - MOON_RADIUS_RAD) / (MOON_HALO_RAD - MOON_RADIUS_RAD);
        color = mix_rgb(color, [92, 104, 138], halo * halo * 0.19);
    }
    color
}

fn mix_rgb(from: [u8; 3], to: [u8; 3], amount: f32) -> [u8; 3] {
    let amount = amount.clamp(0.0, 1.0);
    [
        mix_channel(from[0], to[0], amount),
        mix_channel(from[1], to[1], amount),
        mix_channel(from[2], to[2], amount),
    ]
}

fn mix_channel(from: u8, to: u8, amount: f32) -> u8 {
    (from as f32 + (to as f32 - from as f32) * amount)
        .round()
        .clamp(0.0, 255.0) as u8
}

fn scale_rgb(color: [u8; 3], amount: f32) -> [u8; 3] {
    [
        (color[0] as f32 * amount).round().clamp(0.0, 255.0) as u8,
        (color[1] as f32 * amount).round().clamp(0.0, 255.0) as u8,
        (color[2] as f32 * amount).round().clamp(0.0, 255.0) as u8,
    ]
}

fn smoothstep(value: f32) -> f32 {
    let value = value.clamp(0.0, 1.0);
    value * value * (3.0 - 2.0 * value)
}

fn hash01(seed: u64, x: i32, y: i32) -> f32 {
    let mut value = seed;
    value = value.wrapping_add((x as i64 as u64).wrapping_mul(0x9e37_79b9_7f4a_7c15));
    value = value.wrapping_add((y as i64 as u64).wrapping_mul(0xbf58_476d_1ce4_e5b9));
    value = (value ^ (value >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
    value = (value ^ (value >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
    value ^= value >> 31;
    (value >> 40) as f32 / 16_777_216.0
}

#[cfg(test)]
#[path = "../../../tests/render/world3d__raster__tests.rs"]
mod tests;

#[cfg(any(test, feature = "testing"))]
pub fn one_pass(scene: &Mesh, view: &View3, w: usize, h: usize) -> Vec<u8> {
    let camera = Camera::new(view, w, h);
    let pass = depth_pass(scene, &camera, w, h);
    let mut out = Vec::with_capacity(w * h * 4);
    for py in 0..h {
        for px in 0..w {
            let i = py * w + px;
            let c = if pass.depth[i].is_finite() {
                pass.surface[i].shade(pass.uv[i], pass.depth[i], Firelight::STEADY, FOG_DISTANCE)
            } else {
                sky_color(camera.ray(px, py))
            };
            out.extend([c[0], c[1], c[2], 255]);
        }
    }
    out
}

#[cfg(any(test, feature = "testing"))]
pub fn visible_materials(scene: &Mesh, view: &View3, w: usize, h: usize) -> Vec<u8> {
    let pass = depth_pass(scene, &Camera::new(view, w, h), w, h);
    pass.surface.iter().map(|s| s.mat).collect()
}
