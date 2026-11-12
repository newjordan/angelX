use super::{Catalog, Stock};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet, VecDeque};

pub const WIDTH: i16 = crate::world::WORLD_WIDTH;
pub const HEIGHT: i16 = crate::world::WORLD_HEIGHT;
pub const MAX_CLAIMS: usize = 64;
pub const MAX_BUILDINGS: usize = 160;
pub type Tile = (i16, i16);
pub const CAMP: Tile = (11, 8);
pub const GATE: Tile = (11, 16);

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Claim {
    pub resource: String,
    pub name: String,
    pub source: String,
    pub secured: bool,
    pub rate: u32,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct Building {
    pub plan: String,
    pub tile: Tile,
    pub recipe: Option<String>,
    pub claim: Option<String>,
    pub paused: bool,
    pub progress: u16,
    pub construction: u16,
    pub input: Stock,
    pub output: Stock,
    pub priority: u8,
    /// Zero is unlimited. A workshop stops when all of its products meet this
    /// kingdom-wide target, including buffered goods and cargo in transit.
    pub production_limit: u32,
    pub work_credit: u16,
    pub construction_credit: u16,
    pub power_upgrade: bool,
    pub power_enabled: bool,
    pub fuel_left: u16,
}
impl Default for Building {
    fn default() -> Self {
        Self {
            plan: String::new(),
            tile: (0, 0),
            recipe: None,
            claim: None,
            paused: false,
            progress: 0,
            construction: 0,
            input: Stock::new(),
            output: Stock::new(),
            priority: 1,
            production_limit: 0,
            work_credit: 0,
            construction_credit: 0,
            power_upgrade: false,
            power_enabled: true,
            fuel_left: 0,
        }
    }
}
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct Kingdom {
    pub schema: u32,
    pub catalog: String,
    pub rev: u64,
    pub shift: u64,
    pub stock: Stock,
    pub roads: BTreeSet<Tile>,
    pub buildings: BTreeMap<u32, Building>,
    pub claims: BTreeMap<String, Claim>,
    /// Charter evidence survives the deliberate retirement of a mining claim.
    pub secured_resources: BTreeSet<String>,
    pub next: u32,
    pub provision: bool,
    pub second: u64,
    pub researched: BTreeSet<String>,
    pub shipments: Vec<Shipment>,
    pub dispatch_cursor: u32,
    pub produced: Stock,
    pub delivered: Stock,
    pub world: crate::World,
    pub bridges: BTreeSet<Tile>,
    pub population: crate::population::Population,
    pub survey_goal: Option<crate::Parcel>,
    pub motion: crate::citizen_motion::Motion,
    /// None keeps the normal two meals per resident reserved for the village.
    pub food_reserve: Option<u8>,
    pub power: crate::PowerGrid,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Shipment {
    pub resource: String,
    pub amount: u32,
    pub from: u32,
    pub to: u32,
    pub tile: Tile,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Command {
    Build { plan: String, tile: Tile },
    Road(Tile),
    Demolish(Tile),
    Pause(u32),
    Recipe { building: u32, recipe: String },
    Link { building: u32, claim: String },
    Unlink(u32),
    ReleaseClaim(String),
    Provision,
    Research(String),
    Priority { building: u32, priority: u8 },
    ProductionLimit { building: u32, amount: u32 },
    Survey(crate::Parcel),
    SurveyGoal(Option<crate::Parcel>),
    FoodReserve(u8),
    Bridge(Tile),
    EnablePower(u32),
    PowerLine { kind: crate::PowerKind, tile: Tile },
    RemovePower { kind: crate::PowerKind, tile: Tile },
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Status {
    Working,
    Construction,
    Disconnected,
    Paused,
    Workers,
    Inputs,
    Storage,
    Unclaimed,
    Idle,
    Target,
    PowerDisconnected,
    PowerOverloaded,
    Fuel,
}
impl Status {
    pub fn label(&self) -> &'static str {
        match self {
            Self::Construction => "UNDER CONSTRUCTION",
            Self::Working => "WORKING",
            Self::Disconnected => "NO ROAD TO CAMP",
            Self::Paused => "PAUSED",
            Self::Workers => "NEEDS WORKERS",
            Self::Inputs => "NEEDS INPUTS",
            Self::Storage => "STORAGE FULL",
            Self::Unclaimed => "LINK A CLAIM [2]",
            Self::Idle => "READY",
            Self::Target => "STOCK TARGET MET",
            Self::PowerDisconnected => "CONNECT A POWER SOURCE",
            Self::PowerOverloaded => "NOT ENOUGH POWER",
            Self::Fuel => "NEEDS DELIVERED FUEL",
        }
    }
}
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Report {
    pub status: BTreeMap<u32, Status>,
    pub workers: u16,
    pub employed: u16,
    pub capacity: u32,
    pub outputs: Stock,
    pub carriers: u16,
    pub population: crate::population::PopulationReport,
    pub energy: crate::PowerReport,
}

pub(crate) fn inside((x, y): Tile) -> bool {
    (0..WIDTH).contains(&x) && (0..HEIGHT).contains(&y)
}
pub(crate) fn neighbours((x, y): Tile) -> [Tile; 4] {
    [(x - 1, y), (x + 1, y), (x, y - 1), (x, y + 1)]
}
pub(crate) fn covers(stock: &Stock, cost: &Stock) -> bool {
    cost.iter().all(|(r, n)| stock.get(r).unwrap_or(&0) >= n)
}
pub(crate) fn take(stock: &mut Stock, cost: &Stock) {
    for (r, n) in cost {
        *stock.get_mut(r).expect("cost checked") -= n;
    }
    stock.retain(|_, n| *n > 0);
}
pub(crate) fn add(stock: &mut Stock, resource: &str, amount: u32) {
    if amount > 0 {
        *stock.entry(resource.into()).or_default() += amount;
    }
}

impl Kingdom {
    pub fn is_empty(&self) -> bool {
        self.schema == 0
    }
    pub fn start(&mut self, c: &Catalog) {
        if !self.is_empty() {
            return;
        }
        self.schema = 1;
        self.catalog = c.id.clone();
        self.stock = c.starter.clone();
        self.next = 2;
        self.population = crate::population::Population::founders();
        self.buildings.insert(
            1,
            Building {
                plan: "camp".into(),
                tile: CAMP,
                recipe: None,
                claim: None,
                paused: false,
                progress: 0,
                ..Building::default()
            },
        );
        for x in 8..=15 {
            self.roads.insert((x, 9));
        }
        self.sync_citizens(c, false)
            .expect("founding camp has an open entrance");
        self.rev += 1;
    }
    pub fn valid(&self, c: &Catalog) -> bool {
        if self.is_empty() {
            return *self == Self::default();
        }
        if self.schema != 1
            || self.catalog != c.id
            || self.buildings.len() > MAX_BUILDINGS
            || self.claims.len() > MAX_CLAIMS
            || !self.secured_resources.iter().all(|r| c.resources.contains_key(r))
            || self.roads.len() > (WIDTH * HEIGHT) as usize
            || !c.valid_stock(&self.stock)
            || self.next < 2
            || self.rev == u64::MAX
            || self.shift == u64::MAX
            || self.second == u64::MAX
            || !self.researched.iter().all(|r| {
                c.technologies.contains_key(r)
                    && c.technologies[r]
                        .requires
                        .iter()
                        .all(|p| self.researched.contains(p))
            })
            || !c.valid_stock(&self.produced)
            || !c.valid_stock(&self.delivered)
            || self.shipments.len() > 256
            || !self.world.valid()
            || !self.energy_valid(c)
            || self
                .survey_goal
                .is_some_and(|p| !(0..3).contains(&p.0) || !(0..3).contains(&p.1))
            || !self.population.valid()
            || !self.motion.valid(WIDTH, HEIGHT)
            || self
                .motion
                .actors
                .keys()
                .any(|id| !self.population.citizens.contains_key(id))
            || self.food_reserve.is_some_and(|n| n > 3)
            || self.population.citizens.values().any(|p| {
                p.home.is_some_and(|id| !self.buildings.contains_key(&id))
                    || p.job.is_some_and(|id| !self.buildings.contains_key(&id))
            })
            || !self
                .bridges
                .iter()
                .all(|t| self.roads.contains(t) && self.world.terrain(*t).water())
            || self.shipments.iter().any(|s| {
                !c.resources.contains_key(&s.resource)
                    || !(1..=4).contains(&s.amount)
                    || !inside(s.tile)
                    || s.from == 0
                    || s.to == 0
                    || s.from >= self.next
                    || s.to >= self.next
            })
            || !self.roads.iter().all(|&t| {
                self.world.owned(t)
                    && (self.world.terrain(t).walkable() || self.bridges.contains(&t))
            })
            || self
                .buildings
                .get(&1)
                .is_none_or(|b| b.plan != "camp" || b.tile != CAMP || b.paused)
        {
            return false;
        }
        let mut tiles = BTreeSet::new();
        let mut linked = BTreeSet::new();
        self.claims.iter().all(|(id, q)| {
            !id.is_empty()
                && id.len() <= 96
                && q.name.len() <= 60
                && q.source.len() <= 120
                && c.resources.contains_key(&q.resource)
                && (1..=8).contains(&q.rate)
        }) && self.buildings.iter().all(|(&id, b)| {
            let Some(p) = c.plans.get(&b.plan) else {
                return false;
            };
            id > 0
                && id < self.next
                && p.connector.is_none()
                && (id == 1 || b.plan != "camp")
                && c.valid_stock(&b.input)
                && c.valid_stock(&b.output)
                && b.input.values().all(|n| *n <= 32)
                && b.output.values().all(|n| *n <= 32)
                && b.construction <= p.build_seconds
                && b.priority <= 2
                && b.production_limit <= 1_000_000
                && b.work_credit < 100
                && b.construction_credit < 100
                && inside(b.tile)
                && self.world.owned(b.tile)
                && self.world.terrain(b.tile).walkable()
                && b.tile != GATE
                && tiles.insert(b.tile)
                && (p.passable || !self.roads.contains(&b.tile))
                && b.recipe.as_ref().is_none_or(|r| p.recipes.contains(r))
                && b.progress < b.recipe.as_ref().map_or(1, |r| c.recipes[r].shifts)
                && b.claim.as_ref().is_none_or(|q| {
                    p.extractor && linked.insert(q) && self.claims.get(q).is_some_and(|q| q.secured)
                })
        })
    }
    pub fn at(&self, tile: Tile) -> Option<u32> {
        self.buildings
            .iter()
            .find(|(_, b)| b.tile == tile)
            .map(|(&id, _)| id)
    }
    pub fn amount(&self, resource: &str) -> u32 {
        self.stock.get(resource).copied().unwrap_or(0)
    }
    pub fn discover(&mut self, c: &Catalog, id: String, claim: Claim) -> bool {
        if self.claims.contains_key(&id) || self.claims.len() >= MAX_CLAIMS {
            return false;
        }
        if id.is_empty()
            || id.len() > 96
            || claim.name.len() > 60
            || claim.source.len() > 120
            || !c.resources.contains_key(&claim.resource)
            || !(1..=8).contains(&claim.rate)
            || claim.secured
        {
            return false;
        }
        self.start(c);
        self.claims.insert(id, claim);
        self.rev += 1;
        true
    }
    /// The Delve adapter calls this only after checking the real encounter and
    /// the knight's proximity. Claiming is idempotent; no output is granted here.
    pub fn secure(&mut self, id: &str) -> bool {
        let Some(q) = self.claims.get_mut(id) else {
            return false;
        };
        if q.secured {
            return false;
        }
        q.secured = true;
        self.secured_resources.insert(q.resource.clone());
        self.rev += 1;
        true
    }
    /// Commands are atomic. Failed previews or purchases change no bytes.
    pub fn apply(&mut self, c: &Catalog, command: Command) -> Result<String, String> {
        let mut next = self.clone();
        next.start(c);
        let message = next.execute(c, command)?;
        let sites = next.settlement_sites(c);
        next.population.reconcile(&sites)?;
        next.sync_citizens(c, false)?;
        next.rev = next.rev.checked_add(1).ok_or("kingdom revision limit")?;
        if !next.valid(c) {
            return Err("invalid kingdom change".into());
        }
        *self = next;
        Ok(message)
    }
    fn execute(&mut self, c: &Catalog, command: Command) -> Result<String, String> {
        match command {
            Command::Build { plan, tile } => {
                let p = c.plans.get(&plan).ok_or("unknown plan")?;
                if let Some(kind) = p.connector {
                    return self.lay_power(c, kind, tile);
                }
                if plan == "camp"
                    || tile == GATE
                    || !inside(tile)
                    || self.at(tile).is_some()
                    || (self.roads.contains(&tile) && !p.passable)
                    || self.buildings.len() >= MAX_BUILDINGS
                {
                    return Err("choose an empty plot".into());
                }
                self.check_site(c, &plan, tile)?;
                if !p.requires.iter().all(|r| self.researched.contains(r)) {
                    return Err(format!("research {} first", p.requires.join(", ")));
                }
                if !covers(&self.stock, &p.cost) {
                    return Err(format!("need {}", missing(&self.stock, &p.cost)));
                }
                let next_id = self.next.checked_add(1).ok_or("building limit")?;
                take(&mut self.stock, &p.cost);
                self.world.clear_forest(tile);
                self.buildings.insert(
                    self.next,
                    Building {
                        plan,
                        tile,
                        recipe: p.recipes.first().cloned(),
                        claim: None,
                        paused: false,
                        progress: 0,
                        construction: p.build_seconds,
                        ..Building::default()
                    },
                );
                self.next = next_id;
                Ok(format!("{} construction started", p.name))
            }
            Command::Road(tile) => {
                if !inside(tile)
                    || self
                        .at(tile)
                        .is_some_and(|id| !c.plans[&self.buildings[&id].plan].passable)
                    || self.roads.contains(&tile)
                {
                    return Err("choose an empty plot".into());
                }
                self.check_road_site(tile)?;
                let cost = Stock::from([("timber".into(), 1)]);
                if !covers(&self.stock, &cost) {
                    return Err("need 1 timber".into());
                }
                take(&mut self.stock, &cost);
                self.world.clear_forest(tile);
                self.roads.insert(tile);
                Ok("Road laid".into())
            }
            Command::Demolish(tile) => {
                if let Some(id) = self.at(tile) {
                    if id == 1 {
                        return Err("the founding camp must remain".into());
                    }
                    let b = self.buildings.remove(&id).unwrap();
                    for (r, n) in b.input.iter().chain(&b.output) {
                        add(&mut self.stock, r, *n);
                    }
                    let capacity = self.report(c).capacity;
                    for (r, n) in &c.plans[&b.plan].cost {
                        let room = capacity.saturating_sub(self.amount(r));
                        add(&mut self.stock, r, (n / 2).min(room));
                    }
                    Ok("Dismantled; half materials salvaged".into())
                } else if self.bridges.contains(&tile)
                    && crate::PowerKind::ALL
                        .into_iter()
                        .any(|kind| self.power.contains(kind, tile))
                {
                    Err("Remove power connectors before dismantling their bridge".into())
                } else if self.roads.remove(&tile) {
                    self.bridges.remove(&tile);
                    Ok("Road removed".into())
                } else {
                    Err("nothing on this plot".into())
                }
            }
            Command::Pause(id) => {
                if id == 1 {
                    return Err("the camp stays open".into());
                }
                let b = self.buildings.get_mut(&id).ok_or("no building")?;
                b.paused = !b.paused;
                Ok(if b.paused {
                    "Work paused"
                } else {
                    "Work resumed"
                }
                .into())
            }
            Command::Recipe { building, recipe } => {
                let b = self.buildings.get_mut(&building).ok_or("no building")?;
                if !c.plans[&b.plan].recipes.contains(&recipe) {
                    return Err("recipe needs a different workshop".into());
                }
                for (r, n) in std::mem::take(&mut b.input) {
                    add(&mut self.stock, &r, n);
                }
                b.recipe = Some(recipe);
                b.progress = 0;
                b.work_credit = 0;
                Ok("Recipe selected".into())
            }
            Command::Link { building, claim } => {
                if !self.claims.get(&claim).is_some_and(|q| q.secured) {
                    return Err("clear and claim the deposit in a delve".into());
                }
                if self
                    .buildings
                    .iter()
                    .any(|(&id, b)| id != building && b.claim.as_ref() == Some(&claim))
                {
                    return Err("this claim already supplies a mine".into());
                }
                let b = self.buildings.get_mut(&building).ok_or("no building")?;
                if !c.plans[&b.plan].extractor {
                    return Err("claims supply mine outposts".into());
                }
                b.claim = Some(claim);
                b.progress = 0;
                b.work_credit = 0;
                Ok("Claim linked to outpost".into())
            }
            Command::Unlink(building) => self.unlink_claim(c, building),
            Command::ReleaseClaim(claim) => self.release_claim(&claim),
            Command::Research(id) => {
                let tech = c.technologies.get(&id).ok_or("unknown charter")?;
                if self.researched.contains(&id) {
                    return Err("charter already researched".into());
                }
                if !tech.requires.iter().all(|r| self.researched.contains(r)) {
                    return Err(format!("requires {}", tech.requires.join(", ")));
                }
                if !covers(&self.stock, &tech.cost) {
                    return Err(format!("need {}", missing(&self.stock, &tech.cost)));
                }
                take(&mut self.stock, &tech.cost);
                self.researched.insert(id);
                Ok(format!("{} unlocked", tech.name))
            }
            Command::Provision => {
                self.provision = !self.provision;
                Ok(if self.provision {
                    "Next delve will draw supplies"
                } else {
                    "Expedition supply paused"
                }
                .into())
            }
            Command::Priority { building, priority } => {
                if priority > 2 {
                    return Err("priority must be low, normal or high".into());
                }
                let b = self.buildings.get_mut(&building).ok_or("no building")?;
                b.priority = priority;
                Ok(format!(
                    "{} work priority",
                    ["Low", "Normal", "High"][usize::from(priority)]
                ))
            }
            Command::ProductionLimit { building, amount } => {
                if amount > 1_000_000 {
                    return Err("stock target too large".into());
                }
                let b = self.buildings.get_mut(&building).ok_or("no building")?;
                b.production_limit = amount;
                Ok(if amount == 0 {
                    "Production target: unlimited".into()
                } else {
                    format!("Maintain {amount} of each product")
                })
            }
            Command::Survey(parcel) => self.survey(parcel),
            Command::SurveyGoal(parcel) => {
                if let Some(p) = parcel {
                    self.expansion_offer(p)?;
                }
                self.survey_goal = parcel;
                Ok(if parcel.is_some() {
                    "Survey objective pinned for your expeditions".into()
                } else {
                    "Survey objective cleared".into()
                })
            }
            Command::FoodReserve(meals) => {
                if meals > 3 {
                    return Err("Reserve zero to three meals per resident".into());
                }
                self.food_reserve = Some(meals);
                Ok(format!(
                    "Reserve {meals} meals per resident before cooking rations"
                ))
            }
            Command::Bridge(tile) => self.build_bridge(tile),
            Command::EnablePower(id) => self.enable_power(c, id),
            Command::PowerLine { kind, tile } => self.lay_power(c, kind, tile),
            Command::RemovePower { kind, tile } => self.remove_power(kind, tile),
        }
    }
    pub fn connected(&self) -> BTreeSet<u32> {
        let mut seen = BTreeSet::new();
        let mut queue: VecDeque<_> = neighbours(CAMP)
            .into_iter()
            .filter(|t| self.roads.contains(t))
            .collect();
        while let Some(tile) = queue.pop_front() {
            if !seen.insert(tile) {
                continue;
            }
            for n in neighbours(tile) {
                if self.roads.contains(&n) && !seen.contains(&n) {
                    queue.push_back(n);
                }
            }
        }
        self.buildings
            .iter()
            .filter(|(id, b)| **id == 1 || neighbours(b.tile).iter().any(|t| seen.contains(t)))
            .map(|(&id, _)| id)
            .collect()
    }
    pub fn job(&self, c: &Catalog, b: &Building) -> Option<(Stock, Stock, u16)> {
        let plan = &c.plans[&b.plan];
        if plan.power.as_ref().is_some_and(|p| p.is_source()) {
            None
        } else if plan.extractor {
            b.claim
                .as_ref()
                .and_then(|q| self.claims.get(q))
                .filter(|q| q.secured)
                .map(|q| (Stock::new(), Stock::from([(q.resource.clone(), q.rate)]), 1))
        } else {
            b.recipe.as_ref().map(|r| {
                let r = &c.recipes[r];
                (r.inputs.clone(), r.outputs.clone(), r.shifts)
            })
        }
    }
    /// Machines consume only their own delivered input buffer. Road carriers are
    /// the sole transfer mechanism between these buffers and the founding stock.
    pub fn report(&self, c: &Catalog) -> Report {
        let connected = self.connected();
        let population = self.population.report(&self.settlement_sites(c));
        let mut report = Report {
            workers: population.population,
            population,
            energy: self.energy_report(c),
            ..Report::default()
        };
        for (&id, b) in &self.buildings {
            if connected.contains(&id) && !b.paused && b.construction == 0 {
                let p = &c.plans[&b.plan];
                report.capacity += p.storage;
                report.carriers += p.carriers;
            }
        }
        for (&id, b) in &self.buildings {
            let p = &c.plans[&b.plan];
            let status = if b.paused {
                Status::Paused
            } else if b.construction > 0 {
                Status::Construction
            } else if !connected.contains(&id)
                && (p.workers > 0 || p.housing > 0 || p.storage > 0 || p.carriers > 0)
            {
                Status::Disconnected
            } else if let Some(power) = p.power.as_ref().filter(|p| p.is_source()) {
                if !b.power_enabled {
                    Status::Paused
                } else if !power.fuel.is_empty()
                    && b.fuel_left == 0
                    && !covers(&b.input, &power.fuel)
                {
                    Status::Fuel
                } else if report.population.assigned.get(&id).copied().unwrap_or(0) < p.workers {
                    Status::Workers
                } else if !power.fuel.is_empty() && b.fuel_left == 0 {
                    Status::Idle
                } else {
                    report.employed += p.workers;
                    Status::Working
                }
            } else if let Some((input, output, _)) = self.job(c, b) {
                if self.target_met(b, &output) {
                    Status::Target
                } else if !covers(&b.input, &input) {
                    Status::Inputs
                } else if output
                    .iter()
                    .any(|(r, n)| b.output.get(r).copied().unwrap_or(0) + n > 24)
                {
                    Status::Storage
                } else if report.population.assigned.get(&id).copied().unwrap_or(0) < p.workers {
                    Status::Workers
                } else if b.power_upgrade
                    && b.power_enabled
                    && p.power.as_ref().is_some_and(|p| p.is_consumer())
                    && self.energy_multiplier(c, id, &report.energy) == 0
                {
                    match report
                        .energy
                        .consumer(p.power.as_ref().unwrap().kind, id)
                        .map(|p| p.status)
                    {
                        Some(crate::PowerStatus::Overloaded) => Status::PowerOverloaded,
                        _ => Status::PowerDisconnected,
                    }
                } else {
                    report.employed += p.workers;
                    Status::Working
                }
            } else if p.extractor {
                Status::Unclaimed
            } else {
                Status::Idle
            };
            report.status.insert(id, status);
        }
        report
    }
    pub fn target_met(&self, b: &Building, output: &Stock) -> bool {
        b.production_limit > 0
            && !output.is_empty()
            && output.keys().all(|r| self.total(r) >= b.production_limit)
    }
    pub fn advance_second(&mut self, c: &Catalog) -> Report {
        if self.is_empty() {
            return Report::default();
        }
        self.second = self.second.saturating_add(1);
        let sites = self.settlement_sites(c);
        // A newly discovered claim may found the ledger while the party is
        // still underground. Needs start when the player first breaks ground,
        // so an unseen founding camp cannot starve during that first delve.
        let population = if self.next > 2 {
            self.population.advance_second(&sites, &mut self.stock)
        } else {
            self.population.reconcile(&sites)
        }
        .expect("validated kingdom population");
        for (&id, b) in &mut self.buildings {
            if !b.paused && b.construction > 0 {
                b.construction_credit += population.efficiency.get(&id).copied().unwrap_or(0);
                if b.construction_credit >= 100 {
                    b.construction_credit -= 100;
                    b.construction -= 1;
                }
            }
        }
        self.transport(c);
        self.energy_tick(c);
        let mut report = self.report(c);
        if self.second % 10 == 0 {
            let jobs: Vec<_> = self
                .buildings
                .iter()
                .filter(|(id, _)| report.status.get(id) == Some(&Status::Working))
                .filter_map(|(&id, b)| self.job(c, b).map(|j| (id, j)))
                .collect();
            for (id, (input, output, duration)) in jobs {
                let multiplier = self.energy_multiplier(c, id, &report.energy);
                let gain = report
                    .population
                    .efficiency
                    .get(&id)
                    .copied()
                    .unwrap_or(100)
                    * multiplier
                    / 100;
                let b = self.buildings.get_mut(&id).unwrap();
                let credit = b.work_credit + gain;
                b.work_credit = credit % 100;
                // A checked multiplier is at most300%; no unbounded loop or
                // idle work bank can manufacture free batches after a repair.
                for _ in 0..(credit / 100).min(3) {
                    let b = &self.buildings[&id];
                    if !covers(&b.input, &input)
                        || self.target_met(b, &output)
                        || output
                            .iter()
                            .any(|(r, n)| b.output.get(r).copied().unwrap_or(0) + n > 24)
                    {
                        break;
                    }
                    let b = self.buildings.get_mut(&id).unwrap();
                    b.progress += 1;
                    if b.progress >= duration {
                        take(&mut b.input, &input);
                        for (r, &n) in &output {
                            add(&mut b.output, r, n);
                            add(&mut report.outputs, r, n);
                            capped_add(&mut self.produced, r, n);
                        }
                        b.progress = 0;
                    }
                }
            }
            self.shift = self.shift.saturating_add(1);
        }
        self.rev = self.rev.saturating_add(1);
        self.sync_citizens(c, true)
            .expect("validated kingdom navigation");
        report
    }
    /// Advance a ten-second work shift, including every carrier movement.
    pub fn work(&mut self, c: &Catalog) -> Report {
        let mut report = Report::default();
        for _ in 0..10 {
            report = self.advance_second(c);
        }
        report
    }
    /// Paid once per hero at a real expedition start. A checked supply toggle
    /// cannot grant anything the workshops have not made.
    pub fn outfit(&mut self) -> (bool, bool, bool) {
        if !self.provision {
            return (false, false, false);
        }
        let mut result = [false; 3];
        for (i, r) in ["arms", "rations", "ward"].iter().enumerate() {
            if self.amount(r) > 0 {
                take(&mut self.stock, &Stock::from([((*r).into(), 1)]));
                result[i] = true;
            }
        }
        if result.iter().any(|b| *b) {
            self.rev += 1;
        }
        (result[0], result[1], result[2])
    }
}
pub fn label(stock: &Stock) -> String {
    stock
        .iter()
        .map(|(r, n)| format!("{n} {r}"))
        .collect::<Vec<_>>()
        .join(", ")
}
fn missing(stock: &Stock, cost: &Stock) -> String {
    label(
        &cost
            .iter()
            .filter_map(|(r, n)| {
                let n = n.saturating_sub(stock.get(r).copied().unwrap_or(0));
                (n > 0).then(|| (r.clone(), n))
            })
            .collect(),
    )
}

pub(crate) fn capped_add(stock: &mut Stock, resource: &str, amount: u32) {
    if amount > 0 {
        let n = stock.entry(resource.into()).or_default();
        *n = n.saturating_add(amount).min(1_000_000);
    }
}
