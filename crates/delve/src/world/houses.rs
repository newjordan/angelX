//! The model houses: one castle per model family, and the knights it raises.
//!
//! Every family that serves in angelX holds a great castle in the realm's
//! southern March. The family's own model named the castle and six knights
//! of its house, chose how the castle is built and the two colours of its
//! banner (`assets/realm/houses/houses.json`, asked once per family through
//! its own route). A self-hosted model is a mode of the `local` club and
//! belongs to its model family's house; no machine ever has a castle.
//!
//! Which house is serving is a fact the app publishes each frame
//! ([`note_serving`]): the lead (the route in hand, or the route a running
//! turn resolved to) and the houses seated with it by a formation. The
//! overworld raises those banners and sends those knights out; the Delve
//! names the party's knights from those houses.

use std::cell::RefCell;
use std::sync::OnceLock;

use serde::Deserialize;


/// A house: an index into [`all`], in the Round Table's own order.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct HouseId(pub u8);

/// How a castle is built, as its model chose from the kit.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Walls {
    PaleStone,
    DarkStone,
    Plaster,
    Timber,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Tops {
    Battlements,
    Pointed,
    Domes,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RoofKind {
    Slate,
    Tile,
    Thatch,
}

/// One knight of a house, as the house's model named them.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Knight {
    pub name: String,
    pub epithet: String,
}

/// One model family's house.
#[derive(Clone, Debug)]
pub struct House {
    /// The roster key (`deepseek`, `sol`, …), also what saves record.
    pub key: &'static str,
    /// What the family is called on the map.
    pub family: &'static str,
    pub castle: String,
    pub knights: Vec<Knight>,
    pub walls: Walls,
    pub tops: Tops,
    pub towers: u8,
    pub roof: RoofKind,
    /// Banner inks (signal bank): the field and its charge.
    pub field: char,
    pub charge: char,
    /// No answer yet: the castle stands, its knights are unnamed.
    pub placeholder: bool,
}

impl House {
    /// The castle's name in capitals for a map label.
    pub fn label(&self) -> String {
        self.family.to_uppercase()
    }

    /// The knight of this house who wears kit `slot` (0-based), if named.
    pub fn knight(&self, slot: usize) -> Option<&Knight> {
        (!self.knights.is_empty()).then(|| &self.knights[slot % self.knights.len()])
    }
}

/// The houses in placement order: each key and its family's name.
const ORDER: [(&str, &str); 16] = [
    ("sol", "Sol"),
    ("luna", "Luna"),
    ("grok", "Grok"),
    ("deepseek", "DeepSeek"),
    ("glm", "GLM"),
    ("kimi", "Kimi"),
    ("qwen", "Qwen"),
    ("muse", "Muse"),
    ("longcat", "LongCat"),
    ("hy", "Hy"),
    ("nemotron", "Nemotron"),
    ("gemma", "Gemma"),
    ("inkling", "Inkling"),
    ("laguna", "Laguna"),
    ("north", "North"),
    ("astra", "Astra"),
];

const ASSET: &str = include_str!("../../../../cockpit/assets/realm/houses/houses.json");

#[derive(Deserialize)]
struct Asset {
    colours: std::collections::BTreeMap<String, String>,
    houses: Vec<Entry>,
}

#[derive(Deserialize)]
struct Entry {
    key: String,
    #[serde(default)]
    castle: String,
    #[serde(default)]
    knights: Vec<EntryKnight>,
    #[serde(default)]
    walls: String,
    #[serde(default)]
    tops: String,
    #[serde(default)]
    towers: u8,
    #[serde(default)]
    roof: Option<String>,
    #[serde(default)]
    field: String,
    #[serde(default)]
    charge: String,
    #[serde(default)]
    placeholder: bool,
}

#[derive(Deserialize)]
struct EntryKnight {
    name: String,
    #[serde(default)]
    epithet: String,
}

fn parse() -> Vec<House> {
    let asset: Asset = serde_json::from_str(ASSET).unwrap_or(Asset {
        colours: Default::default(),
        houses: Vec::new(),
    });
    let ink = |name: &str, fallback: char| {
        asset
            .colours
            .get(name.trim())
            .and_then(|s| s.chars().next())
            .unwrap_or(fallback)
    };
    ORDER
        .iter()
        .map(|&(key, family)| {
            let entry = asset.houses.iter().find(|e| e.key == key);
            let Some(e) = entry.filter(|e| !e.placeholder && !e.knights.is_empty()) else {
                // Not asked yet: a bare castle in the house's place, no names.
                return House {
                    key,
                    family,
                    castle: format!("{family} castle"),
                    knights: Vec::new(),
                    walls: Walls::PaleStone,
                    tops: Tops::Battlements,
                    towers: 2,
                    roof: RoofKind::Slate,
                    field: 'j',
                    charge: 'J',
                    placeholder: true,
                };
            };
            let lower = |s: &str| s.trim().to_ascii_lowercase();
            House {
                key,
                family,
                castle: e.castle.trim().to_string(),
                knights: e
                    .knights
                    .iter()
                    .map(|k| Knight {
                        name: k.name.trim().to_string(),
                        epithet: k.epithet.trim().to_string(),
                    })
                    .collect(),
                walls: match lower(&e.walls).as_str() {
                    "dark stone" => Walls::DarkStone,
                    "plaster and beams" | "plaster" => Walls::Plaster,
                    "timber" => Walls::Timber,
                    _ => Walls::PaleStone,
                },
                tops: match lower(&e.tops).as_str() {
                    "pointed roofs" | "pointed" => Tops::Pointed,
                    "domes" | "dome" => Tops::Domes,
                    _ => Tops::Battlements,
                },
                towers: e.towers.clamp(1, 3),
                roof: match e.roof.as_deref().map(lower).as_deref() {
                    Some("red tile") | Some("tile") => RoofKind::Tile,
                    Some("thatch") => RoofKind::Thatch,
                    _ => RoofKind::Slate,
                },
                field: ink(&e.field, '1'),
                charge: ink(&e.charge, '5'),
                placeholder: false,
            }
        })
        .collect()
}

