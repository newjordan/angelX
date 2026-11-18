//! How hard the delve pushes back, and how little a hurt knight gets back.
//!
//! Three things keep a built party honest:
//!
//!  - **Monsters grow with the depth and with the party's power.** Their
//!    health follows the floor, the number of knights and how strong the
//!    held cards have made them (`Hero::strength`); what they deal follows
//!    the floor and the party's size. A party meets more of them, firing
//!    thicker, on every floor (`EXTRA_PER_KNIGHT`, `PARTY_FURY`,
//!    `EARLY_PARTY_HIT`). For a lone knight the first two floors stay as
//!    they were, to learn on.
//!  - **A knight's health is capped** at about a hundred and fifty, and
//!    mail turns a share of a hit, not a fixed piece of it.
//!  - **Healing is not free.** Every mend names what it is (`Mend`) and is
//!    cut by the moment: in a fight a half of it, at a boss a third. The
//!    mends that fire on their own (a kill's leech, a vigil, a rune) also
//!    pass a small per-second budget.
//!  - **Lifesteal does not multiply.** A kill's leech, a blow's mend, the
//!    Red Ballad and the marrow sip together give a knight at most
//!    `LEECH_PER_SECOND` a second, on every floor; a tick's kills and hits
//!    mend once however many one blow took (`aoe` for the blows).
//!
//! The balance harness (`balance_sim` in the tests) holds the numbers to
//! their targets.

