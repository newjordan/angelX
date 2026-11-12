//! The works of Bomrekedan: the fort's building projects, the kingdom loop's
//! first real use. Each work stands for something the fortress was short of
//! (`expansions/bomrekedan/kingdom/buildouts.json`, its id, cost and gift),
//! and has a sign on walkable floor of its level, by the place it is about.
//!
//! A knight holding E at a work's sign orders its next level as a raising
//! (`raisings::Order`, plan `bom:<id>`, no tile): the order waits with the
//! kingdom's raisings for the next home claim, and the realm pays for it
//! there, from the one treasury (`settle`, called from `home_settlement`).
//! Only the claim writes `Home::bomrekedan`; the run reads it back when the
//! claim is delivered (`rebuild_home`). A built work gives the delve what its
//! gift gives (`raisings::Gift`, in `raisings::raised_gifts`).

use crate::realm::{Spoil, Spoils};
use crate::rules::raisings::{Gift, Order};
use crate::rules::{Input, RoomKind, Run, TILE_UNITS, home};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// A work's raise order is `bom:` and its id.
pub const PREFIX: &str = "bom:";

/// A building project of the fort.
pub struct Work {
    /// Its id in `buildouts.json`.
    pub id: &'static str,
    pub name: &'static str,
    /// The level of the fort its sign stands on.
    pub level: RoomKind,
    /// The sign: a plate of walkable floor, in level tiles (col, row, w, h).
    pub sign: (i32, i32, i32, i32),
    /// What the first level wants; each next level wants that many times
    /// more (as a raising's `Plan::price`).
    pub wants: &'static [(Spoil, u32)],
    pub gift: Gift,
    /// The gift, on the sign.
    pub says: &'static str,
    /// Its top level.
    pub most: u8,
}

/// The first works. Costs are `buildouts.json`'s `cost.wants`; a gift the
/// file proposes but the game has not (Edge, Draught) or none (the dining
/// halls) is the nearest gift the game has.
pub const WORKS: [Work; 6] = [
    // Rank 6 shortage, refined coal: a second smelter beside the one there
    // was, for coke. A lot: never built in the fort.
    Work {
        id: "coke-smelter",
        name: "The Coke Smelter",
        level: RoomKind::BomForges,
        sign: (24, 33, 3, 2),
        wants: &[(Spoil::Gold, 300), (Spoil::Ore, 20), (Spoil::Ember, 8)],
        gift: Gift::Ward,
        says: "WARDED ON EVERY ARRIVAL",
        most: 3,
    },
    // Rank 3 shortage, steel bars: the smelter and the forge as one line.
    // The file proposes Edge (better gear); steel kit is mail meanwhile.
    Work {
        id: "steel-line",
        name: "The Steel Line",
        level: RoomKind::BomForges,
        sign: (44, 21, 3, 2),
        wants: &[(Spoil::Gold, 650), (Spoil::Ore, 40), (Spoil::Ember, 10), (Spoil::Forgeheart, 2)],
        gift: Gift::Mail,
        says: "MAIL MORE EACH DELVE",
        most: 3,
    },
    // Rank 2 shortage, drink: eight stills and one brewer. The file
    // proposes Draught; the taverns pour, as the Tavern's gift.
    Work {
        id: "still",
        name: "The Stills",
        level: RoomKind::BomFarms,
        // In the still room under its third still, east of Zuglar and out
        // of his hearing: E held here is the sign's alone.
        sign: (66, 51, 3, 2),
        wants: &[(Spoil::Gold, 120), (Spoil::Wax, 4)],
        gift: Gift::Purse,
        says: "GOLD ON EVERY NEW FLOOR",
        most: 3,
    },
    // Rank 1 shortage, food: the halls a full table is set in. The file
    // gives them no gift; a fed fort wishes.
    Work {
        id: "dining-halls",
        name: "The Dining Halls",
        level: RoomKind::BomNobleHalls,
        sign: (57, 51, 3, 2),
        wants: &[(Spoil::Gold, 90)],
        gift: Gift::Wishes,
        says: "A WISH MORE EACH DELVE",
        most: 3,
    },
    // The windmills and the screw pump stack: every drop of the fort's
    // water is lifted. Its sign stands east of the surface's pumps.
    Work {
        id: "machine-works",
        name: "The Machine Works",
        level: RoomKind::BomGate,
        sign: (89, 59, 3, 2),
        wants: &[(Spoil::Gold, 650), (Spoil::Ore, 20), (Spoil::Bellmetal, 2), (Spoil::Wishmote, 1)],
        gift: Gift::Bombs,
        says: "A BOMB MORE EACH DELVE",
        most: 3,
    },
    // The showpiece: platinum statues, tables, doors and cases.
    Work {
        id: "platinum-hall",
        name: "The Platinum Hall",
        level: RoomKind::BomNobleHalls,
        sign: (70, 43, 3, 2),
        wants: &[(Spoil::Gold, 1200), (Spoil::Gem, 12), (Spoil::Wishmote, 3)],
        gift: Gift::Purse,
        says: "GOLD ON EVERY NEW FLOOR",
        most: 3,
    },
];

