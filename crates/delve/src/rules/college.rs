//! The College of the Labyrinth: free rooms, research missions and short lessons.
//! Progress is a host intent, paid only by the ordinary atomic realm transaction.
use super::bounties::Pinned;
use super::world::{Dest, Entrance};
use super::*;
use std::collections::BTreeSet;

pub const NAME: &str = "College of the Labyrinth";
pub const ROOMS: [RoomKind; 9] = [
    RoomKind::CollegeGrounds,
    RoomKind::CollegeHall,
    RoomKind::CollegeClass,
    RoomKind::CollegeLibrary,
    RoomKind::CollegeLab,
    RoomKind::TowerEntry,
    RoomKind::TowerStairs,
    RoomKind::TowerStars,
    RoomKind::WizardStudy,
];
pub fn is_room(kind: RoomKind) -> bool {
    ROOMS.contains(&kind)
}
pub fn label(kind: RoomKind) -> &'static str {
    match kind {
        RoomKind::CollegeGrounds => "College Courtyard",
        RoomKind::CollegeHall => "Great Hall",
        RoomKind::CollegeClass => "Chalk Classroom",
        RoomKind::CollegeLibrary => "Floating Library",
        RoomKind::CollegeLab => "Alchemy Lab",
        RoomKind::TowerEntry => "Tower Entry",
        RoomKind::TowerStairs => "Spiral Landing",
        RoomKind::TowerStars => "Star Observatory",
        RoomKind::WizardStudy => "Bartosz's Study",
        _ => "College Gate",
    }
}
pub const WIZARD: (f32, f32) = (12.0, 5.5);
pub const LECTERN: (f32, f32) = (6.0, 9.5);
pub const BOOKSHELF: (f32, f32) = (5.0, 5.5);
pub const STAR_MAP: (f32, f32) = (12.0, 5.5);
pub const FAMILIAR: (f32, f32) = (19.0, 9.5);
/// Small discoveries in the school itself, separate from the tower mission.
pub const CLUES: [(RoomKind, (f32, f32), &str, &str); 4] = [
    (
        RoomKind::CollegeGrounds,
        (11.5, 8.5),
        "college:clue:fountain",
        "Under the fountain rim: a tiny sketch of a bat's wing.",
    ),
    (
        RoomKind::CollegeHall,
        (12.0, 6.5),
        "college:clue:rug",
        "Under the rug: 'No knight learns alone.' Bartosz has signed it.",
    ),
    (
        RoomKind::CollegeClass,
        (12.0, 4.5),
        "college:clue:chalk",
        "A second chalk layer reads: 'Test the claim, not the hat.'",
    ),
    (
        RoomKind::CollegeLab,
        (12.0, 10.5),
        "college:clue:jar",
        "Inside the jar lid: 'Slime sample. Please do not feed after class.'",
    ),
];
pub const INTENT_LIMIT: usize = 32 + CLUES.len();
pub const SIGILS: [(RoomKind, (f32, f32)); 3] = [
    (RoomKind::TowerEntry, (5.0, 3.5)),
    (RoomKind::TowerStars, (18.0, 5.5)),
    (RoomKind::WizardStudy, (18.0, 8.5)),
];
const fn door(
    id: &'static str,
    room: RoomKind,
    at: (i32, i32, i32, i32),
    to: RoomKind,
    front: (f32, f32),
    label: &'static str,
) -> Entrance {
    Entrance {
        id,
        room,
        at,
        to: Dest::Room(to, front),
        label,
        needs: None,
    }
}
pub const DOORS: [Entrance; 18] = [
    door(
        "college-gate",
        RoomKind::Gate,
        (20, 4, 2, 1),
        RoomKind::CollegeGrounds,
        (12.0, 10.5),
        "COLLEGE GATE",
    ),
    door(
        "college-out",
        RoomKind::CollegeGrounds,
        (11, 12, 2, 1),
        RoomKind::Gate,
        (21.0, 5.5),
        "DELVE COURTYARD",
    ),
    door(
        "college-hall",
        RoomKind::CollegeGrounds,
        (8, 3, 2, 1),
        RoomKind::CollegeHall,
        (12.0, 10.5),
        "GREAT HALL",
    ),
    door(
        "hall-out",
        RoomKind::CollegeHall,
        (11, 12, 2, 1),
        RoomKind::CollegeGrounds,
        (9.0, 5.5),
        "COLLEGE COURTYARD",
    ),
    door(
        "college-class",
        RoomKind::CollegeHall,
        (3, 4, 2, 1),
        RoomKind::CollegeClass,
        (12.0, 10.5),
        "CHALK CLASSROOM",
    ),
    door(
        "class-out",
        RoomKind::CollegeClass,
        (11, 12, 2, 1),
        RoomKind::CollegeHall,
        (4.0, 6.5),
        "GREAT HALL",
    ),
    door(
        "college-library",
        RoomKind::CollegeHall,
        (11, 3, 2, 1),
        RoomKind::CollegeLibrary,
        (12.0, 10.5),
        "FLOATING LIBRARY",
    ),
    door(
        "library-out",
        RoomKind::CollegeLibrary,
        (11, 12, 2, 1),
        RoomKind::CollegeHall,
        (12.0, 5.5),
        "GREAT HALL",
    ),
    door(
        "college-lab",
        RoomKind::CollegeHall,
        (19, 4, 2, 1),
        RoomKind::CollegeLab,
        (12.0, 10.5),
        "ALCHEMY LAB",
    ),
    door(
        "lab-out",
        RoomKind::CollegeLab,
        (11, 12, 2, 1),
        RoomKind::CollegeHall,
        (20.0, 6.5),
        "GREAT HALL",
    ),
    door(
        "college-tower",
        RoomKind::CollegeGrounds,
        (18, 7, 2, 1),
        RoomKind::TowerEntry,
        (12.0, 10.5),
        "WIZARD'S TOWER",
    ),
    door(
        "tower-out",
        RoomKind::TowerEntry,
        (11, 12, 2, 1),
        RoomKind::CollegeGrounds,
        (19.0, 9.5),
        "COLLEGE COURTYARD",
    ),
    door(
        "tower-climb",
        RoomKind::TowerEntry,
        (19, 3, 2, 1),
        RoomKind::TowerStairs,
        (5.0, 10.5),
        "SPIRAL LANDING",
    ),
    door(
        "tower-down",
        RoomKind::TowerStairs,
        (4, 12, 2, 1),
        RoomKind::TowerEntry,
        (20.0, 5.5),
        "TOWER ENTRY",
    ),
    door(
        "tower-stars",
        RoomKind::TowerStairs,
        (19, 3, 2, 1),
        RoomKind::TowerStars,
        (5.0, 10.5),
        "STAR OBSERVATORY",
    ),
    door(
        "stars-down",
        RoomKind::TowerStars,
        (4, 12, 2, 1),
        RoomKind::TowerStairs,
        (20.0, 5.5),
        "SPIRAL LANDING",
    ),
    door(
        "tower-study",
        RoomKind::TowerStars,
        (19, 3, 2, 1),
        RoomKind::WizardStudy,
        (12.0, 10.5),
        "BARTOSZ'S STUDY",
    ),
    door(
        "study-down",
        RoomKind::WizardStudy,
        (11, 12, 2, 1),
        RoomKind::TowerStars,
        (20.0, 5.5),
        "STAR OBSERVATORY",
    ),
];

