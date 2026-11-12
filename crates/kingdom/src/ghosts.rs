//! Ghost plans and the hands that raise them. Anything the hillside builds can
//! be laid out as a ghost, paid for or not: the plan is the party's, shared by
//! every seat, and the camp's own hands (a crew of a labourer to carry and a
//! carpenter to raise, one more for each carpenter's shop that stands) work
//! through it, the most urgent first and the nearest of those first. A crew takes
//! what a plan costs from the camp's stock as it sets out, so nothing is spent
//! twice, walks it to the ground, raises it, and walks back. A plan the camp
//! cannot pay for waits, and says what it waits for.
//!
//! Also here, because they are the same kind of thing (what a player lays and
//! may want back): taking a placement back with its price, and copying a group
//! of works into a pattern that is kept with the realm.
//!
//! Pure and small: it reads the catalogue and the kingdom and changes only the
//! kingdom, once a second (`Kingdom::raise_second`).
use crate::economy::{add, covers, take};
use crate::*;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

/// The most plans laid at once.
pub const MAX_GHOSTS: usize = 256;
/// The most crews at work, however many shops stand.
pub const MAX_CREWS: usize = 4;
/// Tiles a crew covers in a second, carrying or not.
pub const PACE: u32 = 2;
/// The most copies a realm keeps.
pub const MAX_COPIES: usize = 8;
/// The names a copy is kept under, in the order they are made.
pub const COPY_NAMES: [&str; MAX_COPIES] = ["copy-a", "copy-b", "copy-c", "copy-d", "copy-e", "copy-f", "copy-g", "copy-h"];
/// The pseudo-plans of the ground itself.
pub const ROAD: &str = "road";
pub const BRIDGE: &str = "bridge";
/// The pseudo-plans of mending what a raid cut: a piece of rollerway (`mend-roller:<piece>`,
/// facing the way `face` says) and a tile of rail. The camp's hands raise them like any plan
/// (`defence`).
pub const MEND_ROLLER: &str = "mend-roller:";
pub const MEND_RAIL: &str = "mend-rail";

/// Whether `plan` is one of the ground's own pseudo-plans, or a mend.
pub fn is_pseudo(plan: &str) -> bool {
    plan == ROAD || plan == BRIDGE || plan == MEND_RAIL || plan.starts_with(MEND_ROLLER)
}

/// One plan laid on the ground.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Ghost {
    /// A plan's id, or [`ROAD`] or [`BRIDGE`].
    pub plan: String,
    pub tile: Tile,
    /// 0 low, 1 usual, 2 urgent.
    pub priority: u8,
    /// The seat that laid it.
    pub by: u32,
    /// The way a swape planned here is to face when it is raised (0 north, 1 east, 2 south,
    /// 3 west): set by turning it (`Command::Face`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub face: Option<u8>,
}

/// Where a crew is in its round.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Leg {
    /// At the camp, with nothing to do or nothing it can be paid for.
    Idle,
    /// Walking out with the materials.
    Out,
    /// At the plan, raising it.
    Raising,
    /// Walking back, empty-handed.
    Back,
}

/// A crew of the camp's hands: a labourer who carries and a carpenter who raises.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Crew {
    /// The plan it is serving, or 0.
    pub ghost: u32,
    pub leg: Leg,
    /// Seconds left in this leg, and the leg's whole length.
    pub left: u16,
    pub total: u16,
    /// Where it is going (or came from): the plan's tile.
    pub tile: Tile,
    /// What it carries out, taken from the camp's stock.
    pub carrying: Stock,
    /// The plan it carries.
    pub plan: String,
}

impl Crew {
    fn idle() -> Self {
        Crew { ghost: 0, leg: Leg::Idle, left: 0, total: 0, tile: (0, 0), carrying: Stock::new(), plan: String::new() }
    }
    /// How far through its leg, 0 to 1.
    pub fn along(&self) -> f32 {
        if self.total == 0 { 1.0 } else { 1.0 - f32::from(self.left) / f32::from(self.total) }
    }
}

