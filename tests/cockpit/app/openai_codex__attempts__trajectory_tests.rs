use super::*;
use crate::club::Club;
use std::io::Read;
#[test]
fn trajectory_c03c_oauth_attempt_bytes_and_usage_are_sealed_once() {
    let _lock = crate::tests::env_lock();
    let club = crate::openai_codex::tests::club();
    crate::harness::reset_turn_ledger(&club);
    crate::harness::note_timing_origin(Instant::now());
    crate::harness::begin_model_request();
    let wire = br#"{"model":"fixture"}"#;
    let response = "data: π\n\n";
    {
        let mut attempt = Attempt::new(&club, wire);
        let mut body = String::new();
        attempt
            .response_reader(response.as_bytes())
            .read_to_string(&mut body)
            .unwrap();
        attempt.receive(r#"data: {"type":"response.completed","response":{"usage":{"input_tokens":7,"output_tokens":2}}}"#, false);
    }
    let samples = crate::harness::provider_call_samples();
    assert_eq!(samples.len(), 1);
    assert_eq!(samples[0]["request_bytes"], wire.len());
    assert_eq!(samples[0]["response_bytes"], response.len());
    assert_eq!(samples[0]["usage"][0], 7);
    assert_eq!(club.token_usage().unwrap().turns, 1);
    crate::harness::end_model_request();
}
