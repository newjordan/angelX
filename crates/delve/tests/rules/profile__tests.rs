use super::super::home::Home;
use super::super::home_settlement::{HomeTransfer, settle};
use super::*;

fn party() -> Run {
    let mut r = Run::at_home(3, 2, Some("Jordan"), Default::default(), Default::default());
    r.players.get_mut(&1).unwrap().name = "Frank".into();
    r.players.get_mut(&2).unwrap().name = "Jordan".into();
    for hero in r.players.values_mut() {
        hero.knight = Some("percival".into());
    }
    r
}

fn transfer(r: &mut Run) -> HomeTransfer {
    let transfer = HomeTransfer::capture_and_reserve(r).unwrap();
    r.squire_company_reserved = false;
    r.squire_company_claimed = false;
    transfer
}

fn settled(r: &mut Run, home: &mut Home) {
    let mut treasury = Spoils::default();
    let transfer = transfer(r);
    settle(
        &mut super::super::activity::Realm {
            home,
            treasury: &mut treasury,
            reclaimed: &Default::default(),
            raids_won: 0,
        },
        &transfer,
        "Host",
        false,
    )
    .unwrap();
}

#[test]
fn friends_on_one_kit_keep_their_own_prowess() {
    let mut r = party();
    r.marks.insert("xp".into(), 700);
    let mut home = Home::default();
    home.knights.insert("frank".into(), Prowess { xp: 100, learned: Vec::new() });
    settled(&mut r, &mut home);
    assert_eq!(home.prowess("frank").xp, 800);
    assert_eq!(home.prowess("jordan").xp, 700);
    assert!(!home.knights.contains_key("percival"));
}

#[test]
fn a_change_of_class_keeps_the_players_record() {
    let mut r = party();
    r.home.knights.insert("frank".into(), Prowess { xp: 1900, learned: vec![0] });
    assert_eq!(r.home.prowess(&profile_key(&r.players[&1])).level(), 3);
    r.players.get_mut(&1).unwrap().knight = Some("lynette".into());
    assert_eq!(r.home.prowess(&profile_key(&r.players[&1])).level(), 3);
    assert_ne!(knight_key(&r.players[&1]), profile_key(&r.players[&1]));
}

#[test]
fn an_old_kit_record_goes_to_the_first_player_on_it_and_no_further() {
    let mut home = Home::default();
    home.knights.insert("percival".into(), Prowess { xp: 3700, learned: vec![1, 0] });
    home.questlines.knights.insert("percival".into(), Default::default());
    assert!(home.adopt("percival", "frank"));
    assert_eq!(home.prowess("frank").xp, 3700);
    assert!(home.questlines.knights.contains_key("frank"));
    assert!(!home.knights.contains_key("percival"));
    assert!(!home.adopt("percival", "jordan"), "the record has moved");
    assert_eq!(home.prowess("jordan").xp, 0);
    home.knights.insert("percival".into(), Prowess { xp: 50, learned: Vec::new() });
    assert!(!home.adopt("percival", "frank"), "a player's own record stands");
    assert_eq!(home.prowess("frank").xp, 3700);
}

#[test]
fn settling_an_old_realm_moves_the_record_before_it_pays_experience() {
    let mut r = party();
    r.players.remove(&2);
    r.marks.insert("xp".into(), 100);
    let mut home = Home::default();
    home.knights.insert("percival".into(), Prowess { xp: 3700, learned: vec![1] });
    settled(&mut r, &mut home);
    assert_eq!(home.prowess("frank"), Prowess { xp: 3800, learned: vec![1] });
    assert!(!home.knights.contains_key("percival"));
}

#[test]
fn a_kit_id_that_is_also_the_players_name_stays_put() {
    let mut home = Home::default();
    home.knights.insert("percival".into(), Prowess { xp: 10, learned: Vec::new() });
    assert!(!home.adopt("percival", "percival"));
    assert_eq!(home.prowess("percival").xp, 10);
}
