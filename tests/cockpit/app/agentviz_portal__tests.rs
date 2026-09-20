use super::*;
use crate::ui::viz::agentviz::StageSnapshot;

fn fixture(name: &str) -> AngelVizStateV1 {
    let raw = match name {
        "normal" => include_str!("../../../cockpit/fixtures/agentviz-portal/normal-v1.json"),
        "empty" => include_str!("../../../cockpit/fixtures/agentviz-portal/empty-v1.json"),
        "oversized" => include_str!("../../../cockpit/fixtures/agentviz-portal/oversized-v1.json"),
        "stale" => include_str!("../../../cockpit/fixtures/agentviz-portal/stale-v1.json"),
        "invalid" => include_str!("../../../cockpit/fixtures/agentviz-portal/invalid-v1.json"),
        _ => panic!("unknown fixture {name}"),
    };
    serde_json::from_str(raw).expect("fixture must be syntactically valid JSON")
}

#[test]
fn projection_is_utf8_safe_and_bounded() {
    let agents = (0..(MAX_SEATS + 3))
        .map(|index| format!("seat-{index}-{}", "🛰".repeat(20)))
        .collect();
    let activity = ActivitySnapshot {
        sequence: 91,
        stage: Some(StageSnapshot {
            stage_id: 91,
            name: format!("{}tail", "界".repeat(30)),
            agents,
            seat_states: Vec::new(),
            seq: 91,
        }),
    };
    let packet = AngelVizStateV1::project(
        &activity,
        12_345,
        AngelVizHealthV1::Nominal,
        AngelVizPaletteV1::Noir,
    );
    packet.validate().expect("projected packet must validate");
    assert_eq!(packet.seats.len(), MAX_SEATS);
    assert_eq!(packet.status.omitted_seats, 3);
    assert!(packet.stage.as_ref().unwrap().len() <= MAX_STAGE_BYTES);
    assert!(
        packet
            .seats
            .iter()
            .all(|seat| seat.label.len() <= MAX_SEAT_LABEL_BYTES)
    );
    assert!(serde_json::to_vec(&packet).unwrap().len() <= MAX_PACKET_BYTES);
}

#[test]
fn normal_and_empty_fixtures_validate() {
    fixture("normal").validate().unwrap();
    fixture("empty").validate().unwrap();
}

#[test]
fn oversized_and_wrong_version_fixtures_are_rejected() {
    assert_eq!(
        fixture("oversized").validate(),
        Err(PacketError::StageTooLong)
    );
    assert_eq!(
        fixture("invalid").validate(),
        Err(PacketError::WrongSchema { actual: 99 })
    );
}

#[test]
fn latest_slot_coalesces_and_rejects_stale_sequences() {
    let mut slot = LatestStateSlot::default();
    slot.publish(fixture("stale")).unwrap();
    slot.publish(fixture("normal")).unwrap();
    assert_eq!(slot.counts(), (2, 1, 0));
    let latest = slot.take_latest().unwrap();
    assert_eq!(latest.sequence, 42);
    assert_eq!(
        slot.publish(fixture("stale")),
        Err(PacketError::StaleSequence {
            previous: 42,
            incoming: 41,
        })
    );
    assert_eq!(slot.counts(), (2, 1, 1));
}

