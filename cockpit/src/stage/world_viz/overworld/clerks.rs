//! Two ordinary town residents, painted only when the Realm paints a frame.
//! Their names evoke memory upkeep and delivery; they report no work or status.
//! No simulation, scheduler, I/O, or model activity is attached to them.

use std::sync::OnceLock;

use super::ink::{Img, nearest_plain};
use super::map::TILE;
use super::scene::Prop;
use crate::ui::viz::lifecycle_viz::MotionMode;

struct Clerk {
    png: &'static [u8],
    /// Feet centre in the map's authored coordinates, before place translation.
    feet: (i32, i32),
    /// A small road segment, in pixels. Zero keeps the archivist at the library.
    walk: u32,
}

const CLERKS: [Clerk; 2] = [
    Clerk {
        png: include_bytes!("../../../../assets/realm/inhabitants/archivist.png"),
        feet: (28 * TILE + TILE / 2, 19 * TILE + 13),
        walk: 0,
    },
    Clerk {
        png: include_bytes!("../../../../assets/realm/inhabitants/courier.png"),
        feet: (24 * TILE, 19 * TILE + 13),
        walk: 3 * TILE as u32,
    },
];

/// Decode and prepare the tiny, palette-bound cast on the first Realm frame.
/// This runs inside the existing lazy image worker. Mirrored poses are retained
/// too; subsequent scene composition only copies two small sprites into Props.
fn sprites() -> &'static [[Img; 2]; CLERKS.len()] {
    static SPRITES: OnceLock<[[Img; 2]; CLERKS.len()]> = OnceLock::new();
    SPRITES.get_or_init(|| {
        std::array::from_fn(|index| {
            let image = sprite(CLERKS[index].png);
            let mirrored = image.flip_h();
            [image, mirrored]
        })
    })
}

fn sprite(png: &[u8]) -> Img {
    let source = image::load_from_memory(png)
        .expect("bundled town resident PNG")
        .to_rgba8();
    let (mut left, mut top) = source.dimensions();
    let (mut right, mut bottom) = (0, 0);
    for (x, y, pixel) in source.enumerate_pixels() {
        if pixel[3] >= 96 {
            left = left.min(x);
            top = top.min(y);
            right = right.max(x + 1);
            bottom = bottom.max(y + 1);
        }
    }
    if right <= left || bottom <= top {
        return Img::new(1, 1);
    }
    const HEIGHT: u32 = 20;
    let width = ((right - left) * HEIGHT / (bottom - top)).max(1);
    let cropped = image::imageops::crop_imm(&source, left, top, right - left, bottom - top);
    let small = image::imageops::resize(
        &cropped.to_image(),
        width,
        HEIGHT,
        image::imageops::FilterType::Nearest,
    );
    let mut image = Img::new(width as i32, HEIGHT as i32);
    for (x, y, pixel) in small.enumerate_pixels() {
        if pixel[3] >= 96 {
            // Signal colors remain reserved for actual harness events.
            image.set(
                x as i32,
                y as i32,
                nearest_plain([pixel[0] as f32, pixel[1] as f32, pixel[2] as f32]),
            );
        }
    }
    image
}

pub(super) fn stage(tick: u32, motion: MotionMode) -> [Prop; CLERKS.len()] {
    let tick = if motion == MotionMode::Full { tick } else { 0 };
    let sprites = sprites();
    std::array::from_fn(|index| {
        let clerk = &CLERKS[index];
        let step = if clerk.walk == 0 {
            0
        } else {
            tick % (clerk.walk * 2)
        };
        let returning = step > clerk.walk;
        let distance = if returning {
            clerk.walk * 2 - step
        } else {
            step
        };
        let image = &sprites[index][usize::from(returning)];
        let bob = if clerk.walk > 0 {
            (tick / 2 % 2) as i32
        } else {
            0
        };
        Prop {
            x: clerk.feet.0 + distance as i32 - image.w / 2,
            base: clerk.feet.1 - bob,
            img: image.clone(),
        }
    })
}

#[cfg(test)]
#[path = "../../../../../tests/cockpit/world_viz/overworld__clerks_tests.rs"]
mod tests;
