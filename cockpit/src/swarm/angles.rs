//! Deterministic proposer angle roster.

use super::*;

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct Angle {
    pub(crate) key: String,
    pub(crate) sys: String,
}

pub(crate) const STANCES: &[(&str, &str)] = &[
    (
        "stet",
        "Keep the base angle's natural emphasis. Do not add novelty for its own sake.",
    ),
    (
        "minimal",
        "Prefer the smallest sufficient answer. Strip optional complexity unless it clearly pays.",
    ),
    (
        "expansive",
        "Look for omitted branches, adjacent options, and second-path answers others may miss.",
    ),
    (
        "risk",
        "Prioritize failure modes, edge cases, reversibility, and what could make the answer unsafe.",
    ),
    (
        "operational",
        "Translate the angle into concrete implementation, sequencing, and verification constraints.",
    ),
];

/// The active lens roster, filtered and ordered by `ANGEL_MOA_PERSONAS`.
///
/// Read fresh on every call (no caching) so tests stay deterministic. Returns
/// the full built-in `PERSONAS` roster when the knob is unset or empty.
pub(crate) fn selected_personas() -> Vec<(&'static str, &'static str)> {
    let raw = match std::env::var("ANGEL_MOA_PERSONAS") {
        Ok(v) => v,
        Err(_) => return PERSONAS.to_vec(),
    };
    // Unset-or-empty ⇒ today's behavior, exactly and silently.
    if raw.trim().is_empty() {
        return PERSONAS.to_vec();
    }

    let valid: Vec<&str> = PERSONAS.iter().map(|(k, _)| *k).collect();
    let mut out: Vec<(&'static str, &'static str)> = Vec::new();
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
            // Matched against a static const tuple of 'static strs.
            Some(&(k, s)) => out.push((k, s)),
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
        let (key, sys) = personas[i];
        return Angle {
            key: key.to_string(),
            sys: sys.to_string(),
        };
    }
    let idx = (i - base_count) % (base_count * STANCES.len());
    let round = idx / STANCES.len();
    let stance_idx = idx % STANCES.len();
    let base_idx = (round + stance_idx) % base_count;
    let (base_key, base_sys) = personas[base_idx];
    let (stance_key, stance_sys) = STANCES[stance_idx];
    Angle {
        key: format!("{base_key}+{stance_key}"),
        sys: format!("{base_sys}\n\nStance overlay: {stance_sys}"),
    }
}
