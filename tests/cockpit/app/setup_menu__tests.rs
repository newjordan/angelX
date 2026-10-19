use super::*;
use crate::agent::club::Bag;
use crate::app::App;
use crate::ui::viewer::Viewer;

fn press(menu: &mut Menu, code: KeyCode) -> Effect {
    menu.key(KeyEvent::new(code, KeyModifiers::NONE), Instant::now())
}

fn ctrl(menu: &mut Menu, c: char) -> Effect {
    menu.key(
        KeyEvent::new(KeyCode::Char(c), KeyModifiers::CONTROL),
        Instant::now(),
    )
}

fn title(menu: &Menu) -> String {
    menu.page(Instant::now()).title
}

fn labels(menu: &Menu) -> Vec<String> {
    menu.page(Instant::now())
        .items
        .into_iter()
        .map(|i| i.label)
        .collect()
}

/// Move the cursor onto the row with this label and press Enter.
fn choose(menu: &mut Menu, label: &str) -> Effect {
    let index = labels(menu)
        .iter()
        .position(|l| l == label)
        .unwrap_or_else(|| panic!("no row {label:?} in {:?}", labels(menu)));
    press(menu, KeyCode::Home);
    for _ in 0..index {
        press(menu, KeyCode::Down);
    }
    press(menu, KeyCode::Enter)
}

fn ollama() -> Server {
    Server {
        name: "Ollama".into(),
        url: "http://127.0.0.1:11434/v1".into(),
        models: vec!["qwen3-coder:30b".into()],
    }
}

fn answer(status: u16, models: &[&str], url: &str) -> Outcome {
    Outcome::Answer {
        status,
        models: models.iter().map(|m| m.to_string()).collect(),
        url: url.into(),
    }
}

fn saved(effect: Effect) -> Saving {
    match effect {
        Effect::Save(saving) => saving,
        other => panic!("expected a save, got {other:?}"),
    }
}

fn pairs(saving: &Saving) -> Vec<(&str, &str)> {
    saving.pairs.iter().map(|(k, v)| (*k, v.as_str())).collect()
}

#[test]
fn home_lists_what_this_computer_has_first_and_a_number_picks_it() {
    let found = Found {
        servers: vec![ollama()],
        keys: vec![(0, "DEEPSEEK_API_KEY")],
        chatgpt: true,
        grok: false,
    };
    let mut menu = Menu::new(found, Tools::default(), Instant::now());
    let rows = labels(&menu);
    assert_eq!(rows[0], "Use Ollama on this computer");
    assert_eq!(rows[1], "Use my ChatGPT plan");
    assert_eq!(rows[2], "Use my DeepSeek key");
    // Signed in already: no second ChatGPT sign-in row; Grok still offered.
    assert!(!rows.contains(&"Sign in with ChatGPT".to_string()));
    assert!(rows.contains(&"Sign in with Grok".to_string()));
    assert_eq!(rows.last().unwrap(), "Not now");
    assert!(menu.page(Instant::now()).items[0].ready);

    let saving = saved(press(&mut menu, KeyCode::Char('1')));
    assert_eq!(
        pairs(&saving),
        vec![
            ("ANGEL_LOCAL_URL", "http://127.0.0.1:11434/v1"),
            ("ANGEL_LOCAL_MODEL", "qwen3-coder:30b"),
            ("ANGEL_DRIVER", "local"),
        ]
    );
    assert!(saving.tip.unwrap().contains("OLLAMA_CONTEXT_LENGTH"));
}

#[test]
fn a_signed_in_plan_saves_its_driver() {
    let found = Found {
        chatgpt: true,
        ..Found::default()
    };
    let mut menu = Menu::new(found, Tools::default(), Instant::now());
    let saving = saved(choose(&mut menu, "Use my ChatGPT plan"));
    assert_eq!(pairs(&saving), vec![("ANGEL_DRIVER", "openai")]);
}

