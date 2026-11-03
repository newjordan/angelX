//! The Kernel's V8 backend (feature `v8`): warm pooled isolates behind the
//! `Evaluator` every live-module host passes to the loader, plus the low-level
//! script mechanics the cockpit's model-facing code-mode runner shares.
mod pool;
pub mod script;
mod warm;

pub use pool::{evaluate, split_call};
pub use warm::Warm;
