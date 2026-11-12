//! What an open floor keeps of what happened on it (`rules::chronicle`).

use super::*;
use std::collections::BTreeMap;

fn open_run(depth: u32) -> Run {
    let mut run = Run::new(21, 7, None);
    run.join_seat(2, "Wren");
    run.open_floor_for_test(depth);
    run
}

fn traces(run: &Run) -> &[Trace] {
    &run.dungeon.open.as_ref().expect("an open floor").traces
}

fn remains_of(run: &Run, kind: EnemyKind) -> Vec<(f32, f32)> {
    traces(run)
        .iter()
        .filter_map(|t| match t {
            Trace::Remains { kind: k, x, y, .. } if *k == kind => Some((*x, *y)),
            _ => None,
        })
        .collect()
}

/// Knight 1 in the middle of a fight chamber, so its company wakes; the
/// chamber's index.
fn into_a_fight(run: &mut Run) -> usize {
    let o = run.dungeon.open.clone().unwrap();
    let fight = (0..o.chambers.len()).find(|&i| o.chambers[i].kind == RoomKind::Fight).unwrap();
    let (cx, cy) = o.chambers[fight].centre();
    let spot = run.open_spots(o.map, (cx, cy), 1)[0];
    for hero in run.players.values_mut() {
        hero.invulnerable = 100_000;
    }
    (run.players.get_mut(&1).unwrap().x, run.players.get_mut(&1).unwrap().y) = spot;
    run.step(&BTreeMap::new());
    fight
}

#[test]
fn the_dead_leave_their_remains_where_they_fell() {
    let mut run = open_run(2);
    let fight = into_a_fight(&mut run);
    assert!(traces(&run).is_empty(), "a floor starts bare");
    let fallen: Vec<(EnemyKind, f32, f32)> = run
        .enemies
        .iter_mut()
        .filter(|e| e.home == Some(fight as u8) && leaves_remains(e.kind))
        .map(|e| {
            e.hp = 0;
            (e.kind, e.x, e.y)
        })
        .collect();
    assert!(!fallen.is_empty(), "the chamber has a company with bodies");
    run.step(&BTreeMap::new());
    for &(kind, x, y) in &fallen {
        assert!(
            remains_of(&run, kind).iter().any(|&(rx, ry)| (rx - x).hypot(ry - y) < 2.5),
            "{kind:?} fell at {x},{y} and left nothing: {:?}",
            traces(&run)
        );
    }
    assert!(traces(&run).len() <= fallen.len());
    assert!(traces(&run).iter().all(|t| t.born() > 0));
}

#[test]
fn a_spirit_a_prop_and_a_ward_leave_nothing() {
    for kind in [EnemyKind::Wraith, EnemyKind::Wisp, EnemyKind::Dummy, EnemyKind::Ward, EnemyKind::Whirlgrave] {
        assert!(!leaves_remains(kind), "{kind:?}");
    }
    for kind in [EnemyKind::Goblin, EnemyKind::Skeleton, EnemyKind::Slime, EnemyKind::Imp, EnemyKind::Boss, EnemyKind::Dragon] {
        assert!(leaves_remains(kind), "{kind:?}");
    }
}

#[test]
fn remains_crowd_no_closer_than_two_to_a_spot() {
    let mut run = open_run(2);
    let (x, y) = run.dungeon.open.as_ref().unwrap().chambers[1].centre();
    run.tick = 50;
    run.leave_remains(&[(EnemyKind::Skeleton, x, y); 5]);
    assert_eq!(remains_of(&run, EnemyKind::Skeleton).len(), 2);
    run.leave_remains(&[(EnemyKind::Skeleton, x + 6.0, y)]);
    assert_eq!(remains_of(&run, EnemyKind::Skeleton).len(), 3, "a body a little way off is its own");
}

#[test]
fn rooms_and_the_kingdom_keep_no_remains() {
    let mut run = Run::new(5, 7, None);
    run.leave_remains(&[(EnemyKind::Skeleton, 10.0, 10.0)]);
    run.leave_cairn(1);
    assert!(run.dungeon.open.is_none(), "an ordinary room is no open floor");
    // A side room off an open floor: the floor's traces stay as they were.
    let mut open = open_run(2);
    let door = open.dungeon.open.as_ref().unwrap().doors[0];
    open.enter_for_test(door.room);
    open.leave_remains(&[(EnemyKind::Skeleton, 10.0, 10.0)]);
    assert!(traces(&open).is_empty());
}

#[test]
fn a_knight_who_goes_down_leaves_a_cairn_in_their_name() {
    let mut run = open_run(2);
    run.tick = 100;
    let (x, y) = {
        let h = &run.players[&2];
        (h.x, h.y)
    };
    {
        let hero = run.players.get_mut(&2).unwrap();
        (hero.invulnerable, hero.hp) = (0, 3);
    }
    run.hurt_for_test(2, 50);
    assert_eq!(run.players[&2].hp, 0);
    let cairns: Vec<&Trace> = traces(&run).iter().filter(|t| matches!(t, Trace::Cairn { .. })).collect();
    assert_eq!(cairns.len(), 1);
    match cairns[0] {
        Trace::Cairn { knight, name, x: cx, y: cy, born } => {
            assert_eq!(*knight, 2);
            assert_eq!(name, "WREN");
            assert!((cx - x).hypot(cy - y) < 0.01, "where they fell");
            assert!(*born > 0);
        }
        _ => unreachable!(),
    }
    // Standing again changes nothing: the cairn is where they fell.
    run.players.get_mut(&2).unwrap().hp = 5;
    run.step(&BTreeMap::new());
    assert_eq!(traces(&run).len(), 1);
}