#[test]
fn a_pasted_key_is_checked_then_saved() {
    let mut menu = Menu::new(Found::default(), Tools::default(), Instant::now());
    assert_eq!(choose(&mut menu, "Paste an API key"), Effect::Stay);
    assert_eq!(title(&menu), "Which company is your key from?");
    assert_eq!(choose(&mut menu, "DeepSeek"), Effect::Stay);
    assert_eq!(title(&menu), "Paste your DeepSeek key");
    assert_eq!(ctrl(&mut menu, 'o'), Effect::Open(PROVIDERS[0].site.into()));

    // Enter on nothing asks for the key; a paste keeps no whitespace.
    assert_eq!(press(&mut menu, KeyCode::Enter), Effect::Stay);
    assert_eq!(
        menu.page(Instant::now()).note.as_deref(),
        Some("Paste your key first.")
    );
    menu.paste(" sk-abc\n123 ");
    let field = menu.page(Instant::now()).field.unwrap();
    assert_eq!(field.shown, "•••••c123");

    let Effect::Run(serial) = press(&mut menu, KeyCode::Enter) else {
        panic!("the key check should run");
    };
    match menu.take_job() {
        Some(Job::CheckKey { provider: 0, key }) => assert_eq!(key, "sk-abc123"),
        _ => panic!("expected a key check"),
    }
    let page = menu.page(Instant::now());
    assert!(page.busy);
    assert!(
        matches!(page.pose, Pose::Pull | Pose::Strain),
        "{:?}",
        page.pose
    );
    // Keys wait while the check runs.
    assert_eq!(press(&mut menu, KeyCode::Char('x')), Effect::Stay);

    let saving = saved(menu.finish(serial, answer(200, &[], PROVIDERS[0].url), Instant::now()));
    assert_eq!(
        pairs(&saving),
        vec![
            ("DEEPSEEK_API_KEY", "sk-abc123"),
            ("ANGEL_DRIVER", "deepseek")
        ]
    );
    assert_eq!(saving.name, "DeepSeek");
}

#[test]
fn a_rejected_key_stays_on_the_paste_screen_with_a_reason() {
    let mut menu = Menu::new(Found::default(), Tools::default(), Instant::now());
    choose(&mut menu, "Paste an API key");
    choose(&mut menu, "DeepSeek");
    menu.paste("sk-wrong");
    let Effect::Run(serial) = press(&mut menu, KeyCode::Enter) else {
        panic!("the key check should run");
    };
    assert_eq!(
        menu.finish(serial, answer(401, &[], PROVIDERS[0].url), Instant::now()),
        Effect::Stay
    );
    let page = menu.page(Instant::now());
    assert_eq!(page.title, "Paste your DeepSeek key");
    assert_eq!(page.pose, Pose::Stumble, "a problem trips him up");
    assert_eq!(
        menu.page(Instant::now() + Duration::from_secs(2)).pose,
        Pose::LiftKey
    );
    assert!(page.note.unwrap().contains("said no to that key"));
    assert_eq!(page.field.unwrap().shown, "", "a wrong key is cleared");
}

#[test]
fn an_unreachable_provider_can_still_be_saved() {
    let mut menu = Menu::new(Found::default(), Tools::default(), Instant::now());
    choose(&mut menu, "Paste an API key");
    choose(&mut menu, "Kimi Code plan");
    menu.paste("sk-kimi");
    let Effect::Run(serial) = press(&mut menu, KeyCode::Enter) else {
        panic!("the key check should run");
    };
    menu.finish(serial, answer(0, &[], ""), Instant::now());
    assert_eq!(title(&menu), "We couldn't check that key");
    let saving = saved(choose(&mut menu, "Save it anyway"));
    assert_eq!(
        pairs(&saving),
        vec![
            ("KIMI_API_KEY", "sk-kimi"),
            ("KIMI_API_URL", "https://api.kimi.com/coding/v1"),
            ("KIMI_MODEL", "k3"),
            ("ANGEL_DRIVER", "kimi"),
        ]
    );
}

