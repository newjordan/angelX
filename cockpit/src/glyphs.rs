//! Tiny fixed-width glyph vocabulary for cockpit state.
//!
//! These are static terminal marks, not a renderer: callers get borrowed text
//! and can compose it into ratatui spans without allocation.

use std::sync::OnceLock;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Token {
    ascii: &'static str,
    rich: &'static str,
    label: &'static str,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Glyph {
    // State
    #[allow(dead_code)]
    Active,
    Idle,
    #[allow(dead_code)]
    Online,
    #[allow(dead_code)]
    Offline,
    Thinking,
    Loading,
    #[allow(dead_code)]
    Busy,
    #[allow(dead_code)]
    Queued,
    #[allow(dead_code)]
    Paused,
    #[allow(dead_code)]
    Blocked,
    #[allow(dead_code)]
    Error,
    #[allow(dead_code)]
    Ready,
    #[allow(dead_code)]
    Stale,
    // Transport
    Transfer,
    #[allow(dead_code)]
    TransferIn,
    #[allow(dead_code)]
    TransferOut,
    #[allow(dead_code)]
    Sync,
    #[allow(dead_code)]
    Bridge,
    #[allow(dead_code)]
    Remote,
    #[allow(dead_code)]
    Local,
    // Surfaces
    Agent,
    Shell,
    Artifacts,
    #[allow(dead_code)]
    Surface,
    #[allow(dead_code)]
    Scene,
    #[allow(dead_code)]
    Avatar,
    #[allow(dead_code)]
    Transcript,
    #[allow(dead_code)]
    Terminal,
    #[allow(dead_code)]
    Carousel,
    // Actions
    Approval,
    Send,
    Back,
    Switch,
    Tool,
    Result,
    #[allow(dead_code)]
    Check,
    #[allow(dead_code)]
    Warning,
    Cpu,
    Gpu,
    #[allow(dead_code)]
    Memory,
    #[allow(dead_code)]
    Network,
    #[allow(dead_code)]
    Disk,
    #[allow(dead_code)]
    Search,
    #[allow(dead_code)]
    Mail,
    // Agent work modes
    #[allow(dead_code)]
    Plan,
    #[allow(dead_code)]
    Code,
    #[allow(dead_code)]
    Review,
    #[allow(dead_code)]
    Patch,
    #[allow(dead_code)]
    Brain,
    #[allow(dead_code)]
    Spark,
    #[allow(dead_code)]
    Scout,
    #[allow(dead_code)]
    Planner,
    #[allow(dead_code)]
    Coder,
    #[allow(dead_code)]
    Reviewer,
    #[allow(dead_code)]
    Runner,
    #[allow(dead_code)]
    Specialist,
    #[allow(dead_code)]
    Moa,
    // Security/session
    #[allow(dead_code)]
    Lock,
    #[allow(dead_code)]
    Key,
    #[allow(dead_code)]
    Session,
    #[allow(dead_code)]
    Home,
}

impl Glyph {
    #[cfg(test)]
    pub const ALL: [Self; 61] = [
        Self::Active,
        Self::Idle,
        Self::Online,
        Self::Offline,
        Self::Thinking,
        Self::Loading,
        Self::Busy,
        Self::Queued,
        Self::Paused,
        Self::Blocked,
        Self::Error,
        Self::Ready,
        Self::Stale,
        Self::Transfer,
        Self::TransferIn,
        Self::TransferOut,
        Self::Sync,
        Self::Bridge,
        Self::Remote,
        Self::Local,
        Self::Agent,
        Self::Shell,
        Self::Artifacts,
        Self::Surface,
        Self::Scene,
        Self::Avatar,
        Self::Transcript,
        Self::Terminal,
        Self::Carousel,
        Self::Approval,
        Self::Send,
        Self::Back,
        Self::Switch,
        Self::Tool,
        Self::Result,
        Self::Check,
        Self::Warning,
        Self::Cpu,
        Self::Gpu,
        Self::Memory,
        Self::Network,
        Self::Disk,
        Self::Search,
        Self::Mail,
        Self::Plan,
        Self::Code,
        Self::Review,
        Self::Patch,
        Self::Brain,
        Self::Spark,
        Self::Scout,
        Self::Planner,
        Self::Coder,
        Self::Reviewer,
        Self::Runner,
        Self::Specialist,
        Self::Moa,
        Self::Lock,
        Self::Key,
        Self::Session,
        Self::Home,
    ];

