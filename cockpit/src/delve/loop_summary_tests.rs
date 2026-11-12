use crate::delve::loop_summary::*;
use crate::drive::loop_ctl::LoopState;
use angel_realm_host::together_settlement::planning::Input;

impl LoopState {
    /// An owned snapshot of the fields the realm side reads.
    pub(crate) fn summary(&self) -> LoopSummary {
        crate::delve::cart::feed::summary_of(self.feed_facts())
    }
}

#[path = "../../../tests/cockpit/loop_ctl/loop_summary__tests.rs"]
mod tests;
