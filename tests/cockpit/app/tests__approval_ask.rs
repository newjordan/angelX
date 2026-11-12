//! The approval ask's lifetime: the answer it arms belongs to that ask alone,
//! and an ask whose worker has already given up does not stay on the card.
//! Every case drives the real inbox the worker thread uses (`approval_rx`),
//! `advance` and the key and mouse handlers.

use super::{TestEnvGuard, env_lock, seed_live_streaming_app};
use crate::agent::approval::{self, ApprovalScope, Decision};
use crate::app::App;
use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use std::sync::mpsc;

fn key(code: KeyCode) -> KeyEvent {
    KeyEvent::new(code, KeyModifiers::NONE)
}

/// Hand `app` an ask the way a worker does, and let `advance` put it on the card.
fn raise(
    app: &mut App,
    inbox: &mpsc::Sender<approval::Request>,
    prompt: &str,
) -> mpsc::Receiver<Decision> {
    let (reply, answer) = mpsc::channel();
    inbox
        .send(approval::Request {
            prompt: prompt.to_string(),
            scope: ApprovalScope::ActionBatch("repeat".into()),
            reply,
            deadline: std::time::Instant::now() + std::time::Duration::from_secs(600),
        })
        .unwrap();
    app.advance();
    answer
}

/// A live turn with an empty approval inbox. The turn's sender is handed back:
/// dropping it would read as a vanished worker and end the turn.
fn app_with_inbox() -> (App, mpsc::Sender<approval::Request>, super::TurnSender) {
    let (mut app, turn) = seed_live_streaming_app(vec![crate::harness::TurnEvent::Token("x".into())]);
    let (inbox, rx) = mpsc::channel();
    app.approval_rx = rx;
    (app, inbox, turn)
}

#[test]
fn a_repeated_ask_starts_on_approve_not_on_the_last_ask_s_answer() {
    let _lock = env_lock();
    let _full = TestEnvGuard::set("ANGEL_YOLO", "0");
    let _smart = TestEnvGuard::set("ANGEL_YOLO_SMART", "0");
    let (mut app, inbox, _turn) = app_with_inbox();

    let first = raise(&mut app, &inbox, "run the same batch?");
    assert!(app.pending_approval.is_some());
    assert_eq!(app.approval_choice(), 0, "a fresh ask starts on Approve");
    app.on_key(key(KeyCode::Right));
    assert_eq!(app.approval_choice(), 1, "Right arms Approve-all");
    app.on_key(key(KeyCode::Enter));
    assert_eq!(first.recv().unwrap(), Decision::ApproveAll);
    assert!(app.pending_approval.is_none());

    // The same tool raises the same words later in the session.
    let second = raise(&mut app, &inbox, "run the same batch?");
    assert!(app.pending_approval.is_some());
    assert_eq!(
        app.approval_choice(),
        0,
        "the new ask must not inherit the widened answer"
    );
    app.on_key(key(KeyCode::Enter));
    assert_eq!(
        second.recv().unwrap(),
        Decision::Approve,
        "a bare Enter on a new ask approves once, never the whole scope"
    );
}

fn raise_by(
    app: &mut App,
    inbox: &mpsc::Sender<approval::Request>,
    prompt: &str,
    deadline: std::time::Instant,
) -> mpsc::Receiver<Decision> {
    let (reply, answer) = mpsc::channel();
    inbox
        .send(approval::Request {
            prompt: prompt.to_string(),
            scope: ApprovalScope::ActionBatch("deadline".into()),
            reply,
            deadline,
        })
        .unwrap();
    app.advance();
    answer
}

#[test]
fn an_ask_expires_off_the_card_when_its_worker_has_denied_it() {
    use std::time::{Duration, Instant};
    let _lock = env_lock();
    let _full = TestEnvGuard::set("ANGEL_YOLO", "0");
    let _smart = TestEnvGuard::set("ANGEL_YOLO_SMART", "0");
    let (mut app, inbox, _turn) = app_with_inbox();

    let answer = raise_by(
        &mut app,
        &inbox,
        "approve exact action?",
        Instant::now() + Duration::from_millis(60),
    );
    assert!(app.pending_approval.is_some(), "an ask with time left is shown");
    std::thread::sleep(Duration::from_millis(100));
    app.advance();
    assert!(
        app.pending_approval.is_none(),
        "past its deadline the worker has denied it and left; the card must not hold the keyboard"
    );
    assert_eq!(answer.try_recv().unwrap(), Decision::Deny, "the worker, if still listening, gets a denial");
    assert!(
        app.messages.iter().any(|m| m.text.contains("approval request timed out")),
        "the person is told why the card went"
    );
}

