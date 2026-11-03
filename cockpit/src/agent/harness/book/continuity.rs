//! A small inventory of used signal addresses, carried with the conversation.
//! Appending a handoff keeps existing cache prefixes intact. Outbound legend
//! expansion supplies English; local session metadata retains the inventory
//! without adding tokens to ordinary requests.

use super::{d12467_sloptomizer as live, introduction, ledger};
use crate::agent::club::{ChatMsg, ChatRole, RouteIdentity};
use std::collections::BTreeSet;

#[derive(Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub(crate) struct Memory {
    route: Option<String>,
    used: BTreeSet<String>,
    /// Digest of the exact catalog snapshot used by this conversation. A
    /// missing field is the legacy session format.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    catalog_digest: Option<String>,
    /// A restored conversation predating generation binding. Its first new
    /// request must use the embedded baseline, not a later edited chapter.
    #[serde(default, skip_serializing_if = "is_false")]
    catalog_legacy: bool,
}

fn is_false(value: &bool) -> bool {
    !*value
}

/// Read the newest retained book-generation binding from a conversation.
pub(crate) fn generation_digest(history: &[ChatMsg]) -> Option<&str> {
    history
        .iter()
        .rev()
        .find_map(|message| message.book_memory.as_ref()?.catalog_digest.as_deref())
}

pub(crate) fn is_legacy(history: &[ChatMsg]) -> bool {
    history.iter().any(|message| {
        message
            .book_memory
            .as_ref()
            .is_some_and(|memory| memory.catalog_legacy)
    })
}

/// Mark a restored legacy conversation without touching disk. Empty or
/// system-only histories have no earlier exchange whose book must be pinned.
pub(crate) fn mark_legacy(history: &mut [ChatMsg]) {
    if generation_digest(history).is_some() || is_legacy(history) {
        return;
    }
    let mut memory = history
        .iter()
        .rev()
        .find_map(|message| message.book_memory.as_deref().cloned())
        .unwrap_or_else(|| Memory {
            route: None,
            used: BTreeSet::new(),
            catalog_digest: None,
            catalog_legacy: false,
        });
    if let Some(message) = history.iter_mut().rev().find(|message| {
        matches!(
            message.role,
            ChatRole::User | ChatRole::Assistant | ChatRole::Tool
        )
    }) {
        memory.catalog_legacy = true;
        message.book_memory = Some(std::sync::Arc::new(memory));
    }
}

/// Attach an already-published generation to a message that survives bootstrap
/// refresh on resume. A conversation cannot silently change generations.
pub(crate) fn bind_generation(history: &mut [ChatMsg], digest: &str) -> Result<(), String> {
    if digest.len() != 64
        || !digest
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
    {
        return Err("invalid book generation digest".into());
    }
    let mut already_bound = false;
    for message in history.iter() {
        let Some(memory) = message.book_memory.as_deref() else {
            continue;
        };
        if let Some(existing) = memory.catalog_digest.as_deref() {
            if existing != digest {
                return Err("conversation is already bound to a different book generation".into());
            }
            already_bound |= matches!(
                message.role,
                ChatRole::User | ChatRole::Assistant | ChatRole::Tool
            );
        }
    }
    if already_bound {
        return Ok(());
    }
    let mut memory = history
        .iter()
        .rev()
        .find_map(|message| message.book_memory.as_deref().cloned())
        .unwrap_or_else(|| Memory {
            route: None,
            used: BTreeSet::new(),
            catalog_digest: None,
            catalog_legacy: false,
        });
    memory.catalog_digest = Some(digest.to_string());
    memory.catalog_legacy = false;
    let message = history
        .iter_mut()
        .rev()
        .find(|message| {
            matches!(
                message.role,
                ChatRole::User | ChatRole::Assistant | ChatRole::Tool
            )
        })
        .ok_or_else(|| {
            "cannot bind book generation without a retained conversation message".to_string()
        })?;
    message.book_memory = Some(std::sync::Arc::new(memory));
    Ok(())
}

/// Preserve the discarded window's inventory even when compaction runs between
/// turns. Include stamps added after its last provider checkpoint.
pub(crate) fn compaction_memory(history: &[ChatMsg]) -> Option<std::sync::Arc<Memory>> {
    let state = Continuity::new(history);
    (!state.used.is_empty()
        || state.route.is_some()
        || state.catalog_digest.is_some()
        || state.catalog_legacy)
        .then(|| {
            std::sync::Arc::new(Memory {
                route: state.route,
                used: state.used,
                catalog_digest: state.catalog_digest,
                catalog_legacy: state.catalog_legacy,
            })
        })
}

