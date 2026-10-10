use super::*;
use crate::drive::together_realm::Spoil;

/// Floor one's entrance, quiet, Lady Tallow beside knight one.
fn with_tallow() -> Run {
    let mut run = Run::new(13, 1, None);
    run.enter_for_test(0);
    run.calm_for_test();
    run.enemies.clear();
    run.phase = Phase::Exploring;
    let hero = run.players.get_mut(&1).unwrap();
    (hero.x, hero.y) = (24.0, 14.0);
    run.cat = Some(Cat {
        x: 25.4,
        y: 14.9,
        fetching: None,
        carrying: None,
        hiss: 0,
        hissed: 0,
        left: false,
        moving: false,
        fetched: 0,
        stuck: 0,
    });
    run
}

#[test]
fn she_comes_down_the_stair_once_the_hearth_is_warm_enough() {
    for (rung, comes) in [(1u8, false), (HEARTH_FOR_TALLOW, true)] {
        let mut home = home::Home::default();
        home.levels.insert(home::Station::Hearth, rung);
        let mut run = Run::at_home(13, 1, None, home, Default::default());
        run.descend_for_test();
        assert_eq!(run.cat.is_some(), comes, "hearth {rung}");
    }
}

#[test]
fn she_hisses_at_a_monster_that_comes_too_close() {
    let mut run = with_tallow();
    run.spawn_at_for_test(EnemyKind::Skeleton, 27.0, 15.0);
    run.phase = Phase::Fighting;
    run.step(&BTreeMap::new());
    assert!(run.enemies[0].frozen > 0, "startled still");
    assert!(run.cues.iter().any(|c| c == "tallow_hiss"));
    let cat = run.cat.as_ref().unwrap();
    assert!(cat.hiss > 0, "and she rests her voice");
}

#[test]
fn she_fetches_spoils_home_to_her_knight_and_leaves_the_rest() {
    let mut run = with_tallow();
    let gold = run.players[&1].carried.get(Spoil::Gold);
    let room = run.at;
    for (card, x, y) in [("gold", 34.0, 20.0), ("potion", 14.0, 8.0)] {
        run.dungeon.rooms[room].items.push(Item {
            card: card.into(),
            x,
            y,
            held_off: None,
        });
    }
    for _ in 0..6 * HZ {
        run.step(&BTreeMap::new());
        let hero = run.players.get_mut(&1).unwrap();
        (hero.x, hero.y) = (24.0, 14.0);
    }
    assert!(
        run.players[&1].carried.get(Spoil::Gold) > gold,
        "the gold is home"
    );
    let items: Vec<&str> = run.room().items.iter().map(|i| i.card.as_str()).collect();
    assert_eq!(items, ["potion"], "a potion is not shiny enough");
    assert_eq!(run.cat.as_ref().unwrap().fetched, 1);
    assert!(run.cues.iter().any(|c| c == "tallow_fetch"));
}

#[test]
fn she_is_beside_her_knight_in_every_new_room() {
    let mut run = with_tallow();
    let next = (1..run.dungeon.rooms.len())
        .find(|&i| run.dungeon.rooms[i].kind == RoomKind::Fight)
        .unwrap();
    run.enter_for_test(next);
    let (cat, hero) = (run.cat.as_ref().unwrap(), &run.players[&1]);
    assert!((cat.x - hero.x).hypot(cat.y - hero.y) < 2.0);
}

#[test]
fn friends_see_her_too() {
    let run = with_tallow();
    let live = run.live();
    let mut friend: Run = serde_json::from_str(&serde_json::to_string(&run).unwrap()).unwrap();
    friend.cat = None;
    assert!(friend.apply_live(live));
    assert!(friend.cat.is_some());
}

#[test]
fn a_pillar_in_her_way_does_not_keep_her() {
    let mut run = with_tallow();
    // A pillar between her and a pile of gold, square across her path.
    let room = run.at;
    for row in 5..9 {
        run.dungeon.rooms[room].set_for_test(15, row, Tile::Block);
    }
    run.cat.as_mut().unwrap().x = 28.0;
    run.cat.as_mut().unwrap().y = 13.0;
    run.dungeon.rooms[room].items.push(Item {
        card: "gold".into(),
        x: 33.0,
        y: 13.0,
        held_off: None,
    });
    let hero = run.players.get_mut(&1).unwrap();
    (hero.x, hero.y) = (24.0, 13.0);
    for _ in 0..8 * HZ {
        run.step(&BTreeMap::new());
        let hero = run.players.get_mut(&1).unwrap();
        (hero.x, hero.y) = (24.0, 13.0);
    }
    assert_eq!(run.cat.as_ref().unwrap().fetched, 1, "fetched and home");
}
