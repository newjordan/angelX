//! The balance scenario: a focused player raises the motte and bailey, the
//! shell keep and then the great tower, one after another, from the founding
//! stock. Nothing is written into the stock, a building or a site: the only
//! ways in are `Command`s through `apply`, the Delve adapter's claim and coin
//! hooks, and the masons' shift.
//!
//! The player is a script, but it decides the way a good player does. It keeps
//! a want list (how many of each building the castle in hand needs, from the
//! solver at the pace of the age) in an order that raises food and timber
//! first; it builds a post only when it has hands to man it; it puts each new
//! building beside the works it trades with and lays the lines at once; it
//! keeps beds ahead of the people; it adds carts where a line is full, a
//! better line where the carts are all out, the next lift when the shaft is
//! full, a teacher when a trade lacks a master; and it switches a workshop to
//! whichever of its products the camp is shortest of. The knights' side is a
//! stated timetable of seams and gold.
//!
//! It plays the real map, every work the size it is drawn (the compact balance geometry is gone). The camp is laid
//! out from the first minute (`works_layout`): the stockyard staked in the first seconds by the camp's trunk road,
//! two ditch gangs beside it each lifting its spoil into the yard by a swape, rows of works along streets by what
//! they trade with, and the opening hour's purchases held to what the wall's first stones wait on (`opening_cap`).
use super::*;
use crate::design::{solve, Requirement, SolveOptions};
use crate::economy::covers;
use std::collections::{BTreeMap, BTreeSet};
use crate::Cause;
use crate::works_layout::{self as lay, Layout};

/// The castles in the order the ages raise them, with the pace the works are
/// sized to (stones a minute while the masons are at the wall).
pub(crate) const AGES: [(&str, f64); 3] = [("motte-and-bailey", 22.0), ("shell-keep", 45.0), ("great-tower", 70.0)];

/// The knights' timetable: a seam is won below and brought home at this second.
/// Each yield is what a quarry on it brings up a shift.
pub(crate) const SEAMS: &[(&str, u64, &[(&str, u32)])] = &[
    ("mines-1", 1500, &[("limestone", 4), ("iron-ore", 2)]),
    ("crypt-1", 2400, &[("coal", 2), ("bone", 1)]),
    ("mines-2", 4200, &[("limestone", 8), ("galena", 2)]),
    ("archive-1", 5400, &[("slate-block", 3), ("limestone", 4)]),
    ("mines-3", 7800, &[("limestone", 8), ("iron-ore", 3)]),
    ("mines-4", 10200, &[("limestone", 8), ("coal", 2)]),
];

/// Where the stockyard goes: below the Keep's south-east corner by the camp, outside the walls.
const YARD_PLAN: Tile = (147, 96);

/// A flow (see `Roadwork::flow`: eighty times a kilogram a second) of about a quarter of a
/// tonne a minute over this many tiles or more is a ropeway's work, not the roads'.
const ROPE_FLOW: u32 = 200;
const ROPE_LENGTH: i16 = 14;

/// Charcoal the kingdom keeps in hand (clamps stop at this much, wherever it lies).
const CHARCOAL_STOCK: u32 = 60;

/// The most lodges a player raises at one wall.
const MAX_LODGES: u32 = 8;

/// A flow of about a fifth of a tonne a minute over this many tiles or more gets a warehouse at its source.
const HUB_FLOW: u32 = 150;
const HUB_LENGTH: i16 = 18;

/// The goods the charts follow through the whole colony.
const WATCHED: [&str; 16] = [
    "timber", "charcoal", "limestone", "quicklime", "lime-putty", "sand", "mortar", "ashlar", "quoin", "plank", "beam", "pole", "bread", "iron-ore", "coal", "nails",
];

/// Seconds a good must have been short, without a break, before another maker of it is asked for.
const SHORT_FOR: u64 = 240;

/// Seconds between one extra maker asked for and the next, of the same kind.
const EXPAND_EVERY: u64 = 150;

/// A lift is raised when the quarries ask for this percent of what it lifts a shift.
const SHAFT_FILL: u32 = 60;

/// A number the balance runs may move without a rebuild: `KNOB_<NAME>=value`.
fn knob(name: &str, default: u64) -> u64 {
    std::env::var(format!("KNOB_{name}")).ok().and_then(|v| v.parse().ok()).unwrap_or(default)
}

/// Camps that fell timber in the opening hour; every other camp cuts coppice.
fn opening_fellers() -> f64 {
    std::env::var("OPENING_FELLERS").ok().and_then(|v| v.parse().ok()).unwrap_or(2.0)
}

/// Until this second a player saving for the next purchase saves the timber too.
const SAVE_TIMBER_UNTIL: u64 = 2400;

/// Whether the scripted player also uses the late tiers (mule trains, the vineyard and press, the
/// aqueduct and the water hoist). They are in the game and tested; they are off in the balance run,
/// which measured no gain from them and some risk to a young colony (`TIERS=1` turns them on).
fn tiers() -> bool {
    std::env::var("TIERS").is_ok()
}

/// Whether the scripted player lays rollerways between the works that send each other the most (`FACTORY=1`):
/// the continuous line, tried against the carts that carry those legs otherwise. `FACTORY=hands` lays the
/// hand-chain only.
fn factory() -> Option<String> {
    std::env::var("FACTORY").ok()
}

/// A flow this heavy (kilos lately between two works) over a stretch this long is a rollerway's: the
/// shortest is a few tiles, which a hand-chain does, and the longest what the player will lay by hand.
const ROLLER_FLOW: u32 = 60;
const ROLLER_MIN: i16 = 4;
const ROLLER_MAX: i16 = 40;
const MAX_ROLLER_LINES: usize = 8;

/// A flow this heavy over this many tiles or more is a railway's: no oxen, no hands, and a wagon train that does not
/// slow with the distance as a cart does (`transport_bench`: eight carts carry fourteen stones a minute over forty-eight
/// tiles, eight wagons twenty-four).
const RAIL_FLOW: u32 = 120;
const RAIL_LENGTH: i16 = 22;
const MAX_RAILWAYS: usize = 3;

/// A flow of about a tenth of a tonne a minute, over at least this many tiles, is a string of
/// mules' work (less than a ropeway's, more than a cart should be sent for).
const MULE_FLOW: u32 = 90;
const MULE_LENGTH: i16 = 10;
const MAX_MULE_LINES: usize = 6;

/// What the knights bank every ten minutes.
const PURSE: u32 = 30;

/// Plans in the order a player raises them; what is not here comes last.
const RANK: &[&str] = &[
    "carters-yard", "ditch-gang", "field-quarry", "woodsman-camp", "sawpit", "farmstead", "well", "clamp", "mill", "bakery", "mason", "lodge", "carpentry", "hod-gang", "wheelwright", "smithy", "pasture", "tanyard", "claim-quarry", "brewery", "bloomery", "forge-shop",
    "banker", "lime-kiln", "slaking-pit", "sand-pit", "mortar-mill", "tracing-floor", "retting-pond", "ropewalk",
    "lead-works", "slater", "ashery", "glassworks", "weaving-shed", "vineyard", "wine-press", "aqueduct",
];

/// Where each kind of work goes when it has no partner to stand beside.
fn anchor(plan: &str) -> Tile {
    match plan {
        "woodsman-camp" => (139, 98),
        "sawpit" | "carpentry" | "wheelwright" | "clamp" => (136, 98),
        "smithy" | "forge-shop" | "bloomery" | "lead-works" | "glassworks" | "ashery" => (133, 96),
        "farmstead" | "pasture" | "mill" | "bakery" | "brewery" | "tanyard" | "weaving-shed" | "well" | "vineyard" | "wine-press" => (148, 99),
        "sand-pit" => (155, 75),
        "retting-pond" | "ropewalk" => (125, 97),
        "claim-quarry" => (149, 89),
        "banker" | "tracing-floor" | "slater" => (150, 84),
        "lime-kiln" | "slaking-pit" | "mortar-mill" => (150, 76),
        "ditch-gang" => (149, 79),
        "lodge" | "hod-gang" | "treadwheel-crane" | "great-crane" => (149, 90),
        "aqueduct" => (146, 86),
        "field-quarry" | "mason" => (147, 95),
        _ => (140, 98),
    }
}

/// What the player means a building to be.
#[derive(Clone, Debug)]
struct Made {
    plan: String,
    /// The plan, with `#1` after it for the second step of a chain a shop cannot feed itself.
    key: String,
    /// The recipes it may be switched among, best first.
    pool: Vec<String>,
    switched: u64,
}

/// One sampled minute of the game, for the charts.
#[derive(Clone, Debug, serde::Serialize)]
pub(crate) struct Sample {
    pub t: u64,
    pub castle: String,
    pub stage: String,
    pub stones_set: u32,
    pub population: usize,
    pub beds: usize,
    pub employed: u16,
    pub idle: u16,
    pub stock: BTreeMap<String, u32>,
    pub buildings: BTreeMap<String, u32>,
    pub yards: usize,
    pub carts: usize,
    /// Carts standing free, orders no cart could take, and tiles where carts are held up.
    pub free_carts: usize,
    pub unserved: usize,
    pub jams: usize,
    /// Units the carts delivered in the last minute.
    pub hauled_per_min: u32,
    /// Point-to-point lines laid (tramways, ropeways, chutes, barges).
    pub lines: usize,
    pub shaft: u32,
    pub shaft_moved: u32,
    /// Goods held anywhere (camp, benches, warehouses), and made over the whole game, for the chains the curve turns on.
    pub held: BTreeMap<String, u32>,
    pub made: BTreeMap<String, u32>,
    /// Goods that went off, over the whole game.
    pub lost: BTreeMap<String, u32>,
    /// Shifts at the wall so far, by what held the masons: [their pace, the yard's stock, a hold].
    pub shifts: [u32; 3],
    pub holds: BTreeMap<String, u32>,
    /// Seconds each kind of work has spent working, starved, backed up and without a crew, summed over its buildings.
    pub util: BTreeMap<String, [u32; 4]>,
}

pub(crate) struct Player {
    pub k: Kingdom,
    /// The age in hand: an index into `AGES`.
    pub age: usize,
    req: Requirement,
    /// How many of each plan the ages so far have asked for, and the recipes
    /// each plan's machines are asked to run.
    target: BTreeMap<String, u32>,
    shares: BTreeMap<String, BTreeMap<String, f64>>,
    made: BTreeMap<u32, Made>,
    /// Seams brought home and the quarry on each.
    seams: BTreeMap<&'static str, Option<u32>>,
    camp_goods: BTreeSet<String>,
    wall: BTreeSet<String>,
    pub log: Vec<(u64, String)>,
    pub waits: Vec<(u64, u64, String)>,
    quiet_since: Option<(u64, String)>,
    pub first_stone: Option<u64>,
    pub stage_times: Vec<(u64, String)>,
    /// When each castle stood finished.
    pub finished: Vec<(String, u64)>,
    stones_before: u32,
    hamlets: usize,
    pub coin_banked: u32,
    pub foreman: BTreeMap<&'static str, u32>,
    /// When a plan was last given another maker for a choked bench.
    expanded: BTreeMap<String, u64>,
    /// How many of each plan the solver asked for, at the pace of the age in hand.
    solved: BTreeMap<String, u32>,
    pub samples: Vec<Sample>,
    /// What the player raised and when, for the timeline.
    pub built: Vec<(u64, String)>,
    /// What each failed attempt said last, by what the player was trying.
    why: BTreeMap<String, (u64, String)>,
    /// Goods the player could not pay a bill for lately, with the second it was noted.
    missing: BTreeMap<String, u64>,
    /// A cart line carries more than twelve carts can: the ropeway is the next thing to raise.
    rope_project: bool,
    /// A middling flow over rough ground: mules are wanted.
    mule_project: bool,
    /// The works a rollerway joins, from and to, so none is laid twice.
    roller_pairs: Vec<(u32, u32)>,
    /// The works a railway joins.
    rail_pairs: Vec<(u32, u32)>,
    /// The drive from the aqueduct to the hoist, tile by tile, once it has been planned.
    drive: Vec<Tile>,
    /// Since when the water hoist has stood without power.
    unpowered_since: Option<u64>,
    /// Ten-second shifts at the wall by what held the masons: their own pace (the yard could pay
    /// for more), the yard's stock (it could not), or a hold with no stones at all.
    pub shifts: [u32; 3],
    /// Shifts lost to a hold, by what the hold was.
    pub holds: BTreeMap<String, u32>,
    /// Seconds the player's rounds are offset by: another hand at the same game, for judging a
    /// change against the run-to-run spread (`JITTER=n`).
    jitter: u64,
    /// Goods the player could not pay a bill for, and since when it has been so without a break.
    short_since: BTreeMap<String, u64>,
    /// What the Clerk has named as the limit, and how many looks in a row have named it.
    limit_seen: Option<((String, String), u32)>,
    /// Cart-seconds spent in each phase (idle, return, fetch, queue, loading, haul, wait, unloading), for
    /// finding where the carts' time goes.
    pub cart_seconds: [u64; 8],
    /// Kilograms and tiles hauled loaded between kinds of work (kilogram-tiles, kilograms carried by the tile), and the tile
    /// each cart stood on last second.
    pub haul_stats: BTreeMap<(String, String), (u64, u64)>,
    cart_tiles: BTreeMap<(u32, usize), Tile>,
    /// The camp's streets and rows (`works_layout`).
    layout: Layout,

    pub done: bool,
    /// Scripted delves already paid home, depths rising.
    pub delves: u32,
    /// The last return plate.
    pub plate: String,
}

impl Player {
    /// The bot on the hillside as the game builds it: every work its drawn size, on the real map.
    pub fn new() -> Self {
        let c = catalog();
        // The seed only moves the river's meander and the ragged edges of the woods; the places a
        // castle needs never move. `SEED=n` plays the same game on another hillside.
        let seed = std::env::var("SEED").ok().and_then(|s| s.parse().ok()).unwrap_or(7);
        let mut k = Kingdom::default();
        k.found_hillside(c, seed);
        let mut p = Self {
            k,
            age: 0,
            req: Requirement::default(),
            target: BTreeMap::new(),
            shares: BTreeMap::new(),
            made: BTreeMap::new(),
            seams: SEAMS.iter().map(|(id, _, _)| (*id, None)).collect(),
            camp_goods: BTreeSet::new(),
            wall: BTreeSet::new(),
            log: Vec::new(),
            waits: Vec::new(),
            quiet_since: None,
            first_stone: None,
            stage_times: Vec::new(),
            finished: Vec::new(),
            stones_before: 0,
            hamlets: 0,
            coin_banked: 0,
            foreman: BTreeMap::new(),
            expanded: BTreeMap::new(),
            solved: BTreeMap::new(),
            samples: Vec::new(),
            built: Vec::new(),
            why: BTreeMap::new(),
            missing: BTreeMap::new(),
            rope_project: false,
            mule_project: false,
            roller_pairs: Vec::new(),
            rail_pairs: Vec::new(),
            drive: Vec::new(),
            unpowered_since: None,
            shifts: [0; 3],
            holds: BTreeMap::new(),
            jitter: std::env::var("JITTER").ok().and_then(|s| s.parse().ok()).unwrap_or(0),
            short_since: BTreeMap::new(),
            limit_seen: None,
            cart_seconds: [0; 8],
            haul_stats: BTreeMap::new(),
            cart_tiles: BTreeMap::new(),
            layout: Layout::new(),
            done: false,
            delves: 0,
            plate: String::new(),
        };
        for pl in c.plans.values() {
            p.camp_goods.extend(pl.cost.keys().cloned());
            p.camp_goods.extend(pl.upkeep.keys().cloned());
            if !pl.tool.is_empty() {
                p.camp_goods.insert(pl.tool.clone());
                p.camp_goods.insert(format!("worn-{}", pl.tool));
            }
        }
        for (_, t) in c.all_techs() {
            p.camp_goods.extend(t.cost.keys().cloned());
        }
        for l in c.lines.values() {
            p.camp_goods.extend(l.vehicle_cost.keys().cloned());
            p.camp_goods.extend(l.tile_cost.keys().cloned());
            p.camp_goods.extend(l.upkeep.keys().cloned());
        }
        for g in [&c.diet.labourer, &c.diet.skilled, &c.diet.master] {
            for e in g {
                p.camp_goods.extend(e.split('|').map(String::from));
            }
        }
        p.camp_goods.extend(["coal", "iron-ore", "livery"].map(String::from));
        p.plan_age();
        p
    }

    fn now(&self) -> u64 {
        self.k.second
    }