    /// Width-stable default token. Keep this ASCII and exactly two bytes/cells;
    /// richer terminal marks are opt-in through `token()`.
    pub const fn mark(self) -> &'static str {
        self.token_set().ascii
    }

    pub fn token(self) -> &'static str {
        if rich_glyphs_enabled() {
            return self.rich_mark();
        }
        self.mark()
    }

    fn rich_mark(self) -> &'static str {
        self.token_set().rich
    }

    pub const fn label(self) -> &'static str {
        self.token_set().label
    }

    #[allow(dead_code)]
    pub fn named(name: &str) -> Option<Self> {
        let name = trim_symbol_name(name.as_bytes());
        if eq_ignore_ascii_case(name, b"active") || eq_ignore_ascii_case(name, b"run") {
            Some(Self::Active)
        } else if eq_ignore_ascii_case(name, b"online") || eq_ignore_ascii_case(name, b"live") {
            Some(Self::Online)
        } else if eq_ignore_ascii_case(name, b"offline") || eq_ignore_ascii_case(name, b"down") {
            Some(Self::Offline)
        } else if eq_ignore_ascii_case(name, b"idle") || eq_ignore_ascii_case(name, b"sleep") {
            Some(Self::Idle)
        } else if eq_ignore_ascii_case(name, b"thinking")
            || eq_ignore_ascii_case(name, b"think")
            || eq_ignore_ascii_case(name, b"reasoning")
        {
            Some(Self::Thinking)
        } else if eq_ignore_ascii_case(name, b"loading") || eq_ignore_ascii_case(name, b"load") {
            Some(Self::Loading)
        } else if eq_ignore_ascii_case(name, b"busy")
            || eq_ignore_ascii_case(name, b"work")
            || eq_ignore_ascii_case(name, b"working")
        {
            Some(Self::Busy)
        } else if eq_ignore_ascii_case(name, b"queued")
            || eq_ignore_ascii_case(name, b"queue")
            || eq_ignore_ascii_case(name, b"wait")
            || eq_ignore_ascii_case(name, b"waiting")
        {
            Some(Self::Queued)
        } else if eq_ignore_ascii_case(name, b"paused")
            || eq_ignore_ascii_case(name, b"pause")
            || eq_ignore_ascii_case(name, b"hold")
        {
            Some(Self::Paused)
        } else if eq_ignore_ascii_case(name, b"blocked")
            || eq_ignore_ascii_case(name, b"block")
            || eq_ignore_ascii_case(name, b"stuck")
        {
            Some(Self::Blocked)
        } else if eq_ignore_ascii_case(name, b"error")
            || eq_ignore_ascii_case(name, b"err")
            || eq_ignore_ascii_case(name, b"fail")
            || eq_ignore_ascii_case(name, b"failed")
        {
            Some(Self::Error)
        } else if eq_ignore_ascii_case(name, b"ready") {
            Some(Self::Ready)
        } else if eq_ignore_ascii_case(name, b"stale") || eq_ignore_ascii_case(name, b"old") {
            Some(Self::Stale)
        } else if eq_ignore_ascii_case(name, b"transfer") || eq_ignore_ascii_case(name, b"xfer") {
            Some(Self::Transfer)
        } else if eq_ignore_ascii_case(name, b"transfer-in")
            || eq_ignore_ascii_case(name, b"xfer-in")
            || eq_ignore_ascii_case(name, b"in")
            || eq_ignore_ascii_case(name, b"recv")
            || eq_ignore_ascii_case(name, b"receive")
            || eq_ignore_ascii_case(name, b"download")
        {
            Some(Self::TransferIn)
        } else if eq_ignore_ascii_case(name, b"transfer-out")
            || eq_ignore_ascii_case(name, b"xfer-out")
            || eq_ignore_ascii_case(name, b"out")
            || eq_ignore_ascii_case(name, b"send-out")
            || eq_ignore_ascii_case(name, b"upload")
        {
            Some(Self::TransferOut)
        } else if eq_ignore_ascii_case(name, b"sync") {
            Some(Self::Sync)
        } else if eq_ignore_ascii_case(name, b"bridge") || eq_ignore_ascii_case(name, b"route") {
            Some(Self::Bridge)
        } else if eq_ignore_ascii_case(name, b"remote") {
            Some(Self::Remote)
        } else if eq_ignore_ascii_case(name, b"local") {
            Some(Self::Local)
        } else if eq_ignore_ascii_case(name, b"agent") {
            Some(Self::Agent)
        } else if eq_ignore_ascii_case(name, b"shell") || eq_ignore_ascii_case(name, b"sh") {
            Some(Self::Shell)
        } else if eq_ignore_ascii_case(name, b"artifacts")
            || eq_ignore_ascii_case(name, b"artifact")
        {
            Some(Self::Artifacts)
        } else if eq_ignore_ascii_case(name, b"surface") {
            Some(Self::Surface)
        } else if eq_ignore_ascii_case(name, b"scene") {
            Some(Self::Scene)
        } else if eq_ignore_ascii_case(name, b"avatar") || eq_ignore_ascii_case(name, b"portrait") {
            Some(Self::Avatar)
        } else if eq_ignore_ascii_case(name, b"transcript") || eq_ignore_ascii_case(name, b"log") {
            Some(Self::Transcript)
        } else if eq_ignore_ascii_case(name, b"terminal") || eq_ignore_ascii_case(name, b"tty") {
            Some(Self::Terminal)
        } else if eq_ignore_ascii_case(name, b"carousel") || eq_ignore_ascii_case(name, b"media") {
            Some(Self::Carousel)
        } else if eq_ignore_ascii_case(name, b"approval") || eq_ignore_ascii_case(name, b"approve")
        {
            Some(Self::Approval)
        } else if eq_ignore_ascii_case(name, b"send") || eq_ignore_ascii_case(name, b"enter") {
            Some(Self::Send)
        } else if eq_ignore_ascii_case(name, b"back") || eq_ignore_ascii_case(name, b"return") {
            Some(Self::Back)
        } else if eq_ignore_ascii_case(name, b"switch") || eq_ignore_ascii_case(name, b"tab") {
            Some(Self::Switch)
        } else if eq_ignore_ascii_case(name, b"tool") || eq_ignore_ascii_case(name, b"tools") {
            Some(Self::Tool)
        } else if eq_ignore_ascii_case(name, b"result")
            || eq_ignore_ascii_case(name, b"done")
            || eq_ignore_ascii_case(name, b"ready")
        {
            Some(Self::Result)
        } else if eq_ignore_ascii_case(name, b"check") || eq_ignore_ascii_case(name, b"ok") {
            Some(Self::Check)
        } else if eq_ignore_ascii_case(name, b"warning") || eq_ignore_ascii_case(name, b"warn") {
            Some(Self::Warning)
        } else if eq_ignore_ascii_case(name, b"cpu") {
            Some(Self::Cpu)
        } else if eq_ignore_ascii_case(name, b"gpu") {
            Some(Self::Gpu)
        } else if eq_ignore_ascii_case(name, b"memory") || eq_ignore_ascii_case(name, b"mem") {
            Some(Self::Memory)
        } else if eq_ignore_ascii_case(name, b"network") || eq_ignore_ascii_case(name, b"net") {
            Some(Self::Network)
        } else if eq_ignore_ascii_case(name, b"disk") || eq_ignore_ascii_case(name, b"store") {
            Some(Self::Disk)
        } else if eq_ignore_ascii_case(name, b"search") || eq_ignore_ascii_case(name, b"find") {
            Some(Self::Search)
        } else if eq_ignore_ascii_case(name, b"mail") || eq_ignore_ascii_case(name, b"inbox") {
            Some(Self::Mail)
        } else if eq_ignore_ascii_case(name, b"plan") || eq_ignore_ascii_case(name, b"todo") {
            Some(Self::Plan)
        } else if eq_ignore_ascii_case(name, b"code") || eq_ignore_ascii_case(name, b"edit") {
            Some(Self::Code)
        } else if eq_ignore_ascii_case(name, b"review") || eq_ignore_ascii_case(name, b"audit") {
            Some(Self::Review)
        } else if eq_ignore_ascii_case(name, b"patch") || eq_ignore_ascii_case(name, b"diff") {
            Some(Self::Patch)
        } else if eq_ignore_ascii_case(name, b"brain")
            || eq_ignore_ascii_case(name, b"model")
            || eq_ignore_ascii_case(name, b"llm")
        {
            Some(Self::Brain)
        } else if eq_ignore_ascii_case(name, b"spark")
            || eq_ignore_ascii_case(name, b"idea")
            || eq_ignore_ascii_case(name, b"insight")
        {
            Some(Self::Spark)
        } else if eq_ignore_ascii_case(name, b"scout") {
            Some(Self::Scout)
        } else if eq_ignore_ascii_case(name, b"planner") {
            Some(Self::Planner)
        } else if eq_ignore_ascii_case(name, b"coder") {
            Some(Self::Coder)
        } else if eq_ignore_ascii_case(name, b"reviewer") {
            Some(Self::Reviewer)
        } else if eq_ignore_ascii_case(name, b"runner") || eq_ignore_ascii_case(name, b"exec") {
            Some(Self::Runner)
        } else if eq_ignore_ascii_case(name, b"specialist") || eq_ignore_ascii_case(name, b"spec") {
            Some(Self::Specialist)
        } else if eq_ignore_ascii_case(name, b"moa") || eq_ignore_ascii_case(name, b"fleet") {
            Some(Self::Moa)
        } else if eq_ignore_ascii_case(name, b"lock") || eq_ignore_ascii_case(name, b"secure") {
            Some(Self::Lock)
        } else if eq_ignore_ascii_case(name, b"key") || eq_ignore_ascii_case(name, b"auth") {
            Some(Self::Key)
        } else if eq_ignore_ascii_case(name, b"session")
            || eq_ignore_ascii_case(name, b"thread")
            || eq_ignore_ascii_case(name, b"chat")
        {
            Some(Self::Session)
        } else if eq_ignore_ascii_case(name, b"home")
            || eq_ignore_ascii_case(name, b"root")
            || eq_ignore_ascii_case(name, b"workspace")
        {
            Some(Self::Home)
        } else {
            None
        }
    }

    const fn token_set(self) -> Token {
        match self {
            Self::Active => Token {
                ascii: "*>",
                rich: "● ",
                label: "active",
            },
            Self::Idle => Token {
                ascii: "..",
                rich: "○ ",
                label: "idle",
            },
            Self::Online => Token {
                ascii: "ON",
                rich: "◉ ",
                label: "online",
            },
            Self::Offline => Token {
                ascii: "NO",
                rich: "◌ ",
                label: "offline",
            },
            Self::Thinking => Token {
                ascii: "??",
                rich: "◆ ",
                label: "thinking",
            },
            Self::Loading => Token {
                ascii: "-|",
                rich: "◌ ",
                label: "loading",
            },
            Self::Busy => Token {
                ascii: "~~",
                rich: "⟳ ",
                label: "busy",
            },
            Self::Queued => Token {
                ascii: "Q>",
                rich: "… ",
                label: "queued",
            },
            Self::Paused => Token {
                ascii: "||",
                rich: "Ⅱ ",
                label: "paused",
            },
            Self::Blocked => Token {
                ascii: "B!",
                rich: "■ ",
                label: "blocked",
            },
            Self::Error => Token {
                ascii: "E!",
                rich: "× ",
                label: "error",
            },
            Self::Ready => Token {
                ascii: "RD",
                rich: "✓ ",
                label: "ready",
            },
            Self::Stale => Token {
                ascii: "ST",
                rich: "◌ ",
                label: "stale",
            },
            Self::Transfer => Token {
                ascii: "<>",
                rich: "⇄ ",
                label: "transfer",
            },
            Self::TransferIn => Token {
                ascii: "<-",
                rich: "⇠ ",
                label: "transfer-in",
            },
            Self::TransferOut => Token {
                ascii: "->",
                rich: "⇢ ",
                label: "transfer-out",
            },
            Self::Sync => Token {
                ascii: "SY",
                rich: "⟳ ",
                label: "sync",
            },
            Self::Bridge => Token {
                ascii: "BR",
                rich: "⌁ ",
                label: "bridge",
            },
            Self::Remote => Token {
                ascii: "RM",
                rich: "↗ ",
                label: "remote",
            },
            Self::Local => Token {
                ascii: "LC",
                rich: "⌂ ",
                label: "local",
            },
            Self::Agent => Token {
                ascii: "A>",
                rich: "✦ ",
                label: "agent",
            },
            Self::Shell => Token {
                ascii: "$>",
                rich: "$ ",
                label: "shell",
            },
            Self::Artifacts => Token {
                ascii: "[]",
                rich: "▣ ",
                label: "artifacts",
            },
            Self::Surface => Token {
                ascii: "Sf",
                rich: "▧ ",
                label: "surface",
            },
            Self::Scene => Token {
                ascii: "Sc",
                rich: "◫ ",
                label: "scene",
            },
            Self::Avatar => Token {
                ascii: "Av",
                rich: "◉ ",
                label: "avatar",
            },
            Self::Transcript => Token {
                ascii: "Tr",
                rich: "≡ ",
                label: "transcript",
            },
            Self::Terminal => Token {
                ascii: "Ty",
                rich: "$ ",
                label: "terminal",
            },
            Self::Carousel => Token {
                ascii: "Ca",
                rich: "▣ ",
                label: "carousel",
            },
            Self::Approval => Token {
                ascii: "!!",
                rich: "! ",
                label: "approval",
            },
            Self::Send => Token {
                ascii: ">>",
                rich: "↵ ",
                label: "send",
            },
            Self::Back => Token {
                ascii: "<<",
                rich: "← ",
                label: "back",
            },
            Self::Switch => Token {
                ascii: "T>",
                rich: "⇥ ",
                label: "switch",
            },
            Self::Tool => Token {
                ascii: "T:",
                rich: "⚙ ",
                label: "tool",
            },
            Self::Result => Token {
                ascii: "R:",
                rich: "✓ ",
                label: "result",
            },
            Self::Check => Token {
                ascii: "OK",
                rich: "✓ ",
                label: "ok",
            },
            Self::Warning => Token {
                ascii: "W!",
                rich: "⚠ ",
                label: "warning",
            },
            Self::Cpu => Token {
                ascii: "C>",
                rich: "c ",
                label: "cpu",
            },
            Self::Gpu => Token {
                ascii: "G>",
                rich: "g ",
                label: "gpu",
            },
            Self::Memory => Token {
                ascii: "M>",
                rich: "m ",
                label: "memory",
            },
            Self::Network => Token {
                ascii: "N>",
                rich: "⌁ ",
                label: "network",
            },
            Self::Disk => Token {
                ascii: "D>",
                rich: "▤ ",
                label: "disk",
            },
            Self::Search => Token {
                ascii: "S>",
                rich: "⌕ ",
                label: "search",
            },
            Self::Mail => Token {
                ascii: "@>",
                rich: "✉ ",
                label: "mail",
            },
            Self::Plan => Token {
                ascii: "P>",
                rich: "≡ ",
                label: "plan",
            },
            Self::Code => Token {
                ascii: "{}",
                rich: "λ ",
                label: "code",
            },
            Self::Review => Token {
                ascii: "R?",
                rich: "◇ ",
                label: "review",
            },
            Self::Patch => Token {
                ascii: "P+",
                rich: "✚ ",
                label: "patch",
            },
            Self::Brain => Token {
                ascii: "AI",
                rich: "∴ ",
                label: "brain",
            },
            Self::Spark => Token {
                ascii: "SP",
                rich: "✦ ",
                label: "spark",
            },
            Self::Scout => Token {
                ascii: "So",
                rich: "⌕ ",
                label: "scout",
            },
            Self::Planner => Token {
                ascii: "Pl",
                rich: "≡ ",
                label: "planner",
            },
            Self::Coder => Token {
                ascii: "Co",
                rich: "λ ",
                label: "coder",
            },
            Self::Reviewer => Token {
                ascii: "Rv",
                rich: "◇ ",
                label: "reviewer",
            },
            Self::Runner => Token {
                ascii: "Rn",
                rich: "▶ ",
                label: "runner",
            },
            Self::Specialist => Token {
                ascii: "S*",
                rich: "◈ ",
                label: "specialist",
            },
            Self::Moa => Token {
                ascii: "Sw",
                rich: "▦ ",
                label: "moa",
            },
            Self::Lock => Token {
                ascii: "LK",
                rich: "◆ ",
                label: "lock",
            },
            Self::Key => Token {
                ascii: "KY",
                rich: "⌁ ",
                label: "key",
            },
            Self::Session => Token {
                ascii: "S#",
                rich: "▦ ",
                label: "session",
            },
            Self::Home => Token {
                ascii: "~/",
                rich: "⌂ ",
                label: "home",
            },
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AgentBadge {
    Turbo,
    Atlas,
    Sparky,
    Apollo,
    #[allow(dead_code)]
    Casper,
    #[allow(dead_code)]
    Strixus,
    Unknown,
}

impl AgentBadge {
    #[cfg(test)]
    pub const ALL: [Self; 7] = [
        Self::Turbo,
        Self::Atlas,
        Self::Sparky,
        Self::Apollo,
        Self::Casper,
        Self::Strixus,
        Self::Unknown,
    ];

    pub const fn mark(self) -> &'static str {
        self.token_set().ascii
    }

    pub fn token(self) -> &'static str {
        if rich_glyphs_enabled() {
            return self.rich_mark();
        }
        self.mark()
    }

    fn rich_mark(self) -> &'static str {
        self.token_set().rich
    }

    #[allow(dead_code)]
    pub const fn label(self) -> &'static str {
        self.token_set().label
    }

    #[allow(dead_code)]
    pub fn named(name: &str) -> Option<Self> {
        let name = trim_symbol_name(name.as_bytes());
        if eq_ignore_ascii_case(name, b"turbo") {
            Some(Self::Turbo)
        } else if eq_ignore_ascii_case(name, b"atlas") {
            Some(Self::Atlas)
        } else if eq_ignore_ascii_case(name, b"sparky")
            || eq_ignore_ascii_case(name, b"spark")
            || eq_ignore_ascii_case(name, b"spark-r1")
            || eq_ignore_ascii_case(name, b"spark-v4")
        {
            Some(Self::Sparky)
        } else if eq_ignore_ascii_case(name, b"apollo") || eq_ignore_ascii_case(name, b"dice") {
            Some(Self::Apollo)
        } else if eq_ignore_ascii_case(name, b"casper")
            || eq_ignore_ascii_case(name, b"ghost")
            || eq_ignore_ascii_case(name, b"ghost-protocol")
        {
            Some(Self::Casper)
        } else if eq_ignore_ascii_case(name, b"strixus") || eq_ignore_ascii_case(name, b"scout") {
            Some(Self::Strixus)
        } else {
            None
        }
    }

    const fn token_set(self) -> Token {
        match self {
            Self::Turbo => Token {
                ascii: "Tu",
                rich: "» ",
                label: "turbo",
            },
            Self::Atlas => Token {
                ascii: "At",
                rich: "▲ ",
                label: "atlas",
            },
            Self::Sparky => Token {
                ascii: "Sp",
                rich: "✦ ",
                label: "sparky",
            },
            Self::Apollo => Token {
                ascii: "Ap",
                rich: "◈ ",
                label: "apollo",
            },
            Self::Casper => Token {
                ascii: "Cs",
                rich: "◇ ",
                label: "casper",
            },
            Self::Strixus => Token {
                ascii: "St",
                rich: "△ ",
                label: "strixus",
            },
            Self::Unknown => Token {
                ascii: "A>",
                rich: "· ",
                label: "agent",
            },
        }
    }
}

