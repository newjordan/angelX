use super::*;

/// Delegates to the crate-wide test env lock (process env is global — a
/// module-local lock can't serialize against other modules' env tests).
fn env_lock() -> std::sync::MutexGuard<'static, ()> {
    crate::tests::env_lock()
}

/// Build a JWT (header.payload.signature) with the given payload JSON. Only the
/// payload segment is real base64url; header/sig are placeholders.
fn fake_jwt(payload: serde_json::Value) -> String {
    let p = base64::engine::general_purpose::URL_SAFE_NO_PAD
        .encode(serde_json::to_vec(&payload).unwrap());
    format!("aaa.{p}.bbb")
}

#[test]
fn jwt_exp_and_account_id_decode() {
    let exp = now_secs() + 3600;
    let tok = fake_jwt(serde_json::json!({
        "exp": exp,
        "https://api.openai.com/auth": { "chatgpt_account_id": "acct_123" }
    }));
    assert_eq!(jwt_exp(&tok), Some(exp));
    assert_eq!(account_id_from_jwt(&tok).as_deref(), Some("acct_123"));
    assert_eq!(jwt_exp("not.a.jwt"), None);
}

#[test]
fn load_parses_chatgpt_auth_and_falls_back_to_id_token_account() {
    // account_id only present inside the id_token claim.
    let id = fake_jwt(serde_json::json!({
        "https://api.openai.com/auth": { "chatgpt_account_id": "acct_from_id" }
    }));
    let v = serde_json::json!({
        "auth_mode": "chatgpt",
        "tokens": { "id_token": id, "access_token": "AT", "refresh_token": "RT", "account_id": "" },
        "last_refresh": "2026-06-17T19:34:00Z",
    });
    let auth = ChatGptAuth::from_value(&v, PathBuf::from("/tmp/x")).expect("loads");
    assert_eq!(auth.access_token, "AT");
    assert_eq!(auth.refresh_token, "RT");
    assert_eq!(auth.account_id, "acct_from_id");
}

#[test]
fn load_rejects_missing_or_empty_token() {
    // No tokens object.
    assert!(ChatGptAuth::from_value(&serde_json::json!({}), PathBuf::from("/x")).is_none());
    // Empty access token (signed out).
    let v = serde_json::json!({ "tokens": { "access_token": "" } });
    assert!(ChatGptAuth::from_value(&v, PathBuf::from("/x")).is_none());
}

#[test]
fn minimal_in_memory_auth_snapshot_loads_without_a_disk_path() {
    let raw = serde_json::json!({
        "tokens": {
            "access_token": "AT",
            "refresh_token": "RT",
            "account_id": "acct"
        }
    })
    .to_string();
    let auth = ChatGptAuth::from_json(&raw, PathBuf::new()).expect("loads");
    assert_eq!(auth.access_token, "AT");
    assert_eq!(auth.refresh_token, "RT");
    assert_eq!(auth.account_id, "acct");
    assert!(auth.path.as_os_str().is_empty());
}

#[test]
fn expired_when_exp_past_or_unreadable() {
    let stale = ChatGptAuth {
        access_token: fake_jwt(serde_json::json!({ "exp": now_secs() - 10 })),
        refresh_token: "RT".into(),
        account_id: "a".into(),
        path: PathBuf::from("/x"),
        disk_snapshot: None,
    };
    assert!(stale.is_expired());
    let fresh = ChatGptAuth {
        access_token: fake_jwt(serde_json::json!({ "exp": now_secs() + 3600 })),
        ..stale.clone()
    };
    assert!(!fresh.is_expired());
    let opaque = ChatGptAuth {
        access_token: "opaque".into(),
        ..stale
    };
    assert!(
        opaque.is_expired(),
        "an unreadable token is treated as expired"
    );
}

pub(super) fn club() -> CodexClub {
    let auth = ChatGptAuth {
        access_token: "AT".into(),
        refresh_token: "RT".into(),
        account_id: "acct".into(),
        path: PathBuf::from("/x"),
        disk_snapshot: None,
    };
    CodexClub::new("openai", "test-openai-model", auth)
}

#[test]
fn responses_request_preserves_optional_shell_scope() {
    use crate::agent::harness::Tool;

    let shell = crate::agent::tools::shell::ShellTool::in_dir(PathBuf::from("/workspace"));
    let definition = shell.def();
    let original = definition.params.clone();
    let body = club().build_request(&[ChatMsg::user("inspect")], &[definition]);
    let tool = &body["tools"][0];
    assert_eq!(tool["name"], "shell");
    assert_eq!(tool["strict"], false);
    assert_eq!(tool["parameters"], original);
    assert_eq!(
        tool["parameters"]["required"],
        serde_json::json!(["command"])
    );
    assert_eq!(
        tool["parameters"]["properties"]["write_paths"]["type"],
        "array"
    );
    assert!(
        tool["parameters"]["properties"]["write_paths"]
            .get("default")
            .is_none()
    );
}

fn reasoning_club() -> CodexClub {
    let auth = ChatGptAuth {
        access_token: "AT".into(),
        refresh_token: "RT".into(),
        account_id: "acct".into(),
        path: PathBuf::from("/x"),
        disk_snapshot: None,
    };
    CodexClub::new_with_reasoning(
        "openai",
        "gpt-test",
        auth,
        Some("high".into()),
        vec!["low".into(), "medium".into(), "high".into()],
    )
}

