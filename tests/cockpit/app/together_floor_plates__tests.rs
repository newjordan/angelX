//! The floor plates under the cockpit's keys.

use crate::drive::together_shooter::*;
use crate::drive::together_shooter::floor_plates::*;

#[test]
fn together_floor_plates_any_terminal_key_is_consumed_before_game_actions() {
    use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
    let mut app = crate::app::App::preview(crate::ui::viewer::Viewer::static_preview());
    let mut run = Run::new(17, 1, Some("Friend"));
    run.begin_floor_plate(Kind::of(run.dungeon.pack));
    app.dungeon.shooter.set(Some(run));
    app.dungeon.expanded = true;
    app.dungeon.controls_visible = true;
    app.terminal_focused = true;
    assert!(app.dungeon_key(KeyEvent::new(KeyCode::Char('?'), KeyModifiers::NONE)));
    assert!(
        app.dungeon
            .shooter
            .as_ref()
            .unwrap()
            .floor_plates
            .is_empty()
    );
    assert!(app.dungeon.expanded);
    for key in [
        KeyCode::F(6),
        KeyCode::Esc,
        KeyCode::Char('f'),
        KeyCode::Char('v'),
        KeyCode::Char('b'),
        KeyCode::Char('k'),
        KeyCode::Char('5'),
        KeyCode::Right,
    ] {
        let run = app.dungeon.shooter.edit().unwrap();
        run.begin_floor_plate(Kind::of(run.dungeon.pack));
        let muted = app.dungeon.audio.voices.muted;
        let votes = app.dungeon.shooter.as_ref().unwrap().playtest.clone();
        assert!(app.dungeon_key(KeyEvent::new(key, KeyModifiers::NONE)));
        assert_eq!(app.dungeon.audio.voices.muted, muted);
        let run = app.dungeon.shooter.as_ref().unwrap();
        assert_eq!(run.playtest.records, votes.records);
        assert_eq!(run.playtest.seats, votes.seats);
        assert!(
            app.dungeon
                .shooter
                .as_ref()
                .unwrap()
                .floor_plates
                .is_empty()
        );
        assert!(app.dungeon.expanded);
    }
}
