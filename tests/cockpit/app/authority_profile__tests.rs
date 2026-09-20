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
                            assert_eq!(value.sandbox.starts_with("present"), enabled && available);
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
