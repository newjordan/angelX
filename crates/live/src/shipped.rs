//! The policy generations compiled into the binary (feature `v8`): every
//! `kind: policy` module of `cockpit/live`, embedded by `build.rs` with the
//! live loader's capture rules. A process with no live generation active serves
//! these, so a policy never lacks an implementation. Both the cockpit and the
//! Delve rules worker carry this table and evaluate it on the Kernel's V8
//! backend.
use crate::{Program, shipped_program};
use serde_json::Value;
use std::sync::{LazyLock, OnceLock};

include!(concat!(env!("OUT_DIR"), "/shipped_legs.rs"));

static PROGRAMS: LazyLock<Vec<OnceLock<Program>>> =
    LazyLock::new(|| SHIPPED_LEGS.iter().map(|_| OnceLock::new()).collect());

/// The ids of the shipped modules, in id order.
pub fn ids() -> impl Iterator<Item = &'static str> {
    SHIPPED_LEGS.iter().map(|(id, _)| *id)
}

/// The shipped generation of module `id`, checked like a loaded one (receipt,
/// probe, contract) on first use. A shipped module that cannot load is a build
/// defect, not a runtime condition.
pub fn program(id: &str) -> Option<&'static Program> {
    let index = SHIPPED_LEGS.iter().position(|(name, _)| *name == id)?;
    Some(PROGRAMS[index].get_or_init(|| {
        shipped_program(
            id,
            SHIPPED_LEGS[index].1,
            crate::v8::evaluate,
            |_, _| Ok(()),
        )
        .unwrap_or_else(|error| panic!("shipped leg {id} does not load: {error}"))
    }))
}

/// One render of the shipped module `id`, as an `angel_delve::scripted::Engine`
/// (`fn(&str, &Value) -> Result<Value, String>`). Shipped generation only: a
/// process with a live-module loader prefers its newest live generation and
/// uses this as the fallback.
pub fn engine(id: &str, input: &Value) -> Result<Value, String> {
    let program = program(id).ok_or_else(|| format!("no shipped module named {id}"))?;
    // One retry: a run terminated by the evaluator's deadline rebuilds its isolate.
    program.render(input).or_else(|_| program.render(input))
}
