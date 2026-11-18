//! The Round Table panel's picture: the painted station table
//! (`assets/realm/v2/interiors/station-round-table.png`) lit by the live
//! council, composed in Rust and shown through the viewer's Kitty path.
//!
//! Light means activity: a sitting seat's chair is lit and its candle burns before it; a seat that
//! answered has its parchment laid out bright; a failed answer lies there in
//! red; a cut seat and an empty chair sit in the dark. The map at the centre
//! shows the deed: scales while judges weigh, a wax seal while verifiers
//! check, the gathered answers during synthesis.

use crate::ui::viz::agentviz::SeatState;
use image::{Rgba, RgbaImage};
use std::sync::OnceLock;

const STATION: &[u8] = include_bytes!("../../../assets/realm/v2/interiors/station-round-table.png");

const CROP: (u32, u32, u32, u32) = (4, 7, 136, 125);

pub(crate) const WIDTH: u32 = CROP.2;
pub(crate) const HEIGHT: u32 = CROP.3;

const CHAIRS: [((i32, i32), (i32, i32)); 8] = [
    ((72, 26), (72, 45)),
    ((113, 38), (104, 53)),
    ((131, 68), (117, 75)),
    ((121, 104), (102, 95)),
    ((72, 120), (72, 102)),
    ((23, 104), (42, 95)),
    ((13, 68), (27, 75)),
    ((31, 38), (40, 53)),
];

const STOOLS: [(i32, i32); 8] = [
    (88, 46),
    (113, 63),
    (112, 87),
    (88, 101),
    (56, 101),
    (32, 87),
    (31, 63),
    (56, 46),
];

const MAP_CENTRE: (i32, i32) = (72, 74);

const TABLE_TOP: ((i32, i32), f32, f32) = ((72, 75), 57.0, 41.0);

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub(crate) enum Deed {
    #[default]
    Council,
    Synthesis,
    Judge,
    Verify,
}

impl Deed {
    pub(crate) fn of_stage(stage: &str) -> Self {
        match stage {
            "synthesis" => Self::Synthesis,
            stage if stage.starts_with("judge") => Self::Judge,
            "verify" => Self::Verify,
            _ => Self::Council,
        }
    }
}

const INK_N: [u8; 3] = [0x1a, 0x0e, 0x07];
const INK_FLAME_CORE: [u8; 3] = [0xff, 0xf2, 0xc4];
const INK_6: [u8; 3] = [0xfb, 0xd0, 0x70];
const INK_4: [u8; 3] = [0xec, 0xb6, 0x4a];
const INK_C: [u8; 3] = [0xe3, 0xd2, 0xc3];
const INK_DOLLAR: [u8; 3] = [0xc7, 0xb4, 0x8f];
const INK_WAX: [u8; 3] = [0xa8, 0x2a, 0x22];
const INK_WAX_DARK: [u8; 3] = [0x5e, 0x12, 0x0e];
const INK_WAX_LIGHT: [u8; 3] = [0xd8, 0x5a, 0x44];

fn station() -> &'static RgbaImage {
    static PLATE: OnceLock<RgbaImage> = OnceLock::new();
    PLATE.get_or_init(|| {
        image::load_from_memory(STATION)
            .map(|plate| plate.to_rgba8())
            .unwrap_or_else(|_| RgbaImage::new(144, 144))
    })
}

