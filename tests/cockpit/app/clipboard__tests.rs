use super::*;

fn argv(list: &[&str]) -> Vec<String> {
    list.iter().map(|arg| arg.to_string()).collect()
}

/// One production-format read of a local fake command: a far deadline and no
/// cancellation, exactly like a real clipboard attempt.
fn run(program: &str, args: &[&str], limit: usize) -> ToolRead {
    read_tool(
        &ReadControl::default(),
        Instant::now() + Duration::from_secs(30),
        30,
        program,
        args,
        limit,
    )
}

fn wait_for_pid(control: &ReadControl) -> u32 {
    for _ in 0..600 {
        let pid = control.pid.load(Ordering::Acquire);
        if pid != 0 {
            return pid;
        }
        std::thread::sleep(Duration::from_millis(5));
    }
    panic!("the fake tool never registered a pid");
}

#[test]
fn session_backend_prefers_wayland_then_falls_back_to_x11() {
    assert_eq!(select_backend(true, true, true, true), Ok(Backend::Wayland));
    assert_eq!(
        select_backend(true, true, true, false),
        Ok(Backend::Wayland)
    );
    assert_eq!(select_backend(true, true, false, true), Ok(Backend::X11));
    assert_eq!(select_backend(false, true, false, true), Ok(Backend::X11));
}

#[test]
fn missing_backend_names_the_session_and_the_install() {
    let none = select_backend(false, false, false, false).unwrap_err();
    assert!(none.contains("WAYLAND_DISPLAY"), "{none}");
    let wayland = select_backend(true, false, false, false).unwrap_err();
    assert!(wayland.contains("wl-clipboard"), "{wayland}");
    let x11 = select_backend(false, true, false, false).unwrap_err();
    assert!(x11.contains("xclip"), "{x11}");
    // A Wayland session without wl-paste must not silently pick a backend it
    // cannot run, nor pretend xclip is installed.
    assert!(select_backend(true, false, false, true).is_err());
}

/// Every read names its MIME family: tool-side inference must never deliver
/// raster bytes to the text fallback (or an HTML fragment as prose).
#[test]
fn every_read_names_its_mime_family() {
    for (backend, flag) in [(Backend::Wayland, "--type"), (Backend::X11, "-t")] {
        for candidates in [backend.image_args(), backend.text_args()] {
            for args in candidates {
                let index = args
                    .iter()
                    .position(|arg| *arg == flag)
                    .expect("a read names its type");
                let requested = args[index + 1];
                assert!(
                    requested.starts_with("image")
                        || requested.starts_with("text")
                        || requested == "UTF8_STRING",
                    "{args:?}"
                );
            }
        }
    }
}

#[test]
fn native_image_copy_has_typed_backends_and_never_claims_an_osc52_image() {
    assert_eq!(
        select_image_write_backend(true, true, true, true),
        Ok(Backend::Wayland)
    );
    assert_eq!(
        select_image_write_backend(true, true, false, true),
        Ok(Backend::X11)
    );
    assert!(
        select_image_write_backend(false, false, true, true)
            .unwrap_err()
            .contains("Copy path")
    );
    assert!(select_image_write_backend(true, false, false, true).is_err());
    for backend in [Backend::Wayland, Backend::X11] {
        assert!(backend.image_write_args().contains(&"image/png"));
        assert!(!backend.image_write_args().contains(&"text/plain"));
    }
}

#[test]
fn native_image_copy_encodes_original_pixels_and_preserves_confined_reads() {
    let base = std::env::temp_dir().join(format!("angel-native-copy-{}", std::process::id()));
    std::fs::create_dir_all(&base).unwrap();
    let path = base.join("figure.bmp");
    let raster = image::RgbaImage::from_fn(5, 3, |x, y| {
        image::Rgba([x as u8 * 40, y as u8 * 60, 127, 255])
    });
    raster.save(&path).unwrap();
    let source = crate::ui::media::MediaSource {
        path: path.clone(),
        root: Some(base.clone()),
    };
    let (png, width, height) = encode_clipboard_png(ClipboardImageSource::File(source)).unwrap();
    assert_eq!((width, height), (5, 3));
    assert!(png.starts_with(b"\x89PNG\r\n\x1a\n"));
    assert_eq!(image::load_from_memory(&png).unwrap().to_rgba8(), raster);
    let outside = base.with_extension("outside.png");
    raster.save(&outside).unwrap();
    let alias = base.join("alias.png");
    std::os::unix::fs::symlink(&outside, &alias).unwrap();
    let error = encode_clipboard_png(ClipboardImageSource::File(crate::ui::media::MediaSource {
        path: alias,
        root: Some(base.clone()),
    }))
    .unwrap_err();
    assert!(
        error.contains("symlink") || error.contains("Cannot read"),
        "{error}"
    );
    std::fs::remove_dir_all(base).unwrap();
    std::fs::remove_file(outside).unwrap();
}