/// The party's plans and the hands at them.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct Plans {
    pub next: u32,
    pub ghosts: BTreeMap<u32, Ghost>,
    pub crews: Vec<Crew>,
    /// Plans raised, over the whole game.
    pub raised: u32,
}

impl Plans {
    pub fn is_empty(&self) -> bool {
        self.next == 0 && self.ghosts.is_empty() && self.crews.is_empty() && self.raised == 0
    }
    pub fn valid(&self, k: &Kingdom, c: &Catalog) -> bool {
        self.ghosts.len() <= MAX_GHOSTS
            && self.crews.len() <= MAX_CREWS
            && self.ghosts.keys().all(|id| *id > 0 && *id <= self.next)
            && self.ghosts.values().all(|g| {
                g.priority <= 2 && k.inside(g.tile) && (is_pseudo(&g.plan) || c.plans.contains_key(&g.plan))
            })
            && self.crews.iter().all(|w| w.ghost == 0 || self.ghosts.contains_key(&w.ghost) || w.leg == Leg::Back)
            && self.crews.iter().all(|w| c.valid_stock(&w.carrying))
    }
}

/// A thing a player laid, that they may want back.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Laid {
    Ghost(u32),
    Road(Tile),
    Bridge(Tile),
    Building(u32),
    Line(u32),
    /// A tile of rollerway or of rail.
    Roller(Tile),
    Rail(Tile),
}

fn walk_seconds(from: Tile, to: Tile) -> u16 {
    let d = u32::from(from.0.abs_diff(to.0)) + u32::from(from.1.abs_diff(to.1));
    d.div_ceil(PACE).clamp(1, 600) as u16
}

impl Kingdom {
    /// What a plan costs.
    pub fn ghost_cost(&self, c: &Catalog, plan: &str) -> Stock {
        match plan {
            ROAD => Stock::from([("timber".to_string(), 1)]),
            BRIDGE => Stock::from([("plank".to_string(), 4), ("stone".to_string(), 2)]),
            MEND_RAIL => self.rail_spec(c).tile_cost.clone(),
            p if p.starts_with(MEND_ROLLER) => {
                Piece::from_key(&p[MEND_ROLLER.len()..]).and_then(|piece| c.rollers.get(piece.key())).map(|r| r.cost.clone()).unwrap_or_default()
            }
            p => c.plans.get(p).map(|p| p.cost.clone()).unwrap_or_default(),
        }
    }

    /// The tiles a plan would cover.
    pub fn ghost_tiles(&self, c: &Catalog, g: &Ghost) -> Vec<Tile> {
        match g.plan.as_str() {
            ROAD | BRIDGE | MEND_RAIL => vec![g.tile],
            p if p.starts_with(MEND_ROLLER) => vec![g.tile],
            p => c.plans.get(p).map_or_else(|| vec![g.tile], |p| self.footprint(p, g.tile)),
        }
    }

    /// The plan whose ground includes `tile`, if one is laid there.
    pub fn ghost_at(&self, c: &Catalog, tile: Tile) -> Option<u32> {
        self.plans.ghosts.iter().find(|(_, g)| g.tile == tile || self.ghost_tiles(c, g).contains(&tile)).map(|(&id, _)| id)
    }

    /// Whether the ground takes the plan (today, price aside).
    pub fn ghost_land(&self, c: &Catalog, g: &Ghost) -> Result<(), String> {
        match g.plan.as_str() {
            ROAD => {
                if !self.inside(g.tile) || self.roads.contains(&g.tile) || self.at(g.tile).is_some_and(|id| !c.plans[&self.buildings[&id].plan].passable) {
                    return Err("choose an empty plot".into());
                }
                self.check_road_site(g.tile)
            }
            BRIDGE => {
                if !self.world.owned(g.tile) || !self.world.terrain(g.tile).water() || self.roads.contains(&g.tile) || self.at(g.tile).is_some() {
                    return Err("choose an empty river crossing in your land".into());
                }
                Ok(())
            }
            MEND_RAIL => {
                if self.rails.cells.contains_key(&g.tile) {
                    return Err("the rail is laid again".into());
                }
                self.check_rail_site(c, g.tile)
            }
            p if p.starts_with(MEND_ROLLER) => {
                let piece = Piece::from_key(&p[MEND_ROLLER.len()..]).ok_or("no such piece of rollerway")?;
                if self.rollers.cells.contains_key(&g.tile) {
                    return Err("the rollerway is laid again".into());
                }
                self.check_roller_site(c, piece, g.tile)
            }
            p => self.placement(c, p, g.tile).land,
        }
    }

