use ratatui::crossterm::event::{self, Event};
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
    mpsc,
};
use std::time::Duration;

const ACTIVE_WAIT: Duration = Duration::from_millis(25);
const QUIET_WAIT: Duration = Duration::from_millis(100);
const QUIET_AFTER_POLLS: u32 = 40;

pub(crate) struct TerminalInput {
    events: mpsc::Receiver<std::io::Result<Event>>,
    stop: Arc<AtomicBool>,
    worker: Option<std::thread::JoinHandle<()>>,
}

impl TerminalInput {
    pub(crate) fn start() -> std::io::Result<Self> {
        #[cfg(unix)]
        let watch_stdin = unsafe { libc::isatty(libc::STDIN_FILENO) } == 1;
        Self::spawn(move |wait| {
            if event::poll(wait)? {
                return event::read().map(Some);
            }
            #[cfg(unix)]
            if watch_stdin && hung_up(libc::STDIN_FILENO) {
                return Err(std::io::Error::new(
                    std::io::ErrorKind::BrokenPipe,
                    "the terminal hung up",
                ));
            }
            Ok(None)
        })
    }

    pub(crate) fn spawn(
        mut read: impl FnMut(Duration) -> std::io::Result<Option<Event>> + Send + 'static,
    ) -> std::io::Result<Self> {
        let (sender, events) = mpsc::channel();
        let stop = Arc::new(AtomicBool::new(false));
        let stopped = Arc::clone(&stop);
        let worker = std::thread::Builder::new()
            .name("terminal-input".into())
            .spawn(move || {
                let mut quiet = 0u32;
                while !stopped.load(Ordering::Relaxed) {
                    let wait = if quiet >= QUIET_AFTER_POLLS {
                        QUIET_WAIT
                    } else {
                        ACTIVE_WAIT
                    };
                    let event = match read(wait) {
                        Ok(None) => {
                            quiet = quiet.saturating_add(1);
                            continue;
                        }
                        Ok(Some(event)) => {
                            quiet = 0;
                            Ok(event)
                        }
                        Err(error) => Err(error),
                    };
                    let failed = event.is_err();
                    if sender.send(event).is_err() || failed {
                        break;
                    }
                }
            })?;
        Ok(Self {
            events,
            stop,
            worker: Some(worker),
        })
    }

    pub(crate) fn next(&self, wait: Duration) -> std::io::Result<Option<Event>> {
        match self.events.recv_timeout(wait) {
            Ok(event) => event.map(Some),
            Err(mpsc::RecvTimeoutError::Timeout) => Ok(None),
            Err(mpsc::RecvTimeoutError::Disconnected) => {
                Err(std::io::Error::other("terminal input reader stopped"))
            }
        }
    }
}

#[cfg(unix)]
pub(crate) fn hung_up(fd: libc::c_int) -> bool {
    let mut probe = libc::pollfd {
        fd,
        events: 0,
        revents: 0,
    };
    let ready = unsafe { libc::poll(&mut probe, 1, 0) };
    ready > 0 && probe.revents & (libc::POLLHUP | libc::POLLERR | libc::POLLNVAL) != 0
}

impl Drop for TerminalInput {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
        if let Some(worker) = self.worker.take() {
            let _ = worker.join();
        }
    }
}

#[cfg(test)]
#[path = "../../../tests/cockpit/app/input__terminal_input_tests.rs"]
mod terminal_input_tests;

use crate::agent::club::{self, ChatMsg};

