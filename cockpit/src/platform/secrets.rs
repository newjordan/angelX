//! Shared secret redaction for session history, experience, trajectory and Cut
//! persistence. Exact-body rollout stores use the same detection to reject
//! sensitive payloads before hashing/writing; they never rewrite sealed data.
//!
//! Known values and provider shapes are checked in decoded strings. Structured
//! records are walked before serialization, preserving JSON types and escaping.
//!
//! 1. **Known values.** Every environment variable whose name looks secret
//!    (`KEY`, `TOKEN`, `SECRET`, `AUTH`, `PASSWORD` — the same rule
//!    `knowledge::experience::is_secret_name` uses for child-env stripping) and
//!    whose value is at least `MIN_SECRET_LEN` bytes is replaced by
//!    `«redacted:NAME»`. The name survives so a trainer can still see *which*
//!    credential a tool call used; the value never does.
//! 2. **Secret shapes.** Provider key prefixes (`sk-…`, z.ai `hex32.suffix`,
//!    GitHub `ghp_…`, Slack `xox…`, AWS `AKIA…`), bearer tokens, JWTs and PEM
//!    private-key blocks are replaced by `«redacted»` even when the value is
//!    not in this process's environment (pasted by the user, read from a
//!    file, printed by a tool).
//!
//! Over-redaction is the accepted trade, exactly as in `knowledge::barrel`:
//! these records feed training and audit; a shredded token is always safer
//! than a leaked one. The environment is read on every call (a few dozen
//! variables) so tests and late-loaded credentials are seen without a cache
//! to invalidate.

// The redactor lives in the Kernel's host services
// (`angel_live::host::secrets`) so the realm host and the atomic writers stand
// on the same code; every item is re-exported here at its old path.
#[cfg(test)]
pub(crate) use angel_live::host::secrets::{REDACTED, secret_values};
pub(crate) use angel_live::host::secrets::{
    Redactor, contains_secret, redact_bytes, redact_error, redact_str, redact_value,
    to_redacted_vec, to_redacted_vec_pretty,
};
#[cfg(test)]
use std::borrow::Cow;

/// Redact one valid JSONL record after decoding escaped strings. Malformed
/// input is left alone; persistence writers use `to_redacted_vec` directly.
#[cfg(test)]
pub(crate) fn redact_line(line: &mut Vec<u8>) -> usize {
    let mut value = match serde_json::from_slice(line) {
        Ok(value) => value,
        Err(_) => return 0,
    };
    let hits = redact_value(&mut value);
    if hits > 0 {
        *line = serde_json::to_vec(&value).expect("JSON value serialization");
    }
    hits
}

#[cfg(test)]
#[path = "../../../tests/cockpit/app/secrets__tests.rs"]
mod tests;
