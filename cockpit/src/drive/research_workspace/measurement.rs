//! Presentation-only measurements reuse the competition layer's exact decimals.
//! The type, its labels and its baseline comparison live in `angel_live::telemetry`.
pub(crate) use crate::telemetry::Measurement;

#[cfg(test)]
#[path = "../../../../tests/cockpit/app/research_workspace__measurement__tests.rs"]
mod tests;
