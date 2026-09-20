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
//! Grading deliberately reuses `identity::NOIR_*` and the raycaster's moon
//! anchors so a 3D frame sits in the same night as a raycast one — the same
//! `tone_lut` and Bayer screen downstream expect that value range.
//!
//! DETERMINISM LAW: byte-identical output for identical inputs. Triangles are
//! consumed in `Mesh::tris` order, every hash is SplitMix64 over integers, and
//! nothing here reads a clock, an env var, or an unordered collection.

use super::math::{V3, v3};
use super::mesh::{Mesh, mat};
use image::RgbaImage;

/// Full 3D camera, built at the ride seam from RayView + operator pitch.
#[derive(Clone, Copy, Debug)]
pub(crate) struct View3 {
    /// Eye position: x/y in tile units, z = eye height above ground.
    pub pos: V3,
    /// Yaw, same convention as RayView.heading_rad.
    pub heading_rad: f32,
    /// Radians; positive looks up.
    pub pitch: f32,
    /// Horizontal field of view, radians.
    pub fov_rad: f32,
}

/// How hard the live flame in a scene is burning *this frame*.
///
/// The one piece of per-frame lighting state the renderer takes, and it reaches
/// exactly one material family: [`mat::FIRE`] and [`mat::EMBER`]. Windows,
/// moonlight and every lit surface outdoors are untouched by construction — a
/// pane of glass does not gutter and the moon does not blink.
///
/// [`STEADY`](Firelight::STEADY) is the identity: a scene rendered with it is
/// byte-identical to one rendered before this parameter existed, which is what
/// keeps the exterior vista and every pinned frame in the test floor honest.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct Firelight {
    /// Multiplier on the flame's emissive strength. 1.0 is the authored burn.
    pub gain: f32,
    /// Hue swing, signed: positive drives the flame toward its red end (a fire
    /// settling), negative toward its pale yellow one (a fire flaring).
    pub warm: f32,
}

impl Firelight {
    /// A fire that is not breathing — the authored, pre-flicker burn.
    pub(crate) const STEADY: Firelight = Firelight {
        gain: 1.0,
        warm: 0.0,
    };

    /// Whether this is the identity, i.e. whether shading can skip the swing.
    fn is_steady(self) -> bool {
        self.gain == 1.0 && self.warm == 0.0
    }
}

/// Which emissives are *fire* — the only surfaces [`Firelight`] touches.
fn flickers(material: u8) -> bool {
    matches!(material, mat::FIRE | mat::EMBER)
}

/// Nothing closer than this survives the clip; small enough that the camera can
/// stand inside a doorway without the near faces popping.
const Z_NEAR: f32 = 0.05;
/// Night visibility, in tiles. Longer than the raycaster's 11-cell
/// `NIGHT_FOG_DISTANCE` because a real vista stages masses 20–30 tiles out and
/// must still read as silhouette rather than dissolve into the haze.
pub(crate) const FOG_DISTANCE: f32 = 52.0;
/// The rider's lantern pools on the ground and dies within a few tiles. With
/// the fog behind it this is what gives the ground plane the bright→black
/// gradient that makes its dots read as perspective rather than as a flat
/// field.
const LANTERN_REACH: f32 = 5.5;
const LANTERN_GAIN: f32 = 2.05;

/// Where the residual night fog settles in the shared palette.
const NIGHT_HAZE: [u8; 3] = crate::stage::identity::NOIR_NIGHT_HAZE;
const SKY_ZENITH: [u8; 3] = crate::stage::identity::NOIR_SKY_ZENITH;
const SKY_HORIZON: [u8; 3] = crate::stage::identity::NOIR_SKY_HORIZON;
const MOON_SILVER: [u8; 3] = crate::stage::identity::NOIR_MOON_SILVER;

