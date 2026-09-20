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
mod tests {
    use super::*;

    #[test]
    fn authority_profile_all_profiles_surfaces_settings_and_wording() {
        for profile in [Profile::Guarded, Profile::Smart, Profile::Full] {
            for enabled in [false, true] {
                for available in [false, true] {
                    for headless in [false, true] {
                        for mode in [
                            ActionCapsuleMode::Off,
                            ActionCapsuleMode::Observe,
                            ActionCapsuleMode::Approve,
                        ] {
                            let value = render(profile, enabled, available, headless, mode);
                            for field in ["sandbox:", "network:", "filesystem:", "approval mode:"] {
                                assert!(value.text.contains(field), "{}", value.text);
                            }
                            if profile == Profile::Full {
                                let text = serde_json::to_string(&value).unwrap().to_lowercase();
                                assert!(text.contains("broad authority"));
                                for forbidden in ["confined", "sandboxed", "isolated"] {
                                    assert!(!text.contains(forbidden), "{text}");
                                }
                                assert!(value.sandbox.starts_with("absent"));
                                assert!(value.approval_mode.contains("bypassed"));
                            } else {
                                assert_eq!(
                                    value.sandbox.starts_with("present"),
                                    enabled && available
                                );
                                assert!(value.network.starts_with("allowed"));
                                if headless && profile == Profile::Guarded {
                                    assert!(value.approval_mode.contains("off in task mode"));
                                }
                            }
                        }
                    }
                }
            }
        }
    }
}
