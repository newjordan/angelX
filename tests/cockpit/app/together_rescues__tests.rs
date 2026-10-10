use super::*;

/// Floor one of the Crypt, Brother Anselm in a cage in its treasure room,
/// knight one beside the cage.
fn at_the_cage() -> Run {
    let mut run = Run::new(4, 1, None);
    run.begin_in(Pack::Crypt);
    let room = run
        .dungeon
        .rooms
        .iter()
        .position(|r| r.kind == RoomKind::Treasure)
        .unwrap();
    run.enter_for_test(room);
    let r = run.room();
    let (x, y) = (r.width() / 2.0, r.height() / 2.0 - 6.0);
    run.captive = Some(Captive {
        who: "anselm".into(),
        room,
        x,
        y,
        freed: None,
    });
    let hero = run.players.get_mut(&1).unwrap();
    (hero.x, hero.y) = (x, y + 2.0);
    run
}

fn hold_f(run: &mut Run, ticks: u32) {
    let fire = BTreeMap::from([(
        1,
        Input {
            fire: true,
            ..Default::default()
        },
    )]);
    for _ in 0..ticks {
        run.step(&fire);
    }
}

#[test]
fn a_floor_of_their_delve_may_hold_a_cage_until_they_are_home() {
    let caged = |home: home::Home| {
        (0..40u64)
            .filter(|&seed| {
                let mut run = Run::at_home(seed, 1, None, home.clone(), Default::default());
                run.begin_in(Pack::Crypt);
                run.descend_for_test();
                let captive = run.captive.clone();
                if let Some(c) = &captive {
                    assert_eq!(c.who, "anselm", "the Crypt keeps Brother Anselm");
                    assert_eq!(run.dungeon.rooms[c.room].kind, RoomKind::Treasure);
                }
                captive.is_some()
            })
            .count()
    };
    let found = caged(home::Home::default());
    assert!((8..=32).contains(&found), "{found} cages in 40 floors");
    let mut home = home::Home::default();
    home.residents.insert("anselm".into());
    assert_eq!(caged(home), 0, "nobody is caged twice");
}

#[test]
fn holding_f_at_the_cage_sets_them_free() {
    let mut run = at_the_cage();
    hold_f(&mut run, home::BUY_HOLD + 2);
    assert!(run.captive.as_ref().unwrap().freed.is_some());
    assert_eq!(run.marks.get("rescue:anselm"), Some(&1));
    assert!(run.cues.iter().any(|c| c == "rescued:anselm"));
    assert!(run.feats.contains(&"rescuer"));
}

#[test]
fn a_knight_across_the_room_cannot_work_the_lock() {
    let mut run = at_the_cage();
    let hero = run.players.get_mut(&1).unwrap();
    hero.y += 10.0;
    hold_f(&mut run, home::BUY_HOLD + 2);
    assert!(run.captive.as_ref().unwrap().freed.is_none());
}

#[test]
fn the_people_at_home_help_the_party_down() {
    let mut home = home::Home::default();
    for who in ["mabel", "anselm", "pip", "maud"] {
        home.residents.insert(who.into());
    }
    let bare = {
        let mut run = Run::at_home(4, 1, None, home::Home::default(), Default::default());
        run.descend_for_test();
        run.players[&1].max_hp
    };
    let mut run = Run::at_home(4, 1, None, home, Default::default());
    assert!(
        run.players[&1].deck.iter().any(|c| c == "home-stew"),
        "Mabel's stew"
    );
    run.descend_for_test();
    let hero = &run.players[&1];
    assert_eq!(hero.max_hp, bare + 15, "her stew");
    assert_eq!(hero.winds, 1, "Brother Anselm's blessing");
    assert!(hero.hand.iter().any(|c| c == "potion"), "Maud's ale");
    assert!(
        run.dungeon.rooms.iter().all(|r| r.visited),
        "Pip's map, the floor drawn whole"
    );
}

#[test]
fn friends_see_the_cage() {
    let run = at_the_cage();
    let live = run.live();
    let mut friend: Run = serde_json::from_str(&serde_json::to_string(&run).unwrap()).unwrap();
    friend.captive = None;
    assert!(friend.apply_live(live));
    assert_eq!(
        friend.captive.as_ref().map(|c| c.who.as_str()),
        Some("anselm")
    );
}