#[derive(Debug, PartialEq, Eq)]
pub enum ObservatoryCommand {
    Browse,
    Campaign(String),
    OpenReport(String),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum FaceCommand {
    Toggle,
    Say(String),
    Voice(bool),
    Lines,
    Status,
    Lasers,
    Tour,
    TourStop,
    Help,
}

#[derive(Debug)]
pub enum ParsedInput {
    Show(String),
    Assets,
    Tower,
    Hide,
    Open(usize),
    OpenTarget(String),
    ModuleOpen(String),
    ModuleClose(String),
    Modules,
    Layout {
        action: String,
        name: Option<String>,
    },
    MediaPage,
    Observatory(ObservatoryCommand),
    Raytrace,
    Heart,
    HeartLook(bool),
    HeartReset,
    HeartDemo,
    Face(FaceCommand),
    Rl(Option<String>),
    Graph(Option<String>),
    Kg(Option<String>),
    HandoffRl(Option<String>),
    TourneyCalibration(String),
    Sessions,
    Resume(Option<String>),
    Help(Option<String>),
    Learn(Option<String>),
    Ask,
    AskQuestion(String),
    Practice,
    Goal(Option<String>),
    Campaign(Option<String>),
    Loop(Option<String>),
    SelfLoop(Option<String>),
    Moa(ChatMsg),
    MoaDeck(Option<String>),
    Status,
    Save,
    NewChat,
    Diff(Option<String>),
    Review,
    Retry,
    Undo,
    Redo,
    ModelInfo(Option<String>),
    Thinking(Option<String>),
    Rate(Option<String>),
    Init,
    Mcp,
    Cmd {
        name: String,
        arg: Option<String>,
    },
    Exit,
    ForceExit,
    Message(ChatMsg),
}

impl ParsedInput {
    pub fn needs_idle(&self) -> bool {
        match self {
            ParsedInput::Message(_)
            | ParsedInput::AskQuestion(_)
            | ParsedInput::Moa(_)
            | ParsedInput::Review
            | ParsedInput::Retry
            | ParsedInput::Undo
            | ParsedInput::Redo
            | ParsedInput::NewChat
            | ParsedInput::Resume(_) => true,
            ParsedInput::Cmd { name, arg } => match name.as_str() {
                "together" => arg
                    .as_deref()
                    .is_some_and(|value| value.split_whitespace().next() == Some("build")),
                "skills" => arg.as_deref().is_some_and(|value| {
                    let value = value.trim();
                    value != "check" && value != "search" && !value.starts_with("search ")
                }),
                "mention" | "compact" | "fork" | "archive" | "delete" | "import" | "btw" | "cd"
                | "workspace" => true,
                other => cartridge_command(other).is_some(),
            },
            ParsedInput::SelfLoop(arg) => {
                !matches!(arg.as_deref().map(str::trim).unwrap_or(""), "" | "status")
            }
            ParsedInput::HandoffRl(arg) => {
                let verb = arg.as_deref().map(str::trim).unwrap_or("");
                matches!(verb, "step" | "trigger" | "start" | "go" | "next")
            }
            _ => false,
        }
    }
}

const CODEX_CMDS: &[&str] = &[
    "reload",
    "live",
    "peers",
    "usage",
    "context",
    "rollout",
    "keymap",
    "build",
    "run",
    "bench",
    "doc",
    "tree",
    "check",
    "test",
    "lint",
    "verify",
    "fmt",
    "debug-config",
    "definition",
    "diagnostics",
    "hover",
    "symbol",
    "symbols",
    "sandbox",
    "setup-default-sandbox",
    "sandbox-add-read-dir",
    "cd",
    "workspace",
    "approvals",
    "yolo",
    "yolos",
    "approve",
    "experimental",
    "hooks",
    "theme",
    "statusline",
    "title",
    "ps",
    "stop",
    "raw",
    "history",
    "logout",
    "copy",
    "redraw",
    "rename",
    "fork",
    "archive",
    "delete",
    "compact",
    "plan",
    "relentless",
    "solo",
    "selftest",
    "personality",
    "references",
    "mention",
    "ide",
    "vim",
    "pet",
    "plugins",
    "plugin",
    "skills",
    "memories",
    "recall",
    "import",
    "app",
    "btw",
    "feedback",
    "test-approval",
    "world",
    "together",
    "table",
    "dungeon",
    "dungeon_host",
    "game",
    "research",
    "labyrinth",
    "village",
    "quest",
    "moa",
    "dossier",
    "ledger",
    "turns",
    "cut",
    "atlas",
    "vault",
    "habits",
    "refine",
    "conductor",
    "science",
    "repos",
    "still",
    "trace",
    "connect",
    "setup",
    "browser",
];

const DEDICATED_CMDS: &[&str] = &[
    "?",
    "ask",
    "assets",
    "campaign",
    "clear",
    "close",
    "cube",
    "diff",
    "effort",
    "exit",
    "goal",
    "graph",
    "hear",
    "heart",
    "face",
    "onboard",
    "help",
    "hide",
    "init",
    "kg",
    "layout",
    "learn",
    "library",
    "loop",
    "mcp",
    "media",
    "model",
    "modules",
    "new",
    "observatory",
    "open",
    "quit",
    "rate",
    "raytrace",
    "redo",
    "reinforce",
    "resume",
    "retry",
    "review",
    "rl",
    "save",
    "see",
    "self",
    "sessions",
    "show",
    "status",
    "think",
    "thinking",
    "tools",
    "tower",
    "tourney",
    "tutor",
    "undo",
];

pub(crate) fn cartridge_command(
    word: &str,
) -> Option<&'static crate::agent::harness::cartridges::config::Command> {
    if builtin_command(word) {
        return None;
    }
    crate::agent::harness::cartridges::command(word).map(|(_, command)| command)
}

