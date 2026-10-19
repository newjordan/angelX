//! `/setup`: connect a model from a menu anyone can drive. One question per
//! screen in plain words, with what this computer already has listed first.
//! Arrows or a number and Enter choose, Esc goes back, and the mouse clicks.
//!
//! The menu itself never touches the network, a file or a process. It asks
//! for a [`Job`] or a [`Saving`], the cockpit runs it, and the [`Outcome`]
//! comes back through [`Menu::finish`]. The choice lands in
//! ~/.angelX/angel.env (the file `angelX setup --text` writes and bin/angelX
//! loads), and the model list is rebuilt in place, so the new model takes
//! over without a restart.

use super::App;
use crate::agent::club::Bag;
use crate::agent::sandbox::process_owner::OwnedCommandExt;
use ratatui::crossterm::event::{
    KeyCode, KeyEvent, KeyModifiers, MouseButton, MouseEvent, MouseEventKind,
};
use ratatui::layout::Rect;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::mpsc::{self, Receiver, Sender, TryRecvError};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

/// The guide's sheet and the scroll plate, under cockpit/.
pub(crate) const GUIDE_SHEET: &str = "assets/onboarding/setup/guide.png";
pub(crate) const SCROLL_PLATE: &str = "assets/onboarding/setup/scroll.png";

/// The guide recruit's eight poses, in sheet order: the top row left to
/// right, then the bottom row.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Pose {
    Idle,
    Wave,
    Point,
    LiftKey,
    Pull,
    Strain,
    Stumble,
    ThumbsUp,
}

/// An API provider a pasted key can be for. `model_var` is set when the
/// provider has no default model and one must be picked.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct Provider {
    pub(crate) driver: &'static str,
    pub(crate) name: &'static str,
    pub(crate) key_var: &'static str,
    pub(crate) url: &'static str,
    pub(crate) site: &'static str,
    pub(crate) model_var: Option<&'static str>,
}

pub(crate) const PROVIDERS: &[Provider] = &[
    Provider {
        driver: "deepseek",
        name: "DeepSeek",
        key_var: "DEEPSEEK_API_KEY",
        url: "https://api.deepseek.com/v1",
        site: "https://platform.deepseek.com/api_keys",
        model_var: None,
    },
    Provider {
        driver: "openai-api",
        name: "OpenAI",
        key_var: "OPENAI_API_KEY",
        url: "https://api.openai.com/v1",
        site: "https://platform.openai.com/api-keys",
        model_var: Some("ANGEL_OPENAI_API_MODEL"),
    },
    Provider {
        driver: "grok-api",
        name: "xAI Grok",
        key_var: "XAI_API_KEY",
        url: "https://api.x.ai/v1",
        site: "https://console.x.ai",
        model_var: None,
    },
    Provider {
        driver: "glm",
        name: "Z.ai GLM",
        key_var: "ZAI_API_KEY",
        url: "https://api.z.ai/api/coding/paas/v4",
        site: "https://z.ai",
        model_var: None,
    },
    Provider {
        driver: "kimi",
        name: "Kimi",
        key_var: "MOONSHOT_API_KEY",
        url: "https://api.moonshot.ai/v1",
        site: "https://platform.moonshot.ai",
        model_var: None,
    },
    Provider {
        driver: "kimi-code",
        name: "Kimi Code plan",
        key_var: "KIMI_API_KEY",
        url: "https://api.kimi.com/coding/v1",
        site: "https://kimi.com/code",
        model_var: None,
    },
    Provider {
        driver: "openrouter",
        name: "OpenRouter",
        key_var: "OPENROUTER_API_KEY",
        url: "https://openrouter.ai/api/v1",
        site: "https://openrouter.ai/keys",
        model_var: Some("ANGEL_OPENROUTER_MODEL"),
    },
    Provider {
        driver: "cerebras",
        name: "Cerebras",
        key_var: "CEREBRAS_API_KEY",
        url: "https://api.cerebras.ai/v1",
        site: "https://cloud.cerebras.ai",
        model_var: Some("ANGEL_CEREBRAS_MODEL"),
    },
];

/// A provider key already in the environment: the driver, its name, the key
/// variables the cockpit reads (first set wins) and the model variables it
/// needs (the first is saved). The same table as scripts/runtime/angel-setup.sh.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct EnvSeat {
    pub(crate) driver: &'static str,
    pub(crate) name: &'static str,
    pub(crate) vars: &'static [&'static str],
    pub(crate) model_vars: &'static [&'static str],
}

pub(crate) const ENV_SEATS: &[EnvSeat] = &[
    EnvSeat {
        driver: "deepseek",
        name: "DeepSeek",
        vars: &["ANGEL_DEEPSEEK_KEY", "DEEPSEEK_API_KEY"],
        model_vars: &[],
    },
    EnvSeat {
        driver: "openai-api",
        name: "OpenAI API",
        vars: &["ANGEL_OPENAI_KEY", "OPENAI_API_KEY"],
        model_vars: &["ANGEL_OPENAI_API_MODEL", "OPENAI_MODEL"],
    },
    EnvSeat {
        driver: "grok-api",
        name: "xAI Grok API",
        vars: &["ANGEL_XAI_KEY", "XAI_API_KEY", "GROK_API_KEY"],
        model_vars: &[],
    },
    EnvSeat {
        driver: "glm",
        name: "Z.ai GLM",
        vars: &[
            "ANGEL_GLM_KEY",
            "GLM_API_KEY",
            "ZAI_API_KEY",
            "ZHIPU_API_KEY",
            "BIGMODEL_API_KEY",
        ],
        model_vars: &[],
    },
    EnvSeat {
        driver: "kimi",
        name: "Kimi",
        vars: &["ANGEL_KIMI_KEY", "KIMI_API_KEY", "MOONSHOT_API_KEY"],
        model_vars: &[],
    },
    EnvSeat {
        driver: "openrouter",
        name: "OpenRouter",
        vars: &["ANGEL_OPENROUTER_KEY", "OPENROUTER_API_KEY"],
        model_vars: &["ANGEL_OPENROUTER_MODEL", "OPENROUTER_MODEL"],
    },
    EnvSeat {
        driver: "cerebras",
        name: "Cerebras",
        vars: &["ANGEL_CEREBRAS_KEY", "CEREBRAS_API_KEY"],
        model_vars: &["ANGEL_CEREBRAS_MODEL", "CEREBRAS_MODEL"],
    },
    EnvSeat {
        driver: "meta",
        name: "Meta Muse",
        vars: &["ANGEL_META_KEY", "META_API_KEY"],
        model_vars: &[],
    },
    EnvSeat {
        driver: "qwen",
        name: "Qwen Coding Plan",
        vars: &["ANGEL_QWEN_KEY", "QWEN_API_KEY", "DASHSCOPE_API_KEY"],
        model_vars: &[],
    },
    EnvSeat {
        driver: "longcat",
        name: "LongCat",
        vars: &["ANGEL_LONGCAT_KEY", "LONGCAT_API_KEY"],
        model_vars: &[],
    },
];

/// Default ports of the common local model servers.
const LOCAL_PORTS: &[u16] = &[11434, 1234, 8080, 8000, 30000, 1337, 5000, 5001];

/// A plan the operator already pays for, signed in through its own CLI.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Plan {
    ChatGpt,
    Grok,
}

impl Plan {
    fn name(self) -> &'static str {
        match self {
            Plan::ChatGpt => "ChatGPT",
            Plan::Grok => "Grok",
        }
    }
    fn plan_words(self) -> &'static str {
        match self {
            Plan::ChatGpt => "ChatGPT Plus, Pro or Business",
            Plan::Grok => "SuperGrok",
        }
    }
    fn app_name(self) -> &'static str {
        match self {
            Plan::ChatGpt => "OpenAI's Codex app",
            Plan::Grok => "xAI's Grok app",
        }
    }
    pub(crate) fn tool(self) -> &'static str {
        match self {
            Plan::ChatGpt => "codex",
            Plan::Grok => "grok",
        }
    }
    fn login_args(self) -> &'static [&'static str] {
        match self {
            Plan::ChatGpt => &["login"],
            Plan::Grok => &["login", "--oauth"],
        }
    }
    fn package(self) -> &'static str {
        match self {
            Plan::ChatGpt => "@openai/codex",
            Plan::Grok => "@xai-official/grok",
        }
    }
    fn driver(self) -> &'static str {
        match self {
            Plan::ChatGpt => "openai",
            Plan::Grok => "grok",
        }
    }
    pub(crate) fn auth_file(self) -> PathBuf {
        let (var, dir) = match self {
            Plan::ChatGpt => ("CODEX_HOME", ".codex"),
            Plan::Grok => ("GROK_HOME", ".grok"),
        };
        std::env::var_os(var)
            .filter(|v| !v.is_empty())
            .map(PathBuf::from)
            .unwrap_or_else(|| home().join(dir))
            .join("auth.json")
    }
}

