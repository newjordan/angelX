//! Modular competition packages.
//!
//! A competition package bundles the family-specific integration points the
//! harness needs: a submission status source (watch), fleet telemetry, and
//! identity for the operator surface. Packages register themselves here; the
//! harness routes through the registry instead of hard-wiring a family.
//!
//! Yukon is the default package. Adding a family (kernel-bench, a
//! project-local benchmark, a future flywheel variant) means adding a sibling
//! module and registering it — no harness edits.

pub(crate) mod yukon;

/// One registered competition family.
pub(crate) struct CompetitionPackage {
    /// Stable family id, e.g. "yukon".
    pub(crate) id: &'static str,
    /// Human label for operator surfaces (kept for the operator UIs to come).
    #[allow(dead_code)]
    pub(crate) label: &'static str,
}

impl CompetitionPackage {
    pub(crate) const fn new(id: &'static str, label: &'static str) -> Self {
        Self { id, label }
    }
}

/// Packages compiled into this build, in preference order.
pub(crate) const PACKAGES: &[CompetitionPackage] = &[yukon::PACKAGE];

/// The default package used when none is selected.
pub(crate) fn default_package() -> &'static CompetitionPackage {
    PACKAGES
        .first()
        .expect("at least one competition package")
}

/// The active package: `ANGEL_COMP_PACKAGE` selects a compiled family by id;
/// unknown ids fall back to the default with an error note available to the
/// caller via [`selection_error`].
pub(crate) fn active_package() -> &'static CompetitionPackage {
    match std::env::var("ANGEL_COMP_PACKAGE") {
        Ok(id) => PACKAGES
            .iter()
            .find(|p| p.id == id.trim())
            .unwrap_or_else(|| default_package()),
        Err(_) => default_package(),
    }
}

/// Set when [`active_package`] fell back because `ANGEL_COMP_PACKAGE` named an
/// unknown family. Kept for the operator status surface that will surface it.
#[allow(dead_code)]
pub(crate) fn selection_error() -> Option<String> {
    let id = std::env::var("ANGEL_COMP_PACKAGE").ok()?;
    let id = id.trim();
    if id.is_empty() {
        return None;
    }
    PACKAGES
        .iter()
        .find(|p| p.id == id)
        .map(|_| None)
        .unwrap_or_else(|| Some(format!("unknown competition package `{id}`; using default `{}`", default_package().id)))
}
