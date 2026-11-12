//! Converts factory sites to individual jobs and homes without coupling the
//! population module to recipes, roads, build plans or the renderer.
use crate::economy::covers;
use crate::population::{Role, Site};
use crate::*;

impl Kingdom {
    pub fn reserved_food(&self) -> u32 {
        self.population.citizens.len() as u32 * u32::from(self.food_reserve.unwrap_or(2))
    }

    pub fn food_delivery_target(&self, c: &Catalog) -> u32 {
        let reserve = self.reserved_food();
        if self.food_reserve == Some(0) {
            return 0;
        }
        let connected = self.connected();
        let beds: u32 = self
            .buildings
            .iter()
            .filter(|(id, b)| connected.contains(id) && !b.paused && b.construction == 0)
            .map(|(_, b)| u32::from(c.plans[&b.plan].housing))
            .sum();
        let people = self.population.citizens.len() as u32;
        // A working kitchen must not continually consume the provisions that
        // a new resident needs during the arrival countdown.
        // Arrival takes longer than a meal interval: its provisions must
        // survive the next dinner while the kitchen takes surplus harvest.
        if beds > people {
            reserve.max((people + 1) * 2 + people)
        } else {
            reserve
        }
    }

    pub(crate) fn sync_citizens(&mut self, c: &Catalog, advance: bool) -> Result<(), String> {
        use crate::citizen_motion::{Destination, Navigation};
        let (width, height) = self.world.bounds();
        let w = width as usize;
        let camp = self.camp();
        let mut open: Vec<bool> = self.world.walkable_grid();
        for &t in &self.bridges {
            if self.world.owned(t) {
                open[t.1 as usize * w + t.0 as usize] = true;
            }
        }
        for b in self.buildings.values() {
            if !c.plans[&b.plan].passable {
                for t in self.covered(b) {
                    open[t.1 as usize * w + t.0 as usize] = false;
                }
            }
        }
        let spawn = crate::economy::neighbours(camp)
            .into_iter()
            .filter(|&(x, y)| self.inside((x, y)) && open[y as usize * w + x as usize])
            .min_by_key(|t| (!self.roads.contains(t), t.1.abs_diff(camp.1 + 1), *t))
            .or_else(|| {
                open.iter()
                    .position(|&v| v)
                    .map(|i| ((i % w) as i16, (i / w) as i16))
            })
            .ok_or("Keep at least one open village plot")?;
        let nav = Navigation {
            width,
            height,
            walkable: open,
            roads: self.roads.clone(),
            spawn,
        };
        // Every tile a person can walk to from the spawn, on a flat grid so a
        // whole hillside costs a few microseconds a second, not a few ms.
        let mut reached = vec![false; nav.walkable.len()];
        let mut queue = std::collections::VecDeque::from([spawn]);
        reached[spawn.1 as usize * w + spawn.0 as usize] = true;
        while let Some(tile) = queue.pop_front() {
            for next in crate::economy::neighbours(tile) {
                if nav.open(next) && !reached[next.1 as usize * w + next.0 as usize] {
                    reached[next.1 as usize * w + next.0 as usize] = true;
                    queue.push_back(next);
                }
            }
        }
        let reachable = |t: Tile| nav.open(t) && reached[t.1 as usize * w + t.0 as usize];
        // Employment keeps a person at a stalled machine, but its work pose
        // follows actual production readiness (including available power).
        // report() projects settlement sites; it never synchronizes motion.
        let report = self.report(c);
        let destinations: Vec<_> = report
            .population
            .citizens
            .iter()
            .map(|p| {
                let tile = crate::economy::neighbours(p.target)
                    .into_iter()
                    .filter(|t| reachable(*t))
                    .min_by_key(|t| (!self.roads.contains(t), t.1.abs_diff(p.target.1 + 1), *t))
                    .unwrap_or(spawn);
                Destination {
                    citizen: p.id,
                    tile,
                    working: matches!(p.activity, crate::population::Activity::Working)
                        && p.job.is_some_and(|id| {
                            matches!(report.status.get(&id), Some(Status::Working | Status::Construction))
                        }),
                }
            })
            .collect();
        if advance {
            self.motion.advance_second(&destinations, &nav)
        } else {
            self.motion.sync(&destinations, &nav)
        }
    }

    pub fn settlement_sites(&self, c: &Catalog) -> Vec<Site> {
        let connected = self.connected();
        let access = if self.buildings.values().any(|b| b.construction > 0) {
            self.construction_access(c)
        } else {
            Default::default()
        };
        self.buildings
            .iter()
            .map(|(&id, b)| {
                let p = &c.plans[&b.plan];
                let constructing = b.construction > 0;
                let online = !b.paused
                    && if constructing {
                        access.contains(&id)
                    } else {
                        connected.contains(&id)
                    };
                let ready = (!constructing && self.shaft_ready(c, b))
                    || self.source_ready(c, b)
                    || (b.plan == "lodge" && !constructing && self.lodge_ready(c, b))
                    || (p.crane > 0 && !constructing && self.crane_ready(c, b))
                    || (p.stalls > 0 && !constructing && !b.carts.is_empty())
                    || (p.swape > 0 && !constructing)
                    // A watchtower's archer keeps the post whether or not the quiver is full.
                    || (p.watch > 0 && !constructing)
                    || self.job(c, b).is_some_and(|(input, output, _)| {
                        covers(&b.input, &input)
                            && !self.target_met(b, &output)
                            && output
                                .iter()
                                .all(|(r, n)| p.extractor || c.resources[r].waste || b.output.get(r).copied().unwrap_or(0) + n <= 24)
                            && self.post_blocked(c, b).is_none()
                    });
                // A post with work to do but not the goods for it this minute: on the
                // hillside the people who were at it stay (idle, in sight) until another
                // post can use them. Only a post with nothing left to work, or none,
                // lets them go.
                let waiting = self.world.is_hillside()
                    && !constructing
                    && !ready
                    && !matches!(self.post_blocked(c, b), Some(Status::Depleted))
                    && (self.shaft_posted(c, b)
                        || (b.plan == "lodge" && self.lodge_posted(c, b))
                        || (p.crane > 0 && self.crane_posted(c, b))
                        || (p.stalls > 0 && !b.carts.is_empty())
                        || self.job(c, b).is_some());
                Site {
                    id,
                    tile: b.tile,
                    role: if constructing {
                        Role::Builder
                    } else {
                        match b.plan.as_str() {
                            "farm" | "kitchen" => Role::Farmer,
                            "conduit" | "enchanter" => Role::Arcanist,
                            "forester" | "quarry" | "mine" | "clay-pit" => Role::Porter,
                            _ => Role::Builder,
                        }
                    },
                    jobs: if constructing {
                        1
                    } else if id == 1 {
                        // The camp's labourers are the hand-chain's: a pair of hands to a tile.
                        self.chain_crew(c)
                    } else if ready {
                        p.workers
                    } else {
                        0
                    },
                    beds: if constructing { 0 } else { p.housing },
                    held: if waiting { p.workers } else { 0 },
                    online,
                    // Brief construction tasks borrow labour ahead of ordinary
                    // production; the player can also raise workshop priorities.
                    priority: if constructing {
                        b.priority.saturating_add(1).min(2)
                    } else {
                        b.priority
                    },
                    trade: if constructing || p.trade.is_empty() {
                        None
                    } else {
                        let level = b
                            .recipe
                            .as_ref()
                            .map_or(p.level, |r| c.recipes[r].level.max(p.level));
                        crate::population::trade_index(&p.trade).map(|t| (t, level))
                    },
                }
            })
            .collect()
    }
}
