//! Rock breaking in the cave, drawn from sprite sheets: a chip off a wall
//! that holds, a wall crumbling to rubble, a magic blast. Each has a sheet a
//! region, in that region's stone.
//!
//! A sheet's frames run left to right, top to bottom, at the simulation's own
//! thirty a second, so a burst's age in ticks is its frame. Its pixels carry
//! their colour already multiplied by their cover. A frame is mostly empty,
//! so only the pixels that show are kept.

use std::cell::RefCell;
use std::sync::OnceLock;

use crate::ink::Img;
use crate::rules::caves::{Burst, BURST_BLAST, BURST_CHIP, BURST_CRUMBLE};

struct Kind {
    frames: u32,
    /// One frame's size, and how many stand in a row of the sheet.
    w: u32,
    h: u32,
    columns: u32,
    /// The point of a frame that sits on the floor line under the struck
    /// wall's face, at the middle of its cell.
    pivot: (i32, i32),
    /// The sheets, by region: mine, hollow, foundry, frost.
    sheets: [&'static [u8]; 4],
}

macro_rules! sheets {
    ($name:literal) => {
        [
            include_bytes!(concat!("../../../assets/cave_fx/", $name, "-mine.png")),
            include_bytes!(concat!("../../../assets/cave_fx/", $name, "-hollow.png")),
            include_bytes!(concat!("../../../assets/cave_fx/", $name, "-foundry.png")),
            include_bytes!(concat!("../../../assets/cave_fx/", $name, "-frost.png")),
        ]
    };
}

/// By `Burst::kind`: chip, crumble, blast.
static KINDS: [Kind; 3] = [
    Kind { frames: 28, w: 72, h: 72, columns: 6, pivot: (36, 32), sheets: sheets!("rock-chip") },
    Kind { frames: 61, w: 64, h: 72, columns: 9, pivot: (32, 46), sheets: sheets!("rock-crumble") },
    Kind { frames: 67, w: 168, h: 140, columns: 8, pivot: (84, 88), sheets: sheets!("rock-blast") },
];

/// One pixel of a frame: where it lies from the pivot, and its colour
/// multiplied by its cover.
struct Dab {
    dx: i16,
    dy: i16,
    rgba: [u8; 4],
}

struct Sheet {
    frames: Vec<Vec<Dab>>,
}

/// A sheet is read the first time it is drawn, or when `warm` asks.
static SHEETS: [OnceLock<Sheet>; 12] = [const { OnceLock::new() }; 12];

/// Cover fainter than this does not show; such pixels are not kept.
const FAINT: u8 = 6;

fn kind_of(burst: &Burst) -> Option<usize> {
    match burst.kind {
        BURST_CHIP => Some(0),
        BURST_CRUMBLE => Some(1),
        BURST_BLAST => Some(2),
        _ => None,
    }
}

fn sheet(kind: usize, region: u8) -> &'static Sheet {
    let region = usize::from(region.min(3));
    SHEETS[kind * 4 + region].get_or_init(|| {
        let spec = &KINDS[kind];
        decode(spec.sheets[region], spec.frames, spec.w, spec.h, spec.columns, spec.pivot)
    })
}

fn decode(png: &[u8], frames_n: u32, w: u32, h: u32, columns: u32, pivot: (i32, i32)) -> Sheet {
    let png = image::load_from_memory(png).expect("cave effect sheet").to_rgba8();
    let mut frames = Vec::with_capacity(frames_n as usize);
    for n in 0..frames_n {
        let (fx, fy) = ((n % columns) * w, (n / columns) * h);
        let mut dabs = Vec::new();
        for y in 0..h {
            for x in 0..w {
                let p = png.get_pixel(fx + x, fy + y).0;
                if p[3] >= FAINT {
                    dabs.push(Dab {
                        dx: (x as i32 - pivot.0) as i16,
                        dy: (y as i32 - pivot.1) as i16,
                        rgba: p,
                    });
                }
            }
        }
        frames.push(dabs);
    }
    Sheet { frames }
}

/// Read a region's three sheets now, so the first hit there does not wait on it.
pub(crate) fn warm(region: u8) {
    for kind in 0..KINDS.len() {
        let _ = sheet(kind, region);
    }
}

/// Read all twelve rock sheets (four regions, three kinds) during entry.
pub(crate) fn warm_all() {
    for region in 0..4 {
        warm(region);
    }
    warm_loops();
}

/// Lay `burst`'s frame for `tick` over `im`, its pivot at `at` on the screen.
/// Nothing is drawn before it starts or after its last frame.
pub(crate) fn stamp(im: &mut Img, burst: &Burst, region: u8, tick: u32, at: (i32, i32)) {
    stamp_if(im, burst, region, tick, at, |_, _, _| true);
}

/// As `stamp`, but only dabs whose offset from the pivot is between `dy_lo`
/// and `dy_hi`, and only on screen rows above `clip_y`.
pub(crate) fn stamp_band(
    im: &mut Img,
    burst: &Burst,
    region: u8,
    tick: u32,
    at: (i32, i32),
    dy_lo: i32,
    dy_hi: i32,
    clip_y: i32,
) {
    stamp_if(im, burst, region, tick, at, |_, y, dy| dy >= dy_lo && dy <= dy_hi && y < clip_y);
}

