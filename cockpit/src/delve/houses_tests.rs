use crate::delve::houses::*;

/// The house a route serves from: its model family, whichever machine or
/// provider serves it.
pub(crate) fn of_route(agent: &str, driver: &str, model: Option<&str>) -> Option<HouseId> {
    of_agent(crate::ui::agent_panel::profile::profile_for_route(agent, driver, model).key)
}

#[path = "../../../tests/cockpit/app/houses__tests.rs"]
mod tests;