/// The moon's absolute bearing, shared with the raycaster so both renderers
/// hang the same moon in the same place.
const MOON_BEARING: f32 = super::MOON_BEARING;
/// The raycaster's `MOON_ALTITUDE` is a fraction of the *screen* sky band; at
/// the ride's nominal 1.05 fov that band is ~23.5°, so 0.30 down from its top
/// lands the disc a little over 16° above the horizon. A true-3D camera needs
/// that as a real elevation angle.
const MOON_ELEVATION_RAD: f32 = 0.29;
/// Angular radius of the disc and the outer edge of its halo.
const MOON_RADIUS_RAD: f32 = 0.052;
const MOON_HALO_RAD: f32 = 0.16;
/// Elevation the zenith→horizon gradient is spread over. Sized a little wider
/// than the ride's vertical half-fov so pitching up does not hit a flat band.
const SKY_BAND_RAD: f32 = 0.55;
/// The moonlit haze sitting on the horizon: how far up it reaches, what it
/// settles toward, and how strongly. Sized just under the ride's vertical
/// half-field so a level camera always has it, and a camera pitched up to the
/// zenith still loses it.
const HORIZON_GLOW_RAD: f32 = 0.17;
const HORIZON_GLOW: [u8; 3] = [54, 70, 104];
const HORIZON_GLOW_GAIN: f32 = 0.78;

/// Fixed star seed: the sky must not depend on world state that would have to
/// be hashed into `cinematic_key`.
const STAR_SEED: u64 = 0x574f_524c_4433_445f;

/// Camera basis + projection scales, built once per frame.
struct Camera {
    pos: V3,
    right: V3,
    up: V3,
    fwd: V3,
    /// tan(fov/2) horizontally, and vertically after the ~square-dot aspect.
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
        // Screen-right is the raycaster's camera plane, `(-sin, cos)`
        // (raycast.rs:163-165 builds the plane, and px=0 samples `dir - plane`,
        // so +plane is screen-right). Matching it keeps operator yaw and the
        // moon bearing reading identically in both renderers.
        let right = v3(-sin_h, cos_h, 0.0);
        let fwd = v3(cos_h * cos_p, sin_h * cos_p, sin_p);
        let up = fwd.cross(right).normalize();
        let tan_h = (view.fov_rad.clamp(0.20, 2.40) * 0.5).tan();
        // Braille dots are ~square, so the vertical field follows the dot
        // aspect directly rather than the terminal cell aspect.
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

    /// World point → camera space: (lateral, vertical, forward depth).
    fn to_camera(&self, p: V3) -> V3 {
        let d = p - self.pos;
        v3(d.dot(self.right), d.dot(self.up), d.dot(self.fwd))
    }

    /// Camera space → screen dots. Depth must already be ≥ `Z_NEAR`.
    fn project(&self, c: V3) -> (f32, f32) {
        let inv = 1.0 / c.z;
        (
            self.half_w * (1.0 + c.x * inv / self.tan_h),
            self.half_h * (1.0 - c.y * inv / self.tan_v),
        )
    }

    /// Unit ray direction through a dot centre — the sky's only geometry.
    fn ray(&self, px: usize, py: usize) -> V3 {
        let ndc_x = (px as f32 + 0.5) / self.half_w - 1.0;
        let ndc_y = 1.0 - (py as f32 + 0.5) / self.half_h;
        (self.fwd + self.right * (ndc_x * self.tan_h) + self.up * (ndc_y * self.tan_v)).normalize()
    }
}

/// A clipped, projected vertex: screen position, 1/z, and uv/z so the
/// interpolation is perspective-correct.
#[derive(Clone, Copy)]
struct ScreenVertex {
    x: f32,
    y: f32,
    inv_z: f32,
    u_over_z: f32,
    v_over_z: f32,
}

/// A camera-space vertex before projection (near-plane clipping happens here).
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

/// Sutherland–Hodgman against the single `z >= Z_NEAR` plane. A triangle can
/// only ever become a quad, so the output is bounded at four vertices.
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
            // The sign change guarantees a non-zero denominator, but keep the
            // guard so a degenerate edge cannot emit a NaN vertex.
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

