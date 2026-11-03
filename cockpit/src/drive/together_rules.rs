//! Delve rules that run as hot-loaded Bend modules. The first is the Bone
//! Dominoes (`cockpit/live/delve-ossuary`), the pilot for porting a game's
//! pure rules: `angel_delve` keeps the native tick as the oracle and the
//! fallback, and asks a brain for the tick instead when one is installed.
//! This file is the cockpit's half of the seam: it turns a frame into the
//! leg's JSON, the leg's JSON into a step, and picks which generation
//! answers. See docs/architecture/bend-rules-pattern.md.
//!
//! `ANGEL_BEND_RULES=1` arms the newest live generation of the leg for every
//! simulation thread. Without a live generation, or on any error, the native
//! tick answers. (The separate rules-worker process, `ANGEL_DELVE_SYSTEM`,
//! has its own copy of the rules and is not reached by this setting.)
use crate::agent::harness::live_policy;
use angel_delve::rules::nooks::{OssuaryFrame, OssuaryStep, install_shared_ossuary_brain};
use std::sync::Arc;

pub(crate) const OSSUARY_LEG: &str = "delve-ossuary";

/// One tick from `program` (`None`: no generation to ask). Any evaluation or
/// decode failure is an error here, which the rules answer by running the
/// native tick.
fn ask_with(program: Option<angel_live::Program>, frame: &OssuaryFrame) -> Result<OssuaryStep, String> {
    let input = serde_json::to_value(frame).map_err(|error| error.to_string())?;
    let (step, source) = live_policy::decide_with::<Option<OssuaryStep>>(program, OSSUARY_LEG, &input, |_| Ok(()), || None);
    step.ok_or_else(|| format!("{OSSUARY_LEG}: {}", source.as_str()))
}

/// One tick from the newest live generation of the leg, as the cockpit's live
/// client last offered it.
fn ask_latest(frame: &OssuaryFrame) -> Result<OssuaryStep, String> {
    let input = serde_json::to_value(frame).map_err(|error| error.to_string())?;
    let (step, source) = live_policy::decide_latest::<Option<OssuaryStep>>(OSSUARY_LEG, &input, |_| Ok(()), || None);
    step.ok_or_else(|| format!("{OSSUARY_LEG}: {}", source.as_str()))
}

/// Read the setting once, at the cockpit's start with the Delve's other
/// settings.
pub(crate) fn arm_from_env() {
    arm(std::env::var("ANGEL_BEND_RULES").is_ok_and(|value| value == "1"));
}

pub(crate) fn arm(on: bool) {
    install_shared_ossuary_brain(on.then(|| Arc::new(ask_latest) as _));
}

/// The setting, pinned to one generation (`None`: the leg is missing), for tests.
#[cfg(test)]
pub(crate) fn arm_pinned(program: Option<angel_live::Program>) {
    install_shared_ossuary_brain(Some(Arc::new(move |frame| ask_with(program.clone(), frame))));
}

/// A brain pinned to one generation, for tests and measurements.
#[cfg(test)]
pub(crate) fn pinned_brain(program: angel_live::Program) -> angel_delve::rules::nooks::OssuaryBrain {
    std::rc::Rc::new(move |frame| {
        let input = serde_json::to_value(frame).map_err(|error| error.to_string())?;
        let value = program.render(&input)?;
        serde_json::from_value(value).map_err(|error| error.to_string())
    })
}

#[cfg(test)]
#[path = "../../../tests/cockpit/drive/together_rules__tests.rs"]
mod tests;
