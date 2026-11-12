//! The people of Bomrekedan (`assets/bomrekedan/people.json`): every living
//! citizen, resident and visitor of the fort, each on their own level.
//!
//! One the import found on a kept level stands where the game had them, or
//! on the nearest open floor to it; one it found on a dropped level is put
//! on their district's level, by the place of that level their work belongs
//! to (a guard in a barracks, a farmer on a farm plot). Nobody stands on a
//! stair, beside one, or where a party lands, and no two on one tile.
//!
//! Where anyone is, is a pure function of the person and the tick (`at`): a
//! short round between their own tile and one or two beside it, resting a
//! while at each. Nothing of it is saved or sent. A knight's fresh E within
//! two tiles of someone has them say a line of their own (`says`), made from
//! what the fort knows of them: their office, squad, best skill, what they
//! value and scorn, and their temper.

use super::{ARRIVAL, DOORS, LEVELS, TOP, index, places_on};
use crate::rules::world::Dest;
use crate::rules::{HZ, RoomKind, Run, TILE_UNITS, Tile};
use serde::Deserialize;
use std::sync::OnceLock;

/// How near (tiles) a knight stands to talk with someone.
pub const TALK_REACH: f32 = 2.0;

/// The peoples who live in or visit the fort, for their figures.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Race {
    Dwarf,
    Human,
    Erinys,
    Gnoll,
    Other,
}

/// Someone of the fort, placed on their level.
#[derive(Clone, Debug)]
pub struct Person {
    pub id: u32,
    /// Their name in the Delve's letters, the fort's word for them made
    /// English ("Ungiz Brotherkey").
    pub name: String,
    pub race: Race,
    pub female: bool,
    /// `citizen`, `resident` or `visitor`.
    pub role: String,
    /// What the fort has them do (`home_job`: `guard`, `mason`, ...), and
    /// their profession as the game names it.
    pub job: String,
    pub profession: String,
    /// The level they belong to, as the line says it ("the Barracks").
    pub district: String,
    /// The level they stand on.
    pub level: RoomKind,
    /// Whether the import had them on this level (not on a dropped one).
    pub from_data: bool,
    /// Their own tile, and their round: tiles, their own first.
    pub home: (i32, i32),
    pub route: Vec<(i32, i32)>,
    /// Ticks at each stop of the round, and where in it they start.
    pub period: u64,
    pub phase: u64,
    /// Everything they might say, one a press.
    pub lines: Vec<String>,
    /// Where they are in `all()`: their figure's index.
    pub slot: usize,
}

#[derive(Deserialize)]
struct Raw {
    id: u32,
    #[serde(default)]
    name: String,
    #[serde(default)]
    name_english: String,
    #[serde(default)]
    race_name: String,
    #[serde(default)]
    caste: String,
    #[serde(default)]
    role: String,
    #[serde(default)]
    district: String,
    #[serde(default)]
    home_job: String,
    #[serde(default)]
    profession: String,
    #[serde(default)]
    level: Option<String>,
    #[serde(default)]
    x: Option<i32>,
    #[serde(default)]
    y: Option<i32>,
    #[serde(default)]
    notable: Vec<String>,
    #[serde(default)]
    positions: Vec<String>,
    #[serde(default)]
    squad: Option<String>,
    #[serde(default)]
    skills: Vec<RawSkill>,
}

#[derive(Deserialize)]
struct RawSkill {
    skill: String,
}

#[derive(Deserialize)]
struct File {
    people: Vec<Raw>,
}

/// A small fixed mix of two numbers: who stands where, and when.
fn mix(a: u32, b: u32) -> u32 {
    let mut h = a.wrapping_mul(0x9e37_79b9) ^ b.wrapping_mul(0x85eb_ca6b);
    h ^= h >> 15;
    h = h.wrapping_mul(0xc2b2_ae35);
    h ^ (h >> 13)
}

