//! The peddler's bright little handcart and its three engraved buying plates.
//! Offers and prices are derived from the same private seed as the simulation;
//! no generated art, network, new input bindings or estate furniture is needed.

use super::super::ink::Img;
use super::{TILE, at, home, stand};
use crate::realm::Spoil;
use crate::rules::merchant::{Goods, Sale, cart, plate_at, plates};
use crate::rules::{RoomKind, Run, TILE_UNITS};

const PEDDLER: [&str; 17] = [
    ".....BBBB.....",
    "....B6666B....",
    "...BBBBBBBB...",
    "....OOKOO.....",
    "....OOOOO.....",
    ".....OOO......",
    "....3ww33.....",
    "...33ww333....",
    "..O3333333O...",
    "..O33oo333O...",
    "...33oo333....",
    "...3333333....",
    "....bbbbbb....",
    "....II.II.....",
    "....II.II.....",
    "...nnn.nnn....",
    "...nnn.nnn....",
];

fn mark(goods: Goods) -> Img {
    Img::from_rows(match goods {
        Goods::Potion => &[".OOO.", "..O..", ".OOO.", "OOOOO", ".OOO."],
        Goods::Bomb => &["...O.", "..O..", ".OOO.", "OOOOO", ".OOO."],
        Goods::Scroll => &["OOOOO", "O.O.O", "O..OO", "O.O.O", "OOOOO"],
    })
}

pub(super) fn figures(cv: &mut Img, run: &Run) {
    let Some(camp) = run.merchant_camp().filter(|c| c.room == run.at) else {
        return;
    };
    let kind = run.room().kind;
    let spot = cart(kind);
    let (x, y) = at(spot.0 * TILE_UNITS, spot.1 * TILE_UNITS);
    // Two wooden wheels, red awning, striped cloth, bottles and rolled maps.
    cv.ellipse((x - 23) as f32, (y - 2) as f32, 6.0, 6.0, 'b');
    cv.ellipse((x + 23) as f32, (y - 2) as f32, 6.0, 6.0, 'b');
    cv.ellipse((x - 23) as f32, (y - 2) as f32, 2.0, 2.0, 'O');
    cv.ellipse((x + 23) as f32, (y - 2) as f32, 2.0, 2.0, 'O');
    super::super::chrome::inset_panel(
        cv,
        (x - 28, y - 16, 56, 13),
        super::super::chrome::Tone::TIMBER.rimmed(super::super::chrome::rim_of('O')),
    );
    cv.line(x - 30, y - 30, x - 30, y - 3, 'B');
    cv.line(x + 30, y - 30, x + 30, y - 3, 'B');
    for i in 0..8 {
        cv.rect(
            x - 32 + i * 8,
            y - 32,
            8,
            6,
            if i % 2 == 0 { '7' } else { 'h' },
        );
    }
    cv.stamp(&mark(Goods::Potion).recolor(&[('O', '7')]), x - 18, y - 22);
    cv.stamp(&mark(Goods::Scroll).recolor(&[('O', 'T')]), x + 6, y - 22);
    stand(cv, &Img::from_rows(&PEDDLER), x - 43, y + 9);
    for (index, offer) in camp.offers.iter().enumerate() {
        let (c, r) = plates(kind)[index];
        let lit = run
            .players
            .values()
            .any(|h| h.hp > 0 && !h.stone && plate_at(kind, h.x, h.y, camp.offers.len()) == Some(index));
        home::plate_marked(cv, (c, r, 1, 1), lit, &mark(offer.goods));
        cv.text(
            c * TILE,
            (r + 1) * TILE + 2,
            &format!("{}G", offer.gold),
            '6',
        );
    }
    // In the Delve's courtyard the plates are the bazaar out of doors: its
    // name on a board over them.
    if kind == RoomKind::Gate {
        let name = "BAZAAR";
        let row = plates(kind);
        let (left, top) = row[0];
        let right = row[row.len() - 1].0 + 1;
        let w = super::super::ink::text_width(name);
        let x = (left + right) * TILE / 2 - w / 2;
        super::super::chrome::label(cv, (x, top * TILE - 12), name, 'O', super::super::chrome::Tone::TIMBER);
    }
}

