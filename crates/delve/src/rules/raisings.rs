//! The kingdom's raisings: after a delve the party spends its haul on the
//! surface, and what it raises goes back down with them. A knight walking the
//! kingdom presses the next-card key to take up the surveyor's rod: each press
//! is the next plan, and past the last one the rod is put away. The ghost of
//! the plan stands where they stand; E held raises it there, or a level on
//! the same plan in reach. Spoils and wishmotes come from the realm's one
//! treasury, by the same claim every other purchase goes through
//! (`home_settlement`).
//!
//! The realm keeps them in `Home::raised`, and each stands on the map as a
//! built wish on the lot its knight chose (`standing`), so the map's walls,
//! drawing and doors need to know nothing of them. Every plan does one thing
//! for the delve (`Gift`), and does more for a higher level and for its friend
//! standing beside it (`strength`).

use super::map;
use super::*;
use crate::realm::{Status, Wish};
use crate::world::wishes::{self, Lot};

pub const MOST_RAISED: usize = 16;
pub const TOP_LEVEL: u8 = 3;
pub const BESIDE: i32 = 12;
pub const REACH: i32 = 4;
pub const PURSE: u32 = 40;
pub const WARD: u32 = HZ * 2;
pub const MOST_GIVEN: u32 = 3;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Gift {
    Wishes,
    Purse,
    Bombs,
    Mail,
    Ward,
}

pub struct Plan {
    pub id: &'static str,
    pub name: &'static str,
    pub gift: Gift,
    pub says: &'static str,
    pub wants: &'static [(Spoil, u32)],
    pub friend: &'static str,
    source: &'static str,
}

pub const PLANS: [Plan; 5] = [
    Plan {
        id: "wayside-shrine",
        name: "Wayside Shrine",
        gift: Gift::Wishes,
        says: "A WISH MORE EACH DELVE",
        wants: &[(Spoil::Gold, 120), (Spoil::Bone, 12), (Spoil::Wax, 8), (Spoil::Wishmote, 3)],
        friend: "beacon-tower",
        source: include_str!("../../../../cockpit/assets/realm/wishes/wayside-shrine.wish"),
    },
    Plan {
        id: "tavern",
        name: "The Tavern",
        gift: Gift::Purse,
        says: "GOLD ON EVERY NEW FLOOR",
        wants: &[(Spoil::Gold, 950), (Spoil::Bone, 10), (Spoil::Wax, 15), (Spoil::Wishmote, 2)],
        friend: "miners-lodge",
        source: include_str!("../../../../cockpit/assets/realm/wishes/tavern.wish"),
    },
    Plan {
        id: "miners-lodge",
        name: "Miners' Lodge",
        gift: Gift::Bombs,
        says: "A BOMB MORE EACH DELVE",
        wants: &[(Spoil::Gold, 450), (Spoil::Ore, 30), (Spoil::Gem, 6), (Spoil::Wishmote, 2)],
        friend: "tavern",
        source: include_str!("../../../../cockpit/assets/realm/wishes/miners-lodge.wish"),
    },
    Plan {
        id: "fellowship-stone",
        name: "Fellowship Stone",
        gift: Gift::Mail,
        says: "MAIL MORE EACH DELVE",
        wants: &[(Spoil::Gold, 50), (Spoil::Bond, 6), (Spoil::Wishmote, 2)],
        friend: "wayside-shrine",
        source: include_str!("../../../../cockpit/assets/realm/wishes/fellowship-stone.wish"),
    },
    Plan {
        id: "beacon-tower",
        name: "Beacon Tower",
        gift: Gift::Ward,
        says: "WARDED ON EVERY ARRIVAL",
        wants: &[(Spoil::Gold, 200), (Spoil::Ore, 10), (Spoil::Ember, 8), (Spoil::Wishmote, 3)],
        friend: "fellowship-stone",
        source: include_str!("../../../../cockpit/assets/realm/wishes/beacon-tower.wish"),
    },
];

pub fn plan(id: &str) -> Option<&'static Plan> {
    PLANS.iter().find(|p| p.id == id)
}

impl Plan {
    pub fn price(&self, level: u8) -> Spoils {
        let mut price = Spoils::default();
        for &(spoil, n) in self.wants {
            price.add(spoil, n * (u32::from(level) + 1));
        }
        price
    }

    fn art(&self) -> Vec<String> {
        self.source
            .lines()
            .skip_while(|line| *line != "art")
            .skip(1)
            .take_while(|line| !line.is_empty())
            .map(str::to_owned)
            .collect()
    }