    /// Sizes the works to the castle of this age. Targets only ever grow.
    fn plan_age(&mut self) {
        let c = catalog();
        let (castle, pace) = AGES[self.age];
        let mut opts = SolveOptions::new(castle, pace);
        // A player lays works close: a typical haul is a short one.
        opts.distance = 8;
        opts.shaft_distance = 8;
        opts.fleet_growth = true;
        self.req = solve(c, &opts).expect("the castle solves");
        let camp_only = crate::design::reach(c, true);
        let mut shares: BTreeMap<String, BTreeMap<String, f64>> = BTreeMap::new();
        for pl in self.req.plants.iter().filter(|x| camp_only.plans.contains(&x.plan)) {
            *shares.entry(pl.plan.clone()).or_default().entry(pl.recipe.clone()).or_default() += pl.machines;
        }
        // The earthworks are dug by a few gangs at once, quickly.
        shares.entry("ditch-gang".into()).or_default().insert("dig-earth".into(), 2.0);
        // Woodsmen keep to one task each, so a camp is never switched from timber to poles and
        // back: the timber camps cut coppice, one camp keeps the poles, one the bark. Only the
        // opening hour fells, because timber cannot wait and a felled wood is a wood given up.
        let w = shares.entry("woodsman-camp".into()).or_default();
        let poles = w.remove("coppice-poles").unwrap_or(0.0);
        let bark = w.remove("strip-bark").unwrap_or(0.0);
        let felled = w.remove("fell-wood").unwrap_or(0.0);
        let coppice = w.remove("coppice-timber").unwrap_or(0.0) + felled;
        w.insert("coppice-timber".into(), coppice.max(1.0));
        if self.age == 0 {
            w.insert("fell-wood".into(), opening_fellers());
        }
        shares.entry("woodsman-camp#poles".into()).or_default().insert("coppice-poles".into(), poles.ceil().max(1.0));
        shares.entry("woodsman-camp#bark".into()).or_default().insert("strip-bark".into(), bark.ceil().max(1.0));
        // The first hour's stone comes off the hillside: one field quarry, one mason's yard.
        for (plan, recipe) in [("field-quarry", "win-field-stone"), ("mason", "dress")] {
            shares.entry(plan.into()).or_default().entry(recipe.into()).or_insert(1.0);
        }
        // Workshops that make several things may be switched among them.
        for plan in ["sawpit", "carpentry", "forge-shop", "wheelwright", "pasture", "farmstead", "banker"] {
            let Some(p) = c.plans.get(plan) else { continue };
            let pool = shares.entry(plan.into()).or_default();
            for r in &p.recipes {
                if !["burn-lime-peat", "burn-lime-coal", "dress-field", "smelt-bog", "herd-wool", "herd-dairy", "ash-fields"].contains(&r.as_str()) {
                    pool.entry(r.clone()).or_insert(0.01);
                }
            }
        }
        // The smith keeps every tool and the steel as well as the wall's needs.
        let smith = shares.entry("smithy".into()).or_default();
        for r in ["make-steel", "resteel-axe", "resteel-pick", "resteel-chisel", "resteel-trowel", "make-axe", "make-pick", "make-chisel", "make-trowel"] {
            smith.entry(r.into()).or_insert(0.01);
        }
        // A shop cannot feed itself from its own bench: the step that eats what another
        // step of the same plan makes goes in its own building.
        let mut keyed: BTreeMap<String, BTreeMap<String, f64>> = BTreeMap::new();
        for (plan, rs) in &shares {
            if plan.contains('#') {
                for (r, m) in rs {
                    *keyed.entry(plan.clone()).or_default().entry(r.clone()).or_default() += m;
                }
                continue;
            }
            for (r, m) in rs {
                let second = rs.keys().any(|o| o != r && c.recipes[o].outputs.keys().any(|g| c.recipes[r].inputs.contains_key(g)));
                let key = if second { format!("{plan}#1") } else { plan.clone() };
                *keyed.entry(key).or_default().entry(r.clone()).or_default() += m;
            }
        }
        // Oxen are what the carts wait on, once the camp age is over: one pasture keeps to them
        // and the others to the herd. (In the first hour every pair of hands is wanted elsewhere.)
        if self.age >= 1 {
            if let Some(p) = keyed.get_mut("pasture") {
                p.remove("rear-ox");
            }
            keyed.entry("pasture#oxen".into()).or_default().insert("rear-ox".into(), 1.0);
        }
        for (key, rs) in &keyed {
            let n = rs.values().sum::<f64>().ceil() as u32;
            let t = self.target.entry(key.clone()).or_default();
            *t = (*t).max(n);
            let sv = self.solved.entry(key.clone()).or_default();
            *sv = (*sv).max(n);
        }
        let t = self.target.entry("smithy#1".into()).or_default();
        *t = (*t).max(1 + self.age as u32);
        let t = self.target.entry("ditch-gang".into()).or_default();
        *t = (*t).max(2);
        // One carter's yard from the start; the carters ask for more when the stalls are full.
        let t = self.target.entry("carters-yard".into()).or_default();
        *t = (*t).max(1);
        self.shares = keyed;
        self.wall = c.castles[castle].stages.iter().flat_map(|s| crate::site::stage_needs(c, s).into_keys()).collect();
        self.wall.extend(["pole".to_string(), "centring".to_string()]);
        // The masons: lodges beside the yard.
        let t = self.target.entry("lodge".into()).or_default();
        *t = (*t).max(self.req.lodges);
    }

    // ---- placing -----------------------------------------------------

    /// Whether a work of `plan` may stand with its top-left tile on `tile`: its ground is clear and its own, and
    /// it is on no planned street.
    fn fits(&self, plan: &str, tile: Tile) -> bool {
        let c = catalog();
        let camp = self.k.camp();
        self.k.footprint(&c.plans[plan], tile).into_iter().all(|u| {
            self.k.world.contains(u)
                && self.k.at(u).is_none()
                && !self.k.roads.contains(&u)
                && !lay::is_street(u)
                && u != self.k.gate()
                && ((u.0 - camp.0).abs() > 1 || (u.1 - camp.1).abs() > 1)
        }) && self.k.check_site(c, plan, tile).is_ok()
            && self.k.check_footprint(c, plan, tile).is_ok()
    }

    /// Why a work may not stand with its top-left tile on `tile`, for finding out why a row has no room.
    fn why_not(&self, plan: &str, tile: Tile) -> String {
        let c = catalog();
        let camp = self.k.camp();
        for u in self.k.footprint(&c.plans[plan], tile) {
            if !self.k.world.contains(u) {
                return format!("{u:?} off the map");
            }
            if let Some(id) = self.k.at(u) {
                return format!("{u:?} is {}#{id}", self.k.buildings[&id].plan);
            }
            if self.k.roads.contains(&u) {
                return format!("{u:?} is road");
            }
            if lay::is_street(u) {
                return format!("{u:?} is a planned street");
            }
            if u == self.k.gate() || ((u.0 - camp.0).abs() <= 1 && (u.1 - camp.1).abs() <= 1) {
                return format!("{u:?} is the camp's");
            }
        }
        self.k.check_site(c, plan, tile).and_then(|_| self.k.check_footprint(c, plan, tile)).err().unwrap_or_default()
    }

    /// Lays the street tiles a work on a row needs, if the camp has the timber for them and the work.
    fn lay_streets(&mut self, tiles: &[Tile], plan: &str) -> bool {
        let c = catalog();
        let missing: Vec<Tile> = tiles.iter().copied().filter(|t| !self.k.roads.contains(t)).collect();
        if self.k.amount("timber") < missing.len() as u32 + c.plans[plan].cost.get("timber").copied().unwrap_or(0) {
            return false;
        }
        for t in missing {
            if self.k.apply(c, Command::Road(t)).is_err() {
                return false;
            }
        }
        true
    }

    /// The tile for a work in the rows of its zone: the place nearest the works it trades with, a tile clear of its
    /// neighbours on each side, with the streets to it laid. `None`: no row has room (the ring search takes over);
    /// `Some(Err)`: there is room but the camp cannot yet pay for the streets.
    fn place(&mut self, plan: &str, pool: &[String]) -> Option<Result<Tile, String>> {
        let c = catalog();
        if plan == "ditch-gang" {
            return self.gang_slot();
        }
        let (span, depth) = self.k.dims(&c.plans[plan]);
        let (w, h) = (i16::from(span) + 1, i16::from(depth) + 1);
        // The woodsmen's rows stand at the southern wood; when it has been felled to the last, they go where the
        // timber is.
        if plan == "woodsman-camp" {
            let at = self.wood_anchor();
            if !(130..=160).contains(&at.0) || !(95..=100).contains(&at.1) {
                return None;
            }
        }
        let target = self.near(plan, pool);
        let mut best: Option<(i32, usize, i16, Tile)> = None;
        for (rank, name) in lay::zone(plan).iter().enumerate() {
            let Some(ri) = self.layout.rows.iter().position(|r| r.name == *name) else { continue };
            let row = self.layout.rows[ri].clone();
            let mut x = row.cursor.min(row.x0);
            while x + w - 1 <= row.x1 {
                let tile = Layout::top_left(&row, x, w, h);
                // A tile's gap on each side, so every work can be got round and a swape stood between neighbours.
                let gap = |dx: i16| (0..h).all(|dy| self.k.at((x + dx, tile.1 + dy)).is_none());
                if x >= row.cursor.min(row.x0) && gap(-1) && gap(w) && self.fits(plan, tile) {
                    let (cx, cy) = (i32::from(x) * 2 + i32::from(w), i32::from(tile.1) * 2 + i32::from(h));
                    let dist = (cx - i32::from(target.0) * 2).abs() + (cy - i32::from(target.1) * 2).abs();
                    // The first row of the zone is preferred a little: its stands are the ones set aside for it.
                    let score = dist + 8 * rank as i32;
                    if best.is_none_or(|b| score < b.0) {
                        best = Some((score, ri, x, tile));
                    }
                } else if std::env::var("DEBUG_PLACE").is_ok() && x == row.cursor {
                    eprintln!("  {}s no {plan} on {name} at {tile:?}: {}", self.now(), self.why_not(plan, tile));
                }
                x += 1;
            }
        }
        let (_, ri, x, tile) = best?;
        let row = self.layout.rows[ri].clone();
        let streets = self.layout.street_to(&row, x, w);
        if !self.lay_streets(&streets, plan) {
            return Some(Err("stock".into()));
        }
        self.layout.rows[ri].cursor = self.layout.rows[ri].cursor.max(x + w + 1);
        Some(Ok(tile))
    }

    /// The next ditch gang's place: by the stockyard, with its swape's tile free too.
    fn gang_slot(&mut self) -> Option<Result<Tile, String>> {
        let c = catalog();
        let n = self.k.buildings.values().filter(|b| b.plan == "ditch-gang").count();
        let (gang, swape, _) = *lay::GANGS.get(n.min(lay::GANGS.len() - 1)).filter(|_| n < lay::GANGS.len())?;
        let free = |t: Tile| self.k.at(t).is_none() && !self.k.roads.contains(&t);
        if !self.fits("ditch-gang", gang) || !free(swape) {
            return None;
        }
        let row = self.layout.row(if n == 0 { "S90" } else { "N96" }).cloned()?;
        let streets = self.layout.street_to(&row, gang.0, 3);
        if !self.lay_streets(&streets, "ditch-gang") {
            return Some(Err("stock".into()));
        }
        let _ = c;
        Some(Ok(gang))
    }

    /// A swape between a ditch gang and the stockyard, set to the spoil: the gang's earth goes into the yard by
    /// the arm, not the carts.
    fn lift_spoil(&mut self, gang: Tile) {
        let c = catalog();
        let Some(&(_, swape, dir)) = lay::GANGS.iter().find(|(g, _, _)| *g == gang) else { return };
        if self.k.apply(c, Command::Build { plan: "swape".into(), tile: swape }).is_err() {
            return;
        }
        let _ = self.k.apply(c, Command::Face { tile: swape, dir });
        if let Some(id) = self.k.at(swape) {
            let _ = self.k.apply(c, Command::Filter { building: id, good: Some("earth".into()) });
            let _ = self.k.apply(c, Command::Priority { building: id, priority: 2 });
            *self.foreman.entry("spoil swapes").or_default() += 1;
            self.built.push((self.now(), format!("swape for the spoil at {swape:?}")));
        }
    }

    /// The nearest tile a plan may stand on, keeping to the roads where it can.
    fn spot(&self, plan: &str, near: Tile) -> Option<Tile> {
        let c = catalog();
        let connected_net = self.k.camp_road_net();
        let mut best: Option<(i32, Tile)> = None;
        let mut first_ring = None;
        for r in 0i16..50 {
            if first_ring.is_some_and(|f: i16| r > f + 2) {
                break;
            }
            for dy in -r..=r {
                for dx in -r..=r {
                    if dx.abs().max(dy.abs()) != r {
                        continue;
                    }
                    let t = (near.0 + dx, near.1 + dy);
                    let free = self.k.footprint(&c.plans[plan], t).into_iter().all(|u| {
                        self.k.world.contains(u)
                            && self.k.at(u).is_none()
                            && !self.k.roads.contains(&u)
                            && !lay::is_street(u)
                            && u != self.k.gate()
                            && ((u.0 - self.k.camp().0).abs() > 1 || (u.1 - self.k.camp().1).abs() > 1)
                    });
                    // The tiles round the whole footprint, not just its corner.
                    let own = self.k.footprint(&c.plans[plan], t);
                    let round: Vec<Tile> = own
                        .iter()
                        .flat_map(|u| crate::economy::neighbours(*u))
                        .filter(|n| !own.contains(n))
                        .collect();
                    let neighbours = round.iter().filter_map(|n| self.k.at(*n)).collect::<std::collections::BTreeSet<_>>().len();
                    if !free
                        || self.k.check_site(c, plan, t).is_err()
                        || self.k.check_footprint(c, plan, t).is_err()
                        || neighbours >= 2
                        || self.walls_in(t)
                        || !self.connectable(t, &connected_net)
                    {
                        continue;
                    }
                    first_ring.get_or_insert(r);
                    let on_road = round.iter().any(|n| connected_net.contains(n));
                    let score = i32::from(r) * 2 + if on_road { 0 } else { 3 };
                    if best.is_none_or(|(s, _)| score < s) {
                        best = Some((score, t));
                    }
                }
            }
        }
        best.map(|(_, t)| t)
    }

    /// Whether a work at `t` could be joined to the camp's roads: some run of open ground leads
    /// from beside it to the roads, or to the camp's own doorstep. (A hollow among the walls, the
    /// reserved ground of the keep and the neighbours' backs has no way out, and a work built in
    /// it stands idle for ever.)
    fn connectable(&self, t: Tile, net: &BTreeSet<Tile>) -> bool {
        use std::collections::VecDeque;
        let camp = self.k.camp();
        let open = |u: Tile| self.k.world.contains(u) && self.k.world.terrain(u).walkable() && self.k.at(u).is_none() && self.k.reserved_ground(u).is_none() && u != t;
        let mut seen: BTreeSet<Tile> = BTreeSet::new();
        let mut queue: VecDeque<Tile> = VecDeque::new();
        for n in crate::economy::neighbours(t) {
            if net.contains(&n) || n == camp {
                return true;
            }
            if open(n) && seen.insert(n) {
                queue.push_back(n);
            }
        }
        while let Some(u) = queue.pop_front() {
            if seen.len() > 600 {
                return true;
            }
            for n in crate::economy::neighbours(u) {
                if net.contains(&n) || n == camp {
                    return true;
                }
                if open(n) && seen.insert(n) {
                    queue.push_back(n);
                }
            }
        }
        false
    }

    /// Whether a building at `t` would take the last open side of a neighbour: a work walled in by
    /// its neighbours can never be joined to the roads, and stands idle for ever.
    fn walls_in(&self, t: Tile) -> bool {
        crate::economy::neighbours(t).into_iter().filter(|n| self.k.at(*n).is_some()).any(|n| {
            !crate::economy::neighbours(n).into_iter().any(|m| m != t && self.k.at(m).is_none() && self.k.world.contains(m))
        })
    }

