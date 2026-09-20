//! Operator-facing root authority, shared by interactive status and task JSON.
//! Describes configured policy and backend availability, not per-process attestation.
use crate::harness::ActionCapsuleMode;
use crate::yolo::Profile;

#[derive(Clone, Debug, serde::Serialize)]
pub(crate) struct AuthorityProfile {
    pub(crate) profile: &'static str,
    pub(crate) sandbox: &'static str,
    pub(crate) network: &'static str,
    pub(crate) filesystem: &'static str,
    pub(crate) approval_mode: &'static str,
    pub(crate) text: String,
}

pub(crate) fn active(headless: bool) -> AuthorityProfile {
    render(
        crate::yolo::profile(),
        crate::sandbox::enabled(),
        crate::sandbox::available(),
        headless,
        ActionCapsuleMode::parse(std::env::var("ANGEL_ACTION_CAPSULES").ok().as_deref()),
    )
}

fn render(
    profile: Profile,
    sandbox_enabled: bool,
    backend_available: bool,
    headless: bool,
    capsules: ActionCapsuleMode,
) -> AuthorityProfile {
    let full = profile == Profile::Full;
    let sandbox = if full || !sandbox_enabled {
        "absent (disabled by operator)"
    } else if backend_available {
        "present (backend available; tool policy enabled, no per-process attestation)"
    } else {
        "absent (backend unavailable; enforcement requested)"
    };
    let network = if full {
        "allowed; effect/network restrictions bypassed"
    } else {
        "allowed by ordinary subprocess policy; individual tools may deny network"
    };
    let filesystem = if full {
        "broad authority over host reads and writes, subject to OS permissions"
    } else if sandbox_enabled && backend_available {
        "host reads allowed; subprocess writes limited to workspace plus tool scratch/cache/runtime roots; file tools enforce workspace paths; operator shell has full access"
    } else {
        "subprocess writes have no active sandbox guarantee; file tools enforce workspace paths; operator shell has full access"
    };
    let approval_mode = if full {
        "bypassed by operator (broad authority)"
    } else if profile == Profile::Smart {
        "workspace batches and self-tests auto-approved; other enabled gates prompt interactively or deny headless"
    } else if headless {
        "action capsules off in task mode; other enabled approval gates deny without UI"
    } else {
        match capsules {
            ActionCapsuleMode::Approve => {
                "exact action batches prompt; approve-all expires this turn or after 600 seconds; other gates depend on their settings"
            }
            ActionCapsuleMode::Observe => {
                "action batches observed without prompting; other gates depend on their settings"
            }
            ActionCapsuleMode::Off => "action capsules off; other gates depend on their settings",
        }
    };
    let label = if full {
        "YOLO FULL — broad authority"
    } else {
        profile.label()
    };
    AuthorityProfile {
        profile: profile.label(),
        sandbox,
        network,
        filesystem,
        approval_mode,
        text: format!(
            "authority profile: {label}\nsandbox: {sandbox}\nnetwork: {network}\nfilesystem: {filesystem}\napproval mode: {approval_mode}"
        ),
    }
}

#[cfg(test)]
#[path = "../../tests/cockpit/app/authority_profile__tests.rs"]
mod tests;
