use super::*;
use crate::agent::club::ChatRole;

#[test]
fn parses_local_commands() {
    assert!(matches!(
        parse("/show /tmp/a.png").unwrap(),
        ParsedInput::Show(path) if path == "/tmp/a.png"
    ));
    assert!(matches!(parse("/hide").unwrap(), ParsedInput::Hide));
    assert!(matches!(parse("/media").unwrap(), ParsedInput::MediaPage));
    assert!(matches!(
        parse("/observatory").unwrap(),
        ParsedInput::Observatory(ObservatoryCommand::Browse)
    ));
    assert!(matches!(
        parse("/observatory campaign visual-verifier").unwrap(),
        ParsedInput::Observatory(ObservatoryCommand::Campaign(id)) if id == "visual-verifier"
    ));
    assert!(matches!(
        parse("/observatory open verifier-report").unwrap(),
        ParsedInput::Observatory(ObservatoryCommand::OpenReport(id)) if id == "verifier-report"
    ));
    assert!(parse("/observatory campaign").is_err());
    assert!(matches!(parse("/rl").unwrap(), ParsedInput::Rl(None)));
    assert!(matches!(
        parse("/reinforce").unwrap(),
        ParsedInput::Rl(None)
    ));
    assert!(matches!(
        parse("/handoff-rl").unwrap(),
        ParsedInput::HandoffRl(None)
    ));
    assert!(matches!(
        parse("/hrl start hypothesis").unwrap(),
        ParsedInput::HandoffRl(Some(arg)) if arg == "start hypothesis"
    ));
    assert!(matches!(
        parse("/rl gate").unwrap(),
        ParsedInput::Rl(Some(arg)) if arg == "gate"
    ));
    assert!(matches!(
        parse("/rl sweep 3 200").unwrap(),
        ParsedInput::Rl(Some(arg)) if arg == "sweep 3 200"
    ));
    assert!(!parse("/rl").unwrap().needs_idle(), "local command");
    assert!(matches!(parse("/sessions").unwrap(), ParsedInput::Sessions));
    assert!(matches!(parse("/open").unwrap(), ParsedInput::Open(1)));
    assert!(matches!(parse("/open 0").unwrap(), ParsedInput::Open(0)));
    assert!(matches!(
        parse("/open shell").unwrap(),
        ParsedInput::ModuleOpen(id) if id == "shell"
    ));
    assert!(matches!(
        parse("/open http://192.168.1.42:8000/frogger.html").unwrap(),
        ParsedInput::OpenTarget(target) if target == "http://192.168.1.42:8000/frogger.html"
    ));
    assert!(matches!(
        parse("/open 192.168.1.42:8000/frogger.html").unwrap(),
        ParsedInput::OpenTarget(target) if target == "http://192.168.1.42:8000/frogger.html"
    ));
    assert!(matches!(
        parse("/open localhost:8000/frogger.html").unwrap(),
        ParsedInput::OpenTarget(target) if target == "http://localhost:8000/frogger.html"
    ));
    assert!(matches!(
        parse("/open example.com").unwrap(),
        ParsedInput::OpenTarget(target) if target == "http://example.com"
    ));
    assert!(matches!(
        parse("/close artifacts").unwrap(),
        ParsedInput::ModuleClose(id) if id == "artifacts"
    ));
    assert!(matches!(parse("/modules").unwrap(), ParsedInput::Modules));
    assert!(matches!(
        parse("/macro open").unwrap(),
        ParsedInput::Message(_)
    ));
    assert!(matches!(
        parse("/layout save night").unwrap(),
        ParsedInput::Layout { action, name: Some(n) } if action == "save" && n == "night"
    ));
    assert!(matches!(
        parse("/resume abc123").unwrap(),
        ParsedInput::Resume(Some(id)) if id == "abc123"
    ));
    assert!(matches!(
        parse("/resume").unwrap(),
        ParsedInput::Resume(None)
    ));
    assert!(matches!(
        parse("/resumeabc").unwrap(),
        ParsedInput::Message(_)
    ));
}

