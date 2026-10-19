//! Host-owned impact pauses. Simulation time stands still; transport ticks
//! still arrive, collecting presses for the first step after the hold.
use super::*;

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct State {
    pub left: u32,
    pub cooldown: u32,
    /// The last transport tick held, including the final tick of a hold.
    pub held: bool,
    pub buffered: BTreeMap<u32, Input>,
    #[serde(default, skip_serializing_if = "zero")]
    pending: u32,
}

#[derive(Clone, Copy)]
pub(super) enum Impact {
    Heavy,
    Phase,
    EliteKill,
    BossKill,
}

impl State {
    pub fn valid(&self) -> bool {
        self.left <= 12
            && self.cooldown <= 60
            && self.pending <= 12
            && self.buffered.len() <= MAX_PLAYERS as usize
            && self
                .buffered
                .iter()
                .all(|(&id, i)| (1..=MAX_PLAYERS).contains(&id) && i.valid())
    }
    pub fn quiet(&self) -> bool {
        *self == Self::default()
    }
    pub fn frozen(&self) -> bool {
        self.left > 0 || self.held
    }

    pub(super) fn request(&mut self, tune: &tune::Tune, impact: Impact) {
        if tune.hitstop == 0 {
            return;
        }
        let (ticks, kill) = match impact {
            Impact::Heavy => (tune.hitstop_heavy, false),
            Impact::Phase => (tune.hitstop_phase, false),
            Impact::EliteKill => (tune.hitstop_elite_kill, true),
            Impact::BossKill => (tune.hitstop_boss_kill, true),
        };
        if kill || self.cooldown == 0 {
            self.pending = self.pending.max(ticks);
        }
    }

    pub(super) fn finish(&mut self, tune: &tune::Tune) -> bool {
        let ticks = std::mem::take(&mut self.pending);
        if ticks == 0 {
            return false;
        }
        self.left = ticks;
        self.cooldown = tune.hitstop_cooldown;
        true
    }

    /// Latest held controls, with valid action presses during the hold
    /// coalesced like commands that arrive together on the engine wire.
    pub(super) fn inputs(
        &mut self,
        tune: &tune::Tune,
        players: &BTreeMap<u32, Hero>,
        inputs: &BTreeMap<u32, Input>,
    ) -> Option<BTreeMap<u32, Input>> {
        if tune.hitstop == 0 {
            // Disabling a running hold releases its buffered presses too.
            self.left = 0;
            self.cooldown = 0;
            self.held = false;
            self.pending = 0;
        }
        self.cooldown = self.cooldown.saturating_sub(1);
        self.buffered.retain(|id, _| players.contains_key(id));
        if self.left > 0 {
            for (&id, &input) in inputs
                .iter()
                .filter(|(id, i)| players.contains_key(id) && i.valid())
            {
                let press = Input {
                    move_x: if input.dash { input.move_x } else { 0 },
                    move_y: if input.dash { input.move_y } else { 0 },
                    ..input
                };
                if press != Input::default() {
                    let old = self.buffered.entry(id).or_default();
                    *old = command::lay_over(*old, &press);
                }
            }
            self.left -= 1;
            self.held = true;
            return None;
        }
        self.held = false;
        let mut resumed = inputs.clone();
        for (id, press) in std::mem::take(&mut self.buffered) {
            let now = resumed.entry(id).or_default();
            *now = command::lay_over(press, &Some(*now).filter(|i| i.valid()).unwrap_or_default());
        }
        Some(resumed)
    }
}

/// Large bodies retain their authored footprint: party/depth health
/// scaling never turns a small monster into an elite for hit-stop.
pub(super) fn elite(enemy: &Enemy) -> bool {
    !matches!(enemy.kind, EnemyKind::Dummy | EnemyKind::Ward) && enemy.radius() >= 1.0
}

impl Run {
    pub(super) fn boss_phase(&self, enemy: &Enemy) -> u32 {
        enemy
            .boss
            .and_then(|i| self.bosses.get(usize::from(i)))
            .and_then(|b| b.phases.as_ref())
            .map_or(0, |p| {
                (enemy.pattern_volleys / p.volleys.max(1)) % p.chain.len().max(1) as u32
            })
    }

    pub(super) fn finish_hitstop(&mut self) {
        if self.hitstop.finish(&self.tune) {
            // Existing hit flashes, impact sparks and shake hold with the frame.
            self.shake = self.shake.max(feel::SHAKE_SHOT);
        }
    }
}

#[cfg(test)]
#[path = "../../tests/rules/together_hitstop__tests.rs"]
mod tests;
