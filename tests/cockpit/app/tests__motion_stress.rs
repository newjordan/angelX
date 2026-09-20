//! Explicit sustained measurements; preserve every sample, not best-of timing.
use super::*;

#[test]
#[ignore = "explicit sustained rendering measurement; no wall-clock unit gate"]
fn sustained_motion_render_measurement() {
    let _env = env_lock();
    let _motion = TestEnvGuard::set("ANGEL_TUI_MOTION", "full");
    let _image = TestEnvGuard::set("ANGEL_IMAGE_PROTOCOL", "halfblocks");
    let _scan = TestEnvGuard::set("ANGEL_SCAN", "0");
    let _discover = TestEnvGuard::set("ANGEL_HYDRA_DISCOVER", "0");
    let _publish = TestEnvGuard::set("ANGEL_HYDRA_PUBLISH", "0");
    let _lsp = TestEnvGuard::set("ANGEL_LSP", "0");
    let _experience = TestEnvGuard::set("ANGEL_EXPERIENCE", "0");
    let _dossier = TestEnvGuard::set("ANGEL_DOSSIER", "0");
    for scenario in ["idle", "history", "stream", "scrollback", "resize", "image"] {
        let mut app = seed_preview_app();
        app.visual_motion = crate::viz::lifecycle_viz::MotionMode::Full;
        app.messages.clear();
        let count = if scenario == "idle" { 1 } else { 2_000 };
        for i in 0..count {
            app.messages.push(Message {
                role: if i % 2 == 0 { Role::User } else { Role::Angel },
                text: format!(
                    "History {i}: read **bounded pages**, keep `symbols` and verifier evidence. 日本語 e\u{301} 👩‍🔬\nSecond row of stable context."
                ).into(),
            });
        }
        if matches!(scenario, "stream" | "scrollback" | "resize") {
            app.thinking = Some(Thinking::pending_for_test("practice"));
            app.reasoning =
                "Checking the next bounded page and retaining verifier state. ".repeat(8);
        }
        if scenario == "image" {
            let path =
                PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("assets/agents/apollo-neutral.png");
            app.viewer.show(&path).unwrap();
        }
        let mut terminal = Terminal::new(TestBackend::new(144, 48)).unwrap();
        for _ in 0..20 {
            terminal.draw(|frame| ui(frame, &mut app)).unwrap();
        }
        if scenario == "scrollback" {
            app.scroll = 200;
            terminal.draw(|frame| ui(frame, &mut app)).unwrap();
        }
        let frames = 300;
        let mut samples = Vec::with_capacity(frames);
        let mut changed_cells = Vec::with_capacity(frames);
        let mut previous = terminal.backend().buffer().clone();
        for frame_index in 0..frames {
            if matches!(scenario, "stream" | "scrollback" | "resize") {
                app.partial.push_str("bounded output 界 e\u{301} 👩‍🔬\n");
            }
            if scenario == "resize" && frame_index % 50 == 0 {
                let (w, h) = [(72, 24), (240, 60), (144, 48)][frame_index / 50 % 3];
                terminal.backend_mut().resize(w, h);
                terminal.autoresize().unwrap();
                previous = terminal.backend().buffer().clone();
            }
            app.input = format!("draft-{frame_index}-界-e\u{301}");
            app.cursor = app.input.chars().count();
            let started = Instant::now();
            terminal.draw(|frame| ui(frame, &mut app)).unwrap();
            samples.push(started.elapsed().as_micros() as u64);
            let buffer = terminal.backend().buffer();
            changed_cells.push(previous.diff(buffer).len());
            previous = buffer.clone();
            let text = test_backend_text(terminal.backend());
            assert!(
                text.contains(&format!("draft-{frame_index}-"))
                    && text.contains("界")
                    && text.contains("e\u{301}"),
                "draft hidden in {scenario} frame {frame_index}"
            );
            if scenario == "scrollback" {
                assert!(app.scroll > 0, "streaming moved the reader to the bottom");
            }
        }
        let mut sorted = samples.clone();
        sorted.sort_unstable();
        eprintln!(
            "SUSTAINED_RENDER {}",
            serde_json::json!({
                "scenario": scenario, "frames": frames, "history_messages": count,
                "motion": "full", "profile": "default-feature debug unless command specifies release",
                "p50_us": sorted[frames / 2], "p95_us": sorted[frames * 95 / 100],
                "max_us": sorted[frames - 1], "ordered_us": samples,
                "changed_cells": changed_cells,
            })
        );
    }
}
