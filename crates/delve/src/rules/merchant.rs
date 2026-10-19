//! A travelling peddler, wholly local to a run. Its private seed stream never
//! advances the dungeon/loot generator. Below, a knight pays from the gold
//! they carry; on the surface, where nothing is carried, the realm's
//! treasury pays (the cockpit charges it, then the goods are handed over).
//! Stock is bounded to three kinds of consumable.

use super::{Floor, HZ, MAX_BOMBS, MAX_PLAYERS, Rng, RoomKind, Run, TILE_UNITS, Tile, cards};
use crate::realm::{Spoil, Spoils};
use serde::{Deserialize, Serialize};

pub const STAND_TICKS: u32 = HZ;
pub const PLATES: [(i32, i32); 3] = [(17, 9), (19, 9), (21, 9)];
pub const CART_AT: (f32, f32) = (19.5, 7.5);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
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

/// A purchase at the surface peddler (or a town counter), for the cockpit
/// to pay from the realm's treasury: which knight, at which plate.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Purchase {
    pub knight: u32,
    pub offer: usize,
    pub shop: Shop,
}

/// Who is selling: the peddler's cart, or a counter in the town (`town`).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Shop {
    #[default]
    Peddler,
    Bath,
    Smith,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Camp {
    pub room: usize,
    pub name: &'static str,
    pub offers: Vec<Offer>,
}

