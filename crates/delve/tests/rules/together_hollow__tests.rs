use super::*;

/// An open first-floor room of the Crypt, emptied and calmed, knight one on
/// the left, and a Hollow One holding still in the far corner (keeping the
/// fight on, and keeping any hole open).
fn with_a_hollow() -> (Run, u32) {
    let mut run = Run::new(4, 1, None);
    run.begin_in(Pack::Crypt);
    let room = run
        .dungeon
        .rooms
        .iter()
        .position(|r| r.kind == RoomKind::Fight)
        .unwrap();
    run.enter_for_test(room);
    run.calm_for_test();
    run.enemies.clear();
    run.rune = None;
    run.spawn_at_for_test(EnemyKind::Hollow, 44.0, 3.0);
    let hollow = run.enemies.last_mut().unwrap();
    (hollow.stage, hollow.timer) = (1, 0);
    let id = hollow.id;
    let hero = run.players.get_mut(&1).unwrap();
    (hero.x, hero.y, hero.invulnerable) = (10.0, 12.0, 0);
    (run, id)
}

/// A hole opened by Hollow One `owner` at `(x, y)`, its warning done.
fn open(run: &mut Run, owner: u32, (x, y): (f32, f32), warned: bool) {
    let owner_hp = run.enemies.iter().find(|e| e.id == owner).unwrap().hp;
    run.holes.push(Hole {
        x,
        y,
        warn: if warned { 0 } else { HOLE_WARN },
        left: HOLE_PULL,
        owner,
        owner_hp,
    });
}

fn still(run: &mut Run, ticks: u32) {
    for _ in 0..ticks {
        let hero = run.players.get_mut(&1).unwrap();
        (hero.hp, hero.invulnerable) = (hero.max_hp, 0);
        if let Some(h) = run.enemies.iter_mut().find(|e| e.kind == EnemyKind::Hollow) {
            h.timer = 0;
        }
        run.step(&BTreeMap::new());
    }
}

#[test]
fn a_hole_warns_then_drags_a_knight_in() {
    let (mut run, id) = with_a_hollow();
    open(&mut run, id, (15.0, 12.0), false);
    still(&mut run, HOLE_WARN - 2);
    assert_eq!(run.players[&1].x, 10.0, "only a warning so far");
    still(&mut run, 30);
    assert!(
        run.players[&1].x > 11.0,
        "dragged in: {}",
        run.players[&1].x
    );
}

#[test]
fn its_core_burns() {
    let (mut run, id) = with_a_hollow();
    open(&mut run, id, (10.2, 12.0), true);
    let hp = run.players[&1].hp;
    for _ in 0..40 {
        run.players.get_mut(&1).unwrap().invulnerable = 0;
        run.step(&BTreeMap::new());
    }
    assert!(run.players[&1].hp < hp);
}

#[test]
fn a_knight_walking_away_beats_the_pull() {
    let (mut run, id) = with_a_hollow();
    open(&mut run, id, (14.0, 12.0), true);
    let away = BTreeMap::from([(
        1,
        Input {
            move_x: -1,
            ..Default::default()
        },
    )]);
    let x = run.players[&1].x;
    for _ in 0..20 {
        run.step(&away);
    }
    assert!(
        run.players[&1].x < x,
        "walking out: {x} -> {}",
        run.players[&1].x
    );
}

#[test]
fn a_hard_blow_breaks_the_channel() {
    let (mut run, id) = with_a_hollow();
    open(&mut run, id, (15.0, 12.0), true);
    still(&mut run, 4);
    assert_eq!(run.holes.len(), 1);
    let hollow = run.enemies.iter_mut().find(|e| e.id == id).unwrap();
    hollow.hp -= BREAK;
    run.step(&BTreeMap::new());
    assert!(run.holes.is_empty(), "the hole falls shut");
    assert!(run.cues.iter().any(|c| c == "channel_broken"));
    assert!(run.feats.contains(&"event_horizon"));
}

#[test]
fn the_hollow_one_opens_a_hole_where_a_knight_stands() {
    let mut run = Run::new(4, 1, None);
    run.begin_in(Pack::Crypt);
    let room = run
        .dungeon
        .rooms
        .iter()
        .position(|r| r.kind == RoomKind::Fight)
        .unwrap();
    run.enter_for_test(room);
    run.calm_for_test();
    run.enemies.clear();
    run.spawn_at_for_test(EnemyKind::Hollow, 22.0, 12.0);
    let hero = run.players.get_mut(&1).unwrap();
    (hero.x, hero.y) = (12.0, 12.0);
    for _ in 0..14 * HZ {
        let hero = run.players.get_mut(&1).unwrap();
        hero.hp = hero.max_hp;
        run.step(&BTreeMap::new());
        if !run.holes.is_empty() {
            break;
        }
    }
    assert_eq!(run.holes.len(), 1);
    assert!(run.cues.iter().any(|c| c == "black_hole"));
    let hollow = run
        .enemies
        .iter()
        .find(|e| e.kind == EnemyKind::Hollow)
        .unwrap();
    assert_eq!(hollow.stage, 1, "it holds still to keep the hole open");
}
