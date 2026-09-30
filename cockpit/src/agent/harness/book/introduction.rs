//! The introduction: every route is introduced once, in English, where the
//! model first sees its stamp; after that the stamp rides alone.
//!
//! A model does not know the book. Measured on the braille-only wire, a local
//! Qwen and Sonnet both walked past every stamp without reading one, and a
//! model told to read them spent turns doing it. So the wire introduces each
//! stamp the first time it appears in the conversation, beside it, in the
//! fewest English words that carry it:
//!
//! - a personality type (`⠽`) is the model's core instructions: its routes'
//!   pages, verbatim, once;
//! - a page is its sentence;
//! - a route is its signal and its action, or its pages when it has no action
//!   and they are few and short (a larger route's pages are introduced one by
//!   one, as their own stamps appear);
//! - a voiced route (`VOICED`, the advisories carried over from 0.1.6) is all
//!   its pages, whatever their length, in their own second-person words.
//!
//! The first introduction opens with [`LEAD`], the one standing direction in
//! English. After that the cells ride alone; `ledger://` still decodes the
//! full text on demand, and a stamp the model decoded itself needs no
//! introduction.
//!
//! The introductions are appended to the outbound copy of a message only: the
//! history is untouched. Each is a pure function of the messages up to and
//! including its own, so the text a message carries never changes on a later
//! request and the cached prefix never moves. A conversation compacted past a
//! stamp's first sight introduces it again at its next appearance.

use super::{DIGITS, Sub, ledger, y_types};
use crate::agent::club::{ChatMsg, ChatRole};

/// The standing direction, said once with the first introduction.
pub(crate) const LEAD: &str = "Braille from the harness names the situation you are in. Each stamp is \
     introduced once in English where it first appears; after that it rides as its cells alone. Act \
     on it; `read_file ledger://<cells>` gives a route's full text.";

/// A route without an action shows its pages inline when it has this few and
/// they are this short; a larger route's pages are introduced one by one, as
/// their own stamps appear, so each page keeps its address.
const SHORT_PAGES: usize = 4;
const SHORT_PAGES_CHARS: usize = 400;

/// Whether the wire introduces stamps (`ANGEL_BOOK_INTRO`, default on): the
/// reactive, per-session legend that gives each stamp its English the first
/// time the model meets it. `0` sends stamps bare (an ablation arm).
pub(crate) fn enabled() -> bool {
    crate::agent::harness::env_flag("ANGEL_BOOK_INTRO", true)
}

/// `ANGEL_BOOK_INTRO=every`: a bench arm that gives every sighting its
/// English, not only the first — the same routes as English injected into
/// the prompt each time, to weigh the legend against.
fn every_sighting() -> bool {
    std::env::var("ANGEL_BOOK_INTRO").is_ok_and(|value| value.trim() == "every")
}

/// The conversation as the model should read it, for a transport that
/// serializes from messages: each stamp introduced once in English where it
/// first appears, appended to an outbound copy (the stored history never
/// changes). Only where the model holds the ledger reader, as on the Chat
/// Completions wire, so the stamps after an introduction stay decodable.
/// Every wire the harness speaks goes through this or [`apply`], so the legend
/// reaches the model whichever seat it drives.
pub(crate) fn introduced<'a>(
    messages: &'a [ChatMsg],
    tools: &[crate::agent::club::ToolDef],
) -> std::borrow::Cow<'a, [ChatMsg]> {
    if !enabled() || !tools.iter().any(|tool| tool.name == "read_file") {
        return std::borrow::Cow::Borrowed(messages);
    }
    let introductions = introductions(messages, None);
    if introductions.is_empty() {
        return std::borrow::Cow::Borrowed(messages);
    }
    let mut outbound = messages.to_vec();
    for (index, intro) in introductions {
        if let Some(message) = outbound.get_mut(index) {
            message.content = format!("{}\n\n{intro}", message.content).into();
        }
    }
    std::borrow::Cow::Owned(outbound)
}

/// What the model already knows: stamps introduced or decoded, and routes
/// whose pages it has seen in full.
#[derive(Default)]
struct Known {
    exact: Vec<ledger::Address>,
    full: Vec<ledger::Address>,
}

impl Known {
    fn covers(&self, address: &ledger::Address) -> bool {
        self.exact.contains(address)
            || self
                .full
                .iter()
                .any(|known| known.primary == address.primary && known.section == address.section)
    }

    fn note(&mut self, address: ledger::Address) {
        if !self.exact.contains(&address) {
            self.exact.push(address);
        }
    }

    fn note_full(&mut self, address: ledger::Address) {
        let section = ledger::Address {
            page: None,
            ..address
        };
        if !self.full.contains(&section) {
            self.full.push(section);
        }
    }
}