    /// What holds a plan back from being raised now, in a few words: the charter or the
    /// goods it lacks, or the ground. `None` when the camp could send a crew for it.
    pub fn ghost_wait(&self, c: &Catalog, id: u32) -> Option<String> {
        let g = self.plans.ghosts.get(&id)?;
        if let Err(why) = self.ghost_land(c, g) {
            return Some(why);
        }
        if let Some(p) = c.plans.get(&g.plan) {
            if let Some(r) = p.requires.iter().find(|r| !self.researched.contains(*r)) {
                let name = c.tech(r).map_or(r.clone(), |t| t.name.clone());
                return Some(format!("needs the {name} charter"));
            }
        }
        let cost = self.ghost_cost(c, &g.plan);
        if !covers(&self.stock, &cost) {
            return Some(format!("needs {}", self.short_of(c, &cost)));
        }
        None
    }

    /// What of `cost` the camp lacks, by name: "12 plank, 4 block".
    pub fn short_of(&self, c: &Catalog, cost: &Stock) -> String {
        let lacking: Stock = cost.iter().filter_map(|(r, n)| (self.amount(r) < *n).then(|| (r.clone(), n - self.amount(r)))).collect();
        lacking
            .iter()
            .map(|(r, n)| format!("{n} {}", c.resources.get(r).map_or(r.as_str(), |x| x.name.as_str()).to_lowercase()))
            .collect::<Vec<_>>()
            .join(", ")
    }

    /// Lay a plan on the ground. Anything the land takes, whatever the camp holds.
    pub fn plan_ghost(&mut self, c: &Catalog, plan: &str, tile: Tile, priority: u8, by: u32) -> Result<String, String> {
        if self.is_empty() {
            return Err("the kingdom is not founded".into());
        }
        if !is_pseudo(plan) && !c.plans.contains_key(plan) {
            return Err("unknown plan".into());
        }
        if self.plans.ghosts.len() >= MAX_GHOSTS {
            return Err(format!("{MAX_GHOSTS} plans are laid already: raise or cancel some"));
        }
        let g = Ghost { plan: plan.to_string(), tile, priority: priority.min(2), by, face: None };
        self.ghost_land(c, &g)?;
        let mine: BTreeSet<Tile> = self.ghost_tiles(c, &g).into_iter().collect();
        if self.plans.ghosts.values().any(|o| self.ghost_tiles(c, o).iter().any(|t| mine.contains(t))) {
            return Err("a plan is laid there already".into());
        }
        // A plan's ground is a whole footprint, free of roads and buildings as the sim knows it.
        if let Some(p) = c.plans.get(plan) {
            if p.connector.is_none() {
                self.check_footprint(c, plan, tile)?;
            }
        }
        self.plans.next = self.plans.next.checked_add(1).ok_or("plan limit")?;
        let id = self.plans.next;
        self.plans.ghosts.insert(id, g);
        let name = c.plans.get(plan).map_or(plan, |p| p.name.as_str());
        Ok(format!("{name} planned: the camp's hands will raise it"))
    }

    /// Take a plan off the ground. A crew already carrying for it brings the goods back.
    pub fn unplan(&mut self, id: u32) -> Result<String, String> {
        if self.plans.ghosts.remove(&id).is_none() {
            return Err("no such plan".into());
        }
        self.recall(id);
        Ok("Plan cancelled".into())
    }