#[test]
fn a_provider_without_a_default_model_asks_for_one_and_typing_filters() {
    let mut menu = Menu::new(Found::default(), Tools::default(), Instant::now());
    choose(&mut menu, "Paste an API key");
    choose(&mut menu, "OpenRouter");
    menu.paste("sk-or-1");
    let Effect::Run(serial) = press(&mut menu, KeyCode::Enter) else {
        panic!("the key check should run");
    };
    menu.finish(
        serial,
        answer(200, &["a/one:free", "b/two", "c/three"], ""),
        Instant::now(),
    );
    assert_eq!(title(&menu), "Pick a model");
    assert_eq!(labels(&menu), ["a/one:free", "b/two", "c/three"]);
    // Digits type into the search box here; they do not pick.
    press(&mut menu, KeyCode::Char('t'));
    press(&mut menu, KeyCode::Char('w'));
    assert_eq!(labels(&menu), ["b/two"]);
    let saving = saved(press(&mut menu, KeyCode::Enter));
    assert_eq!(
        pairs(&saving),
        vec![
            ("OPENROUTER_API_KEY", "sk-or-1"),
            ("ANGEL_DRIVER", "openrouter"),
            ("ANGEL_OPENROUTER_MODEL", "b/two"),
        ]
    );
}

#[test]
fn a_typed_model_name_is_taken_when_nothing_matches() {
    let mut menu = Menu::new(Found::default(), Tools::default(), Instant::now());
    choose(&mut menu, "Paste an API key");
    choose(&mut menu, "Cerebras");
    menu.paste("csk-1");
    let Effect::Run(serial) = press(&mut menu, KeyCode::Enter) else {
        panic!("the key check should run");
    };
    menu.finish(serial, answer(200, &[], ""), Instant::now());
    menu.paste("my-model");
    let saving = saved(press(&mut menu, KeyCode::Enter));
    assert!(pairs(&saving).contains(&("ANGEL_CEREBRAS_MODEL", "my-model")));
}

#[test]
fn esc_stops_a_check_and_its_late_answer_is_dropped() {
    let mut menu = Menu::new(Found::default(), Tools::default(), Instant::now());
    choose(&mut menu, "Paste an API key");
    choose(&mut menu, "DeepSeek");
    menu.paste("sk-1");
    let Effect::Run(serial) = press(&mut menu, KeyCode::Enter) else {
        panic!("the key check should run");
    };
    assert_eq!(press(&mut menu, KeyCode::Esc), Effect::Stay);
    assert!(!menu.page(Instant::now()).busy);
    assert_eq!(
        menu.finish(serial, answer(200, &[], ""), Instant::now()),
        Effect::Stay
    );
    assert_eq!(title(&menu), "Paste your DeepSeek key");
}

#[test]
fn esc_walks_back_and_then_closes() {
    let mut menu = Menu::new(Found::default(), Tools::default(), Instant::now());
    choose(&mut menu, "Paste an API key");
    choose(&mut menu, "OpenAI");
    assert_eq!(press(&mut menu, KeyCode::Esc), Effect::Stay);
    assert_eq!(title(&menu), "Which company is your key from?");
    // The cursor comes back to where it was.
    assert_eq!(menu.page(Instant::now()).cursor, 1);
    assert_eq!(press(&mut menu, KeyCode::Esc), Effect::Stay);
    assert_eq!(title(&menu), "Connect angelX to an AI model");
    assert_eq!(press(&mut menu, KeyCode::Esc), Effect::Close);
}