/// A name in the Delve's letters: the fort's accents dropped, anything else
/// outside plain ASCII left out.
pub fn ascii(text: &str) -> String {
    text.chars()
        .filter_map(|c| {
            Some(match c {
                'a'..='z' | 'A'..='Z' | '0'..='9' | ' ' | '\'' | '-' | ',' | '.' => c,
                'á' | 'à' | 'â' | 'ä' | 'å' | 'ã' => 'a',
                'Á' | 'À' | 'Â' | 'Ä' | 'Å' | 'Ã' => 'A',
                'é' | 'è' | 'ê' | 'ë' => 'e',
                'É' | 'È' | 'Ê' | 'Ë' => 'E',
                'í' | 'ì' | 'î' | 'ï' => 'i',
                'Í' | 'Ì' | 'Î' | 'Ï' => 'I',
                'ó' | 'ò' | 'ô' | 'ö' | 'õ' | 'ø' => 'o',
                'Ó' | 'Ò' | 'Ô' | 'Ö' | 'Õ' | 'Ø' => 'O',
                'ú' | 'ù' | 'û' | 'ü' => 'u',
                'Ú' | 'Ù' | 'Û' | 'Ü' => 'U',
                'ñ' => 'n',
                'Ñ' => 'N',
                'ç' => 'c',
                'Ç' => 'C',
                'ý' | 'ÿ' => 'y',
                'Ý' => 'Y',
                _ => return None,
            })
        })
        .collect()
}

fn race_of(name: &str) -> Race {
    match name {
        "dwarf" => Race::Dwarf,
        "human" => Race::Human,
        "erinys" => Race::Erinys,
        "gnoll" => Race::Gnoll,
        _ => Race::Other,
    }
}

/// A level's name as a line says it: "The Barracks" is "the Barracks".
fn district_name(slug: &str) -> String {
    LEVELS
        .iter()
        .position(|d| d.slug == slug)
        .map(|i| {
            let name = super::levels()[i].name;
            name.strip_prefix("The ").map_or(name.to_string(), |rest| format!("the {rest}"))
        })
        .unwrap_or_else(|| "Bomrekedan".to_string())
}

/// The kinds of place a job belongs at, best first (`kingdom.json` place
/// kinds; a `workshop:` or `furnace:` prefix matches any).
fn places_for(job: &str) -> &'static [&'static str] {
    match job {
        "guard" => &["barracks", "archery"],
        "farmer" => &["farm", "workshop:farmers", "workshop:still"],
        "mason" => &["workshop:masons", "stockpile"],
        "smith" => &["workshop:metalsmithsforge", "furnace:", "stockpile"],
        "carpenter" => &["workshop:carpenters", "stockpile"],
        "mechanic" => &["workshop:mechanics", "workshop:"],
        "crafter" => &["workshop:craftsdwarfs", "workshop:jewelers", "workshop:clothiers", "workshop:"],
        "jeweler" => &["workshop:jewelers", "workshop:"],
        "doctor" => &["hospital", "well"],
        "noble" => &["office", "dining", "tomb"],
        "clerk" => &["office"],
        "priest" => &["temple"],
        "performer" => &["tavern", "temple", "dining"],
        "cook" => &["workshop:kitchen", "dining"],
        "brewer" => &["workshop:still"],
        "butcher" => &["workshop:butchers"],
        "fisher" => &["workshop:kitchen", "pen"],
        "herder" => &["pen", "farm"],
        "hauler" => &["stockpile", "depot"],
        "miner" => &["stockpile"],
        _ => &["dining", "stockpile"],
    }
}

fn matches_kind(want: &str, kind: &str) -> bool {
    if want.ends_with(':') { kind.starts_with(want) } else { kind == want }
}

/// One level's open ground, for placing its people.
struct Ground {
    kind: RoomKind,
    room: crate::rules::Room,
    taken: Vec<bool>,
    /// Tiles where a party lands, which nobody blocks.
    landings: Vec<(f32, f32)>,
    /// Where the level's questline teachers stand (`questline`): nobody
    /// stands near enough that one press could be for either.
    teachers: Vec<(i32, i32)>,
    /// The level's works signs (`works::WORKS`), kept clear: a press on
    /// one is the works'.
    signs: Vec<(i32, i32, i32, i32)>,
}

/// How far (tiles each way) a person's own tile keeps from a teacher's:
/// their round goes two tiles out, and a knight is heard from two tiles by
/// either, so no spot is in reach of both.
pub const TEACHER_CLEAR: i32 = 6;
/// How far (tiles each way) a person's own tile keeps from a works sign's
/// rect: their round, two tiles out, never steps onto it.
pub const SIGN_CLEAR: i32 = 3;

impl Ground {
    fn new(kind: RoomKind) -> Ground {
        let room = super::room(kind, (0, 0));
        let mut landings: Vec<(f32, f32)> = DOORS
            .iter()
            .filter_map(|e| match e.to {
                Dest::Room(k, at) if k == kind => Some(at),
                _ => None,
            })
            .collect();
        if kind == TOP {
            landings.push(ARRIVAL);
        }
        let taken = vec![false; room.cols * room.rows];
        let teachers = crate::rules::questline::book().lines.iter().filter(|q| q.kind() == kind).map(|q| q.at).collect();
        let signs = super::works::WORKS.iter().filter(|w| w.level == kind).map(|w| w.sign).collect();
        Ground { kind, room, taken, landings, teachers, signs }
    }