    /// The crew serving plan `id` (if one is) puts down its load and walks home.
    pub(crate) fn recall(&mut self, id: u32) {
        let camp = self.camp();
        let mut back = Stock::new();
        for w in self.plans.crews.iter_mut().filter(|w| w.ghost == id) {
            for (r, n) in std::mem::take(&mut w.carrying) {
                add(&mut back, &r, n);
            }
            let there = walk_seconds(camp, w.tile);
            w.ghost = 0;
            w.leg = Leg::Back;
            w.total = there;
            w.left = there.min(w.left.max(1));
            w.plan.clear();
        }
        for (r, n) in back {
            add(&mut self.stock, &r, n);
        }
    }

    /// Set how urgent a plan is: 0 low, 1 usual, 2 urgent.
    pub fn rank_ghost(&mut self, id: u32, priority: u8) -> Result<String, String> {
        let g = self.plans.ghosts.get_mut(&id).ok_or("no such plan")?;
        g.priority = priority.min(2);
        Ok(["Plan set low", "Plan set usual", "Plan set urgent"][usize::from(g.priority)].into())
    }

    /// How many crews the camp can field: its own, and one for each carpenter's shop that stands.
    pub fn crews_wanted(&self, c: &Catalog) -> usize {
        let shops = self
            .buildings
            .values()
            .filter(|b| b.construction == 0 && !b.paused && c.plans.get(&b.plan).is_some_and(|p| p.trade == "carpenter"))
            .count();
        (1 + shops).min(MAX_CREWS)
    }

    fn raise_seconds(&self, c: &Catalog, plan: &str) -> u16 {
        match plan {
            ROAD => 1,
            BRIDGE => 4,
            p => c.plans.get(p).map_or(3, |p| (p.build_seconds / 2).max(3)),
        }
    }

    /// Once a second: crews fetch, walk, raise and come back. Costs nothing while no plan is laid.
    pub(crate) fn raise_second(&mut self, c: &Catalog) {
        if self.plans.ghosts.is_empty() && self.plans.crews.is_empty() {
            return;
        }
        // Plans the ground has taken back (someone raised a thing there by hand) are dropped.
        let serving: BTreeSet<u32> = self.plans.crews.iter().filter(|w| w.ghost != 0).map(|w| w.ghost).collect();
        let stale: Vec<u32> = self.plans.ghosts.iter().filter(|(id, g)| !serving.contains(*id) && self.ghost_land(c, g).is_err()).map(|(&id, _)| id).collect();
        for id in stale {
            self.plans.ghosts.remove(&id);
        }
        let want = self.crews_wanted(c);
        while self.plans.crews.len() < want {
            self.plans.crews.push(Crew::idle());
        }
        while self.plans.crews.len() > want {
            match self.plans.crews.iter().rposition(|w| w.leg == Leg::Idle) {
                Some(i) => {
                    self.plans.crews.remove(i);
                }
                None => break,
            }
        }
        if self.plans.ghosts.is_empty() && self.plans.crews.iter().all(|w| w.leg == Leg::Idle) {
            self.plans.crews.clear();
            return;
        }
        let camp = self.camp();
        for i in 0..self.plans.crews.len() {
            match self.plans.crews[i].leg {
                Leg::Idle => self.fetch(c, i, camp),
                Leg::Out => {
                    let w = &mut self.plans.crews[i];
                    w.left = w.left.saturating_sub(1);
                    if w.left == 0 {
                        let work = self.raise_seconds(c, &self.plans.crews[i].plan);
                        let w = &mut self.plans.crews[i];
                        w.leg = Leg::Raising;
                        w.total = work;
                        w.left = work;
                    }
                }
                Leg::Raising => {
                    let w = &mut self.plans.crews[i];
                    w.left = w.left.saturating_sub(1);
                    if w.left == 0 {
                        self.finish(c, i, camp);
                    }
                }
                Leg::Back => {
                    let w = &mut self.plans.crews[i];
                    w.left = w.left.saturating_sub(1);
                    if w.left == 0 {
                        *w = Crew::idle();
                    }
                }
            }
        }
    }

