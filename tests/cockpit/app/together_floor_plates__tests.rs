//! The floor plates under the cockpit's keys.

use crate::drive::together_shooter::floor_plates::*;
use crate::drive::together_shooter::*;

#[test]
fn together_floor_plates_any_terminal_key_is_consumed_before_game_actions() {
    use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
    let mut app = crate::app::App::preview(crate::ui::viewer::Viewer::static_preview());
    let mut run = Run::new(17, 1, Some("Friend"));
    run.begin_floor_plate(Kind::of(run.dungeon.pack));
    app.realm.dungeon.shooter.set(Some(run));
    app.realm.dungeon.expanded = true;
    app.realm.dungeon.controls_visible = true;
    app.terminal_focused = true;
    assert!(app.dungeon_key(KeyEvent::new(KeyCode::Char('?'), KeyModifiers::NONE)));
    assert!(
        app.realm
            .dungeon
            .shooter
            .as_ref()
            .unwrap()
            .floor_plates
            .is_empty()
    );
    assert!(app.realm.dungeon.expanded);
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
        let run = app.realm.dungeon.shooter.edit().unwrap();
        run.begin_floor_plate(Kind::of(run.dungeon.pack));
        let muted = app.realm.dungeon.audio.voices.muted;
        let votes = app.realm.dungeon.shooter.as_ref().unwrap().playtest.clone();
        assert!(app.dungeon_key(KeyEvent::new(key, KeyModifiers::NONE)));
        assert_eq!(app.realm.dungeon.audio.voices.muted, muted);
        let run = app.realm.dungeon.shooter.as_ref().unwrap();
        assert_eq!(run.playtest.records, votes.records);
        assert_eq!(run.playtest.seats, votes.seats);
        assert!(
            app.realm
                .dungeon
                .shooter
                .as_ref()
                .unwrap()
                .floor_plates
                .is_empty()
        );
        assert!(app.realm.dungeon.expanded);
    }
}