#[test]
fn a_knight_keeps_three_cairns_and_the_oldest_gives_way() {
    let mut run = open_run(2);
    for n in 0..5u64 {
        run.tick = 100 + n * 10;
        run.players.get_mut(&2).unwrap().x = 20.0 + n as f32 * 3.0;
        run.leave_cairn(2);
    }
    let mine: Vec<u64> = traces(&run).iter().filter_map(|t| if let Trace::Cairn { knight: 2, born, .. } = t { Some(*born) } else { None }).collect();
    assert_eq!(mine, vec![120, 130, 140]);
    run.leave_cairn(1);
    assert_eq!(traces(&run).len(), CAIRNS_EACH + 1, "another knight's cairn is their own");
}

#[test]
fn a_full_floor_drops_its_oldest_remains_and_keeps_the_cairns() {
    let mut run = open_run(2);
    run.tick = 10;
    run.leave_cairn(1);
    for n in 0..(TRACES + 30) {
        run.tick = 20 + n as u64;
        run.leave_remains(&[(EnemyKind::Goblin, 3.0 + (n % 40) as f32 * 4.0, 3.0 + (n / 40) as f32 * 4.0)]);
    }
    let all = traces(&run);
    assert_eq!(all.len(), TRACES);
    assert!(all.iter().any(|t| matches!(t, Trace::Cairn { knight: 1, .. })), "the cairn outlives the remains");
    let oldest = all.iter().filter(|t| matches!(t, Trace::Remains { .. })).map(Trace::born).min().unwrap();
    assert!(oldest > 20, "the first remains are gone");
    assert!(valid(all));
}

#[test]
fn a_bad_trace_is_refused() {
    let one = |x: f32| Trace::Remains { kind: EnemyKind::Goblin, x, y: 1.0, born: 1 };
    assert!(valid(&[one(2.0)]));
    assert!(!valid(&[one(f32::NAN)]));
    assert!(!valid(&vec![one(1.0); TRACES + 1]));
    let long = Trace::Cairn { knight: 1, name: "X".repeat(NAME_LEN + 1), x: 1.0, y: 1.0, born: 1 };
    assert!(!valid(&[long]));
}

#[test]
fn an_old_save_loads_and_the_traces_round_trip_on_disk_and_wire() {
    let mut run = open_run(3);
    // A save from before traces: no such field anywhere in the floor.
    let mut save = serde_json::to_value(&run).unwrap();
    assert!(save["dungeon"]["open"].get("traces").is_none(), "an empty floor writes none");
    for chamber in save["dungeon"]["open"]["chambers"].as_array_mut().unwrap() {
        assert!(["sealed_at", "cleared_at"].iter().all(|k| chamber.get(k).is_none()));
    }
    let back: Run = serde_json::from_value(save).unwrap();
    assert!(back.valid_snapshot());
    assert!(traces(&back).is_empty());
    // With traces, a save reads back as it was, and so does the wire.
    run.tick = 77;
    let (x, y) = run.dungeon.open.as_ref().unwrap().chambers[0].centre();
    run.leave_remains(&[(EnemyKind::Skeleton, x, y), (EnemyKind::Boss, x + 9.0, y)]);
    run.leave_cairn(2);
    assert_eq!(traces(&run).len(), 3);
    let again: Run = serde_json::from_str(&serde_json::to_string(&run).unwrap()).unwrap();
    assert!(again.valid_snapshot());
    assert_eq!(traces(&again), traces(&run));
    let tree = crate::wire::publish(&run).unwrap();
    let client = crate::wire::read(&tree).unwrap();
    assert!(crate::wire::drawable(&client));
    assert_eq!(traces(&client), traces(&run));
}

#[test]
fn a_chamber_notes_the_tick_it_shut_and_the_tick_it_was_won() {
    let mut run = open_run(2);
    let o = run.dungeon.open.clone().unwrap();
    let g = usize::from(o.guardian);
    assert!(o.chambers.iter().all(|c| c.sealed_at == 0 && c.cleared_at == 0));
    // The arrival chamber has a knight in it from the first tick.
    run.step(&BTreeMap::new());
    let first = &run.dungeon.open.as_ref().unwrap().chambers[0];
    assert!(first.seen && first.seen_at > 0, "seen at tick {}", first.seen_at);
    assert!(first.cleared && first.cleared_at == 0, "the quiet arrival is cleared and never hushed");
    assert!(run.dungeon.open.as_ref().unwrap().chambers[1..].iter().all(|c| c.seen_at == 0), "no one has been in the rest");
    let (cx, cy) = o.chambers[g].centre();
    let inside = run.open_spots(o.map, (cx, cy + 4.0), 1)[0];
    (run.players.get_mut(&1).unwrap().x, run.players.get_mut(&1).unwrap().y) = inside;
    for hero in run.players.values_mut() {
        hero.invulnerable = 100_000;
    }
    run.step(&BTreeMap::new());
    let sealed_at = run.dungeon.open.as_ref().unwrap().chambers[g].sealed_at;
    assert!(sealed_at > 0 && sealed_at <= run.tick, "shut at tick {sealed_at}");
    assert!(run.shake >= 6, "the slam shakes the view: {}", run.shake);
    for e in run.enemies.iter_mut().filter(|e| e.home == Some(g as u8)) {
        e.hp = 0;
    }
    run.step(&BTreeMap::new());
    let done = &run.dungeon.open.as_ref().unwrap().chambers[g];
    assert!(done.cleared && done.stirred && done.cleared_at >= sealed_at, "won at tick {}", done.cleared_at);
    // And the guardian's remains lie where it fell.
    assert!(traces(&run).iter().any(|t| matches!(t, Trace::Remains { kind: EnemyKind::Boss, .. })), "{:?}", traces(&run));
}
