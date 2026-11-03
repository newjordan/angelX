//! The sampler's pictures drawn on the CPU: a ray-traced still for the
//! inline-image dialect, a synthwave sunset for sixel, and the tunnel the
//! video showcase plays. Every colour is mixed in `f32` and written once.

use crate::motion::{add, mix, neon, scale, smooth, v3, Rgb, V3, WHITE};

fn to_byte(v: f32) -> u8 {
    (v.clamp(0.0, 1.0).powf(1.0 / 2.2) * 255.0).round() as u8
}

/// Linear light out to sRGB bytes, with a soft shoulder so a highlight
/// rolls off instead of clipping flat. For the ray tracer, which works in
/// linear light.
fn tone(c: Rgb) -> [u8; 4] {
    let shoulder = |v: f32| v / (1.0 + v * 0.18);
    [to_byte(shoulder(c[0])), to_byte(shoulder(c[1])), to_byte(shoulder(c[2])), 255]
}

/// Colours already chosen as they should look, out to bytes, with the
/// same soft shoulder. For the sunset and the tunnel.
fn display(c: Rgb) -> [u8; 4] {
    let byte = |v: f32| ((v / (1.0 + v * 0.12) * 1.06).clamp(0.0, 1.0) * 255.0).round() as u8;
    [byte(c[0]), byte(c[1]), byte(c[2]), 255]
}

// ------------------------------------------------------------- ray tracer --

struct Ball {
    centre: V3,
    radius: f32,
    color: Rgb,
    /// 0 matte .. 1 a mirror.
    mirror: f32,
    /// Shines by itself.
    glow: f32,
}

const BALLS: [Ball; 5] = [
    Ball { centre: v3(0.0, 1.0, 0.0), radius: 1.0, color: [0.95, 0.95, 0.97], mirror: 0.92, glow: 0.0 },
    Ball { centre: v3(-2.15, 0.62, 0.65), radius: 0.62, color: [1.0, 0.18, 0.55], mirror: 0.25, glow: 0.0 },
    Ball { centre: v3(2.05, 0.7, 0.35), radius: 0.7, color: [0.16, 0.78, 1.0], mirror: 0.3, glow: 0.0 },
    Ball { centre: v3(0.95, 0.33, 1.75), radius: 0.33, color: [1.0, 0.72, 0.2], mirror: 0.55, glow: 0.0 },
    Ball { centre: v3(-0.9, 0.26, 1.95), radius: 0.26, color: [0.36, 1.0, 0.7], mirror: 0.1, glow: 0.0 },
];

const SUN: V3 = v3(-0.55, 0.72, 0.42);

fn sky(dir: V3) -> Rgb {
    let up = dir.y.max(-0.2);
    let horizon: Rgb = [1.0, 0.52, 0.42];
    let zenith: Rgb = [0.025, 0.03, 0.14];
    let base = mix(horizon, zenith, smooth(-0.02, 0.30, up).powf(0.8));
    let sun = SUN.norm();
    let near = dir.dot(sun).max(0.0);
    add(add(base, scale([1.0, 0.75, 0.45], near.powf(60.0) * 3.0)), scale([1.0, 0.45, 0.6], near.powf(6.0) * 0.35))
}

fn hit_ball(origin: V3, dir: V3, ball: &Ball) -> Option<f32> {
    let oc = origin - ball.centre;
    let b = oc.dot(dir);
    let c = oc.dot(oc) - ball.radius * ball.radius;
    let h = b * b - c;
    if h < 0.0 {
        return None;
    }
    let t = -b - h.sqrt();
    (t > 1e-3).then_some(t)
}

/// How much of the sun reaches `p`: a soft shadow from how near each ball
/// passes the shadow ray.
fn sunlight(p: V3) -> f32 {
    let dir = SUN.norm();
    let mut light: f32 = 1.0;
    for ball in BALLS.iter().filter(|b| b.glow == 0.0) {
        let oc = ball.centre - p;
        let along = oc.dot(dir);
        if along <= 0.0 {
            continue;
        }
        let miss = (oc - dir * along).len() - ball.radius;
        light = light.min(smooth(0.0, 0.11 * along, miss));
    }
    light
}

