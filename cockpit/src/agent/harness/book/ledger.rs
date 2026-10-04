//! The ledger: `read_file ledger://…` decodes the routing tree.
//!
//! `ledger://` is the table of contents, `ledger://⠧` one chapter with its
//! sections, `ledger://⠧⠁` one section (a route) with its pages and the latest
//! evidence recorded for it in this workspace, `ledger://⠞⠉⠃` one page, and
//! `ledger://⠧⠁⠧⠑⠟⠁` any run of addresses in order. A braille letter starts
//! an address; the digits after it pick the section and the page. A
//! personality type's pages are addresses and render in full. Read-only: only
//! the harness writes evidence.

use super::{DIGITS, Primary, Route, Sub, TOC, primary};
use std::collections::BTreeMap;
use std::path::Path;
use std::sync::Mutex;

pub(crate) const SCHEME: &str = "ledger://";

/// Latest evidence per (workspace, route).
static EVIDENCE: Mutex<BTreeMap<(String, Route), String>> = Mutex::new(BTreeMap::new());

const EVIDENCE_MAX_CHARS: usize = 6_000;

pub(crate) fn record(workspace: &Path, route: Route, evidence: &str) {
    let count = evidence.chars().count();
    let evidence = if count > EVIDENCE_MAX_CHARS {
        let tail = evidence
            .chars()
            .skip(count - EVIDENCE_MAX_CHARS)
            .collect::<String>();
        format!("…{tail}")
    } else {
        evidence.to_string()
    };
    if let Ok(mut store) = EVIDENCE.lock() {
        store.insert((workspace_key(workspace), route), evidence);
    }
}

fn recorded(workspace: &Path, route: Route) -> Option<String> {
    EVIDENCE
        .lock()
        .ok()
        .and_then(|store| store.get(&(workspace_key(workspace), route)).cloned())
}

fn workspace_key(workspace: &Path) -> String {
    workspace
        .canonicalize()
        .unwrap_or_else(|_| workspace.to_path_buf())
        .to_string_lossy()
        .into_owned()
}

/// Decode a ledger path for `read_file`.
pub(crate) fn read(workspace: &Path, rest: &str) -> Result<String, String> {
    let rest_trimmed = rest.trim().trim_matches('/');
    if rest_trimmed.is_empty() {
        return Ok(table_of_contents());
    }
    let addresses = addresses(rest_trimmed).ok_or_else(|| unknown(rest))?;
    addresses
        .iter()
        .map(|address| decode(workspace, address, 0))
        .collect::<Option<Vec<_>>>()
        .map(|entries| entries.join("\n"))
        .ok_or_else(|| unknown(rest))
}

/// One address: a chapter letter, then an optional section and page digit.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct Address {
    pub(crate) primary: char,
    pub(crate) section: Option<char>,
    pub(crate) page: Option<char>,
}

/// Split a run of cells into addresses: a braille letter starts each one.
pub(crate) fn addresses(cells: &str) -> Option<Vec<Address>> {
    let mut out: Vec<Address> = Vec::new();
    for cell in cells.chars() {
        if DIGITS.contains(&cell) {
            let last = out.last_mut()?;
            if last.section.is_none() {
                last.section = Some(cell);
            } else if last.page.is_none() {
                last.page = Some(cell);
            } else {
                return None;
            }
        } else {
            primary(cell)?;
            out.push(Address {
                primary: cell,
                section: None,
                page: None,
            });
        }
    }
    (!out.is_empty()).then_some(out)
}

fn decode(workspace: &Path, address: &Address, depth: usize) -> Option<String> {
    let primary = primary(address.primary)?;
    let Some(section) = address.section else {
        return Some(chapter(primary));
    };
    let route = Route::new(address.primary, section);
    let sub = super::find(route)?;
    match address.page {
        Some(page) => {
            let index = DIGITS.iter().position(|digit| *digit == page)?;
            let text = sub.pages.get(index)?;
            Some(format!(
                "{}{page}  {}",
                route.cells(),
                render_page(workspace, text, depth)?
            ))
        }
        None => Some(decode_route_sub(workspace, route, sub, depth)),
    }
}

/// A page is a verbatim sentence, or — in a personality type — addresses,
/// rendered in full.
fn render_page(workspace: &Path, text: &str, depth: usize) -> Option<String> {
    if !is_warpath_line(text) {
        return Some(text.to_string());
    }
    if depth > 2 {
        return Some(text.to_string());
    }
    addresses(text)?
        .iter()
        .map(|address| decode(workspace, address, depth + 1))
        .collect::<Option<Vec<_>>>()
        .map(|parts| format!("{text}\n{}", parts.join("\n")))
}

