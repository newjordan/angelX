//! Caveman-style brevity for token-metered SOTA outbounds and self-hosted
//! serves, spliced as the `⠵` brevity warpath.
//!
//! This is prompt-level output compression. It does not alter tool calls,
//! request JSON, code blocks, commands, or quoted provider errors. A
//! self-hosted serve gets its own reasoning back verbatim (its live KV prefix
//! needs the exact tokens), so brevity there shortens reasoning where it is
//! written.

use super::*;

/// The brevity instruction and where it belongs in `messages` (right after the
/// leading system block), or `None` when the mode is off or already applied.
/// Returned as an index + text so the caller splices it into the outbound JSON
/// — the history itself is never cloned for this (it used to be, every hop).
/// `ANGEL_SOTA_CAVEMAN` decides when set; unset, a self-hosted link is on.
pub(crate) fn sota_caveman_insert(
    messages: &[ChatMsg],
    self_hosted: bool,
) -> Option<(usize, String)> {
    if !sota_caveman_enabled().unwrap_or(self_hosted) || messages.iter().any(has_caveman_marker) {
        return None;
    }
    let insert_at = messages
        .iter()
        .take_while(|m| m.role == ChatRole::System)
        .count();
    Some((insert_at, sota_caveman_instruction()))
}

/// The operator's explicit `ANGEL_SOTA_CAVEMAN` choice, if any.
pub(crate) fn sota_caveman_enabled() -> Option<bool> {
    #[cfg(not(test))]
    {
        static ON: std::sync::OnceLock<Option<bool>> = std::sync::OnceLock::new();
        *ON.get_or_init(sota_caveman_enabled_from_env)
    }
    #[cfg(test)]
    sota_caveman_enabled_from_env()
}

fn sota_caveman_enabled_from_env() -> Option<bool> {
    std::env::var("ANGEL_SOTA_CAVEMAN")
        .ok()
        .map(|v| v.trim().to_ascii_lowercase())
        .filter(|v| !v.is_empty())
        .map(|v| !matches!(v.as_str(), "0" | "false" | "no" | "off" | "none"))
}

/// The brevity warpath: the marker, the `ANGEL_SOTA_CAVEMAN_LEVEL` level and
/// the rules (`⠵⠁⠵⠉⠵⠋` for full). Their words are `ledger://⠵`, verbatim.
pub(crate) fn sota_caveman_instruction() -> String {
    #[cfg(not(test))]
    {
        static TEXT: std::sync::OnceLock<String> = std::sync::OnceLock::new();
        TEXT.get_or_init(sota_caveman_instruction_from_env).clone()
    }
    #[cfg(test)]
    sota_caveman_instruction_from_env()
}

fn sota_caveman_instruction_from_env() -> String {
    use crate::agent::harness::book::z_brevity::{FULL, LITE, MARKER, RULES, ULTRA, WENYAN};
    let level = std::env::var("ANGEL_SOTA_CAVEMAN_LEVEL")
        .ok()
        .map(|s| s.trim().to_ascii_lowercase())
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| "full".to_string());
    let level = match level.as_str() {
        "lite" => LITE,
        "ultra" => ULTRA,
        "wenyan" | "wenyan-lite" | "wenyan-full" | "wenyan-ultra" => WENYAN,
        _ => FULL,
    };
    [MARKER, level, RULES].map(|route| route.cells()).concat()
}

fn has_caveman_marker(m: &ChatMsg) -> bool {
    m.role == ChatRole::System
        && m.content.contains(
            crate::agent::harness::book::z_brevity::MARKER
                .cells()
                .as_str(),
        )
}
