//! Summoned creatures a card defines: `summon N <behaviour>` with the card's
//! own `draw summon` look.
//!
//! What they do is chosen from a fixed menu, never written: a card says how
//! many answer and which behaviour, the checker bounds both, and a spell
//! that calls them recharges no faster than they earn (`cards`). So "tiny
//! golden devils that explode" is a spell that calls a swarm in a gold
//! sheet, and a friend's devils are as strong as anyone's.
//!
//! - `swarm` (up to 6): each hunts the nearest monster and bursts on it,
//!   one stroke under the area rule (`aoe`); unspent, it falls in after six
//!   seconds.
//! - `guard` (up to 4): they circle their knight and each catches one shot.
//! - `turret` (up to 2): it stands where it was called and looses a bolt at
//!   the nearest monster in reach, for eight seconds.
//!
//! A knight keeps at most `MAX_PER_KNIGHT`; calling more lets the oldest go.
//! A room entered leaves them behind.

use super::*;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Behaviour {
    Swarm,
    Guard,
    Turret,
}

impl Behaviour {
    pub fn word(self) -> &'static str {
        match self {
            Behaviour::Swarm => "swarm",
            Behaviour::Guard => "guard",
            Behaviour::Turret => "turret",
        }
    }

    pub fn parse(word: &str) -> Option<Behaviour> {
        Some(match word {
            "swarm" | "swarmer" | "swarmers" => Behaviour::Swarm,
            "guard" | "guards" => Behaviour::Guard,
            "turret" | "turrets" => Behaviour::Turret,
            _ => return None,
        })
    }

    /// The most one card may call.
    pub fn most(self) -> u32 {
        match self {
            Behaviour::Swarm => 6,
            Behaviour::Guard => 4,
            Behaviour::Turret => 2,
        }
    }

    /// A summoning spell's least recharge, in seconds, per creature.
    pub fn seconds_each(self) -> u32 {
        match self {
            Behaviour::Swarm => 4,
            Behaviour::Guard => 5,
            Behaviour::Turret => 10,
        }
    }

    /// How long one lasts, in ticks.
    fn life(self) -> u32 {
        match self {
            Behaviour::Swarm => 6 * HZ,
            Behaviour::Guard => 12 * HZ,
            Behaviour::Turret => 8 * HZ,
        }
    }
}

/// A knight keeps at most this many creatures.
pub const MAX_PER_KNIGHT: usize = 6;
const SWARM_SPEED: f32 = 9.0;
/// A swarmer's burst: this much, scaled by its knight's damage, as one
/// stroke on what is within `SWARM_REACH` of it.
pub const SWARM_BURST: u32 = 30;
const SWARM_REACH: f32 = 1.8;
const SWARM_SEEK: f32 = 14.0;
const HOVER: f32 = 1.4;
const GUARD_RADIUS: f32 = 1.7;
const GUARD_CATCH: f32 = 0.8;
const TURRET_EVERY: u32 = 24;
pub const TURRET_DAMAGE: u32 = 14;
const TURRET_RANGE: f32 = 12.0;
const TURRET_SPEED: f32 = 16.0;

/// One creature: whose, from which card (its look), how it behaves, where
/// it is, and how long it has left.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Imp {
    pub owner: u32,
    pub card: String,
    pub behaviour: Behaviour,
    pub x: f32,
    pub y: f32,
    /// Which way it last moved: true for left (it is drawn flipped).
    pub left_facing: bool,
    pub life: u32,
    /// Its place among those called together.
    pub seat: u8,
    pub of: u8,
    pub rearm: u32,
}

impl Run {
    /// Knight `owner` calls the creatures card `card_id` defines.
    pub(super) fn call_imps(&mut self, owner: u32, card_id: &str) {
        let Some((count, behaviour)) = self.book.get(card_id).and_then(Card::imps) else {
            return;
        };
        // Saves also carry cards: keep allocation bounded even for an old
        // or hand-edited card that did not come through today's checker.
        let count = count.clamp(1, behaviour.most());
        let Some(hero) = self.players.get(&owner) else {
            return;
        };
        let (hx, hy, aim) = (hero.x, hero.y, hero.aim_y.atan2(hero.aim_x));
        for seat in 0..count {
            let a = aim + std::f32::consts::TAU * seat as f32 / count as f32;
            let (x, y) = match behaviour {
                Behaviour::Turret => (
                    hx + aim.cos() * 2.0 + (seat as f32 - 0.5) * 1.5 * -aim.sin(),
                    hy + aim.sin() * 2.0 + (seat as f32 - 0.5) * 1.5 * aim.cos(),
                ),
                _ => (hx + a.cos() * HOVER, hy + a.sin() * HOVER),
            };
            self.imps.push(Imp {
                owner,
                card: card_id.to_string(),
                behaviour,
                x,
                y,
                left_facing: a.cos() < 0.0,
                life: behaviour.life(),
                seat: seat as u8,
                of: count as u8,
                rearm: TURRET_EVERY / 2,
            });
        }
        // The oldest let go past the knight's limit.
        let mine = self.imps.iter().filter(|i| i.owner == owner).count();
        let mut extra = mine.saturating_sub(MAX_PER_KNIGHT);
        self.imps.retain(|i| {
            if extra > 0 && i.owner == owner {
                extra -= 1;
                false
            } else {
                true
            }
        });
        self.cues.push("summoned:imps".into());
    }