/// A model server answering on this computer.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct Server {
    pub(crate) name: String,
    pub(crate) url: String,
    pub(crate) models: Vec<String>,
}

/// What this computer already has.
#[derive(Clone, Debug, Default)]
pub(crate) struct Found {
    pub(crate) servers: Vec<Server>,
    /// Indices into [`ENV_SEATS`], with the variable that holds the key.
    pub(crate) keys: Vec<(usize, &'static str)>,
    pub(crate) chatgpt: bool,
    pub(crate) grok: bool,
}

/// Where the plan CLIs live, when they are installed, and whether npm is.
#[derive(Clone, Debug, Default)]
pub(crate) struct Tools {
    pub(crate) codex: Option<PathBuf>,
    pub(crate) grok: Option<PathBuf>,
    pub(crate) npm: bool,
}

impl Tools {
    fn get(&self, plan: Plan) -> Option<&PathBuf> {
        match plan {
            Plan::ChatGpt => self.codex.as_ref(),
            Plan::Grok => self.grok.as_ref(),
        }
    }
    fn set(&mut self, plan: Plan, path: Option<PathBuf>) {
        match plan {
            Plan::ChatGpt => self.codex = path,
            Plan::Grok => self.grok = path,
        }
    }
}

/// A sign-in in progress: its output so far (it prints the sign-in link)
/// and its process id, so Esc can stop it.
#[derive(Clone, Default)]
pub(crate) struct Login {
    pub(crate) log: Arc<Mutex<String>>,
    pub(crate) pid: Arc<AtomicU32>,
}

/// Work the cockpit runs off the draw thread for the menu.
pub(crate) enum Job {
    Scan,
    CheckKey {
        provider: usize,
        key: String,
    },
    ServerModels {
        url: String,
        key: Option<String>,
    },
    SeatModels {
        driver: &'static str,
        key: String,
    },
    Login {
        plan: Plan,
        tool: PathBuf,
        login: Login,
    },
    Install {
        plan: Plan,
    },
}

/// What a [`Job`] found. `Answer` is an HTTP status (0 when nothing
/// answered), the models the answer listed and the address that gave it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum Outcome {
    Scanned(Vec<Server>),
    Answer {
        status: u16,
        models: Vec<String>,
        url: String,
    },
    Finished {
        ok: bool,
        log: String,
        tool: Option<PathBuf>,
    },
}

/// Settings to save, and the words for what got connected.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct Saving {
    pub(crate) pairs: Vec<(&'static str, String)>,
    pub(crate) name: String,
    pub(crate) tip: Option<String>,
}

/// What a key or click asks of the cockpit.
#[derive(Debug, PartialEq, Eq)]
pub(crate) enum Effect {
    Stay,
    Close,
    Run(u64),
    Save(Saving),
    Open(String),
}

/// A clickable spot the view drew.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Hit {
    Item(usize),
    Back,
    Open,
}

#[derive(Clone, Debug, PartialEq, Eq)]
enum Pick {
    Local {
        url: String,
        name: String,
        key: Option<String>,
    },
    Key {
        provider: usize,
        key: String,
    },
    Seat {
        seat: usize,
    },
}

#[derive(Clone, Debug, PartialEq, Eq)]
enum Screen {
    Home,
    Providers,
    Key {
        provider: usize,
    },
    Unchecked {
        provider: usize,
        key: String,
        why: String,
    },
    Plan(Plan),
    Local,
    Address,
    ServerKey {
        url: String,
    },
    Models {
        models: Vec<String>,
        pick: Pick,
    },
    Done {
        name: String,
        tip: Option<String>,
    },
}

#[derive(Clone, Debug, PartialEq, Eq)]
enum Action {
    UseServer(usize),
    UsePlan(Plan),
    UseSeat(usize),
    SignIn(Plan),
    Providers,
    Local,
    Later,
    Provider(usize),
    Rescan,
    OpenSite(&'static str),
    Address,
    Install(Plan),
    Login(Plan),
    Back,
    Model(String),
    SaveUnchecked,
    RetryKey,
    Finish,
}

/// What a running job answers to.
#[derive(Clone, Debug)]
enum After {
    Key { provider: usize, key: String },
    Server { key: Option<String> },
    Seat { seat: usize },
    Login(Plan),
    Install(Plan),
}

struct Waiting {
    serial: u64,
    title: String,
    lines: Vec<String>,
    after: After,
    login: Option<Login>,
}

/// One row of a menu list.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct Item {
    pub(crate) label: String,
    pub(crate) about: String,
    pub(crate) ready: bool,
}

/// A text box: what it shows (keys are masked) and its hint when empty.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct Field {
    pub(crate) shown: String,
    pub(crate) hint: &'static str,
}

/// Everything the view draws for one frame.
#[derive(Clone, Debug)]
pub(crate) struct Page {
    pub(crate) title: String,
    pub(crate) lines: Vec<String>,
    pub(crate) items: Vec<Item>,
    pub(crate) cursor: usize,
    pub(crate) field: Option<Field>,
    pub(crate) note: Option<String>,
    pub(crate) pose: Pose,
    pub(crate) busy: bool,
    /// The bottom bar: key, then what it does.
    pub(crate) keys: Vec<(&'static str, &'static str)>,
    /// Ctrl+O has somewhere to go.
    pub(crate) link: bool,
}

pub(crate) struct Menu {
    screen: Screen,
    back: Vec<(Screen, usize)>,
    cursor: usize,
    field: String,
    note: Option<String>,
    /// When the note appeared and whether it is good news: the guide
    /// stumbles at a problem and gives a thumbs-up at good news, briefly.
    noted: Option<(Instant, bool)>,
    found: Found,
    tools: Tools,
    scanning: bool,
    waiting: Option<Waiting>,
    /// The job [`Menu::take_job`] hands out next.
    job: Option<Job>,
    serial: u64,
    opened: Instant,
    entered: Instant,
    /// The route in hand, or `None` while angelX has no model.
    pub(crate) current: Option<String>,
    /// The guide's sheet, when its art is in place.
    pub(crate) guide: Option<PathBuf>,
}

impl Menu {
    pub(crate) fn new(found: Found, tools: Tools, now: Instant) -> Self {
        Self {
            screen: Screen::Home,
            back: Vec::new(),
            cursor: 0,
            field: String::new(),
            note: None,
            noted: None,
            found,
            tools,
            scanning: false,
            waiting: None,
            job: None,
            serial: 0,
            opened: now,
            entered: now,
            current: None,
            guide: None,
        }
    }

    /// Start looking for local servers; the result comes back as `Scanned`.
    pub(crate) fn scan(&mut self) -> Effect {
        self.scanning = true;
        self.run(Job::Scan)
    }

    pub(crate) fn take_job(&mut self) -> Option<Job> {
        self.job.take()
    }

    fn run(&mut self, job: Job) -> Effect {
        self.serial += 1;
        self.job = Some(job);
        Effect::Run(self.serial)
    }

    fn wait(&mut self, title: String, lines: Vec<String>, after: After, job: Job) -> Effect {
        let login = match &job {
            Job::Login { login, .. } => Some(login.clone()),
            _ => None,
        };
        let effect = self.run(job);
        self.waiting = Some(Waiting {
            serial: self.serial,
            title,
            lines,
            after,
            login,
        });
        self.note = None;
        self.noted = None;
        effect
    }

    fn goto(&mut self, screen: Screen, now: Instant) {
        let old = std::mem::replace(&mut self.screen, screen);
        self.back.push((old, self.cursor));
        self.cursor = 0;
        self.field.clear();
        self.note = None;
        self.noted = None;
        self.entered = now;
    }

    fn back(&mut self, now: Instant) -> Effect {
        if matches!(self.screen, Screen::Done { .. }) {
            return Effect::Close;
        }
        match self.back.pop() {
            Some((screen, cursor)) => {
                self.screen = screen;
                self.cursor = cursor;
                self.field.clear();
                self.note = None;
                self.noted = None;
                self.entered = now;
                Effect::Stay
            }
            None => Effect::Close,
        }
    }

    /// The saved choice went through: show the finish.
    pub(crate) fn show_done(&mut self, name: String, tip: Option<String>, now: Instant) {
        self.waiting = None;
        self.back.clear();
        self.screen = Screen::Done { name, tip };
        self.cursor = 0;
        self.field.clear();
        self.note = None;
        self.noted = None;
        self.entered = now;
    }

    pub(crate) fn set_note(&mut self, note: String) {
        self.note_at(note, Instant::now(), false);
    }

    fn note_at(&mut self, note: String, now: Instant, good: bool) {
        self.note = Some(note);
        self.noted = Some((now, good));
    }

    fn has_field(&self) -> bool {
        matches!(
            self.screen,
            Screen::Key { .. } | Screen::ServerKey { .. } | Screen::Address | Screen::Models { .. }
        )
    }

