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

/// What has safely reached home, keyed by the player's name in the host realm.
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
    pub knights: BTreeMap<String, Kept>,
    pub legacy: bool,
}
pub fn legacy_wardrobe() -> Wardrobe { Wardrobe { legacy: true, ..Default::default() } }

pub fn piece(id: &str) -> bool { matches!(id, "iron-helm" | "body-armour" | "kite-shield") }
/// Taken with E: arms, helm and mail. A shield is a guard and is taken by walking over it, as ever.
pub fn manual(card: &Card) -> bool { card.kind == cards::Kind::Arm || matches!(card.id.as_str(), "iron-helm" | "body-armour") }

impl Hero {
    pub fn unarmed(&self) -> bool { self.weapon == Weapon::Fists && self.forged.is_none() }
    pub fn helmet_worn(&self) -> bool {
        self.owned.helmet && stage(self.hp, self.max_hp) == 0 && self.pit.bare().unwrap_or(0) == 0
    }
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
    }
    pub fn restore_gear(&mut self, id: u32) {
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
        hero.dress();
    }
    pub(super) fn keep_gear(&mut self, id: u32) {
        if let Some(hero) = self.players.get(&id) {
            self.home.wardrobe.knights.insert(hero.name.clone(), hero.kept());
        }
    }
    pub(super) fn keep_all_gear(&mut self) {
        for id in self.players.keys().copied().collect::<Vec<_>>() { self.keep_gear(id); }
    }

    /// Supplies follow missing equipment, never change the monsters' numbers.
    /// Floor one opens with a weapon cache; helm and mail follow early kills.
    /// Floor two's chest supplies any remaining pieces, including shields.
    pub(super) fn stock_gear_cache(&mut self) {
        let depth = self.dungeon.depth;
        if !(1..=2).contains(&depth) { return; }
        let mut cards = Vec::new();
        for hero in self.players.values() {
            if hero.unarmed() { cards.push(["bow", "crossbow", "handgonne"][cards.len() % 3]); }
            if depth == 2 {
                if !hero.owned.helmet { cards.push("iron-helm"); }
                if !hero.owned.body { cards.push("body-armour"); }
                if !hero.owned.shield { cards.push("kite-shield"); }
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
        // An open map keeps the cache in its arrival chamber.
        let arrival = self.dungeon.open.as_ref().filter(|o| o.map == 0).map(|o| o.arrival);
        let room = &mut self.dungeon.rooms[0];
        let (cx, cy) = arrival.map_or((room.width() / 2.0, room.height() - 9.0), |(x, y)| (x, y - 6.0));
        room.chest = Some(layout::Chest { x: cx, y: cy, open: false });
        for (n, card) in cards.into_iter().enumerate() {
            room.items.push(Item { card: card.into(), x: cx - 4.5 + (n % 4) as f32 * 3.0, y: cy + 3.0 + (n / 4) as f32 * 2.5, held_off: None, took: Vec::new() });
        }
    }

    pub(super) fn early_gear_drop(&mut self, x: f32, y: f32) {
        if !(1..=2).contains(&self.dungeon.depth) { return; }
        for (card, missing) in [
            ("iron-helm", self.players.values().filter(|h| !h.owned.helmet).count()),
            ("body-armour", self.players.values().filter(|h| !h.owned.body).count()),
            ("kite-shield", self.players.values().filter(|h| !h.owned.shield).count()),
        ] {
            let waiting = self.dungeon.rooms.iter().flat_map(|r| &r.items).filter(|i| i.card == card).count();
            if waiting < missing {
                self.drop_item(card.into(), x, y, None);
                break;
            }
        }
    }
}


/// Extra share of every hit, in percent, for a knight without body armour
/// or a helmet. A shield guards in its own way (`Guard::Shield`).
pub const BARE_BODY: u32 = 20;
pub const BARE_HEAD: u32 = 10;

/// The helmet comes off at this share of health, in percent, or below.
pub const HELM_OFF: u32 = 40;
/// The armour and clothes come off at this share, or below.
pub const SMALLCLOTHES: u32 = 15;
/// Ticks the flung helmet (and armour) are in the air.
pub const POP_TICKS: u32 = 24;

/// 0 whole, 1 bare-headed, 2 in smallclothes.
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
    /// Once a tick: dress or undress to the health the knight has, and
    /// start the pop when something has just come off.
    pub(super) fn dress(&mut self) {
        self.pop = self.pop.saturating_sub(1);
        self.pit.tick();
        if self.hp == 0 {
            return;
        }
        // Mid pit stop the straps decide, if they have come off further.
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
