//! Anchored village modules, construction stages and real multi-frame workers.
//! `(x,y)` always names the ground contact. `size` is the group's reference
//! span; citizens use body height. Tools and smoke may extend beyond that span.
use super::super::ink::Img;

struct VillageSprite {
    id: &'static str,
    size: u32,
    frame: usize,
    width: u32,
    height: u32,
    anchor: [i32; 2],
    pixels: &'static [u8],
}
include!(concat!(env!("OUT_DIR"), "/village_art_registry.rs"));

fn frame_at(id: &str, clip: &str, tick: u64) -> usize {
    VILLAGE_CLIPS
        .iter()
        .find(|(name, animation, _, _)| *name == id && *animation == clip)
        .map(|(_, _, ms, frames)| {
            frames[((tick.saturating_mul(1000) / u64::from(crate::rules::HZ) / ms) as usize)
                % frames.len()]
        })
        .unwrap_or(0)
}

fn blend(im: &mut Img, x: i32, y: i32, rgb: [u8; 3], alpha: u8) {
    if alpha < 8 {
        return;
    }
    let under = im.get(x, y).unwrap_or(rgb);
    let a = u32::from(alpha);
    im.set(
        x,
        y,
        std::array::from_fn(|i| {
            ((u32::from(rgb[i]) * a + u32::from(under[i]) * (255 - a) + 127) / 255) as u8
        }),
    );
}

fn paint(
    im: &mut Img,
    id: &str,
    frame: usize,
    x: i32,
    y: i32,
    size: i32,
    flip: bool,
    tint: Option<([u8; 3], u8)>,
) -> bool {
    if size <= 0 {
        return false;
    }
    let Some(s) = VILLAGE_SPRITES
        .iter()
        .filter(|s| s.id == id && s.frame == frame)
        .min_by_key(|s| (i64::from(s.size) - i64::from(size)).abs())
    else {
        return false;
    };
    let scale = size as f64 / f64::from(s.size);
    let w = (f64::from(s.width) * scale).round() as i32;
    let h = (f64::from(s.height) * scale).round() as i32;
    let ax = (f64::from(s.anchor[0]) * scale).round() as i32;
    let ay = (f64::from(s.anchor[1]) * scale).round() as i32;
    for py in 0..h {
        for px in 0..w {
            let sx = (px as u32 * s.width / w as u32).min(s.width - 1);
            let sy = (py as u32 * s.height / h as u32).min(s.height - 1);
            let i = ((sy * s.width + sx) * 4) as usize;
            let mut rgb = [s.pixels[i], s.pixels[i + 1], s.pixels[i + 2]];
            let mut alpha = s.pixels[i + 3];
            if let Some((colour, opacity)) = tint {
                for c in 0..3 {
                    rgb[c] = ((u16::from(rgb[c]) + u16::from(colour[c]) * 2) / 3) as u8;
                }
                alpha = (u16::from(alpha) * u16::from(opacity) / 255) as u8;
            }
            blend(
                im,
                if flip { x + ax - px - 1 } else { x - ax + px },
                y - ay + py,
                rgb,
                alpha,
            );
        }
    }
    true
}

/// Static structure housing, with separately animated moving parts when active.
pub(super) fn paint_module(
    im: &mut Img,
    id: &str,
    x: i32,
    y: i32,
    size: i32,
    tick: u64,
    active: bool,
) -> bool {
    let (asset, frame) = match id {
        "brick-cottage" => (id, 3),
        "brick-wall" => ("brick-wall-ew", 0),
        _ => (id, 0),
    };
    if !paint(im, asset, frame, x, y, size, false, None) {
        return false;
    }
    {
        // These overlays share source-space anchors with their static housing.
        // A stopped machine keeps its rotor; an unlit kiln loses only its fire.
        let moving = match id {
            "brick-kiln" if active => Some("kiln-fire"),
            "waterwheel" => Some("wheel-rotor"),
            "drive-shaft" => Some("shaft-rotor"),
            "brickworks" => Some("brick-press"),
            _ => None,
        };
        if let Some(part) = moving {
            paint(
                im,
                part,
                if active {
                    frame_at(part, "work", tick)
                } else {
                    0
                },
                x,
                y,
                size,
                false,
                None,
            );
        }
    }
    true
}

/// Four authored cottage, wall and gate stages. Returns false when a plan has no authored
/// construction sequence, allowing the caller's normal site marker to remain.
pub(super) fn paint_construction(
    im: &mut Img,
    building: &str,
    stage: u8,
    x: i32,
    y: i32,
    size: i32,
    _tick: u64,
) -> bool {
    let building = match building {
        "brick-cottage" => building,
        "brick-wall" => "brick-wall-stages",
        "brick-gate" => "brick-gate-stages",
        _ => return false,
    };
    paint(
        im,
        building,
        usize::from(stage.min(3)),
        x,
        y,
        size,
        false,
        None,
    )
}

pub(super) fn paint_blueprint(
    im: &mut Img,
    id: &str,
    x: i32,
    y: i32,
    size: i32,
    valid: bool,
) -> bool {
    let (asset, frame) = match id {
        "brick-cottage" => (id, 3),
        "brick-wall" => ("brick-wall-ew", 0),
        _ => (id, 0),
    };
    paint(
        im,
        asset,
        frame,
        x,
        y,
        size,
        false,
        Some((
            if valid {
                [102, 232, 208]
            } else {
                [247, 113, 87]
            },
            135,
        )),
    )
}

/// N=1, E=2, S=4, W=8. Corners are authored for screen-axis orthogonal roads.
pub(super) fn paint_wall(
    im: &mut Img,
    connections: u8,
    x: i32,
    y: i32,
    size: i32,
    stage: u8,
) -> bool {
    if stage < 3 {
        return paint_construction(im, "brick-wall", stage, x, y, size, 0);
    }
    let id = match connections & 15 {
        0 => return paint(im, "brick-wall-post", 0, x, y, size * 3 / 5, false, None),
        1 | 4 | 5 => "brick-wall-ns",
        2 | 8 | 10 => "brick-wall-ew",
        3 => "brick-wall-ne",
        9 => "brick-wall-nw",
        6 => "brick-wall-se",
        12 => "brick-wall-sw",
        7 => "brick-wall-nes",
        11 => "brick-wall-new",
        13 => "brick-wall-nsw",
        14 => "brick-wall-esw",
        _ => {
            paint(im, "brick-wall-nes", 0, x, y, size, false, None);
            "brick-wall-nsw"
        }
    };
    paint(im, id, 0, x, y, size, false, None)
}

pub(super) fn paint_gate(im: &mut Img, x: i32, y: i32, size: i32, open: bool) -> bool {
    paint(im, "brick-gate", usize::from(open), x, y, size, false, None)
}

pub(super) fn paint_bridge(im: &mut Img, horizontal: bool, x: i32, y: i32, size: i32) -> bool {
    paint(
        im,
        if horizontal { "bridge-ew" } else { "bridge-ns" },
        0,
        x,
        y,
        size,
        false,
        None,
    )
}

/// `tick` uses the simulation's 30Hz clock; walk and work use manifest timing.
/// All eight frames use one scale, so carried tools never change body size.
pub(super) fn paint_citizen(
    im: &mut Img,
    role: &str,
    x: i32,
    y: i32,
    body_height: i32,
    tick: u64,
    working: bool,
    moving: bool,
    facing_left: bool,
) -> bool {
    let frame = if moving {
        frame_at(role, "walk", tick)
    } else if working {
        frame_at(role, "work", tick)
    } else {
        0
    };
    paint(im, role, frame, x, y, body_height, facing_left, None)
}