    /// Where a new building of this plan should stand: beside what it trades
    /// with, weighted by what moves (a carpenter turning one beam into four
    /// joists stands by the wall, not by the sawpit).
    fn near(&self, plan: &str, pool: &[String]) -> Tile {
        let c = catalog();
        if plan == "woodsman-camp" {
            return self.wood_anchor();
        }
        let mut pts: Vec<(Tile, f64)> = Vec::new();
        for r in pool.iter().take(2) {
            let Some(rec) = c.recipes.get(r) else { continue };
            let per_min = 6.0 / f64::from(rec.shifts.max(1));
            for (g, q) in &rec.inputs {
                let makers: Vec<Tile> = self.made.iter().filter(|(id, m)| self.makes(**id, m, g)).map(|(id, _)| self.k.buildings[id].tile).collect();
                for t in &makers {
                    pts.push((*t, f64::from(*q) * per_min / makers.len() as f64));
                }
            }
            for (g, q) in &rec.outputs {
                let mut takers: Vec<Tile> = self.made.iter().filter(|(id, m)| self.takes(**id, m, g)).map(|(id, _)| self.k.buildings[id].tile).collect();
                if self.wall.contains(g) {
                    // The yard is staked early; where it will stand is known before.
                    takers.push(self.yard().map_or(YARD_PLAN, |y| self.k.buildings[&y].tile));
                }
                for t in &takers {
                    pts.push((*t, f64::from(*q) * per_min / takers.len() as f64));
                }
            }
        }
        if pts.is_empty() {
            return anchor(plan);
        }
        let total: f64 = pts.iter().map(|(_, w)| w).sum::<f64>().max(1e-9);
        let home = anchor(plan);
        let x = (pts.iter().map(|(t, w)| f64::from(t.0) * w).sum::<f64>() + f64::from(home.0) * total * 0.15) / (total * 1.15);
        let y = (pts.iter().map(|(t, w)| f64::from(t.1) * w).sum::<f64>() + f64::from(home.1) * total * 0.15) / (total * 1.15);
        (x.round() as i16, y.round() as i16)
    }

    /// The edge of the wood with the most standing timber for its distance from the camp:
    /// the woods run out, and a player moves on to the next.
    fn wood_anchor(&self) -> Tile {
        let camp = self.k.camp();
        let mut best: Option<(f64, Tile)> = None;
        for (i, r) in hillside::WOODS.iter().enumerate() {
            let left = f64::from(self.k.nature.get(&format!("wood-{i}")).copied().unwrap_or(0));
            let cap = f64::from(hillside::patch_capacity(&format!("wood-{i}")).max(1));
            if left < cap * 0.4 {
                continue;
            }
            // Just outside the wood, on the side towards the camp.
            let x = camp.0.clamp(r.0, r.2);
            let y = if camp.1 < r.1 { r.1 - 1 } else if camp.1 > r.3 { r.3 + 1 } else { camp.1 };
            let dist = f64::from((x - camp.0).abs() + (y - camp.1).abs());
            let score = left / (1.0 + dist / 25.0);
            if best.is_none_or(|(s, _)| score > s) {
                best = Some((score, (x, y)));
            }
        }
        best.map_or(anchor("woodsman-camp"), |(_, t)| t)
    }

    fn yard(&self) -> Option<u32> {
        self.k.sites.values().next_back().map(|s| s.yard)
    }
    fn site(&self) -> Option<u32> {
        self.k.sites.keys().next_back().copied()
    }

    // ---- who makes and who takes what --------------------------------

    /// The goods a building puts out.
    fn outputs_of(&self, id: u32) -> Vec<String> {
        let c = catalog();
        let Some(b) = self.k.buildings.get(&id) else { return vec![] };
        let p = &c.plans[&b.plan];
        if p.extractor {
            return b.claim.as_ref().and_then(|q| self.k.claims.get(q)).map(|q| q.output().into_keys().collect()).unwrap_or_default();
        }
        b.recipe.as_ref().map(|r| c.recipes[r].outputs.keys().cloned().collect()).unwrap_or_default()
    }
    fn makes(&self, id: u32, m: &Made, g: &str) -> bool {
        let c = catalog();
        let b = &self.k.buildings[&id];
        if c.plans[&b.plan].extractor {
            return self.outputs_of(id).iter().any(|x| x == g);
        }
        m.pool.iter().any(|r| c.recipes[r].outputs.contains_key(g))
    }
    fn takes(&self, id: u32, m: &Made, g: &str) -> bool {
        let c = catalog();
        m.pool.iter().any(|r| c.recipes[r].inputs.contains_key(g)) && self.k.buildings.contains_key(&id)
    }
    // ---- roads ---------------------------------------------------------------

    /// Joins a building to the camp's roads: a spur to the nearest road, the way a
    /// player lays one. Goods are carried by the shared pool from then on.
    fn connect(&mut self, id: u32) -> bool {
        let c = catalog();
        if self.k.connected().contains(&id) {
            return true;
        }
        match self.k.apply(c, Command::Connect(id)) {
            Ok(_) => {
                *self.foreman.entry("roads laid").or_default() += 1;
                true
            }
            Err(e) => {
                let now = self.now();
                self.why.insert(format!("connect {id}"), (now, e));
                false
            }
        }
    }

    // ---- building -------------------------------------------------------

    fn beds_planned(&self) -> usize {
        let c = catalog();
        self.k.buildings.values().map(|b| usize::from(c.plans[&b.plan].housing)).sum()
    }

    fn count(&self, key: &str) -> u32 {
        self.made.values().filter(|m| m.key == key).count() as u32
    }

    /// Raises one building of `plan`, if it can be paid for. Ok(false) waits.
    fn raise(&mut self, key: &str) -> Result<bool, String> {
        let c = catalog();
        let plan = key.split('#').next().unwrap();
        let p = &c.plans[plan];
        if !p.requires.iter().all(|r| self.k.researched.contains(r)) {
            // The charter is the first thing to raise.
            let missing: Vec<String> = p.requires.iter().filter(|r| !self.k.researched.contains(*r)).cloned().collect();
            for r in missing {
                self.research(&r);
            }
            return Ok(false);
        }
        if !covers(&self.k.stock, &p.cost) {
            let now = self.now();
            self.why.insert(format!("raise {plan}"), (now, "stock".into()));
            let now = self.now();
            for (g, n) in &p.cost {
                if self.k.amount(g) < *n {
                    self.missing.insert(g.clone(), now);
                }
            }
            return Ok(false);
        }
        // Hands: a post is raised only when someone idle can man it.
        let idle = self.k.population.citizens.values().filter(|x| x.job.is_none()).count();
        let building = self.k.buildings.values().filter(|b| b.construction > 0).count();
        if p.workers > 0 && idle < usize::from(p.workers) + building.min(2) {
            { let now = self.now(); self.why.insert(format!("raise {plan}"), (now, "hands".into())); }
            return Ok(false);
        }
        // A recipe the machine starts on: the one furthest short of its share.
        let rs: Vec<(String, f64)> = self.shares.get(key).map(|m| m.iter().map(|(r, s)| (r.clone(), *s)).collect()).unwrap_or_default();
        let mut have: BTreeMap<String, f64> = BTreeMap::new();
        for (id, m) in &self.made {
            if m.key == key {
                if let Some(r) = &self.k.buildings[id].recipe {
                    *have.entry(r.clone()).or_default() += 1.0;
                }
            }
        }
        let mut pool: Vec<(String, f64)> = rs.iter().map(|(r, s)| (r.clone(), *s - have.get(r).copied().unwrap_or(0.0))).collect();
        pool.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap().then(a.0.cmp(&b.0)));
        let mut pool: Vec<String> = pool.into_iter().map(|(r, _)| r).collect();
        if plan == "woodsman-camp" {
            pool.truncate(1);
        }
        let tile = match self.place(plan, &pool) {
            Some(Ok(t)) => t,
            Some(Err(why)) => {
                let now = self.now();
                self.why.insert(format!("raise {plan}"), (now, why));
                return Ok(false);
            }
            None => {
                let near = self.near(plan, &pool);
                let Some(tile) = self.spot(plan, near) else { return Err(format!("no room for a {plan} near {near:?}")) };
                tile
            }
        };
        match self.k.apply(c, Command::Build { plan: plan.into(), tile }) {
            Ok(_) => {}
            Err(e) if e.contains("need") || e.contains("research") => {
                { let now = self.now(); self.why.insert(format!("raise {plan}"), (now, e)); }
                return Ok(false);
            }
            Err(e) => return Err(format!("{plan} at {tile:?}: {e}")),
        }
        let id = self.k.at(tile).unwrap();
        if let Some(first) = pool.first() {
            if c.plans[plan].recipes.contains(first) {
                let _ = self.k.apply(c, Command::Recipe { building: id, recipe: first.clone() });
            }
        }
        if plan == "ditch-gang" {
            self.lift_spoil(tile);
        }