/// Every house, in placement order.
pub fn all() -> &'static [House] {
    static HOUSES: OnceLock<Vec<House>> = OnceLock::new();
    HOUSES.get_or_init(parse)
}

pub fn get(id: HouseId) -> &'static House {
    &all()[usize::from(id.0) % all().len()]
}

pub fn ids() -> impl Iterator<Item = HouseId> {
    (0..all().len() as u8).map(HouseId)
}

/// A house by its saved key.
pub fn by_key(key: &str) -> Option<HouseId> {
    ORDER
        .iter()
        .position(|&(k, _)| k.eq_ignore_ascii_case(key.trim()))
        .map(|i| HouseId(i as u8))
}

/// A house by its family or its castle's name, as an operator types it.
pub fn find(name: &str) -> Option<HouseId> {
    let squash = |text: &str| {
        text.chars()
            .filter(char::is_ascii_alphanumeric)
            .map(|c| c.to_ascii_lowercase())
            .collect::<String>()
    };
    let wanted = squash(name);
    if wanted.is_empty() {
        return None;
    }
    ids().find(|&id| {
        let h = get(id);
        squash(h.key) == wanted || squash(&h.castle) == wanted || squash(h.family) == wanted
    })
}

/// Who is serving right now.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Serving {
    /// The house whose route is in hand (or answering the running turn).
    pub lead: Option<HouseId>,
    /// Other houses a formation has seated beside the lead, in seat order.
    pub seated: Vec<HouseId>,
    /// A turn is running.
    pub turn: bool,
}

impl Serving {
    /// Every house with a knight out: the lead first, then the seated.
    pub fn houses(&self) -> impl Iterator<Item = HouseId> + '_ {
        self.lead.into_iter().chain(self.seated.iter().copied())
    }
}

thread_local! {
    // Published by the app on its own thread each frame; the Delve reads it
    // on the same thread when it dresses a party.
    static SERVING: RefCell<Serving> = RefCell::new(Serving::default());
}

/// Publish who is serving (the app, once a frame).
pub fn note_serving(serving: &Serving) {
    SERVING.with(|cell| {
        let mut cell = cell.borrow_mut();
        if *cell != *serving {
            *cell = serving.clone();
        }
    });
}

pub fn serving() -> Serving {
    SERVING.with(|cell| cell.borrow().clone())
}

/// The house a party seat's knight comes from: seat 1 is the serving house,
/// later seats the houses a formation seated beside it, then the castles
/// next along the March. `None` when nothing serves (the stub route): the
/// Keep's own household takes the field.
pub fn for_seat(serving: &Serving, seat: u32) -> Option<HouseId> {
    let lead = serving.lead?;
    let mut order: Vec<HouseId> = Vec::new();
    for house in serving.houses() {
        if !order.contains(&house) {
            order.push(house);
        }
    }
    let n = all().len() as u8;
    for step in 1..n {
        let next = HouseId((lead.0 + step) % n);
        if !order.contains(&next) {
            order.push(next);
        }
    }
    order
        .get(seat.saturating_sub(1) as usize % order.len())
        .copied()
}

/// Where each house's castle stands on the March, in realm tiles: the
/// footprint `(tx, ty, tw, th)` of its curtain wall, by house index. A
/// castle is a great walled exterior, never seen whole: 14 to 21 tiles
/// across and 8 to 10 deep, closed ground with its gate in the middle of
/// its south face. Eight stand with their backs to the Marchwater and
/// their gates on the frontier road, eight more behind the road with their
/// gates on the far lane; each wholly inside one chunk, the two rows
/// staggered, six tiles or more of open ground between any two. Set by
/// hand (`docs/KINGDOM-OPEN.md`).
const SITES: [(i32, i32, i32, i32); 16] = [
    // Along the Marchwater, west to east: gates on the frontier road.
    (3, 146, 16, 8),
    (30, 148, 14, 8),
    (50, 147, 18, 8),
    (80, 146, 16, 8),
    (102, 147, 15, 8),
    (123, 146, 20, 10),
    (149, 146, 15, 8),
    (170, 146, 21, 9),
    // Behind the road, west to east: gates on the far lane.
    (8, 162, 16, 8),
    (32, 162, 15, 8),
    (53, 163, 14, 8),
    (98, 161, 19, 9),
    (123, 162, 17, 8),
    (147, 162, 19, 9),
    (172, 163, 14, 8),
    (192, 161, 17, 9),
];

/// A castle's footprint in realm tiles `(tx, ty, tw, th)`: its curtain
/// wall and all it holds, closed ground for the walker.
pub fn footprint(id: HouseId) -> (i32, i32, i32, i32) {
    SITES[usize::from(id.0) % SITES.len()]
}

/// The road tile before a castle's gate, where its knight stands at home:
/// the middle of its south face, a tile out.
pub fn stand(id: HouseId) -> (i32, i32) {
    let (tx, ty, tw, th) = footprint(id);
    (tx + tw / 2, ty + th)
}

/// Every house castle's footprint, for the walker's closed ground. The
/// Keep and the Dragon Keep are great castles too: `super::castles` has
/// them all.
pub fn footprints() -> impl Iterator<Item = (i32, i32, i32, i32)> {
    ids().map(footprint)
}

#[cfg(test)]
#[path = "../../tests/world/houses__tests.rs"]
mod tests;
