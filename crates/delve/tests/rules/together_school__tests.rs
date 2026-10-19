//! The school of the seven banners: ranks, missions, earned gear, and the
//! locks the strong powers sit behind.
use super::*;
use crate::rules::classes::CLASSES;

fn kingdom() -> Run {
    let mut r = Run::at_home(41, 1, Some("Friend"), Default::default(), Default::default());
    r.walk_kingdom(Some(crate::render::overworld::kingdom::keep_stand()));
    r
}

fn archer(mut r: Run) -> Run {
    let class = CLASSES.iter().position(|c| c.name == "Archer").unwrap();
    let hero = r.players.get_mut(&1).unwrap();
    hero.class = Some("Archer".into());
    hero.knight = Some(CLASSES[class].kit.into());
    r
}

#[test]
fn every_class_has_three_missions_one_for_each_step() {
    for class in &CLASSES {
        let missions = MISSIONS.iter().filter(|m| m.class == class.name);
        assert_eq!(missions.clone().count(), 3, "{}", class.name);
        for step in ["Novice", "Adept", "Master"] {
            assert!(
                missions.clone().any(|m| m.step == step),
                "{} has no {step} mission", class.name
            );
        }
    }
    // Nine classes (the seven banners, the Bard and the Peasant), three
    // missions, every one testable: a deed the game counts, and a fixed
    // need above zero.
    assert_eq!(MISSIONS.len(), 27);
    for m in MISSIONS {
        assert!(m.need > 0, "{} need is fixed", m.id);
        assert!(!m.asks.is_empty());
    }
}

#[test]
fn a_squire_is_turned_away_from_a_master_card_and_a_master_takes_it() {
    let mut r = kingdom();
    let vamp = r.book.get("leech-fang").unwrap().clone();
    assert!(Lock::card(&vamp).is_some_and(|(rank, _)| rank == Rank::Master));
    // A Squire cannot take it: the floor keeps it.
    assert!(locked(&r.home.school, &r.players[&1], &vamp));
    // A Master can.
    let mut standing = Standing::default();
    standing.rank = Rank::Master;
    let key = super::Home::school_key(&r.players[&1]);
    r.home.school.knights.insert(key, standing);
    assert!(!locked(&r.home.school, &r.players[&1], &vamp));
}

#[test]
fn a_locked_wish_answers_with_its_rank_and_the_nearest_mission() {
    let mut r = kingdom();
    let hero = &r.players[&1];
    // Blood Drinker is a Master wish: a Squire hears the rank and a mission.
    let note = wish_locked(&r.home.school, hero, "leech", 1);
    assert_eq!(note.as_deref(), Some("needs Master (any Master mission)"));
    // The first tier of a free wish says nothing.
    assert_eq!(wish_locked(&r.home.school, hero, "volley", 1), None);
    // And the grant path says the same thing.
    let book = phrasebook::book();
    let leech = book.iter().find(|w| w.id == "leech").unwrap();
    let answer = r.grant(1, leech).unwrap_err();
    assert!(answer.contains("Master"), "{answer}");
}

#[test]
fn one_finished_mission_per_class_raises_progress() {
    for class in &CLASSES {
        let mut r = archer(kingdom());
        let m = MISSIONS.iter().find(|m| m.class == class.name && m.step == "Novice").unwrap();
        let mut standing = Standing::default();
        standing.deeds.insert(m.id.into(), m.need);
        standing.done.insert(m.id.into());
        standing.rank = standing.earned(class.name);
        assert_eq!(standing.rank, Rank::Adept, "{}", class.name);
        let key = super::Home::school_key(&r.players[&1]);
        r.home.school.knights.insert(key, standing);
        // The next mission's count shows on the board.
        let rows = board_rows(&r.home.school, &r.players[&1], class);
        assert_eq!(rows.len(), 3, "{}", class.name);
    }
}

#[test]
fn the_rank_survives_a_save_and_a_load() {
    let mut r = kingdom();
    let mut standing = Standing::default();
    standing.rank = Rank::Adept;
    r.home.school.knights.insert("friend".into(), standing);
    let saved = serde_json::to_string(&r.home).unwrap();
    let back: crate::rules::home::Home = serde_json::from_str(&saved).unwrap();
    assert_eq!(back.school.standing("friend").rank, Rank::Adept);
}

#[test]
fn an_old_save_loads_with_every_knight_a_squire() {
    // A home with no school field at all, as an old save would carry.
    let old = r#"{"wardrobe":{},"knights":{}}"#;
    let home: crate::rules::home::Home = serde_json::from_str(old).unwrap();
    assert_eq!(home.school.standing("anyone").rank, Rank::Squire);
}

#[test]
fn the_sheet_shows_the_rank_and_the_locks() {
    let mut r = archer(kingdom());
    let mut standing = Standing::default();
    standing.rank = Rank::Adept;
    let key = super::Home::school_key(&r.players[&1]);
    r.home.school.knights.insert(key, standing);
    let text = crate::client::sheet(&r, 1);
    assert!(text.contains("Rank Adept of the Archer"), "{text}");
    assert!(text.contains("Adept"), "{text}");
    assert!(text.contains("LOCKED"), "{text}");
    assert!(text.contains("SCHOOL"), "{text}");
}

#[test]
fn missions_count_only_for_their_own_class() {
    let mut r = archer(kingdom());
    r.school.note(1, "healer-mend", 100);
    r.flush_school();
    let key = super::Home::school_key(&r.players[&1]);
    let standing = r.home.school.standing(&key);
    assert!(standing.done.is_empty(), "an Archer cannot heal a friend");
    // But the Archer's own deed counts, to its fixed need.
    r.school.note(1, "archer-pierce", 1);
    r.flush_school();
    let standing = r.home.school.standing(&key);
    let m = MISSIONS.iter().find(|m| m.id == "archer-novice").unwrap();
    assert_eq!(standing.count(m), 1);
}

#[test]
fn gear_is_checked_by_the_card_checker_and_never_drops() {
    for card in gear_cards() {
        assert_eq!(card.drop, 0, "{}", card.id);
        assert_eq!(card.chest, 0, "{}", card.id);
        assert!(Lock::card(&card).is_none(), "earned gear is never locked");
    }
    assert_eq!(GEAR.len(), 14, "an Adept and a Master piece per class");
}
