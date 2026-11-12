//! The Clerk of Works: a pure function that says, in one plain sentence, what
//! is holding the build back, with the chain walked back to its cause:
//! "waiting on mortar; the mortar mill is out of slaked lime; the slaking pit
//! has no quicklime; the lime kiln is out of charcoal; the clamp has no timber".
//!
//! It reads the state and the report and changes nothing. Each step names a
//! machine and what it lacks, then asks the same question of the missing good,
//! until it reaches a cause the player can act on: nothing built, nobody to
//! work it, no road, no line, a line at its limit, or a full store.
use crate::haul::LineState;
use crate::site::Hold;
use crate::*;
use std::collections::BTreeSet;

/// What a link of the chain comes down to, for a player (or a script) that wants to act on it
/// rather than read it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Cause {
    /// The makers of `good` all work flat out and it is not enough: build another `plan`.
    Capacity { plan: String, good: String },
    /// Nothing built makes `good`.
    NoMaker { good: String },
    /// A `plan` has no crew: homes and food bring people.
    NoCrew { plan: String },
    /// The goods are made and stand waiting: the roads, the carts or a line are the limit.
    Carriage { plan: String, good: String },
    /// A maker's own shelf is full and nothing takes the goods away.
    Full { plan: String, good: String },
    /// Worn tools, a bare wood, the Great Shaft, fodder: the maker is stopped by something else.
    Stopped { plan: String, why: String },
    /// A `plan` is set to the wrong recipe for `good`.
    WrongRecipe { plan: String, good: String },
}

