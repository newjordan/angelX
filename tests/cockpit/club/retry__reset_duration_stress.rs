use super::*;

#[test]
fn never_panics_on_hostile_reset_headers() {
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
    assert_eq!(parse_reset_duration("6"), Some(Duration::from_secs(6)));
    assert_eq!(parse_reset_duration("1m30s"), Some(Duration::from_secs(90)));
}