fn trace(origin: V3, dir: V3, depth: u32) -> Rgb {
    let mut nearest = f32::MAX;
    let mut what: Option<usize> = None;
    for (i, ball) in BALLS.iter().enumerate() {
        if let Some(t) = hit_ball(origin, dir, ball)
            && t < nearest
        {
            nearest = t;
            what = Some(i);
        }
    }
    let floor = if dir.y < -1e-4 { -origin.y / dir.y } else { f32::MAX };
    if floor < nearest && floor < 60.0 {
        let p = origin + dir * floor;
        let checker = ((p.x * 1.2).floor() + (p.z * 1.2).floor()) as i32 & 1;
        let base: Rgb = if checker == 0 { [0.16, 0.12, 0.26] } else { [0.85, 0.82, 0.9] };
        let n = v3(0.0, 1.0, 0.0);
        let light = sunlight(p);
        let lit = scale(base, 0.12 + 0.95 * light * n.dot(SUN.norm()));
        let fog = smooth(6.0, 30.0, floor);
        let reflected = if depth > 0 {
            trace(p, v3(dir.x, -dir.y, dir.z), depth - 1)
        } else {
            sky(v3(dir.x, -dir.y, dir.z))
        };
        let shine = 0.18 + 0.5 * (1.0 + dir.y).powf(5.0);
        let surface = mix(lit, reflected, shine);
        return mix(surface, sky(dir), fog);
    }
    let Some(i) = what else {
        return sky(dir);
    };
    let ball = &BALLS[i];
    if ball.glow > 0.0 {
        return scale(ball.color, 4.0);
    }
    let p = origin + dir * nearest;
    let n = (p - ball.centre).norm();
    let sun = SUN.norm();
    let light = sunlight(p + n * 1e-3);
    let diffuse = n.dot(sun).max(0.0) * light;
    let half = (sun - dir).norm();
    let spec = n.dot(half).max(0.0).powf(90.0) * light;
    let fresnel = (1.0 + dir.dot(n)).clamp(0.0, 1.0).powf(4.0);
    let bounce = (n.y * -0.5 + 0.5) * 0.25;
    let matte = scale(ball.color, 0.08 + 0.9 * diffuse + bounce * 0.4);
    let reflect = dir - n * (2.0 * dir.dot(n));
    let mirrored = if depth > 0 { trace(p + n * 1e-3, reflect, depth - 1) } else { sky(reflect) };
    let tinted = [mirrored[0] * ball.color[0], mirrored[1] * ball.color[1], mirrored[2] * ball.color[2]];
    let k = (ball.mirror + (1.0 - ball.mirror) * fresnel * 0.6).clamp(0.0, 1.0);
    add(mix(matte, tinted, k), scale(WHITE, spec * 2.5))
}

/// Five balls on a reflecting checkerboard at sunset: a mirror in the
/// middle, glossy colour round it, soft shadows, four rays a pixel.
pub fn raytrace(w: u32, h: u32) -> Vec<u8> {
    let w = w.max(1);
    let h = h.max(1);
    let eye = v3(0.0, 1.65, 6.2);
    let target = v3(0.0, 0.75, 0.0);
    let forward = (target - eye).norm();
    let right = forward.cross(v3(0.0, 1.0, 0.0)).norm();
    let up = right.cross(forward);
    let zoom = 1.0 / (0.42f32).tan();
    let aspect = w as f32 / h as f32;
    let mut rgba = Vec::with_capacity((w as usize) * (h as usize) * 4);
    let offsets = [(0.25, 0.25), (0.75, 0.25), (0.25, 0.75), (0.75, 0.75)];
    for y in 0..h {
        for x in 0..w {
            let mut sum = [0.0f32; 3];
            for (ox, oy) in offsets {
                let sx = ((x as f32 + ox) / w as f32 * 2.0 - 1.0) * aspect;
                let sy = 1.0 - (y as f32 + oy) / h as f32 * 2.0;
                let dir = (forward * zoom + right * sx + up * sy).norm();
                sum = add(sum, trace(eye, dir, 3));
            }
            let px = x as f32 / w as f32 - 0.5;
            let py = y as f32 / h as f32 - 0.5;
            let vignette = 1.0 - 0.55 * (px * px + py * py);
            rgba.extend_from_slice(&tone(scale(sum, 0.25 * vignette)));
        }
    }
    rgba
}

// ------------------------------------------------------------- synthwave --