#[test]
fn slash_catalog_is_unique_and_help_documented() {
    // The completion catalog is the command namespace: a duplicate name
    // here is a field-day collision (see the /recall history), and a name
    // absent from /help is a command the operator cannot discover. Both
    // fail here, at compile-test time, instead of in the field.
    let mut seen: std::collections::HashMap<&str, u32> = std::collections::HashMap::new();
    for cmd in CODEX_CMDS.iter().chain(DEDICATED_CMDS.iter()) {
        *seen.entry(cmd).or_default() += 1;
    }
    let dupes: Vec<_> = seen.iter().filter(|(_, n)| **n > 1).collect();
    assert!(dupes.is_empty(), "duplicate slash commands: {dupes:?}");
    let help = crate::app::local_command::help_text(Some("all"));
    // Names documented under a different spelling (aliases, module toggles
    // opened via /open <module>, or the help wildcard itself).
    const SPELLINGS: &[(&str, &str)] = &[
        ("thinking", "/think"),
        ("exit", "exit (/quit)"),
        ("selftest", "/approvals"),
        ("effort", "/think"),
        ("recall", "/memories"),
        ("graph", "/open <module>"),
        ("reinforce", "/open <module>"),
        ("?", "/help"),
    ];
    for cmd in seen.keys() {
        let documented = help.contains(&format!("/{cmd}"))
            || SPELLINGS
                .iter()
                .any(|(alias, spelling)| alias == cmd && help.contains(spelling));
        assert!(
            documented,
            "completable command /{cmd} is missing from /help"
        );
    }
}

#[test]
fn parses_practice_command() {
    assert!(matches!(parse("/practice").unwrap(), ParsedInput::Practice));
    assert!(matches!(
        parse("  /practice  ").unwrap(),
        ParsedInput::Practice
    ));
    // `/recall` keeps its long-standing memory-recall Cmd meaning.
    assert!(matches!(
        parse("/recall").unwrap(),
        ParsedInput::Cmd { name, arg: None } if name == "recall"
    ));
}

#[test]
fn parses_recall_command() {
    assert!(matches!(
        parse("/recall deadlock in run_turn").unwrap(),
        ParsedInput::Cmd { name, arg: Some(q) } if name == "recall" && q == "deadlock in run_turn"
    ));
    // Bare `/recall` → no arg (the handler prints usage).
    assert!(matches!(
        parse("/recall").unwrap(),
        ParsedInput::Cmd { name, arg: None } if name == "recall"
    ));
}

#[test]
fn exit_and_quit_close_the_app() {
    for word in ["exit", "quit", "/exit", "/quit", "EXIT", "  Quit  "] {
        assert!(
            matches!(parse(word).unwrap(), ParsedInput::Exit),
            "{word:?}"
        );
    }
    for word in ["exit!", "quit!", "/exit!", "/quit!", "EXIT!", "  Quit!  "] {
        assert!(
            matches!(parse(word).unwrap(), ParsedInput::ForceExit),
            "{word:?}"
        );
    }
    // Anything beyond the bare word is an ordinary message, not a close.
    assert!(matches!(
        parse("exit now").unwrap(),
        ParsedInput::Message(_)
    ));
    assert!(matches!(parse("exits").unwrap(), ParsedInput::Message(_)));
}