    /// Crew `i` takes the most urgent plan the camp can pay for, nearest first.
    fn fetch(&mut self, c: &Catalog, i: usize, camp: Tile) {
        let serving: BTreeSet<u32> = self.plans.crews.iter().filter(|w| w.ghost != 0).map(|w| w.ghost).collect();
        let pick = self
            .plans
            .ghosts
            .iter()
            .filter(|(id, _)| !serving.contains(*id))
            .filter(|(id, _)| self.ghost_wait(c, **id).is_none())
            .min_by_key(|(id, g)| (std::cmp::Reverse(g.priority), walk_seconds(camp, g.tile), **id))
            .map(|(&id, g)| (id, g.plan.clone(), g.tile));
        let Some((id, plan, tile)) = pick else { return };
        let cost = self.ghost_cost(c, &plan);
        take(&mut self.stock, &cost);
        let there = walk_seconds(camp, tile);
        self.plans.crews[i] = Crew { ghost: id, leg: Leg::Out, left: there, total: there, tile, carrying: cost, plan };
    }

    /// Crew `i` has raised its plan: the thing stands, built by the hands that carried it.
    fn finish(&mut self, c: &Catalog, i: usize, camp: Tile) {
        let (id, tile, plan) = {
            let w = &self.plans.crews[i];
            (w.ghost, w.tile, w.plan.clone())
        };
        // The goods go back to the camp's stock and are paid again by the building's own
        // command, so every rule that applies to a thing laid by hand applies to this.
        for (r, n) in std::mem::take(&mut self.plans.crews[i].carrying) {
            add(&mut self.stock, &r, n);
        }
        let had = self.plans.ghosts.remove(&id);
        if let Some(g) = &had {
            let made = match plan.as_str() {
                ROAD => self.execute(c, Command::Road(tile)),
                BRIDGE => self.execute(c, Command::Bridge(tile)),
                MEND_RAIL => self.execute(c, Command::Rail(tile)),
                p if p.starts_with(MEND_ROLLER) => match Piece::from_key(&p[MEND_ROLLER.len()..]) {
                    Some(piece) => self.execute(c, Command::Roller { piece, tile, dir: crate::Dir::from_index(g.face.unwrap_or(1)) }),
                    None => Err("no such piece of rollerway".into()),
                },
                p => self.execute(c, Command::Build { plan: p.to_string(), tile }),
            };
            if made.is_ok() {
                if let Some(face) = g.face {
                    // A swape planned facing a way is raised so.
                    let _ = self.execute(c, Command::Face { tile, dir: crate::Dir::from_index(face) });
                }
                self.plans.raised = self.plans.raised.saturating_add(1);
                if let Some(bid) = self.at(tile) {
                    if self.buildings.get(&bid).is_some_and(|b| b.plan == plan) {
                        self.buildings.get_mut(&bid).expect("it stands").construction = 0;
                    }
                }
            }
        }
        let back = walk_seconds(camp, tile);
        self.plans.crews[i] = Crew { ghost: 0, leg: Leg::Back, left: back, total: back, tile, carrying: Stock::new(), plan: String::new() };
    }

    /// The walk a crew is on, tile by tile from the camp to its plan: along the way east or
    /// west first, then north or south.
    pub fn crew_way(&self, w: &Crew) -> Vec<Tile> {
        let (a, b) = (self.camp(), w.tile);
        let mut way = vec![a];
        let mut t = a;
        while t.0 != b.0 {
            t.0 += (b.0 - t.0).signum();
            way.push(t);
        }
        while t.1 != b.1 {
            t.1 += (b.1 - t.1).signum();
            way.push(t);
        }
        way
    }

    // ------------------------------------------------------------ taking back ----

