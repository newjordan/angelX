//! Hits, and bosses grown desperate.
//!
//! A monster that has just been hit shows it for a few ticks (`hurt`, read
//! off a drop in health, so every blow counts however it landed). A boss
//! (a guardian, the dragon, the Pit Lord) below a third of its health starts
//! to flash, slowly at first and ever faster toward death; its beats come
//! sooner; and it begins a last stand: a ring that closes round the knights
//! with one lane left open, and past the half of that, three spiralling
//! arms. Each is shown on the boss for a second before it lands, so a
//! knight who watches can always step out of it. A guardian in its own
//! room only flashes: its rage and its charge-ups are its end (`bosses`).

use super::*;

/// The share of its health under which a boss grows desperate.
pub const FROM: f32 = 0.35;
/// Ticks a hit's flash lasts.
pub const HIT_FLASH: u8 = 6;

/// The last stand's cycle, in ticks, and where its beats fall in it.
pub const CYCLE: u32 = 165;
/// The ring shows for this long before it flies.
pub const RING_WARN: u32 = 36;
/// The arms show from here, fire from `ARMS_FROM`, until `ARMS_TO`.
pub const ARMS_WARN: u32 = 66;
pub const ARMS_FROM: u32 = 90;
pub const ARMS_TO: u32 = 132;
/// The arms take hold at this much desperation.
pub const ARMS_AT: f32 = 0.5;
const RING_SHOTS: u32 = 18;
/// The open lane, in steps of the ring, toward the nearest knight.
const RING_LANE: u32 = 3;

impl Enemy {
    /// A guardian, the dragon or the Pit Lord.
    pub fn is_boss(&self) -> bool {
        self.boss.is_some() || matches!(self.kind, EnemyKind::Dragon | EnemyKind::PitTyrant)
    }

    /// 0 until a boss is under a third of its health, then rising to 1 at
    /// the last of it.
    pub fn desperation(&self) -> f32 {
        if !self.is_boss() || self.max_hp == 0 || self.hp == 0 {
            return 0.0;
        }
        let share = self.hp as f32 / self.max_hp as f32;
        ((FROM - share) / FROM).clamp(0.0, 1.0)
    }

    /// Ticks between the flash's turns, if it is flashing: slow at first,
    /// quick near the end.
    pub fn flash_every(&self) -> Option<u32> {
        let d = self.desperation();
        (d > 0.0).then(|| (40.0 - 32.0 * d * d).round().max(8.0) as u32)
    }

    /// The boss flashes bright on this tick of the run.
    pub fn flashing(&self, tick: u64) -> bool {
        self.flash_every()
            .is_some_and(|every| (tick as u32 / every).is_multiple_of(2))
    }

    /// A beat of `every` ticks, as often again at the last: a desperate
    /// boss fires up to half as often again, never faster than every four.
    pub fn beat(&self, every: u32) -> u32 {
        let d = self.desperation();
        ((every as f32 * (1.0 - 0.34 * d)).round() as u32).max(4)
    }

    /// The last stand's ring is showing, and how far along (0 to 1).
    pub fn ring_warning(&self) -> Option<f32> {
        (self.desperation() > 0.0 && self.dread > 0 && self.dread % CYCLE < RING_WARN)
            .then(|| (self.dread % CYCLE) as f32 / RING_WARN as f32)
    }

    /// The arms are showing, and how far along (0 to 1).
    pub fn arms_warning(&self) -> Option<f32> {
        let t = self.dread % CYCLE;
        (self.desperation() >= ARMS_AT && (ARMS_WARN..ARMS_FROM).contains(&t))
            .then(|| (t - ARMS_WARN) as f32 / (ARMS_FROM - ARMS_WARN) as f32)
    }
}

fn bolt(enemy: &Enemy, angle: f32, speed: f32, damage: u32, kind: Shot) -> Projectile {
    Projectile {
        x: enemy.x,
        y: enemy.y,
        vx: angle.cos() * speed,
        vy: angle.sin() * speed,
        hostile: true,
        look: None,
        kind,
        damage,
        pierce: 0,
        last_hit: None,
        empowered: false,
        traits: Default::default(),
        ttl: 8 * HZ,
    }
}

