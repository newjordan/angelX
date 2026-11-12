//! Generated frontier architecture, material terrain and live workshop effects.
//!
//! All positions are in the caller's canvas. A sprite's `(x, y)` is its
//! bottom-centre ground contact; tile drawing uses a top-left corner. Art may
//! overhang a tile: draw structures in ground-contact Y order.
use super::super::ink::Img;

include!(concat!(env!("OUT_DIR"), "/frontier_art_registry.rs"));

pub(super) fn building_size(plan: &str) -> i32 {
    match plan {
        "keep" | "gate" | "gatehouse" => 64,
        "camp" | "enchanter" | "mage-tower" => 56,
        "wall" => 40,
        _ => 48,
    }
}

fn asset(id: &str, size: u32) -> Option<&'static [u8]> {
    SPRITES
        .iter()
        .find(|(name, s, _)| *name == id && *s == size)
        .map(|(_, _, p)| *p)
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

/// Paint a source sprite with alpha-correct edges. Returns false for unknown
/// IDs so a caller can show a missing-module indicator rather than wrong art.
pub(super) fn paint_prop(im: &mut Img, id: &str, x: i32, y: i32, size: i32) -> bool {
    paint_sprite(im, id, x, y, size, false)
}

fn paint_sprite(im: &mut Img, id: &str, x: i32, y: i32, size: i32, flip: bool) -> bool {
    if size <= 0 {
        return false;
    }
    let native = if size <= 48 { 48 } else { 64 };
    let Some(pixels) = asset(id, native) else {
        return false;
    };
    let n = native as i32;
    for sy in 0..size {
        for sx in 0..size {
            let source_x = if flip { size - 1 - sx } else { sx };
            let i = ((sy * n / size) * n + source_x * n / size) as usize * 4;
            blend(
                im,
                x - size / 2 + sx,
                y - size + sy,
                [pixels[i], pixels[i + 1], pixels[i + 2]],
                pixels[i + 3],
            );
        }
    }
    true
}

/// The proposed structure at its real map scale, before materials are spent.
/// The translucent tint preserves the plan's silhouette and the road beneath.
pub(super) fn paint_blueprint(
    im: &mut Img,
    plan: &str,
    x: i32,
    y: i32,
    size: i32,
    valid: bool,
) -> bool {
    if size <= 0 {
        return false;
    }
    let id = match plan {
        "mage-tower" => "enchanter",
        "gatehouse" => "gate",
        "caravanserai" => "depot",
        other => other,
    };
    let native = if size <= 48 { 48 } else { 64 };
    let Some(pixels) = asset(id, native) else {
        return false;
    };
    let n = native as i32;
    let tint: [u16; 3] = if valid {
        [84, 239, 206]
    } else {
        [246, 104, 87]
    };
    for sy in 0..size {
        for sx in 0..size {
            let i = ((sy * n / size) * n + sx * n / size) as usize * 4;
            let rgb = std::array::from_fn(|c| ((u16::from(pixels[i + c]) * 2 + tint[c]) / 3) as u8);
            let opacity = if (sx + sy) % 4 == 0 { 145 } else { 92 };
            let a = (u16::from(pixels[i + 3]) * opacity / 255) as u8;
            blend(im, x - size / 2 + sx, y - size + sy, rgb, a);
        }
    }
    true
}

/// A moving delivery is a worker and physical load. The narrow coloured
/// cargo strap identifies its resource without replacing the wooden crate.
pub(super) fn paint_carrier(
    im: &mut Img,
    x: i32,
    y: i32,
    size: i32,
    facing_left: bool,
    cargo_colour: u32,
    tick: u64,
    with_cart: bool,
) {
    let frame = tick / 7 % 2;
    let id = match (with_cart, frame) {
        (true, 0) => "hauler-0",
        (true, _) => "hauler-1",
        (false, 0) => "porter-0",
        (false, _) => "porter-1",
    };
    let bob = if tick / 4 % 4 == 1 { 1 } else { 0 };
    paint_sprite(im, id, x, y - bob, size, facing_left);
    let marker_x = x + if facing_left { -size / 6 } else { size / 6 };
    let marker_y = y - size * if with_cart { 42 } else { 57 } / 100 - bob;
    let colour = [
        (cargo_colour >> 16) as u8,
        (cargo_colour >> 8) as u8,
        cargo_colour as u8,
    ];
    for dy in 0..3 {
        for dx in -1..=1 {
            blend(im, marker_x + dx, marker_y + dy, colour, 195);
        }
    }
}