pub(crate) fn builtin_command(word: &str) -> bool {
    let builtin = |name: &&str| name.eq_ignore_ascii_case(word);
    DEDICATED_CMDS.iter().any(builtin) || CODEX_CMDS.iter().any(builtin)
}

pub(crate) fn slash_command_matches(prefix: &str) -> Vec<&'static str> {
    let Some(word) = prefix.strip_prefix('/') else {
        return Vec::new();
    };
    if word.chars().any(char::is_whitespace) {
        return Vec::new();
    }
    let mut matches = DEDICATED_CMDS
        .iter()
        .chain(CODEX_CMDS)
        .copied()
        .chain(
            crate::agent::harness::cartridges::commands()
                .map(|c| c.name.as_str())
                .filter(|name| cartridge_command(name).is_some()),
        )
        .filter(|name| name.starts_with(word))
        .collect::<Vec<_>>();
    matches.sort_unstable();
    matches.dedup();
    matches
}

pub(crate) fn slash_longest_common_prefix(matches: &[&str]) -> String {
    let Some(first) = matches.first() else {
        return String::new();
    };
    let mut length = first.len();
    for candidate in &matches[1..] {
        length = first
            .bytes()
            .zip(candidate.bytes())
            .take(length)
            .take_while(|(left, right)| left == right)
            .count();
    }
    format!("/{}", &first[..length])
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct SlashInlineHint {
    pub suffix: String,
    pub alternates: String,
}

fn slash_usage_ghost(cmd: &str) -> Option<&'static str> {
    Some(match cmd {
        "loop" => " [iterations=25] <task>  | endless: /loop endless <task>",
        "handoff-rl" | "hrl" | "handoff_rl" => {
            " [iterations=25] <task>  | podrace | endless | workshop"
        }
        "goal" => " [<text>|go|criteria|cmd|note|done|clear]",
        "learn" => " [topic] · open Librarium or begin a local lesson",
        "library" | "tutor" => " [topic] · alias for /learn",
        "ask" => " opens an editable tutor question; /ask <question> asks directly",
        "model" => " [filter|exact@effort|auto]",
        "connect" => " [grok|openai|daybreak|glm|deepseek|openrouter|anthropic|local]",
        "setup" => " · a menu that connects a model step by step",
        "browser" => " · choose which Chrome/Chromium the browser tool drives",
        "think" | "thinking" | "effort" => " [filter]",
        "world" => " [visit artisans|colosseum|tournament|follow|help]",
        "together" => " [demo|forge|build|ready|raid|move|fire|cast|return|help]",
        "table" => " [host|join <link>|new|claim|work|submit|apply|merge] · the Round Table work board",
        "dungeon" => " [start|host|join <link>] · invite|kick · F4 menu · Esc composer",
        "game" => " [<name> [host|join <link>|stop]] · list and load game cartridges · Esc composer",
        "dungeon_host" => {
            " --N [--watch]  (N friends, up to 5, or 6 with --watch: one link for everyone; --watch: no knight of your own)"
        }
        "skills" => " [check|search <q>|<name>[,<name>...] [task]]",
        "plugins" | "plugin" => {
            " [search <words>|info <id>|install <id>|update [id]|remove <id>|setup <id>|check <dir>]"
        }
        "memories" => " [add <t>|forget <n>|clear]",
        "refine" => " [status|add …|del|rollback|seed-light]",
        "cd" | "workspace" => " <path>",
        "show" => " [path] · browse assets or show a local file",
        "assets" => " · browse the Scryglass collection",
        "tower" => " · visit the wizard tower",
        "copy" => " [stage [path|text|image|frame]|number|all|live|code]",
        "see" | "hear" => " <path>",
        "rate" => " useful|miss",
        "graph" => " [list|run <name> <task>|status|stop]",
        "campaign" => " [status|new|start|advance|review|…]",
        "kg" => " [stats] · typed knowledge-graph store",
        "moa" => " [cards|ledger|<message>]",
        "self" => " [<goal>|status|integrate|discard|reborn]",
        "solo" | "relentless" | "yolo" | "yolos" => " [on|off|status]",
        "help" | "?" => " · list commands",
        _ => return None,
    })
}