#[allow(dead_code)]
pub const fn activity(thinking: bool) -> Glyph {
    if thinking {
        Glyph::Thinking
    } else {
        Glyph::Idle
    }
}

#[allow(dead_code)]
pub fn token(name: &str) -> Option<&'static str> {
    if let Some(glyph) = Glyph::named(name) {
        return Some(glyph.token());
    }
    AgentBadge::named(name).map(AgentBadge::token)
}

#[allow(dead_code)]
pub fn label(name: &str) -> Option<&'static str> {
    if let Some(glyph) = Glyph::named(name) {
        return Some(glyph.label());
    }
    AgentBadge::named(name).map(AgentBadge::label)
}

#[cfg(test)]
fn agent_badge(label: &str) -> AgentBadge {
    let label = trim_ascii(label.as_bytes());
    if eq_ignore_ascii_case(label, b"turbo") {
        AgentBadge::Turbo
    } else if eq_ignore_ascii_case(label, b"atlas") {
        AgentBadge::Atlas
    } else if eq_ignore_ascii_case(label, b"sparky")
        || eq_ignore_ascii_case(label, b"spark")
        || eq_ignore_ascii_case(label, b"spark-r1")
        || eq_ignore_ascii_case(label, b"spark-v4")
    {
        AgentBadge::Sparky
    } else if eq_ignore_ascii_case(label, b"apollo") || eq_ignore_ascii_case(label, b"dice") {
        AgentBadge::Apollo
    } else if eq_ignore_ascii_case(label, b"casper")
        || eq_ignore_ascii_case(label, b"ghost")
        || eq_ignore_ascii_case(label, b"ghost-protocol")
    {
        AgentBadge::Casper
    } else if eq_ignore_ascii_case(label, b"strixus") || eq_ignore_ascii_case(label, b"scout") {
        AgentBadge::Strixus
    } else {
        AgentBadge::Unknown
    }
}

