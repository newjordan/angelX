//! The forges' goods: what King Brannoc's forges make once they burn, as a
//! chain with stockpiles.
//!
//! The mine-head's miners dig ore into a stockpile; the Ore-Forge smelts it
//! into ingots; the Bell-Forge, the Ember-Forge and the Type-Foundry draw
//! those ingots, and a spoil each from the realm's purse, for bells, blades
//! and type; the Spore-Kiln fires the spores its beds grow into brew.
//!
//! A rent day (each floor the party clears) is a shift. Every burning forge
//! makes up to its `batches` while its inputs last and its rack has room.
//! What a forge cannot make it says, and the fire shows it: smoke only
//! while it works, a cold hearth when it is starved. A knight can bank a
//! forge's fire (hold E on its plate) to save the spoils it would draw.
//!
//! What the goods do:
//! - bells hang at the holdings and call the folk to the wall: `BELL_CALL`
//!   more against a band of raiders (`holdings`);
//! - blades arm the men there, `BLADE_EDGE` more for each, a blade to a man;
//! - type prints Wren's extra notices, `TYPE_PER_NOTICE` for each
//!   (`bounties`);
//! - brew goes to the granary, `BREW_SACKS` sacks a brew.
//!
//! The run only marks (`goods:bank:*`); the rent day is a pure function of
//! the realm's state, so the chain is tested without a cockpit. Every
//! number here is a fixed one.

use super::barony::{self, Barony};
use super::holdings::{Holdings, STORES_CAP};
use super::home::Home;
use super::*;
use crate::realm::{Spoil, Spoils};

/// A good in a stockpile or on a rack.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Good {
    Ore,
    Ingot,
    Spore,
    Bell,
    Blade,
    Type,
    Brew,
}

impl Good {
    pub const ALL: [Good; 7] = [
        Good::Ore,
        Good::Ingot,
        Good::Spore,
        Good::Bell,
        Good::Blade,
        Good::Type,
        Good::Brew,
    ];

    pub fn word(self) -> &'static str {
        match self {
            Good::Ore => "ore",
            Good::Ingot => "ingot",
            Good::Spore => "spore",
            Good::Bell => "bell",
            Good::Blade => "blade",
            Good::Type => "type",
            Good::Brew => "brew",
        }
    }

    pub fn plural(self) -> &'static str {
        match self {
            Good::Ore => "ore",
            Good::Ingot => "ingots",
            Good::Spore => "spores",
            Good::Bell => "bells",
            Good::Blade => "blades",
            Good::Type => "type",
            Good::Brew => "brews",
        }
    }

    /// The most a stockpile or rack holds.
    pub const fn cap(self) -> u32 {
        match self {
            Good::Ore | Good::Spore => 24,
            Good::Ingot => 16,
            Good::Type => 12,
            Good::Bell | Good::Blade | Good::Brew => 6,
        }
    }
}

/// Ore the miners dig every rent day, once the Upper Workings are his.
pub const MINE_DIG: u32 = 8;
/// Spores the kiln's beds grow every rent day, while it burns.
pub const BED_GROW: u32 = 6;
/// Bells a holding's towers hang, and what each adds against a band.
pub const BELLS_HUNG: u32 = 4;
pub const BELL_CALL: u32 = 2;
/// Blades the armoury holds, and what each adds against a band (a blade
/// arms one man: a retainer or a garrison man).
pub const ARMOURY: u32 = 6;
pub const BLADE_EDGE: u32 = 1;
/// Sacks a brew pours into the granary.
pub const BREW_SACKS: u32 = 3;
/// Type a notice Wren prints costs, and the most she prints beyond her
/// three.
pub const TYPE_PER_NOTICE: u32 = 3;
pub const NOTICES_PRINTED: usize = 2;
/// Rent days one settlement runs at most.
pub const DAYS_MAX: usize = 8;

/// One forge's work: what a batch takes (goods from the stockpiles, a
/// spoil from the realm's purse), what it makes, and how many batches a
/// rent day.
pub struct Recipe {
    pub forge: &'static str,
    pub from: &'static [(Good, u32)],
    pub fuel: &'static [(Spoil, u32)],
    pub makes: (Good, u32),
    pub batches: u32,
}

