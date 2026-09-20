//! Terminal lifecycle suites (module-breakup: extracted from the `tests.rs`
//! monolith). Exit/Esc/Ctrl-C behavior, alt-screen + mouse-capture setup and
//! teardown ordering, and focus-aware bell attention.

use super::{
    seed_advancing_app, seed_preview_app, write_attention_signal, write_enter_sequences,
    write_restore_sequences,
};
use crate::agent::club::ChatMsg;
use crate::tests::{TestEnvGuard, env_lock};

#[test]
fn exit_closes_the_app_but_idle_esc_and_ctrl_c_do_not() {
    use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
    let mut app = seed_preview_app();
    // Idle Esc / Ctrl+C must NOT close — closing is deliberate via `exit`.
    app.on_key(KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE));
    app.on_key(KeyEvent::new(KeyCode::Char('c'), KeyModifiers::CONTROL));
    assert!(!app.should_quit, "Esc/Ctrl+C when idle must not quit");
    // Typing `exit` + Enter closes it.
    for c in "exit".chars() {
        app.on_key(KeyEvent::new(KeyCode::Char(c), KeyModifiers::NONE));
    }
    app.on_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));
    assert!(app.should_quit, "typing `exit` + Enter must close the app");
}

// ---- mouse capture lifecycle + pane-aware selection ----

#[test]
fn terminal_setup_enters_alt_screen_then_enables_mouse_capture() {
    let mut buf = Vec::new();
    write_enter_sequences(&mut buf, true).unwrap();
    let s = String::from_utf8_lossy(&buf);
    assert!(s.contains("?1049h"), "alt screen entered: {s:?}");
    assert!(s.contains("?1000h"), "normal mouse capture enabled: {s:?}");
    assert!(s.contains("?1006h"), "SGR mouse capture enabled: {s:?}");
    assert!(s.contains("?2004h"), "bracketed paste enabled: {s:?}");
    assert!(s.contains("?1004h"), "focus reporting enabled: {s:?}");
    // Alt screen must be entered before mouse capture is turned on.
    assert!(
        s.find("?1049h").unwrap() < s.find("?1000h").unwrap(),
        "alt screen before mouse capture: {s:?}"
    );
}

#[test]
fn terminal_setup_can_preserve_native_mouse_selection_without_losing_paste() {
    let mut buf = Vec::new();
    write_enter_sequences(&mut buf, false).unwrap();
    let s = String::from_utf8_lossy(&buf);

    assert!(s.contains("?1049h"), "alternate screen enabled: {s:?}");
    assert!(
        !s.contains("?1000h") && !s.contains("?1006h"),
        "mouse capture must stay disabled: {s:?}"
    );
    assert!(
        s.contains("?2004h"),
        "bracketed paste remains enabled: {s:?}"
    );
    assert!(
        s.contains("?1004h"),
        "focus reporting remains enabled: {s:?}"
    );
}

#[test]
fn terminal_teardown_disables_mouse_capture_before_leaving_alt_screen() {
    // The critical safety property: an exit/panic must turn mouse reporting
    // back OFF (else the host terminal is left spewing escape codes), and it
    // must do so before leaving the alt screen.
    let mut buf = Vec::new();
    write_restore_sequences(&mut buf).unwrap();
    let s = String::from_utf8_lossy(&buf);
    assert!(s.contains("?2004l"), "bracketed paste disabled: {s:?}");
    assert!(s.contains("?1004l"), "focus reporting disabled: {s:?}");
    assert!(s.contains("?1000l"), "normal mouse capture disabled: {s:?}");
    assert!(s.contains("?1006l"), "SGR mouse capture disabled: {s:?}");
    assert!(s.contains("?1049l"), "alt screen left: {s:?}");
    assert!(
        s.find("?2004l").unwrap() < s.find("?1049l").unwrap(),
        "paste disabled before leaving alt screen: {s:?}"
    );
    assert!(
        s.find("?1000l").unwrap() < s.find("?1049l").unwrap(),
        "mouse capture disabled before leaving alt screen: {s:?}"
    );
}