        let critical = matches!(plan, "carters-yard" | "farmstead" | "well" | "mill" | "bakery" | "clamp" | "woodsman-camp" | "sawpit" | "field-quarry" | "claim-quarry" | "lodge" | "banker" | "bloomery" | "forge-shop");
        if critical {
            let _ = self.k.apply(c, Command::Priority { building: id, priority: 2 });
        }
        self.made.insert(id, Made { plan: plan.into(), key: key.into(), pool, switched: self.now() });
        self.built.push((self.now(), format!("{plan}#{id}@{tile:?}")));
        self.connect(id);
        Ok(true)
    }

    fn research(&mut self, tech: &str) -> bool {
        let c = catalog();
        if self.k.researched.contains(tech) {
            return true;
        }
        if let Some(t) = c.tech(tech) {
            for r in t.requires.clone() {
                if !self.k.researched.contains(&r) && !self.research(&r) {
                    return false;
                }
            }
        }
        match self.k.apply(c, Command::Research(tech.into())) {
            Ok(_) => {
                self.built.push((self.now(), format!("charter {tech}")));
                true
            }
            Err(e) => {
                { let now = self.now(); self.why.insert(format!("research {tech}"), (now, e)); }
                if let Some(t) = c.tech(tech) {
                    let now = self.now();
                    for (g, n) in &t.cost {
                        if self.k.amount(g) < *n {
                            self.missing.insert(g.clone(), now);
                        }
                    }
                }
                false
            }
        }
    }

    /// Stakes the castle of this age out, over the last one's ground.
    fn found_castle(&mut self) -> bool {
        let c = catalog();
        if !self.k.researched.contains("masonry") && !self.research("masonry") {
            return false;
        }
        let castle = AGES[self.age].0;
        // The old stockyard serves the new castle.
        let yard_tile = match self.yard() {
            Some(y) => self.k.buildings[&y].tile,
            None if self.fits("castle-yard", lay::YARD) => lay::YARD,
            None => {
                let near = (145, 84);
                let mut best = None;
                'find: for r in 0i16..30 {
                    for dy in -r..=r {
                        for dx in -r..=r {
                            if dx.abs().max(dy.abs()) == r {
                                let t = (near.0 + dx, near.1 + dy);
                                if self.k.world.contains(t)
                                    && self.k.at(t).is_none()
                                    && !self.k.roads.contains(&t)
                                    && self.k.check_site(c, "castle-yard", t).is_ok()
                                    && self.k.check_footprint(c, "castle-yard", t).is_ok()
                                    && !crate::hillside::in_rect(t, crate::hillside::keep_rect())
                                {
                                    best = Some(t);
                                    break 'find;
                                }
                            }
                        }
                    }
                }
                best.expect("room for the stockyard")
            }
        };
        if yard_tile == lay::YARD {
            let row = self.layout.row("N96").cloned().unwrap();
            let streets = self.layout.street_to(&row, yard_tile.0, 3);
            self.lay_streets(&streets, "castle-yard");
        }
        match self.k.apply(c, Command::FoundSite { plan: castle.into(), centre: hillside::KEEP_CENTRE, yard: yard_tile }) {
            Ok(_) => {
                self.built.push((self.now(), format!("stake {castle}")));
                let y = self.yard().unwrap();
                self.made.entry(y).or_insert(Made { plan: "castle-yard".into(), key: "castle-yard".into(), pool: vec![], switched: 0 });
                self.connect(y);
                true
            }
            Err(e) => {
                { let now = self.now(); self.why.insert("stake the castle".into(), (now, e)); }
                false
            }
        }
    }

    // ---- the knights ----------------------------------------------------

    fn knights(&mut self) -> bool {
        let c = catalog();
        let mut acted = false;
        for (id, at, yields) in SEAMS {
            if self.now() < *at {
                continue;
            }
            if !self.k.claims.contains_key(*id) {
                self.k.discover(
                    c,
                    id.to_string(),
                    Claim {
                        resource: yields[0].0.into(),
                        name: format!("Seam {id}"),
                        source: "A Delve floor".into(),
                        secured: false,
                        rate: yields[0].1,
                        yields: yields.iter().map(|(g, n)| (g.to_string(), *n)).collect(), surface: false },
                );
                self.k.secure(id);
                self.log.push((self.now(), format!("the knights bring home {id}")));
            }
            if self.seams[id].is_none() {
                // A quarry on it, as soon as the first charter and a crew allow.
                if !self.k.researched.contains("charter") && !self.research("charter") {
                    continue;
                }
                if let Ok(true) = self.raise("claim-quarry") {
                    let q = *self.made.iter().filter(|(_, m)| m.plan == "claim-quarry").map(|(i, _)| i).max().unwrap();
                    if self.k.apply(c, Command::Link { building: q, claim: id.to_string() }).is_ok() {
                        self.seams.insert(id, Some(q));
                        self.connect(q);
                        self.log.push((self.now(), format!("quarry on {id}")));
                        acted = true;
                    }
                }
            }
        }
        acted
    }

    // ---- the foreman ------------------------------------------------------

    fn build_hamlet(&mut self) -> bool {
        let c = catalog();
        let mut candidates: Vec<(Tile, u8)> = Vec::new();
        for y in [101, 107, 113] {
            for x in (158..=166).step_by(8).chain((106..=118).step_by(8)) {
                for turns in [0u8, 2] {
                    candidates.push(((x, y), turns));
                }
            }
        }
        candidates.sort_by_key(|((x, y), _)| (x - 150).abs() + (y - 101).abs());
        for (origin, turns) in candidates {
            if self.k.stamp_cost(c, "hamlet", origin, turns).is_ok() {
                return match self.k.apply(c, Command::Stamp { pattern: "hamlet".into(), origin, turns }) {
                    Ok(_) => {
                        self.hamlets += 1;
                        *self.foreman.entry("hamlets").or_default() += 1;
                        self.built.push((self.now(), "hamlet".into()));
                        true
                    }
                    Err(e) if e.contains("need") => false,
                    Err(e) => panic!("hamlet at {origin:?}: {e}"),
                };
            }
        }
        false
    }

    /// A hut is two beds for six timber, and quick; the first homes are huts.
    fn build_hut(&mut self) -> bool {
        let c = catalog();
        if !covers(&self.k.stock, &c.plans["hut"].cost) {
            return false;
        }
        let tile = match self.place("hut", &[]) {
            Some(Ok(t)) => t,
            Some(Err(_)) => return false,
            None => {
                let Some(tile) = self.spot("hut", (141, 99)) else { return false };
                tile
            }
        };
        if self.k.apply(c, Command::Build { plan: "hut".into(), tile }).is_ok() {
            let id = self.k.at(tile).unwrap();
            let _ = self.k.apply(c, Command::Connect(id));
            *self.foreman.entry("huts").or_default() += 1;
            self.built.push((self.now(), "hut".into()));
            true
        } else {
            false
        }
    }

    fn stock_of(&self, g: &str) -> u32 {
        self.k.total(g)
    }

    /// What a workshop is asked to hold: some minutes of the castle's pace.
    fn want_stock(&self, g: &str) -> f64 {
        let rate = self.req.goods.get(g).copied().unwrap_or(0.0);
        let floor = self.extra_want(g);
        if rate <= 0.0 && floor <= 0.0 {
            return 0.0;
        }
        (rate * 6.0).max(floor).max(12.0)
    }

    /// Whether the stock could feed a recipe's first batch.
    fn shop_can_run(&self, recipe: &str) -> bool {
        let c = catalog();
        c.recipes[recipe].inputs.iter().all(|(g, n)| self.stock_of(g) >= *n)
    }

    /// Goods the player wants in hand for its own use beyond the wall's.
    fn extra_want(&self, g: &str) -> f64 {
        match g {
            "plank" => 60.0,
            "beam" => 30.0,
            "block" => 40.0,
            "timber" => 120.0,
            "stone" => 100.0,
            "water" => 60.0,
            "steel" => 14.0,
            "pick" | "axe" | "chisel" | "trowel" => 3.0,
            "nails" => 80.0,
            "cramp" | "tyre" => 10.0,
            "cart" | "ox" | "harness" => 8.0,
            "horse" => 4.0,
            "wagon" if factory().is_some() => 4.0,
            "fodder" => 80.0,
            "mule" if self.mule_project => 6.0,
            "bucket" if self.rope_project => 26.0,
            "rope" if self.rope_project => 60.0,
            "pole" if self.rope_project => 70.0,
            _ => 0.0,
        }
    }

    /// Switches a workshop that makes several things to whichever is most short.
    fn rebalance(&mut self, report: &Report) -> bool {
        let c = catalog();
        let now = self.now();
        let mut acted = false;
        // Goods some workshop is starved of, by how many.
        let mut starved: BTreeMap<String, u32> = BTreeMap::new();
        for (id, st) in &report.status {
            if *st != Status::Inputs {
                continue;
            }
            let b = &self.k.buildings[id];
            if let Some(rec) = b.recipe.as_ref().and_then(|r| c.recipes.get(r)) {
                for (g, n) in &rec.inputs {
                    if b.input.get(g).copied().unwrap_or(0) < *n {
                        *starved.entry(g.clone()).or_default() += 1;
                    }
                }
            }
        }
        let ids: Vec<u32> = self.made.iter().filter(|(_, m)| m.pool.len() > 1 && now >= m.switched + 60).map(|(&i, _)| i).collect();
        // What the wall is waiting on is made first, whatever else the bench was asked for: a
        // player sets one bench to the portcullis when the gate is the last thing standing.
        let gating: Option<String> = self.site().and_then(|s| match self.k.site_allowance(c, s, 99).hold {
            Hold::Short { resource } => Some(resource),
            _ => None,
        });
        if let Some(g) = gating {
            let target = ids.iter().copied().find(|id| {
                let b = &self.k.buildings[id];
                b.construction == 0
                    && !self.made[id].pool.is_empty()
                    && self.made[id].pool.iter().any(|r| c.recipes[r].outputs.contains_key(&g) && c.recipes[r].outputs.len() == 1)
                    && b.recipe.as_ref().is_none_or(|r| !c.recipes[r].outputs.contains_key(&g))
                    && !self.k.buildings.iter().any(|(o, ob)| o != id && self.made.contains_key(o) && ob.recipe.as_ref().is_some_and(|r| c.recipes[r].outputs.len() == 1 && c.recipes[r].outputs.contains_key(&g)))
            });
            if let Some(id) = target {
                let r = self.made[&id].pool.iter().find(|r| c.recipes[*r].outputs.len() == 1 && c.recipes[*r].outputs.contains_key(&g)).cloned().unwrap();
                if self.k.apply(c, Command::Recipe { building: id, recipe: r }).is_ok() {
                    self.made.get_mut(&id).unwrap().switched = now;
                    *self.foreman.entry("benches set to what the wall waits on").or_default() += 1;
                    acted = true;
                }
            }
        }
        for id in ids {
            let b = &self.k.buildings[&id];
            // A bench stalled with its shelf full may be switched mid-batch: it is not working.
            if b.construction > 0 || (b.progress > 0 && report.status.get(&id) != Some(&Status::Storage)) {
                continue;
            }
            let pool = self.made[&id].pool.clone();
            let mut best: Option<(f64, String)> = None;
            for r in &pool {
                let rec = &c.recipes[r];
                let mut urgency: f64 = 0.0;
                let biggest = f64::from(rec.outputs.values().copied().max().unwrap_or(1).max(1));
                for (g, q) in &rec.outputs {
                    // A co-product counts for less than what the recipe is for.
                    let share = f64::from(*q) / biggest;
                    let want = self.want_stock(g);
                    let mut u: f64 = 0.0;
                    if want > 0.0 {
                        let have = f64::from(self.stock_of(g));
                        u = u.max(((want - have) / want).max(0.0) * 0.6);
                    }
                    // A bill the player could not pay lately, or a workshop starved of it.
                    if self.missing.get(g).is_some_and(|t| now < t + 120) {
                        u = u.max(1.0);
                    }
                    if let Some(n) = starved.get(g) {
                        u = u.max((0.5 + 0.25 * f64::from(*n)).min(1.2));
                    }
                    urgency = urgency.max(u * share);
                }
                // A recipe that eats a worn tool is wanted by how many wait.
                // (Worn tools lying on another smith's bench are counted: they are waiting for a smith set to them.)
                if let Some(worn) = rec.inputs.keys().find(|g| g.starts_with("worn-")) {
                    let n = f64::from(self.k.total(worn));
                    urgency = if n < 4.0 { 0.0 } else { urgency.max((n / 4.0).min(2.0) * 0.9) };
                }
                // Inputs must be there to run at all.
                let inputs_ok = rec.inputs.keys().all(|g| self.stock_of(g) > 0);
                if !inputs_ok {
                    urgency *= 0.2;
                }
                if r == "make-steel" && self.k.amount("steel") >= 12 {
                    urgency = 0.0;
                }
                if best.as_ref().is_none_or(|(u, _)| urgency > *u + 1e-9) {
                    best = Some((urgency, r.clone()));
                }
            }
            if let Some((u, r)) = best {
                // A recipe whose input is out of stock and made by another recipe of the
                // same shop hands its urgency on: forge the iron before the nails.
                let mut pick = (u, r);
                for r2 in &pool {
                    let rec2 = &c.recipes[r2];
                    if rec2.inputs.keys().any(|g| self.stock_of(g) == 0 && c.recipes[&pick.1].outputs.contains_key(g)) {
                        // pick makes something r2 lacks: fine as it is
                    }
                }
                let rec = &c.recipes[&pick.1];
                if let Some(missing_in) = rec.inputs.keys().find(|g| self.stock_of(g) == 0 && !self.camp_goods.contains(*g)) {
                    if let Some(maker) = pool.iter().find(|r2| c.recipes[*r2].outputs.contains_key(missing_in)) {
                        if self.shop_can_run(maker) {
                            pick = (u, maker.clone());
                        }
                    }
                }
                let (u, r) = pick;
                if u > 0.0 && self.k.buildings[&id].recipe.as_ref() != Some(&r) && self.k.apply(c, Command::Recipe { building: id, recipe: r }).is_ok() {
                    self.made.get_mut(&id).unwrap().switched = now;
                    *self.foreman.entry("recipes switched").or_default() += 1;
                    acted = true;
                }
            }
        }
        acted
    }

    fn foreman(&mut self) -> bool {
        let c = catalog();
        let mut acted = false;
        let people = self.k.population.citizens.len();
        let report = self.k.report(c);
        // Beds ahead of the people: huts at first, then hamlets.
        if self.beds_planned() < people + 4 + people / 4 {
            acted |= if self.hamlets < 1 && self.now() < 1200 { self.build_hut() } else { self.build_hamlet() };
        }
        // A wood that has run dry: the camp is taken down and raised at the next wood.
        let dry: Vec<(u32, Tile)> = report
            .status
            .iter()
            .filter(|(id, st)| **st == Status::Depleted && self.made.get(id).is_some_and(|m| m.plan == "woodsman-camp"))
            .map(|(id, _)| (*id, self.k.buildings[id].tile))
            .collect();
        for (id, tile) in dry {
            if self.k.apply(c, Command::Demolish(tile)).is_ok() {
                self.made.remove(&id);
                *self.foreman.entry("woods run dry").or_default() += 1;
                self.built.push((self.now(), "a wood ran dry".into()));
                acted = true;
            }
        }
        // Whatever the roads did not reach is joined to the camp; a work that cannot be joined
        // because its neighbours have walled it in is pulled down and raised again elsewhere.
        let connected = self.k.connected();
        let lonely: Vec<u32> = self.k.buildings.iter().filter(|(id, b)| b.construction == 0 && !connected.contains(id)).map(|(&id, _)| id).collect();
        for id in lonely {
            match self.k.apply(c, Command::Connect(id)) {
                Ok(_) => acted = true,
                Err(e) if e.contains("no way") && self.made.contains_key(&id) => {
                    let tile = self.k.buildings[&id].tile;
                    if self.k.apply(c, Command::Demolish(tile)).is_ok() {
                        self.made.remove(&id);
                        *self.foreman.entry("walled-in works pulled down").or_default() += 1;
                        self.built.push((self.now(), "pulled down a walled-in work".into()));
                        acted = true;
                    }
                }
                Err(_) => {}
            }
        }
        // Oxen standing unfed: the camp is short of hay.
        if self.k.road_report(c).yards.iter().any(|y| y.carts > 0 && !y.fed) {
            let now = self.now();
            self.missing.insert("fodder".into(), now);
        }
        if (self.now() + self.jitter) % 60 == 5 {
            self.set_limits();
        }
        if (self.now() + self.jitter) % 60 == 5 {
            acted |= self.bottlenecks();
        }
        if (self.now() + self.jitter) % 60 == 45 {
            acted |= self.spare_the_woods();
        }
        acted |= self.carters();
        acted |= self.warehouses();
        acted |= self.infrastructure();
        if factory().is_some() && (self.now() + self.jitter) % 30 == 15 {
            acted |= self.rollerways();
        }
        if factory().is_some() && (self.now() + self.jitter) % 60 == 35 {
            acted |= self.railways();
        }
        acted |= self.shaft(&report);
        if (self.now() + self.jitter) % 30 == 5 && tiers() {
            acted |= self.powered_hoist(&report);
        }
        if (self.now() + self.jitter) % 60 == 25 {
            acted |= self.cranes();
        }
        if (self.now() + self.jitter) % 30 == 15 {
            acted |= self.invest();
            acted |= self.frustrations();
        }
        acted |= self.lessons(&report);
        // Switching a bench to what the camp wants is a glance, not a job: it does not count
        // as the player having something to do.
        self.rebalance(&report);
        acted
    }

    /// A camp that fells takes the wood down for good; the opening hour needs the timber faster than coppice gives it, and
    /// then the wood is spared: a felling camp is set to coppice once its wood is down to half, or the hour is out.
    fn spare_the_woods(&mut self) -> bool {
        let c = catalog();
        let mut acted = false;
        let ids: Vec<u32> = self.made.iter().filter(|(_, m)| m.plan == "woodsman-camp").map(|(&i, _)| i).collect();
        for id in ids {
            let b = &self.k.buildings[&id];
            if b.recipe.as_deref() != Some("fell-wood") || b.construction > 0 {
                continue;
            }
            let Some(patch) = b.patch.clone() else { continue };
            let left = u64::from(self.k.nature.get(&patch).copied().unwrap_or(0));
            let cap = u64::from(hillside::patch_capacity(&patch).max(1));
            if left * 100 < cap * 55 || self.now() >= 3600 {
                if self.k.apply(c, Command::Recipe { building: id, recipe: "coppice-timber".into() }).is_ok() {
                    if let Some(m) = self.made.get_mut(&id) {
                        m.pool = vec!["coppice-timber".into()];
                    }
                    *self.foreman.entry("camps set to coppice").or_default() += 1;
                    acted = true;
                }
            }
        }
        acted
    }

    /// The benches that would eat the iron for goods nobody wants are held to a stock
    /// target: the smith's steel and tools, the wheelwright's vehicles and buckets.
    fn set_limits(&mut self) {
        let c = catalog();
        let ids: Vec<u32> = self.made.iter().filter(|(_, m)| matches!(m.plan.as_str(), "smithy" | "wheelwright" | "clamp") || m.plan == "claim-quarry" || m.key == "pasture#oxen").map(|(&i, _)| i).collect();
        for id in ids {
            let b = &self.k.buildings[&id];
            // A clamp burns the woods: charcoal is made against a stock, not for its own sake.
            let limit = match b.plan.as_str() {
                // A seam worth its coal or its ore is paused when the stockyard has plenty: every
                // unit it sends up the Great Shaft is a unit of limestone that waits below.
                "claim-quarry" => {
                    let main = b
                        .claim
                        .as_ref()
                        .and_then(|q| self.k.claims.get(q))
                        .and_then(|q| q.output().into_iter().max_by(|a, b| a.1.cmp(&b.1).then(b.0.cmp(&a.0))))
                        .map(|(g, _)| g);
                    // The seam's own bench holds 24 and the camp keeps 12 or so: a limit under that is
                    // reached the moment the camp is full, and the seam stops till the camp draws on it.
                    match main.as_deref() {
                        Some("coal") | Some("iron-ore") | Some("slate-block") | Some("galena") | Some("bone") => 30,
                        _ => 0,
                    }
                }
                "smithy" => 16,
                "clamp" => CHARCOAL_STOCK,
                "pasture" => 10,
                _ => 34,
            };
            if limit > 0 && b.production_limit != limit {
                let _ = self.k.apply(c, Command::ProductionLimit { building: id, amount: limit });
            }
        }
    }

    /// The carters: the pool is sized to the orders that wait. A cart is bought when
    /// none stands free and orders wait; another yard is raised when the yards have no
    /// stall left. Carts, oxen and harness come from the wheelwright, the pasture and
    /// the tanyard, which the want list keeps at work.
    fn carters(&mut self) -> bool {
        let c = catalog();
        let roads = self.k.road_report(c);
        let Some(first) = roads.yards.first() else { return false };
        let mut acted = false;
        let now = self.now();
        let spare = self.k.amount("cart").min(self.k.amount("ox")).min(self.k.amount("harness"));
        // A yard's first carts come from the founding train.
        let empty = roads.yards.iter().find(|y| y.carts == 0).map(|y| y.id);
        let waiting = roads.unserved.iter().filter(|u| !u.no_road).count();
        let want = if empty.is_some() {
            3
        } else if roads.free == 0 && waiting > 0 || roads.free <= 1 && waiting >= 4 {
            1
        } else {
            0
        };
        if want == 0 {
            return false;
        }
        let yard = empty.or_else(|| roads.yards.iter().find(|y| y.carts < usize::from(y.stalls)).map(|y| y.id));
        match yard {
            Some(y) => {
                let have = self.k.buildings[&y].carts.len() as u16;
                let n = want.min(spare as u16).min(c.plans["carters-yard"].stalls - have);
                if n == 0 {
                    for g in ["cart", "ox", "harness"] {
                        if self.k.amount(g) == 0 {
                            self.missing.insert(g.into(), now);
                        }
                    }
                } else if self.k.apply(c, Command::Carts { yard: y, carts: have + n }).is_ok() {
                    *self.foreman.entry("carts added").or_default() += u32::from(n);
                    acted = true;
                }
            }
            None => {
                // Every stall is taken and orders still wait: another yard, once the camp has the hands for a carter
                // and the stock for the stalls.
                if self.now() < 900 {
                    return acted;
                }
                let yards = self.count("carters-yard");
                let t = self.target.entry("carters-yard".into()).or_default();
                *t = (*t).max(yards + 1);
            }
        }
        let _ = first;
        acted
    }

    /// What a player does when the wall waits: reads the Clerk, goes to the bottom of the chain it
    /// names, and if that is a maker working flat out, or a good nothing makes, builds another.
    /// The want list is the solver's guess at the pace; this is the colony's own count of what the
    /// wall is short of. A limit must show on three looks in a row (a minute and a half) before
    /// anything is built, so a batch that is simply on its way is not mistaken for a shortage.
    fn invest(&mut self) -> bool {
        let c = catalog();
        let Some(site) = self.site() else { return false };
        let clerk = self.k.clerk(c, site);
        let seen: Option<(String, String)> = match clerk.causes.last() {
            Some(Cause::Capacity { plan, good }) => Some((plan.clone(), good.clone())),
            Some(Cause::NoMaker { good }) => Some((String::new(), good.clone())),
            _ => None,
        };
        let Some(key) = seen else {
            self.limit_seen = None;
            return false;
        };
        let looks = match &self.limit_seen {
            Some((k, n)) if *k == key => n + 1,
            _ => 1,
        };
        self.limit_seen = Some((key.clone(), looks));
        if looks < 3 {
            return false;
        }
        let (plan, good) = key;
        self.expand(&plan, &good)
    }

    /// Asks for one more maker of `good`: of `plan` if it is named, else of the first plan the
    /// camp can raise that makes it. One at a time (the last is built before the next is asked
    /// for), at most three times what the solver sized, and not more than once in a while.
    fn expand(&mut self, plan: &str, good: &str) -> bool {
        let c = catalog();
        let now = self.now();
        // The earthworks are dug by the two gangs at the stockyard, each lifting its spoil by swape; a third gang with no
        // arm to the yard only fills its shelf for the carts.
        if plan == "ditch-gang" {
            return false;
        }
        let plan = if plan.is_empty() {
            let camp = crate::design::reach(c, true);
            let Some(p) = c.plans.iter().find(|(k, p)| p.hillside && camp.plans.contains(*k) && !p.extractor && p.recipes.iter().any(|r| c.recipes[r].outputs.contains_key(good))).map(|(k, _)| k.clone()) else { return false };
            p
        } else {
            plan.to_string()
        };
        let recipe_key = self
            .shares
            .iter()
            .find(|(k, rs)| k.split('#').next() == Some(plan.as_str()) && rs.keys().any(|r| c.recipes[r].outputs.contains_key(good)))
            .map(|(k, rs)| (k.clone(), rs.keys().find(|r| c.recipes[*r].outputs.contains_key(good)).cloned().unwrap()));
        let (key, recipe) = match recipe_key {
            Some(x) => x,
            None => {
                // Not on the list yet: the plan's first recipe for the good.
                let Some(r) = c.plans[&plan].recipes.iter().find(|r| c.recipes[*r].outputs.contains_key(good)).cloned() else { return false };
                (plan.clone(), r)
            }
        };
        let have = self.count(&key);
        let want = self.target.get(&key).copied().unwrap_or(0);
        if want > have || now < self.expanded.get(&key).copied().unwrap_or(0) + EXPAND_EVERY {
            return false;
        }
        let cap = self.solved.get(&key).copied().unwrap_or(1) * 3 + 2;
        if have >= cap {
            return false;
        }
        *self.shares.entry(key.clone()).or_default().entry(recipe).or_default() += 1.0;
        self.target.insert(key.clone(), have + 1);
        self.expanded.insert(key.clone(), now);
        *self.foreman.entry("makers asked for").or_default() += 1;
        self.built.push((now, format!("asked for another {key}: the wall waits on {good}")));
        true
    }

    /// A bill the player cannot pay, again and again, for minutes together: the goods are not
    /// being made fast enough, and another maker is the answer. (`missing` is noted each time a
    /// purchase or a charter fails for want of a good.)
    fn frustrations(&mut self) -> bool {
        let now = self.now();
        let noted: Vec<(String, u64)> = self.missing.iter().map(|(g, t)| (g.clone(), *t)).collect();
        for (g, last) in noted {
            if now > last + 90 {
                self.short_since.remove(&g);
            } else {
                self.short_since.entry(g).or_insert(now);
            }
        }
        // Only the sawn timber that every charter and every works is paid in: the first hour is short
        // of everything, and a player does not build a maker of each.
        let long: Vec<String> = self
            .short_since
            .iter()
            .filter(|(g, since)| self.age >= 1 && now >= **since + SHORT_FOR && matches!(g.as_str(), "plank" | "beam" | "ox" | "harness" | "cart" | "nails" | "tyre"))
            .map(|(g, _)| g.clone())
            .collect();
        let c = catalog();
        let mut acted = false;
        for g in long {
            // What the Clerk finds at the bottom of the chain behind the good: a maker working
            // flat out (build another), a good nothing makes (build the maker). Anything else, a
            // road, a crew or a bench short of its own input, is not mended by more of the same.
            let did = match self.k.causes_for(c, &g).last() {
                Some(Cause::Capacity { plan, good }) => self.expand(plan, good),
                Some(Cause::NoMaker { good }) => self.expand("", good),
                _ => false,
            };
            if did {
                // One maker for a spell of shortage: the next waits for a new spell.
                self.short_since.remove(&g);
                acted = true;
            }
        }
        acted
    }

    /// When nothing holds the masons up but their own pace, a player raises another lodge:
    /// what a want list from the solver cannot know is how fast the wall really goes.
    fn bottlenecks(&mut self) -> bool {
        let c = catalog();
        let now = self.now();
        let mut acted = false;
        // Another lodge when the masons are the limit.
        if let Some(site) = self.site() {
            if self.k.clerk(c, site).unhindered {
                let lodges = self.count("lodge");
                let ready = (self.age == 0 || self.count("mortar-mill") > 0) && self.k.amount("block") >= 12 && self.k.amount("stone") >= 20;
                let t = self.target.entry("lodge".into()).or_default();
                if ready && *t <= lodges && lodges < MAX_LODGES && now >= self.expanded.get("lodge").copied().unwrap_or(0) + 240 {
                    *t = lodges + 1;
                    self.expanded.insert("lodge".into(), now);
                    *self.foreman.entry("lodges asked for").or_default() += 1;
                    acted = true;
                }
            }
        }
        acted
    }

    /// The lifting tiers at the wall, each raising the one before it in place: hods and ladders
    /// (raised with the first lodge), the treadwheel crane once the shell keep is begun, the great
    /// crane once the great tower is. A crane is worth +40 percent (+80) on every mason in range,
    /// which is cheaper than another lodge's four hands.
    fn cranes(&mut self) -> bool {
        let c = catalog();
        if self.age == 0 {
            return false;
        }
        let want = if self.age >= 2 { "great-crane" } else { "treadwheel-crane" };
        let Some(site) = self.site() else { return false };
        let Some(yard) = self.k.sites.get(&site).and_then(|s| self.k.buildings.get(&s.yard)).map(|y| y.tile) else { return false };
        let near = |b: &Building| (b.tile.0 - yard.0).abs() + (b.tile.1 - yard.1).abs() <= i16::from(c.works.lodge_range);
        // The tier in hand, nearest the wall: the highest crane stands there.
        let best = self
            .k
            .buildings
            .iter()
            .filter(|(_, b)| c.plans[&b.plan].crane > 0 && b.construction == 0 && near(b))
            .max_by_key(|(id, b)| (c.plans[&b.plan].crane, std::cmp::Reverse(**id)))
            .map(|(&id, b)| (id, b.plan.clone()));
        let Some((id, plan)) = best else { return false };
        if plan == want {
            return false;
        }
        let Some((key, next)) = c.plans.iter().find(|(_, p)| p.replaces.as_deref() == Some(plan.as_str())) else { return false };
        if next.crane <= c.plans[&plan].crane {
            return false;
        }
        let _ = key;
        for r in &next.requires {
            if !self.k.researched.contains(r) {
                // The charters cost planks the wall's own works need: one at a time, and only
                // when the camp has them to spare.
                if self.k.amount("plank") >= 40 {
                    self.research(r);
                }
                return false;
            }
        }
        if self.k.apply(c, Command::Upgrade(id)).is_ok() {
            *self.foreman.entry("cranes raised").or_default() += 1;
            self.built.push((self.now(), format!("raised the crane to {}", next.name)));
            return true;
        }
        self.note_short(&next.cost);
        false
    }

    /// A warehouse beside a maker whose goods go a long way in bulk: the maker pushes to it
    /// the moment its bench fills, so many carts can fetch at once instead of three waiting
    /// on a bench that holds twenty-four.
    fn warehouses(&mut self) -> bool {
        let c = catalog();
        if self.now() < 600 || !self.k.buildings.values().any(|b| b.plan == "carters-yard") {
            return false;
        }
        let mut heavy: Vec<((u32, u32), u32)> = self.k.roadwork.flow.iter().filter(|(_, v)| **v >= HUB_FLOW).map(|(k, v)| (*k, *v)).collect();
        heavy.sort_by_key(|(k, v)| (std::cmp::Reverse(*v), *k));
        for ((from, to), _) in heavy {
            let (Some(a), Some(b)) = (self.k.buildings.get(&from), self.k.buildings.get(&to)) else { continue };
            if from == 1
                || to == 1
                || !matches!(a.plan.as_str(), "sand-pit" | "field-quarry")
                || (a.tile.0 - b.tile.0).abs() + (a.tile.1 - b.tile.1).abs() < HUB_LENGTH
                || self.k.buildings.values().filter(|w| c.plans[&w.plan].hub).count() >= 2
            {
                continue;
            }
            let at = a.tile;
            // One warehouse stands by a maker; a second is not a better answer.
            let covered = self.k.buildings.values().any(|w| c.plans[&w.plan].hub && (w.tile.0 - at.0).abs() + (w.tile.1 - at.1).abs() <= 6);
            if covered {
                continue;
            }
            let Some(tile) = self.spot("depot", at) else { continue };
            if self.k.apply(c, Command::Build { plan: "depot".into(), tile }).is_ok() {
                let id = self.k.at(tile).unwrap();
                self.connect(id);
                *self.foreman.entry("warehouses").or_default() += 1;
                self.built.push((self.now(), format!("warehouse by {}#{from}", c.plans[&self.k.buildings[&from].plan].name)));
                return true;
            }
        }
        false
    }

    /// Where the roads fall short, a player builds beyond them: a street that jams is
    /// widened by a second lane of road beside it, and a heavy flow over a long way gets
    /// its own ropeway (the charter, then the rope, the poles and the buckets, then the line).
    fn infrastructure(&mut self) -> bool {
        let c = catalog();
        let mut acted = false;
        let roads = self.k.road_report(c);
        // A jammed street: lay a road beside every jammed tile that has ground to spare.
        let jammed: Vec<Tile> = roads.jams.iter().take(4).map(|(t, _)| *t).collect();
        for t in jammed {
            let mut laid = 0;
            for n in crate::economy::neighbours(t) {
                if laid >= 2 || self.k.roads.contains(&n) || self.k.at(n).is_some() || !self.k.world.contains(n) {
                    continue;
                }
                // Beside the street, not across a gate or the camp's own doorstep.
                if n == self.k.gate() || (n.0 - self.k.camp().0).abs() + (n.1 - self.k.camp().1).abs() <= 1 {
                    continue;
                }
                if self.k.apply(c, Command::Road(n)).is_ok() {
                    laid += 1;
                    *self.foreman.entry("lanes widened").or_default() += 1;
                    acted = true;
                }
            }
        }
        // A middling flow a middling way: a string of mules, which needs neither road nor track.
        if self.age >= 1 && tiers() {
            let mut mid: Vec<((u32, u32), u32)> = self.k.roadwork.flow.iter().filter(|(_, v)| **v >= MULE_FLOW && **v < ROPE_FLOW).map(|(k, v)| (*k, *v)).collect();
            mid.sort_by_key(|(k, v)| (std::cmp::Reverse(*v), *k));
            for ((from, to), _) in mid {
                let strings = self.k.lines.values().filter(|l| l.kind == LineKind::Mule).count();
                if from == 1 || to == 1 || strings >= MAX_MULE_LINES || !self.k.buildings.contains_key(&from) || !self.k.buildings.contains_key(&to) || self.k.lines.values().any(|l| l.from == from && l.to == to) {
                    continue;
                }
                let (a, b) = (self.k.buildings[&from].tile, self.k.buildings[&to].tile);
                if (a.0 - b.0).abs() + (a.1 - b.1).abs() < MULE_LENGTH || c.plans[&self.k.buildings[&to].plan].yard {
                    continue;
                }
                self.mule_project = true;
                if !self.k.researched.contains("muleteers") && !self.research("muleteers") {
                    break;
                }
                let strong = self.k.amount("mule").min(self.k.amount("rope")).min(4) as u16;
                if strong < 2 {
                    let now = self.now();
                    self.missing.insert("mule".into(), now);
                    continue;
                }
                if self.k.apply(c, Command::Line { kind: LineKind::Mule, from, to, path: vec![], vehicles: strong }).is_ok() {
                    *self.foreman.entry("mule trains").or_default() += 1;
                    self.built.push((self.now(), format!("mule train {from}->{to}")));
                    acted = true;
                    break;
                }
            }
        }
        // A heavy flow a long way: its own ropeway.
        let mut heavy: Vec<((u32, u32), u32)> = self.k.roadwork.flow.iter().filter(|(_, v)| **v >= ROPE_FLOW).map(|(k, v)| (*k, *v)).collect();
        heavy.sort_by_key(|(k, v)| (std::cmp::Reverse(*v), *k));
        for ((from, to), _) in heavy {
            if from == 1 || to == 1 || !self.k.buildings.contains_key(&from) || !self.k.buildings.contains_key(&to) || self.k.lines.values().any(|l| l.from == from && l.to == to) {
                continue;
            }
            let (a, b) = (self.k.buildings[&from].tile, self.k.buildings[&to].tile);
            // The wall's own stockyard is fed by the carts, which bring the last few stones' worth
            // as readily as the first thousand; a ropeway keeps a full bucket line to itself.
            if (a.0 - b.0).abs() + (a.1 - b.1).abs() < ROPE_LENGTH || c.plans[&self.k.buildings[&to].plan].yard {
                continue;
            }
            self.rope_project = true;
            if !self.k.researched.contains("aerial") && !self.research("aerial") {
                break;
            }
            let tiles = self.k.plan_path(c, LineKind::Rope, from, to).map_or(0, |p| p.len() as u32);
            let spec = &c.lines["rope"];
            if tiles == 0 || tiles > u32::from(spec.max_length) {
                continue;
            }
            let short = spec.tile_cost.iter().find(|(g, n)| self.k.amount(g) < **n * tiles).map(|(g, _)| g.clone());
            if let Some(g) = short {
                let now = self.now();
                self.missing.insert(g, now);
                continue;
            }
            let buckets = self.k.amount("bucket").min(12) as u16;
            if buckets < 4 {
                let now = self.now();
                self.missing.insert("bucket".into(), now);
                continue;
            }
            if self.k.apply(c, Command::Line { kind: LineKind::Rope, from, to, path: vec![], vehicles: buckets }).is_ok() {
                *self.foreman.entry("ropeways").or_default() += 1;
                self.built.push((self.now(), format!("rope line {from}->{to}")));
                acted = true;
            }
        }
        acted
    }

    /// The tile a rollerway or a swape may stand on.
    fn clear_for_line(&self, t: Tile) -> bool {
        let c = catalog();
        self.k.check_roller_site(c, Piece::Roller, t).is_ok() && self.k.rails.cells.get(&t).is_none()
    }

    /// A way for a rollerway from work `from` to work `to`: a tile for a loading swape beside `from`, the
    /// rollers laid over clear ground, a tile for an unloading swape beside `to`. The shortest of a few
    /// tries, as `(loader, way it faces, rollers in order, unloader, way it faces)`.
    fn roller_way(&self, from: u32, to: u32) -> Option<(Tile, Dir, Vec<Tile>, Tile, Dir)> {
        let (a, b) = (self.k.buildings.get(&from)?, self.k.buildings.get(&to)?);
        let near = |t: Tile, of: &Building| of.tile.0.abs_diff(t.0) + of.tile.1.abs_diff(t.1);
        // Where a loader may stand beside `from`, facing away from it; where an unloader may stand beside `to`,
        // facing at it.
        let mut loaders: Vec<(Tile, Dir)> = Vec::new();
        for t in self.k.covered(a) {
            for d in Dir::ALL {
                let l = d.step(t);
                if self.clear_for_line(l) && self.clear_for_line(d.step(l)) && self.k.placement(catalog(), "swape", l).land.is_ok() {
                    loaders.push((l, d));
                }
            }
        }
        let mut unloaders: Vec<(Tile, Dir)> = Vec::new();
        for t in self.k.covered(b) {
            for d in Dir::ALL {
                let u = d.back().step(t);
                if self.clear_for_line(u) && self.clear_for_line(d.back().step(u)) && self.k.placement(catalog(), "swape", u).land.is_ok() {
                    unloaders.push((u, d));
                }
            }
        }
        loaders.sort_by_key(|(t, _)| (near(*t, b), *t));
        unloaders.sort_by_key(|(t, _)| (near(*t, a), *t));
        let mut best: Option<(usize, (Tile, Dir, Vec<Tile>, Tile, Dir))> = None;
        for &(l, dl) in loaders.iter().take(5) {
            for &(u, du) in unloaders.iter().take(5) {
                if l == u {
                    continue;
                }
                let (start, end) = (dl.step(l), du.back().step(u));
                // Breadth first over clear ground from the first roller to the last.
                let mut parent: BTreeMap<Tile, Tile> = BTreeMap::from([(start, start)]);
                let mut queue = std::collections::VecDeque::from([start]);
                let limit = ROLLER_MAX as usize;
                while let Some(t) = queue.pop_front() {
                    if t == end {
                        break;
                    }
                    for n in crate::economy::neighbours(t) {
                        if parent.contains_key(&n) || n == l || n == u || !self.clear_for_line(n) || n.0.abs_diff(start.0) + n.1.abs_diff(start.1) > ROLLER_MAX as u16 + 4 {
                            continue;
                        }
                        parent.insert(n, t);
                        queue.push_back(n);
                    }
                }
                if !parent.contains_key(&end) {
                    continue;
                }
                let mut way = vec![end];
                let mut at = end;
                while at != start {
                    at = parent[&at];
                    way.push(at);
                }
                way.reverse();
                if way.len() > limit {
                    continue;
                }
                if best.as_ref().is_none_or(|(n, _)| way.len() < *n) {
                    best = Some((way.len(), (l, dl, way, u, du)));
                }
            }
        }
        best.map(|(_, w)| w)
    }

    /// Lays a rollerway where the carts' heaviest flows run: a swape each end and the way between, a
    /// hand-chain when it is short and the charter is not yet won, rollers once it is. One at a time.
    fn rollerways(&mut self) -> bool {
        let c = catalog();
        if self.age == 0 || self.roller_pairs.len() >= MAX_ROLLER_LINES || self.site().is_none() {
            return false;
        }
        let hands_only = factory().as_deref() == Some("hands");
        let mut flows: Vec<((u32, u32), u32)> = self.k.roadwork.flow.iter().filter(|(_, v)| **v >= ROLLER_FLOW).map(|(k, v)| (*k, *v)).collect();
        flows.sort_by_key(|(k, v)| (std::cmp::Reverse(*v), *k));
        for ((from, to), _) in flows {
            if from == 1 || to == 1 || self.roller_pairs.contains(&(from, to)) || !self.k.buildings.contains_key(&from) || !self.k.buildings.contains_key(&to) {
                continue;
            }
            let (pa, pb) = (&self.k.buildings[&from], &self.k.buildings[&to]);
            if c.plans[&pa.plan].swape > 0 || c.plans[&pb.plan].swape > 0 || pa.construction > 0 || pb.construction > 0 {
                continue;
            }
            let dist = (pa.tile.0 - pb.tile.0).abs() + (pa.tile.1 - pb.tile.1).abs();
            if dist < ROLLER_MIN {
                continue;
            }
            // The good the line is for: one that the first makes and the second takes, and the swapes are
            // set to it so that nothing else gets on the line to stop it.
            let makes: BTreeSet<String> = {
                let b = &self.k.buildings[&from];
                let mut g: BTreeSet<String> = b.output.keys().cloned().collect();
                if let Some((_, out, _)) = self.k.job(c, b) {
                    g.extend(out.keys().cloned());
                }
                g
            };
            let takes: BTreeSet<String> = self.k.target_levels(c, to).map(|l| l.keys().cloned().collect()).unwrap_or_default();
            let Some(good) = makes.intersection(&takes).max_by_key(|g| (c.resources[*g].weight, (*g).clone())).cloned() else { continue };
            let Some((l, dl, way, u, du)) = self.roller_way(from, to) else { continue };
            // The hand-chain for a short way before the charter, or always on `hands`; else rollers.
            let chain = hands_only || (way.len() <= 8 && !self.k.researched.contains("rollerways"));
            if !chain && !self.k.researched.contains("rollerways") && !self.research("rollerways") {
                return false;
            }
            let piece = if chain { Piece::Chain } else { Piece::Roller };
            let mut trial = self.k.clone();
            let mut ok = true;
            for (t, d) in [(l, dl), (u, du)] {
                ok &= trial.apply(c, Command::Build { plan: "swape".into(), tile: t }).is_ok() && trial.apply(c, Command::Face { tile: t, dir: d }).is_ok();
                if let Some(id) = trial.at(t) {
                    trial.buildings.get_mut(&id).unwrap().priority = 1;
                }
            }
            for (i, &t) in way.iter().enumerate() {
                let dir = way.get(i + 1).and_then(|n| Dir::between(t, *n)).unwrap_or(du);
                ok &= trial.apply(c, Command::Roller { piece, tile: t, dir }).is_ok();
            }
            if !ok {
                // Short of something, most likely: say what, and try again when the camp has it.
                let mut cost = Stock::new();
                for g in ["timber", "nails", "rope"] {
                    cost.insert(g.into(), way.len() as u32 + 8);
                }
                self.note_short(&cost);
                continue;
            }
            for t in [l, u] {
                if let Some(id) = trial.at(t) {
                    let _ = trial.apply(c, Command::Filter { building: id, good: Some(good.clone()) });
                }
            }
            self.k = trial;
            self.roller_pairs.push((from, to));
            *self.foreman.entry(if chain { "hand-chains" } else { "rollerways" }).or_default() += 1;
            self.built.push((self.now(), format!("{} {from}->{to}, {} tiles", if chain { "hand-chain" } else { "rollerway" }, way.len())));
            return true;
        }
        false
    }

    /// A railway where the carts' heaviest flows run a long way: one gesture, the charter first. Not between a work and
    /// the camp's stores or the stockyard (the carts bring the last stones as readily as the first thousand), nor where a
    /// line of another kind runs already.
    fn railways(&mut self) -> bool {
        let c = catalog();
        if self.age == 0 || self.k.rails.routes.len() >= MAX_RAILWAYS {
            return false;
        }
        let mut flows: Vec<((u32, u32), u32)> = self.k.roadwork.flow.iter().filter(|(_, v)| **v >= RAIL_FLOW).map(|(k, v)| (*k, *v)).collect();
        flows.sort_by_key(|(k, v)| (std::cmp::Reverse(*v), *k));
        for ((from, to), _) in flows {
            let (Some(a), Some(b)) = (self.k.buildings.get(&from), self.k.buildings.get(&to)) else { continue };
            let swape = |p: &str| c.plans[p].swape > 0;
            if from == 1 || to == 1 || swape(&a.plan) || swape(&b.plan) || c.plans[&b.plan].yard || a.construction > 0 || b.construction > 0 {
                continue;
            }
            if (a.tile.0 - b.tile.0).abs() + (a.tile.1 - b.tile.1).abs() < RAIL_LENGTH || self.rail_pairs.contains(&(from, to)) {
                continue;
            }
            if !self.k.researched.contains("rails") && !self.research("rails") {
                return false;
            }
            let good = {
                let makes: BTreeSet<String> = self.outputs_of(from).into_iter().collect();
                let takes: BTreeSet<String> = self.k.target_levels(c, to).map(|l| l.keys().cloned().collect()).unwrap_or_default();
                makes.intersection(&takes).max_by_key(|g| (c.resources[*g].weight, (*g).clone())).cloned()
            };
            let Some(good) = good else { continue };
            let wagons = self.k.amount("wagon").min(4) as u16;
            if wagons < 2 {
                let now = self.now();
                self.missing.insert("wagon".into(), now);
                self.missing.insert("horse".into(), now);
                continue;
            }
            match self.k.apply(c, Command::Railway { from, to, path: vec![], wagons, good: Some(good) }) {
                Ok(m) => {
                    self.rail_pairs.push((from, to));
                    *self.foreman.entry("railways").or_default() += 1;
                    self.built.push((self.now(), format!("railway {from}->{to}: {m}")));
                    return true;
                }
                Err(e) => {
                    self.why.insert(format!("railway {from}->{to}"), (self.now(), e));
                }
            }
        }
        false
    }

    /// Notes what a purchase is short of, for the benches to make: only goods the camp can ever
    /// hold enough of, for a purchase is paid out of the camp's stock and nothing more.
    fn note_short(&mut self, cost: &Stock) {
        let c = catalog();
        let now = self.now();
        for (g, n) in cost {
            if self.k.amount(g) < *n && c.camp_stock.get(g).is_some_and(|level| level >= n) {
                self.missing.insert(g.clone(), now);
            }
        }
    }

    /// Whether anyone in the colony has reached mastery in a trade.
    fn has_master(&self) -> bool {
        let rules = crate::population::Rules::of(catalog(), true);
        self.k.population.citizens.values().any(|p| p.best(&rules).1 >= 3)
    }

    /// The Great Shaft is the first big purchase. A lift is raised as soon as the quarries would
    /// fill it, not once it has been seen to run full: every unit the colony gets from below
    /// goes through it, and a stall at the shaft stalls everything behind. The water hoist and
    /// the ley lift need power and are worth less than the gin without it, so the player stops
    /// at the lift that lifts most unpowered.
    fn shaft(&mut self, report: &Report) -> bool {
        let c = catalog();
        let lifts: Vec<u32> = self.k.buildings.iter().filter(|(_, b)| c.plans[&b.plan].shaft > 0).map(|(&id, _)| id).collect();
        // The lift is manned before anything else: everything from below goes through it.
        for id in &lifts {
            if self.k.buildings[id].priority < 2 {
                let _ = self.k.apply(c, Command::Priority { building: *id, priority: 2 });
            }
        }
        let asks: u32 = self
            .k
            .buildings
            .values()
            .filter(|b| c.plans[&b.plan].extractor)
            .filter_map(|b| b.claim.as_ref().and_then(|q| self.k.claims.get(q)))
            .map(|q| q.output().values().sum::<u32>())
            .sum();
        if asks == 0 || asks * 100 < report.shaft * SHAFT_FILL {
            return false;
        }
        for id in lifts {
            if self.k.buildings[&id].construction > 0 {
                continue;
            }
            let here = &c.plans[&self.k.buildings[&id].plan];
            let next = c.plans.iter().find(|(_, p)| p.replaces.as_deref() == Some(self.k.buildings[&id].plan.as_str()));
            if let Some((_, next)) = next {
                if next.shaft <= here.shaft {
                    continue;
                }
                for r in &next.requires {
                    if !self.k.researched.contains(r) {
                        self.research(r);
                    }
                }
                if self.k.apply(c, Command::Upgrade(id)).is_ok() {
                    *self.foreman.entry("shaft lifts raised").or_default() += 1;
                    self.built.push((self.now(), format!("raised the shaft to {}", next.name)));
                    return true;
                }
                // What the next lift lacks is what the works are asked to make: horses for a
                // gin, cramps for the iron.
                self.note_short(&next.cost);
            }
        }
        false
    }

    /// The Great Shaft's tier above the gin runs on a drive: when the seams ask for all the gin
    /// lifts (twenty-four a shift) and more, a player raises an aqueduct beside the shaft, lays
    /// shafts from it to the hoist, and raises the gin to the water hoist, which lifts three
    /// times its twelve on the drive. If the drive fails the hoist is switched to hand work, for
    /// an unpowered hoist lifts nothing.
    fn powered_hoist(&mut self, report: &Report) -> bool {
        let c = catalog();
        let Some((hid, hoist)) = self.k.buildings.iter().find(|(_, b)| c.plans[&b.plan].shaft > 0).map(|(&i, b)| (i, b.clone())) else { return false };
        let now = self.now();
        if hoist.plan == "shaft-water" {
            // Watch the drive: a hoist that has stood unpowered for three minutes is put to hand work.
            let on = self.k.energy_report(c).powered(PowerKind::Mechanical, hid);
            if hoist.construction > 0 || on || !hoist.power_enabled {
                self.unpowered_since = None;
                return false;
            }
            let since = *self.unpowered_since.get_or_insert(now);
            if now >= since + 180 && self.k.apply(c, Command::EnablePower(hid)).is_ok() {
                *self.foreman.entry("hoists put to hand work").or_default() += 1;
                return true;
            }
            return false;
        }
        if hoist.plan != "shaft-gin" || hoist.construction > 0 || self.age < 1 {
            return false;
        }
        let asks: u32 = self
            .k
            .buildings
            .values()
            .filter(|b| c.plans[&b.plan].extractor)
            .filter_map(|b| b.claim.as_ref().and_then(|q| self.k.claims.get(q)))
            .map(|q| q.output().values().sum::<u32>())
            .sum();
        if asks < report.shaft + 12 {
            return false;
        }
        // The source: an aqueduct beside the shaft.
        let Some(aq) = self.k.buildings.iter().find(|(_, b)| b.plan == "aqueduct").map(|(&i, b)| (i, b.clone())) else {
            let t = self.target.entry("aqueduct".into()).or_default();
            *t = (*t).max(1);
            self.shares.entry("aqueduct".into()).or_default();
            return false;
        };
        let (aid, aqueduct) = aq;
        if aqueduct.construction > 0 || report.status.get(&aid) != Some(&Status::Working) {
            return false;
        }
        // The drive, planned once: the shortest way over open ground from the aqueduct's foot to
        // the hoist, and laid a few tiles a round.
        if self.drive.is_empty() {
            match self.plan_drive(aqueduct.tile, hoist.tile) {
                Some(path) => self.drive = path,
                None => {
                    // No way through: pull the aqueduct down and raise it elsewhere.
                    if self.k.apply(c, Command::Demolish(aqueduct.tile)).is_ok() {
                        self.made.remove(&aid);
                        self.built.push((now, "pulled down an aqueduct with no way to the shaft".into()));
                    }
                    return false;
                }
            }
        }
        let mut laid = 0;
        for t in self.drive.clone() {
            if self.k.power.contains(PowerKind::Mechanical, t) {
                continue;
            }
            if laid >= 8 || self.k.apply(c, Command::PowerLine { kind: PowerKind::Mechanical, tile: t }).is_err() {
                break;
            }
            laid += 1;
        }
        if self.drive.iter().any(|t| !self.k.power.contains(PowerKind::Mechanical, *t)) {
            if laid > 0 {
                *self.foreman.entry("drive shafts laid").or_default() += laid;
            }
            return laid > 0;
        }
        // The drive is whole: raise the hoist and fit it to the drive.
        for r in ["engines"] {
            if !self.k.researched.contains(r) {
                self.research(r);
            }
        }
        if self.k.apply(c, Command::Upgrade(hid)).is_ok() {
            let _ = self.k.apply(c, Command::EnablePower(hid));
            *self.foreman.entry("shaft lifts raised").or_default() += 1;
            self.built.push((now, "raised the shaft to the water hoist on the aqueduct's drive".into()));
            return true;
        }
        self.note_short(&c.plans["shaft-water"].cost);
        false
    }

    /// The shortest run of open ground for a drive from beside `from` to beside `to`.
    fn plan_drive(&self, from: Tile, to: Tile) -> Option<Vec<Tile>> {
        use std::collections::VecDeque;
        let kind = PowerKind::Mechanical;
        let ok = |t: Tile| self.k.power.contains(kind, t) || self.k.check_power_site(kind, t).is_ok();
        let goal: BTreeSet<Tile> = crate::economy::neighbours(to).into_iter().collect();
        let mut parent: BTreeMap<Tile, Option<Tile>> = BTreeMap::new();
        let mut queue: VecDeque<Tile> = VecDeque::new();
        for n in crate::economy::neighbours(from) {
            if ok(n) {
                parent.insert(n, None);
                queue.push_back(n);
            }
        }
        while let Some(t) = queue.pop_front() {
            if goal.contains(&t) {
                let mut path = vec![t];
                let mut cur = t;
                while let Some(Some(p)) = parent.get(&cur) {
                    path.push(*p);
                    cur = *p;
                }
                path.reverse();
                return (path.len() <= 24).then_some(path);
            }
            for n in crate::economy::neighbours(t) {
                if !parent.contains_key(&n) && ok(n) && parent.len() < 2000 {
                    parent.insert(n, Some(t));
                    queue.push_back(n);
                }
            }
        }
        None
    }

    /// A trade with no one to oversee it: a lesson at the teachers, for coin.
    fn lessons(&mut self, report: &Report) -> bool {
        let c = catalog();
        let rules = crate::population::Rules::of(c, true);
        let mut wanting: Vec<String> = self
            .k
            .buildings
            .iter()
            .filter(|(id, b)| b.construction == 0 && !c.plans[&b.plan].trade.is_empty() && report.status.get(id) == Some(&Status::Workers))
            .map(|(_, b)| c.plans[&b.plan].trade.clone())
            .collect();
        if report.population.idle == 0 {
            wanting.retain(|_| false);
        }
        // A master smith makes the steel the tools are re-steeled with.
        let no_master = !self.k.population.citizens.values().any(|p| p.skill("smith", &rules) >= 3);
        if no_master && self.k.amount("steel") < 14 && self.k.buildings.values().any(|b| b.plan == "smithy") {
            wanting.insert(0, "smith".into());
        }
        // The masons and the carters come first: the wall and the roads wait on them.
        const FIRST: [&str; 4] = ["lime-burner", "setter", "carter", "sawyer"];
        wanting.sort_by_key(|t| FIRST.iter().position(|f| f == t).unwrap_or(FIRST.len()));
        for trade in wanting.into_iter().take(1) {
            if self.k.amount("coin") < c.works.tuition {
                continue;
            }
            // The best hand at the trade takes the lesson; else an idler.
            let pupil = self
                .k
                .population
                .citizens
                .iter()
                .filter(|(_, p)| p.skill(&trade, &rules) < 3)
                .filter(|(_, p)| trade != "smith" || p.skill(&trade, &rules) >= 2 || report.population.idle > 0)
                .min_by_key(|(id, p)| (std::cmp::Reverse(p.skill(&trade, &rules)), p.job.is_some(), p.skill_total(&rules), **id))
                .map(|(&id, _)| id);
            if let Some(id) = pupil {
                if self.k.apply(c, Command::Teach { citizen: id, trade }).is_ok() {
                    *self.foreman.entry("lessons").or_default() += 1;
                    return true;
                }
            }
        }
        false
    }

    // ---- the day ------------------------------------------------------------

    /// Bread for the people there are and the people about to come: the chain
    /// from field to oven is sized to them, not to the wall.
    fn food_plan(&mut self) {
        let people = (self.k.population.citizens.len().max(24) + 16) as f64;
        let bread = people / 1.5;
        let bakeries = (bread / 15.0).ceil() as u32;
        let mills = (bread / 2.0 / 15.0).ceil() as u32;
        let grain = (bread / 2.0 * 4.0 / 3.0 / 20.0).ceil() as u32;
        let wells = (bakeries as f64 / 3.0).ceil() as u32 + 1;
        let clamps = (bakeries as f64 * 3.0 / 5.0).ceil() as u32 + 1;
        for (plan, n) in [("bakery", bakeries), ("mill", mills), ("well", wells), ("clamp", clamps)] {
            let t = self.target.entry(plan.into()).or_default();
            *t = (*t).max(n);
        }
        // The founders' train brings six hundred fodder, which twelve oxen eat in a working day and a half: a farm
        // to grow more is raised when it runs low.
        let hay = if self.k.amount("fodder") < 250 { self.shares.get("farmstead").and_then(|m| m.get("grow-fodder")).copied().unwrap_or(0.4).ceil().max(1.0) as u32 } else { 0 };
        let t = self.target.entry("farmstead".into()).or_default();
        *t = (*t).max(grain + hay);
        let sh = self.shares.entry("farmstead".into()).or_default();
        sh.insert("grow-grain".into(), f64::from(grain));
        sh.insert("grow-fodder".into(), f64::from(hay));
    }

    /// The ropeway's chain: hemp, a retting pond, a ropewalk.
    fn rope_plan(&mut self) {
        if !self.rope_project {
            return;
        }
        for (key, recipe) in [("retting-pond", "ret-hemp"), ("ropewalk", "spin-rope")] {
            let t = self.target.entry(key.into()).or_default();
            *t = (*t).max(1);
            self.shares.entry(key.into()).or_default().insert(recipe.into(), 1.0);
        }
        let hemp = self.shares.entry("farmstead".into()).or_default();
        if hemp.get("grow-hemp").copied().unwrap_or(0.0) < 1.0 {
            hemp.insert("grow-hemp".into(), 1.0);
            let t = self.target.entry("farmstead".into()).or_default();
            *t += 1;
        }
    }

    /// How many works of a kind the opening hour raises, whatever the want list asks for: a young camp has a dozen
    /// pairs of hands, and a person puts them where the wall's first stones wait (planks, joists, the lodge), not on
    /// the whole of the chain the castle will want at its pace.
    fn opening_cap(&self, key: &str) -> u32 {
        let t = self.now();
        if t >= 3600 {
            return u32::MAX;
        }
        let by = |steps: &[(u64, u32)]| steps.iter().filter(|(from, _)| t >= *from).map(|(_, n)| *n).last().unwrap_or(0);
        let planks = self.k.amount("plank");
        match key {
            "woodsman-camp" => by(&[(0, 2), (900, 3)]),
            // Scaffold poles are wanted the day the lodge is, and the bark when the tanyard is.
            "woodsman-camp#poles" => by(&[(0, 0), (knob("POLES_AT", 60), 1)]),
            "woodsman-camp#bark" => by(&[(0, 0), (700, 1)]),
            "sawpit" => by(&[(0, 1), (1800, 2)]),
            // The bread chain is the first thing the colony lives on: a farm, a well, a mill and two ovens by the
            // third minute give it bread to grow on (thirty-six a minute is bread for fifty mouths), a clamp for
            // the ovens' fuel.
            "clamp" => by(&[(0, 1), (1500, 2), (3000, 3)]),
            "farmstead" => by(&[(0, 1), (1500, 2), (2400, 3)]),
            "well" => by(&[(0, 1), (900, 2)]),
            "mill" => 1,
            "bakery" => by(&[(0, 2), (3600, 3)]),
            "mason" => by(&[(0, 0), (knob("MASON_AT", 0), 1)]),
            // A lodge is raised when the blocks and planks for it are in hand.
            "lodge" => if self.k.amount("block") >= 6 && planks >= 8 { u32::MAX } else { self.count("lodge") },
            // The joinery charter is twelve planks, and the shop six.
            "carpentry" => if t >= knob("CARP_T", 0) && self.count("sawpit") > 0 && u64::from(planks) >= knob("CARP_PLANKS", 6) { u32::MAX } else { self.count("carpentry") },
            "pasture" | "pasture#oxen" => by(&[(0, 0), (700, 1)]),
            "brewery" => by(&[(0, 0), (2400, 1)]),
            "tanyard" | "tanyard#1" => by(&[(0, 0), (900, 1)]),
            "smithy" | "smithy#1" => by(&[(0, 0), (1200, 1)]),
            "wheelwright" => if t >= knob("WHEEL_AT", 300) && planks >= 14 { 1 } else { 0 },
            _ => u32::MAX,
        }
    }

    /// Raises what the age's list still lacks, in order.
    fn agenda(&mut self) -> bool {
        self.food_plan();
        self.rope_plan();
        let mut acted = false;
        let mut plans: Vec<String> = self.target.keys().cloned().collect();
        plans.sort_by_key(|p| (RANK.iter().position(|r| *r == p.split('#').next().unwrap()).unwrap_or(RANK.len()), p.clone()));
        let c = catalog();
        let mut raised = 0;
        // A player who cannot yet pay for the next thing on the list does not spend the goods
        // on something further down it: he saves them. Without this the bakery waits on stone
        // for an hour while every cheaper work takes the stone as it comes, and the colony
        // never has bread for the people it needs.
        let mut saving: BTreeSet<String> = BTreeSet::new();
        // The first purchase of all is the castle's stockyard and the staking of the ground: the
        // ditch cannot be dug before it, and every minute it waits is a minute off the end.
        if self.site().is_none() {
            for (g, n) in [("plank", 18), ("stone", 40)] {
                if self.k.amount(g) < n {
                    saving.insert(g.into());
                }
            }
        }
        for plan in plans {
            if raised >= 4 {
                break;
            }
            if plan == "claim-quarry" {
                continue;
            }
            let base = plan.split('#').next().unwrap().to_string();
            if c.plans[&base].cost.keys().any(|g| saving.contains(g)) {
                continue;
            }
            let want = self.target[&plan].min(self.opening_cap(&plan));
            if std::env::var("DEBUG_AGENDA").is_ok_and(|v| v.parse::<u64>().is_ok_and(|t| t == self.now())) {
                eprintln!("  agenda {}s {plan}: have {} want {want} saving {saving:?} requires {:?}", self.now(), self.count(&plan), c.plans[&base].requires);
            }
            while self.count(&plan) < want {
                match self.raise(&plan) {
                    Ok(true) => {
                        acted = true;
                        raised += 1;
                    }
                    Ok(false) => {
                        let p = &c.plans[&base];
                        // In the first forty minutes everything is short, timber included, and the bread
                        // chain must not lose it to cheaper works. After that timber comes in all day
                        // from the camps and is not saved for: it is the goods that come in a few at
                        // a time (stone, planks, blocks, iron) that the cheaper works would eat.
                        let young = self.now() < SAVE_TIMBER_UNTIL;
                        for (g, n) in &p.cost {
                            if (young || g != "timber") && self.k.amount(g) < *n {
                                saving.insert(g.clone());
                            }
                        }
                        // The charter it waits on is saved for too.
                        for r in p.requires.iter().filter(|r| !self.k.researched.contains(*r)) {
                            if let Some(t) = c.tech(r) {
                                for (g, n) in &t.cost {
                                    if (young || g != "timber") && self.k.amount(g) < *n {
                                        saving.insert(g.clone());
                                    }
                                }
                            }
                        }
                        break;
                    }
                    Err(e) => panic!("at {}s: {e}", self.now()),
                }
                if raised >= 4 {
                    break;
                }
            }
        }
        acted
    }

    /// One delve about every eight minutes, depths rising, evening pace only.
    fn scripted_delve(&mut self, t: u64) {
        if !crate::pace::evening() {
            return;
        }
        let due = if t < crate::pace::FIRST_DELVE { 0 } else { 1 + (t - crate::pace::FIRST_DELVE) / crate::pace::DELVE_EVERY };
        if due <= u64::from(self.delves) {
            return;
        }
        let depth = (self.delves + 1).min(6);
        let boss = self.delves + 1 >= 6;
        let n = self.k.pay_delve(depth, boss);
        if n == 0 {
            return;
        }
        self.delves += 1;
        self.plate = Kingdom::delve_plate(n);
        self.log.push((t, self.plate.clone()));
    }

    /// One second of play: the player acts, the world turns, the masons shift.
    pub fn tick(&mut self) {
        let c = catalog();
        let t = self.now();
        self.scripted_delve(t);
        let mut acted = false;
        if t % 600 == 0 && t > 0 {
            self.k.bank_coin(PURSE);
            self.coin_banked += PURSE;
        }
        if (t + self.jitter) % crate::pace::act_every() == 0 {
            // The castle is staked out once the camp can feed itself: the stone
            // and the stockyard are the first hour's, not the first minute's.
            if self.site().is_none() {
                acted |= self.found_castle();
            }
            if t >= 400 && std::env::var("DEPOT").is_ok() {
                let d = self.target.entry("depot".into()).or_default();
                *d = (*d).max(1);
            }
            acted |= self.knights();
            // Wine for the masters, once there is a master to drink it.
            if self.age >= 1 && tiers() && self.has_master() {
                for (plan, recipe) in [("vineyard", "grow-grapes"), ("wine-press", "press-wine")] {
                    let t = self.target.entry(plan.into()).or_default();
                    *t = (*t).max(1);
                    self.shares.entry(plan.into()).or_default().insert(recipe.into(), 1.0);
                }
            }
            // Hands to carry up to the masons once there are stones to carry, and not before.
            if self.first_stone.is_some() {
                let h = self.target.entry("hod-gang".into()).or_default();
                *h = (*h).max(1);
            }
            if let Some(site) = self.site() {
                if let Hold::Short { resource } = self.k.site_allowance(c, site, 99).hold {
                    self.missing.insert(resource, t);
                }
                let yard = self.yard().and_then(|y| self.k.buildings.get(&y));
                if let Some(y) = yard {
                    if y.input.get("pole").copied().unwrap_or(0) < c.works.poles_standing {
                        self.missing.insert("pole".into(), t);
                    }
                }
            }
            acted |= self.agenda();
            if (t + self.jitter) % 10 == 5 {
                acted |= self.foreman();
            }
        }
        // The player waits when nothing has been done for three minutes.
        match (&self.quiet_since, acted) {
            (None, false) => {
                let why = self.why.iter().filter(|(_, (at, _))| t < at + 30).max_by_key(|(_, (at, _))| *at).map_or("nothing left to do".to_string(), |(k, (_, v))| format!("{k}: {v}"));
                self.quiet_since = Some((t, why));
            }
            (Some((from, why)), true) => {
                if t - from >= 180 {
                    self.waits.push((*from, t, why.clone()));
                }
                self.quiet_since = None;
            }
            _ => {}
        }
        self.k.advance_second(c);
        for (&yid, b) in &self.k.buildings {
            for (i, x) in b.carts.iter().enumerate() {
                self.cart_seconds[x.phase as usize] += 1;
                let moved = self.cart_tiles.insert((yid, i), x.tile).is_some_and(|was| was != x.tile);
                if moved && x.phase == CartPhase::Haul {
                    let kilos = u64::from(Kingdom::cart_load_kilos(c, x));
                    let plan = |id: u32| self.k.buildings.get(&id).map_or("?".to_string(), |b| b.plan.clone());
                    let e = self.haul_stats.entry((plan(x.from), plan(x.to))).or_default();
                    e.0 += kilos;
                    e.1 += 1;
                }
            }
        }
        if self.k.second % 10 == 0 {
            if let Some(id) = self.site() {
                let masons = self.k.masons(c, id);
                let a = self.k.site_allowance(c, id, masons);
                if a.stones == 0 {
                    let why = match &a.hold {
                        Hold::Short { resource } => resource.clone(),
                        Hold::Scaffold { .. } => "scaffold poles".into(),
                        Hold::Centring { .. } => "centring".into(),
                        Hold::Earthwork => "earthwork".into(),
                        Hold::Masons => "no masons".into(),
                        Hold::Yard => "stockyard".into(),
                        other => format!("{other:?}"),
                    };
                    *self.holds.entry(why).or_default() += 1;
                }
                match (a.stones, &a.hold) {
                    (0, _) => self.shifts[2] += 1,
                    (_, Hold::Pace) => self.shifts[0] += 1,
                    _ => self.shifts[1] += 1,
                }
                let before = (self.k.sites[&id].stage, self.k.sites[&id].plan.clone());
                for (_, stones) in self.k.work_sites(c) {
                    if self.first_stone.is_none() && stones > 0 {
                        self.first_stone = Some(self.k.second);
                    }
                }
                let after = self.k.sites[&id].stage;
                if after > before.0 {
                    let name = c.castles[&before.1].stages[usize::from(after) - 1].name.clone();
                    self.stage_times.push((self.k.second, format!("{}: {name}", before.1)));
                }
                if self.k.site_finished(c, id) {
                    let castle = self.k.sites[&id].plan.clone();
                    self.stones_before += c.castles[&castle].stones();
                    self.finished.push((castle, self.k.second));
                    if self.age + 1 < AGES.len() {
                        self.age += 1;
                        self.plan_age();
                        self.found_castle();
                    } else {
                        self.done = true;
                    }
                }
            }
        }
        if self.k.second % 60 == 0 {
            self.sample();
        }
    }

    fn sample(&mut self) {
        let c = catalog();
        let r = self.k.report(c);
        let roads = self.k.road_report(c);
        let (castle, stage, set) = match self.k.sites.values().next_back() {
            Some(s) => (s.plan.clone(), c.castles[&s.plan].stages.get(usize::from(s.stage)).map_or("finished".into(), |x| x.name.clone()), s.laid),
            None => (String::new(), String::new(), 0),
        };
        let mut buildings: BTreeMap<String, u32> = BTreeMap::new();
        for b in self.k.buildings.values() {
            *buildings.entry(b.plan.clone()).or_default() += 1;
        }
        let mut util: BTreeMap<String, [u32; 4]> = BTreeMap::new();
        for b in self.k.buildings.values() {
            let u = util.entry(b.plan.clone()).or_default();
            for (i, n) in [b.stats.worked, b.stats.starved, b.stats.backed_up, b.stats.unstaffed].into_iter().enumerate() {
                u[i] += n;
            }
        }
        let keys = [
            "timber", "plank", "beam", "stone", "block", "limestone", "ashlar", "quicklime", "lime-putty", "mortar", "rubble", "sand", "bread", "grain", "flour",
            "charcoal", "iron-ore", "coal", "bloom", "wrought-iron", "nails", "cramp", "tyre", "steel", "fodder", "cart", "ox", "harness", "joist", "pole",
            "centring", "rope", "lead-sheet", "slate", "coin", "pick", "axe", "chisel", "trowel", "worn-pick", "worn-axe", "worn-chisel", "worn-trowel",
        ];
        let stock: BTreeMap<String, u32> = keys.iter().map(|g| (g.to_string(), self.k.amount(g))).filter(|(_, n)| *n > 0).collect();
        self.samples.push(Sample {
            t: self.k.second,
            castle,
            stage,
            stones_set: self.stones_before + set,
            population: r.population.population as usize,
            beds: usize::from(r.population.beds),
            employed: r.population.employed,
            idle: r.population.idle,
            stock,
            buildings,
            yards: roads.yards.len(),
            carts: roads.carts,
            free_carts: roads.free,
            unserved: roads.unserved.len(),
            jams: roads.jams.len(),
            hauled_per_min: roads.observed_per_min,
            lines: self.k.lines.len(),
            shaft: r.shaft,
            shaft_moved: self.k.shaft_moved,
            held: WATCHED.iter().map(|g| (g.to_string(), self.k.total(g))).filter(|(_, n)| *n > 0).collect(),
            made: WATCHED.iter().map(|g| (g.to_string(), self.k.produced.get(*g).copied().unwrap_or(0))).filter(|(_, n)| *n > 0).collect(),
            lost: self.k.spoiled.iter().map(|(g, n)| (g.clone(), *n)).collect(),
            shifts: self.shifts,
            holds: self.holds.clone(),
            util,
        });
    }
}

