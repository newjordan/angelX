//! What a knight still wears. The helmet goes first, then the rest: a
//! knight hurt to two-fifths of their health is bare-headed, and at
//! fifteen in the hundred stands in their smallclothes, shield and all.
//! Mended back above a mark, the gear is on again at once. Every knight of
//! the company dresses the same way, the dwarf with her beard kept.

use super::*;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct Owned {
    pub helmet: bool,
    pub body: bool,
    pub shield: bool,
}
impl Owned {
    pub fn equipped() -> Self { Self { helmet: true, body: true, shield: true } }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Kept {
    pub owned: Owned,
    pub weapon: Weapon,
    pub arm: Option<String>,
    pub guard: Option<String>,
}
impl Kept {
    pub fn equipped() -> Self {
        Self { owned: Owned::equipped(), weapon: Weapon::Bow, arm: Some("bow".into()), guard: Some("dodge-roll".into()) }
    }
}
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct Wardrobe {
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub looks: BTreeMap<String, creator::Appearance>,
    #[serde(default, skip_serializing_if = "std::collections::BTreeSet::is_empty")]
    pub mirror_seen: std::collections::BTreeSet<String>,
    pub knights: BTreeMap<String, Kept>,
    pub legacy: bool,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub mirror_looks: BTreeMap<String, super::cosmetic_wish::Look>,
}
pub fn legacy_wardrobe() -> Wardrobe { Wardrobe { legacy: true, ..Default::default() } }

pub fn piece(id: &str) -> bool { matches!(id, "iron-helm" | "body-armour" | "kite-shield") }
pub fn manual(card: &Card) -> bool { card.kind == cards::Kind::Arm || matches!(card.id.as_str(), "iron-helm" | "body-armour") }

pub fn manual_for(hero: &Hero, card: &Card) -> bool {
    manual(card)
        || (card.kind == cards::Kind::Guard
            && std::mem::discriminant(&card.guard.unwrap_or_default()) != std::mem::discriminant(&hero.guard))
}

impl Hero {
    pub fn unarmed(&self) -> bool { self.weapon == Weapon::Fists && self.forged.is_none() }
    pub fn helmet_worn(&self) -> bool {
        self.owned.helmet && stage(self.hp, self.max_hp) == 0 && self.pit.bare().unwrap_or(0) == 0
    }
    pub fn holds_shield(&self) -> bool { self.guard == cards::Guard::Shield }
    pub(super) fn kept(&self) -> Kept {
        Kept { owned: self.owned, weapon: self.weapon, arm: self.arm.clone(), guard: self.guard_card.clone() }
    }
    pub(super) fn wear_piece(&mut self, id: &str) {
        match id {
            "iron-helm" => self.owned.helmet = true,
            "body-armour" => self.owned.body = true,
            "kite-shield" => self.owned.shield = true,
            _ => return,
        }
        self.dress();
    }
}

impl Run {
    pub fn rename_host(&mut self, name: &str) {
        self.players.get_mut(&1).unwrap().name = name.into();
        self.restore_gear(1);
        self.owe_rite(1);
    }
    pub fn seat_first(&mut self, name: &str) {
        let name = Hero::new(name, 0.0, 0.0).name;
        if self.players.get(&1).is_none_or(|h| h.name == name) {
            return;
        }
        self.keep_gear(1);
        self.retire_wish_seat(1);
        self.playtest.seats.remove(&1);
        if let Some(hero) = self.players.get_mut(&1) {
            hero.name = name;
        }
        self.restore_gear(1);
        self.stock_gear_cache();
        self.owe_rite(1);
    }
    pub fn restore_gear(&mut self, id: u32) {
        self.restore_freedom_wishes(id);
        self.restore_look(id);
        self.restore_mirror_look(id);
        if let Some(hero) = self.players.get_mut(&id) { hero.mirror_wish = false; }
        if self.identity {
            return self.unmake(id);
        }
        let Some(hero) = self.players.get_mut(&id) else { return; };
        if let Some(kept) = self.home.wardrobe.knights.get(&hero.name) {
            hero.owned = kept.owned;
            hero.weapon = kept.weapon;
            hero.arm = None;
            hero.forged = None;
            if let Some(card) = kept.arm.as_deref().and_then(|id| self.book.get(id)) { hero.equip(card); }
            hero.guard = cards::Guard::Roll;
            hero.guard_card = Some("dodge-roll".into());
            if let Some(card) = kept.guard.as_deref().and_then(|id| self.book.get(id)) {
                hero.guard = card.guard.unwrap_or_default();
                hero.guard_card = Some(card.id.clone());
            }
        } else if self.naked_start {
            hero.owned = Owned::default();
            hero.weapon = Weapon::Fists;
            hero.arm = None;
            hero.forged = None;
            hero.temper = 0;
            hero.bombs = 0;
            hero.hand.clear();
            hero.guard = cards::Guard::Roll;
            hero.guard_card = Some("dodge-roll".into());
        }
        if classes::of(hero).is_some_and(|c| c.name == "Archer") {
            hero.guard = cards::Guard::Roll;
            hero.guard_card = Some("dodge-roll".into());
        }
        hero.dress();
        self.restore_kingdom_gear(id);
    }
    pub(super) fn keep_gear(&mut self, id: u32) {
        if let Some(hero) = self.players.get(&id).filter(|h| h.shape.is_none()) {
            self.home.wardrobe.knights.insert(hero.name.clone(), hero.kept());
        }
    }
    pub(super) fn keep_all_gear(&mut self) {
        for id in self.players.keys().copied().collect::<Vec<_>>() { self.keep_gear(id); }
    }

