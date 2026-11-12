//! Named equipment made by the kingdom's workshops. Finished stock is issued
//! once to a persistent player profile; a bound item can then be equipped
//! without manufacturing another pickup or resetting a combat cooldown.
use super::*;
use std::collections::BTreeSet;
use std::sync::OnceLock;

pub const MAX_PROFILES: usize = 128;

fn valid_profile(key: &str) -> bool {
    !key.is_empty() && key.len() <= 256 && key == key.to_lowercase()
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Slot {
    Arm,
    Guard,
    Spell,
}

pub struct Gear {
    pub id: &'static str,
    pub name: &'static str,
    pub slot: Slot,
    pub station: &'static str,
    pub role: &'static str,
    pub controls: &'static str,
    pub raw: &'static str,
}

pub const ITEMS: &[Gear] = &[
    Gear {
        id: "iron-oathblade",
        name: "Iron Oathblade",
        slot: Slot::Arm,
        station: "forge",
        role: "28 damage / 3.4 reach / 155 degree sweep / 0.53s",
        controls: "F: broad close sweep",
        raw: include_str!("../../../../cockpit/assets/kingdom/items/iron-oathblade.card"),
    },
    Gear {
        id: "ember-edge",
        name: "Ember Edge",
        slot: Slot::Arm,
        station: "forge",
        role: "28 melee / 18 returning blade / 10 throw range",
        controls: "F: sweep nearby; throw out and back at range",
        raw: include_str!("../../../../cockpit/assets/kingdom/items/ember-edge.card"),
    },
    Gear {
        id: "mooncrystal-staff",
        name: "Mooncrystal Staff",
        slot: Slot::Arm,
        station: "enchanter",
        role: "3 x 12 damage shards / 24 degree fan / 0.6s",
        controls: "F: aim and fire the three-shard fan",
        raw: include_str!("../../../../cockpit/assets/kingdom/items/mooncrystal-staff.card"),
    },
    Gear {
        id: "hearth-wardstone",
        name: "Hearth Wardstone",
        slot: Slot::Spell,
        station: "enchanter",
        role: "Heal 18 / ward 2s / reusable after 18s",
        controls: "Its spell key 1-4: heal and briefly ward",
        raw: include_str!("../../../../cockpit/assets/kingdom/items/hearth-wardstone.card"),
    },
    Gear {
        id: "prospectors-pick",
        name: "Prospectors Pick",
        slot: Slot::Arm,
        station: "forge",
        role: "44 damage / 2.8 reach / narrow 55 degree strike / 0.8s",
        controls: "F: aim the narrow, heavy strike",
        raw: include_str!("../../../../cockpit/assets/kingdom/items/prospectors-pick.card"),
    },
    Gear {
        id: "runebound-aegis",
        name: "Runebound Aegis",
        slot: Slot::Guard,
        station: "forge",
        role: "4-wide barrier / 25 mana per second / allies +15%",
        controls: "Hold guard: stop enemy shots and empower allied shots",
        raw: include_str!("../../../../cockpit/assets/kingdom/items/runebound-aegis.card"),
    },
];

pub fn item(id: &str) -> Option<&'static Gear> {
    ITEMS.iter().find(|g| g.id == id)
}