#[test]
fn terminal_attention_is_focus_aware_coalesced_and_standard_bell() {
    let _lock = env_lock();
    let _enabled = TestEnvGuard::unset("ANGEL_TUI_ATTENTION");
    let mut app = seed_preview_app();
    app.request_terminal_attention();
    assert!(
        !app.take_attention_request(),
        "focused work must remain quiet"
    );

    app.set_terminal_focused(false);
    app.request_terminal_attention();
    app.request_terminal_attention();
    assert!(
        app.take_attention_request(),
        "blurred completion must alert"
    );
    assert!(
        !app.take_attention_request(),
        "multiple completions coalesce into one alert"
    );

    app.request_terminal_attention();
    app.set_terminal_focused(true);
    assert!(
        !app.take_attention_request(),
        "regaining focus cancels an unflushed alert"
    );

    let mut buf = Vec::new();
    write_attention_signal(&mut buf).unwrap();
    assert_eq!(buf, b"\x07");

    let history = vec![ChatMsg::user("question"), ChatMsg::assistant("answer")];
    let mut completed = seed_advancing_app(
        Vec::new(),
        Some(Ok((history.clone(), "answer".to_string()))),
    );
    completed.set_terminal_focused(false);
    completed.advance();
    assert!(
        completed.take_attention_request(),
        "a blurred foreground answer must request attention"
    );

    let mut failed = seed_advancing_app(Vec::new(), Some(Err("provider down".to_string())));
    failed.set_terminal_focused(false);
    failed.advance();
    assert!(
        failed.take_attention_request(),
        "a blurred foreground error must request attention"
    );

    let mut visible = seed_advancing_app(Vec::new(), Some(Ok((history, "answer".to_string()))));
    visible.advance();
    assert!(
        !visible.take_attention_request(),
        "a visible foreground answer must stay quiet"
    );
}

/// A7: `ANGEL_TUI_ATTENTION=0` must silence alerts (tests re-read env; production
/// caches the flag so tool-storm attention requests do not re-hit the process env).
#[test]
fn terminal_attention_quiet_mode_env_off() {
    let _lock = env_lock();
    let _quiet = TestEnvGuard::set("ANGEL_TUI_ATTENTION", "0");
    let mut app = seed_preview_app();
    app.set_terminal_focused(false);
    app.request_terminal_attention();
    assert!(
        !app.take_attention_request(),
        "ANGEL_TUI_ATTENTION=0 must suppress blurred alerts"
    );
}

#[test]
fn final_reply_defers_durable_atlas_write_by_one_tick() {
    let _lock = env_lock();
    let root = std::env::temp_dir().join(format!(
        "angel-atlas-final-frame-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    let _atlas = TestEnvGuard::set("ANGEL_ATLAS", "1");
    let _atlas_dir = TestEnvGuard::set("ANGEL_ATLAS_DIR", &root.to_string_lossy());
    let history = vec![ChatMsg::user("question"), ChatMsg::assistant("answer")];
    let mut app = seed_advancing_app(Vec::new(), Some(Ok((history, "answer".to_string()))));
    let atlas = std::sync::Arc::clone(&app.atlas);

    app.advance();
    assert!(
        app.pending_atlas_harvest.is_some(),
        "the completion frame should stage persistence"
    );
    assert!(
        atlas.next_harvest().is_none(),
        "the causal completion frame must not perform the durable write"
    );

    app.advance();
    assert!(app.pending_atlas_harvest.is_none());
    // The next tick queues the durable write and waits a bounded slice for its
    // acknowledgement (ATLAS_ACK_WAIT); a slow disk may overrun that slice, in
    // which case later ticks collect the acknowledgement. Durability is the
    // contract, the slice is not: drain before asserting.
    let ack_deadline = std::time::Instant::now() + std::time::Duration::from_secs(10);
    while app
        .atlas_harvester
        .as_ref()
        .is_some_and(|harvester| harvester.outstanding() > 0)
    {
        assert!(
            std::time::Instant::now() < ack_deadline,
            "harvest acknowledgement never arrived"
        );
        std::thread::sleep(std::time::Duration::from_millis(5));
    }
    let harvest = atlas
        .next_harvest()
        .expect("next tick persists the harvest");
    assert_eq!(harvest.final_answer, "answer");

    drop(app);
    drop(atlas);
    let _ = std::fs::remove_dir_all(root);
}
