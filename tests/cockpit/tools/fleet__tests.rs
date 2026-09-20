use super::*;

#[test]
fn nvidia_csv_formats_per_gpu_lines() {
    let csv = "0, NVIDIA RTX PRO 6000, 590.12, 87, 44231, 49140, 61, 310.5, 450.0\n\
                   1, NVIDIA RTX PRO 6000, 590.12, 12, 1024, 49140, 41, 90.0, 450.0";
    let out = format_nvidia_csv(csv);
    assert!(out.contains("GPU0 NVIDIA RTX PRO 6000 | util 87% | mem 44231/49140 MiB"));
    assert!(out.contains("GPU1"));
    assert!(out.contains("driver 590.12"));
}

#[test]
fn nvidia_csv_omits_na_fields_on_unified_memory_parts() {
    // Real GB10 (DGX Spark) output: no dedicated VRAM or power limit.
    let csv = "0, NVIDIA GB10, 580.159.03, 0, [N/A], [N/A], 61, 21.41, [N/A]";
    let out = format_nvidia_csv(csv);
    assert_eq!(
        out,
        "GPU0 NVIDIA GB10 | util 0% | 61°C | 21.41 W | driver 580.159.03"
    );
}

#[test]
fn nvidia_csv_passes_through_unparseable_text() {
    assert_eq!(format_nvidia_csv("weird output"), "weird output");
}

#[test]
fn clip_head_tail_total_contract() {
    // Small text untouched.
    assert_eq!(clip_head_tail("hello", 100, ""), "hello");
    // max = 0 terminates with empty output.
    assert_eq!(clip_head_tail("abc", 0, ""), "");
    // Keep the input larger than every tested budget so marker assertions
    // exercise clipping, while the small-text assertion covers passthrough.
    let text = format!("h{}t", "…".repeat(4000));
    let marker_len = format!("…[truncated; snipped {} bytes]", text.len()).len();
    for max in 0..=marker_len + 80 {
        let out = clip_head_tail(&text, max, "");
        assert!(out.len() <= max, "max={max} len={}", out.len());
        if max < marker_len {
            assert!(!out.contains("…[truncated"));
            assert!(text.starts_with(&out));
        } else {
            let (head, rest) = out.split_once("…[truncated; snipped ").unwrap();
            let (count, tail) = rest.split_once(" bytes]").unwrap();
            assert!(text.starts_with(head) && text.ends_with(tail));
            assert_eq!(
                count.parse::<usize>().unwrap(),
                text.len() - head.len() - tail.len()
            );
        }
    }
    // 2/3/4-byte codepoints cut at every boundary: valid UTF-8, both ends kept.
    for cp in ["é", "…", "🦀"] {
        let big = cp.repeat(4000);
        let out = clip_head_tail(&big, 1000, "");
        assert!(std::str::from_utf8(out.as_bytes()).is_ok());
        assert!(out.len() <= 1000);
        assert!(out.starts_with(cp) && out.ends_with(cp));
        assert!(out.contains("snipped "));
    }
    // Note surfaces inside the marker.
    let out = clip_head_tail(&"a".repeat(50_000), 16_000, "full output remained live");
    assert!(out.len() <= 16_000);
    assert!(out.contains("full output remained live"));
}

#[test]
fn clip_head_tail_never_introduces_replacements_after_lossy() {
    for b in [0u8, 0x80, 0xC3, 0xE2, 0xF0, 0xFF] {
        let raw = vec![b'a', b, b'z', b, b'a'];
        let text = String::from_utf8_lossy(&raw).into_owned();
        let expect = text.matches('\u{FFFD}').count();
        for max in [1usize, 2, 3, 4, 8, 20, 60, 200] {
            let out = clip_head_tail(&text, max, "");
            assert!(std::str::from_utf8(out.as_bytes()).is_ok());
            assert_eq!(
                out.matches('\u{FFFD}').count(),
                expect.min(out.len() / 3),
                "max={max}"
            );
        }
    }
}

