//! A working forest circuit: the maze is walkable collision, never a text menu.
//! The cook's mushrooms are party-shared intents; settlement pays once.
//!
//! The errand is a forage: the badger cook wants a Boot, a Hat and a Beard
//! mushroom, and all three grow in the hedge maze at once, so the party
//! splits up. The Boot kicks the first knight who reaches for it. Each
//! picker carries theirs over their head back to the cook, and whoever
//! picked the most is made Deputy Stirrer (the crown).
use super::countryside::Country;
use super::{BTreeMap, HZ, Input, Room, RoomKind, Run, Spoil, Spoils, TILE_UNITS, Tile};
use serde::{Deserialize, Serialize};

/// The mushrooms, in the order an older save picked them.
pub const MUSHROOMS: [&str; 3] = ["BOOT", "HAT", "BEARD"];
/// How far the Boot kicks (units).
const KICK: f32 = 3.0;

/// The forage under way: who carries each mushroom (0 while it still
/// grows), and whether the Boot has kicked yet.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Forage {
    pub carried: [u32; 3],
    pub kicked: bool,
}

impl Forage {
    pub fn valid(&self) -> bool {
        self.carried.iter().all(|&id| id <= super::MAX_PLAYERS)
    }
}

pub const PLACES: [RoomKind; 3] =
    [RoomKind::Timberyard, RoomKind::HedgeMaze, RoomKind::Bothy];
pub const VISIT_MARKS: [&str; 3] = ["country:wood:0", "country:wood:1", "country:wood:2"];
pub const QUEST_MARKS: [&str; 5] = [
    "country:soup:0",
    "country:soup:1",
    "country:soup:2",
    "country:soup:3",
    "country:soup:4",
];
pub const ERRAND: [(RoomKind, (f32, f32)); 5] = [
    (RoomKind::Bothy, (5.0, 8.5)),
    (RoomKind::HedgeMaze, (4.0, 4.5)),
    (RoomKind::HedgeMaze, (10.0, 9.0)),
    (RoomKind::HedgeMaze, (19.0, 5.0)),
    (RoomKind::Bothy, (5.0, 8.5)),
];
pub const HEDGES: [(usize, usize, usize, usize); 5] = [
    (6, 2, 1, 4),
    (6, 8, 1, 4),
    (12, 4, 1, 3),
    (12, 9, 1, 3),
    (15, 8, 6, 1),
];
pub const WAGON: (i32, i32, i32, i32) = (19, 10, 2, 1);
/// The hedge maze's prize: a golden acorn behind the long hedge (tiles).
/// The first knight to it each outing wears the crown; the realm is paid
/// once, ever.
pub const ACORN: (f32, f32) = (18.0, 11.0);
pub const ACORN_MARK: &str = "country:acorn";
pub const ACORN_GOLD: u32 = 30;
pub const ORCHARD_WAGON: (i32, i32, i32, i32) = (8, 11, 2, 1);

