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
fn overworld_walk_explicit_control_opens_the_kingdom_and_keeps_the_harness_knight() {
    let _lock = crate::tests::env_lock();
    let _env = env();
    let mut app = app();
    paint(&mut app);
    let knight = app.world.overworld_scene().knight;
    app.start_walk();
    assert!(app.dungeon.expanded);
    assert_eq!(app.dungeon.shooter.as_ref().unwrap().room().kind, RoomKind::Kingdom);
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

#[test]
fn miniviz_blank_click_does_not_start_a_dungeon_or_kingdom() {
    let _lock = crate::tests::env_lock();
    let _env = env();
    for called in [false, true] {
        let mut app = app();
        paint(&mut app);
        app.world.delve_called = called;
        let (area, _) = app.overworld_map.unwrap();
        click(&mut app, area.x + 1, area.y + 1);
        assert!(app.dungeon.shooter.is_none(), "blank map click started a run (called={called})");
        assert!(app.dungeon.intro.is_none(), "blank map click opened the menu (called={called})");
        assert!(!app.dungeon.expanded, "blank map click expanded the game (called={called})");
    }
}

#[test]
fn miniviz_buttons_dispatch_before_map_and_gate_handlers() {
    let _lock = crate::tests::env_lock();
    let _env = env();
    let mut app = app();
    paint(&mut app);
    app.world.delve_called = true;
    let (area, _) = app.overworld_map.unwrap();
    let hit = ratatui::layout::Rect::new(area.x + 1, area.y + 1, 4, 2);
    app.world_buttons.insert(0, (hit, WorldButton::DockToggle));
    let was_open = app.object_dock.open;
    click(&mut app, hit.x + 1, hit.y);
    assert_eq!(app.object_dock.open, !was_open, "visible control lost its click to the map");
    assert!(app.dungeon.shooter.is_none());
    assert!(app.dungeon.intro.is_none());
}

#[test]
fn miniviz_only_rendered_dungeon_sprite_opens_gate_menu() {
    let _lock = crate::tests::env_lock();
    let _env = env();
    use crate::stage::world_viz::overworld::{self, map::TILE, wishes};
    let mut app = app();
    paint(&mut app);
    let gate = wishes::delve_gate();
    let cx = gate.tx * TILE + gate.tw * TILE / 2;
    let cy = (gate.ty + gate.th) * TILE - 15;
    app.world.delve_called = true;
    // A pre-existing run makes the gate resumable without waiting for travel.
    app.start_shooter(None);
    app.collapse_dungeon();
    app.dungeon.intro = None;
    app.scryglass.return_to_world();
    let area = ratatui::layout::Rect::new(12, 6, 40, 20);
    app.overworld_map = Some((area, overworld::View { x: cx - 40, y: cy - 40, w: 80, h: 80 }));
    app.world_buttons.clear();
    assert!(!app.delve_map_hit(area.x, area.y));
    let (x, y) = (area.x + 20, area.y + 10);
    assert!(app.delve_map_hit(x, y), "gate center must pick in the displayed projection");
    click(&mut app, x, y);
    assert!(app.dungeon.intro.is_some());
    app.dungeon.intro = None;
    app.scryglass.surface = crate::ui::scryglass::StageSurface::WorldFirstPerson;
    assert!(!app.delve_map_hit(x, y), "stale map coordinates must not hit in 3D view");
}
