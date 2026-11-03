//! The party's three cards on the pickup cards' black paper.
use super::*;
use crate::rules::HZ;
use crate::rules::crossroads::{self as rules, Game, Stage};

fn centred(im: &mut Img, y: i32, text: &str, ink: char) {
    im.text((im.w - super::super::ink::text_width(text)) / 2, y, text, ink);
}

pub(super) fn showing(run: &Run) -> bool {
    run.crossroads.as_ref().is_some_and(|c| c.stage != Stage::Playing)
}

pub(super) fn pastime(run: &Run) -> bool {
    run.crossroads.as_ref().is_some_and(|c| c.stage == Stage::Playing
        && c.chosen.is_some_and(|i| matches!(c.choices[usize::from(i)], Game::Powder | Game::Gander)))
}

pub(super) fn frame(run: &Run, w: i32, h: i32, focus: Option<u32>) -> Img {
    let c = run.crossroads.as_ref().unwrap();
    let mut im = Img::black(NATIVE_W, NATIVE_H);
    if c.stage == Stage::Results {
        let game = c.choices[usize::from(c.chosen.unwrap())];
        centred(&mut im, 20, game.name(), 'h');
        centred(&mut im, 37, "THE CROSSROADS PURSE", 'J');
        for (place, &id) in c.order.iter().enumerate() {
            let name = run.players.get(&id).map_or("KNIGHT", |h| h.name.as_str());
            let name: String = name.chars().take(18).collect();
            let text = format!("{}  {}  {} GOLD{}", place + 1, name, rules::gold(place), if place == 0 { " + WISH" } else { "" });
            centred(&mut im, 64 + place as i32 * 17, &text.to_uppercase(), 'h');
        }
        centred(&mut im, 198, &format!("FLOOR {} IN {}S", c.depth, c.left.div_ceil(HZ)), 'J');
    } else {
        centred(&mut im, 10, "THE CROSSROADS", 'h');
        centred(&mut im, 25, &format!("FLOOR {} CLEARED   EVERY KNIGHT PLAYS", run.dungeon.depth), 'J');
        centred(&mut im, 39, &format!("CHOOSE A CARD   {}S", c.left.div_ceil(HZ)), 'h');
        let counts = c.counts();
        for (i, game) in c.choices.iter().enumerate() {
            let picked = focus.and_then(|id| c.votes.get(&id)).is_some_and(|&v| usize::from(v) == i);
            let selected = focus.and_then(|id| run.playtest.seats.get(&id)).is_some_and(|s| s.selected.min(2) == i);
            let mut card = pickup_cards::paper(120, 152, false);
            centred(&mut card, 12, &format!("{}{}", i + 1, if picked { " YOUR VOTE" } else if selected { " <" } else { "" }), 'h');
            centred(&mut card, 29, game.name(), 'h');
            centred(&mut card, 47, game.words(), 'J');
            // The prize uses the same grey drawing as a gold pickup.
            if let Some((pw, ph, gold)) = pickup_cards::picture(16) {
                let fit = 24.0 / (*pw).max(*ph) as f32;
                let (iw, ih) = ((*pw as f32 * fit) as u32, (*ph as f32 * fit) as u32);
                for y in 0..ih {
                    for x in 0..iw {
                        let at = (((y * ph / ih) * pw + x * pw / iw) * 4) as usize;
                        if gold[at + 3] >= 128 {
                            card.set((120 - iw as i32) / 2 + x as i32, 61 + y as i32, [gold[at], gold[at + 1], gold[at + 2]]);
                        }
                    }
                }
            }
            centred(&mut card, 90, "100 / 60 / 30 GOLD", 'h');
            centred(&mut card, 102, "FIRST: EXTRA WISH", 'J');
            centred(&mut card, 118, &format!("{} VOTE{}", counts[i], if counts[i] == 1 { "" } else { "S" }), 'h');
            let voters = c.seats.iter().filter(|id| c.votes.get(id) == Some(&(i as u8)))
                .map(|id| format!("P{id}")).collect::<Vec<_>>().join(" ");
            centred(&mut card, 132, &voters, 'J');
            im.stamp(&card, 8 + i as i32 * 124, 54);
        }
        centred(&mut im, 214, "1 2 3 PICK   ARROWS + F CHOOSE   TIES: LEFT CARD", 'J');
    }
    let mut out = Img::black(w.max(0), h.max(0));
    Field::fit(w, h).blit(&im, &Img::new(NATIVE_W, NATIVE_H), &mut out);
    out
}
