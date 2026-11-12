//! The game's seams into the production sim: what the Delve adapter and the
//! Keep's hands read and do on the hillside. Everything the sim does with
//! these is its own (stock, sites, lines, carts, people); this file only
//! gives the game places to stand:
//!
//! - the Great Shaft's limit, set from outside (the stockyard's tier);
//! - the workers staffed at their posts, by trade;
//! - the Keep's site, founded from how far the game's wall has got;
//! - a knight's set stone, paid from the same yard as the masons' (`site_hand_set`);
//! - a knight at the bench, dressing rock from the camp into the yard's cut stone.
use crate::economy::{add, take};
use crate::*;
use std::collections::{BTreeMap, BTreeSet};

/// Where the Keep's stockyard stands: below its south-east corner by the camp, outside the walls.
pub const KEEP_YARD: Tile = (147, 96);

/// The castle the game's Keep raises.
pub const KEEP_PLAN: &str = "great-tower";

/// The most units of rock and ore one batch of a claim's quarry brings up:
/// eight goods of eight, which `Claim::valid` allows. The bucket holds at
/// least this much, so a limit never starves a batch for good.
const MOST_BATCH: u32 = 64;

impl Kingdom {
    /// Limit what goes up the Great Shaft to `per_minute` units, whatever lifts
    /// the camp has built, or lift it again with None.
    pub fn set_shaft_limit(&mut self, per_minute: Option<u32>) {
        if self.shaft_limit != per_minute {
            self.shaft_limit = per_minute;
            self.rev = self.rev.saturating_add(1);
        }
    }

    /// The lift of the shift about to run. `built` is what the staffed lifts
    /// raise in a minute (blocks a minute: 3, 10, 24, 60, 140 up the five lifts,
    /// the one scale the game's stockyard tiers use); the game may hold it lower
    /// (`set_shaft_limit`). A shift lifts a sixth of a minute's worth, saved
    /// from shift to shift in sixths of a unit, never more than a batch of the
    /// biggest claim holds.
    pub(crate) fn shift_lift(&mut self, built: u32) -> u32 {
        let per_minute = self.shaft_limit.map_or(built, |limit| built.min(limit));
        self.shaft_bucket = (self.shaft_bucket + per_minute).min(per_minute.max(6 * MOST_BATCH).max(per_minute * 12));
        self.shaft_bucket / 6
    }

    /// What the shift hoisted comes out of the saved lift.
    pub(crate) fn shaft_spent(&mut self) {
        self.shaft_bucket = self.shaft_bucket.saturating_sub(self.shaft_used.saturating_mul(6));
    }

    /// The people at their posts in each trade: every finished, unpaused post's crew
    /// as the population has staffed it, by the trade the post names.
    pub fn staffed_by_trade(&self, c: &Catalog) -> BTreeMap<String, u32> {
        let report = self.report(c);
        let mut out = BTreeMap::new();
        for (id, b) in &self.buildings {
            let plan = &c.plans[&b.plan];
            if plan.trade.is_empty() || b.construction > 0 || b.paused {
                continue;
            }
            // At a post: at work there, or standing idle there, waiting on goods.
            let crew = u32::from(report.population.assigned.get(id).copied().unwrap_or(0))
                + u32::from(report.population.waiting.get(id).copied().unwrap_or(0));
            if crew > 0 {
                *out.entry(plan.trade.clone()).or_default() += crew;
            }
        }
        out
    }

    /// The site that raises the game's Keep, if one has been staked out.
    pub fn keep_site(&self) -> Option<u32> {
        self.sites.iter().find(|(_, s)| s.plan == KEEP_PLAN).map(|(&id, _)| id)
    }

    /// Stake out the Keep with `laid` stones already standing (the game's wall as
    /// it is), or find it if it is staked. Needs the hillside.
    pub fn found_keep(&mut self, c: &Catalog, laid: u32) -> Result<u32, String> {
        if let Some(id) = self.keep_site() {
            return Ok(id);
        }
        let yard = self.keep_yard_tile(c).ok_or("no ground for the Keep's stockyard")?;
        self.apply(c, Command::FoundSite { plan: KEEP_PLAN.into(), centre: hillside::KEEP_CENTRE, yard })?;
        let id = self.keep_site().ok_or("the Keep was not staked")?;
        self.site_progress(c, id, laid)?;
        Ok(id)
    }

    /// The nearest open ground to [`KEEP_YARD`] a stockyard may stand on, outside
    /// the Keep's walls.
    fn keep_yard_tile(&self, c: &Catalog) -> Option<Tile> {
        for r in 0i16..30 {
            for dy in -r..=r {
                for dx in -r..=r {
                    if dx.abs().max(dy.abs()) != r {
                        continue;
                    }
                    let t = (KEEP_YARD.0 + dx, KEEP_YARD.1 + dy);
                    if self.world.contains(t)
                        && self.at(t).is_none()
                        && !self.roads.contains(&t)
                        && self.check_site(c, "castle-yard", t).is_ok()
                        && !crate::hillside::in_rect(t, crate::hillside::keep_rect())
                    {
                        return Some(t);
                    }
                }
            }
        }
        None
    }