/// Reuses Wren/Brannoc's pinned-mission record, separate from their boards.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct College {
    pub pinned: Vec<Pinned>,
    pub done: BTreeSet<String>,
    pub maps: BTreeSet<String>,
    pub observed: BTreeSet<String>,
    pub samples: BTreeSet<String>,
    pub tome: bool,
    pub trip: bool,
    pub sigils: u8,
    pub bookshelf: bool,
    pub stars: bool,
    pub familiar: bool,
    pub clues: u8,
}
impl College {
    pub fn valid(&self) -> bool {
        self.maps.iter().all(|id| field_key(id))
            && self.samples.iter().all(|id| field_key(id))
            && self.pinned.len() <= QUESTS.len()
            && self.done.len() <= QUESTS.len()
            && self.maps.len() <= 4
            && self.samples.len() <= 3
            && self.observed.len() <= 3
            && self.sigils < 8
            && self.clues < 16
            && self.pinned.iter().enumerate().all(|(i, p)| {
                quest(&p.id).is_some_and(|q| p.have <= q.need)
                    && !self.done.contains(&p.id)
                    && self.pinned[..i].iter().all(|v| v.id != p.id)
            })
            && self.done.iter().all(|id| quest(id).is_some())
            && self
                .observed
                .iter()
                .all(|id| OBSERVABLE.contains(&id.as_str()))
    }
    pub fn active(&self, id: &str) -> bool {
        self.pinned.iter().any(|p| p.id == id)
    }
    pub fn have(&self, q: &Quest) -> u32 {
        match q.id {
            "map" => self.maps.len() as u32,
            "creatures" => self.observed.len() as u32,
            "tome" => u32::from(self.tome),
            "spores" => self.samples.len() as u32,
            "together" => u32::from(self.trip),
            _ => self.sigils.count_ones(),
        }
        .min(q.need)
    }
}
pub struct Quest {
    pub id: &'static str,
    pub title: &'static str,
    pub asks: [&'static str; 2],
    pub need: u32,
    pub gold: u32,
    pub bond: u32,
    pub spell: bool,
}
pub const QUESTS: [Quest; 6] = [
    Quest {
        id: "map",
        title: "Chart the Tunnels",
        asks: [
            "Visit four rooms at depth three or below.",
            "Bring me a map with muddy corners.",
        ],
        need: 4,
        gold: 60,
        bond: 0,
        spell: true,
    },
    Quest {
        id: "creatures",
        title: "Creature Notes",
        asks: [
            "E near a living bat, skeleton and slime.",
            "Observe all three. Leave the ink to me.",
        ],
        need: 3,
        gold: 45,
        bond: 0,
        spell: false,
    },
    Quest {
        id: "tome",
        title: "The Lost Tome",
        asks: [
            "E at a tome marker in an Archive treasure room.",
            "Clear the room first. Books dislike arrows.",
        ],
        need: 1,
        gold: 50,
        bond: 0,
        spell: false,
    },
    Quest {
        id: "spores",
        title: "Spore Specimens",
        asks: [
            "E at spore patches in three Fungal rooms.",
            "Clear each room. Bring jars, not sneezes.",
        ],
        need: 3,
        gold: 55,
        bond: 0,
        spell: false,
    },
    Quest {
        id: "together",
        title: "Study Together",
        asks: [
            "Two living knights visit depth three together.",
            "Both must reach the field marker. Bring notes!",
        ],
        need: 1,
        gold: 40,
        bond: 1,
        spell: false,
    },
    Quest {
        id: "sigils",
        title: "Three Hidden Sigils",
        asks: [
            "Shelf, stars, familiar: the tower keeps secrets.",
            "E uncovers them. Find all three, then return.",
        ],
        need: 3,
        gold: 75,
        bond: 0,
        spell: false,
    },
];
fn quest(id: &str) -> Option<&'static Quest> {
    QUESTS.iter().find(|q| q.id == id)
}
const OBSERVABLE: [&str; 3] = ["bat", "skeleton", "slime"];
pub fn reward(q: &Quest) -> String {
    format!(
        "{} gold{}{}",
        q.gold,
        if q.spell { " + Hedge Maze spell" } else { "" },
        if q.bond > 0 { " + 1 bond" } else { "" }
    )
}
/// Exact names and claims here come from LABYRINTH.md, drive/labyrinth and
/// together_shooter/{layout,monsters,foes}; research claims grant no game loot.
pub const TOPICS: [(&str, [&str; 3]); 6] = [
    (
        "What is the Labyrinth?",
        [
            "I am Bartosz, maker of Labyrinth Exploration.",
            "Its research map links questions and claims.",
            "Here I study our tunnels and their creatures.",
        ],
    ),
    (
        "Claims and corridors",
        [
            "Charted claims, open questions, dark hunches.",
            "Exploratory routes may cross uncertain steps.",
            "Established routes need established evidence.",
        ],
    ),
    (
        "How do we check it?",
        [
            "Checks validate the map and its dependencies.",
            "An independent referee writes separate checks.",
            "Failed checks stay recorded. Useful footprints!",
        ],
    ),
    (
        "How are tunnels built?",
        [
            "From floor three: Gallery, Warrens or Karst.",
            "Seeded growth connects each new cell.",
            "Saved floors keep their geometry. Mind the pits.",
        ],
    ),
    (
        "Bats, bones and slimes",
        [
            "Bats fly erratically; skeleton archers wander.",
            "A fallen slime splits, then its children split.",
            "The Herald's Bestiary counts creatures felled.",
        ],
    ),
    (
        "The Fungal Deep",
        [
            "Slimes and bats abound; imps lurk there too.",
            "Fungal imp shots split into spores.",
            "Observe from a safe distance. Hats are not jars.",
        ],
    ),
];
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum Page {
    #[default]
    Closed,
    Menu,
    Topics,
    Topic(u8),
    Quests,
    Quest(u8),
    Bestiary,
}
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct Dialogue {
    pub owner: Option<u32>,
    pub page: Page,
    pub selected: u8,
    held: BTreeSet<u32>,
    nav: i8,
}
impl Dialogue {
    pub fn close(&mut self) {
        self.owner = None;
        self.page = Page::Closed;
        self.selected = 0;
    }
    pub fn valid(&self) -> bool {
        self.selected < 6
            && self.held.len() <= 4
            && self.held.iter().all(|id| (1..=MAX_PLAYERS).contains(id))
            && self.owner.is_none_or(|id| (1..=MAX_PLAYERS).contains(&id))
            && self.owner.is_some() == (self.page != Page::Closed)
            && (-1..=1).contains(&self.nav)
            && match self.page {
                Page::Topic(i) => usize::from(i) < TOPICS.len(),
                Page::Quest(i) => usize::from(i) < QUESTS.len(),
                _ => true,
            }
    }
}
/// Apply bounded, replay-safe host intents with gold/bond in one realm save.
pub fn settle(
    c: &mut College,
    t: &mut Spoils,
    marks: &BTreeMap<String, u32>,
) -> (bool, Vec<String>) {
    let before = c.clone();
    let mut news = Vec::new();
    for q in &QUESTS {
        if marks.contains_key(&format!("college:accept:{}", q.id))
            && !c.active(q.id)
            && !c.done.contains(q.id)
        {
            c.pinned.push(Pinned {
                id: q.id.into(),
                have: 0,
            });
        }
    }
    for (prefix, id, cap) in [
        ("college:map:", "map", 4),
        ("college:sample:", "spores", 3),
        ("college:observe:", "creatures", 3),
    ] {
        if !c.active(id) {
            continue;
        }
        let set = match id {
            "map" => &mut c.maps,
            "spores" => &mut c.samples,
            _ => &mut c.observed,
        };
        for mark in marks.keys().filter_map(|m| m.strip_prefix(prefix)) {
            if set.len() < cap && (id != "creatures" || OBSERVABLE.contains(&mark)) {
                set.insert(mark.into());
            }
        }
    }
    if c.active("tome") && marks.contains_key("college:tome") {
        c.tome = true;
    }
    if c.active("together") && marks.contains_key("college:together") {
        c.trip = true;
    }
    c.bookshelf |= marks.contains_key("college:bookshelf");
    c.stars |= marks.contains_key("college:stars");
    c.familiar |= marks.contains_key("college:familiar");
    for (i, &(_, _, key, _)) in CLUES.iter().enumerate() {
        if marks.contains_key(key) {
            c.clues |= 1 << i;
        }
    }
    if c.active("sigils") {
        for i in 0..3 {
            if marks.contains_key(&format!("college:sigil:{i}")) {
                c.sigils |= 1 << i;
            }
        }
    }
    for q in &QUESTS {
        let have = c.have(q);
        if let Some(p) = c.pinned.iter_mut().find(|p| p.id == q.id) {
            p.have = have;
        }
        if c.active(q.id)
            && have >= q.need
            && marks.contains_key(&format!("college:turn-in:{}", q.id))
        {
            c.pinned.retain(|p| p.id != q.id);
            c.done.insert(q.id.into());
            t.add(Spoil::Gold, q.gold);
            t.add(Spoil::Bond, q.bond);
            news.push(format!("Bartosz thanks you: {}. {}.", q.title, reward(q)));
        }
    }
    (*c != before, news)
}

