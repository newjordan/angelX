//! Moving a valley village onto the hillside. The realm is one map (the Works,
//! `hillside`): the blank-slate plan's knights walk the 240 x 176 slope, so a
//! save whose industry was founded on the old 72 x 54 valley meadow (layout 1)
//! is brought onto it. Everything the village owns comes with it (stock, charters,
//! claims, people, what it has made and delivered) and every building stands
//! again on the nearest ground the hillside lets it have to where it stood
//! beside the camp, finished, joined to the camp's road, with its recipe, its
//! claim and its priority. What the valley had and the hillside does not
//! (power shafts and leylines, the valley's own works) are left behind and said.
use crate::economy::{add, take};
use crate::*;
use std::collections::BTreeMap;

/// Where the valley's founding camp stood, and so the offset to the hillside's.
const VALLEY_CAMP: Tile = crate::economy::CAMP;

/// What a migration did, for the words and the tests.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Moved {
    /// Buildings that stand on the hillside again, and where each went (old tile to new).
    pub placed: Vec<(Tile, Tile)>,
    /// Buildings the hillside has no place for (a work of the valley only, or no ground).
    pub left: Vec<(String, Tile)>,
    pub roads: usize,
    /// What the works left behind cost, given back to the camp's stock: nothing the
    /// village paid for is lost with them.
    pub refund: Stock,
}

impl Kingdom {
    /// Whether this kingdom is a valley village that the hillside should take in.
    pub fn is_valley_village(&self) -> bool {
        !self.is_empty() && !self.world.is_hillside()
    }

