//! What wears out, runs out, goes off and eats: tools, woods, mortar, fodder.
//! Nothing here invents goods. It only takes them away on a clock, so a site
//! has to keep its smith, its coppice and its mill working to keep standing.
use crate::economy::{covers, take};
use crate::hillside;
use crate::*;
use std::collections::BTreeSet;

/// A small deterministic mix of a number (SplitMix64), for the draws that must agree everywhere.
fn mix(mut n: u64) -> u64 {
    n = n.wrapping_add(0x9e37_79b9_7f4a_7c15);
    n = (n ^ (n >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
    n = (n ^ (n >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
    n ^ (n >> 31)
}

/// Tiles from a carter's yard within which he can fetch hay by hand.
const HAY_REACH: i16 = 8;

/// The tiles of a building whose top-left corner is `tile`, `span` more to the
/// east and `depth` more to the south: the corner first, then its row, then the
/// rows below. (Its door is the south side; a road on any side joins it.)
pub fn rect_of(tile: Tile, span: u8, depth: u8) -> impl Iterator<Item = Tile> {
    (0..=i16::from(depth)).flat_map(move |dy| (0..=i16::from(span)).map(move |dx| (tile.0 + dx, tile.1 + dy)))
}

impl Kingdom {
    /// What stops a post working beyond crew and inputs: a wood or outcrop
    /// with nothing left, no tool to put in a worker's hand, no fodder.
    pub(crate) fn post_blocked(&self, c: &Catalog, b: &Building) -> Option<Status> {
        let p = &c.plans[&b.plan];
        let patch = b.recipe.as_ref().and_then(|r| c.recipes[r].patch.as_ref());
        if patch.is_some_and(|pt| !self.patch_ok(b, pt)) {
            return Some(Status::Depleted);
        }
        if !p.tool.is_empty() && b.tool_left == 0 && self.amount(&p.tool) == 0 {
            return Some(Status::Tools);
        }
        if !p.upkeep.is_empty() && b.fed == 0 && !covers(&self.stock, &p.upkeep) {
            return Some(Status::Fuel);
        }
        None
    }

    /// Whether the patch a building works can give a batch. Buildings with no
    /// patch (the valley's, or ones built before patches) never run out.
    pub(crate) fn patch_ok(&self, b: &Building, patch: &Patch) -> bool {
        let Some(id) = &b.patch else { return true };
        let left = self.nature.get(id).copied().unwrap_or(0);
        if patch.takes > 0 {
            left >= patch.takes
        } else {
            u64::from(left) * 100 >= u64::from(hillside::patch_capacity(id)) * u64::from(patch.min_percent)
        }
    }

    /// The wood or outcrop a new building will work, if its recipes draw on one.
    pub(crate) fn patch_for(&self, c: &Catalog, p: &Plan, tile: Tile) -> Result<Option<String>, String> {
        if !self.world.is_hillside() {
            return Ok(None);
        }
        let kind = p
            .recipes
            .iter()
            .find_map(|r| c.recipes[r].patch.as_ref())
            .map(|pt| pt.kind.as_str());
        match kind {
            None => Ok(None),
            Some(k) => hillside::nearest_patch(k, tile, 3)
                .map(Some)
                .ok_or_else(|| format!("no {k} patch within three plots of that site")),
        }
    }

    /// The tiles a building of this plan would cover at `tile`: its top-left
    /// corner first, then the row east of it, then the rows to the south.
    pub fn footprint(&self, p: &Plan, tile: Tile) -> Vec<Tile> {
        let (span, depth) = self.dims(p);
        rect_of(tile, span, depth).collect()
    }

    /// How many tiles a plan runs to the east and to the south in this world: the
    /// hillside's buildings have the footprints their pictures are drawn on; the
    /// valley's are one tile.
    pub fn dims(&self, p: &Plan) -> (u8, u8) {
        if !self.world.is_hillside() {
            (0, 0)
        } else {
            (p.span, p.depth)
        }
    }

    /// The tiles round a building that a road can touch it from.
    pub fn docks_of(&self, b: &Building) -> Vec<Tile> {
        let own: BTreeSet<Tile> = self.covered(b).collect();
        let mut out = BTreeSet::new();
        for t in &own {
            for n in crate::economy::neighbours(*t) {
                if !own.contains(&n) {
                    out.insert(n);
                }
            }
        }
        out.into_iter().collect()
    }

    /// Every tile a placed building covers.
    pub fn covered(&self, b: &Building) -> impl Iterator<Item = Tile> + '_ {
        rect_of(b.tile, b.span, b.depth)
    }

    /// A long building needs all its tiles clear.
    pub(crate) fn check_footprint(&self, c: &Catalog, plan: &str, tile: Tile) -> Result<(), String> {
        let p = &c.plans[plan];
        for t in self.footprint(p, tile).into_iter().skip(1) {
            if !self.inside(t)
                || self.at(t).is_some()
                || self.roads.contains(&t)
                || self.rollers.cells.contains_key(&t)
                || self.rails.cells.contains_key(&t)
                || t == self.gate()
                || t == self.camp()
                || !self.world.terrain(t).walkable()
                || self.reserved_ground(t).is_some()
            {
                return Err(format!(
                    "a {} needs {} by {} clear plots",
                    p.name.to_lowercase(),
                    p.span + 1,
                    p.depth + 1
                ));
            }
        }
        Ok(())
    }

    /// Mortar sets, bread stales: every perishable good, wherever it stands,
    /// loses a share each shift, so a mill must stand near its wall.
    pub(crate) fn spoil(&mut self, c: &Catalog) {
        let perishable: Vec<(&String, u32)> = c
            .resources
            .iter()
            .filter(|(_, r)| r.shelf > 0)
            .map(|(k, r)| (k, (r.shelf / 10).max(1)))
            .collect();
        if perishable.is_empty() {
            return;
        }
        let now = self.shift;
        for (r, shifts) in perishable {
            let mut lost = 0;
            // A share of `1 / shifts` of what a store holds, to the unit: the whole part every
            // shift, and the remainder as a chance of one more, drawn from the shift and the store
            // so that it is the same on every machine. (It used to round up, so a store holding
            // seven sacks lost one every shift as surely as one holding ninety, and more than half
            // of every batch of mortar went off before the masons could use it.)
            let mut go = |store: &mut Stock, salt: u64| {
                if let Some(n) = store.get_mut(r) {
                    let (held, span) = (u64::from(*n), u64::from(shifts));
                    let chance = mix(now ^ salt.wrapping_mul(0x9e37_79b9_7f4a_7c15) ^ (span << 40)) % span;
                    let loss = (held / span + u64::from(chance < held % span)) as u32;
                    let loss = loss.min(*n);
                    *n -= loss;
                    lost += loss;
                }
                store.retain(|_, n| *n > 0);
            };
            go(&mut self.stock, 0);
            for (&id, b) in self.buildings.iter_mut() {
                go(&mut b.input, u64::from(id) * 2 + 1);
                go(&mut b.output, u64::from(id) * 2 + 2);
            }
            if lost > 0 {
                crate::economy::capped_add(&mut self.spoiled, r, lost);
            }
        }
    }

    /// Woods and the outcrop regrow while they are over a quarter full, so
    /// plain felling can finish them and coppicing cannot.
    pub(crate) fn regrow(&mut self, c: &Catalog) {
        let by = c.works.regrow;
        for (id, cap) in hillside::patches() {
            if let Some(left) = self.nature.get_mut(&id) {
                if u64::from(*left) * 100 >= u64::from(cap) * 25 && *left < cap && id != "field" {
                    *left = (*left + by).min(cap);
                }
            }
        }
    }

    /// Fodder and the like, drawn from the camp every so often by buildings
    /// and by lines (oxen). What cannot be paid for stops its work.
    pub(crate) fn draw_upkeep(&mut self, c: &Catalog) {
        let mut stock = std::mem::take(&mut self.stock);
        let mut hungry: Vec<(u32, Tile, Stock)> = Vec::new();
        for (&id, b) in self.buildings.iter_mut() {
            let p = &c.plans[&b.plan];
            if p.stalls > 0 {
                // The oxen in a carter's yard eat what has been brought to it,
                // else what the carter fetches by hand from the camp.
                if b.construction > 0 || b.paused || b.carts.is_empty() {
                    continue;
                }
                b.fed = b.fed.saturating_sub(1);
                if b.fed == 0 {
                    let spec = &c.lines["cart"];
                    let need: Stock = spec.upkeep.iter().map(|(r, n)| (r.clone(), n * b.carts.len() as u32)).collect();
                    if covers(&b.input, &need) {
                        take(&mut b.input, &need);
                        b.fed = spec.upkeep_seconds.max(1);
                    } else if covers(&stock, &need) {
                        take(&mut stock, &need);
                        b.fed = spec.upkeep_seconds.max(1);
                    } else {
                        hungry.push((id, b.tile, need));
                    }
                }
                continue;
            }
            if p.upkeep.is_empty() || b.construction > 0 || b.paused {
                continue;
            }
            b.fed = b.fed.saturating_sub(1);
            if b.fed == 0 && covers(&stock, &p.upkeep) {
                take(&mut stock, &p.upkeep);
                b.fed = p.upkeep_seconds.max(1);
            }
        }
        for l in self.lines.values_mut() {
            let spec = &c.lines[l.kind.key()];
            if spec.upkeep.is_empty() {
                continue;
            }
            l.fed = l.fed.saturating_sub(1);
            if l.fed == 0 {
                let need: Stock = spec.upkeep.iter().map(|(r, n)| (r.clone(), n * l.fleet.len() as u32)).collect();
                if covers(&stock, &need) {
                    take(&mut stock, &need);
                    l.fed = spec.upkeep_seconds.max(1);
                } else if let Some(b) = self.buildings.get_mut(&l.from).filter(|b| covers(&b.output, &need)) {
                    // Oxen on the road from a farm eat the farm's own hay: the cart that
                    // carries fodder to the camp is never starved of it.
                    take(&mut b.output, &need);
                    l.fed = spec.upkeep_seconds.max(1);
                }
            }
        }
        self.stock = stock;
        // A yard with no fodder of its own and none in the camp: the carter takes hay by
        // hand from a barn or a farm within a few tiles, so a farm's own hay can feed the
        // oxen that carry it.
        for (id, tile, need) in hungry {
            let spec = &c.lines["cart"];
            let source = self
                .buildings
                .iter()
                .filter(|&(&o, b)| {
                    o != id
                        && (b.tile.0 - tile.0).abs() + (b.tile.1 - tile.1).abs() <= HAY_REACH
                        && covers(&b.output, &need)
                })
                .map(|(&o, _)| o)
                .next();
            if let Some(o) = source {
                take(&mut self.buildings.get_mut(&o).unwrap().output, &need);
                self.buildings.get_mut(&id).unwrap().fed = spec.upkeep_seconds.max(1);
            }
        }
    }

    /// The patches a new hillside begins with.
    pub(crate) fn found_nature(&mut self) {
        for (id, cap) in hillside::patches() {
            self.nature.insert(id, cap);
        }
    }

    /// A shaft lift works only while some quarry waits to be served by it, and
    /// nothing stops the post.
    pub(crate) fn shaft_ready(&self, c: &Catalog, b: &Building) -> bool {
        self.shaft_posted(c, b) && self.post_blocked(c, b).is_none()
    }

    /// A shaft lift has a post while some quarry waits to be served by it.
    pub(crate) fn shaft_posted(&self, c: &Catalog, b: &Building) -> bool {
        let p = &c.plans[&b.plan];
        p.shaft > 0
            && self.buildings.values().any(|q| {
                q.construction == 0
                    && c.plans[&q.plan].extractor
                    && q.claim.as_ref().and_then(|id| self.claims.get(id)).is_some_and(|cl| cl.secured)
            })
    }

    /// A teacher brings someone one level on in a trade, for coin, never past
    /// master and never two levels at once.
    pub(crate) fn teach(&mut self, c: &Catalog, citizen: u32, trade: &str) -> Result<String, String> {
        let rules = crate::population::Rules::of(c, self.world.is_hillside());
        if !self.world.is_hillside() || !c.trades.contains_key(trade) {
            return Err("there is no teacher for that here".into());
        }
        let now = self.population.citizens.get(&citizen).ok_or("no such person")?.skill(trade, &rules);
        if now >= 3 {
            return Err("they are already a master".into());
        }
        let fee = c.works.tuition * u32::from(now + 1);
        let cost = Stock::from([("coin".to_string(), fee)]);
        if !covers(&self.stock, &cost) {
            return Err(format!("need {fee} coin for the teacher"));
        }
        take(&mut self.stock, &cost);
        self.population.grant(citizen, trade, now + 1, &rules);
        Ok(format!("taught a {} lesson", trade))
    }

    /// Raise a building in place to the plan that replaces it.
    pub(crate) fn upgrade(&mut self, c: &Catalog, id: u32) -> Result<String, String> {
        let b = self.buildings.get(&id).ok_or("no building")?;
        if b.construction > 0 {
            return Err("it is still being built".into());
        }
        let (key, next) = c
            .plans
            .iter()
            .find(|(_, p)| p.replaces.as_deref() == Some(b.plan.as_str()))
            .ok_or("there is nothing better to raise it to")?;
        if !next.requires.iter().all(|r| self.researched.contains(r)) {
            return Err(format!("research {} first", next.requires.join(", ")));
        }
        if !covers(&self.stock, &next.cost) {
            return Err(format!("need {}", label(&next.cost)));
        }
        take(&mut self.stock, &next.cost);
        let dims = self.dims(next);
        let b = self.buildings.get_mut(&id).unwrap();
        b.plan = key.clone();
        b.construction = next.build_seconds;
        b.recipe = next.recipes.first().cloned();
        b.progress = 0;
        (b.span, b.depth) = dims;
        b.power_upgrade = false;
        Ok(format!("{} begun", next.name))
    }
}