pub fn blocks(kind: RoomKind) -> &'static [(usize, usize, usize, usize)] {
    match kind {
        RoomKind::CollegeGrounds => &[(10, 6, 3, 2), (5, 1, 7, 2), (17, 2, 5, 5)],
        RoomKind::CollegeHall => &[(4, 8, 5, 2), (15, 8, 5, 2), (7, 5, 1, 1), (16, 5, 1, 1)],
        RoomKind::CollegeClass => &[
            (6, 2, 12, 2),
            (4, 6, 4, 1),
            (10, 6, 4, 1),
            (16, 6, 4, 1),
            (4, 9, 4, 1),
            (16, 9, 4, 1),
        ],
        RoomKind::CollegeLibrary => &[
            (2, 2, 6, 2),
            (10, 2, 4, 2),
            (16, 2, 6, 2),
            (3, 7, 3, 2),
            (18, 7, 3, 2),
            (6, 9, 2, 1),
            (16, 9, 2, 1),
        ],
        RoomKind::CollegeLab => &[
            (3, 2, 5, 2),
            (16, 2, 5, 2),
            (4, 6, 3, 2),
            (10, 6, 3, 2),
            (17, 6, 3, 2),
            (9, 9, 6, 1),
        ],
        RoomKind::TowerEntry => &[(3, 4, 4, 1), (10, 3, 4, 1), (15, 9, 4, 1)],
        RoomKind::TowerStairs => &[(10, 5, 3, 3), (3, 3, 4, 2), (16, 9, 4, 1)],
        RoomKind::TowerStars => &[(10, 3, 4, 2), (3, 3, 3, 2), (16, 8, 4, 2)],
        RoomKind::WizardStudy => &[(3, 2, 5, 2), (16, 2, 5, 2), (10, 3, 4, 2), (5, 8, 2, 1)],
        _ => &[],
    }
}
pub fn room(kind: RoomKind, cell: (i32, i32)) -> Room {
    let mut r = Room::solid_rock(cell);
    r.kind = kind;
    for y in 1..ROWS - 1 {
        for x in 1..COLS - 1 {
            r.set(x, y, Tile::Floor);
        }
    }
    for &(x, y, w, h) in blocks(kind) {
        for yy in y..y + h {
            for xx in x..x + w {
                r.set(xx, yy, Tile::Block);
            }
        }
    }
    for e in DOORS.iter().filter(|e| e.room == kind) {
        let (x, y, w, h) = e.at;
        for yy in y..y + h {
            for xx in x..x + w {
                r.set(xx as usize, yy as usize, Tile::Stairs);
            }
        }
    }
    r
}
/// A deterministic reachable research pickup, shared with the renderer.
pub fn field_spot(room: &Room) -> Option<(f32, f32)> {
    (2..room.rows as i32 - 2)
        .flat_map(|y| (2..room.cols as i32 - 2).map(move |x| (x, y)))
        .filter(|&(x, y)| room.tile(x, y) == Tile::Floor && room.tile(x, y + 1) == Tile::Floor)
        .min_by_key(|&(x, y)| (x - 12).pow(2) + (y - 10).pow(2))
        .map(|(x, y)| (x as f32 + 0.5, y as f32 + 0.5))
}
fn near(h: &Hero, at: (f32, f32), tiles: f32) -> bool {
    h.hp > 0
        && !h.stone
        && h.privy == 0
        && (h.x - at.0 * TILE_UNITS).hypot(h.y - at.1 * TILE_UNITS) < tiles * TILE_UNITS
}

