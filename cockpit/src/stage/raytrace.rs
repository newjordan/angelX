//! A tiny CPU raytracer rendered into a ratatui buffer — a rotating cube wearing
//! a rainbow debug-checker texture. It is a *real* raytracer: one ray is cast per
//! sub-cell pixel and intersected against the cube as an axis-aligned box (the
//! classic slab test) in the cube's own rotated frame. No GPU, no extra deps.
//!
//! Resolution trick: each terminal cell is split into two stacked pixels with the
//! `▀` upper-half block — foreground paints the top pixel, background the bottom —
//! so a `w × h` cell panel is a `w × 2h` RGB framebuffer. Pure `Text<'static>`, so
//! it composes like any other panel; driven purely by `time` (elapsed seconds), so
//! the cockpit's redraw loop tumbles it for free (see `anim::vortex` for the same
//! time-driven, allocation-light pattern).

use ratatui::{
    style::{Color, Style},
    text::{Line, Span, Text},
};

/// Upper-half block: fg = top pixel, bg = bottom pixel of the cell.
const HALF_BLOCK: &str = "\u{2580}";
/// Camera sits on +Z looking at the origin; the cube is the box [-1,1]^3.
const CAM_DIST: f32 = 4.0;
/// tan(fov/2): with CAM_DIST=4 the world half-height at z=0 is 2.4, so the cube
/// (reach ~1.73 when tumbling) fills the frame with a comfortable margin.
const TAN_HALF_FOV: f32 = 0.6;
/// Keep frames cheap and bound allocation even if handed a silly-large area.
const MAX_CELLS: u16 = 600;

#[derive(Clone, Copy)]
struct V3 {
    x: f32,
    y: f32,
    z: f32,
}

impl V3 {
    fn new(x: f32, y: f32, z: f32) -> Self {
        V3 { x, y, z }
    }
    fn dot(self, o: V3) -> f32 {
        self.x * o.x + self.y * o.y + self.z * o.z
    }
    fn normalize(self) -> V3 {
        let len = self.dot(self).sqrt();
        if len <= f32::EPSILON {
            self
        } else {
            V3::new(self.x / len, self.y / len, self.z / len)
        }
    }
}

/// Precomputed sin/cos of the two tumble angles — once per frame, never per pixel.
struct Spin {
    sax: f32,
    cax: f32,
    say: f32,
    cay: f32,
}

impl Spin {
    fn at(time: f32) -> Self {
        let (sax, cax) = (time * 0.55).sin_cos();
        let (say, cay) = (time * 0.80).sin_cos();
        Spin { sax, cax, say, cay }
    }

    /// World → cube-local: undo the cube's rotation so we can intersect an AABB.
    /// Inverse of `rot_y(rot_x(v))`, i.e. `rot_x(-ax)·rot_y(-ay)·v`.
    fn to_local(&self, v: V3) -> V3 {
        let x1 = v.x * self.cay - v.z * self.say;
        let z1 = v.x * self.say + v.z * self.cay;
        let y2 = v.y * self.cax + z1 * self.sax;
        let z2 = -v.y * self.sax + z1 * self.cax;
        V3::new(x1, y2, z2)
    }

    /// Cube-local → world, for turning a face normal back into the lit frame.
    /// Forward rotation `rot_y(ay)·rot_x(ax)·v`.
    fn to_world(&self, v: V3) -> V3 {
        let y1 = v.y * self.cax - v.z * self.sax;
        let z1 = v.y * self.sax + v.z * self.cax;
        let x2 = v.x * self.cay + z1 * self.say;
        let z2 = -v.x * self.say + z1 * self.cay;
        V3::new(x2, y1, z2)
    }
}

/// Render the rotating cube into a `width × height` cell panel (each cell = two
/// vertically-stacked RGB pixels). `time` is elapsed seconds.
pub fn render(time: f32, width: u16, height: u16) -> Text<'static> {
    let w = width.min(MAX_CELLS) as usize;
    let h = height.min(MAX_CELLS) as usize;
    if w == 0 || h == 0 {
        return Text::raw("");
    }

    let pw = w; // pixels wide  (1 per cell column)
    let ph = h * 2; // pixels tall (2 per cell row, top/bottom)
    let aspect = pw as f32 / ph as f32;
    let spin = Spin::at(time);
    let light = V3::new(0.4, 0.78, 0.5).normalize();
    let origin = V3::new(0.0, 0.0, CAM_DIST);
    // Camera ray dirs and the box test are in cube-local space, so transform the
    // camera once instead of the cube — the eye and the AABB then stay fixed.
    let local_origin = spin.to_local(origin);

    let shade_pixel = |px: usize, py: usize| -> Color {
        // Pixel center → normalized screen coords (y points up).
        let sx = ((px as f32 + 0.5) / pw as f32 * 2.0 - 1.0) * aspect * TAN_HALF_FOV;
        let sy = (1.0 - (py as f32 + 0.5) / ph as f32 * 2.0) * TAN_HALF_FOV;
        let dir = spin.to_local(V3::new(sx, sy, -1.0)).normalize();
        match intersect_cube(local_origin, dir) {
            Some(hit) => shade(hit, &spin, light),
            None => background(px, py, pw, ph),
        }
    };

    let mut lines = Vec::with_capacity(h);
    for cy in 0..h {
        let mut spans = Vec::with_capacity(w);
        for cx in 0..w {
            let top = shade_pixel(cx, cy * 2);
            let bottom = shade_pixel(cx, cy * 2 + 1);
            spans.push(Span::styled(HALF_BLOCK, Style::new().fg(top).bg(bottom)));
        }
        lines.push(Line::from(spans));
    }
    Text::from(lines)
}

