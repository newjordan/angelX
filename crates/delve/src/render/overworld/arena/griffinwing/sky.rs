//! Griffinwing's sky, painted over the dots the 3D scene left bare: a dusk
//! that glows at the horizon and darkens to the zenith, clouds that slide
//! with the camera's heading (so a bend in the canyon turns the sky), stars
//! and the moon. The realm's own palette, dithered between neighbours.
//!
//! When the painted `griffinwing.sky` bands are whole they are laid instead:
//! each dot looks along its own ray (azimuth to a column, elevation to a row),
//! the dusk is its band and the cloud band side by side, the smoke and the
//! dawn their own band, and the painted moon stands where the code's moon
//! did (bearing 2.35, which the rail's heading never reaches: a canyon whose
//! heading stays within about half a radian of the way ahead never turns that
//! far). The code's sky below is the fallback and is unchanged.
use super::bank;
use crate::render::overworld::ink::{Img, Rgb, bayer, hash, nearest_plain, vnoise};
use crate::render::world3d::raster::View3;
use crate::rules::griffinwing::trig::Det;
use std::sync::OnceLock;

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Sky {
    Dusk,
    Smoke,
    Dawn,
}

fn ramp(sky: Sky) -> &'static [char] {
    match sky {
        Sky::Dusk => &['R', 'p', 'b', 'X', 'x', 's', 'K'],
        Sky::Smoke => &['p', 'b', 'Z', 'n', 'K', 'k', 'k'],
        Sky::Dawn => &['T', 't', 'O', 'o', 'R', 'S', 'x'],
    }
}

const BAND_H: i32 = 112;
const SCALE: f32 = 2.0;
const EL_BOTTOM: f32 = -0.14;
const MOON_AT: f32 = 2.35;
const MOON_COL: f32 = 233.0;
const KEEP: f32 = 0.8;

struct Strip {
    w: i32,
    px: Vec<Rgb>,
    zenith: Rgb,
    mirror: bool,
}

fn strip(sky: Sky) -> Option<&'static Strip> {
    static STRIPS: OnceLock<[Option<Strip>; 3]> = OnceLock::new();
    let all = STRIPS.get_or_init(|| {
        let make = |names: &[&str], mirror: bool| -> Option<Strip> {
            let bands: Option<Vec<&'static Img>> = names
                .iter()
                .map(|n| bank::pic("sky", n, 0).map(|p| &p.img))
                .collect();
            let bands = bands?;
            let w: i32 = bands.iter().map(|b| b.w).sum();
            let mut px = Vec::with_capacity((w * BAND_H) as usize);
            for y in 0..BAND_H {
                for b in &bands {
                    for x in 0..b.w {
                        let c = b.get(x, y).unwrap_or([0, 0, 0]);
                        px.push(nearest_plain(c.map(|v| f32::from(v) * KEEP)));
                    }
                }
            }
            let mut top: Vec<Rgb> = px[..w as usize].to_vec();
            top.sort_unstable();
            let (mut zenith, mut best, mut run) = (top[0], 0, 0);
            for (i, c) in top.iter().enumerate() {
                run = if i > 0 && *c == top[i - 1] { run + 1 } else { 1 };
                if run > best {
                    best = run;
                    zenith = *c;
                }
            }
            Some(Strip { w, px, zenith, mirror })
        };
        [
            make(&["dusk", "clouds"], false),
            make(&["smoke"], true),
            make(&["dawn"], true),
        ]
    });
    all[sky as usize].as_ref()
}

pub fn paint(im: &mut Img, depth: &[f32], view: &View3, sky: Sky, tick: u32) {
    match strip(sky) {
        Some(strip) => paint_strip(im, depth, view, sky, strip, tick),
        None => paint_code(im, depth, view, sky, tick),
    }
}

