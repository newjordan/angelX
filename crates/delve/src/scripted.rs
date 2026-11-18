//! Rules authored as Bend modules. A game hands its tick state to the host's
//! installed engine by module id and applies the answer; the host owns the
//! engine (the cockpit's warm V8 pool, the rules worker's own) and the shipped
//! generation compiled into it, so rules here carry no engine of their own.
use serde::{Serialize, de::DeserializeOwned};
use std::sync::OnceLock;

pub type Engine = fn(&str, &serde_json::Value) -> Result<serde_json::Value, String>;

static ENGINE: OnceLock<Engine> = OnceLock::new();

#[cfg(feature = "bend-engine")]
const SHIPPED: Option<Engine> = Some(angel_live::shipped::engine);
#[cfg(all(not(feature = "bend-engine"), target_arch = "wasm32"))]
const SHIPPED: Option<Engine> = Some(page::engine);
#[cfg(all(not(feature = "bend-engine"), not(target_arch = "wasm32")))]
const SHIPPED: Option<Engine> = None;

#[cfg(target_arch = "wasm32")]
mod page;

pub fn install(engine: Engine) {
    let _ = ENGINE.set(engine);
}

pub fn installed() -> bool {
    ENGINE.get().is_some()
}

#[cfg(any(test, feature = "testing"))]
pub fn answers_natively(id: &str) -> bool {
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

pub fn step<I: Serialize, O: DeserializeOwned>(id: &str, input: &I) -> Result<O, String> {
    let engine = ENGINE
        .get()
        .copied()
        .or(SHIPPED)
        .ok_or_else(|| format!("no rules engine is installed for {id}"))?;
    let input = serde_json::to_value(input).map_err(|e| e.to_string())?;
    serde_json::from_value(engine(id, &input)?).map_err(|e| format!("{id}: {e}"))
}

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

#[cfg(test)]
static TEST_DISPATCH: OnceLock<()> = OnceLock::new();