pub fn card(id: &str) -> Option<&'static Card> {
    static CARDS: OnceLock<Vec<Card>> = OnceLock::new();
    CARDS
        .get_or_init(|| {
            ITEMS
                .iter()
                .map(|g| {
                    cards::check(g.id, g.raw)
                        .unwrap_or_else(|e| panic!("{}: {e:?}", g.id))
                        .card
                })
                .collect()
        })
        .iter()
        .find(|c| c.id == id)
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct Loadout {
    pub owned: BTreeSet<String>,
    pub arm: Option<String>,
    pub guard: Option<String>,
    pub spell: Option<String>,
}
impl Loadout {
    fn selected(&self, slot: Slot) -> Option<&str> {
        match slot {
            Slot::Arm => self.arm.as_deref(),
            Slot::Guard => self.guard.as_deref(),
            Slot::Spell => self.spell.as_deref(),
        }
    }
    fn select(&mut self, gear: &Gear) {
        *match gear.slot {
            Slot::Arm => &mut self.arm,
            Slot::Guard => &mut self.guard,
            Slot::Spell => &mut self.spell,
        } = Some(gear.id.into());
    }
    pub fn valid(&self) -> bool {
        self.owned.len() <= ITEMS.len()
            && self.owned.iter().all(|id| item(id).is_some())
            && [Slot::Arm, Slot::Guard, Slot::Spell]
                .into_iter()
                .all(|slot| {
                    self.selected(slot).is_none_or(|id| {
                        self.owned.contains(id) && item(id).is_some_and(|g| g.slot == slot)
                    })
                })
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct Ledger {
    pub rev: u64,
    pub profiles: BTreeMap<String, Loadout>,
}
impl Ledger {
    pub fn is_empty(&self) -> bool {
        self.rev == 0 && self.profiles.is_empty()
    }
    pub fn valid(&self) -> bool {
        self.rev < u64::MAX
            && self.profiles.len() <= MAX_PROFILES
            && self
                .profiles
                .iter()
                .all(|(key, loadout)| valid_profile(key) && loadout.valid())
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Status {
    pub owned: bool,
    pub equipped: bool,
    pub selected: bool,
    pub available: u32,
    pub action: &'static str,
    pub reason: Option<String>,
}

fn equipped(hero: &Hero, gear: &Gear) -> bool {
    match gear.slot {
        Slot::Arm => hero.arm.as_deref() == Some(gear.id),
        Slot::Guard => hero.guard_card.as_deref() == Some(gear.id),
        Slot::Spell => hero.spells.iter().flatten().any(|id| id == gear.id),
    }
}

fn can_equip(hero: &Hero, gear: &Gear) -> Result<(), String> {
    if gear.slot == Slot::Spell {
        if weave::weaves(hero) {
            return Err(
                "The Mage weaves its own spells; choose another class for this relic".into(),
            );
        }
        if hero.possessed.is_some() {
            return Err("Leave the borrowed body before equipping a spell relic".into());
        }
        if !equipped(hero, gear)
            && !(0..cards::SPELL_SLOTS)
                .any(|i| hero.spells[i].is_none() && hero.module_spells[i].is_none())
        {
            return Err("Free a spell slot before issuing this relic".into());
        }
    }
    Ok(())
}

fn wear(hero: &mut Hero, gear: &Gear, definition: &Card) {
    if equipped(hero, gear) {
        return;
    }
    match gear.slot {
        Slot::Arm => hero.equip(definition),
        Slot::Guard => {
            hero.guard = definition.guard.expect("checked guard definition");
            hero.guard_card = Some(gear.id.into());
            hero.shielding = false;
            hero.walling = false;
        }
        Slot::Spell => {
            if let Some(i) = (0..cards::SPELL_SLOTS)
                .find(|&i| hero.spells[i].is_none() && hero.module_spells[i].is_none())
            {
                hero.spells[i] = Some(gear.id.into());
                // A previously occupied slot may still carry its cooldown.
                // Issuing gear never erases that elapsed-time obligation.
                if let Some(bar) = hero.bar.iter_mut().find(|s| s.is_none()) {
                    *bar = Some(cards::BarSlot::Spell(gear.id.into()));
                }
            }
        }
    }
}

// Seats can change owner without reconstructing the Hero. A previous player's
// bound gear must never travel with that seat into a new player's wardrobe.
fn reclaim_unowned(hero: &mut Hero, loadout: Option<&Loadout>) {
    let unowned = |id: &str| item(id).is_some() && !loadout.is_some_and(|l| l.owned.contains(id));
    if hero.arm.as_deref().is_some_and(unowned) {
        hero.arm = None;
        hero.forged = None;
        hero.temper = 0;
    }
    if hero.guard_card.as_deref().is_some_and(unowned) {
        hero.guard_card = Some("dodge-roll".into());
        hero.guard = cards::Guard::Roll;
        hero.shielding = false;
        hero.walling = false;
    }
    for spell in &mut hero.spells {
        if spell.as_deref().is_some_and(unowned) {
            *spell = None;
        }
    }
    for slot in &mut hero.bar {
        if matches!(slot, Some(cards::BarSlot::Spell(id)) if unowned(id)) {
            *slot = None;
        }
    }
}

impl Run {
    pub fn kingdom_gear_status(&self, player: u32, id: &str) -> Option<Status> {
        let gear = item(id)?;
        let hero = self.players.get(&player)?;
        let profile = talents::profile_key(hero);
        let loadout = self.home.kingdom_gear.profiles.get(&profile);
        let owned = loadout.is_some_and(|l| l.owned.contains(id));
        let available = self.home.industry.amount(id);
        let equipped = equipped(hero, gear);
        let reason = if !valid_profile(&profile) {
            Some("Choose a valid player name before collecting equipment".into())
        } else if self.dungeon.depth != 0 || self.room().kind != RoomKind::Kingdom {
            Some("Visit the kingdom armory to change equipment".into())
        } else if hero.hp == 0 || hero.absent || hero.stone {
            Some("A living, present knight must collect the equipment".into())
        } else if !owned && available == 0 {
            Some(format!(
                "Produce {} in the {} and deliver it to camp",
                gear.name, gear.station
            ))
        } else {
            can_equip(hero, gear).err()
        };
        Some(Status {
            owned,
            equipped,
            available,
            selected: loadout.is_some_and(|l| l.selected(gear.slot) == Some(id)),
            action: if equipped && owned {
                "EQUIPPED"
            } else if owned {
                "EQUIP"
            } else {
                "ISSUE & EQUIP"
            },
            reason,
        })
    }

    /// One atomic armory action: check capacity and the target slot, reserve one
    /// finished item if unowned, then record ownership and equip. Failure leaves
    /// stock, catalogue, hero and ledger unchanged. No floor card is minted.
    pub fn kingdom_gear_action(&mut self, player: u32, id: &str) -> Result<String, String> {
        let gear = item(id).ok_or("Unknown kingdom equipment")?;
        let status = self
            .kingdom_gear_status(player, id)
            .ok_or("Knight not present")?;
        if let Some(reason) = status.reason {
            return Err(reason);
        }
        if !self.home.kingdom_gear.valid() {
            return Err("Equipment ledger is invalid".into());
        }
        let profile = talents::profile_key(&self.players[&player]);
        if !self.home.kingdom_gear.profiles.contains_key(&profile)
            && self.home.kingdom_gear.profiles.len() >= MAX_PROFILES
        {
            return Err("The armory player ledger is full".into());
        }
        if self.book.get(id).is_none() && self.book.cards.len() >= cards::MAX_BOOK {
            return Err("The card book is full".into());
        }
        let definition = card(id).expect("registered kingdom equipment");
        let ledger_changed = !status.owned || !status.selected;
        if ledger_changed && self.home.kingdom_gear.rev >= u64::MAX - 1 {
            return Err("Equipment revision limit reached".into());
        }
        if !status.owned && self.home.industry.rev >= u64::MAX - 1 {
            return Err("Kingdom revision limit reached".into());
        }
        // All possible failure checks precede this commit.
        self.book.insert(definition.clone());
        if !status.owned {
            let amount = self
                .home
                .industry
                .stock
                .get_mut(id)
                .expect("available stock checked");
            *amount -= 1;
            if *amount == 0 {
                self.home.industry.stock.remove(id);
            }
            self.home.industry.rev += 1;
        }
        if ledger_changed {
            let loadout = self.home.kingdom_gear.profiles.entry(profile).or_default();
            loadout.owned.insert(id.into());
            loadout.select(gear);
            self.home.kingdom_gear.rev += 1;
        }
        wear(self.players.get_mut(&player).unwrap(), gear, definition);
        Ok(format!("{} equipped. {}", gear.name, gear.controls))
    }

    /// Apply the selected bound loadout after the ordinary class kit is rebuilt
    /// for an expedition. Repeated calls consume no stock and grant no bonuses,
    /// pickups, healing, mana, spell copies or cooldown resets.
    pub fn restore_kingdom_gear(&mut self, player: u32) {
        if !self.home.kingdom_gear.valid() {
            return;
        }
        let Some(hero) = self.players.get(&player).filter(|h| !h.absent) else {
            return;
        };
        let loadout = self
            .home
            .kingdom_gear
            .profiles
            .get(&talents::profile_key(hero))
            .cloned();
        reclaim_unowned(self.players.get_mut(&player).unwrap(), loadout.as_ref());
        let Some(loadout) = loadout else {
            return;
        };
        for id in [loadout.arm, loadout.guard, loadout.spell]
            .into_iter()
            .flatten()
        {
            let gear = item(&id).expect("validated loadout");
            if can_equip(&self.players[&player], gear).is_err() {
                continue;
            }
            let definition = card(&id).expect("registered equipment");
            if self.book.insert(definition.clone()) {
                wear(self.players.get_mut(&player).unwrap(), gear, definition);
            }
        }
    }

    pub fn outfit_kingdom_gear(&mut self) {
        for player in self.players.keys().copied().collect::<Vec<_>>() {
            self.restore_kingdom_gear(player);
        }
    }
}

#[cfg(test)]
#[path = "../../tests/rules/kingdom_gear__tests.rs"]
mod tests;
