//! Mossbell's bathhouse conservatory: table, keeper, contact art and seat bodies.
use super::super::{
    chrome,
    ink::{Img, text_width},
};
use super::{NATIVE_H, NATIVE_W};
use crate::rules::{
    HZ, Run,
    mossbell::{BUMPERS, Beat, GOLD_PER_SEAT, Q},
};
use std::sync::OnceLock;
const OX: i32 = 64;
const OY: i32 = 46;
// Presentation only: physics remains on the original 160px table.
fn field_x(x: i32) -> i32 {
    x * 8 / 5
}

fn hud() -> &'static serde_json::Value {
    static HUD: OnceLock<serde_json::Value> = OnceLock::new();
    HUD.get_or_init(|| {
        serde_json::from_str(include_str!(
            "../../../../assets/art-slots/mossbell.chrome/hud.json"
        ))
        .unwrap()
    })
}
fn compact_hud() -> bool {
    static COMPACT: OnceLock<bool> = OnceLock::new();
    *COMPACT.get_or_init(|| {
        std::env::var("ANGEL_MINIGAME_HUD").is_ok_and(|v| v == "compact")
            && hud()["compact"]["compact"] == true
    })
}
fn strip_x(n: usize) -> i32 {
    hud()["strip_x"].as_i64().unwrap() as i32
        + n as i32 * hud()["seat_step"].as_i64().unwrap() as i32
}
fn strip_y() -> i32 {
    hud()["strip_y"].as_i64().unwrap() as i32
}
// These conversions run only while populating the native art caches.
fn slot(slot: &str, name: &str) -> Option<Img> {
    if !super::slot_art::enabled() {
        return None;
    }
    let data = super::slot_art::bytes(slot, name)?;
    let mut im = decode(data);
    for y in 0..im.h {
        for x in 0..im.w {
            if let Some(c) = im.get(x, y) {
                if c.iter().copied().max().unwrap() > 224 {
                    im.set(x, y, nearest(c, 224));
                }
            }
        }
    }
    Some(im)
}
fn nearest(c: [u8; 3], ceiling: u8) -> [u8; 3] {
    super::super::ink::PALETTE
        .iter()
        .copied()
        .map(super::super::ink::rgb)
        .filter(|v| v.iter().all(|&n| n <= ceiling))
        .min_by_key(|v| {
            v.iter()
                .zip(c)
                .map(|(&a, b)| (i32::from(a) - i32::from(b)).pow(2))
                .sum::<i32>()
        })
        .unwrap()
}
fn dim(mut im: Img) -> Img {
    for y in 0..im.h {
        for x in 0..im.w {
            if let Some(c) = im.get(x, y) {
                im.set(
                    x,
                    y,
                    nearest(c.map(|v| (u16::from(v) * 150 / 256) as u8), 160),
                );
            }
        }
    }
    im
}
fn skin(source: Option<Img>, w: i32, h: i32) -> Option<Img> {
    let source = source?;
    let mut x0 = source.w;
    let mut y0 = source.h;
    let mut x1 = 0;
    let mut y1 = 0;
    for y in 0..source.h {
        for x in 0..source.w {
            if source.get(x, y).is_some() {
                x0 = x0.min(x);
                y0 = y0.min(y);
                x1 = x1.max(x + 1);
                y1 = y1.max(y + 1);
            }
        }
    }
    if x1 <= x0 || y1 <= y0 {
        return None;
    }
    let source_edge = ((x1 - x0).min(y1 - y0) / 4).max(1);
    let axis = |at: i32, size: i32, len: i32| {
        let edge = 2.min(size / 3);
        if at < edge {
            at * source_edge / edge
        } else if at >= size - edge {
            len - source_edge + (at - size + edge) * source_edge / edge
        } else {
            source_edge + (at - edge) * (len - 2 * source_edge) / (size - 2 * edge)
        }
    };
    let mut out = Img::new(w, h);
    for y in 0..h {
        for x in 0..w {
            if let Some(c) = source.get(x0 + axis(x, w, x1 - x0), y0 + axis(y, h, y1 - y0)) {
                out.set(x, y, c);
            }
        }
    }
    // Empty fields stay flat so source wear never competes with live labels.
    out.rect(2, 2, w - 4, h - 4, 'S');
    Some(out)
}
// Cell banks are baked once; the live path only stamps integer pixels.
fn cell_sprite(source: &Img, palette: &[[u8; 3]], rim: [u8; 3]) -> Img {
    super::slot_art::cell_sprite(source, palette, rim)
}
fn cell_backdrop(source: &Img) -> Img {
    let mut out = source.clone();
    for y in 0..out.h {
        for x in 0..out.w {
            if let Some(c) = source.get(x, y) {
                out.set(x, y, c.map(|v| (u16::from(v) * 3 / 10) as u8));
            }
        }
    }
    out
}
fn clean(source: &Img, data: &serde_json::Value) -> Img {
    let mut erased = vec![false; (source.w * source.h) as usize];
    if let Some(runs) = data["erase_runs"].as_array() {
        for run in runs {
            let y = run[0].as_i64().unwrap() as i32;
            let x = run[1].as_i64().unwrap() as i32;
            let w = run[2].as_i64().unwrap() as i32;
            for x in x..x + w {
                erased[(y * source.w + x) as usize] = true;
            }
        }
    }
    let mut out = Img::new(source.w, source.h);
    for y in 0..source.h {
        for x in 0..source.w {
            if !erased[(y * source.w + x) as usize] {
                if let Some(c) = source.get(x, y) {
                    out.set(x, y, c);
                }
            }
        }
    }
    out
}
fn seat_material(source: &Img, data: &serde_json::Value, seat: char) -> Img {
    let ramp = match seat {
        'r' | '7' => [[54, 15, 24], [130, 28, 42], [195, 47, 57], [218, 105, 99]],
        'S' | 'q' | '1' => [[16, 32, 65], [27, 66, 142], [45, 109, 192], [103, 157, 209]],
        'e' | 'F' | 'l' | 'L' | 'A' => [[14, 40, 31], [21, 90, 52], [45, 143, 69], [110, 183, 106]],
        _ => [[62, 37, 13], [133, 85, 18], [193, 145, 35], [215, 181, 91]],
    };
    let mut out = source.clone();
    if let Some(runs) = data["cloth_runs"].as_array() {
        for run in runs {
            let y = run[0].as_i64().unwrap() as i32;
            let x = run[1].as_i64().unwrap() as i32;
            let w = run[2].as_i64().unwrap() as i32;
            for x in x..x + w {
                if let Some(c) = source.get(x, y) {
                    let shade = (c.into_iter().max().unwrap().saturating_sub(45) / 45).min(3);
                    out.set(x, y, ramp[shade as usize]);
                }
            }
        }
    }
    out
}
struct Art {
    collapse: Option<[Img; 2]>,
    poses: [[Img; 9]; 3],
    cells: [[Img; 9]; 3],
    effects: [[Img; 4]; 3],
    bumpers: [Img; 3],
    flashes: [[Img; 4]; 3],
    ball: Img,
    trail: Img,
    pops: [Img; 3],
    roots: [[Img; 4]; 2],
    crown_floor: Img,
    crown: [Img; 4],
    crown_cells: [Img; 4],
    steam: Option<Img>,
    flippers: [Option<Img>; 4],
}
fn decode(bytes: &[u8]) -> Img {
    let png = image::load_from_memory(bytes)
        .expect("Mossbell art")
        .to_rgba8();
    let mut im = Img::new(png.width() as i32, png.height() as i32);
    for (x, y, p) in png.enumerate_pixels() {
        if p[3] > 0 {
            im.set(x as i32, y as i32, [p[0], p[1], p[2]]);
        }
    }
    im
}
fn composed(bytes: &[u8], w: i32, h: i32) -> Option<Img> {
    if !super::slot_art::enabled() {
        return None;
    }
    let im = decode(bytes);
    ((im.w, im.h) == (w, h)).then_some(im)
}
fn keeper_grade(im: Img, pose: usize) -> Img {
    let (cx, cy) = [
        (25, 23),
        (27, 33),
        (26, 27),
        (27, 34),
        (24, 24),
        (22, 23),
        (25, 27),
        (25, 28),
        (27, 34),
    ][pose];
    keeper_grade_at(im, (cx, cy))
}
fn damaged_keeper(name: &str) -> Option<Img> {
    let eye = match name {
        "stage-1-tell-a" => (29, 28),
        "stage-1-tell-b" => (29, 30),
        "stage-1-attack-b" => (30, 24),
        "stage-1-recover-b" => (33, 25),
        "stage-2-idle" => (29, 27),
        "stage-2-tell-a" => (30, 30),
        "strong-flinch" => (24, 23),
        "defeat-1" => (28, 34),
        "defeat-2" => (28, 40),
        _ => return None,
    };
    slot("mossbell.keeper-damaged", name).map(|im| keeper_grade_at(im, eye))
}
fn keeper_grade_at(mut im: Img, (cx, cy): (i32, i32)) -> Img {
    let ramp = [
        [19, 39, 36],
        [24, 36, 20],
        [38, 59, 28],
        [53, 84, 63],
        [58, 109, 93],
        [111, 164, 109],
    ];
    for y in 0..im.h {
        for x in 0..im.w {
            if let Some([r, g, b]) = im.get(x, y) {
                if r > g.saturating_add(12)
                    && g > b.saturating_add(8)
                    && !((x - cx).abs() <= 4 && (y - cy).abs() <= 2)
                {
                    im.set(x, y, ramp[(usize::from(r) * 6 / 225).min(5)]);
                }
            }
        }
    }
    im
}
fn art() -> &'static Art {
    static ART: OnceLock<Art> = OnceLock::new();
    static ORIGINAL: OnceLock<Art> = OnceLock::new();
    let cache = if super::slot_art::enabled() {
        &ART
    } else {
        &ORIGINAL
    };
    cache.get_or_init(|| {
        macro_rules! s {
            ($n:literal) => {
                decode(include_bytes!(concat!(
                    "../../../../assets/mossbell/",
                    $n,
                    ".png"
                )))
            };
        }
        macro_rules! c {
            ($n:literal, $w:expr, $h:expr) => {
                composed(
                    include_bytes!(concat!(
                        "../../../../assets/mossbell/composed/current-",
                        $n,
                        ".png"
                    )),
                    $w,
                    $h,
                )
            };
        }
        let source = [
            s!("idle"),
            s!("root-tell"),
            s!("root-attack"),
            s!("crown-tell"),
            s!("crown-attack"),
            s!("flinch"),
            s!("damage-1"),
            s!("damage-2"),
            s!("defeat"),
        ];
        let metadata: serde_json::Value =
            serde_json::from_str(include_str!("../../../../assets/mossbell/native.json"))
                .expect("Mossbell native recipe");
        let names = [
            "idle",
            "root-tell",
            "root-attack",
            "crown-tell",
            "crown-attack",
            "flinch",
            "damage-1",
            "damage-2",
            "defeat",
        ];
        let poses: [[Img; 9]; 3] = std::array::from_fn(|stage| {
            std::array::from_fn(|pose| {
                let mut im = source[pose].clone();
                if stage > 0 && pose < 6 {
                    let mut seam = Img::new(48, 48);
                    for (a, b, c) in [
                        ((30, 27), (28, 34), (30, 43)),
                        ((18, 27), (20, 34), (17, 43)),
                    ]
                    .into_iter()
                    .take(stage)
                    {
                        for dx in -3..=3 {
                            seam.line(a.0 + dx, a.1, b.0 + dx, b.1, 's');
                            seam.line(b.0 + dx, b.1, c.0 + dx, c.1, 's');
                        }
                        for dx in 0..2 {
                            seam.line(a.0 + dx, a.1, b.0 + dx, b.1, 't');
                            seam.line(b.0 + dx, b.1, c.0 + dx, c.1, 't');
                        }
                    }
                    for y in 0..48 {
                        for x in 0..48 {
                            if source[pose].get(x, y).is_some() {
                                if let Some(c) = seam.get(x, y) {
                                    im.set(x, y, c);
                                }
                            }
                        }
                    }
                    for light in metadata["states"][names[pose]]["signature_lights"]
                        .as_array()
                        .unwrap()
                    {
                        for layer in ["pixels", "falloff"] {
                            for p in light[layer].as_array().unwrap() {
                                let x = p[0].as_i64().unwrap() as i32;
                                let y = p[1].as_i64().unwrap() as i32;
                                if let Some(c) = source[pose].get(x, y) {
                                    im.set(x, y, c);
                                }
                            }
                        }
                    }
                }
                im
            })
        });
        // Signature lights own their half-cell, just as the combat guardians do.
        // This conversion is baked once; native RGB keeps the authored 3x2 curves.
        let cells: [[Img; 9]; 3] = std::array::from_fn(|stage| {
            std::array::from_fn(|pose| {
                let mut im = poses[stage][pose].clone();
                let damage = if pose == 6 {
                    1
                } else if pose == 7 {
                    2
                } else if pose < 6 {
                    stage
                } else {
                    0
                };
                if damage > 0 {
                    for (branch, cx) in [30, 18].into_iter().take(damage).enumerate() {
                        for y in 28..44 {
                            let bend = if (y - 28) / 4 % 2 == 0 {
                                0
                            } else {
                                if branch == 0 { -4 } else { 4 }
                            };
                            for x in cx - 4 + bend..cx + 8 + bend {
                                if im.get(x, y).is_some() {
                                    im.put(x, y, 's');
                                }
                            }
                            for x in cx + bend..cx + 4 + bend {
                                if im.get(x, y).is_some() {
                                    im.put(x, y, 't');
                                }
                            }
                        }
                    }
                    if damage == 1 {
                        for y in 30..38 {
                            let bend = if y < 34 { 0 } else { 4 };
                            for x in 13 + bend..25 + bend {
                                if im.get(x, y).is_some() {
                                    im.put(x, y, 's');
                                }
                            }
                            for x in 17 + bend..21 + bend {
                                if im.get(x, y).is_some() {
                                    im.put(x, y, 't');
                                }
                            }
                        }
                    }
                }
                for light in metadata["states"][names[pose]]["signature_lights"]
                    .as_array()
                    .unwrap()
                {
                    let pixels = light["pixels"].as_array().unwrap();
                    // Each lobe is four connected source pixels; the seed is one form.
                    let groups = if light["name"] == "curved visor" {
                        2
                    } else {
                        1
                    };
                    for group in 0..groups {
                        let part = if groups == 2 {
                            &pixels[group * 4..group * 4 + 4]
                        } else {
                            &pixels[..]
                        };
                        let sx = part
                            .iter()
                            .map(|p| p[0].as_i64().unwrap() as i32)
                            .sum::<i32>()
                            / part.len() as i32;
                        let sy = part
                            .iter()
                            .map(|p| p[1].as_i64().unwrap() as i32)
                            .sum::<i32>()
                            / part.len() as i32;
                        // The cabinet's fixed native origin is (190,43). Half-cells are 4x4.
                        let x0 = (190 + sx) / 4 * 4 - 190;
                        let y0 = (43 + sy) / 4 * 4 - 43;
                        for y in y0..y0 + 4 {
                            for x in x0..x0 + 4 {
                                if im.get(x, y).is_some() {
                                    im.put(x, y, 't');
                                }
                            }
                        }
                    }
                }
                im
            })
        });
        let poses: [[Img; 9]; 3] = std::array::from_fn(|stage| {
            std::array::from_fn(|pose| {
                let extra = match (stage, pose) {
                    (1, 1) => Some("stage-1-tell-a"),
                    (1, 3) => Some("stage-1-tell-b"),
                    (1, 4) => Some("stage-1-attack-b"),
                    (1, 6) => Some("stage-1-recover-b"),
                    (2, 7) => Some("stage-2-idle"),
                    (2, 1 | 3) => Some("stage-2-tell-a"),
                    (1 | 2, 5) => Some("strong-flinch"),
                    (_, 8) => Some("defeat-2"),
                    _ => None,
                };
                extra
                    .and_then(damaged_keeper)
                    .or_else(|| {
                        slot("mossbell.keeper", names[pose]).map(|im| keeper_grade(im, pose))
                    })
                    .unwrap_or_else(|| poses[stage][pose].clone())
            })
        });
        let cells: [[Img; 9]; 3] = std::array::from_fn(|stage| {
            std::array::from_fn(|pose| {
                if super::slot_art::enabled() {
                    cell_sprite(
                        &poses[stage][pose],
                        &[
                            [19, 39, 36],
                            [38, 59, 28],
                            [58, 109, 93],
                            [111, 164, 109],
                            [190, 145, 75],
                        ],
                        [47, 103, 111],
                    )
                } else {
                    cells[stage][pose].clone()
                }
            })
        });
        let bumpers: [Img; 3] = std::array::from_fn(|i| {
            let mut im = Img::new(28, 28);
            match i {
                0 => {
                    im.ellipse(14., 14., 10., 10., 's');
                    im.ellipse(14., 13., 8., 8., 'o');
                    im.ellipse(14., 12., 5., 5., 't');
                    im.ellipse(14., 12., 3., 3., 'S');
                }
                1 => {
                    for y in 0i32..21 {
                        let w = 10 - (10 - y).abs();
                        im.rect(14 - w, 4 + y, w * 2 + 1, 1, 'S');
                        if w > 2 {
                            im.rect(16 - w, 4 + y, w * 2 - 3, 1, 'U');
                        }
                    }
                    im.line(14, 8, 19, 14, 't');
                    im.line(19, 14, 14, 19, 't');
                }
                _ => {
                    // Three plump seed petals with a full cell of air at the clefts.
                    im.line(9, 12, 14, 18, 's');
                    im.line(19, 12, 14, 18, 's');
                    for (dx, dy, ink) in [(0, 0, 'S'), (12, 0, 'o'), (6, 13, 'o')] {
                        for (a, b) in [
                            ((6, 3), (9, 3)),
                            ((9, 3), (11, 5)),
                            ((11, 5), (11, 10)),
                            ((11, 10), (9, 12)),
                            ((9, 12), (6, 12)),
                            ((6, 12), (4, 10)),
                            ((4, 10), (4, 5)),
                            ((4, 5), (6, 3)),
                        ] {
                            im.line(a.0 + dx, a.1 + dy, b.0 + dx, b.1 + dy, ink);
                        }
                        im.rect(5 + dx, 5 + dy, 6, 5, ink);
                        im.rect(6 + dx, 4 + dy, 4, 7, ink);
                        im.line(
                            6 + dx,
                            5 + dy,
                            9 + dx,
                            5 + dy,
                            if dx == 0 { 'U' } else { 't' },
                        );
                    }
                }
            }
            im
        });
        let bumpers: [Img; 3] = std::array::from_fn(|i| {
            slot("mossbell.targets", ["ring", "diamond", "petals"][i])
                .unwrap_or_else(|| bumpers[i].clone())
        });
        let flashes = std::array::from_fn(|shape| {
            std::array::from_fn(|frame| {
                let mut im = bumpers[shape].clone();
                let depth = if frame == 1 || frame == 2 { 2 } else { 1 };
                let ink = if frame < 2 { 't' } else { 'O' };
                // Colour the existing contour, preserving circle, diamond and petal gaps.
                for y in 0..28 {
                    for x in 0..28 {
                        if bumpers[shape].get(x, y).is_some()
                            && (1..=depth).any(|d| {
                                [(x - d, y), (x + d, y), (x, y - d), (x, y + d)]
                                    .into_iter()
                                    .any(|(nx, ny)| bumpers[shape].get(nx, ny).is_none())
                            })
                        {
                            im.put(x, y, ink);
                        }
                    }
                }
                im
            })
        });
        let mut ball = Img::new(9, 9);
        ball.ellipse(4., 4., 4., 4., 's');
        ball.ellipse(4., 3., 3., 3., 'o');
        ball.rect(3, 2, 3, 2, 'T');
        ball.set(2, 4, [190, 145, 75]);
        let mut trail = Img::new(5, 5);
        trail.ellipse(2., 2., 2., 2., 'I');
        trail.set(2, 2, [120, 70, 32]);
        if let Some(imported) = slot("mossbell.ball", "marble") {
            ball = imported;
            trail = slot("mossbell.trail", "trail").unwrap_or_else(|| {
                let mut echo = Img::new(5, 5);
                for y in 0..5 {
                    for x in 0..5 {
                        if let Some(c) = ball.get(x * 2, y * 2) {
                            echo.set(
                                x,
                                y,
                                nearest(c.map(|v| (u16::from(v) * 90 / 256) as u8), 96),
                            );
                        }
                    }
                }
                echo
            });
        }
        let pops = std::array::from_fn(|i| {
            let text = format!("CHAIN X{}", i + 1);
            let mut small = Img::new(text_width(&text), 8);
            small.text(0, 0, &text, 't');
            let mut im = Img::new(small.w * 2 + 8, 20);
            im.rect(0, 0, im.w, 20, 's');
            for y in 0..8 {
                for x in 0..small.w {
                    if let Some(c) = small.get(x, y) {
                        for dy in 0..2 {
                            for dx in 0..2 {
                                im.set(4 + x * 2 + dx, 2 + y * 2 + dy, c);
                            }
                        }
                    }
                }
            }
            im
        });
        let root_source = [
            s!("root-vines-0"),
            s!("root-vines-1"),
            s!("root-vines-2"),
            s!("root-vines-3"),
        ];
        let root_width = field_x(150) - field_x(7);
        let composed_roots = [
            c!("root-0", root_width, 11),
            c!("root-1", root_width, 11),
            c!("root-2", root_width, 11),
            c!("root-3", root_width, 11),
        ];
        let root_source = std::array::from_fn::<_, 4, _>(|i| {
            composed_roots[i]
                .clone()
                .unwrap_or_else(|| root_source[i].clone())
        });
        let roots = std::array::from_fn(|direction| {
            std::array::from_fn(|stage| {
                let source = &root_source[stage];
                // Mirror inside the band ending at the plunger wall, so
                // either leading bud can reach its own outer corner.
                let mut im = Img::new(field_x(150) - field_x(7), source.h);
                for y in 0..source.h {
                    for x in 0..im.w {
                        let sx = if direction == 0 { x } else { im.w - 1 - x };
                        let sx = sx * source.w / im.w;
                        if let Some(c) = source.get(sx, y) {
                            im.set(x, y, c);
                        }
                    }
                }
                im
            })
        });
        let crown_source = [
            s!("crown-vines-0"),
            s!("crown-vines-1"),
            s!("crown-vines-2"),
            s!("crown-vines-3"),
        ];
        let crown_width = field_x(100) - field_x(60) + 1;
        // The delivered dark leaf contour merges with the Keeper in real cells.
        // Retain the rule art for that path, including its exact growing gap.
        let crown_cells = std::array::from_fn(|i| {
            let source = &crown_source[i];
            let mut out = Img::new(crown_width, source.h);
            for y in 0..out.h {
                for x in 0..out.w {
                    if let Some(c) = source
                        .get(x * source.w / out.w, y)
                        .filter(|c| *c != [24, 43, 43])
                    {
                        out.set(x, y, c);
                    }
                }
            }
            out
        });
        let composed_crowns = [
            c!("crown-0", crown_width, 24),
            c!("crown-1", crown_width, 24),
            c!("crown-2", crown_width, 24),
            c!("crown-3", crown_width, 24),
        ];
        let crown_source = std::array::from_fn::<_, 4, _>(|i| {
            composed_crowns[i]
                .clone()
                .unwrap_or_else(|| crown_source[i].clone())
        });
        let crown_source = crown_source.map(|source| {
            let mut out = Img::new(field_x(100) - field_x(60) + 1, source.h);
            for y in 0..out.h {
                for x in 0..out.w {
                    if let Some(c) = source.get(x * source.w / out.w, y) {
                        out.set(x, y, c);
                    }
                }
            }
            out
        });
        let floor_ink = [24, 43, 43];
        let mut crown_floor = Img::new(crown_source[0].w, crown_source[0].h);
        for y in 0..crown_floor.h {
            for x in 0..crown_floor.w {
                crown_floor.set(x, y, floor_ink);
            }
        }
        let crown = crown_source.map(|source| {
            let mut im = Img::new(source.w, source.h);
            for y in 0..source.h {
                for x in 0..source.w {
                    if let Some(c) = source.get(x, y).filter(|c| *c != floor_ink) {
                        im.set(x, y, c);
                    }
                }
            }
            im
        });
        let collapse = damaged_keeper("defeat-1").map(|body| {
            let cells = cell_sprite(
                &body,
                &[
                    [19, 39, 36],
                    [38, 59, 28],
                    [58, 109, 93],
                    [111, 164, 109],
                    [190, 145, 75],
                ],
                [47, 103, 111],
            );
            [body, cells]
        });
        Art {
            collapse,
            poses,
            cells,
            effects: [
                [
                    slot("mossbell.bumper-burst", "bumper-0").unwrap_or_else(|| s!("bumper-0")),
                    slot("mossbell.bumper-burst", "bumper-1").unwrap_or_else(|| s!("bumper-1")),
                    slot("mossbell.bumper-burst", "bumper-2").unwrap_or_else(|| s!("bumper-2")),
                    slot("mossbell.bumper-burst", "bumper-3").unwrap_or_else(|| s!("bumper-3")),
                ],
                [
                    slot("mossbell.impact", "impact-0").unwrap_or_else(|| s!("impact-0")),
                    slot("mossbell.impact", "impact-1").unwrap_or_else(|| s!("impact-1")),
                    slot("mossbell.impact", "impact-2").unwrap_or_else(|| s!("impact-2")),
                    slot("mossbell.impact", "impact-3").unwrap_or_else(|| s!("impact-3")),
                ],
                [
                    slot("mossbell.jackpot", "jackpot-0").unwrap_or_else(|| s!("jackpot-0")),
                    slot("mossbell.jackpot", "jackpot-1").unwrap_or_else(|| s!("jackpot-1")),
                    slot("mossbell.jackpot", "jackpot-2").unwrap_or_else(|| s!("jackpot-2")),
                    slot("mossbell.jackpot", "jackpot-3").unwrap_or_else(|| s!("jackpot-3")),
                ],
            ],
            bumpers,
            flashes,
            ball,
            trail,
            pops,
            roots,
            crown_floor,
            crown,
            crown_cells,
            steam: slot("mossbell.steam", "steam"),
            flippers: [
                c!("flippers-0", field_x(123) - field_x(37) + 7, 26),
                c!("flippers-1", field_x(123) - field_x(37) + 7, 26),
                c!("flippers-2", field_x(123) - field_x(37) + 7, 26),
                c!("flippers-3", field_x(123) - field_x(37) + 7, 26),
            ],
        }
    })
}
fn room() -> &'static Img {
    static ROOM: OnceLock<Img> = OnceLock::new();
    static ORIGINAL: OnceLock<Img> = OnceLock::new();
    let cache = if super::slot_art::enabled() {
        &ROOM
    } else {
        &ORIGINAL
    };
    cache.get_or_init(|| {
        let mut im = Img::black(NATIVE_W, NATIVE_H);
        // Copper glazing and leafy silhouettes frame a quiet, enclosed bay.
        for row in 0..5 {
            for col in 0..10 {
                let x = 8 + col * 38 + (row % 2) * 8;
                let y = 31 + row * 27;
                im.rect(x, y, 34, 24, 'k');
                im.line(x + 2, y + 23, x + 33, y + 23, 'x');
            }
        }
        im.ellipse(192., 96., 164., 89., 's');
        im.ellipse(192., 96., 160., 85., 'k');
        for x in [344] {
            im.rect(x - 3, 59, 6, 100, 's');
            im.ellipse(x as f32, 158., 18., 7., 'I');
            im.rect(x - 14, 158, 28, 15, 'I');
            im.ellipse(x as f32, 156., 17., 5., 'r');
            for (dx, dy) in [(-13, -41), (12, -29), (-10, -13), (14, -53)] {
                im.line(x, 155, x + dx, 155 + dy, 'S');
                im.ellipse((x + dx) as f32, (150 + dy) as f32, 7., 4., 'S');
            }
        }
        if let Some(backdrop) = slot("mossbell.room", "room") {
            im = dim(backdrop);
        }
        chrome::panel(&mut im, (58, 39, 268, 154), chrome::Tone::BRASS);
        im.rect(OX + field_x(3), OY + 3, field_x(154), 135, 's');
        im.rect(OX + field_x(6), OY + 6, field_x(148), 131, 'k');
        // Dormant roots ground the lower field, well below the attack's amber buds.
        for branch in [
            [(20, 94), (39, 99), (54, 96), (69, 101)],
            [(140, 94), (121, 99), (106, 96), (91, 101)],
            [(46, 102), (61, 107), (80, 105), (99, 107)],
        ] {
            for pair in branch.windows(2) {
                for dy in 0..2 {
                    im.line(
                        OX + field_x(pair[0].0),
                        OY + pair[0].1 + dy,
                        OX + field_x(pair[1].0),
                        OY + pair[1].1 + dy,
                        'f',
                    );
                }
            }
        }
        // Launch rail and sloping returns are the same segments used by physics.
        im.line(OX + field_x(150), OY + 92, OX + field_x(150), OY + 134, 'S');
        im.line(
            OX + field_x(145),
            OY + 108,
            OX + field_x(145),
            OY + 128,
            'I',
        );
        for (x1, y1, x2, y2) in [(8, 98, 38, 123), (152, 98, 122, 123)] {
            for dy in 0..4 {
                im.line(
                    OX + field_x(x1),
                    OY + y1 + dy,
                    OX + field_x(x2),
                    OY + y2 + dy,
                    'o',
                );
            }
        }
        im.rect(OX + field_x(71), OY + 134, field_x(18), 3, 'b');
        for x in [80, 302] {
            im.rect(x, 194, 5, 7, 'I');
            im.rect(x + 1, 195, 2, 5, 'o');
        }
        chrome::panel(&mut im, (2, 47, 56, 111), chrome::Tone::SLATE);
        chrome::panel(&mut im, (328, 47, 56, 146), chrome::Tone::SLATE);
        chrome::panel(&mut im, (58, 3, 268, 36), chrome::Tone::SLATE);
        chrome::panel(&mut im, (88, 203, 140, 17), chrome::Tone::SLATE);
        if super::slot_art::enabled() {
            for (index, rect) in [
                (0, (58, 3, 268, 36)),
                (1, (2, 47, 56, 111)),
                (2, (328, 47, 56, 146)),
                (3, (88, 203, 140, 17)),
            ] {
                if let Some(plate) = &plates()[index] {
                    im.stamp(plate, rect.0, rect.1);
                }
            }
        }
        im
    })
}
fn focus(run: &Run, who: Option<u32>) -> u32 {
    let s = run.mossbell.as_ref().unwrap();
    who.filter(|id| s.tables.contains_key(id))
        .unwrap_or(*s.tables.keys().next().unwrap())
}
fn seat_ink(id: u32) -> char {
    ['r', 'S', 'e', 'o', 'z', 'Y'][(id.saturating_sub(1) % 6) as usize]
}
fn center(x: i32, y: i32, s: String) -> (i32, i32, String) {
    (x - text_width(&s) / 2, y, s)
}
fn name(run: &Run, who: u32, focus: u32) -> String {
    if who == focus {
        "YOU".into()
    } else {
        let full = run.players.get(&who).map_or("KNIGHT", |h| h.name.as_str());
        if full.starts_with("bot ") || full.starts_with("Seat ") {
            format!("K{who}")
        } else {
            full.chars().take(13).collect::<String>().to_uppercase()
        }
    }
}
/// The RGB and terminal routes use these same positions and words.
pub(super) fn labels(run: &Run, who: Option<u32>) -> Vec<(i32, i32, String)> {
    let s = run.mossbell.as_ref().unwrap();
    let id = focus(run, who);
    let b = &s.tables[&id].ball;
    if s.left == 0 {
        let mut v = vec![
            center(192, 30, "THE SEEDS SETTLE".into()),
            (238 - text_width("SCORE"), 47, "SCORE".into()),
            (276, 47, "DRAINS".into()),
        ];
        for (n, &who) in s.order.iter().enumerate() {
            let y = 66 + n as i32 * 24;
            let score = s.points(who).to_string();
            v.push((30, y, s.place(who).to_string()));
            v.push((48, y, name(run, who, id)));
            v.push((238 - text_width(&score), y, score));
            v.push((292, y, s.tables[&who].ball.drains.to_string()));
        }
        let leader = s.order[0];
        let tied = s
            .tables
            .keys()
            .filter(|&&r| s.score(r) == s.score(leader))
            .count() as u32;
        let pot = GOLD_PER_SEAT * s.tables.len() as u32;
        let payout = if tied == 1 {
            format!(
                "{} {} {pot}G + CROWN",
                name(run, leader, id),
                if leader == id { "WIN" } else { "WINS" }
            )
        } else if pot % tied == 0 {
            format!("{}G EACH + SHARED CROWN", pot / tied)
        } else {
            format!("{}G EACH, {} +1G", pot / tied, name(run, leader, id))
        };
        v.push(center(192, 174, payout));
        v.push(center(
            192,
            187,
            "FEWER DRAINS, THEN EARLIER JACKPOT".into(),
        ));
        v.push(center(192, 202, "E AGAIN".into()));
        return v;
    }
    let mut v = vec![
        center(
            192,
            25,
            if b.chain > 0 && b.flash.iter().any(|&f| f > 5) {
                format!("CHAIN X{}", b.chain)
            } else {
                "KEEP IT ROLLING".into()
            },
        ),
        center(192, 7, "MOSSBELL".into()),
        (12, 8, format!("{} S", s.left.div_ceil(HZ))),
        (
            329,
            8,
            format!("{}G POT", GOLD_PER_SEAT * s.tables.len() as u32),
        ),
        (6, 55, "THE RACE".into()),
        (332, 55, "KEEPER".into()),
        (332, 80, format!("{} / 6", b.hp)),
        (332, 99, "JACKPOT".into()),
        (
            332,
            111,
            if s.jackpot.is_empty() {
                "OPEN".into()
            } else {
                "WON".into()
            },
        ),
        (332, 143, "BUMPERS,".into()),
        (332, 155, "THEN".into()),
        (332, 165, "KEEPER".into()),
        center(158, 208, "FLIP AND NUDGE".into()),
    ];
    if compact_hud() {
        v.retain(|&(x, y, _)| !(x < 58 && y >= 55) && y != 208);
        for (n, &who) in s.tables.keys().enumerate() {
            v.push((
                strip_x(n) + 10,
                strip_y() + 1,
                format!("{} {}", s.place(who), s.points(who)),
            ));
        }
        if crate::rules::mossbell::LENGTH - s.left
            < hud()["compact"]["hide_hints_after"].as_u64().unwrap() as u32
        {
            v.push(center(158, 208, "FLIP AND NUDGE".into()));
        }
    } else {
        for (n, &who) in s.tables.keys().enumerate() {
            let y = 72 + n as i32 * 17;
            let short = name(run, who, id).chars().take(3).collect::<String>();
            v.push((6, y, format!("{} {short}", s.place(who))));
            let score = s.points(who).to_string();
            v.push((50 - text_width(&score), y + 8, score));
        }
    }
    v.push((6, 145, format!("{} DRAINS", b.drains)));
    if b.save > 0 {
        v.push(center(192, 157, "F LAUNCH".into()));
    }
    v
}
fn plates() -> &'static [Option<Img>; 5] {
    static PLATES: OnceLock<[Option<Img>; 5]> = OnceLock::new();
    PLATES.get_or_init(|| {
        std::array::from_fn(|i| {
            let (name, w, h) = [
                ("title", 268, 36),
                ("panel", 56, 111),
                ("panel", 56, 146),
                ("panel", 140, 17),
                ("panel", 348, 197),
            ][i];
            skin(slot("mossbell.chrome", name), w, h)
        })
    })
}
fn knight(im: &mut Img, id: u32, left: bool, right: bool, cells: bool) {
    if super::slot_art::enabled() {
        static IMPORTED: OnceLock<[[Option<Img>; 4]; 4]> = OnceLock::new();
        static CELL: OnceLock<[[Option<Img>; 4]; 4]> = OnceLock::new();
        let bank = if cells { &CELL } else { &IMPORTED };
        let bank = bank.get_or_init(|| {
            let cleanup: serde_json::Value = serde_json::from_str(include_str!(
                "../../../../assets/art-slots/mossbell.knight/cleanup.json"
            ))
            .unwrap();
            std::array::from_fn(|seat| {
                std::array::from_fn(|pose| {
                    let name = ["neutral", "left-active", "right-active", "both-active"][pose];
                    slot("mossbell.knight", name).map(|im| {
                        let body = seat_material(
                            &clean(&im, &cleanup[name]),
                            &cleanup[name],
                            seat_ink(seat as u32 + 1),
                        );
                        if cells {
                            let rim =
                                [[195, 47, 57], [45, 109, 192], [45, 143, 69], [193, 145, 35]]
                                    [seat];
                            cell_sprite(
                                &body,
                                &[
                                    [18, 29, 40],
                                    [69, 96, 112],
                                    [143, 163, 168],
                                    rim,
                                    [165, 118, 57],
                                ],
                                rim,
                            )
                        } else {
                            body
                        }
                    })
                })
            })
        });
        if let Some(body) =
            &bank[((id.saturating_sub(1)) % 4) as usize][usize::from(left) + 2 * usize::from(right)]
        {
            im.stamp(body, 240, 195);
            return;
        }
    }
    static KNIGHTS: OnceLock<[[Img; 4]; 4]> = OnceLock::new();
    let cache = KNIGHTS.get_or_init(|| {
        std::array::from_fn(|seat| {
            std::array::from_fn(|pose| {
                let mut im = Img::new(20, 24);
                // A native-grain back view, with articulated forearms on each lever.
                im.rect(6, 1, 8, 7, 's');
                im.rect(5, 3, 10, 4, 's');
                im.rect(7, 1, 6, 2, 'U');
                im.rect(6, 3, 8, 4, 'U');
                im.line(6, 3, 11, 3, 'u');
                im.rect(9, 4, 3, 3, 'S');
                im.rect(7, 8, 6, 2, 'k');
                im.rect(5, 10, 10, 9, 's');
                im.rect(6, 10, 8, 8, seat_ink(seat as u32 + 1));
                im.rect(8, 11, 3, 6, 'S');
                im.line(5, 18, 14, 18, 'o');
                im.rect(5, 19, 4, 4, 'S');
                im.rect(11, 19, 4, 4, 'S');
                im.line(4, 23, 8, 23, 's');
                im.line(11, 23, 15, 23, 's');
                for (x, dx, active) in [(5, -4, pose & 1 > 0), (14, 4, pose & 2 > 0)] {
                    let y = if active { 7 } else { 14 };
                    im.line(x, 10, x + dx, y, 's');
                    im.line(x, 11, x + dx, y + 1, 'S');
                    im.rect(x + dx - 1, y, 3, 2, 'o');
                }
                im
            })
        })
    });
    im.stamp(
        &cache[((id.saturating_sub(1)) % 4) as usize][usize::from(left) + 2 * usize::from(right)],
        240,
        195,
    );
}
pub(super) fn layers(run: &Run, who: Option<u32>) -> (Img, Img) {
    draw(run, who, true)
}
pub(super) fn terminal_layers(run: &Run, who: Option<u32>) -> (Img, Img) {
    draw(run, who, false)
}
fn draw(run: &Run, who: Option<u32>, words: bool) -> (Img, Img) {
    let s = run.mossbell.as_ref().unwrap();
    let id = focus(run, who);
    let b = &s.tables[&id].ball;
    let a = art();
    let mut im = if words {
        room().clone()
    } else {
        static CELL_ROOM: OnceLock<Img> = OnceLock::new();
        CELL_ROOM.get_or_init(|| cell_backdrop(room())).clone()
    };
    let mut fx = Img::new(NATIVE_W, NATIVE_H);
    // Steam belongs to braille atmosphere; every contact remains in solid pixels.
    for side in [0, 1] {
        let x = if side == 0 { 32 } else { 351 };
        let rise = (run.tick / 3 % 9) as i32;
        if let Some(steam) = &a.steam {
            fx.stamp(steam, x - 2, 30 - rise);
        } else {
            fx.line(x, 43 - rise, x + 3, 36 - rise, 'S');
            fx.line(x + 3, 36 - rise, x + 1, 30 - rise, 's');
        }
    }
    if compact_hud() && s.left > 0 {
        static BACK: OnceLock<Option<Img>> = OnceLock::new();
        let backdrop = BACK.get_or_init(|| slot("mossbell.room", "room").map(dim));
        if let Some(backdrop) = backdrop {
            for y in 47..158 {
                for x in 2..56 {
                    if let Some(c) = backdrop.get(x, y) {
                        im.set(
                            x,
                            y,
                            if words {
                                c
                            } else {
                                c.map(|v| (u16::from(v) * 3 / 10) as u8)
                            },
                        );
                    }
                }
            }
        }
        im.rect(58, 33, 268, 12, 's');
        for (n, &who) in s.tables.keys().enumerate() {
            let col = [[195, 47, 57], [45, 109, 192], [45, 143, 69], [193, 145, 35]]
                [(who.saturating_sub(1) % 4) as usize];
            for y in strip_y() + 1..strip_y() + 8 {
                for x in strip_x(n) + 2..strip_x(n) + 8 {
                    im.set(x, y, col);
                }
            }
        }
    }
    if s.left == 0 {
        chrome::panel(&mut im, (18, 20, 348, 197), chrome::Tone::BRASS);
        if super::slot_art::enabled() {
            if let Some(plate) = &plates()[4] {
                im.stamp(plate, 18, 20);
            }
        }
    } else {
        let shake = if b.freeze > 0 || b.burst > 12 {
            if run.tick % 2 == 0 { 1 } else { -1 }
        } else {
            0
        };
        let x = OX + shake;
        let target = |im: &mut Img, i: usize| {
            let (bx, by, _) = BUMPERS[i];
            im.stamp(&a.bumpers[i], x + field_x(bx) - 14, OY + by - 14);
            if b.lit & (1 << i) > 0 {
                im.rect(x + field_x(bx) - 3, OY + by + 14, 6, 2, 't');
            }
            if b.flash[i] > 0 {
                let frame = ((12 - b.flash[i]) / 3).min(3) as usize;
                im.stamp(&a.flashes[i][frame], x + field_x(bx) - 14, OY + by - 14);
            }
        };
        target(&mut im, 0);
        target(&mut im, 1);
        if matches!(b.beat, Beat::RootTell | Beat::Root) {
            let total = if b.hp > 2 { 36 } else { 30 };
            let stage = if b.beat == Beat::Root {
                3
            } else {
                ((total - b.beat_left.min(total)) * 4 / total).min(3) as usize
            };
            im.stamp(
                &a.roots[(b.attack & 1) as usize][stage],
                x + field_x(7),
                OY + 72,
            );
        }
        // The centre pair of drop targets sits above the travelling root.
        target(&mut im, 2);
        if matches!(b.beat, Beat::CrownTell | Beat::Crown) {
            im.stamp(&a.crown_floor, x + field_x(60), OY + 20);
        }
        let stage = if b.hp > 4 {
            0
        } else if b.hp > 2 {
            1
        } else {
            2
        };
        let pose = match b.beat {
            Beat::RootTell => 1,
            Beat::Root => 2,
            Beat::CrownTell => 3,
            Beat::Crown => 4,
            Beat::Flinch => 5,
            Beat::Defeat => 8,
            Beat::Idle => {
                if stage == 0 {
                    0
                } else {
                    5 + stage
                }
            }
        };
        im.stamp(
            if b.beat == Beat::Defeat && b.hit > 4 && a.collapse.is_some() {
                &a.collapse.as_ref().unwrap()[usize::from(!words)]
            } else if words {
                &a.poses[stage][pose]
            } else {
                &a.cells[stage][pose]
            },
            x + field_x(80) - 24,
            OY - 3,
        );
        if matches!(b.beat, Beat::CrownTell | Beat::Crown) {
            // Authored jaws stay inside the real x=60..100, y=20..43 landing gate.
            let total = if b.hp > 2 { 36 } else { 30 };
            let stage = if b.beat == Beat::Crown {
                3
            } else {
                ((total - b.beat_left.min(total)) * 4 / total).min(3) as usize
            };
            im.stamp(
                if words {
                    &a.crown[stage]
                } else {
                    &a.crown_cells[stage]
                },
                x + field_x(60),
                OY + 20,
            );
        }
        im.rect(332, 70, 42, 4, 's');
        im.rect(333, 71, 40 * i32::from(b.hp) / 6, 2, 'o');
        if let Some(pair) = &a.flippers[usize::from(b.left > 0) + 2 * usize::from(b.right > 0)] {
            im.stamp(pair, x + field_x(37) - 3, OY + 111 - 3);
        } else {
            for (left, pivot, tip, up) in [
                (
                    true,
                    (37, 119),
                    (70, if b.left > 0 { 111 } else { 130 }),
                    b.left > 0,
                ),
                (
                    false,
                    (123, 119),
                    (90, if b.right > 0 { 111 } else { 130 }),
                    b.right > 0,
                ),
            ] {
                for dy in -2..=2 {
                    im.line(
                        x + field_x(pivot.0),
                        OY + pivot.1 + dy,
                        x + field_x(tip.0),
                        OY + tip.1 + dy,
                        'I',
                    );
                }
                im.line(
                    x + field_x(pivot.0),
                    OY + pivot.1 - 1,
                    x + field_x(tip.0),
                    OY + tip.1 - 1,
                    if up { 't' } else { 'o' },
                );
                let _ = left;
            }
        }
        if b.save == 0 {
            for &(tx, ty) in b.trail.iter().rev() {
                im.stamp(&a.trail, x + field_x(tx) - 2, OY + ty - 2);
            }
            im.stamp(&a.ball, x + field_x(b.x / Q) - 4, OY + b.y / Q - 4);
        }
        if b.burst > 0 {
            let frame = ((16 - b.burst.min(16)) / 4).min(3) as usize;
            let effect = if b.jackpot_at.is_some() && b.impact == (80, 63) {
                2
            } else {
                1
            };
            im.stamp(
                &a.effects[effect][frame],
                x + field_x(b.impact.0) - 16,
                OY + b.impact.1 - 16,
            );
        }
        if words && b.chain > 0 && b.flash.iter().any(|&f| f > 5) {
            let pop = &a.pops[usize::from(b.chain - 1)];
            im.stamp(pop, 192 - pop.w / 2, 20);
        }
        knight(&mut im, id, b.left > 0, b.right > 0, !words);
    }
    if !words {
        for (x, y, t) in labels(run, who) {
            let x = if s.left == 0 && (49..238).contains(&x) && (y == 47 || (66..=138).contains(&y))
            {
                236 - t.chars().count() as i32 * 4
            } else {
                x / 4 * 4
            };
            for yy in y / 8 * 8..y / 8 * 8 + 8 {
                for xx in x..x + t.chars().count() as i32 * 4 {
                    im.set(xx, yy, [12, 18, 24]);
                }
            }
        }
    }
    if words {
        for (x, y, s) in labels(run, who) {
            if y == 25 && s.starts_with("CHAIN ") {
                continue;
            }
            im.text(x, y, &s, 'i');
        }
    }
    (im, fx)
}
pub(super) fn sign(im: &mut Img, run: &Run) {
    if run.room().kind != crate::rules::RoomKind::Bathhouse || run.mossbell.is_some() {
        return;
    }
    chrome::panel(im, (209, 151, 127, 31), chrome::Tone::BRASS);
    im.text(219, 158, "MOSSBELL", 'O');
    im.text(219, 169, "E PLAY", 'i');
    im.stamp(&art().poses[0][0], 243, 107);
}