/// Roof and yard sprites remain readable at 48px; work is communicated through
/// moving smoke, forge sparks and a pulsing arcane coil rather than a tiny icon.
pub(super) fn paint_building(
    im: &mut Img,
    plan: &str,
    x: i32,
    y: i32,
    size: i32,
    tick: u64,
    working: bool,
) -> bool {
    let id = match plan {
        "mage-tower" => "enchanter",
        "gatehouse" => "gate",
        "caravanserai" => "depot",
        other => other,
    };
    if !paint_prop(im, id, x, y, size) {
        return false;
    }
    if !working {
        return true;
    }
    let left = x - size / 2;
    let top = y - size;
    let chimney = match id {
        "cottage" => Some((63, 7)),
        "kiln" => Some((51, 11)),
        "smelter" => Some((44, 5)),
        "forge" => Some((69, 8)),
        "kitchen" => Some((72, 8)),
        _ => None,
    };
    if let Some((cx, cy)) = chimney {
        let origin = (left + size * cx / 100, top + size * cy / 100);
        for p in 0..4_u64 {
            let age = ((tick / 3 + p * 9) % 36) as i32;
            let px = origin.0 + age / 8 + ((age / 5 + p as i32) % 3 - 1);
            let py = origin.1 - age * size / 100;
            let radius = 1 + age / 12;
            for dy in -radius..=radius {
                for dx in -radius..=radius {
                    if dx * dx + dy * dy <= radius * radius + 1 {
                        blend(
                            im,
                            px + dx,
                            py + dy,
                            [174, 171, 151],
                            (94 - age * 2).max(12) as u8,
                        );
                    }
                }
            }
        }
    }
    if matches!(id, "kiln" | "smelter" | "forge" | "kitchen") {
        let pulse = 24 + ((tick / 3) % 7) as i32 * 4;
        glow(
            im,
            x + if id == "forge" { size / 5 } else { 0 },
            y - size / 4,
            (size / 9).max(2),
            [255, 151, 41],
            pulse as u8,
        );
        for p in 0..3_u64 {
            let age = ((tick / 2 + p * 7) % 23) as i32;
            if age < 17 {
                blend(
                    im,
                    x + (p as i32 * 5 - 5) + age / 6,
                    y - size / 4 - age,
                    [255, 204, 94],
                    (220 - age * 10) as u8,
                );
            }
        }
    }
    if matches!(id, "conduit" | "enchanter") {
        let phase = ((tick / 4) % 20) as i32;
        let amplitude = if phase < 10 { phase } else { 20 - phase };
        let cy = y - size * if id == "conduit" { 54 } else { 27 } / 100;
        glow(
            im,
            x,
            cy,
            (size / 6).max(3),
            [77, 238, 218],
            20 + amplitude as u8 * 4,
        );
        for p in 0..3_u64 {
            let age = ((tick / 3 + p * 11) % 32) as i32;
            let xx = x + (p as i32 * 11 - 11) * size / 48;
            let yy = cy + size / 4 - age * size / 64;
            blend(im, xx, yy, [160, 255, 235], (220 - age * 5) as u8);
            if age % 7 == 0 {
                blend(im, xx + 1, yy, [160, 255, 235], 100);
            }
        }
    }
    if matches!(id, "sawmill" | "waterwheel") {
        // Water flashes at the wheel's lower buckets and the cutting bed.
        let phase = (tick / 4 % 8) as i32;
        for p in 0..3 {
            blend(
                im,
                x + size / 3 + p - 1,
                y - size / 7 + (phase + p * 3) % 5,
                [148, 216, 221],
                150,
            );
        }
    }
    true
}

fn glow(im: &mut Img, x: i32, y: i32, radius: i32, rgb: [u8; 3], alpha: u8) {
    let r2 = radius * radius;
    for dy in -radius..=radius {
        for dx in -radius..=radius {
            let d = dx * dx + dy * dy;
            if d < r2 {
                blend(
                    im,
                    x + dx,
                    y + dy,
                    rgb,
                    (i32::from(alpha) * (r2 - d) / r2) as u8,
                );
            }
        }
    }
}

