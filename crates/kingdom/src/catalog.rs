use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::sync::OnceLock;

pub type Stock = BTreeMap<String, u32>;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Resource {
    pub name: String,
    pub colour: u32,
    /// Seconds a unit keeps before it goes off, zero for goods that keep.
    /// Mortar sets, so the mill must stand near the wall.
    #[serde(default)]
    pub shelf: u32,
    /// A by-product nobody is asked to haul away: when a maker's store of it
    /// is full the surplus is tipped on the midden, it never blocks the work.
    #[serde(default)]
    pub waste: bool,
    /// What one unit weighs, in kilograms. A cart carries a tonne, so a cart
    /// takes ten of the heavy goods (ashlar), twenty sacks, and hundreds of
    /// nails. Zero means the default of fifty.
    #[serde(default)]
    pub weight: u16,
}
impl Resource {
    pub fn kilos(&self) -> u32 {
        if self.weight == 0 { 50 } else { u32::from(self.weight) }
    }
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Recipe {
    pub name: String,
    pub inputs: Stock,
    pub outputs: Stock,
    pub shifts: u16,
    /// Draws on a finite patch of the hillside: a wood, the field stone.
    #[serde(default)]
    pub patch: Option<Patch>,
    /// The skill level of the person who must supervise this recipe, when it
    /// is higher than the building's own (steel wants a master smith).
    #[serde(default)]
    pub level: u8,
}
/// A recipe's claim on the land. A batch takes `takes` units from the patch
/// and cannot run if fewer than `takes` remain, or (for coppicing, which
/// takes none) if the patch has fallen under `min_percent` of what it began
/// with. Patches regrow while they are above a quarter.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Patch {
    /// `wood` or `field`.
    pub kind: String,
    pub takes: u32,
    #[serde(default)]
    pub min_percent: u8,
}
fn yes() -> bool {
    true
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Plan {
    pub name: String,
    pub glyph: String,
    pub colour: u32,
    pub cost: Stock,
    pub workers: u16,
    pub housing: u16,
    pub storage: u32,
    pub recipes: Vec<String>,
    #[serde(default)]
    pub extractor: bool,
    #[serde(default)]
    pub requires: Vec<String>,
    #[serde(default)]
    pub build_seconds: u16,
    #[serde(default)]
    pub carriers: u16,
    #[serde(default)]
    pub siting: Siting,
    #[serde(default)]
    pub passable: bool,
    #[serde(default)]
    pub power: Option<crate::power::MachinePower>,
    #[serde(default)]
    pub connector: Option<crate::PowerKind>,
    /// Goods held at each end of this building's loading dock. Zero keeps the
    /// workshop default: 32 in, 24 out. Yards and depots hold far more.
    #[serde(default)]
    pub buffer: u32,
    /// Vehicles that can load or unload here at once. Zero means one.
    #[serde(default)]
    pub berths: u8,
    /// A transfer yard: takes any goods into its store and lines carry them on.
    #[serde(default)]
    pub hub: bool,
    /// A castle stockyard: takes the goods its castle's bill needs.
    #[serde(default)]
    pub yard: bool,
    /// A carter's yard: stalls for this many carts, which serve every road the
    /// yard touches. Zero means it keeps none.
    #[serde(default)]
    pub stalls: u16,
    /// A quay: where barge lines begin and end.
    #[serde(default)]
    pub quay: bool,
    /// The trade the building's work needs, and the level of the person who
    /// supervises it (1 apprentice, 2 journeyman, 3 master).
    #[serde(default)]
    pub trade: String,
    #[serde(default)]
    pub level: u8,
    /// The tool the work wears out, and how many shifts one lasts. A worn
    /// tool goes back to a smith; the worn good is `worn-<tool>`.
    #[serde(default)]
    pub tool: String,
    #[serde(default)]
    pub tool_life: u16,
    /// Goods drawn from the camp every `upkeep_seconds`: a horse gin's fodder.
    #[serde(default)]
    pub upkeep: Stock,
    #[serde(default)]
    pub upkeep_seconds: u16,
    /// Extra tiles the building runs on to the east: a ropewalk is a long
    /// straight shed you must find room for.
    #[serde(default)]
    pub span: u8,
    /// Extra tiles the building runs on to the south (a sawpit is three wide
    /// and two deep: span 2, depth 1). Its tile is its top-left corner; the
    /// hillside builds it to this size, the valley as one tile.
    #[serde(default)]
    pub depth: u8,
    /// Blocks a minute this building lifts up the Great Shaft.
    #[serde(default)]
    pub shaft: u32,
    /// What working it adds to the Deep's stir.
    #[serde(default)]
    pub smoke: u8,
    #[serde(default)]
    pub noise: u8,
    /// Percent more stones a mason sets within range of this crane.
    #[serde(default)]
    pub crane: u8,
    /// Which world may build it. The valley keeps its own works.
    #[serde(default = "yes")]
    pub valley: bool,
    #[serde(default = "yes")]
    pub hillside: bool,
    /// The plan this one is an upgrade of, in place.
    #[serde(default)]
    pub replaces: Option<String>,
    /// Seconds a swape's arm takes to lift one good between the two things it
    /// stands between (a rollerway and a bench, a store, the camp); zero for
    /// every work that is not a swape.
    #[serde(default)]
    pub swape: u8,
    /// What a raid must break through to bring this down: a palisade, a wall, a
    /// tower, a brazier. Zero is not a defence (`defence`).
    #[serde(default)]
    pub hp: u16,
    /// A watchtower's reach, in tiles, for its archer's arrows; zero is not a tower.
    #[serde(default)]
    pub watch: u8,
    /// The arrows a tower keeps in its quiver: the roads and rollerways keep it full.
    #[serde(default)]
    pub quiver: u8,
    /// A brazier: while its fuel lasts it is lit, and the Deep's things come to the light.
    #[serde(default)]
    pub beacon: bool,
}
impl Plan {
    pub fn input_cap(&self) -> u32 {
        if self.buffer > 0 { self.buffer } else { 32 }
    }
    pub fn output_cap(&self) -> u32 {
        if self.buffer > 0 { self.buffer } else { 24 }
    }
    pub fn berth_count(&self) -> u8 {
        self.berths.max(1)
    }
}

/// Landscape requirements are content, rather than renderer-specific rules.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Siting {
    #[default]
    Any,
    Clay,
    Stone,
    Woodland,
    Riverbank,
    /// Sand along the river.
    Sand,
    /// High ground where the wind blows.
    Hilltop,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Technology {
    pub name: String,
    pub description: String,
    pub cost: Stock,
    pub requires: Vec<String>,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Catalog {
    pub schema: u32,
    pub id: String,
    pub resources: BTreeMap<String, Resource>,
    pub recipes: BTreeMap<String, Recipe>,
    pub plans: BTreeMap<String, Plan>,
    pub menu: Vec<String>,
    pub starter: Stock,
    #[serde(default)]
    pub technologies: BTreeMap<String, Technology>,
    /// The hillside's charters. They live apart from the valley's so the
    /// valley's Charters page and its numbering are what they always were.
    #[serde(default)]
    pub hillside_technologies: BTreeMap<String, Technology>,
    /// The hillside game: what is hauled, how castles are paid for, and what
    /// can be stamped down. All optional so an older catalogue still parses.
    #[serde(default)]
    pub works: Works,
    #[serde(default)]
    pub lines: BTreeMap<String, LineSpec>,
    /// The pieces a rollerway is laid of, by key (`roller::Piece::key`).
    #[serde(default)]
    pub rollers: BTreeMap<String, PieceSpec>,
    #[serde(default)]
    pub bill: BTreeMap<String, BillRule>,
    #[serde(default)]
    pub fixtures: Vec<Fixture>,
    #[serde(default)]
    pub castles: BTreeMap<String, CastlePlan>,
    #[serde(default)]
    pub patterns: BTreeMap<String, Pattern>,
    /// What the hillside founds with, in place of `starter`.
    #[serde(default)]
    pub hillside_starter: Stock,
    /// The hillside's build menu (the valley's stays `menu`).
    #[serde(default)]
    pub hillside_menu: Vec<String>,
    /// How much of each good the camp keeps in hand. The camp is the first
    /// warehouse: it pulls up to these levels from whoever has the goods, and
    /// producers push to it only while it is under them.
    #[serde(default)]
    pub camp_stock: BTreeMap<String, u32>,
    /// The trades: who can supervise whom, and who the founders are.
    #[serde(default)]
    pub trades: BTreeMap<String, Trade>,
    #[serde(default)]
    pub founders: Vec<(String, u8)>,
    #[serde(default)]
    pub diet: Diet,
    /// What each Delve pack gives: the quarry stone and what else comes up.
    #[serde(default)]
    pub geology: BTreeMap<String, Geology>,
    /// Goods that are deliberately made or used by systems outside this sim
    /// (siege stores, extension ores). The checker lets these stand.
    #[serde(default)]
    pub reserved: Vec<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Trade {
    pub name: String,
    /// Trades whose journeymen can supervise this one at one level lower.
    #[serde(default)]
    pub kin: Vec<String>,
}
/// What a person eats at a meal, by how skilled they are. An entry such as
/// `meat|cheese` is satisfied by either.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Diet {
    pub staple: String,
    pub labourer: Vec<String>,
    pub skilled: Vec<String>,
    pub master: Vec<String>,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Geology {
    pub name: String,
    /// The quarry stone this pack's charter yields.
    pub stone: String,
    #[serde(default)]
    pub also: Vec<String>,
    /// What the castle looks like built of it.
    pub look: String,
}

/// A quantity of a castle's bill, in thousandths of the bill's own unit (a
/// stone, a cubic metre, a kilogram), so bills stay exact integers.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord)]
pub struct Qty(pub u64);
impl Serialize for Qty {
    fn serialize<S: serde::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        if self.0 % 1000 == 0 {
            s.serialize_u64(self.0 / 1000)
        } else {
            s.serialize_f64(self.0 as f64 / 1000.0)
        }
    }
}
impl<'de> Deserialize<'de> for Qty {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        let v = f64::deserialize(d)?;
        if !v.is_finite() || !(0.0..=4_000_000.0).contains(&v) {
            return Err(serde::de::Error::custom("bill quantity out of range"));
        }
        Ok(Self((v * 1000.0).round() as u64))
    }
}

/// Numbers of the works that are not a recipe or a building.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Works {
    /// Percent of a stone one mason sets in a ten second shift.
    pub mason_credit: u16,
    /// Tiles (Manhattan) from a stockyard within which a lodge serves it.
    pub lodge_range: u8,
    /// Scaffold poles that must stand in the yard before masons may climb,
    /// and the poles a finished lift wears out.
    pub poles_standing: u32,
    pub poles_worn: u32,
    /// Sets of arch centring that must wait in the yard while a lift holds
    /// voussoirs, and the sets a finished lift breaks.
    pub centring_standing: u32,
    pub centring_worn: u32,
    /// The earthworks gang digs this many units per worker per shift.
    pub dig_per_worker: u32,
    /// Seconds of work at a trade to become apprentice, journeyman, master.
    pub apprentice_xp: u32,
    pub journeyman_xp: u32,
    pub master_xp: u32,
    /// One qualified supervisor serves this many posts of a trade.
    pub posts_per_supervisor: u8,
    /// Wages: every this many seconds a journeyman is paid `wage_journeyman`
    /// coin and a master `wage_master`. Coin is the Delve's gold.
    pub wage_seconds: u32,
    pub wage_journeyman: u32,
    pub wage_master: u32,
    /// Coin a teacher charges to bring someone one level on in a trade.
    pub tuition: u32,
    /// Seconds a coat of livery lasts.
    pub livery_seconds: u32,
    /// The most perishable stock a yard asks for.
    pub perishable_stock: u32,
    /// Units a shift each healthy patch regrows.
    pub regrow: u32,
    /// How many stages ahead a yard asks its carts to stock.
    pub lookahead: u8,
    /// Seconds a welcoming camp waits before the next newcomer walks in.
    pub arrival_seconds: u16,
    /// People in the founding wagon train (the founders list gives trades to
    /// the first of them).
    pub founding_people: u8,
    /// Carts that can pass a road tile at once before the road jams, and the
    /// percent each extra cart adds to the time a tile takes.
    pub road_lanes: u8,
    pub jam_percent: u16,
    /// Meals of bread the camp must hold per person before a newcomer is welcomed.
    pub welcome_meals: u8,
}
impl Default for Works {
    fn default() -> Self {
        Self {
            mason_credit: 125,
            lodge_range: 14,
            poles_standing: 24,
            poles_worn: 6,
            centring_standing: 6,
            centring_worn: 2,
            dig_per_worker: 4,
            apprentice_xp: 300,
            journeyman_xp: 1800,
            master_xp: 5400,
            posts_per_supervisor: 4,
            wage_seconds: 600,
            wage_journeyman: 1,
            wage_master: 3,
            tuition: 6,
            livery_seconds: 3600,
            perishable_stock: 60,
            regrow: 2,
            lookahead: 4,
            arrival_seconds: 120,
            founding_people: 6,
            road_lanes: 2,
            jam_percent: 50,
            welcome_meals: 2,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LineKind {
    Cart,
    Tram,
    Rope,
    Chute,
    Barge,
    /// A string of pack mules on a trodden way: no road, no track, small loads.
    Mule,
}
impl LineKind {
    pub const ALL: [Self; 6] = [Self::Cart, Self::Tram, Self::Rope, Self::Chute, Self::Barge, Self::Mule];
    pub fn key(self) -> &'static str {
        match self {
            Self::Cart => "cart",
            Self::Tram => "tram",
            Self::Rope => "rope",
            Self::Chute => "chute",
            Self::Barge => "barge",
            Self::Mule => "mule",
        }
    }
}

/// One way of moving goods, all of it data: how much a vehicle carries, how
/// long a tile takes, how long a berth holds it, and what it costs.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LineSpec {
    pub name: String,
    pub vehicle: String,
    pub capacity: u16,
    /// Tenths of a second per tile, loaded, along the way that is easy.
    pub speed: u16,
    /// The same against the grain: a barge against the current. Zero means
    /// the same as `speed`.
    #[serde(default)]
    pub against: u16,
    /// Empty vehicles return at once (a chute is gravity and nothing else).
    #[serde(default)]
    pub instant_return: bool,
    /// Seconds a vehicle holds a berth to load, and again to unload.
    pub dwell: u16,
    #[serde(default)]
    pub tile_cost: Stock,
    pub vehicle_cost: Stock,
    pub max_vehicles: u16,
    pub max_length: u16,
    #[serde(default)]
    pub requires: Vec<String>,
    /// What each vehicle eats from the camp every `upkeep_seconds`: oxen and
    /// fodder. A line that cannot be fed stands still.
    #[serde(default)]
    pub upkeep: Stock,
    #[serde(default)]
    pub upkeep_seconds: u16,
}

/// One piece of a rollerway: what it costs and how fast it moves goods.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PieceSpec {
    pub name: String,
    pub cost: Stock,
    #[serde(default)]
    pub requires: Vec<String>,
    /// How many of the four beats a second it moves goods on, one slot a beat:
    /// a hand-chain one, a roller two, a geared run four.
    pub beats: u8,
    /// Labourers it keeps at work: a hand-chain keeps one a tile.
    #[serde(default)]
    pub crew: u8,
}

