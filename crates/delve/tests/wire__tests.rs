use super::*;
use crate::render::overworld::arena::art_v2::testing::{fight, kingdom};
use crate::rules::exits::Entry;
use std::collections::BTreeMap;

fn read_back(tree: &Value) -> Run {
    read(tree).expect("the tree reads back into a run")
}

#[test]
fn the_published_tree_reads_back_into_the_same_run_without_its_dice() {
    for run in [fight(), kingdom(), Run::new(7, 1, None)] {
        let tree = publish(&run).unwrap();
        for name in PRIVATE {
            assert!(tree.field(name).is_none(), "{name} is the server's");
        }
        let back = read_back(&tree);
        assert!(drawable(&back));
        // Everything the tree carries comes back: the client's run, published
        // again, is the same tree.
        assert_eq!(publish(&back).unwrap(), tree);
        assert_eq!((back.tick, back.at, back.players.len()), (run.tick, run.at, run.players.len()));
    }
}

#[test]
fn marks_and_shards_go_to_the_clients_and_saves_are_as_they_were() {
    let mut run = kingdom();
    run.marks.insert("country:survey:2".into(), 1);
    run.pot_shards.push((10.0, 8.0, 5));
    let tree = publish(&run).unwrap();
    let back = read_back(&tree);
    assert_eq!(back.marks.get("country:survey:2"), Some(&1));
    assert_eq!(back.pot_shards, vec![(10.0, 8.0, 5)]);
    assert_eq!(publish(&back).unwrap(), tree);

    // A save is the run's own serde, unchanged: its dice kept, its marks
    // and shards left out, and it loads as it always has.
    let json: serde_json::Value = serde_json::to_value(&run).unwrap();
    let fields = json.as_object().unwrap();
    assert!(fields.contains_key("rng") && fields.contains_key("seed") && fields.contains_key("next_id"));
    assert!(!fields.contains_key("marks") && !fields.contains_key("pot_shards"));
    let saved: Run = serde_json::from_value(json.clone()).unwrap();
    assert!(saved.valid_snapshot() && saved.marks.is_empty());
    assert_eq!(serde_json::to_value(&saved).unwrap(), json, "the save round-trips whole");
}

#[test]
fn a_clients_copy_draws_the_merchant_and_the_pots_where_the_server_has_them() {
    // Both are derived from the run's seed, which the renderer reads.
    for seed in 1..40 {
        let run = Run::new(seed, 1, None);
        let back = read_back(&publish(&run).unwrap());
        assert_eq!(back.merchant_camp(), run.merchant_camp(), "seed {seed}");
        for room in 0..run.dungeon.rooms.len() {
            assert_eq!(back.pots_of(room), run.pots_of(room), "seed {seed}, room {room}");
        }
    }
}

#[test]
fn seats_are_knights_counted_from_one() {
    assert_eq!((knight_of(0), knight_of(3)), (1, 4));
    assert_eq!((seat_of(1), seat_of(4), seat_of(0), seat_of(5)), (Some(0), Some(3), None, None));
}

#[test]
fn a_seat_with_nothing_to_hide_borrows_the_published_tree() {
    let tree = publish(&fight()).unwrap();
    for seat in [None, Some(0), Some(1), Some(3)] {
        assert!(matches!(seat_tree(&tree, seat), Cow::Borrowed(_)), "{seat:?}");
    }
}

#[test]
fn a_seat_outside_the_partys_room_is_sent_none_of_its_shots_or_games() {
    let mut run = fight();
    assert!(!run.projectiles.is_empty(), "the fixture's fight has shots in the air");
    let home = run.at;
    run.exits.outside.insert(2, Entry { at: home, party: BTreeMap::new() });
    let mut tree = publish(&run).unwrap();
    // The seat outside (seat 1, knight 2): no shots.
    let view = seat_tree(&tree, Some(1));
    assert!(matches!(view.field("projectiles"), Some(Value::Seq(s)) if s.is_empty()));
    let back = read_back(&view);
    assert!(back.projectiles.is_empty() && drawable(&back));
    // Everyone else, and a watcher, sees the party's room as it is.
    for seat in [None, Some(0)] {
        let view = seat_tree(&tree, seat);
        assert!(matches!(view, Cow::Borrowed(_)));
    }
    // A game under way in the party's room is left out for the seat
    // outside, and only a game under way: a field that is absent stays so.
    let game = activity::all().flat_map(|a| a.fields.iter()).next().copied().unwrap();
    *tree.field_mut(game).unwrap() = Value::Some(Box::new(Value::Unit));
    assert_eq!(seat_tree(&tree, Some(1)).field(game), Some(&Value::None));
    assert!(matches!(seat_tree(&tree, Some(0)).field(game), Some(Value::Some(_))));
}

