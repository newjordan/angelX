//! The boss gates on the cockpit's screens and a guest's.

use crate::drive::together_shooter::*;
use crate::drive::together_shooter::boss_gate_testing::*;
use std::collections::BTreeMap;

#[test]
fn boss_gates_guest_state_carries_progress_and_input_claims_cannot_grant_defeat() {
    // One chief a floor (`pacing`): the key's carrier leads its camp.
    let mut run = stronghold(1, 1);
    let gate = run.boss_gates.as_ref().unwrap().guardian;
    // A claim on the wire grants nothing: no such field reads.
    let forged = serde_json::json!({ "boss_gates": { "defeated": true }, "boss_defeat": true });
    assert!(serde_json::from_value::<Input>(forged).is_err());
    run.enter(gate, Some(2));
    run.step(&BTreeMap::from([(1, Input::default())]));
    assert!(run.guardian_locked());
    assert!(!run.boss_gates.as_ref().unwrap().leaders[0].defeated);
    // What a friend's client is sent, read back as it reads it.
    let (_, guest, _) = crate::drive::together_seat::guest_view(&run, 2);
    assert!(guest.boss_gate_plain()[0].contains("key's carrier"));
    assert!(!guest.boss_gates.as_ref().unwrap().leaders[0].defeated);
    // The carrier falls in its own room: the route opens, and the guest's
    // copy says so.
    kill_leader(&mut run, 0);
    run.enter(gate, Some(2));
    let (_, guest, _) = crate::drive::together_seat::guest_view(&run, 2);
    assert!(guest.boss_gates.as_ref().unwrap().leaders[0].defeated);
    assert!(guest.boss_gate_plain()[0].contains("may be fought"));
}

#[test]
fn boss_ecology_together_shooter_native_renderer_compact_normal_and_guest_projection() {
    let mut run = stronghold(1, 1);
    kill_leader(&mut run, 0);
    let guardian = run.boss_gates.as_ref().unwrap().guardian;
    run.enter(guardian, Some(2));
    for (width, height) in [(80, 24), (120, 40)] {
        let mut terminal =
            ratatui::Terminal::new(ratatui::backend::TestBackend::new(width, height)).unwrap();
        terminal
            .draw(|frame| {
                assert!(crate::ui::viz::shooter_viz::render(
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
                    false
                ));
            })
            .unwrap();
        let buffer = terminal.backend().buffer();
        let text = buffer
            .content
            .iter()
            .map(|cell| cell.symbol())
            .collect::<String>();
        // The chiefs live in the sidebar, in plain words; the footer keeps
        // to what was found and said.
        assert!(text.contains("guardian may"), "{text}");
        assert!(!text.contains("sealed ALL"), "{text}");
        let snapshot = text.clone();
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
        assert_eq!(
            snapshot,
            terminal
                .backend()
                .buffer()
                .content
                .iter()
                .map(|cell| cell.symbol())
                .collect::<String>()
        );
        println!("ECOLOGY_RENDER {width}x{height} {text}");
    }
    let (_, guest, _) = crate::drive::together_seat::guest_view(&run, 2);
    assert!(guest.boss_support_line().unwrap().contains("control lost"));
    assert_eq!(
        serde_json::to_value(guest.boss_gate_state()).unwrap(),
        serde_json::to_value(run.boss_gate_state()).unwrap()
    );
}