/// The chain, in the order a shift runs it: ingots first, so the forges
/// after draw on what the Ore-Forge just made, and the earlier ones eat
/// first when ingots run short.
pub const RECIPES: [Recipe; 5] = [
    Recipe {
        forge: "ore-forge",
        from: &[(Good::Ore, 2)],
        fuel: &[],
        makes: (Good::Ingot, 1),
        batches: 4,
    },
    Recipe {
        forge: "bell-forge",
        from: &[(Good::Ingot, 2)],
        fuel: &[(Spoil::Wax, 1)],
        makes: (Good::Bell, 1),
        batches: 2,
    },
    Recipe {
        forge: "ember-forge",
        from: &[(Good::Ingot, 1)],
        fuel: &[(Spoil::Ember, 2)],
        makes: (Good::Blade, 1),
        batches: 2,
    },
    Recipe {
        forge: "type-foundry",
        from: &[(Good::Ingot, 1)],
        fuel: &[(Spoil::Bone, 1)],
        makes: (Good::Type, 3),
        batches: 2,
    },
    Recipe {
        forge: "spore-kiln",
        from: &[(Good::Spore, 3)],
        fuel: &[(Spoil::Ember, 1)],
        makes: (Good::Brew, 1),
        batches: 2,
    },
];

pub fn recipe(forge: &str) -> Option<&'static Recipe> {
    RECIPES.iter().find(|r| r.forge == forge)
}

impl Recipe {
    pub fn fuel(&self) -> Spoils {
        let mut s = Spoils::default();
        for &(spoil, n) in self.fuel {
            s.add(spoil, n);
        }
        s
    }

    /// What a batch is short of, by its word, if anything.
    fn short(&self, goods: &Goods, purse: &Spoils) -> Option<String> {
        self.from
            .iter()
            .find(|&&(good, n)| goods.have(good) < n)
            .map(|&(good, _)| good.word().to_string())
            .or_else(|| {
                self.fuel
                    .iter()
                    .find(|&&(spoil, n)| purse.get(spoil) < n)
                    .map(|&(spoil, _)| spoil.word().to_string())
            })
    }

    /// The recipe as a board writes it: `2 INGOT + 1 WAX = 1 BELL`.
    pub fn line(&self) -> String {
        let mut parts: Vec<String> = self
            .from
            .iter()
            .map(|&(good, n)| format!("{n} {}", good.word()))
            .collect();
        parts.extend(
            self.fuel
                .iter()
                .map(|&(spoil, n)| format!("{n} {}", spoil.word())),
        );
        format!(
            "{} = {} {}",
            parts.join(" + "),
            self.makes.1,
            self.makes.0.word()
        )
    }
}

/// How a place went at the last rent day.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Flow {
    /// It made (or dug) something.
    Working,
    /// It made nothing: an input ran short, by its word.
    Starved(String),
    /// It made nothing: its rack is full.
    Full,
}

/// A forge's fire, as the room shows it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Burn {
    /// Not lit.
    Cold,
    /// Lit since the last rent day: nothing made yet.
    Kindled,
    Working,
    /// Lit but starved of an input.
    Starved(String),
    /// Lit, its rack full.
    Full,
    /// Damped by a knight's hand.
    Banked,
}

/// The goods the barony keeps, inside `Home` (so a friend's mirror sees
/// them). Every field defaults, so an older realm save loads as one that
/// has made nothing yet.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Goods {
    #[serde(default)]
    pub stock: BTreeMap<Good, u32>,
    /// How each place went at the last rent day: `mine` and the forges, by
    /// id.
    #[serde(default)]
    pub flow: BTreeMap<String, Flow>,
    /// Forges whose fire a knight has banked.
    #[serde(default)]
    pub banked: Vec<String>,
    /// Rent days run, all told.
    #[serde(default)]
    pub days: u32,
}

impl Goods {
    pub fn is_empty(&self) -> bool {
        self == &Goods::default()
    }

    pub fn have(&self, good: Good) -> u32 {
        self.stock.get(&good).copied().unwrap_or(0)
    }

