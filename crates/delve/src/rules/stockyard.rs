//! The Delve gate's stockyard and the Great Shaft above the Delve.
//!
//! Everything from below has to come up, and the shaft is the first true
//! bottleneck in the kingdom: hand carry, then a windlass, a horse gin, a
//! water-driven hoist and a ley lift, each lifting more than the last and
//! each paid for from what the camp can make. The stockyard is where the
//! stone lands: a real store on the realm's `Home`, saved with it and sent
//! to every guest with the run. `Tier::throughput` is the number the
//! production game reads.
use super::*;
use crate::kingdom::{Stock, catalog};

pub const BLOCK_PACE: f32 = 0.62;
pub const COIN_WAITING: u32 = 1_000_000;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Tier {
    #[default]
    Hand,
    Windlass,
    HorseGin,
    WaterHoist,
    LeyLift,
}

impl Tier {
    pub const ALL: [Tier; 5] = [
        Tier::Hand,
        Tier::Windlass,
        Tier::HorseGin,
        Tier::WaterHoist,
        Tier::LeyLift,
    ];
    pub fn index(self) -> usize {
        Self::ALL.iter().position(|&t| t == self).unwrap_or(0)
    }
    pub fn name(self) -> &'static str {
        ["HAND CARRY", "WINDLASS", "HORSE GIN", "WATER HOIST", "LEY LIFT"][self.index()]
    }
    pub fn says(self) -> &'static str {
        [
            "A rope and bucket, and every knight a porter.",
            "A drum and crank at the shaft head, two to turn it.",
            "A capstan walked round by a horse.",
            "A wheel in the race drives the hoist day and night.",
            "A column of ley light lifts the load on its own.",
        ][self.index()]
    }
    pub fn throughput(self) -> u32 {
        [3, 10, 24, 60, 140][self.index()]
    }
    pub fn capacity(self) -> u32 {
        [16, 40, 96, 200, 400][self.index()]
    }
    pub fn noise(self) -> u16 {
        [0, 1, 1, 2, 2][self.index()]
    }
    pub fn next(self) -> Option<Tier> {
        Self::ALL.get(self.index() + 1).copied()
    }
    pub fn cost(self) -> Stock {
        let goods: &[(&str, u32)] = match self {
            Tier::Hand => &[],
            Tier::Windlass => &[("plank", 12), ("timber", 8), ("stone", 10)],
            Tier::HorseGin => &[("plank", 30), ("ingot", 6), ("block", 10)],
            Tier::WaterHoist => &[("plank", 40), ("ingot", 14), ("block", 30)],
            Tier::LeyLift => &[("ingot", 30), ("mana", 40), ("block", 60)],
        };
        goods.iter().map(|&(r, n)| (r.to_string(), n)).collect()
    }
    pub fn needs(self) -> Option<&'static str> {
        match self {
            Tier::WaterHoist => Some("waterwheel"),
            Tier::LeyLift => Some("mana-engine"),
            _ => None,
        }
    }
}

