//! Cards drawn in realm ink: a card's own art, and the small plate a card
//! makes lying on a dungeon floor.

use super::super::ink::Img;
use crate::rules::cards::{Card, Rarity};

#[cfg(test)]
mod pickup_3d_tests {
    use super::*;

    #[test]
    fn pickup_3d_turns_lit_surfaces_instead_of_a_flat_glint() {
        let a = floor_card(None, 0);
        let b = floor_card(None, 45);
        let changed = a.rgba_bytes().chunks_exact(4)
            .zip(b.rgba_bytes().chunks_exact(4))
            .filter(|(a, b)| a != b).count();
        assert!(changed > 30, "only {changed} pixels changed: pickup is still a flat plate");
        assert_eq!(a.rgba_bytes(), floor_card(None, 0).rgba_bytes());
        assert!(a.words().iter().any(|p| p >> 24 == 0));
    }
}

/// A card's art as authored (or its kind's glyph), on black paper.
pub fn card_art(card: &Card) -> Img {
    super::fx::icon(&card.art_rows())
}

/// A physical pickup, projected through the same lit-solid sampler as the
/// stage's procedural 3D objects. Card art remains available in the inventory;
/// the floor silhouette identifies the item's role, with rarity in its material.
pub(super) fn pickup_material(card: Option<&Card>) -> (u32, [u8; 3]) {
    use crate::rules::cards::Kind;
    let (shape, material): (u32, [u8; 3]) = match card {
        Some(c) if c.id == "gold" => (3, [238, 184, 62]),
        Some(c) if c.id.contains("heart") || c.id.contains("heal") || c.id == "potion" => (1, [228, 78, 88]),
        Some(c) if c.id == "gem" || c.id.contains("mana") => (6, [80, 166, 238]),
        Some(c) => {
            let shape = match c.kind {
                Kind::Take => 1, // supplies: rounded vessels
                Kind::Arm => 2, // arms: a pointed solid
                Kind::Guard => 0, // protection: a sturdy block
                Kind::Hold => 5, // lasting equipment: a ring
                Kind::Play => 0, // a carried card-box
                Kind::Spell => 6, // spells: a faceted crystal
            };
            let material = match c.rarity {
                Rarity::Relic => [206, 132, 244],
                Rarity::Rare => [90, 194, 231],
                _ => [218, 185, 119],
            };
            (shape, material)
        }
        None => (0, [218, 185, 119]),
    };
    (shape, material)
}

/// A card lying on the floor at `(x, y)`, turning (`pickup_cards`): its own
/// art on its face, in greys; a card the book does not know shows its back.
pub fn turning_card(card: Option<&Card>, (x, y): (i32, i32), tick: u32) -> Img {
    let art = card.map(card_art);
    let shine = card.is_some_and(|c| super::pickup_meshes::recipe(c).shine);
    super::pickup_cards::turning_sprite(art.as_ref(), shine, (x as f32, y as f32), tick)
}

pub fn floor_card(card: Option<&Card>, tick: u32) -> Img {
    let (shape, material) = pickup_material(card);
    pickup_image(shape, material, tick as f32 * 0.025)
}

pub(super) fn pickup_image(shape: u32, material: [u8; 3], phase: f32) -> Img {
    const SIZE: i32 = 20;
    let mut im = Img::new(SIZE, SIZE + 3);
    // A grounded contact shadow separates the hovering solid from the floor.
    for y in 18..21 {
        for x in 4..16 {
            let dx = (x as f32 - 9.5) / 6.0;
            let dy = (y as f32 - 19.0) / 1.6;
            if dx * dx + dy * dy < 1.0 {
                im.set(x, y, [24, 22, 30]);
            }
        }
    }
    for y in 0..SIZE {
        for x in 0..SIZE {
            let shade = angel_stage::primitive::sample(
                shape, phase,
                (x as f32 + 0.5) / SIZE as f32 * 2.8 - 1.4,
                1.4 - (y as f32 + 0.5) / SIZE as f32 * 2.8,
            );
            if shade > 0.0 {
                im.set(x, y, material.map(|c| (f32::from(c) * shade).round() as u8));
            }
        }
    }
    im
}

/// Rim and glint inks by rarity. A card on the floor is live state, so its
/// rim may use the signal bank.
fn rim(rarity: Option<Rarity>) -> (char, char) {
    match rarity {
        Some(Rarity::Relic) => ('6', 'w'),
        Some(Rarity::Rare) => ('2', '3'),
        _ => ('h', 'H'),
    }
}

/// A card lying face up: a dark plate rimmed by rarity with the art shrunk
/// to fit, a glint running round the rim. Unknown cards show a blank back.
/// The floor shows a solid now (`floor_card`); the plate's palette inks are
/// what the 3D pickups take their colours from (`pickup_meshes::recipe`).
pub(super) fn floor_plate(card: Option<&Card>, tick: u32) -> Img {
    const W: i32 = 14;
    const H: i32 = 18;
    let (rim, glint) = rim(card.map(|c| c.rarity));
    let mut im = Img::new(W, H);
    im.rect(1, 1, W - 2, H - 2, 'K');
    im.frame(0, 0, W, H, rim);
    for (x, y) in [(0, 0), (W - 1, 0), (0, H - 1), (W - 1, H - 1)] {
        im.clear(x, y);
    }
    if let Some(card) = card {
        let art = card_art(card);
        let fit = 12.0 / art.w.max(art.h).max(12) as f32;
        let (w, h) = (
            (art.w as f32 * fit).round() as i32,
            (art.h as f32 * fit).round() as i32,
        );
        let (ox, oy) = ((W - w) / 2, (H - h) / 2);
        for y in 0..h {
            for x in 0..w {
                if let Some(c) = art.get((x as f32 / fit) as i32, (y as f32 / fit) as i32) {
                    im.set(ox + x, oy + y, c);
                }
            }
        }
    }
    // The glint walks the rim once every two seconds.
    let perimeter = 2 * (W + H) - 4;
    let step = ((tick / 2) % 30) as i32;
    if step < perimeter / 2 {
        let p = step * 2;
        let (x, y) = if p < W {
            (p, 0)
        } else if p < W + H - 1 {
            (W - 1, p - W + 1)
        } else {
            (W - 1 - (p - W - H + 2), H - 1)
        };
        if im.get(x, y).is_some() {
            im.put(x, y, glint);
        }
    }
    im
}
