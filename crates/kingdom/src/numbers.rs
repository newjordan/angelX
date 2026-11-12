//! Numbers on demand: what a seat that points at a works, a line or a store is
//! told of it (a card), what the party made, used and held of each good over the
//! last hour (a production screen), and the ranked list of what is choking the
//! works, each with the place to go to. All read from the books and the
//! Clerk's structured causes, and changing nothing; the host makes them for the
//! one seat that asked and sends them to that seat only.
use crate::books::HOUR;
use crate::clerk::Cause;
use crate::*;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// How a line of a card reads: plain, well, strained or stopped.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum Tone {
    #[default]
    Plain,
    Good,
    Warn,
    Bad,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Row {
    pub label: String,
    pub text: String,
    pub tone: Tone,
}

/// What pointing at a thing says of it.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Card {
    pub title: String,
    /// "WORKS", "LINE", "STORE" or "PLAN".
    pub kind: String,
    pub at: Tile,
    pub rows: Vec<Row>,
}

/// One good over the last hour, oldest minute first.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct GoodLine {
    pub good: String,
    pub name: String,
    pub made: Vec<u16>,
    pub used: Vec<u16>,
    pub stock: Vec<u32>,
    pub now: u32,
    pub made_hour: u32,
    pub used_hour: u32,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Production {
    /// Whole minutes the books hold (at most 59).
    pub minutes: u8,
    pub goods: Vec<GoodLine>,
}

/// One thing holding the works back, and where it is.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Choke {
    pub text: String,
    /// The works at fault (a plan id), the good in the way, and where to go to see it.
    pub plan: String,
    pub good: String,
    pub at: Tile,
    /// Seconds the works lost to it over the last few minutes.
    pub lost: u32,
    pub tone: Tone,
    /// This is what the Keep's chain is waiting on.
    pub keep: bool,
}

/// What one seat is shown, made for it alone.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Numbers {
    /// The sim second it was made.
    pub at: u64,
    pub card: Option<Card>,
    pub production: Option<Production>,
    pub chokes: Vec<Choke>,
}

fn name_of(c: &Catalog, good: &str) -> String {
    c.resources.get(good).map_or_else(|| good.to_string(), |r| r.name.clone())
}

fn rate(n: f32) -> String {
    if n >= 10.0 { format!("{:.0}", n) } else if n >= 0.05 { format!("{:.1}", n) } else { "0".into() }
}

fn goods_per_min(c: &Catalog, goods: &Stock, per_min: f32) -> String {
    goods.iter().map(|(g, n)| format!("{} {}", rate(*n as f32 * per_min), name_of(c, g).to_lowercase())).collect::<Vec<_>>().join(", ")
}

fn row(label: &str, text: impl Into<String>, tone: Tone) -> Row {
    Row { label: label.to_string(), text: text.into(), tone }
}

impl Kingdom {
    /// The card of whatever is at `tile`: a plan, a works, a store, a line.
    pub fn card(&self, c: &Catalog, tile: Tile) -> Option<Card> {
        if let Some(id) = self.ghost_at(c, tile) {
            return Some(self.plan_card(c, id));
        }
        if let Some(id) = self.at(tile) {
            return Some(self.works_card(c, id));
        }
        let hit = self.lines.iter().find(|(_, l)| l.path.contains(&tile)).map(|(&id, _)| id)?;
        Some(self.line_card(c, hit, tile))
    }

    fn plan_card(&self, c: &Catalog, id: u32) -> Card {
        let g = &self.plans.ghosts[&id];
        let cost = self.ghost_cost(c, &g.plan);
        let mut rows = vec![row(
            "PRICE",
            cost.iter().map(|(r, n)| format!("{n} {}", name_of(c, r).to_lowercase())).collect::<Vec<_>>().join(", "),
            Tone::Plain,
        )];
        let crew = self.plans.crews.iter().find(|w| w.ghost == id);
        match (crew, self.ghost_wait(c, id)) {
            (Some(w), _) => rows.push(row("HANDS", if w.leg == crate::ghosts::Leg::Raising { "RAISING IT NOW" } else { "WALKING OUT WITH THE MATERIALS" }, Tone::Good)),
            (None, Some(why)) => rows.push(row("WAITING", why, Tone::Warn)),
            (None, None) => rows.push(row("HANDS", "THE NEXT FREE CREW WILL TAKE IT", Tone::Good)),
        }
        rows.push(row("URGENCY", ["LOW", "USUAL", "URGENT"][usize::from(g.priority.min(2))], Tone::Plain));
        Card { title: crate::ghosts::plan_name(c, &g.plan), kind: "PLAN".into(), at: g.tile, rows }
    }