fn paint_strip(im: &mut Img, depth: &[f32], view: &View3, sky: Sky, strip: &Strip, tick: u32) {
    let (w, h) = (im.w, im.h);
    let f = (w as f32 / 2.0) / (view.fov_rad / 2.0).dtan();
    let phase = if sky == Sky::Dusk { MOON_AT + MOON_COL * SCALE / f } else { 0.0 };
    let drift = tick as f32 * 0.002 / 4.0 * f;
    let az: Vec<f32> = (0..w)
        .map(|x| view.heading_rad - ((x as f32 - w as f32 / 2.0) / f).datan())
        .collect();
    let cols: Vec<i32> = az
        .iter()
        .map(|a| {
            let u = (((phase - a) * f - drift) / SCALE).floor() as i64;
            let wide = i64::from(strip.w) * if strip.mirror { 2 } else { 1 };
            let c = u.rem_euclid(wide) as i32;
            if c >= strip.w { 2 * strip.w - 1 - c } else { c }
        })
        .collect();
    for y in 0..h {
        let el = view.pitch + ((h as f32 / 2.0 - y as f32) / f).datan();
        let v = (BAND_H - 1) as f32 - (el - EL_BOTTOM) * f / SCALE;
        let row = if v < 0.0 { None } else { Some((v as i32).min(BAND_H - 1)) };
        for x in 0..w {
            if depth
                .get((y * w + x) as usize)
                .is_some_and(|d| d.is_finite())
            {
                continue;
            }
            let c = match row {
                Some(r) => strip.px[(r * strip.w + cols[x as usize]) as usize],
                None => strip.zenith,
            };
            im.set(x, y, c);
        }
    }
}

fn paint_code(im: &mut Img, depth: &[f32], view: &View3, sky: Sky, tick: u32) {
    let (w, h) = (im.w, im.h);
    let f = (w as f32 / 2.0) / (view.fov_rad / 2.0).dtan();
    let inks = ramp(sky);
    let top = inks.len() as f32 - 1.0;
    for y in 0..h {
        let el = view.pitch + ((h as f32 / 2.0 - y as f32) / f).datan();
        let t = ((el + 0.12) / 1.05).clamp(0.0, 1.0);
        for x in 0..w {
            if depth
                .get((y * w + x) as usize)
                .is_some_and(|d| d.is_finite())
            {
                continue;
            }
            let az = view.heading_rad - ((x as f32 - w as f32 / 2.0) / f).datan();
            let mut v = t * (1.6 - 0.6 * t) * top;
            let n = vnoise(az * 4.0 + tick as f32 * 0.002, el * 22.0, 3) * 0.7
                + vnoise(az * 9.0, el * 44.0, 4) * 0.3;
            if (0.12..0.85).contains(&t) {
                if n > 0.60 {
                    v += 1.1;
                } else if n > 0.55 {
                    v -= 0.7;
                }
            }
            let v = v.clamp(0.0, top);
            let i = v.floor() as usize;
            let frac = v - i as f32;
            let mut ink = if frac > bayer(x, y) && i + 1 < inks.len() {
                inks[i + 1]
            } else {
                inks[i]
            };
            if sky != Sky::Dawn && t > 0.4 {
                let hs = hash((az * 170.0).floor() as i32, (el * 170.0).floor() as i32, 7);
                if hs % 520 == 0 {
                    ink = if hs % 3 == 0 { 'W' } else { 'v' };
                }
            }
            if sky == Sky::Dusk {
                let (maz, mel) = (2.35f32, 0.52f32);
                let da = (az - maz + std::f32::consts::PI).rem_euclid(std::f32::consts::TAU)
                    - std::f32::consts::PI;
                let d = ((da * el.dcos()).powi(2) + (el - mel).powi(2)).sqrt();
                if d < 0.05 {
                    ink = if d < 0.04 { 'W' } else { 'V' };
                } else if d < 0.085 && 1.0 - (d - 0.05) / 0.035 > bayer(x, y) * 1.4 {
                    ink = 'S';
                }
            }
            im.put(x, y, ink);
        }
    }
}
