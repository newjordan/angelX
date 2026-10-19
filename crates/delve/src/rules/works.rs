//! Everything the skills, the perils, the Boss Key and the Wanted hall keep
//! on a run, in one place: a single field on `Run`, a single field on the
//! mirror's `Live`, and one reset each time the party changes floor or room.
//!
//! - `skills`: gas, fire, mines and barricades every knight can raise.
//! - `perils`: the rooms' own fire jets, gas vents, ember cracks and
//!   crumbling floors, which hurt knights and monsters alike.
//! - `keyring`: the mini-boss's Boss Key and the boss door it opens.
//! - `outlaw`: the Wanted hall, a side-on showdown on every floor.

use super::*;

/// Caps on what the lists may hold, so a mirror's packet stays small and a
/// hostile snapshot cannot grow them.
const MAX_EFFECTS: usize = 48;

/// The run's skill objects, room perils, key and showdown state.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Works {
    /// The floor (raid, depth) these were laid for; another floor starts
    /// them afresh.
    #[serde(default)]
    pub floor: Option<(u64, u32)>,
    /// Each knight's skill bar.
    #[serde(default)]
    pub kits: BTreeMap<u32, skills::Kit>,
    /// This room's clouds of gas, burning ground, mines and barricades.
    #[serde(default)]
    pub clouds: Vec<skills::Cloud>,
    #[serde(default)]
    pub blazes: Vec<skills::Blaze>,
    #[serde(default)]
    pub mines: Vec<skills::Mine>,
    #[serde(default)]
    pub barricades: Vec<skills::Barricade>,
    /// Shields in the air.
    #[serde(default)]
    pub flights: Vec<skills::Flight>,
    /// Blast, flash and dust animations playing out.
    #[serde(default)]
    pub booms: Vec<skills::Boom>,
    /// Monsters poisoned or burning, by id.
    #[serde(default)]
    pub banes: Vec<skills::Ailment>,
    /// This room's own dangers.
    #[serde(default)]
    pub perils: Vec<perils::Peril>,
    /// This room's gauntlet: a bridge and its axes, hammers, a boulder's
    /// lane, a saw's rail. Left out when there is none, so a room without
    /// one is written as it always was.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub gauntlets: Vec<gauntlets::Gauntlet>,
    /// The floor's Boss Key and its door.
    #[serde(default)]
    pub key: keyring::Key,
    /// The Wanted hall's showdown.
    #[serde(default)]
    pub outlaw: outlaw::Showdown,
}

impl Works {
    /// Bounded and finite, as every mirrored list must be.
    pub fn valid(&self) -> bool {
        let finite = |x: f32, y: f32| x.is_finite() && y.is_finite();
        self.kits.len() <= MAX_PLAYERS as usize
            && self.clouds.len() <= MAX_EFFECTS
            && self.blazes.len() <= MAX_EFFECTS
            && self.mines.len() <= MAX_EFFECTS
            && self.barricades.len() <= MAX_EFFECTS
            && self.booms.len() <= MAX_EFFECTS
            && self.flights.len() <= MAX_PLAYERS as usize
            && self
                .flights
                .iter()
                .all(|f| f.x.is_finite() && f.y.is_finite() && f.struck.len() <= 64)
            && self.banes.len() <= 128
            && self.perils.len() <= MAX_EFFECTS
            && self.gauntlets.len() <= MAX_EFFECTS
            && self.gauntlets.iter().all(gauntlets::Gauntlet::valid)
            && self.clouds.iter().all(|c| finite(c.x, c.y))
            && self.clouds.iter().filter(|c| c.wall()).count() <= signatures::WALL_MAX
            && self
                .clouds
                .iter()
                .filter(|c| c.wall())
                .map(|c| std::f32::consts::PI * c.wall_radius().powi(2))
                .sum::<f32>()
                <= signatures::WALL_AREA
            && self.blazes.iter().all(|b| finite(b.x, b.y))
            && self.mines.iter().all(|m| finite(m.x, m.y))
            && self
                .barricades
                .iter()
                .all(|b| finite(b.a.0, b.a.1) && finite(b.b.0, b.b.1))
            && self.booms.iter().all(|b| finite(b.x, b.y))
            && self.key.valid()
            && self.outlaw.valid()
    }

    /// A room is left behind: its clouds and mines go with it.
    pub(super) fn leave_room(&mut self) {
        self.clouds.clear();
        self.blazes.clear();
        self.mines.clear();
        self.barricades.clear();
        self.booms.clear();
        self.flights.clear();
        self.banes.clear();
        self.perils.clear();
        self.gauntlets.clear();
    }
}

impl Run {
    /// A floor of its own: skills' cooldowns, the key and the showdown start
    /// over. A run saved before the Boss Key existed and already past its
    /// chiefs keeps its boss door open.
    pub(super) fn sync_works(&mut self) {
        let stamp = (self.raid_id, self.dungeon.depth);
        if self.works.floor == Some(stamp) {
            return;
        }
        let legacy = self.works.floor.is_none();
        let kits = std::mem::take(&mut self.works.kits);
        self.works = Works {
            floor: Some(stamp),
            kits: if legacy { BTreeMap::new() } else { kits },
            ..Works::default()
        };
        if self.boss_gates.as_ref().is_some_and(|g| g.unlocked()) {
            self.works.key.door = keyring::Door::Open;
        }
    }

    /// A room is entered: stale objects go, the floor's key and the room's
    /// own dangers are laid.
    pub(super) fn works_enter(&mut self) {
        self.sync_works();
        self.works.leave_room();
        self.arm_perils();
        self.arm_gauntlets();
    }

    /// One tick of the skills, the perils, the key and the showdown, after
    /// monsters and shots have moved.
    pub(super) fn tick_works(&mut self, inputs: &BTreeMap<u32, Input>) {
        self.sync_works();
        self.tick_skills(inputs);
        self.tick_perils();
        self.tick_gauntlets();
        self.tick_key();
        self.tick_outlaw();
    }
}