/// What a ray hit on the cube: the local-space point, the face's local normal,
/// and the in-face UV in [0,1] for texturing.
struct Hit {
    normal: V3,
    u: f32,
    v: f32,
    axis: usize,
}

/// Ray vs the AABB [-1,1]^3 via the slab method. Returns the entry face if the
/// ray enters the box in front of the eye.
fn intersect_cube(o: V3, d: V3) -> Option<Hit> {
    let oa = [o.x, o.y, o.z];
    let da = [d.x, d.y, d.z];
    let mut t_near = f32::NEG_INFINITY;
    let mut t_far = f32::INFINITY;
    let mut near_axis = 0usize;
    for axis in 0..3 {
        let inv = 1.0 / da[axis];
        let mut t0 = (-1.0 - oa[axis]) * inv;
        let mut t1 = (1.0 - oa[axis]) * inv;
        if t0 > t1 {
            std::mem::swap(&mut t0, &mut t1);
        }
        if t0 > t_near {
            t_near = t0;
            near_axis = axis;
        }
        if t1 < t_far {
            t_far = t1;
        }
        if t_near > t_far {
            return None;
        }
    }
    // The nearer hit in front of the eye (or the exit face if we start inside).
    let t = if t_near > 1e-3 { t_near } else { t_far };
    if t <= 1e-3 {
        return None;
    }

    let p = [o.x + d.x * t, o.y + d.y * t, o.z + d.z * t];
    let mut normal = [0.0f32; 3];
    // Face normal opposes the ray on the axis we entered through.
    normal[near_axis] = if da[near_axis] < 0.0 { 1.0 } else { -1.0 };

    // UV from the two coords tangent to the hit face, remapped [-1,1] → [0,1].
    let (ua, va) = match near_axis {
        0 => (p[2], p[1]),
        1 => (p[0], p[2]),
        _ => (p[0], p[1]),
    };
    Some(Hit {
        normal: V3::new(normal[0], normal[1], normal[2]),
        u: (ua * 0.5 + 0.5).clamp(0.0, 1.0),
        v: (va * 0.5 + 0.5).clamp(0.0, 1.0),
        axis: near_axis,
    })
}

/// Texture the hit with a rainbow checker and apply simple Lambert + ambient.
fn shade(hit: Hit, spin: &Spin, light: V3) -> Color {
    // Each axis-pair gets its own hue band so the three visible faces read apart;
    // hue then sweeps across the face for the rainbow gradient.
    let face_hue = hit.axis as f32 / 3.0;
    let mut hue = (face_hue + hit.u * 0.5 + hit.v * 0.25).rem_euclid(1.0);
    let (mut sat, mut val) = (0.85f32, 1.0f32);

    // Debug checker: darken alternate squares, and light up a thin face border so
    // the cube's edges stay crisp as it tumbles.
    const N: f32 = 5.0;
    let checker = ((hit.u * N).floor() as i32 + (hit.v * N).floor() as i32) & 1 == 0;
    if checker {
        val *= 0.6;
    }
    let edge = 0.045;
    if hit.u < edge || hit.u > 1.0 - edge || hit.v < edge || hit.v > 1.0 - edge {
        sat = 0.0;
        val = 1.0;
        hue = 0.0;
    }

    let (mut r, mut g, mut b) = hsv_to_rgb(hue, sat, val);

    let world_n = spin.to_world(hit.normal);
    let lambert = world_n.dot(light).max(0.0);
    let lit = 0.32 + 0.68 * lambert; // ambient floor + diffuse
    r *= lit;
    g *= lit;
    b *= lit;
    Color::Rgb(to_u8(r), to_u8(g), to_u8(b))
}

/// Backdrop behind the cube: a dim top-to-bottom slate gradient with a soft
/// vignette toward the edges — quiet enough that the cube stays the subject.
fn background(px: usize, py: usize, pw: usize, ph: usize) -> Color {
    // Colors are in [0,1] (like `shade`), since `to_u8` scales by 255.
    let ty = py as f32 / (ph.max(1) as f32);
    let base_r = 0.047 - 0.035 * ty;
    let base_g = 0.063 - 0.047 * ty;
    let base_b = 0.118 - 0.086 * ty;
    // Distance from center (in roughly square pixel space) → vignette falloff.
    let nx = (px as f32 + 0.5) / pw as f32 * 2.0 - 1.0;
    let ny = (py as f32 + 0.5) / ph as f32 * 2.0 - 1.0;
    let vig = (1.0 - 0.55 * (nx * nx + ny * ny)).clamp(0.0, 1.0);
    Color::Rgb(
        to_u8(base_r * vig),
        to_u8(base_g * vig),
        to_u8(base_b * vig),
    )
}

fn to_u8(c: f32) -> u8 {
    (c * 255.0).round().clamp(0.0, 255.0) as u8
}

/// HSV → linear RGB in [0,1]. `h`, `s`, `v` all in [0,1].
fn hsv_to_rgb(h: f32, s: f32, v: f32) -> (f32, f32, f32) {
    let h6 = (h.rem_euclid(1.0)) * 6.0;
    let c = v * s;
    let x = c * (1.0 - (h6 % 2.0 - 1.0).abs());
    let m = v - c;
    let (r, g, b) = match h6 as i32 {
        0 => (c, x, 0.0),
        1 => (x, c, 0.0),
        2 => (0.0, c, x),
        3 => (0.0, x, c),
        4 => (x, 0.0, c),
        _ => (c, 0.0, x),
    };
    (r + m, g + m, b + m)
}

#[cfg(test)]
#[path = "../../../tests/cockpit/app/raytrace__tests.rs"]
mod tests;