pub fn work(id: &str) -> Option<&'static Work> {
    WORKS.iter().find(|w| w.id == id)
}

/// The work a raise order is for, if it is one of the fort's.
pub fn ordered(plan: &str) -> Option<&'static Work> {
    plan.strip_prefix(PREFIX).and_then(work)
}

impl Work {
    /// What the level after `level` costs: level 0 is building it.
    pub fn price(&self, level: u8) -> Spoils {
        let mut price = Spoils::default();
        for &(spoil, n) in self.wants {
            price.add(spoil, n * (u32::from(level) + 1));
        }
        price
    }

    /// Its raise order's plan.
    pub fn plan(&self) -> String {
        format!("{PREFIX}{}", self.id)
    }

    /// The questline counters for this work (`questline::note`): its id
    /// with `-` as `_`.
    pub fn counter(&self, what: &str) -> String {
        format!("bom_{what}_{}", self.id.replace('-', "_"))
    }

    /// Whether arena point (`x`, `y`) stands on its sign.
    pub fn on_sign(&self, x: f32, y: f32) -> bool {
        let (c, r, w, h) = self.sign;
        let (tx, ty) = ((x / TILE_UNITS).floor() as i32, (y / TILE_UNITS).floor() as i32);
        tx >= c && tx < c + w && ty >= r && ty < r + h
    }
}

/// What the realm has built of the works: each work's level, by id. Only
/// known works, at levels 1 to their top, are kept.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize)]
pub struct Works {
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub built: BTreeMap<String, u8>,
}

impl<'de> Deserialize<'de> for Works {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        #[derive(Deserialize)]
        struct Saved {
            #[serde(default)]
            built: BTreeMap<String, u8>,
        }
        let Saved { mut built } = Saved::deserialize(deserializer)?;
        built.retain(|id, level| match work(id) {
            Some(w) => {
                *level = (*level).min(w.most);
                *level > 0
            }
            None => false,
        });
        Ok(Works { built })
    }
}

impl Works {
    pub fn is_empty(&self) -> bool {
        self.built.is_empty()
    }

    pub fn level(&self, id: &str) -> u8 {
        self.built.get(id).copied().unwrap_or(0)
    }

    /// The points of `gift` the works give: each work's level, at most
    /// `raisings::MOST_GIVEN` in all.
    pub fn points(&self, gift: Gift) -> u32 {
        WORKS
            .iter()
            .filter(|w| w.gift == gift)
            .map(|w| u32::from(self.level(w.id)))
            .sum::<u32>()
            .min(crate::rules::raisings::MOST_GIVEN)
    }
}

/// The realm pays for the works the knights ordered, in order, and says
/// what rose. Orders that are not the fort's are the raisings' and are left
/// alone. Runs only inside the home claim's settlement, whose result is
/// committed with the treasury before it is delivered: a replayed claim is
/// answered from that record and never pays again. Returns whether anything
/// changed.
pub fn settle(works: &mut Works, treasury: &mut Spoils, orders: &[Order]) -> (bool, Vec<String>) {
    let mut changed = false;
    let mut said = Vec::new();
    for order in orders {
        let Some(work) = ordered(&order.plan) else {
            continue;
        };
        let level = works.level(work.id);
        if level >= work.most {
            continue;
        }
        let price = work.price(level);
        if !treasury.covers(&price) {
            said.push(format!("{}: {}", work.name, treasury.shortfall(&price)));
            continue;
        }
        treasury.take(&price);
        changed = true;
        works.built.insert(work.id.to_owned(), level + 1);
        said.push(if level == 0 {
            format!("{} is built in Bomrekedan", work.name)
        } else {
            format!("{} of Bomrekedan stands at level {}", work.name, level + 1)
        });
    }
    (changed, said)
}

