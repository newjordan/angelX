//! The Keep's wall on the production sim (the Works, `kingdom::seam`).
//!
//! On the blank-slate map the realm's industry is the hillside kingdom, and the
//! Keep is the realm's long project: the great tower, begun, whose site in the
//! sim is staked out from how far the game's wall has got (`Run.sites`: the
//! plan, its stages and bills are the sim's `great-tower`). From then on:
//!
//! - **hired setters** are the sim's masons: once a shift the lodges' crews set
//!   what the yard can pay for (`Kingdom::work_sites`), and the stones come
//!   up in the game's wall under the realm's own mark;
//! - **a knight's set stone** is paid from the same yard by the same bill
//!   (`Kingdom::site_hand_set`), laid in order with the masons';
//! - **hired hands by trade** are the sim's people at their posts;
//! - **the Great Shaft's limit** is the stockyard's tier (`Tier::throughput`);
//! - **mortar** is the yard's real stock, which goes off;
//! - **rock a knight brings up** goes into the camp's stock and the sim's carts
//!   take it to the bankers; a knight at the bench dresses camp rock into the
//!   yard's cut stone.
//!
//! None of it applies without the hillside kingdom founded and the Keep staked
//! (a run on the old map, an old save, a kingdom not yet founded): the
//! Works' hand chain keeps its own stores then, as it was (`hands`).
use super::hands::{self, Work};
use super::*;
use crate::kingdom::{Hold, catalog};

#[derive(Clone, Debug, Default)]
pub struct Seen {
    pub staffed: BTreeMap<String, u32>,
    pub tried: u64,
    pub note: String,
    pub walled: std::collections::BTreeSet<(i16, i16)>,
    pub walled_rev: u64,
}

impl PartialEq for Seen {
    fn eq(&self, _: &Self) -> bool {
        true
    }
}

pub const OVERSHOOT: u32 = 4;

impl Run {
    pub fn keep_live(&self) -> Option<u32> {
        let k = &self.home.industry;
        if k.is_empty() || !k.world.is_hillside() {
            return None;
        }
        k.keep_site()
    }

    pub fn sim_hillside(&self) -> bool {
        let k = &self.home.industry;
        !k.is_empty() && k.world.is_hillside()
    }

    pub(super) fn found_industry(&mut self) {
        Self::found_in(&mut self.home.industry, self.seed);
    }

    pub(super) fn found_in(industry: &mut crate::kingdom::Kingdom, seed: u64) -> (Vec<String>, String) {
        let mut left = Vec::new();
        let mut refund = String::new();
        if crate::world::plan::tabula() {
            industry.found_hillside(catalog(), seed);
            if industry.is_valley_village()
                && let Ok(moved) = industry.migrate_to_hillside(catalog(), seed)
            {
                left = moved.left.iter().map(|(plan, _)| catalog().plans.get(plan).map_or(plan.clone(), |p| p.name.clone())).collect();
                refund = moved
                    .refund
                    .iter()
                    .map(|(r, n)| format!("{n} {}", catalog().resources.get(r).map_or(r.as_str(), |x| x.name.as_str()).to_lowercase()))
                    .collect::<Vec<_>>()
                    .join(", ");
            }
        } else {
            industry.found(catalog(), seed);
        }
        (left, refund)
    }

    pub fn hired_hands(&self, trade: super::trades::Trade) -> u32 {
        if self.keep_live().is_some() {
            self.hands.seen.staffed.get(trade.id()).copied().unwrap_or(0)
        } else {
            hands::hired(trade)
        }
    }

    pub fn shaft_lift(&self) -> Option<(u32, super::stockyard::Tier)> {
        use super::stockyard::Tier;
        let k = &self.home.industry;
        if k.is_empty() || !k.world.is_hillside() {
            return None;
        }
        k.buildings.iter().find_map(|(&id, b)| {
            let tier = match b.plan.as_str() {
                "shaft-hand" => Tier::Hand,
                "shaft-windlass" => Tier::Windlass,
                "shaft-gin" => Tier::HorseGin,
                "shaft-water" => Tier::WaterHoist,
                "shaft-ley" => Tier::LeyLift,
                _ => return None,
            };
            Some((id, tier))
        })
    }

    pub(super) fn seam_second(&mut self) {
        if crate::world::plan::tabula() && self.home.industry.is_valley_village() {
            let (left, refund) = Self::found_in(&mut self.home.industry, self.seed);
            if self.home.industry.world.is_hillside() {
                self.notices.push("Your village has come up onto the hillside, round the camp below the Keep.".into());
                if !left.is_empty() {
                    let back = if refund.is_empty() { String::new() } else { format!(" What they cost went back to the camp: {refund}.") };
                    self.notices.push(format!("Left behind in the valley: {}.{back}", left.join(", ")));
                }
            }
        }
        {
            let k = &self.home.industry;
            if k.is_empty() || !k.world.is_hillside() {
                return;
            }
        }
        self.home.industry.set_shaft_limit(None);
        if let Some(tier) = self.shaft_lift().map(|(_, tier)| tier)
            && self.home.stockyard.tier != tier
        {
            self.home.stockyard.tier = tier;
            self.home.stockyard.rev = self.home.stockyard.rev.saturating_add(1);
        }
        let laid = self.sites.keep().map(|s| s.laid);
        if self.keep_live().is_none()
            && let Some(laid) = laid
            && self.tick.saturating_sub(self.hands.seen.tried) >= 10 * u64::from(HZ)
        {
            self.hands.seen.tried = self.tick;
            match self.home.industry.found_keep(catalog(), laid) {
                Ok(_) => self.hands.seen.note.clear(),
                Err(why) => self.hands.seen.note = why,
            }
        }
        let Some(id) = self.keep_live() else { return };
        if let Some(laid) = laid
            && self.home.industry.sites.get(&id).is_some_and(|s| s.laid != laid)
        {
            let _ = self.home.industry.site_progress(catalog(), id, laid);
        }
        self.hands.seen.staffed = self.home.industry.staffed_by_trade(catalog());
        self.derive_bays(id);
        if self.home.industry.second.is_multiple_of(crate::kingdom::pace::job_every()) {
            self.seam_shift(id);
        }
    }

