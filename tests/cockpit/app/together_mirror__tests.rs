use super::*;

fn walk(run: &mut Run, n: usize) {
    for i in 0..n {
        let input = Input {
            move_x: if i % 40 < 20 { 1 } else { -1 },
            aim_x: 1,
            fire: true,
            ..Default::default()
        };
        run.step(&BTreeMap::from([(1, input), (2, input)]));
    }
}

#[test]
fn a_mirror_fed_each_ticks_changes_stays_the_hosts_delve() {
    let mut host = Run::new(3, 9, Some("Matt"));
    let fight = host
        .dungeon
        .rooms
        .iter()
        .position(|r| r.kind == RoomKind::Fight)
        .unwrap();
    host.enter_for_test(fight);
    let mut friend: Run = serde_json::from_slice(&serde_json::to_vec(&host).unwrap()).unwrap();
    for _ in 0..90 {
        walk(&mut host, 1);
        let wire = serde_json::to_vec(&host.live()).unwrap();
        assert!(wire.len() < 16 * 1024, "a tick is small: {}", wire.len());
        assert!(friend.apply_live(serde_json::from_slice(&wire).unwrap()));
    }
    let same = |run: &Run| serde_json::to_string(&run.live()).unwrap();
    assert_eq!(same(&friend), same(&host));
    assert!(!host.projectiles.is_empty(), "shots in flight came across");
    // A new floor needs the whole run first.
    host.descend_for_test();
    assert!(!friend.apply_live(host.live()));
}

#[test]
fn a_view_between_ticks_slides_from_the_last_tick_to_this_one() {
    let mut run = Run::new(3, 9, None);
    run.calm_for_test();
    walk(&mut run, 3);
    let before = run.pose();
    let was = (run.players[&1].x, run.players[&1].y);
    walk(&mut run, 1);
    let now = (run.players[&1].x, run.players[&1].y);
    assert_ne!(was, now);
    let at = |alpha: f32| {
        let view = run.between(&before, alpha);
        (view.players[&1].x, view.players[&1].y)
    };
    assert_eq!(at(0.0), was);
    assert_eq!(at(1.0), now);
    let half = at(0.5);
    assert!((half.0 - (was.0 + now.0) / 2.0).abs() < 1e-4);
    // Drawn in place, then put back as it was.
    let tick = run.drawn_between(&before, 0.25, |view| view.players[&1].x);
    assert!((tick - (was.0 + (now.0 - was.0) * 0.25)).abs() < 1e-4);
    assert_eq!((run.players[&1].x, run.players[&1].y), now);
    // Across a gap of ticks nothing slides.
    walk(&mut run, 2);
    let far = (run.players[&1].x, run.players[&1].y);
    assert_eq!(
        (
            run.between(&before, 0.0).players[&1].x,
            run.between(&before, 0.0).players[&1].y
        ),
        far
    );
}

/// `ANGEL_STREAM_LINES=<file> cargo test stream_lines_parse -- --ignored`:
/// every line a real host streamed reads back on a friend's side.
#[test]
#[ignore]
fn stream_lines_parse() {
    let Some(path) = std::env::var_os("ANGEL_STREAM_LINES") else {
        return;
    };
    let bytes = std::fs::read(path).unwrap();
    for line in bytes.split(|&b| b == b'\n').filter(|l| !l.is_empty()) {
        let value: serde_json::Value = serde_json::from_slice(line).unwrap();
        if let Some(whole) = value.get("whole") {
            serde_json::from_value::<Run>(whole.clone()).unwrap();
        }
        if let Some(live) = value.get("live") {
            serde_json::from_value::<Live>(live.clone()).unwrap();
        }
    }
}

