use super::{MAX_LOOP_INTERVAL_SECS, parse_interval};

#[test]
fn clamps_absurd_intervals_and_never_overflows() {
    // `u64::MAX` seconds parses fine, then `n * mult` overflows (panic in
    // debug / wrap in release) and `Instant + Duration::from_secs(n)` panics.
    // The clamp (`saturating_mul().min(MAX)`) must tame all three suffixes.
    let huge = u64::MAX.to_string();
    assert!(parse_interval(&format!("{huge}s")).unwrap() <= MAX_LOOP_INTERVAL_SECS);
    assert!(parse_interval(&huge).unwrap() <= MAX_LOOP_INTERVAL_SECS);
    assert!(parse_interval(&format!("{huge}h")).unwrap() <= MAX_LOOP_INTERVAL_SECS);
    // Numbers beyond u64 simply fail to parse — also safe (no interval set).
    assert!(parse_interval("99999999999999999999999").is_none());
    // Ordinary values are unaffected.
    assert_eq!(parse_interval("300"), Some(300));
    assert_eq!(parse_interval("5m"), Some(300));
    assert_eq!(parse_interval("2h"), Some(7200));
}
