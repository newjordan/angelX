//! Local accepted-guide material samples and pose studies, decoded once.
use super::super::super::ink::Img;
use super::paint::*;
use std::sync::OnceLock;
macro_rules! assets {
    ($($name:literal),* $(,)?) => { pub(super) fn get(name:&str)->&'static Img {
        $(if name==$name {static IMG:OnceLock<Img>=OnceLock::new();return IMG.get_or_init(||decode(include_bytes!(concat!("../../../../../../../cockpit/assets/dungeon/art-v2/",$name,".png")))) ;})*
        panic!("missing v2 kit sample {name}");
    }};
}
fn decode(bytes: &[u8]) -> Img {
    let png = image::load_from_memory(bytes)
        .expect("local v2 kit PNG")
        .to_rgba8();
    let mut im = Img::new(png.width() as i32, png.height() as i32);
    for (x, y, c) in png.enumerate_pixels() {
        if c[3] > 127 {
            im.set(
                x as i32,
                y as i32,
                crate::render::art::outline([c[0], c[1], c[2]]),
            );
        }
    }
    im
}
assets!(
    "floor-0",
    "floor-1",
    "floor-2",
    "floor-3",
    "floor-4",
    "floor-5",
    "wall-0",
    "wall-1",
    "wall-2",
    "wall-3",
    "wood",
    "grass",
    "gravel",
    "knight-red",
    "knight-blue",
    "knight-green",
    "knight-gold",
    "knight-run",
    "knight-back",
    "ghost",
    "slime",
    "skeleton",
    "imp",
    "helios",
    "helios-calm",
    "horse-brown",
    "horse-grey",
    "horse-tan",
    "horse-black",
    "ripper",
    "engine-boiler",
    "canopy",
    "hall-window",
    "hall-hearth",
    "hall-table",
    "hall-bench",
    "gas-core",
    "autumn-canopy",
    "grey-canopy",
    "water",
    "map-grass",
    "map-road",
    "college-facade",
    "gate-dome",
    "barn",
    "stall",
    "horse-bay",
    "horse-ranch-grey",
    "pen",
    "privy-house",
    "privy-wall",
    "race-dirt",
    "cart-gold",
    "cart-blue",
    "cart-teal",
    "hall-stone",
    "hall-pillar"
);
/// Sample density matches the 768 canvas: no extra 2x legacy enlargement.
pub(super) fn sprite(
    cv: &mut Canvas,
    im: &Img,
    x: f32,
    y: f32,
    east: bool,
    scale: f32,
    tint: Option<u32>,
    stride: f32,
) {
    let w = (im.w as f32 * scale).round() as i32;
    let h = (im.h as f32 * scale).round() as i32;
    let ox = ((x - cv.origin.0) * 2.0).round() as i32;
    let oy = ((y - cv.origin.1) * 2.0).round() as i32;
    for yy in 0..h {
        for xx in 0..w {
            let sx = (xx as f32 / scale) as i32;
            let sy = (yy as f32 / scale) as i32;
            let sx = if east { sx } else { im.w - 1 - sx };
            if let Some(mut c) = im.get(sx, sy) {
                if let Some(id) = tint {
                    if c[0] > c[1].saturating_add(24) && c[0] > c[2].saturating_add(20) {
                        let r = match id % 4 {
                            2 => Ramp::inks('s', '1', 'z'),
                            3 => Ramp::inks('f', 'e', 'M'),
                            0 => Ramp::inks('b', 'o', 'T'),
                            _ => Ramp::inks('b', '7', 'R'),
                        };
                        c = r.at(c[0] as f32 / 255.0);
                    }
                }
                let step = if yy > h * 3 / 4 {
                    if xx < w / 2 { stride } else { -stride }
                } else {
                    0.0
                };
                let (px, py) = (ox + xx + step.round() as i32, oy + yy);
                if px >= 0 && py >= 0 && (px as usize) < cv.w && (py as usize) < cv.h {
                    cv.px[py as usize * cv.w + px as usize] = c;
                }
            }
        }
    }
}
pub(super) fn texture(cv: &mut Canvas, name: &str, brightness: f32) {
    let im = get(name);
    for y in 0..cv.h {
        for x in 0..cv.w {
            let wx = x as i32 + (cv.origin.0 * 2.0) as i32;
            let wy = y as i32 + (cv.origin.1 * 2.0) as i32;
            let tx = wx.div_euclid(im.w);
            let ty = wy.div_euclid(im.h);
            let v = super::super::super::ink::hash(tx, ty, 61);
            let sx = if v % 2 == 0 {
                wx.rem_euclid(im.w)
            } else {
                im.w - 1 - wx.rem_euclid(im.w)
            };
            let sy = wy.rem_euclid(im.h);
            if let Some(c) = im.get(sx, sy) {
                cv.px[y * cv.w + x] = c.map(|c| (c as f32 * brightness).min(255.0) as u8);
            }
        }
    }
}

pub(super) fn area(cv: &mut Canvas, name: &str, x: f32, y: f32, w: f32, h: f32, brightness: f32) {
    let im = get(name);
    let x0 = ((x - cv.origin.0) * 2.0).floor().clamp(0.0, cv.w as f32) as usize;
    let y0 = ((y - cv.origin.1) * 2.0).floor().clamp(0.0, cv.h as f32) as usize;
    let x1 = ((x + w - cv.origin.0) * 2.0).ceil().clamp(0.0, cv.w as f32) as usize;
    let y1 = ((y + h - cv.origin.1) * 2.0).ceil().clamp(0.0, cv.h as f32) as usize;
    for yy in y0..y1 {
        for xx in x0..x1 {
            if let Some(c) = im.get(
                (xx as i32 + (cv.origin.0 * 2.0) as i32).rem_euclid(im.w),
                (yy as i32 + (cv.origin.1 * 2.0) as i32).rem_euclid(im.h),
            ) {
                cv.px[yy * cv.w + xx] = c.map(|v| (v as f32 * brightness).min(255.0) as u8);
            }
        }
    }
}

/// Retained Helios emission mask: its cold material silhouette stays pixels;
/// the gold-to-ivory mane, tail and fire are coloured braille atmosphere.
pub(super) fn emission(
    cv: &mut Canvas,
    full: &Img,
    cold: &Img,
    x: f32,
    y: f32,
    east: bool,
    scale: f32,
) {
    let (w, h) = (
        (full.w as f32 * scale).round() as i32,
        (full.h as f32 * scale).round() as i32,
    );
    let (ox, oy) = (
        ((x - cv.origin.0) * 2.0).round() as i32,
        ((y - cv.origin.1) * 2.0).round() as i32,
    );
    for yy in 0..h {
        for xx in 0..w {
            let sx = (xx as f32 / scale) as i32;
            let sx = if east { sx } else { full.w - 1 - sx };
            let sy = (yy as f32 / scale) as i32;
            if let Some(c) = full.get(sx, sy).filter(|c| Some(*c) != cold.get(sx, sy)) {
                let (px, py) = (ox + xx, oy + yy);
                if px >= 0 && py >= 0 && (px as usize) < cv.w && (py as usize) < cv.h {
                    cv.fx[py as usize * cv.w + px as usize] = c.map(|v| v.min(226));
                }
            }
        }
    }
}
