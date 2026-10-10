use super::*;

/// Floor one's entrance, its monsters gone, the party at ease.
fn delve() -> Run {
    let mut run = Run::new(5, 1, None);
    run.enter_for_test(0);
    run.calm_for_test();
    run.enemies.clear();
    run.phase = Phase::Exploring;
    run
}

#[test]
fn viewers_read_like_a_ratings_board() {
    assert_eq!(viewers(640), "640K");
    assert_eq!(viewers(1240), "1.24M");
    assert_eq!(
        (milestone(1), milestone(2), milestone(3)),
        (250, 1000, 2250),
        "each box needs more than the last"
    );
}

#[test]
fn the_audience_reacts_to_what_the_herald_calls_and_to_every_kill() {
    let mut run = delve();
    run.cues.push("knight_down".into());
    run.tick_audience(run.cues.len() - 1);
    assert_eq!(run.audience, 40, "a knight going down is drama");
    // Two skeletons fall: a little each.
    run.spawn_at_for_test(EnemyKind::Skeleton, 30.0, 10.0);
    run.spawn_at_for_test(EnemyKind::Skeleton, 34.0, 10.0);
    run.phase = Phase::Fighting;
    for enemy in &mut run.enemies {
        enemy.hp = 0;
    }
    run.step(&BTreeMap::new());
    assert!(run.audience >= 44, "{}", run.audience);
}

#[test]
fn the_undercroft_is_off_the_air() {
    let mut run = Run::at_home(5, 1, None, Default::default(), Default::default());
    run.thrill(MILESTONE * 4);
    assert_eq!((run.audience, run.fans), (0, 0));
    assert!(run.fan_boxes.is_empty());
}

#[test]
fn a_milestone_throws_a_fan_box_that_lands_as_a_card_and_gold() {
    let mut run = delve();
    run.thrill(MILESTONE);
    assert_eq!((run.fans, run.fan_boxes.len()), (1, 1));
    assert!(run.cues.iter().any(|c| c == "fan_box"));
    // Set it down well away from the knight, so nothing is picked up.
    let hero = &run.players[&1];
    let (x, y) = (hero.x + 9.0, hero.y);
    (run.fan_boxes[0].x, run.fan_boxes[0].y) = (x, y);
    let items = run.room().items.len();
    for _ in 0..BOX_FALL {
        run.step(&BTreeMap::new());
    }
    assert!(run.fan_boxes.is_empty(), "landed");
    let near = |i: &&Item| (i.x - x).hypot(i.y - y) < 3.0;
    let dropped: Vec<&Item> = run.room().items.iter().filter(near).collect();
    assert_eq!(run.room().items.len(), items + dropped.len());
    assert_eq!(dropped.iter().filter(|i| i.card == "gold").count(), 2);
    assert_eq!(dropped.len(), 3, "and a card");
    assert!(run.cues.iter().any(|c| c == "fan_box_open"));
}

#[test]
fn a_million_viewers_is_must_see_tv() {
    let mut run = delve();
    run.thrill(1000);
    assert_eq!(run.fans, 2, "past 250K and 1M");
    assert!(run.feats.contains(&"must_see_tv"));
    assert!(run.cues.iter().any(|c| c == "audience_million"));
    run.thrill(1000);
    assert_eq!(run.fans, 2, "the third waits for 2.25M");
    run.thrill(8000);
    assert!(run.feats.contains(&"prime_time"), "ten million");
    assert_eq!(run.fans, 6, "past 9M");
}

#[test]
fn boxes_in_the_air_follow_the_party_into_the_next_room() {
    let mut run = delve();
    run.thrill(MILESTONE);
    let fall = run.fan_boxes[0].fall;
    let next = (1..run.dungeon.rooms.len())
        .find(|&i| run.dungeon.rooms[i].kind == RoomKind::Fight)
        .unwrap();
    run.enter_for_test(next);
    assert_eq!(run.fan_boxes.len(), 1);
    assert_eq!(run.fan_boxes[0].fall, fall);
    let room = run.room();
    let parcel = run.fan_boxes[0];
    assert_eq!(
        room.tile(
            (parcel.x / TILE_UNITS) as i32,
            (parcel.y / TILE_UNITS) as i32
        ),
        Tile::Floor
    );
}

#[test]
fn friends_watch_the_same_show() {
    let mut run = delve();
    run.thrill(300);
    let live = run.live();
    let mut friend: Run = serde_json::from_str(&serde_json::to_string(&run).unwrap()).unwrap();
    (friend.audience, friend.fans) = (0, 0);
    friend.fan_boxes.clear();
    assert!(friend.apply_live(live));
    assert_eq!(
        (friend.audience, friend.fans, friend.fan_boxes.len()),
        (300, 1, 1)
    );
}