    /// Every creature's tick: hunt, guard or shoot, and age.
    pub(super) fn tick_imps(&mut self) {
        if self.imps.is_empty() {
            return;
        }
        let mut imps = std::mem::take(&mut self.imps);
        let mut bolts = Vec::new();
        for imp in &mut imps {
            imp.life = imp.life.saturating_sub(1);
            if imp.life == 0 {
                continue;
            }
            let Some(hero) = self
                .players
                .get(&imp.owner)
                .filter(|h| h.hp > 0 && !h.stone)
            else {
                imp.life = 0;
                continue;
            };
            let (hx, hy) = (hero.x, hero.y);
            match imp.behaviour {
                Behaviour::Swarm => {
                    let target = self
                        .enemies
                        .iter()
                        .filter(|e| e.hp > 0 && e.age >= TELEGRAPH)
                        .map(|e| ((e.x - imp.x).hypot(e.y - imp.y), e))
                        .filter(|(d, _)| *d <= SWARM_SEEK)
                        .min_by(|a, b| a.0.total_cmp(&b.0))
                        .map(|(d, e)| (d, e.x, e.y, e.radius()));
                    let goal = match target {
                        Some((d, _, _, r)) if d <= r + 0.4 => {
                            // It bursts: one stroke.
                            let damage = hero.scaled(SWARM_BURST);
                            let at = (imp.x, imp.y);
                            aoe::strike(&mut self.enemies, at, damage, |e| {
                                (e.x - at.0).hypot(e.y - at.1) <= SWARM_REACH + e.radius()
                            });
                            if self.blasts.len() < 16 {
                                self.blasts.push((imp.x, imp.y, BLAST_TICKS));
                            }
                            self.sounds.push("hit_blast");
                            imp.life = 0;
                            continue;
                        }
                        Some((_, ex, ey, _)) => (ex, ey),
                        None => {
                            let a = self.tick as f32 * 2.0 * DT
                                + std::f32::consts::TAU * f32::from(imp.seat)
                                    / f32::from(imp.of.max(1));
                            (hx + a.cos() * HOVER, hy + a.sin() * HOVER)
                        }
                    };
                    let (dx, dy) = (goal.0 - imp.x, goal.1 - imp.y);
                    let d = dx.hypot(dy);
                    if d > 0.05 {
                        let step = (SWARM_SPEED * DT).min(d);
                        imp.x += dx / d * step;
                        imp.y += dy / d * step;
                        if dx.abs() > 0.05 {
                            imp.left_facing = dx < 0.0;
                        }
                    }
                }
                Behaviour::Guard => {
                    let a = self.tick as f32 * 2.5 * DT
                        + std::f32::consts::TAU * f32::from(imp.seat) / f32::from(imp.of.max(1));
                    imp.x = hx + a.cos() * GUARD_RADIUS;
                    imp.y = hy + a.sin() * GUARD_RADIUS;
                    imp.left_facing = a.sin() > 0.0;
                    if let Some(shot) = self.projectiles.iter_mut().find(|p| {
                        p.hostile && p.ttl > 0 && (p.x - imp.x).hypot(p.y - imp.y) <= GUARD_CATCH
                    }) {
                        shot.ttl = 0;
                        imp.life = 0;
                        self.sounds.push("shield_block");
                    }
                }
                Behaviour::Turret => {
                    imp.rearm = imp.rearm.saturating_sub(1);
                    if imp.rearm > 0 {
                        continue;
                    }
                    let target = self
                        .enemies
                        .iter()
                        .filter(|e| e.hp > 0 && e.age >= TELEGRAPH)
                        .map(|e| ((e.x - imp.x).hypot(e.y - imp.y), e.x, e.y))
                        .filter(|(d, _, _)| *d <= TURRET_RANGE)
                        .min_by(|a, b| a.0.total_cmp(&b.0));
                    if let Some((d, ex, ey)) = target {
                        let (ux, uy) = ((ex - imp.x) / d.max(0.01), (ey - imp.y) / d.max(0.01));
                        imp.left_facing = ux < 0.0;
                        imp.rearm = TURRET_EVERY;
                        let look = self
                            .book
                            .get(&imp.card)
                            .and_then(|c| Projectile::look_of(c))
                            .or_else(|| hero.shot_look(&self.book));
                        bolts.push(Projectile {
                            x: imp.x + ux * 0.5,
                            y: imp.y + uy * 0.5,
                            vx: ux * TURRET_SPEED,
                            vy: uy * TURRET_SPEED,
                            hostile: false,
                            kind: Shot::Bolt,
                            look,
                            damage: hero.scaled(TURRET_DAMAGE),
                            pierce: 0,
                            last_hit: None,
                            empowered: false,
                            ttl: (TURRET_RANGE / TURRET_SPEED * HZ as f32).ceil() as u32 + 2,
                            traits: ShotTraits {
                                owner: imp.owner as u8,
                                ..ShotTraits::default()
                            },
                        });
                    }
                }
            }
        }
        imps.retain(|i| i.life > 0);
        self.imps = imps;
        let room = MAX_PROJECTILES.saturating_sub(self.projectiles.len());
        self.projectiles.extend(bolts.into_iter().take(room));
    }
}

#[cfg(test)]
#[path = "../../tests/rules/together_imps__tests.rs"]
mod tests;