    /// Say how many stones stand at a site: its stage, and the work in hand,
    /// are those of the stone after the last. For the game's own wall.
    pub fn site_progress(&mut self, c: &Catalog, id: u32, laid: u32) -> Result<(), String> {
        let plan = &c.castles[&self.sites.get(&id).ok_or("no such site")?.plan];
        if laid > plan.stones() {
            return Err("more stones than the castle has".into());
        }
        let stage = plan
            .stages
            .iter()
            .position(|s| s.count > 0 && laid < s.first + s.count)
            .unwrap_or(plan.stages.len()) as u16;
        let site = self.sites.get_mut(&id).ok_or("no such site")?;
        if (site.stage, site.laid) != (stage, laid) {
            (site.stage, site.laid, site.dug, site.credit) = (stage, laid, 0, 0);
            self.rev = self.rev.saturating_add(1);
        }
        Ok(())
    }

    /// A stone a knight has set by hand: the same bill the masons' stones pay,
    /// from the same yard, and the same stones standing after it. The masons'
    /// pace is not the knight's; their saved effort is left as it was.
    pub fn site_hand_set(&mut self, c: &Catalog, id: u32) -> Result<(), String> {
        let credit = self.sites.get(&id).ok_or("no such site")?.credit;
        self.site_set(c, id, 1, u32::MAX / 200)?;
        if let Some(site) = self.sites.get_mut(&id) {
            site.credit = site.credit.min(credit);
        }
        Ok(())
    }

    /// Whether the yard could pay for the next stone as things stand, whoever
    /// sets it, and if not what stops it.
    pub fn site_ready(&self, c: &Catalog, id: u32) -> Hold {
        let a = self.site_allowance(c, id, u32::MAX / 200);
        if a.stones > 0 { Hold::Pace } else { a.hold }
    }

    /// A knight carpenter stands the camp's poles in the yard's scaffold, as many
    /// as it lacks and the camp has: the one falsework, the sim's, whoever raises it.
    pub fn stand_poles(&mut self, c: &Catalog, id: u32) -> Result<u32, String> {
        let yard = self.sites.get(&id).ok_or("no such site")?.yard;
        let want = c.works.poles_standing + c.works.poles_worn;
        let have = self.yard_stock(id, "pole");
        let n = want.saturating_sub(have).min(self.amount("pole"));
        if n == 0 {
            return Err(if have >= want { "the scaffold is up".into() } else { "no poles in the camp".into() });
        }
        take(&mut self.stock, &Stock::from([("pole".to_string(), n)]));
        add(&mut self.buildings.get_mut(&yard).ok_or("the yard is gone")?.input, "pole", n);
        self.rev = self.rev.saturating_add(1);
        Ok(n)
    }

    /// Whether the site's scaffold stands: the yard holds the poles it wants standing.
    pub fn scaffold_stands(&self, c: &Catalog, id: u32) -> bool {
        self.yard_stock(id, "pole") >= c.works.poles_standing
    }

    /// How much of `good` the site's yard holds.
    pub fn yard_stock(&self, id: u32, good: &str) -> u32 {
        self.sites
            .get(&id)
            .and_then(|s| self.buildings.get(&s.yard))
            .map_or(0, |y| y.input.get(good).copied().unwrap_or(0))
    }

    /// The goods that are rock for the bankers: what the dressing recipes take.
    fn rocks(c: &Catalog) -> BTreeSet<String> {
        c.recipes
            .iter()
            .filter(|(name, _)| name.starts_with("dress-"))
            .flat_map(|(_, r)| r.inputs.keys().cloned())
            .collect()
    }

    /// The rock the camp holds, the best of it first: dressed quarry stone before
    /// the hillside's field stone.
    pub fn rock_in_camp(&self, c: &Catalog) -> Vec<(String, u32)> {
        let mut rocks: Vec<(String, u32)> = Self::rocks(c)
            .into_iter()
            .map(|r| {
                let n = self.amount(&r);
                (r, n)
            })
            .filter(|(_, n)| *n > 0)
            .collect();
        rocks.sort_by_key(|(r, n)| (r == "stone", std::cmp::Reverse(*n), r.clone()));
        rocks
    }

    /// What dressing one cut stone of `good` takes of `rock`: one block of
    /// quarry rock, three of field stone (the hillside's is poor: eight make
    /// three, as the bankers' own recipe has it, and the rest is rubble).
    pub fn dress_cost(rock: &str) -> u32 {
        if rock == "stone" { 3 } else { 1 }
    }

    /// A knight at a bench dresses a stone of `good` (ashlar, quoin or voussoir)
    /// for the site: rock leaves the camp's stock, and the cut stone is laid in
    /// the site's yard as the carts' own would be. Returns the rock it was cut from.
    pub fn hand_dress(&mut self, c: &Catalog, id: u32, good: &str) -> Result<String, String> {
        let yard = self.sites.get(&id).map(|s| s.yard).ok_or("no such site")?;
        let cap = c.plans[&self.buildings.get(&yard).ok_or("the yard is gone")?.plan].input_cap();
        if self.buildings[&yard].input.get(good).copied().unwrap_or(0) >= cap {
            return Err(format!("the yard is full of {good}"));
        }
        let (rock, cost) = self
            .rock_in_camp(c)
            .into_iter()
            .map(|(r, n)| (Self::dress_cost(&r), r, n))
            .find(|(cost, _, n)| n >= cost)
            .map(|(cost, r, _)| (r, cost))
            .ok_or("no rough stone in the camp")?;
        take(&mut self.stock, &Stock::from([(rock.clone(), cost)]));
        add(&mut self.buildings.get_mut(&yard).expect("checked above").input, good, 1);
        self.rev = self.rev.saturating_add(1);
        Ok(rock)
    }
}
