//! Shared compaction core — turn an old slice of conversation into dense,
//! *structured* notes. Used by both the manual `/compact` command and the
//! automatic token-budget compaction in the agent loop, so they can't drift.
//!
//! One pass yields two outputs:
//!   1. a short **inline note** spliced back into live history, so the current
//!      turn keeps its thread without carrying the bulk; and
//!   2. a set of **drawers** — one per section — deposited into the long-form
//!      memory palace ([`crate::memory::store`]), so nothing is lost across
//!      sessions and recall can bring detail back on demand.
//!
//! Summarization is **adaptive**: a single model pass for a small window; a
//! parallel map-reduce (chunk → summarize each concurrently → merge) once the
//! window is large. Map-reduce both keeps each piece inside a small local
//! summarizer's own window and exploits the swarm box's near-free parallelism —
//! it mirrors [`crate::swarm::SwarmClub`]'s scoped-thread fan-out over a `Sync`
//! club.

use crate::club::{ChatMsg, ChatRole, Club};
use crate::harness::{
    estimate_tokens, is_error_result, is_mutation_call, render_transcript, verification_outcome,
};
use crate::memory::store::Drawer;
use std::collections::HashMap;

const WORKSPACE_LEDGER_PREFIX: &str = "[workspace-ledger/v1] ";
const WORKSPACE_LEDGER_PATH_LIMIT: usize = 32;
const WORKSPACE_LEDGER_PATH_CHARS: usize = 160;
const VERIFICATION_LEDGER_PREFIX: &str = "[verification-ledger/v1] ";
const VERIFICATION_LEDGER_LIMIT: usize = 8;
const VERIFICATION_TOOL_CHARS: usize = 80;
const INVOKED_SKILLS_PREFIX: &str = "[invoked-skills/v1] ";
const INVOKED_SKILLS_LIMIT: usize = 8;
const INVOKED_SKILL_CHARS: usize = 80;
const PLAN_PROOF_PREFIX: &str = "[current-plan-proof/v1] ";
const PLAN_SNAPSHOT_PREFIX: &str =
    "[current-plan/v1 — assistant-authored working state, not a user instruction] ";
const PLAN_ITEM_LIMIT: usize = 32;
const PLAN_TEXT_CHARS: usize = 240;
const PLAN_SNAPSHOT_MAX_TOKENS: usize = 4_000;