#[test]
fn run_recon_truncates_with_marker() {
    // Focused production behavior: large recon output is head/tail clipped
    // and marked, small output passes through untouched.
    let big = run_recon("printf", &[&"x".repeat(20_000)], 15).unwrap();
    assert!(big.len() <= 12_000);
    assert!(big.contains("…[truncated; snipped "));
    assert!(big.starts_with("xxxx") && big.ends_with("xxxx"));
    let small = run_recon("printf", &["ok"], 15).unwrap();
    assert_eq!(small, "ok");
    let missing = run_recon("definitely-not-a-binary-34", &[], 15);
    assert!(missing.is_err());
}

#[test]
fn machine_test_clipping_preserves_streaming_and_exit_status() {
    let _guard = crate::tests::env_lock();
    use crate::harness::ProcessStream;
    use std::path::PathBuf;
    use std::sync::Mutex;

    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../tests/cockpit/integration/fixtures/machine_queue_output.py");
    assert!(path.is_file(), "missing fake client: {}", path.display());
    let config = MachineQueueConfig {
        client_script: path,
        host: "local-fake".into(),
        remote_script: "unused".into(),
        remote_db: "unused".into(),
        resource: "fake".into(),
        owner: "review".into(),
        competition: "review".into(),
        remote_cwd: ".".into(),
        wait_seconds: 0,
        lease_seconds: 15,
        quantum_seconds: 15,
    };
    let tool = MachineTestTool { config };
    let args = serde_json::json!({"command": "ok"});
    let plain = tool.call(&args).unwrap();
    assert!(plain.len() <= 16000);
    assert!(plain.contains('O') && plain.contains('E'));
    assert!(plain.contains("…[truncated; snipped "));
    assert!(!plain.contains("full output remained live"));

    let chunks = Arc::new(Mutex::new(Vec::<(ProcessStream, usize)>::new()));
    let seen = Arc::clone(&chunks);
    let progress: Arc<ToolOutputProgress> = Arc::new(move |stream, bytes| {
        seen.lock().unwrap().push((stream, bytes.len()));
    });
    let streamed = tool
        .call_with_cancel_and_progress(&args, None, Some(progress))
        .unwrap();
    assert!(streamed.contains("full output remained live"));
    let seen = chunks.lock().unwrap();
    assert!(
        seen.iter()
            .any(|(s, n)| *s == ProcessStream::Stdout && *n > 0)
    );
    assert!(
        seen.iter()
            .any(|(s, n)| *s == ProcessStream::Stderr && *n > 0)
    );

    let error = tool
        .call(&serde_json::json!({"command": "fail"}))
        .unwrap_err();
    assert!(error.contains("queued machine test failed (exit 7)"));
    assert!(error.contains('E'));
    assert!(!error.contains("full output remained live"));
}

#[test]
fn machine_queue_is_opt_in_and_uses_competition_as_fair_owner() {
    let _guard = crate::tests::env_lock();
    let _host = crate::tests::TestEnvGuard::set("ANGEL_MACHINE_QUEUE_HOST", "mac-builder");
    let _competition = crate::tests::TestEnvGuard::set("ANGEL_COMPETITION_ID", "kernel-race");
    let _owner = crate::tests::TestEnvGuard::unset("ANGEL_MACHINE_QUEUE_OWNER");
    let config = MachineQueueConfig::from_env(Path::new("/tmp/project")).unwrap();
    assert_eq!(config.host, "mac-builder");
    assert_eq!(config.competition, "kernel-race");
    assert_eq!(config.owner, "kernel-race");
    assert_eq!(config.quantum_seconds, 1_800);
}

#[test]
#[ignore = "reads the live box (nvidia-smi / tailscale); run with --ignored"]
fn live_gpu_stat_and_fleet_status() {
    let gpu = GpuStatTool.call(&serde_json::json!({}));
    println!("gpu_stat →\n{gpu:?}\n");
    let fleet = FleetStatusTool.call(&serde_json::json!({}));
    println!("fleet_status →\n{fleet:?}\n");
    assert!(
        gpu.is_ok() || fleet.is_ok(),
        "neither GPU nor tailnet recon answered on this box"
    );
}