    fn works_card(&self, c: &Catalog, id: u32) -> Card {
        let b = &self.buildings[&id];
        let p = &c.plans[&b.plan];
        let report = self.report(c);
        let status = report.status.get(&id).cloned().unwrap_or(Status::Idle);
        let store = id == 1 || p.hub || p.yard || (p.storage > 0 && self.job(c, b).is_none());
        let mut rows = Vec::new();
        let tone = match status {
            Status::Working | Status::Target => Tone::Good,
            Status::Idle | Status::Construction | Status::Paused | Status::Unclaimed => Tone::Plain,
            Status::Inputs | Status::Storage | Status::Workers | Status::Shaft => Tone::Warn,
            _ => Tone::Bad,
        };
        let state = if status == Status::Construction {
            format!("BEING RAISED: {} S LEFT", b.construction)
        } else {
            status.label().to_string()
        };
        rows.push(row("STATE", state, tone));
        if store {
            return self.store_card(c, id, rows);
        }
        let job = self.job(c, b);
        if let Some((input, output, _)) = &job {
            rows.push(row("MAKES", output.keys().map(|g| name_of(c, g)).collect::<Vec<_>>().join(", "), Tone::Plain));
            let w = self.books.window(id, self.second);
            let mins = ((w.seconds + 30) / 60).max(1);
            if w.seconds < 20 {
                rows.push(row("PER MIN", "NO FIGURES YET: GIVE IT A MINUTE", Tone::Plain));
            } else {
                let per = w.per_min();
                if !input.is_empty() {
                    rows.push(row("IN /MIN", goods_per_min(c, input, per), Tone::Plain));
                }
                rows.push(row("OUT /MIN", goods_per_min(c, output, per), if per > 0.0 { Tone::Good } else { Tone::Warn }));
                rows.push(row("OVER", format!("THE LAST {mins} MIN: AT WORK {}%, HELD UP {}%", w.worked * 100 / w.seconds.max(1), w.lost_percent()), Tone::Plain));
            }
            let fills: Vec<String> = self
                .machine_report(c)
                .into_iter()
                .filter(|m| m.id == id)
                .flat_map(|m| {
                    let ins = m.input_fill.into_iter().map(|(g, have, cap)| format!("IN {} {have}/{cap}", name_of(c, &g).to_lowercase()));
                    let outs = m.output_fill.into_iter().map(|(g, have, cap)| format!("OUT {} {have}/{cap}", name_of(c, &g).to_lowercase()));
                    ins.chain(outs).collect::<Vec<_>>()
                })
                .collect();
            if !fills.is_empty() {
                rows.push(row("STOCK", fills.join("  "), Tone::Plain));
            }
        } else if p.housing > 0 {
            rows.push(row("BEDS", format!("{} HOUSED OF {}", report.population.housed.min(p.housing), p.housing), Tone::Plain));
        } else if p.stalls > 0 {
            rows.push(row("CARTS", format!("{} OF {} STALLS", b.carts.len(), p.stalls), Tone::Plain));
        } else if p.shaft > 0 {
            rows.push(row("LIFTS", format!("{} BLOCKS A MINUTE", p.shaft), Tone::Plain));
        }
        if let Some(power) = &p.power {
            if power.is_consumer() && (b.power_upgrade || report.energy.consumer(power.kind, id).is_some_and(|x| x.status == crate::PowerStatus::Powered)) {
                let t = report.energy.consumer(power.kind, id).map(|x| x.status);
                rows.push(row(
                    "POWER",
                    t.map_or("NOT FITTED", |s| s.label()),
                    if t == Some(crate::PowerStatus::Powered) { Tone::Good } else { Tone::Warn },
                ));
            } else {
                rows.push(row("POWER", format!("MAKES {} UNITS{}", power.capacity, if b.power_enabled { "" } else { " (SWITCHED OFF)" }), Tone::Plain));
            }
        }
        if p.workers > 0 {
            let on = report.population.assigned.get(&id).copied().unwrap_or(0);
            let idle = report.population.waiting.get(&id).copied().unwrap_or(0);
            rows.push(row(
                "CREW",
                format!("{on} OF {}{}", p.workers, if idle > 0 { format!(" ({idle} WAITING ON GOODS)") } else { String::new() }),
                if on >= p.workers { Tone::Good } else { Tone::Warn },
            ));
        }
        if !matches!(status, Status::Working | Status::Idle | Status::Construction | Status::Paused) {
            // What it waits for is the thing to read first: straight under its state.
            let why = self.clerk_why_idle(c, id);
            let short: Vec<&str> = why.split("; ").take(2).collect();
            rows.insert(1, row("WAITING", short.join("; "), tone));
        }
        Card { title: p.name.clone(), kind: "WORKS".into(), at: b.tile, rows }
    }

