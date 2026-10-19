//! The Claude seat profile: pages the first-party Anthropic seat
//! (`ANGEL_DRIVER=anthropic`) is never sent.
//!
//! Claude declines a request that orders its reasoning written out as visible
//! text (`stop_reason: refusal`, category `reasoning_extraction`), and two
//! pages order a line written before every tool call: [`OMITTED`]. The seat
//! leaves them out wherever the book reaches it: the introductions of the
//! system prompt's type, of a warpath, an advice turn or a re-introduction
//! after compaction ([`omitting`], around `introduction::introductions`), and
//! its ledger reads ([`strip_ledger_reads`]). A page is left out whole, never
//! reworded, and the pages after it keep their addresses. Every other seat
//! hears the book unchanged. `ANGEL_SEAT_PROFILE=0` sends the pages.

use super::{Route, Sub, ledger, m_method, w_workflow};
use crate::agent::club::{ChatMsg, ChatRole};
use std::cell::Cell;

/// The pages the Anthropic seat is never sent: a route and its page's index.
pub(crate) const OMITTED: &[(Route, usize)] =
    &[(m_method::REPAIR_BATCH, 3), (w_workflow::RESPONSE, 1)];

thread_local! {
    static ACTIVE: Cell<bool> = const { Cell::new(false) };
}

/// Whether the Anthropic seat stands on this profile (`ANGEL_SEAT_PROFILE`,
/// default on).
pub(crate) fn enabled() -> bool {
    crate::agent::harness::env_flag("ANGEL_SEAT_PROFILE", true)
}

/// `f` with [`OMITTED`] left out of what the book renders on this thread.
pub(crate) fn omitting<T>(f: impl FnOnce() -> T) -> T {
    struct Restore(bool);
    impl Drop for Restore {
        fn drop(&mut self) {
            ACTIVE.with(|active| active.set(self.0));
        }
    }
    let _restore = Restore(ACTIVE.with(|active| active.replace(true)));
    f()
}

/// Whether `text`, a page of `route`, is left out here. Only the book's own
/// page is: a cartridge's page on the same route is its own words.
pub(crate) fn omitted(route: Route, text: &str) -> bool {
    ACTIVE.with(Cell::get)
        && OMITTED
            .iter()
            .any(|&(omit, index)| omit == route && book_page(omit, index) == Some(text))
}

/// A route's pages as rendered here, in order.
pub(crate) fn kept(sub: &Sub) -> impl Iterator<Item = &'static str> + '_ {
    sub.pages
        .iter()
        .copied()
        .filter(|page| !omitted(sub.route, page))
}

fn book_page(route: Route, index: usize) -> Option<&'static str> {
    super::primary(route.primary)?
        .subs
        .iter()
        .find(|sub| sub.route == route)?
        .pages
        .get(index)
        .copied()
}

/// The seat's outbound copy, zipped 1:1 with `messages`: a tool's answer — a
/// ledger read, direct or inside a batch — drops each [`OMITTED`] page's line
/// (`ledger::page_line`). The history is untouched, and the copy is a pure
/// function of each message, so the cached prefix holds.
pub(crate) fn strip_ledger_reads(messages: &[ChatMsg], outbound: &mut [serde_json::Value]) {
    let lines: Vec<String> = OMITTED
        .iter()
        .filter_map(|&(route, index)| {
            Some(ledger::page_line(route, index, book_page(route, index)?))
        })
        .collect();
    let left_out = |line: &str| lines.iter().any(|page| page == line);
    for (message, wire) in messages.iter().zip(outbound.iter_mut()) {
        if message.role != ChatRole::Tool {
            continue;
        }
        let Some(text) = wire["content"].as_str() else {
            continue;
        };
        if text.split('\n').any(left_out) {
            let kept = text
                .split('\n')
                .filter(|line| !left_out(*line))
                .collect::<Vec<_>>()
                .join("\n");
            wire["content"] = serde_json::Value::String(kept);
        }
    }
}

#[cfg(test)]
#[path = "../../../../../tests/cockpit/harness/book__claude_seat_tests.rs"]
mod tests;
