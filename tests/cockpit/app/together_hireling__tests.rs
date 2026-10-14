//! The hireling on the cockpit's own screen.

use crate::drive::together_shooter::*;
use crate::drive::together_shooter::hireling::testing::*;

#[test]
fn the_side_panel_names_the_minion_and_not_beaumains() {
    let mut run = fight_with(false, &["marrow-sip"]);
    foe_at(&mut run, EnemyKind::Bat, (44.0, 3.0), u32::MAX / 2);
    play(&mut run, 1);
    let mut terminal = ratatui::Terminal::new(ratatui::backend::TestBackend::new(120, 40)).unwrap();
    terminal
        .draw(|frame| {
            crate::ui::viz::shooter_viz::render(
                frame,
                &run,
                frame.area(),
                "",
                true,
                false,
                false,
                "",
                None,
                None,
                None,
                None,
                false,
            );
        })
        .unwrap();
    let text: String = terminal
        .backend()
        .buffer()
        .content
        .iter()
        .map(|cell| cell.symbol())
        .collect();
    let hire = run.hireling.as_ref().unwrap();
    assert!(
        text.contains(&format!("Leech: {}/{}", hire.hp, hire.max_hp)),
        "the leech by name"
    );
    assert!(!text.contains("Beaumains"));
}