    /// Put `n` on its stockpile, to the cap. How many went on.
    fn add(&mut self, good: Good, n: u32) -> u32 {
        let had = self.have(good);
        let now = (had + n).min(good.cap());
        if now > 0 {
            self.stock.insert(good, now);
        }
        now - had
    }

    /// Take `n` off, as many as there are. How many came off.
    pub fn take(&mut self, good: Good, n: u32) -> u32 {
        let had = self.have(good);
        let took = n.min(had);
        if had == took {
            self.stock.remove(&good);
        } else {
            self.stock.insert(good, had - took);
        }
        took
    }

    pub fn is_banked(&self, forge: &str) -> bool {
        self.banked.iter().any(|f| f == forge)
    }

    /// How a forge's fire stands.
    pub fn burn(&self, b: &Barony, forge: &str) -> Burn {
        if !b.is_lit(forge) {
            return Burn::Cold;
        }
        if self.is_banked(forge) {
            return Burn::Banked;
        }
        match self.flow.get(forge) {
            None => Burn::Kindled,
            Some(Flow::Working) => Burn::Working,
            Some(Flow::Full) => Burn::Full,
            Some(Flow::Starved(of)) => Burn::Starved(of.clone()),
        }
    }

    /// The mine-head's stockpile is full: the miners down tools.
    pub fn mine_full(&self) -> bool {
        self.flow.get("mine") == Some(&Flow::Full)
    }

    /// The mine is digging: carts run the track.
    pub fn mine_digging(&self) -> bool {
        self.flow.get("mine") == Some(&Flow::Working)
    }

    /// What the goods look like, for a drawing kept between frames.
    pub fn print(&self) -> u64 {
        use std::hash::{Hash, Hasher};
        let mut h = std::collections::hash_map::DefaultHasher::new();
        for (good, n) in &self.stock {
            (*good as u8, *n).hash(&mut h);
        }
        for (id, flow) in &self.flow {
            id.hash(&mut h);
            match flow {
                Flow::Working => 1u8.hash(&mut h),
                Flow::Full => 2u8.hash(&mut h),
                Flow::Starved(of) => (3u8, of).hash(&mut h),
            }
        }
        self.banked.hash(&mut h);
        h.finish()
    }

    /// One shift of a forge: its batches, while the inputs last and the
    /// rack has room. How it went.
    fn shift(&mut self, r: &Recipe, purse: &mut Spoils) -> Flow {
        let (out, per) = r.makes;
        let mut made = 0;
        let mut stopped = Flow::Working;
        for _ in 0..r.batches {
            if self.have(out) + per > out.cap() {
                stopped = Flow::Full;
                break;
            }
            if let Some(of) = r.short(self, purse) {
                stopped = Flow::Starved(of);
                break;
            }
            for &(good, n) in r.from {
                self.take(good, n);
            }
            purse.take(&r.fuel());
            self.add(out, per);
            made += 1;
        }
        if made > 0 { Flow::Working } else { stopped }
    }
}

/// The racks empty into the realm: bells hang at the holdings, blades go
/// to the armoury, a brew is poured into the granary while a whole brew
/// fits. Type stays on its rack for Wren.
fn dispatch(g: &mut Goods, h: &mut Holdings) {
    let bells = g.have(Good::Bell).min(BELLS_HUNG.saturating_sub(h.bells));
    h.bells += g.take(Good::Bell, bells);
    let blades = g.have(Good::Blade).min(ARMOURY.saturating_sub(h.blades));
    h.blades += g.take(Good::Blade, blades);
    while g.have(Good::Brew) > 0 && h.stores + BREW_SACKS <= STORES_CAP {
        g.take(Good::Brew, 1);
        h.stores += BREW_SACKS;
    }
}

/// One rent day. True if anything changed.
fn day(home: &mut Home, purse: &mut Spoils) -> bool {
    let Home {
        barony: b,
        goods: g,
        holdings: h,
        ..
    } = home;
    // Nothing is dug, and no forge can be lit, before the workings are his.
    if !b.holds_workings() {
        return false;
    }
    let before = (g.clone(), h.bells, h.blades, h.stores);
    g.days += 1;
    let dug = g.add(Good::Ore, MINE_DIG);
    g.flow.insert(
        "mine".into(),
        if dug > 0 { Flow::Working } else { Flow::Full },
    );
    if b.is_lit("spore-kiln") && !g.is_banked("spore-kiln") {
        g.add(Good::Spore, BED_GROW);
    }
    dispatch(g, h);
    for r in &RECIPES {
        if !b.is_lit(r.forge) || g.is_banked(r.forge) {
            g.flow.remove(r.forge);
            continue;
        }
        let flow = g.shift(r, purse);
        g.flow.insert(r.forge.into(), flow);
    }
    dispatch(g, h);
    before != (g.clone(), h.bells, h.blades, h.stores)
}

