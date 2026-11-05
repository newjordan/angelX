//! Bounded chart data shared by the graph tool and its truthful world projection.
// The data types live in `telemetry`; the old paths stay for their other readers.
#[allow(unused_imports)]
pub use crate::telemetry::{
    Chart, ChartKind, ChartSpec, DataPoint, GraphEvent, GraphReceipt, GraphRequest,
};
#[allow(unused_imports)]
pub(crate) use crate::telemetry::{MAX_PLOTS, MAX_POINTS};

// The ledger itself lives in `telemetry` beside the types it applies.
#[allow(unused_imports)]
pub(crate) use crate::telemetry::GraphStore;

#[cfg(test)]
#[path = "../../../tests/cockpit/app/graph_crop__tests.rs"]
mod tests;
