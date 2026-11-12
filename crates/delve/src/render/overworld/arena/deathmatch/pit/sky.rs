//! What shows over the wall: a smoke-dusk sky going to night overhead, with
//! a few stars and the haze the braziers breathe up. The gradient and its
//! smoke are laid down once in a table of bearing and height.
use super::Frame;
use crate::render::overworld::ink::{hash, vnoise};
use std::sync::OnceLock;

pub(super) const HAZE: [f32; 3] = [0.17, 0.095, 0.06];
const AZ: usize = 256;
const EL: usize = 48;
const TOP: f32 = 0.6;

fn table() -> &'static [[f32; 3]] {
    static TAB: OnceLock<Vec<[f32; 3]>> = OnceLock::new();
    TAB.get_or_init(|| {
        let zen = [0.012, 0.014, 0.03];
        let mut v = Vec::with_capacity(AZ * EL);
        for e in 0..EL {
            let up = ((e as f32 + 0.5) / EL as f32).clamp(0.0, 1.0);
            let k = up.sqrt();
            for a in 0..AZ {
                let az = a as f32 / AZ as f32 * std::f32::consts::TAU;
                // Smoke drifting in bands across the haze (wrapping round the bearing).
                let band = vnoise(az.cos() * 2.2 + 9.0, az.sin() * 2.2 + e as f32 * 0.35, 0x71);
                let lift = (band - 0.45).max(0.0) * 0.5 * (1.0 - up);
                v.push([
                    HAZE[0] + (zen[0] - HAZE[0]) * k + lift * 0.09,
                    HAZE[1] + (zen[1] - HAZE[1]) * k + lift * 0.05,
                    HAZE[2] + (zen[2] - HAZE[2]) * k + lift * 0.03,
                ]);
            }
        }
        v
    })
}

/// `atan2` good to a ten-thousandth of a radian, with no library call.
#[inline]
fn bearing(y: f32, x: f32) -> f32 {
    let (ax, ay) = (x.abs(), y.abs());
    let (mx, mn) = (ax.max(ay), ax.min(ay));
    let a = mn / mx.max(1e-9);
    let s = a * a;
    let mut r = ((-0.046_496_47 * s + 0.159_314_22) * s - 0.327_622_76) * s * a + a;
    if ay > ax {
        r = std::f32::consts::FRAC_PI_2 - r;
    }
    if x < 0.0 {
        r = std::f32::consts::PI - r;
    }
    if y < 0.0 { -r } else { r }
}

pub(super) fn color(dir: [f32; 3], ticks: u32, frame: &Frame) -> [f32; 3] {
    let len2 = dir[0] * dir[0] + dir[1] * dir[1] + dir[2] * dir[2];
    let s = (dir[2] / len2.sqrt().max(1e-6)).clamp(0.0, 0.999);
    let az = bearing(dir[1], dir[0]);
    let ai = (((az + std::f32::consts::PI) * (AZ as f32 / std::f32::consts::TAU)) as usize + (ticks / 9) as usize) % AZ;
    let ei = ((s / TOP * EL as f32) as usize).min(EL - 1);
    let mut c = table()[ei * AZ + ai];
    // Stars: one dot where a cell's hash says so, twinkling.
    if s > 0.12 {
        let (sa, se) = ((az * frame.px_per_rad) as i32, (s * frame.px_per_rad) as i32);
        let h = hash(sa, se, 0x57);
        if h % 211 == 0 {
            let up = (s / TOP).min(1.0);
            let twinkle = 0.55 + 0.45 * (((ticks / 5 + (h >> 8)) % 4) as f32 / 3.0);
            let v = (0.35 + 0.4 * ((h >> 16) & 0xff) as f32 / 255.0) * twinkle * (0.4 + 0.6 * up);
            c = [c[0] + v, c[1] + v * 0.95, c[2] + v * 0.8];
        }
    }
    c
}
