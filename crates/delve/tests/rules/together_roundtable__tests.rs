//! The five Round Table adventures' shared shell: one list, five distinct
//! doors on the kingdom map, crash-safe marks, five knobs each. The
//! integrator's file: a builder's own checks go in `together_<slug>__tests.rs`.
use super::*;
use crate::render::overworld::kingdom as geography;
use crate::world::map::{MAP_H, MAP_W, SCREEN_H, SCREEN_W};
use angel_net::doc::value::Value;

fn press_e_at(adventure: &Adventure) -> Run {
    let mut r = Run::at_home(41, 1, None, Default::default(), Default::default());
    r.walk_kingdom(Some(adventure.entry));
    r.step(&BTreeMap::new());
    r.step(&BTreeMap::from([(
        1,
        Input {
            interact: true,
            ..Default::default()
        },
    )]));
    r
}

#[test]
fn the_five_stand_in_order_each_with_a_name_a_slot_and_a_door_of_its_own() {
    let slugs: Vec<_> = ADVENTURES.iter().map(|a| a.slug).collect();
    assert_eq!(
        slugs,
        [
            "lanternward",
            "moonfall",
            "bannerpeak",
            "griffinwing",
            "lightgallop"
        ]
    );
    for (i, a) in ADVENTURES.iter().enumerate() {
        assert_eq!(a.slot, i, "{}", a.slug);
        assert_eq!(roundtable::adventure(a.slug).map(|b| b.name), Some(a.name));
        assert!(
            a.numbers
                .iter()
                .all(|n| n.name.starts_with(&format!("{}_", a.slug))),
            "{}",
            a.slug
        );
    }
    let names: std::collections::BTreeSet<_> = ADVENTURES.iter().map(|a| a.name).collect();
    assert_eq!(names.len(), 5);
    assert!(roundtable::adventure("nowhere").is_none());
}

#[test]
fn doors_are_distinct_walkable_and_clear_of_every_other_door_and_errand() {
    let geo = map::geography(&[]);
    let manhattan = |a: (i32, i32), b: (i32, i32)| (a.0 - b.0).abs() + (a.1 - b.1).abs();
    let mut taken: Vec<(String, (i32, i32))> = geo
        .sites
        .iter()
        .map(|s| (s.name.clone(), s.stand))
        .collect();
    taken.extend(
        classes::stones(&geo)
            .into_iter()
            .map(|(i, p)| (format!("banner stone {i}"), p)),
    );
    taken.extend(
        trinkets::things(&geo)
            .into_iter()
            .map(|(_, p)| ("a kingdom thing".to_string(), p)),
    );
    if let Some((herald, posts)) = crown::herald(&geo) {
        taken.push(("the herald".into(), herald));
        taken.extend(posts.into_iter().map(|p| ("a herald post".to_string(), p)));
    }
    taken.push(("the north forest road".into(), forest_road::NORTH));
    taken.push(("Old Oak".into(), forest_road::ENT_TILE));
    for a in &ADVENTURES {
        let (x, y) = a.entry;
        assert!(
            x > 1 && y > 1 && x < MAP_W - 2 && y < MAP_H - 2,
            "{} is on the map",
            a.slug
        );
        assert_eq!(
            geo.room.tile(x, y),
            Tile::Floor,
            "{} stands on open ground",
            a.slug
        );
        let open = (-1..=1)
            .flat_map(|dy| (-1..=1).map(move |dx| (dx, dy)))
            .filter(|&(dx, dy)| geo.room.tile(x + dx, y + dy) == Tile::Floor)
            .count();
        assert!(open >= 6, "{} has room for a table round it", a.slug);
        // An authored screen: the party's wishes build in the wild between.
        assert!(
            (x / SCREEN_W) % 2 == 0 && (y / SCREEN_H) % 2 == 0,
            "{} is not in a wild screen",
            a.slug
        );
        for (what, at) in &taken {
            assert!(
                manhattan(a.entry, *at) >= 6,
                "{} is too near {what} {at:?}",
                a.slug
            );
        }
        for b in ADVENTURES.iter().filter(|b| b.slot != a.slot) {
            assert!(
                manhattan(a.entry, b.entry) >= 6,
                "{} and {} share a corner",
                a.slug,
                b.slug
            );
        }
    }
    // Every door can be walked to from the Keep.
    let mut seen = std::collections::BTreeSet::new();
    let mut queue = std::collections::VecDeque::from([geo.sites[0].stand]);
    seen.insert(geo.sites[0].stand);
    while let Some((x, y)) = queue.pop_front() {
        for p in [(x + 1, y), (x - 1, y), (x, y + 1), (x, y - 1)] {
            if p.0 >= 0
                && p.1 >= 0
                && p.0 < MAP_W
                && p.1 < MAP_H
                && geo.room.tile(p.0, p.1) == Tile::Floor
                && seen.insert(p)
            {
                queue.push_back(p);
            }
        }
    }
    for a in &ADVENTURES {
        assert!(seen.contains(&a.entry), "{} can be walked to", a.slug);
    }
}