/// Render the scene to an RGBA frame at dot resolution, with its fires at their
/// authored burn. Must be deterministic: identical inputs produce
/// byte-identical output.
pub(crate) fn render_scene(
    scene: &Mesh,
    view: &View3,
    dot_w: usize,
    dot_h: usize,
) -> image::RgbaImage {
    render_scene_lit(scene, view, dot_w, dot_h, Firelight::STEADY)
}

/// [`render_scene_lit`] with the night haze pulled in or pushed out.
///
/// The court's [`FOG_DISTANCE`] is a clear night over open ground; a mine
/// gallery, a forest and a swamp are not clear nights, and the Zelda regions'
/// danger level is exactly "how far can you see" (Z3 spec §2). Still pure: the
/// same distance renders the same bytes, and the distance itself is a function
/// of state `cinematic_key` already hashes (region + danger).
pub(crate) fn render_scene_fogged(
    scene: &Mesh,
    view: &View3,
    dot_w: usize,
    dot_h: usize,
    fire: Firelight,
    fog: f32,
) -> image::RgbaImage {
    render_fogged(scene, view, dot_w, dot_h, fire, fog)
}

/// [`render_scene`] with the scene's live flame turned up or down a notch —
/// the interior seam's hearth flicker (`interior::firelight`). Still a pure
/// function: the same `Firelight` renders the same bytes, forever.
pub(crate) fn render_scene_lit(
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
    if dot_w == 0 || dot_h == 0 {
        return RgbaImage::new(dot_w as u32, dot_h as u32);
    }
    let camera = Camera::new(view, dot_w, dot_h);
    let pass = depth_pass(scene, &camera, dot_w, dot_h);
    let mut pixels = vec![0u8; dot_w * dot_h * 4];
    for py in 0..dot_h {
        for px in 0..dot_w {
            let index = py * dot_w + px;
            let color = if pass.depth[index].is_finite() {
                pass.surface[index].shade(pass.uv[index], pass.depth[index], fire, fog)
            } else {
                sky_color(camera.ray(px, py))
            };
            let base = index * 4;
            pixels[base] = color[0];
            pixels[base + 1] = color[1];
            pixels[base + 2] = color[2];
            pixels[base + 3] = 255;
        }
    }
    RgbaImage::from_raw(dot_w as u32, dot_h as u32, pixels)
        .expect("dot_w * dot_h * 4 bytes is exactly an RGBA buffer of that size")
}

/// The resolved visible fragment per dot: what the depth pass won, before any
/// shading runs. Deferred so overdraw costs an interpolation, not a shade.
struct DepthPass {
    depth: Vec<f32>,
    surface: Vec<Surface>,
    uv: Vec<[f32; 2]>,
}

fn depth_pass(scene: &Mesh, camera: &Camera, dot_w: usize, dot_h: usize) -> DepthPass {
    let mut depth = vec![f32::INFINITY; dot_w * dot_h];
    let mut surface = vec![Surface::SKY; dot_w * dot_h];
    let mut uv_buffer = vec![[0.0f32; 2]; dot_w * dot_h];

    let mut clipped = [ClipVertex {
        p: V3::ZERO,
        uv: [0.0, 0.0],
    }; 4];
    for tri in &scene.tris {
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
        // Trivial reject before any clipping work.
        if camera_tri.iter().all(|v| v.p.z < Z_NEAR) {
            continue;
        }
        let count = clip_near(camera_tri, &mut clipped);
        if count < 3 {
            continue;
        }
        // Double-sided: flip the flat normal toward the eye once per triangle.
        // Winding is therefore never load-bearing for generators.
        let centroid = (tri.v[0] + tri.v[1] + tri.v[2]) / 3.0;
        let mut normal = tri.normal;
        if normal.dot(centroid - camera.pos) > 0.0 {
            normal = -normal;
        }
        let surf = Surface::new(tri.mat, normal, (camera.pos - centroid).normalize());
        // Fan the clipped polygon; 3 verts → 1 triangle, 4 → 2.
        for i in 1..count - 1 {
            let projected = [
                project_vertex(camera, clipped[0]),
                project_vertex(camera, clipped[i]),
                project_vertex(camera, clipped[i + 1]),
            ];
            fill_triangle(
                projected,
                dot_w,
                dot_h,
                surf,
                &mut depth,
                &mut surface,
                &mut uv_buffer,
            );
        }
    }

    DepthPass {
        depth,
        surface,
        uv: uv_buffer,
    }
}