    fn store_card(&self, c: &Catalog, id: u32, mut rows: Vec<Row>) -> Card {
        let b = &self.buildings[&id];
        let p = &c.plans[&b.plan];
        let mut held: Vec<(String, u32)> = if id == 1 || self.sites.values().any(|s| s.yard == id) {
            self.stock.iter().map(|(g, n)| (g.clone(), *n)).collect()
        } else {
            b.input.iter().chain(&b.output).map(|(g, n)| (g.clone(), *n)).collect()
        };
        held.sort_by_key(|(g, n)| (std::cmp::Reverse(*n), g.clone()));
        let capacity = self.report(c).capacity;
        if id == 1 {
            rows.push(row("ROOM", format!("{capacity} OF EACH GOOD"), Tone::Plain));
        } else if p.storage > 0 {
            rows.push(row("HOLDS", format!("UP TO {} A GOOD", p.storage), Tone::Plain));
        }
        for (g, n) in held.into_iter().take(5) {
            let m = self.books.minutes(&g);
            let trend = if id == 1 && m.len() >= 3 {
                let (a, z) = (i64::from(m[m.len() - 3].2), i64::from(m[m.len() - 1].2));
                // (Only a stock that is moving says which way.)
                match (z - a) / 2 {
                    0 => String::new(),
                    d => format!("  {d:+}/MIN"),
                }
            } else {
                String::new()
            };
            rows.push(row("", format!("{n} {}{trend}", name_of(c, &g).to_lowercase()), Tone::Plain));
        }
        Card { title: p.name.clone(), kind: "STORE".into(), at: b.tile, rows }
    }

    fn line_card(&self, c: &Catalog, id: u32, tile: Tile) -> Card {
        let l = &self.lines[&id];
        let spec = &c.lines[l.kind.key()];
        let plan = |b: u32| self.buildings.get(&b).map_or("?".to_string(), |b| c.plans[&b.plan].name.clone());
        let row_of = self.line_report(c).into_iter().find(|r| r.id == id);
        let mut rows = Vec::new();
        let (state, tone) = match row_of.as_ref().map(|r| r.state) {
            Some(LineState::Flowing) => ("FLOWING", Tone::Good),
            Some(LineState::Saturated) => ("AT ITS LIMIT", Tone::Warn),
            Some(LineState::Starved) => ("NOTHING TO CARRY", Tone::Warn),
            Some(LineState::Backed) => ("BACKED UP AT THE FAR END", Tone::Warn),
            Some(LineState::Cut) => ("THE WAY IS CUT", Tone::Bad),
            Some(LineState::Unfed) => ("NO FODDER", Tone::Bad),
            None => ("?", Tone::Plain),
        };
        rows.push(row("STATE", state, tone));
        rows.push(row("RUNS", format!("{} TO {}", plan(l.from), plan(l.to)), Tone::Plain));
        if let Some(r) = &row_of {
            rows.push(row("PER MIN", format!("{} CARRIED OF {} IT COULD", r.observed_per_min, r.capacity_per_min), if r.observed_per_min * 10 >= r.capacity_per_min * 9 { Tone::Warn } else { Tone::Plain }));
            rows.push(row("FLEET", format!("{} {}S ON {} TILES", r.vehicles, spec.vehicle.to_uppercase(), r.length), Tone::Plain));
        }
        let note = match row_of.as_ref().map(|r| r.state) {
            Some(LineState::Starved) => Some(format!("THE {} HAS NOTHING TO SEND", plan(l.from).to_uppercase())),
            Some(LineState::Backed) => Some(format!("THE {} CANNOT TAKE ANY MORE", plan(l.to).to_uppercase())),
            Some(LineState::Saturated) => Some("ADD VEHICLES OR LAY A SECOND LINE".to_string()),
            _ => None,
        };
        if let Some(n) = note {
            rows.push(row("WAITING", n, Tone::Warn));
        }
        Card { title: spec.name.clone(), kind: "LINE".into(), at: tile, rows }
    }

