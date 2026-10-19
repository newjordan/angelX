use super::*;
use crate::ui::viz::agentviz::StageSnapshot;

fn wave(sequence: u64, name: &str, agents: Vec<String>, seat_states: Vec<SeatState>) -> ActivitySnapshot {
    ActivitySnapshot {
        sequence,
        stage: Some(StageSnapshot {
            stage_id: sequence,
            name: name.into(),
            agents,
            seat_states,
            seq: sequence,
        }),
    }
}

#[test]
fn presentation_is_utf8_safe_and_bounded() {
    let agents = (0..(MAX_SEATS + 3))
        .map(|index| format!("seat-{index}-{}", "🛰".repeat(20)))
        .collect();
    let mut council = Council::painted();
    council.note_activity(&wave(91, &format!("{}tail", "界".repeat(30)), agents, Vec::new()));
    let shown = council.presentation().expect("stage presents");
    assert_eq!(shown.seats.len(), MAX_SEATS);
    assert_eq!(shown.omitted_seats, 3);
    assert!(shown.stage.len() <= MAX_STAGE_BYTES);
    assert!(
        shown
            .seats
            .iter()
            .all(|seat| seat.label.len() <= MAX_SEAT_LABEL_BYTES)
    );
}

#[test]
fn seat_updates_refresh_the_table_and_a_new_stage_replaces_it() {
    let agents = || vec!["a".to_string(), "b".to_string(), "c".to_string()];
    let mut council = Council::painted();
    assert!(council.presentation().is_none(), "no stage, no table");
    council.note_activity(&wave(5, "proposer wave 2", agents(), Vec::new()));
    let shown = council.presentation().expect("stage presents");
    assert_eq!(shown.stage, "proposer wave 2");
    assert_eq!(shown.deed, Deed::Council);

    // Two seats come back under a newer sequence of the same stage.
    council.note_activity(&wave(
        6,
        "proposer wave 2",
        agents(),
        vec![SeatState::Returned, SeatState::Failed],
    ));
    let shown = council.presentation().expect("stage still presents");
    let states: Vec<_> = shown.seats.iter().map(|seat| seat.state).collect();
    assert_eq!(
        states,
        [SeatState::Returned, SeatState::Failed, SeatState::Running],
        "seats without a published update are still at work"
    );

    // A genuinely new stage replaces the council.
    council.note_activity(&wave(7, "judge", vec!["j1".into()], Vec::new()));
    let shown = council.presentation().expect("judge presents");
    assert_eq!(shown.stage, "judge");
    assert_eq!(shown.deed, Deed::Judge);
    assert_eq!(shown.seats.len(), 1);

    // The stage ending clears the table.
    council.note_activity(&ActivitySnapshot {
        sequence: 8,
        stage: None,
    });
    assert!(council.presentation().is_none());
}

#[test]
fn only_a_live_council_follows_the_activity() {
    assert!(!Council::discover(false).is_live());
    assert!(
        !Council::discover(true).is_live(),
        "tests never follow the process-global activity"
    );
    assert!(Council::painted().is_live());
}