#[test]
fn a_friends_own_knight_is_drawn_where_their_keys_take_it() {
    let mut run = Run::new(3, 9, Some("Matt"));
    run.calm_for_test();
    let before = run.pose();
    let start = (run.players[&2].x, run.players[&2].y);
    let right = Input {
        move_x: 1,
        ..Default::default()
    };
    // A tenth of a second ahead: about a knight's pace times 0.1 to the east.
    let drawn = run.drawn_ahead(&before, 1.0, Some((2, right, 0.1)), |view| {
        (view.players[&2].x, view.players[&2].y)
    });
    assert!(
        drawn.0 > start.0 + 0.8 && drawn.0 < start.0 + 1.6,
        "{start:?} -> {drawn:?}"
    );
    assert_eq!(drawn.1, start.1);
    assert_eq!(
        (run.players[&2].x, run.players[&2].y),
        start,
        "the mirror itself is untouched"
    );
    // Never through stone: far ahead into the west wall stops at it.
    let left = Input {
        move_x: -1,
        ..Default::default()
    };
    let hero = run.players.get_mut(&2).unwrap();
    hero.x = 3.0;
    let drawn = run.drawn_ahead(&before, 1.0, Some((2, left, 0.25)), |view| {
        view.players[&2].x
    });
    assert!(drawn >= 1.9, "stopped by the wall: {drawn}");
}

#[test]
fn a_frame_of_lead_still_moves_the_knight() {
    let mut run = Run::new(3, 9, Some("Matt"));
    run.calm_for_test();
    let before = run.pose();
    let start = run.players[&2].x;
    let right = Input {
        move_x: 1,
        ..Default::default()
    };
    // Less than one 30 Hz tick. Rounding the lead away would leave the knight still.
    let drawn = run.drawn_ahead(&before, 1.0, Some((2, right, 1.0 / 120.0)), |view| {
        view.players[&2].x
    });
    let step = drawn - start;
    assert!(
        step > 0.05 && step < 0.2,
        "one frame of lead should be a short step, was {step}"
    );
    assert_eq!(run.players[&2].x, start, "the run itself is untouched");
}

#[test]
fn spell_slots_timers_and_card_edits_cross_the_mirror() {
    let mut host = Run::new(19, 1, Some("friend"));
    let raw = "name Spark\nkind spell\ncooldown 3\ncast bolt\n";
    host.add_card(cards::check("spark", raw).unwrap().card, 2);
    let mut mirror: Run = serde_json::from_value(serde_json::to_value(&host).unwrap()).unwrap();
    assert_eq!(mirror.players[&2].spells[0].as_deref(), Some("spark"));
    host.players.get_mut(&2).unwrap().spell_cooldowns[0] = 41;
    assert!(mirror.apply_live(host.live()));
    assert_eq!(mirror.players[&2].spell_cooldowns[0], 41);
    let key = host.mirror_key();
    let count = host.book.cards.len();
    host.add_card(
        cards::check("spark", &raw.replace("cooldown 3", "cooldown 5"))
            .unwrap()
            .card,
        2,
    );
    assert_eq!(host.book.cards.len(), count);
    assert_ne!(
        host.mirror_key(),
        key,
        "replacement requires a whole mirror, even at the same book length"
    );
    assert_eq!(host.players[&2].spell_cooldowns[0], 41);
    let mut old = serde_json::to_value(&host).unwrap();
    for hero in old["players"].as_object_mut().unwrap().values_mut() {
        hero.as_object_mut().unwrap().remove("spells");
        hero.as_object_mut().unwrap().remove("spell_cooldowns");
    }
    let old: Run = serde_json::from_value(old).unwrap();
    assert_eq!(old.players[&1].spells, [None, None, None]);
    assert_eq!(old.players[&1].spell_cooldowns, [0, 0, 0]);
}

#[test]
fn a_friend_digs_the_west_wing_with_the_host() {
    use crate::drive::together_realm::Spoils;
    use crate::drive::together_shooter::home::{Home, Station};
    let mut host = Run::at_home(3, 9, Some("Matt"), Home::default(), Spoils::default());
    let mut friend: Run = serde_json::from_slice(&serde_json::to_vec(&host).unwrap()).unwrap();
    let mut home = Home {
        deepest: 1,
        ..Home::default()
    };
    home.levels.insert(Station::Wing, 1);
    host.rebuild_home(home, Spoils::default());
    let tavern = host
        .dungeon
        .rooms
        .iter()
        .position(|r| r.kind == RoomKind::Tavern)
        .unwrap();
    host.enter_for_test(tavern);
    host.step(&BTreeMap::new());
    assert!(
        friend.apply_live(host.live()),
        "the friend follows the host in"
    );
    assert_eq!(friend.room().kind, RoomKind::Tavern);
}

