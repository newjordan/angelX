//! The campaign's counters come from the board alone, bounded: a missing,
//! malformed or hostile board reads as zeros, whole.

use super::*;

/// A board as the cartridge writes it, with `quest` as given.
fn board(quest: &str) -> Vec<u8> {
    format!(r#"{{"schema":"vec.board/1","updated":"2026-10-20T21:05:00Z","boards":[],"quest":{quest},"events":[]}}"#)
        .into_bytes()
}

#[test]
fn a_board_reads_its_seven_counters_and_ignores_the_rest() {
    let c = Campaign::parse(&board(
        r#"{"vec_data_staged":1,"vec_heart_proxy_runs":7,"vec_heart_submitted":1,
            "vec_heart_interp_above_floor":0,"vec_heart_extrap_above_floor":1,
            "vec_gata4_above_floor":0,"vec_heart_top10":0,"vec_someday_new":9}"#,
    ));
    assert_eq!(c.count("vec_data_staged"), 1);
    assert_eq!(c.count("vec_heart_proxy_runs"), 7);
    assert_eq!(c.count("vec_heart_extrap_above_floor"), 1);
    assert_eq!(c.count("vec_heart_top10"), 0);
    // A counter the realm does not know is not kept, and reads as 0.
    assert_eq!(c.count("vec_someday_new"), 0);
    assert_eq!(c.counts().count(), COUNTERS.len());
}

#[test]
fn a_missing_counter_is_zero_and_the_rest_still_count() {
    let c = Campaign::parse(&board(r#"{"vec_data_staged":1}"#));
    assert_eq!(c.count("vec_data_staged"), 1);
    assert_eq!(c.count("vec_heart_proxy_runs"), 0);
    assert!(Campaign::parse(&board("{}")).is_empty());
}

#[test]
fn a_missing_or_malformed_board_reads_as_zeros() {
    let zeros = Campaign::default();
    for bad in [
        &b""[..],
        b"not json",
        b"null",
        b"[]",
        b"{}",
        // The quest object missing, or not an object.
        br#"{"schema":"vec.board/1"}"#,
        br#"{"schema":"vec.board/1","quest":[1,2,3]}"#,
        br#"{"schema":"vec.board/1","quest":"vec_data_staged"}"#,
        // Another schema is another contract; no schema is none.
        br#"{"schema":"vec.board/2","quest":{"vec_data_staged":1}}"#,
        br#"{"quest":{"vec_data_staged":1}}"#,
        // Cut off mid-write (the cartridge renames, but a copy may not).
        br#"{"schema":"vec.board/1","quest":{"vec_data_staged":1"#,
    ] {
        assert_eq!(Campaign::parse(bad), zeros, "{}", String::from_utf8_lossy(bad));
    }
}

#[test]
fn a_hostile_board_reads_as_zeros_whole_never_in_part() {
    // Each of these also carries a good counter: none of it is kept.
    let zeros = Campaign::default();
    for value in [
        "-1",
        "-0.5",
        "1.5",
        "1.0",
        "1e3",
        r#""1""#,
        "true",
        "null",
        "[1]",
        r#"{"n":1}"#,
        "1000001",
        "4294967296",
        "18446744073709551616",
        "99999999999999999999999999",
    ] {
        let raw = board(&format!(r#"{{"vec_data_staged":1,"vec_heart_top10":{value}}}"#));
        assert_eq!(Campaign::parse(&raw), zeros, "vec_heart_top10 = {value}");
    }
    // The largest count it takes is a million.
    let c = Campaign::parse(&board(r#"{"vec_heart_proxy_runs":1000000}"#));
    assert_eq!(c.count("vec_heart_proxy_runs"), 1_000_000);
}

#[test]
fn a_huge_or_deep_board_reads_as_zeros_without_a_panic() {
    let zeros = Campaign::default();
    // Over the weight limit, though every byte of it is a good board.
    let mut huge = board(r#"{"vec_data_staged":1}"#);
    huge.truncate(huge.len() - 1);
    huge.extend(std::iter::repeat_n(b' ', MAX_BYTES));
    huge.push(b'}');
    assert!(huge.len() > MAX_BYTES);
    assert_eq!(Campaign::parse(&huge), zeros);
    // Nested past what the reader will follow, inside and outside the quest.
    let deep = "[".repeat(100_000) + &"]".repeat(100_000);
    assert_eq!(Campaign::parse(deep.as_bytes()), zeros);
    let inside = board(&format!(r#"{{"vec_data_staged":1,"x":{}1{}}}"#, "[".repeat(5_000), "]".repeat(5_000)));
    assert_eq!(Campaign::parse(&inside), zeros);
    // Not text at all.
    let noise: Vec<u8> = (0..4096u32).map(|i| (i.wrapping_mul(2_654_435_761) >> 24) as u8).collect();
    assert_eq!(Campaign::parse(&noise), zeros);
}

#[test]
fn only_the_seven_are_the_campaigns() {
    for name in COUNTERS {
        assert!(is_counter(name) && reserved(name), "{name}");
    }
    assert!(!is_counter("vec_made_up") && reserved("vec_made_up"));
    assert!(!is_counter("one_leg_stands") && !reserved("one_leg_stands"));
}