/// The chain as it is walked: the sentence's links, and what each comes down to.
#[derive(Clone, Debug, Default)]
pub struct Chain {
    pub lines: Vec<String>,
    pub causes: Vec<Cause>,
    /// The makers the walk passed through, each with the good it was asked for.
    pub makers: Vec<(String, String)>,
}
impl Chain {
    pub fn push(&mut self, s: String) {
        self.lines.push(s);
    }
    fn cause(&mut self, c: Cause) {
        self.causes.push(c);
    }
    fn is_empty(&self) -> bool {
        self.lines.is_empty()
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Clerk {
    pub site: u32,
    /// The whole sentence, ready to print.
    pub sentence: String,
    /// The same, one link of the chain at a time, the cause last.
    pub chain: Vec<String>,
    /// What each link comes down to, the root cause last.
    pub causes: Vec<Cause>,
    /// True when nothing at all is holding the masons up.
    pub unhindered: bool,
}

impl Kingdom {
    pub fn clerk(&self, c: &Catalog, site: u32) -> Clerk {
        let report = self.report(c);
        let mut chain = Chain::default();
        let mut unhindered = false;
        let Some(s) = self.sites.get(&site) else {
            return Clerk { site, sentence: "there is no such castle site".into(), chain: vec![], causes: vec![], unhindered: false };
        };
        let castle = &c.castles[&s.plan];
        let masons = self.masons(c, site);
        let a = self.site_allowance(c, site, masons.max(1));
        let name = |r: &str| c.resources.get(r).map_or(r.to_string(), |x| x.name.to_lowercase());
        match &a.hold {
            Hold::Done => chain.push(format!("the {} stands finished", castle.name.to_lowercase())),
            Hold::Yard => chain.push("waiting on the stockyard; it is still being built".to_string()),
            Hold::Earthwork => {
                chain.push("waiting on earth; the ditch is still being dug".to_string());
                let mut seen = BTreeSet::new();
                self.explain(c, &report, "earth", self.sites[&site].yard, 0, &mut seen, &mut chain);
            }
            Hold::Scaffold { have, need } => {
                chain.push(format!("waiting on scaffold poles; {have} stand of {need} wanted"));
                let mut seen = BTreeSet::new();
                self.explain(c, &report, "pole", self.sites[&site].yard, 0, &mut seen, &mut chain);
            }
            Hold::Centring { have, need } => {
                chain.push(format!("waiting on arch centring; {have} sets of {need} wanted"));
                let mut seen = BTreeSet::new();
                self.explain(c, &report, "centring", self.sites[&site].yard, 0, &mut seen, &mut chain);
            }
            Hold::Short { resource } => {
                chain.push(format!("waiting on {}", name(resource)));
                let mut seen = BTreeSet::new();
                self.explain(c, &report, resource, self.sites[&site].yard, 0, &mut seen, &mut chain);
            }
            Hold::Masons => {
                if masons == 0 {
                    self.explain_masons(c, &report, site, &mut chain);
                } else {
                    chain.push("the masons are between shifts".to_string());
                }
            }
            Hold::Pace => {
                if masons == 0 {
                    self.explain_masons(c, &report, site, &mut chain);
                } else {
                    unhindered = true;
                    chain.push(format!(
                        "nothing holds the masons up; the pace of {masons} masons is the limit, and another lodge would raise it"
                    ));
                }
            }
        }
        Clerk { site, sentence: chain.lines.join("; "), chain: chain.lines, causes: chain.causes, unhindered }
    }

    /// Why the camp is short of one good, walked back to what can be built about it: the
    /// causes, the root last. Empty when the camp has it or nothing is the matter.
    pub fn causes_for(&self, c: &Catalog, good: &str) -> Vec<Cause> {
        self.walk_for(c, good, 1).causes
    }

    /// The same walk, with the makers it passed through: the chain that makes the good, plan
    /// by plan, each with the good it was asked for.
    pub fn walk_for(&self, c: &Catalog, good: &str, consumer: u32) -> Chain {
        let report = self.report(c);
        let mut chain = Chain::default();
        self.explain(c, &report, good, consumer, 0, &mut BTreeSet::new(), &mut chain);
        chain
    }

    /// Why one building is not making its goods, walked back the same way.
    pub fn clerk_why_idle(&self, c: &Catalog, id: u32) -> String {
        let report = self.report(c);
        let Some(b) = self.buildings.get(&id) else { return "there is no such building".into() };
        let mut chain = Chain::default();
        let status = report.status.get(&id).cloned().unwrap_or(Status::Idle);
        if let Some((_, out, _)) = self.job(c, b) {
            if let Some(good) = out.keys().next() {
                self.explain(c, &report, good, 1, 0, &mut BTreeSet::new(), &mut chain);
            }
        }
        if chain.is_empty() {
            chain.push(status.label().to_lowercase());
        }
        chain.lines.join("; ")
    }

    fn explain_masons(&self, c: &Catalog, report: &Report, site: u32, chain: &mut Chain) {
        let yard = self.sites[&site].yard;
        let range = i16::from(c.works.lodge_range);
        let yt = self.buildings[&yard].tile;
        let lodges: Vec<u32> = self
            .buildings
            .iter()
            .filter(|(_, b)| b.plan == "lodge" && (b.tile.0 - yt.0).abs() + (b.tile.1 - yt.1).abs() <= range)
            .map(|(&id, _)| id)
            .collect();
        if lodges.is_empty() {
            chain.push(format!("no masons at the wall; no lodge stands within {range} tiles of the yard"));
            return;
        }
        let id = lodges[0];
        chain.push("no masons at the wall".to_string());
        match report.status.get(&id) {
            Some(Status::Construction) => chain.push("the masons' lodge is still being built".into()),
            Some(Status::Disconnected) => chain.push("the masons' lodge has no road to the camp".into()),
            Some(Status::Paused) => chain.push("the masons' lodge is paused".into()),
            _ => chain.push(format!(
                "the masons' lodge has no crew; {} of {} people are idle and homes and food bring more",
                report.population.idle, report.population.population
            )),
        }
    }

    /// Why `resource` is not reaching `consumer`, one link at a time. A line
    /// that runs into the consumer is followed first; otherwise the roads are:
    /// who holds the good, whether a yard, a carter, fodder, a free cart and a
    /// clear road stand between it and the consumer, and then what makes it.
    #[allow(clippy::too_many_arguments)]
    fn explain(
        &self,
        c: &Catalog,
        report: &Report,
        resource: &str,
        consumer: u32,
        depth: usize,
        seen: &mut BTreeSet<String>,
        chain: &mut Chain,
    ) {
        if depth > 12 || !seen.insert(format!("{resource}@{consumer}")) {
            return;
        }
        let name = |r: &str| c.resources.get(r).map_or(r.to_string(), |x| x.name.to_lowercase());
        let plan_name = |id: u32| c.plans[&self.buildings[&id].plan].name.to_lowercase();
        let makes = |id: u32| {
            let b = &self.buildings[&id];
            c.plans[&b.plan].recipes.iter().any(|r| c.recipes[r].outputs.contains_key(resource))
        };
        let cp = plan_name(consumer);
        let rows = self.line_report(c);
        let inbound: Vec<&crate::LineRow> = rows.iter().filter(|r| r.to == consumer).collect();
        // A line that is itself the problem is named first.
        for r in &inbound {
            let spec = &c.lines[r.kind.key()];
            let from = plan_name(r.from);
            match r.state {
                LineState::Cut => {
                    chain.push(format!("the {} line from the {from} to the {cp} is cut", spec.name));
                    return;
                }
                LineState::Unfed => {
                    chain.push(format!("the {} line from the {from} to the {cp} has no fodder", spec.name));
                    return;
                }
                LineState::Saturated => {
                    chain.push(format!(
                        "the {} line from the {from} to the {cp} is the limit: {} {}s carry {} a minute ({} delivered); add vehicles or a faster line",
                        spec.name, r.vehicles, spec.vehicle, r.capacity_per_min, r.observed_per_min
                    ));
                    return;
                }
                _ => {}
            }
        }
        // Who holds the good now: a maker's bench, a warehouse, the camp.
        let holders: Vec<u32> = self
            .buildings
            .iter()
            .filter(|&(&id, b)| {
                id != consumer
                    && b.construction == 0
                    && !c.plans[&b.plan].yard
                    && if id == 1 { self.amount(resource) > 0 } else { b.output.get(resource).copied().unwrap_or(0) > 0 }
            })
            .map(|(&id, _)| id)
            .collect();
        let fed_by_line = inbound.iter().any(|r| r.from == 1 || c.plans[&self.buildings[&r.from].plan].hub || makes(r.from));
        if !holders.is_empty() && !fed_by_line {
            if let Some(why) = self.cart_trouble(c, report, &holders, consumer) {
                chain.push(why);
                chain.cause(Cause::Carriage { plan: self.buildings[&holders[0]].plan.clone(), good: resource.to_string() });
                return;
            }
        }
        // Nothing is there to carry: what makes it?
        let makers: Vec<u32> = self.buildings.iter().filter(|(id, b)| b.construction == 0 && makes(**id)).map(|(&id, _)| id).collect();
        if makers.is_empty() {
            chain.cause(Cause::NoMaker { good: resource.to_string() });
            if let Some((&id, _)) = self.buildings.iter().find(|(id, b)| b.construction > 0 && makes(**id)) {
                chain.push(format!("the {} that makes {} is still being built", plan_name(id), name(resource)));
            } else {
                let menu = if self.world.is_hillside() { &c.hillside_menu } else { &c.menu };
                match menu.iter().find(|p| c.plans[*p].recipes.iter().any(|r| c.recipes[r].outputs.contains_key(resource))) {
                    Some(p) => chain.push(format!("nothing makes {} yet; build a {}", name(resource), c.plans[p].name.to_lowercase())),
                    None => chain.push(format!("{} comes from a claim won in a delve", name(resource))),
                }
            }
            return;
        }
        // The maker that lacks something is the cause; one that is working is only slow.
        let ordered: Vec<u32> = {
            let mut m = makers.clone();
            m.sort_by_key(|id| (report.status.get(id) == Some(&Status::Working), *id));
            m
        };
        let u = ordered[0];
        let un = plan_name(u);
        let b = &self.buildings[&u];
        if !chain.makers.iter().any(|(p, g)| *p == b.plan && g == resource) {
            chain.makers.push((b.plan.clone(), resource.to_string()));
        }
        if b.recipe.as_ref().is_none_or(|x| !c.recipes[x].outputs.contains_key(resource)) {
            chain.cause(Cause::WrongRecipe { plan: b.plan.clone(), good: resource.to_string() });
            chain.push(format!(
                "the {un} is set to {}; set it to make {}",
                b.recipe.as_ref().map_or("nothing".to_string(), |x| c.recipes[x].name.to_lowercase()),
                name(resource)
            ));
            return;
        }
        match report.status.get(&u).cloned().unwrap_or(Status::Idle) {
            Status::Working => {
                chain.cause(Cause::Capacity { plan: b.plan.clone(), good: resource.to_string() });
                chain.push(format!("the {un} is making {}, and it is on its way to the {cp}", name(resource)));
            }
            Status::Inputs => {
                let (input, _, _) = self.job(c, b).expect("a producer has a job");
                let missing = input.iter().find(|(r, n)| b.input.get(*r).copied().unwrap_or(0) < **n).map(|(r, _)| r.clone());
                chain.push(format!("the {un} is out of {}", missing.as_deref().map_or("inputs".to_string(), name)));
                if let Some(m) = missing {
                    let before = chain.causes.len();
                    self.explain(c, report, &m, u, depth + 1, seen, chain);
                    if chain.causes.len() == before {
                        chain.cause(Cause::Carriage { plan: b.plan.clone(), good: m });
                    }
                }
            }
            Status::Storage => {
                chain.cause(Cause::Full { plan: b.plan.clone(), good: resource.to_string() });
                chain.push(format!("the {un} is full of finished goods that nothing is taking away"));
                self.explain_outflow(c, report, u, chain);
            }
            Status::Workers => {
                chain.cause(Cause::NoCrew { plan: b.plan.clone() });
                chain.push(format!(
                    "the {un} has no crew; {} of {} people are idle",
                    report.population.idle, report.population.population
                ))
            }
            Status::Disconnected => {
                chain.cause(Cause::Stopped { plan: b.plan.clone(), why: "no road".into() });
                chain.push(format!("the {un} has no road to the camp"))
            }
            Status::Paused => chain.push(format!("the {un} is paused")),
            Status::Fire => {
                chain.cause(Cause::Stopped { plan: b.plan.clone(), why: "fire".into() });
                chain.push(format!("the {un} is on fire; beat it out or build a well beside it"))
            }
            Status::Construction => chain.push(format!("the {un} is still being built")),
            Status::PowerDisconnected => chain.push(format!("the {un} is fitted for power but no shaft reaches a source")),
            Status::PowerOverloaded => chain.push(format!("the {un} is starved of power; its network is overloaded")),
            Status::Fuel => chain.push(format!("the {un} is out of fuel or fodder")),
            Status::Fodder => {
                chain.cause(Cause::Stopped { plan: b.plan.clone(), why: "fodder".into() });
                chain.push(format!("the {un}'s oxen have no fodder"))
            }
            Status::Tools => {
                chain.cause(Cause::Stopped { plan: b.plan.clone(), why: "tools".into() });
                let tool = c.plans[&b.plan].tool.clone();
                chain.push(format!("the {un} has worn out its {tool}s and the camp has none to give"));
                self.explain(c, report, &tool, 1, depth + 1, seen, chain);
            }
            Status::Depleted => {
                chain.cause(Cause::Stopped { plan: b.plan.clone(), why: "depleted".into() });
                chain.push(format!("the {un} has worked its ground bare; let the woods regrow or move on"))
            }
            Status::Shaft => chain.push("the Great Shaft cannot lift more; raise its tier".to_string()),
            Status::Target => chain.push(format!("the {un} has met its stock target")),
            Status::Unclaimed => chain.push(format!("the {un} has no claim linked")),
            Status::Idle => chain.push(format!("the {un} is idle")),
        }
    }

    /// Goods are waiting at `holders` and `consumer` lacks them: what keeps the
    /// roads from carrying them? `None` when nothing does (the goods are on their way).
    fn cart_trouble(&self, c: &Catalog, report: &Report, holders: &[u32], consumer: u32) -> Option<String> {
        let plan_name = |id: u32| c.plans[&self.buildings[&id].plan].name.to_lowercase();
        let cp = plan_name(consumer);
        let from = plan_name(holders[0]);
        let roads = self.road_report(c);
        let yard_name = &c.plans["carters-yard"].name.to_lowercase();
        if roads.yards.is_empty() {
            return Some(format!("no {yard_name} stands; raise one and add carts, or lay a rollerway or a rail from the {from} to the {cp}"));
        }
        if roads.carts == 0 {
            return Some(format!("the {yard_name} keeps no carts; add some"));
        }
        if let Some(y) = roads.yards.iter().find(|y| y.carts > 0 && !y.crewed) {
            let _ = y;
            return Some(format!("the {yard_name} has no carter; {} of {} people are idle", report.population.idle, report.population.population));
        }
        if roads.yards.iter().all(|y| y.carts == 0 || !y.fed) {
            return Some(format!("the {yard_name}'s oxen have no fodder"));
        }
        // No road, or no road that joins them.
        let net = self.roads_for_clerk();
        let near = |id: u32| {
            let b = &self.buildings[&id];
            self.covered(b).any(|t| crate::economy::neighbours(t).iter().any(|n| net.contains(n)))
        };
        if consumer != 1 && !near(consumer) {
            return Some(format!("no road reaches the {cp}; lay one to the camp's roads"));
        }
        if let Some(u) = roads.unserved.iter().find(|u| u.no_road && u.to == consumer) {
            let _ = u;
            return Some(format!("no road joins the {cp} to the {from}; the roads are cut or do not meet"));
        }
        if let Some(&(tile, _)) = roads.jams.first() {
            return Some(format!("{} is jammed; lay a second road, a rollerway or a rail", self.road_name(tile)));
        }
        if roads.free == 0 {
            return Some(format!(
                "the {yard_name} has no free carts; all {} are out, and more carts or another yard would clear it",
                roads.carts
            ));
        }
        let waiting = self.buildings[&holders[0]].output.values().sum::<u32>().max(self.amount_of_camp(holders[0]));
        (roads.unserved.iter().any(|u| u.to == consumer)).then(|| format!("{waiting} goods wait at the {from} for a cart to the {cp}"))
    }

    fn amount_of_camp(&self, id: u32) -> u32 {
        if id == 1 { self.stock.values().sum() } else { 0 }
    }

    /// Road tiles, for reading only.
    fn roads_for_clerk(&self) -> &BTreeSet<Tile> {
        &self.roads
    }

    /// A full store: what should be taking its goods and is not.
    fn explain_outflow(&self, c: &Catalog, report: &Report, producer: u32, chain: &mut Chain) {
        let rows = self.line_report(c);
        let out: Vec<_> = rows.iter().filter(|r| r.from == producer).collect();
        for r in out {
            let spec = &c.lines[r.kind.key()];
            match r.state {
                LineState::Saturated => {
                    chain.push(format!(
                        "its {} line is at capacity ({} a minute); add vehicles or a faster line",
                        spec.name, r.capacity_per_min
                    ));
                    return;
                }
                LineState::Backed => {
                    let to = c.plans[&self.buildings[&r.to].plan].name.to_lowercase();
                    chain.push(format!("its {} line is backed up at the {to}", spec.name));
                    return;
                }
                LineState::Cut => {
                    chain.push(format!("its {} line is cut", spec.name));
                    return;
                }
                _ => {}
            }
        }
        // The roads: someone must want the goods, and a cart must be free to take them.
        if let Some(why) = self.cart_trouble(c, report, &[producer], 1) {
            chain.push(why);
        } else {
            chain.push("the carts are slow to clear it".into());
        }
    }
}
