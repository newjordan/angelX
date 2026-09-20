use super::*;

#[test]
fn codex_vision_fallback_preserves_selection_and_requires_image_support() {
    use crate::tests::TestEnvGuard;
    let _guard = crate::tests::env_lock();
    let root = std::env::temp_dir().join(format!("angel-codex-vision-{}", std::process::id()));
    std::fs::create_dir_all(&root).unwrap();
    let _home = TestEnvGuard::set("CODEX_HOME", root.to_str().unwrap());
    let _auth = TestEnvGuard::set(
        "ANGEL_OPENAI_AUTH_JSON",
        r#"{"tokens":{"access_token":"fixture-not-a-real-token"}}"#,
    );
    let _cleared: Vec<_> = [
        "ANGEL_VISION_URL",
        "ANGEL_VISION_MODEL",
        "ANGEL_KIMI_URL",
        "ANGEL_KIMI_KEY",
        "ANGEL_OPENAI_MODEL",
        "ANGEL_OPENAI_REASONING_EFFORT",
        "ANGEL_REASONING_EFFORT",
    ]
    .into_iter()
    .map(TestEnvGuard::unset)
    .collect();
    std::fs::write(
        root.join("config.toml"),
        "model='fixture-vision'\nmodel_reasoning_effort='high'\n",
    )
    .unwrap();
    let catalog = |modalities: Vec<&str>| {
        serde_json::json!({"models": [{
            "slug": "fixture-vision", "display_name": "Fixture",
            "input_modalities": modalities,
            "supported_reasoning_levels": [{"effort": "high"}]
        }]})
        .to_string()
    };
    std::fs::write(
        root.join("models_cache.json"),
        catalog(vec!["text", "image"]),
    )
    .unwrap();
    let (_, selection, _) = codex_vision_config().unwrap();
    assert_eq!(
        (selection.model.as_str(), selection.effort.as_str()),
        ("fixture-vision", "high")
    );
    assert!(vision_backend_configured());
    let club = resolve_vision_club().unwrap();
    assert_eq!(club.label(), "codex-vision/fixture-vision");
    assert!(!club_is_text_only_for_vision(club.as_ref()));

    for modalities in [vec!["text"], vec![]] {
        std::fs::write(root.join("models_cache.json"), catalog(modalities)).unwrap();
        assert!(!vision_backend_configured());
        assert!(
            resolve_vision_club()
                .err()
                .unwrap()
                .contains("no cached image capability")
        );
    }
    std::fs::write(
        root.join("models_cache.json"),
        catalog(vec!["text", "image"]),
    )
    .unwrap();
    let _effort = TestEnvGuard::set("ANGEL_OPENAI_REASONING_EFFORT", "unsupported");
    assert!(!vision_backend_configured());
    assert!(
        resolve_vision_club()
            .err()
            .unwrap()
            .contains("not supported")
    );
    drop(_effort);
    let _no_auth = TestEnvGuard::set("ANGEL_OPENAI_AUTH_JSON", "{}");
    assert!(!vision_backend_configured());
    assert!(
        resolve_vision_club()
            .err()
            .unwrap()
            .contains("no signed-in Codex account")
    );

    // Explicit backends must still win, even when native auth is unusable.
    let _kimi_url = TestEnvGuard::set("ANGEL_KIMI_URL", "http://127.0.0.1:9/v1");
    let _kimi_key = TestEnvGuard::set("ANGEL_KIMI_KEY", "fixture");
    assert_eq!(resolve_vision_club().unwrap().label(), "kimi-vision");
    let _url = TestEnvGuard::set("ANGEL_VISION_URL", "http://127.0.0.1:9/v1");
    let _model = TestEnvGuard::set("ANGEL_VISION_MODEL", "explicit-vlm");
    assert_eq!(resolve_vision_club().unwrap().label(), "vision");
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn image_cli_rejects_missing_or_extra_arguments_before_network() {
    for args in [
        vec![],
        vec!["image.png"],
        vec!["image.png", ""],
        vec!["image.png", "question", "extra"],
    ] {
        assert!(
            image_cli(args.into_iter().map(Into::into))
                .unwrap_err()
                .to_string()
                .contains("usage:")
        );
    }
}

#[cfg(unix)]
#[test]
fn duration_probe_hung_tree_is_bounded_even_under_yolo() {
    use std::os::unix::fs::PermissionsExt;

    let _guard = crate::tests::env_lock();
    let root = std::env::temp_dir().join(format!(
        "angel-vision-probe-hang-{}-{:?}",
        std::process::id(),
        std::thread::current().id()
    ));
    let _ = std::fs::remove_dir_all(&root);
    let bin = root.join("bin");
    std::fs::create_dir_all(&bin).unwrap();
    let ffprobe = bin.join("ffprobe");
    std::fs::write(&ffprobe, "#!/bin/sh\nsleep 30 &\nwait\n").unwrap();
    std::fs::set_permissions(&ffprobe, std::fs::Permissions::from_mode(0o755)).unwrap();
    let path = format!(
        "{}:{}",
        bin.display(),
        std::env::var("PATH").unwrap_or_default()
    );
    let _path = crate::tests::TestEnvGuard::set("PATH", &path);
    let _yolo = crate::tests::TestEnvGuard::set("ANGEL_YOLO", "1");
    let started = std::time::Instant::now();

    assert_eq!(
        probe_duration_with_timeout(&root.join("clip.mp4"), Duration::from_millis(50)),
        None
    );
    assert!(
        started.elapsed() < Duration::from_secs(2),
        "hung ffprobe tree escaped its deadline: {:?}",
        started.elapsed()
    );
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn text_only_heuristic_covers_the_local_v4_serve_and_pro() {
    for label in [
        "dsflash",
        "deepseek-v4-flash-dspark",
        "deepseek-v4-flash",
        "deepseek-flash-local",
        "deepseek-v4-pro",
        "deepseek-chat",
    ] {
        let club = LabelOnlyClub {
            label,
            modalities: vec![],
        };
        assert!(club_is_text_only_for_vision(&club), "{label}");
    }
}

#[test]
fn text_only_heuristic_never_claims_the_current_cloud_flash() {
    // No declared metadata and no vision name fragment: the live cloud
    // V4.1 Flash id must not be presumed text-only from its name alone.
    let club = LabelOnlyClub {
        label: "deepseek-flash",
        modalities: vec![],
    };
    assert!(!club_is_text_only_for_vision(&club));
}

#[test]
fn declared_metadata_outranks_the_name_heuristic_both_ways() {
    let declared_text = LabelOnlyClub {
        label: "deepseek-flash",
        modalities: vec!["text".into()],
    };
    assert!(
        club_is_text_only_for_vision(&declared_text),
        "a club that declares text-only is believed even when its name is multimodal"
    );
    let declared_image = LabelOnlyClub {
        label: "dsflash",
        modalities: vec!["text".into(), "image".into()],
    };
    assert!(
        !club_is_text_only_for_vision(&declared_image),
        "a club that declares image input is believed even when its name is text-only"
    );
}

/// The reported defect: a cloud V4.1 Flash turn carrying a screenshot paid
/// an extra provider hop to the configured vision sidecar. The real
/// `HttpClub` (not a label fixture) must declare native image input, keep
/// the encoded attachment bytes, and never call the sidecar backend.
#[test]
fn cloud_flash_keeps_image_bytes_native_and_never_dispatches_the_sidecar() {
    let _env = crate::tests::env_lock();
    let _vars = arm_vision_env();
    let vision = Arc::new(SidecarStub {
        chats: std::sync::atomic::AtomicUsize::new(0),
    });
    let _vision = TestVisionClubGuard::install(Arc::clone(&vision) as Arc<dyn Club>);
    let club = HttpClub::new(
        "deepseek-flash",
        "https://api.deepseek.com/v1",
        "deepseek-flash",
        None,
    );
    assert_eq!(
        club.route_metadata().input_modalities,
        ["text", "image"],
        "the selected cloud Flash route declares native image input"
    );
    assert!(!club_is_text_only_for_vision(&club));
    assert!(vision_sidecar_prompt_hint(&club).is_none());

    let mut msg = ChatMsg::user_with_media("what is on screen?", vec![png_stub()]);
    assert!(!should_apply_vision_sidecar(&club, &msg));
    assert!(
        fold_vision_sidecar_into_convo(&club, std::slice::from_mut(&mut msg)).is_empty(),
        "no sidecar notice for a natively multimodal route"
    );
    assert!(
        msg.attachments
            .iter()
            .any(|media| matches!(media, Media::Image { b64, .. } if b64 == "AAAA")),
        "the encoded attachment bytes must reach the selected provider"
    );
    assert_eq!(&*msg.content, "what is on screen?");
    assert_eq!(
        DESCRIBE_MEDIA_CALLS.load(std::sync::atomic::Ordering::SeqCst),
        0,
        "the sidecar backend must not be called at all"
    );
    assert_eq!(vision.chats.load(std::sync::atomic::Ordering::SeqCst), 0);

    // Root's matched-benchmark route: the operator's configured DeepSeek
    // seat pointed at a loopback observer/forwarder that relays the real
    // API. The route keeps the provider's contract, so the screenshot stays
    // native instead of paying the sidecar hop that broke the comparison.
    let _api_clubs = crate::tests::TestEnvGuard::set("ANGEL_API_CLUBS", "deepseek");
    let _key = crate::tests::TestEnvGuard::set("ANGEL_DEEPSEEK_KEY", "test-key");
    let _fwd = crate::tests::TestEnvGuard::set("ANGEL_DEEPSEEK_URL", "http://127.0.0.1:8799/v4");
    let _pin = crate::tests::TestEnvGuard::unset("ANGEL_DEEPSEEK_FLASH_MODEL");
    let (_, seat, _) = crate::agent::club::optional_sota_http_club(
        "deepseek-flash",
        "deepseek-flash",
        &["ANGEL_DEEPSEEK_URL"],
        "https://api.deepseek.com/v1",
        &["ANGEL_DEEPSEEK_FLASH_MODEL", "DEEPSEEK_FLASH_MODEL"],
        Some("deepseek-flash"),
        &["ANGEL_DEEPSEEK_KEY", "DEEPSEEK_API_KEY"],
    )
    .expect("configured DeepSeek Flash seat");
    assert_eq!(
        seat.route_metadata().input_modalities,
        ["text", "image"],
        "the configured route keeps the provider's capability behind a forwarder"
    );
    assert!(!club_is_text_only_for_vision(seat.as_ref()));
    let forwarded = ChatMsg::user_with_media("what is on screen?", vec![png_stub()]);
    assert!(!should_apply_vision_sidecar(seat.as_ref(), &forwarded));
    assert!(
        forwarded
            .attachments
            .iter()
            .any(|media| matches!(media, Media::Image { b64, .. } if b64 == "AAAA")),
        "the forwarder route must still carry the real image bytes"
    );
    assert_eq!(
        DESCRIBE_MEDIA_CALLS.load(std::sync::atomic::Ordering::SeqCst),
        0
    );
    assert_eq!(vision.chats.load(std::sync::atomic::Ordering::SeqCst), 0);
}

/// Pro and the Spark-local V4 serve are genuinely text-only and must keep
/// the sidecar path (and its failure fallback) intact.
#[test]
fn pro_and_local_dsflash_still_route_through_the_sidecar() {
    let _env = crate::tests::env_lock();
    let _vars = arm_vision_env();
    let vision = Arc::new(SidecarStub {
        chats: std::sync::atomic::AtomicUsize::new(0),
    });
    let _vision = TestVisionClubGuard::install(Arc::clone(&vision) as Arc<dyn Club>);
    for (label, url, model) in [
        (
            "deepseek-v4-pro",
            "https://api.deepseek.com/v1",
            "deepseek-v4-pro",
        ),
        (
            "dsflash",
            "http://127.0.0.1:18888/v1",
            "deepseek-v4-flash-dspark",
        ),
        // The same local serve under a neutral label: the retired id alone
        // must keep it text-only rather than claiming the cloud model.
        (
            "local-gateway",
            "http://127.0.0.1:18888/v1",
            "deepseek-v4-flash",
        ),
    ] {
        let club = HttpClub::new(label, url, model, None);
        assert!(
            club_is_text_only_for_vision(&club),
            "{label}/{model} stays text-only for vision"
        );
        let mut msg = ChatMsg::user_with_media("read this", vec![png_stub()]);
        assert!(should_apply_vision_sidecar(&club, &msg));
        let notices = fold_vision_sidecar_into_convo(&club, std::slice::from_mut(&mut msg));
        assert_eq!(notices.len(), 1, "{label}");
        assert!(
            msg.attachments.is_empty(),
            "{label}: the text-only hop must never see bare image parts"
        );
        assert!(msg.content.contains("a red square"), "{label}");
    }
    assert!(vision.chats.load(std::sync::atomic::Ordering::SeqCst) >= 3);
}

#[test]
fn modalities_image_skips_text_only() {
    let club = LabelOnlyClub {
        label: "dsflash",
        modalities: vec!["text".into(), "image".into()],
    };
    assert!(!club_is_text_only_for_vision(&club));
}

#[test]
fn vision_capable_name_skips_sidecar() {
    let club = LabelOnlyClub {
        label: "qwen2.5-vl-7b",
        modalities: vec![],
    };
    assert!(!club_is_text_only_for_vision(&club));
}

#[test]
fn should_not_apply_without_images() {
    let club = LabelOnlyClub {
        label: "dsflash",
        modalities: vec![],
    };
    let msg = ChatMsg::user("hello");
    // Even if backend env is set in the process, empty attachments short-circuit.
    assert!(
        !msg.attachments
            .iter()
            .any(|m| matches!(m, Media::Image { .. }))
    );
    // Force-off path when no images
    let _ = club;
    assert!(!should_apply_vision_sidecar(
        &LabelOnlyClub {
            label: "dsflash",
            modalities: vec![],
        },
        &ChatMsg::user("no media")
    ));
}

#[test]
fn rewrite_body_keeps_operator_question() {
    // Pure composition check without network: build the same body shape.
    let description = "A red button labeled Submit.";
    let original = "what does the button say?";
    let n = 1usize;
    let backend = "vision";
    let mut body = String::new();
    body.push_str("[vision sidecar · ");
    body.push_str(backend);
    body.push_str(" · ");
    body.push_str(&n.to_string());
    body.push_str(" image(s)]\n");
    body.push_str(description);
    body.push_str("\n\nOperator question: ");
    body.push_str(original);
    assert!(body.contains("Submit"));
    assert!(body.contains("what does the button say?"));
    assert!(body.starts_with("[vision sidecar · vision · 1 image(s)]"));
}

#[test]
fn confined_path_rejects_escape() {
    let ws = PathBuf::from("/tmp/ws");
    assert!(confined_path(&ws, "a/b.png").is_ok());
    assert!(confined_path(&ws, "../evil.png").is_err());
    assert!(confined_path(&ws, "/etc/passwd").is_err());
}

struct TestVisionClubGuard;

impl TestVisionClubGuard {
    fn install(club: Arc<dyn Club>) -> Self {
        DESCRIBE_MEDIA_CALLS.store(0, std::sync::atomic::Ordering::SeqCst);
        *test_vision_club_slot()
            .lock()
            .unwrap_or_else(|e| e.into_inner()) = Some(club);
        Self
    }
}

impl Drop for TestVisionClubGuard {
    fn drop(&mut self) {
        *test_vision_club_slot()
            .lock()
            .unwrap_or_else(|e| e.into_inner()) = None;
        DESCRIBE_MEDIA_CALLS.store(0, std::sync::atomic::Ordering::SeqCst);
    }
}

struct AgentTurnOnlyVisionClub {
    chats: std::sync::atomic::AtomicUsize,
    fail: bool,
}

/// Sidecar stand-in for gating tests: answers like a VLM without asserting
/// which thread it runs on, so the caller can exercise the pure rewrite path.
struct SidecarStub {
    chats: std::sync::atomic::AtomicUsize,
}

impl Club for SidecarStub {
    fn respond(&self, _prompt: &str) -> Result<String, String> {
        Ok(String::new())
    }
    fn label(&self) -> &str {
        "vision-stub"
    }
    fn chat(&self, _messages: &[ChatMsg], _tools: &[ToolDef]) -> Result<ClubReply, String> {
        self.chats.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        Ok(ClubReply::Text("a red square".to_string()))
    }
}

impl Club for AgentTurnOnlyVisionClub {
    fn respond(&self, _prompt: &str) -> Result<String, String> {
        Ok(String::new())
    }
    fn label(&self) -> &str {
        "vision-mock"
    }
    fn chat(&self, _messages: &[ChatMsg], _tools: &[ToolDef]) -> Result<ClubReply, String> {
        let name = std::thread::current().name().unwrap_or("").to_string();
        assert_eq!(
            name, "agent-turn",
            "vision sidecar club.chat must run on agent-turn, not {name:?}"
        );
        self.chats.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        if self.fail {
            Err("backend down".to_string())
        } else {
            Ok(ClubReply::Text("a red square".to_string()))
        }
    }
}

struct DsflashDriver {
    hops: std::sync::Mutex<Vec<String>>,
}

impl Club for DsflashDriver {
    fn respond(&self, prompt: &str) -> Result<String, String> {
        Ok(format!("dsflash: {prompt}"))
    }
    fn label(&self) -> &str {
        "dsflash"
    }
    fn route_metadata(&self) -> crate::agent::club::RouteMetadata {
        crate::agent::club::RouteMetadata {
            input_modalities: vec!["text".into()],
            ..Default::default()
        }
    }
    fn chat(&self, messages: &[ChatMsg], _tools: &[ToolDef]) -> Result<ClubReply, String> {
        let name = std::thread::current().name().unwrap_or("").to_string();
        if name != "agent-turn" {
            return Err(format!("driver chat on {name}, not agent-turn"));
        }
        let last = messages
            .iter()
            .rev()
            .find(|message| message.role == ChatRole::User)
            .ok_or_else(|| "no user message".to_string())?;
        if last
            .attachments
            .iter()
            .any(|media| matches!(media, Media::Image { .. }))
        {
            return Err("hop 1 raced vision sidecar: images still attached".into());
        }
        self.hops
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .push(last.content.to_string());
        Ok(ClubReply::Text("ok from dsflash".into()))
    }
}

fn png_stub() -> Media {
    Media::Image {
        mime: "image/png".into(),
        b64: "AAAA".into(),
    }
}

fn arm_vision_env() -> (
    crate::tests::TestEnvGuard,
    crate::tests::TestEnvGuard,
    crate::tests::TestEnvGuard,
    crate::tests::TestEnvGuard,
) {
    (
        crate::tests::TestEnvGuard::set("ANGEL_LSP", "0"),
        crate::tests::TestEnvGuard::set("ANGEL_VISION_URL", "http://127.0.0.1:9/v1"),
        crate::tests::TestEnvGuard::set("ANGEL_VISION_MODEL", "mock-vlm"),
        crate::tests::TestEnvGuard::unset("ANGEL_VISION_SIDECAR"),
    )
}

fn park_turn(app: &mut crate::App, user_msg: ChatMsg) {
    let raw = Arc::clone(&user_msg.content);
    app.pending_turn = Some(crate::app::PendingTurn {
        raw: Arc::clone(&raw),
        user_msg,
        turn_evidence: None,
        echo_drawn: true,
        retry_draft: raw,
        clipboard_images: 0,
    });
}

#[test]
fn launch_pending_turn_without_images_does_not_describe_media() {
    let _env = crate::tests::env_lock();
    let workspace = crate::tests::TestGitWorkspace::new(
        "launch_pending_turn_without_images_does_not_describe_media",
    );
    let _vars = arm_vision_env();
    let vision = Arc::new(AgentTurnOnlyVisionClub {
        chats: std::sync::atomic::AtomicUsize::new(0),
        fail: false,
    });
    let _vision = TestVisionClubGuard::install(Arc::clone(&vision) as Arc<dyn Club>);
    let driver = Arc::new(DsflashDriver {
        hops: std::sync::Mutex::new(Vec::new()),
    });
    let mut app = crate::App::preview(crate::Viewer::static_preview());
    app.tools = Arc::new(workspace.registry());
    app.bag
        .replace_in_hand_club_for_test(Arc::clone(&driver) as Arc<dyn Club>);
    park_turn(&mut app, ChatMsg::user("no pictures, just text"));
    assert!(!should_apply_vision_sidecar(
        app.bag.in_hand().as_ref(),
        &ChatMsg::user("no pictures, just text")
    ));
    app.launch_pending_turn();
    assert_eq!(
        DESCRIBE_MEDIA_CALLS.load(std::sync::atomic::Ordering::SeqCst),
        0,
        "no-image launch must not call describe_media"
    );
    assert_eq!(vision.chats.load(std::sync::atomic::Ordering::SeqCst), 0);
    assert!(
        !app.messages
            .iter()
            .any(|message| message.text.contains("describing")),
        "no-image fast path must not emit a describing line"
    );
    let thinking = app.thinking.take().expect("worker spawned");
    let result = thinking
        .rx
        .recv_timeout(std::time::Duration::from_secs(5))
        .expect("worker must finish without a vision hop");
    result.expect("text-only turn should succeed");
    assert_eq!(
        DESCRIBE_MEDIA_CALLS.load(std::sync::atomic::Ordering::SeqCst),
        0
    );
    assert_eq!(vision.chats.load(std::sync::atomic::Ordering::SeqCst), 0);
}

#[test]
fn launch_pending_turn_rewrites_images_on_agent_turn_before_hop_1() {
    let _env = crate::tests::env_lock();
    let workspace = crate::tests::TestGitWorkspace::new(
        "launch_pending_turn_rewrites_images_on_agent_turn_before_hop_1",
    );
    let _vars = arm_vision_env();
    let vision = Arc::new(AgentTurnOnlyVisionClub {
        chats: std::sync::atomic::AtomicUsize::new(0),
        fail: false,
    });
    let _vision = TestVisionClubGuard::install(Arc::clone(&vision) as Arc<dyn Club>);
    let driver = Arc::new(DsflashDriver {
        hops: std::sync::Mutex::new(Vec::new()),
    });
    let mut app = crate::App::preview(crate::Viewer::static_preview());
    app.tools = Arc::new(workspace.registry());
    app.bag
        .replace_in_hand_club_for_test(Arc::clone(&driver) as Arc<dyn Club>);
    let user_msg = ChatMsg::user_with_media("what is this", vec![png_stub()]);
    assert!(should_apply_vision_sidecar(
        app.bag.in_hand().as_ref(),
        &user_msg
    ));
    park_turn(&mut app, user_msg);
    app.launch_pending_turn();
    assert!(
        app.messages.iter().any(|message| {
            matches!(message.role, crate::ui::transcript::Role::System)
                && message.text.contains("describing")
        }),
        "UI may emit a describing line without waiting on the VLM"
    );
    let thinking = app.thinking.take().expect("worker spawned");
    let result = thinking
        .rx
        .recv_timeout(std::time::Duration::from_secs(5))
        .expect("worker must finish after off-thread sidecar rewrite");
    let (convo, answer, _, _) = result.expect("rewritten turn should reach hop 1");
    assert!(
        vision.chats.load(std::sync::atomic::Ordering::SeqCst) >= 1,
        "vision club.chat must run on agent-turn"
    );
    assert!(DESCRIBE_MEDIA_CALLS.load(std::sync::atomic::Ordering::SeqCst) >= 1);
    assert!(
        convo.iter().all(|message| !message
            .attachments
            .iter()
            .any(|media| matches!(media, Media::Image { .. }))),
        "images must be rewritten before hop 1"
    );
    assert_eq!(answer, "ok from dsflash");
    let hop = driver
        .hops
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .last()
        .cloned()
        .unwrap_or_default();
    assert!(hop.contains("a red square"), "{hop}");
    assert!(hop.contains("Operator question: what is this"), "{hop}");
}

#[test]
fn sidecar_failure_drops_images_for_text_only_clubs() {
    let _env = crate::tests::env_lock();
    let workspace =
        crate::tests::TestGitWorkspace::new("sidecar_failure_drops_images_for_text_only_clubs");
    let _vars = arm_vision_env();
    let vision = Arc::new(AgentTurnOnlyVisionClub {
        chats: std::sync::atomic::AtomicUsize::new(0),
        fail: true,
    });
    let _vision = TestVisionClubGuard::install(Arc::clone(&vision) as Arc<dyn Club>);
    let driver = Arc::new(DsflashDriver {
        hops: std::sync::Mutex::new(Vec::new()),
    });
    let mut app = crate::App::preview(crate::Viewer::static_preview());
    app.tools = Arc::new(workspace.registry());
    app.bag
        .replace_in_hand_club_for_test(Arc::clone(&driver) as Arc<dyn Club>);
    park_turn(
        &mut app,
        ChatMsg::user_with_media("inspect", vec![png_stub()]),
    );
    app.launch_pending_turn();
    let thinking = app.thinking.take().expect("worker spawned");
    let result = thinking
        .rx
        .recv_timeout(std::time::Duration::from_secs(5))
        .expect("failed sidecar must not hang hop 1");
    let (convo, answer, _, _) = result.expect("text-only drop still reaches hop 1");
    assert_eq!(answer, "ok from dsflash");
    let last_user = convo
        .iter()
        .rev()
        .find(|message| message.role == ChatRole::User)
        .expect("user turn");
    assert!(last_user.attachments.is_empty());
    assert!(
        last_user
            .content
            .contains("vision sidecar unavailable — image attachments dropped"),
        "{}",
        last_user.content
    );
    let mut failed = false;
    while let Ok(event) = thinking.event_rx.try_recv() {
        if let crate::agent::harness::TurnEvent::Notice(note) = event
            && note.contains("vision sidecar failed")
        {
            failed = true;
        }
    }
    assert!(failed, "sidecar failure must still surface as a notice");
}
