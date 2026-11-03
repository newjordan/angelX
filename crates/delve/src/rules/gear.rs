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
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub looks: BTreeMap<String, creator::Appearance>,
    #[serde(default, skip_serializing_if = "std::collections::BTreeSet::is_empty")]
    pub mirror_seen: std::collections::BTreeSet<String>,
    pub knights: BTreeMap<String, Kept>,
    pub legacy: bool,
    /// Painted ink only, kept per named knight in this realm.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub mirror_looks: BTreeMap<String, super::cosmetic_wish::Look>,
}
pub fn legacy_wardrobe() -> Wardrobe { Wardrobe { legacy: true, ..Default::default() } }

pub fn piece(id: &str) -> bool { matches!(id, "iron-helm" | "body-armour" | "kite-shield") }
/// Taken with E: arms, helm and mail. A guard of the knight's own kind is taken by walking over it (`manual_for`).
pub fn manual(card: &Card) -> bool { card.kind == cards::Kind::Arm || matches!(card.id.as_str(), "iron-helm" | "body-armour") }

/// Taken with E by this knight, never by walking over it: `manual`'s pieces,
/// and a guard of another kind than the one they play, so a knight on the
/// roll keeps it until they choose a shield, and a shield is never lost to a
/// roll card underfoot. 2026-10-22: "fix the bug where the shield is auto
/// picked up for a character with roll".
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
    /// A shield in hand: the guard is a shield, however it came (the kite
    /// shield found and worn, a shield card, a wish, the kit's own). The
    /// knight is drawn with it all the while it is held, raised or not, on
    /// every seat's screen. 2026-10-19: "if you grab a shield, we should
    /// always see the shield".
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
    }
    /// A friend takes knight 1: a host who plays no knight hands its place
    /// to the last friend in. The knight takes their name and their kept
    /// gear; the name it bore keeps its gear for when it comes back.
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
    }
    pub fn restore_gear(&mut self, id: u32) {
        self.restore_freedom_wishes(id);
        self.restore_look(id);
        self.restore_mirror_look(id);
        if let Some(hero) = self.players.get_mut(&id) { hero.mirror_wish = false; }
        // The identity start: a shape, whatever came home (`identity`).
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
        // A ranger starts on the roll, whatever guard came home with them: a
        // shield kept from the last delve is not in hand. 2026-10-19:
        // "rangers should have roll not shield on start".
        if classes::of(hero).is_some_and(|c| c.name == "Archer") {
            hero.guard = cards::Guard::Roll;
            hero.guard_card = Some("dodge-roll".into());
        }
        hero.dress();
    }
    pub(super) fn keep_gear(&mut self, id: u32) {
        // A shape's ball is no gear to keep: what was kept stays for a
        // delve begun with a kit.
        if let Some(hero) = self.players.get(&id).filter(|h| h.shape.is_none()) {
            self.home.wardrobe.knights.insert(hero.name.clone(), hero.kept());
        }
    }
    pub(super) fn keep_all_gear(&mut self) {
        for id in self.players.keys().copied().collect::<Vec<_>>() { self.keep_gear(id); }
    }

    /// Supplies follow missing equipment, never change the monsters' numbers.
    /// Floor one opens with a weapon cache; helm and mail follow early kills.
    /// Floor two's chest supplies any remaining pieces, including shields.
    /// A piece is shared loot (`loot::shared`): one lying serves every knight
    /// who lacks it, so one goes down for the party, not one a knight (the
    /// operator, 2026-10-19: "get rid of so many shield cards").
    pub(super) fn stock_gear_cache(&mut self) {
        let depth = self.dungeon.depth;
        // A shape wishes its weapon and its body: no cache waits for it.
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
        // An open map keeps the cache in its arrival chamber.
        let arrival = self.dungeon.open.as_ref().filter(|o| o.map == 0).map(|o| o.arrival);
        let room = &mut self.dungeon.rooms[0];
        let (cx, cy) = arrival.map_or((room.width() / 2.0, room.height() - 9.0), |(x, y)| (x, y - 6.0));
        room.chest = Some(layout::Chest { x: cx, y: cy, open: false });
        for (n, card) in cards.into_iter().enumerate() {
            room.items.push(Item { card: card.into(), x: cx - 4.5 + (n % 4) as f32 * 3.0, y: cy + 3.0 + (n / 4) as f32 * 2.5, held_off: None, took: Vec::new(), dropped: 0, owner: None });
        }
    }

    /// An early kill drops the first piece some knight lacks and has none
    /// lying for them to take: one serves them all (shared loot). An empty
    /// seat's knight (the host who plays none, a friend gone) is owed none:
    /// 2026-10-21, shields kept falling for the host's empty seat.
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


/// The pieces a knight is outfitted with, and whether one is owned.
const PIECES: [(&str, fn(&Owned) -> bool); 3] = [
    ("iron-helm", |o| o.helmet),
    ("body-armour", |o| o.body),
    ("kite-shield", |o| o.shield),
];

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
    pub fn dress(&mut self) {
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