pub fn stage(country: &Country, marks: &BTreeMap<String, u32>) -> u8 {
    let mut s = country.soup.min(5);
    while s < 5 && marks.contains_key(QUEST_MARKS[usize::from(s)]) {
        s += 1;
    }
    s
}
pub fn shape(room: &mut Room) {
    let rects: &[(usize, usize, usize, usize)] = match room.kind {
        RoomKind::Timberyard => &[(3, 1, 5, 2), (16, 1, 5, 2), (3, 11, 5, 2), (16, 11, 5, 2)],
        RoomKind::HedgeMaze => &HEDGES,
        RoomKind::Bothy => &[(3, 2, 5, 3)],
        _ => &[],
    };
    for &(c, r, w, h) in rects {
        for y in r..r + h {
            for x in c..c + w {
                room.set(x, y, Tile::Block);
            }
        }
    }
    let stair = match room.kind {
        RoomKind::Bothy => Some(WAGON),
        RoomKind::Orchard => Some(ORCHARD_WAGON),
        _ => None,
    };
    if let Some((c, r, w, h)) = stair {
        for y in r..r + h {
            for x in c..c + w {
                room.set(x as usize, y as usize, Tile::Stairs);
            }
        }
    }
}
impl Run {
    pub(super) fn tick_woodland(&mut self, inputs: &BTreeMap<u32, Input>) {
        if !self
            .players
            .values()
            .any(|h| h.hp > 0 && !h.stone && h.privy == 0)
        {
            return;
        }
        let kind = self.room().kind;
        if let Some(i) = PLACES.iter().position(|&k| k == kind) {
            if self.home.countryside.wood_visited & (1 << i) == 0 {
                self.marks.insert(VISIT_MARKS[i].into(), 1);
            }
        }
        if kind == RoomKind::HedgeMaze && !self.marks.contains_key(ACORN_MARK) {
            let finder = self.players.iter().find_map(|(&id, h)| {
                (h.hp > 0
                    && !h.stone
                    && inputs.get(&id).is_some_and(|i| i.valid() && i.interact)
                    && (h.x - ACORN.0 * TILE_UNITS).hypot(h.y - ACORN.1 * TILE_UNITS) < 1.4 * TILE_UNITS)
                    .then_some(id)
            });
            if let Some(id) = finder {
                self.marks.insert(ACORN_MARK.into(), 1);
                for (&other, hero) in &mut self.players {
                    if other == id {
                        hero.laurel = true;
                    }
                }
                self.sounds.push("chest_open");
                self.found = Some((self.tick, id, "THE GOLDEN ACORN!".into()));
                return;
            }
        }
        let s = stage(&self.home.countryside, &self.marks);
        if (1..5).contains(&s) {
            self.tick_forage(inputs, s);
            return;
        }
        self.forage = None;
        if s == 5 || ERRAND[usize::from(s)].0 != kind {
            return;
        }
        let at = ERRAND[usize::from(s)].1;
        let actor = self.players.iter().find_map(|(&id, h)| {
            (h.hp > 0
                && !h.stone
                && h.privy == 0
                && inputs.get(&id).is_some_and(|i| i.valid() && i.interact)
                && (h.x - at.0 * TILE_UNITS).hypot(h.y - at.1 * TILE_UNITS) < 1.4)
                .then_some(id)
        });
        let Some(actor) = actor else {
            return;
        };
        self.marks.insert(QUEST_MARKS[usize::from(s)].into(), 1);
        self.sounds.push("spoil_pickup");
        self.found = Some((self.tick, actor, "Mushrooms! BOOT, HAT, BEARD. In the hedge maze.".into()));
    }