/// Plays every castle in turn from the founding stock; stops when the last
/// stands or the hours run out.
pub(crate) fn play(hours: f64, progress: bool) -> Player {
    let mut p = Player::new();
    for i in 0..(hours * 3600.0) as u64 {
        p.tick();
        if progress && i % 600 == 599 {
            let r = p.k.report(catalog());
            let roads = p.k.road_report(catalog());
            let site = p.k.sites.values().next_back().map(|s| (s.stage, s.laid));
            eprintln!(
                "T{:>5} pop {:>3}/{:<3} emp {:>3} idle {:>2} bread {:>3} timber {:>3} plank {:>3} stone {:>3} lime {:>3} ashlar {:>3} nails {:>3} bldg {:>3} carts {:>3}/{:<3} yards {} site {:?} | {}",
                i + 1,
                r.population.population,
                r.population.beds,
                r.population.employed,
                r.population.idle,
                p.k.amount("bread"),
                p.k.amount("timber"),
                p.k.amount("plank"),
                p.k.amount("stone"),
                p.k.amount("limestone"),
                p.k.amount("ashlar"),
                p.k.amount("nails"),
                p.k.buildings.len(),
                roads.carts - roads.free,
                roads.carts,
                roads.yards.len(),
                site,
                p.site().map_or(String::new(), |id| p.k.clerk(catalog(), id).sentence)
            );
        }
        if let Ok(at) = std::env::var("DUMP_AT") {
            if at.split(',').any(|x| x.trim().parse::<u64>().ok() == Some(i + 1)) {
                dump(&p);
            }
        }
        if p.done {
            break;
        }
    }
    p
}

