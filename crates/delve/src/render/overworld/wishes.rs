//! Wishes the party paid for, standing in the wild.
//!
//! The harness's work grows the town; the party's delves win the wild. A
//! granted wish (`together_realm`) stands on a cleared lot in a wilderness
//! screen — never on an authored place, road or water — as near as the wild
//! allows to the place it was wished beside. Lots are given out in the order
//! wishes were raised, so a new wish never moves an old one.
//!
//! Every pixel here is real state: a landmark stands only because spoils
//! from real runs paid for it.

use super::ink::Img;
use super::light::Light;
use super::map::TILE;
use super::scene::Prop;
use crate::realm::Wish;

// Where each wish stands is the world's.
pub use crate::world::wishes::*;

/// Inks that burn.
const FLAME: [char; 3] = ['5', '6', '@'];

/// Stage the standing wishes in realm coordinates.
pub fn stage(wishes: &[Wish], props: &mut Vec<Prop>, lights: &mut Vec<Light>, tick: u32) {
    let lots = lots(wishes);
    for (wish, lot) in wishes.iter().zip(lots.iter()) {
        let Some(lot) = lot else {
            continue;
        };
        let img = art(wish);
        let art_h = img.h;
        let base = (lot.ty + lot.th - 1) * TILE;
        let x = lot.tx * TILE + (lot.tw * TILE - img.w) / 2;
        // Firelight only where the art itself burns: flame inks, with a
        // hot core. Banners and gems are signal too, but they do not burn.
        let fires: Vec<(i32, i32)> = wish
            .art
            .iter()
            .enumerate()
            .flat_map(|(y, row)| {
                row.chars()
                    .enumerate()
                    .filter(|(_, c)| FLAME.contains(c))
                    .map(move |(x, _)| (x as i32, y as i32))
            })
            .collect();
        let core = wish.art.iter().any(|row| row.contains(['6', '@']));
        if core && !fires.is_empty() {
            let (sx, sy) = fires
                .iter()
                .fold((0, 0), |(ax, ay), &(px, py)| (ax + px, ay + py));
            let n = fires.len() as i32;
            let breath = 0.92 + 0.08 * (tick as f32 * 0.5 + wish.raised as f32 * 2.3).sin();
            lights.push(Light {
                x: (x + sx / n) as f32,
                y: (base - img.h + sy / n) as f32,
                r: 22.0 + (n as f32).sqrt() * 2.0,
                s: 0.45 * breath,
                fire: true,
            });
        }
        props.push(Prop { x, base, img });
        let plaque = label(wish);
        // Above the roof. The door row is where the party stands.
        props.push(Prop {
            x: lot.tx * TILE + (lot.tw * TILE - plaque.w) / 2,
            base: base - art_h,
            img: plaque,
        });
    }
}

/// The gate drawn: a slate rise with a carved stone arch, steps going down
/// into the dark, and a torch on each side that burns while the Delve is
/// called or on.
pub fn delve_sprite(lit: bool, tick: u32) -> Img {
    let mut im = super::kit::blob(
        40,
        30,
        &[
            (11.0, 19.0, 11.0, 11.0),
            (29.0, 19.0, 11.0, 11.0),
            (20.0, 12.0, 15.0, 11.0),
        ],
        &['x', 'S', 'u', 'U', 'v'],
        0.5,
        29,
        2.0,
    );
    im.outline_inside('s');
    // The arch: dressed stone around a dark mouth, steps going down.
    for y in 11..30 {
        for x in 12..28 {
            let (dx, dy) = ((x as f32 - 19.5) / 8.0, (y as f32 - 19.0) / 8.0);
            let inside_arch = y >= 19 || dx * dx + dy * dy <= 1.0;
            let mouth = (x as f32 - 19.5).abs() <= 5.5
                && (y >= 19 || {
                    let (mx, my) = ((x as f32 - 19.5) / 5.5, (y as f32 - 19.0) / 5.5);
                    mx * mx + my * my <= 1.0
                });
            if mouth {
                // Treads lit a little nearer the top, then only dark.
                let tread = y >= 21 && (y - 21) % 3 == 0;
                im.put(x, y, if tread && y < 28 { 'g' } else { 'k' });
            } else if inside_arch {
                let block = (x + y / 3 * 2) % 4 == 0 || y % 3 == 0;
                im.put(x, y, if block { 'J' } else { 'h' });
            }
        }
    }
    let flame = |k: u32| {
        if !lit {
            'a'
        } else if (tick / 4 + k).is_multiple_of(3) {
            '6'
        } else {
            '@'
        }
    };
    for (x, k) in [(9, 0u32), (30, 1)] {
        im.rect(x, 20, 1, 9, 'n');
        im.put(x, 19, 'b');
        im.put(x, 18, flame(k));
        im.put(x, 17, flame(k + 1));
        if lit {
            im.put(x - 1, 18, '@');
            im.put(x + 1, 18, '@');
        }
    }
    im
}

/// A wish's standard: an iron-shod pole with a teal pennant whose rune
/// catches the light now and then.
fn standard(tick: u32, k: u32) -> Img {
    let mut im = Img::new(5, 14);
    for y in 2..14 {
        im.put(0, y, if y == 13 { 'g' } else { 'B' });
    }
    im.put(0, 1, 'J');
    let glint = (tick / 6 + k * 5).is_multiple_of(9);
    for (x, y, ink) in [
        (1, 2, '1'),
        (2, 2, '1'),
        (3, 2, '1'),
        (4, 2, '0'),
        (1, 3, '1'),
        (2, 3, if glint { '3' } else { '2' }),
        (3, 3, '1'),
        (1, 4, '1'),
        (2, 4, '1'),
        (1, 5, '0'),
    ] {
        im.put(x, y, ink);
    }
    im
}

/// Stage the Delve's gate in realm coordinates, with its torchlight when lit
/// and a standard beside it for each wish the party holds (up to six, on
/// the open ground before the stream).
pub fn stage_gate(
    lit: bool,
    boons: u8,
    props: &mut Vec<Prop>,
    lights: &mut Vec<Light>,
    tick: u32,
) {
    let gate = delve_gate();
    let img = delve_sprite(lit, tick);
    let base = (gate.ty + gate.th - 1) * TILE + TILE - 1;
    let x = gate.tx * TILE + (gate.tw * TILE - img.w) / 2;
    for k in 0..u32::from(boons.min(6)) {
        // A row on the open ground east of the gate, the newest farthest.
        let sx = x + img.w + 3 + k as i32 * 5;
        let flag = standard(tick, k);
        props.push(Prop {
            x: sx,
            base: base - 1,
            img: flag,
        });
    }
    if lit {
        for tx in [9, 30] {
            lights.push(Light {
                x: (x + tx) as f32,
                y: (base - img.h + 18) as f32,
                r: 30.0,
                s: 0.5,
                fire: true,
            });
        }
    }
    props.push(Prop { x, base, img });
}