    /// Take back what was just laid: plans are cancelled; a road or a thing nobody has used
    /// yet comes down with every board paid for it back, and one that has worked comes down
    /// as a demolition does, for half.
    pub fn retract(&mut self, c: &Catalog, what: &[Laid]) -> Result<String, String> {
        let mut undone = 0u32;
        let mut gave = Stock::new();
        // (A thing taken back gives back what it took: the store had room for it once.)
        fn give(k: &mut Kingdom, gave: &mut Stock, r: &str, n: u32) {
            add(&mut k.stock, r, n);
            add(gave, r, n);
        }
        for item in what.iter().rev() {
            match item {
                Laid::Ghost(id) => {
                    if self.unplan(*id).is_ok() {
                        undone += 1;
                    }
                }
                Laid::Road(t) => {
                    if self.roads.remove(t) {
                        self.bridges.remove(t);
                        give(self, &mut gave, "timber", 1);
                        undone += 1;
                    }
                }
                Laid::Bridge(t) => {
                    if self.roads.remove(t) {
                        self.bridges.remove(t);
                        give(self, &mut gave, "plank", 4);
                        give(self, &mut gave, "stone", 2);
                        undone += 1;
                    }
                }
                Laid::Building(id) => {
                    let Some(b) = self.buildings.get(id) else { continue };
                    if *id == 1 || self.sites.values().any(|s| s.yard == *id) {
                        continue;
                    }
                    let fresh = b.input.is_empty() && b.output.is_empty() && b.progress == 0 && b.stats.worked == 0 && b.stats.starved == 0 && b.stats.backed_up == 0;
                    let (tile, cost) = (b.tile, c.plans[&b.plan].cost.clone());
                    let before = self.stock.clone();
                    if self.execute(c, Command::Demolish(tile)).is_ok() {
                        undone += 1;
                        for (r, n) in &self.stock {
                            let d = n.saturating_sub(before.get(r).copied().unwrap_or(0));
                            if d > 0 {
                                add(&mut gave, r, d);
                            }
                        }
                        if fresh {
                            for (r, n) in &cost {
                                give(self, &mut gave, r, n - n / 2);
                            }
                        }
                    }
                }
                Laid::Line(id) => {
                    if self.remove_line(*id) {
                        undone += 1;
                    }
                }
                // A tile of rollerway or rail comes up as a demolition has it (its goods go to the camp).
                Laid::Roller(t) | Laid::Rail(t) => {
                    let there = if matches!(item, Laid::Roller(_)) { self.roller_at(*t).is_some() } else { self.rails.cells.contains_key(t) };
                    if there && self.execute(c, Command::Demolish(*t)).is_ok() {
                        undone += 1;
                    }
                }
            }
        }
        if undone == 0 {
            return Err("nothing of it is left to take back".into());
        }
        let back = if gave.is_empty() {
            String::new()
        } else {
            format!(": {} back in the camp", gave.iter().map(|(r, n)| format!("{n} {r}")).collect::<Vec<_>>().join(", "))
        };
        Ok(format!("Took back {undone}{back}"))
    }

    // ---------------------------------------------------------------- copying ----

    /// The pattern called `name`: a copy the party kept, or one of the catalogue's.
    pub fn pattern<'a>(&'a self, c: &'a Catalog, name: &str) -> Option<&'a Pattern> {
        self.copies.get(name).or_else(|| c.patterns.get(name))
    }