pub(crate) fn slash_inline_hint(input: &str, cursor: usize) -> Option<SlashInlineHint> {
    if !input.starts_with('/') || cursor != input.chars().count() {
        return None;
    }
    if input.chars().any(char::is_whitespace) {
        return None;
    }
    let matches = slash_command_matches(input);
    if matches.is_empty() {
        return None;
    }
    let typed = input.trim_start_matches('/');
    if matches.contains(&typed) {
        if let Some(usage) = slash_usage_ghost(typed) {
            return Some(SlashInlineHint {
                suffix: usage.to_string(),
                alternates: String::new(),
            });
        }
        if let Some(command) = cartridge_command(typed)
            && !command.usage.is_empty()
        {
            return Some(SlashInlineHint {
                suffix: format!(" {}", command.usage),
                alternates: String::new(),
            });
        }
        if matches.len() == 1 {
            return None;
        }
    }

    let common = slash_longest_common_prefix(&matches);
    let mut suffix = String::new();
    if common.len() > input.len() {
        suffix = common[input.len()..].to_string();
    } else if matches.len() == 1 {
        let name = matches[0];
        if name.len() > typed.len() && name.starts_with(typed) {
            suffix = name[typed.len()..].to_string();
        }
    }

    let mut alternates = String::new();
    if matches.len() > 1 {
        const SHOWN: usize = 4;
        let listed = matches
            .iter()
            .take(SHOWN)
            .map(|name| format!("/{name}"))
            .collect::<Vec<_>>()
            .join(" · ");
        alternates.push_str("  ·  ");
        alternates.push_str(&listed);
        if matches.len() > SHOWN {
            alternates.push_str(&format!(" · +{} more", matches.len() - SHOWN));
        }
        alternates.push_str("  · Tab");
    } else if !suffix.is_empty() {
    }

    if suffix.is_empty() && alternates.is_empty() {
        return None;
    }
    Some(SlashInlineHint { suffix, alternates })
}