#[test]
fn an_ask_that_expired_in_the_queue_is_denied_without_being_shown() {
    use std::time::{Duration, Instant};
    let _lock = env_lock();
    let _full = TestEnvGuard::set("ANGEL_YOLO", "0");
    let _smart = TestEnvGuard::set("ANGEL_YOLO_SMART", "0");
    let (mut app, inbox, _turn) = app_with_inbox();

    let answer = raise_by(
        &mut app,
        &inbox,
        "approve exact action?",
        Instant::now() - Duration::from_secs(1),
    );
    assert!(app.pending_approval.is_none());
    assert_eq!(answer.try_recv().unwrap(), Decision::Deny);
}

#[test]
fn an_answer_before_the_deadline_still_reaches_the_worker() {
    use std::time::{Duration, Instant};
    let _lock = env_lock();
    let _full = TestEnvGuard::set("ANGEL_YOLO", "0");
    let _smart = TestEnvGuard::set("ANGEL_YOLO_SMART", "0");
    let (mut app, inbox, _turn) = app_with_inbox();

    let answer = raise_by(
        &mut app,
        &inbox,
        "approve exact action?",
        Instant::now() + Duration::from_secs(600),
    );
    app.advance();
    assert!(app.pending_approval.is_some(), "ticks alone never close a live ask");
    app.on_key(key(KeyCode::Char('y')));
    assert_eq!(answer.try_recv().unwrap(), Decision::Approve);
    assert!(app.pending_approval.is_none());
}

fn picker() -> crate::app::BrowserPicker {
    crate::app::BrowserPicker {
        rows: vec![crate::app::BrowserPickRow {
            label: "Auto".into(),
            detail: String::new(),
            path: None,
            current: true,
        }],
        selected: 0,
    }
}

fn mouse(kind: ratatui::crossterm::event::MouseEventKind, column: u16, row: u16) -> ratatui::crossterm::event::MouseEvent {
    ratatui::crossterm::event::MouseEvent {
        kind,
        column,
        row,
        modifiers: KeyModifiers::NONE,
    }
}

#[test]
fn paste_does_not_reach_the_composer_behind_the_browser_picker() {
    let mut app = super::seed_preview_app();
    app.browser_picker = Some(picker());
    app.on_paste("pasted behind the picker");
    assert!(app.input.is_empty(), "the picker owns the keyboard, paste included");
    assert!(app.browser_picker.is_some());
}

#[test]
fn the_pointer_stays_on_the_browser_picker_until_it_closes() {
    use ratatui::crossterm::event::{MouseButton, MouseEventKind};
    use ratatui::layout::Rect;

    let chip = Rect::new(0, 0, 10, 1);
    let with_routes = || {
        let mut app = super::seed_preview_app();
        app.bag = crate::Bag::for_render_test(&[("alpha", &[("model-a", true), ("model-b", true)])]);
        app
    };
    // Control: with no picker the agent chip opens the route menu.
    let mut control = with_routes();
    control.agent_buttons.push((chip, crate::app::AgentButton::Model));
    control.on_mouse(mouse(MouseEventKind::Down(MouseButton::Left), 1, 0));
    assert!(
        control.agent_menu.is_some(),
        "fixture: the chip opens the menu when nothing is in front"
    );

    let mut app = with_routes();
    app.agent_buttons.push((chip, crate::app::AgentButton::Model));
    app.browser_picker = Some(picker());
    app.on_mouse(mouse(MouseEventKind::Down(MouseButton::Left), 1, 0));
    assert!(app.agent_menu.is_none(), "no second modal opens behind the picker");
    assert!(app.browser_picker.is_some(), "a click away from the rows leaves it open");
}

#[test]
fn a_click_on_a_browser_picker_row_still_commits_the_pick() {
    use ratatui::crossterm::event::{MouseButton, MouseEventKind};
    use ratatui::layout::Rect;

    let mut app = super::seed_preview_app();
    app.browser_picker = Some(picker());
    app.world_buttons
        .push((Rect::new(5, 5, 20, 1), crate::app::WorldButton::BrowserPick(0)));
    app.on_mouse(mouse(MouseEventKind::Down(MouseButton::Left), 6, 5));
    assert!(app.browser_picker.is_none(), "the row click commits and closes");
}
