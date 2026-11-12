//! Factory adapter for the pure power graph. The graph never sees inventory;
//! this module supplies explicit source activity and spends local fuel once.
use crate::economy::{covers, take};
use crate::*;

impl Kingdom {
    pub fn source_ready(&self, c: &Catalog, b: &Building) -> bool {
        c.plans
            .get(&b.plan)
            .and_then(|p| p.power.as_ref())
            .is_some_and(|p| {
                p.is_source()
                    && b.construction == 0
                    && !b.paused
                    && b.power_enabled
                    && (p.fuel.is_empty() || b.fuel_left > 0 || covers(&b.input, &p.fuel))
            })
    }
    /// Transport may stock a source ahead of its next charge. This is a fuel
    /// demand, not a production recipe, and must not be consumed by work().
    pub fn source_fuel(&self, c: &Catalog, b: &Building) -> Option<Stock> {
        let p = c.plans.get(&b.plan)?.power.as_ref()?;
        (p.is_source() && b.power_enabled).then(|| p.fuel.clone())
    }
    fn energy_nodes(&self, c: &Catalog) -> Vec<PowerNode> {
        let connected = self.connected();
        let people = self.population.report_with(&self.settlement_sites(c), &crate::population::Rules::of(c, self.world.is_hillside()));
        self.buildings
            .iter()
            .filter_map(|(&id, b)| {
                let plan = c.plans.get(&b.plan)?;
                let p = plan.power.as_ref()?;
                if b.construction > 0 || (p.is_consumer() && !b.power_upgrade) {
                    return None;
                }
                let staffed = people.assigned.get(&id).copied().unwrap_or(0) >= plan.workers;
                let running = !b.paused && b.power_enabled && connected.contains(&id) && staffed;
                let role = if p.is_source() {
                    PowerRole::Source {
                        capacity: p.capacity,
                        active: running && (p.fuel.is_empty() || b.fuel_left > 0),
                    }
                } else {
                    // Readiness does not call report(): population assignment and
                    // power allocation can therefore use the same recipe facts. A lift up
                    // the Great Shaft has no recipe: it is ready whenever it is manned.
                    let ready = plan.shaft > 0 || self.job(c, b).is_some_and(|(input, output, _)| {
                        covers(&b.input, &input)
                            && !self.target_met(b, &output)
                            && output
                                .iter()
                                .all(|(r, n)| c.resources[r].waste || b.output.get(r).copied().unwrap_or(0) + n <= 24)
                    });
                    PowerRole::Consumer {
                        demand: p.demand,
                        enabled: running && ready,
                    }
                };
                Some(PowerNode {
                    id,
                    tile: b.tile,
                    kind: p.kind,
                    priority: b.priority,
                    role,
                })
            })
            .collect()
    }
    pub fn energy_report(&self, c: &Catalog) -> PowerReport {
        self.power
            .evaluate(&self.energy_nodes(c), self.world.bounds())
            .unwrap_or_default()
    }
    /// The command/snapshot validator calls this in addition to ordinary
    /// building validation. Reports remain harmless for rejected input saves.
    pub fn energy_valid(&self, c: &Catalog) -> bool {
        self.power.valid(self.world.bounds())
            && PowerKind::ALL.into_iter().all(|kind| {
                self.power.connectors(kind).iter().all(|&tile| {
                    self.world.owned(tile)
                        && (self.world.terrain(tile).walkable() || self.bridges.contains(&tile))
                        && self.at(tile).is_none()
                })
            })
            && self.buildings.values().all(|b| {
                let Some(plan) = c.plans.get(&b.plan) else {
                    return false;
                };
                match plan.power.as_ref() {
                    None => !b.power_upgrade && b.fuel_left == 0,
                    Some(p) if p.is_source() => !b.power_upgrade && b.fuel_left <= p.fuel_seconds,
                    Some(_) => b.fuel_left == 0,
                }
            })
    }
    pub fn energy_multiplier(&self, c: &Catalog, id: u32, report: &PowerReport) -> u16 {
        let Some(b) = self.buildings.get(&id) else {
            return 0;
        };
        let Some(p) = c.plans.get(&b.plan).and_then(|p| p.power.as_ref()) else {
            return 100;
        };
        if p.is_source() || !b.power_upgrade || !b.power_enabled {
            return 100;
        }
        if report.powered(p.kind, id) {
            p.multiplier
        } else {
            0
        }
    }
    pub(crate) fn energy_tick(&mut self, c: &Catalog) {
        let connected = self.connected();
        let people = self.population.report_with(&self.settlement_sites(c), &crate::population::Rules::of(c, self.world.is_hillside()));
        for (&id, b) in &mut self.buildings {
            let Some(plan) = c.plans.get(&b.plan) else {
                continue;
            };
            let Some(p) = plan.power.as_ref().filter(|p| p.is_source()) else {
                continue;
            };
            if b.construction > 0
                || b.paused
                || !b.power_enabled
                || !connected.contains(&id)
                || people.assigned.get(&id).copied().unwrap_or(0) < plan.workers
                || p.fuel.is_empty()
            {
                continue;
            }
            // Consume a prior interval, then charge the next one. In
            // particular a one-second fuel charge is not spent and erased
            // before the economy gets to use its one powered work interval.
            b.fuel_left = b.fuel_left.saturating_sub(1);
            if b.fuel_left == 0 && covers(&b.input, &p.fuel) {
                take(&mut b.input, &p.fuel);
                b.fuel_left = p.fuel_seconds;
            }
        }
    }
    pub(crate) fn enable_power(&mut self, c: &Catalog, id: u32) -> Result<String, String> {
        let b = self
            .buildings
            .get(&id)
            .ok_or("No building at this power control.")?;
        let p = c
            .plans
            .get(&b.plan)
            .and_then(|p| p.power.as_ref())
            .ok_or("This workshop has no powered upgrade.")?;
        let source = p.is_source();
        if source || b.power_upgrade {
            let b = self.buildings.get_mut(&id).unwrap();
            b.power_enabled = !b.power_enabled;
            return Ok(match (source, b.power_enabled) {
                (true, true) => "Power source enabled.",
                (true, false) => "Power source stopped; stored fuel is kept.",
                (false, true) => "Powered work enabled.",
                (false, false) => "Manual work enabled; the upgrade is kept.",
            }
            .into());
        }
        if !covers(&self.stock, &p.upgrade_cost) {
            return Err(format!("Powered upgrade needs {}.", label(&p.upgrade_cost)));
        }
        take(&mut self.stock, &p.upgrade_cost);
        let b = self.buildings.get_mut(&id).unwrap();
        b.power_upgrade = true;
        b.power_enabled = true;
        Ok(format!(
            "{} upgrade installed. Connect its {} network.",
            p.kind.name(),
            p.kind.connector().to_lowercase()
        ))
    }
    pub fn check_power_site(&self, kind: PowerKind, tile: Tile) -> Result<(), String> {
        if !self.world.owned(tile)
            || !(self.world.terrain(tile).walkable() || self.bridges.contains(&tile))
            || self.at(tile).is_some()
        {
            return Err("Lay power on owned land or a bridge, beside buildings.".into());
        }
        if self.power.contains(kind, tile) {
            return Err("A connector of this kind is already laid here.".into());
        }
        if self.power.connectors(kind).len() >= crate::power::MAX_CONNECTORS {
            return Err("Power connector limit reached.".into());
        }
        Ok(())
    }
    pub(crate) fn lay_power(
        &mut self,
        c: &Catalog,
        kind: PowerKind,
        tile: Tile,
    ) -> Result<String, String> {
        self.check_power_site(kind, tile)?;
        let plan = c
            .plans
            .values()
            .find(|p| p.connector == Some(kind))
            .ok_or("This catalogue has no connector of that kind.")?;
        if !plan.requires.iter().all(|r| self.researched.contains(r)) {
            return Err(format!(
                "Research {} before laying {}.",
                plan.requires.join(", "),
                kind.connector().to_lowercase()
            ));
        }
        let mut grid = self.power.clone();
        grid.lay(kind, tile, self.world.bounds())?;
        let cost = &plan.cost;
        if !covers(&self.stock, &cost) {
            return Err(format!(
                "A {} needs {}.",
                kind.connector().to_lowercase(),
                label(cost)
            ));
        }
        take(&mut self.stock, cost);
        self.power = grid;
        Ok(format!(
            "{} laid. Roads can share this plot.",
            kind.connector()
        ))
    }
    pub(crate) fn remove_power(&mut self, kind: PowerKind, tile: Tile) -> Result<String, String> {
        if !self.power.remove(kind, tile) {
            return Err("No connector of this kind on this plot.".into());
        }
        Ok(format!(
            "{} removed. Buildings and roads remain.",
            kind.connector()
        ))
    }
}

#[cfg(test)]
#[path = "energy_tests.rs"]
mod tests;