    fn secret_field(&self) -> bool {
        matches!(self.screen, Screen::Key { .. } | Screen::ServerKey { .. })
    }

    /// The list on this screen, each row with what Enter on it does.
    fn actions(&self) -> Vec<(Action, Item)> {
        let item = |label: String, about: &str| Item {
            label,
            about: about.to_string(),
            ready: false,
        };
        let back = || (Action::Back, item("Back".into(), "Go back one step."));
        let mut rows = Vec::new();
        match &self.screen {
            Screen::Home => {
                rows.extend(self.server_rows());
                for plan in [Plan::ChatGpt, Plan::Grok] {
                    if self.signed_in(plan) {
                        rows.push((
                            Action::UsePlan(plan),
                            Item {
                                label: format!("Use my {} plan", plan.name()),
                                about: format!(
                                    "You're already signed in to {} on this computer.",
                                    plan.name()
                                ),
                                ready: true,
                            },
                        ));
                    }
                }
                for (i, (seat, var)) in self.found.keys.iter().enumerate() {
                    rows.push((
                        Action::UseSeat(i),
                        Item {
                            label: format!("Use my {} key", ENV_SEATS[*seat].name),
                            about: format!("It's already set up on this computer (${var})."),
                            ready: true,
                        },
                    ));
                }
                for plan in [Plan::ChatGpt, Plan::Grok] {
                    if !self.signed_in(plan) {
                        rows.push((
                            Action::SignIn(plan),
                            item(
                                format!("Sign in with {}", plan.name()),
                                &format!(
                                    "For a {} plan. Your web browser opens so you can sign in.",
                                    plan.plan_words()
                                ),
                            ),
                        ));
                    }
                }
                rows.push((
                    Action::Providers,
                    item(
                        "Paste an API key".into(),
                        "From DeepSeek, OpenAI, xAI, Z.ai, Kimi, OpenRouter or Cerebras. \
                         You pay that company for what you use.",
                    ),
                ));
                rows.push((
                    Action::Local,
                    item(
                        "Use a free model on this computer".into(),
                        "Ollama, LM Studio or llama.cpp. Free and private, \
                         but it needs a strong computer.",
                    ),
                ));
                rows.push((
                    Action::Later,
                    match &self.current {
                        Some(route) => item("Close".into(), &format!("Keep using {route}.")),
                        None => item(
                            "Not now".into(),
                            "Look around first. Type /setup whenever you're ready.",
                        ),
                    },
                ));
            }
            Screen::Providers => {
                for (i, p) in PROVIDERS.iter().enumerate() {
                    rows.push((
                        Action::Provider(i),
                        item(
                            p.name.into(),
                            &format!("Get a key at {}", short_site(p.site)),
                        ),
                    ));
                }
                rows.push(back());
            }
            Screen::Unchecked { .. } => {
                rows.push((
                    Action::SaveUnchecked,
                    item(
                        "Save it anyway".into(),
                        "Use the key now; if it's wrong, the first reply will say so.",
                    ),
                ));
                rows.push((
                    Action::RetryKey,
                    item("Paste it again".into(), "Go back and paste the key again."),
                ));
            }
            Screen::Plan(plan) => {
                if self.tools.get(*plan).is_some() {
                    rows.push((
                        Action::Login(*plan),
                        item(
                            "Sign in".into(),
                            "Opens your web browser. Sign in there, then come back.",
                        ),
                    ));
                } else if self.tools.npm {
                    rows.push((
                        Action::Install(*plan),
                        item(
                            "Install it for me".into(),
                            &format!("Runs: npm install -g {}", plan.package()),
                        ),
                    ));
                } else {
                    rows.push((
                        Action::OpenSite("https://nodejs.org"),
                        item(
                            "Open nodejs.org".into(),
                            "Opens the Node.js download page in your browser.",
                        ),
                    ));
                }
                rows.push(back());
            }
            Screen::Local => {
                rows.extend(self.server_rows());
                rows.push((
                    Action::Rescan,
                    item(
                        "Look again".into(),
                        "Checks this computer for a running model.",
                    ),
                ));
                rows.push((
                    Action::OpenSite("https://ollama.com"),
                    item(
                        "Get Ollama (free)".into(),
                        "Opens ollama.com in your browser.",
                    ),
                ));
                rows.push((
                    Action::Address,
                    item(
                        "It's on another computer".into(),
                        "Type the address of a model server on your network.",
                    ),
                ));
                rows.push(back());
            }
            Screen::Models { .. } => {
                rows.extend(
                    self.filtered_models()
                        .into_iter()
                        .map(|m| (Action::Model(m.clone()), item(m, ""))),
                );
            }
            Screen::Done { .. } => {
                rows.push((Action::Finish, item("Start".into(), "")));
            }
            Screen::Key { .. } | Screen::ServerKey { .. } | Screen::Address => {}
        }
        rows
    }

    fn server_rows(&self) -> Vec<(Action, Item)> {
        self.found
            .servers
            .iter()
            .enumerate()
            .map(|(i, s)| {
                let first = s.models.first().map(String::as_str).unwrap_or("a model");
                let more = match s.models.len() {
                    0 | 1 => " is".to_string(),
                    n => format!(" and {} more are", n - 1),
                };
                (
                    Action::UseServer(i),
                    Item {
                        label: format!("Use {} on this computer", s.name),
                        about: format!(
                            "{first}{more} ready. Free and private: it runs on your own machine."
                        ),
                        ready: true,
                    },
                )
            })
            .collect()
    }

    fn signed_in(&self, plan: Plan) -> bool {
        match plan {
            Plan::ChatGpt => self.found.chatgpt,
            Plan::Grok => self.found.grok,
        }
    }

    fn filtered_models(&self) -> Vec<String> {
        let Screen::Models { models, .. } = &self.screen else {
            return Vec::new();
        };
        let needle = self.field.trim().to_ascii_lowercase();
        models
            .iter()
            .filter(|m| needle.is_empty() || m.to_ascii_lowercase().contains(&needle))
            .cloned()
            .collect()
    }

    /// Where Ctrl+O goes on this screen.
    fn link(&self) -> Option<String> {
        if let Some(waiting) = &self.waiting {
            return waiting.login.as_ref().and_then(login_url);
        }
        match &self.screen {
            Screen::Key { provider } => Some(PROVIDERS[*provider].site.to_string()),
            Screen::Plan(plan) if self.tools.get(*plan).is_none() && !self.tools.npm => {
                Some("https://nodejs.org".into())
            }
            Screen::Local => Some("https://ollama.com".into()),
            _ => None,
        }
    }

    pub(crate) fn key(&mut self, key: KeyEvent, now: Instant) -> Effect {
        let ctrl = key.modifiers.contains(KeyModifiers::CONTROL);
        if ctrl && matches!(key.code, KeyCode::Char('c' | 'C')) {
            self.cancel();
            return Effect::Close;
        }
        if ctrl && matches!(key.code, KeyCode::Char('o' | 'O')) {
            return self.link().map_or(Effect::Stay, Effect::Open);
        }
        if self.waiting.is_some() {
            if key.code == KeyCode::Esc {
                self.cancel();
            }
            return Effect::Stay;
        }
        let field = self.has_field();
        if ctrl && field && matches!(key.code, KeyCode::Char('u' | 'U')) {
            self.field.clear();
            self.cursor = 0;
            return Effect::Stay;
        }
        match key.code {
            KeyCode::Esc => self.back(now),
            KeyCode::Up | KeyCode::BackTab => self.step(-1),
            KeyCode::Down | KeyCode::Tab => self.step(1),
            KeyCode::PageUp => self.step(-8),
            KeyCode::PageDown => self.step(8),
            KeyCode::Home if !field => {
                self.cursor = 0;
                Effect::Stay
            }
            KeyCode::End if !field => {
                self.cursor = self.actions().len().saturating_sub(1);
                Effect::Stay
            }
            KeyCode::Enter => self.enter(now),
            KeyCode::Backspace if field => {
                self.field.pop();
                self.cursor = 0;
                Effect::Stay
            }
            KeyCode::Char(c) if field && !ctrl => {
                if !(self.secret_field() && c.is_whitespace()) {
                    self.field.push(c);
                    self.cursor = 0;
                }
                Effect::Stay
            }
            KeyCode::Char(c @ '1'..='9') if !field => {
                let index = usize::from(c as u8 - b'1');
                if index < self.actions().len() {
                    self.cursor = index;
                    self.activate(index, now)
                } else {
                    Effect::Stay
                }
            }
            _ => Effect::Stay,
        }
    }