pub fn media_prefix(kind: &str) -> &'static str {
    if rich_glyphs_enabled() {
        return match kind.as_bytes() {
            b"image" | b"img" => "▧  img   ",
            b"video" | b"reel" => "▶  reel  ",
            b"link" => "↗  link  ",
            b"graph" => "⌁  graph ",
            b"resource" | b"res" => "◫  res   ",
            _ => "↗  link  ",
        };
    }
    match kind.as_bytes() {
        b"image" | b"img" => "I> img   ",
        b"video" | b"reel" => "V> reel  ",
        b"link" => "L> link  ",
        b"graph" => "G> graph ",
        b"resource" | b"res" => "R> res   ",
        _ => "L> link  ",
    }
}

#[allow(dead_code)]
const fn trim_ascii(mut bytes: &[u8]) -> &[u8] {
    while let [first, rest @ ..] = bytes {
        if !first.is_ascii_whitespace() {
            break;
        }
        bytes = rest;
    }
    while let [rest @ .., last] = bytes {
        if !last.is_ascii_whitespace() {
            break;
        }
        bytes = rest;
    }
    bytes
}

#[allow(dead_code)]
const fn trim_colons(bytes: &[u8]) -> &[u8] {
    match bytes {
        [b':', inner @ .., b':'] => inner,
        _ => bytes,
    }
}