    /// Bring a valley village onto the hillside, in place. A kingdom that is
    /// empty or already the hillside is left alone.
    pub fn migrate_to_hillside(&mut self, c: &Catalog, seed: u64) -> Result<Moved, String> {
        if !self.is_valley_village() {
            return Err("only a valley village moves onto the hillside".into());
        }
        let old = self.clone();
        let mut fresh = Kingdom::default();
        fresh.found_hillside(c, seed);
        if fresh.is_empty() {
            return Err("the hillside could not be founded".into());
        }
        // The village's wealth: what it held, and what the founding stock of the
        // hillside brings besides (the oxen, the carts and the camp's first goods).
        for (r, n) in &old.stock {
            if c.resources.contains_key(r) {
                add(&mut fresh.stock, r, *n);
            }
        }
        fresh.researched = old.researched.iter().filter(|t| c.tech(t).is_some()).cloned().collect();
        fresh.claims = old.claims.clone();
        fresh.secured_resources = old.secured_resources.clone();
        fresh.produced = old.produced.clone();
        fresh.delivered = old.delivered.clone();
        fresh.provision = old.provision;
        fresh.food_reserve = old.food_reserve;
        fresh.survey_goal = None;
        fresh.second = old.second;
        fresh.shift = old.shift;
        // The people come as they are, unhoused and unemployed: the hillside's posts
        // and beds are counted again at the next second.
        for (id, citizen) in &old.population.citizens {
            let mut citizen = citizen.clone();
            citizen.home = None;
            citizen.job = None;
            citizen.waiting = None;
            // (The founders the hillside brings are the village's own again, not a second set.)
            fresh.population.citizens.insert(*id, citizen);
        }
        fresh.population.next = old.population.next.max(fresh.population.next);
        fresh.population.seconds = old.population.seconds;
        fresh.population.meal_seconds = old.population.meal_seconds;
        fresh.population.arrival_seconds = old.population.arrival_seconds;
        let rules = crate::population::Rules::of(c, true);
        let _ = &rules;

        let offset = (fresh.camp().0 - VALLEY_CAMP.0, fresh.camp().1 - VALLEY_CAMP.1);
        let mut moved = Moved::default();
        let mut ids: BTreeMap<u32, u32> = BTreeMap::new();
        ids.insert(1, 1);
        // Buildings nearest the camp first, so the village keeps its shape round it.
        let mut order: Vec<(&u32, &Building)> = old.buildings.iter().filter(|(id, _)| **id != 1).collect();
        order.sort_by_key(|(id, b)| ((b.tile.0 - VALLEY_CAMP.0).abs() + (b.tile.1 - VALLEY_CAMP.1).abs(), **id));
        for (&old_id, b) in order {
            let Some(p) = c.plans.get(&b.plan).filter(|p| p.hillside && p.connector.is_none() && p.replaces.is_none()) else {
                moved.left.push((b.plan.clone(), b.tile));
                Self::give_back(c, b, &mut fresh, &mut moved.refund);
                continue;
            };
            let want = (b.tile.0 + offset.0, b.tile.1 + offset.1);
            let Some(tile) = fresh.nearest_ground(c, &b.plan, want, 45) else {
                moved.left.push((b.plan.clone(), b.tile));
                Self::give_back(c, b, &mut fresh, &mut moved.refund);
                continue;
            };
            let id = fresh.next;
            fresh.next += 1;
            let (span, depth) = fresh.dims(p);
            let patch = fresh.patch_for(c, p, tile).ok().flatten();
            fresh.buildings.insert(
                id,
                Building {
                    plan: b.plan.clone(),
                    tile,
                    recipe: b.recipe.clone().filter(|r| p.recipes.contains(r)).or_else(|| p.recipes.first().cloned()),
                    claim: None,
                    paused: b.paused,
                    priority: b.priority,
                    production_limit: b.production_limit,
                    power_upgrade: false,
                    power_enabled: true,
                    span,
                    depth,
                    patch,
                    ..Building::default()
                },
            );
            // What it held goes back to the camp's stock (the buildings' stores are
            // not the hillside's to carry over).
            for (r, n) in b.input.iter().chain(&b.output) {
                add(&mut fresh.stock, r, *n);
            }
            ids.insert(old_id, id);
            moved.placed.push((b.tile, tile));
        }
        // Mines keep their claims.
        for (&old_id, b) in &old.buildings {
            if let (Some(claim), Some(&new)) = (&b.claim, ids.get(&old_id)) {
                if fresh.claims.contains_key(claim) && c.plans[&fresh.buildings[&new].plan].extractor {
                    fresh.buildings.get_mut(&new).expect("placed").claim = Some(claim.clone());
                }
            }
        }
        // The roads: each building joined to the camp's network the short way, at no
        // cost (the village already paid for its roads once).
        let walkers: Vec<u32> = ids.values().copied().filter(|id| *id != 1).collect();
        for id in walkers {
            if let Ok(tiles) = fresh.plan_connection(c, id) {
                for t in tiles {
                    fresh.world.clear_forest(t);
                    if fresh.roads.insert(t) {
                        moved.roads += 1;
                    }
                }
            }
        }
        // (A shaft's lift is not valley work: the hillside brings its own.)
        let _ = take;
        fresh.rev = old.rev.saturating_add(1);
        fresh.catalog = c.id.clone();
        fresh.sync_citizens(c, false).map_err(|e| format!("the village could not be settled on the hillside: {e}"))?;
        // Housing and posts are counted again.
        let report = fresh.report(c);
        let _ = report;
        if !fresh.valid(c) {
            return Err("the migrated village is not a valid kingdom".into());
        }
        *self = fresh;
        Ok(moved)
    }

    /// A work left behind: its goods and what it cost go back to the camp's stock.
    fn give_back(c: &Catalog, b: &Building, fresh: &mut Kingdom, refund: &mut Stock) {
        for (r, n) in b.input.iter().chain(&b.output) {
            add(&mut fresh.stock, r, *n);
        }
        if let Some(p) = c.plans.get(&b.plan) {
            for (r, n) in p.cost.iter().filter(|(r, _)| c.resources.contains_key(*r)) {
                add(&mut fresh.stock, r, *n);
                add(refund, r, *n);
            }
        }
    }

    /// The nearest tile to `want`, within `reach` tiles, where a `plan` may be raised
    /// now: land that suits it, clear, off the roads. A ring at a time.
    pub fn nearest_ground(&self, c: &Catalog, plan: &str, want: Tile, reach: i16) -> Option<Tile> {
        for r in 0..=reach {
            for dy in -r..=r {
                for dx in -r..=r {
                    if dx.abs().max(dy.abs()) != r {
                        continue;
                    }
                    let t = (want.0 + dx, want.1 + dy);
                    if self.inside(t) && self.placement(c, plan, t).land.is_ok() && self.footprint(&c.plans[plan], t).iter().all(|u| !self.roads.contains(u)) {
                        return Some(t);
                    }
                }
            }
        }
        None
    }
}
