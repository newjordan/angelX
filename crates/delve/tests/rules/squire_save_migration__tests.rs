use super::*;
use serde_json::{json, Value};

fn head() -> squires::Companion {
    squires::Companion {
        owner: 1,
        squire: squires::Squire { id: 41, kind: squires::Kind::Shield, name: "Head".into(), level: 3 },
        body: hireling::Hireling::new(2, (4.0, 5.0)),
        smoke: 7,
        vigil: 8,
        rescued: true,
    }
}

fn country(owner: u32) -> countryside_squires::Companion {
    countryside_squires::Companion {
        owner,
        recruit: countryside_squires::Squire { id: 93, name: "Incoming".into(), kind: countryside_squires::Kind::Shieldbearer, level: 4, drills: 6 },
        body: hireling::Hireling::new(3, (6.0, 7.0)),
        room: 3, depth: 4, ability: 5, smoke: 6, trick: 7, marked: Some(8),
    }
}

fn save() -> Value { serde_json::to_value(Run::new(7, 1, Some("Pip"))).unwrap() }

#[test]
fn head_save_is_unchanged_and_round_trips() {
    let mut original = save();
    original["squires"] = json!([head()]);
    let run: Run = serde_json::from_value(original.clone()).unwrap();
    assert_eq!(run.squires, vec![head()]);
    assert!(run.countryside_squires.is_empty());
    let saved = serde_json::to_value(&run).unwrap();
    assert_eq!(saved, original);
    let again: Run = serde_json::from_value(saved).unwrap();
    assert_eq!(again.squires, vec![head()]);
}

#[test]
fn incoming_save_migrates_all_companion_fields_and_round_trips() {
    let mut original = save();
    let incoming = json!([country(1), country(2)]);
    original["squires"] = incoming.clone();
    let run: Run = serde_json::from_value(original).unwrap();
    assert!(run.squires.is_empty());
    assert_eq!(serde_json::to_value(&run.countryside_squires).unwrap(), incoming);
    let saved = serde_json::to_value(&run).unwrap();
    assert!(saved.get("squires").is_none());
    assert_eq!(saved["countryside_squires"], incoming);
    let again: Run = serde_json::from_value(saved).unwrap();
    assert_eq!(serde_json::to_value(&again.countryside_squires).unwrap(), incoming);
}

#[test]
fn mixed_and_existing_companions_are_preserved_and_malformed_rejected() {
    let mut original = save();
    original["squires"] = json!([head(), country(1), head(), country(2)]);
    original["countryside_squires"] = json!([country(3)]);
    let run: Run = serde_json::from_value(original.clone()).unwrap();
    assert_eq!(run.squires, vec![head(), head()]);
    assert_eq!(serde_json::to_value(&run.countryside_squires).unwrap(), json!([country(3), country(1), country(2)]));
    let saved = serde_json::to_value(&run).unwrap();
    let again: Run = serde_json::from_value(saved.clone()).unwrap();
    assert_eq!(serde_json::to_value(again).unwrap(), saved);
    original["squires"][1].as_object_mut().unwrap().remove("recruit");
    assert!(serde_json::from_value::<Run>(original).is_err());
}

#[test]
fn network_value_round_trip_keeps_both_shapes() {
    let mut run = Run::new(7, 1, Some("Pip"));
    run.squires.push(head());
    run.countryside_squires.push(country(1));
    let tree = angel_net::doc::value::to_value(&run).unwrap();
    let restored: Run = angel_net::doc::value::from_value(&tree).unwrap();
    assert_eq!(restored.squires, run.squires);
    assert_eq!(serde_json::to_value(restored.countryside_squires).unwrap(), json!([country(1)]));
}
