use super::Delve;
use crate::world_viz::{Building, classify_tool_activity};
use angel_live::kernel::{Binding, Candidate, Resources};
use angel_plug::drive::{DriveWorld, Engine, Labyrinth, Neighbours, Place};
use serde_json::Value;
use std::collections::BTreeMap;
use std::path::Path;
use std::sync::Arc;

#[derive(Clone, Copy, Debug, Default)]
pub(crate) struct EvidenceStats {
    pub(crate) active: bool,
    pub(crate) observations: u32,
    pub(crate) checks: u32,
    pub(crate) contrasts: u32,
    pub(crate) inconclusive: u32,
}

impl EvidenceStats {
    fn from_advice(advice: &Value, active: bool) -> Self {
        let count = |key: &str| advice[key].as_u64().unwrap_or(0).min(u32::MAX as u64) as u32;
        Self {
            active,
            observations: count("relation_count"),
            checks: count("check_count"),
            contrasts: count("contrast_count"),
            inconclusive: count("inconclusive_count"),
        }
    }
}

pub(crate) fn place_of_building(building: Building) -> Place {
    match building {
        Building::Keep | Building::Gatehouse | Building::Rookery => Place::Keep,
        Building::RoundTable => Place::Council,
        Building::Smithy => Place::Smithy,
        Building::Observatory => Place::Observatory,
        Building::Scriptorium | Building::Chapel => Place::Library,
    }
}

pub fn character_resource(
    name: &str,
    value: &Value,
    assets: &BTreeMap<String, Arc<angel_live::Asset>>,
) -> Result<Binding, String> {
    if value["player"]
        .as_u64()
        .is_none_or(|player| name != format!("character-seat-{player}"))
    {
        return Err("character resource name must match its player seat".into());
    }
    crate::together_appearance::bind(value, assets)
}

impl DriveWorld for Delve {
    type SettlementInput = angel_realm_host::together_settlement::planning::Input;

    fn settlement_input(
        root: &Path,
        task: &str,
        labyrinth: &dyn Labyrinth,
    ) -> Result<Option<Self::SettlementInput>, String> {
        crate::settlement_input::input(root, task, labyrinth)
    }

    fn settlement_contract(site_id: &str, loop_id: &str, iteration: usize) -> String {
        angel_realm_host::together_settlement::exhibits::worker_contract(
            site_id, loop_id, iteration,
        )
    }

    fn research_evidence(advice: &Value, active: bool) -> angel_plug::feed::Evidence {
        EvidenceStats::from_advice(advice, active).into()
    }

    fn tool_place(name: &str, args: &str) -> Place {
        place_of_building(classify_tool_activity(name, args).building)
    }

    fn shortest_path(
        count: usize,
        start: usize,
        goal: usize,
        neighbours: Neighbours<'_>,
    ) -> Option<Vec<usize>> {
        crate::routing::shortest_path(count, start, goal, |at| neighbours(at))
    }

    fn bind_resource(
        contract: &str,
        name: &str,
        value: &Value,
        assets: &BTreeMap<String, Arc<angel_live::Asset>>,
        factory: &Candidate,
        imports: &Resources,
    ) -> Result<Option<Binding>, String> {
        if let Some(binding) =
            crate::together_loadouts::bind(contract, name, value, assets, factory, imports)?
        {
            return Ok(Some(binding));
        }
        if contract == crate::together_appearance::CONTRACT {
            return character_resource(name, value, assets).map(Some);
        }
        if contract == crate::together_abilities::CONTRACT {
            return crate::together_abilities::bind(name, value, assets, factory).map(Some);
        }
        Ok(None)
    }

    fn validate_contract(kind: &str, value: &Value) -> Result<(), String> {
        if kind == "game" {
            let policy =
                serde_json::from_value(value.clone()).map_err(|error| error.to_string())?;
            crate::together_sim::validate_live_rules(&policy)?;
        }
        Ok(())
    }

    fn install_scripted_rules(engine: Engine) {
        angel_delve::scripted::install(engine);
    }

    #[cfg(feature = "glass")]
    fn serving_knights(
        &self,
        lead: angel_plug::feed::AgentKey,
        seats: &[angel_plug::feed::AgentKey],
    ) -> Vec<(String, String)> {
        use crate::houses;
        let lead = houses::of_agent(lead);
        let mut seated: Vec<houses::HouseId> = Vec::new();
        for &key in seats {
            if let Some(house) = houses::of_agent(key)
                && !seated.contains(&house)
            {
                seated.push(house);
            }
        }
        let lead = lead.or_else(|| seated.first().copied());
        seated.retain(|&house| Some(house) != lead);
        let serving = houses::Serving {
            lead,
            seated,
            turn: false,
        };
        serving
            .houses()
            .map(|id| {
                let house = houses::get(id);
                (
                    house.key.to_string(),
                    house
                        .knight(0)
                        .map(|k| k.name.clone())
                        .unwrap_or_else(|| house.label()),
                )
            })
            .collect()
    }
}

pub(crate) fn decide_latest<T: serde::de::DeserializeOwned>(
    name: &str,
    input: &Value,
    check: impl FnOnce(&T) -> Result<(), String>,
    native: impl FnOnce() -> T,
) -> (T, angel_live::policy::LegSource) {
    let program = angel_live::policy::offered(name);
    angel_live::policy::decide_with(program, name, input, check, native)
}
