//! Cards drawn in realm ink: a card's own art, and the small plate a card
//! makes lying on a dungeon floor.

use super::super::ink::Img;
use crate::drive::together_shooter::cards::{Card, Rarity};

/// A card's art as authored (or its kind's glyph), on black paper.
pub(crate) fn card_art(card: &Card) -> Img {
    Img::from_rows(&card.art_rows())
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
pub(crate) fn floor_card(card: Option<&Card>, tick: u32) -> Img {
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
