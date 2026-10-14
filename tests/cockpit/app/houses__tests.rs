//! The houses as the cockpit's routes serve them.

use super::*;

#[test]
fn a_route_serves_from_its_model_familys_castle_never_a_machine() {
    let kimi = by_key("kimi").unwrap();
    assert_eq!(of_route("kimi", "kimi", Some("kimi-k3")), Some(kimi));
    // A self-hosted model is a mode of `local`: its family's house.
    let qwen = by_key("qwen").unwrap();
    assert_eq!(
        of_route("local", "local", Some("Qwen3.6-35B-A3B")),
        Some(qwen)
    );
    assert_eq!(
        of_route("local", "local", Some("deepseek-v4-flash")),
        by_key("deepseek")
    );
    assert_eq!(
        of_route("sota", "openai", Some("gpt-6.1-sol")),
        by_key("sol")
    );
    assert_eq!(
        of_route("sota", "openai", Some("gpt-6-astra")),
        by_key("astra")
    );
    // A route with no family has no castle: the Keep's own knight serves.
    assert_eq!(of_route("practice", "practice", None), None);
    assert_eq!(of_route("local", "local", Some("my-finetune")), None);
    for name in ["turbo", "atlas", "spark", "apollo", "toymaker"] {
        assert!(
            all()
                .iter()
                .all(|h| !h.key.contains(name) && !h.family.to_lowercase().contains(name)),
            "a machine has a castle: {name}"
        );
    }
}

/// The Delve's intro with a house serving, as text, for review:
/// `cargo test --bin angel print_delve_intro_with_houses -- --ignored --nocapture`.
#[test]
#[ignore]
fn print_delve_intro_with_houses() {
    use ratatui::{Terminal, backend::TestBackend};
    for (lead, kit) in [
        ("deepseek", 0),
        ("nemotron", 1),
        ("kimi", 2),
        ("longcat", 0),
    ] {
        note_serving(&Serving {
            lead: by_key(lead),
            seated: Vec::new(),
            turn: false,
        });
        let intro = crate::ui::viz::delve_intro_viz::Intro {
            knight: kit,
            ..Default::default()
        };
        let mut terminal = Terminal::new(TestBackend::new(120, 34)).unwrap();
        terminal
            .draw(|frame| {
                crate::ui::viz::delve_intro_viz::render(frame, frame.area(), &intro, None)
            })
            .unwrap();
        let buffer = terminal.backend().buffer().clone();
        let text: Vec<String> = (0..buffer.area.height)
            .map(|y| {
                (0..buffer.area.width)
                    .map(|x| buffer[(x, y)].symbol().to_string())
                    .collect::<String>()
            })
            .collect();
        println!("==== {lead}\n{}", text.join("\n"));
    }
    note_serving(&Serving::default());
}
