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

use super::{DIGITS, DIRECTION, Route, ledger};
use crate::agent::club::{ChatMsg, Club, ClubReply, ToolDef};
use crate::agent::harness::Tool;
use serde_json::Value;
use std::cell::RefCell;
use std::path::{Path, PathBuf};
use std::sync::atomic::AtomicBool;

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

#[cfg(test)]
pub(crate) fn is_ledger_only(tools: &[ToolDef]) -> bool {
    match tools {
        [] => true,
        [only] => only.name == "read_file" && only.description == reader_def().description,
        _ => false,
    }
}

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

pub(crate) const MISSED_HOPS: usize = 2;

pub(crate) const READER_CLOSED: &str = "The ledger reader is closed for this request. Write your answer \
now, in the format asked for above, from what is already in this conversation. Do not call or write \
out any tool call.";

pub(crate) fn converse(
    workspace: &Path,
    messages: &[ChatMsg],
    mut hop: impl FnMut(&[ChatMsg], &[ToolDef]) -> Result<ClubReply, String>,
) -> Result<ClubReply, String> {
    let mut history = messages.to_vec();
    let reader = vec![reader_def()];
    let mut missed = 0;
    loop {
        let tools: &[ToolDef] = if missed < MISSED_HOPS { &reader } else { &[] };
        match hop(&history, tools)? {
            ClubReply::Calls(calls)
                if missed < MISSED_HOPS
                    && !calls.is_empty()
                    && calls.iter().all(|call| call.name == "read_file") =>
            {
                history.push(ChatMsg::assistant_calls_with_reasoning(calls.clone(), None));
                let mut landed = false;
                for call in &calls {
                    let result = match read(workspace, &call.args) {
                        Ok(page) => {
                            landed = true;
                            page
                        }
                        Err(error) => error,
                    };
                    history.push(ChatMsg::tool(call.id.clone(), result));
                }
                missed = if landed { 0 } else { missed + 1 };
                if missed == MISSED_HOPS {
                    history.push(ChatMsg::user(READER_CLOSED));
                }
            }
            other => return Ok(other),
        }
    }
}

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
    static WORKSPACE: RefCell<Option<PathBuf>> = const { RefCell::new(None) };
}

pub(crate) struct WorkspaceScope(Option<PathBuf>);

impl Drop for WorkspaceScope {
    fn drop(&mut self) {
        let previous = self.0.take();
        WORKSPACE.with(|slot| *slot.borrow_mut() = previous);
    }
}

pub(crate) fn enter(workspace: &Path) -> WorkspaceScope {
    WorkspaceScope(WORKSPACE.with(|slot| slot.replace(Some(workspace.to_path_buf()))))
}

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

pub(crate) fn recite(cells: &str) -> String {
    let mut out: Vec<String> = Vec::new();
    for address in ledger::addresses(cells).unwrap_or_default() {
        let Some(section) = address.section else {
            continue;
        };
        let route = Route::new(address.primary, section);
        let Some(sub) = super::find(route) else {
            continue;
        };
        match address.page {
            Some(page) => {
                if let Some(text) = DIGITS
                    .iter()
                    .position(|digit| *digit == page)
                    .and_then(|index| sub.pages.get(index))
                {
                    out.push(super::profile_text(&sub, text));
                }
            }
            None if !sub.pages.is_empty() => {
                out.push(super::profile_text(&sub, &sub.pages.join(" ")))
            }
            None => {}
        }
    }
    out.join(" ")
}