    /// Bracketed paste lands in the text box. A key keeps no whitespace;
    /// anything else keeps its first line.
    pub(crate) fn paste(&mut self, text: &str) {
        if self.waiting.is_some() || !self.has_field() {
            return;
        }
        if self.secret_field() {
            self.field
                .extend(text.chars().filter(|c| !c.is_whitespace()));
        } else {
            self.field
                .push_str(text.lines().next().unwrap_or("").trim());
        }
        self.cursor = 0;
    }

    pub(crate) fn click(&mut self, hit: Hit, now: Instant) -> Effect {
        if self.waiting.is_some() {
            return match hit {
                Hit::Back => {
                    self.cancel();
                    Effect::Stay
                }
                Hit::Open => self.link().map_or(Effect::Stay, Effect::Open),
                Hit::Item(_) => Effect::Stay,
            };
        }
        match hit {
            Hit::Item(index) => {
                self.cursor = index;
                self.activate(index, now)
            }
            Hit::Back => self.back(now),
            Hit::Open => self.link().map_or(Effect::Stay, Effect::Open),
        }
    }

    pub(crate) fn wheel(&mut self, delta: isize) {
        if self.waiting.is_none() {
            self.step(delta);
        }
    }

    fn step(&mut self, delta: isize) -> Effect {
        let len = self.actions().len();
        if len > 0 {
            self.cursor = self.cursor.saturating_add_signed(delta).min(len - 1);
        }
        Effect::Stay
    }

    fn enter(&mut self, now: Instant) -> Effect {
        match self.screen.clone() {
            Screen::Key { provider } => {
                let key = self.field.trim().to_string();
                if let Some(problem) = key_problem(&key) {
                    self.note_at(problem.into(), now, false);
                    return Effect::Stay;
                }
                self.wait(
                    "Checking your key".into(),
                    vec![format!(
                        "Asking {} if the key works.",
                        PROVIDERS[provider].name
                    )],
                    After::Key {
                        provider,
                        key: key.clone(),
                    },
                    Job::CheckKey { provider, key },
                )
            }
            Screen::ServerKey { url } => {
                let key = self.field.trim().to_string();
                if let Some(problem) = key_problem(&key) {
                    self.note_at(problem.into(), now, false);
                    return Effect::Stay;
                }
                self.wait(
                    "Trying the key".into(),
                    vec![format!("Asking {url} for its models.")],
                    After::Server {
                        key: Some(key.clone()),
                    },
                    Job::ServerModels {
                        url,
                        key: Some(key),
                    },
                )
            }
            Screen::Address => {
                let Some(url) = server_url(&self.field) else {
                    self.note_at("Type the server's address first.".into(), now, false);
                    return Effect::Stay;
                };
                self.wait(
                    "Looking for the server".into(),
                    vec![format!("Asking {url} for its models.")],
                    After::Server { key: None },
                    Job::ServerModels { url, key: None },
                )
            }
            Screen::Models { .. } => {
                let models = self.filtered_models();
                match models.get(self.cursor) {
                    Some(model) => self.pick_model(model.clone()),
                    None if !self.field.trim().is_empty() => {
                        let typed = self.field.trim().to_string();
                        self.pick_model(typed)
                    }
                    None => Effect::Stay,
                }
            }
            _ => self.activate(self.cursor, now),
        }
    }

    fn activate(&mut self, index: usize, now: Instant) -> Effect {
        let Some((action, _)) = self.actions().into_iter().nth(index) else {
            return Effect::Stay;
        };
        match action {
            Action::UseServer(i) => {
                let server = self.found.servers[i].clone();
                self.offer_models(
                    server.models,
                    Pick::Local {
                        url: server.url,
                        name: server.name,
                        key: None,
                    },
                    now,
                )
            }
            Action::UsePlan(plan) => Effect::Save(plan_saving(plan)),
            Action::UseSeat(i) => {
                let (seat, var) = self.found.keys[i];
                let entry = ENV_SEATS[seat];
                let needs_model = !entry.model_vars.is_empty()
                    && entry
                        .model_vars
                        .iter()
                        .all(|v| std::env::var(v).map_or(true, |s| s.trim().is_empty()));
                if needs_model {
                    let key = std::env::var(var).unwrap_or_default();
                    self.wait(
                        "Getting the model list".into(),
                        vec![format!(
                            "Asking {} which models your key can use.",
                            entry.name
                        )],
                        After::Seat { seat },
                        Job::SeatModels {
                            driver: entry.driver,
                            key,
                        },
                    )
                } else {
                    Effect::Save(seat_saving(seat, None))
                }
            }
            Action::SignIn(plan) => {
                self.goto(Screen::Plan(plan), now);
                Effect::Stay
            }
            Action::Providers => {
                self.goto(Screen::Providers, now);
                Effect::Stay
            }
            Action::Local => {
                self.goto(Screen::Local, now);
                Effect::Stay
            }
            Action::Later | Action::Finish => Effect::Close,
            Action::Provider(provider) => {
                self.goto(Screen::Key { provider }, now);
                Effect::Stay
            }
            Action::Rescan => self.scan(),
            Action::OpenSite(url) => Effect::Open(url.into()),
            Action::Address => {
                self.goto(Screen::Address, now);
                Effect::Stay
            }
            Action::Install(plan) => self.wait(
                format!("Installing {}", plan.app_name()),
                vec!["This can take a minute.".into()],
                After::Install(plan),
                Job::Install { plan },
            ),
            Action::Login(plan) => {
                let Some(tool) = self.tools.get(plan).cloned() else {
                    return Effect::Stay;
                };
                self.wait(
                    "Finish signing in in your browser".into(),
                    vec![
                        "Your web browser should have opened. Sign in there.".into(),
                        "This moves on by itself when you're done.".into(),
                    ],
                    After::Login(plan),
                    Job::Login {
                        plan,
                        tool,
                        login: Login::default(),
                    },
                )
            }
            Action::Back => self.back(now),
            Action::Model(model) => self.pick_model(model),
            Action::SaveUnchecked => match &self.screen {
                Screen::Unchecked { provider, key, .. } => {
                    Effect::Save(key_saving(*provider, key.clone(), None))
                }
                _ => Effect::Stay,
            },
            Action::RetryKey => {
                self.back(now);
                Effect::Stay
            }
        }
    }

    /// Several models: let the operator pick. One: take it.
    fn offer_models(&mut self, models: Vec<String>, pick: Pick, now: Instant) -> Effect {
        if models.len() == 1 {
            return Effect::Save(pick_saving(&pick, models[0].clone()));
        }
        self.goto(Screen::Models { models, pick }, now);
        Effect::Stay
    }

    fn pick_model(&mut self, model: String) -> Effect {
        match &self.screen {
            Screen::Models { pick, .. } => Effect::Save(pick_saving(pick, model)),
            _ => Effect::Stay,
        }
    }

    /// Stop the job in hand. A sign-in's browser wait is a process; it ends.
    pub(crate) fn cancel(&mut self) {
        if let Some(waiting) = self.waiting.take()
            && let Some(login) = waiting.login
        {
            let pid = login.pid.load(Ordering::Relaxed);
            if pid != 0 {
                // SAFETY: plain signal to the child this menu started.
                unsafe {
                    libc::kill(pid as libc::pid_t, libc::SIGTERM);
                }
            }
        }
    }

