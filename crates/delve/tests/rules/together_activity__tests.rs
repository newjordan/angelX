use super::*;
use angel_net::doc::value::{self, Value};

#[test]
fn every_game_names_fields_the_run_has_and_a_name_of_its_own() {
    let run = Run::new(3, 9, None);
    let Ok(Value::Struct(fields)) = value::to_value(&run) else {
        panic!("a run is a struct");
    };
    // Optional activity fields may be absent while inactive.
    let live = value::to_value(&Run::forest_road_for_test()).unwrap();
    let adventures: Vec<_> = roundtable::ADVENTURES
        .iter()
        .map(|a| value::to_value(&Run::roundtable_for_test(a.slug).unwrap()).unwrap())
        .collect();
    let mut names = std::collections::BTreeSet::new();
    for activity in all() {
        assert!(names.insert(activity.name), "{} twice", activity.name);
        for field in activity.fields {
            assert!(
                fields.iter().any(|(n, _)| n == field)
                    || live.field(field).is_some()
                    || adventures.iter().any(|run| run.field(field).is_some()),
                "{}: no Run field {field}",
                activity.name
            );
        }
    }
}

#[test]
fn a_game_ended_is_off_and_its_fields_are_clear() {
    let mut run = Run::new(3, 9, None);
    for activity in all() {
        (activity.end)(&mut run);
        for field in activity.fields {
            let tree = value::to_value(&run).unwrap();
            assert!(
                matches!(tree.field(field), Some(Value::None) | None),
                "{} left {field} standing",
                activity.name
            );
        }
    }
    assert!(!run.activities_on().contains(&"the Wax Maze"));
}

#[test]
fn a_race_hall_takes_every_key_and_the_quiet_floor_takes_none() {
    let mut run = Run::new(3, 9, None);
    let keys = BTreeMap::from([(
        1,
        Input {
            move_x: 1,
            ..Default::default()
        },
    )]);
    assert!(run.ride(&keys).is_none(), "no ride on a quiet floor");
    // The first floor that holds a race, walked into.
    let mut raced = (0..80u64)
        .find_map(|seed| {
            let mut r = Run::new(seed, 1, None);
            for _ in 0..6 {
                r.descend_for_test();
                if r.race3d_room.is_some() {
                    return Some(r);
                }
            }
            None
        })
        .unwrap();
    raced.enter_for_test(raced.race3d_room.unwrap());
    assert!(raced.activities_on().contains(&"a race hall"));
    assert_eq!(raced.ride(&keys), Some(BTreeMap::new()));
}