#[allow(dead_code)]
const fn trim_agent_prefix(bytes: &[u8]) -> &[u8] {
    match bytes {
        [
            b'a' | b'A',
            b'g' | b'G',
            b'e' | b'E',
            b'n' | b'N',
            b't' | b'T',
            b'.',
            rest @ ..,
        ] => rest,
        _ => bytes,
    }
}

#[allow(dead_code)]
const fn trim_symbol_name(bytes: &[u8]) -> &[u8] {
    trim_agent_prefix(trim_colons(trim_ascii(bytes)))
}

#[allow(dead_code)]
fn eq_ignore_ascii_case(left: &[u8], right: &[u8]) -> bool {
    left.len() == right.len()
        && left
            .iter()
            .zip(right.iter())
            .all(|(left, right)| left.eq_ignore_ascii_case(right))
}

fn rich_glyphs_enabled() -> bool {
    static ENABLED: OnceLock<bool> = OnceLock::new();
    *ENABLED.get_or_init(|| {
        std::env::var("ANGEL_GLYPHS")
            .map(|mode| mode.eq_ignore_ascii_case("rich"))
            .unwrap_or(false)
    })
}

/// 7-bit chrome: box drawing, ellipses, bullets, and world panes.
pub(crate) fn ascii_chrome_enabled() -> bool {
    static ENABLED: OnceLock<bool> = OnceLock::new();
    *ENABLED.get_or_init(|| {
        std::env::var("ANGEL_GLYPHS")
            .map(|mode| mode.eq_ignore_ascii_case("ascii"))
            .unwrap_or(false)
    })
}

pub(crate) fn chrome_ellipsis() -> &'static str {
    if ascii_chrome_enabled() { "..." } else { "…" }
}

pub(crate) fn chrome_bullet() -> &'static str {
    if ascii_chrome_enabled() { "*" } else { "·" }
}

#[allow(dead_code)]
pub(crate) fn chrome_return() -> &'static str {
    if ascii_chrome_enabled() { "<-" } else { "↩" }
}