fn mirrored(v: i32, size: i32) -> i32 {
    let p = v.rem_euclid(size * 2);
    if p < size { p } else { size * 2 - 1 - p }
}

/// World-anchored material texture. Mirrored sampling joins opposite borders
/// without seams and keeps every tile from repeating the same grass clump.
pub(super) fn paint_ground_tile(
    im: &mut Img,
    kind: &str,
    x: i32,
    y: i32,
    size: i32,
    world_x: i32,
    world_y: i32,
    tick: u64,
) {
    let name = match kind {
        "forest" => "forest-floor",
        "stone" => "stone-ground",
        "water" => "water",
        "ground-clay" | "ground-dry-clay" | "ground-ford" | "ground-mud" => kind,
        _ => "grass",
    };
    let Some(pixels) = asset(name, 128) else {
        return;
    };
    for yy in 0..size {
        for xx in 0..size {
            let drift = if name == "water" {
                (tick / 12 % 256) as i32
            } else {
                0
            };
            let sx = mirrored(world_x + xx + drift, 128);
            let sy = mirrored(world_y + yy, 128);
            let i = (sy * 128 + sx) as usize * 4;
            // Muted earth gives bright working machines and roads prominence.
            let dim = if name == "water" { 235 } else { 205 };
            im.set(
                x + xx,
                y + yy,
                std::array::from_fn(|c| (u16::from(pixels[i + c]) * dim / 255) as u8),
            );
        }
    }
}

fn road_contains(x: i32, y: i32, size: i32, links: u8) -> bool {
    let lo = size * 24 / 100;
    let hi = size - lo;
    (x >= lo && x < hi && y >= lo && y < hi)
        || (links & 1 != 0 && x >= lo && x < hi && y < hi)
        || (links & 2 != 0 && y >= lo && y < hi && x >= lo)
        || (links & 4 != 0 && x >= lo && x < hi && y >= lo)
        || (links & 8 != 0 && y >= lo && y < hi && x < hi)
}

/// Connected cobbled lanes: link bits are north=1, east=2, south=4, west=8.
/// Contours reveal missing road connections; no isolated square pavement tiles.
pub(super) fn paint_road(
    im: &mut Img,
    x: i32,
    y: i32,
    size: i32,
    links: u8,
    world_x: i32,
    world_y: i32,
) {
    for yy in 0..size {
        for xx in 0..size {
            if !road_contains(xx, yy, size, links) {
                continue;
            }
            let edge = [(-1, 0), (1, 0), (0, -1), (0, 1)]
                .iter()
                .any(|&(dx, dy)| !road_contains(xx + dx, yy + dy, size, links));
            let wx = world_x + xx;
            let wy = world_y + yy;
            let row = wy.div_euclid(5);
            let mortar = wy.rem_euclid(5) == 0 || (wx + row.rem_euclid(2) * 4).rem_euclid(8) == 0;
            let n = ((wx.div_euclid(8) * 7 + row * 11).rem_euclid(13)) as u8;
            let colour = if edge {
                [88, 80, 58]
            } else if mortar {
                [111, 104, 81]
            } else {
                [151 + n, 140 + n, 110 + n]
            };
            im.set(x + xx, y + yy, colour);
        }
    }
}

/// Construction/selection feedback stays visible around overhanging roofs.
pub(super) fn paint_cursor(
    im: &mut Img,
    x: i32,
    y: i32,
    width: i32,
    height: i32,
    valid: bool,
    tick: u64,
) {
    let colour = if valid {
        [116, 235, 196]
    } else {
        [244, 111, 91]
    };
    let alpha = 150 + (tick / 5 % 6) as u8 * 15;
    for (cx, cy, sx, sy) in [
        (x, y, 1, 1),
        (x + width - 1, y, -1, 1),
        (x, y + height - 1, 1, -1),
        (x + width - 1, y + height - 1, -1, -1),
    ] {
        for n in 0..7 {
            blend(im, cx + n * sx, cy, colour, alpha);
            blend(im, cx, cy + n * sy, colour, alpha);
        }
    }
}
