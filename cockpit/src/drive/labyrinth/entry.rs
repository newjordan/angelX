//! Native entry points shared by the tool, headless CLI and RL research lane.
use super::{campaign, workflow};
use crate::agent::club::{Bag, Club};
use serde_json::{Value, json};
use std::path::Path;
use std::sync::{Arc, atomic::AtomicBool};

pub(crate) fn spec_schema() -> Value {
    let paths = json!({"type":"array","maxItems":1024,"items":{"type":"string","maxLength":512}});
    let check = json!({"type":"object","properties":{
        "argv":{"type":"array","minItems":1,"maxItems":64,"items":{"type":"string","maxLength":4096}},
        "timeout_secs":{"type":"integer","minimum":1,"maximum":14400}
    },"required":["argv"],"additionalProperties":false});
    json!({"type":"object","properties":{
        "id":{"type":"string","pattern":"^[A-Za-z0-9_-]{1,80}$"},
        "task":{"type":"string","minLength":1,"maxLength":16000},
        "doors":{"type":"array","minItems":1,"maxItems":3,"items":{
            "type":"object","properties":{
                "id":{"type":"string","minLength":1,"maxLength":240},
                "statement":{"type":"string","minLength":1,"maxLength":16000},
                "missing":{"type":"string","minLength":1,"maxLength":8000},
                "perspectives":{"type":"array","minItems":5,"maxItems":12,"items":{"type":"string","minLength":1,"maxLength":4000}}
            },"required":["id","statement","missing","perspectives"],"additionalProperties":false
        }},"inputs":paths,"referee_inputs":paths,"canonical_documents":paths,
        "checks":{"type":"array","minItems":1,"maxItems":16,"items":check},
        "spot_checks":{"type":"array","minItems":1,"maxItems":16,"items":check},
        "closed_routes":{"type":"array","maxItems":128,"items":{"type":"object","properties":{
            "door_id":{"type":"string","maxLength":240},"route":{"type":"string","minLength":1,"maxLength":2000},"lesson":{"type":"string","minLength":1,"maxLength":4000}
        },"required":["door_id","route","lesson"],"additionalProperties":false}},
        "side_projects":{"type":"array","maxItems":32,"items":{"type":"string","maxLength":512}},"compute_rules":{"type":"string","minLength":1,"maxLength":16000},
        "concurrency":{"type":"integer","minimum":1,"maximum":4},"max_hops":{"type":"integer","minimum":1,"maximum":128},
        "role_timeout_secs":{"type":"integer","minimum":1,"maximum":14400},
        "campaign_timeout_secs":{"type":"integer","minimum":1,"maximum":86400},
        "max_rounds":{"type":"integer","minimum":1,"maximum":10},"writer_retries":{"type":"integer","minimum":0,"maximum":3}
    },"required":["id","task","doors","referee_inputs","canonical_documents","checks","spot_checks"],"additionalProperties":false})
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