/// How a bill line of `angel-castle/v1` becomes chain goods.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BillRule {
    pub resource: String,
    /// Bill units per one unit of the good, in thousandths.
    pub per: Qty,
    /// One in `one_in` of the units is `other` instead (voussoirs are one in
    /// thirty-two of a great tower's cut stone).
    #[serde(default)]
    pub other: Option<String>,
    #[serde(default)]
    pub one_in: u32,
    /// Stages whose id holds a word are paid in another good: roof timber is
    /// shingle, not joist.
    #[serde(default)]
    pub overrides: Vec<BillOverride>,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BillOverride {
    pub stage: String,
    pub resource: String,
    pub per: Qty,
}

/// Goods a stage needs beyond its bill, matched by a word in the stage id.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Fixture {
    pub stage: String,
    pub needs: Stock,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CastleStage {
    pub id: String,
    pub name: String,
    pub first: u32,
    pub count: u32,
    pub bill: BTreeMap<String, Qty>,
}
/// The stage table of a baked `angel-castle/v1` castle: everything the
/// production game needs from the file, none of the stones.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CastlePlan {
    pub name: String,
    /// Width and depth of the ground it covers, in tiles, gate facing south.
    pub footprint: (i16, i16),
    pub stages: Vec<CastleStage>,
}
impl CastlePlan {
    pub fn stones(&self) -> u32 {
        self.stages.iter().map(|s| s.count).sum()
    }
    /// Reads the stage table from a whole castle file, or from one that has
    /// only the table. Anything else in the file is ignored.
    pub fn from_castle_json(name: &str, footprint: (i16, i16), json: &str) -> Result<Self, String> {
        #[derive(Deserialize)]
        struct File {
            schema: String,
            stages: Vec<CastleStage>,
        }
        let f: File = serde_json::from_str(json).map_err(|e| e.to_string())?;
        if f.schema != "angel-castle/v1" {
            return Err("not an angel-castle/v1 file".into());
        }
        let plan = Self {
            name: name.into(),
            footprint,
            stages: f.stages,
        };
        plan.validate()?;
        Ok(plan)
    }
    pub fn validate(&self) -> Result<(), String> {
        let mut next = 0u32;
        for s in &self.stages {
            if s.first != next || s.id.is_empty() || s.id.len() > 64 {
                return Err(format!("stage {} is out of lay order", s.id));
            }
            next = next.checked_add(s.count).ok_or("too many stones")?;
        }
        if self.stages.is_empty() || self.stages.len() > 64 || next > 100_000 {
            return Err("castle plan bounds".into());
        }
        Ok(())
    }
}

