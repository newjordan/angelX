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

pub const PREFIX: &str = "bom:";

pub struct Work {
    pub id: &'static str,
    pub name: &'static str,
    pub level: RoomKind,
    pub sign: (i32, i32, i32, i32),
    pub wants: &'static [(Spoil, u32)],
    pub gift: Gift,
    pub says: &'static str,
    pub most: u8,
}

pub const WORKS: [Work; 6] = [
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
    Work {
        id: "still",
        name: "The Stills",
        level: RoomKind::BomFarms,
        sign: (66, 51, 3, 2),
        wants: &[(Spoil::Gold, 120), (Spoil::Wax, 4)],
        gift: Gift::Purse,
        says: "GOLD ON EVERY NEW FLOOR",
        most: 3,
    },
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

pub fn ordered(plan: &str) -> Option<&'static Work> {
    plan.strip_prefix(PREFIX).and_then(work)
}

impl Work {
    pub fn price(&self, level: u8) -> Spoils {
        let mut price = Spoils::default();
        for &(spoil, n) in self.wants {
            price.add(spoil, n * (u32::from(level) + 1));
        }
        price
    }

    pub fn plan(&self) -> String {
        format!("{PREFIX}{}", self.id)
    }

    pub fn counter(&self, what: &str) -> String {
        format!("bom_{what}_{}", self.id.replace('-', "_"))
    }

    pub fn on_sign(&self, x: f32, y: f32) -> bool {
        let (c, r, w, h) = self.sign;
        let (tx, ty) = ((x / TILE_UNITS).floor() as i32, (y / TILE_UNITS).floor() as i32);
        tx >= c && tx < c + w && ty >= r && ty < r + h
    }
}

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

    pub fn points(&self, gift: Gift) -> u32 {
        WORKS
            .iter()
            .filter(|w| w.gift == gift)
            .map(|w| u32::from(self.level(w.id)))
            .sum::<u32>()
            .min(crate::rules::raisings::MOST_GIVEN)
    }
}

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

pub struct Offer {
    pub work: &'static Work,
    pub level: u8,
    pub price: Spoils,
    pub refused: Option<String>,
}

impl Run {
    pub fn bom_works_here(&self) -> impl Iterator<Item = &'static Work> + use<> {
        let kind = self.room().kind;
        WORKS.iter().filter(move |w| w.level == kind)
    }

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
                    if hero.buying == 1 && self.home.bomrekedan.level(work.id) > 0 {
                        seen.push((id, work));
                    }
                    if hero.buying >= home::BUY_HOLD {
                        hero.buying = 0;
                        hero.buy_spent = true;
                        asked.push((id, work));
                    }
                }
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
