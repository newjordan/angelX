use super::{Catalog, Stock};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet, VecDeque};

pub const WIDTH: i16 = crate::world::WORLD_WIDTH;
pub const HEIGHT: i16 = crate::world::WORLD_HEIGHT;
pub const MAX_CLAIMS: usize = 64;
pub const MAX_BUILDINGS: usize = 400;
pub const MAX_LINES: usize = 400;
pub const MAX_SITES: usize = 16;
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
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub yields: BTreeMap<String, u32>,
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub surface: bool,
}
impl Claim {
    pub fn output(&self) -> Stock {
        if self.yields.is_empty() {
            Stock::from([(self.resource.clone(), self.rate)])
        } else {
            self.yields.clone()
        }
    }
    pub fn gives(&self, resource: &str) -> bool {
        self.resource == resource || self.yields.contains_key(resource)
    }
    pub fn resources(&self) -> impl Iterator<Item = &String> {
        std::iter::once(&self.resource).chain(self.yields.keys())
    }
    pub fn valid(&self, c: &Catalog) -> bool {
        c.resources.contains_key(&self.resource)
            && (1..=8).contains(&self.rate)
            && self.yields.len() <= 8
            && self
                .yields
                .iter()
                .all(|(r, n)| c.resources.contains_key(r) && (1..=8).contains(n))
    }
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
    pub production_limit: u32,
    pub work_credit: u16,
    pub construction_credit: u16,
    pub power_upgrade: bool,
    pub power_enabled: bool,
    pub fuel_left: u16,
    pub tool_left: u16,
    pub fed: u16,
    pub patch: Option<String>,
    pub span: u8,
    pub depth: u8,
    pub stats: Stats,
    pub carts: Vec<crate::carting::Cart>,
    pub facing: u8,
    pub clock: u16,
    pub lift: u8,
    pub filter: Option<String>,
    pub swape: bool,
}
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct Stats {
    pub worked: u32,
    pub starved: u32,
    pub backed_up: u32,
    pub unstaffed: u32,
    pub stalled: u32,
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
            tool_left: 0,
            fed: 0,
            patch: None,
            span: 0,
            depth: 0,
            stats: Stats::default(),
            carts: Vec::new(),
            facing: 1,
            clock: 0,
            lift: 0,
            filter: None,
            swape: false,
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
    pub food_reserve: Option<u8>,
    pub power: crate::PowerGrid,
    pub sites: BTreeMap<u32, crate::Site>,
    pub next_site: u32,
    pub lines: BTreeMap<u32, crate::Line>,
    pub next_line: u32,
    pub nature: BTreeMap<String, u32>,
    pub shaft_used: u32,
    pub shaft_moved: u32,
    pub spoiled: Stock,
    pub roadwork: crate::carting::Roadwork,
    pub batches: Stock,
    pub shaft_limit: Option<u32>,
    pub shaft_bucket: u32,
    #[serde(skip_serializing_if = "crate::ghosts::Plans::is_empty")]
    pub plans: crate::ghosts::Plans,
    #[serde(skip_serializing_if = "BTreeMap::is_empty")]
    pub copies: BTreeMap<String, crate::Pattern>,
    #[serde(skip)]
    pub books: std::sync::Arc<crate::books::Books>,
    pub rollers: crate::rollers::Rollers,
    pub rails: crate::rails::Rails,
    #[serde(skip_serializing_if = "crate::defence::Havoc::is_calm")]
    pub havoc: crate::defence::Havoc,
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
    FoundSite { plan: String, centre: Tile, yard: Tile },
    Line { kind: crate::LineKind, from: u32, to: u32, path: Vec<Tile>, vehicles: u16 },
    Fleet { line: u32, vehicles: u16 },
    Unline(u32),
    Carts { yard: u32, carts: u16 },
    Connect(u32),
    Teach { citizen: u32, trade: String },
    Upgrade(u32),
    Stamp { pattern: String, origin: Tile, turns: u8 },
    Plan { plan: String, tile: Tile, priority: u8, by: u32 },
    PlanStamp { pattern: String, origin: Tile, turns: u8, priority: u8, by: u32 },
    Unplan(u32),
    Rank { ghost: u32, priority: u8 },
    Retract(Vec<crate::Laid>),
    Clip { name: String, from: Tile, to: Tile },
    Forget(String),
    Roller { piece: crate::rollers::Piece, tile: Tile, dir: crate::rollers::Dir },
    Face { tile: Tile, dir: crate::rollers::Dir },
    Filter { building: u32, good: Option<String> },
    Rail(Tile),
    Bay { tile: Tile, name: Option<String> },
    Route { name: Option<String>, stops: Vec<crate::rails::Stop>, wagons: u16 },
    Wagons { route: u32, wagons: u16 },
    Unroute(u32),
    Railway { from: u32, to: u32, path: Vec<Tile>, wagons: u16, good: Option<String> },
    Mend(Tile),
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
    Tools,
    Depleted,
    Shaft,
    Fodder,
    Fire,
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
            Self::Tools => "WORN OUT TOOLS",
            Self::Depleted => "NOTHING LEFT TO WORK",
            Self::Shaft => "WAITING ON THE SHAFT",
            Self::Fodder => "THE OXEN NEED FODDER",
            Self::Fire => "ON FIRE",
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
    pub shaft: u32,
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
    fn surface_claim(&self, id: u32) -> bool {
        self.buildings
            .get(&id)
            .and_then(|b| b.claim.as_ref())
            .is_some_and(|claim| self.claims.get(claim).is_some_and(|q| q.surface))
    }

    pub fn is_empty(&self) -> bool {
        self.schema == 0
    }
    pub fn bank_coin(&mut self, n: u32) {
        if n > 0 && !self.is_empty() {
            crate::economy::add(&mut self.stock, "coin", n);
            std::sync::Arc::make_mut(&mut self.books).note_brought("coin", n);
            self.rev += 1;
        }
    }
    pub fn camp(&self) -> Tile {
        if self.world.is_hillside() { crate::hillside::HILL_CAMP } else { CAMP }
    }
    pub fn gate(&self) -> Tile {
        if self.world.is_hillside() { crate::hillside::HILL_GATE } else { GATE }
    }
    pub(crate) fn inside(&self, tile: Tile) -> bool {
        self.world.contains(tile)
    }
    pub fn found_hillside(&mut self, c: &Catalog, seed: u64) {
        if self.is_empty() {
            self.world = crate::World::hillside(seed);
            self.start(c);
        }
    }
    pub fn start(&mut self, c: &Catalog) {
        if !self.is_empty() {
            return;
        }
        self.schema = 1;
        self.catalog = c.id.clone();
        self.stock = if self.world.is_hillside() && !c.hillside_starter.is_empty() {
            c.hillside_starter.clone()
        } else {
            c.starter.clone()
        };
        self.next = 2;
        self.next_site = 1;
        self.next_line = 1;
        self.population = if self.world.is_hillside() {
            crate::population::Population::founders_of(usize::from(c.works.founding_people).max(crate::population::FOUNDERS))
        } else {
            crate::population::Population::founders()
        };
        let camp = self.camp();
        self.buildings.insert(
            1,
            Building {
                plan: "camp".into(),
                tile: camp,
                recipe: None,
                claim: None,
                paused: false,
                progress: 0,
                ..Building::default()
            },
        );
        if self.world.is_hillside() {
            for x in camp.0 - 4..=camp.0 + 4 {
                self.roads.insert((x, camp.1 + 1));
            }
            self.found_nature();
            if c.plans.contains_key("shaft-hand") {
                let tile = crate::hillside::HILL_SHAFT;
                self.roads.extend(crate::hillside::shaft_road());
                self.buildings.insert(
                    2,
                    Building { plan: "shaft-hand".into(), tile, ..Building::default() },
                );
                self.next = 3;
            }
            let rules = crate::population::Rules::of(c, true);
            for (i, (trade, level)) in c.founders.iter().enumerate() {
                self.population.grant(i as u32 + 1, trade, *level, &rules);
            }
        } else {
            for x in 8..=15 {
                self.roads.insert((x, 9));
            }
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
            || self.roads.len() > self.world.bounds().0 as usize * self.world.bounds().1 as usize
            || !c.valid_stock(&self.stock)
            || self.next < 2
            || self.rev == u64::MAX
            || self.shift == u64::MAX
            || self.second == u64::MAX
            || !self.researched.iter().all(|r| {
                c.tech(r).is_some()
                    && c.tech(r).unwrap()
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
            || !self.motion.valid(self.world.bounds().0, self.world.bounds().1)
            || !self.works_valid(c)
            || !self.plans.valid(self, c)
            || self.copies.len() > crate::ghosts::MAX_COPIES
            || self.copies.iter().any(|(k, p)| !crate::ghosts::COPY_NAMES.contains(&k.as_str()) || p.builds.len() > 64 || p.roads.len() > 400 || p.builds.iter().any(|b| !c.plans.contains_key(&b.plan)))
            || !self.rollers_valid(c)
            || !self.rails_valid(c)
            || !self.havoc.valid(self, c)
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
                    || !self.inside(s.tile)
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
                .is_none_or(|b| b.plan != "camp" || b.tile != self.camp() || b.paused)
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
                && q.valid(c)
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
                && b.input.values().all(|n| *n <= p.input_cap().max(32))
                && b.output.values().all(|n| *n <= p.output_cap().max(32))
                && b.construction <= p.build_seconds
                && b.priority <= 2
                && b.production_limit <= 1_000_000
                && b.work_credit < 100
                && b.construction_credit < 100
                && self.inside(b.tile)
                && self.world.owned(b.tile)
                && self.world.terrain(b.tile).walkable()
                && b.tile != self.gate()
                && self.covered(b).all(|t| {
                    self.inside(t) && self.world.terrain(t).walkable() && tiles.insert(t)
                })
                && (b.span, b.depth) == self.dims(p)
                && (p.passable || self.covered(b).all(|t| !self.roads.contains(&t)))
                && b.recipe.as_ref().is_none_or(|r| p.recipes.contains(r))
                && b.swape == (p.swape > 0)
                && b.facing < 4
                && b.lift < 3
                && b.filter.as_ref().is_none_or(|g| p.swape > 0 && c.resources.contains_key(g))
                && b.progress < b.recipe.as_ref().map_or(1, |r| c.recipes[r].shifts)
                && b.claim.as_ref().is_none_or(|q| {
                    p.extractor && linked.insert(q) && self.claims.get(q).is_some_and(|q| q.secured)
                })
                && b.carts.len() <= usize::from(p.stalls)
                && b.carts.iter().all(|x| {
                    c.valid_stock(&x.order)
                        && c.valid_stock(&x.load)
                        && x.route.len() <= 400
                        && x.tile.0 >= 0
                        && x.tile.1 >= 0
                        && x.tile.0 < self.world.bounds().0
                        && x.tile.1 < self.world.bounds().1
                        && Self::kilos(c, &x.load) <= Self::cart_capacity(c)
                })
        })
    }
    pub fn at(&self, tile: Tile) -> Option<u32> {
        self.buildings
            .iter()
            .find(|(_, b)| {
                b.tile == tile
                    || ((b.span > 0 || b.depth > 0)
                        && tile.0 >= b.tile.0
                        && tile.0 <= b.tile.0 + i16::from(b.span)
                        && tile.1 >= b.tile.1
                        && tile.1 <= b.tile.1 + i16::from(b.depth))
            })
            .map(|(&id, _)| id)
    }
    pub fn amount(&self, resource: &str) -> u32 {
        self.stock.get(resource).copied().unwrap_or(0)
    }
    pub fn receive(&mut self, c: &Catalog, resource: &str, amount: u32) -> u32 {
        if self.is_empty() || !c.resources.contains_key(resource) {
            return 0;
        }
        let room = self.report(c).capacity.saturating_sub(self.amount(resource));
        let taken = amount.min(room);
        if taken > 0 {
            add(&mut self.stock, resource, taken);
            std::sync::Arc::make_mut(&mut self.books).note_brought(resource, taken);
            self.rev = self.rev.saturating_add(1);
        }
        taken
    }
    pub fn spend(&mut self, c: &Catalog, cost: &Stock) -> Result<(), String> {
        if self.is_empty() {
            return Err("the kingdom is not founded".into());
        }
        if !c.valid_stock(cost) {
            return Err("unknown goods".into());
        }
        if !covers(&self.stock, cost) {
            let missing: Vec<String> = cost
                .iter()
                .filter(|(r, n)| self.amount(r) < **n)
                .map(|(r, n)| format!("{} {}", n - self.amount(r), c.resources[r].name.to_lowercase()))
                .collect();
            return Err(format!("need {}", missing.join(", ")));
        }
        take(&mut self.stock, cost);
        self.rev = self.rev.saturating_add(1);
        Ok(())
    }
    pub fn discover(&mut self, c: &Catalog, id: String, claim: Claim) -> bool {
        if self.claims.contains_key(&id) || self.claims.len() >= MAX_CLAIMS {
            return false;
        }
        if id.is_empty()
            || id.len() > 96
            || claim.name.len() > 60
            || claim.source.len() > 120
            || !claim.valid(c)
            || claim.secured
        {
            return false;
        }
        self.start(c);
        self.claims.insert(id, claim);
        self.rev += 1;
        true
    }
    pub fn secure(&mut self, id: &str) -> bool {
        let Some(q) = self.claims.get_mut(id) else {
            return false;
        };
        if q.secured {
            return false;
        }
        q.secured = true;
        self.secured_resources.extend(q.resources().cloned());
        self.rev += 1;
        true
    }
    pub fn apply(&mut self, c: &Catalog, command: Command) -> Result<String, String> {
        let mut next = self.clone();
        next.start(c);
        next.retire_cart_lines(c);
        if next.has_tramways() {
            next.migrate_tramways(c);
        }
        let message = next.execute(c, command)?;
        let sites = next.settlement_sites(c);
        next.population.reconcile_with(&sites, &crate::population::Rules::of(c, next.world.is_hillside()))?;
        next.sync_citizens(c, false)?;
        next.rev = next.rev.checked_add(1).ok_or("kingdom revision limit")?;
        if !next.valid(c) {
            return Err("invalid kingdom change".into());
        }
        *self = next;
        Ok(message)
    }
    pub(crate) fn execute(&mut self, c: &Catalog, command: Command) -> Result<String, String> {
        match command {
            Command::Build { plan, tile } => {
                let p = c.plans.get(&plan).ok_or("unknown plan")?;
                if let Some(kind) = p.connector {
                    return self.lay_power(c, kind, tile);
                }
                if plan == "camp"
                    || tile == self.gate()
                    || !self.inside(tile)
                    || self.at(tile).is_some()
                    || (self.roads.contains(&tile) && !p.passable)
                    || self.buildings.len() >= MAX_BUILDINGS
                {
                    return Err("choose an empty plot".into());
                }
                if self.rollers.cells.contains_key(&tile) || self.rails.cells.contains_key(&tile) {
                    return Err("a rollerway or a rail runs over this plot".into());
                }
                self.check_site(c, &plan, tile)?;
                if !p.requires.iter().all(|r| self.researched.contains(r)) {
                    return Err(format!("research {} first", p.requires.join(", ")));
                }
                if !covers(&self.stock, &p.cost) {
                    return Err(format!("need {}", missing(&self.stock, &p.cost)));
                }
                let next_id = self.next.checked_add(1).ok_or("building limit")?;
                self.check_footprint(c, &plan, tile)?;
                let patch = self.patch_for(c, p, tile)?;
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
                        patch,
                        span: self.dims(p).0,
                        depth: self.dims(p).1,
                        swape: p.swape > 0,
                        ..Building::default()
                    },
                );
                self.next = next_id;
                Ok(format!("{} construction started", p.name))
            }
            Command::Road(tile) => {
                if !self.inside(tile)
                    || self
                        .at(tile)
                        .is_some_and(|id| !c.plans[&self.buildings[&id].plan].passable)
                    || self.roads.contains(&tile)
                {
                    return Err("choose an empty plot".into());
                }
                if self.rollers.cells.contains_key(&tile) || self.rails.cells.contains_key(&tile) {
                    return Err("a rollerway or a rail runs over this plot".into());
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
                    if self.sites.values().any(|s| s.yard == id) {
                        return Err("a castle's stockyard stays while the castle stands".into());
                    }
                    let b = self.buildings.remove(&id).unwrap();
                    for (r, n) in b.input.iter().chain(&b.output) {
                        add(&mut self.stock, r, *n);
                    }
                    self.release_carts(c, b.carts.clone());
                    self.remove_lines_of(id);
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
                } else if self.lift_roller(tile) {
                    Ok("Rollerway taken up; its goods go to the camp".into())
                } else if self.rails.cells.contains_key(&tile) {
                    self.lift_rail(tile)
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
                let tech = c.tech(&id).ok_or("unknown charter")?;
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
            Command::FoundSite { plan, centre, yard } => self.found_site(c, &plan, centre, yard),
            Command::Line { kind, from, to, path, vehicles } => {
                self.lay_line(c, kind, from, to, path, vehicles)
            }
            Command::Fleet { line, vehicles } => self.set_fleet(c, line, vehicles),
            Command::Carts { yard, carts } => self.set_carts(c, yard, carts),
            Command::Unline(id) => {
                if self.remove_line(id) {
                    Ok("Line taken down; its cargo goes to the camp".into())
                } else {
                    Err("no such line".into())
                }
            }
            Command::Stamp { pattern, origin, turns } => self.stamp(c, &pattern, origin, turns),
            Command::Plan { plan, tile, priority, by } => self.plan_ghost(c, &plan, tile, priority, by),
            Command::PlanStamp { pattern, origin, turns, priority, by } => self.plan_stamp(c, &pattern, origin, turns, priority, by).map(|(words, _)| words),
            Command::Unplan(id) => self.unplan(id),
            Command::Rank { ghost, priority } => self.rank_ghost(ghost, priority),
            Command::Retract(what) => self.retract(c, &what),
            Command::Clip { name, from, to } => self.clip(c, &name, from, to),
            Command::Forget(name) => {
                if self.copies.remove(&name).is_some() {
                    Ok("Copy let go".into())
                } else {
                    Err("no such copy".into())
                }
            }
            Command::Rail(tile) => self.lay_rail(c, tile),
            Command::Mend(tile) => self.mend(c, tile),
            Command::Bay { tile, name } => self.make_bay(c, tile, None, name),
            Command::Route { name, stops, wagons } => self.make_route(c, name, stops, wagons),
            Command::Wagons { route, wagons } => {
                let have = self.rails.wagons.values().filter(|w| w.route == route).count() as u16;
                if wagons > have {
                    self.add_wagons(c, route, wagons - have)
                } else if wagons < have {
                    let mut said = Err("no wagon to sell".to_string());
                    for _ in 0..have - wagons {
                        said = self.sell_wagon(c, route);
                    }
                    said
                } else {
                    Ok("It runs as it did".into())
                }
            }
            Command::Railway { from, to, path, wagons, good } => self.lay_railway(c, from, to, path, wagons, good),
            Command::Unroute(id) => {
                if self.remove_route(id) {
                    Ok("Route taken down; its wagons are sold".into())
                } else {
                    Err("no such route".into())
                }
            }
            Command::Roller { piece, tile, dir } => self.lay_roller(c, piece, tile, dir),
            Command::Face { tile, dir } => self.face(c, tile, dir),
            Command::Filter { building, good } => self.filter(c, building, good),
            Command::Teach { citizen, trade } => self.teach(c, citizen, &trade),
            Command::Upgrade(id) => self.upgrade(c, id),
            Command::Connect(id) => {
                let tiles = self.plan_connection(c, id)?;
                if tiles.is_empty() {
                    return Ok("Already connected to the camp".into());
                }
                let cost = Stock::from([("timber".to_string(), tiles.len() as u32)]);
                if !covers(&self.stock, &cost) {
                    return Err(format!("need {}", missing(&self.stock, &cost)));
                }
                take(&mut self.stock, &cost);
                for t in &tiles {
                    self.world.clear_forest(*t);
                    self.roads.insert(*t);
                }
                Ok(format!("{} road tiles laid to the camp", tiles.len()))
            }
        }
    }
    pub fn connected(&self) -> BTreeSet<u32> {
        let mut seen = BTreeSet::new();
        let mut queue: VecDeque<_> = neighbours(self.camp())
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
            .filter(|(id, b)| **id == 1 || b.swape || self.covered(b).any(|c| neighbours(c).iter().any(|t| seen.contains(t))))
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
                .map(|q| (Stock::new(), q.output(), 1))
        } else {
            b.recipe.as_ref().map(|r| {
                let r = &c.recipes[r];
                (r.inputs.clone(), r.outputs.clone(), r.shifts)
            })
        }
    }
    pub fn report(&self, c: &Catalog) -> Report {
        let connected = self.connected();
        let population = self.population.report_with(&self.settlement_sites(c), &crate::population::Rules::of(c, self.world.is_hillside()));
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
            } else if self.havoc.fires.contains_key(&id) {
                Status::Fire
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
                    .any(|(r, n)| !(p.extractor || c.resources[r].waste) && b.output.get(r).copied().unwrap_or(0) + n > 24)
                {
                    Status::Storage
                } else if let Some(blocked) = self.post_blocked(c, b) {
                    blocked
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
            } else if p.shaft > 0 {
                if report.population.assigned.get(&id).copied().unwrap_or(0) < p.workers {
                    Status::Workers
                } else if let Some(blocked) = self.post_blocked(c, b) {
                    blocked
                } else {
                    report.employed += p.workers;
                    let mult = if p.power.is_some() && b.power_upgrade && b.power_enabled {
                        u32::from(self.energy_multiplier(c, id, &report.energy))
                    } else {
                        100
                    };
                    report.shaft += p.shaft * mult.max(1) / 100;
                    Status::Working
                }
            } else if p.swape > 0 {
                if report.population.assigned.get(&id).copied().unwrap_or(0) < p.workers {
                    Status::Workers
                } else if b.power_upgrade
                    && b.power_enabled
                    && p.power.as_ref().is_some_and(|p| p.is_consumer())
                    && self.energy_multiplier(c, id, &report.energy) == 0
                {
                    match report.energy.consumer(p.power.as_ref().unwrap().kind, id).map(|p| p.status) {
                        Some(crate::PowerStatus::Overloaded) => Status::PowerOverloaded,
                        _ => Status::PowerDisconnected,
                    }
                } else if b.lift == 1 {
                    Status::Inputs
                } else if b.lift == 2 {
                    Status::Storage
                } else {
                    report.employed += p.workers;
                    Status::Working
                }
            } else if p.watch > 0 {
                if report.population.assigned.get(&id).copied().unwrap_or(0) < p.workers {
                    Status::Workers
                } else if b.input.get(crate::defence::ARROW).copied().unwrap_or(0) == 0 {
                    Status::Inputs
                } else {
                    report.employed += p.workers;
                    Status::Working
                }
            } else if p.beacon {
                if b.fed > 0 { Status::Working } else { Status::Fuel }
            } else if p.stalls > 0 {
                if report.population.assigned.get(&id).copied().unwrap_or(0) < p.workers {
                    Status::Workers
                } else if !b.carts.is_empty() && b.fed == 0 {
                    Status::Fodder
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
            && output
                .iter()
                .max_by(|a, b| a.1.cmp(b.1).then(b.0.cmp(a.0)))
                .is_some_and(|(r, _)| self.total(r) >= b.production_limit)
    }
    pub fn advance_second(&mut self, c: &Catalog) -> Report {
        if self.is_empty() {
            return Report::default();
        }
        if self.buildings.values().any(|b| {
            !c.plans.contains_key(&b.plan)
                || b.recipe.as_ref().is_some_and(|r| !c.recipes.contains_key(r))
        }) {
            return Report::default();
        }
        self.second = self.second.saturating_add(1);
        self.books_roll(c);
        self.raise_second(c);
        self.defence_second(c);
        if self.lines.values().any(|l| l.kind == crate::LineKind::Cart) {
            self.retire_cart_lines(c);
        }
        if self.has_tramways() {
            self.migrate_tramways(c);
        }
        let sites = self.settlement_sites(c);
        let population = if self.next > 2 {
            self.population.advance_with(&sites, &mut self.stock, &crate::population::Rules::of(c, self.world.is_hillside()))
        } else {
            self.population.reconcile_with(&sites, &crate::population::Rules::of(c, self.world.is_hillside()))
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
        self.draw_upkeep(c);
        self.transport(c);
        self.energy_tick(c);
        let mut report = self.report(c);
        self.rollers_tick(c, &report);
        self.rails_tick(c, &report);
        let books = std::sync::Arc::make_mut(&mut self.books);
        for (id, status) in &report.status {
            let b = self.buildings.get_mut(id).expect("reported building exists");
            books.note_second(
                *id,
                matches!(status, Status::Working),
                matches!(status, Status::Inputs | Status::Storage | Status::Workers | Status::Disconnected | Status::PowerDisconnected | Status::PowerOverloaded | Status::Fuel | Status::Tools | Status::Fodder | Status::Shaft),
            );
            let n = match status {
                Status::Working => &mut b.stats.worked,
                Status::Inputs => &mut b.stats.starved,
                Status::Storage => &mut b.stats.backed_up,
                Status::Workers => &mut b.stats.unstaffed,
                Status::Disconnected
                | Status::PowerDisconnected
                | Status::PowerOverloaded
                | Status::Fuel => &mut b.stats.stalled,
                _ => continue,
            };
            *n = n.saturating_add(1);
        }
        if self.second % crate::pace::job_every() == 0 {
            let jobs: Vec<_> = self
                .buildings
                .iter()
                .filter(|(id, _)| report.status.get(id) == Some(&Status::Working))
                .filter_map(|(&id, b)| self.job(c, b).map(|j| (id, j)))
                .collect();
            let limited = self.world.is_hillside() && c.plans.values().any(|p| p.shaft > 0);
            let lift = self.shift_lift(report.shaft);
            self.shaft_used = 0;
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
                let plan = &c.plans[&self.buildings[&id].plan];
                let patch = self.buildings[&id]
                    .recipe
                    .as_ref()
                    .and_then(|r| c.recipes[r].patch.as_ref());
                let units: u32 = output.values().sum();
                let b = self.buildings.get_mut(&id).unwrap();
                let credit = b.work_credit + gain;
                b.work_credit = credit % 100;
                for _ in 0..(credit / 100).min(3) {
                    {
                        let b = &self.buildings[&id];
                        if !covers(&b.input, &input)
                            || self.target_met(b, &output)
                            || output
                                .iter()
                                .any(|(r, n)| !(plan.extractor || c.resources[r].waste) && b.output.get(r).copied().unwrap_or(0) + n > 24)
                            || (limited && plan.extractor && !self.surface_claim(id) && self.shaft_used + units > lift)
                            || patch.is_some_and(|pt| !self.patch_ok(b, pt))
                            || (!plan.tool.is_empty() && b.tool_left == 0 && self.amount(&plan.tool) == 0)
                        {
                            break;
                        }
                    }
                    if !plan.tool.is_empty() && self.buildings[&id].tool_left == 0 {
                        take(&mut self.stock, &Stock::from([(plan.tool.clone(), 1)]));
                        self.buildings.get_mut(&id).unwrap().tool_left = plan.tool_life.max(1);
                    }
                    let b = self.buildings.get_mut(&id).unwrap();
                    b.progress += 1;
                    let mut worn = false;
                    if !plan.tool.is_empty() {
                        b.tool_left -= 1;
                        worn = b.tool_left == 0;
                    }
                    if b.progress >= duration {
                        take(&mut b.input, &input);
                        for (r, &n) in &output {
                            add(&mut b.output, r, n);
                            add(&mut report.outputs, r, n);
                            capped_add(&mut self.produced, r, n);
                            std::sync::Arc::make_mut(&mut self.books).note_made(r, n);
                            if plan.extractor || c.resources[r].waste {
                                if let Some(held) = b.output.get_mut(r) {
                                    *held = (*held).min(24);
                                }
                            }
                        }
                        b.progress = 0;
                        std::sync::Arc::make_mut(&mut self.books).note_batch(id);
                        if let (Some(pt), Some(pid)) = (patch, b.patch.clone()) {
                            if let Some(left) = self.nature.get_mut(&pid) {
                                *left = left.saturating_sub(pt.takes);
                            }
                        }
                        if let Some(r) = self.buildings[&id].recipe.clone() {
                            capped_add(&mut self.batches, &r, 1);
                        }
                        if plan.extractor && !self.surface_claim(id) {
                            self.shaft_used += units;
                        }
                    }
                    if worn {
                        add(&mut self.stock, &format!("worn-{}", plan.tool), 1);
                    }
                }
            }
            self.shaft_spent();
            self.shaft_moved = self.shaft_used;
            self.shaft_used = 0;
            self.spoil(c);
            self.regrow(c);
            self.shift = self.shift.saturating_add(1);
        }
        self.rev = self.rev.saturating_add(1);
        self.sync_citizens(c, true)
            .expect("validated kingdom navigation");
        report
    }
    pub fn work(&mut self, c: &Catalog) -> Report {
        let mut report = Report::default();
        for _ in 0..10 {
            report = self.advance_second(c);
        }
        report
    }
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
pub(crate) fn missing(stock: &Stock, cost: &Stock) -> String {
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