#[test]
fn parses_codex_ported_commands() {
    assert!(matches!(parse("/help").unwrap(), ParsedInput::Help(None)));
    assert!(matches!(parse("/?").unwrap(), ParsedInput::Help(None)));
    assert!(matches!(parse("/status").unwrap(), ParsedInput::Status));
    assert!(matches!(parse("/save").unwrap(), ParsedInput::Save));
    assert!(matches!(parse("/new").unwrap(), ParsedInput::NewChat));
    assert!(matches!(parse("/clear").unwrap(), ParsedInput::NewChat));
    assert!(matches!(parse("/diff").unwrap(), ParsedInput::Diff(None)));
    assert!(matches!(
        parse("/diff staged --stat").unwrap(),
        ParsedInput::Diff(Some(arg)) if arg == "staged --stat"
    ));
    assert!(matches!(parse("/review").unwrap(), ParsedInput::Review));
    assert!(matches!(parse("/retry").unwrap(), ParsedInput::Retry));
    assert!(matches!(parse("/undo").unwrap(), ParsedInput::Undo));
    assert!(matches!(parse("/redo").unwrap(), ParsedInput::Redo));
    assert!(matches!(
        parse("/copy 3").unwrap(),
        ParsedInput::Cmd {
            name,
            arg: Some(value)
        } if name == "copy" && value == "3"
    ));
    assert!(matches!(
        parse("/redraw").unwrap(),
        ParsedInput::Cmd { name, arg: None } if name == "redraw"
    ));
    assert!(matches!(
        parse("/model").unwrap(),
        ParsedInput::ModelInfo(None)
    ));
    assert!(matches!(
        parse("/model auto").unwrap(),
        ParsedInput::ModelInfo(Some(value)) if value == "auto"
    ));
    assert!(matches!(
        parse("/think").unwrap(),
        ParsedInput::Thinking(None)
    ));
    for raw in ["/think high", "/thinking high", "/effort high"] {
        assert!(matches!(
            parse(raw).unwrap(),
            ParsedInput::Thinking(Some(value)) if value == "high"
        ));
    }
    assert!(matches!(parse("/rate").unwrap(), ParsedInput::Rate(None)));
    assert!(matches!(
        parse("/rate useful").unwrap(),
        ParsedInput::Rate(Some(value)) if value == "useful"
    ));
    assert!(matches!(parse("/init").unwrap(), ParsedInput::Init));
    assert!(matches!(parse("/goal").unwrap(), ParsedInput::Goal(None)));
    assert!(matches!(
        parse("/goal ship the cockpit").unwrap(),
        ParsedInput::Goal(Some(g)) if g == "ship the cockpit"
    ));
    assert!(matches!(
        parse("/campaign").unwrap(),
        ParsedInput::Campaign(None)
    ));
    assert!(matches!(
        parse("/campaign criterion add cargo test passes").unwrap(),
        ParsedInput::Campaign(Some(arg)) if arg == "criterion add cargo test passes"
    ));
    assert!(matches!(
        parse("/campaigns are useful").unwrap(),
        ParsedInput::Message(_)
    ));
    assert!(matches!(parse("/loop").unwrap(), ParsedInput::Loop(None)));
    assert!(matches!(
        parse("/loop start fix the flaky test").unwrap(),
        ParsedInput::Loop(Some(a)) if a == "start fix the flaky test"
    ));
    assert!(matches!(
        parse("/self").unwrap(),
        ParsedInput::SelfLoop(None)
    ));
    assert!(matches!(
        parse("/self fix a clippy finding").unwrap(),
        ParsedInput::SelfLoop(Some(a)) if a == "fix a clippy finding"
    ));
    // `/selfie` is not `/self` — unknown slash stays an ordinary message.
    assert!(matches!(parse("/selfie").unwrap(), ParsedInput::Message(_)));
    assert!(matches!(
        parse("/moa compare options").unwrap(),
        ParsedInput::Moa(_)
    ));
    assert!(matches!(parse("/moa").unwrap(), ParsedInput::MoaDeck(None)));
    assert!(matches!(
        parse("/moa cards").unwrap(),
        ParsedInput::MoaDeck(Some(a)) if a == "cards"
    ));
    assert!(matches!(
        parse("/moa gpu comp").unwrap(),
        ParsedInput::MoaDeck(Some(a)) if a == "gpu comp"
    ));
    assert!(matches!(
        parse("/moa war").unwrap(),
        ParsedInput::MoaDeck(Some(a)) if a == "war"
    ));
    assert!(matches!(
        parse("/moa grok war").unwrap(),
        ParsedInput::MoaDeck(Some(a)) if a == "grok war"
    ));
    assert!(matches!(
        parse("/moa grokwar").unwrap(),
        ParsedInput::MoaDeck(Some(a)) if a == "grokwar"
    ));
    assert!(matches!(
        parse("/moa math").unwrap(),
        ParsedInput::MoaDeck(Some(a)) if a == "math"
    ));
    assert!(matches!(
        parse("/moa math god").unwrap(),
        ParsedInput::MoaDeck(Some(a)) if a == "math god"
    ));
    assert!(matches!(
        parse("/moa soundness").unwrap(),
        ParsedInput::MoaDeck(Some(a)) if a == "soundness"
    ));
    assert!(matches!(
        parse("/moa tag-team").unwrap(),
        ParsedInput::MoaDeck(Some(a)) if a == "tag-team"
    ));
    assert!(matches!(
        parse("/moa auto").unwrap(),
        ParsedInput::MoaDeck(Some(a)) if a == "auto"
    ));
    assert!(matches!(
        parse("activate the moa").unwrap(),
        ParsedInput::MoaDeck(None)
    ));
    assert!(matches!(
        parse("activate gpu comp moa").unwrap(),
        ParsedInput::MoaDeck(Some(a)) if a == "gpu comp"
    ));
    // Unknown slash + plain text stay ordinary messages.
    assert!(matches!(parse("/nope").unwrap(), ParsedInput::Message(_)));
    assert!(matches!(
        parse("status of the build").unwrap(),
        ParsedInput::Message(_)
    ));
}

