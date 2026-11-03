//! Native entry points shared by the tool, headless CLI and RL research lane.
use super::{campaign, workflow};
use crate::agent::club::{Bag, Club};
use serde_json::{Value, json};
use std::path::Path;
use std::sync::{Arc, atomic::AtomicBool};

pub(crate) fn spec_schema() -> Value {
    crate::agent::harness::core_catalog::baseline_def("labyrinth_campaign_spec").params
}

pub(crate) fn runtime(club: Arc<dyn Club>) -> campaign::CampaignRuntime {
    campaign::CampaignRuntime {
        literature: Arc::clone(&club),
        attack: Arc::clone(&club),
        referee: Arc::clone(&club),
        writer: club,
        integrator: Arc::new(workflow::ArtifactIntegrator),
    }
}

/// Resolve the configured concrete route without a fallback to practice.
pub(crate) fn configured_club(
    driver: Option<&str>,
    model: Option<&str>,
    effort: Option<&str>,
) -> Result<Arc<dyn Club>, String> {
    if model.is_some() && driver.is_none() {
        return Err("campaign --model requires a concrete --driver".into());
    }
    let mut bag = Bag::standard();
    if let Some(driver) = driver {
        bag.select_launch_route(driver, model)?;
    }
    let club = bag.in_hand();
    if club.label() == "practice" || club.route_identity().driver == "practice" {
        return Err("no configured live model route for this campaign".into());
    }
    if let Some(effort) = effort {
        club.set_reasoning_effort(effort)
            .ok_or("campaign reasoning effort is unsupported on the selected route")?;
    }
    Ok(club)
}

pub(crate) fn dispatch(
    workspace: &Path,
    args: &Value,
    club: Option<Arc<dyn Club>>,
    cancel: &AtomicBool,
) -> Result<Value, String> {
    let id = || args["id"].as_str().ok_or("campaign action requires id");
    match args["action"].as_str().unwrap_or("status") {
        "start" => {
            let spec = serde_json::from_value(
                args.get("spec")
                    .ok_or("campaign start requires spec")?
                    .clone(),
            )
            .map_err(|error| format!("invalid campaign spec: {error}"))?;
            campaign::start(workspace, spec)
        }
        "run" => {
            let id = id()?;
            let club = match club {
                Some(club) => club,
                None => configured_club(None, None, None)?,
            };
            campaign::run(workspace, id, runtime(club), cancel)
        }
        "status" => Ok(json!({
            "campaigns":campaign::status(workspace,args["id"].as_str())?,
            "artifacts":workflow::status(workspace)?,
        })),
        "cancel" => campaign::cancel(workspace, id()?),
        "recheck" => campaign::recheck(workspace, id()?),
        "check" => match args.get("bundle").filter(|value| !value.is_null()) {
            Some(value) => workflow::check_integration(
                workspace,
                value
                    .as_str()
                    .ok_or("campaign check bundle must be a relative path")?,
            ),
            None => workflow::check(workspace),
        },
        "recover" => workflow::recover(workspace),
        _ => Err("unknown Labyrinth campaign action".into()),
    }
}
