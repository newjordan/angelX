//! The bazaar: the room through the Sozzled Griffin's east door, six stalls
//! of odd vendors selling the curios (`curios`). What each stall holds is
//! drawn from the run's seed and raid, never from the run's dice, so every
//! seat agrees and walking out and back changes nothing.
//!
//! Three wares a stall, one fence's ware in the tavern, nineteen curios of
//! the thirty-three, each at its price in gold with a little haggling in it
//! (a share either way, fixed by the same seed). A knight stands before a
//! ware and holds E; it is theirs if the gold is in their purse and their
//! pockets have room ([`curios::POCKETS`]). A knight may hold one of each
//! curio, and the stalls never run out for the next knight.

use super::alehouse::{Spot, Toy};
use super::curios::{self, CURIOS, POCKETS};
use super::*;

/// The bazaar's size in tiles (a great hall), and its cell.
pub const HALL: (usize, usize) = (32, 18);

/// The doors: west to the tavern, south to the kingdom.
pub const TAVERN_DOOR: (i32, i32, i32, i32) = (1, 8, 1, 3);
pub const GATE: (i32, i32, i32, i32) = (15, 16, 2, 1);
/// Where the party stands coming in from the tavern (tiles).
pub const ARRIVAL: (f32, f32) = (6.0, 9.4);

/// A stall: its counter, where its vendor stands (behind it), what the
/// vendor is, and where a knight stands before each of its three wares.
pub struct Stall {
    pub counter: (i32, i32, i32, i32),
    pub vendor: (f32, f32),
    pub who: &'static str,
    pub stands: [(f32, f32); 3],
    /// The awning's colours (two inks).
    pub awning: (char, char),
}

pub const WARES: usize = 3;
pub const STALLS: usize = 6;
/// Slots: the stalls' wares in order, then the fence's.
pub const FENCE_SLOT: u8 = (STALLS * WARES) as u8;

const fn north(c: i32, who: &'static str, awning: (char, char)) -> Stall {
    Stall {
        counter: (c, 3, 6, 2),
        vendor: (c as f32 + 3.0, 2.8),
        who,
        stands: [(c as f32 + 1.0, 5.9), (c as f32 + 3.0, 5.9), (c as f32 + 5.0, 5.9)],
        awning,
    }
}

const fn south(c: i32, who: &'static str, awning: (char, char)) -> Stall {
    Stall {
        counter: (c, 12, 6, 2),
        vendor: (c as f32 + 3.0, 11.3),
        who,
        stands: [(c as f32 + 1.0, 14.9), (c as f32 + 3.0, 14.9), (c as f32 + 5.0, 14.9)],
        awning,
    }
}

pub const STALL: [Stall; STALLS] = [
    north(2, "A GNOME WHO ONLY SELLS LEFT BOOTS", ('T', 'R')),
    north(9, "A GOAT IN A HAT (CASHIER)", ('T', 'u')),
    north(16, "A WIZARD WHO IS JUST A BARREL", ('O', 'q')),
    north(23, "A PILGRIM SELLING SOMEONE ELSE'S RELICS", ('T', 'R')),
    south(5, "TWO CHILDREN IN A COAT, POSING AS A MERCHANT", ('O', 'F')),
    south(21, "A VERY CONFIDENT MOLE", ('T', 'u')),
];

/// The central well, the complaints box and the fishbowl.
pub const WELL: (i32, i32, i32, i32) = (14, 8, 4, 3);
pub const BACK_WALL: (i32, i32, i32, i32) = (1, 1, 30, 1);
pub const COMPLAINTS_AT: (f32, f32) = (3.4, 12.9);
pub const FISHBOWL_AT: (f32, f32) = (27.6, 8.6);
pub const COMPLAINTS: (i32, i32, i32, i32) = (1, 12, 1, 2);
pub const FISHBOWL: (i32, i32, i32, i32) = (29, 8, 2, 2);

/// One ware on a stall.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Offer {
    pub slot: u8,
    /// Which of `CURIOS`.
    pub curio: u8,
    pub price: u32,
}

impl Offer {
    pub fn curio(self) -> &'static curios::Curio {
        &CURIOS[usize::from(self.curio)]
    }
}

/// The stand of ware `slot` (tiles).
pub fn stand_of(slot: u8) -> Option<(f32, f32)> {
    let (stall, ware) = (usize::from(slot) / WARES, usize::from(slot) % WARES);
    STALL.get(stall).map(|s| s.stands[ware])
}

/// What the stalls and the fence hold this run: nineteen different curios,
/// in slot order. The same for every seat, every time.
pub fn stock(seed: u64, raid_id: u64) -> Vec<Offer> {
    let mut rng = Rng::new(mix(seed ^ raid_id.rotate_left(29) ^ 0x0ba2_aa2b_a2aa));
    let mut deck: Vec<u8> = (0..CURIOS.len() as u8).collect();
    for i in (1..deck.len()).rev() {
        deck.swap(i, rng.below(i + 1));
    }
    (0..=usize::from(FENCE_SLOT))
        .map(|slot| {
            let curio = deck[slot];
            let base = CURIOS[usize::from(curio)].price;
            // A little haggling in it: a tenth either way, to the nearest five.
            let share = 90 + rng.below(21) as u32;
            let mut price = (base * share / 100 + 2) / 5 * 5;
            if slot == usize::from(FENCE_SLOT) {
                // What fell off the back of a cart.
                price = (base * 65 / 100 + 2) / 5 * 5;
            }
            Offer { slot: slot as u8, curio, price: price.max(5) }
        })
        .collect()
}

