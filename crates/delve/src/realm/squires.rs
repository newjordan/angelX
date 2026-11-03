//! The yard's company book. A named knight owns a small roster, not the
//! party's summons or its one-delve sellsword.
use super::{Spoil, Spoils};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Kind {
    Ninja,
    Healer,
    Ranger,
    Shield,
    Alchemist,
}
impl Kind {
    pub const ALL: [Self; 5] = [
        Self::Ninja,
        Self::Healer,
        Self::Ranger,
        Self::Shield,
        Self::Alchemist,
    ];
    pub fn name(self) -> &'static str {
        match self {
            Self::Ninja => "Ninja",
            Self::Healer => "Healer",
            Self::Ranger => "Ranger",
            Self::Shield => "Shield",
            Self::Alchemist => "Alchemist",
        }
    }
    pub fn abilities(self, level: u8) -> &'static str {
        match (self, level) {
            (Self::Ninja, 0..=1) => "Stars",
            (Self::Ninja, 2) => "Stars / backstab",
            (Self::Ninja, _) => "Stars / backstab / smoke",
            (Self::Healer, 0..=1) => "Mend",
            (Self::Healer, 2) => "Mend / cleanse",
            (Self::Healer, _) => "Mend / cleanse / skull aid",
            (Self::Ranger, 0..=1) => "Arrows",
            (Self::Ranger, _) => "Arrows / mark",
            (Self::Shield, 0..=1) => "Block / bash",
            (Self::Shield, _) => "Block / bash / guard",
            (Self::Alchemist, 0..=1) => "Bomb",
            (Self::Alchemist, _) => "Bomb / potion",
        }
    }
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Squire {
    pub id: u32,
    pub name: String,
    pub kind: Kind,
    pub level: u8,
    pub talent: u8,
}
impl Squire {
    pub fn health(&self) -> u32 {
        (if self.kind == Kind::Shield { 110 } else { 65 })
            + u32::from(self.level) * 15
            + u32::from(self.talent) * 3
    }
    pub fn power(&self) -> u32 {
        7 + u32::from(self.level) * 4 + u32::from(self.talent)
    }
}
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct Roster {
    pub squires: Vec<Squire>,
    pub active: Option<u32>,
}
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct Company {
    pub recruit: u8,
    pub hire: u8,
    pub train: u8,
    pub offers: Vec<Squire>,
    pub rosters: BTreeMap<String, Roster>,
    pub cadence: u64,
    pub next_id: u32,
    pub ready: bool,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Building {
    Recruit,
    Hire,
    Train,
}
impl Company {
    pub fn is_empty(&self) -> bool {
        self == &Self::default()
    }
    pub fn cap(&self) -> usize {
        3 + usize::from(self.hire) * 2
    }
    pub fn ceiling(&self) -> u8 {
        3 + self.train * 2
    }
    pub fn level(&self, building: Building) -> u8 {
        match building {
            Building::Recruit => self.recruit,
            Building::Hire => self.hire,
            Building::Train => self.train,
        }
    }
    pub fn upgrade_price(&self, building: Building) -> u32 {
        150 * (u32::from(self.level(building)) + 1)
    }
    pub fn wage(&self, s: &Squire) -> u32 {
        (90 + u32::from(s.talent) * 15).saturating_sub(u32::from(self.hire) * 15)
    }
    pub fn training_price(&self, s: &Squire) -> u32 {
        (u32::from(s.level) + 1) * 45 / (u32::from(self.train) + 1)
    }
    pub fn refresh(&mut self, cadence: u64) {
        if self.ready && cadence == self.cadence {
            return;
        }
        self.ready = true;
        self.cadence = cadence;
        self.offers.clear();
        for i in 0..3 + usize::from(self.recruit) {
            self.next_id = self.next_id.saturating_add(1);
            let kind = Kind::ALL[(i + cadence as usize) % 5];
            let names = [
                "Ash", "Briar", "Rowan", "Pip", "Moss", "Wren", "Fern", "Kit",
            ];
            self.offers.push(Squire {
                id: self.next_id,
                name: format!(
                    "{} {}",
                    names[(self.next_id as usize + i) % names.len()],
                    self.next_id
                ),
                kind,
                level: 1,
                talent: self.recruit + ((cadence + i as u64) % 3) as u8,
            });
        }
    }
    pub fn upgrade(&mut self, building: Building, gold: &mut Spoils) -> Result<(), &'static str> {
        if self.level(building) >= 3 {
            return Err("Building complete");
        }
        pay(gold, self.upgrade_price(building))?;
        match building {
            Building::Recruit => self.recruit += 1,
            Building::Hire => self.hire += 1,
            Building::Train => self.train += 1,
        }
        // Board improvements show immediately; ordinary offers change once per delve.
        if building == Building::Recruit {
            self.ready = false;
            self.refresh(self.cadence);
        }
        Ok(())
    }
    pub fn hire(
        &mut self,
        knight: &str,
        offer: u32,
        gold: &mut Spoils,
    ) -> Result<(), &'static str> {
        if self
            .rosters
            .get(knight)
            .is_some_and(|r| r.squires.len() >= self.cap())
        {
            return Err("Roster full");
        }
        let i = self
            .offers
            .iter()
            .position(|s| s.id == offer)
            .ok_or("Offer already hired")?;
        pay(gold, self.wage(&self.offers[i]))?;
        let s = self.offers.remove(i);
        let roster = self.rosters.entry(knight.into()).or_default();
        if roster.active.is_none() {
            roster.active = Some(s.id);
        }
        roster.squires.push(s);
        Ok(())
    }
    pub fn select(&mut self, knight: &str, id: Option<u32>) -> Result<(), &'static str> {
        let roster = self.rosters.get_mut(knight).ok_or("Hire a squire first")?;
        if id.is_some_and(|id| !roster.squires.iter().any(|s| s.id == id)) {
            return Err("Not your squire");
        }
        roster.active = id;
        Ok(())
    }
    pub fn train(&mut self, knight: &str, id: u32, gold: &mut Spoils) -> Result<(), &'static str> {
        let s = self
            .rosters
            .get(knight)
            .and_then(|r| r.squires.iter().find(|s| s.id == id))
            .ok_or("Select a squire first")?;
        if s.level >= self.ceiling() {
            return Err("Upgrade training yard");
        }
        let price = self.training_price(s);
        pay(gold, price)?;
        self.rosters
            .get_mut(knight)
            .unwrap()
            .squires
            .iter_mut()
            .find(|s| s.id == id)
            .unwrap()
            .level += 1;
        Ok(())
    }
    pub fn valid(&self) -> bool {
        let mut ids = std::collections::BTreeSet::new();
        let valid_squire = |s: &Squire| {
            (1..=9).contains(&s.level) && s.talent <= 5 && !s.name.is_empty() && s.name.len() <= 80
        };
        self.recruit <= 3
            && self.hire <= 3
            && self.train <= 3
            && self.offers.len() <= 6
            && self
                .offers
                .iter()
                .all(|s| valid_squire(s) && ids.insert(s.id))
            && self.rosters.values().all(|r| {
                r.squires.len() <= self.cap()
                    && r.active
                        .is_none_or(|id| r.squires.iter().any(|s| s.id == id))
                    && r.squires
                        .iter()
                        .all(|s| valid_squire(s) && ids.insert(s.id))
            })
    }
}
fn pay(gold: &mut Spoils, amount: u32) -> Result<(), &'static str> {
    let mut cost = Spoils::default();
    cost.add(Spoil::Gold, amount);
    if !gold.covers(&cost) {
        return Err("Treasury needs gold");
    }
    gold.take(&cost);
    Ok(())
}
#[cfg(test)]
#[path = "../../tests/realm/squires__tests.rs"]
mod tests;