#[test]
fn e_at_each_door_opens_that_adventure_alone_and_elsewhere_opens_nothing() {
    for a in &ADVENTURES {
        let r = press_e_at(a);
        assert_eq!(r.roundtable_under_way().map(|b| b.slug), Some(a.slug));
        assert_eq!(ADVENTURES.iter().filter(|b| (b.showing)(&r)).count(), 1);
        assert_eq!(
            r.kingdom.return_to.as_ref().map(|b| b.name.as_str()),
            Some(a.name)
        );
        assert_eq!(
            r.activities_on().iter().filter(|n| **n == a.name).count(),
            1
        );
        // The fixture is the same door, the same way in.
        assert_eq!(
            Run::roundtable_for_test(a.slug)
                .unwrap()
                .roundtable_under_way()
                .map(|b| b.slug),
            Some(a.slug)
        );
    }
    let mut r = Run::at_home(41, 1, None, Default::default(), Default::default());
    r.walk_kingdom(Some(geography::keep_stand()));
    r.step(&BTreeMap::new());
    r.step(&BTreeMap::from([(
        1,
        Input {
            interact: true,
            ..Default::default()
        },
    )]));
    assert!(!r.roundtable_showing());
    assert!(Run::roundtable_for_test("nowhere").is_none());
}

#[test]
fn a_press_still_down_from_the_last_door_opens_nothing_until_it_is_let_go() {
    let a = &ADVENTURES[0];
    let mut r = Run::at_home(41, 1, None, Default::default(), Default::default());
    r.walk_kingdom(Some(a.entry));
    let held = BTreeMap::from([(
        1,
        Input {
            interact: true,
            ..Default::default()
        },
    )]);
    for _ in 0..HZ {
        r.step(&held);
    }
    assert!(
        !r.roundtable_showing(),
        "E held through the walk in is for the door behind"
    );
    r.step(&BTreeMap::new());
    r.step(&held);
    assert!(r.roundtable_showing());
}

#[test]
fn entering_any_room_closes_every_adventure() {
    for a in &ADVENTURES {
        let mut r = Run::roundtable_for_test(a.slug).unwrap();
        assert!(r.roundtable_showing());
        r.leave_kingdom_place();
        assert!(!r.roundtable_showing(), "{}", a.slug);
        assert!(ADVENTURES.iter().all(|b| (b.valid)(&r)));
    }
}

#[test]
fn every_fixture_is_a_run_the_shell_accepts() {
    // What a session may not hold is each adventure's own check
    // (`a_session_is_bounded_and_repair_mends_it` in its test file).
    for a in &ADVENTURES {
        let r = Run::roundtable_for_test(a.slug).unwrap();
        assert!(
            roundtable::valid(&r) && r.valid_snapshot() && crate::wire::drawable(&r),
            "{}",
            a.slug
        );
        assert_eq!((a.seated)(&r), vec![1], "{}", a.slug);
    }
    assert!(roundtable::valid(&Run::new(3, 9, None)));
}

