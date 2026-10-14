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

/// The moment, for how much a mend is worth.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Stakes {
    /// Nothing is fighting: a quiet room, the stairs.
    #[default]
    Calm,
    Fight,
    /// A guardian, the dragon or the Pit Lord is up.
    Boss,
    /// The first two floors, where healing is as it always was.
    Early,
}

/// What a mend is, for what it is worth in a fight.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Mend {
    /// A rest: the stairs, a Sanctuary, the privy. Calm moments only, in
    /// effect.
    Rest,
    /// A draught drunk, a heart picked up.
    Potion,
    /// A mend that reaches the party: a censer, a linnet, the Lady's Veil.
    Party,
    /// A mend that fires on its own: a kill's leech, a friend's blow, a
    /// vigil, a rune.
    Passive,
}

/// Percent of a mend that lands, by what it is and the moment.
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

/// What `amount` comes to as a `how` in a `stakes` moment, before the
/// passive budget and the room left in the knight.
pub fn landed(amount: u32, how: Mend, stakes: Stakes) -> u32 {
    (amount * worth(how, stakes)).div_ceil(100).max(1)
}

/// What a knight's passive mends may add up to in a second.
const PASSIVE_PER_SECOND: u32 = 1;
/// And in a quiet room, where a vigil mends a knight left to rest.
const PASSIVE_CALM_PER_SECOND: u32 = 2;
/// What lifesteal may give a knight in a second, on every floor (past the
/// second floor, in a fight, the passive budget is tighter still).
pub const LEECH_PER_SECOND: u32 = 2;
/// Ticks after a draught in a fight during which the next is worth less,
/// and what share of its worth it keeps.
const DRAUGHT_REST: u32 = 8 * HZ;
const DRAUGHT_AGAIN: u32 = 30;
/// Ticks after a ward in which the next holds a third as long.
pub(super) const WARD_REST: u32 = 15 * HZ;
/// The most health a knight is carried to, over their kit's own.
const HP_OVER_KIT: u32 = 50;

/// Monsters' health, in percent of their base, by floor (index 0 is floor
/// 1). The two early floors are as they were.
const DEPTH_HEALTH: [u32; 6] = [100, 100, 110, 115, 130, 160];
/// What monsters deal, in percent, by floor.
const DEPTH_HIT: [u32; 6] = [100, 100, 170, 240, 300, 340];
/// Health added per point of party strength above a fresh knight's, in
/// percent; and the most it can add.
const POWER_HEALTH: f32 = 25.0;
const POWER_HEALTH_MAX: f32 = 120.0;
/// Extra shots in each volley a monster fires, by floor.
pub(super) const FURY: [u8; 6] = [0, 0, 0, 3, 3, 3];
/// More monsters in a room on the deep floors, by floor.
const EXTRA_FOES: [usize; 6] = [0, 0, 2, 4, 6, 7];
/// The chance, in percent, that a deep room has an elite among its foes,
/// by floor.
const ELITE_CHANCE: [u32; 6] = [0, 0, 30, 45, 55, 60];

/// What monsters deal, added per point of strength, and per knight past the
/// first.
const POWER_HIT: f32 = 4.0;
const POWER_HIT_MAX: f32 = 25.0;
/// What the head count makes of it from the third floor, in percent: a lone
/// knight meets as many shooters as three, so each hit is lighter.
const PARTY_HIT: [u32; 4] = [60, 100, 120, 135];
/// What the head count makes of it on the first two floors, in percent: a
/// lone knight's hits stay as they were, a party's land harder.
pub(super) const EARLY_PARTY_HIT: [u32; 4] = [100, 115, 130, 145];
/// More gunners in every room for each knight past the first, on every
/// floor; and as many more in each wave through a door.
pub(super) const EXTRA_PER_KNIGHT: usize = 2;
/// Extra shots beside each shot of a volley, by head count (`fan_out`):
/// a party meets thicker fire on every floor, a guardian's too.
pub(super) const PARTY_FURY: [u8; 4] = [0, 1, 2, 3];
/// What a guardian's shots deal on top of the floor's, in percent.
const BOSS_HIT: u32 = 115;
/// A guardian's health on top of the floor's, in percent, by floor.
const BOSS_HEALTH: [u32; 6] = [100, 100, 200, 240, 310, 390];

impl Hero {
    /// How much harder than a fresh knight this one hits: the damage, rate,
    /// arrows per shot and the rest of what the held cards add, multiplied
    /// as they are in play. 1 for a knight with no cards.
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

    /// The most health this knight can be carried to.
    pub fn hp_cap(&self) -> u32 {
        let kit = self
            .knight
            .as_deref()
            .and_then(knights::knight)
            .map_or(100, |k| k.max_hp);
        // A princess's gift rides above the cap for its floor.
        100 + HP_OVER_KIT + kit.saturating_sub(100) / 2 + self.boon.plus
    }

    /// Hold the knight's health to the cap.
    pub(super) fn fit_hp(&mut self) {
        let cap = self.hp_cap();
        if self.max_hp > cap {
            self.max_hp = cap;
        }
        self.hp = self.hp.min(self.max_hp);
    }