/// The peddler's board as the seat playing `focus` reads it (`None`: a
/// screen that plays no one knight), or nothing. A knight sees their own
/// plate, purse and purchase only: a friend buying at the next plate shows
/// on the friend's screen, not over this one (2026-10-19: "buying a potion
/// should only show you your own popup").
pub(super) fn board_lines(run: &Run, focus: Option<u32>) -> Option<Vec<(String, char)>> {
    let camp = run.merchant_camp().filter(|c| c.room == run.at)?;
    let unsafe_room = !run.room().cleared || run.enemies.iter().any(|e| e.hp > 0);
    // On the surface nothing is carried: the realm's treasury pays.
    let surface = run.dungeon.depth == 0;
    let purse = if surface { "TREASURY" } else { "CARRIED" };
    let reader = super::shoppers(run, focus)
        .filter(|(_, h)| h.hp > 0 && !h.stone)
        .find_map(|(id, h)| plate_at(run.room().kind, h.x, h.y, camp.offers.len()).map(|index| (id, h, index)));
    let lines = if let Some((id, hero, index)) = reader {
        let offer = camp.offers[index];
        let have = if surface {
            run.treasury.get(Spoil::Gold)
        } else {
            hero.carried.get(Spoil::Gold)
        };
        // The seat's own approach travels in the Live packet with the rest
        // of the run; a guest's frame reads the host's word on it.
        let sale = (id as usize)
            .checked_sub(1)
            .and_then(|seat| run.merchant.approaches.get(seat))
            .filter(|a| a.spent && a.offer == Some(index))
            .and_then(|a| a.sale);
        let status = match sale {
            _ if unsafe_room => "WAIT UNTIL THE ROOM IS SAFE",
            Some(Sale::Bought) => "BOUGHT - STEP AWAY TO BUY AGAIN",
            Some(Sale::Owed) => "THE TREASURY IS PAYING",
            Some(Sale::Full) => "PACK FULL - NO GOLD SPENT",
            Some(Sale::Short) if surface => "NOT ENOUGH TREASURY GOLD",
            Some(Sale::Short) => "NOT ENOUGH CARRIED GOLD",
            None => "STAND STILL FOR ONE SECOND TO BUY",
        };
        vec![
            (format!("{} THE PEDDLER", camp.name), '9'),
            (offer.goods.name().into(), '6'),
            (offer.goods.description().into(), 'h'),
            (
                format!("PRICE: {} {purse} GOLD", offer.gold),
                if have >= offer.gold { 'H' } else { '7' },
            ),
            (
                if surface {
                    format!("TREASURY: {have} GOLD")
                } else {
                    format!("YOUR PURSE: {have} GOLD")
                },
                'h',
            ),
            (status.into(), '6'),
        ]
    } else {
        // The price list is for knights at the cart, not the whole room.
        let spot = cart(run.room().kind);
        let at_cart = super::shoppers(run, focus).any(|(_, h)| {
            h.hp > 0
                && (h.x - spot.0 * TILE_UNITS).hypot(h.y - spot.1 * TILE_UNITS)
                    < 4.0 * TILE_UNITS
        });
        if !at_cart {
            return None;
        }
        let mut lines = vec![(format!("{} THE PEDDLER", camp.name), '9')];
        for offer in &camp.offers {
            lines.push((format!("{} - {} GOLD", offer.goods.name(), offer.gold), '6'));
        }
        lines.push(("STAND ON A MARKED PLATE TO BUY".into(), 'h'));
        lines.push((
            if surface {
                "PAID FROM THE TREASURY".into()
            } else {
                "PAID FROM YOUR CARRIED GOLD".into()
            },
            'h',
        ));
        lines
    };
    Some(lines)
}

pub(super) fn boards(cv: &mut Img, run: &Run, focus: Option<u32>) {
    let Some(lines) = board_lines(run, focus) else {
        return;
    };
    let board = home::board(&lines);
    // At the top, clear of the offer plates and approaching knights.
    cv.stamp(&board, (cv.w - board.w) / 2, 3 * TILE);
}
