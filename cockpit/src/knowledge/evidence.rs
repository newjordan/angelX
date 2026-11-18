//! Shared provenance framing for recalled Caddy, dossier, Atlas and broker data.
//!
//! Redaction and delimiter escaping keep recalled text inside its evidence
//! block. This is prompt framing, not an execution sandbox: existing tool
//! policy remains responsible for refusing disallowed actions.

pub(crate) const EVIDENCE_FENCE_HEADER: &str = "⠎⠃";
pub(crate) const EVIDENCE_FENCE_SENTINEL: &str = "⠎⠃";

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

fn safe_attr(value: &str) -> String {
    value
        .chars()
        .filter(|c| {
            c.is_ascii_alphanumeric() || matches!(c, '_' | '-' | '.' | ':' | ',' | '/' | '=' | ' ')
        })
        .take(160)
        .collect()
}

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
