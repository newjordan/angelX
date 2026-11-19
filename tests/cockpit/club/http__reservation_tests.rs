use super::*;

fn seat() -> HttpClub {
    HttpClub::new(
        "reservation-test",
        "http://127.0.0.1:9/v1".to_string(),
        "test-model".to_string(),
        None,
    )
}

fn refusal(available: f64, required: f64, reserved_tokens: u64) -> ReservationRefusal {
    ReservationRefusal {
        available,
        required,
        reserved_tokens: Some(reserved_tokens),
        retryable: false,
    }
}

fn capped(club: &HttpClub, mut body: serde_json::Value) -> serde_json::Value {
    club.apply_reservation_cap(&mut body);
    body
}

#[test]
fn the_output_limit_is_scaled_from_the_gateways_amounts() {
    let _lock = crate::tests::env_lock();
    let _share = crate::tests::TestEnvGuard::unset("ANGEL_RESERVATION_FIT_SHARE");
    let _floor = crate::tests::TestEnvGuard::unset("ANGEL_RESERVATION_OUTPUT_FLOOR");

    let club = seat();
    assert_eq!(
        capped(&club, serde_json::json!({"max_tokens": 32_000}))["max_tokens"],
        32_000,
        "no refusal, no ceiling"
    );
    // The first refusal recorded on 2026-11-19: 1.480356 needed, 1.4519148 left.
    assert_eq!(
        club.note_reservation_refusal(Some(32_000), Some(&refusal(1.4519148, 1.480356, 612_178))),
        Some(15_692),
        "the seat says what it lowered the limit to"
    );
    assert_eq!(
        capped(&club, serde_json::json!({"max_tokens": 32_000}))["max_tokens"],
        15_692
    );
    // A later, looser reading never raises it: credit only falls. A request
    // that went out above the ceiling still goes out lower next time.
    assert_eq!(
        club.note_reservation_refusal(Some(32_000), Some(&refusal(1.4, 1.41, 600_000))),
        Some(15_692)
    );
    assert_eq!(
        capped(&club, serde_json::json!({"max_tokens": 32_000}))["max_tokens"],
        15_692
    );
    // A refusal of the request that went out at the ceiling scales again.
    assert_eq!(
        club.note_reservation_refusal(Some(15_692), Some(&refusal(1.4, 1.41, 600_000))),
        Some(8192)
    );
    assert_eq!(
        capped(&club, serde_json::json!({"max_tokens": 32_000}))["max_tokens"],
        8192
    );
}

#[test]
fn the_ceiling_stops_at_the_floor_where_last_night_it_fell_to_2048() {
    let _lock = crate::tests::env_lock();
    let _share = crate::tests::TestEnvGuard::unset("ANGEL_RESERVATION_FIT_SHARE");
    let _floor = crate::tests::TestEnvGuard::unset("ANGEL_RESERVATION_OUTPUT_FLOOR");

    let club = seat();
    // Refusal after refusal, each for the limit the last one left.
    let mut sent = 32_000u64;
    for _ in 0..8 {
        club.note_reservation_refusal(Some(sent), Some(&refusal(0.35, 0.37, 176_172)));
        sent = capped(&club, serde_json::json!({"max_tokens": 32_000}))["max_tokens"]
            .as_u64()
            .unwrap();
    }
    assert_eq!(sent, 8192, "the limit must stop at the floor");
    assert_eq!(
        club.note_reservation_refusal(Some(8192), Some(&refusal(0.35, 0.37, 176_172))),
        None,
        "at the floor nothing is lowered, and the seat says so"
    );

    // Without the gateway's amounts the step is a halving, to the same floor.
    let bare = seat();
    bare.note_reservation_refusal(Some(32_000), None);
    assert_eq!(
        capped(&bare, serde_json::json!({"max_tokens": 32_000}))["max_tokens"],
        16_000
    );
    bare.note_reservation_refusal(Some(16_000), None);
    bare.note_reservation_refusal(Some(8192), None);
    assert_eq!(
        capped(&bare, serde_json::json!({"max_tokens": 32_000}))["max_tokens"],
        8192
    );
}

#[test]
fn the_ceiling_lowers_the_limit_under_the_name_the_request_used() {
    let _lock = crate::tests::env_lock();
    let _share = crate::tests::TestEnvGuard::unset("ANGEL_RESERVATION_FIT_SHARE");
    let _floor = crate::tests::TestEnvGuard::unset("ANGEL_RESERVATION_OUTPUT_FLOOR");

    let club = seat();
    club.note_reservation_refusal(Some(32_000), None);

    let chat = capped(&club, serde_json::json!({"max_completion_tokens": 32_000}));
    assert_eq!(chat["max_completion_tokens"], 16_000);
    assert!(
        chat.get("max_tokens").is_none(),
        "a second limit field is a rejected request: {chat}"
    );

    let small = capped(&club, serde_json::json!({"max_tokens": 4000}));
    assert_eq!(small["max_tokens"], 4000, "a smaller ask is left alone");

    // No stated limit: the wire's own default is what went out, so it is
    // stated now at the ceiling.
    let unstated = capped(&club, serde_json::json!({"messages": []}));
    assert_eq!(unstated["max_tokens"], 16_000);
}