#[test]
fn a_server_that_wants_a_key_asks_for_it() {
    let mut menu = Menu::new(Found::default(), Tools::default(), Instant::now());
    choose(&mut menu, "Use a free model on this computer");
    choose(&mut menu, "It's on another computer");
    menu.paste("192.168.1.5:8000/");
    let Effect::Run(serial) = press(&mut menu, KeyCode::Enter) else {
        panic!("the address check should run");
    };
    match menu.take_job() {
        Some(Job::ServerModels { url, key: None }) => assert_eq!(url, "http://192.168.1.5:8000"),
        _ => panic!("expected a server check"),
    }
    let url = "http://192.168.1.5:8000/v1";
    menu.finish(serial, answer(401, &[], url), Instant::now());
    assert_eq!(title(&menu), "This server wants a key");
    menu.paste("secret");
    let Effect::Run(serial) = press(&mut menu, KeyCode::Enter) else {
        panic!("the key should be tried");
    };
    menu.finish(serial, answer(200, &["m1", "m2"], url), Instant::now());
    assert_eq!(title(&menu), "Pick a model");
    press(&mut menu, KeyCode::Down);
    let saving = saved(press(&mut menu, KeyCode::Enter));
    assert_eq!(
        pairs(&saving),
        vec![
            ("ANGEL_LOCAL_URL", url),
            ("ANGEL_LOCAL_MODEL", "m2"),
            ("ANGEL_BRAIN_KEY", "secret"),
            ("ANGEL_DRIVER", "local"),
        ]
    );
    // Back from the model list is the address, not the key screen.
    let mut menu2 = Menu::new(Found::default(), Tools::default(), Instant::now());
    choose(&mut menu2, "Use a free model on this computer");
    choose(&mut menu2, "It's on another computer");
    menu2.paste("box:1");
    let Effect::Run(s) = press(&mut menu2, KeyCode::Enter) else {
        panic!()
    };
    menu2.finish(s, answer(401, &[], "http://box:1/v1"), Instant::now());
    menu2.paste("k");
    let Effect::Run(s) = press(&mut menu2, KeyCode::Enter) else {
        panic!()
    };
    menu2.finish(
        s,
        answer(200, &["a", "b"], "http://box:1/v1"),
        Instant::now(),
    );
    press(&mut menu2, KeyCode::Esc);
    assert_eq!(title(&menu2), "Where is the model server?");
}

#[test]
fn a_scan_keeps_the_cursor_on_the_same_choice() {
    let mut menu = Menu::new(Found::default(), Tools::default(), Instant::now());
    let _ = menu.scan();
    assert!(
        menu.page(Instant::now())
            .lines
            .iter()
            .any(|l| l.contains("Looking around"))
    );
    press(&mut menu, KeyCode::End);
    let before = labels(&menu).len() - 1;
    assert_eq!(labels(&menu)[before], "Not now");
    menu.finish(0, Outcome::Scanned(vec![ollama()]), Instant::now());
    let page = menu.page(Instant::now());
    assert_eq!(page.items[page.cursor].label, "Not now");
    assert_eq!(page.items[0].label, "Use Ollama on this computer");
    assert!(!page.lines.iter().any(|l| l.contains("Looking around")));
}

#[test]
fn signing_in_runs_the_plan_cli_and_saves_when_it_succeeds() {
    let tools = Tools {
        codex: Some(PathBuf::from("/usr/bin/codex")),
        ..Tools::default()
    };
    let mut menu = Menu::new(Found::default(), tools, Instant::now());
    choose(&mut menu, "Sign in with ChatGPT");
    assert_eq!(title(&menu), "Sign in with ChatGPT");
    let Effect::Run(serial) = choose(&mut menu, "Sign in") else {
        panic!("sign-in should run");
    };
    let Some(Job::Login { plan, tool, login }) = menu.take_job() else {
        panic!("expected a login");
    };
    assert_eq!(plan, Plan::ChatGpt);
    assert_eq!(tool, PathBuf::from("/usr/bin/codex"));
    // The link the CLI prints is shown and Ctrl+O opens it.
    login.log.lock().unwrap().push_str(
        "Starting local login server. If your browser did not open, navigate to this URL to authenticate:\n\nhttps://auth.openai.com/oauth/authorize?x=1.\n",
    );
    let page = menu.page(Instant::now());
    assert!(page.link);
    assert!(
        page.lines
            .contains(&"https://auth.openai.com/oauth/authorize?x=1".to_string())
    );
    assert_eq!(
        ctrl(&mut menu, 'o'),
        Effect::Open("https://auth.openai.com/oauth/authorize?x=1".into())
    );
    let done = Outcome::Finished {
        ok: true,
        log: String::new(),
        tool: None,
    };
    let saving = saved(menu.finish(serial, done, Instant::now()));
    assert_eq!(pairs(&saving), vec![("ANGEL_DRIVER", "openai")]);
}