    fn near_teacher(&self, x: i32, y: i32, clear: i32) -> bool {
        self.teachers.iter().any(|&(tx, ty)| (x - tx).abs() < clear && (y - ty).abs() < clear)
    }

    fn floor(&self, x: i32, y: i32) -> bool {
        x >= 0 && y >= 0 && (x as usize) < self.room.cols && (y as usize) < self.room.rows && self.room.tile(x, y) == Tile::Floor
    }

    /// Open floor a person may stand on: off every stair and the tiles
    /// beside one, clear of where a party lands.
    fn open(&self, x: i32, y: i32) -> bool {
        if !self.floor(x, y) {
            return false;
        }
        for dy in -1..=1 {
            for dx in -1..=1 {
                if self.room.tile(x + dx, y + dy) == Tile::Stairs {
                    return false;
                }
            }
        }
        !self.landings.iter().any(|&(lx, ly)| {
            let (lx, ly) = (lx.floor() as i32, ly.floor() as i32);
            (x - lx).abs() <= 3 && (y - ly).abs() <= 1
        })
    }

    fn free(&self, x: i32, y: i32) -> bool {
        self.open(x, y)
            && !self.near_teacher(x, y, TEACHER_CLEAR)
            && !self.signs.iter().any(|&(c, r, w, h)| x >= c - SIGN_CLEAR && x < c + w + SIGN_CLEAR && y >= r - SIGN_CLEAR && y < r + h + SIGN_CLEAR) && !self.taken[y as usize * self.room.cols + x as usize]
    }

    /// Whether nothing stands on the tile: bare floor, a stockpile, a
    /// field, a shrub.
    fn bare(&self, x: i32, y: i32) -> bool {
        let level = super::level(self.kind).unwrap();
        matches!(level.dress_at(x, y), b' ' | b'p' | b'f' | b'"')
    }

    /// The nearest free tile to `(x, y)`, ring by ring: bare floor first
    /// within a few tiles, else any free floor.
    fn nearest(&self, (x, y): (i32, i32)) -> Option<(i32, i32)> {
        let ring = |r: i32| {
            (-r..=r).flat_map(move |dy| (-r..=r).map(move |dx| (dx, dy))).filter(move |&(dx, dy)| dx.abs().max(dy.abs()) == r).map(move |(dx, dy)| (x + dx, y + dy))
        };
        (0..=6)
            .flat_map(ring)
            .find(|&(px, py)| self.free(px, py) && self.bare(px, py))
            .or_else(|| (0..=60).flat_map(ring).find(|&(px, py)| self.free(px, py)))
    }

    fn take(&mut self, (x, y): (i32, i32)) {
        self.taken[y as usize * self.room.cols + x as usize] = true;
    }

    /// A short round from `home`: a tile or two either side along a straight
    /// line of open floor, home between each.
    fn route(&self, home: (i32, i32), seed: u32) -> Vec<(i32, i32)> {
        const STEPS: [(i32, i32); 8] = [(1, 0), (-1, 0), (0, 1), (0, -1), (2, 0), (-2, 0), (0, 2), (0, -2)];
        let start = (seed % STEPS.len() as u32) as usize;
        let mut stops = Vec::new();
        for k in 0..STEPS.len() {
            let (dx, dy) = STEPS[(start + k * 3) % STEPS.len()];
            let to = (home.0 + dx, home.1 + dy);
            let mid = (home.0 + dx / 2, home.1 + dy / 2);
            if self.open(to.0, to.1) && self.open(mid.0, mid.1) && self.bare(to.0, to.1) && !stops.contains(&to) {
                stops.push(to);
            }
            if stops.len() == 2 {
                break;
            }
        }
        let mut route = vec![home];
        for stop in stops {
            route.push(stop);
            route.push(home);
        }
        // The round closes on its first stop: no second home at its end.
        if route.len() > 1 {
            route.pop();
        }
        route
    }
}