/// A synthwave sunset: a striped sun over dark mountains with a neon rim, a
/// perspective grid running to the horizon, stars, and the sun's glow
/// laid on the floor.
pub fn synthwave(w: u32, h: u32) -> Vec<u8> {
    let w = w.max(1);
    let h = h.max(1);
    let horizon = 0.62;
    let aspect = w as f32 / h as f32;
    let mut rgba = Vec::with_capacity((w as usize) * (h as usize) * 4);
    for y in 0..h {
        for x in 0..w {
            let u = (x as f32 + 0.5) / w as f32;
            let v = (y as f32 + 0.5) / h as f32;
            let sx = (u - 0.5) * aspect;
            let mut c: Rgb;
            if v < horizon {
                let k = v / horizon;
                c = mix([0.03, 0.01, 0.10], [0.30, 0.04, 0.38], smooth(0.0, 0.7, k));
                c = mix(c, [0.95, 0.30, 0.40], smooth(0.55, 1.0, k));
                let star = crate::motion::hash(x / 2 + (y / 2) * 7919);
                if star > 0.996 && k < 0.6 {
                    c = add(c, scale(WHITE, (star - 0.996) * 180.0 * (1.0 - k)));
                }
                let (cx, cy, r) = (0.0, horizon * 0.70, 0.24);
                let d = (sx - cx).hypot(v - cy);
                if d < r {
                    let s = ((v - (cy - r)) / (2.0 * r)).clamp(0.0, 1.0);
                    let sun = mix([1.0, 0.92, 0.35], [1.0, 0.18, 0.55], s);
                    let below = (v - cy + 0.02).max(0.0) / r;
                    let band = ((v - cy) / (r * 0.13)).fract();
                    let cut = below > 0.0 && band < (0.1 + below * 0.55);
                    let edge = smooth(r, r - 0.006, d);
                    if !cut {
                        c = mix(c, sun, edge);
                    }
                }
                c = add(c, scale([1.0, 0.35, 0.55], (-(d - r).max(0.0) * 7.0).exp() * 0.45));
                let ridge = horizon - 0.05
                    - 0.07 * (0.5 + 0.5 * (sx * 5.0 + 1.3).sin()) * (0.6 + 0.4 * (sx * 13.0).sin())
                    - 0.03 * (sx * 23.0).sin().abs();
                if v > ridge {
                    let rim = smooth(0.006, 0.0, v - ridge);
                    c = mix([0.06, 0.02, 0.12], [1.0, 0.25, 0.75], rim);
                }
            } else {
                let depth = (v - horizon).max(1e-3);
                let z = 0.12 / depth;
                let gx = sx * z * 7.0;
                let gz = z * 7.0 - 0.0;
                let line = |g: f32, fw: f32| {
                    let f = (g - g.round()).abs();
                    smooth(fw, 0.0, f)
                };
                let fw_x = (z * 7.0 / h as f32 * 2.2).clamp(0.01, 0.5);
                let fw_z = (z * z * 7.0 / 0.12 / h as f32 * 1.5).clamp(0.01, 0.5);
                // A line finer than about a pixel only aliases: it fades out
                // as it gets that fine, so the horizon is a glow, not a moire.
                let grid = (line(gx, fw_x) * smooth(0.30, 0.10, fw_x)).max(line(gz, fw_z) * smooth(0.30, 0.10, fw_z));
                let fade = smooth(0.0, 0.3, depth);
                let floor = mix([0.06, 0.0, 0.10], [0.02, 0.0, 0.05], fade);
                let neon_line = mix([1.0, 0.2, 0.7], [0.3, 0.8, 1.0], smooth(0.0, 0.4, depth));
                c = mix(floor, neon_line, grid * (0.35 + 0.65 * fade));
                let glow = (-(sx * sx) * 18.0).exp() * (1.0 - smooth(0.0, 0.25, depth));
                c = add(c, scale([1.0, 0.4, 0.5], glow * 0.5));
                c = add(c, scale([1.0, 0.3, 0.6], (1.0 - smooth(0.0, 0.03, depth)) * 0.5));
            }
            rgba.extend_from_slice(&display(c));
        }
    }
    rgba
}

// ----------------------------------------------------------------- tunnel --

/// One frame of a neon tunnel flying forward: a pipe of lit tiles that
/// twists as it goes, bright seams between them, fog to a white-hot
/// vanishing point that wanders.
pub fn tunnel(i: u64, frames: u64, w: u32, h: u32) -> Vec<u8> {
    let w = w.max(1);
    let h = h.max(1);
    let t = i as f32 / frames.max(1) as f32 * 3.0;
    let mut rgba = Vec::with_capacity((w as usize) * (h as usize) * 4);
    let cx = 0.5 + 0.07 * (t * 1.3).sin();
    let cy = 0.5 + 0.05 * (t * 1.7).cos();
    for y in 0..h {
        for x in 0..w {
            let dx = (x as f32 / w as f32 - cx) * w as f32 / h as f32;
            let dy = y as f32 / h as f32 - cy;
            let r = dx.hypot(dy).max(1e-3);
            let a = dy.atan2(dx) / std::f32::consts::TAU + 0.5;
            let depth = 0.30 / r;
            let u = a * 14.0 + depth * 0.35 + t * 0.6;
            let v = depth * 4.5 + t * 6.0;
            let (fu, fv) = (u - u.floor(), v - v.floor());
            let tile = ((u.floor() + v.floor()) as i32) & 1;
            // Seams thin with distance in screen terms: their width is in
            // tile units, and a far tile is small.
            let fw = (0.03 * depth).clamp(0.03, 0.35);
            let seam_u = smooth(fw, 0.0, fu.min(1.0 - fu));
            let seam_v = smooth(fw * 1.4, 0.0, fv.min(1.0 - fv));
            let seam = seam_u.max(seam_v);
            let hue = neon(v * 0.05 + a * 2.0);
            let bevel = 0.75 + 0.25 * ((fu * std::f32::consts::PI).sin() * (fv * std::f32::consts::PI).sin());
            let wall = scale(hue, if tile == 0 { 0.10 } else { 0.42 * bevel });
            let lit = add(wall, scale(mix(hue, WHITE, 0.35), seam * 1.4));
            let fog = smooth(0.015, 0.42, r);
            let core = (-r * r * 70.0).exp();
            let halo = (-r * 9.0).exp() * 0.35;
            let c = add(scale(lit, fog), scale(mix(neon(t * 0.15), WHITE, 0.55), core * 1.6 + halo));
            let vignette = 1.0 - 0.35 * (dx * dx + dy * dy);
            rgba.extend_from_slice(&display(scale(c, vignette)));
        }
    }
    rgba
}

