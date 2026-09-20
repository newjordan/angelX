use ratatui::crossterm::event::{self, Event};
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
    mpsc,
};
use std::time::Duration;

/// The terminal has one reader, independent of rendering. An unbounded FIFO
/// preserves every decoded event when drawing is back-pressured.
pub(crate) struct TerminalInput {
    events: mpsc::Receiver<std::io::Result<Event>>,
    stop: Arc<AtomicBool>,
    worker: Option<std::thread::JoinHandle<()>>,
}

impl TerminalInput {
    pub(crate) fn start() -> std::io::Result<Self> {
        Self::spawn(|wait| {
            if event::poll(wait)? {
                event::read().map(Some)
            } else {
                Ok(None)
            }
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
                while !stopped.load(Ordering::Relaxed) {
                    let event = match read(Duration::from_millis(25)) {
                        Ok(None) => continue,
                        Ok(Some(event)) => Ok(event),
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

#[derive(Debug)]
pub enum ParsedInput {
    Show(String),
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
    /// Browse the native campaign/report evidence catalog in the artifacts pane.
    Observatory(ObservatoryCommand),
    /// Toggle the live CPU-raytraced cube in the artifacts pane (`/raytrace`, `/cube`).
    Raytrace,
    /// Open/drive the Realm/Reinforce RL pipeline stage
    /// (`/rl [run [flags]|status|stop]`); run uses the workspace objective or
    /// explicit task/verifier cases, with an optional independent audit.
    Rl(Option<String>),
    /// Open/drive the Round Table agent-graph stage
    /// (`/graph [list|run <name> <task>|status|stop]`).
    Graph(Option<String>),
    /// Inspect the typed knowledge-graph store (`/kg [stats]`).
    Kg(Option<String>),
    /// Open/drive the fresh-context Handoff RL competition loop
    /// (`/handoff-rl [[time/rolls/budget workshop]|podrace|endless|status|stop|…]`).
    HandoffRl(Option<String>),
    /// Display-only Angel-DMD tourney / knight-journey calibration
    /// (`/tourney calibrate <scene>`).
    TourneyCalibration(String),
    Sessions,
    Resume(Option<String>),
    /// List the available slash commands (`/help`, `/?`, `/help <topic>`).
    Help(Option<String>),
    /// Open the physical Librarium or begin a local-first lesson
    /// (`/learn [topic]`, with `/tutor` and `/library` aliases).
    Learn(Option<String>),
    Ask,
    AskQuestion(String),
    Practice,
    /// Set / view / clear the long-running goal (`/goal [text|clear]`).
    Goal(Option<String>),
    /// Author or control the native proof campaign (`/campaign [...]`).
    Campaign(Option<String>),
    /// Open/drive the autonomous loop (`/loop [<task>|start|restart|stop|pause|resume|status|sota]`).
    Loop(Option<String>),
    /// Self-modification run over the cockpit's own crate
    /// (`/self [<goal>|status|integrate|discard]`).
    SelfLoop(Option<String>),
    /// Run one message through an already armed formation roster (`/moa <msg>`).
    Moa(ChatMsg),
    /// Open or control the MoA formation-roster graph (`/moa`, `/moa cards`).
    MoaDeck(Option<String>),
    /// Show session config + usage (`/status`).
    Status,
    /// Persist current committed history and wait for the FIFO writer barrier
    /// (`/save`).
    Save,
    /// Reset the conversation — a fresh chat (`/new`, `/clear`).
    NewChat,
    /// Show a bounded working-tree git diff or summary
    /// (`/diff [all|staged] [--stat]`).
    Diff(Option<String>),
    /// Hand the current git diff to the in-hand agent to review (`/review`).
    Review,
    /// Replace the last assistant/tool exchange and resend its exact request
    /// prefix (`/retry`).
    Retry,
    /// Remove the latest surviving operator exchange from conversation history
    /// without reverting filesystem state (`/undo`).
    Undo,
    /// Restore the most recently conversation-only undone exchange (`/redo`).
    Redo,
    /// Open/prefilter model controls, prepare exact model+effort confirmation,
    /// or clear remembered selection (`/model [filter|exact@effort|auto]`).
    ModelInfo(Option<String>),
    /// Open or prefilter the selected route's effort deck without mutating it
    /// (`/think [filter]`, with `/thinking` and `/effort` aliases).
    Thinking(Option<String>),
    /// Explicitly label the last completed answer (`/rate useful|miss`).
    Rate(Option<String>),
    /// Create a starter `AGENTS.md` if absent (`/init`).
    Init,
    /// List the registered tool registry, incl. MCP tools (`/mcp`, `/tools`).
    Mcp,
    /// A further Codex command, dispatched by name (with optional args) in
    /// [`App`](crate::App). Real cockpit behavior where one exists.
    Cmd {
        name: String,
        arg: Option<String>,
    },
    /// Close the cockpit (typed `exit`/`quit`, with or without the `/`).
    Exit,
    Message(ChatMsg),
}

impl ParsedInput {
    /// Whether this input needs the single turn slot to be free: it either
    /// starts a model turn or rewrites the history/workspace an in-flight reply
    /// would land in. Everything else is a **local command** and runs even while
    /// a turn or background job is in flight — `/goal`, `/loop pause`, `/status`
    /// are exactly what an operator reaches for mid-run.
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
            // Codex commands that send a turn (`/mention`, `/skills <name>`),
            // rewrite/park the history an in-flight reply would land in
            // (`/compact`, `/fork`, `/archive`, `/delete`, `/import`, `/btw`),
            // or re-root the tool sandbox under a running turn (`/cd`).
            ParsedInput::Cmd { name, arg } => match name.as_str() {
                "skills" => arg.as_deref().is_some_and(|value| {
                    let value = value.trim();
                    value != "check" && value != "search" && !value.starts_with("search ")
                }),
                "mention" | "compact" | "fork" | "archive" | "delete" | "import" | "btw" | "cd"
                | "workspace" => true,
                _ => false,
            },
            // `/self` re-roots the tool sandbox (start) or rewrites git state
            // (integrate/discard); only its status form is a local read.
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

/// The remaining Codex slash commands (beyond the ones with dedicated variants
/// above), each dispatched by name with real cockpit behavior where one exists.
const CODEX_CMDS: &[&str] = &[
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
    "plain",
    "solo",
    "selftest",
    "personality",
    "references",
    "mention",
    "ide",
    "vim",
    "pet",
    "plugins",
    "skills",
    "memories",
    "recall",
    "import",
    "app",
    "btw",
    "feedback",
    "test-approval",
    "world",
    "research",
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
];

const DEDICATED_CMDS: &[&str] = &[
    "?",
    "clear",
    "close",
    "cube",
    "diff",
    "effort",
    "exit",
    "goal",
    "graph",
    "hear",
    "help",
    "hide",
    "init",
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
    "tourney",
    "tutor",
    "undo",
];

/// Deterministic local candidates for a slash word. Arguments are deliberately
/// excluded: once whitespace appears the operator is composing command input,
/// not completing a command name.
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

/// Inline ghost completion for the composer while typing a slash command.
///
/// - `suffix` — characters that would complete the typed prefix (dim ghost text)
/// - `alternates` — short ` · /foo · /bar` list when several commands match
///
/// Only when the caret is at the end of the buffer and the draft is still a
/// bare `/word` (no arguments yet). Tab still *commits* the completion; this
/// only *hints* it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct SlashInlineHint {
    pub suffix: String,
    pub alternates: String,
}

/// Short usage ghosts for fully typed command names (caret at end).
fn slash_usage_ghost(cmd: &str) -> Option<&'static str> {
    Some(match cmd {
        "loop" => " [iterations=25] <task>  | endless: /loop endless <task>",
        "handoff-rl" | "hrl" | "handoff_rl" => {
            " [iterations=25] <task>  | podrace | endless | workshop"
        }
        "goal" => " [<text>|go|criteria|cmd|note|done|clear]",
        "learn" => " [topic] · open Librarium or begin a local lesson",
        "library" | "tutor" => " [topic] · alias for /learn",
        "model" => " [filter|exact@effort|auto]",
        "connect" => " [grok|openai|glm|deepseek|openrouter|local]",
        "think" | "thinking" | "effort" => " [filter]",
        "skills" => " [check|search <q>|<name>[,<name>...] [task]]",
        "memories" => " [add <t>|forget <n>|clear]",
        "refine" => " [status|add …|del|rollback|seed-light]",
        "cd" | "workspace" => " <path>",
        "show" | "see" | "hear" => " <path>",
        "rate" => " useful|miss",
        "graph" => " [list|run <name> <task>|status|stop]",
        "campaign" => " [status|new|start|advance|review|…]",
        "moa" => " [cards|gpu|ledger|<message>]",
        "self" => " [<goal>|status|integrate|discard|reborn]",
        "solo" | "relentless" | "plain" | "yolo" | "yolos" => " [on|off|status]",
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
    // Fully typed unique (or ambiguous but complete) command name → usage ghost.
    if matches.contains(&typed) {
        if let Some(usage) = slash_usage_ghost(typed) {
            return Some(SlashInlineHint {
                suffix: usage.to_string(),
                alternates: String::new(),
            });
        }
        if matches.len() == 1 {
            return None; // exact unique match, no usage text
        }
    }

    let common = slash_longest_common_prefix(&matches);
    let mut suffix = String::new();
    if common.len() > input.len() {
        suffix = common[input.len()..].to_string();
    } else if matches.len() == 1 {
        // Unique match whose LCP equals the full name but typed is a proper prefix
        // of that name (should already be covered); still fill from the match.
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
        // Unique partial: light Tab cue so operators know the ghost is commit-able.
        // (No alternate list.)
    }

    if suffix.is_empty() && alternates.is_empty() {
        return None;
    }
    Some(SlashInlineHint { suffix, alternates })
}

/// Parse an input line into a typed cockpit action.
///
/// `/see <path> [question]` and `/hear <path> [question]` attach media for
/// multimodal clubs. Local UI commands are parsed here but executed by `App`.
pub fn parse(raw: &str) -> Result<ParsedInput, String> {
    if let Some(path) = raw.strip_prefix("/show ") {
        return Ok(ParsedInput::Show(path.trim().to_string()));
    }
    let trimmed = raw.trim();
    // Close the cockpit on a typed `exit`/`quit` (bare or slash-prefixed), so the
    // app is closed deliberately rather than with a stray Ctrl+C.
    if matches!(
        trimmed.to_ascii_lowercase().as_str(),
        "exit" | "quit" | "/exit" | "/quit"
    ) {
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
    if looks_like_gpu_comp_moa_activation(trimmed) {
        return Ok(ParsedInput::MoaDeck(Some("gpu comp".to_string())));
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
    // Codex-style slash commands ported into the cockpit.
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
    // A further recognized Codex command: split into name + optional args.
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
    }
    parse_user_message(raw).map(ParsedInput::Message)
}

fn moa_deck_subcommand(arg: &str) -> bool {
    let word = arg
        .split_whitespace()
        .next()
        .unwrap_or("")
        .to_ascii_lowercase();
    matches!(
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
            | "gpu"
            | "comp"
            | "competition"
            | "overnight"
            | "sleep"
            | "solo"
            | "recon"
            | "duel"
            | "council"
            | "all-in"
            | "allin"
            // Grok War / frontier-trio aliases (first word only — full arg is
            // resolved by formation_alias when the deck opens).
            | "grok"
            | "grokwar"
            | "war"
            | "trio"
            | "battle"
            | "frontier"
            | "last"
            | "sota"
            // Tag Team / local-pair aliases (first word only, as above).
            | "tag"
            | "tagteam"
            | "local"
            | "home"
            // Math God / Lean-solver aliases (first word only, as above).
            | "math"
            | "mathgod"
            | "proximity"
            | "soundness"
            | "lean"
    )
}

fn looks_like_gpu_comp_moa_activation(trimmed: &str) -> bool {
    let lower = trimmed.to_ascii_lowercase();
    matches!(
        lower.as_str(),
        "activate gpu comp moa"
            | "activate gpu competition moa"
            | "activate the gpu comp moa"
            | "activate the gpu competition moa"
            | "open gpu comp moa"
            | "open gpu competition moa"
            | "gpu comp moa"
            | "gpu competition moa"
            | "overnight moa"
            | "sleep moa"
    )
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
    let mut it = rest.trim().splitn(2, char::is_whitespace);
    let path = it.next().unwrap_or("").trim().to_string();
    let q = it
        .next()
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .unwrap_or(default_q)
        .to_string();
    (path, q)
}

#[cfg(test)]
#[path = "../../../tests/cockpit/app/input__tests.rs"]
mod tests;