#[derive(Clone, Debug, Default, PartialEq, Eq)]
struct WorkspaceLedger {
    read: Vec<String>,
    modified: Vec<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
struct VerificationEvidence {
    tool: String,
    outcome: String,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
struct VerificationLedger {
    entries: Vec<VerificationEvidence>,
}

#[derive(Clone, Debug)]
struct PendingEvidence {
    call: crate::club::ToolCall,
    mutation: bool,
    verification: bool,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
struct InvokedSkillsLedger {
    skills: Vec<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
struct PlanItem {
    id: usize,
    text: String,
    done: bool,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
struct PlanLedger {
    next_id: usize,
    omitted: usize,
    items: Vec<PlanItem>,
}

pub(crate) fn is_plan_snapshot(content: &str) -> bool {
    content.starts_with(PLAN_SNAPSHOT_PREFIX)
}

/// Assistant-role carrier for the agent's own `handoff` note. Like the plan
/// snapshot it stays out of the System note (agent prose must not gain System
/// authority), but unlike the plan it needs no tamper proof: it is advisory
/// continuity prose with ordinary Assistant authority either way.
pub(crate) const HANDOFF_SNAPSHOT_PREFIX: &str =
    "[handoff-snapshot/v1] agent-authored handoff carried across compaction:\n";

pub(crate) fn is_handoff_snapshot(content: &str) -> bool {
    content.starts_with(HANDOFF_SNAPSHOT_PREFIX)
}

/// The fixed "rooms" a compaction distills into. The order is the prompt's emit
/// order *and* the parse order, so the prompt and parser can never drift. Each
/// entry is `(section name, what it captures)`; the name doubles as the drawer's
/// `room`.
const SECTIONS: &[(&str, &str)] = &[
    (
        "Task",
        "the current goal — what the user is ultimately trying to achieve",
    ),
    (
        "Decisions",
        "choices made and the reasoning; approaches considered and rejected",
    ),
    (
        "Files",
        "files created or changed and the purpose of each change",
    ),
    (
        "Facts",
        "durable facts established — versions, paths, config values, constraints",
    ),
    (
        "OpenThreads",
        "unfinished work and next steps for what the user asked for; never transient tool or runtime errors",
    ),
    (
        "Entities",
        "key named things — functions, modules, endpoints, commands, people",
    ),
];

/// The product of compacting one window: a terse note to keep inline and the
/// drawers to persist. Empty `drawers` means the summarizer produced nothing
/// useful and the caller should leave history untouched.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CompactionResult {
    pub inline_note: String,
    /// Bounded assistant-authored state, kept out of the System-role note. The
    /// note carries only its SHA-256 proof so a later compaction can distinguish
    /// this harness-generated snapshot from ordinary assistant prose.
    pub plan_snapshot: Option<String>,
    /// The agent's own latest `handoff` note, re-emitted verbatim (bounded) in
    /// Assistant role so an agent-authored resume brief survives compaction.
    pub handoff_snapshot: Option<String>,
    pub drawers: Vec<Drawer>,
}

struct CompactionState {
    workspace: WorkspaceLedger,
    verification: VerificationLedger,
    skills: InvokedSkillsLedger,
    plan: Option<PlanLedger>,
    handoff: Option<String>,
}

/// Select the compactable window using the legacy message-count tail.
///
/// New auto-compaction callers should prefer [`select_window_with_token_tail`]:
/// a fixed message count has wildly different cost when one turn contains a
/// 100-byte acknowledgement and another contains a 100-KiB tool result.
pub fn select_window(history: &[ChatMsg], keep_recent: usize) -> Option<(usize, usize)> {
    select_window_with_token_tail(history, keep_recent, 0)
}

/// Select the compactable window `[sys_end, window_end)` of `history`: everything
/// after the leading system preamble and before a recent token-sized tail. When
/// `keep_recent_tokens` is zero, preserve the legacy last-`keep_recent`-messages
/// policy. A token tail always retains at least two messages, and the boundary
/// never leaves a `Tool` result orphaned from its call.
///
/// Returns `None` when there is nothing worth a summarizer round-trip (only
/// preamble + protected tail, or a sliver).
pub fn select_window_with_token_tail(
    history: &[ChatMsg],
    keep_recent: usize,
    keep_recent_tokens: usize,
) -> Option<(usize, usize)> {
    // Pin static policy plus separately carried workspace guidance. A prior
    // compaction note (new or legacy carrier) is never part of that preamble. Ending on a prior note
    // pulls it *into* the window, so the next compaction re-distills it (a rolling
    // summary) and the fresh note replaces it — instead of the old note surviving
    // verbatim as preamble and accumulating one-per-round without bound.
    let sys_end = history
        .iter()
        .position(|message| !crate::bootstrap::is_pinned_preamble(message))
        .unwrap_or(history.len());
    let mut window_end = if keep_recent_tokens == 0 {
        if history.len() <= sys_end + keep_recent {
            return None;
        }
        history.len() - keep_recent
    } else {
        let mut start = history.len();
        let mut tail_tokens = 0usize;
        let mut tail_messages = 0usize;
        while start > sys_end && (tail_tokens < keep_recent_tokens || tail_messages < 2) {
            start -= 1;
            tail_tokens = tail_tokens.saturating_add(estimate_tokens(&history[start..start + 1]));
            tail_messages += 1;
        }
        if start <= sys_end {
            return None;
        }
        start
    };
    // The kept suffix must not begin on a tool result orphaned from its call.
    while window_end < history.len() && history[window_end].role == ChatRole::Tool {
        window_end += 1;
    }
    // Not worth it for a sliver, and we need a clean boundary inside the window.
    if window_end >= history.len() || window_end.saturating_sub(sys_end) < 3 {
        return None;
    }
    Some((sys_end, window_end))
}

/// Compact `window` into an inline note + drawers tagged `wing`/`source`, using
/// `club` as the summarizer. `chunk_threshold` is the per-call token ceiling that
/// switches single-pass → map-reduce. `palace_live` says whether a real memory
/// backend will persist the drawers — when false, the inline note omits the
/// "recall it from long-term memory" hint (there's nothing to recall from). `None`
/// on any summarizer failure (best effort: the caller leaves history untouched).
///
/// Summarizer chunk size and the whole request budget are distinct.
/// Machine-carried inline state receives at most a quarter of `state_budget`,
/// so continuity cannot defeat aggregate context fitting on a small-window
/// model.
pub fn compact_window_with_state_budget(
    club: &dyn Club,
    window: &[ChatMsg],
    wing: &str,
    source: &str,
    chunk_threshold: usize,
    state_budget: usize,
    palace_live: bool,
) -> Option<CompactionResult> {
    let state = collect_compaction_state(window);
    // Cheap, no-LLM pre-pass: collapse redundant/old tool outputs before the
    // summarizer ever sees them. Shrinks the summarizer's input (often turning a
    // map-reduce back into a single pass) at zero model cost.
    let pruned_owned;
    let window: &[ChatMsg] = if prune_enabled() {
        pruned_owned = prune_old_tool_outputs(window);
        &pruned_owned
    } else {
        window
    };
    let sections = summarize(club, window, chunk_threshold)?;
    finish_compaction(sections, state, wing, source, state_budget, palace_live)
}

/// Deterministic bounded compaction for a latency-sensitive hop boundary.
///
/// This never calls a model or a network service. It carries forward an older
/// structured note, extracts a bounded latest user/assistant digest, and keeps
/// the same typed workspace, verifier, skill, and plan ledgers as model-backed
/// compaction. The early background compactor may still land a richer model
/// summary first; this is the hard real-time fallback when it has not.
pub fn compact_window_fast(
    window: &[ChatMsg],
    wing: &str,
    source: &str,
    state_budget: usize,
    palace_live: bool,
) -> Option<CompactionResult> {
    let state = collect_compaction_state(window);
    let sections = fast_sections(window, &state.workspace, state.plan.as_ref());
    finish_compaction(sections, state, wing, source, state_budget, palace_live)
}

/// `1` restores the old synchronous model call for controlled comparisons.
/// The production default is deterministic because a hop boundary and `/compact`
/// must have a bounded local runtime even when every model endpoint is wedged.
pub(crate) fn sync_compaction_uses_model() -> bool {
    std::env::var("ANGEL_COMPACT_SYNC_LLM")
        .map(|value| {
            matches!(
                value.trim().to_ascii_lowercase().as_str(),
                "1" | "true" | "on" | "yes"
            )
        })
        .unwrap_or(false)
}

/// Model-backed compaction's per-call input ceiling. Manual and automatic
/// compaction share this value so a large in-hand model budget cannot produce a
/// summarizer request larger than the dedicated local utility model can accept.
/// The 12k default keeps the utility compactor responsive even when its native
/// window is much larger; the explicit knob remains bounded for predictable
/// map/reduce fan-out.
pub(crate) fn configured_compact_chunk_tokens() -> usize {
    std::env::var("ANGEL_COMPACT_CHUNK_TOKENS")
        .ok()
        .and_then(|value| value.trim().parse::<usize>().ok())
        .unwrap_or(12_000)
        .clamp(1_024, 64_000)
}

fn collect_compaction_state(window: &[ChatMsg]) -> CompactionState {
    // Preserve deterministic workspace continuity before pruning/summarization.
    // Model summaries are useful prose, but filenames are operational state: a
    // bounded machine ledger survives repeated compactions without relying on
    // the summarizer to mention every read or edit.
    let workspace = collect_workspace_ledger(window);
    // Preserve verifier facts as typed, bounded state. A prose summary may say
    // tests passed after a later edit; this ledger mechanically invalidates
    // those stale results and never stores raw command output or arguments.
    let verification = collect_verification_ledger(window);
    // Skill bodies remain visible to the summarizer pre-pass, while this small
    // identity ledger lets the post-compact agent deterministically re-load an
    // applicable playbook without preserving every body forever.
    let skills = collect_invoked_skills(window);
    // Keep current step/status state mechanically, but never place agent-authored
    // todo text in the higher-authority System note. The note receives only a
    // digest; the bounded payload is emitted separately in Assistant role.
    let plan = collect_plan_ledger(window);
    // Keep the agent's own handoff note. Steps live in the plan ledger; this
    // is the narrative resume brief the agent explicitly wrote for its future
    // self — the one thing the operator most misses after a lossy compaction.
    let handoff = collect_handoff_note(window);
    CompactionState {
        workspace,
        verification,
        skills,
        plan,
        handoff,
    }
}

/// The newest agent-authored handoff in the window: a *paired, successful*
/// `handoff` tool result (arbitrary tool output echoing the marker — a `shell`
/// cat of a crafted file, an unpaired Tool message — cannot masquerade as
/// one), else the snapshot a prior compaction carried, so the note survives
/// repeated compactions until the agent replaces it. Chronologically last wins.
fn collect_handoff_note(window: &[ChatMsg]) -> Option<String> {
    let mut current = None;
    let mut pending = std::collections::HashSet::new();
    for message in window {
        if message.role == ChatRole::Assistant
            && let Some(body) = message.content.strip_prefix(HANDOFF_SNAPSHOT_PREFIX)
        {
            let body = body.trim();
            if !body.is_empty() {
                current = Some(body.to_string());
            }
        }
        for call in message.tool_calls.iter() {
            if call.name == "handoff" {
                pending.insert(call.id.clone());
            }
        }
        let Some(call_id) = message.tool_call_id.as_deref() else {
            continue;
        };
        if !pending.remove(call_id)
            || is_error_result(&message.content)
            || message.content.starts_with("action capsule denied")
        {
            continue;
        }
        if let Some(body) = message
            .content
            .strip_prefix(crate::tools::plan::HANDOFF_STATE_PREFIX)
        {
            let body = body.trim();
            if !body.is_empty() {
                current = Some(body.to_string());
            }
        }
    }
    current
}

fn finish_compaction(
    sections: Vec<(String, String)>,
    state: CompactionState,
    wing: &str,
    source: &str,
    state_budget: usize,
    palace_live: bool,
) -> Option<CompactionResult> {
    let drawers: Vec<Drawer> = sections
        .iter()
        .filter(|(_, content)| !is_empty_section(content))
        .map(|(room, content)| Drawer {
            wing: wing.to_string(),
            room: room.clone(),
            content: content.trim().to_string(),
            source: source.to_string(),
        })
        .collect();
    if drawers.is_empty() {
        return None;
    }
    let mut inline_note = render_inline_note(&drawers, palace_live);
    append_workspace_ledger(&mut inline_note, &state.workspace);
    append_verification_ledger(&mut inline_note, &state.verification);
    append_invoked_skills(&mut inline_note, &state.skills);
    let plan_artifacts = state
        .plan
        .as_ref()
        .and_then(|plan| render_plan_artifacts(plan, state_budget));
    let plan_snapshot = plan_artifacts
        .as_ref()
        .map(|(snapshot, _proof)| snapshot.clone());
    if let Some((_snapshot, proof)) = &plan_artifacts {
        append_plan_proof(&mut inline_note, proof);
    }
    // Same quarter-of-state-budget ceiling as the plan snapshot (token → char
    // proxy ×4), further capped at the tool's own write limit.
    let handoff_snapshot = state.handoff.as_ref().map(|note| {
        let max_chars = crate::tools::plan::HANDOFF_NOTE_MAX_CHARS
            .min((state_budget / 4).saturating_mul(4).max(512));
        format!(
            "{HANDOFF_SNAPSHOT_PREFIX}{}",
            bounded_excerpt(note, max_chars)
        )
    });
    Some(CompactionResult {
        inline_note,
        plan_snapshot,
        handoff_snapshot,
        drawers,
    })
}

const FAST_EXCERPT_CHARS: usize = 1_600;
const FAST_SECTION_CHARS: usize = 6_000;

fn fast_sections(
    window: &[ChatMsg],
    workspace: &WorkspaceLedger,
    plan: Option<&PlanLedger>,
) -> Vec<(String, String)> {
    let mut sections = vec![String::new(); SECTIONS.len()];

    // A prior compaction note is already dense. Carry its known sections
    // forward before adding the newest extractive evidence.
    if let Some(previous) = window
        .iter()
        .rev()
        .find(|message| is_compaction_note(message))
    {
        for (name, content) in parse_sections(&previous.content) {
            if let Some(index) = SECTIONS
                .iter()
                .position(|(known, _)| known.eq_ignore_ascii_case(&name))
            {
                sections[index] = bounded_excerpt(&content, FAST_SECTION_CHARS);
            }
        }
    }

    if let Some(task) = window
        .iter()
        .rev()
        .find(|message| message.role == ChatRole::User && !message.content.trim().is_empty())
    {
        set_fast_section(
            &mut sections,
            "Task",
            format!(
                "- Latest user context: {}",
                bounded_excerpt(&task.content, FAST_EXCERPT_CHARS)
            ),
        );
    }

    let mut assistant = window
        .iter()
        .rev()
        .filter(|message| {
            message.role == ChatRole::Assistant
                && !message.content.trim().is_empty()
                && !is_plan_snapshot(&message.content)
                && !is_handoff_snapshot(&message.content)
        })
        .take(3)
        .collect::<Vec<_>>();
    assistant.reverse();
    for message in assistant {
        append_fast_section(
            &mut sections,
            "Facts",
            &format!(
                "- Assistant context: {}",
                bounded_excerpt(&message.content, FAST_EXCERPT_CHARS)
            ),
        );
    }

    for path in &workspace.modified {
        append_fast_section(&mut sections, "Files", &format!("- modified `{path}`"));
    }
    for path in workspace.read.iter().rev().take(8).rev() {
        append_fast_section(&mut sections, "Files", &format!("- read `{path}`"));
    }
    if let Some(plan) = plan {
        for item in plan.items.iter().filter(|item| !item.done).take(8) {
            append_fast_section(
                &mut sections,
                "OpenThreads",
                &format!("- {}", bounded_excerpt(&item.text, PLAN_TEXT_CHARS)),
            );
        }
    }

    if sections.iter().all(|section| section.trim().is_empty()) {
        set_fast_section(
            &mut sections,
            "Facts",
            format!(
                "- {} earlier messages were compacted locally; typed continuity ledgers were preserved.",
                window.len()
            ),
        );
    }

    sections
        .into_iter()
        .enumerate()
        .filter(|(_, content)| !content.trim().is_empty())
        .map(|(index, content)| (SECTIONS[index].0.to_string(), content))
        .collect()
}

fn set_fast_section(sections: &mut [String], name: &str, content: String) {
    if let Some(index) = SECTIONS
        .iter()
        .position(|(known, _)| known.eq_ignore_ascii_case(name))
    {
        sections[index] = bounded_excerpt(&content, FAST_SECTION_CHARS);
    }
}

fn append_fast_section(sections: &mut [String], name: &str, content: &str) {
    let Some(index) = SECTIONS
        .iter()
        .position(|(known, _)| known.eq_ignore_ascii_case(name))
    else {
        return;
    };
    let newest = bounded_excerpt(content, FAST_SECTION_CHARS);
    if sections[index].is_empty() {
        sections[index] = newest;
        return;
    }
    let newest_chars = newest.chars().count();
    if newest_chars.saturating_add(1) >= FAST_SECTION_CHARS {
        sections[index] = newest;
        return;
    }
    // Prefer the newly observed fact when a carried prior note already fills
    // the section. Retain as much older context as fits ahead of it.
    let older_budget = FAST_SECTION_CHARS - newest_chars - 1;
    let older = bounded_excerpt(&sections[index], older_budget);
    sections[index] = format!("{older}\n{newest}");
}

fn bounded_excerpt(text: &str, max_chars: usize) -> String {
    if max_chars == 0 {
        return String::new();
    }
    let mut out = String::with_capacity(max_chars.min(text.len()));
    let mut count = 0usize;
    let mut whitespace = false;
    let mut truncated = false;
    for ch in text.trim().chars() {
        if count >= max_chars {
            truncated = true;
            break;
        }
        if ch.is_whitespace() {
            whitespace = !out.is_empty();
            continue;
        }
        if whitespace {
            if count >= max_chars {
                truncated = true;
                break;
            }
            out.push(' ');
            count += 1;
            whitespace = false;
        }
        if !ch.is_control() {
            if count >= max_chars {
                truncated = true;
                break;
            }
            out.push(ch);
            count += 1;
        }
    }
    if truncated {
        if count == max_chars {
            out.pop();
        }
        out.push('…');
    }
    out
}

fn collect_workspace_ledger(window: &[ChatMsg]) -> WorkspaceLedger {
    let mut ledger = WorkspaceLedger::default();
    for message in window {
        for line in message
            .content
            .lines()
            .filter(|_| is_compaction_note(message))
        {
            let Some(raw) = line.trim().strip_prefix(WORKSPACE_LEDGER_PREFIX) else {
                continue;
            };
            let Ok(value) = serde_json::from_str::<serde_json::Value>(raw) else {
                continue;
            };
            for path in value
                .get("read")
                .and_then(|paths| paths.as_array())
                .into_iter()
                .flatten()
                .filter_map(|path| path.as_str())
            {
                push_ledger_path(&mut ledger.read, path);
            }
            for path in value
                .get("modified")
                .and_then(|paths| paths.as_array())
                .into_iter()
                .flatten()
                .filter_map(|path| path.as_str())
            {
                push_ledger_path(&mut ledger.modified, path);
            }
        }

        for call in message.tool_calls.iter() {
            match call.name.as_str() {
                "read_file" | "list_dir" | "outline" | "defs" | "grep" | "find_files"
                | "file_search" => {
                    if let Some(path) = call.args.get("path").and_then(|path| path.as_str()) {
                        push_ledger_path(&mut ledger.read, path);
                    }
                }
                "write_file" | "str_replace" | "multi_edit" => {
                    if let Some(path) = call.args.get("path").and_then(|path| path.as_str()) {
                        push_ledger_path(&mut ledger.modified, path);
                    }
                }
                "apply_patch" => {
                    if let Some(diff) = call.args.get("diff").and_then(|diff| diff.as_str()) {
                        collect_patch_paths(diff, &mut ledger.modified);
                    }
                }
                _ => {}
            }
        }
    }
    ledger
}

fn push_ledger_path(paths: &mut Vec<String>, raw: &str) {
    let path: String = raw
        .trim()
        .chars()
        .filter(|ch| !ch.is_control())
        .take(WORKSPACE_LEDGER_PATH_CHARS)
        .collect();
    if path.is_empty() {
        return;
    }
    if let Some(index) = paths.iter().position(|existing| existing == &path) {
        paths.remove(index);
    }
    paths.push(path);
    if paths.len() > WORKSPACE_LEDGER_PATH_LIMIT {
        paths.remove(0);
    }
}

fn collect_patch_paths(diff: &str, modified: &mut Vec<String>) {
    for line in diff.lines() {
        let candidate = [
            "*** Add File: ",
            "*** Update File: ",
            "*** Delete File: ",
            "+++ b/",
            "--- a/",
        ]
        .iter()
        .find_map(|prefix| line.strip_prefix(prefix));
        if let Some(path) = candidate {
            push_ledger_path(modified, path);
        }
    }
}

fn append_workspace_ledger(note: &mut String, ledger: &WorkspaceLedger) {
    if ledger.read.is_empty() && ledger.modified.is_empty() {
        return;
    }
    let serialized = serde_json::json!({
        "read": ledger.read,
        "modified": ledger.modified,
    });
    note.push_str(&format!("\n\n{WORKSPACE_LEDGER_PREFIX}{serialized}"));
}

fn collect_verification_ledger(window: &[ChatMsg]) -> VerificationLedger {
    let mut ledger = VerificationLedger::default();
    let mut pending: HashMap<String, PendingEvidence> = HashMap::new();

    for message in window {
        // Only internal summary carriers restore continuity metadata. User,
        // tool, and ordinary assistant marker lookalikes remain ordinary prose.
        if is_compaction_note(message) {
            for line in message.content.lines() {
                let Some(raw) = line.trim().strip_prefix(VERIFICATION_LEDGER_PREFIX) else {
                    continue;
                };
                let Ok(value) = serde_json::from_str::<serde_json::Value>(raw) else {
                    continue;
                };
                let Some(entries) = value.get("entries").and_then(|entries| entries.as_array())
                else {
                    continue;
                };
                // A marker is a complete snapshot at this point in history, not
                // a delta. Replacing avoids duplication over repeated compacts.
                ledger.entries.clear();
                for entry in entries {
                    let Some(tool) = entry.get("tool").and_then(|tool| tool.as_str()) else {
                        continue;
                    };
                    let Some(outcome) = entry.get("outcome").and_then(|outcome| outcome.as_str())
                    else {
                        continue;
                    };
                    if !matches!(outcome, "passed" | "failed" | "inconclusive") {
                        continue;
                    }
                    push_verification_evidence(&mut ledger, tool, outcome);
                }
            }
        }

        for call in message.tool_calls.iter() {
            let mutation = is_mutation_call(call);
            let verification = crate::harness::is_verification_call(call);
            if mutation || verification {
                pending.insert(
                    call.id.clone(),
                    PendingEvidence {
                        call: call.clone(),
                        mutation,
                        verification,
                    },
                );
            }
        }

        let Some(call_id) = message.tool_call_id.as_deref() else {
            continue;
        };
        let Some(call) = pending.remove(call_id) else {
            continue;
        };
        let denied = message.content.starts_with("action capsule denied");
        if call.mutation && !denied && !is_error_result(&message.content) {
            ledger.entries.clear();
        }
        if call.verification && !denied {
            let outcome = if is_error_result(&message.content) {
                Some(crate::harness::VerificationOutcome::Failed)
            } else {
                verification_outcome(&call.call, &message.content)
            }
            .unwrap_or(crate::harness::VerificationOutcome::Inconclusive);
            push_verification_evidence(&mut ledger, &call.call.name, outcome.as_str());
        }
    }
    ledger
}

fn sanitize_verification_tool(raw: &str) -> String {
    raw.trim()
        .chars()
        .filter(|ch| !ch.is_control())
        .take(VERIFICATION_TOOL_CHARS)
        .collect()
}

fn push_verification_evidence(ledger: &mut VerificationLedger, tool: &str, outcome: &str) {
    let tool = sanitize_verification_tool(tool);
    if tool.is_empty() {
        return;
    }
    ledger.entries.push(VerificationEvidence {
        tool,
        outcome: outcome.to_string(),
    });
    if ledger.entries.len() > VERIFICATION_LEDGER_LIMIT {
        ledger.entries.remove(0);
    }
}

fn append_verification_ledger(note: &mut String, ledger: &VerificationLedger) {
    if ledger.entries.is_empty() {
        return;
    }
    let serialized = serde_json::json!({ "entries": ledger.entries });
    note.push_str(&format!("\n\n{VERIFICATION_LEDGER_PREFIX}{serialized}"));
}

fn collect_invoked_skills(window: &[ChatMsg]) -> InvokedSkillsLedger {
    let mut ledger = InvokedSkillsLedger::default();
    let mut pending: HashMap<String, String> = HashMap::new();
    for message in window {
        if is_compaction_note(message) {
            for line in message.content.lines() {
                let Some(raw) = line.trim().strip_prefix(INVOKED_SKILLS_PREFIX) else {
                    continue;
                };
                let Ok(value) = serde_json::from_str::<serde_json::Value>(raw) else {
                    continue;
                };
                let Some(skills) = value.get("skills").and_then(|skills| skills.as_array()) else {
                    continue;
                };
                ledger.skills.clear();
                for skill in skills.iter().filter_map(|skill| skill.as_str()) {
                    push_invoked_skill(&mut ledger, skill);
                }
            }
        }

        for call in message.tool_calls.iter() {
            if call.name != "skill" {
                continue;
            }
            if let Some(name) = call.args.get("name").and_then(|name| name.as_str()) {
                let name = sanitize_invoked_skill(name);
                if !name.is_empty() {
                    pending.insert(call.id.clone(), name);
                }
            }
        }
        let Some(call_id) = message.tool_call_id.as_deref() else {
            continue;
        };
        let Some(skill) = pending.remove(call_id) else {
            continue;
        };
        if !is_error_result(&message.content)
            && !message.content.starts_with("action capsule denied")
        {
            push_invoked_skill(&mut ledger, &skill);
        }
    }
    ledger
}

fn sanitize_invoked_skill(raw: &str) -> String {
    raw.trim()
        .chars()
        .filter(|ch| !ch.is_control())
        .take(INVOKED_SKILL_CHARS)
        .collect()
}

fn push_invoked_skill(ledger: &mut InvokedSkillsLedger, raw: &str) {
    let skill = sanitize_invoked_skill(raw);
    if skill.is_empty() {
        return;
    }
    if let Some(index) = ledger.skills.iter().position(|existing| existing == &skill) {
        ledger.skills.remove(index);
    }
    ledger.skills.push(skill);
    if ledger.skills.len() > INVOKED_SKILLS_LIMIT {
        ledger.skills.remove(0);
    }
}

fn append_invoked_skills(note: &mut String, ledger: &InvokedSkillsLedger) {
    if ledger.skills.is_empty() {
        return;
    }
    let serialized = serde_json::json!({ "skills": ledger.skills });
    note.push_str(&format!("\n\n{INVOKED_SKILLS_PREFIX}{serialized}"));
}

fn collect_plan_ledger(window: &[ChatMsg]) -> Option<PlanLedger> {
    let mut current = None;
    let mut pending = std::collections::HashSet::new();
    let mut authorized = std::collections::HashSet::new();

    for message in window {
        // The internal summary binds a bounded Assistant-role snapshot by hash;
        // it never carries todo text itself. User/tool/ordinary assistant markers
        // cannot authorize anything.
        if is_compaction_note(message) {
            for line in message.content.lines() {
                let Some(raw) = line.trim().strip_prefix(PLAN_PROOF_PREFIX) else {
                    continue;
                };
                let Ok(value) = serde_json::from_str::<serde_json::Value>(raw) else {
                    continue;
                };
                let Some(hash) = value.get("sha256").and_then(|hash| hash.as_str()) else {
                    continue;
                };
                if hash.len() == 64 && hash.bytes().all(|byte| byte.is_ascii_hexdigit()) {
                    authorized.insert(hash.to_ascii_lowercase());
                }
            }
        }
        if message.role == ChatRole::Assistant && message.content.starts_with(PLAN_SNAPSHOT_PREFIX)
        {
            let hash = crate::cut::sha256_hex(message.content.as_bytes());
            if authorized.remove(&hash)
                && let Some(plan) = parse_plan_snapshot(&message.content)
            {
                current = Some(plan);
            }
        }

        for call in message.tool_calls.iter() {
            if call.name == "todo" {
                pending.insert(call.id.clone());
            }
        }
        let Some(call_id) = message.tool_call_id.as_deref() else {
            continue;
        };
        if !pending.remove(call_id)
            || is_error_result(&message.content)
            || message.content.starts_with("action capsule denied")
        {
            continue;
        }
        if let Some(plan) = parse_todo_state_result(&message.content) {
            current = Some(plan);
        }
    }
    current
}

fn parse_todo_state_result(result: &str) -> Option<PlanLedger> {
    let raw = result
        .lines()
        .last()?
        .trim()
        .strip_prefix(crate::tools::plan::TODO_STATE_PREFIX)?;
    parse_plan_json(raw)
}

fn parse_plan_snapshot(snapshot: &str) -> Option<PlanLedger> {
    parse_plan_json(snapshot.strip_prefix(PLAN_SNAPSHOT_PREFIX)?)
}

fn parse_plan_json(raw: &str) -> Option<PlanLedger> {
    let value = serde_json::from_str::<serde_json::Value>(raw).ok()?;
    let next_id = usize::try_from(value.get("next_id")?.as_u64()?).ok()?;
    let prior_omitted = value
        .get("omitted")
        .and_then(|omitted| omitted.as_u64())
        .map(usize::try_from)
        .transpose()
        .ok()?
        .unwrap_or(0);
    let values = value.get("items")?.as_array()?;
    let mut seen = std::collections::HashSet::new();
    let mut items = Vec::with_capacity(values.len().min(PLAN_ITEM_LIMIT + 1));
    for value in values {
        let id = usize::try_from(value.get("id")?.as_u64()?).ok()?;
        let done = value.get("done")?.as_bool()?;
        let text = sanitize_plan_text(value.get("text")?.as_str()?);
        if id == 0 || id > next_id || text.is_empty() || !seen.insert(id) {
            return None;
        }
        items.push(PlanItem { id, text, done });
    }
    Some(bound_plan(items, next_id, prior_omitted))
}

fn sanitize_plan_text(raw: &str) -> String {
    let truncated = raw.trim().chars().count() > PLAN_TEXT_CHARS;
    let limit = if truncated {
        PLAN_TEXT_CHARS.saturating_sub(1)
    } else {
        PLAN_TEXT_CHARS
    };
    let mut text = String::new();
    for (index, ch) in raw.trim().chars().enumerate() {
        if index >= limit {
            break;
        }
        text.push(if ch.is_control() { ' ' } else { ch });
    }
    let text = text.split_whitespace().collect::<Vec<_>>().join(" ");
    if truncated && !text.is_empty() {
        format!("{}…", text.trim_end_matches('…'))
    } else {
        text
    }
}

fn bound_plan(items: Vec<PlanItem>, next_id: usize, prior_omitted: usize) -> PlanLedger {
    let total = items.len();
    let mut selected = std::collections::HashSet::new();
    for item in items.iter().filter(|item| !item.done).take(PLAN_ITEM_LIMIT) {
        selected.insert(item.id);
    }
    if selected.len() < PLAN_ITEM_LIMIT {
        for item in items.iter().rev().filter(|item| item.done) {
            selected.insert(item.id);
            if selected.len() == PLAN_ITEM_LIMIT {
                break;
            }
        }
    }
    let items = items
        .into_iter()
        .filter(|item| selected.contains(&item.id))
        .collect::<Vec<_>>();
    PlanLedger {
        next_id,
        omitted: prior_omitted.saturating_add(total.saturating_sub(items.len())),
        items,
    }
}

fn render_plan_snapshot(plan: &PlanLedger) -> String {
    let serialized = serde_json::to_string(plan).expect("plan ledger is serializable");
    format!("{PLAN_SNAPSHOT_PREFIX}{serialized}")
}

fn render_plan_artifacts(plan: &PlanLedger, state_budget: usize) -> Option<(String, String)> {
    let max_bytes = PLAN_SNAPSHOT_MAX_TOKENS
        .min(state_budget / 4)
        .saturating_mul(4);
    let mut bounded = plan.clone();
    loop {
        let snapshot = render_plan_snapshot(&bounded);
        let proof = plan_proof(&snapshot);
        if snapshot.len().saturating_add(proof.len()).saturating_add(4) <= max_bytes {
            return Some((snapshot, proof));
        }
        if bounded.items.is_empty() {
            return None;
        }
        let drop_index = bounded
            .items
            .iter()
            .rposition(|item| item.done)
            .unwrap_or(bounded.items.len() - 1);
        bounded.items.remove(drop_index);
        bounded.omitted = bounded.omitted.saturating_add(1);
    }
}

fn plan_proof(snapshot: &str) -> String {
    let proof = serde_json::json!({
        "sha256": crate::cut::sha256_hex(snapshot.as_bytes()),
    });
    format!("{PLAN_PROOF_PREFIX}{proof}")
}

fn append_plan_proof(note: &mut String, proof: &str) {
    note.push_str(&format!("\n\n{proof}"));
}

/// Summarize `window` into `(section, content)` pairs, adaptively choosing a
/// single pass or a parallel map-reduce by estimated size. `None` if the
/// summarizer fails or returns nothing parseable.
pub fn summarize(
    club: &dyn Club,
    window: &[ChatMsg],
    chunk_threshold: usize,
) -> Option<Vec<(String, String)>> {
    let threshold = chunk_threshold.max(1);
    let raw = if estimate_tokens(window) <= threshold {
        // Single pass: the whole window fits one summarizer call.
        let prompt = structured_prompt(&render_transcript(window));
        nonempty(club.respond(&prompt))?
    } else {
        // Map: summarize each chunk concurrently into dense notes.
        let transcripts = chunk_transcripts(window, threshold);
        let map_prompts: Vec<String> = transcripts
            .iter()
            .map(|transcript| map_prompt(transcript))
            .collect();
        let notes: Vec<String> = fan_out(club, &map_prompts)
            .into_iter()
            .filter_map(|r| r.ok())
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
            .collect();
        if notes.is_empty() {
            return None;
        }
        // Reduce: fold the chunk notes down until they fit the per-call ceiling
        // (many/large chunks can themselves overflow a single reduce pass), then
        // distill the merged notes into the structured sections.
        let merged = condense_to_fit(club, notes, threshold)?;
        let prompt = structured_prompt(&merged);
        nonempty(club.respond(&prompt))?
    };
    let parsed = parse_sections(&raw);
    if !parsed.is_empty() {
        return Some(parsed);
    }
    // The summarizer ignored our headers (small local models sometimes do) — don't
    // throw the summary away; keep the whole reply as one free-form note.
    let body = raw.trim();
    if body.is_empty() {
        None
    } else {
        Some(vec![("Notes".to_string(), body.to_string())])
    }
}

/// The palace "wing" for `workspace`, keyed by both human-readable repository
/// name and canonical project identity. A basename alone is not a boundary:
/// unrelated checkouts frequently share names, and `/cd` does not mutate the
/// process cwd. The key makes those wings disjoint and leaves legacy basename
/// wings unread unless a future explicit migration imports them.
pub fn project_wing_for(workspace: &std::path::Path) -> String {
    let identity = crate::workspace_store::repo_identity(workspace);
    let name = identity
        .root
        .file_name()
        .map(|part| part.to_string_lossy().into_owned())
        .filter(|part| !part.is_empty())
        .unwrap_or_else(|| "project".to_string());
    format!("{name}--{}", identity.key)
}

/// Provenance tag for a deposited drawer's `source` field: the session id, plus
/// what produced the drawer (`compact` = manual `/compact`, `auto-compact` = the
/// budget trigger, `swarm` = a swarm synthesis). Tagging every drawer with the
/// session id makes it attributable to *where* it came from — both manual and
/// auto compaction now share this instead of the old bare `"auto-compact"`
/// literal that lost the session. Falls back to just `kind` when no session id
/// is set (e.g. a registry built without one in tests).
pub fn provenance(session_id: &str, kind: &str) -> String {
    let id = session_id.trim();
    if id.is_empty() {
        kind.to_string()
    } else {
        format!("{id}/{kind}")
    }
}

/// Split `window` into sub-slices each estimated at no more than `chunk_tokens`,
/// on message boundaries (never splitting a tool call from its result mid-chunk).
/// A single oversized message becomes its own chunk rather than being dropped.
fn chunk_window(window: &[ChatMsg], chunk_tokens: usize) -> Vec<&[ChatMsg]> {
    let mut chunks = Vec::new();
    let mut start = 0usize;
    let mut acc = 0usize;
    for i in 0..window.len() {
        let t = estimate_tokens(&window[i..i + 1]);
        // A tool result answers the preceding assistant call batch. Let that
        // chunk exceed its soft budget until the entire consecutive result
        // run is present; the next non-Tool message is the first safe cut.
        if acc > 0 && acc + t > chunk_tokens && window[i].role != ChatRole::Tool {
            chunks.push(&window[start..i]);
            start = i;
            acc = 0;
        }
        acc += t;
    }
    if start < window.len() {
        chunks.push(&window[start..]);
    }
    chunks
}

/// Render message-bounded chunks, then losslessly split any single oversized
/// transcript. The old chunker isolated an oversized message but still sent it
/// whole, so one giant user turn or tool payload could overflow every map call.
/// These segments are summarizer text (not provider tool-protocol messages), so
/// splitting is safe and the reduce pass restores one structured summary.
fn chunk_transcripts(window: &[ChatMsg], chunk_tokens: usize) -> Vec<String> {
    let chunks = chunk_window(window, chunk_tokens.max(1));
    let reserve = if chunk_tokens > 512 { 256 } else { 0 };
    let max_bytes = chunk_tokens
        .saturating_sub(reserve)
        .max(1)
        .saturating_mul(4);
    let mut transcripts = Vec::new();
    for chunk in chunks {
        let rendered = render_transcript(chunk);
        if rendered.len() <= max_bytes {
            transcripts.push(rendered);
            continue;
        }
        let mut start = 0usize;
        while start < rendered.len() {
            let mut end = start.saturating_add(max_bytes).min(rendered.len());
            while end > start && !rendered.is_char_boundary(end) {
                end -= 1;
            }
            // max_bytes can land inside the first multi-byte scalar only when a
            // microscopic test threshold is used; advance to one complete char.
            if end == start {
                end = rendered[start..]
                    .char_indices()
                    .nth(1)
                    .map(|(offset, _)| start + offset)
                    .unwrap_or(rendered.len());
            }
            transcripts.push(rendered[start..end].to_string());
            start = end;
        }
    }
    transcripts
}

/// Max summarizer calls in flight at once. The chunk count is data-driven (window
/// tokens ÷ ceiling), so without a cap a huge window would spawn dozens of threads
/// and simultaneous HTTP requests at the backend. Mirrors the swarm's bounded
/// width. Override with `ANGEL_COMPACT_FANOUT`.
fn fanout_width() -> usize {
    std::env::var("ANGEL_COMPACT_FANOUT")
        .ok()
        .and_then(|s| s.trim().parse::<usize>().ok())
        .unwrap_or(8)
        .max(1)
}

/// Run `prompts` over a shared `&dyn Club` (which is `Sync`), at most
/// [`fanout_width`] concurrently — a vLLM backend sees each batch as simultaneous
/// requests and batches them, the way the swarm fan-out does, without an unbounded
/// thread/connection storm. Results preserve input order.
fn fan_out(club: &dyn Club, prompts: &[String]) -> Vec<Result<String, String>> {
    let width = fanout_width();
    let mut out = Vec::with_capacity(prompts.len());
    for batch in prompts.chunks(width) {
        let batch_out: Vec<Result<String, String>> = std::thread::scope(|s| {
            let handles: Vec<_> = batch
                .iter()
                .map(|p| s.spawn(move || club.respond(p)))
                .collect();
            handles
                .into_iter()
                .map(|h| {
                    h.join()
                        .unwrap_or_else(|_| Err("summary worker thread panicked".to_string()))
                })
                .collect()
        });
        out.extend(batch_out);
    }
    out
}

/// Char/4 token estimate for a bare string (the same proxy as
/// [`estimate_tokens`], which only takes a message slice).
fn est_str(s: &str) -> usize {
    s.len() / 4
}

/// Fold a set of map notes down until their joined size fits `threshold`, so the
/// final structured reduce pass stays inside the summarizer's window even when the
/// window produced many (or large) chunk notes. Each round groups the notes into
/// batches that each fit, summarizes every batch concurrently, and repeats —
/// bounded by a small guard so it always terminates. `None` if a round wipes out
/// all notes (every summary failed).
fn condense_to_fit(club: &dyn Club, mut notes: Vec<String>, threshold: usize) -> Option<String> {
    const MAX_ROUNDS: usize = 4;
    let sep = "\n\n---\n\n";
    for _ in 0..MAX_ROUNDS {
        let merged = notes.join(sep);
        if notes.len() <= 1 || est_str(&merged) <= threshold {
            return Some(merged);
        }
        // Group notes greedily so each batch fits the ceiling, then re-summarize.
        let mut groups: Vec<String> = Vec::new();
        let mut cur = String::new();
        for n in &notes {
            if !cur.is_empty() && est_str(&cur) + est_str(n) > threshold {
                groups.push(std::mem::take(&mut cur));
            }
            if !cur.is_empty() {
                cur.push_str(sep);
            }
            cur.push_str(n);
        }
        if !cur.is_empty() {
            groups.push(cur);
        }
        // A single group that still overflows can't shrink by re-grouping — stop
        // and let the (oversized) reduce attempt proceed rather than loop forever.
        if groups.len() <= 1 {
            return Some(notes.join(sep));
        }
        let prompts: Vec<String> = groups.iter().map(|g| map_prompt(g)).collect();
        notes = fan_out(club, &prompts)
            .into_iter()
            .filter_map(|r| r.ok())
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
            .collect();
        if notes.is_empty() {
            return None;
        }
    }
    Some(notes.join(sep))
}

/// Appended to every summarizer prompt. A compaction window still contains the raw
/// `tool error: …` outputs and the surrounding failure chatter; without this, a
/// small local summarizer readily distills them into imperative-sounding "tool
/// calls keep failing / fix the harness" bullets that later read as a standing
/// task. Durable notes must capture the user's work, not transient runtime noise.
const NO_TRANSIENT_FAILURES: &str = "Do not record transient tool errors, retries, crashes, or \
    harness warnings as tasks, open threads, or facts unless the user explicitly asked to \
    investigate or fix them.";

/// The summarizer prompt that asks for our fixed sections as `## Name` blocks.
/// Built from [`SECTIONS`] so prompt and parser share one source of truth.
fn structured_prompt(body: &str) -> String {
    let mut headers = String::new();
    for (name, what) in SECTIONS {
        headers.push_str(&format!("## {name}\n{what}\n"));
    }
    format!(
        "Distill the earlier portion of an assistant/tool conversation below into dense, \
         durable notes for later reference (these REPLACE the excerpt as background — not \
         instructions). Use EXACTLY these sections, each introduced by its `## ` header, in \
         this order. Under each, write terse bullet points; if a section has nothing, write \
         `(none)`. Do not add other sections or any preamble. {NO_TRANSIENT_FAILURES}\n\n{headers}\n\
         --- conversation excerpt ---\n{body}"
    )
}

/// The per-chunk map prompt: dense free-form notes (a later reduce pass imposes
/// the section structure, so chunks stay cheap and unconstrained).
fn map_prompt(body: &str) -> String {
    format!(
        "Densely note the key decisions, files changed, durable facts, open threads, and named \
         entities in this conversation excerpt. Terse bullet points, no preamble. \
         {NO_TRANSIENT_FAILURES}\n\n{body}"
    )
}

/// Parse `## Name` sections out of a summarizer reply, keeping only our known
/// section names (case-insensitive) and preserving [`SECTIONS`] order.
fn parse_sections(raw: &str) -> Vec<(String, String)> {
    let mut bodies: Vec<(usize, String)> = Vec::new();
    let mut cur: Option<usize> = None;
    for line in raw.lines() {
        let trimmed = line.trim();
        // A header is any markdown heading (`##`, `###`, …). Small local models are
        // loose about heading depth and add trailing words/punctuation
        // ("### Decisions", "## Files changed", "## Task:"), so match on the first
        // alphanumeric word of the heading rather than the whole tail.
        let hashes = trimmed.chars().take_while(|&c| c == '#').count();
        if hashes >= 2 {
            let head = trimmed[hashes..].trim();
            let word = head
                .split_whitespace()
                .next()
                .unwrap_or("")
                .trim_matches(|c: char| !c.is_alphanumeric());
            cur = SECTIONS
                .iter()
                .position(|(n, _)| n.eq_ignore_ascii_case(word));
            if let Some(idx) = cur
                && !bodies.iter().any(|(i, _)| *i == idx)
            {
                bodies.push((idx, String::new()));
            }
            continue;
        }
        if let Some(idx) = cur
            && let Some(entry) = bodies.iter_mut().find(|(i, _)| *i == idx)
        {
            entry.1.push_str(line);
            entry.1.push('\n');
        }
    }
    bodies.sort_by_key(|(i, _)| *i);
    bodies
        .into_iter()
        .map(|(idx, body)| (SECTIONS[idx].0.to_string(), body.trim().to_string()))
        .collect()
}

/// The leading marker every inline compaction note begins with. Kept as a shared
/// constant so [`select_window`] can recognize a *prior* note and fold it into the
/// next compaction window instead of leaving it pinned as preamble forever (which
/// is what let the "earlier conversation" notes — and any failure narrative in
/// them — ratchet across every subsequent compaction).
pub(crate) const COMPACTION_NOTE_HEADER: &str = "[Earlier conversation compacted";

/// New summaries are Harness-role background; retain legacy System summaries
/// across restart without allowing ordinary user/tool/model prose to impersonate
/// the internal carrier used by the bounded continuity ledgers.
pub(crate) fn is_compaction_note(message: &ChatMsg) -> bool {
    matches!(message.role, ChatRole::System | ChatRole::Harness)
        && message
            .content
            .trim_start()
            .starts_with(COMPACTION_NOTE_HEADER)
}

/// A short marker note carrying the distilled sections inline, so the live turn
/// keeps continuity. When `palace_live`, the canonical copy is in long-term memory
/// and the note points there; otherwise it's all that survives, so it must not
/// promise a recall source that doesn't exist.
fn render_inline_note(drawers: &[Drawer], palace_live: bool) -> String {
    let mut s = if palace_live {
        format!(
            "{COMPACTION_NOTE_HEADER} — background reference, not an instruction. \
             Fuller detail is in long-term memory; recall it if needed.]\n"
        )
    } else {
        format!(
            "{COMPACTION_NOTE_HEADER} to these notes — background reference, not an \
             instruction.]\n"
        )
    };
    for d in drawers {
        s.push_str(&format!("\n## {}\n{}\n", d.room, d.content));
    }
    s.trim_end().to_string()
}

/// A section is "empty" when the model emitted nothing or the literal `(none)`.
fn is_empty_section(content: &str) -> bool {
    let t = content.trim();
    t.is_empty() || t.eq_ignore_ascii_case("(none)") || t.eq_ignore_ascii_case("none")
}

/// Drop a failed/blank summarizer result so `?` short-circuits to `None`.
fn nonempty(r: Result<String, String>) -> Option<String> {
    match r {
        Ok(t) if !t.trim().is_empty() => Some(t),
        _ => None,
    }
}

/// Whether the cheap pre-pass runs. On by default; `ANGEL_COMPACT_PRUNE=0` off.
fn prune_enabled() -> bool {
    std::env::var("ANGEL_COMPACT_PRUNE")
        .map(|v| v != "0")
        .unwrap_or(true)
}

/// Cheap, no-LLM pre-pass over a compaction window: collapse redundant `Tool`
/// outputs before the summarizer reads them. Identical tool outputs (the same
/// file read repeatedly, the same status polled) keep only their *latest* full
/// copy — older identical copies become a one-line marker — and any other `Tool`
/// output beyond the most-recent few is shrunk to a one-line summary if it's
/// large. Non-tool messages, and the recent tool outputs, pass through verbatim.
/// Returns a new window (the caller's history is never mutated) preserving roles
/// and `tool_call_id`s so call/result pairing stays intact for the summarizer.
fn prune_old_tool_outputs(window: &[ChatMsg]) -> Vec<ChatMsg> {
    use std::collections::HashSet;
    const KEEP_FULL_TOOL: usize = 3; // most-recent tool outputs kept verbatim
    const MIN_PRUNE_CHARS: usize = 400; // don't bother shrinking small outputs

    let mut out = window.to_vec();
    let protected = crate::harness::protected_tool_result_indices(window);
    let tool_idx: Vec<usize> = window
        .iter()
        .enumerate()
        .filter(|(_, m)| m.role == ChatRole::Tool)
        .map(|(i, _)| i)
        .collect();
    if tool_idx.len() <= KEEP_FULL_TOOL {
        return out; // nothing old enough to be worth pruning
    }
    let keep_full: HashSet<usize> = tool_idx
        .iter()
        .rev()
        .take(KEEP_FULL_TOOL)
        .copied()
        .collect();
    let mut seen: HashSet<u64> = HashSet::new();
    // Walk newest → oldest so the *latest* of a set of identical outputs is kept.
    for &i in tool_idx.iter().rev() {
        let content = &window[i].content;
        if protected.contains(&i) {
            seen.insert(content_hash(content));
            continue;
        }
        if !seen.insert(content_hash(content)) {
            out[i].content = "[duplicate of a later identical tool output — elided]".into();
            continue;
        }
        if keep_full.contains(&i) {
            continue; // recent: keep verbatim
        }
        if content.len() >= MIN_PRUNE_CHARS {
            out[i].content = elide_tool_output(content).into();
        }
    }
    out
}

/// One-line stand-in for an elided older tool output, keeping enough of a fingerprint
/// (size + first line) that the summarizer can still reference it meaningfully.
fn elide_tool_output(content: &str) -> String {
    let lines = content.lines().count();
    let head: String = content
        .lines()
        .find(|l| !l.trim().is_empty())
        .unwrap_or("")
        .trim()
        .chars()
        .take(80)
        .collect();
    format!(
        "[older tool output elided — {} chars, {lines} lines; starts: \"{head}\"]",
        content.len()
    )
}

/// Stable content hash for dedup (order-independent identity of a tool output).
fn content_hash(s: &str) -> u64 {
    use std::hash::{Hash, Hasher};
    let mut h = std::collections::hash_map::DefaultHasher::new();
    s.hash(&mut h);
    h.finish()
}

#[cfg(test)]
#[path = "../../tests/cockpit/app/compaction__tests.rs"]
mod tests;