/// The introductions for one request: `(index, text)` for each message that
/// carries a stamp the model has not met, `index` counting `standing` (braille
/// a link adds as a system message at its own index) as one more message.
pub(crate) fn introductions(
    messages: &[ChatMsg],
    standing: Option<(usize, &str)>,
) -> Vec<(usize, String)> {
    introductions_at(messages, standing, every_sighting())
}

/// [`introductions`], at first sight only or (`every`) at every sighting.
fn introductions_at(
    messages: &[ChatMsg],
    standing: Option<(usize, &str)>,
    every: bool,
) -> Vec<(usize, String)> {
    let mut known = Known::default();
    let mut led = false;
    let mut ledger_answers: Vec<&str> = Vec::new();
    let mut out = Vec::new();
    let mut offset = 0;
    for (index, message) in messages.iter().enumerate() {
        if every {
            known = Known::default();
        }
        if let Some((at, text)) = standing
            && at == index
        {
            if let Some(intro) = introduce(text, &mut known, &mut led) {
                out.push((at, intro));
            }
            offset = 1;
        }
        if message.role == ChatRole::Assistant {
            // What the model decoded itself is known, and the answers are the
            // reading, not news.
            for call in message.tool_calls.iter() {
                if let Some(cells) = ledger_read(call) {
                    for address in ledger::addresses(cells).unwrap_or_default() {
                        known.note_full(address);
                    }
                    ledger_answers.push(call.id.as_str());
                }
            }
            continue;
        }
        if message.role == ChatRole::Tool
            && message
                .tool_call_id
                .as_deref()
                .is_some_and(|id| ledger_answers.contains(&id))
        {
            continue;
        }
        if let Some(intro) = introduce(&message.content, &mut known, &mut led) {
            out.push((index + offset, intro));
        }
    }
    if let Some((at, text)) = standing
        && at >= messages.len()
        && let Some(intro) = introduce(text, &mut known, &mut led)
    {
        out.push((at, intro));
    }
    out
}

/// Append each introduction to its outbound message's text.
pub(crate) fn apply(outbound: &mut [serde_json::Value], introductions: Vec<(usize, String)>) {
    for (index, intro) in introductions {
        let Some(wire) = outbound.get_mut(index) else {
            continue;
        };
        match &mut wire["content"] {
            serde_json::Value::String(text) => {
                text.push_str("\n\n");
                text.push_str(&intro);
            }
            serde_json::Value::Array(parts) => {
                if let Some(slot) = parts
                    .iter_mut()
                    .find(|part| part["type"] == "text")
                    .and_then(|part| part.get_mut("text"))
                {
                    let joined = format!("{}\n\n{intro}", slot.as_str().unwrap_or_default());
                    *slot = serde_json::Value::String(joined);
                }
            }
            _ => {}
        }
    }
}

fn introduce(text: &str, known: &mut Known, led: &mut bool) -> Option<String> {
    let mut lines = Vec::new();
    for address in stamps_in(text) {
        if known.covers(&address) {
            continue;
        }
        if let Some(entry) = entry(&address, known) {
            lines.push(entry);
        }
    }
    if lines.is_empty() {
        return None;
    }
    if !*led {
        lines.insert(0, LEAD.to_string());
        *led = true;
    }
    Some(lines.join("\n"))
}

/// One stamp's introduction, noting what it makes known.
fn entry(address: &ledger::Address, known: &mut Known) -> Option<String> {
    let sub = sub_of(address)?;
    let cells = cells_of(address);
    if let Some(page) = address.page {
        let index = DIGITS.iter().position(|digit| *digit == page)?;
        let text = sub.pages.get(index)?;
        known.note(*address);
        return Some(format!("{cells} {}", unslotted(text)));
    }
    if address.primary == y_types::CELL {
        known.note_full(*address);
        let mut paragraphs = Vec::new();
        for page in sub.pages {
            for inner in ledger::addresses(page).unwrap_or_default() {
                if known.covers(&inner) {
                    continue;
                }
                if let Some(text) = pages_of(&inner) {
                    paragraphs.push(text);
                }
                known.note_full(inner);
            }
        }
        return Some(format!("{cells}\n{}", paragraphs.join("\n")));
    }
    if super::is_voiced(sub.route) {
        // A route that speaks in full is met with every page, in its own
        // words. Only the route is known after it: a later page stamp is a
        // new sentence to a model that has already heard the whole.
        known.note(*address);
        let pages = sub
            .pages
            .iter()
            .map(|page| unslotted(page))
            .collect::<Vec<_>>();
        return Some(format!("{cells} {}", pages.join(" ")));
    }
    known.note(*address);
    let mut line = format!("{cells} {}", sub.signal);
    if !sub.action.is_empty() {
        line.push_str(&format!(" → {}", sub.action));
    } else if !sub.pages.is_empty()
        && sub.pages.len() <= SHORT_PAGES
        && sub
            .pages
            .iter()
            .map(|page| page.chars().count())
            .sum::<usize>()
            <= SHORT_PAGES_CHARS
        && !sub.pages.iter().any(|page| ledger::is_warpath_line(page))
    {
        let pages = sub
            .pages
            .iter()
            .map(|page| unslotted(page))
            .collect::<Vec<_>>();
        line.push_str(&format!(": {}", pages.join(" ")));
        known.note_full(*address);
    }
    Some(line)
}

