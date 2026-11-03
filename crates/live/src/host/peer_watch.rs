//! The peer heartbeat record. The cockpit publishes and reads these (see its
//! `platform::peer_watch`); the world view only needs the record and the
//! staleness rule, so those live below both.

use serde::{Deserialize, Serialize};

/// How long without a heartbeat before a peer counts as gone.
pub const STALE_AFTER_MS: u128 = 90_000;

/// One instance's heartbeat. `activity` is a short operator-facing state
/// (`idle`, `turn`, `loop`, …); `detail` is optional human context (the
/// current goal or task label). Load fields let a reader aggregate a fleet
/// view without any external probe binary.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct Heartbeat {
    pub schema: String,
    pub instance_id: String,
    pub host: String,
    pub pid: u32,
    pub version: String,
    pub started_ms: u128,
    pub updated_ms: u128,
    pub workspace: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub session_id: Option<String>,
    pub activity: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub detail: Option<String>,
    pub cpu_pct: f32,
    pub mem_pct: f32,
    pub gpu_pct: f32,
}

impl Heartbeat {
    pub fn age_ms(&self, at_ms: u128) -> u128 {
        at_ms.saturating_sub(self.updated_ms)
    }

    pub fn is_live(&self, at_ms: u128) -> bool {
        self.age_ms(at_ms) <= STALE_AFTER_MS
    }
}
