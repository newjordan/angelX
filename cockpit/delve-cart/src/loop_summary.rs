//! A plain owned snapshot of the loop facts the Delve/realm code reads.
//!
//! The settlement ledger, the crawl and the adventure mirror used to borrow the
//! whole [`LoopState`]. They now take a [`LoopSummary`] built at the call
//! boundary by [`LoopState::summary`], so none of them names `loop_ctl`. The
//! types live in the realm host (`angel_realm_host::loop_summary`), which
//! reads them; the enums mirror `loop_ctl`'s `LoopStatus` / `EscalationTier`
//! variant for variant, and the `From` impls beside `LoopState::feed_facts` are
//! exhaustive, so a new variant fails to compile there instead of drifting.
pub(crate) use angel_realm_host::loop_summary::{
    DurableEvents, EscalationTier, LoopStatus, LoopSummary,
};
