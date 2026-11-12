//! A pinned land charter becomes a concrete expedition purpose. This is a
//! projection of real world/claim state: no quest currency or synthetic kills.
use super::*;
use crate::kingdom::{ExpansionRequirement, Parcel, Stock};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ObjectiveStatus {
    Discover,
    Sentries,
    Guardian,
    Secure,
    SurveySupplies,
    SurveyReady,
}
impl ObjectiveStatus {
    pub fn label(self) -> &'static str {
        match self {
            Self::Discover => "FIND THE ORE SEAM",
            Self::Sentries => "DEFEAT THE ORE SENTRIES",
            Self::Guardian => "WIN THE GUARDIAN CLAIM",
            Self::Secure => "RETURN TO THE VEIN [E]",
            Self::SurveySupplies => "CLAIM WON - PREPARE THE SURVEY",
            Self::SurveyReady => "RETURN TO LAND - SURVEY READY",
        }
    }
    pub fn claim_won(self) -> bool {
        matches!(self, Self::SurveySupplies | Self::SurveyReady)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ObjectiveRoute {
    /// Starting region needed for this exact sequence of campaign floors.
    pub first: Pack,
    pub pack: Pack,
    pub floor: u32,
    pub here: bool,
    /// False means this floor is current or ahead on the active/planned trip.
    /// True explicitly asks for a new trip rather than promising past floors.
    pub next_expedition: bool,
}
impl ObjectiveRoute {
    pub fn label(self) -> String {
        if self.here {
            format!("Here: {}, floor {}", self.pack.name(), self.floor)
        } else if self.next_expedition {
            format!(
                "Next trip: start in {}; {} on floor {}",
                self.first.name(),
                self.pack.name(),
                self.floor
            )
        } else {
            format!("This route: {}, floor {}", self.pack.name(), self.floor)
        }
    }
}

#[derive(Clone, Debug)]
pub struct ExpeditionObjective {
    pub parcel: Parcel,
    pub district: String,
    pub resource: &'static str,
    pub purpose: String,
    pub status: ObjectiveStatus,
    pub route: ObjectiveRoute,
    pub threat: &'static str,
    pub warning: &'static str,
    pub counter: &'static str,
    pub guardian: &'static str,
    pub claim: Option<String>,
    pub remaining_cost: Stock,
    pub blocked_reason: Option<String>,
}
impl ExpeditionObjective {
    pub fn notice(&self) -> String {
        match self.status {
            ObjectiveStatus::SurveyReady => format!(
                "{} charter: the {} claim is secured and survey materials are ready. Return to Kingdom Works, Land, to purchase the parcel.",
                self.district, self.resource
            ),
            ObjectiveStatus::SurveySupplies => format!(
                "{} charter: the {} claim is secured. {}",
                self.district,
                self.resource,
                self.blocked_reason
                    .as_deref()
                    .unwrap_or("Finish the survey preparations on Land.")
            ),
            ObjectiveStatus::Secure => format!(
                "{} charter: guardian defeated. Return to the vein and press E; the claim must be secured before you can survey the new district.",
                self.district
            ),
            _ => format!(
                "Expedition for {}: secure {}. {}. {} {}",
                self.district,
                self.resource,
                self.route.label(),
                self.warning,
                self.counter
            ),
        }
    }
}

/// Find the earliest reachable occurrence using the same mapping as actual
/// deposits. In particular Mines-first does not promise crystal on floor 2:
/// that floor is the Crypt. Iron is unavailable ahead on a Crypt-first trip.
pub fn route_for(run: &Run, resource: &str) -> Option<ObjectiveRoute> {
    let current = run.dungeon.depth;
    if (1..=DEEPEST).contains(&current) && industry::ore_at(run.dungeon.pack, current).0 == resource
    {
        return Some(ObjectiveRoute {
            first: run.first,
            pack: run.dungeon.pack,
            floor: current,
            here: true,
            next_expedition: false,
        });
    }
    for floor in current.saturating_add(1).max(1)..=DEEPEST {
        let pack = Pack::at(floor, run.first);
        if industry::ore_at(pack, floor).0 == resource {
            return Some(ObjectiveRoute {
                first: run.first,
                pack,
                floor,
                here: false,
                next_expedition: false,
            });
        }
    }
    // Choose the shortest real fresh trip, not an arbitrary ore label. These
    // are the authored start-region choices, not the deeper floor packs.
    [Pack::Crypt, Pack::Cavern, Pack::Swamp]
        .into_iter()
        .flat_map(|first| (1..=DEEPEST).map(move |floor| (first, floor)))
        .filter_map(|(first, floor)| {
            let pack = Pack::at(floor, first);
            (industry::ore_at(pack, floor).0 == resource).then_some(ObjectiveRoute {
                first,
                pack,
                floor,
                here: false,
                next_expedition: true,
            })
        })
        .min_by_key(|r| r.floor)
}

impl Run {
    pub fn frontier_objective(&self) -> Option<ExpeditionObjective> {
        self.frontier_objective_for(self.home.industry.survey_goal?)
    }
    /// Production-only charters stay in the land desk. Only real ore-claim
    /// requirements create expedition orders or combat instructions.
    pub fn frontier_objective_for(&self, parcel: Parcel) -> Option<ExpeditionObjective> {
        let kingdom = &self.home.industry;
        let offer = kingdom.expansion_offer(parcel).ok()?;
        let ExpansionRequirement::Claim { resource } = &offer.requirement else {
            return None;
        };
        let kind = ore_threats::OreKind::for_resource(resource)?;
        let resource = kind.resource();
        let guardian = claim_bosses::Guardian::for_resource(resource)?;
        let route = route_for(self, resource)?;
        let secured = kingdom
            .claims
            .iter()
            .find(|(_, q)| q.secured && q.resource == resource)
            .map(|(id, _)| id.clone());
        let remaining_cost: Stock = offer
            .cost
            .iter()
            .filter_map(|(r, n)| {
                let missing = n.saturating_sub(kingdom.amount(r));
                (missing > 0).then_some((r.clone(), missing))
            })
            .collect();
        let blocked_reason = offer.blocked.clone().or_else(|| {
            (!remaining_cost.is_empty()).then(|| {
                format!(
                    "Deliver survey materials: {}.",
                    remaining_cost
                        .iter()
                        .map(|(r, n)| format!("{n} {r}"))
                        .collect::<Vec<_>>()
                        .join(", ")
                )
            })
        });
        let mut claim = secured.clone();
        let status = if kingdom.has_secured_resource(resource) {
            if blocked_reason.is_none() {
                ObjectiveStatus::SurveyReady
            } else {
                ObjectiveStatus::SurveySupplies
            }
        } else if let Some(d) = self
            .industry_deposits()
            .into_iter()
            .find(|d| d.resource == resource && kingdom.claims.contains_key(&d.id))
        {
            claim = Some(d.id.clone());
            if self.claim_guardian_defeated(&d.id) {
                ObjectiveStatus::Secure
            } else if !self.ore_sentries_defeated(&d.id) {
                ObjectiveStatus::Sentries
            } else {
                ObjectiveStatus::Guardian
            }
        } else {
            ObjectiveStatus::Discover
        };
        Some(ExpeditionObjective {
            parcel,
            district: offer.name,
            resource,
            purpose: offer.objective,
            status,
            route,
            threat: kind.name(),
            warning: kind.warning(),
            counter: kind.counter(),
            guardian: guardian.name(),
            claim,
            remaining_cost,
            blocked_reason,
        })
    }
    /// Call only at expedition start or a successful claim interaction, never
    /// every simulation tick. Custom objective text uses the visible notice wire.
    pub(super) fn announce_frontier_objective(&mut self) {
        if let Some(objective) = self.frontier_objective() {
            self.notices.push(objective.notice());
        }
    }
}

#[cfg(test)]
#[path = "../../tests/rules/frontier_objectives__tests.rs"]
pub(crate) mod tests;