#[derive(Default)]
pub(crate) struct Continuity {
    used: BTreeSet<String>,
    route: Option<String>,
    catalog_digest: Option<String>,
    catalog_legacy: bool,
    seen: usize,
    tail: Option<std::sync::Arc<str>>,
    compacted: bool,
}

pub(crate) fn is_handoff(text: &str) -> bool {
    text.lines().next() == Some(format!("{}{}", live::ATTENTION, live::HANDOFF.cells()).as_str())
}

impl Continuity {
    pub(crate) fn new(history: &[ChatMsg]) -> Self {
        let mut state = Self::default();
        state.observe(history);
        state.catalog_digest = generation_digest(history).map(str::to_string);
        state.catalog_legacy = is_legacy(history);
        if let Some(memory) = history.iter().rev().find_map(|m| m.book_memory.as_ref()) {
            state.route = memory.route.clone();
            state.compacted |= !memory.used.is_subset(&state.used);
            state.used.extend(
                memory
                    .used
                    .iter()
                    .filter(|cells| ledger::addresses(cells).is_some())
                    .cloned(),
            );
        }
        state
    }

    pub(crate) fn observe(&mut self, history: &[ChatMsg]) {
        self.compacted |= history.len() < self.seen;
        let intact = self.seen <= history.len()
            && self
                .seen
                .checked_sub(1)
                .and_then(|i| history.get(i))
                .zip(self.tail.as_ref())
                .is_some_and(|(message, tail)| std::sync::Arc::ptr_eq(&message.content, tail));
        for message in &history[if intact { self.seen } else { 0 }..] {
            // Standing profiles already have a stable legend in the system
            // prefix. Carry only signals actually encountered during the run.
            if !matches!(message.role, ChatRole::Tool | ChatRole::Harness) {
                continue;
            }
            for address in introduction::stamps_in(&message.content) {
                if address.primary == live::CELL && address.section == Some(live::HANDOFF.sub) {
                    continue;
                }
                if let Some(section) = address.section {
                    let mut cells: String = [address.primary, section].into_iter().collect();
                    cells.extend(address.page);
                    if ledger::addresses(&cells).is_some() {
                        self.used.insert(cells);
                    }
                }
            }
        }
        self.seen = history.len();
        self.tail = history.last().map(|m| m.content.clone());
    }

    /// Called at the final request boundary, including provider overflow
    /// retries. It neither requests a model nor changes the selected route.
    pub(crate) fn prepare(&mut self, history: &mut Vec<ChatMsg>, route: &RouteIdentity) -> bool {
        self.observe(history);
        let key = crate::knowledge::cut::sha256_hex(
            serde_json::json!([route.driver, route.model])
                .to_string()
                .as_bytes(),
        );
        let changed = self.route.as_ref().is_some_and(|old| old != &key);
        self.route = Some(key.clone());
        let replay = !self.used.is_empty() && (changed || self.compacted);
        self.compacted = false;
        if replay {
            let inventory = self
                .used
                .iter()
                .cloned()
                .collect::<Vec<_>>()
                .chunks(3)
                .map(|chunk| chunk.concat())
                .collect::<Vec<_>>()
                .join("\n");
            let note = format!("{}{}\n{inventory}", live::ATTENTION, live::HANDOFF.cells());
            history.push(ChatMsg::harness(note));
        }
        let already_saved = history
            .iter()
            .rev()
            .find_map(|m| m.book_memory.as_ref())
            .is_some_and(|memory| {
                memory.route.as_ref() == Some(&key)
                    && memory.used == self.used
                    && memory.catalog_digest == self.catalog_digest
                    && memory.catalog_legacy == self.catalog_legacy
            });
        if (!self.used.is_empty() || self.catalog_digest.is_some() || self.catalog_legacy)
            && !already_saved
            && let Some(last) = history.last_mut()
        {
            last.book_memory = Some(std::sync::Arc::new(Memory {
                route: Some(key),
                used: self.used.clone(),
                catalog_digest: self.catalog_digest.clone(),
                catalog_legacy: self.catalog_legacy,
            }));
        }
        self.seen = history.len();
        self.tail = history.last().map(|m| m.content.clone());
        replay
    }
}

#[cfg(test)]
#[path = "../../../../../tests/cockpit/harness/book__continuity_tests.rs"]
mod tests;