    /// A job came back. Answers to a job that was cancelled are dropped.
    pub(crate) fn finish(&mut self, serial: u64, outcome: Outcome, now: Instant) -> Effect {
        if let Outcome::Scanned(servers) = outcome {
            let before = self.actions().into_iter().nth(self.cursor).map(|a| a.0);
            self.found.servers = servers;
            self.scanning = false;
            // New rows arrive at the top: keep the cursor on the same choice.
            if self.cursor > 0
                && let Some(before) = before
                && let Some(index) = self.actions().iter().position(|(a, _)| *a == before)
            {
                self.cursor = index;
            }
            return Effect::Stay;
        }
        let Some(waiting) = self.waiting.take_if(|w| w.serial == serial) else {
            return Effect::Stay;
        };
        match (waiting.after, outcome) {
            (After::Key { provider, key }, Outcome::Answer { status, models, .. }) => {
                let p = PROVIDERS[provider];
                match status {
                    200..=299 if p.model_var.is_some() => {
                        self.goto(
                            Screen::Models {
                                models,
                                pick: Pick::Key { provider, key },
                            },
                            now,
                        );
                        Effect::Stay
                    }
                    200..=299 => Effect::Save(key_saving(provider, key, None)),
                    401 | 403 => {
                        self.field.clear();
                        self.note_at(
                            format!(
                                "{} said no to that key. Copy the whole key and paste it again.",
                                p.name
                            ),
                            now,
                            false,
                        );
                        Effect::Stay
                    }
                    _ => {
                        self.goto(
                            Screen::Unchecked {
                                provider,
                                key,
                                why: answer_words(status),
                            },
                            now,
                        );
                        Effect::Stay
                    }
                }
            }
            (
                After::Server { key },
                Outcome::Answer {
                    status,
                    models,
                    url,
                },
            ) => match status {
                200..=299 if models.is_empty() => {
                    self.note_at(format!("{url} answered but lists no models."), now, false);
                    Effect::Stay
                }
                200..=299 => {
                    let name = host_of(&url);
                    if matches!(self.screen, Screen::ServerKey { .. }) {
                        self.back.pop();
                        self.screen = Screen::Address;
                    }
                    self.offer_models(models, Pick::Local { url, name, key }, now)
                }
                401 | 403 if key.is_none() => {
                    self.goto(Screen::ServerKey { url }, now);
                    Effect::Stay
                }
                401 | 403 => {
                    self.field.clear();
                    self.note_at(
                        "The server didn't take that key. Paste it again.".into(),
                        now,
                        false,
                    );
                    Effect::Stay
                }
                _ => {
                    self.note_at(
                        format!(
                            "Nothing answered at {url}. Check the address, and that the server is running."
                        ),
                        now,
                        false,
                    );
                    Effect::Stay
                }
            },
            (After::Seat { seat }, Outcome::Answer { status, models, .. }) => {
                if (200..300).contains(&status) && !models.is_empty() {
                    self.offer_models(models, Pick::Seat { seat }, now)
                } else {
                    Effect::Save(seat_saving(seat, None))
                }
            }
            (After::Login(plan), Outcome::Finished { ok, log, .. }) => {
                if ok {
                    Effect::Save(plan_saving(plan))
                } else {
                    let note = match last_line(&log) {
                        Some(line) => format!("Sign-in didn't finish: {line}"),
                        None => "Sign-in didn't finish. Choose Sign in to try again.".into(),
                    };
                    self.note_at(note, now, false);
                    Effect::Stay
                }
            }
            (After::Install(plan), Outcome::Finished { ok, log, tool }) => {
                if ok && tool.is_some() {
                    self.tools.set(plan, tool);
                    self.note_at("Installed. Now choose Sign in.".into(), now, true);
                } else {
                    let note = format!(
                        "The install didn't work{}. You can run it yourself in a terminal: \
                         npm install -g {}",
                        last_line(&log)
                            .map(|l| format!(" ({l})"))
                            .unwrap_or_default(),
                        plan.package()
                    );
                    self.note_at(note, now, false);
                }
                Effect::Stay
            }
            _ => Effect::Stay,
        }
    }

    pub(crate) fn page(&self, now: Instant) -> Page {
        let since = now.saturating_duration_since(self.entered);
        let rows = self.actions();
        let cursor = self.cursor.min(rows.len().saturating_sub(1));
        let items: Vec<Item> = rows.into_iter().map(|(_, item)| item).collect();
        let list = vec![("↑↓", "choose"), ("Enter", "pick"), ("Esc", "back")];
        let typing = vec![("Enter", "done"), ("Esc", "back")];
        if let Some(waiting) = &self.waiting {
            let mut lines = waiting.lines.clone();
            if let Some(url) = waiting.login.as_ref().and_then(login_url) {
                lines.push(String::new());
                lines.push("Didn't open? Press Ctrl+O, or visit:".into());
                lines.push(url);
            }
            let link = waiting.login.as_ref().and_then(login_url).is_some();
            return Page {
                title: waiting.title.clone(),
                lines,
                items: Vec::new(),
                cursor: 0,
                field: None,
                note: None,
                // Hauling on the rope while the job runs: pull, strain, pull.
                pose: if (since.as_millis() / 700).is_multiple_of(2) {
                    Pose::Pull
                } else {
                    Pose::Strain
                },
                busy: true,
                keys: vec![("Esc", "stop")],
                link,
            };
        }
        let mut page = Page {
            title: String::new(),
            lines: Vec::new(),
            items,
            cursor,
            field: None,
            note: self.note.clone(),
            pose: Pose::Point,
            busy: false,
            keys: list,
            link: self.link().is_some(),
        };
        match &self.screen {
            Screen::Home => {
                page.title = "Connect angelX to an AI model".into();
                page.lines.push(match &self.current {
                    Some(route) => {
                        format!("angelX is using {route} right now. Pick something to change it.")
                    }
                    None => "angelX needs an AI model before it can do real work.".into(),
                });
                page.lines
                    .push("Pick one: use ↑ ↓ and Enter, or press its number.".into());
                if self.scanning {
                    page.lines.push("Looking around this computer…".into());
                }
                page.pose =
                    if now.saturating_duration_since(self.opened) < Duration::from_millis(2200) {
                        Pose::Wave
                    } else {
                        Pose::Point
                    };
            }
            Screen::Providers => {
                page.title = "Which company is your key from?".into();
                page.lines.push(
                    "No key yet? Pick the company anyway; the next step shows where to get one."
                        .into(),
                );
            }
            Screen::Key { provider } => {
                let p = PROVIDERS[*provider];
                page.title = format!("Paste your {} key", p.name);
                page.lines = vec![
                    format!("1. Make a key at {}  (Ctrl+O opens it)", p.site),
                    "2. Copy the key, then paste it here.".into(),
                    "   To paste: Ctrl+Shift+V (Cmd+V on a Mac), or right-click → Paste.".into(),
                    "3. Press Enter. We'll check that it works.".into(),
                ];
                page.field = Some(Field {
                    shown: masked(&self.field),
                    hint: "paste your key here",
                });
                page.pose = Pose::LiftKey;
                page.keys = vec![
                    ("Enter", "check key"),
                    ("Ctrl+O", "open site"),
                    ("Esc", "back"),
                ];
            }
            Screen::Unchecked { provider, why, .. } => {
                page.title = "We couldn't check that key".into();
                page.lines = vec![
                    format!("{}: {why}.", PROVIDERS[*provider].name),
                    "The key may still be fine. You can save it and try it.".into(),
                ];
                page.pose = Pose::Stumble;
            }
            Screen::Plan(plan) => {
                if self.tools.get(*plan).is_some() {
                    page.title = format!("Sign in with {}", plan.name());
                    page.lines = vec![
                        format!("This uses your {} plan.", plan.plan_words()),
                        "When you choose Sign in, your web browser opens.".into(),
                        "Sign in there, then come back to this window.".into(),
                    ];
                } else {
                    page.title = format!("{} needs one small app first", plan.name());
                    page.lines = vec![format!(
                        "Signing in to {} uses {}, which is free.",
                        plan.name(),
                        plan.app_name()
                    )];
                    if self.tools.npm {
                        page.lines
                            .push("We can install it for you. It takes about a minute.".into());
                    } else {
                        page.lines.push(
                            "Installing it needs Node.js, also free. Get it from nodejs.org,"
                                .into(),
                        );
                        page.lines
                            .push("then close this, open a new terminal and come back.".into());
                    }
                }
            }
            Screen::Local => {
                page.title = "A free model on this computer".into();
                if self.scanning {
                    page.lines
                        .push("Looking for models on this computer…".into());
                } else if self.found.servers.is_empty() {
                    page.lines = vec![
                        "Nothing is running on this computer yet.".into(),
                        "The easy free way: install Ollama, open it, and download a coding model."
                            .into(),
                        "Then come back here and choose Look again.".into(),
                    ];
                } else {
                    page.lines
                        .push("Found these on this computer. Pick one.".into());
                }
            }
            Screen::Address => {
                page.title = "Where is the model server?".into();
                page.lines = vec![
                    "Type its address and press Enter.".into(),
                    "For example: 192.168.1.20:11434  or  http://my-pc:8080/v1".into(),
                ];
                page.field = Some(Field {
                    shown: self.field.clone(),
                    hint: "address",
                });
                page.pose = Pose::Idle;
                page.keys = typing;
            }
            Screen::ServerKey { url } => {
                page.title = "This server wants a key".into();
                page.lines = vec![
                    format!("{url} asked for a key (like a password)."),
                    "Paste it here and press Enter.".into(),
                ];
                page.field = Some(Field {
                    shown: masked(&self.field),
                    hint: "paste the key here",
                });
                page.pose = Pose::LiftKey;
                page.keys = typing;
            }
            Screen::Models { models, pick } => {
                page.title = "Pick a model".into();
                page.lines.push(if models.is_empty() {
                    "Type the name of the model to use and press Enter.".into()
                } else {
                    "Type to search, then pick with ↑ ↓ and Enter.".into()
                });
                if matches!(pick, Pick::Key { provider, .. } if PROVIDERS[*provider].driver == "openrouter")
                {
                    page.lines
                        .push("Models ending in :free cost nothing.".into());
                }
                page.field = Some(Field {
                    shown: self.field.clone(),
                    hint: "type to search",
                });
                page.keys = vec![("↑↓", "choose"), ("Enter", "pick"), ("Esc", "back")];
            }
            Screen::Done { name, tip } => {
                page.title = "You're all set!".into();
                page.lines = vec![
                    format!("angelX now uses {name}."),
                    "Close this, type what you want to make, and press Enter.".into(),
                    "For example: explain what this project does".into(),
                ];
                if let Some(tip) = tip {
                    page.lines.push(String::new());
                    page.lines.push(tip.clone());
                }
                page.lines.push(String::new());
                page.lines.push(
                    "Saved in ~/.angelX/angel.env. Type /setup to change it any time.".into(),
                );
                page.pose = Pose::ThumbsUp;
                page.keys = vec![("Enter", "start")];
            }
        }
        // A fresh note sets the guide's mood for a moment.
        if let Some((at, good)) = self.noted
            && now.saturating_duration_since(at) < Duration::from_millis(1600)
        {
            page.pose = if good { Pose::ThumbsUp } else { Pose::Stumble };
        }
        page
    }
}

