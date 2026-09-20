use super::*;

#[test]
fn dwell_bounce_and_fit_are_clock_deterministic() {
    let at = |ms| window("abcdef", 5, Duration::from_millis(ms));
    assert_eq!(at(0), " abc…");
    assert_eq!(at(1_000), at(0));
    assert_eq!(at(1_160), "…bcd…");
    assert_eq!(at(1_480), "…def ");
    assert_eq!(at(2_480), at(1_480));
    assert_eq!(at(2_640), "…cde…");
    assert_eq!(at(2_960), at(0));
    assert_eq!(window("abc", 3, Duration::MAX), "abc");
}

#[test]
fn unicode_windows_never_split_clusters_or_overrun_cells() {
    for text in ["", "e\u{301}界語🦀xyz", "👩‍🔬ab🇯🇵cd", "界界界"] {
        for width in 0..12 {
            for ms in (0..6_000).step_by(80) {
                let output = window(text, width, Duration::from_millis(ms));
                assert!(
                    UnicodeWidthStr::width(output.as_str()) <= width,
                    "{output:?}"
                );
                assert!(!output.starts_with('\u{301}'));
                if output.contains('👩') {
                    assert!(output.contains("👩‍🔬"), "{output:?}");
                }
            }
        }
    }
}

#[test]
fn pause_hidden_resize_and_static_do_not_catch_up() {
    let now = Instant::now();
    let text = "abcdef";
    let mut roll = RollingText::new(text.into());
    for ms in (0..=1_280).step_by(160) {
        roll.render(5, now + Duration::from_millis(ms), true, false);
    }
    let saved = roll.render(5, now + Duration::from_millis(1_300), true, true);
    assert_eq!(
        roll.render(5, now + Duration::from_secs(8), true, true),
        saved
    );
    assert_eq!(
        roll.render(5, now + Duration::from_secs(9), true, false),
        saved
    );
    assert_eq!(
        roll.render(5, now + Duration::from_secs(20), true, false),
        saved
    );
    assert_eq!(roll.render(6, now, true, false), text);
    assert_eq!(roll.render(5, now, false, false), "abcd…");
}
