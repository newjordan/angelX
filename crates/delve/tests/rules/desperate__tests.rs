use super::*;

/// A guardian at full health in a quiet room, and a skeleton beside it.
fn arena() -> Run {
    let mut run = Run::new(17, 1, Some("Friend"));
    run.enemies.clear();
    run.projectiles.clear();
    run.calm_for_test();
    run.spawn_boss_at_for_test(0, 24.0, 14.0);
    run.spawn_at_for_test(EnemyKind::Skeleton, 30.0, 14.0);
    for e in &mut run.enemies {
        e.seen_hp = e.hp;
    }
    run
}

#[test]
fn a_guardians_bar_holds_a_blows_bite_and_lets_it_go() {
    let mut run = arena();
    let max = run.enemies[0].max_hp;
    let before = run.enemies[0].hp;
    // A blow of a fifth of its health, and one on the skeleton.
    run.enemies[0].hp -= max / 5;
    run.enemies[1].hp -= 1;
    run.mark_hits();
    let bite = |run: &Run| run.enemies[0].drain.saturating_sub(run.enemies[0].hp);
    assert_eq!(run.enemies[0].hurt, HIT_FLASH);
    assert!(run.enemies[0].drain < before && bite(&run) > max / 6, "it lets an eighth go a tick");
    assert_eq!(run.enemies[1].drain, 0, "only a guardian's bar drains");
    // It falls a little each tick, quickest at first, and is gone inside
    // a second.
    let mut last = bite(&run);
    for tick in 0..30 {
        run.mark_hits();
        let now = bite(&run);
        assert!(now < last || now == 0, "tick {tick}: {now} after {last}");
        last = now;
    }
    assert_eq!(run.enemies[0].drain, 0);
    // A second blow while one still drains adds to the bite it shows.
    run.enemies[0].hp -= max / 10;
    run.mark_hits();
    let one = bite(&run);
    run.enemies[0].hp -= max / 10;
    run.mark_hits();
    assert!(bite(&run) > one);
    // Mended past it, the bar has nothing to drain.
    run.enemies[0].hp = max;
    run.mark_hits();
    assert_eq!(run.enemies[0].drain, 0);
}

#[test]
fn the_drain_is_written_only_while_it_shows() {
    let mut run = arena();
    let at_rest = serde_json::to_value(&run.enemies[0]).unwrap();
    assert!(at_rest.get("drain").is_none());
    run.enemies[0].hp -= run.enemies[0].max_hp / 5;
    run.mark_hits();
    let struck = serde_json::to_value(&run.enemies[0]).unwrap();
    assert_eq!(struck["drain"].as_u64(), Some(u64::from(run.enemies[0].drain)));
    // A save from before the bar drained reads as at rest.
    let old: Enemy = serde_json::from_value(at_rest).unwrap();
    assert_eq!(old.drain, 0);
}