/// One target shared by the input handler and the room's single prompt.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Interaction {
    Wizard,
    Bestiary,
    Sigil(usize),
    Shelf,
    Stars,
    Familiar,
    Clue(usize),
    Book,
    Stone,
}
impl Interaction {
    pub fn label(self) -> &'static str {
        match self {
            Self::Wizard => "Bartosz the Wizard",
            Self::Bestiary => "Bestiary",
            Self::Sigil(_) => "Hidden Sigil",
            Self::Shelf => "Odd Shelf",
            Self::Stars => "Star Map",
            Self::Familiar => "Familiar",
            Self::Clue(i) => ["Fountain", "Woven Rug", "Chalk Mark", "Sealed Jars"][i],
            Self::Book => "Curious Book",
            Self::Stone => "Loose Stone",
        }
    }
}
pub fn nearest(run: &Run, h: &Hero) -> Option<(Interaction, f32)> {
    let kind = run.room().kind;
    let state = run.college_progress();
    let mut best: Option<(Interaction, f32)> = None;
    let mut offer = |target, room, spot: (f32, f32), reach| {
        if kind == room && near(h, spot, reach) {
            let distance = (h.x / TILE_UNITS - spot.0).hypot(h.y / TILE_UNITS - spot.1);
            if best.is_none_or(|(_, d)| distance < d) {
                best = Some((target, distance));
            }
        }
    };
    offer(Interaction::Wizard, RoomKind::WizardStudy, WIZARD, 2.0);
    offer(Interaction::Bestiary, RoomKind::WizardStudy, LECTERN, 1.5);
    if state.active("sigils") {
        for (i, &(room, at)) in SIGILS.iter().enumerate() {
            if [state.bookshelf, state.stars, state.familiar][i] && state.sigils & (1 << i) == 0 {
                offer(Interaction::Sigil(i), room, at, 1.2);
            }
        }
    }
    offer(Interaction::Shelf, RoomKind::TowerEntry, BOOKSHELF, 1.5);
    offer(Interaction::Stars, RoomKind::TowerStars, STAR_MAP, 1.5);
    offer(Interaction::Familiar, RoomKind::WizardStudy, FAMILIAR, 1.5);
    for (i, &(room, at, _, _)) in CLUES.iter().enumerate() {
        offer(Interaction::Clue(i), room, at, 1.5);
    }
    offer(
        Interaction::Book,
        RoomKind::CollegeLibrary,
        (12.0, 6.0),
        1.5,
    );
    offer(Interaction::Stone, RoomKind::TowerStairs, (6.0, 6.0), 1.5);
    best
}
impl Run {
    pub fn college_progress(&self) -> College {
        let mut c = self.home.college.clone();
        settle(&mut c, &mut Spoils::default(), &self.marks);
        c
    }
    pub(super) fn deal_college_spell(&mut self, id: u32) {
        if self.home.college.done.contains("map")
            && let Some(card) = self.book.get("hedge-maze")
            && let Some(hero) = self.players.get_mut(&id)
        {
            hero.equip_spell(card);
        }
    }
    fn college_mark(&mut self, key: &str, id: u32, note: &str) {
        self.marks.insert(key.into(), 1);
        self.sounds.push("spoil_pickup");
        self.found = Some((self.tick, id, note.into()));
    }
    pub(super) fn tick_college(&mut self, inputs: &BTreeMap<u32, Input>) {
        let held: BTreeSet<u32> = self
            .players
            .keys()
            .copied()
            .filter(|id| inputs.get(id).is_some_and(|i| i.valid() && i.interact))
            .collect();
        let pressed: Vec<u32> = held.difference(&self.college.held).copied().collect();
        self.college.held = held;
        let kind = self.room().kind;
        let c = self.college_progress();
        if kind == RoomKind::TowerEntry && c.bookshelf {
            let mut changed = false;
            for x in 3..7 {
                if self.dungeon.rooms[self.at].tile(x, 4) == Tile::Block {
                    self.dungeon.rooms[self.at].set(x as usize, 4, Tile::Floor);
                    changed = true;
                }
            }
            if changed {
                self.dungeon.world_revision = self.dungeon.world_revision.wrapping_add(1);
            }
        }
        let live = self
            .players
            .values()
            .filter(|h| h.hp > 0 && !h.stone && h.privy == 0)
            .count();
        if self.dungeon.depth >= 3 && live > 0 && c.active("map") && c.maps.len() < 4 {
            let (cellx, celly) = self.room().cell;
            self.marks.insert(
                format!(
                    "college:map:{}:{}:{}:{cellx}:{celly}",
                    self.raid_id, self.dungeon.depth, self.dungeon.pack as u8
                ),
                1,
            );
        }
        if let Some(owner) = self.college.owner {
            if kind != RoomKind::WizardStudy
                || !self
                    .players
                    .get(&owner)
                    .is_some_and(|h| near(h, WIZARD, 4.5) || near(h, LECTERN, 2.5))
            {
                self.college.close();
                return;
            }
            let input = inputs.get(&owner).copied().unwrap_or_default();
            let nav = if input.valid() {
                input.aim_y.signum()
            } else {
                0
            };
            let count = match self.college.page {
                Page::Menu => 4,
                Page::Topics => TOPICS.len(),
                Page::Quests => QUESTS.len(),
                _ => 0,
            };
            if count > 0 && nav != 0 && nav != self.college.nav {
                self.college.selected = (i32::from(self.college.selected) + i32::from(nav))
                    .rem_euclid(count as i32) as u8;
            }
            self.college.nav = nav;
            if input.aim_x < 0 {
                self.college.page = Page::Menu;
                self.college.selected = 0;
            }
            if !pressed.contains(&owner) {
                return;
            }
            let selected = self.college.selected;
            match self.college.page {
                Page::Menu => {
                    self.college.page = match selected {
                        0 => Page::Topics,
                        1 => Page::Quests,
                        2 => Page::Bestiary,
                        _ => Page::Closed,
                    };
                    self.college.selected = 0;
                    if self.college.page == Page::Closed {
                        self.college.close();
                    }
                }
                Page::Topics => self.college.page = Page::Topic(selected),
                Page::Topic(_) => {
                    self.college.page = Page::Topics;
                    self.college.selected = 0;
                }
                Page::Quests => self.college.page = Page::Quest(selected),
                Page::Quest(i) => {
                    let q = &QUESTS[usize::from(i)];
                    if c.done.contains(q.id) {
                        self.college.page = Page::Quests;
                        self.college.selected = 0;
                    } else if !c.active(q.id) {
                        self.college_mark(
                            &format!("college:accept:{}", q.id),
                            owner,
                            "Research quest accepted. Return to Bartosz with your notes.",
                        );
                    } else if c.have(q) >= q.need {
                        self.college_mark(
                            &format!("college:turn-in:{}", q.id),
                            owner,
                            "Excellent notes! Your thanks will be saved with the realm.",
                        );
                    } else {
                        self.college.page = Page::Quests;
                        self.college.selected = 0;
                    }
                }
                _ => {
                    self.college.page = Page::Menu;
                    self.college.selected = 0;
                }
            }
            return;
        }
        for id in pressed {
            let Some(h) = self.players.get(&id) else {
                continue;
            };
            if h.hp == 0 || h.stone || h.privy != 0 {
                continue;
            }
            let target = nearest(self, h).map(|(target, _)| target);
            if matches!(target, Some(Interaction::Wizard | Interaction::Bestiary)) {
                self.college.owner = Some(id);
                self.college.page = if target == Some(Interaction::Bestiary) {
                    Page::Bestiary
                } else {
                    Page::Menu
                };
                self.college.selected = 0;
                self.college.nav = 0;
                return;
            }
            if let Some(Interaction::Sigil(i)) = target {
                self.college_mark(
                    &format!("college:sigil:{i}"),
                    id,
                    "A hidden sigil joins your notes.",
                );
                return;
            }
            let puzzle = match target {
                Some(Interaction::Shelf) => Some((
                    "college:bookshelf",
                    "A shelf slides aside. A small sigil waits behind it.",
                )),
                Some(Interaction::Stars) => Some((
                    "college:stars",
                    "The star map turns. Follow its brightest star east.",
                )),
                Some(Interaction::Familiar) => Some((
                    "college:familiar",
                    "The familiar purrs and points a paw toward a sigil.",
                )),
                _ => None,
            };
            if let Some((key, note)) = puzzle {
                self.college_mark(key, id, note);
                return;
            }
            if let Some(Interaction::Clue(i)) = target {
                let (_, _, key, note) = CLUES[i];
                self.college_mark(key, id, note);
                return;
            }
            if c.active("creatures")
                && let Some(e) = self.enemies.iter().find(|e| {
                    e.hp > 0
                        && matches!(
                            e.kind,
                            EnemyKind::Bat | EnemyKind::Skeleton | EnemyKind::Slime
                        )
                        && (h.x - e.x).hypot(h.y - e.y) < 6.0
                })
            {
                let name = match e.kind {
                    EnemyKind::Bat => "bat",
                    EnemyKind::Skeleton => "skeleton",
                    _ => "slime",
                };
                self.college_mark(
                    &format!("college:observe:{name}"),
                    id,
                    "Creature observed. A living subject makes the best notes.",
                );
                return;
            }
            if self.dungeon.depth > 0
                && self.room().cleared
                && let Some(at) = field_spot(self.room())
                && near(h, at, 1.5)
            {
                if c.active("tome")
                    && self.dungeon.pack == Pack::Archive
                    && kind == RoomKind::Treasure
                    && !c.tome
                {
                    self.college_mark(
                        "college:tome",
                        id,
                        "The lost tome is dry inside. Return it to Bartosz.",
                    );
                    return;
                }
                if c.active("spores") && self.dungeon.pack == Pack::Fungal && c.samples.len() < 3 {
                    let (x, y) = self.room().cell;
                    let mark = format!(
                        "college:sample:{}:{}:{}:{x}:{y}",
                        self.raid_id, self.dungeon.depth, self.dungeon.pack as u8
                    );
                    if !c
                        .samples
                        .contains(mark.trim_start_matches("college:sample:"))
                    {
                        self.college_mark(
                            &mark,
                            id,
                            "Spore jar sealed. Three different room patches, please.",
                        );
                        return;
                    }
                }
                if c.active("together")
                    && self.dungeon.depth >= 3
                    && !c.trip
                    && self.players.values().filter(|h| near(h, at, 3.5)).count() >= 2
                {
                    self.college_mark(
                        "college:together",
                        id,
                        "Two knights, one field notebook. Return to Bartosz together.",
                    );
                    return;
                }
            }
            if target == Some(Interaction::Book) {
                self.college_mark(
                    "college:library-note",
                    id,
                    "The floating book opens: ask a question, then test it.",
                );
                return;
            }
            if target == Some(Interaction::Stone) {
                self.college_mark(
                    "college:landing-note",
                    id,
                    "Under the loose stone: a sketch of a bat, wearing spectacles.",
                );
            }
        }
    }
}
#[cfg(test)]
#[path = "../../tests/rules/together_college__tests.rs"]
mod tests;