use super::*;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Stakes {
    #[default]
    Calm,
    Fight,
    Boss,
    Early,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Mend {
    Rest,
    Potion,
    Party,
    Passive,
}

fn worth(how: Mend, stakes: Stakes) -> u32 {
    match (how, stakes) {
        (_, Stakes::Calm | Stakes::Early) => 100,
        (Mend::Rest, Stakes::Fight) => 60,
        (Mend::Rest, Stakes::Boss) => 40,
        (Mend::Potion, Stakes::Fight) => 50,
        (Mend::Potion, Stakes::Boss) => 40,
        (Mend::Party, Stakes::Fight) => 35,
        (Mend::Party, Stakes::Boss) => 20,
        (Mend::Passive, Stakes::Fight) => 30,
        (Mend::Passive, Stakes::Boss) => 15,
    }
}

pub fn landed(amount: u32, how: Mend, stakes: Stakes) -> u32 {
    amount
        .saturating_mul(worth(how, stakes))
        .div_ceil(100)
        .max(1)
}

const PASSIVE_PER_SECOND: u32 = 1;
const PASSIVE_CALM_PER_SECOND: u32 = 2;
pub const LEECH_PER_SECOND: u32 = 2;
const DRAUGHT_REST: u32 = 8 * HZ;
const DRAUGHT_AGAIN: u32 = 30;
pub(super) const WARD_REST: u32 = 15 * HZ;
const HP_OVER_KIT: u32 = 50;

const DEPTH_HEALTH: [u32; 6] = [100, 100, 110, 115, 130, 160];
const DEPTH_HIT: [u32; 6] = [100, 115, 135, 165, 200, 240];
const POWER_HEALTH: f32 = 25.0;
const POWER_HEALTH_MAX: f32 = 120.0;
pub(super) const FURY: [u8; 6] = [0, 0, 0, 3, 3, 3];
pub(super) const EXTRA_FOES: [usize; 6] = [0, 0, 2, 4, 6, 7];
pub(super) const RAMP_FIRST: u32 = 40;
pub(super) const ELITE_CHANCE: [u32; 6] = [0, 0, 30, 45, 55, 60];

const POWER_HIT: f32 = 4.0;
const POWER_HIT_MAX: f32 = 25.0;
pub(super) const PARTY_HIT: [u32; 6] = [60, 100, 120, 135, 140, 145];
pub(super) const EARLY_PARTY_HIT: [u32; 6] = [100, 115, 130, 145, 150, 155];
pub(super) const EXTRA_PER_KNIGHT: usize = 2;
pub(super) const GENTLE_FIRST_TO: u32 = 2;
pub(super) const PARTY_EXTRA: [usize; 6] = [0, 2, 4, 6, 7, 8];
const _: () = {
    assert!(PARTY_EXTRA[1] == EXTRA_PER_KNIGHT);
    assert!(PARTY_EXTRA[2] == 2 * EXTRA_PER_KNIGHT);
    assert!(PARTY_EXTRA[3] == 3 * EXTRA_PER_KNIGHT);
    assert!(PARTY_EXTRA[4] == 3 * EXTRA_PER_KNIGHT + 1);
    assert!(PARTY_EXTRA[5] == 3 * EXTRA_PER_KNIGHT + 2);
};
pub(super) const OPEN_WAVE: [usize; 6] = [usize::MAX, usize::MAX, usize::MAX, usize::MAX, 8, 8];
pub(super) const PARTY_FURY: [u8; 6] = [0, 1, 2, 3, 3, 3];
const BOSS_HIT: u32 = 115;
const BOSS_HEALTH: [u32; 6] = [140, 160, 210, 240, 280, 320];
const BOSS_POWER_HEALTH: f32 = 35.0;
const BOSS_POWER_HEALTH_MAX: f32 = 220.0;

impl Hero {
    pub fn strength(&self) -> f32 {
        let b = &self.bonus;
        let damage = 1.0 + b.damage as f32 / 100.0;
        let rate = 1.0 + b.rate as f32 / 100.0;
        let volley = if b.volley > 0 {
            1.0 + 2.0 / b.volley as f32
        } else {
            1.0
        };
        let arrows = (1 + b.shots) as f32 * volley;
        let reach = 1.0 + 0.12 * b.pierce as f32 + 0.1 * b.chain as f32 + 0.1 * b.bounce as f32;
        let orbit = 1.0 + 0.25 * b.orbit as f32;
        let burst = 1.0 + b.burst as f32 / 150.0;
        damage * rate * arrows * reach * orbit * burst
    }

    pub fn hp_cap(&self) -> u32 {
        let kit = self
            .knight
            .as_deref()
            .and_then(knights::knight)
            .map_or(100, |k| k.max_hp);
        100_u32
            .saturating_add(HP_OVER_KIT)
            .saturating_add(kit.saturating_sub(100) / 2)
            .saturating_add(self.boon.plus)
    }

    pub(super) fn fit_hp(&mut self) {
        let cap = self.hp_cap();
        if self.max_hp > cap {
            self.max_hp = cap;
        }
        self.hp = self.hp.min(self.max_hp);
    }

    pub fn mend(&mut self, amount: u32, how: Mend) -> u32 {
        if self.hp == 0 || amount == 0 {
            return 0;
        }
        let mut gain = landed(amount, how, self.stakes);
        if how == Mend::Potion && matches!(self.stakes, Stakes::Fight | Stakes::Boss) {
            if self.sipped > 0 {
                gain = gain.saturating_mul(DRAUGHT_AGAIN).div_ceil(100).max(1);
            }
            self.sipped = DRAUGHT_REST;
        }
        if how == Mend::Passive && self.stakes != Stakes::Early {
            let budget = if self.stakes == Stakes::Calm {
                PASSIVE_CALM_PER_SECOND
            } else {
                PASSIVE_PER_SECOND
            };
            gain = gain.min(budget.saturating_sub(self.passive));
            self.passive += gain;
        }
        let room = self.max_hp.saturating_sub(self.hp);
        let gain = gain.min(room);
        self.hp += gain;
        #[cfg(any(test, feature = "testing"))]
        HEALED.with(|h| h.borrow_mut()[how as usize] += gain);
        gain
    }

    pub fn leech(&mut self, amount: u32, how: Mend) -> u32 {
        let amount = amount.min(LEECH_PER_SECOND.saturating_sub(self.leeched));
        let gain = self.mend(amount, how);
        self.leeched += gain;
        gain
    }
}

impl Run {
    pub fn stakes(&self) -> Stakes {
        if self.dungeon.depth <= 2 {
            Stakes::Early
        } else if !self.fighting() {
            Stakes::Calm
        } else if self.enemies.iter().any(|e| e.hp > 0 && e.is_boss()) {
            Stakes::Boss
        } else {
            Stakes::Fight
        }
    }

    fn party_strength(&self) -> f32 {
        let living: Vec<f32> = self
            .players
            .values()
            .filter(|h| h.hp > 0 && !h.away)
            .map(Hero::strength)
            .collect();
        if living.is_empty() {
            1.0
        } else {
            living.iter().sum::<f32>() / living.len() as f32
        }
    }

    pub(super) fn toughness(&self) -> u32 {
        let floor = self.deep_health().unwrap_or(DEPTH_HEALTH[(self.dungeon.depth.clamp(1, 6) - 1) as usize]);
        let power = if self.dungeon.depth <= 2 {
            0.0
        } else {
            ((self.party_strength() - 1.0) * POWER_HEALTH).clamp(0.0, POWER_HEALTH_MAX)
        };
        super::tune::Tune::percent(floor + power as u32, self.tune.foe_health)
    }

    pub(super) fn guardian_toughness(&self) -> u32 {
        let floor = self.deep_health().unwrap_or(DEPTH_HEALTH[(self.dungeon.depth.clamp(1, 6) - 1) as usize]);
        let power = ((self.party_strength() - 1.0) * BOSS_POWER_HEALTH).clamp(0.0, BOSS_POWER_HEALTH_MAX);
        super::tune::Tune::percent(floor + power as u32, self.tune.foe_health)
    }

    pub(super) fn boss_health(&self) -> u32 {
        self.deep_boss_health().unwrap_or(BOSS_HEALTH[(self.dungeon.depth.clamp(1, 6) - 1) as usize])
    }

    fn pressure(&self) -> u32 {
        let floor = self.deep_hit().unwrap_or(DEPTH_HIT[(self.dungeon.depth.clamp(1, 6) - 1) as usize]);
        let knights = self.party_len().clamp(1, PARTY_HIT.len().min(EARLY_PARTY_HIT.len()));
        let power = if self.dungeon.depth <= 2 {
            0.0
        } else {
            ((self.party_strength() - 1.0) * POWER_HIT).clamp(0.0, POWER_HIT_MAX)
        };
        let hit = if self.dungeon.depth <= 2 {
            floor * EARLY_PARTY_HIT[knights - 1] / 100
        } else {
            (floor + power as u32) * PARTY_HIT[knights - 1] / 100
        };
        super::tune::Tune::percent(hit, self.tune.foe_hit)
    }

    pub(super) fn set_stakes(&mut self) {
        let stakes = self.stakes();
        let pressure = self.pressure()
            * if stakes == Stakes::Boss {
                BOSS_HIT
            } else {
                100
            }
            / 100;
        let new_second = self.tick.is_multiple_of(u64::from(HZ));
        let ward = self.tune.hit_ward_ticks();
        for hero in self.players.values_mut() {
            hero.stakes = stakes;
            hero.incoming = pressure;
            hero.hit_ward = ward;
            if new_second {
                hero.passive = 0;
                hero.leeched = 0;
            }
            hero.sipped = hero.sipped.saturating_sub(1);
            hero.ward_rest = hero.ward_rest.saturating_sub(1);
            hero.fit_hp();
        }
    }
}

pub(super) fn party_extra(knights: usize) -> usize {
    if knights == 0 { 0 } else { PARTY_EXTRA[knights.min(PARTY_EXTRA.len()) - 1] }
}

pub(super) fn open_wave(knights: usize) -> usize {
    OPEN_WAVE[knights.clamp(1, OPEN_WAVE.len()) - 1]
}

pub(super) fn party_fury(knights: usize) -> u8 {
    PARTY_FURY[knights.clamp(1, PARTY_FURY.len()) - 1]
}

pub(crate) const FAN_SHOTS: usize = 12;
pub(crate) const FAN_ANGLE: f32 = 0.2;

pub(super) fn fan_out(fury: u8, shots: &mut Vec<Projectile>, from: usize) {
    let fired: Vec<Projectile> = shots[from..]
        .iter()
        .filter(|shot| shot.traits.path.beam.is_none())
        .take(FAN_SHOTS)
        .cloned()
        .collect();
    for shot in fired {
        for k in 1..=fury {
            let turn = FAN_ANGLE * f32::from(k);
            for turn in [turn, -turn] {
                let (c, s) = (turn.cos(), turn.sin());
                let mut copy = shot.clone();
                (copy.vx, copy.vy) = (shot.vx * c - shot.vy * s, shot.vx * s + shot.vy * c);
                shots.push(copy);
            }
        }
    }
}

impl Run {
    pub fn floor_progress(&self) -> u32 {
        if let Some(progress) = self.open_progress() {
            return progress;
        }
        let floor = &self.dungeon;
        let n = floor.rooms.len();
        if n <= 1 || self.at >= n {
            return 100;
        }
        let mut dist = vec![usize::MAX; n];
        dist[0] = 0;
        let mut queue = std::collections::VecDeque::from([0usize]);
        while let Some(i) = queue.pop_front() {
            for dir in 0..4 {
                if let Some(j) = floor.neighbour(i, dir)
                    && dist[j] == usize::MAX
                {
                    dist[j] = dist[i] + 1;
                    queue.push_back(j);
                }
            }
        }
        let far = dist.iter().copied().filter(|&d| d != usize::MAX).max().unwrap_or(0);
        if far == 0 || dist[self.at] == usize::MAX {
            return 100;
        }
        (dist[self.at] * 100 / far) as u32
    }

    pub(super) fn ramp(&self) -> usize {
        (RAMP_FIRST + (100 - RAMP_FIRST) * self.floor_progress() / 100) as usize
    }

    pub(super) fn deepen(&mut self) {
        let depth = self.dungeon.depth;
        if depth == 0 || self.at_home_now() {
            return;
        }
        let guarded = self.enemies.iter().any(|e| e.is_boss());
        let index = (depth.clamp(1, 6) - 1) as usize;
        let pack = self.dungeon.pack;
        let gunners: Vec<(EnemyKind, u32)> = pack
            .roster_at(depth)
            .iter()
            .copied()
            .filter(|&(kind, _)| {
                !matches!(
                    kind,
                    EnemyKind::Bat | EnemyKind::Slime | EnemyKind::Spiderling
                )
            })
            .collect();
        let (w, h) = (self.room().width(), self.room().height());
        let alone = guarded && depth <= BOSS_ALONE_TO;
        let doors = self.dungeon.steps().get(self.at).copied().unwrap_or(usize::MAX);
        let gentle = depth <= GENTLE_FIRST_TO && doors <= 1;
        let extra = (if guarded { 0 } else { EXTRA_FOES[index] }
            + if gentle { 0 } else { party_extra(self.party_len()) })
            * if alone { 0 } else if guarded { 100 } else { self.ramp() }
            / 100;
        for _ in 0..extra {
            if gunners.is_empty() {
                break;
            }
            let kind = self.rng.pick(&gunners);
            for _ in 0..12 {
                let (x, y) = if self.side_on() {
                    self.perch(kind.flies())
                } else {
                    (
                        4.0 + self.rng.below(1000) as f32 / 1000.0 * (w - 8.0),
                        4.0 + self.rng.below(1000) as f32 / 1000.0 * (h - 10.0),
                    )
                };
                let far = self
                    .players
                    .values()
                    .all(|hero| (hero.x - x).hypot(hero.y - y) > 7.0);
                if far && self.spawn_at(kind, x, y).is_some() {
                    break;
                }
            }
        }
        if depth >= 3
            && !guarded
            && !self.enemies.iter().any(|e| e.hp > 0 && e.elite)
            && doors >= super::pacing::ELITE_FROM
            && self.rng.chance(ELITE_CHANCE[index])
            && let Some(elite) = self
                .enemies
                .iter_mut()
                .filter(|e| {
                    e.hp > 0 && !e.elite && e.kind != EnemyKind::Dummy && e.kind != EnemyKind::Ward
                })
                .max_by_key(|e| e.kind.hp())
        {
            elite.raise_elite();
            self.cues.push("elite".into());
        }
    }
}

impl Enemy {
    pub(super) fn raise_elite(&mut self) {
        if self.elite {
            return;
        }
        self.elite = true;
        let base = self.base_hp() * 5 / 2;
        self.base_hp = Some(base);
        self.max_hp = self.max_hp * 5 / 2;
        self.hp = self.max_hp;
        self.seen_hp = self.hp;
        self.size = Some(self.kind.radius() * 1.3);
        self.fury = self.fury.saturating_add(1);
    }
}

#[cfg(any(test, feature = "testing"))]
thread_local! {
    pub static HEALED: std::cell::RefCell<[u32; 4]> = const { std::cell::RefCell::new([0; 4]) };
}
