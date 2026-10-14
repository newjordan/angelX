//! Two ordinary town residents, painted only when the Realm paints a frame.
//! Their names evoke memory upkeep and delivery; they report no work or status.
//! No simulation, scheduler, I/O, or model activity is attached to them.

use std::sync::OnceLock;

use super::ink::{Img, nearest_plain};
use super::map::TILE;
use super::scene::Prop;
use super::scene::Motion;

struct Clerk {
    /// The sprite as `build.rs` shrank it from the painting: width and
    /// height (u16 LE), then RGBA rows.
    rgba: &'static [u8],
    /// Feet centre in the map's authored coordinates, before place translation.
    feet: (i32, i32),
    /// A small road segment, in pixels. Zero keeps the archivist at the library.
    walk: u32,
}

const CLERKS: [Clerk; 2] = [
    Clerk {
        rgba: include_bytes!(concat!(env!("OUT_DIR"), "/archivist.rgba")),
        feet: (28 * TILE + TILE / 2, 19 * TILE + 13),
        walk: 0,
    },
    Clerk {
        rgba: include_bytes!(concat!(env!("OUT_DIR"), "/courier.rgba")),
        feet: (24 * TILE, 19 * TILE + 13),
        walk: 3 * TILE as u32,
    },
];

/// Prepare the tiny, palette-bound cast on the first Realm frame. Mirrored
/// poses are retained too; subsequent scene composition only copies two
/// small sprites into Props.
fn sprites() -> &'static [[Img; 2]; CLERKS.len()] {
    static SPRITES: OnceLock<[[Img; 2]; CLERKS.len()]> = OnceLock::new();
    SPRITES.get_or_init(|| {
        std::array::from_fn(|index| {
            let image = sprite(CLERKS[index].rgba);
            let mirrored = image.flip_h();
            [image, mirrored]
        })
    })
}

/// The sprite's height, in pixels (`build.rs` shrinks the paintings to it).
#[cfg(test)]
const HEIGHT: u32 = 20;
/// A pixel this opaque is part of the figure.
const SOLID: u8 = 96;

/// A shrunk resident in the plain palette.
fn sprite(rgba: &[u8]) -> Img {
    let size = |at: usize| u16::from_le_bytes([rgba[at], rgba[at + 1]]) as i32;
    let (width, height) = (size(0), size(2));
    if width == 0 || height == 0 {
        return Img::new(1, 1);
    }
    let mut image = Img::new(width, height);
    for (i, pixel) in rgba[4..].chunks_exact(4).enumerate() {
        if pixel[3] >= SOLID {
            let (x, y) = (i as i32 % width, i as i32 / width);
            // Signal colors remain reserved for actual harness events.
            image.set(x, y, nearest_plain([pixel[0] as f32, pixel[1] as f32, pixel[2] as f32]));
        }
    }
    image
}

pub(super) fn stage(tick: u32, motion: Motion) -> [Prop; CLERKS.len()] {
    let tick = if motion == Motion::Full { tick } else { 0 };
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
#[path = "../../../tests/render/overworld__clerks_tests.rs"]
mod tests;
