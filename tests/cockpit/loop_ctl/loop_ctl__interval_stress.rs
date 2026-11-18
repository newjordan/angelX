use super::{MAX_LOOP_INTERVAL_SECS, parse_interval};

#[test]
fn clamps_absurd_intervals_and_never_overflows() {
    let huge = u64::MAX.to_string();
    assert!(parse_interval(&format!("{huge}s")).unwrap() <= MAX_LOOP_INTERVAL_SECS);
    assert!(parse_interval(&huge).unwrap() <= MAX_LOOP_INTERVAL_SECS);
    assert!(parse_interval(&format!("{huge}h")).unwrap() <= MAX_LOOP_INTERVAL_SECS);
    assert!(parse_interval("99999999999999999999999").is_none());
    assert_eq!(parse_interval("300"), Some(300));
    assert_eq!(parse_interval("5m"), Some(300));
    assert_eq!(parse_interval("2h"), Some(7200));
}
