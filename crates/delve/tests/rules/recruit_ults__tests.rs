//! The recruits' ultimates: the first knight to take a recruit up is
//! congratulated and handed a wish, their scroll asks for the recruit's
//! ultimate and nothing else, and what they wish is the recruit's from then
//! on, for whoever takes them up, down every stair and through a save.
use super::*;
use crate::rules::{alehouse, phrasebook, ults::Ult};

fn tavern(seats: u32) -> Run {
    let mut r = Run::at_home(41, 1, Some("Friend"), Default::default(), Default::default());
    for id in 3..=seats {
        r.join_seat(id, &format!("Seat {id}"));
    }
    r.players.retain(|&id, _| id <= seats.max(1));
    r.walk_alehouse();
    r
}

/// The first recruit at a table, and one of the old company.
fn patrons() -> (usize, usize) {
    let new = alehouse::PATRONS.iter().position(|p| recruit(p.kit)).expect("a recruit sits in the tavern");
    let old = alehouse::PATRONS.iter().position(|p| !recruit(p.kit)).expect("the old company sits there too");
    (new, old)
}

fn said(r: &Run, id: u32) -> String {
    r.alehouse.said.get(&id).map(|s| s.text.clone()).unwrap_or_default()
}

fn wish(r: &mut Run, id: u32, word: &str) -> Result<String, String> {
    let w = phrasebook::get(word).unwrap_or_else(|| panic!("{word}"));
    r.wish(id, &w)
}

#[test]
fn the_old_company_carry_their_own_and_the_recruits_none() {
    let owned: Vec<&str> = knights::COMPANY.iter().map(|k| k.id).take_while(|id| Ult::own(id).is_some()).collect();
    assert_eq!(owned.len(), 24, "the company before the recruits");
    let ults: std::collections::BTreeSet<&str> = owned.iter().map(|id| Ult::own(id).unwrap().word()).collect();
    assert_eq!(ults.len(), owned.len(), "each their own");
    // The code-drawn knights came later too, but they are plain knights.
    for k in knights::COMPANY[owned.len()..].iter().filter(|k| knights::plain_seat(k.id).is_none()) {
        assert!(recruit(k.id), "{} came after the company and has none of their own", k.id);
    }
    for k in knights::COMPANY.iter().filter(|k| knights::plain_seat(k.id).is_some()) {
        assert!(!recruit(k.id), "{} is a plain knight, not a recruit", k.id);
    }
    assert_eq!(Ult::of(None), Ult::PilgrimsArrow, "a new player is still Sir Percival");
}

#[test]
fn the_first_to_recruit_is_congratulated_and_handed_a_wish_for_its_ultimate() {
    let (recruit, _) = patrons();
    let kit = alehouse::PATRONS[recruit].kit;
    let mut r = tavern(1);
    let wishes = r.players[&1].wishes;
    r.pick_patron(1, recruit);
    let h = &r.players[&1];
    assert_eq!(h.knight.as_deref(), Some(kit));
    assert!(said(&r, 1).contains("FIRST TO RECRUIT"), "{}", said(&r, 1));
    assert!(r.cues.iter().any(|c| c == &format!("recruit_first:{kit}")));
    assert_eq!(h.wishes, wishes + 1, "a wish to spend on it");
    let m = h.making(&r.book).expect("the scroll has a question");
    assert_eq!(m.heading(), Some(question(kit)));
    assert!(m.brief().contains("first knight to recruit"), "{}", m.brief());
    assert_eq!(m.asks().first(), Some(&identity::Ask::Ultimate));
}

#[test]
fn the_scroll_grants_only_the_ultimate_and_it_stays_the_recruits() {
    let (recruit, old) = patrons();
    let kit = alehouse::PATRONS[recruit].kit;
    let mut r = tavern(2);
    r.pick_patron(1, recruit);
    let held = r.players[&1].wishes;
    let refused = wish(&mut r, 1, "volley").unwrap_err();
    assert!(refused.contains("recruit's ultimate"), "{refused}");
    assert_eq!(r.players[&1].wishes, held, "a refused wish is kept");

    wish(&mut r, 1, "trebuchet").unwrap();
    let h = &r.players[&1];
    assert_eq!(h.wishes, held - 1);
    assert_eq!(h.ult(), Ult::Trebuchet);
    assert!(h.recruit_ult.is_none() && h.making(&r.book).is_none(), "the question is answered");
    assert!(!h.deck.iter().any(|c| c.starts_with("wish-trebuchet")), "the card is the recruit's, not the wisher's");
    let kept = &r.home.recruit_ults[kit];
    assert_eq!((kept.ult, kept.by.as_str()), (Ult::Trebuchet, h.name.as_str()));
    assert!(r.cues.iter().any(|c| c == &format!("recruit_ult:{kit}:trebuchet")));

    // The first knight moves on; a friend takes the recruit up and casts it,
    // with no congratulation and no wish of their own.
    r.pick_patron(1, old);
    assert_eq!(r.players[&1].ult(), Ult::of(Some(alehouse::PATRONS[old].kit)));
    let wishes = r.players[&2].wishes;
    r.pick_patron(2, recruit);
    assert!(!said(&r, 2).contains("FIRST"), "{}", said(&r, 2));
    assert_eq!(r.players[&2].wishes, wishes);
    assert!(r.players[&2].making(&r.book).is_none());
    assert_eq!(r.players[&2].ult(), Ult::Trebuchet);

    // Down the stair, and through a save.
    r.descend();
    assert_eq!(r.players[&2].ult(), Ult::Trebuchet, "dealt again on the way down");
    let home: home::Home = serde_json::from_value(serde_json::to_value(&r.home).unwrap()).unwrap();
    assert_eq!(home.recruit_ults[kit].ult, Ult::Trebuchet);
}

#[test]
fn walking_away_lets_the_question_go_and_picking_again_gives_no_second_wish() {
    let (recruit, old) = patrons();
    let mut r = tavern(1);
    r.pick_patron(1, recruit);
    let wishes = r.players[&1].wishes;
    r.pick_patron(1, old);
    assert!(r.players[&1].making(&r.book).is_none(), "the question goes with the recruit");
    assert!(wish(&mut r, 1, "volley").is_ok(), "an ordinary wish again");
    r.pick_patron(1, recruit);
    assert!(said(&r, 1).contains("FIRST TO RECRUIT"), "still unwished, still theirs to wish");
    assert_eq!(r.players[&1].wishes, wishes - 1, "but no second wish for it");
}

#[test]
fn the_old_company_need_no_wish() {
    let (_, old) = patrons();
    let mut r = tavern(1);
    let wishes = r.players[&1].wishes;
    r.pick_patron(1, old);
    assert!(!said(&r, 1).contains("FIRST"));
    assert_eq!(r.players[&1].wishes, wishes);
    assert!(r.players[&1].making(&r.book).is_none());
}

#[test]
fn a_home_without_recruit_ultimates_saves_as_before() {
    let home = home::Home::default();
    assert!(!serde_json::to_string(&home).unwrap().contains("recruit_ults"));
}
