use super::*;

fn argv(args: &[&str]) -> Vec<OsString> {
    args.iter().map(OsString::from).collect()
}

#[test]
fn complete_interactive_arguments_are_checked_without_echoing_text() {
    for args in [
        vec!["--yolo", "--yolo"],
        vec!["--comp", "--lean"],
        vec!["lost bare text"],
        vec!["--unknown"],
        vec!["--draft"],
        vec!["--draft", "  \n\t"],
        vec!["--draft", "a", "trailing-secret"],
        vec!["--draft", "a", "--draft", "b"],
        vec!["--workspace", "a", "--workspace", "b"],
        vec!["--resume", "--resume"],
        vec!["--resume", "id", "extra"],
        vec!["--draft", "a", "--resume"],
        vec!["--resume", "--draft", "a"],
        vec!["--doctor", "secret"],
        vec!["--help", "secret"],
    ] {
        let error = parse(&argv(&args)).unwrap_err().to_string();
        assert!(!error.contains("secret"), "diagnostic leaked argv");
    }
    for args in [
        vec!["--model", "fixture-private"],
        vec!["--prompt", "a", "--draft", "b"],
        vec!["--prompt", "a", "--resume"],
        vec!["--driver", "practice", "--driver", "practice"],
        vec!["--effort", "high", "--effort", "low"],
        vec!["--driver", "  "],
        vec!["--model", " x "],
    ] {
        assert!(parse(&argv(&args)).is_err());
    }
    let launch = parse(&argv(&[
        "--driver",
        "openai",
        "--model",
        "exact/slash-id",
        "--effort",
        "pi:high",
        "--prompt",
        "  literal  ",
    ]))
    .unwrap()
    .unwrap();
    assert_eq!(launch.model.as_deref(), Some("exact/slash-id"));
    assert_eq!(launch.effort.as_deref(), Some("pi:high"));
    assert_eq!(launch.prompt.as_deref(), Some("  literal  "));
}

#[test]
fn option_looking_values_remain_literal_drafts_and_machine_payloads() {
    for value in [
        "--doctor",
        "--task",
        "--resume",
        "--help",
        "--workspace",
        "--draft",
        "/quit",
        "/yolo on",
        "  α\nβ  ",
    ] {
        let launch = parse(&argv(&["--draft", value])).unwrap().unwrap();
        assert_eq!(launch.draft.as_deref(), Some(value));
        assert_eq!(launch.resume, None);
        assert_eq!(
            parse(&argv(&["--prompt", value]))
                .unwrap()
                .unwrap()
                .prompt
                .as_deref(),
            Some(value)
        );
        for mode in ["--ask", "--task", "--task-json", "--look-image"] {
            assert!(parse(&argv(&[mode, value])).unwrap().is_none());
        }
    }
}

#[test]
fn resume_and_leading_globals_preserve_the_existing_shape() {
    assert_eq!(
        parse(&argv(&["--resume"])).unwrap().unwrap().resume,
        Some(None)
    );
    assert_eq!(
        parse(&argv(&["--yolo", "--comp", "--resume", "id"]))
            .unwrap()
            .unwrap()
            .resume,
        Some(Some("id".into()))
    );
    assert!(
        parse(&argv(&["--yolo", "--ask", "--doctor"]))
            .unwrap()
            .is_none()
    );
    assert_eq!(
        ordinary_args(&argv(&["--yolo", "--comp", "--doctor"])),
        argv(&["--doctor"])
    );
    assert_ne!(
        ordinary_args(&argv(&["--ask", "--doctor"])),
        argv(&["--doctor"])
    );
}

#[test]
fn initial_text_uses_the_composer_byte_bound() {
    let max = crate::app::control::MAX_COMPOSER_PASTE_BYTES;
    assert!(parse(&argv(&["--draft", &"a".repeat(max)])).is_ok());
    assert!(parse(&argv(&["--draft", &"a".repeat(max + 1)])).is_err());
    assert!(parse(&argv(&["--draft", &"α".repeat(max / 2 + 1)])).is_err());
    assert!(parse(&argv(&["--prompt", &"a".repeat(max + 1)])).is_err());
    for value in ["\u{85}", "\u{a0}\u{2003}\u{3000}", "\u{2028}\u{2029}"] {
        assert!(parse(&argv(&["--draft", value])).is_err());
        assert!(parse(&argv(&["--resume", value])).is_err());
    }
    #[cfg(unix)]
    {
        use std::os::unix::ffi::OsStringExt;
        for flag in ["--draft", "--prompt", "--resume", "--model", "--effort"] {
            assert!(parse(&[flag.into(), OsString::from_vec(vec![0xff])]).is_err());
        }
    }
}