/// What the frame is *made of*, as fractions of the dot canvas:
/// `(sky, built mass, ground)`. This is the vista law's own vocabulary — ≥ 20%
/// sky, ≤ 50% wall — measured on the geometry rather than guessed from colour,
/// which stone and night sky share too much of to tell apart.
pub(crate) fn composition_mix(
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
            mat::GRASS | mat::PATH | mat::WATER | mat::FLOOR
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

/// Screen-space scan with barycentric interpolation of depth and uv into the
/// deferred buffers. Only the depth test runs per covered pixel; shading is one
/// pass at the end, so overdraw costs an interpolation, not a shade.
fn fill_triangle(
    mut tri: [ScreenVertex; 3],
    dot_w: usize,
    dot_h: usize,
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
        // Deterministic re-winding: the same swap for the same input.
        tri.swap(1, 2);
        area = -area;
    }
    let min_x = tri[0].x.min(tri[1].x).min(tri[2].x).floor().max(0.0) as usize;
    let max_x =
        (tri[0].x.max(tri[1].x).max(tri[2].x).ceil() as i64).clamp(0, dot_w as i64) as usize;
    let min_y = tri[0].y.min(tri[1].y).min(tri[2].y).floor().max(0.0) as usize;
    let max_y =
        (tri[0].y.max(tri[1].y).max(tri[2].y).ceil() as i64).clamp(0, dot_h as i64) as usize;
    if min_x >= max_x.min(dot_w) || min_y >= max_y.min(dot_h) {
        return;
    }
    let inv_area = 1.0 / area;
    for py in min_y..max_y.min(dot_h) {
        let y = py as f32 + 0.5;
        let row = py * dot_w;
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

/// The moonlit key light: a unit vector toward the moon, in the same bearing
/// space the sky hangs it in.
fn moon_direction() -> V3 {
    let azimuth = MOON_BEARING * std::f32::consts::TAU;
    let (sin_a, cos_a) = azimuth.sin_cos();
    let (sin_e, cos_e) = MOON_ELEVATION_RAD.sin_cos();
    v3(cos_a * cos_e, sin_a * cos_e, sin_e)
}

/// Everything a covered pixel needs to shade, resolved per triangle: the
/// material and the light terms that depend only on the flat normal.
#[derive(Clone, Copy)]
struct Surface {
    mat: u8,
    /// Moon lambert, already clamped.
    key: f32,
    /// Sky-dome ambient, stronger on up-facing surfaces.
    ambient: f32,
    /// Grazing-angle silver catch keeps skylines legible through fog.
    /// The rasterizer derives it from the face's own angle to the eye,
    /// which is exactly the edge of a tower, a merlon, or a roof ridge.
    rim: f32,
}

impl Surface {
    const SKY: Surface = Surface {
        mat: 0,
        key: 0.0,
        ambient: 0.0,
        rim: 0.0,
    };

    fn new(mat: u8, normal: V3, to_eye: V3) -> Surface {
        let key = normal.dot(moon_direction()).max(0.0);
        // The night sky is the fill light, so up-facing surfaces pick up more
        // of it; a downward face (an arch soffit) keeps a floor of bounce.
        let ambient = 0.16 + 0.34 * (normal.z * 0.5 + 0.5);
        let grazing = 1.0 - normal.dot(to_eye).abs().clamp(0.0, 1.0);
        // Ground planes are grazing everywhere — rim them and the whole floor
        // turns silver. The catch belongs to built masses.
        let rim = if lantern_lit(mat) {
            0.0
        } else {
            grazing.powi(6) * (0.22 + 0.18 * key)
        };
        Surface {
            mat,
            key,
            ambient,
            rim,
        }
    }

    fn shade(self, uv: [f32; 2], depth: f32, fire: Firelight, fog: f32) -> [u8; 3] {
        let (base, fog_resistance, emissive) = material_base(self.mat, uv);
        if emissive {
            // Lit windows are the one thing that punches through the haze —
            // but not all fires are one fire. A forge throat has to outburn a
            // candle across the same frame, and at dot scale the only currency
            // that says so is value, so each emissive material carries its own
            // strength (`emissive_strength`).
            let mut strength = emissive_strength(self.mat);
            let mut base = base;
            if !fire.is_steady() && flickers(self.mat) {
                strength *= fire.gain.clamp(0.5, 2.0);
                base = warm_swing(base, fire.warm);
            }
            let lit = scale_rgb(base, strength);
            return mix_rgb(lit, NIGHT_HAZE, fog_amount(depth, fog) * fog_resistance);
        }
        // Black-paper lighting: reserve large quiet shadow shapes, then reveal
        // the moon-facing planes. The smooth shoulder avoids hard band pops
        // as geometry changes; existing emissive windows remain the focal light.
        let moon_plane = smoothstep((self.key - 0.12) / 0.76);
        let mut light = if self.mat == mat::FLOOR {
            // Interior flags carry the rider's lantern, so keep the readable
            // floor pool while the surrounding architecture takes deeper ink.
            self.ambient + self.key * 0.74
        } else {
            self.ambient * 0.82 + moon_plane * 0.82
        };
        if lantern_lit(self.mat) {
            light *= 1.0 + LANTERN_GAIN * (1.0 - (depth / LANTERN_REACH).clamp(0.0, 1.0)).powi(2);
        }
        let lit = mix_rgb(scale_rgb(base, light.min(2.6)), MOON_SILVER, self.rim);
        // The rim resists the haze so a distant skyline keeps its edge.
        let resistance = fog_resistance * (1.0 - 0.62 * self.rim.min(1.0));
        mix_rgb(lit, NIGHT_HAZE, fog_amount(depth, fog) * resistance)
    }
}

/// How hard a self-lit material burns, as a multiplier on its own base colour.
///
/// The base colours say what *kind* of light a surface is; this says how much
/// of it there is. Without the split, a hearth mouth and a candle flame twelve
/// tiles behind it dither to the same solid block of dots and the frame loses
/// its brightest point — which is the one thing a night composition cannot
/// afford. Ordered the way an eye ranks them: coals, then flame behind glass,
/// then the moon.
fn emissive_strength(material: u8) -> f32 {
    match material {
        mat::EMBER => 1.24,
        mat::WINDOW | mat::FIRE => 1.0,
        mat::MOONLIGHT => 0.66,
        _ => 1.0,
    }
}

/// Push a flame's colour along the warm axis without changing how bright it is
/// — red up and blue down for a settling fire, the reverse for a flare.
///
/// Deliberately lopsided: the eye reads a fire's temperature off its *blue*
/// end, so the blue channel carries most of the swing and green barely moves.
/// At the amplitudes the interior seam authors (|warm| ≤ 0.08) this is a
/// couple of code values on a 255-scale — a colour that settles, not a colour
/// that changes.
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

/// Which materials sit under the rider's lantern. Ground only: a wall face
/// caught by the pool would read as a spotlight, not as a road.
fn lantern_lit(material: u8) -> bool {
    matches!(material, mat::GRASS | mat::PATH | mat::FLOOR | mat::WATER)
}

fn fog_amount(depth: f32, distance: f32) -> f32 {
    if !depth.is_finite() {
        return 1.0;
    }
    (depth.max(0.0) / distance.max(1.0)).clamp(0.0, 1.0)
}

/// Material id → un-lit night base colour, fog resistance, and whether the
/// surface is self-lit. Surface patterning rides the world-scaled uv, so a
/// wall's courses stay the same physical size at every distance.
fn material_base(material: u8, uv: [f32; 2]) -> ([u8; 3], f32, bool) {
    let (u, v) = (uv[0], uv[1]);
    match material {
        mat::STONE => (course_tint([116, 122, 138], u, v, 0.88), 1.0, false),
        mat::STONE_DARK => (course_tint([68, 74, 90], u, v, 0.90), 1.0, false),
        mat::ROOF => (shingle_tint([76, 68, 84], u, v), 1.0, false),
        mat::WOOD => (grain_tint([96, 70, 48], u, v), 1.0, false),
        // Warm candle light, and it barely cares about the fog. `FIRE` is the
        // same light seen from inside the room it burns in — identical at rest,
        // and the only thing `Firelight` is allowed to breathe.
        mat::WINDOW | mat::FIRE => (window_tint(u, v), 0.22, true),
        // Live coals: hotter, oranger, and just as fog-proof — a forge mouth
        // has to still be a forge mouth from across the hamlet.
        mat::EMBER => (ember_tint(u, v), 0.20, true),
        // Moonlight: cold, unpatterned, and only half as fog-proof as a flame,
        // because a shaft of light *is* the haze it travels through.
        mat::MOONLIGHT => (moonlight_tint(u, v), 0.58, true),
        mat::DOOR => (grain_tint([124, 86, 48], u, v), 0.85, false),
        mat::GRASS => (speckle([68, 94, 58], [48, 70, 46], u, v, 1.7), 1.0, false),
        mat::PATH => (speckle([130, 120, 98], [98, 90, 74], u, v, 2.3), 1.0, false),
        mat::WATER => (speckle([26, 42, 66], [18, 30, 52], u, v, 1.1), 1.0, false),
        mat::FOLIAGE => (speckle([40, 62, 44], [24, 42, 32], u, v, 3.1), 1.0, false),
        mat::TRUNK => (grain_tint([62, 48, 36], u, v), 1.0, false),
        mat::BANNER => (speckle([146, 62, 58], [116, 46, 46], u, v, 2.0), 0.7, false),
        mat::BOOKS => (speckle([112, 78, 52], [72, 50, 36], u, v, 4.0), 0.6, false),
        mat::FLOOR => (course_tint([92, 88, 84], u, v, 0.84), 1.0, false),
        mat::BRASS => ([190, 145, 75], 0.85, false),
        mat::PARCHMENT => ([226, 203, 139], 1.0, false),
        _ => ([96, 100, 110], 1.0, false),
    }
}

/// Ashlar courses: alternating rows, offset every other course, with a darker
/// mortar joint. At braille resolution this reads as texture, not as stripes.
fn course_tint(base: [u8; 3], u: f32, v: f32, mortar: f32) -> [u8; 3] {
    const COURSE: f32 = 0.42;
    const BLOCK: f32 = 0.62;
    let row = (v / COURSE).floor();
    let offset = if (row as i32).rem_euclid(2) == 0 {
        0.0
    } else {
        0.5
    };
    let across = (u / BLOCK + offset).rem_euclid(1.0);
    let along = (v / COURSE).rem_euclid(1.0);
    if across < 0.055 || along < 0.075 {
        return scale_rgb(base, mortar);
    }
    // A per-block value jitter keeps a long wall from reading as flat paint.
    let jitter = hash01(0x5354_4f4e_4531_3131, (u / BLOCK) as i32, row as i32);
    scale_rgb(base, 0.88 + 0.22 * jitter)
}

/// Overlapping slate rows: a hard dark line at each course, lighter mid-tile.
fn shingle_tint(base: [u8; 3], u: f32, v: f32) -> [u8; 3] {
    let row = (v / 0.24).rem_euclid(1.0);
    let stagger = ((v / 0.24).floor() as i32).rem_euclid(2) as f32 * 0.5;
    let across = (u / 0.30 + stagger).rem_euclid(1.0);
    if row < 0.16 {
        scale_rgb(base, 0.66)
    } else if across < 0.08 {
        scale_rgb(base, 0.80)
    } else {
        scale_rgb(base, 1.0 + 0.12 * row)
    }
}

/// Plank grain: a soft along-the-board ripple plus the seam between boards.
fn grain_tint(base: [u8; 3], u: f32, v: f32) -> [u8; 3] {
    let board = (u / 0.26).rem_euclid(1.0);
    let seam = if board < 0.09 { 0.72 } else { 1.0 };
    let ripple = 0.92 + 0.16 * ((v * 7.3).sin() * 0.5 + 0.5);
    scale_rgb(base, seam * ripple)
}

/// A coal bed: hot cores scattered through a duller mass. Flat orange paint
/// would read as a lit signboard at dot scale; a mottle reads as *fire*,
/// because the dither turns the hot cells into a cluster of full dots and the
/// cool ones into gaps.
fn ember_tint(u: f32, v: f32) -> [u8; 3] {
    const COAL: f32 = 0.14;
    let heat = hash01(
        0x454d_4245_5200_5f31,
        (u / COAL).floor() as i32,
        (v / COAL).floor() as i32,
    );
    if heat > 0.62 {
        [255, 188, 98]
    } else if heat > 0.26 {
        [236, 116, 38]
    } else {
        [142, 52, 18]
    }
}

/// Leaded panes: a warm hearth glow behind a dark cross-mullion, brightest at
/// the centre so a distant window reads as one hot dot rather than a block.
///
/// The centre-bright promise is kept by a rounded falloff inside each pane —
/// ±8% of the glow across the glass, the lead lattice still owns the shape.
/// Static and cache-safe by construction: same uv in, same glass out.
fn window_tint(u: f32, v: f32) -> [u8; 3] {
    let across = (u / 0.16).rem_euclid(1.0);
    let along = (v / 0.20).rem_euclid(1.0);
    let lead = across < 0.16 || along < 0.16;
    if lead {
        [96, 62, 34]
    } else {
        // A smooth dome over the pane: 1.0 at its centre, easing toward the
        // lead. Subtle is the law — the window still reads as glass, but a
        // distant pane now concentrates into the promised hot dot.
        let dx = (across - 0.5) / 0.5;
        let dy = (along - 0.5) / 0.5;
        let dome = (1.0 - dx * dx) * (1.0 - dy * dy);
        let scale = 0.92 + 0.08 * dome;
        [
            (255.0 * scale) as u8,
            (198.0 * scale) as u8,
            (116.0 * scale) as u8,
        ]
    }
}

/// Cold light: silver-blue, and **no lattice**.
///
/// The lattice is the whole reason the first interior light shafts had to be
/// deleted (interior.rs `light_pool`): they were tagged `WINDOW`, whose leaded
/// grid is sized for a pane, so stretched down two and a half tiles they read
/// as a glowing trellis propped against the wall. Light is not glazing. This
/// carries only a coarse grain — one cell per ~1.4 tiles, far too big for the
/// Bayer screen to turn into a weave — and a fall-off along `v`, so a slab
/// authored with its **sill corner at uv (0, 0)** thins as it reaches the
/// floor, the way a real shaft does.
fn moonlight_tint(u: f32, v: f32) -> [u8; 3] {
    // Steep on purpose. A slab of light that holds its value all the way to
    // the floor is a slab of *paint*; what tells the eye it is looking at air
    // is that the far end has almost gone.
    let fade = 1.0 / (1.0 + v.max(0.0) * 0.62);
    let grain = hash01(
        0x4D4F_4F4E_4C49_5445, // "MOONLITE"
        (u * 0.72).floor() as i32,
        (v * 0.72).floor() as i32,
    );
    scale_rgb(
        [150, 176, 218],
        (0.30 + 0.70 * fade) * (0.90 + 0.16 * grain),
    )
}

/// Hashed two-tone stipple on a fixed world grid — deterministic, world
/// anchored, and one hash per pixel.
fn speckle(a: [u8; 3], b: [u8; 3], u: f32, v: f32, scale: f32) -> [u8; 3] {
    let cell_x = (u * scale).floor() as i32;
    let cell_y = (v * scale).floor() as i32;
    let roll = hash01(0x4752_4f55_4e44_3344, cell_x, cell_y);
    mix_rgb(a, b, roll)
}

/// The noir night sky for a ray that hit nothing: near-black zenith into a dim
/// blue horizon, sparse bearing-anchored stars, and the moon at its fixed mark.
/// Below the horizon the world is fog, not sky, so the ground plane's edge
/// never shows as a bright band.
fn sky_color(dir: V3) -> [u8; 3] {
    if dir.z < 0.0 {
        return NIGHT_HAZE;
    }
    let elevation = dir.z.clamp(-1.0, 1.0).asin();
    let t = 1.0 - (elevation / SKY_BAND_RAD).clamp(0.0, 1.0);
    let bearing = dir.y.atan2(dir.x) / std::f32::consts::TAU;
    let bearing = bearing.rem_euclid(1.0);
    let mut color = mix_rgb(SKY_ZENITH, SKY_HORIZON, smoothstep(t));
    // A luminous band right on the horizon — moonlit haze. A mass that is
    // turned away from the moon has no lit
    // face of its own, so the only thing that can carry its silhouette is a sky
    // brighter than it is. Without this band a backlit tower is black on black.
    let glow = 1.0 - (elevation / HORIZON_GLOW_RAD).clamp(0.0, 1.0);
    color = mix_rgb(color, HORIZON_GLOW, glow * glow * HORIZON_GLOW_GAIN);

    // Stars sit on a fixed bearing×altitude grid but each is a pinpoint at a
    // hashed spot inside its cell — one lit dot, not a block, and no per-frame
    // twinkle to shimmer the dither.
    //
    // The row coordinate must be the *unclamped* altitude, not the sky
    // gradient's `t`: `t` saturates at 0 above `SKY_BAND_RAD`, so every
    // direction up there shares one row and a single star smears into a
    // vertical stripe running off the top of the frame. (Seen in the first
    // Phase-C vantage dump; the northern-rise plate grew a silver mast.)
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
            color = mix_rgb(color, [204, 218, 244], brightness);
        }
    }

    // One true angular distance to the disc: round moon, soft limb, wide halo.
    let angle = dir.dot(moon_direction()).clamp(-1.0, 1.0).acos();
    if angle < MOON_RADIUS_RAD {
        let limb = smoothstep((angle / MOON_RADIUS_RAD).clamp(0.0, 1.0));
        let disc = mix_rgb([230, 234, 242], MOON_SILVER, limb);
        color = mix_rgb(color, disc, 1.0 - limb * 0.30);
    } else if angle < MOON_HALO_RAD {
        let halo = 1.0 - (angle - MOON_RADIUS_RAD) / (MOON_HALO_RAD - MOON_RADIUS_RAD);
        color = mix_rgb(color, [92, 104, 138], halo * halo * 0.30);
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

/// SplitMix64 avalanche over two integer coordinates — the same compact,
/// state-free hash the raycast art uses, so cached frames stay byte-identical.
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
#[path = "../../../../../tests/cockpit/world_viz/world3d__raster__tests.rs"]
mod tests;
