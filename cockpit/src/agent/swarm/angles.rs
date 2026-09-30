//! Deterministic proposer angle roster.

use super::*;
use crate::agent::harness::book::{Route, d3_roles, d4_angles};

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct Angle {
    pub(crate) key: String,
    /// The lens as routes: its persona, then any stance overlay's pages.
    pub(crate) sys: String,
}

/// Stances that overlay a lens: each is its pages on `⠈⠛`, after the overlay
/// label (page 1).
pub(crate) const STANCES: &[(&str, &[usize])] = &[
    ("stet", &[2, 3]),
    ("minimal", &[4, 5]),
    ("expansive", &[6]),
    ("risk", &[7]),
    ("operational", &[8]),
];

/// The active lens roster, filtered and ordered by `ANGEL_MOA_PERSONAS`.
///
/// Read fresh on every call (no caching) so tests stay deterministic. Returns
/// the full built-in `PERSONAS` roster when the knob is unset or empty.
pub(crate) fn selected_personas() -> Vec<(&'static str, Route)> {
    let raw = match std::env::var("ANGEL_MOA_PERSONAS") {
        Ok(v) => v,
        Err(_) => return PERSONAS.to_vec(),
    };
    // Unset-or-empty ⇒ today's behavior, exactly and silently.
    if raw.trim().is_empty() {
        return PERSONAS.to_vec();
    }

    let valid: Vec<&str> = PERSONAS.iter().map(|(k, _)| *k).collect();
    let mut out: Vec<(&'static str, Route)> = Vec::new();
    for piece in raw.split(',') {
        let want = piece.trim();
        if want.is_empty() {
            continue;
        }
        // First occurrence wins its position; later duplicates are dropped.
        if out.iter().any(|(k, _)| k.eq_ignore_ascii_case(want)) {
            continue;
        }
        match PERSONAS.iter().find(|(k, _)| k.eq_ignore_ascii_case(want)) {
            // Matched against a static const tuple of a 'static key and a route.
            Some(&(k, route)) => out.push((k, route)),
            None => {
                // Silent gating reads as nonexistence — the gate must speak.
                eprintln!(
                    "ANGEL_MOA_PERSONAS: unknown persona key {want:?}; valid keys: {}",
                    valid.join(", ")
                );
            }
        }
    }

    if out.is_empty() {
        eprintln!(
            "ANGEL_MOA_PERSONAS: no valid persona keys in {raw:?}; falling back to full roster"
        );
        return PERSONAS.to_vec();
    }
    out
}

pub(crate) fn roster_len() -> usize {
    let n = selected_personas().len();
    n + n * STANCES.len()
}

pub(crate) fn angle(i: usize) -> Angle {
    let personas = selected_personas();
    let base_count = personas.len();
    if i < base_count {
        let (key, route) = personas[i];
        return Angle {
            key: key.to_string(),
            sys: route.cells(),
        };
    }
    let idx = (i - base_count) % (base_count * STANCES.len());
    let round = idx / STANCES.len();
    let stance_idx = idx % STANCES.len();
    let base_idx = (round + stance_idx) % base_count;
    let (base_key, base_route) = personas[base_idx];
    let (stance_key, stance_pages) = STANCES[stance_idx];
    // The overlay label, then the stance's own pages.
    let overlay = d3_roles::pages(
        d4_angles::STANCES,
        std::iter::once(1).chain(stance_pages.iter().copied()),
    );
    Angle {
        key: format!("{base_key}+{stance_key}"),
        sys: format!("{}\n\n{overlay}", base_route.cells()),
    }
}