/// The place on `kind` where someone of `job` would be found, by `seed`
/// among the fittest: its rect's middle tile.
fn workplace(kind: RoomKind, job: &str, seed: u32) -> Option<(i32, i32)> {
    for want in places_for(job) {
        let fits: Vec<_> = places_on(kind).filter(|p| matches_kind(want, &p.kind)).collect();
        if !fits.is_empty() {
            let p = fits[(seed % fits.len() as u32) as usize];
            return Some((p.rect.x + p.rect.w / 2, p.rect.y + p.rect.h / 2));
        }
    }
    places_on(kind).nth((seed % 7) as usize).map(|p| (p.rect.x + p.rect.w / 2, p.rect.y + p.rect.h / 2))
}

/// A skill's name as a line says it.
fn skill_words(skill: &str) -> String {
    match skill {
        "melee_combat" => "a fight at close quarters".into(),
        "situational_awareness" => "keeping my eyes open".into(),
        "cutgem" => "cutting gems".into(),
        "encrustgem" => "setting gems".into(),
        "siegeoperate" => "working the siege engines".into(),
        "siegecraft" => "building siege engines".into(),
        "processplants" => "pressing and threshing".into(),
        "processfish" => "cleaning fish".into(),
        "animaltrain" => "training beasts".into(),
        "animalcare" => "tending beasts".into(),
        "geld" => "gelding".into(),
        "milk" => "milking".into(),
        "plant" => "growing things underground".into(),
        "mining" => "mining".into(),
        "pacify" => "calming tempers".into(),
        "discipline" => "discipline".into(),
        "console" => "consoling the grieving".into(),
        other => other.replace('_', " "),
    }
}

/// An office as a line says it ("MILITIA_CAPTAIN" is "militia captain").
fn office_words(code: &str) -> String {
    let words = code.to_lowercase().replace('_', " ");
    words.strip_prefix("custom ").map(str::to_string).unwrap_or(words)
}

/// What one of their traits makes them say, if anything.
fn trait_line(t: &str) -> Option<String> {
    if let Some(v) = t.strip_prefix("values ") {
        return Some(format!("Nothing matters more to me than {v}."));
    }
    if let Some(v) = t.strip_prefix("scorns ") {
        return Some(format!("{}? Bah. Waste of good beer.", capital(v)));
    }
    Some(
        match t {
            "high anger propensity" => "Don't try my temper, knight.",
            "low anger propensity" => "Nothing ruffles me. Not even goblins.",
            "high cheer propensity" => "What a fine day under the mountain!",
            "low cheer propensity" => "Another day, another tunnel.",
            "high depression propensity" => "The dark gets into you down here.",
            "high stress vulnerability" => "Too much. It's all too much.",
            "low stress vulnerability" => "Sieges, floods, forgotten beasts. I sleep fine.",
            "high anxiety propensity" => "Did you hear that? In the walls?",
            "low anxiety propensity" => "Worry? Over what?",
            "high love propensity" => "Have you met my sweetheart? Lovely beard.",
            "high hate propensity" => "I don't forget a slight.",
            "low hate propensity" => "I hold no grudges. Life's too short.",
            "high altruism" => "Need anything? Just ask.",
            "low altruism" => "What's in it for me?",
            "high friendliness" => "A knight! Come, sit, have a drink.",
            "low friendliness" => "Hmph. Knights.",
            "high cruelty" => "Mercy is for elves.",
            "low cruelty" => "I wouldn't hurt a cave fly.",
            "high violent" => "Point me at a goblin.",
            "low violent" => "Can't we settle it over an ale?",
            "high bravery" => "Afraid? Of what?",
            "low bravery" => "I'm not going near the caverns again.",
            "high dutifulness" => "My duty comes first.",
            "low dutifulness" => "Orders? I'll get round to them.",
            "high curious" => "What's it like, up there in your kingdom?",
            "high vanity" => "Admire this craftsmanship. My own beard.",
            "high greed" => "Gold, knight. Do you carry any?",
            "high privacy" => "I keep to myself.",
            "high gregariousness" => "More folk the merrier!",
            "low activity level" => "I'll get to it. Later.",
            "high activity level" => "No time to talk! Well, a little.",
            "high orderliness" => "Everything in its stockpile.",
            "high perfectionist" => "If it isn't a masterwork, it isn't done.",
            "high vengeful" => "They'll pay. They always pay.",
            "high immoderation" => "One more barrel won't hurt.",
            "high abstract inclined" => "Have you considered the nature of stone?",
            "high art inclined" => "I've an engraving in mind for that wall.",
            "high excitement seeking" => "Is there a dragon? Say there's a dragon.",
            "high trust" => "I'd trust you with my axe.",
            "low trust" => "I've got my eye on you.",
            "high tolerant" => "All are welcome under the mountain.",
            "low tolerant" => "Too many strangers in the halls these days.",
            "high gratitude" => "Thank you for stopping by, truly.",
            "high thoughtlessness" => "Plans? I just dig.",
            "low thoughtlessness" => "Measure twice, dig once.",
            _ => return None,
        }
        .to_string(),
    )
}

