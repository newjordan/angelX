//! Two ordinary town residents, painted only when the Realm paints a frame.
//! Their names evoke memory upkeep and delivery; they report no work or status.
//! No simulation, scheduler, I/O, or model activity is attached to them.

use std::sync::OnceLock;

use super::ink::{Img, nearest_plain};
use super::map::TILE;
use super::scene::Prop;
use super::scene::Motion;

struct Clerk {
    rgba: &'static [u8],
    feet: (i32, i32),
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

#[cfg(test)]
const HEIGHT: u32 = 20;
const SOLID: u8 = 96;

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