#[test]
fn another_knights_heat_is_theirs_alone() {
    let mut tree = publish(&kingdom()).unwrap();
    // Heats for knights 2 and 3 (their rounds stand in as plain values: the
    // filter reads only the keys).
    let heats = tree.field_mut("pastimes").and_then(|p| p.field_mut("heats")).unwrap();
    *heats = Value::Map(vec![(Value::U(2), Value::U(20)), (Value::U(3), Value::U(30))]);
    let heats_of = |seat: Option<u32>| match seat_tree(&tree, seat).field("pastimes").and_then(|p| p.field("heats")) {
        Some(Value::Map(m)) => m.iter().map(|(k, _)| k.as_id().unwrap()).collect::<Vec<_>>(),
        other => panic!("{other:?}"),
    };
    assert_eq!(heats_of(Some(1)), vec![2]);
    assert_eq!(heats_of(Some(2)), vec![3]);
    assert_eq!(heats_of(Some(0)), Vec::<u64>::new());
    assert_eq!(heats_of(None), Vec::<u64>::new(), "a watcher plays no heat");
}

#[test]
fn events_read_back_and_malformed_ones_do_not() {
    let events = [
        Event::Sound { hear: Hear::Party, name: "knights/a_cheer_1".into() },
        Event::Sound { hear: Hear::Knight(3), name: "gold".into() },
        Event::Say { hear: Hear::All, who: "blaise".into(), name: "Old Blaise".into(), words: "Mind the stair.".into() },
        Event::Notice { hear: Hear::All, text: "Gwen joined the party".into() },
    ];
    for e in &events {
        let bytes = e.to_vec();
        assert_eq!(Event::decode(&bytes).as_ref(), Ok(e));
        for n in 0..bytes.len() {
            assert!(Event::decode(&bytes[..n]).is_err(), "{e:?} cut at {n}");
        }
        let mut long = bytes.clone();
        long.push(0);
        assert!(Event::decode(&long).is_err());
    }
    assert!(Event::decode(&[9, 0, 0]).is_err(), "an unknown kind");
    assert!(Event::decode(&[1, 7, 0]).is_err(), "an unknown hearer");
    assert!(Event::decode(&[1, 0, 2, 0xff, 0xfe]).is_err(), "not UTF-8");
    // Overlong text is cut on the way out, between characters.
    let long = Event::Notice { hear: Hear::All, text: "é".repeat(400) };
    let Ok(Event::Notice { text, .. }) = Event::decode(&long.to_vec()) else { panic!() };
    assert_eq!(text.len(), 512);
}

#[test]
fn a_client_makes_shell_lines_of_what_it_hears() {
    let mut run = fight();
    let sound = Event::Sound { hear: Hear::Party, name: "knights/a_cheer_1".into() };
    assert_eq!(sound.line(Some(1), Some(&run)).as_deref(), Some("sfx knights-a_cheer_1"));
    assert_eq!(sound.line(None, Some(&run)).as_deref(), Some("sfx knights-a_cheer_1"));
    // Outside the party's room, its sounds are not heard.
    run.exits.outside.insert(2, Entry { at: run.at, party: BTreeMap::new() });
    assert_eq!(sound.line(Some(2), Some(&run)), None);
    assert!(sound.line(Some(1), Some(&run)).is_some());
    let mine = Event::Sound { hear: Hear::Knight(2), name: "gold".into() };
    assert_eq!(mine.line(Some(2), Some(&run)).as_deref(), Some("sfx gold"));
    assert_eq!(mine.line(Some(1), Some(&run)), None);
    assert_eq!(mine.line(None, Some(&run)), None);
    let say = Event::Say { hear: Hear::All, who: "wren".into(), name: "Wren".into(), words: "Two\nlines".into() };
    assert_eq!(say.line(None, None).as_deref(), Some("say Wren: Two lines"));
    // The run's sounds come out of it once, as the party's.
    run.sounds = vec!["hit", "knights/b_oof_2"];
    let taken = take_sounds(&mut run);
    assert!(run.sounds.is_empty());
    assert_eq!(taken[1], Event::Sound { hear: Hear::Party, name: "knights/b_oof_2".into() });
}

