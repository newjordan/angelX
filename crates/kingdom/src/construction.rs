//! Placement and expansion rules shared by command validation and previews.
use crate::economy::{covers, take};
use crate::*;
use std::collections::BTreeSet;

impl Kingdom {
    /// New sessions get real geography. Existing saves deliberately retain the
    /// meadow beneath their original buildings and roads.
    pub fn found(&mut self, c: &Catalog, seed: u64) {
        if self.is_empty() {
            self.world = World::new(seed);
            self.start(c);
        }
    }
    pub fn check_site(&self, c: &Catalog, plan: &str, tile: Tile) -> Result<(), String> {
        let p = c.plans.get(plan).ok_or("unknown plan")?;
        if let Some(kind) = p.connector {
            return self.check_power_site(kind, tile);
        }
        if PowerKind::ALL
            .into_iter()
            .any(|kind| self.power.contains(kind, tile))
        {
            return Err("Remove the power connector before building on this plot".into());
        }
        if !self.world.owned(tile) {
            return Err("Survey this parcel on the Land page first".into());
        }
        if !self.world.terrain(tile).walkable() {
            return Err("Build on land; lay a bridge to cross the river".into());
        }
        if self.world.legacy_plot(tile) && p.siting != Siting::Riverbank {
            return Ok(());
        }
        let (valid, reason) = match p.siting {
            Siting::Any => (true, ""),
            Siting::Clay => (
                self.world.near(tile, Terrain::ClayBank, 2),
                "Clay pits need a clay bank within two plots",
            ),
            Siting::Stone => (
                self.world.near(tile, Terrain::Stone, 3),
                "Quarries need a stone outcrop within three plots",
            ),
            Siting::Woodland => (
                self.world.near(tile, Terrain::Forest, 3),
                "Foresters need woodland within three plots",
            ),
            Siting::Riverbank => (
                self.world.water_adjacent(tile),
                "Waterwheels need a dry plot beside flowing water",
            ),
        };
        if valid { Ok(()) } else { Err(reason.into()) }
    }
    pub fn check_road_site(&self, tile: Tile) -> Result<(), String> {
        if !self.world.owned(tile) {
            Err("Survey this parcel on the Land page first".into())
        } else if !self.world.terrain(tile).walkable() {
            Err("River crossing needs a bridge: 4 planks and 2 stone".into())
        } else {
            Ok(())
        }
    }
    pub fn expansion_offer(&self, parcel: Parcel) -> Result<ExpansionOffer, String> {
        let claims = self.secured_evidence();
        self.world.offer(
            parcel,
            &ExpansionEvidence {
                produced: &self.produced,
                delivered: &self.delivered,
                secured_resources: &claims,
            },
        )
    }
    pub(crate) fn survey(&mut self, parcel: Parcel) -> Result<String, String> {
        let offer = self.expansion_offer(parcel)?;
        if let Some(reason) = offer.blocked {
            return Err(reason);
        }
        if !covers(&self.stock, &offer.cost) {
            return Err(format!("Survey needs {}", label(&offer.cost)));
        }
        let claims = self.secured_evidence();
        self.world.unlock(
            parcel,
            &ExpansionEvidence {
                produced: &self.produced,
                delivered: &self.delivered,
                secured_resources: &claims,
            },
        )?;
        take(&mut self.stock, &offer.cost);
        if self.survey_goal == Some(parcel) {
            self.survey_goal = None;
        }
        Ok(format!(
            "{} surveyed. New building land is ready",
            offer.name
        ))
    }
    pub(crate) fn build_bridge(&mut self, tile: Tile) -> Result<String, String> {
        if !self.world.owned(tile)
            || !self.world.terrain(tile).water()
            || self.roads.contains(&tile)
            || self.at(tile).is_some()
        {
            return Err("Choose an empty river crossing in your land".into());
        }
        let cost = Stock::from([("plank".into(), 4), ("stone".into(), 2)]);
        if !covers(&self.stock, &cost) {
            return Err("Bridge needs 4 planks and 2 stone".into());
        }
        take(&mut self.stock, &cost);
        self.roads.insert(tile);
        self.bridges.insert(tile);
        Ok("Timber bridge laid; connect roads on both banks".into())
    }
    pub fn walkable(&self, c: &Catalog, tile: Tile) -> bool {
        self.world.owned(tile)
            && (self.world.terrain(tile).walkable() || self.bridges.contains(&tile))
            && self
                .at(tile)
                .is_none_or(|id| c.plans[&self.buildings[&id].plan].passable)
    }

    /// Builders can carry construction supplies over owned dry ground. Roads
    /// are still required for workshop deliveries and occupied housing. This
    /// permits real courtyards and continuous walls without a road per brick.
    pub fn construction_access(&self, c: &Catalog) -> BTreeSet<u32> {
        use std::collections::{BTreeMap, VecDeque};
        let occupied: BTreeMap<_, _> = self.buildings.iter().map(|(&id, b)| (b.tile, id)).collect();
        let mut reached = BTreeSet::new();
        let mut queue = VecDeque::from([CAMP]);
        while let Some(tile) = queue.pop_front() {
            if !reached.insert(tile) {
                continue;
            }
            for next in crate::economy::neighbours(tile) {
                let open = self.world.owned(next)
                    && (self.world.terrain(next).walkable() || self.bridges.contains(&next))
                    && occupied.get(&next).is_none_or(|id| {
                        let b = &self.buildings[id];
                        b.construction > 0 || c.plans[&b.plan].passable
                    });
                if open && !reached.contains(&next) {
                    queue.push_back(next);
                }
            }
        }
        self.buildings
            .iter()
            .filter(|(_, b)| {
                reached.contains(&b.tile)
                    || crate::economy::neighbours(b.tile)
                        .iter()
                        .any(|t| reached.contains(t))
            })
            .map(|(&id, _)| id)
            .collect()
    }
}