#[cfg(test)]
mod tests {
    use super::*;

    fn px(buf: &[u8], w: u32, x: u32, y: u32) -> [u8; 4] {
        let i = ((y * w + x) * 4) as usize;
        [buf[i], buf[i + 1], buf[i + 2], buf[i + 3]]
    }

    #[test]
    fn the_raytrace_has_a_mirror_a_shadow_and_a_sky() {
        let (w, h) = (96, 64);
        let picture = raytrace(w, h);
        assert_eq!(picture.len(), (w * h * 4) as usize);
        let lum = |p: [u8; 4]| u32::from(p[0]) + u32::from(p[1]) + u32::from(p[2]);
        let top = px(&picture, w, w / 2, 1);
        let floor = px(&picture, w, 2, h - 2);
        assert_ne!(top, floor, "sky and floor differ");
        let mut seen = std::collections::BTreeSet::new();
        for p in picture.chunks(4) {
            seen.insert([p[0] / 8, p[1] / 8, p[2] / 8]);
        }
        assert!(seen.len() > 150, "a ray-traced still is more than a few colours: {}", seen.len());
        let lums: Vec<u32> = picture.chunks(4).map(|p| lum([p[0], p[1], p[2], 255])).collect();
        let (brightest, darkest) = (*lums.iter().max().unwrap(), *lums.iter().min().unwrap());
        assert!(brightest > 700, "a highlight: {brightest}");
        assert!(brightest > darkest + 450, "shadow {darkest} against highlight {brightest}");
    }

    #[test]
    fn the_sunset_has_a_sun_a_grid_and_a_dark_sky() {
        let lum = |p: [u8; 4]| u32::from(p[0]) + u32::from(p[1]) + u32::from(p[2]);
        let (w, h) = (160, 90);
        let picture = synthwave(w, h);
        assert_eq!(picture.len(), (w * h * 4) as usize);
        let sun = px(&picture, w, w / 2, (h as f32 * 0.62 * 0.55) as u32);
        assert!(sun[0] > 200 && sun[1] > 120, "the sun's upper half is warm and bright: {sun:?}");
        let zenith = px(&picture, w, 3, 1);
        assert!(u32::from(zenith[0]) + u32::from(zenith[1]) + u32::from(zenith[2]) < 120, "{zenith:?}");
        let floor_row = h - 3;
        let lit = (0..w).filter(|x| lum(px(&picture, w, *x, floor_row)) > 250).count();
        assert!(lit > 2 && lit < w as usize, "grid lines cross the floor: {lit}");
    }

    /// `SAMPLER_DUMP=dir cargo test --release --example glass_sampler --
    /// --ignored dump_stills` writes each still as a PNG to look at.
    #[test]
    #[ignore]
    fn dump_stills() {
        let dir = std::env::var("SAMPLER_DUMP").expect("SAMPLER_DUMP names a directory");
        let save = |name: &str, w: u32, h: u32, rgba: Vec<u8>| {
            image::save_buffer(format!("{dir}/{name}.png"), &rgba, w, h, image::ExtendedColorType::Rgba8)
                .expect("saved");
        };
        save("raytrace", 960, 600, raytrace(960, 600));
        save("synthwave", 960, 480, synthwave(960, 480));
        save("tunnel", 480, 270, tunnel(17, 90, 480, 270));
    }

    #[test]
    fn tunnel_frames_fill_and_fly() {
        let a = tunnel(0, 60, 80, 45);
        let b = tunnel(5, 60, 80, 45);
        assert_eq!(a.len(), 80 * 45 * 4);
        assert_ne!(a, b);
        assert!(a.chunks(4).all(|p| p[3] == 255));
    }
}