/// A route's or a page's verbatim English.
fn pages_of(address: &ledger::Address) -> Option<String> {
    let sub = sub_of(address)?;
    match address.page {
        Some(page) => {
            let index = DIGITS.iter().position(|digit| *digit == page)?;
            sub.pages.get(index).map(|text| unslotted(text))
        }
        None => Some(
            sub.pages
                .iter()
                .map(|page| unslotted(page))
                .collect::<Vec<_>>()
                .join(" "),
        ),
    }
}

/// A page's English with each value slot (`{level}`, `{next_offset}`, `{}`)
/// read as `…`. The values ride beside the stamp as data; the model meets the
/// sentence, not the template syntax. A doubled brace (`{{connector_id}}`) is
/// literal text a page ported from another harness's prompt carries, and is
/// kept as written.
fn unslotted(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut rest = text;
    while let Some(open) = rest.find('{') {
        out.push_str(&rest[..open]);
        let tail = &rest[open + 1..];
        if tail.starts_with('{')
            && let Some(close) = tail.find("}}")
        {
            out.push_str(&rest[open..open + 1 + close + 2]);
            rest = &tail[close + 2..];
            continue;
        }
        match tail.find('}') {
            Some(close)
                if tail[..close]
                    .chars()
                    .all(|ch| ch.is_ascii_lowercase() || ch == '_') =>
            {
                out.push('…');
                rest = &tail[close + 1..];
            }
            _ => {
                out.push('{');
                rest = tail;
            }
        }
    }
    out.push_str(rest);
    out
}

/// The stamps a message carries: each braille run that opens a line (or a
/// bracket, a line inside a JSON string, or a compact JSON string value — a
/// tool's `"warpath":"⡪⠁"`) and names pages the book has.
/// Braille elsewhere — quoted file content, a recalled note broken with `·` —
/// is text, not a stamp.
fn stamps_in(text: &str) -> Vec<ledger::Address> {
    let mut out: Vec<ledger::Address> = Vec::new();
    let mut run = String::new();
    let mut run_opens = false;
    let mut previous: [char; 2] = ['\n', '\n'];
    for ch in text.chars().chain(std::iter::once(' ')) {
        if ledger::is_cell(ch) {
            if run.is_empty() {
                run_opens = matches!(previous[1], '\n' | '[' | super::l_loops::WARNING)
                    || (previous[0] == '\\' && previous[1] == 'n')
                    || (previous[0] == ':' && previous[1] == '"');
            }
            run.push(ch);
        } else if !run.is_empty() {
            if run_opens {
                for address in ledger::addresses(&run).unwrap_or_default() {
                    if sub_of(&address).is_some_and(|sub| page_exists(sub, &address))
                        && !out.contains(&address)
                    {
                        out.push(address);
                    }
                }
            }
            run.clear();
        }
        previous = [previous[1], ch];
    }
    out
}

fn ledger_read(call: &crate::agent::club::ToolCall) -> Option<&str> {
    if call.name != "read_file" {
        return None;
    }
    call.args
        .get("path")?
        .as_str()?
        .trim_start()
        .strip_prefix(ledger::SCHEME)
}

fn sub_of(address: &ledger::Address) -> Option<&'static Sub> {
    let route = super::Route::new(address.primary, address.section?);
    super::primary(address.primary)?
        .subs
        .iter()
        .find(|sub| sub.route == route)
}

fn page_exists(sub: &Sub, address: &ledger::Address) -> bool {
    address.page.is_none_or(|page| {
        DIGITS
            .iter()
            .position(|digit| *digit == page)
            .is_some_and(|index| index < sub.pages.len())
    })
}

fn cells_of(address: &ledger::Address) -> String {
    [Some(address.primary), address.section, address.page]
        .into_iter()
        .flatten()
        .collect()
}

#[cfg(test)]
#[path = "../../../../../tests/cockpit/harness/book__introduction_tests.rs"]
mod tests;
