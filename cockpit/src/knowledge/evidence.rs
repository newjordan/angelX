//! Shared provenance framing for recalled Caddy, dossier, Atlas and broker data.
//!
//! Redaction and delimiter escaping keep recalled text inside its evidence
//! block. This is prompt framing, not an execution sandbox: existing tool
//! policy remains responsible for refusing disallowed actions.

/// Fixed fence open, `⠎⠃`: its provenance and its instruction are the ledger
/// pages.
pub(crate) const EVIDENCE_FENCE_HEADER: &str = "⠎⠃";
/// Fixed fence close, the same route. Content occurrences are neutralised by
/// [`fence`], so the only close in a rendered block is the real one.
pub(crate) const EVIDENCE_FENCE_SENTINEL: &str = "⠎⠃";

/// Neutralise every sentinel-shaped run inside recalled content. Every run of
/// braille cells is broken with `·`, so recalled text can forge neither a
/// fence (`⠎⠃`, `⠎⠉`, …) nor a route, and the rendered block contains exactly
/// one sentinel — the real close. Bracketed role markers get the same `·`.
pub(crate) fn neutralize_sentinels(body: &str) -> String {
    let mut broken = String::with_capacity(body.len());
    let mut previous_cell = false;
    for ch in body.chars() {
        let cell = crate::agent::harness::book::ledger::is_cell(ch);
        if cell && previous_cell {
            broken.push('·');
        }
        broken.push(ch);
        previous_cell = cell;
    }
    // Fences written before the book carried bracketed prose; their shapes
    // stay neutralised for recalled text quoting them.
    let mut body = broken
        .replace("[/recalled-memory]", "[/recalled-memory·")
        .replace("[recalled-memory/v1", "[recalled-memory/v1·")
        .replace("[evidence store=", "[evidence·store=");
    for marker in [
        "[SYSTEM]",
        "[OPERATOR]",
        "[source ",
        "[/source]",
        "[/knowledge-broker]",
        "[/living-atlas]",
    ] {
        body = body.replace(marker, &marker.replacen('[', "[·", 1));
    }
    body
}

/// Strip only the framing of a broker-owned, already sanitized payload when
/// reselecting it. This prevents framing growth on every model hop.
pub(crate) fn unfence(body: &str) -> &str {
    let body = body.trim();
    if !body.starts_with(EVIDENCE_FENCE_HEADER) {
        return body;
    }
    body.splitn(3, '\n')
        .nth(2)
        .and_then(|body| body.strip_suffix(EVIDENCE_FENCE_SENTINEL))
        .map(str::trim)
        .unwrap_or(body)
}

/// Reduce a provenance attribute to characters that cannot forge the header
/// structure (mirrors the broker's `safe_attr`).
fn safe_attr(value: &str) -> String {
    value
        .chars()
        .filter(|c| {
            c.is_ascii_alphanumeric() || matches!(c, '_' | '-' | '.' | ':' | ',' | '/' | '=' | ' ')
        })
        .take(160)
        .collect()
}

/// A harness-rendered store's own route line — the dossier's frame and its
/// withheld count, the caddy card's headings
/// (`book::d456_knowledge::store_route`): a line that opens with addresses on
/// that store's own section, any values after it plain text. Recalled text in
/// those stores never opens a line that way, and such a line can only point at
/// the store's own pages, so it stays readable; any other braille is broken.
fn own_route_line(store: &str, line: &str) -> bool {
    use crate::agent::harness::book::{d456_knowledge, ledger};
    let Some(route) = d456_knowledge::store_route(store) else {
        return false;
    };
    let line = line.trim();
    let split = line
        .char_indices()
        .find(|(_, ch)| !ledger::is_cell(*ch))
        .map_or(line.len(), |(index, _)| index);
    let (lead, rest) = line.split_at(split);
    !lead.is_empty()
        && !rest.chars().any(ledger::is_cell)
        && (rest.is_empty() || rest.starts_with(' '))
        && ledger::addresses(lead).is_some_and(|addresses| {
            addresses.iter().all(|address| {
                address.primary == route.primary && address.section == Some(route.sub)
            })
        })
}

/// Wrap recalled memory `body` from `store` (caddy/dossier/atlas/…) inside
/// the evidence fence. `provenance` carries store metadata (repo key, age,
/// verification state) already formatted as `key=value` pairs by the caller;
/// it is attribute-sanitised here. The body is secret-redacted and
/// sentinel-neutralised, all but the store's own route lines. Returns an
/// empty string for empty bodies.
pub(crate) fn fence(store: &str, provenance: &str, body: &str) -> String {
    let body = body.trim_start_matches('\n');
    if body.trim().is_empty() {
        return String::new();
    }
    let neutralized = body
        .split('\n')
        .map(|line| {
            if own_route_line(store, line) {
                line.to_string()
            } else {
                neutralize_sentinels(line)
            }
        })
        .collect::<Vec<_>>()
        .join("\n");
    let redacted = crate::platform::secrets::redact_str(&neutralized);
    format!(
        "\n\n{EVIDENCE_FENCE_HEADER}\n[evidence store={} {}]\n{}\n{EVIDENCE_FENCE_SENTINEL}\n",
        safe_attr(store),
        safe_attr(provenance),
        redacted.trim_end()
    )
}

#[cfg(test)]
#[path = "../../../tests/cockpit/app/evidence__tests.rs"]
mod tests;
