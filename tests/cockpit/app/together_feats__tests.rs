use super::*;
use crate::drive::together_shooter::home::{BUY_HOLD, Home, Station};

fn noticed(run: &Run, id: &str) -> bool {
    run.feats.contains(&id)
}

#[test]
fn every_feat_has_a_name_a_line_and_a_unique_id() {
    let mut ids: Vec<&str> = FEATS.iter().map(|f| f.id).collect();
    ids.sort_unstable();
    ids.dedup();
    assert_eq!(ids.len(), FEATS.len());
    for f in FEATS {
        assert!(!f.name.is_empty() && f.says.len() > 20, "{}", f.id);
    }
    assert!(FEATS.iter().any(|f| f.tier == Tier::Legendary));
}

#[test]
fn boxes_hold_more_the_better_they_are() {
    use crate::drive::together_realm::Spoil;
    for n in 0..20 {
        let worth = |t: Tier| t.contents(n).get(Spoil::Gold);
        assert!(worth(Tier::Bronze) < worth(Tier::Silver));
        assert!(worth(Tier::Silver) < worth(Tier::Gold));
        assert!(worth(Tier::Gold) < worth(Tier::Legendary));
        assert_eq!(
            Tier::Bronze.contents(n),
            Tier::Bronze.contents(n),
            "the same box twice"
        );
    }
    assert!(Tier::Legendary.contents(0).get(Spoil::Scale) == 1);
}

#[test]
fn the_first_delve_and_the_wheel_are_noticed() {
    let mut run = Run::at_home(5, 1, None, Home::default(), Default::default());
    run.enter_for_test(1);
    let (c, r, w, h) = fortune::LEVER;
    let hero = run.players.get_mut(&1).unwrap();
    hero.x = (c as f32 + w as f32 / 2.0) * TILE_UNITS;
    hero.y = (r as f32 + h as f32 / 2.0) * TILE_UNITS;
    let fire = BTreeMap::from([(
        1,
        Input {
            fire: true,
            ..Input::default()
        },
    )]);
    for _ in 0..BUY_HOLD + 1 {
        run.step(&fire);
    }
    assert!(noticed(&run, "spin"));
    run.descend_for_test();
    assert!(noticed(&run, "first_delve"));
}

#[test]
fn a_clean_room_with_a_friend_is_flawless_and_bonded() {
    let mut run = Run::new(5, 1, Some("Friend"));
    let fight = run
        .dungeon
        .rooms
        .iter()
        .position(|r| r.kind == RoomKind::Fight)
        .unwrap();
    run.enter_for_test(fight);
    run.feats.clear();
    run.clear_for_test();
    assert!(noticed(&run, "flawless") && noticed(&run, "bonded"));
}

#[test]
fn the_coffer_plate_asks_to_open_a_box() {
    let mut run = Run::at_home(5, 1, None, Home::default(), Default::default());
    run.enter_for_test(1);
    let (c, r, w, h) = fortune::COFFER_PLATE;
    let hero = run.players.get_mut(&1).unwrap();
    hero.x = (c as f32 + w as f32 / 2.0) * TILE_UNITS;
    hero.y = (r as f32 + h as f32 / 2.0) * TILE_UNITS;
    let fire = BTreeMap::from([(
        1,
        Input {
            fire: true,
            ..Input::default()
        },
    )]);
    for _ in 0..BUY_HOLD + 1 {
        run.step(&fire);
    }
    assert_eq!(run.orders.len(), 1);
    assert_eq!(run.orders[0].station, Station::Coffer);
}

#[test]
fn an_announced_feat_shows_its_banner_for_a_while() {
    let mut run = Run::new(5, 1, None);
    run.announce("roadkill");
    assert_eq!(run.banner_now().map(|f| f.id), Some("roadkill"));
    assert!(run.cues.iter().any(|c| c == "achievement"));
    run.calm_for_test();
    run.enemies.clear();
    for _ in 0..BANNER_TICKS {
        run.tick += 1;
    }
    assert!(run.banner_now().is_none());
}

#[test]
fn a_keg_that_fells_three_is_demolitions() {
    let mut run = Run::new(5, 1, None);
    run.enter_for_test(0);
    run.calm_for_test();
    run.enemies.clear();
    for dx in [-1.0, 0.0, 1.0] {
        run.spawn_at_for_test(EnemyKind::Bat, 30.0 + dx, 14.0);
    }
    run.phase = Phase::Fighting;
    run.kegs.push(foes::Keg {
        x: 30.0,
        y: 14.0,
        fuse: 1,
    });
    run.feats.clear();
    run.step(&BTreeMap::new());
    assert!(noticed(&run, "demolitions"));
}
