//! Records the wish bookkeeping, the book and the small card predicates for
//! the Bend port (`delve/rules/home/overclass.bendx`, `cards.bend`,
//! `dead_wishes_gifts.bendx`).
use super::{wish_kept, wish_returned};
use crate::rules::cards::{self, Bonus, Card};
use crate::rules::dead_wishes::Gifts;
use crate::rules::home::bend_cases::{Cases, Mix, keep, state, states, to};
use crate::rules::Run;
use serde_json::json;

fn view(run: &Run) -> serde_json::Value {
    keep(&state(run), &["tick", "players", "window", "cues", "makers", "seat_notices", "dungeon"])
}

#[test]
#[ignore]
fn record_wishes() {
    let mut m = Mix(0x0c1a55);
    let mut give = Cases::new("overclass.Run.give_wishes");
    let mut can = Cases::new("overclass.Run.can_wish");
    let mut spend = Cases::new("overclass.Run.spend_wish_kind");
    let mut tell = Cases::new("overclass.Run.tell");
    let mut refund = Cases::new("overclass.Run.refund_told");
    let mut newcomer = Cases::new("overclass.Run.wishes_for_newcomer");
    let mut made = Cases::new("overclass.Run.made");
    let mut returned = Cases::new("overclass.wish_returned");
    let mut kept = Cases::new("overclass.wish_kept");
    for (_, run) in states(3) {
        let ids: Vec<u32> = run.players.keys().copied().collect();
        for _ in 0..3 {
            let mut run = run.clone();
            for hero in run.players.values_mut() {
                hero.wishes = *m.pick(&[0, 1, 2, 5, 9, 250]);
                hero.great_wishes = *m.pick(&[0, 0, 1, 3, 9, 255]);
                hero.hp = *m.pick(&[0, 10, hero.max_hp]);
                hero.absent = m.chance(10);
            }
            if m.chance(40) {
                run.window = Some(super::Window { opened: 5, wished: vec![1], given: 2, great: false });
            }
            let id = if m.chance(5) { 99 } else { ids[m.below(ids.len() as u64) as usize] };
            let before = view(&run);
            let (plain, great) = (*m.pick(&[0u8, 1, 2, 3, 250]), *m.pick(&[0u8, 1, 1, 4]));
            let mut after = run.clone();
            after.give_wishes(plain, great);
            give.add(json!({"run": before, "plain": plain, "great": great}), view(&after));
            can.add(json!({"run": before, "id": id}), json!(run.can_wish(id)));
            let g = m.chance(50);
            let mut after = run.clone();
            let spent = after.spend_wish_kind(id, g);
            spend.add(json!({"run": before, "id": id, "great": g}), json!({"run": view(&after), "spent": spent}));
            let line = (*m.pick(&["a line", "", "another"])).to_string();
            let mut after = run.clone();
            for _ in 0..m.below(10) {
                after.tell(id, "x");
            }
            let b = view(&after);
            after.tell(id, line.clone());
            tell.add(json!({"run": b, "id": id, "line": line}), view(&after));
            let why = *m.pick(&["", "it did not go through", " (a lock). ", "the book is full", "...(x)..."]);
            let mut after = run.clone();
            after.refund_told(id, g, why);
            refund.add(json!({"run": before, "id": id, "great": g, "why": why}), view(&after));
            let mut after = run.clone();
            after.wishes_for_newcomer(id);
            newcomer.add(json!({"run": before, "id": id}), view(&after));
            let (wish, name) = (*m.pick(&["golden-grace", "bow"]), *m.pick(&["", "  ", "Ada", " Bors "]));
            let mut after = run.clone();
            after.made(id, wish, name);
            made.add(json!({"run": before, "id": id, "wish": wish, "name": name}), view(&after));
            returned.add(json!({"why": why}), json!(wish_returned(why)));
            kept.add(json!({"why": why}), json!(wish_kept(why)));
        }
    }
    for c in [&give, &can, &spend, &tell, &refund, &newcomer, &made, &returned, &kept] {
        c.save();
    }
}

#[test]
#[ignore]
fn record_gifts_and_book() {
    let mut m = Mix(0x61f7);
    let mut hit = Cases::new("dead_wishes.Gifts.hit");
    let mut rest = Cases::new("dead_wishes.Gifts.rest");
    let mut pace = Cases::new("dead_wishes.Gifts.pace");
    let mut taken = Cases::new("dead_wishes.Gifts.taken");
    for _ in 0..80 {
        let bonus = Bonus { damage: m.below(41) as u32, rate: m.below(31) as u32, speed: m.below(26) as u32, armor: m.below(3) as u32, ..Bonus::default() };
        let g = Gifts { bonus, hp: m.below(50) as u32 };
        let w = *m.pick(&[0u32, 1, 7, 30, 99, 1000]);
        hit.add(json!({"g": to(&g), "weight": w}), json!(g.hit(w)));
        rest.add(json!({"g": to(&g), "ticks": w}), json!(g.rest(w)));
        pace.add(json!({"g": to(&g)}), json!(g.pace()));
        taken.add(json!({"g": to(&g), "damage": w}), json!(g.taken(w)));
    }
    for c in [&hit, &rest, &pace, &taken] {
        c.save();
    }
    let book = cards::Book::builtin();
    let mut get = Cases::new("cards.Book.get");
    let mut insert = Cases::new("cards.Book.insert");
    let mut spoil = Cases::new("cards.Card.is_spoil");
    let mut drinks = Cases::new("cards.Card.drinks");
    let mut scroll = Cases::new("cards.Card.scroll");
    for (i, card) in book.cards.iter().enumerate() {
        if i % 5 == 0 {
            get.add(json!({"book": to(&book), "id": card.id}), to(card));
        }
        spoil.add(json!({"card": to(card)}), json!(card.is_spoil()));
        drinks.add(json!({"card": to(card)}), json!(card.drinks()));
        scroll.add(json!({"card": to(card)}), json!(card.scroll()));
    }
    get.add(json!({"book": to(&book), "id": "no-such-card"}), json!(null));
    for round in 0..40 {
        let mut b = book.clone();
        let mut card: Card = b.cards[m.below(b.cards.len() as u64) as usize].clone();
        match round % 4 {
            0 => card.name = format!("{} again", card.name),
            1 => card.id = format!("zz-new-{round}"),
            2 => card.id = format!("aa-new-{round}"),
            _ => {}
        }
        if round == 39 {
            while b.cards.len() < cards::MAX_BOOK {
                let mut filler = card.clone();
                filler.id = format!("filler-{}", b.cards.len());
                b.insert(filler);
            }
            card.id = "one-too-many".into();
        }
        let before = to(&b);
        let ok = b.insert(card.clone());
        insert.add(json!({"book": before, "card": to(&card)}), json!({"book": to(&b), "ok": ok}));
    }
    for c in [&get, &insert, &spoil, &drinks, &scroll] {
        c.save();
    }
}