/// What a work's sign offers: the level standing, the next level's price,
/// and why E would do nothing (the sign and the key ask the same question).
pub struct Offer {
    pub work: &'static Work,
    pub level: u8,
    pub price: Spoils,
    pub refused: Option<String>,
}

impl Run {
    /// The works whose signs stand in the room the party is in.
    pub fn bom_works_here(&self) -> impl Iterator<Item = &'static Work> + use<> {
        let kind = self.room().kind;
        WORKS.iter().filter(move |w| w.level == kind)
    }

    /// Whether the run holds an order for `work` its claim has not settled.
    pub fn bom_work_pending(&self, work: &Work) -> bool {
        let plan = work.plan();
        self.raise_orders.iter().any(|o| o.plan == plan)
    }

    pub fn bom_work_offer(&self, work: &'static Work) -> Offer {
        let level = self.home.bomrekedan.level(work.id);
        let price = work.price(level.min(work.most.saturating_sub(1)));
        let refused = if level >= work.most {
            Some("AT ITS TOP LEVEL".to_owned())
        } else if self.bom_work_pending(work) {
            Some("ORDERED: PAID AT THE HOME CLAIM".to_owned())
        } else {
            let short = self.treasury.shortfall(&price);
            (!short.is_empty()).then(|| short.to_uppercase())
        };
        Offer { work, level, price, refused }
    }

    /// The works' signs, a tick: E held on one orders its next level. A
    /// knight at the sign of a work already built is counted for the
    /// questlines; one whose order is placed, too.
    pub(in crate::rules) fn tick_bom_works(&mut self, inputs: &BTreeMap<u32, Input>) {
        if self.bom_works_here().next().is_none() {
            return;
        }
        let kind = self.room().kind;
        let mut asked = Vec::new();
        let mut seen = Vec::new();
        for (&id, hero) in self.players.iter_mut() {
            let pressing = inputs.get(&id).copied().filter(|i| i.valid()).is_some_and(|i| i.interact);
            if !pressing {
                hero.buy_spent = false;
            }
            let awake = hero.hp > 0 && !hero.stone && !hero.absent;
            let on = WORKS
                .iter()
                .find(|w| w.level == kind && awake && w.on_sign(hero.x, hero.y));
            match on {
                Some(work) if pressing && !hero.buy_spent => {
                    hero.buying = hero.buying.saturating_add(1);
                    // The first tick of a press at a work already built: the
                    // knight has seen it standing, which a questline counts.
                    // Only the whole hold orders (and pays for) the next level.
                    if hero.buying == 1 && self.home.bomrekedan.level(work.id) > 0 {
                        seen.push((id, work));
                    }
                    if hero.buying >= home::BUY_HOLD {
                        hero.buying = 0;
                        hero.buy_spent = true;
                        asked.push((id, work));
                    }
                }
                // Let go, spent, or off every sign: the hold begins again,
                // so stepping from one sign to another orders nothing early.
                _ => hero.buying = 0,
            }
        }
        for (id, work) in seen {
            self.note_quest(id, &work.counter("built"), 1);
        }
        for (id, work) in asked {
            let offer = self.bom_work_offer(work);
            if offer.refused.is_some() {
                self.sounds.push("mana_empty");
                continue;
            }
            let by = self.players.get(&id).map_or_else(String::new, |h| h.name.clone());
            self.raise_orders.push(Order { plan: work.plan(), tx: 0, ty: 0, by });
            self.note_quest(id, &work.counter("work_ordered"), 1);
            self.sounds.push("play_card");
        }
    }
}

#[cfg(test)]
#[path = "../../../../tests/rules/bomrekedan__works__tests.rs"]
mod tests;
