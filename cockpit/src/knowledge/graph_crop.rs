//! Bounded chart data shared by the graph tool and its truthful world projection.
#[allow(unused_imports)]
pub(crate) use crate::telemetry::{MAX_PLOTS, MAX_POINTS};
#[allow(unused_imports)]
pub use crate::telemetry::{
    Chart, ChartKind, ChartSpec, DataPoint, GraphEvent, GraphReceipt, GraphRequest,
};

#[allow(unused_imports)]
pub(crate) use crate::telemetry::GraphStore;

#[cfg(test)]
#[path = "../../../tests/cockpit/app/graph_crop__tests.rs"]
mod tests;