    fn seam_shift(&mut self, id: u32) {
        let Some(site) = self.sites.keep() else { return };
        let Some(castle) = site.castle() else { return };
        let laid = site.next();
        if self.site_replay.is_some()
            || laid >= castle.len()
            || self.hands.seats.values().any(|h| matches!(h.work, Some(Work::Set { .. })))
            || !self.hands.yard.done.is_empty()
            || laid >= hands::limit(castle, &self.hands.yard, laid).saturating_add(OVERSHOOT)
        {
            return;
        }
        let set: u32 = self.home.industry.work_sites(catalog()).into_iter().filter(|&(site, _)| site == id).map(|(_, n)| n).sum();
        if set == 0 {
            return;
        }
        let runs = self.home.industry.sites.get(&id).map(|sim| {
            let from = sim.laid.saturating_sub(set);
            let mut runs = Vec::new();
            let mut i = from;
            while i < sim.laid {
                let kind = super::site::Site::stone_kind(&sim.bands, i);
                let mut end = i + 1;
                while end < sim.laid && super::site::Site::stone_kind(&sim.bands, end) == kind {
                    end += 1;
                }
                runs.push((end - i, kind));
                i = end;
            }
            runs
        }).unwrap_or_else(|| vec![(set, 0)]);
        if let Some(site) = self.sites.keep_mut() {
            for (n, kind) in runs {
                for _ in 0..n {
                    site.lay_set(0, false);
                }
                site.note_kind(n, kind);
            }
        }
        self.sounds.push("rock_land");
    }

    fn derive_bays(&mut self, id: u32) {
        let Some(castle) = self.sites.keep().and_then(|s| s.castle()) else { return };
        let laid = self.sites.keep().map_or(0, |s| s.laid);
        let stands = self.home.industry.scaffold_stands(catalog(), id);
        let bays = hands::bays(castle);
        let passed = bays.iter().filter(|b| b.appears < laid).map(|b| b.appears).max().unwrap_or(0);
        let ahead = if stands {
            bays.iter().map(|b| b.appears).find(|&a| a >= laid).unwrap_or(castle.len())
        } else {
            passed
        };
        self.hands.yard.timber = ahead.max(passed);
    }

    pub fn yard_holds(&self, n: u32) -> Option<String> {
        let id = self.keep_live()?;
        let a = self.home.industry.site_allowance(catalog(), id, u32::MAX / 200);
        if a.stones >= n {
            return None;
        }
        let name = |r: &str| catalog().resources.get(r).map_or_else(|| r.to_uppercase(), |x| x.name.to_uppercase());
        Some(match a.hold {
            Hold::Done => "THE KEEP STANDS".into(),
            Hold::Yard => "NO STOCKYARD YET".into(),
            Hold::Earthwork => "THE DITCH IS BEING DUG".into(),
            Hold::Scaffold { .. } => "WAITING ON SCAFFOLD POLES".into(),
            Hold::Centring { .. } => "WAITING ON ARCH CENTRING".into(),
            Hold::Short { resource } => format!("WAITING ON {}: THE YARD HAS NONE", name(&resource)),
            Hold::Masons | Hold::Pace => "THE YARD CANNOT PAY FOR ANOTHER STONE".into(),
        })
    }

    pub fn keep_clerk(&self) -> Option<String> {
        let id = self.keep_live()?;
        Some(self.home.industry.clerk(catalog(), id).sentence.to_uppercase())
    }

    pub fn rough_ready(&self) -> bool {
        if self.keep_live().is_some() {
            let k = &self.home.industry;
            k.rock_in_camp(catalog()).iter().any(|(r, n)| *n >= crate::kingdom::Kingdom::dress_cost(r))
        } else {
            self.home.stockyard.can_cut()
        }
    }

    pub fn tub_stock(&self) -> u32 {
        match self.keep_live() {
            Some(id) => self.home.industry.yard_stock(id, "mortar"),
            None => self.hands.yard.tub,
        }
    }

    pub fn yard_view(&self) -> super::stockyard::Stockyard {
        let mut view = self.home.stockyard.clone();
        if self.keep_live().is_some() {
            view.stone.clear();
            for (r, n) in self.home.industry.rock_in_camp(catalog()) {
                let name = if r == "stone" { super::geology::RUBBLE.to_string() } else { r };
                *view.stone.entry(name).or_default() += n;
            }
        }
        view
    }

    pub fn stock_rough(&mut self, stone: &str, n: u32) -> u32 {
        if self.keep_live().is_some() {
            let good = if stone == super::geology::RUBBLE { "stone" } else { stone };
            let took = self.home.industry.receive(catalog(), good, n);
            if took > 0 {
                self.home.stockyard.hauled = self.home.stockyard.hauled.saturating_add(took);
                self.home.stockyard.rev = self.home.stockyard.rev.saturating_add(1);
            }
            return took;
        }
        self.home.stockyard.stack(stone, n)
    }
}

#[cfg(test)]
#[path = "../../tests/rules/works_seam__tests.rs"]
mod tests;
