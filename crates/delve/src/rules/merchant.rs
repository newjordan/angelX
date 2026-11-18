//! A travelling peddler, wholly local to a run. Its private seed stream never
//! advances the dungeon/loot generator. Below, a knight pays from the gold
//! they carry; on the surface, where nothing is carried, the realm's
//! treasury pays (the cockpit charges it, then the goods are handed over).
//! Stock is bounded to three kinds of consumable.

use super::{Floor, HZ, MAX_PLAYERS, Rng, RoomKind, Run, TILE_UNITS, Tile, cards};
use crate::realm::{Spoil, Spoils};
use serde::{Deserialize, Serialize};

pub const STAND_TICKS: u32 = HZ;
pub const PLATES: [(i32, i32); 3] = [(17, 9), (19, 9), (21, 9)];
pub const CART_AT: (f32, f32) = (19.5, 7.5);
pub const COURTYARD_PLATES: [(i32, i32); 3] = [(2, 9), (4, 9), (6, 9)];
pub const COURTYARD_CART: (f32, f32) = (4.5, 12.6);

pub fn plates(kind: RoomKind) -> [(i32, i32); 3] {
    if kind == RoomKind::Gate { COURTYARD_PLATES } else { PLATES }
}

pub fn cart(kind: RoomKind) -> (f32, f32) {
    if kind == RoomKind::Gate { COURTYARD_CART } else { CART_AT }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Goods {
    Potion,
    Bomb,
    Scroll,
}

impl Goods {
    pub fn name(self) -> &'static str {
        match self {
            Self::Potion => "POTION",
            Self::Bomb => "BOMB",
            Self::Scroll => "THUNDER SCROLL",
        }
    }

    pub fn description(self) -> &'static str {
        match self {
            Self::Potion => "A HEALING CARD FOR YOUR HAND",
            Self::Bomb => "ONE BOMB FOR YOUR BELT",
            Self::Scroll => "A THUNDER CARD FOR YOUR HAND",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Offer {
    pub goods: Goods,
    pub gold: u32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct Purchase {
    pub knight: u32,
    pub offer: usize,
    pub shop: Shop,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum Shop {
    #[default]
    Peddler,
    Bath,
    Smith,
    Squires,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Camp {
    pub room: usize,
    pub name: &'static str,
    pub offers: Vec<Offer>,
}

pub fn camp(seed: u64, raid_id: u64, floor: &Floor) -> Option<Camp> {
    let mut rng = Rng::new(
        seed ^ raid_id.rotate_left(23)
            ^ u64::from(floor.depth).rotate_left(41)
            ^ 0x7065_6464_6c65_7201,
    );
    let suitable = |room: &super::Room| {
        let at = cart(room.kind);
        plates(room.kind).iter().all(|&(c, r)| room.tile(c, r) == Tile::Floor)
            && room.tile(at.0 as i32, at.1 as i32) == Tile::Floor
    };
    let room = if floor.depth == 0 {
        let preferred = if rng.chance(50) {
            RoomKind::Gate
        } else {
            RoomKind::MineHead
        };
        let other = if preferred == RoomKind::Gate {
            RoomKind::MineHead
        } else {
            RoomKind::Gate
        };
        [preferred, other].into_iter().find_map(|kind| {
            floor
                .rooms
                .iter()
                .position(|room| room.kind == kind && suitable(room))
        })?
    } else {
        floor
            .rooms
            .iter()
            .position(|room| room.kind == RoomKind::Treasure && suitable(room))
            .or_else(|| {
                floor
                    .rooms
                    .first()
                    .filter(|room| {
                        floor.depth >= 2 && room.kind == RoomKind::Sanctuary && suitable(room)
                    })
                    .map(|_| 0)
            })?
    };
    let name = ["MERRIN", "TANSY", "OSWIN"][rng.below(3)];
    let mut offers = vec![
        Offer {
            goods: Goods::Potion,
            gold: 18 + rng.below(5) as u32,
        },
        Offer {
            goods: Goods::Bomb,
            gold: 12 + rng.below(5) as u32,
        },
    ];
    if rng.chance(50) {
        offers.push(Offer {
            goods: Goods::Scroll,
            gold: 28 + rng.below(5) as u32,
        });
    }
    let last = rng.below(offers.len());
    offers.swap(0, last);
    Some(Camp { room, name, offers })
}

pub fn plate_at(kind: RoomKind, x: f32, y: f32, offers: usize) -> Option<usize> {
    if !x.is_finite() || !y.is_finite() {
        return None;
    }
    plates(kind)
        .iter()
        .take(offers.min(PLATES.len()))
        .position(|&(c, r)| {
            (c as f32..c as f32 + 1.0).contains(&(x / TILE_UNITS))
                && (r as f32..r as f32 + 1.0).contains(&(y / TILE_UNITS))
        })
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Sale {
    Bought,
    Owed,
    Full,
    Short,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Approach {
    pub offer: Option<usize>,
    pub ticks: u32,
    pub spent: bool,
    pub x: f32,
    pub y: f32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sale: Option<Sale>,
}

fn fresh_approaches() -> Vec<Approach> {
    vec![Approach::default(); MAX_PLAYERS as usize]
}

fn approaches_padded<'de, D: serde::Deserializer<'de>>(de: D) -> Result<Vec<Approach>, D::Error> {
    let mut seen = Vec::<Approach>::deserialize(de)?;
    if seen.len() < MAX_PLAYERS as usize {
        seen.resize(MAX_PLAYERS as usize, Approach::default());
    }
    Ok(seen)
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct State {
    #[serde(default = "fresh_approaches", deserialize_with = "approaches_padded")]
    pub approaches: Vec<Approach>,
}

impl Default for State {
    fn default() -> Self {
        Self { approaches: fresh_approaches() }
    }
}

impl State {
    pub fn valid(&self) -> bool {
        self.approaches.len() <= MAX_PLAYERS as usize
            && self.approaches.iter().all(|a| {
            a.offer.is_none_or(|i| i < PLATES.len())
                && a.ticks <= STAND_TICKS
                && a.x.is_finite()
                && a.y.is_finite()
        })
    }
}

impl Run {
    pub fn merchant_camp(&self) -> Option<Camp> {
        camp(self.seed, self.raid_id, &self.dungeon)
    }

    pub(super) fn tick_merchant(&mut self) {
        let Some(camp) = self.merchant_camp().filter(|c| c.room == self.at) else {
            self.merchant = State::default();
            return;
        };
        let unsafe_room = !self.room().cleared || self.enemies.iter().any(|e| e.hp > 0);
        let surface = self.dungeon.depth == 0;
        let kind = self.room().kind;
        for (slot, approach) in self.merchant.approaches.iter_mut().enumerate() {
            let id = slot as u32 + 1;
            let Some(hero) = self.players.get_mut(&id) else {
                *approach = Approach::default();
                continue;
            };
            let on = plate_at(kind, hero.x, hero.y, camp.offers.len());
            let Some(index) = on else {
                *approach = Approach::default();
                continue;
            };
            if approach.spent {
                continue;
            }
            if unsafe_room
                || hero.hp == 0
                || hero.stone
                || hero.privy > 0
                || hero.tossed > 0
                || hero.frog()
                || hero.aiming.is_some()
            {
                approach.ticks = 0;
                continue;
            }
            let moved = (hero.x - approach.x).hypot(hero.y - approach.y) > 0.01;
            if approach.offer != Some(index) || moved {
                approach.offer = Some(index);
                approach.ticks = 0;
            }
            (approach.x, approach.y) = (hero.x, hero.y);
            approach.ticks = (approach.ticks + 1).min(STAND_TICKS);
            if approach.ticks < STAND_TICKS {
                continue;
            }
            approach.spent = true;
            let offer = camp.offers[index];
            let full = match offer.goods {
                Goods::Bomb => hero.bombs >= hero.bomb_limit(),
                Goods::Potion | Goods::Scroll => hero.hand.len() >= cards::HAND,
            };
            let have = if surface {
                self.treasury.get(Spoil::Gold)
            } else {
                hero.carried.get(Spoil::Gold)
            };
            approach.sale = Some(if full {
                Sale::Full
            } else if have < offer.gold {
                Sale::Short
            } else if surface {
                self.peddled.push(Purchase { knight: id, offer: index, shop: Shop::Peddler });
                Sale::Owed
            } else {
                let mut price = Spoils::default();
                price.add(Spoil::Gold, offer.gold);
                hero.carried.take(&price);
                match offer.goods {
                    Goods::Bomb => hero.give_bombs(1),
                    Goods::Potion => hero.hand.push("potion".into()),
                    Goods::Scroll => hero.hand.push("thunder-scroll".into()),
                }
                Sale::Bought
            });
        }
    }

    pub fn peddle_price(&self, p: Purchase) -> Option<u32> {
        if p.shop == Shop::Squires {
            return self.squire_price(p);
        }
        if p.shop != Shop::Peddler {
            return self.town_price(p);
        }
        let camp = self.merchant_camp().filter(|c| c.room == self.at)?;
        let offer = *camp.offers.get(p.offer)?;
        let hero = self.players.get(&p.knight)?;
        let full = match offer.goods {
            Goods::Bomb => hero.bombs >= hero.bomb_limit(),
            Goods::Potion | Goods::Scroll => hero.hand.len() >= cards::HAND,
        };
        (!full).then_some(offer.gold)
    }

    pub fn peddle(&mut self, p: Purchase) {
        if p.shop == Shop::Squires {
            self.squire_purchase(p);
            return;
        }
        if p.shop != Shop::Peddler {
            return self.town_paid(p);
        }
        let Some(camp) = self.merchant_camp() else {
            return;
        };
        let Some(offer) = camp.offers.get(p.offer).copied() else {
            return;
        };
        let card = match offer.goods {
            Goods::Potion => Some("potion".into()),
            Goods::Scroll => Some("thunder-scroll".into()),
            Goods::Bomb => None,
        };
        self.deliver_peddled(p, offer.goods, card, true);
    }
    pub(super) fn deliver_peddled(
        &mut self,
        p: Purchase,
        goods: Goods,
        card: Option<String>,
        update_board: bool,
    ) -> bool {
        let Some(hero) = self.players.get_mut(&p.knight) else {
            return false;
        };
        match goods {
            Goods::Bomb if hero.bombs < hero.bomb_limit() => hero.give_bombs(1),
            Goods::Potion | Goods::Scroll if hero.hand.len() < cards::HAND => {
                let Some(card) = card else {
                    return false;
                };
                hero.hand.push(card);
            }
            _ => return false,
        }
        let seat = (p.knight as usize).checked_sub(1);
        if update_board
            && let Some(approach) = seat.and_then(|s| self.merchant.approaches.get_mut(s))
            && approach.spent
            && approach.offer == Some(p.offer)
        {
            approach.sale = Some(Sale::Bought);
        }
        true
    }
}

#[cfg(test)]
mod merge_tests {
    use super::*;

    #[test]
    fn merchant_journal_delivery_respects_bomb_belt_and_card_binding() {
        let mut run = Run::new(4, 1, Some("Bree"));
        let purchase = Purchase { knight: 1, offer: 0, shop: Shop::Peddler };
        let hero = run.players.get_mut(&1).unwrap();
        hero.bomb_capacity = 6;
        hero.bombs = 5;
        assert!(run.deliver_peddled(purchase, Goods::Bomb, None, false));
        assert_eq!(run.players[&1].bombs, 6);
        assert!(!run.deliver_peddled(purchase, Goods::Bomb, None, false));
        assert_eq!(run.players[&1].bombs, 6);
        assert!(run.deliver_peddled(purchase, Goods::Potion, Some("bound-potion".into()), false));
        assert_eq!(run.players[&1].hand.last().map(String::as_str), Some("bound-potion"));
    }
}