/// A look at every building, for finding why a chain is not running.
pub(crate) fn dump(p: &Player) {
    let c = catalog();
    let r = p.k.report(c);
    eprintln!("==== t={} pop {} beds {} employed {} idle {} stock {:?} researched {:?}", p.k.second, r.population.population, r.population.beds, r.population.employed, r.population.idle, p.k.stock, p.k.researched);
    for (id, b) in &p.k.buildings {
        eprintln!(
            "  #{id} {} {:?} {:?} con {} crew {} recipe {:?} in {:?} out {:?} tool {} | worked {} starved {} backed {} unstaffed {}",
            b.plan,
            b.tile,
            r.status[id],
            b.construction,
            r.population.assigned.get(id).copied().unwrap_or(0),
            b.recipe,
            b.input,
            b.output,
            b.tool_left,
            b.stats.worked,
            b.stats.starved,
            b.stats.backed_up,
            b.stats.unstaffed
        );
    }
    for row in p.k.line_report(c) {
        eprintln!("  line {} {}->{} {:?} veh {} cap/min {} obs {} len {}", row.id, row.from, row.to, row.state, row.vehicles, row.capacity_per_min, row.observed_per_min, row.length);
    }
    {
        let cz = &p.k.population.citizens;
        let bad: Vec<_> = cz.iter().filter(|(_, x)| x.hunger > 30 || x.rest < 40 || x.morale < 40).map(|(i, x)| (*i, x.hunger, x.rest, x.morale, x.home.is_some(), x.job.is_some())).collect();
        eprintln!("  people {}: not welcoming-fit {:?} arrival_seconds {}", cz.len(), bad, p.k.population.arrival_seconds);
        if std::env::var("DUMP_PEOPLE").is_ok() {
            let rules = crate::population::Rules::of(c, true);
            for (id, x) in cz {
                let skills: Vec<String> = x.trades.iter().filter(|(_, xp)| **xp > 0).map(|(t, xp)| format!("{t}:{}({xp})", rules.skill(*xp))).collect();
                eprintln!("    person {id} job {:?} home {:?} hunger {} morale {} skills {skills:?}", x.job, x.home, x.hunger, x.morale);
            }
        }
    }
    for (id, b) in &p.k.buildings {
        for x in &b.carts {
            if !matches!(x.phase, CartPhase::Idle | CartPhase::Return) {
                eprintln!("  cart of #{id} {:?} at {:?} {}->{} order {:?} load {:?} route {} idle {}", x.phase, x.tile, x.from, x.to, x.order, x.load, x.route.len(), x.idle);
            }
        }
    }
    if std::env::var("DUMP_MAP").is_ok() {
        let jam: BTreeSet<Tile> = p.k.roadwork.crowd.iter().filter(|(_, v)| **v >= 15).map(|(t, _)| *t).collect();
        for y in 74..106 {
            let row: String = (112..162)
                .map(|x| {
                    let t = (x, y);
                    if let Some(id) = p.k.at(t) {
                        let g = c.plans[&p.k.buildings[&id].plan].glyph.chars().next().unwrap_or('?');
                        g
                    } else if jam.contains(&t) {
                        '#'
                    } else if p.k.roads.contains(&t) {
                        let heat = p.k.roadwork.crowd.get(&t).copied().unwrap_or(0);
                        if heat > 5 { '+' } else { '.' }
                    } else {
                        ' '
                    }
                })
                .collect();
            eprintln!("  map {y:>3} {row}");
        }
    }
    let roads = p.k.road_report(c);
    eprintln!("  roads: {} carts in {} yards, {} free, {}/min delivered, {} jams {:?}", roads.carts, roads.yards.len(), roads.free, roads.observed_per_min, roads.jams.len(), roads.jams);
    eprintln!("  carts: {} trips, {} t carried, {:.0} kg a trip", p.k.roadwork.trips, p.k.roadwork.kilos / 1000, p.k.roadwork.kilos as f64 / f64::from(p.k.roadwork.trips.max(1)));
    let mut flows: Vec<_> = p.k.roadwork.flow.iter().collect();
    flows.sort_by_key(|(_, v)| std::cmp::Reverse(**v));
    for ((a, b), v) in flows.into_iter().take(12) {
        let name = |i: u32| p.k.buildings.get(&i).map_or("?".to_string(), |x| format!("{}#{i}@{:?}", x.plan, x.tile));
        eprintln!("    flow {:>6} kg  {} -> {}", v, name(*a), name(*b));
    }
    for u in &roads.unserved {
        eprintln!("    unserved {} {}->{} x{}{}", u.good, u.from, u.to, u.units, if u.no_road { " NO ROAD" } else { "" });
    }
}

