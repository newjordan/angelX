//! Operator-owned execution profiles.
//!
//! Three postures:
//!
//! - **Guarded** (default) — ordinary approvals, sandbox, timeouts, hooks.
//! - **Smart** (`ANGEL_YOLO_SMART=1`, `/yolos on`, `/yolo smart`) — powerful
//!   interactive coding: skip workspace action-capsule modals, auto-approve
//!   local action batches and gate-green self-edit merges, allow effectful
//!   `code_mode`. Keeps Landlock, tool timeouts, PreToolUse hooks, secret
//!   scrubbing, and turn hop/progress/completion/verification gates.
//! - **Full** (`ANGEL_YOLO=1`, `/yolo on`) — unrestricted machine authority:
//!   approvals, sandbox, timeouts, hooks, and effect/network restrictions all
//!   bypassed. Behavioral runaway/completion gates still apply.
//!
//! Full wins when both env flags are set. Smart never means "circle forever."
//!
//! A7: non-test builds cache the resolved profile so hop/tool/UI hot paths do
//! not re-read `ANGEL_YOLO*` env on every gate check. `/yolo` toggles update the
//! cache via [`set`]/ [`set_smart`], and [`clear_all`]. Tests always re-read
//! env so `TestEnvGuard` remains live.

use std::sync::atomic::{AtomicU8, Ordering};

/// Pure parser kept separate for deterministic tests.
pub(crate) fn parse(raw: Option<&str>) -> bool {
    match raw {
        Some(value) => {
            let value = value.trim().to_ascii_lowercase();
            !matches!(
                value.as_str(),
                "" | "0" | "false" | "no" | "off" | "disable" | "disabled"
            )
        }
        None => false,
    }
}

/// Operator execution posture.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Profile {
    /// Ordinary interactive guards.
    Guarded,
    /// Powerful coding without full-machine YOLO.
    Smart,
    /// Unrestricted execution (classic YOLO).
    Full,
}

impl Profile {
    pub(crate) fn label(self) -> &'static str {
        match self {
            Self::Guarded => "guarded",
            Self::Smart => "smart",
            Self::Full => "full",
        }
    }
}

// 0 = uncached (must re-read env), 1 = Guarded, 2 = Smart, 3 = Full.
const PROF_UNCACHED: u8 = 0;
const PROF_GUARDED: u8 = 1;
const PROF_SMART: u8 = 2;
const PROF_FULL: u8 = 3;

static CACHED_PROFILE: AtomicU8 = AtomicU8::new(PROF_UNCACHED);

fn profile_from_env() -> Profile {
    if parse(std::env::var("ANGEL_YOLO").ok().as_deref()) {
        Profile::Full
    } else if parse(std::env::var("ANGEL_YOLO_SMART").ok().as_deref()) {
        Profile::Smart
    } else {
        Profile::Guarded
    }
}

fn store_profile(profile: Profile) {
    let code = match profile {
        Profile::Guarded => PROF_GUARDED,
        Profile::Smart => PROF_SMART,
        Profile::Full => PROF_FULL,
    };
    CACHED_PROFILE.store(code, Ordering::Release);
}

fn invalidate_profile_cache() {
    CACHED_PROFILE.store(PROF_UNCACHED, Ordering::Release);
}

/// Resolve the active profile. Full overrides Smart when both are set.
pub(crate) fn profile() -> Profile {
    #[cfg(test)]
    {
        profile_from_env()
    }
    #[cfg(not(test))]
    {
        match CACHED_PROFILE.load(Ordering::Acquire) {
            PROF_GUARDED => return Profile::Guarded,
            PROF_SMART => return Profile::Smart,
            PROF_FULL => return Profile::Full,
            _ => {}
        }
        let profile = profile_from_env();
        store_profile(profile);
        profile
    }
}

/// Whether the operator has selected the unrestricted-execution profile.
pub(crate) fn enabled() -> bool {
    matches!(profile(), Profile::Full)
}

/// Whether smart / powerful-coding posture is active (and Full is not).
pub(crate) fn smart_enabled() -> bool {
    matches!(profile(), Profile::Smart)
}

/// Workspace action power: skip action-capsule approvals and allow confident
/// local mutations. True for Smart and Full.
pub(crate) fn workspace_power() -> bool {
    matches!(profile(), Profile::Smart | Profile::Full)
}

/// Effectful `code_mode` is allowed without a separate env pin.
pub(crate) fn code_effects_allowed() -> bool {
    workspace_power()
}

/// Gate-green self-edit merges skip the human modal under Smart/Full.
pub(crate) fn auto_integrate_self_edits() -> bool {
    workspace_power()
}

/// Whether this approval scope should auto-approve under Smart (workspace
/// work only). Full still uses the blanket path in [`approval::ask`].
pub(crate) fn smart_auto_approves(scope: &crate::approval::ApprovalScope) -> bool {
    if !smart_enabled() {
        return false;
    }
    matches!(
        scope,
        crate::approval::ApprovalScope::ActionBatch(_) | crate::approval::ApprovalScope::SelfTest
    )
}

/// Change the live full-YOLO posture. Enabling Full clears Smart so the two
/// profiles never stack. Consumers read flags at action/turn time.
pub(crate) fn set(enabled: bool) {
    if enabled {
        // TODO: Audit that the environment access only happens in single-threaded code.
        unsafe { std::env::set_var("ANGEL_YOLO", "1") };
        // TODO: Audit that the environment access only happens in single-threaded code.
        unsafe { std::env::remove_var("ANGEL_YOLO_SMART") };
        store_profile(Profile::Full);
    } else {
        // TODO: Audit that the environment access only happens in single-threaded code.
        unsafe { std::env::remove_var("ANGEL_YOLO") };
        // Smart may still be set — re-resolve from env on next read.
        invalidate_profile_cache();
    }
}

