//! Placement and expansion rules shared by command validation and previews.
use crate::economy::{covers, take};
use crate::*;
use std::collections::{BTreeSet, VecDeque};

/// What the ghost of a building says: the land, the price, what is short.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Placement {
    pub land: Result<(), String>,
    pub cost: Stock,
    pub short: Option<String>,
}
impl Placement {
    /// Whether `Command::Build` would be taken as it stands.
    pub fn ok(&self) -> bool {
        self.land.is_ok() && self.short.is_none()
    }
}

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
        if self.world.is_hillside() && !p.hillside {
            return Err("The hillside has its own works for that: see its build menu".into());
        }
        if !self.world.is_hillside() && !p.valley {
            return Err("That work belongs to the hillside".into());
        }
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
        if let Some(why) = self.reserved_ground(tile) {
            return Err(why);
        }
        if self.world.is_hillside() && self.track_tiles().contains(&tile) {
            return Err("A line's track runs over this plot".into());
        }
        if self.rollers.cells.contains_key(&tile) || self.rails.cells.contains_key(&tile) {
            return Err("A rollerway or a rail runs over this plot".into());
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
                "Waterwheels and quays need a dry plot beside flowing water",
            ),
            Siting::Sand => (
                self.world.near_deposit(tile, Deposit::Sand, 2),
                "A sand pit needs a sand bank within two plots",
            ),
            Siting::Hilltop => (
                self.world.hilltop(tile),
                "A windmill needs high ground where the wind blows",
            ),
        };
        if valid { Ok(()) } else { Err(reason.into()) }
    }
    /// Whether a `plan` may be raised with its top-left corner on `tile`, and at
    /// what price, without changing anything: the ghost the walking builder sees.
    /// `land` is why the ground refuses it (or Ok); `short` what the camp lacks
    /// of the price, or the charter it waits on.
    pub fn placement(&self, c: &Catalog, plan: &str, tile: Tile) -> Placement {
        let Some(p) = c.plans.get(plan) else {
            return Placement { land: Err("unknown plan".into()), cost: Stock::new(), short: None };
        };
        let land = (|| {
            if p.connector.is_some() {
                return self.check_power_site(p.connector.unwrap(), tile);
            }
            if plan == "camp"
                || tile == self.gate()
                || !self.inside(tile)
                || self.at(tile).is_some()
                || (self.roads.contains(&tile) && !p.passable)
                || self.buildings.len() >= crate::economy::MAX_BUILDINGS
            {
                return Err("choose an empty plot".to_string());
            }
            self.check_site(c, plan, tile)?;
            self.check_footprint(c, plan, tile)?;
            self.patch_for(c, p, tile).map(|_| ())
        })();
        let short = if !p.requires.iter().all(|r| self.researched.contains(r)) {
            Some(format!("research {} first", p.requires.join(", ")))
        } else if !covers(&self.stock, &p.cost) {
            Some(format!("need {}", crate::economy::missing(&self.stock, &p.cost)))
        } else {
            None
        };
        Placement { land, cost: p.cost.clone(), short }
    }

    /// Ground nothing may be built or laid on: a castle's footprint, or a
    /// model house's camp.
    pub fn reserved_ground(&self, tile: Tile) -> Option<String> {
        if !self.world.is_hillside() {
            return None;
        }
        if let Some(s) = self.sites.values().find(|s| s.covers(tile)) {
            return Some(format!("This ground is the {}'s: build outside its walls", s.plan));
        }
        // The Keep's ground is the Keep's from the first day, staked or not
        // (the game's wall is raised there): only its own fixtures stand on it.
        if crate::hillside::in_rect(tile, crate::hillside::keep_rect())
            && tile != crate::hillside::HILL_SHAFT
            && !crate::hillside::shaft_road().contains(&tile)
        {
            return Some("This ground is the Keep's: build outside its walls".to_string());
        }
        crate::hillside::CAMP_GROUNDS
            .iter()
            .any(|&r| crate::hillside::in_rect(tile, r))
            .then(|| "This is a model house's camp ground".to_string())
    }
    pub fn check_road_site(&self, tile: Tile) -> Result<(), String> {
        if !self.world.owned(tile) {
            Err("Survey this parcel on the Land page first".into())
        } else if let Some(why) = self.reserved_ground(tile) {
            Err(why)
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
        let (width, height) = self.world.bounds();
        let w = width as usize;
        let at = |t: Tile| t.1 as usize * w + t.0 as usize;
        let mut open = self.world.walkable_grid();
        for &t in &self.bridges {
            if self.world.owned(t) {
                open[at(t)] = true;
            }
        }
        // A finished building that cannot be walked through blocks the builders;
        // one still going up does not.
        for b in self.buildings.values() {
            if b.construction == 0 && !c.plans[&b.plan].passable {
                for t in self.covered(b) {
                    open[at(t)] = false;
                }
            }
        }
        let mut reached = vec![false; open.len()];
        let mut queue = VecDeque::from([self.camp()]);
        reached[at(self.camp())] = true;
        while let Some(tile) = queue.pop_front() {
            for next in crate::economy::neighbours(tile) {
                if next.0 >= 0 && next.1 >= 0 && next.0 < width && next.1 < height {
                    let i = at(next);
                    if open[i] && !reached[i] {
                        reached[i] = true;
                        queue.push_back(next);
                    }
                }
            }
        }
        let got = |t: Tile| t.0 >= 0 && t.1 >= 0 && t.0 < width && t.1 < height && reached[at(t)];
        self.buildings
            .iter()
            .filter(|(_, b)| got(b.tile) || self.docks_of(b).into_iter().any(got))
            .map(|(&id, _)| id)
            .collect()
    }

    /// The roads that would join a building to the camp's road network, new
    /// tiles only, nearest way first: a road already beside the building (and
    /// whatever it already reaches) is used before any new road is cut.
    pub fn plan_connection(&self, c: &Catalog, id: u32) -> Result<Vec<Tile>, String> {
        use std::cmp::Reverse;
        use std::collections::BinaryHeap;
        let b = self.buildings.get(&id).ok_or("no such building")?;
        if self.connected().contains(&id) {
            return Ok(Vec::new());
        }
        let (width, height) = self.world.bounds();
        let w = width as usize;
        let at = |t: Tile| t.1 as usize * w + t.0 as usize;
        let inside = |t: Tile| t.0 >= 0 && t.1 >= 0 && t.0 < width && t.1 < height;
        let mut open = self.world.walkable_grid();
        for &t in &self.bridges {
            open[at(t)] = true;
        }
        for other in self.buildings.values() {
            if !c.plans[&other.plan].passable {
                for t in self.covered(other) {
                    open[at(t)] = false;
                }
            }
        }
        // A rollerway is not a road: a road is cut round it.
        for t in self.rollers.cells.keys().chain(self.rails.cells.keys()) {
            open[at(*t)] = false;
        }
        // Where the camp's network already is.
        let mut camp_net = BTreeSet::new();
        let mut queue: VecDeque<Tile> = crate::economy::neighbours(self.camp())
            .into_iter()
            .filter(|t| self.roads.contains(t))
            .collect();
        while let Some(t) = queue.pop_front() {
            if camp_net.insert(t) {
                queue.extend(crate::economy::neighbours(t).into_iter().filter(|n| self.roads.contains(n)));
            }
        }
        let camp_docks: BTreeSet<Tile> = crate::economy::neighbours(self.camp()).into_iter().collect();
        // This building's own island of road, free to walk along.
        let mut island = BTreeSet::new();
        let mut queue: VecDeque<Tile> = self.docks_of(b).into_iter().filter(|t| self.roads.contains(t)).collect();
        while let Some(t) = queue.pop_front() {
            if island.insert(t) {
                queue.extend(crate::economy::neighbours(t).into_iter().filter(|n| self.roads.contains(n)));
            }
        }
        let mut best = vec![u32::MAX; open.len()];
        let mut parent: Vec<Option<Tile>> = vec![None; open.len()];
        let mut heap = BinaryHeap::new();
        let mut seed = |t: Tile, cost: u32| {
            if inside(t) && cost < best[at(t)] {
                best[at(t)] = cost;
                heap.push(Reverse((cost, t)));
            }
        };
        for &t in &island {
            seed(t, 0);
        }
        for t in self.docks_of(b) {
            if inside(t) && open[at(t)] && self.reserved_ground(t).is_none() {
                seed(t, if self.roads.contains(&t) { 0 } else { 4 });
            }
        }
        let mut end = None;
        while let Some(Reverse((d, t))) = heap.pop() {
            if d != best[at(t)] {
                continue;
            }
            if camp_net.contains(&t) || camp_docks.contains(&t) && self.roads.contains(&t) {
                end = Some(t);
                break;
            }
            if camp_docks.contains(&t) {
                end = Some(t);
                break;
            }
            for n in crate::economy::neighbours(t) {
                if !inside(n) || !open[at(n)] || self.reserved_ground(n).is_some() {
                    continue;
                }
                let nd = d + if self.roads.contains(&n) { 1 } else { 4 };
                if nd < best[at(n)] {
                    best[at(n)] = nd;
                    parent[at(n)] = Some(t);
                    heap.push(Reverse((nd, n)));
                }
            }
        }
        let mut tile = end.ok_or("no way to reach the camp's roads from there")?;
        let mut lay = Vec::new();
        loop {
            if !self.roads.contains(&tile) {
                lay.push(tile);
            }
            match parent[at(tile)] {
                Some(p) => tile = p,
                None => break,
            }
        }
        lay.reverse();
        Ok(lay)
    }
}