    /// Mend the knight by `amount`, as `how` and the moment make it worth.
    /// Returns what landed.
    pub(super) fn mend(&mut self, amount: u32, how: Mend) -> u32 {
        if self.hp == 0 || amount == 0 {
            return 0;
        }
        let mut gain = landed(amount, how, self.stakes);
        if how == Mend::Potion && matches!(self.stakes, Stakes::Fight | Stakes::Boss) {
            // Draughts one on another do less.
            if self.sipped > 0 {
                gain = (gain * DRAUGHT_AGAIN).div_ceil(100).max(1);
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

    /// Lifesteal: a mend of `amount` as `how`, within what the second's
    /// `LEECH_PER_SECOND` has left. Returns what landed.
    pub(super) fn leech(&mut self, amount: u32, how: Mend) -> u32 {
        let amount = amount.min(LEECH_PER_SECOND.saturating_sub(self.leeched));
        let gain = self.mend(amount, how);
        self.leeched += gain;
        gain
    }
}

impl Run {
    /// Fight, or a boss, or calm.
    pub fn stakes(&self) -> Stakes {
        if self.dungeon.depth <= 2 {
            Stakes::Early
        } else if self.phase != Phase::Fighting {
            Stakes::Calm
        } else if self.enemies.iter().any(|e| e.hp > 0 && e.is_boss()) {
            Stakes::Boss
        } else {
            Stakes::Fight
        }
    }

    /// The living knights' average strength.
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

    /// Monsters' health from here on, in percent of their base (before the
    /// head count): by floor, and by the party's strength.
    pub(super) fn toughness(&self) -> u32 {
        let floor = DEPTH_HEALTH[(self.dungeon.depth.clamp(1, 6) - 1) as usize];
        let power = if self.dungeon.depth <= 2 {
            0.0
        } else {
            ((self.party_strength() - 1.0) * POWER_HEALTH).clamp(0.0, POWER_HEALTH_MAX)
        };
        super::tune::Tune::percent(floor + power as u32, self.tune.foe_health)
    }

    /// A guardian's health on top of the floor's, in percent.
    pub(super) fn boss_health(&self) -> u32 {
        BOSS_HEALTH[(self.dungeon.depth.clamp(1, 6) - 1) as usize]
    }

    /// What monsters deal, in percent: by floor, by the head count, and a
    /// little by the party's strength.
    fn pressure(&self) -> u32 {
        let floor = DEPTH_HIT[(self.dungeon.depth.clamp(1, 6) - 1) as usize];
        let knights = self.party_len().clamp(1, PARTY_HIT.len());
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

    /// Once a tick, before anything acts: tell every knight what the moment
    /// is worth, and what a hit weighs now.
    pub(super) fn set_stakes(&mut self) {
        let stakes = self.stakes();
        // A guardian's blows land a little harder than the floor's.
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

/// The extra fury a party of `knights` brings to every monster's volleys.
pub(super) fn party_fury(knights: usize) -> u8 {
    PARTY_FURY[knights.clamp(1, PARTY_FURY.len()) - 1]
}

/// A volley's shots from `from` on, each with `fury` more beside it, fanned
/// a little to either side.
pub(super) fn fan_out(fury: u8, shots: &mut Vec<Projectile>, from: usize) {
    let fired: Vec<Projectile> = shots[from..].iter().take(12).cloned().collect();
    for shot in fired {
        for k in 1..=fury {
            let turn = 0.2 * f32::from(k);
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
    /// A deep room has more in it, and now and then an elite: tougher, larger,
    /// firing thicker. Every room has more for each knight past the first;
    /// the two early floors are otherwise left as they are.
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
        let extra = if guarded { 0 } else { EXTRA_FOES[index] }
            + EXTRA_PER_KNIGHT * self.party_len().saturating_sub(1);
        for _ in 0..extra {
            if gunners.is_empty() {
                break;
            }
            let kind = self.rng.pick(&gunners);
            for _ in 0..12 {
                // Side-on, a gunner needs ground under it (or wings).
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
            && self.rng.chance(ELITE_CHANCE[index])
            && let Some(elite) = self
                .enemies
                .iter_mut()
                .filter(|e| {
                    e.hp > 0 && !e.elite && e.kind != EnemyKind::Dummy && e.kind != EnemyKind::Ward
                })
                .max_by_key(|e| e.kind.hp())
        {
            elite.elite = true;
            let base = elite.base_hp() * 5 / 2;
            elite.base_hp = Some(base);
            elite.max_hp = elite.max_hp * 5 / 2;
            elite.hp = elite.max_hp;
            elite.seen_hp = elite.hp;
            elite.size = Some(elite.kind.radius() * 1.3);
            elite.fury = elite.fury.saturating_add(1);
            self.cues.push("elite".into());
        }
    }
}

#[cfg(any(test, feature = "testing"))]
thread_local! {
    /// What was mended, by `Mend` as a number, for the balance harness.
    pub static HEALED: std::cell::RefCell<[u32; 4]> = const { std::cell::RefCell::new([0; 4]) };
}
