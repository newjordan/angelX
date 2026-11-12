//! Rules authored as Bend modules. A game hands its tick state to the host's
//! installed engine by module id and applies the answer; the host owns the
//! engine (the cockpit's warm V8 pool, the rules worker's own) and the shipped
//! generation compiled into it, so rules here carry no engine of their own.
use serde::{Serialize, de::DeserializeOwned};
use std::sync::OnceLock;

pub type Engine = fn(&str, &serde_json::Value) -> Result<serde_json::Value, String>;

static ENGINE: OnceLock<Engine> = OnceLock::new();

/// What a native host runs when nothing is installed: the Kernel's shipped
/// generations. A host that installs an engine (the cockpit prefers the newest
/// live generation) overrides it.
#[cfg(feature = "bend-engine")]
const SHIPPED: Option<Engine> = Some(angel_live::shipped::engine);
/// In a browser seat the host is the page: its own JavaScript runs the same
/// shipped modules (`page`).
#[cfg(all(not(feature = "bend-engine"), target_arch = "wasm32"))]
const SHIPPED: Option<Engine> = Some(page::engine);
#[cfg(all(not(feature = "bend-engine"), not(target_arch = "wasm32")))]
const SHIPPED: Option<Engine> = None;

#[cfg(target_arch = "wasm32")]
mod page;

/// Install the process's rules engine once; later installs are ignored.
pub fn install(engine: Engine) {
    let _ = ENGINE.set(engine);
}

pub fn installed() -> bool {
    ENGINE.get().is_some()
}

/// Whether module `id` lays its answer with the native rules kept as its
/// oracle: nothing is installed, or this crate's tests installed the shared
/// test dispatcher for other games and registered nothing for `id`.
#[cfg(any(test, feature = "testing"))]
pub fn answers_natively(id: &str) -> bool {
    // Only a test build asks the shared dispatcher by name.
    #[cfg(not(test))]
    let _ = id;
    #[cfg(test)]
    if TEST_DISPATCH.get().is_some() {
        return !TEST_ENGINES
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .contains_key(id);
    }
    !installed()
}

/// One step of module `id`. A missing engine or a refused answer is an error
/// the caller surfaces; rules never guess a native answer.
pub fn step<I: Serialize, O: DeserializeOwned>(id: &str, input: &I) -> Result<O, String> {
    let engine = ENGINE
        .get()
        .copied()
        .or(SHIPPED)
        .ok_or_else(|| format!("no rules engine is installed for {id}"))?;
    let input = serde_json::to_value(input).map_err(|e| e.to_string())?;
    serde_json::from_value(engine(id, &input)?).map_err(|e| format!("{id}: {e}"))
}

/// Tests in this crate have no host engine: each game registers the answers
/// it is tested against (its oracle or captured answers) under its module id,
/// and one dispatcher serves them all, whichever test installs first.
#[cfg(test)]
static TEST_ENGINES: std::sync::Mutex<std::collections::BTreeMap<&'static str, Engine>> =
    std::sync::Mutex::new(std::collections::BTreeMap::new());

#[cfg(test)]
pub fn install_for_test(id: &'static str, engine: Engine) {
    TEST_ENGINES
        .lock()
        .unwrap_or_else(|p| p.into_inner())
        .insert(id, engine);
    let dispatch: Engine = |id, input| {
        let engine = TEST_ENGINES
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .get(id)
            .copied()
            .ok_or_else(|| format!("no test engine is registered for {id}"))?;
        engine(id, input)
    };
    if ENGINE.set(dispatch).is_ok() {
        let _ = TEST_DISPATCH.set(());
    }
}

/// Set when the installed engine is the test dispatcher above.
#[cfg(test)]
static TEST_DISPATCH: OnceLock<()> = OnceLock::new();
