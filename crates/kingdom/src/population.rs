//! Named, persistent settlers. The economy supplies connected work and housing
//! sites; this module assigns people and budgets their needs, never goods output.
use crate::Stock;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

/// The trades a person can hold, in the order Site refers to them.
pub const TRADES: [&str; 23] = [
    "labourer",
    "quarryman",
    "banker-mason",
    "setter",
    "master-mason",
    "lime-burner",
    "collier",
    "woodsman",
    "sawyer",
    "carpenter",
    "smith",
    "bloomer",
    "roper",
    "carter",
    "ostler",
    "bargeman",
    "miller",
    "baker",
    "brewer",
    "herder",
    "tanner",
    "wheelwright",
    "clerk-of-works",
];
pub fn trade_index(id: &str) -> Option<u8> {
    TRADES.iter().position(|t| *t == id).map(|i| i as u8)
}

/// How the village lives: legacy valleys eat one `food` and hold no trades;
/// the hillside reads the catalogue's diet, trades and wages.
#[derive(Clone, Copy)]
pub struct Rules<'a> {
    pub catalog: Option<&'a crate::Catalog>,
}
impl<'a> Rules<'a> {
    pub fn legacy() -> Self {
        Self { catalog: None }
    }
    pub fn of(catalog: &'a crate::Catalog, hillside: bool) -> Self {
        Self { catalog: hillside.then_some(catalog) }
    }
    fn staple(&self) -> &str {
        self.catalog
            .map(|c| c.diet.staple.as_str())
            .filter(|s| !s.is_empty())
            .unwrap_or("food")
    }
    /// XP thresholds for apprentice, journeyman and master.
    fn xp(&self) -> [u32; 3] {
        self.catalog.map_or([300, 1800, 5400], |c| [c.works.apprentice_xp, c.works.journeyman_xp, c.works.master_xp])
    }
    pub fn skill(&self, xp: u32) -> u8 {
        let t = self.xp();
        u8::from(xp >= t[0]) + u8::from(xp >= t[1]) + u8::from(xp >= t[2])
    }
    pub fn xp_for(&self, level: u8) -> u32 {
        match level {
            0 => 0,
            n => self.xp()[usize::from(n.min(3)) - 1],
        }
    }
    /// Seconds of welcome before a newcomer arrives.
    fn arrival(&self) -> u16 {
        self.catalog.map_or(ARRIVAL_SECONDS, |c| c.works.arrival_seconds)
    }
    /// Meals of bread a newcomer's welcome needs in the camp for every person already
    /// there, the new one included: the camp must be able to feed the settlement a
    /// day, not merely be eating this minute.
    fn welcome_meals(&self) -> u32 {
        self.catalog.map_or(2, |c| u32::from(c.works.welcome_meals.max(1)))
    }
    fn per_supervisor(&self) -> usize {
        self.catalog.map_or(4, |c| usize::from(c.works.posts_per_supervisor.max(1)))
    }
    fn kin(&self, trade: &str) -> &[String] {
        self.catalog.and_then(|c| c.trades.get(trade)).map_or(&[], |t| t.kin.as_slice())
    }
}

pub const MAX_CITIZENS: usize = 256;
pub const MAX_SITES: usize = 512;
pub const MEAL_SECONDS: u16 = 90;
pub const ARRIVAL_SECONDS: u16 = 120;
pub const FOUNDERS: usize = 6;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Role {
    #[default]
    Builder,
    Farmer,
    Porter,
    Arcanist,
}
impl Role {
    pub fn label(self) -> &'static str {
        match self {
            Self::Builder => "Builder",
            Self::Farmer => "Farmer",
            Self::Porter => "Porter",
            Self::Arcanist => "Arcanist",
        }
    }
    pub fn asset(self) -> &'static str {
        match self {
            Self::Builder => "builder",
            Self::Farmer => "farmer",
            Self::Porter => "porter",
            Self::Arcanist => "arcanist",
        }
    }
    fn order(self) -> u8 {
        match self {
            Self::Farmer => 0,
            Self::Builder => 1,
            Self::Porter => 2,
            Self::Arcanist => 3,
        }
    }
}

