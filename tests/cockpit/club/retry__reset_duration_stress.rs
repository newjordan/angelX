use super::*;

#[test]
fn never_panics_on_hostile_reset_headers() {
    // A hostile/broken upstream can send these on the 200-success path of
    // every request; `Duration::from_secs_f64` would panic on non-finite or
    // overflowing input. All must degrade quietly (no panic).
    for v in [
        "inf",
        "-inf",
        "nan",
        "1e400",
        "1e309",
        "1e30",
        "99999999999999999999999999999",
        "1e400s",
        "999999999999999999999999999999s",
    ] {
        let _ = parse_reset_duration(v);
    }
    assert!(parse_reset_duration("inf").is_none());
    assert!(parse_reset_duration("1e400").is_none());
    // Sane values still parse correctly.
    assert_eq!(parse_reset_duration("6"), Some(Duration::from_secs(6)));
    assert_eq!(parse_reset_duration("1m30s"), Some(Duration::from_secs(90)));
}