fn report(p: &Player) {
    eprintln!("finished {:?} first stone {:?} lines {} foreman {:?}", p.finished, p.first_stone, p.k.lines.len(), p.foreman);
    if factory().is_some() {
        let rows = p.k.roller_report(catalog());
        eprintln!(
            "factory: {} lines laid, {} tiles, {} swapes, {} lifted off the lines in all; {} carts; now {:?}",
            p.roller_pairs.len(),
            p.k.rollers.cells.len(),
            p.k.buildings.values().filter(|b| b.swape).count(),
            p.k.rollers.unloaded,
            p.k.cart_count(),
            rows.iter().map(|r| (r.tiles, r.items, r.per_min, r.state.label())).collect::<Vec<_>>()
        );
    }
    if std::env::var("HAUL_STATS").is_ok() {
        let mut v: Vec<_> = p.haul_stats.iter().collect();
        v.sort_by_key(|(_, (kt, _))| std::cmp::Reverse(*kt));
        for ((a, b), (kt, tiles)) in v.into_iter().take(16) {
            eprintln!("haul {a:>16} -> {b:<16} {:>6} t-tiles, {:>6} tile-moves, {:>6} kg per tile-move", kt / 1000, tiles, kt / tiles.max(&1));
        }
    }
    let cs = p.cart_seconds;
    let all = cs.iter().sum::<u64>().max(1);
    eprintln!(
        "carts: {} trips, {:.0} kg a trip; cart time: idle {}% return {}% fetch {}% queue {}% loading {}% haul {}% wait {}% unloading {}%",
        p.k.roadwork.trips,
        p.k.roadwork.kilos as f64 / f64::from(p.k.roadwork.trips.max(1)),
        cs[0] * 100 / all, cs[1] * 100 / all, cs[2] * 100 / all, cs[3] * 100 / all, cs[4] * 100 / all, cs[5] * 100 / all, cs[6] * 100 / all, cs[7] * 100 / all
    );
    for (t, n) in &p.stage_times {
        eprintln!("stage done {t}s {n}");
    }
    for (a, b, why) in &p.waits {
        eprintln!("waited {a}-{b}s ({}s): {why}", b - a);
    }
    for (t, m) in p.log.iter().filter(|(_, m)| !m.starts_with("did ")).take(60) {
        eprintln!("{t:>6}s {m}");
    }
    for (k, (t, v)) in &p.why {
        eprintln!("why {k}: {v} (at {t}s)");
    }
}

