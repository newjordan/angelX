#[test]
fn r04c_cont2_original_final_sanitizer_cost() {
    let _guard = crate::tests::env_lock();
    let text = "ordinary prose with words and a newline.\n".repeat(160_000);
    if let Some(path) = std::env::var_os("ANGEL_FRAME_TIMING_LOG") {
        use std::io::Write;
        let mut file = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(path)
            .unwrap();
        writeln!(
            file,
            "# fixture=original_final_sanitizer_only (not the complete frame loop)"
        )
        .unwrap();
    }
    let mut timing = crate::ui::frame_timing::FrameTiming::from_env();
    let start = std::time::Instant::now();
    let sanitized = super::sanitize_complete_terminal_text(&text);
    let elapsed = start.elapsed();
    assert_eq!(sanitized, text);
    if let Some(timing) = &mut timing {
        let draw = std::time::Instant::now();
        timing.completed(
            draw,
            draw,
            crate::ui::frame_timing::Phases {
                advance_us: elapsed.as_micros(),
                ..Default::default()
            },
        );
    }
    eprintln!(
        "original final sanitizer bytes={} advance_component_us={}",
        text.len(),
        elapsed.as_micros()
    );
}
