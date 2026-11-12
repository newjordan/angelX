use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use ratatui::{Terminal, backend::TestBackend};
use std::time::{Duration, Instant};

fn draw(app: &mut crate::app::App) -> String {
    let mut terminal = Terminal::new(TestBackend::new(100, 40)).unwrap();
    terminal
        .draw(|frame| crate::ui::draw::ui(frame, app))
        .unwrap();
    let buffer = terminal.backend().buffer().clone();
    let mut text = String::new();
    for y in 0..buffer.area.height {
        for x in 0..buffer.area.width {
            text.push_str(buffer[(x, y)].symbol());
        }
        text.push('\n');
    }
    text
}

fn played(app: &mut crate::app::App) -> bool {
    let deadline = Instant::now() + Duration::from_secs(10);
    while Instant::now() < deadline {
        app.tick_realm();
        let index = app.games.find("muster").unwrap();
        if app.games.carts[index].status().contains("playing") {
            return true;
        }
        std::thread::sleep(Duration::from_millis(10));
    }
    false
}

#[test]
fn the_slot_lists_the_realm_game_and_every_cartridge() {
    let _lock = crate::tests::env_lock();
    let mut app = crate::seed_preview_app();
    let list = app.game_command(None);
    assert_eq!(list.contains("delve"), cfg!(feature = "delve"), "{list}");
    assert!(list.contains("muster · The Muster · ready"), "{list}");
    assert!(
        app.game_command(Some("pinball"))
            .contains("No cartridge named `pinball`")
    );
    assert!(app.cart_on_screen().is_none());
}

#[test]
fn a_cartridge_loads_takes_the_screen_and_gives_it_back() {
    let _lock = crate::tests::env_lock();
    let mut app = crate::seed_preview_app();
    let reply = app.game_command(Some("muster"));
    assert!(reply.contains("The Muster · solo"), "{reply}");
    assert!(app.cart_on_screen().is_some());
    assert!(app.dungeon_view_active());
    assert!(app.game_keyboard_active());
    assert!(app.realm_wants_fast_tick());
    assert!(played(&mut app));
    let screen = draw(&mut app);
    assert!(
        screen.contains("The Muster · solo · playing · seat 1"),
        "{screen}"
    );
    assert!(screen.contains("▀"));

    assert!(app.dungeon_key(KeyEvent::new(KeyCode::Char('d'), KeyModifiers::NONE)));
    assert!(
        app.input.is_empty(),
        "a game key never lands in the composer"
    );
    assert!(!app.dungeon_key(KeyEvent::new(KeyCode::Char('c'), KeyModifiers::CONTROL)));

    assert!(app.dungeon_key(KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE)));
    assert!(app.cart_on_screen().is_none());
    assert!(!app.dungeon_view_active());
    let index = app.games.find("muster").unwrap();
    assert!(
        app.games.carts[index].running(),
        "giving the screen back keeps it running"
    );

    app.game_command(Some("muster"));
    assert!(app.cart_on_screen().is_some());
    assert!(app.game_command(Some("muster stop")).contains("closed"));
    assert!(app.cart_on_screen().is_none());
    assert!(!app.games.carts[index].running());
}

#[test]
fn quiet_verbs_leave_the_screen_where_it_is() {
    let _lock = crate::tests::env_lock();
    let mut app = crate::seed_preview_app();
    app.game_command(Some("muster"));
    app.dungeon_key(KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE));
    let reply = app.game_command(Some("muster link"));
    assert!(reply.contains("solo"), "{reply}");
    assert!(app.cart_on_screen().is_none());
    app.game_command(Some("muster stop"));
}

#[cfg(feature = "delve")]
#[test]
fn naming_the_realm_game_puts_any_cartridge_away() {
    let _lock = crate::tests::env_lock();
    let mut app = crate::seed_preview_app();
    app.game_command(Some("muster"));
    assert!(app.cart_on_screen().is_some());
    app.game_command(Some("delve"));
    assert!(app.cart_on_screen().is_none());
    app.game_command(Some("muster stop"));
}

#[cfg(not(feature = "delve"))]
#[test]
fn the_console_holds_no_realm_and_says_so() {
    let _lock = crate::tests::env_lock();
    let mut app = crate::seed_preview_app();
    assert_eq!(app.dungeon_command(None), crate::console::NO_REALM);
    assert!(
        app.game_command(Some("delve"))
            .contains("No cartridge named `delve`")
    );
    assert!(!app.dungeon_view_active());
    app.game_command(Some("muster"));
    assert!(app.dungeon_view_active());
    app.game_command(Some("muster stop"));
}