    /// Copy every work, road and shaft whole inside the rectangle `a`..`b` (inclusive,
    /// either corner first) into the pattern `name`, offsets from its top left; lines
    /// between two copied works come with it. Kept with the realm.
    pub fn clip(&mut self, c: &Catalog, name: &str, a: Tile, b: Tile) -> Result<String, String> {
        if !COPY_NAMES.contains(&name) {
            return Err("no such place to keep a copy".into());
        }
        let (x0, x1, y0, y1) = (a.0.min(b.0), a.0.max(b.0), a.1.min(b.1), a.1.max(b.1));
        if i32::from(x1 - x0) > 40 || i32::from(y1 - y0) > 40 {
            return Err("copy a smaller patch: forty tiles each way at most".into());
        }
        let inside = |t: Tile| t.0 >= x0 && t.0 <= x1 && t.1 >= y0 && t.1 <= y1;
        let rel = |t: Tile| (t.0 - x0, t.1 - y0);
        let mut builds = Vec::new();
        let mut index = BTreeMap::new();
        for (&id, b) in &self.buildings {
            if id == 1 || self.sites.values().any(|s| s.yard == id) {
                continue;
            }
            let p = &c.plans[&b.plan];
            if p.connector.is_some() || p.replaces.is_some() || !self.covered(b).all(inside) {
                continue;
            }
            index.insert(id, builds.len());
            builds.push(PatternBuild { plan: b.plan.clone(), at: rel(b.tile), recipe: b.recipe.clone().filter(|r| p.recipes.contains(r)) });
        }
        let mut roads: Vec<(i16, i16)> = self.roads.iter().filter(|t| inside(**t) && !self.bridges.contains(*t)).map(|t| rel(*t)).collect();
        roads.sort_unstable();
        let mut shafts: Vec<(i16, i16)> = self.power.shafts.iter().filter(|t| inside(**t)).map(|t| rel(*t)).collect();
        shafts.sort_unstable();
        let lines: Vec<PatternLine> = self
            .lines
            .values()
            .filter_map(|l| Some(PatternLine { kind: l.kind, from: *index.get(&l.from)?, to: *index.get(&l.to)?, vehicles: l.fleet.len().min(64) as u16 }))
            .collect();
        if builds.is_empty() && roads.is_empty() {
            return Err("nothing of the works stands in that patch".into());
        }
        if builds.len() > 64 || roads.len() > 400 {
            return Err("that is too much to keep as one pattern".into());
        }
        let label = format!("Copy {}", name.trim_start_matches("copy-").to_uppercase());
        let words = format!("{} works, {} roads kept as {label}", builds.len(), roads.len());
        let p = Pattern { name: label, description: "A group of works the party copied.".into(), roads, shafts, builds, lines };
        self.copies.insert(name.to_string(), p);
        Ok(words)
    }

    /// Lay a pattern down as plans rather than as buildings: every work and road becomes a
    /// ghost the hands will raise. Lines between the works are not planned (they join
    /// buildings that stand), and the words say so.
    pub fn plan_stamp(&mut self, c: &Catalog, name: &str, origin: Tile, turns: u8, priority: u8, by: u32) -> Result<(String, Vec<u32>), String> {
        let p = self.pattern(c, name).ok_or("unknown pattern")?.clone();
        let at = |off: (i16, i16)| {
            let (dx, dy) = match turns % 4 {
                0 => off,
                1 => (-off.1, off.0),
                2 => (-off.0, -off.1),
                _ => (off.1, -off.0),
            };
            (origin.0 + dx, origin.1 + dy)
        };
        let want = p.builds.len() + p.roads.len();
        if self.plans.ghosts.len() + want > MAX_GHOSTS {
            return Err(format!("{MAX_GHOSTS} plans are laid already: raise or cancel some"));
        }
        // All or nothing: on a copy of the kingdom first.
        let mut trial = self.clone();
        let mut ids = Vec::new();
        for &r in &p.roads {
            let t = at(r);
            if trial.roads.contains(&t) || trial.ghost_at(c, t).is_some() {
                continue;
            }
            trial.plan_ghost(c, ROAD, t, priority, by)?;
            ids.push(trial.plans.next);
        }
        for b in &p.builds {
            trial.plan_ghost(c, &b.plan, at(b.at), priority, by)?;
            ids.push(trial.plans.next);
        }
        *self = trial;
        let lines = if p.lines.is_empty() { String::new() } else { format!("; its {} lines are laid by hand once the works stand", p.lines.len()) };
        Ok((format!("{} planned: {} works and roads{lines}", p.name, ids.len()), ids))
    }
}

/// How a plan reads in a sentence: its name.
pub fn plan_name(c: &Catalog, plan: &str) -> String {
    match plan {
        ROAD => "Road".into(),
        BRIDGE => "Bridge".into(),
        p => c.plans.get(p).map_or_else(|| p.to_string(), |p| p.name.clone()),
    }
}
