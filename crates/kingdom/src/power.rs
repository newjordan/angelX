//! Separate mechanical shafts and arcane leylines. This pure graph projection
//! allocates capacity, never fuel, recipe inputs, inventory, progress or time.
//! The command layer owns placement/payment; the economy owns source activity.
use crate::{Catalog, Stock, Tile};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet, VecDeque};

pub const MAX_CONNECTORS: usize = 4096;
pub const MAX_NODES: usize = 512;
pub const MAX_UNITS: u16 = 4096;

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PowerKind {
    Mechanical,
    Arcane,
}
impl PowerKind {
    pub const ALL: [Self; 2] = [Self::Mechanical, Self::Arcane];
    pub fn name(self) -> &'static str {
        match self {
            Self::Mechanical => "Mechanical",
            Self::Arcane => "Arcane",
        }
    }
    pub fn connector(self) -> &'static str {
        match self {
            Self::Mechanical => "Shaft",
            Self::Arcane => "Leyline",
        }
    }
}

/// Catalogue metadata. Consumers are optional purchased upgrades; sources are
/// purpose-built machines. Fuel is a source's local-input cost per charge.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct MachinePower {
    pub kind: PowerKind,
    pub demand: u16,
    pub capacity: u16,
    pub fuel: Stock,
    pub fuel_seconds: u16,
    pub multiplier: u16,
    pub upgrade_cost: Stock,
}
impl Default for MachinePower {
    fn default() -> Self {
        Self {
            kind: PowerKind::Mechanical,
            demand: 0,
            capacity: 0,
            fuel: Stock::new(),
            fuel_seconds: 0,
            multiplier: 100,
            upgrade_cost: Stock::new(),
        }
    }
}
impl MachinePower {
    pub fn is_source(&self) -> bool {
        self.capacity > 0 && self.demand == 0
    }
    pub fn is_consumer(&self) -> bool {
        self.demand > 0 && self.capacity == 0
    }
    pub fn valid(&self, c: &Catalog) -> bool {
        (self.is_source() || self.is_consumer())
            && self.capacity <= MAX_UNITS
            && self.demand <= MAX_UNITS
            && (100..=300).contains(&self.multiplier)
            && c.valid_stock(&self.fuel)
            && self.fuel.values().all(|n| *n <= 24)
            && c.valid_stock(&self.upgrade_cost)
            && if self.is_source() {
                self.upgrade_cost.is_empty()
                    && if self.fuel.is_empty() {
                        self.fuel_seconds == 0
                    } else {
                        (1..=600).contains(&self.fuel_seconds)
                    }
            } else {
                self.fuel.is_empty() && self.fuel_seconds == 0
            }
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct PowerGrid {
    pub shafts: BTreeSet<Tile>,
    pub leylines: BTreeSet<Tile>,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PowerRole {
    /// Capacity remains the installed rating when inactive, so a fuel-starved
    /// source is distinguishable from a physically disconnected consumer.
    Source { capacity: u16, active: bool },
    /// A disabled machine draws nothing, but its physical manifold remains.
    Consumer { demand: u16, enabled: bool },
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PowerNode {
    pub id: u32,
    pub tile: Tile,
    pub kind: PowerKind,
    pub priority: u8,
    pub role: PowerRole,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PowerStatus {
    Powered,
    Disconnected,
    Overloaded,
    Inactive,
}
impl PowerStatus {
    pub fn label(self) -> &'static str {
        match self {
            Self::Powered => "POWERED",
            Self::Disconnected => "NO SOURCE CONNECTED",
            Self::Overloaded => "NOT ENOUGH POWER",
            Self::Inactive => "NOT DRAWING POWER",
        }
    }
}
/// The smallest coordinate in a physical component, independent of insertion
/// order. Components of different power kinds never share an identity.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct PowerComponentId {
    pub kind: PowerKind,
    pub anchor: Tile,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ConsumerPower {
    pub component: PowerComponentId,
    pub status: PowerStatus,
    pub demand: u16,
    pub requested: u16,
    pub allocated: u16,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PowerComponent {
    pub id: PowerComponentId,
    pub connectors: usize,
    pub source_ids: Vec<u32>,
    pub active_sources: u16,
    pub rated_capacity: u32,
    pub active_capacity: u32,
    pub requested: u32,
    pub allocated: u32,
    pub consumers: u16,
    pub enabled_consumers: u16,
}
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct PowerTotals {
    pub sources: u16,
    pub active_sources: u16,
    pub rated_capacity: u32,
    pub active_capacity: u32,
    pub requested: u32,
    pub allocated: u32,
    pub powered: u16,
    pub overloaded: u16,
    pub disconnected: u16,
    pub inactive: u16,
}
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct PowerReport {
    pub consumers: BTreeMap<(PowerKind, u32), ConsumerPower>,
    pub components: BTreeMap<PowerComponentId, PowerComponent>,
    /// Contains both connector and terminal tiles. It is a transient report,
    /// never extra save state that could disagree with the physical network.
    pub membership: BTreeMap<(PowerKind, Tile), PowerComponentId>,
}
impl PowerReport {
    pub fn consumer(&self, kind: PowerKind, id: u32) -> Option<&ConsumerPower> {
        self.consumers.get(&(kind, id))
    }
    pub fn powered(&self, kind: PowerKind, id: u32) -> bool {
        self.consumer(kind, id)
            .is_some_and(|r| r.status == PowerStatus::Powered)
    }
    pub fn component_at(&self, kind: PowerKind, tile: Tile) -> Option<&PowerComponent> {
        self.components.get(self.membership.get(&(kind, tile))?)
    }
    pub fn totals(&self, kind: PowerKind) -> PowerTotals {
        let mut total = PowerTotals::default();
        for component in self.components.values().filter(|c| c.id.kind == kind) {
            total.sources += component.source_ids.len() as u16;
            total.active_sources += component.active_sources;
            total.rated_capacity += component.rated_capacity;
            total.active_capacity += component.active_capacity;
            total.requested += component.requested;
            total.allocated += component.allocated;
        }
        for ((k, _), consumer) in &self.consumers {
            if *k != kind {
                continue;
            }
            match consumer.status {
                PowerStatus::Powered => total.powered += 1,
                PowerStatus::Disconnected => total.disconnected += 1,
                PowerStatus::Overloaded => total.overloaded += 1,
                PowerStatus::Inactive => total.inactive += 1,
            }
        }
        total
    }
}

fn inside((x, y): Tile, (width, height): Tile) -> bool {
    width > 0 && height > 0 && (0..width).contains(&x) && (0..height).contains(&y)
}
fn adjacent((x, y): Tile) -> [Tile; 4] {
    [(x - 1, y), (x + 1, y), (x, y - 1), (x, y + 1)]
}
impl PowerGrid {
    pub fn connectors(&self, kind: PowerKind) -> &BTreeSet<Tile> {
        match kind {
            PowerKind::Mechanical => &self.shafts,
            PowerKind::Arcane => &self.leylines,
        }
    }
    fn connectors_mut(&mut self, kind: PowerKind) -> &mut BTreeSet<Tile> {
        match kind {
            PowerKind::Mechanical => &mut self.shafts,
            PowerKind::Arcane => &mut self.leylines,
        }
    }
    pub fn contains(&self, kind: PowerKind, tile: Tile) -> bool {
        self.connectors(kind).contains(&tile)
    }
    pub fn valid(&self, bounds: Tile) -> bool {
        bounds.0 > 0
            && bounds.1 > 0
            && PowerKind::ALL.into_iter().all(|kind| {
                let wires = self.connectors(kind);
                wires.len() <= MAX_CONNECTORS && wires.iter().all(|&p| inside(p, bounds))
            })
    }
    /// The owner additionally checks charter ownership, occupancy and price.
    /// A repeated command fails without changing the grid or charging it twice.
    pub fn lay(&mut self, kind: PowerKind, tile: Tile, bounds: Tile) -> Result<(), String> {
        if !inside(tile, bounds) {
            return Err("Power connector is outside the valley.".into());
        }
        let wires = self.connectors_mut(kind);
        if wires.contains(&tile) {
            return Err("A connector is already laid here.".into());
        }
        if wires.len() >= MAX_CONNECTORS {
            return Err("Power connector limit reached.".into());
        }
        wires.insert(tile);
        Ok(())
    }
    pub fn remove(&mut self, kind: PowerKind, tile: Tile) -> bool {
        self.connectors_mut(kind).remove(&tile)
    }
    pub fn evaluate(&self, nodes: &[PowerNode], bounds: Tile) -> Result<PowerReport, String> {
        if !self.valid(bounds) || nodes.len() > MAX_NODES {
            return Err("Invalid power network bounds.".into());
        }
        let mut ids = BTreeSet::new();
        let mut places = BTreeSet::new();
        for node in nodes {
            let units = match node.role {
                PowerRole::Source { capacity, .. } => capacity,
                PowerRole::Consumer { demand, .. } => demand,
            };
            if node.id == 0
                || node.id == u32::MAX
                || !inside(node.tile, bounds)
                || node.priority > 2
                || !(1..=MAX_UNITS).contains(&units)
                || !ids.insert((node.kind, node.id))
                || !places.insert((node.kind, node.tile))
                || self.shafts.contains(&node.tile)
                || self.leylines.contains(&node.tile)
            {
                return Err("Invalid or overlapping power terminal.".into());
            }
        }
        let mut report = PowerReport::default();
        for kind in PowerKind::ALL {
            let wires = self.connectors(kind);
            let terminals: BTreeMap<_, _> = nodes
                .iter()
                .filter(|n| n.kind == kind)
                .map(|n| (n.tile, n))
                .collect();
            let vertices: BTreeSet<_> = wires.iter().chain(terminals.keys()).copied().collect();
            let mut visited = BTreeSet::new();
            for &start in &vertices {
                if !visited.insert(start) {
                    continue;
                }
                let id = PowerComponentId {
                    kind,
                    anchor: start,
                };
                let mut queue = VecDeque::from([start]);
                let mut attached = Vec::new();
                let mut component = PowerComponent {
                    id,
                    connectors: 0,
                    source_ids: Vec::new(),
                    active_sources: 0,
                    rated_capacity: 0,
                    active_capacity: 0,
                    requested: 0,
                    allocated: 0,
                    consumers: 0,
                    enabled_consumers: 0,
                };
                while let Some(tile) = queue.pop_front() {
                    report.membership.insert((kind, tile), id);
                    let wire = wires.contains(&tile);
                    if wire {
                        component.connectors += 1;
                    }
                    if let Some(node) = terminals.get(&tile) {
                        attached.push(**node);
                    }
                    for next in adjacent(tile) {
                        // Machines conduct through their internal manifold,
                        // but two adjacent machines still need a connector.
                        if vertices.contains(&next)
                            && (wire || wires.contains(&next))
                            && visited.insert(next)
                        {
                            queue.push_back(next);
                        }
                    }
                }
                for node in &attached {
                    if let PowerRole::Source { capacity, active } = node.role {
                        component.source_ids.push(node.id);
                        component.rated_capacity += u32::from(capacity);
                        if active {
                            component.active_sources += 1;
                            component.active_capacity += u32::from(capacity);
                        }
                    }
                }
                component.source_ids.sort_unstable();
                // Stable IDs resolve equal priorities regardless of caller
                // order, hash implementation, restore or multiplayer timing.
                attached.sort_by_key(|n| (std::cmp::Reverse(n.priority), n.id));
                let mut available = component.active_capacity;
                for node in attached {
                    let PowerRole::Consumer { demand, enabled } = node.role else {
                        continue;
                    };
                    component.consumers += 1;
                    let requested = if enabled { demand } else { 0 };
                    component.requested += u32::from(requested);
                    if enabled {
                        component.enabled_consumers += 1;
                    }
                    let status = if !enabled {
                        PowerStatus::Inactive
                    } else if component.source_ids.is_empty() {
                        PowerStatus::Disconnected
                    } else if available < u32::from(demand) {
                        PowerStatus::Overloaded
                    } else {
                        PowerStatus::Powered
                    };
                    let allocated = if status == PowerStatus::Powered {
                        demand
                    } else {
                        0
                    };
                    available -= u32::from(allocated);
                    component.allocated += u32::from(allocated);
                    report.consumers.insert(
                        (kind, node.id),
                        ConsumerPower {
                            component: id,
                            status,
                            demand,
                            requested,
                            allocated,
                        },
                    );
                }
                report.components.insert(id, component);
            }
        }
        Ok(report)
    }
}

#[cfg(test)]
#[path = "power_tests.rs"]
mod tests;