// --- what the choices save --------------------------------------------------

fn key_saving(provider: usize, key: String, model: Option<String>) -> Saving {
    let p = PROVIDERS[provider];
    if p.driver == "kimi-code" {
        return Saving {
            pairs: vec![
                ("KIMI_API_KEY", key),
                ("KIMI_API_URL", p.url.into()),
                ("KIMI_MODEL", "k3".into()),
                ("ANGEL_DRIVER", "kimi".into()),
            ],
            name: "the Kimi Code plan (k3)".into(),
            tip: None,
        };
    }
    let mut pairs = vec![(p.key_var, key), ("ANGEL_DRIVER", p.driver.into())];
    if let (Some(var), Some(model)) = (p.model_var, &model) {
        pairs.push((var, model.clone()));
    }
    Saving {
        pairs,
        name: match model {
            Some(model) => format!("{} ({model})", p.name),
            None => p.name.into(),
        },
        tip: None,
    }
}

fn plan_saving(plan: Plan) -> Saving {
    Saving {
        pairs: vec![("ANGEL_DRIVER", plan.driver().into())],
        name: format!("your {} plan", plan.name()),
        tip: None,
    }
}

fn seat_saving(seat: usize, model: Option<String>) -> Saving {
    let entry = ENV_SEATS[seat];
    let mut pairs = vec![("ANGEL_DRIVER", entry.driver.into())];
    if let (Some(var), Some(model)) = (entry.model_vars.first(), &model) {
        pairs.push((var, model.clone()));
    }
    Saving {
        pairs,
        name: match model {
            Some(model) => format!("{} ({model})", entry.name),
            None => entry.name.into(),
        },
        tip: None,
    }
}

fn pick_saving(pick: &Pick, model: String) -> Saving {
    match pick {
        Pick::Local { url, name, key } => {
            let mut pairs = vec![
                ("ANGEL_LOCAL_URL", url.clone()),
                ("ANGEL_LOCAL_MODEL", model.clone()),
            ];
            if let Some(key) = key {
                pairs.push(("ANGEL_BRAIN_KEY", key.clone()));
            }
            pairs.push(("ANGEL_DRIVER", "local".into()));
            Saving {
                pairs,
                name: format!("{name} ({model})"),
                tip: (name == "Ollama").then(|| {
                    "Tip: Ollama gives models a short memory by default. For coding, start it \
                     with OLLAMA_CONTEXT_LENGTH=32768 or more."
                        .into()
                }),
            }
        }
        Pick::Key { provider, key } => key_saving(*provider, key.clone(), Some(model)),
        Pick::Seat { seat } => seat_saving(*seat, Some(model)),
    }
}

// --- small words ------------------------------------------------------------

fn key_problem(key: &str) -> Option<&'static str> {
    if key.is_empty() {
        Some("Paste your key first.")
    } else if !key
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || "._~+/=:-".contains(c))
    {
        Some("That doesn't look like a key. Copy it again and paste it.")
    } else {
        None
    }
}

/// Dots for a key, with its last four characters so a paste can be checked.
fn masked(key: &str) -> String {
    let count = key.chars().count();
    if count <= 4 {
        return "•".repeat(count);
    }
    let tail: String = key.chars().skip(count - 4).collect();
    format!("{}{tail}", "•".repeat((count - 4).min(24)))
}

fn short_site(site: &str) -> &str {
    site.trim_start_matches("https://")
        .trim_start_matches("http://")
}

fn host_of(url: &str) -> String {
    short_site(url).split('/').next().unwrap_or(url).to_string()
}

fn answer_words(status: u16) -> String {
    match status {
        0 => "no answer (check the internet connection)".into(),
        429 => "too many tries right now (error 429)".into(),
        code => format!("an unexpected answer (error {code})"),
    }
}

/// A typed server address as a URL: http:// when no scheme, no trailing /.
fn server_url(typed: &str) -> Option<String> {
    let url = typed.trim().trim_end_matches('/');
    if url.is_empty() {
        return None;
    }
    Some(
        if url.starts_with("http://") || url.starts_with("https://") {
            url.to_string()
        } else {
            format!("http://{url}")
        },
    )
}

fn last_line(log: &str) -> Option<String> {
    log.lines()
        .map(str::trim)
        .rfind(|l| !l.is_empty())
        .map(|l| l.chars().take(160).collect())
}

/// The sign-in link a login printed, if it printed one.
fn login_url(login: &Login) -> Option<String> {
    let log = login.log.lock().ok()?;
    log.split_whitespace()
        .find(|word| word.starts_with("https://"))
        .map(|word| {
            word.trim_end_matches(['.', ',', ')', '"', '\''])
                .to_string()
        })
}

fn home() -> PathBuf {
    std::env::var_os("HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("."))
}

pub(crate) fn settings_path() -> PathBuf {
    home().join(".angelX").join("angel.env")
}

// --- reading this computer --------------------------------------------------

/// Logins and keys this computer already has. Cheap: env reads and two stats.
pub(crate) fn found_here() -> Found {
    let set = |var: &str| std::env::var(var).is_ok_and(|v| !v.trim().is_empty());
    let keys = ENV_SEATS
        .iter()
        .enumerate()
        .filter_map(|(i, seat)| seat.vars.iter().copied().find(|v| set(v)).map(|v| (i, v)))
        .collect();
    Found {
        servers: Vec::new(),
        keys,
        chatgpt: Plan::ChatGpt.auth_file().is_file(),
        grok: Plan::Grok.auth_file().is_file(),
    }
}

/// PATH only: asking npm where its global tools live takes a node start,
/// so that waits for an install, which runs off the draw thread.
pub(crate) fn tools_here() -> Tools {
    Tools {
        codex: on_path("codex"),
        grok: on_path("grok"),
        npm: on_path("npm").is_some(),
    }
}

fn on_path(tool: &str) -> Option<PathBuf> {
    std::env::split_paths(&std::env::var_os("PATH")?)
        .map(|dir| dir.join(tool))
        .find(|path| is_executable(path))
}

fn is_executable(path: &Path) -> bool {
    use std::os::unix::fs::PermissionsExt;
    path.metadata()
        .is_ok_and(|m| m.is_file() && m.permissions().mode() & 0o111 != 0)
}

/// A tool on PATH, or in npm's global bin when that is not on PATH.
fn find_tool(tool: &str) -> Option<PathBuf> {
    on_path(tool).or_else(|| {
        on_path("npm")?;
        let out = Command::new("npm")
            .args(["prefix", "-g"])
            .output_owned()
            .ok()?;
        let prefix = String::from_utf8(out.stdout).ok()?;
        let path = Path::new(prefix.trim()).join("bin").join(tool);
        is_executable(&path).then_some(path)
    })
}

/// Model ids in an OpenAI-style `/models` answer, in order.
pub(crate) fn model_ids(body: &str) -> Vec<String> {
    serde_json::from_str::<serde_json::Value>(body)
        .ok()
        .and_then(|v| {
            v.get("data")?.as_array().map(|rows| {
                rows.iter()
                    .filter_map(|row| row.get("id")?.as_str().map(str::to_string))
                    .collect()
            })
        })
        .unwrap_or_default()
}

/// Chat models only: provider lists also carry embedding, speech and image
/// models.
pub(crate) fn chat_models(body: &str) -> Vec<String> {
    const NOT_CHAT: &[&str] = &[
        "embed",
        "tts",
        "whisper",
        "dall-e",
        "image",
        "audio",
        "realtime",
        "moderation",
        "transcribe",
        "search",
    ];
    let mut ids: Vec<String> = model_ids(body)
        .into_iter()
        .filter(|id| !NOT_CHAT.iter().any(|word| id.contains(word)))
        .collect();
    ids.sort();
    ids
}

/// OpenRouter's catalog with the free models first.
pub(crate) fn openrouter_models(body: &str) -> Vec<String> {
    let mut ids = model_ids(body);
    ids.sort_by_key(|id| (!id.ends_with(":free"), id.clone()));
    ids
}

fn server_name(owned_by: &str, port: u16) -> &'static str {
    match owned_by {
        "library" => return "Ollama",
        "organization_owner" => return "LM Studio",
        "llamacpp" => return "llama.cpp",
        "vllm" => return "vLLM",
        "sglang" => return "SGLang",
        _ => {}
    }
    match port {
        11434 => "Ollama",
        1234 => "LM Studio",
        8080 => "llama.cpp",
        8000 => "vLLM",
        30000 => "SGLang",
        1337 => "Jan",
        _ => "a model server",
    }
}