#[test]
fn native_image_writer_delivers_binary_bytes_to_a_local_fake_only() {
    let path = std::env::temp_dir().join(format!("angel-copy-sink-{}", std::process::id()));
    let control = ReadControl::default();
    let bytes = b"\x89PNG\x00\x01\xff\n";
    write_image_tool(
        &control,
        Instant::now() + Duration::from_secs(5),
        "/bin/sh",
        &[
            "-c",
            "cat > \"$1\"",
            "clipboard-fixture",
            path.to_str().unwrap(),
        ],
        bytes,
    )
    .unwrap();
    assert_eq!(std::fs::read(&path).unwrap(), bytes);
    assert_eq!(control.pid.load(Ordering::Acquire), 0);
    std::fs::remove_file(path).unwrap();
}

#[test]
fn native_image_writer_rejects_tool_failure_and_bounds_a_stalled_input() {
    let control = ReadControl::default();
    let error = write_image_tool(
        &control,
        Instant::now() + Duration::from_secs(5),
        "/bin/sh",
        &["-c", "cat >/dev/null; exit 3"],
        b"image bytes",
    )
    .unwrap_err();
    assert!(error.contains("rejected"), "{error}");
    let started = Instant::now();
    let error = write_image_tool(
        &control,
        started + Duration::from_millis(100),
        "/bin/sh",
        &["-c", "exec sleep 30"],
        &vec![0; 1024 * 1024],
    )
    .unwrap_err();
    assert!(error.contains("within"), "{error}");
    assert!(started.elapsed() < Duration::from_secs(5));
    assert_eq!(control.pid.load(Ordering::Acquire), 0);
}

#[test]
fn canceled_native_image_copy_does_not_start_a_clipboard_command() {
    let control = ReadControl::default();
    control.cancel.store(true, Ordering::Release);
    let error = write_image_tool(
        &control,
        Instant::now() + Duration::from_secs(5),
        "/nonexistent/must-not-run",
        &[],
        b"image",
    )
    .unwrap_err();
    assert!(error.contains("canceled"));
    assert_eq!(control.pid.load(Ordering::Acquire), 0);
}

#[test]
fn canceling_a_pending_native_image_write_reaps_its_tool() {
    let control = Arc::new(ReadControl::default());
    let worker_control = Arc::clone(&control);
    let worker = std::thread::spawn(move || {
        write_image_tool(
            &worker_control,
            Instant::now() + Duration::from_secs(30),
            "/bin/sh",
            &["-c", "exec sleep 30"],
            &vec![0; 1024 * 1024],
        )
    });
    let pid = wait_for_pid(&control);
    control.cancel.store(true, Ordering::Release);
    let error = worker.join().unwrap().unwrap_err();
    assert!(error.contains("canceled"), "{error}");
    assert!(
        !process_is_running(pid),
        "the canceled writer must be reaped"
    );
    assert_eq!(control.pid.load(Ordering::Acquire), 0);
}

#[test]
fn decoded_size_ignores_base64_padding() {
    let image = |b64: &str| Media::Image {
        mime: "image/png".into(),
        b64: b64.into(),
    };
    assert_eq!(decoded_bytes(&image("AAAA")), 3);
    assert_eq!(decoded_bytes(&image("AAA=")), 2);
    assert_eq!(decoded_bytes(&image("AA==")), 1);
    assert_eq!(decoded_bytes(&image("")), 0);
}

// ---- bounded runs against local fake commands (no clipboard) ----

#[test]
fn a_healthy_read_returns_the_tool_bytes() {
    let read = run("/bin/sh", &["-c", "printf hello"], 64);
    assert!(
        matches!(read, ToolRead::Bytes(ref bytes) if bytes == b"hello"),
        "{read:?}"
    );
}

/// The limit is the answer even though the ended child exits non-zero: the
/// caller must report "too large", not "tool failed" and not fall through to
/// a text read of the same bytes.
#[test]
fn an_oversized_read_reports_the_limit_before_the_exit_status() {
    let payload = "x".repeat(200);
    let script = format!("printf '%s' {payload}");
    let read = run("/bin/sh", &["-c", &script], 64);
    assert!(matches!(read, ToolRead::TooLarge), "{read:?}");
}

#[test]
fn an_empty_read_is_distinct_from_a_launch_failure() {
    // A tool that ran and offered nothing is "empty", never a partial success.
    let read = run("/bin/sh", &["-c", "exit 3"], 64);
    assert!(matches!(read, ToolRead::Empty(_)), "{read:?}");
    let missing = run("/nonexistent/angel-clipboard-tool", &[], 64);
    assert!(
        matches!(missing, ToolRead::Failed(detail) if detail.contains("not installed")),
        "a missing tool is a hard failure, not an empty clipboard"
    );
}

/// A tool that exits non-zero while a descendant still holds the pipe is
/// reported with its status, not as a partial success.
#[test]
fn a_tool_that_exits_nonzero_behind_a_descendant_reports_its_status() {
    let read = run("/bin/sh", &["-c", "sleep 30 & exit 3"], 64);
    assert!(
        matches!(read, ToolRead::Empty(ref detail) if detail.contains("exited")),
        "{read:?}"
    );
}

