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
    Active,
    Idle,
    #[allow(dead_code)]
    Online,
    Offline,
    Thinking,
    Transfer,
    Shell,
    Approval,
    Back,
    Tool,
    Result,
    Warning,
    Cpu,
    Gpu,
    Memory,
}

impl Glyph {
    #[cfg(test)]
    pub const ALL: [Self; 15] = [
        Self::Active,
        Self::Idle,
        Self::Online,
        Self::Offline,
        Self::Thinking,
        Self::Transfer,
        Self::Shell,
        Self::Approval,
        Self::Back,
        Self::Tool,
        Self::Result,
        Self::Warning,
        Self::Cpu,
        Self::Gpu,
        Self::Memory,
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
            Self::Transfer => Token {
                ascii: "<>",
                rich: "⇄ ",
                label: "transfer",
            },
            Self::Shell => Token {
                ascii: "$>",
                rich: "$ ",
                label: "shell",
            },
            Self::Approval => Token {
                ascii: "!!",
                rich: "! ",
                label: "approval",
            },
            Self::Back => Token {
                ascii: "<<",
                rich: "← ",
                label: "back",
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

#[cfg(test)]
#[path = "../../../tests/cockpit/app/glyphs__tests.rs"]
mod tests;