/// One camp per floor: either safe surface room, or a treasure room below.
/// A deep floor without treasure may use its initial sanctuary. This keeps
/// carried-gold purchases usable during a delve, before spoils are banked.
/// Walking away or loading a checkpoint cannot reroll the camp.
/// Do not depend on the driver's additional surface room kinds.
pub fn camp(seed: u64, raid_id: u64, floor: &Floor) -> Option<Camp> {
    let mut rng = Rng::new(
        seed ^ raid_id.rotate_left(23)
            ^ u64::from(floor.depth).rotate_left(41)
            ^ 0x7065_6464_6c65_7201,
    );
    let suitable = |room: &super::Room| {
        PLATES.iter().all(|&(c, r)| room.tile(c, r) == Tile::Floor)
            && room.tile(CART_AT.0 as i32, CART_AT.1 as i32) == Tile::Floor
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
        // Never select a fight room just because it has been cleared, nor a
        // boss hall that becomes a sanctuary later: the route cannot reroll.
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

pub fn plate_at(x: f32, y: f32, offers: usize) -> Option<usize> {
    if !x.is_finite() || !y.is_finite() {
        return None;
    }
    PLATES
        .iter()
        .take(offers.min(PLATES.len()))
        .position(|&(c, r)| {
            (c as f32..c as f32 + 1.0).contains(&(x / TILE_UNITS))
                && (r as f32..r as f32 + 1.0).contains(&(y / TILE_UNITS))
        })
}

/// Fixed-size approach memory, separate from station/input-control holds.
/// Kept in checkpoints so even a save made on a plate cannot charge again.
#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Approach {
    pub offer: Option<usize>,
    pub ticks: u32,
    pub spent: bool,
    pub x: f32,
    pub y: f32,
}

/// A fresh memory for each seat: `MAX_PLAYERS` of them.
fn fresh_approaches() -> Vec<Approach> {
    vec![Approach::default(); MAX_PLAYERS as usize]
}

/// Reads the seats' memories from a save made when the party was smaller
/// (this was once a fixed array four wide, and a bump of the party's size
/// must never turn an old save into "invalid length"): the seats the save
/// never had get fresh memories.
fn approaches_padded<'de, D: serde::Deserializer<'de>>(de: D) -> Result<Vec<Approach>, D::Error> {
    let mut seen = Vec::<Approach>::deserialize(de)?;
    if seen.len() < MAX_PLAYERS as usize {
        seen.resize(MAX_PLAYERS as usize, Approach::default());
    }
    Ok(seen)
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct State {
    /// By seat, from the first (a knight's number less one). A `Vec`, not an
    /// array, so saves of any party size load: see `approaches_padded`.
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
        // A treasure chest may have awakened a mimic. Trading pauses while
        // the room is unsafe, without moving the cart or changing its offers.
        let unsafe_room = !self.room().cleared || self.enemies.iter().any(|e| e.hp > 0);
        let surface = self.dungeon.depth == 0;
        // IDs are bounded by Run's admission. Iterate by ID, not player arrival,
        // so simultaneous approaches have deterministic outcomes as well.
        for (slot, approach) in self.merchant.approaches.iter_mut().enumerate() {
            let id = slot as u32 + 1;
            let Some(hero) = self.players.get_mut(&id) else {
                *approach = Approach::default();
                continue;
            };
            let on = plate_at(hero.x, hero.y, camp.offers.len());
            let Some(index) = on else {
                *approach = Approach::default();
                continue;
            };
            // A spent approach remains latched even if the knight changes to
            // another plate without leaving the buying area.
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
                // Threats or incapacity interrupt the dwell, not a purchase
                // latch. Only actually stepping away can rearm a spent visit.
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
            // Success AND refusal consume this approach. No repeated charges,
            // retries or message spam until the knight steps off all plates.
            approach.spent = true;
            let offer = camp.offers[index];
            let full = match offer.goods {
                Goods::Bomb => hero.bombs >= MAX_BOMBS,
                Goods::Potion | Goods::Scroll => hero.hand.len() >= cards::HAND,
            };
            let have = if surface {
                self.treasury.get(Spoil::Gold)
            } else {
                hero.carried.get(Spoil::Gold)
            };
            let message = if full {
                format!(
                    "{}: {} full; no gold spent. Step away to try again.",
                    camp.name,
                    if offer.goods == Goods::Bomb {
                        "bomb belt"
                    } else {
                        "card hand"
                    }
                )
            } else if have < offer.gold {
                format!(
                    "{}: {} costs {} {} gold; {} {}. Step away to try again.",
                    camp.name,
                    offer.goods.name(),
                    offer.gold,
                    if surface { "treasury" } else { "carried" },
                    if surface { "the treasury holds" } else { "you have" },
                    have
                )
            } else if surface {
                self.peddled.push(Purchase { knight: id, offer: index, shop: Shop::Peddler });
                format!(
                    "{}: {} for {} treasury gold, once the realm pays.",
                    camp.name,
                    offer.goods.name(),
                    offer.gold
                )
            } else {
                let mut price = Spoils::default();
                price.add(Spoil::Gold, offer.gold);
                hero.carried.take(&price);
                match offer.goods {
                    Goods::Bomb => hero.bombs += 1,
                    Goods::Potion => hero.hand.push("potion".into()),
                    Goods::Scroll => hero.hand.push("thunder-scroll".into()),
                }
                format!(
                    "{}: bought {} for {} carried gold. Step away before buying again.",
                    camp.name,
                    offer.goods.name(),
                    offer.gold
                )
            };
            self.found = Some((self.tick, id, message));
        }
    }

    /// What surface purchase `p` costs, while the peddler still stands here
    /// and the knight still has room for it.
    pub fn peddle_price(&self, p: Purchase) -> Option<u32> {
        if p.shop != Shop::Peddler {
            return self.town_price(p);
        }
        let camp = self.merchant_camp().filter(|c| c.room == self.at)?;
        let offer = *camp.offers.get(p.offer)?;
        let hero = self.players.get(&p.knight)?;
        let full = match offer.goods {
            Goods::Bomb => hero.bombs >= MAX_BOMBS,
            Goods::Potion | Goods::Scroll => hero.hand.len() >= cards::HAND,
        };
        (!full).then_some(offer.gold)
    }

    /// The cockpit paid for surface purchase `p` from the realm's treasury:
    /// the knight gets the goods.
    pub fn peddle(&mut self, p: Purchase) {
        if p.shop != Shop::Peddler {
            return self.town_paid(p);
        }
        let Some(camp) = self.merchant_camp() else {
            return;
        };
        let Some(offer) = camp.offers.get(p.offer).copied() else {
            return;
        };
        let Some(hero) = self.players.get_mut(&p.knight) else {
            return;
        };
        match offer.goods {
            Goods::Bomb => hero.bombs = (hero.bombs + 1).min(MAX_BOMBS),
            Goods::Potion => hero.hand.push("potion".into()),
            Goods::Scroll => hero.hand.push("thunder-scroll".into()),
        }
        self.found = Some((
            self.tick,
            p.knight,
            format!(
                "{}: bought {} for {} treasury gold. Step away before buying again.",
                camp.name,
                offer.goods.name(),
                offer.gold
            ),
        ));
    }
}
