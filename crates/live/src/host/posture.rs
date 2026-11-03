//! The Lean / Comp / Turbo posture as the environment states it.
//!
//! The cockpit's `drive::comp_mode` owns the live posture (a cached flag its
//! `/comp` and `/turbo` commands flip); it reads the environment through here.
//! The realm host, which cannot call the cockpit, falls back on the same
//! reading when no live posture was lent to it, so a test and the cockpit's own
//! tests see exactly what the cockpit would.

/// Parse a boolean flag value: anything but empty / 0 / false / no / off /
/// disable / disabled is on.
pub fn parse(raw: Option<&str>) -> bool {
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

/// Whether any of the Lean / Comp / Turbo variables is on, read now.
pub fn from_env_uncached() -> bool {
    parse(std::env::var("ANGEL_COMP_MODE").ok().as_deref())
        || parse(std::env::var("ANGEL_TURBO").ok().as_deref())
        || parse(std::env::var("ANGEL_TURBO_MODE").ok().as_deref())
        || parse(std::env::var("ANGEL_LEAN_MODE").ok().as_deref())
        || parse(std::env::var("ANGEL_LEAN").ok().as_deref())
}
