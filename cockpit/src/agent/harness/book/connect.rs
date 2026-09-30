//! The connection: every seat reads the book. A seat that carries its own
//! tools reads `ledger://` through `read_file`; a seat with none — a consult,
//! the aggregator, the summarizer, a text-only worker, a bare spawn seat, a
//! mixture stage — is given the ledger reader alone: [`LedgerReader`] in a
//! turn-engine registry, or [`chat`] / [`converse`] for a direct club call,
//! which resolves its reads for as long as the seat reads and returns its
//! answer. So a braille prompt reaches every model the harness calls, and "no
//! tools" still means no workspace access.
//!
//! The ledger's evidence is kept per workspace: a turn enters its workspace
//! ([`enter`]) around the club call, so a connected seat deep inside that call
//! — a mixture stage on a worker thread that carried the scope over — reads
//! the evidence the turn recorded. A seat with no tool channel at all (a CLI
//! scout, an out-of-process peer) cannot be offered the reader; it hears the
//! pages themselves ([`recite`]), still read from the book.

use super::{DIGITS, DIRECTION, Route, ledger, primary};
use crate::agent::club::{ChatMsg, Club, ClubReply, ToolDef};
use crate::agent::harness::Tool;
use serde_json::Value;
use std::cell::RefCell;
use std::path::{Path, PathBuf};
use std::sync::atomic::AtomicBool;

/// The ledger reader's schema: `read_file`, `ledger://` paths only.
pub(crate) fn reader_def() -> ToolDef {
    ToolDef {
        name: "read_file".to_string(),
        description: format!(
            "Read the ledger (`{}` paths only); {DIRECTION}.",
            ledger::SCHEME
        ),
        params: serde_json::json!({
            "type": "object",
            "properties": {
                "path": {"type": "string", "description": "`ledger://<cells>`"}
            },
            "required": ["path"],
        }),
    }
}

fn read(workspace: &Path, args: &Value) -> Result<String, String> {
    let path = args.get("path").and_then(Value::as_str).unwrap_or_default();
    match path.trim_start().strip_prefix(ledger::SCHEME) {
        Some(rest) => ledger::read(workspace, rest),
        None => Err(format!("only `{}` paths are readable here", ledger::SCHEME)),
    }
}

/// Is this tool set the ledger reader alone (or nothing)? A connected seat's
/// surface: no workspace tools.
#[cfg(test)]
pub(crate) fn is_ledger_only(tools: &[ToolDef]) -> bool {
    match tools {
        [] => true,
        [only] => only.name == "read_file" && only.description == reader_def().description,
        _ => false,
    }
}

/// The ledger reader, for a registry that grants no tools.
pub(crate) struct LedgerReader {
    pub(crate) workspace: PathBuf,
}

impl Tool for LedgerReader {
    fn name(&self) -> &str {
        "read_file"
    }
    fn def(&self) -> ToolDef {
        reader_def()
    }
    fn call(&self, args: &Value) -> Result<String, String> {
        read(&self.workspace, args)
    }
}

/// The connected hop loop over any transport: `hop` makes one call with the
/// ledger reader offered, every hop, with no cap on how much the seat reads.
/// Ledger reads are resolved here; the seat's answer comes back, and so does
/// a call for any tool but the reader, for the caller to refuse its own way.
pub(crate) fn converse(
    workspace: &Path,
    messages: &[ChatMsg],
    mut hop: impl FnMut(&[ChatMsg], &[ToolDef]) -> Result<ClubReply, String>,
) -> Result<ClubReply, String> {
    let mut history = messages.to_vec();
    let tools = vec![reader_def()];
    loop {
        match hop(&history, &tools)? {
            ClubReply::Calls(calls)
                if !calls.is_empty() && calls.iter().all(|call| call.name == "read_file") =>
            {
                history.push(ChatMsg::assistant_calls_with_reasoning(calls.clone(), None));
                for call in &calls {
                    let result = read(workspace, &call.args).unwrap_or_else(|error| error);
                    history.push(ChatMsg::tool(call.id.clone(), result));
                }
            }
            other => return Ok(other),
        }
    }
}

/// A direct club call, connected: the seat's messages with the ledger reader
/// offered; its reads resolved until it answers.
pub(crate) fn chat(
    club: &dyn Club,
    workspace: &Path,
    messages: &[ChatMsg],
    effort: Option<&str>,
    cancel: &AtomicBool,
) -> Result<String, String> {
    let reply = converse(workspace, messages, |history, tools| {
        club.chat_streaming_with_effort(history, tools, effort, cancel, &mut |_| {})
    })?;
    match reply {
        ClubReply::Text(text) => Ok(text),
        // The ledger reader is the only tool here; asking for anything else is
        // the seat misreading its surface, surfaced as before.
        ClubReply::Calls(calls) => {
            let names = calls
                .iter()
                .map(|call| call.name.as_str())
                .collect::<Vec<_>>();
            Err(format!(
                "the seat requested a tool (only the ledger reader offered): {names:?}"
            ))
        }
    }
}

/// One prompt, connected: the single-message form of [`chat`].
pub(crate) fn respond(club: &dyn Club, workspace: &Path, prompt: &str) -> Result<String, String> {
    chat(
        club,
        workspace,
        &[ChatMsg::user(prompt)],
        None,
        &AtomicBool::new(false),
    )
}

thread_local! {
    /// The workspace whose ledger this thread's connected seats read.
    static WORKSPACE: RefCell<Option<PathBuf>> = const { RefCell::new(None) };
}

/// The ledger workspace entered on this thread, restored when dropped.
pub(crate) struct WorkspaceScope(Option<PathBuf>);

impl Drop for WorkspaceScope {
    fn drop(&mut self) {
        let previous = self.0.take();
        WORKSPACE.with(|slot| *slot.borrow_mut() = previous);
    }
}

/// Enter `workspace` for this thread's connected seats: a turn around its club
/// call, a worker thread carrying its caller's scope over.
pub(crate) fn enter(workspace: &Path) -> WorkspaceScope {
    WorkspaceScope(WORKSPACE.with(|slot| slot.replace(Some(workspace.to_path_buf()))))
}

/// The ledger workspace for a connected seat on this thread: the one entered,
/// else the process's own (`$ANGEL_WORKSPACE`, else the current directory).
pub(crate) fn workspace() -> PathBuf {
    WORKSPACE
        .with(|slot| slot.borrow().clone())
        .unwrap_or_else(|| {
            crate::agent::harness::resolve_workspace(
                None,
                crate::agent::harness::current_dir_workspace,
            )
        })
}

/// The pages at `cells` — a section gives all its pages, a page address that
/// page — verbatim, in order, joined by a space. For a seat with no tool
/// channel, which cannot be offered the reader.
pub(crate) fn recite(cells: &str) -> String {
    let mut out: Vec<&str> = Vec::new();
    for address in ledger::addresses(cells).unwrap_or_default() {
        let Some(section) = address.section else {
            continue;
        };
        let route = Route::new(address.primary, section);
        let Some(sub) = primary(address.primary)
            .and_then(|primary| primary.subs.iter().find(|sub| sub.route == route))
        else {
            continue;
        };
        match address.page {
            Some(page) => {
                if let Some(text) = DIGITS
                    .iter()
                    .position(|digit| *digit == page)
                    .and_then(|index| sub.pages.get(index))
                {
                    out.push(text);
                }
            }
            None => out.extend(sub.pages.iter().copied()),
        }
    }
    out.join(" ")
}
