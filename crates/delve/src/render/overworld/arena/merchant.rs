//! The peddler's bright little handcart and its three engraved buying plates.
//! Offers and prices are derived from the same private seed as the simulation;
//! no generated art, network, new input bindings or estate furniture is needed.

use super::super::ink::Img;
use super::{TILE, at, home, stand};
use crate::realm::Spoil;
use crate::rules::merchant::{CART_AT, Goods, PLATES, plate_at};
use crate::rules::{Run, TILE_UNITS};

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
    let (x, y) = at(CART_AT.0 * TILE_UNITS, CART_AT.1 * TILE_UNITS);
    // Two wooden wheels, red awning, striped cloth, bottles and rolled maps.
    cv.ellipse((x - 23) as f32, (y - 2) as f32, 6.0, 6.0, 'b');
    cv.ellipse((x + 23) as f32, (y - 2) as f32, 6.0, 6.0, 'b');
    cv.ellipse((x - 23) as f32, (y - 2) as f32, 2.0, 2.0, 'O');
    cv.ellipse((x + 23) as f32, (y - 2) as f32, 2.0, 2.0, 'O');
    cv.rect(x - 28, y - 16, 56, 13, 'B');
    cv.frame(x - 28, y - 16, 56, 13, 'O');
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
        let (c, r) = PLATES[index];
        let lit = run
            .players
            .values()
            .any(|h| h.hp > 0 && !h.stone && plate_at(h.x, h.y, camp.offers.len()) == Some(index));
        home::plate_marked(cv, (c, r, 1, 1), lit, &mark(offer.goods));
        cv.text(
            c * TILE,
            (r + 1) * TILE + 2,
            &format!("{}G", offer.gold),
            '6',
        );
    }
}

pub(super) fn boards(cv: &mut Img, run: &Run) {
    let Some(camp) = run.merchant_camp().filter(|c| c.room == run.at) else {
        return;
    };
    let unsafe_room = !run.room().cleared || run.enemies.iter().any(|e| e.hp > 0);
    // On the surface nothing is carried: the realm's treasury pays.
    let surface = run.dungeon.depth == 0;
    let purse = if surface { "TREASURY" } else { "CARRIED" };
    let reader = run
        .players
        .iter()
        .filter(|(_, h)| h.hp > 0 && !h.stone)
        .find_map(|(&id, h)| plate_at(h.x, h.y, camp.offers.len()).map(|index| (id, h, index)));
    let lines = if let Some((id, hero, index)) = reader {
        let offer = camp.offers[index];
        let have = if surface {
            run.treasury.get(Spoil::Gold)
        } else {
            hero.carried.get(Spoil::Gold)
        };
        let last = run.found.as_ref().filter(|(when, who, text)| {
            *who == id
                && text.starts_with(&format!("{}:", camp.name))
                && run.tick.saturating_sub(*when)
                    < u64::from(5 * crate::rules::HZ)
        });
        // found travels in the existing Live packet; do not rely on local
        // approach timers when drawing a guest's authoritative host frame.
        let status = match last {
            _ if unsafe_room => "WAIT UNTIL THE ROOM IS SAFE",
            Some((_, _, text)) if text.contains("bought") => "BOUGHT - STEP AWAY TO BUY AGAIN",
            Some((_, _, text)) if text.contains("full") => "PACK FULL - NO GOLD SPENT",
            Some(_) if surface => "NOT ENOUGH TREASURY GOLD",
            Some(_) => "NOT ENOUGH CARRIED GOLD",
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
        if !super::knight_near(run, CART_AT, 4.0) {
            return;
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
    let board = home::board(&lines);
    // At the top, clear of the offer plates and approaching knights.
    cv.stamp(&board, (cv.w - board.w) / 2, 3 * TILE);
}
