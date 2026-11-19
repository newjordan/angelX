use super::*;

/// The refusal body a metering gateway sent on 2026-11-19 (private run
/// `local-4af456627eb642d4`), in the shape the HTTP seat reports it. `first`
/// is the run's first refusal: a 286,573-byte request with a 32,000-token
/// output limit. `last` is the one that was then resent unchanged 774 times.
fn recorded(available: f64, required: f64, reserved_tokens: u64) -> String {
    let details = serde_json::json!({
        "error_schema_version": 1,
        "category": "correctable",
        "run_action": "continue",
        "action": "correct_request",
        "retryable": false,
        "available_usd": available,
        "required_reservation_usd": required,
        "budget_usd": 3.0,
        "reserved_tokens": reserved_tokens,
        "upstream_request_sent": false,
    });
    let mut body = serde_json::json!({
        "error": "Insufficient credits for this request's conservative reservation; \
                  shorten the input or lower an explicit output-token limit. \
                  This request was not sent to the provider.",
        "code": "model_reservation_exceeds_remaining_budget",
        "request_id": "b89eee154f924bda9f64775f652536d2",
    });
    let mut diagnostic = serde_json::json!({
        "code": "model_reservation_exceeds_remaining_budget",
        "request_id": "b89eee154f924bda9f64775f652536d2",
    });
    for (key, value) in details.as_object().unwrap() {
        body[key] = value.clone();
        diagnostic[key] = value.clone();
    }
    body["diagnostic"] = diagnostic;
    format!("HTTP 400: {body}")
}

fn first() -> String {
    recorded(1.4519148, 1.480356, 612_178)
}

fn last() -> String {
    recorded(0.3517859999999997, 0.368728, 176_172)
}

#[test]
fn the_recorded_refusal_is_read_with_its_amounts() {
    let refusal = reservation_refusal(&first()).expect("a reservation refusal");
    assert_eq!(
        refusal,
        ReservationRefusal {
            available: 1.4519148,
            required: 1.480356,
            reserved_tokens: Some(612_178),
            retryable: false,
        }
    );
    assert!(error_indicates_reservation_exceeds(&first()));
    assert!(!error_indicates_gateway_retryable(&first()));
    assert!(!error_indicates_gateway_stop(&first()));
}

#[test]
fn text_after_the_body_does_not_hide_the_amounts() {
    let reported = format!("{}; not sent again{}", last(), " [errors leg: native]");
    let refusal = reservation_refusal(&reported).expect("still a reservation refusal");
    assert_eq!(refusal.reserved_tokens, Some(176_172));
    assert_eq!(refusal.required, 0.368728);
}

#[test]
fn a_refusal_the_gateway_calls_retryable_is_waited_out_not_shrunk() {
    // Other calls hold the credit; the same request fits once they settle.
    let held = recorded(0.313, 0.938, 404_810).replace("\"retryable\":false", "\"retryable\":true");
    let refusal = reservation_refusal(&held).expect("a reservation refusal");
    assert!(refusal.retryable);
    assert!(error_indicates_gateway_retryable(&held));
    assert!(
        !error_indicates_reservation_exceeds(&held),
        "a retryable hold must not shrink the request"
    );
}

#[test]
fn the_word_in_prose_is_not_a_reservation_refusal() {
    for error in [
        "HTTP 503: capacity reservation for this model is exhausted, try later",
        "HTTP 400: {\"error\":{\"code\":\"invalid_request\",\"message\":\"no reservation found\"}}",
        "transport error after 3 attempts: reservation of socket failed",
    ] {
        assert!(reservation_refusal(error).is_none(), "{error}");
        assert!(!error_indicates_reservation_exceeds(error), "{error}");
    }
    // A reservation code without amounts is still a reservation refusal; it
    // is corrected without the gateway's numbers.
    let bare = "HTTP 400: {\"diagnostic\":{\"code\":\"model_reservation_exceeds_remaining_budget\",\"run_action\":\"continue\",\"retryable\":false}}";
    assert!(reservation_refusal(bare).is_none());
    assert!(error_indicates_reservation_exceeds(bare));
}

#[test]
fn the_corrected_request_is_sized_to_half_the_credit_left() {
    let _lock = crate::tests::env_lock();
    let _share = crate::tests::TestEnvGuard::unset("ANGEL_RESERVATION_FIT_SHARE");
    let _floor = crate::tests::TestEnvGuard::unset("ANGEL_RESERVATION_OUTPUT_FLOOR");

    let refusal = reservation_refusal(&first()).unwrap();
    let keep = refusal.keep_fraction();
    assert!((keep - 0.5 * 1.4519148 / 1.480356).abs() < 1e-12, "{keep}");
    // Scaling input size and output limit by the same fraction scales the
    // hold by it, whatever the two prices are: it fits half the credit left.
    assert!(refusal.required * keep <= 0.5 * refusal.available + 1e-12);
    assert_eq!(refusal.output_limit(32_000, reservation_output_floor()), 15_692);
    assert_eq!(refusal.context_budget(80_000), 39_231);
}

#[test]
fn the_output_limit_is_never_pushed_below_the_floor() {
    let _lock = crate::tests::env_lock();
    let _share = crate::tests::TestEnvGuard::unset("ANGEL_RESERVATION_FIT_SHARE");
    let _floor = crate::tests::TestEnvGuard::unset("ANGEL_RESERVATION_OUTPUT_FLOOR");

    assert_eq!(reservation_output_floor(), 8192);
    // Almost no credit left: the fraction is tiny, the limit stops at the floor.
    let starved = reservation_refusal(&recorded(0.02, 0.9, 400_000)).unwrap();
    assert_eq!(starved.keep_fraction(), 0.05);
    assert_eq!(starved.output_limit(32_000, 8192), 8192);
    assert_eq!(starved.output_limit(16_000, 8192), 8192);
    // A limit already at or under the floor is left alone, never raised.
    assert_eq!(starved.output_limit(8192, 8192), 8192);
    assert_eq!(starved.output_limit(4000, 8192), 4000);
    assert_eq!(starved.context_budget(1000), 256);
}

#[test]
fn a_correction_counts_only_when_the_request_got_smaller() {
    let before = reservation_refusal(&first()).unwrap();
    let smaller = reservation_refusal(&recorded(1.4519148, 0.9, 380_000)).unwrap();
    assert!(smaller.smaller_than(&before));
    assert!(!before.smaller_than(&smaller));

    // The 2026-11-19 wedge: the same request, refused again and again.
    let wedged = reservation_refusal(&last()).unwrap();
    let again = reservation_refusal(&last()).unwrap();
    assert!(
        !again.smaller_than(&wedged),
        "an unchanged request is not a correction"
    );

    // Without a size from the gateway, the amount held decides.
    let mut unsized_before = before.clone();
    unsized_before.reserved_tokens = None;
    assert!(smaller.smaller_than(&unsized_before));
    assert!(!unsized_before.smaller_than(&unsized_before.clone()));
}

#[test]
fn the_seats_note_of_a_lower_output_limit_is_read_and_hides_nothing() {
    let noted = format!("{} {RESERVATION_LIMIT_LOWERED}8192]", last());
    assert!(error_notes_lower_output_limit(&noted));
    assert!(!error_notes_lower_output_limit(&last()));
    assert!(error_indicates_reservation_exceeds(&noted));
    assert_eq!(
        reservation_refusal(&noted),
        reservation_refusal(&last()),
        "the note rides after the body and changes none of its amounts"
    );
}