#[test]
fn openai_codex_resolved_selection_reaches_responses_fixture() {
    let _guard = env_lock();
    use crate::tests::TestEnvGuard;
    let dir = std::env::temp_dir().join(format!("codex-selection-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(
        dir.join("config.toml"),
        "model='gpt-6-astra'\nmodel_reasoning_effort='medium'\n",
    )
    .unwrap();
    std::fs::write(dir.join("models_cache.json"), serde_json::json!({"models":[
            {"slug":"gpt-5.6-luna","display_name":"Luna","supported_reasoning_levels":[{"effort":"max"}]},
            {"slug":"gpt-6-astra","display_name":"Astra","supported_reasoning_levels":[{"effort":"medium"}]}
        ]}).to_string()).unwrap();
    let _codex_home = TestEnvGuard::set("CODEX_HOME", dir.to_str().unwrap());
    let _driver = TestEnvGuard::unset("ANGEL_DRIVER");
    let _global = TestEnvGuard::unset("ANGEL_REASONING_EFFORT");
    for (pins, model, effort, source) in [
        (true, "gpt-5.6-luna", "max", "env"),
        (false, "gpt-6-astra", "medium", "codex-config"),
    ] {
        let _model = if pins {
            TestEnvGuard::set("ANGEL_OPENAI_MODEL", model)
        } else {
            TestEnvGuard::unset("ANGEL_OPENAI_MODEL")
        };
        let _effort = if pins {
            TestEnvGuard::set("ANGEL_OPENAI_REASONING_EFFORT", effort)
        } else {
            TestEnvGuard::unset("ANGEL_OPENAI_REASONING_EFFORT")
        };
        let selection = CodexClub::resolve_selection();
        assert!(selection.error.is_none(), "{:?}", selection.error);
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = listener.local_addr().unwrap();
        let server = std::thread::spawn(move || {
            let (mut sock, _) = listener.accept().unwrap();
            let request = read_http_request(&mut sock);
            let body: serde_json::Value =
                serde_json::from_str(request.split_once("\r\n\r\n").unwrap().1).unwrap();
            sock.write_all(b"HTTP/1.1 200 OK\r\nContent-Type: text/event-stream\r\nConnection: close\r\n\r\ndata: {\"type\":\"response.output_text.delta\",\"delta\":\"ok\"}\n\ndata: {\"type\":\"response.completed\",\"response\":{}}\n\n").unwrap();
            body
        });
        let auth = ChatGptAuth {
            access_token: fake_jwt(serde_json::json!({"exp":now_secs()+3600})),
            refresh_token: "fixture".into(),
            account_id: "fixture".into(),
            path: dir.join("unused-auth.json"),
            disk_snapshot: None,
        };
        let mut club = CodexClub::new_with_reasoning(
            "openai",
            model,
            auth,
            Some(effort.into()),
            vec![effort.into()],
        )
        .with_selection(selection);
        club.responses_url_override = Some(format!("http://{addr}"));
        let defaults = club.resolved_model_defaults();
        assert_eq!(defaults["model_source"], source);
        assert_eq!(defaults["reasoning_effort_source"], source);
        assert_eq!(club.respond("fixture").unwrap(), "ok");
        let wire = server.join().unwrap();
        assert_eq!(wire["model"], model);
        assert_eq!(wire["model"], defaults["model"]);
        assert_eq!(wire["reasoning"]["effort"], effort);
        assert_eq!(wire["reasoning"]["effort"], defaults["reasoning_effort"]);
        assert_eq!(wire["reasoning"]["summary"], "auto");
    }
    let _model = TestEnvGuard::set("ANGEL_OPENAI_MODEL", "gpt-5.6-luna");
    let _effort = TestEnvGuard::set("ANGEL_OPENAI_REASONING_EFFORT", "invented");
    // Invalid selection must return before auth refresh or any POST.
    let club = club().with_selection(CodexClub::resolve_selection());
    let error = club.respond("fixture").unwrap_err();
    assert!(error.contains("supported efforts: [max]"), "{error}");
    assert_eq!(
        club.resolved_model_defaults()["reasoning_effort"],
        "invented"
    );
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn codex_config_reads_model_and_reasoning_effort() {
    let config = config_from_str(
        r#"
model = "gpt-5.6-sol"
model_reasoning_effort = "ultra"
"#,
    )
    .unwrap();
    assert_eq!(config.model.as_deref(), Some("gpt-5.6-sol"));
    assert_eq!(config.model_reasoning_effort.as_deref(), Some("ultra"));
}

#[test]
fn model_catalog_is_visible_sorted_and_carries_exact_efforts() {
    let models = model_catalog_from_str(
        r#"{
  "models": [
    {"slug":"hidden","display_name":"Hidden","visibility":"hide","priority":0},
    {"slug":"terra","display_name":"Terra","visibility":"list","priority":2,
     "default_reasoning_level":"medium","supported_reasoning_levels":[{"effort":"low","description":"fast"},{"effort":"medium","description":"balanced"}]},
    {"slug":"sol","display_name":"Sol","visibility":"list","priority":1,
     "description":"  Frontier\n agentic coding model.  ","context_window":372000,
     "input_modalities":["text","image"],"additional_speed_tiers":["fast"],
     "default_reasoning_level":"ultra","supported_reasoning_levels":[{"effort":"high","description":"deep"},{"effort":"ultra","description":"deepest"}]}
  ]
}"#,
    );
    assert_eq!(models.len(), 2);
    assert_eq!(models[0].slug, "sol");
    assert_eq!(models[0].default_reasoning_level, "ultra");
    assert_eq!(models[0].supported_reasoning_levels[1].effort, "ultra");
    let metadata = models[0].route_metadata();
    assert_eq!(
        metadata.description.as_deref(),
        Some("Frontier agentic coding model.")
    );
    assert_eq!(metadata.context_window, Some(372_000));
    assert_eq!(metadata.input_modalities, ["text", "image"]);
    assert_eq!(metadata.speed_tiers, ["fast"]);
    assert_eq!(metadata.reasoning_description("ULTRA"), Some("deepest"));
    assert_eq!(models[1].slug, "terra");
}

#[test]
fn reasoning_effort_is_backend_owned_and_selects_only_supported_levels() {
    let club = reasoning_club();
    let body = club.build_request(&[ChatMsg::user("hi")], &[]);
    assert_eq!(body["reasoning"]["effort"], "high");
    assert_eq!(
        body["reasoning"]["summary"], "auto",
        "hosted models expose only summaries — dropping them blanks the thinking panel"
    );
    assert_eq!(club.reasoning_effort().as_deref(), Some("high"));
    assert_eq!(club.set_reasoning_effort("low").as_deref(), Some("low"));
    assert_eq!(club.set_reasoning_effort("invented"), None);
    let body = club.build_request(&[ChatMsg::user("again")], &[]);
    assert_eq!(body["reasoning"]["effort"], "low");
}

#[test]
fn per_call_reasoning_effort_does_not_mutate_codex_route_state() {
    let club = reasoning_club();
    let body = club.build_request_with_effort(&[ChatMsg::user("seat")], &[], Some("medium"));
    assert_eq!(body["reasoning"]["effort"], "medium");
    assert_eq!(club.reasoning_effort().as_deref(), Some("high"));

    let body = club.build_request(&[ChatMsg::user("ordinary turn")], &[]);
    assert_eq!(body["reasoning"]["effort"], "high");
}

#[test]
fn legacy_ultra_effort_uses_the_responses_api_xhigh_spelling() {
    let auth = ChatGptAuth {
        access_token: "AT".into(),
        refresh_token: "RT".into(),
        account_id: "acct".into(),
        path: PathBuf::from("/x"),
        disk_snapshot: None,
    };
    let club = CodexClub::new_with_reasoning(
        "openai",
        "gpt-test",
        auth,
        Some("ultra".into()),
        vec!["high".into(), "ultra".into()],
    );
    let body = club.build_request(&[ChatMsg::user("hi")], &[]);
    assert_eq!(body["reasoning"]["effort"], "xhigh");
    assert_eq!(club.reasoning_effort().as_deref(), Some("xhigh"));
    assert_eq!(
        club.resolved_model_defaults()["reasoning_effort"],
        body["reasoning"]["effort"]
    );
}

#[test]
fn selectable_models_share_rotating_auth_and_session_usage() {
    let auth = ChatGptAuth {
        access_token: "AT".into(),
        refresh_token: "RT".into(),
        account_id: "acct".into(),
        path: PathBuf::from("/x"),
        disk_snapshot: None,
    };
    let shared = CodexClub::shared_state(auth);
    let sol = CodexClub::new_with_reasoning_shared(
        "openai",
        "sol",
        Arc::clone(&shared),
        Some("low".into()),
        vec!["low".into(), "high".into()],
    );
    let terra = CodexClub::new_with_reasoning_shared(
        "openai",
        "terra",
        Arc::clone(&shared),
        Some("medium".into()),
        vec!["medium".into(), "high".into()],
    );
    assert!(Arc::ptr_eq(&sol.shared, &terra.shared));
    sol.record_usage(Usage {
        input: Some(11),
        output: Some(3),
        reasoning: Some(2),
        ..Usage::default()
    });
    assert_eq!(terra.token_usage().map(|usage| usage.turns), Some(1));
    sol.shared.auth.lock().unwrap().refresh_token = "ROTATED".into();
    assert_eq!(
        terra.shared.auth.lock().unwrap().refresh_token,
        "ROTATED",
        "every model must see the latest rotating refresh token"
    );
}

#[test]
fn token_adopts_credentials_rotated_by_another_process() {
    static NEXT_PATH: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
    let nonce = NEXT_PATH.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let dir = std::env::temp_dir().join(format!(
        "angel_codex_auth_adopt_{}_{}",
        std::process::id(),
        nonce
    ));
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("auth.json");
    let fresh_access = fake_jwt(serde_json::json!({ "exp": now_secs() + 3600 }));
    std::fs::write(
        &path,
        serde_json::to_vec(&serde_json::json!({
            "auth_mode": "chatgpt",
            "tokens": {
                "access_token": fresh_access,
                "refresh_token": "disk-rotated-refresh",
                "account_id": "disk-account"
            }
        }))
        .unwrap(),
    )
    .unwrap();

    let stale = ChatGptAuth {
        access_token: fake_jwt(serde_json::json!({ "exp": now_secs() - 10 })),
        refresh_token: "stale-refresh".into(),
        account_id: "stale-account".into(),
        path: path.clone(),
        disk_snapshot: None,
    };
    let club = CodexClub::new("openai", "test-openai-model", stale);
    let (access, account) = club.token().expect("fresh disk token avoids refresh");
    assert_eq!(access, fresh_access);
    assert_eq!(account, "disk-account");
    assert_eq!(
        club.shared.auth.lock().unwrap().refresh_token,
        "disk-rotated-refresh"
    );

    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn concurrent_token_reads_on_a_valid_token_share_the_fast_path() {
    // A fresh disk token: every concurrent caller returns it via the fast
    // path (cheap lock, no refresh gate) with no deadlock or lock inversion —
    // guards the single-flight refactor against a concurrency regression.
    let dir = std::env::temp_dir().join(format!(
        "angel_codex_auth_concurrent_{}_{}",
        std::process::id(),
        now_secs()
    ));
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("auth.json");
    let fresh_access = fake_jwt(serde_json::json!({ "exp": now_secs() + 3600 }));
    std::fs::write(
        &path,
        serde_json::to_vec(&serde_json::json!({
            "auth_mode": "chatgpt",
            "tokens": {
                "access_token": fresh_access,
                "refresh_token": "r",
                "account_id": "acct"
            }
        }))
        .unwrap(),
    )
    .unwrap();
    let auth = ChatGptAuth::from_path(path.clone()).unwrap();
    let club = Arc::new(CodexClub::new("openai", "test-openai-model", auth));
    let handles: Vec<_> = (0..8)
        .map(|_| {
            let c = Arc::clone(&club);
            std::thread::spawn(move || c.token())
        })
        .collect();
    for h in handles {
        let (access, account) = h.join().unwrap().expect("valid token returns");
        assert_eq!(access, fresh_access);
        assert_eq!(account, "acct");
    }
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn unchanged_stale_disk_does_not_undo_an_in_memory_refresh() {
    let dir = std::env::temp_dir().join(format!(
        "angel_codex_auth_persist_{}_{}",
        std::process::id(),
        now_secs()
    ));
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("auth.json");
    let write_auth = |access: &str, refresh: &str| {
        std::fs::write(
            &path,
            serde_json::to_vec(&serde_json::json!({
                "auth_mode": "chatgpt",
                "tokens": {
                    "access_token": access,
                    "refresh_token": refresh,
                    "account_id": "account"
                }
            }))
            .unwrap(),
        )
        .unwrap();
    };
    write_auth("disk-old-access", "disk-old-refresh");
    let mut auth = ChatGptAuth::from_path(path.clone()).unwrap();

    // Model a successful refresh whose best-effort disk write failed.
    auth.access_token = "memory-new-access".into();
    auth.refresh_token = "memory-new-refresh".into();
    assert!(!auth.adopt_disk_credentials());
    assert_eq!(auth.access_token, "memory-new-access");
    assert_eq!(auth.refresh_token, "memory-new-refresh");

    // A genuinely different later disk snapshot is still adopted.
    write_auth("external-new-access", "external-new-refresh");
    assert!(auth.adopt_disk_credentials());
    assert_eq!(auth.access_token, "external-new-access");
    assert_eq!(auth.refresh_token, "external-new-refresh");

    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn refresh_conflict_guard_preserves_a_later_disk_rotation() {
    let dir = std::env::temp_dir().join(format!(
        "angel_codex_auth_race_{}_{}",
        std::process::id(),
        now_secs()
    ));
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("auth.json");
    let source = ChatGptAuth {
        access_token: "source-access".into(),
        refresh_token: "source-refresh".into(),
        account_id: "source-account".into(),
        path: path.clone(),
        disk_snapshot: None,
    };
    std::fs::write(
        &path,
        serde_json::to_vec(&serde_json::json!({
            "auth_mode": "chatgpt",
            "tokens": {
                "access_token": "later-access",
                "refresh_token": "later-refresh",
                "account_id": "later-account"
            }
        }))
        .unwrap(),
    )
    .unwrap();

    let mut refresh_result = source.clone();
    refresh_result.access_token = "network-response-access".into();
    refresh_result.refresh_token = "network-response-refresh".into();
    assert!(refresh_result.adopt_disk_credentials_changed_since(&source));
    assert_eq!(refresh_result.access_token, "later-access");
    assert_eq!(refresh_result.refresh_token, "later-refresh");
    assert_eq!(refresh_result.account_id, "later-account");

    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn persist_conflict_guard_never_overwrites_later_disk_credentials() {
    let dir = std::env::temp_dir().join(format!(
        "angel_codex_auth_persist_race_{}_{}",
        std::process::id(),
        now_secs()
    ));
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("auth.json");
    let source = ChatGptAuth {
        access_token: "source-access".into(),
        refresh_token: "source-refresh".into(),
        account_id: "source-account".into(),
        path: path.clone(),
        disk_snapshot: None,
    };
    std::fs::write(
        &path,
        serde_json::to_vec(&serde_json::json!({
            "auth_mode": "chatgpt",
            "tokens": {
                "access_token": "later-access",
                "refresh_token": "later-refresh",
                "account_id": "later-account"
            }
        }))
        .unwrap(),
    )
    .unwrap();

    let mut refresh_result = source.clone();
    refresh_result.access_token = "network-response-access".into();
    refresh_result.refresh_token = "network-response-refresh".into();
    refresh_result.persist(None, &source);
    assert_eq!(refresh_result.access_token, "later-access");
    assert_eq!(refresh_result.refresh_token, "later-refresh");
    let disk = ChatGptAuth::from_path(path.clone()).unwrap();
    assert_eq!(disk.access_token, "later-access");
    assert_eq!(disk.refresh_token, "later-refresh");

    let _ = std::fs::remove_dir_all(dir);
}

#[cfg(unix)]
#[test]
fn auth_persist_preserves_permissions_and_leaves_no_shared_temp_file() {
    use std::os::unix::fs::PermissionsExt;

    let dir = std::env::temp_dir().join(format!(
        "angel_codex_auth_mode_{}_{}",
        std::process::id(),
        now_secs()
    ));
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("auth.json");
    std::fs::write(
        &path,
        serde_json::to_vec(&serde_json::json!({
            "auth_mode": "chatgpt",
            "tokens": {
                "access_token": "source-access",
                "refresh_token": "source-refresh",
                "account_id": "source-account"
            }
        }))
        .unwrap(),
    )
    .unwrap();
    std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o640)).unwrap();
    let source = ChatGptAuth::from_path(path.clone()).unwrap();
    let mut refreshed = source.clone();
    refreshed.access_token = "refreshed-access".into();
    refreshed.refresh_token = "refreshed-refresh".into();

    refreshed.persist(None, &source);

    assert_eq!(
        std::fs::metadata(&path).unwrap().permissions().mode() & 0o777,
        0o640
    );
    let disk = ChatGptAuth::from_path(path.clone()).unwrap();
    assert_eq!(disk.access_token, "refreshed-access");
    assert_eq!(disk.refresh_token, "refreshed-refresh");
    let entries = std::fs::read_dir(&dir)
        .unwrap()
        .map(|entry| entry.unwrap().file_name())
        .collect::<Vec<_>>();
    assert_eq!(entries, [std::ffi::OsString::from("auth.json")]);

    let _ = std::fs::remove_dir_all(dir);
}

/// The ChatGPT-backed Codex Responses endpoint rejects `max_output_tokens`
/// ("HTTP 400: Unsupported parameter"), so the request must never carry it —
/// tuned or not. A SOTA link relies on `keep_truncated` for a cut-off reply.
#[test]
fn build_request_never_sends_max_output_tokens() {
    for club in [club(), club().sota_tuned()] {
        let body = club.build_request(&[ChatMsg::user("hi")], &[]);
        assert!(
            body.get("max_output_tokens").is_none(),
            "Codex Responses body must not carry max_output_tokens: {body}"
        );
        assert_eq!(
            club.route_metadata().output_budget,
            crate::agent::club::OutputBudgetPolicy::EndpointManaged
        );
    }
}

/// Read one full HTTP request (headers + `Content-Length` body) off a
/// socket, mirroring the local-server helpers the chat-route tests use.
fn read_http_request(sock: &mut std::net::TcpStream) -> String {
    use std::io::Read;
    let mut req = Vec::new();
    let mut buf = [0u8; 1024];
    while let Ok(n) = sock.read(&mut buf) {
        if n == 0 {
            break;
        }
        req.extend_from_slice(&buf[..n]);
        if let Some(pos) = req.windows(4).position(|w| w == b"\r\n\r\n") {
            let headers = String::from_utf8_lossy(&req[..pos]).to_ascii_lowercase();
            let need = headers
                .lines()
                .find_map(|l| l.strip_prefix("content-length:"))
                .and_then(|v| v.trim().parse::<usize>().ok())
                .unwrap_or(0);
            if req.len() >= pos + 4 + need {
                break;
            }
        }
    }
    String::from_utf8_lossy(&req).into_owned()
}

#[test]
fn codex_stream_stall_knob_defaults_disables_and_falls_back() {
    let _guard = env_lock();
    {
        let _unset = crate::tests::TestEnvGuard::unset(CODEX_STREAM_STALL_ENV);
        assert_eq!(codex_stream_stall_secs(), 120);
    }
    {
        let _off = crate::tests::TestEnvGuard::set(CODEX_STREAM_STALL_ENV, "0");
        assert_eq!(codex_stream_stall_secs(), 0);
    }
    {
        let _invalid = crate::tests::TestEnvGuard::set(CODEX_STREAM_STALL_ENV, "not-secs");
        assert_eq!(codex_stream_stall_secs(), 120);
    }
}

/// A Responses stream that answers HTTP 200, emits one `response.created`
/// event, and then goes silent must fail at the stall bound instead of
/// holding the turn until the plain 300 s socket timeout (observed live:
/// `openai/gpt-6-astra@high` silent for 8+ minutes while steers queued).
#[test]
fn codex_stream_stall_bound_fires_on_a_silent_responses_stream() {
    let _guard = env_lock();
    {
        // Must be set before construction: the knob is resolved where the
        // ureq agent (and its read deadline) is built.
        let _stall = crate::tests::TestEnvGuard::set(CODEX_STREAM_STALL_ENV, "1");
        use std::io::Write;
        use std::net::TcpListener;
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = listener.local_addr().unwrap();
        let handle = std::thread::spawn(move || {
            let Ok((mut sock, _)) = listener.accept() else {
                return;
            };
            let _ = read_http_request(&mut sock);
            let _ = sock.write_all(
                b"HTTP/1.1 200 OK\r\nContent-Type: text/event-stream\r\nConnection: close\r\n\r\n",
            );
            // One real event, then silence: the 1 s stall bound must fire
            // long before the plain 300 s read timeout.
            let _ = sock.write_all(
                b"event: response.created\n\
                       data: {\"type\":\"response.created\",\"response\":{}}\n\n",
            );
            let _ = sock.flush();
            std::thread::sleep(std::time::Duration::from_secs(3));
        });
        let auth = ChatGptAuth {
            access_token: fake_jwt(serde_json::json!({ "exp": now_secs() + 3600 })),
            refresh_token: "RT".into(),
            account_id: "acct".into(),
            path: PathBuf::from("/x"),
            disk_snapshot: None,
        };
        let mut club = CodexClub::new("openai", "test-openai-model", auth);
        club.responses_url_override = Some(format!("http://{addr}"));
        let started = std::time::Instant::now();
        let err = club
            .chat_streaming(
                &[ChatMsg::user("hi")],
                &[],
                &AtomicBool::new(false),
                &mut |_| {},
            )
            .expect_err("a silent Responses stream must fail, not hang");
        assert!(err.contains("stream stalled"), "{err}");
        assert!(err.contains(CODEX_STREAM_STALL_ENV), "{err}");
        assert!(err.contains("test-openai-model"), "{err}");
        assert!(
            started.elapsed() < std::time::Duration::from_secs(3),
            "the stall bound must fire well before the 300 s plain timeout ({:?})",
            started.elapsed()
        );
        let stats = club.shared.usage.lock().unwrap();
        assert_eq!(
            stats.recent_attempts.back().expect("one receipt").outcome,
            "stalled"
        );
        drop(handle.join());
    }
}

/// A stream that keeps sending keep-alive comments and bookkeeping frames
/// but never produces output must also stall out: bytes are not progress.
#[test]
fn codex_stream_stall_bound_fires_on_a_heartbeat_only_responses_stream() {
    let _guard = env_lock();
    {
        let _stall = crate::tests::TestEnvGuard::set(CODEX_STREAM_STALL_ENV, "1");
        use std::io::Write;
        use std::net::TcpListener;
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = listener.local_addr().unwrap();
        let handle = std::thread::spawn(move || {
            let Ok((mut sock, _)) = listener.accept() else {
                return;
            };
            let _ = read_http_request(&mut sock);
            let _ = sock.write_all(
                b"HTTP/1.1 200 OK\r\nContent-Type: text/event-stream\r\nConnection: close\r\n\r\n",
            );
            let _ = sock.write_all(b"data: {\"type\":\"response.created\",\"response\":{}}\n\n");
            let _ = sock.flush();
            // Keep-alives every 200 ms for 4 s: the socket never times out,
            // yet nothing meaningful arrives.
            for _ in 0..20 {
                if sock
                    .write_all(
                        b": ping\n\ndata: {\"type\":\"response.in_progress\",\"response\":{}}\n\n",
                    )
                    .is_err()
                {
                    break;
                }
                let _ = sock.flush();
                std::thread::sleep(std::time::Duration::from_millis(200));
            }
        });
        let auth = ChatGptAuth {
            access_token: fake_jwt(serde_json::json!({ "exp": now_secs() + 3600 })),
            refresh_token: "RT".into(),
            account_id: "acct".into(),
            path: PathBuf::from("/x"),
            disk_snapshot: None,
        };
        let mut club = CodexClub::new("openai", "test-openai-model", auth);
        club.responses_url_override = Some(format!("http://{addr}"));
        let started = std::time::Instant::now();
        let err = club
            .chat_streaming(
                &[ChatMsg::user("hi")],
                &[],
                &AtomicBool::new(false),
                &mut |_| {},
            )
            .expect_err("a heartbeat-only Responses stream must fail, not hang");
        assert!(err.contains("stream stalled"), "{err}");
        assert!(err.contains("keep-alives"), "{err}");
        assert!(err.contains(CODEX_STREAM_STALL_ENV), "{err}");
        assert!(
            started.elapsed() < std::time::Duration::from_secs(3),
            "heartbeats must not defeat the stall bound ({:?})",
            started.elapsed()
        );
        let stats = club.shared.usage.lock().unwrap();
        assert_eq!(
            stats.recent_attempts.back().expect("one receipt").outcome,
            "stalled"
        );
        drop(handle.join());
    }
}

#[test]
fn build_request_maps_system_to_instructions_and_roles() {
    let msgs = vec![
        ChatMsg::system("be terse"),
        ChatMsg::user("hi"),
        ChatMsg::assistant("hello"),
        ChatMsg::user("more"),
    ];
    let body = club().build_request(&msgs, &[]);
    assert_eq!(body["model"], "test-openai-model");
    assert_eq!(body["instructions"], "be terse");
    assert_eq!(body["stream"], true);
    assert_eq!(body["store"], false);
    let input = body["input"].as_array().unwrap();
    assert_eq!(input.len(), 3, "system is lifted out of input");
    assert_eq!(input[0]["role"], "user");
    assert_eq!(input[0]["content"][0]["type"], "input_text");
    assert_eq!(input[0]["content"][0]["text"], "hi");
    assert_eq!(input[1]["role"], "assistant");
    assert_eq!(input[1]["content"][0]["type"], "output_text");
}

#[test]
fn sota_tuned_build_request_keeps_caveman_opt_in() {
    let _guard = env_lock();
    let saved = std::env::var_os("ANGEL_SOTA_CAVEMAN");
    // TODO: Audit that the environment access only happens in single-threaded code.
    unsafe { std::env::remove_var("ANGEL_SOTA_CAVEMAN") };

    let body = club()
        .sota_tuned()
        .build_request(&[ChatMsg::user("hi")], &[]);
    assert_eq!(body["instructions"], "");

    // TODO: Audit that the environment access only happens in single-threaded code.
    unsafe { std::env::set_var("ANGEL_SOTA_CAVEMAN", "1") };
    let body = club()
        .sota_tuned()
        .build_request(&[ChatMsg::user("hi")], &[]);
    assert!(
        body["instructions"]
            .as_str()
            .unwrap_or_default()
            .contains("angelX SOTA brevity mode"),
        "{body}"
    );
    assert_eq!(body["input"][0]["role"], "user");
    assert_eq!(body["input"][0]["content"][0]["text"], "hi");

    match saved {
        // TODO: Audit that the environment access only happens in single-threaded code.
        Some(v) => unsafe { std::env::set_var("ANGEL_SOTA_CAVEMAN", v) },
        // TODO: Audit that the environment access only happens in single-threaded code.
        None => unsafe { std::env::remove_var("ANGEL_SOTA_CAVEMAN") },
    }
}

#[test]
fn build_request_preserves_image_attachments_for_responses_api() {
    let msg = ChatMsg::user_with_media(
        "what is this",
        vec![Media::Image {
            mime: "image/png".into(),
            b64: "AAAA".into(),
        }],
    );
    let body = club().build_request(&[msg], &[]);
    let content = body["input"][0]["content"].as_array().unwrap();
    assert_eq!(content[0]["type"], "input_text");
    assert_eq!(content[0]["text"], "what is this");
    assert_eq!(content[1]["type"], "input_image");
    assert_eq!(content[1]["image_url"], "data:image/png;base64,AAAA");
}

#[test]
fn final_mile_codex_retains_schemas_and_disables_calls() {
    let _guard = crate::tests::env_lock();
    let tools = [ToolDef {
        name: "read_file".into(),
        description: "Read".into(),
        params: serde_json::json!({"type":"object", "properties":{}}),
    }];
    let mut messages = vec![ChatMsg::system("stable"), ChatMsg::user("work")];
    let before = club().build_request(&messages, &tools);
    messages.push(ChatMsg::harness(
        crate::agent::club::FINAL_MILE_ANSWER_NUDGE,
    ));
    let after = club().build_request(&messages, &tools);
    assert_eq!(before["tools"], after["tools"]);
    assert_eq!(before["instructions"], after["instructions"]);
    assert_eq!(after["tool_choice"], "none");
    let old = before["input"].as_array().unwrap();
    assert_eq!(
        old.as_slice(),
        &after["input"].as_array().unwrap()[..old.len()]
    );
    messages.push(ChatMsg::user("continue"));
    assert!(
        club()
            .build_request(&messages, &tools)
            .get("tool_choice")
            .is_none()
    );
}

#[test]
fn build_request_advertises_tools_and_preserves_tool_history() {
    let tools = [ToolDef {
        name: "list_dir".into(),
        description: "List files".into(),
        params: serde_json::json!({
            "type": "object",
            "properties": { "path": { "type": "string" } },
            "required": ["path"]
        }),
    }];
    let msgs = vec![
        ChatMsg::user("inspect"),
        ChatMsg::assistant_calls(vec![ToolCall {
            id: "call_1".into(),
            name: "list_dir".into(),
            args: serde_json::json!({ "path": "." }),
        }]),
        ChatMsg::tool("call_1", "README.md\nsrc"),
    ];
    let body = club().build_request(&msgs, &tools);
    assert_eq!(body["tools"][0]["type"], "function");
    assert_eq!(body["tools"][0]["name"], "list_dir");
    assert_eq!(body["tools"][0]["parameters"]["required"][0], "path");
    let input = body["input"].as_array().unwrap();
    assert_eq!(input[1]["type"], "function_call");
    assert_eq!(input[1]["call_id"], "call_1");
    assert_eq!(input[1]["name"], "list_dir");
    assert_eq!(input[1]["arguments"], r#"{"path":"."}"#);
    assert_eq!(input[2]["type"], "function_call_output");
    assert_eq!(input[2]["call_id"], "call_1");
    assert_eq!(input[2]["output"], "README.md\nsrc");
}

#[test]
fn sse_events_decode_text_reasoning_done_and_error() {
    let text =
        parse_responses_event(r#"data: {"type":"response.output_text.delta","delta":"Hel"}"#);
    assert!(matches!(text, ResponseEvent::Text(t) if t == "Hel"));
    let summary = parse_responses_event(
        r#"data: {"type":"response.reasoning_summary_text.delta","delta":"think"}"#,
    );
    assert!(matches!(summary, ResponseEvent::ReasoningSummary(s) if s == "think"));
    let reason =
        parse_responses_event(r#"data: {"type":"response.reasoning_text.delta","delta":"raw"}"#);
    assert!(matches!(reason, ResponseEvent::Reasoning(r) if r == "raw"));
    let start = parse_responses_event(
        r#"data: {"type":"response.output_item.added","item":{"type":"function_call","id":"fc_1","call_id":"call_1","name":"list_dir"}}"#,
    );
    assert!(matches!(
        start,
        ResponseEvent::ToolCallStart { key, call_id, name }
            if key == "fc_1" && call_id.as_deref() == Some("call_1") && name.as_deref() == Some("list_dir")
    ));
    let delta = parse_responses_event(
        r#"data: {"type":"response.function_call_arguments.delta","item_id":"fc_1","delta":"{\"path\""}"#,
    );
    assert!(matches!(
        delta,
        ResponseEvent::ToolArgumentsDelta { key, delta }
            if key == "fc_1" && delta == "{\"path\""
    ));
    let done = parse_responses_event(
        r#"data: {"type":"response.output_item.done","item":{"type":"function_call","id":"fc_1","call_id":"call_1","name":"list_dir","arguments":"{\"path\":\".\"}"}}"#,
    );
    assert!(matches!(
        done,
        ResponseEvent::ToolCallDone { key, call }
            if key == "fc_1" && call.id == "call_1" && call.name == "list_dir" && call.args["path"] == "."
    ));
    // A bare complete carries no usage; one with `usage` is parsed through.
    assert!(matches!(
        parse_responses_event(r#"data: {"type":"response.completed","response":{}}"#),
        ResponseEvent::Done(None)
    ));
    let done_usage = parse_responses_event(
        r#"data: {"type":"response.completed","response":{"usage":{"input_tokens":12,"output_tokens":34,"output_tokens_details":{"reasoning_tokens":8}}}}"#,
    );
    assert!(matches!(
        done_usage,
        ResponseEvent::Done(Some(u)) if u.input == Some(12) && u.output == Some(34) && u.reasoning == Some(8)
    ));
    let incomplete = parse_responses_event(
        r#"data: {"type":"response.completed","response":{"status":"incomplete","incomplete_details":{"reason":"max_output_tokens"}}}"#,
    );
    assert!(matches!(
        incomplete,
        ResponseEvent::Incomplete(m, _)
            if m == "response incomplete: endpoint reported max_output_tokens (plan-managed)"
    ));
    let fail = parse_responses_event(
        r#"data: {"type":"response.failed","response":{"error":{"message":"nope"}}}"#,
    );
    assert!(matches!(fail, ResponseEvent::Failed(m, _) if m == "nope"));
    // event: lines, comments, blanks, and unrelated events are ignored.
    assert!(matches!(
        parse_responses_event("event: response.output_text.delta"),
        ResponseEvent::Ignore
    ));
    assert!(matches!(
        parse_responses_event(": keep-alive"),
        ResponseEvent::Ignore
    ));
    assert!(matches!(
        parse_responses_event(r#"data: {"type":"response.created"}"#),
        ResponseEvent::Ignore
    ));
}

#[test]
fn response_tool_call_accumulator_builds_calls_from_deltas() {
    let mut calls = ResponseToolCalls::default();
    calls.start(
        "fc_1".into(),
        Some("call_1".into()),
        Some("list_dir".into()),
    );
    calls.push_args("fc_1", r#"{"path""#);
    calls.push_args("fc_1", r#":"."}"#);
    let (out, notes) = calls.into_calls_with_notes();
    assert!(
        notes.is_empty(),
        "a clean stream needs no repair: {notes:?}"
    );
    assert_eq!(out.len(), 1);
    assert_eq!(out[0].id, "call_1");
    assert_eq!(out[0].name, "list_dir");
    assert_eq!(out[0].args["path"], ".");
}

#[test]
fn response_tool_call_accumulator_merges_item_and_call_ids() {
    // Responses streams identify the output item as `fc_*`, while the
    // dispatch/result protocol uses `call_*`. A completed item must finish
    // the started call, not create a second callable entry.
    let mut calls = ResponseToolCalls::default();
    calls.start(
        "fc_1".into(),
        Some("call_1".into()),
        Some("list_dir".into()),
    );
    // Some gateways key the final argument event by call_id instead of
    // item_id; this must still join the original entry.
    calls.set_args("call_1", r#"{"path":"."}"#.into());
    calls.done(
        "fc_1".into(),
        ToolCall {
            id: "call_1".into(),
            name: "list_dir".into(),
            args: serde_json::json!({"path":"."}),
        },
    );

    let (out, notes) = calls.into_calls_with_notes();
    assert!(
        notes.is_empty(),
        "a clean stream needs no repair: {notes:?}"
    );
    assert_eq!(out.len(), 1, "one logical stream item must dispatch once");
    assert_eq!(out[0].id, "call_1");
    assert_eq!(out[0].name, "list_dir");
    assert_eq!(out[0].args["path"], ".");
}

#[test]
fn response_tool_call_accumulator_keeps_multi_call_item_order() {
    let mut calls = ResponseToolCalls::default();
    calls.start(
        "fc_first".into(),
        Some("call_first".into()),
        Some("read_file".into()),
    );
    calls.start(
        "fc_second".into(),
        Some("call_second".into()),
        Some("list_dir".into()),
    );
    // Completion order is allowed to differ from output-item order.
    calls.done(
        "fc_second".into(),
        ToolCall {
            id: "call_second".into(),
            name: "list_dir".into(),
            args: serde_json::json!({"path":"src"}),
        },
    );
    calls.done(
        "fc_first".into(),
        ToolCall {
            id: "call_first".into(),
            name: "read_file".into(),
            args: serde_json::json!({"path":"Cargo.toml"}),
        },
    );

    let (out, notes) = calls.into_calls_with_notes();
    assert!(
        notes.is_empty(),
        "a clean stream needs no repair: {notes:?}"
    );
    assert_eq!(out.len(), 2);
    assert_eq!(out[0].id, "call_first");
    assert_eq!(out[1].id, "call_second");
}

/// Anything the assembler repairs or drops is reported, never silent: a call
/// that completes under a different name than it started with, completed
/// arguments that differ from the streamed ones, final arguments that replace
/// the deltas, and argument bytes that never got a name and so cannot dispatch.
#[test]
fn response_tool_call_accumulator_notes_repairs_and_drops() {
    let mut calls = ResponseToolCalls::default();
    calls.start("fc_1".into(), Some("call_1".into()), Some("shell".into()));
    calls.push_args("fc_1", r#"{"command":"cmake -S . -B build"}"#);
    calls.done(
        "fc_1".into(),
        ToolCall {
            id: "call_1".into(),
            name: "cargo".into(),
            args: serde_json::json!({"args": "?"}),
        },
    );
    calls.start(
        "fc_2".into(),
        Some("call_2".into()),
        Some("read_file".into()),
    );
    calls.push_args("fc_2", r#"{"path":"a"}"#);
    calls.set_args("fc_2", r#"{"path":"b"}"#.into());
    calls.push_args("output:3", r#"{"path":"src/lib.rs"}"#);

    let (out, notes) = calls.into_calls_with_notes();
    assert_eq!(out.len(), 2, "the unnamed entry cannot dispatch");
    assert_eq!(out[0].name, "cargo", "the completed item wins");
    assert_eq!(out[1].args["path"], "b");
    let kinds: Vec<&str> = notes.iter().map(|(kind, _)| *kind).collect();
    assert_eq!(
        kinds,
        [
            "tool_name_changed",
            "tool_args_changed",
            "tool_args_replaced",
            "tool_entry_dropped"
        ]
    );
    assert!(
        notes[0].1.contains("started as shell, completed as cargo"),
        "{notes:?}"
    );
    assert!(notes[3].1.contains("src/lib.rs"), "{notes:?}");
}

#[test]
fn format_usage_renders_tokens_and_limits() {
    // Nothing reported yet → no status noise.
    assert!(format_usage(&UsageStats::default()).is_none());

    let stats = UsageStats {
        turns: 2,
        last: Usage {
            input: Some(100),
            output: Some(50),
            reasoning: Some(20),
            ..Usage::default()
        },
        total_input: 300,
        total_output: 180,
        total_reasoning: 40,
        rate_limits: vec![
            ("x-codex-plan-type".into(), "pro".into()),
            ("x-codex-active-limit".into(), "premium".into()),
            ("x-codex-primary-used-percent".into(), "37".into()),
            ("x-codex-primary-reset-after-seconds".into(), "1185".into()),
            ("x-codex-primary-window-minutes".into(), "300".into()),
        ],
        ..UsageStats::default()
    };
    let out = format_usage(&stats).expect("usage present");
    assert!(out.contains("turns     2"));
    assert!(out.contains("last      in 100 · out 50 · reasoning 20"));
    assert!(out.contains("total 480"));
    assert!(out.contains("mix       in [########....]  63% · out [#####.......]  38%"));
    assert!(out.contains("plan"));
    assert!(out.contains("type      pro · active premium"));
    assert!(out.contains("primary   [####........]  37% · reset 19m · window 5h"));
}

#[test]
fn parse_usage_handles_absent_partial_and_all_zero() {
    // Absent usage object → None.
    assert!(parse_usage(None).is_none());
    // Explicit zero is reported usage, distinct from absent/unknown usage.
    let zero = serde_json::json!({ "input_tokens": 0, "output_tokens": 0 });
    assert_eq!(parse_usage(Some(&zero)).unwrap().input, Some(0));
    // Only reasoning tokens present → still a Usage.
    let only_reasoning = serde_json::json!({
        "output_tokens_details": { "reasoning_tokens": 7 }
    });
    let u = parse_usage(Some(&only_reasoning)).expect("reasoning-only counts");
    assert_eq!((u.input, u.output, u.reasoning), (None, None, Some(7)));
    // Input only, with a misshaped reasoning detail (ignored).
    let input_only = serde_json::json!({ "input_tokens": 5, "output_tokens_details": 9 });
    let u2 = parse_usage(Some(&input_only)).expect("input counts");
    assert_eq!((u2.input, u2.output, u2.reasoning), (Some(5), None, None));
}

#[test]
fn record_usage_accumulates_and_usage_report_renders() {
    let c = club();
    // No turns yet → no report.
    assert!(c.usage_report().is_none());
    assert!(c.token_usage().is_none());
    c.record_usage(Usage {
        input: Some(10),
        output: Some(4),
        reasoning: Some(0),
        ..Usage::default()
    });
    c.record_usage(Usage {
        input: Some(20),
        output: Some(6),
        reasoning: Some(3),
        ..Usage::default()
    });
    let report = c.usage_report().expect("usage after two turns");
    assert!(report.contains("turns     2"));
    // `last` reflects the most recent turn; totals sum across both.
    assert!(report.contains("last      in 20 · out 6 · reasoning 3"));
    assert!(report.contains("session   in 30 · out 10 · total 40"));
    let usage = c.token_usage().expect("structured usage after two turns");
    assert_eq!(usage.turns, 2);
    assert_eq!(
        (usage.last_input, usage.last_output, usage.last_reasoning),
        (20, 6, 3)
    );
    assert_eq!(
        (usage.total_input, usage.total_output, usage.total_reasoning),
        (30, 10, 3)
    );
}

#[test]
fn format_usage_omits_reasoning_when_zero_and_keeps_plain_limit_labels() {
    let stats = UsageStats {
        turns: 1,
        last: Usage {
            input: Some(8),
            output: Some(2),
            reasoning: Some(0),
            ..Usage::default()
        },
        total_input: 8,
        total_output: 2,
        total_reasoning: 0,
        // A non-vendor-prefixed limit header keeps its name (dashes → spaces).
        rate_limits: vec![("ratelimit-remaining".into(), "9".into())],
        ..UsageStats::default()
    };
    let out = format_usage(&stats).expect("present");
    assert!(
        !out.contains("(reasoning"),
        "no reasoning suffix when zero:\n{out}"
    );
    assert!(out.contains("limits    ratelimit remaining 9"));
}

#[test]
fn synth_session_id_is_uuid_shaped() {
    let id = synth_session_id();
    let parts: Vec<&str> = id.split('-').collect();
    assert_eq!(parts.len(), 5);
    assert_eq!(
        parts.iter().map(|p| p.len()).collect::<Vec<_>>(),
        vec![8, 4, 4, 4, 12]
    );
    assert!(id.chars().all(|c| c.is_ascii_hexdigit() || c == '-'));
}

#[test]
fn default_model_comes_from_env_or_codex_config() {
    let _guard = env_lock();
    let old_env = std::env::var("ANGEL_OPENAI_MODEL").ok();
    let old_home = std::env::var("CODEX_HOME").ok();
    let old_effort = std::env::var("ANGEL_OPENAI_REASONING_EFFORT").ok();
    // TODO: Audit that the environment access only happens in single-threaded code.
    unsafe { std::env::set_var("ANGEL_OPENAI_MODEL", " env-model ") };
    assert_eq!(CodexClub::default_model().as_deref(), Some("env-model"));

    // TODO: Audit that the environment access only happens in single-threaded code.
    unsafe { std::env::remove_var("ANGEL_OPENAI_MODEL") };
    let dir = std::env::temp_dir().join(format!(
        "angel_codex_home_{}_{}",
        std::process::id(),
        now_secs()
    ));
    std::fs::create_dir_all(&dir).expect("create temp codex home");
    std::fs::write(dir.join("config.toml"), "model = \"config-model\"\n")
        .expect("write temp config");
    // TODO: Audit that the environment access only happens in single-threaded code.
    unsafe { std::env::set_var("CODEX_HOME", &dir) };
    assert_eq!(CodexClub::default_model().as_deref(), Some("config-model"));
    let _ = std::fs::remove_dir_all(dir);

    // A forbidden pin stays visible so validation can refuse it truthfully.
    // TODO: Audit that the environment access only happens in single-threaded code.
    unsafe { std::env::set_var("ANGEL_OPENAI_MODEL", "gpt-5.3-codex-spark") };
    assert_eq!(
        CodexClub::default_model().as_deref(),
        Some("gpt-5.3-codex-spark")
    );
    assert!(CodexClub::resolve_selection().error.is_some());
    assert!(is_private_test_codex_model("gpt-5.3-codex-spark"));
    assert_eq!(
        canonicalize_openai_model("gpt-5.3-codex-spark"),
        OPENAI_LUNA_MODEL
    );

    match old_env {
        // TODO: Audit that the environment access only happens in single-threaded code.
        Some(v) => unsafe { std::env::set_var("ANGEL_OPENAI_MODEL", v) },
        // TODO: Audit that the environment access only happens in single-threaded code.
        None => unsafe { std::env::remove_var("ANGEL_OPENAI_MODEL") },
    }
    match old_home {
        // TODO: Audit that the environment access only happens in single-threaded code.
        Some(v) => unsafe { std::env::set_var("CODEX_HOME", v) },
        // TODO: Audit that the environment access only happens in single-threaded code.
        None => unsafe { std::env::remove_var("CODEX_HOME") },
    }
    match old_effort {
        // TODO: Audit that the environment access only happens in single-threaded code.
        Some(v) => unsafe { std::env::set_var("ANGEL_OPENAI_REASONING_EFFORT", v) },
        // TODO: Audit that the environment access only happens in single-threaded code.
        None => unsafe { std::env::remove_var("ANGEL_OPENAI_REASONING_EFFORT") },
    }
}

#[test]
fn model_catalog_strips_codex_spark_surfaces() {
    let models = model_catalog_from_str(
        r#"{
  "models": [
    {"slug":"gpt-5.6-luna","display_name":"Luna","visibility":"list","priority":1,
     "default_reasoning_level":"max","supported_reasoning_levels":[{"effort":"max","description":"max"}]},
    {"slug":"gpt-5.3-codex-spark","display_name":"Spark","visibility":"list","priority":0,
     "default_reasoning_level":"high","supported_reasoning_levels":[{"effort":"high","description":"h"}]}
  ]
}"#,
    );
    assert_eq!(models.len(), 1);
    assert_eq!(models[0].slug, "gpt-5.6-luna");
    assert!(!models.iter().any(|m| m.slug.contains("spark")));
}

/// An API-key Responses seat (Meta's Muse Spark) posts to its own endpoint with
/// only the bearer key, none of the ChatGPT account headers, asks for
/// reasoning summaries at the configured effort, and streams the summary to
/// the thinking panel. Meta's Chat Completions endpoint redacts reasoning.
#[test]
fn an_api_key_responses_seat_streams_reasoning_summaries() {
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let addr = listener.local_addr().unwrap();
    let server = std::thread::spawn(move || {
        let (mut sock, _) = listener.accept().unwrap();
        let request = read_http_request(&mut sock);
        sock.write_all(
            b"HTTP/1.1 200 OK\r\nContent-Type: text/event-stream\r\nConnection: close\r\n\r\n\
data: {\"type\":\"response.reasoning_summary_text.delta\",\"delta\":\"Checking the two buckets\"}\n\n\
data: {\"type\":\"response.output_text.delta\",\"delta\":\"ok\"}\n\n\
data: {\"type\":\"response.completed\",\"response\":{}}\n\n",
        )
        .unwrap();
        request
    });
    let club = CodexClub::api_key_seat(
        "muse-spark-1.3",
        "muse-spark-1.3",
        format!("http://{addr}/v1/responses"),
        "meta-fixture-key",
        Some("low".into()),
        vec!["minimal".into(), "low".into(), "medium".into()],
        crate::agent::club::RouteMetadata::default(),
    );
    let mut reasoning = String::new();
    let reply = club
        .chat_streaming(
            &[ChatMsg::user("fixture")],
            &[],
            &AtomicBool::new(false),
            &mut |delta| {
                if let StreamDelta::Reasoning(text) = delta {
                    reasoning.push_str(text);
                }
            },
        )
        .unwrap();
    assert!(matches!(reply, ClubReply::Text(ref text) if text == "ok"));
    assert_eq!(reasoning, "Checking the two buckets");
    let request = server.join().unwrap();
    let (head, body) = request.split_once("\r\n\r\n").unwrap();
    let head_lc = head.to_ascii_lowercase();
    assert!(head.starts_with("POST /v1/responses "), "{head}");
    assert!(
        head_lc.contains("authorization: bearer meta-fixture-key"),
        "{head}"
    );
    for chatgpt_only in ["chatgpt-account-id", "originator", "session_id"] {
        assert!(!head_lc.contains(chatgpt_only), "{chatgpt_only}: {head}");
    }
    let body: serde_json::Value = serde_json::from_str(body).unwrap();
    assert_eq!(body["model"], "muse-spark-1.3");
    assert_eq!(body["reasoning"]["effort"], "low");
    assert_eq!(body["reasoning"]["summary"], "auto");
}
