//! Optional Caveman-style brevity for token-metered SOTA outbounds.
//!
//! This is prompt-level output compression. It does not alter tool calls,
//! request JSON, code blocks, commands, or quoted provider errors.

use super::*;

const CAVEMAN_MARKER: &str = "angelX SOTA brevity mode";

/// The brevity instruction and where it belongs in `messages` (right after the
/// leading system block), or `None` when the mode is off or already applied.
/// Returned as an index + text so the caller splices it into the outbound JSON
/// — the history itself is never cloned for this (it used to be, every hop).
pub(crate) fn sota_caveman_insert(messages: &[ChatMsg]) -> Option<(usize, String)> {
    if !sota_caveman_enabled() || messages.iter().any(has_caveman_marker) {
        return None;
    }
    let insert_at = messages
        .iter()
        .take_while(|m| m.role == ChatRole::System)
        .count();
    Some((insert_at, sota_caveman_instruction()))
}

pub(crate) fn sota_caveman_enabled() -> bool {
    #[cfg(not(test))]
    {
        static ON: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
        *ON.get_or_init(sota_caveman_enabled_from_env)
    }
    #[cfg(test)]
    sota_caveman_enabled_from_env()
}

fn sota_caveman_enabled_from_env() -> bool {
    match std::env::var("ANGEL_SOTA_CAVEMAN")
        .ok()
        .map(|v| v.trim().to_ascii_lowercase())
        .filter(|v| !v.is_empty())
    {
        Some(v) if matches!(v.as_str(), "0" | "false" | "no" | "off" | "none") => false,
        Some(_) => true,
        None => false,
    }
}

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
    let level = std::env::var("ANGEL_SOTA_CAVEMAN_LEVEL")
        .ok()
        .map(|s| s.trim().to_ascii_lowercase())
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| "full".to_string());
    let level_rule = match level.as_str() {
        "lite" => "Lite: remove filler and hedging, but keep normal grammar.",
        "ultra" => {
            "Ultra: shortest unambiguous answer. State each fact once. Use fragments when clear."
        }
        "wenyan" | "wenyan-lite" | "wenyan-full" | "wenyan-ultra" => {
            "Wenyan levels are disabled for angelX SOTA outbounds; use full terse English unless the user writes Chinese."
        }
        _ => "Full: drop filler, pleasantries, and padded narration. Fragments OK.",
    };
    format!(
        "{CAVEMAN_MARKER} ({level}). {level_rule} Preserve technical substance. \
         Keep code blocks, CLI commands, JSON, tool-call arguments, API names, file paths, \
         commit keywords, and exact error strings verbatim. Obey explicit output formats, \
         safety warnings, and destructive-action confirmations over brevity. Do not mention \
         this mode."
    )
}

fn has_caveman_marker(m: &ChatMsg) -> bool {
    m.role == ChatRole::System && m.content.contains(CAVEMAN_MARKER)
}
