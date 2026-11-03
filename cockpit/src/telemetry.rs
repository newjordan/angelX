//! Plain telemetry data the realm (game) code reads from the rest of the cockpit.
//!
//! The types live in `angel_live::telemetry`, below both the cockpit and the
//! realm host, so neither rebuilds the other. Each type keeps its old path as a
//! re-export (`agent::harness::ToolOutcome`, `knowledge::graph_crop::Chart`,
//! `ui::viz::lifecycle_viz::MotionMode`, ...); behavior that needs cockpit
//! internals stays beside the old home.

pub use angel_live::telemetry::*;
