//! Starfall: a floor's stars come at its own quiet moments and never in a
//! fight, a boss's room, a village or at home; every knight under one
//! catches a free wish that lands one step up and fades out of its room;
//! its shard is the first toucher's wish, or fades.
use super::*;
use crate::rules::phrasebook;

/// A run whose party stands in a plain fight room it has cleared, with
/// the floor's quiet moments brought up to `moments`.
fn quiet(seed: u64, knights: u32, moments: usize) -> Run {
    let mut run = Run::new(seed, 1, None);
    for id in 2..=knights {
        run.join_seat(id, &format!("Seat {id}"));
    }
    let room = run
        .dungeon
        .rooms
        .iter()
        .position(|r| r.kind == RoomKind::Fight && !r.great())
        .unwrap();
    run.enter_for_test(room);
    run.clear_for_test();
    for i in 0..run.dungeon.rooms.len() {
        if run.quiet_moments() >= moments {
            break;
        }
        let r = &mut run.dungeon.rooms[i];
        if counted(r.kind) && !(r.visited && r.cleared) {
            r.visited = true;
            r.cleared = true;
        }
    }
    for hero in run.players.values_mut() {
        hero.invulnerable = 0;
    }
    run
}

fn steps(run: &mut Run, n: u64) {
    for _ in 0..n {
        run.step(&BTreeMap::new());
    }
}

/// Until the floor's first star falls, at most `limit` ticks.
fn until_star(run: &mut Run, limit: u64) -> bool {
    for _ in 0..limit {
        run.step(&BTreeMap::new());
        if run.starfall.star.is_some() {
            return true;
        }
    }
    false
}

#[test]
fn a_floors_stars_come_once_or_twice_never_at_its_first_room() {
    let mut twice = 0;
    for seed in 0..60u64 {
        let run = Run::new(seed, 1, None);
        let (count, at) = run.star_plan();
        assert!((1..=2).contains(&count));
        assert!(at[0] >= FIRST_MOMENT, "never the first room cleared");
        assert!(at[1] >= at[0] + 2);
        twice += usize::from(count == 2);
    }
    assert!((8..40).contains(&twice), "two stars on some floors, not most: {twice}");
}

#[test]
fn a_star_crosses_a_still_room_at_the_floors_moment_and_every_knight_catches_its_wish() {
    let mut run = quiet(7, 3, 0);
    let (_, at) = run.star_plan();
    // Short of the floor's moment, nothing comes, and nothing is written.
    let mut early = quiet(7, 3, at[0] - 1);
    steps(&mut early, QUIET * 3);
    assert!(early.starfall.star.is_none() && Starfall::omit(&early.starfall), "short of its moment");
    run = quiet(7, 3, at[0]);
    assert!(until_star(&mut run, QUIET + 5), "the room still a moment, the star comes");
    assert!(run.starfall.star.as_ref().unwrap().in_sky(run.tick));
    for id in 1..=3 {
        assert!(run.players[&id].star_wish, "knight {id} catches its wish");
        assert!(run.can_wish(id));
    }
    assert!(run.sounds.contains(&"fairy_wish") || run.cues.iter().any(|c| c == "starfall"));
    assert_eq!(run.starfall.fell, 1);
    assert!(run.valid_snapshot());
}

#[test]
fn no_star_comes_while_anything_fights_or_a_boss_is_awake() {
    let mut run = quiet(7, 1, 9);
    let (x, y) = (run.players[&1].x + 12.0, run.players[&1].y);
    let _ = run.spawn_at(EnemyKind::Slime, x, y);
    run.phase = Phase::Fighting;
    for _ in 0..QUIET * 2 {
        run.step(&BTreeMap::new());
        if run.enemies.iter().any(|e| e.hp > 0) {
            assert!(run.starfall.due.is_none() && run.starfall.star.is_none(), "not in a fight");
        }
    }
    // In the floor boss's own room, cleared, there is no star either.
    let mut boss = Run::new(4, 1, Some("Matt"));
    boss.boss_gates = None;
    let room = boss.dungeon.rooms.iter().position(|r| r.kind == RoomKind::Stairs).unwrap();
    boss.enter_for_test(room);
    boss.clear_for_test();
    for r in &mut boss.dungeon.rooms {
        r.visited = true;
        r.cleared = true;
    }
    steps(&mut boss, QUIET * 3);
    assert!(boss.starfall.star.is_none(), "never in the guardian's room");
}

