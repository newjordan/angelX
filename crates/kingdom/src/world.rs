//! Persistent geography and earned frontier parcels. Terrain is a versioned,
//! deterministic function: saves retain a seed, owned parcels and cleared trees,
//! not thousands of mutable cells. Economy commands own all payment/revisions.
use crate::{Stock, Tile};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeSet, VecDeque};

pub const PARCEL_WIDTH: i16 = 24;
pub const PARCEL_HEIGHT: i16 = 18;
pub const WORLD_WIDTH: i16 = PARCEL_WIDTH * 3;
pub const WORLD_HEIGHT: i16 = PARCEL_HEIGHT * 3;
pub type Parcel = (i16, i16);

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Terrain {
    Meadow,
    Forest,
    Stone,
    ClayBank,
    River,
    /// A shallow crossing. Roads can cross without an advanced bridge, so
    /// no survey or construction chain depends on a resource beyond a river.
    Ford,
}
impl Terrain {
    pub fn name(self) -> &'static str {
        match self {
            Self::Meadow => "Open meadow",
            Self::Forest => "Timber woodland",
            Self::Stone => "Stone outcrop",
            Self::ClayBank => "Clay bank",
            Self::River => "Flowing river",
            Self::Ford => "Shallow ford",
        }
    }
    pub fn resource(self) -> Option<&'static str> {
        match self {
            Self::Forest => Some("timber"),
            Self::Stone => Some("stone"),
            Self::ClayBank => Some("clay"),
            _ => None,
        }
    }
    pub fn walkable(self) -> bool {
        self != Self::River
    }
    pub fn water(self) -> bool {
        matches!(self, Self::River | Self::Ford)
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct World {
    pub version: u8,
    pub seed: u64,
    pub parcels: BTreeSet<Parcel>,
    /// Missing pre-geography saves retain their entire original build area.
    /// Resource-site restrictions must also be skipped on these legacy plots.
    pub legacy_core: bool,
    pub cleared: BTreeSet<Tile>,
}
impl Default for World {
    fn default() -> Self {
        Self::legacy()
    }
}

/// Production totals are lifetime evidence, not inventory: a charter never
/// consumes them, and demolishing a workshop cannot revoke an earned parcel.
#[derive(Clone, Copy, Debug)]
pub struct ExpansionEvidence<'a> {
    pub produced: &'a Stock,
    pub delivered: &'a Stock,
    pub secured_resources: &'a BTreeSet<String>,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ExpansionRequirement {
    Delivered { resource: String, amount: u32 },
    Produced { resource: String, amount: u32 },
    Claim { resource: String },
}
impl ExpansionRequirement {
    pub fn resource(&self) -> &str {
        match self {
            Self::Delivered { resource, .. }
            | Self::Produced { resource, .. }
            | Self::Claim { resource } => resource,
        }
    }
    pub fn met(&self, evidence: &ExpansionEvidence<'_>) -> bool {
        match self {
            Self::Delivered { resource, amount } => {
                evidence.delivered.get(resource).copied().unwrap_or(0) >= *amount
            }
            Self::Produced { resource, amount } => {
                evidence.produced.get(resource).copied().unwrap_or(0) >= *amount
            }
            Self::Claim { resource } => evidence.secured_resources.contains(resource),
        }
    }
    fn progress(&self, evidence: &ExpansionEvidence<'_>) -> String {
        match self {
            Self::Delivered { resource, amount } => format!(
                "Deliver {amount} {resource} to camp ({}/{amount}).",
                evidence.delivered.get(resource).copied().unwrap_or(0)
            ),
            Self::Produced { resource, amount } => format!(
                "Produce {amount} {resource} in workshops ({}/{amount}).",
                evidence.produced.get(resource).copied().unwrap_or(0)
            ),
            Self::Claim { resource } => format!("Win and secure a {resource} claim in a delve."),
        }
    }
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ExpansionOffer {
    pub parcel: Parcel,
    pub name: String,
    pub cost: Stock,
    pub requirement: ExpansionRequirement,
    pub objective: String,
    pub blocked: Option<String>,
}

fn adjacent((x, y): (i16, i16)) -> [(i16, i16); 4] {
    [(x - 1, y), (x + 1, y), (x, y - 1), (x, y + 1)]
}
fn parcel_inside((x, y): Parcel) -> bool {
    (0..3).contains(&x) && (0..3).contains(&y)
}
fn mix(mut n: u64) -> u64 {
    n = n.wrapping_add(0x9e3779b97f4a7c15);
    n = (n ^ (n >> 30)).wrapping_mul(0xbf58476d1ce4e5b9);
    n = (n ^ (n >> 27)).wrapping_mul(0x94d049bb133111eb);
    n ^ (n >> 31)
}

impl World {
    pub fn new(seed: u64) -> Self {
        Self {
            version: 1,
            seed,
            parcels: BTreeSet::from([(0, 0)]),
            legacy_core: false,
            cleared: BTreeSet::new(),
        }
    }
    pub fn legacy() -> Self {
        Self {
            legacy_core: true,
            ..Self::new(0)
        }
    }
    pub fn bounds(&self) -> (i16, i16) {
        (WORLD_WIDTH, WORLD_HEIGHT)
    }
    pub fn contains(&self, (x, y): Tile) -> bool {
        (0..WORLD_WIDTH).contains(&x) && (0..WORLD_HEIGHT).contains(&y)
    }
    pub fn parcel_at(&self, tile: Tile) -> Option<Parcel> {
        self.contains(tile)
            .then_some((tile.0 / PARCEL_WIDTH, tile.1 / PARCEL_HEIGHT))
    }
    pub fn owned(&self, tile: Tile) -> bool {
        self.parcel_at(tile)
            .is_some_and(|p| self.parcels.contains(&p))
    }
    pub fn legacy_plot(&self, tile: Tile) -> bool {
        self.legacy_core && self.parcel_at(tile) == Some((0, 0))
    }
    pub fn terrain(&self, tile: Tile) -> Terrain {
        if self.cleared.contains(&tile) {
            Terrain::Meadow
        } else {
            self.natural_terrain(tile)
        }
    }
    fn natural_terrain(&self, (x, y): Tile) -> Terrain {
        // Callers must use owned/contains for admission. Outside pixels are
        // harmless meadow, allowing renderer sampling at the viewport border.
        if !self.contains((x, y)) || self.legacy_plot((x, y)) {
            return Terrain::Meadow;
        }
        let (lx, ly) = (x % PARCEL_WIDTH, y % PARCEL_HEIGHT);
        let bend = (mix(self.seed ^ (y as u64 / 6)) % 3) as i16 - 1;
        let river = [21 + bend, 45 - bend]
            .iter()
            .any(|&center| x == center || x == center + 1);
        if river {
            return if (8..=10).contains(&ly) {
                Terrain::Ford
            } else {
                Terrain::River
            };
        }
        // Wide dry axes make every owned parcel reachable from every other
        // one. They protect the original camp, road row and expedition gate.
        if (10..=12).contains(&lx) || (8..=10).contains(&ly) {
            return Terrain::Meadow;
        }
        // Guaranteed local sites, close to roads. These do not rely on the
        // noise sample and are available before the first expansion payment.
        if (3..=7).contains(&lx) && (3..=7).contains(&ly) {
            return Terrain::Forest;
        }
        if (15..=18).contains(&lx) && (4..=7).contains(&ly) {
            return Terrain::Stone;
        }
        if (6..=8).contains(&lx) && (11..=13).contains(&ly) {
            return Terrain::ClayBank;
        }
        // River-side clay terraces and low-frequency wooded/mineral clusters
        // vary the rest of each parcel while preserving open building space.
        if (19..=23).contains(&lx) && (11..=15).contains(&ly) {
            return Terrain::ClayBank;
        }
        let patch = mix(self.seed
            ^ (x as u64 / 3).wrapping_mul(0xd6e8feb86659fd93)
            ^ (y as u64 / 3).wrapping_mul(0xa5a3564e27f8862f))
            % 100;
        match patch {
            0..=17 => Terrain::Forest,
            18..=23 => Terrain::Stone,
            _ => Terrain::Meadow,
        }
    }
    /// Manhattan distance matches orthogonal roads and makes placement hints
    /// reproducible. Unowned sites do not supply a workshop over the border.
    pub fn near(&self, tile: Tile, terrain: Terrain, radius: u8) -> bool {
        if !self.contains(tile) {
            return false;
        }
        let r = i16::from(radius.min(12));
        (-r..=r).any(|dy| {
            (-r..=r).any(|dx| {
                dx.abs() + dy.abs() <= r
                    && self.owned((tile.0 + dx, tile.1 + dy))
                    && self.terrain((tile.0 + dx, tile.1 + dy)) == terrain
            })
        })
    }
    pub fn water_adjacent(&self, tile: Tile) -> bool {
        self.owned(tile)
            && !self.terrain(tile).water()
            && adjacent(tile)
                .iter()
                .any(|&p| self.owned(p) && self.terrain(p).water())
    }
    /// The command layer pays its chosen forestry/construction effort first.
    /// Clearing gives no resources here and cannot be repeated for rewards.
    pub fn clear_forest(&mut self, tile: Tile) -> bool {
        if !self.owned(tile) || self.terrain(tile) != Terrain::Forest {
            return false;
        }
        self.cleared.insert(tile)
    }
    pub fn valid(&self) -> bool {
        if self.version != 1
            || self.parcels.is_empty()
            || self.parcels.len() > 9
            || !self.parcels.contains(&(0, 0))
            || self.parcels.iter().any(|&p| !parcel_inside(p))
            || self.cleared.len() > (WORLD_WIDTH * WORLD_HEIGHT) as usize
            || self
                .cleared
                .iter()
                .any(|&p| !self.owned(p) || self.natural_terrain(p) != Terrain::Forest)
        {
            return false;
        }
        let mut seen = BTreeSet::from([(0, 0)]);
        let mut queue = VecDeque::from([(0, 0)]);
        while let Some(p) = queue.pop_front() {
            for next in adjacent(p) {
                if self.parcels.contains(&next) && seen.insert(next) {
                    queue.push_back(next);
                }
            }
        }
        seen.len() == self.parcels.len()
    }
    pub fn offer(
        &self,
        parcel: Parcel,
        evidence: &ExpansionEvidence<'_>,
    ) -> Result<ExpansionOffer, String> {
        if !parcel_inside(parcel) {
            return Err("This parcel is beyond the surveyed valley.".into());
        }
        if self.parcels.contains(&parcel) {
            return Err("This parcel already belongs to the kingdom.".into());
        }
        use ExpansionRequirement::{Claim, Delivered, Produced};
        let (name, payments, requirement, reward) = match parcel {
            (1, 0) => (
                "Claybank Reach",
                [("timber", 12), ("stone", 6)],
                Delivered {
                    resource: "timber".into(),
                    amount: 24,
                },
                "Open the eastern clay terraces and meadow beyond the river.",
            ),
            (0, 1) => (
                "Southwood",
                [("plank", 8), ("stone", 6)],
                Produced {
                    resource: "plank".into(),
                    amount: 12,
                },
                "Extend the timber road into sheltered southern woodland.",
            ),
            (1, 1) => (
                "Millwater Meadow",
                [("plank", 12), ("block", 6)],
                Delivered {
                    resource: "plank".into(),
                    amount: 32,
                },
                "Survey broad meadow and more sites for river-powered workshops.",
            ),
            (2, 0) => (
                "Ironridge",
                [("plank", 16), ("block", 8)],
                Claim {
                    resource: "iron-ore".into(),
                },
                "Secure iron supplies for a fortified settlement on the ridge.",
            ),
            (0, 2) => (
                "Westwood Reserve",
                [("plank", 14), ("food", 12)],
                Delivered {
                    resource: "food".into(),
                    amount: 60,
                },
                "Feed a survey party to open the valley's southern forest.",
            ),
            (2, 1) => (
                "Prism Highlands",
                [("block", 16), ("ingot", 8)],
                Claim {
                    resource: "crystal".into(),
                },
                "Win crystal supplies for a magical quarter in the highlands.",
            ),
            (1, 2) => (
                "River March",
                [("block", 12), ("plank", 20)],
                Delivered {
                    resource: "block".into(),
                    amount: 24,
                },
                "Supply the stone markers for another river crossing and town district.",
            ),
            (2, 2) => (
                "Ember Reach",
                [("block", 24), ("ingot", 16)],
                Claim {
                    resource: "ember".into(),
                },
                "Tame living ember for the kingdom's final frontier foundries.",
            ),
            _ => return Err("The founding parcel cannot be purchased.".into()),
        };
        let connected = adjacent(parcel).iter().any(|p| self.parcels.contains(p));
        let blocked = if !connected {
            Some("Survey a neighboring parcel first so workers can reach this land.".into())
        } else if !requirement.met(evidence) {
            Some(requirement.progress(evidence))
        } else {
            None
        };
        Ok(ExpansionOffer {
            parcel,
            name: name.into(),
            cost: payments.into_iter().map(|(r, n)| (r.into(), n)).collect(),
            objective: format!("{} {reward}", requirement.progress(evidence)),
            requirement,
            blocked,
        })
    }
    /// Evidence and adjacency are checked again at commit. The surrounding
    /// Kingdom command must pay `offer.cost` atomically before committing us.
    pub fn unlock(
        &mut self,
        parcel: Parcel,
        evidence: &ExpansionEvidence<'_>,
    ) -> Result<(), String> {
        let offer = self.offer(parcel, evidence)?;
        if let Some(reason) = offer.blocked {
            return Err(reason);
        }
        self.parcels.insert(parcel);
        Ok(())
    }
}

#[cfg(test)]
#[path = "world_tests.rs"]
mod tests;