#[test]
fn parses_librarium_commands_and_aliases() {
    assert!(matches!(parse("/ask").unwrap(), ParsedInput::Ask));
    assert!(matches!(parse("  /ask  ").unwrap(), ParsedInput::Ask));
    for raw in ["/learn", "/tutor", "/library", "  /learn   "] {
        assert!(
            matches!(parse(raw).unwrap(), ParsedInput::Learn(None)),
            "{raw:?}"
        );
    }
    for raw in [
        "/learn linear algebra",
        "/tutor linear algebra",
        "/library linear algebra",
        "  /learn   linear algebra  ",
    ] {
        assert!(
            matches!(parse(raw).unwrap(), ParsedInput::Learn(Some(topic)) if topic == "linear algebra"),
            "{raw:?}"
        );
    }
    assert!(matches!(
        parse("/learner").unwrap(),
        ParsedInput::Message(_)
    ));
}

#[test]
fn slash_completion_catalog_is_sorted_deduped_and_prefix_scoped() {
    let all = slash_command_matches("/");
    assert!(all.windows(2).all(|pair| pair[0] < pair[1]));
    assert!(all.contains(&"save"));
    assert!(all.contains(&"redraw"));
    assert!(all.contains(&"diagnostics"));
    assert!(all.contains(&"build"));
    assert!(all.contains(&"run"));
    assert!(all.contains(&"bench"));
    assert!(all.contains(&"doc"));
    assert!(all.contains(&"tree"));
    assert!(all.contains(&"check"));
    assert!(all.contains(&"test"));
    assert!(all.contains(&"lint"));
    assert!(all.contains(&"verify"));
    assert!(all.contains(&"fmt"));
    assert!(all.contains(&"definition"));
    assert!(all.contains(&"hover"));
    assert!(all.contains(&"references"));
    assert!(all.contains(&"symbol"));
    assert!(all.contains(&"symbols"));
    assert!(all.contains(&"learn"));
    assert!(all.contains(&"library"));
    assert!(all.contains(&"tutor"));
    assert_eq!(slash_command_matches("/lea"), vec!["learn"]);
    assert_eq!(slash_command_matches("/redr"), vec!["redraw"]);
    assert_eq!(slash_longest_common_prefix(&["redo", "redraw"]), "/red");
    assert!(slash_command_matches("/save now").is_empty());
    assert!(slash_command_matches("save").is_empty());
}

#[test]
fn slash_inline_hint_ghosts_unique_and_lists_ambiguity() {
    // Unique prefix → ghost the remainder.
    let redr = slash_inline_hint("/redr", 5).expect("unique ghost");
    assert_eq!(redr.suffix, "aw");
    assert!(redr.alternates.is_empty());

    // Ambiguous prefix → alternates list + Tab cue.
    let re = slash_inline_hint("/re", 3).expect("ambiguous ghost");
    assert!(re.alternates.contains("/redo") || re.alternates.contains("/redraw"));
    assert!(re.alternates.contains("Tab"));

    // Fully typed /loop → usage ghost (same as the old special-case).
    let loop_hint = slash_inline_hint("/loop", 5).expect("loop usage");
    assert!(loop_hint.suffix.contains("<task>"));

    let learn_hint = slash_inline_hint("/learn", 6).expect("learn usage");
    assert!(learn_hint.suffix.contains("[topic]"));
    assert!(learn_hint.suffix.contains("Librarium"));

    // Mid-line caret or args → no ghost.
    assert!(slash_inline_hint("/loop", 2).is_none());
    assert!(slash_inline_hint("/loop ship", 10).is_none());
    assert!(slash_inline_hint("hello", 5).is_none());
}

