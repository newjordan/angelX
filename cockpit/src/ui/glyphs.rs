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
#[path = "../../../tests/cockpit/app/glyphs__tests.rs"]
mod tests;
