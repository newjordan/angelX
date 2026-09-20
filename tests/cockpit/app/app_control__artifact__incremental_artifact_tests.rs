use super::*;

#[test]
fn r04c_cont2_incremental_detector_matches_complete_prefixes() {
    let cases = [
        "ordinary prose ".repeat(1000),
        format!("{}<!DOCTYPE HTML><html>", "p".repeat(300)),
        format!(
            "{}```python\n{}",
            "intro ".repeat(500),
            "print(1)\n".repeat(12)
        ),
        format!("```unknown\n{}``` tail", "x".repeat(1800)),
        format!("```  HTML   \nsmall body```{}", "tail".repeat(500)),
        format!("{}<canvas", "界".repeat(260)),
        format!("```unknown\n<!doctype html>```{}", "x".repeat(1700)),
        format!("```unknown\n{}\n```", "界".repeat(1700)),
    ];
    for text in cases {
        for step in [1, 7, 239, 4096] {
            let mut detector = StreamArtifactDetector::default();
            let mut start = 0;
            let mut detected = false;
            while start < text.len() {
                let mut end = (start + step).min(text.len());
                while !text.is_char_boundary(end) {
                    end += 1;
                }
                detected |= detector.push(&text[start..end]);
                let expected = large_code_document(&text[..end]).is_some()
                    || probable_streaming_artifact(&text[..end]);
                assert_eq!(detected, expected, "step={step} end={end}");
                start = end;
            }
        }
    }
}

#[test]
fn r04c_cont2_original_detector_reproduction() {
    let _guard = crate::tests::env_lock();
    let chunk = "ordinary prose with words and a newline.\n".repeat(100);
    let mut text = chunk.repeat((6 * 1024 * 1024usize).div_ceil(chunk.len()));
    if let Some(path) = std::env::var_os("ANGEL_FRAME_TIMING_LOG") {
        use std::io::Write;
        let mut file = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(path)
            .unwrap();
        writeln!(
            file,
            "# fixture=original_detector_only (not the complete frame loop)"
        )
        .unwrap();
    }
    let mut timing = crate::ui::frame_timing::FrameTiming::from_env();
    let mut max = std::time::Duration::ZERO;
    for _ in 0..8 {
        let start = std::time::Instant::now();
        let mut bytes = 0;
        while bytes < crate::app::control::STREAM_BYTES_PER_FRAME {
            text.push_str(&chunk);
            assert!(large_code_document(&text).is_none());
            assert!(!probable_streaming_artifact(&text));
            bytes += chunk.len();
        }
        let elapsed = start.elapsed();
        max = max.max(elapsed);
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
    }
    eprintln!(
        "original whole-partial detector bytes={} frame_batches=8 max_us={}",
        text.len(),
        max.as_micros()
    );
}
