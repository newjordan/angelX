//! Teaser frame capture — shot plates for the 04-telemetry section video.
//!
//! Inert unless `ANGELX_FRAMES_DIR` is set: the test renders four scripted
//! "shots" through the real compositor (`ui::draw::ui`) on a ratatui
//! `TestBackend`, each as a numbered frame sequence:
//!
//! - `shotA-*` — the operator typing "what is angelX?" into the composer
//! - `shotB-*` — the agent's answer holding in the transcript
//! - `shotC-*` — an autonomous loop running in the loop stage
//! - `shotD-*` — the formations deck with Council selected
//!
//! `tools/teaser-to-mp4.mjs` turns the sequences into a slow, looped teaser.

use crate::ui::transcript::{Message, Role};

fn write_frame(
    term: &ratatui::Terminal<ratatui::backend::TestBackend>,
    dir: &std::path::Path,
    name: &str,
) {
    let buffer = term.backend().buffer();
    let area = buffer.area();
    let mut text = String::with_capacity((area.width as usize + 1) * area.height as usize);
    for row in buffer.content().chunks(area.width as usize) {
        for cell in row {
            text.push_str(cell.symbol());
        }
        text.push('\n');
    }
    let mut cells = Vec::with_capacity((area.width * area.height) as usize);
    for cell in buffer.content() {
        cells.push(format!("{:?}|{:?}", cell.fg, cell.bg));
    }
    std::fs::write(dir.join(format!("{name}.txt")), &text).unwrap();
    std::fs::write(dir.join(format!("{name}.cells")), cells.join("\n")).unwrap();
}

#[test]
fn frames_teaser() {
    use crate::agent::backplane::{ModelRevision, RouteId};
    use crate::agent::formations::{FormationId, MoaDeckState, MoaModelChoice, MoaModelRef};
    use crate::drive::loop_ctl::LoopState;
    use ratatui::{Terminal, backend::TestBackend as Backend};

    let Some(dir) = std::env::var_os("ANGELX_FRAMES_DIR").map(std::path::PathBuf::from) else {
        return;
    };
    std::fs::create_dir_all(&dir).unwrap();

    let question = "what is angelX?";
    let answer = "angelX is a Rust terminal workspace for coding and research agents. \
One cockpit: model formations, repository tools, content-checked edits, \
code_mode scripting, long-session context, project memory in Atlas, \
persistent goals, headless runs, measured campaigns and benchmarks — \
and a world TUI where the work is drawn as it happens.";

    // Shot A — the operator types the question into the composer.
    {
        let mut app = crate::seed_preview_app();
        app.input.clear();
        let mut term = Terminal::new(Backend::new(120, 40)).unwrap();
        let total = question.chars().count();
        for upto in 0..=total {
            app.input = question.chars().take(upto).collect();
            let _ = term.draw(|f| crate::ui::draw::ui(f, &mut app));
            write_frame(&term, &dir, &format!("shotA-{upto:02}"));
        }
    }

    // Shot B — the answer lands in the transcript; hold on the response.
    {
        let mut app = crate::seed_preview_app();
        app.messages.push(Message::new(Role::User, question));
        app.messages.push(Message::new(Role::Angel, answer));
        app.settle_transcript_spawns();
        let mut term = Terminal::new(Backend::new(120, 40)).unwrap();
        for i in 0..4 {
            let _ = term.draw(|f| crate::ui::draw::ui(f, &mut app));
            write_frame(&term, &dir, &format!("shotB-{i:02}"));
        }
    }

    // Shot C — an autonomous loop is running; the loop stage holds.
    {
        let mut app = crate::seed_preview_app();
        app.messages.push(Message::new(
            Role::User,
            "run the competition loop on the flaky kernel test",
        ));
        app.messages.push(Message::new(
            Role::System,
            "loop · iteration 12 · verifier green · 3 candidates promoted",
        ));
        app.settle_transcript_spawns();
        let mut loop_state = LoopState::default();
        loop_state.task = "stabilize the kernel test".into();
        loop_state.status = crate::drive::loop_ctl::LoopStatus::Running;
        loop_state.iteration = 12;
        loop_state.max_iters = 40;
        app.loop_ctl = loop_state;
        let mut term = Terminal::new(Backend::new(120, 40)).unwrap();
        for i in 0..4 {
            let _ = term.draw(|f| crate::ui::draw::ui(f, &mut app));
            write_frame(&term, &dir, &format!("shotC-{i:02}"));
        }
    }

    // Shot D — the formations deck: Council selected, highlight settled.
    {
        let mut app = crate::seed_preview_app();
        let route = MoaModelRef {
            agent_index: 0,
            slot_index: 0,
            agent: "turbo".into(),
            driver: "practice".into(),
            model: "turbo".into(),
            route_id: RouteId("0".to_string()),
            expected_revision: ModelRevision("0".to_string()),
            metered: false,
        };
        let mut deck = MoaDeckState::new(vec![MoaModelChoice {
            route,
            available: true,
        }]);
        deck.select(FormationId::Council);
        app.moa_deck = Some(deck);
        // The deck overlay only owns input (and thus renders its panel) when
        // the artifacts module is focused; the preview seed defaults to the
        // transcript. Focus artifacts so the deck actually draws for the shot.
        app.focus_module("artifacts");
        let mut term = Terminal::new(Backend::new(120, 40)).unwrap();
        for i in 0..4 {
            let _ = term.draw(|f| crate::ui::draw::ui(f, &mut app));
            write_frame(&term, &dir, &format!("shotD-{i:02}"));
        }
    }
}
