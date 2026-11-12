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

/// What the seam keeps between ticks: not saved, not sent, never compared.
#[derive(Clone, Debug, Default)]
pub struct Seen {
    /// The sim's people at their posts, by trade (refreshed each second).
    pub staffed: BTreeMap<String, u32>,
    /// The tick the Keep's staking was last tried, and what the sim said.
    pub tried: u64,
    pub note: String,
    /// The tiles the kingdom's buildings shut in the realm's room, and the
    /// revision of the kingdom they were read at (`hillside::follow_works_tiles`).
    pub walled: std::collections::BTreeSet<(i16, i16)>,
    pub walled_rev: u64,
}

impl PartialEq for Seen {
    fn eq(&self, _: &Self) -> bool {
        true
    }
}

/// How far a mason's stone is set past the last bay the carpenters raised, at
/// most: the masons' shift may cross the line by this many stones before the
/// next shift waits.
pub const OVERSHOOT: u32 = 4;

impl Run {
    /// The id of the sim's site for the Keep, when the hillside kingdom is
    /// founded and the Keep is staked in it: the seams are live.
    pub fn keep_live(&self) -> Option<u32> {
        let k = &self.home.industry;
        if k.is_empty() || !k.world.is_hillside() {
            return None;
        }
        k.keep_site()
    }

    /// Whether the realm's industry is the hillside kingdom (founded, the Keep
    /// staked yet or not): the Works' stub crew then stands down, for the sim's own
    /// people are the realm's hired hands and the sim pays the wall's bill.
    pub fn sim_hillside(&self) -> bool {
        let k = &self.home.industry;
        !k.is_empty() && k.world.is_hillside()
    }

    /// Found the realm's industry: the hillside on the blank-slate map, the
    /// valley as ever elsewhere.
    pub(super) fn found_industry(&mut self) {
        Self::found_in(&mut self.home.industry, self.seed);
    }

    /// [`Self::found_industry`] for a kingdom borrowed apart from the run. Gives the names of
    /// the works an old village had that the hillside has no place for, when it was brought up,
    /// and what they cost (given back to the camp's stock).
    pub(super) fn found_in(industry: &mut crate::kingdom::Kingdom, seed: u64) -> (Vec<String>, String) {
        let mut left = Vec::new();
        let mut refund = String::new();
        if crate::world::plan::tabula() {
            industry.found_hillside(catalog(), seed);
            // A village founded on the old valley meadow comes up onto the hillside:
            // the realm is one map.
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

    /// The people the sim has at work in `trade`, when live; else what the
    /// Works' crew was before it (`hands::hired`).
    pub fn hired_hands(&self, trade: super::trades::Trade) -> u32 {
        if self.keep_live().is_some() {
            self.hands.seen.staffed.get(trade.id()).copied().unwrap_or(0)
        } else {
            hands::hired(trade)
        }
    }

    /// The Great Shaft's lift in the sim, and the stockyard tier it stands for.
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

    /// Once a simulation second, after the sim has moved: set the Great Shaft's
    /// limit from the stockyard's tier, stake out the Keep if it can be,
    /// keep the sim's wall at the game's, read who is at work, and on every
    /// tenth second let the masons set what the yard pays for.
    pub(super) fn seam_second(&mut self) {
        if crate::world::plan::tabula() && self.home.industry.is_valley_village() {
            let (left, refund) = Self::found_in(&mut self.home.industry, self.seed);
            if self.home.industry.world.is_hillside() {
                self.notices.push("Your village has come up onto the hillside, round the camp below the Keep.".into());
                // What the hillside had no place for is said, not lost in silence.
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
        // One scale: the Great Shaft is the sim's lift (blocks a minute: 3, 10, 24, 60,
        // 140 up its five lifts), and the stockyard's tier is the lift the camp has built.
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
        // The wall the game shows is the wall the sim pays for.
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

    /// A ten-second shift of masonry: the lodges' crews set what the yard can pay
    /// for, and those stones stand in the game's wall under the realm's mark.
    /// The masons wait while a knight is bedding a stone at the course, and
    /// when the next bay of falsework is not raised.
    fn seam_shift(&mut self, id: u32) {
        let Some(site) = self.sites.keep() else { return };
        let Some(castle) = site.castle() else { return };
        // The masons lay the stones a raid knocked out before they lay another.
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

    /// The one falsework: the bays of scaffold the carpenters raise ahead of the
    /// masons (`hands::Yard::timber`, the picture's scaffold and the masons' limit) are
    /// the sim's poles in the yard. While the scaffold stands the next bay is
    /// raised ahead of the wall; with the poles gone it is raised only as far as
    /// the wall has gone. A knight carpenter stands poles from the camp's stock
    /// (`Kingdom::stand_poles`) and the hired carpenters are the sim's woodsmen.
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

    /// Why the yard cannot pay for another stone, in a few words, or none if it can pay for `n` more.
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

    /// The Clerk of Works' sentence for the Keep, from the sim, when live.
    pub fn keep_clerk(&self) -> Option<String> {
        let id = self.keep_live()?;
        Some(self.home.industry.clerk(catalog(), id).sentence.to_uppercase())
    }

    /// Whether a knight at a bench has rough stone to cut: the camp's rock when
    /// the sim is live, the stockyard's store else.
    pub fn rough_ready(&self) -> bool {
        if self.keep_live().is_some() {
            let k = &self.home.industry;
            k.rock_in_camp(catalog()).iter().any(|(r, n)| *n >= crate::kingdom::Kingdom::dress_cost(r))
        } else {
            self.home.stockyard.can_cut()
        }
    }

    /// Mortar at the tub: the yard's own, with the sim live, which goes off in
    /// its shelf's time; else the stub's tub, filled by the mill's stub.
    pub fn tub_stock(&self) -> u32 {
        match self.keep_live() {
            Some(id) => self.home.industry.yard_stock(id, "mortar"),
            None => self.hands.yard.tub,
        }
    }

    /// The stockyard as the picture shows it: the realm's store, or, when the
    /// sim is live, the camp's rock stood in the yard (the field stone as rubble).
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

    /// Rough stone landing at the stockyard. With the sim live it goes into the
    /// camp's stock, which the sim's carts take on to the bankers; else into the
    /// stockyard's own store. Returns how many went in.
    pub fn stock_rough(&mut self, stone: &str, n: u32) -> u32 {
        if self.keep_live().is_some() {
            // The hillside's field stone is the sim's `stone`; "rubble" from the old store is the same.
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