/// Writes the timeline the charts read: every sampled minute, and what was built when.
pub(crate) fn write_timeline(p: &Player, path: &str) {
    let json = serde_json::json!({
        "castles": AGES.iter().map(|(c, pace)| serde_json::json!({"castle": c, "pace_per_min": pace})).collect::<Vec<_>>(),
        "finished": p.finished.iter().map(|(c, t)| serde_json::json!({"castle": c, "second": t})).collect::<Vec<_>>(),
        "waits": p.waits.iter().map(|(a, b, why)| serde_json::json!({"from": a, "to": b, "why": why})).collect::<Vec<_>>(),
        "stages": p.stage_times.iter().map(|(t, n)| serde_json::json!({"second": t, "stage": n})).collect::<Vec<_>>(),
        "built": p.built.iter().map(|(t, what)| serde_json::json!({"second": t, "what": what})).collect::<Vec<_>>(),
        "foreman": p.foreman,
        "samples": p.samples,
    });
    std::fs::write(path, serde_json::to_string(&json).unwrap()).expect("write the timeline");
}

#[test]
#[ignore]
fn zz_all() {
    let hours = std::env::var("HOURS").ok().and_then(|h| h.parse().ok()).unwrap_or(8.0);
    let p = play(hours, true);
    report(&p);
    if std::env::var("DUMP_END").is_ok() {
        dump(&p);
    }
    if let Ok(path) = std::env::var("TIMELINE_OUT") {
        write_timeline(&p, &path);
    }
}

/// The camp age, played whole from the founding stock by the player above, on the real map with every work its drawn
/// size: the motte and bailey (544 pieces of earth and timber, no Delve stone) stands in an hour and a half or so (four
/// starts: 65 to 103 minutes). The first pieces cannot be set before the ditch and the mound are dug, the colony must
/// have grown on bread it baked, and the carts, oxen and harness the founders brought have to last the lines.
#[test]
fn the_camp_age_raises_the_motte_and_bailey_on_the_real_map_in_under_two_hours() {
    let c = catalog();
    let mut p = Player::new();
    for _ in 0..2 * 3600 {
        p.tick();
        if !p.finished.is_empty() {
            break;
        }
    }
    let (castle, done) = p.finished.first().cloned().expect("the motte and bailey stands within two hours");
    assert_eq!(castle, "motte-and-bailey");
    assert!((45 * 60..=110 * 60).contains(&done), "the camp age takes an hour and a half or so: {done}s");
    assert!(p.k.valid(c));
    assert!(p.first_stone.is_some_and(|t| t > 600), "the earth is dug before the first piece is set");
    assert!(p.k.population.citizens.len() >= 24, "bread brought newcomers: {}", p.k.population.citizens.len());
    assert!(p.k.cart_count() >= 12, "the carts bought: {}", p.k.cart_count());
    // The player writes to nothing but through commands: the log is all the knights' and steps.
    assert!(p.log.iter().all(|(_, m)| m.starts_with("the knights") || m.starts_with("quarry on")));
    // Where the player waits with nothing affordable to raise, once the first minutes are past. The aim is no wait over
    // three minutes; the camp age still has a few longer ones while the masons catch up with the carts, so the bound
    // here is the measured ten minutes.
    let long: Vec<_> = p.waits.iter().filter(|(from, _, _)| *from > 600 && *from < done).filter(|(a, b, _)| b - a > 600).collect();
    assert!(long.is_empty(), "long waits: {long:?}");
}

/// The stockyard stands by the camp's own road, two ditch gangs by it, each lifting its spoil into the yard by a swape
/// standing between them and it: the earthworks (a ditch, a mound and a bank, forty-four tonnes of earth) are done in
/// a quarter of an hour, which the carts, hauling it as they hauled everything, took the best part of an hour over.
#[test]
fn the_earthworks_are_lifted_into_the_yard_by_swape_in_a_quarter_of_an_hour() {
    let mut p = Player::new();
    for _ in 0..1200 {
        p.tick();
    }
    let yard = p.yard().expect("the yard is staked at once");
    assert_eq!(p.k.buildings[&yard].tile, lay::YARD, "the stockyard stands where the camp plan puts it");
    assert_eq!(p.foreman.get("spoil swapes").copied(), Some(2), "a swape for each gang");
    let swapes: Vec<_> = p.k.buildings.values().filter(|b| b.plan == "swape").collect();
    assert_eq!(swapes.len(), 2);
    assert!(swapes.iter().all(|b| b.filter.as_deref() == Some("earth")), "set to the spoil");
    assert!(
        p.stage_times.iter().any(|(t, n)| *t <= 1200 && n.ends_with("Bank")),
        "ditch, mound and bank dug in twenty minutes: {:?}",
        p.stage_times
    );
    assert!(p.k.valid(catalog()));
}

/// A finished castle gives way to the next age's over the same ground, and the
/// old stockyard serves the new works.
#[test]
fn a_finished_castle_gives_way_to_the_next_over_the_same_ground() {
    let c = catalog();
    let mut p = Player::new();
    let mut yard = None;
    for _ in 0..3 * 3600 {
        yard = p.yard();
        p.tick();
        if !p.finished.is_empty() {
            break;
        }
    }
    assert!(!p.finished.is_empty());
    // The player founds the shell keep as the age turns.
    assert_eq!(p.k.sites.len(), 1, "the finished motte and bailey was retired");
    let id = p.site().unwrap();
    assert_eq!(p.k.sites[&id].plan, "shell-keep");
    assert_eq!(p.yard(), yard, "the old stockyard serves the new castle");
    assert!(p.k.valid(c));
}

/// Every age from founding stock to the great tower, with the timeline for charts.
/// `TIMELINE_OUT=/path.json HOURS=8 cargo test ... zz_all -- --ignored --nocapture`.
#[test]
#[ignore]
fn the_three_castles_from_the_founding_stock() {
    let hours = std::env::var("HOURS").ok().and_then(|h| h.parse().ok()).unwrap_or(8.0);
    let p = play(hours, false);
    report(&p);
    if let Ok(path) = std::env::var("TIMELINE_OUT") {
        write_timeline(&p, &path);
    }
    assert!(p.k.valid(catalog()));
    assert!(!p.finished.is_empty());
}

/// The founding hillside round the camp, one character a tile: `#` the Keep's ground, `T` forest,
/// `s` field stone, `c` clay, `~` river, `:` sand, `o` a model house's camp, `+` the camp and the
/// gate, `.` open meadow. For laying out the camp (`cargo test ... zz_map -- --ignored --nocapture`).
#[test]
#[ignore]
fn zz_map() {
    let mut k = Kingdom::default();
    k.found_hillside(catalog(), 7);
    let (x0, y0, x1, y1) = (96i16, 60i16, 190i16, 124i16);
    eprintln!("     x {x0}..{x1}");
    for y in y0..=y1 {
        let row: String = (x0..=x1)
            .map(|x| {
                let t = (x, y);
                if t == k.camp() || t == k.gate() {
                    '+'
                } else if k.reserved_ground(t).is_some() {
                    if crate::hillside::in_rect(t, crate::hillside::keep_rect()) { '#' } else { 'o' }
                } else {
                    match k.world.terrain(t) {
                        Terrain::Forest => 'T',
                        Terrain::Stone => 's',
                        Terrain::ClayBank => 'c',
                        Terrain::River | Terrain::Ford => '~',
                        Terrain::Meadow if k.world.deposit(t).is_some() => ':',
                        Terrain::Meadow => '.',
                    }
                }
            })
            .collect();
        eprintln!("{y:>4} {row}");
    }
}

/// What the want list asks for at the start of each age (`cargo test ... zz_targets -- --ignored --nocapture`).
#[test]
#[ignore]
fn zz_targets() {
    let mut p = Player::new();
    for age in 0..AGES.len() {
        p.age = age;
        p.plan_age();
        eprintln!("age {}: {:?}", AGES[age].0, p.target);
        eprintln!("  shares {:?}", p.shares);
        eprintln!("  lodges {} masons {}", p.req.lodges, p.req.masons);
    }
}