/// Change the live Smart posture. Enabling Smart clears Full.
pub(crate) fn set_smart(enabled: bool) {
    if enabled {
        // TODO: Audit that the environment access only happens in single-threaded code.
        unsafe { std::env::set_var("ANGEL_YOLO_SMART", "1") };
        // TODO: Audit that the environment access only happens in single-threaded code.
        unsafe { std::env::remove_var("ANGEL_YOLO") };
        store_profile(Profile::Smart);
    } else {
        // TODO: Audit that the environment access only happens in single-threaded code.
        unsafe { std::env::remove_var("ANGEL_YOLO_SMART") };
        invalidate_profile_cache();
    }
}

/// Clear both Full and Smart (return to Guarded).
pub(crate) fn clear_all() {
    // TODO: Audit that the environment access only happens in single-threaded code.
    unsafe { std::env::remove_var("ANGEL_YOLO") };
    // TODO: Audit that the environment access only happens in single-threaded code.
    unsafe { std::env::remove_var("ANGEL_YOLO_SMART") };
    store_profile(Profile::Guarded);
}

pub(crate) fn status_text() -> String {
    match profile() {
        Profile::Full => {
            "YOLO FULL — approvals, sandboxing, child-env scrubbing, command/HTTP/formation \
deadlines, descendant cleanup, hook denials, and effect/network restrictions are bypassed; \
turn hop/deadline/progress/completion/verification gates and multi-turn loop budgets remain \
active unless their own ANGEL_* control disables them; Esc/^C still obeys the operator"
                .to_string()
        }
        Profile::Smart => {
            "YOLO SMART (/yolos) — workspace action capsules off; local shell/write batches and \
gate-green self-edit merges auto-approve; effectful code_mode allowed; sandbox, tool \
timeouts, PreToolUse hooks, secret scrubbing, and hop/progress/verify gates stay ON; remote \
phone/peer/SOTA escalations still prompt; Esc/^C still obeys the operator"
                .to_string()
        }
        Profile::Guarded => {
            "YOLO OFF — ordinary approval, sandbox, effect, timeout, and harness guard settings \
apply · /yolos on for powerful coding · /yolo on for full machine authority"
                .to_string()
        }
    }
}

pub(crate) fn smart_status_text() -> String {
    status_text()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_only_explicit_truthy_values() {
        assert!(!parse(None));
        for value in ["", "0", "false", "NO", "off", "disabled"] {
            assert!(!parse(Some(value)), "{value}");
        }
        for value in ["1", "true", "YES", "on", "yolo"] {
            assert!(parse(Some(value)), "{value}");
        }
    }

    #[test]
    fn status_names_the_behavioral_boundary() {
        let _guard = crate::tests::env_lock();
        let _yolo = crate::tests::TestEnvGuard::set("ANGEL_YOLO", "1");
        let _smart = crate::tests::TestEnvGuard::unset("ANGEL_YOLO_SMART");
        let status = status_text();
        assert!(status.contains("approvals"));
        assert!(status.contains("sandboxing"));
        assert!(status.contains("turn hop/deadline/progress/completion/verification gates"));
        assert!(status.contains("remain"));
    }

    #[test]
    fn smart_and_full_are_mutually_exclusive_when_set() {
        let _guard = crate::tests::env_lock();
        let _full = crate::tests::TestEnvGuard::unset("ANGEL_YOLO");
        let _smart = crate::tests::TestEnvGuard::unset("ANGEL_YOLO_SMART");
        assert_eq!(profile(), Profile::Guarded);

        set_smart(true);
        assert!(smart_enabled());
        assert!(!enabled());
        assert_eq!(profile(), Profile::Smart);
        assert!(workspace_power());
        assert!(code_effects_allowed());

        set(true);
        assert!(enabled());
        assert!(!smart_enabled());
        assert_eq!(profile(), Profile::Full);

        set_smart(true);
        assert_eq!(profile(), Profile::Smart);
        assert!(!enabled());

        clear_all();
        assert_eq!(profile(), Profile::Guarded);
    }

    #[test]
    fn smart_auto_approves_only_workspace_scopes() {
        let _guard = crate::tests::env_lock();
        let _full = crate::tests::TestEnvGuard::unset("ANGEL_YOLO");
        let _smart = crate::tests::TestEnvGuard::set("ANGEL_YOLO_SMART", "1");
        assert!(smart_auto_approves(
            &crate::approval::ApprovalScope::ActionBatch("action-capsule:deadbeef".into())
        ));
        assert!(smart_auto_approves(
            &crate::approval::ApprovalScope::SelfTest
        ));
        assert!(!smart_auto_approves(
            &crate::approval::ApprovalScope::PhoneModel("sota".into())
        ));
        assert!(!smart_auto_approves(
            &crate::approval::ApprovalScope::RemoteHost("spark".into())
        ));
        assert!(!smart_auto_approves(&crate::approval::ApprovalScope::Peer(
            "reviewer".into()
        )));
    }

    #[test]
    fn smart_status_names_kept_guards() {
        let _guard = crate::tests::env_lock();
        let _full = crate::tests::TestEnvGuard::unset("ANGEL_YOLO");
        let _smart = crate::tests::TestEnvGuard::set("ANGEL_YOLO_SMART", "1");
        let status = status_text();
        assert!(status.contains("SMART"));
        assert!(status.contains("sandbox"));
        assert!(status.contains("timeouts"));
        assert!(status.contains("hooks"));
        assert!(status.contains("verify"));
    }
}
