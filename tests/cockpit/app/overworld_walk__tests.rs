use super::*;
use crate::drive::together_shooter::RoomKind;
use crate::tests::TestEnvGuard;
use ratatui::{Terminal, backend::TestBackend};
fn app() -> App {
    App::preview(crate::ui::viewer::Viewer::new())
}
fn paint(app: &mut App) {
    let mut terminal = Terminal::new(TestBackend::new(144, 48)).unwrap();
    terminal.draw(|f| crate::ui::draw::ui(f, app)).unwrap();
}
fn env() -> Vec<TestEnvGuard> {
    vec![
        TestEnvGuard::unset("ANGEL_WORLD_MAP"),
        TestEnvGuard::unset("ANGEL_COMP_MODE"),
        TestEnvGuard::unset("ANGEL_BACKDROP"),
        TestEnvGuard::set("ANGEL_IMAGE_PROTOCOL", "halfblocks"),
    ]
}
fn click(app: &mut App, x: u16, y: u16) {
    for kind in [
        event::MouseEventKind::Down(event::MouseButton::Left),
        event::MouseEventKind::Up(event::MouseButton::Left),
    ] {
        app.on_mouse(event::MouseEvent {
            kind,
            column: x,
            row: y,
            modifiers: KeyModifiers::NONE,
        });
    }
}
#[test]
fn overworld_walk_click_opens_the_continuous_kingdom_and_keeps_the_harness_knight() {
    let _lock = crate::tests::env_lock();
    let _env = env();
    let mut app = app();
    paint(&mut app);
    let knight = app.world.overworld_scene().knight;
    let (area, _) = app.overworld_map.unwrap();
    click(&mut app, area.x + area.width / 2, area.y + area.height / 2);
    assert!(app.dungeon.expanded);
    assert_eq!(
        app.dungeon.shooter.as_ref().unwrap().room().kind,
        RoomKind::Kingdom
    );
    assert_eq!(app.world.overworld_scene().knight, knight);
    assert!(!app.walk.on);
    app.on_key(event::KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE));
    paint(&mut app);
    assert!(!app.dungeon.expanded);
    assert!(app.walk_map_shown());
}
#[test]
fn overworld_walk_command_and_p_enter_the_same_big_map() {
    let _lock = crate::tests::env_lock();
    let _env = env();
    let mut app = app();
    let said = app.walk_from_command();
    assert!(said.contains("kingdom"));
    assert!(app.dungeon.expanded);
    assert_eq!(
        app.dungeon.shooter.as_ref().unwrap().room().kind,
        RoomKind::Kingdom
    );
    app.collapse_dungeon();
    paint(&mut app);
    app.focus_module("artifacts");
    app.on_key(event::KeyEvent::new(KeyCode::Char('p'), KeyModifiers::NONE));
    assert!(app.dungeon.expanded);
}
#[test]
fn overworld_walk_drag_keeps_the_overview_camera() {
    let _lock = crate::tests::env_lock();
    let _env = env();
    let mut app = app();
    paint(&mut app);
    let (area, _) = app.overworld_map.unwrap();
    let (x, y) = (area.x + area.width / 2, area.y + area.height / 2);
    for (kind, x, y) in [
        (event::MouseEventKind::Down(event::MouseButton::Left), x, y),
        (
            event::MouseEventKind::Drag(event::MouseButton::Left),
            x + 2,
            y + 1,
        ),
        (
            event::MouseEventKind::Up(event::MouseButton::Left),
            x + 2,
            y + 1,
        ),
    ] {
        app.on_mouse(event::MouseEvent {
            kind,
            column: x,
            row: y,
            modifiers: KeyModifiers::NONE,
        });
    }
    assert!(!app.dungeon.expanded);
    assert!(!app.scryglass.follow_agent);
}
#[test]
fn overworld_walk_uses_the_existing_dungeon_controls_and_fires_with_f() {
    let _lock = crate::tests::env_lock();
    let _env = env();
    let mut app = app();
    app.walk_from_command();
    paint(&mut app);
    app.on_key(event::KeyEvent::new(KeyCode::Char('d'), KeyModifiers::NONE));
    assert!(app.dungeon.local_inputs(std::time::Instant::now())[&1].move_x > 0);
    app.on_key(event::KeyEvent::new(KeyCode::Char('e'), KeyModifiers::NONE));
    let input = app.dungeon.local_inputs(std::time::Instant::now())[&1];
    assert!(input.interact);
    assert!(!input.fire);
    app.on_key(event::KeyEvent::new(KeyCode::Char('f'), KeyModifiers::NONE));
    assert!(app.dungeon.local_inputs(std::time::Instant::now())[&1].fire);
    assert!(app.input.is_empty());
}