fn http_get(url: &str, key: Option<&str>, patience: Duration) -> (u16, String) {
    let agent = ureq::AgentBuilder::new()
        .timeout_connect(patience.min(Duration::from_secs(5)))
        .timeout(patience)
        .build();
    let mut request = agent.get(url);
    if let Some(key) = key {
        request = request.set("Authorization", &format!("Bearer {key}"));
    }
    match request.call() {
        Ok(response) => {
            let status = response.status();
            (status, response.into_string().unwrap_or_default())
        }
        Err(ureq::Error::Status(status, response)) => {
            (status, response.into_string().unwrap_or_default())
        }
        Err(_) => (0, String::new()),
    }
}

fn scan_local() -> Vec<Server> {
    let probes: Vec<_> = LOCAL_PORTS
        .iter()
        .map(|&port| {
            std::thread::spawn(move || {
                let url = format!("http://127.0.0.1:{port}/v1");
                let (status, body) =
                    http_get(&format!("{url}/models"), None, Duration::from_secs(3));
                if !(200..300).contains(&status) {
                    return None;
                }
                let models = model_ids(&body);
                if models.is_empty() {
                    return None;
                }
                let owned_by = serde_json::from_str::<serde_json::Value>(&body)
                    .ok()
                    .and_then(|v| v["data"][0]["owned_by"].as_str().map(str::to_string))
                    .unwrap_or_default();
                Some(Server {
                    name: server_name(&owned_by, port).into(),
                    url,
                    models,
                })
            })
        })
        .collect();
    probes
        .into_iter()
        .filter_map(|probe| probe.join().ok().flatten())
        .collect()
}

fn api_url(provider: &Provider) -> String {
    if provider.driver == "openai-api"
        && let Ok(base) = std::env::var("OPENAI_BASE_URL")
        && !base.trim().is_empty()
    {
        return base.trim().trim_end_matches('/').to_string();
    }
    provider.url.to_string()
}

/// A key's check and, for a provider without a default model, its models.
fn check_key(driver: &str, url: &str, key: &str) -> (u16, Vec<String>) {
    let patience = Duration::from_secs(20);
    match driver {
        // OpenRouter lists models for anyone; its key endpoint is the check.
        "openrouter" => {
            let (status, _) = http_get(&format!("{url}/key"), Some(key), patience);
            if !(200..300).contains(&status) {
                return (status, Vec::new());
            }
            let (_, body) = http_get(&format!("{url}/models"), None, patience);
            (status, openrouter_models(&body))
        }
        _ => {
            let (status, body) = http_get(&format!("{url}/models"), Some(key), patience);
            let models = match driver {
                "openai-api" | "cerebras" => chat_models(&body),
                _ => Vec::new(),
            };
            (status, models)
        }
    }
}

fn server_models(url: &str, key: Option<&str>) -> Outcome {
    // OpenAI-compatible servers answer under /v1; try that first unless typed.
    let bases = if url.ends_with("/v1") {
        vec![url.to_string()]
    } else {
        vec![format!("{url}/v1"), url.to_string()]
    };
    let mut last = 0;
    for base in bases {
        let (status, body) = http_get(&format!("{base}/models"), key, Duration::from_secs(10));
        if (200..300).contains(&status) || status == 401 || status == 403 {
            return Outcome::Answer {
                status,
                models: model_ids(&body),
                url: base,
            };
        }
        last = status;
    }
    Outcome::Answer {
        status: last,
        models: Vec::new(),
        url: url.to_string(),
    }
}

fn run_login(plan: Plan, tool: &Path, login: &Login) -> Outcome {
    let child = Command::new(tool)
        .args(plan.login_args())
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn_owned();
    let mut child = match child {
        Ok(child) => child,
        Err(error) => {
            return Outcome::Finished {
                ok: false,
                log: error.to_string(),
                tool: None,
            };
        }
    };
    login.pid.store(child.id(), Ordering::Relaxed);
    let readers: Vec<_> = [
        child
            .stdout
            .take()
            .map(|s| Box::new(s) as Box<dyn Read + Send>),
        child
            .stderr
            .take()
            .map(|s| Box::new(s) as Box<dyn Read + Send>),
    ]
    .into_iter()
    .flatten()
    .map(|mut stream| {
        let log = Arc::clone(&login.log);
        std::thread::spawn(move || {
            let mut chunk = [0u8; 1024];
            while let Ok(n) = stream.read(&mut chunk) {
                if n == 0 {
                    break;
                }
                if let Ok(mut log) = log.lock() {
                    log.push_str(&String::from_utf8_lossy(&chunk[..n]));
                }
            }
        })
    })
    .collect();
    let status = child.wait();
    login.pid.store(0, Ordering::Relaxed);
    for reader in readers {
        let _ = reader.join();
    }
    let log = login.log.lock().map(|l| l.clone()).unwrap_or_default();
    Outcome::Finished {
        ok: status.is_ok_and(|s| s.success()) && plan.auth_file().is_file(),
        log,
        tool: None,
    }
}

fn run_install(plan: Plan) -> Outcome {
    match Command::new("npm")
        .args(["install", "-g", plan.package()])
        .output_owned()
    {
        Ok(out) => {
            let tool = find_tool(plan.tool());
            Outcome::Finished {
                ok: out.status.success(),
                log: format!(
                    "{}{}",
                    String::from_utf8_lossy(&out.stdout),
                    String::from_utf8_lossy(&out.stderr)
                ),
                tool,
            }
        }
        Err(error) => Outcome::Finished {
            ok: false,
            log: error.to_string(),
            tool: None,
        },
    }
}

impl Job {
    fn run(self) -> Outcome {
        match self {
            Job::Scan => Outcome::Scanned(scan_local()),
            Job::CheckKey { provider, key } => {
                let p = &PROVIDERS[provider];
                let url = api_url(p);
                let (status, models) = check_key(p.driver, &url, &key);
                Outcome::Answer {
                    status,
                    models,
                    url,
                }
            }
            Job::ServerModels { url, key } => server_models(&url, key.as_deref()),
            Job::SeatModels { driver, key } => {
                let url = PROVIDERS
                    .iter()
                    .find(|p| p.driver == driver)
                    .map(api_url)
                    .unwrap_or_default();
                let (status, models) = check_key(driver, &url, &key);
                Outcome::Answer {
                    status,
                    models,
                    url,
                }
            }
            Job::Login { plan, tool, login } => run_login(plan, &tool, &login),
            Job::Install { plan } => run_install(plan),
        }
    }
}

// --- the settings file --------------------------------------------------------

fn shell_quote(value: &str) -> String {
    format!("'{}'", value.replace('\'', "'\\''"))
}

/// Replace these names in the settings file and keep every other line, the
/// way `angelX setup --text` does. Written whole, owner-only, then renamed.
pub(crate) fn write_settings(path: &Path, pairs: &[(&str, String)]) -> std::io::Result<()> {
    use std::io::Write;
    use std::os::unix::fs::OpenOptionsExt;
    let dir = path.parent().unwrap_or(Path::new("."));
    std::fs::create_dir_all(dir)?;
    let mut text = match std::fs::read_to_string(path) {
        Ok(old) => old
            .lines()
            .filter(|line| {
                let line = line.trim_start();
                let line = line.strip_prefix("export").map_or(line, str::trim_start);
                !pairs
                    .iter()
                    .any(|(name, _)| line.strip_prefix(name).is_some_and(|r| r.starts_with('=')))
            })
            .map(|line| format!("{line}\n"))
            .collect::<String>(),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            "# angelX settings. `/setup` and `angelX setup` write this file; edit it freely.\n\
             # bin/angelX loads it on every interactive launch.\n"
                .to_string()
        }
        Err(error) => return Err(error),
    };
    for (name, value) in pairs {
        text.push_str(&format!("{name}={}\n", shell_quote(value)));
    }
    let tmp = dir.join(format!(".angel.env.{}", std::process::id()));
    let mut file = std::fs::OpenOptions::new()
        .write(true)
        .create(true)
        .truncate(true)
        .mode(0o600)
        .open(&tmp)?;
    file.write_all(text.as_bytes())?;
    file.sync_all()?;
    std::fs::rename(&tmp, path)
}

// --- the cockpit side ---------------------------------------------------------