    pub(super) fn stock_gear_cache(&mut self) {
        let depth = self.dungeon.depth;
        if !(1..=2).contains(&depth) || self.identity { return; }
        let mut cards = Vec::new();
        for hero in self.players.values() {
            if hero.unarmed() { cards.push(["bow", "crossbow", "handgonne"][cards.len() % 3]); }
        }
        if depth == 2 {
            for (card, owned) in PIECES {
                if self.players.values().any(|h| !owned(&h.owned)) { cards.push(card); }
            }
        }
        let mut reserved = BTreeMap::<String, usize>::new();
        for item in self.dungeon.rooms.iter().flat_map(|r| &r.items) {
            if self.book.get(&item.card).is_some_and(|c| c.kind == cards::Kind::Arm) {
                *reserved.entry("weapon".into()).or_default() += 1;
            } else { *reserved.entry(item.card.clone()).or_default() += 1; }
        }
        cards.retain(|card| {
            let key = if matches!(*card, "bow" | "crossbow" | "handgonne") { "weapon" } else { card };
            if let Some(n) = reserved.get_mut(key) && *n > 0 { *n -= 1; false } else { true }
        });
        if cards.is_empty() { return; }
        let arrival = self.dungeon.open.as_ref().filter(|o| o.map == 0).map(|o| o.arrival);
        let room = &mut self.dungeon.rooms[0];
        let (cx, cy) = arrival.map_or((room.width() / 2.0, room.height() - 9.0), |(x, y)| (x, y - 6.0));
        room.chest = Some(layout::Chest { x: cx, y: cy, open: false });
        for (n, card) in cards.into_iter().enumerate() {
            room.items.push(Item { card: card.into(), x: cx - 4.5 + (n % 4) as f32 * 3.0, y: cy + 3.0 + (n / 4) as f32 * 2.5, held_off: None, took: Vec::new(), dropped: 0, owner: None });
        }
    }

    pub(super) fn early_gear_drop(&mut self, x: f32, y: f32) {
        if !(1..=2).contains(&self.dungeon.depth) || self.identity { return; }
        for (card, owned) in PIECES {
            let waiting = |id: u32| self.dungeon.rooms.iter().flat_map(|r| &r.items).any(|i| i.card == card && i.seen_by(Some(id)));
            if self.players.iter().any(|(&id, h)| !h.absent && !owned(&h.owned) && !waiting(id)) {
                self.drop_item(card.into(), x, y, None);
                break;
            }
        }
    }
}

const PIECES: [(&str, fn(&Owned) -> bool); 3] = [
    ("iron-helm", |o| o.helmet),
    ("body-armour", |o| o.body),
    ("kite-shield", |o| o.shield),
];

pub const BARE_BODY: u32 = 20;
pub const BARE_HEAD: u32 = 10;

pub const HELM_OFF: u32 = 40;
pub const SMALLCLOTHES: u32 = 15;
pub const POP_TICKS: u32 = 24;

pub fn stage(hp: u32, max_hp: u32) -> u8 {
    if hp * 100 <= max_hp * SMALLCLOTHES {
        2
    } else if hp * 100 <= max_hp * HELM_OFF {
        1
    } else {
        0
    }
}

impl Hero {
    pub fn dress(&mut self) {
        self.pop = self.pop.saturating_sub(1);
        self.pit.tick();
        self.blessed = self.blessed.saturating_sub(1);
        if self.hp == 0 {
            return;
        }
        let health = stage(self.hp, self.max_hp).max(self.pit.bare().unwrap_or(0));
        let now = if !self.owned.body { 2 } else if !self.owned.helmet { health.max(1) } else { health };
        if now > self.gear {
            self.gear_was = self.gear;
            self.pop = POP_TICKS;
        } else if now < self.gear {
            self.pop = 0;
        }
        self.gear = now;
    }
}

#[cfg(test)]
#[path = "../../tests/rules/together_gear__tests.rs"]
mod tests;