#[test]
fn a_cracked_wall_the_host_blew_comes_down_for_friends() {
    let mut host = (0..200u64)
        .map(|seed| {
            let mut run = Run::new(seed, 9, Some("Matt"));
            run.begin_in(Pack::Crypt);
            run
        })
        .find(|run| run.dungeon.secret.is_some())
        .unwrap();
    let secret = host.dungeon.secret.unwrap();
    host.enter_for_test(secret.host);
    let mut friend: Run = serde_json::from_slice(&serde_json::to_vec(&host).unwrap()).unwrap();
    host.step(&BTreeMap::from([(
        1,
        Input {
            bomb: true,
            ..Default::default()
        },
    )]));
    assert!(host.dungeon.secret.unwrap().found);
    assert!(friend.apply_live(host.live()));
    assert!(friend.dungeon.secret.unwrap().found);
    assert_eq!(
        friend.dungeon.neighbour(secret.host, secret.side),
        Some(secret.vault),
        "the doorway is open on the friend's screen too"
    );
}

#[test]
fn a_ravage_ripples_and_throws_on_a_friends_screen_too() {
    let mut host = Run::new(4, 9, Some("Matt"));
    host.begin_in(Pack::Crypt);
    let mut friend: Run = serde_json::from_slice(&serde_json::to_vec(&host).unwrap()).unwrap();
    let (x, y) = (host.players[&2].x + tide::RING_GAP, host.players[&2].y);
    host.ravages
        .push(tide::Ravage::cast((x, y), 3, 50.0, 8.0, 16, 5));
    host.players.get_mut(&2).unwrap().tossed = tide::TOSSED;
    host.step(&BTreeMap::new());
    assert!(friend.apply_live(host.live()));
    assert_eq!(friend.ravages, host.ravages, "the rings, rippling");
    assert!(friend.players[&2].tossed > 0, "and the friend in the air");
}

#[test]
fn beaumains_fights_on_a_friends_screen_too() {
    let mut host = Run::new(4, 9, Some("Matt"));
    host.begin_in(Pack::Crypt);
    let mut friend: Run = serde_json::from_slice(&serde_json::to_vec(&host).unwrap()).unwrap();
    let at = (host.players[&1].x, host.players[&1].y);
    host.hireling = Some(hireling::Hireling::new(1, at));
    host.step(&BTreeMap::new());
    assert!(friend.apply_live(host.live()));
    assert_eq!(friend.hireling, host.hireling);
}

#[test]
fn two_frames_of_lead_move_further_and_a_wall_still_stops_the_knight() {
    let mut run = Run::new(3, 9, Some("Matt"));
    run.calm_for_test();
    let before = run.pose();
    let start = run.players[&2].x;
    let right = Input {
        move_x: 1,
        ..Default::default()
    };
    let one = run.drawn_ahead(&before, 1.0, Some((2, right, 1.0 / 120.0)), |view| {
        view.players[&2].x
    });
    let two = run.drawn_ahead(&before, 1.0, Some((2, right, 2.0 / 120.0)), |view| {
        view.players[&2].x
    });
    assert!(
        two > one + 0.02,
        "a second frame of lead is another step: {one} then {two}"
    );
    assert_eq!(run.players[&2].x, start, "the run itself is untouched");

    // Pressed against the west wall, two frames of lead go nowhere.
    let mut boxed = Run::new(3, 9, Some("Matt"));
    boxed.calm_for_test();
    let before = boxed.pose();
    boxed.players.get_mut(&2).unwrap().x = 3.0;
    let left = Input {
        move_x: -1,
        ..Default::default()
    };
    let wall = boxed.drawn_ahead(&before, 1.0, Some((2, left, 0.25)), |view| {
        view.players[&2].x
    });
    assert!(wall < 2.9, "a quarter second walks up to the wall: {wall}");
    boxed.players.get_mut(&2).unwrap().x = wall;
    let before = boxed.pose();
    let drawn = boxed.drawn_ahead(&before, 1.0, Some((2, left, 2.0 / 120.0)), |view| {
        view.players[&2].x
    });
    assert!(
        (drawn - wall).abs() < 1e-4,
        "two frames of lead do not pass the wall: {wall} then {drawn}"
    );
}