#[allow(dead_code)]
pub(crate) fn chrome_sep() -> &'static str {
    if ascii_chrome_enabled() {
        " | "
    } else {
        " │ "
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use unicode_width::UnicodeWidthStr;

    #[test]
    fn glyph_marks_are_static_and_compact() {
        for glyph in Glyph::ALL {
            assert_eq!(glyph.mark().len(), 2);
            assert!(glyph.mark().is_ascii());
            assert_eq!(
                UnicodeWidthStr::width(glyph.rich_mark()),
                2,
                "{glyph:?}: {:?}",
                glyph.rich_mark()
            );
            assert!(!glyph.label().is_empty());
        }
        assert_eq!(Glyph::Active.mark(), "*>");
        assert_eq!(Glyph::Thinking.label(), "thinking");
        assert_eq!(Glyph::Online.mark(), "ON");
        assert_eq!(Glyph::Offline.mark(), "NO");
        assert_eq!(activity(false), Glyph::Idle);
        assert_eq!(activity(true), Glyph::Thinking);
    }

    #[test]
    fn system_and_event_glyphs_form_a_tiny_fixed_width_language() {
        for glyph in [
            Glyph::Tool,
            Glyph::Result,
            Glyph::Check,
            Glyph::Warning,
            Glyph::Online,
            Glyph::Offline,
            Glyph::Busy,
            Glyph::Queued,
            Glyph::Paused,
            Glyph::Blocked,
            Glyph::Error,
            Glyph::Ready,
            Glyph::Stale,
            Glyph::TransferIn,
            Glyph::TransferOut,
            Glyph::Sync,
            Glyph::Bridge,
            Glyph::Remote,
            Glyph::Local,
            Glyph::Cpu,
            Glyph::Gpu,
            Glyph::Memory,
            Glyph::Network,
            Glyph::Disk,
            Glyph::Search,
            Glyph::Mail,
        ] {
            assert_eq!(glyph.mark().len(), 2);
            assert!(glyph.mark().is_ascii());
            assert_eq!(
                UnicodeWidthStr::width(glyph.rich_mark()),
                2,
                "{glyph:?}: {:?}",
                glyph.rich_mark()
            );
        }
        assert_eq!(Glyph::Tool.mark(), "T:");
        assert_eq!(Glyph::Result.mark(), "R:");
        assert_eq!(Glyph::Network.label(), "network");
        assert_eq!(Glyph::Busy.label(), "busy");
        assert_eq!(Glyph::TransferIn.mark(), "<-");
        assert_eq!(Glyph::TransferOut.mark(), "->");
        assert_eq!(Glyph::named("wait"), Some(Glyph::Queued));
        assert_eq!(Glyph::named("hold"), Some(Glyph::Paused));
        assert_eq!(Glyph::named("stuck"), Some(Glyph::Blocked));
        assert_eq!(Glyph::named("fail"), Some(Glyph::Error));
        assert_eq!(Glyph::named("ready"), Some(Glyph::Ready));
        assert_eq!(Glyph::named("old"), Some(Glyph::Stale));
        assert_eq!(Glyph::named("xfer-in"), Some(Glyph::TransferIn));
        assert_eq!(Glyph::named("xfer-out"), Some(Glyph::TransferOut));
        assert_eq!(Glyph::named("route"), Some(Glyph::Bridge));
    }

    #[test]
    fn agent_work_glyphs_are_fixed_width_and_alias_aware() {
        for glyph in [
            Glyph::Plan,
            Glyph::Code,
            Glyph::Review,
            Glyph::Patch,
            Glyph::Brain,
            Glyph::Spark,
            Glyph::Scout,
            Glyph::Planner,
            Glyph::Coder,
            Glyph::Reviewer,
            Glyph::Runner,
            Glyph::Specialist,
            Glyph::Moa,
            Glyph::Lock,
            Glyph::Key,
            Glyph::Session,
            Glyph::Home,
        ] {
            assert_eq!(glyph.mark().len(), 2);
            assert!(glyph.mark().is_ascii());
            assert_eq!(
                UnicodeWidthStr::width(glyph.rich_mark()),
                2,
                "{glyph:?}: {:?}",
                glyph.rich_mark()
            );
        }
        assert_eq!(Glyph::named(":plan:"), Some(Glyph::Plan));
        assert_eq!(Glyph::named("edit"), Some(Glyph::Code));
        assert_eq!(Glyph::named("audit"), Some(Glyph::Review));
        assert_eq!(Glyph::named("diff"), Some(Glyph::Patch));
        assert_eq!(Glyph::named("llm"), Some(Glyph::Brain));
        assert_eq!(Glyph::named("insight"), Some(Glyph::Spark));
        assert_eq!(Glyph::named("scout"), Some(Glyph::Scout));
        assert_eq!(Glyph::named("planner"), Some(Glyph::Planner));
        assert_eq!(Glyph::named("coder"), Some(Glyph::Coder));
        assert_eq!(Glyph::named("reviewer"), Some(Glyph::Reviewer));
        assert_eq!(Glyph::named("exec"), Some(Glyph::Runner));
        assert_eq!(Glyph::named("spec"), Some(Glyph::Specialist));
        assert_eq!(Glyph::named("fleet"), Some(Glyph::Moa));
        assert_eq!(Glyph::named("secure"), Some(Glyph::Lock));
        assert_eq!(Glyph::named("auth"), Some(Glyph::Key));
        assert_eq!(Glyph::named("thread"), Some(Glyph::Session));
        assert_eq!(Glyph::named("workspace"), Some(Glyph::Home));
    }

    #[test]
    fn surface_glyphs_are_fixed_width_and_alias_aware() {
        for glyph in [
            Glyph::Surface,
            Glyph::Scene,
            Glyph::Avatar,
            Glyph::Transcript,
            Glyph::Terminal,
            Glyph::Carousel,
        ] {
            assert_eq!(glyph.mark().len(), 2);
            assert!(glyph.mark().is_ascii());
            assert_eq!(
                UnicodeWidthStr::width(glyph.rich_mark()),
                2,
                "{glyph:?}: {:?}",
                glyph.rich_mark()
            );
        }
        assert_eq!(Glyph::named("surface"), Some(Glyph::Surface));
        assert_eq!(Glyph::named("scene"), Some(Glyph::Scene));
        assert_eq!(Glyph::named("portrait"), Some(Glyph::Avatar));
        assert_eq!(Glyph::named("log"), Some(Glyph::Transcript));
        assert_eq!(Glyph::named("tty"), Some(Glyph::Terminal));
        assert_eq!(Glyph::named("media"), Some(Glyph::Carousel));
        assert_eq!(Glyph::named("shell"), Some(Glyph::Shell));
    }

    #[test]
    fn tiny_font_names_resolve_to_static_tokens() {
        assert_eq!(Glyph::named("live"), Some(Glyph::Online));
        assert_eq!(Glyph::named("down"), Some(Glyph::Offline));
        assert_eq!(Glyph::named(":loading:"), Some(Glyph::Loading));
        assert_eq!(Glyph::named("working"), Some(Glyph::Busy));
        assert_eq!(Glyph::named("xfer"), Some(Glyph::Transfer));
        assert_eq!(Glyph::named("download"), Some(Glyph::TransferIn));
        assert_eq!(Glyph::named("upload"), Some(Glyph::TransferOut));
        assert_eq!(Glyph::named("ready"), Some(Glyph::Ready));
        assert_eq!(Glyph::named("net"), Some(Glyph::Network));
        assert_eq!(Glyph::named("find"), Some(Glyph::Search));
        assert_eq!(token(":active:"), Some(Glyph::Active.mark()));
        assert_eq!(token(":code:"), Some(Glyph::Code.mark()));
        assert_eq!(token(":agent.spark-r1:"), Some(AgentBadge::Sparky.mark()));
        assert_eq!(token(":mail:"), Some(Glyph::Mail.mark()));
        assert_eq!(token("agent.atlas"), Some(AgentBadge::Atlas.mark()));
        assert_eq!(label("agent.ghost-protocol"), Some("casper"));
        assert_eq!(Glyph::named("not-a-token"), None);
        assert_eq!(token("not-a-token"), None);
    }

    #[test]
    fn agent_badges_are_static_compact_and_alias_aware() {
        for badge in AgentBadge::ALL {
            assert_eq!(badge.mark().len(), 2);
            assert!(badge.mark().is_ascii());
            assert_eq!(UnicodeWidthStr::width(badge.token()), 2);
            assert_eq!(UnicodeWidthStr::width(badge.rich_mark()), 2);
            assert!(!badge.label().is_empty());
        }
        assert_eq!(agent_badge("turbo"), AgentBadge::Turbo);
        assert_eq!(agent_badge(" Atlas "), AgentBadge::Atlas);
        assert_eq!(agent_badge("spark-r1"), AgentBadge::Sparky);
        assert_eq!(agent_badge("DICE"), AgentBadge::Apollo);
        assert_eq!(agent_badge("ghost-protocol"), AgentBadge::Casper);
        assert_eq!(agent_badge("scout"), AgentBadge::Strixus);
        assert_eq!(agent_badge("unknown"), AgentBadge::Unknown);
        assert_eq!(AgentBadge::named("turbo"), Some(AgentBadge::Turbo));
        assert_eq!(AgentBadge::named(" Atlas "), Some(AgentBadge::Atlas));
        assert_eq!(AgentBadge::named("agent.DICE"), Some(AgentBadge::Apollo));
        assert_eq!(
            AgentBadge::named("ghost-protocol"),
            Some(AgentBadge::Casper)
        );
        assert_eq!(AgentBadge::named("unknown"), None);
    }

    #[test]
    fn every_glyph_alias_resolves_to_its_token() {
        // Locks the documented alias vocabulary: each input string must resolve to
        // exactly the intended glyph (every `||` branch of `Glyph::named`). A
        // regression that drops or reroutes an alias trips here.
        let cases: &[(&str, Glyph)] = &[
            ("active", Glyph::Active),
            ("run", Glyph::Active),
            ("online", Glyph::Online),
            ("offline", Glyph::Offline),
            ("idle", Glyph::Idle),
            ("sleep", Glyph::Idle),
            ("thinking", Glyph::Thinking),
            ("think", Glyph::Thinking),
            ("reasoning", Glyph::Thinking),
            ("loading", Glyph::Loading),
            ("load", Glyph::Loading),
            ("busy", Glyph::Busy),
            ("work", Glyph::Busy),
            ("queued", Glyph::Queued),
            ("queue", Glyph::Queued),
            ("waiting", Glyph::Queued),
            ("paused", Glyph::Paused),
            ("pause", Glyph::Paused),
            ("blocked", Glyph::Blocked),
            ("block", Glyph::Blocked),
            ("error", Glyph::Error),
            ("err", Glyph::Error),
            ("failed", Glyph::Error),
            ("stale", Glyph::Stale),
            ("transfer", Glyph::Transfer),
            ("transfer-in", Glyph::TransferIn),
            ("recv", Glyph::TransferIn),
            ("receive", Glyph::TransferIn),
            ("in", Glyph::TransferIn),
            ("transfer-out", Glyph::TransferOut),
            ("send-out", Glyph::TransferOut),
            ("out", Glyph::TransferOut),
            ("sync", Glyph::Sync),
            ("bridge", Glyph::Bridge),
            ("remote", Glyph::Remote),
            ("local", Glyph::Local),
            ("agent", Glyph::Agent),
            ("sh", Glyph::Shell),
            ("artifacts", Glyph::Artifacts),
            ("artifact", Glyph::Artifacts),
            ("avatar", Glyph::Avatar),
            ("transcript", Glyph::Transcript),
            ("terminal", Glyph::Terminal),
            ("carousel", Glyph::Carousel),
            ("approval", Glyph::Approval),
            ("approve", Glyph::Approval),
            ("send", Glyph::Send),
            ("enter", Glyph::Send),
            ("back", Glyph::Back),
            ("return", Glyph::Back),
            ("switch", Glyph::Switch),
            ("tab", Glyph::Switch),
            ("tool", Glyph::Tool),
            ("tools", Glyph::Tool),
            ("done", Glyph::Result), // "result" maps to Result; "done" too
            ("check", Glyph::Check),
            ("ok", Glyph::Check),
            ("warning", Glyph::Warning),
            ("warn", Glyph::Warning),
            ("cpu", Glyph::Cpu),
            ("gpu", Glyph::Gpu),
            ("memory", Glyph::Memory),
            ("mem", Glyph::Memory),
            ("network", Glyph::Network),
            ("disk", Glyph::Disk),
            ("store", Glyph::Disk),
            ("search", Glyph::Search),
            ("mail", Glyph::Mail),
            ("inbox", Glyph::Mail),
            ("plan", Glyph::Plan),
            ("todo", Glyph::Plan),
            ("code", Glyph::Code),
            ("review", Glyph::Review),
            ("patch", Glyph::Patch),
            ("brain", Glyph::Brain),
            ("model", Glyph::Brain),
            ("spark", Glyph::Spark),
            ("idea", Glyph::Spark),
            ("specialist", Glyph::Specialist),
            ("moa", Glyph::Moa),
            ("lock", Glyph::Lock),
            ("key", Glyph::Key),
            ("session", Glyph::Session),
            ("chat", Glyph::Session),
            ("home", Glyph::Home),
            ("root", Glyph::Home),
        ];
        for (name, expect) in cases {
            assert_eq!(Glyph::named(name), Some(*expect), "alias {name:?}");
            // Case-insensitivity + the `token`/`label` lookups must agree.
            assert_eq!(
                Glyph::named(&name.to_uppercase()),
                Some(*expect),
                "alias {name:?} is case-insensitive"
            );
            assert_eq!(token(name), Some(expect.mark()));
            assert_eq!(label(name), Some(expect.label()));
        }
        // `ready` is shared: it resolves to Ready (checked before Result).
        assert_eq!(Glyph::named("ready"), Some(Glyph::Ready));
        // Unknown names fall through to None on every public lookup.
        assert_eq!(Glyph::named("definitely-not-a-glyph"), None);
        assert_eq!(token("definitely-not-a-glyph"), None);
        assert_eq!(label("definitely-not-a-glyph"), None);
    }

    #[test]
    fn agent_badge_aliases_and_token_routing() {
        // Names here are badge-unique (no colliding Glyph alias), so the public
        // `token`/`label` lookups route through the badge.
        let cases: &[(&str, AgentBadge)] = &[
            ("sparky", AgentBadge::Sparky),
            ("spark-v4", AgentBadge::Sparky),
            ("apollo", AgentBadge::Apollo),
            ("dice", AgentBadge::Apollo),
            ("casper", AgentBadge::Casper),
            ("ghost", AgentBadge::Casper),
            ("strixus", AgentBadge::Strixus),
        ];
        for (name, expect) in cases {
            assert_eq!(AgentBadge::named(name), Some(*expect), "badge {name:?}");
            assert_eq!(agent_badge(name), *expect);
            // Public `token()`/`label()` route through the badge when no Glyph matches.
            assert_eq!(token(name), Some(expect.mark()));
            assert_eq!(label(name), Some(expect.label()));
        }
        // `spark` is a SHARED alias: `AgentBadge::named` resolves it to Sparky, but
        // the public `token` lookup checks `Glyph` first, so it wins as Glyph::Spark.
        assert_eq!(AgentBadge::named("spark"), Some(AgentBadge::Sparky));
        assert_eq!(token("spark"), Some(Glyph::Spark.mark()));
        // The `:agent.<name>:` symbol form trims to the badge.
        assert_eq!(AgentBadge::named(":agent.turbo:"), Some(AgentBadge::Turbo));
        // Unknown badge → None, and `agent_badge` (its lossy cousin) → Unknown.
        assert_eq!(AgentBadge::named("nobody"), None);
        assert_eq!(agent_badge("nobody"), AgentBadge::Unknown);
        // The Unknown badge still has a real 2-cell mark/token.
        assert_eq!(AgentBadge::Unknown.mark(), "A>");
        assert_eq!(AgentBadge::Unknown.label(), "agent");
    }

    #[test]
    fn symbol_name_trimming_strips_colons_whitespace_and_agent_prefix() {
        // `trim_symbol_name` = trim_ascii ∘ trim_colons ∘ trim_agent_prefix, all
        // reached through `named`. Each wrapper must compose.
        assert_eq!(Glyph::named("  :code:  "), Some(Glyph::Code));
        assert_eq!(Glyph::named("AGENT.cpu"), Some(Glyph::Cpu));
        assert_eq!(Glyph::named(":agent.gpu:"), Some(Glyph::Gpu));
        // A lone colon pair with no inner name trims to empty → no match.
        assert_eq!(Glyph::named("::"), None);
    }

    #[test]
    fn media_prefixes_map_known_artifact_kinds() {
        assert_eq!(media_prefix("image"), "I> img   ");
        assert_eq!(media_prefix("img"), "I> img   ");
        assert_eq!(media_prefix("video"), "V> reel  ");
        assert_eq!(media_prefix("link"), "L> link  ");
        assert_eq!(media_prefix("graph"), "G> graph ");
        assert_eq!(media_prefix("res"), "R> res   ");
        assert_eq!(media_prefix("unknown"), "L> link  ");
    }

    #[test]
    fn rich_media_prefixes_remain_fixed_width() {
        for prefix in [
            "▧  img   ",
            "▶  reel  ",
            "↗  link  ",
            "⌁  graph ",
            "◫  res   ",
        ] {
            assert_eq!(UnicodeWidthStr::width(prefix), 9);
        }
    }
}