#[test]
fn needs_idle_classifies_turn_starting_vs_local() {
    // Turn-starting / history-rewriting inputs wait for the flight slot…
    for raw in [
        "hello there",
        "/review",
        "/retry",
        "/undo",
        "/new",
        "/resume abc",
        "/moa compare options",
        "/mention src/main.rs",
        "/skills refactor do it",
        "/compact",
        "/fork",
        "/btw quick tangent",
        "/cd /tmp",
        // /self start re-roots the sandbox; integrate/discard rewrite git
        // state; reborn rebuilds and replaces the process.
        "/self fix a clippy finding",
        "/self integrate",
        "/self discard",
        "/self reborn",
    ] {
        assert!(parse(raw).unwrap().needs_idle(), "{raw:?} needs idle");
    }
    // …local commands do not: they're the operator's mid-run control surface.
    for raw in [
        "/goal ship the cockpit",
        "/goal",
        "/goal cmd cargo test",
        "/campaign pause",
        "/campaign status",
        "/loop status",
        "/loop pause",
        "/loop stop",
        "/self",
        "/self status",
        "/status",
        "/diagnostics src/main.rs",
        "/build --workspace cockpit",
        "/run --bin angel -- --help",
        "/bench --bench parser",
        "/doc --no-deps",
        "/tree -i serde",
        "/check --workspace cockpit",
        "/test --workspace cockpit",
        "/lint --workspace cockpit",
        "/verify --workspace cockpit",
        "/fmt check",
        "/definition src/main.rs main",
        "/hover src/main.rs main",
        "/references src/main.rs main",
        "/symbol App",
        "/symbols src/main.rs",
        "/skills",
        "/skills check",
        "/moa",
        "/learn linear algebra",
        "/help",
        "/model",
        "/model auto",
        "/think high",
        "/rate miss",
        "/observatory campaign visual-verifier",
        "/theme",
        "/stop",
        "/ps",
        "exit",
    ] {
        assert!(!parse(raw).unwrap().needs_idle(), "{raw:?} is local");
    }
}

#[test]
fn further_codex_commands_route_by_name_with_args() {
    assert!(matches!(
        parse("/usage").unwrap(),
        ParsedInput::Cmd { name, arg: None } if name == "usage"
    ));
    assert!(matches!(
        parse("/context").unwrap(),
        ParsedInput::Cmd { name, arg: None } if name == "context"
    ));
    assert!(matches!(
        parse("/diagnostics src/main.rs").unwrap(),
        ParsedInput::Cmd { name, arg: Some(a) }
            if name == "diagnostics" && a == "src/main.rs"
    ));
    assert!(matches!(
        parse("/build --workspace cockpit").unwrap(),
        ParsedInput::Cmd { name, arg: Some(a) }
            if name == "build" && a == "--workspace cockpit"
    ));
    assert!(matches!(
        parse("/run --bin angel -- --help").unwrap(),
        ParsedInput::Cmd { name, arg: Some(a) }
            if name == "run" && a == "--bin angel -- --help"
    ));
    assert!(matches!(
        parse("/bench --bench parser").unwrap(),
        ParsedInput::Cmd { name, arg: Some(a) }
            if name == "bench" && a == "--bench parser"
    ));
    assert!(matches!(
        parse("/doc --no-deps").unwrap(),
        ParsedInput::Cmd { name, arg: Some(a) }
            if name == "doc" && a == "--no-deps"
    ));
    assert!(matches!(
        parse("/tree -i serde").unwrap(),
        ParsedInput::Cmd { name, arg: Some(a) }
            if name == "tree" && a == "-i serde"
    ));
    assert!(matches!(
        parse("/check --workspace cockpit").unwrap(),
        ParsedInput::Cmd { name, arg: Some(a) }
            if name == "check" && a == "--workspace cockpit"
    ));
    assert!(matches!(
        parse("/test --workspace cockpit").unwrap(),
        ParsedInput::Cmd { name, arg: Some(a) }
            if name == "test" && a == "--workspace cockpit"
    ));
    assert!(matches!(
        parse("/lint --workspace cockpit").unwrap(),
        ParsedInput::Cmd { name, arg: Some(a) }
            if name == "lint" && a == "--workspace cockpit"
    ));
    assert!(matches!(
        parse("/verify --workspace cockpit").unwrap(),
        ParsedInput::Cmd { name, arg: Some(a) }
            if name == "verify" && a == "--workspace cockpit"
    ));
    assert!(matches!(
        parse("/fmt write").unwrap(),
        ParsedInput::Cmd { name, arg: Some(a) }
            if name == "fmt" && a == "write"
    ));
    assert!(matches!(
        parse("/definition src/main.rs main").unwrap(),
        ParsedInput::Cmd { name, arg: Some(a) }
            if name == "definition" && a == "src/main.rs main"
    ));
    assert!(matches!(
        parse("/hover src/main.rs main").unwrap(),
        ParsedInput::Cmd { name, arg: Some(a) }
            if name == "hover" && a == "src/main.rs main"
    ));
    assert!(matches!(
        parse("/references src/main.rs main").unwrap(),
        ParsedInput::Cmd { name, arg: Some(a) }
            if name == "references" && a == "src/main.rs main"
    ));
    assert!(matches!(
        parse("/symbols src/main.rs").unwrap(),
        ParsedInput::Cmd { name, arg: Some(a) }
            if name == "symbols" && a == "src/main.rs"
    ));
    assert!(matches!(
        parse("/symbol App").unwrap(),
        ParsedInput::Cmd { name, arg: Some(a) }
            if name == "symbol" && a == "App"
    ));
    assert!(matches!(
        parse("/sandbox").unwrap(),
        ParsedInput::Cmd { .. }
    ));
    assert!(matches!(
        parse("/mention src/main.rs").unwrap(),
        ParsedInput::Cmd { name, arg: Some(a) } if name == "mention" && a == "src/main.rs"
    ));
    assert!(matches!(
        parse("/rename my thread").unwrap(),
        ParsedInput::Cmd { name, arg: Some(a) } if name == "rename" && a == "my thread"
    ));
    assert!(matches!(
        parse("/conductor status").unwrap(),
        ParsedInput::Cmd { name, arg: Some(a) } if name == "conductor" && a == "status"
    ));
    assert!(matches!(
        parse("/conductor brief").unwrap(),
        ParsedInput::Cmd { name, arg: Some(a) } if name == "conductor" && a == "brief"
    ));
    // Codex-only-with-no-analog still routes (answered honestly at dispatch).
    assert!(matches!(parse("/pet").unwrap(), ParsedInput::Cmd { .. }));
    assert!(matches!(
        parse("/atlas cargo tests").unwrap(),
        ParsedInput::Cmd { name, arg: Some(a) } if name == "atlas" && a == "cargo tests"
    ));
    assert!(matches!(
        parse("/vault").unwrap(),
        ParsedInput::Cmd { name, arg: None } if name == "vault"
    ));
    assert!(matches!(
        parse("/cut").unwrap(),
        ParsedInput::Cmd { name, arg: None } if name == "cut"
    ));
    assert!(matches!(
        parse("/cut help").unwrap(),
        ParsedInput::Cmd { name, arg: Some(a) } if name == "cut" && a == "help"
    ));
}

