//! ⠽ y — personality types: bundles of sections. Each page of a type is one
//! paragraph of the prompt it replaces, as addresses, in the original order;
//! the ledger renders every referenced section or page in full, so
//! `ledger://⠽⠃` reads as the whole Driver prompt with teammates.
//!
//! The entry warpath is the system prompt: the type, then the task pace
//! (`⠍⠁` / `⠍⠃`) and the coding repair discipline (`⠽⠑`) for a headless task,
//! or the cockpit type (`⠽⠋` / `⠽⠛`) and its lane routes for the interactive
//! cockpit.

use super::{Primary, Raise, Route, Sub, m_method, n_environment, o_orchestration, warpath};
use std::path::Path;

pub(crate) const CELL: char = '⠽';
pub(crate) const DRIVER: Route = Route::new(CELL, '⠁');
pub(crate) const DRIVER_TEAM: Route = Route::new(CELL, '⠃');
pub(crate) const COMPACT: Route = Route::new(CELL, '⠉');
pub(crate) const COMPACT_TEAM: Route = Route::new(CELL, '⠙');
pub(crate) const REPAIR: Route = Route::new(CELL, '⠑');
pub(crate) const COCKPIT: Route = Route::new(CELL, '⠋');
pub(crate) const COCKPIT_TEAM: Route = Route::new(CELL, '⠛');

pub(crate) const PRIMARY: Primary = Primary {
    cell: CELL,
    name: "types",
    surface: "personality types: bundles of sections, rendered in full",
    subs: &[
        Sub {
            route: DRIVER,
            name: "driver",
            signal: "the Driver, no delegate routes configured",
            action: "",
            ideas: "",
            pages: &["⠞⠁", "⠺⠁⠺⠉⠁⠺⠙⠺⠉⠃", "⠺⠑", "⠞⠉⠞⠑", "⠧⠊", "⠕⠁", "⠞⠋"],
        },
        Sub {
            route: DRIVER_TEAM,
            name: "driver-team",
            signal: "the Driver with configured delegate routes",
            action: "",
            ideas: "",
            pages: &[
                "⠞⠁",
                "⠺⠁⠺⠉⠁⠺⠙⠺⠉⠃",
                "⠺⠑",
                "⠞⠉⠞⠑",
                "⠧⠊",
                "⠕⠃",
                "⠕⠉",
                "⠕⠙",
                "⠕⠑",
                "⠞⠋",
            ],
        },
        Sub {
            route: COMPACT,
            name: "compact",
            signal: "the compact task core, no delegate routes configured",
            action: "",
            ideas: "",
            pages: &["⠞⠃", "⠺⠃⠺⠉⠁⠺⠙", "⠺⠋", "⠞⠙⠧⠚", "⠞⠋"],
        },
        Sub {
            route: COMPACT_TEAM,
            name: "compact-team",
            signal: "the compact task core with configured delegate routes",
            action: "",
            ideas: "",
            pages: &["⠞⠃", "⠺⠃⠺⠉⠁⠺⠙", "⠺⠋", "⠞⠙⠧⠚", "⠕⠋", "⠞⠋"],
        },
        Sub {
            route: REPAIR,
            name: "repair",
            signal: "the coding repair discipline",
            action: "",
            ideas: "",
            pages: &["⠍⠉", "⠍⠙", "⠍⠑", "⠍⠋", "⠍⠛"],
        },
        Sub {
            route: COCKPIT,
            name: "cockpit",
            signal: "the Driver in the interactive cockpit, no delegate routes configured",
            action: "",
            ideas: "",
            pages: &[
                "⠞⠁",
                "⠺⠁⠺⠉⠁⠺⠙⠺⠉⠃",
                "⠺⠑",
                "⠞⠉⠞⠑",
                "⠧⠊",
                "⠕⠁",
                "⠞⠋",
                "⠝⠁",
                "⠝⠃",
            ],
        },
        Sub {
            route: COCKPIT_TEAM,
            name: "cockpit-team",
            signal: "the Driver in the interactive cockpit with configured delegate routes",
            action: "",
            ideas: "",
            pages: &[
                "⠞⠁",
                "⠺⠁⠺⠉⠁⠺⠙⠺⠉⠃",
                "⠺⠑",
                "⠞⠉⠞⠑",
                "⠧⠊",
                "⠕⠃⠕⠉",
                "⠕⠙⠕⠑",
                "⠞⠋",
                "⠝⠁",
                "⠝⠃",
            ],
        },
    ],
};

/// The interactive cockpit's entry warpath: the cockpit type, then its lane
/// routes (Treebeard). The session's verified capabilities are the `⠝⠁`
/// evidence.
pub(crate) fn cockpit_entry(
    workspace: &Path,
    specialists: &[(String, String)],
    capabilities: &str,
    lanes: &[Route],
) -> String {
    let kind = if specialists.is_empty() {
        COCKPIT
    } else {
        record_routes(workspace, specialists, o_orchestration::ROUTES);
        COCKPIT_TEAM
    };
    // The capabilities ride inside the type; only their evidence is recorded.
    super::ledger::record(workspace, n_environment::CAPABILITIES, capabilities);
    let mut raises = vec![Raise::new(kind, None)];
    raises.extend(lanes.iter().map(|lane| Raise::new(*lane, None)));
    warpath(workspace, &raises)
}

/// Configured delegate routes (`label`, role) as the `listed` section's evidence.
fn record_routes(workspace: &Path, specialists: &[(String, String)], listed: Route) {
    let routes = specialists
        .iter()
        .map(|(label, role)| format!("- {label}: {role}"))
        .collect::<Vec<_>>()
        .join("\n");
    super::ledger::record(workspace, listed, &routes);
}

/// The entry warpath: the personality type, then a headless task's pace and
/// its repair discipline. Configured delegate routes (`label`, role) are the
/// `⠕⠃` / `⠕⠋` evidence.
pub(crate) fn entry(
    workspace: &Path,
    specialists: &[(String, String)],
    compact: bool,
    pace: Option<Route>,
    repair: bool,
) -> String {
    let team = !specialists.is_empty();
    let kind = match (compact, team) {
        (false, false) => DRIVER,
        (false, true) => DRIVER_TEAM,
        (true, false) => COMPACT,
        (true, true) => COMPACT_TEAM,
    };
    if team {
        let listed = if compact {
            o_orchestration::ROUTES_COMPACT
        } else {
            o_orchestration::ROUTES
        };
        record_routes(workspace, specialists, listed);
    }
    let mut raises = vec![Raise::new(kind, None)];
    if let Some(pace) = pace.filter(|pace| [m_method::RAPID, m_method::DEEP].contains(pace)) {
        raises.push(Raise::new(pace, None));
    }
    if repair {
        raises.push(Raise::new(REPAIR, None));
    }
    warpath(workspace, &raises)
}