    fn wish(&self, by: &str, tile: (i32, i32), raised: u32) -> Wish {
        Wish {
            id: format!("raised-{}-{}-{}", self.id, tile.0, tile.1),
            name: self.name.to_owned(),
            words: self.says.to_lowercase(),
            by: by.to_owned(),
            status: Status::Built,
            price: Spoils::default(),
            art: self.art(),
            near: String::new(),
            paid: Vec::new(),
            raised,
            lot: Some(tile),
        }
    }

    pub fn lot(&self, tile: (i32, i32)) -> Lot {
        let (tw, th) = wishes::footprint(&self.wish("", tile, 0));
        Lot { tx: tile.0, ty: tile.1, tw, th }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Raised {
    pub plan: String,
    pub tx: i32,
    pub ty: i32,
    pub level: u8,
    pub by: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Order {
    pub plan: String,
    pub tx: i32,
    pub ty: i32,
    pub by: String,
}

pub fn standing(raised: &[Raised]) -> Vec<Wish> {
    raised
        .iter()
        .enumerate()
        .filter_map(|(i, r)| Some(plan(&r.plan)?.wish(&r.by, (r.tx, r.ty), 1 << 20 | i as u32)))
        .collect()
}

pub fn strength(raised: &[Raised], i: usize) -> u32 {
    let Some(here) = raised.get(i) else {
        return 0;
    };
    let friend = plan(&here.plan).is_some_and(|p| {
        raised.iter().any(|r| {
            r.plan == p.friend && (r.tx - here.tx).abs() <= BESIDE && (r.ty - here.ty).abs() <= BESIDE
        })
    });
    u32::from(here.level) + u32::from(friend)
}

pub fn settle(raised: &mut Vec<Raised>, treasury: &mut Spoils, orders: &[Order]) -> (bool, Vec<String>) {
    let mut changed = false;
    let mut said = Vec::new();
    for order in orders {
        if order.plan.starts_with(super::world::bomrekedan::works::PREFIX) {
            continue;
        }
        let Some(plan) = plan(&order.plan) else {
            continue;
        };
        let tile = (order.tx, order.ty);
        let there = raised.iter().position(|r| r.plan == order.plan && (r.tx, r.ty) == tile);
        let level = there.map_or(0, |i| raised[i].level);
        let lot = plan.lot(tile);
        let crowded = there.is_none()
            && (raised.len() >= MOST_RAISED
                || raised
                    .iter()
                    .filter_map(|r| Some(self::plan(&r.plan)?.lot((r.tx, r.ty))))
                    .any(|other| other.overlaps(&lot)));
        if level >= TOP_LEVEL || crowded {
            continue;
        }
        let price = plan.price(level);
        if !treasury.covers(&price) {
            said.push(format!("{}: {}", plan.name, treasury.shortfall(&price)));
            continue;
        }
        treasury.take(&price);
        changed = true;
        match there {
            Some(i) => {
                raised[i].level += 1;
                said.push(format!("{} stands at level {}", plan.name, raised[i].level));
            }
            None => {
                raised.push(Raised {
                    plan: order.plan.clone(),
                    tx: order.tx,
                    ty: order.ty,
                    level: 1,
                    by: order.by.clone(),
                });
                said.push(format!("{} rises in the kingdom", plan.name));
            }
        }
    }
    (changed, said)
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Survey {
    pub pick: u8,
    pub hold: u32,
}

pub struct Target {
    pub plan: &'static Plan,
    pub lot: Lot,
    pub level: u8,
    pub price: Spoils,
    pub refused: Option<String>,
}

impl Run {
    pub fn survey_target(&self, id: u32) -> Option<Target> {
        let plan = PLANS.get(usize::from(self.kingdom.survey.get(&id)?.pick))?;
        let hero = self.players.get(&id)?;
        let (hx, hy) = ((hero.x / TILE_UNITS) as i32, (hero.y / TILE_UNITS) as i32);
        let near = self
            .home
            .raised
            .iter()
            .filter(|r| r.plan == plan.id)
            .map(|r| (r, plan.lot((r.tx, r.ty))))
            .map(|(r, lot)| (r, lot, (lot.tx + lot.tw / 2 - hx).abs().max((lot.ty + lot.th - hy).abs())))
            .filter(|&(_, _, far)| far <= REACH)
            .min_by_key(|&(_, _, far)| far);
        if let Some((raised, lot, _)) = near {
            let top = raised.level >= TOP_LEVEL;
            let price = plan.price(raised.level.min(TOP_LEVEL - 1));
            let refused = if top {
                Some("AT ITS TOP LEVEL".to_owned())
            } else {
                self.short_of(&price)
            };
            return Some(Target { plan, lot, level: raised.level + 1, price, refused });
        }
        let (tw, th) = (plan.lot((0, 0)).tw, plan.lot((0, 0)).th);
        let lot = plan.lot((hx - tw / 2, hy - th));
        let price = plan.price(0);
        let refused = if self.home.raised.len() >= MOST_RAISED {
            Some("THE REALM IS FULL".to_owned())
        } else if !self.ground_for(lot) {
            Some("NO GROUND HERE".to_owned())
        } else {
            self.short_of(&price)
        };
        Some(Target { plan, lot, level: 1, price, refused })
    }

    fn short_of(&self, price: &Spoils) -> Option<String> {
        let short = self.treasury.shortfall(price);
        (!short.is_empty()).then(|| short.to_uppercase())
    }

    fn ground_for(&self, lot: Lot) -> bool {
        let geo = map::geography(&self.kingdom.wishes);
        wishes::open_for(&self.kingdom.wishes, lot)
            && !crate::world::kingdom::OPEN_PLOT.overlaps(&lot)
            && geo.sites.iter().all(|s| {
                !lot.contains(s.stand.0, s.stand.1) && s.lot.is_none_or(|l| !l.overlaps(&lot))
            })
    }

    pub(super) fn tick_survey(&mut self, inputs: &BTreeMap<u32, Input>) {
        let ids: Vec<u32> = self.players.keys().copied().collect();
        for id in ids {
            let input = inputs.get(&id).copied().filter(|i| i.valid()).unwrap_or_default();
            let fresh = if input.cycle {
                self.kingdom.survey_held.insert(id)
            } else {
                self.kingdom.survey_held.remove(&id);
                false
            };
            let awake = self.players.get(&id).is_some_and(|h| h.hp > 0 && !h.stone);
            if fresh && awake {
                match self.kingdom.survey.get_mut(&id) {
                    None => {
                        self.kingdom.survey.insert(id, Survey::default());
                    }
                    Some(survey) if usize::from(survey.pick) + 1 < PLANS.len() => {
                        *survey = Survey { pick: survey.pick + 1, hold: 0 };
                    }
                    Some(_) => {
                        self.kingdom.survey.remove(&id);
                    }
                }
            }
            let latched = self.kingdom.door_latched.contains(&id);
            let Some(survey) = self.kingdom.survey.get_mut(&id) else {
                continue;
            };
            if !input.interact || latched || !awake {
                survey.hold = 0;
                continue;
            }
            survey.hold += 1;
            if survey.hold < home::BUY_HOLD {
                continue;
            }
            survey.hold = 0;
            self.kingdom.door_latched.insert(id);
            let Some(target) = self.survey_target(id) else {
                continue;
            };
            if target.refused.is_some() {
                self.sounds.push("mana_empty");
                continue;
            }
            let by = self.players.get(&id).map_or_else(String::new, |h| h.name.clone());
            self.raise_orders.push(Order {
                plan: target.plan.id.to_owned(),
                tx: target.lot.tx,
                ty: target.lot.ty,
                by,
            });
            self.sounds.push("play_card");
        }
        let players = &self.players;
        self.kingdom.survey.retain(|id, _| players.contains_key(id));
        self.kingdom.survey_held.retain(|id| players.contains_key(id));
    }

    pub(super) fn raised_gifts(&mut self, first: bool) {
        let raised = &self.home.raised;
        let works = &self.home.bomrekedan;
        let points = |gift: Gift| -> u32 {
            (0..raised.len())
                .filter(|&i| plan(&raised[i].plan).is_some_and(|p| p.gift == gift))
                .map(|i| strength(raised, i))
                .sum::<u32>()
                + works.points(gift)
        };
        let (wishes, purse) = (points(Gift::Wishes), points(Gift::Purse));
        let (bombs, mail, ward) = (points(Gift::Bombs), points(Gift::Mail), points(Gift::Ward));
        for hero in self.players.values_mut().filter(|h| !h.absent) {
            if first {
                let more = wishes.min(MOST_GIVEN) as u8;
                hero.wishes = hero.wishes.saturating_add(more).min(overclass::MOST_WISHES);
                hero.give_bombs(bombs.min(MOST_GIVEN));
                hero.armor += mail.min(MOST_GIVEN);
            } else {
                hero.carried.add(Spoil::Gold, purse * PURSE);
            }
            hero.invulnerable = hero.invulnerable.max(ward * WARD);
        }
    }
}

#[cfg(test)]
#[path = "../../tests/rules/raisings__tests.rs"]
mod tests;