/// The private checkpoint stores these intents alongside countryside intents.
/// No general kill counters or arbitrary dialogue strings enter that journal.
pub fn intent_key(key: &str) -> bool {
    if CLUES.iter().any(|&(_, _, mark, _)| key == mark) {
        return true;
    }
    if [
        "college:tome",
        "college:together",
        "college:bookshelf",
        "college:stars",
        "college:familiar",
        "college:library-note",
        "college:landing-note",
    ]
    .contains(&key)
    {
        return true;
    }
    if (0..3).any(|i| key == format!("college:sigil:{i}")) {
        return true;
    }
    for prefix in ["college:accept:", "college:turn-in:"] {
        if let Some(id) = key.strip_prefix(prefix) {
            return quest(id).is_some();
        }
    }
    if let Some(id) = key.strip_prefix("college:observe:") {
        return OBSERVABLE.contains(&id);
    }
    for prefix in ["college:map:", "college:sample:"] {
        if let Some(id) = key.strip_prefix(prefix) {
            return field_key(id)
                && (prefix != "college:map:"
                    || id
                        .split(':')
                        .nth(1)
                        .and_then(|v| v.parse::<u32>().ok())
                        .is_some_and(|d| d >= 3));
        }
    }
    false
}
fn field_key(id: &str) -> bool {
    let parts: Vec<_> = id.split(':').collect();
    let cell_at = if parts.len() == 5 { 3 } else { 2 };
    (parts.len() == 4 || parts.len() == 5)
        && parts[0].parse::<u64>().is_ok()
        && parts[1].parse::<u32>().is_ok_and(|d| d >= 1)
        && (parts.len() == 4 || parts[2].parse::<u8>().is_ok_and(|p| p <= 5))
        && parts[cell_at..]
            .iter()
            .all(|v| v.parse::<i32>().is_ok_and(|n| (-128..=128).contains(&n)))
}