/// The rent days of a settlement: the party cleared `floors` floors.
/// True if anything changed.
pub fn rent_days(home: &mut Home, purse: &mut Spoils, floors: usize) -> bool {
    let mut changed = false;
    for _ in 0..floors.min(DAYS_MAX) {
        changed |= day(home, purse);
    }
    changed
}

/// What the bells and blades add against a band that the walls, the
/// garrison and the retainers fall `short` of: the bells and blades it
/// takes (the fewest of them, bells spared), or none when they fall short
/// too. A blade needs a man to carry it, so `blades` is what the men can
/// arm.
pub fn cover(short: u32, bells: u32, blades: u32) -> Option<(u32, u32)> {
    let mut best: Option<(u32, u32)> = None;
    for rang in 0..=bells {
        let left = short.saturating_sub(rang * BELL_CALL);
        let armed = left.div_ceil(BLADE_EDGE);
        if armed > blades {
            continue;
        }
        if best.is_none_or(|(r, a)| rang + armed < r + a) {
            best = Some((rang, armed));
        }
    }
    best
}

/// A forge's fire banked or stoked by a knight's hand, from the marks.
pub fn settle(
    realm: &mut super::activity::Realm<'_>,
    marks: &BTreeMap<String, u32>,
) -> super::activity::Settled {
    let mut out = super::activity::Settled::default();
    let mut ids: Vec<&str> = marks
        .keys()
        .filter_map(|m| m.strip_prefix("goods:bank:"))
        .filter_map(|m| m.split(':').next())
        .collect();
    ids.sort_unstable();
    ids.dedup();
    for id in ids {
        let Some(f) = barony::forge(id) else { continue };
        if recipe(f.id).is_none() || !realm.home.barony.is_lit(f.id) {
            continue;
        }
        let g = &mut realm.home.goods;
        g.flow.remove(f.id);
        out.changed = true;
        if g.is_banked(f.id) {
            g.banked.retain(|b| b != f.id);
            out.said.push((
                format!("forge_stoked:{}", f.id),
                format!("{} is stoked: it works again at the next rent day", f.name),
            ));
        } else {
            g.banked.push(f.id.into());
            out.said.push((
                format!("forge_banked:{}", f.id),
                format!("{} is banked: it draws nothing until it is stoked", f.name),
            ));
        }
    }
    out
}

impl Run {
    /// A knight holding E on a lit forge's plate banks its fire, or stokes
    /// it again.
    pub(super) fn tick_goods(&mut self, inputs: &BTreeMap<u32, Input>) {
        if self.room().kind != RoomKind::Forge {
            return;
        }
        let Some(f) = barony::forge_of(self.dungeon.pack) else {
            return;
        };
        if recipe(f.id).is_some() && self.home.barony.is_lit(f.id) {
            self.hold_plate(inputs, barony::FORGE_PLATE, &format!("goods:bank:{}", f.id));
        }
    }
}

/// A forge-hall's bins (tiles): the stockpile its inputs are drawn from,
/// west of the furnace, and the rack its output waits on, east of it.
pub const IN_BIN: (i32, i32, i32, i32) = (3, 3, 3, 2);
pub const OUT_BIN: (i32, i32, i32, i32) = (18, 3, 3, 2);

/// The bins stand on the floor: walls to walk round.
pub fn bins(room: &mut Room) {
    for (c, r, w, h) in [IN_BIN, OUT_BIN] {
        for row in r..r + h {
            for col in c..c + w {
                room.set(col as usize, row as usize, Tile::Block);
            }
        }
    }
}

#[cfg(test)]
#[path = "../../tests/rules/goods__tests.rs"]
mod tests;
