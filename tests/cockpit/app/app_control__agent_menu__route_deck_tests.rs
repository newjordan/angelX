use super::*;
use crate::club::Bag;
use crate::viewer::Viewer;
use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

fn seed_app() -> App {
    let mut app = App::new(
        Bag::for_render_test(&[
            ("alpha", &[("model-a", true), ("model-b", true)]),
            ("beta", &[("model-c", true)]),
        ]),
        Viewer::new(),
    );
    app.bag.select_route(0, 0);
    app
}

#[test]
fn arrow_keys_scroll_transcript_without_cycling_the_model() {
    let mut app = seed_app();
    let before = (app.bag.in_hand_label().to_string(), app.bag.in_hand_mode());
    app.on_key(KeyEvent::new(KeyCode::Right, KeyModifiers::NONE));
    app.on_key(KeyEvent::new(KeyCode::Left, KeyModifiers::NONE));
    assert_eq!(
        (app.bag.in_hand_label().to_string(), app.bag.in_hand_mode()),
        before,
        "arrow keys must never cycle the model"
    );
    assert!(app.agent_menu.is_none(), "arrows must not open the deck");
}

#[test]
fn thinking_effort_cycles_via_brackets_not_arrows() {
    let mut app = App::new(Bag::for_reasoning_render_test(), Viewer::new());
    assert!(app.bag.select_route_with_effort(0, 0, "high"));
    assert_eq!(app.bag.reasoning_effort().as_deref(), Some("high"));
    let cycled = app.cycle_thinking(true);
    assert_eq!(cycled.as_deref(), Some("low"));
    assert_eq!(app.bag.reasoning_effort().as_deref(), Some("low"));
    let cycled = app.cycle_thinking(false);
    assert_eq!(cycled.as_deref(), Some("high"));
}
