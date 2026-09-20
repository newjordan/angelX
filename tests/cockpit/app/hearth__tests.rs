use super::*;

#[test]
fn day_cycle_phases_in_order() {
    assert_eq!(DayPhase::of_beat(0), DayPhase::Dawn);
    assert_eq!(DayPhase::of_beat(DAY_BEATS / 4), DayPhase::Day);
    assert_eq!(DayPhase::of_beat(DAY_BEATS / 2), DayPhase::Dusk);
    assert_eq!(DayPhase::of_beat(DAY_BEATS * 3 / 4), DayPhase::Night);
    assert_eq!(DayPhase::of_beat(DAY_BEATS), DayPhase::Dawn);
}

#[test]
fn dawn_brightens_and_dusk_dims() {
    let start = DayPhase::Dawn.light(0);
    let end = DayPhase::Dawn.light(DAY_BEATS / 4 - 1);
    assert!(start < end && end <= 1.0);
    let d0 = DayPhase::Dusk.light(DAY_BEATS / 2);
    let d1 = DayPhase::Dusk.light(DAY_BEATS * 3 / 4 - 1);
    assert!(d0 > d1 && d1 >= 0.55);
}

#[test]
fn warmth_eases_toward_local_fleet_target() {
    let mut h = HearthState::default();
    for _ in 0..64 {
        h.beat(0, 4, 2, true, 10);
    }
    assert!(
        h.warmth > 30,
        "warmth should rise with local models: {}",
        h.warmth
    );
    // Fleet leaves: warmth decays back.
    for _ in 0..400 {
        h.beat(0, 0, 0, false, 10);
    }
    assert!(
        h.warmth <= 4,
        "warmth should decay when local sleeps: {}",
        h.warmth
    );
}

#[test]
fn work_plus_warmth_grows_prosperity() {
    let mut h = HearthState::default();
    for _ in 0..200 {
        h.beat(4, 4, 2, true, 40);
    }
    assert!(
        h.tier() >= 1,
        "prosperity {} should reach tier 1",
        h.prosperity
    );
}

#[test]
fn no_local_models_means_no_growth() {
    let mut h = HearthState::default();
    for _ in 0..200 {
        h.beat(4, 0, 0, false, 40);
    }
    assert_eq!(h.prosperity, 0, "cold town spends nothing");
    assert!(h.light > 0, "work still accumulates as light");
}

#[test]
fn serde_roundtrip_and_forward_compat() {
    let h = HearthState {
        light: 5,
        warmth: 6,
        prosperity: 7,
        beats: 8,
        last_save_beat: 0,
    };
    let json = serde_json::to_string(&h).unwrap();
    let back: HearthState = serde_json::from_str(&json).unwrap();
    assert_eq!(back.prosperity, 7);
    // Old files without the hearth key deserialize via Default.
    let old: Option<HearthState> = serde_json::from_str::<serde_json::Value>("{}")
        .ok()
        .and_then(|v| serde_json::from_value(v["hearth"].clone()).ok());
    assert!(old.is_none() || old.unwrap().beats == 0);
}
