//! Recruitment, wages, lessons, and the company's three Yard stations.
//!
//! Stock is a pure function of the saved delve counter and recruitment board.
//! Only a paid, revalidated `Shop::Squires` order changes the company (selection
//! is free). No run RNG, seat identity, carried gold, or new save fields.
use super::*;
use crate::rules::merchant::{Purchase, Shop};

/// Recruitment plates, west to east, in the Yard's open north aisle (tiles).
pub const RECRUIT_PLATES: [(i32, i32, i32, i32); 5] = [
    (2, 3, 3, 1),
    (6, 3, 3, 1),
    (10, 3, 3, 1),
    (14, 3, 3, 1),
    (18, 3, 3, 1),
];
/// Free selection and paid training, clear of Ector's south-west lecterns.
pub const SELECT_PLATE: (i32, i32, i32, i32) = (2, 8, 2, 1);
pub const TRAIN_PLATE: (i32, i32, i32, i32) = (5, 8, 2, 1);
/// Board, hall, training yard, west to east. All three have three upgrades.
pub const UPGRADE_PLATES: [(i32, i32, i32, i32); 3] = [(2, 9, 2, 1), (5, 9, 2, 1), (8, 9, 2, 1)];

const TRAIN: usize = 5;
const UPGRADE: usize = 6;
const NAMES: [&str; 20] = [
    "Ash", "Bramble", "Clover", "Dew", "Ember", "Fern", "Gale", "Hazel", "Ivy", "Juniper",
    "Kestrel", "Lark", "Moss", "Nettle", "Oak", "Pip", "Quill", "Reed", "Sorrel", "Thistle",
];

fn mix(mut n: u32) -> u32 {
    n ^= n >> 16;
    n = n.wrapping_mul(0x7feb352d);
    n ^= n >> 15;
    n = n.wrapping_mul(0x846ca68b);
    n ^ (n >> 16)
}

impl Company {
    /// The same applicants for the whole delve, shared by all knights. A
    /// knight may claim each once; other knights have independent claims.
    /// Offer IDs are stock keys, replaced by unique owned IDs when hired.
    pub fn offers(&self) -> Vec<Squire> {
        let board = self.board.min(3);
        (0..usize::from(2 + board))
            .map(|slot| {
                let id = self.delves.wrapping_mul(5).wrapping_add(slot as u32);
                let roll = mix(id.wrapping_add(0x51a1_7e));
                // More board rungs improve both the chance and ceiling of a
                // veteran applicant; level is the schema's measure of rarity.
                // Healers and alchemists begin as rare applicants (five percent
                // each). At board III every role has equal weight.
                let rare = 5 + 5 * u32::from(board);
                let kind = match (roll >> 8) % 100 {
                    n if n < rare => Kind::Healer,
                    n if n < 2 * rare => Kind::Alchemist,
                    _ => [Kind::Ninja, Kind::Ranger, Kind::Shield][roll as usize % 3],
                };
                let mut level = 1;
                for rung in 0..board {
                    if (roll.rotate_left(u32::from(rung) * 7) % 100) < 35 + u32::from(board) * 15 {
                        level += 1;
                    }
                }
                Squire {
                    id,
                    name: format!(
                        "{} {}",
                        NAMES[(roll as usize / 5) % NAMES.len()],
                        kind.name()
                    ),
                    kind,
                    level: level.min(self.level_cap()),
                    trained_on: None,
                }
            })
            .collect()
    }

    pub fn roster_cap(&self) -> usize {
        usize::from(2 + self.hall.min(3))
    }

    pub fn level_cap(&self) -> u8 {
        4 + 2 * self.yard.min(3)
    }

    /// Hiring wages, discounted ten percent per hall rung.
    pub fn hire_price(&self, slot: usize) -> Option<u32> {
        let offer = self.offers().into_iter().nth(slot)?;
        let wage = match offer.kind {
            Kind::Ninja => 90,
            Kind::Healer => 100,
            Kind::Ranger => 95,
            Kind::Shield => 110,
            Kind::Alchemist => 120,
        } * u32::from(offer.level);
        Some(wage * (100 - 10 * u32::from(self.hall.min(3))) / 100)
    }

