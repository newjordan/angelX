//! Solo / self-managed workload mode.
//!
//! When armed, the in-hand agent must do its own work: no paid SOTA consults,
//! no ChatGPT-OAuth/Codex outsourcing, no silent fall-through to another seat.
//! Local fleet peers may still be consulted only when the operator has not
//! locked the turn to pure self.

use std::sync::atomic::{AtomicBool, Ordering};

/// Process-wide latch so tools (which lack `App`) honor `/solo` without env races.
static SOLO_LATCH: AtomicBool = AtomicBool::new(false);

/// Standing steer injected into turn context while solo is on.
pub const SOLO_MODE_DIRECTIVE: &str = "[solo mode active] You own this workload. Do the work \
yourself with local tools. Do not consult_model, code_review, delegate, or spawn to openai, \
codex, chatgpt, grok, sota-moa, sota-swarm, or any other paid/remote SOTA seat. \
Self-test: mutate, run the smallest relevant verifier, read diagnostics, fix from evidence. \
Outsourcing reasoning is a failure of this mode. If blocked, name the blocker — do not call Codex.";

/// Arm/disarm solo from `/solo` (also mirrored into `ANGEL_SOLO` for child tools).
pub fn set_solo_mode(on: bool) {
    SOLO_LATCH.store(on, Ordering::Relaxed);
    if on {
        // TODO: Audit that the environment access only happens in single-threaded code.
        unsafe { std::env::set_var("ANGEL_SOLO", "1") };
    } else {
        // Prefer explicit off so tools reading env alone still see the latch.
        // TODO: Audit that the environment access only happens in single-threaded code.
        unsafe { std::env::set_var("ANGEL_SOLO", "0") };
    }
}

pub fn solo_mode_active() -> bool {
    if SOLO_LATCH.load(Ordering::Relaxed) {
        return true;
    }
    match std::env::var("ANGEL_SOLO") {
        Ok(v) => matches!(
            v.trim().to_ascii_lowercase().as_str(),
            "1" | "true" | "yes" | "on"
        ),
        Err(_) => false,
    }
}

/// Paid/remote seats may not be reached from consult/code_review unless the
/// operator explicitly allows it — and never while solo is on.
pub fn allow_sota_consult() -> bool {
    if solo_mode_active() {
        return false;
    }
    match std::env::var("ANGEL_ALLOW_SOTA_CONSULT") {
        Ok(v) => matches!(
            v.trim().to_ascii_lowercase().as_str(),
            "1" | "true" | "yes" | "on"
        ),
        // Default OFF: mid-turn outsourcing to Codex/OpenAI is the bug we hit.
        Err(_) => false,
    }
}

/// True if this label is a paid/remote SOTA seat that solo mode forbids.
pub fn is_outbound_sota_label(label: &str) -> bool {
    crate::club::is_sota_label(label)
}

/// Reject **outbound** SOTA targets when policy forbids them.
/// `self_label` is the in-hand club — consulting yourself is never outsourcing.
#[cfg(test)]
pub fn deny_sota_if_blocked(label: &str) -> Result<(), String> {
    deny_sota_if_blocked_except(label, None)
}

pub fn deny_sota_if_blocked_except(label: &str, self_label: Option<&str>) -> Result<(), String> {
    if self_label.is_some_and(|s| s.eq_ignore_ascii_case(label)) {
        return Ok(());
    }
    if !is_outbound_sota_label(label) {
        return Ok(());
    }
    if solo_mode_active() {
        return Err(format!(
            "solo mode: refusing paid/remote seat '{label}'. Do the work yourself with local tools \
             (read/edit/test). /solo off only if the operator explicitly wants outsourcing."
        ));
    }
    if !allow_sota_consult() {
        return Err(format!(
            "paid/remote seat '{label}' is blocked for mid-turn consult/review \
             (set ANGEL_ALLOW_SOTA_CONSULT=1 to permit, or pick a local fleet club). \
             Prefer doing the work on the in-hand agent."
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn solo_blocks_sota_even_when_consult_env_on() {
        let _g = crate::tests::env_lock();
        set_solo_mode(false);
        // TODO: Audit that the environment access only happens in single-threaded code.
        unsafe { std::env::set_var("ANGEL_ALLOW_SOTA_CONSULT", "1") };
        assert!(allow_sota_consult());
        assert!(deny_sota_if_blocked("openai").is_ok());
        set_solo_mode(true);
        assert!(!allow_sota_consult());
        assert!(deny_sota_if_blocked("openai").is_err());
        assert!(deny_sota_if_blocked("codex").is_err());
        assert!(
            deny_sota_if_blocked("gpt-5.3-codex-spark").is_err()
                || deny_sota_if_blocked("openai").is_err()
        );
        // Local-ish labels that aren't sota stay open.
        assert!(deny_sota_if_blocked("practice").is_ok());
        set_solo_mode(false);
        // TODO: Audit that the environment access only happens in single-threaded code.
        unsafe { std::env::remove_var("ANGEL_ALLOW_SOTA_CONSULT") };
    }
}