/// Current outgoing ring directions, including the party's existing fan.
/// Ordinary attacks that fire before the ring consume the same fan slots.
/// This is a visual prediction; the rules still choose their target on release.
pub fn ring_preview(enemy: &Enemy, boss: &Boss, aim: f32) -> Vec<(f32, bool)> {
    let age = enemy.age + RING_WARN.saturating_sub(enemy.dread % CYCLE);
    let raging = boss.rage > 0.0 && (enemy.hp as f32) < boss.rage * enemy.max_hp as f32;
    let prefix: usize = boss
        .attacks
        .iter()
        .enumerate()
        .filter_map(|(index, attack)| {
            if matches!(
                attack.pattern,
                bosses::Pattern::Named(_) | bosses::Pattern::Ravage
            ) {
                return None;
            }
            let every = if raging {
                (attack.every * 2 / 3).max(4)
            } else {
                attack.every.max(4)
            };
            (age + index as u32 * 11)
                .is_multiple_of(enemy.beat(every))
                .then_some(attack.shots.max(1) as usize)
        })
        .sum();
    let slots = stakes::FAN_SHOTS.saturating_sub(prefix);
    let step = std::f32::consts::TAU / RING_SHOTS as f32;
    let mut angles = Vec::new();
    let mut base = 0;
    for i in 0..RING_SHOTS {
        let angle = aim + (i as f32 + 0.5) * step;
        let off = (angle - aim).sin().atan2((angle - aim).cos()).abs();
        if off < step * RING_LANE as f32 / 2.0 {
            continue;
        }
        angles.push((angle, true));
        if base < slots {
            for k in 1..=enemy.fury {
                let turn = stakes::FAN_ANGLE * f32::from(k);
                angles.push((angle + turn, false));
                angles.push((angle - turn, false));
            }
        }
        base += 1;
    }
    angles
}

/// A desperate boss's step in its last stand. Called once a tick by each
/// boss's own `act`; it does nothing until the boss is under `FROM`.
pub(super) fn last_stand(
    enemy: &mut Enemy,
    target: Option<(f32, f32)>,
    shots: &mut Vec<Projectile>,
    deeds: &mut foes::Deeds,
) {
    let d = enemy.desperation();
    if d <= 0.0 {
        enemy.dread = 0;
        return;
    }
    if enemy.dread == 0 {
        deeds.cues.push("last_stand");
    }
    enemy.dread += 1;
    let Some((tx, ty)) = target else {
        return;
    };
    let t = enemy.dread % CYCLE;
    if t == RING_WARN {
        // The ring closes on the knights, one lane open toward the nearest.
        let aim = (ty - enemy.y).atan2(tx - enemy.x);
        let step = std::f32::consts::TAU / RING_SHOTS as f32;
        for i in 0..RING_SHOTS {
            let angle = aim + (i as f32 + 0.5) * step;
            let off = (angle - aim).sin().atan2((angle - aim).cos()).abs();
            if off < step * RING_LANE as f32 / 2.0 {
                continue;
            }
            shots.push(bolt(enemy, angle, 5.0, 14, Shot::Ember));
        }
        deeds.cues.push("last_stand_ring");
    }
    if d >= ARMS_AT && (ARMS_FROM..ARMS_TO).contains(&t) && (t - ARMS_FROM).is_multiple_of(6) {
        let turn = (t - ARMS_FROM) as f32 * 0.16;
        for arm in 0..3 {
            let angle = turn + arm as f32 * std::f32::consts::TAU / 3.0;
            shots.push(bolt(enemy, angle, 4.5, 10, Shot::Orb));
        }
    }
}

impl Run {
    /// Read the tick's hits off the monsters' health: a drop since last
    /// tick starts the flash; the flash runs down by itself.
    pub(super) fn mark_hits(&mut self) {
        for enemy in &mut self.enemies {
            if enemy.seen_hp == 0 {
                enemy.seen_hp = enemy.hp;
            }
            if enemy.hp < enemy.seen_hp {
                enemy.hurt = HIT_FLASH;
            } else {
                enemy.hurt = enemy.hurt.saturating_sub(1);
            }
            enemy.seen_hp = enemy.hp;
        }
    }
}