fn seat_places(count: usize) -> Vec<Place> {
    let count = count.min(CHAIRS.len() + STOOLS.len());
    if count <= CHAIRS.len() {
        return (0..count)
            .map(|seat| Place::Chair(seat * CHAIRS.len() / count.max(1)))
            .collect();
    }
    (0..CHAIRS.len())
        .map(Place::Chair)
        .chain((0..count - CHAIRS.len()).map(Place::Stool))
        .collect()
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Place {
    Chair(usize),
    Stool(usize),
}

fn bayer(x: i32, y: i32) -> f32 {
    const M: [[u8; 4]; 4] = [[0, 8, 2, 10], [12, 4, 14, 6], [3, 11, 1, 9], [15, 7, 13, 5]];
    (f32::from(M[(y & 3) as usize][(x & 3) as usize]) + 0.5) / 16.0
}

fn pool(x: i32, y: i32, at: (i32, i32), rx: f32, ry: f32) -> f32 {
    let dx = (x - at.0) as f32 / rx;
    let dy = (y - at.1) as f32 / ry;
    let d = (dx * dx + dy * dy).sqrt();
    if d >= 1.0 {
        return 0.0;
    }
    let raw = (1.0 - d) * 3.0;
    let step = raw.floor();
    let frac = raw - step;
    ((step + if frac > bayer(x, y) { 1.0 } else { 0.0 }) / 3.0).min(1.0)
}

fn near(x: i32, y: i32, at: (i32, i32), rx: f32, ry: f32) -> bool {
    let dx = (x - at.0) as f32 / rx;
    let dy = (y - at.1) as f32 / ry;
    dx * dx + dy * dy <= 1.0
}

pub(crate) fn compose(states: &[SeatState], deed: Deed) -> RgbaImage {
    let plate = station();
    let places = seat_places(states.len());
    let seated: Vec<(Place, SeatState)> =
        places.iter().copied().zip(states.iter().copied()).collect();
    let chair_state = |chair: usize| {
        seated
            .iter()
            .find(|(place, _)| *place == Place::Chair(chair))
            .map(|(_, state)| *state)
    };

    let mut out = RgbaImage::new(WIDTH, HEIGHT);
    for y in 0..HEIGHT as i32 {
        for x in 0..WIDTH as i32 {
            let (px, py) = (x + CROP.0 as i32, y + CROP.1 as i32);
            let Some(&Rgba([r, g, b, a])) = plate.get_pixel_checked(px as u32, py as u32) else {
                continue;
            };
            if a == 0 {
                continue;
            }
            let mut light: f32 = if near(px, py, MAP_CENTRE, 31.0, 21.0) {
                0.95
            } else {
                0.78
            };
            let mut warm: f32 = 0.0;
            let mut grey: f32 = 0.0;
            let on_table = near(px, py, TABLE_TOP.0, TABLE_TOP.1, TABLE_TOP.2);
            for (chair, (cushion, setting)) in CHAIRS.iter().enumerate() {
                let in_chair = !on_table && near(px, py, *cushion, 12.0, 15.0);
                match chair_state(chair) {
                    Some(SeatState::Running | SeatState::Returned) => {
                        let mid = ((cushion.0 + setting.0) / 2, (cushion.1 + setting.1) / 2);
                        let lit = pool(px, py, mid, 20.0, 20.0);
                        if lit > 0.0 {
                            light = light.max(0.78 + 0.42 * lit);
                            warm = warm.max(lit * 0.25);
                        }
                    }
                    Some(SeatState::Failed) if in_chair => {
                        light = light.min(0.6);
                        warm = -0.12;
                    }
                    Some(SeatState::Cut) | None if in_chair => {
                        light = light.min(0.42);
                        grey = grey.max(0.55);
                    }
                    _ => {}
                }
            }
            for (place, state) in &seated {
                if let (Place::Stool(stool), SeatState::Running | SeatState::Returned) =
                    (place, state)
                {
                    let lit = pool(px, py, STOOLS[*stool], 11.0, 9.0);
                    if lit > 0.0 {
                        light = light.max(0.78 + 0.3 * lit);
                        warm = warm.max(lit * 0.2);
                    }
                }
            }
            let mut c = [f32::from(r), f32::from(g), f32::from(b)];
            let luma = 0.3 * c[0] + 0.59 * c[1] + 0.11 * c[2];
            for ch in &mut c {
                *ch = *ch * (1.0 - grey) + luma * grey;
            }
            c[0] = c[0] * light + warm * 90.0;
            c[1] = c[1] * light + warm * 55.0;
            c[2] = c[2] * light + warm * 10.0;
            out.put_pixel(
                x as u32,
                y as u32,
                Rgba([
                    c[0].clamp(0.0, 255.0) as u8,
                    c[1].clamp(0.0, 255.0) as u8,
                    c[2].clamp(0.0, 255.0) as u8,
                    255,
                ]),
            );
        }
    }

    deed_mark(&mut out, deed);
    for (place, state) in &seated {
        match *place {
            Place::Chair(chair) => {
                let (_, setting) = CHAIRS[chair];
                match state {
                    SeatState::Running => candle(&mut out, setting),
                    SeatState::Returned => parchment(&mut out, setting, false),
                    SeatState::Failed => parchment(&mut out, setting, true),
                    SeatState::Cut => {}
                }
            }
            Place::Stool(stool) => stool_mark(&mut out, STOOLS[stool], *state),
        }
    }
    out
}

fn put(out: &mut RgbaImage, x: i32, y: i32, ink: [u8; 3]) {
    let (x, y) = (x - CROP.0 as i32, y - CROP.1 as i32);
    if x >= 0 && y >= 0 && (x as u32) < out.width() && (y as u32) < out.height() {
        out.put_pixel(x as u32, y as u32, Rgba([ink[0], ink[1], ink[2], 255]));
    }
}

fn sprite(out: &mut RgbaImage, at: (i32, i32), rows: &[&str], inks: &[(char, [u8; 3])]) {
    let h = rows.len() as i32;
    let w = rows.iter().map(|row| row.len()).max().unwrap_or(0) as i32;
    for (dy, row) in rows.iter().enumerate() {
        for (dx, ch) in row.chars().enumerate() {
            if let Some((_, ink)) = inks.iter().find(|(key, _)| *key == ch) {
                put(
                    out,
                    at.0 - w / 2 + dx as i32,
                    at.1 - h / 2 + dy as i32,
                    *ink,
                );
            }
        }
    }
}

fn candle(out: &mut RgbaImage, at: (i32, i32)) {
    sprite(
        out,
        (at.0, at.1 - 2),
        &[
            "..o..",
            ".oyo.",
            ".oWo.",
            "oyWyo",
            ".nwn.",
            ".nwn.",
            ".nwn.",
            "nnnnn",
        ],
        &[
            ('W', INK_FLAME_CORE),
            ('y', INK_6),
            ('o', INK_4),
            ('w', INK_C),
            ('n', INK_N),
        ],
    );
}

fn parchment(out: &mut RgbaImage, at: (i32, i32), failed: bool) {
    let (sheet, line, seal) = if failed {
        (INK_WAX_LIGHT, INK_WAX_DARK, INK_WAX_DARK)
    } else {
        (INK_C, INK_DOLLAR, INK_4)
    };
    sprite(
        out,
        at,
        &[
            "nnnnnnnnn",
            "nwwwwwwwn",
            "nwllllwwn",
            "nwwwwwwwn",
            "nwlllllwn",
            "nwwwwwssn",
            "nnnnnnnnn",
        ],
        &[('w', sheet), ('l', line), ('s', seal), ('n', INK_N)],
    );
}

fn stool_mark(out: &mut RgbaImage, at: (i32, i32), state: SeatState) {
    match state {
        SeatState::Running => sprite(
            out,
            (at.0, at.1 - 1),
            &[
                ".o.",
                "oWo",
                "oyo",
                "nwn",
                "nwn",
                "nnn",
            ],
            &[
                ('W', INK_FLAME_CORE),
                ('y', INK_6),
                ('o', INK_4),
                ('w', INK_C),
                ('n', INK_N),
            ],
        ),
        SeatState::Returned | SeatState::Failed => {
            let (sheet, line) = if state == SeatState::Failed {
                (INK_WAX_LIGHT, INK_WAX_DARK)
            } else {
                (INK_C, INK_DOLLAR)
            };
            sprite(
                out,
                at,
                &[
                    "nnnnnnn",
                    "nwwwwwn",
                    "nwllwwn",
                    "nwwwwwn",
                    "nnnnnnn",
                ],
                &[('w', sheet), ('l', line), ('n', INK_N)],
            );
        }
        SeatState::Cut => {}
    }
}

fn deed_mark(out: &mut RgbaImage, deed: Deed) {
    match deed {
        Deed::Council => {}
        Deed::Judge => sprite(
            out,
            MAP_CENTRE,
            &[
                "..........nnn..........",
                "nnnnnnnnnnnynnnnnnnnnnn",
                "nyyyyyyyyyyyyyyyyyyyyyn",
                "nnnonnnnnnnynnnnnnnonnn",
                "..non.....nyn.....non..",
                ".no.on....nyn....no.on.",
                ".no.on....nyn....no.on.",
                "no...on...nyn...no...on",
                "nyyyyyn...nyn...nyyyyyn",
                ".noooon...nyn...noooon.",
                "..nnnn...nnynn...nnnn..",
                ".........nyyyn.........",
                "........nnnnnnn........",
            ],
            &[('y', INK_6), ('o', INK_4), ('n', INK_N)],
        ),
        Deed::Verify => sprite(
            out,
            MAP_CENTRE,
            &[
                "....nnnnnnn....",
                "..nnwwwwwwwnn..",
                ".nwwddddddwwwn.",
                ".nwdwwwlwwwdwn.",
                "nwwdwwwlwwwdwwn",
                "nwdwwlllllwwdwn",
                "nwdwwwwlwwwwdwn",
                "nwdwwwwlwwwwdwn",
                ".nwdwwwwwwwdwn.",
                ".nwwddddddwwwn.",
                "..nnwwwwwwwnn..",
                "....nnnnnnn....",
            ],
            &[
                ('w', INK_WAX),
                ('d', INK_WAX_DARK),
                ('l', INK_WAX_LIGHT),
                ('n', INK_N),
            ],
        ),
        Deed::Synthesis => {
            for (dx, dy) in [(-8, -2), (8, -3), (0, 1)] {
                parchment(out, (MAP_CENTRE.0 + dx, MAP_CENTRE.1 + dy), false);
            }
        }
    }
}

pub(crate) fn fit(picture: &RgbaImage, target: (u32, u32)) -> RgbaImage {
    let (w, h) = (picture.width().max(1), picture.height().max(1));
    let s = (target.0 as f32 / w as f32).min(target.1 as f32 / h as f32);
    let out = (
        ((w as f32 * s).floor() as u32).max(1),
        ((h as f32 * s).floor() as u32).max(1),
    );
    let k = s.ceil().clamp(1.0, 8.0) as u32;
    let big = image::imageops::resize(picture, w * k, h * k, image::imageops::FilterType::Nearest);
    if (big.width(), big.height()) == out {
        return big;
    }
    image::imageops::resize(&big, out.0, out.1, image::imageops::FilterType::Triangle)
}

pub(crate) fn columns_for(rows: u16, cell: (u16, u16)) -> u16 {
    let (cw, ch) = (f32::from(cell.0.max(1)), f32::from(cell.1.max(1)));
    let px_h = f32::from(rows) * ch;
    let px_w = px_h * WIDTH as f32 / HEIGHT as f32;
    (px_w / cw).round().max(1.0) as u16
}

pub(crate) fn picture_key(states: &[SeatState], deed: Deed) -> u64 {
    use std::hash::{Hash, Hasher};
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    deed.hash(&mut hasher);
    states.len().hash(&mut hasher);
    for state in states {
        (*state as u8).hash(&mut hasher);
    }
    hasher.finish()
}

#[cfg(test)]
#[path = "../../../../tests/cockpit/app/council_table__tests.rs"]
mod tests;