/// `/setup`'s place in the cockpit: the open menu, its jobs' answers, and a
/// rebuilt model list waiting for the cockpit to be idle.
pub(crate) struct Setup {
    pub(crate) menu: Option<Menu>,
    pub(crate) hits: Vec<(Rect, Hit)>,
    #[cfg(feature = "glass")]
    pub(crate) scroll_motion: angel_glass::scroll::Motion,
    #[cfg(feature = "glass")]
    pub(crate) scroll_page: Option<angel_glass::scroll::Page>,
    tx: Sender<(u64, Outcome)>,
    rx: Receiver<(u64, Outcome)>,
    rebuild: Option<(Receiver<Bag>, String)>,
}

impl Default for Setup {
    fn default() -> Self {
        let (tx, rx) = mpsc::channel();
        Self {
            menu: None,
            hits: Vec::new(),
            #[cfg(feature = "glass")]
            scroll_motion: angel_glass::scroll::Motion::default(),
            #[cfg(feature = "glass")]
            scroll_page: None,
            tx,
            rx,
            rebuild: None,
        }
    }
}

impl Setup {
    pub(crate) fn open(&self) -> bool {
        self.menu.is_some()
    }

    pub(crate) fn wants_tick(&self) -> bool {
        self.menu.is_some() || self.rebuild.is_some() || self.scroll_is_moving()
    }

    pub(crate) fn scroll_is_moving(&self) -> bool {
        #[cfg(feature = "glass")]
        { self.scroll_motion.moving() }
        #[cfg(not(feature = "glass"))]
        { false }
    }
}

impl App {
    /// The route in hand as words, or `None` on the practice floor.
    fn setup_current_route(&self) -> Option<String> {
        let choice = self
            .bag
            .route_choices()
            .iter()
            .find(|choice| choice.selected)
            .cloned()?;
        (choice.agent != "practice").then(|| format!("{} ▸ {}", choice.agent, choice.model))
    }

    pub(crate) fn open_setup(&mut self) {
        if self.setup.menu.is_some() {
            return;
        }
        self.agent_menu = None;
        let now = Instant::now();
        let (found, tools) = if cfg!(test) {
            (Found::default(), Tools::default())
        } else {
            (found_here(), tools_here())
        };
        let mut menu = Menu::new(found, tools, now);
        menu.current = self.setup_current_route();
        let guide = crate::platform::runtime_paths::cockpit_dir().join(GUIDE_SHEET);
        menu.guide = guide.is_file().then_some(guide);
        crate::ui::views::setup_view::warm_plate();
        let effect = menu.scan();
        self.setup.menu = Some(menu);
        #[cfg(feature = "glass")]
        if crate::ui::glass::active() {
            if self.visual_motion == crate::ui::viz::lifecycle_viz::MotionMode::Full {
                self.setup.scroll_motion.target(true, now);
            } else { self.setup.scroll_motion.snap(true, now); }
        }
        self.setup_apply(effect);
    }

    fn close_setup(&mut self) {
        #[cfg(feature = "glass")]
        if crate::ui::glass::active() {
            if self.visual_motion == crate::ui::viz::lifecycle_viz::MotionMode::Full {
                self.setup.scroll_motion.target(false, Instant::now());
            } else {
                self.setup.scroll_motion.snap(false, Instant::now());
                self.setup.scroll_page = None;
            }
        }
        if let Some(mut menu) = self.setup.menu.take() {
            menu.cancel();
        }
        self.setup.hits.clear();
        // Asked and declined: the first launch does not ask again.
        let path = settings_path();
        if !cfg!(test) && !path.exists() {
            let _ = write_settings(&path, &[("ANGEL_SETUP_SKIPPED", "1".into())]);
        }
    }

    pub(crate) fn setup_key(&mut self, key: KeyEvent) {
        let Some(menu) = self.setup.menu.as_mut() else {
            return;
        };
        let effect = menu.key(key, Instant::now());
        self.setup_apply(effect);
    }

    pub(crate) fn setup_paste(&mut self, text: &str) {
        if let Some(menu) = self.setup.menu.as_mut() {
            menu.paste(text);
        }
    }

    pub(crate) fn setup_mouse(&mut self, ev: MouseEvent) {
        // The native page bends and travels independently of the cell grid.
        // Ignore unreleased/suppressed paper rather than activating hidden rows.
        #[cfg(feature = "glass")]
        let ev = if let Some(page) = self.setup.scroll_page.as_ref() {
            let Some((column, row)) = page.hit(ev.column, ev.row) else {
                return;
            };
            MouseEvent { column, row, ..ev }
        } else {
            ev
        };
        let Some(menu) = self.setup.menu.as_mut() else {
            return;
        };
        let effect = match ev.kind {
            MouseEventKind::ScrollUp => {
                menu.wheel(-1);
                Effect::Stay
            }
            MouseEventKind::ScrollDown => {
                menu.wheel(1);
                Effect::Stay
            }
            MouseEventKind::Down(MouseButton::Left) => {
                match self
                    .setup
                    .hits
                    .iter()
                    .find(|(rect, _)| crate::ui::mouse::point_in(*rect, ev.column, ev.row))
                {
                    Some((_, hit)) => menu.click(*hit, Instant::now()),
                    None => Effect::Stay,
                }
            }
            _ => Effect::Stay,
        };
        self.setup_apply(effect);
    }

    fn setup_apply(&mut self, effect: Effect) {
        match effect {
            Effect::Stay => {}
            Effect::Close => self.close_setup(),
            Effect::Run(serial) => {
                let Some(job) = self.setup.menu.as_mut().and_then(Menu::take_job) else {
                    return;
                };
                // Tests answer jobs by hand; nothing leaves the process.
                if cfg!(test) {
                    return;
                }
                let tx = self.setup.tx.clone();
                let _ = std::thread::Builder::new()
                    .name("setup-job".into())
                    .spawn(move || {
                        let _ = tx.send((serial, job.run()));
                    });
            }
            Effect::Open(url) => {
                let _ = crate::ui::media::open_target(&url);
            }
            Effect::Save(saving) => self.setup_save(saving),
        }
    }

    fn setup_save(&mut self, saving: Saving) {
        let written = if cfg!(test) {
            Ok(())
        } else {
            write_settings(&settings_path(), &saving.pairs)
        };
        let now = Instant::now();
        match written {
            Ok(()) => {
                if !cfg!(test) {
                    for (name, value) in &saving.pairs {
                        // SAFETY: the cockpit sets its own env from the UI
                        // thread, as main does at startup; the rebuild below
                        // reads it.
                        unsafe { std::env::set_var(name, value) };
                    }
                    // The new choice outranks the last /model pick.
                    let _ = crate::platform::route_preferences::clear();
                    let (tx, rx) = mpsc::channel();
                    let _ = std::thread::Builder::new()
                        .name("setup-routes".into())
                        .spawn(move || {
                            let startup = crate::agent::codex_startup::CodexStartup::load();
                            let _ = tx.send(Bag::standard_in_run(startup));
                        });
                    self.setup.rebuild = Some((rx, saving.name.clone()));
                }
                if let Some(menu) = self.setup.menu.as_mut() {
                    menu.show_done(saving.name, saving.tip, now);
                }
            }
            Err(error) => {
                if let Some(menu) = self.setup.menu.as_mut() {
                    menu.set_note(format!(
                        "Couldn't save {}: {error}",
                        settings_path().display()
                    ));
                }
            }
        }
    }

    /// Answers from the menu's jobs, and the rebuilt model list once nothing
    /// is using the old one.
    pub(crate) fn advance_setup(&mut self) {
        while let Ok((serial, outcome)) = self.setup.rx.try_recv() {
            let Some(menu) = self.setup.menu.as_mut() else {
                continue;
            };
            let effect = menu.finish(serial, outcome, Instant::now());
            self.setup_apply(effect);
        }
        let idle = self.thinking.is_none()
            && self.pending_turn.is_none()
            && self.bg_job.is_none()
            && !self.launch_is_pending();
        if !idle {
            return;
        }
        let Some((rx, name)) = &self.setup.rebuild else {
            return;
        };
        match rx.try_recv() {
            Ok(mut bag) => {
                let name = name.clone();
                self.setup.rebuild = None;
                bag.generation = self.bag.generation.wrapping_add(1);
                self.bag = bag;
                self.launch_route = None;
                self.agent_menu = None;
                self.bag.warm_background();
                let route = self.setup_current_route();
                if let Some(menu) = self.setup.menu.as_mut() {
                    menu.current = route.clone();
                }
                self.system_msg(match route {
                    Some(route) => format!("setup · connected {name} · in hand: {route}"),
                    None => format!(
                        "setup · saved {name}; it hasn't answered yet, and takes over as soon as it does"
                    ),
                });
            }
            Err(TryRecvError::Empty) => {}
            Err(TryRecvError::Disconnected) => self.setup.rebuild = None,
        }
    }
}

#[cfg(test)]
#[path = "../../../tests/cockpit/app/setup_menu__tests.rs"]
mod tests;