    /// Training the named knight's selected companion. The yard cuts the
    /// wait from three delves to two, then one; it never allows repeat
    /// training in the same delve. A new recruit may train immediately.
    pub fn train_price(&self, name: &str) -> Option<u32> {
        let roster = self.rosters.get(name)?;
        let squire = roster
            .squires
            .iter()
            .find(|s| Some(s.id) == roster.active)?;
        if squire.level == 0 || squire.level >= self.level_cap() {
            return None;
        }
        if squire
            .trained_on
            .is_some_and(|d| self.delves < d || self.delves - d < u32::from(3 - self.yard.min(2)))
        {
            return None;
        }
        Some(60 * u32::from(squire.level).pow(2))
    }

    pub fn upgrade_price(&self, station: usize) -> Option<u32> {
        let level = *[self.board, self.hall, self.yard].get(station)?;
        (level < 3).then(|| [200, 450, 900][usize::from(level)])
    }

    fn price(&self, name: &str, action: usize) -> Option<u32> {
        match action {
            0..=4 => {
                let offer = self.offers().into_iter().nth(action)?;
                if let Some(roster) = self.rosters.get(name) {
                    if roster.squires.len() >= self.roster_cap()
                        || roster.claimed.contains(&offer.id)
                    {
                        return None;
                    }
                }
                // IDs must remain unique, including partially populated old saves.
                self.next_owned_id()?;
                self.hire_price(action)
            }
            TRAIN => self.train_price(name),
            UPGRADE..=8 => self.upgrade_price(action - UPGRADE),
            _ => None,
        }
    }

    fn next_owned_id(&self) -> Option<u32> {
        self.rosters
            .values()
            .flat_map(|r| r.squires.iter().map(|s| s.id))
            .fold(self.next_id, u32::max)
            .checked_add(1)
    }

    fn select(&mut self, name: &str) {
        let Some(roster) = self.rosters.get_mut(name) else {
            return;
        };
        roster.active = match roster
            .active
            .and_then(|id| roster.squires.iter().position(|s| s.id == id))
        {
            Some(i) => roster.squires.get(i + 1).map(|s| s.id),
            None => roster.squires.first().map(|s| s.id),
        };
    }

    fn buy(&mut self, name: &str, action: usize) {
        match action {
            0..=4 => {
                let mut squire = self.offers().swap_remove(action);
                let stock_id = squire.id;
                let stock: Vec<u32> = self.offers().iter().map(|s| s.id).collect();
                let Some(id) = self.next_owned_id() else {
                    return;
                };
                self.next_id = id;
                squire.id = id;
                let roster = self.rosters.entry(name.into()).or_default();
                if roster.squires.is_empty() {
                    roster.active = Some(id);
                }
                roster.squires.push(squire);
                roster.claimed.retain(|id| stock.contains(id));
                roster.claimed.push(stock_id);
            }
            TRAIN => {
                let roster = self.rosters.get_mut(name).unwrap();
                let squire = roster
                    .squires
                    .iter_mut()
                    .find(|s| Some(s.id) == roster.active)
                    .unwrap();
                squire.level += 1;
                squire.trained_on = Some(self.delves);
            }
            6 => self.board += 1,
            7 => self.hall += 1,
            8 => self.yard += 1,
            _ => unreachable!(),
        }
    }

    /// A deterministic, platform-independent quote tag in Purchase's existing
    /// offer word. Hash only state relevant to this purchase, so different
    /// knights can hire in the same payment batch. This is stale-quote
    /// detection, not authentication (purchases originate on the host).
    fn quote(&self, name: &str, action: usize) -> Option<usize> {
        let bytes = match action {
            0..=4 => serde_json::to_vec(&(
                name,
                self.board,
                self.hall,
                self.delves,
                self.rosters.get(name),
            )),
            TRAIN => serde_json::to_vec(&(name, self.yard, self.delves, self.rosters.get(name))),
            UPGRADE..=8 => serde_json::to_vec(&(
                name,
                self.delves,
                [self.board, self.hall, self.yard][action - UPGRADE],
            )),
            _ => return None,
        }
        .ok()?;
        let hash = bytes.into_iter().fold(0x811c9dc5u32, |h, b| {
            (h ^ u32::from(b)).wrapping_mul(0x01000193)
        });
        Some(((hash & 0x0fff_ffff) as usize) << 4 | action)
    }
}