/// As `stamp`, but a dab is laid only when `keep` says so. `dy` is the dab's
/// offset from the pivot: negative is the breaking cloud, positive the rubble.
pub(crate) fn stamp_if(
    im: &mut Img,
    burst: &Burst,
    region: u8,
    tick: u32,
    at: (i32, i32),
    mut keep: impl FnMut(i32, i32, i32) -> bool,
) {
    let Some(kind) = kind_of(burst) else {
        return;
    };
    let Some(age) = tick.checked_sub(burst.at) else {
        return;
    };
    if age >= KINDS[kind].frames {
        return;
    }
    for dab in &sheet(kind, region).frames[age as usize] {
        let dy = i32::from(dab.dy);
        let (x, y) = (at.0 + i32::from(dab.dx), at.1 + dy);
        if x < 0 || y < 0 || x >= im.w || y >= im.h || !keep(x, y, dy) {
            continue;
        }
        let [r, g, b, a] = dab.rgba;
        if a == 255 {
            im.set(x, y, [r, g, b]);
            continue;
        }
        let under = im.get(x, y).unwrap_or([0, 0, 0]);
        let stay = 255 - u32::from(a);
        let mix = |over: u8, under: u8| (u32::from(over) + (u32::from(under) * stay + 127) / 255).min(255) as u8;
        im.set(x, y, [mix(r, under[0]), mix(g, under[1]), mix(b, under[2])]);
    }
}

// One sheet each, not per region. Premultiplied, same blend as the rock sheets.
const LOOPS: [Loop; 4] = [
    Loop { frames: 14, w: 24, h: 28, columns: 5, pivot: (12, 24), png: include_bytes!("../../../assets/cave_fx/flame-cell.png") },
    Loop { frames: 24, w: 24, h: 36, columns: 6, pivot: (12, 30), png: include_bytes!("../../../assets/cave_fx/steam-wisp.png") },
    Loop { frames: 18, w: 16, h: 16, columns: 5, pivot: (8, 12), png: include_bytes!("../../../assets/cave_fx/acid-fizz.png") },
    Loop { frames: 36, w: 16, h: 20, columns: 7, pivot: (8, 14), png: include_bytes!("../../../assets/cave_fx/ash-embers.png") },
];

pub(crate) const LOOP_FLAME: usize = 0;
pub(crate) const LOOP_STEAM: usize = 1;
pub(crate) const LOOP_FIZZ: usize = 2;
pub(crate) const LOOP_ASH: usize = 3;

struct Loop {
    frames: u32,
    w: u32,
    h: u32,
    columns: u32,
    pivot: (i32, i32),
    png: &'static [u8],
}

fn warm_loops() {
    for i in 0..LOOPS.len() {
        let _ = loop_sheet(i);
    }
}

fn loop_sheet(which: usize) -> &'static Sheet {
    static SHEETS: [OnceLock<Sheet>; 4] = [OnceLock::new(), OnceLock::new(), OnceLock::new(), OnceLock::new()];
    SHEETS[which].get_or_init(|| {
        let spec = &LOOPS[which];
        decode(spec.png, spec.frames, spec.w, spec.h, spec.columns, spec.pivot)
    })
}

pub(crate) fn loop_frames(which: usize) -> u32 {
    LOOPS[which].frames
}

/// Sheet frame for this tick, offset by a hash of the cell so neighbours do not pulse together.
pub(crate) fn loop_frame(tick: u32, x: i32, y: i32, which: usize, salt: u32) -> u32 {
    let frames = loop_frames(which);
    let off = crate::ink::hash(x, y, salt) % frames;
    tick.wrapping_add(off) % frames
}

pub(crate) fn stamp_loop(im: &mut Img, which: usize, frame: u32, at: (i32, i32), cover: u8) {
    if cover == 0 {
        return;
    }
    let frames = loop_sheet(which).frames.len() as u32;
    if frames == 0 {
        return;
    }
    let frame = (frame % frames) as usize;
    for dab in &loop_sheet(which).frames[frame] {
        let (x, y) = (at.0 + i32::from(dab.dx), at.1 + i32::from(dab.dy));
        if x < 0 || y < 0 || x >= im.w || y >= im.h {
            continue;
        }
        let [r, g, b, a] = scale_cover(dab.rgba, cover);
        if a == 0 {
            continue;
        }
        if a == 255 {
            im.set(x, y, [r, g, b]);
            continue;
        }
        let under = im.get(x, y).unwrap_or([0, 0, 0]);
        let stay = 255 - u32::from(a);
        let mix = |over: u8, under: u8| (u32::from(over) + (u32::from(under) * stay + 127) / 255).min(255) as u8;
        im.set(x, y, [mix(r, under[0]), mix(g, under[1]), mix(b, under[2])]);
    }
}

fn scale_cover(px: [u8; 4], cover: u8) -> [u8; 4] {
    if cover == 255 {
        return px;
    }
    let c = u32::from(cover);
    [
        ((u32::from(px[0]) * c) / 255) as u8,
        ((u32::from(px[1]) * c) / 255) as u8,
        ((u32::from(px[2]) * c) / 255) as u8,
        ((u32::from(px[3]) * c) / 255) as u8,
    ]
}

struct AshBook {
    seed: u32,
    tick: u32,
    born: Vec<(i32, i32, u32)>,
}

thread_local! {
    static ASH: RefCell<AshBook> = RefCell::new(AshBook { seed: 0, tick: 0, born: Vec::new() });
}

/// First 240 ticks after the painter saw this cell turn to ash.
pub(crate) fn ash_embers_live(seed: u32, x: i32, y: i32, tick: u32, is_ash: bool) -> bool {
    ASH.with(|slot| {
        let mut book = slot.borrow_mut();
        if book.seed != seed || tick < book.tick {
            book.born.clear();
            book.seed = seed;
        }
        book.tick = tick;
        if !is_ash {
            book.born.retain(|born| born.0 != x || born.1 != y);
            return false;
        }
        let born = if let Some(found) = book.born.iter().find(|born| born.0 == x && born.1 == y) {
            found.2
        } else {
            if book.born.len() > 512 {
                book.born.drain(0..256);
            }
            book.born.push((x, y, tick));
            tick
        };
        tick.saturating_sub(born) < 240
    })
}