fn table_of_contents() -> String {
    let mut out = String::from(
        "# book of behaviors — a routing tree; observations and routes, never requirements\n\
         A chapter is one cell, a section two (a route), a page three; a warpath is up to three \
         routes in order.\n\n",
    );
    for primary in TOC {
        out.push_str(&format!(
            "{}  {} — {}\n",
            primary.cell, primary.name, primary.surface
        ));
    }
    out
}

fn chapter(primary: &'static Primary) -> String {
    let mut out = format!(
        "{}  {} — {}\n\n",
        primary.cell, primary.name, primary.surface
    );
    for sub in super::subs(primary) {
        out.push_str(&format!("{}  {}", sub.route.cells(), sub.signal));
        if !sub.action.is_empty() {
            out.push_str(&format!(" → {}", sub.action));
        }
        if !sub.pages.is_empty() {
            out.push_str(&format!(" ({} pages)", sub.pages.len()));
        }
        out.push('\n');
    }
    out
}

fn decode_route_sub(workspace: &Path, route: Route, sub: &Sub, depth: usize) -> String {
    let mut out = format!("{}  {}\n", route.cells(), sub.signal);
    if !sub.action.is_empty() {
        out.push_str(&format!("→ {}\n", sub.action));
    }
    if !sub.ideas.is_empty() {
        out.push_str(sub.ideas);
        out.push('\n');
    }
    for (index, page) in sub.pages.iter().enumerate() {
        let rendered = render_page(workspace, page, depth).unwrap_or_else(|| page.to_string());
        out.push_str(&format!("{}{}  {rendered}\n", route.cells(), DIGITS[index]));
    }
    if let Some(evidence) = recorded(workspace, route) {
        out.push_str("Latest evidence:\n");
        out.push_str(&evidence);
        out.push('\n');
    }
    out
}

fn unknown(rest: &str) -> String {
    let cells = TOC.iter().map(|primary| primary.cell).collect::<String>();
    format!("no ledger entry `{rest}`; primaries: {cells} ({SCHEME} for the table of contents)")
}

pub(crate) fn is_cell(ch: char) -> bool {
    ('\u{2800}'..='\u{28FF}').contains(&ch)
}

/// A line that is nothing but braille: a warpath. A loop route's own turn
/// leads with the warning sign.
pub(crate) fn is_warpath_line(line: &str) -> bool {
    let line = line.trim();
    let line = line
        .strip_prefix(super::l_loops::WARNING)
        .or_else(|| line.strip_prefix(super::d12467_sloptomizer::ATTENTION))
        .unwrap_or(line);
    !line.is_empty() && line.chars().all(is_cell)
}

/// Text with each braille word it carries — a route, or a run of page
/// addresses — replaced by the verbatim pages it names: a section's route
/// reads as all its pages. For the harness's own indexes (tool search), never
/// for the wire.
pub(crate) fn expand(text: &str) -> String {
    text.split_whitespace()
        .map(|word| {
            let Some(found) = is_warpath_line(word).then(|| addresses(word)).flatten() else {
                return word.to_string();
            };
            found
                .iter()
                .filter_map(|address| {
                    let route = Route::new(address.primary, address.section?);
                    let sub = super::find(route)?;
                    match address.page {
                        Some(page) => {
                            let index = DIGITS.iter().position(|digit| *digit == page)?;
                            sub.pages.get(index).map(|text| text.to_string())
                        }
                        None => Some(sub.pages.join(" ")),
                    }
                })
                .collect::<Vec<_>>()
                .join(" ")
        })
        .collect::<Vec<_>>()
        .join(" ")
}

/// A harness message that opens with a warpath: the stamps, and any facts
/// (a failing run's tail, a receipt) beside them as data.
pub(crate) fn is_warpath_message(content: &str) -> bool {
    content.lines().next().is_some_and(is_warpath_line)
}

/// Bytes of a trailing `\n<warpath>` line, 0 when there is none.
pub(crate) fn tail_bytes(content: &str) -> usize {
    match content.rsplit_once('\n') {
        Some((_, last)) if is_warpath_line(last) => last.len() + 1,
        _ => 0,
    }
}

/// Content with its warpath lines removed, for summaries: warpaths steer the
/// live tail only and must never be distilled into durable notes.
pub(crate) fn without_warpaths(content: &str) -> std::borrow::Cow<'_, str> {
    if !content.lines().any(is_warpath_line) {
        return std::borrow::Cow::Borrowed(content);
    }
    std::borrow::Cow::Owned(
        content
            .lines()
            .filter(|line| !is_warpath_line(line))
            .collect::<Vec<_>>()
            .join("\n"),
    )
}

/// Does a read_file path target the ledger?
pub(crate) fn is_ledger_path(path: &str) -> bool {
    path.trim_start().starts_with(SCHEME)
}