fn inside((col, row, w, h): (i32, i32, i32, i32), hero: &Hero) -> bool {
    let (x, y) = (hero.x / TILE_UNITS, hero.y / TILE_UNITS);
    x >= col as f32 && x < (col + w) as f32 && y >= row as f32 && y < (row + h) as f32
}

fn plate(action: usize) -> Option<(i32, i32, i32, i32)> {
    match action {
        0..=4 => Some(RECRUIT_PLATES[action]),
        TRAIN => Some(TRAIN_PLATE),
        UPGRADE..=8 => Some(UPGRADE_PLATES[action - UPGRADE]),
        _ => None,
    }
}

// The driver prices its whole batch before delivering any goods. Mutually
// dependent orders must therefore be serialized at request time.
fn conflicting(a: usize, b: usize) -> bool {
    let affects = |upgrade, action| match upgrade {
        6 | 7 => action < TRAIN,
        8 => action == TRAIN,
        _ => false,
    };
    (a >= UPGRADE && a == b) || affects(a, b) || affects(b, a)
}

/// A customer/location/quote validated under the host's short run lock. The
/// captured name, not the seat's later occupant or position, owns the purchase.
#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PreparedPurchase {
    name: String,
    offer: usize,
}

impl PreparedPurchase {
    pub(crate) fn matches(&self, purchase: Purchase, name: &str) -> bool {
        purchase.shop == Shop::Squires && self.name == name && self.offer == purchase.offer
    }

    /// Recheck only deterministic company state as earlier sales are staged.
    pub fn price(&self, company: &Company) -> Option<u32> {
        let action = self.offer & 15;
        (company.quote(&self.name, action)? == self.offer)
            .then(|| company.price(&self.name, action))?
    }

    /// Apply before charging. An invalidated quote never buys or costs gold.
    pub fn apply(&self, company: &mut Company) -> Option<u32> {
        let gold = self.price(company)?;
        company.buy(&self.name, self.offer & 15);
        Some(gold)
    }
}

impl Run {
    /// Capture customer validation before reserving the company's mutations.
    /// No full Run (map, book, RNG, actors) is cloned for realm settlement.
    pub fn prepare_squire_purchase(&self, p: Purchase) -> Option<PreparedPurchase> {
        if p.shop != Shop::Squires {
            return None;
        }
        self.squire_price(p)?;
        Some(PreparedPurchase {
            name: self.players.get(&p.knight)?.name.clone(),
            offer: p.offer,
        })
    }

    /// Reserve free selection, orders and cadence while the host saves off-lock.
    pub fn reserve_squire_company(&mut self) -> Option<Company> {
        if self.squire_company_reserved || self.squire_company_claimed {
            return None;
        }
        self.squire_company_reserved = true;
        Some(self.home.squire_company.clone())
    }

    /// Deliver an already committed company without customer revalidation. On
    /// failure, keep the live company (including free selections) for retry.
    /// Cadence accumulated during either outcome belongs to the live run, not
    /// the older realm snapshot; the next settlement will persist it.
    pub fn complete_squire_company(&mut self, committed: Option<Company>) {
        debug_assert!(self.squire_company_reserved || self.squire_company_claimed);
        if let Some(company) = committed {
            self.home.squire_company = company;
        }
        let delves = std::mem::take(&mut self.squire_pending_delves);
        if delves > 0 {
            self.home.squire_company.delves = self.home.squire_company.delves.saturating_add(delves);
            for roster in self.home.squire_company.rosters.values_mut() {
                roster.claimed.clear();
            }
        }
        self.squire_company_reserved = false;
        self.squire_company_claimed = false;
    }