#[test]
fn no_star_falls_at_home() {
    let mut run = Run::at_home(4, 1, Some("Matt"), Default::default(), Default::default());
    for r in &mut run.dungeon.rooms {
        r.visited = true;
        r.cleared = true;
    }
    steps(&mut run, QUIET * 3);
    assert!(run.starfall.star.is_none() && Starfall::omit(&run.starfall));
}

#[test]
fn a_star_wish_is_free_and_lands_one_step_up_before_a_super_wish() {
    let mut run = quiet(7, 2, 0);
    run.star_falls_for_test();
    let volley = phrasebook::get("volley").unwrap();
    {
        let hero = run.players.get_mut(&1).unwrap();
        (hero.wishes, hero.great_wishes) = (0, 1);
    }
    run.offer_wish(1, &volley.name, Some(&volley)).unwrap();
    let offer = run.players[&1].offer.clone().unwrap();
    assert!(offer.star && !offer.great, "the star's scroll, never a super wish");
    assert!(offer.choices.iter().all(|c| c.steps == 1), "one step");
    assert!(offer.choices.iter().all(|c| c.tip.is_none() && c.take != genie::Take::Bolder));
    let name = run.choose(1, 0).unwrap();
    assert!(name.starts_with("Triple Volley I") && !name.starts_with("Triple Volley II"), "a new wish at its first tier: {name}");
    let hero = &run.players[&1];
    assert!(!hero.star_wish, "spent");
    assert_eq!((hero.wishes, hero.great_wishes), (0, 1), "and nothing of the knight's own");
    assert!(run.starfall.granted.iter().any(|g| g.knight == 1), "its moment is drawn");
    // The super wish is the knight's next.
    run.offer_wish(1, &volley.name, Some(&volley)).unwrap();
    assert!(run.players[&1].offer.as_ref().is_some_and(|o| o.great && !o.star));
}

#[test]
fn a_knight_with_only_a_star_wish_opens_the_wish_line_and_says_new_words() {
    let mut run = quiet(7, 1, 0);
    run.players.get_mut(&1).unwrap().wishes = 0;
    assert!(!run.wish_line_opens(1));
    run.star_falls_for_test();
    assert!(run.wish_line_opens(1), "the wish line opens for a star's wish alone");
    run.offer_wish(1, "a cloak of falling stars", None).unwrap();
    assert!(run.players[&1].offer.as_ref().is_some_and(|o| o.star));
    run.choose(1, 0).unwrap();
    let asked = run.asked.last().unwrap();
    assert!(asked.star && asked.steps == 1 && !asked.great, "to the learner, as the star's");
    assert!(!run.players[&1].star_wish);
    assert!(!run.can_wish(1), "nothing left to spend");
}

#[test]
fn a_star_wish_fades_once_the_party_leaves_its_room_but_an_open_scroll_keeps_it() {
    let mut run = quiet(7, 2, 0);
    run.star_falls_for_test();
    let volley = phrasebook::get("volley").unwrap();
    run.offer_wish(2, &volley.name, Some(&volley)).unwrap();
    let other = (0..run.dungeon.rooms.len()).find(|&i| i != run.at && counted(run.dungeon.rooms[i].kind)).unwrap();
    run.enter_for_test(other);
    run.enemies.clear();
    steps(&mut run, 2);
    assert!(!run.players[&1].star_wish, "said under its own sky, or not at all");
    assert!(run.players[&2].star_wish, "a scroll already open with it is kept");
    run.choose(2, 0).unwrap();
    assert!(!run.players[&2].star_wish);
}