    /// The last hour of the goods the party has dealt in, busiest first.
    pub fn production(&self, c: &Catalog, limit: usize) -> Production {
        let closed = self.books.minute.saturating_sub(self.books.began).min(HOUR as u64 - 1) as u8;
        let mut goods: Vec<GoodLine> = self
            .books
            .goods
            .keys()
            .filter_map(|g| {
                let m = self.books.minutes(g);
                let (made_hour, used_hour): (u32, u32) = m.iter().fold((0, 0), |a, x| (a.0 + u32::from(x.0), a.1 + u32::from(x.1)));
                let busy = made_hour > 0 || used_hour > 0;
                (busy && c.resources.contains_key(g)).then(|| GoodLine {
                    good: g.clone(),
                    name: name_of(c, g),
                    made: m.iter().map(|x| x.0).collect(),
                    used: m.iter().map(|x| x.1).collect(),
                    stock: m.iter().map(|x| x.2).collect(),
                    now: self.amount(g),
                    made_hour,
                    used_hour,
                })
            })
            .collect();
        goods.sort_by(|a, b| (b.made_hour + b.used_hour).cmp(&(a.made_hour + a.used_hour)).then(a.name.cmp(&b.name)));
        goods.truncate(limit);
        Production { minutes: closed, goods }
    }

    /// What is choking the works, worst first: each cause the Clerk finds behind a
    /// starved or stopped works, with the seconds it has cost and where it stands.
    pub fn bottlenecks(&self, c: &Catalog, limit: usize) -> Vec<Choke> {
        let report = self.report(c);
        let machines: BTreeMap<u32, crate::MachineRow> = self.machine_report(c).into_iter().map(|m| (m.id, m)).collect();
        let mut found: BTreeMap<(String, String, String), Choke> = BTreeMap::new();
        // What each missing good comes down to, asked of the Clerk once.
        let mut roots: BTreeMap<String, Option<Cause>> = BTreeMap::new();
        let mut put = |key: (String, String, String), choke: Choke| match found.get_mut(&key) {
            Some(e) => e.lost += choke.lost,
            None => {
                found.insert(key, choke);
            }
        };
        let plan_tile = |plan: &str| -> Option<Tile> {
            self.buildings
                .iter()
                .filter(|(_, b)| b.plan == plan)
                .max_by_key(|(id, _)| self.books.window(**id, self.second).lost)
                .map(|(_, b)| b.tile)
        };
        let pname = |plan: &str| c.plans.get(plan).map_or(plan.to_string(), |p| p.name.to_uppercase());
        let gname = |g: &str| name_of(c, g).to_uppercase();
        // Every works that is held up, and why.
        for (&id, b) in &self.buildings {
            if id == 1 || b.construction > 0 {
                continue;
            }
            let status = report.status.get(&id).cloned().unwrap_or(Status::Idle);
            let w = self.books.window(id, self.second);
            let lost = w.lost.max(u32::from(!matches!(status, Status::Working | Status::Idle | Status::Paused | Status::Target | Status::Unclaimed | Status::Construction)));
            if lost == 0 {
                continue;
            }
            match status {
                Status::Inputs => {
                    let Some(m) = machines.get(&id) else { continue };
                    let Some((good, _, _)) = m.input_fill.iter().find(|(_, have, need)| have < need) else { continue };
                    let root = roots.entry(good.clone()).or_insert_with(|| self.causes_for(c, good).last().cloned()).clone();
                    let Some(root) = root else { continue };
                    let (text, plan, at) = self.cause_words(c, &root, b.tile);
                    put((format!("{root:?}"), plan.clone(), good.clone()), Choke { text, plan, good: good.clone(), at, lost, tone: Tone::Warn, keep: false });
                }
                Status::Storage => {
                    let good = machines.get(&id).and_then(|m| m.output_fill.first().map(|x| x.0.clone())).unwrap_or_default();
                    put(
                        ("full".into(), b.plan.clone(), good.clone()),
                        Choke { text: format!("{} IS FULL OF {}: NOTHING TAKES IT AWAY", pname(&b.plan), gname(&good)), plan: b.plan.clone(), good, at: b.tile, lost, tone: Tone::Warn, keep: false },
                    );
                }
                Status::Idle | Status::Working | Status::Paused | Status::Target | Status::Unclaimed | Status::Construction => {}
                other => {
                    put(
                        (other.label().to_string(), b.plan.clone(), String::new()),
                        Choke { text: format!("{}: {}", pname(&b.plan), other.label()), plan: b.plan.clone(), good: String::new(), at: b.tile, lost, tone: Tone::Bad, keep: false },
                    );
                }
            }
        }
        // What the Keep waits on is always among them, and counts for more.
        if let Some(site) = self.keep_site() {
            if let Some(root) = self.clerk(c, site).causes.last().cloned() {
                let at = self.buildings.get(&self.sites[&site].yard).map_or(self.camp(), |y| y.tile);
                let (text, plan, tile) = self.cause_words(c, &root, at);
                let good = match &root {
                    Cause::NoMaker { good } | Cause::Capacity { good, .. } | Cause::Carriage { good, .. } | Cause::Full { good, .. } | Cause::WrongRecipe { good, .. } => good.clone(),
                    _ => String::new(),
                };
                let key = (format!("{root:?}"), plan.clone(), good.clone());
                let tile = plan_tile(&plan).unwrap_or(tile);
                match found.get_mut(&key) {
                    Some(e) => {
                        e.keep = true;
                        e.lost += 120;
                    }
                    None => {
                        found.insert(key, Choke { text, plan, good, at: tile, lost: 120, tone: Tone::Warn, keep: true });
                    }
                }
            }
        }
        let mut out: Vec<Choke> = found.into_values().collect();
        out.sort_by(|a, b| b.lost.cmp(&a.lost).then(b.keep.cmp(&a.keep)).then(a.text.cmp(&b.text)));
        out.truncate(limit);
        out
    }

