use super::*;

#[test]
fn empty_rects_are_not_published() {
    let mut frames = PanelFrames::default();
    frames.push(PanelKind::Header, Rect::new(0, 0, 0, 3));
    frames.push(PanelKind::Footer, Rect::new(0, 0, 10, 0));
    assert!(frames.is_empty(), "zero-area panels must be dropped");
}

#[test]
fn get_returns_the_last_drawn_rect() {
    let mut frames = PanelFrames::default();
    let r = Rect::new(2, 1, 40, 12);
    frames.push(PanelKind::Transcript, r);
    assert_eq!(frames.get(PanelKind::Transcript), Some(r));
    assert_eq!(frames.get(PanelKind::Shell), None);
    assert_eq!(frames.len(), 1);
}

#[test]
fn names_are_stable_lowercase_tags() {
    assert_eq!(PanelKind::AgentBay.name(), "agent");
    assert_eq!(PanelKind::Transcript.name(), "transcript");
    assert_eq!(PanelKind::Input.name(), "input");
}
