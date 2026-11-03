//! Host services the Kernel owns: the pieces of the cockpit's platform layer
//! that the game side (the realm host) and the cockpit both stand on, so
//! neither has to link the other. Persistence transactions (atomic writes,
//! the private lock), secret redaction, the install paths, the peer
//! heartbeat shape and the Lean/Comp posture's environment reading live here;
//! the cockpit re-exports each item at its old path and the realm host reaches
//! them through `angel_live::host`.
//!
//! Everything below is the cockpit's own code moved verbatim: error text, file
//! modes, fsync and rename order, and redaction output are unchanged.

pub mod peer_watch;
pub mod posture;
pub mod runtime_paths;
pub mod secrets;
pub mod workspace_store;

/// Stable SHA-256 content identity using the already-linked platform implementation.
pub fn sha256_hex(data: &[u8]) -> String {
    use std::fmt::Write as _;
    let digest = ring::digest::digest(&ring::digest::SHA256, data);
    let mut hex = String::with_capacity(64);
    for byte in digest.as_ref() {
        let _ = write!(hex, "{byte:02x}");
    }
    hex
}

#[cfg(test)]
mod tests;