#[test]
fn the_fallen_star_is_the_first_toucher_s_wish_and_otherwise_fades() {
    let mut run = quiet(7, 2, 0);
    run.star_falls_for_test();
    let land = run.starfall.star.as_ref().unwrap().land;
    // It lands on open floor, a walk from the party.
    let grid = Grid { room: run.room(), barred: false };
    assert_eq!(grid.tile_at(land.0, land.1), Tile::Floor);
    for h in run.players.values() {
        assert!((h.x - land.0).hypot(h.y - land.1) >= CLEAR_OF_KNIGHTS, "someone walks to it");
    }
    steps(&mut run, LANDS + 1);
    assert_eq!(run.starfall.star.as_ref().unwrap().fall, Fall::Lying);
    assert!(run.starfall.shard(run.tick).is_some());
    let before = run.players[&2].wishes;
    {
        let h = run.players.get_mut(&2).unwrap();
        (h.x, h.y) = land;
    }
    steps(&mut run, 1);
    assert!(matches!(run.starfall.star.as_ref().unwrap().fall, Fall::Taken { knight: 2, .. }), "the star is theirs");
    assert_eq!(run.players[&2].wishes, before + 1);
    // Nobody's: it fades.
    let mut left = quiet(7, 1, 0);
    left.star_falls_for_test();
    let land = left.starfall.star.as_ref().unwrap().land;
    {
        let h = left.players.get_mut(&1).unwrap();
        (h.x, h.y) = (land.0 + 8.0, land.1);
    }
    let wishes = left.players[&1].wishes;
    for _ in 0..LANDS + SHARD_LIFE + 2 {
        left.step(&BTreeMap::new());
        let h = left.players.get_mut(&1).unwrap();
        (h.x, h.y) = (land.0 + 8.0, land.1);
    }
    assert_eq!(left.starfall.star.as_ref().unwrap().fall, Fall::Gone);
    assert_eq!(left.players[&1].wishes, wishes);
}

#[test]
fn a_new_floor_lets_the_last_floors_stars_go() {
    let mut run = quiet(7, 1, 0);
    run.star_falls_for_test();
    assert!(run.players[&1].star_wish);
    run.descend_for_test();
    assert!(!run.players[&1].star_wish);
    assert!(Starfall::omit(&run.starfall));
}

#[test]
fn a_star_survives_the_save_and_an_empty_sky_writes_nothing() {
    let run = quiet(7, 1, 0);
    let json = serde_json::to_string(&run).unwrap();
    assert!(!json.contains("\"starfall\"") && !json.contains("star_wish"), "no star, no field");
    let mut run = run;
    run.star_falls_for_test();
    let json = serde_json::to_string(&run).unwrap();
    let back: Run = serde_json::from_str(&json).unwrap();
    assert_eq!(back.starfall, run.starfall);
    assert!(back.players[&1].star_wish);
    assert!(back.valid_snapshot());
}

#[test]
fn a_star_crosses_an_open_floors_map_once_its_chambers_have_been_cleared() {
    let mut run = Run::new(9, 1, None);
    run.open_floor_for_test(2);
    // The companies sleep far off; the party's chambers are still.
    run.enemies.clear();
    let (_, at) = run.star_plan();
    if let Some(o) = run.dungeon.open.as_mut() {
        let mut n = 0;
        for c in o.chambers.iter_mut() {
            if n < at[0] && counted(c.kind) {
                c.cleared = true;
                n += 1;
            }
        }
    }
    assert!(run.quiet_moments() >= at[0]);
    assert!(until_star(&mut run, QUIET + 5), "the open map has its sky too");
    let land = run.starfall.star.as_ref().unwrap().land;
    let grid = Grid { room: run.room(), barred: false };
    assert_eq!(grid.tile_at(land.0, land.1), Tile::Floor);
}

/// How often stars come on real floors: `cargo test star_frequency --
/// --ignored --nocapture`. For each floor, the rooms a party could clear
/// against the floor's plan: the stars it would see if it cleared them all.
#[test]
#[ignore]
fn star_frequency() {
    let (mut floors, mut seen, mut twice, mut rooms) = (0, 0, 0, 0);
    for seed in 0..40u64 {
        let mut run = Run::new(seed, 1, None);
        for _ in 0..6 {
            if run.dungeon.depth >= 1 && run.dungeon.open.is_none() {
                let n = run.dungeon.rooms.iter().filter(|r| counted(r.kind)).count();
                let (count, at) = run.star_plan();
                floors += 1;
                rooms += n;
                let stars = usize::from(n >= at[0]) + usize::from(count == 2 && n >= at[1]);
                seen += usize::from(stars >= 1);
                twice += usize::from(stars == 2);
            }
            run.descend_for_test();
        }
    }
    println!("floors {floors}, rooms a floor {:.1}, a star on {seen}, two on {twice}", rooms as f32 / floors as f32);
}