    /// A cause in words, the plan at fault and where to find it (`near` when no such works stands).
    fn cause_words(&self, c: &Catalog, cause: &Cause, near: Tile) -> (String, String, Tile) {
        let pname = |plan: &str| c.plans.get(plan).map_or(plan.to_string(), |p| p.name.to_uppercase());
        let gname = |g: &str| name_of(c, g).to_uppercase();
        let tile_of = |plan: &str| {
            self.buildings
                .values()
                .filter(|b| b.plan == plan)
                .min_by_key(|b| b.tile.0.abs_diff(near.0) + b.tile.1.abs_diff(near.1))
                .map_or(near, |b| b.tile)
        };
        match cause {
            Cause::Capacity { plan, good } => (format!("{} IS THE LIMIT ON {}: BUILD ANOTHER", pname(plan), gname(good)), plan.clone(), tile_of(plan)),
            Cause::NoMaker { good } => {
                let menu = if self.world.is_hillside() { &c.hillside_menu } else { &c.menu };
                let maker = menu.iter().find(|p| c.plans[*p].recipes.iter().any(|r| c.recipes[r].outputs.contains_key(good))).cloned();
                let text = match &maker {
                    Some(m) => format!("NOTHING MAKES {}: BUILD A {}", gname(good), pname(m)),
                    None => format!("{} COMES FROM A CLAIM WON IN THE DELVE", gname(good)),
                };
                (text, maker.unwrap_or_default(), near)
            }
            Cause::NoCrew { plan } => (format!("{} HAS NO CREW: HOMES AND FOOD BRING PEOPLE", pname(plan)), plan.clone(), tile_of(plan)),
            Cause::Carriage { plan, good } => (format!("{} WAITS AT THE {}: NO CART OR LINE TAKES IT", gname(good), pname(plan)), plan.clone(), tile_of(plan)),
            Cause::Full { plan, good } => (format!("{} IS FULL OF {}: NOTHING TAKES IT AWAY", pname(plan), gname(good)), plan.clone(), tile_of(plan)),
            Cause::Stopped { plan, why } => (format!("{}: {}", pname(plan), why.to_uppercase()), plan.clone(), tile_of(plan)),
            Cause::WrongRecipe { plan, good } => (format!("{} IS NOT SET TO MAKE {}", pname(plan), gname(good)), plan.clone(), tile_of(plan)),
        }
    }
}
