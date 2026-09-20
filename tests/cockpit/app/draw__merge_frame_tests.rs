#[test]
fn merge_frame_borders_reuses_scratch_rects() {
    let src = include_str!("../../../cockpit/src/draw.rs");
    let start = src
        .find("fn merge_frame_borders(")
        .expect("merge_frame_borders");
    let body = src[start..]
        .split("pub(crate) fn render_message_composer(")
        .next()
        .expect("body");
    assert!(
        src.contains("static MERGE_RECTS"),
        "junction repair must keep a rect scratch"
    );
    assert!(
        body.contains("rects.clear()") && body.contains("rects.extend("),
        "scratch must be reused, not rebuilt: {body}"
    );
    assert!(
        !body.contains(".collect()"),
        "merge_frame_borders must not collect a fresh Vec: {body}"
    );
}