#[test]
fn canonical_workspace_precedence_and_invalid_paths() {
    let root = std::env::temp_dir().join(format!("angel-entry-{}", std::process::id()));
    std::fs::create_dir(&root).unwrap();
    struct Cleanup(PathBuf);
    impl Drop for Cleanup {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }
    let _cleanup = Cleanup(root.clone());
    for name in ["cli", "env"] {
        std::fs::create_dir(root.join(name)).unwrap();
    }
    let launch = parse(&argv(&["--workspace", "cli/../cli"]))
        .unwrap()
        .unwrap();
    assert_eq!(
        workspace(&launch, Some("env".into()), &root).unwrap(),
        root.join("cli").canonicalize().unwrap()
    );
    assert_eq!(
        workspace(&InteractiveLaunch::default(), Some("env".into()), &root).unwrap(),
        root.join("env").canonicalize().unwrap()
    );
    assert_eq!(
        workspace(&InteractiveLaunch::default(), None, &root).unwrap(),
        root.canonicalize().unwrap()
    );
    assert!(workspace(&launch, None, &root.join("missing")).is_err());
    std::fs::write(root.join("file"), b"fixture").unwrap();
    assert!(workspace(&InteractiveLaunch::default(), Some("file".into()), &root).is_err());
}

#[test]
fn native_cli_rejects_before_terminal_and_does_not_scan_payload_for_doctor() {
    let binary = std::env::var_os("CARGO_BIN_EXE_angel")
        .expect("native gate supplies the private CLI image");
    for args in [
        vec!["--draft", "--doctor", "--unknown"],
        vec!["--model", "private-fixture"],
        vec!["--task-json", "--", "--doctor", "--unknown"],
    ] {
        let output = isolated_native(&binary).args(&args).output().unwrap();
        assert!(
            !output.status.success(),
            "payload must not elect doctor mode"
        );
        assert!(
            !output.stdout.contains(&0x1b),
            "terminal must not initialize"
        );
        assert!(!String::from_utf8_lossy(&output.stderr).contains("private-fixture"));
    }
}

fn isolated_native(binary: &OsStr) -> std::process::Command {
    let mut cmd = std::process::Command::new(binary);
    cmd.env_clear();
    for key in [
        "PATH",
        "HOME",
        "CODEX_HOME",
        "XDG_CONFIG_HOME",
        "XDG_DATA_HOME",
        "XDG_STATE_HOME",
        "XDG_CACHE_HOME",
        "CARGO_HOME",
        "RUSTUP_HOME",
        "TMPDIR",
    ] {
        cmd.env(
            key,
            std::env::var_os(key).expect("sanitized gate must supply every fixture root"),
        );
    }
    cmd.env("USER", "fixture")
        .env("LOGNAME", "fixture")
        .env("CARGO_NET_OFFLINE", "true")
        .env("ANGEL_API_CLUBS", "none")
        .env("ANGEL_PROBE", "0")
        .env("ANGEL_BAG_PROBE", "0")
        .env("ANGEL_HYDRA_DISCOVER", "0")
        .env("ANGEL_TAILNET_RESOLVE", "0")
        .env("ANGEL_LSP", "0")
        .env("ANGEL_YOLO", "0");
    cmd
}

#[test]
fn native_helpers_use_the_parsed_mode_slice_with_leading_globals_and_opaque_values() {
    let binary = std::env::var_os("CARGO_BIN_EXE_angel").unwrap();
    for args in [
        vec!["--comp", "--atlas", "--unknown"],
        vec!["--comp", "--look-image", "--doctor"],
        vec!["--comp", "--sandbox-exec"],
        vec!["--comp", "--audit-coding-eval-training"],
        vec!["--comp", "--tool-http-helper"],
        vec![
            "--driver",
            "openai",
            "--model",
            "private-fixture",
            "--prompt",
            "--doctor",
        ],
        vec!["--driver", "openai-api", "--prompt", "--doctor"],
        vec!["--driver", "auto"],
    ] {
        let output = isolated_native(&binary).args(args).output().unwrap();
        assert!(!output.status.success());
        assert!(
            !output.stdout.contains(&0x1b),
            "helper must never enter the TUI"
        );
        assert!(!String::from_utf8_lossy(&output.stderr).contains("private-fixture"));
    }
}

#[test]
fn native_private_cli_practice_draft_and_literal_prompt_fixture() {
    let binary = std::env::var_os("CARGO_BIN_EXE_angel").unwrap();
    let mut cmd = isolated_native(OsStr::new("python3"));
    let output = cmd
        .arg("../scripts/check/native-entry-fixture.py")
        .arg(binary)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(String::from_utf8_lossy(&output.stdout).contains("native entry fixture: 6 passed"));
}

#[test]
fn launch_draft_is_composer_only_and_preserves_exact_unicode() {
    let _guard = crate::tests::env_lock();
    let (_tx, rx) = std::sync::mpsc::channel();
    let mut app = crate::app::App::from_parts(
        crate::agent::club::Bag::for_render_test(&[("practice", &[("practice", true)])]),
        crate::ui::viewer::Viewer::static_preview(),
        Vec::new(),
        std::sync::Arc::new(crate::agent::harness::ToolRegistry::new()),
        rx,
        crate::knowledge::session::Session::disabled(),
        crate::platform::overwatch::Overwatch::disabled(),
    );
    let history = app.history.len();
    let messages = app.messages.len();
    let text = "  /yolo on\nα --doctor  ";
    app.install_launch_draft(text.into());
    assert_eq!(app.input, text);
    assert_eq!(app.cursor, text.chars().count());
    assert_eq!(app.history.len(), history);
    assert_eq!(app.messages.len(), messages);
    assert!(app.pending_turn.is_none());
    assert!(app.thinking.is_none());
}
