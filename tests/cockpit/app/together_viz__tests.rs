use super::*;

#[test]
fn forge_and_dungeon_render_inside_their_pane_at_small_and_large_sizes() {
    let mut mode = Together::default();
    mode.command(Some("demo"), 1, "Realm", std::path::Path::new("."))
        .unwrap();
    for raid in [false, true] {
        if raid {
            mode.command(Some("raid"), 1, "Realm", std::path::Path::new("."))
                .unwrap();
        }
        for (width, height) in [(24, 12), (72, 32)] {
            let mut terminal =
                ratatui::Terminal::new(ratatui::backend::TestBackend::new(width + 4, height + 4))
                    .unwrap();
            let pane = Rect::new(2, 2, width, height);
            terminal
                .draw(|frame| {
                    for cell in &mut frame.buffer_mut().content {
                        cell.set_char('~');
                    }
                    render(frame, &mode, pane);
                })
                .unwrap();
            let buffer = terminal.backend().buffer();
            for y in 0..height + 4 {
                for x in 0..width + 4 {
                    if !pane.contains((x, y).into()) {
                        assert_eq!(buffer[(x, y)].symbol(), "~");
                    }
                }
            }
            let text = buffer
                .content
                .iter()
                .map(|cell| cell.symbol())
                .collect::<String>();
            assert!(text.contains("TOGETHER"));
            if width == 72 {
                assert!(text.contains(if raid { "HP 100" } else { "Spark Wand" }));
            }
        }
    }
}

#[test]
fn a_player_standing_in_a_telegraph_gets_a_visible_danger_label() {
    let mut mode = Together::default();
    mode.command(Some("demo"), 1, "Realm", std::path::Path::new("."))
        .unwrap();
    mode.command(Some("raid"), 1, "Realm", std::path::Path::new("."))
        .unwrap();
    mode.room
        .as_mut()
        .unwrap()
        .run
        .as_mut()
        .unwrap()
        .danger
        .insert((2, 1));
    let mut terminal = ratatui::Terminal::new(ratatui::backend::TestBackend::new(80, 32)).unwrap();
    terminal
        .draw(|frame| render(frame, &mode, frame.area()))
        .unwrap();
    let text = terminal
        .backend()
        .buffer()
        .content
        .iter()
        .map(|cell| cell.symbol())
        .collect::<String>();
    assert!(text.contains("DANGER"));
}

#[test]
fn fullscreen_dungeon_keeps_both_key_sets_and_scaled_map_inside_its_area() {
    let mut mode = Together::default();
    mode.start_dungeon(1, "Realm", "Guest 界").unwrap();
    for (width, height) in [(80, 24), (120, 40), (160, 48)] {
        let mut terminal =
            ratatui::Terminal::new(ratatui::backend::TestBackend::new(width + 4, height + 4))
                .unwrap();
        let area = Rect::new(2, 2, width, height);
        terminal
            .draw(|frame| {
                for cell in &mut frame.buffer_mut().content {
                    cell.set_char('~');
                }
                assert!(render_game(
                    frame,
                    &mode,
                    area,
                    "Both players choose an action."
                ));
            })
            .unwrap();
        let buffer = terminal.backend().buffer();
        for y in 0..height + 4 {
            for x in 0..width + 4 {
                if !area.contains((x, y).into()) {
                    assert_eq!(buffer[(x, y)].symbol(), "~");
                }
            }
        }
        let text = buffer
            .content
            .iter()
            .map(|cell| cell.symbol())
            .collect::<String>();
        for label in [
            "P1  WASD",
            "P2  arrows",
            "Backspace wait",
            "Shift + direction",
            "F6: game",
        ] {
            assert!(text.contains(label), "missing {label} at {width}×{height}");
        }
        let tiles = buffer
            .content
            .iter()
            .filter(|cell| cell.bg == FLOOR || cell.bg == WALL)
            .count();
        assert!(
            tiles > 21 * 11,
            "map must occupy scaled tiles, not a 21-column text paragraph"
        );
    }
}

#[test]
fn fullscreen_dungeon_small_terminal_keeps_an_escape_hint_and_hides_combat() {
    let mut mode = Together::default();
    mode.start_dungeon(1, "Realm", "Friend").unwrap();
    let mut terminal = ratatui::Terminal::new(ratatui::backend::TestBackend::new(48, 12)).unwrap();
    terminal
        .draw(|frame| assert!(!render_game(frame, &mode, frame.area(), "")))
        .unwrap();
    let text = terminal
        .backend()
        .buffer()
        .content
        .iter()
        .map(|cell| cell.symbol())
        .collect::<String>();
    assert!(text.contains("controls paused") && text.contains("Esc returns"));
    assert!(!text.contains("P1  WASD"));
}