#[test]
fn marks_are_each_an_adventures_own_and_the_journal_is_sized_for_them() {
    let all: Vec<&str> = ADVENTURES
        .iter()
        .flat_map(|a| a.marks.iter().copied())
        .collect();
    assert_eq!(all.len(), roundtable::INTENT_MARKS);
    assert_eq!(
        all.iter().collect::<std::collections::BTreeSet<_>>().len(),
        all.len(),
        "no mark twice"
    );
    for a in &ADVENTURES {
        assert!(
            a.marks
                .iter()
                .all(|m| m.starts_with(&format!("{}:", a.slug))),
            "{}",
            a.slug
        );
        for m in a.marks {
            assert!(
                roundtable::intent_key(m)
                    && countryside::intent_key(m)
                    && countryside::intent_valid(m, 1)
            );
            assert!(!countryside::intent_valid(m, 2) && !countryside::intent_valid(m, 0));
        }
    }
    assert!(!roundtable::intent_key("lanternward:anything-else"));
    assert!(countryside::INTENT_LIMIT >= college::INTENT_LIMIT + roundtable::INTENT_MARKS);
}

#[test]
fn five_knobs_each_stand_at_their_start_and_a_moved_one_survives_the_wire() {
    let tune = tune::Tune::default();
    let knobs = tune.knobs();
    for a in &ADVENTURES {
        let mine: Vec<_> = knobs.iter().filter(|k| k.group == a.name).collect();
        assert_eq!(mine.len(), 5, "{}", a.slug);
        for (k, n) in mine.iter().zip(a.numbers.iter()) {
            assert_eq!(
                (k.name, k.default, k.value),
                (n.name, f64::from(n.start), f64::from(n.start))
            );
            assert!(k.min <= k.default && k.default <= k.max);
        }
    }
    assert_eq!(roundtable::starts()[2][3], ADVENTURES[2].numbers[3].start);
    let mut run = Run::new(3, 1, None);
    let n = ADVENTURES[1].numbers[1];
    let moved = if n.start == n.max { n.min } else { n.max };
    assert!(run.tune.set(n.name, f64::from(moved)));
    assert_eq!(roundtable::number(&run.tune, 1, 1), moved);
    let back: Run = serde_json::from_str(&serde_json::to_string(&run).unwrap()).unwrap();
    assert_eq!(back.tune.roundtable[1][1], moved);
    run.tune.roundtable[0][0] = u32::MAX;
    assert!(!run.tune.valid() && !run.valid_snapshot());
}

#[test]
fn the_activity_registry_names_each_and_the_wire_hides_it_from_a_seat_outside() {
    let names: Vec<_> = activity::all().map(|a| a.name).collect();
    for a in &ADVENTURES {
        assert_eq!(
            names.iter().filter(|n| **n == a.name).count(),
            1,
            "{}",
            a.slug
        );
        assert!(
            activity::all().any(|x| x.name == a.name && x.fields == [a.slug] && x.ends_outside)
        );
    }
    let mut r = Run::moonfall_for_test();
    r.join_seat(2, "Friend");
    assert!(r.leave_activity(2));
    let tree = crate::wire::publish(&r).unwrap();
    // Seat 1 plays knight 2, who stands outside; seat 0 plays knight 1, inside.
    let outside = crate::wire::seat_tree(&tree, Some(1));
    assert_eq!(
        outside.field("moonfall"),
        Some(&Value::None),
        "a seat outside is sent no game"
    );
    let inside = crate::wire::seat_tree(&tree, Some(0));
    assert!(matches!(inside.field("moonfall"), Some(Value::Some(_))));
}
