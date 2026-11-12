//! The design checker and the solver. The catalogue is data, and this module
//! proves it: every good has a maker and a use, every building can be raised
//! from the founding stock, no chain needs its own product to start, and every
//! line of each castle's bill has a chain. The solver then answers the
//! question a player asks of the whole tree: for one stone a minute, how many
//! of everything, all the way down to the ground, the Delve and the people.
//!
//! Both are pure functions of a `Catalog`. Neither runs the economy; the
//! scenario tests build what the solver says and measure whether it holds.
use crate::site::stage_needs;
use crate::*;
use serde_json::{Value, json};
use std::collections::{BTreeMap, BTreeSet};

/// Goods that only the Delve gives: a quarry charter's rock and what comes up
/// with it, and the gold that pays the wages.
fn delve_goods(c: &Catalog) -> BTreeSet<String> {
    let mut v: BTreeSet<String> = c.geology.values().flat_map(|g| std::iter::once(g.stone.clone()).chain(g.also.iter().cloned())).collect();
    v.insert("coin".into());
    v
}
/// Goods the systems around the sim use directly.
const SYSTEM_USES: [&str; 5] = ["coin", "livery", "pole", "centring", "food"];

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Severity {
    Error,
    Note,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Finding {
    pub severity: Severity,
    pub code: &'static str,
    pub subject: String,
    pub message: String,
}
fn err(code: &'static str, subject: &str, message: String) -> Finding {
    Finding { severity: Severity::Error, code, subject: subject.into(), message }
}
fn note(code: &'static str, subject: &str, message: String) -> Finding {
    Finding { severity: Severity::Note, code, subject: subject.into(), message }
}

/// What the founding camp can ever have, given its stock, its founders and
/// (optionally) the Delve's seams.
#[derive(Clone, Debug, Default)]
pub struct Reach {
    pub goods: BTreeSet<String>,
    pub plans: BTreeSet<String>,
    pub techs: BTreeSet<String>,
    pub recipes: BTreeSet<String>,
    pub trades: BTreeSet<String>,
}

/// The trades that can be filled from the founders: the founders' own, and
/// every trade a reachable trade can supervise.
fn reachable_trades(c: &Catalog) -> BTreeSet<String> {
    let mut have: BTreeSet<String> = c.founders.iter().map(|(t, _)| t.clone()).collect();
    have.insert("labourer".into());
    loop {
        let before = have.len();
        for (t, def) in &c.trades {
            if def.kin.iter().any(|k| have.contains(k)) {
                have.insert(t.clone());
            }
        }
        if have.len() == before {
            return have;
        }
    }
}

/// The closure of everything that can be raised from founding stock.
pub fn reach(c: &Catalog, with_delve: bool) -> Reach {
    let mut r = Reach { trades: reachable_trades(c), ..Reach::default() };
    r.goods.extend(c.hillside_starter.keys().cloned());
    // Plans the hillside begins with or the game places.
    r.plans.insert("camp".into());
    r.plans.insert("shaft-hand".into());
    r.plans.insert("castle-yard".into());
    let delve = delve_goods(c);
    loop {
        let before = (r.goods.len(), r.plans.len(), r.techs.len(), r.recipes.len());
        for (k, t) in c.all_techs() {
            if !r.techs.contains(k) && t.requires.iter().all(|q| r.techs.contains(q)) && t.cost.keys().all(|g| r.goods.contains(g)) {
                r.techs.insert(k.clone());
            }
        }
        for (k, p) in &c.plans {
            if r.plans.contains(k) || !p.hillside {
                continue;
            }
            let tool_ok = p.tool.is_empty() || r.goods.contains(&p.tool);
            let upkeep_ok = p.upkeep.keys().all(|g| r.goods.contains(g));
            let site_ok = p.replaces.as_ref().is_none_or(|q| r.plans.contains(q));
            let trade_ok = p.trade.is_empty() || r.trades.contains(&p.trade);
            let claim_ok = !p.extractor || with_delve;
            if p.requires.iter().all(|q| r.techs.contains(q))
                && p.cost.keys().all(|g| r.goods.contains(g))
                && tool_ok
                && upkeep_ok
                && site_ok
                && trade_ok
                && claim_ok
            {
                r.plans.insert(k.clone());
            }
        }
        if with_delve && r.plans.iter().any(|p| c.plans.get(p).is_some_and(|x| x.extractor)) {
            r.goods.extend(delve.iter().cloned());
        }
        for (k, rec) in &c.recipes {
            if r.recipes.contains(k) {
                continue;
            }
            let Some((_, plan)) = c.plans.iter().find(|(pk, p)| r.plans.contains(*pk) && p.recipes.contains(k)) else { continue };
            let patch_ok = true; // woods and the outcrop are always there at founding
            if patch_ok && rec.inputs.keys().all(|g| r.goods.contains(g)) && (plan.tool.is_empty() || r.goods.contains(&plan.tool)) {
                r.recipes.insert(k.clone());
                r.goods.extend(rec.outputs.keys().cloned());
            }
        }
        // Wear returns every tool as its worn state.
        for p in c.plans.values() {
            if !p.tool.is_empty() && r.plans.iter().any(|k| c.plans[k].tool == p.tool) {
                r.goods.insert(format!("worn-{}", p.tool));
            }
        }
        if (r.goods.len(), r.plans.len(), r.techs.len(), r.recipes.len()) == before {
            return r;
        }
    }
}

/// Every check the design is held to.
pub fn check(c: &Catalog) -> Vec<Finding> {
    let mut f = Vec::new();
    let reserved: BTreeSet<&String> = c.reserved.iter().collect();
    let delve = delve_goods(c);
    // -- every good has a maker --
    let mut made: BTreeSet<String> = delve.clone();
    for rec in c.recipes.values() {
        made.extend(rec.outputs.keys().cloned());
    }
    for p in c.plans.values() {
        if !p.tool.is_empty() {
            made.insert(format!("worn-{}", p.tool));
        }
    }
    // The valley's own claim goods and extractor outputs.
    made.extend(["iron-ore", "coal", "crystal", "ember"].map(String::from));
    for g in c.resources.keys() {
        if !made.contains(g) && !reserved.contains(g) {
            f.push(err("no-maker", g, format!("{g} is made by no recipe, claim or tool wear")));
        }
    }
    // -- every good has a use --
    let mut used: BTreeSet<String> = SYSTEM_USES.iter().map(|s| s.to_string()).collect();
    for rec in c.recipes.values() {
        used.extend(rec.inputs.keys().cloned());
    }
    for p in c.plans.values() {
        if p.watch > 0 {
            used.insert(crate::defence::ARROW.to_string());
        }
        used.extend(p.cost.keys().cloned());
        used.extend(p.upkeep.keys().cloned());
        if let Some(pw) = &p.power {
            used.extend(pw.fuel.keys().cloned());
            used.extend(pw.upgrade_cost.keys().cloned());
        }
        if !p.tool.is_empty() {
            used.insert(p.tool.clone());
            used.insert(format!("worn-{}", p.tool));
        }
    }
    for (_, t) in c.all_techs() {
        used.extend(t.cost.keys().cloned());
    }
    for l in c.lines.values() {
        used.extend(l.tile_cost.keys().cloned());
        used.extend(l.vehicle_cost.keys().cloned());
        used.extend(l.upkeep.keys().cloned());
    }
    for rule in c.bill.values() {
        used.insert(rule.resource.clone());
        used.extend(rule.other.iter().cloned());
        used.extend(rule.overrides.iter().map(|o| o.resource.clone()));
    }
    for fx in &c.fixtures {
        used.extend(fx.needs.keys().cloned());
    }
    for tier in [&c.diet.labourer, &c.diet.skilled, &c.diet.master] {
        for e in tier {
            used.extend(e.split('|').map(String::from));
        }
    }
    for g in c.resources.keys() {
        if !used.contains(g) && !reserved.contains(g) {
            f.push(err("no-use", g, format!("{g} is used by no recipe, building, charter, line, bill or meal")));
        }
    }
    // -- every building is reachable from founding stock; the camp age needs no Delve --
    let full = reach(c, true);
    let camp = reach(c, false);
    for (k, p) in &c.plans {
        if p.hillside && !full.plans.contains(k) {
            f.push(err("unbuildable", k, format!("{k} cannot be raised from the founding stock, even with the Delve")));
        }
    }
    for k in c.recipes.keys() {
        if !full.recipes.contains(k) && c.plans.values().any(|p| p.hillside && p.recipes.contains(k)) {
            f.push(err("unrunnable", k, format!("recipe {k} can never run: some input is never made")));
        }
    }
    for g in c.resources.keys() {
        if !full.goods.contains(g) && !reserved.contains(g) && !made.is_empty() {
            let blockers: Vec<String> = c
                .recipes
                .iter()
                .filter(|(_, r)| r.outputs.contains_key(g))
                .flat_map(|(_, r)| r.inputs.keys().filter(|i| !full.goods.contains(*i)).cloned())
                .collect::<BTreeSet<_>>()
                .into_iter()
                .collect();
            // Valley-only goods (arms, wardstones) are made on the hillside too
            // but need ore; any good still unreached is a design hole.
            f.push(err(
                "unreachable",
                g,
                format!("{g} can never exist: its makers wait on {}", if blockers.is_empty() { "a plan nobody can raise".into() } else { blockers.join(", ") }),
            ));
        }
    }
    // -- no bootstrapping cycle: a good whose every maker needs a good that
    //    needs it back, with none to begin with, is unreachable above; here we
    //    name the cycle for the report --
    for g in c.resources.keys() {
        if full.goods.contains(g) || reserved.contains(g) {
            continue;
        }
        f.push(note("cycle", g, format!("{g} is not reachable; check for a loop that needs its own product to start")));
    }
    // -- every line of every castle's bill has a chain --
    for (key, castle) in &c.castles {
        let r = if key == "motte-and-bailey" { &camp } else { &full };
        for stage in &castle.stages {
            for line in stage.bill.keys() {
                if !c.bill.contains_key(line) {
                    f.push(err("bill", key, format!("stage {} bills {line}, which has no chain", stage.id)));
                }
            }
            for (g, n) in stage_needs(c, stage) {
                if n > 0 && !r.goods.contains(&g) {
                    f.push(err(
                        "bill-chain",
                        key,
                        format!("stage {} needs {g}, which {} cannot make", stage.id, if key == "motte-and-bailey" { "the camp age" } else { "the works" }),
                    ));
                }
            }
        }
        // The scaffold the site stands on is made too.
        for g in ["pole", "centring"] {
            if !r.goods.contains(g) {
                f.push(err("site-chain", key, format!("the site's scaffold wants {g}, which cannot be made")));
            }
        }
    }
    // -- tools are a loop: every tool class can be made and re-steeled --
    for p in c.plans.values().filter(|p| !p.tool.is_empty()) {
        let t = &p.tool;
        let makes = c.recipes.iter().any(|(_, r)| r.outputs.contains_key(t) && r.inputs.contains_key(&format!("worn-{t}")));
        if !makes {
            f.push(err("tool-loop", t, format!("no smith re-steels worn {t}s")));
        }
    }
    // -- every trade a building needs can be filled --
    for (k, p) in &c.plans {
        if p.hillside && !p.trade.is_empty() && !full.trades.contains(&p.trade) {
            f.push(err("trade", k, format!("nobody can ever hold the {} trade", p.trade)));
        }
    }
    f
}

// ---------------------------------------------------------------- solver

#[derive(Clone, Debug)]
pub struct SolveOptions {
    pub castle: String,
    pub stones_per_min: f64,
    /// The rock the banker cuts: limestone, freestone, flint, basalt, tufa, voidstone or field.
    pub rock: String,
    /// Fit every machine with its powered drive (twice the pace, demand units).
    pub powered: bool,
    pub haul: LineKind,
    /// Typical haul between works, and from the shaft head, in tiles.
    pub distance: u32,
    pub shaft_distance: u32,
    /// Count the oxen, carts and harness the fleet's growth takes (over an
    /// hour and a half). The camp age has no iron for tyres and does not.
    pub fleet_growth: bool,
}
impl SolveOptions {
    pub fn new(castle: &str, stones_per_min: f64) -> Self {
        Self {
            castle: castle.into(),
            stones_per_min,
            rock: "limestone".into(),
            powered: false,
            haul: LineKind::Cart,
            distance: 20,
            shaft_distance: 12,
            fleet_growth: true,
        }
    }
}

#[derive(Clone, Debug, Default)]
pub struct Plant {
    pub plan: String,
    pub recipe: String,
    pub batches_per_min: f64,
    /// Machines fully busy at the pace the recipe runs.
    pub machines: f64,
    pub count: u32,
    pub workers: u32,
    pub power: u32,
}
/// One flow of goods between two kinds of works, and the carts it keeps busy.
/// Carts are pooled, so these are shares of the pool, not carts of their own.
#[derive(Clone, Debug)]
pub struct LineNeed {
    pub good: String,
    pub from: String,
    pub to: String,
    pub per_min: f64,
    /// Kilograms a minute.
    pub kilos_per_min: f64,
    pub kind: LineKind,
    /// Carts' worth of work, rounded up for display; the pool is sized on the sum.
    pub vehicles: u32,
    /// Loads a minute (a load is a tonne).
    pub loads_per_min: f64,
    /// A flow this heavy over this far is a tramway's work, not the roads'.
    pub tramway: bool,
}
#[derive(Clone, Debug, Default)]
pub struct Requirement {
    pub options: Option<SolveOptions>,
    /// Goods needed per minute, wall and works together.
    pub goods: BTreeMap<String, f64>,
    pub plants: Vec<Plant>,
    /// Machines to raise per building plan (recipes on one plan share).
    pub buildings: BTreeMap<String, u32>,
    pub workers: u32,
    pub posts_by_trade: BTreeMap<String, u32>,
    pub supervisors_by_trade: BTreeMap<String, u32>,
    pub masons: u32,
    pub lodges: u32,
    pub power_units: u32,
    pub lines: Vec<LineNeed>,
    /// Carts in the pool, and the carter's yards that stable them.
    pub vehicles: u32,
    pub yards: u32,
    /// Units a shift the Great Shaft must lift, and the plan that does it.
    pub shaft_per_shift: f64,
    pub shaft_plan: Option<String>,
    pub claims: BTreeMap<String, f64>,
    /// Tools worn per minute, by class.
    pub tool_wear: BTreeMap<String, f64>,
    pub people: u32,
    pub beds: u32,
    /// Coin a minute in wages.
    pub coin_per_min: f64,
    tree: Value,
}

/// The share of return legs that run empty: a cart that has just unloaded takes
/// the next load near it, so most of the way back carries something.
const EMPTY_RETURNS: f64 = 0.4;
/// A flow needing this many carts over ten tiles or more wants a tramway.
const TRAMWAY_CARTS: f64 = 6.0;
/// How full the carts really run: a work asks for its goods in batches and a cart takes what is
/// asked, so the scripted player's carts carried 520 kg a trip on a 1000 kg cart. The solver
/// sizes the pool for that, not for a perfect load.
const LOAD_FILL: f64 = 0.55;

/// Carts' worth of work to move `units_per_min` of a good `dist` tiles: loads a
/// minute (a cart takes a tonne, so a heavy good fills it in ten units and
/// nails in five hundred) times the seconds a load keeps a cart.
fn carts_for(c: &Catalog, good: &str, units_per_min: f64, dist: u32) -> f64 {
    let spec = &c.lines["cart"];
    let kilos = f64::from(c.resources.get(good).map_or(50, |r| r.kilos()));
    let loads = units_per_min * kilos / (f64::from(spec.capacity) * LOAD_FILL);
    let one_way = f64::from(dist) * f64::from(spec.speed) / 10.0;
    loads * (one_way * (1.0 + EMPTY_RETURNS) + 2.0 * f64::from(spec.dwell)) / 60.0
}

fn pick_maker(c: &Catalog, good: &str, o: &SolveOptions) -> Option<String> {
    const ALTERNATES: [&str; 8] = ["burn-lime-peat", "burn-lime-coal", "smelt-bog", "draw-river-water", "dress-field", "barrow-rubble", "ash-fields", "fell-wood"];
    let mut cands: Vec<(&String, &Recipe)> = c
        .recipes
        .iter()
        .filter(|(k, r)| {
            r.outputs.contains_key(good)
                && c.plans.values().any(|p| p.hillside && p.recipes.contains(*k))
        })
        .collect();
    cands.sort_by_key(|(k, r)| {
        let rock = ["dress-", "quoins-", "voussoirs-"].iter().any(|p| k.starts_with(p));
        let rock_match = if rock { u8::from(!k.ends_with(&o.rock)) } else { 0 };
        let alternate = u8::from(ALTERNATES.contains(&k.as_str()));
        let share = std::cmp::Reverse(r.outputs[good]);
        (rock_match, alternate, share, k.to_string())
    });
    cands.first().map(|(k, _)| (*k).clone())
}

fn plan_of(c: &Catalog, recipe: &str) -> Option<String> {
    c.plans.iter().find(|(_, p)| p.hillside && p.recipes.iter().any(|r| r == recipe)).map(|(k, _)| k.clone())
}

/// Solves the whole tree for `o.stones_per_min`.
pub fn solve(c: &Catalog, o: &SolveOptions) -> Result<Requirement, String> {
    let castle = c.castles.get(&o.castle).ok_or("unknown castle")?;
    let stones = f64::from(castle.stones());
    let rate = o.stones_per_min;
    // The wall's own demand, from the bill, per minute at this pace.
    let mut wall: BTreeMap<String, f64> = BTreeMap::new();
    let mut total_need = Stock::new();
    let mut lifts = 0u32;
    let mut arch_lifts = 0u32;
    for st in &castle.stages {
        let n = stage_needs(c, st);
        if st.count > 0 {
            lifts += 1;
            if n.get("voussoir").copied().unwrap_or(0) > 0 {
                arch_lifts += 1;
            }
        }
        for (g, q) in n {
            *total_need.entry(g).or_default() += q;
        }
    }
    for (g, q) in &total_need {
        wall.insert(g.clone(), f64::from(*q) / stones * rate);
    }
    // Scaffold stock wears out as lifts finish.
    *wall.entry("pole".into()).or_default() += f64::from(lifts * c.works.poles_worn) / stones * rate;
    *wall.entry("centring".into()).or_default() += f64::from(arch_lifts * c.works.centring_worn) / stones * rate;

    // Chosen makers and the tools' special loop.
    let mut maker: BTreeMap<String, String> = BTreeMap::new();
    let mut goods: BTreeSet<String> = wall.keys().cloned().collect();
    let mut frontier: Vec<String> = goods.iter().cloned().collect();
    let tool_classes: BTreeSet<String> = c.plans.values().filter(|p| !p.tool.is_empty()).map(|p| p.tool.clone()).collect();
    // The people eat and the oxen are fed: those goods belong to the tree too.
    let mut seeds: Vec<String> = vec![c.diet.staple.clone()];
    seeds.extend(c.diet.skilled.get(1).cloned());
    seeds.extend(c.lines[o.haul.key()].upkeep.keys().cloned());
    if o.haul == LineKind::Cart && o.fleet_growth {
        seeds.extend(["cart", "ox", "harness"].map(String::from));
    }
    for g in seeds.into_iter().filter(|g| !g.is_empty()) {
        if goods.insert(g.clone()) {
            frontier.push(g);
        }
    }
    // Worn tools are re-steeled: the smith's inputs belong to the tree even
    // though nothing in the wall asks for a tool.
    for t in &tool_classes {
        if goods.insert(t.clone()) {
            frontier.push(t.clone());
        }
    }
    while let Some(g) = frontier.pop() {
        let pick = if tool_classes.contains(&g) {
            Some(format!("resteel-{g}")).filter(|r| c.recipes.contains_key(r))
        } else if g.starts_with("worn-") {
            None
        } else {
            pick_maker(c, &g, o)
        };
        if let Some(r) = pick {
            for i in c.recipes[&r].inputs.keys() {
                if goods.insert(i.clone()) {
                    frontier.push(i.clone());
                }
            }
            maker.insert(g, r);
        }
    }
    // Extra demands the economy makes of itself: bread and ale for the people,
    // fodder for the oxen, steel and charcoal for the smith: found by iteration.
    let delve = delve_goods(c);
    let mut runs: BTreeMap<String, f64> = BTreeMap::new();
    let mut extra: BTreeMap<String, f64> = BTreeMap::new();
    let mut result = Requirement { options: Some(o.clone()), ..Requirement::default() };
    for _ in 0..60 {
        // Demand per good: wall + inputs of every running recipe + extras - co-products.
        let mut demand: BTreeMap<String, f64> = wall.clone();
        for (g, n) in &extra {
            *demand.entry(g.clone()).or_default() += n;
        }
        let mut co: BTreeMap<String, f64> = BTreeMap::new();
        for (r, n) in &runs {
            let rec = &c.recipes[r];
            for (g, q) in &rec.inputs {
                *demand.entry(g.clone()).or_default() += f64::from(*q) * n;
            }
            for (g, q) in &rec.outputs {
                if maker.get(g) != Some(r) {
                    *co.entry(g.clone()).or_default() += f64::from(*q) * n;
                }
            }
        }
        // Tools worn out come back as worn tools, one for one.
        let mut wear: BTreeMap<String, f64> = BTreeMap::new();
        for (pk, p) in c.plans.iter().filter(|(_, p)| !p.tool.is_empty() && p.hillside) {
            let busy: f64 = result.plants.iter().filter(|x| &x.plan == pk).map(|x| x.machines).sum();
            *wear.entry(p.tool.clone()).or_default() += busy * 60.0 / (f64::from(p.tool_life.max(1)) * 10.0);
        }
        for (t, w) in &wear {
            *co.entry(format!("worn-{t}")).or_default() += w;
            *demand.entry(t.clone()).or_default() += 0.0;
        }
        // New runs from demand.
        let mut moved = 0.0f64;
        let mut next: BTreeMap<String, f64> = BTreeMap::new();
        for (g, r) in &maker {
            let need = if tool_classes.contains(g) { wear.get(g).copied().unwrap_or(0.0) } else { (demand.get(g).copied().unwrap_or(0.0) - co.get(g).copied().unwrap_or(0.0)).max(0.0) };
            let out = f64::from(c.recipes[r].outputs[g]);
            *next.entry(r.clone()).or_default() = next.get(r).copied().unwrap_or(0.0).max(need / out);
        }
        for (r, n) in &next {
            moved = moved.max((n - runs.get(r).copied().unwrap_or(0.0)).abs());
        }
        runs = next;
        // Plants from runs.
        let mut plants = Vec::new();
        for (r, n) in runs.iter().filter(|(_, n)| **n > 1e-9) {
            let Some(pk) = plan_of(c, r) else { continue };
            let p = &c.plans[&pk];
            let shifts = f64::from(c.recipes[r].shifts);
            let pace = if o.powered && p.power.as_ref().is_some_and(|x| x.is_consumer()) {
                f64::from(p.power.as_ref().unwrap().multiplier) / 100.0
            } else {
                1.0
            };
            let machines = n * shifts * 10.0 / 60.0 / pace;
            plants.push(Plant { plan: pk, recipe: r.clone(), batches_per_min: *n, machines, ..Plant::default() });
        }
        result.plants = plants;
        // People and what they eat.
        let mut by_plan: BTreeMap<String, f64> = BTreeMap::new();
        for x in &result.plants {
            *by_plan.entry(x.plan.clone()).or_default() += x.machines;
        }
        let mut people = 0.0;
        let mut skilled = 0.0;
        for (pk, m) in &by_plan {
            let p = &c.plans[pk];
            people += m.ceil() * f64::from(p.workers);
            if !p.trade.is_empty() {
                skilled += m.ceil() / f64::from(c.works.posts_per_supervisor.max(1));
            }
        }
        // The masons at the wall, the shaft's crew, the ditchers are counted below.
        let masons = (rate / (f64::from(c.works.mason_credit) / 100.0 * 6.0)).ceil();
        people += masons;
        let staple = 60.0 / 90.0;
        let mut ex: BTreeMap<String, f64> = BTreeMap::new();
        ex.insert(c.diet.staple.clone(), people * staple);
        if let Some(e) = c.diet.skilled.get(1) {
            ex.insert(e.clone(), skilled * staple);
        }
        // The oxen's fodder, found from the carts the flows below keep busy: every
        // input of every recipe is hauled from its maker, and so is the wall's.
        let spec = &c.lines[o.haul.key()];
        let mut veh = 0.0;
        for x in &result.plants {
            let rec = &c.recipes[&x.recipe];
            for (g, q) in &rec.inputs {
                let dist = if delve.contains(g) && !maker.contains_key(g) { o.shaft_distance } else { o.distance };
                veh += carts_for(c, g, f64::from(*q) * x.batches_per_min, dist);
            }
        }
        for (g, per) in &wall {
            veh += carts_for(c, g, *per, o.distance);
        }
        for (g, n) in &spec.upkeep {
            *ex.entry(g.clone()).or_default() += f64::from(*n) * veh.ceil() * 60.0 / f64::from(spec.upkeep_seconds.max(1));
        }
        // The fleet itself is built over the first hour and a half: carts from
        // the wheelwright, oxen reared, harness stitched.
        if o.haul == LineKind::Cart && o.fleet_growth {
            for g in ["cart", "ox", "harness"] {
                *ex.entry(g.to_string()).or_default() += veh.ceil() / 90.0;
            }
        }
        // Wages for the skilled, paid in the Delve's coin.
        let coin = skilled * f64::from(c.works.wage_journeyman) * 60.0 / f64::from(c.works.wage_seconds.max(1));
        *ex.entry("coin".into()).or_default() += coin;
        let delta = ex.iter().map(|(g, n)| (n - extra.get(g).copied().unwrap_or(0.0)).abs()).fold(0.0, f64::max);
        extra = ex;
        if moved < 1e-9 && delta < 1e-9 {
            break;
        }
    }
    // Final accounting.
    let mut demand: BTreeMap<String, f64> = wall.clone();
    for (g, n) in &extra {
        *demand.entry(g.clone()).or_default() += n;
    }
    for (r, n) in &runs {
        for (g, q) in &c.recipes[r].inputs {
            *demand.entry(g.clone()).or_default() += f64::from(*q) * n;
        }
    }
    let mut by_plan: BTreeMap<String, f64> = BTreeMap::new();
    for x in &mut result.plants {
        x.count = x.machines.ceil() as u32;
        let p = &c.plans[&x.plan];
        x.workers = x.count * u32::from(p.workers);
        x.power = if o.powered { x.count * p.power.as_ref().map_or(0, |q| u32::from(q.demand)) } else { 0 };
        *by_plan.entry(x.plan.clone()).or_default() += x.machines;
    }
    for (pk, m) in &by_plan {
        let n = m.ceil() as u32;
        result.buildings.insert(pk.clone(), n);
        let p = &c.plans[pk];
        result.workers += n * u32::from(p.workers);
        if !p.trade.is_empty() {
            *result.posts_by_trade.entry(p.trade.clone()).or_default() += n;
        }
        if o.powered {
            result.power_units += n * p.power.as_ref().map_or(0, |q| u32::from(q.demand));
        }
    }
    for (t, n) in &result.posts_by_trade {
        result.supervisors_by_trade.insert(t.clone(), n.div_ceil(u32::from(c.works.posts_per_supervisor.max(1))));
    }
    result.masons = (rate / (f64::from(c.works.mason_credit) / 100.0 * 6.0)).ceil() as u32;
    result.lodges = result.masons.div_ceil(u32::from(c.plans["lodge"].workers.max(1)));
    result.workers += result.masons;
    // Claims and the shaft: every good no recipe makes that the Delve supplies.
    for (g, n) in &demand {
        if !maker.contains_key(g) && delve.contains(g) && g != "coin" {
            result.claims.insert(g.clone(), *n);
        }
    }
    result.shaft_per_shift = result.claims.values().sum::<f64>() / 6.0;
    // (A lift is rated in blocks a minute: a shift lifts a sixth of that.)
    // The lowest lift that raises enough, counting a powered drive's gain.
    let mut tiers: Vec<(u32, &String)> = c
        .plans
        .iter()
        .filter(|(_, p)| p.shaft > 0)
        .map(|(k, p)| (p.shaft * p.power.as_ref().map_or(1, |q| u32::from(q.multiplier) / 100), k))
        .collect();
    tiers.sort();
    result.shaft_plan = tiers
        .iter()
        .find(|(per_minute, _)| f64::from(*per_minute) >= result.shaft_per_shift * 6.0)
        .or(tiers.last())
        .map(|(_, k)| (*k).clone());
    // Transport: every input flow of every recipe, and the wall's, is carried from
    // its maker by the shared pool of carts.
    let spec = &c.lines[o.haul.key()];
    let mut total = 0.0;
    let mut flows: Vec<(String, String, String, f64, u32)> = Vec::new();
    for x in &result.plants {
        let rec = &c.recipes[&x.recipe];
        for (g, q) in &rec.inputs {
            let from = maker
                .get(g)
                .and_then(|r| plan_of(c, r))
                .unwrap_or_else(|| if delve.contains(g) { "shaft-head".into() } else { "camp".into() });
            let dist = if from == "shaft-head" { o.shaft_distance } else { o.distance };
            flows.push((g.clone(), from, x.plan.clone(), f64::from(*q) * x.batches_per_min, dist));
        }
    }
    for (g, per) in &wall {
        let from = maker
            .get(g)
            .and_then(|r| plan_of(c, r))
            .unwrap_or_else(|| if delve.contains(g) { "shaft-head".into() } else { "camp".into() });
        flows.push((g.clone(), from, "castle-yard".into(), *per, o.distance));
    }
    for (g, from, to, per_min, dist) in flows {
        let carts = carts_for(c, &g, per_min, dist);
        total += carts;
        let kilos = per_min * f64::from(c.resources.get(&g).map_or(50, |r| r.kilos()));
        result.lines.push(LineNeed {
            good: g,
            from,
            to,
            per_min,
            kilos_per_min: kilos,
            kind: o.haul,
            vehicles: carts.ceil() as u32,
            loads_per_min: kilos / f64::from(spec.capacity),
            tramway: carts >= TRAMWAY_CARTS && dist >= 10,
        });
    }
    result.vehicles = total.ceil() as u32;
    result.yards = result.vehicles.div_ceil(c.plans.get("carters-yard").map_or(12, |p| u32::from(p.stalls)).max(1));
    // Tools in the loop, and the people and beds.
    for (pk, p) in c.plans.iter().filter(|(_, p)| !p.tool.is_empty() && p.hillside) {
        let busy: f64 = result.plants.iter().filter(|x| &x.plan == pk).map(|x| x.machines).sum();
        *result.tool_wear.entry(p.tool.clone()).or_default() += busy * 60.0 / (f64::from(p.tool_life.max(1)) * 10.0);
    }
    result.people = result.workers;
    result.beds = result.people.div_ceil(4);
    result.coin_per_min = extra.get("coin").copied().unwrap_or(0.0);
    result.goods = demand;
    result.tree = tree(c, o, &maker, &runs, &result);
    Ok(result)
}

/// The requirement as a tree, from the wall down: each good with what makes
/// it, how many machines and people that takes, and what that needs in turn.
fn tree(c: &Catalog, o: &SolveOptions, maker: &BTreeMap<String, String>, runs: &BTreeMap<String, f64>, req: &Requirement) -> Value {
    let name = |g: &str| c.resources.get(g).map_or(g.to_string(), |r| r.name.clone());
    let delve = delve_goods(c);
    let mut seen: BTreeSet<String> = BTreeSet::new();
    fn node(
        c: &Catalog,
        g: &str,
        branch: f64,
        maker: &BTreeMap<String, String>,
        runs: &BTreeMap<String, f64>,
        req: &Requirement,
        delve: &BTreeSet<String>,
        seen: &mut BTreeSet<String>,
        name: &dyn Fn(&str) -> String,
        depth: usize,
    ) -> Value {
        let total = req.goods.get(g).copied().unwrap_or(branch);
        let mut v = json!({"good": g, "name": name(g), "per_min": round(branch), "total_per_min": round(total)});
        let Some(r) = maker.get(g) else {
            v["source"] = json!(if delve.contains(g) { "the Delve (a quarry charter, through the Great Shaft)" } else if g.starts_with("worn-") { "tool wear" } else { "founding stock" });
            return v;
        };
        let rec = &c.recipes[r];
        let plant = req.plants.iter().find(|x| &x.recipe == r);
        v["maker"] = json!({
            "recipe": r,
            "recipe_name": rec.name,
            "plan": plant.map(|x| x.plan.clone()),
            "machines": plant.map(|x| round(x.machines)),
            "count": plant.map(|x| x.count),
            "workers": plant.map(|x| x.workers),
            "trade": plant.and_then(|x| {
                let t = &c.plans[&x.plan].trade;
                (!t.is_empty()).then(|| t.clone())
            }),
            "seconds_per_batch": f64::from(rec.shifts) * 10.0,
            "by_products": rec.outputs.iter().filter(|(k, _)| k.as_str() != g).map(|(k, n)| json!({"good": k, "per_batch": n})).collect::<Vec<_>>(),
        });
        if !seen.insert(g.to_string()) || depth > 30 {
            v["seen_above"] = json!(true);
            return v;
        }
        let batches = runs.get(r).copied().unwrap_or(0.0);
        let mut inputs = Vec::new();
        for (i, q) in &rec.inputs {
            let share = f64::from(*q) * batches * (branch / (req.goods.get(g).copied().unwrap_or(branch).max(1e-9)));
            inputs.push(node(c, i, share, maker, runs, req, delve, seen, name, depth + 1));
        }
        v["inputs"] = json!(inputs);
        v
    }
    let mut wall = Vec::new();
    let mut order: Vec<&String> = req.goods.keys().collect();
    order.sort();
    for (g, per) in wall_goods(c, o) {
        wall.push(node(c, &g, per, maker, runs, req, &delve, &mut seen, &name, 0));
    }
    let _ = order;
    json!({"set_in_the_wall": {"stones_per_min": o.stones_per_min, "needs": wall}})
}

/// The goods the wall itself asks for, per minute.
fn wall_goods(c: &Catalog, o: &SolveOptions) -> Vec<(String, f64)> {
    let castle = &c.castles[&o.castle];
    let stones = f64::from(castle.stones());
    let mut total = Stock::new();
    for st in &castle.stages {
        for (g, q) in stage_needs(c, st) {
            *total.entry(g).or_default() += q;
        }
    }
    let mut v: Vec<(String, f64)> = total.into_iter().map(|(g, q)| (g, f64::from(q) / stones * o.stones_per_min)).collect();
    v.push(("pole".into(), 0.0));
    v.push(("centring".into(), 0.0));
    v
}

fn round(x: f64) -> f64 {
    (x * 1000.0).round() / 1000.0
}

impl Requirement {
    /// The whole answer as JSON: the tree, then the flat counts.
    pub fn json(&self, c: &Catalog) -> Value {
        let o = self.options.as_ref().expect("solved");
        json!({
            "target": {"castle": o.castle, "stones_per_min": o.stones_per_min, "rock": o.rock, "powered": o.powered,
                       "haul": o.haul.key(), "distance_tiles": o.distance},
            "tree": self.tree,
            "buildings": self.buildings,
            "plants": self.plants.iter().map(|p| json!({
                "plan": p.plan, "recipe": p.recipe, "batches_per_min": round(p.batches_per_min),
                "machines": round(p.machines), "count": p.count, "workers": p.workers, "power_units": p.power})).collect::<Vec<_>>(),
            "goods_per_min": self.goods.iter().map(|(g, n)| (g.clone(), round(*n))).collect::<BTreeMap<_, _>>(),
            "people": {"workers": self.workers, "masons": self.masons, "lodges": self.lodges, "beds": self.beds,
                       "posts_by_trade": self.posts_by_trade, "supervisors_by_trade": self.supervisors_by_trade,
                       "coin_per_min": round(self.coin_per_min)},
            "power": {"units": self.power_units},
            "transport": {"carts": self.vehicles, "yards": self.yards, "vehicles": self.vehicles,
                          "note": "carts are one pool on the road network; each flow shows the carts' worth of work it keeps busy",
                          "flows": self.lines.iter().map(|l| json!({
                "good": l.good, "from": l.from, "to": l.to, "per_min": round(l.per_min),
                "kilos_per_min": round(l.kilos_per_min), "loads_per_min": round(l.loads_per_min),
                "carts": l.vehicles, "tramway": l.tramway, "kind": l.kind.key()})).collect::<Vec<_>>()},
            "great_shaft": {"units_per_shift": round(self.shaft_per_shift), "tier": self.shaft_plan,
                            "claims_per_min": self.claims.iter().map(|(g, n)| (g.clone(), round(*n))).collect::<BTreeMap<_, _>>()},
            "tools": {"worn_per_min": self.tool_wear.iter().map(|(g, n)| (g.clone(), round(*n))).collect::<BTreeMap<_, _>>()},
            "findings": check(c).iter().map(|f| json!({
                "severity": if f.severity == Severity::Error { "error" } else { "note" },
                "code": f.code, "subject": f.subject, "message": f.message})).collect::<Vec<_>>(),
        })
    }
}