    fn company_customer(&self, id: u32) -> Option<&Hero> {
        let hero = self.players.get(&id)?;
        (self.dungeon.depth == 0
            && self.room().kind == RoomKind::SquireBarracks
            && hero.hp > 0
            && !hero.stone
            && hero.privy == 0
            && hero.soak == 0)
            .then_some(hero)
    }

    /// Make a payable order without granting anything or touching treasury.
    pub(in crate::rules) fn company_order(&self, id: u32, action: usize) -> Option<Purchase> {
        if self.squire_company_reserved || self.squire_company_claimed {
            return None;
        }
        let hero = self.company_customer(id)?;
        if !inside(plate(action)?, hero) {
            return None;
        }
        self.home.squire_company.price(&hero.name, action)?;
        Some(Purchase {
            knight: id,
            offer: self.home.squire_company.quote(&hero.name, action)?,
            shop: Shop::Squires,
        })
    }

    pub(in crate::rules) fn squire_price(&self, p: Purchase) -> Option<u32> {
        if p.shop != Shop::Squires {
            return None;
        }
        let action = p.offer & 15;
        if self.company_order(p.knight, action)? != p {
            return None;
        }
        self.home
            .squire_company
            .price(&self.players.get(&p.knight)?.name, action)
    }

    pub(in crate::rules) fn squire_purchase(&mut self, p: Purchase) {
        if self.squire_price(p).is_none() {
            return;
        }
        let name = self.players[&p.knight].name.clone();
        self.home.squire_company.buy(&name, p.offer & 15);
        self.sounds.push("card_pickup");
        self.found = Some((self.tick, p.knight, "THE COMPANY: PAID AND READY.".into()));
    }

    /// Yard integration hook: call once each surface tick with the same
    /// inputs as `tick_yard`. Uses the world's existing E latch, since the
    /// Yard and countryside room tick paths are mutually exclusive.
    pub fn tick_squire_company(&mut self, inputs: &BTreeMap<u32, Input>) {
        if self.squire_company_reserved || self.squire_company_claimed
            || self.dungeon.depth != 0
            || self.room().kind != RoomKind::SquireBarracks
        {
            return;
        }
        let held = self
            .players
            .keys()
            .filter(|id| inputs.get(id).is_some_and(|i| i.valid() && i.interact))
            .fold(0u8, |bits, &id| bits | 1 << (id & 7));
        let pressed = held & !self.rooms_held;
        self.rooms_held = held;
        let ids: Vec<_> = self.players.keys().copied().collect();
        for id in ids {
            if !(1..=MAX_PLAYERS).contains(&id)
                || pressed & (1 << (id & 7)) == 0
                || !inputs.get(&id).is_some_and(|i| i.valid() && i.interact)
            {
                continue;
            }
            let Some(hero) = self.company_customer(id) else {
                continue;
            };
            let name = hero.name.clone();
            let same_name_pending = self.peddled.iter().any(|p| {
                p.shop == Shop::Squires
                    && self.players.get(&p.knight).is_some_and(|h| h.name == name)
            });
            if same_name_pending {
                continue;
            }
            if inside(SELECT_PLATE, hero) {
                self.home.squire_company.select(&name);
                self.sounds.push("card_pickup");
                continue;
            }
            let Some(action) = (0..=8).find(|&a| plate(a).is_some_and(|p| inside(p, hero))) else {
                continue;
            };
            // Do not ask the driver's pre-priced batch to charge two orders
            // that would invalidate one another (same roster or same rung).
            if self
                .peddled
                .iter()
                .any(|p| p.shop == Shop::Squires && conflicting(action, p.offer & 15))
            {
                continue;
            }
            if let Some(p) = self.company_order(id, action) {
                self.peddled.push(p);
            }
        }
    }
}

#[cfg(test)]
#[path = "../../../tests/rules/together_squire_company__tests.rs"]
mod tests;
