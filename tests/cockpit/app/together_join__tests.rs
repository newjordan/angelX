use super::*;

#[test]
fn invitations_parse_with_or_without_the_command_and_scheme() {
    let token = "0123456789abcdef0123456789abcdef";
    for link in [
        format!("http://100.94.1.2:8767/#{token}"),
        format!("/dungeon join http://100.94.1.2:8767/#{token}"),
        format!("100.94.1.2:8767/#{token}"),
    ] {
        let (base, got) = parse_link(&link).unwrap();
        assert_eq!(base, "http://100.94.1.2:8767");
        assert_eq!(got, token);
    }
    assert!(parse_link("http://100.94.1.2:8767/").is_err());
    assert!(parse_link("http://100.94.1.2:8767/#short").is_err());
}

#[test]
fn whole_stream_rejects_bad_boss_index_without_replacing_mirror() {
    let previous = Run::new(17, 1, None);
    let fresh = Run::new(29, 2, None);
    assert!(previous.valid_snapshot());
    assert!(fresh.valid_snapshot());

    let shared = Mutex::new(Shared::default());
    {
        let mut state = shared.lock().unwrap();
        state.before = previous.pose();
        state.mirror = Some(previous.clone());
        state.ticked = Some(Instant::now());
    }
    let (before_pose, before_ticked) = {
        let state = shared.lock().unwrap();
        (format!("{:?}", state.before), state.ticked)
    };

    let mut invalid_value = serde_json::to_value(&fresh).unwrap();
    invalid_value["boss_gates"]["leaders"][0]["boss"] = serde_json::json!(u8::MAX);
    let invalid_run: Run = serde_json::from_value(invalid_value.clone()).unwrap();
    assert!(!invalid_run.valid_snapshot());
    let invalid_line = serde_json::to_vec(&serde_json::json!({"whole": invalid_value})).unwrap();
    take_line(&invalid_line, &shared);

    {
        let state = shared.lock().unwrap();
        assert_eq!(state.mirror.as_ref().unwrap().raid_id, previous.raid_id);
        assert_eq!(format!("{:?}", state.before), before_pose);
        assert_eq!(state.ticked, before_ticked);
    }

    let fresh_line = serde_json::to_vec(&serde_json::json!({
        "whole": serde_json::to_value(&fresh).unwrap()
    }))
    .unwrap();
    take_line(&fresh_line, &shared);
    let state = shared.lock().unwrap();
    assert_eq!(state.mirror.as_ref().unwrap().raid_id, fresh.raid_id);
    assert_eq!(format!("{:?}", state.before), format!("{:?}", fresh.pose()));
    assert!(state.ticked.is_some());
}