#[test]
fn seat_updates_refresh_returned_count_and_carry_the_frame() {
    use crate::ui::viz::agentviz::SeatState;
    let wave = |sequence: u64, seat_states: Vec<SeatState>| ActivitySnapshot {
        sequence,
        stage: Some(StageSnapshot {
            stage_id: 5,
            name: "proposer wave 2".into(),
            agents: vec!["a".into(), "b".into(), "c".into()],
            seat_states,
            seq: sequence,
        }),
    };
    let mut runtime = PortalRuntime::with_renderer(None);
    runtime.note_activity(&wave(5, Vec::new()));
    let shown = runtime.presentation().expect("stage presents");
    assert_eq!((shown.active_seats, shown.returned_seats), (3, 0));

    // A frame lands for the current sequence, then two seats come back:
    // the returned count refreshes and the frame rides the seq bump.
    runtime.frame = Some(PortalFrame {
        sequence: 5,
        pixels: Arc::from(vec![0u8; 4]),
    });
    runtime.note_activity(&wave(6, vec![SeatState::Returned, SeatState::Failed]));
    let shown = runtime.presentation().expect("stage still presents");
    assert_eq!(shown.returned_seats, 1, "only Returned counts as back");
    assert!(
        shown.frame.is_some(),
        "the frame carries across a same-stage seat bump"
    );

    // A genuinely new stage drops the stale frame as before.
    runtime.note_activity(&ActivitySnapshot {
        sequence: 7,
        stage: Some(StageSnapshot {
            stage_id: 7,
            name: "judge".into(),
            agents: vec!["j1".into()],
            seat_states: Vec::new(),
            seq: 7,
        }),
    });
    let shown = runtime.presentation().expect("judge presents");
    assert_eq!((shown.active_seats, shown.returned_seats), (1, 0));
    assert!(
        shown.frame.is_none(),
        "a new stage never wears an old frame"
    );
}

#[test]
fn fixed_frame_contract_stays_within_preview_budget() {
    assert_eq!(FRAME_BYTES, 230_400);
    assert!(
        FRAME_WIDTH.saturating_mul(FRAME_HEIGHT) <= 120_000,
        "portal frame must stay within the current inline preview pixel budget"
    );
}

fn renderer_output(sequence: u64, pixels: &[u8]) -> Vec<u8> {
    let receipt = serde_json::json!({
        "schema_version": SCHEMA_VERSION,
        "sequence": sequence,
        "width": FRAME_WIDTH,
        "height": FRAME_HEIGHT,
        "format": "rgba8-srgb",
        "frame_bytes": pixels.len(),
        "production_time_us": 1234,
        "adapter": "test adapter",
        "error": null
    });
    let receipt = serde_json::to_vec(&receipt).unwrap();
    let mut output = (receipt.len() as u32).to_be_bytes().to_vec();
    output.extend_from_slice(&receipt);
    output.extend_from_slice(pixels);
    output
}

#[test]
fn renderer_receipt_accepts_only_the_fixed_frame_contract() {
    let pixels = vec![7; FRAME_BYTES];
    let output = renderer_output(42, &pixels);
    let frame = parse_renderer_output(42, &output).unwrap();
    assert_eq!(frame.sequence, 42);
    assert_eq!(frame.pixels.len(), FRAME_BYTES);

    assert!(
        parse_renderer_output(41, &output)
            .unwrap_err()
            .contains("expected 41")
    );
    let mut trailing = output;
    trailing.push(0);
    assert!(
        parse_renderer_output(42, &trailing)
            .unwrap_err()
            .contains("expected 230400")
    );
}

#[test]
fn renderer_error_receipt_cannot_smuggle_a_frame() {
    let receipt = serde_json::json!({
        "schema_version": SCHEMA_VERSION,
        "sequence": 9,
        "width": FRAME_WIDTH,
        "height": FRAME_HEIGHT,
        "format": "rgba8-srgb",
        "frame_bytes": 0,
        "production_time_us": 50,
        "adapter": null,
        "error": {"code": "no_adapter", "message": "none found"}
    });
    let receipt = serde_json::to_vec(&receipt).unwrap();
    let mut output = (receipt.len() as u32).to_be_bytes().to_vec();
    output.extend_from_slice(&receipt);
    assert_eq!(
        parse_renderer_output(9, &output).unwrap_err(),
        "no_adapter: none found"
    );
    output.push(1);
    assert_eq!(
        parse_renderer_output(9, &output).unwrap_err(),
        "renderer error receipt included frame bytes"
    );
}
