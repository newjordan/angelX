use super::*;
use unicode_width::UnicodeWidthStr;

#[test]
fn glyph_marks_are_static_and_compact() {
    for glyph in Glyph::ALL {
        assert_eq!(glyph.mark().len(), 2);
        assert!(glyph.mark().is_ascii());
        assert_eq!(
            UnicodeWidthStr::width(glyph.rich_mark()),
            2,
            "{glyph:?}: {:?}",
            glyph.rich_mark()
        );
        assert!(!glyph.label().is_empty());
    }
    assert_eq!(Glyph::Active.mark(), "*>");
    assert_eq!(Glyph::Thinking.label(), "thinking");
    assert_eq!(Glyph::Online.mark(), "ON");
    assert_eq!(Glyph::Offline.mark(), "NO");
    assert_eq!(activity(false), Glyph::Idle);
    assert_eq!(activity(true), Glyph::Thinking);
}

#[test]
fn media_prefixes_map_known_artifact_kinds() {
    assert_eq!(media_prefix("image"), "I> img   ");
    assert_eq!(media_prefix("img"), "I> img   ");
    assert_eq!(media_prefix("video"), "V> reel  ");
    assert_eq!(media_prefix("link"), "L> link  ");
    assert_eq!(media_prefix("graph"), "G> graph ");
    assert_eq!(media_prefix("res"), "R> res   ");
    assert_eq!(media_prefix("unknown"), "L> link  ");
}

#[test]
fn rich_media_prefixes_remain_fixed_width() {
    for prefix in [
        "▧  img   ",
        "▶  reel  ",
        "↗  link  ",
        "⌁  graph ",
        "◫  res   ",
    ] {
        assert_eq!(UnicodeWidthStr::width(prefix), 9);
    }
}
