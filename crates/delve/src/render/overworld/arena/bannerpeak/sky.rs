//! The painted sky behind the level: one static 384 by 224 picture under the
//! tiles, props, foes and knights (`bannerpeak.sky`). It does not scroll, so a
//! knight crossing the mountain always has the same valley behind them, under
//! a dark layer of a quarter so the play reads over it. Under
//! the street's stone there is no sky: the picture fades into the flat dark the
//! code's own backdrop has there, so the hub's cellar and the cave under the
//! marsh are not lit by a moon.
use super::super::super::ink::{Img, ink, nearest_plain};
use super::{H, W, bank};
use std::sync::OnceLock;

const UNDER: i32 = 800;
const FADE: i32 = 32;
const DEEP: char = 's';

const KEEP: f32 = 0.75;

fn backdrop() -> Option<&'static Img> {
    static DIMMED: OnceLock<Option<Img>> = OnceLock::new();
    DIMMED
        .get_or_init(|| {
            let mut sky = bank::at("sky", "night", 0)?.img.clone();
            for y in 0..sky.h {
                for x in 0..sky.w {
                    if let Some(c) = sky.get(x, y) {
                        sky.set(x, y, nearest_plain(c.map(|v| f32::from(v) * KEEP)));
                    }
                }
            }
            Some(sky)
        })
        .as_ref()
}

pub(super) fn paint(im: &mut Img, cam: (i32, i32)) -> bool {
    let Some(sky) = backdrop() else {
        return false;
    };
    im.stamp(sky, 0, 0);
    let deep = ink(DEEP).expect("a deep ink");
    for y in 0..H {
        let wy = cam.1 + y;
        if wy <= UNDER {
            continue;
        }
        if wy >= UNDER + FADE {
            im.rect(0, y, W, 1, DEEP);
            continue;
        }
        let k = (wy - UNDER) as f32 / FADE as f32;
        for x in 0..W {
            if let Some(c) = im.get(x, y) {
                let mixed = [0, 1, 2].map(|i| c[i] as f32 + (deep[i] as f32 - c[i] as f32) * k);
                im.set(x, y, nearest_plain(mixed));
            }
        }
    }
    true
}