#[test]
fn a_failed_sign_in_says_why() {
    let tools = Tools {
        grok: Some(PathBuf::from("/usr/bin/grok")),
        ..Tools::default()
    };
    let mut menu = Menu::new(Found::default(), tools, Instant::now());
    choose(&mut menu, "Sign in with Grok");
    let Effect::Run(serial) = choose(&mut menu, "Sign in") else {
        panic!("sign-in should run");
    };
    let failed = Outcome::Finished {
        ok: false,
        log: "opening browser\nError: login timed out\n".into(),
        tool: None,
    };
    assert_eq!(menu.finish(serial, failed, Instant::now()), Effect::Stay);
    let page = menu.page(Instant::now());
    assert_eq!(page.title, "Sign in with Grok");
    assert_eq!(
        page.note.as_deref(),
        Some("Sign-in didn't finish: Error: login timed out")
    );
}

#[test]
fn a_missing_plan_app_is_installed_first() {
    let tools = Tools {
        npm: true,
        ..Tools::default()
    };
    let mut menu = Menu::new(Found::default(), tools, Instant::now());
    choose(&mut menu, "Sign in with ChatGPT");
    assert_eq!(title(&menu), "ChatGPT needs one small app first");
    let Effect::Run(serial) = choose(&mut menu, "Install it for me") else {
        panic!("install should run");
    };
    let installed = Outcome::Finished {
        ok: true,
        log: String::new(),
        tool: Some(PathBuf::from("/home/u/.npm-global/bin/codex")),
    };
    menu.finish(serial, installed, Instant::now());
    assert_eq!(title(&menu), "Sign in with ChatGPT");
    assert_eq!(labels(&menu)[0], "Sign in");
    assert_eq!(menu.page(Instant::now()).pose, Pose::ThumbsUp, "good news");
    assert_eq!(
        menu.page(Instant::now()).note.as_deref(),
        Some("Installed. Now choose Sign in.")
    );
}

#[test]
fn without_node_the_plan_screen_points_at_nodejs() {
    let mut menu = Menu::new(Found::default(), Tools::default(), Instant::now());
    choose(&mut menu, "Sign in with Grok");
    assert_eq!(
        choose(&mut menu, "Open nodejs.org"),
        Effect::Open("https://nodejs.org".into())
    );
}

#[test]
fn done_gives_a_thumbs_up_and_enter_closes() {
    let start = Instant::now();
    let mut menu = Menu::new(Found::default(), Tools::default(), start);
    menu.show_done("DeepSeek".into(), None, start);
    assert_eq!(menu.page(start).pose, Pose::ThumbsUp);
    assert!(menu.page(start).lines[0].contains("DeepSeek"));
    assert_eq!(press(&mut menu, KeyCode::Enter), Effect::Close);
}

#[test]
fn the_home_guide_waves_hello_then_points_at_the_choices() {
    let start = Instant::now();
    let menu = Menu::new(Found::default(), Tools::default(), start);
    assert_eq!(menu.page(start).pose, Pose::Wave);
    assert_eq!(menu.page(start + Duration::from_secs(3)).pose, Pose::Point);
}

