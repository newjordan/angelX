//! Magic keywords — prose triggers that opt a turn into specialized behavior.
//!
//! Ported from oh-my-pi's magic keywords (github.com/can1357/oh-my-pi). Three
//! standalone lowercase words, matched only in prose (never inside code spans,
//! fenced blocks, XML/HTML, identifiers, or paths):
//!
//! - `ultrathink` — request careful multi-step reasoning and the highest
//!   supported automatic thinking effort for this turn.
//! - `orchestrate` — prefer parallel subagents (`spawn` / `delegate`) for
//!   independent work and verify each phase.
//! - `workflowz` — build a deterministic multi-subagent workflow with the
//!   active task tools.
//!
//! Matching is word-boundary, case-insensitive on the whole token. Keywords
//! remain in the operator-visible echo; the harness appends a short steer so
//! the model sees the intent without re-parsing the raw word.

/// Which magic keywords fired in a user message.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(crate) struct MagicEffects {
    pub(crate) ultrathink: bool,
    pub(crate) orchestrate: bool,
    pub(crate) workflowz: bool,
    pub(crate) handoff_rl: bool,
}

impl MagicEffects {
    pub(crate) fn any(&self) -> bool {
        self.ultrathink || self.orchestrate || self.workflowz || self.handoff_rl
    }

    /// Bounded harness note injected as a replaceable turn-context fragment.
    /// Empty when nothing fired.
    pub(crate) fn steer_note(&self) -> Option<String> {
        if !self.any() {
            return None;
        }
        let mut lines = Vec::new();
        if self.ultrathink {
            lines.push(
                "ultrathink: take careful multi-step reasoning; prefer the highest \
                 available thinking effort; do not rush to a final answer.",
            );
        }
        if self.orchestrate {
            lines.push(
                "orchestrate: fan substantial independent work through parallel \
                 subagents (spawn/delegate), verify each phase, and merge only \
                 after checks pass.",
            );
        }
        if self.workflowz {
            lines.push(
                "workflowz: build a deterministic multi-subagent workflow with \
                 ordered phases and explicit handoffs; prefer structured yields \
                 over free-form prose between workers.",
            );
        }
        if self.handoff_rl {
            lines.push(
                "handoff_rl: compete using cockpit tools/resources; place victories on the board \
                 (check board before submitting); poll live candidate score, promote/reset from \
                 evidence, isolate next hot-path hypothesis on newest winning baseline, run focused \
                 correctness checks, and immediately submit next candidate. After a submission \
                 RESULT is in (score/status), the cockpit DEMANDS handoff: it wipes conversation \
                 context and prompt-injects a forced restart starting with 'hit it chewy' — not optional.",
            );
        }
        Some(format!("[magic-keywords]\n{}", lines.join("\n")))
    }
}

/// Scan operator prose for magic keywords. Strips fenced code, inline code,
/// and simple path/URL-like tokens before matching whole words.
pub(crate) fn scan(text: &str) -> MagicEffects {
    let prose = strip_non_prose(text);
    let mut effects = MagicEffects::default();
    let lower_prose = prose.to_ascii_lowercase();
    if lower_prose.contains("handoff_rl") || lower_prose.contains("handoff-rl") {
        effects.handoff_rl = true;
    }
    for token in prose.split(|c: char| !c.is_ascii_alphanumeric() && c != '_') {
        let lower = token.to_ascii_lowercase();
        match lower.as_str() {
            "ultrathink" => effects.ultrathink = true,
            "orchestrate" => effects.orchestrate = true,
            "workflowz" => effects.workflowz = true,
            "hrl" => effects.handoff_rl = true,
            _ => {}
        }
    }
    effects
}

/// Drop fenced blocks (``` … ```), inline `code`, and contiguous path/URL
/// tokens so keywords only fire as free-standing prose words.
fn strip_non_prose(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let bytes = text.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        // Fenced code block.
        if bytes[i] == b'`' && i + 2 < bytes.len() && bytes[i + 1] == b'`' && bytes[i + 2] == b'`' {
            i += 3;
            while i + 2 < bytes.len() {
                if bytes[i] == b'`' && bytes[i + 1] == b'`' && bytes[i + 2] == b'`' {
                    i += 3;
                    break;
                }
                i += 1;
            }
            out.push(' ');
            continue;
        }
        // Inline code span.
        if bytes[i] == b'`' {
            i += 1;
            while i < bytes.len() && bytes[i] != b'`' {
                i += 1;
            }
            if i < bytes.len() {
                i += 1;
            }
            out.push(' ');
            continue;
        }
        // Path / URL-ish: skip contiguous runs that look like identifiers joined
        // by / or :// so `path/to/orchestrate.rs` never fires.
        if looks_like_path_start(bytes, i) {
            while i < bytes.len() {
                let b = bytes[i];
                if b.is_ascii_alphanumeric()
                    || b == b'/'
                    || b == b'.'
                    || b == b'-'
                    || b == b'_'
                    || b == b':'
                    || b == b'?'
                    || b == b'='
                    || b == b'&'
                    || b == b'%'
                {
                    i += 1;
                } else {
                    break;
                }
            }
            out.push(' ');
            continue;
        }
        out.push(bytes[i] as char);
        i += 1;
    }
    out
}

fn looks_like_path_start(bytes: &[u8], i: usize) -> bool {
    // `./` `../` `/abs` `scheme://` or `ident/`
    if bytes[i] == b'/' || bytes[i] == b'.' {
        return true;
    }
    if bytes[i].is_ascii_alphanumeric() {
        // Look ahead for `/` or `://` within the same token.
        let mut j = i;
        while j < bytes.len()
            && (bytes[j].is_ascii_alphanumeric() || bytes[j] == b'-' || bytes[j] == b'_')
        {
            j += 1;
        }
        if j < bytes.len() && bytes[j] == b'/' {
            return true;
        }
        if j + 2 < bytes.len() && &bytes[j..j + 3] == b"://" {
            return true;
        }
    }
    false
}

#[cfg(test)]
#[path = "../../tests/cockpit/app/magic_keywords__tests.rs"]
mod tests;