/// The fence's ware.
pub fn fence(seed: u64, raid_id: u64) -> (u8, Offer) {
    let offer = stock(seed, raid_id)[usize::from(FENCE_SLOT)];
    (FENCE_SLOT, offer)
}

/// The thing a knight at `(x, y)` (arena units) would reach in the bazaar.
pub fn spot_near(x: f32, y: f32) -> Option<Spot> {
    let mut best: Option<(f32, Spot)> = None;
    let mut see = |spot: Spot, (sx, sy): (f32, f32)| {
        let d = (x - sx * TILE_UNITS).hypot(y - sy * TILE_UNITS);
        let reach = if matches!(spot, Spot::Ware(_)) { 2.2 } else { super::alehouse::REACH };
        if d < reach && best.is_none_or(|(b, _)| d < b) {
            best = Some((d, spot));
        }
    };
    for slot in 0..FENCE_SLOT {
        if let Some(at) = stand_of(slot) {
            see(Spot::Ware(slot), at);
        }
    }
    see(Spot::Toy(Toy::Complaints), COMPLAINTS_AT);
    see(Spot::Toy(Toy::Fishbowl), (FISHBOWL_AT.0, FISHBOWL_AT.1 + 1.6));
    best.map(|(_, spot)| spot)
}

fn fill(room: &mut Room, (c, r, w, h): (i32, i32, i32, i32), tile: Tile) {
    for y in r..r + h {
        for x in c..c + w {
            room.set(x as usize, y as usize, tile);
        }
    }
}

/// The bazaar laid out: counters, the well, the boxes, and the doors' plates.
pub fn room(cell: (i32, i32)) -> Room {
    let mut r = Room::solid_rock(cell);
    r.kind = RoomKind::Bazaar;
    r.regrow(HALL.0, HALL.1);
    fill(&mut r, BACK_WALL, Tile::Block);
    for s in &STALL {
        fill(&mut r, s.counter, Tile::Block);
    }
    for rect in [WELL, COMPLAINTS, FISHBOWL] {
        fill(&mut r, rect, Tile::Block);
    }
    for e in super::alehouse::DOORS.iter().filter(|e| e.room == RoomKind::Bazaar) {
        fill(&mut r, e.at, Tile::Stairs);
    }
    r
}

impl Run {
    /// The stock of this run's stalls.
    pub fn bazaar_stock(&self) -> Vec<Offer> {
        stock(self.seed, self.raid_id)
    }

    /// The fence's ware in the tavern.
    pub fn fence_offer(&self) -> Offer {
        fence(self.seed, self.raid_id).1
    }

    /// What ware `slot` costs knight `id` now: its price, less their
    /// coupon, never nothing.
    pub fn ware_price(&self, id: u32, slot: u8) -> Option<u32> {
        let offer = self.bazaar_stock().get(usize::from(slot)).copied()?;
        let off = self.players.get(&id)?.curio_coupon();
        Some((offer.price * (100 - off) / 100).max(1))
    }

    /// Knight `id` holds E before ware `slot`: it is theirs if they can pay
    /// and carry it.
    pub fn buy_curio(&mut self, id: u32, slot: u8) {
        let Some(offer) = self.bazaar_stock().get(usize::from(slot)).copied() else {
            return;
        };
        let curio = offer.curio();
        let Some(price) = self.ware_price(id, slot) else {
            return;
        };
        let book = &self.book;
        let Some(hero) = self.players.get_mut(&id) else {
            return;
        };
        let name = curio.name.to_uppercase();
        let (text, good) = if hero.pockets.has(curio.id) {
            (format!("YOU HAVE THE {name} ALREADY. THEY DO NOT STACK."), false)
        } else if hero.pockets.items.len() >= POCKETS {
            (format!("YOUR POCKETS ARE FULL ({POCKETS}). SOMETHING HAS TO GO. NOTHING CAN."), false)
        } else if hero.carried.get(Spoil::Gold) < price {
            (format!("NEEDS {} MORE GOLD.", price - hero.carried.get(Spoil::Gold)), false)
        } else {
            hero.spend_gold(price);
            hero.pockets.items.push(curio.id.to_string());
            hero.rebonus(book);
            (format!("{name} IS YOURS FOR {price} GOLD. + {}. - {}.", curio.boon, curio.bane), true)
        };
        if good {
            self.sounds.push("play_card");
            *self.marks.entry("curio_bought".into()).or_default() += 1;
            self.call_sale(id, slot, curio.name);
        }
        self.say(id, text, good);
    }
}