#[test]
fn clicks_pick_rows_and_the_back_chip_goes_back() {
    let mut menu = Menu::new(Found::default(), Tools::default(), Instant::now());
    let index = labels(&menu)
        .iter()
        .position(|l| l == "Paste an API key")
        .unwrap();
    assert_eq!(menu.click(Hit::Item(index), Instant::now()), Effect::Stay);
    assert_eq!(title(&menu), "Which company is your key from?");
    menu.click(Hit::Back, Instant::now());
    assert_eq!(title(&menu), "Connect angelX to an AI model");
    assert_eq!(menu.click(Hit::Back, Instant::now()), Effect::Close);
}

#[test]
fn ctrl_c_closes_from_anywhere() {
    let mut menu = Menu::new(Found::default(), Tools::default(), Instant::now());
    choose(&mut menu, "Paste an API key");
    assert_eq!(ctrl(&mut menu, 'c'), Effect::Close);
}

#[test]
fn model_lists_keep_chat_models_and_free_ones_lead() {
    let body = r#"{"data":[{"id":"gpt-z"},{"id":"text-embedding-3"},{"id":"gpt-a"},{"id":"whisper-1"},{"id":"gpt-image-1"}]}"#;
    assert_eq!(chat_models(body), ["gpt-a", "gpt-z"]);
    let body = r#"{"data":[{"id":"z/paid"},{"id":"b/x:free"},{"id":"a/y:free"}]}"#;
    assert_eq!(openrouter_models(body), ["a/y:free", "b/x:free", "z/paid"]);
    assert!(model_ids("<html>not json</html>").is_empty());
    assert_eq!(model_ids(r#"{"data":[{"id":"m"}]}"#), ["m"]);
}

#[test]
fn small_words() {
    assert_eq!(masked(""), "");
    assert_eq!(masked("abc"), "•••");
    assert_eq!(masked("sk-123456"), "•••••3456");
    assert_eq!(key_problem("sk-ok_1.2"), None);
    assert!(key_problem("sk ok").is_some());
    assert_eq!(
        server_url(" 10.0.0.2:8080/ ").as_deref(),
        Some("http://10.0.0.2:8080")
    );
    assert_eq!(server_url("https://x/v1").as_deref(), Some("https://x/v1"));
    assert_eq!(server_url("  "), None);
    assert_eq!(server_name("library", 9999), "Ollama");
    assert_eq!(server_name("", 1234), "LM Studio");
    assert_eq!(server_name("", 4242), "a model server");
}

#[test]
fn settings_replace_only_their_names_and_source_back_in_bash() {
    let dir = std::env::temp_dir().join(format!("angel-setup-test-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    let path = dir.join("angel.env");
    // A new file gets its header.
    write_settings(&path, &[("ANGEL_SETUP_SKIPPED", "1".into())]).unwrap();
    assert!(
        std::fs::read_to_string(&path)
            .unwrap()
            .starts_with("# angelX settings.")
    );
    std::fs::write(
        &path,
        "# mine\nexport DEEPSEEK_API_KEY='old'\nOTHER=1\nANGEL_DRIVER_EXTRA=keep\nANGEL_DRIVER=local\n",
    )
    .unwrap();
    write_settings(
        &path,
        &[
            ("DEEPSEEK_API_KEY", "it's $HOME".into()),
            ("ANGEL_DRIVER", "deepseek".into()),
        ],
    )
    .unwrap();
    let text = std::fs::read_to_string(&path).unwrap();
    assert_eq!(
        text,
        "# mine\nOTHER=1\nANGEL_DRIVER_EXTRA=keep\nDEEPSEEK_API_KEY='it'\\''s $HOME'\nANGEL_DRIVER='deepseek'\n"
    );
    use std::os::unix::fs::PermissionsExt;
    assert_eq!(
        std::fs::metadata(&path).unwrap().permissions().mode() & 0o777,
        0o600
    );
    // bin/angelX sources the file with set -a; the value must come back exact.
    let out = std::process::Command::new("bash")
        .arg("-c")
        .arg("set -a; . \"$1\"; set +a; printf '%s|%s' \"$DEEPSEEK_API_KEY\" \"$ANGEL_DRIVER\"")
        .arg("bash")
        .arg(&path)
        .output()
        .unwrap();
    assert_eq!(String::from_utf8_lossy(&out.stdout), "it's $HOME|deepseek");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn slash_setup_opens_the_menu_and_esc_closes_it() {
    assert!(crate::ui::input::slash_command_matches("/setu").contains(&"setup"));
    let mut app = App::new(Bag::practice_for_test(), Viewer::new());
    app.open_setup();
    assert!(app.setup.open());
    assert!(app.setup.wants_tick());
    let menu = app.setup.menu.as_ref().unwrap();
    assert_eq!(menu.current, None, "practice is no model");
    // Keys and pastes go to the menu, not the composer.
    app.on_key(KeyEvent::new(KeyCode::Char('3'), KeyModifiers::NONE));
    assert!(app.input.is_empty());
    app.on_key(KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE));
    app.on_key(KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE));
    assert!(!app.setup.open());
}

#[test]
fn a_typed_slash_setup_opens_the_menu() {
    let mut app = App::new(Bag::practice_for_test(), Viewer::new());
    app.input = "/setup".into();
    app.submit();
    assert!(app.setup.open());
    assert!(app.input.is_empty());
}

#[test]
fn with_a_model_in_hand_the_last_row_closes_instead_of_putting_off() {
    let mut menu = Menu::new(Found::default(), Tools::default(), Instant::now());
    menu.current = Some("local ▸ qwen3-coder:30b".into());
    assert_eq!(labels(&menu).last().unwrap(), "Close");
    assert!(menu.page(Instant::now()).lines[0].contains("local ▸ qwen3-coder:30b"));
    assert_eq!(choose(&mut menu, "Close"), Effect::Close);
}

#[test]
#[cfg(feature = "glass")]
fn setup_scroll_mouse_maps_displayed_rows_and_ignores_unreleased_paper() {
    use angel_glass::scroll::{Page as ScrollPage, Pose as ScrollPose, surface};
    use ratatui::buffer::Buffer;
    let mut app = App::new(Bag::practice_for_test(), Viewer::new());
    app.setup.menu = Some(Menu::new(Found::default(), Tools::default(), Instant::now()));
    let area = Rect::new(10, 5, 80, 30);
    let target = Rect::new(15, 15, 1, 1);
    let pose = ScrollPose { open: 1.0, sway: 0.0 };
    app.setup.hits = vec![(target, Hit::Back)];
    app.setup.scroll_page = ScrollPage::new(area, Buffer::empty(Rect::new(0, 0, 80, 30)), pose);
    let mut event = MouseEvent {
        kind: MouseEventKind::Down(MouseButton::Left), column: target.x, row: target.y,
        modifiers: KeyModifiers::NONE,
    };
    // A flat-coordinate click must not activate a still-hidden action.
    app.setup.scroll_page.as_mut().unwrap().pose.open = 0.5;
    app.setup_mouse(event);
    assert!(app.setup.open(), "unreleased paper must reject menu clicks");
    app.setup.scroll_page.as_mut().unwrap().pose = pose;
    app.setup.scroll_page.as_mut().unwrap().interactive = false;
    app.setup_mouse(event);
    assert!(app.setup.open(), "suppressed paper must reject menu clicks");
    app.setup.scroll_page.as_mut().unwrap().interactive = true;
    let s = (f32::from(target.y - area.y) + 0.5) / f32::from(area.height);
    event.row = area.y + (surface(pose, s)[0] * f32::from(area.height)) as u16;
    assert_ne!(event.row, target.y, "exercise the real projection, not identity");
    app.setup_mouse(event);
    assert!(!app.setup.open(), "the projected Back target must close the home page");
}
