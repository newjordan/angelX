//! The two derived views of cockpit state: which modal is in front and who
//! holds the flight slot. Both are read off the existing fields, so these
//! pin the precedence and that no input channel keeps a ladder of its own.

use super::seed_preview_app;
use crate::app::control::BackgroundJob;
use crate::app::flight::FlightOwner;
use crate::app::{Overlay, PendingApproval};
use crate::agent::turn::Thinking;
use std::sync::mpsc;

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

#[test]
fn the_top_overlay_follows_one_precedence() {
    let mut app = seed_preview_app();
    assert_eq!(app.top_overlay(), Overlay::None);

    let menu = crate::ui::agent_panel::controls::AgentControlMenu {
        kind: crate::ui::agent_panel::controls::AgentMenuKind::Model,
        selected: 0,
        route_target: None,
        confirm_target: None,
    };
    app.agent_menu = Some(menu);
    assert_eq!(app.top_overlay(), Overlay::AgentMenu);

    app.browser_picker = Some(picker());
    assert_eq!(app.top_overlay(), Overlay::BrowserPicker, "the picker is in front of the route menu");

    app.open_setup();
    assert_eq!(app.top_overlay(), Overlay::Setup, "setup is in front of the picker");

    let (reply, _answer) = mpsc::channel();
    app.pending_approval = Some(PendingApproval {
        prompt: "ask".into(),
        scope_label: None,
        reply,
    });
    assert_eq!(app.top_overlay(), Overlay::Approval, "an ask a turn waits on is in front of everything");

    app.pending_approval = None;
    app.setup.menu = None;
    assert_eq!(app.top_overlay(), Overlay::BrowserPicker);
    app.browser_picker = None;
    app.agent_menu = Some(menu);
    assert_eq!(app.top_overlay(), Overlay::AgentMenu);
    app.agent_menu = None;
    assert_eq!(app.top_overlay(), Overlay::None);
}

#[test]
fn only_the_overlays_that_take_the_pointer_say_so() {
    for taking in [
        Overlay::Approval,
        Overlay::Setup,
        Overlay::BrowserPicker,
        Overlay::LoopDialog,
        Overlay::AgentMenu,
    ] {
        assert!(taking.takes_pointer(), "{taking:?}");
    }
    assert!(!Overlay::None.takes_pointer());
    assert!(!Overlay::MoaDeck.takes_pointer(), "the deck's controls ride the world buttons");
}

#[test]
fn the_flight_owner_is_the_first_field_set() {
    let mut app = seed_preview_app();
    assert_eq!(app.flight_owner(), FlightOwner::Idle);
    assert!(!app.flight_owner().is_agent_busy());
    assert!(!app.flight_owner().holds_turn_slot());

    let (_tx, job) = BackgroundJob::channel("job", "Retry the job");
    app.bg_job = Some(job);
    assert_eq!(app.flight_owner(), FlightOwner::BgJob);
    assert!(app.flight_owner().is_agent_busy());
    assert!(app.flight_owner().holds_turn_slot());

    app.thinking = Some(Thinking::pending_for_test("practice"));
    assert_eq!(
        app.flight_owner(),
        FlightOwner::Turn { draining: false },
        "a live turn outranks a job set beside it"
    );
    app.thinking.as_mut().unwrap().begin_draining();
    assert_eq!(app.flight_owner(), FlightOwner::Turn { draining: true });
    assert!(app.flight_owner().is_agent_busy(), "a draining worker still holds the slot");
    app.thinking = None;
    app.bg_job = None;
    assert_eq!(app.flight_owner(), FlightOwner::Idle);
}

#[test]
fn a_lone_experiment_does_not_hold_the_turn_slot() {
    assert!(!FlightOwner::Experiment.holds_turn_slot());
    assert!(!FlightOwner::Experiment.is_agent_busy());
    for holder in [
        FlightOwner::Turn { draining: false },
        FlightOwner::BgJob,
        FlightOwner::ParkedTurn,
        FlightOwner::LoopWork,
        FlightOwner::Campaign,
    ] {
        assert!(holder.holds_turn_slot(), "{holder:?}");
    }
}