fn capital(s: &str) -> String {
    let mut c = s.chars();
    c.next().map_or(String::new(), |f| f.to_uppercase().chain(c).collect())
}

fn lines_of(raw: &Raw) -> Vec<String> {
    let mut out = Vec::new();
    let prof = raw.profession.to_lowercase();
    if prof.contains("baby") {
        return vec!["Goo.".to_string()];
    }
    if prof.contains("child") {
        out.push("Are you a real knight? Can I hold your sword?".to_string());
    }
    for office in &raw.positions {
        out.push(format!("I serve the fort as {}.", office_words(office)));
    }
    if let Some(squad) = &raw.squad {
        out.push(format!("I stand with {}.", ascii(squad)));
    }
    if let Some(best) = raw.skills.first() {
        out.push(format!("Nobody here is better at {}.", skill_words(&best.skill)));
    }
    match raw.role.as_str() {
        "visitor" => out.push("Only passing through. Fine halls, though.".to_string()),
        "resident" => out.push("I have made my home here among the dwarves.".to_string()),
        _ => {}
    }
    out.extend(raw.notable.iter().filter_map(|t| trait_line(t)));
    if out.is_empty() {
        out.push("Welcome to Bomrekedan.".to_string());
    }
    out
}

fn read() -> Vec<Person> {
    let file: File = serde_json::from_str(include_str!("../../../../assets/bomrekedan/people.json"))
        .unwrap_or_else(|e| panic!("bomrekedan people.json: {e}"));
    let mut grounds: Vec<Ground> = LEVELS.iter().map(|d| Ground::new(d.kind)).collect();
    let mut out: Vec<Person> = Vec::new();
    // Those the game had on a kept level first, so their tiles are theirs;
    // then those from dropped levels, round their work.
    let mut order: Vec<&Raw> = file.people.iter().filter(|r| r.level.is_some()).collect();
    order.extend(file.people.iter().filter(|r| r.level.is_none()));
    for raw in order {
        let from_data = raw.level.is_some();
        let slug = raw.level.as_deref().unwrap_or(&raw.district);
        let Some(i) = LEVELS.iter().position(|d| d.slug == slug) else {
            continue;
        };
        let kind = LEVELS[i].kind;
        let seed = mix(raw.id, 0x5eed);
        let want = match (raw.x, raw.y) {
            (Some(x), Some(y)) if from_data => Some((x, y)),
            _ => workplace(kind, &raw.home_job, seed),
        };
        let ground = &mut grounds[i];
        let Some(home) = want.and_then(|w| ground.nearest(w)).or_else(|| ground.nearest((58, 50))) else {
            continue;
        };
        ground.take(home);
        let route = ground.route(home, seed >> 3);
        let period = u64::from(HZ) * (5 + u64::from(seed % 6));
        let phase = u64::from(mix(raw.id, 0xfa5e)) % (period * route.len() as u64);
        let name = ascii(if raw.name_english.is_empty() { &raw.name } else { &raw.name_english });
        out.push(Person {
            id: raw.id,
            name: name.split_whitespace().collect::<Vec<_>>().join(" "),
            race: race_of(&raw.race_name),
            female: raw.caste == "female",
            role: raw.role.clone(),
            job: raw.home_job.clone(),
            profession: ascii(&raw.profession),
            district: district_name(&raw.district),
            level: kind,
            from_data,
            home,
            route,
            period,
            phase,
            lines: lines_of(raw),
            slot: 0,
        });
    }
    // By level, then as the file had them.
    out.sort_by_key(|p| index(p.level));
    for (slot, p) in out.iter_mut().enumerate() {
        p.slot = slot;
    }
    out
}

/// Everyone, read and placed once, by level top down.
pub fn all() -> &'static [Person] {
    static PEOPLE: OnceLock<Vec<Person>> = OnceLock::new();
    PEOPLE.get_or_init(read)
}