#[test]
fn atlas_off_control_restores_unrecognized_message_parsing() {
    let _guard = crate::tests::env_lock();
    let before = std::env::var_os("ANGEL_ATLAS");
    // TODO: Audit that the environment access only happens in single-threaded code.
    unsafe { std::env::set_var("ANGEL_ATLAS", "0") };
    for raw in ["/atlas cargo tests", "/vault"] {
        let ParsedInput::Message(message) = parse(raw).unwrap() else {
            panic!("{raw} should remain an ordinary user message when Atlas is off");
        };
        assert_eq!(&*message.content, raw);
    }
    match before {
        // TODO: Audit that the environment access only happens in single-threaded code.
        Some(value) => unsafe { std::env::set_var("ANGEL_ATLAS", value) },
        // TODO: Audit that the environment access only happens in single-threaded code.
        None => unsafe { std::env::remove_var("ANGEL_ATLAS") },
    }
}

#[test]
fn parses_plain_chat_message() {
    let ParsedInput::Message(msg) = parse("hello agent").unwrap() else {
        panic!("expected chat message");
    };
    assert_eq!(msg.role, ChatRole::User);
    assert_eq!(&*msg.content, "hello agent");
}

#[test]
fn parses_long_plain_and_moa_messages_without_truncating() {
    let raw = format!("prefix {} suffix", "x".repeat(12_000));
    let ParsedInput::Message(msg) = parse(&raw).unwrap() else {
        panic!("expected plain chat message");
    };
    assert_eq!(&*msg.content, raw);
    assert!(msg.content.ends_with("suffix"));

    let moa_raw = format!("/moa {raw}");
    let ParsedInput::Moa(msg) = parse(&moa_raw).unwrap() else {
        panic!("expected moa chat message");
    };
    assert_eq!(&*msg.content, raw);
    assert!(msg.content.ends_with("suffix"));
}

#[test]
fn see_command_attaches_image_with_default_question() {
    let path = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("assets/agents/sparky-neutral.png");
    let ParsedInput::Message(msg) = parse(&format!("/see {}", path.display())).unwrap() else {
        panic!("expected multimodal message");
    };
    assert_eq!(msg.role, ChatRole::User);
    assert_eq!(&*msg.content, "Describe this image.");
    assert_eq!(msg.attachments.len(), 1);
}

#[test]
fn see_command_accepts_custom_question() {
    let path = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("assets/agents/sparky-neutral.png");
    let ParsedInput::Message(msg) =
        parse(&format!("/see {} who is this?", path.display())).unwrap()
    else {
        panic!("expected multimodal message");
    };
    assert_eq!(&*msg.content, "who is this?");
}