    /// The forage: pick in the maze (any order, anyone), carry them home.
    fn tick_forage(&mut self, inputs: &BTreeMap<u32, Input>, s: u8) {
        let kind = self.room().kind;
        // An older save may already have some in the pot.
        let already = usize::from(s - 1).min(3);
        let mut forage = self.forage.take().unwrap_or_default();
        let pressing = |at: (f32, f32), players: &BTreeMap<u32, super::hero::Hero>| {
            players.iter().find_map(|(&id, h)| {
                (h.hp > 0
                    && !h.stone
                    && h.privy == 0
                    && inputs.get(&id).is_some_and(|i| i.valid() && i.interact)
                    && (h.x - at.0 * TILE_UNITS).hypot(h.y - at.1 * TILE_UNITS) < 1.4 * TILE_UNITS)
                    .then_some(id)
            })
        };
        if kind == RoomKind::HedgeMaze {
            for i in already..3 {
                if forage.carried[i] != 0 {
                    continue;
                }
                let at = ERRAND[1 + i].1;
                let Some(id) = pressing(at, &self.players) else {
                    continue;
                };
                if i == 0 && !forage.kicked {
                    // The Boot kicks first; a second try picks it.
                    forage.kicked = true;
                    let h = self.players.get_mut(&id).unwrap();
                    // Away from the Boot; straight back if right on top of it.
                    let (ax, ay) = (h.x - at.0 * TILE_UNITS, h.y - at.1 * TILE_UNITS);
                    let len = ax.hypot(ay);
                    let (ux, uy) = if len < 0.3 { (0.0, 1.0) } else { (ax / len, ay / len) };
                    let room = &self.dungeon.rooms[self.at];
                    let open = |x: f32, y: f32| room.tile((x / TILE_UNITS) as i32, (y / TILE_UNITS) as i32) == Tile::Floor;
                    if let Some(to) = [KICK, KICK / 2.0]
                        .into_iter()
                        .map(|d| (h.x + ux * d, h.y + uy * d))
                        .find(|&(x, y)| open(x, y))
                    {
                        (h.x, h.y) = to;
                    }
                    h.invulnerable = h.invulnerable.max(HZ / 2);
                    self.sounds.push("hero_hurt");
                    self.found = Some((self.tick, id, "THE BOOT KICKED BACK!".into()));
                    continue;
                }
                forage.carried[i] = id;
                self.sounds.push("spoil_pickup");
                self.found = Some((self.tick, id, [
                    "BOOT PICKED. IT SMELLS OF KNIGHT.",
                    "HAT PICKED. A MILLINER PROTESTS.",
                    "BEARD PICKED. IT TICKLES.",
                ][i].into()));
            }
        }
        if kind == RoomKind::Bothy {
            if let Some(id) = pressing(ERRAND[4].1, &self.players) {
                if forage.carried[already..].iter().all(|&c| c != 0) {
                    for mark in &QUEST_MARKS[usize::from(s)..] {
                        self.marks.insert((*mark).into(), 1);
                    }
                    self.deputy_stirrer(&forage, id);
                    return;
                }
                if self.found.as_ref().is_none_or(|f| self.tick >= f.0 + u64::from(2 * HZ)) {
                    self.found = Some((self.tick, id, "COOK: THREE MUSHROOMS, PLEASE.".into()));
                }
            }
        }
        self.forage = Some(forage);
    }

    /// Soup's on: the knight who picked the most stirs it (the crown).
    fn deputy_stirrer(&mut self, forage: &Forage, giver: u32) {
        let mut picks: BTreeMap<u32, u8> = BTreeMap::new();
        for &id in forage.carried.iter().filter(|&&id| id != 0) {
            *picks.entry(id).or_default() += 1;
        }
        let best = picks
            .iter()
            .max_by_key(|(id, n)| (**n, std::cmp::Reverse(**id)))
            .map_or(giver, |(&id, _)| id);
        if self.party_len() > 1 {
            for (&id, hero) in self.players.iter_mut() {
                hero.laurel = id == best;
            }
        }
        let name = self.players.get(&best).map_or_else(String::new, |h| h.name.to_uppercase());
        self.sounds.push("chest_open");
        self.found = Some((self.tick, best, format!("SOUP'S ON! {name} IS DEPUTY STIRRER.")));
    }
}
pub fn settle(
    country: &mut Country,
    treasury: &mut Spoils,
    marks: &BTreeMap<String, u32>,
    news: &mut Vec<String>,
) {
    for (i, mark) in VISIT_MARKS.iter().enumerate() {
        if marks.contains_key(*mark) {
            country.wood_visited |= 1 << i;
        }
    }
    if marks.contains_key(ACORN_MARK) && !country.acorn_paid {
        country.acorn_paid = true;
        treasury.add(Spoil::Gold, ACORN_GOLD);
        news.push(format!("The golden acorn: {ACORN_GOLD} gold."));
    }
    country.soup = stage(country, marks);
    if country.soup == 5 && !country.soup_paid {
        country.soup_paid = true;
        treasury.add(Spoil::Gold, 70);
        treasury.add(Spoil::Wax, 2);
        news.push("The Badger's Soup: 70 gold, 2 wax.".into());
    }
}

#[cfg(test)]
#[path = "../../tests/rules/together_forage__tests.rs"]
mod tests;
