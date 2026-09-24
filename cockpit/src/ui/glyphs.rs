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
    #[allow(dead_code)]
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
    #[allow(dead_code)]
    Agent,
    Shell,
    #[allow(dead_code)]
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
    #[allow(dead_code)]
    Send,
    Back,
    #[allow(dead_code)]
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

#[allow(dead_code)]
pub const fn activity(thinking: bool) -> Glyph {
    if thinking {
        Glyph::Thinking
    } else {
        Glyph::Idle
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
