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
        let mut open: Vec<_> = (0..HEIGHT)
            .flat_map(|y| (0..WIDTH).map(move |x| (x, y)))
            .map(|tile| {
                self.world.owned(tile)
                    && (self.world.terrain(tile).walkable() || self.bridges.contains(&tile))
            })
            .collect();
        for b in self.buildings.values() {
            if !c.plans[&b.plan].passable {
                open[(b.tile.1 * WIDTH + b.tile.0) as usize] = false;
            }
        }
        let spawn = crate::economy::neighbours(CAMP)
            .into_iter()
            .filter(|&(x, y)| crate::economy::inside((x, y)) && open[(y * WIDTH + x) as usize])
            .min_by_key(|t| (!self.roads.contains(t), t.1.abs_diff(CAMP.1 + 1), *t))
            .or_else(|| {
                open.iter()
                    .position(|&v| v)
                    .map(|i| ((i % WIDTH as usize) as i16, (i / WIDTH as usize) as i16))
            })
            .ok_or("Keep at least one open village plot")?;
        let nav = Navigation {
            width: WIDTH,
            height: HEIGHT,
            walkable: open,
            roads: self.roads.clone(),
            spawn,
        };
        let mut reached = std::collections::BTreeSet::new();
        let mut queue = std::collections::VecDeque::from([spawn]);
        while let Some(tile) = queue.pop_front() {
            if !reached.insert(tile) {
                continue;
            }
            for next in crate::economy::neighbours(tile) {
                if nav.open(next) && !reached.contains(&next) {
                    queue.push_back(next);
                }
            }
        }
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
                    .filter(|t| reached.contains(t))
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
                let ready = self.source_ready(c, b)
                    || self.job(c, b).is_some_and(|(input, output, _)| {
                        covers(&b.input, &input)
                            && !self.target_met(b, &output)
                            && output
                                .iter()
                                .all(|(r, n)| b.output.get(r).copied().unwrap_or(0) + n <= 24)
                    });
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
                    } else if ready {
                        p.workers
                    } else {
                        0
                    },
                    beds: if constructing { 0 } else { p.housing },
                    online,
                    // Brief construction tasks borrow labour ahead of ordinary
                    // production; the player can also raise workshop priorities.
                    priority: if constructing {
                        b.priority.saturating_add(1).min(2)
                    } else {
                        b.priority
                    },
                }
            })
            .collect()
    }
}