#[test]
fn params_read_back_and_a_newer_host_is_refused() {
    for art_v2 in [false, true] {
        let p = Params { art_v2 };
        assert_eq!(Params::decode(&p.encode()), Ok(p));
    }
    assert!(Params::decode(&[]).is_err());
    assert!(Params::decode(&[2, 0]).is_err(), "another version");
    assert!(Params::decode(&[1, 2]).is_err(), "a flag this client does not know");
    assert!(Params::decode(&[1, 0, 0]).is_err(), "bytes left over");
}

#[test]
fn a_shot_flying_straight_is_sent_as_it_starts_and_read_back_where_it_is() {
    use crate::rules::Input;
    use angel_net::doc::delta::{Patch, diff};
    // The busy fight: every shot and monster read back where the run has
    // it, and the client's run publishes the same tree.
    let mut run = fight();
    for _ in 0..40 {
        run.step(&BTreeMap::new());
        let tree = publish(&run).unwrap();
        let back = read_back(&tree);
        for (a, b) in run.projectiles.iter().zip(&back.projectiles) {
            assert!((a.x - b.x).abs() <= 1.0 / 256.0 && (a.y - b.y).abs() <= 1.0 / 256.0, "{} {} / {} {}", a.x, a.y, b.x, b.y);
            assert_eq!((a.ttl, a.vx, a.vy), (b.ttl, b.vx, b.vy));
        }
        for (a, b) in run.enemies.iter().zip(&back.enemies) {
            assert_eq!(a.age, b.age);
        }
        assert_eq!(publish(&back).unwrap(), tree);
    }
    // A knight's arrows in a quiet room: sent as they leave, then not again
    // while they fly on.
    let mut run = Run::new(3, 9, None);
    run.calm_for_test();
    let fire = Input { fire: true, aim_x: 1, ..Input::default() };
    run.step(&BTreeMap::from([(1, fire)]));
    assert!(!run.projectiles.is_empty());
    let mut before = publish(&run).unwrap();
    for _ in 0..5 {
        run.step(&BTreeMap::new());
        if run.tick.is_multiple_of(RECKON_SPAN) {
            break;
        }
        let tree = publish(&run).unwrap();
        let shots_moved = matches!(diff(&before, &tree), Some(Patch::Struct { set, .. }) if set.iter().any(|(n, _)| n == "projectiles"));
        assert!(!shots_moved || run.projectiles.is_empty(), "a straight shot is not sent again");
        before = tree;
    }
}

#[test]
fn shots_keep_their_ids_so_one_landing_moves_no_other() {
    use angel_net::doc::delta::{Patch, diff};
    let mut run = fight();
    let mut publisher = Publisher::default();
    let mut before = publisher.publish(&run).unwrap();
    let mut whole_shots = 0;
    let mut shot_ticks = 0;
    for _ in 0..60 {
        run.step(&BTreeMap::new());
        let tree = publisher.publish(&run).unwrap();
        // The ids read past: the client's run is the run.
        let back = read_back(&tree);
        assert_eq!(back.projectiles.len(), run.projectiles.len());
        for (a, b) in run.projectiles.iter().zip(&back.projectiles) {
            assert!((a.x - b.x).abs() <= 1.0 / 256.0 && a.kind == b.kind);
        }
        if let Some(Patch::Struct { set, .. }) = diff(&before, &tree)
            && let Some((_, Patch::Keyed { items, .. })) = set.iter().find(|(n, _)| n == "projectiles")
        {
            whole_shots += items.iter().filter(|(_, p)| matches!(p, Patch::Set(_))).count();
        }
        shot_ticks += run.projectiles.len();
        before = tree;
    }
    // Only shots that are new are sent whole.
    assert!(shot_ticks > 300);
    assert!(whole_shots * 5 < shot_ticks, "{whole_shots} of {shot_ticks} shot-ticks sent whole");
}
