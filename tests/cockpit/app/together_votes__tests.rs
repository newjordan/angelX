//! The playtest's votes in the cockpit's realm and its file.

use crate::drive::together_shooter::*;
use crate::drive::together_shooter::votes::*;
use crate::drive::together_shooter::votes::testing::*;

#[test]
fn old_realm_run_and_guest_inputs_load_and_votes_round_trip() {
    let old: crate::drive::together_realm::Realm =
        serde_json::from_str(r#"{"treasury":{}}"#).unwrap();
    assert!(old.votes.is_empty());
    let old_input =
        r#"{"move_x":0,"move_y":0,"aim_x":0,"aim_y":0,"fire":false,"dash":false,"bomb":false}"#;
    assert!(!serde_json::from_str::<Input>(old_input).unwrap().skip_plate);
    assert!(
        serde_json::from_str::<Input>(old_input)
            .unwrap()
            .vote
            .is_none()
    );
    assert!(
        serde_json::to_value(Input::default())
            .unwrap()
            .get("vote")
            .is_none()
    );
    assert!(
        serde_json::to_value(Input::default())
            .unwrap()
            .get("skip_plate")
            .is_none()
    );
    let input = Input {
        skip_plate: true,
        vote: Some(Command {
            serial: 1,
            action: Action::Toggle,
        }),
        ..Default::default()
    };
    let wire = serde_json::to_value(input).unwrap();
    assert_eq!(wire["skip_plate"], true);
    assert!(wire.get("vote").is_some());
    assert_eq!(serde_json::from_value::<Input>(wire).unwrap(), input);
    let mut run = Run::new(91, 7, Some("Friend"));
    let mut old_run = serde_json::to_value(&run).unwrap();
    old_run.as_object_mut().unwrap().remove("playtest");
    assert!(
        serde_json::from_value::<Run>(old_run)
            .unwrap()
            .playtest
            .records
            .is_empty()
    );
    command(&mut run, 2, Action::Toggle);
    command(&mut run, 2, Action::Rate(4));
    let mut realm = crate::drive::together_realm::Realm::default();
    realm.votes = run.playtest.records.clone();
    let loaded: crate::drive::together_realm::Realm =
        serde_json::from_slice(&serde_json::to_vec(&realm).unwrap()).unwrap();
    assert_eq!(loaded.votes["Friend"][ITEMS[0].id], Rating::Stars(4));
    assert!(summary(&loaded.votes).contains("New controls: 4.0/5 (1)"));
    let mirror = run.wire_copy();
    assert!(mirror.playtest.open(2));
    assert_eq!(mirror.playtest.records, run.playtest.records);
}

#[test]
fn votes_append_plain_jsonl_changes_including_clear_and_untried() {
    let dir = std::env::temp_dir().join(format!(
        "votes-export-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    let path = dir.join("playtest/votes-0.2.2.jsonl");
    let mut run = Run::new(91, 7, Some("Friend"));
    command(&mut run, 2, Action::Toggle);
    command(&mut run, 2, Action::Rate(4));
    crate::drive::together_files::append_votes(&path, &run.playtest.changes).unwrap();
    run.playtest.changes.clear();
    command(&mut run, 2, Action::Untried);
    command(&mut run, 2, Action::Clear);
    crate::drive::together_files::append_votes(&path, &run.playtest.changes).unwrap();
    let lines: Vec<serde_json::Value> = std::fs::read_to_string(&path)
        .unwrap()
        .lines()
        .map(|s| serde_json::from_str(s).unwrap())
        .collect();
    assert_eq!(lines.len(), 3);
    assert_eq!(lines[0]["player"], "Friend");
    assert_eq!(lines[0]["item"], ITEMS[0].id);
    assert!(lines[0]["time"].as_u64().unwrap() > 0);
    assert_eq!(lines[0]["stars"], 4);
    assert_eq!(lines[1]["stars"], "untried");
    assert!(lines[2]["stars"].is_null());
    std::fs::remove_dir_all(dir).unwrap();
}