pub fn parse(raw: &str) -> Result<ParsedInput, String> {
    let trimmed = raw.trim();
    if matches!(trimmed, "/assets" | "/show") {
        return Ok(ParsedInput::Assets);
    }
    if trimmed == "/tower" {
        return Ok(ParsedInput::Tower);
    }
    if let Some(path) = raw.strip_prefix("/show ") {
        return Ok(ParsedInput::Show(path.trim().to_string()));
    }
    let lower = trimmed.to_ascii_lowercase();
    if matches!(lower.as_str(), "exit!" | "quit!" | "/exit!" | "/quit!") {
        return Ok(ParsedInput::ForceExit);
    }
    if matches!(lower.as_str(), "exit" | "quit" | "/exit" | "/quit") {
        return Ok(ParsedInput::Exit);
    }
    if trimmed == "/hide" {
        return Ok(ParsedInput::Hide);
    }
    if let Some(arg) = raw.strip_prefix("/open") {
        let arg = arg.trim();
        if arg.is_empty() {
            return Ok(ParsedInput::Open(1));
        }
        if let Some(target) = normalize_open_target(arg) {
            return Ok(ParsedInput::OpenTarget(target));
        }
        return match arg.parse::<usize>() {
            Ok(n) => Ok(ParsedInput::Open(n)),
            Err(_) => Ok(ParsedInput::ModuleOpen(arg.to_string())),
        };
    }
    if trimmed == "/modules" {
        return Ok(ParsedInput::Modules);
    }
    if let Some(arg) = trimmed.strip_prefix("/close ") {
        return Ok(ParsedInput::ModuleClose(arg.trim().to_string()));
    }
    if let Some(rest) = trimmed.strip_prefix("/layout") {
        let mut it = rest.trim().splitn(2, char::is_whitespace);
        let action = it.next().unwrap_or("").trim();
        if action.is_empty() {
            return Ok(ParsedInput::Layout {
                action: "show".to_string(),
                name: None,
            });
        }
        let name = it
            .next()
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .map(str::to_string);
        return Ok(ParsedInput::Layout {
            action: action.to_string(),
            name,
        });
    }
    if trimmed == "/media" {
        return Ok(ParsedInput::MediaPage);
    }
    if trimmed == "/observatory" {
        return Ok(ParsedInput::Observatory(ObservatoryCommand::Browse));
    }
    if let Some(arg) = trimmed.strip_prefix("/observatory ") {
        let mut parts = arg.split_whitespace();
        let operation = parts.next().unwrap_or_default();
        let id = parts.next().unwrap_or_default();
        if id.is_empty() || parts.next().is_some() {
            return Err("usage: /observatory [campaign <id>|open <report-id>]".to_string());
        }
        let command = match operation {
            "campaign" => ObservatoryCommand::Campaign(id.to_string()),
            "open" => ObservatoryCommand::OpenReport(id.to_string()),
            _ => return Err("usage: /observatory [campaign <id>|open <report-id>]".to_string()),
        };
        return Ok(ParsedInput::Observatory(command));
    }
    if matches!(trimmed, "/raytrace" | "/cube") {
        return Ok(ParsedInput::Raytrace);
    }
    if trimmed == "/face" {
        return Ok(ParsedInput::Face(FaceCommand::Toggle));
    }
    if trimmed == "/onboard" {
        return Ok(ParsedInput::Face(FaceCommand::Tour));
    }
    if trimmed == "/onboard stop" {
        return Ok(ParsedInput::Face(FaceCommand::TourStop));
    }
    if let Some(rest) = trimmed.strip_prefix("/face ").map(str::trim) {
        return Ok(ParsedInput::Face(match rest {
            "mute" => FaceCommand::Voice(false),
            "voice" => FaceCommand::Voice(true),
            "lines" => FaceCommand::Lines,
            "status" => FaceCommand::Status,
            "lasers" => FaceCommand::Lasers,
            "tour" => FaceCommand::Tour,
            say => match say.strip_prefix("say").map(str::trim) {
                Some(what) if !what.is_empty() => FaceCommand::Say(what.to_string()),
                _ => FaceCommand::Help,
            },
        }));
    }
    if trimmed == "/heart" {
        return Ok(ParsedInput::Heart);
    }
    match trimmed.strip_prefix("/heart ").map(str::trim) {
        Some("float") => return Ok(ParsedInput::HeartLook(true)),
        Some("plate") => return Ok(ParsedInput::HeartLook(false)),
        Some("reset") => return Ok(ParsedInput::HeartReset),
        Some("demo") => return Ok(ParsedInput::HeartDemo),
        _ => {}
    }
    if trimmed == "/graph" {
        return Ok(ParsedInput::Graph(None));
    }
    if let Some(arg) = trimmed.strip_prefix("/graph ") {
        return Ok(ParsedInput::Graph(Some(arg.trim().to_string())));
    }
    if trimmed == "/kg" {
        return Ok(ParsedInput::Kg(None));
    }
    if let Some(arg) = trimmed.strip_prefix("/kg ") {
        return Ok(ParsedInput::Kg(Some(arg.trim().to_string())));
    }
    if matches!(trimmed, "/rl" | "/reinforce") {
        return Ok(ParsedInput::Rl(None));
    }
    for prefix in ["/rl ", "/reinforce "] {
        if let Some(arg) = trimmed.strip_prefix(prefix) {
            return Ok(ParsedInput::Rl(Some(arg.trim().to_string())));
        }
    }
    if matches!(trimmed, "/handoff-rl" | "/handoff_rl" | "/hrl") {
        return Ok(ParsedInput::HandoffRl(None));
    }
    for prefix in ["/handoff-rl ", "/handoff_rl ", "/hrl "] {
        if let Some(arg) = trimmed.strip_prefix(prefix) {
            return Ok(ParsedInput::HandoffRl(Some(arg.trim().to_string())));
        }
    }
    if let Some(scene) = trimmed.strip_prefix("/tourney calibrate ") {
        return Ok(ParsedInput::TourneyCalibration(scene.trim().to_string()));
    }
    if trimmed == "/tourney" || trimmed == "/tourney calibrate" {
        return Ok(ParsedInput::TourneyCalibration(String::new()));
    }
    if matches!(trimmed, "/practice") {
        return Ok(ParsedInput::Practice);
    }
    if let Some(question) = trimmed.strip_prefix("/ask ") {
        return Ok(ParsedInput::AskQuestion(question.trim().to_string()));
    }
    if matches!(trimmed, "/ask") {
        return Ok(ParsedInput::Ask);
    }
    if matches!(trimmed, "/learn" | "/tutor" | "/library") {
        return Ok(ParsedInput::Learn(None));
    }
    for prefix in ["/learn ", "/tutor ", "/library "] {
        if let Some(topic) = trimmed.strip_prefix(prefix) {
            let topic = topic.trim();
            return Ok(ParsedInput::Learn(
                (!topic.is_empty()).then(|| topic.to_string()),
            ));
        }
    }
    if looks_like_moa_activation(trimmed) {
        return Ok(ParsedInput::MoaDeck(None));
    }
    if trimmed == "/sessions" {
        return Ok(ParsedInput::Sessions);
    }
    if trimmed == "/resume" {
        return Ok(ParsedInput::Resume(None));
    }
    if let Some(arg) = trimmed.strip_prefix("/resume ") {
        let arg = arg.trim();
        return Ok(ParsedInput::Resume(Some(arg.to_string())));
    }
    if let Some(arg) = trimmed.strip_prefix("/help ") {
        let arg = arg.trim();
        return Ok(ParsedInput::Help(if arg.is_empty() {
            None
        } else {
            Some(arg.to_string())
        }));
    }
    match trimmed {
        "/help" | "/?" => return Ok(ParsedInput::Help(None)),
        "/status" => return Ok(ParsedInput::Status),
        "/save" => return Ok(ParsedInput::Save),
        "/new" | "/clear" => return Ok(ParsedInput::NewChat),
        "/diff" => return Ok(ParsedInput::Diff(None)),
        "/review" => return Ok(ParsedInput::Review),
        "/retry" => return Ok(ParsedInput::Retry),
        "/undo" => return Ok(ParsedInput::Undo),
        "/redo" => return Ok(ParsedInput::Redo),
        "/model" => return Ok(ParsedInput::ModelInfo(None)),
        "/think" | "/thinking" | "/effort" => return Ok(ParsedInput::Thinking(None)),
        "/rate" => return Ok(ParsedInput::Rate(None)),
        "/init" => return Ok(ParsedInput::Init),
        "/mcp" | "/tools" => return Ok(ParsedInput::Mcp),
        "/goal" => return Ok(ParsedInput::Goal(None)),
        "/campaign" => return Ok(ParsedInput::Campaign(None)),
        "/loop" => return Ok(ParsedInput::Loop(None)),
        "/self" => return Ok(ParsedInput::SelfLoop(None)),
        _ => {}
    }
    if let Some(arg) = trimmed.strip_prefix("/goal ") {
        return Ok(ParsedInput::Goal(Some(arg.trim().to_string())));
    }
    if let Some(arg) = trimmed.strip_prefix("/diff ") {
        return Ok(ParsedInput::Diff(Some(arg.trim().to_string())));
    }
    if let Some(arg) = trimmed.strip_prefix("/campaign ") {
        return Ok(ParsedInput::Campaign(Some(arg.trim().to_string())));
    }
    if let Some(arg) = trimmed.strip_prefix("/rate ") {
        return Ok(ParsedInput::Rate(Some(arg.trim().to_string())));
    }
    if let Some(arg) = trimmed.strip_prefix("/model ") {
        return Ok(ParsedInput::ModelInfo(Some(arg.trim().to_string())));
    }
    for prefix in ["/think ", "/thinking ", "/effort "] {
        if let Some(arg) = trimmed.strip_prefix(prefix) {
            return Ok(ParsedInput::Thinking(Some(arg.trim().to_string())));
        }
    }
    if let Some(arg) = trimmed.strip_prefix("/loop ") {
        return Ok(ParsedInput::Loop(Some(arg.trim().to_string())));
    }
    if let Some(arg) = trimmed.strip_prefix("/self ") {
        return Ok(ParsedInput::SelfLoop(Some(arg.trim().to_string())));
    }
    if let Some(arg) = raw.strip_prefix("/moa ") {
        let arg = arg.trim();
        if moa_deck_subcommand(arg) {
            return Ok(ParsedInput::MoaDeck(Some(arg.to_string())));
        }
        return parse_user_message(arg).map(ParsedInput::Moa);
    }
    if trimmed == "/moa" {
        return Ok(ParsedInput::MoaDeck(None));
    }
    if let Some(rest) = trimmed.strip_prefix('/') {
        let mut it = rest.splitn(2, char::is_whitespace);
        let word = it.next().unwrap_or("");
        if CODEX_CMDS.contains(&word) {
            if matches!(word, "atlas" | "vault") && crate::knowledge::atlas::atlas_env_disabled() {
                return parse_user_message(raw).map(ParsedInput::Message);
            }
            let arg = it
                .next()
                .map(str::trim)
                .filter(|s| !s.is_empty())
                .map(str::to_string);
            return Ok(ParsedInput::Cmd {
                name: word.to_string(),
                arg,
            });
        }
        if let Some(command) = cartridge_command(word) {
            let arg = it
                .next()
                .map(str::trim)
                .filter(|s| !s.is_empty())
                .map(str::to_string);
            return Ok(ParsedInput::Cmd {
                name: command.name.clone(),
                arg,
            });
        }
    }
    parse_user_message(raw).map(ParsedInput::Message)
}