/// A group of buildings, roads, shafts and lines that stamps down as one
/// command. Coordinates are offsets from the stamp's origin.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Pattern {
    pub name: String,
    pub description: String,
    #[serde(default)]
    pub roads: Vec<(i16, i16)>,
    #[serde(default)]
    pub shafts: Vec<(i16, i16)>,
    pub builds: Vec<PatternBuild>,
    #[serde(default)]
    pub lines: Vec<PatternLine>,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PatternBuild {
    pub plan: String,
    pub at: (i16, i16),
    #[serde(default)]
    pub recipe: Option<String>,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PatternLine {
    pub kind: LineKind,
    /// Indexes into `builds`.
    pub from: usize,
    pub to: usize,
    #[serde(default)]
    pub vehicles: u16,
}

fn id(s: &str) -> bool {
    !s.is_empty()
        && s.len() <= 48
        && s.bytes()
            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-')
}
impl Catalog {
    pub fn parse(json: &str) -> Result<Self, String> {
        let catalog: Self = serde_json::from_str(json).map_err(|e| e.to_string())?;
        catalog.validate()?;
        Ok(catalog)
    }
    /// A charter of either world.
    pub fn tech(&self, id: &str) -> Option<&Technology> {
        self.technologies.get(id).or_else(|| self.hillside_technologies.get(id))
    }
    /// Every charter, the valley's first.
    pub fn all_techs(&self) -> impl Iterator<Item = (&String, &Technology)> {
        self.technologies.iter().chain(self.hillside_technologies.iter())
    }
    pub fn valid_stock(&self, stock: &Stock) -> bool {
        stock.len() <= self.resources.len()
            && stock
                .iter()
                .all(|(r, &n)| self.resources.contains_key(r) && n > 0 && n <= 1_000_000)
    }
    pub fn validate(&self) -> Result<(), String> {
        if self.schema != 1
            || !id(&self.id)
            || self.resources.is_empty()
            || self.resources.len() > 200
            || self.recipes.len() > 300
            || self.plans.len() > 112
            || self.technologies.len() > 64
            || self.hillside_technologies.len() > 64
            || self.hillside_technologies.keys().any(|k| self.technologies.contains_key(k))
            || self.menu.is_empty()
            || self.menu.len() > 64
            || !self.valid_stock(&self.starter)
            || !self.plans.contains_key("camp")
            || !self.resources.contains_key("timber")
        {
            return Err("invalid kingdom catalogue bounds or foundation".into());
        }
        for (key, resource) in &self.resources {
            if !id(key)
                || resource.name.is_empty()
                || resource.name.len() > 40
                || resource.colour > 0xffffff
            {
                return Err(format!("invalid resource {key}"));
            }
        }
        for (key, recipe) in &self.recipes {
            if !id(key)
                || recipe.name.len() > 40
                || recipe.shifts == 0
                || recipe.shifts > 100
                || !self.valid_stock(&recipe.inputs)
                || !self.valid_stock(&recipe.outputs)
                || recipe.outputs.is_empty()
                || recipe
                    .inputs
                    .values()
                    .chain(recipe.outputs.values())
                    .any(|n| *n > 24)
            {
                return Err(format!("invalid recipe {key}"));
            }
        }
        for (key, plan) in &self.plans {
            if !id(key)
                || plan.name.len() > 40
                || plan.glyph.len() != 1
                || !plan.glyph.is_ascii()
                || plan.colour > 0xffffff
                || !self.valid_stock(&plan.cost)
                || plan.workers > 20
                || plan.housing > 40
                || plan.storage > 10_000
                || plan.recipes.len() > 24
                || plan.buffer > 10_000
                || plan.berths > 12
                || ((plan.hub || plan.yard)
                    && (plan.buffer == 0 || !plan.recipes.is_empty() || plan.extractor || plan.workers > 0))
                || (plan.hub && plan.yard)
                || plan.recipes.iter().any(|r| !self.recipes.contains_key(r))
                || (plan.extractor && !plan.recipes.is_empty())
                || plan.build_seconds > 600
                || plan.carriers > 20
                || plan.power.as_ref().is_some_and(|p| !p.valid(self))
                || plan
                    .power
                    .as_ref()
                    .is_some_and(|p| p.is_source() && (!plan.recipes.is_empty() || plan.extractor))
                || plan.connector.is_some_and(|kind| {
                    plan.power.is_some()
                        || plan.extractor
                        || !plan.recipes.is_empty()
                        || plan.workers > 0
                        || plan.housing > 0
                        || plan.storage > 0
                        || plan.carriers > 0
                        || plan.build_seconds > 0
                        || !plan.passable
                        || self
                            .plans
                            .values()
                            .filter(|p| p.connector == Some(kind))
                            .count()
                            != 1
                })
                || plan
                    .requires
                    .iter()
                    .any(|r| self.tech(r).is_none())
            {
                return Err(format!("invalid building {key}"));
            }
        }
        for (key, spec) in &self.rollers {
            if crate::rollers::Piece::from_key(key).is_none()
                || spec.name.is_empty()
                || spec.name.len() > 40
                || !self.valid_stock(&spec.cost)
                || !(1..=4).contains(&spec.beats)
                || spec.crew > 1
                || spec.requires.iter().any(|r| self.tech(r).is_none())
            {
                return Err(format!("invalid rollerway piece {key}"));
            }
        }
        for (key, plan) in &self.plans {
            if plan.swape > 0 && (plan.workers == 0 && plan.power.is_none() || plan.swape > 60 || !plan.recipes.is_empty() || plan.extractor) {
                return Err(format!("invalid swape {key}"));
            }
        }
        for (key, tech) in self.all_techs() {
            if !id(key)
                || tech.name.len() > 40
                || tech.description.len() > 160
                || !self.valid_stock(&tech.cost)
                || tech
                    .requires
                    .iter()
                    .any(|r| r == key || self.tech(r).is_none())
            {
                return Err(format!("invalid technology {key}"));
            }
        }
        let mut resolved = std::collections::BTreeSet::new();
        loop {
            let before = resolved.len();
            for (key, tech) in self.all_techs() {
                if tech.requires.iter().all(|r| resolved.contains(r)) {
                    resolved.insert(key.clone());
                }
            }
            if resolved.len() == before {
                break;
            }
        }
        if resolved.len() != self.technologies.len() + self.hillside_technologies.len() {
            return Err("cyclic technology prerequisites".into());
        }
        self.validate_works()?;
        let unique: std::collections::BTreeSet<_> = self.menu.iter().collect();
        if unique.len() != self.menu.len()
            || self
                .menu
                .iter()
                .any(|p| p == "camp" || !self.plans.contains_key(p))
        {
            return Err("invalid build menu".into());
        }
        Ok(())
    }
}

impl Catalog {
    fn validate_works(&self) -> Result<(), String> {
        let have = |r: &String| self.resources.contains_key(r);
        for (key, r) in &self.resources {
            if r.shelf > 100_000 || r.weight > 1000 {
                return Err(format!("invalid shelf life or weight for {key}"));
            }
        }
        for (key, r) in &self.recipes {
            if r.level > 3 || r.patch.as_ref().is_some_and(|p| !matches!(p.kind.as_str(), "wood" | "field") || p.takes > 1000 || p.min_percent > 100) {
                return Err(format!("invalid recipe {key}"));
            }
        }
        for (key, t) in &self.trades {
            if crate::population::trade_index(key).is_none() || t.kin.iter().any(|k| !self.trades.contains_key(k) || k == key) {
                return Err(format!("invalid trade {key}"));
            }
        }
        if self.founders.iter().any(|(t, l)| !self.trades.contains_key(t) || !(1..=3).contains(l)) || self.founders.len() > 16 || !(1..=16).contains(&self.works.founding_people) || !(10..=600).contains(&self.works.arrival_seconds) {
            return Err("invalid founders".into());
        }
        for (key, p) in &self.plans {
            let worn_ok = p.tool.is_empty()
                || (have(&p.tool) && self.resources.contains_key(&format!("worn-{}", p.tool)) && p.tool_life > 0);
            if (!p.trade.is_empty() && !self.trades.contains_key(&p.trade))
                || p.level > 3
                || !worn_ok
                || !self.valid_stock(&p.upkeep)
                || (!p.upkeep.is_empty() && p.upkeep_seconds == 0)
                || p.span > 8
                || p.depth > 6
                || p.shaft > 1000
                || p.stalls > 64
                || (p.stalls > 0 && (p.hub || p.yard || p.workers == 0))
                || p.replaces.as_ref().is_some_and(|r| !self.plans.contains_key(r))
                || (!p.valley && !p.hillside)
            {
                return Err(format!("invalid building {key}"));
            }
        }
        if self.menu.iter().any(|p| !self.plans[p].valley) {
            return Err("the valley menu lists a hillside building".into());
        }
        let hm: std::collections::BTreeSet<_> = self.hillside_menu.iter().collect();
        if hm.len() != self.hillside_menu.len()
            || self.hillside_menu.iter().any(|p| p == "camp" || !self.plans.get(p).is_some_and(|x| x.hillside))
        {
            return Err("invalid hillside menu".into());
        }
        let diet = &self.diet;
        if (!diet.staple.is_empty() && !have(&diet.staple))
            || [&diet.labourer, &diet.skilled, &diet.master]
                .iter()
                .flat_map(|t| t.iter())
                .any(|e| e.split('|').any(|a| !self.resources.contains_key(a)))
            || (!diet.labourer.is_empty() && diet.labourer[0] != diet.staple)
            || (!diet.labourer.is_empty() && (diet.skilled.is_empty() || diet.master.is_empty()))
        {
            return Err("invalid diet".into());
        }
        for (key, g) in &self.geology {
            if !have(&g.stone) || g.also.iter().any(|r| !have(r)) || key.is_empty() {
                return Err(format!("invalid geology {key}"));
            }
        }
        if self.reserved.iter().any(|r| !have(r)) {
            return Err("invalid reserved goods".into());
        }
        if self.camp_stock.iter().any(|(r, n)| !have(r) || *n > 10_000) {
            return Err("invalid camp stock levels".into());
        }
        if !(1..=8).contains(&self.works.road_lanes) || self.works.jam_percent > 400 || self.works.welcome_meals > 6 {
            return Err("invalid road lanes".into());
        }
        for (line, rule) in &self.bill {
            if rule.overrides.iter().any(|o| !have(&o.resource) || o.per.0 == 0) {
                return Err(format!("invalid bill rule {line}"));
            }
        }
        for (key, spec) in &self.lines {
            if !id(key)
                || !LineKind::ALL.iter().any(|k| k.key() == key)
                || spec.capacity == 0
                || spec.speed == 0
                || spec.max_vehicles == 0
                || spec.max_length == 0
                || spec.dwell > 60
                || !self.valid_stock(&spec.tile_cost)
                || !self.valid_stock(&spec.vehicle_cost)
                || spec.requires.iter().any(|r| self.tech(r).is_none())
            {
                return Err(format!("invalid line kind {key}"));
            }
        }
        for (line, rule) in &self.bill {
            if line.is_empty()
                || !self.resources.contains_key(&rule.resource)
                || rule.per.0 == 0
                || rule.other.as_ref().is_some_and(|o| !self.resources.contains_key(o) || rule.one_in < 2)
            {
                return Err(format!("invalid bill rule {line}"));
            }
        }
        for f in &self.fixtures {
            if f.stage.is_empty() || !self.valid_stock(&f.needs) || f.needs.is_empty() {
                return Err("invalid stage fixture".into());
            }
        }
        for (key, castle) in &self.castles {
            castle.validate()?;
            if !id(key)
                || castle.footprint.0 < 3
                || castle.footprint.1 < 3
                || castle.footprint.0 > 80
                || castle.footprint.1 > 80
            {
                return Err(format!("invalid castle plan {key}"));
            }
            for stage in &castle.stages {
                if stage.bill.keys().any(|k| !self.bill.contains_key(k)) {
                    return Err(format!("stage {} bills a line with no chain", stage.id));
                }
            }
        }
        if !self.valid_stock(&self.hillside_starter) {
            return Err("invalid hillside starter".into());
        }
        for (key, p) in &self.patterns {
            if !id(key)
                || p.builds.is_empty()
                || p.builds.len() > 24
                || p.roads.len() > 128
                || p.shafts.len() > 128
                || p.builds.iter().any(|b| {
                    !self.plans.contains_key(&b.plan)
                        || b.recipe.as_ref().is_some_and(|r| {
                            !self.plans[&b.plan].recipes.contains(r)
                        })
                })
                || p.lines.iter().any(|l| {
                    l.from >= p.builds.len()
                        || l.to >= p.builds.len()
                        || l.from == l.to
                        || !self.lines.contains_key(l.kind.key())
                })
            {
                return Err(format!("invalid pattern {key}"));
            }
        }
        Ok(())
    }
}

pub fn catalog() -> &'static Catalog {
    static CATALOG: OnceLock<Catalog> = OnceLock::new();
    CATALOG.get_or_init(|| {
        let text = include_str!("../../../delve/kingdom/frontier.json");
        // A balance run may try numbers without regenerating the file: `CATALOG_PATCH` is a JSON
        // object merged over the catalogue (tests only; the game reads the file as it is).
        #[cfg(test)]
        if let Ok(patch) = std::env::var("CATALOG_PATCH") {
            fn merge(into: &mut serde_json::Value, from: serde_json::Value) {
                match (into, from) {
                    (serde_json::Value::Object(a), serde_json::Value::Object(b)) => {
                        for (k, v) in b {
                            merge(a.entry(k).or_insert(serde_json::Value::Null), v);
                        }
                    }
                    (a, b) => *a = b,
                }
            }
            let mut whole: serde_json::Value = serde_json::from_str(text).expect("the catalogue is json");
            merge(&mut whole, serde_json::from_str(&patch).expect("CATALOG_PATCH is json"));
            return Catalog::parse(&whole.to_string()).expect("the patched catalogue checks");
        }
        Catalog::parse(text).expect("checked frontier catalogue")
    })
}