pub fn cost_label(cost: &Stock) -> String {
    cost.iter()
        .map(|(r, n)| format!("{n} {}", catalog().resources[r].name.to_uppercase()))
        .collect::<Vec<_>>()
        .join(", ")
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct Stockyard {
    pub rev: u64,
    pub tier: Tier,
    pub stone: BTreeMap<String, u32>,
    pub hauled: u32,
    pub coin: u32,
}

impl Stockyard {
    pub fn is_empty(&self) -> bool {
        *self == Self::default()
    }
    pub fn held(&self) -> u32 {
        self.stone.values().sum()
    }
    pub fn room(&self) -> u32 {
        self.tier.capacity().saturating_sub(self.held())
    }
    pub fn count(&self, stone: &str) -> u32 {
        self.stone.get(stone).copied().unwrap_or(0)
    }
    pub fn valid(&self) -> bool {
        self.rev < u64::MAX
            && self.stone.len() <= geology::ROWS.len() + 1
            && self
                .stone
                .iter()
                .all(|(r, &n)| n > 0 && geology::is_rough(r) && catalog().resources.contains_key(r))
            && self.held() <= self.tier.capacity()
            && self.hauled < u32::MAX / 2
            && self.coin <= COIN_WAITING
    }
    pub fn stack(&mut self, stone: &str, n: u32) -> u32 {
        let mut put = n.min(self.room());
        if stone == geology::RUBBLE {
            put = put.min(Self::RUBBLE_MOST.saturating_sub(self.count(stone)));
        }
        if put > 0 && geology::is_rough(stone) {
            *self.stone.entry(stone.to_string()).or_default() += put;
            self.hauled = self.hauled.saturating_add(put);
            self.rev = self.rev.saturating_add(1);
            return put;
        }
        0
    }
    pub fn draw(&mut self, stone: &str, n: u32) -> u32 {
        let took = n.min(self.count(stone));
        if took > 0 {
            if let Some(held) = self.stone.get_mut(stone) {
                *held -= took;
            }
            self.stone.retain(|_, n| *n > 0);
            self.rev = self.rev.saturating_add(1);
        }
        took
    }
    pub const RUBBLE_MOST: u32 = 8;

    pub fn can_cut(&self) -> bool {
        self.held() > 0
    }

    pub fn wants_rubble(&self) -> bool {
        self.room() > 0 && self.count(geology::RUBBLE) < Self::RUBBLE_MOST
    }

    pub fn take_for_cut(&mut self) -> Option<String> {
        let best = self
            .stone
            .iter()
            .filter(|(r, _)| r.as_str() != geology::RUBBLE)
            .max_by_key(|&(_, n)| *n)
            .map(|(r, _)| r.clone());
        if let Some(stone) = best {
            self.draw(&stone, 1);
            return Some(stone);
        }
        if self.draw(geology::RUBBLE, 1) > 0 {
            return Some(geology::RUBBLE.to_string());
        }
        None
    }

    pub fn bank_gold(&mut self, gold: u32) {
        if gold > 0 {
            self.coin = self.coin.saturating_add(gold).min(COIN_WAITING);
            self.rev = self.rev.saturating_add(1);
        }
    }
}

impl Run {
    pub fn shaft_throughput(&self) -> u32 {
        if self.shaft_lift().is_some() {
            return self.home.industry.report(catalog()).shaft;
        }
        self.home.stockyard.tier.throughput()
    }

    pub fn shaft_next(&self) -> Option<(Tier, Stock, Result<(), String>)> {
        if let Some((id, tier)) = self.shaft_lift() {
            let next = tier.next()?;
            let k = &self.home.industry;
            let c = catalog();
            let (_, plan) = c.plans.iter().find(|(_, p)| p.replaces.as_deref() == k.buildings.get(&id).map(|b| b.plan.as_str()))?;
            let ready = if k.buildings.get(&id).is_some_and(|b| b.construction > 0) {
                Err("the lift is still being raised".to_string())
            } else if !plan.requires.iter().all(|r| k.researched.contains(r)) {
                Err(format!("research {} first", plan.requires.join(", ")))
            } else {
                let missing: Vec<String> = plan
                    .cost
                    .iter()
                    .filter(|(r, n)| k.amount(r) < **n)
                    .map(|(r, n)| format!("{} {}", n - k.amount(r), c.resources[r].name.to_lowercase()))
                    .collect();
                if missing.is_empty() { Ok(()) } else { Err(format!("need {}", missing.join(", "))) }
            };
            return Some((next, plan.cost.clone(), ready));
        }
        let next = self.home.stockyard.tier.next()?;
        let cost = next.cost();
        let k = &self.home.industry;
        let ready = if k.is_empty() {
            Err("the kingdom is not founded: open the desk [4]".to_string())
        } else if let Some(plan) = next
            .needs()
            .filter(|p| catalog().plans.contains_key(*p))
            .filter(|p| !k.buildings.values().any(|b| b.plan == *p && b.construction == 0))
        {
            Err(format!("build a {} first", catalog().plans[plan].name.to_lowercase()))
        } else {
            let missing: Vec<String> = cost
                .iter()
                .filter(|(r, n)| k.amount(r) < **n)
                .map(|(r, n)| format!("{} {}", n - k.amount(r), catalog().resources[r].name.to_lowercase()))
                .collect();
            if missing.is_empty() {
                Ok(())
            } else {
                Err(format!("need {}", missing.join(", ")))
            }
        };
        Some((next, cost, ready))
    }

    pub fn upgrade_shaft(&mut self) -> Result<String, String> {
        let Some((next, cost, ready)) = self.shaft_next() else {
            return Err("the shaft is as deep and as strong as it gets".into());
        };
        ready?;
        if let Some((id, _)) = self.shaft_lift() {
            self.home.industry.apply(catalog(), crate::kingdom::Command::Upgrade(id))?;
        } else {
            self.home.industry.spend(catalog(), &cost)?;
        }
        self.home.stockyard.tier = next;
        self.home.stockyard.rev = self.home.stockyard.rev.saturating_add(1);
        self.stir_up(stir::UPGRADE);
        self.sounds.push("door_open");
        let line = format!(
            "The Great Shaft: {}. It lifts {} blocks a minute.",
            next.name(),
            next.throughput()
        );
        self.notices.push(line.clone());
        Ok(line)
    }

    pub(super) fn tally_coin(&mut self, spoils: &Spoils) {
        self.home.stockyard.bank_gold(spoils.get(Spoil::Gold));
    }

    pub(super) fn deliver_coin(&mut self) {
        if self.home.stockyard.coin == 0 || self.home.industry.is_empty() {
            return;
        }
        let taken = self
            .home
            .industry
            .receive(catalog(), "coin", self.home.stockyard.coin);
        if taken > 0 {
            self.home.stockyard.coin -= taken;
            self.home.stockyard.rev = self.home.stockyard.rev.saturating_add(1);
        }
    }
}

#[cfg(test)]
#[path = "../../tests/rules/stockyard__tests.rs"]
mod tests;