/// The people of level `kind` (none for a room that is no level).
pub fn on(kind: RoomKind) -> &'static [Person] {
    static RANGES: OnceLock<Vec<(usize, usize)>> = OnceLock::new();
    let ranges = RANGES.get_or_init(|| {
        LEVELS
            .iter()
            .map(|d| {
                let start = all().iter().position(|p| p.level == d.kind).unwrap_or(0);
                let n = all().iter().filter(|p| p.level == d.kind).count();
                (start, start + n)
            })
            .collect()
    });
    index(kind).map_or(&[][..], |i| &all()[ranges[i].0..ranges[i].1])
}

impl Person {
    /// The leg of their round they are on at `tick`: from, to, and the
    /// ticks into it.
    fn leg(&self, tick: u64) -> ((i32, i32), (i32, i32), u64) {
        let n = self.route.len() as u64;
        let t = tick + self.phase;
        let stop = (t / self.period) % n;
        let from = self.route[((stop + n - 1) % n) as usize];
        (from, self.route[stop as usize], t % self.period)
    }

    /// Ticks to walk from one stop to the next: two tiles a second.
    fn walk_ticks(from: (i32, i32), to: (i32, i32)) -> u64 {
        let d = (from.0 - to.0).abs() + (from.1 - to.1).abs();
        d as u64 * u64::from(HZ) / 2
    }

    /// Where they stand at `tick` (tiles; a tile's middle at rest).
    pub fn at(&self, tick: u64) -> (f32, f32) {
        let (from, to, into) = self.leg(tick);
        let walk = Self::walk_ticks(from, to);
        let k = if walk == 0 { 1.0 } else { (into as f32 / walk as f32).min(1.0) };
        (from.0 as f32 + (to.0 - from.0) as f32 * k + 0.5, from.1 as f32 + (to.1 - from.1) as f32 * k + 0.5)
    }

    /// Whether they are on the move at `tick`, and which way across
    /// (`true`: west).
    pub fn walking(&self, tick: u64) -> Option<bool> {
        let (from, to, into) = self.leg(tick);
        (into < Self::walk_ticks(from, to)).then_some(to.0 < from.0)
    }

    /// What they say when a knight speaks to them at `tick`.
    pub fn says(&self, tick: u64) -> String {
        let line = &self.lines[((tick / 3) as usize + self.id as usize) % self.lines.len()];
        let job = if self.job.is_empty() { self.profession.to_lowercase() } else { self.job.clone() };
        format!("{}, {job} of {}: {line}", self.name, self.district)
    }
}

/// The person of level `kind` nearest `(x, y)` (tiles) at `tick`, within
/// talking reach.
pub fn near(kind: RoomKind, (x, y): (f32, f32), tick: u64) -> Option<&'static Person> {
    on(kind)
        .iter()
        .map(|p| {
            let (px, py) = p.at(tick);
            ((px - x).hypot(py - y), p)
        })
        .filter(|&(d, _)| d < TALK_REACH)
        .min_by(|a, b| a.0.total_cmp(&b.0))
        .map(|(_, p)| p)
}

impl Run {
    /// A knight's fresh E (`pressed`, a bit a knight) beside someone of the
    /// fort: they say their line. A knight on a stair is taking it.
    pub(crate) fn tick_bomrekedan_people(&mut self, pressed: u8) {
        // Most ticks have no fresh press: nothing more to look at.
        if pressed == 0 {
            return;
        }
        let kind = self.room().kind;
        if !super::is_level(kind) {
            return;
        }
        let mut said = None;
        for (&id, h) in &self.players {
            if pressed & (1 << (id & 7)) == 0 || h.hp == 0 || h.stone {
                continue;
            }
            let (x, y) = (h.x / TILE_UNITS, h.y / TILE_UNITS);
            if self.room().tile(x.floor() as i32, y.floor() as i32) == Tile::Stairs {
                continue;
            }
            // A press within a questline teacher's hearing is the teacher's.
            let teacher = crate::rules::questline::book().lines.iter().any(|q| {
                // The same reach `tick_teachers` hears a press at.
                q.kind() == kind && (x - q.at.0 as f32).abs() <= crate::rules::questline::TEACHER_REACH && (y - q.at.1 as f32).abs() <= crate::rules::questline::TEACHER_REACH
            });
            // So is one on a works sign (`works`, its E held to raise).
            if teacher || super::works::WORKS.iter().any(|w| w.level == kind && w.on_sign(h.x, h.y)) {
                continue;
            }
            if let Some(p) = near(kind, (x, y), self.tick) {
                said = Some((id, p.says(self.tick)));
            }
        }
        if let Some((id, line)) = said {
            self.found = Some((self.tick, id, line));
        }
    }
}
