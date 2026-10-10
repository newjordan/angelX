//! The Training Yard, north of the Undercroft, and the goblin who moved in.
//!
//! Three quintains stand in the yard: hit them with anything — a bow, a
//! blade, an ultimate — and each shows the damage it took in the last
//! second. They never fall and never hit back, and the blows still charge a
//! knight's ultimate, so the yard is where a knight tries one out.
//!
//! Once a loot goblin has slipped away with a sack of gold, he comes back:
//! Grubbins sets up a stall in the yard and sells the party three cards a
//! delve, for gold. He would like everyone to know that his stock is
//! legitimately acquired.

use super::home::{Order, Station};
use super::*;

/// Where the quintains stand, in tiles (their feet).
pub(crate) const QUINTAINS: [(f32, f32); 3] = [(6.5, 6.5), (12.0, 6.5), (17.5, 6.5)];
/// Grubbins' stall along the east wall, its three plates, and Grubbins.
pub(crate) const STALL: (i32, i32, i32, i32) = (17, 9, 6, 2);
pub(crate) const STALL_PLATES: [(i32, i32, i32, i32); 3] =
    [(17, 11, 2, 1), (19, 11, 2, 1), (21, 11, 2, 1)];
pub(crate) const GRUBBINS_AT: (f32, f32) = (20.0, 8.7);

/// What a card of each rarity costs at Grubbins' stall, in gold.
pub(crate) fn stall_price(card: &Card) -> u32 {
    match card.rarity {
        cards::Rarity::Common => 60,
        cards::Rarity::Rare => 150,
        cards::Rarity::Relic => 300,
    }
}

fn inside((col, row, w, h): (i32, i32, i32, i32), x: f32, y: f32) -> bool {
    let (c, r) = (x / TILE_UNITS, y / TILE_UNITS);
    c >= col as f32 && c < (col + w) as f32 && r >= row as f32 && r < (row + h) as f32
}

/// The stall's plate a point stands on, if any: 0, 1 or 2.
pub(crate) fn stall_plate(x: f32, y: f32) -> Option<usize> {
    STALL_PLATES.iter().position(|&p| inside(p, x, y))
}

impl Run {
    /// Put up the yard's quintains as the party walks in.
    pub(super) fn raise_quintains(&mut self) {
        for (c, r) in QUINTAINS {
            self.spawn_at(EnemyKind::Dummy, c * TILE_UNITS, r * TILE_UNITS);
            if let Some(dummy) = self.enemies.last_mut() {
                dummy.age = TELEGRAPH;
            }
        }
    }

    /// Grubbins' stock for this delve: three cards from the book, the
    /// rarer ones likelier, the same three for the whole delve.
    pub(super) fn stock_stall(&mut self) {
        if self.home.goblins == 0 {
            self.stall.clear();
            return;
        }
        let mut pool: Vec<(String, u32)> = self
            .book
            .cards
            .iter()
            .filter(|c| !c.is_spoil() && c.drop + c.chest > 0)
            .filter(|c| {
                matches!(
                    c.kind,
                    cards::Kind::Hold | cards::Kind::Play | cards::Kind::Arm
                )
            })
            .map(|c| {
                let weight = match c.rarity {
                    cards::Rarity::Common => 1,
                    cards::Rarity::Rare => 3,
                    cards::Rarity::Relic => 2,
                };
                (c.id.clone(), weight)
            })
            .collect();
        let mut rng = Rng::new(mix(self.seed ^ self.raid_id.wrapping_mul(0x5851_f42d)));
        self.stall.clear();
        for _ in 0..3 {
            if pool.is_empty() {
                break;
            }
            let total: u32 = pool.iter().map(|(_, w)| w).sum();
            let mut roll = rng.below(total as usize) as u32;
            let pick = pool
                .iter()
                .position(|(_, w)| {
                    if roll < *w {
                        true
                    } else {
                        roll -= w;
                        false
                    }
                })
                .unwrap_or(0);
            self.stall.push(pool.remove(pick).0);
        }
    }

    /// The yard each tick: quintains count what they took and stand up
    /// again; the stall's plates take orders.
    pub(super) fn tick_yard(&mut self, inputs: &BTreeMap<u32, Input>) {
        for dummy in self
            .enemies
            .iter_mut()
            .filter(|e| e.kind == EnemyKind::Dummy)
        {
            let took = dummy.max_hp.saturating_sub(dummy.hp);
            dummy.hp = dummy.max_hp;
            dummy.dir.0 += took as f32;
            dummy.timer += 1;
            if dummy.timer >= HZ {
                dummy.dir.1 = dummy.dir.0;
                dummy.dir.0 = 0.0;
                dummy.timer = 0;
            }
        }
        self.tick_lessons(inputs);
        if self.stall.is_empty() {
            return;
        }
        let mut asked = Vec::new();
        for (&id, hero) in self.players.iter_mut() {
            let fire = inputs
                .get(&id)
                .copied()
                .filter(|i| i.valid())
                .is_some_and(|i| i.fire);
            if !fire {
                hero.buy_spent = false;
            }
            let on = stall_plate(hero.x, hero.y).filter(|_| hero.hp > 0 && !hero.stone);
            match on {
                Some(item) if fire && !hero.buy_spent => {
                    hero.buying = hero.buying.saturating_add(1);
                    if hero.buying >= home::BUY_HOLD {
                        hero.buying = 0;
                        hero.buy_spent = true;
                        asked.push(Order {
                            knight: id,
                            station: [Station::StallA, Station::StallB, Station::StallC][item],
                        });
                    }
                }
                _ if on.is_some() => hero.buying = 0,
                _ => {}
            }
        }
        self.orders.extend(asked);
        // Grubbins greets whoever walks up to his stall.
        const GRUBBINS_BIT: u8 = 1 << 6;
        let (gx, gy) = (GRUBBINS_AT.0 * TILE_UNITS, GRUBBINS_AT.1 * TILE_UNITS);
        let near = self
            .players
            .values()
            .any(|h| h.hp > 0 && (h.x - gx).hypot(h.y - gy) < 5.0);
        if near && self.greeted & GRUBBINS_BIT == 0 {
            self.cues.push("npc:grubbins".into());
        }
        self.greeted = if near {
            self.greeted | GRUBBINS_BIT
        } else {
            self.greeted & !GRUBBINS_BIT
        };
    }

    /// The cockpit sold stall item `item` to knight `id`: it lands at their
    /// feet, and the stall's slot is empty.
    pub(crate) fn sell(&mut self, item: usize, id: u32) -> Option<String> {
        let card = self.stall.get(item).filter(|c| !c.is_empty())?.clone();
        self.stall[item] = String::new();
        let (x, y) = self.players.get(&id).map(|h| (h.x, h.y + 1.5))?;
        self.drop_item(card.clone(), x, y, None);
        self.sounds.push("card_pickup");
        self.cues.push("grubbins_sold".into());
        Some(card)
    }
}

#[cfg(test)]
#[path = "../../../../tests/cockpit/app/together_yard__tests.rs"]
mod tests;