/// A read-only projection of one building. Offline sites cannot hold homes or
/// jobs. Construction sites may expose one Builder job and no beds.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Site {
    pub id: u32,
    pub tile: (i16, i16),
    pub role: Role,
    pub jobs: u16,
    pub beds: u16,
    pub online: bool,
    pub priority: u8,
    /// The trade (an index into `TRADES`) and the level of the person who
    /// must supervise the work here.
    pub trade: Option<(u8, u8)>,
    /// How many of the people who last worked here stand idle at the post while
    /// it has no work this minute (it has no jobs): they are free for any post
    /// that can use them, and wait here otherwise, not at home.
    pub held: u16,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Citizen {
    pub name: String,
    pub home: Option<u32>,
    pub job: Option<u32>,
    pub role: Role,
    /// Zero means fed; one hundred means very hungry.
    pub hunger: u8,
    /// Rest and morale are full at one hundred.
    pub rest: u8,
    pub morale: u8,
    pub arrived: u64,
    /// Seconds of work at each trade; the level follows from the rules.
    #[serde(default)]
    pub trades: BTreeMap<String, u32>,
    /// Seconds the coat of livery they were issued still has to run.
    #[serde(default)]
    pub livery: u32,
    /// The post they stand idle at while it waits on goods and no running post
    /// wants them (`Site::held`). They have no job there: they count as idle
    /// for every other purpose, and are free for any post that needs them.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub waiting: Option<u32>,
}
impl Citizen {
    pub fn skill(&self, trade: &str, rules: &Rules) -> u8 {
        self.trades.get(trade).map_or(0, |&xp| rules.skill(xp))
    }
    /// Every level they hold, added: how much of a tradesman they are.
    pub fn skill_total(&self, rules: &Rules) -> u32 {
        self.trades.values().map(|&xp| u32::from(rules.skill(xp))).sum()
    }
    /// The best skill they have in anything.
    pub fn best(&self, rules: &Rules) -> (Option<&str>, u8) {
        self.trades
            .iter()
            .map(|(t, &xp)| (t.as_str(), rules.skill(xp)))
            .max_by_key(|&(t, l)| (l, std::cmp::Reverse(t.len())))
            .map_or((None, 0), |(t, l)| (Some(t), l))
    }
    /// Whether they can supervise work needing `trade` at `level`: skilled in
    /// it, or one level higher in a kindred trade.
    pub fn supervises(&self, trade: &str, level: u8, rules: &Rules) -> bool {
        self.skill(trade, rules) >= level
            || rules.kin(trade).iter().any(|k| self.skill(k, rules) > level)
    }
    /// Even an exhausted, hungry settler can grow food and build shelter.
    /// The whole workplace receives the average, avoiding alternating workers
    /// that could leave a two-person job permanently unable to complete.
    pub fn efficiency(&self) -> u16 {
        (100u16
            .saturating_sub(u16::from(self.hunger) / 2)
            .saturating_sub(u16::from(100u8.saturating_sub(self.rest)) / 4)
            .saturating_sub(u16::from(100u8.saturating_sub(self.morale)) / 4))
        .max(25)
    }
    pub fn activity(&self) -> Activity {
        if self.hunger >= 75 {
            Activity::Hungry
        } else if self.rest < 35 {
            Activity::Resting
        } else if self.job.is_some() {
            Activity::Working
        } else {
            Activity::Idle
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Activity {
    Working,
    Resting,
    Hungry,
    Idle,
}
impl Activity {
    pub fn label(self) -> &'static str {
        match self {
            Self::Working => "Working",
            Self::Resting => "Needs rest",
            Self::Hungry => "Needs food",
            Self::Idle => "Awaiting work",
        }
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct Population {
    pub schema: u8,
    pub next: u32,
    pub citizens: BTreeMap<u32, Citizen>,
    pub seconds: u64,
    pub meal_seconds: u16,
    pub arrival_seconds: u16,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CitizenView {
    pub id: u32,
    pub name: String,
    pub role: Role,
    pub activity: Activity,
    pub home: Option<u32>,
    pub job: Option<u32>,
    /// The post they stand idle at, waiting on goods.
    pub waiting: Option<u32>,
    pub target: (i16, i16),
    pub efficiency: u16,
    pub hunger: u8,
    pub rest: u8,
    pub morale: u8,
    /// Every trade they have any time at, with the level it has reached
    /// (0 none yet, 1 apprentice, 2 journeyman, 3 master), best first.
    pub trades: Vec<(String, u8)>,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct PopulationReport {
    pub population: u16,
    pub housed: u16,
    pub beds: u16,
    pub employed: u16,
    pub hungry: u16,
    pub idle: u16,
    pub food_per_meal: u16,
    pub food_eaten: u16,
    pub assigned: BTreeMap<u32, u16>,
    /// People standing idle at a post that waits on goods, by post.
    pub waiting: BTreeMap<u32, u16>,
    /// Percent work credit for each online site; zero when understaffed.
    pub efficiency: BTreeMap<u32, u16>,
    /// IDs that arrived during this mutation, empty in a read-only report.
    pub arrivals: Vec<u32>,
    pub citizens: Vec<CitizenView>,
}

fn checked_sites(sites: &[Site]) -> Result<BTreeMap<u32, Site>, String> {
    if sites.len() > MAX_SITES {
        return Err("Too many population sites".into());
    }
    let mut map = BTreeMap::new();
    for site in sites {
        if site.id == 0
            || site.id == u32::MAX
            || site.priority > 2
            || site.jobs > 64
            || site.beds > 256
            || site.tile.0 < 0
            || site.tile.1 < 0
            || map.insert(site.id, *site).is_some()
        {
            return Err("Invalid population site".into());
        }
    }
    Ok(map)
}

fn name(id: u32) -> String {
    const FIRST: &[&str] = &[
        "Alda", "Bram", "Cora", "Dain", "Edda", "Finn", "Greta", "Hugh", "Iona", "Jory", "Kara",
        "Lenn", "Mara", "Ned", "Orla", "Perrin", "Quinn", "Rosa", "Seth", "Tess", "Una", "Venn",
        "Wren", "Yara", "Bela", "Clem", "Dora", "Emil", "Fara", "Garin", "Hesta", "Ivo",
    ];
    const FAMILY: &[&str] = &[
        "Reed", "Stone", "Moss", "Ash", "Vale", "Clay", "Brook", "Alder", "Wick", "Thorn", "Cairn",
        "Fallow", "Grove", "Briar", "Hearth", "Willow",
    ];
    let index = id.saturating_sub(1) as usize;
    format!(
        "{} {}",
        FIRST[index % FIRST.len()],
        FAMILY[index / FIRST.len() % FAMILY.len()]
    )
}

impl Population {
    /// Explicit new-world initialization. New cottages create beds, not people.
    pub fn founders() -> Self {
        Self::founders_of(FOUNDERS)
    }
    /// A wagon train of `n` people.
    pub fn founders_of(n: usize) -> Self {
        let mut population = Self {
            schema: 1,
            next: 1,
            ..Self::default()
        };
        for _ in 0..n.min(MAX_CITIZENS) {
            population.arrive();
        }
        population
    }

    /// New settlers arrive without waiting for the clock, for tests.
    #[cfg(test)]
    pub(crate) fn add_people(&mut self, n: usize) {
        for _ in 0..n {
            self.arrive();
        }
    }

    /// A person takes up a trade at `level`: a founder's craft, a knight's, or
    /// what a teacher gave them. Never lowers a skill.
    pub fn grant(&mut self, id: u32, trade: &str, level: u8, rules: &Rules) -> bool {
        let xp = rules.xp_for(level.min(3));
        match self.citizens.get_mut(&id) {
            Some(c) if trade_index(trade).is_some() => {
                let e = c.trades.entry(trade.to_string()).or_default();
                *e = (*e).max(xp);
                true
            }
            _ => false,
        }
    }

    pub fn valid(&self) -> bool {
        if self.schema == 0 {
            return *self == Self::default();
        }
        let mut names = BTreeSet::new();
        self.schema == 1
            && self.citizens.len() <= MAX_CITIZENS
            && self.next > 0
            && self.next < u32::MAX
            && self.seconds < u64::MAX
            && self.meal_seconds < MEAL_SECONDS
            && self.arrival_seconds < 600
            && self.citizens.iter().all(|(&id, c)| {
                id > 0
                    && id < self.next
                    && !c.name.is_empty()
                    && c.name.len() <= 64
                    && !c.name.chars().any(char::is_control)
                    && names.insert(&c.name)
                    && c.hunger <= 100
                    && c.rest <= 100
                    && c.morale <= 100
                    && c.arrived <= self.seconds
                    && c.trades.len() <= TRADES.len()
                    && c.trades.keys().all(|t| trade_index(t).is_some())
                    && c.trades.values().all(|&xp| xp <= 100_000)
                    && c.livery <= 1_000_000
                    && c.home.is_none_or(|id| id > 0 && id < u32::MAX)
                    && c.job.is_none_or(|id| id > 0 && id < u32::MAX)
            })
    }

    fn arrive(&mut self) -> u32 {
        let id = self.next;
        self.next += 1;
        let base = name(id);
        let mut name = base.clone();
        for suffix in 1..=MAX_CITIZENS {
            if self.citizens.values().all(|c| c.name != name) {
                break;
            }
            name = format!("{base} {suffix}");
        }
        self.citizens.insert(
            id,
            Citizen {
                name,
                home: None,
                job: None,
                role: Role::Builder,
                hunger: 0,
                rest: 100,
                morale: 100,
                arrived: self.seconds,
                trades: BTreeMap::new(),
                livery: 0,
                waiting: None,
            },
        );
        id
    }

    fn migrate(&mut self, sites: &BTreeMap<u32, Site>) {
        if self.schema != 0 {
            return;
        }
        // Existing settlements previously received labour directly from beds.
        // Preserve enough housed workers for their current jobs, with six
        // founders minimum. Migration grants no food or other resources.
        let beds: usize = sites
            .values()
            .filter(|s| s.online)
            .map(|s| usize::from(s.beds))
            .sum();
        let jobs: usize = sites
            .values()
            .filter(|s| s.online)
            .map(|s| usize::from(s.jobs))
            .sum();
        let count = jobs.max(FOUNDERS).min(beds.max(FOUNDERS)).min(MAX_CITIZENS);
        self.schema = 1;
        self.next = 1;
        for _ in 0..count {
            self.arrive();
        }
    }

    fn assign(&mut self, sites: &BTreeMap<u32, Site>, rules: &Rules) {
        let mut beds: BTreeMap<u32, u16> = sites
            .values()
            .filter(|s| s.online && s.beds > 0)
            .map(|s| (s.id, s.beds))
            .collect();
        for citizen in self.citizens.values_mut() {
            if let Some(home) = citizen.home {
                match beds.get_mut(&home).filter(|n| **n > 0) {
                    Some(left) => *left -= 1,
                    None => citizen.home = None,
                }
            }
        }
        for citizen in self.citizens.values_mut().filter(|c| c.home.is_none()) {
            if let Some((&home, left)) = beds.iter_mut().find(|(_, n)| **n > 0) {
                citizen.home = Some(home);
                *left -= 1;
            }
        }
        // Where each stood last: at work, or idle at a post waiting on goods.
        let previous: BTreeMap<_, _> = self.citizens.iter().map(|(&id, c)| (id, c.job.or(c.waiting))).collect();
        let mut free: BTreeSet<_> = self.citizens.keys().copied().collect();
        let mut work: Vec<_> = sites.values().filter(|s| s.online && s.jobs > 0).collect();
        work.sort_by_key(|s| (std::cmp::Reverse(s.priority), s.role.order(), s.id));
        for citizen in self.citizens.values_mut() {
            citizen.job = None;
            citizen.waiting = None;
        }
        // Qualified people at work and posts running, by trade: one supervisor
        // can stand over only so many posts of their trade.
        let mut posts: BTreeMap<u8, usize> = BTreeMap::new();
        let mut supervisors: BTreeMap<u8, usize> = BTreeMap::new();
        for site in work {
            // A partial crew cannot produce. Leave those people available for
            // smaller workplaces, especially a farm that can feed the village.
            if free.len() < usize::from(site.jobs) {
                continue;
            }
            let rank = |id: &u32| {
                if previous[id] == Some(site.id) {
                    0
                } else if previous[id].is_none() {
                    1
                } else {
                    2
                }
            };
            let mut crew: Vec<u32> = Vec::new();
            if let (Some((t, level)), true) = (site.trade, rules.catalog.is_some()) {
                let name = TRADES[usize::from(t)];
                let running = posts.get(&t).copied().unwrap_or(0);
                let have = supervisors.get(&t).copied().unwrap_or(0);
                // Every `per_supervisor` posts need one more qualified person.
                if have < (running + 1).div_ceil(rules.per_supervisor()) {
                    let mut able: Vec<u32> = free
                        .iter()
                        .copied()
                        .filter(|id| self.citizens[id].supervises(name, level, rules))
                        .collect();
                    able.sort_by_key(|id| (rank(id), std::cmp::Reverse(self.citizens[id].skill(name, rules)), *id));
                    match able.first() {
                        Some(&lead) => crew.push(lead),
                        None => continue,
                    }
                }
                // The rest of the crew learns beside them: those with some
                // time at the trade first.
                let mut rest: Vec<u32> = free.iter().copied().filter(|id| !crew.contains(id)).collect();
                rest.sort_by_key(|id| {
                    (
                        rank(id),
                        std::cmp::Reverse(self.citizens[id].trades.get(name).copied().unwrap_or(0)),
                        self.citizens[id].skill_total(rules),
                        *id,
                    )
                });
                let need = usize::from(site.jobs).saturating_sub(crew.len());
                crew.extend(rest.into_iter().take(need));
                if crew.len() < usize::from(site.jobs) {
                    continue;
                }
                *posts.entry(t).or_default() += 1;
                *supervisors.entry(t).or_default() +=
                    crew.iter().filter(|id| self.citizens[id].supervises(name, level, rules)).count().min(1);
            } else {
                // Plain work goes to the unskilled first, so a trade's people
                // are still free when its posts come up.
                let mut candidates: Vec<u32> = free.iter().copied().collect();
                candidates.sort_by_key(|id| (rank(id), self.citizens[id].skill_total(rules), *id));
                crew.extend(candidates.into_iter().take(usize::from(site.jobs)));
            }
            for id in crew {
                free.remove(&id);
                let citizen = self.citizens.get_mut(&id).unwrap();
                citizen.job = Some(site.id);
                citizen.role = site.role;
            }
        }
        // Whoever no running post wanted goes back to the post they were at, if it
        // is only waiting on goods: they stand idle there (the post is theirs, in
        // sight), and are free for any post that needs them at the next count.
        for site in sites.values().filter(|s| s.online && s.jobs == 0 && s.held > 0) {
            let mut room = site.held;
            let back: Vec<u32> = free.iter().copied().filter(|id| previous[id] == Some(site.id)).collect();
            for id in back {
                if room == 0 {
                    break;
                }
                room -= 1;
                free.remove(&id);
                self.citizens.get_mut(&id).unwrap().waiting = Some(site.id);
            }
        }
    }

    /// Reconcile on successful building/road/priority commands. Failures are
    /// atomic; no malformed site can rewrite a person's name or assignment.
    pub fn reconcile(&mut self, sites: &[Site]) -> Result<PopulationReport, String> {
        self.reconcile_with(sites, &Rules::legacy())
    }
    pub fn reconcile_with(&mut self, sites: &[Site], rules: &Rules) -> Result<PopulationReport, String> {
        if !self.valid() {
            return Err("Invalid population save".into());
        }
        let sites = checked_sites(sites)?;
        self.migrate(&sites);
        self.assign(&sites, rules);
        Ok(self.describe(&sites, rules))
    }

    /// A read-only report also previews migration and reassignment, so loading
    /// an older save never displays zero workers before its first actual tick.
    pub fn report(&self, sites: &[Site]) -> PopulationReport {
        self.report_with(sites, &Rules::legacy())
    }
    pub fn report_with(&self, sites: &[Site], rules: &Rules) -> PopulationReport {
        let mut preview = self.clone();
        preview.reconcile_with(sites, rules).unwrap_or_default()
    }

    pub fn advance_second(
        &mut self,
        sites: &[Site],
        stock: &mut Stock,
    ) -> Result<PopulationReport, String> {
        self.advance_with(sites, stock, &Rules::legacy())
    }
    pub fn advance_with(
        &mut self,
        sites: &[Site],
        stock: &mut Stock,
        rules: &Rules,
    ) -> Result<PopulationReport, String> {
        if !self.valid() || self.seconds >= u64::MAX - 1 {
            return Err("Invalid population clock or save".into());
        }
        let sites = checked_sites(sites)?;
        self.migrate(&sites);
        self.assign(&sites, rules);
        self.seconds += 1;
        self.meal_seconds += 1;
        let mut eaten = 0;
        // Time at a trade is how a trade is learned: a post that is running
        // with its crew teaches everyone on it.
        if rules.catalog.is_some() {
            for c in self.citizens.values_mut() {
                let Some(site) = c.job.and_then(|j| sites.get(&j)) else { continue };
                if let (Some((t, _)), true) = (site.trade, site.online) {
                    let xp = c.trades.entry(TRADES[usize::from(t)].to_string()).or_default();
                    *xp = xp.saturating_add(1).min(100_000);
                }
            }
        }
        let wages = rules.catalog.is_some_and(|c| c.works.wage_seconds > 0 && self.seconds % u64::from(c.works.wage_seconds) == 0);
        if self.meal_seconds >= MEAL_SECONDS {
            self.meal_seconds = 0;
            // Rotate scarce meals rather than always feeding the oldest IDs.
            let mut people: Vec<_> = self.citizens.keys().copied().collect();
            if !people.is_empty() {
                let rotation =
                    (self.seconds / u64::from(MEAL_SECONDS) % people.len() as u64) as usize;
                people.rotate_left(rotation);
            }
            for id in people {
                let citizen = self.citizens.get_mut(&id).unwrap();
                let (_, level) = citizen.best(rules);
                // A meal: the staple, then the extras a skilled person expects.
                let (staple, extras): (String, Vec<String>) = match rules.catalog {
                    Some(c) if !c.diet.labourer.is_empty() => {
                        let tier = match level {
                            0 | 1 => &c.diet.labourer,
                            2 => &c.diet.skilled,
                            _ => &c.diet.master,
                        };
                        (tier[0].clone(), tier[1..].to_vec())
                    }
                    _ => (rules.staple().to_string(), Vec::new()),
                };
                let mut eat = |what: &str| -> bool {
                    for alt in what.split('|') {
                        if stock.get(alt).copied().unwrap_or(0) > 0 {
                            let n = stock.get_mut(alt).unwrap();
                            *n -= 1;
                            if *n == 0 {
                                stock.remove(alt);
                            }
                            return true;
                        }
                    }
                    false
                };
                if eat(&staple) {
                    eaten += 1;
                    citizen.hunger = citizen.hunger.saturating_sub(35);
                } else {
                    citizen.hunger = citizen.hunger.saturating_add(20).min(100);
                }
                let missing = extras.iter().filter(|e| !eat(e)).count() as u8;
                // A coat of livery, issued when the last has worn out.
                citizen.livery = citizen.livery.saturating_sub(u32::from(MEAL_SECONDS));
                if citizen.livery == 0 {
                    if let Some(c) = rules.catalog {
                        if c.resources.contains_key("livery") && eat("livery") {
                            citizen.livery = c.works.livery_seconds;
                        }
                    }
                }
                citizen.rest = if citizen.home.is_some() {
                    citizen.rest.saturating_add(15).min(100)
                } else {
                    citizen.rest.saturating_sub(10)
                };
                if citizen.job.is_some() {
                    citizen.rest = citizen.rest.saturating_sub(5);
                }
                let target = 100u8
                    .saturating_sub(citizen.hunger / 2)
                    .saturating_sub(if citizen.home.is_none() { 25 } else { 0 })
                    .saturating_sub(if citizen.job.is_none() { 10 } else { 0 })
                    .saturating_sub(missing * 8)
                    .saturating_add(if citizen.livery > 0 { 5 } else { 0 })
                    .min(100);
                citizen.morale = if citizen.morale < target {
                    citizen.morale.saturating_add(5).min(target)
                } else {
                    citizen.morale.saturating_sub(5).max(target)
                };
            }
        }
        // Wages: the Delve's gold pays journeymen and masters. A pay day
        // missed is felt in the morale of everyone owed.
        if wages {
            let c = rules.catalog.unwrap();
            for citizen in self.citizens.values_mut() {
                let owed = match citizen.best(rules).1 {
                    2 => c.works.wage_journeyman,
                    3 => c.works.wage_master,
                    _ => 0,
                };
                if owed == 0 {
                    continue;
                }
                if stock.get("coin").copied().unwrap_or(0) >= owed {
                    let n = stock.get_mut("coin").unwrap();
                    *n -= owed;
                    if *n == 0 {
                        stock.remove("coin");
                    }
                } else {
                    citizen.morale = citizen.morale.saturating_sub(10);
                }
            }
        }
        let beds: usize = sites
            .values()
            .filter(|s| s.online)
            .map(|s| usize::from(s.beds))
            .sum();
        let welcoming = self.citizens.len() < beds
            && self.citizens.len() < MAX_CITIZENS
            && self.next < u32::MAX - 1
            && stock.get(rules.staple()).copied().unwrap_or(0) >= (self.citizens.len() as u32 + 1) * rules.welcome_meals()
            // A settlement is welcoming while hardly anyone in it is going hungry or worn out:
            // up to one person in eight may be (none, below eight people).
            && self
                .citizens
                .values()
                .filter(|c| !(c.hunger <= 30 && c.rest >= 40 && c.morale >= 40))
                .count()
                <= if rules.catalog.is_some() { self.citizens.len() / 8 } else { 0 };
        self.arrival_seconds = if welcoming {
            self.arrival_seconds + 1
        } else {
            0
        };
        let mut arrivals = Vec::new();
        if self.arrival_seconds >= rules.arrival() {
            self.arrival_seconds = 0;
            arrivals.push(self.arrive());
            self.assign(&sites, rules);
        }
        let mut report = self.describe(&sites, rules);
        report.food_eaten = eaten;
        report.arrivals = arrivals;
        Ok(report)
    }

    fn describe(&self, sites: &BTreeMap<u32, Site>, rules: &Rules) -> PopulationReport {
        let mut report = PopulationReport {
            population: self.citizens.len() as u16,
            beds: sites
                .values()
                .filter(|s| s.online)
                .map(|s| u32::from(s.beds))
                .sum::<u32>()
                .min(u32::from(u16::MAX)) as u16,
            food_per_meal: self.citizens.len() as u16,
            ..PopulationReport::default()
        };
        let fallback = sites
            .values()
            .find(|s| s.online && s.beds > 0)
            .map_or((0, 0), |s| s.tile);
        let mut ability: BTreeMap<u32, u32> = BTreeMap::new();
        for (&id, citizen) in &self.citizens {
            report.housed += u16::from(citizen.home.is_some());
            report.employed += u16::from(citizen.job.is_some());
            report.hungry += u16::from(citizen.hunger >= 50);
            report.idle += u16::from(citizen.job.is_none());
            if let Some(post) = citizen.waiting.filter(|_| citizen.job.is_none()) {
                *report.waiting.entry(post).or_default() += 1;
            }
            if let Some(job) = citizen.job {
                *report.assigned.entry(job).or_default() += 1;
                // At a trade, skill is the speed of the hand: a beginner is
                // slow, a master quick.
                let skill = sites.get(&job).and_then(|s| s.trade).map(|(t, _)| citizen.skill(TRADES[usize::from(t)], rules));
                let factor = match skill {
                    None => 100,
                    Some(0) => 60,
                    Some(1) => 80,
                    Some(2) => 100,
                    Some(_) => 125,
                };
                *ability.entry(job).or_default() += u32::from(citizen.efficiency()) * factor / 100;
            }
            let activity = citizen.activity();
            let target = match activity {
                Activity::Working => citizen.job,
                Activity::Idle => citizen.waiting.or(citizen.home),
                Activity::Resting => citizen.home,
                Activity::Hungry => None,
            }
            .and_then(|id| sites.get(&id))
            .map_or(fallback, |site| site.tile);
            report.citizens.push(CitizenView {
                id,
                name: citizen.name.clone(),
                role: citizen.role,
                activity,
                home: citizen.home,
                job: citizen.job,
                waiting: citizen.waiting,
                target,
                efficiency: citizen.efficiency(),
                hunger: citizen.hunger,
                rest: citizen.rest,
                morale: citizen.morale,
                trades: {
                    let mut t: Vec<(String, u8)> =
                        citizen.trades.iter().map(|(k, &xp)| (k.clone(), rules.skill(xp))).collect();
                    t.sort_by(|a, b| b.1.cmp(&a.1).then(a.0.cmp(&b.0)));
                    t
                },
            });
        }
        for site in sites.values() {
            let assigned = report.assigned.get(&site.id).copied().unwrap_or(0);
            let efficiency = if !site.online || assigned < site.jobs {
                0
            } else if site.jobs == 0 {
                100
            } else {
                (ability.get(&site.id).copied().unwrap_or(0) / u32::from(site.jobs))
                    .min(if site.trade.is_some() && rules.catalog.is_some() { 150 } else { 100 }) as u16
            };
            report.efficiency.insert(site.id, efficiency);
        }
        report
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sites() -> Vec<Site> {
        vec![
            Site {
                id: 1,
                tile: (2, 2),
                role: Role::Porter,
                jobs: 0,
                beds: 6,
                online: true,
                priority: 1,
                held: 0,
                trade: None,
            },
            Site {
                id: 2,
                tile: (3, 2),
                role: Role::Farmer,
                jobs: 1,
                beds: 0,
                online: true,
                priority: 1,
                held: 0,
                trade: None,
            },
            Site {
                id: 3,
                tile: (4, 2),
                role: Role::Builder,
                jobs: 2,
                beds: 0,
                online: true,
                priority: 1,
                held: 0,
                trade: None,
            },
        ]
    }
    #[test]
    fn new_cottages_create_space_not_instant_people_and_migration_is_once_only() {
        let mut sites = sites();
        sites[0].beds = 30;
        sites[2].jobs = 14;
        let mut old = Population::default();
        assert_eq!(old.report(&sites).population, 15);
        assert_eq!(old, Population::default(), "read-only migration preview");
        old.reconcile(&sites).unwrap();
        let people = old.citizens.clone();
        old.reconcile(&sites).unwrap();
        assert_eq!(people, old.citizens);
        let mut fresh = Population::founders();
        fresh.reconcile(&sites).unwrap();
        assert_eq!(
            fresh.citizens.len(),
            6,
            "new houses do not seed old workers"
        );
    }
    #[test]
    fn people_keep_names_and_work_until_a_priority_change_requires_reassignment() {
        let mut sites = sites();
        sites[2].jobs = 5;
        let mut people = Population::founders();
        people.reconcile(&sites).unwrap();
        let before = people.clone();
        people.reconcile(&sites).unwrap();
        assert_eq!(before, people);
        sites.push(Site {
            id: 4,
            tile: (5, 2),
            role: Role::Arcanist,
            jobs: 2,
            beds: 0,
            online: true,
            priority: 2,
                held: 0,
                trade: None,
        });
        let report = people.reconcile(&sites).unwrap();
        assert_eq!(report.assigned[&4], 2);
        assert_eq!(
            report.assigned[&2], 1,
            "food wins ties, not explicit higher priority"
        );
        assert_eq!(
            report.efficiency[&3], 0,
            "partial crews do not claim full work"
        );
        for (id, c) in &people.citizens {
            assert_eq!(c.name, before.citizens[id].name);
        }
    }
    #[test]
    fn demolishing_homes_and_jobs_releases_people_without_losing_or_duplicating_them() {
        let mut sites = sites();
        let mut people = Population::founders();
        people.reconcile(&sites).unwrap();
        let names: Vec<_> = people.citizens.values().map(|c| c.name.clone()).collect();
        sites.retain(|s| s.id == 2);
        let report = people.reconcile(&sites).unwrap();
        assert_eq!(
            (report.population, report.housed, report.employed),
            (6, 0, 1)
        );
        assert_eq!(
            names,
            people
                .citizens
                .values()
                .map(|c| c.name.clone())
                .collect::<Vec<_>>()
        );
        sites[0].online = false;
        assert_eq!(people.reconcile(&sites).unwrap().employed, 0);
    }
    #[test]
    fn one_simple_farm_sustains_six_founders_and_meals_spend_real_stock() {
        let sites = sites();
        let mut people = Population::founders();
        let mut stock = Stock::from([("food".into(), 12)]);
        let mut eaten = 0;
        for second in 1..=900 {
            if second % 20 == 0 {
                *stock.entry("food".into()).or_default() += 4;
            }
            let report = people.advance_second(&sites, &mut stock).unwrap();
            eaten += report.food_eaten;
            assert_eq!(report.hungry, 0);
            assert!(report.efficiency[&2] >= 95);
        }
        assert_eq!(eaten, 60);
        assert_eq!(stock["food"], 12 + 45 * 4 - 60);
        assert_eq!(people.citizens.len(), 6, "full housing prevents arrivals");
    }
    #[test]
    fn hunger_and_homelessness_slow_work_but_recovery_is_possible() {
        let mut sites = sites();
        sites[0].online = false;
        let mut people = Population::founders();
        let mut stock = Stock::new();
        for _ in 0..1800 {
            people.advance_second(&sites, &mut stock).unwrap();
        }
        let report = people.report(&sites);
        assert_eq!(report.hungry, 6);
        assert!((25..60).contains(&report.efficiency[&2]));
        assert!((25..60).contains(&report.efficiency[&3]));
        sites[0].online = true;
        stock.insert("food".into(), 300);
        for _ in 0..1800 {
            people.advance_second(&sites, &mut stock).unwrap();
        }
        let recovered = people.report(&sites);
        assert_eq!(recovered.hungry, 0);
        assert!(recovered.efficiency[&2] >= 90);
    }
    #[test]
    fn arrivals_need_spare_beds_food_and_time_and_names_stay_unique() {
        let mut sites = sites();
        sites[0].beds = 8;
        let mut people = Population::founders();
        let mut stock = Stock::new();
        for _ in 0..120 {
            assert!(
                people
                    .advance_second(&sites, &mut stock)
                    .unwrap()
                    .arrivals
                    .is_empty()
            );
        }
        stock.insert("food".into(), 200);
        let mut arrived = Vec::new();
        for _ in 0..360 {
            arrived.extend(people.advance_second(&sites, &mut stock).unwrap().arrivals);
        }
        assert_eq!(arrived, vec![7, 8]);
        assert_eq!(people.citizens.len(), 8);
        assert!(people.valid());
    }
    #[test]
    fn checkpoint_replay_makes_identical_meals_people_and_assignments() {
        let mut sites = sites();
        sites[0].beds = 10;
        let mut a = Population::founders();
        let mut food_a = Stock::from([("food".into(), 200)]);
        for _ in 0..137 {
            a.advance_second(&sites, &mut food_a).unwrap();
        }
        let mut b: Population = serde_json::from_str(&serde_json::to_string(&a).unwrap()).unwrap();
        let mut food_b = food_a.clone();
        for _ in 0..600 {
            assert_eq!(
                a.advance_second(&sites, &mut food_a).unwrap(),
                b.advance_second(&sites, &mut food_b).unwrap()
            );
        }
        assert_eq!(a, b);
        assert_eq!(food_a, food_b);
    }
    #[test]
    fn malformed_sites_and_population_fail_before_any_food_or_state_mutation() {
        let mut people = Population::founders();
        let mut stock = Stock::from([("food".into(), 20)]);
        let before = (people.clone(), stock.clone());
        let mut sites = sites();
        sites.push(sites[0]);
        assert!(people.advance_second(&sites, &mut stock).is_err());
        assert_eq!((people.clone(), stock.clone()), before);
        people.citizens.get_mut(&1).unwrap().hunger = 101;
        let before = (people.clone(), stock.clone());
        assert!(people.advance_second(&[], &mut stock).is_err());
        assert_eq!((people, stock), before);
    }
    #[test]
    fn scarce_food_rotates_and_steady_starvation_cannot_overflow_needs() {
        let sites = sites();
        let mut people = Population::founders();
        let mut stock = Stock::new();
        let mut fed = BTreeSet::new();
        for meal in 0..12 {
            stock.insert("food".into(), 1);
            let before: BTreeMap<_, _> = people
                .citizens
                .iter()
                .map(|(&id, c)| (id, c.hunger))
                .collect();
            for _ in 0..MEAL_SECONDS {
                people.advance_second(&sites, &mut stock).unwrap();
            }
            for (&id, c) in &people.citizens {
                if c.hunger < before[&id] {
                    fed.insert(id);
                }
            }
            assert!(people.valid(), "meal {meal}");
        }
        assert_eq!(fed.len(), FOUNDERS, "each person receives rotating meals");
    }

    #[test]
    fn understaffed_high_priority_work_leaves_a_complete_food_crew_available() {
        let mut sites = sites();
        sites[1].priority = 0;
        sites[2].priority = 2;
        sites[2].jobs = 7;
        let mut people = Population::founders();
        let report = people.reconcile(&sites).unwrap();
        assert_eq!(report.assigned.get(&3), None);
        assert_eq!(report.efficiency[&3], 0);
        assert_eq!(report.assigned[&2], 1);
        assert_eq!(report.efficiency[&2], 100);
        assert_eq!(report.idle, 5);

        // A now-affordable crew honours its higher priority, while the farm
        // keeps the final worker. Repeated reports do not shuffle those IDs.
        sites[2].jobs = 5;
        let staffed = people.reconcile(&sites).unwrap();
        assert_eq!(staffed.assigned[&3], 5);
        assert_eq!(staffed.assigned[&2], 1);
        assert_eq!(staffed, people.reconcile(&sites).unwrap());
    }

    #[test]
    fn high_priority_farm_survives_construction_and_workers_return_after_completion() {
        let mut sites = sites();
        sites[1].priority = 2;
        sites[2].priority = 2;
        sites[2].jobs = 5;
        let mut people = Population::founders();
        let report = people.reconcile(&sites).unwrap();
        let farmer = report
            .citizens
            .iter()
            .find(|c| c.job == Some(2))
            .unwrap()
            .id;
        assert_eq!(report.efficiency[&2], 100);
        assert_eq!(report.assigned[&3], 5);

        // Completing a large construction phase releases its crew. A smaller
        // ready workshop and new extraction site get real individual workers.
        sites[2].jobs = 2;
        sites.push(Site {
            id: 4,
            tile: (5, 2),
            role: Role::Porter,
            jobs: 3,
            beds: 0,
            online: true,
            priority: 1,
                held: 0,
                trade: None,
        });
        let report = people.reconcile(&sites).unwrap();
        assert_eq!(people.citizens[&farmer].job, Some(2));
        assert_eq!(report.assigned[&3], 2);
        assert_eq!(report.assigned[&4], 3);
        assert_eq!(report.idle, 0);
    }

    #[test]
    fn a_completed_home_and_slow_farm_recover_without_free_food() {
        let mut sites = sites();
        sites[0].online = false;
        let mut people = Population::founders();
        let mut food = Stock::new();
        for _ in 0..1800 {
            people.advance_second(&sites, &mut food).unwrap();
        }
        assert_eq!(people.report(&sites).hungry, 6);
        sites[0].online = true;
        let (mut credit, mut shifts, mut grown, mut eaten) = (0u16, 0, 0, 0);
        // Use the real farm cadence: two ten-second shifts grow four food.
        // Work credit comes exclusively from the hungry farmer's efficiency.
        for second in 1..=3600 {
            let report = people.advance_second(&sites, &mut food).unwrap();
            eaten += u32::from(report.food_eaten);
            if second % 10 == 0 {
                credit += report.efficiency[&2];
                if credit >= 100 {
                    credit -= 100;
                    shifts += 1;
                    if shifts == 2 {
                        shifts = 0;
                        grown += 4;
                        *food.entry("food".into()).or_default() += 4;
                    }
                }
            }
        }
        let recovered = people.report(&sites);
        assert_eq!(recovered.hungry, 0);
        assert!(recovered.efficiency[&2] >= 90);
        assert_eq!(food.get("food").copied().unwrap_or(0), grown - eaten);
        assert!(people.valid());
    }
}