#[cfg(test)]
mod slot_tests {
    use super::*;

    #[test]
    fn composed_danger_art_retains_registration_and_every_crown_safe_gap() {
        super::super::slot_art::use_original(true);
        let crown = art().crown.clone();
        super::super::slot_art::use_original(false);
        assert_ne!(art().crown, crown);
        assert_eq!(
            art().crown_cells,
            crown,
            "unreadable cell composition falls back"
        );
        for (old, new) in crown.iter().zip(&art().crown) {
            assert_eq!((new.w, new.h), (65, 24));
            for y in 0..new.h {
                for x in 0..new.w {
                    assert_eq!(old.get(x, y).is_some(), new.get(x, y).is_some());
                }
            }
        }
        for direction in &art().roots {
            for frame in direction {
                assert_eq!((frame.w, frame.h), (field_x(150) - field_x(7), 11));
            }
        }
        for pair in &art().flippers {
            let pair = pair.as_ref().expect("composed paddle pair");
            assert_eq!((pair.w, pair.h), (144, 26));
        }
        let bytes = include_bytes!("../../../../assets/mossbell/composed/current-root-0.png");
        assert!(
            composed(bytes, 179, 11).is_none(),
            "wrong-size art falls back"
        );
        super::super::slot_art::use_original(true);
        assert!(art().flippers.iter().all(Option::is_none));
        assert!(composed(bytes, 229, 11).is_none());
        super::super::slot_art::use_original(false);
    }

    #[test]
    fn four_seat_operators_keep_distinct_cloth_and_no_lever_posts() {
        super::super::slot_art::use_original(false);
        for pose in 0..4 {
            let frames: Vec<_> = (1..=4)
                .map(|seat| {
                    let mut frame = Img::new(NATIVE_W, NATIVE_H);
                    knight(&mut frame, seat, pose & 1 != 0, pose & 2 != 0, false);
                    frame
                })
                .collect();
            for a in 0..4 {
                for b in a + 1..4 {
                    assert_ne!(frames[a], frames[b], "pose {pose}: seats {a}/{b}");
                }
            }
            for frame in frames {
                for y in 213..219 {
                    for x in [240, 241, 242, 243, 256, 257, 258, 259] {
                        assert_eq!(frame.get(x, y), None, "lever hardware remains");
                    }
                }
            }
        }
    }
}