fn moa_deck_subcommand(arg: &str) -> bool {
    let word = arg
        .split_whitespace()
        .next()
        .unwrap_or("")
        .to_ascii_lowercase();
    if matches!(
        word.as_str(),
        "cards"
            | "deck"
            | "panel"
            | "options"
            | "clear"
            | "off"
            | "status"
            | "ledger"
            | "report"
            | "solo"
            | "recon"
            | "duel"
            | "council"
            | "all-in"
            | "allin"
            | "grok"
            | "grokwar"
            | "war"
            | "trio"
            | "battle"
            | "frontier"
            | "last"
            | "sota"
            | "tag"
            | "tagteam"
            | "local"
            | "home"
    ) {
        return true;
    }
    crate::agent::formations::formation_alias(arg).is_some()
}

fn looks_like_moa_activation(trimmed: &str) -> bool {
    let lower = trimmed.to_ascii_lowercase();
    matches!(
        lower.as_str(),
        "activate moa"
            | "activate the moa"
            | "open moa"
            | "open the moa"
            | "show moa"
            | "show the moa"
            | "moa cards"
            | "moa deck"
    )
}

fn normalize_open_target(arg: &str) -> Option<String> {
    let arg = arg.trim();
    if arg.starts_with("http://") || arg.starts_with("https://") || arg.starts_with("file://") {
        return Some(arg.to_string());
    }
    if arg.contains(char::is_whitespace) {
        return None;
    }
    if arg.starts_with('/') || arg.starts_with("~/") {
        let expanded = if let Some(rest) = arg.strip_prefix("~/") {
            std::env::var("HOME")
                .ok()
                .map(|home| format!("{home}/{rest}"))
                .unwrap_or_else(|| arg.to_string())
        } else {
            arg.to_string()
        };
        if std::path::Path::new(&expanded).exists() {
            return Some(format!("file://{expanded}"));
        }
        return None;
    }
    if arg.starts_with("localhost:")
        || arg.starts_with("127.")
        || arg.starts_with("[::1]:")
        || arg.starts_with("10.")
        || arg.starts_with("192.168.")
        || arg.starts_with("172.")
        || looks_like_dns_name(arg)
    {
        return Some(format!("http://{arg}"));
    }
    None
}

