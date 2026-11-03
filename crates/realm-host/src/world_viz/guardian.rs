//! The sleeping giant by the keep: the peer guardian anthropomorphized.
//! A guardian instance on the fleet (its heartbeat activity says so)
//! becomes a giant NPC in the realm — asleep while every knight breathes,
//! standing watch when one falls silent, and kneeling in a shower of
//! sparks while a resurrection runs. Pure derivation from peer heartbeats
//! plus a pose table; rendering reuses the villager silhouette at giant
//! scale, so the realm gains a character without new art.

use super::*;
use angel_live::host::peer_watch::{Heartbeat, STALE_AFTER_MS};

/// What the giant is doing, derived fresh from each peer scan.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum GuardianState {
    /// No guardian on the fleet: no giant in the realm.
    #[default]
    Absent,
    /// A guardian lives and every knight breathes: asleep by the keep.
    Sleeping,
    /// A guardian lives and a knight has fallen silent: awake, watching.
    Watching,
    /// A resurrection is running somewhere on the fleet.
    Resurrecting,
}

/// Derive the giant's state from one peer scan. The giant *is* the
/// guardian: with no guardian heartbeat there is no giant, however many
/// knights lie fallen — the realm only shows help that actually exists.
pub fn guardian_state(peers: &[Heartbeat], at_ms: u128) -> GuardianState {
    let live = |peer: &Heartbeat| peer.is_live(at_ms);
    if peers.iter().any(|peer| {
        live(peer) && (peer.activity == "resurrecting" || peer.activity == "guardian-resurrecting")
    }) {
        return GuardianState::Resurrecting;
    }
    let guardian_present = peers
        .iter()
        .any(|peer| live(peer) && peer.activity.starts_with("guardian"));
    if !guardian_present {
        return GuardianState::Absent;
    }
    let fallen = peers.iter().any(|peer| {
        !live(peer)
            && !peer.activity.starts_with("guardian")
            && peer.activity != "resurrecting"
            && peer.age_ms(at_ms) > STALE_AFTER_MS
    });
    if fallen {
        GuardianState::Watching
    } else {
        GuardianState::Sleeping
    }
}

/// The giant's pose for one state: (width, height, lit) in sprite units,
/// where a villager stands 0.30 × 0.58. Sleeping slumps wide and low;
/// waking states stand roughly three villagers tall.
pub fn giant_pose(state: GuardianState) -> Option<(f32, f32, bool)> {
    match state {
        GuardianState::Absent => None,
        GuardianState::Sleeping => Some((1.15, 0.72, false)),
        GuardianState::Watching => Some((0.92, 1.72, true)),
        GuardianState::Resurrecting => Some((0.92, 1.45, true)),
    }
}

/// Where the giant stands, in world tiles: beside the keep, easing a
/// half-step toward the town when kneeling over the fallen.
pub fn giant_position(
    state: GuardianState,
    keep: (usize, usize),
    granary: Option<(usize, usize)>,
) -> Option<(f32, f32)> {
    match state {
        GuardianState::Absent => None,
        GuardianState::Sleeping | GuardianState::Watching => {
            Some((keep.0 as f32 + 2.4, keep.1 as f32 + 0.6))
        }
        GuardianState::Resurrecting => {
            let (tx, ty) = granary.unwrap_or(keep);
            Some((tx as f32 + 0.5, ty as f32 + 1.4))
        }
    }
}

impl World {
    pub fn set_live_realm_style(&mut self, style: angel_live::RealmStyle) {
        self.live_realm_style = style;
    }

    /// Fold one peer scan into the giant. Transitions announce themselves
    /// once, in the gain line, the way the town announces its growth.
    pub fn note_guardian_peers(&mut self, peers: &[Heartbeat], at_ms: u128) {
        let next = guardian_state(peers, at_ms);
        if next == self.guardian_state {
            return;
        }
        let note = match (self.guardian_state, next) {
            (GuardianState::Absent, GuardianState::Sleeping) => {
                Some(self.live_realm_style.sleeping.as_str())
            }
            (_, GuardianState::Watching) => Some(self.live_realm_style.watching.as_str()),
            (_, GuardianState::Resurrecting) => Some(self.live_realm_style.resurrecting.as_str()),
            (GuardianState::Resurrecting, GuardianState::Sleeping) => {
                Some(self.live_realm_style.recovered.as_str())
            }
            (GuardianState::Watching, GuardianState::Sleeping) => {
                Some(self.live_realm_style.accounted.as_str())
            }
            (_, GuardianState::Absent) => Some(self.live_realm_style.absent.as_str()),
            _ => None,
        };
        self.guardian_state = next;
        if let Some(note) = note {
            self.flash_gain(note.to_string(), self.live_realm_style.notice_ticks);
            if next == GuardianState::Resurrecting {
                self.sparkle_until = self.tick + self.live_realm_style.sparkle_ticks;
            }
        }
    }

    /// `/world`-facing one-liner for reports that list the realm's folk.
    pub fn guardian_line(&self) -> Option<&'static str> {
        match self.guardian_state {
            GuardianState::Absent => None,
            GuardianState::Sleeping => Some("the giant sleeps by the keep"),
            GuardianState::Watching => Some("the giant stands watch — a knight is silent"),
            GuardianState::Resurrecting => Some("the giant is raising a fallen knight"),
        }
    }
}

#[cfg(test)]
#[path = "../../tests/world_viz/guardian__tests.rs"]
mod tests;