/// A tool that closes stdout and then keeps running must not hold the read:
/// the answer is complete at EOF, and the tool is ended and reaped.
#[test]
fn a_tool_that_closes_stdout_while_alive_is_ended() {
    let control = ReadControl::default();
    let started = Instant::now();
    let read = read_tool(
        &control,
        started + Duration::from_secs(30),
        30,
        "/bin/sh",
        &["-c", "exec 1>&-; sleep 30"],
        64,
    );
    assert!(matches!(read, ToolRead::Empty(_)), "{read:?}");
    assert!(
        started.elapsed() < Duration::from_secs(5),
        "closed stdout must not wait for the process to exit"
    );
    assert_eq!(
        control.pid.load(Ordering::Acquire),
        0,
        "the reader released its child"
    );
}

/// A descendant that inherits the pipe keeps it open after the tool exits;
/// the read must end at the tool's own exit and kill the group.
#[test]
fn a_descendant_holding_the_pipe_cannot_wedge_the_read() {
    let descendant_file =
        std::env::temp_dir().join(format!("angel-clipboard-descendant-{}", std::process::id()));
    let _ = std::fs::remove_file(&descendant_file);
    let script = format!(
        "sleep 30 & echo $! > {pid}; printf hello",
        pid = descendant_file.display()
    );
    let control = ReadControl::default();
    let started = Instant::now();
    let read = read_tool(
        &control,
        started + Duration::from_secs(30),
        30,
        "/bin/sh",
        &["-c", script.as_str()],
        64,
    );
    assert!(
        matches!(read, ToolRead::Bytes(ref bytes) if bytes == b"hello"),
        "{read:?}"
    );
    assert!(
        started.elapsed() < Duration::from_secs(5),
        "the descendant cannot hold the read open"
    );
    let descendant: u32 = std::fs::read_to_string(&descendant_file)
        .expect("the fake tool published its descendant pid")
        .trim()
        .parse()
        .expect("descendant pid");
    let _ = std::fs::remove_file(&descendant_file);
    for _ in 0..600 {
        if !process_is_running(descendant) {
            break;
        }
        std::thread::sleep(Duration::from_millis(5));
    }
    assert!(
        !process_is_running(descendant),
        "the group kill must end the descendant the tool left behind"
    );
    assert_eq!(control.pid.load(Ordering::Acquire), 0);
}

/// Cancelling a read ends its tool immediately and reports the cancel instead
/// of a partial answer.
#[test]
fn a_canceled_read_ends_its_tool() {
    let control = Arc::new(ReadControl::default());
    let worker_control = Arc::clone(&control);
    let worker = std::thread::spawn(move || {
        read_tool(
            &worker_control,
            Instant::now() + Duration::from_secs(30),
            30,
            "/bin/sh",
            &["-c", "exec sleep 30"],
            64,
        )
    });
    let pid = wait_for_pid(&control);
    control.cancel.store(true, Ordering::Release);
    let read = worker.join().expect("reader finished");
    assert!(
        matches!(read, ToolRead::Failed(ref detail) if detail.contains("canceled")),
        "{read:?}"
    );
    for _ in 0..600 {
        if !process_is_running(pid) {
            break;
        }
        std::thread::sleep(Duration::from_millis(5));
    }
    assert!(!process_is_running(pid), "the canceled tool was reaped");
    assert_eq!(control.pid.load(Ordering::Acquire), 0);
}

/// The production classifier, driven by local fake commands: an oversized
/// image is reported as such and never falls back to reading text.
#[test]
fn an_oversized_image_is_reported_and_the_text_fallback_is_not_consulted() {
    let oversized = format!("head -c {} /dev/zero", MAX_IMAGE_ATTACHMENT_BYTES + 1);
    let plan = Plan::Local {
        program: "/bin/sh".into(),
        image: vec![argv(&["-c", oversized.as_str()])],
        text: vec![argv(&["-c", "printf 'text must not be read'"])],
    };
    let error = fetch_with(
        &ReadControl::default(),
        Instant::now() + Duration::from_secs(30),
        30,
        &plan,
    )
    .unwrap_err();
    assert!(error.contains("exceeds the 5 MiB"), "{error}");
}

#[test]
fn an_image_only_clipboard_reports_no_text_without_reading_one() {
    let plan = Plan::Local {
        program: "/bin/sh".into(),
        image: vec![argv(&["-c", "exit 1"])],
        text: vec![argv(&["-c", "exit 1"])],
    };
    let error = fetch_with(
        &ReadControl::default(),
        Instant::now() + Duration::from_secs(30),
        30,
        &plan,
    )
    .unwrap_err();
    assert!(error.contains("no image and no text"), "{error}");
}

#[test]
fn a_text_clipboard_arrives_as_text_after_the_image_miss() {
    let plan = Plan::Local {
        program: "/bin/sh".into(),
        image: vec![argv(&["-c", "exit 1"])],
        text: vec![argv(&["-c", "printf 'hello clipboard'"])],
    };
    match fetch_with(
        &ReadControl::default(),
        Instant::now() + Duration::from_secs(30),
        30,
        &plan,
    ) {
        Ok(Fetched::Text(text)) => assert_eq!(text, "hello clipboard"),
        other => panic!("expected text, got {other:?}"),
    }
}