fn looks_like_dns_name(arg: &str) -> bool {
    let host = arg.split('/').next().unwrap_or(arg);
    let host = host.split(':').next().unwrap_or(host);
    host.contains('.')
        && host
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '.'))
}

fn parse_user_message(raw: &str) -> Result<ChatMsg, String> {
    if let Some(rest) = raw.strip_prefix("/see ") {
        let (path, q) = split_path_and_question(rest, "Describe this image.");
        let media = club::Media::image_from_path(std::path::Path::new(&path))?;
        Ok(ChatMsg::user_with_media(q, vec![media]))
    } else if let Some(rest) = raw.strip_prefix("/hear ") {
        let (path, q) = split_path_and_question(rest, "Transcribe and describe this audio.");
        let media = club::Media::audio_from_path(std::path::Path::new(&path))?;
        Ok(ChatMsg::user_with_media(q, vec![media]))
    } else {
        Ok(ChatMsg::user(raw))
    }
}

fn split_path_and_question(rest: &str, default_q: &str) -> (String, String) {
    let rest = rest.trim();
    let unquoted = || {
        let mut it = rest.splitn(2, char::is_whitespace);
        (it.next().unwrap_or("").trim(), it.next().unwrap_or(""))
    };
    let quoted = rest
        .chars()
        .next()
        .filter(|quote| matches!(*quote, '\'' | '"'))
        .and_then(|quote| {
            let after_open = &rest[quote.len_utf8()..];
            let end = after_open.find(quote)?;
            let question = &after_open[end + quote.len_utf8()..];
            (question.is_empty() || question.starts_with(char::is_whitespace))
                .then_some((&after_open[..end], question))
        });
    let (path, question) = quoted.unwrap_or_else(unquoted);
    let q = if question.trim().is_empty() {
        default_q.to_string()
    } else {
        question.trim().to_string()
    };
    (path.to_string(), q)
}

#[cfg(test)]
#[path = "../../../tests/cockpit/app/input__tests.rs"]
mod tests;
